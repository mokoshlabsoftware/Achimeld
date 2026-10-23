use super::artifact3d::render;
use achimeld_application::{
    artifact3d::*,
    text3d::{TextProjection, TextShading},
};
use image::{Rgba, RgbaImage};
use serde_json::json;

fn settings(part: ArtifactPart) -> ArtifactSettings {
    ArtifactSettings {
        parts: vec![part],
        projection: TextProjection::Orthographic,
        rotation: [0.0; 3],
        ..Default::default()
    }
}
fn image(settings: &ArtifactSettings) -> RgbaImage {
    render(&json!(settings), (96, 96), &|| false).unwrap()
}
fn coverage(image: &RgbaImage) -> u64 {
    image.pixels().map(|p| u64::from(p[3])).sum()
}

#[test]
fn artifact3d_primitives_rounding_ranges_and_sphere_cuts_render_geometry() {
    for primitive in [
        Primitive::Cube,
        Primitive::Cone,
        Primitive::Torus,
        Primitive::Cylinder,
        Primitive::Sphere,
    ] {
        let part = ArtifactPart {
            primitive,
            size: [280.0; 3],
            ..Default::default()
        };
        let base = settings(part.clone());
        let full = image(&base);
        assert!(coverage(&full) > 100_000, "Missing {primitive:?}");
        let rotated = image(&ArtifactSettings {
            rotation: [20.0, 30.0, 15.0],
            ..base.clone()
        });
        if primitive != Primitive::Sphere {
            assert_ne!(full, rotated);
        }
        if primitive != Primitive::Cube {
            let half = settings(ArtifactPart {
                start_angle: 90.0,
                sweep_angle: 180.0,
                ..part.clone()
            });
            assert!(
                coverage(&image(&half)) < coverage(&full),
                "Angular range did not cut {primitive:?}"
            );
        }
        if matches!(primitive, Primitive::Cube | Primitive::Cylinder) {
            let chamfer = image(&settings(ArtifactPart {
                rounding: 0.15,
                ..part.clone()
            }));
            let round = image(&settings(ArtifactPart {
                rounding: 1.0,
                ..part.clone()
            }));
            assert_ne!(full, chamfer);
            assert_ne!(chamfer, round);
            let sphere = image(&settings(ArtifactPart {
                primitive: Primitive::Sphere,
                ..part
            }));
            assert!((coverage(&round) as f64 / coverage(&sphere) as f64 - 1.0).abs() < 0.04);
        }
    }
    let sphere = ArtifactPart {
        primitive: Primitive::Sphere,
        size: [280.0; 3],
        ..Default::default()
    };
    let full = coverage(&image(&settings(sphere.clone())));
    for part in [
        ArtifactPart {
            cut_top: 0.0,
            ..sphere.clone()
        },
        ArtifactPart {
            cut_bottom: 0.0,
            ..sphere.clone()
        },
    ] {
        let cut = coverage(&image(&settings(part)));
        assert!((cut as f64 / full as f64 - 0.5).abs() < 0.03);
    }
    let bowl = ArtifactPart {
        cut_top: 0.0,
        wall_thickness: 0.15,
        ..sphere.clone()
    };
    let a = ArtifactSettings {
        rotation: [65.0, 0.0, 0.0],
        ..settings(bowl.clone())
    };
    let b = ArtifactSettings {
        parts: vec![ArtifactPart {
            wall_thickness: 0.0,
            ..bowl
        }],
        ..a.clone()
    };
    assert_ne!(image(&a), image(&b), "Hollow bowl lost its interior");
    // Cuts outside the inner sphere must retain a flat external cap, rather
    // than connecting the inner pole to the cut with a spurious conical wall.
    for (lo, hi, angle) in [(-0.9, 0.9, 90.0), (-0.9, 0.9, -90.0)] {
        let solid = ArtifactSettings {
            rotation: [angle, 0.0, 0.0],
            ..settings(ArtifactPart {
                cut_bottom: lo,
                cut_top: hi,
                ..sphere.clone()
            })
        };
        let mut shell = solid.clone();
        shell.parts[0].wall_thickness = 0.25;
        assert_eq!(image(&solid), image(&shell));
    }
}

#[test]
fn artifact3d_depth_material_alpha_camera_and_cancellation() {
    let front = ArtifactPart {
        position: [0.0, 0.0, 40.0],
        size: [220.0; 3],
        color_hex: "#80FF0000".into(),
        shading: TextShading::Unlit,
        ..Default::default()
    };
    let back = ArtifactPart {
        id: achimeld_application::new_id(),
        position: [0.0, 0.0, -40.0],
        size: [240.0; 3],
        color_hex: "#FF0000FF".into(),
        shading: TextShading::Unlit,
        ..Default::default()
    };
    let scene = ArtifactSettings {
        parts: vec![front.clone(), back.clone()],
        ..settings(front.clone())
    };
    let color = image(&scene);
    assert_eq!(*color.get_pixel(48, 48), Rgba([255, 0, 0, 128]));
    assert_eq!(
        color,
        image(&ArtifactSettings {
            parts: vec![back, front],
            ..scene.clone()
        })
    );
    let mask = image(&ArtifactSettings {
        output: ArtifactOutput::AlphaMask,
        ..scene.clone()
    });
    for (rgb, alpha) in color.pixels().zip(mask.pixels()) {
        assert_eq!(rgb[3], alpha[3]);
        if alpha[3] > 0 {
            assert_eq!(&alpha.0[..3], &[255, 255, 255]);
        }
    }
    for shading in [TextShading::Matte, TextShading::Glossy] {
        let mut lit = scene.clone();
        lit.parts[0].shading = shading;
        assert_ne!(color, image(&lit));
    }
    let perspective = ArtifactSettings {
        projection: TextProjection::Perspective,
        ..scene.clone()
    };
    assert_ne!(
        image(&perspective),
        image(&ArtifactSettings {
            focal_length: 80.0,
            ..perspective.clone()
        })
    );
    assert_ne!(
        image(&perspective),
        image(&ArtifactSettings {
            camera_distance: 1100.0,
            ..perspective
        })
    );
    for z in [780.0, 800.0, 2000.0] {
        image(&ArtifactSettings {
            position: [0.0, 0.0, z],
            projection: TextProjection::Perspective,
            ..scene.clone()
        });
    }
    let calls = std::cell::Cell::new(0);
    let result = render(&json!(scene), (96, 96), &|| {
        calls.set(calls.get() + 1);
        calls.get() > 15
    });
    assert!(result.unwrap_err().contains("cancelled"));
    assert!(render(&json!({}), (4097, 1), &|| false).is_err());
}

#[test]
fn artifact3d_presets_render_and_edits_change_the_output() {
    for (kind, name) in PRESETS {
        let base = ArtifactSettings {
            parts: preset(kind),
            ..Default::default()
        };
        let full = image(&base);
        assert!(coverage(&full) > 60_000, "Empty {name}");
        let mut changed = base.clone();
        changed.parts[0].size[0] *= 0.7;
        assert_ne!(full, image(&changed), "Cannot edit {name}");
    }
}

#[test]
fn artifact3d_composes_with_layer_effects_opacity_and_undo() {
    use achimeld_application::{Command, Editor, EntityKind, new_id};
    struct Assets;
    impl crate::AssetSource for Assets {
        fn image(&mut self, _: &serde_json::Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            Err("No artwork".into())
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
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Artifact3d","opacity":0.5,"artifact3d":ArtifactSettings::default(),"effects":[{"id":new_id(),"kind":"ColorOverlay","colorHex":"#FFFF0000","opacity":1.0}]}]});
    editor
        .execute(
            "Artifact",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let result = crate::render(editor.document(), &id, Some((128, 128)), &mut Assets).unwrap();
    assert!(coverage(&result.achieved) > 10_000);
    assert!(result.achieved.pixels().all(|p| p[3] <= 128));
    assert!(
        result
            .achieved
            .pixels()
            .filter(|p| p[3] > 100)
            .all(|p| p[0] > 200 && p[1] == 0 && p[2] == 0)
    );
    assert!(editor.undo());
    assert!(editor.redo());
}

#[test]
#[ignore = "Writes a visual contact sheet to the external build output directory"]
fn artifact3d_contact_sheet() {
    let root = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_ARTIFACT_ROOT").expect("External output root"),
    );
    let mut sheet = RgbaImage::from_pixel(5 * 240, 3 * 240, Rgba([27, 29, 34, 255]));
    let mut cases = Vec::new();
    for (kind, name) in PRESETS {
        cases.push((
            name.to_owned(),
            ArtifactSettings {
                parts: preset(kind),
                ..Default::default()
            },
        ));
    }
    for primitive in [
        Primitive::Cube,
        Primitive::Cone,
        Primitive::Torus,
        Primitive::Cylinder,
        Primitive::Sphere,
    ] {
        let part = ArtifactPart {
            primitive,
            size: if primitive == Primitive::Torus {
                [250.0, 80.0, 250.0]
            } else {
                [250.0; 3]
            },
            rotation: if primitive == Primitive::Torus {
                [90.0, 0.0, 0.0]
            } else {
                [0.0; 3]
            },
            rounding: 0.2,
            ..Default::default()
        };
        cases.push((
            format!("{primitive:?}"),
            ArtifactSettings {
                rotation: [20.0, -25.0, -10.0],
                ..settings(part)
            },
        ));
    }
    for (kind, name) in PRESETS {
        cases.push((
            format!("{name} mask"),
            ArtifactSettings {
                parts: preset(kind),
                output: ArtifactOutput::AlphaMask,
                ..Default::default()
            },
        ));
    }
    for (i, (name, settings)) in cases.iter().enumerate() {
        let start = std::time::Instant::now();
        let quick = render(&json!(settings), (64, 64), &|| false).unwrap();
        let first = start.elapsed();
        let detail = render(&json!(settings), (224, 224), &|| false).unwrap();
        if i < 5 {
            detail
                .save(root.join(format!("artifact3d-{}.png", name.to_lowercase())))
                .unwrap();
        }
        println!(
            "{name}: first visible {first:?}, detail {:?}; quick coverage {}",
            start.elapsed(),
            coverage(&quick)
        );
        image::imageops::overlay(
            &mut sheet,
            &detail,
            ((i % 5) * 240 + 8) as i64,
            ((i / 5) * 240 + 8) as i64,
        );
    }
    let path = root.join("artifact3d-preview.png");
    sheet.save(&path).unwrap();
    println!("{}", path.display());
}
