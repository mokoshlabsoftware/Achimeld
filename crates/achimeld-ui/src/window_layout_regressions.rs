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
