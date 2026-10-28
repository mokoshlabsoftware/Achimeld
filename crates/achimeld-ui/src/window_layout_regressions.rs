use super::*;
use crate::inspector_panel::Placement;

fn labels(shape: &egui::Shape, output: &mut BTreeMap<String, egui::Pos2>) {
    match shape {
        egui::Shape::Text(text) => {
            output.insert(
                text.galley.text().to_owned(),
                text.pos + text.galley.rect.center().to_vec2(),
            );
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                labels(shape, output);
            }
        }
        _ => {}
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
fn window_size_panel_applies_and_cancels_without_editing_the_project() {
    let ctx = egui::Context::default();
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let revision = fixture.editor.revision();
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        );
        output.textures_delta.clear();
        let mut shown = BTreeMap::new();
        for shape in &output.shapes {
            labels(&shape.shape, &mut shown);
        }
        let commands = output
            .viewport_output
            .remove(&egui::ViewportId::ROOT)
            .map_or_else(Vec::new, |viewport| viewport.commands);
        (shown, commands)
    };
    for _ in 0..3 {
        frame(&mut desktop, &mut fixture, vec![]);
    }
    let click = |desktop: &mut Desktop, fixture: &mut Fixture, position| {
        frame(
            desktop,
            fixture,
            vec![egui::Event::PointerMoved(position), button(position, true)],
        );
        frame(
            desktop,
            fixture,
            vec![egui::Event::PointerMoved(position), button(position, false)],
        )
    };
    let windows = frame(&mut desktop, &mut fixture, vec![]).0["Windows"];
    click(&mut desktop, &mut fixture, windows);
    let menu = frame(&mut desktop, &mut fixture, vec![]).0;
    click(&mut desktop, &mut fixture, menu["Set window size…"]);
    let panel = frame(&mut desktop, &mut fixture, vec![]).0;
    assert!(panel.contains_key("Width") && panel.contains_key("Height"));
    let start = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("set-window-size")))
        .unwrap();
    let title = start.min + egui::vec2(60.0, 12.0);
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(title), button(title, true)],
    );
    let moved_title = title + egui::vec2(75.0, 45.0);
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(moved_title)],
    );
    frame(&mut desktop, &mut fixture, vec![button(moved_title, false)]);
    let moved = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("set-window-size")))
        .unwrap();
    assert!(
        moved.min.distance(start.min) > 50.0,
        "Panel title must drag it"
    );
    let panel = frame(&mut desktop, &mut fixture, vec![]).0;
    let (_, commands) = click(&mut desktop, &mut fixture, panel["Apply"]);
    assert!(commands.contains(&egui::ViewportCommand::InnerSize(egui::vec2(1440.0, 920.0))));
    assert!(desktop.window_size.is_none());
    let windows = frame(&mut desktop, &mut fixture, vec![]).0["Windows"];
    click(&mut desktop, &mut fixture, windows);
    let menu = frame(&mut desktop, &mut fixture, vec![]).0;
    click(&mut desktop, &mut fixture, menu["Set window size…"]);
    let panel = frame(&mut desktop, &mut fixture, vec![]).0;
    let (_, commands) = click(&mut desktop, &mut fixture, panel["Cancel"]);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::InnerSize(_)))
    );
    assert!(desktop.window_size.is_none());
    assert_eq!(fixture.editor.revision(), revision);
}

#[test]
fn windows_menu_resets_visible_and_closed_windows_without_editing() {
    for (visible, sidebar) in [(false, true), (true, true), (true, false)] {
        for scale in [1.0, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let mut fixture = Fixture::new();
            fixture
                .editor
                .execute(
                    "Layout preference",
                    Command::Preferences(Preferences {
                        layer_properties_in_sidebar: sidebar,
                        ..fixture.preferences().clone()
                    }),
                    None,
                )
                .unwrap();
            fixture.layout.position = Some([1380.0, 150.0]);
            let mut desktop = Desktop::default();
            desktop.inspector_panel.placement = Placement::Floating;
            if visible {
                desktop.selection = vec![
                    fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                ];
                desktop.open_layers();
            }
            let document = fixture.editor.document().clone();
            let revision = fixture.editor.revision();
            let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1440.0, 920.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| desktop.show(ui, fixture),
                );
                output.textures_delta.clear();
                let mut result = BTreeMap::new();
                for shape in &output.shapes {
                    labels(&shape.shape, &mut result);
                }
                result
            };
            for _ in 0..3 {
                frame(&mut desktop, &mut fixture, vec![]);
            }
            let selection = desktop.selection.clone();
            let selected_layer = desktop.selected_layer.clone();
            let camera = (desktop.zoom, desktop.pan);
            let split = desktop.layer_palette.split;
            // Seed closed tools and a dynamically identified movable color editor.
            let ids = [
                egui::Id::new("gradient-explorer-window"),
                egui::Id::new("effect-explorer-window"),
                egui::Id::new("preferences-window"),
                egui::Id::new(("color-editor", "arbitrary-layer-id")),
            ];
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
                for id in ids {
                    egui::Window::new("Remembered tool")
                        .id(id)
                        .current_pos([1000.0, 400.0])
                        .show(&ctx, |ui| {
                            ui.label("Tool contents");
                        });
                }
            });
            output.textures_delta.clear();
            assert!(
                ids.iter()
                    .all(|id| ctx.memory(|m| m.area_rect(*id).is_some()))
            );
            // Restore the editor's hit registry after seeding closed windows.
            let mut shown = BTreeMap::new();
            for _ in 0..2 {
                shown = frame(&mut desktop, &mut fixture, vec![]);
            }
            let windows = shown["Windows"];
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    &mut fixture,
                    vec![egui::Event::PointerMoved(windows), button(windows, pressed)],
                );
            }
            for _ in 0..2 {
                shown = frame(&mut desktop, &mut fixture, vec![]);
            }
            let reset = shown["Reset layout"];
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    &mut fixture,
                    vec![egui::Event::PointerMoved(reset), button(reset, pressed)],
                );
            }
            for _ in 0..3 {
                shown = frame(&mut desktop, &mut fixture, vec![]);
            }
            assert!(
                !shown.contains_key("Reset layout"),
                "Menu must close after activation"
            );
            assert!(
                ids.iter()
                    .all(|id| ctx.memory(|m| m.area_rect(*id).is_none()))
            );
            assert_eq!(desktop.inspector_panel.placement, Placement::Right);
            if visible {
                let rect = desktop.layer_palette.panel_rect.unwrap();
                assert!(ctx.content_rect().contains_rect(rect), "{rect:?}");
                assert!(
                    rect.center().distance(ctx.content_rect().center()) < 2.0,
                    "{rect:?}"
                );
                assert_eq!(fixture.layout.position, Some(rect.min.into()));
            } else {
                assert_eq!(fixture.layout.position, None);
                // Closing/reopening the app uses the persisted recovered placement.
                let mut reopened = Desktop::default();
                reopened.selection = vec![
                    fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                ];
                reopened.open_layers();
                for _ in 0..3 {
                    frame(&mut reopened, &mut fixture, vec![]);
                }
                assert!(
                    ctx.content_rect()
                        .contains_rect(reopened.layer_palette.panel_rect.unwrap())
                );
            }
            assert_eq!(desktop.selection, selection);
            assert_eq!(desktop.selected_layer, selected_layer);
            assert_eq!((desktop.zoom, desktop.pan), camera);
            assert_eq!(desktop.layer_palette.split, split);
            assert_eq!(fixture.editor.document(), &document);
            assert_eq!(fixture.editor.revision(), revision);
            assert_eq!(fixture.requests, 0);
        }
    }
}

fn toolbar_texts(shape: &egui::Shape, output: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => output.push((
            text.galley.text().to_owned(),
            egui::Rect::from_min_size(text.pos, text.galley.size()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                toolbar_texts(shape, output);
            }
        }
        _ => {}
    }
}

struct ToolbarSnap {
    filter: egui::Rect,
    sync: egui::Rect,
    project: Option<egui::Rect>,
    tab_labels: bool,
}

fn toolbar_snap(width: f32, scale: f32, name: &str) -> ToolbarSnap {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    configure_test_fonts(&ctx);
    let mut fixture = Fixture::new();
    if name != "Untitled project" {
        fixture
            .execute(
                "Rename",
                Command::ProjectSettings {
                    name: name.into(),
                    app_id: 480,
                    atlas: None,
                    assets: None,
                },
                None,
            )
            .unwrap();
    }
    let mut desktop = Desktop::default();
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 800.0),
                )),
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        )
    };
    // egui keeps the previous pass's widget rects. Three frames let the
    // settled toolbar positions replace the first pass.
    let mut output = frame(&mut desktop, &mut fixture);
    output.textures_delta.clear();
    output = frame(&mut desktop, &mut fixture);
    output.textures_delta.clear();
    output = frame(&mut desktop, &mut fixture);
    let mut texts = Vec::new();
    for shape in &output.shapes {
        toolbar_texts(&shape.shape, &mut texts);
    }
    output.textures_delta.clear();
    let filter = ctx
        .read_response(egui::Id::new("toolbar-filter"))
        .unwrap_or_else(|| panic!("Missing filter at width {width}, scale {scale}"))
        .rect;
    let sync = ctx
        .read_response(egui::Id::new("toolbar-sync"))
        .unwrap()
        .rect;
    // A truncated project name no longer matches the full string. Any toolbar
    // text sitting between the filter and the action is that name.
    let project = texts.iter().find_map(|(_, rect)| {
        (rect.center().y < 90.0 * scale
            && rect.left() >= filter.right() - 1.0
            && rect.right() <= sync.left() + 1.0)
            .then_some(*rect)
    });
    let tab_labels = [
        "Achievements",
        "Stats",
        "Sync",
        "Steam",
        "Art Data Bank",
        "Style Viewer",
    ]
    .iter()
    .all(|label| {
        texts
            .iter()
            .any(|(text, rect)| text == label && rect.center().y < 80.0)
    });
    ToolbarSnap {
        filter,
        sync,
        project,
        tab_labels,
    }
}

fn assert_filter_clear(snap: &ToolbarSnap, width: f32) {
    assert!(
        snap.filter.right() <= snap.sync.left() + 0.5,
        "width {width}: filter {:?} overlaps sync {:?}",
        snap.filter,
        snap.sync
    );
    if let Some(project) = snap.project {
        assert!(
            snap.filter.right() <= project.left() + 0.5,
            "width {width}: filter {:?} overlaps project {project:?}",
            snap.filter
        );
        assert!(
            project.right() <= snap.sync.left() + 0.5,
            "width {width}: project {project:?} overlaps sync {:?}",
            snap.sync
        );
    }
    assert!(
        snap.filter.width() <= 210.5,
        "width {width}: filter grew past 210 ({})",
        snap.filter.width()
    );
    assert!(
        snap.filter.left() >= -0.5
            && snap.filter.right() <= width + 1.0
            && snap.sync.right() <= width + 1.0,
        "width {width}: filter {:?} sync {:?}",
        snap.filter,
        snap.sync
    );
}

#[test]
fn narrow_toolbar_shrinks_the_filter_and_drops_labels_without_overlap() {
    let wide = toolbar_snap(1680.0, 1.0, "Untitled project");
    assert!(wide.tab_labels, "A wide window keeps the toolbar labels");
    assert!(
        (wide.filter.width() - 210.0).abs() < 1.5,
        "Preferred filter width, got {}",
        wide.filter.width()
    );
    assert_filter_clear(&wide, 1680.0);

    let mut saw_shrunk_labels = false;
    for width in [
        1500.0, 1400.0, 1300.0, 1200.0, 1100.0, 1000.0, 900.0, 800.0, 720.0,
    ] {
        let snap = toolbar_snap(width, 1.0, "Untitled project");
        assert_filter_clear(&snap, width);
        if snap.tab_labels && snap.filter.width() < 200.0 {
            saw_shrunk_labels = true;
        }
        if !snap.tab_labels {
            assert!(
                snap.filter.width() + 0.5 >= 96.0,
                "width {width}: icon mode still needs a usable filter, got {}",
                snap.filter.width()
            );
        }
    }
    assert!(
        saw_shrunk_labels,
        "The filter must shrink while the labels are still visible"
    );
    let narrow = toolbar_snap(720.0, 1.0, "Untitled project");
    assert!(
        !narrow.tab_labels,
        "The minimum window keeps icons and hides toolbar labels"
    );
    assert!(narrow.filter.width() + 0.5 >= 96.0);
    assert!(
        narrow.sync.width() + 8.0 < wide.sync.width(),
        "The staged action drops its label when the window is narrow"
    );

    let long = "A very long project name that must stay beside the filter";
    for (width, scale) in [(860.0, 1.0), (900.0, 1.5), (720.0, 1.25)] {
        let snap = toolbar_snap(width, scale, long);
        assert_filter_clear(&snap, width);
        assert!(
            snap.project.is_some(),
            "width {width} scale {scale}: the project name must remain visible"
        );
    }
}
