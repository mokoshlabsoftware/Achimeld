namespace Achimeld.Core;

public static class IconStyleCloner
{
    public static IconRecipe Clone(IconRecipe recipe)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        if (recipe.Layers is not { Count: > 0 } || recipe.Layers.Any(layer => layer is null))
            throw new ArgumentException("A style must contain a valid layer stack.", nameof(recipe));
        if (recipe.Layers.Select(layer => layer.Id).Distinct().Count() != recipe.Layers.Count)
            throw new ArgumentException("Style layer IDs must be unique.", nameof(recipe));
        var ids = recipe.Layers.ToDictionary(layer => layer.Id, _ => IconLayerId.New());
        return recipe with
        {
            Layers = recipe.Layers.Select(layer => layer with
            {
                Id = ids[layer.Id],
                ParentFolderId = Map(layer.ParentFolderId),
                MaskLayerId = Map(layer.MaskLayerId),
                MaskSourceLayerId = Map(layer.MaskSourceLayerId),
                Effects = layer.EffectiveEffects.Select(effect => effect with { Id = IconLayerEffectId.New() }).ToArray(),
            }).ToArray(),
        };

        IconLayerId? Map(IconLayerId? source) => source is { } id && ids.TryGetValue(id, out var mapped) ? mapped : null;
    }
}
