use super::*;

#[test]
fn particle_seed_input_accepts_the_full_unsigned_seed_range() {
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut seed = 1;
    let frame = |seed: &mut u32, events| {
        let mut id = egui::Id::NULL;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                ui.set_style(ui.ctx().global_style());
                id = crate::particles::seed_input(ui, seed).id;
            },
        );
        output.textures_delta.clear();
        id
    };
    let id = frame(&mut seed, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&mut seed, vec![]);
    frame(&mut seed, vec![egui::Event::Text("4294967295".into())]);
    assert_eq!(
        seed,
        u32::MAX,
        "The seed field must allow typing every valid seed without a slider"
    );
}

#[test]
#[ignore = "Native particle inspector screenshot; writes to external build output"]
fn native_particle_inspector_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        layer: Value,
        textures: Vec<egui::TextureHandle>,
        output: std::path::PathBuf,
        started: std::time::Instant,
        requested: bool,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), &Preferences::default());
            ui.set_style(ui.ctx().global_style());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ui.heading("Particle layer");
                    ui.weak("Seeded particle effect");
                    for (index, texture) in self.textures.iter().enumerate() {
                        ui.label(format!("Achievement {}", index + 1));
                        ui.add(egui::Image::new((texture.id(), egui::vec2(210.0, 210.0))));
                    }
                    ui.weak("Ring = Sphere − smaller Sphere");
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        crate::particles::action_bar(
                            ui,
                            &mut self.fixture,
                            &mut self.layer,
                            "",
                            false,
                            None,
                        );
                        self.desktop
                            .particle_properties(ui, &mut self.fixture, &mut self.layer);
                    });
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
                "Particle inspector screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    let large = std::env::var_os("ACHIMELD_PARTICLE_LARGE").is_some();
    let output = crate::test_output::root().join(if large {
        "particle-inspector-large.png"
    } else {
        "particle-inspector.png"
    });
    eframe::run_native("Achimeld particles",eframe::NativeOptions {
        viewport:egui::ViewportBuilder::default().with_decorations(false).with_inner_size([1220.0,1000.0]),
        event_loop_builder:Some(Box::new(|builder|{builder.with_any_thread(true);})),..Default::default()
    },Box::new(move|cc| {
        configure_test_fonts(&cc.egui_ctx);
            if large { cc.egui_ctx.set_pixels_per_point(1.5); }
        let mut fixture=Fixture::new();
        let mut settings=achimeld_application::particles::ParticleSettings { different_seed_per_instance:true,
            size_min:40.0,size_max:100.0,softness_min:Some(0.02),softness:0.2,
            shapes:vec![Default::default(),achimeld_application::particles::ParticleShape {operation:"Subtract".into(),scale:0.6,..Default::default()}],
            ..Default::default() };
        settings.gradient=json!({"kind":"Radial","stops":[{"offset":0.0,"colorHex":"#FF70E5FF"},{"offset":1.0,"colorHex":"#FF725DFF"}]});
        fixture.editor.execute("Add",Command::Add(EntityKind::Achievement),None).unwrap();
        let mut textures=Vec::new();
        let entities=fixture.editor.document().entities(EntityKind::Achievement).to_vec();
        let layer=json!({"id":new_id(),"kind":"Particle","particles":settings});
        for mut entity in entities.into_iter().take(1) {
            let id=entity["id"].as_str().unwrap().to_owned();
            entity["icon"]=json!({"backgroundColorHex":"#FF1C1E24","layers":[layer]});
            fixture.editor.execute("Particles",Command::ReplaceEntity{kind:EntityKind::Achievement,id:id.clone(),value:entity},None).unwrap();
            let image=render(fixture.editor.document(),&id,Some((256,256)),&mut NoAssets).unwrap().achieved;
            textures.push(cc.egui_ctx.load_texture(format!("particle-{id}"),egui::ColorImage::from_rgba_unmultiplied([256,256],image.as_raw()),egui::TextureOptions::LINEAR));
        }
        Ok(Box::new(Window{desktop:Desktop::default(),fixture,layer,textures,output,started:std::time::Instant::now(),requested:false}))
    })).unwrap();
}

#[test]
fn particle_inspector_preserves_sparse_records_and_exposes_instance_seed_switch() {
    let mut fixture = Fixture::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    let mut layer = json!({"id":new_id(),"kind":"Particle"});
    let before = layer.clone();
    let mut labels = String::new();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(520.0, 1600.0),
            )),
            ..Default::default()
        },
        |ui| {
            theme::apply(ui.ctx(), &Preferences::default());
            crate::particles::action_bar(ui, &mut fixture, &mut layer, "test", false, None);
            desktop.particle_properties(ui, &mut fixture, &mut layer);
        },
    );
    fn collect(shape: &egui::Shape, labels: &mut String) {
        match shape {
            egui::Shape::Text(text) => labels.push_str(text.galley.text()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, labels);
                }
            }
            _ => {}
        }
    }
    for shape in &output.shapes {
        collect(&shape.shape, &mut labels);
    }
    output.textures_delta.clear();
    assert!(
        ctx.read_response(egui::Id::new((
            "particle-seed-per-achievement",
            layer["id"].as_str().unwrap()
        )))
        .is_some()
    );
    assert!(
        (labels.contains("P. Count") || labels.contains("Particle count"))
            && labels.contains("Particle gradient")
    );
    assert_eq!(
        layer, before,
        "Opening a sparse layer must not dirty its recipe"
    );
}

#[test]
fn particle_globals_precede_shape_editor_and_star_inner_radius_has_two_handles() {
    for width in [220.0, 520.0] {
        let mut fixture = Fixture::new();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        let mut layer = json!({"id":new_id(),"kind":"Particle","particles":{"shape":"Star"}});
        let original = layer.clone();
        let mut labels = Vec::new();
        let mut knobs = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 2200.0),
                )),
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                ui.set_style(ui.ctx().global_style());
                crate::particles::action_bar(ui, &mut fixture, &mut layer, "test", false, None);
                desktop.particle_properties(ui, &mut fixture, &mut layer);
            },
        );
        fn collect(
            shape: &egui::Shape,
            labels: &mut Vec<(String, f32)>,
            knobs: &mut Vec<egui::Pos2>,
        ) {
            match shape {
                egui::Shape::Text(text) => labels.push((
                    text.galley.text().to_owned(),
                    text.pos.y + text.galley.size().y / 2.0,
                )),
                egui::Shape::Circle(circle) if circle.radius == 5.0 => knobs.push(circle.center),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, labels, knobs);
                    }
                }
                _ => {}
            }
        }
        for shape in &output.shapes {
            collect(&shape.shape, &mut labels, &mut knobs);
        }
        output.textures_delta.clear();
        let y = |label: &str| {
            labels
                .iter()
                .find(|(text, _)| crate::fields::label_matches(text, label))
                .unwrap_or_else(|| panic!("Missing {label} at {width}: {labels:?}"))
                .1
        };
        for label in [
            "Size",
            "Rotation",
            "Shape X",
            "Shape Y",
            "Opacity",
            "SDF Softness",
            "Regularity",
        ] {
            assert!(
                y(label) < y("Spawn area"),
                "{label} must precede spawn and component controls"
            );
        }
        let inner = y("I. Radius");
        assert_eq!(
            knobs
                .iter()
                .filter(|point| (point.y - inner).abs() < 2.0)
                .count(),
            2,
            "Inner radius needs a minimum and maximum handle"
        );
        assert_eq!(
            layer, original,
            "Viewing a legacy star must not change its recipe"
        );
    }
}

#[test]
fn particle_spawn_gizmo_edits_center_in_layer_space_at_multiple_scales() {
    for scale in [1.0, 1.5, 2.0] {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut value = json!(achimeld_application::particles::ParticleSettings::default());
        let mut frame = |events| {
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(420.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = crate::particles::spawn_gizmo(ui, &mut value, 2.0, "instance").rect;
                },
            );
            output.textures_delta.clear();
            rect
        };
        let rect = frame(vec![]);
        let point = rect.min + rect.size() * egui::vec2(0.75, 0.25);
        frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!((value["centerX"].as_f64().unwrap() - 0.75).abs() < 0.01);
        assert!((value["centerY"].as_f64().unwrap() - 0.25).abs() < 0.01);
    }
}
