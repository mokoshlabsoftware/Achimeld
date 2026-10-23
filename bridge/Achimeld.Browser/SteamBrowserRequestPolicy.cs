using System.Globalization;
using Achimeld.Security;

namespace Achimeld.Browser;

public enum BrowserRequestRejection { None, OtherPage, Navigation, PartnerHome, PartnerDashboard, Script, Stylesheet, Image, Font, Dynamic, WebSocket, Other, Ping, Media, Manifest, EventSource, UnavailableFrame }

// SEC-READ-002: This guards trusted application/browser requests, not compromised
// worker code. CONNECT/WFP still cannot inspect HTTPS methods or paths.
public sealed class SteamBrowserRequestPolicy
{
    private enum Phase { Dormant, Authentication, Read, ReviewedDraft }
    private readonly object gate = new();
    private Phase phase;
    private uint? appId;
    private bool rejected;
    private BrowserRequestRejection rejection;
    public BrowserRequestRejection Rejection { get { lock (gate) return rejection; } }
    private PortalEntityKind? draftKind;
    private string? armedPost;

    public void BeginAuthentication()
    {
        lock (gate)
        {
            RequireHealthy();
            if (phase is Phase.Read or Phase.ReviewedDraft) throw new InvalidDataException("Read sessions cannot regain login authority.");
            phase = Phase.Authentication;
        }
    }

    public void PinApp(uint selected)
    {
        lock (gate)
        {
            RequireHealthy();
            if (selected == 0 || (appId is uint pinned && pinned != selected))
                throw new InvalidDataException("Browser App ID changed.");
            appId = selected;
            if (phase == Phase.Dormant) phase = Phase.Authentication;
        }
    }

    public void EnterRead()
    {
        lock (gate)
        {
            RequireHealthy();
            if (appId is null) throw new InvalidDataException("Read phase requires a pinned App ID.");
            if (phase != Phase.ReviewedDraft) phase = Phase.Read;
        }
    }

    public void Reject(BrowserRequestRejection reason = BrowserRequestRejection.Other) { lock (gate) { if (!rejected) rejection = reason; rejected = true; } }
    public void RequireHealthy() { lock (gate) if (rejected) throw new InvalidDataException("Browser request policy rejected traffic."); }

    public Uri ConfigurationUri(uint selected, string endpoint)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (endpoint is not ("fetchstats" or "fetchachievements")) throw new InvalidDataException("Configuration endpoint rejected.");
            return new Uri($"https://partner.steamgames.com/apps/{endpoint}/{selected.ToString(CultureInfo.InvariantCulture)}");
        }
    }

    public void RequireIcon(Uri uri)
    {
        lock (gate)
        {
            RequireRead(appId);
            BrowserReadPolicy.RequireIcon(uri);
        }
    }

    public Uri GameBannerUri(uint selected)
    {
        lock (gate)
        {
            RequireRead(selected);
            return new Uri($"https://cdn.akamai.steamstatic.com/steam/apps/{selected.ToString(CultureInfo.InvariantCulture)}/capsule_231x87_2x.jpg");
        }
    }

    public Uri GameBannerMetadataUri(uint selected)
    {
        lock (gate)
        {
            RequireRead(selected);
            return new Uri($"https://store.steampowered.com/api/appdetails?appids={selected.ToString(CultureInfo.InvariantCulture)}&filters=basic");
        }
    }

    public Uri? ResolveGameBanner(uint selected, byte[] metadata)
    {
        lock (gate)
        {
            RequireRead(selected);
            return SteamGameBanner.Resolve(selected, metadata);
        }
    }

    private void RequireRead(uint? selected)
    {
        RequireHealthy();
        if (phase is not (Phase.Read or Phase.ReviewedDraft) || selected is null || appId != selected) throw new InvalidDataException("Browser read phase unavailable.");
    }

    public void BeginReviewedDraft(uint selected, PortalEntityKind kind)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.Read || kind is not (PortalEntityKind.Stat or PortalEntityKind.Achievement or PortalEntityKind.Icon)) throw new InvalidDataException("Draft scope unavailable.");
            draftKind = kind;
            armedPost = null;
            phase = Phase.ReviewedDraft;
        }
    }

    public void EndReviewedDraft()
    {
        lock (gate) { RequireHealthy(); if (phase != Phase.ReviewedDraft) throw new InvalidDataException("Draft scope unavailable."); draftKind = null; armedPost = null; phase = Phase.Read; }
    }

    public void ArmReviewedPost(uint selected)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind is null || armedPost is not null) throw new InvalidDataException("Draft save is not available.");
            armedPost = draftKind == PortalEntityKind.Icon ? "/images/uploadachievement" :
                $"/apps/{(draftKind == PortalEntityKind.Stat ? "savestat" : "saveachievement")}/{selected.ToString(CultureInfo.InvariantCulture)}";
        }
    }

    public void ArmReviewedCreation(uint selected)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind is not (PortalEntityKind.Stat or PortalEntityKind.Achievement) || armedPost is not null)
                throw new InvalidDataException("Draft creation is not available.");
            armedPost = $"/apps/{(draftKind == PortalEntityKind.Stat ? "newstat" : "newachievement")}/{selected.ToString(CultureInfo.InvariantCulture)}";
        }
    }

    public bool AllowsDraftNavigation(Uri uri)
    {
        lock (gate) return phase == Phase.ReviewedDraft && draftKind == PortalEntityKind.Stat && appId is uint id &&
            uri.AbsoluteUri == $"https://partner.steamgames.com/apps/stats/{id.ToString(CultureInfo.InvariantCulture)}";
    }

    public bool Allows(string method, Uri uri, string resourceType, bool navigation)
    {
        lock (gate)
        {
            if (rejected || phase == Phase.Dormant || uri.OriginalString.Length > 8192) return false;
            try { SteamNetworkPolicy.RequireHttps(uri); } catch (Exception e) when (e is InvalidDataException or ArgumentException) { return false; }
            if (!string.IsNullOrEmpty(uri.Fragment) || method is not ("GET" or "POST")) return false;
            string host = uri.IdnHost.ToLowerInvariant(), path = uri.AbsolutePath;
            bool partner = host == "partner.steamgames.com";
            string? pinned = appId?.ToString(CultureInfo.InvariantCulture);
            if (phase == Phase.Authentication && IsLoginPath(host, path)) return true;
            if (phase == Phase.ReviewedDraft && partner && string.IsNullOrEmpty(uri.Query))
            {
                bool stat = draftKind == PortalEntityKind.Stat;
                if (method == "POST" && armedPost == path &&
                    (draftKind == PortalEntityKind.Icon || !navigation && resourceType is "xhr" or "fetch")) { armedPost = null; return true; }
                if (method == "GET" && navigation && stat && path == $"/apps/stats/{pinned}") return true;
                // Steam's legacy editor uses POST for individual record reads.
                // Keep them confined to canonical pinned-app metadata routes.
                if (!navigation && resourceType is "xhr" or "fetch")
                {
                    string[] parts = path.Split('/');
                    if (parts.Length >= 5 && parts[1] == "apps" && parts[3] == pinned &&
                        (stat ? parts.Length == 5 && parts[2] == "fetchstat" && RecordPart(parts[4])
                            : parts.Length == 6 && parts[2] == "fetchachievement" && RecordPart(parts[4]) && RecordPart(parts[5]))) return true;
                }
            }
            if (method != "GET") return false;
            if (partner && string.IsNullOrEmpty(uri.Query))
            {
                if (navigation && ((phase == Phase.Authentication && path == "/") || (pinned is not null && path == $"/apps/achievements/{pinned}"))) return true;
                if (!navigation && pinned is not null && (path == $"/apps/fetchstats/{pinned}" || path == $"/apps/fetchachievements/{pinned}" || path == $"/apps/getachievementcounts/{pinned}")) return true;
            }
            if (navigation || resourceType is not ("script" or "stylesheet" or "image" or "font")) return false;
            return BrowserReadPolicy.IsStaticPath(path) &&
                (BrowserReadPolicy.IsContentHost(host) ||
                 ((partner || (phase == Phase.Authentication && host is "store.steampowered.com" or "steamcommunity.com" or "login.steampowered.com")) &&
                  (path.StartsWith("/public/", StringComparison.Ordinal) || path.StartsWith("/javascript/", StringComparison.Ordinal) ||
                   path.StartsWith("/css/", StringComparison.Ordinal) || path.StartsWith("/images/", StringComparison.Ordinal))));
        }
    }
    private static bool RecordPart(string text) => text.Length is > 0 and <= 10 &&
        uint.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out uint id) && text == id.ToString(CultureInfo.InvariantCulture);

    private static bool IsLoginPath(string host, string path) =>
        (host is "partner.steamgames.com" or "store.steampowered.com" or "steamcommunity.com" or "login.steampowered.com") &&
        (path is "/login" or "/openid" || path.StartsWith("/login/", StringComparison.Ordinal) || path.StartsWith("/openid/", StringComparison.Ordinal) ||
         (host == "login.steampowered.com" && path is "/jwt/finalizelogin" or "/jwt/ajaxrefresh"));
}
