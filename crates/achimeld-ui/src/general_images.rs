//! Shared achievement image explorer. Only the host performs artwork I/O.
use crate::{Desktop, buttons, icons, localization, source_picker::SourceKind};
use achimeld_application::{EditorPort, UiRequest};
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};
use serde_json::{Value, json};
pub(crate) const THUMBNAIL: f32 = 70.0;
pub(crate) const CELL: Vec2 = Vec2::new(86.0, 114.0);
pub(crate) const MIN_HEIGHT: f32 = 124.0;
#[derive(Default)]
pub(crate) struct GeneralImages {
    pub achievement: String,
    pub selected: Option<i64>,
    pub added: Option<(String, i64, u64)>,
    pub focus: Option<egui::Id>,
    pub drop_zones: std::collections::BTreeMap<u64, (egui::ViewportId, Rect, String, u64)>,
}
pub(crate) use achimeld_application::{
    append_general_image as append, general_image_entries as entries,
    store_general_image_entries as store_entries,
};
pub(crate) fn tile_uv(size: [usize; 2], settings: &Value, tile: u64) -> Option<Rect> {
    let w = settings["tileWidth"].as_u64().unwrap_or(512);
    let h = settings["tileHeight"].as_u64().unwrap_or(512);
    let left = settings["leftOffset"].as_u64().unwrap_or(0);
    let top = settings["topOffset"].as_u64().unwrap_or(0);
    let dx = w.checked_add(settings["horizontalSpacing"].as_u64().unwrap_or(0))?;
    let dy = h.checked_add(settings["verticalSpacing"].as_u64().unwrap_or(0))?;
    if w == 0 || h == 0 || dx == 0 || dy == 0 || size.contains(&0) {
        return None;
    }
    let columns = (size[0] as u64).saturating_sub(left).saturating_sub(w) / dx
        + u64::from((size[0] as u64).saturating_sub(left) >= w);
    let rows = (size[1] as u64).saturating_sub(top).saturating_sub(h) / dy
        + u64::from((size[1] as u64).saturating_sub(top) >= h);
    if columns == 0 || rows == 0 || tile >= columns.checked_mul(rows)? {
        return None;
    }
    let (x, y) = if settings["traversal"] == "ColumnMajor" {
        (tile / rows, tile % rows)
    } else {
        (tile % columns, tile / columns)
    };
    let x = left.checked_add(x.checked_mul(dx)?)?;
    let y = top.checked_add(y.checked_mul(dy)?)?;
    Some(Rect::from_min_max(
        Pos2::new(x as f32 / size[0] as f32, y as f32 / size[1] as f32),
        Pos2::new(
            (x + w) as f32 / size[0] as f32,
            (y + h) as f32 / size[1] as f32,
        ),
    ))
}
#[derive(Clone)]
struct DragEntry {
    achievement: String,
    index: usize,
}
#[derive(Default)]
struct Actions {
    copy: bool,
    paste: bool,
    use_image: bool,
}

const MENU_ICONS: [(&str, &[u8]); 6] = [
    (
        "GnImportImages",
        include_bytes!("../assets/toolbar/menu-import-images.rgba"),
    ),
    (
        "GnAtIm",
        include_bytes!("../assets/toolbar/menu-import-atlas.rgba"),
    ),
    (
        "GnBank",
        include_bytes!("../assets/toolbar/menu-choose-art.rgba"),
    ),
    ("WkCopy", include_bytes!("../assets/toolbar/menu-copy.rgba")),
    (
        "WkPste",
        include_bytes!("../assets/toolbar/menu-paste.rgba"),
    ),
    (
        "GnUseI",
        include_bytes!("../assets/toolbar/menu-use-in-icon.rgba"),
    ),
];
const MENU_ICON_SIZE: f32 = 20.0;
const MENU_ICON_GAP: f32 = 8.0;

fn menu_button(ui: &mut egui::Ui, key: &str, enabled: bool) -> egui::Response {
    let id = egui::Id::new(("general-image-menu-icon", key));
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let bytes = MENU_ICONS
                .iter()
                .find(|(label, _)| *label == key)
                .expect("Every General images menu action has an embedded icon")
                .1;
            let texture = ui.ctx().load_texture(
                format!("general-image-menu-{key}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    ui.add_enabled(
        enabled,
        buttons::Button::new((
            egui::Image::new((texture.id(), Vec2::splat(MENU_ICON_SIZE))),
            localization::text(ui.ctx(), key),
        ))
        .align(Align2::LEFT_CENTER)
        .min_size(Vec2::new(ui.available_width(), 0.0))
        .square_corners()
        .icon_spacing(MENU_ICON_GAP)
        .image_tint_follows_text_color(true),
    )
}

impl Desktop {
    pub(crate) fn general_explorer(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        entity: &mut Value,
        mut layer: Option<&mut Value>,
    ) {
        let achievement = entity["id"].as_str().unwrap_or("").to_owned();
        let mut list = entries(entity);
        if self.general_images.achievement != achievement {
            self.general_images.achievement = achievement.clone();
            self.general_images.selected = list.first().and_then(|e| e["id"].as_i64());
        }
        let mut current = layer
            .as_ref()
            .map(|l| l["generalImageIndex"].as_i64().unwrap_or(0))
            .or(self.general_images.selected);
        ui.label(egui::RichText::new(localization::text(ui.ctx(), "GnImgs")).weak());
        ui.add_space(6.0);
        let columns = ((ui.available_width() - 10.0) / CELL.x).floor().max(1.0) as usize;
        let height = (list.len().div_ceil(columns) as f32 * CELL.y + 10.0).max(MIN_HEIGHT);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        self.general_images.drop_zones.insert(
            response.id.value(),
            (
                ui.ctx().viewport_id(),
                rect.intersect(ui.clip_rect()),
                achievement.clone(),
                ui.ctx().cumulative_frame_nr(),
            ),
        );
        self.image_drop_zone(
            ui,
            rect,
            crate::image_drop::DropZone::General(achievement.clone()),
            3,
        );
        if ui.input(|i| !i.raw.hovered_files.is_empty()) {
            let viewport = rect.intersect(ui.clip_rect());
            if let Some(pointer) = self
                .image_drop
                .pointer
                .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                && viewport.contains(pointer)
                && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id())
            {
                let edge = 40.0_f32.min(viewport.height() / 3.0);
                let distance = if pointer.y < viewport.top() + edge {
                    (viewport.top() + edge - pointer.y) / edge
                } else if pointer.y > viewport.bottom() - edge {
                    -(pointer.y - viewport.bottom() + edge) / edge
                } else {
                    0.0
                };
                if distance != 0.0 {
                    ui.scroll_with_delta(Vec2::new(
                        0.0,
                        distance * 500.0 * ui.input(|i| i.stable_dt.min(0.05)),
                    ));
                    ui.ctx().request_repaint();
                }
            }
        }
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, 0.0, ui.visuals().widgets.inactive.bg_fill);
        painter.rect_stroke(
            rect,
            0.0,
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let notice = port
            .general_image_added()
            .into_iter()
            .chain(self.general_images.added.clone())
            .max_by_key(|n| n.2);
        if let Some((owner, index, revision)) = notice
            && owner == achievement
            && revision == port.editor().revision()
            && ui.data(|data| data.get_temp::<u64>(response.id.with("import-selection")))
                != Some(revision)
        {
            self.general_images.selected = Some(index);
            current = Some(index);
            if let Some(layer) = layer.as_mut() {
                layer["generalImageIndex"] = json!(index);
            }
            ui.data_mut(|data| data.insert_temp(response.id.with("import-selection"), revision));
        }
        let mut selected = None;
        let mut remove = None;
        let mut reorder = None;
        let mut actions = Actions::default();
        if response.clicked() || response.secondary_clicked() {
            response.request_focus();
            self.general_images.focus = Some(response.id);
        }
        for (index, entry) in list.iter().enumerate() {
            let id = entry["id"].as_i64().unwrap_or(index as i64);
            let cell = Rect::from_min_size(
                rect.min
                    + Vec2::new(
                        5.0 + (index % columns) as f32 * CELL.x,
                        5.0 + (index / columns) as f32 * CELL.y,
                    ),
                CELL.min(Vec2::new((rect.width() - 10.0).max(32.0), CELL.y)),
            );
            let item = ui.interact(cell, response.id.with(id), Sense::click_and_drag());
            if current == Some(id) {
                painter.rect_stroke(
                    cell,
                    0,
                    egui::Stroke::new(2.0, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            if item.clicked() || item.secondary_clicked() {
                selected = Some(id);
                response.request_focus();
                self.general_images.focus = Some(response.id);
            }
            if item.drag_started() {
                item.dnd_set_drag_payload(DragEntry {
                    achievement: achievement.clone(),
                    index,
                });
                response.request_focus();
            }
            if let Some(drag) = item.dnd_release_payload::<DragEntry>()
                && drag.achievement == achievement
                && drag.index != index
            {
                reorder = Some((drag.index, index));
            }
            let image_rect = Rect::from_min_size(
                cell.min + Vec2::new(8.0, 6.0),
                Vec2::splat(THUMBNAIL.min((cell.width() - 16.0).max(16.0))),
            );
            let atlas = !entry["atlasId"].is_null();
            let art_image = entry["artImageId"].as_str();
            let empty = !atlas && entry["imageId"].is_null() && art_image.is_none();
            let asset = entry[if atlas { "atlasId" } else { "imageId" }]
                .as_str()
                .filter(|id| *id != "00000000-0000-0000-0000-000000000000");
            let preview = if let Some(art) = art_image {
                port.canvas_preview(art, 128)
            } else if !empty {
                port.asset_preview(atlas, asset)
            } else {
                None
            };
            if let Some(preview) = preview {
                let uv = if atlas {
                    tile_uv(
                        preview.source_size,
                        &port.editor().document().project()["atlas"],
                        entry["tileIndex"].as_u64().unwrap_or(0),
                    )
                } else {
                    Some(Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)))
                };
                let dimensions = uv.map(|uv| {
                    Vec2::new(
                        uv.width() * preview.source_size[0] as f32,
                        uv.height() * preview.source_size[1] as f32,
                    )
                });
                if let (Some(uv), Some(dimensions), Some(texture)) = (
                    uv,
                    dimensions,
                    self.preview_texture(
                        ui.ctx(),
                        &format!(
                            "general:{atlas}:{}",
                            art_image.or(asset).unwrap_or("default")
                        ),
                        preview,
                    ),
                ) {
                    let size = dimensions
                        * (image_rect.width() / dimensions.x.max(1.0))
                            .min(image_rect.height() / dimensions.y.max(1.0));
                    painter.image(
                        texture,
                        Rect::from_center_size(image_rect.center(), size),
                        uv,
                        Color32::WHITE,
                    );
                }
            } else {
                icons::paint(
                    &painter,
                    image_rect.shrink(20.0),
                    if atlas {
                        icons::Icon::Atlas
                    } else {
                        icons::Icon::Layers
                    },
                    ui.visuals().weak_text_color(),
                );
            }
            let close_rect = Rect::from_min_size(
                image_rect.right_top() - Vec2::new(20.0, 0.0),
                Vec2::splat(22.0),
            );
            ui.scope(|ui| {
                ui.spacing_mut().button_padding = Vec2::ZERO;
                ui.spacing_mut().interact_size = Vec2::splat(22.0);
                ui.visuals_mut().widgets.inactive.corner_radius = egui::CornerRadius::ZERO;
                let close = ui.put(
                    close_rect,
                    buttons::Button::new("").min_size(Vec2::splat(22.0)),
                );
                let (_, ink) = buttons::colors(ui, &close, buttons::ButtonStyle::Standard);
                icons::paint(
                    &painter,
                    Rect::from_center_size(close_rect.center(), Vec2::splat(10.0)),
                    icons::Icon::Close,
                    ink,
                );
                if close.clicked() {
                    remove = Some(index);
                    response.request_focus();
                }
                close.on_hover_text(localization::text(ui.ctx(), "AtRemo"));
            });
            painter.text(
                Pos2::new(cell.center().x, cell.top() + 80.0),
                Align2::CENTER_TOP,
                localization::text(ui.ctx(), "GnIdTx").replace("{0}", &id.to_string()),
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            let source = if empty {
                localization::text(ui.ctx(), "Empty image")
            } else if atlas {
                localization::text(ui.ctx(), "GnAtTl")
                    .replace("{0}", &entry["tileIndex"].as_u64().unwrap_or(0).to_string())
            } else {
                localization::text(ui.ctx(), "AtImag")
            };
            let mut job = egui::text::LayoutJob::simple(
                source.clone(),
                egui::TextStyle::Small.resolve(ui.style()),
                ui.visuals().text_color(),
                cell.width() - 4.0,
            );
            job.wrap.max_rows = 1;
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            painter.galley(
                Pos2::new(cell.center().x - galley.size().x / 2.0, cell.top() + 96.0),
                galley,
                ui.visuals().text_color(),
            );
            buttons::context_menu(&item.on_hover_text(source), |ui| {
                self.general_menu(ui, &achievement, true, &mut actions)
            });
        }
        buttons::context_menu(&response, |ui| {
            self.general_menu(ui, &achievement, current.is_some(), &mut actions)
        });
        if reorder.is_none()
            && let Some(drag) = response.dnd_release_payload::<DragEntry>()
            && drag.achievement == achievement
            && !list.is_empty()
        {
            reorder = Some((drag.index, list.len() - 1));
        }
        if response.has_focus() {
            actions.copy |= ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::C));
            actions.paste |= ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::V));
            actions.copy |= ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
            actions.paste |=
                ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))));
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
                remove = current.and_then(|id| list.iter().position(|e| e["id"] == id));
            }
        }
        if actions.copy
            && let Some(entry) = selected
                .or(current)
                .and_then(|id| list.iter().find(|e| e["id"] == id))
        {
            self.request(UiRequest::CopyGeneralImage {
                source: entry.clone(),
            });
        }
        if actions.paste {
            self.request(UiRequest::PasteGeneralImage {
                achievement_id: achievement.clone(),
            });
        }
        let choose_source = selected.is_some();
        if let Some(index) = remove.filter(|i| *i < list.len()) {
            list.remove(index);
            store_entries(entity, list.clone());
            selected = Some(if list.is_empty() { -1 } else { 0 });
            port.finish_group();
        }
        if let Some((from, to)) =
            reorder.filter(|(from, to)| *from < list.len() && *to < list.len() && from != to)
        {
            let entry = list.remove(from);
            list.insert(to, entry);
            store_entries(entity, list);
            selected = Some(to as i64);
            port.finish_group();
        }
        if let Some(id) = selected {
            self.general_images.selected = (id >= 0).then_some(id);
            if choose_source
                && id >= 0
                && let Some(layer) = layer.as_mut()
            {
                layer["generalImageIndex"] = json!(id);
            }
        }
        if actions.use_image
            && let Some(id) = self.general_images.selected.or(current)
        {
            let mut icon = crate::layer_palette::effective_icon(entity);
            let mut layer = crate::layer_palette::new_layer("AchievementImage");
            layer["generalImageIndex"] = json!(id);
            self.selected_layer = layer["id"].as_str().map(str::to_owned);
            if let Some(layers) = icon["layers"].as_array_mut() {
                layers.push(layer);
            }
            entity["icon"] = icon;
            self.open_layers();
        }
    }
    fn general_menu(
        &mut self,
        ui: &mut egui::Ui,
        achievement: &str,
        selected: bool,
        actions: &mut Actions,
    ) {
        // Measure all labels up front so every row shares the same icon and text
        // columns, including disabled actions and translated labels.
        let width = MENU_ICONS
            .iter()
            .map(|(key, _)| {
                ui.painter()
                    .layout_no_wrap(
                        localization::text(ui.ctx(), key),
                        egui::TextStyle::Button.resolve(ui.style()),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max)
            + MENU_ICON_SIZE
            + MENU_ICON_GAP
            + 2.0 * ui.spacing().button_padding.x;
        ui.set_width(width);
        if menu_button(ui, "GnImportImages", true).clicked() {
            self.request(UiRequest::ImportGeneralImage {
                achievement_id: achievement.into(),
            });
            ui.close();
        }
        if menu_button(ui, "GnAtIm", true).clicked() {
            self.open_general_picker(ui.ctx(), achievement, SourceKind::Atlas);
            ui.close();
        }
        if menu_button(ui, "GnBank", true).clicked() {
            self.open_general_picker(ui.ctx(), achievement, SourceKind::Image);
            ui.close();
        }
        ui.separator();
        if menu_button(ui, "WkCopy", selected).clicked() {
            actions.copy = true;
            ui.close();
        }
        if menu_button(ui, "WkPste", true).clicked() {
            actions.paste = true;
            ui.close();
        }
        ui.separator();
        if menu_button(ui, "GnUseI", selected).clicked() {
            actions.use_image = true;
            ui.close();
        }
    }
}
