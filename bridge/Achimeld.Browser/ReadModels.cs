using Achimeld.Core;

namespace Achimeld.Browser;

public sealed record ChromeLaunchSettings(string ExecutablePath, string ProfileDirectory, TimeSpan OperationTimeout)
{
    public static readonly TimeSpan DefaultTimeout = TimeSpan.FromSeconds(30);
    public int? ProxyPort { get; init; }
}

public sealed record ManagedPageDocument(Uri Uri, string Title, string Html);

public interface IManagedChromeSession
{
    Task<ManagedPageDocument> ReadCompleteConfigurationAsync(SteamAppId appId, CancellationToken cancellationToken = default);
}
