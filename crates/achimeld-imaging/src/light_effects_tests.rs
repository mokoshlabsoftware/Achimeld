use image::{Rgba, RgbaImage};
use serde_json::json;

fn source() -> RgbaImage {
    RgbaImage::from_fn(64, 64, |x, y| {
        if (16..48).contains(&x) && (16..48).contains(&y) {
            Rgba([24, 24, 24, 255])
        } else {
            Rgba([0; 4])
        }
    })
}

#[test]
fn inner_and_outer_light_have_distinct_visible_coverage_and_editable_parameters() {
    let source = source();
    let inner = json!({"kind":"InnerLight","colorHex":"#FFFFFFFF","blendMode":"Screen",
        "offsetX":0,"offsetY":8,"blur":12});
    let apply = |effect| {
        crate::effects::apply(
            source.clone(),
            &json!({"effects":[effect]}),
            64,
            64,
            &|| false,
            None,
        )
        .unwrap()
    };
    let inside = apply(inner.clone());
    assert!(inside.get_pixel(32, 16)[0] > 24);
    assert_eq!(inside.get_pixel(32, 8)[3], 0);
    assert!(
        inside
            .pixels()
            .zip(source.pixels())
            .all(|(a, b)| a[3] == b[3])
    );
    let mut moved = inner;
    moved["offsetY"] = json!(-32);
    assert_ne!(inside, apply(moved));

    let outer = json!({"kind":"OuterLight","colorHex":"#FFFF0000","blendMode":"Screen","width":16,"blur":32});
    let outside = apply(outer.clone());
    assert!(outside.get_pixel(32, 14)[3] > 0);
    assert_eq!(outside.get_pixel(32, 14)[0], 255);
    assert_eq!(outside.get_pixel(32, 32), source.get_pixel(32, 32));
    assert_eq!(outside.get_pixel(0, 0)[3], 0);
    for (key, value) in [
        ("opacity", json!(0)),
        ("visible", json!(false)),
        ("colorHex", json!("#00FF0000")),
    ] {
        let mut effect = outer.clone();
        effect[key] = value;
        assert_eq!(source, apply(effect));
    }
    let mut wide = outer.clone();
    wide["width"] = json!(64);
    assert!(apply(wide).get_pixel(32, 10)[3] > outside.get_pixel(32, 10)[3]);
    let mut sharp = outer;
    sharp["blur"] = json!(0);
    assert_ne!(apply(sharp), outside);
}

#[test]
fn outer_light_is_bounded_cancellable_and_scales_with_the_icon() {
    let effect =
        json!({"effects":[{"kind":"OuterLight","width":16,"blur":32,"colorHex":"#FFFFFFFF"}]});
    assert!(crate::effects::apply(source(), &effect, 64, 64, &|| true, None).is_err());
    for size in [32, 64, 128] {
        let image = RgbaImage::from_fn(size, size, |x, y| {
            if (size / 4..size * 3 / 4).contains(&x) && (size / 4..size * 3 / 4).contains(&y) {
                Rgba([24, 24, 24, 128])
            } else {
                Rgba([0; 4])
            }
        });
        let result =
            crate::effects::apply(image.clone(), &effect, size, size, &|| false, None).unwrap();
        assert_eq!(result.dimensions(), (size, size));
        assert!(result.get_pixel(size / 2, size / 4 - 1)[3] > 0);
        assert_eq!(
            result.get_pixel(size / 2, size / 2),
            image.get_pixel(size / 2, size / 2)
        );
    }
}
