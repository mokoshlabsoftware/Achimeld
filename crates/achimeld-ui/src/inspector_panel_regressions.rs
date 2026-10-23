use super::*;
use crate::inspector_panel::Placement;

struct Harness {
    ctx: egui::Context,
    desktop: Desktop,
    fixture: Fixture,
    time: f64,
    extra_pass: bool,
    screen_size: egui::Vec2,
}

impl Harness {
    fn new(extra_pass: bool, kind: EntityKind) -> Self {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        desktop.kind = kind;
        desktop.view = if kind == EntityKind::Stat {
            desktop::View::Stats
        } else {
            desktop::View::Achievements
        };
        let mut harness = Self {
            ctx,
            desktop,
            fixture: Fixture::new(),
            time: 0.0,
            extra_pass,
            screen_size: egui::vec2(1600.0, 1000.0),
        };
        harness.frame(vec![], true);
        harness.frame(vec![], true);
        harness
    }

    fn frame(&mut self, events: Vec<egui::Event>, enabled: bool) {
        self.time += 0.02;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    self.screen_size,
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                if !enabled {
                    ui.disable();
                }
                self.desktop.show(ui, &mut self.fixture);
                if self.extra_pass && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("Inspector docking regression");
                }
            },
        );
        output.textures_delta.clear();
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn drag(&mut self, target: egui::Pos2) {
        let header = self.desktop.inspector_panel.header.unwrap().center();
        self.frame(
            vec![
                egui::Event::PointerMoved(header),
                Self::button(header, true),
            ],
            true,
        );
        self.frame(vec![egui::Event::PointerMoved(target)], true);
        assert_eq!(self.desktop.inspector_panel.placement, Placement::Floating);
        self.frame(vec![Self::button(target, false)], true);
        self.frame(vec![], true);
    }
}

#[test]
fn inspector_drags_from_right_to_float_left_and_right_in_both_editors() {
    for extra_pass in [false, true] {
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            let mut h = Harness::new(extra_pass, kind);
            let document = h.fixture.editor.document().clone();
            let revision = h.fixture.editor.revision();
            assert_eq!(h.desktop.inspector_panel.placement, Placement::Right);
            h.drag(egui::pos2(800.0, 180.0));
            assert_eq!(h.desktop.inspector_panel.placement, Placement::Floating);
            let rect = h.desktop.inspector_panel.rect.unwrap();
            assert!(rect.left() > 0.0 && rect.right() < 1600.0);
            h.drag(egui::pos2(8.0, 200.0));
            assert_eq!(h.desktop.inspector_panel.placement, Placement::Left);
            assert!(h.desktop.inspector_panel.rect.unwrap().left() < 10.0);
            h.drag(egui::pos2(1590.0, 240.0));
            assert_eq!(h.desktop.inspector_panel.placement, Placement::Right);
            assert!(h.desktop.inspector_panel.rect.unwrap().right() > 1590.0);
            assert_eq!(h.fixture.editor.document(), &document);
            assert_eq!(h.fixture.editor.revision(), revision);
        }
    }
}

#[test]
fn inspector_drag_cancels_on_escape_and_disabled_ui_and_ignores_body_drags() {
    for extra_pass in [false, true] {
        let mut h = Harness::new(extra_pass, EntityKind::Achievement);
        let header = h.desktop.inspector_panel.header.unwrap().center();
        h.frame(
            vec![
                egui::Event::PointerMoved(header),
                Harness::button(header, true),
            ],
            true,
        );
        h.frame(
            vec![egui::Event::PointerMoved(egui::pos2(800.0, 180.0))],
            true,
        );
        assert_eq!(h.desktop.inspector_panel.placement, Placement::Floating);
        h.frame(
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            true,
        );
        assert_eq!(h.desktop.inspector_panel.placement, Placement::Right);
        h.frame(vec![Harness::button(header, false)], true);
        h.frame(
            vec![
                egui::Event::PointerMoved(header),
                Harness::button(header, true),
            ],
            false,
        );
        h.frame(
            vec![
                egui::Event::PointerMoved(egui::pos2(800.0, 180.0)),
                Harness::button(egui::pos2(800.0, 180.0), false),
            ],
            false,
        );
        assert_eq!(h.desktop.inspector_panel.placement, Placement::Right);
        h.frame(vec![], true);
        let body = h.desktop.inspector_panel.rect.unwrap().center();
        h.frame(
            vec![egui::Event::PointerMoved(body), Harness::button(body, true)],
            true,
        );
        h.frame(
            vec![
                egui::Event::PointerMoved(egui::pos2(800.0, 180.0)),
                Harness::button(egui::pos2(800.0, 180.0), false),
            ],
            true,
        );
        assert_eq!(h.desktop.inspector_panel.placement, Placement::Right);
    }
}

#[test]
fn floating_inspector_blocks_canvas_wheel_and_retains_selection_and_drafts() {
    let mut h = Harness::new(true, EntityKind::Achievement);
    let id = h
        .fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.desktop.selection = vec![id.clone()];
    h.frame(vec![], true);
    h.drag(egui::pos2(800.0, 180.0));
    let before = (h.desktop.zoom, h.desktop.pan);
    let pointer = h.desktop.inspector_panel.rect.unwrap().center();
    h.frame(vec![egui::Event::PointerMoved(pointer)], true);
    h.frame(
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 40.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        }],
        true,
    );
    assert_eq!((h.desktop.zoom, h.desktop.pan), before);
    assert_eq!(h.desktop.selection.as_slice(), std::slice::from_ref(&id));
    let live = h
        .fixture
        .editor
        .document()
        .entity(EntityKind::Achievement, &id)
        .unwrap()
        .clone();
    h.desktop.entity_drafts.insert(
        id.clone(),
        desktop::EntityDraft {
            base: live.clone(),
            value: live,
            error: "Keep this draft".into(),
        },
    );
    h.drag(egui::pos2(8.0, 200.0));
    assert!(h.desktop.entity_drafts.contains_key(&id));
    assert_eq!(h.desktop.selection, [id]);
}

#[test]
fn inspector_release_uses_release_position_before_later_pointer_movement() {
    for extra_pass in [false, true] {
        let mut h = Harness::new(extra_pass, EntityKind::Stat);
        let header = h.desktop.inspector_panel.header.unwrap().center();
        h.frame(
            vec![
                egui::Event::PointerMoved(header),
                Harness::button(header, true),
            ],
            true,
        );
        h.frame(
            vec![
                egui::Event::PointerMoved(egui::pos2(8.0, 180.0)),
                Harness::button(egui::pos2(8.0, 180.0), false),
                egui::Event::PointerMoved(egui::pos2(1590.0, 180.0)),
            ],
            true,
        );
        assert_eq!(h.desktop.inspector_panel.placement, Placement::Left);
    }
}

#[test]
fn floating_inspector_resizes_and_stays_reachable_after_workspace_shrinks() {
    let mut h = Harness::new(false, EntityKind::Achievement);
    h.drag(egui::pos2(1100.0, 180.0));
    let before = h.desktop.inspector_panel.rect.unwrap();
    let corner = before.max - egui::Vec2::splat(2.0);
    h.frame(
        vec![
            egui::Event::PointerMoved(corner),
            Harness::button(corner, true),
        ],
        true,
    );
    h.frame(
        vec![egui::Event::PointerMoved(corner + egui::vec2(100.0, -80.0))],
        true,
    );
    h.frame(
        vec![Harness::button(corner + egui::vec2(100.0, -80.0), false)],
        true,
    );
    h.frame(vec![], true);
    let resized = h.desktop.inspector_panel.rect.unwrap();
    assert!(
        resized.width() > before.width() + 50.0,
        "{before:?} -> {resized:?}"
    );
    assert!(
        resized.height() < before.height() - 40.0,
        "{before:?} -> {resized:?}"
    );
    h.drag(egui::pos2(8.0, 180.0));
    assert!((h.desktop.inspector_panel.rect.unwrap().width() - resized.width()).abs() < 1.0);
    h.drag(egui::pos2(1100.0, 180.0));
    h.screen_size = egui::vec2(900.0, 600.0);
    h.frame(vec![], true);
    h.frame(vec![], true);
    let header = h.desktop.inspector_panel.header.unwrap();
    assert!(header.left() >= 0.0 && header.right() <= 900.0);
    assert!(header.top() >= 24.0 && header.bottom() < 570.0);
}

#[test]
fn inspector_placement_survives_view_and_project_switches() {
    let mut h = Harness::new(false, EntityKind::Achievement);
    h.drag(egui::pos2(8.0, 200.0));
    h.desktop.view = desktop::View::Art;
    h.frame(vec![], true);
    h.desktop.view = desktop::View::Stats;
    h.desktop.kind = EntityKind::Stat;
    h.frame(vec![], true);
    assert_eq!(h.desktop.inspector_panel.placement, Placement::Left);
    h.fixture = Fixture::new();
    h.frame(vec![], true);
    assert_eq!(h.desktop.inspector_panel.placement, Placement::Left);
}

#[test]
fn inspector_header_placement_menu_supports_keyboard_docking() {
    let mut h = Harness::new(true, EntityKind::Achievement);
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    h.ctx
        .memory_mut(|memory| memory.request_focus(egui::Id::new("inspector-drag-header")));
    h.frame(vec![key(egui::Key::Enter)], true);
    h.frame(vec![], true);
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.frame(vec![key(egui::Key::ArrowDown)], true);
    h.frame(vec![key(egui::Key::Enter)], true);
    h.frame(vec![], true);
    assert_eq!(h.desktop.inspector_panel.placement, Placement::Left);
    assert!(!egui::Popup::is_any_open(&h.ctx));
}

#[test]
#[ignore = "Native docking screenshots; writes to the external build output"]
fn native_inspector_docking_screenshots() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Capture {
        desktop: Desktop,
        fixture: Fixture,
        phase: usize,
        tick: usize,
        requested: bool,
        started: std::time::Instant,
    }
    impl eframe::App for Capture {
        fn raw_input_hook(&mut self, _: &egui::Context, raw: &mut egui::RawInput) {
            raw.focused = true;
            if self.phase > 0 && self.phase < 3 && self.tick <= 2 {
                let target = if self.phase == 1 {
                    egui::pos2(680.0, 160.0)
                } else {
                    egui::pos2(8.0, 180.0)
                };
                let header = self.desktop.inspector_panel.header.unwrap().center();
                match self.tick {
                    0 => raw.events.extend([
                        egui::Event::PointerMoved(header),
                        Harness::button(header, true),
                    ]),
                    1 => raw.events.push(egui::Event::PointerMoved(target)),
                    _ => raw.events.push(Harness::button(target, false)),
                }
            }
        }

        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.desktop.show(ui, &mut self.fixture);
            if let Some(image) = ui.input(|i| {
                i.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let name = [
                    "inspector-docked-right.png",
                    "inspector-floating.png",
                    "inspector-docked-left.png",
                ][self.phase];
                let bytes: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    crate::test_output::root().join(name),
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                assert_eq!(
                    self.desktop.inspector_panel.placement,
                    [Placement::Right, Placement::Floating, Placement::Left][self.phase]
                );
                self.phase += 1;
                self.tick = 0;
                self.requested = false;
                if self.phase == 3 {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.ctx().request_repaint();
                return;
            }
            if self.tick >= 8 && !self.requested {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                self.requested = true;
            }
            self.tick += 1;
            assert!(
                self.started.elapsed().as_secs() < 30,
                "Docking screenshot timed out"
            );
            ui.ctx().request_repaint();
        }
    }
    eframe::run_native(
        "Achimeld sidebar docking",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 820.0])
                .with_decorations(false),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(|cc| {
            configure_test_fonts(&cc.egui_ctx);
            let fixture = Fixture::new();
            let mut desktop = Desktop::default();
            desktop.selection = vec![
                fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                    .as_str()
                    .unwrap()
                    .into(),
            ];
            Ok(Box::new(Capture {
                desktop,
                fixture,
                phase: 0,
                tick: 0,
                requested: false,
                started: std::time::Instant::now(),
            }))
        }),
    )
    .unwrap();
}
