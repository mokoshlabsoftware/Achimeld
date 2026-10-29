use crate::{AssetSource, render};
use achimeld_application::{Command, Editor, EntityKind, new_id};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
struct Assets;
#[test]
fn locked_artwork_is_derived_from_current_icon_despite_legacy_override() {
    struct Imported;
    impl AssetSource for Imported {
        fn image(&mut self, _: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            panic!("A separate locked artwork asset must never be loaded");
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            None
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut project = editor.document().project().clone();
    let image_id = new_id();
    let id = project["achievements"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    project["achievements"][0]["icon"] =
        json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF0000"}]});
    project["achievements"][0]["unachievedImageId"] = json!(image_id);
    project["assets"]["images"] = json!([{"id":image_id,"kind":"SteamImportedArtwork","displayName":"Locked","image":{"kind":"RelativeToProject","path":"locked.png"}}]);
    let document =
        achimeld_application::Document::from_parts(project, editor.document().catalog().clone())
            .unwrap();
    let pair = render(&document, &id, Some((16, 12)), &mut Imported).unwrap();
    assert_eq!(pair.unachieved.dimensions(), (16, 12));
    assert_eq!(*pair.unachieved.get_pixel(0, 0), Rgba([54, 54, 54, 255]));
    assert_eq!(*pair.achieved.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}
#[test]
fn sdf_vignette_inversion_complements_legacy_coverage() {
    for (width, height) in [(16, 16), (48, 16), (16, 48)] {
        for (roundness, inset, softness) in [(0.0, 0.0, 0.0), (0.84, 0.15, 1.0), (1.0, 0.95, 0.2)] {
            let mut editor = Editor::default();
            editor
                .execute("Add", Command::Add(EntityKind::Achievement), None)
                .unwrap();
            let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
            let id = entity["id"].as_str().unwrap().to_owned();
            entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"SdfVignette",
                "sdfVignette":{"roundness":roundness,"inset":inset,"softness":softness}}]});
            let mut outputs = Vec::new();
            for invert in [false, true] {
                entity["icon"]["layers"][0]["sdfVignette"]["invert"] = json!(invert);
                editor
                    .execute(
                        "Recipe",
                        Command::ReplaceEntity {
                            kind: EntityKind::Achievement,
                            id: id.clone(),
                            value: entity.clone(),
                        },
                        None,
                    )
                    .unwrap();
                let preview = crate::render_achieved_cancellable(
                    editor.document(),
                    &id,
                    Some((width, height)),
                    &mut Assets,
                    &|| false,
                )
                .unwrap();
                let exported = render(editor.document(), &id, Some((width, height)), &mut Assets)
                    .unwrap()
                    .achieved;
                assert_eq!(preview, exported);
                outputs.push(preview);
            }
            for (a, b) in outputs[0].pixels().zip(outputs[1].pixels()) {
                assert!((i16::from(a[3]) + i16::from(b[3]) - 255).abs() <= 1);
            }
        }
    }
    let legacy = icon(json!([{"id":new_id(),"kind":"SdfVignette"}]));
    let explicit = icon(json!([{"id":new_id(),"kind":"SdfVignette",
        "sdfVignette":{"invert":false,"useGradient":false,"colorHex":"#FF000000","roundness":0.5,"inset":0.15,"softness":0.2}}]));
    assert_eq!(legacy, explicit);
    assert_eq!(legacy.get_pixel(8, 8)[3], 0);
    assert_eq!(legacy.get_pixel(0, 0)[3], 255);
}

#[test]
fn sdf_vignette_maps_the_fade_in_both_directions_and_respects_stop_alpha() {
    for invert in [false, true] {
        let mut layer = json!({"id":new_id(),"kind":"SdfVignette","sdfVignette":{
            "invert":invert,"roundness":0.84,"softness":1.0,"inset":0.15,"useGradient":true,
            "gradient":{"stops":[{"offset":1.0,"colorHex":"#800000FF"},
                {"offset":0.0,"colorHex":"#80FF0000"},{"offset":0.5,"colorHex":"#8000FF00"}]}}});
        let mapped = icon(json!([layer]));
        layer["sdfVignette"]["useGradient"] = json!(false);
        let coverage = icon(json!([layer]));
        for (color, mask) in mapped.pixels().zip(coverage.pixels()) {
            let t = f64::from(mask[3]) / 255.0;
            let expected = if t < 0.5 {
                [255.0 * (1.0 - t * 2.0), 255.0 * t * 2.0, 0.0]
            } else {
                [0.0, 255.0 * (2.0 - t * 2.0), 255.0 * (t * 2.0 - 1.0)]
            };
            if color[3] > 0 {
                for channel in 0..3 {
                    assert!((f64::from(color[channel]) - expected[channel]).abs() <= 2.0);
                }
            }
            assert!((f64::from(color[3]) - f64::from(mask[3]) * 128.0 / 255.0).abs() <= 1.0);
        }
        assert!(mapped.pixels().any(|p| p[0] > 50 && p[3] > 0));
        assert!(mapped.pixels().any(|p| p[1] > 100 && p[3] > 0));
        assert!(mapped.pixels().any(|p| p[2] > 100 && p[3] > 0));
    }
}

#[test]
fn sdf_vignette_single_stop_matches_solid_fill_with_layer_opacity_and_mask() {
    let mut layer = json!({"id":new_id(),"kind":"SdfVignette","opacity":0.4,
        "sdfVignette":{"invert":true,"colorHex":"#804080C0","useGradient":true,
            "gradient":{"stops":[{"offset":0.3,"colorHex":"#804080C0"}]}}});
    let mask = json!({"id":new_id(),"kind":"SdfVignette"});
    layer["maskLayerId"] = mask["id"].clone();
    let mapped = icon(json!([layer, mask]));
    layer["sdfVignette"]["useGradient"] = json!(false);
    assert_eq!(mapped, icon(json!([layer, mask])));
    assert!(mapped.pixels().any(|p| p[3] > 0));
}

#[test]
fn wide_shape_rotation_preserves_width_and_height_in_canvas_space() {
    let recipe = |kind| {
        json!([{"id":new_id(),"kind":"Shape","shape":{"kind":kind,
        "scaleX":1.37,"scaleY":0.21,"rotation":45,"colorHex":"#FFFFFFFF"}}])
    };
    let image = icon(recipe(json!("Rectangle")));
    assert_eq!(image.get_pixel(3, 3)[3], 255);
    assert_eq!(image.get_pixel(12, 12)[3], 255);
    assert_eq!(image.get_pixel(3, 8)[3], 0);
    assert_eq!(image, icon(recipe(json!(0))));
    assert!(
        image.pixels().any(|p| p[3] > 0 && p[3] < 255),
        "Vector edges need antialiasing"
    );
}
#[test]
fn rotated_layers_keep_the_pixel_center_and_resample_premultiplied_colors() {
    let gradient = json!({"kind":"Linear","angle":0,"stops":[{"offset":0,"colorHex":"#FFFF0000"},{"offset":1,"colorHex":"#FF0000FF"}]});
    let original = icon(json!([{"id":new_id(),"kind":"Gradient","gradient":gradient}]));
    let rotated =
        icon(json!([{"id":new_id(),"kind":"Gradient","gradient":gradient,"rotation":180}]));
    assert_eq!(rotated, image::imageops::rotate180(&original));
    let translated =
        icon(json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000","offsetX":16}]));
    assert_eq!(*translated.get_pixel(0, 8), Rgba([255, 0, 0, 128]));
    assert!(
        translated
            .pixels()
            .all(|p| p[0] == 255 && p[1] == 0 && p[2] == 0)
    );
}
#[test]
fn positive_contrast_uses_the_legacy_reciprocal_curve() {
    let image = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FF406080"},
        {"id":new_id(),"kind":"Contrast","adjustment":{"contrast":50}}]),
    );
    assert_eq!(*image.get_pixel(8, 8), Rgba([0, 65, 129, 255]));
}
#[test]
fn outlines_split_odd_widths_and_preserve_translucent_artwork() {
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        if (10..22).contains(&x) && (10..22).contains(&y) {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0; 4])
        }
    });
    let image=crate::effects::apply(source,&json!({"effects":[{"kind":"Outline","colorHex":"#FF0000FF","width":48,"placement":"Center"}]}),32,32,&||false,None).unwrap();
    assert_eq!(*image.get_pixel(8, 16), Rgba([0, 0, 255, 255]));
    assert_eq!(image.get_pixel(7, 16)[3], 0);
    assert_eq!(*image.get_pixel(10, 16), Rgba([0, 0, 255, 255]));
    assert_eq!(*image.get_pixel(11, 16), Rgba([255, 0, 0, 255]));
    let source = RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 128]));
    let image=crate::effects::apply(source,&json!({"effects":[{"kind":"Outline","colorHex":"#FF0000FF","width":32,"placement":"Inward"}]}),32,32,&||false,None).unwrap();
    assert_eq!(
        image.get_pixel(0, 16)[3],
        128,
        "Inside outlines preserve artwork alpha"
    );
}
#[test]
fn round_outline_uses_a_disk_and_directional_inner_light_protects_the_outline() {
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        if x == 16 && y == 16 {
            Rgba([255; 4])
        } else {
            Rgba([0; 4])
        }
    });
    let mut layer = json!({"effects":[{"kind":"Outline","colorHex":"#FF0000FF","width":48,"placement":"Outward","outlineShape":"Round"}]});
    let round = crate::effects::apply(source.clone(), &layer, 32, 32, &|| false, None).unwrap();
    assert_eq!(round.get_pixel(19, 19)[3], 0);
    assert_eq!(round.get_pixel(19, 16)[3], 255);
    layer["effects"][0]["outlineShape"] = json!("Square");
    let square = crate::effects::apply(source, &layer, 32, 32, &|| false, None).unwrap();
    assert_eq!(square.get_pixel(19, 19)[3], 255);
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        if (8..24).contains(&x) && (8..24).contains(&y) {
            Rgba([64, 64, 64, 255])
        } else {
            Rgba([0; 4])
        }
    });
    let effects = json!({"effects":[{"kind":"Outline","colorHex":"#FF0000FF","width":32,"placement":"Outward"},
        {"kind":"InnerLight","colorHex":"#FFFFFFFF","offsetX":0,"offsetY":64,"blurEnabled":false,"affectOutline":false}]});
    let image = crate::effects::apply(source, &effects, 32, 32, &|| false, None).unwrap();
    assert_eq!(*image.get_pixel(16, 9), Rgba([255; 4]));
    assert_eq!(*image.get_pixel(16, 22), Rgba([64, 64, 64, 255]));
    assert_eq!(*image.get_pixel(16, 6), Rgba([0, 0, 255, 255]));
}

#[test]
fn text_has_edge_padding_and_vector_outlines_without_changing_alignment() {
    let font = crate::Font::from_bytes(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap();
    let fill = font
        .text(
            "1",
            64.0,
            Rgba([255; 4]),
            "BottomRight",
            (128, 128),
            &|| false,
        )
        .unwrap();
    assert!(
        fill.enumerate_pixels()
            .filter(|(_, _, p)| p[3] > 0)
            .all(|(x, y, _)| x < 121 && y < 121)
    );
    let outline = font
        .text_outline(
            "1",
            64.0,
            Rgba([255; 4]),
            "BottomRight",
            (128, 128),
            &|| false,
            4.0,
            "Outward",
        )
        .unwrap();
    assert!(
        fill.pixels()
            .zip(outline.pixels())
            .any(|(f, o)| f[3] == 0 && o[3] > 0)
    );
    assert!(
        fill.pixels()
            .zip(outline.pixels())
            .all(|(f, o)| f[3] < 255 || o[3] == 0)
    );
    let multiline = font
        .text("1\n1", 32.0, Rgba([255; 4]), "Center", (128, 128), &|| {
            false
        })
        .unwrap();
    assert!(
        multiline
            .enumerate_pixels()
            .any(|(_, y, p)| y < 64 && p[3] > 0)
    );
    assert!(
        multiline
            .enumerate_pixels()
            .any(|(_, y, p)| y > 64 && p[3] > 0)
    );
}
#[test]
fn inner_light_blur_has_independent_opacity_and_outline_blur_stays_protected() {
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        if (8..24).contains(&x) && (8..24).contains(&y) {
            Rgba([32, 32, 32, 128])
        } else {
            Rgba([0; 4])
        }
    });
    let layer = json!({"effects":[{"kind":"InnerLight","opacity":0,"offsetX":0,"offsetY":64,
        "blur":64,"blurEnabled":true,"blurColorHex":"#FFFF0000","blurOpacity":1,"blurBlendMode":"Screen"}]});
    let lit = crate::effects::apply(source.clone(), &layer, 32, 32, &|| false, None).unwrap();
    assert!(lit.get_pixel(16, 9)[0] > 32);
    assert_eq!(lit.get_pixel(16, 9)[1], 32);
    assert!(lit.pixels().zip(source.pixels()).all(|(a, b)| a[3] == b[3]));
    let layer = json!({"effects":[{"kind":"Outline","colorHex":"#FF0000FF","width":32,"placement":"Outward"},
        {"kind":"BoxBlur","blur":64,"affectOutline":false}]});
    let blurred = crate::effects::apply(source, &layer, 32, 32, &|| false, None).unwrap();
    let ring = blurred.get_pixel(16, 6);
    assert!(ring[2] > ring[0] && ring[2] > ring[1]);
    assert!(crate::effects::apply(RgbaImage::new(32, 32), &layer, 32, 32, &|| true, None).is_err());
}
#[test]
fn missing_art_keeps_other_preview_layers_and_masks_but_export_reports_it() {
    struct Missing;
    impl AssetSource for Missing {
        fn image(&mut self, _: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            Err("Missing image".into())
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            None
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let mask = new_id();
    entity["icon"] = json!({"layers":[
        {"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"},
        {"id":new_id(),"kind":"AtlasTile"},
        {"id":new_id(),"kind":"StandaloneImage","imageId":new_id()},
        {"id":new_id(),"kind":"Solid","solidColorHex":"#FF00FF00","maskLayerId":mask},
        {"id":mask,"kind":"StandaloneImage","imageId":new_id(),"visible":false}
    ]});
    editor
        .execute(
            "Art",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let preview = crate::render_achieved_cancellable(
        editor.document(),
        &id,
        Some((16, 16)),
        &mut Missing,
        &|| false,
    )
    .unwrap();
    assert!(preview.pixels().all(|p| *p == Rgba([255, 0, 0, 255])));
    assert!(render(editor.document(), &id, Some((16, 16)), &mut Missing).is_err());
    assert!(
        crate::render_achieved_cancellable(
            editor.document(),
            &id,
            Some((16, 16)),
            &mut Missing,
            &|| true
        )
        .is_err()
    );
}
#[test]
fn displayed_composition_matches_export_and_obeys_cancellation() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#80FF2040"}]});
    editor
        .execute(
            "Icon",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let pair = render(editor.document(), &id, Some((128, 64)), &mut Assets).unwrap();
    let preview = crate::render_achieved_cancellable(
        editor.document(),
        &id,
        Some((128, 64)),
        &mut Assets,
        &|| false,
    )
    .unwrap();
    assert_eq!(preview, pair.achieved);
    assert!(
        crate::render_achieved_cancellable(
            editor.document(),
            &id,
            Some((128, 64)),
            &mut Assets,
            &|| true
        )
        .is_err()
    );
}
impl AssetSource for Assets {
    fn image(&mut self, _: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
        Ok(std::sync::Arc::new(RgbaImage::new(1, 1)))
    }
    fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
        None
    }
}
#[test]
fn perlin_masks_use_brightness_and_preserve_legacy_alpha_masks() {
    let source = json!({"id":new_id(),"kind":"PerlinNoise","visible":false});
    let owner = json!({"id":new_id(),"kind":"Solid","solidColorHex":"#FF80A0C0","maskLayerId":source["id"]});
    let legacy = icon(json!([owner, source]));
    assert!(legacy.pixels().all(|p| p[3] == 255));
    let mut luminance = owner.clone();
    luminance["maskMode"] = json!("Luminance");
    let masked = icon(json!([luminance, source]));
    assert!(masked.pixels().any(|p| p[3] < 160));
    assert!(masked.pixels().any(|p| p[3] > 100));
    assert!(masked.pixels().all(|p| p.0[0..3] == [128, 160, 192]));
    let noise = icon(json!([{"id":new_id(),"kind":"PerlinNoise"}]));
    for (pixel, noise) in masked.pixels().zip(noise.pixels()) {
        assert_eq!(pixel[3], noise[0]);
    }
    luminance["maskInvert"] = json!(true);
    let inverted = icon(json!([luminance, source]));
    for (a, b) in masked.pixels().zip(inverted.pixels()) {
        assert_eq!(u16::from(a[3]) + u16::from(b[3]), 255);
    }
    let mut half = source.clone();
    half["opacity"] = json!(0.5);
    let half_mask = icon(json!([luminance, half]));
    for (a, b) in inverted.pixels().zip(half_mask.pixels()) {
        assert!((f64::from(a[3]) * 0.5 - f64::from(b[3])).abs() <= 0.5);
    }
}

#[test]
fn adjustments_accept_masks_and_can_be_used_as_mask_sources() {
    let bottom = json!({"id":new_id(),"kind":"Solid","solidColorHex":"#FF804020"});
    let source = json!({"id":new_id(),"kind":"Solid","solidColorHex":"#FF000000"});
    let mut adjustment = json!({"id":new_id(),"kind":"Contrast","adjustment":{"brightness":0.5},"maskLayerId":source["id"],"maskMode":"Luminance"});
    let original = icon(json!([bottom]));
    assert_eq!(original, icon(json!([bottom, source, adjustment])));
    adjustment["maskInvert"] = json!(true);
    let changed = icon(json!([bottom, source, adjustment]));
    assert_ne!(original, changed);
    let mut full = adjustment.clone();
    full["maskLayerId"] = Value::Null;
    assert_eq!(changed, icon(json!([bottom, full])));
    // A standalone adjustment mask processes an immutable backdrop snapshot.
    let adjustment_source =
        json!({"id":new_id(),"kind":"Contrast","adjustment":{"brightness":-100.0}});
    let owner = json!({"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000","maskLayerId":adjustment_source["id"],"maskMode":"Luminance"});
    assert_eq!(original, icon(json!([bottom, adjustment_source, owner])));
}

fn icon(layers: Value) -> RgbaImage {
    icon_with_assets(layers, &mut Assets)
}

#[test]
fn reference_masks_copy_coverage_and_follow_source_changes() {
    let mask = new_id();
    let source = new_id();
    let reference = new_id();
    let target = new_id();
    for mode in ["Alpha", "Luminance"] {
        for invert in [false, true] {
            for opacity in [0.25, 1.0] {
                let mut layers = json!([
                    {"id":mask,"kind":"Solid","solidColorHex":"#80808080","opacity":opacity,"visible":false},
                    {"id":source,"kind":"Solid","solidColorHex":"#FFFF0000","opacity":0.1,"visible":false,
                     "maskLayerId":mask,"maskMode":mode,"maskInvert":invert},
                    {"id":reference,"kind":"MaskReference","maskSourceLayerId":source,"visible":false},
                    {"id":target,"kind":"Solid","solidColorHex":"#FF80A0C0","maskLayerId":reference,"maskMode":"Luminance"}
                ]);
                let copied = icon(layers.clone());
                layers[3]["maskLayerId"] = json!(mask);
                layers[3]["maskMode"] = json!(mode);
                layers[3]["maskInvert"] = json!(invert);
                assert_eq!(copied, icon(layers.clone()));
                layers[0]["solidColorHex"] = json!("#FF000000");
                layers[3]["maskLayerId"] = json!(reference);
                layers[3]["maskMode"] = json!("Luminance");
                layers[3]["maskInvert"] = json!(false);
                assert_ne!(copied, icon(layers));
            }
        }
    }
}

#[test]
fn unassigned_reference_masks_and_sources_without_masks_fully_reveal() {
    let source = new_id();
    let reference = new_id();
    for assigned in [false, true] {
        let layers = json!([
            {"id":source,"kind":"Solid","solidColorHex":"#FFFF0000","visible":false},
            {"id":reference,"kind":"MaskReference","maskSourceLayerId":if assigned { json!(source) } else { Value::Null },"visible":false},
            {"id":new_id(),"kind":"Solid","solidColorHex":"#FF80A0C0","maskLayerId":reference,"maskMode":"Luminance"}
        ]);
        assert!(icon(layers).pixels().all(|p| p.0 == [128, 160, 192, 255]));
    }
}

#[test]
fn colored_mask_artwork_preserves_channels_and_legacy_luminance_coverage() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let mask = new_id();
    entity["icon"] = json!({"layers":[
        {"id":mask,"kind":"Solid","solidColorHex":"#FFFF0000","effects":[{"kind":"ColorOverlay","colorHex":"#FF00FF00"}]},
        {"id":new_id(),"kind":"Solid","maskLayerId":mask,"maskMode":"Luminance"}
    ]});
    editor
        .execute(
            "Mask",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let thumbnail =
        crate::render_layer_cancellable(editor.document(), &id, &mask, &mut Assets, &|| false)
            .unwrap();
    assert!(
        thumbnail
            .pixels()
            .all(|p| p[0] == 0 && p[1] == 255 && p[2] == 0)
    );
    let output = render(editor.document(), &id, Some((16, 16)), &mut Assets).unwrap();
    assert_eq!(
        output.achieved.get_pixel(8, 8)[3],
        achimeld_application::masks::grayscale(thumbnail.get_pixel(64, 64).0)[0]
    );
}
fn icon_with_assets(layers: Value, assets: &mut dyn AssetSource) -> RgbaImage {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":layers});
    editor
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    render(editor.document(), &id, Some((16, 16)), assets)
        .unwrap()
        .achieved
}
#[test]
fn saved_text_outline_enabled_and_effect_outline_share_vector_coverage() {
    struct FontAssets;
    impl AssetSource for FontAssets {
        fn image(&mut self, _: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            Err("No images".into())
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            Some(std::sync::Arc::new(
                crate::Font::from_bytes(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap(),
            ))
        }
    }
    let mut layer = json!({"id":new_id(),"kind":"Text","text":{"content":"1","fontSize":256,
        "colorHex":"#FFFFFFFF","alignment":"Center","outlineEnabled":true,
        "outlineWidth":32,"outlineColorHex":"#FFFF0000","outlinePlacement":"Outward"}});
    let legacy = icon_with_assets(json!([layer.clone()]), &mut FontAssets);
    assert!(legacy.pixels().any(|p| p[0] > p[1] && p[3] > 0));
    layer["text"]["outlineEnabled"] = json!(false);
    layer["effects"] =
        json!([{"kind":"Outline","colorHex":"#FFFF0000","width":32,"placement":"Outward"}]);
    let effect = icon_with_assets(json!([layer]), &mut FontAssets);
    assert_eq!(legacy, effect);
}
#[test]
fn unicode_color_is_an_error_and_never_a_slice_panic() {
    for color in ["#ééé", "#🔥ab", "#１２34", "#abcd"] {
        assert!(crate::renderer::rgba(color).is_err());
    }
    assert_eq!(
        crate::renderer::rgba("#80ff0000").unwrap(),
        Rgba([255, 0, 0, 128])
    );
}
#[test]
fn folder_visibility_opacity_and_masks_control_only_their_children() {
    let folder = new_id();
    let mask = new_id();
    let layer = new_id();
    let image = icon(
        json!([{"id":folder,"kind":"Folder","opacity":0.5,"maskLayerId":mask},{"id":layer,"kind":"Solid","solidColorHex":"#FFFF0000","parentFolderId":folder},{"id":mask,"kind":"Shape","shape":{"kind":"Sphere","scaleX":0.5,"scaleY":0.5,"colorHex":"#FFFFFFFF"}}]),
    );
    assert_eq!(image.get_pixel(0, 0)[3], 0);
    assert_eq!(*image.get_pixel(8, 8), Rgba([255, 0, 0, 128]));
    let image = icon(
        json!([{"id":folder,"kind":"Folder","visible":false},{"id":layer,"kind":"Solid","solidColorHex":"#FFFF0000","parentFolderId":folder}]),
    );
    assert!(image.pixels().all(|p| p[3] == 0));
}
#[test]
fn hue_shift_and_grayscale_export_use_the_same_composition() {
    let image = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"},{"id":new_id(),"kind":"HueSaturation","adjustment":{"hue":120.0}}]),
    );
    assert_eq!(*image.get_pixel(0, 0), Rgba([0, 255, 0, 255]));
}
#[test]
fn box_blur_uses_premultiplied_colors_without_transparent_color_fringe() {
    let image = icon(
        json!([{"id":new_id(),"kind":"Shape","shape":{"kind":"Rectangle","scaleX":0.5,"scaleY":0.5,"colorHex":"#FFFF0000"},"effects":[{"kind":"BoxBlur","blur":64.0}]}]),
    );
    assert!(
        image
            .pixels()
            .filter(|p| p[3] > 0)
            .all(|p| p[0] == 255 && p[1] == 0 && p[2] == 0)
    );
}
#[test]
fn mask_opacity_and_color_overlay_preserve_translucent_coverage() {
    let mask = new_id();
    let masked = icon(json!([
        {"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000","maskLayerId":mask},
        {"id":mask,"kind":"Solid","solidColorHex":"#FFFFFFFF","opacity":0.5}
    ]));
    assert_eq!(*masked.get_pixel(8, 8), Rgba([255, 0, 0, 128]));
    let overlay = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#80000000","effects":[{"kind":"ColorOverlay","colorHex":"#80FFFFFF"}]}]),
    );
    assert_eq!(*overlay.get_pixel(8, 8), Rgba([128, 128, 128, 128]));
}
#[test]
fn maintained_font_renderer_renders_unicode_and_rejects_malformed_fonts() {
    assert!(crate::Font::from_bytes(vec![0; 64]).is_err());
    let font = crate::Font::from_bytes(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap();
    let image = font
        .text(
            "Żółć",
            24.0,
            Rgba([13, 21, 34, 128]),
            "TopLeft",
            (128, 64),
            &|| false,
        )
        .unwrap();
    assert!(image.pixels().any(|pixel| pixel[3] > 0));
    assert!(
        image
            .pixels()
            .filter(|pixel| pixel[3] > 0)
            .all(|pixel| pixel.0[0..3] == [13, 21, 34] && pixel[3] <= 128)
    );
    assert!(image.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 128));
}
#[test]
fn branching_folder_mask_graph_has_a_total_render_work_limit() {
    let folders: Vec<_> = (0..13).map(|_| new_id()).collect();
    let mut layers = vec![json!({"id":new_id(),"kind":"Solid","maskLayerId":folders[0]})];
    for (index, folder) in folders.iter().enumerate() {
        layers.push(json!({"id":folder,"kind":"Folder"}));
        if let Some(next) = folders.get(index + 1) {
            for _ in 0..2 {
                layers.push(json!({"id":new_id(),"kind":"Solid","parentFolderId":folder,"maskLayerId":next}));
            }
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":layers});
    editor
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let error = render(editor.document(), &id, Some((16, 16)), &mut Assets)
        .err()
        .unwrap();
    assert!(error.contains("work budget"));
    assert!(
        crate::render_cancellable(editor.document(), &id, Some((16, 16)), &mut Assets, &|| {
            true
        })
        .err()
        .unwrap()
        .contains("cancelled")
    );
}
#[test]
fn configured_unachieved_tint_and_opacity_are_not_silently_ignored() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#80FF0000"}]});
    let mut atlas = editor.document().project()["atlas"].clone();
    atlas["unachievedIcon"] = json!({"mode":"Tint","opacity":0.5,"tintHex":"#0000FF"});
    editor
        .execute(
            "Configure",
            Command::Batch(vec![
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                Command::ProjectSettings {
                    name: "Tint test".into(),
                    app_id: 0,
                    atlas: Some(atlas),
                    assets: None,
                },
            ]),
            None,
        )
        .unwrap();
    let pair = render(editor.document(), &id, Some((16, 16)), &mut Assets).unwrap();
    assert_eq!(*pair.achieved.get_pixel(0, 0), Rgba([255, 0, 0, 128]));
    assert_eq!(*pair.unachieved.get_pixel(0, 0), Rgba([27, 27, 155, 64]));
}
#[test]
fn adjustment_layers_honor_their_selected_blend_mode() {
    for kind in ["Contrast", "Curves", "HueSaturation", "UnsharpMask"] {
        let image = icon(
            json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FF808080"},{"id":new_id(),"kind":kind,"blendMode":"Multiply"}]),
        );
        assert_eq!(*image.get_pixel(0, 0), Rgba([64, 64, 64, 255]), "{kind}");
    }
}
#[test]
fn missing_named_atlas_and_unknown_blend_mode_do_not_use_substitutes() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"AtlasTile","atlasId":new_id()}]});
    editor
        .execute(
            "Missing atlas",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity.clone(),
            },
            None,
        )
        .unwrap();
    assert!(
        render(editor.document(), &id, Some((16, 16)), &mut Assets)
            .err()
            .unwrap()
            .contains("missing")
    );
    entity["icon"] =
        json!({"layers":[{"id":new_id(),"kind":"Solid","blendMode":"UnsupportedFutureMode"}]});
    editor
        .execute(
            "Unsupported blend",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    assert!(
        render(editor.document(), &id, Some((16, 16)), &mut Assets)
            .err()
            .unwrap()
            .contains("Unsupported blend")
    );
}

#[test]
fn gradient_effect_maps_selected_channel_and_preserves_alpha() {
    let image = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#8000FF00","effects":[{"id":new_id(),"kind":"GradientOverlay","gradient":{"inputChannel":"Green","stops":[{"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFF0000"}]}}]}]),
    );
    assert!(image.pixels().all(|p| *p == Rgba([255, 0, 0, 128])));
}
#[test]
fn layer_thumbnail_keeps_hidden_artwork_and_applies_folder_mask() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut e = editor.document().entities(EntityKind::Achievement)[0].clone();
    let achievement = e["id"].as_str().unwrap().to_owned();
    let folder = new_id();
    let child = new_id();
    let mask = new_id();
    e["icon"] = json!({"layers":[{"id":mask,"kind":"Solid","solidColorHex":"#80FFFFFF","visible":false},{"id":child,"kind":"Solid","solidColorHex":"#FFFF0000","parentFolderId":folder},{"id":folder,"kind":"Folder","maskLayerId":mask,"visible":false}]});
    editor
        .execute(
            "Layers",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: achievement.clone(),
                value: e,
            },
            None,
        )
        .unwrap();
    let thumb = crate::render_layer_cancellable(
        editor.document(),
        &achievement,
        &folder,
        &mut Assets,
        &|| false,
    )
    .unwrap();
    assert!(thumb.pixels().all(|p| *p == Rgba([255, 0, 0, 128])));
    assert!(
        crate::render_layer_cancellable(
            editor.document(),
            &achievement,
            &folder,
            &mut Assets,
            &|| true
        )
        .is_err()
    );
}
#[test]
fn selective_color_range_does_not_change_unrelated_hues() {
    let red = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"},{"id":new_id(),"kind":"SelectiveColor","adjustment":{"selectiveColors":[{"range":"Reds","cyan":100}]}}]),
    );
    let green = icon(
        json!([{"id":new_id(),"kind":"Solid","solidColorHex":"#FF00FF00"},{"id":new_id(),"kind":"SelectiveColor","adjustment":{"selectiveColors":[{"range":"Reds","cyan":100}]}}]),
    );
    assert_eq!(*red.get_pixel(0, 0), Rgba([0, 0, 0, 255]));
    assert_eq!(*green.get_pixel(0, 0), Rgba([0, 255, 0, 255]));
}

#[test]
fn general_image_default_atlas_entry_resolves_like_the_explorer() {
    struct Atlas;
    impl AssetSource for Atlas {
        fn image(&mut self, reference: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            assert_eq!(reference["path"], "atlas.png");
            Ok(std::sync::Arc::new(RgbaImage::from_pixel(
                16,
                16,
                Rgba([240, 80, 20, 255]),
            )))
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            None
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Fixture", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut project = editor.document().project().clone();
    project["atlas"]["tileWidth"] = json!(16);
    project["atlas"]["tileHeight"] = json!(16);
    project["assets"]["atlas"] = json!({"kind":"RelativeToProject","path":"atlas.png"});
    let entity = &mut project["achievements"][0];
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["generalImageEntries"] = json!([{"id":0,"imageId":null,"atlasId":"00000000-0000-0000-0000-000000000000","tileIndex":0}]);
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0,"visible":true,"opacity":1.0}]});
    let document =
        achimeld_application::Document::from_parts(project, editor.document().catalog().clone())
            .unwrap();
    let result = render(&document, &id, Some((16, 16)), &mut Atlas).unwrap();
    assert_eq!(*result.achieved.get_pixel(8, 8), Rgba([240, 80, 20, 255]));
}

#[test]
fn intentionally_empty_image_layers_render_and_export_without_resolving_art() {
    struct Empty;
    impl AssetSource for Empty {
        fn image(&mut self, _: &Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            panic!("Empty sources must never resolve a default image");
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            None
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut a = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = a["id"].as_str().unwrap().to_owned();
    a["generalImageEntries"] = json!([{"id":0,"imageId":null,"atlasId":null}]);
    a["icon"] = json!({"layers":[
        {"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"},
        {"id":new_id(),"kind":"StandaloneImage","imageId":null},
        {"id":new_id(),"kind":"SteamImportedArtwork","imageId":null},
        {"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}
    ]});
    editor
        .execute(
            "Set",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: a,
            },
            None,
        )
        .unwrap();
    let result = render(editor.document(), &id, Some((16, 16)), &mut Empty).unwrap();
    assert!(
        result
            .achieved
            .pixels()
            .all(|pixel| *pixel == Rgba([255, 0, 0, 255]))
    );
    let preview = crate::render_achieved_cancellable(
        editor.document(),
        &id,
        Some((16, 16)),
        &mut Empty,
        &|| false,
    )
    .unwrap();
    assert!(
        preview
            .pixels()
            .all(|pixel| *pixel == Rgba([255, 0, 0, 255]))
    );
}
