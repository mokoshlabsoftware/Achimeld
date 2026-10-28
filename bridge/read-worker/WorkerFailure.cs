using Achimeld.Browser;
using Achimeld.Core;

namespace Achimeld.ReadWorker;

// Only this closed vocabulary crosses the worker boundary. Exception messages,
// Playwright call logs, URLs, portal contents and credentials never cross it.
internal static class WorkerFailure
{
    internal static System.Text.Json.JsonElement Details(string stage, Exception exception, long elapsed,
        BrowserRequestDiagnostic? request) => System.Text.Json.JsonSerializer.SerializeToElement(new {
            stage, kind = Kind(exception), elapsedMs = Math.Clamp(elapsed, 0, 300000), request,
        }, Achimeld.Security.SecurityWire.Json);

    internal static string Kind(Exception exception) => exception switch {
        OperationCanceledException => "cancelled", System.TimeoutException => "timeout",
        Microsoft.Playwright.PlaywrightException => "playwright", PortalSafetyException => "portal-policy",
        InvalidDataException => "invalid-data", InvalidOperationException => "invalid-state",
        System.ComponentModel.Win32Exception => "windows", IOException => "io",
        UnauthorizedAccessException => "access-denied", _ => "other",
    };
    // Owned-process exit status preserves a closed diagnostic when a request
    // rejection terminates the monitor before the active IPC reply can finish.
    internal static int ExitCode(BrowserRequestRejection rejection) => rejection switch
    {
        BrowserRequestRejection.OtherPage => 201, BrowserRequestRejection.Navigation => 202,
        BrowserRequestRejection.PartnerHome => 203, BrowserRequestRejection.PartnerDashboard => 204,
        BrowserRequestRejection.Script => 205, BrowserRequestRejection.Stylesheet => 206,
        BrowserRequestRejection.Image => 207, BrowserRequestRejection.Font => 208,
        BrowserRequestRejection.Dynamic => 209, BrowserRequestRejection.WebSocket => 210,
        BrowserRequestRejection.Other => 211, BrowserRequestRejection.Ping => 212,
        BrowserRequestRejection.Media => 213, BrowserRequestRejection.Manifest => 214,
        BrowserRequestRejection.EventSource => 215, BrowserRequestRejection.UnavailableFrame => 216, _ => 125,
    };
    internal static string Code(string operation, Exception exception, BrowserRequestRejection rejection = BrowserRequestRejection.None) => rejection != BrowserRequestRejection.None
        ? rejection switch {
            BrowserRequestRejection.OtherPage => "steam.request.other-page",
            BrowserRequestRejection.Ping => "steam.request.ping", BrowserRequestRejection.Media => "steam.request.media",
            BrowserRequestRejection.Manifest => "steam.request.manifest", BrowserRequestRejection.EventSource => "steam.request.eventsource",
            BrowserRequestRejection.UnavailableFrame => "steam.request.frame-unavailable",
            BrowserRequestRejection.PartnerHome => "steam.request.home", BrowserRequestRejection.PartnerDashboard => "steam.request.dashboard",
            BrowserRequestRejection.Navigation => "steam.request.navigation", BrowserRequestRejection.Script => "steam.request.script",
            BrowserRequestRejection.Stylesheet => "steam.request.stylesheet", BrowserRequestRejection.Image => "steam.request.image",
            BrowserRequestRejection.Font => "steam.request.font", BrowserRequestRejection.Dynamic => "steam.request.dynamic",
            BrowserRequestRejection.WebSocket => "steam.request.websocket", _ => "steam.request.rejected",
        }
        : exception switch
    {
        OperationCanceledException => "security.operation.interrupted",
        System.TimeoutException => "steam.operation.timeout",
        FileNotFoundException or DirectoryNotFoundException => "steam.runtime.missing",
        UnauthorizedAccessException => "steam.runtime.access-denied",
        System.Text.Json.JsonException when operation == "prepareSync" => "steam.project.invalid",
        PortalSafetyException => "steam.portal.rejected",
        UnsupportedPortalStructureException => "steam.portal.unsupported",
        IncompleteRemoteSnapshotException => "steam.snapshot.incomplete",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_TUNNEL_CONNECTION_FAILED", StringComparison.Ordinal) => "steam.network.tunnel",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_PROXY_CONNECTION_FAILED", StringComparison.Ordinal) => "steam.network.proxy",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_CERT_", StringComparison.Ordinal) => "steam.network.certificate",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_NAME_NOT_RESOLVED", StringComparison.Ordinal) => "steam.network.dns",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_TIMED_OUT", StringComparison.Ordinal) => "steam.network.timeout",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_NETWORK_CHANGED", StringComparison.Ordinal) => "steam.network.changed",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_INTERNET_DISCONNECTED", StringComparison.Ordinal) => "steam.network.disconnected",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_FAILED", StringComparison.Ordinal) => "steam.network.failed",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_CONNECTION_", StringComparison.Ordinal) => "steam.network.connection",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_HTTP_RESPONSE_CODE_FAILURE", StringComparison.Ordinal) => "steam.network.http",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("net::ERR_ABORTED", StringComparison.Ordinal) ||
            exception.Message.Contains("is interrupted by another navigation", StringComparison.Ordinal) => "steam.navigation.interrupted",
        Microsoft.Playwright.PlaywrightException when exception.Message.Contains("Target page, context or browser has been closed", StringComparison.Ordinal) => "steam.browser.closed",
        _ when operation == "start" => "steam.browser.start.failed",
        _ when operation is "navigate" or "authenticate" => "steam.navigation.failed",
        _ when operation is "prepareSync" or "beginSync" or "syncStep" or "finishSync" => "steam.sync.failed",
        _ => "steam.read.failed",
    };
}
