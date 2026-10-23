use super::*;
use crate::source_picker::SourceKind;
use egui::{Color32, Pos2, Rect, Vec2};

fn picker_fixture(atlas: bool) -> (Fixture, String) {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    let achievement = project["achievements"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let assets: Vec<_> = (0..80)
        .map(|index| {
            let id = new_id();
            let (width, height) = match index {
                0 => (256, 64),
                1 => (64, 256),
                _ => (64, 64),
            };
            fixture.previews.insert(
                format!("asset:{}:{id}", if atlas { "atlas" } else { "image" }),
                Preview {
                    width,
                    height,
                    source_size: [512, 512],
                    rgba: [80 + index as u8, 150, 210, 255].repeat(width * height).into(),
                    fingerprint: id.clone(),
                },
            );
            json!({"id":id,"displayName":if index == 0 {
                "Artwork with an extremely long descriptive title that must never stretch the chooser ".repeat(4)
            } else { format!("Artwork {index:02}") }, "kind":"User",
                "image":{"kind":"RelativeToProject","path":format!("{index}.png")}})
        })
        .collect();
    project["assets"][if atlas { "atlases" } else { "images" }] = json!(assets);
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    (fixture, achievement)
}

struct PickerFrame {
    window: Rect,
    tiles: Vec<(Rect, Color32)>,
    labels: Vec<(String, Pos2, Rect)>,
    images: Vec<Rect>,
    outlines: Vec<egui::epaint::RectShape>,
}
impl Default for PickerFrame {
    fn default() -> Self {
        Self {
            window: Rect::NOTHING,
            tiles: Vec::new(),
            labels: Vec::new(),
            images: Vec::new(),
            outlines: Vec::new(),
        }
    }
}
fn collect(shape: &egui::Shape, clip: Rect, frame: &mut PickerFrame) {
    match shape {
        egui::Shape::Rect(shape)
            if (140.0..160.0).contains(&shape.rect.width()) && shape.rect.height() > 150.0 =>
        {
            if shape.fill.a() > 0 {
                frame.tiles.push((shape.rect, shape.fill));
            } else if shape.stroke.color.a() > 0 {
                frame.outlines.push(shape.clone());
            }
        }
        egui::Shape::Text(text) => frame
            .labels
            .push((text.galley.text().into(), text.pos, clip)),
        egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
            frame.images.push(mesh.calc_bounds());
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect(shape, clip, frame);
            }
        }
        _ => {}
    }
}
fn picker_frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    size: Vec2,
    time: f64,
    events: Vec<egui::Event>,
) -> PickerFrame {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ui| {
            theme::apply(ui.ctx(), fixture.preferences());
            desktop.source_picker_window(ui.ctx(), fixture);
        },
    );
    output.textures_delta.clear();
    let mut result = PickerFrame {
        window: ctx
            .memory(|memory| memory.area_rect(egui::Id::new("layer-source-picker")))
            .unwrap(),
        ..Default::default()
    };
    for shape in output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut result);
    }
    result
}
fn label(frame: &PickerFrame, title: &str) -> Pos2 {
    frame
        .labels
        .iter()
        .find(|(text, _, _)| text == title)
        .unwrap_or_else(|| panic!("Missing {title}: {:?}", frame.labels))
        .1
}
fn click(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn source_picker_wraps_uniform_tiles_and_stays_on_screen_after_resizing() {
    for atlas in [false, true] {
        for scale in [1.0, 1.25, 2.0] {
            let (mut fixture, achievement) = picker_fixture(atlas);
            let original = fixture.editor.document().clone();
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            ctx.set_pixels_per_point(scale);
            let mut desktop = Desktop::default();
            desktop.open_general_picker(
                &ctx,
                &achievement,
                if atlas {
                    SourceKind::Atlas
                } else {
                    SourceKind::Image
                },
            );
            for (step, size) in [
                egui::vec2(1920.0, 1080.0),
                egui::vec2(800.0, 700.0),
                egui::vec2(480.0, 540.0),
                egui::vec2(320.0, 480.0),
            ]
            .into_iter()
            .enumerate()
            {
                let mut frame = PickerFrame::default();
                for tick in 0..4 {
                    frame = picker_frame(
                        &ctx,
                        &mut desktop,
                        &mut fixture,
                        size,
                        (step * 4 + tick) as f64 * 0.05,
                        vec![],
                    );
                }
                assert!(
                    frame.window.left() >= 0.0
                        && frame.window.top() >= 0.0
                        && frame.window.right() <= size.x + 1.0
                        && frame.window.bottom() <= size.y + 1.0,
                    "Picker exceeds {size:?} at {scale}: {:?}",
                    frame.window
                );
                assert!(
                    frame.tiles.len() > 1,
                    "No tiles at {size:?}/{scale}; labels: {:?}",
                    frame.labels
                );
                let first = frame.tiles[0].0;
                assert!(
                    frame
                        .tiles
                        .iter()
                        .all(|(tile, _)| (tile.width() - first.width()).abs() < 0.1
                            && (tile.height() - first.height()).abs() < 0.1)
                );
                assert!(
                    frame
                        .tiles
                        .iter()
                        .all(|(tile, _)| tile.left() >= frame.window.left()
                            && tile.right() <= frame.window.right())
                );
                assert!(
                    frame
                        .tiles
                        .iter()
                        .any(|(tile, _)| tile.top() > first.bottom()),
                    "Images did not form a grid at {size:?}"
                );
                assert!(label(&frame, "Cancel").y < size.y);
            }
            assert_eq!(fixture.editor.document(), &original);
        }
    }
}

#[test]
fn source_picker_selects_the_whole_tile_and_scrolls_to_the_last_image() {
    let (mut fixture, achievement) = picker_fixture(false);
    let original = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.open_general_picker(&ctx, &achievement, SourceKind::Image);
    let size = egui::vec2(900.0, 760.0);
    let mut frame = PickerFrame::default();
    for tick in 0..4 {
        frame = picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            tick as f64 * 0.05,
            vec![],
        );
    }
    // Landscape and portrait images must retain their different aspect ratios.
    assert!(
        frame
            .images
            .iter()
            .any(|rect| (rect.aspect_ratio() - 4.0).abs() < 0.01)
    );
    assert!(
        frame
            .images
            .iter()
            .any(|rect| (rect.aspect_ratio() - 0.25).abs() < 0.01)
    );
    let tile = frame.tiles[1].0;
    let point = tile.left_top() + egui::vec2(3.0, 3.0);
    for (tick, pressed) in [true, false].into_iter().enumerate() {
        picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            0.2 + tick as f64 * 0.02,
            click(point, pressed),
        );
    }
    for tick in 0..20 {
        frame = picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            0.24 + tick as f64 * 0.02,
            vec![],
        );
    }
    let fill = theme::color(fixture.preferences(), "Control");
    assert!(
        frame
            .tiles
            .iter()
            .any(|(rect, color)| *color == fill
                && (rect.width() / tile.width() - 1.03).abs() < 0.001)
    );
    assert!(
        frame
            .outlines
            .iter()
            .any(|outline| outline.stroke.color == Color32::WHITE
                && (outline.stroke.width - 2.06).abs() < 0.01
                && outline.corner_radius == egui::CornerRadius::ZERO)
    );
    assert_eq!(fixture.editor.document(), &original);

    picker_frame(
        &ctx,
        &mut desktop,
        &mut fixture,
        size,
        0.7,
        vec![
            egui::Event::PointerMoved(tile.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -20000.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for tick in 0..20 {
        frame = picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            0.72 + tick as f64 * 0.02,
            vec![],
        );
    }
    let last = label(&frame, "Artwork 79");
    assert!(
        frame
            .labels
            .iter()
            .any(|(text, pos, clip)| text == "Artwork 79" && clip.contains(*pos))
    );
    let point = last + egui::vec2(0.0, 5.0);
    for (tick, pressed) in [true, false].into_iter().enumerate() {
        frame = picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            1.2 + tick as f64 * 0.02,
            click(point, pressed),
        );
    }
    let point = label(&frame, "Use source") + egui::vec2(10.0, 5.0);
    for (tick, pressed) in [true, false].into_iter().enumerate() {
        picker_frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            size,
            1.3 + tick as f64 * 0.02,
            click(point, pressed),
        );
    }
    assert!(
        desktop.source_picker.is_none(),
        "Picker error: {:?}",
        desktop.error
    );
    let entity = fixture
        .editor
        .document()
        .entity(EntityKind::Achievement, &achievement)
        .unwrap();
    assert_eq!(
        crate::general_images::entries(entity).last().unwrap()["imageId"],
        original.project()["assets"]["images"][79]["id"]
    );
    fixture.undo();
    assert_eq!(fixture.editor.document(), &original);
}

#[test]
fn source_picker_double_click_confirms_an_image_or_atlas_cell_and_undoes_once() {
    for atlas in [false, true] {
        let (mut fixture, achievement) = picker_fixture(atlas);
        let original = fixture.editor.document().clone();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        desktop.open_general_picker(
            &ctx,
            &achievement,
            if atlas {
                SourceKind::Atlas
            } else {
                SourceKind::Image
            },
        );
        let size = egui::vec2(900.0, 760.0);
        let mut frame = PickerFrame::default();
        for tick in 0..4 {
            frame = picker_frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                size,
                tick as f64 * 0.05,
                vec![],
            );
        }
        let mut point = frame.tiles[if atlas { 2 } else { 1 }].0.center();
        if atlas {
            for (tick, pressed) in [true, false].into_iter().enumerate() {
                picker_frame(
                    &ctx,
                    &mut desktop,
                    &mut fixture,
                    size,
                    0.2 + tick as f64 * 0.02,
                    click(point, pressed),
                );
            }
            frame = picker_frame(&ctx, &mut desktop, &mut fixture, size, 0.3, vec![]);
            point = frame
                .images
                .iter()
                .max_by(|a, b| a.area().total_cmp(&b.area()))
                .unwrap()
                .center();
        }
        for (tick, pressed) in [true, false, true, false].into_iter().enumerate() {
            picker_frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                size,
                0.4 + tick as f64 * 0.04,
                click(point, pressed),
            );
        }
        assert!(
            desktop.source_picker.is_none(),
            "Double click did not confirm atlas={atlas}, point={point:?}: {:?}",
            desktop.error
        );
        let entity = fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .unwrap();
        let entry = crate::general_images::entries(entity).pop().unwrap();
        assert_eq!(
            entry[if atlas { "atlasId" } else { "imageId" }],
            original.project()["assets"][if atlas { "atlases" } else { "images" }][1]["id"]
        );
        fixture.undo();
        assert_eq!(fixture.editor.document(), &original);
    }
}

pub(super) fn native_fixture() -> Fixture {
    let (mut fixture, _) = picker_fixture(false);
    let artwork = [
        include_bytes!("../../achimeld-app/assets/achimeld.png").as_slice(),
        include_bytes!("../assets/toolbar/art-general-images.png").as_slice(),
        include_bytes!("../assets/toolbar/layer-gradient.png").as_slice(),
        include_bytes!("../assets/toolbar/layer-contrast.png").as_slice(),
    ]
    .map(|bytes| {
        image::load_from_memory(bytes)
            .unwrap()
            .thumbnail(128, 128)
            .to_rgba8()
    });
    for (index, preview) in fixture.previews.values_mut().enumerate() {
        let image = &artwork[index % artwork.len()];
        preview.width = image.width() as usize;
        preview.height = image.height() as usize;
        preview.rgba = image.as_raw().clone().into();
    }
    fixture
}

#[test]
#[ignore = "Native source chooser screenshot; dedicated external output path"]
fn native_source_picker_screenshot() {
    native_window(false, false, false, false, false);
}

#[test]
fn source_picker_offers_composed_art_as_live_general_images_and_direct_layer_sources() {
    for direct in [false, true] {
        let mut fixture = Fixture::new();
        let mut receiver = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        receiver["icon"] = json!({"layers":[{"id":new_id(),"kind":"StandaloneImage"}]});
        fixture
            .editor
            .execute(
                "Receiver layer",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: receiver["id"].as_str().unwrap().to_owned(),
                    value: receiver,
                },
                None,
            )
            .unwrap();
        fixture
            .editor
            .execute(
                "Create art",
                Command::AddArtImageAt { position: [80, 0] },
                None,
            )
            .unwrap();
        let art = fixture
            .editor
            .document()
            .entities(EntityKind::Achievement)
            .last()
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let achievement = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = fixture.editor.document().clone();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        desktop.selection = vec![achievement.clone()];
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        if direct {
            let layer =
                fixture.editor.document().entities(EntityKind::Achievement)[0]["icon"]["layers"][0]
                    ["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
            desktop.open_source_picker(&ctx, &fixture, &layer, SourceKind::Image);
        } else {
            desktop.open_general_picker(&ctx, &achievement, SourceKind::Image);
        }
        let size = egui::vec2(900.0, 760.0);
        let mut frame = PickerFrame::default();
        for tick in 0..4 {
            frame = picker_frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                size,
                tick as f64 * 0.05,
                vec![],
            );
        }
        let point = label(&frame, "Art image · Art image") + egui::vec2(3.0, 5.0);
        for (tick, pressed) in [true, false].into_iter().enumerate() {
            frame = picker_frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                size,
                0.4 + tick as f64 * 0.05,
                click(point, pressed),
            );
        }
        let point = label(&frame, "Use source") + egui::vec2(10.0, 5.0);
        for (tick, pressed) in [true, false].into_iter().enumerate() {
            picker_frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                size,
                0.6 + tick as f64 * 0.05,
                click(point, pressed),
            );
        }
        assert!(desktop.source_picker.is_none(), "{:?}", desktop.error);
        let entity = fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .unwrap();
        let entry = crate::general_images::entries(entity)
            .last()
            .unwrap()
            .clone();
        assert_eq!(entry["artImageId"], art);
        assert!(entry["imageId"].is_null());
        if direct {
            assert_eq!(entity["icon"]["layers"][0]["kind"], "AchievementImage");
            assert_eq!(
                entity["icon"]["layers"][0]["generalImageIndex"],
                entry["id"]
            );
        }
        fixture.undo();
        assert_eq!(fixture.editor.document(), &before);
    }
}
