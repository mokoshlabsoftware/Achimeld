using System.Text.Json;
using Achimeld.Core;
using Achimeld.Persistence;

namespace Achimeld.Browser;

public sealed record SteamDeletionTarget(string Kind, string ApiName);
public sealed record SteamDeletionRequest(JsonElement Project, SteamDeletionTarget[] Targets);

// A single-use, explicitly confirmed batch. Missing targets are successful no-ops;
// changed targets and unreadable snapshots are never treated as missing.
public sealed class SteamDeletionSession(ISynchronizationPortal portal)
{
    private bool consumed;
    public async Task<object> RunAsync(SteamDeletionRequest request, uint appId)
    {
        if (consumed) throw new InvalidDataException("Deletion batch already consumed.");
        consumed = true;
        if (appId == 0 || request.Targets.Length is < 1 or > 1000 ||
            request.Targets.Distinct().Count() != request.Targets.Length ||
            request.Targets.Any(t => t.Kind is not ("Achievement" or "Stat") || string.IsNullOrEmpty(t.ApiName)))
            throw new InvalidDataException("Invalid deletion selection.");
        var options = new JsonSerializerOptions(JsonSerializerDefaults.Web) { Converters = { new System.Text.Json.Serialization.JsonStringEnumConverter() } };
        JsonElement expected = request.Project.GetProperty("lastRemoteSnapshot");
        static string Scalar(JsonElement value) => value.ValueKind == JsonValueKind.Object ? value.GetProperty("value").ToString() : value.ToString();
        bool managed = expected.TryGetProperty("steamAppId", out JsonElement expectedApp);
        if (!managed) expectedApp = expected.GetProperty("appId");
        if (Scalar(expectedApp) != appId.ToString(System.Globalization.CultureInfo.InvariantCulture) ||
            (managed ? !expected.GetProperty("isComplete").GetBoolean() : expected.GetProperty("revision").GetString() != "steamworks-configuration-json-v1"))
            throw new InvalidDataException("Deletion requires a complete matching Steam snapshot.");
        JsonElement Expected(SteamDeletionTarget target) => expected.GetProperty(target.Kind == "Stat" ? "stats" : "achievements")
            .EnumerateArray().Single(e => Scalar(e.GetProperty("apiName")) == target.ApiName);
        var mapped = System.Text.Json.Nodes.JsonNode.Parse(request.Project.GetRawText())!.AsObject();
        mapped["lastRemoteSnapshot"] = null;
        mapped["achievements"] = new System.Text.Json.Nodes.JsonArray();
        mapped["stats"] = new System.Text.Json.Nodes.JsonArray();
        var dto = mapped.Deserialize<V1ProjectDto>(options) ?? throw new InvalidDataException();
        var project = V1ProjectMapper.FromDto(dto);
        if (project.AppId.Value != appId)
            throw new InvalidDataException("Deletion requires a matching Steam snapshot.");
        string Resolve(RemoteSnapshot s, SteamDeletionTarget t) => t.Kind == "Stat"
            ? s.Stats.SingleOrDefault(x => x.ApiName.Value == t.ApiName)?.ContentFingerprint ?? SyncFingerprint.Absent
            : s.Achievements.SingleOrDefault(x => x.ApiName.Value == t.ApiName)?.ContentFingerprint ?? SyncFingerprint.Absent;
        foreach (var target in request.Targets)
        {
            string? hash = Expected(target).GetProperty("contentFingerprint").GetString();
            if (hash is null || hash.Length != 64 || !hash.All(char.IsAsciiHexDigit)) throw new InvalidDataException("Unreviewed deletion target.");
        }
        async Task<RemoteSnapshot> Read()
        {
            var snapshot = await portal.ReadAsync(project.AppId);
            SynchronizationSnapshotPolicy.Validate(snapshot, project.AppId);
            return snapshot;
        }
        var results = new List<object>();
        var current = await Read();
        foreach (var target in request.Targets.OrderBy(t => t.Kind == "Achievement" ? 0 : 1))
        {
            current = await Read();
            string actual = Resolve(current, target);
            if (actual == SyncFingerprint.Absent)
            {
                results.Add(new { target.Kind, target.ApiName, status = "AlreadyAbsent" });
                continue;
            }
            if (actual != Expected(target).GetProperty("contentFingerprint").GetString()) throw new InvalidDataException("A selected Steam entry changed; refresh and review again.");
            string? Identity(RemoteSnapshot s) => target.Kind == "Stat" ? s.Stats.SingleOrDefault(x => x.ApiName.Value == target.ApiName)?.SteamId : s.Achievements.SingleOrDefault(x => x.ApiName.Value == target.ApiName)?.SteamId;
            string? expectedIdentity = Expected(target).GetProperty("steamId").GetString();
            if (string.IsNullOrEmpty(expectedIdentity) || expectedIdentity != Identity(current))
                throw new InvalidDataException("Steam target identity changed; refresh and review again.");
            if (target.Kind == "Stat" && current.Achievements.Any(a => a.ProgressStatApiName?.Value == target.ApiName))
                throw new InvalidDataException("A remaining achievement uses the selected stat.");
            var kind = target.Kind == "Stat" ? PortalEntityKind.Stat : PortalEntityKind.Achievement;
            var before = current;
            await portal.ApplyAsync(new(Guid.NewGuid(), project.AppId, new(kind, new(target.ApiName)),
                actual, SyncFingerprint.Absent, new Dictionary<string, string>(), new Dictionary<string, byte[]>(), Delete: true, ExpectedSteamId: Identity(current)));
            current = await Read();
            if (Resolve(current, target) != SyncFingerprint.Absent)
                throw new InvalidDataException("Steam did not confirm deletion; refresh before retrying.");
            SynchronizationSession.RequireOtherTargetsUnchanged(before, current, new(kind, new(target.ApiName)));
            results.Add(new { target.Kind, target.ApiName, status = "Deleted" });
        }
        current = await Read();
        return new { snapshot = V1ProjectMapper.ToDto(project with { LastRemoteSnapshot = current }, "").LastRemoteSnapshot, results };
    }
}
