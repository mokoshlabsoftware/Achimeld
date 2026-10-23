using System.Text;

namespace Achimeld.Core;

public enum ValidationSeverity { Warning, Error }

public sealed record ValidationIssue(
    string Code,
    string Path,
    string Message,
    ValidationSeverity Severity = ValidationSeverity.Error);

public sealed record ProjectValidationContext(
    ProjectLocalizationCatalog? LocalizationCatalog = null,
    string? ProjectFilePath = null,
    int? AtlasPixelWidth = null,
    int? AtlasPixelHeight = null,
    bool CheckReferencedFiles = false,
    IReadOnlyDictionary<Guid, (int Width, int Height)>? AtlasImageSizes = null);

public static class ProjectValidator
{
    public static IReadOnlyList<ValidationIssue> Validate(
        AchimeldProject project,
        ProjectValidationContext? context = null)
    {
        ArgumentNullException.ThrowIfNull(project);
        context ??= new ProjectValidationContext();
        List<ValidationIssue> issues = [];

        RequiredMetadata(project, issues);
        UniqueIdentities(project, issues);
        ValidateAtlas(project, context, issues);
        ValidateStats(project, issues);
        ValidateAchievements(project, issues);
        ValidateLocalization(project, context.LocalizationCatalog, issues);
        ValidateReferences(project, context, issues);

        return issues;
    }

    private static void RequiredMetadata(AchimeldProject project, List<ValidationIssue> issues)
    {
        if (!string.Equals(project.Format, AchimeldProject.FormatIdentifier, StringComparison.Ordinal))
        {
            Add(issues, "project.format", "$.format", "The project format identifier is not supported.");
        }

        if (project.SchemaVersion != AchimeldProject.CurrentSchemaVersion)
        {
            Add(issues, "project.schema", "$.schemaVersion", "The project schema version is not supported.");
        }

        if (project.Id.Value == Guid.Empty)
        {
            Add(issues, "project.id", "$.id", "The project ID cannot be empty.");
        }

        if (string.IsNullOrWhiteSpace(project.DisplayName))
        {
            Add(issues, "project.displayName", "$.displayName", "A display name is required.");
        }

        if (project.AppId.Value == 0)
        {
            Add(issues, "project.appId", "$.appId", "A nonzero Steam App ID is required.");
        }
    }

    private static void UniqueIdentities(AchimeldProject project, List<ValidationIssue> issues)
    {
        AddDuplicates(project.Achievements, static value => value.Id.Value, "$.achievements", "achievement.id.duplicate", issues);
        AddDuplicates(project.Achievements, static value => value.ApiName.Value, "$.achievements", "achievement.apiName.duplicate", issues);
        AddDuplicates(project.Stats, static value => value.Id.Value, "$.stats", "stat.id.duplicate", issues);
        AddDuplicates(project.Stats, static value => value.ApiName.Value, "$.stats", "stat.apiName.duplicate", issues);
    }

    private static void ValidateAtlas(
        AchimeldProject project,
        ProjectValidationContext context,
        List<ValidationIssue> issues)
    {
        AtlasSettings atlas = project.Atlas;
        if (atlas.TileWidth <= 0)
        {
            Add(issues, "atlas.tileWidth", "$.atlas.tileWidth", "Tile width must be positive.");
        }

        if (atlas.TileHeight <= 0)
        {
            Add(issues, "atlas.tileHeight", "$.atlas.tileHeight", "Tile height must be positive.");
        }

        if (atlas.LeftOffset < 0 || atlas.TopOffset < 0 || atlas.HorizontalSpacing < 0 || atlas.VerticalSpacing < 0)
        {
            Add(issues, "atlas.geometry", "$.atlas", "Offsets and spacing cannot be negative.");
        }

        if (atlas.OutputSize is <= 0)
        {
            Add(issues, "atlas.outputSize", "$.atlas.outputSize", "Output size must be positive when specified.");
        }

        if (atlas.OutputWidth is <= 0)
        {
            Add(issues, "atlas.outputWidth", "$.atlas.outputWidth", "Icon width must be positive when specified.");
        }

        if (atlas.OutputHeight is <= 0)
        {
            Add(issues, "atlas.outputHeight", "$.atlas.outputHeight", "Icon height must be positive when specified.");
        }

        if (atlas.EffectiveUnachievedIcon.Opacity is < 0 or > 1)
        {
            Add(issues, "atlas.unachieved.opacity", "$.atlas.unachievedIcon.opacity", "Opacity must be from 0 through 1.");
        }

        IReadOnlyList<AtlasDefinition> atlases = project.Assets.EffectiveAtlases;
        AddDuplicates(atlases, static value => value.Id.Value, "$.assets.atlases", "atlas.id.duplicate", issues);
        for (int index = 0; index < project.Assets.Atlases.Count; index++)
        {
            AtlasDefinition definition = project.Assets.Atlases[index];
            string path = $"$.assets.atlases[{index}]";
            if (string.IsNullOrWhiteSpace(definition.DisplayName))
            {
                Add(issues, "atlas.displayName", $"{path}.displayName", "An atlas display name is required.");
            }

            if (definition.Columns < 0 || definition.Rows < 0)
            {
                Add(issues, "atlas.grid", $"{path}", "Grid divisions cannot be negative.");
            }
            else if (definition.Columns == 0 ^ definition.Rows == 0)
            {
                Add(issues, "atlas.grid.incomplete", path, "Grid columns and rows must be specified together.");
            }

            if (definition.UsesExplicitGrid &&
                TryGetAtlasSize(definition, context, out int width, out int height))
            {
                AtlasGridMetrics? metrics = AtlasGridGeometry.TryMeasure(width, height, definition.Columns, definition.Rows);
                if (metrics is null)
                {
                    Add(issues, "atlas.grid.bounds", path, "The atlas image is too small for the configured grid.");
                }
                else if (!metrics.Value.IsSquare)
                {
                    Add(
                        issues,
                        "atlas.grid.notSquare",
                        path,
                        "The atlas grid does not produce square tiles.",
                        ValidationSeverity.Warning);
                }
            }
        }

        AddDuplicates(project.Assets.Images, static value => value.Id.Value, "$.assets.images", "image.id.duplicate", issues);
        for (int index = 0; index < project.Assets.Images.Count; index++)
        {
            if (string.IsNullOrWhiteSpace(project.Assets.Images[index].DisplayName))
            {
                Add(issues, "image.displayName", $"$.assets.images[{index}].displayName", "An image display name is required.");
            }
        }

        Dictionary<(Guid Atlas, int Tile), int> tiles = [];
        for (int index = 0; index < project.Achievements.Count; index++)
        {
            AchievementDefinition achievement = project.Achievements[index];
            string achievementPath = $"$.achievements[{index}]";
            if (achievement.UnachievedImageId is ImageAssetId lockedImage && project.ResolveStandaloneImage(lockedImage) is null)
            {
                Add(issues, "achievement.unachievedImage.missing", $"{achievementPath}.unachievedImageId",
                    "The unachieved icon image does not exist.");
            }
            IconRecipe recipe = achievement.EffectiveIcon;
            if (!string.IsNullOrWhiteSpace(recipe.BackgroundColorHex) &&
                !IconRecipe.TryParseRgba(recipe.BackgroundColorHex, out _, out _, out _, out _))
            {
                Add(issues, "achievement.icon.background", $"{achievementPath}.icon.backgroundColorHex",
                    "Background color must use #RRGGBB or #AARRGGBB.");
            }

            if (recipe.Layers.Count == 0)
            {
                Add(issues, "achievement.icon.layers", $"{achievementPath}.icon.layers",
                    "An icon recipe must contain at least one layer.");
                continue;
            }

            bool simpleAtlas = recipe.IsSimpleLegacy(
                achievement.TileIndex, achievement.AtlasId, achievement.StandaloneImageId)
                && achievement.IconKind == AchievementIconKind.AtlasTile;

            ValidateMaskReferences(recipe, $"{achievementPath}.icon.layers", issues);

            for (int layerIndex = 0; layerIndex < recipe.Layers.Count; layerIndex++)
            {
                IconLayer layer = achievement.ResolveLayerLinks(recipe.Layers[layerIndex]);
                string layerPath = $"{achievementPath}.icon.layers[{layerIndex}]";
                if (!double.IsFinite(layer.Scale) || layer.Scale is < 0.1 or > 4)
                {
                    Add(issues, "achievement.icon.scale", $"{layerPath}.scale", "Layer scale must be from 0.1 through 4.");
                }

                if (!double.IsFinite(layer.OffsetX) || layer.OffsetX is < -4096 or > 4096 ||
                    !double.IsFinite(layer.OffsetY) || layer.OffsetY is < -4096 or > 4096)
                {
                    Add(issues, "achievement.icon.offset", $"{layerPath}.offset", "Layer offsets must be from -4096 through 4096.");
                }

                if (!double.IsFinite(layer.Rotation) || layer.Rotation is < -360 or > 360)
                {
                    Add(issues, "achievement.icon.rotation", $"{layerPath}.rotation", "Layer angle must be from -360 through 360.");
                }

                if (!double.IsFinite(layer.Opacity) || layer.Opacity is < 0 or > 1)
                {
                    Add(issues, "achievement.icon.opacity", $"{layerPath}.opacity", "Layer opacity must be from 0 through 1.");
                }


                if (!Enum.IsDefined(layer.MaskMode))
                    Add(issues, "achievement.icon.maskMode", layerPath, "Invalid mask channel.");

                for (int effectIndex = 0; effectIndex < layer.EffectiveEffects.Count; effectIndex++)
                {
                    ValidateLayerEffect(layer.EffectiveEffects[effectIndex],
                        $"{layerPath}.effects[{effectIndex}]", issues);
                }

                switch (layer.Kind)
                {
                    case IconLayerKind.StandaloneImage:
                        if (layer.ImageId is not ImageAssetId imageId ||
                            project.Assets.Images.All(value => value.Id != imageId))
                        {
                            Add(issues, "achievement.image.missing", $"{layerPath}.imageId",
                                "The referenced standalone image does not exist.");
                        }

                        break;
                    case IconLayerKind.SteamImportedArtwork:
                        if (layer.ImageId is not ImageAssetId steamImageId ||
                            project.ResolveStandaloneImage(steamImageId) is not { Kind: ImageAssetKind.SteamImportedArtwork })
                        {
                            Add(issues, "achievement.steamArtwork.missing", $"{layerPath}.imageId",
                                "The imported Steam artwork asset does not exist.");
                        }

                        break;
                    case IconLayerKind.AtlasTile:
                        ValidateAtlasTileLayer(
                            project, atlas, context, layer, layerPath, issues,
                            simpleAtlas ? tiles : null, index);
                        break;
                    case IconLayerKind.Text:
                        ValidateTextLayer(layer.Text, layerPath, issues);
                        break;
                    case IconLayerKind.Text3d:
                        ValidateTextLayer(layer.Text, layerPath, issues);
                        if (!(layer.Text3d ?? new()).IsValid() || layer.Text is { FontSize: < 1 or > 512 } ||
                            (layer.Text?.Content ?? string.Empty).EnumerateRunes().Count() > 256)
                            Add(issues, "achievement.icon.text3d", $"{layerPath}.text3d", "Invalid 3D text camera, geometry, lighting, or text budget.");
                        break;
                    case IconLayerKind.Artifact3d:
                        if (!(layer.Artifact3d ?? new()).IsValid())
                            Add(issues, "achievement.icon.artifact3d", $"{layerPath}.artifact3d", "Invalid 3D artifact parts, material, geometry, or camera.");
                        break;
                    case IconLayerKind.Shape:
                        IconShapeStyle shape = layer.Shape ?? new();
                        if (!IconRecipe.TryParseRgba(shape.ColorHex, out _, out _, out _, out _) ||
                            !Enum.IsDefined(shape.Kind) || !double.IsFinite(shape.ScaleX) || shape.ScaleX is < 0.01 or > 4 ||
                            !double.IsFinite(shape.ScaleY) || shape.ScaleY is < 0.01 or > 4 ||
                            !double.IsFinite(shape.Rotation) || shape.Rotation is < -360 or > 360 ||
                            shape.VertexCount is < 3 or > 64 ||
                            !double.IsFinite(shape.InnerRadius) || shape.InnerRadius is < 0.01 or > 1 ||
                            !double.IsFinite(shape.OuterRounding) || shape.OuterRounding is < 0 or > 1 ||
                            !double.IsFinite(shape.InnerRounding) || shape.InnerRounding is < 0 or > 1)
                            Add(issues, "achievement.icon.shape", $"{layerPath}.shape", "Shape needs a valid color, X/Y scale from 0.01 through 4, rotation from -360 through 360, 3 through 64 vertices, inner radius from 0.01 through 1, and rounding from 0 through 1.");
                        break;
                    case IconLayerKind.Solid:
                        ValidateSolidLayer(layer, layerPath, issues);
                        break;
                    case IconLayerKind.SdfVignette:
                        IconSdfVignetteStyle sdf = layer.SdfVignette ?? new();
                        if (!IconRecipe.TryParseRgba(sdf.ColorHex, out _, out _, out _, out _) ||
                            !double.IsFinite(sdf.Roundness) || sdf.Roundness is < 0 or > 1 ||
                            !double.IsFinite(sdf.Inset) || sdf.Inset is < 0 or > 0.95 ||
                            !double.IsFinite(sdf.Softness) || sdf.Softness is < 0 or > 1)
                            Add(issues, "achievement.icon.sdfVignette", layerPath, "Invalid SDF vignette color or parameters.");
                        if (sdf.UseGradient || sdf.Gradient is not null)
                            ValidateGradientLayer(layer with { Gradient = sdf.Gradient }, $"{layerPath}.sdfVignette", issues);
                        break;
                    case IconLayerKind.Particle:
                        IconParticleStyle particles = layer.Particles ?? new();
                        if (!particles.ValidParameters())
                            Add(issues, "achievement.icon.particles", layerPath, "Invalid particle count, shape or variation range.");
                        ValidateGradientLayer(layer with { Gradient = particles.Gradient }, $"{layerPath}.particles", issues);
                        break;
                    case IconLayerKind.Explosion:
                        IconExplosionStyle explosion = layer.Explosion ?? new();
                        if (!explosion.ValidParameters())
                            Add(issues, "achievement.icon.explosion", layerPath, "Invalid explosion count, triangle or variation range.");
                        ValidateGradientLayer(layer with { Gradient = explosion.Gradient }, $"{layerPath}.explosion", issues, allowSdf: true);
                        break;
                    case IconLayerKind.PerlinNoise:
                        IconPerlinNoiseStyle noise = layer.PerlinNoise ?? new();
                        if (noise.Seed < 0 || noise.Octaves is < 1 or > 12 ||
                            !double.IsFinite(noise.Frequency) || noise.Frequency is < 0.1 or > 128 ||
                            !double.IsFinite(noise.Persistence) || noise.Persistence is < 0 or > 1 ||
                            !double.IsFinite(noise.Lacunarity) || noise.Lacunarity is < 1 or > 4 ||
                            noise.EnabledOctaves is { Count: > 12 })
                            Add(issues, "achievement.icon.perlinNoise", layerPath, "Invalid Perlin noise parameters or octave switches.");
                        if (noise.UseGradient || noise.Gradient is not null)
                            ValidateGradientLayer(layer with { Gradient = noise.Gradient }, $"{layerPath}.perlinNoise", issues);
                        break;
                    case IconLayerKind.Gradient:
                        ValidateGradientLayer(layer, layerPath, issues);
                        break;
                    case IconLayerKind.Curves:
                        if (layer.Curves is { } curves && (curves.Points.Count < 2 || curves.Points.Any(p =>
                            !double.IsFinite(p.Input) || !double.IsFinite(p.Output) || p.Input is < 0 or > 1 || p.Output is < 0 or > 1)))
                            Add(issues,"achievement.icon.curves",layerPath,"Curve points must be between zero and one.");
                        break;
                    case IconLayerKind.Contrast:
                    case IconLayerKind.HueSaturation:
                    case IconLayerKind.SelectiveColor:
                    case IconLayerKind.GradientOverlay:
                        if (layer.Adjustment is { } adjustment &&
                            (new[] { adjustment.Brightness, adjustment.Contrast, adjustment.Saturation, adjustment.Lightness,
                                adjustment.Cyan, adjustment.Magenta, adjustment.Yellow, adjustment.Black }.Any(v => !double.IsFinite(v) || v is < -100 or > 100)
                                || !double.IsFinite(adjustment.Hue) || adjustment.Hue is < -180 or > 180))
                            Add(issues,"achievement.icon.adjustment",layerPath,"Adjustment values are outside their allowed range.");
                        if (layer.Kind == IconLayerKind.GradientOverlay) ValidateGradientLayer(layer,layerPath,issues);
                        if (layer.Adjustment is { } selective && selective.EffectiveSelectiveColors.Any(c => !Enum.IsDefined(c.Range) ||
                            new[] { c.Cyan,c.Magenta,c.Yellow,c.Black }.Any(v => !double.IsFinite(v) || v is < -100 or > 100)))
                            Add(issues,"achievement.icon.selectiveColor",layerPath,"Invalid selective color correction.");
                        break;
                    case IconLayerKind.RadialBlur:
                        var radial = layer.Adjustment ?? new();
                        if (!double.IsFinite(radial.RadialBlur) || radial.RadialBlur is < 0 or > 100 ||
                            !double.IsFinite(radial.AngularBlur) || radial.AngularBlur is < 0 or > 100)
                            Add(issues, "achievement.icon.radialBlur", layerPath, "Radial and angular blur must be between zero and 100%.");
                        break;
                    case IconLayerKind.UnsharpMask:
                        var unsharp = layer.Adjustment ?? new();
                        if (!double.IsFinite(unsharp.UnsharpAmount) || unsharp.UnsharpAmount is < 1 or > 500 ||
                            !double.IsFinite(unsharp.UnsharpRadius) || unsharp.UnsharpRadius is < .1 or > 250 ||
                            unsharp.UnsharpThreshold is < 0 or > 255)
                            Add(issues, "achievement.icon.unsharpMask", layerPath, "Unsharp Mask requires amount 1–500%, radius 0.1–250 pixels, and threshold 0–255.");
                        break;
                    case IconLayerKind.Folder:
                        break;
                    default:
                        Add(issues, "achievement.icon.layerKind", $"{layerPath}.kind", "The icon layer kind is not supported.");
                        break;
                }
            }
        }
    }

    private static void ValidateLayerEffect(
        IconLayerEffect effect,
        string effectPath,
        List<ValidationIssue> issues)
    {
        if (effect.Kind is IconLayerEffectKind.Skew or IconLayerEffectKind.Perspective or IconLayerEffectKind.Pinch or IconLayerEffectKind.LensBlur)
        {
            bool Invalid(double value, double min, double max) => !double.IsFinite(value) || value < min || value > max;
            bool invalid = effect.Kind switch
            {
                IconLayerEffectKind.Skew => Invalid(effect.SkewX, -60, 60) || Invalid(effect.SkewY, -60, 60),
                IconLayerEffectKind.Perspective => Invalid(effect.PerspectiveX, -0.45, 0.45) || Invalid(effect.PerspectiveY, -0.45, 0.45),
                IconLayerEffectKind.Pinch => Invalid(effect.Amount, -1, 1),
                IconLayerEffectKind.LensBlur => Invalid(effect.Blur, 0, 128) || Invalid(effect.Aberration, 0, 64),
                _ => false,
            };
            if (invalid || Invalid(effect.Opacity, 0, 1))
                Add(issues, "achievement.icon.effectDistortion", effectPath, "Invalid spatial effect parameters or opacity.");
        }
        if (!IconRecipe.TryParseRgba(effect.ColorHex, out _, out _, out _, out _))
        {
            Add(issues, "achievement.icon.effectColor", $"{effectPath}.colorHex",
                "Effect color must use #RRGGBB or #AARRGGBB.");
        }

        if (effect.Kind == IconLayerEffectKind.GradientOverlay)
        {
            ValidateGradientLayer(IconLayer.FromGradient(effect.Gradient ?? IconGradientStyle.Default()), effectPath, issues);
            if (!double.IsFinite(effect.Opacity) || effect.Opacity is < 0 or > 1 || !Enum.IsDefined(effect.BlendMode))
                Add(issues, "achievement.icon.effectOpacity", effectPath, "Gradient overlay requires valid opacity and blend mode.");
        }
        if (effect.Kind == IconLayerEffectKind.InnerLight)
        {
            if (!double.IsFinite(effect.Cutout) || effect.Cutout is < 0 or > 256)
                Add(issues, "achievement.icon.effectCutout", $"{effectPath}.cutout", "Cutout width must be from 0 through 256.");
            if (!IconRecipe.TryParseRgba(effect.BlurColorHex, out _, out _, out _, out _))
                Add(issues, "achievement.icon.effectBlurColor", $"{effectPath}.blurColorHex", "Blur color must use #RRGGBB or #AARRGGBB.");
            if (!double.IsFinite(effect.BlurOpacity) || effect.BlurOpacity is < 0 or > 1 ||
                !double.IsFinite(effect.Opacity) || effect.Opacity is < 0 or > 1)
                Add(issues, "achievement.icon.effectOpacity", $"{effectPath}.opacity", "Effect opacity must be from 0 through 1.");
        }
        if (effect.Kind is IconLayerEffectKind.Outline or IconLayerEffectKind.OuterLight &&
            (!double.IsFinite(effect.Width) || effect.Width is < 0 or > 256))
        {
            Add(issues, "achievement.icon.effectWidth", $"{effectPath}.width",
                "Outline width must be from 0 through 256.");
        }

        if (effect.Kind == IconLayerEffectKind.BoxBlur &&
            (!double.IsFinite(effect.Blur) || effect.Blur is < 0 or > 256 ||
             !double.IsFinite(effect.Opacity) || effect.Opacity is < 0 or > 1))
            Add(issues, "achievement.icon.effectBlur", $"{effectPath}.blur", "Box blur radius must be from 0 through 256 and opacity from 0 through 1.");
        if (effect.Kind == IconLayerEffectKind.OuterLight &&
            (!double.IsFinite(effect.Blur) || effect.Blur is < 0 or > 256 ||
             !double.IsFinite(effect.Opacity) || effect.Opacity is < 0 or > 1 || !Enum.IsDefined(effect.BlendMode)))
            Add(issues, "achievement.icon.effectBlur", $"{effectPath}.blur", "Outer light requires blur from 0 through 256 and valid opacity and blend mode.");
        if (effect.Kind is IconLayerEffectKind.Shadow or IconLayerEffectKind.InnerLight)
        {
            if (!double.IsFinite(effect.Blur) || effect.Blur is < 0 or > 256)
            {
                Add(issues, "achievement.icon.effectBlur", $"{effectPath}.blur",
                    "Shadow blur must be from 0 through 256.");
            }

            if (!double.IsFinite(effect.OffsetX) || effect.OffsetX is < -4096 or > 4096 ||
                !double.IsFinite(effect.OffsetY) || effect.OffsetY is < -4096 or > 4096)
            {
                Add(issues, "achievement.icon.effectOffset", $"{effectPath}.offset",
                    "Shadow offsets must be from -4096 through 4096.");
            }
        }
    }

    private static void ValidateAtlasTileLayer(
        AchimeldProject project,
        AtlasSettings atlas,
        ProjectValidationContext context,
        IconLayer layer,
        string layerPath,
        List<ValidationIssue> issues,
        Dictionary<(Guid Atlas, int Tile), int>? uniqueTiles,
        int achievementIndex)
    {
        int tile = layer.TileIndex;
        if (tile < 0)
        {
            Add(issues, "achievement.tileIndex", $"{layerPath}.tileIndex", "Tile index cannot be negative.");
            return;
        }

        AtlasDefinition definition;
        try
        {
            definition = project.ResolveAtlas(layer.AtlasId);
        }
        catch (InvalidOperationException)
        {
            Add(issues, "achievement.atlas.missing", $"{layerPath}.atlasId", "The referenced atlas does not exist.");
            return;
        }

        if (uniqueTiles is not null)
        {
            (Guid Atlas, int Tile) key = (definition.Id.Value, tile);
            if (!uniqueTiles.TryAdd(key, achievementIndex))
            {
                Add(issues, "achievement.tileIndex.duplicate", $"{layerPath}.tileIndex",
                    "Tile indices must be unique within an atlas.");
            }
        }

        if (definition.UsesExplicitGrid)
        {
            if (!AtlasGridGeometry.TileFits(tile, definition.Columns, definition.Rows))
            {
                Add(issues, "achievement.tileIndex.bounds", $"{layerPath}.tileIndex", "The assigned tile is outside the atlas.");
            }
        }
        else if (TryGetAtlasSize(definition, context, out int width, out int height) &&
                 atlas.TileWidth > 0 && atlas.TileHeight > 0 && !TileFits(tile, atlas, width, height))
        {
            Add(issues, "achievement.tileIndex.bounds", $"{layerPath}.tileIndex", "The assigned tile is outside the atlas.");
        }
    }

    private static void ValidateTextLayer(IconTextStyle? text, string layerPath, List<ValidationIssue> issues)
    {
        if (text is null)
        {
            Add(issues, "achievement.icon.text", $"{layerPath}.text", "A text layer requires text settings.");
            return;
        }

        if (string.IsNullOrWhiteSpace(text.FontFamily))
        {
            Add(issues, "achievement.icon.font", $"{layerPath}.text.fontFamily", "A font is required.");
        }

        if (!double.IsFinite(text.FontSize) || text.FontSize <= 0)
        {
            Add(issues, "achievement.icon.fontSize", $"{layerPath}.text.fontSize", "Font size must be positive.");
        }

        if (!IconRecipe.TryParseRgba(text.ColorHex, out _, out _, out _, out _))
        {
            Add(issues, "achievement.icon.color", $"{layerPath}.text.colorHex", "Text color must use #RRGGBB or #AARRGGBB.");
        }

        if (text.OutlineEnabled)
        {
            if (!IconRecipe.TryParseRgba(text.OutlineColorHex, out _, out _, out _, out _))
            {
                Add(issues, "achievement.icon.outline", $"{layerPath}.text.outlineColorHex",
                    "Outline color must use #RRGGBB or #AARRGGBB.");
            }

            if (!double.IsFinite(text.OutlineWidth) || text.OutlineWidth < 0)
            {
                Add(issues, "achievement.icon.outlineWidth", $"{layerPath}.text.outlineWidth",
                    "Outline width cannot be negative.");
            }
        }
    }

    private static void ValidateSolidLayer(IconLayer layer, string layerPath, List<ValidationIssue> issues)
    {
        if (!IconRecipe.TryParseRgba(layer.SolidColorHex, out _, out _, out _, out _))
        {
            Add(issues, "achievement.icon.solid", $"{layerPath}.solidColorHex",
                "Solid color must use #RRGGBB or #AARRGGBB.");
        }
    }

    private static void ValidateGradientLayer(IconLayer layer, string layerPath, List<ValidationIssue> issues, bool allowSdf = false)
    {
        if (layer.Gradient is not IconGradientStyle gradient)
        {
            Add(issues, "achievement.icon.gradient", $"{layerPath}.gradient", "A gradient layer requires gradient settings.");
            return;
        }

        if (!Enum.IsDefined(gradient.InputChannel))
        {
            Add(issues, "achievement.icon.gradient.inputChannel", $"{layerPath}.gradient.inputChannel",
                "Gradient input channel must be RGB, Red, Green, or Blue.");
        }
        if (gradient.Kind == IconGradientKind.SDF && !allowSdf)
            Add(issues, "achievement.icon.gradient.kind", $"{layerPath}.gradient.kind",
                "SDF gradients require an explosion shape.");

        if (!double.IsFinite(gradient.Angle))
        {
            Add(issues, "achievement.icon.gradient.angle", $"{layerPath}.gradient.angle",
                "Gradient angle must be finite.");
        }

        if (gradient.Stops.Count == 0)
        {
            Add(issues, "achievement.icon.gradient.stops", $"{layerPath}.gradient.stops",
                "A gradient requires at least one color stop.");
            return;
        }

        for (int stopIndex = 0; stopIndex < gradient.Stops.Count; stopIndex++)
        {
            IconGradientStop stop = gradient.Stops[stopIndex];
            string stopPath = $"{layerPath}.gradient.stops[{stopIndex}]";
            if (!double.IsFinite(stop.Offset) || stop.Offset is < 0 or > 1)
            {
                Add(issues, "achievement.icon.gradient.offset", $"{stopPath}.offset",
                    "Gradient stop offsets must be between 0 and 1.");
            }

            if (!IconRecipe.TryParseRgba(stop.ColorHex, out _, out _, out _, out _))
            {
                Add(issues, "achievement.icon.gradient.color", $"{stopPath}.colorHex",
                    "Gradient stop color must use #RRGGBB or #AARRGGBB.");
            }
        }
    }

    private static bool TryGetAtlasSize(
        AtlasDefinition definition,
        ProjectValidationContext context,
        out int width,
        out int height)
    {
        if (context.AtlasImageSizes is not null &&
            context.AtlasImageSizes.TryGetValue(definition.Id.Value, out (int Width, int Height) sized))
        {
            width = sized.Width;
            height = sized.Height;
            return true;
        }

        if (definition.Id.Value == Guid.Empty &&
            context.AtlasPixelWidth is int legacyWidth &&
            context.AtlasPixelHeight is int legacyHeight)
        {
            width = legacyWidth;
            height = legacyHeight;
            return true;
        }

        if (context.AtlasPixelWidth is int fallbackWidth && context.AtlasPixelHeight is int fallbackHeight)
        {
            width = fallbackWidth;
            height = fallbackHeight;
            return true;
        }

        width = 0;
        height = 0;
        return false;
    }

    private static bool TileFits(int tileIndex, AtlasSettings atlas, int imageWidth, int imageHeight)
    {
        if (tileIndex < 0 || imageWidth <= atlas.LeftOffset || imageHeight <= atlas.TopOffset)
        {
            return false;
        }

        int columns = 1 + ((imageWidth - atlas.LeftOffset - atlas.TileWidth) /
            (atlas.TileWidth + atlas.HorizontalSpacing));
        int rows = 1 + ((imageHeight - atlas.TopOffset - atlas.TileHeight) /
            (atlas.TileHeight + atlas.VerticalSpacing));
        if (columns <= 0 || rows <= 0 || tileIndex >= checked(columns * rows))
        {
            return false;
        }

        int column = atlas.Traversal == AtlasTraversal.RowMajor ? tileIndex % columns : tileIndex / rows;
        int row = atlas.Traversal == AtlasTraversal.RowMajor ? tileIndex / columns : tileIndex % rows;
        int right = atlas.LeftOffset + (column * (atlas.TileWidth + atlas.HorizontalSpacing)) + atlas.TileWidth;
        int bottom = atlas.TopOffset + (row * (atlas.TileHeight + atlas.VerticalSpacing)) + atlas.TileHeight;
        return right <= imageWidth && bottom <= imageHeight;
    }

    private static void ValidateStats(AchimeldProject project, List<ValidationIssue> issues)
    {
        for (int index = 0; index < project.Stats.Count; index++)
        {
            StatDefinition stat = project.Stats[index];
            string path = $"$.stats[{index}]";
            if (stat.Id.Value == Guid.Empty)
            {
                Add(issues, "stat.id", $"{path}.id", "The stat ID cannot be empty.");
            }

            if (stat.Minimum is decimal minimum && stat.Maximum is decimal maximum && minimum > maximum)
            {
                Add(issues, "stat.range", path, "Minimum cannot exceed maximum.");
            }

            if (stat.DefaultValue is decimal defaultValue &&
                ((stat.Minimum is decimal min && defaultValue < min) ||
                 (stat.Maximum is decimal max && defaultValue > max)))
            {
                Add(issues, "stat.default.range", $"{path}.defaultValue", "Default value must be within the configured range.");
            }

            if (stat.MaximumChange is <= 0)
            {
                Add(issues, "stat.maximumChange", $"{path}.maximumChange", "Maximum change must be positive when specified.");
            }

            if (stat.IncrementOnly && stat.MaximumChange is < 0)
            {
                Add(issues, "stat.incrementOnly", path, "An increment-only stat cannot permit a negative maximum change.");
            }

            if (stat.Type == StatType.AverageRate)
            {
                if (stat.AverageRateWindowSeconds is null or <= 0)
                {
                    Add(issues, "stat.averageRate.window", $"{path}.averageRateWindowSeconds", "AVGRATE requires a positive window.");
                }
            }
            else if (stat.AverageRateWindowSeconds is not null)
            {
                Add(issues, "stat.averageRate.notApplicable", $"{path}.averageRateWindowSeconds", "The AVGRATE window is only valid for AVGRATE stats.");
            }

            if (stat.Type == StatType.Int)
            {
                ValidateIntegral(stat.Minimum, $"{path}.minimum", issues);
                ValidateIntegral(stat.Maximum, $"{path}.maximum", issues);
                ValidateIntegral(stat.DefaultValue, $"{path}.defaultValue", issues);
                ValidateIntegral(stat.MaximumChange, $"{path}.maximumChange", issues);
            }
        }
    }

    private static void ValidateIntegral(decimal? value, string path, List<ValidationIssue> issues)
    {
        if (value is decimal number && decimal.Truncate(number) != number)
        {
            Add(issues, "stat.int.fraction", path, "INT stat values must be whole numbers.");
        }
    }

    private static void ValidateAchievements(AchimeldProject project, List<ValidationIssue> issues)
    {
        Dictionary<StatId, StatDefinition> stats = project.Stats
            .GroupBy(static stat => stat.Id)
            .ToDictionary(static group => group.Key, static group => group.First());

        for (int index = 0; index < project.Achievements.Count; index++)
        {
            AchievementDefinition achievement = project.Achievements[index];
            string path = $"$.achievements[{index}]";
            if (achievement.Id.Value == Guid.Empty)
            {
                Add(issues, "achievement.id", $"{path}.id", "The achievement ID cannot be empty.");
            }

            if (achievement.ProgressStatId is StatId statId)
            {
                if (achievement.UnlockValue is null)
                {
                    Add(issues, "achievement.progress.incomplete", path, "A progress stat requires an unlock value.");
                }
                else if (!stats.TryGetValue(statId, out StatDefinition? stat))
                {
                    Add(issues, "achievement.progress.missingStat", $"{path}.progressStatId", "The progress stat does not exist.");
                }
                else if (achievement.UnlockValue is decimal unlock &&
                    ((stat.Minimum is decimal min && unlock < min) ||
                     (stat.Maximum is decimal max && unlock > max) ||
                     (stat.Type == StatType.Int && decimal.Truncate(unlock) != unlock)))
                {
                    Add(issues, "achievement.progress.range", $"{path}.unlockValue", "Unlock value must be within the progress stat range.");
                }
            }
            else if (achievement.UnlockValueKind == UnlockValueKind.Integer &&
                achievement.UnlockValue is decimal manualUnlock &&
                decimal.Truncate(manualUnlock) != manualUnlock)
            {
                Add(issues, "achievement.unlock.wholeNumber", $"{path}.unlockValue", "Unlock value must be a whole number.");
            }
        }
    }

    private static void ValidateLocalization(
        AchimeldProject project,
        ProjectLocalizationCatalog? catalog,
        List<ValidationIssue> issues)
    {
        if (catalog is null)
        {
            Add(issues, "localization.catalog.missing", "$.assets.localizationCatalog",
                "A project localization catalog is required.", ValidationSeverity.Warning);
            return;
        }

        foreach (IGrouping<LocalizationId, ProjectLocalizationRow> duplicate in
                 catalog.Rows.GroupBy(static row => row.Id).Where(static group => group.Count() > 1))
        {
            Add(issues, "localization.id.duplicate", $"localization[{duplicate.Key}]",
                "Localization IDs must be unique.");
        }

        Dictionary<LocalizationId, ProjectLocalizationRow> rows = catalog.Rows
            .GroupBy(static row => row.Id)
            .ToDictionary(static group => group.Key, static group => group.First());

        foreach (AchievementDefinition achievement in project.Achievements)
        {
            CheckRow(achievement.DisplayNameLocalizationId,
                new LocalizationOwner(ProjectEntityType.Achievement, achievement.Id.Value, LocalizationField.AchievementDisplayName),
                rows, issues);
            CheckRow(achievement.DescriptionLocalizationId,
                new LocalizationOwner(ProjectEntityType.Achievement, achievement.Id.Value, LocalizationField.AchievementDescription),
                rows, issues);
        }

        foreach (StatDefinition stat in project.Stats)
        {
            CheckRow(stat.DisplayNameLocalizationId,
                new LocalizationOwner(ProjectEntityType.Stat, stat.Id.Value, LocalizationField.StatDisplayName),
                rows, issues);
        }
    }

    private static void CheckRow(
        LocalizationId id,
        LocalizationOwner expectedOwner,
        Dictionary<LocalizationId, ProjectLocalizationRow> rows,
        List<ValidationIssue> issues)
    {
        if (!rows.TryGetValue(id, out ProjectLocalizationRow? row))
        {
            Add(issues, "localization.row.missing", $"localization[{id}]", "The referenced localization row is missing.");
            return;
        }

        if (string.IsNullOrWhiteSpace(row.English))
        {
            Add(issues, "localization.english.required", $"localization[{id}].English", "English text is required.");
        }

        if (row.Owner != expectedOwner)
        {
            Add(issues, "localization.owner", $"localization[{id}].owner", "The localization row has the wrong owner.");
        }
    }

    private static void ValidateReferences(
        AchimeldProject project,
        ProjectValidationContext context,
        List<ValidationIssue> issues)
    {
        if (!context.CheckReferencedFiles)
        {
            return;
        }

        if (string.IsNullOrWhiteSpace(context.ProjectFilePath))
        {
            Add(issues, "project.path.required", "$", "A project file path is required to validate referenced files.");
            return;
        }

        if (project.RequiresPrimaryAtlasFile)
        {
            CheckFile(project.Assets.Atlas, context.ProjectFilePath, "$.assets.atlas", issues, ValidationSeverity.Warning);
        }
        CheckFile(project.Assets.LocalizationCatalog, context.ProjectFilePath, "$.assets.localizationCatalog", issues);
        for (int index = 0; index < project.Assets.Atlases.Count; index++)
        {
            CheckFile(project.Assets.Atlases[index].Image, context.ProjectFilePath, $"$.assets.atlases[{index}].image", issues, ValidationSeverity.Warning);
        }

        for (int index = 0; index < project.Assets.Images.Count; index++)
        {
            CheckFile(project.Assets.Images[index].Image, context.ProjectFilePath, $"$.assets.images[{index}].image", issues, ValidationSeverity.Warning);
        }
    }

    private static void ValidateMaskReferences(IconRecipe recipe, string path, List<ValidationIssue> issues)
    {
        if (!recipe.Layers.Any(layer => layer.Kind == IconLayerKind.MaskReference || layer.MaskSourceLayerId is not null)) return;
        if (recipe.Layers.Count > 512 || recipe.Layers.Select(layer => layer.Id).Distinct().Count() != recipe.Layers.Count)
        {
            Add(issues, "achievement.icon.maskReference", path, "Reference masks require at most 512 uniquely identified layers.");
            return;
        }
        var layers = recipe.Layers.ToDictionary(layer => layer.Id);
        var masks = recipe.Layers.Where(layer => layer.MaskLayerId is not null).Select(layer => layer.MaskLayerId!.Value).ToHashSet();
        var done = new HashSet<IconLayerId>();
        var active = new HashSet<IconLayerId>();
        bool Visit(IconLayerId id)
        {
            if (done.Contains(id)) return true;
            if (!layers.TryGetValue(id, out var layer) || !active.Add(id) || active.Count > 65) return false;
            if (layer.MaskLayerId is { } mask && !Visit(mask)) return false;
            if (layer.MaskSourceLayerId is { } source && (layer.Kind != IconLayerKind.MaskReference || !Visit(source))) return false;
            if (layer.Kind == IconLayerKind.Folder)
                foreach (var child in recipe.Layers.Where(child => child.ParentFolderId == id && child.Visible && !masks.Contains(child.Id)))
                    if (!Visit(child.Id)) return false;
            active.Remove(id);
            done.Add(id);
            return true;
        }
        foreach (var layer in recipe.Layers)
            if (!Visit(layer.Id))
            {
                Add(issues, "achievement.icon.maskReference", path, "Mask references must resolve without cycles and stay within the nesting limit.");
                return;
            }
    }

    private static void CheckFile(
        PathReference reference,
        string projectFilePath,
        string path,
        List<ValidationIssue> issues,
        ValidationSeverity severity = ValidationSeverity.Error)
    {
        if (!File.Exists(reference.Resolve(projectFilePath)))
        {
            Add(issues, "asset.file.missing", path, "The referenced file does not exist.", severity);
        }
    }

    private static void AddDuplicates<T, TKey>(
        IEnumerable<T> values,
        Func<T, TKey> key,
        string path,
        string code,
        List<ValidationIssue> issues)
        where TKey : notnull
    {
        foreach (TKey duplicate in values.GroupBy(key).Where(static group => group.Count() > 1).Select(static group => group.Key))
        {
            Add(issues, code, path, $"Duplicate value '{duplicate}'.");
        }
    }

    private static void Add(
        List<ValidationIssue> issues,
        string code,
        string path,
        string message,
        ValidationSeverity severity = ValidationSeverity.Error) =>
        issues.Add(new ValidationIssue(code, path, message, severity));
}
