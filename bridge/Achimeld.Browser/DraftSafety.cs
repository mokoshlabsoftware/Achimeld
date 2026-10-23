using Microsoft.Playwright;
using System.Security.Cryptography;

namespace Achimeld.Browser;

public sealed partial class PlaywrightChromeDriver
{
    public void BeginReviewedDraft(uint appId, PortalEntityKind kind) => _requestPolicy.BeginReviewedDraft(appId, kind);
    public void EndReviewedDraft() => _requestPolicy.EndReviewedDraft();

    private static async Task EnsureUniqueAsync(ILocator locator, string description)
    {
        if (await locator.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("Expected one " + description + ".");
    }

    private async Task<bool> VerifySavedIconAsync(string url, byte[] approved, string? expectedHash = null)
    {
        if (string.IsNullOrEmpty(url)) return false;
        byte[] saved = await DownloadIconAsync(url).ConfigureAwait(false);
        if (expectedHash is not null && Convert.ToHexStringLower(SHA256.HashData(saved)) != expectedHash) return false;
        if (!BoundedRaster.HasSafeDimensions(approved, 4096) || !BoundedRaster.HasSafeDimensions(saved, 256)) return false;
        // Fixed application script; only bounded image bytes are supplied. Steam converts
        // uploads to 256px JPEGs, so compare decoded pixels. Persist raw remote hashes.
        bool matches = await (await EnsurePageAsync().ConfigureAwait(false)).EvaluateAsync<bool>("""
            async ({expected, actual}) => {
                const decode = async value => createImageBitmap(new Blob([Uint8Array.from(atob(value), c => c.charCodeAt(0))]));
                const a = await decode(expected), b = await decode(actual);
                try {
                    const w = Math.min(a.width, 256), h = Math.min(a.height, 256);
                    if (a.width > 4096 || a.height > 4096 || b.width !== w || b.height !== h) return false;
                    const pixels = image => {
                        const canvas = document.createElement('canvas'); canvas.width = w; canvas.height = h;
                        const ctx = canvas.getContext('2d', {willReadFrequently: true});
                        ctx.fillStyle = 'black'; ctx.fillRect(0, 0, w, h);
                        ctx.imageSmoothingEnabled = true; ctx.imageSmoothingQuality = 'high';
                        ctx.drawImage(image, 0, 0, w, h); return ctx.getImageData(0, 0, w, h).data;
                    };
                    const x = pixels(a), y = pixels(b); let error = 0;
                    for (let i = 0; i < x.length; i++) if (i % 4 !== 3) error += (x[i] - y[i]) ** 2;
                    return Math.sqrt(error / (w * h * 3)) / 255 <= 0.03;
                } finally { a.close(); b.close(); }
            }
            """, new { expected = Convert.ToBase64String(approved), actual = Convert.ToBase64String(saved) }).ConfigureAwait(false);
        return matches;
    }

    public async Task<bool> VerifyIconPairAsync(PortalWriteOperation operation, Achimeld.Core.RemoteIconMetadata? actual) =>
        actual?.AchievedPreviewUri is Uri achieved && actual.UnachievedPreviewUri is Uri unachieved &&
        actual.AchievedIdentity is string achievedHash && actual.UnachievedIdentity is string unachievedHash &&
        await VerifySavedIconAsync(achieved.AbsoluteUri, operation.GeneratedUploads["achievedIcon"], achievedHash).ConfigureAwait(false) &&
        await VerifySavedIconAsync(unachieved.AbsoluteUri, operation.GeneratedUploads["unachievedIcon"], unachievedHash).ConfigureAwait(false);

    public Task<byte[]> ReadApprovedArtworkAsync(Uri uri) => DownloadIconAsync(uri.AbsoluteUri);

    public async Task<byte[]?> ReadGameBannerAsync(uint appId)
    {
        // Resolve Steam's content-addressed capsule using fixed public metadata
        // for the pinned app. Both requests use the cookie-free, no-redirect client.
        Uri fallback = _requestPolicy.GameBannerUri(appId);
        Uri? current = null;
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
            }
        }
        catch (Exception exception) when (exception is HttpRequestException or OperationCanceledException or InvalidDataException)
        {
            // Optional artwork must not prevent configuration validation.
        }
        if (current is not null)
        {
            byte[]? currentBytes = await DownloadGameBannerAsync(current).ConfigureAwait(false);
            if (currentBytes is not null) return currentBytes;
        }
        return await DownloadGameBannerAsync(fallback).ConfigureAwait(false);
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
