use super::*;
use crate::{Editor, new_id};
use serde_json::json;

fn document(achievements: usize, stats: usize) -> Document {
    let mut editor = Editor::default();
    for (kind, count) in [
        (EntityKind::Achievement, achievements),
        (EntityKind::Stat, stats),
    ] {
        for _ in 0..count {
            editor.execute("Add", Command::Add(kind), None).unwrap();
        }
    }
    editor.document().clone()
}
fn rebuild(document: &Document, project: Value) -> Document {
    Document::from_parts(project, document.catalog().clone()).unwrap()
}
fn position(document: &Document, kind: EntityKind, id: &str) -> [i64; 2] {
    let entity = document.entity(kind, id).unwrap();
    [
        entity["canvasColumn"].as_i64().unwrap(),
        entity["canvasRow"].as_i64().unwrap(),
    ]
}
fn arranged(
    document: Document,
    kind: EntityKind,
    preferences: &Preferences,
    styles: &[IconStyle],
    selection: &[String],
) -> Editor {
    let commands = align_nodes(&document, kind, preferences, styles, selection);
    let mut editor = Editor::new(document);
    editor
        .execute("Align nodes to grid", Command::Batch(commands), None)
        .unwrap();
    editor
}
fn style(name: &str, color: &str) -> IconStyle {
    IconStyle {
        preview_png: None,
        id: new_id(),
        name: name.into(),
        recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":color}]}),
    }
}

#[test]
fn helpers_are_optional_and_follow_every_regular_sorted_node() {
    let kind = EntityKind::Achievement;
    let mut original = Editor::default();
    original
        .execute(
            "Interleaved helpers",
            Command::Batch(vec![
                Command::AddStyleAchievementAt {
                    position: [-80, -80],
                },
                Command::Add(kind),
                Command::AddArtImageAt {
                    position: [-60, -60],
                },
                Command::Add(kind),
            ]),
            None,
        )
        .unwrap();
    let helpers = style("A helpers", "#FF000000");
    let regular = style("Z regular", "#FFFFFFFF");
    let mut project = original.document().project().clone();
    for (index, entity) in project["achievements"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        entity["internalId"] = json!(100 - index);
        entity["icon"] = if crate::style_achievements::is_local_icon(entity) {
            helpers.recipe.clone()
        } else {
            regular.recipe.clone()
        };
        entity["icon"]["layers"][0]["id"] = json!(new_id());
    }
    let original = rebuild(original.document(), project);
    let styles = [helpers, regular];
    for columns in [false, true] {
        for by_style in [false, true] {
            for selected_only in [false, true] {
                for include in [false, true] {
                    let preferences = Preferences {
                        achievement_nodes_per_row: 3,
                        fill_nodes_by_columns: columns,
                        sort_nodes_by_icon_style: by_style,
                        new_line_per_icon_style: true,
                        sort_nodes_by_stat_activation: true,
                        order_selected_nodes_only: selected_only,
                        include_helpers_in_sorting: include,
                        ..Default::default()
                    };
                    let selection: Vec<_> = original
                        .entities(kind)
                        .iter()
                        .take(3)
                        .map(|entity| entity["id"].as_str().unwrap().to_owned())
                        .collect();
                    let mut editor =
                        arranged(original.clone(), kind, &preferences, &styles, &selection);
                    let span = canvas::cell_span(kind).map(i64::from);
                    let mut regular_slots = Vec::new();
                    let mut helper_slots = Vec::new();
                    for entity in editor.document().entities(kind) {
                        let id = entity["id"].as_str().unwrap();
                        let helper = crate::style_achievements::is_local_icon(entity);
                        if (helper && !include)
                            || (selected_only && !selection.iter().any(|selected| selected == id))
                        {
                            assert_eq!(
                                position(editor.document(), kind, id),
                                position(&original, kind, id)
                            );
                        } else {
                            let [x, y] = position(editor.document(), kind, id);
                            let slot = if columns {
                                x / span[0] * 3 + y / span[1]
                            } else {
                                y / span[1] * 3 + x / span[0]
                            };
                            if helper {
                                helper_slots.push(slot)
                            } else {
                                regular_slots.push(slot)
                            }
                        }
                    }
                    if include {
                        assert!(
                            regular_slots.iter().max().unwrap()
                                < helper_slots.iter().min().unwrap()
                        );
                    }
                    let after = editor.document().clone();
                    assert!(editor.undo());
                    assert_eq!(editor.document(), &original);
                    assert!(editor.redo());
                    assert_eq!(editor.document(), &after);
                }
            }
        }
    }
    assert!(!Preferences::default().include_helpers_in_sorting);
    assert!(
        !serde_json::from_value::<Preferences>(json!({}))
            .unwrap()
            .include_helpers_in_sorting
    );
    let preferences = Preferences {
        include_helpers_in_sorting: true,
        ..Default::default()
    };
    assert_eq!(
        serde_json::from_value::<Preferences>(serde_json::to_value(&preferences).unwrap()).unwrap(),
        preferences
    );
}

#[test]
fn id_sort_moves_cards_without_reordering_records_or_changing_identity_and_undoes_once() {
    let doc = document(2, 0);
    let mut project = doc.project().clone();
    project["achievements"].as_array_mut().unwrap().reverse();
    project["achievements"][0]["internalId"] = json!(1);
    project["achievements"][1]["internalId"] = json!(2);
    let doc = rebuild(&doc, project);
    let first_id = doc.entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap();
    let mut editor = arranged(
        doc.clone(),
        EntityKind::Achievement,
        &Preferences::default(),
        &[],
        &[],
    );
    assert_eq!(
        position(editor.document(), EntityKind::Achievement, first_id),
        [0, 0]
    );
    assert_eq!(editor.document().catalog(), doc.catalog());
    for (before, after) in doc
        .entities(EntityKind::Achievement)
        .iter()
        .zip(editor.document().entities(EntityKind::Achievement))
    {
        let mut expected = before.clone();
        expected["canvasColumn"] = after["canvasColumn"].clone();
        expected["canvasRow"] = after["canvasRow"].clone();
        assert_eq!(&expected, after);
    }
    // Column filling is also a single reversible operation.
    let preferences = Preferences {
        fill_nodes_by_columns: true,
        ..Default::default()
    };
    editor = arranged(doc.clone(), EntityKind::Achievement, &preferences, &[], &[]);
    let after = editor.document().clone();
    editor.undo();
    assert_eq!(editor.document(), &doc);
    assert!(editor.undo_label().is_none());
    editor.redo();
    assert_eq!(editor.document(), &after);
}

#[test]
fn style_groups_take_precedence_over_activation_and_start_new_rows_or_columns() {
    for columns in [false, true] {
        let doc = document(4, 1);
        let mut project = doc.project().clone();
        let adventure = style("adventure", "#FFFF0000");
        let violet = style("Violet", "#FF00FF00");
        let stat = project["stats"][0]["id"].clone();
        for (index, threshold) in [100, 1, 10, 0].into_iter().enumerate() {
            let node = &mut project["achievements"][index];
            node["icon"] = if index == 1 {
                violet.recipe.clone()
            } else {
                adventure.recipe.clone()
            };
            node["icon"]["layers"][0]["id"] = json!(new_id());
            if index != 3 {
                node["progressStatId"] = stat.clone();
                node["unlockValue"] = json!(threshold);
            }
        }
        let doc = rebuild(&doc, project);
        let preferences = Preferences {
            sort_nodes_by_icon_style: true,
            sort_nodes_by_stat_activation: true,
            new_line_per_icon_style: true,
            fill_nodes_by_columns: columns,
            ..Default::default()
        };
        let editor = arranged(
            doc.clone(),
            EntityKind::Achievement,
            &preferences,
            &[adventure, violet],
            &[],
        );
        let pos = |index: usize| {
            position(
                editor.document(),
                EntityKind::Achievement,
                doc.entities(EntityKind::Achievement)[index]["id"]
                    .as_str()
                    .unwrap(),
            )
        };
        assert_eq!(pos(2), [0, 0]);
        assert_eq!(pos(0), if columns { [0, 6] } else { [17, 0] });
        assert_eq!(pos(3), if columns { [0, 12] } else { [34, 0] });
        assert_eq!(pos(1), if columns { [17, 0] } else { [0, 6] });
    }
}

#[test]
fn selected_sort_avoids_whole_stationary_cards_for_both_kinds_and_directions() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for columns in [false, true] {
            for off_grid in [false, true] {
                let doc = document(2, 2);
                let mut project = doc.project().clone();
                let stationary = &mut project[kind.key()][0];
                stationary["canvasColumn"] = if off_grid { json!(1) } else { Value::Null };
                stationary["canvasRow"] = if off_grid { json!(1) } else { Value::Null };
                let doc = rebuild(&doc, project);
                let id = doc.entities(kind)[1]["id"].as_str().unwrap().to_owned();
                let preferences = Preferences {
                    fill_nodes_by_columns: columns,
                    order_selected_nodes_only: true,
                    ..Default::default()
                };
                let editor = arranged(
                    doc.clone(),
                    kind,
                    &preferences,
                    &[],
                    std::slice::from_ref(&id),
                );
                assert_eq!(&editor.document().entities(kind)[0], &doc.entities(kind)[0]);
                let steps = if off_grid { 2 } else { 1 };
                let span = canvas::cell_span(kind).map(i64::from);
                assert_eq!(
                    position(editor.document(), kind, &id),
                    if columns {
                        [0, span[1] * steps]
                    } else {
                        [span[0] * steps, 0]
                    }
                );
                assert_eq!(
                    editor.document().entities(if kind == EntityKind::Stat {
                        EntityKind::Achievement
                    } else {
                        EntityKind::Stat
                    }),
                    doc.entities(if kind == EntityKind::Stat {
                        EntityKind::Achievement
                    } else {
                        EntityKind::Stat
                    })
                );
                assert!(align_nodes(&doc, kind, &preferences, &[], &[]).is_empty());
            }
        }
    }
}

#[test]
fn stats_keep_display_order_and_use_their_own_line_count() {
    let doc = document(0, 3);
    let mut project = doc.project().clone();
    project["stats"][0]["displayOrder"] = json!(8);
    let doc = rebuild(&doc, project);
    let preferences = Preferences {
        stat_nodes_per_row: 2,
        sort_nodes_by_icon_style: true,
        sort_nodes_by_stat_activation: true,
        new_line_per_icon_style: true,
        ..Default::default()
    };
    let editor = arranged(doc.clone(), EntityKind::Stat, &preferences, &[], &[]);
    let pos = |index: usize| {
        position(
            editor.document(),
            EntityKind::Stat,
            doc.entities(EntityKind::Stat)[index]["id"]
                .as_str()
                .unwrap(),
        )
    };
    assert_eq!(pos(1), [0, 0]);
    assert_eq!(pos(2), [12, 0]);
    assert_eq!(pos(0), [0, 9]);
}

#[test]
fn grid_preferences_round_trip_and_old_settings_receive_original_defaults() {
    let defaults: Preferences = serde_json::from_str("{}").unwrap();
    assert_eq!(defaults.achievement_nodes_per_row, 5);
    assert_eq!(defaults.stat_nodes_per_row, 5);
    let preferences = Preferences {
        achievement_nodes_per_row: 3,
        stat_nodes_per_row: 7,
        sort_nodes_by_icon_style: true,
        sort_nodes_by_stat_activation: true,
        new_line_per_icon_style: true,
        fill_nodes_by_columns: true,
        order_selected_nodes_only: true,
        ..defaults
    };
    let copy: Preferences =
        serde_json::from_str(&serde_json::to_string(&preferences).unwrap()).unwrap();
    assert_eq!(copy, preferences);
    copy.validate().unwrap();
    let invalid = Preferences {
        stat_nodes_per_row: 0,
        ..copy
    };
    assert!(invalid.validate().is_err());
}
