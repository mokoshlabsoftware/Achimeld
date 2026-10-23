use crate::perlin::render;
use serde_json::json;

#[test]
fn seeded_perlin_is_repeatable_and_octaves_keep_their_identity() {
    let mut settings = json!({"seed":71,"frequency":3.4,"octaves":6});
    let first = render(&settings, 64, 32, &|| false).unwrap();
    assert_eq!(first, render(&settings, 64, 32, &|| false).unwrap());
    assert!(first.pixels().any(|p| p != first.get_pixel(0, 0)));
    assert!(
        first
            .pixels()
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
    );
    settings["seed"] = json!(72);
    assert_ne!(first, render(&settings, 64, 32, &|| false).unwrap());
    settings["seed"] = json!(71);
    settings["enabledOctaves"] = json!([true, false, true, false, true, false]);
    let disabled = render(&settings, 64, 32, &|| false).unwrap();
    assert_ne!(first, disabled);
    settings["octaves"] = json!(5);
    assert_eq!(disabled, render(&settings, 64, 32, &|| false).unwrap());
    settings["octaves"] = json!(6);
    assert_eq!(disabled, render(&settings, 64, 32, &|| false).unwrap());
    settings["enabledOctaves"] = json!([false, false, false, false, false, false]);
    assert!(
        render(&settings, 64, 32, &|| false)
            .unwrap()
            .pixels()
            .all(|p| p.0 == [128, 128, 128, 255])
    );
}

#[test]
fn perlin_gradient_maps_values_and_stop_transparency() {
    let plain = render(&json!({}), 48, 48, &|| false).unwrap();
    let mapped = render(
        &json!({"useGradient":true,"gradient":{"stops":[
        {"offset":0.0,"colorHex":"#00FF0000"},{"offset":1.0,"colorHex":"#FF0000FF"}]}}),
        48,
        48,
        &|| false,
    )
    .unwrap();
    for (grey, color) in plain.pixels().zip(mapped.pixels()) {
        assert_eq!(color[0], 255 - grey[0]);
        assert_eq!(color[1], 0);
        assert_eq!(color[2], grey[0]);
        assert_eq!(color[3], grey[0]);
    }
}

#[test]
fn perlin_coordinates_match_across_preview_resolutions_and_cancel_between_rows() {
    let settings =
        json!({"seed":2147483647,"frequency":0.1,"octaves":12,"lacunarity":4,"persistence":1});
    let small = render(&settings, 8, 16, &|| false).unwrap();
    let large = render(&settings, 24, 48, &|| false).unwrap();
    for y in 0..16 {
        for x in 0..8 {
            assert_eq!(small.get_pixel(x, y), large.get_pixel(x * 3 + 1, y * 3 + 1));
        }
    }
    let rows = std::cell::Cell::new(0);
    assert!(
        render(&settings, 64, 64, &|| {
            rows.set(rows.get() + 1);
            rows.get() > 3
        })
        .unwrap_err()
        .contains("cancelled")
    );
    assert_eq!(rows.get(), 4);
}
