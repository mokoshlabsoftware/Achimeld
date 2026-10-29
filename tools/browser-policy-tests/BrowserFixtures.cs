using System.Security.Cryptography;
using System.Text.Json;
using System.Net;
using System.Net.Sockets;
using Achimeld.Browser;
using Achimeld.Security;
using Microsoft.Playwright;

internal static class BrowserFixtures
{
    internal static async Task RunAsync(string[] arguments)
    {
        if (!OperatingSystem.IsWindows() || arguments.Length != 2 || arguments[0] != "--browser-fixtures")
            throw new ArgumentException("Use --browser-fixtures with a freshly provisioned audit worker directory.");
        string repository = FindRepository();
        string package = Path.GetFullPath(arguments[1]);
        string artifacts = Path.GetFullPath(Environment.GetEnvironmentVariable("ACHIMELD_ARTIFACT_ROOT") ?? throw new InvalidDataException("Use build/Test-BrowserRequests.ps1."));
        if (!package.StartsWith(artifacts + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Browser fixture package boundary rejected.");
        DedicatedProfilePolicy.RejectReparsePoints(package);
        using JsonDocument evidence = JsonDocument.Parse(await File.ReadAllBytesAsync(Path.Combine(package, "browser-provenance.json")));
        using JsonDocument runtime = JsonDocument.Parse(await File.ReadAllBytesAsync(Path.Combine(repository, "build", "browser-runtime.lock.json")));
        if (Hash(Path.Combine(repository, "build", "browser-runtime.lock.json")) != evidence.RootElement.GetProperty("runtimeLockSha256").GetString())
            throw new InvalidDataException("Browser fixture lock mismatch.");
        foreach (JsonElement file in evidence.RootElement.GetProperty("files").EnumerateArray())
        {
            string path = Path.GetFullPath(Path.Combine(package, file.GetProperty("path").GetString()!));
            if (!path.StartsWith(package + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) || Hash(path) != file.GetProperty("sha256").GetString())
                throw new InvalidDataException("Browser fixture runtime mismatch.");
            DedicatedProfilePolicy.RejectReparsePoints(path);
        }
        string browserPath = Path.Combine(package, runtime.RootElement.GetProperty("executable").GetString()!);
        string nodePath = Path.Combine(package, ".playwright", "node", "win32_x64", "node.exe");
        using JsonDocument worker = JsonDocument.Parse(await File.ReadAllBytesAsync(Path.Combine(package, "read-worker-provenance.json")));
        JsonElement node = worker.RootElement.GetProperty("inputs").EnumerateArray().Single(file => file.GetProperty("path").GetString() == ".playwright/node/win32_x64/node.exe");
        if (Hash(nodePath) != node.GetProperty("sha256").GetString()) throw new InvalidDataException("Browser fixture Node digest mismatch.");
        DedicatedProfilePolicy.RejectReparsePoints(nodePath);
        // Fixture-only launch: fixed digest-verified vendor runtimes, inherited Job,
        // synthetic pages and a rejecting loopback proxy. No production profile/session.
        WorkerLifetime lifetime = new();
        using CancellationTokenSource proxyStop = new();
        TcpListener proxy = new(IPAddress.Loopback, 0);
        proxy.Start();
        int proxyConnections = 0;
        Task proxyTask = Task.Run(async () =>
        {
            try
            {
                while (!proxyStop.IsCancellationRequested)
                {
                    using TcpClient peer = await proxy.AcceptTcpClientAsync(proxyStop.Token);
                    Interlocked.Increment(ref proxyConnections);
                    await peer.GetStream().WriteAsync("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"u8.ToArray(), proxyStop.Token);
                }
            }
            catch (OperationCanceledException) when (proxyStop.IsCancellationRequested) { }
            catch (SocketException) when (proxyStop.IsCancellationRequested) { }
        });
        try
        {
            Environment.SetEnvironmentVariable("PLAYWRIGHT_NODEJS_PATH", nodePath);
            foreach (string name in new[] { "PLAYWRIGHT_DRIVER_SEARCH_PATH", "NODE_OPTIONS", "NODE_PATH", "NODE_EXTRA_CA_CERTS", "NODE_TLS_REJECT_UNAUTHORIZED" }) Environment.SetEnvironmentVariable(name, null);
            using IPlaywright playwright = await Playwright.CreateAsync();
            string startupProfile = Path.Combine(artifacts, "browser-startup-" + Guid.NewGuid().ToString("N"));
            await using (IBrowserContext persistent = await playwright.Chromium.LaunchPersistentContextAsync(startupProfile,
                new BrowserTypeLaunchPersistentContextOptions {
                    ExecutablePath = browserPath, Headless = true, ChromiumSandbox = true,
                    Offline = true, ServiceWorkers = ServiceWorkerPolicy.Block, AcceptDownloads = false,
                    Proxy = new Proxy { Server = $"http://127.0.0.1:{((IPEndPoint)proxy.LocalEndpoint).Port}" },
                    Args = BrowserStartup.LaunchArguments,
                    Timeout = 15000,
                }))
            {
                if (persistent.Pages.Count == 0) throw new Exception("Persistent browser fixture must begin with a restored page.");
                await persistent.NewPageAsync();
                SteamBrowserRequestPolicy startupPolicy = new();
                IPage? owned = null;
                owned = await BrowserStartup.CreateOwnedPageAsync(persistent, startupPolicy, () => owned).WaitAsync(TimeSpan.FromSeconds(15));
                if (owned.IsClosed || owned.Url != "about:blank" || persistent.Pages.Count != 1 || await owned.EvaluateAsync<int>("() => 42") != 42)
                    throw new Exception("Persistent browser did not retain exactly one live owned page.");
                startupPolicy.RequireHealthy();
                Console.WriteLine("PASS: persistent browser replaces restored pages while remaining offline and alive.");
                startupPolicy.BeginAuthentication();
                int beforeConnect = Volatile.Read(ref proxyConnections);
                await persistent.SetOfflineAsync(false);
                try { await owned.GotoAsync("https://partner.steamgames.com/", new PageGotoOptions { Timeout = 10000 }); }
                catch (PlaywrightException) { }
                startupPolicy.RequireHealthy();
                if (Volatile.Read(ref proxyConnections) <= beforeConnect)
                    throw new Exception("Dedicated browser could not reach the rejecting loopback broker with the production launch recipe.");
                Console.WriteLine("PASS: dedicated browser reaches the rejecting loopback broker; no upstream connection is permitted.");
            }
            await using IBrowser browser = await playwright.Chromium.LaunchAsync(new BrowserTypeLaunchOptions
            {
                ExecutablePath = browserPath, Headless = true, ChromiumSandbox = true,
                Proxy = new Proxy { Server = $"http://127.0.0.1:{((IPEndPoint)proxy.LocalEndpoint).Port}" },
                Args = BrowserStartup.LaunchArguments,
            });
            int reached = 0;
            async Task<(IBrowserContext Context, IPage Page, SteamBrowserRequestPolicy Policy)> Session(string? configuration = null, bool redirectConfiguration = false, bool authenticate = false, bool cspReport = false)
            {
                IBrowserContext context = await browser.NewContextAsync(new BrowserNewContextOptions { Offline = true, ServiceWorkers = ServiceWorkerPolicy.Block, AcceptDownloads = false });
                context.SetDefaultTimeout(10000);
                await context.RouteAsync("**/*", async route =>
                {
                    reached++;
                    if (route.Request.Url == "https://partner.steamgames.com/apps/fetchstats/480" && redirectConfiguration)
                    {
                        await route.FulfillAsync(new RouteFulfillOptions { Status = 302, Headers = new Dictionary<string, string> { ["Location"] = "/apps/fetchstats/481" }, Body = "" });
                        return;
                    }
                    if (route.Request.Url == "https://partner.steamgames.com/apps/fetchstats/480" && configuration is not null)
                    {
                        await route.FulfillAsync(new RouteFulfillOptions { Status = 200, ContentType = "application/json", Body = configuration });
                        return;
                    }
                    await route.FulfillAsync(new RouteFulfillOptions { Status = 200, ContentType = "text/html", Body = "<html><body>synthetic Steamworks</body></html>",
                        Headers = cspReport ? new Dictionary<string, string> { ["Content-Security-Policy"] = "img-src 'none'; report-uri /actions/CSPReport" } : null });
                });
                IPage page = await context.NewPageAsync();
                SteamBrowserRequestPolicy policy = new();
                await BrowserRequestGuard.InstallAsync(context, policy, () => page);
                policy.PinApp(480);
                if (!authenticate) policy.EnterRead();
                await context.SetOfflineAsync(false);
                await page.GotoAsync("https://partner.steamgames.com/apps/achievements/480");
                return (context, page, policy);
            }
            async Task Denied(Func<IPage, Task> action)
            {
                var session = await Session();
                await using IBrowserContext context = session.Context;
                int before = reached;
                try { await action(session.Page).WaitAsync(TimeSpan.FromSeconds(15)); } catch (PlaywrightException) { }
                if (reached != before) throw new Exception("Forbidden browser request reached the synthetic upstream.");
                try { session.Policy.RequireHealthy(); } catch (InvalidDataException) { return; }
                throw new Exception("Forbidden browser traffic did not terminate policy authority.");
            }
            // Steam's public apps.js deletion closures put record IDs in the
            // URL, with an empty request body. Exercise the actual routing guard.
            foreach (PortalEntityKind kind in new[] { PortalEntityKind.Achievement, PortalEntityKind.Stat })
            {
                var deletion = await Session();
                await using IBrowserContext context = deletion.Context;
                deletion.Policy.BeginReviewedDraft(480, kind);
                string identity = kind == PortalEntityKind.Stat ? "9" : "9_0";
                Uri uri = deletion.Policy.ArmReviewedDeletion(480, identity);
                int before = reached;
                await deletion.Page.EvaluateAsync("url => fetch(url, {method:'POST', body:''})", uri.AbsoluteUri);
                deletion.Policy.RequireHealthy();
                if (reached != before + 1) throw new Exception("Reviewed record deletion did not reach synthetic upstream.");
                await deletion.Page.EvaluateAsync("url => fetch(url, {method:'POST'}).catch(() => {})", uri.AbsoluteUri);
                if (reached != before + 1 || deletion.Policy.Rejection != BrowserRequestRejection.Dynamic)
                    throw new Exception("Repeated deletion regained write authority.");
            }
            Console.WriteLine("PASS: record-specific achievement and stat deletions route once and reject replay.");
            var login = await Session(authenticate: true);
            await using (IBrowserContext context = login.Context)
            {
                await login.Page.GotoAsync("https://partner.steamgames.com/");
                await login.Page.SetContentAsync("<button id='login_btn_signin'>Sign in</button><script>window.g_ShowLoginDialog = destination => { window.loginDestination = destination; };</script>");
                await SteamLoginPrompt.OpenIfNeededAsync(login.Page, new Uri("https://partner.steamgames.com/apps/achievements/480"));
                if (await login.Page.EvaluateAsync<string>("() => window.loginDestination") != "https://partner.steamgames.com/apps/achievements/480")
                    throw new Exception("Login prompt did not retain the pinned game return destination.");
                bool invalidDestination = false;
                try { await SteamLoginPrompt.OpenIfNeededAsync(login.Page, new Uri("https://partner.steamgames.com/dashboard")); }
                catch (InvalidDataException) { invalidDestination = true; }
                if (!invalidDestination) throw new Exception("Login prompt accepted an unrelated return destination.");
                await login.Page.GotoAsync("https://partner.steamgames.com/apps/achievements/480");
                await login.Page.SetContentAsync("<button id='login_btn_signin'>Sign in</button><script>window.g_ShowLoginDialog = () => { throw new Error('must not open'); };</script>");
                await SteamLoginPrompt.OpenIfNeededAsync(login.Page, new Uri("https://partner.steamgames.com/apps/achievements/480"));
                int before = reached;
                await login.Page.EvaluateAsync("() => fetch('https://api.steampowered.com/IAuthenticationService/BeginAuthSessionViaQR/v1', {method:'POST'}).catch(() => {})");
                if (reached != before + 1) throw new Exception("Login RPC did not reach synthetic upstream.");
                await login.Page.EvaluateAsync("() => fetch('/um/Authentication/BeginAuthSessionViaQR/', {method:'POST', headers:{'Content-Type':'application/octet-stream'}, body:new Uint8Array([0])}).catch(() => {})");
                if (reached != before + 2) throw new Exception("Steamworks browser login RPC did not reach synthetic upstream.");
                login.Policy.RequireHealthy();
                login.Policy.EnterRead();
                before = reached;
                await login.Page.EvaluateAsync("() => fetch('/um/Authentication/BeginAuthSessionViaQR/', {method:'POST'}).catch(() => {})");
                if (reached != before || login.Policy.Rejection != BrowserRequestRejection.Dynamic) throw new Exception("Read phase regained login RPC authority.");
            }
            var transition = await Session(authenticate: true);
            await using (IBrowserContext context = transition.Context)
            {
                for (int attempt = 0; attempt < 12; attempt++)
                {
                    Task navigation = transition.Page.GotoAsync(attempt % 2 == 0
                        ? "https://partner.steamgames.com/" : "https://partner.steamgames.com/apps/achievements/480");
                    await SteamAuthenticationProbe.ReadAsync(transition.Page);
                    await navigation;
                }
                await transition.Page.SetContentAsync("<input type='password' value='fixture-secret'>");
                if (await SteamAuthenticationProbe.ReadAsync(transition.Page) != PortalAccessState.SignedOut)
                    throw new Exception("Login probe lost its signed-out indicator.");
                transition.Policy.RequireHealthy();
                await transition.Page.CloseAsync();
                bool closedRejected = false;
                try { await SteamAuthenticationProbe.ReadAsync(transition.Page); }
                catch (PlaywrightException) { closedRejected = true; }
                if (!closedRejected) throw new Exception("Closed browser was treated as a navigation transition.");
                Console.WriteLine("PASS: authentication polling survives document transitions and still rejects a closed browser.");
            }
            if (!SteamAuthenticationProbe.IsNavigationTransition(new PlaywrightException("Execution context was destroyed, most likely because of a navigation.")) ||
                SteamAuthenticationProbe.IsNavigationTransition(new PlaywrightException("Target page, context or browser has been closed")) ||
                SteamAuthenticationProbe.IsNavigationTransition(new PlaywrightException("net::ERR_CONNECTION_RESET")))
                throw new Exception("Authentication retry escaped its document-transition boundary.");
            var reports = await Session(cspReport: true);
            await using (IBrowserContext context = reports.Context)
            {
                TaskCompletionSource<bool> reported = new(TaskCreationOptions.RunContinuationsAsynchronously);
                int reportRequests = 0, reportFailures = 0;
                bool IsReport(IRequest request) => request.Url == "https://partner.steamgames.com/actions/CSPReport";
                context.Request += (_, request) => { if (IsReport(request)) Interlocked.Increment(ref reportRequests); };
                context.RequestFailed += (_, request) => { if (IsReport(request)) Interlocked.Increment(ref reportFailures); };
                context.RequestFinished += (_, request) => { if (IsReport(request)) reported.TrySetResult(true); };
                int before = reached;
                await reports.Page.EvaluateAsync("() => { const image = new Image(); image.src = '/public/images/csp-blocked.png'; document.body.append(image); }");
                try { await reported.Task.WaitAsync(TimeSpan.FromSeconds(15)); }
                catch (TimeoutException)
                {
                    throw new Exception($"CSP fixture report did not finish: requests={reportRequests}; failed={reportFailures}; rejection={reports.Policy.Rejection}; details={reports.Policy.RejectionDetails}; upstream delta={reached - before}.");
                }
                reports.Policy.RequireHealthy();
                if (reached != before) throw new Exception("CSP-blocked resource or its report reached the synthetic upstream.");
                Console.WriteLine("PASS: Chromium enforces CSP; its optional report is completed locally without terminating the session.");
            }
            await Denied(page => page.EvaluateAsync("() => fetch('/actions/CSPReport', {method:'POST',headers:{'Content-Type':'application/csp-report'},body:'fixture'}).catch(() => {})"));
            var allowed = await Session();
            await using (IBrowserContext context = allowed.Context)
            {
                await allowed.Page.EvaluateAsync("() => fetch('/apps/fetchstats/480').then(r => r.text())");
                await allowed.Page.EvaluateAsync("() => fetch('/apps/getachievementcounts/480').then(r => r.text())");
                allowed.Policy.RequireHealthy();
            }
            var bounded = await Session("{\"ok\":true}");
            var media = await Session();
            await using (IBrowserContext context = media.Context)
            {
                foreach (string url in new[] { "https://cdn.steamstatic.com/public/audio/notification.mp3?v=1", "https://partner.steamgames.com/public/video/intro.webm" })
                {
                    int beforeMedia = reached;
                    // The synthetic body is intentionally not playable: verify routing, not decoding.
                    await media.Page.EvaluateAsync("url => new Promise(resolve => { const item = document.createElement('video'); item.preload = 'auto'; item.onerror = () => resolve(true); item.onloadeddata = () => resolve(true); item.src = url; document.body.append(item); item.load(); })", url).WaitAsync(TimeSpan.FromSeconds(15));
                    if (reached <= beforeMedia) throw new Exception("Approved static media did not reach synthetic upstream.");
                    media.Policy.RequireHealthy();
                }
            }
            await Denied(page => page.EvaluateAsync("() => new Promise(resolve => { const item = new Audio('https://unapproved.invalid/audio.mp3'); item.onerror = () => resolve(true); item.load(); })"));
            await using (IBrowserContext context = bounded.Context)
            {
                string json = await bounded.Page.EvaluateAsync<string>(FixedReadScripts.Configuration,
                    new { url = "https://partner.steamgames.com/apps/fetchstats/480", maximumBytes = 128, timeoutMs = 10000 });
                if (json != "{\"ok\":true}") throw new Exception("Fixed configuration streaming changed response bytes.");
                bounded.Policy.RequireHealthy();
            }
            var excessive = await Session(new string('x', 256));
            await using (IBrowserContext context = excessive.Context)
            {
                bool refused = false;
                try { await excessive.Page.EvaluateAsync<string>(FixedReadScripts.Configuration, new { url = "https://partner.steamgames.com/apps/fetchstats/480", maximumBytes = 16, timeoutMs = 10000 }); }
                catch (PlaywrightException) { refused = true; }
                if (!refused) throw new Exception("Oversized browser configuration was accepted.");
                excessive.Policy.RequireHealthy();
            }
            var redirects = await Session(redirectConfiguration: true);
            await using (IBrowserContext context = redirects.Context)
            {
                int beforeRedirect = reached;
                bool refused = false;
                try { await redirects.Page.EvaluateAsync<string>(FixedReadScripts.Configuration, new { url = "https://partner.steamgames.com/apps/fetchstats/480", maximumBytes = 128, timeoutMs = 10000 }); }
                catch (PlaywrightException) { refused = true; }
                if (!refused || reached != beforeRedirect + 1) throw new Exception("Configuration redirect was followed.");
                redirects.Policy.RequireHealthy();
            }
            await Denied(page => page.EvaluateAsync("() => fetch('/apps/setachievement/480', {method:'POST', body:'synthetic'}).catch(() => {})"));
            await Denied(page => page.EvaluateAsync("() => fetch('/apps/deleteachievement/480').catch(() => {})"));
            await Denied(page => page.EvaluateAsync("() => fetch('/apps/fetchstats/481').catch(() => {})"));
            await Denied(page => page.EvaluateAsync("() => fetch('/apps/getachievementcounts/481').catch(() => {})"));
            await Denied(page => page.EvaluateAsync("() => fetch('/apps/getachievementcounts/480', {method:'POST'}).catch(() => {})"));
            await Denied(page => page.GotoAsync("https://store.steampowered.com/login/"));
            await Denied(async page =>
            {
                Task<IPage> popup = page.Context.WaitForPageAsync();
                await page.EvaluateAsync("() => { window.open('https://partner.steamgames.com/apps/achievements/480'); }");
                IPage opened = await popup;
                try { await opened.WaitForLoadStateAsync(); } catch (PlaywrightException) { }
            });
            var workers = await Session();
            await using (IBrowserContext context = workers.Context)
            {
                int beforeWorker = reached;
                // The block policy does not promise a rejected registration task.
                await workers.Page.EvaluateAsync("() => { navigator.serviceWorker.register('/public/worker.js').catch(() => {}); }").WaitAsync(TimeSpan.FromSeconds(15));
                await Task.Delay(250);
                int registrations = await workers.Page.EvaluateAsync<int>("() => navigator.serviceWorker.getRegistrations().then(items => items.length)").WaitAsync(TimeSpan.FromSeconds(15));
                workers.Policy.RequireHealthy();
                if (registrations != 0 || reached != beforeWorker) throw new Exception("Service worker registration reached upstream.");
            }
            // Match production's terminal Job exit for socket policy loss. The
            // pinned SDK throws on graceful teardown of intercepted WebSockets;
            // that vendor behavior is recorded separately from denial evidence.
            var sockets = await Session();
            int beforeSocket = reached;
            await sockets.Page.EvaluateAsync("() => { window.fixtureSocket = new WebSocket('wss://partner.steamgames.com/socket'); }").WaitAsync(TimeSpan.FromSeconds(15));
            bool rejected = false;
            for (int attempt = 0; attempt < 500 && !rejected; attempt++)
            {
                try { sockets.Policy.RequireHealthy(); } catch (InvalidDataException) { rejected = true; }
                if (!rejected) await Task.Delay(10);
            }
            if (!rejected || reached != beforeSocket) throw new Exception("WebSocket policy did not deny server authority.");
            Console.WriteLine("PASS: synthetic Chromium requests, writes, App ID changes, login redirects, popup first requests, WebSockets and service-worker denial; no live Steam traffic.");
            GC.KeepAlive(lifetime);
            Environment.Exit(0);
        }
        finally
        {
            await proxyStop.CancelAsync();
            proxy.Stop();
            await proxyTask;
            // The OS closes this process-lifetime Job on fixture exit. Disposing a
            // Job containing this process would kill the test before its exit code.
            GC.KeepAlive(lifetime);
        }
    }

    private static string Hash(string path) { using var input = File.OpenRead(path); return Convert.ToHexStringLower(SHA256.HashData(input)); }
    private static string FindRepository()
    {
        string? source = Environment.GetEnvironmentVariable("ACHIMELD_SOURCE_ROOT");
        if (source is not null && File.Exists(Path.Combine(source, "build", "browser-runtime.lock.json")))
            return Path.GetFullPath(source);
        throw new InvalidDataException("Browser fixture repository unavailable.");
    }
}
