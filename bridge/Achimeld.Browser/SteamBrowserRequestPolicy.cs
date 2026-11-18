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
    private BrowserRequestDiagnostic? rejectionDetails;
    public BrowserRequestDiagnostic? RejectionDetails { get { lock (gate) return rejectionDetails; } }
    public BrowserRequestRejection Rejection { get { lock (gate) return rejection; } }
    private PortalEntityKind? draftKind;
    private string? armedPost;
    private Dictionary<string, string>? armedModernForm;
    private (string FileName, byte[] Image)? armedModernIcon;
    private string? armedModernApiName;
    private string? armedModernEnglish;
    private uint modernWriteRejectionMask;
    public uint ModernWriteRejectionMask { get { lock (gate) return modernWriteRejectionMask; } }
    private uint modernWriteIdentityShape;
    public uint ModernWriteIdentityShape { get { lock (gate) return modernWriteIdentityShape; } }
    private uint modernWriteIdentityLocations;
    public uint ModernWriteIdentityLocations { get { lock (gate) return modernWriteIdentityLocations; } }
    private ulong modernWritePartSequence;
    public ulong ModernWritePartSequence { get { lock (gate) return modernWritePartSequence; } }
    private IReadOnlySet<string>? armedKnownApiNames;
    private bool armedModernImport;
    private int discardedStoreBrowseRequests;

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

    public void Reject(BrowserRequestRejection reason = BrowserRequestRejection.Other, BrowserRequestDiagnostic? details = null)
    {
        lock (gate) { if (!rejected) { rejection = reason; rejectionDetails = details; } rejected = true; }
    }
    public void RequireHealthy() { lock (gate) if (rejected) throw new InvalidDataException("Browser request policy rejected traffic."); }

    // Chromium has already enforced CSP before reporting its violation. Do not
    // send report bodies upstream or treat this optional report as a page action.
    public bool DiscardsCspReport(string method, Uri uri, string resourceType, bool navigation, string? contentType)
    {
        lock (gate)
            return !rejected && phase != Phase.Dormant && method == "POST" && !navigation &&
                resourceType is "cspreport" or "other" && contentType == "application/csp-report" &&
                uri.AbsoluteUri == "https://partner.steamgames.com/actions/CSPReport";
    }

    // The new achievements editor asks Steam's store service for optional
    // display metadata. Abort this exact request locally: it is never sent to
    // the broker, and its query is not inspected, stored or trusted.
    public bool DiscardsOptionalStoreBrowse(string method, Uri uri, string resourceType, bool navigation)
    {
        lock (gate)
        {
            if (rejected || appId is null || phase is not (Phase.Authentication or Phase.Read or Phase.ReviewedDraft) ||
                method != "GET" || navigation || resourceType != "fetch" ||
                uri.Scheme != Uri.UriSchemeHttps || uri.IdnHost != "partner.steamgames.com" ||
                uri.AbsolutePath != "/um/StoreBrowse/GetItems/" || uri.Query.Length is < 1 or > 2048 ||
                !uri.Query.StartsWith("?input_protobuf_encoded=", StringComparison.Ordinal) ||
                discardedStoreBrowseRequests >= 16) return false;
            discardedStoreBrowseRequests++;
            return true;
        }
    }

    public Uri ConfigurationUri(uint selected, string endpoint)
    {
        lock (gate)
        {
            RequireRead(selected);
            string route = endpoint switch
            {
                "fetchstats" or "fetchachievements" => "apps/" + endpoint,
                "ajaxgetstatsschema" or "ajaxgetappinfo" => "achievements/" + endpoint,
                _ => throw new InvalidDataException("Configuration endpoint rejected."),
            };
            return new Uri($"https://partner.steamgames.com/{route}/{selected.ToString(CultureInfo.InvariantCulture)}");
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
            armedModernForm = null;
            armedModernIcon = null;
            armedModernApiName = null;
            armedModernEnglish = null;
            modernWriteRejectionMask = 0;
            modernWriteIdentityShape = 0;
            modernWriteIdentityLocations = 0;
            modernWritePartSequence = 0;
            armedKnownApiNames = null;
            armedModernImport = false;
            phase = Phase.ReviewedDraft;
        }
    }

    public void EndReviewedDraft()
    {
        lock (gate) { RequireHealthy(); if (phase != Phase.ReviewedDraft) throw new InvalidDataException("Draft scope unavailable."); draftKind = null; armedPost = null; armedModernForm = null; armedModernIcon = null; armedModernApiName = null; armedModernEnglish = null; armedKnownApiNames = null; armedModernImport = false; phase = Phase.Read; }
    }

    internal Uri ArmReviewedModernForm(uint selected, bool moveSet, Dictionary<string, string> fields)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind != PortalEntityKind.Achievement || armedPost is not null)
                throw new InvalidDataException("Modern achievement draft save is unavailable.");
            string endpoint = moveSet ? "ajaxmoveachievements" : "ajaxcreateorupdateachievement";
            armedPost = $"/achievements/{endpoint}/{selected.ToString(CultureInfo.InvariantCulture)}";
            armedModernForm = new(fields, StringComparer.Ordinal);
            return new Uri("https://partner.steamgames.com" + armedPost);
        }
    }

    internal Uri ArmReviewedModernIcon(uint selected, Dictionary<string, string> fields, string fileName, byte[] image)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind != PortalEntityKind.Icon || armedPost is not null || image.Length is 0 or > 8 * 1024 * 1024)
                throw new InvalidDataException("Modern achievement icon upload is unavailable.");
            armedPost = "/images/uploadachievement";
            armedModernForm = new(fields, StringComparer.Ordinal);
            armedModernIcon = (fileName, image);
            return new Uri("https://partner.steamgames.com" + armedPost);
        }
    }

    public void ArmReviewedModernAchievementPost(uint selected, string apiName, string english,
        IReadOnlySet<string>? knownApiNames = null, bool localizationImport = false)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind != PortalEntityKind.Achievement ||
                armedPost is not null || apiName.Length is < 1 or > 80 ||
                english.Length is < 1 or > 4096 ||
                (localizationImport && (knownApiNames is null || !knownApiNames.Contains(apiName))) ||
                !System.Text.RegularExpressions.Regex.IsMatch(apiName, @"^ACHIEVEMENT_[A-Za-z0-9_]{1,68}$"))
                throw new InvalidDataException("Modern achievement draft save is unavailable.");
            armedPost = $"/achievements/ajaxcreateorupdateachievement/{selected.ToString(CultureInfo.InvariantCulture)}";
            armedModernApiName = apiName;
            armedModernEnglish = english;
            armedKnownApiNames = knownApiNames;
            armedModernImport = localizationImport;
        }
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

    public Uri ArmReviewedDeletion(uint selected, string recordIdentity)
    {
        lock (gate)
        {
            RequireRead(selected);
            if (phase != Phase.ReviewedDraft || draftKind is not (PortalEntityKind.Stat or PortalEntityKind.Achievement) || armedPost is not null)
                throw new InvalidDataException("Reviewed deletion is unavailable.");
            string[] parts = recordIdentity.Split('_');
            if (parts.Length != (draftKind == PortalEntityKind.Stat ? 1 : 2) || !parts.All(RecordPart))
                throw new InvalidDataException("Reviewed deletion identity is invalid.");
            armedPost = $"/apps/{(draftKind == PortalEntityKind.Stat ? "deletestat" : "deleteachievement")}/{selected.ToString(CultureInfo.InvariantCulture)}/{string.Join('/', parts)}";
            return new Uri("https://partner.steamgames.com" + armedPost);
        }
    }

    public bool AllowsDraftNavigation(Uri uri)
    {
        lock (gate) return phase == Phase.ReviewedDraft && draftKind == PortalEntityKind.Stat && appId is uint id &&
            uri.AbsoluteUri == $"https://partner.steamgames.com/apps/stats/{id.ToString(CultureInfo.InvariantCulture)}";
    }

    public bool Allows(string method, Uri uri, string resourceType, bool navigation,
        string? postBody = null, string? contentType = null, byte[]? postBytes = null)
    {
        lock (gate)
        {
            if (rejected || phase == Phase.Dormant || uri.OriginalString.Length > 8192) return false;
            try { SteamNetworkPolicy.RequireHttps(uri); } catch (Exception e) when (e is InvalidDataException or ArgumentException) { return false; }
            if (!string.IsNullOrEmpty(uri.Fragment) || method is not ("GET" or "POST")) return false;
            string host = uri.IdnHost.ToLowerInvariant(), path = uri.AbsolutePath;
            bool partner = host == "partner.steamgames.com";
            // Media has no API/login authority, even when a request uses an allowed endpoint.
            if (resourceType == "media") return method == "GET" && !navigation &&
                BrowserReadPolicy.IsMediaPath(path) && BrowserReadPolicy.IsMediaQuery(uri.Query) &&
                (BrowserReadPolicy.IsContentHost(host) || partner && path.StartsWith("/public/", StringComparison.Ordinal));
            string? pinned = appId?.ToString(CultureInfo.InvariantCulture);
            if (host == "api.steampowered.com")
                return phase == Phase.Authentication && !navigation && resourceType is "xhr" or "fetch" &&
                    IsAuthenticationApi(method, path);
            if (partner && appId is uint pinnedApp && method == "GET" && !navigation && resourceType == "fetch" &&
                IsPinnedAchievementPlayerRead(uri, path, pinnedApp)) return true;
            if (partner && path.StartsWith("/um/", StringComparison.Ordinal))
                return phase == Phase.Authentication && !navigation && resourceType is "xhr" or "fetch" &&
                    IsAuthenticationApi(method, path, browserApi: true);
            if (phase == Phase.Authentication && IsLoginPath(host, path)) return true;
            if (phase == Phase.ReviewedDraft && partner && appId is uint reviewedApp &&
                string.IsNullOrEmpty(uri.Query))
            {
                bool stat = draftKind == PortalEntityKind.Stat;
                if (method == "POST" && armedPost == path &&
                    (draftKind == PortalEntityKind.Icon || !navigation && resourceType is "xhr" or "fetch"))
                {
                    if (armedModernForm is not null && (armedModernIcon is { } icon
                        ? navigation || resourceType is not ("xhr" or "fetch") || !ModernAchievementDraft.MatchesIcon(contentType, postBytes, armedModernForm, icon.FileName, icon.Image)
                        : !ModernAchievementDraft.MatchesForm(contentType, postBody, armedModernForm))) return false;
                    if (armedModernApiName is string apiName &&
                        !(armedModernImport
                            ? SteamworksModernWriteShape.AllowsReviewedLocalizationImport(method,
                                uri, resourceType, contentType, postBody, reviewedApp, apiName,
                                armedModernEnglish!, armedKnownApiNames)
                            : SteamworksModernWriteShape.AllowsReviewed(method, uri, resourceType,
                                contentType, postBody, reviewedApp, apiName, armedModernEnglish!)))
                    {
                        int shape = SteamworksModernWriteShape.Read(method, uri, resourceType,
                            contentType, postBody, reviewedApp, apiName, armedModernEnglish!);
                        int structure = SteamworksModernWriteShape.ReadStructure(contentType, postBody);
                        int guard = SteamworksModernWriteShape.ReadGuardConditions(contentType,
                            postBody, apiName);
                        modernWriteRejectionMask = (uint)shape | ((uint)structure << 14) |
                            ((uint)guard << 30);
                        if ((guard & 2) == 0)
                        {
                            modernWriteIdentityShape = (uint)SteamworksModernWriteShape.ReadIdentityShape(
                                postBody, apiName);
                            modernWriteIdentityLocations = SteamworksModernWriteShape.ReadIdentityLocations(
                                contentType, postBody, apiName, armedModernEnglish!);
                            modernWritePartSequence = SteamworksModernWriteShape.ReadMultipartSequence(
                                contentType, postBody, apiName, armedModernEnglish!, armedKnownApiNames);
                        }
                        return false;
                    }
                    armedPost = null;
                    armedModernForm = null;
                    armedModernIcon = null;
                    armedModernApiName = null;
                    armedModernEnglish = null;
                    armedKnownApiNames = null;
                    armedModernImport = false;
                    return true;
                }
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
                // The achievements editor loads these read-only views for
                // the selected app after its document has loaded.
                if (!navigation && resourceType == "xhr" && pinned is not null &&
                    (path == $"/achievements/ajaxgetappinfo/{pinned}" ||
                     path == $"/achievements/ajaxgetdlc/{pinned}" ||
                     path == $"/achievements/ajaxgetstatsschema/{pinned}" ||
                     path == $"/achievements/ajaxgetlanguagelist/{pinned}" ||
                     path == $"/achievements/ajaxgetlanguageoptionslist/{pinned}")) return true;
                if (phase == Phase.Read && !navigation && resourceType == "fetch" && pinned is not null &&
                    (path == $"/achievements/ajaxgetstatsschema/{pinned}" ||
                     path == $"/achievements/ajaxgetappinfo/{pinned}")) return true;
                if (navigation && ((phase == Phase.Authentication && path == "/") ||
                    (pinned is not null && (path == $"/apps/achievements/{pinned}" ||
                        ((phase is Phase.Authentication or Phase.Read) && path == $"/apps/stats/{pinned}"))))) return true;
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

    private static bool IsPinnedAchievementPlayerRead(Uri uri, string path, uint pinnedApp)
    {
        bool game = path == "/um/Player/GetGameAchievements/";
        if (!game && path != "/um/Player/GetGlobalAchievementPercentages/") return false;
        // Steam encodes the App ID as protobuf field 1. The game-achievements
        // request also contains the fixed English language field. Compare the
        // complete payload so a page cannot select another app or add fields.
        List<byte> payload = [0x08];
        uint value = pinnedApp;
        do
        {
            byte part = (byte)(value & 0x7f);
            value >>= 7;
            payload.Add(value == 0 ? part : (byte)(part | 0x80));
        } while (value != 0);
        if (game)
        {
            payload.Add(0x12);
            payload.Add(0x07);
            payload.AddRange(System.Text.Encoding.ASCII.GetBytes("english"));
        }
        return Uri.UnescapeDataString(uri.Query) == "?input_protobuf_encoded=" + Convert.ToBase64String([.. payload]);
    }

    // Steam's public login component uses these anonymous authentication RPCs.
    // No account-management, token-administration or generic API authority.
    private static bool IsAuthenticationApi(string method, string path, bool browserApi = false)
    {
        string prefix = browserApi ? "/um/Authentication/" : "/IAuthenticationService/";
        string suffix = browserApi ? "/" : "/v1";
        if (!path.StartsWith(prefix, StringComparison.Ordinal) || !path.EndsWith(suffix, StringComparison.Ordinal) ||
            path.Length <= prefix.Length + suffix.Length) return false;
        string rpc = path[prefix.Length..^suffix.Length];
        return method switch {
            "GET" => rpc == "GetPasswordRSAPublicKey",
            "POST" => rpc is "BeginAuthSessionViaQR" or "BeginAuthSessionViaCredentials" or
                "PollAuthSessionStatus" or "UpdateAuthSessionWithSteamGuardCode",
            _ => false,
        };
    }

    private static bool IsLoginPath(string host, string path) =>
        (host is "partner.steamgames.com" or "store.steampowered.com" or "steamcommunity.com" or "login.steampowered.com") &&
        (path is "/login" or "/openid" || path.StartsWith("/login/", StringComparison.Ordinal) || path.StartsWith("/openid/", StringComparison.Ordinal) ||
         (host == "login.steampowered.com" && path is "/jwt/finalizelogin" or "/jwt/ajaxrefresh"));
}
