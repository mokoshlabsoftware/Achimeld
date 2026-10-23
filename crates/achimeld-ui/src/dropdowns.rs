//! Application dropdowns: shared buttons, full short lists, and one popup contract.
use crate::buttons::{self, Button};

// A geometric disclosure marker avoids relying on a font's symbol coverage.
pub(crate) fn disclosure(ui: &egui::Ui) -> egui::Image<'static> {
    let id = egui::Id::new("dropdown-disclosure");
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let pixels = (0..16)
                .flat_map(|y| {
                    (0..32).map(move |x| {
                        let coverage = (16.0 - y as f32 - (x as f32 - 15.5).abs()).clamp(0.0, 1.0);
                        egui::Color32::from_white_alpha((coverage * 255.0) as u8)
                    })
                })
                .collect();
            let texture = ui.ctx().load_texture(
                "dropdown-disclosure",
                egui::ColorImage::new([32, 16], pixels),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), egui::vec2(8.0, 4.0)))
}

pub(crate) fn is_popup(ui: &egui::Ui) -> bool {
    ui.stack()
        .iter()
        .any(|stack| matches!(stack.kind(), Some(egui::UiKind::Menu | egui::UiKind::Popup)))
}

pub(crate) fn contents<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let global = ui.ctx().global_style();
    for style in [
        egui::TextStyle::Button,
        egui::TextStyle::Body,
        egui::TextStyle::Small,
    ] {
        let mut font = style.resolve(&global);
        font.size = (font.size - 1.0).max(1.0);
        ui.style_mut().text_styles.insert(style, font);
    }
    content(ui)
}

pub(crate) struct Dropdown {
    salt: egui::Id,
    width: Option<f32>,
    selected: egui::WidgetText,
}

impl Dropdown {
    pub(crate) fn from_id_salt(salt: impl egui::AsId) -> Self {
        Self {
            salt: egui::Id::new(salt),
            width: None,
            selected: Default::default(),
        }
    }
    pub(crate) fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub(crate) fn truncate(self) -> Self {
        self
    }
    pub(crate) fn selected_text(mut self, text: impl Into<egui::WidgetText>) -> Self {
        self.selected = text.into();
        self
    }
    pub(crate) fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let width = self
            .width
            .unwrap_or(ui.spacing().combo_width)
            .min(ui.available_width());
        ui.scope(|ui| {
            let mut font = egui::TextStyle::Button.resolve(&ui.ctx().global_style());
            font.size = (font.size - 1.0).max(1.0);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, font);
            let response = ui.add_sized(
                [width, ui.spacing().interact_size.y],
                Button::new((self.selected, disclosure(ui)))
                    .image_tint_follows_text_color(true)
                    .id(ui.make_persistent_id(self.salt))
                    .bar()
                    .wrap_mode(egui::TextWrapMode::Truncate),
            );
            if !response.enabled() {
                egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&response));
            }
            let inner = buttons::menu_popup(ui, &response, |ui| {
                ui.set_width(width);
                egui::ScrollArea::vertical()
                    .max_height((ui.ctx().content_rect().height() * 0.75).max(32.0))
                    .auto_shrink([false, true])
                    .show(ui, content)
                    .inner
            });
            egui::InnerResponse { response, inner }
        })
        .inner
    }
}
