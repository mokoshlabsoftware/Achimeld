use crate::document::Result;
use crate::{Command, Document, Preferences};

struct Entry {
    delta: crate::delta::Delta,
    before_preferences: Preferences,
    after_preferences: Preferences,
    label: String,
    group: Option<String>,
    bytes: usize,
}

/// All document mutation passes through this owner, including live edits and compound actions.
pub struct Editor {
    document: Document,
    saved: Document,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    revision: u64,
    preview_revision: u64,
    bytes: usize,
    preferences: Preferences,
}
impl Default for Editor {
    fn default() -> Self {
        Self::new(Document::new())
    }
}
impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            saved: document.clone(),
            document,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            preview_revision: 0,
            bytes: 0,
            preferences: Preferences::default(),
        }
    }
    pub fn with_preferences(mut self, preferences: Preferences) -> Self {
        self.preferences = preferences;
        self
    }
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Changes to rendered content, independent of canvas navigation and placement.
    pub fn preview_revision(&self) -> u64 {
        self.preview_revision
    }
    pub fn is_dirty(&self) -> bool {
        self.document != self.saved
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.document.clone();
        self.finish_group();
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }
    pub fn finish_group(&mut self) {
        if let Some(e) = self.undo.last_mut() {
            e.group = None;
        }
    }
    pub fn execute(&mut self, label: &str, command: Command, group: Option<&str>) -> Result<()> {
        self.execute_with(label, command, group, |_, _| Ok(()))
    }
    pub fn execute_with(
        &mut self,
        label: &str,
        command: Command,
        group: Option<&str>,
        before_commit: impl FnOnce(&Preferences, &Preferences) -> Result<()>,
    ) -> Result<()> {
        if let Command::CanvasCamera { kind, .. } = &command {
            let key = match kind {
                crate::EntityKind::Achievement => "achievementsCamera",
                crate::EntityKind::Stat => "statsCamera",
            };
            // Mixed command groups keep the general transactional path. Only a
            // single validated camera field can use this bounded fast path.
            if !self.coalescing(group)
                || self
                    .undo
                    .last()
                    .is_some_and(|e| e.delta.camera_before(key).is_some())
            {
                return self.execute_camera(label, &command, group, before_commit);
            }
        }
        let mut after = self.document.clone();
        let before_preferences = self.preferences.clone();
        let mut after_preferences = before_preferences.clone();
        apply(&mut after, &mut after_preferences, &command, 0)?;
        crate::style_achievements::reconcile_styles(&self.document, &mut after)?;
        after.validate()?;
        after_preferences.validate()?;
        if self.document == after && before_preferences == after_preferences {
            return Ok(());
        }
        let coalescing = group.is_some()
            && self.redo.is_empty()
            && self
                .undo
                .last()
                .is_some_and(|e| e.group.as_deref() == group);
        let original = if coalescing {
            let mut original = self.document.clone();
            if let Some(entry) = self.undo.last() {
                entry.delta.apply(&mut original, false)?;
            }
            Some(original)
        } else {
            None
        };
        let delta =
            crate::delta::Delta::between(original.as_ref().unwrap_or(&self.document), &after)?;
        let affects_previews = if coalescing {
            crate::delta::Delta::between(&self.document, &after)?.affects_previews()
        } else {
            delta.affects_previews()
        };
        let initial_preferences = if coalescing {
            self.undo
                .last()
                .map(|e| &e.before_preferences)
                .unwrap_or(&before_preferences)
        } else {
            &before_preferences
        };
        let bytes = serde_json::to_vec(&(&delta, initial_preferences, &after_preferences))
            .map_err(|_| "Could not measure history")?
            .len();
        if bytes > 64 * 1024 * 1024 {
            return Err("Edit exceeds the history memory budget".into());
        }
        before_commit(&before_preferences, &after_preferences)?;
        if let Some(entry) = self
            .undo
            .last_mut()
            .filter(|e| group.is_some() && e.group.as_deref() == group && self.redo.is_empty())
        {
            self.bytes = self.bytes - entry.bytes + bytes;
            entry.delta = delta;
            entry.after_preferences = after_preferences.clone();
            entry.bytes = bytes;
        } else {
            self.undo.push(Entry {
                delta,
                before_preferences,
                after_preferences: after_preferences.clone(),
                label: label.into(),
                group: group.map(str::to_owned),
                bytes,
            });
            self.bytes += bytes;
        }
        self.redo.clear();
        self.document = after;
        self.preferences = after_preferences;
        self.revision += 1;
        self.preview_revision += u64::from(affects_previews);
        while self.undo.len() > 128 || self.bytes > 64 * 1024 * 1024 {
            self.bytes -= self.undo.remove(0).bytes;
        }
        Ok(())
    }
    fn coalescing(&self, group: Option<&str>) -> bool {
        group.is_some()
            && self.redo.is_empty()
            && self
                .undo
                .last()
                .is_some_and(|entry| entry.group.as_deref() == group)
    }
    fn execute_camera(
        &mut self,
        label: &str,
        command: &Command,
        group: Option<&str>,
        before_commit: impl FnOnce(&Preferences, &Preferences) -> Result<()>,
    ) -> Result<()> {
        let Command::CanvasCamera {
            kind,
            zoom,
            offset_x,
            offset_y,
        } = command
        else {
            return Err("Expected canvas camera command".into());
        };
        let key = match kind {
            crate::EntityKind::Achievement => "achievementsCamera",
            crate::EntityKind::Stat => "statsCamera",
        };
        // Camera validation is shared with Document::apply. These values affect
        // no other document invariant and cannot change rendered artwork.
        let value = crate::document::camera_value(*zoom, *offset_x, *offset_y)?;
        self.preferences.validate()?;
        let current = self.document.project().get(key);
        if current == Some(&value) {
            return Ok(());
        }
        let coalescing = self.coalescing(group);
        let before = if coalescing {
            self.undo
                .last()
                .and_then(|e| e.delta.camera_before(key))
                .cloned()
                .unwrap_or_default()
        } else {
            current.cloned()
        };
        let delta = crate::delta::Delta::camera(key, before, value);
        let initial_preferences = self
            .undo
            .last()
            .filter(|_| coalescing)
            .map(|entry| &entry.before_preferences)
            .unwrap_or(&self.preferences);
        let bytes = serde_json::to_vec(&(&delta, initial_preferences, &self.preferences))
            .map_err(|_| "Could not measure history")?
            .len();
        if bytes > 64 * 1024 * 1024 {
            return Err("Edit exceeds the history memory budget".into());
        }
        before_commit(&self.preferences, &self.preferences)?;
        self.document.apply(command, 0)?;
        if let Some(entry) = self.undo.last_mut().filter(|_| coalescing) {
            self.bytes = self.bytes - entry.bytes + bytes;
            entry.delta = delta;
            entry.after_preferences = self.preferences.clone();
            entry.bytes = bytes;
        } else {
            self.undo.push(Entry {
                delta,
                before_preferences: self.preferences.clone(),
                after_preferences: self.preferences.clone(),
                label: label.into(),
                group: group.map(str::to_owned),
                bytes,
            });
            self.bytes += bytes;
        }
        self.redo.clear();
        self.revision += 1;
        while self.undo.len() > 128 || self.bytes > 64 * 1024 * 1024 {
            self.bytes -= self.undo.remove(0).bytes;
        }
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        self.undo_with(|_, _| Ok(())).unwrap_or(false)
    }
    pub fn undo_with(
        &mut self,
        before_commit: impl FnOnce(&Preferences, &Preferences) -> Result<()>,
    ) -> Result<bool> {
        let mut restored = self.document.clone();
        if let Some(entry) = self.undo.last() {
            entry.delta.apply(&mut restored, false)?;
            before_commit(&self.preferences, &entry.before_preferences)?;
        }
        self.finish_group();
        if let Some(entry) = self.undo.pop() {
            self.preview_revision += u64::from(entry.delta.affects_previews());
            self.bytes -= entry.bytes;
            self.document = restored;
            self.preferences = entry.before_preferences.clone();
            self.redo.push(entry);
            self.revision += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn redo(&mut self) -> bool {
        self.redo_with(|_, _| Ok(())).unwrap_or(false)
    }
    pub fn redo_with(
        &mut self,
        before_commit: impl FnOnce(&Preferences, &Preferences) -> Result<()>,
    ) -> Result<bool> {
        let mut restored = self.document.clone();
        if let Some(entry) = self.redo.last() {
            entry.delta.apply(&mut restored, true)?;
            before_commit(&self.preferences, &entry.after_preferences)?;
        }
        if let Some(mut entry) = self.redo.pop() {
            self.preview_revision += u64::from(entry.delta.affects_previews());
            entry.group = None;
            self.document = restored;
            self.preferences = entry.after_preferences.clone();
            self.bytes += entry.bytes;
            self.undo.push(entry);
            self.revision += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn apply(
    document: &mut Document,
    preferences: &mut Preferences,
    command: &Command,
    depth: u8,
) -> Result<()> {
    if depth > 16 {
        return Err("Command transaction is too deeply nested".into());
    }
    match command {
        Command::Batch(commands) => {
            for c in commands {
                apply(document, preferences, c, depth + 1)?;
            }
            Ok(())
        }
        Command::Preferences(value) => {
            value.validate()?;
            *preferences = value.clone();
            Ok(())
        }
        Command::Add(crate::EntityKind::Achievement) => document.add_named(preferences, None),
        Command::AddAt {
            kind: crate::EntityKind::Achievement,
            position,
        } => document.add_named(preferences, Some(*position)),
        _ => document.apply(command, depth),
    }
}

#[cfg(test)]
mod camera_regressions {
    use super::*;
    use crate::EntityKind;
    fn camera(kind: EntityKind, zoom: f64, offset_x: f64) -> Command {
        Command::CanvasCamera {
            kind,
            zoom,
            offset_x,
            offset_y: -20.0,
        }
    }
    #[test]
    fn camera_history_coalesces_preserves_save_state_and_restores_absent_fields() {
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            let mut editor = Editor::default();
            let original = editor.document().clone();
            for i in 0..120 {
                editor
                    .execute("Camera", camera(kind, 0.2, f64::from(i)), Some("camera"))
                    .unwrap();
            }
            assert_eq!(editor.undo.len(), 1);
            assert_eq!(editor.preview_revision(), 0);
            assert!(editor.is_dirty());
            editor.document().validate().unwrap();
            let moved = editor.document().clone();
            assert!(editor.undo());
            assert_eq!(editor.document(), &original);
            assert!(!editor.is_dirty());
            assert!(editor.redo());
            assert_eq!(editor.document(), &moved);
            editor.mark_saved();
            editor
                .execute("Camera", camera(kind, 0.5, 9.0), Some("camera"))
                .unwrap();
            assert!(editor.is_dirty());
            assert!(editor.undo());
            assert!(!editor.is_dirty());
            assert_eq!(editor.document(), &moved);
        }
    }
    #[test]
    fn camera_validation_and_rejected_persistence_leave_document_and_history_intact() {
        let mut editor = Editor::default();
        editor
            .execute(
                "Camera",
                camera(EntityKind::Achievement, 0.2, 1.0),
                Some("camera"),
            )
            .unwrap();
        let before = editor.document().clone();
        let revision = editor.revision();
        let bytes = editor.bytes;
        for command in [
            camera(EntityKind::Achievement, f64::NAN, 1.0),
            camera(EntityKind::Achievement, 5.0, 1.0),
            camera(EntityKind::Achievement, 1.0, f64::INFINITY),
            camera(EntityKind::Achievement, 1.0, f64::MAX),
        ] {
            assert!(
                editor
                    .execute_with("Invalid", command, Some("camera"), |_, _| panic!(
                        "Invalid camera reached persistence"
                    ))
                    .is_err()
            );
        }
        assert!(
            editor
                .execute_with(
                    "Rejected",
                    camera(EntityKind::Achievement, 1.0, 4.0),
                    Some("camera"),
                    |_, _| Err("Rejected persistence".into())
                )
                .is_err()
        );
        assert_eq!(editor.document(), &before);
        assert_eq!(editor.revision(), revision);
        assert_eq!(editor.bytes, bytes);
        assert_eq!(editor.undo.len(), 1);
        assert!(editor.undo());
        assert!(!editor.is_dirty());
    }
    #[test]
    fn mixed_groups_and_different_camera_kinds_remain_one_atomic_history_entry() {
        for camera_first in [false, true] {
            let mut editor = Editor::default();
            let original = editor.document().clone();
            let mut commands = vec![
                camera(EntityKind::Achievement, 0.4, 2.0),
                Command::Add(EntityKind::Achievement),
            ];
            if !camera_first {
                commands.reverse();
            }
            commands.push(camera(EntityKind::Stat, 0.6, 8.0));
            commands.push(camera(EntityKind::Achievement, 0.8, 3.0));
            for command in commands {
                editor.execute("Mixed", command, Some("mixed")).unwrap();
            }
            let after = editor.document().clone();
            assert_eq!(editor.undo.len(), 1);
            assert_eq!(editor.preview_revision(), 1);
            assert!(editor.undo());
            assert_eq!(editor.document(), &original);
            assert!(editor.redo());
            assert_eq!(editor.document(), &after);
        }
    }
    #[test]
    fn coalesced_camera_retains_preference_history_and_its_measured_budget() {
        let mut editor = Editor::default();
        let original = editor.document().clone();
        let preferences = editor.preferences().clone();
        editor
            .execute(
                "Camera",
                camera(EntityKind::Achievement, 0.2, 1.0),
                Some("mixed"),
            )
            .unwrap();
        let mut changed = preferences.clone();
        changed.language = "Polish".into();
        editor
            .execute(
                "Preferences",
                Command::Preferences(changed.clone()),
                Some("mixed"),
            )
            .unwrap();
        editor
            .execute(
                "Camera",
                camera(EntityKind::Achievement, 0.4, 2.0),
                Some("mixed"),
            )
            .unwrap();
        let entry = &editor.undo[0];
        assert_eq!(
            editor.bytes,
            serde_json::to_vec(&(&entry.delta, &preferences, &changed))
                .unwrap()
                .len()
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &original);
        assert_eq!(editor.preferences(), &preferences);
        assert!(editor.redo());
        assert_eq!(editor.preferences(), &changed);
    }
    #[test]
    fn camera_fast_path_validates_initial_preferences_before_persistence() {
        let preferences = Preferences {
            normal_size: f32::NAN,
            ..Preferences::default()
        };
        let mut editor = Editor::default().with_preferences(preferences);
        let before = editor.document().clone();
        assert!(
            editor
                .execute_with(
                    "Camera",
                    camera(EntityKind::Achievement, 0.2, 1.0),
                    None,
                    |_, _| panic!("Invalid preferences reached persistence")
                )
                .is_err()
        );
        assert_eq!(editor.document(), &before);
        assert_eq!(editor.revision(), 0);
        assert!(editor.undo.is_empty());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mixed_coalesced_edits_invalidate_artwork_but_navigation_alone_does_not() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let start = editor.preview_revision();
        entity["icon"] = serde_json::json!({"layers":[{"id":crate::new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF"}]});
        editor
            .execute(
                "Artwork",
                Command::Batch(vec![
                    Command::CanvasCamera {
                        kind: EntityKind::Achievement,
                        zoom: 2.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                    },
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.clone(),
                        value: entity,
                    },
                ]),
                Some("mixed"),
            )
            .unwrap();
        assert_eq!(editor.preview_revision(), start + 1);
        editor
            .execute(
                "Move",
                Command::Move {
                    kind: EntityKind::Achievement,
                    id,
                    column: 2,
                    row: 3,
                },
                Some("mixed"),
            )
            .unwrap();
        assert_eq!(editor.preview_revision(), start + 1);
        editor.finish_group();
        assert!(editor.undo());
        assert_eq!(editor.preview_revision(), start + 2);
        assert!(editor.redo());
        assert_eq!(editor.preview_revision(), start + 3);
    }

    #[test]
    fn excessive_curve_points_cannot_enter_the_renderer() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        entity["icon"] = serde_json::json!({"layers":[{"id":crate::new_id(),"kind":"Curves","curves":{"points":vec![serde_json::json!({"input":0.5,"output":0.5});257]}}]});
        assert!(
            editor
                .execute(
                    "Curve",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id,
                        value: entity
                    },
                    None
                )
                .is_err()
        );
    }
    #[test]
    fn malformed_entity_records_are_rejected_before_legacy_migration() {
        let document = Document::new();
        for record in [
            serde_json::json!(false),
            serde_json::json!(null),
            serde_json::json!([]),
        ] {
            let mut project = document.project().clone();
            project["achievements"] = serde_json::json!([record]);
            assert!(Document::from_parts(project, document.catalog().clone()).is_err());
        }
    }
    #[test]
    fn legacy_identity_and_missing_internal_ids_migrate_like_the_reference() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut project = editor.document().project().clone();
        let id = project["projectId"].clone();
        project["schemaVersion"] = serde_json::json!(0);
        project["id"] = serde_json::json!(crate::new_id());
        project
            .as_object_mut()
            .unwrap()
            .remove("nextAchievementInternalId");
        project["achievements"][0]
            .as_object_mut()
            .unwrap()
            .remove("internalId");
        let document = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        assert_eq!(document.project()["projectId"], id);
        assert_eq!(document.project()["achievements"][0]["internalId"], 0);
        assert_eq!(document.project()["nextAchievementInternalId"], 1);
    }
    #[test]
    fn mixed_folder_and_mask_cycles_cannot_commit() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let original = editor.document().clone();
        let mut entity = original.entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let folder = crate::new_id();
        entity["icon"] = serde_json::json!({"layers":[{"id":folder,"kind":"Folder"},{"id":crate::new_id(),"kind":"Solid","parentFolderId":folder,"maskLayerId":folder}]});
        assert!(
            editor
                .execute(
                    "Cycle",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id,
                        value: entity
                    },
                    None
                )
                .is_err()
        );
        assert_eq!(editor.document(), &original);
    }
    #[test]
    fn small_edits_keep_history_without_copying_large_opaque_content() {
        let original = Document::new();
        let mut project = original.project().clone();
        project["futureMetadata"] = serde_json::json!("x".repeat(2 * 1024 * 1024));
        let original = Document::from_parts(project, original.catalog().clone()).unwrap();
        let mut editor = Editor::new(original.clone());
        for n in 0..40 {
            editor
                .execute(
                    "Rename",
                    Command::ProjectSettings {
                        name: format!("Project {n}"),
                        app_id: 0,
                        atlas: None,
                        assets: None,
                    },
                    None,
                )
                .unwrap();
        }
        assert_eq!(editor.undo.len(), 40);
        assert!(editor.bytes < 64 * 1024);
        for _ in 0..40 {
            assert!(editor.undo());
        }
        assert_eq!(editor.document(), &original);
        for _ in 0..40 {
            assert!(editor.redo());
        }
        assert_eq!(editor.document().project()["displayName"], "Project 39");
    }
    use super::*;
    use crate::EntityKind;
    use serde_json::json;
    #[test]
    fn malformed_atlas_or_counter_is_rejected_before_editor_commands() {
        let document = Document::new();
        for change in [
            json!({"atlas":true}),
            json!({"nextAchievementInternalId":u64::MAX}),
        ] {
            let mut project = document.project().clone();
            for (key, value) in change.as_object().unwrap() {
                project[key] = value.clone();
            }
            assert!(Document::from_parts(project, document.catalog().clone()).is_err());
        }
        let mut project = document.project().clone();
        project["assets"].as_object_mut().unwrap().remove("images");
        project["assets"].as_object_mut().unwrap().remove("atlases");
        let compatible = Document::from_parts(project, document.catalog().clone()).unwrap();
        assert_eq!(compatible.project()["assets"]["images"], json!([]));
    }
    #[test]
    fn failed_preference_storage_leaves_document_and_history_unchanged() {
        let mut editor = Editor::default();
        let original = editor.preferences().clone();
        let mut changed = original.clone();
        changed.control_radius = 12;
        assert!(
            editor
                .execute_with(
                    "Preferences",
                    Command::Preferences(changed.clone()),
                    None,
                    |_, _| Err("Disk full".into())
                )
                .is_err()
        );
        assert_eq!(editor.preferences(), &original);
        assert_eq!(editor.revision(), 0);
        assert!(editor.undo_label().is_none());
        editor
            .execute("Preferences", Command::Preferences(changed.clone()), None)
            .unwrap();
        assert!(editor.undo_with(|_, _| Err("Disk full".into())).is_err());
        assert_eq!(editor.preferences(), &changed);
        assert_eq!(editor.undo_label(), Some("Preferences"));
        assert!(editor.redo_label().is_none());
    }
    #[test]
    fn naming_duplication_translation_and_layer_reorder_are_undoable() {
        let mut editor = Editor::default();
        let mut preferences = editor.preferences().clone();
        preferences.naming_convention = "WIN_{0:0000}".into();
        editor
            .execute("Preferences", Command::Preferences(preferences), None)
            .unwrap();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let source = editor.document().entities(EntityKind::Achievement)[0].clone();
        assert_eq!(source["apiName"], "WIN_0001");
        let id = source["id"].as_str().unwrap().to_owned();
        let loc = source["descriptionLocalizationId"]
            .as_str()
            .unwrap()
            .to_owned();
        editor
            .execute(
                "Translate",
                Command::SetLocalization {
                    id: loc,
                    language: "Polish".into(),
                    text: "Opis".into(),
                },
                None,
            )
            .unwrap();
        let mut recipe = source;
        let folder = crate::new_id();
        let layer = crate::new_id();
        recipe["icon"] = json!({"layers":[{"id":folder,"kind":"Folder"},{"id":layer,"kind":"Solid","parentFolderId":folder}]});
        editor
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: recipe,
                },
                None,
            )
            .unwrap();
        let before = editor.document().clone();
        editor
            .execute(
                "Duplicate",
                Command::Duplicate {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                },
                None,
            )
            .unwrap();
        let copied = &editor.document().entities(EntityKind::Achievement)[1];
        assert_eq!(copied["canvasRow"], 6);
        assert_eq!(
            editor.document().catalog().text(
                copied["descriptionLocalizationId"].as_str().unwrap(),
                "Polish"
            ),
            "Opis"
        );
        assert_ne!(copied["icon"]["layers"][0]["id"], folder);
        assert_eq!(
            copied["icon"]["layers"][1]["parentFolderId"],
            copied["icon"]["layers"][0]["id"]
        );
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(
            editor
                .execute(
                    "Bad reorder",
                    Command::ReorderLayers {
                        achievement_id: id.clone(),
                        order: vec![folder.clone(), folder.clone()]
                    },
                    None
                )
                .is_err()
        );
        assert_eq!(editor.document(), &before);
        editor
            .execute(
                "Reorder icon layers",
                Command::ReorderLayers {
                    achievement_id: id,
                    order: vec![layer, folder],
                },
                None,
            )
            .unwrap();
        assert_eq!(editor.undo_label(), Some("Reorder icon layers"));
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
    }
    #[test]
    fn failed_compound_edit_is_atomic_and_does_not_change_history() {
        let mut editor = Editor::default();
        let before = editor.document.clone();
        assert!(
            editor
                .execute(
                    "Invalid import",
                    Command::Batch(vec![
                        Command::Add(EntityKind::Achievement),
                        Command::SetRemoteSnapshot(json!({"appId":{"value":480}}))
                    ]),
                    None
                )
                .is_err()
        );
        assert_eq!(editor.document, before);
        assert_eq!(editor.revision(), 0);
        assert!(editor.undo_label().is_none());
    }
    #[test]
    fn delete_stat_restores_dependent_achievement_and_saved_state() {
        let mut editor = Editor::default();
        editor
            .execute(
                "Add",
                Command::Batch(vec![
                    Command::Add(EntityKind::Stat),
                    Command::Add(EntityKind::Achievement),
                ]),
                None,
            )
            .unwrap();
        let stat = editor.document.entities(EntityKind::Stat)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut achievement = editor.document.entities(EntityKind::Achievement)[0].clone();
        let id = achievement["id"].as_str().unwrap().to_owned();
        achievement["progressStatId"] = json!(stat);
        achievement["unlockValue"] = json!(10);
        editor
            .execute(
                "Link",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: achievement,
                },
                None,
            )
            .unwrap();
        editor.mark_saved();
        let saved = editor.document.clone();
        editor
            .execute(
                "Delete stat",
                Command::Delete {
                    kind: EntityKind::Stat,
                    ids: vec![stat],
                },
                None,
            )
            .unwrap();
        assert!(
            editor
                .document
                .entity(EntityKind::Achievement, &id)
                .unwrap()["progressStatId"]
                .is_null()
        );
        assert!(editor.undo());
        assert_eq!(editor.document, saved);
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert!(editor.is_dirty());
    }
    #[test]
    fn gestures_merge_until_finished_and_new_edits_discard_redo() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Stat), None)
            .unwrap();
        let id = editor.document.entities(EntityKind::Stat)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let start = editor.document.clone();
        for column in 1..5 {
            editor
                .execute(
                    "Move",
                    Command::Move {
                        kind: EntityKind::Stat,
                        id: id.clone(),
                        column,
                        row: 0,
                    },
                    Some("drag"),
                )
                .unwrap();
        }
        editor.finish_group();
        assert!(editor.undo());
        assert_eq!(editor.document, start);
        editor
            .execute("Another edit", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        assert!(editor.redo_label().is_none());
    }
}
