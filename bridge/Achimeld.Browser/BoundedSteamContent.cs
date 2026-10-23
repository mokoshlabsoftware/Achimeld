namespace Achimeld.Browser;

public static class BoundedSteamContent
{
    public static async Task<byte[]> ReadAsync(HttpContent content, int maximumBytes, CancellationToken cancellationToken = default)
    {
        if (maximumBytes is <= 0 or > 16 * 1024 * 1024) throw new ArgumentOutOfRangeException(nameof(maximumBytes));
        if (content.Headers.ContentLength is long length && length > maximumBytes) throw new InvalidDataException("Steam content exceeds its read budget.");
        await using Stream input = await content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        using MemoryStream output = new();
        byte[] buffer = new byte[65536];
        while (true)
        {
            int count = await input.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (count == 0) return output.ToArray();
            if (output.Length + count > maximumBytes) throw new InvalidDataException("Steam content exceeds its read budget.");
            output.Write(buffer, 0, count);
        }
    }
}
