//! Shared content field and trailing unlock-value toggle for Text and Text3d.
use crate::buttons::Button;
use egui::{Id, TextureHandle, Vec2};
use serde_json::{Value, json};

const LABEL: &str = "Link text to achievement unlock value";

pub(crate) fn control_id(layer: &str, control: &str) -> Id {
    Id::new(("layer-text-content", layer, control))
}

fn texture(ctx: &egui::Context) -> egui::TextureId {
    let id = Id::new("text-unlock-link-icon");
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture.id();
    }
    let texture = ctx.load_texture(
        "text-unlock-link",
        egui::ColorImage::from_rgba_unmultiplied(
            [64, 64],
            include_bytes!("../assets/toolbar/text-unlock-link.rgba"),
        ),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    let texture_id = texture.id();
    ctx.data_mut(|data| data.insert_temp(id, texture));
    texture_id
}

pub(crate) fn show(ui: &mut egui::Ui, layer: &str, text: &mut Value) {
    let linked = text["linkToAchievementValue"] == true;
    let mut content = text["content"].as_str().unwrap_or("").to_owned();
    let icon_size = 20.0;
    let height = crate::buttons::bar_height(ui, icon_size);
    let field_height = ui.spacing().interact_size.y;
    crate::fields::row_with_min_control(ui, "Text", height + 24.0, |ui| {
        crate::buttons::bar_with_height(ui, height, |ui| {
            let field_width = (ui.available_width() - height).max(1.0);
            let response = ui
                .scope(|ui| {
                    ui.spacing_mut().interact_size.y = field_height;
                    ui.add_enabled(
                        !linked,
                        egui::TextEdit::singleline(&mut content)
                            .id(control_id(layer, "content"))
                            .interactive(!linked)
                            .desired_width(field_width),
                    )
                })
                .inner;
            if response.changed() {
                text["content"] = json!(content);
            }
            let label = crate::localization::text(ui.ctx(), LABEL);
            let icon = texture(ui.ctx());
            let mut response = ui
                .scope(|ui| {
                    ui.spacing_mut().button_padding = Vec2::ZERO;
                    ui.add_sized(
                        Vec2::splat(height),
                        Button::selectable(
                            linked,
                            egui::Image::new((icon, Vec2::splat(icon_size))),
                        )
                        .id(control_id(layer, "link"))
                        .bar()
                        .image_tint_follows_text_color(true),
                    )
                })
                .inner;
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    response.enabled(),
                    linked,
                    &label,
                )
            });
            let frame = ui.ctx().cumulative_frame_nr();
            let activation = response.id.with("last-activation-frame");
            // egui may repeat a layout pass after the first pass has committed.
            if response.clicked() && ui.data(|data| data.get_temp::<u64>(activation)) != Some(frame)
            {
                ui.data_mut(|data| data.insert_temp(activation, frame));
                text["linkToAchievementValue"] = json!(!linked);
                response.mark_changed();
            }
            response.on_hover_text(&label).on_disabled_hover_text(label);
        });
    });
}
