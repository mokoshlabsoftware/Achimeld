using System.Globalization;
using System.Text.Json;

namespace Achimeld.Browser;

public static class LivePortalIdentity
{
    public static string RecordId(JsonElement record, string field)
    {
        if (!record.TryGetProperty(field, out JsonElement value) || value.ValueKind is not (JsonValueKind.Number or JsonValueKind.String)) throw Rejected();
        string text = value.ToString();
        if (text.Length is < 1 or > 10 || !uint.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out uint id) || text != id.ToString(CultureInfo.InvariantCulture)) throw Rejected();
        return text;
    }
    public static string EditorPrefix(string inputId)
    {
        int suffix = inputId.LastIndexOf('_');
        if (suffix <= 0 || suffix > 128) throw Rejected();
        string prefix = inputId[..suffix];
        if (!char.IsAsciiLetter(prefix[0]) && prefix[0] != '_' || !prefix.All(c => char.IsAsciiLetterOrDigit(c) || c is '_' or '-')) throw Rejected();
        return prefix;
    }
    private static InvalidDataException Rejected() => new("Steam returned an invalid record or editor identity.");
}
