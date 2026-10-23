using System.Globalization;
using System.Text.Json;
using Achimeld.Security;

namespace Achimeld.Browser;

// Store metadata is untrusted. Only the selected app's capsule on the known
// Steam CDN may supply a content hash; it cannot choose a generic download.
public static class SteamGameBanner
{
    public static Uri? Resolve(uint appId, byte[] metadata)
    {
        if (appId == 0 || metadata.Length > 512 * 1024) return null;
        try
        {
            using JsonDocument document = JsonDocument.Parse(metadata, new JsonDocumentOptions { MaxDepth = 32 });
            if (!document.RootElement.TryGetProperty(appId.ToString(CultureInfo.InvariantCulture), out JsonElement app) ||
                !app.TryGetProperty("success", out JsonElement success) || success.ValueKind != JsonValueKind.True ||
                !app.TryGetProperty("data", out JsonElement data) ||
                !data.TryGetProperty("capsule_image", out JsonElement capsule) || capsule.ValueKind != JsonValueKind.String)
                return null;
            string? text = capsule.GetString();
            if (text is null || text.Length > 2048 || !Uri.TryCreate(text, UriKind.Absolute, out Uri? uri)) return null;
            SteamNetworkPolicy.RequireHttps(uri);
            if (uri.IdnHost is not ("shared.fastly.steamstatic.com" or "shared.akamai.steamstatic.com" or "cdn.akamai.steamstatic.com") ||
                uri.Fragment.Length != 0) return null;
            // Steam's cache timestamp is irrelevant to the content-addressed
            // image. Accept only that query and never forward it.
            if (uri.Query.Length != 0 && !(uri.Query.StartsWith("?t=", StringComparison.Ordinal) &&
                uri.Query.Length is > 3 and <= 23 && uri.Query.AsSpan(3).ContainsAnyExceptInRange('0', '9') == false))
                return null;
            string id = appId.ToString(CultureInfo.InvariantCulture);
            string prefix = $"/store_item_assets/steam/apps/{id}/";
            string path = uri.AbsolutePath;
            if (path.StartsWith(prefix, StringComparison.Ordinal))
            {
                string[] parts = path[prefix.Length..].Split('/');
                if (parts.Length != 2 || parts[0].Length != 40 || !parts[0].All(char.IsAsciiHexDigit) ||
                    parts[1] != "capsule_231x87.jpg") return null;
                return new Uri($"https://{uri.IdnHost}{prefix}{parts[0]}/capsule_231x87_2x.jpg");
            }
            if (path == $"/steam/apps/{id}/capsule_231x87.jpg")
                return new Uri($"https://{uri.IdnHost}/steam/apps/{id}/capsule_231x87_2x.jpg");
            return null;
        }
        catch (Exception exception) when (exception is JsonException or InvalidOperationException or InvalidDataException or ArgumentException)
        {
            return null;
        }
    }
}
