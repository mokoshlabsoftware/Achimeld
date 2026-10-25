//! The supervised host owns the viewer's separate document, artwork and files.
mod collections;
use super::*;
use achimeld_application::{
    EntityKind,
    style_viewer::{
        SteamAchievementSetLink, StyleViewerAction as Action, StyleViewerCollection,
        StyleViewerImage, StyleViewerPreview, StyleViewerSource, StyleViewerState,
    },
};

struct Import {
    receiver: mpsc::Receiver<Result<crate::art_bank::Prepared, String>>,
    quick: mpsc::Receiver<Preview>,
    preview: Option<Preview>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Import {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

pub(super) struct Viewer {
    pub state: StyleViewerState,
    editor: Editor,
    store: ProjectStore,
    previews: PreviewWorker,
    achievement: String,
    import: Option<Import>,
    source_context: Option<(String, u64)>,
    collections: Vec<Collection>,
    library_revision: u64,
    render_document: Option<(u64, u64, Document)>,
    render_generation: u64,
    unavailable_collections: Vec<PathBuf>,
    collection_assets: std::collections::BTreeSet<String>,
}
struct Collection {
    summary: StyleViewerCollection,
    store: ProjectStore,
    steam_sets: Vec<SteamAchievementSetLink>,
    images: Vec<Value>,
}
struct LoadedSet {
    editor: Editor,
    store: ProjectStore,
    achievement: String,
    styles: Vec<achimeld_application::IconStyle>,
    selected: Option<String>,
    steam_sets: Vec<SteamAchievementSetLink>,
}
impl Viewer {
    fn new(host: &Host) -> Result<Self, String> {
        let mut store = ProjectStore::new();
        let cup = store.retain_art(include_bytes!(
            "../../../achimeld-application/assets/styles/achievement-cup.png"
        ))?;
        let cup_id = "c13dc47a-5afd-4bf3-a2bb-e5456020ac01";
        let trophy = store.retain_art(include_bytes!(
            "../../../achimeld-application/assets/styles/golden-trophy.png"
        ))?;
        let trophy_id = "c13dc47a-5afd-4bf3-a2bb-e5456020ac02";
        let default_images: Vec<Value> = (0..3)
            .map(|id| json!({"id":id,"imageId":trophy_id,"atlasId":null,"tileIndex":0}))
            .collect();
        let mut project = Document::new().project().clone();
        project["displayName"] = json!("Style set");
        project["styleViewerVersion"] = json!(1);
        project["assets"] = host.editor.document().project()["assets"].clone();
        project["atlas"] = host.editor.document().project()["atlas"].clone();
        let base = host
            .store
            .asset_base()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("C:\\Untitled.achv"));
        fn absolute(reference: &mut Value, base: &Path) -> Result<(), String> {
            if !reference.is_null() {
                let path = resolve_reference(reference, base)?;
                *reference =
                    json!({"kind":"Absolute","path":path.to_str().ok_or("Invalid artwork path")?});
            }
            Ok(())
        }
        absolute(&mut project["assets"]["atlas"], &base)?;
        for key in ["images", "atlases"] {
            if let Some(entries) = project["assets"][key].as_array_mut() {
                for entry in entries {
                    absolute(&mut entry["image"], &base)?;
                }
            }
        }
        project["assets"]["images"].as_array_mut().ok_or("Invalid preview image library")?
            .push(json!({"id":cup_id,"displayName":"Default achievement cup","kind":"User","image":cup,"pixelWidth":512,"pixelHeight":512}));
        project["assets"]["images"].as_array_mut().ok_or("Invalid preview image library")?
            .push(json!({"id":trophy_id,"displayName":"Golden trophy","kind":"User","image":trophy,"pixelWidth":512,"pixelHeight":512}));
        let styles = host.editor.document().icon_styles();
        project["iconStyles"] = json!(styles);
        let mut editor = Editor::new(Document::from_parts(project, Default::default())?);
        editor.execute(
            "Preview achievement",
            Command::Add(EntityKind::Achievement),
            None,
        )?;
        let achievement = editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .ok_or("Missing preview achievement")?
            .to_owned();
        let entity = editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .ok_or("Missing preview achievement")?;
        let name_id = entity["displayNameLocalizationId"]
            .as_str()
            .ok_or("Missing preview name")?
            .to_owned();
        let description_id = entity["descriptionLocalizationId"]
            .as_str()
            .ok_or("Missing preview description")?
            .to_owned();
        editor.execute(
            "Preview text",
            Command::Batch(vec![
                Command::SetLocalization {
                    id: name_id,
                    language: "English".into(),
                    text: "Unlock {0}".into(),
                },
                Command::SetLocalization {
                    id: description_id,
                    language: "English".into(),
                    text: "Preview achievement at {0}".into(),
                },
            ]),
            None,
        )?;
        let mut viewer = Self {
            state: StyleViewerState {
                styles,
                status: "Choose a style and add your preview images.".into(),
                ..Default::default()
            },
            editor,
            store,
            previews: PreviewWorker::new(host.ctx.clone()),
            achievement,
            import: None,
            source_context: None,
            collections: Vec::new(),
            library_revision: 0,
            render_document: None,
            render_generation: 0,
            unavailable_collections: Vec::new(),
            collection_assets: Default::default(),
        };
        viewer.collections.push(Collection {
            summary: StyleViewerCollection {
                id: new_id(), name: "Built-in".into(), dirty: false, read_only: true, steam_sets:Vec::new(),
                styles: vec![achimeld_application::IconStyle {
                    id: "b5947f1e-5d30-4609-900a-218314076c01".into(),
                    name: "Original image".into(), preview_png: None,
                    recipe: json!({"layers":[{"id":"b5947f1e-5d30-4609-900a-218314076c02","kind":"AchievementImage","generalImageIndex":0,"visible":true}]}),
                }],
            }, store: ProjectStore::new(), steam_sets: Vec::new(), images: Vec::new(),
        });
        for collection in achimeld_application::style_collections::builtins()? {
            viewer.collections.push(Collection {
                summary: StyleViewerCollection {
                    id: collection.id,
                    name: collection.name,
                    styles: collection.styles,
                    dirty: false,
                    read_only: true,
                    steam_sets: Vec::new(),
                },
                store: ProjectStore::new(),
                steam_sets: Vec::new(),
                images: Vec::new(),
            });
        }
        if !host.global_icon_styles().is_empty() {
            viewer.collections.push(Collection {
                summary: StyleViewerCollection {
                    id: new_id(),
                    name: "Saved app styles".into(),
                    styles: host.global_icon_styles().to_vec(),
                    dirty: false,
                    read_only: true,
                    steam_sets: Vec::new(),
                },
                store: ProjectStore::new(),
                steam_sets: Vec::new(),
                images: Vec::new(),
            });
        }
        for collection in &mut viewer.collections {
            collection.images = default_images.clone();
        }
        for path in &host.editor_state.state.style_collections {
            if let Err(error) = viewer.load_collection(path) {
                viewer.state.status = crate::desktop::logged_status(format!(
                    "Collection {}: {error}",
                    path.display()
                ));
                viewer.unavailable_collections.push(path.clone());
            }
        }
        viewer.sync_collections();
        viewer.sync_images();
        if let Some(style) = viewer.state.styles.first() {
            viewer.select(&style.id.clone())?;
        } else if let Some(collection) = viewer.collections.first()
            && let Some(style) = collection.summary.styles.first()
        {
            viewer.select(&format!("{}/{}", collection.summary.id, style.id))?;
        }
        viewer.editor.mark_saved();
        Ok(viewer)
    }
    fn sync_preview(
        &mut self,
        document: &Document,
        base: Option<&Path>,
        revision: u64,
    ) -> Result<(), String> {
        let context = (
            document.project()["projectId"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
            revision,
        );
        if self.source_context.as_ref() == Some(&context) {
            return Ok(());
        }
        let preview = StyleViewerPreview::from_project(document.project());
        let base = base.unwrap_or_else(|| Path::new("C:\\Untitled.achv"));
        let mut assets = document.project()["assets"].clone();
        let absolute = |reference: &mut Value| -> Result<(), String> {
            if !reference.is_null() {
                let path = resolve_reference(reference, base)?;
                *reference =
                    json!({"kind":"Absolute","path":path.to_str().ok_or("Invalid artwork path")?});
            }
            Ok(())
        };
        absolute(&mut assets["atlas"])?;
        for key in ["images", "atlases"] {
            if let Some(entries) = assets[key].as_array_mut() {
                for entry in entries.iter_mut() {
                    absolute(&mut entry["image"])?;
                }
                // Style sets retain their own imported artwork as well.
                for entry in self.editor.document().project()["assets"][key]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if !entries.iter().any(|item| item["id"] == entry["id"]) {
                        let mut entry = entry.clone();
                        let path = self.store.asset_base().unwrap_or(base);
                        if !entry["image"].is_null() {
                            let image = resolve_reference(&entry["image"], path)?;
                            entry["image"] = json!({"kind":"Absolute","path":image.to_str().ok_or("Invalid artwork path")?});
                        }
                        entries.push(entry);
                    }
                }
            }
        }
        let mut entity = self.entity().clone();
        entity["unlockValue"] = json!(preview.unlock_number()?);
        entity["unlockValueKind"] = json!("Float");
        let mut atlas = document.project()["atlas"].clone();
        // Preview size follows the existing viewer; source atlas geometry follows the project.
        for key in ["outputSize", "outputWidth", "outputHeight"] {
            if let Some(value) = self.editor.document().project()["atlas"].get(key) {
                atlas[key] = value.clone();
            }
        }
        self.editor.execute(
            "Style preview inputs",
            Command::Batch(vec![
                Command::ProjectSettings {
                    name: "Style set".into(),
                    app_id: 0,
                    atlas: Some(atlas),
                    assets: Some(assets),
                },
                Command::StyleViewerPreview(preview.clone()),
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: self.achievement.clone(),
                    value: entity,
                },
            ]),
            Some("style-viewer-inputs"),
        )?;
        self.state.preview = preview;
        self.source_context = Some(context);
        Ok(())
    }
    fn entity(&self) -> &Value {
        self.editor
            .document()
            .entity(EntityKind::Achievement, &self.achievement)
            .expect("Viewer owns one preview achievement")
    }
    fn replace_entity(&mut self, entity: Value) -> Result<(), String> {
        self.editor.execute(
            "Change style preview",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: self.achievement.clone(),
                value: entity,
            },
            Some("style-viewer-preview"),
        )
    }
    fn select(&mut self, id: &str) -> Result<(), String> {
        if let Some((collection, style)) = id.split_once('/') {
            if !self.collections.iter().any(|item| {
                item.summary.id == collection
                    && item.summary.styles.iter().any(|item| item.id == style)
            }) {
                return Err("Collection style is unavailable".into());
            }
            self.state.active_collection = Some(collection.into());
            self.state.selected_style = Some(id.into());
            self.state.status.clear();
            return Ok(());
        }
        self.state
            .styles
            .iter()
            .find(|style| style.id == id)
            .ok_or("Style no longer exists in this set")?;
        self.state.selected_style = Some(id.into());
        self.state.active_collection = None;
        self.state.status.clear();
        Ok(())
    }
    fn sync_images(&mut self) {
        self.state.images = achimeld_application::general_image_entries(self.entity())
            .iter()
            .filter_map(|entry| {
                let id = entry["imageId"].as_str()?;
                let asset = self.editor.document().project()["assets"]["images"]
                    .as_array()?
                    .iter()
                    .find(|asset| asset["id"] == id)?;
                Some(StyleViewerImage {
                    id: id.into(),
                    name: asset["displayName"].as_str().unwrap_or("Image").into(),
                })
            })
            .collect();
    }
    fn edit_images(&mut self, index: usize, up: Option<bool>) -> Result<(), String> {
        let mut entity = self.entity().clone();
        let mut entries = achimeld_application::general_image_entries(&entity);
        if index >= entries.len() {
            return Err("General image no longer exists".into());
        }
        if let Some(up) = up {
            let other = if up {
                index.checked_sub(1)
            } else {
                index.checked_add(1).filter(|next| *next < entries.len())
            }
            .ok_or("Image is already at the end")?;
            entries.swap(index, other);
        } else {
            entries.remove(index);
        }
        for (index, entry) in entries.iter_mut().enumerate() {
            entry["id"] = json!(index);
        }
        achimeld_application::store_general_image_entries(&mut entity, entries);
        self.replace_entity(entity)?;
        self.sync_images();
        self.state.dirty = true;
        Ok(())
    }
    fn add_prepared(&mut self, art: crate::art_bank::Prepared) -> Result<(), String> {
        if self.state.images.len() >= 32 {
            return Err("A style set supports up to 32 preview images".into());
        }
        let reference = self.store.retain_art(&art.bytes)?;
        let id = new_id();
        let mut assets = self.editor.document().project()["assets"].clone();
        assets["images"].as_array_mut().ok_or("Invalid viewer image library")?.push(json!({"id":id,"displayName":art.name,"image":reference,"kind":"User","pixelWidth":art.dimensions[0],"pixelHeight":art.dimensions[1]}));
        let mut entity = self.entity().clone();
        achimeld_application::append_general_image(
            &mut entity,
            json!({"imageId":id,"atlasId":null,"tileIndex":0}),
        );
        self.editor.execute(
            "Add viewer image",
            Command::Batch(vec![
                Command::ProjectSettings {
                    name: "Style set".into(),
                    app_id: 0,
                    atlas: None,
                    assets: Some(assets),
                },
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: self.achievement.clone(),
                    value: entity,
                },
            ]),
            None,
        )?;
        self.sync_images();
        self.state.dirty = true;
        self.state.status = crate::desktop::logged_status("Preview image added.".into());
        Ok(())
    }
    fn start_image(&mut self, path: PathBuf, ctx: &egui::Context) -> Result<(), String> {
        if self.import.is_some() || self.state.images.len() >= 32 {
            return Err(
                "Finish the current import; at most 32 preview images are supported".into(),
            );
        }
        let (tx, receiver) = mpsc::sync_channel(1);
        let (quick_tx, quick) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = crate::art_bank::prepare_progressive(
                path,
                None,
                |preview| {
                    let _ = quick_tx.try_send(preview);
                    ctx.request_repaint();
                },
                &|| stopped.load(Ordering::Acquire),
            );
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.import = Some(Import {
            receiver,
            quick,
            preview: None,
            cancel,
        });
        self.state.busy = true;
        Ok(())
    }
    fn poll(&mut self) {
        self.previews.poll();
        if let Some(error) = self.previews.error(&self.achievement) {
            self.state.status = crate::desktop::logged_status(format!("Preview: {error}"));
        }
        let Some(import) = &mut self.import else {
            return;
        };
        if let Ok(preview) = import.quick.try_recv() {
            import.preview = Some(preview);
        }
        let result = match import.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Image importer stopped".into()),
        };
        self.import = None;
        self.state.busy = false;
        if let Err(error) = result.and_then(|art| self.add_prepared(art)) {
            self.state.status = crate::desktop::logged_status(error);
        }
    }
    fn read_set(path: &Path) -> Result<LoadedSet, String> {
        let mut store = ProjectStore::new();
        let document = store.load(path)?;
        if document.project()["styleViewerVersion"] != 1
            || document.entities(EntityKind::Achievement).len() != 1
            || !document.entities(EntityKind::Stat).is_empty()
        {
            return Err("Choose a Style Viewer style set; the current set was preserved".into());
        }
        let achievement = document.entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .ok_or("Invalid preview achievement")?
            .to_owned();
        let entries = achimeld_application::general_image_entries(
            document
                .entity(EntityKind::Achievement, &achievement)
                .ok_or("Missing preview achievement")?,
        );
        if entries.len() > 32
            || entries.iter().enumerate().any(|(index, entry)| {
                entry["id"].as_u64() != Some(index as u64)
                    || entry["imageId"].as_str().is_none()
                    || !entry["atlasId"].is_null()
                    || !document.project()["assets"]["images"]
                        .as_array()
                        .is_some_and(|images| {
                            images.iter().any(|image| image["id"] == entry["imageId"])
                        })
            })
        {
            return Err("Invalid style-set preview images".into());
        }
        let styles = document.icon_styles();
        let selected = match document.project().get("styleViewerSelectedStyleId") {
            Some(Value::String(id)) if styles.iter().any(|style| style.id == *id) => {
                Some(id.clone())
            }
            Some(Value::Null) | None => styles.first().map(|style| style.id.clone()),
            _ => return Err("The style set's selected style is unavailable".into()),
        };
        let steam_sets: Vec<SteamAchievementSetLink> = document
            .project()
            .get("styleViewerSteamSets")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("Invalid Steam set links: {error}"))?
            .unwrap_or_default();
        if steam_sets.len() > 512 {
            return Err("Too many Steam set links".into());
        }
        for (index, link) in steam_sets.iter().enumerate() {
            link.validate()?;
            if !styles.iter().any(|style| style.id == link.style_id)
                || steam_sets[..index]
                    .iter()
                    .any(|previous| previous.same_target(link))
            {
                return Err(
                    "Steam set links reference a missing style or repeat a game/set".into(),
                );
            }
        }
        let mut editor = Editor::new(document);
        if let Some(id) = &selected {
            let mut entity = editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .ok_or("Missing preview achievement")?
                .clone();
            entity["icon"] = styles
                .iter()
                .find(|style| &style.id == id)
                .ok_or("Missing preview style")?
                .recipe
                .clone();
            editor.execute(
                "Load style preview",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: achievement.clone(),
                    value: entity,
                },
                None,
            )?;
            editor.mark_saved();
        }
        Ok(LoadedSet {
            editor,
            store,
            achievement,
            styles,
            selected,
            steam_sets,
        })
    }
    #[cfg(test)]
    fn load(&mut self, path: &Path) -> Result<(), String> {
        let LoadedSet {
            editor,
            store,
            achievement,
            styles,
            selected,
            steam_sets,
        } = Self::read_set(path)?;
        self.editor = editor;
        self.store = store;
        self.achievement = achievement;
        self.state = StyleViewerState {
            styles,
            steam_sets,
            selected_style: selected,
            status: "Style set loaded.".into(),
            ..Default::default()
        };
        self.source_context = None;
        self.sync_images();
        self.sync_collections();
        Ok(())
    }
    fn save(&mut self, path: &Path) -> Result<(), String> {
        let mut project = self.editor.document().project().clone();
        project["styleViewerSteamSets"] = json!(self.state.steam_sets);
        project["styleViewerSelectedStyleId"] = json!(
            self.state
                .selected_style
                .as_ref()
                .filter(|id| !id.contains('/'))
        );
        if let Some(style) = self
            .state
            .styles
            .iter()
            .find(|style| Some(&style.id) == self.state.selected_style.as_ref())
            && let Some(entity) = project["achievements"]
                .as_array_mut()
                .and_then(|items| items.first_mut())
        {
            entity["icon"] = style.recipe.clone();
        }
        let document = Self::export_document(project, self.editor.document().catalog().clone())?;
        self.store.save(&document, path)?;
        self.editor.mark_saved();
        self.state.dirty = false;
        self.state.status = crate::desktop::logged_status("Style set saved.".into());
        Ok(())
    }
}

impl Host {
    pub(super) fn ensure_style_viewer(&mut self) -> Result<(), String> {
        if self.style_viewer.is_none() {
            self.style_viewer = Some(Viewer::new(self)?);
        }
        self.style_viewer
            .as_mut()
            .ok_or("Style Viewer unavailable")?
            .sync_preview(
                self.editor.document(),
                self.store.asset_base(),
                self.editor.revision(),
            )?;
        Ok(())
    }
    pub(super) fn import_collection_style(
        &mut self,
        collection: &str,
        style: &str,
    ) -> Result<Value, String> {
        self.ensure_style_viewer()?;
        let source = self
            .style_viewer
            .as_ref()
            .ok_or("Style Viewer unavailable")?
            .collection_style(collection, style)?;
        let imported = achimeld_application::style_collections::create(
            &source.name,
            &source.recipe,
            &Value::Null,
        )?;
        let id = imported.id.clone();
        self.finish_group();
        self.execute(
            "Import collection icon style",
            Command::SaveIconStyle(imported),
            None,
        )?;
        Ok(json!({"styleId":id,"sourceStyleId":style,"collectionId":collection,"saved":false}))
    }
    pub(super) fn persist_viewer_collections(&mut self) {
        let Some(viewer) = &self.style_viewer else {
            return;
        };
        let paths = viewer.collection_paths();
        if self.editor_state.state.style_collections != paths
            && let Err(error) = self.editor_state.style_collections(paths)
            && let Some(viewer) = &mut self.style_viewer
        {
            viewer.state.status = crate::desktop::logged_status(format!(
                "Collection is open; could not save the library list: {error}"
            ));
        }
    }
    pub(super) fn viewer_action(&mut self, action: Action) {
        if let Action::ImportStyle(id) = &action {
            let result = id
                .split_once('/')
                .ok_or("Choose a collection style".to_owned())
                .and_then(|(collection, style)| self.import_collection_style(collection, style));
            if let Some(viewer) = &mut self.style_viewer {
                viewer.state.status = crate::desktop::logged_status(match result {
                    Ok(_) => {
                        "Style imported into Project styles. Save the project to keep it.".into()
                    }
                    Err(error) => error,
                });
            }
            return;
        }
        if matches!(action, Action::Open) {
            if self.style_viewer.is_none() {
                match Viewer::new(self) {
                    Ok(viewer) => self.style_viewer = Some(viewer),
                    Err(error) => self.status = crate::desktop::logged_status(error),
                }
            }
            self.sync_viewer_preview();
            return;
        }
        if matches!(
            action,
            Action::SelectImage(_) | Action::SetImageSlot { .. } | Action::SetUnlockValue(_)
        ) {
            let mut preview = StyleViewerPreview::from_project(self.editor.document().project());
            match action {
                Action::SelectImage(ref source)
                | Action::SetImageSlot {
                    source: Some(ref source),
                    ..
                } => {
                    let assets = &self.editor.document().project()["assets"];
                    let exists = match &source {
                        StyleViewerSource::Image { id } => assets["images"]
                            .as_array()
                            .is_some_and(|items| items.iter().any(|item| item["id"] == *id)),
                        StyleViewerSource::Atlas { id: Some(id), .. } => assets["atlases"]
                            .as_array()
                            .is_some_and(|items| items.iter().any(|item| item["id"] == *id)),
                        StyleViewerSource::Atlas { id: None, .. } => !assets["atlas"].is_null(),
                    };
                    if !exists {
                        return;
                    }
                    if let Action::SetImageSlot { index, .. } = action {
                        if preview.set_slot(index, Some(source.clone())).is_err() {
                            return;
                        }
                    } else {
                        preview.image = Some(source.clone());
                        preview.slots = None;
                        preview.default_slots = None;
                    }
                }
                Action::SetImageSlot {
                    index,
                    source: None,
                } => {
                    if preview.set_slot(index, None).is_err() {
                        return;
                    }
                }
                Action::SetUnlockValue(value) => preview.unlock_value = value.trim().into(),
                _ => unreachable!(),
            }
            let result = self.editor.execute(
                "Change style preview inputs",
                Command::StyleViewerPreview(preview),
                Some("style-viewer-inputs"),
            );
            if let Err(error) = result {
                if let Some(viewer) = &mut self.style_viewer {
                    viewer.state.status = crate::desktop::logged_status(error);
                }
            } else {
                self.sync_viewer_preview();
            }
            return;
        }
        let Some(viewer) = &mut self.style_viewer else {
            return;
        };
        if viewer.state.busy {
            return;
        }
        if let Action::UnloadCollection(id) = &action
            && viewer
                .collections
                .iter()
                .any(|item| item.summary.id == *id && item.summary.dirty)
        {
            self.start_dialog(Dialog::ViewerDiscard(ViewerNext::Unload(id.clone())));
            return;
        }
        match viewer.library_action(&action) {
            Ok(true) => {
                self.persist_viewer_collections();
                return;
            }
            Err(error) => {
                viewer.state.status = crate::desktop::logged_status(error);
                return;
            }
            Ok(false) => {}
        }
        let result = match action {
            Action::SelectStyle(id) => viewer.select(&id),
            Action::RenameStyle { id, name } => (|| {
                let mut style = viewer
                    .state
                    .styles
                    .iter()
                    .find(|style| style.id == id)
                    .cloned()
                    .ok_or("Style no longer exists")?;
                style.name = name.trim().into();
                style.validate()?;
                viewer.editor.execute(
                    "Rename viewer style",
                    Command::SaveIconStyle(style.clone()),
                    None,
                )?;
                if let Some(current) = viewer
                    .state
                    .styles
                    .iter_mut()
                    .find(|current| current.id == id)
                {
                    *current = style;
                }
                viewer.state.dirty = true;
                Ok(())
            })(),
            Action::LinkSet(link) => {
                (|| {
                    link.validate()?;
                    if !viewer
                        .state
                        .styles
                        .iter()
                        .any(|style| style.id == link.style_id)
                    {
                        return Err("Choose a style before linking a Steam set".into());
                    }
                    if let Some(existing) = viewer
                        .state
                        .steam_sets
                        .iter_mut()
                        .find(|existing| existing.same_target(&link))
                    {
                        if existing.style_id != link.style_id {
                            return Err("This Steam set is already linked to another style. Unlink it first.".into());
                        }
                        *existing = link;
                    } else if viewer.state.steam_sets.len() < 512 {
                        viewer.state.steam_sets.push(link);
                    } else {
                        return Err("A style set supports up to 512 Steam set links".into());
                    }
                    viewer.state.dirty = true;
                    viewer.state.status = crate::desktop::logged_status(
                        "Steam achievement set linked to this style.".into(),
                    );
                    Ok(())
                })()
            }
            Action::UnlinkSet(link) => {
                viewer.state.steam_sets.retain(|existing| existing != &link);
                viewer.state.dirty = true;
                Ok(())
            }
            Action::RemoveImage(index) => viewer.edit_images(index, None),
            Action::MoveImage { index, up } => viewer.edit_images(index, Some(up)),
            Action::RemoveStyle(id) => {
                let result = viewer.editor.execute(
                    "Remove viewer style",
                    Command::DeleteIconStyle(id.clone()),
                    None,
                );
                if result.is_ok() {
                    viewer.state.styles.retain(|style| style.id != id);
                    viewer.state.steam_sets.retain(|link| link.style_id != id);
                    viewer.state.dirty = true;
                    if viewer.state.selected_style.as_deref() == Some(&id) {
                        viewer.state.selected_style = None;
                        let mut entity = viewer.entity().clone();
                        entity["icon"] = Value::Null;
                        let _ = viewer.replace_entity(entity);
                    }
                }
                result
            }
            Action::AddImage => {
                self.start_dialog(Dialog::ViewerImage);
                Ok(())
            }
            Action::LoadSet => {
                self.start_dialog(Dialog::ViewerLoad);
                Ok(())
            }
            Action::SaveSet => self.save_viewer(false, None),
            Action::SaveSetAs => self.save_viewer(true, None),
            Action::Open
            | Action::ImportStyle(_)
            | Action::SelectImage(_)
            | Action::SetImageSlot { .. }
            | Action::SetUnlockValue(_)
            | Action::NewCollection
            | Action::SelectCollection(_)
            | Action::CopyToCollection { .. }
            | Action::UnloadCollection(_) => Ok(()),
        };
        if let Err(error) = result
            && let Some(viewer) = &mut self.style_viewer
        {
            viewer.state.status = crate::desktop::logged_status(error);
        }
    }
    pub(super) fn save_viewer(
        &mut self,
        save_as: bool,
        then: Option<ViewerNext>,
    ) -> Result<(), String> {
        let viewer = self
            .style_viewer
            .as_mut()
            .ok_or("Style Viewer is unavailable")?;
        let collection = if let Some(ViewerNext::Unload(id)) = &then {
            Some(id.clone())
        } else if matches!(&then, Some(ViewerNext::Exit)) {
            viewer
                .collections
                .iter()
                .find(|item| item.summary.dirty)
                .map(|item| item.summary.id.clone())
        } else {
            viewer.state.active_collection.clone()
        };
        if let Some(id) = &collection {
            if let Some(path) = viewer
                .collections
                .iter()
                .find(|item| &item.summary.id == id && !item.summary.read_only)
                .and_then(|item| item.store.path())
                .filter(|_| !save_as)
                .map(Path::to_path_buf)
            {
                viewer.save_collection(id, &path)?;
                self.persist_viewer_collections();
                self.viewer_continue(then);
            } else {
                self.start_dialog(Dialog::ViewerSave { then, collection });
            }
            return Ok(());
        }
        if let Some(path) = viewer
            .store
            .path()
            .filter(|_| !save_as)
            .map(Path::to_path_buf)
        {
            viewer.save(&path)?;
            self.viewer_continue(then);
        } else {
            self.start_dialog(Dialog::ViewerSave {
                then,
                collection: None,
            });
        }
        Ok(())
    }
    pub(super) fn viewer_dialog(&mut self, result: DialogResult) {
        let outcome = match result {
            DialogResult::ViewerDiscard(SaveChoice::Save, then) => {
                self.save_viewer(false, Some(then))
            }
            DialogResult::ViewerDiscard(SaveChoice::Discard, then) => {
                if let Some(viewer) = &mut self.style_viewer {
                    if let ViewerNext::Unload(id) = &then {
                        if let Some(collection) = viewer
                            .collections
                            .iter_mut()
                            .find(|item| item.summary.id == *id)
                        {
                            collection.summary.dirty = false;
                        }
                    } else {
                        viewer.state.dirty = false;
                        for collection in &mut viewer.collections {
                            collection.summary.dirty = false;
                        }
                    }
                    viewer.sync_collections();
                }
                self.viewer_continue(Some(then));
                Ok(())
            }
            DialogResult::ViewerSave {
                path: Some(path),
                then,
                collection,
            } => {
                let result = self
                    .style_viewer
                    .as_mut()
                    .ok_or("Style Viewer is unavailable".into())
                    .and_then(|viewer| match collection {
                        Some(id) => viewer.save_collection(&id, &path),
                        None => viewer.save(&path),
                    });
                if result.is_ok() {
                    self.persist_viewer_collections();
                    self.viewer_continue(then);
                }
                result
            }
            DialogResult::ViewerLoad(Some(path)) => self
                .style_viewer
                .as_mut()
                .ok_or("Style Viewer is unavailable".into())
                .and_then(|viewer| viewer.load_collection(&path)),
            DialogResult::ViewerImage(Some(path)) => self
                .style_viewer
                .as_mut()
                .ok_or("Style Viewer is unavailable".into())
                .and_then(|viewer| viewer.start_image(path, &self.ctx)),
            _ => Ok(()),
        };
        self.persist_viewer_collections();
        if let Err(error) = outcome
            && let Some(viewer) = &mut self.style_viewer
        {
            viewer.state.status = crate::desktop::logged_status(error);
        }
    }
    fn viewer_continue(&mut self, then: Option<ViewerNext>) {
        match then {
            #[cfg(test)]
            Some(ViewerNext::Load) => self.start_dialog(Dialog::ViewerLoad),
            Some(ViewerNext::Exit) => {
                let dirty = self.style_viewer.as_ref().is_some_and(|viewer| {
                    viewer.state.dirty || viewer.collections.iter().any(|item| item.summary.dirty)
                });
                if dirty {
                    if let Err(error) = self.save_viewer(false, Some(ViewerNext::Exit))
                        && let Some(viewer) = &mut self.style_viewer
                    {
                        viewer.state.status = crate::desktop::logged_status(error);
                    }
                } else {
                    self.continue_request(UiRequest::Exit);
                }
            }
            Some(ViewerNext::Unload(id)) => {
                if let Some(viewer) = &mut self.style_viewer
                    && let Err(error) = viewer.library_action(&Action::UnloadCollection(id))
                {
                    viewer.state.status = crate::desktop::logged_status(error);
                }
                self.persist_viewer_collections();
            }
            None => {}
        }
    }
    pub(super) fn poll_viewer(&mut self) {
        self.sync_viewer_preview();
        if let Some(viewer) = &mut self.style_viewer {
            viewer.poll();
        }
    }
    fn sync_viewer_preview(&mut self) {
        if let Some(viewer) = &mut self.style_viewer
            && let Err(error) = viewer.sync_preview(
                self.editor.document(),
                self.store.asset_base(),
                self.editor.revision(),
            )
        {
            viewer.state.status = crate::desktop::logged_status(error);
        }
    }
    pub(super) fn viewer_preview(&mut self) -> Option<Preview> {
        let id = self.style_viewer.as_ref()?.state.selected_style.clone()?;
        self.viewer_style_preview(&id, 512)
    }
    pub(super) fn viewer_style_preview(&mut self, style: &str, edge: usize) -> Option<Preview> {
        let viewer = self.style_viewer.as_mut()?;
        if !viewer.state.styles.iter().any(|item| item.id == style)
            && !style.split_once('/').is_some_and(|(collection, id)| {
                viewer.collections.iter().any(|item| {
                    item.summary.id == collection
                        && item.summary.styles.iter().any(|style| style.id == id)
                })
            })
        {
            return None;
        }
        viewer.prepare_render_document().ok()?;
        let edge = edge.clamp(64, 512).next_power_of_two();
        viewer.previews.preview(
            &format!("viewer-style:{edge}:{style}:{}", viewer.achievement),
            viewer.render_generation,
            &viewer.render_document.as_ref()?.2,
            viewer.store.asset_base(),
        )
    }
    pub(super) fn viewer_image_preview(&mut self, index: usize) -> Option<Preview> {
        let viewer = self.style_viewer.as_mut()?;
        viewer.prepare_render_document().ok()?;
        if index == viewer.state.images.len() {
            return viewer.import.as_ref()?.preview.clone();
        }
        let image = viewer.state.images.get(index)?;
        viewer.previews.preview(
            &format!("asset:image:{}", image.id),
            viewer.render_generation,
            &viewer.render_document.as_ref()?.2,
            viewer.store.asset_base(),
        )
    }
    pub(super) fn viewer_default_image_preview(&mut self) -> Option<Preview> {
        let viewer = self.style_viewer.as_mut()?;
        viewer.prepare_render_document().ok()?;
        viewer.previews.preview(
            "asset:image:c13dc47a-5afd-4bf3-a2bb-e5456020ac01",
            viewer.render_generation,
            &viewer.render_document.as_ref()?.2,
            viewer.store.asset_base(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use achimeld_application::{IconStyle, style_viewer::SteamAchievementSet};

    fn host() -> Host {
        let mut host = Host::new(egui::Context::default());
        host.editor
            .execute(
                "Canvas achievement",
                Command::Add(EntityKind::Achievement),
                None,
            )
            .unwrap();
        for (name, color) in [
            ("Image on blue", "#2040FFFF"),
            ("Image on red", "#FF2040FF"),
        ] {
            host.editor.execute("Canvas style", Command::SaveIconStyle(IconStyle {
                preview_png: None,
                id: new_id(), name: name.into(), recipe: json!({"layers":[
                    {"id":new_id(),"kind":"AchievementImage","generalImageIndex":0,"visible":true},
                    {"id":new_id(),"kind":"Solid","solidColorHex":color,"visible":true}
                ]}),
            }), None).unwrap();
        }
        host.editor.mark_saved();
        host
    }
    fn add_image(viewer: &mut Viewer, color: [u8; 4]) {
        let prepared = crate::art_bank::encode(
            "Custom preview".into(),
            RgbaImage::from_pixel(8, 8, image::Rgba(color)),
            crate::art_bank::ImportSource::Clipboard,
        )
        .unwrap();
        viewer.add_prepared(prepared).unwrap();
    }
    fn link(style: &str, app: u32) -> SteamAchievementSetLink {
        SteamAchievementSetLink {
            style_id: style.into(),
            app_id: app,
            set: SteamAchievementSet::Custom {
                id: "dlc_1".into(),
                name: "Expansion".into(),
            },
        }
    }
    fn wait_preview(host: &mut Host) -> Preview {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            host.poll_viewer();
            if let Some(preview) = host.viewer_preview() {
                return preview;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Viewer preview did not arrive"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn viewer_thumbnails_do_not_cancel_unselected_style_detail() {
        let mut host = host();
        host.viewer_action(Action::Open);
        let ids: Vec<_> = host
            .style_viewer
            .as_ref()
            .unwrap()
            .state
            .styles
            .iter()
            .map(|style| style.id.clone())
            .collect();
        assert!(ids.len() >= 2);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            host.poll_viewer();
            let ready = ids
                .iter()
                .map(|id| {
                    host.viewer_style_preview(id, 512)
                        .is_some_and(|p| p.width == 512)
                })
                .collect::<Vec<_>>();
            host.viewer_default_image_preview();
            host.viewer_image_preview(0);
            if ready.iter().all(|ready| *ready) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Visible unselected styles never reached final resolution"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn bank_atlas(host: &mut Host) -> String {
        let art = crate::art_bank::encode(
            "Preview atlas".into(),
            RgbaImage::from_fn(8, 4, |x, _| {
                image::Rgba(if x < 4 {
                    [240, 30, 20, 255]
                } else {
                    [20, 240, 60, 255]
                })
            }),
            crate::art_bank::ImportSource::Clipboard,
        )
        .unwrap();
        let reference = host.store.retain_art(&art.bytes).unwrap();
        let id = new_id();
        let mut assets = host.editor.document().project()["assets"].clone();
        assets["atlases"] = json!([{"id":id,"displayName":"Preview atlas","image":reference}]);
        let mut atlas = host.editor.document().project()["atlas"].clone();
        atlas["tileWidth"] = json!(4);
        atlas["tileHeight"] = json!(4);
        host.editor
            .execute(
                "Bank atlas",
                Command::ProjectSettings {
                    name: "Preview project".into(),
                    app_id: 0,
                    atlas: Some(atlas),
                    assets: Some(assets),
                },
                None,
            )
            .unwrap();
        id
    }
    #[test]
    fn three_preview_slots_render_independently_and_clear_without_reindexing() {
        let mut host = host();
        let atlas = bank_atlas(&mut host);
        let mut ids = Vec::new();
        for index in 0..3 {
            let id = new_id();
            host.editor.execute("Slot style", Command::SaveIconStyle(IconStyle {
                id:id.clone(), name:format!("ID {index}"), preview_png:None,
                recipe:json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":index,"visible":true}]}),
            }), None).unwrap();
            ids.push(id);
        }
        host.viewer_action(Action::Open);
        let entity = host.editor.document().entities(EntityKind::Achievement)[0].clone();
        for (index, tile) in [(0, 0), (1, 1), (2, 0)] {
            host.viewer_action(Action::SetImageSlot {
                index,
                source: Some(StyleViewerSource::Atlas {
                    id: Some(atlas.clone()),
                    tile,
                }),
            });
        }
        for (id, expected) in
            ids.iter()
                .zip([[240, 30, 20, 255], [20, 240, 60, 255], [240, 30, 20, 255]])
        {
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            loop {
                host.poll_viewer();
                if host
                    .viewer_style_preview(id, 256)
                    .is_some_and(|preview| preview.rgba[..4] == expected)
                {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Independent slot preview did not arrive"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        host.viewer_action(Action::SetImageSlot {
            index: 1,
            source: None,
        });
        let preview = StyleViewerPreview::from_project(host.editor.document().project());
        assert!(preview.slot(0).is_some());
        assert!(preview.slot(1).is_none());
        assert!(preview.slot(2).is_some());
        assert_eq!(
            host.editor.document().entities(EntityKind::Achievement)[0],
            entity
        );
        let serialized = serde_json::to_value(&preview).unwrap();
        assert_eq!(
            serde_json::from_value::<StyleViewerPreview>(serialized).unwrap(),
            preview
        );
        let revision = host.editor.revision();
        host.viewer_action(Action::SetImageSlot {
            index: 3,
            source: None,
        });
        assert_eq!(host.editor.revision(), revision);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            host.poll_viewer();
            if host
                .viewer_style_preview(&ids[1], 256)
                .is_some_and(|preview| {
                    preview
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|pixel| pixel[3] == 0)
                })
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Cleared slot kept stale artwork"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn collections_load_additively_and_save_only_the_selected_library() {
        let mut host = host();
        host.viewer_action(Action::Open);
        let folder = crate::test_output::root().join(format!("collections-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let first = folder.join("First.achv");
        let second = folder.join("Second.achv");
        let viewer = host.style_viewer.as_mut().unwrap();
        viewer.save(&first).unwrap();
        viewer.save(&second).unwrap();
        let original = viewer.state.styles.clone();
        let count = viewer.collections.len();
        viewer.load_collection(&first).unwrap();
        let one = viewer.state.active_collection.clone().unwrap();
        viewer.load_collection(&second).unwrap();
        let two = viewer.state.active_collection.clone().unwrap();
        assert_ne!(one, two);
        assert_eq!(viewer.collections.len(), count + 2);
        viewer.load_collection(&first).unwrap();
        assert_eq!(viewer.collections.len(), count + 2);
        assert_eq!(viewer.state.styles, original);
        let id = format!("{one}/{}", original[0].id);
        viewer
            .library_action(&Action::RenameStyle {
                id: id.clone(),
                name: "First collection only".into(),
            })
            .unwrap();
        assert_eq!(
            viewer
                .collections
                .iter()
                .find(|item| item.summary.id == two)
                .unwrap()
                .summary
                .styles,
            original
        );
        viewer.save_collection(&one, &first).unwrap();
        assert_eq!(
            Viewer::read_set(&first).unwrap().styles[0].name,
            "First collection only"
        );
        assert_eq!(Viewer::read_set(&second).unwrap().styles, original);
        assert_eq!(viewer.state.styles, original);
        let mut collection_link = link(&id, 480);
        viewer
            .library_action(&Action::LinkSet(collection_link.clone()))
            .unwrap();
        assert_eq!(
            viewer
                .collections
                .iter()
                .find(|item| item.summary.id == one)
                .unwrap()
                .steam_sets[0]
                .style_id,
            original[0].id
        );
        assert!(
            viewer
                .collections
                .iter()
                .find(|item| item.summary.id == two)
                .unwrap()
                .steam_sets
                .is_empty()
        );
        collection_link.style_id = format!("{one}/{}", original[1].id);
        assert!(
            viewer
                .library_action(&Action::LinkSet(collection_link))
                .is_err()
        );
        let ordinary = folder.join("Project.achv");
        ProjectStore::new()
            .save(&Document::new(), &ordinary)
            .unwrap();
        assert!(viewer.load_collection(&ordinary).is_err());
        assert_eq!(viewer.collections.len(), count + 2);
        viewer.select(&id).unwrap();
        assert!(!wait_preview(&mut host).rgba.is_empty());
    }
    #[test]
    fn collection_atlas_geometry_and_cancelled_unload_remain_local() {
        let mut source = host();
        let atlas = bank_atlas(&mut source);
        let style = new_id();
        source.editor.execute("Atlas style", Command::SaveIconStyle(IconStyle {
            id:style.clone(), name:"Collection atlas tile".into(), preview_png:None,
            recipe:json!({"layers":[{"id":new_id(),"kind":"AtlasTile","atlasId":atlas,"tileIndex":1,"visible":true}]}),
        }), None).unwrap();
        source.viewer_action(Action::Open);
        let folder = crate::test_output::root().join(format!("collection-grid-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Tiles.achv");
        source.style_viewer.as_mut().unwrap().save(&path).unwrap();
        let mut target = host();
        target.editor_state =
            crate::editor_state::EditorState::new(Some(folder.join("editor-state.json")));
        let before = target.editor.document().clone();
        target.viewer_action(Action::Open);
        target
            .style_viewer
            .as_mut()
            .unwrap()
            .load_collection(&path)
            .unwrap();
        let collection = target
            .style_viewer
            .as_ref()
            .unwrap()
            .state
            .active_collection
            .clone()
            .unwrap();
        let qualified = format!("{collection}/{style}");
        target.viewer_action(Action::SelectStyle(qualified.clone()));
        assert_eq!(wait_preview(&mut target).rgba[..4], [20, 240, 60, 255]);
        target.viewer_action(Action::RenameStyle {
            id: qualified,
            name: "Unsaved tile style".into(),
        });
        target.viewer_action(Action::UnloadCollection(collection.clone()));
        assert!(target.dialog.is_some());
        target.dialog = None;
        target.viewer_dialog(DialogResult::ViewerDiscard(
            SaveChoice::Cancel,
            ViewerNext::Unload(collection.clone()),
        ));
        assert!(
            target
                .style_viewer
                .as_ref()
                .unwrap()
                .collections
                .iter()
                .any(|item| item.summary.id == collection && item.summary.dirty)
        );
        target.viewer_dialog(DialogResult::ViewerDiscard(
            SaveChoice::Discard,
            ViewerNext::Unload(collection.clone()),
        ));
        assert!(
            !target
                .style_viewer
                .as_ref()
                .unwrap()
                .collections
                .iter()
                .any(|item| item.summary.id == collection)
        );
        assert_eq!(target.editor.document(), &before);
    }

    #[test]
    fn style_viewer_inputs_round_trip_with_project_and_render_live_atlas_instead_of_cached_banner()
    {
        use base64::Engine as _;
        let mut host = host();
        let atlas = bank_atlas(&mut host);
        let cached = crate::art_bank::encode(
            "Cached".into(),
            RgbaImage::from_pixel(4, 4, image::Rgba([180, 0, 180, 255])),
            crate::art_bank::ImportSource::Clipboard,
        )
        .unwrap();
        let style = IconStyle {
            id: new_id(),
            name: "Live banner".into(),
            preview_png: Some(base64::engine::general_purpose::STANDARD.encode(cached.bytes)),
            recipe: json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":7,"visible":true}]}),
        };
        host.editor
            .execute("Style", Command::SaveIconStyle(style.clone()), None)
            .unwrap();
        host.editor.mark_saved();
        let canvas = host.editor.document().project()["achievements"].clone();
        host.viewer_action(Action::Open);
        assert_eq!(
            host.style_viewer
                .as_ref()
                .unwrap()
                .state
                .preview
                .unlock_value,
            "10"
        );
        host.viewer_action(Action::SelectStyle(style.id.clone()));
        let source = StyleViewerSource::Atlas {
            id: Some(atlas),
            tile: 1,
        };
        host.viewer_action(Action::SelectImage(source.clone()));
        host.viewer_action(Action::SetUnlockValue("123.50".into()));
        let viewer = host.style_viewer.as_ref().unwrap();
        assert_eq!(
            viewer
                .editor
                .document()
                .display_name(viewer.entity(), "English"),
            "Unlock 123.5"
        );
        assert!(!viewer.state.dirty);
        assert_eq!(host.editor.document().project()["achievements"], canvas);
        let preview = wait_preview(&mut host);
        assert_eq!(&preview.rgba[..4], &[20, 240, 60, 255]);
        host.viewer_action(Action::RenameStyle {
            id: style.id.clone(),
            name: "Cave art".into(),
        });
        assert_eq!(
            host.style_viewer
                .as_ref()
                .unwrap()
                .state
                .styles
                .iter()
                .find(|s| s.id == style.id)
                .unwrap()
                .name,
            "Cave art"
        );
        assert_eq!(
            host.editor
                .document()
                .icon_styles()
                .iter()
                .find(|s| s.id == style.id)
                .unwrap()
                .name,
            "Live banner"
        );
        let folder = crate::test_output::root().join(format!("viewer-project-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Preview project.achv");
        host.store.save(host.editor.document(), &path).unwrap();
        let mut reopened = Host::new(egui::Context::default());
        reopened.editor = Editor::new(reopened.store.load(&path).unwrap());
        reopened.viewer_action(Action::Open);
        let state = &reopened.style_viewer.as_ref().unwrap().state;
        assert_eq!(state.preview.image, Some(source));
        assert_eq!(state.preview.unlock_value, "123.50");
        reopened.viewer_action(Action::SelectStyle(style.id));
        assert_eq!(&wait_preview(&mut reopened).rgba[..4], &[20, 240, 60, 255]);
        let before = reopened.editor.document().clone();
        reopened.viewer_action(Action::SetUnlockValue("NaN".into()));
        assert_eq!(reopened.editor.document(), &before);
        let _ = std::fs::remove_dir_all(folder);
    }
    #[test]
    fn style_viewer_round_trip_owns_images_and_never_changes_canvas_history() {
        let mut host = host();
        let before = host.editor.document().clone();
        let revision = host.editor.revision();
        let history = host.editor.undo_label().map(str::to_owned);
        host.viewer_action(Action::Open);
        // A missing General image must leave the other style layers renderable.
        assert!(!wait_preview(&mut host).rgba.is_empty());
        let styles = host.style_viewer.as_ref().unwrap().state.styles.clone();
        add_image(host.style_viewer.as_mut().unwrap(), [20, 240, 60, 255]);
        add_image(host.style_viewer.as_mut().unwrap(), [220, 180, 20, 255]);
        host.viewer_action(Action::MoveImage { index: 1, up: true });
        host.viewer_action(Action::RemoveImage(1));
        host.viewer_action(Action::SelectStyle(styles[1].id.clone()));
        host.viewer_action(Action::LinkSet(link(&styles[1].id, 480)));
        let folder = crate::test_output::root().join(format!("viewer-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Style set.achv");
        host.style_viewer.as_mut().unwrap().save(&path).unwrap();
        host.style_viewer = None; // Drops the original staging directory and render pool.
        host.viewer_action(Action::Open);
        host.style_viewer.as_mut().unwrap().load(&path).unwrap();
        let viewer = host.style_viewer.as_ref().unwrap();
        assert_eq!(viewer.state.images.len(), 1);
        assert_eq!(viewer.state.steam_sets, [link(&styles[1].id, 480)]);
        let image_id = &viewer.state.images[0].id;
        let reference = &viewer.editor.document().project()["assets"]["images"]
            .as_array()
            .unwrap()
            .iter()
            .find(|asset| asset["id"] == *image_id)
            .unwrap()["image"];
        let owned = resolve_reference(reference, &path).unwrap();
        assert!(
            owned
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".art-data-bank")
        );
        assert_eq!(
            image::open(owned).unwrap().to_rgba8().get_pixel(0, 0).0,
            [220, 180, 20, 255]
        );
        assert!(!wait_preview(&mut host).rgba.is_empty());
        assert_eq!(host.editor.document(), &before);
        assert_eq!(host.editor.revision(), revision);
        assert_eq!(host.editor.undo_label(), history.as_deref());
        assert!(!host.editor.is_dirty());
        assert!(host.store.path().is_none());
        // Loading a style set via File > Open cannot replace the canvas.
        assert!(host.open(&path).is_err());
        assert_eq!(host.editor.document(), &before);
    }

    #[test]
    fn steam_set_links_are_app_scoped_and_conflicts_or_invalid_loads_preserve_work() {
        let mut host = host();
        host.viewer_action(Action::Open);
        let styles = host.style_viewer.as_ref().unwrap().state.styles.clone();
        let first = link(&styles[0].id, 480);
        host.viewer_action(Action::LinkSet(first.clone()));
        host.viewer_action(Action::LinkSet(link(&styles[1].id, 481)));
        let links = host.style_viewer.as_ref().unwrap().state.steam_sets.clone();
        host.viewer_action(Action::LinkSet(link(&styles[1].id, 480)));
        assert_eq!(host.style_viewer.as_ref().unwrap().state.steam_sets, links);
        assert!(
            host.style_viewer
                .as_ref()
                .unwrap()
                .state
                .status
                .contains("already linked")
        );
        let mut invalid = first.clone();
        invalid.app_id = 0;
        host.viewer_action(Action::LinkSet(invalid));
        assert_eq!(host.style_viewer.as_ref().unwrap().state.steam_sets, links);
        host.viewer_action(Action::LoadSet);
        assert!(host.dialog.is_some());
        host.dialog = None;
        host.viewer_dialog(DialogResult::ViewerDiscard(
            SaveChoice::Cancel,
            ViewerNext::Load,
        ));
        assert_eq!(host.style_viewer.as_ref().unwrap().state.steam_sets, links);
        host.request(UiRequest::Exit);
        assert!(!host.exit_allowed);
        host.dialog = None;
        host.viewer_dialog(DialogResult::ViewerDiscard(
            SaveChoice::Save,
            ViewerNext::Exit,
        ));
        assert!(host.dialog.is_some());
        host.dialog = None;
        host.viewer_dialog(DialogResult::ViewerSave {
            path: None,
            then: Some(ViewerNext::Exit),
            collection: None,
        });
        assert!(!host.exit_allowed);
        let folder = crate::test_output::root().join(format!("viewer-invalid-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("Invalid set.achv");
        let viewer = host.style_viewer.as_mut().unwrap();
        let mut project = viewer.editor.document().project().clone();
        project["styleViewerSteamSets"] = json!([first.clone(), first.clone()]);
        ProjectStore::new()
            .save(
                &Document::from_parts(project, viewer.editor.document().catalog().clone()).unwrap(),
                &path,
            )
            .unwrap();
        assert!(viewer.load(&path).is_err());
        assert_eq!(viewer.state.steam_sets, links);
        assert_eq!(viewer.state.styles.len(), 2);
        host.viewer_action(Action::UnlinkSet(first));
        host.viewer_action(Action::RemoveStyle(styles[1].id.clone()));
        assert!(
            host.style_viewer
                .as_ref()
                .unwrap()
                .state
                .steam_sets
                .is_empty()
        );
    }

    #[test]
    #[ignore = "Native Style Viewer visual acceptance; writes to external current outputs"]
    fn native_style_viewer_screenshot() {
        use winit::platform::windows::EventLoopBuilderExtWindows as _;
        struct Window {
            host: Host,
            ui: achimeld_ui::Desktop,
            started: std::time::Instant,
            requested: bool,
            output: PathBuf,
        }
        impl eframe::App for Window {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                self.host.poll_viewer();
                self.host.previews.poll();
                self.ui.show(ui, &mut self.host);
                if !self.requested && self.started.elapsed().as_secs_f32() > 1.5 {
                    self.requested = true;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                            egui::UserData::default(),
                        ));
                }
                for event in ui.input(|input| input.raw.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let bytes: Vec<_> = image
                            .pixels
                            .iter()
                            .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                            .collect();
                        image::save_buffer(
                            &self.output,
                            &bytes,
                            image.width() as u32,
                            image.height() as u32,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                assert!(
                    self.started.elapsed() < Duration::from_secs(20),
                    "Native screenshot timed out"
                );
                ui.ctx().request_repaint();
            }
        }
        let output = crate::test_output::root().join("native-style-viewer.png");
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1180.0, 900.0])
                .with_decorations(false),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };
        eframe::run_native(
            "Achimeld Style Viewer acceptance",
            options,
            Box::new(move |cc| {
                crate::ui_fonts::configure(&cc.egui_ctx);
                let mut host = host();
                host.ctx = cc.egui_ctx.clone();
                bank_atlas(&mut host);
                host.viewer_action(Action::Open);
                let image = RgbaImage::from_fn(128, 128, |x, y| {
                    let light = ((x / 16 + y / 16) % 2) as u8;
                    image::Rgba([
                        40 + light * 50,
                        120 + (x / 2) as u8,
                        160 + (y / 2) as u8,
                        255,
                    ])
                });
                let art = crate::art_bank::encode(
                    "Preview artwork".into(),
                    image,
                    crate::art_bank::ImportSource::Clipboard,
                )
                .unwrap();
                let image_id = new_id();
                let reference = host.store.retain_art(&art.bytes).unwrap();
                let mut assets = host.editor.document().project()["assets"].clone();
                assets["images"] =
                    json!([{"id":image_id,"displayName":"Preview artwork","image":reference}]);
                host.editor
                    .execute(
                        "Preview artwork",
                        Command::ProjectSettings {
                            name: "Style preview".into(),
                            app_id: 0,
                            atlas: None,
                            assets: Some(assets),
                        },
                        None,
                    )
                    .unwrap();
                host.viewer_action(Action::SelectImage(StyleViewerSource::Image {
                    id: image_id,
                }));
                host.style_viewer
                    .as_mut()
                    .unwrap()
                    .add_prepared(art)
                    .unwrap();
                let id = host.style_viewer.as_ref().unwrap().state.styles[0]
                    .id
                    .clone();
                host.viewer_action(Action::LinkSet(link(&id, 480)));
                let mut ui = achimeld_ui::Desktop::default();
                ui.open_style_viewer();
                Ok(Box::new(Window {
                    host,
                    ui,
                    started: std::time::Instant::now(),
                    requested: false,
                    output,
                }))
            }),
        )
        .unwrap();
    }
}
