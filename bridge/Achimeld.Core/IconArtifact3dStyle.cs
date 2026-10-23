namespace Achimeld.Core;

/// <summary>Editable primitive collection; all dimensions use 512-unit icon space.</summary>
public sealed record IconArtifact3dStyle
{
    public IReadOnlyList<IconArtifactPart> Parts { get; init; } = Person();
    public string Output { get; init; } = "FullColor";
    public string Projection { get; init; } = "Perspective";
    public double CameraDistance { get; init; } = 800;
    public double FocalLength { get; init; } = 50;
    public double OrthographicSize { get; init; } = 512;
    public IReadOnlyList<double> Rotation { get; init; } = new double[] { 12, -20, 0 };
    public IReadOnlyList<double> Position { get; init; } = new double[] { 0, 0, 0 };
    public double Ambient { get; init; } = 0.3;
    public double LightIntensity { get; init; } = 0.8;
    public double LightAzimuth { get; init; } = -35;
    public double LightElevation { get; init; } = 45;

    public bool IsValid() => Parts is not null && Parts.Count <= 64 &&
        Parts.All(p => p is not null && p.IsValid()) && Parts.Select(p => p.Id).Distinct(StringComparer.OrdinalIgnoreCase).Count() == Parts.Count &&
        Output is "FullColor" or "AlphaMask" && Projection is "Perspective" or "Orthographic" &&
        Vector(Rotation, -360, 360) && Vector(Position, -4096, 4096) &&
        Bound(CameraDistance, 32, 8192) && Bound(FocalLength, 10, 300) && Bound(OrthographicSize, 32, 8192) &&
        Bound(Ambient, 0, 1) && Bound(LightIntensity, 0, 2) && Bound(LightAzimuth, -180, 180) && Bound(LightElevation, -90, 90);

    // JSON encodes names and all ordered part fields unambiguously, including
    // hidden parts and every material/geometry value used by local rendering.
    public string Fingerprint() => System.Text.Json.JsonSerializer.Serialize(this);
    internal static bool Bound(double n, double min, double max) => double.IsFinite(n) && n >= min && n <= max;
    internal static bool Vector(IReadOnlyList<double>? v, double min, double max) => v is { Count: 3 } && v.All(n => Bound(n, min, max));

    private static IReadOnlyList<IconArtifactPart> Person()
    {
        IconArtifactPart Part(int id, string name, string primitive, double[] size, double[] position, string color, double rounding = 0, double z = 0) => new()
        {
            Id = $"00000000-0000-4000-8000-{id:D12}", Name = name, Primitive = primitive, Size = size,
            Position = position, ColorHex = color, Rounding = rounding, Rotation = new double[] { 0, 0, z }
        };
        return new[] {
            Part(1,"Head","Sphere",[82,82,82],[0,135,0],"#FFCCD9E8"),
            Part(2,"Torso","Cylinder",[108,140,66],[0,25,0],"#FF4E91CF",0.8),
            Part(3,"Left hand","Cylinder",[36,135,36],[-78,25,0],"#FFCCD9E8",0.8,-12),
            Part(4,"Right hand","Cylinder",[36,135,36],[78,25,0],"#FFCCD9E8",0.8,12),
            Part(5,"Left foot","Cylinder",[42,136,48],[-32,-108,5],"#FFCCD9E8",0.8),
            Part(6,"Right foot","Cylinder",[42,136,48],[32,-108,5],"#FFCCD9E8",0.8)
        };
    }
}

public sealed record IconArtifactPart
{
    public string Id { get; init; } = "00000000-0000-4000-8000-000000000001";
    public string Name { get; init; } = "Cube";
    public string Primitive { get; init; } = "Cube";
    public bool Visible { get; init; } = true;
    public IReadOnlyList<double> Size { get; init; } = new double[] { 120, 120, 120 };
    public IReadOnlyList<double> Position { get; init; } = new double[] { 0, 0, 0 };
    public IReadOnlyList<double> Rotation { get; init; } = new double[] { 0, 0, 0 };
    public string ColorHex { get; init; } = "#FFD1D9E8";
    public string Shading { get; init; } = "Glossy";
    public double Roughness { get; init; } = 0.3;
    public double Reflectivity { get; init; } = 0.55;
    public double Rounding { get; init; }
    public double StartAngle { get; init; }
    public double SweepAngle { get; init; } = 360;
    public double CutBottom { get; init; } = -1;
    public double CutTop { get; init; } = 1;
    public double WallThickness { get; init; }
    public double TubeRadius { get; init; } = 0.22;

    public bool IsValid() => Guid.TryParseExact(Id, "D", out Guid id) && id != Guid.Empty && Name is not null && Name.EnumerateRunes().Count() <= 128 &&
        Primitive is "Cube" or "Cone" or "Torus" or "Cylinder" or "Sphere" && Shading is "Matte" or "Glossy" or "Unlit" &&
        IconRecipe.TryParseRgba(ColorHex, out _, out _, out _, out _) &&
        IconArtifact3dStyle.Vector(Size, 0.1, 2048) && IconArtifact3dStyle.Vector(Position, -4096, 4096) && IconArtifact3dStyle.Vector(Rotation, -360, 360) &&
        IconArtifact3dStyle.Bound(Roughness, 0, 1) && IconArtifact3dStyle.Bound(Reflectivity, 0, 1) && IconArtifact3dStyle.Bound(Rounding, 0, 1) &&
        IconArtifact3dStyle.Bound(StartAngle, -360, 360) && IconArtifact3dStyle.Bound(SweepAngle, 0.5, 360) &&
        IconArtifact3dStyle.Bound(CutBottom, -1, 1) && IconArtifact3dStyle.Bound(CutTop, -1, 1) && CutTop - CutBottom >= 0.001 &&
        IconArtifact3dStyle.Bound(WallThickness, 0, 0.95) && IconArtifact3dStyle.Bound(TubeRadius, 0.01, 0.95);
}
