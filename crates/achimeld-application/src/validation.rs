use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

type Result = std::result::Result<(), String>;

fn integer(value: &Value, key: &str, min: i64, max: i64) -> Result {
    if !value[key].is_null()
        && value[key]
            .as_i64()
            .is_none_or(|n| !(min..=max).contains(&n))
    {
        return Err(format!("{key} must be between {min} and {max}"));
    }
    Ok(())
}
fn number(value: &Value, key: &str, min: f64, max: f64) -> Result {
    if !value[key].is_null()
        && value[key]
            .as_f64()
            .is_none_or(|n| !n.is_finite() || !(min..=max).contains(&n))
    {
        return Err(format!("{key} must be between {min} and {max}"));
    }
    Ok(())
}
fn colors(value: &Value, depth: usize) -> Result {
    if depth > 64 {
        return Err("Icon settings are too deeply nested".into());
    }
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if key.ends_with("ColorHex") || key == "colorHex" || key == "tintHex" {
                    if !value.is_null()
                        && value.as_str().is_none_or(|s| {
                            !s.starts_with('#')
                                || ![7, 9].contains(&s.len())
                                || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                        })
                    {
                        return Err(format!("{key} requires #RRGGBB or #AARRGGBB"));
                    }
                } else {
                    colors(value, depth + 1)?;
                }
            }
        }
        Value::Array(array) => {
            for item in array {
                colors(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn project(project: &Value) -> Result {
    let atlas = &project["atlas"];
    if !atlas.is_object() || !project["assets"].is_object() {
        return Err("Atlas settings and assets must be objects".into());
    }
    integer(project, "nextAchievementInternalId", 0, i32::MAX.into())?;
    for key in ["outputSize", "outputWidth", "outputHeight"] {
        integer(atlas, key, 1, 4096)?;
    }
    for key in ["tileWidth", "tileHeight"] {
        integer(atlas, key, 1, 16384)?;
    }
    for key in [
        "leftOffset",
        "topOffset",
        "horizontalSpacing",
        "verticalSpacing",
    ] {
        integer(atlas, key, 0, 16384)?;
    }
    if !atlas["traversal"].is_null()
        && !matches!(
            atlas["traversal"].as_str(),
            Some("RowMajor" | "ColumnMajor")
        )
    {
        return Err("Invalid atlas traversal".into());
    }
    let unachieved = &atlas["unachievedIcon"];
    if !unachieved.is_null() && !unachieved.is_object() {
        return Err("Unachieved settings must be an object".into());
    }
    if !unachieved["mode"].is_null()
        && !matches!(unachieved["mode"].as_str(), Some("Grayscale" | "Tint"))
    {
        return Err("Unsupported unachieved icon mode".into());
    }
    number(unachieved, "opacity", 0.0, 1.0)?;
    for key in ["images", "atlases"] {
        let entries = project["assets"][key]
            .as_array()
            .ok_or("Missing asset library")?;
        if entries.len() > 20000 {
            return Err("Asset library is too large".into());
        }
        let mut ids = BTreeSet::new();
        for entry in entries {
            let id = entry["id"].as_str().ok_or("Missing asset ID")?;
            if id.is_empty() || !ids.insert(id.to_ascii_lowercase()) {
                return Err("Duplicate asset identity".into());
            }
            reference(&entry["image"])?;
        }
    }
    colors(atlas, 0)
}
fn reference(value: &Value) -> Result {
    if !matches!(
        value["kind"].as_str(),
        Some("RelativeToProject" | "Absolute")
    ) || value["path"]
        .as_str()
        .is_none_or(|s| s.is_empty() || s.len() > 32767 || s.contains('\0'))
    {
        return Err("Invalid image path reference".into());
    }
    Ok(())
}
fn perlin_noise(settings: &Value) -> Result {
    if !settings.is_null() && !settings.is_object() {
        return Err("Perlin noise settings must be an object".into());
    }
    integer(settings, "seed", 0, i32::MAX.into())?;
    integer(settings, "octaves", 1, 12)?;
    number(settings, "frequency", 0.1, 128.0)?;
    number(settings, "persistence", 0.0, 1.0)?;
    number(settings, "lacunarity", 1.0, 4.0)?;
    if !settings["enabledOctaves"].is_null()
        && settings["enabledOctaves"]
            .as_array()
            .is_none_or(|items| items.len() > 12 || items.iter().any(|v| !v.is_boolean()))
    {
        return Err("Perlin noise supports at most twelve octave switches".into());
    }
    if !settings["useGradient"].is_null() && !settings["useGradient"].is_boolean() {
        return Err("Perlin noise gradient selection must be a boolean".into());
    }
    let gradient = &settings["gradient"];
    if settings["useGradient"] == true || !gradient.is_null() {
        let stops = gradient["stops"]
            .as_array()
            .ok_or("Noise gradient requires stops")?;
        if stops.is_empty() || stops.len() > 128 {
            return Err("Noise gradient requires one to 128 stops".into());
        }
        for stop in stops {
            if stop["offset"]
                .as_f64()
                .is_none_or(|n| !n.is_finite() || !(0.0..=1.0).contains(&n))
                || !stop["colorHex"].is_string()
            {
                return Err("Invalid noise gradient stop".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn entity(entity: &Value) -> Result {
    for key in ["canvasColumn", "canvasRow"] {
        integer(entity, key, -100000, 100000)?;
    }
    integer(entity, "displayOrder", 0, 20000)?;
    integer(entity, "tileIndex", 0, i32::MAX.into())?;
    let icon = &entity["icon"];
    colors(icon, 0)?;
    if icon.is_null() {
        return Ok(());
    }
    let layers = icon["layers"]
        .as_array()
        .ok_or("Icon layers must be an array")?;
    if layers.len() > 512 {
        return Err("An icon may contain at most 512 layers".into());
    }
    let mut by_id = BTreeMap::new();
    for layer in layers {
        if layer["kind"] == "Artifact3d" {
            crate::artifact3d::ArtifactSettings::from_value(&layer["artifact3d"])?;
        }
        if layer["kind"] == "Text3d" {
            crate::text3d::Text3dSettings::from_value(&layer["text3d"])?;
            number(&layer["text"], "fontSize", 1.0, 512.0)?;
            if layer["text"]["content"]
                .as_str()
                .is_some_and(|s| s.chars().count() > 256)
            {
                return Err("3D text may contain at most 256 characters".into());
            }
        }
        if layer["kind"] == "Particle" {
            crate::particles::ParticleSettings::from_value(&layer["particles"])?;
        }
        if layer["kind"] == "Explosion" {
            crate::explosion::ExplosionSettings::from_value(&layer["explosion"])?;
        }
        if layer["kind"] == "RadialBlur" {
            for key in ["radialBlur", "angularBlur"] {
                number(&layer["adjustment"], key, 0.0, 100.0)?;
            }
        }
        let id = layer["id"].as_str().ok_or("Missing layer ID")?;
        if !super::document::valid_id(id) || by_id.insert(id, layer).is_some() {
            return Err("Invalid or duplicate layer ID".into());
        }
        for (key, min, max) in [
            ("opacity", 0.0, 1.0),
            ("scale", 0.001, 100.0),
            ("rotation", -36000.0, 36000.0),
            ("offsetX", -100000.0, 100000.0),
            ("offsetY", -100000.0, 100000.0),
        ] {
            number(layer, key, min, max)?;
        }
        if layer["kind"] == "PerlinNoise" {
            perlin_noise(&layer["perlinNoise"])?;
        }
        if !layer["maskMode"].is_null()
            && !matches!(layer["maskMode"].as_str(), Some("Alpha" | "Luminance"))
        {
            return Err("Mask channel must be Alpha or Luminance".into());
        }
        if !layer["maskInvert"].is_null() && !layer["maskInvert"].is_boolean() {
            return Err("Mask inversion must be a boolean".into());
        }
        if !layer["maskSourceLayerId"].is_null() && layer["kind"] != "MaskReference" {
            return Err("Only reference masks can reference another layer's mask".into());
        }
        number(&layer["text"], "fontSize", 1.0, 4096.0)?;
        if layer["text"]["content"]
            .as_str()
            .is_some_and(|s| s.len() > 16384)
        {
            return Err("Icon text is too long".into());
        }
        let effects = layer["effects"].as_array();
        if effects.is_some_and(|a| a.len() > 32) {
            return Err("A layer may contain at most 32 effects".into());
        }
        for effect in effects.into_iter().flatten() {
            let fields: &[(&str, f64, f64)] = match effect["kind"].as_str() {
                Some("Skew") => &[("skewX", -60.0, 60.0), ("skewY", -60.0, 60.0)],
                Some("Perspective") => {
                    &[("perspectiveX", -0.45, 0.45), ("perspectiveY", -0.45, 0.45)]
                }
                Some("Pinch") => &[("amount", -1.0, 1.0)],
                Some("LensBlur") => &[("blur", 0.0, 128.0), ("aberration", 0.0, 64.0)],
                Some("OuterLight") => &[("width", 0.0, 256.0), ("blur", 0.0, 256.0)],
                _ => continue,
            };
            number(effect, "opacity", 0.0, 1.0)?;
            for &(key, min, max) in fields {
                number(effect, key, min, max)?;
            }
        }
        if layer["gradient"]["stops"]
            .as_array()
            .is_some_and(|a| a.len() > 128)
        {
            return Err("Too many gradient stops".into());
        }
        if let Some(points) = layer["curves"]["points"].as_array() {
            if points.len() > 256 {
                return Err("Too many curve points".into());
            }
            for point in points {
                number(point, "input", 0.0, 1.0)?;
                number(point, "output", 0.0, 1.0)?;
            }
        }
    }
    for layer in layers {
        let id = layer["id"].as_str().ok_or("Missing layer ID")?;
        for link in ["parentFolderId", "maskLayerId", "maskSourceLayerId"] {
            if !layer[link].is_null() && !layer[link].is_string() {
                return Err("Layer links must be layer IDs".into());
            }
            let mut seen = BTreeSet::from([id]);
            let mut current = layer;
            while let Some(next) = current[link].as_str() {
                if !seen.insert(next) || seen.len() > 65 {
                    return Err("Layer links contain a cycle or exceed the nesting limit".into());
                }
                current = by_id
                    .get(next)
                    .copied()
                    .ok_or("Layer link refers to a missing layer")?;
                if link == "parentFolderId" && current["kind"] != "Folder" {
                    return Err("A layer parent must be a folder".into());
                }
            }
        }
    }
    // Rendering follows folder children and mask sources together. Checking each
    // link kind independently misses cycles spanning both kinds of dependency.
    let mask_sources: BTreeSet<_> = layers
        .iter()
        .filter_map(|l| l["maskLayerId"].as_str())
        .collect();
    let mut done = BTreeSet::new();
    fn visit<'a>(
        id: &'a str,
        by_id: &BTreeMap<&'a str, &'a Value>,
        masks: &BTreeSet<&str>,
        active: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result {
        if done.contains(id) {
            return Ok(());
        }
        if !active.insert(id) || active.len() > 65 {
            return Err(
                "Icon render dependencies contain a cycle or exceed the nesting limit".into(),
            );
        }
        let layer = by_id.get(id).ok_or("Missing render dependency")?;
        if let Some(mask) = layer["maskLayerId"].as_str() {
            visit(mask, by_id, masks, active, done)?;
        }
        if layer["kind"] == "MaskReference"
            && let Some(source) = layer["maskSourceLayerId"].as_str()
        {
            visit(source, by_id, masks, active, done)?;
        }
        if layer["kind"] == "Folder" {
            for (child, value) in by_id {
                if value["parentFolderId"].as_str() == Some(id)
                    && !masks.contains(child)
                    && value["visible"] != false
                {
                    visit(child, by_id, masks, active, done)?;
                }
            }
        }
        active.remove(id);
        done.insert(id);
        Ok(())
    }
    for id in by_id.keys() {
        visit(id, &by_id, &mask_sources, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(())
}
