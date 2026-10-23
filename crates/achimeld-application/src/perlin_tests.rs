use crate::{Command, Editor, EntityKind, new_id};
use serde_json::json;

#[test]
fn perlin_and_mask_options_validate_atomically_and_survive_history() {
    let mut editor = Editor::default();
    editor
        .execute("Add", Command::Add(EntityKind::Achievement), None)
        .unwrap();
    editor.finish_group();
    let before = editor.document().clone();
    let mut entity = before.entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let mask = new_id();
    entity["icon"] = json!({"layers":[
        {"id":new_id(),"kind":"Solid","maskLayerId":mask,"maskMode":"Luminance","maskInvert":true},
        {"id":mask,"kind":"PerlinNoise","perlinNoise":{"seed":2147483647,"frequency":128,"octaves":12,
            "persistence":1,"lacunarity":4,"enabledOctaves":[true,false,true,false],"useGradient":true,
            "gradient":{"stops":[{"offset":0.0,"colorHex":"#00102030"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}}}]});
    let replace = |value| Command::ReplaceEntity {
        kind: EntityKind::Achievement,
        id: id.clone(),
        value,
    };
    editor
        .execute("Noise mask", replace(entity.clone()), None)
        .unwrap();
    let valid = editor.document().clone();
    for (key, bad) in [
        ("seed", json!(-1)),
        ("seed", json!(2147483648u64)),
        ("octaves", json!(13)),
        ("octaves", json!(1.5)),
        ("frequency", json!(0)),
        ("frequency", json!(129)),
        ("persistence", json!(1.01)),
        ("lacunarity", json!(4.01)),
        ("enabledOctaves", json!([true, 0])),
        ("enabledOctaves", json!(vec![true; 13])),
        ("useGradient", json!("yes")),
        ("gradient", json!({"stops":[]})),
        (
            "gradient",
            json!({"stops":[{"offset":1.1,"colorHex":"#FFFFFFFF"}]}),
        ),
        (
            "gradient",
            json!({"stops":[{"offset":0.0,"colorHex":"invalid"}]}),
        ),
    ] {
        let mut bad_entity = entity.clone();
        bad_entity["icon"]["layers"][1]["perlinNoise"][key] = bad;
        assert!(
            editor
                .execute("Invalid", replace(bad_entity), None)
                .is_err(),
            "{key}"
        );
        assert_eq!(editor.document(), &valid);
    }
    for (key, bad) in [
        ("maskMode", json!("Other")),
        ("maskInvert", json!(1)),
        ("maskLayerId", json!(false)),
    ] {
        let mut bad_entity = entity.clone();
        bad_entity["icon"]["layers"][0][key] = bad;
        assert!(
            editor
                .execute("Invalid", replace(bad_entity), None)
                .is_err()
        );
        assert_eq!(editor.document(), &valid);
    }
    editor.undo();
    assert_eq!(editor.document(), &before);
    editor.redo();
    assert_eq!(editor.document(), &valid);
    let restored = crate::Document::from_parts(
        serde_json::from_str(&serde_json::to_string(valid.project()).unwrap()).unwrap(),
        valid.catalog().clone(),
    )
    .unwrap();
    assert_eq!(&restored, &valid);
}
