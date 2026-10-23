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
            async Task<(IBrowserContext Context, IPage Page, SteamBrowserRequestPolicy Policy)> Session(string? configuration = null, bool redirectConfiguration = false)
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
                    await route.FulfillAsync(new RouteFulfillOptions { Status = 200, ContentType = "text/html", Body = "<html><body>synthetic Steamworks</body></html>" });
                });
                IPage page = await context.NewPageAsync();
                SteamBrowserRequestPolicy policy = new();
                await BrowserRequestGuard.InstallAsync(context, policy, () => page);
                policy.PinApp(480);
                policy.EnterRead();
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
            var allowed = await Session();
            await using (IBrowserContext context = allowed.Context)
            {
                await allowed.Page.EvaluateAsync("() => fetch('/apps/fetchstats/480').then(r => r.text())");
                await allowed.Page.EvaluateAsync("() => fetch('/apps/getachievementcounts/480').then(r => r.text())");
                allowed.Policy.RequireHealthy();
            }
            var bounded = await Session("{\"ok\":true}");
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
