using Microsoft.Playwright;

namespace Achimeld.Browser;

public static class BrowserStartup
{
    // Block direct DNS while keeping the literal loopback broker resolvable.
    // Chromium applies MAP rules to proxy endpoints, including IP literals.
    public static string[] LaunchArguments => ["--proxy-bypass-list=<-loopback>", "--disable-quic",
        "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1",
        "--disable-extensions", "--disable-component-update", "--disable-background-networking"];

    // The persistent context is offline throughout page ownership setup.
    public static async Task<IPage> CreateOwnedPageAsync(IBrowserContext context, SteamBrowserRequestPolicy policy, Func<IPage?> ownedPage)
    {
        // Pages is a live list: closing a page mutates it. Keep an owned blank
        // page alive before closing restored tabs, especially the last tab of a
        // headed persistent context. Networking remains disabled by the caller.
        IPage[] restoredPages = context.Pages.ToArray();
        await BrowserRequestGuard.InstallAsync(context, policy, ownedPage).ConfigureAwait(false);
        IPage owned = await context.NewPageAsync().ConfigureAwait(false);
        foreach (IPage restored in restoredPages)
            await restored.CloseAsync(new PageCloseOptions { RunBeforeUnload = false }).ConfigureAwait(false);
        return owned;
    }
}
