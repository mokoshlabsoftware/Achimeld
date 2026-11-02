using Microsoft.Playwright;
using System.Security.Cryptography;

namespace Achimeld.Browser;

public sealed record SteamGameDetails(string? Name, byte[]? Banner);

public sealed partial class PlaywrightChromeDriver
{
    private readonly Dictionary<(uint AppId, string ApiName), (string Achieved, string Unachieved)> _uploadedIconHashes = new();

    public void BeginReviewedDraft(uint appId, PortalEntityKind kind)
    {
        _requestPolicy.BeginReviewedDraft(appId, kind);
        if (kind == PortalEntityKind.Icon) _uploadedIconHashes.Clear();
    }
    public void EndReviewedDraft() => _requestPolicy.EndReviewedDraft();

    private static async Task EnsureUniqueAsync(ILocator locator, string description)
    {
        if (await locator.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("Expected one " + description + ".");
    }

    private async Task<string?> ReadSavedIconHashAsync(string url)
    {
        if (string.IsNullOrEmpty(url)) return null;
        byte[] saved = await DownloadIconAsync(url).ConfigureAwait(false);
        if (!BoundedRaster.HasSafeDimensions(saved, 256))
            throw new InvalidOperationException("Steamworks saved an icon with unexpected image dimensions or format.");
        return Convert.ToHexStringLower(SHA256.HashData(saved));
    }

    public Task<bool> VerifyIconPairAsync(PortalWriteOperation operation, Achimeld.Core.RemoteIconMetadata? actual) =>
        Task.FromResult(_uploadedIconHashes.TryGetValue((operation.AppId.Value, operation.Target.ApiName.Value), out var expected) &&
            actual?.AchievedPreviewUri is not null && actual.UnachievedPreviewUri is not null &&
            actual.AchievedIdentity == expected.Achieved && actual.UnachievedIdentity == expected.Unachieved);

    public Task<byte[]> ReadApprovedArtworkAsync(Uri uri) => DownloadIconAsync(uri.AbsoluteUri);

    public async Task<byte[]?> ReadGameBannerAsync(uint appId) => (await ReadGameDetailsAsync(appId)).Banner;

    public async Task<SteamGameDetails> ReadGameDetailsAsync(uint appId)
    {
        await EnterReadForAppAsync(new Achimeld.Core.SteamAppId(appId)).ConfigureAwait(false);
        // Resolve Steam's content-addressed capsule using fixed public metadata
        // for the pinned app. Both requests use the cookie-free, no-redirect client.
        Uri fallback = _requestPolicy.GameBannerUri(appId);
        Uri? current = null;
        string? name = null;
        try
        {
            using CancellationTokenSource deadline = new(TimeSpan.FromSeconds(5));
            using HttpRequestMessage request = new(HttpMethod.Get, _requestPolicy.GameBannerMetadataUri(appId));
            using HttpResponseMessage response = await _iconDownloads!.SendAsync(request,
                HttpCompletionOption.ResponseHeadersRead, deadline.Token).ConfigureAwait(false);
            if (response.IsSuccessStatusCode)
            {
                byte[] metadata = await BoundedSteamContent.ReadAsync(response.Content, 512 * 1024, deadline.Token).ConfigureAwait(false);
                current = _requestPolicy.ResolveGameBanner(appId, metadata);
                name = SteamGameBanner.ResolveName(appId, metadata);
            }
        }
        catch (Exception exception) when (exception is HttpRequestException or OperationCanceledException or InvalidDataException)
        {
            // Optional artwork must not prevent configuration validation.
        }
        if (current is not null)
        {
            byte[]? currentBytes = await DownloadGameBannerAsync(current).ConfigureAwait(false);
            if (currentBytes is not null) return new(name, currentBytes);
        }
        return new(name, await DownloadGameBannerAsync(fallback).ConfigureAwait(false));
    }

    private async Task<byte[]?> DownloadGameBannerAsync(Uri uri)
    {
        try
        {
            using CancellationTokenSource deadline = new(TimeSpan.FromSeconds(5));
            using HttpRequestMessage request = new(HttpMethod.Get, uri);
            using HttpResponseMessage response = await _iconDownloads!.SendAsync(request,
                HttpCompletionOption.ResponseHeadersRead, deadline.Token).ConfigureAwait(false);
            if (!response.IsSuccessStatusCode) return null;
            byte[] bytes = await BoundedSteamContent.ReadAsync(response.Content, 2 * 1024 * 1024, deadline.Token).ConfigureAwait(false);
            return BoundedRaster.HasSafeDimensions(bytes, 1024) ? bytes : null;
        }
        catch (Exception exception) when (exception is HttpRequestException or OperationCanceledException or InvalidDataException)
        {
            return null;
        }
    }
}
