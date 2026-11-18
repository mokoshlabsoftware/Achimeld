using Achimeld.Core;
using Microsoft.Playwright;

namespace Achimeld.Browser;

// Fixed page-structure facts only. No text, field value, URL, or arbitrary
// selector is returned or retained by the diagnostic sink.
public sealed class SteamworksAchievementPageShape
{
    public uint Flags { get; set; }
    public int Tables { get; set; }
    public int TableRows { get; set; }
    public int GridRows { get; set; }
    public int Frames { get; set; }
    public int ApiTokens { get; set; }
    public int ApiLeaves { get; set; }
    public int AchievementClasses { get; set; }
    public int Images { get; set; }
    public int Buttons { get; set; }
    public int ListItems { get; set; }
    public int SingleTokenCards { get; set; }
    public int CardsWithTwoImages { get; set; }
    public int CardsWithButtons { get; set; }
    public int CardsWithIdText { get; set; }
    public int CardsWithInputs { get; set; }
    public int LargestCardDepth { get; set; }
    public string CardLinePattern { get; set; } = "";
    public int CardPatternMatches { get; set; }
    public uint ExportMask { get; set; }
    public string LinkAction { get; set; } = "none";

    public static async Task<SteamworksAchievementPageShape> ReadAsync(IPage page, SteamAppId appId) =>
        await page.EvaluateAsync<SteamworksAchievementPageShape>(Script, new { appId = appId.Value })
            .ConfigureAwait(false);

    // Switches only the editor's read-only tab. It does not start an export or
    // accept a download; this records fixed control structure for the adapter.
    public async Task ProbeExportControlsAsync(IPage page)
    {
        ILocator tab = page.GetByText("Bulk Import/Export", new() { Exact = true });
        if (await tab.CountAsync().ConfigureAwait(false) != 1) return;
        ExportMask |= 1u;
        await tab.ClickAsync().ConfigureAwait(false);
        ExportMask |= 1u << 1;
        ExportMask |= await page.EvaluateAsync<uint>(ExportScript).ConfigureAwait(false);
    }

    private const string ExportScript = """
        () => {
            let mask = 0;
            for (const [text, offset] of [
                ['Export Definitions (.csv)', 2], ['Export Localization (.csv)', 5],
                ['Export Set Localization (.csv)', 8]]) {
                const match = [...document.querySelectorAll('button,a')]
                    .filter(element => (element.innerText || '').trim() === text);
                if (match.length !== 1) continue;
                const control = match[0];
                mask |= 1 << offset;
                if (control.tagName === 'A') mask |= 1 << (offset + 1);
                if (control.tagName === 'BUTTON') mask |= 1 << (offset + 2);
            }
            return mask;
        }
        """;

    private const string Script = """
        ({appId}) => {
            const body = document.body;
            const text = (body?.innerText || '').slice(0, 200000);
            const tables = document.querySelectorAll('table');
            const tableRows = document.querySelectorAll('table tr').length;
            const gridRows = document.querySelectorAll('[role="grid"] [role="row"]').length;
            const apiPattern = /\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b/g;
            const apiTokens = new Set((text.match(apiPattern) || []).slice(0, 10000));
            let apiLeaves = 0;
            let achievementClasses = 0;
            let singleTokenCards = 0, cardsWithTwoImages = 0, cardsWithButtons = 0;
            let cardsWithIdText = 0, cardsWithInputs = 0, largestCardDepth = 0;
            let cardLinePattern = '', cardPatternMatches = 0;
            for (const element of document.querySelectorAll('*')) {
                if (element.children.length === 0 && /^ACHIEVEMENT_[A-Za-z0-9_]{1,80}$/.test((element.textContent || '').trim())) {
                    apiLeaves++;
                    let card = null, depth = 0;
                    for (let ancestor = element.parentElement; ancestor && depth < 8; ancestor = ancestor.parentElement) {
                        depth++;
                        const names = (ancestor.innerText || '').match(apiPattern) || [];
                        if (new Set(names).size !== 1) break;
                        if (ancestor.querySelectorAll('img').length >= 1 && ancestor.querySelectorAll('button').length >= 1) { card = ancestor; break; }
                    }
                    if (card) {
                        singleTokenCards++;
                        largestCardDepth = Math.max(largestCardDepth, depth);
                        if (card.querySelectorAll('img').length >= 2) cardsWithTwoImages++;
                        if (card.querySelectorAll('button').length >= 1) cardsWithButtons++;
                        if (/\bID\s*[:#]?\s*\d+\b/i.test(card.innerText || '')) cardsWithIdText++;
                        if (card.querySelector('input, textarea, select')) cardsWithInputs++;
                        const pattern = (card.innerText || '').split(/\r?\n/).map(line => line.trim())
                            .filter(Boolean).slice(0, 32).map(line => {
                                if (/^ACHIEVEMENT_[A-Za-z0-9_]{1,80}$/.test(line)) return 'A';
                                if (/\bID\s*[:#]?\s*\d+\b/i.test(line)) return 'I';
                                if (/^\d+(?:\.\d+)?\s*%$/.test(line)) return 'P';
                                if (/^(?:Client|GameServer|OfficialGameServer|Game Server|Official Game Server)$/.test(line)) return 'C';
                                if (/^(?:Visible|Spoiler|Archived|Available)$/.test(line)) return 'V';
                                if (/^(?:Base game|Base Game|Base game achievements)$/.test(line)) return 'S';
                                if (/^\d+$/.test(line)) return 'N';
                                return 'T';
                            }).join('');
                        if (!cardLinePattern) cardLinePattern = pattern;
                        if (pattern === cardLinePattern) cardPatternMatches++;
                    }
                }
                if ([...element.classList].some(name => name.toLowerCase().includes('achievement'))) achievementClasses++;
            }
            let flags = 0;
            if (document.getElementById('achievementTable')) flags |= 1 << 0;
            if (text.includes('API Name')) flags |= 1 << 1;
            if (text.includes('Manage Achievements')) flags |= 1 << 2;
            if (text.includes('Bulk Import/Export')) flags |= 1 << 3;
            if (text.includes('Create Achievement')) flags |= 1 << 4;
            if (text.includes('Display Name')) flags |= 1 << 5;
            if (text.includes('Description')) flags |= 1 << 6;
            if (text.includes('Achievement Configuration')) flags |= 1 << 7;
            if (document.querySelector('[data-achievement-id]')) flags |= 1 << 8;
            if (document.querySelector('#root, #react-root, [data-reactroot]')) flags |= 1 << 9;
            if (tableRows >= 50) flags |= 1 << 10;
            if (gridRows >= 50) flags |= 1 << 11;
            if (document.querySelector('input[value="Edit"]')) flags |= 1 << 12;
            let linkAction = 'none';
            for (const anchor of document.querySelectorAll('a[href]')) {
                let url;
                try { url = new URL(anchor.href, location.href); } catch { continue; }
                if (url.origin !== 'https://partner.steamgames.com' || url.search || url.hash) continue;
                const match = /^\/achievements\/([A-Za-z]{1,32})\/([0-9]{1,10})\/?$/.exec(url.pathname);
                if (match && match[2] === String(appId)) { linkAction = match[1]; flags |= 1 << 13; break; }
            }
            return {Flags: flags, Tables: Math.min(tables.length, 10000),
                TableRows: Math.min(tableRows, 10000), GridRows: Math.min(gridRows, 10000),
                Frames: Math.min(document.querySelectorAll('iframe').length, 10000),
                ApiTokens: Math.min(apiTokens.size, 10000), ApiLeaves: Math.min(apiLeaves, 10000),
                AchievementClasses: Math.min(achievementClasses, 10000),
                Images: Math.min(document.querySelectorAll('img').length, 10000),
                Buttons: Math.min(document.querySelectorAll('button').length, 10000),
                ListItems: Math.min(document.querySelectorAll('[role="listitem"], li').length, 10000),
                SingleTokenCards: Math.min(singleTokenCards, 10000),
                CardsWithTwoImages: Math.min(cardsWithTwoImages, 10000),
                CardsWithButtons: Math.min(cardsWithButtons, 10000),
                CardsWithIdText: Math.min(cardsWithIdText, 10000),
                CardsWithInputs: Math.min(cardsWithInputs, 10000),
                LargestCardDepth: largestCardDepth,
                CardLinePattern: cardLinePattern, CardPatternMatches: Math.min(cardPatternMatches, 10000),
                LinkAction: linkAction};
        }
        """;
}
