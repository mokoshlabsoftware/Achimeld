using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class Text3dFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconLayer layer = IconLayer.FromText3d();
        Require(layer.Kind == IconLayerKind.Text3d && layer.Text3d!.IsValid(), "Default 3D text rejected.");
        Require((int)IconLayerKind.Text == 3 && (int)IconLayerKind.UnsharpMask == 15, "Existing layer IDs changed.");
        IconText3dStyle settings = new(Projection: "Orthographic", CameraDistance: 900, FocalLength: 80,
            OrthographicSize: 700, Extrusion: 100, RotationX: -20, RotationY: 110, RotationZ: 30,
            PositionX: 80, PositionY: -40, PositionZ: 160, Shading: "Glossy", Roughness: 0.8,
            Push: -2.5, Emboss: 6, Output: "EmbossMaskMinusFace", ShadowStrength: 0.75, SmoothNormals: true,
            Reflectivity: 0.8);
        Require(JsonSerializer.Deserialize<IconText3dStyle>(JsonSerializer.Serialize(settings, json), json) == settings,
            "3D text settings changed during JSON round trip.");
        Require(JsonSerializer.Deserialize<IconText3dStyle>("{}", json) == new IconText3dStyle(), "Missing 3D fields lost defaults.");
        AchievementDefinition achievement = new(AchievementId.New(), new("TEXT3D"), new("Name01"), new("Desc01"),
            false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("3D text fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512,512));
        IReadOnlyList<ValidationIssue> Validate(IconText3dStyle style) => ProjectValidator.Validate(project with
        {
            Achievements = [achievement with { Icon = new([layer with { Text3d = style }]) }]
        }).Where(issue => issue.Code.StartsWith("achievement.icon", StringComparison.Ordinal)).ToArray();
        Require(Validate(settings).Count == 0, "Valid 3D text recipe rejected.");
        foreach (double fontSize in new[] { 0.5, 513.0 })
            Require(ProjectValidator.Validate(project with {
                Achievements = [achievement with { Icon = new([layer with { Text = layer.Text! with { FontSize = fontSize } }]) }]
            }).Any(issue => issue.Code == "achievement.icon.text3d"), "Invalid 3D font size accepted.");
        foreach (IconText3dStyle invalid in new[] { settings with { CameraDistance = 0 }, settings with { FocalLength = 301 },
            settings with { Extrusion = -1 }, settings with { RotationY = double.NaN }, settings with { PositionZ = 4097 },
            settings with { Projection = "Other" }, settings with { Shading = "Other" }, settings with { Ambient = 2 },
            settings with { Roughness = -1 }, settings with { LightIntensity = double.PositiveInfinity }, settings with { SideColorHex = "bad" },
            settings with { Push = -65 }, settings with { Push = double.NaN }, settings with { Emboss = -1 },
            settings with { Emboss = 65 }, settings with { Output = "Other" }, settings with { ShadowStrength = 1.1 },
            settings with { Reflectivity = -0.01 }, settings with { Reflectivity = 1.01 }, settings with { Reflectivity = double.NaN } })
            Require(Validate(invalid).Any(issue => issue.Code == "achievement.icon.text3d"), "Invalid 3D settings accepted.");
        Require(settings.Fingerprint() != (settings with { PositionZ = 161 }).Fingerprint(), "Object transform omitted from fingerprint.");
        foreach (IconText3dStyle changed in new[] { settings with { Push = 3 }, settings with { Emboss = 4 },
            settings with { Output = "FaceMask" }, settings with { ShadowStrength = 0 }, settings with { SmoothNormals = false },
            settings with { Reflectivity = 0.5 } })
            Require(settings.Fingerprint() != changed.Fingerprint(), "Relief or output omitted from fingerprint.");
        Console.WriteLine("3D text: defaults, serialization, validation and settings fingerprints passed.");
    }
    private static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
}
