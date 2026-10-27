//! Opening breadcrumbs without writing on every animation/repaint frame.
use achimeld_application::diagnostics::{Operation, effect_kind};
use serde_json::Value;

pub(crate) fn opening(ui: &egui::Ui, name: &'static str, value: &Value) -> Option<Operation> {
    // Ids are used only for local change detection and never emitted.
    let identity = egui::Id::new((value["id"].as_str(), value["kind"].as_str()));
    let key = if name == "open layer effects" {
        ui.id().with(("effect diagnostics", name, identity))
    } else {
        ui.id().with(("effect diagnostics", name))
    };
    let frame = ui.ctx().cumulative_frame_nr();
    let opened = ui.ctx().data_mut(|data| {
        let previous = data.get_temp::<(egui::Id, u64)>(key);
        data.insert_temp(key, (identity, frame));
        !previous.is_some_and(|(id, last)| id == identity && frame <= last.saturating_add(1))
    });
    opened.then(|| {
        let operation = Operation::begin(name);
        operation.checkpoint(&format!(
            "kind={}; effect_count={}",
            effect_kind(value["kind"].as_str().unwrap_or("")),
            value["effects"].as_array().map_or(0, Vec::len),
        ));
        operation
    })
}

pub(crate) fn completed(operation: Option<Operation>) {
    if let Some(operation) = operation {
        operation.finish("completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_opening_is_logged_on_selection_and_reopen_but_not_repaints() {
        let context = egui::Context::default();
        for (id, visible, expected) in [
            ("a", true, true),
            ("a", true, false),
            ("b", true, true),
            ("b", false, false),
            ("b", true, true),
            ("b", true, false),
        ] {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                if visible {
                    let operation = opening(
                        ui,
                        "open effect properties",
                        &serde_json::json!({"id":id,"kind":"Outline"}),
                    );
                    assert_eq!(operation.is_some(), expected);
                    completed(operation);
                }
            });
            output.textures_delta.clear();
        }
    }
}
