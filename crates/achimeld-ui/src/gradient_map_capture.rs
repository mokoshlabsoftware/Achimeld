use super::*;
use achimeld_application::particles::ParticleSettings;

#[test]
#[ignore = "Native color-map screenshot; writes to the external build output"]
fn native_particle_color_map_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        settings: ParticleSettings,
        fixture: Fixture,
        achievement: String,
        texture: egui::TextureHandle,
        rendered: Value,
        output: std::path::PathBuf,
        started: std::time::Instant,
        requested: bool,
        pointer: Option<egui::Pos2>,
        stage: u8,
    }
    impl Window {
        fn update_preview(&mut self) {
            let value = json!(self.settings);
            if self.rendered == value {
                return;
            }
            let mut entity = self
                .fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &self.achievement)
                .unwrap()
                .clone();
            entity["icon"] = json!({"backgroundColorHex":"#FF1C1E24","layers":[{"id":self.achievement,"kind":"Particle","particles":value}]});
            self.fixture
                .editor
                .execute(
                    "Particles",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: self.achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            let image = render(
                self.fixture.editor.document(),
                &self.achievement,
                Some((256, 256)),
                &mut NoAssets,
            )
            .unwrap()
            .achieved;
            self.texture.set(
                egui::ColorImage::from_rgba_unmultiplied([256, 256], image.as_raw()),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            self.rendered = value;
        }
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, raw: &mut egui::RawInput) {
            raw.focused = true;
            if let Some(pos) = self.pointer {
                raw.events.push(egui::Event::PointerMoved(pos));
                if self.stage == 2 || self.stage == 3 {
                    raw.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: self.stage == 2,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                self.stage = self.stage.saturating_add(1);
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), &Preferences::default());
            ui.set_style(ui.ctx().global_style());
            ui.heading("Particle color map");
            ui.weak("Each particle chooses a top-to-bottom slice. Move the guide left or right to test a slice.");
            ui.horizontal_top(|ui| {
                let map_width = (ui.available_width() * 0.62).max(180.0);
                let preview_width = (ui.available_width() - map_width - 20.0).max(120.0);
                ui.vertical(|ui| {
                    ui.set_width(map_width);
                    let map = self.settings.color_map.as_mut().unwrap();
                    crate::gradient_map::presets(ui, None, map);
                    let rect = crate::gradient_map::editor(ui, map).rect.shrink(5.0);
                    self.pointer = Some(egui::pos2(
                        egui::lerp(rect.x_range(), 0.4),
                        egui::lerp(rect.y_range(), 0.4),
                    ));
                    ui.weak("All points can move · Shift-click adds · Alt-click removes");
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(preview_width);
                    ui.strong("Effect preview");
                    crate::particles::seed_input(ui, &mut self.settings.seed);
                    self.update_preview();
                    ui.add(egui::Image::new((
                        self.texture.id(),
                        egui::Vec2::splat(preview_width.min(260.0)),
                    )));
                });
            });
            if !self.requested && self.stage >= 5 && self.started.elapsed().as_secs_f32() > 0.8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                self.requested = true;
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
                let pixels: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    &self.output,
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            assert!(
                self.started.elapsed().as_secs() < 20,
                "Color-map screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    let large = std::env::var_os("ACHIMELD_PARTICLE_LARGE").is_some();
    let output = crate::test_output::root().join(if large {
        "particle-color-map-large.png"
    } else {
        "particle-color-map.png"
    });
    eframe::run_native(
        "Achimeld particle color map",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size(if large {
                    [1000.0, 800.0]
                } else {
                    [800.0, 650.0]
                }),
            event_loop_builder: Some(Box::new(|b| {
                b.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            if large {
                cc.egui_ctx.set_pixels_per_point(1.5);
            }
            let mut map = achimeld_application::gradient_map::presets().remove(1).1;
            map.points.push(ColorPoint {
                x: 0.4,
                y: 0.4,
                color_hex: "#FFF14DAB".into(),
                power: 1.8,
                range_x: 0.4,
                range_y: 0.22,
            });
            let settings = ParticleSettings {
                color_map: Some(map),
                shape: "Star".into(),
                vertex_count: 5,
                inner_radius: 0.45,
                count: 32,
                size_min: 40.0,
                size_max: 100.0,
                softness: 0.05,
                seed: 42,
                ..Default::default()
            };
            let fixture = Fixture::new();
            let achievement = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let texture = cc.egui_ctx.load_texture(
                "particle-preview",
                egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]),
                egui::TextureOptions::LINEAR,
            );
            let mut window = Window {
                settings,
                fixture,
                achievement,
                texture,
                rendered: Value::Null,
                output,
                started: std::time::Instant::now(),
                requested: false,
                pointer: None,
                stage: 0,
            };
            window.update_preview();
            Ok(Box::new(window))
        }),
    )
    .unwrap();
}
