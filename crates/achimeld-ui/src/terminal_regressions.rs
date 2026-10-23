use super::*;

fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => result.push((
            text.galley.text().into(),
            egui::Rect::from_min_size(text.pos, text.galley.size()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                labels(shape, result);
            }
        }
        _ => {}
    }
}

#[test]
fn tools_terminal_opens_after_the_last_import_command_without_changing_the_project() {
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    let mut fixture = Fixture::new();
    let before = fixture.editor.document().clone();
    let mut draw = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
        let mut texts = Vec::new();
        for shape in output.shapes {
            labels(&shape.shape, &mut texts);
        }
        texts
    };
    let mut shown = Vec::new();
    for _ in 0..3 {
        shown = draw(vec![]);
    }
    let tools = shown
        .iter()
        .find(|(text, _)| text == "Tools")
        .unwrap()
        .1
        .center();
    let click = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    draw(click(tools, true));
    draw(click(tools, false));
    for _ in 0..3 {
        shown = draw(vec![]);
    }
    let terminal = shown.iter().find(|(text, _)| text == "Terminal").unwrap().1;
    let import = shown
        .iter()
        .find(|(text, _)| text.starts_with("Import achievement translations"))
        .unwrap()
        .1;
    assert!(terminal.top() > import.bottom());
    draw(click(terminal.center(), true));
    draw(click(terminal.center(), false));
    for _ in 0..3 {
        shown = draw(vec![]);
    }
    assert!(shown.iter().any(|(text, _)| text == "Copy all"));
    assert!(shown.iter().any(|(text, _)| text == "Pause output"));
    assert!(desktop.terminal.open);
    assert_eq!(fixture.editor.document(), &before);
    assert_eq!(fixture.requests, 0);
}

#[test]
#[ignore = "Native Terminal preview written to external build storage"]
fn native_terminal_preview() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        frames: usize,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.frames += 1;
            self.desktop.show(ui, &mut self.fixture);
            if self.frames == 8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            for event in ui.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
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
            }
            assert!(self.frames < 300);
            ui.ctx().request_repaint();
        }
    }
    for message in [
        "Validate requested · Steam App ID 480 · saved project: true",
        "Checking saved project…",
        "Starting the supervised Steam worker…",
        "Starting the dedicated Steam browser…",
        "Waiting for Steam sign-in · complete sign-in in the dedicated browser",
    ] {
        achimeld_application::diagnostics::record("INFO", "validation", message);
    }
    eframe::run_native(
        "Achimeld Terminal preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(|cc| {
            configure_test_fonts(&cc.egui_ctx);
            let mut desktop = Desktop::default();
            desktop.open_terminal();
            Ok(Box::new(Window {
                desktop,
                fixture: Fixture::new(),
                frames: 0,
                output: crate::test_output::root().join("terminal-preview.png"),
            }))
        }),
    )
    .unwrap();
}
