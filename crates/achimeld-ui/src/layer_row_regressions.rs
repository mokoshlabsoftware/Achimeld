use super::*;

#[path = "effect_row_regressions.rs"]
mod effect_row_regressions;
#[path = "folder_mask_regressions.rs"]
mod folder_mask_regressions;
#[path = "layer_reroll_regressions.rs"]
mod layer_reroll_regressions;
#[path = "text_content_regressions.rs"]
mod text_content_regressions;

struct LayerRows {
    fixture: Fixture,
    desktop: Desktop,
    ctx: egui::Context,
    achievement: String,
    ids: Vec<String>,
    width: f32,
    time: f64,
    repeat_layout: bool,
}

impl LayerRows {
    fn new(width: f32, passes: usize) -> Self {
        let mut fixture = Fixture::combined_layers();
        let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        let achievement = entity["id"].as_str().unwrap().to_owned();
        let ids: Vec<_> = (0..3).map(|_| new_id()).collect();
        entity["icon"] = json!({"layers":ids.iter().enumerate().map(|(index,id)|json!({
            "id":id,"displayName":format!("Layer {index}"),"kind":"Solid",
            "solidColorHex":"#FFECA44B","visible":true,"opacity":1.0,
            "effects":[{"id":new_id(),"kind":"Outline","visible":true,"width":4.0,"colorHex":"#FFFFFFFF"}]
        })).collect::<Vec<_>>()});
        fixture
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        fixture.finish_group();
        let mut desktop = Desktop::default();
        desktop.selection = vec![achievement.clone()];
        desktop.selected_layer = Some(ids[0].clone());
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.options_mut(|options| {
            options.max_passes = std::num::NonZeroUsize::new(passes).unwrap()
        });
        let mut rows = Self {
            fixture,
            desktop,
            ctx,
            achievement,
            ids,
            width,
            time: 0.0,
            repeat_layout: false,
        };
        rows.frame(vec![]);
        rows.frame(vec![]);
        rows
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.input_frame(events, true, true)
    }

    fn input_frame(
        &mut self,
        events: Vec<egui::Event>,
        enabled: bool,
        focused: bool,
    ) -> egui::FullOutput {
        self.time += 0.05;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                time: Some(self.time),
                focused,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(self.width, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), self.fixture.preferences());
                if !enabled {
                    ui.disable();
                }
                self.desktop
                    .sync_blend_preview(ui.ctx(), &mut self.fixture, enabled);
                if self.repeat_layout && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("Layer controls layout regression");
                }
                self.desktop.layer_editor(ui, &mut self.fixture);
                self.desktop
                    .sync_blend_preview(ui.ctx(), &mut self.fixture, enabled);
            },
        );
        output.textures_delta.clear();
        output
    }

    fn row(&self, index: usize) -> egui::Rect {
        self.desktop.layer_palette.rows[&self.ids[index]]
    }

    fn press(&mut self, pos: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    }
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn text_layer_fx_opens_cold_menu_without_blocking_or_mutating_the_document() {
    // Keep a deadlock in native-style first-use icon/menu setup from hanging the suite.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut rows = LayerRows::new(1100.0, 2);
        let mut entity = rows.entity().clone();
        entity["icon"]["layers"][0]["kind"] = json!("Text");
        entity["icon"]["layers"][0]["text"] = json!("Text");
        entity["icon"]["layers"][0]["effects"] = json!([]);
        rows.fixture
            .execute(
                "Text recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: rows.achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        rows.frame(vec![]);
        rows.frame(vec![]);
        let before = rows.fixture.editor.document().clone();
        let fx = rows
            .ctx
            .read_response(layer_palette::layer_control_id(
                &rows.achievement,
                &rows.ids[0],
                "fx",
            ))
            .unwrap()
            .rect
            .center();
        rows.press(fx);
        rows.frame(vec![button(fx, false)]);
        let output = rows.frame(vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Outline")));
        assert_eq!(rows.fixture.editor.document(), &before);
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("Text FX menu must complete without deadlocking");
}

#[test]
fn eye_click_toggles_once_with_repeated_layout_and_undo() {
    for passes in [1, 2] {
        for same_frame in [false, true] {
            let mut rows = LayerRows::new(1100.0, passes);
            rows.repeat_layout = true;
            let eye = rows
                .ctx
                .read_response(layer_palette::layer_control_id(
                    &rows.achievement,
                    &rows.ids[0],
                    "eye",
                ))
                .unwrap()
                .rect
                .center();
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let mut events = vec![egui::Event::PointerMoved(eye), button(eye, true)];
            if same_frame {
                events.push(button(eye, false));
            }
            rows.frame(events);
            assert_eq!(
                rows.entity()["icon"]["layers"][0]["visible"],
                false,
                "passes={passes}, same_frame={same_frame}"
            );
            if !same_frame {
                rows.frame(vec![button(eye, false)]);
            }
            rows.frame(vec![]);
            assert_eq!(rows.entity()["icon"]["layers"][0]["visible"], false);
            assert_eq!(rows.fixture.editor.revision(), revision + 1);
            assert!(rows.desktop.layer_drag.is_none());
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
            rows.fixture.redo();
            rows.press(eye);
            rows.frame(vec![button(eye, false)]);
            assert_eq!(rows.entity()["icon"]["layers"][0]["visible"], true);
        }
    }
}

#[test]
fn blend_hover_previews_without_edits_and_restores_on_leave_dismiss_and_commit() {
    fn label_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|paint| match &paint.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing blend mode {label}"))
    }
    for width in [440.0, 1100.0] {
        for passes in [1, 2] {
            let mut rows = LayerRows::new(width, passes);
            rows.desktop.inspector_tab = desktop::InspectorTab::Icon;
            rows.desktop.layer_palette.open = true;
            rows.fixture.editor.mark_saved();
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let undo = rows.fixture.editor.undo_label().map(str::to_owned);
            let trigger = rows
                .ctx
                .read_response(layer_palette::layer_control_id(
                    &rows.achievement,
                    &rows.ids[1],
                    "blend",
                ))
                .unwrap()
                .rect
                .center();
            rows.press(trigger);
            rows.frame(vec![button(trigger, false)]);
            let menu = rows.frame(vec![]);
            let multiply = label_position(&menu, "Multiply");
            let screen = label_position(&menu, "Screen");
            rows.frame(vec![egui::Event::PointerMoved(multiply)]);
            rows.frame(vec![]);
            let preview = rows.fixture.blend_preview.as_ref().unwrap();
            assert_eq!(preview.achievement_id, rows.achievement);
            assert_eq!(preview.layer_id, rows.ids[1]);
            assert_eq!(preview.mode, "Multiply");
            assert_eq!(rows.fixture.editor.document(), &before);
            assert_eq!(rows.fixture.editor.revision(), revision);
            assert!(!rows.fixture.editor.is_dirty());
            assert_eq!(rows.fixture.editor.undo_label(), undo.as_deref());
            rows.frame(vec![egui::Event::PointerMoved(screen)]);
            rows.frame(vec![]);
            assert_eq!(rows.fixture.blend_preview.as_ref().unwrap().mode, "Screen");
            rows.frame(vec![egui::Event::PointerMoved(egui::pos2(1.0, 1.0))]);
            assert!(rows.fixture.blend_preview.is_none());
            rows.frame(vec![egui::Event::PointerMoved(multiply)]);
            rows.frame(vec![]);
            assert!(rows.fixture.blend_preview.is_some());
            rows.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(rows.fixture.blend_preview.is_none());
            assert_eq!(rows.fixture.editor.document(), &before);
            assert!(!rows.fixture.editor.is_dirty());
            rows.press(trigger);
            rows.frame(vec![button(trigger, false)]);
            let menu = rows.frame(vec![]);
            let difference = label_position(&menu, "Difference");
            rows.frame(vec![egui::Event::PointerMoved(difference)]);
            rows.frame(vec![]);
            assert!(rows.fixture.blend_preview.is_some());
            rows.press(difference);
            rows.frame(vec![button(difference, false)]);
            rows.frame(vec![]);
            assert!(rows.fixture.blend_preview.is_none());
            let layer = &rows
                .fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &rows.achievement)
                .unwrap()["icon"]["layers"][1];
            assert_eq!(layer["blendMode"], "Difference");
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
            rows.fixture.redo();
            assert_eq!(
                rows.fixture
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap()["icon"]["layers"][1]["blendMode"],
                "Difference"
            );
        }
    }
}

#[test]
fn blend_property_previews_every_layer_kind_and_keyboard_selection_survives_idle_mouse() {
    for kind in crate::layer_icons::LAYERS
        .into_iter()
        .chain(crate::layer_icons::ADJUSTMENTS)
        .chain(["Folder"])
    {
        let mut rows = LayerRows::new(1100.0, 2);
        rows.desktop.inspector_tab = desktop::InspectorTab::Icon;
        rows.desktop.layer_palette.open = true;
        let mut entity = rows.entity().clone();
        let mut layer = layer_palette::new_layer(kind);
        layer["id"] = json!(rows.ids[0]);
        entity["icon"]["layers"][0] = layer;
        rows.fixture
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: rows.achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        rows.fixture.finish_group();
        rows.frame(vec![]);
        rows.frame(vec![]);
        let trigger = rows
            .ctx
            .read_response(layer_palette::layer_control_id(
                &rows.achievement,
                &rows.ids[0],
                "property-blend",
            ))
            .expect(kind)
            .rect
            .center();
        let before = rows.fixture.editor.document().clone();
        rows.press(trigger);
        rows.frame(vec![button(trigger, false)]);
        let output = rows.frame(vec![]);
        let mut labels = std::collections::BTreeMap::new();
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && crate::blend_preview::MODES.contains(&text.galley.text())
            {
                labels.insert(
                    text.galley.text().to_owned(),
                    text.pos + text.galley.rect.center().to_vec2(),
                );
            }
        }
        assert_eq!(labels.len(), 8, "{kind}: all blend modes visible");
        let pointer = labels["Multiply"];
        rows.frame(vec![egui::Event::PointerMoved(pointer)]);
        assert_eq!(
            rows.fixture.blend_preview.as_ref().expect(kind).mode,
            "Multiply"
        );
        rows.frame(vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        for _ in 0..3 {
            rows.frame(vec![egui::Event::PointerMoved(pointer)]);
        }
        assert_eq!(
            rows.fixture.blend_preview.as_ref().expect(kind).mode,
            "Screen"
        );
        assert_eq!(rows.fixture.editor.document(), &before);
        rows.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(rows.fixture.blend_preview.is_none());
        assert_eq!(rows.fixture.editor.document(), &before);
    }
}

#[test]
fn empty_collapsed_fx_opens_creation_menu_and_expands_only_after_adding() {
    for width in [440.0, 1100.0] {
        for passes in [1, 2] {
            for effects in [None, Some(Value::Null), Some(json!([]))] {
                let mut rows = LayerRows::new(width, passes);
                let layer = rows.ids[2].clone();
                let mut entity = rows
                    .fixture
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap()
                    .clone();
                let target = &mut entity["icon"]["layers"][2];
                if let Some(effects) = effects {
                    target["effects"] = effects;
                } else {
                    target.as_object_mut().unwrap().remove("effects");
                }
                rows.fixture
                    .execute(
                        "Recipe",
                        Command::ReplaceEntity {
                            kind: EntityKind::Achievement,
                            id: rows.achievement.clone(),
                            value: entity,
                        },
                        None,
                    )
                    .unwrap();
                rows.fixture.finish_group();
                rows.frame(vec![]);
                rows.frame(vec![]);
                assert!(rows.desktop.layer_palette.expanded_effects.is_empty());
                assert!(rows.ids.iter().all(|id| {
                    rows.ctx
                        .read_response(layer_palette::layer_control_id(
                            &rows.achievement,
                            id,
                            "add",
                        ))
                        .is_none()
                }));
                let before = rows.fixture.editor.document().clone();
                let revision = rows.fixture.editor.revision();
                let rect = rows.row(2);
                let fx = rows
                    .ctx
                    .read_response(layer_palette::layer_control_id(
                        &rows.achievement,
                        &layer,
                        "fx",
                    ))
                    .unwrap()
                    .rect
                    .center();
                rows.press(fx);
                rows.frame(vec![button(fx, false)]);
                let menu = rows.frame(vec![]);
                assert!(menu.shapes.iter().any(|paint| matches!(&paint.shape,
                    egui::Shape::Text(text) if text.galley.text() == "Outline")));
                assert_eq!(
                    rows.row(2),
                    rect,
                    "Opening creation must not expand the row"
                );
                assert!(!rows.desktop.layer_palette.expanded_effects.contains(&layer));
                assert_eq!(rows.fixture.editor.document(), &before);
                rows.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                rows.frame(vec![]);
                assert_eq!(rows.fixture.editor.document(), &before);
                assert_eq!(rows.row(2), rect);

                rows.press(fx);
                rows.frame(vec![button(fx, false)]);
                let menu = rows.frame(vec![]);
                let outline = menu
                    .shapes
                    .iter()
                    .find_map(|paint| match &paint.shape {
                        egui::Shape::Text(text) if text.galley.text() == "Outline" => {
                            Some(text.pos + text.galley.rect.center().to_vec2())
                        }
                        _ => None,
                    })
                    .unwrap();
                rows.press(outline);
                rows.frame(vec![button(outline, false)]);
                rows.frame(vec![]);
                let entity = rows
                    .fixture
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap();
                let effects = entity["icon"]["layers"][2]["effects"].as_array().unwrap();
                assert_eq!(effects.len(), 1);
                assert_eq!(effects[0]["kind"], "Outline");
                assert_eq!(
                    rows.desktop.layer_palette.selected_effect.as_deref(),
                    effects[0]["id"].as_str()
                );
                assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&layer));
                assert!(rows.desktop.layer_palette.expanded_effects.contains(&layer));
                assert!(rows.row(2).height() > rect.height());
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                let with_effect = rows.fixture.editor.document().clone();
                for expanded in [false, true] {
                    rows.press(fx);
                    rows.frame(vec![button(fx, false)]);
                    rows.frame(vec![]);
                    assert_eq!(
                        rows.desktop.layer_palette.expanded_effects.contains(&layer),
                        expanded
                    );
                    assert_eq!(rows.fixture.editor.document(), &with_effect);
                    assert!(!egui::Popup::is_any_open(&rows.ctx));
                }
                rows.fixture.undo();
                assert_eq!(rows.fixture.editor.document(), &before);
            }
        }
    }
}

fn label_positions(output: &egui::FullOutput) -> Vec<egui::Pos2> {
    output
        .shapes
        .iter()
        .filter_map(|paint| match &paint.shape {
            egui::Shape::Text(text) if text.galley.text() == "Layer 2" => Some(text.pos),
            _ => None,
        })
        .collect()
}

#[test]
fn layer_row_padding_and_effect_gutters_select_without_editing() {
    for width in [440.0, 1100.0] {
        for passes in [1, 2] {
            let mut rows = LayerRows::new(width, passes);
            let rect = rows.row(2);
            let before = rows.fixture.editor.document().clone();
            for pos in [
                rect.left_top() + egui::vec2(2.0, 2.0),
                rect.left_center() + egui::vec2(2.0, 0.0),
                rect.right_bottom() - egui::vec2(2.0, 2.0),
                rect.left_bottom() + egui::vec2(20.0, -14.0),
            ] {
                rows.desktop.selected_layer = Some(rows.ids[0].clone());
                rows.desktop.layer_palette.selected_effect = Some("previous-effect".into());
                rows.press(pos);
                rows.frame(vec![button(pos, false)]);
                assert_eq!(
                    rows.desktop.selected_layer.as_ref(),
                    Some(&rows.ids[2]),
                    "Missed row at {pos:?}, width {width}"
                );
                assert!(rows.desktop.layer_palette.selected_effect.is_none());
                assert!(rows.desktop.layer_drag.is_none());
                assert_eq!(rows.fixture.editor.document(), &before);
            }
        }
    }
}

#[test]
fn layer_row_background_drag_has_a_pointer_anchored_ghost_and_one_undo() {
    for width in [440.0, 1100.0] {
        let mut rows = LayerRows::new(width, 2);
        let rect = rows.row(2);
        let start = rect.left_top() + egui::vec2(2.0, 2.0);
        let rest = rows.frame(vec![]);
        let label = label_positions(&rest)
            .into_iter()
            .find(|pos| rect.contains(*pos))
            .unwrap();
        let before = rows.fixture.editor.document().clone();
        let revision = rows.fixture.editor.revision();
        rows.press(start);
        let target = egui::pos2(start.x + 170.0, rows.row(0).bottom() + 6.0);
        let drag = rows.frame(vec![egui::Event::PointerMoved(target)]);
        assert!(rows.desktop.layer_drag.is_some());
        assert_eq!(rows.fixture.editor.document(), &before);
        assert_eq!(rows.fixture.editor.revision(), revision);
        let ghost_label = label + (target - start);
        assert!(
            label_positions(&drag)
                .iter()
                .any(|pos| pos.distance(ghost_label) < 0.1),
            "Missing pointer-anchored ghost"
        );
        assert_eq!(rows.row(2), rect, "The ghost must not move the source row");
        let paused = rows.frame(vec![]);
        assert!(
            label_positions(&paused)
                .iter()
                .any(|pos| pos.distance(ghost_label) < 0.1)
        );
        let reversed = target - egui::vec2(30.0, 12.0);
        let reverse = rows.frame(vec![egui::Event::PointerMoved(reversed)]);
        assert!(
            label_positions(&reverse)
                .iter()
                .any(|pos| pos.distance(label + (reversed - start)) < 0.1)
        );
        let released = rows.frame(vec![
            egui::Event::PointerMoved(target),
            button(target, false),
        ]);
        assert!(rows.desktop.layer_drag.is_none());
        assert!(
            !label_positions(&released)
                .iter()
                .any(|pos| pos.distance(ghost_label) < 0.1)
        );
        assert_eq!(rows.fixture.editor.revision(), revision + 1);
        assert_eq!(
            rows.fixture.editor.undo_label(),
            Some("Reorder icon layers")
        );
        assert_eq!(
            rows.fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &rows.achievement)
                .unwrap()["icon"]["layers"][0]["id"],
            rows.ids[2]
        );
        rows.fixture.undo();
        assert_eq!(rows.fixture.editor.document(), &before);
    }
}

#[test]
fn layer_row_controls_never_start_a_background_reorder() {
    for control in ["fx", "add"] {
        let mut rows = LayerRows::new(1100.0, 2);
        if control == "add" {
            rows.desktop
                .layer_palette
                .expanded_effects
                .insert(rows.ids[2].clone());
            rows.frame(vec![]);
        }
        let pos = rows
            .ctx
            .read_response(layer_palette::layer_control_id(
                &rows.achievement,
                &rows.ids[2],
                control,
            ))
            .unwrap()
            .rect
            .center();
        let before = rows.fixture.editor.document().clone();
        rows.press(pos);
        let target = rows.row(0).center();
        rows.frame(vec![egui::Event::PointerMoved(target)]);
        assert!(
            rows.desktop.layer_drag.is_none(),
            "{control} started a reorder"
        );
        assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&rows.ids[0]));
        rows.frame(vec![button(target, false)]);
        assert_eq!(rows.fixture.editor.document(), &before);
    }
}

#[test]
fn layer_row_drag_cancels_on_escape_disabled_editor_and_focus_loss() {
    for cancel in ["escape", "disabled", "focus"] {
        let mut rows = LayerRows::new(1100.0, 2);
        let start = rows.row(2).left_top() + egui::vec2(2.0, 2.0);
        let target = rows.row(0).center_bottom();
        let before = rows.fixture.editor.document().clone();
        rows.press(start);
        rows.frame(vec![egui::Event::PointerMoved(target)]);
        assert!(rows.desktop.layer_drag.is_some());
        let events = if cancel == "escape" {
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        } else {
            vec![]
        };
        rows.input_frame(events, cancel != "disabled", cancel != "focus");
        assert!(
            rows.desktop.layer_drag.is_none(),
            "{cancel} did not cancel the drag"
        );
        rows.frame(vec![button(target, false)]);
        assert_eq!(rows.fixture.editor.document(), &before);
    }
}

#[test]
fn folder_effects_open_before_owned_layers_and_keep_independent_disclosure() {
    for width in [700.0, 1100.0] {
        for passes in [1, 2] {
            let mut rows = LayerRows::new(width, passes);
            let effects: Vec<_> = (0..3).map(|_| new_id()).collect();
            let mut entity = rows
                .fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &rows.achievement)
                .unwrap()
                .clone();
            entity["icon"] = json!({"layers":[
                {"id":rows.ids[2],"displayName":"Child layer","kind":"Solid",
                    "solidColorHex":"#FFECA44B","parentFolderId":rows.ids[1],
                    "effects":[{"id":effects[2],"kind":"BoxBlur","visible":true,"blur":2.0}]},
                {"id":rows.ids[1],"displayName":"Nested folder","kind":"Folder",
                    "parentFolderId":rows.ids[0],
                    "effects":[{"id":effects[1],"kind":"Shadow","visible":true}]},
                {"id":rows.ids[0],"displayName":"Folder","kind":"Folder",
                    "effects":[{"id":effects[0],"kind":"Outline","visible":true,"width":2.0}]}
            ]});
            rows.fixture
                .execute(
                    "Recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: rows.achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            let before = rows.fixture.editor.document().clone();
            rows.desktop.selected_layer = Some(rows.ids[2].clone());
            rows.desktop.layer_palette.selected_effect = Some(effects[2].clone());
            rows.desktop
                .layer_palette
                .expanded_effects
                .extend(rows.ids[1..].iter().cloned());
            rows.frame(vec![]);
            rows.frame(vec![]);
            let fx = rows
                .ctx
                .read_response(layer_palette::layer_control_id(
                    &rows.achievement,
                    &rows.ids[0],
                    "fx",
                ))
                .unwrap()
                .rect
                .center();
            rows.press(fx);
            rows.frame(vec![button(fx, false)]);
            let output = rows.frame(vec![]);
            assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&rows.ids[0]));
            assert!(rows.desktop.layer_palette.selected_effect.is_none());
            let main_labels = |output: &egui::FullOutput, row: egui::Rect| {
                output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if row.contains(text.pos) && text.pos.y < row.top() + 46.0 =>
                        {
                            Some((text.galley.text().to_owned(), text.pos))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let expanded_controls = main_labels(&output, rows.row(0));
            for id in &rows.ids {
                let [fx, add] = ["fx", "add"].map(|control| {
                    rows.ctx
                        .read_response(layer_palette::layer_control_id(
                            &rows.achievement,
                            id,
                            control,
                        ))
                        .unwrap()
                        .rect
                });
                assert!((add.top() - fx.bottom() - 7.5).abs() < 1.0);
                assert!(
                    (add.center().x - fx.center().x).abs() < 1.0,
                    "Folder indentation must keep add centered beneath FX: {fx:?}, {add:?}"
                );
            }
            let ctx = rows.ctx.clone();
            let achievement_id = rows.achievement.clone();
            let layer_ids = rows.ids.clone();
            let effect_position = |_: &egui::FullOutput, label: &str| {
                let index = match label {
                    "Outline" => 0,
                    "Shadow" => 1,
                    "Box blur" => 2,
                    _ => unreachable!(),
                };
                ctx.read_response(crate::layer_effects::effect_control_id(
                    &achievement_id,
                    &layer_ids[index],
                    &effects[index],
                    "row",
                ))
                .unwrap()
                .rect
                .center()
            };
            let folder_fx = effect_position(&output, "Outline");
            let nested_fx = effect_position(&output, "Shadow");
            let child_fx = effect_position(&output, "Box blur");
            assert!(rows.row(0).contains(folder_fx));
            assert!(folder_fx.y < rows.row(1).top());
            assert!(rows.row(1).contains(nested_fx));
            assert!(nested_fx.y < rows.row(2).top());
            assert!(rows.row(2).contains(child_fx));
            rows.press(folder_fx);
            rows.frame(vec![button(folder_fx, false)]);
            assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&rows.ids[0]));
            assert_eq!(
                rows.desktop.layer_palette.selected_effect.as_ref(),
                Some(&effects[0])
            );

            // Hiding folder FX leaves its children and their FX expanded.
            rows.press(fx);
            rows.frame(vec![button(fx, false)]);
            let output = rows.frame(vec![]);
            assert!(
                !rows
                    .desktop
                    .layer_palette
                    .expanded_effects
                    .contains(&rows.ids[0])
            );
            assert!(rows.row(1).contains(effect_position(&output, "Shadow")));
            assert_eq!(
                main_labels(&output, rows.row(0)),
                expanded_controls,
                "Toggling folder FX must preserve opacity and the other main-row controls"
            );
            assert!(rows.row(2).contains(effect_position(&output, "Box blur")));

            // Hiding the children leaves the folder's own FX accessible.
            rows.desktop
                .layer_palette
                .collapsed
                .insert(rows.ids[0].clone());
            rows.press(fx);
            rows.frame(vec![button(fx, false)]);
            let output = rows.frame(vec![]);
            assert!(rows.row(0).contains(effect_position(&output, "Outline")));
            assert!(!rows.desktop.layer_palette.rows.contains_key(&rows.ids[1]));
            assert!(!rows.desktop.layer_palette.rows.contains_key(&rows.ids[2]));
            assert_eq!(rows.fixture.editor.document(), &before);
        }
    }
}

#[test]
fn effect_drag_changes_drawing_order_once_and_cancels_without_edits() {
    for kind in ["Solid", "Folder"] {
        for passes in [1, 2] {
            for cancel in [None, Some("escape"), Some("focus"), Some("disabled")] {
                let mut rows = LayerRows::new(1100.0, passes);
                let ids: Vec<_> = (0..3).map(|_| new_id()).collect();
                let mut entity = rows
                    .fixture
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap()
                    .clone();
                entity["icon"]["layers"][2]["kind"] = json!(kind);
                entity["icon"]["layers"][2]["effects"] = json!([
                    {"id":ids[0],"kind":"ColorOverlay","visible":true,"colorHex":"#FFFF0000"},
                    {"id":ids[1],"kind":"ColorOverlay","visible":true,"colorHex":"#FF0000FF"},
                    {"id":ids[2],"kind":"Outline","visible":false,"width":4.0}
                ]);
                if kind == "Folder" {
                    entity["icon"]["layers"][1]["parentFolderId"] = json!(rows.ids[2]);
                }
                rows.fixture
                    .execute(
                        "Recipe",
                        Command::ReplaceEntity {
                            kind: EntityKind::Achievement,
                            id: rows.achievement.clone(),
                            value: entity,
                        },
                        None,
                    )
                    .unwrap();
                rows.fixture.finish_group();
                rows.desktop
                    .layer_palette
                    .expanded_effects
                    .insert(rows.ids[2].clone());
                rows.frame(vec![]);
                let output = rows.frame(vec![]);
                let rect = |effect: &str| {
                    rows.ctx
                        .read_response(crate::layer_effects::effect_control_id(
                            &rows.achievement,
                            &rows.ids[2],
                            effect,
                            "row",
                        ))
                        .unwrap()
                        .rect
                };
                let first = rect(&ids[0]);
                let second = rect(&ids[1]);
                assert!(output.shapes.iter().any(|paint| matches!(&paint.shape,
                    egui::Shape::Rect(body) if body.rect.min.distance(first.min) < 1.0 && body.rect.max.distance(first.max) < 1.0 && body.corner_radius == egui::CornerRadius::ZERO && body.stroke == egui::Stroke::NONE)));
                let before = rows.fixture.editor.document().clone();
                let revision = rows.fixture.editor.revision();
                rows.press(first.center());
                let target = second.center_bottom() + egui::vec2(80.0, 2.0);
                rows.frame(vec![egui::Event::PointerMoved(target)]);
                assert!(rows.desktop.layer_palette.effect_drag.is_some());
                assert!(rows.desktop.layer_drag.is_none());
                assert_eq!(rows.fixture.editor.document(), &before);
                assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&rows.ids[2]));
                assert_eq!(
                    rows.desktop.layer_palette.selected_effect.as_ref(),
                    Some(&ids[0])
                );
                rows.frame(vec![]);
                if let Some(cancel) = cancel {
                    let events = if cancel == "escape" {
                        vec![egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }]
                    } else {
                        vec![]
                    };
                    rows.input_frame(events, cancel != "disabled", cancel != "focus");
                    assert!(rows.desktop.layer_palette.effect_drag.is_none());
                }
                rows.frame(vec![button(target, false)]);
                rows.frame(vec![]);
                assert!(rows.desktop.layer_palette.effect_drag.is_none());
                if cancel.is_some() {
                    assert_eq!(rows.fixture.editor.document(), &before);
                    assert_eq!(rows.fixture.editor.revision(), revision);
                    continue;
                }
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                assert_eq!(
                    rows.fixture.editor.undo_label(),
                    Some("Reorder layer effects")
                );
                let mut expected = before
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap()
                    .clone();
                expected["icon"]["layers"][2]["effects"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, 1);
                assert_eq!(
                    rows.fixture
                        .editor
                        .document()
                        .entity(EntityKind::Achievement, &rows.achievement),
                    Some(&expected)
                );
                let blue =
                    render(&before, &rows.achievement, Some((16, 16)), &mut NoAssets).unwrap();
                let red = render(
                    rows.fixture.editor.document(),
                    &rows.achievement,
                    Some((16, 16)),
                    &mut NoAssets,
                )
                .unwrap();
                assert_eq!(blue.achieved.get_pixel(8, 8).0, [0, 0, 255, 255]);
                assert_eq!(red.achieved.get_pixel(8, 8).0, [255, 0, 0, 255]);
                rows.fixture.undo();
                assert_eq!(rows.fixture.editor.document(), &before);
            }
        }
    }
}

#[test]
#[ignore = "Native screenshot of a held layer drag; dedicated external output path"]
fn native_layer_row_drag_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        rows: LayerRows,
        frames: usize,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
            input.focused = true;
            if self.frames == 3 {
                let pos = self.rows.row(2).left_top() + egui::vec2(2.0, 2.0);
                input
                    .events
                    .extend([egui::Event::PointerMoved(pos), button(pos, true)]);
            } else if self.frames == 4 {
                let pos = self.rows.row(0).left_bottom() + egui::vec2(100.0, -8.0);
                input.events.push(egui::Event::PointerMoved(pos));
            }
        }

        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.rows.fixture.preferences());
            self.rows.desktop.layer_editor(ui, &mut self.rows.fixture);
            self.frames += 1;
            if self.frames == 8 {
                assert!(self.rows.desktop.layer_drag.is_some());
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            if let Some(image) = ui.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let bytes: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    &self.output,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            assert!(
                self.frames < 128,
                "Native layer drag screenshot was not received"
            );
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    ))
    .unwrap();
    let boundary = std::path::absolute(crate::test_output::root()).unwrap();
    assert!(output.starts_with(boundary) && output.extension().is_some_and(|ext| ext == "png"));
    eframe::run_native(
        "Achimeld layer drag preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 700.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            cc.egui_ctx
                .options_mut(|options| options.max_passes = std::num::NonZeroUsize::MIN);
            Ok(Box::new(Window {
                rows: LayerRows::new(1100.0, 1),
                frames: 0,
                output,
            }))
        }),
    )
    .unwrap();
}
