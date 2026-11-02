//! Named effect libraries share complete, embedded settings; I/O stays in the host.
use crate::{
    Desktop,
    buttons::{self, Button},
};
use achimeld_application::{
    EditorPort, UiRequest,
    effect_presets::{EffectKind, EffectPreset},
};
use serde_json::Value;

const OPEN: &str = "open-effect-explorer";
#[derive(Clone, Default)]
struct Request {
    kind: EffectKind,
    source: Option<Value>,
}
const APPLY: &str = "apply-effect-preset";
fn preset_icon(ctx: &egui::Context, preset: &EffectPreset, size: f32) -> egui::Image<'static> {
    let id = egui::Id::new(("effect-preset-icon", &preset.id));
    if let Some((layer, texture)) =
        ctx.data(|data| data.get_temp::<(Value, egui::TextureHandle)>(id))
        && layer == preset.layer
    {
        return egui::Image::new((texture.id(), egui::Vec2::splat(size)));
    }
    let stops = preset.layer[preset.kind.settings_key()]["gradient"]["stops"].as_array();
    let colors: Vec<_> = stops
        .into_iter()
        .flatten()
        .filter_map(|stop| stop["colorHex"].as_str())
        .filter_map(|hex| {
            let hex = hex.trim_start_matches('#');
            let rgb = hex.get(hex.len().checked_sub(6)?..)?;
            u32::from_str_radix(rgb, 16).ok()
        })
        .collect();
    let low = colors.first().copied().unwrap_or(0x334455);
    let high = colors.last().copied().unwrap_or(0xFFFFFF);
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let dx = (x as f32 - 31.5) / 31.5;
            let dy = (y as f32 - 31.5) / 31.5;
            let radius = (dx * dx + dy * dy).sqrt();
            let limit = if preset.kind == EffectKind::Explosion {
                0.64 + 0.28 * (dy.atan2(dx) * 9.0).cos().abs()
            } else {
                0.78
            };
            if radius > limit {
                continue;
            }
            let t = (1.0 - radius / limit).clamp(0.0, 1.0);
            let index = (y * 64 + x) * 4;
            for channel in 0..3 {
                let shift = 16 - channel * 8;
                let a = ((low >> shift) & 255) as f32;
                let b = ((high >> shift) & 255) as f32;
                rgba[index + channel] = (a * (1.0 - t) + b * t) as u8;
            }
            rgba[index + 3] = ((limit - radius) * 255.0 * 8.0).min(255.0) as u8;
        }
    }
    let texture = ctx.load_texture(
        format!("effect-icon-{}", preset.id),
        egui::ColorImage::from_rgba_unmultiplied([64, 64], &rgba),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    ctx.data_mut(|data| data.insert_temp(id, (preset.layer.clone(), texture.clone())));
    egui::Image::new((texture.id(), egui::Vec2::splat(size)))
}
fn effect_tile(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    preset: &EffectPreset,
    selected: bool,
    edge: f32,
) -> egui::Response {
    let tile = egui::vec2(edge + 16.0, edge + 34.0);
    let response = ui.add_sized(
        [tile.x, tile.y],
        Button::selectable(selected, "")
            .id(ui.make_persistent_id(("effect-tile", &preset.id)))
            .bar(),
    );
    let image_rect = egui::Rect::from_min_size(
        response.rect.min + egui::vec2(8.0, 4.0),
        egui::Vec2::splat(edge),
    );
    crate::gradient::checker(ui, image_rect);
    if let Some(preview) = port.effect_thumbnail(preset) {
        let id = egui::Id::new(("effect-tile-texture", &preset.id));
        let texture = ui
            .ctx()
            .data(|data| data.get_temp::<(String, egui::TextureHandle)>(id));
        let texture = if let Some((fingerprint, texture)) = texture
            && fingerprint == preview.fingerprint
        {
            texture
        } else {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [preview.width, preview.height],
                &preview.rgba,
            );
            let texture = ui.ctx().load_texture(
                format!("effect-tile-{}", preset.id),
                image,
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, (preview.fingerprint, texture.clone())));
            texture
        };
        egui::Image::new((texture.id(), image_rect.size())).paint_at(ui, image_rect);
    }
    let style = if selected {
        buttons::ButtonStyle::Accent
    } else {
        buttons::ButtonStyle::Quiet
    };
    let (_, ink) = buttons::colors(ui, &response, style);
    let name = if preset.name.chars().count() > 15 {
        format!("{}...", preset.name.chars().take(12).collect::<String>())
    } else {
        preset.name.clone()
    };
    ui.painter()
        .with_clip_rect(response.rect.shrink2(egui::vec2(4.0, 0.0)))
        .text(
            egui::pos2(response.rect.center().x, image_rect.bottom() + 13.0),
            egui::Align2::CENTER_CENTER,
            name,
            egui::TextStyle::Button.resolve(ui.style()),
            ink,
        );
    response.on_hover_text(&preset.name)
}
#[derive(Default)]
pub(crate) struct Explorer {
    pub open: bool,
    kind: Option<EffectKind>,
    selected: Option<String>,
    draft: Option<EffectPreset>,
    unsaved: Option<EffectPreset>,
    target_layer: Option<String>,
    removed: Option<EffectPreset>,
    filter: String,
    icon_size: f64,
    error: Option<String>,
    texture: Option<(String, egui::TextureHandle)>,
}
pub(crate) fn request(ctx: &egui::Context, kind: EffectKind, source: Option<Value>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(OPEN), Request { kind, source }));
}
pub(crate) fn preset_icon_controls(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    kind: EffectKind,
    width: f32,
) {
    let pending = ui.ctx().data_mut(|d| {
        let id = egui::Id::new(APPLY);
        let pending = d.get_temp::<(String, EffectPreset)>(id);
        d.remove::<(String, EffectPreset)>(id);
        pending
    });
    if let Some((id, preset)) = pending {
        if layer["id"].as_str() == Some(&id) {
            port.finish_group();
            if let Err(error) = preset.apply_to(layer) {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        } else {
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new(APPLY), (id, preset)));
        }
    }
    let label = format!("Open {}", kind.title());
    let image = crate::particles::glyph(
        ui.ctx(),
        "effect-presets",
        include_bytes!("../assets/toolbar/layer-folder.rgba"),
    );
    let response = ui.add_sized(
        [width, ui.spacing().interact_size.y],
        Button::new(image)
            .id(ui.make_persistent_id(("effect-presets", layer["id"].as_str().unwrap_or(""))))
            .bar()
            .image_tint_follows_text_color(true),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
    });
    if response.on_hover_text(label).clicked() {
        request(ui.ctx(), kind, Some(layer.clone()));
    }
}
pub(crate) fn quick_load_controls(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    kind: EffectKind,
    width: f32,
) {
    let image = crate::particles::glyph(
        ui.ctx(),
        "effect-quick-load",
        include_bytes!("../assets/toolbar/style-set-load.rgba"),
    );
    let response = ui.add_sized(
        [width, ui.spacing().interact_size.y],
        Button::new(image)
            .id(ui.make_persistent_id(("effect-quick-load", layer["id"].as_str().unwrap_or(""))))
            .bar()
            .image_tint_follows_text_color(true),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            response.enabled(),
            "Quick load preset",
        )
    });
    let response = response.on_hover_text("Quick load preset");
    let mut chosen = None;
    buttons::menu_popup(ui, &response, |ui| {
        for preset in port
            .effect_presets()
            .iter()
            .filter(|preset| preset.kind == kind)
        {
            let icon = preset_icon(ui.ctx(), preset, 28.0);
            if ui
                .add(
                    Button::new((icon, preset.name.as_str()))
                        .bar()
                        .min_size(egui::vec2(ui.available_width(), 32.0))
                        .image_tint_follows_text_color(true),
                )
                .clicked()
            {
                chosen = Some(preset.clone());
                ui.close();
            }
        }
        if port
            .effect_presets()
            .iter()
            .all(|preset| preset.kind != kind)
        {
            ui.weak("No saved presets");
        }
    });
    if let Some(preset) = chosen {
        port.finish_group();
        if let Err(error) = preset.apply_to(layer) {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}
impl Explorer {
    pub(crate) fn needs_recovery(&self) -> bool {
        self.error.is_some()
    }
    pub(crate) fn revert_failed_edit(&mut self, port: &dyn EditorPort) {
        if self.needs_recovery() {
            self.draft = self
                .selected
                .as_ref()
                .and_then(|id| port.effect_presets().iter().find(|preset| &preset.id == id))
                .cloned()
                .or_else(|| self.unsaved.clone());
            self.error = None;
            self.texture = None;
        }
    }
    pub(crate) fn save(&mut self, port: &mut dyn EditorPort) -> bool {
        if self.selected.is_none() {
            return true;
        }
        let Some(draft) = &self.draft else {
            return true;
        };
        if port.effect_presets().contains(draft) {
            return true;
        }
        match draft
            .validate()
            .and_then(|()| port.save_effect_preset(draft.clone()))
        {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }
    fn create(&mut self, kind: EffectKind, source: Option<Value>) {
        let layer = source.unwrap_or_else(|| crate::layer_palette::new_layer(kind.layer_kind()));
        let name = format!("New {}", kind.layer_kind().to_lowercase());
        match EffectPreset::from_layer(kind, &layer, name) {
            Ok(preset) => {
                self.selected = None;
                self.unsaved = Some(preset.clone());
                self.draft = Some(preset);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        // Keep the preview visible while the settings pane scrolls. The narrow
        // layout is retained for compact test surfaces and very small windows.
        if ui.available_width() < 700.0 {
            self.controls(ui, port);
            self.preview(ui, port, ui.available_width().min(220.0));
            return;
        }
        let preview_width = (ui.available_width() * 0.36).clamp(240.0, 340.0);
        let height = ui.available_height();
        let controls_width =
            (ui.available_width() - preview_width - ui.spacing().item_spacing.x - 20.0).max(1.0);
        ui.horizontal_top(|ui| {
            let (preview_rect, _) =
                ui.allocate_exact_size(egui::vec2(preview_width, height), egui::Sense::hover());
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(controls_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("effect-explorer-controls")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.controls(ui, port));
                },
            );
            // Draw after the controls so a changed value appears in this frame.
            self.preview_at(ui, port, preview_rect);
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let kind = self.kind.unwrap_or(EffectKind::Explosion);
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 2.0;
            if crate::gradient_explorer::action(
                ui,
                "New",
                kind.settings_key().trim_end_matches('s'),
                width,
                true,
            )
            .clicked()
                && self.save(port)
            {
                self.create(kind, None);
            }
            if crate::gradient_explorer::action(ui, "Import…", "load", width, true).clicked()
                && self.save(port)
            {
                port.request(UiRequest::ImportEffects(kind));
            }
        });
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 2.0;
            if crate::gradient_explorer::action(
                ui,
                "Export selected…",
                "save",
                width,
                self.draft.is_some(),
            )
            .clicked()
                && self.save(port)
            {
                port.request(UiRequest::ExportEffects {
                    kind,
                    presets: self.draft.iter().cloned().collect(),
                });
            }
            if crate::gradient_explorer::action(
                ui,
                "Export collection…",
                "save",
                width,
                port.effect_presets().iter().any(|p| p.kind == kind),
            )
            .clicked()
                && self.save(port)
            {
                port.request(UiRequest::ExportEffects {
                    kind,
                    presets: port
                        .effect_presets()
                        .iter()
                        .filter(|p| p.kind == kind)
                        .cloned()
                        .collect(),
                });
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Find presets")
                .desired_width(ui.available_width()),
        );
        let mut size =
            serde_json::json!({"size": if self.icon_size < 24.0 { 96.0 } else { self.icon_size }});
        crate::knobs::bank(
            ui,
            port.preferences(),
            &mut size,
            &serde_json::json!({"size":96.0}),
            &[crate::knobs::Parameter::new("Icon size", "size", 24.0..=96.0, 0).integer()],
        );
        self.icon_size = size["size"].as_f64().unwrap_or(96.0).clamp(24.0, 96.0);
        let needle = self.filter.to_lowercase();
        let presets: Vec<_> = port
            .effect_presets()
            .iter()
            .filter(|p| p.kind == kind && p.name.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        let preset_count = presets.len();
        let mut choose = None;
        let edge = self.icon_size as f32;
        let columns = ((ui.available_width() - 16.0) / (edge + 16.0))
            .floor()
            .max(1.0) as usize;
        let mut tiles = Vec::with_capacity(presets.len() + 1);
        if let Some(unsaved) = &self.unsaved {
            let mut unsaved = unsaved.clone();
            unsaved.name = "Unsaved changes".into();
            tiles.push((unsaved, true));
        }
        tiles.extend(presets.into_iter().map(|preset| (preset, false)));
        egui::ScrollArea::vertical()
            .id_salt("effect-library")
            .max_height((edge + 34.0) * 2.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.weak(format!(
                    "{} {}",
                    preset_count,
                    if preset_count == 1 {
                        "preset"
                    } else {
                        "presets"
                    }
                ));
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                for row in tiles.chunks(columns) {
                    ui.horizontal(|ui| {
                        for (preset, unsaved) in row {
                            let selected = if *unsaved {
                                self.selected.is_none()
                            } else {
                                self.selected.as_deref() == Some(&preset.id)
                            };
                            if effect_tile(ui, port, preset, selected, edge).clicked() {
                                choose = Some(if *unsaved {
                                    String::new()
                                } else {
                                    preset.id.clone()
                                });
                            }
                        }
                    });
                }
            });
        if let Some(id) = choose
            && self.save(port)
        {
            if id.is_empty() {
                self.selected = None;
                self.draft = self.unsaved.clone();
            } else if let Some(preset) = port.effect_presets().iter().find(|p| p.id == id).cloned()
            {
                self.selected = Some(preset.id.clone());
                self.draft = Some(preset);
            }
        }
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 5.0;
            if crate::gradient_explorer::action(
                ui,
                "Save preset",
                "save",
                width,
                self.draft.is_some(),
            )
            .clicked()
            {
                if self.selected.is_none() {
                    if let Some(draft) = &self.draft {
                        let mut preset = draft.clone();
                        preset.id = achimeld_application::new_id();
                        match preset
                            .validate()
                            .and_then(|()| port.save_effect_preset(preset.clone()))
                        {
                            Ok(()) => {
                                self.selected = Some(preset.id.clone());
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
            if crate::gradient_explorer::action(
                ui,
                "Apply to layer",
                "copy",
                width,
                self.target_layer.is_some() && self.draft.is_some(),
            )
            .clicked()
                && self.save(port)
                && let (Some(id), Some(preset)) = (&self.target_layer, &self.draft)
            {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new(APPLY), (id.clone(), preset.clone()))
                });
            }
            if crate::gradient_explorer::action(
                ui,
                "Duplicate",
                "copy",
                width,
                self.draft.is_some(),
            )
            .clicked()
                && self.save(port)
                && let Some(draft) = &mut self.draft
            {
                draft.id = achimeld_application::new_id();
                draft.name = format!("{} copy", draft.name.chars().take(60).collect::<String>());
                self.selected = Some(draft.id.clone());
            }
            if crate::gradient_explorer::action(
                ui,
                "Remove",
                "delete",
                width,
                self.selected.is_some(),
            )
            .clicked()
                && let Some(id) = &self.selected
            {
                let original = port
                    .effect_presets()
                    .iter()
                    .find(|p| p.id == *id)
                    .cloned()
                    .or_else(|| self.draft.clone().filter(|p| p.validate().is_ok()));
                match port.delete_effect_preset(id) {
                    Ok(()) => {
                        self.removed = original;
                        self.draft = self.unsaved.clone();
                        self.selected = None;
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            if crate::gradient_explorer::action(
                ui,
                "Undo remove",
                "undo",
                width,
                self.removed.is_some(),
            )
            .clicked()
                && let Some(preset) = self.removed.clone()
            {
                match port.save_effect_preset(preset.clone()) {
                    Ok(()) => {
                        self.selected = Some(preset.id.clone());
                        self.draft = Some(preset);
                        self.removed = None;
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        });
        if let Some(draft) = &mut self.draft {
            crate::inspector::field(ui, "Name", 160.0, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .char_limit(128)
                        .desired_width(ui.available_width()),
                );
            });
            ui.weak("Gradient and color map data are stored with this preset.");
            crate::inspector::boolean(ui, "Visible", &mut draft.layer, "visible");
            crate::inspector::number(ui, "Opacity", &mut draft.layer, "opacity", 0.0..=1.0, 0.01);
            crate::layers::blend_mode(ui, &mut draft.layer);
            crate::layers::transform_ui(ui, &mut draft.layer);
            ui.push_id(&draft.id, |ui| match kind {
                EffectKind::Explosion => crate::explosion::settings_properties(
                    ui,
                    port,
                    &mut draft.layer,
                    "effect-explorer",
                ),
                EffectKind::Particle => {
                    Desktop::default().particle_editor(ui, port, &mut draft.layer, true)
                }
            });
            crate::layers::effects_ui(ui, &mut draft.layer);
        }
        if self.selected.is_none() {
            self.unsaved = self.draft.clone();
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.weak(port.status());
        ui.weak("Choose saved presets from the matching layer's presets menu.");
    }

    fn update_preview(&mut self, ui: &egui::Ui, port: &mut dyn EditorPort) {
        let Some(draft) = &self.draft else {
            self.texture = None;
            return;
        };
        if let Some(preview) = port.effect_preview(draft)
            && self
                .texture
                .as_ref()
                .is_none_or(|(fingerprint, _)| fingerprint != &preview.fingerprint)
        {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [preview.width, preview.height],
                &preview.rgba,
            );
            let texture = ui.ctx().load_texture(
                "effect-explorer-preview",
                image,
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            self.texture = Some((preview.fingerprint, texture));
        }
    }

    fn preview_at(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort, area: egui::Rect) {
        self.update_preview(ui, port);
        if let Some((_, texture)) = &self.texture {
            let edge = area.width().min(area.height()).min(340.0);
            let rect = egui::Rect::from_center_size(area.center(), egui::Vec2::splat(edge));
            crate::gradient::checker(ui, rect);
            egui::Image::new((texture.id(), rect.size())).paint_at(ui, rect);
        }
        if let Some(error) = port.effect_preview_error() {
            ui.painter().text(
                area.left_bottom() - egui::vec2(0.0, 8.0),
                egui::Align2::LEFT_BOTTOM,
                error,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().error_fg_color,
            );
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort, edge: f32) {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(edge), egui::Sense::hover());
        self.preview_at(ui, port, rect);
    }
}
impl Desktop {
    pub(crate) fn effect_explorer_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        let mut explorer = std::mem::take(&mut self.effect_explorer);
        if let Some(request) = ctx.data_mut(|d| {
            let id = egui::Id::new(OPEN);
            let request = d.get_temp::<Request>(id);
            d.remove::<Request>(id);
            request
        }) && explorer.save(port)
        {
            if explorer.kind != Some(request.kind) || request.source.is_some() {
                explorer.draft = None;
                explorer.filter.clear();
                explorer.texture = None;
            }
            explorer.kind = Some(request.kind);
            explorer.open = true;
            if let Some(source) = request.source {
                explorer.target_layer = source["id"].as_str().map(str::to_owned);
                explorer.create(request.kind, Some(source));
            } else {
                explorer.target_layer = None;
            }
        }
        if explorer.open {
            let kind = explorer.kind.unwrap_or(EffectKind::Explosion);
            if explorer.draft.is_none() {
                explorer.create(kind, None);
            }
            let mut open = true;
            egui::Window::new(kind.title())
                .id(egui::Id::new("effect-explorer-window"))
                .frame(
                    egui::Frame::window(&ctx.global_style())
                        .corner_radius(egui::CornerRadius::ZERO),
                )
                .open(&mut open)
                .default_pos([34.0, 60.0])
                .default_size([960.0, 700.0])
                .min_width(700.0)
                .collapsible(false)
                .enabled(enabled)
                .show(ctx, |ui| explorer.contents(ui, port));
            explorer.open = open || !explorer.save(port);
        }
        self.effect_explorer = explorer;
    }
}

#[cfg(test)]
#[path = "effect_explorer_tests.rs"]
mod tests;
