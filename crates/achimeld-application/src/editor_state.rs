use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    #[serde(alias = "Offset")]
    pub offset: f64,
    #[serde(rename = "colorHex", alias = "ColorHex")]
    pub color_hex: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientPreset {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Stops")]
    pub stops: Vec<GradientStop>,
}
impl GradientPreset {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::document::valid_id(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.stops.is_empty()
            || self.stops.len() > 128
            || self.stops.iter().any(|stop| {
                !stop.offset.is_finite()
                    || !(0.0..=1.0).contains(&stop.offset)
                    || !stop.color_hex.starts_with('#')
                    || ![7, 9].contains(&stop.color_hex.len())
                    || !stop.color_hex.as_bytes()[1..]
                        .iter()
                        .all(u8::is_ascii_hexdigit)
            })
        {
            return Err("Invalid gradient preset".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayerLayout {
    pub floating: bool,
    pub dock_height: f32,
    pub split: f32,
    pub size: [f32; 2],
    pub position: Option<[f32; 2]>,
}
impl Default for LayerLayout {
    fn default() -> Self {
        Self {
            floating: true,
            dock_height: 320.0,
            split: 0.5,
            size: [760.0, 560.0],
            position: None,
        }
    }
}
impl LayerLayout {
    pub fn validate(&self) -> Result<(), String> {
        if !(180.0..=650.0).contains(&self.dock_height)
            || !(0.25..=0.75).contains(&self.split)
            || !(360.0..=8192.0).contains(&self.size[0])
            || !(180.0..=8192.0).contains(&self.size[1])
            || self
                .position
                .is_some_and(|p| p.iter().any(|n| !n.is_finite() || n.abs() > 100000.0))
        {
            return Err("Invalid layer window layout".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconStyle {
    pub id: String,
    pub name: String,
    pub recipe: Value,
    /// Original JSON libraries store their default icon as a base64 PNG.
    #[serde(default, alias = "PreviewPng", skip_serializing_if = "Option::is_none")]
    pub preview_png: Option<String>,
}
impl IconStyle {
    pub fn validate(&self) -> Result<(), String> {
        if !crate::document::valid_id(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || !self.recipe.is_object()
            || self.recipe["layers"].as_array().is_none_or(Vec::is_empty)
            || self
                .preview_png
                .as_ref()
                .is_some_and(|png| png.len() > 4 * 1024 * 1024)
        {
            return Err("Invalid icon style identity or recipe".into());
        }
        if serde_json::to_vec(&self.recipe)
            .map_err(|e| e.to_string())?
            .len()
            > 1024 * 1024
        {
            return Err("Icon style is too large".into());
        }
        crate::validation::entity(&json!({"icon":self.recipe}))
    }
}
/// Recover repeated explicit recipes in pre-library projects. Names and IDs are
/// stable on reload, and only the project content participates in discovery.
pub(crate) fn discover_styles(project: &Value) -> Result<Vec<IconStyle>, String> {
    use sha2::{Digest, Sha256};
    let mut groups = BTreeMap::new();
    for entity in project["achievements"].as_array().into_iter().flatten() {
        let recipe = &entity["icon"];
        if recipe["layers"].as_array().is_none_or(Vec::is_empty) {
            continue;
        }
        let candidate = IconStyle {
            preview_png: None,
            id: entity["id"].as_str().unwrap_or("").into(),
            name: "Recovered".into(),
            recipe: recipe.clone(),
        };
        if candidate.validate().is_err() {
            continue;
        }
        let signature = style_signature(recipe);
        let hash = hex::encode(Sha256::digest(
            serde_json::to_vec(&signature).map_err(|e| e.to_string())?,
        ));
        let entry = groups.entry(hash).or_insert((0usize, recipe));
        entry.0 += 1;
    }
    groups
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .take(128)
        .enumerate()
        .map(|(index, (hash, (_, recipe)))| {
            let style = IconStyle {
                preview_png: None,
                id: format!(
                    "{}-{}-{}-{}-{}",
                    &hash[..8],
                    &hash[8..12],
                    &hash[12..16],
                    &hash[16..20],
                    &hash[20..32]
                ),
                name: format!("Project style {}", index + 1),
                recipe: recipe.clone(),
            };
            style.validate()?;
            Ok(style)
        })
        .collect()
}
/// Identity-independent comparison, including folder/mask references by index.
/// General artwork and linked text are supplied by each achievement.
pub fn style_signature(recipe: &Value) -> Value {
    let mut signature = recipe.clone();
    if let Some(hex) = signature["backgroundColorHex"].as_str() {
        signature["backgroundColorHex"] = json!(hex.trim().to_uppercase());
    }
    let Some(layers) = signature["layers"].as_array_mut() else {
        return signature;
    };
    let indices: BTreeMap<_, _> = layers
        .iter()
        .enumerate()
        .filter_map(|(i, l)| Some((l["id"].as_str()?.to_owned(), i)))
        .collect();
    for layer in layers {
        for key in ["parentFolderId", "maskLayerId", "maskSourceLayerId"] {
            if let Some(id) = layer[key].as_str() {
                layer[key] = indices.get(id).map_or(json!(-1), |i| json!(i));
            }
        }
        if layer["text"]["linkToAchievementValue"] == true {
            layer["text"]["content"] = json!("");
        }
        if layer["kind"] == "AchievementImage" || layer["linkAtlasToGeneral"] == true {
            for key in ["imageId", "atlasId", "tileIndex"] {
                if let Some(object) = layer.as_object_mut() {
                    object.remove(key);
                }
            }
        }
        if let Some(object) = layer.as_object_mut() {
            object.remove("id");
            object.remove("displayName");
            object.remove("effectiveEffects");
        }
        if let Some(effects) = layer["effects"].as_array_mut() {
            for effect in effects {
                if let Some(object) = effect.as_object_mut() {
                    object.remove("id");
                }
            }
        }
        strip_nulls(layer);
    }
    strip_nulls(&mut signature);
    signature
}
fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|_, v| !v.is_null());
            for v in object.values_mut() {
                strip_nulls(v);
            }
        }
        Value::Array(array) => {
            for v in array {
                strip_nulls(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod project_style_tests {
    use super::*;
    use crate::{Command, Document, Editor, EntityKind, new_id};

    #[test]
    fn legacy_recipes_recover_shared_styles_without_changing_icons() {
        let mut editor = Editor::default();
        for _ in 0..3 {
            editor
                .execute("Add", Command::Add(EntityKind::Achievement), None)
                .unwrap();
        }
        let mut project = editor.document().project().clone();
        project.as_object_mut().unwrap().remove("iconStyles");
        for (i, entity) in project["achievements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            entity["icon"] = json!({"layers":[
                {"id":new_id(),"kind":"AchievementImage","imageId":new_id(),"tileIndex":i,"displayName":format!("Art {i}")},
                {"id":new_id(),"kind":"Text","text":{"linkToAchievementValue":true,"content":format!("Value {i}")}},
                {"id":new_id(),"kind":"Solid","opacity":if i == 2 {0.5} else {1.0},"solidColorHex":"#FF102030"}
            ]});
        }
        let original_icons = project["achievements"].clone();
        let first =
            Document::from_parts(project.clone(), editor.document().catalog().clone()).unwrap();
        let second = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        assert_eq!(first.icon_styles(), second.icon_styles());
        assert_eq!(first.icon_styles().len(), 1);
        assert_eq!(first.project()["achievements"], original_icons);
        assert_eq!(
            style_signature(&first.icon_styles()[0].recipe),
            style_signature(&original_icons[1]["icon"])
        );
        // An explicitly empty library stays empty, including after deletion/reload.
        let mut project = first.project().clone();
        project["iconStyles"] = json!([]);
        assert!(
            Document::from_parts(project, first.catalog().clone())
                .unwrap()
                .icon_styles()
                .is_empty()
        );
    }

    #[test]
    fn project_styles_validate_and_rejected_edits_preserve_history() {
        let style = IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Local".into(),
            recipe: json!({"layers":[{"id":new_id(),"kind":"Solid"}]}),
        };
        let mut editor = Editor::default();
        editor
            .execute("Save", Command::SaveIconStyle(style.clone()), None)
            .unwrap();
        let before = editor.document().clone();
        let revision = editor.revision();
        let mut invalid = style.clone();
        invalid.recipe["layers"][0]["opacity"] = json!(2.0);
        assert!(
            editor
                .execute("Bad", Command::SaveIconStyle(invalid), None)
                .is_err()
        );
        assert_eq!(editor.document(), &before);
        assert_eq!(editor.revision(), revision);
        let mut project = before.project().clone();
        project["iconStyles"] = json!([style.clone(), style]);
        assert!(Document::from_parts(project, before.catalog().clone()).is_err());
        editor
            .execute(
                "Delete",
                Command::DeleteIconStyle(before.icon_styles()[0].id.clone()),
                None,
            )
            .unwrap();
        assert!(editor.document().icon_styles().is_empty());
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(editor.undo());
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert_eq!(editor.document(), &before);
    }
}
