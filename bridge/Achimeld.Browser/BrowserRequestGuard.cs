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
            try
            {
                IRequest request = route.Request;
                allowed = ReferenceEquals(request.Frame.Page, ownedPage()) &&
                    Uri.TryCreate(request.Url, UriKind.Absolute, out Uri? uri) &&
                    policy.Allows(request.Method, uri, request.ResourceType, request.IsNavigationRequest);
                if (!allowed) rejection = DescribeRejection(request);
            }
            catch (PlaywrightException) { }
            catch (UriFormatException) { }
            if (!allowed)
            {
                policy.Reject(rejection);
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
            "xhr" or "fetch" => BrowserRequestRejection.Dynamic, _ => BrowserRequestRejection.Other,
        };
    }
}
