using System.Text.Json;

namespace Achimeld.Browser;

public sealed record SteamworksSchemaOutline(uint Mask, string Kind, int ItemCount,
    int NamedCount, int ObjectCount, int LargestArray, uint TypeMask, uint ContainerMask,
    int ContainerObjects, int ContainerArrays);

// Reports only the presence of fixed schema field names and JSON kinds. No
// values or arbitrary property names leave the supervised worker.
public static class SteamworksSchemaShape
{
    public static SteamworksSchemaOutline Describe(string json)
    {
        using JsonDocument document = JsonDocument.Parse(json);
        uint mask = 0;
        JsonElement root = document.RootElement;
        if (root.ValueKind == JsonValueKind.Object) mask |= 1u << 0;
        if (root.ValueKind == JsonValueKind.Array) mask |= 1u << 1;
        if (root.ValueKind == JsonValueKind.Object)
        {
            if (root.TryGetProperty("success", out _)) mask |= 1u << 2;
            if (Object(root, "data")) mask |= 1u << 3;
            if (Object(root, "response")) mask |= 1u << 4;
            if (Array(root, "stats")) mask |= 1u << 5;
            if (Array(root, "achievements")) mask |= 1u << 6;
            if (Array(root, "sets")) mask |= 1u << 7;
            if (root.TryGetProperty("statsschema", out _)) mask |= 1u << 8;
            if (root.TryGetProperty("schema", out _)) mask |= 1u << 9;
        }
        string kind = "missing";
        int itemCount = 0;
        uint containerMask = 0;
        int containerObjects = 0, containerArrays = 0;
        if (root.ValueKind == JsonValueKind.Object && root.TryGetProperty("schema", out JsonElement schema))
        {
            kind = schema.ValueKind switch
            {
                JsonValueKind.Object => "object",
                JsonValueKind.Array => "array",
                JsonValueKind.String => "string",
                JsonValueKind.Null => "null",
                _ => "other",
            };
            itemCount = schema.ValueKind switch
            {
                JsonValueKind.Object => schema.EnumerateObject().Take(10001).Count(),
                JsonValueKind.Array => Math.Min(schema.GetArrayLength(), 10001),
                _ => 0,
            };
            if (schema.ValueKind == JsonValueKind.Object)
            {
                foreach (JsonProperty child in schema.EnumerateObject().Take(128))
                {
                    if (child.Value.ValueKind == JsonValueKind.Object) containerObjects++;
                    if (child.Value.ValueKind == JsonValueKind.Array) containerArrays++;
                    int bit = child.Name switch
                    {
                        "stats" => 0, "achievements" => 1, "sets" => 2,
                        "statschema" or "statsschema" => 3,
                        "achievementschema" => 4, "stat" => 5,
                        "achievement" => 6, "languages" => 7,
                        "localizations" => 8, "data" => 9,
                        "entries" => 10, "items" => 11,
                        "schema" => 12, "definitions" => 13,
                        "game" => 14, "results" => 15,
                        _ => -1,
                    };
                    if (bit >= 0) containerMask |= 1u << bit;
                }
            }
        }
        int namedCount = 0, objectCount = 0, largestArray = 0;
        uint typeMask = 0;
        Visit(root, 0, ref mask, ref namedCount, ref objectCount, ref largestArray, ref typeMask);
        return new(mask, kind, itemCount, namedCount, objectCount, largestArray, typeMask,
            containerMask, containerObjects, containerArrays);
    }

    private static bool Object(JsonElement element, string key) =>
        element.TryGetProperty(key, out JsonElement value) && value.ValueKind == JsonValueKind.Object;

    private static bool Array(JsonElement element, string key) =>
        element.TryGetProperty(key, out JsonElement value) && value.ValueKind == JsonValueKind.Array;

    private static void Visit(JsonElement element, int depth, ref uint mask,
        ref int namedCount, ref int objectCount, ref int largestArray, ref uint typeMask)
    {
        if (depth > 5) return;
        if (element.ValueKind == JsonValueKind.Array)
        {
            largestArray = Math.Max(largestArray, Math.Min(element.GetArrayLength(), 10001));
            // Survey at most 256 records, without retaining their contents.
            foreach (JsonElement item in element.EnumerateArray().Take(256))
                Visit(item, depth + 1, ref mask, ref namedCount, ref objectCount, ref largestArray, ref typeMask);
            return;
        }
        if (element.ValueKind != JsonValueKind.Object) return;
        objectCount = Math.Min(objectCount + 1, 10001);
        if (element.TryGetProperty("name", out JsonElement name) && name.ValueKind == JsonValueKind.String)
            namedCount = Math.Min(namedCount + 1, 10001);
        if (element.TryGetProperty("type", out JsonElement type) && type.ValueKind == JsonValueKind.String)
            typeMask |= type.GetString() switch
            {
                "INT" => 1u, "FLOAT" => 2u, "AVGRATE" => 4u,
                "ACHIEVEMENT" => 8u, "STAT" => 16u, _ => 32u,
            };
        int visited = 0;
        foreach (JsonProperty property in element.EnumerateObject())
        {
            if (++visited > 128) break;
            int bit = property.Name switch
            {
                "stats" when property.Value.ValueKind == JsonValueKind.Array => 10,
                "achievements" when property.Value.ValueKind == JsonValueKind.Array => 11,
                "sets" when property.Value.ValueKind == JsonValueKind.Array => 12,
                "api_name" when property.Value.ValueKind == JsonValueKind.String => 13,
                "name" when property.Value.ValueKind == JsonValueKind.String => 14,
                "stat_id" => 15,
                "bit_id" => 16,
                "display_name" => 17,
                "description" => 18,
                "icon" => 19,
                "icon_gray" => 20,
                "permission" => 21,
                "hidden" => 22,
                "progress" => 23,
                "entries" when property.Value.ValueKind == JsonValueKind.Array => 24,
                "stat" => 25,
                "achievement" => 26,
                "localizations" => 27,
                "id" => 28,
                "type" => 29,
                "value" => 30,
                "items" when property.Value.ValueKind == JsonValueKind.Array => 31,
                _ => -1,
            };
            if (bit >= 0) mask |= 1u << bit;
            if (property.Value.ValueKind is JsonValueKind.Object or JsonValueKind.Array)
                Visit(property.Value, depth + 1, ref mask, ref namedCount, ref objectCount, ref largestArray, ref typeMask);
        }
    }
}
