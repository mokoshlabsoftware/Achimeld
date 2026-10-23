use crate::buttons::{self, Button};
use achimeld_application::gradient_map::{
    ColorField, ColorPoint, GradientMap, MAX_POINTS, decode_color, encode_color,
};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
#[path = "gradient_map_preview.rs"]
mod preview;

#[path = "gradient_map_controls.rs"]
mod controls;
#[path = "gradient_map_interaction.rs"]
mod interaction;
use interaction::State;
fn screen(rect: Rect, point: &ColorPoint) -> Pos2 {
    Pos2::new(
        egui::lerp(rect.x_range(), point.x as f32),
        egui::lerp(rect.y_range(), point.y as f32),
    )
}

fn position(rect: Rect, pointer: Pos2) -> [f64; 2] {
    [
        f64::from(((pointer.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0)),
        f64::from(((pointer.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0)),
    ]
}

fn hit(map: &GradientMap, rect: Rect, pointer: Pos2) -> Option<usize> {
    map.points
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let distance = screen(rect, p).distance(pointer);
            (distance <= 10.0).then_some((i, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Inverting middle gray has almost no contrast; lift that case toward white.
pub(super) fn point_ink([r, g, b, _]: [u8; 4]) -> Color32 {
    let mut color = [255 - r, 255 - g, 255 - b];
    if color
        .iter()
        .zip([r, g, b])
        .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
        .sum::<i32>()
        < 120 * 120
    {
        color = color.map(|v| ((u16::from(v) + 4 * 255) / 5) as u8);
    }
    Color32::from_rgb(color[0], color[1], color[2])
}

fn checker(ui: &egui::Ui, rect: Rect) {
    let colors = [
        ui.visuals().panel_fill,
        ui.visuals().widgets.inactive.bg_fill,
    ];
    ui.painter().rect_filled(rect, 0.0, colors[0]);
    for y in 0..(rect.height() / 8.0).ceil() as usize {
        for x in 0..(rect.width() / 8.0).ceil() as usize {
            if (x + y) % 2 == 1 {
                let cell = Rect::from_min_size(
                    rect.min + egui::vec2(x as f32 * 8.0, y as f32 * 8.0),
                    Vec2::splat(8.0),
                );
                ui.painter()
                    .rect_filled(cell.intersect(rect), 0.0, colors[1]);
            }
        }
    }
}

fn slice_strip(ui: &egui::Ui, field: &ColorField, x: f64, rect: Rect) {
    checker(ui, rect);
    let colors = field.slice(x, 256);
    let mut mesh = egui::Mesh::default();
    for (i, [r, g, b, a]) in colors.into_iter().enumerate() {
        let y = egui::lerp(rect.y_range(), i as f32 / 255.0);
        let color = Color32::from_rgba_unmultiplied(r, g, b, a);
        mesh.colored_vertex(Pos2::new(rect.left(), y), color);
        mesh.colored_vertex(Pos2::new(rect.right(), y), color);
        if i > 0 {
            let v = i as u32 * 2;
            mesh.add_triangle(v - 2, v - 1, v);
            mesh.add_triangle(v, v - 1, v + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub(crate) fn presets(ui: &mut egui::Ui, map: &mut GradientMap) {
    crate::inspector::field(ui, "Color map preset", 120.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt("2d-gradient-presets")
            .width(ui.available_width().min(180.0))
            .truncate()
            .selected_text("Choose preset…")
            .show_ui(ui, |ui| {
                for (name, preset) in achimeld_application::gradient_map::presets() {
                    if ui
                        .add(
                            Button::new(name)
                                .bar()
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        *map = preset;
                        ui.close();
                    }
                }
            });
    });
}

pub(crate) fn editor(ui: &mut egui::Ui, map: &mut GradientMap) -> egui::Response {
    let id = ui.make_persistent_id("2d-gradient-editor");
    let mut state = ui
        .data_mut(|data| data.get_temp::<State>(id))
        .unwrap_or_default();
    state.retain(map.points.len());
    let before = map.clone();
    buttons::bar(ui, 24.0, |ui| {
        let image = crate::particles::glyph(
            ui.ctx(),
            "gradient-flip",
            include_bytes!("../assets/toolbar/gradient-flip.rgba"),
        );
        let response = ui.add(
            Button::new(image)
                .bar()
                .id(id.with("flip"))
                .min_size(Vec2::splat(buttons::bar_height(ui, 24.0)))
                .image_tint_follows_text_color(true),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                response.enabled(),
                "Reverse gradient",
            )
        });
        if response
            .on_hover_text("Reverse particle gradients from top to bottom")
            .clicked()
            && map.reverse_gradient().is_ok()
        {
            state.reset_gesture();
        }
        if ui
            .add_enabled(
                !state.selected.is_empty(),
                Button::new("Delete").id(id.with("delete-points")).bar(),
            )
            .on_hover_text("Delete selected color points")
            .clicked()
        {
            state.delete(map);
        }
    });
    let Ok(field) = map.compile() else {
        return ui.label("Invalid 2D gradient");
    };
    let (outer, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 128.0), Sense::hover());
    let rect = Rect::from_min_max(
        outer.min + Vec2::splat(5.0),
        outer.max - egui::vec2(37.0, 5.0),
    );
    let slice_rect = Rect::from_min_max(
        egui::pos2(rect.right() + 8.0, rect.top()),
        outer.max - Vec2::splat(5.0),
    );
    let mut response = ui.interact(rect.expand(5.0), id.with("map"), Sense::click_and_drag());
    interaction::handle(ui, &response, rect, map, &field, &mut state);
    // Gesture edits get their quick preview in this frame; refinement stays on the bounded worker.
    let field = if *map == before {
        field
    } else {
        match map.compile() {
            Ok(field) => field,
            Err(_) => return ui.label("Invalid 2D gradient"),
        }
    };
    let texture = preview::texture(ui, map, &field);
    checker(ui, rect);
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let scale = ui.ctx().pixels_per_point();
    let line_x = egui::lerp(rect.x_range(), state.slice as f32);
    for y in (0..rect.height() as usize).step_by(8) {
        let y = rect.top() + y as f32;
        ui.painter().line_segment(
            [
                Pos2::new(line_x, y),
                Pos2::new(line_x, (y + 4.0).min(rect.bottom())),
            ],
            Stroke::new(
                1.0 / scale,
                point_ink(field.sample(state.slice, f64::from((y - rect.top()) / rect.height()))),
            ),
        );
    }
    if let Some(area) = state.marquee() {
        ui.painter().rect_filled(
            area,
            0.0,
            ui.visuals().selection.bg_fill.gamma_multiply(0.2),
        );
        ui.painter().rect_stroke(
            area,
            0.0,
            Stroke::new(1.0, ui.visuals().selection.bg_fill),
            egui::StrokeKind::Inside,
        );
    }
    let hovered = response.hovered()
        && ui.input(|i| i.pointer.hover_pos().is_some_and(|p| outer.contains(p)));
    if hovered
        || !state.selected.is_empty()
        || state.moving()
        || state.marquee().is_some()
        || state.color
    {
        for (i, p) in map.points.iter().enumerate() {
            let center = screen(rect, p);
            let ink = point_ink(field.sample(p.x, p.y));
            ui.painter().circle_filled(center, 3.0 / scale, ink);
            if state.selected.contains(&i) {
                ui.painter()
                    .circle_stroke(center, 7.0 / scale, Stroke::new(1.0 / scale, ink));
            }
        }
    }
    if state.moving() && ui.input(|i| i.pointer.primary_down()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if hovered
        && ui
            .input(|i| i.pointer.hover_pos())
            .and_then(|p| hit(map, rect, p))
            .is_some()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    ui.weak("Click to test a slice · Drag empty space to select · Drag selected points to move");
    ui.weak("Shift-click adds to selection · Ctrl-click removes · Double-click to add a point");
    controls::slice(ui, &mut state.slice);
    slice_strip(ui, &field, state.slice, slice_rect);
    controls::show(ui, id, map, &mut state, outer);
    ui.data_mut(|data| data.insert_temp(id, state));
    if *map != before {
        response.mark_changed();
        ui.ctx().request_repaint();
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            ui.is_enabled(),
            "2D gradient color map",
        )
    });
    response
}
