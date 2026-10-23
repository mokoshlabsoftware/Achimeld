using System.Collections.Concurrent;
using System.Text.Json;
using System.Security.Cryptography;
using System.Text;
using System.Net;
using Achimeld.Core;
using Microsoft.Playwright;

namespace Achimeld.Browser;

public sealed partial class PlaywrightChromeDriver : IAsyncDisposable
{
    private static readonly string[] IconFields = ["icon", "icon_gray"];
    private readonly SteamworksPageClassifier _classifier = new();
    private readonly ConcurrentQueue<IPage> _unexpectedPages = new();
    private readonly Dictionary<string, string> _iconContentHashes = new(StringComparer.Ordinal);
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
                    AcceptDownloads = false,
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

            // Never enable networking for restored pages from the persistent profile.
            _page = await BrowserStartup.CreateOwnedPageAsync(_context, _requestPolicy, () => _page).ConfigureAwait(false);
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
        // Inspect only authentication indicators, never password values or login HTML.
        string indicators = await page.EvaluateAsync<string>("""
            () => {
                if (document.querySelector('input[type="password"], #login_form')) return 'login_form';
                return (document.body?.innerText || '').slice(0, 4096);
            }
            """).ConfigureAwait(false);
        return _classifier.ClassifyAuthentication(new ManagedPageDocument(new Uri(page.Url), "", indicators));
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
        if (uri.AbsoluteUri != $"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{appId.Value}" && !_requestPolicy.AllowsDraftNavigation(uri))
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
        DiagnosticStage = "navigation.complete";
    }

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
        await RequireHealthyAsync().ConfigureAwait(false);
        SteamworksUrlGuard.EnsureAllowed(new Uri((await EnsurePageAsync().ConfigureAwait(false)).Url), appId);
        if (await ReadAuthenticationStateAsync().ConfigureAwait(false) != PortalAccessState.Authenticated)
            throw new PortalSafetyException("portal.authentication.required", "Steamworks authentication is required.");
        _requestPolicy.PinApp(appId.Value);
        _requestPolicy.EnterRead();
        string stats = await ReadConfigurationJsonAsync(appId, "fetchstats").ConfigureAwait(false);
        string achievements = await ReadConfigurationJsonAsync(appId, "fetchachievements").ConfigureAwait(false);
        await ReadIconContentHashesAsync(achievements).ConfigureAwait(false);
        return LiveSteamworksConfiguration.Normalize(appId, stats, achievements, _iconContentHashes);
    }

    private async Task ReadIconContentHashesAsync(string achievementsJson)
    {
        // Re-read content on every snapshot, including URLs that have not changed.
        _iconContentHashes.Clear();
        using JsonDocument configuration = JsonDocument.Parse(achievementsJson);
        string[] urls = configuration.RootElement.GetProperty("achievements").EnumerateArray()
            .SelectMany(achievement => IconFields
                .Select(key => achievement.TryGetProperty(key, out JsonElement value) &&
                    value.ValueKind == JsonValueKind.String ? value.GetString() : null))
            .OfType<string>()
            .Where(static url => url.Length > 0)
            .ToArray();
        IReadOnlyDictionary<string, string> hashes = await SteamIconHashReader.ReadAsync(
            urls, DownloadIconAsync).ConfigureAwait(false);
        foreach ((string url, string hash) in hashes) _iconContentHashes[url] = hash;
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
        Uri uri = _requestPolicy.ConfigurationUri(appId.Value, endpoint);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        return await page.EvaluateAsync<string>(FixedReadScripts.Configuration, new { url = uri.AbsoluteUri, maximumBytes = 16 * 1024 * 1024,
            timeoutMs = (int)ChromeLaunchSettings.DefaultTimeout.TotalMilliseconds }).ConfigureAwait(false);
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
                await _context.CloseAsync().ConfigureAwait(false);
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
