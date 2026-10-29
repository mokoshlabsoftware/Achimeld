//! Mask entries and commands share the ordinary editor transaction and history.
use crate::buttons::UiButtonExt as _;
use crate::layer_palette::{
    LayerAction, blocked_mask_sources, blocked_mask_targets, layer_control_id,
};
use crate::{Desktop, icons};
use achimeld_application::EditorPort;
use serde_json::{Value, json};

/// Snapshot only graph and label fields before the editable layer-row iteration.
pub(crate) fn menu_layers(layers: &[Value]) -> Vec<Value> {
    layers
        .iter()
        .map(|layer| {
            json!({
                "id":layer["id"], "kind":layer["kind"], "displayName":layer["displayName"],
                "parentFolderId":layer["parentFolderId"], "maskLayerId":layer["maskLayerId"],
                "maskSourceLayerId":layer["maskSourceLayerId"], "visible":layer["visible"]
            })
        })
        .collect()
}

pub(crate) fn new_mask(kind: &str) -> Value {
    let mut mask = crate::layer_palette::new_layer(kind);
    mask["displayName"] = json!("");
    if kind == "AchievementImage" {
        mask["generalImageIndex"] = json!(0);
    }
    mask["visible"] = json!(false);
    for key in [
        "opacity",
        "blendMode",
        "effects",
        "maskLayerId",
        "maskMode",
        "maskInvert",
    ] {
        mask.as_object_mut().expect("Layer object").remove(key);
    }
    achimeld_application::masks::grayscale_colors(&mut mask);
    mask
}

impl Desktop {
    pub(crate) fn mask_commands(&mut self, ui: &mut egui::Ui, layer: &Value, layers: &[Value]) {
        let owner = layer["id"].as_str().unwrap_or("");
        if achimeld_application::masks::is_source(layers, owner) {
            return;
        }
        let has_mask = layer["maskLayerId"].is_string();
        let mask_ids: std::collections::BTreeSet<_> = layers
            .iter()
            .filter_map(|l| l["maskLayerId"].as_str())
            .chain(
                layers
                    .iter()
                    .filter(|l| l["kind"] == "MaskReference")
                    .filter_map(|l| l["id"].as_str()),
            )
            .collect();
        ui.app_menu_button(if has_mask { "Replace mask" } else { "Add mask" }, |ui| {
            if let Some(kind) = crate::layer_palette::mask_type_menu(ui) {
                self.layer_palette.pending = Some(LayerAction::AddMask {
                    owner: owner.into(),
                    kind,
                });
            }
        });
        let blocked = blocked_mask_sources(layers, owner);
        let sources: Vec<_> = layers
            .iter()
            .filter(|source| {
                source["id"]
                    .as_str()
                    .is_some_and(|id| !blocked.contains(id) && !mask_ids.contains(id))
                    && source["maskLayerId"].is_string()
            })
            .collect();
        if !sources.is_empty() {
            ui.app_menu_button("Use mask from", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for source in sources {
                            if ui
                                .app_button(source["displayName"].as_str().unwrap_or("Layer"))
                                .clicked()
                            {
                                self.layer_palette.pending = Some(LayerAction::ReferenceMask {
                                    owner: owner.into(),
                                    source: source["id"].as_str().unwrap_or("").into(),
                                });
                                ui.close();
                            }
                        }
                    });
            });
        }
        if !has_mask {
            return;
        }
        if ui.app_button("Edit mask").clicked() {
            self.selected_layer = layer["maskLayerId"].as_str().map(str::to_owned);
            self.layer_palette.selected_effect = None;
            ui.close();
        }
        let blocked = blocked_mask_targets(layers, owner);
        let targets: Vec<_> = layers
            .iter()
            .filter(|target| {
                target["id"]
                    .as_str()
                    .is_some_and(|id| !mask_ids.contains(id) && !blocked.contains(id))
            })
            .collect();
        if !targets.is_empty() {
            ui.app_menu_button("Reference mask to", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for target in targets {
                            if ui
                                .app_button(target["displayName"].as_str().unwrap_or("Layer"))
                                .clicked()
                            {
                                self.layer_palette.pending = Some(LayerAction::ReferenceMask {
                                    owner: target["id"].as_str().unwrap_or("").into(),
                                    source: owner.into(),
                                });
                                ui.close();
                            }
                        }
                    });
            });
        }
        if ui.app_button("Invert mask").clicked() {
            self.layer_palette.pending = Some(LayerAction::InvertMask {
                owner: owner.into(),
            });
            ui.close();
        }
        if layer["maskMode"] != "Luminance" && ui.app_button("Use grayscale mask").clicked() {
            self.layer_palette.pending = Some(LayerAction::GrayscaleMask {
                owner: owner.into(),
            });
            ui.close();
        }
        for (title, delete_source) in [("Remove mask", false), ("Delete mask", true)] {
            if ui.app_button(title).clicked() {
                self.layer_palette.pending = Some(LayerAction::RemoveMask {
                    owner: owner.into(),
                    delete_source,
                });
                ui.close();
            }
        }
    }

    pub(crate) fn mask_context_menu(
        &mut self,
        response: &egui::Response,
        layer: &Value,
        layers: &[Value],
    ) {
        if layer["maskLayerId"].is_string() {
            crate::buttons::context_menu(response, |ui| self.mask_commands(ui, layer, layers));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mask_entry(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        achievement: &str,
        owner: &mut Value,
        layers: &[Value],
        depth: usize,
    ) -> Option<(String, egui::Rect)> {
        let mask = owner["maskLayerId"]
            .as_str()
            .and_then(|id| layers.iter().find(|l| l["id"] == id))?;
        let id = mask["id"].as_str().unwrap_or("");
        let texture = self.layer_texture(ui.ctx(), port, achievement, id);
        let (response, preview_rect) = ui
            .push_id(
                ("nested-mask", owner["id"].as_str().map(str::to_owned)),
                |ui| {
                    let height = crate::layer_toolbar::HEIGHT;
                    crate::buttons::bar_with_height(ui, height, |ui| {
                        ui.spacing_mut().button_padding.x = 0.0;
                        ui.add_space(
                            ((depth + 1) as f32 * 22.0).min((ui.available_width() - 80.0).max(0.0)),
                        );
                        if mask["kind"] == "Folder" {
                            let collapsed = self.layer_palette.collapsed.contains(id);
                            if icons::sized_button(
                                ui,
                                icons::Icon::Chevron(collapsed),
                                "Expand / collapse folder",
                                true,
                                height,
                            )
                            .clicked()
                            {
                                if collapsed {
                                    self.layer_palette.collapsed.remove(id);
                                } else {
                                    self.layer_palette.collapsed.insert(id.into());
                                }
                            }
                        }
                        let preview = texture
                            .or_else(|| crate::layer_icons::texture(ui.ctx(), "MaskReference"))
                            .expect("Embedded mask icon");
                        let thumbnail = ui.add(
                            crate::buttons::Button::new(egui::Image::new((
                                preview,
                                egui::Vec2::splat(height - 4.0),
                            )))
                            .id(layer_control_id(
                                achievement,
                                owner["id"].as_str().unwrap_or(""),
                                "mask-entry-preview",
                            ))
                            .bar()
                            .min_size(egui::Vec2::splat(height))
                            .image_tint_follows_text_color(texture.is_none()),
                        );
                        channels(ui, owner);
                        let label = ui.add(
                            crate::buttons::Button::new("")
                                .id(layer_control_id(
                                    achievement,
                                    owner["id"].as_str().unwrap_or(""),
                                    "mask-entry",
                                ))
                                .bar()
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .selected(self.selected_layer.as_deref() == Some(id))
                                .align(egui::Align2::LEFT_CENTER)
                                .min_size(egui::vec2(ui.available_width(), height)),
                        );
                        let preview_rect = egui::Rect::from_center_size(
                            thumbnail.rect.center(),
                            egui::Vec2::splat(height - 4.0),
                        );
                        (thumbnail | label, preview_rect)
                    })
                    .inner
                },
            )
            .inner;
        if response.clicked() {
            self.selected_layer = Some(id.into());
            self.layer_palette.selected_effect = None;
        }
        self.mask_context_menu(&response, owner, layers);
        Some((id.into(), preview_rect))
    }
}

pub(crate) fn channels(ui: &mut egui::Ui, owner: &mut Value) {
    let mut selected: [bool; 4] = std::array::from_fn(|i| {
        owner["maskChannels"][i]
            .as_bool()
            .unwrap_or(owner["maskMode"] == "Luminance" || i == 3)
    });
    for (i, label) in ["R", "G", "B", "A"].iter().enumerate() {
        if ui
            .add(
                crate::buttons::Button::new(*label)
                    .bar()
                    .selected(selected[i]),
            )
            .clicked()
        {
            selected[i] = !selected[i];
            owner["maskChannels"] = json!(selected);
        }
    }
    let inverted = owner["maskInvert"].as_bool().unwrap_or(false);
    if ui
        .add(
            crate::buttons::Button::new(crate::localization::text(ui.ctx(), "Invert mask"))
                .bar()
                .selected(inverted),
        )
        .clicked()
    {
        owner["maskInvert"] = json!(!inverted);
    }
}
