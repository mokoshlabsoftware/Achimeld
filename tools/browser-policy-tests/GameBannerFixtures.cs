using System.Text;
using System.Text.Json;
using Achimeld.Browser;

internal static class GameBannerFixtures
{
    public static int Run()
    {
        int checks = 0;
        const uint app = 4670140;
        const string root = "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/4670140/80b5555223644751a17f93ef56a9135e7aa2a422/";
        static byte[] Metadata(string url, uint id = app) => JsonSerializer.SerializeToUtf8Bytes(
            new Dictionary<string, object> { [id.ToString(System.Globalization.CultureInfo.InvariantCulture)] = new { success = true, data = new { capsule_image = url } } });
        void Check(bool condition) { if (!condition) throw new Exception("Game banner discovery fixture failed."); checks++; }
        foreach (string host in new[] { "shared.fastly", "shared.akamai" })
        {
            string source = root.Replace("shared.fastly", host, StringComparison.Ordinal);
            Check(SteamGameBanner.Resolve(app, Metadata(source + "capsule_231x87.jpg?t=1779725548"))?.AbsoluteUri ==
                source + "capsule_231x87_2x.jpg");
        }
        foreach (string url in new[] {
            root.Replace("/4670140/", "/480/", StringComparison.Ordinal) + "capsule_231x87.jpg",
            root.Replace("steamstatic.com", "steamstatic.com.evil.test", StringComparison.Ordinal) + "capsule_231x87.jpg",
            root.Replace("https:", "http:", StringComparison.Ordinal) + "capsule_231x87.jpg",
            root.Replace("https://", "https://user:secret@", StringComparison.Ordinal) + "capsule_231x87.jpg",
            root.Replace(".com/", ".com:8443/", StringComparison.Ordinal) + "capsule_231x87.jpg",
            root + "header.jpg", root + "capsule_231x87.jpg?token=secret",
            root + "capsule_231x87.jpg?t=1&token=secret", root + "capsule_231x87.jpg#secret",
            root.Replace("80b5555223644751a17f93ef56a9135e7aa2a422", "invalid", StringComparison.Ordinal) + "capsule_231x87.jpg",
            "https://partner.steamgames.com/apps/deleteachievement/4670140",
        }) Check(SteamGameBanner.Resolve(app, Metadata(url)) is null);
        Check(SteamGameBanner.Resolve(app, Metadata(root + "capsule_231x87.jpg", 480)) is null);
        foreach (string json in new[] { "[]", "null", "{}", "{", """{"4670140":{"success":false}}""", """{"4670140":{"success":true,"data":{"capsule_image":2}}}""" })
            Check(SteamGameBanner.Resolve(app, Encoding.UTF8.GetBytes(json)) is null);
        Check(SteamGameBanner.Resolve(app, new byte[512 * 1024 + 1]) is null);
        SteamBrowserRequestPolicy policy = new();
        policy.PinApp(app);
        bool rejected = false;
        try { policy.GameBannerMetadataUri(app); } catch (InvalidDataException) { rejected = true; }
        Check(rejected);
        policy.EnterRead();
        Check(policy.GameBannerMetadataUri(app).AbsoluteUri == "https://store.steampowered.com/api/appdetails?appids=4670140&filters=basic");
        rejected = false;
        try { policy.ResolveGameBanner(480, Metadata(root + "capsule_231x87.jpg")); } catch (InvalidDataException) { rejected = true; }
        Check(rejected);
        return checks;
    }
}
