//! Project style transactions shared by the viewer and its layer editor.
use crate::{Command, Document, EntityKind, IconStyle, style_signature};
use serde_json::Value;

pub fn uses_style(entity: &Value, style: &IconStyle) -> bool {
    if crate::style_achievements::is_local_icon(entity) {
        return entity["iconStyleId"].as_str() == Some(&style.id);
    }
    style_signature(&entity["icon"]) == style_signature(&style.recipe)
}

pub fn edit(document: &Document, style: IconStyle) -> Result<Command, String> {
    style.validate()?;
    let old = document
        .icon_styles()
        .into_iter()
        .find(|s| s.id == style.id)
        .ok_or("Project style no longer exists")?;
    let mut commands = Vec::new();
    if old.recipe != style.recipe {
        for entity in document
            .entities(EntityKind::Achievement)
            .iter()
            .filter(|e| uses_style(e, &old))
        {
            // Explicit style nodes are reconciled by history in this same transaction.
            if crate::style_achievements::is_style_achievement(entity) {
                continue;
            }
            let mut value = entity.clone();
            value["icon"] = crate::style_collections::clone_recipe(&style.recipe)?;
            commands.push(Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: entity["id"]
                    .as_str()
                    .ok_or("Missing achievement identity")?
                    .into(),
                value,
            });
        }
    }
    commands.push(Command::SaveIconStyle(style));
    Ok(Command::Batch(commands))
}

pub fn remove(document: &Document, id: &str, replacement: Option<&str>) -> Result<Command, String> {
    let styles = document.icon_styles();
    let style = styles
        .iter()
        .find(|s| s.id == id)
        .ok_or("Project style no longer exists")?;
    let replacement = replacement
        .map(|other| {
            styles
                .iter()
                .find(|s| s.id == other && other != id)
                .ok_or("Choose another project style")
        })
        .transpose()?;
    let mut commands = Vec::new();
    for entity in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|e| uses_style(e, style))
    {
        let mut value = entity.clone();
        if let Some(next) = replacement {
            value["icon"] = crate::style_collections::clone_recipe(&next.recipe)?;
            if crate::style_achievements::is_style_achievement(entity) {
                value["iconStyleId"] = serde_json::json!(next.id);
            }
        } else {
            value
                .as_object_mut()
                .ok_or("Invalid achievement")?
                .remove("iconStyleId");
        }
        commands.push(Command::ReplaceEntity {
            kind: EntityKind::Achievement,
            id: entity["id"]
                .as_str()
                .ok_or("Missing achievement identity")?
                .into(),
            value,
        });
    }
    commands.push(Command::DeleteIconStyle(id.into()));
    Ok(Command::Batch(commands))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Editor, new_id};
    use serde_json::json;

    fn fixture() -> (Editor, IconStyle, IconStyle) {
        let mut editor = Editor::default();
        editor
            .execute(
                "Node",
                Command::AddStyleAchievementAt { position: [0, 0] },
                None,
            )
            .unwrap();
        let first = editor.document().icon_styles()[0].clone();
        let second = IconStyle {
            id: new_id(),
            name: "Replacement".into(),
            preview_png: None,
            recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF102030","visible":true}]}),
        };
        editor
            .execute("Second", Command::SaveIconStyle(second.clone()), None)
            .unwrap();
        editor
            .execute("Achievement", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut icon = editor.document().entities(EntityKind::Achievement)[1].clone();
        icon["icon"] = crate::style_collections::clone_recipe(&first.recipe).unwrap();
        editor
            .execute(
                "Apply",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: icon["id"].as_str().unwrap().into(),
                    value: icon,
                },
                None,
            )
            .unwrap();
        (editor, first, second)
    }

    #[test]
    fn editing_updates_used_icons_and_linked_nodes_in_one_undo_step() {
        let (mut editor, mut style, _) = fixture();
        let before = editor.document().clone();
        style.recipe["layers"][0]["opacity"] = json!(0.35);
        editor
            .execute(
                "Edit",
                edit(editor.document(), style.clone()).unwrap(),
                None,
            )
            .unwrap();
        for icon in editor.document().entities(EntityKind::Achievement) {
            assert_eq!(
                style_signature(&icon["icon"]),
                style_signature(&style.recipe)
            );
        }
        let after = editor.document().clone();
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(editor.redo());
        assert_eq!(editor.document(), &after);
    }

    #[test]
    fn deleting_keeps_exact_appearance_or_relinks_and_is_atomic() {
        for replace in [false, true] {
            let (mut editor, first, second) = fixture();
            let before = editor.document().clone();
            let command = remove(
                editor.document(),
                &first.id,
                replace.then_some(second.id.as_str()),
            )
            .unwrap();
            editor.execute("Delete", command, None).unwrap();
            assert!(
                !editor
                    .document()
                    .icon_styles()
                    .iter()
                    .any(|s| s.id == first.id)
            );
            for (old, new) in before
                .entities(EntityKind::Achievement)
                .iter()
                .zip(editor.document().entities(EntityKind::Achievement))
            {
                if replace {
                    assert_eq!(
                        style_signature(&new["icon"]),
                        style_signature(&second.recipe)
                    );
                } else {
                    assert_eq!(old["icon"], new["icon"]);
                    assert!(new.get("iconStyleId").is_none());
                }
            }
            assert!(editor.undo());
            assert_eq!(editor.document(), &before);
            assert!(remove(editor.document(), &first.id, Some(&first.id)).is_err());
            assert!(remove(editor.document(), &first.id, Some(&new_id())).is_err());
            assert_eq!(editor.document(), &before);
        }
    }
}
