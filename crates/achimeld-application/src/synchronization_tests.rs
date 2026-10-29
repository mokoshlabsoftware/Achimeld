use crate::{Command, Document, Editor, EntityKind};
use serde_json::json;

#[test]
fn pending_api_rename_preserves_verified_origin_across_edits_save_and_revert() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut editor = Editor::default();
        editor.execute("Add", Command::Add(kind), None).unwrap();
        let original = editor.document().entities(kind)[0].clone();
        let id = original["id"].as_str().unwrap().to_owned();
        let key = if kind == EntityKind::Stat {
            "stats"
        } else {
            "achievements"
        };
        let mut project = editor.document().project().clone();
        project["steamAppId"] = json!(480);
        project["lastRemoteSnapshot"] =
            json!({"appId":480, key:[{"apiName":original["apiName"],"steamId":"1"}]});
        let document = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        editor = Editor::new(document);
        for name in ["RENAMED_ONCE", "RENAMED_TWICE"] {
            let mut value = editor.document().entity(kind, &id).unwrap().clone();
            value["apiName"] = json!(name);
            editor
                .execute(
                    "Rename",
                    Command::ReplaceEntity {
                        kind,
                        id: id.clone(),
                        value,
                    },
                    None,
                )
                .unwrap();
            assert_eq!(
                editor.document().entity(kind, &id).unwrap()["pendingApiRename"]["from"],
                original["apiName"]
            );
        }
        let reopened = Document::from_parts(
            editor.document().project().clone(),
            editor.document().catalog().clone(),
        )
        .unwrap();
        let mut value = reopened.entity(kind, &id).unwrap().clone();
        value["apiName"] = original["apiName"].clone();
        editor
            .execute(
                "Revert rename",
                Command::ReplaceEntity {
                    kind,
                    id: id.clone(),
                    value,
                },
                None,
            )
            .unwrap();
        assert!(
            editor
                .document()
                .entity(kind, &id)
                .unwrap()
                .get("pendingApiRename")
                .is_none()
        );
        assert!(editor.undo());
        assert_eq!(
            editor.document().entity(kind, &id).unwrap()["apiName"],
            "RENAMED_TWICE"
        );
    }
}

#[test]
fn inactive_achievements_keep_their_identity_and_undoable_status_without_sync_authority() {
    let mut editor = Editor::default();
    editor
        .execute("Achievement", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let active = editor.document().clone();
    let id = active.entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap();
    for status in ["Unsynced", "Retired", "Active"] {
        let mut project = active.project().clone();
        project["achievements"][0]["steamSyncStatus"] = json!(status);
        let document = Document::from_parts(project, active.catalog().clone()).unwrap();
        editor
            .execute(
                "Sync status",
                Command::ApplyAutomation(Box::new(document.clone())),
                None,
            )
            .unwrap();
        assert_eq!(
            editor
                .document()
                .steam_entities(EntityKind::Achievement)
                .count(),
            1
        );
        assert_eq!(
            editor
                .document()
                .synchronized_entities(EntityKind::Achievement)
                .count(),
            usize::from(status != "Retired")
        );
        assert_eq!(
            editor.document().entities(EntityKind::Achievement)[0]["id"],
            id
        );
        assert!(editor.undo());
        assert!(editor.redo());
        let mut changed = document.project().clone();
        changed["achievements"][0]["internalName"] = json!("Unexpected remote edit");
        let changed = Document::from_parts(changed, document.catalog().clone()).unwrap();
        if status == "Retired" {
            let mut catalog = document.catalog().clone();
            let loc = document.entities(EntityKind::Achievement)[0]["displayNameLocalizationId"]
                .as_str()
                .unwrap();
            catalog
                .rows
                .get_mut(loc)
                .unwrap()
                .insert("English".into(), "Unexpected remote text".into());
            let changed_text = Document::from_parts(document.project().clone(), catalog).unwrap();
            assert!(
                editor
                    .execute(
                        "Sync",
                        Command::ApplySynchronization(Box::new(changed_text)),
                        None
                    )
                    .is_err()
            );
            assert!(
                editor
                    .execute(
                        "Sync",
                        Command::ApplySynchronization(Box::new(changed)),
                        None
                    )
                    .is_err()
            );
            assert!(
                editor
                    .execute(
                        "Sync",
                        Command::ApplySynchronization(Box::new(active.clone())),
                        None
                    )
                    .is_err()
            );
        }
    }
    for status in [json!(null), json!(true), json!("Deleted")] {
        let mut project = active.project().clone();
        project["achievements"][0]["steamSyncStatus"] = status;
        assert!(Document::from_parts(project, active.catalog().clone()).is_err());
    }
}

#[test]
fn changing_the_app_id_clears_previous_destination_history_and_undo_restores_it() {
    let mut editor = Editor::default();
    editor
        .execute("Achievement", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut project = editor.document().project().clone();
    project["steamAppId"] = json!(480);
    project["lastRemoteSnapshot"] = json!({"appId":480});
    project["lastVerifiedAtUtc"] = json!("2026-10-18T10:00:00Z");
    project["syncBaselines"] =
        json!([{"localId":project["achievements"][0]["id"], "entityFingerprint":"previous"}]);
    project["tombstones"] = json!([{"entityId":crate::new_id(), "apiName":"OLD"}]);
    let original = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
    let mut editor = Editor::new(original.clone());
    let settings = |app_id| Command::ProjectSettings {
        name: original.project()["displayName"].as_str().unwrap().into(),
        app_id,
        atlas: None,
        assets: None,
    };
    editor
        .execute("Same destination", settings(480), None)
        .unwrap();
    assert_eq!(editor.document(), &original);
    editor
        .execute("New destination", settings(481), None)
        .unwrap();
    assert_eq!(
        editor.document().project()["lastRemoteSnapshot"],
        serde_json::Value::Null
    );
    assert_eq!(
        editor.document().project()["lastVerifiedAtUtc"],
        serde_json::Value::Null
    );
    for key in ["syncBaselines", "tombstones"] {
        assert_eq!(editor.document().project()[key], json!([]));
    }
    assert_eq!(
        editor.document().entities(EntityKind::Achievement),
        original.entities(EntityKind::Achievement)
    );
    assert_eq!(
        editor.document().project()["assets"],
        original.project()["assets"]
    );
    assert!(editor.undo());
    assert_eq!(editor.document(), &original);
    assert!(editor.redo());
    assert_eq!(editor.document().app_id(), 481);
}

#[test]
fn synchronization_applies_one_local_history_transaction_and_preserves_project_pins() {
    let mut editor = Editor::default();
    editor
        .execute(
            "Project",
            Command::ProjectSettings {
                name: "Sync project".into(),
                app_id: 480,
                atlas: None,
                assets: None,
            },
            None,
        )
        .unwrap();
    editor
        .execute("Stat", Command::Add(EntityKind::Stat), None)
        .unwrap();
    let original = editor.document().clone();
    let mut project = original.project().clone();
    project["lastRemoteSnapshot"] =
        json!({"appId":480,"isComplete":true,"stats":[],"achievements":[]});
    project["lastVerifiedAtUtc"] = json!("2026-10-15T12:00:00Z");
    let verified = Document::from_parts(project, original.catalog().clone()).unwrap();
    editor
        .execute(
            "Apply local synchronization result",
            Command::ApplySynchronization(Box::new(verified.clone())),
            None,
        )
        .unwrap();
    assert_eq!(editor.document(), &verified);
    assert!(editor.undo());
    assert_eq!(editor.document(), &original);
    assert!(editor.redo());
    assert_eq!(editor.document(), &verified);
    for field in ["projectId", "steamAppId"] {
        let mut foreign = verified.project().clone();
        foreign[field] = if field == "projectId" {
            json!(crate::new_id())
        } else {
            json!(481)
        };
        let foreign = Document::from_parts(foreign, verified.catalog().clone()).unwrap();
        let revision = editor.revision();
        assert!(
            editor
                .execute(
                    "Foreign synchronization",
                    Command::ApplySynchronization(Box::new(foreign)),
                    None
                )
                .is_err()
        );
        assert_eq!(editor.document(), &verified);
        assert_eq!(editor.revision(), revision);
    }
}
