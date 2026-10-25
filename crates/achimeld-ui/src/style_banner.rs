//! Style artwork behind its name; banners have no separate caption band.
use crate::buttons;

pub(crate) fn paint(
    ui: &egui::Ui,
    response: &egui::Response,
    name: &str,
    texture: Option<(egui::TextureId, egui::Vec2)>,
    selected: bool,
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
        painter.rect_filled(
            rect,
            0,
            egui::Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 190),
        );
    }
    let mut job = egui::text::LayoutJob::simple(
        name.to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        ink,
        (rect.width() - 16.0).max(1.0),
    );
    job.wrap.max_rows = 1;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    painter.add(
        egui::epaint::TextShape::new(
            egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
            galley,
            ink,
        )
        .with_override_text_color(ink),
    );
}
