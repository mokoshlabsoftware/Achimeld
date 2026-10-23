namespace Achimeld.Security;

public static class DedicatedProfilePolicy
{
    public static string Resolve(string? configured = null, string? localApplicationData = null)
    {
        string local = Path.GetFullPath(localApplicationData ?? Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData));
        string current = Path.Combine(local, "Achimeld", "Browser", "ChromeProfile");
        string selected = string.IsNullOrWhiteSpace(configured)
            ? current
            : Path.TrimEndingDirectorySeparator(Path.GetFullPath(configured.Trim()));
        if (!string.Equals(selected, current, StringComparison.OrdinalIgnoreCase))
            throw new ArgumentException("Only Achimeld's dedicated browser profile is permitted.", nameof(configured));
        RejectReparsePoints(selected);
        return selected;
    }

    public static void RejectReparsePoints(string path)
    {
        for (string? current = Path.GetFullPath(path); current is not null; current = Path.GetDirectoryName(current))
        {
            if ((File.Exists(current) || Directory.Exists(current)) &&
                (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Security-sensitive paths cannot traverse reparse points.");
        }
    }
}
