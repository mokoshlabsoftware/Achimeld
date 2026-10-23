using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace Achimeld.Core;

public sealed record CanonicalField(string Name, object? Value);

public sealed record CanonicalFingerprintResult(
    string EntityFingerprint,
    IReadOnlyDictionary<string, string> FieldFingerprints);

public static class CanonicalFingerprint
{
    public const int Version = 1;

    public static CanonicalFingerprintResult Compute(IEnumerable<CanonicalField> fields)
    {
        ArgumentNullException.ThrowIfNull(fields);
        CanonicalField[] ordered = fields
            .OrderBy(static field => field.Name, StringComparer.Ordinal)
            .ToArray();

        Dictionary<string, string> fieldFingerprints = new(StringComparer.Ordinal);
        StringBuilder entity = new();
        entity.Append("achimeld-canonical-v").Append(Version.ToString(CultureInfo.InvariantCulture)).Append('\n');

        foreach (CanonicalField field in ordered)
        {
            ArgumentException.ThrowIfNullOrWhiteSpace(field.Name);
            string name = Normalize(field.Name);
            string value = Encode(field.Value);
            string line = LengthPrefix(name) + "=" + value + "\n";
            fieldFingerprints.Add(field.Name, Hash(line));
            entity.Append(line);
        }

        return new CanonicalFingerprintResult(
            Hash(entity.ToString()),
            fieldFingerprints);
    }

    public static string ComputeValue(object? value) => Hash(Encode(value));

    public static CanonicalFingerprintResult For(StatDefinition stat, bool includeDisplayOrder = true) =>
        Compute(
        [
            new("aggregated", stat.Aggregated),
            new("apiName", stat.ApiName.Value),
            new("averageRateWindowSeconds", stat.AverageRateWindowSeconds),
            new("defaultValue", stat.DefaultValue),
            new("displayOrder", includeDisplayOrder ? (int?)stat.DisplayOrder : null),
            new("incrementOnly", stat.IncrementOnly),
            new("maximum", stat.Maximum),
            new("maximumChange", stat.MaximumChange),
            new("minimum", stat.Minimum),
            new("setBy", stat.SetBy),
            new("type", stat.Type),
        ]);

    public static CanonicalFingerprintResult For(
        AchievementDefinition achievement,
        SteamApiName? progressStatApiName, bool includeDisplayOrder = true) =>
        Compute(
        [
            new("apiName", achievement.ApiName.Value),
            new("displayOrder", includeDisplayOrder ? (int?)achievement.DisplayOrder : null),
            new("hidden", achievement.Hidden),
            new("progressStatApiName", progressStatApiName?.Value),
            new("setBy", achievement.SetBy),
            // Steam stores an unlock threshold only as part of a progress-stat link.
            // A standalone local value can still drive icon text, but is not exported.
            new("unlockValue", progressStatApiName is null ? null : achievement.UnlockValue),
        ]);

    public static CanonicalFingerprintResult For(RemoteStat stat, bool includeDisplayOrder = true) =>
        Compute(
        [
            new("aggregated", stat.Aggregated), new("apiName", stat.ApiName.Value),
            new("averageRateWindowSeconds", stat.AverageRateWindowSeconds), new("defaultValue", stat.DefaultValue),
            new("displayOrder", includeDisplayOrder ? (int?)stat.DisplayOrder : null),
            new("incrementOnly", stat.IncrementOnly), new("maximum", stat.Maximum),
            new("maximumChange", stat.MaximumChange), new("minimum", stat.Minimum),
            new("setBy", stat.SetBy), new("type", stat.Type),
        ]);

    public static CanonicalFingerprintResult For(RemoteAchievement achievement, bool includeDisplayOrder = true) =>
        Compute(
        [
            new("apiName", achievement.ApiName.Value),
            new("displayOrder", includeDisplayOrder ? (int?)achievement.DisplayOrder : null),
            new("hidden", achievement.Hidden), new("progressStatApiName", achievement.ProgressStatApiName?.Value),
            new("setBy", achievement.SetBy), new("unlockValue", achievement.UnlockValue),
        ]);

    public static CanonicalFingerprintResult For(ProjectLocalizationRow row) =>
        Compute(
        [
            new("ChineseSimplified", row.ChineseSimplified),
            new("ChineseTraditional", row.ChineseTraditional),
            new("English", row.English),
            new("French", row.French),
            new("Japanese", row.Japanese),
            new("Korean", row.Korean),
            new("Polish", row.Polish),
        ]);

    public static string ForCompleteSnapshot(RemoteSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        if (!snapshot.IsComplete)
        {
            throw new IncompleteRemoteSnapshotException();
        }

        List<CanonicalField> fields =
        [
            new("appId", snapshot.AppId.Value),
            new("portalRevision", snapshot.PortalRevisionFingerprint),
            new("supportsDisplayOrder", snapshot.SupportsDisplayOrder),
        ];
        fields.AddRange(snapshot.Stats
            .OrderBy(static value => value.ApiName.Value, StringComparer.Ordinal)
            .Select((value, index) => new CanonicalField(
                $"stat:{index}:{value.ApiName.Value}", value.ContentFingerprint)));
        fields.AddRange(snapshot.Achievements
            .OrderBy(static value => value.ApiName.Value, StringComparer.Ordinal)
            .Select((value, index) => new CanonicalField(
                $"achievement:{index}:{value.ApiName.Value}", value.ContentFingerprint)));
        fields.AddRange(snapshot.Localizations
            .OrderBy(static value => value.EntityType)
            .ThenBy(static value => value.ApiName.Value, StringComparer.Ordinal)
            .ThenBy(static value => value.Field)
            .Select((value, index) => new CanonicalField(
                $"localization:{index}:{value.EntityType}:{value.ApiName.Value}:{value.Field}",
                value.ContentFingerprint)));
        fields.AddRange(snapshot.Icons
            .OrderBy(static value => value.AchievementApiName.Value, StringComparer.Ordinal)
            .Select((value, index) => new CanonicalField(
                $"icon:{index}:{value.AchievementApiName.Value}", value.ContentFingerprint)));
        return Compute(fields).EntityFingerprint;
    }

    private static string Encode(object? value) => value switch
    {
        null => "null:",
        string text => "string:" + LengthPrefix(Normalize(text)),
        bool boolean => boolean ? "bool:true" : "bool:false",
        byte number => Integer(number),
        sbyte number => Integer(number),
        short number => Integer(number),
        ushort number => Integer(number),
        int number => Integer(number),
        uint number => Integer(number),
        long number => Integer(number),
        ulong number => Integer(number),
        decimal number => "decimal:" + number.ToString("G29", CultureInfo.InvariantCulture),
        float number => "float:" + number.ToString("R", CultureInfo.InvariantCulture),
        double number => "double:" + number.ToString("R", CultureInfo.InvariantCulture),
        DateTimeOffset timestamp => "datetime:" + timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
        Guid guid => "guid:" + guid.ToString("D", CultureInfo.InvariantCulture),
        Enum enumeration => "enum:" + enumeration.GetType().FullName + ":" + enumeration.ToString(),
        IEnumerable<string> strings => "strings:[" + string.Join(",", strings.Select(item => LengthPrefix(Normalize(item)))) + "]",
        _ => throw new ArgumentException($"Type '{value.GetType().FullName}' is not supported by canonical fingerprinting.", nameof(value)),
    };

    private static string Integer<T>(T number) where T : IFormattable =>
        "integer:" + number.ToString(null, CultureInfo.InvariantCulture);

    private static string Normalize(string value) =>
        value.Replace("\r\n", "\n", StringComparison.Ordinal)
            .Replace('\r', '\n')
            .Normalize(NormalizationForm.FormC);

    private static string LengthPrefix(string value) =>
        value.Length.ToString(CultureInfo.InvariantCulture) + ":" + value;

    private static string Hash(string value) =>
        Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(value)));
}
