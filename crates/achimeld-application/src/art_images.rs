//! Editable local compositions, linked by identity through General images.
use crate::{Document, EntityKind};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MAX_LINK_DEPTH: usize = 16;

pub fn is_art_image(entity: &Value) -> bool {
    entity["artImage"] == true
}

pub fn layer_source<'a>(entity: &'a Value, layer: &Value) -> Option<&'a str> {
    if layer["kind"] != "AchievementImage" {
        return None;
    }
    entity["generalImageEntries"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == layer["generalImageIndex"])?["artImageId"]
        .as_str()
}

/// Check the entire link graph once per edit, including unused General image slots.
/// Keeping it acyclic makes future layer changes safe and bounds render nesting.
pub(crate) fn validate(document: &Document) -> Result<(), String> {
    let entities: BTreeMap<_, _> = document
        .entities(EntityKind::Achievement)
        .iter()
        .map(|entity| (entity["id"].as_str().unwrap_or(""), entity))
        .collect();
    let mut edges = BTreeMap::new();
    for (id, entity) in &entities {
        let mut links = Vec::new();
        for entry in entity["generalImageEntries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if entry["artImageId"].is_null() {
                continue;
            }
            if entry["id"].as_u64().is_none_or(|id| id > 20_000) {
                return Err("Art image links require a General image slot ID".into());
            }
            let target = entry["artImageId"]
                .as_str()
                .ok_or("Invalid art image link")?;
            if !entry["imageId"].is_null()
                || !entry["atlasId"].is_null()
                || !entities
                    .get(target)
                    .is_some_and(|entity| is_art_image(entity))
            {
                return Err(
                    "General image must reference an existing art image exclusively".into(),
                );
            }
            if links.len() >= 512 {
                return Err("Too many art image links in one icon".into());
            }
            links.push(target);
        }
        edges.insert(*id, links);
    }
    fn depth<'a>(
        id: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        states: &mut BTreeMap<&'a str, Option<usize>>,
        nesting: usize,
    ) -> Result<usize, String> {
        if nesting > MAX_LINK_DEPTH {
            return Err("Art image links are too deeply nested".into());
        }
        if let Some(state) = states.get(id) {
            return state
                .ok_or_else(|| "Art images cannot link to themselves or form a cycle".into());
        }
        states.insert(id, None);
        let mut longest = 0;
        for target in &edges[id] {
            longest = longest.max(1 + depth(target, edges, states, nesting + 1)?);
        }
        if longest > MAX_LINK_DEPTH {
            return Err("Art image links are too deeply nested".into());
        }
        states.insert(id, Some(longest));
        Ok(longest)
    }
    let mut states = BTreeMap::new();
    for id in edges.keys() {
        depth(id, &edges, &mut states, 0)?;
    }
    Ok(())
}

/// Preserve slot numbers and layer recipes when a source is deleted; undo restores links.
pub(crate) fn unlink_deleted(project: &mut Value, ids: &[String]) {
    for entity in project["achievements"].as_array_mut().into_iter().flatten() {
        for entry in entity
            .get_mut("generalImageEntries")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if entry["artImageId"]
                .as_str()
                .is_some_and(|id| ids.iter().any(|removed| removed == id))
                && let Some(entry) = entry.as_object_mut()
            {
                entry.remove("artImageId");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Editor, Preferences, new_id};
    use serde_json::json;
    fn fixture() -> Editor {
        let mut editor = Editor::default();
        editor
            .execute(
                "Create",
                Command::Batch(vec![
                    Command::AddArtImageAt { position: [0, 0] },
                    Command::AddArtImageAt { position: [40, 0] },
                    Command::Add(EntityKind::Achievement),
                ]),
                None,
            )
            .unwrap();
        editor
    }
    fn link(entity: &mut Value, source: &str) {
        entity["generalImageEntries"] = json!([{"id":0,"artImageId":source}]);
        entity["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}]});
    }
    fn replace(editor: &mut Editor, entity: Value) -> Result<(), String> {
        editor.execute(
            "Link",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: entity["id"].as_str().unwrap().into(),
                value: entity,
            },
            None,
        )
    }
    #[test]
    fn art_images_create_duplicate_link_delete_and_undo_as_local_images() {
        let mut editor = fixture();
        let art = editor.document().entities(EntityKind::Achievement)[0].clone();
        let art_id = art["id"].as_str().unwrap().to_owned();
        assert!(is_art_image(&art));
        assert!(!crate::style_achievements::is_style_achievement(&art));
        assert!(art["unlockValue"].is_null());
        let mut target = editor.document().entities(EntityKind::Achievement)[2].clone();
        link(&mut target, &art_id);
        replace(&mut editor, target).unwrap();
        let before = editor.document().clone();
        editor
            .execute(
                "Duplicate",
                Command::Duplicate {
                    kind: EntityKind::Achievement,
                    id: art_id.clone(),
                },
                None,
            )
            .unwrap();
        let copy = editor
            .document()
            .entities(EntityKind::Achievement)
            .last()
            .unwrap();
        assert!(is_art_image(copy));
        assert_ne!(copy["id"], art["id"]);
        editor.undo();
        assert_eq!(editor.document(), &before);
        editor
            .execute(
                "Delete",
                Command::Delete {
                    kind: EntityKind::Achievement,
                    ids: vec![art_id],
                },
                None,
            )
            .unwrap();
        let target = &editor.document().entities(EntityKind::Achievement)[1];
        assert_eq!(target["generalImageEntries"][0]["id"], 0);
        assert!(target["generalImageEntries"][0]["artImageId"].is_null());
        assert_eq!(target["icon"]["layers"][0]["generalImageIndex"], 0);
        editor.undo();
        assert_eq!(editor.document(), &before);
        let round_trip = Document::from_parts(
            serde_json::from_str(&before.project().to_string()).unwrap(),
            before.catalog().clone(),
        )
        .unwrap();
        assert_eq!(round_trip, before);
    }
    #[test]
    fn art_image_cycles_identity_changes_and_achievement_settings_are_rejected_atomically() {
        let mut editor = fixture();
        let a = editor.document().entities(EntityKind::Achievement)[0].clone();
        let b = editor.document().entities(EntityKind::Achievement)[1].clone();
        let a_id = a["id"].as_str().unwrap();
        let b_id = b["id"].as_str().unwrap();
        let mut linked = b.clone();
        link(&mut linked, a_id);
        replace(&mut editor, linked).unwrap();
        let before = editor.document().clone();
        for source in [a_id, b_id, "missing"] {
            let mut invalid = a.clone();
            link(&mut invalid, source);
            assert!(replace(&mut editor, invalid).is_err());
            assert_eq!(editor.document(), &before);
        }
        for (key, value) in [
            ("artImage", json!(false)),
            ("styleAchievement", json!(true)),
            ("unlockValue", json!(10)),
        ] {
            let mut invalid = a.clone();
            invalid[key] = value;
            assert!(replace(&mut editor, invalid).is_err());
        }
        let mut invalid = a.clone();
        link(&mut invalid, b_id);
        invalid["generalImageEntries"][0]["imageId"] = json!(new_id());
        assert!(replace(&mut editor, invalid).is_err());
    }
    #[test]
    fn art_images_stay_local_through_sync_and_sorting_while_links_survive() {
        let mut editor = fixture();
        let art = editor.document().entities(EntityKind::Achievement)[0].clone();
        let mut target = editor.document().entities(EntityKind::Achievement)[2].clone();
        link(&mut target, art["id"].as_str().unwrap());
        replace(&mut editor, target).unwrap();
        let original = editor.document();
        let mut project = crate::style_achievements::synchronization_project(original);
        assert_eq!(project["achievements"].as_array().unwrap().len(), 1);
        let mut catalog = original.catalog().clone();
        crate::style_achievements::restore(original, &mut project, &mut catalog).unwrap();
        let restored = Document::from_parts(project, catalog).unwrap();
        assert_eq!(&restored, original);
        crate::style_achievements::require_preserved(original, &restored).unwrap();
        let sorted = crate::sorting::align_nodes(
            original,
            EntityKind::Achievement,
            &Preferences::default(),
            &[],
            &[],
        );
        for command in sorted {
            if let Command::Move { id, .. } = command {
                assert!(!is_art_image(
                    original.entity(EntityKind::Achievement, &id).unwrap()
                ));
            }
        }
    }
    #[test]
    fn art_image_dependency_chains_have_a_bounded_depth() {
        let mut editor = Editor::default();
        editor
            .execute(
                "Create",
                Command::Batch(
                    (0..18)
                        .map(|i| Command::AddArtImageAt {
                            position: [i * 20, 0],
                        })
                        .collect(),
                ),
                None,
            )
            .unwrap();
        let mut project = editor.document().project().clone();
        let ids: Vec<_> = project["achievements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect();
        for i in 1..17 {
            link(&mut project["achievements"][i], &ids[i - 1]);
        }
        Document::from_parts(project.clone(), editor.document().catalog().clone()).unwrap();
        link(&mut project["achievements"][17], &ids[16]);
        assert!(
            Document::from_parts(project, editor.document().catalog().clone())
                .unwrap_err()
                .contains("deeply nested")
        );
    }
}
