//! Rotary parameter banks. Palette and small type come from Preferences.
use crate::{localization, theme};
use achimeld_application::Preferences;
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use serde_json::{Value, json};
use std::ops::RangeInclusive;

#[cfg(test)]
#[path = "knob_tests.rs"]
mod tests;

fn read(data: &Value, key: &str) -> Option<f64> {
    (if key.starts_with('/') {
        data.pointer(key)
    } else {
        data.get(key)
    })
    .and_then(Value::as_f64)
}

fn write(data: &mut Value, key: &str, value: Value) {
    let target = if key.starts_with('/') {
        data.pointer_mut(key)
    } else {
        data.get_mut(key)
    };
    *target.expect("declared knob property") = value;
}

pub(crate) struct Parameter<'a> {
    pub label: &'a str,
    pub key: &'a str,
    pub high: Option<&'a str>,
    pub bounds: RangeInclusive<f64>,
    pub factor: f64,
    pub suffix: &'a str,
    pub decimals: usize,
    pub icon: usize,
    pub unity: bool,
}

impl<'a> Parameter<'a> {
    pub fn new(label: &'a str, key: &'a str, bounds: RangeInclusive<f64>, icon: usize) -> Self {
        Self {
            label,
            key,
            high: None,
            bounds,
            factor: 1.0,
            suffix: "",
            decimals: 3,
            icon,
            unity: false,
        }
    }
    pub fn percent(mut self) -> Self {
        self.factor = 100.0;
        self.suffix = "%";
        self
    }
    pub fn pair(mut self, high: &'a str) -> Self {
        self.high = Some(high);
        self
    }
    pub fn integer(mut self) -> Self {
        self.decimals = 0;
        self
    }
    pub fn multiplier(mut self) -> Self {
        self.unity = true;
        self.suffix = "×";
        self.decimals = 2;
        self
    }
    pub fn ratio(mut self) -> Self {
        self.suffix = ":1";
        self.decimals = 2;
        self
    }
}

fn position(value: f64, bounds: &RangeInclusive<f64>, unity: bool) -> f64 {
    if unity {
        if value <= 1.0 {
            value / 2.0
        } else {
            0.5 + (value - 1.0) / 18.0
        }
    } else {
        (value - bounds.start()) / (bounds.end() - bounds.start())
    }
    .clamp(0.0, 1.0)
}

fn from_position(t: f64, bounds: &RangeInclusive<f64>, unity: bool) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if unity {
        if t <= 0.5 {
            t * 2.0
        } else {
            1.0 + (t - 0.5) * 18.0
        }
    } else {
        bounds.start() + t * (bounds.end() - bounds.start())
    }
}

fn arc(ui: &Ui, center: Pos2, radius: f32, start: f64, end: f64, stroke: Stroke) {
    if (end - start).abs() < 0.00001 {
        return;
    }
    let at = |t: f64| {
        let angle = (135.0 + 270.0 * t).to_radians() as f32;
        center + Vec2::angled(angle) * radius
    };
    let points = (0..=48)
        .map(|i| at(start + (end - start) * f64::from(i) / 48.0))
        .collect();
    ui.painter().add(egui::Shape::line(points, stroke));
    for t in [start, end] {
        ui.painter()
            .circle_filled(at(t), stroke.width * 0.5, stroke.color);
    }
}

fn face(ui: &Ui, center: Pos2, radius: f32, low: Color32, high: Color32, radial: bool) {
    let mut mesh = egui::Mesh::default();
    // Custom gradient meshes need their own antialiased perimeter. Feather by
    // one physical pixel, independent of Windows scaling, rather than exposing
    // the opaque triangle edge. Keep the artwork procedural at native density.
    let feather = ui.ctx().pixels_per_point().recip();
    const SEGMENTS: usize = 96;
    const RINGS: usize = 16;
    for ring in 0..=RINGS + 1 {
        let r = if ring > RINGS {
            radius + feather * 0.5
        } else {
            (radius - feather * 0.5) * ring as f32 / RINGS as f32
        };
        for segment in 0..=SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let point = center + Vec2::angled(angle) * r;
            let t = if radial {
                (point.distance(center - Vec2::new(0.0, radius * 0.4)) / (radius * 1.5))
                    .clamp(0.0, 1.0)
            } else {
                (point.y - center.y + radius) / (2.0 * radius)
            };
            let color = if ring > RINGS {
                Color32::TRANSPARENT
            } else {
                theme::mix(high, low, t)
            };
            mesh.colored_vertex(point, color);
            if ring > 0 && segment > 0 {
                let b = (ring * (SEGMENTS + 1) + segment) as u32;
                mesh.add_triangle(b - (SEGMENTS + 2) as u32, b - 1, b);
                mesh.add_triangle(b - (SEGMENTS + 2) as u32, b, b - (SEGMENTS + 1) as u32);
            }
        }
    }
    ui.painter().add(mesh);
}

/// A bank reserves its own layout; no shared label column or divider participates.
pub(crate) fn bank(
    ui: &mut Ui,
    p: &Preferences,
    data: &mut Value,
    defaults: &Value,
    params: &[Parameter<'_>],
) {
    if params.is_empty() {
        return;
    }
    let available = ui.available_width().max(1.0);
    let min_cell = (76.0 * (p.knob_value_size / 10.0).max(1.0)).min(available);
    let columns = ((available + 4.0) / (min_cell + 4.0)).floor().max(1.0) as usize;
    let cell = (available - 4.0 * (columns - 1) as f32) / columns as f32;
    let font = FontId::proportional(p.knob_label_size);
    let titles: Vec<_> = params
        .iter()
        .map(|s| {
            let slots = if s.high.is_some() { 2.min(columns) } else { 1 };
            ui.painter().layout(
                localization::text(ui.ctx(), s.label),
                font.clone(),
                theme::color(p, "Muted text"),
                (cell * slots as f32 + 4.0 * (slots - 1) as f32
                    - if s.high.is_some() { 28.0 } else { 8.0 })
                .max(1.0),
            )
        })
        .collect();
    let endpoint_height = ["Minimum", "Maximum"]
        .iter()
        .map(|text| {
            ui.painter()
                .layout(
                    localization::text(ui.ctx(), text),
                    font.clone(),
                    theme::color(p, "Muted text"),
                    (cell - 8.0).max(1.0),
                )
                .size()
                .y
        })
        .fold(0.0, f32::max)
        + 3.0;
    let stack_pairs = columns == 1;
    let header_height = params
        .iter()
        .zip(&titles)
        .map(|(s, title)| {
            if s.high.is_some() {
                title.size().y.max(16.0) + 3.0 + endpoint_height
            } else {
                19.0 + title.size().y
            }
        })
        .fold(0.0, f32::max);
    let height =
        8.0 + header_height + 5.0 + 45.0 + 5.0 + (p.knob_value_size + 10.0).max(24.0) + 6.0;
    let pair_step = 45.0 + 5.0 + (p.knob_value_size + 10.0).max(24.0) + endpoint_height;
    let mut col = 0;
    let mut row_top = 0.0;
    let mut row_height = height;
    let mut placements = Vec::new();
    for param in params {
        let slots = if param.high.is_some() {
            2.min(columns)
        } else {
            1
        };
        if col + slots > columns {
            row_top += row_height + 4.0;
            row_height = height;
            col = 0;
        }
        let tile_height = height
            + if stack_pairs && param.high.is_some() {
                pair_step
            } else {
                0.0
            };
        row_height = row_height.max(tile_height);
        placements.push((col, row_top, slots, tile_height));
        col += slots;
    }
    let (area, _) =
        ui.allocate_exact_size(Vec2::new(available, row_top + row_height), Sense::hover());
    let atlas = icon_atlas(ui);
    for ((s, title), (col, row_top, slots, tile_height)) in
        params.iter().zip(titles).zip(placements)
    {
        let rect = Rect::from_min_size(
            area.min + Vec2::new(col as f32 * (cell + 4.0), row_top),
            Vec2::new(cell * slots as f32 + 4.0 * (slots - 1) as f32, tile_height),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(s.key).max_rect(rect));
        let hover = ui.rect_contains_pointer(rect);
        let panel = theme::color(p, "Panel");
        child.painter().rect_filled(
            rect,
            0.0,
            if hover {
                theme::mix(panel, theme::color(p, "Primary text"), 0.025)
            } else {
                panel
            },
        );
        // A range's icon and common name share the first header row; endpoint
        // names use the second. This keeps ordinary cells compact below values.
        let range_header_height = title.size().y.max(16.0);
        let group_left = rect.center().x - (20.0 + title.size().x) / 2.0;
        let icon_center = if s.high.is_some() {
            Pos2::new(
                group_left + 8.0,
                rect.top() + 8.0 + range_header_height / 2.0,
            )
        } else {
            Pos2::new(rect.center().x, rect.top() + 16.0)
        };
        let title_position = if s.high.is_some() {
            Pos2::new(
                group_left + 20.0,
                rect.top() + 8.0 + (range_header_height - title.size().y) / 2.0,
            )
        } else {
            Pos2::new(rect.center().x - title.size().x / 2.0, rect.top() + 27.0)
        };
        let icon = Rect::from_center_size(icon_center, Vec2::splat(16.0));
        let uv = Rect::from_min_max(
            egui::pos2((s.icon % 4) as f32 / 4.0, (s.icon / 4) as f32 / 4.0),
            egui::pos2((s.icon % 4 + 1) as f32 / 4.0, (s.icon / 4 + 1) as f32 / 4.0),
        );
        child.painter().image(
            atlas.id(),
            icon,
            uv,
            theme::color(p, if hover { "Primary text" } else { "Muted text" }),
        );
        child
            .painter()
            .galley(title_position, title, theme::color(p, "Muted text"));
        let dial_top = rect.top() + 8.0 + header_height + 5.0;
        let keys: Vec<_> = std::iter::once(s.key).chain(s.high).collect();
        for (index, key) in keys.iter().enumerate() {
            let width = rect.width() / if stack_pairs { 1.0 } else { keys.len() as f32 };
            let x = rect.left() + width * (if stack_pairs { 0.5 } else { index as f32 + 0.5 });
            let top = dial_top
                + if stack_pairs {
                    index as f32 * pair_step
                } else {
                    0.0
                };
            let mut value = read(data, key).unwrap_or(*s.bounds.start());
            let default = read(defaults, key).unwrap_or(*s.bounds.start());
            let label = localization::text(ui.ctx(), s.label);
            let endpoint = if s.high.is_some() {
                localization::text(ui.ctx(), if index == 0 { "Minimum" } else { "Maximum" })
            } else {
                String::new()
            };
            let accessible = format!("{label} {endpoint}").trim().to_owned();
            if s.high.is_some() {
                let galley = child.painter().layout(
                    endpoint,
                    font.clone(),
                    theme::color(p, "Muted text"),
                    (width - 8.0).max(1.0),
                );
                child.painter().galley(
                    Pos2::new(
                        x - galley.size().x / 2.0,
                        if stack_pairs && index > 0 {
                            top - endpoint_height
                        } else {
                            rect.top() + 8.0 + range_header_height + 3.0
                        },
                    ),
                    galley,
                    theme::color(p, "Muted text"),
                );
            }
            let dial = Rect::from_center_size(
                Pos2::new(x, top + 22.5),
                Vec2::splat(45.0_f32.min(width - 4.0)),
            );
            let response = rotary(
                &mut child,
                p,
                dial,
                key,
                &accessible,
                &mut value,
                default,
                s,
                if let Some(high) = s.high {
                    if index == 0 {
                        *s.bounds.start()..=read(data, high).unwrap_or(*s.bounds.end())
                    } else {
                        read(data, s.key).unwrap_or(*s.bounds.start())..=*s.bounds.end()
                    }
                } else {
                    s.bounds.clone()
                },
            );
            let mut changed = response.changed();
            let number_rect = Rect::from_center_size(
                Pos2::new(
                    x,
                    top + 45.0 + 5.0 + (p.knob_value_size + 10.0).max(24.0) / 2.0,
                ),
                Vec2::new((width - 8.0).max(1.0), (p.knob_value_size + 10.0).max(24.0)),
            );
            let mut number_ui = child.new_child(
                egui::UiBuilder::new()
                    .id_salt((*key, "number"))
                    .max_rect(number_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            number_ui.style_mut().override_font_id = Some(FontId::monospace(p.knob_value_size));
            number_ui.spacing_mut().interact_size = Vec2::new(1.0, number_rect.height());
            number_ui.spacing_mut().button_padding = Vec2::new(2.0, 2.0);
            let mut display = value * s.factor;
            let number = number_ui.add_sized(
                number_rect.size(),
                egui::DragValue::new(&mut display)
                    .speed(if s.decimals == 0 { 1.0 } else { 0.1 })
                    .range(*s.bounds.start() * s.factor..=*s.bounds.end() * s.factor)
                    .max_decimals(s.decimals)
                    .suffix(s.suffix),
            );
            number.widget_info(|| egui::WidgetInfo::slider(number.enabled(), display, &accessible));
            if number.changed() && display.is_finite() {
                value = display / s.factor;
                changed = true;
            }
            if changed {
                let quantum = 10.0_f64.powi(s.decimals as i32) * s.factor;
                value = (value * quantum).round() / quantum;
                value = value.clamp(*s.bounds.start(), *s.bounds.end());
                if let Some(high) = s.high {
                    if index == 0 {
                        value = value.min(read(data, high).unwrap_or(value));
                    } else {
                        value = value.max(read(data, s.key).unwrap_or(value));
                    }
                }
                write(
                    data,
                    key,
                    if s.decimals == 0 {
                        json!(value as u32)
                    } else {
                        json!(value)
                    },
                );
            }
        }
    }
}

fn icon_atlas(ui: &Ui) -> egui::TextureHandle {
    let id = egui::Id::new("rotary-parameter-icons");
    if let Some(t) = ui.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [256, 256],
        include_bytes!("../assets/knob-icons.rgba"),
    );
    let t = ui.ctx().load_texture(
        "Knob icons",
        image,
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    ui.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

#[allow(clippy::too_many_arguments)]
fn rotary(
    ui: &mut Ui,
    p: &Preferences,
    rect: Rect,
    key: &str,
    label: &str,
    value: &mut f64,
    default: f64,
    s: &Parameter<'_>,
    limits: RangeInclusive<f64>,
) -> Response {
    let id = ui.id().with((key, "dial"));
    let mut r = ui
        .interact(rect, id, Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let previous = *value;
    if r.enabled() {
        if r.clicked() || r.drag_started() {
            r.request_focus();
        }
        let fine = ui.input(|i| i.modifiers.shift);
        if r.dragged() {
            let delta = ui.input(|i| i.pointer.delta());
            let carry_id = id.with("drag-position");
            let t = ui
                .data(|d| d.get_temp::<f64>(carry_id))
                .unwrap_or_else(|| position(*value, &s.bounds, s.unity));
            let t = (t + f64::from(delta.x - delta.y) / if fine { 2000.0 } else { 200.0 })
                .clamp(0.0, 1.0);
            ui.data_mut(|d| d.insert_temp(carry_id, t));
            *value = from_position(t, &s.bounds, s.unity);
        }
        if !r.dragged() {
            ui.data_mut(|d| d.remove::<f64>(id.with("drag-position")));
        }
        if r.double_clicked() {
            *value = default;
        }
        if r.has_focus() {
            let mut delta = 0.0;
            ui.input_mut(|i| {
                let modifiers = i.modifiers;
                for (key, step) in [
                    (egui::Key::ArrowUp, 0.01),
                    (egui::Key::ArrowRight, 0.01),
                    (egui::Key::ArrowDown, -0.01),
                    (egui::Key::ArrowLeft, -0.01),
                    (egui::Key::PageUp, 0.1),
                    (egui::Key::PageDown, -0.1),
                ] {
                    if i.consume_key(modifiers, key) {
                        delta += step;
                    }
                }
                if r.hovered() {
                    delta += f64::from(i.smooth_scroll_delta.y) / 1000.0;
                    i.smooth_scroll_delta = Vec2::ZERO;
                }
                if i.consume_key(modifiers, egui::Key::Home) {
                    *value = *s.bounds.start();
                }
                if i.consume_key(modifiers, egui::Key::End) {
                    *value = *s.bounds.end();
                }
                if i.consume_key(modifiers, egui::Key::Delete)
                    || i.consume_key(modifiers, egui::Key::Backspace)
                {
                    *value = default;
                }
            });
            if delta != 0.0 {
                *value = from_position(
                    position(*value, &s.bounds, s.unity) + delta / if fine { 10.0 } else { 1.0 },
                    &s.bounds,
                    s.unity,
                );
            }
        }
    }
    if *value != previous {
        let unrounded = *value;
        let quantum = 10.0_f64.powi(s.decimals as i32) * s.factor;
        *value = ((*value * quantum).round() / quantum).clamp(*limits.start(), *limits.end());
        if r.dragged() {
            ui.data_mut(|d| {
                d.insert_temp(
                    id.with("drag-position"),
                    position(
                        unrounded.clamp(*limits.start(), *limits.end()),
                        &s.bounds,
                        s.unity,
                    ),
                )
            });
        }
    }
    if *value != previous {
        r.mark_changed();
    }
    r.widget_info(|| egui::WidgetInfo::slider(r.enabled(), *value * s.factor, label));
    let control = theme::color(p, "Control");
    let window = theme::color(p, "Window");
    let text = theme::color(p, "Primary text");
    let accent = theme::color(p, "Accent");
    let tint = |c| {
        if r.enabled() {
            c
        } else {
            theme::mix(theme::color(p, "Panel"), c, 0.35)
        }
    };
    let c = rect.center();
    let scale = rect.width() / 45.0;
    let t = position(*value, &s.bounds, s.unity);
    // Only programmatic changes ease; pointer adjustment has no animation lag.
    let animated = ui.ctx().animate_value_with_time(
        id.with("angle"),
        t as f32,
        if r.dragged() { 0.0 } else { 0.16 },
    );
    arc(
        ui,
        c,
        20.0 * scale,
        0.0,
        1.0,
        Stroke::new(3.0 * scale, tint(theme::mix(control, window, 0.4))),
    );
    let origin = if *s.bounds.start() < 0.0 && *s.bounds.end() > 0.0 {
        position(0.0, &s.bounds, s.unity)
    } else {
        0.0
    };
    arc(
        ui,
        c,
        20.0 * scale,
        origin,
        f64::from(animated),
        Stroke::new(3.0 * scale, tint(accent.gamma_multiply(0.10))),
    );
    arc(
        ui,
        c,
        20.0 * scale,
        origin,
        f64::from(animated),
        Stroke::new(1.5 * scale, tint(accent)),
    );
    ui.painter().circle_filled(
        c + Vec2::new(0.0, 1.5 * scale),
        15.8 * scale,
        window.gamma_multiply(0.5),
    );
    face(
        ui,
        c,
        15.5 * scale,
        tint(window),
        tint(theme::mix(control, text, 0.25)),
        false,
    );
    face(
        ui,
        c,
        14.75 * scale,
        tint(theme::mix(control, window, 0.45)),
        tint(theme::mix(
            control,
            text,
            if r.dragged() {
                0.25
            } else if r.hovered() {
                0.20
            } else {
                0.15
            },
        )),
        true,
    );
    let direction = Vec2::angled((135.0 + 270.0 * animated).to_radians());
    ui.painter().line_segment(
        [c + direction * 6.0 * scale, c + direction * 11.5 * scale],
        Stroke::new(2.5 * scale, tint(text)),
    );
    if r.has_focus() {
        ui.painter()
            .circle_stroke(c, 22.0 * scale, Stroke::new(1.0, tint(accent)));
    }
    r.on_hover_text(format!("{label}\n{}{}", if key == "regularity" { "0: equal spacing; 1: fully random.\n" } else { "" }, "Drag up/right to increase. Shift: fine adjustment. Click to focus for wheel and arrow keys. Double-click or Delete: reset."))
}
