use crate::icons::{self, Icon};
use crate::theme;
use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

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
    let response = crate::sliders::range_track(&mut child, percent, 0.0..=100.0, track.width());
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *percent, &label));
    response.on_hover_text(format!("{label}: {percent:.0}%"))
}
