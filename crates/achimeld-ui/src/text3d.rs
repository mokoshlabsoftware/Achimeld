use crate::buttons::UiButtonExt as _;
use crate::inspector::{choice, number};
use serde_json::Value;

pub(crate) fn properties(ui: &mut egui::Ui, settings: &mut Value) {
    output(ui, settings);
    ui.separator();
    ui.strong("Camera");
    choice(
        ui,
        "Projection",
        settings,
        "projection",
        &["Perspective", "Orthographic"],
    );
    number(
        ui,
        "Camera distance",
        settings,
        "cameraDistance",
        32.0..=8192.0,
        2.0,
    );
    if settings["projection"] == "Orthographic" {
        number(
            ui,
            "View height",
            settings,
            "orthographicSize",
            32.0..=8192.0,
            2.0,
        );
    } else {
        number(ui, "Lens (mm)", settings, "focalLength", 10.0..=300.0, 0.5);
    }
    ui.separator();
    ui.strong("3D object");
    number(
        ui,
        "Extrusion depth",
        settings,
        "extrusion",
        0.0..=512.0,
        0.5,
    );
    slider(
        ui,
        "Push",
        settings,
        "push",
        -64.0..=64.0,
        "Negative values thin the letters; positive values make them bolder.",
    );
    slider(
        ui,
        "Emboss",
        settings,
        "emboss",
        0.0..=64.0,
        "Adds a raised beveled rim around the letter faces.",
    );
    axes(
        ui,
        "Position",
        settings,
        ["positionX", "positionY", "positionZ"],
        -4096.0..=4096.0,
        1.0,
    );
    axes(
        ui,
        "Rotation (°)",
        settings,
        ["rotationX", "rotationY", "rotationZ"],
        -360.0..=360.0,
        1.0,
    );
    ui.weak("X: right · Y: up · Z: toward camera");
    ui.separator();
    ui.strong("Shading");
    choice(
        ui,
        "Material",
        settings,
        "shading",
        &["Matte", "Glossy", "Unlit"],
    );
    let mut smooth = settings["smoothNormals"].as_bool().unwrap_or(false);
    if ui
        .checkbox(
            &mut smooth,
            crate::localization::text(ui.ctx(), "Smooth normals"),
        )
        .changed()
    {
        settings["smoothNormals"] = serde_json::json!(smooth);
    }
    crate::layers::color(ui, "Side color", settings, "sideColorHex");
    if settings["shading"] != "Unlit" {
        slider(
            ui,
            "Letter shadows",
            settings,
            "shadowStrength",
            0.0..=1.0,
            "Strength of shadows cast by the text onto other letter surfaces.",
        );
        number(ui, "Ambient light", settings, "ambient", 0.0..=1.0, 0.01);
        number(
            ui,
            "Light intensity",
            settings,
            "lightIntensity",
            0.0..=2.0,
            0.01,
        );
        number(
            ui,
            "Light azimuth (°)",
            settings,
            "lightAzimuth",
            -180.0..=180.0,
            1.0,
        );
        number(
            ui,
            "Light elevation (°)",
            settings,
            "lightElevation",
            -90.0..=90.0,
            1.0,
        );
        if settings["shading"] == "Glossy" {
            slider(
                ui,
                "Reflectivity",
                settings,
                "reflectivity",
                0.0..=1.0,
                "Strength of the glossy reflection highlight.",
            );
            number(ui, "Roughness", settings, "roughness", 0.0..=1.0, 0.01);
        }
    }
}

fn slider(
    ui: &mut egui::Ui,
    label: &str,
    settings: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<f64>,
    hint: &str,
) {
    let mut value =
        settings[key]
            .as_f64()
            .unwrap_or(if key == "reflectivity" { 0.55 } else { 0.0 });
    let response = ui.scope(|ui| {
        if crate::sliders::value_slider(ui, label, &mut value, range, "", 2) {
            settings[key] = serde_json::json!(value);
        }
    });
    response
        .response
        .on_hover_text(crate::localization::text(ui.ctx(), hint));
}

fn axes(
    ui: &mut egui::Ui,
    label: &str,
    settings: &mut Value,
    keys: [&str; 3],
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) {
    ui.push_id(label, |ui| {
        for (axis, key) in ["X", "Y", "Z"].into_iter().zip(keys) {
            number(
                ui,
                &format!("{label} {axis}"),
                settings,
                key,
                range.clone(),
                speed,
            );
        }
    });
}

fn output(ui: &mut egui::Ui, settings: &mut Value) {
    let options = [
        ("FullColor", "Full color"),
        ("FullMask", "Full mask"),
        ("FaceMask", "Face mask"),
        ("EmbossMask", "Emboss mask"),
        ("EmbossMaskMinusFace", "Emboss mask − face"),
    ];
    let mut selected = settings["output"]
        .as_str()
        .unwrap_or("FullColor")
        .to_owned();
    let before = selected.clone();
    crate::inspector::field(ui, "Output", 120.0, |ui| {
        let label = options
            .iter()
            .find(|(key, _)| *key == selected)
            .map_or("Full color", |(_, label)| *label);
        crate::dropdowns::Dropdown::from_id_salt("text3d-output")
            .width(ui.available_width().min(180.0))
            .truncate()
            .selected_text(crate::localization::text(ui.ctx(), label))
            .show_ui(ui, |ui| {
                for (key, label) in options {
                    ui.app_selectable_value(
                        &mut selected,
                        key.into(),
                        crate::localization::text(ui.ctx(), label),
                    );
                }
            })
            .response
            .on_hover_text(crate::localization::text(
                ui.ctx(),
                if selected == "EmbossMaskMinusFace" {
                    "Embossed text with flat faces removed; keeps bevels and 3D sides."
                } else {
                    label
                },
            ));
    });
    if before != selected {
        settings["output"] = serde_json::json!(selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use achimeld_application::text3d::Text3dSettings;

    #[test]
    fn text3d_is_available_as_a_layer_and_reading_properties_preserves_settings() {
        assert!(crate::layer_icons::LAYERS.contains(&"Text3d"));
        let layer = crate::layer_palette::new_layer("Text3d");
        assert_eq!(layer["displayName"], "3D text");
        assert_eq!(
            Text3dSettings::from_value(&layer["text3d"]).unwrap(),
            Text3dSettings::default()
        );
        for width in [180.0, 320.0, 600.0] {
            for (projection, output) in [
                ("Perspective", "FullColor"),
                ("Orthographic", "FullColor"),
                ("Perspective", "EmbossMaskMinusFace"),
                ("Orthographic", "EmbossMaskMinusFace"),
            ] {
                let mut value = layer["text3d"].clone();
                value["projection"] = serde_json::json!(projection);
                value["output"] = serde_json::json!(output);
                value["shading"] = serde_json::json!("Glossy");
                let before = value.clone();
                let ctx = egui::Context::default();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let available = ui.available_width();
                        properties(ui, &mut value);
                        assert!(
                            ui.min_rect().width() <= available + 1.0,
                            "Property fields overflow at {width}: {projection}/{output}, available {available}, used {}",
                            ui.min_rect().width()
                        );
                    },
                );
                output.textures_delta.clear();
                assert_eq!(
                    value, before,
                    "Inspecting a layer must not change the project"
                );
            }
        }
    }
}
