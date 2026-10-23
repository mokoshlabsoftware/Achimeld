using System.Collections.Concurrent;
using System.Security.Cryptography;

namespace Achimeld.Browser;

internal static class SteamIconHashReader
{
    internal const int MaximumConcurrentDownloads = 8;

    internal static async Task<IReadOnlyDictionary<string, string>> ReadAsync(
        IEnumerable<string> urls,
        Func<string, Task<byte[]>> download)
    {
        ConcurrentDictionary<string, string> hashes = new(StringComparer.Ordinal);
        await Parallel.ForEachAsync(urls.Distinct(StringComparer.Ordinal),
            new ParallelOptions { MaxDegreeOfParallelism = MaximumConcurrentDownloads },
            async (url, _) =>
            {
                byte[] bytes = await download(url).ConfigureAwait(false);
                string hash = Convert.ToHexStringLower(SHA256.HashData(bytes));
                hashes[url] = hash;
            }).ConfigureAwait(false);
        return hashes;
    }
}