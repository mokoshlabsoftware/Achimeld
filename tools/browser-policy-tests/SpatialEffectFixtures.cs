using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class SpatialEffectFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconLayerEffect legacy = JsonSerializer.Deserialize<IconLayerEffect>("""{"kind":"Outline","width":4}""", json)!;
        Require(legacy.SkewX == 0 && legacy.SkewY == 0 && legacy.PerspectiveX == 0 && legacy.PerspectiveY == 0 &&
            legacy.Amount == 0 && legacy.Aberration == 0, "Legacy effects acquired distortion.");
        Require((int)IconLayerEffectKind.GradientOverlay == 5 && (int)IconLayerEffectKind.Skew == 6,
            "Existing numeric effect identities changed.");
        Require((int)IconLayerEffectKind.LensBlur == 9 && (int)IconLayerEffectKind.OuterLight == 10,
            "Outer light must append its identity without changing old values.");
        IconLayerEffect[] effects = [IconLayerEffect.Skew(24.5, -10), IconLayerEffect.Perspective(0.25, -0.125),
            IconLayerEffect.Pinch(0.75), IconLayerEffect.LensBlur(8, 12)];
        foreach (IconLayerEffect effect in effects)
            Require(JsonSerializer.Deserialize<IconLayerEffect>(JsonSerializer.Serialize(effect, json), json) == effect,
                "Spatial effect settings were lost during recipe serialization.");
        AchievementDefinition achievement = new(AchievementId.New(), new("SPATIAL"), new("Name01"), new("Desc01"),
            false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("Spatial effect fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512, 512));
        IReadOnlyList<ValidationIssue> Validate(IconLayerEffect effect) => ProjectValidator.Validate(project with
        {
            Achievements = [achievement with { Icon = new([IconLayer.Solid() with { Effects = [effect] }]) }]
        }).Where(issue => issue.Code.StartsWith("achievement.icon", StringComparison.Ordinal)).ToArray();
        foreach (IconLayerEffect effect in effects)
            Require(Validate(effect).Count == 0, "Valid spatial effect rejected.");
        IconLayerEffect light = IconLayerEffect.OuterLight();
        Require(Validate(light).Count == 0 &&
            JsonSerializer.Deserialize<IconLayerEffect>(JsonSerializer.Serialize(light, json), json) == light,
            "Outer light failed validation or recipe serialization.");
        foreach (IconLayerEffect invalid in new[] { light with { Width = 256.1 }, light with { Blur = -1 },
            light with { Opacity = 1.1 }, light with { Blur = double.NaN } })
            Require(Validate(invalid).Count > 0, "Invalid outer light accepted.");
        foreach (IconLayerEffect effect in new[] { effects[0] with { SkewX = 60.01 }, effects[0] with { SkewY = double.NaN },
            effects[1] with { PerspectiveX = 0.451 }, effects[1] with { PerspectiveY = -0.451 },
            effects[2] with { Amount = 1.01 }, effects[2] with { Amount = double.PositiveInfinity },
            effects[3] with { Blur = 128.01 }, effects[3] with { Aberration = -1 }, effects[3] with { Opacity = 1.1 } })
            Require(Validate(effect).Any(issue => issue.Code == "achievement.icon.effectDistortion"), "Invalid spatial effect accepted.");
        Console.WriteLine("Spatial effects: legacy defaults, recipe serialization and parameter validation passed.");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
