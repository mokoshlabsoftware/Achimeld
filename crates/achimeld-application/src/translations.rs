//! Translator copies use stable localization IDs, never array positions or fallback text.
use crate::{Catalog, Command, Document, EntityKind};
use achimeld_core::LANGUAGES;

pub fn achievement_catalog(document: &Document) -> Result<Catalog, String> {
    let mut catalog = Catalog::default();
    for achievement in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| !crate::art_images::is_art_image(entity))
    {
        let api = achievement["apiName"]
            .as_str()
            .ok_or("Missing achievement API name")?;
        for (key, field) in [
            ("displayNameLocalizationId", "Display name"),
            ("descriptionLocalizationId", "Description"),
        ] {
            let id = achievement[key].as_str().ok_or("Missing localization ID")?;
            let mut row = document
                .catalog()
                .rows
                .get(id)
                .ok_or("Missing achievement localization row")?
                .clone();
            row.insert("Context".into(), format!("{api} / {field}"));
            if catalog.rows.insert(id.into(), row).is_some() {
                return Err("Achievements share a localization ID; give each field its own row before exporting".into());
            }
        }
    }
    if document.entities(EntityKind::Stat).iter().any(|stat| {
        stat["displayNameLocalizationId"]
            .as_str()
            .is_some_and(|id| catalog.rows.contains_key(id))
    }) {
        return Err("Achievement and stat text share a localization ID; separate their localization rows before exporting".into());
    }
    Ok(catalog)
}

/// Validate every row before returning a single undoable transaction. Blank cells
/// mean no change; clearing a translation remains an explicit in-app edit.
pub fn import_achievements(
    document: &Document,
    translated: &Catalog,
) -> Result<(Command, usize), String> {
    let expected = achievement_catalog(document)?;
    let mut changes = Vec::new();
    for (id, row) in &translated.rows {
        let source = expected.rows.get(id).ok_or_else(|| {
            format!("Unknown achievement translation ID {id}; export a fresh CSV from this project")
        })?;
        for field in ["Context", "English"] {
            if row.get(field) != source.get(field) {
                return Err(format!(
                    "{field} changed for translation {id}; keep ID, Context and English unchanged, or export a fresh CSV"
                ));
            }
        }
        let english = source.get("English").ok_or("Missing English source text")?;
        for language in LANGUAGES
            .into_iter()
            .filter(|language| *language != "English")
        {
            let Some(text) = row.get(language).filter(|text| !text.is_empty()) else {
                continue;
            };
            if text.matches("{0}").count() != english.matches("{0}").count() {
                return Err(format!(
                    "Keep the same {{0}} placeholders in {language} for translation {id}"
                ));
            }
            if text.len() > 1_048_576 {
                return Err(format!("Translation {id} in {language} is too long"));
            }
            if source.get(language) != Some(text) {
                changes.push(Command::SetLocalization {
                    id: id.clone(),
                    language: language.into(),
                    text: text.clone(),
                });
            }
        }
    }
    let count = changes.len();
    Ok((Command::Batch(changes), count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Editor;

    fn editor() -> Editor {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        editor
            .execute("Add stat", Command::Add(EntityKind::Stat), None)
            .unwrap();
        editor.mark_saved();
        editor
    }

    #[test]
    fn translator_copy_preserves_templates_and_import_is_one_undoable_edit() {
        let mut editor = editor();
        let id =
            editor.document().entities(EntityKind::Achievement)[0]["displayNameLocalizationId"]
                .as_str()
                .unwrap()
                .to_owned();
        editor
            .execute(
                "Source",
                Command::SetLocalization {
                    id: id.clone(),
                    language: "English".into(),
                    text: "Find {0} sticks, \"please\"\nNow".into(),
                },
                None,
            )
            .unwrap();
        editor.mark_saved();
        let before = editor.document().clone();
        let mut copy = achievement_catalog(&before).unwrap();
        assert_eq!(copy.rows.len(), 2);
        assert_eq!(
            copy.rows[&id]["English"],
            "Find {0} sticks, \"please\"\nNow"
        );
        assert_eq!(copy.rows[&id]["Polish"], "");
        copy.rows.get_mut(&id).unwrap().insert(
            "Polish".into(),
            "Znajdź {0} patyków, \"proszę\"\nTeraz".into(),
        );
        copy.rows
            .get_mut(&id)
            .unwrap()
            .insert("Japanese".into(), "棒を{0}本見つける".into());
        let (command, count) = import_achievements(&before, &copy).unwrap();
        assert_eq!(count, 2);
        editor
            .execute("Import translations", command, None)
            .unwrap();
        assert_eq!(editor.document().project(), before.project());
        assert!(editor.is_dirty());
        let after = editor.document().clone();
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(!editor.is_dirty());
        assert!(editor.redo());
        assert_eq!(editor.document(), &after);
        let mut blank = achievement_catalog(editor.document()).unwrap();
        blank
            .rows
            .get_mut(&id)
            .unwrap()
            .insert("Polish".into(), String::new());
        assert_eq!(import_achievements(editor.document(), &blank).unwrap().1, 0);
        blank.rows.retain(|key, _| key == &id);
        assert_eq!(import_achievements(editor.document(), &blank).unwrap().1, 0);
    }

    #[test]
    fn stale_foreign_and_broken_placeholder_rows_reject_the_entire_import() {
        let editor = editor();
        let before = editor.document().clone();
        let copy = achievement_catalog(&before).unwrap();
        let id = copy.rows.keys().next().unwrap().clone();
        for field in ["English", "Context"] {
            let mut bad = copy.clone();
            bad.rows
                .get_mut(&id)
                .unwrap()
                .insert(field.into(), "changed".into());
            assert!(import_achievements(&before, &bad).is_err());
        }
        let mut bad = copy.clone();
        bad.rows
            .get_mut(&id)
            .unwrap()
            .insert("French".into(), "{0}".into());
        assert!(import_achievements(&before, &bad).is_err());
        let mut bad = copy.clone();
        bad.rows.insert("FOREIG".into(), copy.rows[&id].clone());
        assert!(import_achievements(&before, &bad).is_err());
        assert_eq!(editor.document(), &before);
    }

    #[test]
    fn shared_stat_rows_cannot_be_modified_through_achievement_translation_import() {
        let editor = editor();
        let mut project = editor.document().project().clone();
        project["stats"][0]["displayNameLocalizationId"] =
            project["achievements"][0]["displayNameLocalizationId"].clone();
        let shared = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        assert!(achievement_catalog(&shared).is_err());
        assert!(import_achievements(&shared, &Catalog::default()).is_err());
    }
}
