use crate::buttons::{self, Button, UiButtonExt as _};
use achimeld_application::particles::{ParticleSettings, ParticleShape};
use serde_json::{Value, json};

pub(super) fn show(ui: &mut egui::Ui, value: &mut Value) {
    let Ok(settings) = ParticleSettings::from_value(value) else {
        return;
    };
    let mut parts = json!(settings.components());
    let before = parts.clone();
    let id = ui.id().with("particle-shape-selection");
    let count = parts.as_array().map_or(1, Vec::len);
    let mut selected = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(count - 1);
    ui.strong("Particle shapes");
    ui.weak("Every particle uses this composite.");
    crate::inspector::field(ui, "Shape preset", 100.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt("particle-shape-presets")
            .width(ui.available_width().min(180.0))
            .truncate()
            .selected_text("Choose…")
            .show_ui(ui, |ui| {
                for (name, kind, vertices, inner) in [
                    ("Sphere", "Sphere", 6, 0.45),
                    ("Triangle", "Polygon", 3, 0.45),
                    ("Square", "Polygon", 4, 0.45),
                    ("Hexagon", "Polygon", 6, 0.45),
                    ("Star", "Star", 5, 0.45),
                    ("Sparkle", "Star", 4, 0.2),
                ] {
                    if ui
                        .add(
                            Button::new(name)
                                .bar()
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        parts = json!([ParticleShape {
                            kind: kind.into(),
                            vertex_count: vertices,
                            inner_radius: inner,
                            ..Default::default()
                        }]);
                        selected = 0;
                        ui.close();
                    }
                }
                if ui
                    .add(
                        Button::new("Ring")
                            .bar()
                            .min_size(egui::vec2(ui.available_width(), 28.0)),
                    )
                    .clicked()
                {
                    parts = json!([
                        ParticleShape::default(),
                        ParticleShape {
                            operation: "Subtract".into(),
                            scale: 0.6,
                            ..Default::default()
                        }
                    ]);
                    selected = 0;
                    ui.close();
                }
            });
    });
    egui::ScrollArea::vertical()
        .id_salt("particle-parts")
        .max_height(160.0)
        .show(ui, |ui| {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                for (index, part) in parts.as_array().into_iter().flatten().enumerate() {
                    let label = format!(
                        "{}  {}{}",
                        index + 1,
                        part["kind"].as_str().unwrap_or("Sphere"),
                        if index == 0 {
                            String::new()
                        } else {
                            format!(" · {}", part["operation"].as_str().unwrap_or("Union"))
                        }
                    );
                    buttons::bar(ui, 20.0, |ui| {
                        if ui
                            .add(
                                Button::selectable(selected == index, label)
                                    .bar()
                                    .min_size(egui::vec2(
                                        ui.available_width(),
                                        buttons::bar_height(ui, 20.0),
                                    ))
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                        {
                            selected = index;
                        }
                    });
                }
            });
        });
    let mut move_by = 0;
    let mut remove = false;
    let mut add = None;
    buttons::bar(ui, 20.0, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add_enabled_ui(parts.as_array().unwrap().len() < 16, |ui| {
            ui.app_menu_button("Add", |ui| {
                for kind in ["Sphere", "Polygon", "Star"] {
                    if ui
                        .add(
                            Button::new(kind)
                                .bar()
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        add = Some(kind);
                        ui.close();
                    }
                }
            });
        });
        if ui
            .add_enabled(selected > 0, Button::new("↑").bar())
            .on_hover_text("Move shape up")
            .clicked()
        {
            move_by = -1;
        }
        if ui
            .add_enabled(
                selected + 1 < parts.as_array().unwrap().len(),
                Button::new("↓").bar(),
            )
            .on_hover_text("Move shape down")
            .clicked()
        {
            move_by = 1;
        }
        remove = ui
            .add_enabled(
                parts.as_array().unwrap().len() > 1,
                Button::new("Remove").bar(),
            )
            .clicked();
    });
    let array = parts.as_array_mut().unwrap();
    if let Some(kind) = add {
        array.push(json!(ParticleShape {
            kind: kind.into(),
            scale: 0.65,
            ..Default::default()
        }));
        selected = array.len() - 1;
    }
    if remove {
        array.remove(selected);
        selected = selected.min(array.len() - 1);
    }
    if move_by != 0 {
        let next = (selected as i32 + move_by) as usize;
        array.swap(selected, next);
        selected = next;
    }
    ui.ctx().data_mut(|data| data.insert_temp(id, selected));
    let part = &mut array[selected];
    ui.push_id(("particle-part", selected), |ui| {
        crate::inspector::choice(ui, "Shape", part, "kind", &["Sphere", "Polygon", "Star"]);
        ui.add_enabled_ui(selected != 0, |ui| {
            crate::inspector::choice(
                ui,
                "Combine",
                part,
                "operation",
                &["Union", "Subtract", "Intersect"],
            );
        });
        for (label, key, min, max, factor) in [
            ("Scale", "scale", 0.05, 2.0, 100.0),
            ("Offset X", "offsetX", -1.0, 1.0, 100.0),
            ("Offset Y", "offsetY", -1.0, 1.0, 100.0),
            ("Angle", "rotation", -360.0, 360.0, 1.0),
        ] {
            super::sliders::scalar(ui, label, part, key, min..=max, false, factor);
        }
        if part["kind"] != "Sphere" {
            super::sliders::scalar(
                ui,
                "Sides / points",
                part,
                "vertexCount",
                3.0..=16.0,
                true,
                1.0,
            );
        }
        if part["kind"] == "Star" {
            super::sliders::range(
                ui,
                "Inner radius",
                part,
                "innerRadiusMin",
                "innerRadius",
                0.05..=1.0,
                100.0,
                "%",
            );
        }
    });
    if parts != before {
        value["shapes"] = parts;
    }
}
