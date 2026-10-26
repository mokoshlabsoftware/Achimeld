use crate::buttons::UiButtonExt as _;
use crate::{Desktop, icons, theme};
use achimeld_application::{Command, EditorPort, EntityKind, UiRequest};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::{Value, json};

const SOURCE_TILE_WIDTH: f32 = 144.0;
const SOURCE_TILE_IMAGE: f32 = 128.0;
const SOURCE_TILE_GAP: f32 = 12.0;
const SOURCE_GRID_PADDING: f32 = 6.0;

fn source_tile(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    title: &str,
    preview: Option<(egui::TextureId, Vec2)>,
    selected: bool,
) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            response.enabled(),
            selected,
            title,
        )
    });
    let progress = ui.ctx().animate_bool_with_time_and_easing(
        id.with("source-tile-hover"),
        response.enabled() && (response.hovered() || response.has_focus()),
        0.2,
        egui::emath::easing::cubic_in_out,
    );
    // Only the paint scales: grid geometry and hit targets stay stable during hover.
    let scale = 1.0 + 0.03 * progress;
    let transform = egui::emath::TSTransform::new(rect.center().to_vec2() * (1.0 - scale), scale);
    let mut paint_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    paint_ui.with_visual_transform(transform, |ui| {
        let painter = ui.painter();
        let ink = ui.visuals().text_color();
        let fill = ui.visuals().widgets.inactive.bg_fill;
        let radius = egui::CornerRadius::ZERO;
        painter.rect_filled(rect, radius, fill);
        let image_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 8.0 + SOURCE_TILE_IMAGE / 2.0),
            Vec2::new((rect.width() - 16.0).max(1.0), SOURCE_TILE_IMAGE),
        );
        if let Some((texture, size)) = preview {
            let fit = (image_rect.width() / size.x).min(image_rect.height() / size.y);
            painter.image(
                texture,
                Rect::from_center_size(image_rect.center(), size * fit),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            icons::paint(
                painter,
                Rect::from_center_size(image_rect.center(), Vec2::splat(32.0)),
                icons::Icon::Layers,
                ui.visuals().weak_text_color(),
            );
        }
        let label_rect = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, image_rect.bottom() + 4.0),
            rect.right_bottom() - Vec2::splat(8.0),
        );
        let mut job = egui::text::LayoutJob::simple(
            title.into(),
            egui::TextStyle::Button.resolve(ui.style()),
            ink,
            label_rect.width(),
        );
        job.wrap.max_rows = 2;
        job.halign = egui::Align::Center;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        painter.galley(
            Pos2::new(
                label_rect.center().x,
                label_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            ink,
        );
        if selected || response.clicked() || progress > 0.0 {
            painter.rect_stroke(
                rect,
                radius,
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    Color32::WHITE.gamma_multiply(if selected || response.clicked() {
                        1.0
                    } else {
                        progress
                    }),
                ),
                egui::StrokeKind::Inside,
            );
        }
    });
    response.on_hover_text(title)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SourceKind {
    Image,
    Atlas,
    General,
}
pub(crate) struct SourcePicker {
    pub viewport: egui::ViewportId,
    project: String,
    achievement: String,
    layer: String,
    mode: SourceKind,
    asset: Option<String>,
    tile: u64,
    general: i64,
    append_general: bool,
}

pub(crate) struct TileGeometry {
    w: u64,
    h: u64,
    left: u64,
    top: u64,
    dx: u64,
    dy: u64,
    columns: u64,
    rows: u64,
}
pub(crate) fn tile_geometry(size: [usize; 2], settings: &Value) -> Option<TileGeometry> {
    let w = settings["tileWidth"].as_u64().unwrap_or(512);
    let h = settings["tileHeight"].as_u64().unwrap_or(512);
    let left = settings["leftOffset"].as_u64().unwrap_or(0);
    let top = settings["topOffset"].as_u64().unwrap_or(0);
    let dx = w.checked_add(settings["horizontalSpacing"].as_u64().unwrap_or(0))?;
    let dy = h.checked_add(settings["verticalSpacing"].as_u64().unwrap_or(0))?;
    if w == 0 || h == 0 || dx == 0 || dy == 0 {
        return None;
    }
    let columns = (size[0] as u64).saturating_sub(left).saturating_sub(w) / dx
        + u64::from((size[0] as u64).saturating_sub(left) >= w);
    let rows = (size[1] as u64).saturating_sub(top).saturating_sub(h) / dy
        + u64::from((size[1] as u64).saturating_sub(top) >= h);
    Some(TileGeometry {
        w,
        h,
        left,
        top,
        dx,
        dy,
        columns,
        rows,
    })
}
pub(crate) fn tile_at(size: [usize; 2], settings: &Value, point: [f64; 2]) -> Option<u64> {
    let TileGeometry {
        w,
        h,
        left,
        top,
        dx,
        dy,
        columns,
        rows,
    } = tile_geometry(size, settings)?;
    if point.iter().any(|p| !p.is_finite() || *p < 0.0)
        || point[0] < left as f64
        || point[1] < top as f64
    {
        return None;
    }
    let x = point[0] as u64 - left;
    let y = point[1] as u64 - top;
    let c = x / dx;
    let r = y / dy;
    if c >= columns || r >= rows || x % dx >= w || y % dy >= h {
        return None;
    }
    Some(if settings["traversal"] == "ColumnMajor" {
        c * rows + r
    } else {
        r * columns + c
    })
}
pub(crate) fn tile_uv(size: [usize; 2], settings: &Value, tile: u64) -> Option<Rect> {
    let geometry = tile_geometry(size, settings)?;
    let TileGeometry {
        w,
        h,
        left,
        top,
        dx,
        dy,
        columns,
        rows,
    } = geometry;
    if columns == 0 || rows == 0 || tile >= columns.checked_mul(rows)? {
        return None;
    }
    let (column, row) = if settings["traversal"] == "ColumnMajor" {
        (tile / rows, tile % rows)
    } else {
        (tile % columns, tile / columns)
    };
    let point = |x: u64, y: u64| {
        Pos2::new(
            x as f32 / size[0].max(1) as f32,
            y as f32 / size[1].max(1) as f32,
        )
    };
    Some(Rect::from_min_max(
        point(left + column * dx, top + row * dy),
        point(left + column * dx + w, top + row * dy + h),
    ))
}
impl Desktop {
    pub(crate) fn open_source_picker(
        &mut self,
        ctx: &egui::Context,
        port: &dyn EditorPort,
        layer_id: &str,
        mode: SourceKind,
    ) {
        let Some(achievement) = self.layer_edit_target(port) else {
            return;
        };
        let Some(entity) = self.layer_edit_entity(port, &achievement) else {
            return;
        };
        let recipe = crate::layer_palette::effective_icon(&entity);
        let Some(layer) = recipe["layers"]
            .as_array()
            .and_then(|layers| layers.iter().find(|l| l["id"].as_str() == Some(layer_id)))
        else {
            return;
        };
        self.source_picker = Some(SourcePicker {
            viewport: ctx.viewport_id(),
            project: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            achievement,
            layer: layer_id.into(),
            mode,
            asset: layer[if mode == SourceKind::Atlas {
                "atlasId"
            } else {
                "imageId"
            }]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                (mode == SourceKind::Image)
                    .then(|| {
                        achimeld_application::art_images::layer_source(&entity, layer)
                            .map(|id| format!("art:{id}"))
                    })
                    .flatten()
            }),
            tile: layer["tileIndex"].as_u64().unwrap_or(0),
            general: layer["generalImageIndex"].as_i64().unwrap_or(0),
            append_general: false,
        });
    }
    pub(crate) fn open_general_picker(
        &mut self,
        ctx: &egui::Context,
        achievement: &str,
        mode: SourceKind,
    ) {
        self.source_picker = Some(SourcePicker {
            viewport: ctx.viewport_id(),
            project: self.picker_project_id(),
            achievement: achievement.into(),
            layer: String::new(),
            mode,
            asset: None,
            tile: 0,
            general: 0,
            append_general: true,
        });
    }
    pub(crate) fn source_picker_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        if self
            .source_picker
            .as_ref()
            .is_some_and(|picker| picker.viewport != ctx.viewport_id())
        {
            return;
        }
        let Some(mut picker) = self.source_picker.take() else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut dismissed = false;
        ctx.set_sublayer(
            egui::LayerId::new(
                egui::Order::Foreground,
                crate::layer_palette::layers_panel_id(),
            ),
            egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("layer-source-picker"),
            ),
        );
        let bounds = ctx.content_rect().shrink(8.0);
        egui::Window::new(crate::localization::text(
            ctx,
            if picker.append_general {
                "GnSelectSource"
            } else {
                "GnImportLayer"
            },
        ))
        .id(egui::Id::new("layer-source-picker"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .default_size([820.0, 640.0])
        .min_size(Vec2::new(320.0, 280.0).min(bounds.size()))
        .max_size(bounds.size())
        .constrain_to(bounds)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.app_selectable_value(&mut picker.mode, SourceKind::Image, "Images");
                ui.app_selectable_value(&mut picker.mode, SourceKind::Atlas, "Atlas tiles");
                if !picker.append_general {
                    ui.app_selectable_value(
                        &mut picker.mode,
                        SourceKind::General,
                        crate::localization::text(ui.ctx(), "GnImgs"),
                    );
                }
            });
            ui.separator();
            let footer_height =
                crate::buttons::bar_height(ui, 0.0) + 3.0 * ui.spacing().item_spacing.y + 1.0;
            let mut valid = false;
            if picker.mode == SourceKind::General && port.icon_styles().iter().any(|s| s.id == picker.achievement) {
                crate::fields::row(ui,"Image ID",|ui| { ui.add(egui::DragValue::new(&mut picker.general).range(0..=20000)); });
                valid = true;
            } else if picker.mode == SourceKind::General {
                if let Some(mut entity) = port
                    .editor()
                    .document()
                    .entity(EntityKind::Achievement, &picker.achievement)
                    .cloned()
                {
                    let before = entity.clone();
                    let mut selected = json!({"generalImageIndex":picker.general});
                    egui::ScrollArea::vertical()
                        .id_salt("source-general-images")
                        .max_height((ui.available_height() - footer_height).max(1.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.general_explorer(ui, port, &mut entity, Some(&mut selected));
                        });
                    picker.general = selected["generalImageIndex"].as_i64().unwrap_or(0);
                    valid = crate::general_images::entries(&entity)
                        .iter()
                        .any(|entry| entry["id"] == picker.general);
                    if entity != before {
                        port.finish_group();
                        if let Err(error) = port.execute(
                            "Edit general images",
                            Command::ReplaceEntity {
                                kind: EntityKind::Achievement,
                                id: picker.achievement.clone(),
                                value: entity,
                            },
                            None,
                        ) {
                            self.error = Some(error);
                        }
                    }
                }
            } else {
                let atlas = picker.mode == SourceKind::Atlas;
                let mut assets = port.editor().document().project()["assets"]
                    [if atlas { "atlases" } else { "images" }]
                .as_array()
                .cloned()
                .unwrap_or_default();
                assets.retain(|asset| asset["id"].is_string());
                if atlas {
                    assets.insert(0, json!({"displayName":"Default atlas"}));
                }
                if !atlas {
                    let document = port.editor().document();
                    assets.extend(document.entities(EntityKind::Achievement).iter()
                        .filter(|entity| achimeld_application::art_images::is_art_image(entity) && entity["id"] != picker.achievement)
                        .map(|entity| json!({"id":format!("art:{}", entity["id"].as_str().unwrap_or("")),
                            "displayName":format!("{} · {}", document.display_name(entity, &port.preferences().language), crate::localization::text(ui.ctx(), "Art image"))})));
                }
                let height = (ui.available_height() - footer_height).max(1.0);
                egui::ScrollArea::vertical()
                    .id_salt("source-assets")
                    .max_height(if atlas {
                        (height * 0.4).min(180.0)
                    } else {
                        height
                    })
                    .auto_shrink([false, false])
                    .show_viewport(ui, |ui, viewport| {
                        let width = (ui.available_width() - 2.0 * SOURCE_GRID_PADDING).max(1.0);
                        let columns = ((width + SOURCE_TILE_GAP)
                            / (SOURCE_TILE_WIDTH + SOURCE_TILE_GAP))
                            .floor()
                            .max(1.0) as usize;
                        let tile_size = Vec2::new(
                            SOURCE_TILE_WIDTH.min(width),
                            SOURCE_TILE_IMAGE
                                + 24.0
                                + 2.0 * ui.text_style_height(&egui::TextStyle::Button),
                        );
                        let rows = assets.len().div_ceil(columns);
                        let stride = tile_size.y + SOURCE_TILE_GAP;
                        let height = (rows as f32 * stride - SOURCE_TILE_GAP).max(0.0)
                            + 2.0 * SOURCE_GRID_PADDING;
                        let (_, grid) = ui.allocate_space(Vec2::new(ui.available_width(), height));
                        let first = ((viewport.top() - SOURCE_GRID_PADDING) / stride)
                            .floor()
                            .max(0.0) as usize;
                        let last = ((viewport.bottom() / stride).ceil() as usize + 1).min(rows);
                        for row in first..last {
                            for column in 0..columns {
                                let Some(asset) = assets.get(row * columns + column) else {
                                    break;
                                };
                                let id = asset["id"].as_str();
                                let rect = Rect::from_min_size(
                                    grid.min
                                        + Vec2::splat(SOURCE_GRID_PADDING)
                                        + Vec2::new(
                                            column as f32 * (tile_size.x + SOURCE_TILE_GAP),
                                            row as f32 * stride,
                                        ),
                                    tile_size,
                                );
                                let preview = if let Some(art) = id.and_then(|id| id.strip_prefix("art:")) {
                                    port.canvas_preview(art, 128)
                                } else {
                                    port.asset_preview(atlas, id)
                                }.and_then(|preview| {
                                    let size = Vec2::new(
                                        preview.width.max(1) as f32,
                                        preview.height.max(1) as f32,
                                    );
                                    self.preview_texture(
                                        ui.ctx(),
                                        &format!("asset:{atlas}:{}", id.unwrap_or("default")),
                                        preview,
                                    )
                                    .map(|texture| (texture, size))
                                });
                                let response = source_tile(
                                    ui,
                                    rect,
                                    ui.make_persistent_id(("source-tile", atlas, id)),
                                    asset["displayName"].as_str().unwrap_or("Image"),
                                    preview,
                                    picker.asset.as_deref() == id,
                                );
                                if response.clicked() {
                                    picker.asset = id.map(str::to_owned);
                                    apply |= !atlas
                                        && (response.double_clicked() || response.triple_clicked());
                                }
                            }
                        }
                        if assets.is_empty() {
                            ui.weak("No images in the Art Data Bank yet.");
                        }
                    });
                if atlas {
                    if let Some(preview) = port.asset_preview(true, picker.asset.as_deref()) {
                        let size = preview.source_size;
                        let ratio = preview.width as f32 / preview.height.max(1) as f32;
                        let height = (ui.available_height()
                            - footer_height
                            - crate::buttons::bar_height(ui, 0.0))
                        .clamp(1.0, 300.0);
                        let width = ui.available_width().min(480.0).min(height * ratio);
                        let display = Vec2::new(width, width / ratio);
                        let texture = self.preview_texture(
                            ui.ctx(),
                            &format!(
                                "atlas-picker:{}",
                                picker.asset.as_deref().unwrap_or("default")
                            ),
                            preview,
                        );
                        let (rect, response) = ui.allocate_exact_size(display, Sense::click());
                        if let Some(texture) = texture {
                            ui.painter().image(
                                texture,
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        let settings = &port.editor().document().project()["atlas"];
                        if let Some(TileGeometry {
                            w,
                            h,
                            left,
                            top,
                            dx,
                            dy,
                            columns,
                            rows,
                        }) = tile_geometry(size, settings)
                            && columns > 0
                            && rows > 0
                            && picker.tile < columns * rows
                        {
                            valid = true;
                            let (c, r) = if settings["traversal"] == "ColumnMajor" {
                                (picker.tile / rows, picker.tile % rows)
                            } else {
                                (picker.tile % columns, picker.tile / columns)
                            };
                            let point = |x: u64, y: u64| {
                                rect.min
                                    + Vec2::new(
                                        x as f32 / size[0].max(1) as f32 * rect.width(),
                                        y as f32 / size[1].max(1) as f32 * rect.height(),
                                    )
                            };
                            for col in (0..columns).step_by(columns.div_ceil(128).max(1) as usize) {
                                let x = left + col * dx;
                                ui.painter().line_segment(
                                    [point(x, top), point(x, top + rows * dy)],
                                    Stroke::new(0.5, Color32::from_white_alpha(100)),
                                );
                            }
                            for row in (0..rows).step_by(rows.div_ceil(128).max(1) as usize) {
                                let y = top + row * dy;
                                ui.painter().line_segment(
                                    [point(left, y), point(left + columns * dx, y)],
                                    Stroke::new(0.5, Color32::from_white_alpha(100)),
                                );
                            }
                            ui.painter().rect_stroke(
                                Rect::from_min_max(
                                    point(left + c * dx, top + r * dy),
                                    point(left + c * dx + w, top + r * dy + h),
                                ),
                                0.0,
                                Stroke::new(2.0, theme::color(port.preferences(), "Accent")),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked()
                            && let Some(pos) = response.interact_pointer_pos()
                            && let Some(tile) = tile_at(
                                size,
                                settings,
                                [
                                    (pos.x - rect.left()) as f64 / rect.width() as f64
                                        * size[0] as f64,
                                    (pos.y - rect.top()) as f64 / rect.height() as f64
                                        * size[1] as f64,
                                ],
                            )
                        {
                            picker.tile = tile;
                            apply |= response.double_clicked() || response.triple_clicked();
                        }
                        crate::fields::row(ui, "Tile index", |ui| {
                            ui.add(egui::DragValue::new(&mut picker.tile).range(0..=20000));
                        });
                    } else {
                        let key = format!(
                            "asset:atlas:{}",
                            picker.asset.as_deref().unwrap_or("default")
                        );
                        if let Some(error) = port.preview_error(&key) {
                            ui.colored_label(
                                theme::color(port.preferences(), "Warning"),
                                format!("Atlas preview could not load: {error}"),
                            );
                        } else {
                            ui.spinner();
                            ui.weak("Loading atlas preview…");
                        }
                    }
                } else {
                    valid = picker.asset.is_some();
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(valid, crate::buttons::Button::new("Use source"))
                    .clicked()
                {
                    apply = true;
                }
                if ui.app_button("Import image…").clicked() {
                    if picker.append_general {
                        self.request(UiRequest::ImportGeneralImage {
                            achievement_id: picker.achievement.clone(),
                        });
                    } else {
                        self.request(UiRequest::ImportLayerImage {
                            achievement_id: picker.achievement.clone(),
                            layer_id: Some(picker.layer.clone()),
                        });
                    }
                    dismissed = true;
                }
                if ui.app_button("Cancel").clicked() {
                    dismissed = true;
                }
            });
        });
        if dismissed {
            open = false;
        }
        if apply {
            let result = self.apply_source(port, &picker);
            if let Err(error) = result {
                self.error = Some(error);
            } else {
                open = false;
            }
        }
        if open {
            self.source_picker = Some(picker);
        }
    }
    fn apply_source(
        &mut self,
        port: &mut dyn EditorPort,
        picker: &SourcePicker,
    ) -> Result<(), String> {
        if port.editor().document().project()["projectId"].as_str() != Some(&picker.project) {
            return Err("The project changed while choosing the layer source".into());
        }
        let style = port
            .icon_styles()
            .into_iter()
            .find(|s| s.id == picker.achievement);
        let live = self
            .layer_edit_entity(port, &picker.achievement)
            .ok_or("Edited icon or style no longer exists")?;
        if self
            .entity_drafts
            .get(&picker.achievement)
            .is_some_and(|draft| draft.base != live)
        {
            return Err("This icon changed while its edit was invalid. Revert the draft before choosing a source.".into());
        }
        let mut entity = self
            .entity_drafts
            .get(&picker.achievement)
            .map(|draft| draft.value.clone())
            .unwrap_or(live);
        if picker.append_general {
            let source = if let Some(art) = picker
                .asset
                .as_deref()
                .filter(|_| picker.mode == SourceKind::Image)
                .and_then(|id| id.strip_prefix("art:"))
            {
                json!({"artImageId":art,"imageId":null,"atlasId":null,"tileIndex":0})
            } else {
                json!({"imageId":if picker.mode==SourceKind::Image {picker.asset.as_ref().map_or(Value::Null,|id|json!(id))}else{Value::Null},
                "atlasId":if picker.mode==SourceKind::Atlas {json!(picker.asset.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000"))}else{Value::Null},"tileIndex":picker.tile})
            };
            let selected = crate::general_images::append(&mut entity, source);
            port.finish_group();
            port.execute(
                "Add general image",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: picker.achievement.clone(),
                    value: entity,
                },
                None,
            )?;
            self.general_images.selected = Some(selected);
            self.general_images.added = Some((
                picker.achievement.clone(),
                selected,
                port.editor().revision(),
            ));
            return Ok(());
        }
        let art_general = picker
            .asset
            .as_deref()
            .filter(|_| picker.mode == SourceKind::Image)
            .and_then(|id| id.strip_prefix("art:"))
            .map(|art| {
                crate::general_images::append(
                    &mut entity,
                    json!({"artImageId":art,"imageId":null,"atlasId":null,"tileIndex":0}),
                )
            });
        entity["icon"] = crate::layer_palette::effective_icon(&entity);
        let layers = entity["icon"]["layers"]
            .as_array_mut()
            .ok_or("Invalid layer stack")?;
        let index = layers
            .iter()
            .position(|l| l["id"].as_str() == Some(&picker.layer))
            .ok_or("Layer no longer exists")?;
        let mask = layers[index]["kind"] == "Folder";
        let target = if mask {
            let mut layer = crate::layer_palette::new_layer("StandaloneImage");
            layer["visible"] = json!(false);
            layer["displayName"] = json!("Folder mask");
            let id = layer["id"].clone();
            layers.push(layer);
            layers[index]["maskLayerId"] = id;
            layers[index]["maskMode"] = json!("Luminance");
            layers[index]["maskInvert"] = json!(false);
            layers.len() - 1
        } else {
            index
        };
        let layer = &mut layers[target];
        layer["kind"] = json!(if art_general.is_some() {
            "AchievementImage"
        } else {
            match picker.mode {
                SourceKind::Image => "StandaloneImage",
                SourceKind::Atlas => "AtlasTile",
                SourceKind::General => "AchievementImage",
            }
        });
        layer["imageId"] = if picker.mode == SourceKind::Image && art_general.is_none() {
            picker.asset.as_ref().map_or(Value::Null, |id| json!(id))
        } else {
            Value::Null
        };
        layer["atlasId"] = if picker.mode == SourceKind::Atlas {
            picker.asset.as_ref().map_or(Value::Null, |id| json!(id))
        } else {
            Value::Null
        };
        layer["tileIndex"] = json!(picker.tile);
        layer["generalImageIndex"] = json!(art_general.unwrap_or(picker.general));
        layer["linkAtlasToGeneral"] = json!(false);
        if let Some(mut style) = style {
            if art_general.is_some() {
                return Err("Use a General image ID for project style artwork".into());
            }
            style.recipe = entity["icon"].clone();
            let command =
                achimeld_application::style_management::edit(port.editor().document(), style)?;
            port.finish_group();
            port.execute("Replace style layer source", command, None)?;
            self.entity_drafts.remove(&picker.achievement);
            return Ok(());
        }
        port.finish_group();
        port.execute(
            if mask {
                "Set folder mask"
            } else {
                "Replace layer source"
            },
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: picker.achievement.clone(),
                value: entity,
            },
            None,
        )?;
        self.entity_drafts.remove(&picker.achievement);
        Ok(())
    }
}
