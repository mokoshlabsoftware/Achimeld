using Achimeld.Security;

namespace Achimeld.Browser;

public static class BrowserReadPolicy
{
    // Application-owned network policy is visible here and in the native broker.
    // Redirects are disabled by the driver; no caller can supply a download route.
    public static void RequireIcon(Uri uri)
    {
        SteamNetworkPolicy.RequireHttps(uri);
        string path = uri.AbsolutePath;
        if (!(IsContentHost(uri.IdnHost) ||
              (uri.IdnHost.Equals("steamcommunity.com", StringComparison.OrdinalIgnoreCase) && path.StartsWith("/public/images/apps/", StringComparison.Ordinal)) ||
              (uri.IdnHost.Equals("partner.steamgames.com", StringComparison.OrdinalIgnoreCase) && path.StartsWith("/public/images/", StringComparison.Ordinal))) ||
            !IsImagePath(path))
            throw new InvalidDataException("Only Steam content icon hosts are permitted.");
        if (!string.IsNullOrEmpty(uri.Query) || !string.IsNullOrEmpty(uri.Fragment))
            throw new InvalidDataException("Steam icon URLs cannot carry query or fragment data.");
    }

    internal static bool IsContentHost(string host) => new[] { "steamstatic.com", "steamcontent.com" }.Any(domain =>
        host.Equals(domain, StringComparison.OrdinalIgnoreCase) || host.EndsWith('.' + domain, StringComparison.OrdinalIgnoreCase));

    internal static bool IsImagePath(string path) => HasExtension(path, [".png", ".jpg", ".jpeg", ".webp", ".gif", ".ico"]);

    internal static bool IsMediaPath(string path) => HasExtension(path, [".mp3", ".ogg", ".wav", ".m4a", ".aac", ".mp4", ".webm", ".ogv"]);

    internal static bool IsMediaQuery(string query) => query.Length == 0 ||
        (query.Length <= 24 && (query.StartsWith("?v=", StringComparison.Ordinal) || query.StartsWith("?t=", StringComparison.Ordinal)) &&
         query.Length > 3 && query[3..].All(char.IsAsciiDigit));

    internal static bool IsStaticPath(string path) => HasExtension(path, [".js", ".css", ".png", ".jpg", ".jpeg", ".webp", ".gif", ".ico", ".svg", ".woff", ".woff2", ".ttf"]);

    private static bool HasExtension(string path, string[] extensions) =>
        path.Length <= 2048 && path.All(c => char.IsAsciiLetterOrDigit(c) || c is '/' or '.' or '_' or '-' or '~') &&
        extensions.Any(extension => path.EndsWith(extension, StringComparison.OrdinalIgnoreCase));
}
