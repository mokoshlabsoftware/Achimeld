//! Compact, movable color controls. Geometry never depends on channel values.
use egui::{
    Color32, Pos2, Rect, Sense, Stroke, Vec2,
    ecolor::{Hsva, HsvaGamma},
};

pub(crate) const GRAYSCALE: &str = "mask-grayscale";

pub(crate) fn grayscale_scope(ui: &egui::Ui) -> bool {
    ui.stack()
        .iter()
        .any(|entry| entry.tags().contains(GRAYSCALE))
}

fn grayscale_picker(ui: &mut egui::Ui, color: &mut Hsva) -> bool {
    let original = color.to_srgba_unmultiplied();
    let mut rgba = achimeld_application::masks::grayscale(original);
    let width = 224.0_f32.min((ui.ctx().content_rect().width() - 48.0).max(160.0));
    ui.set_width(width);
    let mut gray = f64::from(rgba[0]);
    let mut alpha = f64::from(rgba[3]);
    crate::opacity::value_slider(ui, "Gray", &mut gray, 0.0..=255.0, "", 0);
    crate::opacity::value_slider(ui, "Alpha", &mut alpha, 0.0..=255.0, "", 0);
    rgba[..3].fill(gray.round() as u8);
    rgba[3] = alpha.round() as u8;
    let hex_id = ui.make_persistent_id("grayscale-hex");
    let (previous, mut hex) = ui
        .data(|data| data.get_temp::<([u8; 4], String)>(hex_id))
        .unwrap_or((rgba, encode_hex(rgba)));
    if previous != rgba {
        hex = encode_hex(rgba);
    }
    if crate::fields::row(ui, "Hex", |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut hex)
                .id(hex_id)
                .char_limit(9),
        )
        .changed()
    }) && let Some(bytes) = decode_hex(&hex)
    {
        rgba = achimeld_application::masks::grayscale(bytes);
        hex = encode_hex(rgba);
    }
    ui.data_mut(|data| data.insert_temp(hex_id, (rgba, hex)));
    let (swatch, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
    crate::gradient::checker(ui, swatch);
    ui.painter().rect_filled(
        swatch,
        0.0,
        Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]),
    );
    ui.weak(crate::localization::text(
        ui.ctx(),
        "White reveals. Black hides.",
    ));
    *color = Hsva::from_srgba_unmultiplied(rgba);
    original != rgba
}

fn ink(ui: &egui::Ui, rect: Rect, pos: Pos2) {
    let p = pos.clamp(rect.min, rect.max);
    ui.painter()
        .circle_stroke(p, 6.0, Stroke::new(3.0, Color32::BLACK));
    ui.painter()
        .circle_stroke(p, 6.0, Stroke::new(1.5, Color32::WHITE));
}

fn strip(ui: &mut egui::Ui, id: &str, width: f32, value: &mut f32, color: impl Fn(f32) -> Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.2), Sense::hover());
    let response = ui.interact(rect, ui.make_persistent_id(id), Sense::click_and_drag());
    if let Some(p) = response.interact_pointer_pos() {
        *value = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        response.request_focus();
    }
    nudge(ui, &response, value);
    crate::gradient::checker(ui, rect);
    let mut mesh = egui::Mesh::default();
    for i in 0..=72 {
        let t = i as f32 / 72.0;
        let x = egui::lerp(rect.x_range(), t);
        mesh.colored_vertex(Pos2::new(x, rect.top()), color(t));
        mesh.colored_vertex(Pos2::new(x, rect.bottom()), color(t));
        if i > 0 {
            let v = i * 2;
            mesh.add_triangle(v - 2, v - 1, v);
            mesh.add_triangle(v, v - 1, v + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    ink(
        ui,
        rect,
        Pos2::new(egui::lerp(rect.x_range(), *value), rect.center().y),
    );
    response.on_hover_text(id);
}

fn nudge(ui: &mut egui::Ui, response: &egui::Response, value: &mut f32) {
    if response.has_focus() {
        let delta = ui.input_mut(|i| {
            i8::from(i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
                - i8::from(i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
        });
        *value = (*value + f32::from(delta) / 255.0).clamp(0.0, 1.0);
    }
}

pub(crate) fn picker(ui: &mut egui::Ui, color: &mut Hsva) -> bool {
    let original = *color;
    let font = ui.text_style_height(&egui::TextStyle::Body);
    let width =
        ((font * 22.0).max(320.0) * 0.7).min((ui.ctx().content_rect().width() - 48.0).max(160.0));
    ui.set_width(width);
    ui.spacing_mut().interact_size.y = (ui.spacing().interact_size.y * 0.7).max(font + 4.0);
    let mut rgba = color.to_srgba_unmultiplied();
    let mut changed = false;
    for (i, label) in ["Red", "Green", "Blue", "Alpha"].into_iter().enumerate() {
        changed |= crate::fields::row(ui, label, |ui| {
            ui.add_sized(
                [ui.available_width(), ui.spacing().interact_size.y],
                egui::DragValue::new(&mut rgba[i]).range(0..=255).speed(0.5),
            )
            .changed()
        });
    }
    if changed {
        let next = Hsva::from_srgba_unmultiplied(rgba);
        // Keep the selected hue when numeric edits produce gray or black.
        *color = Hsva {
            h: if next.s == 0.0 { color.h } else { next.h },
            ..next
        };
    }
    // The color editor is the explicit exception to the application's swatch-only fields.
    let hex_id = ui.make_persistent_id("color-hex");
    let bytes = color.to_srgba_unmultiplied();
    let (previous, mut hex) = ui
        .data(|data| data.get_temp::<([u8; 4], String)>(hex_id))
        .unwrap_or((bytes, encode_hex(bytes)));
    if previous != bytes {
        hex = encode_hex(bytes);
    }
    let edited = crate::fields::row(ui, "Hex", |ui| {
        ui.add_sized(
            [ui.available_width(), ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut hex)
                .id(hex_id)
                .char_limit(9),
        )
        .on_hover_text("#RRGGBB or #AARRGGBB")
        .changed()
    });
    if edited && let Some(bytes) = decode_hex(&hex) {
        *color = Hsva::from_srgba_unmultiplied(bytes);
    }
    ui.data_mut(|data| data.insert_temp(hex_id, (color.to_srgba_unmultiplied(), hex)));
    let mut hsv = HsvaGamma::from(*color);
    let (swatch, _) = ui.allocate_exact_size(Vec2::new(width, 18.2), Sense::hover());
    crate::gradient::checker(ui, swatch);
    ui.painter().rect_filled(swatch, 0.0, Color32::from(*color));
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(width), Sense::hover());
    let response = ui.interact(
        rect,
        ui.make_persistent_id("saturation-value"),
        Sense::click_and_drag(),
    );
    if let Some(p) = response.interact_pointer_pos() {
        hsv.s = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        hsv.v = (1.0 - (p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        response.request_focus();
    }
    nudge(ui, &response, &mut hsv.s);
    if response.has_focus() {
        let delta = ui.input_mut(|i| {
            i8::from(i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
                - i8::from(i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        });
        hsv.v = (hsv.v + f32::from(delta) / 255.0).clamp(0.0, 1.0);
    }
    let mut mesh = egui::Mesh::default();
    const N: u32 = 24;
    for y in 0..=N {
        for x in 0..=N {
            let s = x as f32 / N as f32;
            let v = 1.0 - y as f32 / N as f32;
            mesh.colored_vertex(
                Pos2::new(
                    egui::lerp(rect.x_range(), s),
                    egui::lerp(rect.y_range(), 1.0 - v),
                ),
                Color32::from(HsvaGamma {
                    s,
                    v,
                    a: 1.0,
                    ..hsv
                }),
            );
            if x < N && y < N {
                let i = y * (N + 1) + x;
                mesh.add_triangle(i, i + 1, i + N + 1);
                mesh.add_triangle(i + 1, i + N + 2, i + N + 1);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    ink(
        ui,
        rect,
        Pos2::new(
            egui::lerp(rect.x_range(), hsv.s),
            egui::lerp(rect.y_range(), 1.0 - hsv.v),
        ),
    );
    response.on_hover_text("Saturation and brightness");
    strip(ui, "Hue", width, &mut hsv.h, |h| {
        Color32::from(Hsva {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        })
    });
    let opaque = HsvaGamma { a: 1.0, ..hsv };
    strip(ui, "Opacity", width, &mut hsv.a, |a| {
        Color32::from(HsvaGamma { a, ..opaque })
    });
    *color = Hsva::from(hsv);
    color.to_srgba_unmultiplied() != original.to_srgba_unmultiplied()
}

fn encode_hex(bytes: [u8; 4]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        bytes[3], bytes[0], bytes[1], bytes[2]
    )
}
fn decode_hex(text: &str) -> Option<[u8; 4]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if !text.is_ascii() || !matches!(text.len(), 6 | 8) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some([
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
        if text.len() == 8 {
            (value >> 24) as u8
        } else {
            255
        },
    ])
}

pub(crate) fn window(
    ui: &egui::Ui,
    id: egui::Id,
    title: &str,
    anchor: Rect,
    open: &mut bool,
    color: &mut Hsva,
) -> bool {
    if *open && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        *open = false;
    }
    let mut changed = false;
    let grayscale = grayscale_scope(ui);
    egui::Window::new(crate::localization::text(ui.ctx(), title))
        .id(id)
        .open(open)
        .default_pos(egui::pos2(anchor.right() + 8.0, anchor.top()))
        .movable(true)
        .resizable(false)
        .collapsible(false)
        .auto_sized()
        .min_width(0.0)
        .min_height(0.0)
        .show(ui.ctx(), |ui| {
            crate::fields::fixed_width(ui, |ui| {
                changed = if grayscale {
                    grayscale_picker(ui, color)
                } else {
                    picker(ui, color)
                };
            })
        });
    changed
}

pub(crate) fn edit_button(ui: &mut egui::Ui, rgba: &mut [u8; 4]) -> egui::Response {
    edit_button_sized(ui, rgba, 40.0)
}

pub(crate) fn edit_button_sized(
    ui: &mut egui::Ui,
    rgba: &mut [u8; 4],
    width: f32,
) -> egui::Response {
    let grayscale = grayscale_scope(ui);
    let shown = if grayscale {
        achimeld_application::masks::grayscale(*rgba)
    } else {
        *rgba
    };
    let mut response = ui.add(
        crate::buttons::Button::new("").min_size(Vec2::new(width, ui.spacing().interact_size.y)),
    );
    let swatch = response.rect.shrink(4.0);
    crate::gradient::checker(ui, swatch);
    ui.painter().rect_filled(
        swatch,
        0.0,
        Color32::from_rgba_unmultiplied(shown[0], shown[1], shown[2], shown[3]),
    );
    let cache = response.id.with("color-hue");
    let mut color = ui
        .data(|d| d.get_temp::<([u8; 4], Hsva)>(cache))
        .filter(|(previous, _)| previous == rgba)
        .map_or_else(|| Hsva::from_srgba_unmultiplied(shown), |(_, color)| color);
    let open_id = response.id.with("color-open");
    let mut open = ui
        .data(|data| data.get_temp::<bool>(open_id))
        .unwrap_or(false)
        || response.clicked();
    if window(
        ui,
        response.id.with("color-popup"),
        "Color",
        response.rect,
        &mut open,
        &mut color,
    ) {
        *rgba = color.to_srgba_unmultiplied();
        response.mark_changed();
    }
    ui.data_mut(|data| data.insert_temp(open_id, open));
    ui.data_mut(|d| d.insert_temp(cache, (*rgba, color)));
    response.on_hover_text("Change color")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mask_color_editor_is_grayscale_and_scoped_to_its_property_pane() {
        let ctx = egui::Context::default();
        let mut color = Hsva::from_srgba_unmultiplied([255, 0, 0, 128]);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .ui_stack_info(egui::UiStackInfo::default().with_tag(GRAYSCALE)),
                |ui| {
                    assert!(grayscale_scope(ui));
                    assert!(grayscale_picker(ui, &mut color));
                },
            );
            assert!(!grayscale_scope(ui));
        });
        let bytes = color.to_srgba_unmultiplied();
        assert_eq!(bytes, [54, 54, 54, 128]);
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.text())
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.contains(&"Gray"));
        assert!(!labels.contains(&"Red"));
        assert!(!labels.contains(&"Hue"));
        output.textures_delta.clear();
    }
    #[test]
    fn color_editor_hex_accepts_rgb_and_alpha_without_mutating_invalid_input() {
        assert_eq!(decode_hex("#CC99DD"), Some([204, 153, 221, 255]));
        assert_eq!(decode_hex("80402010"), Some([64, 32, 16, 128]));
        for text in ["#XYZXYZ", "#12345", "#123456789", "#💜0000"] {
            assert_eq!(decode_hex(text), None);
        }
        for bytes in [[0; 4], [255; 4], [204, 153, 221, 128]] {
            assert_eq!(decode_hex(&encode_hex(bytes)), Some(bytes));
        }
    }
    #[test]
    fn color_window_drags_by_its_title_without_changing_the_color() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("draggable-color-editor-test");
        let mut open = true;
        let mut color = Hsva::from_srgba_unmultiplied([204, 149, 209, 255]);
        let original = color;
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
                    window(
                        ui,
                        id,
                        "Color stop",
                        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::splat(10.0)),
                        &mut open,
                        &mut color,
                    );
                },
            );
            output.textures_delta.clear();
            ctx.memory(|memory| memory.area_rect(id)).unwrap()
        };
        frame(vec![]);
        let initial = frame(vec![]);
        let press = initial.left_top() + Vec2::new(35.0, 12.0);
        frame(vec![egui::Event::PointerMoved(press)]);
        frame(vec![egui::Event::PointerButton {
            pos: press,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        let moved = press + Vec2::new(90.0, 55.0);
        frame(vec![egui::Event::PointerMoved(moved)]);
        let result = frame(vec![egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(
            (result.min - initial.min - Vec2::new(90.0, 55.0)).length() < 2.0,
            "{initial:?} -> {result:?}"
        );
        drop(frame);
        assert_eq!(
            color.to_srgba_unmultiplied(),
            original.to_srgba_unmultiplied()
        );
        assert!(open);
    }

    #[test]
    fn gradient_color_picker_geometry_is_fixed_across_channel_digits_and_scales() {
        for scale in [1.0, 1.5, 2.0] {
            for font in [13.0, 20.0] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                let mut previous = None;
                for rgba in [[0, 0, 0, 0], [9, 10, 99, 100], [255; 4], [143, 96, 96, 255]] {
                    let mut bounds = Rect::NOTHING;
                    let mut expected_width = 0.0;
                    let mut color = Hsva::from_srgba_unmultiplied(rgba);
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(1000.0, 1000.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            crate::theme::apply(
                                ui.ctx(),
                                &achimeld_application::Preferences {
                                    normal_size: font,
                                    ..Default::default()
                                },
                            );
                            ui.set_style(ui.ctx().global_style());
                            expected_width = (ui.text_style_height(&egui::TextStyle::Body) * 22.0)
                                .max(320.0)
                                * 0.7;
                            picker(ui, &mut color);
                            bounds = ui
                                .ctx()
                                .read_response(ui.make_persistent_id("saturation-value"))
                                .unwrap()
                                .rect;
                        },
                    );
                    output.textures_delta.clear();
                    assert!(
                        (bounds.width() - expected_width).abs() < 1.0,
                        "{} vs {expected_width}",
                        bounds.width()
                    );
                    assert_eq!(bounds.width(), bounds.height());
                    if let Some(previous) = previous {
                        assert_eq!(bounds, previous);
                    }
                    previous = Some(bounds);
                }
            }
        }
    }
}
