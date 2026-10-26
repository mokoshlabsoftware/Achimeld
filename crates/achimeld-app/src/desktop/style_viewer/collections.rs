//! Additive collection libraries share the viewer's bounded renderer and artwork cache.
use super::*;

impl Viewer {
    pub(in crate::desktop) fn style_export_document(
        &self,
        collection: &str,
        style: &str,
    ) -> Result<(Document, String), String> {
        let style = self.collection_style(collection, style)?;
        achimeld_application::style_collections::portable(&style)?;
        let mut project = self.editor.document().project().clone();
        let entity = project["achievements"]
            .as_array_mut()
            .and_then(|a| a.first_mut())
            .ok_or("Missing preview achievement")?;
        entity["icon"] = style.recipe;
        entity["unlockValue"] = json!(self.state.preview.unlock_number()?);
        entity["generalImageEntries"] = json!(
            self.collections
                .iter()
                .find(|c| c.summary.id == collection)
                .map(|c| c.images.clone())
                .unwrap_or_default()
        );
        let defaults: Vec<_> = (0..3).map(|id| json!({"id":id,"imageId":"c13dc47a-5afd-4bf3-a2bb-e5456020ac01","atlasId":null,"tileIndex":0})).collect();
        self.state.preview.apply_images(entity, &defaults);
        Ok((
            Document::from_parts(project, self.editor.document().catalog().clone())?,
            self.achievement.clone(),
        ))
    }
    pub(in crate::desktop) fn library_revision(&self) -> u64 {
        self.library_revision
    }

    pub(in crate::desktop) fn collection_style(
        &self,
        collection: &str,
        style: &str,
    ) -> Result<achimeld_application::IconStyle, String> {
        self.collections
            .iter()
            .find(|c| c.summary.id == collection)
            .and_then(|c| c.summary.styles.iter().find(|s| s.id == style))
            .cloned()
            .ok_or("Unknown collection or style identity".into())
    }

    pub(in crate::desktop) fn collection_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        use achimeld_application::style_collections;
        let string = |key: &str| args[key].as_str().ok_or_else(|| format!("Missing {key}"));
        match operation {
            "list_style_collections" => Ok(json!(self.collections.iter().map(|c| json!({
                "id":c.summary.id,"name":c.summary.name,"readOnly":c.summary.read_only,"dirty":c.summary.dirty,
                "styles":c.summary.styles.iter().map(style_collections::summary).collect::<Vec<_>>()
            })).collect::<Vec<_>>())),
            "get_icon_style" => Ok(json!(self.collection_style(string("collectionId")?, string("styleId")?)?)),
            "create_style_collection" => {
                let name = string("name")?.trim();
                if name.is_empty() || name.len() > 256 { return Err("Enter a collection name of 1–256 UTF-8 bytes".into()); }
                let id = new_id();
                self.collections.push(Collection {
                    summary: StyleViewerCollection {id:id.clone(), name:name.into(), styles:Vec::new(), dirty:true, read_only:false, steam_sets:Vec::new()},
                    store:ProjectStore::new(), steam_sets:Vec::new(), images:Vec::new(),
                });
                self.state.active_collection = Some(id.clone());
                self.sync_collections();
                Ok(json!({"collectionId":id,"saved":false}))
            }
            "create_icon_style" => {
                let collection = string("collectionId")?;
                if args["recipe"].is_object() == args["sourceStyleId"].is_string() {
                    return Err("Supply exactly one recipe or sourceStyleId with sourceCollectionId".into());
                }
                let recipe = if args["recipe"].is_object() {
                    if !args["sourceCollectionId"].is_null() { return Err("sourceCollectionId requires sourceStyleId".into()); }
                    args["recipe"].clone()
                } else {
                    self.collection_style(string("sourceCollectionId")?, string("sourceStyleId")?)?.recipe
                };
                let style = style_collections::create(string("name")?, &recipe, &args["edits"])?;
                let target = self.collections.iter_mut().find(|c| c.summary.id == collection && !c.summary.read_only)
                    .ok_or("Choose an editable collection; create_style_collection first")?;
                if target.summary.styles.len() >= 128 { return Err("A collection supports up to 128 styles".into()); }
                let library_bytes: usize = self.state.collections.iter().flat_map(|c| &c.styles)
                    .map(|s| serde_json::to_vec(s).map_or(usize::MAX, |b| b.len())).fold(0usize, usize::saturating_add);
                let bytes = library_bytes.saturating_add(serde_json::to_vec(self.editor.document().project()).map_err(|e| e.to_string())?.len());
                if bytes.saturating_add(serde_json::to_vec(&style).map_err(|e| e.to_string())?.len()) > 128 * 1024 * 1024 {
                    return Err("Open style libraries exceed 128 MiB; unload a collection".into());
                }
                let result = json!({"collectionId":collection,"style":style,"saved":false});
                target.summary.styles.push(style);
                target.summary.dirty = true;
                self.sync_collections();
                Ok(result)
            }
            "export_style_collection" => {
                let path = PathBuf::from(string("destinationFilePath")?);
                if !super::super::automation::local_path(&path) || path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("achv")) || path.exists() {
                    return Err("Choose an unused absolute local-drive .achv destination".into());
                }
                achimeld_security::windows::reject_reparse(path.parent().ok_or("Missing destination folder")?).map_err(|e| e.to_string())?;
                self.save_collection(string("collectionId")?, &path)?;
                Ok(json!({"filePath":path,"saved":true}))
            }
            _ => Err("Unknown collection operation".into())
        }
    }

    pub(super) fn sync_collections(&mut self) {
        self.state.collections = self
            .collections
            .iter()
            .map(|item| {
                let mut summary = item.summary.clone();
                summary.steam_sets = item.steam_sets.clone();
                summary
            })
            .collect();
        self.library_revision = self.library_revision.wrapping_add(1);
    }

    pub(super) fn collection_paths(&self) -> Vec<PathBuf> {
        self.collections
            .iter()
            .filter(|item| !item.summary.read_only)
            .filter_map(|item| item.store.path().map(Path::to_path_buf))
            .chain(self.unavailable_collections.iter().cloned())
            .collect()
    }

    pub(super) fn load_collection(&mut self, path: &Path) -> Result<(), String> {
        let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
        if let Some(collection) = self
            .collections
            .iter()
            .find(|item| item.store.path() == Some(absolute.as_path()))
        {
            self.state.active_collection = Some(collection.summary.id.clone());
            return Ok(());
        }
        let LoadedSet {
            editor,
            store,
            achievement,
            mut styles,
            steam_sets,
            selected,
        } = Self::read_set(&absolute)?;
        let mut incoming = editor.document().project()["assets"].clone();
        let mut assets = self.editor.document().project()["assets"].clone();
        let mut identities = std::collections::BTreeMap::new();
        for key in ["images", "atlases"] {
            for asset in incoming[key].as_array_mut().into_iter().flatten() {
                if key == "atlases" {
                    let grid = asset
                        .get("styleViewerAtlasGrid")
                        .cloned()
                        .unwrap_or_else(|| editor.document().project()["atlas"].clone());
                    let empty = Document::new();
                    let mut candidate = empty.project().clone();
                    candidate["atlas"] = grid.clone();
                    Document::from_parts(candidate, empty.catalog().clone())?;
                    asset["styleViewerAtlasGrid"] = grid;
                }
                let old = asset["id"]
                    .as_str()
                    .ok_or("Missing collection artwork identity")?
                    .to_owned();
                let id = new_id();
                identities.insert(old, id.clone());
                asset["id"] = json!(id);
                let image = resolve_reference(&asset["image"], &absolute)?;
                asset["image"] =
                    json!({"kind":"Absolute","path":image.to_str().ok_or("Invalid artwork path")?});
                assets[key]
                    .as_array_mut()
                    .ok_or("Invalid artwork library")?
                    .push(asset.clone());
            }
        }
        if !incoming["atlas"].is_null() {
            let id = new_id();
            identities.insert("00000000-0000-0000-0000-000000000000".into(), id.clone());
            let image = resolve_reference(&incoming["atlas"], &absolute)?;
            assets["atlases"].as_array_mut().ok_or("Invalid atlas library")?.push(json!({
                "id":id,"displayName":"Collection atlas","image":{"kind":"Absolute","path":image.to_str().ok_or("Invalid artwork path")?},"styleViewerAtlasGrid":editor.document().project()["atlas"]
            }));
        }
        fn remap(value: &mut Value, identities: &std::collections::BTreeMap<String, String>) {
            match value {
                Value::Object(object) => {
                    if object.get("kind").is_some_and(|value| value == "AtlasTile")
                        && object.get("atlasId").is_none_or(Value::is_null)
                        && let Some(id) = identities.get("00000000-0000-0000-0000-000000000000")
                    {
                        object.insert("atlasId".into(), json!(id));
                    }
                    for (key, value) in object.iter_mut() {
                        if matches!(key.as_str(), "imageId" | "atlasId") {
                            if let Some(id) = value.as_str().and_then(|id| identities.get(id)) {
                                *value = json!(id);
                            }
                        } else {
                            remap(value, identities);
                        }
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        remap(value, identities);
                    }
                }
                _ => {}
            }
        }
        for style in &mut styles {
            remap(&mut style.recipe, &identities);
            style.validate()?;
        }
        let mut images = achimeld_application::general_image_entries(
            editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .ok_or("Missing preview achievement")?,
        );
        for image in &mut images {
            remap(image, &identities);
        }
        // Validate the combined candidate before changing the open libraries.
        let mut project = self.editor.document().project().clone();
        project["assets"] = assets.clone();
        let mut bytes = serde_json::to_vec(&project)
            .map_err(|error| error.to_string())?
            .len();
        for collection in &self.collections {
            bytes = bytes.saturating_add(
                serde_json::to_vec(&collection.summary.styles)
                    .map_err(|error| error.to_string())?
                    .len(),
            );
        }
        bytes = bytes.saturating_add(
            serde_json::to_vec(&styles)
                .map_err(|error| error.to_string())?
                .len(),
        );
        if bytes > 128 * 1024 * 1024 {
            return Err("The open style libraries exceed the preview memory budget. Unload a collection first.".into());
        }
        let _ = Document::from_parts(project, self.editor.document().catalog().clone())?;
        self.editor.execute(
            "Load collection artwork",
            Command::ProjectSettings {
                name: "Style set".into(),
                app_id: 0,
                atlas: None,
                assets: Some(assets),
            },
            None,
        )?;
        self.collection_assets.extend(identities.values().cloned());
        let id = new_id();
        self.collections.push(Collection {
            summary: StyleViewerCollection {
                id: id.clone(),
                name: absolute
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Style collection")
                    .into(),
                styles,
                dirty: false,
                read_only: false,
                steam_sets: Vec::new(),
            },
            store,
            steam_sets,
            images,
        });
        self.unavailable_collections
            .retain(|path| path != &absolute);
        self.state.active_collection = Some(id.clone());
        if let Some(style) = selected {
            self.state.selected_style = Some(format!("{id}/{style}"));
        }
        self.state.status = crate::desktop::logged_status(
            "Style collection loaded alongside the other libraries.".into(),
        );
        self.sync_collections();
        Ok(())
    }

    pub(super) fn save_collection(&mut self, id: &str, path: &Path) -> Result<(), String> {
        if let Some(source) = self
            .collections
            .iter()
            .find(|item| item.summary.id == id && item.summary.read_only)
        {
            let mut summary = source.summary.clone();
            summary.id = new_id();
            summary.read_only = false;
            summary.dirty = true;
            let copy = Collection {
                summary,
                store: ProjectStore::new(),
                steam_sets: source.steam_sets.clone(),
                images: source.images.clone(),
            };
            let id = copy.summary.id.clone();
            self.collections.push(copy);
            if let Err(error) = self.save_collection(&id, path) {
                self.collections.pop();
                return Err(error);
            }
            self.state.active_collection = Some(id);
            return Ok(());
        }
        let collection = self
            .collections
            .iter_mut()
            .find(|item| item.summary.id == id)
            .ok_or("Collection unavailable")?;
        let mut project = self.editor.document().project().clone();
        project["displayName"] = json!(collection.summary.name);
        project["iconStyles"] = json!(collection.summary.styles);
        project["styleViewerSteamSets"] = json!(collection.steam_sets);
        project["styleViewerSelectedStyleId"] =
            json!(collection.summary.styles.first().map(|style| &style.id));
        let entity = project["achievements"]
            .as_array_mut()
            .and_then(|items| items.first_mut())
            .ok_or("Missing preview achievement")?;
        entity["generalImageEntries"] = json!(collection.images);
        entity["icon"] = collection
            .summary
            .styles
            .first()
            .map_or(Value::Null, |style| style.recipe.clone());
        let document = Self::export_document(project, self.editor.document().catalog().clone())?;
        collection.store.save(&document, path)?;
        collection.summary.name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Style collection")
            .into();
        collection.summary.dirty = false;
        self.sync_collections();
        self.state.status = crate::desktop::logged_status("Style collection saved.".into());
        Ok(())
    }

    pub(super) fn library_action(&mut self, action: &Action) -> Result<bool, String> {
        match action {
            Action::NewCollection => {
                let id = new_id();
                self.collections.push(Collection {
                    summary: StyleViewerCollection {
                        id: id.clone(),
                        name: "Untitled collection".into(),
                        styles: Vec::new(),
                        dirty: true,
                        read_only: false,
                        steam_sets: Vec::new(),
                    },
                    store: ProjectStore::new(),
                    steam_sets: Vec::new(),
                    images: Vec::new(),
                });
                self.state.active_collection = Some(id);
            }
            Action::SelectCollection(id) => {
                if id
                    .as_ref()
                    .is_some_and(|id| !self.collections.iter().any(|item| &item.summary.id == id))
                {
                    return Err("Collection unavailable".into());
                }
                self.state.active_collection = id.clone();
                return Ok(true);
            }
            Action::CopyToCollection { style, collection } => {
                let original = if let Some((source, id)) = style.split_once('/') {
                    self.collections
                        .iter()
                        .find(|item| item.summary.id == source)
                        .and_then(|item| item.summary.styles.iter().find(|style| style.id == id))
                } else {
                    self.state.styles.iter().find(|item| item.id == *style)
                }
                .cloned()
                .ok_or("Style unavailable")?;
                let target = self
                    .collections
                    .iter_mut()
                    .find(|item| item.summary.id == *collection && !item.summary.read_only)
                    .ok_or("Choose an editable collection")?;
                if target.summary.styles.len() >= 128 {
                    return Err(
                        "A collection supports up to 128 styles. Create another collection.".into(),
                    );
                }
                let mut copy = original;
                copy.id = new_id();
                target.summary.styles.push(copy);
                target.summary.dirty = true;
            }
            Action::UnloadCollection(id) => {
                let Some(item) = self.collections.iter().find(|item| item.summary.id == *id) else {
                    return Err("Collection unavailable".into());
                };
                if item.summary.read_only {
                    return Err("App presets remain available".into());
                }
                if item.summary.dirty {
                    return Err("Save this collection before unloading it.".into());
                }
                self.collections.retain(|item| item.summary.id != *id);
                self.prune_collection_artwork()?;
                if self.state.active_collection.as_ref() == Some(id) {
                    self.state.active_collection = None;
                }
                if self
                    .state
                    .selected_style
                    .as_ref()
                    .is_some_and(|style| style.starts_with(&format!("{id}/")))
                {
                    self.state.selected_style =
                        self.state.styles.first().map(|style| style.id.clone());
                }
            }
            Action::RenameStyle { id, name } if id.contains('/') => {
                let (collection, id) = id.split_once('/').ok_or("Invalid collection style")?;
                let collection = self
                    .collections
                    .iter_mut()
                    .find(|item| item.summary.id == collection && !item.summary.read_only)
                    .ok_or("Copy app presets to a collection before editing")?;
                let style = collection
                    .summary
                    .styles
                    .iter_mut()
                    .find(|style| style.id == id)
                    .ok_or("Style unavailable")?;
                let mut candidate = style.clone();
                candidate.name = name.trim().into();
                candidate.validate()?;
                *style = candidate;
                collection.summary.dirty = true;
            }
            Action::RemoveStyle(id) if id.contains('/') => {
                let (collection, style) = id.split_once('/').ok_or("Invalid collection style")?;
                let collection = self
                    .collections
                    .iter_mut()
                    .find(|item| item.summary.id == collection && !item.summary.read_only)
                    .ok_or("App presets cannot be removed")?;
                collection.summary.styles.retain(|item| item.id != style);
                collection.steam_sets.retain(|item| item.style_id != style);
                collection.summary.dirty = true;
                if self.state.selected_style.as_ref() == Some(id) {
                    self.state.selected_style = None;
                }
            }
            Action::LinkSet(link) if link.style_id.contains('/') => {
                let (collection, style) = link
                    .style_id
                    .split_once('/')
                    .ok_or("Invalid collection style")?;
                let collection = self
                    .collections
                    .iter_mut()
                    .find(|item| item.summary.id == collection && !item.summary.read_only)
                    .ok_or("Choose an editable collection")?;
                let mut link = link.clone();
                link.style_id = style.into();
                link.validate()?;
                if !collection
                    .summary
                    .styles
                    .iter()
                    .any(|item| item.id == style)
                {
                    return Err("Style unavailable".into());
                }
                if let Some(existing) = collection
                    .steam_sets
                    .iter_mut()
                    .find(|item| item.same_target(&link))
                {
                    if existing.style_id != link.style_id {
                        return Err(
                            "This Steam set is already linked to another style. Unlink it first."
                                .into(),
                        );
                    }
                    *existing = link;
                } else if collection.steam_sets.len() < 512 {
                    collection.steam_sets.push(link);
                } else {
                    return Err("A collection supports up to 512 Steam set links".into());
                }
                collection.summary.dirty = true;
            }
            Action::UnlinkSet(link) if link.style_id.contains('/') => {
                let (collection, style) = link
                    .style_id
                    .split_once('/')
                    .ok_or("Invalid collection style")?;
                let collection = self
                    .collections
                    .iter_mut()
                    .find(|item| item.summary.id == collection && !item.summary.read_only)
                    .ok_or("Choose an editable collection")?;
                let mut link = link.clone();
                link.style_id = style.into();
                collection.steam_sets.retain(|item| item != &link);
                collection.summary.dirty = true;
            }
            _ => return Ok(false),
        }
        self.sync_collections();
        Ok(true)
    }

    pub(super) fn prepare_render_document(&mut self) -> Result<(), String> {
        // Viewer input metadata deliberately does not invalidate canvas previews.
        // Its own renderer must still observe slot edits, including clearing a slot.
        let revision = self.editor.revision();
        if self
            .render_document
            .as_ref()
            .is_some_and(|(previous, library, _)| {
                *previous == revision && *library == self.library_revision
            })
        {
            return Ok(());
        }
        let mut project = self.editor.document().project().clone();
        project["styleViewerLibrary"] = json!({});
        project["styleViewerLibraryImages"] = json!({});
        project["styleViewerDefaultImages"] = json!((0..3).map(|id| json!({"id":id,"imageId":"c13dc47a-5afd-4bf3-a2bb-e5456020ac01","atlasId":null,"tileIndex":0})).collect::<Vec<_>>());
        for collection in &self.collections {
            project["styleViewerLibrary"][&collection.summary.id] =
                json!(collection.summary.styles);
            project["styleViewerLibraryImages"][&collection.summary.id] = json!(collection.images);
        }
        if let Some(blend) = &self.blend
            && let Some(style) = project["iconStyles"]
                .as_array_mut()
                .and_then(|styles| styles.iter_mut().find(|s| s["id"] == blend.achievement_id))
            && let Some(layer) = style["recipe"]["layers"]
                .as_array_mut()
                .and_then(|layers| layers.iter_mut().find(|l| l["id"] == blend.layer_id))
        {
            layer["blendMode"] = json!(blend.mode);
        }
        if let Some(style) = self.state.selected() {
            let recipe = project["iconStyles"]
                .as_array()
                .and_then(|styles| styles.iter().find(|s| s["id"] == style.id))
                .map_or(style.recipe, |s| s["recipe"].clone());
            let defaults = project["styleViewerDefaultImages"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if let Some(entity) = project["achievements"]
                .as_array_mut()
                .and_then(|a| a.first_mut())
            {
                entity["icon"] = recipe;
                self.state.preview.apply_images(entity, &defaults);
            }
        }
        self.render_document = Some((
            revision,
            self.library_revision,
            Document::from_parts(project, self.editor.document().catalog().clone())?,
        ));
        self.render_generation = self.render_generation.wrapping_add(1);
        Ok(())
    }

    fn prune_collection_artwork(&mut self) -> Result<(), String> {
        fn referenced(value: &Value, ids: &mut std::collections::BTreeSet<String>) {
            match value {
                Value::String(id) => {
                    ids.insert(id.clone());
                }
                Value::Array(values) => {
                    for value in values {
                        referenced(value, ids);
                    }
                }
                Value::Object(values) => {
                    for value in values.values() {
                        referenced(value, ids);
                    }
                }
                _ => {}
            }
        }
        let mut used = std::collections::BTreeSet::new();
        referenced(&json!(self.state.styles), &mut used);
        referenced(self.entity(), &mut used);
        referenced(&json!(self.state.preview), &mut used);
        for collection in &self.collections {
            referenced(&json!(collection.summary.styles), &mut used);
            referenced(&json!(collection.images), &mut used);
        }
        let mut assets = self.editor.document().project()["assets"].clone();
        for key in ["images", "atlases"] {
            if let Some(entries) = assets[key].as_array_mut() {
                entries.retain(|entry| {
                    entry["id"]
                        .as_str()
                        .is_none_or(|id| !self.collection_assets.contains(id) || used.contains(id))
                });
            }
        }
        self.editor.execute(
            "Unload collection artwork",
            Command::ProjectSettings {
                name: "Style set".into(),
                app_id: 0,
                atlas: None,
                assets: Some(assets),
            },
            Some("viewer-library-artwork"),
        )?;
        self.collection_assets.retain(|id| used.contains(id));
        Ok(())
    }

    pub(super) fn export_document(
        mut project: Value,
        catalog: achimeld_application::Catalog,
    ) -> Result<Document, String> {
        fn references(
            value: &Value,
            ids: &mut std::collections::BTreeSet<String>,
            default_atlas: &mut bool,
        ) {
            match value {
                Value::Object(object) => {
                    if object.get("kind").is_some_and(|kind| kind == "AtlasTile")
                        && object.get("atlasId").is_none_or(Value::is_null)
                    {
                        *default_atlas = true;
                    }
                    for (key, value) in object {
                        if matches!(
                            key.as_str(),
                            "imageId" | "atlasId" | "generalImageId" | "generalAtlasId"
                        ) {
                            if let Some(id) = value.as_str() {
                                ids.insert(id.into());
                                if id == "00000000-0000-0000-0000-000000000000" {
                                    *default_atlas = true;
                                }
                            }
                        } else {
                            references(value, ids, default_atlas);
                        }
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        references(value, ids, default_atlas);
                    }
                }
                _ => {}
            }
        }
        let mut used = std::collections::BTreeSet::new();
        let mut default_atlas = false;
        for key in ["iconStyles", "achievements"] {
            references(&project[key], &mut used, &mut default_atlas);
        }
        let preview = StyleViewerPreview::from_project(&project);
        for source in std::iter::once(preview.image.as_ref())
            .chain(preview.slots.iter().flatten().map(Option::as_ref))
            .flatten()
        {
            match source {
                StyleViewerSource::Image { id } | StyleViewerSource::Atlas { id: Some(id), .. } => {
                    used.insert(id.clone());
                }
                StyleViewerSource::Atlas { id: None, .. } => default_atlas = true,
            }
        }
        for key in ["images", "atlases"] {
            if let Some(entries) = project["assets"][key].as_array_mut() {
                entries.retain(|entry| entry["id"].as_str().is_some_and(|id| used.contains(id)));
            }
        }
        if !default_atlas {
            project["assets"]["atlas"] = Value::Null;
        }
        Document::from_parts(project, catalog)
    }
}
