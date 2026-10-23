using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using Achimeld.Core;

namespace Achimeld.Browser;

public sealed class PortalSafetyException : InvalidOperationException
{
    public PortalSafetyException(string code, string message)
        : base(message)
    {
        Code = code;
    }

    public string Code { get; }
}

public sealed class UnsupportedPortalStructureException : InvalidOperationException
{
    public UnsupportedPortalStructureException(string message)
        : base(message)
    {
    }
}

public static class SteamworksUrlGuard
{
    public const string AllowedOrigin = "https://partner.steamgames.com";

    public static Uri BuildAppAdminUri(SteamAppId appId) =>
        new($"{AllowedOrigin}/apps/landing/{appId.Value.ToString(CultureInfo.InvariantCulture)}");

    public static Uri BuildPublicationReviewUri(SteamAppId appId) =>
        new($"{AllowedOrigin}/apps/publishing/{appId.Value.ToString(CultureInfo.InvariantCulture)}");

    public static void EnsureAllowed(Uri uri, SteamAppId expectedAppId)
    {
        ArgumentNullException.ThrowIfNull(uri);
        if (!uri.IsAbsoluteUri ||
            !string.Equals(uri.Scheme, Uri.UriSchemeHttps, StringComparison.Ordinal) ||
            !string.Equals(uri.Host, "partner.steamgames.com", StringComparison.OrdinalIgnoreCase) ||
            !uri.IsDefaultPort ||
            !string.IsNullOrEmpty(uri.UserInfo))
        {
            throw new PortalSafetyException(
                "portal.origin.rejected",
                "Automation is restricted to the exact https://partner.steamgames.com origin.");
        }

        IReadOnlyList<uint> discovered = ExtractAppIds(uri);
        if (discovered.Count == 0 || discovered.Any(value => value == 0 || value != expectedAppId.Value))
        {
            throw new PortalSafetyException(
                "portal.appId.rejected",
                "The Steamworks page does not unambiguously match the expected nonzero App ID.");
        }
    }

    public static IReadOnlyList<uint> ExtractAppIds(Uri uri)
    {
        ArgumentNullException.ThrowIfNull(uri);
        List<uint> values = [];

        string[] segments = uri.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries);
        for (int index = 0; index < segments.Length; index++)
        {
            if (uint.TryParse(segments[index], NumberStyles.None, CultureInfo.InvariantCulture, out uint value) &&
                index > 0 &&
                (segments[index - 1].Equals("apps", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("app", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("landing", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("publishing", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("achievements", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("stats", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("fetchstats", StringComparison.OrdinalIgnoreCase) ||
                 segments[index - 1].Equals("fetchachievements", StringComparison.OrdinalIgnoreCase)))
            {
                values.Add(value);
            }
        }

        string query = uri.Query.TrimStart('?');
        foreach (string pair in query.Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            string[] parts = pair.Split('=', 2);
            string name = Uri.UnescapeDataString(parts[0].Replace('+', ' '));
            if ((name.Equals("appid", StringComparison.OrdinalIgnoreCase) ||
                 name.Equals("app_id", StringComparison.OrdinalIgnoreCase)) &&
                parts.Length == 2 &&
                uint.TryParse(
                    Uri.UnescapeDataString(parts[1].Replace('+', ' ')),
                    NumberStyles.None,
                    CultureInfo.InvariantCulture,
                    out uint value))
            {
                values.Add(value);
            }
        }

        return values.Distinct().ToArray();
    }
}

public enum SteamworksPageKind
{
    Unknown,
    SignIn,
    AuthenticationChallenge,
    PermissionDenied,
    AppAdmin,
    StatsAndAchievements,
    PublicationReview,
}

public enum PortalAccessState
{
    Unknown,
    SignedOut,
    AuthenticationRequired,
    PermissionDenied,
    Authenticated,
}

public sealed record SteamworksPageClassification(
    SteamworksPageKind PageKind,
    PortalAccessState AccessState,
    string? Revision,
    bool IsAutomationSafe,
    string Reason);

public sealed class SteamworksPageClassifier
{
    [SuppressMessage("Performance", "CA1822:Mark members as static", Justification = "Classifier is an injectable adapter boundary.")]
    public PortalAccessState ClassifyAuthentication(ManagedPageDocument document)
    {
        ArgumentNullException.ThrowIfNull(document);
        if (document.Uri.Scheme == Uri.UriSchemeHttps && document.Uri.IsDefaultPort &&
            (document.Uri.Host is "partner.steamgames.com" or "steamcommunity.com" or "store.steampowered.com") &&
            (document.Uri.AbsolutePath.StartsWith("/login", StringComparison.OrdinalIgnoreCase) ||
             document.Uri.AbsolutePath.StartsWith("/openid", StringComparison.OrdinalIgnoreCase)))
            return PortalAccessState.AuthenticationRequired;
        string combined = document.Title + "\n" + document.Html;
        if (ContainsAny(combined, "steam guard", "auth challenge", "captcha", "two-factor"))
        {
            return PortalAccessState.AuthenticationRequired;
        }

        if (ContainsAny(combined, "name=\"password\"", "type=\"password\"", "sign in", "login_form"))
        {
            return PortalAccessState.SignedOut;
        }

        if (ContainsAny(combined, "permission denied", "access denied", "not authorized", "insufficient privilege"))
        {
            return PortalAccessState.PermissionDenied;
        }

        return IsPartnerOrigin(document.Uri)
            ? PortalAccessState.Authenticated
            : PortalAccessState.Unknown;
    }

    [SuppressMessage("Performance", "CA1822:Mark members as static", Justification = "Classifier is an injectable adapter boundary.")]
    public SteamworksPageClassification Classify(
        ManagedPageDocument document,
        SteamAppId expectedAppId)
    {
        ArgumentNullException.ThrowIfNull(document);
        string html = document.Html ?? string.Empty;
        string combined = document.Title + "\n" + html;

        if (!IsPartnerOrigin(document.Uri))
        {
            return ContainsAny(combined, "steam guard", "two-factor", "captcha", "account login", "openid")
                ? new(
                    SteamworksPageKind.AuthenticationChallenge,
                    PortalAccessState.AuthenticationRequired,
                    null,
                    false,
                    "Authentication redirects require manual interaction.")
                : Unknown("The page left the allowed Steamworks origin.");
        }

        if (ContainsAny(combined, "steam guard", "auth challenge", "captcha", "two-factor"))
        {
            return new(
                SteamworksPageKind.AuthenticationChallenge,
                PortalAccessState.AuthenticationRequired,
                null,
                false,
                "A manual authentication challenge is active.");
        }

        if (ContainsAny(combined, "name=\"password\"", "type=\"password\"", "sign in", "login_form"))
        {
            return new(
                SteamworksPageKind.SignIn,
                PortalAccessState.SignedOut,
                null,
                false,
                "The managed profile is signed out.");
        }

        if (ContainsAny(combined, "permission denied", "access denied", "not authorized", "insufficient privilege"))
        {
            return new(
                SteamworksPageKind.PermissionDenied,
                PortalAccessState.PermissionDenied,
                null,
                false,
                "The signed-in account lacks access to this App ID.");
        }

        try
        {
            SteamworksUrlGuard.EnsureAllowed(document.Uri, expectedAppId);
        }
        catch (PortalSafetyException exception)
        {
            return Unknown(exception.Message);
        }

        string? revision = ReadAttribute(html, "data-steamworks-revision");
        string? page = ReadAttribute(html, "data-steamworks-page");
        if (!string.Equals(revision, SteamworksSelectorCatalog.SupportedRevision, StringComparison.Ordinal) &&
            !string.Equals(revision, LiveSteamworksConfiguration.Revision, StringComparison.Ordinal))
        {
            return Unknown("The Steamworks page structure is not a supported portal revision.");
        }

        SteamworksPageKind kind = page switch
        {
            "app-admin" => SteamworksPageKind.AppAdmin,
            "stats-achievements" => SteamworksPageKind.StatsAndAchievements,
            "publication-review" => SteamworksPageKind.PublicationReview,
            _ => SteamworksPageKind.Unknown,
        };

        return kind == SteamworksPageKind.Unknown
            ? Unknown("The supported revision marker has an unknown page type.")
            : new(kind, PortalAccessState.Authenticated, revision, true, "Supported portal structure.");
    }

    private static bool IsPartnerOrigin(Uri uri) =>
        uri.IsAbsoluteUri &&
        string.Equals(uri.Scheme, Uri.UriSchemeHttps, StringComparison.Ordinal) &&
        string.Equals(uri.Host, "partner.steamgames.com", StringComparison.OrdinalIgnoreCase) &&
        uri.IsDefaultPort;

    private static bool ContainsAny(string value, params string[] candidates) =>
        candidates.Any(candidate => value.Contains(candidate, StringComparison.OrdinalIgnoreCase));

    private static SteamworksPageClassification Unknown(string reason) =>
        new(SteamworksPageKind.Unknown, PortalAccessState.Unknown, null, false, reason);

    internal static string? ReadAttribute(string html, string attribute)
    {
        string prefix = attribute + "=";
        int start = html.IndexOf(prefix, StringComparison.OrdinalIgnoreCase);
        if (start < 0)
        {
            return null;
        }

        start += prefix.Length;
        if (start >= html.Length || html[start] is not ('"' or '\''))
        {
            return null;
        }

        char quote = html[start++];
        int end = html.IndexOf(quote, start);
        return end < 0 ? null : html[start..end];
    }
}

public sealed record SteamworksSelectors(
    string Revision,
    string SaveDraft,
    IReadOnlyDictionary<string, string> FieldSelectors,
    IReadOnlyDictionary<string, string> UploadSelectors);

public static class SteamworksSelectorCatalog
{
    public const string SupportedRevision = "2026-09-sanitized-fixture";

    private static readonly IReadOnlyDictionary<string, string> Fields =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["apiName"] = "[name='api_name']",
            ["internalName"] = "[name='display_name']",
            ["hidden"] = "[name='hidden']",
            ["setBy"] = "[name='set_by']",
            ["progressStatApiName"] = "[name='progress_stat']",
            ["unlockValue"] = "[name='unlock_value']",
            ["type"] = "[name='type']",
            ["incrementOnly"] = "[name='increment_only']",
            ["minimum"] = "[name='min_value']",
            ["maximum"] = "[name='max_value']",
            ["defaultValue"] = "[name='default_value']",
            ["maximumChange"] = "[name='max_change']",
            ["aggregated"] = "[name='aggregated']",
            ["averageRateWindowSeconds"] = "[name='window_seconds']",
            ["displayOrder"] = "[name='display_order']",
            ["English"] = "[name='English']",
            ["Polish"] = "[name='Polish']",
            ["ChineseSimplified"] = "[name='ChineseSimplified']",
            ["ChineseTraditional"] = "[name='ChineseTraditional']",
            ["Japanese"] = "[name='Japanese']",
            ["Korean"] = "[name='Korean']",
            ["French"] = "[name='French']",
        };

    private static readonly IReadOnlyDictionary<string, string> Uploads =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["achievedIcon"] = "input[name='achieved_icon']",
            ["unachievedIcon"] = "input[name='unachieved_icon']",
        };

    public static SteamworksSelectors Get(string revision)
    {
        if (!string.Equals(revision, SupportedRevision, StringComparison.Ordinal))
        {
            throw new UnsupportedPortalStructureException(
                $"Steamworks portal revision '{revision}' is not supported.");
        }

        return new SteamworksSelectors(
            SupportedRevision,
            "button[data-achimeld-action='save-draft']",
            Fields,
            Uploads);
    }
}
