//! Generated, palette-tinted artwork for the compact layer action bar.
use crate::buttons::Button;
use egui::{TextureHandle, Vec2};

pub(crate) const HEIGHT: f32 = 34.0;
pub(crate) const ACTION_COUNT: usize = 8;

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Create,
    Folder,
    Up,
    Down,
    Delete,
    Import,
    Background,
    Reroll,
}

impl Action {
    fn key(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Folder => "folder",
            Self::Up => "up",
            Self::Down => "down",
            Self::Delete => "delete",
            Self::Import => "import",
            Self::Background => "background",
            Self::Reroll => "reroll",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Create => "Create layer",
            Self::Folder => "Create folder",
            Self::Up => "Move layer up",
            Self::Down => "Move layer down",
            Self::Delete => "Delete layer",
            Self::Import => "Import…",
            Self::Background => "Background",
            Self::Reroll => "Re-roll all random layers",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Create => include_bytes!("../assets/toolbar/layer-action-create.rgba"),
            Self::Folder => include_bytes!("../assets/toolbar/layer-action-folder.rgba"),
            Self::Up => include_bytes!("../assets/toolbar/layer-action-up.rgba"),
            Self::Down => include_bytes!("../assets/toolbar/layer-action-down.rgba"),
            Self::Delete => include_bytes!("../assets/toolbar/layer-action-delete.rgba"),
            Self::Import => include_bytes!("../assets/toolbar/layer-action-import.rgba"),
            Self::Background => include_bytes!("../assets/toolbar/layer-action-background.rgba"),
            Self::Reroll => include_bytes!("../assets/toolbar/layer-action-reroll.rgba"),
        }
    }

    pub(crate) fn id(self) -> egui::Id {
        crate::layer_palette::layers_panel_id().with(("footer", self.key()))
    }

    pub(crate) fn button(self, ui: &mut egui::Ui, size: Vec2, enabled: bool) -> egui::Response {
        let id = egui::Id::new(("layer-action-icon", self.key()));
        // The finite, compiled action set bounds the texture cache.
        let texture = ui
            .ctx()
            .data(|data| data.get_temp::<TextureHandle>(id))
            .unwrap_or_else(|| {
                let texture = ui.ctx().load_texture(
                    format!("layer-action-{}", self.key()),
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], self.bytes()),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                ui.ctx()
                    .data_mut(|data| data.insert_temp(id, texture.clone()));
                texture
            });
        let glyph = Vec2::splat((size.min_elem() - 8.0).clamp(1.0, 24.0));
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().button_padding = Vec2::ZERO;
                ui.add_enabled(
                    enabled,
                    Button::new(egui::Image::new((texture.id(), glyph)))
                        .id(self.id())
                        .bar()
                        .min_size(size)
                        .image_tint_follows_text_color(true),
                )
            })
            .inner;
        let label = crate::localization::text(ui.ctx(), self.label());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
        });
        response
            .on_hover_cursor(egui::CursorIcon::Default)
            .on_hover_text(label)
    }
}
