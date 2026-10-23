using System.Text.Json;
using System.Xml.Linq;
using Achimeld.Core;

namespace Achimeld.Browser;

/// <summary>Normalizes the configuration returned by Steamworks' own read-only table requests.</summary>
public static class LiveSteamworksConfiguration
{
    public const string Revision = "steamworks-configuration-json-v1";

    public static ManagedPageDocument Normalize(SteamAppId appId, string statsJson, string achievementsJson,
        IReadOnlyDictionary<string, string>? iconContentHashes = null)
    {
        try
        {
            using JsonDocument stats = JsonDocument.Parse(statsJson);
            using JsonDocument achievements = JsonDocument.Parse(achievementsJson);
            if (stats.RootElement.ValueKind != JsonValueKind.Array ||
                !achievements.RootElement.TryGetProperty("achievements", out JsonElement achievementArray) ||
                achievementArray.ValueKind != JsonValueKind.Array)
                throw new UnsupportedPortalStructureException("Steamworks did not return complete stats and achievement arrays.");

            XElement statSection = new("section", new XAttribute("id", "stats"));
            XElement achievementSection = new("section", new XAttribute("id", "achievements"));
            XElement localizations = new("section", new XAttribute("id", "localizations"));
            XElement icons = new("section", new XAttribute("id", "icons"));
            int order = 0;
            foreach (JsonElement stat in stats.RootElement.EnumerateArray())
            {
                string name = Required(stat, "name");
                string type = Required(stat, "type") switch
                {
                    "INT" => "Int", "FLOAT" => "Float", "AVGRATE" => "AverageRate",
                    _ => throw new UnsupportedPortalStructureException("Unknown Steam stat type.")
                };
                XElement item = new("article", Attr("api-name", name), Attr("steam-id", Required(stat, "stat_id")),
                    Attr("type", type), Attr("set-by", Permission(stat)),
                    Attr("increment-only", Boolean(stat, "incrementonly")), Attr("aggregated", Boolean(stat, "aggregated")),
                    Attr("display-order", order++));
                foreach ((string source, string target) in new[] { ("min", "minimum"), ("max", "maximum"),
                    ("default", "default"), ("maxchange", "maximum-change"), ("windowsize", "average-window") })
                    OptionalAttribute(item, target, Text(stat, source));
                statSection.Add(item);
                if (stat.TryGetProperty("display", out JsonElement display))
                    AddLocalization(localizations, "Stat", name, "StatDisplayName", display, "name");
            }
            order = 0;
            foreach (JsonElement achievement in achievementArray.EnumerateArray())
            {
                string name = Required(achievement, "api_name");
                XElement item = new("article", Attr("api-name", name),
                    Attr("steam-id", Required(achievement, "stat_id") + "_" + Required(achievement, "bit_id")),
                    Attr("hidden", Boolean(achievement, "hidden")), Attr("set-by", Permission(achievement)),
                    Attr("display-order", order++));
                if (achievement.TryGetProperty("progress", out JsonElement progress) && progress.ValueKind == JsonValueKind.Object)
                {
                    JsonElement value = progress.GetProperty("value");
                    if (Required(value, "operation") != "statvalue")
                        throw new UnsupportedPortalStructureException("The achievement uses an unsupported progress expression.");
                    item.Add(Attr("progress-stat", Required(value, "operand1")), Attr("unlock-value", Required(progress, "max_val")));
                }
                achievementSection.Add(item);
                AddLocalization(localizations, "Achievement", name, "AchievementDisplayName", achievement, "display_name");
                AddLocalization(localizations, "Achievement", name, "AchievementDescription", achievement, "description");
                XElement icon = new("article", Attr("api-name", name));
                foreach ((string source, string target) in new[] { ("icon", "achieved"), ("icon_gray", "unachieved") })
                {
                    string? url = Text(achievement, source);
                    if (string.IsNullOrWhiteSpace(url)) continue;
                    if (!Uri.TryCreate(url, UriKind.Absolute, out Uri? uri) || uri.Scheme != Uri.UriSchemeHttps)
                        throw new UnsupportedPortalStructureException("Steamworks returned an invalid icon preview URL.");
                    icon.Add(Attr(target + "-preview", uri.AbsoluteUri), Attr(target + "-identity", iconContentHashes?.GetValueOrDefault(uri.AbsoluteUri) ?? uri.AbsolutePath));
                }
                icons.Add(icon);
            }
            XElement root = new("html", new XAttribute("data-appid", appId.Value),
                new XAttribute("data-steamworks-revision", Revision), new XAttribute("data-steamworks-page", "stats-achievements"),
                new XElement("body", statSection, achievementSection, localizations, icons));
            return new ManagedPageDocument(new Uri($"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{appId.Value}"),
                "Steamworks configuration", root.ToString(SaveOptions.DisableFormatting));
        }
        catch (Exception exception) when (exception is JsonException or InvalidOperationException or KeyNotFoundException)
        {
            throw new UnsupportedPortalStructureException("Steamworks returned incomplete or unsupported configuration data: " + exception.Message);
        }
    }

    private static void AddLocalization(XElement section, string entity, string name, string field,
        JsonElement source, string key)
    {
        if (!source.TryGetProperty(key, out JsonElement text)) return;
        XElement item = new("article", Attr("entity-type", entity), Attr("api-name", name), Attr("field", field));
        foreach ((string steam, string column) in new[] { ("english", "English"), ("polish", "Polish"),
            ("schinese", "ChineseSimplified"), ("tchinese", "ChineseTraditional"), ("japanese", "Japanese"),
            ("koreana", "Korean"), ("french", "French") })
        {
            string? value = text.ValueKind == JsonValueKind.String
                ? (steam == "english" ? text.GetString() : null) : Text(text, steam);
            if (value is not null) item.Add(new XElement("span", new XAttribute("data-language", column), value));
        }
        if (item.Elements().Any(static value => value.Attribute("data-language")?.Value == "English" && !string.IsNullOrWhiteSpace(value.Value)))
            section.Add(item);
    }

    private static XAttribute Attr(string name, object value) => new("data-" + name, value);
    private static void OptionalAttribute(XElement item, string key, string? value)
    {
        if (!string.IsNullOrWhiteSpace(value)) item.Add(Attr(key, value));
    }
    private static string Required(JsonElement item, string key) => Text(item, key) is { Length: > 0 } value
        ? value : throw new UnsupportedPortalStructureException($"Steamworks omitted '{key}'.");
    private static string? Text(JsonElement item, string key) => item.ValueKind == JsonValueKind.Object && item.TryGetProperty(key, out JsonElement value)
        ? value.ValueKind switch { JsonValueKind.String => value.GetString(), JsonValueKind.Null => null, _ => value.GetRawText() } : null;
    private static string Permission(JsonElement item) => Text(item, "permission") switch
    {
        null or "0" => "GameClient", "1" => "GameServer", "2" => "OfficialGameServer",
        _ => throw new UnsupportedPortalStructureException("Unknown Steamworks set-by permission.")
    };
    private static bool Boolean(JsonElement item, string key) => Text(item, key) switch
    {
        null or "" or "0" or "false" => false, "1" or "true" => true,
        _ => throw new UnsupportedPortalStructureException($"Invalid Steamworks boolean '{key}'.")
    };
}
