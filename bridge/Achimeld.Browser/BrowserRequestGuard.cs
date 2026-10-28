using Microsoft.Playwright;

namespace Achimeld.Browser;

public static class BrowserRequestGuard
{
    // Install while Offline=true, before enabling network or opening live pages.
    // Routing covers redirects and first popup requests; direct APIRequest calls
    // use SteamBrowserRequestPolicy separately and prohibit redirects.
    public static async Task InstallAsync(IBrowserContext context, SteamBrowserRequestPolicy policy, Func<IPage?> ownedPage)
    {
        await context.RouteAsync("**/*", async route =>
        {
            bool allowed = false;
            BrowserRequestRejection rejection = BrowserRequestRejection.Other;
            BrowserRequestDiagnostic? details = null;
            try
            {
                IRequest request = route.Request;
                details = BrowserRequestDiagnostic.Describe(request.Method, request.Url, request.IsNavigationRequest, request.ResourceType);
                bool owned = ReferenceEquals(request.Frame.Page, ownedPage());
                bool validUri = Uri.TryCreate(request.Url, UriKind.Absolute, out Uri? uri);
                request.Headers.TryGetValue("content-type", out string? contentType);
                if (owned && validUri && policy.DiscardsCspReport(request.Method, uri!, request.ResourceType, request.IsNavigationRequest, contentType))
                {
                    // A local empty acknowledgement: never inspect or forward the
                    // report body, and never disable the page's CSP enforcement.
                    await route.FulfillAsync(new RouteFulfillOptions { Status = 204, Body = "" }).ConfigureAwait(false);
                    return;
                }
                allowed = owned && validUri && policy.Allows(request.Method, uri!, request.ResourceType, request.IsNavigationRequest);
                if (!allowed) rejection = ReferenceEquals(request.Frame.Page, ownedPage())
                    ? DescribeRejection(request) : BrowserRequestRejection.OtherPage;
            }
            catch (PlaywrightException) { rejection = BrowserRequestRejection.UnavailableFrame; }
            catch (UriFormatException) { }
            if (!allowed)
            {
                policy.Reject(rejection, details);
                await route.AbortAsync("blockedbyclient").ConfigureAwait(false);
                return;
            }
            await route.FallbackAsync().ConfigureAwait(false);
        }).ConfigureAwait(false);
        await context.RouteWebSocketAsync("**/*", socket =>
        {
            policy.Reject(BrowserRequestRejection.WebSocket);
            // A routed socket has no network connection until ConnectToServer is
            // called. Never connect it; the worker's terminal policy check kills
            // its Job. Own closure instead of the SDK's default server forwarding.
            socket.OnClose((_, _) => { });
        }).ConfigureAwait(false);
    }

    // A closed vocabulary, never a URL, query, header, body or arbitrary error.
    private static BrowserRequestRejection DescribeRejection(IRequest request)
    {
        if (request.IsNavigationRequest)
        {
            if (Uri.TryCreate(request.Url, UriKind.Absolute, out Uri? uri) && uri.IdnHost == "partner.steamgames.com")
            {
                if (uri.AbsolutePath is "/home" or "/home/") return BrowserRequestRejection.PartnerHome;
                if (uri.AbsolutePath is "/dashboard" or "/dashboard/") return BrowserRequestRejection.PartnerDashboard;
            }
            return BrowserRequestRejection.Navigation;
        }
        return request.ResourceType switch {
            "script" => BrowserRequestRejection.Script, "stylesheet" => BrowserRequestRejection.Stylesheet,
            "image" => BrowserRequestRejection.Image, "font" => BrowserRequestRejection.Font,
            "xhr" or "fetch" => BrowserRequestRejection.Dynamic,
            "ping" => BrowserRequestRejection.Ping, "media" => BrowserRequestRejection.Media,
            "manifest" => BrowserRequestRejection.Manifest, "eventsource" => BrowserRequestRejection.EventSource,
            _ => BrowserRequestRejection.Other,
        };
    }
}
