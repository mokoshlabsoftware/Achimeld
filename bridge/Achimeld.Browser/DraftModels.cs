using Achimeld.Core;

namespace Achimeld.Browser;

public enum PortalEntityKind { Stat, Achievement, Localization, Icon }
public sealed record PortalWriteTarget(PortalEntityKind Kind, SteamApiName ApiName, LocalizationField? LocalizationField = null);
public sealed record PortalWriteOperation(Guid OperationId, SteamAppId AppId, PortalWriteTarget Target,
    string ExpectedOldFingerprint, string DesiredFingerprint, IReadOnlyDictionary<string, string> Fields,
    IReadOnlyDictionary<string, byte[]> GeneratedUploads);
