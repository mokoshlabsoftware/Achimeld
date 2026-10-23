using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class ExplosionFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconExplosionStyle settings = new() { CountMin = 2, Count = 100, ScaleMin = .2, ScaleMax = 1.2,
            DifferentSeedPerInstance = true, DisableIn = true, Out = new() { OffsetMin = -.1, OffsetMax = .2,
                Flip = true, Scale = 1.5, ScaleMin = .5, ScaleMax = 1.5 },
            In = new() { Scale = .75, ScaleMin = .25, ScaleMax = 1.25 } };
        IconLayer layer = IconLayer.FromExplosion() with { Explosion = settings, OffsetX = 25, OffsetY = -12, Rotation = 35 };
        IconLayer restored = JsonSerializer.Deserialize<IconLayer>(JsonSerializer.Serialize(layer, json), json)!;
        Require(restored.Kind == IconLayerKind.Explosion && restored.Explosion!.Fingerprint() == settings.Fingerprint() &&
            restored.OffsetX == 25 && restored.OffsetY == -12 && restored.Rotation == 35, "Explosion recipe lost parameters.");
        Require((int)IconLayerKind.Particle == 18 && (int)IconLayerKind.Explosion == 19, "Existing layer IDs changed.");
        Require(new IconExplosionStyle().ValidParameters() && settings.ValidParameters(), "Valid explosion rejected.");
        Require(JsonSerializer.Deserialize<IconExplosionStyle>("{}", json)!.Fingerprint() == new IconExplosionStyle().Fingerprint(), "Sparse defaults differ.");
        Require(new IconExplosionStyle().BottomPower == 0 && new IconExplosionStyle().BottomStretch == .5 &&
            new IconExplosionStyle().BottomCurve == "Linear", "Bottom scaling changed legacy defaults.");
        foreach (string curve in new[] { "Linear", "Quadratic", "Logarithmic" })
        {
            IconExplosionStyle filled = settings with { BottomPower = 9, BottomStretch = .75, BottomCurve = curve };
            Require(filled.ValidParameters() && filled.Fingerprint() != settings.Fingerprint(), "Bottom scaling validation or identity missing.");
            Require(JsonSerializer.Deserialize<IconExplosionStyle>(JsonSerializer.Serialize(filled, json), json)!.Fingerprint() == filled.Fingerprint(),
                "Bottom scaling lost on round trip.");
            Require(filled.Fingerprint() != (filled with { BottomPower = 8 }).Fingerprint() &&
                filled.Fingerprint() != (filled with { BottomStretch = .5 }).Fingerprint() &&
                filled.Fingerprint() != (filled with { BottomCurve = "Other" }).Fingerprint(), "Bottom controls missing from identity.");
        }
        foreach (IconExplosionStyle invalid in new[] { settings with { BottomPower = -.1 }, settings with { BottomPower = 9.1 },
            settings with { BottomPower = double.NaN }, settings with { BottomPower = double.PositiveInfinity },
            settings with { BottomStretch = -.1 }, settings with { BottomStretch = 1.1 }, settings with { BottomStretch = double.NaN },
            settings with { BottomCurve = "Other" }, settings with { BottomCurve = null! } })
            Require(!invalid.ValidParameters(), "Invalid bottom scaling accepted.");
        IconExplosionTriangle legacy = JsonSerializer.Deserialize<IconExplosionTriangle>("{}", json)!;
        Require(!legacy.Flip && legacy.Scale == 1 && legacy.ScaleMin == 1 && legacy.ScaleMax == 1 && legacy.ValidParameters(), "Legacy triangles changed their scale or orientation.");
        Require(!JsonSerializer.Serialize(IconLayer.Solid(), json).Contains("explosion", StringComparison.Ordinal), "Existing layers gained wire fields.");
        foreach (IconExplosionStyle changed in new[] { settings with { CountMin = null }, settings with { Regularity = 1 },
            settings with { ScaleMin = .3 }, settings with { DisableOut = true }, settings with { DisableIn = false },
            settings with { In = new() { HeightMin = .3 } }, settings with { Out = new() }, settings with { Seed = 2 },
            settings with { Out = settings.Out with { Flip = false } }, settings with { Out = settings.Out with { Scale = 1.4 } },
            settings with { In = settings.In with { ScaleMin = .5 } }, settings with { In = settings.In with { ScaleMax = 1.5 } },
            settings with { ShapeX = 2 }, settings with { ShapeY = 3 } })
            Require(settings.Fingerprint() != changed.Fingerprint(), "Explosion parameter missing from identity.");
        AchievementDefinition achievement = new(AchievementId.New(), new("EXPLOSION"), new("Name01"), new("Desc01"), false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("Explosion fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512,512));
        IReadOnlyList<ValidationIssue> Validate(IconExplosionStyle style) => ProjectValidator.Validate(project with {
            Achievements = [achievement with { Icon = new([layer with { Explosion = style }]) }]
        }).Where(issue => issue.Code.StartsWith("achievement.icon", StringComparison.Ordinal)).ToArray();
        Require(Validate(settings).Count == 0, "Valid explosion recipe rejected.");
        IconExplosionStyle deltoid = settings with { Deltoid = new() {
            TopHeightMin = .1, TopHeightMax = .3, BottomHeightMin = .4, BottomHeightMax = .8,
            Scale = 1.2, ScaleMin = .5, ScaleMax = 1.5 } };
        Require(IconLayer.FromExplosion().Explosion!.Deltoid is not null, "New explosion must use a deltoid.");
        IconExplosionStyle defaults = IconLayer.FromExplosion().Explosion!;
        Require(defaults.CoverCanvas && defaults.ScaleMin == 1 && defaults.ScaleMax == 1, "New explosion must cover canvas corners at 100%.");
        Require(!JsonSerializer.Deserialize<IconExplosionStyle>("{}", json)!.CoverCanvas, "Legacy polar radius changed.");
        Require((deltoid with { CoverCanvas = true }).Fingerprint() != deltoid.Fingerprint(), "Polar fit missing from identity.");
        Require(Validate(deltoid).Count == 0 && deltoid.Fingerprint() != settings.Fingerprint(), "Deltoid validation or identity missing.");
        Require(JsonSerializer.Deserialize<IconExplosionStyle>(JsonSerializer.Serialize(deltoid, json), json)!.Fingerprint() == deltoid.Fingerprint(), "Deltoid shape lost on round trip.");
        Require(!JsonSerializer.Serialize(settings, json).Contains("deltoid", StringComparison.Ordinal), "Legacy recipe gained a deltoid.");
        IconExplosionStyle sdf = deltoid with { CoverCanvas = true, Gradient = deltoid.Gradient with { Kind = IconGradientKind.SDF } };
        Require(Validate(sdf).Count == 0 && sdf.Fingerprint() != deltoid.Fingerprint(), "SDF gradient validation or identity missing.");
        string sdfJson = JsonSerializer.Serialize(sdf, json);
        Require(sdfJson.Contains("\"kind\":\"SDF\"", StringComparison.Ordinal) &&
            JsonSerializer.Deserialize<IconExplosionStyle>(sdfJson, json)!.Fingerprint() == sdf.Fingerprint(), "SDF gradient or polar fit lost on round trip.");
        Require((int)IconGradientKind.Linear == 0 && (int)IconGradientKind.Radial == 1 &&
            (int)IconGradientKind.Polygon == 2 && (int)IconGradientKind.Star == 3, "Existing gradient IDs changed.");
        Require(ProjectValidator.Validate(project with { Achievements = [achievement with {
            Icon = new([IconLayer.FromGradient(sdf.Gradient)]) }] }).Any(issue => issue.Code == "achievement.icon.gradient.kind"),
            "SDF gradient accepted without an explosion distance field.");
        IconExplosionDeltoid shape = deltoid.Deltoid!;
        IconExplosionStyle rounded = sdf with { Count = 512, CountMin = 500, ShapeX = 10, ShapeY = 3,
            Deltoid = shape with { Ellipse = .65 }, Brightness = .7, BrightnessMin = .2, BrightnessMax = 1.6, InvertColors = true };
        Require(new IconExplosionStyle().Brightness == 1 && new IconExplosionStyle().BrightnessMin == 1 &&
            new IconExplosionStyle().BrightnessMax == 1 && !new IconExplosionStyle().InvertColors, "Brightness legacy defaults changed.");
        foreach (IconExplosionStyle invalid in new[] { rounded with { Brightness = double.NaN }, rounded with { Brightness = -1 },
            rounded with { Brightness = 2.1 }, rounded with { BrightnessMin = 1.8, BrightnessMax = 1.0 }, rounded with { BrightnessMax = 2.1 } })
            Require(!invalid.ValidParameters(), "Invalid explosion brightness accepted.");
        Require(rounded.Fingerprint() != (rounded with { InvertColors = false }).Fingerprint() &&
            rounded.Fingerprint() != (rounded with { Brightness = .8 }).Fingerprint() &&
            rounded.Fingerprint() != (rounded with { BrightnessMin = .1 }).Fingerprint() &&
            rounded.Fingerprint() != (rounded with { BrightnessMax = 1.5 }).Fingerprint(), "Brightness or inversion missing from identity.");
        Require(Validate(rounded).Count == 0, "512 objects, shape multipliers or ellipse morph rejected.");
        Require(JsonSerializer.Deserialize<IconExplosionStyle>(JsonSerializer.Serialize(rounded, json), json)!.Fingerprint() == rounded.Fingerprint(),
            "Shape multipliers or ellipse morph lost on round trip.");
        Require(rounded.Fingerprint() != (rounded with { Deltoid = rounded.Deltoid! with { Ellipse = .5 } }).Fingerprint(),
            "Ellipse morph missing from identity.");
        IconExplosionStyle sparseShape = JsonSerializer.Deserialize<IconExplosionStyle>("{\"deltoid\":{}}", json)!;
        Require(sparseShape.ShapeX == 1 && sparseShape.ShapeY == 1 && sparseShape.Deltoid!.Ellipse == 0,
            "New shape controls changed legacy defaults.");
        foreach (IconExplosionDeltoid changed in new[] { shape with { Scale = 1.1 }, shape with { ScaleMin = .6 },
            shape with { ScaleMax = 1.6 }, shape with { WidthMin = .1 }, shape with { WidthMax = .4 },
            shape with { TopHeightMin = .2 }, shape with { TopHeightMax = .4 },
            shape with { BottomHeightMin = .3 }, shape with { BottomHeightMax = .9 },
            shape with { OffsetMin = -.1 }, shape with { OffsetMax = .1 },
            shape with { SharpnessMin = .7 }, shape with { SharpnessMax = .9 } })
            Require(deltoid.Fingerprint() != (deltoid with { Deltoid = changed }).Fingerprint(), "Deltoid property missing from identity.");
        foreach (IconExplosionDeltoid invalid in new[] { shape with { TopHeightMin = -.1 }, shape with { TopHeightMax = 1.1 },
            shape with { BottomHeightMin = .9 }, shape with { BottomHeightMax = double.NaN },
            shape with { Scale = 0 }, shape with { ScaleMin = 1.6 }, shape with { ScaleMax = 2.1 },
            shape with { WidthMin = -.1 }, shape with { WidthMax = 2.1 }, shape with { OffsetMax = 1.6 },
            shape with { SharpnessMin = 1.1 }, shape with { Ellipse = -.1 }, shape with { Ellipse = 1.1 },
            shape with { Ellipse = double.NaN } })
            Require(Validate(deltoid with { Deltoid = invalid }).Any(issue => issue.Code == "achievement.icon.explosion"), "Invalid deltoid accepted.");
        foreach (IconExplosionStyle invalid in new[] { settings with { Count = 0 }, settings with { Count = 513 },
            settings with { CountMin = 101 }, settings with { AspectMin = .5 }, settings with { AspectMax = 11 },
            settings with { ScaleMin = 0 }, settings with { ScaleMax = .1 }, settings with { Regularity = double.NaN },
            settings with { Out = new() { SharpnessMax = 2 } }, settings with { In = new() { HeightMin = 2 } },
            settings with { Out = new() { OffsetMin = -.6 } }, settings with { Out = new() { Scale = 0 } },
            settings with { In = new() { Scale = 2.1 } }, settings with { Out = new() { ScaleMin = 1.1 } },
            settings with { In = new() { ScaleMax = .9 } }, settings with { In = new() { ScaleMin = double.NaN } },
            settings with { ShapeX = .9 }, settings with { ShapeX = 10.1 }, settings with { ShapeY = .9 },
            settings with { ShapeY = 10.1 }, settings with { ShapeX = double.NaN },
            settings with { BottomPower = 9.1 }, settings with { BottomStretch = double.NaN }, settings with { BottomCurve = "Other" } })
            Require(Validate(invalid).Any(issue => issue.Code == "achievement.icon.explosion"), "Invalid explosion accepted.");
        Console.WriteLine("Explosion count/ranges, transforms, validation, fingerprints and JSON compatibility passed.");
    }
    private static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
}
