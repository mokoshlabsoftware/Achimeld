use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

fn render(source: &RgbaImage, effects: Value) -> RgbaImage {
    crate::effects::apply(
        source.clone(),
        &json!({"effects":effects}),
        source.width(),
        source.height(),
        &|| false,
        None,
    )
    .unwrap()
}

fn coordinates(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 3) as u8, (y * 3) as u8, 128, 255])
    })
}

#[test]
fn spatial_effects_identity_visibility_and_zero_opacity_preserve_every_pixel() {
    let source = RgbaImage::from_fn(63, 35, |x, y| {
        Rgba([(x * 4) as u8, (y * 7) as u8, 200, ((x + y) * 2) as u8])
    });
    for mut effect in [
        json!({"kind":"Skew","skewX":45,"skewY":-30}),
        json!({"kind":"Perspective","perspectiveX":0.4,"perspectiveY":0.3}),
        json!({"kind":"Pinch","amount":0.8}),
        json!({"kind":"LensBlur","blur":32,"aberration":32}),
    ] {
        assert_eq!(render(&source, json!([{"kind":effect["kind"]}])), source);
        effect["visible"] = json!(false);
        assert_eq!(render(&source, json!([effect])), source);
        effect["visible"] = json!(true);
        effect["opacity"] = json!(0.0);
        assert_eq!(render(&source, json!([effect])), source);
    }
}

#[test]
fn spatial_skew_axes_and_perspective_use_centered_inverse_mapping() {
    let source = coordinates(65, 65);
    let horizontal = render(&source, json!([{"kind":"Skew","skewX":45}]));
    assert_eq!(horizontal.get_pixel(40, 40), source.get_pixel(32, 40));
    let vertical = render(&source, json!([{"kind":"Skew","skewY":45}]));
    assert_eq!(vertical.get_pixel(40, 40), source.get_pixel(40, 32));
    // Equal 45-degree shears must remain invertible, rather than collapsing a diagonal.
    let both = render(&source, json!([{"kind":"Skew","skewX":45,"skewY":45}]));
    assert_eq!(both.get_pixel(40, 48), source.get_pixel(32, 40));
    for (px, py) in [(0.4, 0.0), (0.0, -0.4), (0.45, 0.45), (-0.45, -0.45)] {
        let result = render(
            &source,
            json!([{"kind":"Perspective","perspectiveX":px,"perspectiveY":py}]),
        );
        assert_eq!(result.get_pixel(32, 32), source.get_pixel(32, 32));
        let denominator = 1.0 - px * 8.0 / 32.5 - py * 12.0 / 32.5;
        assert_eq!(
            *result.get_pixel(40, 44),
            crate::renderer::bilinear(&source, 32.0 + 8.0 / denominator, 32.0 + 12.0 / denominator)
        );
        assert!(result.pixels().any(|p| p[3] > 0));
    }
}

#[test]
fn spatial_pinch_bulges_or_pinches_and_keeps_the_outer_region_unchanged() {
    let source = coordinates(81, 41);
    let bulge = render(&source, json!([{"kind":"Pinch","amount":1}]));
    let pinch = render(&source, json!([{"kind":"Pinch","amount":-1}]));
    assert!(bulge.get_pixel(48, 20)[0] < source.get_pixel(48, 20)[0]);
    assert!(pinch.get_pixel(48, 20)[0] > source.get_pixel(48, 20)[0]);
    assert_eq!(bulge.get_pixel(40, 20), source.get_pixel(40, 20));
    for (x, y) in [(0, 0), (80, 40), (4, 20), (76, 20)] {
        assert_eq!(bulge.get_pixel(x, y), source.get_pixel(x, y));
        assert_eq!(pinch.get_pixel(x, y), source.get_pixel(x, y));
    }
}

#[test]
fn spatial_lens_blur_keeps_center_sharp_and_creates_transparent_edge_fringes() {
    let source = RgbaImage::from_fn(65, 65, |x, y| {
        if (x == 32 && y == 32) || (50..57).contains(&x) {
            Rgba([255; 4])
        } else {
            Rgba([0; 4])
        }
    });
    let lens = render(
        &source,
        json!([{"kind":"LensBlur","blur":32,"aberration":32}]),
    );
    assert_eq!(lens.get_pixel(32, 32), source.get_pixel(32, 32));
    assert!(
        lens.enumerate_pixels()
            .any(|(x, _, p)| x > 45 && p[3] > 0 && p[0].abs_diff(p[2]) > 100)
    );
    let blur_only = render(&source, json!([{"kind":"LensBlur","blur":32}]));
    assert!(blur_only.get_pixel(48, 32)[3] > 0);
    assert!(
        blur_only
            .pixels()
            .all(|p| p[3] == 0 || (p[0] == p[1] && p[1] == p[2]))
    );
    assert!(lens.pixels().all(|p| p[3] > 0 || p.0 == [0; 4]));
    let fringe_only = render(&source, json!([{"kind":"LensBlur","aberration":32}]));
    assert_ne!(lens, fringe_only);
}

#[test]
fn spatial_effect_order_moves_outline_masks_and_disables_stale_vector_strokes() {
    let source = RgbaImage::from_fn(65, 65, |x, y| {
        if (24..41).contains(&x) && (16..49).contains(&y) {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0; 4])
        }
    });
    let skew = json!({"kind":"Skew","skewX":45});
    let outline = json!({"kind":"Outline","width":24,"colorHex":"#FF0000FF","placement":"Outward"});
    let before = render(&source, json!([outline, skew]));
    let outlined = render(&source, json!([outline]));
    assert_eq!(before, render(&outlined, json!([skew])));
    let after = render(&source, json!([skew, outline]));
    assert_ne!(before, after);
    let lit = render(
        &source,
        json!([skew,{"kind":"InnerLight","offsetX":0,"offsetY":8,"blur":0,"blurEnabled":false}]),
    );
    let warped = render(&source, json!([skew]));
    assert_eq!(
        lit,
        render(
            &warped,
            json!([{"kind":"InnerLight","offsetX":0,"offsetY":8,"blur":0,"blurEnabled":false}])
        )
    );
    let stale_outline = |_: &Value| -> Result<RgbaImage, String> {
        panic!("Spatially transformed text must use its current raster coverage")
    };
    crate::effects::apply(
        source,
        &json!({"effects":[skew,outline]}),
        65,
        65,
        &|| false,
        Some(&stale_outline),
    )
    .unwrap();
}

#[test]
fn spatial_effects_cancel_during_sampling_and_handle_small_canvases() {
    for effect in [
        json!({"kind":"Skew","skewX":60,"skewY":60}),
        json!({"kind":"Perspective","perspectiveX":0.45,"perspectiveY":-0.45}),
        json!({"kind":"Pinch","amount":1}),
        json!({"kind":"LensBlur","blur":128,"aberration":64}),
    ] {
        for (w, h) in [(1, 1), (1, 5), (8, 1), (32, 16)] {
            let source = coordinates(w, h);
            assert_eq!(render(&source, json!([effect])).dimensions(), (w, h));
        }
        let rows = std::cell::Cell::new(0);
        let cancelled = || {
            rows.set(rows.get() + 1);
            rows.get() > 3
        };
        assert!(crate::spatial_effects::apply(coordinates(64, 64), &effect, &cancelled).is_err());
    }
}

#[test]
fn spatial_neutral_effect_preserves_vector_text_outline_quality() {
    for effect in [
        json!({"kind":"Skew"}),
        json!({"kind":"Perspective"}),
        json!({"kind":"Pinch"}),
        json!({"kind":"LensBlur"}),
        json!({"kind":"Skew","skewX":45,"opacity":0}),
    ] {
        let calls = std::cell::Cell::new(0);
        let source = coordinates(32, 32);
        let outline = |_: &Value| {
            calls.set(calls.get() + 1);
            Ok(RgbaImage::new(32, 32))
        };
        crate::effects::apply(
            source,
            &json!({"effects":[effect,{"kind":"Outline"}]}),
            32,
            32,
            &|| false,
            Some(&outline),
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
    }
}

#[test]
#[ignore = "Writes a visual comparison to the external build output directory"]
fn spatial_effects_visual_fixture() {
    let source = RgbaImage::from_fn(256, 256, |x, y| {
        if !(24..232).contains(&x) || !(24..232).contains(&y) {
            return Rgba([0; 4]);
        }
        if x % 26 < 2 || y % 26 < 2 {
            return Rgba([255; 4]);
        }
        Rgba([(x * 255 / 256) as u8, (y * 255 / 256) as u8, 180, 255])
    });
    let effects = [
        json!([]),
        json!([{"kind":"Skew","skewX":24,"skewY":-10}]),
        json!([{"kind":"Perspective","perspectiveX":0.3,"perspectiveY":-0.15}]),
        json!([{"kind":"Pinch","amount":0.9}]),
        json!([{"kind":"LensBlur","blur":12,"aberration":16}]),
    ];
    let mut sheet = RgbaImage::from_fn(256 * 5, 256, |x, y| {
        let c = if (x / 16 + y / 16) % 2 == 0 { 40 } else { 55 };
        Rgba([c, c, c, 255])
    });
    for (index, effect) in effects.into_iter().enumerate() {
        let partial =
            image::imageops::resize(&source, 64, 64, image::imageops::FilterType::Triangle);
        let start = std::time::Instant::now();
        render(&partial, effect.clone());
        let first = start.elapsed();
        let start = std::time::Instant::now();
        let result = render(&source, effect);
        eprintln!(
            "Spatial effect {index}: first 64px {first:?}, final 256px {:?}",
            start.elapsed()
        );
        image::imageops::overlay(&mut sheet, &result, (index * 256) as i64, 0);
    }
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .expect("Use build/Rust.ps1 for external outputs");
    sheet
        .save(std::path::PathBuf::from(root).join("spatial-effects-preview.png"))
        .unwrap();
}
