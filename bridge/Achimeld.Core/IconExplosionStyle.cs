using System.Globalization;
using System.Text.Json.Serialization;

namespace Achimeld.Core;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record IconExplosionDeltoid
{
    public double Ellipse { get; init; }
    public double Scale { get; init; } = 1;
    public double ScaleMin { get; init; } = 1;
    public double ScaleMax { get; init; } = 1;
    public double WidthMin { get; init; } = .12;
    public double WidthMax { get; init; } = .35;
    public double TopHeightMin { get; init; } = .2;
    public double TopHeightMax { get; init; } = .45;
    public double BottomHeightMin { get; init; } = .2;
    public double BottomHeightMax { get; init; } = .45;
    public double OffsetMin { get; init; }
    public double OffsetMax { get; init; }
    public double SharpnessMin { get; init; } = .8;
    public double SharpnessMax { get; init; } = 1;
    public bool ValidParameters() => IconExplosionTriangle.Range(Ellipse, Ellipse, 0, 1) &&
        IconExplosionTriangle.Range(Scale, Scale, .01, 2) &&
        IconExplosionTriangle.Range(ScaleMin, ScaleMax, .01, 2) &&
        IconExplosionTriangle.Range(WidthMin, WidthMax, 0, 2) &&
        IconExplosionTriangle.Range(TopHeightMin, TopHeightMax, 0, 1) &&
        IconExplosionTriangle.Range(BottomHeightMin, BottomHeightMax, 0, 1) &&
        IconExplosionTriangle.Range(OffsetMin, OffsetMax, -1.5, 1.5) &&
        IconExplosionTriangle.Range(SharpnessMin, SharpnessMax, 0, 1);
    public string Fingerprint() => string.Join(",", new[] { Scale, ScaleMin, ScaleMax, WidthMin, WidthMax,
        TopHeightMin, TopHeightMax, BottomHeightMin, BottomHeightMax, OffsetMin, OffsetMax,
        SharpnessMin, SharpnessMax }.Select(v => v.ToString("G17", CultureInfo.InvariantCulture))) +
        (Ellipse == 0 ? "" : ";ellipse=" + Ellipse.ToString("G17", CultureInfo.InvariantCulture));
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record IconExplosionTriangle
{
    public bool Flip { get; init; }
    public double Scale { get; init; } = 1;
    public double ScaleMin { get; init; } = 1;
    public double ScaleMax { get; init; } = 1;
    public double WidthMin { get; init; } = .12;
    public double WidthMax { get; init; } = .35;
    public double HeightMin { get; init; } = .2;
    public double HeightMax { get; init; } = .45;
    public double OffsetMin { get; init; }
    public double OffsetMax { get; init; }
    public double SharpnessMin { get; init; } = .8;
    public double SharpnessMax { get; init; } = 1;
    public bool ValidParameters() => Range(Scale, Scale, .01, 2) && Range(ScaleMin, ScaleMax, .01, 2) &&
        Range(WidthMin, WidthMax, 0, 2) && Range(HeightMin, HeightMax, 0, 1) &&
        Range(OffsetMin, OffsetMax, -.5, .5) && Range(SharpnessMin, SharpnessMax, 0, 1);
    internal static bool Range(double low, double high, double min, double max) =>
        double.IsFinite(low) && double.IsFinite(high) && low >= min && high <= max && low <= high;
    public string Fingerprint() => string.Join(",", new[] { WidthMin, WidthMax, HeightMin, HeightMax,
        OffsetMin, OffsetMax, SharpnessMin, SharpnessMax }.Select(v => v.ToString("G17", CultureInfo.InvariantCulture))) +
        (Flip || Scale != 1 || ScaleMin != 1 || ScaleMax != 1
            ? $";flip={Flip};scale=" + string.Join(",", new[] { Scale, ScaleMin, ScaleMax }.Select(v => v.ToString("G17", CultureInfo.InvariantCulture))) : "");
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed record IconExplosionStyle
{
    public int Count { get; init; } = 9;
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? CountMin { get; init; }
    public uint Seed { get; init; } = 1;
    public bool DifferentSeedPerInstance { get; init; }
    public double Regularity { get; init; } = .35;
    public double AspectMin { get; init; } = 3;
    public double AspectMax { get; init; } = 5;
    public double ScaleMin { get; init; } = .9;
    public double ScaleMax { get; init; } = .9;
    public double ShapeX { get; init; } = 1;
    public double ShapeY { get; init; } = 1;
    public double BottomPower { get; init; }
    public double BottomStretch { get; init; } = .5;
    public string BottomCurve { get; init; } = "Linear";
    public double RadialBlur { get; init; }
    public double AngularBlur { get; init; }
    public double Brightness { get; init; } = 1;
    public double BrightnessMin { get; init; } = 1;
    public double BrightnessMax { get; init; } = 1;
    public bool InvertColors { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public IconExplosionDeltoid? Deltoid { get; init; }
    public bool CoverCanvas { get; init; }
    public bool DisableOut { get; init; }
    public bool DisableIn { get; init; }
    public IconExplosionTriangle Out { get; init; } = new();
    public IconExplosionTriangle In { get; init; } = new();
    public IconGradientStyle Gradient { get; init; } = new(IconGradientKind.Radial, 90,
        [new(0, "#FF000000"), new(1, "#FFFFFFFF")]);

    public bool ValidParameters() => Count is >= 1 and <= 512 &&
        (CountMin is null || CountMin >= 1 && CountMin <= Count) &&
        IconExplosionTriangle.Range(AspectMin, AspectMax, 1, 10) &&
        IconExplosionTriangle.Range(ScaleMin, ScaleMax, .01, 2) &&
        IconExplosionTriangle.Range(ShapeX, ShapeX, 1, 10) &&
        IconExplosionTriangle.Range(ShapeY, ShapeY, 1, 10) &&
        IconExplosionTriangle.Range(BottomPower, BottomPower, 0, 9) &&
        IconExplosionTriangle.Range(BottomStretch, BottomStretch, 0, 1) &&
        BottomCurve is "Linear" or "Quadratic" or "Logarithmic" &&
        IconExplosionTriangle.Range(RadialBlur, RadialBlur, 0, 1) &&
        IconExplosionTriangle.Range(AngularBlur, AngularBlur, 0, 1) &&
        IconExplosionTriangle.Range(Brightness, Brightness, 0, 2) &&
        IconExplosionTriangle.Range(BrightnessMin, BrightnessMax, 0, 2) &&
        IconExplosionTriangle.Range(Regularity, Regularity, 0, 1) &&
        Out is not null && In is not null && Out.ValidParameters() && In.ValidParameters() &&
        (Deltoid is null || Deltoid.ValidParameters()) &&
        Gradient?.Kind is IconGradientKind.Linear or IconGradientKind.Radial or IconGradientKind.SDF &&
        Gradient.Stops is { Count: >= 2 and <= 128 };

    public string Fingerprint() => string.Join("/", Count.ToString(CultureInfo.InvariantCulture),
        CountMin?.ToString(CultureInfo.InvariantCulture) ?? "exact", Seed.ToString(CultureInfo.InvariantCulture),
        DifferentSeedPerInstance, DisableOut, DisableIn,
        string.Join(",", new[] { Regularity, AspectMin, AspectMax, ScaleMin, ScaleMax }
            .Select(v => v.ToString("G17", CultureInfo.InvariantCulture))),
        Out?.Fingerprint(), In?.Fingerprint(), Gradient?.Fingerprint()) +
        (Deltoid is null ? "" : "/deltoid=" + Deltoid.Fingerprint()) +
        (CoverCanvas ? "/coverCanvas" : "") +
        (BottomPower == 0 && BottomStretch == .5 && BottomCurve == "Linear" ? "" :
            "/bottomScale=" + string.Join(",", new[] { BottomPower, BottomStretch }
                .Select(v => v.ToString("G17", CultureInfo.InvariantCulture))) + ";curve=" + BottomCurve) +
        (ShapeX == 1 && ShapeY == 1 ? "" : "/shapeScale=" + string.Join(",", new[] { ShapeX, ShapeY }
            .Select(v => v.ToString("G17", CultureInfo.InvariantCulture)))) +
        (RadialBlur == 0 && AngularBlur == 0 ? "" : "/blur=" + string.Join(",", new[] { RadialBlur, AngularBlur }
            .Select(v => v.ToString("G17", CultureInfo.InvariantCulture)))) +
        (Brightness == 1 && BrightnessMin == 1 && BrightnessMax == 1 && !InvertColors ? "" :
            "/brightness=" + string.Join(",", new[] { Brightness, BrightnessMin, BrightnessMax }
                .Select(v => v.ToString("G17", CultureInfo.InvariantCulture))) + ";invert=" + InvertColors);
}
