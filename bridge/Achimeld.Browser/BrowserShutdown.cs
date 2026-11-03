using Microsoft.Playwright;

namespace Achimeld.Browser;

public static class BrowserShutdown
{
    public static async Task CloseAsync(IBrowserContext context)
    {
        try
        {
            // Stop new network activity, but keep the last page alive until the
            // persistent context closes. Closing that page first lets headed
            // Chromium exit ahead of Playwright's context-close acknowledgement.
            // Context closure itself bypasses page beforeunload prompts.
            await context.SetOfflineAsync(true).ConfigureAwait(false);
        }
        finally
        {
            // Persistent Chromium shutdown may never acknowledge CloseAsync.
            // Surface a bounded failure to the supervised host, which owns the
            // worker Job and confirms process cleanup before another launch.
            await context.CloseAsync().WaitAsync(TimeSpan.FromSeconds(60)).ConfigureAwait(false);
        }
    }
}
