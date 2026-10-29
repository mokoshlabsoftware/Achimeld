use achimeld_application::{
    Command, Document, Editor, EntityKind,
    explosion::{DeltoidSettings, ExplosionSettings, TriangleSettings},
    new_id,
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn explosion_zero_shape_multipliers_render_without_geometry() {
    for base in [
        ExplosionSettings::default(),
        ExplosionSettings::new_deltoid(),
    ] {
        for (x, y) in [(0.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
            let settings = ExplosionSettings {
                shape_x: x,
                shape_y: y,
                ..base.clone()
            };
            let image =
                crate::explosion::render(&json!(settings), "zero", 32, 32, &|| false).unwrap();
            assert!(image.pixels().all(|pixel| pixel[3] == 0));
        }
    }
}

struct NoAssets;
impl crate::AssetSource for NoAssets {
    fn image(&mut self, _: &serde_json::Value) -> Result<Arc<image::RgbaImage>, String> {
        Err("Unexpected asset".into())
    }
    fn font(&mut self, _: &str) -> Option<Arc<crate::Font>> {
        None
    }
}
fn fixed() -> ExplosionSettings {
    ExplosionSettings {
        count: 9,
        regularity: 0.0,
        aspect_min: 4.0,
        aspect_max: 4.0,
        scale_min: 0.9,
        scale_max: 0.9,
        out: TriangleSettings {
            width_min: 0.3,
            width_max: 0.3,
            height_min: 0.4,
            height_max: 0.4,
            sharpness_min: 1.0,
            sharpness_max: 1.0,
            ..Default::default()
        },
        r#in: TriangleSettings {
            width_min: 0.3,
            width_max: 0.3,
            height_min: 0.4,
            height_max: 0.4,
            sharpness_min: 1.0,
            sharpness_max: 1.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn explosion_bottom_fill_zero_identity_polar_center_and_blur_cancellation() {
    use achimeld_application::explosion::BottomCurve;
    for deltoid in [false, true] {
        let mut settings = fixed();
        if deltoid {
            settings.deltoid = Some(DeltoidSettings {
                width_min: 0.3,
                width_max: 0.3,
                top_height_min: 0.5,
                top_height_max: 0.5,
                bottom_height_min: 0.7,
                bottom_height_max: 0.7,
                sharpness_min: 1.0,
                sharpness_max: 1.0,
                ..Default::default()
            });
        } else {
            settings.r#in.height_min = 0.7;
            settings.r#in.height_max = 0.7;
        }
        let draw = |settings: &ExplosionSettings| {
            crate::explosion::render(&json!(settings), "fill", 128, 128, &|| false).unwrap()
        };
        for blur in [0.0, 0.2] {
            settings.radial_blur = blur;
            settings.angular_blur = blur;
            let original = draw(&settings);
            settings.bottom_curve = BottomCurve::Logarithmic;
            settings.bottom_stretch = 0.9;
            assert_eq!(draw(&settings), original, "Zero power must preserve pixels");
            settings.bottom_power = 9.0;
            settings.bottom_stretch = 0.0;
            assert_eq!(
                draw(&settings),
                original,
                "Zero stretch must preserve pixels"
            );
            settings.bottom_stretch = 0.5;
            let filled = draw(&settings);
            let central_alpha = |image: &image::RgbaImage| {
                image
                    .enumerate_pixels()
                    .filter(|(x, y, _)| (f64::from(*x) - 63.5).hypot(f64::from(*y) - 63.5) < 20.0)
                    .map(|(_, _, pixel)| u64::from(pixel[3]))
                    .sum::<u64>()
            };
            assert!(central_alpha(&filled) > central_alpha(&original));
            let calls = std::cell::Cell::new(0);
            assert!(
                crate::explosion::render(&json!(settings), "fill", 128, 128, &|| {
                    calls.set(calls.get() + 1);
                    calls.get() > 4
                })
                .unwrap_err()
                .contains("cancelled")
            );
            settings.bottom_power = 0.0;
        }
    }
}

#[test]
#[ignore = "Writes bottom fill comparison outside source"]
fn explosion_bottom_fill_gallery() {
    use achimeld_application::explosion::BottomCurve;
    let mut gallery = image::RgbaImage::from_pixel(1024, 512, image::Rgba([28, 30, 36, 255]));
    for index in 0..8 {
        let mut settings = ExplosionSettings {
            count: 9,
            regularity: 0.15,
            aspect_min: 4.0,
            aspect_max: 4.0,
            bottom_power: if index % 4 == 0 { 0.0 } else { 5.0 },
            bottom_stretch: if index < 4 { 0.5 } else { 0.85 },
            bottom_curve: match index % 4 {
                2 => BottomCurve::Quadratic,
                3 => BottomCurve::Logarithmic,
                _ => BottomCurve::Linear,
            },
            ..ExplosionSettings::new_deltoid()
        };
        settings.deltoid = Some(DeltoidSettings {
            width_min: 0.25,
            width_max: 0.4,
            top_height_min: 0.35,
            top_height_max: 0.5,
            bottom_height_min: 0.65,
            bottom_height_max: 0.8,
            sharpness_min: 1.0,
            sharpness_max: 1.0,
            ..Default::default()
        });
        settings.gradient = json!({"kind":"Radial","angle":90.0,"stops":[
            {"offset":0.0,"colorHex":"#FFFFF4D0"},{"offset":0.4,"colorHex":"#FFFFA52E"},{"offset":1.0,"colorHex":"#FFDA3620"}]});
        let image = crate::explosion::render(&json!(settings), "fill gallery", 256, 256, &|| false)
            .unwrap();
        image::imageops::overlay(&mut gallery, &image, (index % 4) * 256, (index / 4) * 256);
    }
    let root = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_ARTIFACT_ROOT").expect("External artifact root"),
    );
    gallery
        .save(root.join("explosion-bottom-fill.png"))
        .unwrap();
}
#[test]
fn explosion_polar_out_in_sharpness_scale_gradient_alpha_and_cancellation() {
    let mut s = fixed();
    let draw = |s: &ExplosionSettings| {
        crate::explosion::render(&json!(s), "fixture", 192, 128, &|| false).unwrap()
    };
    let both = draw(&s);
    assert!(both.pixels().any(|p| p[3] == 255));
    assert_eq!(both.get_pixel(0, 0)[3], 0);
    s.disable_in = true;
    let out = draw(&s);
    s.disable_in = false;
    s.disable_out = true;
    let inward = draw(&s);
    let energy = |image: &image::RgbaImage, outer: bool| {
        image
            .enumerate_pixels()
            .filter(|(x, y, _)| {
                let radius = (f64::from(*x) + 0.5 - 96.0).hypot(f64::from(*y) + 0.5 - 64.0);
                if outer { radius > 32.0 } else { radius < 25.0 }
            })
            .map(|(_, _, p)| u64::from(p[3]))
            .sum::<u64>()
    };
    assert!(energy(&out, true) > 0 && energy(&out, false) == 0);
    assert!(energy(&inward, false) > 0 && energy(&inward, true) == 0);
    s.disable_in = true;
    assert!(draw(&s).pixels().all(|p| p[3] == 0));
    s.disable_out = false;
    s.disable_in = false;
    s.scale_min = 0.4;
    s.scale_max = 0.4;
    let small = draw(&s);
    assert!(
        small.pixels().filter(|p| p[3] > 0).count()
            < both.pixels().filter(|p| p[3] > 0).count() / 2
    );
    s = fixed();
    s.out.sharpness_min = 0.0;
    s.out.sharpness_max = 0.0;
    assert_ne!(draw(&s), both);
    s.gradient = json!({"kind":"Linear","angle":0.0,"stops":[
        {"offset":0.0,"colorHex":"#80FF0000"},{"offset":1.0,"colorHex":"#800000FF"}]});
    let tinted = draw(&s);
    assert!(tinted.pixels().all(|p| p[3] <= 128));
    assert!(tinted.pixels().any(|p| p[0] > p[2] && p[3] > 0));
    assert!(tinted.pixels().any(|p| p[2] > p[0] && p[3] > 0));
    assert!(
        crate::explosion::render(&json!(s), "fixture", 128, 128, &|| true)
            .unwrap_err()
            .contains("cancelled")
    );
    s.count = 100;
    s.out.width_min = 2.0;
    s.out.width_max = 2.0;
    assert!(
        crate::explosion::estimate(&json!(s), "fixture", 4096, 4096)
            .unwrap_err()
            .contains("budget")
    );
}
#[test]
fn explosion_sdf_gradient_preserves_coverage_stop_transparency_and_ignores_angle() {
    let mut settings = fixed();
    settings.deltoid = Some(DeltoidSettings {
        width_min: 0.4,
        width_max: 0.4,
        top_height_min: 0.4,
        top_height_max: 0.4,
        bottom_height_min: 0.4,
        bottom_height_max: 0.4,
        sharpness_min: 1.0,
        sharpness_max: 1.0,
        ..Default::default()
    });
    let draw = |s: &ExplosionSettings| {
        crate::explosion::render(&json!(s), "sdf", 128, 128, &|| false).unwrap()
    };
    let radial = draw(&settings);
    settings.gradient["kind"] = json!("SDF");
    let sdf = draw(&settings);
    assert!(radial != sdf);
    assert!(radial.pixels().zip(sdf.pixels()).all(|(r, s)| r[3] == s[3]));
    assert!(
        sdf.pixels()
            .filter(|p| p[3] == 255 && p[0] > 10 && p[0] < 240)
            .count()
            > 100
    );
    settings.gradient["angle"] = json!(-137.0);
    assert!(draw(&settings) == sdf);
    settings.gradient["stops"] = json!([
        {"offset":1.0,"colorHex":"#800000FF"},
        {"offset":0.25,"colorHex":"#8000FF00"},
        {"offset":0.0,"colorHex":"#80FF0000"}]);
    let tinted = draw(&settings);
    assert!(tinted.pixels().all(|p| p[3] <= 128));
    assert!(
        tinted
            .pixels()
            .any(|p| p[3] == 128 && p[1] > p[0] && p[1] > p[2])
    );
    assert!(tinted.pixels().any(|p| p[3] == 128 && p[2] > p[1]));
    let calls = std::cell::Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() >= 4
    };
    assert!(
        crate::explosion::render(&json!(settings), "sdf", 512, 512, &cancel)
            .unwrap_err()
            .contains("cancelled")
    );
}

#[test]
fn explosion_recipe_history_round_trip_transform_and_preview_export_agree() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let mut s = ExplosionSettings {
        count_min: Some(2),
        different_seed_per_instance: true,
        scale_min: 0.6,
        scale_max: 0.9,
        shape_x: 1.5,
        shape_y: 2.0,
        bottom_power: 3.0,
        bottom_stretch: 0.65,
        bottom_curve: achimeld_application::explosion::BottomCurve::Logarithmic,
        radial_blur: 0.2,
        angular_blur: 0.3,
        deltoid: Some(DeltoidSettings {
            ellipse: 0.65,
            scale: 1.5,
            scale_min: 0.5,
            scale_max: 1.25,
            top_height_min: 0.1,
            top_height_max: 0.25,
            bottom_height_min: 0.3,
            bottom_height_max: 0.6,
            ..Default::default()
        }),
        ..Default::default()
    };
    s.cover_canvas = true;
    s.gradient["kind"] = json!("SDF");
    let layer = json!({"id":new_id(),"kind":"Explosion","explosion":s,"offsetX":40.0,"offsetY":-20.0,"rotation":37.0});
    entity["icon"] = json!({"layers":[layer]});
    editor
        .execute(
            "Explosion",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let restored = Document::from_parts(
        serde_json::from_slice(&serde_json::to_vec(editor.document().project()).unwrap()).unwrap(),
        editor.document().catalog().clone(),
    )
    .unwrap();
    let export = crate::render(&restored, &id, Some((128, 96)), &mut NoAssets)
        .unwrap()
        .achieved;
    let preview =
        crate::render_achieved_cancellable(&restored, &id, Some((128, 96)), &mut NoAssets, &|| {
            false
        })
        .unwrap();
    assert_eq!(preview, export);
    assert_ne!(
        preview,
        crate::explosion::render(&json!(s), &id, 128, 96, &|| false).unwrap()
    );
    assert!(editor.undo());
    assert!(editor.redo());
    assert_eq!(
        crate::render(editor.document(), &id, Some((128, 96)), &mut NoAssets)
            .unwrap()
            .achieved,
        export
    );
    let mut bad = editor.document().project().clone();
    bad["achievements"][0]["icon"]["layers"][0]["explosion"]["count"] = json!(513);
    assert!(Document::from_parts(bad, editor.document().catalog().clone()).is_err());
}

#[test]
#[ignore = "Writes gradient comparison outside source"]
fn explosion_gradient_gallery() {
    let mut gallery = image::RgbaImage::from_pixel(768, 256, image::Rgba([28, 30, 36, 255]));
    let mut settings = fixed();
    settings.cover_canvas = true;
    settings.scale_min = 1.0;
    settings.scale_max = 1.0;
    settings.deltoid = Some(DeltoidSettings {
        width_min: 0.4,
        width_max: 0.4,
        top_height_min: 0.5,
        top_height_max: 0.5,
        bottom_height_min: 0.5,
        bottom_height_max: 0.5,
        sharpness_min: 1.0,
        sharpness_max: 1.0,
        ..Default::default()
    });
    for (index, kind) in ["Radial", "Linear", "SDF"].iter().enumerate() {
        settings.gradient["kind"] = json!(kind);
        let image =
            crate::explosion::render(&json!(settings), "gradient-gallery", 256, 256, &|| false)
                .unwrap();
        image::imageops::overlay(&mut gallery, &image, index as i64 * 256, 0);
    }
    let root = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_ARTIFACT_ROOT").expect("External artifact root"),
    );
    gallery
        .save(root.join("explosion-gradient-gallery.png"))
        .unwrap();
}

#[test]
#[ignore = "Writes morph, blur and axis comparison outside source"]
fn explosion_shape_and_blur_gallery() {
    let mut gallery = image::RgbaImage::from_pixel(768, 768, image::Rgba([28, 30, 36, 255]));
    let mut base = ExplosionSettings {
        count: 9,
        regularity: 0.0,
        aspect_min: 4.0,
        aspect_max: 4.0,
        ..ExplosionSettings::new_deltoid()
    };
    let shape = base.deltoid.as_mut().unwrap();
    shape.width_min = 0.3;
    shape.width_max = 0.3;
    shape.top_height_min = 0.4;
    shape.top_height_max = 0.4;
    shape.bottom_height_min = 0.3;
    shape.bottom_height_max = 0.3;
    shape.sharpness_min = 1.0;
    shape.sharpness_max = 1.0;
    base.gradient["kind"] = json!("SDF");
    for index in 0..9 {
        let mut settings = base.clone();
        match index {
            0..=2 => settings.deltoid.as_mut().unwrap().ellipse = f64::from(index) * 0.5,
            3 => settings.radial_blur = 0.5,
            4 => settings.angular_blur = 0.5,
            5 => {
                settings.radial_blur = 0.4;
                settings.angular_blur = 0.4;
            }
            7 => settings.shape_x = 3.0,
            8 => settings.shape_y = 3.0,
            _ => {}
        }
        let image =
            crate::explosion::render(&json!(settings), "gallery", 256, 256, &|| false).unwrap();
        for (x, y, pixel) in image.enumerate_pixels() {
            let target = gallery.get_pixel_mut(index % 3 * 256 + x, index / 3 * 256 + y);
            let alpha = f64::from(pixel[3]) / 255.0;
            for channel in 0..3 {
                target[channel] = (f64::from(target[channel]) * (1.0 - alpha)
                    + f64::from(pixel[channel]) * alpha)
                    .round() as u8;
            }
        }
    }
    let root = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_ARTIFACT_ROOT").expect("External artifact root"),
    );
    gallery
        .save(root.join("explosion-shape-and-blur.png"))
        .unwrap();
}

#[test]
#[ignore = "Writes explosion visual evidence outside source"]
fn explosion_gallery() {
    let mut gallery = image::RgbaImage::from_pixel(1024, 512, image::Rgba([28, 30, 36, 255]));
    for index in 0..8 {
        let mut settings = fixed();
        settings.deltoid = Some(DeltoidSettings {
            width_min: 0.3,
            width_max: 0.3,
            top_height_min: 0.4,
            top_height_max: 0.4,
            bottom_height_min: 0.4,
            bottom_height_max: 0.4,
            sharpness_min: 1.0,
            sharpness_max: 1.0,
            ..Default::default()
        });
        let shape = settings.deltoid.as_mut().unwrap();
        match index {
            1 => {
                shape.bottom_height_min = 0.1;
                shape.bottom_height_max = 0.1;
            }
            2 => {
                shape.top_height_min = 0.1;
                shape.top_height_max = 0.1;
            }
            3 => {
                settings.regularity = 1.0;
                shape.sharpness_min = 0.2;
                shape.sharpness_max = 0.5;
            }
            4 => {
                shape.top_height_min = 0.0;
                shape.top_height_max = 0.0;
            }
            5 => {
                shape.scale_min = 0.25;
                shape.scale_max = 1.75;
            }
            6 => {
                shape.scale = 0.75;
            }
            7 => {
                shape.sharpness_min = 0.1;
                shape.sharpness_max = 0.1;
            }
            _ => {}
        }
        let image =
            crate::explosion::render(&json!(settings), "gallery", 256, 256, &|| false).unwrap();
        image::imageops::overlay(&mut gallery, &image, (index % 4) * 256, (index / 4) * 256);
    }
    let root = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_ARTIFACT_ROOT").expect("External artifact root"),
    );
    gallery.save(root.join("explosion-gallery.png")).unwrap();
}
