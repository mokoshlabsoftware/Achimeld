use crate::{
    buttons::{self, Button},
    particles::{glyph, sliders},
};
use achimeld_application::{
    EditorPort,
    explosion::{DeltoidSettings, ExplosionSettings, MAX_OBJECTS, layout},
};
use egui::{Pos2, Vec2};
use serde_json::{Value, json};

#[cfg(test)]
pub(crate) fn properties(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    instance: &str,
) {
    action_bar(ui, port, layer, None);
    body_properties(ui, port, layer, instance);
}

pub(crate) fn settings_properties(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    instance: &str,
) {
    settings_action_bar(ui, port, layer, false, None);
    body_properties(ui, port, layer, instance);
}

pub(crate) type MaskControl<'a> = dyn FnMut(&mut egui::Ui, &mut Value, f32) + 'a;

pub(crate) fn action_bar(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    mask: Option<&mut MaskControl<'_>>,
) {
    settings_action_bar(ui, port, layer, true, mask);
}

fn settings_action_bar(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    presets: bool,
    mask: Option<&mut MaskControl<'_>>,
) {
    let count = 6 + usize::from(presets) + usize::from(mask.is_some());
    buttons::bar(ui, 24.0, |ui| {
        let pixels_per_point = ui.ctx().pixels_per_point();
        let width = ((ui.available_width() / count as f32 * pixels_per_point).floor()
            / pixels_per_point)
            .min(ui.spacing().interact_size.y);
        ui.spacing_mut().button_padding.x = ((width - 20.0) / 2.0)
            .floor()
            .max(0.0)
            .min(ui.spacing().button_padding.x);
        if let Some(mask) = mask {
            mask(ui, layer, width);
        }
        if presets {
            crate::effect_explorer::preset_icon_controls(
                ui,
                port,
                layer,
                achimeld_application::effect_presets::EffectKind::Explosion,
                width,
            );
        }
        toolbar_actions(ui, port, layer, width);
    });
}

fn action_button(
    ui: &mut egui::Ui,
    layer: &Value,
    key: &'static str,
    label: &str,
    bytes: &'static [u8],
    width: f32,
    selected: bool,
) -> egui::Response {
    let image = glyph(ui.ctx(), key, bytes);
    let response = ui.add(
        Button::new(image)
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
    response.on_hover_text(label)
}

fn toolbar_actions(ui: &mut egui::Ui, port: &mut dyn EditorPort, layer: &mut Value, width: f32) {
    let Ok(parsed) = ExplosionSettings::from_value(&layer["explosion"]) else {
        ui.label("Invalid explosion settings");
        return;
    };
    let mut settings = json!(parsed);
    let initial = settings.clone();
    let range = settings["countMin"].is_number();
    if action_button(
            ui,
            layer,
            "explosion-count-range",
            "Count range: switch between an exact object count and a random count between two bounds (1-512).",
            include_bytes!("../assets/toolbar/explosion-range.rgba"),
            width,
            range,
        ).clicked() {
            if range {
                settings.as_object_mut().expect("settings").remove("countMin");
            } else {
                settings["countMin"] = settings["count"].clone();
            }
            port.finish_group();
    }
    if action_button(
        ui,
        layer,
        "explosion-randomize",
        "Randomize",
        include_bytes!("../assets/toolbar/art-refresh.rgba"),
        width,
        false,
    )
    .clicked()
    {
        settings["seed"] =
            json!(u32::from_str_radix(&achimeld_application::new_id()[..8], 16).unwrap_or(1));
        port.finish_group();
    }
    for (key, label, bytes, setting) in [
        (
            "explosion-invert-colors",
            "Invert colors",
            include_bytes!("../assets/toolbar/layer-contrast.rgba").as_slice(),
            "invertColors",
        ),
        (
            "explosion-seed-per-achievement",
            "Different seed for each achievement",
            include_bytes!("../assets/toolbar/explosion-seed-per-achievement.rgba").as_slice(),
            "differentSeedPerInstance",
        ),
        (
            "explosion-cover-canvas",
            "Cover canvas: enlarge the polar circle to cover all icon corners",
            include_bytes!("../assets/toolbar/explosion-cover-canvas.rgba").as_slice(),
            "coverCanvas",
        ),
    ] {
        let selected = settings[setting] == true;
        if action_button(ui, layer, key, label, bytes, width, selected).clicked() {
            settings[setting] = json!(!selected);
            port.finish_group();
        }
    }
    bottom_curve_picker(ui, port, layer, &mut settings, width);
    if settings != initial {
        layer["explosion"] = settings;
    }
}

fn bottom_curve_picker(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &Value,
    settings: &mut Value,
    width: f32,
) {
    let curves = [
        (
            "Linear",
            "explosion-curve-linear",
            include_bytes!("../assets/toolbar/explosion-curve-linear.rgba").as_slice(),
        ),
        (
            "Quadratic",
            "explosion-curve-quadratic",
            include_bytes!("../assets/toolbar/explosion-curve-quadratic.rgba").as_slice(),
        ),
        (
            "Logarithmic",
            "explosion-curve-logarithmic",
            include_bytes!("../assets/toolbar/explosion-curve-logarithmic.rgba").as_slice(),
        ),
    ];
    let current = settings["bottomCurve"]
        .as_str()
        .unwrap_or("Linear")
        .to_owned();
    let (_, key, bytes) = curves.iter().find(|(name, _, _)| *name == current).unwrap();
    let image = glyph(ui.ctx(), key, bytes);
    let response = ui
        .add(
            Button::new(image)
                .id(egui::Id::new((
                    "explosion-bottom-curve",
                    layer["id"].as_str().unwrap_or(""),
                )))
                .bar()
                .min_size(Vec2::new(width, ui.spacing().interact_size.y))
                .image_tint_follows_text_color(true),
        )
        .on_hover_text(format!("Bottom scaling curve: {current}"));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            response.enabled(),
            format!("Bottom scaling curve: {current}"),
        )
    });
    buttons::menu_popup(ui, &response, |ui| {
        for (name, key, bytes) in curves {
            let image = glyph(ui.ctx(), key, bytes);
            if ui
                .add(
                    Button::new((image, name))
                        .id(egui::Id::new((
                            "explosion-bottom-curve-option",
                            layer["id"].as_str().unwrap_or(""),
                            name,
                        )))
                        .selected(current == name)
                        .image_tint_follows_text_color(true)
                        .bar()
                        .min_size(Vec2::new(ui.available_width(), 28.0)),
                )
                .clicked()
            {
                settings["bottomCurve"] = json!(name);
                port.finish_group();
                ui.close();
            }
        }
    });
}

pub(crate) fn body_properties(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    layer: &mut Value,
    instance: &str,
) {
    let Ok(parsed) = ExplosionSettings::from_value(&layer["explosion"]) else {
        ui.label("Invalid explosion settings");
        return;
    };
    let mut settings = json!(parsed);
    let initial = settings.clone();
    use crate::knobs::{Parameter as P, bank};
    let preferences = port.preferences().clone();
    let mut defaults = json!(ExplosionSettings::new_deltoid());
    defaults["countMin"] = json!(9);
    defaults["overallScale"] = json!(1.0);
    settings["overallScale"] = json!(layout(&parsed, instance).scale);
    let params = vec![
        P::new("Base brightness", "brightness", 0.0..=2.0, 0).percent(),
        if settings["countMin"].is_number() {
            P::new("Objects", "countMin", 1.0..=f64::from(MAX_OBJECTS), 1)
                .pair("count")
                .integer()
        } else {
            P::new("Objects", "count", 1.0..=f64::from(MAX_OBJECTS), 1).integer()
        },
        P::new("Overall scale", "overallScale", 0.01..=2.0, 2).percent(),
        P::new("Shape X", "shapeX", 0.0..=10.0, 3).multiplier(),
        P::new("Shape Y", "shapeY", 0.0..=10.0, 4).multiplier(),
        P::new("Fill power", "bottomPower", 0.0..=9.0, 5).percent(),
        P::new("Fill stretch", "bottomStretch", 0.0..=1.0, 6).percent(),
        P::new("Radial blur", "radialBlur", 0.0..=1.0, 7).percent(),
        P::new("Angular blur", "angularBlur", 0.0..=1.0, 8).percent(),
        P::new("Aspect ratio", "aspectMin", 1.0..=10.0, 9)
            .pair("aspectMax")
            .ratio(),
        P::new("Regularity", "regularity", 0.0..=1.0, 10),
    ];
    let previous_scale = settings["overallScale"].clone();
    bank(ui, &preferences, &mut settings, &defaults, &params);
    if settings["overallScale"] != previous_scale {
        settings["scaleMin"] = settings["overallScale"].clone();
        settings["scaleMax"] = settings["overallScale"].clone();
    }
    settings
        .as_object_mut()
        .expect("explosion settings")
        .remove("overallScale");
    egui::CollapsingHeader::new("Shape")
        .default_open(true)
        .show_unindented(ui, |ui| {
            sliders::range(
                ui,
                "Brightness variation",
                &mut settings,
                "brightnessMin",
                "brightnessMax",
                0.0..=2.0,
                100.0,
                "%",
            );
            if settings["deltoid"].is_null() {
                buttons::bar(ui, 24.0, |ui| {
                    if ui
                        .add(
                            Button::new("Use deltoid")
                                .id(egui::Id::new((
                                    "explosion-use-deltoid",
                                    layer["id"].as_str().unwrap_or(""),
                                )))
                                .bar()
                                .min_size(Vec2::new(
                                    ui.available_width(),
                                    buttons::bar_height(ui, 24.0),
                                )),
                        )
                        .clicked()
                    {
                        settings["deltoid"] = json!(DeltoidSettings::default());
                        port.finish_group();
                    }
                });
            }
            if settings["deltoid"].is_object() {
                let shape = &mut settings["deltoid"];
                sliders::scalar(
                    ui,
                    "Deltoid → Ellipse",
                    shape,
                    "ellipse",
                    0.0..=1.0,
                    false,
                    100.0,
                );
                sliders::scalar(ui, "Global scale", shape, "scale", 0.01..=2.0, false, 100.0);
                sliders::range(
                    ui,
                    "Random scale",
                    shape,
                    "scaleMin",
                    "scaleMax",
                    0.01..=2.0,
                    100.0,
                    "%",
                );
                for (label, min, max, bounds, factor, suffix) in [
                    ("Width", "widthMin", "widthMax", 0.0..=2.0, 100.0, "%"),
                    (
                        "Top height",
                        "topHeightMin",
                        "topHeightMax",
                        0.0..=1.0,
                        100.0,
                        "%",
                    ),
                    (
                        "Bottom height",
                        "bottomHeightMin",
                        "bottomHeightMax",
                        0.0..=1.0,
                        100.0,
                        "%",
                    ),
                    ("Offset", "offsetMin", "offsetMax", -1.5..=1.5, 100.0, "%"),
                    (
                        "Sharpness",
                        "sharpnessMin",
                        "sharpnessMax",
                        0.0..=1.0,
                        100.0,
                        "%",
                    ),
                ] {
                    sliders::range(ui, label, shape, min, max, bounds, factor, suffix);
                }
                ui.weak("Top points outward · bottom points inward");
            }
        });
    if let Ok(parsed) = ExplosionSettings::from_value(&settings) {
        construction_preview(ui, port, &parsed, instance);
    }
    ui.strong("Explosion gradient overlay");
    crate::layers::gradient_presets(ui, port, &mut settings["gradient"]);
    crate::inspector::choice(
        ui,
        "Gradient",
        &mut settings["gradient"],
        "kind",
        &["Radial", "Linear", "SDF"],
    );
    if settings["gradient"]["kind"] == "SDF" {
        ui.weak("Gradient follows distance from the shape edge to its interior.");
    }
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
    crate::layers::gradient_stops_ui(ui, &mut settings["gradient"]);
    if settings != initial {
        layer["explosion"] = settings;
    }
}

fn construction_preview(
    ui: &mut egui::Ui,
    port: &mut dyn EditorPort,
    settings: &ExplosionSettings,
    instance: &str,
) {
    egui::CollapsingHeader::new("Construction").show(ui, |ui| {
        ui.weak("Phase 1: rectangle → Phase 2: polar");
        let layout = layout(settings, instance);
        let width = ui.available_width().min(280.0);
        let height = width / layout.aspect as f32;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
        crate::gradient::checker(ui, rect);
        let id = egui::Id::new("explosion-construction-texture");
        let mut texture = ui.data(|data| data.get_temp::<(String, egui::TextureHandle)>(id));
        let instance_id = id.with("instance");
        if ui.data(|data| data.get_temp::<String>(instance_id)).as_deref() != Some(instance) {
            texture = None;
            ui.data_mut(|data| {
                data.insert_temp(instance_id, instance.to_owned());
                data.remove::<(String, egui::TextureHandle)>(id);
            });
        }
        if let Some(preview) = port.explosion_construction_preview(&json!(settings), instance)
            && texture.as_ref().is_none_or(|(key, _)| key != &preview.fingerprint)
        {
            let image = egui::ColorImage::from_rgba_unmultiplied([preview.width, preview.height], &preview.rgba);
            let options = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
            let handle = if let Some((_, handle)) = &mut texture {
                handle.set(image, options); handle.clone()
            } else { ui.ctx().load_texture("Explosion construction", image, options) };
            texture = Some((preview.fingerprint, handle));
        }
        if let Some((_, handle)) = &texture {
            ui.painter().image(handle.id(), rect, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
        }
        if let Some(texture) = texture { ui.data_mut(|data| data.insert_temp(id, texture)); }
        ui.weak("Actual shape coverage and blur before polar projection. Spatial gradients are applied after projection.");
        if let Some(error) = port.explosion_construction_error() { ui.weak(format!("Construction could not update: {error}")); }
    });
}
