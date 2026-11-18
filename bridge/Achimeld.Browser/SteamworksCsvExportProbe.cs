using System.Text;
using Achimeld.Core;
using Microsoft.Playwright;
using Microsoft.VisualBasic.FileIO;

namespace Achimeld.Browser;

// Reads only the new editor's two fixed, read-only CSV exports. The browser's
// temporary download is deleted after a bounded in-memory read. No CSV value
// reaches the diagnostic sink.
public sealed record SteamworksCsvExportShape(int DefinitionsRows, int LocalizationRows,
    uint DefinitionsHeaderMask, uint LocalizationHeaderMask, uint TransportMask);
public sealed record SteamworksCsvExportData(string DefinitionsCsv, string LocalizationCsv,
    SteamworksCsvExportShape Shape);

public static class SteamworksCsvExportProbe
{
    private const int MaximumBytes = 2 * 1024 * 1024;

    public static async Task<SteamworksCsvExportData> ReadAsync(IPage page, SteamAppId appId,
        Action<bool> armDownload)
    {
        (string definitionsCsv, int definitionRows, uint definitionHeaders, uint definitionTransport) = await ReadOneAsync(
            page, appId, "Export Definitions (.csv)", "achievements-definitions", armDownload)
            .ConfigureAwait(false);
        (string localizationCsv, int localizationRows, uint localizationHeaders, uint localizationTransport) = await ReadOneAsync(
            page, appId, "Export Localization (.csv)", "achievements-localization", armDownload)
            .ConfigureAwait(false);
        return new(definitionsCsv, localizationCsv,
            new(definitionRows, localizationRows, definitionHeaders, localizationHeaders,
                definitionTransport | (localizationTransport << 4)));
    }

    private static async Task<(string Csv, int Rows, uint HeaderMask, uint Transport)> ReadOneAsync(
        IPage page, SteamAppId appId, string controlText, string fileKind, Action<bool> armDownload)
    {
        ILocator control = page.GetByText(controlText, new() { Exact = true });
        if (await control.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("The modern Steamworks export control is unavailable.");
        IDownload download;
        armDownload(true);
        try
        {
            download = await page.RunAndWaitForDownloadAsync(
                () => control.ClickAsync(), new() { Timeout = 15000 }).ConfigureAwait(false);
        }
        finally { armDownload(false); }
        try
        {
            string expected = $"{appId.Value}-{fileKind}.csv";
            if (download.SuggestedFilename != expected ||
                !download.Url.StartsWith("blob:https://partner.steamgames.com/", StringComparison.Ordinal))
                throw new UnsupportedPortalStructureException("Steamworks export identity or transport changed.");
            await using Stream stream = await download.CreateReadStreamAsync().ConfigureAwait(false);
            using MemoryStream buffer = new();
            byte[] chunk = new byte[8192];
            while (true)
            {
                int read = await stream.ReadAsync(chunk).ConfigureAwait(false);
                if (read == 0) break;
                if (buffer.Length + read > MaximumBytes)
                    throw new UnsupportedPortalStructureException("Steamworks export exceeded the read limit.");
                buffer.Write(chunk, 0, read);
            }
            string csv = new UTF8Encoding(false, true).GetString(buffer.ToArray()).TrimStart('\uFEFF');
            (int rows, uint headerMask, uint transport) = Describe(csv, fileKind == "achievements-definitions");
            return (csv, rows, headerMask, transport);
        }
        finally { await download.DeleteAsync().ConfigureAwait(false); }
    }

    private static (int Rows, uint HeaderMask, uint Transport) Describe(string csv, bool definitions)
    {
        using TextFieldParser parser = new(new StringReader(csv));
        parser.SetDelimiters(",");
        parser.HasFieldsEnclosedInQuotes = true;
        string[] headers = parser.ReadFields() ?? [];
        uint mask = 0;
        string[] known = definitions
            ? ["api_name", "setid", "permission", "spoiler", "archived", "progress_stat_name", "progress_stat_min", "progress_stat_max"]
            : ["api_name", "field", "english", "polish", "french", "japanese", "schinese", "tchinese"];
        foreach (string header in headers)
        {
            string normalized = header.Trim().Split('(')[0].Trim().ToLowerInvariant();
            int index = Array.IndexOf(known, normalized);
            if (index >= 0) mask |= 1u << index;
        }
        int rows = 0;
        while (!parser.EndOfData)
        {
            string[]? fields = parser.ReadFields();
            if (fields is null || fields.Length != headers.Length || ++rows > 5000)
                throw new UnsupportedPortalStructureException("Steamworks export CSV shape changed.");
        }
        return (rows, mask, 1u);
    }
}
