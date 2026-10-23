use super::*;
use crate::particles::{ParticlePreset, ParticleSettings, layout_for_instance};
use serde_json::json;

#[test]
fn gradient_map_reverse_preserves_edge_positions_and_mirrors_colors_alpha_and_influence() {
    let mut map = presets().remove(1).1;
    map.points.push(ColorPoint {
        x: 0.375,
        y: 0.25,
        color_hex: "#50123456".into(),
        power: 1.4,
        range_x: 0.4,
        range_y: 0.7,
    });
    let original = map.clone();
    let before = original.compile().unwrap();
    map.reverse_gradient().unwrap();
    map.validate().unwrap();
    let after = map.compile().unwrap();
    for (i, p) in map.points.iter().take(EDGE_POINTS).enumerate() {
        assert_eq!((p.x, p.y), anchor_position(i));
    }
    for x in [0.0, 0.2, 0.375, 0.75, 1.0] {
        for y in [0.0, 0.25, 0.6, 1.0] {
            let a = before.sample(x, y);
            let b = after.sample(x, 1.0 - y);
            assert!(
                a.iter()
                    .zip(b)
                    .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 1)
            );
        }
    }
    map.reverse_gradient().unwrap();
    assert_eq!(map, original);
}

#[test]
fn particle_color_map_anchors_bounds_presets_and_alpha_are_validated() {
    let map = GradientMap::default();
    assert_eq!(map.points.len(), EDGE_POINTS);
    for (i, p) in map.points.iter().enumerate() {
        assert_eq!((p.x, p.y), anchor_position(i));
        if p.y == 0.0 {
            assert_eq!(decode_color(&p.color_hex), Some([0, 0, 0, 255]));
        }
        if p.y == 1.0 {
            assert_eq!(decode_color(&p.color_hex), Some([255; 4]));
        }
    }
    for (_, map) in presets() {
        let field = map.compile().unwrap();
        for p in &map.points {
            let color = decode_color(&p.color_hex).unwrap();
            assert_eq!(
                field.sample(p.x, p.y),
                if color[3] == 0 { [0; 4] } else { color }
            );
        }
    }
    let mut moved = map.clone();
    moved.points[0].x = 0.1;
    assert!(moved.validate().is_ok());
    let mut removed = map.clone();
    removed.points.remove(0);
    assert!(removed.validate().is_ok());
    let mut duplicate = map.clone();
    duplicate.points.push(map.points[0].clone());
    assert!(duplicate.validate().is_err());
    let mut huge = map.clone();
    huge.points.resize(MAX_POINTS + 1, ColorPoint::default());
    assert!(huge.validate().is_err());
    for (power, range_x, range_y, x, color) in [
        (f64::NAN, 0.35, 0.35, 0.5, "#FFFFFFFF"),
        (1.0, 0.0, 0.35, 0.5, "#FFFFFFFF"),
        (1.0, 0.35, 3.0, 0.5, "#FFFFFFFF"),
        (1.0, 0.35, 0.35, -0.5, "#FFFFFFFF"),
        (1.0, 0.35, 0.35, 0.5, "#garbage"),
    ] {
        let mut invalid = map.clone();
        invalid.points.push(ColorPoint {
            x,
            y: 0.5,
            color_hex: color.into(),
            power,
            range_x,
            range_y,
        });
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn particle_color_map_free_points_empty_fields_and_reversal_round_trip() {
    let mut map = GradientMap {
        points: vec![
            ColorPoint {
                x: 0.25,
                y: 0.375,
                color_hex: "#80FF4080".into(),
                ..Default::default()
            },
            ColorPoint {
                x: 1.0,
                y: 0.0,
                color_hex: "#FF40FFFF".into(),
                ..Default::default()
            },
        ],
    };
    let original = map.clone();
    let before = map.compile().unwrap();
    map.reverse_gradient().unwrap();
    let reversed = map.compile().unwrap();
    for x in [0.0, 0.25, 1.0] {
        for y in [0.0, 0.375, 1.0] {
            assert_eq!(before.sample(x, y), reversed.sample(x, 1.0 - y));
        }
    }
    map.reverse_gradient().unwrap();
    assert_eq!(map, original);
    for points in [original.points, Vec::new()] {
        let settings = ParticleSettings {
            color_map: Some(GradientMap { points }),
            ..Default::default()
        };
        let value = json!(settings);
        assert_eq!(
            ParticlePreset::decode(&ParticlePreset::encode(&value).unwrap()).unwrap(),
            value
        );
    }
    map.points.clear();
    let empty = map.compile().unwrap();
    assert_eq!(empty.sample(0.5, 0.5), [0; 4]);
    assert!(
        empty
            .raster(8, 8, &|| false)
            .unwrap()
            .iter()
            .all(|p| *p == [0; 4])
    );
    map.reverse_gradient().unwrap();
}

#[test]
fn particle_color_map_influence_slices_and_premultiplied_blending() {
    let mut map = GradientMap::default();
    for p in &mut map.points {
        p.color_hex = "#FF000000".into();
    }
    map.points.push(ColorPoint {
        x: 0.5,
        y: 0.5,
        color_hex: "#FFFF0000".into(),
        range_x: 0.7,
        range_y: 0.07,
        ..Default::default()
    });
    let field = map.compile().unwrap();
    assert_eq!(field.sample(0.5, 0.5), [255, 0, 0, 255]);
    assert!(field.sample(0.65, 0.5)[0] > field.sample(0.5, 0.65)[0] + 30);
    let before = field.sample(0.7, 0.55)[0];
    map.points[EDGE_POINTS].power = 8.0;
    assert!(map.compile().unwrap().sample(0.7, 0.55)[0] > before);
    let slice = field.slice(0.37, 257);
    for (y, color) in slice.into_iter().enumerate() {
        assert_eq!(color, field.sample(0.37, y as f64 / 256.0));
    }
    let raster = field.raster(5, 9, &|| false).unwrap();
    assert_eq!(raster[4], field.sample(1.0, 0.0));
    assert_eq!(raster[40], field.sample(0.0, 1.0));
    assert!(field.raster(64, 32, &|| true).is_none());

    for p in &mut map.points {
        p.color_hex = "#FFFF0000".into();
    }
    map.points[EDGE_POINTS].color_hex = "#000000FF".into();
    map.points[EDGE_POINTS].power = 1.0;
    map.points[EDGE_POINTS].range_x = 0.35;
    map.points[EDGE_POINTS].range_y = 0.35;
    let color = map.compile().unwrap().sample(0.65, 0.5);
    assert!(color[3] > 0 && color[3] < 255);
    assert_eq!(
        &color[..3],
        &[255, 0, 0],
        "Transparent blue must not tint the visible red"
    );
}

#[test]
fn particle_color_map_slices_are_unique_stable_and_independent_of_legacy_geometry() {
    let mut settings = ParticleSettings {
        count: 128,
        ..Default::default()
    };
    let legacy = layout_for_instance(&settings, 1.0, "first");
    settings.color_map = Some(presets().remove(1).1);
    let mapped = layout_for_instance(&settings, 1.0, "first");
    let mut slices = Vec::new();
    let mut strata = Vec::new();
    for (a, b) in legacy.iter().zip(&mapped) {
        assert_eq!(
            (a.x, a.y, a.diameter, a.rotation, a.opacity, a.color),
            (b.x, b.y, b.diameter, b.rotation, b.opacity, b.color)
        );
        assert_eq!(a.slice, b.slice);
        assert!((0.0..1.0).contains(&b.slice));
        assert!(!slices.contains(&b.slice));
        slices.push(b.slice);
        strata.push((b.slice * f64::from(settings.count)).floor() as u32);
    }
    strata.sort();
    assert_eq!(strata, (0..settings.count).collect::<Vec<_>>());
    assert_eq!(
        mapped[0].slice,
        layout_for_instance(&settings, 1.0, "second")[0].slice
    );
    settings.different_seed_per_instance = true;
    assert_ne!(
        layout_for_instance(&settings, 1.0, "first")[0].slice,
        layout_for_instance(&settings, 1.0, "second")[0].slice
    );
    let value = json!(settings);
    let loaded = ParticlePreset::decode(&ParticlePreset::encode(&value).unwrap()).unwrap();
    assert_eq!(loaded, value);
    let loaded = ParticleSettings::from_value(&loaded).unwrap();
    assert_eq!(
        layout_for_instance(&settings, 1.0, "first")[0].slice,
        layout_for_instance(&loaded, 1.0, "first")[0].slice
    );
    let sparse = ParticleSettings::from_value(&json!({})).unwrap();
    assert!(
        sparse.color_map.is_none(),
        "Legacy 1D recipes keep their appearance until explicitly converted"
    );
}
