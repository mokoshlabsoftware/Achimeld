//! Width-budgeted inline effect editors. Hidden controls stay in the property pane.
use egui::{Ui, Vec2};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Parameter {
    Number {
        key: &'static str,
        label: &'static str,
        icon: &'static str,
        default: f64,
        min: f64,
        max: f64,
        factor: f64,
    },
    Color,
}
impl Parameter {
    fn label(self) -> &'static str {
        match self {
            Self::Number { label, .. } => label,
            Self::Color => "Color",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Number { icon, .. } => icon,
            Self::Color => "Color",
        }
    }
}

pub(crate) struct Layout {
    pub selector_width: f32,
    pub name: bool,
    labels: bool,
    parameters: Vec<Parameter>,
    icon_size: f32,
    number_width: f32,
    color_width: f32,
}
fn number(
    key: &'static str,
    label: &'static str,
    icon: &'static str,
    default: f64,
    min: f64,
    max: f64,
    factor: f64,
) -> Parameter {
    Parameter::Number {
        key,
        label,
        icon,
        default,
        min,
        max,
        factor,
    }
}
fn parameters(kind: &str) -> Vec<Parameter> {
    match kind {
        "Outline" => vec![
            number("width", "Width", "Width", 2.0, 0.0, 64.0, 1.0),
            Parameter::Color,
        ],
        "InnerLight" => vec![
            number("blur", "Blur", "BoxBlur", 2.0, 0.0, 128.0, 1.0),
            Parameter::Color,
        ],
        "OuterLight" => vec![
            number("width", "Width", "Width", 2.0, 0.0, 64.0, 1.0),
            Parameter::Color,
        ],
        "Shadow" => vec![
            number("blur", "Blur", "BoxBlur", 2.0, 0.0, 128.0, 1.0),
            Parameter::Color,
        ],
        "BoxBlur" => vec![number("blur", "Blur", "BoxBlur", 2.0, 0.0, 128.0, 1.0)],
        "ColorOverlay" => vec![Parameter::Color],
        "GradientOverlay" => vec![number(
            "opacity",
            "Opacity",
            "GradientOverlay",
            1.0,
            0.0,
            100.0,
            100.0,
        )],
        "Skew" => vec![
            number("skewX", "Horizontal", "Skew", 0.0, -60.0, 60.0, 1.0),
            number("skewY", "Vertical", "Skew", 0.0, -60.0, 60.0, 1.0),
        ],
        "Perspective" => vec![
            number(
                "perspectiveX",
                "Horizontal",
                "Perspective",
                0.0,
                -45.0,
                45.0,
                100.0,
            ),
            number(
                "perspectiveY",
                "Vertical",
                "Perspective",
                0.0,
                -45.0,
                45.0,
                100.0,
            ),
        ],
        "Pinch" => vec![number(
            "amount", "Amount", "Pinch", 0.0, -100.0, 100.0, 100.0,
        )],
        "LensBlur" => vec![
            number("blur", "Edge blur", "LensBlur", 2.0, 0.0, 128.0, 1.0),
            number(
                "aberration",
                "Chromatic aberration",
                "LensBlur",
                0.0,
                0.0,
                64.0,
                1.0,
            ),
        ],
        _ => vec![],
    }
}
fn text_width(ui: &Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            crate::localization::text(ui.ctx(), text),
            egui::TextStyle::Button.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
}
pub(crate) fn layout(ui: &Ui, effect: &Value, available: f32, enabled: bool) -> Layout {
    let font = egui::TextStyle::Button.resolve(ui.style()).size;
    let icon_size = font.clamp(16.0, 24.0);
    let selector_min = icon_size + 8.0;
    let number_width = (font * 4.0 + 8.0).ceil();
    let color_width = (font * 2.2).max(28.0).ceil();
    let mut result = Layout {
        selector_width: available,
        name: false,
        labels: false,
        parameters: vec![],
        icon_size,
        number_width,
        color_width,
    };
    let width = |p: Parameter| {
        icon_size
            + 4.0
            + match p {
                Parameter::Color => color_width,
                _ => number_width,
            }
            + 4.0
    };
    let mut used = 0.0;
    if enabled {
        for parameter in parameters(effect["kind"].as_str().unwrap_or("")) {
            if available - used - width(parameter) < selector_min {
                break;
            }
            used += width(parameter);
            result.parameters.push(parameter);
        }
    }
    let name_width = selector_min
        + 4.0
        + text_width(
            ui,
            crate::layer_palette::label(effect["kind"].as_str().unwrap_or("Effect")),
        );
    result.name = available - used >= name_width;
    let label_width: f32 = result
        .parameters
        .iter()
        .map(|p| text_width(ui, p.label()) + 4.0)
        .sum();
    if result.name && !result.parameters.is_empty() && available - used - label_width >= name_width
    {
        result.labels = true;
        used += label_width;
    }
    result.selector_width = (available - used).max(selector_min);
    result
}

pub(crate) fn show(ui: &mut Ui, effect: &mut Value, layout: &Layout) -> bool {
    let mut active = false;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
        ui.spacing_mut().interact_size.y = 22.0;
        for parameter in &layout.parameters {
            ui.add(
                crate::effect_icons::image(ui.ctx(), parameter.icon(), layout.icon_size)
                    .tint(ui.visuals().text_color()),
            )
            .on_hover_text(crate::localization::text(ui.ctx(), parameter.label()));
            if layout.labels {
                ui.label(crate::localization::text(ui.ctx(), parameter.label()));
            }
            ui.push_id(parameter.label(), |ui| match *parameter {
                Parameter::Number {
                    key,
                    default,
                    min,
                    max,
                    factor,
                    ..
                } => {
                    let mut value = effect[key].as_f64().unwrap_or(default) * factor;
                    let response = ui.add_sized(
                        [layout.number_width, 22.0],
                        egui::DragValue::new(&mut value)
                            .range(min..=max)
                            .speed(0.5)
                            .fixed_decimals(1),
                    );
                    if response.changed() && (value / factor).is_finite() {
                        effect[key] = json!(value / factor);
                    }
                    active |= response.clicked() || response.drag_started() || response.has_focus();
                    response.on_hover_text(crate::localization::text(ui.ctx(), parameter.label()));
                }
                Parameter::Color => {
                    let hex = effect["colorHex"].as_str().unwrap_or("#FF000000");
                    let decoded = hex
                        .strip_prefix('#')
                        .and_then(|s| u32::from_str_radix(s, 16).ok().map(|n| (n, s.len())));
                    let mut rgba = match decoded {
                        Some((n, 8)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, (n >> 24) as u8],
                        Some((n, 6)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255],
                        _ => [255; 4],
                    };
                    let response =
                        crate::color_picker::edit_button_sized(ui, &mut rgba, layout.color_width);
                    active |= response.clicked() || response.changed();
                    if response.changed() {
                        effect["colorHex"] = json!(format!(
                            "#{:02X}{:02X}{:02X}{:02X}",
                            rgba[3], rgba[0], rgba[1], rgba[2]
                        ));
                    }
                }
            });
        }
    });
    active
}
