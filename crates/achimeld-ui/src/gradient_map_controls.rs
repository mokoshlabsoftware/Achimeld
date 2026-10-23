use super::{ColorPoint, GradientMap, State, checker, decode_color, encode_color, interaction};
use crate::buttons::Button;
use egui::{Color32, Rect, Vec2};

// Every point value reserves the same right-hand numeric column, independent of digits or suffix.
fn value(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    bounds: std::ops::RangeInclusive<f64>,
    suffix: &str,
) -> bool {
    let value_id = ui
        .make_persistent_id("2d-gradient-editor")
        .with(("value", label));
    let number_width = ui
        .painter()
        .layout_no_wrap(
            "200.0%".into(),
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
        + 2.0 * ui.spacing().button_padding.x
        + 8.0;
    crate::fields::row_with_min_control(
        ui,
        label,
        number_width + 24.0 + ui.spacing().item_spacing.x,
        |ui| {
            let gap = ui.spacing().item_spacing.x;
            let width = ui.available_width();
            let number_width = number_width.min((width - gap - 1.0).max(1.0));
            ui.horizontal(|ui| {
                let response = crate::opacity::range_track(
                    ui,
                    value,
                    bounds.clone(),
                    (width - number_width - gap).max(1.0),
                );
                response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), *value, label));
                let changed = ui
                    .push_id(value_id, |ui| {
                        let response = ui.add_sized(
                            [number_width, ui.spacing().interact_size.y],
                            egui::DragValue::new(value)
                                .range(bounds)
                                .speed(if suffix == "×" { 0.01 } else { 0.1 })
                                .suffix(suffix)
                                .max_decimals(if suffix == "×" { 2 } else { 1 }),
                        );
                        #[cfg(test)]
                        ui.data_mut(|data| data.insert_temp(value_id, response.rect));
                        response.changed()
                    })
                    .inner;
                changed || response.changed()
            })
            .inner
        },
    )
}

pub(super) fn slice(ui: &mut egui::Ui, slice: &mut f64) {
    let mut display = *slice * 100.0;
    value(ui, "Test slice", &mut display, 0.0..=100.0, "%");
    *slice = display / 100.0;
}

pub(super) fn show(
    ui: &mut egui::Ui,
    id: egui::Id,
    map: &mut GradientMap,
    state: &mut State,
    anchor: Rect,
) {
    let Some(index) = state.active else {
        return;
    };
    crate::fields::row(ui, "Selected", |ui| {
        ui.label(state.selected.len().to_string())
            .on_hover_text("Color and influence values show the active point. Edits apply to all selected points; position moves the group.");
    });
    let mut color_anchor = anchor;
    crate::fields::row(ui, "Color", |ui| {
        let c = decode_color(&map.points[index].color_hex).unwrap_or([255; 4]);
        let button = ui.add(
            Button::new("")
                .id(id.with("point-color-swatch"))
                .min_size(Vec2::new(40.0, ui.spacing().interact_size.y)),
        );
        let swatch = button.rect.shrink(4.0);
        checker(ui, swatch);
        ui.painter().rect_filled(
            swatch,
            0.0,
            Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]),
        );
        color_anchor = button.rect;
        if button.clicked() {
            state.color = true;
        }
        button.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                button.enabled(),
                "Change selected point colors",
            )
        });
        button.on_hover_text("Change selected point colors");
    });
    for (axis, label) in ["Point X", "Point Y"].into_iter().enumerate() {
        let points = state.snapshots(map);
        let center = points.iter().map(|(_, p)| p[axis]).sum::<f64>() / points.len() as f64;
        let mut display = center * 100.0;
        if value(ui, label, &mut display, 0.0..=100.0, "%") {
            let mut delta = [0.0; 2];
            delta[axis] = display / 100.0 - center;
            interaction::translate(map, &points, delta);
        }
    }
    let point = map.points[index].clone();
    for (key, label, current, bounds, factor, suffix) in [
        (0, "Power", point.power, 0.05..=8.0, 1.0, "×"),
        (1, "X influence", point.range_x, 0.02..=2.0, 100.0, "%"),
        (2, "Y influence", point.range_y, 0.02..=2.0, 100.0, "%"),
    ] {
        let mut display = current * factor;
        if value(
            ui,
            label,
            &mut display,
            *bounds.start() * factor..=*bounds.end() * factor,
            suffix,
        ) {
            for &i in &state.selected {
                let point: &mut ColorPoint = &mut map.points[i];
                match key {
                    0 => point.power = display / factor,
                    1 => point.range_x = display / factor,
                    _ => point.range_y = display / factor,
                }
            }
        }
    }
    if state.color {
        let bytes = decode_color(&map.points[index].color_hex).unwrap_or([255; 4]);
        let mut color = state
            .picker
            .filter(|(i, c, _)| *i == index && *c == bytes)
            .map_or_else(
                || egui::ecolor::Hsva::from_srgba_unmultiplied(bytes),
                |(_, _, v)| v,
            );
        let mut open = true;
        if crate::color_picker::window(
            ui,
            id.with("point-color"),
            "Point color",
            color_anchor,
            &mut open,
            &mut color,
        ) {
            let encoded = encode_color(color.to_srgba_unmultiplied());
            for &i in &state.selected {
                map.points[i].color_hex = encoded.clone();
            }
        }
        state.picker = Some((
            index,
            decode_color(&map.points[index].color_hex).unwrap_or([255; 4]),
            color,
        ));
        state.color = open;
    }
}
