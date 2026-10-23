use super::*;

#[derive(Default)]
struct Library {
    editor: achimeld_application::Editor,
    gradients: Vec<GradientPreset>,
    fail_delete: bool,
    fail_restore: bool,
}
impl EditorPort for Library {
    fn preferences(&self) -> &achimeld_application::Preferences {
        self.editor.preferences()
    }
    fn editor(&self) -> &achimeld_application::Editor {
        &self.editor
    }
    fn execute(
        &mut self,
        label: &str,
        command: achimeld_application::Command,
        group: Option<&str>,
    ) -> Result<(), String> {
        self.editor.execute(label, command, group)
    }
    fn finish_group(&mut self) {
        self.editor.finish_group();
    }
    fn undo(&mut self) {
        self.editor.undo();
    }
    fn redo(&mut self) {
        self.editor.redo();
    }
    fn request(&mut self, _: UiRequest) {}
    fn status(&self) -> &str {
        "Ready"
    }
    fn preview(&mut self, _: &str) -> Option<achimeld_application::Preview> {
        None
    }
    fn gradients(&self) -> &[GradientPreset] {
        &self.gradients
    }
    fn save_gradient(&mut self, gradient: GradientPreset) -> Result<(), String> {
        gradient.validate()?;
        if let Some(old) = self.gradients.iter_mut().find(|g| g.id == gradient.id) {
            *old = gradient;
        } else {
            self.gradients.push(gradient);
        }
        Ok(())
    }
    fn delete_gradient(&mut self, id: &str) -> Result<(), String> {
        self.gradients.retain(|g| g.id != id);
        Ok(())
    }
    fn delete_gradients(&mut self, ids: &[String]) -> Result<(), String> {
        if self.fail_delete {
            return Err("Library write failed".into());
        }
        self.gradients.retain(|g| !ids.contains(&g.id));
        Ok(())
    }
    fn restore_gradients(&mut self, gradients: Vec<GradientPreset>) -> Result<(), String> {
        if self.fail_restore {
            return Err("Library write failed".into());
        }
        self.gradients.extend(gradients);
        Ok(())
    }
}

fn library(count: usize) -> Library {
    let mut library = Library::default();
    for i in 0..count {
        let mut explorer = Explorer::default();
        explorer.new_gradient();
        explorer.draft.as_mut().unwrap().name = format!("Gradient {i}");
        assert!(explorer.save(&mut library));
    }
    library
}

#[test]
fn gradient_tiles_add_subtract_and_batch_delete_and_restore_without_partial_changes() {
    let mut library = library(5);
    let original = library.gradients.clone();
    let mut explorer = Explorer::default();
    explorer.choose(&mut library, original[0].clone(), false, false);
    explorer.choose(&mut library, original[3].clone(), true, false);
    explorer.choose(&mut library, original[1].clone(), true, false);
    assert_eq!(
        explorer.selection,
        [0, 1, 3].map(|i| original[i].id.clone()).into()
    );
    explorer.choose(&mut library, original[3].clone(), false, true);
    assert_eq!(
        explorer.selection,
        [0, 1].map(|i| original[i].id.clone()).into()
    );
    assert_eq!(explorer.selected.as_deref(), Some(original[1].id.as_str()));
    explorer.choose(&mut library, original[4].clone(), false, false);
    explorer.choose(&mut library, original[2].clone(), true, false);
    assert_eq!(
        explorer.selection,
        [2, 4].map(|i| original[i].id.clone()).into()
    );
    library.fail_delete = true;
    explorer.delete_selection(&mut library);
    assert_eq!(library.gradients, original);
    assert_eq!(explorer.selection.len(), 2);
    library.fail_delete = false;
    explorer.delete_selection(&mut library);
    assert_eq!(library.gradients, [0, 1, 3].map(|i| original[i].clone()));
    assert!(explorer.selection.is_empty());
    assert_eq!(explorer.removed.len(), 2);
    library.fail_restore = true;
    explorer.undo_delete(&mut library);
    assert_eq!(library.gradients.len(), 3);
    assert_eq!(explorer.removed.len(), 2);
    library.fail_restore = false;
    explorer.undo_delete(&mut library);
    assert_eq!(library.gradients.len(), original.len());
    assert_eq!(explorer.selection.len(), 2);
    assert!(explorer.removed.is_empty());
    explorer.choose(&mut library, original[2].clone(), false, true);
    explorer.choose(&mut library, original[4].clone(), false, true);
    assert!(explorer.selection.is_empty());
    assert!(explorer.draft.is_none());
}

#[test]
fn gradient_tiles_modifier_clicks_and_delete_work_at_multiple_widths_and_scales() {
    for (scale, width, font) in [(1.0, 620.0, 13.0), (1.5, 480.0, 20.0), (2.0, 280.0, 13.0)] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        let mut library = library(5);
        library
            .editor
            .execute(
                "Add",
                achimeld_application::Command::Add(achimeld_application::EntityKind::Achievement),
                None,
            )
            .unwrap();
        let original = library.gradients.clone();
        let project = library.editor.document().clone();
        let mut desktop = Desktop::default();
        desktop.selection = vec![
            project.entities(achimeld_application::EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .into(),
        ];
        desktop.gradient_explorer.open = true;
        let mut explorer = Explorer::default();
        let mut time = 0.0;
        let mut frame = |explorer: &mut Explorer,
                         library: &mut Library,
                         mut events: Vec<egui::Event>,
                         modifiers| {
            time += 0.1;
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1000.0, 1200.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::theme::apply(
                        ui.ctx(),
                        &achimeld_application::Preferences {
                            normal_size: font,
                            ..Default::default()
                        },
                    );
                    ui.set_style(ui.ctx().global_style());
                    ui.set_width(width);
                    desktop.shortcuts(ui.ctx(), library);
                    explorer.contents(ui, library);
                },
            );
            output.textures_delta.clear();
        };
        frame(&mut explorer, &mut library, vec![], egui::Modifiers::NONE);
        frame(&mut explorer, &mut library, vec![], egui::Modifiers::NONE);
        let first = ctx
            .read_response(tiles::tile_id(&original[0].id))
            .unwrap()
            .rect;
        assert!(first.width() <= width && first.height() > 90.0);
        let second = ctx
            .read_response(tiles::tile_id(&original[1].id))
            .unwrap()
            .rect;
        assert!(second.right() <= width + 8.0);
        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let alt = egui::Modifiers {
            alt: true,
            ..Default::default()
        };
        for (pos, modifiers) in [
            (first.center(), egui::Modifiers::NONE),
            (second.center(), shift),
            (first.center(), alt),
        ] {
            for pressed in [true, false] {
                frame(
                    &mut explorer,
                    &mut library,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers,
                        },
                    ],
                    modifiers,
                );
            }
        }
        assert_eq!(explorer.selection, BTreeSet::from([original[1].id.clone()]));
        // The Alt-clicked tile is no longer selected, but grid focus still owns Delete.
        frame(
            &mut explorer,
            &mut library,
            vec![egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert_eq!(library.gradients, [0, 2, 3, 4].map(|i| original[i].clone()));
        assert_eq!(
            library.editor.document(),
            &project,
            "Delete must never reach the project behind the explorer"
        );
    }
}

#[test]
fn gradient_explorer_saves_to_shared_library_and_keeps_invalid_drafts_repairable() {
    let mut explorer = Explorer::default();
    let mut library = Library::default();
    let before = library.editor.document().clone();
    explorer.new_gradient();
    explorer.draft.as_mut().unwrap().name = "Twilight".into();
    assert!(explorer.save(&mut library));
    let id = library.gradients[0].id.clone();
    explorer.draft.as_mut().unwrap().name = "".into();
    assert!(!explorer.save(&mut library));
    assert_eq!(library.gradients[0].name, "Twilight");
    explorer.draft.as_mut().unwrap().name = "Dawn".into();
    assert!(explorer.save(&mut library));
    assert_eq!(library.gradients.len(), 1);
    assert_eq!(library.gradients[0].id, id);
    assert_eq!(library.gradients[0].name, "Dawn");
    assert_eq!(library.editor.document(), &before);
}

#[test]
#[ignore = "Native gradient explorer and fixed picker capture; external output only"]
fn native_gradient_explorer_picker_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Capture {
        desktop: Desktop,
        library: Library,
        color: egui::ecolor::Hsva,
        frames: usize,
        output: std::path::PathBuf,
    }
    impl eframe::App for Capture {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
            ui.set_style(ui.ctx().global_style());
            self.desktop
                .gradient_explorer_window(ui.ctx(), &mut self.library, true);
            let mut open = true;
            crate::color_picker::window(
                ui,
                egui::Id::new("native-color-editor"),
                "Color stop",
                egui::Rect::from_min_size(egui::pos2(712.0, 55.0), egui::Vec2::ZERO),
                &mut open,
                &mut self.color,
            );
            self.frames += 1;
            if self.frames == 8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            if let Some(image) = ui.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let bytes = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect::<Vec<_>>();
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
            assert!(self.frames < 128, "Gradient screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    let large = std::env::var_os("ACHIMELD_GRADIENT_LARGE").is_some();
    let output = crate::test_output::root().join(if large {
        "gradient-explorer-picker-large.png"
    } else {
        "gradient-explorer-picker.png"
    });
    eframe::run_native(
        "Gradient Explorer preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size(if large {
                    [1800.0, 1200.0]
                } else {
                    [1200.0, 800.0]
                }),
            event_loop_builder: Some(Box::new(|b| {
                b.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::tests::configure_test_fonts(&cc.egui_ctx);
            if large {
                cc.egui_ctx.set_pixels_per_point(1.5);
            }
            let mut library = Library::default();
            for (name, colors) in [
                ("Twilight", ["#FF2454B8", "#FF8D60D1", "#FFF0A37A"]),
                ("Ocean glass", ["#0029BBC3", "#802DDDC9", "#FFE3F7EE"]),
                ("Copper", ["#FF391D24", "#FFC7794D", "#FFFFDEA0"]),
            ] {
                library.gradients.push(GradientPreset {
                    id: achimeld_application::new_id(),
                    name: name.into(),
                    stops: colors
                        .into_iter()
                        .enumerate()
                        .map(|(i, color)| achimeld_application::GradientStop {
                            offset: i as f64 / 2.0,
                            color_hex: color.into(),
                        })
                        .collect(),
                });
            }
            let mut desktop = Desktop::default();
            desktop.gradient_explorer.open = true;
            desktop.gradient_explorer.initialized = true;
            let first = library.gradients[0].clone();
            let last = library.gradients[2].clone();
            desktop
                .gradient_explorer
                .choose(&mut library, first, false, false);
            desktop
                .gradient_explorer
                .choose(&mut library, last, true, false);
            Ok(Box::new(Capture {
                desktop,
                library,
                color: egui::ecolor::Hsva::from_srgba_unmultiplied([143, 96, 96, 255]),
                frames: 0,
                output,
            }))
        }),
    )
    .unwrap();
}
