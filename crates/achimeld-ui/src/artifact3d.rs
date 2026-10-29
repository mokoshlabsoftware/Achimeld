use crate::{
    buttons::{self, Button, UiButtonExt as _},
    inspector::{number, text},
};
use achimeld_application::artifact3d::{
    ArtifactPart, ArtifactSettings, MAX_PARTS, PRESETS, Primitive, preset,
};
use serde_json::{Value, json};

pub(crate) fn properties(ui: &mut egui::Ui, stored: &mut Value) {
    let Ok(parsed) = ArtifactSettings::from_value(stored) else {
        ui.label("Invalid 3D artifact settings");
        return;
    };
    let mut settings = json!(parsed);
    let initial = settings.clone();
    crate::inspector::field(ui, "Output", 120.0, |ui| {
        let options = [
            ("FullColor", "Full color (RGB)"),
            ("AlphaMask", "Alpha mask"),
        ];
        let mut selected = settings["output"]
            .as_str()
            .unwrap_or("FullColor")
            .to_owned();
        let before = selected.clone();
        let label = options
            .iter()
            .find(|(key, _)| *key == selected)
            .map_or("Full color (RGB)", |(_, label)| *label);
        crate::dropdowns::Dropdown::from_id_salt("artifact-output")
            .truncate()
            .width(ui.available_width().min(180.0))
            .selected_text(label)
            .show_ui(ui, |ui| {
                for (key, label) in options {
                    ui.app_selectable_value(&mut selected, key.into(), label);
                }
            });
        if selected != before {
            settings["output"] = json!(selected);
        }
    });
    ui.weak("Full color (RGB) or a white alpha mask on transparency.");
    let selected_key = ui.id().with("artifact-part");
    let mut selected = ui
        .ctx()
        .data(|d| d.get_temp::<String>(selected_key))
        .unwrap_or_default();
    crate::inspector::field(ui, "Artifact preset", 120.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt("artifact-presets")
            .truncate()
            .width(ui.available_width().min(180.0))
            .selected_text("Replace with preset…")
            .show_ui(ui, |ui| {
                for (kind, name) in PRESETS {
                    if ui.app_button(name).clicked() {
                        settings["parts"] = json!(preset(kind));
                        selected.clear();
                        ui.close();
                    }
                }
            });
    });
    ui.strong("Parts");
    let parts = settings["parts"].as_array_mut().expect("Parsed parts");
    if !parts.iter().any(|p| p["id"] == selected) {
        selected = parts
            .first()
            .and_then(|p| p["id"].as_str())
            .unwrap_or("")
            .into();
    }
    buttons::bar(ui, 0.0, |ui| {
        let width: f32 = ["Add part", "Edit part"]
            .iter()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(
                        (*label).into(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
                    + 2.0 * ui.spacing().button_padding.x
            })
            .sum();
        let compact = width > ui.available_width();
        ui.add_enabled_ui(parts.len() < MAX_PARTS, |ui| {
            buttons::menu(
                ui,
                Button::new(if compact { "Add" } else { "Add part" }).bar(),
                |ui| {
                    for (primitive, name) in [
                        (Primitive::Cube, "Cube"),
                        (Primitive::Cone, "Cone"),
                        (Primitive::Torus, "Torus"),
                        (Primitive::Cylinder, "Cylinder / capsule"),
                        (Primitive::Sphere, "Sphere"),
                    ] {
                        if ui.app_button(name).clicked() {
                            let part = ArtifactPart {
                                id: achimeld_application::new_id(),
                                name: name.into(),
                                primitive,
                                size: if primitive == Primitive::Torus {
                                    [180.0, 40.0, 180.0]
                                } else {
                                    [120.0; 3]
                                },
                                rotation: if primitive == Primitive::Torus {
                                    [90.0, 0.0, 0.0]
                                } else {
                                    [0.0; 3]
                                },
                                ..Default::default()
                            };
                            selected = part.id.clone();
                            parts.push(json!(part));
                            ui.close();
                        }
                    }
                },
            );
        });
        if let Some(index) = parts.iter().position(|p| p["id"] == selected) {
            buttons::menu(
                ui,
                Button::new(if compact { "Edit" } else { "Edit part" }).bar(),
                |ui| {
                    if ui
                        .add_enabled(parts.len() < MAX_PARTS, Button::new("Duplicate"))
                        .clicked()
                    {
                        let mut copy = parts[index].clone();
                        selected = achimeld_application::new_id();
                        copy["id"] = json!(selected);
                        parts.insert(index + 1, copy);
                        ui.close();
                    }
                    if ui.add_enabled(index > 0, Button::new("Move up")).clicked() {
                        parts.swap(index, index - 1);
                        ui.close();
                    }
                    if ui
                        .add_enabled(index + 1 < parts.len(), Button::new("Move down"))
                        .clicked()
                    {
                        parts.swap(index, index + 1);
                        ui.close();
                    }
                    if ui.app_button("Remove").clicked() {
                        parts.remove(index);
                        selected.clear();
                        ui.close();
                    }
                },
            );
        }
    });
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .id_salt("artifact-parts")
            .max_height(190.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for part in parts.iter() {
                    let name = part["name"].as_str().unwrap_or("Part");
                    let label = if part["visible"] == false {
                        format!("{name} (hidden)")
                    } else {
                        name.into()
                    };
                    let height = buttons::bar_height(ui, 0.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), height],
                            Button::selectable(part["id"] == selected, label)
                                .bar()
                                .wrap_mode(egui::TextWrapMode::Truncate),
                        )
                        .clicked()
                    {
                        selected = part["id"].as_str().unwrap_or("").into();
                    }
                }
            });
    });
    if let Some(part) = parts.iter_mut().find(|p| p["id"] == selected) {
        let id = selected.clone();
        ui.push_id(id, |ui| part_properties(ui, part));
    } else {
        ui.weak("Add a primitive or choose an artifact preset.");
    }
    ui.ctx().data_mut(|d| d.insert_temp(selected_key, selected));
    ui.separator();
    ui.strong("Artifact angle and position");
    axes(
        ui,
        "Rotation (°)",
        &mut settings,
        "rotation",
        -360.0..=360.0,
    );
    axes(ui, "Position", &mut settings, "position", -4096.0..=4096.0);
    ui.weak("X: right · Y: up · Z: toward camera");
    ui.separator();
    ui.strong("Camera");
    choice(
        ui,
        "Projection",
        &mut settings,
        "projection",
        &["Perspective", "Orthographic"],
    );
    number(
        ui,
        "Camera distance",
        &mut settings,
        "cameraDistance",
        32.0..=8192.0,
        2.0,
    );
    if settings["projection"] == "Orthographic" {
        number(
            ui,
            "View height",
            &mut settings,
            "orthographicSize",
            32.0..=8192.0,
            2.0,
        );
    } else {
        number(
            ui,
            "Lens (mm)",
            &mut settings,
            "focalLength",
            10.0..=300.0,
            0.5,
        );
    }
    ui.separator();
    ui.strong("Lighting");
    number(
        ui,
        "Ambient light",
        &mut settings,
        "ambient",
        0.0..=1.0,
        0.01,
    );
    number(
        ui,
        "Light intensity",
        &mut settings,
        "lightIntensity",
        0.0..=2.0,
        0.01,
    );
    number(
        ui,
        "Light azimuth (°)",
        &mut settings,
        "lightAzimuth",
        -180.0..=180.0,
        1.0,
    );
    number(
        ui,
        "Light elevation (°)",
        &mut settings,
        "lightElevation",
        -90.0..=90.0,
        1.0,
    );
    if settings != initial {
        *stored = settings;
    }
}

fn part_properties(ui: &mut egui::Ui, part: &mut Value) {
    text(ui, "Part name", part, "name");
    crate::inspector::boolean(ui, "Visible", part, "visible");
    choice(
        ui,
        "Primitive",
        part,
        "primitive",
        &["Cube", "Cone", "Torus", "Cylinder", "Sphere"],
    );
    axes(ui, "Size", part, "size", 0.1..=2048.0);
    axes(ui, "Position", part, "position", -4096.0..=4096.0);
    axes(ui, "Rotation (°)", part, "rotation", -360.0..=360.0);
    if matches!(part["primitive"].as_str(), Some("Cube" | "Cylinder")) {
        slider(ui, "Rounding", part, "rounding");
        ui.weak("0: sharp · small values: chamfer · 1: fully rounded");
    }
    if part["primitive"] != "Cube" {
        number(
            ui,
            "Range start (°)",
            part,
            "startAngle",
            -360.0..=360.0,
            1.0,
        );
        number(ui, "Range sweep (°)", part, "sweepAngle", 0.5..=360.0, 1.0);
    }
    if part["primitive"] == "Torus" {
        number(ui, "Tube radius", part, "tubeRadius", 0.01..=0.95, 0.01);
    }
    if part["primitive"] == "Sphere" {
        let top = part["cutTop"].as_f64().unwrap_or(1.0);
        number(
            ui,
            "Bottom cut",
            part,
            "cutBottom",
            -1.0..=top - 0.001,
            0.01,
        );
        let bottom = part["cutBottom"].as_f64().unwrap_or(-1.0);
        number(ui, "Top cut", part, "cutTop", bottom + 0.001..=1.0, 0.01);
        ui.weak("Cuts run from −1 to 1 along local Y. Zero gives a hemisphere.");
        number(
            ui,
            "Wall thickness",
            part,
            "wallThickness",
            0.0..=0.95,
            0.01,
        );
        ui.weak("Zero is solid; positive values create a hollow shell.");
    }
    choice(
        ui,
        "Material",
        part,
        "shading",
        &["Matte", "Glossy", "Unlit"],
    );
    crate::layers::color(ui, "Material color", part, "colorHex");
    if part["shading"] == "Glossy" {
        slider(ui, "Reflectivity", part, "reflectivity");
        slider(ui, "Roughness", part, "roughness");
    }
}

fn slider(ui: &mut egui::Ui, label: &str, value: &mut Value, key: &str) {
    let mut n = value[key].as_f64().unwrap_or(0.0);
    if crate::sliders::value_slider(ui, label, &mut n, 0.0..=1.0, "", 2) {
        value[key] = json!(n);
    }
}

fn choice(ui: &mut egui::Ui, label: &str, value: &mut Value, key: &str, options: &[&str]) {
    let mut selected = value[key].as_str().unwrap_or(options[0]).to_owned();
    let before = selected.clone();
    crate::inspector::field(ui, label, 120.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt(key)
            .truncate()
            .width(ui.available_width().min(180.0))
            .selected_text(&selected)
            .show_ui(ui, |ui| {
                for option in options {
                    ui.app_selectable_value(&mut selected, (*option).into(), *option);
                }
            });
    });
    if selected != before {
        value[key] = json!(selected);
    }
}

fn axes(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<f64>,
) {
    ui.push_id((label, key), |ui| {
        let mut field = |ui: &mut egui::Ui, i: usize| {
            let mut n = value[key][i].as_f64().unwrap_or(0.0);
            crate::inspector::field(ui, &format!("{label} {}", ["X", "Y", "Z"][i]), 60.0, |ui| {
                if ui
                    .add(egui::DragValue::new(&mut n).range(range.clone()).speed(0.5))
                    .changed()
                    && n.is_finite()
                {
                    value[key][i] = json!(n);
                }
            });
        };
        for i in 0..3 {
            field(ui, i);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact3d_inspector_preserves_recipes_and_fits_narrow_widths() {
        assert!(crate::layer_icons::LAYERS.contains(&"Artifact3d"));
        let layer = crate::layer_palette::new_layer("Artifact3d");
        assert_eq!(layer["displayName"], "3D artifact");
        for (width, font_size) in [
            (180.0, 13.0),
            (180.0, 22.0),
            (320.0, 13.0),
            (320.0, 22.0),
            (600.0, 13.0),
        ] {
            for (preset_kind, _) in PRESETS {
                let mut value = json!(ArtifactSettings {
                    parts: preset(preset_kind),
                    ..Default::default()
                });
                let before = value.clone();
                let ctx = egui::Context::default();
                ctx.global_style_mut(|style| {
                    for font in style.text_styles.values_mut() {
                        font.size = font_size;
                    }
                });
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 2600.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let available = ui.available_width();
                        properties(ui, &mut value);
                        assert!(
                            ui.min_rect().width() <= available + 1.0,
                            "Overflow at {width}: {} > {available}",
                            ui.min_rect().width()
                        );
                    },
                );
                output.textures_delta.clear();
                assert_eq!(value, before, "Inspection mutated recipe");
            }
        }
    }

    #[test]
    #[ignore = "Native artifact inspector screenshot in external build output"]
    fn artifact3d_native_inspector() {
        use winit::platform::windows::EventLoopBuilderExtWindows as _;
        struct Window {
            settings: Value,
            texture: egui::TextureHandle,
            started: std::time::Instant,
            requested: bool,
        }
        impl eframe::App for Window {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
                ui.set_style(ui.ctx().global_style());
                egui::Panel::left("artifact-inspector")
                    .exact_size(320.0)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| properties(ui, &mut self.settings));
                    });
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.heading("3D artifact · Cup");
                    ui.add(egui::Image::new((
                        self.texture.id(),
                        egui::vec2(460.0, 460.0),
                    )));
                });
                if !self.requested && self.started.elapsed().as_secs_f32() > 0.5 {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                            egui::UserData::default(),
                        ));
                    self.requested = true;
                }
                if let Some(image) = ui.input(|i| {
                    i.events.iter().find_map(|e| {
                        if let egui::Event::Screenshot { image, .. } = e {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                }) {
                    let bytes: Vec<_> = image
                        .pixels
                        .iter()
                        .flat_map(|p| p.to_srgba_unmultiplied())
                        .collect();
                    image::save_buffer(
                        crate::test_output::root().join("artifact3d-inspector.png"),
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                assert!(
                    self.started.elapsed().as_secs() < 20,
                    "Screenshot timed out"
                );
                ui.ctx().request_repaint();
            }
        }
        let pixels = image::open(crate::test_output::root().join("artifact3d-cup.png"))
            .unwrap()
            .to_rgba8();
        eframe::run_native(
            "Achimeld 3D artifact",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_decorations(false)
                    .with_inner_size([840.0, 920.0]),
                event_loop_builder: Some(Box::new(|builder| {
                    builder.with_any_thread(true);
                })),
                ..Default::default()
            },
            Box::new(move |cc| {
                crate::tests::configure_test_fonts(&cc.egui_ctx);
                let texture = cc.egui_ctx.load_texture(
                    "artifact-preview",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [pixels.width() as usize, pixels.height() as usize],
                        pixels.as_raw(),
                    ),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                Ok(Box::new(Window {
                    settings: json!(ArtifactSettings {
                        parts: preset(achimeld_application::artifact3d::ArtifactPreset::Cup),
                        ..Default::default()
                    }),
                    texture,
                    started: std::time::Instant::now(),
                    requested: false,
                }))
            }),
        )
        .unwrap();
    }
}
