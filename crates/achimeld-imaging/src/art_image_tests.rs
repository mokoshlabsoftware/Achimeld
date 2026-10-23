use crate::{AssetSource, render, render_achieved_cancellable, render_layer_cancellable};
use achimeld_application::{Command, Editor, EntityKind, new_id};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::Arc;
struct NoAssets;
impl AssetSource for NoAssets {
    fn image(&mut self, _: &Value) -> Result<Arc<RgbaImage>, String> {
        Err("Unavailable artwork".into())
    }
    fn font(&mut self, _: &str) -> Option<Arc<crate::Font>> {
        None
    }
}
fn replace(editor: &mut Editor, entity: Value) {
    editor
        .execute(
            "Edit",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: entity["id"].as_str().unwrap().into(),
                value: entity,
            },
            None,
        )
        .unwrap();
}
#[test]
fn composed_art_images_are_live_general_sources_in_previews_layers_and_exports() {
    let mut editor = Editor::default();
    editor
        .execute(
            "Create",
            Command::Batch(vec![
                Command::AddArtImageAt { position: [0, 0] },
                Command::AddArtImageAt { position: [40, 0] },
                Command::Add(EntityKind::Achievement),
                Command::Add(EntityKind::Achievement),
            ]),
            None,
        )
        .unwrap();
    let ids: Vec<_> = editor
        .document()
        .entities(EntityKind::Achievement)
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    let mut source = editor
        .document()
        .entity(EntityKind::Achievement, &ids[0])
        .unwrap()
        .clone();
    source["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#80FF0000"}]});
    replace(&mut editor, source.clone());
    for index in 1..4 {
        let mut entity = editor
            .document()
            .entity(EntityKind::Achievement, &ids[index])
            .unwrap()
            .clone();
        entity["generalImageEntries"] =
            json!([{"id":0,"artImageId":ids[if index==3 {0}else{index-1}]}]);
        entity["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}]});
        replace(&mut editor, entity);
    }
    let before = editor.document().clone();
    for color in ["#80FF0000", "#8000FF00"] {
        source["icon"]["layers"][0]["solidColorHex"] = json!(color);
        replace(&mut editor, source.clone());
        let expected = if color == "#80FF0000" {
            Rgba([255, 0, 0, 128])
        } else {
            Rgba([0, 255, 0, 128])
        };
        for id in &ids {
            let output = render(editor.document(), id, Some((128, 128)), &mut NoAssets).unwrap();
            assert!(output.achieved.pixels().all(|p| *p == expected));
            let quick = render_achieved_cancellable(
                editor.document(),
                id,
                Some((64, 64)),
                &mut NoAssets,
                &|| false,
            )
            .unwrap();
            assert!(quick.pixels().all(|p| *p == expected));
            let layer = editor
                .document()
                .entity(EntityKind::Achievement, id)
                .unwrap()["icon"]["layers"][0]["id"]
                .as_str()
                .unwrap();
            assert_eq!(
                render_layer_cancellable(editor.document(), id, layer, &mut NoAssets, &|| false)
                    .unwrap(),
                output.achieved
            );
        }
    }
    editor.undo();
    assert_eq!(editor.document(), &before);
    assert!(
        render_achieved_cancellable(
            editor.document(),
            &ids[2],
            Some((64, 64)),
            &mut NoAssets,
            &|| true
        )
        .is_err()
    );
    editor
        .execute(
            "Delete art",
            Command::Delete {
                kind: EntityKind::Achievement,
                ids: vec![ids[0].clone()],
            },
            None,
        )
        .unwrap();
    assert!(
        render(editor.document(), &ids[2], Some((16, 16)), &mut NoAssets)
            .unwrap()
            .achieved
            .pixels()
            .all(|p| p[3] == 0)
    );
    editor.undo();
    assert_eq!(editor.document(), &before);
}
#[test]
fn linked_art_uses_the_receiving_layers_opacity_and_preserves_other_layers_with_missing_artwork() {
    let mut editor = Editor::default();
    editor
        .execute(
            "Create",
            Command::Batch(vec![
                Command::AddArtImageAt { position: [0, 0] },
                Command::Add(EntityKind::Achievement),
            ]),
            None,
        )
        .unwrap();
    let mut art = editor.document().entities(EntityKind::Achievement)[0].clone();
    let mut target = editor.document().entities(EntityKind::Achievement)[1].clone();
    art["icon"] = json!({"layers":[{"id":new_id(),"kind":"StandaloneImage","imageId":new_id()},
        {"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]});
    replace(&mut editor, art.clone());
    target["generalImageEntries"] = json!([{"id":0,"artImageId":art["id"]}]);
    target["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF0000FF"},
        {"id":new_id(),"kind":"AchievementImage","generalImageIndex":0,"opacity":0.5}]});
    let id = target["id"].as_str().unwrap().to_owned();
    replace(&mut editor, target);
    let output = render_achieved_cancellable(
        editor.document(),
        &id,
        Some((16, 16)),
        &mut NoAssets,
        &|| false,
    )
    .unwrap();
    assert_eq!(*output.get_pixel(8, 8), Rgba([128, 0, 128, 255]));
    assert!(render(editor.document(), &id, Some((16, 16)), &mut NoAssets).is_err());
}

#[test]
fn linked_art_keeps_folder_nesting_inside_one_render_budget() {
    let mut editor = Editor::default();
    editor
        .execute(
            "Create",
            Command::Batch(vec![
                Command::AddArtImageAt { position: [0, 0] },
                Command::Add(EntityKind::Achievement),
            ]),
            None,
        )
        .unwrap();
    let art = editor.document().entities(EntityKind::Achievement)[0]["id"].clone();
    let commands=editor.document().entities(EntityKind::Achievement).iter().enumerate().map(|(index,entity)|{
        let mut entity=entity.clone();let mut layers=Vec::new();let mut parent:Option<String>=None;
        for _ in 0..40 {let id=new_id();layers.push(json!({"id":id,"kind":"Folder","parentFolderId":parent}));parent=Some(id);}
        layers.push(if index==0 {json!({"id":new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF","parentFolderId":parent})}
            else {json!({"id":new_id(),"kind":"AchievementImage","generalImageIndex":0,"parentFolderId":parent})});
        entity["icon"]=json!({"layers":layers});
        if index==1 {entity["generalImageEntries"]=json!([{"id":0,"artImageId":art}]);}
        Command::ReplaceEntity{kind:EntityKind::Achievement,id:entity["id"].as_str().unwrap().into(),value:entity}
    }).collect();
    editor
        .execute("Nested recipes", Command::Batch(commands), None)
        .unwrap();
    let target = editor.document().entities(EntityKind::Achievement)[1]["id"]
        .as_str()
        .unwrap();
    assert!(
        render_achieved_cancellable(
            editor.document(),
            target,
            Some((1, 1)),
            &mut NoAssets,
            &|| false
        )
        .unwrap_err()
        .contains("nesting")
    );
}
