// Only transport interfaces are substituted. Parsing and hashing use local application source.
namespace Achimeld.Browser
{
    public sealed record ManagedPageDocument(Uri Uri, string Title, string Html);
    public interface IManagedChromeSession
    {
        Task<ManagedPageDocument> ReadCompleteConfigurationAsync(Achimeld.Core.SteamAppId appId, CancellationToken cancellationToken = default);
    }
}
namespace Achimeld.Application
{
    public interface IRemoteSnapshotReader
    {
        Task<Achimeld.Core.RemoteSnapshot> ReadSnapshotAsync(Achimeld.Core.SteamAppId appId, CancellationToken cancellationToken = default);
    }
}
