using System.Globalization;
using System.Text.Json;
using System.Xml.Linq;
using Achimeld.Core;
using Microsoft.VisualBasic.FileIO;

namespace Achimeld.Browser;

// Adapts Steam's documented definitions/localization exports to the existing
// complete-snapshot reader. Set assignments are retained as remote-only data;
// reviewed definition writes cannot silently replace them.
public static class ModernSteamworksConfiguration
{
    private static readonly IReadOnlyDictionary<string, string> Languages =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["english"] = "english", ["polish"] = "polish", ["french"] = "french",
            ["japanese"] = "japanese", ["schinese"] = "schinese",
            ["tchinese"] = "tchinese", ["koreana"] = "koreana",
        };

    public static string AchievementJson(SteamworksCsvExportData export,
        IReadOnlyDictionary<string, ModernAchievementIconPair> icons,
        Action<string>? diagnosticStage = null)
    {
        diagnosticStage?.Invoke("modern-parse-csv");
        List<Dictionary<string, string>> definitions = ReadRows(export.DefinitionsCsv);
        List<Dictionary<string, string>> localized = ReadRows(export.LocalizationCsv);
        diagnosticStage?.Invoke("modern-check-counts");
        if (definitions.Count != export.Shape.DefinitionsRows || localized.Count != export.Shape.LocalizationRows ||
            (export.Shape.DefinitionsHeaderMask & 255) != 255 ||
            (export.Shape.LocalizationHeaderMask & 7) != 7 || definitions.Count != icons.Count)
            throw new UnsupportedPortalStructureException("The modern Steamworks exports are incomplete.");
        HashSet<string> names = new(StringComparer.Ordinal);
        diagnosticStage?.Invoke("modern-check-definitions");
        foreach (Dictionary<string, string> row in definitions)
        {
            string name = Required(row, "api_name");
            _ = new SteamApiName(name);
            foreach ((string key, string value) in row)
                if (key is not ("api_name" or "setid" or "permission" or "spoiler" or
                    "archived" or "progress_stat_name" or "progress_stat_min" or "progress_stat_max") &&
                    !string.IsNullOrWhiteSpace(value))
                    throw new UnsupportedPortalStructureException("Steamworks exported an unsupported definition field.");
            if (!names.Add(name) || !icons.ContainsKey(name))
                throw new UnsupportedPortalStructureException("Steamworks export and icon cards do not match.");
        }
        Dictionary<(string Name, string Field), Dictionary<string, string>> translations = new();
        diagnosticStage?.Invoke("modern-check-localizations");
        foreach (Dictionary<string, string> row in localized)
        {
            string name = Required(row, "api_name"), field = Required(row, "field");
            if (!names.Contains(name) || field is not ("name" or "description"))
                throw new UnsupportedPortalStructureException("Steamworks exported an unsupported localization row.");
            Dictionary<string, string> values = new(StringComparer.Ordinal);
            foreach ((string key, string value) in row)
            {
                if (key is "api_name" or "field") continue;
                if (!Languages.TryGetValue(key, out string? language))
                {
                    if (!string.IsNullOrWhiteSpace(value))
                        throw new UnsupportedPortalStructureException("Steamworks uses an unsupported localization language.");
                    continue;
                }
                values[language] = value;
            }
            if (!translations.TryAdd((name, field), values))
                throw new UnsupportedPortalStructureException("Steamworks exported duplicate localization rows.");
        }
        List<Dictionary<string, object>> achievements = [];
        foreach (Dictionary<string, string> row in definitions)
        {
            diagnosticStage?.Invoke("modern-check-english");
            string name = Required(row, "api_name");
            if (!translations.TryGetValue((name, "name"), out Dictionary<string, string>? display) ||
                !translations.TryGetValue((name, "description"), out Dictionary<string, string>? description) ||
                !display.TryGetValue("english", out string? english) || string.IsNullOrWhiteSpace(english))
                throw new UnsupportedPortalStructureException("Steamworks exported incomplete English localization.");
            diagnosticStage?.Invoke("modern-check-archived");
            if (!bool.TryParse(Required(row, "archived"), out bool archived) || archived)
                throw new UnsupportedPortalStructureException("Steamworks achievement archives need a newer snapshot model.");
            diagnosticStage?.Invoke("modern-check-set-id");
            string setId = row.GetValueOrDefault("setid") ?? "";
            if (setId.Length > 0 && setId != "0" &&
                !ulong.TryParse(setId, NumberStyles.None, CultureInfo.InvariantCulture, out _))
                throw new UnsupportedPortalStructureException("Steamworks exported an invalid achievement set ID.");
            diagnosticStage?.Invoke("modern-check-permission");
            string permission = Required(row, "permission") switch
            {
                "Client" => "0", "GameServer" => "1", "OfficialGameServer" => "2",
                _ => throw new UnsupportedPortalStructureException("Steamworks exported an unknown achievement permission."),
            };
            diagnosticStage?.Invoke("modern-check-spoiler");
            string hidden = Required(row, "spoiler") switch
            {
                "true" => "1", "false" => "0",
                _ => throw new UnsupportedPortalStructureException("Steamworks exported an unknown spoiler value."),
            };
            ModernAchievementIconPair pair = icons[name];
            Dictionary<string, object> item = new(StringComparer.Ordinal)
            {
                ["api_name"] = name, ["stat_id"] = "0", ["bit_id"] = "0",
                ["hidden"] = hidden, ["permission"] = permission,
                ["display_name"] = display, ["description"] = description,
                ["icon"] = pair.Achieved, ["icon_gray"] = pair.Unachieved,
            };
            if (setId.Length > 0 && setId != "0") item["setid"] = setId;
            string progress = row.GetValueOrDefault("progress_stat_name") ?? "";
            string minimum = row.GetValueOrDefault("progress_stat_min") ?? "";
            string maximum = row.GetValueOrDefault("progress_stat_max") ?? "";
            diagnosticStage?.Invoke("modern-check-progress");
            if (progress.Length > 0)
            {
                diagnosticStage?.Invoke("modern-progress-name");
                _ = new SteamApiName(progress);
                diagnosticStage?.Invoke("modern-progress-min-parse");
                if (!decimal.TryParse(minimum, NumberStyles.Number, CultureInfo.InvariantCulture, out decimal parsedMin))
                    throw new UnsupportedPortalStructureException("Steamworks exported an unsupported progress minimum.");
                diagnosticStage?.Invoke("modern-progress-min-value");
                if (parsedMin != 0)
                    throw new UnsupportedPortalStructureException("Steamworks exported an unsupported progress minimum.");
                diagnosticStage?.Invoke("modern-progress-max-parse");
                if (!decimal.TryParse(maximum, NumberStyles.Number, CultureInfo.InvariantCulture, out decimal parsedMax))
                    throw new UnsupportedPortalStructureException("Steamworks exported an unsupported progress maximum.");
                diagnosticStage?.Invoke("modern-progress-max-value");
                if (parsedMax <= 0)
                    throw new UnsupportedPortalStructureException("Steamworks exported an unsupported progress range.");
                item["progress"] = new { value = new { operation = "statvalue", operand1 = progress }, max_val = maximum };
            }
            else if (!EmptyOrZero(minimum) || !EmptyOrZero(maximum))
            {
                diagnosticStage?.Invoke("modern-progress-orphan-range");
                throw new UnsupportedPortalStructureException("Steamworks exported an incomplete progress range.");
            }
            achievements.Add(item);
        }
        diagnosticStage?.Invoke("modern-serialize");
        return JsonSerializer.Serialize(new { achievements });
    }

    public static ManagedPageDocument RemoveSyntheticAchievementIds(ManagedPageDocument document)
    {
        XDocument xml = XDocument.Parse(document.Html, LoadOptions.None);
        XElement? section = xml.Descendants("section").SingleOrDefault(element =>
            element.Attribute("id")?.Value == "achievements");
        if (section is null) throw new UnsupportedPortalStructureException("The achievement snapshot is incomplete.");
        foreach (XElement article in section.Elements("article"))
            article.Attribute("data-steam-id")?.Remove();
        return document with { Html = xml.ToString(SaveOptions.DisableFormatting) };
    }

    private static List<Dictionary<string, string>> ReadRows(string csv)
    {
        using TextFieldParser parser = new(new StringReader(csv.TrimStart('\uFEFF')));
        parser.SetDelimiters(",");
        parser.HasFieldsEnclosedInQuotes = true;
        string[] headers = (parser.ReadFields() ?? []).Select(header =>
            header.Trim().Split('(')[0].Trim().ToLowerInvariant()).ToArray();
        if (headers.Length is < 2 or > 64 || headers.Distinct(StringComparer.Ordinal).Count() != headers.Length)
            throw new UnsupportedPortalStructureException("Steamworks export columns are ambiguous.");
        List<Dictionary<string, string>> rows = [];
        while (!parser.EndOfData)
        {
            string[]? fields = parser.ReadFields();
            if (fields is null || fields.Length != headers.Length || rows.Count >= 5000)
                throw new UnsupportedPortalStructureException("Steamworks export rows are incomplete.");
            rows.Add(headers.Zip(fields).ToDictionary(pair => pair.First, pair => pair.Second,
                StringComparer.Ordinal));
        }
        return rows;
    }

    private static string Required(IReadOnlyDictionary<string, string> row, string key) =>
        row.TryGetValue(key, out string? value) && value.Length > 0 ? value :
            throw new UnsupportedPortalStructureException("Steamworks export is missing a required field.");

    private static bool EmptyOrZero(string value) => value.Length == 0 ||
        (decimal.TryParse(value, NumberStyles.Number, CultureInfo.InvariantCulture, out decimal parsed) &&
         parsed == 0);
}
