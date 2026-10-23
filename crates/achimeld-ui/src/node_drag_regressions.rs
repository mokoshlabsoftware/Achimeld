use super::*;

struct CanvasGesture {
    fixture: Fixture,
    desktop: Desktop,
    ctx: egui::Context,
    ids: Vec<String>,
    modifiers: egui::Modifiers,
    time: f64,
    disabled: bool,
}

impl CanvasGesture {
    fn new(kind: EntityKind, zoom: f32) -> Self {
        let mut fixture = Fixture::new();
        while fixture.editor.document().entities(kind).len() < 3 {
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
        project[camera] = json!({"zoom":zoom,"offsetX":-30,"offsetY":-30});
        for (entity, [column, row]) in
            project[key]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip([[3, 4], [22, 4], [42, 20]])
        {
            entity["canvasColumn"] = json!(column);
            entity["canvasRow"] = json!(row);
        }
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
        let mut gesture = Self {
            fixture,
            desktop,
            ctx: egui::Context::default(),
            ids,
            modifiers: egui::Modifiers::NONE,
            time: 0.0,
            disabled: false,
        };
        gesture.frame(vec![]);
        gesture.frame(vec![]);
        gesture
    }

    fn frame(&mut self, mut events: Vec<egui::Event>) -> BTreeMap<String, egui::Rect> {
        events.insert(0, egui::Event::ModifiersChanged(self.modifiers));
        self.time += 0.02;
        let preferences = self.fixture.preferences().clone();
        let mut bounds = BTreeMap::new();
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                time: Some(self.time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(2000.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                if self.disabled {
                    ui.disable();
                }
                self.desktop
                    .canvas(ui, &mut self.fixture, &preferences, None);
                for id in &self.ids {
                    if let Some(response) = ui.ctx().read_response(egui::Id::new(("node", id))) {
                        bounds.insert(id.clone(), response.rect);
                    }
                }
            },
        );
        output.textures_delta.clear();
        bounds
    }

    fn button(&mut self, pos: egui::Pos2, pressed: bool) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: self.modifiers,
            },
        ]);
    }

    fn escape(&mut self) {
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}

#[test]
fn rectangle_selection_intersects_cards_in_both_directions_and_preserves_ctrl_selection() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for zoom in [0.5, 2.0] {
            for reverse in [false, true] {
                let mut g = CanvasGesture::new(kind, zoom);
                let before = g.fixture.editor.document().clone();
                let bounds = g.frame(vec![]);
                // Cross small portions of both cards: containment is not required.
                let start = bounds[&g.ids[0]].left_top() - egui::vec2(8.0, 8.0);
                let end = bounds[&g.ids[1]].left_top() + egui::vec2(8.0, 8.0);
                let (start, end) = if reverse {
                    (egui::pos2(end.x, start.y), egui::pos2(start.x, end.y))
                } else {
                    (start, end)
                };
                g.desktop.selection = vec![g.ids[2].clone()];
                g.button(start, true);
                g.frame(vec![egui::Event::PointerMoved(end)]);
                assert_eq!(g.desktop.selection, g.ids[..2]);
                assert!(g.desktop.automation_busy());
                // Reversing the rectangle removes cards that no longer intersect it.
                g.frame(vec![egui::Event::PointerMoved(start)]);
                assert!(g.desktop.selection.is_empty());
                g.button(end, false);
                assert_eq!(g.desktop.selection, g.ids[..2]);
                assert!(g.desktop.selection_drag.is_none());
                assert_eq!(g.fixture.editor.document(), &before);
                assert_eq!(g.fixture.editor.revision(), 0);

                g.desktop.selection = vec![g.ids[2].clone()];
                g.modifiers.ctrl = true;
                g.button(start, true);
                g.button(end, false);
                assert_eq!(g.desktop.selection.len(), 3);
                assert!(g.ids.iter().all(|id| g.desktop.selection.contains(id)));
                assert_eq!(g.fixture.editor.document(), &before);
                assert_eq!(g.fixture.editor.revision(), 0);
            }
        }
    }
}

#[test]
fn selected_nodes_preview_and_move_together_in_one_undo_step() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for zoom in [0.5, 2.0] {
            for over_art in [false, true] {
                let mut g = CanvasGesture::new(kind, zoom);
                let before = g.fixture.editor.document().clone();
                let original = g.frame(vec![]);
                g.desktop.selection = g.ids[..2].to_vec();
                let card = original[&g.ids[1]];
                let press = if over_art {
                    card.min + egui::vec2(40.0, 40.0) * zoom
                } else {
                    card.max - egui::vec2(20.0, 20.0) * zoom
                };
                g.frame(vec![egui::Event::PointerMoved(press)]);
                g.button(press, true);
                for offset in [
                    egui::vec2(40.0, 30.0),
                    egui::vec2(100.0, 80.0),
                    egui::vec2(60.0, 40.0),
                ] {
                    let delta = offset * zoom;
                    g.frame(vec![egui::Event::PointerMoved(press + delta)]);
                    for _ in 0..2 {
                        let bounds = g.frame(vec![]);
                        for id in &g.ids[..2] {
                            assert!(bounds[id].min.distance(original[id].min + delta) < 0.01);
                        }
                        assert_eq!(bounds[&g.ids[2]], original[&g.ids[2]]);
                        assert_eq!(g.desktop.selection, g.ids[..2]);
                        assert_eq!(g.fixture.editor.document(), &before);
                    }
                }
                // Include the final displacement on release, beyond the canvas.
                let delta = egui::vec2(-1000.0, -800.0);
                g.button(press + delta * zoom, false);
                assert!(g.desktop.node_drag.is_none());
                assert_eq!(g.fixture.editor.revision(), 1);
                assert_eq!(g.fixture.editor.undo_label(), Some("Move nodes"));
                for id in &g.ids[..2] {
                    let moved = g.fixture.editor.document().entity(kind, id).unwrap();
                    let initial = before.entity(kind, id).unwrap();
                    assert_eq!(
                        moved["canvasColumn"].as_i64().unwrap(),
                        initial["canvasColumn"].as_i64().unwrap() - 50
                    );
                    assert_eq!(
                        moved["canvasRow"].as_i64().unwrap(),
                        initial["canvasRow"].as_i64().unwrap() - 40
                    );
                }
                assert_eq!(
                    g.fixture.editor.document().entity(kind, &g.ids[2]),
                    before.entity(kind, &g.ids[2])
                );
                let after = g.fixture.editor.document().clone();
                g.fixture.editor.undo();
                assert_eq!(g.fixture.editor.document(), &before);
                g.fixture.editor.redo();
                assert_eq!(g.fixture.editor.document(), &after);
            }
        }
    }
}

#[test]
fn rectangle_and_group_drags_cancel_without_document_changes() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut g = CanvasGesture::new(kind, 1.0);
        let before = g.fixture.editor.document().clone();
        let bounds = g.frame(vec![]);
        let start = bounds[&g.ids[0]].min - egui::vec2(8.0, 8.0);
        let end = bounds[&g.ids[1]].center();
        g.desktop.selection = vec![g.ids[2].clone()];
        g.button(start, true);
        g.frame(vec![egui::Event::PointerMoved(end)]);
        g.escape();
        assert!(g.desktop.selection_drag.is_none());
        assert_eq!(g.desktop.selection, vec![g.ids[2].clone()]);
        g.button(end, false);
        g.desktop.selection = g.ids[..2].to_vec();
        let press = bounds[&g.ids[1]].center();
        g.button(press, true);
        g.frame(vec![egui::Event::PointerMoved(
            press + egui::vec2(100.0, 80.0),
        )]);
        g.escape();
        g.button(press, false);
        assert!(g.desktop.node_drag.is_none());
        assert_eq!(g.fixture.editor.document(), &before);
        assert_eq!(g.fixture.editor.revision(), 0);
        g.disabled = true;
        g.button(start, true);
        g.button(end, false);
        assert!(g.desktop.selection_drag.is_none());
        assert_eq!(g.desktop.selection, g.ids[..2]);
    }
}

#[test]
fn ctrl_clicks_include_achievement_icons_and_dragging_an_unselected_node_replaces_the_group() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut g = CanvasGesture::new(kind, 1.0);
        let original = g.fixture.editor.document().clone();
        let bounds = g.frame(vec![]);
        let first = bounds[&g.ids[0]].min + egui::vec2(40.0, 40.0);
        let second = bounds[&g.ids[1]].min + egui::vec2(40.0, 40.0);
        g.button(first, true);
        g.button(first, false);
        assert_eq!(g.desktop.selection, g.ids[..1]);
        g.modifiers.ctrl = true;
        g.button(second, true);
        g.button(second, false);
        assert_eq!(g.desktop.selection, g.ids[..2]);
        g.button(first, true);
        g.button(first, false);
        assert_eq!(g.desktop.selection, vec![g.ids[1].clone()]);
        assert_eq!(g.fixture.editor.document(), &original);
        g.modifiers.ctrl = false;
        g.desktop.selection = g.ids[..2].to_vec();
        let third = bounds[&g.ids[2]].center();
        g.button(third, true);
        g.frame(vec![egui::Event::PointerMoved(
            third + egui::vec2(100.0, 80.0),
        )]);
        g.button(third + egui::vec2(100.0, 80.0), false);
        assert_eq!(g.desktop.selection, vec![g.ids[2].clone()]);
        for id in &g.ids[..2] {
            assert_eq!(
                g.fixture.editor.document().entity(kind, id),
                original.entity(kind, id)
            );
        }
        assert_eq!(g.fixture.editor.undo_label(), Some("Move node"));
    }
}

#[test]
fn rectangle_selection_respects_filter_and_zero_displacement_does_not_add_history() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut g = CanvasGesture::new(kind, 1.0);
        let original = g.fixture.editor.document().clone();
        let bounds = g.frame(vec![]);
        g.desktop.filter = original.entity(kind, &g.ids[1]).unwrap()["apiName"]
            .as_str()
            .unwrap()
            .to_owned();
        g.frame(vec![]);
        let start = bounds[&g.ids[0]].min - egui::vec2(8.0, 8.0);
        let end = bounds[&g.ids[1]].max + egui::vec2(8.0, 8.0);
        g.button(start, true);
        g.button(end, false);
        assert_eq!(g.desktop.selection, vec![g.ids[1].clone()]);
        g.desktop.filter.clear();
        g.desktop.selection = g.ids[..2].to_vec();
        g.frame(vec![]);
        let press = bounds[&g.ids[1]].center();
        g.button(press, true);
        g.frame(vec![egui::Event::PointerMoved(
            press + egui::vec2(100.0, 80.0),
        )]);
        g.button(press, false);
        assert_eq!(g.desktop.selection, g.ids[..2]);
        assert_eq!(g.fixture.editor.document(), &original);
        assert_eq!(g.fixture.editor.revision(), 0);
    }
}

#[test]
#[ignore = "Native rectangle-selection preview; dedicated external screenshot path"]
fn native_group_selection_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        gesture: CanvasGesture,
        output: std::path::PathBuf,
        frames: usize,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
            // This fixture drives synthetic gestures even if another local
            // window has focus while the screenshot process starts.
            input.focused = true;
            input.events.retain(|event| {
                !matches!(
                    event,
                    egui::Event::PointerMoved(_)
                        | egui::Event::PointerButton { .. }
                        | egui::Event::PointerGone
                        | egui::Event::MouseWheel { .. }
                        | egui::Event::ModifiersChanged(_)
                )
            });
            input
                .events
                .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
            let bounds = |index| {
                ctx.read_response(egui::Id::new(("node", &self.gesture.ids[index])))
                    .map(|response| response.rect)
            };
            if let (Some(first), Some(second)) = (bounds(0), bounds(1)) {
                let start = first.min - egui::vec2(8.0, 8.0);
                let end = second.max + egui::vec2(8.0, 8.0);
                match self.frames {
                    8 => input.events.push(egui::Event::PointerMoved(start)),
                    9 => input.events.push(egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    }),
                    10.. => input.events.push(egui::Event::PointerMoved(end)),
                    _ => {}
                }
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.gesture.desktop.show(ui, &mut self.gesture.fixture);
            self.frames += 1;
            if self.frames == 20 {
                assert_eq!(self.gesture.desktop.selection, self.gesture.ids[..2]);
                assert!(self.gesture.desktop.selection_drag.is_some());
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
            assert!(self.frames < 1000, "Selection screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    let kind = if std::env::var_os("ACHIMELD_SCREENSHOT_STATS").is_some() {
        EntityKind::Stat
    } else {
        EntityKind::Achievement
    };
    let output = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    );
    eframe::run_native(
        "Achimeld selection preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 900.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            Ok(Box::new(Window {
                gesture: CanvasGesture::new(kind, 1.0),
                output,
                frames: 0,
            }))
        }),
    )
    .unwrap();
}

#[test]
fn node_drag_tracks_the_pointer_through_pause_reverse_and_drop() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for zoom in [0.5, 2.0] {
            for over_art in [false, true] {
                for outside in [false, true] {
                    check_node_drag(kind, zoom, over_art, outside);
                }
            }
        }
    }
}

fn check_node_drag(kind: EntityKind, zoom: f32, over_art: bool, outside: bool) {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    let camera = if kind == EntityKind::Achievement {
        "achievementsCamera"
    } else {
        "statsCamera"
    };
    project[camera] = json!({"zoom": zoom, "offsetX": -30, "offsetY": -30});
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    let entity = &fixture.editor.document().entities(kind)[0];
    let id = entity["id"].as_str().unwrap().to_owned();
    let start = [entity["canvasColumn"].clone(), entity["canvasRow"].clone()];
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    let mut desktop = Desktop::default();
    desktop.kind = kind;
    desktop.view = if kind == EntityKind::Achievement {
        desktop::View::Achievements
    } else {
        desktop::View::Stats
    };
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        time += 0.02;
        let preferences = fixture.preferences().clone();
        let mut rendered = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                desktop.canvas(ui, fixture, &preferences, None);
                rendered = ui
                    .ctx()
                    .read_response(egui::Id::new(("node", &id)))
                    .map(|r| r.rect);
            },
        );
        output.textures_delta.clear();
        rendered
    };
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut desktop, &mut fixture, vec![]);
    let original = frame(&mut desktop, &mut fixture, vec![]).unwrap();
    let press = if over_art {
        original.min + egui::vec2(40.0, 40.0) * zoom
    } else {
        original.max - egui::vec2(20.0, 20.0) * zoom
    };
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(press)],
    );
    frame(&mut desktop, &mut fixture, vec![button(press, true)]);
    for offset in [
        egui::vec2(40.0, 30.0),
        egui::vec2(100.0, 80.0),
        egui::vec2(80.0, 60.0),
    ] {
        let delta = offset * zoom;
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerMoved(press + delta)],
        );
        // Idle frames must preserve both the preview and the uncommitted model.
        for _ in 0..2 {
            let rendered = frame(&mut desktop, &mut fixture, vec![]).unwrap();
            let drag = desktop.node_drag.as_ref().expect("Active node drag");
            assert!(drag.starts.contains_key(&id));
            assert!(
                rendered.min.distance(original.min + delta) < 0.01,
                "Node drifted: {kind:?}, zoom={zoom}, art={over_art}, actual={:?}, expected={:?}, delta={:?}",
                rendered.min,
                original.min + delta,
                drag.delta
            );
            assert_eq!(fixture.editor.document(), &before);
            assert_eq!(fixture.editor.revision(), revision);
        }
    }
    let drop_offset = if outside {
        egui::vec2(-1000.0, -800.0)
    } else {
        egui::vec2(100.0, 80.0)
    };
    let drop = press + drop_offset * zoom;
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(drop), button(drop, false)],
    );
    frame(&mut desktop, &mut fixture, vec![]);
    assert!(desktop.node_drag.is_none());
    assert_eq!(fixture.editor.revision(), revision + 1);
    assert_eq!(fixture.editor.undo_label(), Some("Move node"));
    let moved = fixture.editor.document().entity(kind, &id).unwrap();
    for (field, initial, delta) in [
        ("canvasColumn", &start[0], drop_offset.x),
        ("canvasRow", &start[1], drop_offset.y),
    ] {
        assert_eq!(
            moved[field].as_i64().unwrap(),
            initial.as_i64().unwrap() + (delta / 20.0).round() as i64
        );
    }
    let after = fixture.editor.document().clone();
    fixture.editor.undo();
    assert_eq!(fixture.editor.document(), &before);
    fixture.editor.redo();
    assert_eq!(fixture.editor.document(), &after);
}
