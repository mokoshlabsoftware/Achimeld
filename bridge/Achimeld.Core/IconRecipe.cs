using System.Globalization;

namespace Achimeld.Core;

public readonly record struct IconLayerId(Guid Value)
{
    public static IconLayerId New() => new(Guid.CreateVersion7());
}

public readonly record struct IconLayerEffectId(Guid Value)
{
    public static IconLayerEffectId New() => new(Guid.CreateVersion7());
}

public enum IconLayerKind { AtlasTile, StandaloneImage, SteamImportedArtwork, Text, Solid, Gradient, AchievementImage, SdfVignette, Folder, Shape, Curves, Contrast, HueSaturation, SelectiveColor, GradientOverlay, UnsharpMask, PerlinNoise, Text3d, Particle, Explosion, Artifact3d, RadialBlur, MaskReference }

public enum IconMaskMode { Alpha, Luminance }

public sealed record IconPerlinNoiseStyle(int Seed = 0, double Frequency = 4, int Octaves = 6,
    double Persistence = 0.5, double Lacunarity = 2, IReadOnlyList<bool>? EnabledOctaves = null,
    bool UseGradient = false, IconGradientStyle? Gradient = null)
{
    public string Fingerprint() => FormattableString.Invariant($"{Seed},{Frequency:G17},{Octaves},{Persistence:G17},{Lacunarity:G17}") +
        ":" + string.Join("", Enumerable.Range(0, Math.Clamp(Octaves, 0, 12)).Select(i => EnabledOctaves is null || i >= EnabledOctaves.Count || EnabledOctaves[i] ? "1" : "0")) +
        (UseGradient ? $":gradient={Gradient?.Fingerprint()}" : "");
}

public enum IconSelectiveColorRange { Reds, Yellows, Greens, Cyans, Blues, Magentas, Whites, Neutrals, Blacks }
public sealed record IconSelectiveColorCorrection(IconSelectiveColorRange Range, double Cyan = 0, double Magenta = 0, double Yellow = 0, double Black = 0);
public sealed record IconAdjustmentStyle(double Brightness = 0, double Contrast = 0, double Hue = 0,
    double Saturation = 0, double Lightness = 0, IconSelectiveColorRange ColorRange = IconSelectiveColorRange.Reds,
    double Cyan = 0, double Magenta = 0, double Yellow = 0, double Black = 0, IReadOnlyList<IconSelectiveColorCorrection>? SelectiveColors = null,
    double UnsharpAmount = 100, double UnsharpRadius = 1, int UnsharpThreshold = 0,
    double RadialBlur = 0, double AngularBlur = 0)
{
    public IReadOnlyList<IconSelectiveColorCorrection> EffectiveSelectiveColors => SelectiveColors ?? [new(ColorRange,Cyan,Magenta,Yellow,Black)];
    public string Fingerprint() => string.Join("/",EffectiveSelectiveColors.Select(c => FormattableString.Invariant($"{c.Range},{c.Cyan:G17},{c.Magenta:G17},{c.Yellow:G17},{c.Black:G17}"))) + FormattableString.Invariant($"{Brightness:G17},{Contrast:G17},{Hue:G17},{Saturation:G17},{Lightness:G17},{ColorRange},{Cyan:G17},{Magenta:G17},{Yellow:G17},{Black:G17},{UnsharpAmount:G17},{UnsharpRadius:G17},{UnsharpThreshold}") +
        (RadialBlur == 0 && AngularBlur == 0 ? "" : FormattableString.Invariant($"/radialBlur={RadialBlur:G17},{AngularBlur:G17}"));
}

public sealed record IconCurvePoint(double Input, double Output);
public sealed record IconCurvesStyle(IReadOnlyList<IconCurvePoint> Points)
{
    public static IconCurvesStyle Default() => new([new(0, 0), new(1, 1)]);
    public double Map(double input)
    {
        var points = Points.Where(p => double.IsFinite(p.Input) && double.IsFinite(p.Output)).OrderBy(p => p.Input).DistinctBy(p => p.Input).ToArray();
        if (points.Length == 0) return input;
        if (input <= points[0].Input) return Math.Clamp(points[0].Output, 0, 1);
        for (int i = 1; i < points.Length; i++)
            if (input <= points[i].Input)
            {
                var p1 = points[i-1]; var p2 = points[i];
                var p0 = points[Math.Max(0,i-2)]; var p3 = points[Math.Min(points.Length-1,i+1)];
                double span = p2.Input-p1.Input, t = (input-p1.Input)/span;
                // Catmull–Rom expressed as Hermite interpolation over input intensity.
                // Endpoint tangents use the adjacent secant; no tangent handles are needed.
                double m1 = (p2.Output-p0.Output)/(p2.Input-p0.Input)*span;
                double m2 = (p3.Output-p1.Output)/(p3.Input-p1.Input)*span;
                return Math.Clamp((2*t*t*t-3*t*t+1)*p1.Output + (t*t*t-2*t*t+t)*m1
                    + (-2*t*t*t+3*t*t)*p2.Output + (t*t*t-t*t)*m2,0,1);
            }
        return Math.Clamp(points[^1].Output, 0, 1);
    }
    public string Fingerprint() => string.Join("/", Points.Select(p => FormattableString.Invariant($"{p.Input:G17},{p.Output:G17}")));
}

public enum IconTextAlignment { Center, TopLeft, TopRight, BottomLeft, BottomRight }

public enum IconOutlineShape { Square, Round }

public enum IconOutlinePlacement { Inward, Center, Outward }

public enum IconLayerEffectKind { ColorOverlay, Outline, Shadow, InnerLight, BoxBlur, GradientOverlay, Skew, Perspective, Pinch, LensBlur, OuterLight }

public enum IconBlendMode
{
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Add,
    Difference,
}

public enum IconShapeKind { Rectangle, Sphere, Polygon, Star }

public sealed record IconShapeStyle(IconShapeKind Kind = IconShapeKind.Rectangle,
    string ColorHex = "#FFFFFFFF", double ScaleX = 0.5, double ScaleY = 0.5, double Rotation = 0,
    int VertexCount = 5, double InnerRadius = 0.5, double OuterRounding = 0, double InnerRounding = 0)
{
    public string Fingerprint() => FormattableString.Invariant($"{Kind},{ColorHex},{ScaleX:G17},{ScaleY:G17},{Rotation:G17},{VertexCount},{InnerRadius:G17},{OuterRounding:G17},{InnerRounding:G17}");
}

public enum IconGradientKind { Linear, Radial, Polygon, Star, SDF }
public enum IconGradientInputChannel { RGB, Red, Green, Blue }

public sealed record IconTextStyle(
    string Content,
    string FontFamily,
    double FontSize,
    string ColorHex,
    bool OutlineEnabled = false,
    string OutlineColorHex = "#000000",
    double OutlineWidth = 2,
    IconTextAlignment Alignment = IconTextAlignment.BottomRight,
    IconOutlinePlacement OutlinePlacement = IconOutlinePlacement.Center,
    bool LinkToAchievementValue = false)
{
    public const double DefaultFontSize = 128;

    public const double DefaultOutlineWidth = 14;

    public static IconTextStyle Default(string content = "") =>
        new(content, "Segoe UI", DefaultFontSize, "#FFFFFF", true, "#000000", DefaultOutlineWidth,
            IconTextAlignment.BottomRight, IconOutlinePlacement.Center, false);
}

public sealed record IconSdfVignetteStyle(string ColorHex = "#FF000000", double Roundness = 0.5,
    double Inset = 0.15, double Softness = 0.2, bool Invert = false,
    bool UseGradient = false, IconGradientStyle? Gradient = null)
{
    public string Fingerprint() =>
        FormattableString.Invariant($"{ColorHex},{Roundness:G17},{Inset:G17},{Softness:G17}") +
        (Invert ? ",invert" : "") + (UseGradient ? $",gradient:{Gradient?.Fingerprint()}" : "");
}

public sealed record IconGradientStop(double Offset, string ColorHex);

public sealed record IconGradientStyle(
    IconGradientKind Kind,
    double Angle,
    IReadOnlyList<IconGradientStop> Stops,
    IconGradientInputChannel InputChannel = IconGradientInputChannel.RGB,
    int VertexCount = 5, double InnerRadius = 0.5, double OuterRounding = 0, double InnerRounding = 0)
{
    public static IconGradientStyle Default() =>
        new(
            IconGradientKind.Linear,
            90,
            [new IconGradientStop(0, "#FF000000"), new IconGradientStop(1, "#FFFFFFFF")]);

    public string Fingerprint() =>
        string.Join(
            ",",
            Kind,
            InputChannel,
            FormattableString.Invariant($"{VertexCount},{InnerRadius:G17},{OuterRounding:G17},{InnerRounding:G17}"),
            Angle.ToString("G17", CultureInfo.InvariantCulture),
            string.Join(
                "/",
                Stops.Select(stop =>
                    string.Concat(
                        stop.Offset.ToString("G17", CultureInfo.InvariantCulture),
                        "=",
                        stop.ColorHex))));
}

public sealed record IconLayerEffect(
    IconLayerEffectId Id,
    IconLayerEffectKind Kind,
    bool Visible = true,
    string ColorHex = "#FF000000",
    double Width = 2,
    IconOutlinePlacement Placement = IconOutlinePlacement.Center,
    double OffsetX = 2,
    double OffsetY = 2,
    double Blur = 2,
    IconBlendMode BlendMode = IconBlendMode.Normal,
    double Opacity = 1,
    bool BlurEnabled = true,
    string BlurColorHex = "#FFFFFFFF",
    IconBlendMode BlurBlendMode = IconBlendMode.Screen,
    double BlurOpacity = 1,
    IconOutlineShape OutlineShape = IconOutlineShape.Square,
    bool AffectOutline = false,
    double Cutout = 0, IconGradientStyle? Gradient = null,
    double SkewX = 0, double SkewY = 0, double PerspectiveX = 0, double PerspectiveY = 0,
    double Amount = 0, double Aberration = 0)
{
    public static IconLayerEffect Skew(double horizontal = 0, double vertical = 0) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.Skew, SkewX: horizontal, SkewY: vertical);

    public static IconLayerEffect Perspective(double horizontal = 0, double vertical = 0) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.Perspective, PerspectiveX: horizontal, PerspectiveY: vertical);

    public static IconLayerEffect Pinch(double amount = 0.5) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.Pinch, Amount: amount);

    public static IconLayerEffect LensBlur(double blur = 4, double aberration = 8) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.LensBlur, Blur: blur, Aberration: aberration);

    public static IconLayerEffect GradientOverlay(IconGradientStyle? gradient = null) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.GradientOverlay, Gradient: gradient ?? IconGradientStyle.Default());

    public static IconLayerEffect BoxBlur(double radius = 2) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.BoxBlur, Blur: radius, AffectOutline: true);

    public static IconLayerEffect InnerLight() =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.InnerLight, ColorHex: "#FFFFFFFF", BlendMode: IconBlendMode.Screen,
            OffsetX: 0, OffsetY: 8, Blur: 12);

    public static IconLayerEffect OuterLight() =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.OuterLight, ColorHex: "#FFFFFFFF", BlendMode: IconBlendMode.Screen,
            Width: 4, Blur: 12);

    public static IconLayerEffect ColorOverlay(string colorHex = "#FFFFFFFF") =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.ColorOverlay, ColorHex: colorHex);

    public static IconLayerEffect Outline(
        string colorHex = "#FF000000",
        double width = 2,
        IconOutlinePlacement placement = IconOutlinePlacement.Center,
        IconOutlineShape shape = IconOutlineShape.Square) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.Outline, ColorHex: colorHex,
            Width: width, Placement: placement, OutlineShape: shape);

    public static IconLayerEffect Shadow(
        string colorHex = "#80000000",
        double offsetX = 2,
        double offsetY = 2,
        double blur = 2) =>
        new(IconLayerEffectId.New(), IconLayerEffectKind.Shadow, ColorHex: colorHex,
            OffsetX: offsetX, OffsetY: offsetY, Blur: blur);
}

public sealed record IconLayer(
    IconLayerId Id,
    string DisplayName,
    IconLayerKind Kind,
    bool Visible = true,
    AtlasId? AtlasId = null,
    int TileIndex = 0,
    ImageAssetId? ImageId = null,
    IconTextStyle? Text = null,
    IconBlendMode BlendMode = IconBlendMode.Normal,
    string? SolidColorHex = null,
    IconGradientStyle? Gradient = null,
    double Scale = 1,
    bool LinkAtlasToGeneral = false,
    double OffsetX = 0,
    double OffsetY = 0,
    double Opacity = 1,
    IReadOnlyList<IconLayerEffect>? Effects = null,
    int GeneralImageIndex = 0, IconSdfVignetteStyle? SdfVignette = null, IconLayerId? ParentFolderId = null, IconLayerId? MaskLayerId = null, IconShapeStyle? Shape = null, IconCurvesStyle? Curves = null, IconAdjustmentStyle? Adjustment = null, double Rotation = 0,
    IconPerlinNoiseStyle? PerlinNoise = null, IconMaskMode MaskMode = IconMaskMode.Alpha, bool MaskInvert = false,
    [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    IconText3dStyle? Text3d = null, IconParticleStyle? Particles = null,
    [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    IconExplosionStyle? Explosion = null,
    [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    IconArtifact3dStyle? Artifact3d = null,
    [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    IconLayerId? MaskSourceLayerId = null)
{
    public IReadOnlyList<IconLayerEffect> EffectiveEffects => Effects ?? [];

    public static IconLayer Atlas(AtlasId? atlasId, int tileIndex, string displayName = "Atlas tile") =>
        new(IconLayerId.New(), displayName, IconLayerKind.AtlasTile, true, atlasId, tileIndex);

    public static IconLayer Image(ImageAssetId imageId, string displayName = "Image") =>
        new(IconLayerId.New(), displayName, IconLayerKind.StandaloneImage, true, ImageId: imageId);

    public static IconLayer AchievementImage(string displayName = "General image") =>
        new(IconLayerId.New(), displayName, IconLayerKind.AchievementImage);

    public static IconLayer SteamArtwork(ImageAssetId imageId, string displayName = "Imported Steam artwork") =>
        new(IconLayerId.New(), displayName, IconLayerKind.SteamImportedArtwork, true, ImageId: imageId);

    public static IconLayer FromText(IconTextStyle? style = null, string displayName = "Text") =>
        new(IconLayerId.New(), displayName, IconLayerKind.Text, true, Text: style ?? IconTextStyle.Default());

    public static IconLayer FromText3d(IconTextStyle? text = null, IconText3dStyle? settings = null) =>
        new(IconLayerId.New(), "3D text", IconLayerKind.Text3d,
            Text: text ?? IconTextStyle.Default("Text") with { OutlineEnabled = false, Alignment = IconTextAlignment.Center },
            Text3d: settings ?? new());

    public static IconLayer FromArtifact3d(IconArtifact3dStyle? settings = null) =>
        new(IconLayerId.New(), "3D artifact", IconLayerKind.Artifact3d, Artifact3d: settings ?? new());

    public static IconLayer Solid(string colorHex = "#FFFFFFFF", string displayName = "Solid") =>
        new(IconLayerId.New(), displayName, IconLayerKind.Solid, true, SolidColorHex: colorHex);

    public static IconLayer FromShape(IconShapeStyle? shape = null) =>
        new(IconLayerId.New(), "Shape", IconLayerKind.Shape, Shape: shape ?? new());

    public static IconLayer FromSdfVignette() =>
        new(IconLayerId.New(), "SDF vignette", IconLayerKind.SdfVignette, SdfVignette: new());

    public static IconLayer FromParticles() =>
        new(IconLayerId.New(), "Particles", IconLayerKind.Particle, Particles: new() { ColorMap = new() });

    public static IconLayer FromExplosion() =>
        new(IconLayerId.New(), "Explosion", IconLayerKind.Explosion, Explosion: new() { Deltoid = new(), CoverCanvas = true, ScaleMin = 1, ScaleMax = 1 });

    public static IconLayer FromPerlinNoise() =>
        new(IconLayerId.New(), "Perlin noise", IconLayerKind.PerlinNoise, PerlinNoise: new());

    public static IconLayer FromGradient(IconGradientStyle? style = null, string displayName = "Gradient") =>
        new(IconLayerId.New(), displayName, IconLayerKind.Gradient, true, Gradient: style ?? IconGradientStyle.Default());
}

public sealed record IconRecipe(
    IReadOnlyList<IconLayer> Layers,
    string? BackgroundColorHex = null)
{
    public static IconRecipe FromLegacy(int tileIndex, AtlasId? atlasId, ImageAssetId? standaloneImageId) =>
        standaloneImageId is ImageAssetId imageId
            ? new([IconLayer.Image(imageId)])
            : new([IconLayer.Atlas(atlasId, tileIndex)]);

    public bool IsSimpleLegacy(int tileIndex, AtlasId? atlasId, ImageAssetId? standaloneImageId)
    {
        if (!string.IsNullOrWhiteSpace(BackgroundColorHex) || Layers.Count != 1)
        {
            return false;
        }

        IconLayer layer = Layers[0];
        if (!layer.Visible
            || layer.BlendMode != IconBlendMode.Normal
            || layer.Scale != 1
            || layer.OffsetX != 0
            || layer.OffsetY != 0
            || layer.Rotation != 0
            || layer.Opacity != 1
            || layer.EffectiveEffects.Count != 0
            || layer.LinkAtlasToGeneral
            || layer.Text is not null
            || layer.Text3d is not null
            || layer.Artifact3d is not null
            || layer.SolidColorHex is not null
            || layer.Gradient is not null)
        {
            return false;
        }

        if (standaloneImageId is ImageAssetId imageId)
        {
            return layer.Kind == IconLayerKind.StandaloneImage
                && layer.ImageId == imageId
                && layer.AtlasId is null
                && layer.TileIndex == 0;
        }

        return layer.Kind == IconLayerKind.AtlasTile
            && layer.TileIndex == tileIndex
            && layer.AtlasId == atlasId
            && layer.ImageId is null;
    }

    public static bool TryParseRgba(string? value, out byte alpha, out byte red, out byte green, out byte blue)
    {
        alpha = 255;
        red = 0;
        green = 0;
        blue = 0;
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        string hex = value.Trim();
        if (hex[0] == '#')
        {
            hex = hex[1..];
        }

        if (hex.Length is not (6 or 8))
        {
            return false;
        }

        if (!uint.TryParse(hex, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out uint packed))
        {
            return false;
        }

        if (hex.Length == 8)
        {
            alpha = (byte)((packed >> 24) & 0xFF);
            red = (byte)((packed >> 16) & 0xFF);
            green = (byte)((packed >> 8) & 0xFF);
            blue = (byte)(packed & 0xFF);
            return true;
        }

        red = (byte)((packed >> 16) & 0xFF);
        green = (byte)((packed >> 8) & 0xFF);
        blue = (byte)(packed & 0xFF);
        return true;
    }
}
