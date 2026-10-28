using System.Globalization;
using Microsoft.Playwright;

namespace Achimeld.Browser;

public static class SteamLoginPrompt
{
    // Open Steam's own form, without reading credentials or submitting login.
    // Its public root-page button otherwise returns to an unrelated dashboard.
    public static async Task OpenIfNeededAsync(IPage page, Uri destination)
    {
        IReadOnlyList<uint> ids = SteamworksUrlGuard.ExtractAppIds(destination);
        if (ids.Count != 1 || ids[0] == 0 || destination.AbsoluteUri !=
            $"https://partner.steamgames.com/apps/achievements/{ids[0].ToString(CultureInfo.InvariantCulture)}")
            throw new InvalidDataException("Authentication destination rejected.");
        if (page.Url != "https://partner.steamgames.com/" ||
            !await page.Locator("#login_btn_signin").IsVisibleAsync().ConfigureAwait(false)) return;
        await page.WaitForFunctionAsync("() => typeof window.g_ShowLoginDialog === 'function'",
            options: new PageWaitForFunctionOptions { Timeout = 15000 }).ConfigureAwait(false);
        await page.EvaluateAsync("destination => window.g_ShowLoginDialog(destination)",
            destination.AbsoluteUri).ConfigureAwait(false);
    }
}
