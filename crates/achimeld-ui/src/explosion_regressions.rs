use super::*;

fn text_shapes(shape: &egui::Shape, text: &mut String) {
    match shape {
        egui::Shape::Text(s) => {
            text.push_str(s.galley.text());
            text.push('\n');
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, text);
            }
        }
        _ => {}
    }
}
#[test]
fn explosion_inspector_preserves_sparse_records_and_switches_to_a_count_range() {
    let mut fixture = Fixture::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut layer = json!({"id":new_id(),"kind":"Explosion"});
    let original = layer.clone();
    let button_id = egui::Id::new(("explosion-count-range", layer["id"].as_str().unwrap()));
    let mut frame = |events, width| {
        let mut text = String::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 1800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                ui.set_style(ui.ctx().global_style());
                crate::explosion::properties(ui, &mut fixture, &mut layer, "test");
            },
        );
        for shape in &output.shapes {
            text_shapes(&shape.shape, &mut text);
        }
        output.textures_delta.clear();
        text
    };
    let labels = frame(vec![], 440.0);
    assert!(
        labels
            .lines()
            .any(|text| crate::fields::label_matches(text, "Overall scale"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Aspect ratio"))
    );
    for label in [
        "Base brightness",
        "B. Variation",
        "Shape X",
        "Shape Y",
        "D. → Ellipse",
        "Radial blur",
        "Angular blur",
        "Fill power",
        "Fill stretch",
    ] {
        assert!(
            labels
                .lines()
                .any(|text| crate::fields::label_matches(text, label)),
            "Missing explosion control: {label}"
        );
    }
    assert!(
        labels
            .lines()
            .any(|text| crate::fields::label_matches(text, "Shape"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "T. Height"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "B. Height"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Regularity"))
    );
    let point = ctx.read_response(button_id).unwrap().rect.center();
    for pressed in [true, false] {
        frame(
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            440.0,
        );
    }
    drop(frame);
    assert_eq!(layer["explosion"]["countMin"], 9);
    assert_eq!(layer["explosion"]["count"], 9);
    assert_eq!(original["explosion"], Value::Null);
    achimeld_application::explosion::ExplosionSettings::from_value(&layer["explosion"]).unwrap();
    // A fresh sparse inspector at narrow width must be a read-only operation.
    let mut sparse = original.clone();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(220.0, 1800.0),
            )),
            ..Default::default()
        },
        |ui| crate::explosion::properties(ui, &mut fixture, &mut sparse, "test"),
    );
    output.textures_delta.clear();
    assert_eq!(sparse, original);
}

#[test]
fn explosion_bottom_curve_menu_edits_explicitly_and_respects_disabled_controls() {
    let mut fixture = Fixture::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut layer = crate::layer_palette::new_layer("Explosion");
    let id = layer["id"].as_str().unwrap().to_owned();
    let button = egui::Id::new(("explosion-bottom-curve", id.as_str()));
    let option = egui::Id::new(("explosion-bottom-curve-option", id.as_str(), "Logarithmic"));
    let original = layer.clone();
    let mut frame = |events, enabled| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(440.0, 1800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                ui.set_style(ui.ctx().global_style());
                ui.add_enabled_ui(enabled, |ui| {
                    crate::explosion::properties(ui, &mut fixture, &mut layer, "fill")
                });
            },
        );
        output.textures_delta.clear();
    };
    frame(vec![], true);
    frame(vec![], true);
    let click = |point, pressed| {
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let point = ctx.read_response(button).unwrap().rect.center();
    for pressed in [true, false] {
        frame(click(point, pressed), false);
    }
    assert!(!egui::Popup::is_id_open(
        &ctx,
        egui::Popup::default_response_id(&ctx.read_response(button).unwrap())
    ));
    frame(vec![], true);
    let point = ctx.read_response(button).unwrap().rect.center();
    for pressed in [true, false] {
        frame(click(point, pressed), true);
    }
    frame(vec![], true);
    assert!(
        egui::Popup::is_id_open(
            &ctx,
            egui::Popup::default_response_id(&ctx.read_response(button).unwrap())
        ),
        "Curve picker did not open"
    );
    let point = ctx
        .read_response(option)
        .expect("curve menu option")
        .rect
        .center();
    for pressed in [true, false] {
        frame(click(point, pressed), true);
    }
    frame(vec![], true);
    drop(frame);
    assert_eq!(layer["explosion"]["bottomCurve"], "Logarithmic");
    let mut expected = original;
    expected["explosion"]["bottomCurve"] = json!("Logarithmic");
    assert_eq!(layer, expected);
    achimeld_application::explosion::ExplosionSettings::from_value(&layer["explosion"]).unwrap();
}

#[test]
fn explosion_legacy_shape_conversion_requires_an_explicit_edit() {
    let mut fixture = Fixture::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut layer = json!({"id":new_id(), "kind":"Explosion", "explosion":achimeld_application::explosion::ExplosionSettings::default()});
    layer["explosion"]["out"]["flip"] = json!(true);
    let original = layer.clone();
    let button_id = egui::Id::new(("explosion-use-deltoid", layer["id"].as_str().unwrap()));
    let mut frame = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(440.0, 1800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| crate::explosion::properties(ui, &mut fixture, &mut layer, "legacy"),
        );
        output.textures_delta.clear();
    };
    frame(vec![]);
    let point = ctx.read_response(button_id).unwrap().rect.center();
    drop(frame);
    assert_eq!(layer, original);
    for pressed in [true, false] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(440.0, 1800.0),
                )),
                events: vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| crate::explosion::properties(ui, &mut fixture, &mut layer, "legacy"),
        );
        output.textures_delta.clear();
    }
    assert!(layer["explosion"]["deltoid"].is_object());
    assert_eq!(layer["explosion"]["out"], original["explosion"]["out"]);
    achimeld_application::explosion::ExplosionSettings::from_value(&layer["explosion"]).unwrap();
}

#[test]
fn explosion_uses_shared_single_value_offset_rotation_row() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let mut achievement = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let mut layers = vec![crate::layer_palette::new_layer("Explosion")];
    desktop.selected_layer = layers[0]["id"].as_str().map(str::to_owned);
    desktop.selection = vec![achievement["id"].as_str().unwrap().to_owned()];
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(480.0, 2400.0),
            )),
            ..Default::default()
        },
        |ui| {
            theme::apply(ui.ctx(), &Preferences::default());
            desktop.layer_properties(ui, &mut fixture, &mut layers, &mut achievement);
        },
    );
    let mut labels = String::new();
    for shape in &output.shapes {
        text_shapes(&shape.shape, &mut labels);
    }
    output.textures_delta.clear();
    assert!(
        labels
            .lines()
            .any(|text| crate::fields::label_matches(text, "Offset X"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Offset Y"))
            && labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Angle"))
    );
    assert!(!labels.lines().any(|label| label == "Scale"));
    assert_eq!(layers[0]["offsetX"], 0.0);
    assert_eq!(layers[0]["offsetY"], 0.0);
    assert_eq!(layers[0]["rotation"], 0.0);
}

#[test]
fn explosion_deltoid_shapes_fit_narrow_inspectors_without_dirtying_recipes() {
    fn overflowing_text(shape: &egui::Shape, width: f32, labels: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) if text.pos.x + text.galley.size().x > width + 1.0 => {
                labels.push(format!(
                    "{} ({:.1})",
                    text.galley.text(),
                    text.pos.x + text.galley.size().x
                ));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    overflowing_text(shape, width, labels);
                }
            }
            _ => {}
        }
    }
    for (width, font_size) in [
        (180.0, 14.0),
        (180.0, 22.0),
        (220.0, 14.0),
        (220.0, 22.0),
        (320.0, 14.0),
        (320.0, 22.0),
        (600.0, 22.0),
    ] {
        let mut fixture = Fixture::new();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.global_style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size = font_size;
            }
        });
        let mut layer = crate::layer_palette::new_layer("Explosion");
        layer["explosion"]["countMin"] = json!(1);
        layer["explosion"]["count"] = json!(512);
        layer["explosion"]["deltoid"]["scale"] = json!(1.5);
        layer["explosion"]["deltoid"]["scaleMin"] = json!(0.5);
        layer["explosion"]["deltoid"]["scaleMax"] = json!(1.5);
        let before = layer.clone();
        let mut rendered_width = 0.0;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 3000.0),
                )),
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                crate::explosion::properties(ui, &mut fixture, &mut layer, "test");
                rendered_width = ui.min_rect().width();
            },
        );
        let mut labels = String::new();
        let mut overflow = Vec::new();
        for shape in &output.shapes {
            text_shapes(&shape.shape, &mut labels);
            overflowing_text(&shape.shape, width, &mut overflow);
        }
        output.textures_delta.clear();
        assert!(
            rendered_width <= width + 1.0,
            "Explosion controls overflow at {width}: {rendered_width}; {overflow:?}"
        );
        assert!(
            labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Shape"))
                && labels
                    .lines()
                    .any(|text| crate::fields::label_matches(text, "D. → Ellipse"))
        );
        assert!(
            labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Shape X"))
                && labels
                    .lines()
                    .any(|text| crate::fields::label_matches(text, "Shape Y"))
        );
        assert!(
            labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "T. Height"))
                && labels
                    .lines()
                    .any(|text| crate::fields::label_matches(text, "B. Height"))
        );
        assert!(
            !labels
                .lines()
                .any(|text| crate::fields::label_matches(text, "Flip triangle"))
                && !labels
                    .lines()
                    .any(|text| crate::fields::label_matches(text, "Triangle pairs"))
        );
        assert_eq!(
            labels
                .lines()
                .filter(|text| crate::fields::label_matches(text, "G. Scale"))
                .count(),
            1
        );
        assert_eq!(
            labels
                .lines()
                .filter(|text| crate::fields::label_matches(text, "R. Scale"))
                .count(),
            1
        );
        assert_eq!(layer, before);
    }
}

#[test]
fn shape_keeps_double_sliders_and_starts_flush_with_the_main_panel() {
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut fixture = Fixture::new();
    let mut layer = crate::layer_palette::new_layer("Explosion");
    let before = layer.clone();
    let mut left = 0.0;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        theme::apply(ui.ctx(), &Preferences::default());
        ui.set_style(ui.ctx().global_style());
        ui.set_width(440.0);
        left = ui.available_rect_before_wrap().left();
        crate::explosion::body_properties(ui, &mut fixture, &mut layer, "shape-layout");
    });
    output.textures_delta.clear();
    let label = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(text)
                if crate::fields::label_matches(text.galley.text(), "B. Variation") =>
            {
                Some(text.pos.x)
            }
            _ => None,
        })
        .expect("Original Shape brightness range label");
    assert!(
        (label - left).abs() < 1.0,
        "Shape is indented: {label} vs {left}"
    );
    let handles = output
        .shapes
        .iter()
        .filter(|s| matches!(&s.shape, egui::Shape::Circle(c) if c.radius == 5.0))
        .count();
    assert!(
        handles >= 14,
        "Shape retains the seven original double-ended ranges"
    );
    assert_eq!(layer, before);
}

#[test]
#[ignore = "Native explosion inspector screenshot; writes outside source"]
fn native_explosion_inspector_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        achievement: Value,
        layers: Vec<Value>,
        texture: egui::TextureHandle,
        started: std::time::Instant,
        requested: bool,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), &Preferences::default());
            ui.set_style(ui.ctx().global_style());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.heading("Explosion");
                    if let Some(id) = crate::layer_icons::texture(ui.ctx(), "Explosion") {
                        ui.add(egui::Image::new((id, egui::vec2(32.0, 32.0))));
                    }
                    ui.add(egui::Image::new((
                        self.texture.id(),
                        egui::vec2(290.0, 290.0),
                    )));
                    ui.weak("Symmetric deltoids → polar → gradient");
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    self.desktop.layer_properties(
                        ui,
                        &mut self.fixture,
                        &mut self.layers,
                        &mut self.achievement,
                    );
                });
            });
            if !self.requested && self.started.elapsed().as_secs_f32() > 0.6 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                self.requested = true;
            }
            if let Some(image) = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            }) {
                let pixels: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    crate::test_output::root().join("explosion-inspector.png"),
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
                "Explosion screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    eframe::run_native(
        "Achimeld explosion",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([720.0, 1700.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(|cc| {
            configure_test_fonts(&cc.egui_ctx);
            let fixture = Fixture::new();
            let mut achievement =
                fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
            let mut layer = crate::layer_palette::new_layer("Explosion");
            layer["explosion"]["bottomPower"] = json!(3.0);
            layer["explosion"]["bottomStretch"] = json!(0.65);
            layer["explosion"]["bottomCurve"] = json!("Logarithmic");
            achievement["icon"] = json!({"layers":[layer]});
            let mut project = fixture.editor.document().project().clone();
            project["achievements"] = json!([achievement]);
            let document = achimeld_application::Document::from_parts(
                project,
                fixture.editor.document().catalog().clone(),
            )
            .unwrap();
            let id = achievement["id"].as_str().unwrap();
            let image = render(&document, id, Some((256, 256)), &mut NoAssets)
                .unwrap()
                .achieved;
            let texture = cc.egui_ctx.load_texture(
                "explosion-preview",
                egui::ColorImage::from_rgba_unmultiplied([256, 256], image.as_raw()),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            let mut desktop = Desktop::default();
            desktop.selection = vec![id.to_owned()];
            desktop.selected_layer = layer["id"].as_str().map(str::to_owned);
            Ok(Box::new(Window {
                desktop,
                fixture,
                achievement,
                layers: vec![layer],
                texture,
                started: std::time::Instant::now(),
                requested: false,
            }))
        }),
    )
    .unwrap();
}
