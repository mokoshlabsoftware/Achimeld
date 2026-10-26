use crate::Desktop;
use crate::buttons::UiButtonExt as _;
use crate::inspector::{choice, integer, number};
use achimeld_application::EditorPort;
use serde_json::{Value, json};

pub(crate) fn property_scroll_area(ui: &egui::Ui) -> egui::ScrollArea {
    let scroll = &ui.spacing().scroll;
    let mut margin = scroll.content_margin;
    if scroll.floating {
        // Reserve the expanded bar's hit area even while it is thin or hidden.
        // Keeping this gutter constant also prevents controls moving on hover.
        margin.right = margin
            .right
            .max((scroll.bar_width + scroll.bar_inner_margin).ceil() as i8);
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .content_margin(margin)
}

impl Desktop {
    pub(crate) fn layer_properties(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        layers: &mut [Value],
        achievement: &mut Value,
    ) {
        let mask = self
            .selected_layer
            .as_deref()
            .is_some_and(|id| achimeld_application::masks::is_source(layers, id));
        if let Some(id) = &self.selected_layer
            && let Some(layer) = layers
                .iter_mut()
                .find(|layer| layer["id"].as_str() == Some(id))
        {
            ui.push_id(("layer-opacity", self.selected_layer.clone()), |ui| {
                if !mask {
                    opacity(ui, layer);
                    crate::blend_preview::property_row(ui, |ui| {
                        let selected = layer["blendMode"].as_str().unwrap_or("Normal").to_owned();
                        let achievement = self.layer_edit_target(port).unwrap_or_default();
                        self.layer_blend_menu(
                            ui,
                            port,
                            &achievement,
                            layer,
                            crate::blend_preview::button(
                                ui,
                                &selected,
                                ui.available_width().min(180.0),
                                true,
                            )
                            .id(
                                crate::layer_palette::layer_control_id(
                                    &achievement,
                                    layer["id"].as_str().unwrap_or(""),
                                    "property-blend",
                                ),
                            ),
                        );
                    });
                }
                identity(ui, layer);
            });
        }
        // Keep the three shared rows visible while type-specific properties scroll below.
        property_scroll_area(ui)
            .id_salt(("layer-property-scroll", &self.selected_layer))
            .show(ui, |ui| {
                self.layer_property_fields(ui, port, layers, achievement);
            });
    }
    fn layer_property_fields(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        layers: &mut [Value],
        achievement: &mut Value,
    ) {
        let selected = self.selected_layer.clone();
        let mask = selected
            .as_deref()
            .is_some_and(|id| achimeld_application::masks::is_source(layers, id));
        let mask_content = achimeld_application::masks::content_ids(layers);
        let grayscale = selected
            .as_deref()
            .is_some_and(|id| mask_content.contains(id));
        let reference_sources: Vec<_> = layers
            .iter()
            .filter(|source| {
                source["id"]
                    .as_str()
                    .is_some_and(|id| !achimeld_application::masks::is_source(layers, id))
            })
            .filter_map(|source| {
                Some((
                    source["id"].as_str()?.to_owned(),
                    source["displayName"].as_str().unwrap_or("Layer").to_owned(),
                ))
            })
            .collect();
        let blocked_masks = selected
            .as_deref()
            .map(|id| crate::layer_palette::blocked_mask_sources(layers, id))
            .unwrap_or_default();
        if let Some(id) = &selected
            && let Some(layer) = layers.iter_mut().find(|l| l["id"].as_str() == Some(id))
        {
            let stored_layer = layer;
            let mut effective = stored_layer.clone();
            if matches!(effective["kind"].as_str(), Some("Text" | "Text3d"))
                && !effective["text"].is_object()
            {
                effective["text"] = json!({"content":"","fontFamily":"Segoe UI","fontSize":128.0,"colorHex":"#FFFFFFFF","alignment":"BottomRight"});
            }
            if matches!(
                effective["kind"].as_str(),
                Some("Gradient" | "GradientOverlay")
            ) && !effective["gradient"].is_object()
            {
                effective["gradient"] = json!({"kind":"Linear","angle":90.0,"stops":[{"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]});
            }
            if matches!(
                effective["kind"].as_str(),
                Some(
                    "Contrast" | "HueSaturation" | "SelectiveColor" | "UnsharpMask" | "RadialBlur"
                )
            ) && !effective["adjustment"].is_object()
            {
                effective["adjustment"] = json!({});
            }
            if effective["kind"] == "Shape" && !effective["shape"].is_object() {
                effective["shape"] = json!({"kind":"Rectangle","colorHex":effective["solidColorHex"],"scaleX":0.5,"scaleY":0.5,"vertexCount":5,"innerRadius":0.5});
            }
            if effective["kind"] == "Curves" && !effective["curves"].is_object() {
                effective["curves"] =
                    json!({"points":[{"input":0.0,"output":0.0},{"input":1.0,"output":1.0}]});
            }
            if effective["kind"] == "SdfVignette" && !effective["sdfVignette"].is_object() {
                effective["sdfVignette"] =
                    json!({"colorHex":"#FF000000","roundness":0.5,"inset":0.15,"softness":0.2});
            }
            if effective["kind"] == "Text3d" {
                let mut settings =
                    serde_json::to_value(achimeld_application::text3d::Text3dSettings::default())
                        .expect("Finite 3D text defaults");
                if let Some(stored) = effective["text3d"].as_object() {
                    for (key, value) in stored {
                        settings[key] = value.clone();
                    }
                }
                effective["text3d"] = settings;
            }
            if effective["kind"] == "Artifact3d" && !effective["artifact3d"].is_object() {
                effective["artifact3d"] =
                    json!(achimeld_application::artifact3d::ArtifactSettings::default());
            }
            if grayscale {
                achimeld_application::masks::grayscale_colors(&mut effective);
            }
            let initial = effective.clone();
            let layer = &mut effective;
            if matches!(layer["kind"].as_str(), Some("Gradient" | "GradientOverlay")) {
                gradient_presets(ui, port, &mut layer["gradient"]);
            }
            if layer["kind"] == "Particle" {
                let achievement = self.layer_edit_target(port).unwrap_or_default();
                crate::particles::action_bar(ui, port, layer, &achievement, false, None);
            } else if layer["kind"] == "Explosion" {
                crate::explosion::action_bar(ui, port, layer, None);
            }
            match layer["kind"].as_str().unwrap_or("") {
                "MaskReference" => {
                    let candidates = reference_sources
                        .iter()
                        .filter(|(source, _)| !blocked_masks.contains(source))
                        .cloned()
                        .collect::<Vec<_>>();
                    reference_choice(
                        ui,
                        "Source layer",
                        layer,
                        "maskSourceLayerId",
                        &candidates,
                        "None",
                    );
                    ui.weak(crate::localization::text(
                        ui.ctx(),
                        "Copies the source layer’s mask and inversion. Updates with the source.",
                    ));
                    if layer["maskSourceLayerId"].is_null() {
                        ui.weak(crate::localization::text(
                            ui.ctx(),
                            "No source mask: fully reveals.",
                        ));
                    }
                }
                "Artifact3d" => crate::artifact3d::properties(ui, &mut layer["artifact3d"]),
                "Text" | "Text3d" => {
                    crate::text_content::show(ui, id, &mut layer["text"]);
                    crate::font_picker::show(ui, port.system_font_families(), &mut layer["text"]);
                    number(
                        ui,
                        "Font size",
                        &mut layer["text"],
                        "fontSize",
                        1.0..=512.0,
                        1.0,
                    );
                    color(ui, "Text color", &mut layer["text"], "colorHex");
                    if layer["kind"] == "Text3d" {
                        crate::text3d::properties(ui, &mut layer["text3d"]);
                    } else {
                        crate::text_alignment::show(ui, &mut layer["text"]);
                    }
                }
                "Solid" => color(ui, "Fill color", layer, "solidColorHex"),
                "PerlinNoise" => crate::perlin::properties(ui, port, &mut layer["perlinNoise"]),
                "Shape" => {
                    if !layer["shape"].is_object() {
                        layer["shape"] = json!({"kind":"Rectangle","colorHex":layer["solidColorHex"],"scaleX":0.5,"scaleY":0.5,"vertexCount":5,"innerRadius":0.5});
                    }
                    choice(
                        ui,
                        "Shape",
                        &mut layer["shape"],
                        "kind",
                        &["Rectangle", "Sphere", "Polygon", "Star"],
                    );
                    color(ui, "Fill color", &mut layer["shape"], "colorHex");
                    if ui.available_width() >= 300.0 {
                        ui.columns(2, |columns| {
                            number(
                                &mut columns[0],
                                "Width",
                                &mut layer["shape"],
                                "scaleX",
                                0.01..=2.0,
                                0.01,
                            );
                            number(
                                &mut columns[1],
                                "Height",
                                &mut layer["shape"],
                                "scaleY",
                                0.01..=2.0,
                                0.01,
                            );
                        });
                    } else {
                        number(ui, "Width", &mut layer["shape"], "scaleX", 0.01..=2.0, 0.01);
                        number(
                            ui,
                            "Height",
                            &mut layer["shape"],
                            "scaleY",
                            0.01..=2.0,
                            0.01,
                        );
                    }
                    number(
                        ui,
                        "Inner radius",
                        &mut layer["shape"],
                        "innerRadius",
                        0.01..=2.0,
                        0.01,
                    );
                    integer(ui, "Vertices", &mut layer["shape"], "vertexCount", 3..=64);
                    number(
                        ui,
                        "Shape angle",
                        &mut layer["shape"],
                        "rotation",
                        -360.0..=360.0,
                        1.0,
                    );
                    ui.horizontal(|ui| {
                        if crate::icons::button(
                            ui,
                            crate::icons::Icon::Copy,
                            "Copy shape transform",
                            true,
                        )
                        .clicked()
                        {
                            self.layer_palette.shape_transform = Some(layer["shape"].clone());
                        }
                        if crate::icons::button(
                            ui,
                            crate::icons::Icon::Paste,
                            "Paste shape transform",
                            self.layer_palette.shape_transform.is_some(),
                        )
                        .clicked()
                            && let Some(transform) = &self.layer_palette.shape_transform
                        {
                            for key in ["scaleX", "scaleY", "rotation"] {
                                layer["shape"][key] = transform[key].clone();
                            }
                        }
                    });
                }
                "StandaloneImage" | "SteamImportedArtwork" => {
                    if ui.app_button("Choose image…").clicked() {
                        self.open_source_picker(
                            ui.ctx(),
                            port,
                            id,
                            crate::source_picker::SourceKind::Image,
                        );
                    }
                    let images = library(port, "images");
                    reference_choice(ui, "Image", layer, "imageId", &images, "Select image");
                }
                "AtlasTile" => {
                    if ui.app_button("Pick atlas tile…").clicked() {
                        self.open_source_picker(
                            ui.ctx(),
                            port,
                            id,
                            crate::source_picker::SourceKind::Atlas,
                        );
                    }
                    crate::inspector::boolean(
                        ui,
                        "Use general image atlas / tile",
                        layer,
                        "linkAtlasToGeneral",
                    );
                    let atlases = library(port, "atlases");
                    reference_choice(ui, "Atlas", layer, "atlasId", &atlases, "Default atlas");
                    integer(ui, "Tile index", layer, "tileIndex", 0..=20000);
                }
                "AchievementImage" => {
                    if self.view == crate::desktop::View::Style {
                        integer(ui, "Image ID", layer, "generalImageIndex", 0..=20000);
                        ui.weak("Set preview artwork in Style options. Each using icon supplies its own General images.");
                    } else {
                        self.general_explorer(ui, port, achievement, Some(layer));
                    }
                }
                "SdfVignette" => {
                    crate::vignette::properties(ui, port, &mut layer["sdfVignette"]);
                }
                "Particle" => self.particle_properties(ui, port, layer),
                "Explosion" => {
                    let instance = self.layer_edit_target(port).unwrap_or_default();
                    crate::explosion::body_properties(ui, port, layer, &instance);
                }
                "Curves" => {
                    if !layer["curves"].is_object() {
                        layer["curves"] = json!({"points":[{"input":0.0,"output":0.0},{"input":1.0,"output":1.0}]});
                    }
                    if let Some(points) = layer["curves"]["points"].as_array_mut() {
                        let achievement = self.layer_edit_target(port).unwrap_or_default();
                        let preview = port.layer_preview(&achievement, id);
                        crate::curves::editor(
                            ui,
                            points,
                            &mut self.layer_palette.curve_drag,
                            preview.as_ref(),
                        );
                        let mut remove = None;
                        for (i, point) in points.iter_mut().enumerate() {
                            ui.push_id(("curve", i), |ui| {
                                number(ui, "Input", point, "input", 0.0..=1.0, 0.01);
                                number(ui, "Output", point, "output", 0.0..=1.0, 0.01);
                                if ui.small_button("Remove point").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove {
                            points.remove(i);
                        }
                        if ui.app_button("Add curve point").clicked() {
                            points.push(json!({"input":0.5,"output":0.5}));
                        }
                    }
                }
                "RadialBlur" => {
                    for (label, key) in [
                        ("Radial blur", "radialBlur"),
                        ("Angular blur", "angularBlur"),
                    ] {
                        let mut value = layer["adjustment"][key].as_f64().unwrap_or(0.0);
                        if crate::sliders::value_slider(ui, label, &mut value, 0.0..=100.0, "%", 1)
                        {
                            layer["adjustment"][key] = json!(value);
                        }
                    }
                }
                "Contrast" | "HueSaturation" | "SelectiveColor" | "UnsharpMask" => {
                    if !layer["adjustment"].is_object() {
                        layer["adjustment"] = json!({});
                    }
                    let previous_adjustment = layer["adjustment"].clone();
                    if layer["kind"] == "SelectiveColor" {
                        selective_range(ui, &mut layer["adjustment"]);
                    }
                    let fields: &[(&str, &str)] = match layer["kind"].as_str() {
                        Some("Contrast") => {
                            &[("Brightness", "brightness"), ("Contrast", "contrast")]
                        }
                        Some("HueSaturation") => &[
                            ("Hue", "hue"),
                            ("Saturation", "saturation"),
                            ("Lightness", "lightness"),
                        ],
                        Some("SelectiveColor") => &[
                            ("Cyan", "cyan"),
                            ("Magenta", "magenta"),
                            ("Yellow", "yellow"),
                            ("Black", "black"),
                        ],
                        _ => &[
                            ("Amount", "unsharpAmount"),
                            ("Radius", "unsharpRadius"),
                            ("Threshold", "unsharpThreshold"),
                        ],
                    };
                    for &(label, key) in fields {
                        adjustment_slider(
                            ui,
                            label,
                            &mut layer["adjustment"],
                            key,
                            if key == "hue" {
                                -180.0..=180.0
                            } else if key == "unsharpAmount" {
                                0.0..=500.0
                            } else if key == "unsharpRadius" {
                                0.1..=250.0
                            } else if key == "unsharpThreshold" {
                                0.0..=255.0
                            } else {
                                -100.0..=100.0
                            },
                            1.0,
                        );
                    }
                    if layer["kind"] == "SelectiveColor"
                        && layer["adjustment"] != previous_adjustment
                    {
                        save_selective_range(&mut layer["adjustment"]);
                    }
                }
                "Gradient" | "GradientOverlay" => {
                    if layer["kind"] == "GradientOverlay" {
                        choice(
                            ui,
                            "Input channel",
                            &mut layer["gradient"],
                            "inputChannel",
                            &["RGB", "Red", "Green", "Blue"],
                        );
                    }
                    gradient_ui(ui, &mut layer["gradient"]);
                }
                _ => {}
            }
            if !crate::layer_palette::is_adjustment(layer["kind"].as_str().unwrap_or(""))
                && layer["kind"] != "Folder"
            {
                ui.separator();
                ui.strong("Transform");
                transform_ui(ui, layer);
                ui.horizontal(|ui| {
                    if crate::icons::button(ui,crate::icons::Icon::Copy,"Copy transform",true).clicked(){self.layer_palette.transform=Some(json!({"scale":layer["scale"],"rotation":layer["rotation"],"offsetX":layer["offsetX"],"offsetY":layer["offsetY"]}));}
                    if crate::icons::button(ui,crate::icons::Icon::Paste,"Paste transform",self.layer_palette.transform.is_some()).clicked() && let Some(transform)=&self.layer_palette.transform {for key in ["scale","rotation","offsetX","offsetY"] {layer[key]=transform[key].clone();}}
                });
            }
            if !mask {
                effects_ui(ui, layer);
            }
            if *layer != initial {
                if mask {
                    achimeld_application::masks::retain_source_controls(stored_layer, layer);
                }
                if grayscale {
                    achimeld_application::masks::grayscale_colors(layer);
                }
                *stored_layer = layer.clone();
            }
        }
    }
}
fn library(port: &dyn EditorPort, key: &str) -> Vec<(String, String)> {
    port.editor().document().project()["assets"][key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some((
                a["id"].as_str()?.into(),
                a["displayName"].as_str().unwrap_or("Image").into(),
            ))
        })
        .collect()
}
fn reference_choice(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    items: &[(String, String)],
    empty: &str,
) {
    let mut selected = value[key].as_str().unwrap_or("").to_owned();
    let original = selected.clone();
    crate::inspector::field(ui, label, 120.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt(key)
            .width(ui.available_width().min(180.0))
            .truncate()
            .selected_text(
                items
                    .iter()
                    .find(|(id, _)| id == &selected)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or(empty),
            )
            .show_ui(ui, |ui| {
                ui.app_selectable_value(&mut selected, String::new(), empty);
                for (id, name) in items {
                    ui.app_selectable_value(&mut selected, id.clone(), name);
                }
            })
    });
    if selected != original {
        value[key] = if selected.is_empty() {
            Value::Null
        } else {
            json!(selected)
        };
    }
}
pub(crate) fn opacity(ui: &mut egui::Ui, value: &mut Value) {
    let mut percent = value["opacity"].as_f64().unwrap_or(1.0) * 100.0;
    if crate::sliders::value_slider(ui, "Opacity", &mut percent, 0.0..=100.0, "%", 0) {
        value["opacity"] = json!(percent / 100.0);
    }
}
pub(crate) fn transform_ui(ui: &mut egui::Ui, layer: &mut Value) {
    let explosion = layer["kind"] == "Explosion";
    let fields: Vec<_> = [
        ("Offset X", "offsetX", -4096.0, 4096.0, 1.0, false),
        ("Offset Y", "offsetY", -4096.0, 4096.0, 1.0, false),
        ("Scale", "scale", 0.01, 10.0, 0.01, true),
        ("Angle", "rotation", -360.0, 360.0, 1.0, false),
    ]
    .into_iter()
    .filter(|(_, key, ..)| !explosion || *key != "scale")
    .collect();
    for (label, key, min, max, speed, percent) in fields {
        crate::fields::row(ui, label, |ui| {
            compact_number(ui, layer, key, min, max, speed, percent);
        });
    }
}
pub(crate) fn blend_mode(ui: &mut egui::Ui, value: &mut Value) {
    crate::blend_preview::property(ui, value);
}
fn identity(ui: &mut egui::Ui, layer: &mut Value) {
    let mut name = layer["displayName"].as_str().unwrap_or("").to_owned();
    let kind = layer["kind"].as_str().unwrap_or("Layer");
    let icon = crate::layer_icons::texture(ui.ctx(), kind).map(|texture| {
        (
            egui::Image::new((texture, egui::Vec2::splat(20.0))).tint(ui.visuals().text_color()),
            format!(
                "{}: {}",
                crate::localization::text(ui.ctx(), "LpType"),
                crate::localization::text(ui.ctx(), crate::layer_palette::label(kind))
            ),
        )
    });
    let changed = crate::fields::row_with_icon(ui, "LpName", icon, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut name)
                .id_salt("displayName")
                .desired_width(ui.available_width())
                .min_size(egui::Vec2::ZERO),
        )
        .changed()
    });
    if changed {
        layer["displayName"] = json!(name);
    }
}
fn compact_number(
    ui: &mut egui::Ui,
    value: &mut Value,
    key: &str,
    min: f64,
    max: f64,
    speed: f64,
    percent: bool,
) {
    let factor = if percent { 100.0 } else { 1.0 };
    let mut number = value[key]
        .as_f64()
        .unwrap_or(if key == "scale" { 1.0 } else { 0.0 })
        * factor;
    if ui
        .add(
            egui::DragValue::new(&mut number)
                .range(min * factor..=max * factor)
                .speed(speed * factor)
                .suffix(if percent { "%" } else { "" }),
        )
        .changed()
    {
        value[key] = json!(number / factor);
    }
}
pub(crate) fn color(ui: &mut egui::Ui, label: &str, value: &mut Value, key: &str) {
    crate::inspector::field(ui, label, 100.0, |ui| {
        let hex = value[key].as_str().unwrap_or("#FFFFFFFF");
        let decoded = hex
            .strip_prefix('#')
            .and_then(|hex| u32::from_str_radix(hex, 16).ok().map(|n| (n, hex.len())));
        let mut rgba = match decoded {
            Some((n, 8)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, (n >> 24) as u8],
            Some((n, 6)) => [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255],
            _ => [255; 4],
        };
        if crate::color_picker::edit_button(ui, &mut rgba).changed() {
            value[key] = json!(format!(
                "#{:02X}{:02X}{:02X}{:02X}",
                rgba[3], rgba[0], rgba[1], rgba[2]
            ));
        }
        if key == "backgroundColorHex" {
            if value[key].is_null() {
                ui.weak("None");
            } else if ui.app_button("Clear").clicked() {
                value[key] = Value::Null;
            }
        }
    });
}
pub(crate) fn effects_ui(ui: &mut egui::Ui, layer: &mut Value) {
    ui.collapsing("Effects", |ui| {
        let mut remove = None;
        if let Some(effects) = layer.get_mut("effects").and_then(Value::as_array_mut) {
            for (i, effect) in effects.iter_mut().enumerate() {
                ui.push_id(("effect", i), |ui| {
                    ui.collapsing(
                        crate::layer_palette::label(effect["kind"].as_str().unwrap_or("Effect"))
                            .to_owned(),
                        |ui| {
                            effect_properties(ui, effect);
                            ui.horizontal(|ui| {
                                if ui.small_button("Move up").clicked() {
                                    remove = Some((i, -1));
                                }
                                if ui.small_button("Move down").clicked() {
                                    remove = Some((i, 1));
                                }
                                if ui.small_button("Delete effect").clicked() {
                                    remove = Some((i, 0));
                                }
                            });
                        },
                    );
                });
            }
            if let Some((i, delta)) = remove {
                if delta == 0 {
                    effects.remove(i);
                } else {
                    let next = i as i32 + delta;
                    if next >= 0 && (next as usize) < effects.len() {
                        effects.swap(i, next as usize);
                    }
                }
            }
        }
        ui.app_menu_button("Add effect", |ui| {
            crate::layer_palette::effect_menu(ui, layer)
        });
    });
}
pub(crate) fn effect_properties(ui: &mut egui::Ui, effect: &mut Value) {
    effect_blending(ui, effect);
    effect_property_fields(ui, effect);
}
fn spatial_effect(effect: &Value) -> bool {
    matches!(
        effect["kind"].as_str(),
        Some("Skew" | "Perspective" | "Pinch" | "LensBlur")
    )
}
pub(crate) fn effect_blending(ui: &mut egui::Ui, effect: &mut Value) {
    opacity(ui, effect);
    if !spatial_effect(effect) {
        blend_mode(ui, effect);
    }
}
pub(crate) fn effect_property_fields(ui: &mut egui::Ui, effect: &mut Value) {
    let mut effective = effect.clone();
    for (key, default) in json!({"visible":true,"colorHex":"#FF000000",
        "width":2.0,"blur":2.0,
        "offsetX":2.0,"offsetY":2.0,"cutout":0.0,"blurEnabled":true,
        "blurColorHex":"#FFFFFFFF","blurOpacity":1.0,"blurBlendMode":"Screen"})
    .as_object()
    .unwrap()
    {
        if effective[key].is_null() {
            effective[key] = default.clone();
        }
    }
    let initial = effective.clone();
    effect_property_fields_inner(ui, &mut effective);
    if effective != initial {
        *effect = effective;
    }
}
fn effect_property_fields_inner(ui: &mut egui::Ui, effect: &mut Value) {
    ui.heading(crate::layer_palette::label(
        effect["kind"].as_str().unwrap_or("Effect"),
    ));
    crate::inspector::boolean(ui, "Enabled", effect, "visible");
    if !spatial_effect(effect) && effect["kind"] != "GradientOverlay" && effect["kind"] != "BoxBlur"
    {
        color(ui, "Color", effect, "colorHex");
    }
    match effect["kind"].as_str() {
        Some("Skew") => {
            spatial_axes(ui, effect, "skewX", "skewY", 60.0, 1.0, "°");
            ui.weak("Shear horizontally, then vertically, around the canvas center.");
        }
        Some("Perspective") => {
            spatial_axes(ui, effect, "perspectiveX", "perspectiveY", 45.0, 100.0, "%");
            ui.weak("Tilt horizontally and vertically around the canvas center.");
        }
        Some("Pinch") => {
            spatial_number(ui, "Amount", effect, "amount", -100.0..=100.0, 100.0, "%");
            ui.weak("Positive values bulge the center (fisheye); negative values pinch it inward.");
        }
        Some("LensBlur") => {
            number(ui, "Edge blur", effect, "blur", 0.0..=128.0, 0.5);
            number(
                ui,
                "Chromatic aberration",
                effect,
                "aberration",
                0.0..=64.0,
                0.5,
            );
            ui.weak("Softens the edges and separates red and blue while keeping the center clear.");
        }
        Some("Outline") => {
            number(ui, "Width", effect, "width", 0.0..=64.0, 0.5);
            choice(
                ui,
                "Placement",
                effect,
                "placement",
                &["Center", "Inward", "Outward"],
            );
        }
        Some("InnerLight") => {
            number(ui, "Offset X", effect, "offsetX", -4096.0..=4096.0, 0.5);
            number(ui, "Offset Y", effect, "offsetY", -4096.0..=4096.0, 0.5);
            number(ui, "Cutout", effect, "cutout", 0.0..=256.0, 0.5);
            crate::inspector::boolean(ui, "Affect outline", effect, "affectOutline");
            // Missing values retain the renderer's legacy enabled default.
            let mut enabled = effect["blurEnabled"].as_bool().unwrap_or(true);
            if crate::fields::checkbox(ui, "Soft light", &mut enabled).changed() {
                effect["blurEnabled"] = json!(enabled);
            }
            ui.add_enabled_ui(enabled, |ui| {
                number(ui, "Blur", effect, "blur", 0.0..=128.0, 0.5);
                color(ui, "Light color", effect, "blurColorHex");
                let mut opacity = effect["blurOpacity"].as_f64().unwrap_or(1.0) * 100.0;
                if crate::inspector::field(ui, "Light opacity", 72.0, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut opacity)
                            .range(0.0..=100.0)
                            .suffix("%"),
                    )
                    .changed()
                }) {
                    effect["blurOpacity"] = json!(opacity / 100.0);
                }
                crate::blend_preview::property_key(
                    ui,
                    "Light blend",
                    effect,
                    "blurBlendMode",
                    "Screen",
                );
            });
        }
        Some("OuterLight") => {
            number(ui, "Width", effect, "width", 0.0..=64.0, 0.5);
            number(ui, "Blur", effect, "blur", 0.0..=128.0, 0.5);
        }
        Some("Shadow") => {
            number(ui, "Offset X", effect, "offsetX", -512.0..=512.0, 1.0);
            number(ui, "Offset Y", effect, "offsetY", -512.0..=512.0, 1.0);
            number(ui, "Blur", effect, "blur", 0.0..=128.0, 0.5);
        }
        Some("BoxBlur") => number(ui, "Blur", effect, "blur", 0.0..=128.0, 0.5),
        Some("GradientOverlay") => {
            choice(
                ui,
                "Input channel",
                &mut effect["gradient"],
                "inputChannel",
                &["RGB", "Red", "Green", "Blue"],
            );
            gradient_ui(ui, &mut effect["gradient"]);
        }
        _ => {}
    }
}
fn spatial_axes(
    ui: &mut egui::Ui,
    effect: &mut Value,
    x: &str,
    y: &str,
    limit: f64,
    scale: f64,
    suffix: &str,
) {
    if ui.available_width() >= 300.0 {
        ui.columns(2, |columns| {
            spatial_number(
                &mut columns[0],
                "Horizontal",
                effect,
                x,
                -limit..=limit,
                scale,
                suffix,
            );
            spatial_number(
                &mut columns[1],
                "Vertical",
                effect,
                y,
                -limit..=limit,
                scale,
                suffix,
            );
        });
    } else {
        spatial_number(ui, "Horizontal", effect, x, -limit..=limit, scale, suffix);
        spatial_number(ui, "Vertical", effect, y, -limit..=limit, scale, suffix);
    }
}
fn spatial_number(
    ui: &mut egui::Ui,
    label: &str,
    effect: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<f64>,
    scale: f64,
    suffix: &str,
) {
    let mut value = effect[key].as_f64().unwrap_or(0.0) * scale;
    if crate::inspector::field(ui, label, 72.0, |ui| {
        ui.add(
            egui::DragValue::new(&mut value)
                .range(range)
                .speed(0.5)
                .suffix(suffix),
        )
        .changed()
    }) {
        effect[key] = json!(value / scale);
    }
}
pub(crate) fn gradient_presets(ui: &mut egui::Ui, port: &mut dyn EditorPort, gradient: &mut Value) {
    ui.app_menu_button("Gradient presets", |ui| {
        if ui.app_button("Gradient Explorer…").clicked() {
            crate::gradient_explorer::request(ui.ctx());
            ui.close();
        }
        let mut selected = None;
        for preset in port.gradients() {
            if ui.app_button(&preset.name).clicked() {
                selected = Some(preset.stops.clone());
            }
        }
        if let Some(stops) = selected {
            gradient["stops"] = json!(stops);
            ui.close();
        }
        ui.separator();
        let name_id = ui.id().with("gradient-preset-name");
        let mut name = ui
            .ctx()
            .data_mut(|data| data.get_temp::<String>(name_id))
            .unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("Preset name"));
        if ui
            .add_enabled(
                !name.trim().is_empty(),
                crate::buttons::Button::new("Save gradient preset"),
            )
            .clicked()
        {
            let stops = serde_json::from_value(gradient["stops"].clone());
            let result = stops.map_err(|e| e.to_string()).and_then(|stops| {
                port.save_gradient(achimeld_application::GradientPreset {
                    id: achimeld_application::new_id(),
                    name: name.trim().into(),
                    stops,
                })
            });
            match result {
                Ok(()) => ui.close(),
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        }
        ui.ctx().data_mut(|data| data.insert_temp(name_id, name));
    });
}
pub(crate) fn gradient_ui(ui: &mut egui::Ui, gradient: &mut Value) {
    let original = gradient.clone();
    if !gradient.is_object() {
        *gradient = json!({"kind":"Linear","angle":90.0,"stops":[{"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]});
    }
    let initial = gradient.clone();
    choice(ui, "Gradient kind", gradient, "kind", &["Linear", "Radial"]);
    number(ui, "Angle", gradient, "angle", -360.0..=360.0, 1.0);
    gradient_stops_ui(ui, gradient);
    if *gradient == initial {
        *gradient = original;
    }
}
pub(crate) fn gradient_stops_ui(ui: &mut egui::Ui, gradient: &mut Value) {
    crate::gradient::editor(ui, gradient);
}

fn adjustment_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<f64>,
    step: f64,
) {
    let default = match key {
        "unsharpAmount" => 100.0,
        "unsharpRadius" => 1.0,
        _ => 0.0,
    };
    let mut number = value[key].as_f64().unwrap_or(default);
    let decimals = if step < 1.0 { 1 } else { 0 };
    if crate::sliders::value_slider(ui, label, &mut number, range, "", decimals) {
        value[key] = json!(number);
    }
}
fn save_selective_range(value: &mut Value) {
    let range = value["colorRange"].as_str().unwrap_or("Reds").to_owned();
    let correction = json!({"range":range,"cyan":value["cyan"].as_f64().unwrap_or(0.0),"magenta":value["magenta"].as_f64().unwrap_or(0.0),"yellow":value["yellow"].as_f64().unwrap_or(0.0),"black":value["black"].as_f64().unwrap_or(0.0)});
    if !value["selectiveColors"].is_array() {
        value["selectiveColors"] = json!([]);
    }
    let colors = value["selectiveColors"]
        .as_array_mut()
        .expect("selective colors initialized");
    if let Some(old) = colors.iter_mut().find(|c| c["range"] == range) {
        *old = correction;
    } else {
        colors.push(correction);
    }
}
fn selective_range(ui: &mut egui::Ui, value: &mut Value) {
    let old = value.clone();
    let previous = value["colorRange"].as_str().unwrap_or("Reds").to_owned();
    choice(
        ui,
        "Color range",
        value,
        "colorRange",
        &[
            "Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas", "Whites", "Neutrals",
            "Blacks",
        ],
    );
    if value["colorRange"].as_str().unwrap_or("Reds") != previous {
        let next = value["colorRange"].clone();
        let mut retained = old;
        save_selective_range(&mut retained);
        value["selectiveColors"] = retained["selectiveColors"].clone();
        let selected = value["selectiveColors"]
            .as_array()
            .and_then(|colors| colors.iter().find(|c| c["range"] == next))
            .cloned()
            .unwrap_or(Value::Null);
        for key in ["cyan", "magenta", "yellow", "black"] {
            value[key] = json!(selected[key].as_f64().unwrap_or(0.0));
        }
    }
}
