use super::*;
fn image_fixture() -> (Fixture, Value, String) {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    let image = new_id();
    let id = project["achievements"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    project["assets"]["images"] = json!([{"id":image,"displayName":"General artwork","kind":"User","image":{"kind":"RelativeToProject","path":"general.png"}}]);
    let stat = project["stats"][0]["id"].clone();
    project["stats"][0]["apiName"] = json!("EarthquakeUses");
    let entity = &mut project["achievements"][0];
    entity["generalImageEntries"] = json!([{"id":0,"imageId":image,"atlasId":null,"tileIndex":0},{"id":1,"imageId":image,"atlasId":null,"tileIndex":0}]);
    entity["progressStatId"] = stat;
    entity["unlockValue"] = json!(100);
    entity["displayOrder"] = json!(25);
    entity["apiName"] = json!("ACHIEVEMENT_026_100_EARTHQUAKE");
    let mut layer = crate::layer_palette::new_layer("AchievementImage");
    layer["generalImageIndex"] = json!(0);
    layer["displayName"] = json!("Earthquake artwork");
    entity["icon"] = json!({"layers":[layer]});
    project["lastRemoteSnapshot"] =
        json!({"achievements":[{"apiName":"ACHIEVEMENT_026_100_EARTHQUAKE","steamId":"7_26"}]});
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    let entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    for (key, text) in [
        ("displayNameLocalizationId", "Earthquake - {0} Uses"),
        ("descriptionLocalizationId", "Use Earthquake 100 times."),
    ] {
        fixture
            .execute(
                "Fixture",
                Command::SetLocalization {
                    id: entity[key].as_str().unwrap().into(),
                    language: "English".into(),
                    text: text.into(),
                },
                None,
            )
            .unwrap();
    }
    let pixels = vec![255; 64 * 64 * 4];
    let preview = Preview {
        width: 64,
        height: 64,
        source_size: [64, 64],
        rgba: pixels.into(),
        fingerprint: "general-fixture".into(),
    };
    fixture
        .previews
        .insert(format!("asset:image:{image}"), preview.clone());
    fixture.previews.insert(id.clone(), preview);
    fixture
        .save_icon_style(achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Shiny Icon".into(),
            recipe: entity["icon"].clone(),
        })
        .unwrap();
    (fixture, entity, id)
}
fn click(point: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn explorer_frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    entity: &mut Value,
    layer: &mut Value,
    events: Vec<egui::Event>,
) -> egui::Rect {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 500.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            theme::apply(ui.ctx(), fixture.preferences());
            ui.set_style(ui.ctx().global_style());
            desktop.shortcuts(ui.ctx(), fixture);
            desktop.general_explorer(ui, fixture, entity, Some(layer));
        },
    );
    output.textures_delta.clear();
    desktop.general_images.drop_zones.values().next().unwrap().1
}
#[test]
fn explorer_selects_a_source_and_delete_removes_only_that_entry() {
    let (mut fixture, mut entity, id) = image_fixture();
    let before = fixture.editor.document().clone();
    let mut layer = entity["icon"]["layers"][0].clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    explorer_frame(
        &ctx,
        &mut desktop,
        &mut fixture,
        &mut entity,
        &mut layer,
        vec![],
    );
    let rect = explorer_frame(
        &ctx,
        &mut desktop,
        &mut fixture,
        &mut entity,
        &mut layer,
        vec![],
    );
    assert_eq!(rect.height(), 124.0);
    let point = rect.min + egui::vec2(5.0 + 86.0 + 40.0, 45.0);
    for pressed in [true, false] {
        explorer_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut entity,
            &mut layer,
            click(point, pressed),
        );
    }
    assert_eq!(layer["generalImageIndex"], 1);
    assert_eq!(desktop.general_images.selected, Some(1));
    explorer_frame(
        &ctx,
        &mut desktop,
        &mut fixture,
        &mut entity,
        &mut layer,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(entity["generalImageEntries"].as_array().unwrap().len(), 1);
    assert_eq!(entity["generalImageEntries"][0]["id"], 0);
    assert_eq!(
        fixture.editor.document(),
        &before,
        "Delete must never reach the selected achievement"
    );
}
#[test]
fn explorer_removal_migrates_legacy_sources_and_cannot_resurrect_them() {
    let image = new_id();
    let atlas = new_id();
    let mut entity = json!({"generalImageId":image,"generalAtlasId":atlas,"generalTileIndex":3});
    let entries = crate::general_images::entries(&entity);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1]["id"], 1);
    crate::general_images::store_entries(&mut entity, vec![]);
    assert!(crate::general_images::entries(&entity).is_empty());
    assert!(entity["generalImageId"].is_null());
    assert!(entity["generalAtlasId"].is_null());
}
#[test]
fn explorer_atlas_uvs_follow_offsets_spacing_and_traversal() {
    let settings = json!({"tileWidth":16,"tileHeight":12,"leftOffset":3,"topOffset":5,"horizontalSpacing":2,"verticalSpacing":4,"traversal":"ColumnMajor"});
    let uv = crate::general_images::tile_uv([55, 45], &settings, 2).unwrap();
    assert!((uv.left() - 21.0 / 55.0).abs() < 0.0001);
    assert!((uv.top() - 5.0 / 45.0).abs() < 0.0001);
    assert!(crate::general_images::tile_uv([55, 45], &settings, 6).is_none());
}
fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) => out.push(t.galley.text().into()),
        egui::Shape::Vec(v) => {
            for s in v {
                texts(s, out)
            }
        }
        _ => {}
    }
}
#[test]
fn general_panel_draws_the_original_fields_and_shared_banner_without_editing() {
    for width in [280.0, 360.0, 500.0] {
        for scale in [1.0, 1.25, 2.0] {
            let (mut fixture, mut entity, id) = image_fixture();
            let before = entity.clone();
            let document = fixture.editor.document().clone();
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            ctx.set_pixels_per_point(scale);
            let mut desktop = Desktop::default();
            desktop.selection = vec![id];
            let mut labels = Vec::new();
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 850.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), fixture.preferences());
                        ui.set_style(ui.ctx().global_style());
                        desktop.achievement_general(ui, &mut fixture, &mut entity);
                    },
                );
                labels.clear();
                for s in &output.shapes {
                    texts(&s.shape, &mut labels)
                }
                output.textures_delta.clear();
            }
            for expected in [
                "E. D. Name",
                "E. Description",
                "S. API Name",
                "General images",
                "Follows the progress stat type.",
                "U. Value",
                "D. Order",
                "Shiny Icon",
            ] {
                assert!(
                    labels
                        .iter()
                        .any(|s| crate::fields::label_matches(s, expected)),
                    "missing {expected} at {width}/{scale}: {labels:?}"
                );
            }
            assert_eq!(entity, before);
            assert_eq!(fixture.editor.document(), &document);
        }
    }
}
#[test]
fn right_hand_properties_display_the_generated_layer_type_symbol() {
    let (mut fixture, mut entity, id) = image_fixture();
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    let mut layers = entity["icon"]["layers"].as_array().unwrap().clone();
    desktop.selected_layer = layers[0]["id"].as_str().map(str::to_owned);
    let mut expected = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        expected = crate::layer_icons::texture(ui.ctx(), "AchievementImage");
        desktop.layer_properties(ui, &mut fixture, &mut layers, &mut entity);
    });
    fn contains(shape: &egui::Shape, id: egui::TextureId) -> bool {
        match shape {
            egui::Shape::Mesh(mesh) => mesh.texture_id == id,
            egui::Shape::Rect(rect) => rect.brush.as_ref().is_some_and(|b| b.fill_texture_id == id),
            egui::Shape::Vec(shapes) => shapes.iter().any(|s| contains(s, id)),
            _ => false,
        }
    }
    assert!(
        output
            .shapes
            .iter()
            .any(|s| contains(&s.shape, expected.unwrap())),
        "The generated type texture must actually be painted in properties"
    );
    output.textures_delta.clear();
}
#[test]
#[ignore = "Native General and layer-properties preview; external screenshot output"]
fn native_general_and_layer_properties_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        entity: Value,
        output: std::path::PathBuf,
        started: std::time::Instant,
        requested: bool,
        menu_step: u8,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
            if std::env::var_os("ACHIMELD_PREVIEW_GENERAL_MENU").is_none()
                || self.started.elapsed().as_secs_f32() < 0.15
                || self.menu_step >= 2
            {
                return;
            }
            if let Some((_, rect, _, _)) = self
                .desktop
                .general_images
                .drop_zones
                .values()
                .min_by(|a, b| a.1.left().total_cmp(&b.1.left()))
            {
                let pos = rect.center();
                input.events.push(egui::Event::PointerMoved(pos));
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed: self.menu_step == 0,
                    modifiers: egui::Modifiers::NONE,
                });
                self.menu_step += 1;
                if self.menu_step == 2 {
                    self.started = std::time::Instant::now();
                }
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.fixture.preferences());
            ui.set_style(ui.ctx().global_style());
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                theme::color(self.fixture.preferences(), "Panel"),
            );
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(294.0, 780.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.desktop.inspector(ui, &mut self.fixture);
                    },
                );
                ui.add_space(20.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(372.0, 780.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let mut layers = self.entity["icon"]["layers"].as_array().unwrap().clone();
                        self.desktop.layer_properties(
                            ui,
                            &mut self.fixture,
                            &mut layers,
                            &mut self.entity,
                        );
                    },
                );
            });
            let menu_ready =
                std::env::var_os("ACHIMELD_PREVIEW_GENERAL_MENU").is_none() || self.menu_step == 2;
            if !self.requested && menu_ready && self.started.elapsed().as_secs_f32() > 0.5 {
                self.requested = true;
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
                let bytes: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
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
            assert!(self.started.elapsed().as_secs() < 20);
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::PathBuf::from(std::env::var_os("ACHIMELD_SCREENSHOT_TO").unwrap());
    assert!(output.starts_with(crate::test_output::root()));
    let (mut fixture, mut entity, id) = image_fixture();
    entity["generalImageEntries"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    if std::env::var_os("ACHIMELD_PREVIEW_GENERAL_MENU").as_deref()
        == Some(std::ffi::OsStr::new("empty"))
    {
        entity["generalImageEntries"] = json!([]);
    }
    fixture
        .execute(
            "Fixture",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity.clone(),
            },
            None,
        )
        .unwrap();
    if let Some(path) = std::env::var_os("ACHIMELD_GENERAL_PREVIEW_SOURCE") {
        let image = image::open(path).unwrap().into_rgba8();
        let preview = Preview {
            width: image.width() as usize,
            height: image.height() as usize,
            source_size: [image.width() as usize, image.height() as usize],
            rgba: image.into_raw().into(),
            fingerprint: "reference-art".into(),
        };
        let image_id = entity["generalImageEntries"][0]["imageId"]
            .as_str()
            .unwrap();
        fixture
            .previews
            .insert(format!("asset:image:{image_id}"), preview.clone());
        fixture.previews.insert(id.clone(), preview);
    }
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    desktop.selected_layer = entity["icon"]["layers"][0]["id"]
        .as_str()
        .map(str::to_owned);
    eframe::run_native(
        "Achimeld General and layer properties",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([710.0, 800.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            Ok(Box::new(Window {
                desktop,
                fixture,
                entity,
                output,
                started: std::time::Instant::now(),
                requested: false,
                menu_step: 0,
            }))
        }),
    )
    .unwrap();
}

#[test]
fn choosing_a_style_image_slot_preserves_each_achievements_sources_and_undo() {
    let (mut fixture, entity, id) = image_fixture();
    let preferences = Preferences {
        layer_properties_in_sidebar: false,
        ..fixture.preferences().clone()
    };
    fixture.editor = fixture.editor.with_preferences(preferences);
    let mut other = fixture.editor.document().entities(EntityKind::Achievement)[1].clone();
    let (layers, _) =
        crate::layer_palette::fresh_bundle(entity["icon"]["layers"].as_array().unwrap(), "", None);
    other["icon"] = json!({"layers":layers});
    other["generalImageEntries"] = entity["generalImageEntries"].clone();
    fixture
        .execute(
            "Fixture",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: other["id"].as_str().unwrap().into(),
                value: other.clone(),
            },
            None,
        )
        .unwrap();
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    desktop.layer_palette.achievement = Some(id);
    desktop.layer_palette.active_style = Some(fixture.icon_styles()[0].id.clone());
    desktop.selected_layer = entity["icon"]["layers"][0]["id"]
        .as_str()
        .map(str::to_owned);
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), fixture.preferences());
                ui.set_style(ui.ctx().global_style());
                desktop.layer_editor(ui, fixture);
            },
        );
        out.textures_delta.clear();
    };
    frame(&mut desktop, &mut fixture, vec![]);
    frame(&mut desktop, &mut fixture, vec![]);
    let rect = desktop.general_images.drop_zones.values().next().unwrap().1;
    let point = rect.min + egui::vec2(131.0, 45.0);
    for pressed in [true, false] {
        frame(&mut desktop, &mut fixture, click(point, pressed));
    }
    for updated in fixture.editor.document().entities(EntityKind::Achievement) {
        assert_eq!(updated["icon"]["layers"][0]["generalImageIndex"], 1);
        let old = before
            .entity(EntityKind::Achievement, updated["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(updated["generalImageEntries"], old["generalImageEntries"]);
    }
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
}
