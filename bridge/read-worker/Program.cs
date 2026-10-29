using System.Diagnostics;
using System.IO.Pipes;
using System.Net;
using System.Runtime.InteropServices;
using System.Text.Json;
using Achimeld.Browser;
using Achimeld.Core;
using Achimeld.Security;
using Achimeld.ReadWorker;

// Only the normal authenticated supervised entry point exists in the deployed assembly.
if (!OperatingSystem.IsWindows() || args.Length != 1 ||
    !args[0].StartsWith("Achimeld.Steam.", StringComparison.Ordinal) ||
    args[0].Length != "Achimeld.Steam.".Length + 32 || !args[0]["Achimeld.Steam.".Length..].All(char.IsAsciiHexDigit)) return 1;
try
{
    SupervisionSession supervision = await SupervisionSession.ConnectAsync(CancellationToken.None);
    if (supervision.Status.NetworkEnforced && !await NetworkSelfTest.DirectAccessDeniedAsync(supervision))
    {
        await supervision.ReportPolicyFailureAsync(CancellationToken.None);
        return 125;
    }
    using WorkerLifetime lifetime = new();
    await using var pipe = new NamedPipeClientStream(".", args[0], PipeDirection.InOut,
        PipeOptions.Asynchronous | PipeOptions.CurrentUserOnly);
    await pipe.ConnectAsync(15000);
    if (!Native.GetNamedPipeServerProcessId(pipe.SafePipeHandle.DangerousGetHandle(), out uint serverPid)) return 1;
    using Process editor = Process.GetProcessById(checked((int)serverPid));
    if (!string.Equals(editor.MainModule?.FileName, Path.Combine(supervision.PackageRoot, "Achimeld.App.exe"), StringComparison.OrdinalIgnoreCase)) return 1;
    await using PlaywrightChromeDriver driver = new();
    using var shutdown = new CancellationTokenSource();
    Task health = MonitorAsync(supervision, driver, shutdown);
    HashSet<Guid> requests = [];
    ReadOnlyPolicy policy = new();
    SynchronizationSession synchronization = new(new DriverSynchronizationPortal(driver));
    SteamDeletionSession deletion = new(new DriverSynchronizationPortal(driver));
    bool failed = false;
    try
    {
        while (!shutdown.IsCancellationRequested)
        {
            SteamWorkerRequest request = await SecurityWire.ReadAsync<SteamWorkerRequest>(pipe, shutdown.Token);
            if (request.RequestId == Guid.Empty || request.Operation is null || request.Operation.Length > 32 ||
                requests.Count >= 9000 || !requests.Add(request.RequestId)) throw new InvalidDataException("Invalid or replayed worker request.");
            SteamWorkerResponse response;
            Stopwatch operationTimer = Stopwatch.StartNew();
            driver.BeginDiagnosticOperation();
            try
            {
                // The authenticated supervisor supplies mode. Caller environment cannot enable probes.
                await supervision.RequireHealthyAsync(shutdown.Token);
                object data;
                if (request.Operation == "deleteSteam")
                {
                    data = await deletion.RunAsync(request.Arguments.Deserialize<SteamDeletionRequest>(SecurityWire.Json) ?? throw new InvalidDataException(), policy.RequireSynchronizationApp());
                }
                else if (request.Operation is "prepareSync" or "beginSync" or "syncStep" or "finishSync")
                {
                    uint appId = policy.RequireSynchronizationApp();
                    data = request.Operation switch
                    {
                        "prepareSync" => await synchronization.PrepareAsync(request.Arguments.Deserialize<PrepareSyncRequest>(SecurityWire.Json) ?? throw new InvalidDataException(), appId),
                        "beginSync" => await synchronization.BeginAsync(request.Arguments.Deserialize<BeginSyncRequest>(SecurityWire.Json) ?? throw new InvalidDataException()),
                        "syncStep" => await synchronization.StepAsync(request.Arguments.Deserialize<SyncStepRequest>(SecurityWire.Json) ?? throw new InvalidDataException()),
                        "finishSync" => await synchronization.FinishAsync(request.Arguments.Deserialize<FinishSyncRequest>(SecurityWire.Json) ?? throw new InvalidDataException()),
                        _ => throw new InvalidDataException(),
                    };
                }
                else
                {
                    ReadAction action = policy.Validate(request.Operation, request.Arguments,
                        supervision.Status.PackageMode == "development" && supervision.Status.NetworkEnforced);
                    data = await ExecuteAsync(action, driver, supervision, shutdown.Token);
                }
                driver.RequireRequestPolicyHealthy();
                response = new(request.RequestId, true, JsonSerializer.SerializeToElement(data, SecurityWire.Json));
            }
            catch (Exception exception) when (exception is not OutOfMemoryException)
            {
                string code = WorkerFailure.Code(request.Operation, exception, driver.RequestRejection);
                response = new(request.RequestId, false, WorkerFailure.Details(
                    request.Operation == "navigate" ? driver.DiagnosticStage : "operation",
                    exception, operationTimer.ElapsedMilliseconds, driver.RequestRejectionDetails), code);
            }
            await SecurityWire.WriteAsync(pipe, response, shutdown.Token);
            if (!response.Success)
            {
                failed = true;
                // The host checks child liveness before reading the pipe. Keep
                // this failed, non-dispatching worker alive briefly so a fully
                // written failure frame is consumed before process teardown.
                await Task.Delay(TimeSpan.FromSeconds(1), shutdown.Token);
                break;
            }
        }
    }
    finally { await shutdown.CancelAsync(); await health; }
    return failed ? 125 : 0;
}
catch (Exception exception) when (exception is not OutOfMemoryException)
{
    Console.Error.WriteLine("Achimeld read worker stopped: security session or transport unavailable.");
    return 1;
}

static async Task MonitorAsync(SupervisionSession session, PlaywrightChromeDriver driver, CancellationTokenSource shutdown)
{
    try
    {
        while (!shutdown.IsCancellationRequested)
        {
            await Task.Delay(TimeSpan.FromSeconds(1), shutdown.Token);
            await session.RequireHealthyAsync(shutdown.Token);
            driver.RequireRequestPolicyHealthy();
            if (session.Status.NetworkEnforced && !await NetworkSelfTest.DirectAccessDeniedAsync(session))
            {
                await session.ReportPolicyFailureAsync(shutdown.Token);
                throw new InvalidOperationException("Mandatory network probe failed.");
            }
        }
    }
    catch (OperationCanceledException) when (shutdown.IsCancellationRequested) { }
    catch (Exception exception) when (exception is not OutOfMemoryException)
    {
        // Requests are already denied by the latched policy. Give the host's
        // next authentication poll a bounded opportunity to receive the safe
        // failure details before the watchdog exits the process.
        if (driver.RequestRejection != BrowserRequestRejection.None)
        {
            try { await Task.Delay(TimeSpan.FromSeconds(3), shutdown.Token); }
            catch (OperationCanceledException) when (shutdown.IsCancellationRequested) { return; }
        }
        await shutdown.CancelAsync();
        Environment.Exit(WorkerFailure.ExitCode(driver.RequestRejection));
    }
}

static async Task<object> ExecuteAsync(ReadAction action, PlaywrightChromeDriver driver,
    SupervisionSession session, CancellationToken cancellationToken)
{
    switch (action.Operation)
    {
        case ReadOperation.Start:
            await driver.StartAsync(session);
            return true;
        case ReadOperation.Authenticate: await driver.NavigateForAuthenticationAsync(); return true;
        case ReadOperation.AuthenticationState: return await driver.ReadAuthenticationStateAsync();
        case ReadOperation.Navigate: await driver.NavigateAsync(new Uri(action.Route)); return true;
        case ReadOperation.ReadConfiguration:
            return PortalDocumentSanitizer.ForTransport(await driver.ReadCompleteConfigurationAsync(new SteamAppId(action.AppId)));
        case ReadOperation.Stop: await driver.StopAsync(); return true;
        case ReadOperation.NetworkSelfTest: return await NetworkProbeAsync(action.Hold, driver, session, cancellationToken);
        default: throw new InvalidDataException("Read session command rejected.");
    }
}

static async Task<object> NetworkProbeAsync(bool hold, PlaywrightChromeDriver driver,
    SupervisionSession session, CancellationToken cancellationToken)
{
    if (!await NetworkSelfTest.DirectAccessDeniedAsync(session)) return new DevelopmentNetworkTest(false, "direct-network-denial");
    using (var client = new System.Net.Sockets.TcpClient())
    {
        await client.ConnectAsync(IPAddress.Loopback, session.Status.ProxyPort, cancellationToken);
        await using var stream = client.GetStream();
        byte[] forbidden = System.Text.Encoding.ASCII.GetBytes("CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n");
        await stream.WriteAsync(forbidden, cancellationToken);
        byte[] result = new byte[256];
        int length = await stream.ReadAsync(result, cancellationToken);
        if (!System.Text.Encoding.ASCII.GetString(result, 0, length).StartsWith("HTTP/1.1 403 Forbidden", StringComparison.Ordinal))
            return new DevelopmentNetworkTest(false, "forbidden-broker-host");
    }
    using var downloads = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false, UseCookies = false,
        UseProxy = true, Proxy = new WebProxy($"http://127.0.0.1:{session.Status.ProxyPort}") { BypassProxyOnLocal = false } })
        { Timeout = TimeSpan.FromMinutes(2) };
    using (HttpResponseMessage response = await downloads.GetAsync("https://partner.steamgames.com/", HttpCompletionOption.ResponseHeadersRead, cancellationToken))
        if ((int)response.StatusCode is < 200 or >= 400) return new DevelopmentNetworkTest(false, "anonymous-Steam-http");
    await driver.StartAsync(session);
    await driver.NavigateForAuthenticationAsync();
    ManagedPageDocument probe = await driver.ReadDocumentAsync();
    if (!hold) await driver.StopAsync();
    return new DevelopmentNetworkTest(SteamNetworkPolicy.IsAllowedHost(probe.Uri.IdnHost), "private-browser-host");
}
internal static class Native
{
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool GetNamedPipeServerProcessId(nint pipe, out uint processId);
}
internal sealed record DevelopmentNetworkTest(bool Passed, string Stage);
