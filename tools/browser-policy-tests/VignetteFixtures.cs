using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class VignetteFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconSdfVignetteStyle legacy = JsonSerializer.Deserialize<IconSdfVignetteStyle>(
            """{"colorHex":"#FF102030","roundness":0.84,"inset":0,"softness":1}""", json)!;
        Require(!legacy.Invert && !legacy.UseGradient && legacy.Gradient is null, "Legacy defaults changed.");
        Require(legacy.Fingerprint() == FormattableString.Invariant($"{legacy.ColorHex},{legacy.Roundness:G17},{legacy.Inset:G17},{legacy.Softness:G17}"),
            "Default settings must retain existing synchronization fingerprints.");
        IconGradientStyle gradient = new(IconGradientKind.Linear, 90,
            [new(0, "#FF36D1DC"), new(0.5, "#807C4DFF"), new(1, "#FFFF4FA3")]);
        IconSdfVignetteStyle mapped = legacy with { Invert = true, UseGradient = true, Gradient = gradient };
        string saved = JsonSerializer.Serialize(IconLayer.FromSdfVignette() with { SdfVignette = mapped }, json);
        IconSdfVignetteStyle restored = JsonSerializer.Deserialize<IconLayer>(saved, json)!.SdfVignette!;
        Require(restored.Invert && restored.UseGradient && restored.Gradient!.Stops.SequenceEqual(gradient.Stops),
            "Serialized recipes lost vignette options or stop transparency.");
        Require(mapped.Fingerprint() != legacy.Fingerprint(), "New options must affect synchronization identity.");
        Require(mapped.Fingerprint() != (mapped with { Invert = false }).Fingerprint(), "Inversion must affect identity.");
        Require(mapped.Fingerprint() != (mapped with { Gradient = gradient with { Stops = [new(0, "#FFFFFFFF")] } }).Fingerprint(),
            "Gradient colors must affect identity.");
        Require(legacy.Fingerprint() == (legacy with { Gradient = gradient }).Fingerprint(), "An unused gradient changed solid-fill identity.");

        AchievementDefinition achievement = new(AchievementId.New(), new("VIGNETTE"), new("Name01"), new("Desc01"),
            false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("Vignette fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512, 512));
        IReadOnlyList<ValidationIssue> Validate(IconSdfVignetteStyle style) => ProjectValidator.Validate(project with
        {
            Achievements = [achievement with { Icon = new([IconLayer.FromSdfVignette() with { SdfVignette = style }]) }]
        }).Where(issue => issue.Code.StartsWith("achievement.icon", StringComparison.Ordinal)).ToArray();
        Require(Validate(mapped).Count == 0 && Validate(legacy).Count == 0, "Valid vignettes were rejected.");
        foreach (IconGradientStyle? invalid in new IconGradientStyle?[]
        {
            null, gradient with { Stops = [] }, gradient with { Stops = [new(-0.1, "#FFFFFFFF")] },
            gradient with { Stops = [new(1.1, "#FFFFFFFF")] }, gradient with { Stops = [new(0, "invalid")] },
            gradient with { Stops = [new(double.NaN, "#FFFFFFFF")] }
        })
            Require(Validate(mapped with { Gradient = invalid }).Any(issue => issue.Path.Contains(".sdfVignette.gradient", StringComparison.Ordinal)),
                "Invalid nested gradient settings were accepted or attributed to the wrong path.");
        Console.WriteLine("SDF vignette legacy defaults, recipe round-trip, validation and synchronization identity fixtures passed.");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
