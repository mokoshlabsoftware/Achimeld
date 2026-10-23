//! The shared gradient library; all persistence and native pickers go through the host.
use crate::{
    Desktop,
    buttons::{self, Button, ButtonStyle},
};
use achimeld_application::{EditorPort, GradientPreset, UiRequest};
use egui::{Rect, Vec2};
use serde_json::json;
use std::collections::BTreeSet;
#[path = "gradient_explorer_tiles.rs"]
mod tiles;

const OPEN: &str = "open-gradient-explorer";

#[derive(Default)]
pub(crate) struct Explorer {
    pub open: bool,
    selected: Option<String>,
    selection: BTreeSet<String>,
    initialized: bool,
    draft: Option<GradientPreset>,
    filter: String,
    error: Option<String>,
    removed: Vec<GradientPreset>,
}

pub(crate) fn request(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(OPEN), true));
}

pub(crate) fn action(
    ui: &mut egui::Ui,
    label: &str,
    symbol: &str,
    width: f32,
    enabled: bool,
) -> egui::Response {
    let bytes: &[u8] = match symbol {
        "explosion" => include_bytes!("../assets/toolbar/layer-explosion.rgba"),
        "particle" => include_bytes!("../assets/toolbar/layer-particle.rgba"),
        "load" => include_bytes!("../assets/toolbar/style-set-load.rgba"),
        "save" => include_bytes!("../assets/toolbar/style-set-save.rgba"),
        "copy" => include_bytes!("../assets/toolbar/menu-copy.rgba"),
        "delete" => include_bytes!("../assets/toolbar/delete.rgba"),
        "undo" => include_bytes!("../assets/toolbar/undo.rgba"),
        _ => include_bytes!("../assets/toolbar/gradient-explorer.rgba"),
    };
    let id = egui::Id::new(("gradient-explorer-action", symbol));
    let texture = ui
        .ctx()
        .data_mut(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture = ui.ctx().load_texture(
                format!("gradient-{symbol}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized(
            [width, ui.spacing().interact_size.y],
            Button::new((egui::Image::new((texture.id(), Vec2::splat(20.0))), label))
                .id(ui.id().with(label))
                .bar()
                .icon_spacing(5.0)
                .image_tint_follows_text_color(true)
                .wrap_mode(egui::TextWrapMode::Truncate),
        )
    })
    .inner
    .on_hover_text(label)
}

impl Explorer {
    fn save(&mut self, port: &mut dyn EditorPort) -> bool {
        let Some(draft) = &self.draft else {
            return true;
        };
        if port.gradients().iter().any(|preset| preset == draft) {
            return true;
        }
        if let Err(error) = draft
            .validate()
            .and_then(|()| port.save_gradient(draft.clone()))
        {
            self.error = Some(error);
            false
        } else {
            self.error = None;
            true
        }
    }
    fn new_gradient(&mut self) {
        let id = achimeld_application::new_id();
        self.selected = Some(id.clone());
        self.selection = BTreeSet::from([id.clone()]);
        self.draft = Some(GradientPreset {
            id,
            name: "New gradient".into(),
            stops: vec![
                achimeld_application::GradientStop {
                    offset: 0.0,
                    color_hex: "#FF000000".into(),
                },
                achimeld_application::GradientStop {
                    offset: 1.0,
                    color_hex: "#FFFFFFFF".into(),
                },
            ],
        });
        self.error = None;
    }
    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 4.0;
            if action(ui, "New", "new", width, true).clicked() && self.save(port) {
                self.new_gradient();
            }
            if action(ui, "Import…", "load", width, true)
                .on_hover_text("Import Photoshop gradient presets (.grd)")
                .clicked()
                && self.save(port)
            {
                port.request(UiRequest::ImportGradients);
            }
            if action(
                ui,
                "Export selected…",
                "save",
                width,
                !self.selection.is_empty(),
            )
            .clicked()
                && self.save(port)
            {
                port.request(UiRequest::ExportGradients(
                    port.gradients()
                        .iter()
                        .filter(|g| self.selection.contains(&g.id))
                        .cloned()
                        .collect(),
                ));
            }
            if action(
                ui,
                "Export all…",
                "save",
                width,
                !port.gradients().is_empty(),
            )
            .clicked()
                && self.save(port)
            {
                port.request(UiRequest::ExportGradients(port.gradients().to_vec()));
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Find gradients")
                .desired_width(ui.available_width()),
        );
        self.tiles(ui, port);
        ui.weak(format!(
            "{} selected · Shift-click adds · Alt-click removes",
            self.selection.len()
        ));
        ui.separator();
        if let Some(draft) = &mut self.draft {
            crate::inspector::field(ui, "Name", ui.available_width() - 70.0, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .char_limit(256)
                        .desired_width(ui.available_width()),
                );
            });
            let mut value = json!({"stops":draft.stops});
            ui.push_id(&draft.id, |ui| crate::gradient::editor(ui, &mut value));
            if let Ok(stops) = serde_json::from_value(value["stops"].clone()) {
                draft.stops = stops;
            }
        }
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 4.0;
            if action(ui, "Save gradient", "save", width, self.draft.is_some()).clicked() {
                self.save(port);
            }
            if action(ui, "Duplicate", "copy", width, self.draft.is_some()).clicked()
                && self.save(port)
                && let Some(draft) = &mut self.draft
            {
                draft.id = achimeld_application::new_id();
                draft.name = format!("{} copy", draft.name.chars().take(60).collect::<String>());
                self.selected = Some(draft.id.clone());
                self.selection = BTreeSet::from([draft.id.clone()]);
            }
            if action(
                ui,
                "Delete selected",
                "delete",
                width,
                !self.selection.is_empty(),
            )
            .clicked()
            {
                self.delete_selection(port);
            }
            if action(ui, "Undo delete", "undo", width, !self.removed.is_empty()).clicked() {
                self.undo_delete(port);
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.weak("Choose saved gradients from Gradient presets in any gradient layer or effect.");
        ui.weak(port.status());
    }
}

#[cfg(test)]
#[path = "gradient_explorer_tests.rs"]
mod tests;

impl Desktop {
    pub(crate) fn gradient_explorer_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        if ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new(OPEN)))
            .unwrap_or(false)
        {
            self.gradient_explorer.open = true;
        }
        if !self.gradient_explorer.open {
            return;
        }
        let mut explorer = std::mem::take(&mut self.gradient_explorer);
        if !explorer.initialized {
            explorer.initialized = true;
            if let Some(preset) = port.gradients().first() {
                explorer.selected = Some(preset.id.clone());
                explorer.selection.insert(preset.id.clone());
                explorer.draft = Some(preset.clone());
            } else {
                explorer.new_gradient();
            }
        }
        let mut open = true;
        egui::Window::new("Gradient Explorer")
            .id(egui::Id::new("gradient-explorer-window"))
            .open(&mut open)
            .default_pos([24.0, 60.0])
            .default_size([640.0, 620.0])
            .min_width(480.0)
            .collapsible(false)
            .enabled(enabled)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("gradient-explorer-body")
                    .show(ui, |ui| explorer.contents(ui, port));
            });
        // Closing/switching/exporting saves valid drafts. Invalid names remain repairable.
        explorer.open = open || !explorer.save(port);
        self.gradient_explorer = explorer;
    }
}
