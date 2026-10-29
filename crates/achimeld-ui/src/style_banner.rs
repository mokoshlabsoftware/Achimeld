//! Style artwork behind its name; banners have no separate caption band.
use crate::buttons;

/// A moving white alpha trail identifies a recipe whose edits affect its users.
/// Paint inside the banner so gapless neighbors and hit targets stay unchanged.
pub(crate) fn paint_shared_edit_glow(ui: &egui::Ui, rect: egui::Rect) {
    if !ui.is_rect_visible(rect) || !rect.is_positive() {
        return;
    }
    let phase = ui.input(|input| (input.time / 3.0).rem_euclid(1.0)) as f32;
    let depth = 3.0_f32.min(rect.height() * 0.25).min(rect.width() * 0.25);
    let inner = rect.shrink(depth);
    let outside = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let inside = [
        inner.left_top(),
        inner.right_top(),
        inner.right_bottom(),
        inner.left_bottom(),
    ];
    let perimeter = 2.0 * (rect.width() + rect.height());
    let mut distance = 0.0;
    let mut mesh = egui::Mesh::default();
    for side in 0..4 {
        let next = (side + 1) % 4;
        let length = outside[side].distance(outside[next]);
        // Bounded geometry, with a smoothly interpolated alpha across each quad.
        for step in 0..32 {
            let base = mesh.vertices.len() as u32;
            for endpoint in [step, step + 1] {
                let t = endpoint as f32 / 32.0;
                let position = (distance + length * t) / perimeter;
                let age = (phase - position).rem_euclid(1.0);
                let trail = (1.0 - age / 0.65).max(0.0);
                let alpha = (180.0 * trail * trail) as u8;
                mesh.colored_vertex(
                    outside[side].lerp(outside[next], t),
                    egui::Color32::from_white_alpha(alpha),
                );
                mesh.colored_vertex(
                    inside[side].lerp(inside[next], t),
                    egui::Color32::TRANSPARENT,
                );
            }
            mesh.add_triangle(base, base + 2, base + 1);
            mesh.add_triangle(base + 1, base + 2, base + 3);
        }
        distance += length;
    }
    ui.painter().add(mesh);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}

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
