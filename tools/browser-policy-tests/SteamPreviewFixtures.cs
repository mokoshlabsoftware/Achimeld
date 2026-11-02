using System.Xml.Linq;
using Achimeld.Browser;
using Achimeld.Core;

internal static class SteamPreviewFixtures
{
    internal static int Run()
    {
        const string url = "https://cdn.steamstatic.com/steamcommunity/public/images/apps/480/a.png";
        const string grayUrl = "https://cdn.steamstatic.com/steamcommunity/public/images/apps/480/b.png";
        const string achievements = """
            {"achievements":[{"api_name":"FIRST","stat_id":1,"bit_id":0,"hidden":false,
            "permission":0,"display_name":"First","description":"First achievement","icon":"https://cdn.steamstatic.com/steamcommunity/public/images/apps/480/a.png",
            "icon_gray":"https://cdn.steamstatic.com/steamcommunity/public/images/apps/480/b.png"}]}
            """;
        byte[] pixels = [1, 2, 3, 4];
        string html = LiveSteamworksConfiguration.Normalize(new SteamAppId(480), "[]", achievements,
            new Dictionary<string, string> { [url] = "source-hash", [grayUrl] = "gray-hash" },
            new Dictionary<string, byte[]> { [url] = pixels, [grayUrl] = pixels }).Html;
        XElement icon = XDocument.Parse(html).Descendants("section")
            .Single(section => (string?)section.Attribute("id") == "icons").Element("article")!;
        if ((string?)icon.Attribute("data-achieved-identity") != "source-hash" ||
            (string?)icon.Attribute("data-achieved-preview-data") != Convert.ToBase64String(pixels) ||
            (string?)icon.Attribute("data-unachieved-identity") != "gray-hash" ||
            (string?)icon.Attribute("data-unachieved-preview-data") != Convert.ToBase64String(pixels))
            throw new Exception("Steam icon preview bytes were not carried with the approved icon metadata.");
        string withoutPreview = LiveSteamworksConfiguration.Normalize(new SteamAppId(480), "[]", achievements).Html;
        if (withoutPreview.Contains("-preview-data", StringComparison.Ordinal))
            throw new Exception("Missing icon content produced a preview.");
        return 2;
    }
}
