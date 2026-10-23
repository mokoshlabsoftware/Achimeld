//! Pure artwork identity edits. Immutable files remain owned by the host and undo.
use crate::{document::Result, new_id};
use serde_json::{Value, json};

const DEFAULT_ATLAS: &str = "00000000-0000-0000-0000-000000000000";

fn includes_default(project: &Value, id: &str) -> bool {
    id == "default"
        || project["assets"]["atlases"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| entry["image"] == project["assets"]["atlas"])
            .is_some_and(|entry| entry["id"] == id)
}
fn atlas_matches(id: &str, reference: &Value, default: bool) -> bool {
    reference.as_str() == Some(id)
        || (default && (reference.is_null() || reference.as_str() == Some(DEFAULT_ATLAS)))
}

/// Every achievement using this identity, once, including hidden layers and masks.
pub fn artwork_usage<'a>(project: &'a Value, atlas: bool, id: &str) -> Vec<&'a Value> {
    let default = atlas && includes_default(project, id);
    project["achievements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|achievement| {
            let general = |entry: &Value| {
                if atlas {
                    !entry["atlasId"].is_null() && atlas_matches(id, &entry["atlasId"], default)
                } else {
                    entry["imageId"].as_str() == Some(id)
                }
            };
            let legacy_general = if atlas {
                !achievement["generalAtlasId"].is_null()
                    && atlas_matches(id, &achievement["generalAtlasId"], default)
            } else {
                achievement["generalImageId"].as_str() == Some(id)
            };
            legacy_general
                || achievement["generalImageEntries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(general)
                || (!atlas && achievement["unachievedImageId"].as_str() == Some(id))
                || if achievement["icon"].is_null() {
                    if atlas {
                        achievement["standaloneImageId"].is_null()
                            && atlas_matches(id, &achievement["atlasId"], default)
                    } else {
                        achievement["standaloneImageId"].as_str() == Some(id)
                    }
                } else {
                    achievement["icon"]["layers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|layer| match layer["kind"].as_str() {
                            Some("AtlasTile") if atlas => atlas_matches(
                                id,
                                if layer["linkAtlasToGeneral"] == true {
                                    &achievement["atlasId"]
                                } else {
                                    &layer["atlasId"]
                                },
                                default,
                            ),
                            Some("StandaloneImage" | "SteamImportedArtwork") if !atlas => {
                                layer["imageId"].as_str() == Some(id)
                            }
                            _ => false,
                        })
                }
        })
        .collect()
}

pub(crate) fn duplicate(
    project: &mut Value,
    atlas: bool,
    id: &str,
    new: &str,
    name: &str,
) -> Result<()> {
    if !crate::document::valid_id(new) {
        return Err("Invalid artwork identity".into());
    }
    let key = if atlas { "atlases" } else { "images" };
    let mut entry = if atlas && id == "default" {
        if project["assets"]["atlas"].is_null() {
            return Err("Artwork no longer exists".into());
        }
        json!({"image":project["assets"]["atlas"],"importSource":project["assets"]["atlasImportSource"]})
    } else {
        project["assets"][key]
            .as_array()
            .and_then(|items| items.iter().find(|entry| entry["id"] == id))
            .cloned()
            .ok_or("Artwork no longer exists")?
    };
    let items = project["assets"][key]
        .as_array_mut()
        .ok_or("Missing asset library")?;
    if items.iter().any(|entry| entry["id"] == new) {
        return Err("Duplicate artwork identity".into());
    }
    entry["id"] = json!(new);
    entry["displayName"] = json!(name);
    items.push(entry);
    Ok(())
}

pub(crate) fn delete(project: &mut Value, atlas: bool, id: &str) -> Result<()> {
    let default = atlas && includes_default(project, id);
    let key = if atlas { "atlases" } else { "images" };
    let items = project["assets"][key]
        .as_array_mut()
        .ok_or("Missing asset library")?;
    if !(atlas && id == "default") && !items.iter().any(|entry| entry["id"] == id) {
        return Err("Artwork no longer exists".into());
    }
    items.retain(|entry| entry["id"] != id);
    if default {
        project["assets"]["atlas"] = Value::Null;
        project["assets"]["atlasImportSource"] = Value::Null;
    }
    for achievement in project["achievements"].as_array_mut().into_iter().flatten() {
        // Materialize legacy artwork first; clearing its ID must not select the default atlas.
        let legacy = achievement["icon"].is_null()
            && if atlas {
                achievement["standaloneImageId"].is_null()
                    && atlas_matches(id, &achievement["atlasId"], default)
            } else {
                achievement["standaloneImageId"].as_str() == Some(id)
            };
        if legacy {
            achievement["icon"] = json!({"layers":[{"id":new_id(),"kind":"StandaloneImage",
                "displayName":"","imageId":null,"visible":true,"opacity":1}]});
        }
        let linked_atlas = achievement["atlasId"].clone();
        for layer in achievement
            .get_mut("icon")
            .and_then(|icon| icon.get_mut("layers"))
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            let affected = if atlas {
                layer["kind"] == "AtlasTile"
                    && atlas_matches(
                        id,
                        if layer["linkAtlasToGeneral"] == true {
                            &linked_atlas
                        } else {
                            &layer["atlasId"]
                        },
                        default,
                    )
            } else {
                matches!(
                    layer["kind"].as_str(),
                    Some("StandaloneImage" | "SteamImportedArtwork")
                ) && layer["imageId"].as_str() == Some(id)
            };
            if affected {
                if atlas {
                    layer["kind"] = json!("StandaloneImage");
                }
                layer["imageId"] = Value::Null;
                layer["atlasId"] = Value::Null;
                layer["linkAtlasToGeneral"] = json!(false);
            }
        }
        for entry in achievement
            .get_mut("generalImageEntries")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            let affected = if atlas {
                !entry["atlasId"].is_null() && atlas_matches(id, &entry["atlasId"], default)
            } else {
                entry["imageId"].as_str() == Some(id)
            };
            if affected {
                entry["imageId"] = Value::Null;
                entry["atlasId"] = Value::Null;
            }
        }
        for field in if atlas {
            ["atlasId", "generalAtlasId"]
        } else {
            ["standaloneImageId", "generalImageId"]
        } {
            let affected = if atlas {
                !achievement[field].is_null() && atlas_matches(id, &achievement[field], default)
            } else {
                achievement[field].as_str() == Some(id)
            };
            if affected {
                achievement[field] = Value::Null;
            }
        }
        if !atlas && achievement["unachievedImageId"].as_str() == Some(id) {
            achievement["unachievedImageId"] = Value::Null;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Document, Editor, EntityKind};
    #[test]
    fn delete_artwork_preserves_layers_general_indices_masks_and_undo() {
        let mut project = Document::new().project().clone();
        let image = new_id();
        project["assets"]["images"] = json!([{"id":image,"displayName":"Source","image":{"kind":"RelativeToProject","path":"image.png"}}]);
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut a = editor.document().entities(EntityKind::Achievement)[0].clone();
        let layer = new_id();
        a["generalImageEntries"] = json!([{"id":0,"imageId":image},{"id":1,"imageId":"other"}]);
        a["unachievedImageId"] = json!(image);
        a["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","maskLayerId":layer},
            {"id":layer,"kind":"StandaloneImage","imageId":image,"visible":false},
            {"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}]});
        let mut b = a.clone();
        b["id"] = json!(new_id());
        b["apiName"] = json!("LEGACY");
        b["generalImageEntries"] = json!([]);
        b["unachievedImageId"] = Value::Null;
        b["standaloneImageId"] = json!(image);
        b["icon"] = Value::Null;
        project["achievements"] = json!([a, b]);
        let doc = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        assert_eq!(artwork_usage(doc.project(), false, &image).len(), 2);
        let mut editor = Editor::new(doc.clone());
        editor
            .execute(
                "Delete artwork",
                Command::DeleteArtwork {
                    atlas: false,
                    id: image.clone(),
                },
                None,
            )
            .unwrap();
        let p = editor.document().project();
        assert_eq!(p["assets"]["images"], json!([]));
        assert_eq!(p["achievements"][0]["icon"]["layers"][1]["id"], layer);
        assert!(p["achievements"][0]["icon"]["layers"][1]["imageId"].is_null());
        assert_eq!(
            p["achievements"][0]["icon"]["layers"][0]["maskLayerId"],
            layer
        );
        assert_eq!(
            p["achievements"][0]["generalImageEntries"][1]["imageId"],
            "other"
        );
        assert_eq!(p["achievements"][0]["generalImageEntries"][0]["id"], 0);
        assert_eq!(
            p["achievements"][1]["icon"]["layers"][0]["kind"],
            "StandaloneImage"
        );
        assert!(artwork_usage(p, false, &image).is_empty());
        assert!(editor.undo());
        assert_eq!(editor.document(), &doc);
        assert!(editor.redo());
        assert!(artwork_usage(editor.document().project(), false, &image).is_empty());
    }
    #[test]
    fn default_atlas_aliases_and_linked_layers_are_cleared_without_fallback() {
        let id = new_id();
        let mut p = json!({"assets":{"atlas":{"kind":"RelativeToProject","path":"a.png"},"images":[],
            "atlases":[{"id":id,"image":{"kind":"RelativeToProject","path":"a.png"}}]},
            "achievements":[{"atlasId":id,"icon":{"layers":[
                {"id":"linked","kind":"AtlasTile","atlasId":"ignored","linkAtlasToGeneral":true},
                {"id":"default","kind":"AtlasTile"},
                {"id":"other","kind":"AtlasTile","atlasId":"other"}]},
                "generalImageEntries":[{"id":0,"atlasId":DEFAULT_ATLAS,"tileIndex":7}]},
                {"icon":null}]});
        assert_eq!(artwork_usage(&p, true, &id).len(), 2);
        delete(&mut p, true, &id).unwrap();
        assert!(p["assets"]["atlas"].is_null());
        for index in [0, 1] {
            assert_eq!(
                p["achievements"][0]["icon"]["layers"][index]["kind"],
                "StandaloneImage"
            );
        }
        assert_eq!(
            p["achievements"][0]["icon"]["layers"][2]["atlasId"],
            "other"
        );
        assert!(p["achievements"][0]["generalImageEntries"][0]["atlasId"].is_null());
        assert_eq!(
            p["achievements"][1]["icon"]["layers"][0]["kind"],
            "StandaloneImage"
        );
    }
    #[test]
    fn duplicate_artwork_has_independent_identity_and_shares_immutable_content() {
        let mut p = Document::new().project().clone();
        let source = new_id();
        let copy = new_id();
        let entry = json!({"id":source,"displayName":"Image","image":{"kind":"RelativeToProject","path":"owned.png"},"pixelWidth":12});
        p["assets"]["images"] = json!([entry]);
        duplicate(&mut p, false, &source, &copy, "Image copy").unwrap();
        assert_eq!(p["assets"]["images"][1]["id"], copy);
        assert_eq!(
            p["assets"]["images"][1]["image"],
            p["assets"]["images"][0]["image"]
        );
        delete(&mut p, false, &copy).unwrap();
        assert_eq!(p["assets"]["images"], json!([entry]));
    }
}

#[cfg(test)]
mod alias_tests {
    use super::*;
    #[test]
    fn deleting_an_unused_atlas_duplicate_preserves_the_original_and_default() {
        let original = new_id();
        let copy = new_id();
        let mut p = json!({"assets":{"atlas":{"kind":"RelativeToProject","path":"atlas.png"},"images":[],
            "atlases":[{"id":original,"image":{"kind":"RelativeToProject","path":"atlas.png"}}]},
            "achievements":[{"icon":null}]});
        duplicate(&mut p, true, &original, &copy, "Copy").unwrap();
        let before = p.clone();
        assert!(artwork_usage(&p, true, &copy).is_empty());
        delete(&mut p, true, &copy).unwrap();
        assert_eq!(p["assets"]["atlas"], before["assets"]["atlas"]);
        assert_eq!(p["achievements"], before["achievements"]);
        assert_eq!(p["assets"]["atlases"].as_array().unwrap().len(), 1);
    }
}
