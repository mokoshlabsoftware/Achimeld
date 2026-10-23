use achimeld_application::EditorPort;
use serde_json::{Value, json};

pub(crate) fn properties(ui: &mut egui::Ui, port: &mut dyn EditorPort, settings: &mut Value) {
    let mut inverted = settings["invert"].as_bool().unwrap_or(false);
    if ui
        .checkbox(
            &mut inverted,
            crate::localization::text(ui.ctx(), "Invert vignette"),
        )
        .on_hover_text(crate::localization::text(
            ui.ctx(),
            "Fill the center instead of the edges.",
        ))
        .changed()
    {
        settings["invert"] = json!(inverted);
    }
    let previous = settings["useGradient"].as_bool().unwrap_or(false);
    let mut fill = json!({"fill":if previous {"Gradient map"} else {"Color"}});
    crate::inspector::choice(ui, "Fill", &mut fill, "fill", &["Color", "Gradient map"]);
    let gradient = fill["fill"] == "Gradient map";
    if gradient != previous {
        settings["useGradient"] = json!(gradient);
        if gradient && !settings["gradient"].is_object() {
            settings["gradient"] = default_gradient();
        }
    }
    if gradient {
        // Keep sparse legacy records unchanged until a control is edited.
        let mut value = settings["gradient"].clone();
        if !value.is_object() {
            value = default_gradient();
        }
        let initial = value.clone();
        crate::layers::gradient_presets(ui, port, &mut value);
        ui.weak(crate::localization::text(
            ui.ctx(),
            "Stops follow the fade from transparent to full strength.",
        ));
        crate::layers::gradient_stops_ui(ui, &mut value);
        if value != initial {
            settings["gradient"] = value;
        }
    } else {
        crate::layers::color(ui, "Color", settings, "colorHex");
    }
    for (name, key, default, maximum) in [
        ("Roundness", "roundness", 0.5, 1.0),
        ("Inset", "inset", 0.15, 0.95),
        ("Softness", "softness", 0.2, 1.0),
    ] {
        let mut value = json!({key:settings[key].as_f64().unwrap_or(default)});
        let initial = value.clone();
        crate::inspector::number(ui, name, &mut value, key, 0.0..=maximum, 0.01);
        if value != initial {
            settings[key] = value[key].clone();
        }
    }
}

fn default_gradient() -> Value {
    json!({"kind":"Linear","angle":90.0,"stops":[
        {"offset":0.0,"colorHex":"#FF36D1DC"},
        {"offset":0.5,"colorHex":"#FF7C4DFF"},
        {"offset":1.0,"colorHex":"#FFFF4FA3"}
    ]})
}
