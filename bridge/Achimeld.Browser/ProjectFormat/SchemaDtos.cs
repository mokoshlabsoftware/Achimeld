namespace Achimeld.Persistence;

public sealed class V1ProjectDto
{
    public string Format { get; set; } = "";
    public int SchemaVersion { get; set; }
    public Guid ProjectId { get; set; }
    public string DisplayName { get; set; } = "";
    public uint SteamAppId { get; set; }
    public string LocalizationCatalogSha256 { get; set; } = "";
    public V1AssetsDto Assets { get; set; } = new();
    public V1AtlasDto Atlas { get; set; } = new();
    public List<V1AchievementDto> Achievements { get; set; } = [];
    public List<V1StatDto> Stats { get; set; } = [];
    public List<V1TombstoneDto> Tombstones { get; set; } = [];
    public V1RemoteSnapshotDto? LastRemoteSnapshot { get; set; }
    public List<V1SyncBaselineDto> SyncBaselines { get; set; } = [];
    public DateTimeOffset? LastVerifiedAtUtc { get; set; }
    public int? NextAchievementInternalId { get; set; }
    public V1CanvasCameraDto? AchievementsCamera { get; set; }
    public V1CanvasCameraDto? StatsCamera { get; set; }
}

public sealed class V1CanvasCameraDto
{
    public double Zoom { get; set; } = 1;
    public double OffsetX { get; set; }
    public double OffsetY { get; set; }
}

public sealed class V1AssetsDto
{
    public V1PathReferenceDto Atlas { get; set; } = new();
    public V1PathReferenceDto LocalizationCatalog { get; set; } = new();
    public List<V1AtlasAssetDto> Atlases { get; set; } = [];
    public List<V1StandaloneImageDto> Images { get; set; } = [];
}

public sealed class V1AtlasAssetDto
{
    public Guid Id { get; set; }
    public string DisplayName { get; set; } = "";
    public V1PathReferenceDto Image { get; set; } = new();
    public int Columns { get; set; }
    public int Rows { get; set; }
}

public sealed class V1StandaloneImageDto
{
    public Guid Id { get; set; }
    public string DisplayName { get; set; } = "";
    public V1PathReferenceDto Image { get; set; } = new();
    public string? Kind { get; set; }
}

public sealed class V1PathReferenceDto
{
    public string Kind { get; set; } = "";
    public string Path { get; set; } = "";
}

public sealed class V1AtlasDto
{
    public int TileWidth { get; set; }
    public int TileHeight { get; set; }
    public int LeftOffset { get; set; }
    public int TopOffset { get; set; }
    public int HorizontalSpacing { get; set; }
    public int VerticalSpacing { get; set; }
    public string Traversal { get; set; } = "";
    public int? OutputSize { get; set; }
    public int? OutputWidth { get; set; }
    public int? OutputHeight { get; set; }
    public V1UnachievedIconDto UnachievedIcon { get; set; } = new();
}

public sealed class V1UnachievedIconDto
{
    public string Mode { get; set; } = "";
    public decimal Opacity { get; set; }
    public string TintHex { get; set; } = "";
}

public sealed class V1AchievementDto
{
    public Guid Id { get; set; }
    public string InternalName { get; set; } = "";
    public string ApiName { get; set; } = "";
    public string DisplayNameLocalizationId { get; set; } = "";
    public string DescriptionLocalizationId { get; set; } = "";
    public bool Hidden { get; set; }
    public string SetBy { get; set; } = "";
    public Guid? ProgressStatId { get; set; }
    public decimal? UnlockValue { get; set; }
    public string? UnlockValueKind { get; set; }
    public int TileIndex { get; set; }
    public int DisplayOrder { get; set; }
    public Guid? AtlasId { get; set; }
    public Guid? StandaloneImageId { get; set; }
    public V1IconRecipeDto? Icon { get; set; }
    public Guid? UnachievedImageId { get; set; }
    public List<V1GeneralImageDto>? GeneralImageEntries { get; set; }
    public Guid? GeneralImageId { get; set; }
    public Guid? GeneralAtlasId { get; set; }
    public int GeneralTileIndex { get; set; }
    public int? InternalId { get; set; }
    public int? CanvasColumn { get; set; }
    public int? CanvasRow { get; set; }
}

public sealed class V1GeneralImageDto
{
    public int Id { get; set; }
    public Guid? ImageId { get; set; }
    public Guid? AtlasId { get; set; }
    public int TileIndex { get; set; }
}

public sealed class V1IconRecipeDto
{
    public string? BackgroundColorHex { get; set; }
    public List<V1IconLayerDto> Layers { get; set; } = [];
}

public sealed class V1IconLayerDto
{
    public Guid? ParentFolderId { get; set; }
    public Guid? MaskLayerId { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    public Guid? MaskSourceLayerId { get; set; }
    public string? MaskMode { get; set; }
    public bool MaskInvert { get; set; }
    public Achimeld.Core.IconPerlinNoiseStyle? PerlinNoise { get; set; }
    public int GeneralImageIndex { get; set; }
    public Guid Id { get; set; }
    public string DisplayName { get; set; } = "";
    public string Kind { get; set; } = "";
    public bool Visible { get; set; } = true;
    public Guid? AtlasId { get; set; }
    public int TileIndex { get; set; }
    public Guid? ImageId { get; set; }
    public V1IconTextDto? Text { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    public Achimeld.Core.IconText3dStyle? Text3d { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    public Achimeld.Core.IconArtifact3dStyle? Artifact3d { get; set; }
    public string? BlendMode { get; set; }
    public string? SolidColorHex { get; set; }
    public Achimeld.Core.IconAdjustmentStyle? Adjustment { get; set; }
    public Achimeld.Core.IconCurvesStyle? Curves { get; set; }
    public Achimeld.Core.IconShapeStyle? Shape { get; set; }
    public V1IconGradientDto? Gradient { get; set; }
    public Achimeld.Core.IconSdfVignetteStyle? SdfVignette { get; set; }
    public Achimeld.Core.IconParticleStyle? Particles { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    public Achimeld.Core.IconExplosionStyle? Explosion { get; set; }
    public double? Scale { get; set; }
    public double? Rotation { get; set; }
    public bool LinkAtlasToGeneral { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(
        Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingDefault)]
    public bool LinkAtlasToAchievement { get; set; }
    [System.Text.Json.Serialization.JsonIgnore(
        Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingDefault)]
    public bool LinkTileIndexToAchievement { get; set; }
    public double? OffsetX { get; set; }
    public double? OffsetY { get; set; }
    public double? Opacity { get; set; }
    public List<V1IconLayerEffectDto> Effects { get; set; } = [];
}

public sealed class V1IconLayerEffectDto
{
    public double SkewX { get; set; }
    public double SkewY { get; set; }
    public double PerspectiveX { get; set; }
    public double PerspectiveY { get; set; }
    public double Amount { get; set; }
    public double Aberration { get; set; }
    public V1IconGradientDto? Gradient { get; set; }
    public bool? AffectOutline { get; set; }
    public double Cutout { get; set; }
    public bool BlurEnabled { get; set; } = true;
    public string BlurColorHex { get; set; } = "#FFFFFFFF";
    public string? BlurBlendMode { get; set; }
    public double BlurOpacity { get; set; } = 1;
    public string? BlendMode { get; set; }
    public double? Opacity { get; set; }
    public Guid Id { get; set; }
    public string Kind { get; set; } = "";
    public bool Visible { get; set; } = true;
    public string ColorHex { get; set; } = "";
    public double Width { get; set; }
    public string? Placement { get; set; }
    public string? OutlineShape { get; set; }
    public double OffsetX { get; set; }
    public double OffsetY { get; set; }
    public double Blur { get; set; }
}

public sealed class V1IconGradientDto
{
    public int VertexCount { get; set; } = 5;
    public double InnerRadius { get; set; } = 0.5;
    public double OuterRounding { get; set; }
    public double InnerRounding { get; set; }
    public string InputChannel { get; set; } = "RGB";
    public string Kind { get; set; } = "";
    public double Angle { get; set; }
    public List<V1IconGradientStopDto> Stops { get; set; } = [];
}

public sealed class V1IconGradientStopDto
{
    public double Offset { get; set; }
    public string ColorHex { get; set; } = "";
}

public sealed class V1IconTextDto
{
    public string Content { get; set; } = "";
    public string FontFamily { get; set; } = "";
    public double FontSize { get; set; }
    public string ColorHex { get; set; } = "";
    public bool OutlineEnabled { get; set; }
    public string OutlineColorHex { get; set; } = "";
    public double OutlineWidth { get; set; }
    public string Alignment { get; set; } = "";
    public string? OutlinePlacement { get; set; }
    public bool LinkToAchievementValue { get; set; }
}

public sealed class V1StatDto
{
    public Guid Id { get; set; }
    public string InternalName { get; set; } = "";
    public string ApiName { get; set; } = "";
    public string DisplayNameLocalizationId { get; set; } = "";
    public string Type { get; set; } = "";
    public string SetBy { get; set; } = "";
    public bool IncrementOnly { get; set; }
    public decimal? Minimum { get; set; }
    public decimal? Maximum { get; set; }
    public decimal? DefaultValue { get; set; }
    public decimal? MaximumChange { get; set; }
    public bool Aggregated { get; set; }
    public decimal? AverageRateWindowSeconds { get; set; }
    public int DisplayOrder { get; set; }
    public int? CanvasColumn { get; set; }
    public int? CanvasRow { get; set; }
}

public sealed class V1TombstoneDto
{
    public string EntityType { get; set; } = "";
    public Guid LocalId { get; set; }
    public string ApiName { get; set; } = "";
    public DateTimeOffset DeletedAtUtc { get; set; }
}

public sealed class V1SyncBaselineDto
{
    public string EntityType { get; set; } = "";
    public Guid LocalId { get; set; }
    public string ApiName { get; set; } = "";
    public SortedDictionary<string, string> FieldFingerprints { get; set; } = new(StringComparer.Ordinal);
    public DateTimeOffset VerifiedAtUtc { get; set; }
    public string? EntityFingerprint { get; set; }
    public string? LocalIconFingerprint { get; set; }
    public string? RemoteIconFingerprint { get; set; }
}

public sealed class V1RemoteSnapshotDto
{
    public uint SteamAppId { get; set; }
    public DateTimeOffset ReadAtUtc { get; set; }
    public string PortalRevisionFingerprint { get; set; } = "";
    public bool SupportsDisplayOrder { get; set; } = true;
    public bool IsComplete { get; set; }
    public List<V1RemoteStatDto> Stats { get; set; } = [];
    public List<V1RemoteAchievementDto> Achievements { get; set; } = [];
    public List<V1RemoteLocalizationDto> Localizations { get; set; } = [];
    public List<V1RemoteIconDto> Icons { get; set; } = [];
}

public sealed class V1RemoteStatDto
{
    public string? SteamId { get; set; }
    public string ApiName { get; set; } = "";
    public string InternalName { get; set; } = "";
    public string Type { get; set; } = "";
    public string SetBy { get; set; } = "";
    public bool IncrementOnly { get; set; }
    public decimal? Minimum { get; set; }
    public decimal? Maximum { get; set; }
    public decimal? DefaultValue { get; set; }
    public decimal? MaximumChange { get; set; }
    public bool Aggregated { get; set; }
    public decimal? AverageRateWindowSeconds { get; set; }
    public int DisplayOrder { get; set; }
    public string ContentFingerprint { get; set; } = "";
}

public sealed class V1RemoteAchievementDto
{
    public string? SteamId { get; set; }
    public string ApiName { get; set; } = "";
    public string InternalName { get; set; } = "";
    public bool Hidden { get; set; }
    public string SetBy { get; set; } = "";
    public string? ProgressStatApiName { get; set; }
    public decimal? UnlockValue { get; set; }
    public int DisplayOrder { get; set; }
    public string ContentFingerprint { get; set; } = "";
}

public sealed class V1RemoteLocalizationDto
{
    public string EntityType { get; set; } = "";
    public string ApiName { get; set; } = "";
    public string Field { get; set; } = "";
    public SortedDictionary<string, string> Values { get; set; } = new(StringComparer.Ordinal);
    public string ContentFingerprint { get; set; } = "";
}

public sealed class V1RemoteIconDto
{
    public string AchievementApiName { get; set; } = "";
    public string? AchievedIdentity { get; set; }
    public string? UnachievedIdentity { get; set; }
    public string? AchievedPreviewUri { get; set; }
    public string? UnachievedPreviewUri { get; set; }
    public string ContentFingerprint { get; set; } = "";
}

public sealed class V0ProjectDto
{
    public string Format { get; set; } = "";
    public int SchemaVersion { get; set; }
    public Guid Id { get; set; }
    public Guid ProjectId { get; set; }
    public string DisplayName { get; set; } = "";
    public uint SteamAppId { get; set; }
    public V1AssetsDto Assets { get; set; } = new();
    public V1AtlasDto Atlas { get; set; } = new();
    public List<V1AchievementDto> Achievements { get; set; } = [];
    public List<V1StatDto> Stats { get; set; } = [];
    public List<V1TombstoneDto> Tombstones { get; set; } = [];
    public V1RemoteSnapshotDto? LastRemoteSnapshot { get; set; }
    public List<V1SyncBaselineDto> SyncBaselines { get; set; } = [];
    public DateTimeOffset? LastVerifiedAtUtc { get; set; }
    public int? NextAchievementInternalId { get; set; }
    public V1CanvasCameraDto? AchievementsCamera { get; set; }
    public V1CanvasCameraDto? StatsCamera { get; set; }
}
