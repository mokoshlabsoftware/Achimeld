using Achimeld.Browser;
using System.Text.Json;

internal static class MediaPolicyFixtures
{
    internal static int Run()
    {
        int checks = 0;
        SteamBrowserRequestPolicy policy = new();
        const string audio = "https://cdn.steamstatic.com/public/audio/notification.mp3";
        void Check(bool expected, string url = audio, string method = "GET", bool navigation = false, string resource = "media")
        {
            if (policy.Allows(method, new Uri(url), resource, navigation) != expected)
                throw new Exception("Media request policy decision changed.");
            checks++;
        }
        Check(false);
        policy.BeginAuthentication();
        for (int phase = 0; phase < 3; phase++)
        {
            foreach (string extension in new[] { "mp3", "ogg", "wav", "m4a", "aac", "mp4", "webm", "ogv" })
            {
                Check(true, $"https://cdn.steamstatic.com/public/audio/clip.{extension}");
                Check(true, $"https://partner.steamgames.com/public/audio/clip.{extension}?v=123");
                Check(true, $"https://cdn.steamcontent.com/clip.{extension}?t=456");
            }
            foreach (string url in new[] { "http://cdn.steamstatic.com/a.mp3", "https://cdn.steamstatic.com.evil.test/a.mp3",
                "https://user:secret@cdn.steamstatic.com/a.mp3", "https://cdn.steamstatic.com:8443/a.mp3",
                audio + "?token=secret", audio + "?v=secret", audio + "?v=1&t=2", audio + "#secret",
                "https://partner.steamgames.com/apps/deleteachievement/480", "https://partner.steamgames.com/apps/fetchstats/480",
                "https://login.steampowered.com/jwt/finalizelogin", "https://partner.steamgames.com/private/a.mp3",
                "https://cdn.steamstatic.com/a%2fb.mp3", "https://cdn.steamstatic.com/a.m3u8", "https://cdn.steamstatic.com/a.exe" }) Check(false, url);
            Check(false, method: "POST"); Check(false, method: "HEAD"); Check(false, navigation: true);
            Check(false, resource: "fetch"); Check(false, resource: "script");
            if (phase == 0) { policy.PinApp(480); policy.EnterRead(); }
            if (phase == 1) policy.BeginReviewedDraft(480, PortalEntityKind.Stat);
        }
        var first = BrowserRequestDiagnostic.Describe("GET", audio, false, "media");
        policy.Reject(BrowserRequestRejection.Media, first);
        policy.Reject(BrowserRequestRejection.Other, BrowserRequestDiagnostic.Describe("POST", audio, true));
        if (policy.RejectionDetails != first) throw new Exception("First rejected request was overwritten.");
        Check(false);
        const string secret = "secret-marker";
        var detail = BrowserRequestDiagnostic.Describe(secret, $"https://{secret}.invalid/{secret}?token={secret}", false, secret, secret);
        if (JsonSerializer.Serialize(detail).Contains(secret, StringComparison.Ordinal)) throw new Exception("Request diagnostics leaked arbitrary text.");
        if (BrowserRequestDiagnostic.Describe("GET", audio, false, "media", "net::ERR_CONNECTION_RESET").Failure != "reset")
            throw new Exception("Chromium failure classification changed.");
        Console.WriteLine($"PASS: {checks} media policy checks and safe request diagnostic categories.");
        return checks + 3;
    }
}
