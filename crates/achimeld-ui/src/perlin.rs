use achimeld_application::EditorPort;
use serde_json::{Value, json};

pub(crate) fn defaults() -> Value {
    json!({"seed":0,"frequency":4.0,"octaves":6,"persistence":0.5,"lacunarity":2.0,
        "enabledOctaves":[true,true,true,true,true,true,true,true,true,true,true,true],"useGradient":false,
        "gradient":{"kind":"Linear","angle":90.0,"stops":[
            {"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}})
}

pub(crate) fn properties(ui: &mut egui::Ui, port: &mut dyn EditorPort, settings: &mut Value) {
    let mut value = defaults();
    if let Some(stored) = settings.as_object() {
        for (key, stored) in stored {
            value[key] = stored.clone();
        }
    }
    let initial = value.clone();
    crate::inspector::integer(ui, "Seed", &mut value, "seed", 0..=i64::from(i32::MAX));
    crate::inspector::number(
        ui,
        "Noise frequency",
        &mut value,
        "frequency",
        0.1..=128.0,
        0.1,
    );
    crate::inspector::integer(ui, "Octaves", &mut value, "octaves", 1..=12);
    crate::inspector::number(
        ui,
        "Detail strength",
        &mut value,
        "persistence",
        0.0..=1.0,
        0.01,
    );
    crate::inspector::number(
        ui,
        "Frequency spacing",
        &mut value,
        "lacunarity",
        1.0..=4.0,
        0.05,
    );
    ui.weak(crate::localization::text(
        ui.ctx(),
        "Enable individual octaves, from coarse to fine.",
    ));
    let count = value["octaves"].as_u64().unwrap_or(6).clamp(1, 12) as usize;
    let mut enabled: Vec<bool> = (0..12)
        .map(|i| {
            value["enabledOctaves"]
                .get(i)
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .collect();
    let mut edited = false;
    ui.horizontal_wrapped(|ui| {
        for (i, enabled) in enabled.iter_mut().enumerate().take(count) {
            edited |= ui.checkbox(enabled, (i + 1).to_string()).changed();
        }
    });
    if edited {
        value["enabledOctaves"] = json!(enabled);
    }
    crate::inspector::boolean(ui, "Gradient colors", &mut value, "useGradient");
    if value["useGradient"] == true {
        ui.weak(crate::localization::text(
            ui.ctx(),
            "Stops map low noise values to high noise values.",
        ));
        crate::layers::gradient_presets(ui, port, &mut value["gradient"]);
        crate::layers::gradient_stops_ui(ui, &mut value["gradient"]);
    }
    if value != initial {
        *settings = value;
    }
}
