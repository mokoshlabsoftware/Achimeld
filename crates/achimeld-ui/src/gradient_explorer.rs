//! The shared gradient library; all persistence and native pickers go through the host.
use crate::{
    Desktop,
    buttons::{self, Button, ButtonStyle},
};
use achimeld_application::{EditorPort, GradientPreset, UiRequest};
use egui::{Rect, Vec2};
use serde_json::{Value, json};
use std::collections::BTreeSet;
#[path = "gradient_explorer_tiles.rs"]
mod tiles;

const OPEN: &str = "open-gradient-explorer";
const APPLY: &str = "apply-gradient-preset";
pub(crate) fn preset_icon(
    ctx: &egui::Context,
    preset: &GradientPreset,
    size: f32,
) -> egui::Image<'static> {
    let id = egui::Id::new(("gradient-preset-icon", &preset.id));
    if let Some((stops, texture)) = ctx.data(|data| {
        data.get_temp::<(Vec<achimeld_application::GradientStop>, egui::TextureHandle)>(id)
    }) && stops == preset.stops
    {
        return egui::Image::new((texture.id(), Vec2::splat(size)));
    }
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for x in 0..64 {
        let t = x as f32 / 63.0;
        let upper = preset
            .stops
            .iter()
            .position(|stop| stop.offset as f32 >= t)
            .unwrap_or(preset.stops.len().saturating_sub(1));
        let lower = upper.saturating_sub(1);
        let a = &preset.stops[lower];
        let b = &preset.stops[upper];
        let mix = if upper == lower {
            0.0
        } else {
            ((t - a.offset as f32) / (b.offset as f32 - a.offset as f32)).clamp(0.0, 1.0)
        };
        let parse = |hex: &str| -> u32 {
            u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xFF000000)
        };
        let ca = parse(&a.color_hex);
        let cb = parse(&b.color_hex);
        for y in 0..64 {
            let index = (y * 64 + x) * 4;
            for channel in 0..4 {
                let shift = (3 - channel) * 8;
                let av = ((ca >> shift) & 255) as f32;
                let bv = ((cb >> shift) & 255) as f32;
                rgba[index + (channel + 3) % 4] = (av * (1.0 - mix) + bv * mix) as u8;
            }
        }
    }
    let texture = ctx.load_texture(
        format!("gradient-icon-{}", preset.id),
        egui::ColorImage::from_rgba_unmultiplied([64, 64], &rgba),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(id, (preset.stops.clone(), texture.clone())));
    egui::Image::new((texture.id(), Vec2::splat(size)))
}

#[derive(Default)]
pub(crate) struct Explorer {
    pub open: bool,
    selected: Option<String>,
    selection: BTreeSet<String>,
    initialized: bool,
    draft: Option<GradientPreset>,
    unsaved: Option<GradientPreset>,
    target: Option<egui::Id>,
    icon_size: f64,
    filter: String,
    error: Option<String>,
    removed: Vec<GradientPreset>,
}

pub(crate) fn request(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(OPEN), true));
}
pub(crate) fn request_source(ctx: &egui::Context, gradient: Value, target: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("gradient-explorer-source"), gradient));
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("gradient-explorer-target"), target));
    request(ctx);
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
        if self.selected.is_none() {
            return true;
        }
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
    #[cfg(test)]
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
    fn set_unsaved(&mut self, source: Option<Value>) {
        let stops = source
            .and_then(|source| serde_json::from_value(source["stops"].clone()).ok())
            .unwrap_or_else(|| {
                vec![
                    achimeld_application::GradientStop {
                        offset: 0.0,
                        color_hex: "#FF000000".into(),
                    },
                    achimeld_application::GradientStop {
                        offset: 1.0,
                        color_hex: "#FFFFFFFF".into(),
                    },
                ]
            });
        let preset = GradientPreset {
            id: achimeld_application::new_id(),
            name: "New gradient".into(),
            stops,
        };
        self.unsaved = Some(preset.clone());
        self.selected = None;
        self.selection.clear();
        self.draft = Some(preset);
        self.error = None;
    }
    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 4.0;
            if action(ui, "New", "new", width, true).clicked() && self.save(port) {
                self.set_unsaved(None);
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
        let mut size = json!({"size":if self.icon_size < 24.0 { 80.0 } else { self.icon_size }});
        crate::knobs::bank(
            ui,
            port.preferences(),
            &mut size,
            &json!({"size":80.0}),
            &[crate::knobs::Parameter::new("Icon size", "size", 24.0..=160.0, 0).integer()],
        );
        self.icon_size = size["size"].as_f64().unwrap_or(80.0).clamp(24.0, 160.0);
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
            let width = ui.available_width() / 5.0;
            if action(ui, "Save gradient", "save", width, self.draft.is_some()).clicked() {
                if self.selected.is_none() {
                    if let Some(draft) = &self.draft {
                        let mut preset = draft.clone();
                        preset.id = achimeld_application::new_id();
                        match preset
                            .validate()
                            .and_then(|()| port.save_gradient(preset.clone()))
                        {
                            Ok(()) => {
                                self.selected = Some(preset.id.clone());
                                self.selection.insert(preset.id.clone());
                                self.draft = Some(preset);
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                } else {
                    self.save(port);
                }
            }
            if action(
                ui,
                "Apply to layer",
                "copy",
                width,
                self.target.is_some() && self.draft.is_some(),
            )
            .clicked()
                && self.save(port)
                && let (Some(target), Some(draft)) = (self.target, &self.draft)
            {
                match draft.validate() {
                    Ok(()) => ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new(APPLY), (target, draft.stops.clone()));
                    }),
                    Err(error) => self.error = Some(error),
                }
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
        if self.selected.is_none() {
            self.unsaved = self.draft.clone();
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
            let source =
                ctx.data_mut(|d| d.remove_temp::<Value>(egui::Id::new("gradient-explorer-source")));
            self.gradient_explorer.target = ctx.data_mut(|d| {
                let id = egui::Id::new("gradient-explorer-target");
                let target = d.get_temp::<egui::Id>(id);
                d.remove::<egui::Id>(id);
                target
            });
            if source.is_some() {
                self.gradient_explorer.set_unsaved(source);
            }
        }
        if !self.gradient_explorer.open {
            return;
        }
        let mut explorer = std::mem::take(&mut self.gradient_explorer);
        if !explorer.initialized {
            explorer.initialized = true;
            if explorer.draft.is_none() {
                explorer.set_unsaved(None);
            }
        }
        let mut open = true;
        egui::Window::new("Gradient Explorer")
            .id(egui::Id::new("gradient-explorer-window"))
            .frame(egui::Frame::window(&ctx.global_style()).corner_radius(egui::CornerRadius::ZERO))
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
