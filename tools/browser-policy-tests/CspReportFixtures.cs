using Achimeld.Browser;

internal static class CspReportFixtures
{
    internal static int Run()
    {
        const string endpoint = "https://partner.steamgames.com/actions/CSPReport";
        SteamBrowserRequestPolicy policy = new();
        const string contentType = "application/csp-report";
        if (policy.DiscardsCspReport("POST", new Uri(endpoint), "cspreport", false, contentType)) throw new Exception("Dormant report scope accepted.");
        policy.PinApp(480);
        foreach (bool read in new[] { false, true })
        {
            if (read) policy.EnterRead();
            foreach (string resource in new[] { "cspreport", "other" })
                if (!policy.DiscardsCspReport("POST", new Uri(endpoint), resource, false, contentType)) throw new Exception("Known browser report was not discarded.");
            foreach (string? type in new[] { null, "application/json", "text/plain", "application/reports+json" })
                if (policy.DiscardsCspReport("POST", new Uri(endpoint), "other", false, type)) throw new Exception("Unknown report format accepted.");
            if (policy.Allows("POST", new Uri(endpoint), "cspreport", false)) throw new Exception("CSP report gained upstream authority.");
            foreach (string url in new[] { endpoint + "/", endpoint + "?secret=fixture", endpoint + "#fragment", endpoint.Replace("https:", "http:"), endpoint.Replace(".com/", ".com:8443/"), endpoint.Replace("partner.steamgames.com", "store.steampowered.com"), endpoint.Replace(".com/", ".com.evil.test/"), endpoint.Replace("https://", "https://user:password@"), endpoint.Replace("CSPReport", "saveachievement") })
                if (policy.DiscardsCspReport("POST", new Uri(url), "cspreport", false, contentType)) throw new Exception("Report scope escaped its exact destination.");
            foreach (string resource in new[] { "fetch", "xhr", "document", "ping" })
                if (policy.DiscardsCspReport("POST", new Uri(endpoint), resource, false, contentType)) throw new Exception("Ordinary request disguised itself as a browser report.");
            if (policy.DiscardsCspReport("GET", new Uri(endpoint), "cspreport", false, contentType) || policy.DiscardsCspReport("POST", new Uri(endpoint), "cspreport", true, contentType)) throw new Exception("Report method/navigation escaped.");
        }
        policy.Reject();
        if (policy.DiscardsCspReport("POST", new Uri(endpoint), "cspreport", false, contentType)) throw new Exception("Rejected policy regained report handling.");
        var diagnostic = BrowserRequestDiagnostic.Describe("POST", endpoint, false, "cspreport");
        if (diagnostic.Path != "csp-report" || diagnostic.Resource != "cspreport") throw new Exception("Report diagnostic lost its closed categories.");
        return 47;
    }
}
