//! Portable achievement recipes. These libraries never become project styles implicitly.
use crate::{IconStyle, new_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleCollection {
    pub id: String,
    pub name: String,
    pub styles: Vec<IconStyle>,
}

pub fn builtins() -> Result<Vec<StyleCollection>, String> {
    let mut collections: Vec<StyleCollection> =
        serde_json::from_str(include_str!("../assets/styles/originals.json"))
            .map_err(|e| e.to_string())?;
    let variants: Vec<StyleCollection> =
        serde_json::from_str(include_str!("../assets/styles/variants.json"))
            .map_err(|e| e.to_string())?;
    collections.extend(variants);
    let objects: Vec<StyleCollection> =
        serde_json::from_str(include_str!("../assets/styles/preview-objects.json"))
            .map_err(|e| e.to_string())?;
    collections.extend(objects);
    for collection in &collections {
        if !crate::document::valid_id(&collection.id) || collection.styles.len() > 128 {
            return Err("Invalid built-in style collection".into());
        }
        for style in &collection.styles {
            portable(style)?;
        }
    }
    Ok(collections)
}

/// Collection recipes use achievement image slots, never machine/project artwork IDs.
pub fn portable(style: &IconStyle) -> Result<(), String> {
    style.validate()?;
    fn inspect(value: &Value) -> Result<(), String> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "imageId" | "atlasId" | "path" | "filePath")
                        && !value.is_null()
                    {
                        return Err("Portable styles require AchievementImage slots instead of artwork IDs or paths".into());
                    }
                    inspect(value)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    inspect(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    inspect(&style.recipe)
}

pub fn clone_recipe(recipe: &Value) -> Result<Value, String> {
    let mut recipe = recipe.clone();
    let layers = recipe["layers"].as_array_mut().ok_or("Missing layers")?;
    let ids: BTreeMap<_, _> = layers
        .iter()
        .map(|layer| {
            Ok((
                layer["id"].as_str().ok_or("Missing layer ID")?.to_owned(),
                new_id(),
            ))
        })
        .collect::<Result<_, String>>()?;
    for layer in layers {
        for key in ["id", "parentFolderId", "maskLayerId", "maskSourceLayerId"] {
            if let Some(mapped) = layer[key].as_str().and_then(|id| ids.get(id)) {
                layer[key] = json!(mapped);
            }
        }
        for effect in layer["effects"].as_array_mut().into_iter().flatten() {
            effect["id"] = json!(new_id());
        }
    }
    Ok(recipe)
}

/// Replace existing JSON-pointer values only. Structural edits use a complete recipe.
pub fn create(name: &str, recipe: &Value, edits: &Value) -> Result<IconStyle, String> {
    let mut style = IconStyle {
        id: new_id(),
        name: name.trim().into(),
        recipe: recipe.clone(),
        preview_png: None,
    };
    portable(&style)?;
    if !edits.is_null() {
        let edits = edits.as_array().ok_or("edits must be an array")?;
        if edits.len() > 128 {
            return Err("Use at most 128 property edits".into());
        }
        for edit in edits {
            let path = edit["path"].as_str().ok_or("Missing edit path")?;
            if !path.starts_with('/')
                || path.split('/').any(|key| {
                    matches!(
                        key,
                        "id" | "parentFolderId"
                            | "maskLayerId"
                            | "maskSourceLayerId"
                            | "imageId"
                            | "atlasId"
                            | "kind"
                    )
                })
            {
                return Err("Property edits cannot change layer identities, types or links; supply a complete recipe".into());
            }
            let value = edit.get("value").ok_or("Missing edit value")?;
            let target = style
                .recipe
                .pointer_mut(path)
                .ok_or("Edit path does not exist; inspect get_icon_style first")?;
            if target.is_object() || target.is_array() || value.is_object() || value.is_array() {
                return Err("Property edits replace scalar values only; supply a complete recipe for structure changes".into());
            }
            *target = value.clone();
        }
    }
    portable(&style)?;
    style.recipe = clone_recipe(&style.recipe)?;
    style.validate()?;
    Ok(style)
}

pub fn summary(style: &IconStyle) -> Value {
    let mut slots = std::collections::BTreeSet::new();
    for layer in style.recipe["layers"].as_array().into_iter().flatten() {
        if layer["kind"] == "AchievementImage" {
            slots.insert(layer["generalImageIndex"].as_u64().unwrap_or(0));
        }
    }
    json!({"id":style.id,"name":style.name,"layerCount":style.recipe["layers"].as_array().map_or(0, Vec::len),"generalImageIndices":slots})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_portable_opt_in_and_clones_preserve_mask_links() {
        let collections = builtins().unwrap();
        assert!(collections.iter().map(|c| c.styles.len()).sum::<usize>() >= 8);
        assert!(crate::Document::new().icon_styles().is_empty());
        let original = &collections[0].styles[0];
        let copy = create("Copy", &original.recipe, &json!([])).unwrap();
        assert_ne!(copy.id, original.id);
        assert_eq!(
            crate::style_signature(&copy.recipe),
            crate::style_signature(&original.recipe)
        );
        let folder = copy.recipe["layers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["kind"] == "Folder")
            .unwrap();
        assert!(
            copy.recipe["layers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["id"] == folder["maskLayerId"])
        );
        assert!(
            create(
                "Bad",
                &original.recipe,
                &json!([{"path":"/layers/0/id","value":new_id()}])
            )
            .is_err()
        );
        assert!(
            create(
                "Bad",
                &original.recipe,
                &json!([{"path":"/layers/0/opacity","value":99}])
            )
            .is_err()
        );
        let mut external = original.clone();
        external.recipe["layers"][1]["imageId"] = json!(new_id());
        assert!(portable(&external).is_err());
    }
}
