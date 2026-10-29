//! Standard numeric sliders. See docs/numeric-sliders.md.
use crate::theme;
use egui::{Pos2, Response, Sense, Stroke, Ui, Vec2};

pub(crate) fn value_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    decimals: usize,
) -> bool {
    value_slider_scaled(ui, label, value, range, suffix, decimals, false)
}

fn multiplier_position(value: f64) -> f64 {
    if value <= 1.0 {
        value / 2.0
    } else {
        0.5 + (value - 1.0) / 18.0
    }
}

fn multiplier_value(position: f64) -> f64 {
    if position <= 0.5 {
        position * 2.0
    } else {
        1.0 + (position - 0.5) * 18.0
    }
}

fn multiplier_track(ui: &mut Ui, value: &mut f64, width: f32) -> Response {
    let mut position = multiplier_position(*value);
    let response = range_track(ui, &mut position, 0.0..=1.0, width);
    if response.changed() {
        *value = multiplier_value(position);
    }
    response
}

fn value_slider_scaled(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    decimals: usize,
    unity_centered: bool,
) -> bool {
    let previous = *value;
    let number_width = ui
        .painter()
        .layout_no_wrap(
            format!("{:.*}{suffix}", decimals, range.end()),
            egui::TextStyle::Monospace.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
        + 2.0 * ui.spacing().button_padding.x
        + 8.0;
    crate::fields::row_with_min_control(ui, label, number_width, |ui| {
        let gap = ui.spacing().item_spacing.x;
        let compact = ui.available_width() < number_width + 24.0 + gap;
        let controls = |ui: &mut Ui| {
            if compact {
                ui.spacing_mut().button_padding.x = 0.0;
                ui.spacing_mut().interact_size.x =
                    ui.spacing().interact_size.x.min(ui.available_width());
            }
            let width = if compact {
                ui.available_width()
            } else {
                (ui.available_width() - number_width - gap).clamp(24.0, 360.0)
            };
            let response = if unity_centered {
                multiplier_track(ui, value, width)
            } else {
                range_track(ui, value, range.clone(), width)
            };
            response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *value, label));
            let mut changed = response.changed();
            changed |= ui
                .add(
                    egui::DragValue::new(value)
                        .range(range.clone())
                        .speed(if decimals == 0 { 1.0 } else { 0.1 })
                        .suffix(suffix)
                        .max_decimals(decimals),
                )
                .changed();
            changed
        };
        let changed = if compact {
            ui.vertical(controls).inner
        } else {
            ui.horizontal(controls).inner
        };
        if changed && !value.is_finite() {
            *value = previous;
            return false;
        }
        if changed {
            let factor = 10.0_f64.powi(decimals as i32);
            *value = ((*value * factor).round() / factor).clamp(*range.start(), *range.end());
        }
        changed
    })
}

/// Standard thin rail and filled 5-point handle; also used inside opacity controls.
pub(crate) fn range_track(
    ui: &mut Ui,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    width: f32,
) -> Response {
    let (track, _) = ui.allocate_exact_size(Vec2::new(width.max(1.0), 12.5), Sense::hover());
    let fill = theme::mix(
        ui.visuals().widgets.inactive.bg_fill,
        ui.visuals().extreme_bg_color,
        0.8,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("numeric-slider-track")
            .max_rect(track)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().slider_width = track.width();
    child.spacing_mut().slider_rail_height = 4.0;
    child.spacing_mut().interact_size.y = 12.5;
    child
        .style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(10.0));
    let widgets = &mut child.visuals_mut().widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        widget.fg_stroke = Stroke::NONE;
        widget.expansion = 0.0;
    }
    let response = child.add(
        egui::Slider::new(value, range.clone())
            .clamping(egui::SliderClamping::Edits)
            .show_value(false)
            .handle_shape(egui::style::HandleShape::Circle),
    );
    let radius = response.rect.height() / 2.5;
    let center = Pos2::new(
        egui::lerp(
            (response.rect.left() + radius)..=(response.rect.right() - radius),
            ((*value - *range.start()) / (*range.end() - *range.start()).max(f64::EPSILON))
                .clamp(0.0, 1.0) as f32,
        ),
        response.rect.center().y,
    );
    // Primary text is the palette's white; keep the knob identical on both sides.
    let knob = if ui.is_enabled() {
        ui.visuals().text_color()
    } else {
        theme::mix(fill, ui.visuals().text_color(), 0.4)
    };
    if response.has_focus() {
        child.painter().line_segment(
            [response.rect.left_center(), response.rect.right_center()],
            Stroke::new(2.0, ui.visuals().selection.stroke.color),
        );
    }
    child.painter().circle_filled(center, radius, knob);
    response
}

// Discrete values retain an evenly spaced rail and an editable actual value.
pub(crate) fn discrete_slider(
    ui: &mut Ui,
    label: &str,
    index: &mut u32,
    values: &[u32],
    suffix: &str,
) -> bool {
    if values.is_empty() {
        return false;
    }
    crate::fields::row_with_min_control(ui, label, 80.0, |ui| {
        let mut position = f64::from(*index);
        let mut number = values[(*index as usize).min(values.len() - 1)];
        let width = (ui.available_width() - 88.0).clamp(24.0, 360.0);
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            let response = range_track(ui, &mut position, 0.0..=(values.len() - 1) as f64, width);
            response.widget_info(|| {
                egui::WidgetInfo::slider(ui.is_enabled(), f64::from(number), label)
            });
            if response.changed() {
                *index = position.round() as u32;
                number = values[*index as usize];
                changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut number)
                        .range(values[0]..=*values.last().unwrap())
                        .suffix(suffix),
                )
                .changed()
            {
                *index = values
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, value)| value.abs_diff(number))
                    .unwrap()
                    .0 as u32;
                changed = true;
            }
        });
        changed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplier_track_clicks_match_zero_unity_and_ten() {
        let ctx = egui::Context::default();
        let mut value = 1.0;
        let mut rect = egui::Rect::NOTHING;
        let mut frame = |events, value: &mut f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = multiplier_track(ui, value, 200.0).rect;
                },
            );
            output.textures_delta.clear();
            rect
        };
        let rect = frame(vec![], &mut value);
        assert_eq!(value, 1.0);
        for (x, expected) in [
            (rect.left() + 5.0, 0.0),
            (rect.center().x, 1.0),
            (rect.right() - 5.0, 10.0),
        ] {
            let pos = egui::pos2(x, rect.center().y);
            for pressed in [true, false] {
                frame(
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    &mut value,
                );
            }
            assert!((value - expected).abs() < 1e-9, "{value} != {expected}");
        }
    }

    #[test]
    fn standard_handle_stays_small_solid_and_disabled_values_stay_unchanged() {
        for scale in [1.0, 1.5, 2.0] {
            for font in [13.0, 24.0] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                let mut value = 170.0;
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Body, egui::FontId::proportional(font));
                    ui.add_enabled_ui(false, |ui| {
                        assert!(!value_slider(
                            ui,
                            "Particle count",
                            &mut value,
                            0.0..=500.0,
                            "",
                            0
                        ));
                    });
                });
                output.textures_delta.clear();
                assert_eq!(value, 170.0);
                fn solid_knob(shape: &egui::Shape) -> bool {
                    match shape {
                        egui::Shape::Circle(circle) => {
                            circle.radius == 5.0
                                && circle.stroke == Stroke::NONE
                                && circle.fill != egui::Color32::TRANSPARENT
                        }
                        egui::Shape::Vec(shapes) => shapes.iter().any(solid_knob),
                        _ => false,
                    }
                }
                assert!(
                    output.shapes.iter().any(|shape| solid_knob(&shape.shape)),
                    "Missing compact solid knob at {scale}x / {font}pt"
                );
            }
        }
    }
}
