using Achimeld.Core;

namespace Achimeld.Browser;

public static class SynchronizationSnapshotPolicy
{
    private static readonly string[] Languages = ["English", "Polish", "ChineseSimplified", "ChineseTraditional", "Japanese", "Korean", "French"];
    public static void Validate(RemoteSnapshot snapshot, SteamAppId appId)
    {
        if (!snapshot.IsComplete || snapshot.AppId != appId || snapshot.Stats.Count + snapshot.Achievements.Count > 10_000) throw Rejected();
        HashSet<string> stats = Unique(snapshot.Stats.Select(s => s.ApiName.Value)), achievements = Unique(snapshot.Achievements.Select(a => a.ApiName.Value));
        foreach (RemoteStat stat in snapshot.Stats)
            if (!Enum.IsDefined(stat.Type) || !Enum.IsDefined(stat.SetBy) || stat.ContentFingerprint != CanonicalFingerprint.For(stat, snapshot.SupportsDisplayOrder).EntityFingerprint) throw Rejected();
        foreach (RemoteAchievement achievement in snapshot.Achievements)
            if (!Enum.IsDefined(achievement.SetBy) || achievement.ProgressStatApiName is SteamApiName progress && !stats.Contains(progress.Value) ||
                achievement.ContentFingerprint != CanonicalFingerprint.For(achievement, snapshot.SupportsDisplayOrder).EntityFingerprint) throw Rejected();
        HashSet<(ProjectEntityType, string, LocalizationField)> localized = [];
        foreach (RemoteLocalization text in snapshot.Localizations)
        {
            bool stat = text.EntityType == ProjectEntityType.Stat;
            if (!Enum.IsDefined(text.EntityType) || !Enum.IsDefined(text.Field) ||
                !(stat ? stats : achievements).Contains(text.ApiName.Value) || stat != (text.Field == LocalizationField.StatDisplayName) ||
                !localized.Add((text.EntityType, text.ApiName.Value.ToUpperInvariant(), text.Field)) || text.Values.Count > Languages.Length ||
                text.Values.Any(p => !Languages.Contains(p.Key) || p.Value.Length > 1_048_576) ||
                text.ContentFingerprint != CanonicalFingerprint.Compute(Languages.Select(l => new CanonicalField(l, text.Values.GetValueOrDefault(l, "")))).EntityFingerprint) throw Rejected();
        }
        Unique(snapshot.Icons.Select(i => i.AchievementApiName.Value));
        foreach (RemoteIconMetadata icon in snapshot.Icons)
        {
            if (!achievements.Contains(icon.AchievementApiName.Value)) throw Rejected();
            foreach (var pair in new[] { (icon.AchievedIdentity, icon.AchievedPreviewUri), (icon.UnachievedIdentity, icon.UnachievedPreviewUri) })
            {
                if ((pair.Item1 is null) != (pair.Item2 is null)) throw Rejected();
                if (pair.Item2 is Uri uri)
                {
                    BrowserReadPolicy.RequireIcon(uri);
                    if (pair.Item1!.Length != 64 || !pair.Item1.All(char.IsAsciiHexDigit)) throw Rejected();
                }
            }
            if (icon.ContentFingerprint != CanonicalFingerprint.Compute([new("achievedIdentity", icon.AchievedIdentity), new("unachievedIdentity", icon.UnachievedIdentity)]).EntityFingerprint) throw Rejected();
        }
    }
    private static HashSet<string> Unique(IEnumerable<string> names)
    {
        HashSet<string> result = new(StringComparer.Ordinal), identities = new(StringComparer.OrdinalIgnoreCase);
        foreach (string name in names) { if (string.IsNullOrEmpty(name) || !identities.Add(name)) throw Rejected(); result.Add(name); }
        return result;
    }
    private static InvalidDataException Rejected() => new("The synchronization snapshot has inconsistent identities, ownership or fingerprints.");
}
