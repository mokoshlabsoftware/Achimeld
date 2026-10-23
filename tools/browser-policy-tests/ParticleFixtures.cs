using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;

internal static class ParticleFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconParticleStyle settings = new() { DifferentSeedPerInstance = true, SizeMin = 40, SizeMax = 100,
            RadiusMin = .1, SoftnessMin = .05, Shapes = [new(), new() { Operation = "Subtract", Scale = .6 }] };
        IconLayer layer = IconLayer.FromParticles() with { Particles = settings };
        Require(IconLayer.FromParticles().Particles!.ColorMap is { Points.Count: 12 }, "New particle layers must have a 2D color map.");
        IconLayer restored = JsonSerializer.Deserialize<IconLayer>(JsonSerializer.Serialize(layer, json), json)!;
        Require(restored.Kind == IconLayerKind.Particle && restored.Particles!.DifferentSeedPerInstance &&
            restored.Particles.Shapes.SequenceEqual(settings.Shapes) && restored.Particles.SizeMin == 40 &&
            restored.Particles.SizeMax == 100 && restored.Particles.SoftnessMin == .05 && restored.Particles.RadiusMin == .1,
            "Particle components, instance seed or variation brackets were lost during serialization.");
        Require(settings.ValidParameters(), "Valid composite particles rejected.");
        Require(settings.Fingerprint() != (settings with { DifferentSeedPerInstance = false }).Fingerprint(), "Instance seed does not affect identity.");
        Require(settings.Fingerprint() != (settings with { Shapes = [new()] }).Fingerprint(), "Composite shape does not affect identity.");
        Require(settings.Fingerprint() != (settings with { SoftnessMin = .1 }).Fingerprint(), "Softness variation does not affect identity.");
        IconParticleStyle varied = settings with { ShapeX = 2, ShapeY = 3, Regularity = .25,
            InnerRadiusMin = .1, Shapes = [new() { Kind = "Star", InnerRadiusMin = .2, InnerRadius = .6 }] };
        IconParticleStyle variedRestored = JsonSerializer.Deserialize<IconParticleStyle>(JsonSerializer.Serialize(varied, json), json)!;
        Require(varied.ValidParameters() && variedRestored.ShapeX == 2 && variedRestored.ShapeY == 3 &&
            variedRestored.Regularity == .25 && variedRestored.InnerRadiusMin == .1 &&
            variedRestored.Shapes[0].InnerRadiusMin == .2, "Particle axes, regularity or star range were lost.");
        foreach (IconParticleStyle edit in new[] { settings with { ShapeX = 2 }, settings with { ShapeY = 3 },
            settings with { Regularity = 0 }, settings with { InnerRadiusMin = .1 },
            settings with { Shapes = [new() { InnerRadiusMin = .2 }, settings.Shapes[1]] } })
            Require(settings.Fingerprint() != edit.Fingerprint(), "Particle controls do not affect identity.");
        Require(settings.Fingerprint() == (settings with { ShapeX = 1, ShapeY = 1, Regularity = 1, InnerRadiusMin = settings.InnerRadius }).Fingerprint(), "Neutral particle controls changed legacy identity.");
        IconGradientMap map = new();
        map = map with { Points = [..map.Points, new() { X = .4, Y = .6, ColorHex = "#80FF40A0", Power = 2, RangeX = .2, RangeY = .8 }] };
        IconParticleStyle mapped = settings with { ColorMap = map };
        IconParticleStyle mappedRestored = JsonSerializer.Deserialize<IconParticleStyle>(JsonSerializer.Serialize(mapped,json),json)!;
        Require(mapped.ValidParameters() && mappedRestored.ColorMap!.Points.SequenceEqual(map.Points), "2D color-map points or influence were lost.");
        Require(mapped.Fingerprint() != settings.Fingerprint(), "2D gradient does not affect particle identity.");
        foreach (IconGradientMap invalidMap in new[] {
            map with { Points = [..map.Points.Take(12), new() { X = .4, Y = .6, Power = double.NaN }] },
            map with { Points = [..map.Points.Take(12), new() { ColorHex = "#XX" }] },
            map with { Points = [..map.Points.Take(12), new() { RangeY = 0 }] },
            map with { Points = [..map.Points.Take(12), map.Points[0]] },
            map with { Points = [map.Points[0] with { X = -.1 }, ..map.Points.Skip(1)] }
        }) Require(!(mapped with { ColorMap = invalidMap }).ValidParameters(), "Invalid 2D gradient or duplicate point accepted.");
        foreach (IconGradientMap editable in new[] { map with { Points = map.Points.Skip(1).ToArray() },
            map with { Points = [map.Points[0] with { X = .1 }, ..map.Points.Skip(1)] }, map with { Points = [] } })
        {
            IconParticleStyle free = mapped with { ColorMap = editable };
            IconParticleStyle roundTrip = JsonSerializer.Deserialize<IconParticleStyle>(JsonSerializer.Serialize(free, json), json)!;
            Require(free.ValidParameters() && roundTrip.ColorMap!.Points.SequenceEqual(editable.Points), "Moved, deleted or empty gradient points failed to round trip.");
            if (editable.Points.Count == 0)
                Require(free.Fingerprint() != (free with { ColorMap = null }).Fingerprint(), "An empty transparent map must have a different identity from the legacy gradient.");
        }
        foreach (IconParticleStyle invalid in new[] { settings with { Count = 513 }, settings with { SizeMin = 101 },
            settings with { ShapeX = .99 }, settings with { ShapeY = 10.01 }, settings with { Regularity = double.NaN },
            settings with { Regularity = 1.01 }, settings with { InnerRadiusMin = .9 },
            settings with { Shapes = [new() { InnerRadiusMin = .9 }] },
            settings with { RadiusMin = 2 }, settings with { SoftnessMin = .9 }, settings with { OpacityMax = double.NaN },
            settings with { Shapes = [new() { Scale = 0 }] }, settings with { Shapes = [new() { Kind = "Unknown" }] },
            settings with { Shapes = Enumerable.Range(0,17).Select(_ => new IconParticleShape()).ToArray() } })
            Require(!invalid.ValidParameters(), "Invalid particle shape or range accepted.");
        IconParticleStyle legacy = JsonSerializer.Deserialize<IconParticleStyle>("{}",json)!;
        Require(!legacy.DifferentSeedPerInstance && legacy.Shapes.Count == 0 && legacy.RadiusMin == 0 &&
            legacy.SoftnessMin is null && legacy.ColorMap is null && legacy.ShapeX == 1 && legacy.ShapeY == 1 &&
            legacy.Regularity == 1 && legacy.InnerRadiusMin is null && legacy.ValidParameters(), "Sparse particle defaults changed.");
        Console.WriteLine("Particle composite shapes, variation brackets, per-instance seeds, validation and recipe round trips passed.");
    }
    private static void Require(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
}
