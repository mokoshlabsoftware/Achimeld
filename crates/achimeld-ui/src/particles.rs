use crate::{
    Desktop,
    buttons::{self, Button},
};
use achimeld_application::{
    EditorPort,
    particles::{ParticleSettings, layout_for_instance, presets},
};
use egui::{Pos2, Sense, Stroke, Vec2};
use serde_json::{Value, json};
#[path = "particle_shapes.rs"]
mod shapes;
pub(crate) use shapes::shape_icon;
#[path = "particle_sliders.rs"]
pub(crate) mod sliders;

impl Desktop {
    pub(crate) fn particle_properties(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        layer: &mut Value,
    ) {
        self.particle_editor(ui, port, layer, false);
    }
    pub(crate) fn particle_editor(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        layer: &mut Value,
        explorer: bool,
    ) {
        let achievement = self.layer_edit_target(port).unwrap_or_default();
        if explorer {
            action_bar(ui, port, layer, &achievement, true, None);
        }
        let id = layer["id"].as_str().unwrap_or("").to_owned();
        if let Some(loaded) = port.take_particle_preset(&achievement, &id) {
            port.finish_group();
            layer["particles"] = loaded;
        }
        let Ok(parsed) = editable_settings(&layer["particles"]) else {
            ui.label("Invalid particle settings");
            return;
        };
        let mut settings = json!(parsed);
        let initial = settings.clone();
        crate::inspector::field(ui, "Effect preset", 120.0, |ui| {
            crate::dropdowns::Dropdown::from_id_salt("particle-presets")
                .width(ui.available_width().min(180.0))
                .truncate()
                .selected_text("Choose preset…")
                .show_ui(ui, |ui| {
                    for (name, mut preset) in presets() {
                        if ui
                            .add(
                                Button::new(name)
                                    .bar()
                                    .min_size(egui::vec2(ui.available_width(), 28.0)),
                            )
                            .clicked()
                        {
                            preset.seed = settings["seed"].as_u64().unwrap_or(1) as u32;
                            preset.different_seed_per_instance =
                                settings["differentSeedPerInstance"] == true;
                            settings = json!(preset);
                            port.finish_group();
                            ui.close();
                        }
                    }
                });
        });
        use crate::knobs::{Parameter as P, bank};
        let preferences = port.preferences().clone();
        let defaults = json!(ParticleSettings::default());
        let basic = [
            P::new("Particle count", "count", 0.0..=512.0, 1).integer(),
            P::new("Size", "sizeScale", 0.0..=10.0, 2).multiplier(),
        ];
        let shape = [
            P::new("Shape X", "shapeX", 1.0..=10.0, 3).multiplier(),
            P::new("Shape Y", "shapeY", 1.0..=10.0, 4).multiplier(),
        ];
        let pair_width = 156.0 * (preferences.knob_value_size / 10.0).max(1.0);
        if ui.available_width() >= pair_width * 2.0 + 8.0 {
            ui.columns(2, |columns| {
                bank(
                    &mut columns[0],
                    &preferences,
                    &mut settings,
                    &defaults,
                    &basic,
                );
                bank(
                    &mut columns[1],
                    &preferences,
                    &mut settings,
                    &defaults,
                    &shape,
                );
            });
        } else {
            bank(ui, &preferences, &mut settings, &defaults, &basic);
            bank(ui, &preferences, &mut settings, &defaults, &shape);
        }
        let mut seed = settings["seed"].as_u64().unwrap_or(1) as u32;
        if seed_input(ui, &mut seed).changed() {
            settings["seed"] = json!(seed);
        }
        sliders::range(
            ui,
            "Size",
            &mut settings,
            "sizeMin",
            "sizeMax",
            1.0..=512.0,
            1.0,
            "",
        );
        sliders::range(
            ui,
            "Rotation",
            &mut settings,
            "rotationMin",
            "rotationMax",
            -360.0..=360.0,
            1.0,
            "°",
        );
        if ui
            .add(Button::new("No rotation"))
            .on_hover_text("Set the rotation range to 0 degrees for every particle")
            .clicked()
        {
            settings["rotationMin"] = json!(0.0);
            settings["rotationMax"] = json!(0.0);
        }
        sliders::range(
            ui,
            "Opacity",
            &mut settings,
            "opacityMin",
            "opacityMax",
            0.0..=1.0,
            100.0,
            "%",
        );
        sliders::range(
            ui,
            "SDF softness",
            &mut settings,
            "softnessMin",
            "softness",
            0.0..=1.0,
            100.0,
            "%",
        );
        sliders::scalar(
            ui,
            "Regularity",
            &mut settings,
            "regularity",
            0.0..=1.0,
            false,
            1.0,
        );
        ui.weak("0: equal spacing · 1: fully random");
        let project = port.editor().document().project();
        let width = project["atlas"]["outputWidth"]
            .as_f64()
            .or(project["atlas"]["outputSize"].as_f64())
            .unwrap_or(512.0);
        let height = project["atlas"]["outputHeight"]
            .as_f64()
            .or(project["atlas"]["outputSize"].as_f64())
            .unwrap_or(512.0);
        spawn_controls(
            ui,
            &mut settings,
            (width / height) as f32,
            &achievement,
            &preferences,
        );
        egui::CollapsingHeader::new("Particle shapes")
            .default_open(true)
            .show(ui, |ui| shapes::show(ui, port, &mut settings));
        ui.strong("Particle gradient");
        buttons::bar(ui, 24.0, |ui| {
            let is_2d = settings["colorMap"].is_object();
            let icon = if is_2d {
                glyph(
                    ui.ctx(),
                    "2d-gradient-collection",
                    include_bytes!("../assets/toolbar/gradient-map-collection.rgba"),
                )
            } else {
                glyph(
                    ui.ctx(),
                    "1d-gradient",
                    include_bytes!("../assets/toolbar/layer-gradient.rgba"),
                )
            };
            let response = ui.add(
                Button::new((
                    icon,
                    if is_2d {
                        "2D gradient ▾"
                    } else {
                        "1D gradient ▾"
                    },
                ))
                .id(ui.id().with("particle-gradient-type"))
                .bar()
                .image_tint_follows_text_color(true),
            );
            buttons::menu_popup(ui, &response, |ui| {
                if ui
                    .add(
                        Button::new((
                            glyph(
                                ui.ctx(),
                                "1d-gradient",
                                include_bytes!("../assets/toolbar/layer-gradient.rgba"),
                            ),
                            "1D gradient",
                        ))
                        .bar()
                        .image_tint_follows_text_color(true),
                    )
                    .clicked()
                {
                    settings["colorMap"] = Value::Null;
                    port.finish_group();
                    ui.close();
                }
                if ui
                    .add(
                        Button::new((
                            glyph(
                                ui.ctx(),
                                "2d-gradient-collection",
                                include_bytes!("../assets/toolbar/gradient-map-collection.rgba"),
                            ),
                            "2D gradient",
                        ))
                        .bar()
                        .image_tint_follows_text_color(true),
                    )
                    .clicked()
                {
                    if !is_2d {
                        settings["colorMap"] = json!(map_from_gradient(&settings["gradient"]));
                    }
                    settings["colorMode"] = json!("Particle");
                    port.finish_group();
                    ui.close();
                }
            });
            let collection = ui.add(
                Button::new(glyph(
                    ui.ctx(),
                    "2d-gradient-collection",
                    include_bytes!("../assets/toolbar/gradient-map-collection.rgba"),
                ))
                .id(ui.id().with("open-2d-gradient-collection"))
                .bar()
                .min_size(Vec2::splat(buttons::bar_height(ui, 24.0)))
                .image_tint_follows_text_color(true),
            );
            collection.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    collection.enabled(),
                    "2D Gradient Collection",
                )
            });
            if collection.on_hover_text("2D Gradient Collection").clicked() {
                crate::gradient_map_collection::request(ui.ctx());
            }
        });
        let has_map = settings["colorMap"].is_object();
        if has_map {
            ui.weak("Each particle uses a different random top-to-bottom slice of this color map.");
            let mut map = serde_json::from_value(settings["colorMap"].clone()).unwrap_or_default();
            crate::gradient_map::presets(ui, Some(port), &mut map);
            crate::gradient_map::editor(ui, &mut map);
            settings["colorMap"] = json!(map);
        } else {
            crate::inspector::choice(
                ui,
                "Color placement",
                &mut settings,
                "colorMode",
                &["Particle", "Spawn radius", "Random"],
            );
            crate::layers::gradient_presets(ui, port, &mut settings["gradient"]);
        }
        crate::inspector::choice(
            ui,
            "Gradient",
            &mut settings["gradient"],
            "kind",
            &["Linear", "Radial"],
        );
        if settings["gradient"]["kind"] == "Linear" {
            sliders::scalar(
                ui,
                "Gradient angle",
                &mut settings["gradient"],
                "angle",
                -360.0..=360.0,
                false,
                1.0,
            );
        }
        if !has_map {
            crate::layers::gradient_stops_ui(ui, &mut settings["gradient"]);
        }
        if settings != initial {
            match ParticleSettings::from_value(&settings) {
                Ok(_) => layer["particles"] = settings,
                Err(_) => {
                    self.error = Some("The particle edit was rejected. The previous settings and other project changes are preserved.".into());
                    achimeld_application::diagnostics::record(
                        "WARN",
                        "editor",
                        "Rejected invalid particle control candidate; previous settings retained",
                    );
                }
            }
        }
    }
}

fn map_from_gradient(gradient: &Value) -> achimeld_application::gradient_map::GradientMap {
    let mut stops: Vec<_> = gradient["stops"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["offset"].as_f64()?,
                achimeld_application::gradient_map::decode_color(s["colorHex"].as_str()?)?,
            ))
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    achimeld_application::gradient_map::GradientMap::from_stops(&stops)
}

pub(crate) fn action_bar(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    _achievement: &str,
    explorer: bool,
    mask: Option<&mut crate::explosion::MaskControl<'_>>,
) {
    let Ok(parsed) = editable_settings(&layer["particles"]) else {
        ui.label("Invalid particle settings");
        return;
    };
    let mut settings = json!(parsed);
    let initial = settings.clone();
    let count = if explorer { 2 } else { 4 } + usize::from(mask.is_some());
    buttons::bar(ui, 24.0, |ui| {
        let width = ui.available_width() / count as f32;
        ui.spacing_mut().button_padding.x = ((width - 20.0) / 2.0)
            .max(0.0)
            .min(ui.spacing().button_padding.x);
        if let Some(mask) = mask {
            mask(ui, layer, width);
        }
        if !explorer {
            crate::effect_explorer::preset_icon_controls(
                ui,
                port,
                layer,
                achimeld_application::effect_presets::EffectKind::Particle,
                width,
            );
            crate::effect_explorer::quick_load_controls(
                ui,
                port,
                layer,
                achimeld_application::effect_presets::EffectKind::Particle,
                width,
            );
            // A library preset may have replaced the complete recipe.
            if let Ok(parsed) = editable_settings(&layer["particles"]) {
                settings = json!(parsed);
            }
        }
        for (key, label, bytes) in [
            (
                "particle-random",
                "Randomize",
                include_bytes!("../assets/toolbar/art-refresh.rgba").as_slice(),
            ),
            (
                "particle-seed-per-achievement",
                "Different seed for each achievement",
                include_bytes!("../assets/toolbar/explosion-seed-per-achievement.rgba").as_slice(),
            ),
        ] {
            let selected = key == "particle-seed-per-achievement"
                && settings["differentSeedPerInstance"] == true;
            let response = ui.add(
                Button::new(glyph(ui.ctx(), key, bytes))
                    .id(egui::Id::new((key, layer["id"].as_str().unwrap_or(""))))
                    .selected(selected)
                    .bar()
                    .min_size(Vec2::new(width, ui.spacing().interact_size.y))
                    .image_tint_follows_text_color(true),
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    response.enabled(),
                    selected,
                    label,
                )
            });
            if response.on_hover_text(label).clicked() {
                match key {
                    "particle-random" => {
                        settings["seed"] = json!(
                            u32::from_str_radix(&achimeld_application::new_id()[..8], 16)
                                .unwrap_or(1)
                        );
                        port.finish_group();
                    }
                    _ => {
                        settings["differentSeedPerInstance"] = json!(!selected);
                        port.finish_group();
                    }
                }
            }
        }
    });
    if settings != initial {
        layer["particles"] = settings;
    }
}

pub(crate) fn seed_input(ui: &mut egui::Ui, seed: &mut u32) -> egui::Response {
    crate::inspector::field(ui, "Random seed", 150.0, |ui| {
        ui.add(egui::DragValue::new(seed).range(0..=u32::MAX).speed(1.0))
            .on_hover_text("Type a seed to change particle placement and gradient slices.")
    })
}

fn spawn_controls(
    ui: &mut egui::Ui,
    settings: &mut Value,
    aspect: f32,
    instance: &str,
    preferences: &achimeld_application::Preferences,
) {
    ui.strong("Spawn area");
    spawn_gizmo(ui, settings, aspect, instance);
    use crate::knobs::{Parameter as P, bank};
    bank(
        ui,
        preferences,
        settings,
        &json!(ParticleSettings::default()),
        &[
            P::new("Center X", "centerX", 0.0..=1.0, 3).percent(),
            P::new("Center Y", "centerY", 0.0..=1.0, 4).percent(),
            P::new("Edge", "edgeBias", 0.0..=1.0, 6).percent(),
        ],
    );
    sliders::range(
        ui,
        "Spawn radius",
        settings,
        "radiusMin",
        "radius",
        0.0..=1.0,
        100.0,
        "%",
    );
}
pub(crate) fn glyph(
    ctx: &egui::Context,
    name: &'static str,
    bytes: &'static [u8],
) -> egui::Image<'static> {
    let id = egui::Id::new(("particle-action", name));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture = ctx.load_texture(
                name,
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), Vec2::splat(20.0)))
}

pub(crate) fn spawn_gizmo(
    ui: &mut egui::Ui,
    value: &mut Value,
    aspect: f32,
    instance: &str,
) -> egui::Response {
    ui.weak("Drag to move the spawn center");
    let aspect = aspect.clamp(0.1, 10.0);
    let side = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click_and_drag());
    ui.painter()
        .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        value["centerX"] = json!(f64::from(
            ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0)
        ));
        value["centerY"] = json!(f64::from(
            ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0)
        ));
    }
    if let Ok(settings) = ParticleSettings::from_value(value) {
        let painter = ui.painter().with_clip_rect(rect);
        let center = Pos2::new(
            rect.left() + settings.center_x as f32 * rect.width(),
            rect.top() + settings.center_y as f32 * rect.height(),
        );
        let color = ui.visuals().selection.bg_fill;
        for radius in [settings.radius_min, settings.radius] {
            painter.circle_stroke(
                center,
                radius as f32 * rect.width().min(rect.height()),
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
        }
        for p in layout_for_instance(&settings, f64::from(aspect), instance) {
            painter.circle_filled(
                Pos2::new(
                    rect.left() + p.x as f32 * rect.width(),
                    rect.top() + p.y as f32 * rect.height(),
                ),
                1.5,
                color,
            );
        }
        painter.circle_filled(center, 6.0, color);
        let ink = ui.visuals().text_color();
        for direction in [Vec2::X, Vec2::Y] {
            painter.line_segment(
                [center - direction * 10.0, center + direction * 10.0],
                Stroke::new(1.5, ink),
            );
        }
        painter.circle_stroke(center, 6.0, Stroke::new(1.0, ink));
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair)
}

// Validation belongs at the commit boundary. Keep repair controls available for
// structurally readable drafts even when a numeric value is out of range.
pub(super) fn editable_settings(value: &Value) -> Result<ParticleSettings, serde_json::Error> {
    if value.is_null() {
        Ok(ParticleSettings::default())
    } else {
        serde_json::from_value(value.clone())
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;

    #[test]
    fn invalid_transform_remains_editable_without_resetting_the_effect() {
        let mut value = json!(ParticleSettings::default());
        value["shapes"] = json!([achimeld_application::particles::ParticleShape::default()]);
        value["shapes"][0]["scale"] = json!(2.01);
        value["seed"] = json!(123);
        assert!(ParticleSettings::from_value(&value).is_err());
        let draft = editable_settings(&value).unwrap();
        assert_eq!(draft.seed, 123);
        assert_eq!(draft.shapes[0].scale, 2.01);
        value["shapes"][0]["scale"] = json!(2.0);
        assert!(ParticleSettings::from_value(&value).is_ok());
    }
}
