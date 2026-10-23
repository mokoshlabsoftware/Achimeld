//! Mask identities and color conversion shared by the editor and compositor.
use serde_json::Value;
use std::collections::BTreeSet;

/// A mask source has a reduced editor; folder contents remain ordinary layers.
pub fn is_source(layers: &[Value], id: &str) -> bool {
    layers.iter().any(|layer| {
        layer["maskLayerId"] == id || (layer["id"] == id && layer["kind"] == "MaskReference")
    })
}

/// Applying a procedural preset edits mask content, never its layer controls.
/// Preserve legacy values without exposing new ways to edit them.
pub fn retain_source_controls(source: &Value, edited: &mut Value) {
    if let Some(object) = edited.as_object_mut() {
        for key in [
            "opacity",
            "blendMode",
            "effects",
            "maskLayerId",
            "maskMode",
            "maskInvert",
        ] {
            if let Some(value) = source.get(key) {
                object.insert(key.into(), value.clone());
            } else {
                object.remove(key);
            }
        }
    }
}

/// Includes folder contents so their property editors also stay grayscale.
pub fn content_ids(layers: &[Value]) -> BTreeSet<&str> {
    let mut ids: BTreeSet<_> = layers
        .iter()
        .filter_map(|layer| layer["maskLayerId"].as_str())
        .collect();
    loop {
        let count = ids.len();
        for layer in layers {
            if (layer["kind"] == "MaskReference"
                || layer["parentFolderId"]
                    .as_str()
                    .is_some_and(|id| ids.contains(id)))
                && let Some(id) = layer["id"].as_str()
            {
                ids.insert(id);
            }
        }
        if count == ids.len() {
            return ids;
        }
    }
}

pub fn grayscale(mut rgba: [u8; 4]) -> [u8; 4] {
    let gray =
        (0.2126 * f64::from(rgba[0]) + 0.7152 * f64::from(rgba[1]) + 0.0722 * f64::from(rgba[2]))
            .round() as u8;
    rgba[..3].fill(gray);
    rgba
}

/// Presets may contain nested gradients, effect colors and material colors.
pub fn grayscale_colors(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if key.ends_with("Hex")
                    && let Some(hex) = value.as_str().and_then(|s| s.strip_prefix('#'))
                    && matches!(hex.len(), 6 | 8)
                    && let Ok(n) = u32::from_str_radix(hex, 16)
                {
                    let alpha = if hex.len() == 8 { (n >> 24) as u8 } else { 255 };
                    let [r, g, b, a] = grayscale([(n >> 16) as u8, (n >> 8) as u8, n as u8, alpha]);
                    *value = Value::String(format!("#{a:02X}{r:02X}{g:02X}{b:02X}"));
                } else {
                    grayscale_colors(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(grayscale_colors),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Document, Editor, EntityKind, new_id};
    use serde_json::json;

    #[test]
    fn procedural_mask_presets_only_change_content_and_preserve_legacy_controls() {
        use crate::effect_presets::{EffectKind, EffectPreset};
        let preset = EffectPreset::from_layer(
            EffectKind::Particle,
            &json!({"kind":"Particle", "particles":crate::particles::ParticleSettings::default()}),
            "Mask content".into(),
        )
        .unwrap();
        for legacy in [false, true] {
            let mut mask = json!({"id":new_id(),"kind":"Particle"});
            if legacy {
                mask["opacity"] = json!(0.4);
                mask["effects"] = json!([{"kind":"Shadow"}]);
            }
            let before = mask.clone();
            preset.apply_to(&mut mask).unwrap();
            retain_source_controls(&before, &mut mask);
            assert_eq!(mask.get("opacity"), before.get("opacity"));
            assert_eq!(mask.get("effects"), before.get("effects"));
            assert!(mask["blendMode"].is_null());
            assert!(mask["maskLayerId"].is_null());
            assert!(mask["particles"].is_object());
        }
    }

    #[test]
    fn mask_reference_graph_validates_atomically_and_round_trips() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        editor.finish_group();
        let before = editor.document().clone();
        let mut entity = before.entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let mask = new_id();
        let source = new_id();
        let reference = new_id();
        let owner = new_id();
        entity["icon"] = json!({"layers":[
            {"id":mask,"kind":"Solid"},
            {"id":source,"kind":"Folder","maskLayerId":mask},
            {"id":reference,"kind":"MaskReference","maskSourceLayerId":source},
            {"id":owner,"kind":"Folder","maskLayerId":reference,"maskMode":"Luminance"}
        ]});
        let replace = |value| Command::ReplaceEntity {
            kind: EntityKind::Achievement,
            id: id.clone(),
            value,
        };
        editor
            .execute("Reference mask", replace(entity.clone()), None)
            .unwrap();
        editor.finish_group();
        let valid = editor.document().clone();
        for bad in [
            json!(false),
            json!(new_id()),
            json!(reference),
            json!(owner),
        ] {
            let mut value = entity.clone();
            value["icon"]["layers"][2]["maskSourceLayerId"] = bad;
            assert!(
                editor
                    .execute("Invalid reference", replace(value), None)
                    .is_err()
            );
            assert_eq!(editor.document(), &valid);
        }
        let mut mixed = entity.clone();
        mixed["icon"]["layers"][3]["parentFolderId"] = json!(source);
        assert!(editor.execute("Mixed cycle", replace(mixed), None).is_err());
        assert_eq!(editor.document(), &valid);
        let restored = Document::from_parts(
            serde_json::from_str(&serde_json::to_string(valid.project()).unwrap()).unwrap(),
            valid.catalog().clone(),
        )
        .unwrap();
        assert_eq!(restored, valid);
        editor.undo();
        assert_eq!(editor.document(), &before);
        editor.redo();
        assert_eq!(editor.document(), &valid);
    }

    #[test]
    fn mask_colors_preserve_transparency_and_normalize_nested_presets() {
        let mut value = json!({"solidColorHex":"#80FF0000","text":{"colorHex":"#00FF00"},
            "effects":[{"colorHex":"#400000FF"}],"displayName":"#FF0000"});
        grayscale_colors(&mut value);
        assert_eq!(value["solidColorHex"], "#80363636");
        assert_eq!(value["text"]["colorHex"], "#FFB6B6B6");
        assert_eq!(value["effects"][0]["colorHex"], "#40121212");
        assert_eq!(value["displayName"], "#FF0000");
        let first = value.clone();
        grayscale_colors(&mut value);
        assert_eq!(first, value);
    }
}
