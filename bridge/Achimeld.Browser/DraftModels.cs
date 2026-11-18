using Achimeld.Core;

namespace Achimeld.Browser;

public enum PortalEntityKind { Stat, Achievement, Localization, Icon, AchievementSet }
public sealed record PortalWriteTarget(PortalEntityKind Kind, SteamApiName ApiName, LocalizationField? LocalizationField = null);
public sealed record PortalWriteOperation(Guid OperationId, SteamAppId AppId, PortalWriteTarget Target,
    string ExpectedOldFingerprint, string DesiredFingerprint, IReadOnlyDictionary<string, string> Fields,
    IReadOnlyDictionary<string, byte[]> GeneratedUploads, bool Delete = false,
    string? RenameFrom = null, string? ExpectedSteamId = null,
    IReadOnlyDictionary<string, string>? ReviewedAchievementDefinition = null,
    RemoteIconMetadata? ReviewedIconBaseline = null);
