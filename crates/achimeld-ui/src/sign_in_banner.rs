//! A bounded, non-interactive ribbon attached to the navigation bar.
use achimeld_application::Preferences;
use egui::{Color32, Pos2, Rect, Sense, Vec2};

pub(crate) fn show(root: &mut egui::Ui, waiting: bool, preferences: &Preferences) {
    if !waiting {
        return;
    }
    egui::Panel::top("sign-in-attention")
        .exact_size(38.0)
        .frame(egui::Frame::new())
        .show(root, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let painter = ui.painter_at(rect);
            let time = ui.input(|i| i.time) as f32;
            let warning = crate::theme::color(preferences, "Warning");
            let orange = Color32::from_rgb(warning.r(), warning.g() / 2, warning.b() / 5);
            painter.rect_filled(rect, 0.0, orange);
            // Traveling copper bands and diagonal streaks evoke a demoscene raster
            // effect. The label stays steady while the background moves.
            for row in 0..19 {
                let glow = ((row as f32 * 0.5 - time * 2.0).sin() * 0.5 + 0.5) * 0.3;
                let band = Rect::from_min_size(
                    rect.min + Vec2::new(0.0, row as f32 * 2.0),
                    Vec2::new(rect.width(), 2.0),
                );
                painter.rect_filled(band, 0.0, crate::theme::mix(orange, Color32::WHITE, glow));
            }
            let phase = (time * 65.0).rem_euclid(96.0);
            for index in -1..((rect.width() / 96.0).ceil() as i32 + 1).min(256) {
                let x = rect.left() + index as f32 * 96.0 + phase;
                painter.line_segment(
                    [Pos2::new(x, rect.bottom()), Pos2::new(x + 38.0, rect.top())],
                    egui::Stroke::new(8.0, Color32::from_black_alpha(35)),
                );
            }
            let label = crate::localization::text(ui.ctx(), "Waiting for sign-in");
            let font = egui::FontId::proportional(preferences.normal_size.max(16.0));
            let galley = painter.layout_no_wrap(label, font, Color32::BLACK);
            painter.galley(rect.center() - galley.size() / 2.0, galley, Color32::BLACK);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        });
}
