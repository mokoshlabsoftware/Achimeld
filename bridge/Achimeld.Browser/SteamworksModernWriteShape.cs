namespace Achimeld.Browser;

// A fixed diagnostic mask for one blocked, reviewed achievement save. No
// request body, field value, URL, or arbitrary page text leaves the worker.
public static class SteamworksModernWriteShape
{
    // Six closed bits per part, in request order, for at most ten parts.
    // Category: 0 API, 1 name, 2 description, 3 stat, 4 image,
    // 5 achievement/localization, 6 data/file, 7 other. The remaining bits
    // say only whether reviewed ID, another ID, and desired text occur.
    public static ulong ReadMultipartSequence(string? contentType, string? body,
        string apiName, string desired, IReadOnlySet<string>? knownApiNames = null)
    {
        if (body is null || body.Length > 65_536 || contentType is null ||
            contentType.Length > 200) return 0;
        var boundaryMatch = System.Text.RegularExpressions.Regex.Match(contentType,
            @"\bboundary=(?<value>[A-Za-z0-9_-]{1,80})\b",
            System.Text.RegularExpressions.RegexOptions.IgnoreCase);
        if (!boundaryMatch.Success) return 0;
        ulong result = 0;
        int index = 0;
        HashSet<string> knownOthers = new(StringComparer.Ordinal);
        foreach (string part in body.Split("--" + boundaryMatch.Groups["value"].Value,
            StringSplitOptions.None))
        {
            int separator = part.IndexOf("\r\n\r\n", StringComparison.Ordinal);
            if (separator < 0 || separator > 1024) continue;
            var nameMatch = System.Text.RegularExpressions.Regex.Match(part[..separator],
                "name=\"(?<value>[A-Za-z0-9_.-]{1,64})\"");
            if (!nameMatch.Success) continue;
            string name = nameMatch.Groups["value"].Value.ToLowerInvariant();
            string value = part[(separator + 4)..];
            int category = name.Contains("api", StringComparison.Ordinal) ? 0 :
                name.Contains("description", StringComparison.Ordinal) ? 2 :
                name.Contains("progress", StringComparison.Ordinal) ||
                    name.Contains("stat", StringComparison.Ordinal) ? 3 :
                name.Contains("icon", StringComparison.Ordinal) ||
                    name.Contains("image", StringComparison.Ordinal) ? 4 :
                name.Contains("localiz", StringComparison.Ordinal) ||
                    name.Contains("translat", StringComparison.Ordinal) ||
                    name.Contains("achievement", StringComparison.Ordinal) ? 5 :
                name.Contains("name", StringComparison.Ordinal) ? 1 :
                name.Contains("data", StringComparison.Ordinal) ||
                    name.Contains("file", StringComparison.Ordinal) ? 6 : 7;
            string[] ids = System.Text.RegularExpressions.Regex.Matches(value,
                    @"ACHIEVEMENT_[A-Za-z0-9_]{1,80}")
                .Cast<System.Text.RegularExpressions.Match>()
                .Select(match => match.Value).Distinct(StringComparer.Ordinal).ToArray();
            if (knownApiNames is not null)
                foreach (string id in ids)
                    if (id != apiName && knownApiNames.Contains(id)) knownOthers.Add(id);
            int bits = category | (ids.Contains(apiName, StringComparer.Ordinal) ? 8 : 0) |
                (ids.Any(id => id != apiName) ? 16 : 0) |
                (value.Contains(desired, StringComparison.Ordinal) ? 32 : 0);
            if (index < 10) result |= (ulong)bits << (index * 6);
            index++;
        }
        return result | ((ulong)Math.Min(knownOthers.Count, 3) << 60);
    }
    // Summarize only fixed field categories and identity presence in a blocked
    // multipart request. Never return part names, values, or request bytes.
    public static uint ReadIdentityLocations(string? contentType, string? body,
        string apiName, string desired)
    {
        if (body is null || body.Length > 65_536 || contentType is null ||
            contentType.Length > 200) return 0;
        var boundaryMatch = System.Text.RegularExpressions.Regex.Match(contentType,
            @"\bboundary=(?<value>[A-Za-z0-9_-]{1,80})\b", 
            System.Text.RegularExpressions.RegexOptions.IgnoreCase);
        if (!boundaryMatch.Success) return 0;
        int selected = 0, other = 0, parts = 0, desiredWithSelected = 0,
            desiredWithOther = 0;
        foreach (string part in body.Split("--" + boundaryMatch.Groups["value"].Value,
            StringSplitOptions.None))
        {
            int separator = part.IndexOf("\r\n\r\n", StringComparison.Ordinal);
            if (separator < 0 || separator > 1024) continue;
            string header = part[..separator];
            var nameMatch = System.Text.RegularExpressions.Regex.Match(header,
                "name=\"(?<value>[A-Za-z0-9_.-]{1,64})\"");
            if (!nameMatch.Success) continue;
            parts++;
            string name = nameMatch.Groups["value"].Value.ToLowerInvariant();
            int category = name.Contains("api", StringComparison.Ordinal) ? 1 :
                name.Contains("description", StringComparison.Ordinal) ? 4 :
                name.Contains("progress", StringComparison.Ordinal) ||
                    name.Contains("stat", StringComparison.Ordinal) ? 8 :
                name.Contains("icon", StringComparison.Ordinal) ||
                    name.Contains("image", StringComparison.Ordinal) ? 16 :
                name.Contains("localiz", StringComparison.Ordinal) ||
                    name.Contains("translat", StringComparison.Ordinal) ? 32 :
                name.Contains("achievement", StringComparison.Ordinal) ? 64 :
                name.Contains("name", StringComparison.Ordinal) ? 2 : 128;
            string value = part[(separator + 4)..];
            string[] ids = System.Text.RegularExpressions.Regex.Matches(value,
                    @"ACHIEVEMENT_[A-Za-z0-9_]{1,80}")
                .Cast<System.Text.RegularExpressions.Match>()
                .Select(match => match.Value).Distinct(StringComparer.Ordinal).ToArray();
            if (ids.Contains(apiName, StringComparer.Ordinal))
            {
                selected |= category;
                if (value.Contains(desired, StringComparison.Ordinal)) desiredWithSelected = 1;
            }
            if (ids.Any(id => id != apiName))
            {
                other |= category;
                if (value.Contains(desired, StringComparison.Ordinal)) desiredWithOther = 1;
            }
        }
        return (uint)(selected | (other << 8) | (Math.Min(parts, 255) << 16) |
            (desiredWithSelected << 24) | (desiredWithOther << 25));
    }
    public static bool AllowsReviewed(string method, Uri uri, string resource,
        string? contentType, string? body, uint appId, string apiName, string desired) =>
        Read(method, uri, resource, contentType, body, appId, apiName, desired) == 1383 &&
        ReadStructure(contentType, body) == 1041 &&
        ReadGuardConditions(contentType, body, apiName) == 3;

    // Steam's current localization import emits one additional ACHIEVEMENT_ token
    // that is absent from the fresh export. Permit only the observed one-row
    // shape after a reviewed import preview and a known-API inventory check.
    public static bool AllowsReviewedLocalizationImport(string method, Uri uri,
        string resource, string? contentType, string? body, uint appId,
        string apiName, string desired, IReadOnlySet<string>? knownApiNames) =>
        knownApiNames is { Count: >= 1 and <= 5000 } && knownApiNames.Contains(apiName) &&
        Read(method, uri, resource, contentType, body, appId, apiName, desired) == 1383 &&
        ReadStructure(contentType, body) == 1041 &&
        ReadIdentityShape(body, apiName) == 426 &&
        ReadMultipartSequence(contentType, body, apiName, desired, knownApiNames) ==
            215_713_711_419_843UL;

    // Two fixed booleans: exact multipart header shape and a single reviewed
    // achievement identity in the bounded body. No header or body text is logged.
    public static int ReadGuardConditions(string? contentType, string? body, string apiName)
    {
        int mask = contentType is not null && contentType.Length <= 200 &&
            contentType.StartsWith("multipart/form-data; boundary=", StringComparison.OrdinalIgnoreCase) ? 1 : 0;
        if (body is not null && body.Length <= 65_536 &&
            System.Text.RegularExpressions.Regex.Matches(body, @"ACHIEVEMENT_[A-Za-z0-9_]{1,80}")
                .Cast<System.Text.RegularExpressions.Match>().Select(match => match.Value)
                .Distinct(StringComparer.Ordinal).Take(2).SequenceEqual([apiName])) mask |= 2;
        return mask;
    }

    public static int ReadIdentityShape(string? body, string apiName)
    {
        if (body is null || body.Length > 65_536) return 0;
        string[] matches = System.Text.RegularExpressions.Regex.Matches(body,
                @"ACHIEVEMENT_[A-Za-z0-9_]{1,80}")
            .Cast<System.Text.RegularExpressions.Match>().Select(match => match.Value).ToArray();
        string[] unique = matches.Distinct(StringComparer.Ordinal).ToArray();
        int mask = Math.Min(unique.Length, 7);
        if (unique.Contains(apiName, StringComparer.Ordinal)) mask |= 8;
        if (unique.Any(value => value != apiName && value.StartsWith(apiName, StringComparison.Ordinal)))
            mask |= 16;
        if (unique.Any(value => value != apiName && !value.StartsWith(apiName, StringComparison.Ordinal)))
            mask |= 32;
        if (matches.Length > unique.Length) mask |= 64;
        if (body.Contains("Content-Disposition:", StringComparison.OrdinalIgnoreCase)) mask |= 128;
        string decoded;
        try { decoded = Uri.UnescapeDataString(body.Replace('+', ' ')); }
        catch (UriFormatException) { decoded = body; }
        if (decoded.Contains(apiName, StringComparison.Ordinal)) mask |= 256;
        return mask;
    }

    public static int ReadStructure(string? contentType, string? body)
    {
        if (body is null || body.Length > 65_536) return 0;
        int mask = 1;
        if (body.TrimStart().StartsWith('{')) mask |= 2;
        try
        {
            using System.Text.Json.JsonDocument document = System.Text.Json.JsonDocument.Parse(body);
            if (document.RootElement.ValueKind is System.Text.Json.JsonValueKind.Object or
                System.Text.Json.JsonValueKind.Array) mask |= 4;
        }
        catch (System.Text.Json.JsonException) { }
        if (string.IsNullOrEmpty(contentType)) mask |= 8;
        if (contentType?.StartsWith("multipart/form-data", StringComparison.OrdinalIgnoreCase) == true)
            mask |= 16;
        if (contentType?.StartsWith("text/plain", StringComparison.OrdinalIgnoreCase) == true)
            mask |= 32;
        string decoded;
        try { decoded = Uri.UnescapeDataString(body.Replace('+', ' ')).ToLowerInvariant(); }
        catch (UriFormatException) { decoded = body.ToLowerInvariant(); }
        if (decoded.Contains("setid", StringComparison.Ordinal)) mask |= 64;
        if (decoded.Contains("set_id", StringComparison.Ordinal)) mask |= 128;
        if (decoded.Contains("achievementset", StringComparison.Ordinal)) mask |= 256;
        if (decoded.Contains("\"set\"", StringComparison.Ordinal) ||
            decoded.Contains("set=", StringComparison.Ordinal)) mask |= 512;
        if (decoded.Contains("progressstat", StringComparison.Ordinal) ||
            decoded.Contains("progress_stat", StringComparison.Ordinal)) mask |= 1024;
        if (decoded.Contains("permission", StringComparison.Ordinal) ||
            decoded.Contains("setby", StringComparison.Ordinal)) mask |= 2048;
        if (decoded.Contains("hidden", StringComparison.Ordinal) ||
            decoded.Contains("spoiler", StringComparison.Ordinal)) mask |= 4096;
        if (decoded.Contains("description", StringComparison.Ordinal)) mask |= 8192;
        if (decoded.Contains("localization", StringComparison.Ordinal) ||
            decoded.Contains("translation", StringComparison.Ordinal)) mask |= 16384;
        if (decoded.Contains("icon", StringComparison.Ordinal) ||
            decoded.Contains("image", StringComparison.Ordinal)) mask |= 32768;
        return mask;
    }

    public static int Read(string method, Uri uri, string resource, string? contentType,
        string? body, uint appId, string apiName, string desired)
    {
        if (method != "POST" || uri.Scheme != Uri.UriSchemeHttps ||
            uri.IdnHost != "partner.steamgames.com" || uri.Query.Length != 0 ||
            uri.AbsolutePath != $"/achievements/ajaxcreateorupdateachievement/{appId}" ||
            resource is not ("xhr" or "fetch")) return 0;
        int mask = 1;
        if (body is null) return mask;
        mask |= 2;
        if (body.Length > 65_536) return mask;
        mask |= 4;
        if (contentType?.StartsWith("application/json", StringComparison.OrdinalIgnoreCase) == true)
            mask |= 8;
        if (contentType?.StartsWith("application/x-www-form-urlencoded", StringComparison.OrdinalIgnoreCase) == true)
            mask |= 16;
        string decoded;
        try { decoded = Uri.UnescapeDataString(body.Replace('+', ' ')); }
        catch (UriFormatException) { decoded = body; }
        if (decoded.Contains(apiName, StringComparison.Ordinal)) mask |= 32;
        if (decoded.Contains(desired, StringComparison.Ordinal)) mask |= 64;
        if (decoded.Contains(appId.ToString(System.Globalization.CultureInfo.InvariantCulture),
            StringComparison.Ordinal)) mask |= 128;
        string lower = decoded.ToLowerInvariant();
        if (lower.Contains("english", StringComparison.Ordinal)) mask |= 256;
        if (lower.Contains("setid", StringComparison.Ordinal)) mask |= 512;
        if (lower.Contains("progressstat", StringComparison.Ordinal)) mask |= 1024;
        if (lower.Contains("archived", StringComparison.Ordinal)) mask |= 2048;
        if (lower.Contains("icon", StringComparison.Ordinal)) mask |= 4096;
        if (lower.Contains("stat_id", StringComparison.Ordinal)) mask |= 8192;
        return mask;
    }
}
