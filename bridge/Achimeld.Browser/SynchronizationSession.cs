using System.Globalization;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;
using Achimeld.Core;
using Achimeld.Persistence;
using Achimeld.Security;

namespace Achimeld.Browser;

public sealed record SyncIconHashes(string Achieved, string Unachieved);
public sealed record PrepareSyncRequest(JsonElement Project, Dictionary<string, Dictionary<string, string>> Catalog,
    Dictionary<string, SyncIconHashes> Icons);
public sealed record SyncChoice(string Key, string Action);
public sealed record BeginSyncRequest(Guid Token, SyncChoice[] Choices);
public sealed record SyncStepRequest(Guid Token, string Key, byte[]? Achieved = null, byte[]? Unachieved = null);
public sealed record FinishSyncRequest(Guid Token);
public sealed record SyncReviewRow(string Key, string Kind, string ApiName, string State, string DefaultAction,
    string[] Actions, object[] Fields, bool HasArtwork, string? UploadError = null);
public sealed record SyncReview(Guid Token, uint AppId, SyncReviewRow[] Rows, string RemoteFingerprint, byte[]? GameBanner = null);
public sealed record SyncStep(string Key, string Action, string ApiName, bool NeedsIcons);
public sealed record SyncArtwork(Guid Id, byte[] Bytes);
public sealed record SyncStepResult(SyncArtwork[] Artwork);

public interface ISynchronizationPortal
{
    Task<RemoteSnapshot> ReadAsync(SteamAppId appId);
    Task ApplyAsync(PortalWriteOperation operation);
    Task<byte[]> DownloadArtworkAsync(Uri uri);
    Task<byte[]?> ReadGameBannerAsync(uint appId) => Task.FromResult<byte[]?>(null);
    Task<bool> VerifyIconsAsync(PortalWriteOperation operation, RemoteIconMetadata? actual) =>
        throw new InvalidDataException("This portal cannot verify uploaded icon content.");
}

// SEC-SYNC-001: the receiver retains the desired data and fresh remote pins.
// Execution takes only selected row identities and bounded, hash-pinned PNG bytes.
// It cannot accept caller-selected URLs, paths, fields, scripts or a generic operation.
public sealed class SynchronizationSession(ISynchronizationPortal portal)
{
    private static readonly JsonSerializerOptions ProjectJson = new(JsonSerializerDefaults.Web) { Converters = { new System.Text.Json.Serialization.JsonStringEnumConverter() } };
    private static readonly string[] Languages = ["English", "Polish", "ChineseSimplified", "ChineseTraditional", "Japanese", "Korean", "French"];
    private JsonObject? raw;
    private AchimeldProject? project;
    private Dictionary<string, Dictionary<string, string>> catalog = [];
    private Dictionary<string, SyncIconHashes> icons = [];
    private RemoteSnapshot? current;
    private SyncReview? review;
    private SyncStep[]? steps;
    private int next;
    private bool terminal;
    private readonly Dictionary<string, JsonObject> renamedAchievements = new(StringComparer.Ordinal);

    private static bool Active(JsonObject entity) => entity["steamSyncStatus"] is null || entity["steamSyncStatus"]!.GetValue<string>() != "Retired";

    public async Task<SyncReview> PrepareAsync(PrepareSyncRequest request, uint pinnedAppId)
    {
        if (review is not null || terminal || pinnedAppId == 0 || request.Project.GetRawText().Length > 16 * 1024 * 1024 ||
            request.Catalog.Count > 100_000 || request.Icons.Count > 10_000) throw Rejected();
        raw = JsonNode.Parse(request.Project.GetRawText())?.AsObject() ?? throw Rejected();
        // The pure legacy mapper only needs synchronization fields. Preserve newer
        // recipe/asset properties in raw; never round-trip them through older DTOs.
        JsonObject mapped = (JsonObject)raw.DeepClone();
        mapped["lastRemoteSnapshot"] = null;
        foreach (JsonNode? achievement in mapped["achievements"]!.AsArray())
        {
            JsonObject entity = achievement?.AsObject() ?? throw Rejected();
            if (entity.TryGetPropertyValue("steamSyncStatus", out JsonNode? status) &&
                (status is not JsonValue text || !text.TryGetValue(out string? syncStatus) || syncStatus is not ("Active" or "Unsynced" or "Retired"))) throw Rejected();
            // Local canvas icons must be removed by the host before they reach
            // the legacy mapper, which does not retain their identity markers.
            foreach (string key in new[] { "styleAchievement", "artImage" })
                if (entity.TryGetPropertyValue(key, out JsonNode? marker) &&
                    (marker is not JsonValue value || !value.TryGetValue(out bool local) || local)) throw Rejected();
            // These host-owned artwork fields are irrelevant to Steam
            // definitions. Newer projects allow null legacy tile selectors;
            // do not feed them through older non-nullable artwork DTOs.
            foreach (string key in new[] { "icon", "atlasId", "standaloneImageId", "unachievedImageId", "generalImageId", "generalAtlasId", "generalTileIndex", "generalImageEntries" })
                entity.Remove(key);
        }
        V1ProjectDto dto = mapped.Deserialize<V1ProjectDto>(ProjectJson) ?? throw Rejected();
        if (dto.SteamAppId != pinnedAppId || dto.SchemaVersion != 1 || dto.ProjectId == Guid.Empty ||
            dto.Stats.Count + dto.Achievements.Count > 10_000) throw Rejected();
        project = V1ProjectMapper.FromDto(dto);
        catalog = request.Catalog;
        icons = request.Icons;
        foreach (var row in catalog.Values)
            if (row.Count > 9 || row.Any(p => p.Value.Length > 1_048_576 || !Languages.Contains(p.Key) && p.Key != "Context")) throw Rejected();
        foreach (SyncIconHashes pair in icons.Values)
            if (!Hash(pair.Achieved) || !Hash(pair.Unachieved)) throw Rejected();
        current = await ReadAsync();
        // Match only a unique, stable Steam identity from the previous verified
        // snapshot. Names or artwork resemblance never establish a rename.
        JsonObject? previousSnapshot = raw["lastRemoteSnapshot"] as JsonObject;
        bool sameDestination = previousSnapshot?["steamAppId"]?.GetValue<uint>() == pinnedAppId && previousSnapshot["isComplete"]?.GetValue<bool>() == true;
        JsonObject[] previousAchievements = sameDestination ? previousSnapshot!["achievements"]?.AsArray().OfType<JsonObject>().ToArray() ?? [] : [];
        var localAchievements = Entities(false).OfType<JsonObject>().ToDictionary(a => a["apiName"]!.GetValue<string>(), StringComparer.Ordinal);
        var retiredNames = localAchievements.Where(a => !Active(a.Value)).Select(a => a.Key).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var remoteNames = current.Achievements.Select(a => a.ApiName.Value).ToHashSet(StringComparer.Ordinal);
        var remoteIdCounts = current.Achievements.Where(a => !string.IsNullOrEmpty(a.SteamId)).GroupBy(a => a.SteamId!).ToDictionary(g => g.Key, g => g.Count(), StringComparer.Ordinal);
        var identityHistory = previousAchievements.Concat(raw["syncBaselines"]!.AsArray().OfType<JsonObject>()
            .Where(b => b["entityType"]?.GetValue<string>() == "Achievement" && b["steamId"] is not null));
        var predecessors = identityHistory.Where(a => !string.IsNullOrEmpty(a["steamId"]?.GetValue<string>()) &&
                a["apiName"]?.GetValue<string>() is string name && localAchievements.TryGetValue(name, out JsonObject? candidate) && Active(candidate) && !remoteNames.Contains(name))
            .GroupBy(a => a["steamId"]!.GetValue<string>()).ToDictionary(g => g.Key,
                g => g.Select(a => a["apiName"]!.GetValue<string>()).Distinct(StringComparer.Ordinal).ToArray(), StringComparer.Ordinal);
        foreach (RemoteAchievement remote in current.Achievements)
        {
            if (string.IsNullOrEmpty(remote.SteamId) || localAchievements.ContainsKey(remote.ApiName.Value) || remoteIdCounts[remote.SteamId] != 1 ||
                !predecessors.TryGetValue(remote.SteamId, out string[]? matches) || matches.Length != 1) continue;
            string oldName = matches[0];
            JsonObject old = localAchievements[oldName];
            renamedAchievements.Add(remote.ApiName.Value, old);
            // Retain the identity even if another selected entry is synced
            // while this rename is skipped and LastRemoteSnapshot advances.
            JsonArray baselines = raw["syncBaselines"]!.AsArray();
            JsonObject? baseline = baselines.OfType<JsonObject>().SingleOrDefault(b => b["localId"]?.GetValue<Guid>() == old["id"]!.GetValue<Guid>());
            if (baseline is null)
            {
                baseline = new JsonObject { ["entityType"] = "Achievement", ["localId"] = old["id"]!.DeepClone(), ["apiName"] = oldName,
                    ["fieldFingerprints"] = new JsonObject(), ["verifiedAtUtc"] = JsonValue.Create(DateTimeOffset.UtcNow) };
                baselines.Add(baseline);
            }
            baseline["steamId"] = remote.SteamId;
        }
        List<SyncReviewRow> rows = [];
        foreach (bool stat in new[] { true, false })
        {
            string kind = stat ? "Stat" : "Achievement";
            IEnumerable<string> names = (stat ? project.Stats.Select(s => s.ApiName.Value) : project.Achievements.Select(a => a.ApiName.Value))
                .Concat(stat ? current.Stats.Select(s => s.ApiName.Value) : current.Achievements.Select(a => a.ApiName.Value))
                .Where(name => stat || !retiredNames.Contains(name)).Distinct(StringComparer.Ordinal);
            string[] identities = names.ToArray();
            if (identities.Distinct(StringComparer.OrdinalIgnoreCase).Count() != identities.Length)
                throw new InvalidDataException("Project and Steam API names differ only by case; align their spelling before synchronizing.");
            foreach (string name in identities)
            {
                JsonObject? local = Local(stat, name);
                if (!stat && (local is not null && !Active(local) || renamedAchievements.Values.Any(old => old["apiName"]!.GetValue<string>() == name))) continue;
                string localHash = local is null ? SyncFingerprint.Absent : DefinitionFingerprint(stat, name).EntityFingerprint;
                string remoteHash = Resolve(current, stat ? PortalEntityKind.Stat : PortalEntityKind.Achievement, name);
                Guid? id = local?["id"]?.GetValue<Guid>();
                FieldSyncBaseline? baseline = project.SyncBaselines.SingleOrDefault(b => b.LocalId == id && b.EntityType == (stat ? ProjectEntityType.Stat : ProjectEntityType.Achievement));
                SyncState state = SyncPlanner.Classify(local is null ? null : localHash, remoteHash == SyncFingerprint.Absent ? null : remoteHash, baseline?.EntityFingerprint);
                if (local is not null && remoteHash != SyncFingerprint.Absent)
                    foreach (var text in Localizations(stat, name))
                    {
                        string desired = CanonicalFingerprint.For(text.Row).EntityFingerprint;
                        string actual = Resolve(current, PortalEntityKind.Localization, name, text.Field);
                        if (desired != actual)
                        {
                            string? previous = baseline?.FieldFingerprints.GetValueOrDefault("localization:" + text.Field);
                            SyncState textState = SyncPlanner.Classify(desired, actual, previous);
                            state = Merge(state, textState);
                        }
                    }
                string[] actions = local is null ? ["Skip", "Import"] : remoteHash == SyncFingerprint.Absent ? ["Skip", "Upload"] : ["Skip", "Upload", "Import"];
                string initial = state is SyncState.LocalOnly or SyncState.LocalChanged ? "Upload" : state == SyncState.SteamOnly ? "Import" : "Skip";
                string? uploadError = null;
                if (local is not null)
                {
                    try { ValidateUpload(stat, name); }
                    catch (InvalidDataException exception) { uploadError = exception.Message; actions = actions.Where(a => a != "Upload").ToArray(); if (initial == "Upload") initial = "Skip"; }
                }
                object[] fields = FieldReview(stat, name, local);
                JsonObject? predecessor = renamedAchievements.GetValueOrDefault(name);
                bool renamed = !stat && predecessor is not null;
                if (renamed) fields = fields.Append(new { field = "Previous API name (retained as Retired)", local = predecessor!["apiName"]!.GetValue<string>(), steam = name }).ToArray();
                rows.Add(new(kind + ":" + name, kind, name, renamed ? "RenamedOnSteam" : state.ToString(), initial, actions,
                    fields, !stat && current.Icons.Any(i => i.AchievementApiName.Value == name && i.AchievedPreviewUri is not null), uploadError));
                if (!stat && local is not null)
                {
                    bool available = icons.TryGetValue(name, out SyncIconHashes? pair);
                    RemoteIconMetadata? remoteIcon = current.Icons.SingleOrDefault(i => i.AchievementApiName.Value == name);
                    string? localIcon = pair is null ? null : PairFingerprint(pair);
                    SyncState iconState = baseline?.Icons is IconSyncBaseline previous && localIcon == previous.LocalFingerprint && remoteIcon?.ContentFingerprint == previous.RemoteFingerprint
                        ? SyncState.InSync : remoteIcon is null || remoteIcon.AchievedIdentity is null ? SyncState.LocalOnly
                        : baseline?.Icons is IconSyncBaseline old && remoteIcon.ContentFingerprint == old.RemoteFingerprint ? SyncState.LocalChanged : SyncState.Conflict;
                    rows.Add(new("Icon:" + name, "Icon", name, available ? iconState.ToString() : "ArtworkUnavailable",
                        available && iconState is SyncState.LocalOnly or SyncState.LocalChanged ? "Upload" : "Skip",
                        available ? ["Skip", "Upload"] : ["Skip"], [], available));
                }
            }
        }
        byte[]? banner = await portal.ReadGameBannerAsync(pinnedAppId);
        if (banner is not null && (banner.Length > 2 * 1024 * 1024 || !BoundedRaster.HasSafeDimensions(banner, 1024))) banner = null;
        review = new(Guid.NewGuid(), pinnedAppId, rows.ToArray(), Fingerprint(current), banner);
        return review;
    }

    public async Task<SyncStep[]> BeginAsync(BeginSyncRequest request)
    {
        RequireToken(request.Token);
        if (steps is not null || request.Choices.Length > review!.Rows.Length || request.Choices.Select(c => c.Key).Distinct().Count() != request.Choices.Length) throw Rejected();
        Dictionary<string, SyncReviewRow> rows = review.Rows.ToDictionary(r => r.Key);
        foreach (SyncChoice choice in request.Choices)
            if (!rows.TryGetValue(choice.Key, out SyncReviewRow? row) || !row.Actions.Contains(choice.Action)) throw Rejected();
        RemoteSnapshot fresh = await ReadAsync();
        if (Fingerprint(fresh) != review.RemoteFingerprint) throw new InvalidDataException("Steam changed after review; prepare a new synchronization plan.");
        current = fresh;
        SyncStep[] selectedSteps = request.Choices.Where(c => c.Action != "Skip").Select(c => new SyncStep(c.Key, c.Action, rows[c.Key].ApiName, rows[c.Key].Kind == "Icon"))
            .OrderBy(s => s.Key.StartsWith("Stat:", StringComparison.Ordinal) ? 0 : s.Key.StartsWith("Achievement:", StringComparison.Ordinal) ? 1 : 2).ToArray();
        var uploadStats = selectedSteps.Where(s => s.Key.StartsWith("Stat:", StringComparison.Ordinal) && s.Action == "Upload").Select(s => s.ApiName).ToHashSet(StringComparer.Ordinal);
        var importStats = selectedSteps.Where(s => s.Key.StartsWith("Stat:", StringComparison.Ordinal) && s.Action == "Import").Select(s => s.ApiName).ToHashSet(StringComparer.Ordinal);
        var uploadAchievements = selectedSteps.Where(s => s.Key.StartsWith("Achievement:", StringComparison.Ordinal) && s.Action == "Upload").Select(s => s.ApiName).ToHashSet(StringComparer.Ordinal);
        var importAchievements = selectedSteps.Where(s => s.Key.StartsWith("Achievement:", StringComparison.Ordinal) && s.Action == "Import").Select(s => s.ApiName).ToHashSet(StringComparer.Ordinal);
        var localStatNames = project!.Stats.Select(s => s.ApiName.Value).ToHashSet(StringComparer.Ordinal);
        var remoteStatNames = current.Stats.Select(s => s.ApiName.Value).ToHashSet(StringComparer.Ordinal);
        var remoteAchievementNames = current.Achievements.Select(a => a.ApiName.Value).ToHashSet(StringComparer.Ordinal);
        foreach (SyncStep step in selectedSteps)
        {
            if (step.Key.StartsWith("Achievement:", StringComparison.Ordinal))
            {
                string? progress = step.Action == "Import" ? current.Achievements.Single(a => a.ApiName.Value == step.ApiName).ProgressStatApiName?.Value
                    : project!.Achievements.Single(a => a.ApiName.Value == step.ApiName).ProgressStatId is StatId id ? project!.Stats.Single(s => s.Id == id).ApiName.Value : null;
                if (progress is not null && (step.Action == "Import" ? !localStatNames.Contains(progress) && !importStats.Contains(progress)
                    : !remoteStatNames.Contains(progress) && !uploadStats.Contains(progress)))
                    throw new InvalidDataException("Select the progress stat before its dependent achievement.");
            }
            if (step.NeedsIcons && !remoteAchievementNames.Contains(step.ApiName) && !uploadAchievements.Contains(step.ApiName))
                throw new InvalidDataException("An icon upload requires its achievement definition.");
            if (step.NeedsIcons && importAchievements.Contains(step.ApiName)) throw Rejected();
        }
        ValidateSelectedDefinitions(uploadStats, uploadAchievements);
        steps = selectedSteps;
        return selectedSteps;
    }

    public async Task<SyncStepResult> StepAsync(SyncStepRequest request)
    {
        RequireToken(request.Token);
        if (steps is null || next >= steps.Length || steps[next].Key != request.Key) throw Rejected();
        SyncStep step = steps[next];
        // Consume the step before any asynchronous effect. A failure is terminal and
        // a new session must reconcile a fresh snapshot, never replay this request.
        terminal = true;
        await RequireUnchangedAsync();
        SyncArtwork[] artwork = [];
        if (step.Action == "Import")
        {
            if (request.Achieved is not null || request.Unachieved is not null) throw Rejected();
            artwork = await ImportAsync(step);
        }
        else if (step.NeedsIcons)
        {
            SyncIconHashes pair = icons[step.ApiName];
            ValidatePng(request.Achieved, pair.Achieved);
            ValidatePng(request.Unachieved, pair.Unachieved);
            string desired = PairFingerprint(pair);
            await WriteAsync(new(Guid.NewGuid(), project!.AppId, new(PortalEntityKind.Icon, new(step.ApiName)),
                Resolve(current!, PortalEntityKind.Icon, step.ApiName), desired, new Dictionary<string, string>(),
                new Dictionary<string, byte[]> { ["achievedIcon"] = request.Achieved!, ["unachievedIcon"] = request.Unachieved! }));
            UpdateIconBaseline(step.ApiName, desired);
        }
        else
        {
            if (request.Achieved is not null || request.Unachieved is not null) throw Rejected();
            bool stat = step.Key.StartsWith("Stat:", StringComparison.Ordinal);
            ValidateUpload(stat, step.ApiName);
            PortalEntityKind kind = stat ? PortalEntityKind.Stat : PortalEntityKind.Achievement;
            await WriteAsync(new(Guid.NewGuid(), project!.AppId, new(kind, new(step.ApiName)), Resolve(current!, kind, step.ApiName),
                DefinitionFingerprint(stat, step.ApiName).EntityFingerprint, DefinitionFields(stat, step.ApiName), new Dictionary<string, byte[]>()));
            foreach (var text in Localizations(stat, step.ApiName))
                await WriteAsync(new(Guid.NewGuid(), project.AppId, new(PortalEntityKind.Localization, new(step.ApiName), text.Field),
                    Resolve(current!, PortalEntityKind.Localization, step.ApiName, text.Field), CanonicalFingerprint.For(text.Row).EntityFingerprint,
                    TextFields(text.Row), new Dictionary<string, byte[]>()));
            UpdateBaseline(stat, step.ApiName);
        }
        next++;
        terminal = false;
        return new(artwork);
    }

    public async Task<object> FinishAsync(FinishSyncRequest request)
    {
        RequireToken(request.Token);
        if (steps is null || next != steps.Length) throw Rejected();
        terminal = true;
        await RequireUnchangedAsync();
        raw!["lastRemoteSnapshot"] = JsonSerializer.SerializeToNode(V1ProjectMapper.ToDto(project! with { LastRemoteSnapshot = current }, "").LastRemoteSnapshot, ProjectJson);
        raw["lastVerifiedAtUtc"] = JsonValue.Create(DateTimeOffset.UtcNow);
        return new { project = raw, catalog };
    }

    private async Task WriteAsync(PortalWriteOperation operation)
    {
        await RequireUnchangedAsync();
        if (operation.Target.Kind != PortalEntityKind.Icon && Resolve(current!, operation.Target.Kind, operation.Target.ApiName.Value, operation.Target.LocalizationField) == operation.DesiredFingerprint) return;
        RemoteSnapshot before = current!;
        await portal.ApplyAsync(operation);
        RemoteSnapshot after = await ReadAsync();
        async Task<bool> Matches(RemoteSnapshot snapshot) => operation.Target.Kind == PortalEntityKind.Icon
            ? await portal.VerifyIconsAsync(operation, snapshot.Icons.SingleOrDefault(i => i.AchievementApiName == operation.Target.ApiName))
            : Resolve(snapshot, operation.Target.Kind, operation.Target.ApiName.Value, operation.Target.LocalizationField) == operation.DesiredFingerprint;
        for (int attempt = 0; attempt < 3 && !await Matches(after); attempt++)
        {
            await Task.Delay(250 << attempt);
            after = await ReadAsync();
        }
        if (!await Matches(after))
            throw new InvalidDataException("Steam draft read-back did not match the approved change.");
        RequireOtherTargetsUnchanged(before, after, operation.Target);
        current = after;
    }

    private async Task<RemoteSnapshot> ReadAsync()
    {
        RemoteSnapshot snapshot = await portal.ReadAsync(project!.AppId);
        SynchronizationSnapshotPolicy.Validate(snapshot, project.AppId);
        return snapshot;
    }
    private async Task RequireUnchangedAsync()
    {
        RemoteSnapshot fresh = await ReadAsync();
        if (Fingerprint(fresh) != Fingerprint(current!)) throw new InvalidDataException("Steam changed during synchronization; prepare a fresh plan.");
        current = fresh;
    }
    private void RequireToken(Guid token) { if (terminal || review is null || token == Guid.Empty || token != review.Token) throw Rejected(); }
    private static InvalidDataException Rejected() => new("Reviewed synchronization command rejected.");
    private static bool Hash(string value) => value.Length == 64 && value.All(char.IsAsciiHexDigit);
    public static string Fingerprint(RemoteSnapshot value) => CanonicalFingerprint.ForCompleteSnapshot(value);
    private static string PairFingerprint(SyncIconHashes pair) => CanonicalFingerprint.Compute([new("achievedIdentity", pair.Achieved), new("unachievedIdentity", pair.Unachieved)]).EntityFingerprint;
    private static void ValidatePng(byte[]? bytes, string hash)
    {
        if (bytes is null || bytes.Length is < 24 or > 8 * 1024 * 1024 || !bytes.AsSpan(0, 8).SequenceEqual(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }) ||
            Convert.ToHexStringLower(SHA256.HashData(bytes)) != hash) throw Rejected();
        uint width = System.Buffers.Binary.BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(16, 4));
        uint height = System.Buffers.Binary.BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(20, 4));
        if (width is 0 or > 4096 || height is 0 or > 4096) throw Rejected();
    }
    private JsonArray Entities(bool stat) => raw![stat ? "stats" : "achievements"]!.AsArray();
    private JsonObject? Local(bool stat, string name) => Entities(stat).OfType<JsonObject>().SingleOrDefault(e => e["apiName"]!.GetValue<string>() == name);
    private CanonicalFingerprintResult DefinitionFingerprint(bool stat, string name)
    {
        if (stat) return CanonicalFingerprint.For(project!.Stats.Single(s => s.ApiName.Value == name), current!.SupportsDisplayOrder);
        AchievementDefinition achievement = project!.Achievements.Single(a => a.ApiName.Value == name);
        SteamApiName? progressName = achievement.ProgressStatId is StatId id
            ? project.Stats.Single(s => s.Id == id).ApiName : null;
        return CanonicalFingerprint.For(achievement, progressName, current!.SupportsDisplayOrder);
    }
    private static SyncState Merge(SyncState a, SyncState b) => a == SyncState.InSync ? b : b == SyncState.InSync || a == b ? a : SyncState.Conflict;

    private IEnumerable<(LocalizationField Field, ProjectLocalizationRow Row)> Localizations(bool stat, string name)
    {
        JsonObject entity = Local(stat, name)!;
        decimal? unlockValue = stat ? null : project!.Achievements.Single(a => a.ApiName.Value == name).UnlockValue;
        foreach (var (key, field) in stat ? new[] { ("displayNameLocalizationId", LocalizationField.StatDisplayName) }
            : new[] { ("displayNameLocalizationId", LocalizationField.AchievementDisplayName), ("descriptionLocalizationId", LocalizationField.AchievementDescription) })
        {
            string id = entity[key]!.GetValue<string>();
            if (!catalog.TryGetValue(id, out var values)) throw new InvalidDataException("A localization row is missing.");
            string Text(string column) => values.GetValueOrDefault(column, "");
            ProjectLocalizationRow row = new(new(id), Text("Context"), Text("English"), Text("Polish"), Text("ChineseSimplified"), Text("ChineseTraditional"), Text("Japanese"), Text("Korean"), Text("French"));
            yield return (field, stat ? row : AchievementText.Resolve(row, unlockValue));
        }
    }
    private static Dictionary<string, string> TextFields(ProjectLocalizationRow row) => new()
    {
        ["English"] = row.English, ["Polish"] = row.Polish, ["ChineseSimplified"] = row.ChineseSimplified,
        ["ChineseTraditional"] = row.ChineseTraditional, ["Japanese"] = row.Japanese, ["Korean"] = row.Korean, ["French"] = row.French,
    };
    private Dictionary<string, string> DefinitionFields(bool stat, string name)
    {
        JsonObject local = (stat ? JsonSerializer.SerializeToNode(project!.Stats.Single(s => s.ApiName.Value == name), ProjectJson)
            : JsonSerializer.SerializeToNode(project!.Achievements.Single(a => a.ApiName.Value == name), ProjectJson))!.AsObject();
        string[] keys = stat ? ["type", "setBy", "incrementOnly", "minimum", "maximum", "defaultValue", "maximumChange", "aggregated", "averageRateWindowSeconds"] : ["hidden", "setBy", "unlockValue"];
        Dictionary<string, string> fields = keys.ToDictionary(key => key, key => local[key]?.ToString() ?? "");
        fields["apiName"] = name;
        if (!stat)
        {
            Guid? progress = Local(false, name)!["progressStatId"]?.GetValue<Guid>();
            fields["progressStatApiName"] = progress is null ? "" : project!.Stats.Single(s => s.Id.Value == progress).ApiName.Value;
        }
        return fields;
    }
    private object[] FieldReview(bool stat, string name, JsonObject? local)
    {
        JsonNode? remote = stat ? JsonSerializer.SerializeToNode(current!.Stats.SingleOrDefault(s => s.ApiName.Value == name), ProjectJson)
            : JsonSerializer.SerializeToNode(current!.Achievements.SingleOrDefault(a => a.ApiName.Value == name), ProjectJson);
        Dictionary<string, string>? localFields = local is null ? null : DefinitionFields(stat, name);
        List<object> fields = [];
        foreach (string key in stat ? new[] { "type", "setBy", "incrementOnly", "minimum", "maximum", "defaultValue", "maximumChange", "aggregated", "averageRateWindowSeconds" }
            : new[] { "hidden", "setBy", "unlockValue" })
        {
            string a = localFields?.GetValueOrDefault(key) ?? "—", b = remote is null ? "—" : remote[key]?.ToString() ?? "";
            if (a != b) fields.Add(new { field = key, local = a, steam = b });
        }
        if (!stat)
        {
            string? localProgress = local is null ? null : DefinitionFields(false, name)["progressStatApiName"];
            string? remoteProgress = current!.Achievements.SingleOrDefault(a => a.ApiName.Value == name)?.ProgressStatApiName?.Value;
            if ((localProgress ?? "") != (remoteProgress ?? "")) fields.Add(new { field = "progressStatApiName", local = localProgress ?? "—", steam = remoteProgress ?? "—" });
        }
        var localText = local is null ? new Dictionary<LocalizationField, Dictionary<string, string>>() : Localizations(stat, name).ToDictionary(t => t.Field, t => TextFields(t.Row));
        foreach (LocalizationField field in stat ? new[] { LocalizationField.StatDisplayName } : new[] { LocalizationField.AchievementDisplayName, LocalizationField.AchievementDescription })
            foreach (string language in Languages)
            {
                string desired = localText.GetValueOrDefault(field)?.GetValueOrDefault(language, "") ?? "";
                string actual = current!.Localizations.SingleOrDefault(l => l.ApiName.Value == name && l.Field == field)?.Values.GetValueOrDefault(language, "") ?? "";
                if (actual != desired) fields.Add(new { field = field + " · " + language, local = desired, steam = actual });
            }
        return fields.ToArray();
    }
    private void ValidateUpload(bool stat, string name)
    {
        foreach (var text in Localizations(stat, name))
        {
            if (text.Field != LocalizationField.AchievementDescription && string.IsNullOrWhiteSpace(text.Row.English)) throw new InvalidDataException("English display names are required for upload.");
            if (stat && TextFields(text.Row).Any(p => p.Key != "English" && p.Value.Length > 0)) throw new InvalidDataException("Steam stat display names support English only.");
            if (TextFields(text.Row).Any(p => p.Value.Length > 4096)) throw new InvalidDataException("Upload text must fit within 4096 characters per field.");
        }
    }
    private void ValidateSelectedDefinitions(HashSet<string> uploadStats, HashSet<string> uploadAchievements)
    {
        foreach (string name in uploadStats) ValidateUpload(true, name);
        foreach (string name in uploadAchievements) ValidateUpload(false, name);
        var changedStats = project!.Stats.Where(s => uploadStats.Contains(s.ApiName.Value) && DefinitionFingerprint(true, s.ApiName.Value).EntityFingerprint != Resolve(current!, PortalEntityKind.Stat, s.ApiName.Value)).ToArray();
        HashSet<string> changedStatNames = changedStats.Select(s => s.ApiName.Value).ToHashSet(StringComparer.Ordinal);
        // A stat edit must also leave unselected Steam achievements in range.
        // Those definitions remain remote-owned; validate their actual thresholds.
        var affectedRemote = current!.Achievements.Where(a => !uploadAchievements.Contains(a.ApiName.Value) &&
            a.ProgressStatApiName is SteamApiName progress && changedStatNames.Contains(progress.Value)).Select(a =>
            new AchievementDefinition(new(Guid.NewGuid()), a.ApiName, new("Aa0000"), new("Aa0001"), a.Hidden, a.SetBy,
                project.Stats.Single(s => s.ApiName == a.ProgressStatApiName).Id, a.UnlockValue, 0, a.DisplayOrder, UnlockValueKind: UnlockValueKind.Float));
        var changedAchievements = project.Achievements.Where(a => uploadAchievements.Contains(a.ApiName.Value)).Concat(affectedRemote).ToArray();
        HashSet<StatId> required = changedAchievements.Where(a => a.ProgressStatId is not null).Select(a => a.ProgressStatId!.Value).Concat(changedStats.Select(s => s.Id)).ToHashSet();
        var effectiveStats = project.Stats.Where(s => required.Contains(s.Id)).Select(s =>
        {
            RemoteStat? remote = current!.Stats.SingleOrDefault(r => r.ApiName == s.ApiName);
            return uploadStats.Contains(s.ApiName.Value) || remote is null ? s : s with { Type = remote.Type, Minimum = remote.Minimum, Maximum = remote.Maximum,
                DefaultValue = remote.DefaultValue, MaximumChange = remote.MaximumChange, AverageRateWindowSeconds = remote.AverageRateWindowSeconds };
        }).ToArray();
        var candidate = project with { Stats = effectiveStats, Achievements = changedAchievements };
        if (ProjectValidator.Validate(candidate).Any(issue => issue.Severity == ValidationSeverity.Error &&
            (issue.Code.StartsWith("stat.", StringComparison.Ordinal) || issue.Code.StartsWith("achievement.progress.", StringComparison.Ordinal) || issue.Code.StartsWith("achievement.unlock.", StringComparison.Ordinal))))
            throw new InvalidDataException("Selected definitions have invalid numeric settings or progress dependencies.");
    }
    private static string Resolve(RemoteSnapshot snapshot, PortalEntityKind kind, string name, LocalizationField? field = null) => kind switch
    {
        PortalEntityKind.Stat => snapshot.Stats.SingleOrDefault(s => s.ApiName.Value == name)?.ContentFingerprint,
        PortalEntityKind.Achievement => snapshot.Achievements.SingleOrDefault(a => a.ApiName.Value == name)?.ContentFingerprint,
        PortalEntityKind.Localization => snapshot.Localizations.SingleOrDefault(l => l.ApiName.Value == name && l.Field == field)?.ContentFingerprint,
        PortalEntityKind.Icon => snapshot.Icons.SingleOrDefault(i => i.AchievementApiName.Value == name)?.ContentFingerprint,
        _ => null,
    } ?? SyncFingerprint.Absent;

    private static void RequireOtherTargetsUnchanged(RemoteSnapshot before, RemoteSnapshot after, PortalWriteTarget target)
    {
        bool created = Resolve(before, target.Kind, target.ApiName.Value, target.LocalizationField) == SyncFingerprint.Absent && target.Kind is PortalEntityKind.Stat or PortalEntityKind.Achievement;
        Dictionary<string, string> Values(RemoteSnapshot snapshot)
        {
            Dictionary<string, string> result = [];
            foreach (var s in snapshot.Stats) if (target.Kind != PortalEntityKind.Stat || s.ApiName != target.ApiName) result["s:" + s.ApiName] = s.ContentFingerprint;
            foreach (var a in snapshot.Achievements) if (target.Kind != PortalEntityKind.Achievement || a.ApiName != target.ApiName) result["a:" + a.ApiName] = a.ContentFingerprint;
            foreach (var l in snapshot.Localizations) if (!(l.ApiName == target.ApiName && (created && l.EntityType == (target.Kind == PortalEntityKind.Stat ? ProjectEntityType.Stat : ProjectEntityType.Achievement) || target.Kind == PortalEntityKind.Localization && l.Field == target.LocalizationField))) result["l:" + l.ApiName + ":" + l.Field] = l.ContentFingerprint;
            foreach (var i in snapshot.Icons) if (!(i.AchievementApiName == target.ApiName && (created && target.Kind == PortalEntityKind.Achievement || target.Kind == PortalEntityKind.Icon))) result["i:" + i.AchievementApiName] = i.ContentFingerprint;
            return result;
        }
        var a = Values(before); var b = Values(after);
        if (a.Count != b.Count || a.Any(p => b.GetValueOrDefault(p.Key) != p.Value)) throw new InvalidDataException("An unrelated Steam value changed during the draft save.");
    }

    private void UpdateBaseline(bool stat, string name)
    {
        JsonObject entity = Local(stat, name)!;
        if (!stat) entity["steamSyncStatus"] = "Active";
        Guid id = entity["id"]!.GetValue<Guid>();
        JsonArray baselines = raw!["syncBaselines"]!.AsArray();
        JsonObject? previous = baselines.OfType<JsonObject>().SingleOrDefault(b => b["localId"]!.GetValue<Guid>() == id);
        JsonNode? oldLocalIcon = previous?["localIconFingerprint"]?.DeepClone();
        JsonNode? oldRemoteIcon = previous?["remoteIconFingerprint"]?.DeepClone();
        if (previous is not null) baselines.Remove(previous);
        string fingerprint = Resolve(current!, stat ? PortalEntityKind.Stat : PortalEntityKind.Achievement, name);
        var fields = new Dictionary<string, string>(stat ? CanonicalFingerprint.For(current!.Stats.Single(s => s.ApiName.Value == name), current.SupportsDisplayOrder).FieldFingerprints
            : CanonicalFingerprint.For(current!.Achievements.Single(a => a.ApiName.Value == name), current.SupportsDisplayOrder).FieldFingerprints);
        foreach (var l in current!.Localizations.Where(l => l.ApiName.Value == name && l.EntityType == (stat ? ProjectEntityType.Stat : ProjectEntityType.Achievement))) fields["localization:" + l.Field] = l.ContentFingerprint;
        baselines.Add(new JsonObject { ["entityType"] = stat ? "Stat" : "Achievement", ["localId"] = JsonValue.Create(id), ["apiName"] = name,
            ["steamId"] = stat ? current!.Stats.Single(s => s.ApiName.Value == name).SteamId : current!.Achievements.Single(a => a.ApiName.Value == name).SteamId,
            ["fieldFingerprints"] = JsonSerializer.SerializeToNode(fields, ProjectJson), ["entityFingerprint"] = fingerprint,
            ["verifiedAtUtc"] = JsonValue.Create(DateTimeOffset.UtcNow), ["localIconFingerprint"] = oldLocalIcon, ["remoteIconFingerprint"] = oldRemoteIcon });
    }
    private void UpdateIconBaseline(string name, string local)
    {
        Guid id = Local(false, name)!["id"]!.GetValue<Guid>();
        JsonObject? baseline = raw!["syncBaselines"]!.AsArray().OfType<JsonObject>().SingleOrDefault(b => b["localId"]!.GetValue<Guid>() == id);
        if (baseline is null)
        {
            // Do not label an unselected, different definition as synchronized.
            if (DefinitionFingerprint(false, name).EntityFingerprint != Resolve(current!, PortalEntityKind.Achievement, name)) return;
            UpdateBaseline(false, name);
            baseline = raw["syncBaselines"]!.AsArray().OfType<JsonObject>().Single(b => b["localId"]!.GetValue<Guid>() == id);
        }
        baseline["localIconFingerprint"] = local;
        baseline["remoteIconFingerprint"] = Resolve(current!, PortalEntityKind.Icon, name);
    }

    private async Task<SyncArtwork[]> ImportAsync(SyncStep step)
    {
        bool stat = step.Key.StartsWith("Stat:", StringComparison.Ordinal);
        JsonObject? existing = Local(stat, step.ApiName);
        JsonObject value = existing is null ? new() : (JsonObject)existing.DeepClone();
        Guid id = existing?["id"]?.GetValue<Guid>() ?? Guid.NewGuid();
        value["id"] = JsonValue.Create(id); value["apiName"] = step.ApiName; value["internalName"] = step.ApiName;
        JsonObject definition = (stat ? JsonSerializer.SerializeToNode(current!.Stats.Single(s => s.ApiName.Value == step.ApiName), ProjectJson)
            : JsonSerializer.SerializeToNode(current!.Achievements.Single(a => a.ApiName.Value == step.ApiName), ProjectJson))!.AsObject();
        foreach (var pair in definition)
            if (pair.Key is not ("apiName" or "steamId" or "contentFingerprint" or "progressStatApiName" or "displayOrder")) value[pair.Key] = pair.Value?.DeepClone();
        value["displayOrder"] ??= Entities(stat).Count;
        if (!stat)
        {
            string? progress = current!.Achievements.Single(a => a.ApiName.Value == step.ApiName).ProgressStatApiName?.Value;
            value["progressStatId"] = progress is null ? null : Local(true, progress)?["id"]?.DeepClone() ?? throw new InvalidDataException("Import the progress stat first.");
            value["unlockValueKind"] = value["unlockValue"]?.ToString().Contains('.') == true ? "Float" : "Integer";
            value["tileIndex"] ??= Entities(false).OfType<JsonObject>().Select(e => e["tileIndex"]?.GetValue<int>() ?? -1).DefaultIfEmpty(-1).Max() + 1;
            if (existing is null)
            {
                int internalId = raw!["nextAchievementInternalId"]?.GetValue<int>() ?? Entities(false).Count;
                value["internalId"] = internalId; raw!["nextAchievementInternalId"] = internalId + 1;
                value["icon"] = new JsonObject { ["layers"] = new JsonArray() };
            }
        }
        foreach (var (key, field) in stat ? new[] { ("displayNameLocalizationId", LocalizationField.StatDisplayName) }
            : new[] { ("displayNameLocalizationId", LocalizationField.AchievementDisplayName), ("descriptionLocalizationId", LocalizationField.AchievementDescription) })
        {
            string loc = value[key]?.GetValue<string>() ?? NewLocalizationId();
            value[key] = loc;
            var remote = current!.Localizations.SingleOrDefault(l => l.ApiName.Value == step.ApiName && l.Field == field);
            catalog[loc] = Languages.ToDictionary(l => l, l => remote?.Values.GetValueOrDefault(l, "") ?? "");
            catalog[loc]["Context"] = "Imported from Steam: " + step.ApiName + " " + field;
        }
        if (existing is not null) Entities(stat).Remove(existing);
        Entities(stat).Add(value);
        List<SyncArtwork> artwork = [];
        if (!stat && current!.Icons.SingleOrDefault(i => i.AchievementApiName.Value == step.ApiName) is RemoteIconMetadata icon)
        {
            foreach (var pair in new[] { (Uri: icon.AchievedPreviewUri, Locked: false), (Uri: icon.UnachievedPreviewUri, Locked: true) })
            {
                if (pair.Uri is null) continue;
                BrowserReadPolicy.RequireIcon(pair.Uri);
                byte[] bytes = await portal.DownloadArtworkAsync(pair.Uri);
                if (bytes.Length > 8 * 1024 * 1024) throw new InvalidDataException("Imported artwork exceeds 8 MiB.");
                string? identity = pair.Locked ? icon.UnachievedIdentity : icon.AchievedIdentity;
                if (identity is null || !Hash(identity) || Convert.ToHexStringLower(SHA256.HashData(bytes)) != identity)
                    throw new InvalidDataException("Steam artwork changed after review; prepare a fresh plan.");
                Guid imageId = Guid.NewGuid();
                raw!["assets"]!["images"]!.AsArray().Add(new JsonObject { ["id"] = JsonValue.Create(imageId), ["displayName"] = step.ApiName + (pair.Locked ? " · Locked" : ""), ["kind"] = "SteamImportedArtwork", ["image"] = null });
                artwork.Add(new(imageId, bytes));
                if (pair.Locked) value["unachievedImageId"] = JsonValue.Create(imageId);
                else value["icon"] = new JsonObject { ["layers"] = new JsonArray(new JsonObject { ["id"] = JsonValue.Create(Guid.NewGuid()), ["kind"] = "StandaloneImage",
                    ["imageId"] = JsonValue.Create(imageId), ["displayName"] = "Steam artwork", ["visible"] = true, ["opacity"] = 1, ["effects"] = new JsonArray() }) };
            }
        }
        UpdateBaseline(stat, step.ApiName);
        if (!stat)
        {
            JsonObject baseline = raw!["syncBaselines"]!.AsArray().OfType<JsonObject>().Single(b => b["localId"]!.GetValue<Guid>() == id);
            baseline["localIconFingerprint"] = null;
            baseline["remoteIconFingerprint"] = null;
            if (renamedAchievements.TryGetValue(step.ApiName, out JsonObject? predecessor))
                predecessor["steamSyncStatus"] = "Retired";
        }
        return artwork.ToArray();
    }
    private string NewLocalizationId()
    {
        string id; do { id = Guid.NewGuid().ToString("N")[..6]; } while (catalog.ContainsKey(id)); return id;
    }
}

public sealed class DriverSynchronizationPortal(PlaywrightChromeDriver driver) : ISynchronizationPortal, ICompletePortalDocumentSource
{
    public Task<byte[]?> ReadGameBannerAsync(uint appId) => driver.ReadGameBannerAsync(appId);
    public Task<ManagedPageDocument> ReadCompleteConfigurationAsync(SteamAppId appId, CancellationToken cancellationToken = default) => driver.ReadCompleteConfigurationAsync(appId);
    public Task<RemoteSnapshot> ReadAsync(SteamAppId appId) => new SteamworksSnapshotReader(this).ReadSnapshotAsync(appId);
    public Task<byte[]> DownloadArtworkAsync(Uri uri) => driver.ReadApprovedArtworkAsync(uri);
    public Task<bool> VerifyIconsAsync(PortalWriteOperation operation, RemoteIconMetadata? actual) => driver.VerifyIconPairAsync(operation, actual);
    public async Task ApplyAsync(PortalWriteOperation operation)
    {
        PortalEntityKind scope = operation.Target.Kind == PortalEntityKind.Localization ? operation.Target.LocalizationField == LocalizationField.StatDisplayName ? PortalEntityKind.Stat : PortalEntityKind.Achievement : operation.Target.Kind;
        driver.BeginReviewedDraft(operation.AppId.Value, scope);
        try { await driver.ApplyLiveOperationAsync(operation); }
        finally { driver.EndReviewedDraft(); }
    }
}
