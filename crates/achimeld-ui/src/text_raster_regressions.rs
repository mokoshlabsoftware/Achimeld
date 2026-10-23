use egui::{Color32, FontId, Pos2, Rect, Vec2};

#[test]
fn glyph_atlas_has_separate_rgb_coverage_at_each_display_scale() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let ctx = egui::Context::default();
        crate::tests::configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(1.0);
        let mut output = ctx.run_ui(input, |ui| {
            crate::theme::apply(ui.ctx(), &Default::default());
            ui.label("Minimum Il1 0123 — Zażółć gęślą jaźń");
            ui.label(
                egui::RichText::new("Bold glyphs").family(egui::FontFamily::Name(
                    crate::typography::BOLD_FAMILY.into(),
                )),
            );
        });
        let atlas = output
            .textures_delta
            .set
            .iter()
            .find_map(|(id, deltas)| {
                if *id == egui::TextureId::Managed(0) {
                    let delta = deltas.first()?;
                    let egui::ImageData::Color(image) = &delta.image;
                    Some(image)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            atlas
                .pixels
                .iter()
                .filter(|p| p.r().abs_diff(p.b()) > 12)
                .count()
                > 30,
            "Missing independent RGB edge coverage at {scale}x"
        );
        assert!(atlas.pixels.contains(&Color32::WHITE));
        assert!(atlas.pixels.contains(&Color32::TRANSPARENT));
        output.textures_delta.clear();
    }
}

/// Exercises the actual Glow shader and blend state, including state after an
/// ordinary image and the following font-atlas mesh. No Steam or project I/O.
#[test]
#[ignore = "Native GPU regression and screenshot; set ACHIMELD_SCREENSHOT_TO under external build output"]
fn native_rgb_subpixel_text() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;

    struct Window {
        output: std::path::PathBuf,
        frame: usize,
        sample: egui::TextureHandle,
        scale: f32,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.frame += 1;
            let painter = ui.painter();
            painter.rect_filled(ui.max_rect(), 0, Color32::from_gray(128));
            for (top, background, ink) in [
                (0.0, Color32::WHITE, Color32::BLACK),
                (110.0, Color32::BLACK, Color32::WHITE),
            ] {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(0.0, top), Vec2::new(800.0, 110.0)),
                    0,
                    background,
                );
                for (line, size) in [(0.0, 12.0), (25.0, 14.0), (52.0, 18.0)] {
                    painter.text(
                        Pos2::new(16.25, top + 8.0 + line),
                        egui::Align2::LEFT_TOP,
                        "Achimeld · Minimum Il1 0123 · Zażółć gęślą jaźń",
                        FontId::proportional(size),
                        ink,
                    );
                }
            }
            // An ordinary colored texture must retain its own channels and alpha.
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(0.0, 225.0), Vec2::new(250.0, 75.0)),
                0,
                Color32::BLACK,
            );
            painter.image(
                self.sample.id(),
                Rect::from_min_size(Pos2::new(10.0, 235.0), Vec2::splat(40.0)),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(60.0, 235.0), Vec2::splat(40.0)),
                0,
                Color32::from_gray(90),
            );
            painter.text(
                Pos2::new(120.25, 235.0),
                egui::Align2::LEFT_TOP,
                "RGB after image",
                FontId::proportional(14.0),
                Color32::WHITE,
            );

            // Exercise bundled CJK fallbacks in the actual native renderer,
            // including bold UI labels and mixed scripts without language packs.
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(0.0, 300.0), Vec2::new(800.0, 220.0)),
                0,
                Color32::from_gray(24),
            );
            for (row, sample) in [
                "한국어 · 업적 통계 설정 · 한글",
                "简体中文 · 成就 统计 设置",
                "繁體中文 · 成就 統計 設定",
                "日本語 · 実績 統計 設定 · ひらがな カタカナ",
            ]
            .into_iter()
            .enumerate()
            {
                let top = 310.0 + 50.0 * row as f32;
                for (left, family) in [
                    (16.0, egui::FontFamily::Proportional),
                    (
                        410.0,
                        egui::FontFamily::Name(crate::typography::BOLD_FAMILY.into()),
                    ),
                ] {
                    painter.text(
                        Pos2::new(left, top),
                        egui::Align2::LEFT_TOP,
                        sample,
                        FontId::new(16.0, family),
                        Color32::WHITE,
                    );
                }
            }

            if self.frame == 6 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            for event in ui.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let sample = |x: f32, y: f32| {
                        image[((x * self.scale) as usize, (y * self.scale) as usize)]
                    };
                    let actual = sample(30.0, 255.0).to_array();
                    for (actual, expected) in actual.into_iter().zip([30_u8, 60, 90, 255]) {
                        assert!(
                            actual.abs_diff(expected) <= 2,
                            "Image blend changed: {actual} != {expected}"
                        );
                    }
                    let gray = sample(80.0, 255.0);
                    assert!(gray.r().abs_diff(gray.g()) <= 1 && gray.g().abs_diff(gray.b()) <= 1);
                    assert!(gray.r().abs_diff(90) <= 2, "Solid atlas geometry changed");
                    for top in [0.0, 110.0] {
                        let mut colored_edges = 0;
                        for y in ((top + 5.0) * self.scale) as usize
                            ..((top + 85.0) * self.scale) as usize
                        {
                            for x in (10.0 * self.scale) as usize..(450.0 * self.scale) as usize {
                                let p = image[(x, y)];
                                colored_edges += usize::from(p.r().abs_diff(p.b()) > 8);
                            }
                        }
                        assert!(
                            colored_edges > 50,
                            "Grayscale text on background at y={top}"
                        );
                    }
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
            assert!(self.frame < 300, "GPU screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").unwrap(),
    ))
    .unwrap();
    assert!(output.starts_with(std::path::absolute(crate::test_output::root()).unwrap()));
    let scale = std::env::var("ACHIMELD_TEXT_SCALE")
        .ok()
        .map(|s| s.parse::<f32>().unwrap())
        .unwrap_or(1.0);
    assert!([1.0, 1.25, 1.5, 2.0].contains(&scale));
    eframe::run_native(
        "Achimeld RGB text fixture",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800.0 * scale, 520.0 * scale])
                .with_decorations(false),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::tests::configure_test_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(scale);
            let sample = cc.egui_ctx.load_texture(
                "ordinary image",
                egui::ColorImage::filled([1, 1], Color32::from_rgba_premultiplied(30, 60, 90, 128)),
                egui::TextureOptions::NEAREST,
            );
            Ok(Box::new(Window {
                output,
                frame: 0,
                sample,
                scale,
            }))
        }),
    )
    .unwrap();
}
