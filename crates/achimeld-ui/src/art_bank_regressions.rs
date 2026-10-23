use super::*;
use crate::art_bank::ArtTab;

#[test]
fn image_categories_follow_general_and_direct_layer_references() {
    let project = json!({"achievements":[
        {"generalImageId":"legacy-general", "generalImageEntries":[
            {"id":0,"imageId":"general"}, {"id":1,"imageId":"shared"},
            {"id":2,"atlasId":"atlas"}
        ], "icon":{"layers":[
            {"kind":"StandaloneImage","imageId":"icon"},
            {"kind":"SteamImportedArtwork","imageId":"shared","visible":false},
            {"kind":"AchievementImage","generalImageIndex":0,"imageId":"stale"},
            {"kind":"AtlasTile","atlasId":"atlas"}
        ]}},
        {"standaloneImageId":"legacy-icon","icon":null}
    ]});
    let (general, icon) = crate::art_bank::image_categories(&project);
    assert_eq!(general, ["general", "legacy-general", "shared"].into());
    assert_eq!(icon, ["icon", "legacy-icon", "shared"].into());
}

fn labels(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Pos2, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => out.push((text.galley.text().to_owned(), text.pos, clip)),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                labels(shape, clip, out);
            }
        }
        _ => {}
    }
}

#[test]
fn artwork_grid_wraps_uniform_tiles_and_selects_from_the_whole_body() {
    fn geometry(shape: &egui::Shape, tiles: &mut Vec<egui::Rect>, images: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(shape)
                if (143.0..145.0).contains(&shape.rect.width()) && shape.rect.height() > 160.0 =>
            {
                assert_eq!(shape.corner_radius, egui::CornerRadius::ZERO);
                if shape.fill != egui::Color32::TRANSPARENT {
                    tiles.push(shape.rect);
                }
            }
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                images.push(mesh.calc_bounds());
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    geometry(shape, tiles, images);
                }
            }
            _ => {}
        }
    }
    for scale in [1.0, 1.25, 2.0] {
        let mut fixture = Fixture::new();
        let mut project = fixture.editor.document().project().clone();
        let entries: Vec<_> = (0..80)
            .map(|index| {
                let id = new_id();
                let (width, height) = if index == 0 { (256, 64) } else { (64, 256) };
                fixture.previews.insert(
                    format!("asset:image:{id}"),
                    Preview {
                        width,
                        height,
                        source_size: [width, height],
                        rgba: [80, 150, 210, 255].repeat(width * height).into(),
                        fingerprint: id.clone(),
                    },
                );
                json!({"id":id,"displayName":format!("Artwork {index:02}"),"kind":"User",
                    "image":{"kind":"RelativeToProject","path":format!("{index}.png")}})
            })
            .collect();
        project["assets"]["images"] = json!(entries);
        fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let before = fixture.editor.document().clone();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut desktop = Desktop::default();
        desktop.art_bank.select_tab(ArtTab::Icon);
        let mut frame = |width, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 760.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), fixture.preferences());
                    desktop.art_bank_view(ui, &mut fixture);
                },
            );
            output.textures_delta.clear();
            let (mut tiles, mut images, mut text) = (Vec::new(), Vec::new(), Vec::new());
            for shape in output.shapes {
                geometry(&shape.shape, &mut tiles, &mut images);
                labels(&shape.shape, shape.clip_rect, &mut text);
            }
            images.retain(|image| tiles.iter().any(|tile| tile.contains_rect(*image)));
            (tiles, images, text)
        };
        let mut wide_columns = 0;
        for width in [1440.0, 760.0, 480.0] {
            frame(width, vec![]);
            let (tiles, images, text) = frame(width, vec![]);
            assert!(tiles.len() > 1 && tiles.len() < entries.len());
            assert!(tiles.iter().all(|rect| rect.size() == tiles[0].size()));
            assert!(
                tiles
                    .iter()
                    .all(|rect| rect.left() >= 0.0 && rect.right() <= width)
            );
            let columns = tiles
                .iter()
                .filter(|rect| rect.top() == tiles[0].top())
                .count();
            assert!(columns >= 2);
            if width == 1440.0 {
                wide_columns = columns;
            } else {
                assert!(columns < wide_columns);
            }
            assert!((images[0].aspect_ratio() - 4.0).abs() < 0.01);
            assert!((images[1].aspect_ratio() - 0.25).abs() < 0.01);
            for name in ["Artwork 00", "Artwork 01"] {
                let (_, pos, clip) = text.iter().find(|(label, _, _)| label == name).unwrap();
                assert!(
                    clip.contains(*pos),
                    "{width}: {name} at {pos:?}, clip {clip:?}"
                );
            }
        }
        let (tiles, _, _) = frame(1440.0, vec![]);
        // Empty tile padding is a hit target, just like the thumbnail and label.
        let pointer = tiles[1].right_bottom() - egui::vec2(3.0, 3.0);
        for pressed in [true, false] {
            frame(
                1440.0,
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        let (_, _, text) = frame(1440.0, vec![]);
        assert_eq!(
            text.iter()
                .filter(|(label, _, _)| label == "Artwork 01")
                .count(),
            2
        );
        assert_eq!(fixture.editor.document(), &before);
    }
}

#[test]
fn selected_artwork_keeps_list_and_details_scrollable_in_both_layouts() {
    for width in [480.0, 1200.0] {
        for tab in [ArtTab::General, ArtTab::Icon, ArtTab::Atlases] {
            let mut fixture = Fixture::new();
            let mut project = fixture.editor.document().project().clone();
            let entries: Vec<_> = (0..40)
                .map(|i| {
                    json!({
                        "id":new_id(),"displayName":format!("Artwork {i:02}"),
                        "kind":"User","image":{"kind":"RelativeToProject","path":format!("{i}.png")}
                    })
                })
                .collect();
            let id = entries[0]["id"].as_str().unwrap().to_owned();
            project["assets"][if tab == ArtTab::Atlases {
                "atlases"
            } else {
                "images"
            }] = json!(entries);
            project["achievements"][0]["generalImageEntries"] = json!(
                entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| json!({"id":i,"imageId":e["id"]}))
                    .collect::<Vec<_>>()
            );
            if tab == ArtTab::Icon {
                project["achievements"][0]["generalImageEntries"] = json!([]);
            }
            fixture.editor = Editor::new(
                achimeld_application::Document::from_parts(
                    project,
                    fixture.editor.document().catalog().clone(),
                )
                .unwrap(),
            );
            fixture.previews.insert(
                format!(
                    "asset:{}:{id}",
                    if tab == ArtTab::Atlases {
                        "atlas"
                    } else {
                        "image"
                    }
                ),
                Preview {
                    width: 64,
                    height: 64,
                    source_size: [1024, 1024],
                    rgba: vec![255; 64 * 64 * 4].into(),
                    fingerprint: "preview".into(),
                },
            );
            let before = fixture.editor.document().clone();
            let ctx = egui::Context::default();
            let mut desktop = Desktop::default();
            desktop.art_bank.select_tab(tab);
            desktop.art_bank.select(&id);
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 460.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| desktop.art_bank_view(ui, &mut fixture),
                );
                output.textures_delta.clear();
                let mut text = Vec::new();
                for shape in output.shapes {
                    labels(&shape.shape, shape.clip_rect, &mut text);
                }
                text
            };
            frame(vec![]);
            let initial = frame(vec![]);
            let list = initial.iter().find(|(s, _, _)| s == "Artwork 01").unwrap();
            let heading = initial
                .iter()
                .find(|(s, _, clip)| s == "Artwork 00" && *clip != list.2)
                .unwrap();
            let list_pointer = list.1 + egui::vec2(12.0, 8.0);
            let wheel = |pointer, delta| {
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, delta),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            frame(wheel(list_pointer, -130.0));
            let scrolled = frame(vec![]);
            assert!(
                scrolled
                    .iter()
                    .find(|(s, _, _)| s == "Artwork 01")
                    .is_none_or(|(_, pos, _)| pos.y < list.1.y),
                "list did not scroll at {width}: {tab:?}: {scrolled:?}"
            );
            // Find the details viewport by its heading, then scroll inside it.
            let detail = scrolled
                .iter()
                .find(|(s, _, clip)| s == "Artwork 00" && *clip == heading.2)
                .unwrap_or_else(|| panic!("missing details at {width}: {tab:?}; initial={initial:?}; scrolled={scrolled:?}"));
            let details_pointer = detail.1 + egui::vec2(20.0, 35.0);
            let details_y = detail.1.y;
            assert!(heading.2.height() <= 460.0);
            frame(wheel(details_pointer, -1000.0));
            for _ in 0..12 {
                frame(vec![]);
            }
            let scrolled = frame(vec![]);
            let after = scrolled
                .iter()
                .find(|(s, _, clip)| s == "Artwork 00" && *clip == heading.2);
            assert!(
                after.is_none_or(|(_, pos, _)| pos.y < details_y),
                "details did not scroll at {width}: {tab:?}"
            );
            assert!(scrolled.iter().any(|(s, _, _)| s == "Reload from file…"));
            assert_eq!(fixture.editor.document(), &before);
        }
    }
}

#[test]
fn artwork_divider_resizes_both_panes_and_source_wraps_without_editing() {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    let source = "C:\\Artwork\\Achievement icons\\Original source artwork with a long filename.png";
    project["assets"]["images"] = json!([{
        "id":"source-fixture", "displayName":"Selected artwork", "kind":"User",
        "image":{"kind":"RelativeToProject","path":"owned.png"},
        "importSource":{"kind":"File","path":source}
    }]);
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    let before = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx, fixture.preferences());
    let panel_fill = ctx.global_style().visuals.panel_fill;
    let mut desktop = Desktop::default();
    desktop.art_bank.select_tab(ArtTab::Icon);
    desktop.art_bank.select("source-fixture");
    fn panels(shape: &egui::Shape, fill: egui::Color32, out: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(rect) if rect.fill == fill && rect.rect.height() > 100.0 => {
                out.push(rect.rect)
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    panels(shape, fill, out);
                }
            }
            _ => {}
        }
    }
    let mut frame = |width, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 760.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.art_bank_view(ui, &mut fixture),
        );
        output.textures_delta.clear();
        let mut text = Vec::new();
        let mut rects = Vec::new();
        for shape in output.shapes {
            labels(&shape.shape, shape.clip_rect, &mut text);
            panels(&shape.shape, panel_fill, &mut rects);
        }
        (
            rects
                .into_iter()
                .max_by(|a, b| a.area().total_cmp(&b.area()))
                .unwrap(),
            text,
        )
    };
    frame(1200.0, vec![]);
    let (initial, text) = frame(1200.0, vec![]);
    assert!(text.iter().any(|(label, _, _)| label == source));
    assert!(text.iter().any(|(label, _, _)| label == "Imported from:"));
    let pointer = egui::pos2(initial.left() + 2.0, initial.center().y);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(1200.0, vec![egui::Event::PointerMoved(pointer)]);
    frame(1200.0, vec![button(pointer, true)]);
    let moved = pointer - egui::vec2(160.0, 0.0);
    frame(1200.0, vec![egui::Event::PointerMoved(moved)]);
    frame(1200.0, vec![]);
    frame(1200.0, vec![button(moved, false)]);
    let (resized, _) = frame(1200.0, vec![]);
    assert!(
        resized.width() > initial.width() + 100.0,
        "{initial:?} -> {resized:?}"
    );
    assert!((resized.right() - initial.right()).abs() < 1.0);
    // The remembered allocation must still leave the list usable after a window resize.
    let (bounded, _) = frame(760.0, vec![]);
    assert!(
        bounded.width() >= 280.0 && bounded.width() <= 501.0,
        "{bounded:?}"
    );
    let (stacked, text) = frame(480.0, vec![]);
    assert!(stacked.width() <= 480.0);
    let (_, pos, clip) = text.iter().find(|(label, _, _)| label == source).unwrap();
    assert!(pos.x >= clip.left() && pos.x < clip.right());
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn artwork_context_shows_achievement_tiles_and_delete_supports_cancel_confirm_and_undo() {
    for scale in [1.0, 1.5] {
        let mut fixture = Fixture::new();
        let image = new_id();
        let mut project = fixture.editor.document().project().clone();
        project["assets"]["images"] = json!([{"id":image,"displayName":"Shared artwork","kind":"User",
            "image":{"kind":"RelativeToProject","path":"missing.png"}}]);
        let achievement = project["achievements"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let api = project["achievements"][0]["apiName"]
            .as_str()
            .unwrap()
            .to_owned();
        project["achievements"][0]["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"StandaloneImage","imageId":image}]});
        fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let before = fixture.editor.document().clone();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut desktop = Desktop::default();
        desktop.art_bank.select_tab(ArtTab::Icon);
        let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), fixture.preferences());
                    desktop.art_bank_view(ui, fixture);
                },
            );
            output.textures_delta.clear();
            let mut text = Vec::new();
            for shape in output.shapes {
                labels(&shape.shape, shape.clip_rect, &mut text);
            }
            text
        };
        let pointer = |pos, button, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let point = |text: &[(String, egui::Pos2, egui::Rect)], label: &str| {
            text.iter()
                .find(|(s, _, _)| s == label)
                .unwrap_or_else(|| panic!("Missing {label}: {text:?}"))
                .1
                + egui::vec2(5.0, 5.0)
        };
        frame(&mut desktop, &mut fixture, vec![]);
        let text = frame(&mut desktop, &mut fixture, vec![]);
        let tile = point(&text, "Shared artwork");
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(tile, egui::PointerButton::Secondary, pressed),
            );
        }
        let text = frame(&mut desktop, &mut fixture, vec![]);
        for expected in ["Duplicate", "Delete", "Used by 1 achievement", api.as_str()] {
            assert!(
                text.iter().any(|(s, _, _)| s == expected),
                "Missing {expected}: {text:?}"
            );
        }
        assert_eq!(fixture.editor.document(), &before);
        let delete = point(&text, "Delete");
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(delete, egui::PointerButton::Primary, pressed),
            );
        }
        let text = frame(&mut desktop, &mut fixture, vec![]);
        assert!(
            text.iter()
                .any(|(s, _, _)| s == "Delete \"Shared artwork\"?")
        );
        assert!(text.iter().any(|(s, _, _)| s == &api));
        let cancel = point(&text, "Cancel");
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(cancel, egui::PointerButton::Primary, pressed),
            );
        }
        assert_eq!(fixture.editor.document(), &before);
        frame(&mut desktop, &mut fixture, vec![]);
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(tile, egui::PointerButton::Secondary, pressed),
            );
        }
        let text = frame(&mut desktop, &mut fixture, vec![]);
        let delete = point(&text, "Delete");
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(delete, egui::PointerButton::Primary, pressed),
            );
        }
        let text = frame(&mut desktop, &mut fixture, vec![]);
        // The last title is the bottom destructive action; the first is the header.
        let confirm = text
            .iter()
            .rev()
            .find(|(s, _, _)| s == "Delete artwork")
            .unwrap()
            .1
            + egui::vec2(5.0, 5.0);
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(confirm, egui::PointerButton::Primary, pressed),
            );
        }
        assert!(
            fixture.editor.document().project()["assets"]["images"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let a = fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .unwrap();
        assert!(a["icon"]["layers"][0]["imageId"].is_null());
        fixture.editor.undo();
        assert_eq!(fixture.editor.document(), &before);
        // Duplicating from the same context keeps the original and gets a fresh identity.
        frame(&mut desktop, &mut fixture, vec![]);
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(tile, egui::PointerButton::Secondary, pressed),
            );
        }
        let text = frame(&mut desktop, &mut fixture, vec![]);
        let duplicate = point(&text, "Duplicate");
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                pointer(duplicate, egui::PointerButton::Primary, pressed),
            );
        }
        let images = &fixture.editor.document().project()["assets"]["images"];
        assert_eq!(images.as_array().unwrap().len(), 2);
        assert_ne!(images[0]["id"], images[1]["id"]);
        assert_eq!(images[0]["image"], images[1]["image"]);
        fixture.editor.undo();
        assert_eq!(fixture.editor.document(), &before);
    }
}

#[test]
#[ignore = "Native Art Data Bank usage and confirmation screenshot; external output path"]
fn native_artwork_usage_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        fixture: Fixture,
        desktop: Desktop,
        frames: usize,
        prompt: bool,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
            if !self.prompt && self.frames >= 10 {
                let pos = egui::pos2(88.0, 150.0);
                input.events.push(egui::Event::PointerMoved(pos));
                if self.frames == 10 || self.frames == 11 {
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed: self.frames == 10,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.fixture.preferences());
            ui.set_style(ui.ctx().global_style());
            ui.painter().rect_filled(
                ui.max_rect(),
                0,
                theme::color(self.fixture.preferences(), "Window"),
            );
            self.desktop.art_bank_view(ui, &mut self.fixture);
            self.frames += 1;
            if self.frames == 45 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            for event in ui.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
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
            }
            assert!(self.frames < 1000, "Screenshot did not arrive");
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Screenshot destination"),
    ))
    .unwrap();
    assert!(output.starts_with(std::path::absolute(crate::test_output::root()).unwrap()));
    let prompt = std::env::var_os("ACHIMELD_ART_PROMPT").is_some();
    let scale = std::env::var("ACHIMELD_ART_SCALE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    let mut catalog = fixture.editor.document().catalog().clone();
    let source = new_id();
    project["assets"]["images"] = json!([{"id":source,"displayName":"Shared artwork","kind":"User",
        "image":{"kind":"RelativeToProject","path":"source.png"}}]);
    let seed = project["achievements"][0].clone();
    let rgba: std::sync::Arc<[u8]> = include_bytes!("../assets/toolbar/layer-eye.rgba")
        .as_slice()
        .into();
    let preview = Preview {
        width: 64,
        height: 64,
        source_size: [64, 64],
        rgba,
        fingerprint: "eye".into(),
    };
    fixture
        .previews
        .insert(format!("asset:image:{source}"), preview.clone());
    let mut achievements = Vec::new();
    for index in 0..28 {
        let mut a = seed.clone();
        let id = new_id();
        let name = new_id().replace('-', "")[..6].to_owned();
        a["id"] = json!(id);
        a["apiName"] = json!(format!("MISSION_{:02}", index + 1));
        a["displayOrder"] = json!(index);
        a["displayNameLocalizationId"] = json!(name);
        a["internalId"] = json!(index);
        catalog.rows.insert(
            name,
            [
                ("Context".into(), "Artwork usage fixture".into()),
                ("English".into(), format!("Mission {}", index + 1)),
            ]
            .into(),
        );
        a["icon"] = json!({"layers":[{"id":new_id(),"kind":"StandaloneImage","imageId":source}]});
        fixture.previews.insert(id, preview.clone());
        achievements.push(a);
    }
    project["achievements"] = json!(achievements);
    project["nextAchievementInternalId"] = json!(28);
    fixture.editor =
        Editor::new(achimeld_application::Document::from_parts(project, catalog).unwrap());
    let mut desktop = Desktop::default();
    desktop.art_bank.select_tab(ArtTab::Icon);
    desktop.art_bank.select(&source);
    if prompt {
        desktop.art_bank.confirm_delete(
            fixture.editor.document().project()["projectId"]
                .as_str()
                .unwrap()
                .into(),
            source,
        );
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([1100.0, 900.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "Art Data Bank review",
        options,
        Box::new(move |creation| {
            creation.egui_ctx.set_pixels_per_point(scale);
            Ok(Box::new(Window {
                fixture,
                desktop,
                frames: 0,
                prompt,
                output,
            }))
        }),
    )
    .unwrap();
}
