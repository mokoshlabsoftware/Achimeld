use egui::{Pos2, Response, Sense, Stroke, Vec2};
use serde_json::{Value, json};

#[allow(clippy::too_many_arguments)]
pub(crate) fn scalar(
    ui: &mut egui::Ui,
    label: &str,
    settings: &mut Value,
    key: &str,
    bounds: std::ops::RangeInclusive<f64>,
    integer: bool,
    factor: f64,
) {
    let mut value = settings[key].as_f64().unwrap_or(*bounds.start()) * factor;
    let suffix = if factor == 100.0 {
        "%"
    } else if matches!(key, "angle" | "rotation") {
        "°"
    } else {
        ""
    };
    if crate::opacity::value_slider(
        ui,
        label,
        &mut value,
        *bounds.start() * factor..=*bounds.end() * factor,
        suffix,
        if integer { 0 } else { 3 },
    ) {
        settings[key] = if integer {
            json!((value / factor).round() as u64)
        } else {
            json!(value / factor)
        };
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn range(
    ui: &mut egui::Ui,
    label: &str,
    settings: &mut Value,
    min_key: &str,
    max_key: &str,
    bounds: std::ops::RangeInclusive<f64>,
    factor: f64,
    suffix: &str,
) {
    let mut high = settings[max_key].as_f64().unwrap_or(*bounds.end());
    let mut low = settings[min_key].as_f64().unwrap_or(high);
    crate::inspector::field(ui, label, 170.0, |ui| {
        let text = if factor == 100.0 || bounds.end() - bounds.start() > 10.0 {
            format!("{:.0}–{:.0}{suffix}", low * factor, high * factor)
        } else {
            format!("{:.2}–{:.2}{suffix}", low * factor, high * factor)
        };
        let text_width = ui
            .painter()
            .layout_no_wrap(
                text.clone(),
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x;
        let changed = if ui.available_width() >= text_width + ui.spacing().item_spacing.x + 60.0 {
            ui.horizontal(|ui| {
                let width = ui.available_width() - text_width - ui.spacing().item_spacing.x;
                let changed =
                    range_track(ui, label, &mut low, &mut high, bounds.clone(), width).changed();
                ui.label(&text);
                changed
            })
            .inner
        } else {
            ui.vertical(|ui| {
                let width = ui.available_width();
                let changed =
                    range_track(ui, label, &mut low, &mut high, bounds.clone(), width).changed();
                ui.add(egui::Label::new(egui::RichText::new(&text).weak()).truncate())
                    .on_hover_text(&text);
                changed
            })
            .inner
        };
        if changed {
            settings[min_key] = json!(low);
            settings[max_key] = json!(high);
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RangePart {
    Low,
    High,
    Bar,
}

#[derive(Clone, Copy)]
struct RangeDrag {
    part: RangePart,
    pointer_x: f32,
    low: f64,
    high: f64,
}

const KNOB_RADIUS: f32 = 5.0;
const KNOB_HIT_RADIUS: f32 = 7.0;

fn range_part(pos: Pos2, a: f32, b: f32, y: f32) -> RangePart {
    let near_low = pos.distance(Pos2::new(a, y)) <= KNOB_HIT_RADIUS;
    let near_high = pos.distance(Pos2::new(b, y)) <= KNOB_HIT_RADIUS;
    let nearer_high = if (a - b).abs() < 1.0 {
        pos.x >= a
    } else {
        (pos.x - b).abs() < (pos.x - a).abs()
    };
    if !near_low && !near_high && pos.x > a && pos.x < b && (pos.y - y).abs() <= 6.0 {
        RangePart::Bar
    } else if nearer_high {
        RangePart::High
    } else {
        RangePart::Low
    }
}

fn shift_range(low: f64, high: f64, delta: f64, min: f64, max: f64) -> (f64, f64) {
    let delta = delta.clamp(min - low, max - high);
    (low + delta, high + delta)
}

pub(crate) fn range_track(
    ui: &mut egui::Ui,
    label: &str,
    low: &mut f64,
    high: &mut f64,
    bounds: std::ops::RangeInclusive<f64>,
    width: f32,
) -> Response {
    let height = ui.spacing().interact_size.y.max(22.0);
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width.max(24.0), height), Sense::click_and_drag());
    let track = rect.shrink2(egui::vec2(7.0, 0.0));
    let (min, max) = (*bounds.start(), *bounds.end());
    let to_x = |value: f64| track.left() + ((value - min) / (max - min)) as f32 * track.width();
    let y = rect.center().y;
    let selected_id = response.id.with("range-part");
    let drag_id = response.id.with("range-drag");
    let mut drag = ui.ctx().data(|data| data.get_temp::<RangeDrag>(drag_id));
    let mut selected = ui
        .ctx()
        .data(|data| data.get_temp::<RangePart>(selected_id))
        .unwrap_or(RangePart::Low);
    let pressed = response.is_pointer_button_down_on()
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    if response.enabled()
        && (pressed || (drag.is_none() && (response.clicked() || response.drag_started())))
    {
        response.request_focus();
        if let Some(pos) = ui
            .input(|i| i.pointer.press_origin())
            .or(response.interact_pointer_pos())
        {
            let a = to_x(*low);
            let b = to_x(*high);
            selected = range_part(pos, a, b, y);
            let mut start = RangeDrag {
                part: selected,
                pointer_x: pos.x,
                low: *low,
                high: *high,
            };
            // Empty-track clicks move the nearest end; grabbing a knob keeps its offset.
            let value = min + f64::from((pos.x - track.left()) / track.width()) * (max - min);
            match selected {
                RangePart::Low if pos.distance(Pos2::new(a, y)) > KNOB_HIT_RADIUS => {
                    start.low = value.clamp(min, *high);
                }
                RangePart::High if pos.distance(Pos2::new(b, y)) > KNOB_HIT_RADIUS => {
                    start.high = value.clamp(*low, max);
                }
                _ => {}
            }
            ui.ctx().data_mut(|data| {
                data.insert_temp(selected_id, selected);
                data.insert_temp(drag_id, start);
            });
            drag = Some(start);
        }
    }
    let mut next = (*low, *high);
    if response.enabled()
        && (response.is_pointer_button_down_on() || response.clicked() || response.drag_stopped())
        && let Some(start) = drag
        && let Some(pos) = response.interact_pointer_pos()
    {
        let delta = f64::from((pos.x - start.pointer_x) / track.width()) * (max - min);
        next = match start.part {
            RangePart::Low => ((start.low + delta).clamp(min, start.high), start.high),
            RangePart::High => (start.low, (start.high + delta).clamp(start.low, max)),
            RangePart::Bar => shift_range(start.low, start.high, delta, min, max),
        };
    }
    if !response.enabled() || !ui.input(|i| i.pointer.primary_down()) {
        ui.ctx().data_mut(|data| data.remove::<RangeDrag>(drag_id));
    }
    if response.enabled() && response.has_focus() {
        let step = (max - min)
            / if ui.input(|i| i.modifiers.shift) {
                10.0
            } else {
                100.0
            };
        let left = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
        let right = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight));
        let delta = (i32::from(right) - i32::from(left)) as f64 * step;
        if delta != 0.0 {
            next = match selected {
                RangePart::Low => ((next.0 + delta).clamp(min, next.1), next.1),
                RangePart::High => (next.0, (next.1 + delta).clamp(next.0, max)),
                RangePart::Bar => shift_range(next.0, next.1, delta, min, max),
            };
        }
    }
    if next != (*low, *high) {
        (*low, *high) = next;
        response.mark_changed();
    }
    let a = to_x(*low);
    let b = to_x(*high);
    let ink = ui.visuals().text_color();
    let accent = ui.visuals().selection.bg_fill;
    ui.painter().line_segment(
        [Pos2::new(track.left(), y), Pos2::new(track.right(), y)],
        Stroke::new(3.0, ui.visuals().widgets.inactive.bg_fill),
    );
    // Match the ordinary slider's rail focus indicator without recoloring its knobs.
    if response.has_focus() {
        ui.painter().line_segment(
            [Pos2::new(track.left(), y), Pos2::new(track.right(), y)],
            Stroke::new(2.0, ui.visuals().selection.stroke.color),
        );
    }
    let over_bar = response.enabled()
        && response.hovered()
        && ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|pos| range_part(pos, a, b, y) == RangePart::Bar);
    let dragging_bar =
        response.enabled() && response.is_pointer_button_down_on() && selected == RangePart::Bar;
    let highlight = over_bar || dragging_bar;
    ui.painter().line_segment(
        [Pos2::new(a, y), Pos2::new(b, y)],
        Stroke::new(
            if highlight { 6.0 } else { 4.0 },
            if highlight {
                crate::theme::mix(accent, ink, 0.3)
            } else {
                accent
            },
        ),
    );
    for x in [a, b] {
        ui.painter()
            .circle_filled(Pos2::new(x, y), KNOB_RADIUS, ink);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, response.enabled(), label)
    });
    if dragging_bar {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    let cursor = if dragging_bar {
        egui::CursorIcon::Grabbing
    } else if over_bar {
        egui::CursorIcon::Grab
    } else {
        egui::CursorIcon::ResizeHorizontal
    };
    response.on_hover_cursor(cursor).on_hover_text("Drag either knob to set the minimum or maximum. Drag the bar to move both ends together. Arrow keys adjust the focused knob or bar.")
}

#[cfg(test)]
mod tests {
    struct Slider {
        ctx: egui::Context,
        low: f64,
        high: f64,
        enabled: bool,
        rect: egui::Rect,
    }

    impl Slider {
        fn new(low: f64, high: f64) -> Self {
            let mut slider = Self {
                ctx: egui::Context::default(),
                low,
                high,
                enabled: true,
                rect: egui::Rect::NOTHING,
            };
            slider.frame(vec![]);
            slider
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> (egui::Response, egui::FullOutput) {
            let mut response = None;
            let mut output = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 100.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(self.enabled, |ui| {
                        response = Some(super::range_track(
                            ui,
                            "Size",
                            &mut self.low,
                            &mut self.high,
                            0.0..=200.0,
                            240.0,
                        ));
                    });
                },
            );
            output.textures_delta.clear();
            let response = response.unwrap();
            self.rect = response.rect;
            (response, output)
        }

        fn point(&self, value: f64) -> egui::Pos2 {
            egui::pos2(
                self.rect.left() + 7.0 + (self.rect.width() - 14.0) * (value / 200.0) as f32,
                self.rect.center().y,
            )
        }

        fn button(&mut self, pos: egui::Pos2, pressed: bool) -> egui::Response {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ])
            .0
        }

        fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
            self.button(from, true);
            self.frame(vec![egui::Event::PointerMoved(to)]);
            self.button(to, false);
        }

        fn assert_range(&self, low: f64, high: f64) {
            assert!((self.low - low).abs() < 0.0001, "{} != {low}", self.low);
            assert!((self.high - high).abs() < 0.0001, "{} != {high}", self.high);
        }
    }

    #[test]
    fn range_knobs_move_independently_and_cannot_cross() {
        for (from, to, low, high) in [
            (40.0, 80.0, 80.0, 100.0),
            (100.0, 70.0, 40.0, 70.0),
            (40.0, 160.0, 100.0, 100.0),
            (100.0, 0.0, 40.0, 40.0),
        ] {
            let mut slider = Slider::new(40.0, 100.0);
            slider.drag(slider.point(from), slider.point(to));
            slider.assert_range(low, high);
        }
    }

    #[test]
    fn range_bar_moves_both_ends_without_jumping_and_preserves_width_at_limits() {
        let mut slider = Slider::new(40.0, 100.0);
        let from = slider.point(65.0);
        assert!(!slider.button(from, true).changed());
        slider.assert_range(40.0, 100.0);
        for (pointer, low, high) in [
            (95.0, 70.0, 130.0),
            (280.0, 140.0, 200.0),
            (-100.0, 0.0, 60.0),
            (65.0, 40.0, 100.0),
        ] {
            slider.frame(vec![egui::Event::PointerMoved(slider.point(pointer))]);
            slider.assert_range(low, high);
        }
        slider.button(from, false);
        // A fresh drag must start from the current range, not the previous gesture.
        slider.drag(slider.point(80.0), slider.point(100.0));
        slider.assert_range(60.0, 120.0);
    }

    #[test]
    fn range_knob_grab_keeps_offset_and_empty_track_clicks_move_nearest_end() {
        let mut slider = Slider::new(40.0, 100.0);
        let from = slider.point(40.0) + egui::vec2(3.0, 0.0);
        assert!(!slider.button(from, true).changed());
        slider.frame(vec![egui::Event::PointerMoved(
            from + egui::vec2(22.6, 0.0),
        )]);
        slider.button(from + egui::vec2(22.6, 0.0), false);
        slider.assert_range(60.0, 100.0);
        for value in [160.0, 20.0] {
            let pos = slider.point(value);
            slider.button(pos, true);
            slider.button(pos, false);
        }
        slider.assert_range(20.0, 160.0);
    }

    #[test]
    fn coincident_range_knobs_can_be_separated_in_either_direction() {
        for (offset, end, low, high) in [(-1.0, 20.0, 20.0, 80.0), (1.0, 140.0, 80.0, 140.0)] {
            let mut slider = Slider::new(80.0, 80.0);
            let offset = egui::vec2(offset, 0.0);
            slider.drag(slider.point(80.0) + offset, slider.point(end) + offset);
            slider.assert_range(low, high);
        }
    }

    #[test]
    fn disabled_range_ignores_knob_bar_and_empty_track_interactions() {
        let mut slider = Slider::new(40.0, 100.0);
        slider.enabled = false;
        slider.frame(vec![]);
        for value in [40.0, 70.0, 100.0, 160.0] {
            slider.drag(slider.point(value), slider.point(value + 30.0));
            slider.assert_range(40.0, 100.0);
        }
    }

    #[test]
    fn arrow_keys_adjust_selected_range_part_including_whole_bar() {
        for (selected, shift, expected) in [
            (40.0, false, (42.0, 100.0)),
            (100.0, false, (40.0, 102.0)),
            (70.0, false, (42.0, 102.0)),
            (70.0, true, (60.0, 120.0)),
        ] {
            let mut slider = Slider::new(40.0, 100.0);
            let point = slider.point(selected);
            slider.button(point, true);
            slider.button(point, false);
            let modifiers = egui::Modifiers {
                shift,
                ..Default::default()
            };
            slider.frame(vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ]);
            slider.assert_range(expected.0, expected.1);
        }
    }

    #[test]
    fn bar_highlights_on_hover_but_round_knobs_keep_their_color() {
        let mut slider = Slider::new(40.0, 100.0);
        let appearance = |output: &egui::FullOutput| {
            let mut knobs = Vec::new();
            let mut bar = None;
            for shape in &output.shapes {
                match &shape.shape {
                    egui::Shape::Circle(circle) => knobs.push((circle.radius, circle.fill)),
                    egui::Shape::LineSegment { stroke, .. } if stroke.width >= 4.0 => {
                        bar = Some(*stroke)
                    }
                    _ => {}
                }
            }
            (knobs, bar.unwrap())
        };
        let resting = appearance(&slider.frame(vec![]).1);
        assert_eq!(resting.0.len(), 2);
        assert!(resting.0.iter().all(|(radius, _)| *radius == 5.0));
        let point = slider.point(70.0);
        let hovered = slider.frame(vec![egui::Event::PointerMoved(point)]).1;
        assert_eq!(hovered.platform_output.cursor_icon, egui::CursorIcon::Grab);
        let hovered = appearance(&hovered);
        assert_eq!(resting.0, hovered.0);
        assert_ne!(resting.1.color, hovered.1.color);
        slider.button(slider.point(40.0), true);
        let focused = appearance(&slider.frame(vec![]).1);
        assert_eq!(resting.0, focused.0);
        assert_eq!(resting.1, focused.1);
    }
}
