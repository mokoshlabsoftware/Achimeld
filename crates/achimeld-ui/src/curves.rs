use crate::buttons::UiButtonExt as _;
use achimeld_application::Preview;
use egui::{Pos2, Sense, Stroke, Vec2};
use serde_json::{Value, json};

pub(crate) fn mapped(points: &[(f64, f64)], input: f64) -> f64 {
    if points.is_empty() {
        return input;
    }
    if input <= points[0].0 {
        return points[0].1;
    }
    for i in 1..points.len() {
        if input <= points[i].0 {
            let p0 = points[i.saturating_sub(2)];
            let p1 = points[i - 1];
            let p2 = points[i];
            let p3 = points[(i + 1).min(points.len() - 1)];
            let span = p2.0 - p1.0;
            if span <= 0.0 {
                return p2.1;
            }
            let t = (input - p1.0) / span;
            let m1 = (p2.1 - p0.1) / (p2.0 - p0.0) * span;
            let m2 = (p3.1 - p1.1) / (p3.0 - p1.0) * span;
            return ((2.0 * t * t * t - 3.0 * t * t + 1.0) * p1.1
                + (t * t * t - 2.0 * t * t + t) * m1
                + (-2.0 * t * t * t + 3.0 * t * t) * p2.1
                + (t * t * t - t * t) * m2)
                .clamp(0.0, 1.0);
        }
    }
    points.last().map_or(input, |p| p.1)
}
pub(crate) fn editor(
    ui: &mut egui::Ui,
    points: &mut Vec<Value>,
    drag: &mut Option<usize>,
    histogram: Option<&Preview>,
) {
    ui.weak("RGB curves — drag points to adjust; right-click to remove");
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(360.0), 220.0),
        Sense::click_and_drag(),
    );
    let rect = rect.shrink(10.0);
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    let position = |x: f64, y: f64| {
        Pos2::new(
            rect.left() + x as f32 * rect.width(),
            rect.bottom() - y as f32 * rect.height(),
        )
    };
    if let Some(preview) = histogram {
        let mut counts = [0u32; 256];
        for pixel in preview.rgba.as_chunks::<4>().0 {
            if pixel[3] > 0 {
                for channel in &pixel[..3] {
                    counts[*channel as usize] += 1;
                }
            }
        }
        let max = counts.iter().copied().max().unwrap_or(1).max(1) as f32;
        for (i, count) in counts.iter().enumerate() {
            painter.line_segment(
                [
                    position(i as f64 / 255.0, 0.0),
                    position(i as f64 / 255.0, f64::from(*count as f32 / max)),
                ],
                Stroke::new(1.0, ui.visuals().weak_text_color().gamma_multiply(0.35)),
            );
        }
    }
    for i in 0..=4 {
        let t = i as f64 / 4.0;
        painter.line_segment(
            [position(t, 0.0), position(t, 1.0)],
            Stroke::new(0.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        painter.line_segment(
            [position(0.0, t), position(1.0, t)],
            Stroke::new(0.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
    }
    let mut values: Vec<_> = points
        .iter()
        .map(|p| {
            (
                p["input"].as_f64().unwrap_or(0.0),
                p["output"].as_f64().unwrap_or(0.0),
            )
        })
        .collect();
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    values.dedup_by(|a, b| a.0 == b.0);
    painter.add(egui::Shape::line(
        (0..=255)
            .map(|i| {
                let x = i as f64 / 255.0;
                position(x, mapped(&values, x))
            })
            .collect(),
        Stroke::new(2.0, ui.visuals().selection.stroke.color),
    ));
    for point in points.iter() {
        painter.circle_filled(
            position(
                point["input"].as_f64().unwrap_or(0.0),
                point["output"].as_f64().unwrap_or(0.0),
            ),
            4.0,
            ui.visuals().text_color(),
        );
    }
    if let Some(pointer) = response.interact_pointer_pos() {
        let nearest = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    i,
                    position(
                        p["input"].as_f64().unwrap_or(0.0),
                        p["output"].as_f64().unwrap_or(0.0),
                    )
                    .distance(pointer),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, distance)| *distance < 12.0)
            .map(|(i, _)| i);
        if response.secondary_clicked()
            && points.len() > 2
            && let Some(index) = nearest
        {
            points.remove(index);
            *drag = None;
        } else {
            if response.drag_started() || response.clicked() {
                *drag=nearest.or_else(||{if points.len()>=256 {return None;}let i=points.len();points.push(json!({"input":((pointer.x-rect.left())/rect.width()).clamp(0.0,1.0),"output":((rect.bottom()-pointer.y)/rect.height()).clamp(0.0,1.0)}));Some(i)});
            }
            if (response.dragged() || response.clicked())
                && let Some(index) = *drag
                && let Some(point) = points.get_mut(index)
            {
                point["input"] = json!(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
                point["output"] =
                    json!(((rect.bottom() - pointer.y) / rect.height()).clamp(0.0, 1.0));
            }
        }
    }
    if response.drag_stopped() {
        *drag = None;
    }
    if ui.app_button("Reset curves").clicked() {
        *points = vec![
            json!({"input":0.0,"output":0.0}),
            json!({"input":1.0,"output":1.0}),
        ];
        *drag = None;
    }
}
