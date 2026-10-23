using System.Collections.ObjectModel;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Text.RegularExpressions;

namespace Achimeld.Core;

public readonly record struct SteamAppId
{
    public SteamAppId(uint value)
    {
        ArgumentOutOfRangeException.ThrowIfZero(value);
        Value = value;
    }

    public uint Value { get; }
    public override string ToString() => Value.ToString(CultureInfo.InvariantCulture);
}

public readonly partial record struct SteamApiName
{
    public const int MaximumLength = 128;

    public SteamApiName(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        value = value.Trim();
        if (value.Length is < 1 or > MaximumLength || !ValidName().IsMatch(value))
        {
            throw new ArgumentException(
                $"A Steam API name must be 1-{MaximumLength} ASCII letters, digits, underscores, hyphens, periods, or colons.",
                nameof(value));
        }

        Value = value;
    }

    public string Value { get; }
    public override string ToString() => Value ?? string.Empty;

    [GeneratedRegex(@"^[A-Za-z0-9_.:-]+$", RegexOptions.CultureInvariant)]
    private static partial Regex ValidName();
}

public readonly partial record struct LocalizationId
{
    public LocalizationId(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (!ValidId().IsMatch(value))
        {
            throw new ArgumentException("A localization ID must contain exactly six Base62 characters.", nameof(value));
        }

        Value = value;
    }

    public string Value { get; }
    public override string ToString() => Value ?? string.Empty;

    [GeneratedRegex("^[0-9A-Za-z]{6}$", RegexOptions.CultureInvariant)]
    private static partial Regex ValidId();
}

public enum PathReferenceKind { RelativeToProject, Absolute }

public readonly record struct PathReference
{
    public PathReference(PathReferenceKind kind, string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        path = path.Trim();
        bool rooted = System.IO.Path.IsPathRooted(path);
        if (kind == PathReferenceKind.RelativeToProject && rooted)
        {
            throw new ArgumentException("A project-relative path cannot be rooted.", nameof(path));
        }

        if (kind == PathReferenceKind.Absolute && !rooted)
        {
            throw new ArgumentException("An absolute path must be rooted.", nameof(path));
        }

        Kind = kind;
        Path = path;
    }

    public PathReferenceKind Kind { get; }
    public string Path { get; }

    public static PathReference RelativeToProject(string path) => new(PathReferenceKind.RelativeToProject, path);
    public static PathReference Absolute(string path) => new(PathReferenceKind.Absolute, path);

    public string Resolve(string projectFilePath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(projectFilePath);
        return Kind == PathReferenceKind.Absolute
            ? System.IO.Path.GetFullPath(Path)
            : System.IO.Path.GetFullPath(System.IO.Path.Combine(
                System.IO.Path.GetDirectoryName(System.IO.Path.GetFullPath(projectFilePath))!,
                Path));
    }
}

public readonly record struct ProjectId(Guid Value)
{
    public static ProjectId New() => new(Guid.CreateVersion7());
}

public readonly record struct AchievementId(Guid Value)
{
    public static AchievementId New() => new(Guid.CreateVersion7());
}

public readonly record struct AtlasId(Guid Value)
{
    public static AtlasId New() => new(Guid.CreateVersion7());
}

public readonly record struct ImageAssetId(Guid Value)
{
    public static ImageAssetId New() => new(Guid.CreateVersion7());
}

public enum ImageAssetKind { User, SteamImportedArtwork }

public enum AchievementIconKind { AtlasTile, StandaloneImage }

public readonly record struct StatId(Guid Value)
{
    public static StatId New() => new(Guid.CreateVersion7());
}

/// <summary>
/// The persisted position of an achievement or stat node on its node canvas, expressed in
/// grid cells. A missing position means the node is still auto-arranged.
/// </summary>
public readonly record struct CanvasPosition(int Column, int Row);

[SuppressMessage("Naming", "CA1720:Identifier contains type name", Justification = "Names match Steam's INT and FLOAT stat types.")]
public enum StatType { Int, Float, AverageRate }

[SuppressMessage("Naming", "CA1720:Identifier contains type name", Justification = "Names match Steam's INT and FLOAT stat types.")]
public enum UnlockValueKind { Integer, Float }
public enum SetBy { GameClient, GameServer, OfficialGameServer }
public enum AtlasTraversal { RowMajor, ColumnMajor }
public enum ProjectEntityType { Achievement, Stat }
public enum LocalizationField { AchievementDisplayName, AchievementDescription, StatDisplayName }
public enum SyncState
{
    LocalOnly,
    SteamOnly,
    InSync,
    LocalChanged,
    SteamChanged,
    Conflict,
    PendingDeletion,
}

public enum UnachievedIconMode { Grayscale, Tint }

public sealed record UnachievedIconSettings(
    UnachievedIconMode Mode = UnachievedIconMode.Grayscale,
    decimal Opacity = 0.55m,
    string TintHex = "#808080");

public sealed record AtlasSettings(
    int TileWidth,
    int TileHeight,
    int LeftOffset = 0,
    int TopOffset = 0,
    int HorizontalSpacing = 0,
    int VerticalSpacing = 0,
    AtlasTraversal Traversal = AtlasTraversal.RowMajor,
    int? OutputSize = null,
    UnachievedIconSettings? UnachievedIcon = null,
    int? OutputWidth = null,
    int? OutputHeight = null)
{
    public const int DefaultIconSize = 512;

    public UnachievedIconSettings EffectiveUnachievedIcon =>
        UnachievedIcon ?? new UnachievedIconSettings();

    public int EffectiveOutputWidth =>
        OutputWidth is > 0 and int width ? width
        : OutputSize is > 0 and int size ? size
        : DefaultIconSize;

    public int EffectiveOutputHeight =>
        OutputHeight is > 0 and int height ? height
        : OutputSize is > 0 and int size ? size
        : DefaultIconSize;
}

public sealed record AtlasDefinition(
    AtlasId Id,
    string DisplayName,
    PathReference Image,
    int Columns,
    int Rows)
{
    public bool UsesExplicitGrid => Columns > 0 && Rows > 0;
}

public sealed record StandaloneImageDefinition(
    ImageAssetId Id,
    string DisplayName,
    PathReference Image,
    ImageAssetKind Kind = ImageAssetKind.User);

public sealed record ProjectAssets(
    PathReference Atlas,
    PathReference LocalizationCatalog,
    IReadOnlyList<AtlasDefinition> Atlases,
    IReadOnlyList<StandaloneImageDefinition> Images)
{
    public ProjectAssets(PathReference atlas, PathReference localizationCatalog)
        : this(atlas, localizationCatalog, [], [])
    {
    }

    public IReadOnlyList<AtlasDefinition> EffectiveAtlases =>
        Atlases.Count > 0
            ? Atlases
            : [new AtlasDefinition(new AtlasId(Guid.Empty), "Atlas", Atlas, 0, 0)];
}

public sealed record GeneralImageDefinition(int Id, ImageAssetId? ImageId = null, AtlasId? AtlasId = null, int TileIndex = 0)
{
    public IconLayer ToLayer() => AtlasId is AtlasId atlas
        ? IconLayer.Atlas(atlas, TileIndex)
        : new IconLayer(IconLayerId.New(), "Image", IconLayerKind.StandaloneImage, ImageId: ImageId);
}

public sealed record AchievementDefinition(
    AchievementId Id,
    SteamApiName ApiName,
    LocalizationId DisplayNameLocalizationId,
    LocalizationId DescriptionLocalizationId,
    bool Hidden,
    SetBy SetBy,
    StatId? ProgressStatId,
    decimal? UnlockValue,
    int TileIndex,
    int DisplayOrder,
    AtlasId? AtlasId = null,
    ImageAssetId? StandaloneImageId = null,
    IconRecipe? Icon = null,
    ImageAssetId? UnachievedImageId = null,
    ImageAssetId? GeneralImageId = null,
    UnlockValueKind UnlockValueKind = UnlockValueKind.Integer,
    AtlasId? GeneralAtlasId = null,
    int GeneralTileIndex = 0,
    int InternalId = 0,
    CanvasPosition? Canvas = null,
    IReadOnlyList<GeneralImageDefinition>? GeneralImageEntries = null)
{
    public IReadOnlyList<GeneralImageDefinition> EffectiveGeneralImages => GeneralImageEntries ??
        (GeneralImageId is ImageAssetId image
            ? new[] { new GeneralImageDefinition(0, ImageId: image) }
            : Array.Empty<GeneralImageDefinition>())
        .Concat(GeneralAtlasId is AtlasId atlas
            ? new[] { new GeneralImageDefinition(GeneralImageId is null ? 0 : 1, AtlasId: atlas, TileIndex: GeneralTileIndex) }
            : Array.Empty<GeneralImageDefinition>()).ToArray();

    public IconRecipe EffectiveIcon =>
        Icon ?? IconRecipe.FromLegacy(TileIndex, AtlasId, StandaloneImageId);

    public IconRecipe ResolvedIcon => EffectiveIcon with
    {
        Layers = EffectiveIcon.Layers.Select(ResolveLayerLinks).ToArray(),
    };

    public IconLayer ResolveLayerLinks(IconLayer layer)
    {
        IconTextStyle? text = layer.Text;
        if (text is { LinkToAchievementValue: true } && UnlockValue is decimal unlockValue)
        {
            text = text with
            {
                Content = unlockValue.ToString(CultureInfo.InvariantCulture),
            };
        }

        if (layer.Kind == IconLayerKind.AchievementImage && GeneralImageEntries is not null)
        {
            GeneralImageDefinition? entry = GeneralImageEntries.FirstOrDefault(value => value.Id == layer.GeneralImageIndex);
            return layer with
            {
                Kind = entry?.AtlasId is not null ? IconLayerKind.AtlasTile : IconLayerKind.StandaloneImage,
                ImageId = entry?.ImageId,
                AtlasId = entry?.AtlasId,
                TileIndex = entry?.TileIndex ?? 0,
                LinkAtlasToGeneral = false,
                Text = text,
            };
        }

        if (layer.Kind == IconLayerKind.AchievementImage)
        {
            return layer with
            {
                Kind = IconLayerKind.StandaloneImage,
                ImageId = GeneralImageId,
                AtlasId = null,
                TileIndex = 0,
                LinkAtlasToGeneral = false,
                Text = text,
            };
        }

        bool useGeneralAtlas = layer.Kind == IconLayerKind.AtlasTile && layer.LinkAtlasToGeneral;
        return layer with
        {
            AtlasId = useGeneralAtlas ? GeneralAtlasId : layer.AtlasId,
            TileIndex = useGeneralAtlas ? GeneralTileIndex : layer.TileIndex,
            Text = text,
        };
    }

    public AchievementIconKind IconKind =>
        StandaloneImageId is not null ? AchievementIconKind.StandaloneImage : AchievementIconKind.AtlasTile;

    public AchievementDefinition WithIcon(IconRecipe recipe)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        IconLayer? firstAtlas = recipe.Layers.FirstOrDefault(static layer => layer.Kind == IconLayerKind.AtlasTile);
        IconLayer? firstImage = recipe.Layers.FirstOrDefault(static layer =>
            layer.Kind is IconLayerKind.StandaloneImage or IconLayerKind.SteamImportedArtwork);
        bool usesAtlas = firstAtlas is not null;
        AtlasId? atlasId = firstAtlas?.AtlasId;
        ImageAssetId? imageId = usesAtlas ? null : firstImage?.ImageId;
        int tileIndex = firstAtlas?.TileIndex ?? 0;
        IconRecipe? stored = recipe.IsSimpleLegacy(tileIndex, atlasId, imageId) ? null : recipe;
        return this with
        {
            TileIndex = tileIndex,
            AtlasId = atlasId,
            StandaloneImageId = imageId,
            Icon = stored,
        };
    }

    public static AchievementDefinition Create(
        SteamApiName apiName,
        LocalizationId displayNameLocalizationId,
        LocalizationId descriptionLocalizationId,
        int tileIndex,
        int displayOrder = 0,
        AtlasId? atlasId = null,
        ImageAssetId? standaloneImageId = null,
        int internalId = 0) =>
        new(AchievementId.New(), apiName, displayNameLocalizationId,
            descriptionLocalizationId, false, SetBy.GameClient, null, null, tileIndex, displayOrder,
            atlasId, standaloneImageId, InternalId: internalId);
}

public sealed record StatDefinition(
    StatId Id,
    SteamApiName ApiName,
    LocalizationId DisplayNameLocalizationId,
    StatType Type,
    SetBy SetBy,
    bool IncrementOnly,
    decimal? Minimum,
    decimal? Maximum,
    decimal? DefaultValue,
    decimal? MaximumChange,
    bool Aggregated,
    decimal? AverageRateWindowSeconds,
    int DisplayOrder,
    CanvasPosition? Canvas = null,
    IReadOnlyList<GeneralImageDefinition>? GeneralImageEntries = null)
{
    public static StatDefinition Create(
        SteamApiName apiName,
        LocalizationId displayNameLocalizationId,
        StatType type,
        int displayOrder = 0) =>
        new(StatId.New(), apiName, displayNameLocalizationId, type,
            SetBy.GameClient, false, null, null, null, null, false, null, displayOrder);
}

public sealed record LocalizationOwner(
    ProjectEntityType EntityType,
    Guid EntityId,
    LocalizationField Field);

public sealed record ProjectLocalizationRow(
    LocalizationId Id,
    string Context,
    string English,
    string Polish = "",
    string ChineseSimplified = "",
    string ChineseTraditional = "",
    string Japanese = "",
    string Korean = "",
    string French = "",
    LocalizationOwner? Owner = null)
{
    public string Resolve(string language) => language switch
    {
        nameof(Polish) when !string.IsNullOrWhiteSpace(Polish) => Polish,
        nameof(ChineseSimplified) when !string.IsNullOrWhiteSpace(ChineseSimplified) => ChineseSimplified,
        nameof(ChineseTraditional) when !string.IsNullOrWhiteSpace(ChineseTraditional) => ChineseTraditional,
        nameof(Japanese) when !string.IsNullOrWhiteSpace(Japanese) => Japanese,
        nameof(Korean) when !string.IsNullOrWhiteSpace(Korean) => Korean,
        nameof(French) when !string.IsNullOrWhiteSpace(French) => French,
        _ => English,
    };
}

public sealed class ProjectLocalizationCatalog
{
    public static readonly IReadOnlyList<string> Columns = Array.AsReadOnly(
        ["ID", "Context", "English", "Polish", "ChineseSimplified", "ChineseTraditional", "Japanese", "Korean", "French"]);

    private readonly ReadOnlyCollection<ProjectLocalizationRow> _rows;

    public ProjectLocalizationCatalog(IEnumerable<ProjectLocalizationRow> rows)
    {
        ArgumentNullException.ThrowIfNull(rows);
        _rows = Array.AsReadOnly(rows.ToArray());
    }

    public IReadOnlyList<ProjectLocalizationRow> Rows => _rows;

    public ProjectLocalizationRow? Find(LocalizationId id) =>
        _rows.FirstOrDefault(row => row.Id == id);

    public ProjectLocalizationCatalog Upsert(ProjectLocalizationRow row) =>
        new(_rows.Where(existing => existing.Id != row.Id).Append(row));

    public ProjectLocalizationCatalog Remove(LocalizationId id) =>
        new(_rows.Where(row => row.Id != id));
}

public sealed record RemoteTombstone(
    ProjectEntityType EntityType,
    Guid LocalId,
    SteamApiName ApiName,
    DateTimeOffset DeletedAtUtc);

public sealed record FieldSyncBaseline(
    ProjectEntityType EntityType,
    Guid LocalId,
    SteamApiName ApiName,
    IReadOnlyDictionary<string, string> FieldFingerprints,
    DateTimeOffset VerifiedAtUtc)
{
    public string? EntityFingerprint { get; init; }
    public IconSyncBaseline? Icons { get; init; }
}

public sealed record IconSyncBaseline(string LocalFingerprint, string RemoteFingerprint);

public sealed record RemoteAchievement(
    string? SteamId,
    SteamApiName ApiName,
    bool Hidden,
    SetBy SetBy,
    SteamApiName? ProgressStatApiName,
    decimal? UnlockValue,
    int DisplayOrder,
    string ContentFingerprint);

public sealed record RemoteStat(
    string? SteamId,
    SteamApiName ApiName,
    StatType Type,
    SetBy SetBy,
    bool IncrementOnly,
    decimal? Minimum,
    decimal? Maximum,
    decimal? DefaultValue,
    decimal? MaximumChange,
    bool Aggregated,
    decimal? AverageRateWindowSeconds,
    int DisplayOrder,
    string ContentFingerprint);

public sealed record RemoteLocalization(
    ProjectEntityType EntityType,
    SteamApiName ApiName,
    LocalizationField Field,
    IReadOnlyDictionary<string, string> Values,
    string ContentFingerprint);

public sealed record RemoteIconMetadata(
    SteamApiName AchievementApiName,
    string? AchievedIdentity,
    string? UnachievedIdentity,
    Uri? AchievedPreviewUri,
    Uri? UnachievedPreviewUri,
    string ContentFingerprint);

public sealed record RemoteSnapshot(
    SteamAppId AppId,
    DateTimeOffset ReadAtUtc,
    string PortalRevisionFingerprint,
    bool IsComplete,
    IReadOnlyList<RemoteStat> Stats,
    IReadOnlyList<RemoteAchievement> Achievements,
    IReadOnlyList<RemoteLocalization> Localizations,
    IReadOnlyList<RemoteIconMetadata> Icons)
{
    public bool SupportsDisplayOrder { get; init; } = true;
}

public sealed record CanvasCamera(double Zoom = 1, double OffsetX = 0, double OffsetY = 0)
{
    public const double MinimumZoom = 0.1;
    public const double MaximumZoom = 4;

    public CanvasCamera Normalize() => new(
        double.IsFinite(Zoom) ? Math.Clamp(Zoom, MinimumZoom, MaximumZoom) : 1,
        double.IsFinite(OffsetX) ? OffsetX : 0,
        double.IsFinite(OffsetY) ? OffsetY : 0);
}

public sealed record AchimeldProject(
    string Format,
    int SchemaVersion,
    ProjectId Id,
    string DisplayName,
    SteamAppId AppId,
    ProjectAssets Assets,
    AtlasSettings Atlas,
    IReadOnlyList<AchievementDefinition> Achievements,
    IReadOnlyList<StatDefinition> Stats,
    IReadOnlyList<RemoteTombstone> Tombstones,
    RemoteSnapshot? LastRemoteSnapshot,
    IReadOnlyList<FieldSyncBaseline> SyncBaselines,
    DateTimeOffset? LastVerifiedAtUtc,
    int NextAchievementInternalId = 0,
    CanvasCamera? AchievementsCamera = null,
    CanvasCamera? StatsCamera = null)
{
    public const string FormatIdentifier = "com.achimeld.project";
    public const int CurrentSchemaVersion = 1;

    // The legacy atlas path is a placeholder until an atlas is used or added to the library.
    public bool RequiresPrimaryAtlasFile => Assets.Atlases.Count > 0 ||
        Achievements.Any(static achievement => achievement.EffectiveIcon.Layers.Any(
            static layer => layer.Kind == IconLayerKind.AtlasTile));

    public static AchimeldProject Create(
        string displayName,
        SteamAppId appId,
        ProjectAssets assets,
        AtlasSettings atlas) =>
        new(FormatIdentifier, CurrentSchemaVersion, ProjectId.New(), displayName, appId,
            assets, atlas, [], [], [], null, [], null);

    /// <summary>
    /// Returns the stable internal id to assign to the next achievement. Ids only ever increase and
    /// are never recycled, so the value is the greater of the persisted counter and the highest id
    /// already in use (which keeps directly constructed projects safe).
    /// </summary>
    public int AllocateAchievementInternalId() =>
        Math.Max(
            NextAchievementInternalId,
            Achievements.Count == 0
                ? 0
                : Achievements.Max(static achievement => achievement.InternalId) + 1);

    public AtlasDefinition ResolveAtlas(AchievementDefinition achievement)
    {
        ArgumentNullException.ThrowIfNull(achievement);
        return ResolveAtlas(achievement.AtlasId);
    }

    public AtlasDefinition ResolveAtlas(AtlasId? atlasId)
    {
        IReadOnlyList<AtlasDefinition> atlases = Assets.EffectiveAtlases;
        if (atlasId is AtlasId id)
        {
            return atlases.FirstOrDefault(value => value.Id == id)
                ?? throw new InvalidOperationException($"Atlas '{id.Value:D}' is not in the project.");
        }

        return atlases[0];
    }

    public StandaloneImageDefinition? ResolveStandaloneImage(AchievementDefinition achievement)
    {
        ArgumentNullException.ThrowIfNull(achievement);
        return achievement.StandaloneImageId is ImageAssetId imageId
            ? ResolveStandaloneImage(imageId)
            : null;
    }

    public StandaloneImageDefinition? ResolveStandaloneImage(ImageAssetId imageId) =>
        Assets.Images.FirstOrDefault(value => value.Id == imageId);
}

public static class ProjectArtworkOwnership
{
    public static AchimeldProject PruneUnreferencedSteamArtwork(AchimeldProject project)
    {
        ArgumentNullException.ThrowIfNull(project);
        HashSet<ImageAssetId> importedIds = project.Assets.Images
            .Where(static image => image.Kind == ImageAssetKind.SteamImportedArtwork)
            .Select(static image => image.Id)
            .ToHashSet();
        if (importedIds.Count == 0)
        {
            return project;
        }

        AchievementDefinition[] achievements = project.Achievements.Select(achievement =>
        {
            bool hasSteamArtworkLayer = achievement.EffectiveIcon.Layers.Any(
                static layer => layer.Kind == IconLayerKind.SteamImportedArtwork);
            return !hasSteamArtworkLayer &&
                   achievement.UnachievedImageId is ImageAssetId unachieved &&
                   importedIds.Contains(unachieved)
                ? achievement with { UnachievedImageId = null }
                : achievement;
        }).ToArray();

        HashSet<ImageAssetId> referenced = achievements
            .SelectMany(static achievement => achievement.EffectiveIcon.Layers)
            .Where(static layer => layer.ImageId is not null)
            .Select(static layer => layer.ImageId!.Value)
            .ToHashSet();
        foreach (AchievementDefinition achievement in achievements)
        {
            foreach (var entry in achievement.EffectiveGeneralImages)
                if (entry.ImageId is ImageAssetId image) referenced.Add(image);
            if (achievement.UnachievedImageId is ImageAssetId unachieved)
            {
                referenced.Add(unachieved);
            }
        }

        return project with
        {
            Achievements = achievements,
            Assets = project.Assets with
            {
                Images = project.Assets.Images
                    .Where(image => image.Kind != ImageAssetKind.SteamImportedArtwork || referenced.Contains(image.Id))
                    .ToArray(),
            },
        };
    }
}
