using Achimeld.Browser;
using Achimeld.ReadWorker;

internal static class WorkerDiagnosticFixtures
{
    internal static int Run()
    {
        const string secret = "https://secret.invalid/?token=private password=hidden";
        (Exception Error, string Code)[] cases = [
            (new OperationCanceledException(secret), "security.operation.interrupted"),
            (new System.TimeoutException(secret), "steam.operation.timeout"),
            (new Achimeld.Core.IncompleteRemoteSnapshotException(), "steam.snapshot.incomplete"),
            (new FileNotFoundException(secret), "steam.runtime.missing"),
            (new DirectoryNotFoundException(secret), "steam.runtime.missing"),
            (new UnauthorizedAccessException(secret), "steam.runtime.access-denied"),
            (new System.Text.Json.JsonException(secret), "steam.project.invalid"),
            (new PortalSafetyException(secret, secret), "steam.portal.rejected"),
            (new UnsupportedPortalStructureException(secret), "steam.portal.unsupported"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_RESET " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_FAILED " + secret), "steam.network.failed"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_TIMED_OUT " + secret), "steam.network.timeout"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_NETWORK_CHANGED " + secret), "steam.network.changed"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_INTERNET_DISCONNECTED " + secret), "steam.network.disconnected"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_CLOSED " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_REFUSED " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_HTTP_RESPONSE_CODE_FAILURE " + secret), "steam.network.http"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_ABORTED " + secret), "steam.navigation.interrupted"),
            (new Microsoft.Playwright.PlaywrightException("Navigation to " + secret + " is interrupted by another navigation"), "steam.navigation.interrupted"),
            (new Microsoft.Playwright.PlaywrightException("Target page, context or browser has been closed " + secret), "steam.browser.closed"),
        ];
        foreach (var test in cases)
        {
            if (WorkerFailure.Code("prepareSync", test.Error) != test.Code)
                throw new Exception("Worker diagnostic classification changed or exposed raw error data.");
            if (WorkerFailure.Kind(test.Error).Contains(secret, StringComparison.Ordinal))
                throw new Exception("Worker diagnostic kind exposed raw error data.");
        }
        foreach (BrowserRequestRejection rejection in Enum.GetValues<BrowserRequestRejection>())
        {
            if (rejection != BrowserRequestRejection.None && !WorkerFailure.Code("authenticate", new Exception(secret), rejection).StartsWith("steam.request.", StringComparison.Ordinal))
                throw new Exception("Request rejection did not map to declared static diagnostics.");
            int exitCode = WorkerFailure.ExitCode(rejection);
            if (rejection == BrowserRequestRejection.None ? exitCode != 125 : exitCode is < 201 or > 216)
                throw new Exception("Monitor exit code escaped the declared diagnostic vocabulary.");
        }
        (BrowserRequestRejection Reason, string Code, int Exit)[] requestCases = [
            (BrowserRequestRejection.OtherPage, "steam.request.other-page", 201),
            (BrowserRequestRejection.Ping, "steam.request.ping", 212),
            (BrowserRequestRejection.Media, "steam.request.media", 213),
            (BrowserRequestRejection.Manifest, "steam.request.manifest", 214),
            (BrowserRequestRejection.EventSource, "steam.request.eventsource", 215),
            (BrowserRequestRejection.UnavailableFrame, "steam.request.frame-unavailable", 216),
        ];
        foreach (var test in requestCases)
            if (WorkerFailure.Code("navigate", new Exception(secret), test.Reason) != test.Code ||
                WorkerFailure.ExitCode(test.Reason) != test.Exit)
                throw new Exception("Request rejection lost its specific safe diagnostic.");
        if (WorkerFailure.Code("start", new Exception(secret)) != "steam.browser.start.failed" ||
            WorkerFailure.Code("prepareSync", new Exception(secret)) != "steam.sync.failed" ||
            WorkerFailure.Code("authenticate", new Exception(secret)) != "steam.navigation.failed" ||
            WorkerFailure.Code("navigate", new Exception(secret)) != "steam.navigation.failed" ||
            WorkerFailure.Code("readConfiguration", new Exception(secret)) != "steam.read.failed")
            throw new Exception("Worker fallback diagnostic classification changed.");
        return cases.Length + 3;
    }
}
