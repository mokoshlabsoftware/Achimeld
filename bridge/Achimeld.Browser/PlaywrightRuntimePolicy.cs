namespace Achimeld.Browser;

public static class PlaywrightRuntimePolicy
{
    public static void Configure(string packageRoot)
    {
        // The SDK otherwise accepts executable/search-path overrides from the environment.
        string root = Path.TrimEndingDirectorySeparator(Path.GetFullPath(packageRoot));
        if (!string.Equals(root, Path.TrimEndingDirectorySeparator(Path.GetFullPath(AppContext.BaseDirectory)), StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Playwright must run from the authenticated worker package.");
        string node = Path.Combine(root, ".playwright", "node", "win32_x64", "node.exe");
        Achimeld.Security.DedicatedProfilePolicy.RejectReparsePoints(node);
        if (!File.Exists(node)) throw new FileNotFoundException("The packaged Playwright runtime is unavailable.");
        Environment.SetEnvironmentVariable("PLAYWRIGHT_NODEJS_PATH", Path.Combine(root, "Achimeld.NodeHost.exe"));
        Environment.SetEnvironmentVariable("PLAYWRIGHT_DRIVER_SEARCH_PATH", null);
        Environment.SetEnvironmentVariable("NODE_OPTIONS", null);
        Environment.SetEnvironmentVariable("NODE_PATH", null);
        Environment.SetEnvironmentVariable("NODE_EXTRA_CA_CERTS", null);
        Environment.SetEnvironmentVariable("NODE_TLS_REJECT_UNAUTHORIZED", null);
    }
}
