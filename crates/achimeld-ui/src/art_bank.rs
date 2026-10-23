use crate::buttons::UiButtonExt as _;
use crate::desktop::Desktop;
use crate::{buttons, localization};
use achimeld_application::{Command, EditorPort, EntityKind, UiRequest};
use serde_json::Value;

const ART_TILE_WIDTH: f32 = 144.0;
const ART_TILE_IMAGE: f32 = 128.0;
const ART_TILE_GAP: f32 = 12.0;

fn art_tile_height(ui: &egui::Ui) -> f32 {
    ART_TILE_IMAGE + 24.0 + 2.0 * ui.text_style_height(&egui::TextStyle::Button)
}

fn art_tile(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: egui::Id,
    title: &str,
    preview: Option<(egui::TextureId, egui::Vec2)>,
    selected: bool,
    unavailable: bool,
) -> egui::Response {
    let response = ui.interact(rect, id, egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.inactive.bg_fill);
    let ink = ui.visuals().text_color();
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0,
            egui::Stroke::new(if selected { 2.0 } else { 1.0 }, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    let image_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 8.0 + ART_TILE_IMAGE / 2.0),
        egui::vec2((rect.width() - 16.0).max(1.0), ART_TILE_IMAGE),
    );
    if let Some((texture, size)) = preview {
        let fit = (image_rect.width() / size.x.max(1.0)).min(image_rect.height() / size.y.max(1.0));
        ui.painter().image(
            texture,
            egui::Rect::from_center_size(image_rect.center(), size * fit),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_center_size(image_rect.center(), egui::Vec2::splat(32.0)),
            crate::icons::Icon::Layers,
            ink,
        );
        ui.painter().text(
            egui::pos2(image_rect.center().x, image_rect.bottom()),
            egui::Align2::CENTER_BOTTOM,
            if unavailable {
                "Unavailable"
            } else {
                "Loading…"
            },
            egui::TextStyle::Small.resolve(ui.style()),
            ink,
        );
    }
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, image_rect.bottom() + 4.0),
        rect.right_bottom() - egui::Vec2::splat(8.0),
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
    ui.painter().add(
        egui::epaint::TextShape::new(
            egui::pos2(
                label_rect.center().x,
                label_rect.center().y - galley.size().y / 2.0,
            ),
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
            title,
        )
    });
    response
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum ArtTab {
    #[default]
    General,
    Icon,
    Atlases,
}

#[derive(Default)]
pub(crate) struct ArtBank {
    tab: ArtTab,
    icons: Option<[egui::TextureHandle; 5]>,
    selected: Option<String>,
    search: String,
    dimensions: [u32; 2],
    fingerprint: Option<Value>,
    delete: Option<DeleteArtwork>,
}

struct DeleteArtwork {
    project: String,
    atlas: bool,
    id: String,
    title: String,
}
impl ArtBank {
    pub(crate) fn pending(&self) -> bool {
        self.delete.is_some()
    }
}
#[cfg(test)]
impl ArtBank {
    pub(crate) fn confirm_delete(&mut self, project: String, id: String) {
        self.delete = Some(DeleteArtwork {
            project,
            atlas: false,
            id,
            title: "Shared artwork".into(),
        });
    }

    pub(crate) fn select_tab(&mut self, tab: ArtTab) {
        self.tab = tab;
    }
    pub(crate) fn select(&mut self, id: &str) {
        self.selected = Some(id.into());
    }
}

// Categorize by actual references. General-image layers resolve through General
// settings, so they do not turn that artwork into a direct icon-image reference.
pub(crate) fn image_categories(
    project: &Value,
) -> (
    std::collections::BTreeSet<&str>,
    std::collections::BTreeSet<&str>,
) {
    let mut general = std::collections::BTreeSet::new();
    let mut icon = std::collections::BTreeSet::new();
    for achievement in project["achievements"].as_array().into_iter().flatten() {
        if let Some(id) = achievement["generalImageId"].as_str() {
            general.insert(id);
        }
        for entry in achievement["generalImageEntries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(id) = entry["imageId"].as_str() {
                general.insert(id);
            }
        }
        if achievement["icon"].is_null()
            && let Some(id) = achievement["standaloneImageId"].as_str()
        {
            icon.insert(id);
        }
        for layer in achievement["icon"]["layers"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if matches!(
                layer["kind"].as_str(),
                Some("StandaloneImage" | "SteamImportedArtwork")
            ) && let Some(id) = layer["imageId"].as_str()
            {
                icon.insert(id);
            }
        }
    }
    (general, icon)
}

fn space(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

impl Desktop {
    pub(crate) fn art_bank_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        self.art_bank_contents(ui, port);
    }
    fn art_bank_contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let mut bank = std::mem::take(&mut self.art_bank);
        let project = port.editor().document().project();
        let (general, icon) = image_categories(project);
        let assets = project["assets"].clone();
        let images = assets["images"].as_array().cloned().unwrap_or_default();
        let general_images: Vec<_> = images
            .iter()
            .filter(|entry| entry["id"].as_str().is_some_and(|id| general.contains(id)))
            .cloned()
            .collect();
        // Unassigned imports remain accessible and repairable in Icon images.
        let icon_images: Vec<_> = images
            .iter()
            .filter(|entry| {
                entry["id"]
                    .as_str()
                    .is_some_and(|id| icon.contains(id) || !general.contains(id))
            })
            .cloned()
            .collect();
        let mut atlases = assets["atlases"].as_array().cloned().unwrap_or_default();
        let default_available = port.art_file_bytes(&assets["atlas"]).is_some();
        let default_used = port.editor().document().project()["achievements"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|entry| {
                (entry["icon"].is_null()
                    && entry["standaloneImageId"].is_null()
                    && entry["atlasId"].is_null())
                    || entry["icon"]["layers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|layer| layer["kind"] == "AtlasTile" && layer["atlasId"].is_null())
            });
        if (default_available || default_used)
            && !atlases.iter().any(|a| a["image"] == assets["atlas"])
        {
            atlases.insert(0, serde_json::json!({"id":"default","displayName":"Default atlas","image":assets["atlas"],"importSource":assets["atlasImportSource"]}));
        }
        let mut unique = std::collections::BTreeSet::new();
        let mut total = 0;
        let mut count = 0;
        let mut unavailable = 0;
        for items in [&images, &atlases] {
            for entry in items {
                count += 1;
                if unique.insert(entry["image"].to_string()) {
                    if let Some(bytes) = port.art_file_bytes(&entry["image"]) {
                        total += bytes;
                    } else {
                        unavailable += 1;
                    }
                }
            }
        }
        let icons = bank
            .icons
            .get_or_insert_with(|| {
                [
                    (
                        "art-general-images",
                        include_bytes!("../assets/toolbar/art-general-images.rgba").as_slice(),
                    ),
                    (
                        "art-icon-images",
                        include_bytes!("../assets/toolbar/art-icon-images.rgba").as_slice(),
                    ),
                    (
                        "art-atlases",
                        include_bytes!("../assets/toolbar/art-atlases.rgba").as_slice(),
                    ),
                    (
                        "art-refresh",
                        include_bytes!("../assets/toolbar/art-refresh.rgba").as_slice(),
                    ),
                    (
                        "art-import",
                        include_bytes!("../assets/toolbar/art-import.rgba").as_slice(),
                    ),
                ]
                .map(|(name, bytes)| {
                    ui.ctx().load_texture(
                        name,
                        egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                        egui::TextureOptions::LINEAR
                            .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                    )
                })
            })
            .each_ref()
            .map(|icon| (icon.id(), egui::Vec2::splat(20.0)));
        egui::Panel::top("art-bank-toolbar")
            .exact_size(54.0)
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                // Keep the approved row height, with the same edge-to-edge
                // button layout and resting fill as the main toolbar.
                let height = ui.available_height();
                egui::ScrollArea::horizontal()
                    .id_salt("art-bank-toolbar-scroll")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        crate::buttons::bar_with_height(ui, height, |ui| {
                            for (tab, label, image) in [
                                (ArtTab::General, "General images", icons[0]),
                                (ArtTab::Icon, "Icon images", icons[1]),
                                (ArtTab::Atlases, "Atlases", icons[2]),
                            ] {
                                if crate::desktop::toolbar_tab(ui, &mut bank.tab, tab, label, image)
                                    .changed()
                                {
                                    bank.selected = None;
                                    bank.fingerprint = None;
                                }
                            }
                            ui.separator();
                            if ui
                                .add(
                                    crate::buttons::Button::new((
                                        egui::Image::new(icons[3]),
                                        "Refresh",
                                    ))
                                    .bar()
                                    .icon_spacing(5.0)
                                    .image_tint_follows_text_color(true),
                                )
                                .clicked()
                            {
                                self.request(UiRequest::RefreshArt);
                            }
                            let atlas = bank.tab == ArtTab::Atlases;
                            if ui
                                .add_enabled(
                                    !port.art_busy(),
                                    crate::buttons::Button::new((
                                        egui::Image::new(icons[4]),
                                        if atlas {
                                            "Import atlas…"
                                        } else {
                                            "Import artwork…"
                                        },
                                    ))
                                    .bar()
                                    .icon_spacing(5.0)
                                    .image_tint_follows_text_color(true)
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .clicked()
                            {
                                self.request(if atlas {
                                    UiRequest::ImportAtlas
                                } else {
                                    UiRequest::ImportImage
                                });
                                if bank.tab == ArtTab::General {
                                    // Imports are unassigned until attached in
                                    // General settings; keep the result visible.
                                    bank.tab = ArtTab::Icon;
                                    bank.selected = None;
                                    bank.fingerprint = None;
                                }
                            }
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut bank.search)
                                    .hint_text("Find artwork")
                                    .desired_width(180.0),
                            );
                        })
                    });
            });
        egui::Panel::bottom("art-bank-stats")
            .exact_size(26.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let storage = if port.art_inventory_ready() {
                        space(total)
                    } else {
                        "Calculating storage…".into()
                    };
                    let missing = if unavailable > 0 && port.art_inventory_ready() {
                        format!(" · {unavailable} unavailable")
                    } else {
                        String::new()
                    };
                    ui.weak(format!("{count} assets · {storage}{missing}"));
                    if port.art_busy() {
                        ui.weak("Processing artwork…");
                    }
                });
            });
        let items = match bank.tab {
            ArtTab::General => general_images,
            ArtTab::Icon => icon_images,
            ArtTab::Atlases => atlases,
        };
        if let Some(preview) = port.art_pending_preview()
            && let Some(texture) = self.preview_texture(ui.ctx(), "art-import-quick", preview)
        {
            egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
                crate::icons::thumbnail(ui, Some(texture), "Import preview", 64.0);
            });
        }
        let height = ui.available_height().max(0.0);
        let width = ui.available_width();
        if width >= 740.0 {
            // Native panel resizing retains the allocation across tabs and frames.
            // Leave enough room for both panes, including at larger font sizes.
            let details = egui::Panel::right("art-bank-details-pane")
                .default_size((width * 0.3).clamp(280.0, 400.0))
                .min_size(280.0)
                .max_size(width - 260.0)
                .resizable(true)
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(16.0),
                )
                .show(ui, |ui| {
                    let height = ui.available_height().max(0.0);
                    self.art_details(ui, port, &items, &mut bank, height);
                });
            egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
                let height = ui.available_height().max(0.0);
                self.art_grid(ui, port, &items, &mut bank, height);
            });
            // A visible grip advertises the panel's native horizontal resize edge.
            let grip = egui::Rect::from_center_size(
                egui::pos2(
                    details.response.rect.left() + 2.0,
                    details.response.rect.center().y,
                ),
                egui::vec2(4.0, 36.0),
            );
            ui.painter()
                .rect_filled(grip, 2.0, ui.visuals().weak_text_color());
        } else {
            // Narrow windows keep two independently scrollable stacked panes.
            let grid_height = (height * 0.4)
                .max(art_tile_height(ui))
                .min((height - 64.0).max(0.0));
            egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
                self.art_grid(ui, port, &items, &mut bank, grid_height);
            });
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    let height = ui.available_height().max(0.0);
                    ui.set_min_height(height);
                    self.art_details(ui, port, &items, &mut bank, height);
                });
        }
        self.art_delete_prompt(ui.ctx(), port, &mut bank);
        self.art_bank = bank;
    }

    fn art_grid(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        items: &[Value],
        bank: &mut ArtBank,
        height: f32,
    ) {
        if items.is_empty() {
            ui.weak(match bank.tab {
                ArtTab::General => {
                    "Images attached in achievement General settings will appear here."
                }
                ArtTab::Icon => "Import artwork or drag image files into this window.",
                ArtTab::Atlases => "Your atlases will appear here.",
            });
            return;
        }
        let search = bank.search.to_lowercase();
        let filtered: Vec<_> = items
            .iter()
            .filter(|entry| {
                entry["id"].is_string()
                    && entry["displayName"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&search)
            })
            .collect();
        if filtered.is_empty() {
            ui.weak("No artwork matches your search.");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt(("art-bank-grid", bank.tab))
            .max_height(height)
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                let width = ui.available_width().max(1.0);
                let columns = ((width + ART_TILE_GAP) / (ART_TILE_WIDTH + ART_TILE_GAP))
                    .floor()
                    .max(1.0) as usize;
                let tile_size = egui::vec2(ART_TILE_WIDTH.min(width), art_tile_height(ui));
                let rows = filtered.len().div_ceil(columns);
                let stride = tile_size.y + ART_TILE_GAP;
                let (_, grid) = ui.allocate_space(egui::vec2(
                    width,
                    (rows as f32 * stride - ART_TILE_GAP).max(0.0),
                ));
                let first = (viewport.top() / stride).floor().max(0.0) as usize;
                let last = ((viewport.bottom() / stride).ceil() as usize).min(rows);
                for row in first..last {
                    for column in 0..columns {
                        let Some(entry) = filtered.get(row * columns + column) else {
                            break;
                        };
                        let id = entry["id"].as_str().unwrap_or_default();
                        let title = entry["displayName"].as_str().unwrap_or("Image");
                        let rect = egui::Rect::from_min_size(
                            grid.min
                                + egui::vec2(
                                    column as f32 * (tile_size.x + ART_TILE_GAP),
                                    row as f32 * stride,
                                ),
                            tile_size,
                        );
                        let atlas = bank.tab == ArtTab::Atlases;
                        let preview = port.asset_preview(atlas, Some(id));
                        let dimensions = preview.as_ref().map(|p| p.source_size);
                        let texture = preview.and_then(|p| {
                            let size = egui::vec2(p.width as f32, p.height as f32);
                            self.preview_texture(ui.ctx(), &format!("bank:{atlas}:{id}"), p)
                                .map(|texture| (texture, size))
                        });
                        let bytes = port.art_file_bytes(&entry["image"]);
                        let key = format!("asset:{}:{id}", if atlas { "atlas" } else { "image" });
                        let unavailable = (bytes.is_none() && port.art_inventory_ready())
                            || port.preview_error(&key).is_some();
                        let response = art_tile(
                            ui,
                            rect,
                            ui.make_persistent_id(("art-tile", bank.tab, id)),
                            title,
                            texture,
                            bank.selected.as_deref() == Some(id),
                            unavailable,
                        );
                        if response.clicked() || response.secondary_clicked() {
                            bank.selected = Some(id.into());
                            bank.fingerprint = None;
                        }
                        buttons::context_menu(&response, |ui| {
                            let width =
                                520.0_f32.min((ui.ctx().content_rect().width() - 32.0).max(120.0));
                            ui.set_width(width);
                            ui.set_max_height((ui.ctx().content_rect().height() - 32.0).max(120.0));
                            if ui
                                .add_enabled(
                                    !port.art_busy(),
                                    buttons::Button::new((
                                        art_action_image(ui.ctx(), "copy", 20.0),
                                        localization::text(ui.ctx(), "Duplicate"),
                                    ))
                                    .image_tint_follows_text_color(true)
                                    .align(egui::Align2::LEFT_CENTER)
                                    .min_size(egui::vec2(width, 0.0))
                                    .square_corners(),
                                )
                                .clicked()
                            {
                                let new_id = achimeld_application::new_id();
                                let name =
                                    localization::text(ui.ctx(), "{0} copy").replace("{0}", title);
                                if self.apply(
                                    port,
                                    "Duplicate artwork",
                                    Command::DuplicateArtwork {
                                        atlas,
                                        id: id.into(),
                                        new_id: new_id.clone(),
                                        name,
                                    },
                                    None,
                                ) {
                                    bank.selected = Some(new_id);
                                    bank.fingerprint = None;
                                    if !atlas {
                                        bank.tab = ArtTab::Icon;
                                    }
                                }
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    !port.art_busy(),
                                    buttons::Button::new((
                                        art_action_image(ui.ctx(), "delete", 20.0),
                                        localization::text(ui.ctx(), "Delete"),
                                    ))
                                    .image_tint_follows_text_color(true)
                                    .align(egui::Align2::LEFT_CENTER)
                                    .min_size(egui::vec2(width, 0.0))
                                    .square_corners(),
                                )
                                .clicked()
                            {
                                if achimeld_application::art_bank::artwork_usage(
                                    port.editor().document().project(),
                                    atlas,
                                    id,
                                )
                                .is_empty()
                                {
                                    if self.apply(
                                        port,
                                        "Delete artwork",
                                        Command::DeleteArtwork {
                                            atlas,
                                            id: id.into(),
                                        },
                                        None,
                                    ) {
                                        bank.selected = None;
                                        bank.fingerprint = None;
                                    }
                                } else {
                                    bank.delete = Some(DeleteArtwork {
                                        project: port.editor().document().project()["projectId"]
                                            .as_str()
                                            .unwrap_or("")
                                            .into(),
                                        atlas,
                                        id: id.into(),
                                        title: title.into(),
                                    });
                                }
                                ui.close();
                            }
                            egui::Frame::NONE.inner_margin(12.0).show(ui, |ui| {
                                let height = (ui.ctx().content_rect().height() - 180.0).max(60.0);
                                self.art_usage_grid(ui, port, atlas, id, height, true);
                            });
                        });
                        response.on_hover_ui(|ui| {
                            ui.label(title);
                            if let Some([w, h]) = dimensions {
                                ui.weak(format!("{w} × {h} pixels"));
                            }
                            if let Some(bytes) = bytes {
                                ui.weak(format!("Project storage: {}", space(bytes)));
                            } else if unavailable {
                                ui.label("Unavailable · reload to repair");
                            } else {
                                ui.weak("Reading artwork details…");
                            }
                        });
                    }
                }
            });
    }

    fn art_details(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        items: &[Value],
        bank: &mut ArtBank,
        height: f32,
    ) {
        let Some(entry) = items
            .iter()
            .find(|entry| entry["id"].as_str() == bank.selected.as_deref())
        else {
            ui.weak("Select artwork to preview it, change its resolution, or reload it.");
            return;
        };
        let id = entry["id"].as_str().unwrap_or("");
        egui::ScrollArea::vertical().id_salt(("art-bank-details", bank.tab)).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
            ui.heading(entry["displayName"].as_str().unwrap_or("Image"));
            let preview = port.asset_preview(bank.tab == ArtTab::Atlases, Some(id));
            if bank.fingerprint.as_ref() != Some(&entry["image"]) && let Some(preview) = &preview {
                    bank.dimensions = [entry["pixelWidth"].as_u64().unwrap_or(preview.source_size[0] as u64) as u32, entry["pixelHeight"].as_u64().unwrap_or(preview.source_size[1] as u64) as u32];
                    bank.fingerprint = Some(entry["image"].clone());
            }
            if let Some(preview) = preview {
                let dimensions = preview.source_size;
                if let Some(texture) = self.preview_texture(ui.ctx(), &format!("bank-detail:{}:{id}", bank.tab == ArtTab::Atlases), preview) {
                    let ratio = (ui.available_width().min(260.0) / dimensions[0].max(dimensions[1]) as f32).min(1.0);
                    ui.image((texture, egui::vec2(dimensions[0] as f32 * ratio, dimensions[1] as f32 * ratio)));
                }
                ui.label(format!("{} × {} pixels", dimensions[0], dimensions[1]));
            } else {
                let key = format!("asset:{}:{id}", if bank.tab == ArtTab::Atlases { "atlas" } else { "image" });
                ui.label(port.preview_error(&key).unwrap_or("Loading artwork preview…"));
            }
            if let Some(bytes) = port.art_file_bytes(&entry["image"]) { ui.label(format!("Project storage: {}", space(bytes))); }
            ui.horizontal_wrapped(|ui| {
                ui.weak("Imported from:");
                let source = &entry["importSource"];
                let label = match source["kind"].as_str() {
                    Some("File") => source["path"].as_str().filter(|path| !path.is_empty()).unwrap_or("Source not recorded"),
                    Some("Clipboard") => "Clipboard",
                    _ => "Source not recorded",
                };
                ui.add(egui::Label::new(label).wrap().selectable(true));
            });
            ui.add_space(12.0);
            ui.add_enabled_ui(!port.art_busy() && bank.fingerprint.is_some(), |ui| {
                crate::fields::row(ui, "Width", |ui| {
                    ui.add(egui::DragValue::new(&mut bank.dimensions[0]).range(1..=16384).suffix(" px"));
                });
                crate::fields::row(ui, "Height", |ui| {
                    ui.add(egui::DragValue::new(&mut bank.dimensions[1]).range(1..=16384).suffix(" px"));
                });
                if ui.app_button("Apply resolution").clicked() {
                    self.request(UiRequest::ResizeArt { atlas: bank.tab == ArtTab::Atlases, id: id.into(), width: bank.dimensions[0], height: bank.dimensions[1] });
                }
            });
            if bank.tab == ArtTab::Atlases { ui.weak("Resizing an atlas changes its tile grid. Check tile sizes in Project settings."); }
            ui.add_space(10.0);
            ui.add_enabled_ui(!port.art_busy(), |ui| {
                if ui.app_button("Reload from file…").clicked() { self.request(UiRequest::ReloadArt { atlas: bank.tab == ArtTab::Atlases, id: id.into() }); }
            });
            ui.weak("Reload chooses a fresh source file. Existing icon references and undo history are preserved.");
        });
    }
}

fn art_action_image(ctx: &egui::Context, action: &str, size: f32) -> egui::Image<'static> {
    let id = egui::Id::new(("art-bank-action", action));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let bytes: &[u8] = match action {
                "copy" => include_bytes!("../assets/toolbar/menu-copy.rgba"),
                "delete" => include_bytes!("../../achimeld-app/assets/prompts/discard.rgba"),
                _ => include_bytes!("../../achimeld-app/assets/prompts/cancel.rgba"),
            };
            let texture = ctx.load_texture(
                format!("art-bank-{action}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), egui::Vec2::splat(size)))
}

fn art_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}

impl Desktop {
    fn art_usage_grid(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        atlas: bool,
        id: &str,
        max_height: f32,
        navigate: bool,
    ) {
        let document = port.editor().document();
        let uses: Vec<_> =
            achimeld_application::art_bank::artwork_usage(document.project(), atlas, id)
                .into_iter()
                .map(|a| {
                    (
                        a["id"].as_str().unwrap_or("").to_owned(),
                        document.display_name(a, &port.preferences().language),
                        a["apiName"].as_str().unwrap_or("").to_owned(),
                    )
                })
                .collect();
        ui.label(if uses.len() == 1 {
            localization::text(ui.ctx(), "Used by 1 achievement")
        } else {
            localization::text(ui.ctx(), "Used by {0} achievements")
                .replace("{0}", &uses.len().to_string())
        });
        if uses.is_empty() {
            ui.weak(localization::text(
                ui.ctx(),
                "No achievements use this artwork.",
            ));
            return;
        }
        let gap = 8.0;
        let width = ui.available_width().max(1.0);
        let columns = ((width + gap) / 112.0).floor().max(1.0) as usize;
        let cell_width = ((width - (columns - 1) as f32 * gap) / columns as f32).max(1.0);
        let label_height = ui.text_style_height(&egui::TextStyle::Small);
        let height = 64.0 + 16.0 + 3.0 * label_height;
        let rows = uses.len().div_ceil(columns);
        let row_spacing = ui.spacing().item_spacing.y;
        let mut selected = None;
        egui::ScrollArea::vertical()
            .id_salt(("art-usage", atlas, id))
            .max_height(
                (5.0 * (height + gap + row_spacing) - gap - row_spacing).min(max_height.max(32.0)),
            )
            .auto_shrink([false, true])
            .show_rows(ui, height + gap, rows, |ui, range| {
                for row in range {
                    let (rect, _) = ui
                        .allocate_exact_size(egui::vec2(width, height + gap), egui::Sense::hover());
                    for column in 0..columns {
                        let Some((achievement, name, api)) = uses.get(row * columns + column)
                        else {
                            break;
                        };
                        let cell = egui::Rect::from_min_size(
                            rect.min + egui::vec2(column as f32 * (cell_width + gap), 0.0),
                            egui::vec2(cell_width, height),
                        );
                        let response = ui.interact(
                            cell,
                            ui.id().with(("used-achievement", achievement)),
                            if navigate {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            },
                        );
                        ui.painter()
                            .rect_filled(cell, 0, ui.visuals().widgets.inactive.bg_fill);
                        if response.hovered() || response.has_focus() {
                            ui.painter().rect_stroke(
                                cell,
                                0,
                                egui::Stroke::new(1.0, egui::Color32::WHITE),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let image = egui::Rect::from_center_size(
                            egui::pos2(cell.center().x, cell.top() + 38.0),
                            egui::Vec2::splat(64.0_f32.min((cell_width - 12.0).max(1.0))),
                        );
                        if let Some(preview) = port.canvas_preview(achievement, 64)
                            && let Some(texture) = self.preview_texture(
                                ui.ctx(),
                                &format!("art-usage:{achievement}"),
                                preview,
                            )
                        {
                            ui.painter().image(
                                texture,
                                image,
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            crate::icons::paint(
                                ui.painter(),
                                image.shrink(12.0),
                                crate::icons::Icon::Layers,
                                ui.visuals().weak_text_color(),
                            );
                        }
                        let mut job = egui::text::LayoutJob::simple(
                            name.clone(),
                            egui::TextStyle::Small.resolve(ui.style()),
                            ui.visuals().text_color(),
                            (cell_width - 12.0).max(1.0),
                        );
                        job.wrap.max_rows = 2;
                        job.halign = egui::Align::Center;
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        ui.painter().galley(
                            egui::pos2(cell.center().x, cell.top() + 76.0),
                            galley,
                            ui.visuals().text_color(),
                        );
                        let mut job = egui::text::LayoutJob::simple(
                            api.clone(),
                            egui::TextStyle::Small.resolve(ui.style()),
                            ui.visuals().weak_text_color(),
                            (cell_width - 12.0).max(1.0),
                        );
                        job.wrap.max_rows = 1;
                        job.halign = egui::Align::Center;
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        ui.painter().galley(
                            egui::pos2(cell.center().x, cell.bottom() - label_height - 4.0),
                            galley,
                            ui.visuals().weak_text_color(),
                        );
                        if navigate && response.clicked() {
                            selected = Some(achievement.clone());
                            ui.close();
                        }
                        response.on_hover_text(format!("{name}\n{api}"));
                    }
                }
            });
        if let Some(id) = selected {
            self.automation_select(EntityKind::Achievement, id, port);
        }
    }

    fn art_delete_prompt(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        bank: &mut ArtBank,
    ) {
        let Some(delete) = bank.delete.take() else {
            return;
        };
        if port.editor().document().project()["projectId"].as_str() != Some(&delete.project) {
            return;
        }
        let exists = if delete.atlas && delete.id == "default" {
            !port.editor().document().project()["assets"]["atlas"].is_null()
        } else {
            port.editor().document().project()["assets"]
                [if delete.atlas { "atlases" } else { "images" }]
            .as_array()
            .into_iter()
            .flatten()
            .any(|entry| entry["id"] == delete.id)
        };
        if !exists {
            return;
        }
        let mut approve = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("delete-artwork-prompt"))
            .frame(egui::Frame::new().fill(egui::Color32::BLACK).corner_radius(0)
                .stroke(egui::Stroke::new(1.0, ctx.global_style().visuals.widgets.noninteractive.bg_stroke.color)))
            .show(ctx, |ui| {
                let actions = [("Delete artwork", "delete"), ("Cancel", "cancel")];
                let labels = actions.map(|(label, _)| localization::text(ctx, label));
                let font = egui::TextStyle::Button.resolve(ui.style());
                let natural = labels.each_ref().map(|label| ui.painter().layout_no_wrap(label.clone(), font.clone(), ui.visuals().text_color()).size().x + 28.0 + 2.0 * ui.spacing().button_padding.x);
                let total: f32 = natural.iter().sum();
                let width = 620.0_f32.max(total).min((ctx.content_rect().width() - 32.0).max(1.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let title = ui.painter().layout_no_wrap(localization::text(ctx, "Delete artwork"), egui::TextStyle::Heading.resolve(ui.style()), ui.visuals().text_color());
                let header_height = title.size().y.max(24.0) + 24.0;
                let (header, _) = ui.allocate_exact_size(egui::vec2(width, header_height), egui::Sense::hover());
                art_action_image(ctx, "delete", 24.0).tint(ui.visuals().text_color()).paint_at(ui,
                    egui::Rect::from_center_size(egui::pos2(header.left() + 36.0, header.center().y), egui::Vec2::splat(24.0)));
                ui.painter().galley(header.center() - title.size() / 2.0, title, ui.visuals().text_color());
                art_divider(ui);
                let action_height = buttons::bar_height(ui, 20.0);
                let body_height = (ctx.content_rect().height() - header_height - action_height - 80.0).max(48.0);
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(24,20)).show(ui, |ui| {
                    ui.set_width((width - 48.0).max(1.0));
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);
                    egui::ScrollArea::vertical().id_salt("delete-artwork-body").max_height(body_height)
                        .auto_shrink([false, true]).show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(localization::text(ctx, "Delete \"{0}\"?").replace("{0}", &delete.title)).wrap().halign(egui::Align::Center));
                                ui.add(egui::Label::new(localization::text(ctx, "Image layers using this artwork will become empty. Other layers will be preserved. You can undo this deletion.")).wrap().halign(egui::Align::Center));
                            });
                            self.art_usage_grid(ui, port, delete.atlas, &delete.id, (body_height - 130.0).max(64.0), false);
                        });
                });
                art_divider(ui);
                ui.allocate_ui_with_layout(egui::vec2(width, action_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let start = ui.cursor().left();
                    let split = ((start + width * natural[0] / total) * ctx.pixels_per_point()).round() / ctx.pixels_per_point();
                    for (index, (_, glyph)) in actions.into_iter().enumerate() {
                        let right = if index == 0 { split } else { start + width };
                        if ui.add_enabled(index == 1 || !port.art_busy(), buttons::Button::new((art_action_image(ctx, glyph, 20.0), labels[index].clone()))
                            .bar().min_size(egui::vec2(right - ui.cursor().left(), action_height)).icon_spacing(8.0).image_tint_follows_text_color(true)).clicked() {
                            if index == 0 { approve = true; } else { cancel = true; }
                        }
                    }
                });
            });
        if cancel || response.should_close() {
            return;
        }
        if approve
            && self.apply(
                port,
                "Delete artwork",
                Command::DeleteArtwork {
                    atlas: delete.atlas,
                    id: delete.id.clone(),
                },
                None,
            )
        {
            if bank.selected.as_deref() == Some(&delete.id) {
                bank.selected = None;
                bank.fingerprint = None;
            }
            return;
        }
        bank.delete = Some(delete);
    }
}
