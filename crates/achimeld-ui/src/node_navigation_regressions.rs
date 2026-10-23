use super::*;

struct Navigation {
    fixture: Fixture,
    desktop: Desktop,
    ctx: egui::Context,
    ids: Vec<String>,
    time: f64,
    enabled: bool,
    focused: bool,
    extra_pass: bool,
}

impl Navigation {
    fn new(kind: EntityKind, positions: &[[i32; 2]], zoom: f32) -> Self {
        let mut fixture = Fixture::new();
        while fixture.editor.document().entities(kind).len() < positions.len() {
            fixture
                .editor
                .execute("Add", Command::Add(kind), None)
                .unwrap();
        }
        let mut project = fixture.editor.document().project().clone();
        let key = if kind == EntityKind::Achievement {
            "achievements"
        } else {
            "stats"
        };
        let camera = if kind == EntityKind::Achievement {
            "achievementsCamera"
        } else {
            "statsCamera"
        };
        project[key]
            .as_array_mut()
            .unwrap()
            .truncate(positions.len());
        for (index, (entity, position)) in project[key]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(positions)
            .enumerate()
        {
            entity["canvasColumn"] = json!(position[0]);
            entity["canvasRow"] = json!(position[1]);
            entity["apiName"] = json!(format!("NAV_{index}"));
        }
        project[camera] = json!({"zoom":zoom,"offsetX":-30,"offsetY":-30});
        fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let ids = fixture
            .editor
            .document()
            .entities(kind)
            .iter()
            .map(|entity| entity["id"].as_str().unwrap().to_owned())
            .collect();
        let mut desktop = Desktop::default();
        desktop.kind = kind;
        desktop.view = if kind == EntityKind::Achievement {
            desktop::View::Achievements
        } else {
            desktop::View::Stats
        };
        let mut navigation = Self {
            fixture,
            desktop,
            ctx: egui::Context::default(),
            ids,
            time: 0.0,
            enabled: true,
            focused: true,
            extra_pass: false,
        };
        navigation.frame(vec![]);
        navigation.frame(vec![]);
        navigation
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.02;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                time: Some(self.time),
                focused: self.focused,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                if !self.enabled {
                    ui.disable();
                }
                self.desktop.show(ui, &mut self.fixture);
                if self.extra_pass && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("Navigation layout regression");
                }
            },
        );
        output.textures_delta.clear();
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }

    fn arrow(&mut self, key: egui::Key) {
        self.frame(vec![Self::key(key, egui::Modifiers::NONE, false)]);
    }
}

#[test]
fn node_arrows_follow_all_four_spatial_directions_without_editing_either_canvas() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for zoom in [0.5, 1.0] {
            let mut n = Navigation::new(
                kind,
                &[[20, 20], [40, 20], [20, 30], [0, 20], [20, 5]],
                zoom,
            );
            let before = n.fixture.editor.document().clone();
            n.desktop.selection = vec![n.ids[0].clone()];
            n.desktop.selected_layer = Some("old-layer".into());
            for (key, index) in [
                (egui::Key::ArrowRight, 1),
                (egui::Key::ArrowLeft, 0),
                (egui::Key::ArrowDown, 2),
                (egui::Key::ArrowUp, 0),
                (egui::Key::ArrowLeft, 3),
                (egui::Key::ArrowRight, 0),
                (egui::Key::ArrowUp, 4),
            ] {
                n.arrow(key);
                assert_eq!(n.desktop.selection, [n.ids[index].clone()]);
                assert!(n.ctx.memory(|memory| memory.focused()).is_none());
            }
            assert!(n.desktop.selected_layer.is_none());
            assert_eq!(n.fixture.editor.document(), &before);
            assert!(n.fixture.editor.undo_label().is_none());
        }
    }
}

#[test]
fn node_arrows_preserve_event_order_and_key_repeat_and_stop_at_dead_ends() {
    let mut n = Navigation::new(EntityKind::Achievement, &[[0, 0], [20, 0], [40, 0]], 0.5);
    n.desktop.selection = vec![n.ids[0].clone()];
    n.frame(vec![
        Navigation::key(egui::Key::ArrowRight, egui::Modifiers::NONE, false),
        Navigation::key(egui::Key::ArrowRight, egui::Modifiers::NONE, true),
    ]);
    assert_eq!(n.desktop.selection, [n.ids[2].clone()]);
    n.arrow(egui::Key::ArrowRight);
    assert_eq!(n.desktop.selection, [n.ids[2].clone()]);
    n.frame(vec![Navigation::key(
        egui::Key::ArrowLeft,
        egui::Modifiers::NONE,
        true,
    )]);
    assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
    n.frame(vec![
        Navigation::key(egui::Key::ArrowLeft, egui::Modifiers::NONE, false),
        Navigation::key(egui::Key::ArrowRight, egui::Modifiers::NONE, false),
    ]);
    assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
}

#[test]
fn node_arrows_move_once_per_press_across_layout_passes_and_leave_drag_gestures_alone() {
    let mut n = Navigation::new(EntityKind::Achievement, &[[0, 0], [20, 0], [40, 0]], 0.5);
    n.extra_pass = true;
    n.desktop.selection = vec![n.ids[0].clone()];
    n.arrow(egui::Key::ArrowRight);
    assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
    n.desktop.node_drag = Some(desktop::NodeDrag {
        starts: BTreeMap::from([(n.ids[1].clone(), [20, 0])]),
        pointer_start: egui::pos2(300.0, 300.0),
        delta: egui::Vec2::ZERO,
    });
    n.frame(vec![
        egui::Event::PointerButton {
            pos: egui::pos2(300.0, 300.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Navigation::key(egui::Key::ArrowRight, egui::Modifiers::NONE, false),
    ]);
    assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
}

#[test]
fn node_arrows_skip_filtered_neighbours_from_a_visible_anchor() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut n = Navigation::new(kind, &[[0, 0], [20, 0], [40, 0]], 0.5);
        let mut project = n.fixture.editor.document().project().clone();
        let key = if kind == EntityKind::Achievement {
            "achievements"
        } else {
            "stats"
        };
        project[key][1]["apiName"] = json!("HIDDEN");
        n.fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                n.fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        n.desktop.filter = "nav_".into();
        n.desktop.selection = vec![n.ids[0].clone()];
        n.arrow(egui::Key::ArrowRight);
        assert_eq!(n.desktop.selection, [n.ids[2].clone()]);
    }
}

#[test]
fn node_arrows_respect_filter_automatic_positions_and_empty_selection() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut n = Navigation::new(kind, &[[0, 0], [20, 0], [40, 0]], 0.5);
        n.desktop.selection = vec![n.ids[0].clone()];
        n.desktop.filter = "NAV_2".into();
        n.arrow(egui::Key::ArrowRight);
        assert_eq!(n.desktop.selection, [n.ids[2].clone()]);
        n.desktop.filter = "no matching nodes".into();
        n.arrow(egui::Key::ArrowLeft);
        assert_eq!(n.desktop.selection, [n.ids[2].clone()]);
        n.desktop.filter.clear();
        let mut project = n.fixture.editor.document().project().clone();
        let key = if kind == EntityKind::Achievement {
            "achievements"
        } else {
            "stats"
        };
        for entity in project[key].as_array_mut().unwrap() {
            entity.as_object_mut().unwrap().remove("canvasColumn");
            entity.as_object_mut().unwrap().remove("canvasRow");
        }
        n.fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                n.fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        n.desktop.selection = vec![n.ids[0].clone()];
        n.arrow(egui::Key::ArrowRight);
        assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
        n.desktop.selection.clear();
        n.arrow(egui::Key::ArrowDown);
        assert_eq!(n.desktop.selection.len(), 1);
        assert!(n.ids.contains(&n.desktop.selection[0]));
    }
}

#[test]
fn node_arrows_reveal_offscreen_targets_and_keep_zoom() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for zoom in [0.5, 2.0] {
            let mut n = Navigation::new(kind, &[[0, 0], [200, 0], [200, 150]], zoom);
            n.desktop.selection = vec![n.ids[0].clone()];
            for (key, index) in [
                (egui::Key::ArrowRight, 1),
                (egui::Key::ArrowDown, 2),
                (egui::Key::ArrowUp, 1),
                (egui::Key::ArrowLeft, 0),
            ] {
                n.arrow(key);
                assert_eq!(n.desktop.selection, [n.ids[index].clone()]);
                let response = n
                    .ctx
                    .read_response(egui::Id::new(("node", &n.ids[index])))
                    .expect("target drawn immediately");
                assert!(response.interact_rect.contains_rect(response.rect));
                assert_eq!(n.desktop.zoom, zoom);
            }
            assert_eq!(n.fixture.editor.undo_label(), Some("Canvas camera"));
        }
    }
}

#[test]
fn node_arrows_leave_controls_dialogs_modified_keys_and_disabled_views_alone() {
    for blocked in [
        "disabled",
        "unfocused",
        "text",
        "preferences",
        "about",
        "settings",
        "sort",
        "error",
        "popup",
        "modal",
        "layer",
        "ctrl",
        "shift",
        "alt",
        "art",
        "style",
    ] {
        let mut n = Navigation::new(EntityKind::Achievement, &[[0, 0], [20, 0]], 0.5);
        n.desktop.selection = vec![n.ids[0].clone()];
        let mut modifiers = egui::Modifiers::NONE;
        match blocked {
            "disabled" => n.enabled = false,
            "unfocused" => n.focused = false,
            "text" => {
                n.ctx
                    .memory_mut(|memory| memory.request_focus(egui::Id::new("inspector-control")));
            }
            "preferences" => n.desktop.preferences = Some(n.fixture.preferences().clone()),
            "about" => n.desktop.about = true,
            "settings" => n.desktop.settings = true,
            "sort" => n.desktop.sort_open = true,
            "error" => n.desktop.error = Some("Test".into()),
            "popup" => egui::Popup::open_id(&n.ctx, egui::Id::new("test-popup")),
            "modal" => {
                n.ctx.memory_mut(|memory| {
                    memory.set_modal_layer(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("test-modal"),
                    ))
                });
                n.frame(vec![]);
            }
            "layer" => n.desktop.layer_palette.keyboard_active = true,
            "ctrl" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            "art" => n.desktop.view = desktop::View::Art,
            "style" => n.desktop.view = desktop::View::Style,
            _ => unreachable!(),
        }
        let before = n.fixture.editor.document().clone();
        n.frame(vec![
            egui::Event::ModifiersChanged(modifiers),
            Navigation::key(egui::Key::ArrowRight, modifiers, false),
        ]);
        assert_eq!(n.desktop.selection, [n.ids[0].clone()], "{blocked}");
        assert_eq!(n.fixture.editor.document(), &before, "{blocked}");
    }
}

#[test]
fn node_arrows_work_after_clicking_card_body_or_artwork() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for artwork in [false, true] {
            let mut n = Navigation::new(kind, &[[0, 0], [20, 0]], 0.5);
            let card = n
                .ctx
                .read_response(egui::Id::new(("node", &n.ids[0])))
                .unwrap()
                .rect;
            let pos = if artwork {
                card.left_center() + egui::vec2(20.0, 0.0)
            } else {
                card.right_center() - egui::vec2(15.0, 0.0)
            };
            for pressed in [true, false] {
                n.frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            assert_eq!(n.desktop.selection, [n.ids[0].clone()]);
            n.arrow(egui::Key::ArrowRight);
            assert_eq!(n.desktop.selection, [n.ids[1].clone()]);
        }
    }
}
