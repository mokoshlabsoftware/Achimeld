using Achimeld.Core;

namespace Achimeld.Persistence;

internal static class V1ProjectMapper
{
    public static V1ProjectDto ToDto(AchimeldProject project, string catalogHash) => new()
    {
        Format = project.Format,
        SchemaVersion = AchimeldProject.CurrentSchemaVersion,
        ProjectId = project.Id.Value,
        DisplayName = project.DisplayName,
        SteamAppId = project.AppId.Value,
        LocalizationCatalogSha256 = catalogHash,
        Assets = new V1AssetsDto
        {
            Atlas = ToDto(project.Assets.Atlas),
            LocalizationCatalog = ToDto(project.Assets.LocalizationCatalog),
            Atlases = project.Assets.Atlases.Select(ToDto).ToList(),
            Images = project.Assets.Images.Select(ToDto).ToList(),
        },
        Atlas = new V1AtlasDto
        {
            TileWidth = project.Atlas.TileWidth,
            TileHeight = project.Atlas.TileHeight,
            LeftOffset = project.Atlas.LeftOffset,
            TopOffset = project.Atlas.TopOffset,
            HorizontalSpacing = project.Atlas.HorizontalSpacing,
            VerticalSpacing = project.Atlas.VerticalSpacing,
            Traversal = project.Atlas.Traversal.ToString(),
            OutputSize = project.Atlas.OutputSize,
            OutputWidth = project.Atlas.OutputWidth,
            OutputHeight = project.Atlas.OutputHeight,
            UnachievedIcon = new V1UnachievedIconDto
            {
                Mode = project.Atlas.EffectiveUnachievedIcon.Mode.ToString(),
                Opacity = project.Atlas.EffectiveUnachievedIcon.Opacity,
                TintHex = project.Atlas.EffectiveUnachievedIcon.TintHex,
            },
        },
        Achievements = project.Achievements.Select(ToDto).ToList(),
        Stats = project.Stats.Select(ToDto).ToList(),
        Tombstones = project.Tombstones.Select(ToDto).ToList(),
        LastRemoteSnapshot = project.LastRemoteSnapshot is null ? null : ToDto(project.LastRemoteSnapshot),
        SyncBaselines = project.SyncBaselines.Select(ToDto).ToList(),
        LastVerifiedAtUtc = project.LastVerifiedAtUtc,
        NextAchievementInternalId = project.NextAchievementInternalId,
        AchievementsCamera = ToDto(project.AchievementsCamera),
        StatsCamera = ToDto(project.StatsCamera),
    };

    public static AchimeldProject FromDto(V1ProjectDto dto)
    {
        AchievementDefinition[] achievements = dto.Achievements.Select(FromDto).ToArray();
        int nextInternalId;
        if (dto.NextAchievementInternalId is int storedNext &&
            dto.Achievements.All(static achievement => achievement.InternalId is not null))
        {
            nextInternalId = storedNext;
        }
        else
        {
            // Projects written before internal ids existed lack both the per-achievement id and the
            // counter, so assign ids in list order to keep the value stable from now on.
            achievements = achievements
                .Select(static (achievement, index) => achievement with { InternalId = index })
                .ToArray();
            nextInternalId = achievements.Length;
        }

        return new(
            RebrandCompatibility.NormalizeProjectFormat(dto.Format),
            AchimeldProject.CurrentSchemaVersion,
            new ProjectId(dto.ProjectId),
            dto.DisplayName,
            new SteamAppId(dto.SteamAppId),
            new ProjectAssets(
                FromDto(dto.Assets.Atlas),
                FromDto(dto.Assets.LocalizationCatalog),
                dto.Assets.Atlases.Select(FromDto).ToArray(),
                dto.Assets.Images.Select(FromDto).ToArray()),
            new AtlasSettings(
                dto.Atlas.TileWidth,
                dto.Atlas.TileHeight,
                dto.Atlas.LeftOffset,
                dto.Atlas.TopOffset,
                dto.Atlas.HorizontalSpacing,
                dto.Atlas.VerticalSpacing,
                ParseEnum<AtlasTraversal>(dto.Atlas.Traversal),
                dto.Atlas.OutputSize,
                new UnachievedIconSettings(
                    ParseEnum<UnachievedIconMode>(dto.Atlas.UnachievedIcon.Mode),
                    dto.Atlas.UnachievedIcon.Opacity,
                    dto.Atlas.UnachievedIcon.TintHex),
                dto.Atlas.OutputWidth,
                dto.Atlas.OutputHeight),
            achievements,
            dto.Stats.Select(FromDto).ToArray(),
            dto.Tombstones.Select(FromDto).ToArray(),
            dto.LastRemoteSnapshot is null ? null : FromDto(dto.LastRemoteSnapshot),
            dto.SyncBaselines.Select(FromDto).ToArray(),
            dto.LastVerifiedAtUtc,
            nextInternalId, FromDto(dto.AchievementsCamera), FromDto(dto.StatsCamera));
    }

    private static V1CanvasCameraDto? ToDto(CanvasCamera? camera)
    {
        if (camera is null) return null;
        camera = camera.Normalize();
        return new() { Zoom = camera.Zoom, OffsetX = camera.OffsetX, OffsetY = camera.OffsetY };
    }

    private static CanvasCamera? FromDto(V1CanvasCameraDto? camera) => camera is null ? null :
        new CanvasCamera(camera.Zoom, camera.OffsetX, camera.OffsetY).Normalize();

    public static V1ProjectDto Migrate(V0ProjectDto dto, string catalogHash) => new()
    {
        Format = dto.Format,
        SchemaVersion = AchimeldProject.CurrentSchemaVersion,
        ProjectId = dto.ProjectId != Guid.Empty ? dto.ProjectId : dto.Id,
        DisplayName = dto.DisplayName,
        SteamAppId = dto.SteamAppId,
        LocalizationCatalogSha256 = catalogHash,
        Assets = dto.Assets,
        Atlas = dto.Atlas,
        Achievements = dto.Achievements,
        Stats = dto.Stats,
        Tombstones = dto.Tombstones,
        LastRemoteSnapshot = dto.LastRemoteSnapshot,
        SyncBaselines = dto.SyncBaselines,
        LastVerifiedAtUtc = dto.LastVerifiedAtUtc,
        NextAchievementInternalId = dto.NextAchievementInternalId,
    };

    private static V1PathReferenceDto ToDto(PathReference value) => new()
    {
        Kind = value.Kind.ToString(),
        Path = value.Path,
    };

    private static PathReference FromDto(V1PathReferenceDto value) =>
        new(ParseEnum<PathReferenceKind>(value.Kind), value.Path);

    private static V1AtlasAssetDto ToDto(AtlasDefinition value) => new()
    {
        Id = value.Id.Value,
        DisplayName = value.DisplayName,
        Image = ToDto(value.Image),
        Columns = value.Columns,
        Rows = value.Rows,
    };

    private static AtlasDefinition FromDto(V1AtlasAssetDto value) => new(
        new AtlasId(value.Id),
        value.DisplayName,
        FromDto(value.Image),
        value.Columns,
        value.Rows);

    private static V1StandaloneImageDto ToDto(StandaloneImageDefinition value) => new()
    {
        Id = value.Id.Value,
        DisplayName = value.DisplayName,
        Image = ToDto(value.Image),
        Kind = value.Kind == ImageAssetKind.User ? null : value.Kind.ToString(),
    };

    private static StandaloneImageDefinition FromDto(V1StandaloneImageDto value) => new(
        new ImageAssetId(value.Id),
        value.DisplayName,
        FromDto(value.Image),
        string.IsNullOrWhiteSpace(value.Kind)
            ? ImageAssetKind.User
            : ParseEnum<ImageAssetKind>(value.Kind));

    private static V1AchievementDto ToDto(AchievementDefinition value) => new()
    {
        Id = value.Id.Value,
        // There is no separate internal name; the Steam API name is the single identity. The legacy
        // slot stays populated so older project files and readers keep loading.
        InternalName = value.ApiName.Value,
        ApiName = value.ApiName.Value,
        DisplayNameLocalizationId = value.DisplayNameLocalizationId.Value,
        DescriptionLocalizationId = value.DescriptionLocalizationId.Value,
        Hidden = value.Hidden,
        SetBy = value.SetBy.ToString(),
        ProgressStatId = value.ProgressStatId?.Value,
        UnlockValue = value.UnlockValue,
        UnlockValueKind = value.UnlockValueKind.ToString(),
        TileIndex = value.TileIndex,
        DisplayOrder = value.DisplayOrder,
        AtlasId = value.AtlasId?.Value,
        StandaloneImageId = value.StandaloneImageId?.Value,
        Icon = ToDto(value.Icon, value.TileIndex, value.AtlasId, value.StandaloneImageId),
        UnachievedImageId = value.UnachievedImageId?.Value,
        GeneralImageEntries = value.GeneralImageEntries?.Select(entry => new V1GeneralImageDto { Id = entry.Id, ImageId = entry.ImageId?.Value, AtlasId = entry.AtlasId?.Value, TileIndex = entry.TileIndex }).ToList(),
        GeneralImageId = value.GeneralImageId?.Value,
        GeneralAtlasId = value.GeneralAtlasId?.Value,
        GeneralTileIndex = value.GeneralTileIndex,
        InternalId = value.InternalId,
        CanvasColumn = value.Canvas?.Column,
        CanvasRow = value.Canvas?.Row,
    };

    private static AchievementDefinition FromDto(V1AchievementDto value)
    {
        AtlasId? atlasId = value.AtlasId is Guid atlas ? new AtlasId(atlas) : null;
        ImageAssetId? imageId = value.StandaloneImageId is Guid image ? new ImageAssetId(image) : null;
        return new(
            new AchievementId(value.Id),
            new SteamApiName(value.ApiName),
            new LocalizationId(value.DisplayNameLocalizationId),
            new LocalizationId(value.DescriptionLocalizationId),
            value.Hidden,
            ParseEnum<SetBy>(value.SetBy),
            value.ProgressStatId is Guid id ? new StatId(id) : null,
            value.UnlockValue,
            value.TileIndex,
            value.DisplayOrder,
            atlasId,
            imageId,
            FromDto(value.Icon, value.TileIndex, atlasId, imageId),
            value.UnachievedImageId is Guid lockedImage ? new ImageAssetId(lockedImage) : null,
            value.GeneralImageId is Guid generalImage ? new ImageAssetId(generalImage) : null,
            ResolveUnlockValueKind(value),
            value.GeneralAtlasId is Guid generalAtlas ? new AtlasId(generalAtlas) : null,
            value.GeneralTileIndex,
            value.InternalId ?? 0,
            ToCanvas(value.CanvasColumn, value.CanvasRow),
            value.GeneralImageEntries?.Select(entry => new GeneralImageDefinition(entry.Id,
                entry.ImageId is Guid imageEntry ? new ImageAssetId(imageEntry) : null,
                entry.AtlasId is Guid atlasEntry ? new AtlasId(atlasEntry) : null, entry.TileIndex)).ToArray());
    }

    private static V1IconRecipeDto? ToDto(
        IconRecipe? recipe,
        int tileIndex,
        AtlasId? atlasId,
        ImageAssetId? standaloneImageId)
    {
        if (recipe is null || recipe.IsSimpleLegacy(tileIndex, atlasId, standaloneImageId))
        {
            return null;
        }

        return new V1IconRecipeDto
        {
            BackgroundColorHex = string.IsNullOrWhiteSpace(recipe.BackgroundColorHex)
                ? null
                : recipe.BackgroundColorHex,
            Layers = recipe.Layers.Select(ToDto).ToList(),
        };
    }

    private static IconRecipe? FromDto(
        V1IconRecipeDto? value,
        int tileIndex,
        AtlasId? atlasId,
        ImageAssetId? standaloneImageId)
    {
        if (value is null || value.Layers.Count == 0)
        {
            return null;
        }

        IconRecipe recipe = new(
            value.Layers.Select(FromDto).ToArray(),
            string.IsNullOrWhiteSpace(value.BackgroundColorHex) ? null : value.BackgroundColorHex);
        return recipe.IsSimpleLegacy(tileIndex, atlasId, standaloneImageId) ? null : recipe;
    }

    private static V1IconLayerDto ToDto(IconLayer value) => new()
    {
        Id = value.Id.Value,
        DisplayName = value.DisplayName,
        Kind = value.Kind.ToString(),
        Visible = value.Visible,
        AtlasId = value.AtlasId?.Value,
        TileIndex = value.TileIndex,
        ImageId = value.ImageId?.Value,
        ParentFolderId = value.ParentFolderId?.Value,
        MaskLayerId = value.MaskLayerId?.Value,
        MaskSourceLayerId = value.MaskSourceLayerId?.Value,
        MaskMode = value.MaskMode == IconMaskMode.Alpha ? null : value.MaskMode.ToString(),
        MaskInvert = value.MaskInvert,
        PerlinNoise = value.PerlinNoise,
        Text = value.Text is null ? null : ToDto(value.Text),
        Text3d = value.Text3d,
        Artifact3d = value.Artifact3d,
        BlendMode = value.BlendMode == IconBlendMode.Normal ? null : value.BlendMode.ToString(),
        SolidColorHex = value.SolidColorHex,
        Gradient = value.Gradient is null ? null : ToDto(value.Gradient),
        SdfVignette = value.SdfVignette,
        Particles = value.Particles,
        Explosion = value.Explosion,
        Shape = value.Shape,
        Curves = value.Curves,
        Adjustment = value.Adjustment,
        Scale = value.Scale == 1 ? null : value.Scale,
        Rotation = value.Rotation == 0 ? null : value.Rotation,
        GeneralImageIndex = value.GeneralImageIndex,
        LinkAtlasToGeneral = value.LinkAtlasToGeneral,
        OffsetX = value.OffsetX == 0 ? null : value.OffsetX,
        OffsetY = value.OffsetY == 0 ? null : value.OffsetY,
        Opacity = value.Opacity == 1 ? null : value.Opacity,
        Effects = value.EffectiveEffects.Select(ToDto).ToList(),
    };

    private static IconLayer FromDto(V1IconLayerDto value) => new(
        value.Id == Guid.Empty ? IconLayerId.New() : new IconLayerId(value.Id),
        value.DisplayName,
        ParseEnum<IconLayerKind>(value.Kind),
        value.Visible,
        value.AtlasId is Guid atlasId ? new AtlasId(atlasId) : null,
        value.TileIndex,
        value.ImageId is Guid imageId ? new ImageAssetId(imageId) : null,
        value.Text is null ? null : FromDto(value.Text),
        string.IsNullOrWhiteSpace(value.BlendMode)
            ? IconBlendMode.Normal
            : ParseEnum<IconBlendMode>(value.BlendMode),
        string.IsNullOrWhiteSpace(value.SolidColorHex) ? null : value.SolidColorHex,
        value.Gradient is null ? null : FromDto(value.Gradient),
        value.Scale is > 0 ? value.Scale.Value : 1,
        value.LinkAtlasToGeneral || value.LinkAtlasToAchievement || value.LinkTileIndexToAchievement,
        value.OffsetX ?? 0,
        value.OffsetY ?? 0,
        value.Opacity is >= 0 and <= 1 ? value.Opacity.Value : 1,
        value.Effects.Select(FromDto).ToArray(),
        value.GeneralImageIndex, value.SdfVignette,
        value.ParentFolderId is Guid parent ? new IconLayerId(parent) : null,
        value.MaskLayerId is Guid mask ? new IconLayerId(mask) : null, value.Shape, value.Curves, value.Adjustment, value.Rotation ?? 0,
        value.PerlinNoise, string.IsNullOrWhiteSpace(value.MaskMode) ? IconMaskMode.Alpha : ParseEnum<IconMaskMode>(value.MaskMode), value.MaskInvert, value.Text3d, value.Particles, value.Explosion, value.Artifact3d,
        value.MaskSourceLayerId is Guid maskSource ? new IconLayerId(maskSource) : null);

    private static V1IconLayerEffectDto ToDto(IconLayerEffect value) => new()
    {
        SkewX = value.SkewX,
        SkewY = value.SkewY,
        PerspectiveX = value.PerspectiveX,
        PerspectiveY = value.PerspectiveY,
        Amount = value.Amount,
        Aberration = value.Aberration,
        Id = value.Id.Value,
        Kind = value.Kind.ToString(),
        Visible = value.Visible,
        ColorHex = value.ColorHex,
        Width = value.Width,
        Placement = value.Placement == IconOutlinePlacement.Center ? null : value.Placement.ToString(),
        OutlineShape = value.OutlineShape == IconOutlineShape.Square ? null : value.OutlineShape.ToString(),
        OffsetX = value.OffsetX,
        OffsetY = value.OffsetY,
        Blur = value.Blur,
        BlurEnabled = value.BlurEnabled,
        BlurColorHex = value.BlurColorHex,
        BlurBlendMode = value.BlurBlendMode.ToString(),
        BlurOpacity = value.BlurOpacity,
        AffectOutline = value.AffectOutline,
        Cutout = value.Cutout,
        Gradient = value.Gradient is null ? null : ToDto(value.Gradient),
        BlendMode = value.BlendMode == IconBlendMode.Normal ? null : value.BlendMode.ToString(),
        Opacity = value.Opacity == 1 ? null : value.Opacity,
    };

    private static IconLayerEffect FromDto(V1IconLayerEffectDto value) => new(
        value.Id == Guid.Empty ? IconLayerEffectId.New() : new IconLayerEffectId(value.Id),
        ParseEnum<IconLayerEffectKind>(value.Kind),
        value.Visible,
        string.IsNullOrWhiteSpace(value.ColorHex) ? "#FF000000" : value.ColorHex,
        value.Width,
        string.IsNullOrWhiteSpace(value.Placement)
            ? IconOutlinePlacement.Center
            : ParseEnum<IconOutlinePlacement>(value.Placement),
        value.OffsetX,
        value.OffsetY,
        value.Blur,
        string.IsNullOrWhiteSpace(value.BlendMode) ? IconBlendMode.Normal : ParseEnum<IconBlendMode>(value.BlendMode),
        value.Opacity is >= 0 and <= 1 ? value.Opacity.Value : 1, value.BlurEnabled, value.BlurColorHex,
        string.IsNullOrWhiteSpace(value.BlurBlendMode) ? IconBlendMode.Screen : ParseEnum<IconBlendMode>(value.BlurBlendMode),
        double.IsFinite(value.BlurOpacity) ? Math.Clamp(value.BlurOpacity, 0, 1) : 1,
        string.IsNullOrWhiteSpace(value.OutlineShape) ? IconOutlineShape.Square : ParseEnum<IconOutlineShape>(value.OutlineShape),
        value.AffectOutline ?? ParseEnum<IconLayerEffectKind>(value.Kind) == IconLayerEffectKind.BoxBlur,
        double.IsFinite(value.Cutout) ? Math.Clamp(value.Cutout, 0, 256) : 0, value.Gradient is null ? null : FromDto(value.Gradient),
        value.SkewX, value.SkewY, value.PerspectiveX, value.PerspectiveY, value.Amount, value.Aberration);

    private static V1IconGradientDto ToDto(IconGradientStyle value) => new()
    {
        VertexCount = value.VertexCount,
        InnerRadius = value.InnerRadius,
        OuterRounding = value.OuterRounding,
        InnerRounding = value.InnerRounding,
        InputChannel = value.InputChannel.ToString(),
        Kind = value.Kind.ToString(),
        Angle = value.Angle,
        Stops = value.Stops.Select(stop => new V1IconGradientStopDto
        {
            Offset = stop.Offset,
            ColorHex = stop.ColorHex,
        }).ToList(),
    };

    private static IconGradientStyle FromDto(V1IconGradientDto value) => new(
        string.IsNullOrWhiteSpace(value.Kind)
            ? IconGradientKind.Linear
            : ParseEnum<IconGradientKind>(value.Kind),
        value.Angle,
        value.Stops.Select(stop => new IconGradientStop(stop.Offset, stop.ColorHex)).ToArray(),
        string.IsNullOrWhiteSpace(value.InputChannel) ? IconGradientInputChannel.RGB : ParseEnum<IconGradientInputChannel>(value.InputChannel),
        Math.Clamp(value.VertexCount, 3, 64),
        double.IsFinite(value.InnerRadius) ? Math.Clamp(value.InnerRadius, .01, 1) : .5,
        double.IsFinite(value.OuterRounding) ? Math.Clamp(value.OuterRounding, 0, 1) : 0,
        double.IsFinite(value.InnerRounding) ? Math.Clamp(value.InnerRounding, 0, 1) : 0);

    private static V1IconTextDto ToDto(IconTextStyle value) => new()
    {
        Content = value.Content,
        FontFamily = value.FontFamily,
        FontSize = value.FontSize,
        ColorHex = value.ColorHex,
        OutlineEnabled = value.OutlineEnabled,
        OutlineColorHex = value.OutlineColorHex,
        OutlineWidth = value.OutlineWidth,
        Alignment = value.Alignment.ToString(),
        OutlinePlacement = value.OutlinePlacement == IconOutlinePlacement.Center
            ? null
            : value.OutlinePlacement.ToString(),
        LinkToAchievementValue = value.LinkToAchievementValue,
    };

    private static IconTextStyle FromDto(V1IconTextDto value) => new(
        value.Content,
        string.IsNullOrWhiteSpace(value.FontFamily) ? "Segoe UI" : value.FontFamily,
        value.FontSize <= 0 ? IconTextStyle.DefaultFontSize : value.FontSize,
        string.IsNullOrWhiteSpace(value.ColorHex) ? "#FFFFFF" : value.ColorHex,
        value.OutlineEnabled,
        string.IsNullOrWhiteSpace(value.OutlineColorHex) ? "#000000" : value.OutlineColorHex,
        value.OutlineWidth,
        string.IsNullOrWhiteSpace(value.Alignment)
            ? IconTextAlignment.BottomRight
            : ParseEnum<IconTextAlignment>(value.Alignment),
        string.IsNullOrWhiteSpace(value.OutlinePlacement)
            ? IconOutlinePlacement.Center
            : ParseEnum<IconOutlinePlacement>(value.OutlinePlacement),
        value.LinkToAchievementValue);

    private static V1StatDto ToDto(StatDefinition value) => new()
    {
        Id = value.Id.Value,
        // Legacy slot: mirrors the API name because there is no separate internal name.
        InternalName = value.ApiName.Value,
        ApiName = value.ApiName.Value,
        DisplayNameLocalizationId = value.DisplayNameLocalizationId.Value,
        Type = value.Type.ToString(),
        SetBy = value.SetBy.ToString(),
        IncrementOnly = value.IncrementOnly,
        Minimum = value.Minimum,
        Maximum = value.Maximum,
        DefaultValue = value.DefaultValue,
        MaximumChange = value.MaximumChange,
        Aggregated = value.Aggregated,
        AverageRateWindowSeconds = value.AverageRateWindowSeconds,
        DisplayOrder = value.DisplayOrder,
        CanvasColumn = value.Canvas?.Column,
        CanvasRow = value.Canvas?.Row,
    };

    private static StatDefinition FromDto(V1StatDto value) => new(
        new StatId(value.Id),
        new SteamApiName(value.ApiName),
        new LocalizationId(value.DisplayNameLocalizationId),
        ParseEnum<StatType>(value.Type),
        ParseEnum<SetBy>(value.SetBy),
        value.IncrementOnly,
        value.Minimum,
        value.Maximum,
        value.DefaultValue,
        value.MaximumChange,
        value.Aggregated,
        value.AverageRateWindowSeconds,
        value.DisplayOrder,
        ToCanvas(value.CanvasColumn, value.CanvasRow));

    private static CanvasPosition? ToCanvas(int? column, int? row) =>
        column is int x && row is int y ? new CanvasPosition(x, y) : null;

    private static V1TombstoneDto ToDto(RemoteTombstone value) => new()
    {
        EntityType = value.EntityType.ToString(),
        LocalId = value.LocalId,
        ApiName = value.ApiName.Value,
        DeletedAtUtc = value.DeletedAtUtc,
    };

    private static RemoteTombstone FromDto(V1TombstoneDto value) => new(
        ParseEnum<ProjectEntityType>(value.EntityType),
        value.LocalId,
        new SteamApiName(value.ApiName),
        value.DeletedAtUtc);

    private static V1SyncBaselineDto ToDto(FieldSyncBaseline value) => new()
    {
        EntityType = value.EntityType.ToString(),
        LocalId = value.LocalId,
        ApiName = value.ApiName.Value,
        FieldFingerprints = new SortedDictionary<string, string>(
            value.FieldFingerprints.ToDictionary(static pair => pair.Key, static pair => pair.Value),
            StringComparer.Ordinal),
        VerifiedAtUtc = value.VerifiedAtUtc,
        EntityFingerprint = value.EntityFingerprint,
        LocalIconFingerprint = value.Icons?.LocalFingerprint,
        RemoteIconFingerprint = value.Icons?.RemoteFingerprint,
    };

    private static FieldSyncBaseline FromDto(V1SyncBaselineDto value) => new(
        ParseEnum<ProjectEntityType>(value.EntityType),
        value.LocalId,
        new SteamApiName(value.ApiName),
        new Dictionary<string, string>(value.FieldFingerprints, StringComparer.Ordinal),
        value.VerifiedAtUtc)
    {
        EntityFingerprint = value.EntityFingerprint,
        Icons = value.LocalIconFingerprint is { } local && value.RemoteIconFingerprint is { } remote
            ? new IconSyncBaseline(local, remote) : null,
    };

    private static V1RemoteSnapshotDto ToDto(RemoteSnapshot value) => new()
    {
        SteamAppId = value.AppId.Value,
        ReadAtUtc = value.ReadAtUtc,
        PortalRevisionFingerprint = value.PortalRevisionFingerprint,
        SupportsDisplayOrder = value.SupportsDisplayOrder,
        IsComplete = value.IsComplete,
        Stats = value.Stats.Select(ToDto).ToList(),
        Achievements = value.Achievements.Select(ToDto).ToList(),
        Localizations = value.Localizations.Select(ToDto).ToList(),
        Icons = value.Icons.Select(ToDto).ToList(),
    };

    private static RemoteSnapshot FromDto(V1RemoteSnapshotDto value) => new(
        new SteamAppId(value.SteamAppId),
        value.ReadAtUtc,
        value.PortalRevisionFingerprint,
        value.IsComplete,
        value.Stats.Select(FromDto).ToArray(),
        value.Achievements.Select(FromDto).ToArray(),
        value.Localizations.Select(FromDto).ToArray(),
        value.Icons.Select(FromDto).ToArray()) { SupportsDisplayOrder = value.SupportsDisplayOrder };

    private static V1RemoteStatDto ToDto(RemoteStat value) => new()
    {
        SteamId = value.SteamId,
        ApiName = value.ApiName.Value,
        InternalName = value.ApiName.Value,
        Type = value.Type.ToString(),
        SetBy = value.SetBy.ToString(),
        IncrementOnly = value.IncrementOnly,
        Minimum = value.Minimum,
        Maximum = value.Maximum,
        DefaultValue = value.DefaultValue,
        MaximumChange = value.MaximumChange,
        Aggregated = value.Aggregated,
        AverageRateWindowSeconds = value.AverageRateWindowSeconds,
        DisplayOrder = value.DisplayOrder,
        ContentFingerprint = value.ContentFingerprint,
    };

    private static RemoteStat FromDto(V1RemoteStatDto value) => new(
        value.SteamId,
        new SteamApiName(value.ApiName),
        ParseEnum<StatType>(value.Type),
        ParseEnum<SetBy>(value.SetBy),
        value.IncrementOnly,
        value.Minimum,
        value.Maximum,
        value.DefaultValue,
        value.MaximumChange,
        value.Aggregated,
        value.AverageRateWindowSeconds,
        value.DisplayOrder,
        value.ContentFingerprint);

    private static V1RemoteAchievementDto ToDto(RemoteAchievement value) => new()
    {
        SteamId = value.SteamId,
        ApiName = value.ApiName.Value,
        InternalName = value.ApiName.Value,
        Hidden = value.Hidden,
        SetBy = value.SetBy.ToString(),
        ProgressStatApiName = value.ProgressStatApiName?.Value,
        UnlockValue = value.UnlockValue,
        DisplayOrder = value.DisplayOrder,
        ContentFingerprint = value.ContentFingerprint,
    };

    private static RemoteAchievement FromDto(V1RemoteAchievementDto value) => new(
        value.SteamId,
        new SteamApiName(value.ApiName),
        value.Hidden,
        ParseEnum<SetBy>(value.SetBy),
        value.ProgressStatApiName is null ? null : new SteamApiName(value.ProgressStatApiName),
        value.UnlockValue,
        value.DisplayOrder,
        value.ContentFingerprint);

    private static V1RemoteLocalizationDto ToDto(RemoteLocalization value) => new()
    {
        EntityType = value.EntityType.ToString(),
        ApiName = value.ApiName.Value,
        Field = value.Field.ToString(),
        Values = new SortedDictionary<string, string>(
            value.Values.ToDictionary(static pair => pair.Key, static pair => pair.Value),
            StringComparer.Ordinal),
        ContentFingerprint = value.ContentFingerprint,
    };

    private static RemoteLocalization FromDto(V1RemoteLocalizationDto value) => new(
        ParseEnum<ProjectEntityType>(value.EntityType),
        new SteamApiName(value.ApiName),
        ParseEnum<LocalizationField>(value.Field),
        new Dictionary<string, string>(value.Values, StringComparer.Ordinal),
        value.ContentFingerprint);

    private static V1RemoteIconDto ToDto(RemoteIconMetadata value) => new()
    {
        AchievementApiName = value.AchievementApiName.Value,
        AchievedIdentity = value.AchievedIdentity,
        UnachievedIdentity = value.UnachievedIdentity,
        AchievedPreviewUri = value.AchievedPreviewUri?.OriginalString,
        UnachievedPreviewUri = value.UnachievedPreviewUri?.OriginalString,
        ContentFingerprint = value.ContentFingerprint,
    };

    private static RemoteIconMetadata FromDto(V1RemoteIconDto value) => new(
        new SteamApiName(value.AchievementApiName),
        value.AchievedIdentity,
        value.UnachievedIdentity,
        ParseUri(value.AchievedPreviewUri),
        ParseUri(value.UnachievedPreviewUri),
        value.ContentFingerprint);

    private static Uri? ParseUri(string? value) =>
        value is null ? null : new Uri(value, UriKind.Absolute);

    private static UnlockValueKind ResolveUnlockValueKind(V1AchievementDto value)
    {
        if (string.IsNullOrWhiteSpace(value.UnlockValueKind))
        {
            return value.UnlockValue is decimal number && decimal.Truncate(number) != number
                ? UnlockValueKind.Float
                : UnlockValueKind.Integer;
        }

        return ParseEnum<UnlockValueKind>(value.UnlockValueKind);
    }

    private static T ParseEnum<T>(string value) where T : struct, Enum =>
        Enum.TryParse(value, ignoreCase: false, out T parsed) && Enum.IsDefined(parsed)
            ? parsed
            : throw new ProjectDataException($"Value '{value}' is not valid for {typeof(T).Name}.");
}
