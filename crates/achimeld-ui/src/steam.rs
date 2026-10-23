use crate::Desktop;
use crate::buttons::UiButtonExt as _;
use achimeld_application::{EditorPort, EntityKind, UiRequest};
use serde_json::Value;
use std::collections::BTreeMap;

fn api_name(entity: &Value) -> &str {
    entity["apiName"]
        .as_str()
        .or_else(|| entity.pointer("/apiName/value").and_then(Value::as_str))
        .unwrap_or("")
}

fn localizations<'a>(snapshot: &'a Value, language: &str) -> BTreeMap<(&'a str, &'a str), &'a str> {
    snapshot["localizations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let field = entry["field"].as_str()?;
            let values = entry["values"].as_object()?;
            let text = values
                .iter()
                .find(|(key, value)| {
                    key.eq_ignore_ascii_case(language)
                        && value.as_str().is_some_and(|text| !text.is_empty())
                })
                .or_else(|| {
                    values
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case("English"))
                })
                .and_then(|(_, value)| value.as_str())?;
            Some(((api_name(entry), field), text))
        })
        .collect()
}

impl Desktop {
    pub(crate) fn steam_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            // Both sections share one scroll position. Never consult the editable
            // entities or their artwork when presenting the remote snapshot.
            egui::ScrollArea::vertical()
                .id_salt("steam-configuration")
                .show(ui, |ui| {
                    ui.heading("Steam");
                    let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Last loaded Steam configuration · read only");
                        if port.reading() {
                            ui.spinner();
                            ui.label("Reading Steam…");
                            if ui.app_button("Stop").clicked() {
                                self.request(UiRequest::CancelRead);
                            }
                        } else if ui.app_button("Refresh Steam configuration").clicked() {
                            self.request(UiRequest::ReadSteam);
                        }
                    });
                    if let Some(read_at) = snapshot["readAtUtc"].as_str() {
                        ui.weak(format!("Read at {read_at}"));
                    }
                    if snapshot.is_null() {
                        ui.label("Read Steam to view its configuration.");
                    } else if snapshot["isComplete"].as_bool() == Some(false) {
                        ui.colored_label(
                            crate::theme::color(port.preferences(), "Warning"),
                            "This Steam snapshot is incomplete.",
                        );
                    }
                    let filter = self.filter.to_lowercase();
                    let localized = localizations(snapshot, &port.preferences().language);
                    for (kind, title, name_field) in [
                        (
                            EntityKind::Achievement,
                            "Achievements",
                            "AchievementDisplayName",
                        ),
                        (EntityKind::Stat, "Stats", "StatDisplayName"),
                    ] {
                        ui.add_space(12.0);
                        ui.separator();
                        let entities = snapshot[kind.key()]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        ui.heading(format!("{title} ({})", entities.len()));
                        let mut shown = 0;
                        for (index, entity) in entities.iter().enumerate() {
                            let api = api_name(entity);
                            let name = localized.get(&(api, name_field)).copied().unwrap_or("");
                            if !filter.is_empty()
                                && !format!("{name} {api}").to_lowercase().contains(&filter)
                            {
                                continue;
                            }
                            shown += 1;
                            ui.push_id((kind.key(), index), |ui| {
                                egui::Frame::new()
                                    .fill(crate::theme::color(port.preferences(), "Panel"))
                                    .inner_margin(12)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.strong(if name.is_empty() { api } else { name });
                                        if !name.is_empty() {
                                            ui.weak(api);
                                        }
                                        if kind == EntityKind::Achievement {
                                            let description = localized
                                                .get(&(api, "AchievementDescription"))
                                                .copied()
                                                .unwrap_or("");
                                            if !description.is_empty() {
                                                ui.label(description);
                                            }
                                        }
                                        ui.horizontal_wrapped(|ui| {
                                            for (key, label, attribute) in [
                                                ("type", "Type", "data-type"),
                                                ("setBy", "Set by", "data-set-by"),
                                                ("hidden", "Hidden", "data-hidden"),
                                                (
                                                    "progressStatApiName",
                                                    "Progress stat",
                                                    "data-progress-stat",
                                                ),
                                                (
                                                    "unlockValue",
                                                    "Unlock value",
                                                    "data-unlock-value",
                                                ),
                                                (
                                                    "incrementOnly",
                                                    "Increment only",
                                                    "data-increment-only",
                                                ),
                                                ("minimum", "Minimum", "data-minimum"),
                                                ("maximum", "Maximum", "data-maximum"),
                                                ("defaultValue", "Default", "data-default"),
                                                (
                                                    "maximumChange",
                                                    "Maximum change",
                                                    "data-maximum-change",
                                                ),
                                                ("aggregated", "Aggregated", "data-aggregated"),
                                                (
                                                    "averageRateWindowSeconds",
                                                    "Rate window (s)",
                                                    "data-average-window",
                                                ),
                                                (
                                                    "displayOrder",
                                                    "Display order",
                                                    "data-display-order",
                                                ),
                                            ] {
                                                // The read CLI preserves normalized attributes;
                                                // synchronization stores the compatible typed schema.
                                                let value = entity
                                                    .get(key)
                                                    .filter(|v| !v.is_null())
                                                    .or_else(|| entity["attributes"].get(attribute))
                                                    .unwrap_or(&Value::Null);
                                                let value = value.get("value").unwrap_or(value);
                                                if !value.is_null() && value.as_str() != Some("") {
                                                    let text = match value {
                                                        Value::String(text) if text == "true" => {
                                                            "Yes".into()
                                                        }
                                                        Value::String(text) if text == "false" => {
                                                            "No".into()
                                                        }
                                                        Value::String(text) => text.clone(),
                                                        Value::Bool(value) => {
                                                            if *value { "Yes" } else { "No" }.into()
                                                        }
                                                        _ => value.to_string(),
                                                    };
                                                    ui.label(format!("{label}: {text}"));
                                                }
                                            }
                                        });
                                    });
                            });
                        }
                        if shown == 0 {
                            ui.weak(if snapshot.is_null() {
                                "Not loaded"
                            } else if entities.is_empty() {
                                "None in the loaded Steam configuration"
                            } else {
                                "No matches"
                            });
                        }
                    }
                });
        });
    }
}
