using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class RadialBlurFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        Require((int)IconLayerKind.Explosion == 19 && (int)IconLayerKind.Artifact3d == 20 && (int)IconLayerKind.RadialBlur == 21, "Existing layer IDs changed.");
        IconLayer layer = new(IconLayerId.New(), "Radial blur", IconLayerKind.RadialBlur,
            Adjustment: new(RadialBlur: 70, AngularBlur: 30));
        IconLayer restored = JsonSerializer.Deserialize<IconLayer>(JsonSerializer.Serialize(layer, json), json)!;
        Require(restored.Kind == IconLayerKind.RadialBlur && restored.Adjustment!.Fingerprint() == layer.Adjustment!.Fingerprint(), "Radial blur lost on round trip.");
        Require(layer.Adjustment!.Fingerprint() != (layer.Adjustment with { AngularBlur = 20 }).Fingerprint(), "Blur missing from identity.");
        AchimeldProject project = AchimeldProject.Create("Blur fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512, 512));
        AchievementDefinition achievement = new(AchievementId.New(), new("BLUR"), new("Name01"), new("Desc01"), false, SetBy.GameClient, null, null, 0, 0);
        IReadOnlyList<ValidationIssue> Validate(IconLayer value) => ProjectValidator.Validate(project with {
            Achievements = [achievement with { Icon = new([value]) }]
        }).Where(issue => issue.Code.StartsWith("achievement.icon", StringComparison.Ordinal)).ToArray();
        Require(Validate(layer).Count == 0 && Validate(layer with { Adjustment = new() }).Count == 0, "Valid radial blur rejected.");
        foreach (IconAdjustmentStyle invalid in new[] { new IconAdjustmentStyle(RadialBlur: -1), new(RadialBlur: 101),
            new(AngularBlur: -1), new(AngularBlur: 101), new(RadialBlur: double.NaN) })
            Require(Validate(layer with { Adjustment = invalid }).Any(issue => issue.Code == "achievement.icon.radialBlur"), "Invalid radial blur accepted.");
        IconExplosionStyle explosion = IconLayer.FromExplosion().Explosion! with { RadialBlur = .4, AngularBlur = .6 };
        Require(JsonSerializer.Deserialize<IconExplosionStyle>(JsonSerializer.Serialize(explosion, json), json)!.Fingerprint() == explosion.Fingerprint(), "Explosion construction blur lost.");
        Require(explosion.Fingerprint() != (explosion with { RadialBlur = 0 }).Fingerprint(), "Explosion blur missing from identity.");
        Require(!(explosion with { RadialBlur = 1.1 }).ValidParameters() && !(explosion with { AngularBlur = -.1 }).ValidParameters(), "Invalid construction blur accepted.");
        Console.WriteLine("Radial/angular blur: layer identity, managed validation, JSON and explosion settings passed.");
    }
    private static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
}
