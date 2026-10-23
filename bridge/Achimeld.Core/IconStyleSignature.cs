using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Achimeld.Core;

/// <summary>A content signature independent of copied layer and effect identities.</summary>
public static class IconStyleSignature
{
    public static string Compute(IconRecipe recipe)
    {
        var indices = recipe.Layers.Select((layer, index) => (layer.Id, index))
            .ToDictionary(pair => pair.Id, pair => pair.index);
        var layers = new JsonArray();
        foreach (var layer in recipe.Layers)
        {
            var node = JsonSerializer.SerializeToNode(layer)!.AsObject();
            node.Remove("Id");
            node.Remove("DisplayName");
            node.Remove("EffectiveEffects");
            node["ParentFolderId"] = Reference(layer.ParentFolderId);
            node["MaskLayerId"] = Reference(layer.MaskLayerId);
            if (layer.MaskSourceLayerId is not null)
                node["MaskSourceLayerId"] = Reference(layer.MaskSourceLayerId);
            var effects = new JsonArray();
            foreach (var effect in layer.EffectiveEffects)
            {
                var item = JsonSerializer.SerializeToNode(effect)!.AsObject();
                item.Remove("Id");
                effects.Add(item);
            }
            node["Effects"] = effects;
            // These values are supplied by each achievement, rather than by the style.
            if (layer.Text is { LinkToAchievementValue: true }) node["Text"]!["Content"] = "";
            if (layer.Kind == IconLayerKind.AchievementImage || layer.LinkAtlasToGeneral)
            {
                node["ImageId"] = null;
                node["AtlasId"] = null;
                node["TileIndex"] = 0;
            }
            layers.Add(node);
        }
        var canonical = new JsonObject
        {
            ["Background"] = string.IsNullOrWhiteSpace(recipe.BackgroundColorHex) ? null : recipe.BackgroundColorHex.Trim().ToUpperInvariant(),
            ["Layers"] = layers,
        };
        return Convert.ToHexString(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(canonical)));

        int? Reference(IconLayerId? id) => id is { } value ? indices.GetValueOrDefault(value, -1) : null;
    }
}
