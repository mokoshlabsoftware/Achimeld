namespace Achimeld.Security;

public static class SteamNetworkPolicy
{
    private static readonly string[] ExactHosts = ["partner.steamgames.com", "store.steampowered.com",
        "login.steampowered.com", "steamcommunity.com"];
    private static readonly string[] ContentDomains = ["steamstatic.com", "steamcontent.com"];

    public static bool IsAllowedHost(string host) =>
        host.Length is > 0 and <= 253 && host.Split('.').All(label => label.Length is > 0 and <= 63 &&
            !label.StartsWith('-') && !label.EndsWith('-') && label.All(character => char.IsAsciiLetterOrDigit(character) || character == '-')) &&
        (ExactHosts.Contains(host, StringComparer.OrdinalIgnoreCase) || ContentDomains.Any(domain =>
            string.Equals(host, domain, StringComparison.OrdinalIgnoreCase) || host.EndsWith('.' + domain, StringComparison.OrdinalIgnoreCase)));

    public static void RequireHttps(Uri uri)
    {
        ArgumentNullException.ThrowIfNull(uri);
        if (!uri.IsAbsoluteUri || uri.Scheme != Uri.UriSchemeHttps || !uri.IsDefaultPort ||
            !string.IsNullOrEmpty(uri.UserInfo) || !IsAllowedHost(uri.IdnHost))
            throw new InvalidDataException("Only approved Steam HTTPS destinations are permitted.");
    }
}
