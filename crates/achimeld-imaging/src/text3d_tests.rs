use super::text3d::render;
use achimeld_application::text3d::{Text3dOutput, Text3dSettings, TextProjection, TextShading};
use image::{Rgba, RgbaImage};

fn font() -> crate::Font {
    crate::Font::from_bytes(epaint_default_fonts::UBUNTU_LIGHT.to_vec()).unwrap()
}
fn image(settings: &Text3dSettings) -> RgbaImage {
    render(
        &font(),
        "BO8",
        192.0,
        Rgba([245, 170, 50, 255]),
        settings,
        (128, 128),
        &|| false,
    )
    .unwrap()
}
fn alpha(image: &RgbaImage) -> u64 {
    image.pixels().map(|p| u64::from(p[3])).sum()
}
fn flat() -> Text3dSettings {
    Text3dSettings {
        rotation_x: 0.0,
        rotation_y: 0.0,
        extrusion: 0.0,
        projection: TextProjection::Orthographic,
        shading: TextShading::Unlit,
        ..Default::default()
    }
}

#[test]
fn text3d_push_changes_weight_and_keeps_counters_or_erodes_letters_completely() {
    let base = flat();
    let normal = image(&base);
    let thin = image(&Text3dSettings {
        push: -2.0,
        ..base.clone()
    });
    let bold = image(&Text3dSettings {
        push: 6.0,
        ..base.clone()
    });
    assert!(alpha(&thin) < alpha(&normal));
    assert!(alpha(&bold) > alpha(&normal));
    assert_eq!(
        alpha(&image(&Text3dSettings {
            push: -64.0,
            ..base.clone()
        })),
        0
    );
    for push in [-2.0, 0.0, 6.0] {
        let ring = render(
            &font(),
            "O",
            320.0,
            Rgba([255; 4]),
            &Text3dSettings {
                push,
                emboss: 4.0,
                ..base.clone()
            },
            (128, 128),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            ring.get_pixel(64, 64)[3],
            0,
            "Counter was filled by a contour offset"
        );
    }
}

#[test]
fn text3d_output_masks_isolate_visible_faces_and_bevels_without_revealing_hidden_surfaces() {
    for settings in [
        Text3dSettings::default(),
        Text3dSettings {
            push: 3.0,
            ..Default::default()
        },
        Text3dSettings {
            push: 3.0,
            emboss: 5.0,
            ..Default::default()
        },
    ] {
        let full = image(&Text3dSettings {
            output: Text3dOutput::FullMask,
            ..settings.clone()
        });
        let face = image(&Text3dSettings {
            output: Text3dOutput::FaceMask,
            ..settings.clone()
        });
        let bevel = image(&Text3dSettings {
            output: Text3dOutput::EmbossMask,
            ..settings.clone()
        });
        let minus_face = image(&Text3dSettings {
            output: Text3dOutput::EmbossMaskMinusFace,
            ..settings.clone()
        });
        let mut walls = 0;
        for (((result, all), face), bevel) in minus_face
            .pixels()
            .zip(full.pixels())
            .zip(face.pixels())
            .zip(bevel.pixels())
        {
            assert!(
                result[3].abs_diff(all[3].saturating_sub(face[3])) <= 1,
                "The flat face must be cut out of the complete embossed object"
            );
            assert!(u16::from(result[3]) + 1 >= u16::from(bevel[3]));
            if all[3] == 255 && face[3] == 0 && bevel[3] == 0 {
                assert_eq!(
                    result[3], 255,
                    "Extrusion walls must remain after removing the face"
                );
                walls += 1;
            }
            if result[3] != 0 {
                assert_eq!(&result.0[..3], &[255; 3]);
            }
        }
        assert!(
            walls > 10,
            "The new output must include solid 3D side geometry"
        );
        assert!(alpha(&minus_face) > alpha(&bevel) + 1000);
        assert!(alpha(&full) > alpha(&face)); // Visible extrusion walls are excluded.
        assert!(alpha(&face) > 10_000);
        assert_eq!(alpha(&bevel) > 0, settings.emboss > 0.0);
        for ((all, face), bevel) in full.pixels().zip(face.pixels()).zip(bevel.pixels()) {
            assert!(
                u16::from(face[3]) + u16::from(bevel[3]) <= u16::from(all[3]) + 1,
                "Surface masks overlap or include hidden geometry"
            );
            for mask in [all, face, bevel] {
                if mask[3] != 0 {
                    assert_eq!(&mask.0[..3], &[255; 3]);
                }
            }
        }
        // Mask geometry is independent of material color and material opacity.
        let transparent = render(
            &font(),
            "BO8",
            192.0,
            Rgba([0; 4]),
            &Text3dSettings {
                output: Text3dOutput::FullMask,
                side_color_hex: "#00000000".into(),
                ..settings
            },
            (128, 128),
            &|| false,
        )
        .unwrap();
        assert_eq!(full, transparent);
        let lit_mask = image(&Text3dSettings {
            output: Text3dOutput::FullMask,
            shadow_strength: 1.0,
            smooth_normals: true,
            shading: TextShading::Glossy,
            light_azimuth: 160.0,
            ..settings.clone()
        });
        assert_eq!(
            full, lit_mask,
            "Lighting options must not change a geometry mask"
        );
    }
}

#[test]
fn text3d_reflectivity_controls_glossy_highlights_without_changing_masks_or_silhouettes() {
    let base = Text3dSettings {
        shading: TextShading::Glossy,
        emboss: 5.0,
        roughness: 0.8,
        smooth_normals: true,
        reflectivity: 0.0,
        ..Default::default()
    };
    let low = image(&base);
    let high = image(&Text3dSettings {
        reflectivity: 1.0,
        ..base.clone()
    });
    assert_eq!(alpha(&low), alpha(&high));
    assert_ne!(low, high);
    for (a, b) in low.pixels().zip(high.pixels()) {
        for i in 0..3 {
            assert!(a[i] <= b[i]);
        }
    }
    for shading in [TextShading::Matte, TextShading::Unlit] {
        assert_eq!(
            image(&Text3dSettings {
                shading,
                ..base.clone()
            }),
            image(&Text3dSettings {
                shading,
                reflectivity: 1.0,
                ..base.clone()
            })
        );
    }
    for output in [
        Text3dOutput::FullMask,
        Text3dOutput::FaceMask,
        Text3dOutput::EmbossMask,
        Text3dOutput::EmbossMaskMinusFace,
    ] {
        assert_eq!(
            image(&Text3dSettings {
                output,
                ..base.clone()
            }),
            image(&Text3dSettings {
                output,
                reflectivity: 1.0,
                ..base.clone()
            })
        );
    }
}

#[test]
fn text3d_masks_remain_antialiased_on_large_exports() {
    let settings = Text3dSettings {
        emboss: 6.0,
        rotation_y: 145.0,
        ..Default::default()
    };
    let export = |output| {
        render(
            &font(),
            "BO8",
            192.0,
            Rgba([255; 4]),
            &Text3dSettings {
                output,
                ..settings.clone()
            },
            (520, 128),
            &|| false,
        )
        .unwrap()
    };
    for output in [
        Text3dOutput::FullMask,
        Text3dOutput::FaceMask,
        Text3dOutput::EmbossMask,
        Text3dOutput::EmbossMaskMinusFace,
    ] {
        let result = export(output);
        let levels: std::collections::BTreeSet<_> = result
            .pixels()
            .map(|p| p[3])
            .filter(|a| *a > 0 && *a < 255)
            .collect();
        assert!(
            levels.len() >= 12,
            "{output:?} has only {} fractional coverage levels",
            levels.len()
        );
        assert!(result.pixels().any(|p| p[3] == 0));
        assert!(result.pixels().any(|p| p[3] == 255));
    }
}

#[test]
fn text3d_mask_refinement_cancels_obsolete_work_and_keeps_letter_holes() {
    for output in [
        Text3dOutput::FullMask,
        Text3dOutput::FaceMask,
        Text3dOutput::EmbossMask,
        Text3dOutput::EmbossMaskMinusFace,
    ] {
        let settings = Text3dSettings {
            output,
            emboss: 8.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            projection: TextProjection::Orthographic,
            ..Default::default()
        };
        let ring = render(
            &font(),
            "O",
            320.0,
            Rgba([255; 4]),
            &settings,
            (64, 64),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            ring.get_pixel(32, 32)[3],
            0,
            "Mask antialiasing filled a letter counter"
        );
        let levels: std::collections::BTreeSet<_> = ring
            .pixels()
            .map(|p| p[3])
            .filter(|a| *a > 0 && *a < 255)
            .collect();
        assert!(
            levels.len() >= 8,
            "Small masks also need smooth fractional coverage"
        );
        let calls = std::cell::Cell::new(0);
        let cancelled = render(
            &font(),
            "3D",
            192.0,
            Rgba([255; 4]),
            &settings,
            (64, 64),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() > 80
            },
        );
        assert_eq!(cancelled.unwrap_err(), "Icon rendering cancelled");
    }
}

#[test]
fn text3d_shadows_darken_surfaces_and_smooth_normals_preserve_the_silhouette() {
    let settings = Text3dSettings {
        emboss: 6.0,
        extrusion: 70.0,
        rotation_y: -45.0,
        light_azimuth: 70.0,
        light_elevation: 10.0,
        smooth_normals: true,
        ..Default::default()
    };
    let lit = image(&settings);
    let shadowed = image(&Text3dSettings {
        shadow_strength: 1.0,
        ..settings.clone()
    });
    assert_eq!(alpha(&lit), alpha(&shadowed));
    let mut darker = 0;
    for (a, b) in lit.pixels().zip(shadowed.pixels()) {
        assert_eq!(a[3], b[3]);
        for i in 0..3 {
            assert!(b[i] <= a[i]);
        }
        darker += usize::from(a[0] > b[0]);
    }
    assert!(darker > 10, "Letter geometry did not cast any shadows");
    let faceted = image(&Text3dSettings {
        smooth_normals: false,
        ..settings.clone()
    });
    assert_eq!(alpha(&lit), alpha(&faceted));
    assert_ne!(lit, faceted);
    assert_eq!(
        image(&Text3dSettings {
            shading: TextShading::Unlit,
            ..settings.clone()
        }),
        image(&Text3dSettings {
            shading: TextShading::Unlit,
            shadow_strength: 1.0,
            ..settings
        })
    );
}

#[test]
fn text3d_camera_lens_depth_and_world_position_have_distinct_behaviors() {
    let near = Text3dSettings {
        rotation_x: 0.0,
        rotation_y: 0.0,
        ..Default::default()
    };
    let far = Text3dSettings {
        camera_distance: 1600.0,
        ..near.clone()
    };
    assert!(alpha(&image(&near)) > alpha(&image(&far)) * 3);
    let long_lens = Text3dSettings {
        focal_length: 100.0,
        ..far.clone()
    };
    assert!(alpha(&image(&long_lens)) > alpha(&image(&far)) * 3);
    let ortho = Text3dSettings {
        projection: TextProjection::Orthographic,
        ..near
    };
    assert_eq!(
        image(&ortho),
        image(&Text3dSettings {
            camera_distance: 2400.0,
            ..ortho.clone()
        })
    );
    assert_ne!(
        image(&ortho),
        image(&Text3dSettings {
            orthographic_size: 800.0,
            ..ortho
        })
    );
    let flat = flat();
    let normal = image(&flat);
    assert_eq!(
        normal,
        image(&Text3dSettings {
            position_z: 200.0,
            ..flat.clone()
        })
    );
    for moved in [
        Text3dSettings {
            position_x: 80.0,
            ..flat.clone()
        },
        Text3dSettings {
            position_y: 80.0,
            ..flat.clone()
        },
    ] {
        assert_ne!(normal, image(&moved));
    }
    let perspective = Text3dSettings {
        projection: TextProjection::Perspective,
        ..flat
    };
    assert!(
        alpha(&image(&Text3dSettings {
            position_z: 200.0,
            ..perspective.clone()
        })) > alpha(&image(&perspective))
    );
}

#[test]
fn text3d_keeps_letter_holes_and_extrudes_all_three_rotation_axes() {
    let font = font();
    let settings = flat();
    let ring = render(
        &font,
        "O",
        320.0,
        Rgba([255; 4]),
        &settings,
        (128, 128),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        ring.get_pixel(64, 64)[3],
        0,
        "A letter counter stays transparent"
    );
    assert!(alpha(&ring) > 20_000);
    assert!(
        ring.pixels().any(|p| p[3] > 0 && p[3] < 255),
        "Glyph edges are antialiased"
    );
    let base = image(&settings);
    for rotated in [
        Text3dSettings {
            rotation_x: 50.0,
            extrusion: 80.0,
            ..settings.clone()
        },
        Text3dSettings {
            rotation_y: 50.0,
            extrusion: 80.0,
            ..settings.clone()
        },
        Text3dSettings {
            rotation_z: 35.0,
            extrusion: 80.0,
            ..settings.clone()
        },
        Text3dSettings {
            rotation_y: 180.0,
            extrusion: 80.0,
            ..settings.clone()
        },
    ] {
        let output = image(&rotated);
        assert!(alpha(&output) > 10_000, "Rotated text still renders");
        assert_ne!(base, output);
    }
    let rotated = Text3dSettings {
        rotation_y: 50.0,
        ..settings
    };
    assert_ne!(
        image(&rotated),
        image(&Text3dSettings {
            extrusion: 100.0,
            ..rotated
        })
    );
}

#[test]
fn text3d_material_lighting_and_transparency_render_on_faces_and_sides() {
    let base = Text3dSettings::default();
    let matte = image(&base);
    let glossy = image(&Text3dSettings {
        shading: TextShading::Glossy,
        roughness: 0.9,
        ..base.clone()
    });
    let unlit = image(&Text3dSettings {
        shading: TextShading::Unlit,
        ..base.clone()
    });
    assert_ne!(matte, glossy);
    assert_ne!(matte, unlit);
    assert_ne!(
        matte,
        image(&Text3dSettings {
            light_azimuth: 120.0,
            ..base.clone()
        })
    );
    assert_eq!(matte.get_pixel(0, 0)[3], 0);
    assert!(
        unlit
            .pixels()
            .any(|p| p[0] == 184 && p[1] == 196 && p[2] == 216),
        "Extrusion walls use the side material"
    );
    let transparent = render(
        &font(),
        "O",
        192.0,
        Rgba([255, 255, 255, 100]),
        &Text3dSettings {
            side_color_hex: "#64FFFFFF".into(),
            ..base
        },
        (128, 128),
        &|| false,
    )
    .unwrap();
    assert!(transparent.pixels().all(|p| p[3] <= 100));
}

#[test]
fn text3d_clips_camera_crossings_and_cancels_bounded_work() {
    for settings in [
        Text3dSettings {
            position_z: 2000.0,
            ..Default::default()
        },
        Text3dSettings {
            camera_distance: 32.0,
            extrusion: 80.0,
            ..Default::default()
        },
        Text3dSettings {
            position_x: 4096.0,
            ..Default::default()
        },
    ] {
        assert_eq!(image(&settings).dimensions(), (128, 128));
    }
    assert_eq!(
        alpha(&image(&Text3dSettings {
            position_z: 2000.0,
            ..Default::default()
        })),
        0
    );
    let calls = std::cell::Cell::new(0);
    let result = render(
        &font(),
        "3D text",
        192.0,
        Rgba([255; 4]),
        &Default::default(),
        (128, 128),
        &|| {
            calls.set(calls.get() + 1);
            calls.get() > 20
        },
    );
    assert_eq!(result.unwrap_err(), "Icon rendering cancelled");
    assert!(
        render(
            &font(),
            &"A".repeat(257),
            128.0,
            Rgba([255; 4]),
            &Default::default(),
            (64, 64),
            &|| false
        )
        .is_err()
    );
    assert!(
        render(
            &font(),
            "A",
            513.0,
            Rgba([255; 4]),
            &Default::default(),
            (64, 64),
            &|| false
        )
        .is_err()
    );
    assert_eq!(
        alpha(
            &render(
                &font(),
                " \n ",
                128.0,
                Rgba([255; 4]),
                &Default::default(),
                (64, 64),
                &|| false
            )
            .unwrap()
        ),
        0
    );
}

#[test]
fn text3d_layer_composes_through_the_normal_recipe_and_effect_pipeline() {
    use achimeld_application::{Command, Editor, EntityKind, new_id};
    use serde_json::json;
    struct Fonts;
    impl crate::AssetSource for Fonts {
        fn image(&mut self, _: &serde_json::Value) -> Result<std::sync::Arc<RgbaImage>, String> {
            Err("No artwork".into())
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
            Some(std::sync::Arc::new(font()))
        }
    }
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Text3d","opacity":0.5,
        "text":{"content":"3D","fontSize":192.0,"colorHex":"#FFFFFFFF","linkToAchievementValue":true},
        "text3d":Text3dSettings::default(),"effects":[{"id":new_id(),"kind":"ColorOverlay","colorHex":"#FFFF0000","opacity":1.0}]}]});
    entity["unlockValue"] = json!(12);
    editor
        .execute(
            "3D text",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let result = crate::render(editor.document(), &id, Some((128, 128)), &mut Fonts).unwrap();
    assert!(alpha(&result.achieved) > 10_000);
    assert!(result.achieved.pixels().all(|p| p[3] <= 128));
    assert!(
        result
            .achieved
            .pixels()
            .filter(|p| p[3] > 100)
            .all(|p| p[0] > 200 && p[1] == 0 && p[2] == 0)
    );
    assert_ne!(result.achieved, result.unachieved);
    assert!(editor.undo());
    assert!(editor.redo());
}

#[test]
#[ignore = "Writes a visual 3D text contact sheet into external build output"]
fn text3d_contact_sheet() {
    let font = font();
    let base = Text3dSettings {
        side_color_hex: "#FF6B4CB8".into(),
        ..Default::default()
    };
    let variants = [
        base.clone(),
        Text3dSettings {
            projection: TextProjection::Orthographic,
            ..base.clone()
        },
        Text3dSettings {
            shading: TextShading::Glossy,
            roughness: 0.8,
            ..base.clone()
        },
        Text3dSettings {
            rotation_x: -25.0,
            rotation_y: 35.0,
            rotation_z: -12.0,
            ..base.clone()
        },
        Text3dSettings {
            extrusion: 100.0,
            ..base.clone()
        },
        Text3dSettings {
            rotation_y: 155.0,
            ..base
        },
    ];
    let mut sheet = RgbaImage::from_pixel(768, 512, Rgba([32, 34, 41, 255]));
    for (i, settings) in variants.iter().enumerate() {
        let start = std::time::Instant::now();
        let _quick = render(
            &font,
            "3D",
            256.0,
            Rgba([255, 193, 65, 255]),
            settings,
            (64, 64),
            &|| false,
        )
        .unwrap();
        let quick = start.elapsed();
        let output = render(
            &font,
            "3D",
            256.0,
            Rgba([255, 193, 65, 255]),
            settings,
            (256, 256),
            &|| false,
        )
        .unwrap();
        println!(
            "3D text variant {i}: quick pass ready {quick:?}, final pass ready {:?}",
            start.elapsed()
        );
        image::imageops::overlay(
            &mut sheet,
            &output,
            (i % 3 * 256) as i64,
            (i / 3 * 256) as i64,
        );
    }
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(std::path::PathBuf::from)
        .expect("External build path required");
    let path = root.join("text3d-preview.png");
    sheet.save(&path).unwrap();
    println!("3D text preview: {}", path.display());
}

#[test]
#[ignore = "Writes relief, mask and shadow comparisons into external build output"]
fn text3d_relief_contact_sheet() {
    let font = font();
    let base = Text3dSettings {
        side_color_hex: "#FF6B4CB8".into(),
        push: 2.0,
        ..Default::default()
    };
    let bevel = Text3dSettings {
        emboss: 8.0,
        smooth_normals: true,
        ..base.clone()
    };
    let shadow = Text3dSettings {
        rotation_y: -45.0,
        extrusion: 70.0,
        light_azimuth: 70.0,
        light_elevation: 10.0,
        ..bevel.clone()
    };
    let variants = [
        (
            "Push -3 (thinner)",
            Text3dSettings {
                push: -3.0,
                ..base.clone()
            },
        ),
        (
            "Push 0",
            Text3dSettings {
                push: 0.0,
                ..base.clone()
            },
        ),
        (
            "Push +6 (bolder)",
            Text3dSettings {
                push: 6.0,
                ..base.clone()
            },
        ),
        ("Emboss 8 / smooth normals", bevel.clone()),
        (
            "Full mask",
            Text3dSettings {
                output: Text3dOutput::FullMask,
                ..bevel.clone()
            },
        ),
        (
            "Face mask",
            Text3dSettings {
                output: Text3dOutput::FaceMask,
                ..bevel.clone()
            },
        ),
        (
            "Emboss mask",
            Text3dSettings {
                output: Text3dOutput::EmbossMask,
                ..bevel.clone()
            },
        ),
        (
            "Faceted normals",
            Text3dSettings {
                smooth_normals: false,
                ..bevel
            },
        ),
        ("Without letter shadows", shadow.clone()),
        (
            "Letter shadows",
            Text3dSettings {
                shadow_strength: 1.0,
                ..shadow.clone()
            },
        ),
        (
            "Orthographic + emboss",
            Text3dSettings {
                projection: TextProjection::Orthographic,
                shadow_strength: 0.8,
                ..shadow.clone()
            },
        ),
        (
            "Emboss without extrusion",
            Text3dSettings {
                extrusion: 0.0,
                ..shadow
            },
        ),
    ];
    let mut sheet = RgbaImage::from_pixel(1024, 840, Rgba([32, 34, 41, 255]));
    for (i, (label, settings)) in variants.iter().enumerate() {
        let start = std::time::Instant::now();
        let _quick = render(
            &font,
            "3D",
            256.0,
            Rgba([255, 193, 65, 255]),
            settings,
            (64, 64),
            &|| false,
        )
        .unwrap();
        let quick = start.elapsed();
        let output = render(
            &font,
            "3D",
            256.0,
            Rgba([255, 193, 65, 255]),
            settings,
            (256, 256),
            &|| false,
        )
        .unwrap();
        println!(
            "{label}: quick pass ready {quick:?}, final pass ready {:?}",
            start.elapsed()
        );
        let x = (i % 4 * 256) as i64;
        let y = (i / 4 * 280) as i64;
        image::imageops::overlay(&mut sheet, &output, x, y);
        let caption = font
            .text(
                label,
                15.0,
                Rgba([235, 235, 245, 255]),
                "Center",
                (256, 24),
                &|| false,
            )
            .unwrap();
        image::imageops::overlay(&mut sheet, &caption, x, y + 256);
    }
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(std::path::PathBuf::from)
        .expect("External build path required");
    let path = root.join("text3d-relief-preview.png");
    sheet.save(&path).unwrap();
    println!("3D text relief preview: {}", path.display());
}

#[test]
#[ignore = "Writes reflectivity and mask subtraction comparisons into external build output"]
fn text3d_reflectivity_contact_sheet() {
    let font = font();
    let base = Text3dSettings {
        emboss: 8.0,
        push: 2.0,
        shading: TextShading::Glossy,
        roughness: 0.75,
        smooth_normals: true,
        side_color_hex: "#FF6B4CB8".into(),
        ..Default::default()
    };
    let variants = [
        (
            "Reflectivity 0",
            Text3dSettings {
                reflectivity: 0.0,
                ..base.clone()
            },
        ),
        ("Reflectivity 0.55", base.clone()),
        (
            "Reflectivity 1",
            Text3dSettings {
                reflectivity: 1.0,
                ..base.clone()
            },
        ),
        (
            "Emboss mask",
            Text3dSettings {
                output: Text3dOutput::EmbossMask,
                ..base.clone()
            },
        ),
        (
            "Face mask",
            Text3dSettings {
                output: Text3dOutput::FaceMask,
                ..base.clone()
            },
        ),
        (
            "Emboss mask minus face",
            Text3dSettings {
                output: Text3dOutput::EmbossMaskMinusFace,
                ..base
            },
        ),
    ];
    let mut sheet = RgbaImage::from_pixel(768, 560, Rgba([32, 34, 41, 255]));
    for (i, (label, settings)) in variants.iter().enumerate() {
        let output = render(
            &font,
            "3D",
            256.0,
            Rgba([255, 193, 65, 255]),
            settings,
            (256, 256),
            &|| false,
        )
        .unwrap();
        let x = (i % 3 * 256) as i64;
        let y = (i / 3 * 280) as i64;
        image::imageops::overlay(&mut sheet, &output, x, y);
        let caption = font
            .text(
                label,
                15.0,
                Rgba([235, 235, 245, 255]),
                "Center",
                (256, 24),
                &|| false,
            )
            .unwrap();
        image::imageops::overlay(&mut sheet, &caption, x, y + 256);
    }
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(std::path::PathBuf::from)
        .expect("External build path required");
    let path = root.join("text3d-reflectivity-preview.png");
    sheet.save(&path).unwrap();
    println!("3D text reflectivity preview: {}", path.display());
}
