use super::*;

#[test]
fn second_right_click_dismisses_canvas_menu_without_moving_it() {
    fn collect_menu_positions(shape: &egui::Shape, label: &str, positions: &mut Vec<egui::Pos2>) {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => positions.push(text.pos),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_menu_positions(shape, label, positions);
                }
            }
            _ => {}
        }
    }
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for extra_pass in [false, true] {
            let mut fixture = Fixture::new();
            let before = fixture.editor.document().clone();
            let mut desktop = Desktop::default();
            desktop.kind = kind;
            desktop.view = if kind == EntityKind::Achievement {
                desktop::View::Achievements
            } else {
                desktop::View::Stats
            };
            let ctx = egui::Context::default();
            let mut time = 0.0;
            let mut frame = |desktop: &mut Desktop, events| {
                time += 0.02;
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
                        desktop.show(ui, &mut fixture);
                        if extra_pass && ui.ctx().current_pass_index() == 0 {
                            ui.ctx().request_discard("Canvas menu dismissal regression");
                        }
                    },
                );
                output.textures_delta.clear();
                let mut positions = Vec::new();
                let label = if kind == EntityKind::Achievement {
                    "Create achievement"
                } else {
                    "Create stat"
                };
                for shape in &output.shapes {
                    collect_menu_positions(&shape.shape, label, &mut positions);
                }
                positions
            };
            let pointer = |pos, pressed| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            frame(&mut desktop, vec![]);
            let first = egui::pos2(300.0, 600.0);
            frame(&mut desktop, pointer(first, true));
            frame(&mut desktop, pointer(first, false));
            let original = frame(&mut desktop, vec![]);
            assert!(!original.is_empty());
            let second = egui::pos2(650.0, 800.0);
            for pressed in [true, false] {
                let positions = frame(&mut desktop, pointer(second, pressed));
                assert!(
                    positions.iter().all(|position| original.contains(position)),
                    "Menu moved during dismissal: {kind:?}, extra_pass={extra_pass}, {positions:?}"
                );
            }
            assert!(desktop.canvas_menu.is_none());
            assert!(!egui::Popup::is_any_open(&ctx));
            frame(&mut desktop, vec![]);
            frame(&mut desktop, pointer(second, true));
            frame(&mut desktop, pointer(second, false));
            assert!(desktop.canvas_menu.is_some());
            assert!(egui::Popup::is_any_open(&ctx));
            assert_eq!(
                frame(&mut desktop, vec![])[0],
                original[0] + (second - first)
            );
            assert_eq!(fixture.editor.document(), &before);
        }
    }
}

#[test]
fn right_click_opens_when_press_and_release_arrive_in_one_startup_frame() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for warmup in [0, 2] {
            for extra_pass in [false, true] {
                let mut fixture = Fixture::new();
                let before = fixture.editor.document().clone();
                let mut desktop = Desktop::default();
                desktop.kind = kind;
                desktop.view = if kind == EntityKind::Achievement {
                    desktop::View::Achievements
                } else {
                    desktop::View::Stats
                };
                let ctx = egui::Context::default();
                let frame = |desktop: &mut Desktop, fixture: &mut Fixture, time, events| {
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
                            desktop.show(ui, fixture);
                            if extra_pass && ui.ctx().current_pass_index() == 0 {
                                ui.ctx().request_discard("Batched camera input regression");
                            }
                        },
                    );
                    output.textures_delta.clear();
                };
                for tick in 0..warmup {
                    frame(&mut desktop, &mut fixture, f64::from(tick) * 0.02, vec![]);
                }
                let pointer = egui::pos2(300.0, 600.0);
                frame(
                    &mut desktop,
                    &mut fixture,
                    1.0,
                    [true, false]
                        .map(|pressed| egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Secondary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        })
                        .to_vec(),
                );
                assert!(
                    desktop.canvas_menu.is_some() && egui::Popup::is_any_open(&ctx),
                    "Lost batched right click: {kind:?}, warmup={warmup}, extra_pass={extra_pass}"
                );
                frame(&mut desktop, &mut fixture, 1.02, vec![]);
                assert!(desktop.canvas_menu.is_some());
                assert_eq!(fixture.editor.document(), &before);
                assert_eq!(desktop.pan, egui::vec2(30.0, 30.0));
            }
        }
    }
}

#[test]
fn startup_right_click_survives_pointer_movement_after_release() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for after_click in [egui::pos2(600.0, 700.0), egui::pos2(1500.0, 700.0)] {
            for (extra_pass, batched) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let mut fixture = Fixture::new();
                fixture.editor = Editor::default();
                let before = fixture.editor.document().clone();
                let mut desktop = Desktop::default();
                desktop.kind = kind;
                desktop.view = if kind == EntityKind::Achievement {
                    desktop::View::Achievements
                } else {
                    desktop::View::Stats
                };
                let ctx = egui::Context::default();
                let pointer = egui::pos2(300.0, 600.0);
                let button = |pressed| egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                let mut frame = |time, events| {
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
                            desktop.show(ui, &mut fixture);
                            if extra_pass && ui.ctx().current_pass_index() == 0 {
                                ui.ctx()
                                    .request_discard("Startup click followed by hover movement");
                            }
                        },
                    );
                    output.textures_delta.clear();
                };
                let mut events = Vec::new();
                if batched {
                    events.push(button(true));
                } else {
                    frame(1.0, vec![button(true)]);
                }
                events.extend([button(false), egui::Event::PointerMoved(after_click)]);
                frame(1.02, events);
                frame(1.04, vec![]);
                assert!(
                    desktop.canvas_menu.is_some() && egui::Popup::is_any_open(&ctx),
                    "Lost completed startup click: {kind:?}, {after_click:?}, extra_pass={extra_pass}, batched={batched}"
                );
                assert_eq!(desktop.pan, egui::vec2(30.0, 30.0));
                assert_eq!(fixture.editor.document(), &before);
            }
        }
    }
}

#[test]
fn zoom_buttons_keep_camera_center_after_panning_and_at_zoom_limits() {
    use achimeld_application::canvas::{MAX_ZOOM, MIN_ZOOM};

    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for (initial_zoom, zoom_in) in [
            (0.81, true),
            (0.81, false),
            (MAX_ZOOM / 1.1, true),
            (MIN_ZOOM * 1.1, false),
            (MAX_ZOOM, true),
            (MIN_ZOOM, false),
        ] {
            let mut fixture = Fixture::new();
            fixture
                .editor
                .execute(
                    "Initial camera",
                    Command::CanvasCamera {
                        kind,
                        zoom: f64::from(initial_zoom),
                        offset_x: 275.0,
                        offset_y: -143.0,
                    },
                    None,
                )
                .unwrap();
            let mut desktop = Desktop::default();
            desktop.kind = kind;
            desktop.view = if kind == EntityKind::Achievement {
                desktop::View::Achievements
            } else {
                desktop::View::Stats
            };
            let ctx = egui::Context::default();
            let frame = |desktop: &mut Desktop,
                         fixture: &mut Fixture,
                         action: Option<canvas::CanvasAction>| {
                let preferences = fixture.preferences().clone();
                let mut rect = egui::Rect::NOTHING;
                let mut action = action;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1600.0, 1000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                egui::pos2(173.0, 91.0),
                                egui::vec2(1000.0, 700.0),
                            )),
                            |ui| {
                                rect = ui.available_rect_before_wrap();
                                desktop.canvas(ui, fixture, &preferences, action.take());
                            },
                        );
                    },
                );
                output.textures_delta.clear();
                rect
            };
            let rect = frame(&mut desktop, &mut fixture, None);
            let before = fixture.editor.document().clone();
            let world = (rect.center() - rect.min - desktop.pan) / desktop.zoom;
            frame(
                &mut desktop,
                &mut fixture,
                Some(if zoom_in {
                    canvas::CanvasAction::ZoomIn
                } else {
                    canvas::CanvasAction::ZoomOut
                }),
            );
            let expected = if zoom_in {
                initial_zoom * 1.2
            } else {
                initial_zoom / 1.2
            }
            .clamp(MIN_ZOOM, MAX_ZOOM);
            assert_eq!(desktop.zoom, expected);
            let anchored = rect.min + desktop.pan + world * desktop.zoom;
            assert!(anchored.distance(rect.center()) < 0.001);
            if expected == initial_zoom {
                assert_eq!(fixture.editor.document(), &before);
            } else {
                let after = fixture.editor.document().clone();
                assert_eq!(fixture.editor.undo_label(), Some("Canvas camera"));
                fixture.undo();
                assert_eq!(fixture.editor.document(), &before);
                fixture.redo();
                assert_eq!(fixture.editor.document(), &after);
            }
        }
    }
}

#[test]
fn wheel_zoom_keeps_pointer_after_panning_through_smoothing_and_zoom_limits() {
    use achimeld_application::canvas::{MAX_ZOOM, MIN_ZOOM};

    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for (initial_zoom, scroll) in [
            (0.81, 40.0),
            (0.81, -40.0),
            (MAX_ZOOM / 1.001, 40.0),
            (MIN_ZOOM * 1.001, -40.0),
            (MAX_ZOOM, 40.0),
            (MIN_ZOOM, -40.0),
        ] {
            let mut fixture = Fixture::new();
            fixture
                .editor
                .execute(
                    "Initial camera",
                    Command::CanvasCamera {
                        kind,
                        zoom: f64::from(initial_zoom),
                        offset_x: 275.0,
                        offset_y: -143.0,
                    },
                    None,
                )
                .unwrap();
            let mut desktop = Desktop::default();
            desktop.kind = kind;
            desktop.view = if kind == EntityKind::Achievement {
                desktop::View::Achievements
            } else {
                desktop::View::Stats
            };
            let ctx = egui::Context::default();
            let mut tick = 0;
            let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
                tick += 1;
                let preferences = fixture.preferences().clone();
                let mut rect = egui::Rect::NOTHING;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(f64::from(tick) * 0.02),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1600.0, 1000.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                egui::pos2(173.0, 91.0),
                                egui::vec2(1000.0, 700.0),
                            )),
                            |ui| {
                                rect = ui.available_rect_before_wrap();
                                desktop.canvas(ui, fixture, &preferences, None);
                            },
                        );
                    },
                );
                output.textures_delta.clear();
                rect
            };
            let rect = frame(&mut desktop, &mut fixture, vec![]);
            let pointer = rect.min + egui::vec2(810.0, 510.0);
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(pointer)],
            );
            let before = fixture.editor.document().clone();
            let pan = desktop.pan;
            let world = (pointer - rect.min - pan) / initial_zoom;
            let mut events = vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, scroll),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }];
            for _ in 0..40 {
                frame(&mut desktop, &mut fixture, std::mem::take(&mut events));
                let anchored = rect.min + desktop.pan + world * desktop.zoom;
                assert!(
                    anchored.distance(pointer) < 0.001,
                    "Wheel anchor drifted: {kind:?}, initial={initial_zoom}, scroll={scroll}"
                );
            }
            if initial_zoom == MAX_ZOOM || initial_zoom == MIN_ZOOM {
                assert_eq!(desktop.zoom, initial_zoom);
                assert_eq!(desktop.pan, pan);
                assert_eq!(fixture.editor.document(), &before);
            } else {
                assert!((desktop.zoom - initial_zoom) * scroll > 0.0);
                if initial_zoom > MAX_ZOOM / 1.01 {
                    assert_eq!(desktop.zoom, MAX_ZOOM);
                } else if initial_zoom < MIN_ZOOM * 1.01 {
                    assert_eq!(desktop.zoom, MIN_ZOOM);
                }
                let after = fixture.editor.document().clone();
                assert_eq!(fixture.editor.undo_label(), Some("Canvas camera"));
                fixture.undo();
                assert_eq!(fixture.editor.document(), &before);
                fixture.redo();
                assert_eq!(fixture.editor.document(), &after);
            }
        }
    }
}

#[test]
fn camera_pan_tracks_drags_over_background_cards_and_icons_in_icon_mode() {
    for icon_mode in [false, true] {
        for button in [egui::PointerButton::Middle, egui::PointerButton::Secondary] {
            for target in ["background", "card", "icon"] {
                for max_passes in [1, 2] {
                    check_camera_pan(icon_mode, button, target, max_passes);
                }
            }
        }
    }
}

fn check_camera_pan(icon_mode: bool, button: egui::PointerButton, target: &str, max_passes: usize) {
    let mut fixture = Fixture::new();
    fixture.layout.position = Some([1000.0, 300.0]);
    fixture.layout.size = [450.0, 500.0];
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    if icon_mode {
        desktop.open_layers();
    }
    let ctx = egui::Context::default();
    ctx.options_mut(|options| options.max_passes = max_passes.try_into().unwrap());
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        time += 0.02;
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
                desktop.show(ui, fixture);
                if max_passes == 2 && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("Camera drag layout regression");
                }
            },
        );
        output.textures_delta.clear();
    };
    frame(&mut desktop, &mut fixture, vec![]);
    frame(&mut desktop, &mut fixture, vec![]);
    let press = match target {
        "card" => {
            ctx.read_response(egui::Id::new(("node", &id)))
                .unwrap()
                .rect
                .right_center()
                - egui::vec2(25.0, 0.0)
        }
        "icon" => ctx
            .read_response(egui::Id::new(("icon-layers", &id)))
            .unwrap()
            .rect
            .center(),
        _ => egui::pos2(100.0, 750.0),
    };
    let pointer_button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::PointerMoved(press),
            pointer_button(press, true),
        ],
    );
    let initial = desktop.pan;
    for delta in [
        egui::vec2(40.0, 30.0),
        egui::vec2(100.0, 80.0),
        egui::vec2(70.0, 50.0),
    ] {
        let destination = initial + delta;
        let distance = (desktop.pan - destination).length();
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerMoved(press + delta)],
        );
        let mut remaining = (desktop.pan - destination).length();
        assert!(
            remaining > 0.1 && remaining < distance,
            "Drag must ease toward the pointer"
        );
        for _ in 0..20 {
            frame(&mut desktop, &mut fixture, vec![]);
            let next = (desktop.pan - destination).length();
            assert!(next <= remaining, "Drag must settle without overshooting");
            remaining = next;
            assert!(desktop.node_drag.is_none());
            assert_eq!(desktop.selection, vec![id.clone()]);
            assert_eq!(desktop.layers_available(), icon_mode);
        }
        assert!(
            (desktop.pan - initial - delta).length() < 0.01,
            "Pan blocked or drifted: icon_mode={icon_mode}, {button:?}, {target}, passes={max_passes}, actual={:?}, expected={:?}",
            desktop.pan,
            initial + delta,
        );
    }
    let delta = egui::vec2(-150.0, -100.0);
    let release = press + delta;
    frame(
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::PointerMoved(release),
            pointer_button(release, false),
            egui::Event::PointerMoved(press + egui::vec2(400.0, 100.0)),
        ],
    );
    assert!((desktop.pan - initial - delta).length() > 0.1);
    assert!(
        desktop.camera_editing,
        "Release must retain the inertia undo group"
    );
    for _ in 0..25 {
        frame(&mut desktop, &mut fixture, vec![]);
    }
    assert!((desktop.pan - initial - delta).length() < 0.01);
    assert!(!desktop.camera_editing);
    assert!(desktop.camera_motion.is_none());
    assert!(
        desktop.canvas_menu.is_none(),
        "Dragging must not open a menu"
    );
    assert_eq!(fixture.editor.undo_label(), Some("Canvas camera"));
    fixture.editor.undo();
    assert_eq!(fixture.editor.document(), &before);
    fixture.editor.redo();
    assert_eq!(
        fixture.editor.document().project()["achievementsCamera"]["offsetX"],
        json!(f64::from(-desktop.pan.x)),
    );
}

#[test]
fn camera_inertia_stops_for_new_input_focus_loss_and_camera_or_view_changes() {
    for interrupt in [
        "hold", "escape", "disabled", "focus", "view", "camera", "wheel", "menu",
    ] {
        let mut fixture = Fixture::new();
        let mut desktop = Desktop::default();
        let ctx = egui::Context::default();
        let frame = |desktop: &mut Desktop, fixture: &mut Fixture, time, events, state: &str| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    focused: state != "focus",
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    if state == "disabled" {
                        ui.disable();
                    }
                    desktop.show(ui, fixture);
                    if ui.ctx().current_pass_index() == 0 {
                        ui.ctx()
                            .request_discard("Camera inertia interruption regression");
                    }
                },
            );
            output.textures_delta.clear();
        };
        let start = egui::pos2(100.0, 750.0);
        let end = start + egui::vec2(100.0, 50.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut desktop, &mut fixture, 1.0, vec![], "");
        frame(
            &mut desktop,
            &mut fixture,
            2.0,
            vec![button(start, true)],
            "",
        );
        assert_eq!(desktop.pan, egui::vec2(30.0, 30.0));
        if interrupt == "hold" {
            let held = 2.0 + ctx.options(|options| options.input_options.max_click_duration) + 0.1;
            frame(&mut desktop, &mut fixture, held, vec![], "");
            frame(
                &mut desktop,
                &mut fixture,
                held + 0.02,
                vec![button(start, false)],
                "",
            );
            assert!(desktop.canvas_menu.is_none());
            assert!(desktop.camera_motion.is_none());
            assert_eq!(desktop.pan, egui::vec2(30.0, 30.0));
            continue;
        }
        frame(
            &mut desktop,
            &mut fixture,
            2.52,
            vec![egui::Event::PointerMoved(end)],
            "",
        );
        frame(
            &mut desktop,
            &mut fixture,
            2.54,
            vec![button(end, false)],
            "",
        );
        assert!(desktop.camera_motion.is_some() && desktop.camera_editing);
        let pan = desktop.pan;
        let events = match interrupt {
            "escape" => vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            "view" => {
                desktop.view = desktop::View::Art;
                vec![]
            }
            "camera" => {
                fixture
                    .execute(
                        "External camera",
                        Command::CanvasCamera {
                            kind: EntityKind::Achievement,
                            zoom: 1.0,
                            offset_x: 123.0,
                            offset_y: 456.0,
                        },
                        None,
                    )
                    .unwrap();
                vec![]
            }
            "wheel" => vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 40.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }],
            "menu" => vec![button(end, true), button(end, false)],
            _ => vec![],
        };
        frame(&mut desktop, &mut fixture, 2.56, events, interrupt);
        assert!(
            desktop.camera_motion.is_none(),
            "{interrupt} must stop the glide"
        );
        assert!(desktop.camera_drag.is_none());
        if interrupt == "camera" {
            assert_eq!(desktop.pan, egui::vec2(-123.0, -456.0));
        } else if interrupt != "wheel" {
            assert_eq!(
                desktop.pan, pan,
                "{interrupt} must stop at the visible position"
            );
        }
        if interrupt == "menu" {
            assert!(desktop.canvas_menu.is_some() && egui::Popup::is_any_open(&ctx));
        }
        let stopped = desktop.pan;
        frame(&mut desktop, &mut fixture, 3.0, vec![], "");
        if interrupt != "wheel" {
            assert_eq!(
                desktop.pan, stopped,
                "Cancelled inertia must stay cancelled"
            );
        }
    }
}

#[test]
fn canvas_gestures_ignore_disabled_canvas_inspector_and_layers_window() {
    for target in ["disabled", "inspector", "layers"] {
        for button in [
            egui::PointerButton::Primary,
            egui::PointerButton::Middle,
            egui::PointerButton::Secondary,
        ] {
            let mut fixture = Fixture::new();
            fixture.layout.position = Some([400.0, 300.0]);
            fixture.layout.size = [450.0, 500.0];
            let revision = fixture.editor.revision();
            let mut desktop = Desktop::default();
            desktop.selection = vec![
                fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ];
            desktop.open_layers();
            let ctx = egui::Context::default();
            let mut frame = |desktop: &mut Desktop, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1600.0, 1000.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        if target == "disabled" {
                            ui.disable();
                        }
                        desktop.show(ui, &mut fixture);
                    },
                );
                output.textures_delta.clear();
            };
            frame(&mut desktop, vec![]);
            frame(&mut desktop, vec![]);
            let initial = desktop.pan;
            let press = match target {
                "layers" => ctx
                    .read_response(crate::layer_palette::layers_panel_id().with("header"))
                    .unwrap()
                    .rect
                    .center(),
                "inspector" => egui::pos2(1550.0, 850.0),
                _ => egui::pos2(100.0, 750.0),
            };
            let pointer_button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(
                &mut desktop,
                vec![
                    egui::Event::PointerMoved(press),
                    pointer_button(press, true),
                ],
            );
            let end = egui::pos2(150.0, 700.0);
            frame(&mut desktop, vec![egui::Event::PointerMoved(end)]);
            frame(&mut desktop, vec![pointer_button(end, false)]);
            frame(&mut desktop, vec![]);
            assert_eq!(
                desktop.pan, initial,
                "Camera moved from {target}, {button:?}"
            );
            assert!(desktop.camera_drag.is_none());
            assert!(desktop.selection_drag.is_none());
            assert!(!desktop.camera_editing);
            assert_eq!(fixture.editor.revision(), revision);
        }
    }
}

#[test]
fn cards_and_localization_preview_resolve_unlock_text_while_edits_keep_templates() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let name = entity["displayNameLocalizationId"]
        .as_str()
        .unwrap()
        .to_owned();
    let description = entity["descriptionLocalizationId"]
        .as_str()
        .unwrap()
        .to_owned();
    entity["unlockValue"] = json!(5);
    fixture
        .editor
        .execute(
            "Template",
            Command::Batch(vec![
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                Command::SetLocalization {
                    id: name,
                    language: "English".into(),
                    text: "Find {0} sticks".into(),
                },
                Command::SetLocalization {
                    id: description,
                    language: "English".into(),
                    text: "Collect {0} sticks".into(),
                },
            ]),
            None,
        )
        .unwrap();
    let before = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    desktop.inspector_tab = desktop::InspectorTab::Localization;
    fn collect(shape: &egui::Shape, labels: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => labels.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, labels)),
            _ => {}
        }
    }
    let mut draw = |inspector| {
        let preferences = fixture.preferences().clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 2000.0),
                )),
                ..Default::default()
            },
            |ui| {
                if inspector {
                    desktop.inspector(ui, &mut fixture);
                } else {
                    desktop.canvas(ui, &mut fixture, &preferences, None);
                }
            },
        );
        output.textures_delta.clear();
        let mut labels = Vec::new();
        for shape in output.shapes {
            collect(&shape.shape, &mut labels);
        }
        labels
    };
    draw(false);
    let labels = draw(false);
    assert!(labels.iter().any(|text| text == "Find 5 sticks"));
    assert!(labels.iter().any(|text| text == "Collect 5 sticks"));
    draw(true);
    let labels = draw(true);
    assert!(labels.iter().any(|text| text == "Find {0} sticks"));
    assert!(labels.iter().any(|text| text == "Preview: Find 5 sticks"));
    assert!(
        labels
            .iter()
            .any(|text| text == "Preview: Collect 5 sticks")
    );
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn zoom_labels_fade_for_four_tenths_and_stat_wallpaper_is_scoped_to_linked_cards() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let api = entity["apiName"].as_str().unwrap().to_owned();
    entity["progressStatId"] =
        fixture.editor.document().entities(EntityKind::Stat)[0]["id"].clone();
    entity["unlockValue"] = json!(1);
    fixture
        .editor
        .execute(
            "Link stat",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: entity["id"].as_str().unwrap().into(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture| {
        time += 0.02;
        let p = fixture.preferences().clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| desktop.canvas(ui, fixture, &p, None),
        );
        output.textures_delta.clear();
        output.shapes
    };
    let alpha = |shapes: &[egui::epaint::ClippedShape]| {
        shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == api => Some(t.fallback_color.a()),
                _ => None,
            })
            .unwrap()
    };
    let before = fixture.editor.document().clone();
    let output = frame(&mut desktop, &mut fixture);
    assert_eq!(alpha(&output), 255);
    let cue = theme::mix(
        theme::color(fixture.preferences(), "Warning"),
        theme::color(fixture.preferences(), "Error"),
        0.25,
    )
    .gamma_multiply(0.1);
    let patterned: Vec<_> = output
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Path(path) if path.fill == cue => Some(s.clip_rect),
            _ => None,
        })
        .collect();
    assert!(!patterned.is_empty());
    assert!(
        patterned.iter().all(|rect| *rect == patterned[0]),
        "Only the linked card gets arrows"
    );
    assert_eq!(fixture.editor.document(), &before);
    let camera = |zoom| Command::CanvasCamera {
        kind: EntityKind::Achievement,
        zoom,
        offset_x: -30.0,
        offset_y: -30.0,
    };
    fixture
        .editor
        .execute("Zoom out", camera(0.5), None)
        .unwrap();
    let first = alpha(&frame(&mut desktop, &mut fixture));
    for _ in 0..9 {
        frame(&mut desktop, &mut fixture);
    }
    let halfway = alpha(&frame(&mut desktop, &mut fixture));
    assert!(first > halfway && halfway > 0);
    for _ in 0..11 {
        frame(&mut desktop, &mut fixture);
    }
    assert_eq!(alpha(&frame(&mut desktop, &mut fixture)), 0);
    fixture
        .editor
        .execute("Zoom in", camera(1.0), None)
        .unwrap();
    for _ in 0..9 {
        frame(&mut desktop, &mut fixture);
    }
    let rising = alpha(&frame(&mut desktop, &mut fixture));
    assert!(rising > 0 && rising < 255);
    fixture
        .editor
        .execute("Reverse fade", camera(0.4), None)
        .unwrap();
    let reversed = alpha(&frame(&mut desktop, &mut fixture));
    assert!(reversed < rising && reversed > 0);
}

#[test]
fn layer_connectors_use_current_frame_geometry_and_style_membership() {
    let mut fixture = Fixture::new();
    let recipe = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]});
    let entities = fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)
        .to_vec();
    let selected = entities[0]["id"].as_str().unwrap().to_owned();
    for mut entity in entities {
        entity["icon"] = recipe.clone();
        fixture
            .editor
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: entity["id"].as_str().unwrap().into(),
                    value: entity,
                },
                None,
            )
            .unwrap();
    }
    let style = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Shared".into(),
        recipe,
    };
    fixture.save_icon_style(style.clone()).unwrap();
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![selected.clone()];
    desktop.open_layers();
    desktop.layer_palette.achievement = Some(selected);
    let mut panel = egui::Rect::from_min_size(egui::pos2(900.0, 80.0), egui::vec2(500.0, 600.0));
    desktop.layer_palette.layout.position = Some(panel.min.into());
    desktop.layer_palette.layout.size = panel.size().into();
    desktop.layer_palette.layout_loaded = true;
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop| {
        time += 0.02;
        let p = fixture.preferences().clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| {
                desktop.canvas(ui, &mut fixture, &p, None);
                desktop.layers_window(ui.ctx(), &mut fixture, true);
                desktop.paint_layer_connections();
            },
        );
        output.textures_delta.clear();
        output
            .shapes
            .into_iter()
            .filter_map(|s| match s.shape {
                egui::Shape::LineSegment { points, stroke } if stroke.width == 0.6 => {
                    Some((points, stroke.color.a()))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for _ in 0..23 {
        frame(&mut desktop);
    }
    let single = frame(&mut desktop);
    assert_eq!(single.len(), 1);
    let header_icon = desktop.layer_palette.header.as_ref().unwrap().icon;
    let thumbnail = ctx
        .read_response(crate::layer_palette::layers_panel_id().with("center-icon"))
        .unwrap()
        .rect
        .shrink(4.0);
    assert_eq!(header_icon, thumbnail);
    assert!(header_icon.expand(0.01).contains(single[0].0[1]));
    assert!(!header_icon.shrink(0.01).contains(single[0].0[1]));
    desktop.layer_palette.active_style = Some(style.id.clone());
    let first = frame(&mut desktop);
    assert_eq!(first.len(), 2);
    assert!(first[1].1 > 0 && first[1].1 < single[0].1);
    for _ in 0..4 {
        frame(&mut desktop);
    }
    let middle = frame(&mut desktop);
    assert!(middle[1].1 > first[1].1 && middle[1].1 < single[0].1);
    for _ in 0..5 {
        frame(&mut desktop);
    }
    let full = frame(&mut desktop);
    assert_eq!(full[1].1, single[0].1);
    desktop.layer_palette.active_style = None;
    let fading = frame(&mut desktop);
    assert!(fading[1].1 > 0 && fading[1].1 < full[1].1);
    desktop.layer_palette.active_style = Some(style.id);
    assert!(frame(&mut desktop)[1].1 > fading[1].1);
    for _ in 0..11 {
        frame(&mut desktop);
    }
    let before_move = frame(&mut desktop);
    panel = panel.translate(egui::vec2(-80.0, 120.0));
    desktop.layer_palette.layout.position = Some(panel.min.into());
    desktop.layer_palette.panel_rect = None;
    let moved = frame(&mut desktop);
    assert_eq!(moved.len(), 2);
    assert_eq!(
        moved[0].0[1] - before_move[0].0[1],
        egui::vec2(-80.0, 120.0)
    );
    assert_ne!(moved[0].0[0], before_move[0].0[0]);
    desktop.layer_palette.active_style = None;
    for _ in 0..11 {
        frame(&mut desktop);
    }
    assert_eq!(frame(&mut desktop).len(), 1);
    desktop.layer_palette.open = false;
    desktop.inspector_tab = crate::desktop::InspectorTab::General;
    assert!(frame(&mut desktop).is_empty());
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn icon_focus_mask_fades_and_keeps_only_edited_art_above_it() {
    let mut fixture = Fixture::new();
    fixture
        .editor
        .execute("Third icon", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let entities = fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)
        .to_vec();
    let ids: Vec<_> = entities
        .iter()
        .map(|entity| entity["id"].as_str().unwrap().to_owned())
        .collect();
    let recipe = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]});
    for (index, mut entity) in entities.into_iter().enumerate() {
        entity["icon"] = recipe.clone();
        entity["icon"]["layers"][0]["id"] = json!(new_id());
        if index == 2 {
            entity["icon"]["layers"][0]["solidColorHex"] = json!("#FF00FF00");
        }
        fixture
            .editor
            .execute(
                "Icon recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: ids[index].clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        fixture.previews.insert(
            ids[index].clone(),
            Preview {
                width: 1,
                height: 1,
                source_size: [1, 1],
                rgba: vec![255, 0, 0, 128].into(),
                fingerprint: ids[index].clone(),
            },
        );
    }
    let style = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Shared".into(),
        recipe,
    };
    fixture.save_icon_style(style.clone()).unwrap();
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![ids[0].clone()];
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop| {
        time += 0.02;
        let p = fixture.preferences().clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| desktop.canvas(ui, &mut fixture, &p, None),
        );
        output.textures_delta.clear();
        output.shapes
    };
    let mask = |shapes: &[egui::epaint::ClippedShape]| {
        shapes
            .iter()
            .enumerate()
            .find_map(|(index, shape)| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.width() > 1500.0
                        && rect.fill.r() == 0
                        && rect.fill.g() == 0
                        && rect.fill.b() == 0
                        && rect.fill.a() > 0 =>
                {
                    Some((index, rect.fill.a()))
                }
                _ => None,
            })
    };
    assert!(mask(&frame(&mut desktop)).is_none());
    desktop.open_layers();
    let first_alpha = mask(&frame(&mut desktop)).unwrap().1;
    for _ in 0..9 {
        frame(&mut desktop);
    }
    let middle_alpha = mask(&frame(&mut desktop)).unwrap().1;
    for _ in 0..12 {
        frame(&mut desktop);
    }
    let output = frame(&mut desktop);
    let (mask_index, final_alpha) = mask(&output).unwrap();
    assert!(first_alpha < middle_alpha && middle_alpha < final_alpha);
    assert_eq!(final_alpha, 166);
    let check_order =
        |desktop: &Desktop, shapes: &[egui::epaint::ClippedShape], mask_index, exempt: &[bool]| {
            for (id, exempt) in ids.iter().zip(exempt) {
                let texture = desktop.textures[&format!("canvas:{id}")].1.id();
                let positions: Vec<_> = shapes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, shape)| match &shape.shape {
                        egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(index),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    positions.len(),
                    1,
                    "Translucent artwork must be painted once"
                );
                assert_eq!(positions[0] > mask_index, *exempt);
            }
        };
    check_order(&desktop, &output, mask_index, &[true, false, false]);
    desktop.layer_palette.achievement = Some(ids[0].clone());
    desktop.layer_palette.active_style = Some(style.id);
    let output = frame(&mut desktop);
    check_order(
        &desktop,
        &output,
        mask(&output).unwrap().0,
        &[true, true, false],
    );
    let tint = |shapes: &[egui::epaint::ClippedShape], id: &str| {
        let texture = desktop.textures[&format!("canvas:{id}")].1.id();
        shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                    Some(mesh.vertices[0].color)
                }
                _ => None,
            })
            .unwrap()
    };
    let entering = tint(&output, &ids[1]);
    assert!(entering.r() > 89 && entering.r() < 255);
    assert_eq!(entering.a(), 255, "The texture's own alpha stays intact");
    assert_eq!(tint(&output, &ids[0]), egui::Color32::WHITE);
    for _ in 0..10 {
        frame(&mut desktop);
    }
    // Style members stay above the mask until their 0.2-second fade ends.
    desktop.layer_palette.active_style = None;
    let output = frame(&mut desktop);
    check_order(
        &desktop,
        &output,
        mask(&output).unwrap().0,
        &[true, true, false],
    );
    for _ in 0..10 {
        frame(&mut desktop);
    }
    let output = frame(&mut desktop);
    check_order(
        &desktop,
        &output,
        mask(&output).unwrap().0,
        &[true, false, false],
    );
    desktop.layer_palette.open = false;
    let closing_alpha = mask(&frame(&mut desktop)).unwrap().1;
    assert!(closing_alpha < final_alpha && closing_alpha > 0);
    desktop.layer_palette.open = true;
    assert!(mask(&frame(&mut desktop)).unwrap().1 > closing_alpha);
    desktop.inspector_tab = desktop::InspectorTab::General;
    for _ in 0..23 {
        frame(&mut desktop);
    }
    assert!(mask(&frame(&mut desktop)).is_none());
    desktop.kind = EntityKind::Stat;
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    for _ in 0..23 {
        frame(&mut desktop);
    }
    assert!(mask(&frame(&mut desktop)).is_none());
    assert_eq!(fixture.editor.document(), &before);
}
