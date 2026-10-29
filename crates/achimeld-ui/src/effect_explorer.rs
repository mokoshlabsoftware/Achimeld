//! Named effect libraries share complete, embedded settings; I/O stays in the host.
use crate::{
    Desktop,
    buttons::{self, Button, UiButtonExt as _},
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
#[derive(Default)]
pub(crate) struct Explorer {
    pub open: bool,
    kind: Option<EffectKind>,
    selected: Option<String>,
    draft: Option<EffectPreset>,
    removed: Option<EffectPreset>,
    filter: String,
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
    preset_menu(ui, port, layer, kind, Some(width));
}
fn preset_menu(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    kind: EffectKind,
    width: Option<f32>,
) {
    let mut apply = None;
    let mut error = None;
    let title = if ui.available_width() < 210.0 {
        "Presets".to_owned()
    } else {
        format!("{} presets", kind.layer_kind())
    };
    let label = format!("{} presets", kind.layer_kind());
    let button = if let Some(width) = width {
        let image = crate::particles::glyph(
            ui.ctx(),
            "effect-presets",
            include_bytes!("../assets/toolbar/layer-folder.rgba"),
        );
        Button::new(image)
            .id(egui::Id::new((
                "effect-presets",
                layer["id"].as_str().unwrap_or(""),
            )))
            .bar()
            .min_size(egui::vec2(width, ui.spacing().interact_size.y))
            .image_tint_follows_text_color(true)
    } else {
        Button::new(title).style(buttons::ButtonStyle::Quiet)
    };
    let response = buttons::menu(ui, button, |ui| {
        if ui.app_button(format!("{}…", kind.title())).clicked() {
            request(ui.ctx(), kind, Some(layer.clone()));
            ui.close();
        }
        if ui.app_button("Save current as preset").clicked() {
            let name = layer["displayName"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(kind.layer_kind())
                .to_owned();
            error = EffectPreset::from_layer(kind, layer, name)
                .and_then(|p| port.save_effect_preset(p))
                .err();
            ui.close();
        }
        for preset in port.effect_presets().iter().filter(|p| p.kind == kind) {
            if ui.app_button(&preset.name).clicked() {
                apply = Some(preset.clone());
                ui.close();
            }
        }
    })
    .response;
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
    });
    response.on_hover_text(label);
    if let Some(preset) = apply {
        port.finish_group();
        error = preset.apply_to(layer).err();
    }
    if let Some(error) = error {
        ui.colored_label(ui.visuals().error_fg_color, error);
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
                .cloned();
            self.error = None;
            self.texture = None;
        }
    }
    pub(crate) fn save(&mut self, port: &mut dyn EditorPort) -> bool {
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
                self.selected = Some(preset.id.clone());
                self.draft = Some(preset);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let kind = self.kind.unwrap_or(EffectKind::Explosion);
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 4.0;
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
        let needle = self.filter.to_lowercase();
        let presets: Vec<_> = port
            .effect_presets()
            .iter()
            .filter(|p| p.kind == kind && p.name.to_lowercase().contains(&needle))
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        let mut choose = None;
        egui::ScrollArea::vertical()
            .id_salt("effect-library")
            .max_height(150.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.weak(format!(
                    "{} {}",
                    presets.len(),
                    if presets.len() == 1 {
                        "preset"
                    } else {
                        "presets"
                    }
                ));
                ui.spacing_mut().item_spacing.y = 0.0;
                for (id, name) in presets {
                    if ui
                        .add_sized(
                            [ui.available_width(), buttons::bar_height(ui, 24.0)],
                            Button::selectable(self.selected.as_deref() == Some(&id), &name).bar(),
                        )
                        .clicked()
                    {
                        choose = Some(id);
                    }
                }
            });
        if let Some(id) = choose
            && self.save(port)
            && let Some(preset) = port.effect_presets().iter().find(|p| p.id == id).cloned()
        {
            self.selected = Some(preset.id.clone());
            self.draft = Some(preset);
        }
        buttons::bar(ui, 24.0, |ui| {
            let width = ui.available_width() / 4.0;
            if crate::gradient_explorer::action(
                ui,
                "Save preset",
                "save",
                width,
                self.draft.is_some(),
            )
            .clicked()
            {
                self.save(port);
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
                        self.draft = None;
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
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                self.texture = Some((preview.fingerprint, texture));
            }
            if let Some((_, texture)) = &self.texture {
                let edge = ui.available_width().min(220.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(edge), egui::Sense::hover());
                crate::gradient::checker(ui, rect);
                egui::Image::new((texture.id(), rect.size())).paint_at(ui, rect);
            }
            if let Some(error) = port.effect_preview_error() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
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
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.weak(port.status());
        ui.weak("Choose saved presets from the matching layer's presets menu.");
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
                explorer.create(request.kind, Some(source));
            }
        }
        if explorer.open {
            let kind = explorer.kind.unwrap_or(EffectKind::Explosion);
            if explorer.draft.is_none() {
                if let Some(preset) = port.effect_presets().iter().find(|p| p.kind == kind) {
                    explorer.selected = Some(preset.id.clone());
                    explorer.draft = Some(preset.clone());
                } else {
                    explorer.create(kind, None);
                }
            }
            let mut open = true;
            egui::Window::new(kind.title())
                .id(egui::Id::new("effect-explorer-window"))
                .open(&mut open)
                .default_pos([34.0, 60.0])
                .default_size([660.0, 700.0])
                .min_width(360.0)
                .collapsible(false)
                .enabled(enabled)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("effect-explorer-body")
                        .show(ui, |ui| explorer.contents(ui, port));
                });
            explorer.open = open || !explorer.save(port);
        }
        self.effect_explorer = explorer;
    }
}

#[cfg(test)]
#[path = "effect_explorer_tests.rs"]
mod tests;
