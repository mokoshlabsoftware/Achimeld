using System.Globalization;

namespace Achimeld.Core;

/// <summary>Resolves achievement unlock tokens for presentation and synchronization, preserving source templates.</summary>
public static class AchievementText
{
    public static string Resolve(string template, decimal? unlockValue) => unlockValue is decimal value
        ? template.Replace("{0}", value.ToString("0.############################", CultureInfo.InvariantCulture), StringComparison.Ordinal)
        : template;

    public static ProjectLocalizationRow Resolve(ProjectLocalizationRow row, decimal? unlockValue) => row with
    {
        English = Resolve(row.English, unlockValue),
        Polish = Resolve(row.Polish, unlockValue),
        ChineseSimplified = Resolve(row.ChineseSimplified, unlockValue),
        ChineseTraditional = Resolve(row.ChineseTraditional, unlockValue),
        Japanese = Resolve(row.Japanese, unlockValue),
        Korean = Resolve(row.Korean, unlockValue),
        French = Resolve(row.French, unlockValue),
    };
}
