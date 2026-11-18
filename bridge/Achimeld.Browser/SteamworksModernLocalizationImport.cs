using System.Text;
using Microsoft.VisualBasic.FileIO;

namespace Achimeld.Browser;

// Builds only the reviewed localization row from Steam's fresh export.
// Other achievements, fields and languages are never sourced from the project.
public static class SteamworksModernLocalizationImport
{
    public static IReadOnlySet<string> KnownApiNames(string exportCsv)
    {
        using TextFieldParser parser = new(new StringReader(exportCsv.TrimStart('\uFEFF')));
        parser.SetDelimiters(",");
        parser.HasFieldsEnclosedInQuotes = true;
        string[] headers = parser.ReadFields() ?? [];
        int apiColumn = Array.FindIndex(headers, header =>
            header.Trim().Split('(')[0].Trim().Equals("api_name", StringComparison.OrdinalIgnoreCase));
        if (apiColumn < 0 || headers.Length is < 3 or > 64)
            throw new UnsupportedPortalStructureException("Steamworks localization columns changed.");
        HashSet<string> names = new(StringComparer.Ordinal);
        int rows = 0;
        while (!parser.EndOfData)
        {
            string[]? row = parser.ReadFields();
            if (row is null || row.Length != headers.Length || ++rows > 10_000 ||
                row[apiColumn].Length is < 1 or > 80)
                throw new UnsupportedPortalStructureException("Steamworks localization export changed.");
            names.Add(row[apiColumn]);
        }
        return names;
    }
    public static byte[] BuildOneAchievement(string exportCsv, string apiName, string field, string desired)
    {
        if (field is not ("name" or "description") || apiName.Length is < 1 or > 80 ||
            desired.Length is < 1 or > 4096)
            throw new UnsupportedPortalStructureException("The reviewed localization row is invalid.");
        using TextFieldParser parser = new(new StringReader(exportCsv.TrimStart('\uFEFF')));
        parser.SetDelimiters(",");
        parser.HasFieldsEnclosedInQuotes = true;
        string[] headers = parser.ReadFields() ?? [];
        string[] names = headers.Select(header => header.Trim().Split('(')[0].Trim().ToLowerInvariant()).ToArray();
        if (headers.Length is < 3 or > 64 || names.Distinct(StringComparer.Ordinal).Count() != names.Length ||
            !names.Contains("api_name") || !names.Contains("field") || !names.Contains("english"))
            throw new UnsupportedPortalStructureException("Steamworks localization columns changed.");
        int apiColumn = Array.IndexOf(names, "api_name");
        int fieldColumn = Array.IndexOf(names, "field");
        int englishColumn = Array.IndexOf(names, "english");
        string[]? nameRow = null, descriptionRow = null;
        int rows = 0;
        while (!parser.EndOfData)
        {
            string[]? row = parser.ReadFields();
            if (row is null || row.Length != headers.Length || ++rows > 5000)
                throw new UnsupportedPortalStructureException("Steamworks localization export changed.");
            if (row[apiColumn] != apiName) continue;
            if (row[fieldColumn] == "name")
            {
                if (nameRow is not null)
                    throw new UnsupportedPortalStructureException("Steamworks localization contains duplicate rows.");
                nameRow = row;
            }
            else if (row[fieldColumn] == "description")
            {
                if (descriptionRow is not null)
                    throw new UnsupportedPortalStructureException("Steamworks localization contains duplicate rows.");
                descriptionRow = row;
            }
            else
                throw new UnsupportedPortalStructureException("Steamworks localization contains an unsupported field.");
        }
        if (nameRow is null || descriptionRow is null)
            throw new UnsupportedPortalStructureException("The reviewed Steamworks localization pair is unavailable.");
        string[] selected = field == "name" ? nameRow : descriptionRow;
        if (selected[englishColumn] == desired)
            throw new UnsupportedPortalStructureException("The reviewed Steamworks localization change is unavailable.");
        selected[englishColumn] = desired;
        string csv = string.Join(',', headers.Select(Escape)) + "\r\n" +
            string.Join(',', selected.Select(Escape)) + "\r\n";
        byte[] bytes = new UTF8Encoding(false, true).GetBytes(csv);
        if (bytes.Length > 65_536)
            throw new UnsupportedPortalStructureException("The reviewed localization row is too large.");
        return bytes;
    }

    private static string Escape(string value) => value.IndexOfAny([',', '"', '\r', '\n']) < 0
        ? value : '"' + value.Replace("\"", "\"\"", StringComparison.Ordinal) + '"';
}
