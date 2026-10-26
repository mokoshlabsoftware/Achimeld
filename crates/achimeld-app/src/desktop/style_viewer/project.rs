use super::*;
use achimeld_application::{IconStyle, style_management};

const JSON_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StyleFile {
    version: u32,
    styles: Vec<IconStyle>,
}

fn validate_json_style(style: &IconStyle, document: Option<&Document>) -> Result<(), String> {
    style.validate()?;
    fn inspect(value: &Value, document: Option<&Document>) -> Result<(), String> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "path" | "filePath") && !value.is_null() {
                        return Err("Style JSON cannot contain filesystem paths".into());
                    }
                    if let Some(id) = value.as_str()
                        && let Some(document) = document
                    {
                        let exists = match key.as_str() {
                            "imageId" => Some(
                                document.project()["assets"]["images"]
                                    .as_array()
                                    .is_some_and(|a| a.iter().any(|e| e["id"] == id)),
                            ),
                            "atlasId" if id != "00000000-0000-0000-0000-000000000000" => Some(
                                document.project()["assets"]["atlases"]
                                    .as_array()
                                    .is_some_and(|a| a.iter().any(|e| e["id"] == id)),
                            ),
                            _ => None,
                        };
                        if exists == Some(false) {
                            return Err("A style uses artwork absent from this project's Art Data Bank. Use General image IDs for portable styles.".into());
                        }
                    }
                    inspect(value, document)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    inspect(value, document)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    inspect(&style.recipe, document)
}

fn copies(styles: Vec<IconStyle>) -> Result<Vec<IconStyle>, String> {
    styles
        .into_iter()
        .map(|mut style| {
            style.validate()?;
            style.id = new_id();
            style.recipe = achimeld_application::style_collections::clone_recipe(&style.recipe)?;
            style.preview_png = None;
            Ok(style)
        })
        .collect()
}

impl Host {
    pub(super) fn viewer_project_action(&mut self, action: &Action) -> bool {
        let rename = matches!(action, Action::RenameStyle { id, .. } if !id.contains('/'));
        if !rename
            && !matches!(
                action,
                Action::DuplicateStyle(_)
                    | Action::CloneBuiltins
                    | Action::DeleteProjectStyle { .. }
                    | Action::LoadJson
                    | Action::SaveJson
            )
        {
            return false;
        }
        if self.dialog.is_some() {
            return true;
        }
        let result = (|| -> Result<(), String> {
            self.ensure_style_viewer()?;
            match action {
                Action::LoadJson => self.start_dialog(Dialog::StylesJsonLoad),
                Action::SaveJson => {
                    let styles = self.editor.document().icon_styles();
                    for style in &styles {
                        validate_json_style(style, None)?;
                    }
                    self.start_dialog(Dialog::StylesJsonSave(styles));
                }
                Action::RenameStyle { id, name } => {
                    let mut style = self
                        .editor
                        .document()
                        .icon_styles()
                        .into_iter()
                        .find(|s| &s.id == id)
                        .ok_or("Project style no longer exists")?;
                    style.name = name.trim().into();
                    self.finish_group();
                    self.execute("Rename project style", Command::SaveIconStyle(style), None)?;
                }
                Action::DeleteProjectStyle { id, replacement } => {
                    let command = style_management::remove(
                        self.editor.document(),
                        id,
                        replacement.as_deref(),
                    )?;
                    self.finish_group();
                    self.execute("Delete project style", command, None)?;
                }
                Action::DuplicateStyle(id) => {
                    let viewer = self
                        .style_viewer
                        .as_ref()
                        .ok_or("Style Viewer unavailable")?;
                    let source = if let Some((collection, id)) = id.split_once('/') {
                        viewer.collection_style(collection, id)?
                    } else {
                        self.editor
                            .document()
                            .icon_styles()
                            .into_iter()
                            .find(|s| &s.id == id)
                            .ok_or("Project style no longer exists")?
                    };
                    let mut style = copies(vec![source])?.remove(0);
                    let names = self.editor.document().icon_styles();
                    let base = style.name.clone();
                    style.name = (1..)
                        .map(|n| format!("{base} copy {n}"))
                        .find(|name| !names.iter().any(|s| &s.name == name))
                        .ok_or("Cannot name style")?;
                    let id = style.id.clone();
                    self.finish_group();
                    self.execute(
                        "Duplicate style into project",
                        Command::SaveIconStyle(style),
                        None,
                    )?;
                    self.sync_viewer_preview();
                    self.style_viewer
                        .as_mut()
                        .ok_or("Style Viewer unavailable")?
                        .select(&id)?;
                }
                Action::CloneBuiltins => {
                    let viewer = self
                        .style_viewer
                        .as_ref()
                        .ok_or("Style Viewer unavailable")?;
                    let sources = viewer
                        .state
                        .collections
                        .iter()
                        .filter(|c| c.read_only && c.name == "Built-in")
                        .flat_map(|c| c.styles.clone())
                        .collect();
                    let styles = copies(sources)?;
                    let first = styles.first().map(|s| s.id.clone());
                    self.finish_group();
                    self.execute(
                        "Clone all built-in styles",
                        Command::Batch(styles.into_iter().map(Command::SaveIconStyle).collect()),
                        None,
                    )?;
                    self.sync_viewer_preview();
                    if let Some(id) = first {
                        self.style_viewer
                            .as_mut()
                            .ok_or("Style Viewer unavailable")?
                            .select(&id)?;
                    }
                }
                _ => unreachable!(),
            }
            self.sync_viewer_preview();
            Ok(())
        })();
        if let Err(error) = result {
            self.status = crate::desktop::logged_status(error.clone());
            if let Some(viewer) = &mut self.style_viewer {
                viewer.state.status = error;
            }
        }
        true
    }

    pub(in crate::desktop) fn load_styles_json(&mut self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > JSON_LIMIT {
            return Err("Style file exceeds 16 MiB".into());
        }
        let mut bytes = Vec::new();
        file.take(JSON_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > JSON_LIMIT {
            return Err("Style file exceeds 16 MiB".into());
        }
        let data: StyleFile =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid style JSON file")?;
        if data.version != 1 || data.styles.len() > 128 {
            return Err("Unsupported style file version or more than 128 styles".into());
        }
        for style in &data.styles {
            validate_json_style(style, Some(self.editor.document()))?;
        }
        let styles = copies(data.styles)?;
        let first = styles.first().map(|s| s.id.clone());
        self.finish_group();
        self.execute(
            "Load project styles from JSON",
            Command::Batch(styles.into_iter().map(Command::SaveIconStyle).collect()),
            None,
        )?;
        self.ensure_style_viewer()?;
        if let Some(id) = first {
            self.style_viewer
                .as_mut()
                .ok_or("Style Viewer unavailable")?
                .select(&id)?;
        }
        Ok(())
    }
}

pub(in crate::desktop) fn save_styles_json(
    styles: Vec<IconStyle>,
    path: &Path,
) -> Result<(), String> {
    use std::io::Write as _;
    if styles.len() > 128 {
        return Err("A style file supports up to 128 styles".into());
    }
    for style in &styles {
        validate_json_style(style, None)?;
    }
    let bytes =
        serde_json::to_vec_pretty(&StyleFile { version: 1, styles }).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > JSON_LIMIT {
        return Err("Style file exceeds 16 MiB".into());
    }
    let temporary = path
        .parent()
        .ok_or("Choose an export folder")?
        .join(format!(".achimeld-styles-{}.tmp", new_id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result
        .and_then(|()| std::fs::hard_link(&temporary, path))
        .map_err(|e| format!("Could not save styles; choose an unused filename: {e}"));
    let _ = std::fs::remove_file(temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_tracks_current_project_styles_through_add_edit_undo_and_project_switch() {
        let mut host = Host::new(egui::Context::default());
        host.viewer_action(Action::Open);
        assert!(host.style_viewer.as_ref().unwrap().state.styles.is_empty());
        host.editor
            .execute(
                "Add style",
                Command::AddStyleAchievementAt { position: [0, 0] },
                None,
            )
            .unwrap();
        host.poll_viewer();
        let first = host.editor.document().icon_styles()[0].clone();
        assert_eq!(
            host.style_viewer.as_ref().unwrap().state.styles,
            vec![first.clone()]
        );
        host.viewer_action(Action::SelectStyle(first.id.clone()));
        host.viewer_action(Action::RenameStyle {
            id: first.id.clone(),
            name: "Changed".into(),
        });
        assert_eq!(host.editor.document().icon_styles()[0].name, "Changed");
        host.editor.undo();
        host.poll_viewer();
        assert_eq!(
            host.style_viewer.as_ref().unwrap().state.styles[0].name,
            first.name
        );
        host.editor = Editor::default();
        host.poll_viewer();
        assert!(host.style_viewer.as_ref().unwrap().state.styles.is_empty());
        assert!(
            host.style_viewer
                .as_ref()
                .unwrap()
                .state
                .selected_style
                .is_none()
        );
    }

    #[test]
    fn duplicate_and_clone_all_create_editable_project_entries_and_leave_builtins_unchanged() {
        let mut host = Host::new(egui::Context::default());
        host.viewer_action(Action::Open);
        let builtin = host
            .style_viewer
            .as_ref()
            .unwrap()
            .state
            .collections
            .clone();
        assert!(builtin.iter().all(|c| c.name == "Built-in" && c.read_only));
        for style in builtin.iter().flat_map(|c| &c.styles) {
            assert!(!style.name.to_lowercase().contains("mosaica"));
            assert!(!style.name.to_lowercase().contains("waifus"));
            assert!(
                style.recipe["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|l| l["kind"] != "Artifact3d")
            );
        }
        let id = format!("{}/{}", builtin[0].id, builtin[0].styles[0].id);
        host.viewer_action(Action::DuplicateStyle(id.clone()));
        let project = host.editor.document().icon_styles();
        assert_eq!(project.len(), 1);
        assert_ne!(project[0].id, builtin[0].styles[0].id);
        assert_eq!(
            achimeld_application::style_signature(&project[0].recipe),
            achimeld_application::style_signature(&builtin[0].styles[0].recipe)
        );
        host.viewer_action(Action::RenameStyle {
            id: id.clone(),
            name: "Cannot edit".into(),
        });
        host.viewer_action(Action::DeleteProjectStyle {
            id,
            replacement: None,
        });
        assert_eq!(host.editor.document().icon_styles(), project);
        let before = host.editor.document().clone();
        host.viewer_action(Action::CloneBuiltins);
        assert_eq!(
            host.editor.document().icon_styles().len(),
            1 + builtin.iter().map(|c| c.styles.len()).sum::<usize>()
        );
        host.editor.undo();
        assert_eq!(host.editor.document(), &before);
        for (before, after) in builtin
            .iter()
            .zip(&host.style_viewer.as_ref().unwrap().state.collections)
        {
            assert_eq!(before.styles, after.styles);
        }
    }

    #[test]
    fn json_roundtrip_is_additive_undoable_and_rejects_invalid_files_without_partial_import() {
        let mut host = Host::new(egui::Context::default());
        host.viewer_action(Action::Open);
        host.viewer_action(Action::CloneBuiltins);
        let before = host.editor.document().clone();
        let folder = crate::test_output::root().join(format!("style-json-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("styles.json");
        save_styles_json(before.icon_styles(), &path).unwrap();
        assert!(save_styles_json(before.icon_styles(), &path).is_err());
        host.load_styles_json(&path).unwrap();
        assert_eq!(
            host.editor.document().icon_styles().len(),
            before.icon_styles().len() * 2
        );
        host.editor.undo();
        assert_eq!(host.editor.document(), &before);
        let invalid = folder.join("invalid.json");
        std::fs::write(&invalid, br#"{"version":2,"styles":[]}"#).unwrap();
        assert!(host.load_styles_json(&invalid).is_err());
        assert_eq!(host.editor.document(), &before);
        let mut data = StyleFile {
            version: 1,
            styles: before.icon_styles(),
        };
        data.styles.last_mut().unwrap().recipe["layers"][0]["imageId"] = json!(new_id());
        std::fs::write(&invalid, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(host.load_styles_json(&invalid).is_err());
        assert_eq!(host.editor.document(), &before);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
