use crate::{Desktop, buttons, localization};
use achimeld_application::{EditorPort, ports::LayerBlendPreview};
use serde_json::{Value, json};

pub(crate) const MODES: [&str; 8] = [
    "Normal",
    "Multiply",
    "Screen",
    "Overlay",
    "Darken",
    "Lighten",
    "Add",
    "Difference",
];

pub(crate) struct BlendHover {
    request: LayerBlendPreview,
    popup: egui::Id,
    layer: egui::LayerId,
    rows: Vec<(&'static str, egui::Id, egui::Rect)>,
    revision: u64,
    active: Option<LayerBlendPreview>,
}

impl Desktop {
    pub(crate) fn sync_blend_preview(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        let valid = self.blend_hover.as_ref().is_some_and(|hover| {
            enabled
                && self.layers_available()
                && self.layer_edit_target(port).as_ref() == Some(&hover.request.achievement_id)
                && port.editor().revision() == hover.revision
                && egui::Popup::is_id_open(ctx, hover.popup)
                && ctx.input(|i| i.focused && !i.key_pressed(egui::Key::Escape))
        });
        if !valid && self.blend_hover.take().is_some() {
            ctx.request_repaint();
        }
        let preview = self.blend_hover.as_mut().and_then(|hover| {
            let pointer = ctx.input(|i| i.pointer.hover_pos());
            let mode =
                if let Some(active) = crate::dropdown_navigation::keyboard_row(ctx, hover.layer) {
                    hover.rows.iter().find(|(_, id, _)| *id == active)
                } else {
                    pointer
                        .and_then(|p| (ctx.layer_id_at(p) == Some(hover.layer)).then_some(p))
                        .and_then(|p| hover.rows.iter().find(|(_, _, rect)| rect.contains(p)))
                };
            let preview = mode.map(|(mode, _, _)| LayerBlendPreview {
                mode: (*mode).into(),
                ..hover.request.clone()
            });
            if hover.active != preview {
                ctx.request_repaint();
                hover.active = preview.clone();
            }
            preview
        });
        port.set_layer_blend_preview(preview);
    }

    pub(crate) fn layer_blend_menu(
        &mut self,
        ui: &mut egui::Ui,
        port: &dyn EditorPort,
        achievement: &str,
        layer: &mut Value,
        button: buttons::Button<'_>,
    ) -> egui::Response {
        let (response, menu) = blend_menu(ui, layer, "blendMode", "Normal", button);
        if let Some(menu) = menu {
            let popup = egui::Popup::default_response_id(&response);
            if menu.committed {
                self.blend_hover = None;
            } else {
                let active = self
                    .blend_hover
                    .as_ref()
                    .filter(|hover| hover.popup == popup)
                    .and_then(|hover| hover.active.clone());
                self.blend_hover = Some(BlendHover {
                    request: LayerBlendPreview {
                        achievement_id: achievement.into(),
                        layer_id: layer["id"].as_str().unwrap_or("").into(),
                        mode: String::new(),
                    },
                    popup,
                    layer: menu.layer,
                    rows: menu.rows,
                    revision: port.editor().revision(),
                    active,
                });
            }
        }
        response
    }
}

pub(crate) fn button(
    ui: &egui::Ui,
    mode: &str,
    width: f32,
    label: bool,
) -> buttons::Button<'static> {
    let button = if label {
        buttons::Button::new((
            crate::blend_icons::image(ui.ctx(), mode, 18.0),
            localization::text(ui.ctx(), mode),
            crate::dropdowns::disclosure(ui),
        ))
    } else {
        let edge = (width - 2.0 * ui.spacing().button_padding.x).clamp(0.0, 18.0);
        buttons::Button::new(crate::blend_icons::image(ui.ctx(), mode, edge))
    };
    button
        .bar()
        .icon_spacing(5.0)
        .image_tint_follows_text_color(true)
        .min_size(egui::vec2(width, 0.0))
        .wrap_mode(egui::TextWrapMode::Truncate)
}

struct MenuRows {
    layer: egui::LayerId,
    rows: Vec<(&'static str, egui::Id, egui::Rect)>,
    committed: bool,
}

fn blend_menu(
    ui: &mut egui::Ui,
    value: &mut Value,
    key: &str,
    default: &str,
    button: buttons::Button<'_>,
) -> (egui::Response, Option<MenuRows>) {
    ui.scope(|ui| {
        let mut font = egui::TextStyle::Button.resolve(&ui.ctx().global_style());
        font.size = (font.size - 1.0).max(1.0);
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Button, font.clone());
        let response = ui.add(button.bar());
        let menu = buttons::menu_popup(ui, &response, |ui| {
            // Match every resting row to its popup, including the committed mode.
            ui.visuals_mut().panel_fill = ui.visuals().window_fill;
            let width = MODES
                .iter()
                .map(|mode| {
                    ui.painter()
                        .layout_no_wrap(
                            localization::text(ui.ctx(), mode),
                            font.clone(),
                            ui.visuals().text_color(),
                        )
                        .size()
                        .x
                })
                .fold(response.rect.width(), |width, text| {
                    width.max(text + 18.0 + 5.0 + 2.0 * ui.spacing().button_padding.x)
                });
            let height = buttons::bar_height(ui, 18.0);
            let mut menu = MenuRows {
                layer: ui.layer_id(),
                rows: Vec::with_capacity(MODES.len()),
                committed: false,
            };
            for mode in MODES {
                let item = ui.add_sized(
                    [width, height],
                    buttons::Button::selectable(
                        value[key].as_str().unwrap_or(default) == mode,
                        (
                            crate::blend_icons::image(ui.ctx(), mode, 18.0),
                            localization::text(ui.ctx(), mode),
                        ),
                    )
                    .bar()
                    .style(buttons::ButtonStyle::Quiet)
                    .align(egui::Align2::LEFT_CENTER)
                    .icon_spacing(5.0)
                    .image_tint_follows_text_color(true)
                    .wrap_mode(egui::TextWrapMode::Extend),
                );
                if item.enabled() {
                    menu.rows
                        .push((mode, item.id, item.rect.intersect(ui.clip_rect())));
                }
                if item.clicked() {
                    value[key] = json!(mode);
                    menu.committed = true;
                    ui.close();
                }
            }
            menu
        });
        (response, menu)
    })
    .inner
}

pub(crate) fn property(ui: &mut egui::Ui, value: &mut Value) {
    property_key(ui, "Blend mode", value, "blendMode", "Normal");
}

pub(crate) fn property_key(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    default: &str,
) {
    let icon = crate::blend_icons::image(ui.ctx(), "control", 20.0).tint(ui.visuals().text_color());
    let hint = localization::text(ui.ctx(), label);
    crate::fields::row_with_icon(ui, label, Some((icon, hint)), |ui| {
        let mode = value[key].as_str().unwrap_or(default);
        let button = button(ui, mode, ui.available_width().min(180.0), true);
        blend_menu(ui, value, key, default, button);
    });
}

pub(crate) fn property_row<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let icon = crate::blend_icons::image(ui.ctx(), "control", 20.0).tint(ui.visuals().text_color());
    let hint = localization::text(ui.ctx(), "Blend mode");
    crate::fields::row_with_icon(ui, "Blend mode", Some((icon, hint)), content)
}
