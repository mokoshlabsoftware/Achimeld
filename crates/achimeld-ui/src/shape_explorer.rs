//! Saved shape recipes. The host owns persistence; applying a recipe stays in the layer edit.
use crate::buttons::{self, Button};
use achimeld_application::{
    EditorPort,
    shape_presets::{ShapeKind, ShapePreset},
};
use serde_json::Value;

const OPEN: &str = "shape-explorer-open";
const APPLY: &str = "shape-explorer-apply";
#[derive(Clone)]
struct Request {
    kind: ShapeKind,
    source: Value,
    target: egui::Id,
}
#[derive(Default)]
pub(crate) struct Explorer {
    pub open: bool,
    kind: Option<ShapeKind>,
    target: Option<egui::Id>,
    unsaved: Option<ShapePreset>,
    draft: Option<ShapePreset>,
    selected: Option<String>,
    removed: Option<ShapePreset>,
    filter: String,
    icon_size: f64,
    error: Option<String>,
}
pub(crate) fn controls(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    kind: ShapeKind,
) {
    let target = ui.id().with("shape-preset-controls");
    let pending = ui.ctx().data_mut(|data| {
        let id = egui::Id::new(APPLY);
        let pending = data.get_temp::<(egui::Id, ShapePreset)>(id);
        data.remove::<(egui::Id, ShapePreset)>(id);
        pending
    });
    if let Some((id, preset)) = pending {
        if id == target {
            port.finish_group();
            if let Err(error) = preset.apply_to(layer) {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        } else {
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(APPLY), (id, preset)));
        }
    }
    buttons::bar(ui, 24.0, |ui| {
        let width = ui.available_width() / 2.0;
        let folder = crate::particles::glyph(
            ui.ctx(),
            "shape-explorer-folder",
            include_bytes!("../assets/toolbar/layer-folder.rgba"),
        );
        if ui
            .add_sized(
                [width, ui.spacing().interact_size.y],
                Button::new(folder)
                    .bar()
                    .image_tint_follows_text_color(true),
            )
            .on_hover_text("Open Shape Explorer")
            .clicked()
        {
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    egui::Id::new(OPEN),
                    Request {
                        kind,
                        source: layer.clone(),
                        target,
                    },
                )
            });
        }
        let load = crate::particles::glyph(
            ui.ctx(),
            "shape-explorer-load",
            include_bytes!("../assets/toolbar/style-set-load.rgba"),
        );
        let response = ui
            .add_sized(
                [width, ui.spacing().interact_size.y],
                Button::new(load).bar().image_tint_follows_text_color(true),
            )
            .on_hover_text("Quick load shape");
        let mut chosen = None;
        buttons::menu_popup(ui, &response, |ui| {
            for preset in port
                .shape_presets()
                .iter()
                .filter(|preset| preset.kind == kind)
            {
                let icon = shape_icon(ui.ctx(), preset, 28.0);
                if ui
                    .add(
                        Button::new((icon, preset.name.as_str()))
                            .bar()
                            .min_size(egui::vec2(ui.available_width(), 32.0)),
                    )
                    .clicked()
                {
                    chosen = Some(preset.clone());
                    ui.close();
                }
            }
            if port
                .shape_presets()
                .iter()
                .all(|preset| preset.kind != kind)
            {
                ui.weak("No saved shapes");
            }
        });
        if let Some(preset) = chosen {
            port.finish_group();
            if let Err(error) = preset.apply_to(layer) {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        }
    });
}
fn shape_icon(ctx: &egui::Context, preset: &ShapePreset, size: f32) -> egui::Image<'static> {
    let shape = match preset.kind {
        ShapeKind::Layer => &preset.shape,
        ShapeKind::Particle => &preset.shape[0],
    };
    let name = match shape["kind"].as_str().unwrap_or("Sphere") {
        "Rectangle" => "Square",
        "Polygon" => match shape["vertexCount"].as_u64().unwrap_or(6) {
            3 => "Triangle",
            4 => "Square",
            6 => "Hexagon",
            _ => "Polygon",
        },
        "Star" => {
            if shape["vertexCount"].as_u64() == Some(4) {
                "Sparkle"
            } else {
                "Star"
            }
        }
        _ => "Sphere",
    };
    crate::particles::shape_icon(ctx, name).fit_to_exact_size(egui::Vec2::splat(size))
}
impl Explorer {
    fn save(&mut self, port: &mut dyn EditorPort) -> bool {
        if self.selected.is_none() {
            return true;
        }
        let Some(draft) = &self.draft else {
            return true;
        };
        if port.shape_presets().contains(draft) {
            return true;
        }
        match draft
            .validate()
            .and_then(|()| port.save_shape_preset(draft.clone()))
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
    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let kind = self.kind.unwrap_or(ShapeKind::Layer);
        let mut size =
            serde_json::json!({"size":if self.icon_size < 24.0 { 48.0 } else { self.icon_size }});
        crate::knobs::bank(
            ui,
            port.preferences(),
            &mut size,
            &serde_json::json!({"size":48.0}),
            &[crate::knobs::Parameter::new("Icon size", "size", 24.0..=96.0, 0).integer()],
        );
        self.icon_size = size["size"].as_f64().unwrap_or(48.0).clamp(24.0, 96.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Find shapes")
                .desired_width(ui.available_width()),
        );
        let presets: Vec<_> = port
            .shape_presets()
            .iter()
            .filter(|preset| {
                preset.kind == kind
                    && preset
                        .name
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .cloned()
            .collect();
        let mut choose = None;
        egui::ScrollArea::vertical()
            .id_salt("shape-presets")
            .max_height(280.0)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if let Some(preset) = &self.unsaved
                    && ui
                        .add_sized(
                            [ui.available_width(), self.icon_size as f32 + 8.0],
                            Button::selectable(
                                self.selected.is_none(),
                                (
                                    shape_icon(ui.ctx(), preset, self.icon_size as f32),
                                    "Unsaved changes",
                                ),
                            )
                            .bar(),
                        )
                        .clicked()
                {
                    choose = Some(String::new());
                }
                for preset in presets {
                    if ui
                        .add_sized(
                            [ui.available_width(), self.icon_size as f32 + 8.0],
                            Button::selectable(
                                self.selected.as_deref() == Some(&preset.id),
                                (
                                    shape_icon(ui.ctx(), &preset, self.icon_size as f32),
                                    preset.name.as_str(),
                                ),
                            )
                            .bar(),
                        )
                        .clicked()
                    {
                        choose = Some(preset.id);
                    }
                }
            });
        if let Some(id) = choose
            && self.save(port)
        {
            if id.is_empty() {
                self.selected = None;
                self.draft = self.unsaved.clone();
            } else if let Some(preset) = port.shape_presets().iter().find(|preset| preset.id == id)
            {
                self.selected = Some(id);
                self.draft = Some(preset.clone());
            }
        }
        if let Some(draft) = &mut self.draft {
            crate::inspector::field(ui, "Name", 100.0, |ui| {
                ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(128));
            });
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
                        match port.save_shape_preset(preset.clone()) {
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
                self.target.is_some() && self.draft.is_some(),
            )
            .clicked()
                && self.save(port)
                && let (Some(target), Some(preset)) = (self.target, &self.draft)
            {
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new(APPLY), (target, preset.clone()))
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
                    .shape_presets()
                    .iter()
                    .find(|preset| preset.id == *id)
                    .cloned();
                match port.delete_shape_preset(id) {
                    Ok(()) => {
                        self.removed = original;
                        self.selected = None;
                        self.draft = self.unsaved.clone();
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
                match port.save_shape_preset(preset.clone()) {
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
        if self.selected.is_none() {
            self.unsaved = self.draft.clone();
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}
impl crate::Desktop {
    pub(crate) fn shape_explorer_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        let mut explorer = std::mem::take(&mut self.shape_explorer);
        if let Some(request) = ctx.data_mut(|data| {
            let id = egui::Id::new(OPEN);
            let request = data.get_temp::<Request>(id);
            data.remove::<Request>(id);
            request
        }) && explorer.save(port)
        {
            let name = "New shape".to_owned();
            match ShapePreset::from_layer(request.kind, &request.source, name) {
                Ok(preset) => {
                    explorer.kind = Some(request.kind);
                    explorer.target = Some(request.target);
                    explorer.unsaved = Some(preset.clone());
                    explorer.draft = Some(preset);
                    explorer.selected = None;
                    explorer.open = true;
                    explorer.error = None;
                }
                Err(error) => explorer.error = Some(error),
            }
        }
        if explorer.open {
            let mut open = true;
            egui::Window::new("Shape Explorer")
                .id(egui::Id::new("shape-explorer-window"))
                .frame(
                    egui::Frame::window(&ctx.global_style())
                        .corner_radius(egui::CornerRadius::ZERO),
                )
                .open(&mut open)
                .default_size([520.0, 560.0])
                .min_width(360.0)
                .collapsible(false)
                .enabled(enabled)
                .show(ctx, |ui| explorer.contents(ui, port));
            explorer.open = open || !explorer.save(port);
        }
        self.shape_explorer = explorer;
    }
}
