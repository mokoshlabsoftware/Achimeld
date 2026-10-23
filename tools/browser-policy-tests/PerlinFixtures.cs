using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;
using Achimeld.Persistence;

internal static class PerlinFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconPerlinNoiseStyle sparse = JsonSerializer.Deserialize<IconPerlinNoiseStyle>("{}",json)!;
        Require(sparse.Seed == 0 && sparse.Octaves == 6 && sparse.Frequency == 4 && sparse.Persistence == 0.5 && sparse.Lacunarity == 2,
            "Sparse Perlin defaults changed.");
        IconGradientStyle gradient = new(IconGradientKind.Linear,90,[new(0,"#00102030"),new(1,"#FFFFFFFF")]);
        IconPerlinNoiseStyle settings = new(71,3.4,4,0.6,2.2,[true,false,true,false],true,gradient);
        IconLayer source = IconLayer.FromPerlinNoise() with { PerlinNoise = settings };
        IconLayer owner = IconLayer.Solid() with { MaskLayerId=source.Id, MaskMode=IconMaskMode.Luminance, MaskInvert=true };
        IconRecipe restored = JsonSerializer.Deserialize<IconRecipe>(JsonSerializer.Serialize(new IconRecipe([owner,source]),json),json)!;
        Require(restored.Layers[0].MaskLayerId == source.Id && restored.Layers[0].MaskMode == IconMaskMode.Luminance && restored.Layers[0].MaskInvert,
            "Mask settings did not round trip.");
        Require(restored.Layers[1].PerlinNoise!.Fingerprint() == settings.Fingerprint(),"Noise settings did not round trip.");
        Require(settings.Fingerprint() != (settings with { Seed=72 }).Fingerprint(),"Seed is absent from noise identity.");
        Require(settings.Fingerprint() != (settings with { EnabledOctaves=[true,true,true,false] }).Fingerprint(),"Octave switches are absent from noise identity.");
        V1IconLayerDto dto = JsonSerializer.Deserialize<V1IconLayerDto>(JsonSerializer.Serialize(new V1IconLayerDto {
            Id=source.Id.Value, Kind="PerlinNoise", PerlinNoise=settings, MaskMode="Luminance", MaskInvert=true },json),json)!;
        Require(dto.PerlinNoise!.Fingerprint() == settings.Fingerprint() && dto.MaskMode == "Luminance" && dto.MaskInvert,
            "Version-one recipe DTO lost mask or noise settings.");
        Require((int)IconLayerKind.UnsharpMask == 15,"Existing layer enum identities changed.");
        AchievementDefinition achievement = new(AchievementId.New(),new("PERLIN"),new("Name01"),new("Desc01"),false,SetBy.GameClient,null,null,0,0);
        AchimeldProject project = AchimeldProject.Create("Noise fixture",new(480),
            new(PathReference.RelativeToProject("atlas.png"),PathReference.RelativeToProject("project.csv")),new(512,512));
        IReadOnlyList<ValidationIssue> Validate(IconPerlinNoiseStyle noise) => ProjectValidator.Validate(project with {
            Achievements=[achievement with { Icon=new([owner,source with { PerlinNoise=noise }]) }] }).Where(i=>i.Code.StartsWith("achievement.icon",StringComparison.Ordinal)).ToArray();
        Require(Validate(settings).Count == 0 && Validate(sparse).Count == 0,"Valid noise was rejected.");
        foreach (IconPerlinNoiseStyle invalid in new[] { settings with { Octaves=13 },settings with { Seed=-1 },settings with { Frequency=0 },
            settings with { Frequency=double.NaN },settings with { Persistence=1.1 },settings with { Lacunarity=4.1 },
            settings with { EnabledOctaves=Enumerable.Repeat(true,13).ToArray() },settings with { Gradient=null },
            settings with { Gradient=gradient with { Stops=[] } } })
            Require(Validate(invalid).Count > 0,"Invalid noise was accepted.");
        Console.WriteLine("Perlin noise and generic mask model fixtures passed.");
    }
    private static void Require(bool condition,string message) { if(!condition) throw new Exception(message); }
}
