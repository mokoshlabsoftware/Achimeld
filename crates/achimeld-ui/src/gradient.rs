//! Achiever's direct gradient editing, using the existing recipe and shared controls.
use crate::buttons::{self, Button, UiButtonExt as _};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::{Value, json};

const MAX_STOPS: usize = 128;

#[derive(Clone, Copy, Default)]
struct EditorState {
    selected: usize,
    pending_click: Option<(f64, f64)>,
    closing: bool,
    drag: Option<(usize, f32)>,
    menu: Option<(Option<usize>, f64)>,
    color: Option<usize>,
    picker: Option<(usize, [u8; 4], egui::ecolor::Hsva)>,
}

fn offset(stop: &Value) -> f64 {
    stop["offset"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0)
}

fn rgba(stop: &Value) -> [u8; 4] {
    let hex = stop["colorHex"].as_str().unwrap_or("#FFFFFFFF");
    let decoded = hex
        .strip_prefix('#')
        .and_then(|hex| u32::from_str_radix(hex, 16).ok().map(|n| (n, hex.len())));
    match decoded {
        Some((n, 8)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, (n >> 24) as u8],
        Some((n, 6)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255],
        _ => [255; 4],
    }
}

fn set_color(stop: &mut Value, color: [u8; 4]) {
    stop["colorHex"] = json!(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color[3], color[0], color[1], color[2]
    ));
}

fn ordered(stops: &[Value]) -> Vec<(f64, [u8; 4])> {
    let mut colors: Vec<_> = stops
        .iter()
        .map(|stop| (offset(stop), rgba(stop)))
        .collect();
    colors.sort_by(|a, b| a.0.total_cmp(&b.0));
    colors
}

fn sample(colors: &[(f64, [u8; 4])], position: f64) -> [u8; 4] {
    let Some(first) = colors.first() else {
        return [0; 4];
    };
    let left = colors.iter().rfind(|s| s.0 <= position).unwrap_or(first);
    let right = colors.iter().find(|s| s.0 > position).unwrap_or(left);
    let t = if right.0 > left.0 {
        ((position - left.0) / (right.0 - left.0)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    std::array::from_fn(|i| {
        (f64::from(left.1[i]) + (f64::from(right.1[i]) - f64::from(left.1[i])) * t).round() as u8
    })
}

fn add(stops: &mut Vec<Value>, position: f64, state: &mut EditorState) {
    if stops.len() >= MAX_STOPS {
        return;
    }
    let color = sample(&ordered(stops), position);
    let mut stop = json!({"offset":position});
    set_color(&mut stop, color);
    state.selected = stops.len();
    stops.push(stop);
}

fn delete(stops: &mut Vec<Value>, index: usize, state: &mut EditorState) {
    if stops.len() <= 1 || index >= stops.len() {
        return;
    }
    state.pending_click = None;
    stops.remove(index);
    state.selected = index.min(stops.len() - 1);
    state.drag = None;
    state.color = None;
}

pub(crate) fn checker(ui: &egui::Ui, rect: Rect) {
    let colors = [
        ui.visuals().panel_fill,
        ui.visuals().widgets.inactive.bg_fill,
    ];
    ui.painter().rect_filled(rect, 0.0, colors[0]);
    let columns = (rect.width() / 8.0).ceil() as usize;
    let rows = (rect.height() / 8.0).ceil() as usize;
    for x in 0..columns {
        for y in 0..rows {
            if (x + y) % 2 == 1 {
                let cell = Rect::from_min_size(
                    rect.min + Vec2::new(x as f32 * 8.0, y as f32 * 8.0),
                    Vec2::splat(8.0),
                );
                ui.painter()
                    .rect_filled(cell.intersect(rect), 0.0, colors[1]);
            }
        }
    }
}

pub(crate) fn strip(ui: &egui::Ui, rect: Rect, stops: &[Value]) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    checker(ui, rect);
    let colors = ordered(stops);
    // One small mesh, bounded independently of display scale and stop count.
    let mut mesh = egui::Mesh::default();
    let columns = (rect.width() * ui.ctx().pixels_per_point())
        .ceil()
        .clamp(1.0, 1024.0) as u32;
    for i in 0..=columns {
        let t = f64::from(i) / f64::from(columns);
        let x = egui::lerp(rect.x_range(), t as f32);
        let [r, g, b, a] = sample(&colors, t);
        let color = Color32::from_rgba_unmultiplied(r, g, b, a);
        mesh.colored_vertex(Pos2::new(x, rect.top()), color);
        mesh.colored_vertex(Pos2::new(x, rect.bottom()), color);
        if i > 0 {
            let v = i * 2;
            mesh.add_triangle(v - 2, v - 1, v);
            mesh.add_triangle(v, v - 1, v + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn handle_rect(ramp: Rect, stop: &Value) -> Rect {
    Rect::from_center_size(
        Pos2::new(
            egui::lerp(ramp.x_range(), offset(stop) as f32),
            ramp.bottom() + 9.0,
        ),
        Vec2::new(12.0, 18.0),
    )
}

fn hit(stops: &[Value], ramp: Rect, pointer: Pos2, selected: usize) -> Option<usize> {
    let contains = |i: usize| handle_rect(ramp, &stops[i]).expand(4.0).contains(pointer);
    // A selected coincident stop remains draggable when it crosses another stop.
    if selected < stops.len() && contains(selected) {
        return Some(selected);
    }
    (0..stops.len()).rev().find(|&i| contains(i))
}

pub(crate) fn editor(ui: &mut egui::Ui, gradient: &mut Value) {
    let original = gradient["stops"].clone();
    let mut stops = original
        .as_array()
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            vec![
                json!({"offset":0.0,"colorHex":"#FF000000"}),
                json!({"offset":1.0,"colorHex":"#FFFFFFFF"}),
            ]
        });
    let initial = stops.clone();
    let id = ui.make_persistent_id("gradient-editor");
    let mut state = ui
        .data_mut(|data| data.get_temp::<EditorState>(id))
        .unwrap_or_default();
    state.selected = state.selected.min(stops.len() - 1);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 80.0), Sense::hover());
    let response = ui.interact(rect, id.with("ramp"), Sense::click_and_drag());
    let ramp = Rect::from_min_max(
        rect.min + Vec2::splat(8.0),
        Pos2::new(rect.right() - 8.0, rect.top() + 48.0),
    );
    let position = |pointer: Pos2| {
        f64::from(((pointer.x - ramp.left()) / ramp.width().max(1.0)).clamp(0.0, 1.0))
    };
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(press) = ui.input(|input| input.pointer.press_origin())
        && let Some(index) = hit(&stops, ramp, press, state.selected)
    {
        state.pending_click = None;
        state.selected = index;
        state.drag = Some((index, press.x - handle_rect(ramp, &stops[index]).center().x));
        response.request_focus();
    }
    if response.dragged_by(egui::PointerButton::Primary)
        && let Some((index, grab)) = state.drag
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(stop) = stops.get_mut(index)
    {
        stop["offset"] = json!(position(pointer - Vec2::new(grab, 0.0)));
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.drag_stopped() || !ui.input(|input| input.pointer.primary_down()) {
        state.drag = None;
    }
    if response.double_clicked_by(egui::PointerButton::Primary)
        && let Some(pointer) = response.interact_pointer_pos()
        && hit(&stops, ramp, pointer, state.selected).is_none()
    {
        state.pending_click = None;
        add(&mut stops, position(pointer), &mut state);
        response.request_focus();
    } else if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(index) = hit(&stops, ramp, pointer, state.selected)
    {
        state.pending_click = None;
        state.selected = index;
        state.color = Some(index);
        state.closing = false;
        response.request_focus();
    }
    if response.clicked()
        && !response.double_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && hit(&stops, ramp, pointer, state.selected).is_none()
    {
        state.pending_click = Some((position(pointer), ui.input(|i| i.time)));
        state.closing = true;
    }
    if !ui.is_enabled() {
        state.pending_click = None;
    }
    if let Some((position, time)) = state.pending_click {
        if ui.input(|i| i.time) - time
            > ui.ctx().options(|o| o.input_options.max_double_click_delay)
        {
            let nearest = (0..stops.len())
                .min_by(|&a, &b| {
                    (offset(&stops[a]) - position)
                        .abs()
                        .total_cmp(&(offset(&stops[b]) - position).abs())
                })
                .unwrap();
            stops[nearest]["offset"] = json!(position);
            state.selected = nearest;
            state.pending_click = None;
            response.request_focus();
        } else {
            ui.ctx().request_repaint();
        }
    }
    if response.has_focus()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
    {
        let selected = state.selected;
        delete(&mut stops, selected, &mut state);
    }
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let index = hit(&stops, ramp, pointer, state.selected);
        state.menu = Some((index, position(pointer)));
        if let Some(index) = index {
            state.selected = index;
        }
    }
    buttons::context_menu(&response, |ui| {
        if let Some((index, position)) = state.menu {
            if let Some(index) = index {
                if ui.app_button("Change color…").clicked() {
                    state.color = Some(index);
                    state.closing = false;
                    ui.close();
                }
                if ui
                    .add_enabled(stops.len() > 1, Button::new("Delete color stop"))
                    .clicked()
                {
                    delete(&mut stops, index, &mut state);
                    ui.close();
                }
            } else if ui
                .add_enabled(stops.len() < MAX_STOPS, Button::new("Create color stop"))
                .clicked()
            {
                add(&mut stops, position, &mut state);
                ui.close();
            }
        }
    });
    if response.has_focus() {
        let step = ui.input(|input| if input.modifiers.shift { 0.1 } else { 0.01 });
        let direction = ui.input_mut(|input| {
            i8::from(
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                    || input.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowRight),
            ) - i8::from(
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                    || input.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowLeft),
            )
        });
        if direction != 0 {
            let stop = &mut stops[state.selected];
            stop["offset"] = json!((offset(stop) + f64::from(direction) * step).clamp(0.0, 1.0));
        }
    }
    strip(ui, ramp, &stops);
    for (index, stop) in stops.iter().enumerate() {
        let handle = handle_rect(ramp, stop);
        let ink = if index == state.selected {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().text_color()
        };
        let [r, g, b, a] = rgba(stop);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                handle.center_top(),
                Pos2::new(handle.right(), handle.top() + handle.width() * 0.5),
                handle.right_bottom(),
                handle.left_bottom(),
                Pos2::new(handle.left(), handle.top() + handle.width() * 0.5),
            ],
            Color32::from_rgba_unmultiplied(r, g, b, a),
            Stroke::new(if index == state.selected { 2.0 } else { 1.0 }, ink),
        ));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, ui.is_enabled(), "Gradient stops")
    });
    response.on_hover_text("Double-click empty space to add a stop. Click a stop to change its color. Drag to move it. Right-click the strip to add a stop, or a stop to change or delete it.");
    let selected = state.selected;
    buttons::bar(ui, 0.0, |ui| {
        let height = ui.spacing().interact_size.y;
        let image = crate::particles::glyph(
            ui.ctx(),
            "gradient-flip",
            include_bytes!("../assets/toolbar/gradient-flip.rgba"),
        );
        let response = ui.add_sized(
            Vec2::splat(height),
            Button::new(image)
                .bar()
                .id(id.with("flip"))
                .image_tint_follows_text_color(true),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                response.enabled(),
                "Reverse gradient",
            )
        });
        if response.on_hover_text("Reverse gradient").clicked() {
            for stop in &mut stops {
                stop["offset"] = json!(1.0 - offset(stop));
            }
            stops.reverse();
            state.selected = stops.len() - 1 - state.selected;
            state.drag = None;
            state.menu = None;
            state.color = None;
            state.picker = None;
            egui::Popup::close_id(ui.ctx(), id.with("color"));
        }
        let size = Vec2::new(ui.available_width() * 0.5, height);
        if ui
            .add_enabled_ui(stops.len() < MAX_STOPS, |ui| {
                ui.add_sized(
                    size,
                    Button::new("Add stop")
                        .bar()
                        .id(id.with("add-stop"))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                )
            })
            .inner
            .clicked()
        {
            add(&mut stops, 0.5, &mut state);
        }
        if ui
            .add_enabled_ui(stops.len() > 1, |ui| {
                ui.add_sized(
                    size,
                    Button::new("Delete stop")
                        .bar()
                        .id(id.with("delete-stop"))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                )
            })
            .inner
            .clicked()
        {
            delete(&mut stops, selected, &mut state);
        }
    });

    // The movable editor keeps its position while editing different stops.
    let color_id = id.with("color");
    if let Some(index) = state.color.filter(|&i| i < stops.len()) {
        let bytes = rgba(&stops[index]);
        // Keep hue/saturation while editing black or transparent colors.
        let mut color = state
            .picker
            .filter(|(i, previous, _)| *i == index && *previous == bytes)
            .map_or_else(
                || egui::ecolor::Hsva::from_srgba_unmultiplied(bytes),
                |(_, _, color)| color,
            );
        let mut open = !state.closing;
        if crate::color_picker::window(
            ui,
            color_id,
            "Color stop",
            handle_rect(ramp, &stops[index]),
            &mut open,
            &mut color,
        ) {
            set_color(&mut stops[index], color.to_srgba_unmultiplied());
        }
        state.picker = Some((index, rgba(&stops[index]), color));
        if !open {
            state.closing = true;
        }
    } else {
        state.color = None;
        egui::Popup::close_id(ui.ctx(), color_id);
    }
    ui.data_mut(|data| data.insert_temp(id, state));
    if stops != initial {
        gradient["stops"] = json!(stops);
    }
}
