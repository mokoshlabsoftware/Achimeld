use achimeld_application::{IconStyle, Preferences};
use serde_json::{Value, json};

// The C# user library uses domain JSON (PascalCase, wrapped IDs and numeric
// enums). Projects use the separate camelCase DTO schema. Translate only here.
pub(crate) fn icon_styles(bytes: &[u8]) -> Result<Vec<IconStyle>, String> {
    let mut value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    camel_case(&mut value);
    let items = value
        .as_array_mut()
        .ok_or("Icon style library must be an array")?;
    if items.len() > 128 {
        return Err("Too many saved icon styles".into());
    }
    for style in items {
        recipe(&mut style["recipe"])?;
    }
    let styles: Vec<IconStyle> = serde_json::from_value(value).map_err(|e| e.to_string())?;
    for style in &styles {
        style.validate()?;
    }
    Ok(styles)
}
fn camel_case(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let source = std::mem::take(object);
            for (key, mut value) in source {
                let mut chars = key.chars();
                let key = chars
                    .next()
                    .map(|c| format!("{}{}", c.to_ascii_lowercase(), chars.as_str()))
                    .unwrap_or(key);
                camel_case(&mut value);
                object.insert(key, value);
            }
        }
        Value::Array(items) => {
            for item in items {
                camel_case(item);
            }
        }
        _ => {}
    }
}
const BLENDS: &[&str] = &[
    "Normal",
    "Multiply",
    "Screen",
    "Overlay",
    "Darken",
    "Lighten",
    "Add",
    "Difference",
];
const RANGES: &[&str] = &[
    "Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas", "Whites", "Neutrals", "Blacks",
];
fn enumeration(value: &mut Value, key: &str, names: &[&str]) -> Result<(), String> {
    if let Some(index) = value[key].as_u64() {
        value[key] = json!(
            names
                .get(index as usize)
                .ok_or_else(|| format!("Unknown legacy {key}"))?
        );
    } else if !value[key].is_null() && value[key].as_str().is_none_or(|s| !names.contains(&s)) {
        return Err(format!("Unknown legacy {key}"));
    }
    Ok(())
}
fn gradient(value: &mut Value) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    enumeration(value, "kind", &["Linear", "Radial", "Polygon", "Star"])?;
    enumeration(value, "inputChannel", &["RGB", "Red", "Green", "Blue"])
}
fn recipe(value: &mut Value) -> Result<(), String> {
    let layers = value["layers"]
        .as_array_mut()
        .ok_or("Missing style layers")?;
    for layer in layers {
        for key in ["id", "atlasId", "imageId", "parentFolderId", "maskLayerId"] {
            if layer[key].is_object() {
                layer[key] = layer[key]["value"].clone();
            }
        }
        enumeration(
            layer,
            "kind",
            &[
                "AtlasTile",
                "StandaloneImage",
                "SteamImportedArtwork",
                "Text",
                "Solid",
                "Gradient",
                "AchievementImage",
                "SdfVignette",
                "Folder",
                "Shape",
                "Curves",
                "Contrast",
                "HueSaturation",
                "SelectiveColor",
                "GradientOverlay",
                "UnsharpMask",
                "PerlinNoise",
                "Text3d",
                "Particle",
                "Explosion",
                "Artifact3d",
            ],
        )?;
        enumeration(layer, "blendMode", BLENDS)?;
        gradient(&mut layer["gradient"])?;
        if layer["shape"].is_object() {
            enumeration(
                &mut layer["shape"],
                "kind",
                &["Rectangle", "Sphere", "Polygon", "Star"],
            )?;
        }
        if layer["text"].is_object() {
            enumeration(
                &mut layer["text"],
                "alignment",
                &["Center", "TopLeft", "TopRight", "BottomLeft", "BottomRight"],
            )?;
        }
        if layer["adjustment"].is_object() {
            enumeration(&mut layer["adjustment"], "colorRange", RANGES)?;
            if let Some(colors) = layer["adjustment"]["selectiveColors"].as_array_mut() {
                for color in colors {
                    enumeration(color, "range", RANGES)?;
                }
            }
        }
        // EffectiveEffects is a computed alias in domain JSON, never a second stack.
        if layer["effects"].is_null() {
            layer["effects"] = layer["effectiveEffects"]
                .as_array()
                .cloned()
                .map_or(json!([]), Value::Array);
        }
        if let Some(object) = layer.as_object_mut() {
            object.remove("effectiveEffects");
        }
        if let Some(effects) = layer["effects"].as_array_mut() {
            for effect in effects {
                if effect["id"].is_object() {
                    effect["id"] = effect["id"]["value"].clone();
                }
                enumeration(
                    effect,
                    "kind",
                    &[
                        "ColorOverlay",
                        "Outline",
                        "Shadow",
                        "InnerLight",
                        "BoxBlur",
                        "GradientOverlay",
                        "Skew",
                        "Perspective",
                        "Pinch",
                        "LensBlur",
                        "OuterLight",
                    ],
                )?;
                for key in ["blendMode", "blurBlendMode"] {
                    enumeration(effect, key, BLENDS)?;
                }
                enumeration(effect, "placement", &["Inward", "Center", "Outward"])?;
                enumeration(effect, "outlineShape", &["Square", "Round"])?;
                gradient(&mut effect["gradient"])?;
            }
        }
    }
    Ok(())
}
pub(crate) fn preferences(bytes: &[u8]) -> Result<Preferences, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut preferences =
        serde_json::to_value(Preferences::default()).map_err(|e| e.to_string())?;
    for (old, new) in [
        ("Language", "language"),
        ("ButtonCornerRadius", "controlRadius"),
        ("HeaderFontSize", "headerSize"),
        ("NormalFontSize", "normalSize"),
        ("SmallFontSize", "smallSize"),
        ("InspectorSpacing", "inspectorSpacing"),
        ("AchievementNamingConvention", "namingConvention"),
    ] {
        if let Some(v) = value.get(old) {
            preferences[new] = v.clone();
        }
    }
    for (old, new) in [
        ("Window", "Window"),
        ("Panel", "Panel"),
        ("Control", "Control"),
        ("Border", "Border"),
        ("PrimaryText", "Primary text"),
        ("MutedText", "Muted text"),
        ("Accent", "Accent"),
        ("Success", "Success"),
        ("Warning", "Warning"),
        ("Error", "Error"),
        ("BackgroundTop", "Background top"),
        ("BackgroundBottom", "Background bottom"),
        ("CanvasGrid", "Grid"),
    ] {
        if let Some(hex) = value[old].as_str() {
            let hex = hex
                .strip_prefix('#')
                .filter(|h| [6, 8].contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or("Invalid legacy preference color")?;
            let parts: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            preferences["palette"][new] = if parts.len() == 4 {
                json!([parts[1], parts[2], parts[3], parts[0]])
            } else {
                json!([parts[0], parts[1], parts[2], 255])
            };
        }
    }
    let preferences: Preferences =
        serde_json::from_value(preferences).map_err(|e| e.to_string())?;
    preferences.validate()?;
    Ok(preferences)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domain_library_translates_enums_and_wrapped_links_without_mutating_source() {
        let id = achimeld_application::new_id();
        let folder = achimeld_application::new_id();
        let layer = achimeld_application::new_id();
        let bytes=serde_json::to_vec(&json!([{"Id":id,"Name":"Legacy","Recipe":{"Layers":[{"Id":{"Value":folder},"Kind":8,"Effects":[]},{"Id":{"Value":layer},"Kind":5,"ParentFolderId":{"Value":folder},"Gradient":{"Kind":1,"Stops":[{"Offset":0,"ColorHex":"#FF112233"}]},"Effects":null,"EffectiveEffects":[]}]}}])).unwrap();
        let styles = icon_styles(&bytes).unwrap();
        assert_eq!(styles[0].recipe["layers"][1]["kind"], "Gradient");
        assert_eq!(styles[0].recipe["layers"][1]["gradient"]["kind"], "Radial");
        assert_eq!(styles[0].recipe["layers"][1]["parentFolderId"], folder);
        assert!(
            styles[0].recipe["layers"][1]
                .get("effectiveEffects")
                .is_none()
        );
        assert!(
            serde_json::from_slice::<Value>(&bytes).unwrap()[0]["Recipe"]["Layers"][1]["Id"]
                .is_object()
        );
        let mut library: Value = serde_json::from_slice(&bytes).unwrap();
        library[0]["PreviewPng"] = json!("iVBORw0KGgo=");
        let imported = icon_styles(&serde_json::to_vec(&library).unwrap()).unwrap();
        assert_eq!(imported[0].preview_png.as_deref(), Some("iVBORw0KGgo="));
        let round_trip: Vec<IconStyle> =
            serde_json::from_slice(&serde_json::to_vec(&imported).unwrap()).unwrap();
        assert_eq!(round_trip, imported);
    }
}
