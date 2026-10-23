use super::*;

fn labels(shape: &egui::Shape, output: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => output.push((
            text.galley.text().to_owned(),
            egui::Rect::from_min_size(text.pos, text.galley.size()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                labels(shape, output);
            }
        }
        _ => {}
    }
}

#[test]
fn help_pages_show_compiled_version_and_switch_without_editing() {
    for (width, scale) in [(1440.0, 1.0), (440.0, 1.5)] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        let mut desktop = Desktop::default();
        let mut fixture = Fixture::new();
        let before = fixture.editor.document().clone();
        let revision = fixture.editor.revision();
        desktop.open_about();
        let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 920.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| desktop.show(ui, fixture),
            );
            output.textures_delta.clear();
            let mut result = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut result);
            }
            result
        };
        let mut shown = Vec::new();
        for _ in 0..3 {
            shown = frame(&mut desktop, &mut fixture, vec![]);
        }
        let version = format!("Version {}", crate::change_log::current_version());
        assert!(shown.iter().any(|(text, _)| text == &version));
        let change_log = shown
            .iter()
            .find(|(text, _)| text == "Change log")
            .unwrap()
            .1;
        assert!(change_log.left() >= 0.0 && change_log.right() <= width);
        let pos = change_log.center();
        frame(
            &mut desktop,
            &mut fixture,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(desktop.help_page == crate::about::HelpPage::Changes);
        for _ in 0..3 {
            shown = frame(&mut desktop, &mut fixture, vec![]);
        }
        assert!(shown.iter().any(|(text, _)| text == &version));
        assert!(
            shown
                .iter()
                .any(|(text, _)| text == crate::change_log::ENTRIES[0].changes[0])
        );
        let close = shown.iter().find(|(text, _)| text == "Close").unwrap().1;
        assert!(close.bottom() <= 920.0 && close.right() <= width);
        assert!(desktop.automation_busy());
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::Key {
                key: egui::Key::N,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }],
        );
        assert_eq!(fixture.requests, 0);
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!desktop.about);
        assert!(matches!(
            fixture.last_request,
            Some(UiRequest::AboutDismissed)
        ));
        assert_eq!(fixture.editor.document(), &before);
        assert_eq!(fixture.editor.revision(), revision);
    }
}
