//! Style artwork behind its name; banners have no separate caption band.
use crate::buttons;

pub(crate) fn paint(
    ui: &egui::Ui,
    response: &egui::Response,
    name: &str,
    texture: Option<(egui::TextureId, egui::Vec2)>,
    selected: bool,
) {
    paint_impl(ui, response, name, texture, selected, false);
}

pub(crate) fn paint_viewer(
    ui: &egui::Ui,
    response: &egui::Response,
    name: &str,
    texture: Option<(egui::TextureId, egui::Vec2)>,
    selected: bool,
) {
    paint_impl(ui, response, name, texture, selected, true);
}

fn paint_impl(
    ui: &egui::Ui,
    response: &egui::Response,
    name: &str,
    texture: Option<(egui::TextureId, egui::Vec2)>,
    selected: bool,
    envelope: bool,
) {
    let (fill, ink) = buttons::colors(
        ui,
        response,
        if selected {
            buttons::ButtonStyle::Accent
        } else {
            buttons::ButtonStyle::Quiet
        },
    );
    let rect = response.rect;
    let painter = ui.painter().with_clip_rect(rect);
    if let Some((texture, size)) = texture {
        let scale = (rect.width() / size.x.max(1.0)).max(rect.height() / size.y.max(1.0));
        let uv_size = rect.size() / (size * scale).max(egui::Vec2::splat(1.0));
        let uv = egui::Rect::from_center_size(egui::pos2(0.5, 0.5), uv_size);
        painter.image(texture, rect, uv, egui::Color32::WHITE);
        if !envelope {
            painter.rect_filled(
                rect,
                0,
                egui::Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 190),
            );
        }
    }
    let mut job = egui::text::LayoutJob::simple(
        name.to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        ink,
        (rect.width() - 16.0).max(1.0),
    );
    job.wrap.max_rows = 1;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    if envelope {
        let text = egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + 4.0,
                rect.center().y - galley.size().y * 0.5 - 4.0,
            ),
            galley.size() + egui::vec2(12.0, 8.0),
        );
        painter.rect_filled(text.intersect(rect), 3.0, fill);
    }
    painter.add(
        egui::epaint::TextShape::new(
            egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
            galley,
            ink,
        )
        .with_override_text_color(ink),
    );
}
