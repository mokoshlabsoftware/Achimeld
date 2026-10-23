use achimeld_application::{Command, Document, Editor, EntityKind, new_id};
use serde_json::{Value, json};

struct NoAssets;
impl crate::AssetSource for NoAssets {
    fn image(&mut self, _: &Value) -> Result<std::sync::Arc<image::RgbaImage>, String> {
        Err("No images".into())
    }
    fn font(&mut self, _: &str) -> Option<std::sync::Arc<crate::Font>> {
        None
    }
}

#[test]
fn radial_blur_adjustment_round_trips_masks_opacity_and_history_with_bounded_work() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let source = json!({"id":new_id(),"kind":"Shape","shape":{"kind":"Rectangle","colorHex":"#8000FF00","scaleX":0.15,"scaleY":0.2},"offsetX":128.0});
    let blur = json!({"id":new_id(),"kind":"RadialBlur","opacity":1.0,"adjustment":{"radialBlur":0.0,"angularBlur":0.0}});
    let save = |editor: &mut Editor, entity: &Value| {
        editor
            .execute(
                "Blur",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity.clone(),
                },
                None,
            )
            .unwrap()
    };
    let render = |editor: &Editor| {
        crate::render(editor.document(), &id, Some((48, 48)), &mut NoAssets)
            .unwrap()
            .achieved
    };
    entity["icon"] = json!({"layers":[source,blur]});
    save(&mut editor, &entity);
    let original = render(&editor);
    entity["icon"]["layers"][1]["adjustment"] = json!({"radialBlur":70.0,"angularBlur":30.0});
    save(&mut editor, &entity);
    let blurred = render(&editor);
    assert_ne!(original, blurred);
    let restored = Document::from_parts(
        serde_json::from_slice(&serde_json::to_vec(editor.document().project()).unwrap()).unwrap(),
        editor.document().catalog().clone(),
    )
    .unwrap();
    assert_eq!(
        crate::render(&restored, &id, Some((48, 48)), &mut NoAssets)
            .unwrap()
            .achieved,
        blurred
    );
    entity["icon"]["layers"][1]["opacity"] = json!(0.0);
    save(&mut editor, &entity);
    assert_eq!(render(&editor), original);
    assert!(editor.undo());
    assert_eq!(render(&editor), blurred);
    assert!(editor.redo());
    assert_eq!(render(&editor), original);
    entity["icon"]["layers"][1]["opacity"] = json!(1.0);
    let mask_id = new_id();
    entity["icon"]["layers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":mask_id,"kind":"Solid","solidColorHex":"#FF000000"}));
    entity["icon"]["layers"][1]["maskLayerId"] = json!(mask_id);
    entity["icon"]["layers"][1]["maskMode"] = json!("Luminance");
    save(&mut editor, &entity);
    assert_eq!(render(&editor), original);
    entity["icon"]["layers"][2]["solidColorHex"] = json!("#FFFFFFFF");
    save(&mut editor, &entity);
    assert_eq!(render(&editor), blurred);
    let mut invalid = editor.document().project().clone();
    invalid["achievements"][0]["icon"]["layers"][1]["adjustment"]["radialBlur"] = json!(101.0);
    assert!(Document::from_parts(invalid, editor.document().catalog().clone()).is_err());
    let mut dense = editor.document().project().clone();
    let mut layers = vec![source];
    for _ in 0..20 {
        let mut effect = blur.clone();
        effect["id"] = json!(new_id());
        effect["adjustment"] = json!({"radialBlur":100.0,"angularBlur":100.0});
        layers.push(effect);
    }
    dense["achievements"][0]["icon"] = json!({"layers":layers});
    let dense = Document::from_parts(dense, editor.document().catalog().clone()).unwrap();
    assert!(
        crate::render(&dense, &id, Some((16, 16)), &mut NoAssets)
            .err()
            .unwrap()
            .contains("budget")
    );
}
