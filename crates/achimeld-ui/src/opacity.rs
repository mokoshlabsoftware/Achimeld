use crate::icons::{self, Icon};
use crate::theme;
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

pub(crate) fn value_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    decimals: usize,
) -> bool {
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
            let response = range_track(ui, value, range.clone(), width);
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
        if changed {
            let factor = 10.0_f64.powi(decimals as i32);
            *value = ((*value * factor).round() / factor).clamp(*range.start(), *range.end());
        }
        changed
    })
}

/// The same compact opacity control is used by layer rows and properties.
pub(crate) fn slider(ui: &mut Ui, percent: &mut f64, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::hover());
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    let fill = theme::mix(
        ui.visuals().widgets.inactive.bg_fill,
        ui.visuals().extreme_bg_color,
        0.8,
    );
    let shadow = Color32::from_rgb(fill.r() / 2, fill.g() / 2, fill.b() / 2);
    let highlight = theme::mix(fill, ui.visuals().text_color(), 0.12);
    let radius = ui.visuals().widgets.inactive.corner_radius;
    // A single physical pixel: shadow above/left, light below/right, recessed fill.
    ui.painter().rect_filled(rect, radius, highlight);
    ui.painter().rect_filled(
        Rect::from_min_max(rect.min, rect.max - Vec2::splat(pixel)),
        radius,
        shadow,
    );
    ui.painter().rect_filled(rect.shrink(pixel), radius, fill);

    let label = crate::localization::text(ui.ctx(), "Opacity");
    let ink = ui.visuals().weak_text_color();
    icons::paint(
        ui.painter(),
        Rect::from_min_size(rect.min + Vec2::new(6.0, 3.0), Vec2::splat(12.0)),
        Icon::Opacity,
        ink,
    );
    let mut text = egui::text::LayoutJob::simple(
        label.clone(),
        egui::FontId::proportional(10.0),
        ink,
        (width - 28.0).max(0.0),
    );
    text.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(text);
    ui.painter().galley(
        Pos2::new(rect.left() + 21.0, rect.top() + 9.0 - galley.size().y / 2.0),
        galley,
        ink,
    );

    let track = Rect::from_min_max(
        rect.min + Vec2::new(7.0, 18.5),
        rect.max - Vec2::new(7.0, 3.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(track)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let response = range_track(&mut child, percent, 0.0..=100.0, track.width());
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *percent, &label));
    response.on_hover_text(format!("{label}: {percent:.0}%"))
}

/// The filled compact knob shared with the opacity controls in layer rows.
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
            .id_salt("opacity-track")
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
