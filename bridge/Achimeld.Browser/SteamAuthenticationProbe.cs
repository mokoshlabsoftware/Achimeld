using Microsoft.Playwright;

namespace Achimeld.Browser;

public static class SteamAuthenticationProbe
{
    // Read URL and indicators in one document, never values from login fields.
    // A redirect can destroy that document while Playwright is evaluating it.
    public static async Task<PortalAccessState> ReadAsync(IPage page)
    {
        try
        {
            Probe probe = await page.EvaluateAsync<Probe>("""
                () => ({
                    Url: location.href,
                    Loading: document.readyState === 'loading',
                    Indicators: document.querySelector('input[type="password"], #login_form')
                        ? 'login_form' : (document.body?.innerText || '').slice(0, 4096)
                })
                """).ConfigureAwait(false);
            if (probe.Loading || probe.Url != page.Url) return PortalAccessState.Unknown;
            return new SteamworksPageClassifier().ClassifyAuthentication(
                new ManagedPageDocument(new Uri(probe.Url), "", probe.Indicators));
        }
        catch (PlaywrightException exception) when (!page.IsClosed && IsNavigationTransition(exception))
        {
            // Only this read-only observation is deferred. The host's existing
            // sign-in deadline and health checks still apply; no operation is replayed.
            return PortalAccessState.Unknown;
        }
    }

    public static bool IsNavigationTransition(PlaywrightException exception) =>
        exception.Message.Contains("Execution context was destroyed", StringComparison.Ordinal);

    private sealed class Probe
    {
        public Probe() { }
        public string Url { get; set; } = "";
        public bool Loading { get; set; }
        public string Indicators { get; set; } = "";
    }
}
