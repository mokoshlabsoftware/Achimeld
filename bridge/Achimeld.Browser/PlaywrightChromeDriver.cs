using System.Collections.Concurrent;
using System.Text.Json;
using System.Security.Cryptography;
using System.Text;
using System.Net;
using System.Diagnostics;
using Achimeld.Core;
using Microsoft.Playwright;

namespace Achimeld.Browser;

// Closed, content-free facts about a reviewed synchronization operation.
public sealed record SyncOperationDiagnostic(string Phase, long PhaseElapsedMs, string Endpoint,
    string WriteState, string ResponseClass, uint SchemaMask = 0, string SchemaKind = "missing",
    int SchemaItems = 0, int SchemaNamed = 0, int SchemaObjects = 0, int SchemaLargestArray = 0,
    uint SchemaTypes = 0, uint SchemaContainerMask = 0, int SchemaContainerObjects = 0,
    int SchemaContainerArrays = 0, uint DomFlags = 0, int DomTables = 0,
    int DomTableRows = 0, int DomGridRows = 0, int DomFrames = 0,
    int DomApiTokens = 0, int DomApiLeaves = 0, int DomAchievementClasses = 0,
    int DomImages = 0, int DomButtons = 0, int DomListItems = 0,
    int DomSingleTokenCards = 0, int DomCardsWithTwoImages = 0,
    int DomCardsWithButtons = 0, int DomCardsWithIdText = 0,
    int DomCardsWithInputs = 0, int DomLargestCardDepth = 0,
    string DomCardLinePattern = "", int DomCardPatternMatches = 0,
    int HtmlApiTokens = 0, int HtmlScripts = 0, uint HtmlFieldMask = 0,
    int AppInfoApiTokens = 0, uint AppInfoFieldMask = 0,
    uint DomExportMask = 0,
    int ExportDefinitionsRows = 0, int ExportLocalizationRows = 0,
    uint ExportDefinitionsHeaderMask = 0, uint ExportLocalizationHeaderMask = 0,
    uint ExportTransportMask = 0,
    int DomIconOrderEvidence = 0,
    string DomLinkAction = "none");

public sealed partial class PlaywrightChromeDriver : IAsyncDisposable
{
    private static readonly string[] IconFields = ["icon", "icon_gray"];
    private readonly SteamworksPageClassifier _classifier = new();
    private readonly ConcurrentQueue<IPage> _unexpectedPages = new();
    private readonly Dictionary<string, string> _iconContentHashes = new(StringComparer.Ordinal);
    private readonly Dictionary<string, byte[]> _iconPreviewBytes = new(StringComparer.Ordinal);
    private Achimeld.Security.SupervisionSession? _supervision;
    private IPlaywright? _playwright;
    private HttpClient? _iconDownloads;

    private IBrowserContext? _context;
    private IPage? _page;
    private ProfileMutexLock? _profileLock;
    private bool _creatingOwnedPage;
    private bool _started;
    private SteamBrowserRequestPolicy _requestPolicy = new();
    public BrowserRequestRejection RequestRejection => _requestPolicy.Rejection;
    private BrowserRequestDiagnostic? _lastRequestFailure;
    public BrowserRequestDiagnostic? RequestRejectionDetails => _requestPolicy.RejectionDetails ?? Volatile.Read(ref _lastRequestFailure);
    private string _syncPhase = "none";
    private long _syncPhaseStarted = Stopwatch.GetTimestamp();
    private string _syncEndpoint = "none";
    private string _syncWriteState = "none";
    private string _syncResponseClass = "none";
    private uint _syncSchemaMask;
    private SteamworksSchemaOutline? _syncSchemaOutline;
    private SteamworksAchievementPageShape? _syncDomShape;
    private int _syncHtmlApiTokens;
    private int _syncHtmlScripts;
    private uint _syncHtmlFieldMask;
    private int _syncAppInfoApiTokens;
    private uint _syncAppInfoFieldMask;
    private SteamworksCsvExportShape? _syncExportShape;
    private SteamworksCsvExportData? _modernExport;
    private Dictionary<string, ModernAchievementRecord> _modernAchievementRecords = new(StringComparer.Ordinal);
    private bool _modernImportNeedsFreshPage;
    private int _syncIconOrderEvidence;
    private IReadOnlyDictionary<string, ModernAchievementIconPair> _priorIconPairs =
        new Dictionary<string, ModernAchievementIconPair>(StringComparer.Ordinal);
    private int _armedExportDownload;
    public SyncOperationDiagnostic SyncDiagnostic => new(_syncPhase,
        Math.Clamp((long)Stopwatch.GetElapsedTime(_syncPhaseStarted).TotalMilliseconds, 0, 300000),
        _syncEndpoint, _requestPolicy.ModernWritePartSequence is ulong sequence && sequence != 0
            ? $"modern-part-sequence-{sequence}" :
            _requestPolicy.ModernWriteIdentityLocations is uint locations && locations != 0
            ? $"modern-id-parts-{locations}" :
            _requestPolicy.ModernWriteIdentityShape is uint identityShape && identityShape != 0
            ? $"modern-identity-shape-{identityShape}" :
            _requestPolicy.ModernWriteRejectionMask is uint rejectedShape && rejectedShape != 0
                ? $"modern-route-shape-{rejectedShape}" : _syncWriteState,
        _syncResponseClass, _syncSchemaMask,
        _syncSchemaOutline?.Kind ?? "missing", _syncSchemaOutline?.ItemCount ?? 0,
        _syncSchemaOutline?.NamedCount ?? 0, _syncSchemaOutline?.ObjectCount ?? 0,
        _syncSchemaOutline?.LargestArray ?? 0, _syncSchemaOutline?.TypeMask ?? 0,
        _syncSchemaOutline?.ContainerMask ?? 0, _syncSchemaOutline?.ContainerObjects ?? 0,
        _syncSchemaOutline?.ContainerArrays ?? 0, _syncDomShape?.Flags ?? 0,
        _syncDomShape?.Tables ?? 0, _syncDomShape?.TableRows ?? 0,
        _syncDomShape?.GridRows ?? 0, _syncDomShape?.Frames ?? 0,
        _syncDomShape?.ApiTokens ?? 0, _syncDomShape?.ApiLeaves ?? 0,
        _syncDomShape?.AchievementClasses ?? 0, _syncDomShape?.Images ?? 0,
        _syncDomShape?.Buttons ?? 0, _syncDomShape?.ListItems ?? 0,
        _syncDomShape?.SingleTokenCards ?? 0,
        _syncDomShape?.CardsWithTwoImages ?? 0,
        _syncDomShape?.CardsWithButtons ?? 0,
        _syncDomShape?.CardsWithIdText ?? 0,
        _syncDomShape?.CardsWithInputs ?? 0,
        _syncDomShape?.LargestCardDepth ?? 0,
        _syncDomShape?.CardLinePattern ?? "", _syncDomShape?.CardPatternMatches ?? 0,
        _syncHtmlApiTokens, _syncHtmlScripts, _syncHtmlFieldMask,
        _syncAppInfoApiTokens, _syncAppInfoFieldMask,
        _syncDomShape?.ExportMask ?? 0,
        _syncExportShape?.DefinitionsRows ?? 0, _syncExportShape?.LocalizationRows ?? 0,
        _syncExportShape?.DefinitionsHeaderMask ?? 0,
        _syncExportShape?.LocalizationHeaderMask ?? 0,
        _syncExportShape?.TransportMask ?? 0,
        _syncIconOrderEvidence,
        _syncDomShape?.LinkAction ?? "none");
    public void BeginDiagnosticOperation()
    {
        Volatile.Write(ref _lastRequestFailure, null);
        _syncPhase = "none";
        _syncPhaseStarted = Stopwatch.GetTimestamp();
        _syncEndpoint = "none";
        _syncWriteState = "none";
        _syncResponseClass = "none";
        _syncSchemaMask = 0;
        _syncSchemaOutline = null;
        _syncDomShape = null;
        _syncHtmlApiTokens = 0;
        _syncHtmlScripts = 0;
        _syncHtmlFieldMask = 0;
        _syncAppInfoApiTokens = 0;
        _syncAppInfoFieldMask = 0;
        _syncExportShape = null;
        _syncIconOrderEvidence = 0;
    }
    public void SetSyncDiagnosticPhase(string phase)
    {
        _syncPhase = phase;
        _syncPhaseStarted = Stopwatch.GetTimestamp();
        _syncEndpoint = "none";
        _syncResponseClass = "none";
        _syncSchemaMask = 0;
        _syncSchemaOutline = null;
        _syncDomShape = null;
        _syncHtmlApiTokens = 0;
        _syncHtmlScripts = 0;
        _syncHtmlFieldMask = 0;
        _syncAppInfoApiTokens = 0;
        _syncAppInfoFieldMask = 0;
        _syncExportShape = null;
        _syncIconOrderEvidence = 0;
    }
    public void SetSyncDiagnosticWriteState(string state) => _syncWriteState = state;
    public void SetPriorIconPairs(IReadOnlyDictionary<string, ModernAchievementIconPair> pairs) =>
        _priorIconPairs = pairs;
    public void SetSyncDiagnosticEndpoint(string endpoint)
    {
        _syncEndpoint = endpoint;
        _syncResponseClass = "none";
    }
    public void SetSyncDiagnosticResponseStatus(int status) => _syncResponseClass = status switch
    {
        >= 200 and < 300 => "2xx", >= 300 and < 400 => "3xx",
        >= 400 and < 500 => "4xx", >= 500 and < 600 => "5xx", _ => "other",
    };
    public string DiagnosticStage { get; private set; } = "idle";


    public async Task StartAsync(Achimeld.Security.SupervisionSession supervision)
    {
        ArgumentNullException.ThrowIfNull(supervision);
        await supervision.RequireHealthyAsync(CancellationToken.None).ConfigureAwait(false);
        ChromeLaunchSettings settings = new(supervision.Status.BrowserExecutable, supervision.Status.ProfileDirectory, ChromeLaunchSettings.DefaultTimeout) { ProxyPort = supervision.Status.ProxyPort };
        _supervision = supervision;
        if (_started && _context?.Browser is { IsConnected: true }) return;
        await StopCoreAsync().ConfigureAwait(false);
        if (settings.ProxyPort is not >= 1024 or > 65535)
            throw new InvalidOperationException("Steam browser startup requires the supervised network broker.");
        Achimeld.Security.DedicatedProfilePolicy.Resolve(settings.ProfileDirectory);
        Achimeld.Security.DedicatedProfilePolicy.RejectReparsePoints(settings.ProfileDirectory);
        Directory.CreateDirectory(settings.ProfileDirectory);
        _profileLock = ProfileMutexLock.Acquire(settings.ProfileDirectory, settings.OperationTimeout);
        try
        {
            _iconDownloads = new HttpClient(new SocketsHttpHandler {
                AllowAutoRedirect = false, UseCookies = false, UseProxy = true,
                Proxy = new WebProxy($"http://127.0.0.1:{settings.ProxyPort}") { BypassProxyOnLocal = false },
                AutomaticDecompression = DecompressionMethods.GZip | DecompressionMethods.Deflate | DecompressionMethods.Brotli,
            }) { Timeout = settings.OperationTimeout };
            PlaywrightRuntimePolicy.Configure(supervision.PackageRoot);
            _playwright = await Playwright.CreateAsync().ConfigureAwait(false);
            _requestPolicy = new();
            _context = await _playwright.Chromium.LaunchPersistentContextAsync(
                settings.ProfileDirectory,
                new BrowserTypeLaunchPersistentContextOptions
                {
                    ExecutablePath = settings.ExecutablePath,
                    Proxy = new Proxy { Server = $"http://127.0.0.1:{settings.ProxyPort}" },
                    Headless = false,
                    ChromiumSandbox = true,
                    AcceptDownloads = true,
                    IgnoreHTTPSErrors = false,
                    BypassCSP = false,
                    Offline = true,
                    ServiceWorkers = ServiceWorkerPolicy.Block,
                    Locale = "en-US",
                    Args = BrowserStartup.LaunchArguments,
                    Timeout = (float)settings.OperationTimeout.TotalMilliseconds,
                }).ConfigureAwait(false);
            _context.SetDefaultTimeout((float)settings.OperationTimeout.TotalMilliseconds);
            _context.SetDefaultNavigationTimeout((float)settings.OperationTimeout.TotalMilliseconds);
            _context.RequestFailed += (_, request) => {
                try {
                    if (ReferenceEquals(request.Frame.Page, _page))
                        Volatile.Write(ref _lastRequestFailure, BrowserRequestDiagnostic.Describe(request.Method,
                            request.Url, request.IsNavigationRequest, request.ResourceType, request.Failure ?? "unknown"));
                } catch (PlaywrightException) { }
            };

            // Never enable networking for restored pages from the persistent profile.
            _page = await BrowserStartup.CreateOwnedPageAsync(_context, _requestPolicy, () => _page).ConfigureAwait(false);
            _page.Download += OnOwnedDownload;
            _context.Page += OnContextPage;
            await _context.SetOfflineAsync(false).ConfigureAwait(false);
            _started = true;
        }
        catch
        {
            await StopCoreAsync().ConfigureAwait(false);
            throw;
        }
    }

    public async Task<PortalAccessState> ReadAuthenticationStateAsync()
    {
        await RequireHealthyAsync().ConfigureAwait(false);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        PortalAccessState state = await SteamAuthenticationProbe.ReadAsync(page).ConfigureAwait(false);
        await RequireHealthyAsync().ConfigureAwait(false);
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        return state;
    }

    public async Task NavigateForAuthenticationAsync()
    {
        await RequireHealthyAsync().ConfigureAwait(false);
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        _requestPolicy.BeginAuthentication();
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        await page.BringToFrontAsync().ConfigureAwait(false);
        await page.GotoAsync(
            SteamworksUrlGuard.AllowedOrigin,
            new PageGotoOptions { WaitUntil = WaitUntilState.DOMContentLoaded }).ConfigureAwait(false);
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
    }

    public async Task NavigateAsync(Uri uri)
    {
        DiagnosticStage = "navigation.health";
        await RequireHealthyAsync().ConfigureAwait(false);
        DiagnosticStage = "navigation.route";
        ArgumentNullException.ThrowIfNull(uri);
        IReadOnlyList<uint> appIds = SteamworksUrlGuard.ExtractAppIds(uri);
        if (appIds.Count != 1 || appIds[0] == 0)
        {
            throw new PortalSafetyException(
                "portal.navigation.rejected",
                "Managed navigation requires one nonzero Steam App ID.");
        }

        SteamAppId appId = new(appIds[0]);
        SteamworksUrlGuard.EnsureAllowed(uri, appId);
        if (uri.AbsoluteUri != $"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{appId.Value}" &&
            uri.AbsoluteUri != $"{SteamworksUrlGuard.AllowedOrigin}/apps/stats/{appId.Value}" &&
            !_requestPolicy.AllowsDraftNavigation(uri))
            throw new InvalidDataException("Browser navigation route rejected.");
        _requestPolicy.PinApp(appId.Value);
        DiagnosticStage = "navigation.popup-check";
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        DiagnosticStage = "navigation.page";
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        DiagnosticStage = "navigation.dom-content-loaded";
        await page.GotoAsync(
            uri.AbsoluteUri,
            new PageGotoOptions { WaitUntil = WaitUntilState.DOMContentLoaded }).ConfigureAwait(false);
        DiagnosticStage = "navigation.final-popup-check";
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        DiagnosticStage = "navigation.login-prompt";
        if (uri.AbsoluteUri == $"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{appId.Value}" ||
            uri.AbsoluteUri == $"{SteamworksUrlGuard.AllowedOrigin}/apps/stats/{appId.Value}")
            await SteamLoginPrompt.OpenIfNeededAsync(page, uri).ConfigureAwait(false);
        DiagnosticStage = "navigation.complete";
    }

    public bool BrowserOpen() => _page is { IsClosed: false } && _context?.Browser is { IsConnected: true };

    public async Task<ManagedPageDocument> ReadDocumentAsync()
    {
        await RequireHealthyAsync().ConfigureAwait(false);
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        string title = await page.TitleAsync().ConfigureAwait(false);
        string html = await page.EvaluateAsync<string>(PortalDocumentSanitizer.Script).ConfigureAwait(false);
        return new ManagedPageDocument(new Uri(page.Url, UriKind.Absolute), title, html);
    }

    public async Task<ManagedPageDocument> ReadCompleteConfigurationAsync(SteamAppId appId)
    {
        _modernExport = null;
        _modernAchievementRecords.Clear();
        await EnterReadForAppAsync(appId).ConfigureAwait(false);
        if (_modernImportNeedsFreshPage)
        {
            // Import Save leaves the editor on its upload preview. Reload the
            // pinned app document before collecting cards and CSV exports for
            // read-back; a stale preview can keep the export tab unclickable.
            IPage readbackPage = await EnsurePageAsync().ConfigureAwait(false);
            Uri canonical = new($"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{appId.Value}");
            SteamworksUrlGuard.EnsureAllowed(canonical, appId);
            await readbackPage.GotoAsync(canonical.AbsoluteUri,
                new PageGotoOptions { WaitUntil = WaitUntilState.DOMContentLoaded })
                .ConfigureAwait(false);
            _requestPolicy.RequireHealthy();
            _modernImportNeedsFreshPage = false;
        }
        string stats = await ReadConfigurationJsonAsync(appId, "fetchstats").ConfigureAwait(false);
        SetSyncDiagnosticWriteState("read-stats");
        string achievements = await ReadConfigurationJsonAsync(appId, "fetchachievements").ConfigureAwait(false);
        char first = achievements.TrimStart().FirstOrDefault();
        if (first == '<')
        {
            SetSyncDiagnosticWriteState("read-achievements-html");
            _syncHtmlApiTokens = Math.Min(System.Text.RegularExpressions.Regex.Matches(
                achievements, @"\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b")
                .Select(match => match.Value).Distinct(StringComparer.Ordinal).Count(), 10000);
            _syncHtmlScripts = Math.Min(System.Text.RegularExpressions.Regex.Matches(
                achievements, @"<script\b", System.Text.RegularExpressions.RegexOptions.IgnoreCase).Count, 10000);
            foreach ((string field, int bit) in new[] { ("api_name", 0), ("display_name", 1),
                ("description", 2), ("icon_gray", 3), ("bit_id", 4), ("permission", 5) })
                if (achievements.Contains(field, StringComparison.Ordinal)) _syncHtmlFieldMask |= 1u << bit;
            await Task.Delay(TimeSpan.FromSeconds(1)).ConfigureAwait(false);
            _syncDomShape = await SteamworksAchievementPageShape.ReadAsync(
                await EnsurePageAsync().ConfigureAwait(false), appId).ConfigureAwait(false);
            var cardRead = await SteamworksModernAchievementCards.ReadAsync(
                await EnsurePageAsync().ConfigureAwait(false), _priorIconPairs).ConfigureAwait(false);
            _syncIconOrderEvidence = cardRead.EvidenceCount;
            Dictionary<string, ModernAchievementIconPair> iconPairs = cardRead.Pairs;
            string schema = await ReadConfigurationJsonAsync(appId, "ajaxgetstatsschema").ConfigureAwait(false);
            _modernAchievementRecords = ModernAchievementDraft.ReadRecords(schema);
            _syncSchemaOutline = SteamworksSchemaShape.Describe(schema);
            _syncSchemaMask = _syncSchemaOutline.Mask;
            string appInfo = await ReadConfigurationJsonAsync(appId, "ajaxgetappinfo").ConfigureAwait(false);
            _syncSchemaOutline = SteamworksSchemaShape.Describe(appInfo);
            _syncSchemaMask = _syncSchemaOutline.Mask;
            _syncAppInfoApiTokens = Math.Min(System.Text.RegularExpressions.Regex.Matches(
                appInfo, @"\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b")
                .Select(match => match.Value).Distinct(StringComparer.Ordinal).Count(), 10000);
            foreach ((string field, int bit) in new[] { ("api_name", 0), ("display_name", 1),
                ("description", 2), ("icon_gray", 3), ("bit_id", 4), ("permission", 5),
                ("achievements", 6), ("localization", 7), ("stats", 8), ("setid", 9) })
                if (appInfo.Contains(field, StringComparison.Ordinal)) _syncAppInfoFieldMask |= 1u << bit;
            await _syncDomShape.ProbeExportControlsAsync(await EnsurePageAsync().ConfigureAwait(false)).ConfigureAwait(false);
            SetSyncDiagnosticWriteState("modern-export-reading");
            SteamworksCsvExportData export = await SteamworksCsvExportProbe.ReadAsync(
                await EnsurePageAsync().ConfigureAwait(false), appId,
                armed => Volatile.Write(ref _armedExportDownload, armed ? 1 : 0)).ConfigureAwait(false);
            _modernExport = export;
            _syncExportShape = export.Shape;
            SetSyncDiagnosticWriteState("modern-export-probed");
            string modernJson = ModernSteamworksConfiguration.AchievementJson(export, iconPairs,
                SetSyncDiagnosticWriteState);
            SetSyncDiagnosticWriteState("modern-export-adapted");
            // The export tab hides every achievement card. Leave the supervised
            // browser on the verified management view for the review precheck.
            IPage modernPage = await EnsurePageAsync().ConfigureAwait(false);
            ILocator manageTab = modernPage.GetByText("Manage Achievements", new() { Exact = true });
            if (await manageTab.CountAsync().ConfigureAwait(false) != 1)
                throw new UnsupportedPortalStructureException("The achievement management tab is unavailable.");
            await manageTab.ClickAsync().ConfigureAwait(false);
            await modernPage.WaitForFunctionAsync(
                "expected => [...document.querySelectorAll('*')].filter(element => element.children.length === 0 && /^ACHIEVEMENT_[A-Za-z0-9_]{1,80}$/.test((element.textContent || '').trim())).length === expected",
                iconPairs.Count, new() { Timeout = 15000 }).ConfigureAwait(false);
            var restoredCards = await SteamworksModernAchievementCards.ReadAsync(modernPage, _priorIconPairs)
                .ConfigureAwait(false);
            if (restoredCards.Pairs.Count != iconPairs.Count)
                throw new UnsupportedPortalStructureException("The achievement management view is incomplete.");
            SetSyncDiagnosticWriteState("modern-management-restored");
            await ReadIconContentHashesAsync(modernJson).ConfigureAwait(false);
            SetSyncDiagnosticWriteState("read-icons");
            return ModernSteamworksConfiguration.RemoveSyntheticAchievementIds(
                LiveSteamworksConfiguration.Normalize(appId, stats, modernJson,
                    _iconContentHashes, _iconPreviewBytes));
        }
        else if (first is not ('{' or '[')) SetSyncDiagnosticWriteState("read-achievements-non-json");
        using (JsonDocument shape = JsonDocument.Parse(achievements))
        {
            SetSyncDiagnosticWriteState(shape.RootElement.ValueKind == JsonValueKind.Object &&
                shape.RootElement.TryGetProperty("achievements", out JsonElement rows) && rows.ValueKind == JsonValueKind.Array
                ? "read-achievements-array" : "read-achievements-other");
        }
        await ReadIconContentHashesAsync(achievements).ConfigureAwait(false);
        SetSyncDiagnosticWriteState("read-icons");
        return LiveSteamworksConfiguration.Normalize(appId, stats, achievements, _iconContentHashes, _iconPreviewBytes);
    }

    private async Task EnterReadForAppAsync(SteamAppId appId)
    {
        await RequireHealthyAsync().ConfigureAwait(false);
        SteamworksUrlGuard.EnsureAllowed(new Uri((await EnsurePageAsync().ConfigureAwait(false)).Url), appId);
        if (await ReadAuthenticationStateAsync().ConfigureAwait(false) != PortalAccessState.Authenticated)
            throw new PortalSafetyException("portal.authentication.required", "Steamworks authentication is required.");
        _requestPolicy.PinApp(appId.Value);
        _requestPolicy.EnterRead();
    }

    private async Task ReadIconContentHashesAsync(string achievementsJson)
    {
        // Re-read content on every snapshot, including URLs that have not changed.
        _iconContentHashes.Clear();
        _iconPreviewBytes.Clear();
        using JsonDocument configuration = JsonDocument.Parse(achievementsJson);
        string[] urls = configuration.RootElement.GetProperty("achievements").EnumerateArray()
            .SelectMany(achievement => IconFields
                .Select(key => achievement.TryGetProperty(key, out JsonElement value) &&
                    value.ValueKind == JsonValueKind.String ? value.GetString() : null))
            .OfType<string>()
            .Where(static url => url.Length > 0)
            .ToArray();
        int previewBytes = 0;
        object previewLock = new();
        IReadOnlyDictionary<string, string> hashes = await SteamIconHashReader.ReadAsync(
            urls, DownloadIconAsync, (url, bytes) =>
            {
                if (bytes.Length is < 1 or > 256 * 1024) return;
                lock (previewLock)
                {
                    if (_iconPreviewBytes.Count >= 256) return;
                    if (previewBytes + bytes.Length > 8 * 1024 * 1024) return;
                    _iconPreviewBytes[url] = bytes;
                    previewBytes += bytes.Length;
                }
            }).ConfigureAwait(false);
        foreach ((string url, string hash) in hashes) _iconContentHashes[url] = hash;
    }
    public byte[]? CachedIconPreview(RemoteIconMetadata icon, bool unachieved)
    {
        Uri? uri = unachieved ? icon.UnachievedPreviewUri : icon.AchievedPreviewUri;
        string? identity = unachieved ? icon.UnachievedIdentity : icon.AchievedIdentity;
        if (uri is null || identity is null ||
            !_iconContentHashes.TryGetValue(uri.AbsoluteUri, out string? hash) ||
            hash != identity ||
            !_iconPreviewBytes.TryGetValue(uri.AbsoluteUri, out byte[]? bytes) ||
            bytes.Length is < 1 or > 256 * 1024) return null;
        return bytes.ToArray();
    }
    private async Task<byte[]> DownloadIconAsync(string url)
    {
        Uri uri = new(url);
        _requestPolicy.RequireIcon(uri);
        // ResponseHeadersRead ends HttpClient's timeout at the headers. Keep the
        // same deadline alive through the bounded, decompressed response stream.
        using CancellationTokenSource deadline = new(ChromeLaunchSettings.DefaultTimeout);
        using HttpRequestMessage request = new(HttpMethod.Get, uri);
        request.Headers.Add("Cache-Control", "no-cache"); request.Headers.Add("Pragma", "no-cache");
        using HttpResponseMessage response = await _iconDownloads!.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, deadline.Token).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode) throw new UnsupportedPortalStructureException("Steamworks icon content could not be verified.");
        return await BoundedSteamContent.ReadAsync(response.Content, 16 * 1024 * 1024, deadline.Token).ConfigureAwait(false);
    }

    private async Task<string> ReadConfigurationJsonAsync(SteamAppId appId, string endpoint)
    {
        SetSyncDiagnosticEndpoint(endpoint);
        Volatile.Write(ref _lastRequestFailure, null);
        Uri uri = _requestPolicy.ConfigurationUri(appId.Value, endpoint);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        // The response class is safe to retain even when the fixed browser read
        // rejects the body. Never record its URL, headers or contents.
        void ObserveResponse(object? _, IResponse response)
        {
            if (response.Url == uri.AbsoluteUri) SetSyncDiagnosticResponseStatus(response.Status);
        }
        page.Response += ObserveResponse;
        try
        {
            return await page.EvaluateAsync<string>(FixedReadScripts.Configuration, new { url = uri.AbsoluteUri, maximumBytes = 16 * 1024 * 1024,
                timeoutMs = (int)ChromeLaunchSettings.DefaultTimeout.TotalMilliseconds }).ConfigureAwait(false);
        }
        catch (PlaywrightException exception)
        {
            // Match only fixed literals from the bundled read script. The raw
            // exception may contain browser data and must never leave the worker.
            string reason = exception.Message switch
            {
                var message when message.Contains("AchimeldRead:status", StringComparison.Ordinal) => "read-status",
                var message when message.Contains("AchimeldRead:url", StringComparison.Ordinal) => "read-url",
                var message when message.Contains("AchimeldRead:body", StringComparison.Ordinal) => "read-body",
                var message when message.Contains("AchimeldRead:size", StringComparison.Ordinal) => "read-size",
                _ => "read-other",
            };
            SetSyncDiagnosticWriteState(reason);
            throw;
        }
        finally { page.Response -= ObserveResponse; }
    }

    public async Task CloseForUrgentReadCancellationAsync()
    {
        if (_page is { IsClosed: false } page)
        {
            _page = null;
            await page.CloseAsync(new PageCloseOptions { RunBeforeUnload = false }).ConfigureAwait(false);
        }
    }

    public async Task StopAsync()
    {
        await StopCoreAsync().ConfigureAwait(false);
    }

    public void RequireRequestPolicyHealthy() => _requestPolicy.RequireHealthy();

    public async ValueTask DisposeAsync()
    {
        await StopCoreAsync().ConfigureAwait(false);
    }

    private async Task<IPage> EnsurePageAsync()
    {
        if (_page is { IsClosed: false } current)
        {
            return current;
        }

        _page = null;
        _creatingOwnedPage = true;
        try
        {
            _page = await _context!.NewPageAsync().ConfigureAwait(false);
            return _page;
        }
        finally
        {
            _creatingOwnedPage = false;
        }
    }

    private void OnContextPage(object? sender, IPage page)
    {
        if (!_creatingOwnedPage && !ReferenceEquals(page, _page))
        {
            _unexpectedPages.Enqueue(page);
        }
    }

    private async void OnOwnedDownload(object? sender, IDownload download)
    {
        if (Volatile.Read(ref _armedExportDownload) != 0) return;
        _requestPolicy.Reject(BrowserRequestRejection.Other);
        try
        {
            await download.CancelAsync().ConfigureAwait(false);
            await download.DeleteAsync().ConfigureAwait(false);
        }
        catch (PlaywrightException) { }
        catch (IOException) { }
    }

    private async Task RejectUnexpectedPopupsAsync()
    {
        bool rejected = false;
        while (_unexpectedPages.TryDequeue(out IPage? popup))
        {
            rejected = true;
            if (!popup.IsClosed)
            {
                await popup.CloseAsync(new PageCloseOptions { RunBeforeUnload = false }).ConfigureAwait(false);
            }
        }

        if (rejected)
        {
            throw new PortalSafetyException(
                "portal.popup.rejected",
                "Steamworks opened an unexpected popup; automation stopped.");
        }
    }

    private async Task StopCoreAsync()
    {
        _started = false;
        _iconDownloads?.Dispose(); _iconDownloads = null;

        try
        {
            if (_context is not null)
            {
                _context.Page -= OnContextPage;
                if (_page is not null) _page.Download -= OnOwnedDownload;
                await BrowserShutdown.CloseAsync(_context).ConfigureAwait(false);
            }
        }
        finally
        {
            _context = null;

            _page = null;
            _playwright?.Dispose();
            _playwright = null;
            _profileLock?.Dispose();
            _profileLock = null;
        }
    }

    private async Task RequireHealthyAsync()
    {
        if (!_started || _context is null || _supervision is null)
            throw new InvalidOperationException("The supervised browser is not running.");
        await _supervision.RequireHealthyAsync(CancellationToken.None).ConfigureAwait(false);
        _requestPolicy.RequireHealthy();
    }
}

internal sealed class ProfileMutexLock : IDisposable
{
    private readonly ManualResetEventSlim _release = new(false);
    private readonly Thread _ownerThread;
    private readonly TaskCompletionSource<Exception?> _acquired =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private bool _disposed;

    private ProfileMutexLock(string mutexName)
    {
        _ownerThread = new Thread(() => OwnMutex(mutexName))
        {
            IsBackground = true,
            Name = "Achimeld Chrome profile lock",
        };
        _ownerThread.Start();
    }

    public static ProfileMutexLock Acquire(string profileDirectory, TimeSpan timeout)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(profileDirectory);
        string normalized = Path.GetFullPath(profileDirectory).ToUpperInvariant();
        string hash = Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(normalized)))[..24];
        ProfileMutexLock profileLock = new($"Local\\Achimeld.ChromeProfile.{hash}");

        if (!profileLock._acquired.Task.Wait(timeout))
        {
            profileLock.Dispose();
            throw new TimeoutException("Timed out while acquiring the managed Chrome profile lock.");
        }

        Exception? error = profileLock._acquired.Task.GetAwaiter().GetResult();
        if (error is not null)
        {
            profileLock.Dispose();
            throw error;
        }

        return profileLock;
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _release.Set();
        if (Thread.CurrentThread != _ownerThread)
        {
            _ownerThread.Join();
        }

        _release.Dispose();
    }

    private void OwnMutex(string mutexName)
    {
        try
        {
            using Mutex mutex = new(initiallyOwned: false, mutexName);
            bool ownsMutex;
            try
            {
                ownsMutex = mutex.WaitOne(TimeSpan.Zero);
            }
            catch (AbandonedMutexException)
            {
                ownsMutex = true;
            }

            if (!ownsMutex)
            {
                _acquired.TrySetResult(
                    new InvalidOperationException("The managed Chrome profile is already in use."));
                return;
            }

            _acquired.TrySetResult(null);
            _release.Wait();
            mutex.ReleaseMutex();
        }
        catch (Exception exception)
        {
            _acquired.TrySetResult(exception);
        }
    }
}
