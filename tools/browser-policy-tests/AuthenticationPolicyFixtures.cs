using Achimeld.Browser;
using Achimeld.Security;

internal static class AuthenticationPolicyFixtures
{
    internal static int Run()
    {
        SteamBrowserRequestPolicy policy = new();
        policy.BeginAuthentication();
        policy.PinApp(480);
        string[] methods = ["GetPasswordRSAPublicKey", "BeginAuthSessionViaQR", "BeginAuthSessionViaCredentials", "PollAuthSessionStatus", "UpdateAuthSessionWithSteamGuardCode"];
        (string Host, string Prefix, string Suffix)[] transports = [
            ("api.steampowered.com", "/IAuthenticationService/", "/v1"),
            ("partner.steamgames.com", "/um/Authentication/", "/"),
        ];
        int checks = 0;
        foreach (var transport in transports)
        foreach (string rpc in methods)
        {
            string method = rpc == "GetPasswordRSAPublicKey" ? "GET" : "POST";
            string url = $"https://{transport.Host}{transport.Prefix}{rpc}{transport.Suffix}";
            foreach (string type in new[] { "fetch", "xhr" })
                if (!policy.Allows(method, new Uri(url + "?origin=fixture"), type, false)) throw new Exception("Login RPC rejected.");
            foreach (string invalid in new[] { url + "/", url + "#fragment", url[..^transport.Suffix.Length], url + "v2", url.Replace(transport.Host, transport.Host + ".evil.test"), url.Replace("https:", "http:"), url.Replace(".com/", ".com:8443/"), url.Replace(transport.Host, transport.Host == "api.steampowered.com" ? "partner.steamgames.com" : "api.steampowered.com") })
                if (policy.Allows(method, new Uri(invalid), "fetch", false)) throw new Exception("Malformed login RPC accepted.");
            foreach (string type in new[] { "document", "script", "image", "media" })
                if (policy.Allows(method, new Uri(url), type, false)) throw new Exception("Login resource type escaped.");
            if (policy.Allows(method, new Uri(url), "fetch", true) || policy.Allows(method == "GET" ? "POST" : "GET", new Uri(url), "fetch", false)) throw new Exception("Login method/navigation escaped.");
            checks += 16;
        }
        foreach (var transport in transports)
        foreach (string rpc in new[] { "RevokeToken", "EnumerateTokens", "GenerateAccessTokenForApp", "GetAuthSessionInfo", "UpdateAuthSessionWithMobileConfirmation" })
            if (policy.Allows("POST", new Uri($"https://{transport.Host}{transport.Prefix}{rpc}{transport.Suffix}"), "fetch", false)) throw new Exception("Account administration accepted.");
        policy.EnterRead();
        foreach (var transport in transports)
        foreach (string rpc in methods)
            foreach (string method in new[] { "GET", "POST" })
                if (policy.Allows(method, new Uri($"https://{transport.Host}{transport.Prefix}{rpc}{transport.Suffix}"), "fetch", false)) throw new Exception("Read regained authentication authority.");
        if (!SteamNetworkPolicy.IsAllowedHost("api.steampowered.com") || SteamNetworkPolicy.IsAllowedHost("sub.api.steampowered.com")) throw new Exception("API host boundary changed.");
        if (BrowserRequestDiagnostic.Describe("POST", "https://partner.steamgames.com/um/Authentication/BeginAuthSessionViaQR/", false, "fetch").Path != "authentication-api") throw new Exception("Browser login diagnostic category missing.");
        return checks + 32;
    }
}
