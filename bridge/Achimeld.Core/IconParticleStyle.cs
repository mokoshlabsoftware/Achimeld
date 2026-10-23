using System.Globalization;

namespace Achimeld.Core;

/// <summary>Portable static particle recipe; diameters are in 512-unit artwork space.</summary>
public sealed record IconParticleStyle
{
    public int Count { get; init; } = 48;
    public uint Seed { get; init; } = 1;
    public bool DifferentSeedPerInstance { get; init; }
    public string Shape { get; init; } = "Sphere";
    public IReadOnlyList<IconParticleShape> Shapes { get; init; } = [];
    public int VertexCount { get; init; } = 6;
    public double InnerRadius { get; init; } = .45;
    public double? InnerRadiusMin { get; init; }
    public double ShapeX { get; init; } = 1;
    public double ShapeY { get; init; } = 1;
    public double Regularity { get; init; } = 1;
    public double CenterX { get; init; } = .5;
    public double CenterY { get; init; } = .5;
    public double Radius { get; init; } = .42;
    public double RadiusMin { get; init; }
    public double EdgeBias { get; init; } = .5;
    public double SizeMin { get; init; } = 8;
    public double SizeMax { get; init; } = 40;
    public double RotationMin { get; init; } = -180;
    public double RotationMax { get; init; } = 180;
    public double OpacityMin { get; init; } = .35;
    public double OpacityMax { get; init; } = 1;
    public double Softness { get; init; } = .3;
    public double? SoftnessMin { get; init; }
    public string ColorMode { get; init; } = "Particle";
    public IconGradientMap? ColorMap { get; init; }
    public IconGradientStyle Gradient { get; init; } = new(IconGradientKind.Radial, 90,
        [new(0, "#FFFFFFFF"), new(.45, "#FFFFCF70"), new(1, "#00FF7040")]);

    public bool ValidParameters() => Count is >= 0 and <= 512 && VertexCount is >= 3 and <= 16 &&
        Shape is "Sphere" or "Polygon" or "Star" && ColorMode is "Particle" or "Spawn radius" or "Random" &&
        In(InnerRadius, .05, 1) && In(CenterX, 0, 1) && In(CenterY, 0, 1) && In(Radius, 0, 1) &&
        In(InnerRadiusMin ?? InnerRadius, .05, InnerRadius) && In(ShapeX, 1, 10) && In(ShapeY, 1, 10) && In(Regularity, 0, 1) &&
        In(RadiusMin, 0, Radius) && Shapes is { Count: <= 16 } && Shapes.All(s => s is not null && s.Valid()) &&
        In(EdgeBias, 0, 1) && In(SizeMin, 1, 512) && In(SizeMax, SizeMin, 512) &&
        In(RotationMin, -360, 360) && In(RotationMax, RotationMin, 360) &&
        In(OpacityMin, 0, 1) && In(OpacityMax, OpacityMin, 1) && In(Softness, 0, 1) &&
        In(SoftnessMin ?? Softness, 0, Softness) && (ColorMap is null || ColorMap.Valid()) && Gradient?.Stops is { Count: >= 2 and <= 128 };

    private static bool In(double value, double min, double max) => double.IsFinite(value) && value >= min && value <= max;

    public string Fingerprint() => string.Join(",", Count, Seed, DifferentSeedPerInstance, Shape, VertexCount, ColorMode,
        string.Join(",", new[] { InnerRadius, CenterX, CenterY, Radius, EdgeBias, SizeMin, SizeMax,
            RotationMin, RotationMax, OpacityMin, OpacityMax, Softness, SoftnessMin ?? Softness, RadiusMin }.Select(v => v.ToString("G17", CultureInfo.InvariantCulture))),
        string.Join(";", Shapes.Select(s => s.Fingerprint())),
        Gradient?.Fingerprint(), ColorMap?.Fingerprint()) +
        (ShapeX == 1 && ShapeY == 1 ? "" : ";shapeScale=" + string.Join(",", new[] { ShapeX, ShapeY }.Select(v => v.ToString("G17", CultureInfo.InvariantCulture)))) +
        (Regularity == 1 ? "" : ";regularity=" + Regularity.ToString("G17", CultureInfo.InvariantCulture)) +
        (InnerRadiusMin is null || InnerRadiusMin == InnerRadius ? "" : ";innerRadiusMin=" + InnerRadiusMin.Value.ToString("G17", CultureInfo.InvariantCulture));
}

public sealed record IconParticleShape
{
    public string Kind { get; init; } = "Sphere";
    public string Operation { get; init; } = "Union";
    public double Scale { get; init; } = 1;
    public double OffsetX { get; init; }
    public double OffsetY { get; init; }
    public double Rotation { get; init; }
    public int VertexCount { get; init; } = 6;
    public double InnerRadius { get; init; } = .45;
    public double? InnerRadiusMin { get; init; }
    public bool Valid() => Kind is "Sphere" or "Polygon" or "Star" && Operation is "Union" or "Subtract" or "Intersect" &&
        VertexCount is >= 3 and <= 16 && In(Scale,.05,2) && In(OffsetX,-1,1) && In(OffsetY,-1,1) && In(Rotation,-360,360) && In(InnerRadius,.05,1) && In(InnerRadiusMin ?? InnerRadius,.05,InnerRadius);
    private static bool In(double v,double min,double max) => double.IsFinite(v) && v >= min && v <= max;
    public string Fingerprint() => $"{Kind},{Operation},{VertexCount}," + string.Join(",", new[] { Scale, OffsetX, OffsetY, Rotation, InnerRadius }.Select(v => v.ToString("G17",CultureInfo.InvariantCulture))) +
        (InnerRadiusMin is null || InnerRadiusMin == InnerRadius ? "" : ";innerRadiusMin=" + InnerRadiusMin.Value.ToString("G17", CultureInfo.InvariantCulture));
}
