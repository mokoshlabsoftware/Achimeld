namespace Achimeld.Browser;

public static class IconUploadVerification
{
    public static bool IsFresh(string? beforeHash, string? savedHash) =>
        savedHash is { Length: 64 } && savedHash.All(char.IsAsciiHexDigit) &&
        !string.Equals(beforeHash, savedHash, StringComparison.OrdinalIgnoreCase);

    // Steam's image URL can precede CDN propagation. Retry only the read-back;
    // never repeat a draft POST whose outcome may already be committed.
    public static async Task<bool> WaitForSavedArtworkAsync(
        Func<Task<bool>> fresh, Func<TimeSpan, Task> delay)
    {
        ArgumentNullException.ThrowIfNull(fresh);
        ArgumentNullException.ThrowIfNull(delay);
        for (int attempt = 0; attempt < 5; attempt++)
        {
            if (await fresh().ConfigureAwait(false)) return true;
            if (attempt < 4)
                await delay(TimeSpan.FromMilliseconds(500 << attempt)).ConfigureAwait(false);
        }
        return false;
    }
}
