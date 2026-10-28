namespace Achimeld.Browser;

// Only closed categories leave Chromium. Never retain request URLs or exception text.
public sealed record BrowserRequestDiagnostic(string Method, string Host, string Path, string Extension, bool Query, bool Navigation, string Resource, string Failure)
{
    public static BrowserRequestDiagnostic Describe(string method, string url, bool navigation, string resource = "other", string? networkFailure = null)
    {
        Uri.TryCreate(url, UriKind.Absolute, out Uri? uri);
        string host = uri?.IdnHost ?? "";
        string path = uri?.AbsolutePath ?? "";
        string extension = System.IO.Path.GetExtension(path).ToLowerInvariant();
        return new(method is "GET" or "POST" or "HEAD" or "PUT" or "DELETE" ? method : "other",
            host is "partner.steamgames.com" ? "partner" :
            host is "login.steampowered.com" ? "login" :
            host is "api.steampowered.com" ? "api" :
            host is "store.steampowered.com" ? "store" :
            host is "steamcommunity.com" ? "community" : BrowserReadPolicy.IsContentHost(host) ? "content" : "other",
            path.StartsWith("/public/", StringComparison.Ordinal) ? "public" :
            path.StartsWith("/apps/", StringComparison.Ordinal) ? "apps" :
            path.StartsWith("/login", StringComparison.Ordinal) ? "login" :
            path == "/actions/CSPReport" ? "csp-report" :
            path.StartsWith("/IAuthenticationService/", StringComparison.Ordinal) ||
                path.StartsWith("/um/Authentication/", StringComparison.Ordinal) ? "authentication-api" : "other",
            extension is ".mp3" or ".ogg" or ".wav" or ".m4a" or ".aac" or ".mp4" or ".webm" or ".ogv" or ".js" or ".css" or ".png" or ".jpg" or ".woff" or ".woff2" ? extension : "other",
            !string.IsNullOrEmpty(uri?.Query), navigation,
            resource is "document" or "media" or "script" or "stylesheet" or "image" or "font" or "xhr" or "fetch" or "ping" or "manifest" or "eventsource" or "cspreport" ? resource : "other",
            networkFailure is null ? "policy" : NetworkFailure(networkFailure));
    }

    private static string NetworkFailure(string failure) => failure switch {
        "net::ERR_TUNNEL_CONNECTION_FAILED" => "tunnel", "net::ERR_PROXY_CONNECTION_FAILED" => "proxy",
        "net::ERR_NAME_NOT_RESOLVED" => "dns", "net::ERR_TIMED_OUT" => "timeout",
        "net::ERR_CONNECTION_RESET" => "reset", "net::ERR_CONNECTION_REFUSED" => "refused",
        "net::ERR_CONNECTION_CLOSED" => "closed", "net::ERR_INTERNET_DISCONNECTED" => "disconnected",
        "net::ERR_NETWORK_CHANGED" => "changed", "net::ERR_ABORTED" => "aborted",
        "net::ERR_BLOCKED_BY_CLIENT" => "blocked", "net::ERR_FAILED" => "failed",
        _ when failure.StartsWith("net::ERR_CERT_", StringComparison.Ordinal) => "certificate",
        _ => "other",
    };
}
