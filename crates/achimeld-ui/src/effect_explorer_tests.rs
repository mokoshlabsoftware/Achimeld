use super::*;
use serde_json::json;
#[derive(Default)]
struct Library {
    editor: achimeld_application::Editor,
    presets: Vec<EffectPreset>,
    preview: Option<achimeld_application::Preview>,
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
    fn effect_preview(&mut self, _: &EffectPreset) -> Option<achimeld_application::Preview> {
        self.preview.clone()
    }
    fn effect_presets(&self) -> &[EffectPreset] {
        &self.presets
    }
    fn save_effect_preset(&mut self, preset: EffectPreset) -> Result<(), String> {
        preset.validate()?;
        if let Some(old) = self.presets.iter_mut().find(|p| p.id == preset.id) {
            *old = preset;
        } else {
            self.presets.push(preset);
        }
        Ok(())
    }
    fn delete_effect_preset(&mut self, id: &str) -> Result<(), String> {
        self.presets.retain(|p| p.id != id);
        Ok(())
    }
}

#[test]
#[ignore = "Native effect explorer screenshots; external output only"]
fn native_procedural_explorer_screenshots() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct NoAssets;
    impl achimeld_imaging::AssetSource for NoAssets {
        fn image(&mut self, _: &Value) -> Result<std::sync::Arc<image::RgbaImage>, String> {
            Err("No artwork required".into())
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<achimeld_imaging::Font>> {
            None
        }
    }
    struct Capture {
        desktop: Desktop,
        library: Library,
        scenes: Vec<(EffectPreset, achimeld_application::Preview)>,
        index: usize,
        frames: usize,
    }
    impl Capture {
        fn select(&mut self) {
            let (preset, preview) = &self.scenes[self.index];
            self.library.preview = Some(preview.clone());
            self.desktop.effect_explorer = Explorer {
                open: true,
                kind: Some(preset.kind),
                selected: Some(preset.id.clone()),
                draft: Some(preset.clone()),
                ..Default::default()
            };
        }
    }
    impl eframe::App for Capture {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
            ui.set_style(ui.ctx().global_style());
            self.desktop
                .effect_explorer_window(ui.ctx(), &mut self.library, true);
            self.frames += 1;
            if self.frames == 8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            if let Some(image) = ui.input(|i| {
                i.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let pixels: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                let name = format!(
                    "{}-explorer.png",
                    self.scenes[self.index].0.kind.layer_kind().to_lowercase()
                );
                image::save_buffer(
                    crate::test_output::root().join(name),
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                self.index += 1;
                if self.index == self.scenes.len() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.frames = 0;
                    self.select();
                }
            }
            assert!(self.frames < 300, "Screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    let mut library = Library::default();
    let mut scenes = Vec::new();
    library
        .editor
        .execute(
            "Preview",
            achimeld_application::Command::Add(achimeld_application::EntityKind::Achievement),
            None,
        )
        .unwrap();
    let id = library
        .editor
        .document()
        .entities(achimeld_application::EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for kind in [EffectKind::Explosion, EffectKind::Particle] {
        let layer = crate::layer_palette::new_layer(kind.layer_kind());
        let preset =
            EffectPreset::from_layer(kind, &layer, format!("{} example", kind.layer_kind()))
                .unwrap();
        let mut project = library.editor.document().project().clone();
        project["achievements"][0]["icon"] = json!({"layers":[preset.layer]});
        let document = achimeld_application::Document::from_parts(
            project,
            library.editor.document().catalog().clone(),
        )
        .unwrap();
        let image = achimeld_imaging::render(&document, &id, Some((256, 256)), &mut NoAssets)
            .unwrap()
            .achieved;
        let preview = achimeld_application::Preview {
            width: 256,
            height: 256,
            source_size: [256, 256],
            rgba: image.into_raw().into(),
            fingerprint: kind.layer_kind().into(),
        };
        library.presets.push(preset.clone());
        scenes.push((preset, preview));
    }
    eframe::run_native(
        "Effect explorers",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([1000.0, 820.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::configure_fonts(&cc.egui_ctx, None);
            let mut app = Capture {
                desktop: Desktop::default(),
                library,
                scenes,
                index: 0,
                frames: 0,
            };
            app.select();
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
#[test]
fn procedural_explorers_save_embedded_gradients_separately_and_keep_invalid_drafts_repairable() {
    let mut explorer = Explorer::default();
    let mut library = Library::default();
    let before = library.editor.document().clone();
    for kind in [EffectKind::Explosion, EffectKind::Particle] {
        explorer.kind = Some(kind);
        explorer.create(kind, None);
        let draft = explorer.draft.as_mut().unwrap();
        draft.name = format!("Custom {}", kind.layer_kind());
        draft.layer[kind.settings_key()]["gradient"]["stops"][0]["colorHex"] = json!("#50123456");
        let saved = draft.clone();
        assert!(explorer.save(&mut library));
        explorer.draft.as_mut().unwrap().name.clear();
        assert!(!explorer.save(&mut library));
        assert!(library.presets.contains(&saved));
        explorer.draft.as_mut().unwrap().name = "Repaired".into();
        assert!(explorer.save(&mut library));
        assert_eq!(library.presets.iter().filter(|p| p.kind == kind).count(), 1);
        assert_eq!(
            library.presets.last().unwrap().layer[kind.settings_key()]["gradient"]["stops"][0]["colorHex"],
            "#50123456"
        );
    }
    assert_eq!(library.editor.document(), &before);
}
#[test]
fn procedural_explorers_render_compact_panels_without_changing_drafts() {
    for kind in [EffectKind::Explosion, EffectKind::Particle] {
        for (width, font) in [(360.0, 13.0), (440.0, 22.0), (660.0, 13.0)] {
            let ctx = egui::Context::default();
            crate::configure_fonts(&ctx, Default::default());
            let mut library = Library::default();
            let mut explorer = Explorer {
                kind: Some(kind),
                ..Default::default()
            };
            explorer.create(kind, None);
            let before = explorer.draft.clone();
            let mut rendered = 0.0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 5000.0),
                    )),
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
                    explorer.contents(ui, &mut library);
                    rendered = ui.min_rect().width();
                },
            );
            output.textures_delta.clear();
            assert!(
                rendered <= width + 1.0,
                "{kind:?} width {width} became {rendered}"
            );
            assert_eq!(explorer.draft, before);
            assert!(library.presets.is_empty());
        }
    }
}
