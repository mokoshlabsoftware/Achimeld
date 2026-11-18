using Achimeld.Core;
using Microsoft.Playwright;

namespace Achimeld.Browser;

public sealed record ModernAchievementIconPair(string Achieved, string Unachieved);

// The modern editor renders one card per achievement. Only the exact API name
// and its two approved CDN image URLs are used; visible descriptions and page
// script state are not treated as authoritative configuration.
public static class SteamworksModernAchievementCards
{
    private sealed class Card
    {
        public string ApiName { get; set; } = "";
        public Image[] Images { get; set; } = [];
    }

    private sealed class Image
    {
        public string Src { get; set; } = "";
        public string Label { get; set; } = "";
        public string Classes { get; set; } = "";
    }

    public static async Task<(Dictionary<string, ModernAchievementIconPair> Pairs, int EvidenceCount)> ReadAsync(
        IPage page, IReadOnlyDictionary<string, ModernAchievementIconPair> previous)
    {
        Card[] cards = await page.EvaluateAsync<Card[]>(Script).ConfigureAwait(false);
        if (cards.Length is < 1 or > 5000)
            throw new UnsupportedPortalStructureException("The modern Steamworks achievement cards are incomplete.");
        Dictionary<string, ModernAchievementIconPair> pairs = new(StringComparer.Ordinal);
        int firstAchieved = 0, secondAchieved = 0;
        foreach (Card card in cards)
        {
            _ = new SteamApiName(card.ApiName);
            if (card.Images.Length != 2 || pairs.ContainsKey(card.ApiName))
                throw new UnsupportedPortalStructureException("The modern Steamworks achievement cards are ambiguous.");
            foreach (Image image in card.Images)
            {
                if (!Uri.TryCreate(image.Src, UriKind.Absolute, out Uri? uri))
                    throw new UnsupportedPortalStructureException("An achievement icon URL is incomplete.");
                BrowserReadPolicy.RequireIcon(uri);
            }
            int firstRole = Role(card.Images[0]), secondRole = Role(card.Images[1]);
            if (firstRole != 0 && secondRole != 0 && firstRole != secondRole)
            {
                if (firstRole == 1) firstAchieved++;
                else secondAchieved++;
            }
            if (previous.TryGetValue(card.ApiName, out ModernAchievementIconPair? pair) &&
                pair.Achieved != pair.Unachieved && card.Images[0].Src != card.Images[1].Src)
            {
                if (card.Images[0].Src == pair.Achieved && card.Images[1].Src == pair.Unachieved)
                    firstAchieved++;
                else if (card.Images[0].Src == pair.Unachieved && card.Images[1].Src == pair.Achieved)
                    secondAchieved++;
            }
        }
        if (firstAchieved > 0 && secondAchieved > 0 || Math.Max(firstAchieved, secondAchieved) < 10)
            throw new UnsupportedPortalStructureException("Steamworks icon order could not be verified.");
        bool firstIsAchieved = firstAchieved > 0;
        foreach (Card card in cards)
            pairs.Add(card.ApiName, firstIsAchieved
                ? new(card.Images[0].Src, card.Images[1].Src)
                : new(card.Images[1].Src, card.Images[0].Src));
        return (pairs, Math.Max(firstAchieved, secondAchieved));
    }

    private static int Role(Image image)
    {
        string label = (image.Label + " " + image.Classes).ToLowerInvariant();
        if (label.Contains("unachieved", StringComparison.Ordinal)) return 2;
        if (label.Contains("achieved", StringComparison.Ordinal) ||
            label.Contains("unlocked", StringComparison.Ordinal) ||
            label.Contains("earned", StringComparison.Ordinal)) return 1;
        if (label.Contains("locked", StringComparison.Ordinal)) return 2;
        return 0;
    }

    private const string Script = """
        () => {
            const names = /^ACHIEVEMENT_[A-Za-z0-9_]{1,80}$/;
            const cards = [];
            for (const element of document.querySelectorAll('*')) {
                if (element.children.length || !names.test((element.textContent || '').trim())) continue;
                const card = element.parentElement?.parentElement;
                if (!card) continue;
                cards.push({ApiName: (element.textContent || '').trim(),
                    Images: [...card.querySelectorAll('img')].map(image => ({
                        Src: image.currentSrc || image.src || '',
                        Label: [image.alt, image.title, image.getAttribute('aria-label'),
                            image.parentElement?.title, image.parentElement?.getAttribute('aria-label')].filter(Boolean).join(' '),
                        Classes: [image.className, image.parentElement?.className].filter(Boolean).join(' ')
                    }))});
                if (cards.length > 5000) break;
            }
            return cards;
        }
        """;
}
