using System.Globalization;
using System.Text.Json;
using Achimeld.Core;
using Achimeld.Browser;

if (args.Length != 2) throw new ArgumentException("Supply normalized fixture and output JSON paths.");
string fixture = File.ReadAllText(args[0]);
List<object> snapshots = [];
foreach (string revision in new[] { SteamworksSelectorCatalog.SupportedRevision, LiveSteamworksConfiguration.Revision })
{
    var source = new FixtureSource(fixture.Replace(SteamworksSelectorCatalog.SupportedRevision, revision, StringComparison.Ordinal));
    RemoteSnapshot snapshot = await new SteamworksSnapshotReader(source).ReadSnapshotAsync(new SteamAppId(480));
    snapshots.Add(new { revision, fingerprint = CanonicalFingerprint.ForCompleteSnapshot(snapshot),
        stats = snapshot.Stats.Select(s => s.ContentFingerprint).ToArray(),
        achievements = snapshot.Achievements.Select(s => s.ContentFingerprint).ToArray(),
        localizations = snapshot.Localizations.OrderBy(s => s.EntityType).ThenBy(s => s.ApiName.Value, StringComparer.Ordinal).ThenBy(s => s.Field).Select(s => s.ContentFingerprint).ToArray(),
        icons = snapshot.Icons.Select(s => s.ContentFingerprint).ToArray() });
}
string[] numbers = ["1.2300", "0.00001", "-0.00000125", "0.0001", "79228162514264337593543950335", "0.0000000000000000000000000001", "-0.000"];
var decimals = numbers.Select(input => { decimal value = decimal.Parse(input, CultureInfo.InvariantCulture); return new { input, encoded = value.ToString("G29", CultureInfo.InvariantCulture), fingerprint = CanonicalFingerprint.ComputeValue(value) }; }).ToArray();
var unicode = CanonicalFingerprint.Compute([new("😀", "e\u0301\r\n😀"), new("\ue000", new[] {"é", "line\rbreak"})]);
using MemoryStream wire = new();
// Same direct SteamAppId argument and envelope fields as SecureSteamWorkerClient.
await Achimeld.Security.SecurityWire.WriteAsync(wire, new {
    RequestId = Guid.Parse("11111111-2222-3333-4444-555555555555"),
    Operation = "readConfiguration",
    Arguments = JsonSerializer.SerializeToElement(new SteamAppId(480), Achimeld.Security.SecurityWire.Json)
}, CancellationToken.None);
File.WriteAllText(args[1], JsonSerializer.Serialize(new { source="Local C# snapshot reader, canonical fingerprint and SecurityWire implementations", snapshots, decimals, unicodeFingerprint=unicode.EntityFingerprint, workerReadRequestFrameHex=Convert.ToHexStringLower(wire.ToArray()) }, new JsonSerializerOptions { WriteIndented=true }) + "\n");

sealed class FixtureSource(string html) : ICompletePortalDocumentSource
{
    public Task<ManagedPageDocument> ReadCompleteConfigurationAsync(SteamAppId appId, CancellationToken cancellationToken = default) =>
        Task.FromResult(new ManagedPageDocument(new Uri($"https://partner.steamgames.com/apps/achievements/{appId.Value}"), "Stats and Achievements", html));
}
