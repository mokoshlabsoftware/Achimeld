//! Local style cards share the canvas/artwork model but never enter the Steam plan.
use crate::{Catalog, Document, EntityKind};
use serde_json::Value;

pub(crate) fn new_node_style(entity: &mut Value, name: &str) -> crate::IconStyle {
    let id = crate::new_id();
    entity["iconStyleId"] = serde_json::json!(id);
    crate::IconStyle {
        id,
        name: name.into(),
        recipe: entity["icon"].clone(),
        preview_png: None,
    }
}

pub(crate) fn next_style_name(document: &Document) -> String {
    let styles = document.icon_styles();
    (1..)
        .map(|index| format!("Icon style {index}"))
        .find(|name| !styles.iter().any(|style| style.name == *name))
        .expect("Finite project style library")
}

/// Reconcile explicit node links inside the same validated history transaction.
/// Equal recipes on other helper cards never imply a relationship.
pub(crate) fn reconcile_styles(before: &Document, after: &mut Document) -> Result<(), String> {
    if before.project()["iconStyles"] == after.project()["iconStyles"]
        && !after.entities(EntityKind::Achievement).iter().any(|node| {
            is_style_achievement(node)
                && node["iconStyleId"].is_string()
                && before
                    .entity(EntityKind::Achievement, node["id"].as_str().unwrap_or(""))
                    .is_some_and(|old| {
                        old["icon"] != node["icon"] || old["iconStyleId"] != node["iconStyleId"]
                    })
        })
    {
        return Ok(());
    }
    let old_styles = before.icon_styles();
    let mut styles = after.icon_styles();
    // Collect recipe edits before updating nodes, so multiple cards linked to
    // one style receive the same final recipe regardless of their canvas order.
    for node in after.entities(EntityKind::Achievement) {
        if !is_style_achievement(node) {
            continue;
        }
        let old = before.entity(EntityKind::Achievement, node["id"].as_str().unwrap_or(""));
        if old.is_some_and(|old| {
            old["iconStyleId"] == node["iconStyleId"] && old["icon"] != node["icon"]
        }) && let Some(style) = node["iconStyleId"].as_str().and_then(|link| {
            styles
                .iter_mut()
                .find(|style| style.id.eq_ignore_ascii_case(link))
        }) {
            style.recipe = node["icon"].clone();
            style.preview_png = None;
            style.validate()?;
        }
    }
    let (project, _) = after.history_parts();
    for node in project["achievements"]
        .as_array_mut()
        .ok_or("Missing achievements")?
    {
        if !is_style_achievement(node) {
            continue;
        }
        let Some(link) = node["iconStyleId"].as_str().map(str::to_owned) else {
            continue;
        };
        let Some(style) = styles
            .iter()
            .find(|style| style.id.eq_ignore_ascii_case(&link))
        else {
            // Deleting a linked style retains the editable card and its artwork.
            if old_styles
                .iter()
                .any(|style| style.id.eq_ignore_ascii_case(&link))
            {
                node.as_object_mut()
                    .ok_or("Invalid icon style node")?
                    .remove("iconStyleId");
            }
            continue;
        };
        let old = before.entity(EntityKind::Achievement, node["id"].as_str().unwrap_or(""));
        if old.is_some_and(|old| old["iconStyleId"] != node["iconStyleId"])
            || old_styles
                .iter()
                .any(|old| old.id == style.id && old.recipe != style.recipe)
        {
            // Relinking adopts the chosen style without overwriting it. Renames
            // preserve the exact node recipe and layer selection identities.
            node["icon"] = style.recipe.clone();
        }
    }
    project["iconStyles"] = serde_json::json!(styles);
    Ok(())
}

pub fn is_style_achievement(entity: &Value) -> bool {
    entity["styleAchievement"] == true
}

/// Local recipe cards never participate in Steam or shared-style mutation.
pub fn is_local_icon(entity: &Value) -> bool {
    is_style_achievement(entity) || crate::art_images::is_art_image(entity)
}

/// Keep the existing worker contract; local cards are owned and restored by the host.
pub fn synchronization_project(document: &Document) -> Value {
    let mut project = document.project().clone();
    let locals: Vec<_> = document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| is_local_icon(entity))
        .collect();
    if let Some(entities) = project["achievements"].as_array_mut() {
        entities.retain(|entity| !is_local_icon(entity));
    }
    for key in ["syncBaselines", "tombstones"] {
        if let Some(records) = project[key].as_array_mut() {
            records.retain(|record| {
                !locals.iter().any(|local| {
                    record["entityId"] == local["id"] || record["localId"] == local["id"]
                })
            });
        }
    }
    project
}

/// Restore exact local records before validation/save, rejecting remote identity collisions.
pub fn restore(
    original: &Document,
    project: &mut Value,
    catalog: &mut Catalog,
) -> Result<(), String> {
    let entities = project["achievements"]
        .as_array_mut()
        .ok_or("Synchronization returned invalid achievements")?;
    if entities.iter().any(is_local_icon) {
        return Err("Synchronization returned a local style achievement".into());
    }
    let locals: Vec<_> = original
        .entities(EntityKind::Achievement)
        .iter()
        .enumerate()
        .filter(|(_, entity)| is_local_icon(entity))
        .collect();
    // Check all collisions before changing the result.
    for (_, local) in &locals {
        if entities.iter().any(|entity| {
            ["id", "apiName"].iter().any(|key| {
                entity[*key]
                    .as_str()
                    .zip(local[*key].as_str())
                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            })
        }) {
            return Err("Synchronization conflicts with a local style achievement identity".into());
        }
    }
    for (index, local) in locals {
        entities.insert(index.min(entities.len()), local.clone());
        for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
            let id = local[key]
                .as_str()
                .ok_or("Missing local style text identity")?;
            let row = original
                .catalog()
                .rows
                .get(id)
                .ok_or("Missing local style text")?;
            catalog.rows.insert(id.into(), row.clone());
        }
    }
    Ok(())
}

pub fn require_preserved(original: &Document, result: &Document) -> Result<(), String> {
    // Inactive achievements are ordinary editable cards, but synchronization
    // has no authority to change their definition, recipe or localized text.
    for entity in original
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| {
            entity
                .get("steamSyncStatus")
                .is_some_and(|status| status == "Retired")
        })
    {
        if result.entity(EntityKind::Achievement, entity["id"].as_str().unwrap_or(""))
            != Some(entity)
        {
            return Err("Synchronization changed an inactive achievement".into());
        }
        for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
            let id = entity[key].as_str().unwrap_or("");
            if original.catalog().rows.get(id) != result.catalog().rows.get(id) {
                return Err("Synchronization changed inactive achievement text".into());
            }
        }
    }
    let locals = |document: &Document| {
        document
            .entities(EntityKind::Achievement)
            .iter()
            .filter(|entity| is_local_icon(entity))
            .cloned()
            .collect::<Vec<_>>()
    };
    if locals(original) != locals(result) {
        return Err("Synchronization changed local style achievements".into());
    }
    for entity in locals(original) {
        for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
            let id = entity[key].as_str().unwrap_or("");
            if original.catalog().rows.get(id) != result.catalog().rows.get(id) {
                return Err("Synchronization changed local style achievement text".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Editor, Preferences};
    use serde_json::json;

    fn fixture() -> Editor {
        let mut editor = Editor::default();
        for command in [
            Command::Add(EntityKind::Achievement),
            Command::AddStyleAchievementAt { position: [1, 1] },
            Command::Add(EntityKind::Achievement),
            Command::AddStyleAchievementAt { position: [35, 7] },
        ] {
            editor.execute("Create", command, None).unwrap();
        }
        editor
    }

    #[test]
    fn renamed_and_duplicated_local_icons_do_not_consume_achievement_numbers() {
        for convention in ["ACHIEVEMENT_{0:000}", "WIN_{0}"] {
            let preferences = Preferences {
                naming_convention: convention.into(),
                ..Default::default()
            };
            let mut editor = Editor::default().with_preferences(preferences.clone());
            for command in [
                Command::AddStyleAchievementAt { position: [0, 0] },
                Command::AddArtImageAt { position: [20, 0] },
            ] {
                editor.execute("Local icon", command, None).unwrap();
                let local = editor
                    .document()
                    .entities(EntityKind::Achievement)
                    .last()
                    .unwrap()
                    .clone();
                let id = local["id"].as_str().unwrap();
                editor
                    .execute(
                        "Rename local icon",
                        Command::SetLocalization {
                            id: local["displayNameLocalizationId"].as_str().unwrap().into(),
                            language: "English".into(),
                            text: "A renamed icon without a style prefix".into(),
                        },
                        None,
                    )
                    .unwrap();
                editor
                    .execute(
                        "Duplicate local icon",
                        Command::Duplicate {
                            kind: EntityKind::Achievement,
                            id: id.into(),
                        },
                        None,
                    )
                    .unwrap();
            }
            let locals = editor.document().clone();
            assert_eq!(locals.steam_entities(EntityKind::Achievement).count(), 0);
            assert_eq!(locals.project()["nextAchievementInternalId"], 1);
            // Reloading retains the markers independently from the editable name.
            editor = Editor::new(
                Document::from_parts(locals.project().clone(), locals.catalog().clone()).unwrap(),
            )
            .with_preferences(preferences.clone());
            for index in 1..=2 {
                editor
                    .execute(
                        "Add achievement",
                        Command::Add(EntityKind::Achievement),
                        None,
                    )
                    .unwrap();
                let document = editor.document();
                let real = document.entities(EntityKind::Achievement).last().unwrap();
                assert_eq!(
                    real["apiName"],
                    preferences.achievement_api_name(index).unwrap()
                );
                assert_eq!(
                    document.display_name(real, "English"),
                    format!("Achievement {index}")
                );
                assert_eq!(real["internalId"], index);
                assert_eq!(real["displayOrder"], index - 1);
                assert_eq!(document.project()["nextAchievementInternalId"], index + 1);
                assert_eq!(
                    document.steam_entities(EntityKind::Achievement).count(),
                    index
                );
                assert_eq!(
                    synchronization_project(document)["achievements"]
                        .as_array()
                        .unwrap()
                        .len(),
                    index
                );
                assert!(editor.undo());
                assert_eq!(
                    editor
                        .document()
                        .steam_entities(EntityKind::Achievement)
                        .count(),
                    index - 1
                );
                assert!(editor.redo());
            }
            require_preserved(&locals, editor.document()).unwrap();
        }
    }

    #[test]
    fn legacy_numbering_excludes_local_icons_and_sync_metadata() {
        let editor = fixture();
        let mut project = editor.document().project().clone();
        project
            .as_object_mut()
            .unwrap()
            .remove("nextAchievementInternalId");
        for entity in project["achievements"].as_array_mut().unwrap() {
            entity.as_object_mut().unwrap().remove("internalId");
        }
        let local_id = project["achievements"][1]["id"].clone();
        let real_id = project["achievements"][0]["id"].clone();
        for key in ["syncBaselines", "tombstones"] {
            project[key] = json!([
                {"entityId":local_id,"entityType":"Achievement","apiName":"LOCAL"},
                {"localId":local_id,"entityType":"Achievement","apiName":"LOCAL_CURRENT"},
                {"entityId":real_id,"entityType":"Achievement","apiName":"REAL"}
            ]);
        }
        let loaded = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        assert_eq!(loaded.project()["nextAchievementInternalId"], 2);
        assert_eq!(loaded.entities(EntityKind::Achievement)[0]["internalId"], 0);
        assert_eq!(loaded.entities(EntityKind::Achievement)[2]["internalId"], 1);
        let projected = synchronization_project(&loaded);
        for key in ["syncBaselines", "tombstones"] {
            assert_eq!(projected[key].as_array().unwrap().len(), 1);
            assert_eq!(projected[key][0]["entityId"], real_id);
        }
    }

    #[test]
    fn icon_style_node_links_are_distinct_bidirectional_and_undoable() {
        let mut editor = Editor::default();
        for position in [[0, 0], [20, 0]] {
            editor
                .execute(
                    "Create icon style node",
                    Command::AddStyleAchievementAt { position },
                    None,
                )
                .unwrap();
        }
        let created = editor.document().clone();
        let first = created.entities(EntityKind::Achievement)[0].clone();
        let second = created.entities(EntityKind::Achievement)[1].clone();
        let link = first["iconStyleId"].as_str().unwrap();
        assert_ne!(first["iconStyleId"], second["iconStyleId"]);
        assert_ne!(created.icon_styles()[0].name, created.icon_styles()[1].name);
        let mut edited = first.clone();
        edited["icon"] =
            json!({"layers":[{"id":crate::new_id(),"kind":"Solid","solidColorHex":"#FF123456"}]});
        editor
            .execute(
                "Edit node",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: first["id"].as_str().unwrap().into(),
                    value: edited.clone(),
                },
                None,
            )
            .unwrap();
        let updated = editor.document().clone();
        assert_eq!(
            updated
                .icon_styles()
                .iter()
                .find(|style| style.id == link)
                .unwrap()
                .recipe,
            edited["icon"]
        );
        assert_eq!(updated.entities(EntityKind::Achievement)[1], second);
        assert!(editor.undo());
        assert_eq!(editor.document(), &created);
        assert!(editor.redo());
        assert_eq!(editor.document(), &updated);

        let mut style = updated
            .icon_styles()
            .into_iter()
            .find(|style| style.id == link)
            .unwrap();
        style.recipe["layers"][0]["solidColorHex"] = json!("#FF654321");
        style.name = "Named style".into();
        editor
            .execute(
                "Edit linked style",
                Command::SaveIconStyle(style.clone()),
                None,
            )
            .unwrap();
        assert_eq!(
            editor.document().entities(EntityKind::Achievement)[0]["icon"],
            style.recipe
        );
        assert_eq!(
            editor.document().entities(EntityKind::Achievement)[1],
            second
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &updated);
        editor
            .execute(
                "Delete linked style",
                Command::DeleteIconStyle(link.into()),
                None,
            )
            .unwrap();
        let node = &editor.document().entities(EntityKind::Achievement)[0];
        assert!(node.get("iconStyleId").is_none());
        assert_eq!(node["icon"], edited["icon"]);
        assert!(editor.undo());
        assert_eq!(editor.document(), &updated);
        assert_eq!(
            Document::from_parts(updated.project().clone(), updated.catalog().clone()).unwrap(),
            updated
        );

        let mut invalid = updated.project().clone();
        invalid["achievements"][0]["iconStyleId"] = json!(crate::new_id());
        assert!(Document::from_parts(invalid, updated.catalog().clone()).is_err());
    }

    #[test]
    fn node_relink_preserves_styles_and_shared_edits_follow_the_link_in_either_order() {
        let mut editor = fixture();
        let mut target = editor.document().icon_styles().pop().unwrap();
        target.recipe =
            json!({"layers":[{"id":crate::new_id(),"kind":"Solid","solidColorHex":"#FF123456"}]});
        editor
            .execute(
                "Distinct target style",
                Command::SaveIconStyle(target),
                None,
            )
            .unwrap();
        let before = editor.document().clone();
        let nodes: Vec<_> = before
            .entities(EntityKind::Achievement)
            .iter()
            .filter(|node| is_style_achievement(node))
            .cloned()
            .collect();
        let target = before
            .icon_styles()
            .into_iter()
            .find(|style| nodes[1]["iconStyleId"] == style.id)
            .unwrap();
        let mut relinked = nodes[0].clone();
        relinked["iconStyleId"] = json!(target.id);
        relinked["unlockValue"] = json!(27.5);
        editor
            .execute(
                "Relink",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: relinked["id"].as_str().unwrap().into(),
                    value: relinked,
                },
                None,
            )
            .unwrap();
        let linked = editor.document().clone();
        assert_eq!(linked.icon_styles(), before.icon_styles());
        assert_eq!(
            linked
                .entity(EntityKind::Achievement, nodes[0]["id"].as_str().unwrap())
                .unwrap()["icon"],
            target.recipe
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(editor.redo());
        assert_eq!(editor.document(), &linked);
        for source in &nodes {
            let id = source["id"].as_str().unwrap();
            let mut edited = linked.entity(EntityKind::Achievement, id).unwrap().clone();
            edited["icon"] = json!({"layers":[{"id":crate::new_id(),"kind":"Solid","solidColorHex":"#FF654321"}]});
            editor
                .execute(
                    "Edit shared node",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.into(),
                        value: edited.clone(),
                    },
                    None,
                )
                .unwrap();
            for node in &nodes {
                assert_eq!(
                    editor
                        .document()
                        .entity(EntityKind::Achievement, node["id"].as_str().unwrap())
                        .unwrap()["icon"],
                    edited["icon"]
                );
            }
            assert_eq!(editor.document().icon_styles()[0], before.icon_styles()[0]);
            assert!(editor.undo());
            assert_eq!(editor.document(), &linked);
        }
        let id = nodes[0]["id"].as_str().unwrap();
        let mut detached = linked.entity(EntityKind::Achievement, id).unwrap().clone();
        detached.as_object_mut().unwrap().remove("iconStyleId");
        editor
            .execute(
                "Detach",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.into(),
                    value: detached.clone(),
                },
                None,
            )
            .unwrap();
        assert_eq!(
            editor
                .document()
                .entity(EntityKind::Achievement, id)
                .unwrap(),
            &detached
        );
        assert_eq!(editor.document().icon_styles(), linked.icon_styles());
        let mut invalid = detached.clone();
        invalid["iconStyleId"] = json!(crate::new_id());
        assert!(
            editor
                .execute(
                    "Invalid link",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.into(),
                        value: invalid,
                    },
                    None
                )
                .is_err()
        );
        assert_eq!(
            editor
                .document()
                .entity(EntityKind::Achievement, id)
                .unwrap(),
            &detached
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &linked);
    }

    #[test]
    fn style_achievement_creation_duplicate_delete_and_identity_round_trip() {
        let mut editor = Editor::default();
        let empty = editor.document().clone();
        editor
            .execute(
                "Create icon style node",
                Command::AddStyleAchievementAt { position: [0, 0] },
                None,
            )
            .unwrap();
        let created = editor.document().clone();
        let local = &created.entities(EntityKind::Achievement)[0];
        let id = local["id"].as_str().unwrap();
        assert!(is_style_achievement(local));
        assert_eq!(local["unlockValue"], 10);
        assert_eq!(created.display_name(local, "English"), "Icon style node");
        let style = created.icon_styles().pop().unwrap();
        assert_eq!(local["iconStyleId"], style.id);
        assert_eq!(local["icon"], style.recipe);
        assert!(editor.undo());
        assert_eq!(editor.document(), &empty);
        assert!(editor.redo());
        assert_eq!(editor.document(), &created);
        assert_eq!(
            Document::from_parts(created.project().clone(), created.catalog().clone()).unwrap(),
            created
        );
        editor
            .execute(
                "Duplicate",
                Command::Duplicate {
                    kind: EntityKind::Achievement,
                    id: id.into(),
                },
                None,
            )
            .unwrap();
        let copy = &editor.document().entities(EntityKind::Achievement)[1];
        assert!(is_style_achievement(copy));
        assert_ne!(copy["id"], local["id"]);
        assert_ne!(copy["iconStyleId"], local["iconStyleId"]);
        assert_eq!(editor.document().icon_styles().len(), 2);
        assert_ne!(
            copy["displayNameLocalizationId"],
            local["displayNameLocalizationId"]
        );
        editor
            .execute(
                "Delete",
                Command::Delete {
                    kind: EntityKind::Achievement,
                    ids: vec![id.into()],
                },
                None,
            )
            .unwrap();
        assert!(
            editor.document().project()["tombstones"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let mut converted = local.clone();
        converted["styleAchievement"] = json!(false);
        editor.undo();
        assert!(
            editor
                .execute(
                    "Convert",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.into(),
                        value: converted,
                    },
                    None
                )
                .is_err()
        );
        let mut invalid = created.project().clone();
        invalid["achievements"][0]["unlockValue"] = Value::Null;
        assert!(Document::from_parts(invalid, created.catalog().clone()).is_err());
    }

    #[test]
    fn sorting_always_preserves_local_cards_and_reserves_their_entire_rectangles() {
        let editor = fixture();
        let mut project = editor.document().project().clone();
        for (index, entity) in project["achievements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            if !is_style_achievement(entity) {
                entity["canvasColumn"] = json!(200 + index * 17);
                entity["canvasRow"] = json!(200);
            }
        }
        let original = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        let all: Vec<_> = original
            .entities(EntityKind::Achievement)
            .iter()
            .map(|entity| entity["id"].as_str().unwrap().to_owned())
            .collect();
        for columns in [false, true] {
            for selected_only in [false, true] {
                for by_style in [false, true] {
                    let preferences = Preferences {
                        fill_nodes_by_columns: columns,
                        order_selected_nodes_only: selected_only,
                        sort_nodes_by_icon_style: by_style,
                        sort_nodes_by_stat_activation: true,
                        new_line_per_icon_style: true,
                        ..Default::default()
                    };
                    let moves = crate::sorting::align_nodes(
                        &original,
                        EntityKind::Achievement,
                        &preferences,
                        &[],
                        &all,
                    );
                    let mut arranged = Editor::new(original.clone());
                    arranged
                        .execute("Sort", Command::Batch(moves), None)
                        .unwrap();
                    let span = crate::canvas::cell_span(EntityKind::Achievement).map(i64::from);
                    for local in original
                        .entities(EntityKind::Achievement)
                        .iter()
                        .filter(|entity| is_style_achievement(entity))
                    {
                        assert_eq!(
                            arranged
                                .document()
                                .entity(EntityKind::Achievement, local["id"].as_str().unwrap()),
                            Some(local)
                        );
                        for remote in arranged
                            .document()
                            .entities(EntityKind::Achievement)
                            .iter()
                            .filter(|entity| !is_style_achievement(entity))
                        {
                            let delta = ["canvasColumn", "canvasRow"].map(|key| {
                                (remote[key].as_i64().unwrap() - local[key].as_i64().unwrap()).abs()
                            });
                            assert!(delta[0] >= span[0] || delta[1] >= span[1]);
                        }
                    }
                    assert!(arranged.undo());
                    assert_eq!(arranged.document(), &original);
                }
            }
        }
        let local_only = vec![all[1].clone(), all[3].clone()];
        let preferences = Preferences {
            order_selected_nodes_only: true,
            ..Default::default()
        };
        assert!(
            crate::sorting::align_nodes(
                &original,
                EntityKind::Achievement,
                &preferences,
                &[],
                &local_only
            )
            .is_empty()
        );
    }

    #[test]
    fn synchronization_excludes_then_restores_local_cards_and_their_exact_text() {
        let mut editor = fixture();
        let original = editor.document().clone();
        let mut projected = synchronization_project(&original);
        assert_eq!(projected["achievements"].as_array().unwrap().len(), 2);
        assert!(
            projected["achievements"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entity| !is_style_achievement(entity))
        );
        let mut catalog = original.catalog().clone();
        for local in original
            .entities(EntityKind::Achievement)
            .iter()
            .filter(|entity| is_style_achievement(entity))
        {
            for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
                catalog.rows.remove(local[key].as_str().unwrap());
            }
        }
        projected["achievements"][0]["hidden"] = json!(true);
        restore(&original, &mut projected, &mut catalog).unwrap();
        let result = Document::from_parts(projected, catalog).unwrap();
        require_preserved(&original, &result).unwrap();
        editor
            .execute(
                "Sync",
                Command::ApplySynchronization(Box::new(result.clone())),
                None,
            )
            .unwrap();
        editor.undo();
        assert_eq!(editor.document(), &original);
        editor.redo();
        assert_eq!(editor.document(), &result);
        let mut modified = result.project().clone();
        modified["achievements"][1]["unlockValue"] = json!(50);
        let invalid = Document::from_parts(modified, result.catalog().clone()).unwrap();
        assert!(
            editor
                .execute(
                    "Invalid sync",
                    Command::ApplySynchronization(Box::new(invalid)),
                    None
                )
                .is_err()
        );
        assert_eq!(editor.document(), &result);
    }

    #[test]
    fn synchronization_rejects_remote_collisions_and_local_records() {
        let editor = fixture();
        let original = editor.document();
        for key in ["apiName", "id", "styleAchievement"] {
            let mut project = synchronization_project(original);
            project["achievements"][0][key] =
                original.entities(EntityKind::Achievement)[1][key].clone();
            let before = project.clone();
            let mut catalog = original.catalog().clone();
            assert!(restore(original, &mut project, &mut catalog).is_err());
            assert_eq!(project, before);
        }
    }
}
