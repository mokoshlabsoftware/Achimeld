use crate::{Desktop, buttons, icons, layer_palette, theme};
use achimeld_application::{EditorPort, ReorderGesture};
use egui::{Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::{Value, json};

pub(crate) struct EffectDrag {
    pub layer: String,
    gesture: ReorderGesture,
}

pub(crate) fn effect_control_id(
    achievement: &str,
    layer: &str,
    effect: &str,
    control: &str,
) -> egui::Id {
    egui::Id::new(("effect-control", achievement, layer, effect, control))
}

impl Desktop {
    pub(crate) fn layer_effects(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        achievement: &str,
        layer: &mut Value,
        indent: f32,
        origin: Pos2,
    ) {
        let owner = layer["id"].as_str().unwrap_or("").to_owned();
        let inset = (indent + 58.0).min((ui.available_width() - 96.0).max(0.0));
        // Center the add action on the FX spine, halfway to the first junction.
        let size = icons::layer_control_size(icons::Icon::Effects);
        let row_height = (ui
            .text_style_height(&egui::TextStyle::Button)
            .max(ui.text_style_height(&egui::TextStyle::Body))
            + 4.0)
            .max(30.0);
        let add_top = origin.y
            + if layer["effects"]
                .as_array()
                .is_some_and(|effects| !effects.is_empty())
            {
                row_height * 0.25
            } else {
                0.0
            };
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().interact_size.y = size;
            let height = (origin.y + size - ui.cursor().top()).max(0.0);
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
            let add = ui
                .place(
                    Rect::from_min_size(
                        Pos2::new(origin.x - size * 0.5, add_top),
                        Vec2::splat(size),
                    ),
                    buttons::Button::new(egui::Atoms::default())
                        .id(layer_palette::layer_control_id(achievement, &owner, "add"))
                        .bar()
                        .min_size(Vec2::splat(size)),
                )
                .on_hover_text("Add effect");
            buttons::menu_popup(ui, &add, |ui| {
                if let Some(effect) = layer_palette::effect_menu(ui, layer) {
                    self.selected_layer = Some(owner.clone());
                    self.layer_palette.selected_effect = Some(effect);
                }
            });
            let (_, ink) = buttons::colors(ui, &add, buttons::ButtonStyle::Quiet);
            icons::paint(
                ui.painter(),
                Rect::from_center_size(add.rect.center(), Vec2::splat(16.0)),
                icons::Icon::Plus,
                ink,
            );
            add.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, add.enabled(), "Add effect")
            });
        });
        let Some(effects) = layer.get_mut("effects").and_then(Value::as_array_mut) else {
            return;
        };
        if effects.is_empty() {
            return;
        }
        let ids: Vec<_> = effects
            .iter()
            .map(|effect| effect["id"].as_str().unwrap_or("").to_owned())
            .collect();
        let mut rects = Vec::new();
        let mut started = None;
        let mut remove = None;
        let mut preview = None;
        let mut spine = Pos2::new(origin.x, add_top + size);
        let stroke = Stroke::new(
            1.0 / ui.ctx().pixels_per_point(),
            ui.visuals().weak_text_color().gamma_multiply(0.6),
        );
        ui.horizontal(|ui| {
            ui.add_space(inset);
            ui.vertical(|ui| {
                for (index, effect) in effects.iter_mut().enumerate() {
                    let id = &ids[index];
                    ui.push_id(id, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
                            ui.set_min_height(row_height);
                            let mut visible = effect["visible"].as_bool().unwrap_or(true);
                            let visibility = ui
                                .add(egui::Checkbox::without_text(&mut visible))
                                .on_hover_text("Effect visibility");
                            if visibility.changed() {
                                effect["visible"] = json!(visible);
                            }
                            let junction = Pos2::new(spine.x, visibility.rect.center().y);
                            ui.painter().line_segment([spine, junction], stroke);
                            ui.painter().line_segment(
                                [
                                    junction,
                                    Pos2::new(
                                        (visibility.rect.left() - 4.0).max(spine.x),
                                        junction.y,
                                    ),
                                ],
                                stroke,
                            );
                            spine = junction;
                            ui.add_space(4.0);
                            // The selector, mini fields and delete action share one resting strip.
                            // Keep the layer's tinted backdrop from showing as patches between them.
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new(ui.available_width(), row_height),
                                ),
                                0.0,
                                ui.visuals().panel_fill,
                            );
                            let paint_start = ui.painter().add(egui::Shape::Noop);
                            let plan = crate::effect_mini::layout(
                                ui,
                                effect,
                                (ui.available_width() - 28.0).max(24.0),
                                port.preferences().show_effect_mini_values,
                            );
                            let kind = effect["kind"].as_str().unwrap_or("Effect").to_owned();
                            let name = if plan.name {
                                layer_palette::label(&kind)
                            } else {
                                ""
                            };
                            let row = ui
                                .add_sized(
                                    Vec2::new(plan.selector_width, row_height),
                                    buttons::Button::selectable(
                                        self.layer_palette.selected_effect.as_ref() == Some(id),
                                        (
                                            crate::effect_icons::image(ui.ctx(), &kind, 16.0),
                                            crate::localization::text(ui.ctx(), name),
                                        ),
                                    )
                                    .id(effect_control_id(achievement, &owner, id, "row"))
                                    .bar()
                                    .image_tint_follows_text_color(true)
                                    .icon_spacing(4.0)
                                    .sense(Sense::click_and_drag())
                                    .align(if plan.name {
                                        egui::Align2::LEFT_CENTER
                                    } else {
                                        egui::Align2::CENTER_CENTER
                                    })
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                                )
                                .on_hover_cursor(egui::CursorIcon::Grab)
                                .on_hover_text(format!(
                                    "{} · {}",
                                    crate::localization::text(
                                        ui.ctx(),
                                        layer_palette::label(&kind)
                                    ),
                                    crate::localization::text(
                                        ui.ctx(),
                                        "Drag to change effect order"
                                    )
                                ));
                            row.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    row.enabled(),
                                    self.layer_palette.selected_effect.as_ref() == Some(id),
                                    crate::localization::text(
                                        ui.ctx(),
                                        layer_palette::label(&kind),
                                    ),
                                )
                            });
                            rects.push(row.rect);
                            if row.clicked() || row.drag_started() {
                                self.selected_layer = Some(owner.clone());
                                self.layer_palette.selected_effect = Some(id.clone());
                            }
                            if row.drag_started() {
                                started = Some(id.clone());
                                self.layer_drag = None;
                            }
                            if crate::effect_mini::show(ui, effect, &plan) {
                                self.selected_layer = Some(owner.clone());
                                self.layer_palette.selected_effect = Some(id.clone());
                            }
                            if started.as_ref() == Some(id)
                                || self.layer_palette.effect_drag.as_ref().is_some_and(|drag| {
                                    drag.layer == owner
                                        && self.layer_palette.selected_effect.as_ref() == Some(id)
                                })
                            {
                                preview = ui.ctx().graphics(|graphics| {
                                    graphics.get(ui.layer_id()).map(|list| {
                                        (
                                            row.rect,
                                            list.all_entries()
                                                .skip(paint_start.0)
                                                .map(|paint| paint.shape.clone())
                                                .collect::<Vec<_>>(),
                                        )
                                    })
                                });
                            }
                            let delete =
                                ui.add_sized(
                                    [24.0, row_height],
                                    buttons::Button::new(egui::Atoms::default())
                                        .id(effect_control_id(achievement, &owner, id, "delete"))
                                        .bar(),
                                )
                                .on_hover_text(
                                    crate::localization::text(ui.ctx(), "Delete effect"),
                                );
                            let (_, ink) =
                                buttons::colors(ui, &delete, buttons::ButtonStyle::Quiet);
                            delete.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    delete.enabled(),
                                    crate::localization::text(ui.ctx(), "Delete effect"),
                                )
                            });
                            icons::paint(
                                ui.painter(),
                                Rect::from_center_size(delete.rect.center(), Vec2::splat(16.0)),
                                icons::Icon::Close,
                                ink,
                            );
                            if delete.clicked() {
                                remove = Some(index);
                                if self.layer_palette.selected_effect.as_ref() == Some(id) {
                                    self.layer_palette.selected_effect = None;
                                }
                            }
                        });
                    });
                }
            });
        });
        if let Some(index) = remove {
            effects.remove(index);
            self.layer_palette.effect_drag = None;
            return;
        }
        if let Some(id) = started {
            self.layer_palette.effect_drag = ReorderGesture::begin(
                ids.clone(),
                rects.iter().map(|rect| rect.center().y).collect(),
                &id,
            )
            .map(|gesture| EffectDrag {
                layer: owner.clone(),
                gesture,
            });
        }
        if !self
            .layer_palette
            .effect_drag
            .as_ref()
            .is_some_and(|drag| drag.layer == owner)
        {
            return;
        }
        let drag = self.layer_palette.effect_drag.as_mut().unwrap();
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            drag.gesture.update(pos.y);
        }
        let slot = drag.gesture.slot();
        let marker = if slot == 0 {
            rects.first().map(|rect| rect.top())
        } else {
            rects.get(slot - 1).map(|rect| rect.bottom())
        };
        if let Some(y) = marker {
            ui.painter().line_segment(
                [
                    Pos2::new(rects[0].left(), y),
                    Pos2::new(rects[0].right(), y),
                ],
                Stroke::new(2.0, theme::color(port.preferences(), "Accent")),
            );
        }
        if ui.input(|i| i.pointer.primary_released()) {
            let drag = self.layer_palette.effect_drag.take().unwrap();
            if let Ok(order) = drag.gesture.finish(&ids)
                && order != ids
            {
                port.finish_group();
                effects.sort_by_key(|effect| order.iter().position(|id| effect["id"] == *id));
                self.layer_palette.edit_label = Some("Reorder layer effects");
            }
        } else if let Some((rect, shapes)) = preview
            && let Some((origin, pointer)) =
                ui.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()))
        {
            let delta = pointer - origin;
            let mut painter = ui
                .ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new(("effect-drag-preview", achievement)),
                ))
                .with_clip_rect(rect.translate(delta).intersect(ui.ctx().content_rect()));
            painter.set_opacity(0.72);
            for mut shape in shapes {
                shape.translate(delta);
                painter.add(shape);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            ui.ctx().request_repaint();
        }
    }
}
