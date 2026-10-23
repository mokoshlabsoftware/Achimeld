use achimeld_application::{
    Command, Document, Editor, EntityKind, new_id, particles::ParticleSettings,
};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::sync::Arc;

struct NoAssets;
#[test]
fn particle_axes_rotate_with_the_shape_and_inner_radius_range_changes_the_star() {
    let mut settings = ParticleSettings {
        count: 1,
        radius: 0.0,
        size_min: 128.0,
        size_max: 128.0,
        opacity_min: 1.0,
        opacity_max: 1.0,
        rotation_min: 0.0,
        rotation_max: 0.0,
        softness: 0.0,
        shape_x: 2.0,
        gradient: json!({"kind":"Radial","stops":[{"offset":0.0,"colorHex":"#FFFFFFFF"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}),
        ..Default::default()
    };
    let draw = |s: &ParticleSettings| {
        crate::particles::render(&json!(s), "axes", 128, 128, &|| false).unwrap()
    };
    let wide = draw(&settings);
    assert!(wide.get_pixel(90, 64)[3] > 240);
    assert_eq!(wide.get_pixel(64, 90)[3], 0);
    settings.rotation_min = 90.0;
    settings.rotation_max = 90.0;
    let tall = draw(&settings);
    assert!(tall.get_pixel(64, 90)[3] > 240);
    assert_eq!(tall.get_pixel(90, 64)[3], 0);
    settings.shape_x = 1.0;
    settings.shape = "Star".into();
    let fixed = draw(&settings);
    settings.inner_radius_min = Some(0.05);
    let varied = draw(&settings);
    assert_ne!(fixed, varied);
    assert_eq!(
        varied,
        draw(&settings),
        "Star ranges must be seeded and repeatable"
    );
    settings.inner_radius_min = Some(settings.inner_radius);
    assert_eq!(
        fixed,
        draw(&settings),
        "Collapsed ranges preserve legacy stars"
    );
    settings.count = 512;
    settings.size_min = 512.0;
    settings.size_max = 512.0;
    settings.shape_x = 10.0;
    settings.shape_y = 10.0;
    assert!(
        crate::particles::estimate(&json!(settings), "axes", 512, 512)
            .unwrap_err()
            .contains("budget")
    );
}

#[test]
fn particle_color_map_overlays_the_selected_slice_across_each_particle() {
    use achimeld_application::{gradient_map, particles::layout_for_instance};
    let mut settings = ParticleSettings {
        count: 1,
        radius: 0.0,
        size_min: 256.0,
        size_max: 256.0,
        opacity_min: 1.0,
        opacity_max: 1.0,
        rotation_min: 0.0,
        rotation_max: 0.0,
        softness: 0.0,
        color_mode: "Random".into(),
        color_map: Some(gradient_map::presets().remove(1).1),
        gradient: json!({"kind":"Linear","angle":0.0,"stops":[{"offset":0.0,"colorHex":"#FFFFFFFF"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}),
        ..Default::default()
    };
    let field = settings.color_map.as_ref().unwrap().compile().unwrap();
    let draw = |s: &ParticleSettings, id: &str| {
        crate::particles::render(&json!(s), id, 128, 128, &|| false).unwrap()
    };
    let image = draw(&settings, "one");
    let slice = layout_for_instance(&settings, 1.0, "one")[0].slice;
    for x in [48, 64, 80] {
        let offset = (((f64::from(x) + 0.5 - 64.0) / 32.0 / 2.0 + 0.5) * 255.0).round() / 255.0;
        assert_eq!(image.get_pixel(x, 64).0, field.sample(slice, offset));
    }
    assert_ne!(
        image.get_pixel(48, 64),
        image.get_pixel(80, 64),
        "A particle gets a gradient overlay, rather than a single sampled color"
    );
    assert_eq!(image, draw(&settings, "two"));
    settings.different_seed_per_instance = true;
    assert_ne!(draw(&settings, "one"), draw(&settings, "two"));
    settings.seed += 1;
    assert_ne!(image, draw(&settings, "one"));
}

#[test]
fn particle_sdf_components_combine_cut_out_and_intersect() {
    use achimeld_application::particles::ParticleShape;
    let mut settings = ParticleSettings {
        count: 1,
        radius: 0.0,
        size_min: 256.0,
        size_max: 256.0,
        opacity_min: 1.0,
        opacity_max: 1.0,
        rotation_min: 0.0,
        rotation_max: 0.0,
        softness: 0.0,
        gradient: json!({"kind":"Radial","stops":[{"offset":0.0,"colorHex":"#FFFFFFFF"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}),
        shapes: vec![
            ParticleShape::default(),
            ParticleShape {
                operation: "Subtract".into(),
                scale: 0.5,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let draw = |settings: &ParticleSettings| {
        crate::particles::render(&json!(settings), "composite", 128, 128, &|| false).unwrap()
    };
    let ring = draw(&settings);
    assert_eq!(ring.get_pixel(64, 64)[3], 0);
    assert!(ring.get_pixel(88, 64)[3] > 240);
    settings.shapes[1].operation = "Union".into();
    assert!(draw(&settings).get_pixel(64, 64)[3] > 240);
    settings.shapes[1].operation = "Intersect".into();
    let intersection = draw(&settings);
    assert!(intersection.get_pixel(64, 64)[3] > 240);
    assert_eq!(intersection.get_pixel(88, 64)[3], 0);
    settings.shapes[1].offset_x = 0.4;
    settings.shapes[1].rotation = 45.0;
    settings.shapes[1].kind = "Star".into();
    assert_ne!(draw(&settings), intersection);
    let encoded =
        achimeld_application::particles::ParticlePreset::encode(&json!(settings)).unwrap();
    assert_eq!(
        achimeld_application::particles::ParticlePreset::decode(&encoded).unwrap(),
        json!(settings)
    );
}
impl crate::AssetSource for NoAssets {
    fn image(&mut self, _: &serde_json::Value) -> Result<Arc<RgbaImage>, String> {
        Err("Unexpected artwork request".into())
    }
    fn font(&mut self, _: &str) -> Option<Arc<crate::Font>> {
        None
    }
}

#[test]
fn particle_shapes_softness_gradient_alpha_and_cancellation() {
    let mut settings = ParticleSettings {
        count: 1,
        radius: 0.0,
        size_min: 256.0,
        size_max: 256.0,
        opacity_min: 1.0,
        opacity_max: 1.0,
        rotation_min: 0.0,
        rotation_max: 0.0,
        softness: 0.0,
        gradient: json!({"kind":"Radial","stops":[{"offset":0.0,"colorHex":"#80FF0000"},{"offset":1.0,"colorHex":"#800000FF"}]}),
        ..Default::default()
    };
    let mut images = Vec::new();
    for shape in ["Sphere", "Polygon", "Star"] {
        settings.shape = shape.into();
        let sharp =
            crate::particles::render(&json!(settings), "first", 128, 128, &|| false).unwrap();
        assert!(sharp.get_pixel(64, 64)[0] > 200);
        assert!(sharp.pixels().any(|p| p[2] > 180 && p[3] > 20));
        assert!(sharp.pixels().all(|p| p[3] <= 128));
        settings.softness = 0.8;
        let blurred =
            crate::particles::render(&json!(settings), "first", 128, 128, &|| false).unwrap();
        assert!(
            blurred.pixels().filter(|p| p[3] > 0 && p[3] < 120).count()
                > sharp.pixels().filter(|p| p[3] > 0 && p[3] < 120).count()
        );
        images.push(sharp);
        settings.softness = 0.0;
    }
    assert_ne!(images[0], images[1]);
    assert_ne!(images[1], images[2]);
    assert!(
        crate::particles::render(&json!(settings), "first", 128, 128, &|| true)
            .unwrap_err()
            .contains("cancelled")
    );
    settings.count = 0;
    assert!(
        crate::particles::render(&json!(settings), "first", 128, 128, &|| false)
            .unwrap()
            .pixels()
            .all(|p| p[3] == 0)
    );
    settings.count = 512;
    settings.size_min = 512.0;
    settings.size_max = 512.0;
    settings.vertex_count = 16;
    assert!(
        crate::particles::render(&json!(settings), "first", 512, 512, &|| false)
            .unwrap_err()
            .contains("budget")
    );
}

#[test]
fn particle_per_achievement_seed_survives_history_project_round_trip_preview_and_export() {
    let mut editor = Editor::default();
    for _ in 0..2 {
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
    }
    let ids: Vec<_> = editor
        .document()
        .entities(EntityKind::Achievement)
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    let mut settings = ParticleSettings {
        color_map: Some(achimeld_application::gradient_map::presets().remove(1).1),
        ..Default::default()
    };
    let layer = new_id();
    let set = |editor: &mut Editor, settings: &ParticleSettings| {
        let commands = ids
            .iter()
            .map(|id| {
                let mut value = editor
                    .document()
                    .entity(EntityKind::Achievement, id)
                    .unwrap()
                    .clone();
                value["icon"] =
                    json!({"layers":[{"id":layer,"kind":"Particle","particles":settings}]});
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value,
                }
            })
            .collect();
        editor
            .execute("Particles", Command::Batch(commands), None)
            .unwrap();
    };
    set(&mut editor, &settings);
    let draw = |document: &Document, id: &str| {
        crate::render(document, id, Some((128, 96)), &mut NoAssets)
            .unwrap()
            .achieved
    };
    let shared = draw(editor.document(), &ids[0]);
    assert_eq!(shared, draw(editor.document(), &ids[1]));
    settings.different_seed_per_instance = true;
    set(&mut editor, &settings);
    let first = draw(editor.document(), &ids[0]);
    let second = draw(editor.document(), &ids[1]);
    assert_ne!(first, second);
    assert_ne!(first, shared);
    let restored = Document::from_parts(
        editor.document().project().clone(),
        editor.document().catalog().clone(),
    )
    .unwrap();
    assert_eq!(first, draw(&restored, &ids[0]));
    let preview = crate::render_achieved_cancellable(
        &restored,
        &ids[0],
        Some((128, 96)),
        &mut NoAssets,
        &|| false,
    )
    .unwrap();
    assert_eq!(first, preview);
    assert!(editor.undo());
    assert_eq!(shared, draw(editor.document(), &ids[0]));
    assert!(editor.redo());
    assert_eq!(second, draw(editor.document(), &ids[1]));
}

#[test]
#[ignore = "Writes a visual contact sheet to the external build output directory"]
fn particle_preset_gallery() {
    let mut gallery = RgbaImage::from_pixel(1024, 256, Rgba([28, 30, 36, 255]));
    for (index, (_, settings)) in achimeld_application::particles::presets()
        .iter()
        .enumerate()
    {
        let image =
            crate::particles::render(&json!(settings), "gallery", 256, 256, &|| false).unwrap();
        image::imageops::overlay(&mut gallery, &image, (index * 256) as i64, 0);
    }
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(std::path::PathBuf::from)
        .expect("External artifact root");
    gallery.save(root.join("particle-presets.png")).unwrap();
}
