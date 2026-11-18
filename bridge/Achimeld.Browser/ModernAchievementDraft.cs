using System.Globalization;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Achimeld.Core;
using Microsoft.VisualBasic.FileIO;

namespace Achimeld.Browser;

internal sealed record ModernAchievementRecord(string StatId, string BitId, string? SetId,
    string? NameToken, string? DescriptionToken)
{
    public string Identity => StatId + "_" + BitId;
}

internal static class ModernAchievementDraft
{
    private static readonly (string Project, string Steam)[] Languages =
    [ ("English", "english"), ("Polish", "polish"), ("ChineseSimplified", "schinese"),
      ("ChineseTraditional", "tchinese"), ("Japanese", "japanese"), ("Korean", "koreana"), ("French", "french") ];

    public static Dictionary<string, ModernAchievementRecord> ReadRecords(string json)
    {
        using JsonDocument document = JsonDocument.Parse(json);
        Dictionary<string, ModernAchievementRecord> records = new(StringComparer.Ordinal);
        HashSet<string> identities = new(StringComparer.Ordinal);
        if (!Object(document.RootElement, "schema", out JsonElement schema) ||
            !Collection(schema, "stats", out JsonElement stats)) return records;
        foreach ((string statId, JsonElement stat) in Entries(stats))
        {
            if (Value(stat, "type") != "ACHIEVEMENTS") continue;
            if (!Id(statId) || !Collection(stat, "bits", out JsonElement bits)) return new(StringComparer.Ordinal);
            foreach ((string bitId, JsonElement bit) in Entries(bits))
            {
                if (bit.ValueKind == JsonValueKind.Null) continue;
                string? name = Value(bit, "name");
                string? set = Value(bit, "groupid");
                if (!Id(bitId) || string.IsNullOrEmpty(name) || records.Count >= 5000 ||
                    !string.IsNullOrEmpty(set) && !Id(set)) return new(StringComparer.Ordinal);
                Object(bit, "display", out var display);
                Object(display, "name", out var displayName);
                Object(display, "desc", out var description);
                ModernAchievementRecord record = new(statId, bitId, set is "" or "0" ? null : set,
                    Value(displayName, "token"), Value(description, "token"));
                if (!identities.Add(record.Identity) || !records.TryAdd(name, record)) return new(StringComparer.Ordinal);
            }
        }
        return records;
    }

    public static Dictionary<string, string> Definition(string apiName, ModernAchievementRecord record,
        IReadOnlyDictionary<string, string> fields)
    {
        _ = new SteamApiName(apiName);
        Dictionary<string, string> form = new()
        {
            ["statid"] = record.StatId, ["bitid"] = record.BitId, ["name"] = apiName,
        };
        if (record.SetId is string set) form["groupid"] = set;
        string permission = fields["setBy"] switch
        {
            "GameClient" => "0", "GameServer" => "1", "OfficialGameServer" => "2",
            _ => throw new InvalidDataException("Unsupported achievement permission."),
        };
        if (permission != "0") form["setby"] = permission;
        if (bool.Parse(fields["hidden"])) form["hidden"] = "true";
        string progress = fields["progressStatApiName"];
        if (!string.IsNullOrEmpty(progress))
        {
            _ = new SteamApiName(progress);
            if (!decimal.TryParse(fields["unlockValue"], NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out decimal maximum) || maximum <= 0)
                throw new InvalidDataException("The reviewed achievement progress maximum is invalid.");
            form["progressstat"] = progress;
            form["progressmin"] = "0";
            form["progressmax"] = maximum.ToString("G29", CultureInfo.InvariantCulture);
        }
        return form;
    }

    public static void AddLocalization(Dictionary<string, string> form, ModernAchievementRecord record,
        PortalWriteOperation operation, string localizationCsv)
    {
        if (operation.Target.LocalizationField is not (LocalizationField.AchievementDisplayName or LocalizationField.AchievementDescription))
            throw new InvalidDataException("The reviewed localization field is not an achievement field.");
        using TextFieldParser csv = new(new StringReader(localizationCsv.TrimStart('\uFEFF')));
        csv.SetDelimiters(",");
        csv.HasFieldsEnclosedInQuotes = true;
        HashSet<string> columns = (csv.ReadFields() ?? []).Select(value => value.Trim().Split('(')[0].Trim()).ToHashSet(StringComparer.OrdinalIgnoreCase);
        bool name = operation.Target.LocalizationField == LocalizationField.AchievementDisplayName;
        string token = (name ? record.NameToken : record.DescriptionToken) ?? operation.Target.ApiName.Value + (name ? "_NAME" : "_DESC");
        Dictionary<string, string> values = new() { ["token"] = token };
        foreach ((string project, string steam) in Languages)
            if (operation.Fields.TryGetValue(project, out string? text))
            {
                if (columns.Contains(steam)) values[steam] = text;
                else if (text.Length != 0) throw new UnsupportedPortalStructureException("The reviewed text uses a language that is unavailable in Steamworks.");
            }
        form[name ? "displayname" : "displaydesc"] = JsonSerializer.Serialize(values);
    }

    public static bool MatchesForm(string? contentType, string? body, IReadOnlyDictionary<string, string> fields)
    {
        if (body is null || body.Length > 262_144 || Boundary(contentType) is not string boundary) return false;
        string expected = Parts(boundary, fields) + $"--{boundary}--";
        return body == expected + "\r\n" || body == expected;
    }

    public static bool MatchesIcon(string? contentType, byte[]? body, IReadOnlyDictionary<string, string> fields,
        string fileName, byte[] image)
    {
        if (body is null || body.Length > 8 * 1024 * 1024 + 2048 || Boundary(contentType) is not string boundary) return false;
        byte[] header = Encoding.ASCII.GetBytes(Parts(boundary, fields) +
            $"--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{fileName}\"\r\nContent-Type: image/png\r\n\r\n");
        byte[] trailer = Encoding.ASCII.GetBytes($"\r\n--{boundary}--");
        var remaining = body.AsSpan();
        if (!remaining.StartsWith(header)) return false;
        remaining = remaining[header.Length..];
        if (!remaining.StartsWith(image)) return false;
        remaining = remaining[image.Length..];
        return remaining.SequenceEqual(trailer) || remaining.StartsWith(trailer) && remaining[trailer.Length..].SequenceEqual("\r\n"u8);
    }

    private static string Parts(string boundary, IReadOnlyDictionary<string, string> fields) => string.Concat(fields.Select(field =>
        $"--{boundary}\r\nContent-Disposition: form-data; name=\"{field.Key}\"\r\n\r\n{field.Value}\r\n"));

    private static string? Boundary(string? contentType)
    {
        if (contentType is null || contentType.Length > 180 ||
            !MediaTypeHeaderValue.TryParse(contentType, out var media) || media.MediaType != "multipart/form-data" ||
            media.Parameters.Count != 1 || media.Parameters.Single().Name != "boundary") return null;
        string? value = media.Parameters.Single().Value?.Trim('"');
        return value is { Length: >= 1 and <= 80 } && value.All(static c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_') ? value : null;
    }

    private static bool Id(string value) => ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out ulong id) &&
        id.ToString(CultureInfo.InvariantCulture) == value;

    private static string? Value(JsonElement parent, string key) => parent.ValueKind == JsonValueKind.Object &&
        parent.TryGetProperty(key, out var value) && value.ValueKind is JsonValueKind.String or JsonValueKind.Number
        ? value.ToString() : null;

    private static bool Object(JsonElement parent, string key, out JsonElement value)
    {
        value = default;
        return parent.ValueKind == JsonValueKind.Object && parent.TryGetProperty(key, out value) && value.ValueKind == JsonValueKind.Object;
    }

    private static bool Collection(JsonElement parent, string key, out JsonElement value)
    {
        value = default;
        return parent.ValueKind == JsonValueKind.Object && parent.TryGetProperty(key, out value) &&
            value.ValueKind is JsonValueKind.Object or JsonValueKind.Array;
    }

    private static IEnumerable<(string Key, JsonElement Value)> Entries(JsonElement collection) =>
        collection.ValueKind == JsonValueKind.Object
            ? collection.EnumerateObject().Select(entry => (entry.Name, entry.Value))
            : collection.EnumerateArray().Select((entry, index) => (index.ToString(CultureInfo.InvariantCulture), entry));
}
