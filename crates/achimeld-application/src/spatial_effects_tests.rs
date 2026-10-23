use crate::{Command, Editor, EntityKind, new_id};
use serde_json::json;

#[test]
fn spatial_effects_are_validated_atomically_and_round_trip_through_history() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let before = editor.document().clone();
    let mut entity = before.entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","effects":[
        {"id":new_id(),"kind":"Skew","skewX":60,"skewY":-60},
        {"id":new_id(),"kind":"Perspective","perspectiveX":0.45,"perspectiveY":-0.45},
        {"id":new_id(),"kind":"Pinch","amount":1},
        {"id":new_id(),"kind":"LensBlur","blur":128,"aberration":64}]}]});
    let replace = |value| Command::ReplaceEntity {
        kind: EntityKind::Achievement,
        id: id.clone(),
        value,
    };
    editor
        .execute("Spatial effects", replace(entity.clone()), None)
        .unwrap();
    let valid = editor.document().clone();
    for (index, key, invalid) in [
        (0, "skewX", json!(60.01)),
        (0, "skewY", json!(-60.01)),
        (1, "perspectiveX", json!(0.451)),
        (1, "perspectiveY", json!(-0.451)),
        (2, "amount", json!(1.01)),
        (3, "blur", json!(128.01)),
        (3, "aberration", json!(64.01)),
        (2, "opacity", json!(-0.1)),
        (0, "skewX", json!("bad")),
    ] {
        let mut bad = entity.clone();
        bad["icon"]["layers"][0]["effects"][index][key] = invalid;
        assert!(editor.execute("Invalid", replace(bad), None).is_err());
        assert_eq!(editor.document(), &valid);
    }
    editor.undo();
    assert_eq!(editor.document(), &before);
    editor.redo();
    assert_eq!(editor.document(), &valid);
}

#[test]
fn outer_light_round_trips_and_rejects_invalid_parameters_without_committing() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    let before = editor.document().clone();
    let mut entity = before.entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","effects":[
        {"id":new_id(),"kind":"OuterLight","width":4,"blur":12,"opacity":0.75,"colorHex":"#FFFFFFFF","blendMode":"Screen"}]}]});
    let replace = |value| Command::ReplaceEntity {
        kind: EntityKind::Achievement,
        id: id.clone(),
        value,
    };
    editor
        .execute("Outer light", replace(entity.clone()), None)
        .unwrap();
    let valid = editor.document().clone();
    for (key, value) in [
        ("width", json!(256.1)),
        ("blur", json!(-0.1)),
        ("blur", json!("bad")),
        ("opacity", json!(1.1)),
    ] {
        let mut bad = entity.clone();
        bad["icon"]["layers"][0]["effects"][0][key] = value;
        assert!(editor.execute("Invalid light", replace(bad), None).is_err());
        assert_eq!(editor.document(), &valid);
    }
    editor.undo();
    assert_eq!(editor.document(), &before);
    editor.redo();
    assert_eq!(editor.document(), &valid);
}
