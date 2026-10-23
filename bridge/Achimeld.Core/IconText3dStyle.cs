namespace Achimeld.Core;

/// <summary>Extruded text in 512-unit icon space. Lens uses a 36 mm vertical gate.</summary>
public sealed record IconText3dStyle(
    string Projection = "Perspective", double CameraDistance = 800, double FocalLength = 50,
    double OrthographicSize = 512, double Extrusion = 32,
    double RotationX = 15, double RotationY = -25, double RotationZ = 0,
    double PositionX = 0, double PositionY = 0, double PositionZ = 0,
    string Shading = "Matte", string SideColorHex = "#FFB8C4D8",
    double Ambient = 0.3, double LightIntensity = 0.8, double LightAzimuth = -35,
    double LightElevation = 45, double Roughness = 0.35,
    double Push = 0, double Emboss = 0, string Output = "FullColor",
    double ShadowStrength = 0, bool SmoothNormals = false, double Reflectivity = 0.55)
{
    public bool IsValid() => Projection is "Perspective" or "Orthographic" &&
        Shading is "Unlit" or "Matte" or "Glossy" &&
        Output is "FullColor" or "FullMask" or "FaceMask" or "EmbossMask" or "EmbossMaskMinusFace" &&
        IconRecipe.TryParseRgba(SideColorHex, out _, out _, out _, out _) &&
        new (double Value, double Min, double Max)[] {
            (CameraDistance,32,8192), (FocalLength,10,300), (OrthographicSize,32,8192), (Extrusion,0,512),
            (RotationX,-360,360), (RotationY,-360,360), (RotationZ,-360,360),
            (PositionX,-4096,4096), (PositionY,-4096,4096), (PositionZ,-4096,4096),
            (Ambient,0,1), (LightIntensity,0,2), (LightAzimuth,-180,180), (LightElevation,-90,90), (Roughness,0,1),
            (Push,-64,64), (Emboss,0,64), (ShadowStrength,0,1), (Reflectivity,0,1)
        }.All(v => double.IsFinite(v.Value) && v.Value >= v.Min && v.Value <= v.Max);

    public string Fingerprint() => FormattableString.Invariant(
        $"{Projection},{CameraDistance:G17},{FocalLength:G17},{OrthographicSize:G17},{Extrusion:G17},{RotationX:G17},{RotationY:G17},{RotationZ:G17},{PositionX:G17},{PositionY:G17},{PositionZ:G17},{Shading},{SideColorHex},{Ambient:G17},{LightIntensity:G17},{LightAzimuth:G17},{LightElevation:G17},{Roughness:G17},{Push:G17},{Emboss:G17},{Output},{ShadowStrength:G17},{SmoothNormals},{Reflectivity:G17}");
}
