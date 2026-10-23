use super::*;

#[test]
fn text_content_link_toggles_both_layer_types_with_undo_and_keyboard() {
    for kind in ["Text", "Text3d"] {
        for width in [440.0, 1100.0] {
            for passes in [1, 2] {
                let mut rows = LayerRows::new(width, passes);
                let mut layer = layer_palette::new_layer(kind);
                let id = layer["id"].as_str().unwrap().to_owned();
                layer["text"]["content"] = json!("My custom text");
                layer["text"]["linkToAchievementValue"] = json!(false);
                let mut entity = rows.entity();
                entity["icon"] = json!({"layers":[layer]});
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
                rows.desktop.selected_layer = Some(id.clone());
                rows.frame(vec![]);
                rows.frame(vec![]);
                rows.repeat_layout = true;
                rows.frame(vec![]);
                let toggle_id = crate::text_content::control_id(&id, "link");
                let field_id = crate::text_content::control_id(&id, "content");
                let toggle = rows.ctx.read_response(toggle_id).unwrap();
                let field = rows.ctx.read_response(field_id).unwrap();
                assert!(field.enabled() && toggle.enabled());
                assert!(field.rect.right() <= toggle.rect.left() + 0.5);
                assert!((field.rect.center().y - toggle.rect.center().y).abs() <= 0.5);
                let before = rows.fixture.editor.document().clone();
                let revision = rows.fixture.editor.revision();
                let point = toggle.rect.center();
                rows.press(point);
                rows.frame(vec![button(point, false)]);
                rows.repeat_layout = false;
                rows.frame(vec![]);
                let text = rows.entity()["icon"]["layers"][0]["text"].clone();
                assert_eq!(text["linkToAchievementValue"], true);
                assert_eq!(text["content"], "My custom text");
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                assert!(rows.ctx.read_response(toggle_id).unwrap().enabled());
                let linked = rows.fixture.editor.document().clone();
                let field_point = field.rect.center();
                rows.press(field_point);
                rows.frame(vec![
                    button(field_point, false),
                    egui::Event::Text("Do not edit".into()),
                ]);
                assert_eq!(rows.fixture.editor.document(), &linked);
                rows.fixture.undo();
                assert_eq!(rows.fixture.editor.document(), &before);
                rows.fixture.redo();
                assert_eq!(rows.fixture.editor.document(), &linked);
                rows.ctx.memory_mut(|memory| {
                    memory.request_focus(crate::text_content::control_id(&id, "source"))
                });
                rows.repeat_layout = true;
                rows.frame(vec![egui::Event::Key {
                    key: egui::Key::Space,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                rows.repeat_layout = false;
                rows.frame(vec![]);
                assert_eq!(
                    rows.entity()["icon"]["layers"][0]["text"],
                    before
                        .entity(EntityKind::Achievement, &rows.achievement)
                        .unwrap()["icon"]["layers"][0]["text"]
                );
                rows.frame(vec![]);
                rows.press(field_point);
                rows.frame(vec![
                    button(field_point, false),
                    egui::Event::Text(" edited ".into()),
                ]);
                assert!(
                    rows.entity()["icon"]["layers"][0]["text"]["content"]
                        .as_str()
                        .unwrap()
                        .contains(" edited "),
                    "{kind}/{width}/{passes}: unlinking must restore editing"
                );
                let before_disabled = rows.fixture.editor.document().clone();
                rows.input_frame(vec![], false, true);
                rows.input_frame(
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                    false,
                    true,
                );
                rows.input_frame(vec![button(point, false)], false, true);
                assert_eq!(rows.fixture.editor.document(), &before_disabled);
            }
        }
    }
}

#[test]
fn text_content_source_buttons_fit_narrow_rows_at_larger_fonts_and_scales() {
    for width in [150.0, 220.0, 420.0] {
        for font in [13.0, 24.0] {
            for scale in [1.0, 1.5, 2.0] {
                let ctx = egui::Context::default();
                configure_test_fonts(&ctx);
                ctx.set_pixels_per_point(scale);
                let preferences = Preferences {
                    normal_size: font,
                    ..Default::default()
                };
                let mut text = json!({"content":"Custom text"});
                let original = text.clone();
                for _ in 0..2 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 200.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            theme::apply(ui.ctx(), &preferences);
                            ui.set_style(ui.ctx().global_style());
                            ui.set_width(width);
                            crate::text_content::show(ui, "sample", &mut text);
                        },
                    );
                    output.textures_delta.clear();
                }
                assert_eq!(text, original, "Viewing old recipes must not add a flag");
                let source = ctx
                    .read_response(crate::text_content::control_id("sample", "source"))
                    .unwrap()
                    .rect;
                let toggle = ctx
                    .read_response(crate::text_content::control_id("sample", "link"))
                    .unwrap()
                    .rect;
                let field = ctx
                    .read_response(crate::text_content::control_id("sample", "content"))
                    .unwrap()
                    .rect;
                assert!(field.width() >= 20.0);
                assert!(
                    toggle.right() <= width + 0.5,
                    "{width}/{font}/{scale}: {field:?} {toggle:?}"
                );
                assert!((field.right() - source.left()).abs() <= 0.5);
                assert!((source.right() - toggle.left()).abs() <= 0.5);
                assert!((field.center().y - source.center().y).abs() <= 0.5);
                assert!((field.center().y - toggle.center().y).abs() <= 0.5);
                assert_eq!(source.width(), source.height());
                assert_eq!(toggle.width(), toggle.height());
            }
        }
    }
}

#[test]
#[ignore = "Native text link toggle preview; external screenshot output"]
fn native_text_content_link_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        fixture: Fixture,
        desktops: [Desktop; 2],
        layers: [Vec<Value>; 2],
        achievement: Value,
        frames: usize,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.fixture.preferences());
            ui.set_style(ui.ctx().global_style());
            ui.columns(2, |columns| {
                for (index, ui) in columns.iter_mut().enumerate() {
                    ui.heading(if index == 0 {
                        "Text · link off"
                    } else {
                        "3D text · link on"
                    });
                    ui.push_id(index, |ui| {
                        self.desktops[index].layer_properties(
                            ui,
                            &mut self.fixture,
                            &mut self.layers[index],
                            &mut self.achievement,
                        );
                    });
                }
            });
            self.frames += 1;
            if self.frames == 8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            if let Some(image) = ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            }) {
                let pixels: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    crate::test_output::root().join("text-content-link.png"),
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            assert!(self.frames < 120, "Text link screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    eframe::run_native(
        "Achimeld text link",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([820.0, 740.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(|cc| {
            configure_test_fonts(&cc.egui_ctx);
            let fixture = Fixture::new();
            let achievement =
                fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
            let mut desktops = [Desktop::default(), Desktop::default()];
            let layers = ["Text", "Text3d"].map(|kind| {
                let mut layer = layer_palette::new_layer(kind);
                layer["text"]["content"] = json!("My custom text");
                layer["text"]["linkToAchievementValue"] = json!(kind == "Text3d");
                vec![layer]
            });
            for index in 0..2 {
                desktops[index].selected_layer = layers[index][0]["id"].as_str().map(str::to_owned);
            }
            Ok(Box::new(Window {
                fixture,
                desktops,
                layers,
                achievement,
                frames: 0,
            }))
        }),
    )
    .unwrap();
}
