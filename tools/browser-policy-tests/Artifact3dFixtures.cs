using System.Text.Json;
using Achimeld.Core;

internal static class Artifact3dFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        IconLayer layer = IconLayer.FromArtifact3d();
        IconArtifact3dStyle settings = layer.Artifact3d!;
        Require(settings.IsValid() && settings.Parts.Count == 6, "Default artifact rejected.");
        Require((int)IconLayerKind.Text3d == 17 && (int)IconLayerKind.Explosion == 19 && (int)IconLayerKind.Artifact3d == 20, "Existing layer IDs changed.");
        string serialized = JsonSerializer.Serialize(settings, json);
        Require(JsonSerializer.Deserialize<IconArtifact3dStyle>(serialized, json)!.Fingerprint() == settings.Fingerprint(), "Artifact JSON changed.");
        Require(JsonSerializer.Deserialize<IconArtifact3dStyle>("{}", json)!.Fingerprint() == settings.Fingerprint(), "Artifact defaults differ.");
        AchievementDefinition achievement = new(AchievementId.New(), new("ARTIFACT"), new("Name01"), new("Desc01"), false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("Artifact fixture", new(480), new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512,512));
        bool Valid(IconArtifact3dStyle style) => !ProjectValidator.Validate(project with {Achievements = [achievement with {Icon = new([layer with {Artifact3d = style}])}]}).Any(i => i.Code == "achievement.icon.artifact3d");
        Require(Valid(settings), "Valid artifact rejected by project validation.");
        IconArtifactPart part = settings.Parts[0];
        foreach (IconArtifact3dStyle invalid in new[] {settings with {CameraDistance = 0}, settings with {Output = "FaceMask"}, settings with {Parts = Enumerable.Repeat(part,65).ToArray()}, settings with {Parts = [part,part]}, settings with {Rotation = [0,0]}})
            Require(!Valid(invalid), "Invalid artifact accepted.");
        foreach (IconArtifactPart invalid in new[] {part with {Rounding = 1.1},part with {Size = [0,120,120]},part with {CutTop = -1},part with {SweepAngle = 0},part with {Primitive = "Other"},part with {WallThickness = 1},part with {TubeRadius = 0},part with {ColorHex = "bad"},part with {Id = "bad"},part with {Size = [double.NaN,1,1]}})
            Require(!Valid(settings with {Parts = [invalid]}), "Invalid artifact part accepted.");
        foreach (IconArtifactPart changed in new[] {part with {Size = [82,83,82]},part with {Rounding = 0.5},part with {SweepAngle = 180},part with {CutTop = 0},part with {Visible = false},part with {ColorHex = "#FFFFFFFF"},part with {Shading = "Unlit"}})
            Require((settings with {Parts = [changed]}).Fingerprint() != (settings with {Parts = [part]}).Fingerprint(), "Part field missing from fingerprint.");
        Console.WriteLine("3D artifacts: defaults, serialization, part/camera bounds and fingerprints passed.");
    }
    private static void Require(bool condition, string message) {if (!condition) throw new InvalidOperationException(message);}
}
