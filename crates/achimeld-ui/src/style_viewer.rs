//! Presentation for a host-owned preview document, independent of the canvas.
mod deletion;
mod layout;
pub(crate) struct DeleteStyle {
    project: String,
    style: String,
    replacement: Option<String>,
}
use crate::{Desktop, buttons, icons};
use achimeld_application::{
    EditorPort, UiRequest,
    style_viewer::{StyleViewerAction as Action, StyleViewerState},
};

#[derive(Clone, Copy, Default, PartialEq)]
enum BankTab {
    #[default]
    All,
    General,
    Icon,
    Atlases,
}

pub(crate) struct ViewerUi {
    tab: BankTab,
    search: String,
    name_filter: String,
    atlas: Option<String>,
    atlas_open: bool,
    unlock: String,
    saved_unlock: String,
    slot: usize,
    banner_height: f32,
    pub(crate) options_open: bool,
    builtins_open: bool,
    project_open: bool,
    zoom: f32,
    pan: egui::Vec2,
    pub(crate) deletion: Option<DeleteStyle>,
    resolution_power: u32,
}

impl Default for ViewerUi {
    fn default() -> Self {
        Self {
            tab: BankTab::default(),
            search: String::new(),
            name_filter: String::new(),
            atlas: None,
            atlas_open: false,
            unlock: String::new(),
            saved_unlock: String::new(),
            slot: 0,
            banner_height: 64.0,
            options_open: false,
            builtins_open: true,
            project_open: true,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            deletion: None,
            resolution_power: 9,
        }
    }
}

fn glyph(ctx: &egui::Context, name: &'static str, bytes: &'static [u8]) -> egui::Image<'static> {
    let id = egui::Id::new(("viewer-glyph", name));
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
    egui::Image::new((texture.id(), egui::Vec2::splat(20.0)))
}

fn fit(size: egui::Vec2, bounds: egui::Rect) -> egui::Rect {
    let scale = (bounds.width() / size.x.max(1.0)).min(bounds.height() / size.y.max(1.0));
    egui::Rect::from_center_size(bounds.center(), size * scale)
}

impl Desktop {
    fn viewer_texture(
        &mut self,
        ui: &egui::Ui,
        port: &mut dyn EditorPort,
        style: &str,
        edge: usize,
    ) -> Option<(egui::TextureId, egui::Vec2)> {
        let key = format!("viewer-style:{edge}:{style}");
        if let Some(preview) = port.style_viewer_style_preview(style, edge) {
            self.preview_texture(ui.ctx(), &key, preview);
        }
        self.textures
            .get(&key)
            .map(|(_, texture, _)| (texture.id(), texture.size_vec2()))
    }

    fn viewer_style_menu(
        &mut self,
        response: &egui::Response,
        port: &dyn EditorPort,
        style: &achimeld_application::IconStyle,
        _state: &StyleViewerState,
        busy: bool,
        actions: &mut Vec<Action>,
    ) {
        buttons::context_menu(response, |ui| {
            if ui
                .add_enabled(!busy, buttons::Button::new("Duplicate into project").bar())
                .clicked()
            {
                actions.push(Action::DuplicateStyle(style.id.clone()));
                ui.close();
            }
            let editable = !style.id.contains('/');
            if ui
                .add_enabled(
                    !busy && editable,
                    buttons::Button::new("Rename style…")
                        .id(egui::Id::new("viewer-rename-menu"))
                        .bar(),
                )
                .clicked()
            {
                self.open_viewer_style_rename(port, style);
                ui.close();
            }
            if ui
                .add_enabled(
                    !busy && editable,
                    buttons::Button::new("Delete project style…").bar(),
                )
                .clicked()
            {
                self.open_style_delete(port, &style.id);
                ui.close();
            }
        });
    }

    fn viewer_bank(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        egui::Frame::new().inner_margin(8).show(ui, |ui| {
            ui.strong("Preview artwork");
            ui.weak(format!(
                "Choose an image or atlas tile for ID {}.",
                viewer.slot
            ));
        });
        egui::ScrollArea::horizontal()
            .id_salt("viewer-bank-tabs")
            .auto_shrink([false, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                buttons::bar(ui, 24.0, |ui| {
                    for (tab, label, bytes) in [
                        (
                            BankTab::All,
                            "All",
                            include_bytes!("../assets/toolbar/tab-art-bank.rgba").as_slice(),
                        ),
                        (
                            BankTab::General,
                            "General",
                            include_bytes!("../assets/toolbar/art-general-images.rgba").as_slice(),
                        ),
                        (
                            BankTab::Icon,
                            "Icons",
                            include_bytes!("../assets/toolbar/art-icon-images.rgba").as_slice(),
                        ),
                        (
                            BankTab::Atlases,
                            "Atlases",
                            include_bytes!("../assets/toolbar/art-atlases.rgba").as_slice(),
                        ),
                    ] {
                        let icon = glyph(ui.ctx(), label, bytes);
                        if ui
                            .add(
                                buttons::Button::selectable(viewer.tab == tab, (icon, label))
                                    .bar()
                                    .icon_spacing(5.0)
                                    .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            viewer.tab = tab;
                            viewer.atlas_open = false;
                        }
                    }
                });
            });
        egui::Frame::new().inner_margin(8).show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut viewer.search)
                    .hint_text("Find artwork")
                    .desired_width(ui.available_width()),
            );
        });
        let project = port.editor().document().project();
        let (general, icon) = crate::art_bank::image_categories(project);
        let mut entries = Vec::new();
        if viewer.tab != BankTab::Atlases {
            for asset in project["assets"]["images"].as_array().into_iter().flatten() {
                let Some(id) = asset["id"].as_str() else {
                    continue;
                };
                if match viewer.tab {
                    BankTab::General => general.contains(id),
                    BankTab::Icon => icon.contains(id) || !general.contains(id),
                    _ => true,
                } {
                    entries.push((
                        false,
                        Some(id.to_owned()),
                        asset["displayName"].as_str().unwrap_or("Image").to_owned(),
                    ));
                }
            }
        }
        if matches!(viewer.tab, BankTab::All | BankTab::Atlases) {
            if !project["assets"]["atlas"].is_null() {
                entries.push((true, None, "Default atlas".into()));
            }
            for asset in project["assets"]["atlases"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(id) = asset["id"].as_str() {
                    entries.push((
                        true,
                        Some(id.to_owned()),
                        asset["displayName"].as_str().unwrap_or("Atlas").to_owned(),
                    ));
                }
            }
        }
        let search = viewer.search.to_lowercase();
        entries.retain(|(_, _, name)| name.to_lowercase().contains(&search));
        let atlas_settings = project["atlas"].clone();
        egui::ScrollArea::vertical()
            .id_salt("viewer-bank-images")
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                if viewer.atlas_open {
                    if ui
                        .add(buttons::Button::new("Back to artwork").bar())
                        .clicked()
                    {
                        viewer.atlas_open = false;
                    }
                    if let Some(preview) = port.asset_preview(true, viewer.atlas.as_deref()) {
                        let source = preview.source_size;
                        let size = egui::vec2(preview.width as f32, preview.height as f32);
                        let texture = self.preview_texture(
                            ui.ctx(),
                            &format!(
                                "viewer-atlas:{}",
                                viewer.atlas.as_deref().unwrap_or("default")
                            ),
                            preview,
                        );
                        let bounds = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(ui.available_width(), 260.0),
                        );
                        let display = fit(size, bounds).size();
                        let (rect, response) =
                            ui.allocate_exact_size(display, egui::Sense::click());
                        if let Some(texture) = texture {
                            ui.painter().image(
                                texture,
                                rect,
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                        let settings = &atlas_settings;
                        if let Some(uv) = match state.preview.slot(viewer.slot) {
                            Some(
                                achimeld_application::style_viewer::StyleViewerSource::Atlas {
                                    id,
                                    tile,
                                },
                            ) if *id == viewer.atlas => {
                                crate::source_picker::tile_uv(source, settings, *tile)
                            }
                            _ => None,
                        } {
                            let selected = egui::Rect::from_min_max(
                                rect.min + uv.min.to_vec2() * rect.size(),
                                rect.min + uv.max.to_vec2() * rect.size(),
                            );
                            ui.painter().rect_stroke(
                                selected,
                                0,
                                egui::Stroke::new(2.0, ui.visuals().selection.bg_fill),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked()
                            && let Some(pos) = response.interact_pointer_pos()
                            && let Some(tile) = crate::source_picker::tile_at(
                                source,
                                settings,
                                [
                                    ((pos.x - rect.left()) / rect.width()) as f64
                                        * source[0] as f64,
                                    ((pos.y - rect.top()) / rect.height()) as f64
                                        * source[1] as f64,
                                ],
                            )
                        {
                            actions.push(Action::SetImageSlot {
                                index: viewer.slot,
                                source: Some(
                                    achimeld_application::style_viewer::StyleViewerSource::Atlas {
                                        id: viewer.atlas.clone(),
                                        tile,
                                    },
                                ),
                            });
                        }
                        ui.weak("Click a tile to preview it.");
                        crate::fields::row(ui, "Tile index", |ui| {
                            let mut tile = match state.preview.slot(viewer.slot) {
                                Some(
                                    achimeld_application::style_viewer::StyleViewerSource::Atlas {
                                        id,
                                        tile,
                                    },
                                ) if *id == viewer.atlas => *tile,
                                _ => 0,
                            };
                            if ui
                                .add(egui::DragValue::new(&mut tile).range(0..=i32::MAX as u64))
                                .changed()
                                && crate::source_picker::tile_uv(source, settings, tile).is_some()
                            {
                                actions.push(Action::SetImageSlot { index:viewer.slot, source:Some(
                                    achimeld_application::style_viewer::StyleViewerSource::Atlas {
                                        id: viewer.atlas.clone(),
                                        tile,
                                    },
                                ) });
                            }
                        });
                    } else {
                        ui.spinner();
                        ui.weak("Atlas unavailable or loading. Repair it in Art Data Bank.");
                    }
                    return;
                }
                if entries.is_empty() {
                    ui.weak("No matching artwork. Import images in Art Data Bank.");
                }
                let columns = (ui.available_width() / 112.0).floor().max(1.0) as usize;
                let width = ui.available_width() / columns as f32;
                let height =
                    width * 0.65 + 2.0 * ui.text_style_height(&egui::TextStyle::Small) + 8.0;
                let rows = entries.len().div_ceil(columns);
                let (_, grid) =
                    ui.allocate_space(egui::vec2(ui.available_width(), rows as f32 * height));
                let first = (viewport.top() / height).floor().max(0.0) as usize;
                let last = ((viewport.bottom() / height).ceil() as usize + 1).min(rows);
                for row in first..last {
                    for column in 0..columns {
                        let Some((atlas, id, name)) = entries.get(row * columns + column) else {
                            break;
                        };
                        let rect = egui::Rect::from_min_size(
                            grid.min + egui::vec2(column as f32 * width, row as f32 * height),
                            egui::vec2(width, height),
                        );
                        let selected = match state.preview.slot(viewer.slot) {
                            Some(
                                achimeld_application::style_viewer::StyleViewerSource::Image {
                                    id: chosen,
                                },
                            ) => !atlas && id.as_ref() == Some(chosen),
                            Some(
                                achimeld_application::style_viewer::StyleViewerSource::Atlas {
                                    id: chosen,
                                    ..
                                },
                            ) => *atlas && chosen == id,
                            None => false,
                        };
                        let mut tile_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        let response = tile_ui.add_sized(
                            rect.size(),
                            buttons::Button::selectable(selected, "")
                                .bar()
                                .id(egui::Id::new(("viewer-bank-image", atlas, id))),
                        );
                        let (_, ink) = buttons::colors(
                            &tile_ui,
                            &response,
                            if selected {
                                buttons::ButtonStyle::Accent
                            } else {
                                buttons::ButtonStyle::Quiet
                            },
                        );
                        let image_rect = egui::Rect::from_min_size(
                            rect.min + egui::Vec2::splat(8.0),
                            egui::vec2((width - 16.0).max(1.0), (width * 0.65 - 16.0).max(1.0)),
                        );
                        if let Some(preview) = port.asset_preview(*atlas, id.as_deref()) {
                            let size = egui::vec2(preview.width as f32, preview.height as f32);
                            if let Some(texture) = self.preview_texture(
                                ui.ctx(),
                                &format!(
                                    "viewer-bank:{atlas}:{}",
                                    id.as_deref().unwrap_or("default")
                                ),
                                preview,
                            ) {
                                ui.painter().image(
                                    texture,
                                    fit(size, image_rect),
                                    egui::Rect::from_min_max(
                                        egui::Pos2::ZERO,
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            }
                        } else {
                            icons::paint(
                                ui.painter(),
                                egui::Rect::from_center_size(
                                    image_rect.center(),
                                    egui::Vec2::splat(24.0),
                                ),
                                icons::Icon::Layers,
                                ink,
                            );
                        }
                        let label = egui::Rect::from_min_max(
                            egui::pos2(rect.left() + 4.0, image_rect.bottom() + 4.0),
                            rect.max - egui::vec2(4.0, 0.0),
                        );
                        let mut job = egui::text::LayoutJob::simple(
                            if *atlas {
                                format!("{name} · Atlas")
                            } else {
                                name.clone()
                            },
                            egui::TextStyle::Small.resolve(ui.style()),
                            ink,
                            label.width().max(1.0),
                        );
                        job.wrap.max_rows = 2;
                        job.halign = egui::Align::Center;
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        ui.painter().with_clip_rect(label).add(
                            egui::epaint::TextShape::new(
                                egui::pos2(label.center().x, label.top()),
                                galley,
                                ink,
                            )
                            .with_override_text_color(ink),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                response.enabled(),
                                selected,
                                name,
                            )
                        });
                        if response.on_hover_text(name).clicked() {
                            if *atlas {
                                viewer.atlas = id.clone();
                                viewer.atlas_open = true;
                            } else if let Some(id) = id {
                                actions.push(Action::SetImageSlot { index:viewer.slot, source:Some(
                                    achimeld_application::style_viewer::StyleViewerSource::Image {
                                        id: id.clone(),
                                    },
                                ) });
                            }
                        }
                    }
                }
            });
    }
}

fn clicked_once(response: &egui::Response) -> bool {
    if !response.clicked() {
        return false;
    }
    let frame = response.ctx.cumulative_frame_nr();
    response.ctx.data_mut(|data| {
        let key = response.id.with("activation-frame");
        if data.get_temp::<u64>(key) == Some(frame) {
            false
        } else {
            data.insert_temp(key, frame);
            true
        }
    })
}
