//! Contextual artwork import, prepared by one bounded worker and committed atomically.
use super::*;
use achimeld_application::{EntityKind, ImageImportTarget};

pub(super) fn native_drop_position(ctx: &egui::Context, frame: &eframe::Frame) -> Option<[f32; 2]> {
    if !ctx.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty()) {
        return None;
    }
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    let RawWindowHandle::Win32(handle) = frame.window_handle().ok()?.as_raw() else {
        return None;
    };
    let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    // SAFETY: both APIs write to our initialized POINT. The HWND is borrowed
    // from the live eframe window and remains valid for this synchronous call.
    unsafe {
        if windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point) == 0
            || windows_sys::Win32::Graphics::Gdi::ScreenToClient(handle.hwnd.get() as _, &mut point)
                == 0
        {
            return None;
        }
    }
    ctx.request_repaint_after(Duration::from_millis(16));
    let scale = ctx.pixels_per_point();
    Some([point.x as f32 / scale, point.y as f32 / scale])
}

pub(super) struct Pending {
    project: String,
    target: ImageImportTarget,
    receiver: mpsc::Receiver<Result<Vec<ImportFile>, String>>,
    cancelled: Arc<AtomicBool>,
    ready: Option<Vec<ImportFile>>,
    preview_receiver: mpsc::Receiver<Preview>,
    preview: Option<Preview>,
}
impl Pending {
    pub(super) fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.ready = None;
    }
}
struct ImportFile {
    name: String,
    frames: Vec<ImportFrame>,
    entire: bool,
    selected: std::collections::BTreeSet<usize>,
    textures: Vec<Option<egui::TextureHandle>>,
}
struct ImportFrame {
    art: crate::art_bank::Prepared,
    preview: egui::ColorImage,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl Host {
    pub(super) fn start_image_drop(
        &mut self,
        paths: Vec<PathBuf>,
        target: ImageImportTarget,
    ) -> Result<(), String> {
        if self.art_busy() || self.synchronization.is_some() {
            return Err("Wait for the current artwork operation to finish".into());
        }
        if paths.is_empty() || paths.len() > 32 {
            return Err("Drop 1–32 artwork files at a time".into());
        }
        let (tx, receiver) = mpsc::sync_channel(1);
        let (preview_tx, preview_receiver) = mpsc::sync_channel(1);
        let ctx = self.ctx.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        // A General image is the saved composite, without a Photoshop layer choice.
        let frame_limit = if matches!(target, ImageImportTarget::General { .. }) {
            1
        } else {
            usize::MAX
        };
        std::thread::Builder::new()
            .name("dropped-artwork".into())
            .spawn(move || {
                let result = (|| {
                    let mut prepared = Vec::new();
                    let mut total = 0usize;
                    for path in paths {
                        if !path.is_absolute() {
                            return Err("Dropped artwork must use an absolute file path".into());
                        }
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Image")
                            .to_owned();
                        let photoshop =
                            path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                                s.eq_ignore_ascii_case("psd") || s.eq_ignore_ascii_case("psb")
                            });
                        let frames = if photoshop {
                            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                            let mut bytes = Vec::new();
                            file.take(64 * 1024 * 1024 + 1)
                                .read_to_end(&mut bytes)
                                .map_err(|e| e.to_string())?;
                            achimeld_imaging::photoshop::read(&bytes, &|| {
                                worker_cancelled.load(Ordering::Acquire)
                            })?
                            .into_iter()
                            .take(frame_limit)
                            .map(|frame| {
                                let preview = image::imageops::thumbnail(&frame.pixels, 192, 192);
                                let preview = egui::ColorImage::from_rgba_unmultiplied(
                                    [preview.width() as usize, preview.height() as usize],
                                    preview.as_raw(),
                                );
                                let source = crate::art_bank::ImportSource::File {
                                    path: path
                                        .to_str()
                                        .ok_or("Invalid artwork source path")?
                                        .into(),
                                };
                                crate::art_bank::encode(
                                    format!("{name}-{}", frame.name),
                                    frame.pixels,
                                    source,
                                )
                                .map(|art| ImportFrame { art, preview })
                            })
                            .collect::<Result<Vec<_>, String>>()?
                        } else {
                            let art = crate::art_bank::prepare_progressive(
                                path,
                                None,
                                |preview| {
                                    let _ = preview_tx.try_send(preview);
                                    ctx.request_repaint();
                                },
                                &|| worker_cancelled.load(Ordering::Acquire),
                            )?;
                            vec![ImportFrame {
                                art,
                                preview: egui::ColorImage::new([0, 0], Vec::new()),
                            }]
                        };
                        total = total.saturating_add(
                            frames
                                .iter()
                                .map(|f| f.art.bytes.len() + f.preview.pixels.len() * 4)
                                .sum::<usize>(),
                        );
                        if total > 128 * 1024 * 1024 {
                            return Err("Dropped artwork exceeds the 128 MiB batch limit".into());
                        }
                        let textures = (0..frames.len()).map(|_| None).collect();
                        prepared.push(ImportFile {
                            name,
                            frames,
                            entire: true,
                            selected: Default::default(),
                            textures,
                        });
                    }
                    Ok(prepared)
                })();
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .map_err(|e| e.to_string())?;
        self.image_drop = Some(Pending {
            project: self.editor.document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            target,
            receiver,
            cancelled,
            ready: None,
            preview_receiver,
            preview: None,
        });
        self.status = crate::desktop::logged_status("Importing dropped artwork…".into());
        Ok(())
    }
    pub(super) fn poll_image_drop(&mut self) {
        if let Some(job) = &mut self.image_drop
            && let Ok(preview) = job.preview_receiver.try_recv()
        {
            job.preview = Some(preview);
        }
        if self
            .image_drop
            .as_ref()
            .is_some_and(|job| job.ready.is_some())
        {
            return;
        }
        let result = self
            .image_drop
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Artwork processing stopped".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result
            && let Some(mut job) = self.image_drop.take()
        {
            let result = if self.editor.document().project()["projectId"] != job.project {
                Err("Artwork completed for a different project; result discarded".into())
            } else {
                result.and_then(|files| {
                    if files.iter().any(|f| f.frames.len() > 1) {
                        job.ready = Some(files);
                        self.status = crate::desktop::logged_status(
                            "Choose Photoshop artwork to import".into(),
                        );
                        Ok(())
                    } else {
                        self.commit_image_drop(
                            files
                                .into_iter()
                                .flat_map(|f| f.frames.into_iter().map(|f| f.art))
                                .collect(),
                            &job.target,
                        )
                    }
                })
            };
            if job.ready.is_some() {
                self.image_drop = Some(job);
                return;
            }
            self.report(result, "Artwork imported");
        }
    }
    pub(super) fn drop_picker_open(&self) -> bool {
        self.image_drop.as_ref().is_some_and(|j| j.ready.is_some())
    }
    pub(super) fn drop_preview(&self) -> Option<Preview> {
        self.image_drop
            .as_ref()
            .filter(|j| self.editor.document().project()["projectId"] == j.project)
            .and_then(|j| j.preview.clone())
    }
    pub(super) fn show_drop_picker(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.image_drop else {
            return;
        };
        let Some(files) = &mut job.ready else {
            return;
        };
        if self.editor.document().project()["projectId"] != job.project {
            self.image_drop = None;
            return;
        }
        let creating = matches!(job.target, ImageImportTarget::NewAchievements { .. });
        let mut import = false;
        let response = egui::Modal::new(egui::Id::new("photoshop-drop-picker"))
            .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(24))
            .show(ctx, |ui| {
                use achimeld_ui::buttons::{Button, ButtonStyle, UiButtonExt as _};
                ui.set_width(700.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.heading("Import Photoshop artwork");
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 210.0).max(180.0))
                    .show(ui, |ui| {
                        for file in files.iter_mut().filter(|f| f.frames.len() > 1) {
                            ui.push_id(&file.name, |ui| {
                                ui.strong(&file.name);
                                for (index, frame) in file.frames.iter().enumerate() {
                                    file.textures[index].get_or_insert_with(|| {
                                        ctx.load_texture(
                                            format!("photoshop-preview-{}-{index}", file.name),
                                            frame.preview.clone(),
                                            egui::TextureOptions::LINEAR.with_mipmap_mode(Some(
                                                egui::TextureFilter::Linear,
                                            )),
                                        )
                                    });
                                }
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(210.0);
                                        if ui
                                            .app_selectable_label(file.entire, "Entire image")
                                            .clicked()
                                        {
                                            file.entire = true;
                                        }
                                        ui.add(
                                            egui::Image::new((
                                                file.textures[0].as_ref().unwrap().id(),
                                                egui::Vec2::splat(192.0),
                                            ))
                                            .fit_to_exact_size(egui::Vec2::splat(192.0)),
                                        );
                                    });
                                    ui.vertical(|ui| {
                                        ui.label(if creating {
                                            "Choose a layer"
                                        } else {
                                            "Choose one or more layers"
                                        });
                                        for index in 1..file.frames.len() {
                                            let selected =
                                                !file.entire && file.selected.contains(&index);
                                            if ui
                                                .add(Button::selectable(
                                                    selected,
                                                    (
                                                        egui::Image::new((
                                                            file.textures[index]
                                                                .as_ref()
                                                                .unwrap()
                                                                .id(),
                                                            egui::Vec2::splat(36.0),
                                                        )),
                                                        file.frames[index].art.name.as_str(),
                                                    ),
                                                ))
                                                .clicked()
                                            {
                                                file.entire = false;
                                                if creating {
                                                    file.selected.clear();
                                                }
                                                if !file.selected.insert(index) {
                                                    file.selected.remove(&index);
                                                }
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                            });
                        }
                    });
                ui.horizontal(|ui| {
                    let valid = files.iter().all(|f| f.entire || !f.selected.is_empty());
                    if ui
                        .add_enabled(valid, Button::new("Import").style(ButtonStyle::Accent))
                        .clicked()
                    {
                        import = true;
                    }
                    if ui.app_button("Cancel").clicked() {
                        ui.close();
                    }
                });
            });
        if import {
            let Some(mut job) = self.image_drop.take() else {
                return;
            };
            let artwork = job
                .ready
                .take()
                .unwrap_or_default()
                .into_iter()
                .flat_map(|file| {
                    let indices = if file.entire {
                        [0].into()
                    } else {
                        file.selected
                    };
                    // Photoshop records are top first. Append from bottom to top.
                    file.frames
                        .into_iter()
                        .enumerate()
                        .rev()
                        .filter_map(move |(i, f)| indices.contains(&i).then_some(f.art))
                })
                .collect();
            let result = self.commit_image_drop(artwork, &job.target);
            self.report(result, "Artwork imported");
        } else if response.should_close() {
            self.image_drop = None;
            self.status = crate::desktop::logged_status("Artwork import cancelled".into());
        }
    }
    fn commit_image_drop(
        &mut self,
        artwork: Vec<crate::art_bank::Prepared>,
        target: &ImageImportTarget,
    ) -> Result<(), String> {
        let mut candidate = self.editor.document().clone();
        let editing_style = match target {
            ImageImportTarget::General {
                achievement_id,
                style_id: Some(style_id),
                ..
            }
            | ImageImportTarget::Layers {
                achievement_id,
                style_id: Some(style_id),
                ..
            } => {
                if candidate
                    .entity(EntityKind::Achievement, achievement_id)
                    .is_some_and(achimeld_application::style_achievements::is_local_icon)
                {
                    return Err("Style achievements edit only their own recipe".into());
                }
                let style = candidate
                    .icon_styles()
                    .into_iter()
                    .find(|s| s.id == *style_id)
                    .ok_or("Project style no longer exists")?;
                let signature = achimeld_application::style_signature(&style.recipe);
                let members = candidate
                    .entities(EntityKind::Achievement)
                    .iter()
                    .filter(|e| !achimeld_application::style_achievements::is_local_icon(e))
                    .filter(|e| {
                        e["id"] == *achievement_id
                            || achimeld_application::style_signature(&e["icon"]) == signature
                    })
                    .map(|e| e["id"].as_str().unwrap_or("").to_owned())
                    .collect::<Vec<_>>();
                Some((achievement_id.clone(), style, members))
            }
            _ => None,
        };
        let mut selected = None;
        let mut general_added = None;
        for art in artwork {
            let mut project = candidate.project().clone();
            let image_id = new_id();
            let reference = self.store.retain_art(&art.bytes)?;
            project["assets"]["images"].as_array_mut().ok_or("Invalid image library")?.push(json!({"id":image_id,"displayName":art.name,"image":reference,"kind":"User","pixelWidth":art.dimensions[0],"pixelHeight":art.dimensions[1],"fileBytes":art.bytes.len(),"importSource":art.source}));
            candidate = Document::from_parts(project, candidate.catalog().clone())?;
            if let ImageImportTarget::NewAchievements { name, position } = target {
                if name.trim().is_empty() {
                    return Err("Enter an achievement name".into());
                }
                let api = unique_api_name(name, &candidate);
                let position = achimeld_application::canvas::vacant_position(
                    EntityKind::Achievement,
                    candidate.entities(EntityKind::Achievement),
                    *position,
                );
                let edit = achimeld_application::automation::prepare(
                    &candidate,
                    "create_achievement",
                    &json!({"apiName":api,"displayName":name,"description":format!("{api} description"),"column":position[0],"row":position[1]}),
                )?;
                selected = edit.selection.map(|(_, id)| id);
                candidate = edit.document;
            }
            let (achievement, layer, general) = match target {
                ImageImportTarget::ArtBank => continue,
                ImageImportTarget::NewAchievements { .. } => (
                    selected.as_deref().ok_or("Achievement creation failed")?,
                    None,
                    true,
                ),
                ImageImportTarget::General {
                    achievement_id,
                    selected_layer_id,
                    ..
                } => (achievement_id.as_str(), selected_layer_id.as_deref(), true),
                ImageImportTarget::Layers {
                    achievement_id,
                    selected_layer_id,
                    ..
                } => (achievement_id.as_str(), selected_layer_id.as_deref(), false),
            };
            let mut project = candidate.project().clone();
            let entity = project["achievements"]
                .as_array_mut()
                .and_then(|list| list.iter_mut().find(|e| e["id"] == achievement))
                .ok_or("Achievement no longer exists")?;
            if general {
                let index = achimeld_application::append_general_image(
                    entity,
                    json!({"imageId":image_id,"atlasId":null,"tileIndex":0}),
                );
                if general_added.is_none() {
                    general_added = Some((achievement.to_owned(), index));
                    if let Some(selected) =
                        entity["icon"]["layers"].as_array_mut().and_then(|layers| {
                            layers.iter_mut().find(|l| {
                                l["id"].as_str() == layer && l["kind"] == "AchievementImage"
                            })
                        })
                    {
                        selected["generalImageIndex"] = json!(index);
                    }
                }
                if matches!(target, ImageImportTarget::NewAchievements { .. }) {
                    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":index,"displayName":"General image","visible":true,"opacity":1.0,"effects":[]}]});
                }
            } else {
                if entity["icon"].is_null() {
                    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":if entity["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"atlasId":entity["atlasId"],"imageId":entity["standaloneImageId"],"tileIndex":entity["tileIndex"],"displayName":"Icon","visible":true,"effects":[]}]});
                }
                let layers = entity["icon"]["layers"]
                    .as_array_mut()
                    .ok_or("Invalid icon stack")?;
                let parent = layers
                    .iter()
                    .find(|l| l["id"].as_str() == layer)
                    .map(|l| {
                        if l["kind"] == "Folder" {
                            l["id"].clone()
                        } else {
                            l["parentFolderId"].clone()
                        }
                    })
                    .unwrap_or(Value::Null);
                layers.push(json!({"id":new_id(),"displayName":art.name,"kind":"StandaloneImage","imageId":image_id,"visible":true,"opacity":1.0,"effects":[],"parentFolderId":parent}));
            }
            candidate = Document::from_parts(project, candidate.catalog().clone())?;
        }
        if let Some((achievement, mut style, members)) = editing_style {
            style.recipe = candidate
                .entity(EntityKind::Achievement, &achievement)
                .ok_or("Achievement no longer exists")?["icon"]
                .clone();
            let mut editor = Editor::new(candidate);
            editor.execute(
                "Update imported style",
                Command::SaveIconStyle(style.clone()),
                None,
            )?;
            candidate = editor.document().clone();
            let mut project = candidate.project().clone();
            for member in members.into_iter().filter(|id| id != &achievement) {
                if let Some(entity) = project["achievements"]
                    .as_array_mut()
                    .and_then(|list| list.iter_mut().find(|e| e["id"] == member))
                {
                    entity["icon"] = fresh_recipe(style.recipe.clone());
                }
            }
            candidate = Document::from_parts(project, candidate.catalog().clone())?;
        }
        self.finish_group();
        self.execute(
            "Import dropped artwork",
            Command::ImportArtwork(Box::new(candidate)),
            None,
        )?;
        if let Some(id) = selected {
            self.image_import_selection = Some((id, self.editor.revision()));
        }
        if let Some((id, index)) = general_added {
            self.general_image_added = Some((id, index, self.editor.revision()));
        }
        Ok(())
    }
}
fn fresh_recipe(mut recipe: Value) -> Value {
    if let Some(layers) = recipe["layers"].as_array_mut() {
        let ids: BTreeMap<_, _> = layers
            .iter()
            .filter_map(|l| l["id"].as_str().map(|id| (id.to_owned(), new_id())))
            .collect();
        for layer in layers {
            for key in ["id", "parentFolderId", "maskLayerId"] {
                if let Some(id) = layer[key].as_str().and_then(|id| ids.get(id)) {
                    layer[key] = json!(id);
                }
            }
            if let Some(effects) = layer["effects"].as_array_mut() {
                for effect in effects {
                    effect["id"] = json!(new_id());
                }
            }
        }
    }
    recipe
}
fn unique_api_name(name: &str, document: &Document) -> String {
    let mut stem = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() || "_.:-".contains(c) {
            stem.push(c.to_ascii_uppercase());
        } else if (c.is_whitespace() || c == '/' || c == '\\')
            && !stem.is_empty()
            && !stem.ends_with('_')
        {
            stem.push('_');
        }
    }
    let stem = stem.trim_matches('_');
    let stem = if stem.is_empty() { "ACHIEVEMENT" } else { stem };
    let stem = &stem[..stem.len().min(128)];
    for suffix in 1..=20001 {
        let tail = if suffix == 1 {
            String::new()
        } else {
            format!("_{suffix}")
        };
        let candidate = format!("{}{}", &stem[..stem.len().min(128 - tail.len())], tail);
        if !document.entities(EntityKind::Achievement).iter().any(|e| {
            e["apiName"]
                .as_str()
                .is_some_and(|api| api.eq_ignore_ascii_case(&candidate))
        }) {
            return candidate;
        }
    }
    new_id().replace('-', "").to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn artwork(color: u8) -> crate::art_bank::Prepared {
        crate::art_bank::encode(
            "Dropped image".into(),
            RgbaImage::from_pixel(4, 2, image::Rgba([color, 40, 90, 255])),
            crate::art_bank::ImportSource::File {
                path: "C:/art/source.png".into(),
            },
        )
        .unwrap()
    }
    fn host() -> Host {
        let mut host = Host::new(egui::Context::default());
        host.editor
            .execute("Fixture", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        host
    }
    fn photoshop(large: bool) -> Vec<u8> {
        let length = |n: usize| {
            if large {
                (n as u64).to_be_bytes().to_vec()
            } else {
                (n as u32).to_be_bytes().to_vec()
            }
        };
        let mut bytes = b"8BPS".to_vec();
        bytes.extend(if large { 2u16 } else { 1u16 }.to_be_bytes());
        bytes.extend([0; 6]);
        bytes.extend(4u16.to_be_bytes());
        bytes.extend(1u32.to_be_bytes());
        bytes.extend(2u32.to_be_bytes());
        bytes.extend(8u16.to_be_bytes());
        bytes.extend(3u16.to_be_bytes());
        bytes.extend([0; 8]);
        let mut info = (-1i16).to_be_bytes().to_vec();
        for coordinate in [0i32, 1, 1, 2] {
            info.extend(coordinate.to_be_bytes());
        }
        info.extend(4u16.to_be_bytes());
        for id in [0i16, 1, 2, -1] {
            info.extend(id.to_be_bytes());
            info.extend(length(3));
        }
        info.extend(b"8BIMnorm");
        info.extend([255, 0, 0, 0]);
        let extra = [0, 0, 0, 0, 0, 0, 0, 0, 3, b'A', b'r', b't'];
        info.extend((extra.len() as u32).to_be_bytes());
        info.extend(extra);
        for value in [80, 120, 160, 128] {
            info.extend([0, 0, value]);
        }
        let mut masks = length(info.len());
        masks.extend(info);
        masks.extend([0; 4]);
        bytes.extend(length(masks.len()));
        bytes.extend(masks);
        // The saved composite differs from the individual raster layer.
        bytes.extend([0, 0, 10, 20, 30, 40, 50, 60, 70, 80]);
        bytes
    }
    #[test]
    fn general_photoshop_drops_import_only_the_composite_without_a_picker() {
        for large in [false, true] {
            let mut host = host();
            let before = host.editor.document().clone();
            let id = before.entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let path = crate::test_output::root().join(format!(
                "drop-composite-{}.{}",
                new_id(),
                if large { "psb" } else { "psd" }
            ));
            let bytes = photoshop(large);
            assert_eq!(
                achimeld_imaging::photoshop::read(&bytes, &|| false)
                    .unwrap()
                    .len(),
                2
            );
            std::fs::write(&path, bytes).unwrap();
            host.start_image_drop(
                vec![path.clone()],
                ImageImportTarget::General {
                    achievement_id: id,
                    selected_layer_id: None,
                    style_id: None,
                },
            )
            .unwrap();
            for _ in 0..300 {
                host.poll_image_drop();
                assert!(!host.drop_picker_open(), "General drops must not prompt");
                if host.image_drop.is_none() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                host.image_drop.is_none(),
                "Import did not finish: {}",
                host.status
            );
            let after = host.editor.document().clone();
            let assets = after.project()["assets"]["images"].as_array().unwrap();
            assert_eq!(
                assets.len(),
                1,
                "Individual layers must not become General images"
            );
            let pixels = host.assets.image(&assets[0]["image"]).unwrap();
            assert_eq!(pixels.get_pixel(0, 0).0, [10, 30, 50, 70]);
            assert_eq!(pixels.get_pixel(1, 0).0, [20, 40, 60, 80]);
            assert_eq!(
                achimeld_application::general_image_entries(
                    &after.entities(EntityKind::Achievement)[0]
                )
                .len(),
                1
            );
            host.undo();
            assert_eq!(host.editor.document(), &before);
            host.redo();
            assert_eq!(host.editor.document(), &after);

            // Explicit layer imports retain the Photoshop picker.
            host.start_image_drop(
                vec![path],
                ImageImportTarget::Layers {
                    achievement_id: after.entities(EntityKind::Achievement)[0]["id"]
                        .as_str()
                        .unwrap()
                        .into(),
                    selected_layer_id: None,
                    style_id: None,
                },
            )
            .unwrap();
            for _ in 0..300 {
                host.poll_image_drop();
                if host.drop_picker_open() || host.image_drop.is_none() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                host.drop_picker_open(),
                "Layer imports still need a layer choice"
            );
            assert_eq!(host.editor.document(), &after);
        }
    }
    #[test]
    fn dropped_style_layers_update_matching_cards_and_undo_together() {
        let mut host = host();
        host.editor
            .execute("Another card", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        host.editor
            .execute(
                "Local style card",
                Command::AddStyleAchievementAt { position: [60, 0] },
                None,
            )
            .unwrap();
        let style = achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Shared".into(),
            recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF","effects":[]}]}),
        };
        host.editor
            .execute("Style", Command::SaveIconStyle(style.clone()), None)
            .unwrap();
        let mut project = host.editor.document().project().clone();
        for entity in project["achievements"].as_array_mut().unwrap() {
            entity["icon"] = fresh_recipe(style.recipe.clone());
        }
        let document =
            Document::from_parts(project, host.editor.document().catalog().clone()).unwrap();
        host.editor = Editor::new(document);
        let before = host.editor.document().clone();
        let id = before.entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        host.commit_image_drop(
            vec![artwork(80)],
            &ImageImportTarget::Layers {
                achievement_id: id,
                selected_layer_id: None,
                style_id: Some(style.id.clone()),
            },
        )
        .unwrap();
        let cards = host.editor.document().entities(EntityKind::Achievement);
        assert_eq!(cards[0]["icon"]["layers"].as_array().unwrap().len(), 2);
        assert_eq!(
            achimeld_application::style_signature(&cards[0]["icon"]),
            achimeld_application::style_signature(&cards[1]["icon"])
        );
        assert_eq!(cards[2], before.entities(EntityKind::Achievement)[2]);
        assert_ne!(
            cards[0]["icon"]["layers"][1]["id"],
            cards[1]["icon"]["layers"][1]["id"]
        );
        assert_eq!(
            host.editor
                .document()
                .icon_styles()
                .into_iter()
                .find(|saved| saved.id == style.id)
                .unwrap()
                .recipe["layers"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        host.undo();
        assert_eq!(host.editor.document(), &before);
    }
    #[test]
    fn local_style_general_image_import_renders_independently_and_undoes() {
        let mut host = host();
        host.editor
            .execute(
                "Local style",
                Command::AddStyleAchievementAt { position: [60, 0] },
                None,
            )
            .unwrap();
        let before = host.editor.document().clone();
        let id = before.entities(EntityKind::Achievement).last().unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        host.commit_image_drop(
            vec![artwork(180)],
            &ImageImportTarget::General {
                achievement_id: id.clone(),
                selected_layer_id: None,
                style_id: None,
            },
        )
        .unwrap();
        let after = host.editor.document().clone();
        let local = after.entity(EntityKind::Achievement, &id).unwrap();
        assert!(achimeld_application::style_achievements::is_style_achievement(local));
        assert_eq!(local["generalImageEntries"].as_array().unwrap().len(), 1);
        assert_eq!(
            after.entities(EntityKind::Achievement)[0],
            before.entities(EntityKind::Achievement)[0]
        );
        let rendered =
            achimeld_imaging::render(&after, &id, Some((16, 16)), &mut host.assets).unwrap();
        assert_eq!(rendered.achieved.get_pixel(8, 8).0, [180, 40, 90, 255]);
        host.editor.undo();
        assert_eq!(host.editor.document(), &before);
        host.editor.redo();
        assert_eq!(host.editor.document(), &after);
    }
    #[test]
    fn dropped_creation_names_general_icon_and_owned_art_undo_as_one_edit() {
        let mut host = host();
        let before = host.editor.document().clone();
        host.commit_image_drop(
            vec![artwork(50), artwork(70)],
            &ImageImportTarget::NewAchievements {
                name: "New image".into(),
                position: [40, 30],
            },
        )
        .unwrap();
        let after = host.editor.document().clone();
        let created = &after.entities(EntityKind::Achievement)[1..];
        assert_eq!(created.len(), 2);
        assert_eq!(created[0]["apiName"], "NEW_IMAGE");
        assert_eq!(created[1]["apiName"], "NEW_IMAGE_2");
        assert_eq!(
            [
                created[0]["canvasColumn"].as_i64().unwrap(),
                created[0]["canvasRow"].as_i64().unwrap()
            ],
            [40, 30]
        );
        let span = achimeld_application::canvas::cell_span(EntityKind::Achievement);
        for (index, entity) in after.entities(EntityKind::Achievement).iter().enumerate() {
            for other in &after.entities(EntityKind::Achievement)[..index] {
                assert!(
                    (entity["canvasColumn"].as_i64().unwrap()
                        - other["canvasColumn"].as_i64().unwrap())
                    .abs()
                        >= i64::from(span[0])
                        || (entity["canvasRow"].as_i64().unwrap()
                            - other["canvasRow"].as_i64().unwrap())
                        .abs()
                            >= i64::from(span[1])
                );
            }
        }
        for entity in created {
            assert_eq!(after.display_name(entity, "English"), "New image");
            assert_eq!(achimeld_application::general_image_entries(entity).len(), 1);
            assert_eq!(entity["icon"]["layers"][0]["kind"], "AchievementImage");
        }
        for asset in after.project()["assets"]["images"].as_array().unwrap() {
            assert_ne!(asset["image"]["path"], asset["importSource"]["path"]);
            assert!(host.assets.image(&asset["image"]).is_ok());
        }
        host.undo();
        assert_eq!(host.editor.document(), &before);
        host.redo();
        assert_eq!(host.editor.document(), &after);
        let path = crate::test_output::root().join(format!("drop-owned-{}.achv", new_id()));
        host.save_to(&path).unwrap();
        let loaded = ProjectStore::new().load(&path).unwrap();
        for asset in loaded.project()["assets"]["images"].as_array().unwrap() {
            assert_eq!(asset["image"]["kind"], "RelativeToProject");
            assert!(
                asset["image"]["path"]
                    .as_str()
                    .unwrap()
                    .contains("art-data-bank")
            );
        }
    }
    #[test]
    fn dropped_general_retargets_first_image_and_layers_preserve_folder_and_recipe() {
        let mut host = host();
        let id = host.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let folder = new_id();
        let linked = new_id();
        let mut entity = host
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap()
            .clone();
        entity["icon"] = json!({"layers":[{"id":folder,"kind":"Folder","displayName":"Folder"},{"id":linked,"kind":"AchievementImage","generalImageIndex":0,"parentFolderId":folder,"displayName":"Artwork"}]});
        host.execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
        let before = host.editor.document().clone();
        host.commit_image_drop(
            vec![artwork(90), artwork(100)],
            &ImageImportTarget::General {
                achievement_id: id.clone(),
                selected_layer_id: Some(linked.clone()),
                style_id: None,
            },
        )
        .unwrap();
        let entity = host
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap();
        assert_eq!(achimeld_application::general_image_entries(entity).len(), 2);
        assert_eq!(entity["icon"]["layers"][1]["generalImageIndex"], 0);
        host.undo();
        assert_eq!(host.editor.document(), &before);
        host.commit_image_drop(
            vec![artwork(90), artwork(100)],
            &ImageImportTarget::Layers {
                achievement_id: id.clone(),
                selected_layer_id: Some(folder.clone()),
                style_id: None,
            },
        )
        .unwrap();
        let entity = host
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap();
        let layers = entity["icon"]["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 4);
        assert_eq!(
            &layers[..2],
            before.entity(EntityKind::Achievement, &id).unwrap()["icon"]["layers"]
                .as_array()
                .unwrap()
        );
        assert!(layers[2..].iter().all(|l| l["parentFolderId"] == folder));
        host.undo();
        assert_eq!(host.editor.document(), &before);
        assert!(
            host.commit_image_drop(
                vec![artwork(50)],
                &ImageImportTarget::General {
                    achievement_id: new_id(),
                    selected_layer_id: None,
                    style_id: None,
                }
            )
            .is_err()
        );
        assert_eq!(host.editor.document(), &before);
    }
    #[test]
    fn failed_batch_and_project_switch_never_commit_partial_drops() {
        let mut host = host();
        let before = host.editor.document().clone();
        let path = crate::test_output::root().join(format!("drop-valid-{}.png", new_id()));
        RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        host.start_image_drop(
            vec![path.clone(), path.with_extension("missing")],
            ImageImportTarget::ArtBank,
        )
        .unwrap();
        for _ in 0..300 {
            host.poll_image_drop();
            if host.image_drop.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(host.image_drop.is_none());
        assert_eq!(host.editor.document(), &before);
        host.start_image_drop(vec![path], ImageImportTarget::ArtBank)
            .unwrap();
        host.editor = Editor::default();
        let fresh = host.editor.document().clone();
        for _ in 0..300 {
            host.poll_image_drop();
            if host.image_drop.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(host.image_drop.is_none());
        assert_eq!(host.editor.document(), &fresh);
    }
}
