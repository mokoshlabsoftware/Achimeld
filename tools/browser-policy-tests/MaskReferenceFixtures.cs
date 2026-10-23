using System.Text.Json;
using System.Text.Json.Serialization;
using Achimeld.Core;
using Achimeld.Persistence;

internal static class MaskReferenceFixtures
{
    internal static void Run()
    {
        JsonSerializerOptions json = new(JsonSerializerDefaults.Web);
        json.Converters.Add(new JsonStringEnumConverter());
        IconLayer mask = IconLayer.Solid();
        IconLayer source = new(IconLayerId.New(), "Source", IconLayerKind.Folder,
            MaskLayerId: mask.Id, MaskMode: IconMaskMode.Luminance, MaskInvert: true);
        IconLayer reference = new(IconLayerId.New(), "Reference", IconLayerKind.MaskReference, MaskSourceLayerId: source.Id);
        IconLayer owner = new(IconLayerId.New(), "Owner", IconLayerKind.Folder, MaskLayerId: reference.Id, MaskMode: IconMaskMode.Luminance);
        IconRecipe recipe = new([mask, source, reference, owner]);
        IconRecipe restored = JsonSerializer.Deserialize<IconRecipe>(JsonSerializer.Serialize(recipe, json), json)!;
        Require(restored.Layers[2].Kind == IconLayerKind.MaskReference && restored.Layers[2].MaskSourceLayerId == source.Id,
            "Reference mask did not round trip.");
        V1IconLayerDto dto = new() { Id = reference.Id.Value, Kind = "MaskReference", MaskSourceLayerId = source.Id.Value };
        Require(JsonSerializer.Deserialize<V1IconLayerDto>(JsonSerializer.Serialize(dto, json), json)!.MaskSourceLayerId == source.Id.Value,
            "Project DTO lost mask reference.");
        IconRecipe clone = IconStyleCloner.Clone(recipe);
        Require(clone.Layers[2].MaskSourceLayerId == clone.Layers[1].Id && clone.Layers[2].MaskSourceLayerId != source.Id,
            "Style clone retained an original reference.");
        Require(IconStyleSignature.Compute(clone) == IconStyleSignature.Compute(recipe), "Reference identity changed on cloning.");
        Require((int)IconLayerKind.RadialBlur == 21 && (int)IconLayerKind.MaskReference == 22, "Layer enum compatibility changed.");
        AchievementDefinition achievement = new(AchievementId.New(), new("MASK"), new("Name01"), new("Desc01"), false, SetBy.GameClient, null, null, 0, 0);
        AchimeldProject project = AchimeldProject.Create("Mask fixture", new(480),
            new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512, 512));
        bool Valid(IconRecipe value) => !ProjectValidator.Validate(project with { Achievements = [achievement with { Icon = value }] })
            .Any(issue => issue.Code == "achievement.icon.maskReference");
        Require(Valid(recipe), "Valid reference mask rejected.");
        Require(Valid(new([mask, source, reference with { MaskSourceLayerId = null }, owner])), "Unassigned reference rejected.");
        foreach (IconLayerId invalid in new[] { IconLayerId.New(), reference.Id, owner.Id })
            Require(!Valid(new([mask, source, reference with { MaskSourceLayerId = invalid }, owner])), "Missing or cyclic reference accepted.");
        Require(!Valid(new([mask, source, reference, owner with { ParentFolderId = source.Id }])), "Mixed folder/reference cycle accepted.");
        Console.WriteLine("Reference masks: JSON/DTO, clone identity, missing references and mixed cycles passed.");
    }
    private static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
}
