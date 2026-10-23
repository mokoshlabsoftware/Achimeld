//! The text layer's five anchor choices, using the existing recipe values.
use crate::buttons::Button;
use egui::{TextureHandle, Vec2};
use serde_json::{Value, json};

const OPTIONS: [(&str, &str, &[u8]); 5] = [
    (
        "TopLeft",
        "Top left",
        include_bytes!("../assets/toolbar/text-align-top-left.rgba"),
    ),
    (
        "TopRight",
        "Top right",
        include_bytes!("../assets/toolbar/text-align-top-right.rgba"),
    ),
    (
        "Center",
        "Center",
        include_bytes!("../assets/toolbar/text-align-center.rgba"),
    ),
    (
        "BottomLeft",
        "Bottom left",
        include_bytes!("../assets/toolbar/text-align-bottom-left.rgba"),
    ),
    (
        "BottomRight",
        "Bottom right",
        include_bytes!("../assets/toolbar/text-align-bottom-right.rgba"),
    ),
];

pub(crate) fn show(ui: &mut egui::Ui, text: &mut Value) {
    let icon_size = 20.0;
    let button_size = crate::buttons::bar_height(ui, icon_size)
        .max(icon_size + 2.0 * ui.spacing().button_padding.x);
    crate::inspector::field(ui, "Alignment", button_size * OPTIONS.len() as f32, |ui| {
        ui.push_id("text-alignment", |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal_wrapped(|ui| {
                for (anchor, label, bytes) in OPTIONS {
                    let id = egui::Id::new(("text-alignment-icon", anchor));
                    let texture = ui
                        .ctx()
                        .data(|data| data.get_temp::<TextureHandle>(id))
                        .unwrap_or_else(|| {
                            let texture = ui.ctx().load_texture(
                                format!("text-alignment-{anchor}"),
                                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                                egui::TextureOptions::LINEAR
                                    .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                            );
                            ui.ctx()
                                .data_mut(|data| data.insert_temp(id, texture.clone()));
                            texture
                        });
                    // Missing values use the renderer's default. Other saved anchors
                    // remain untouched until the user explicitly chooses a button.
                    let selected = text["alignment"].as_str().unwrap_or("BottomRight") == anchor;
                    let label = crate::localization::text(ui.ctx(), label);
                    let mut response = ui
                        .add_sized(
                            Vec2::splat(button_size),
                            Button::selectable(
                                selected,
                                egui::Image::new((texture.id(), Vec2::splat(icon_size))),
                            )
                            .id(ui.id().with(anchor))
                            .bar()
                            .image_tint_follows_text_color(true),
                        )
                        .on_hover_text(&label);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            selected,
                            &label,
                        )
                    });
                    if response.clicked() && !selected {
                        text["alignment"] = json!(anchor);
                        response.mark_changed();
                    }
                }
            });
        });
    });
}
