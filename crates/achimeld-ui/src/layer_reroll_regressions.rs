use super::*;
use crate::layer_toolbar::Action;

fn install_layers(rows: &mut LayerRows, layers: Vec<Value>) {
    let mut entity = rows.entity();
    entity["icon"] = json!({"layers":layers});
    rows.fixture
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: rows.achievement.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    rows.fixture.finish_group();
    rows.frame(vec![]);
    rows.frame(vec![]);
}

#[test]
fn reroll_footer_updates_entire_icon_once_and_undo_restores_every_seed() {
    for width in [440.0, 1100.0] {
        for passes in [1, 2] {
            let mut rows = LayerRows::new(width, passes);
            let mut layers: Vec<_> = [
                "Solid",
                "Folder",
                "Particle",
                "Folder",
                "Explosion",
                "PerlinNoise",
                "Particle",
            ]
            .into_iter()
            .map(layer_palette::new_layer)
            .collect();
            let folder = layers[1]["id"].clone();
            let nested = layers[3]["id"].clone();
            for index in [2, 3] {
                layers[index]["parentFolderId"] = folder.clone();
            }
            layers[4]["parentFolderId"] = nested;
            layers[4]["explosion"]["seed"] = json!(u32::MAX);
            layers[4]["explosion"]["differentSeedPerInstance"] = json!(true);
            layers[5]["perlinNoise"]["seed"] = json!(i32::MAX);
            layers[6]["visible"] = json!(false);
            layers[6]["particles"]["seed"] = json!(0);
            layers[6]["particles"]["differentSeedPerInstance"] = json!(true);
            layers[1]["maskLayerId"] = layers[6]["id"].clone();
            let selected = layers[0]["id"].as_str().unwrap().to_owned();
            let mask = layers[6]["id"].as_str().unwrap().to_owned();
            install_layers(&mut rows, layers);
            rows.desktop.selected_layer = Some(selected.clone());
            rows.desktop
                .layer_palette
                .collapsed
                .insert(folder.as_str().unwrap().into());
            rows.repeat_layout = true;
            rows.frame(vec![]);
            let before = rows.fixture.editor.document().clone();
            let background = rows.ctx.read_response(Action::Background.id()).unwrap();
            let reroll = rows.ctx.read_response(Action::Reroll.id()).unwrap();
            assert!(reroll.enabled());
            assert_eq!(reroll.rect.left(), background.rect.right());
            assert_eq!(reroll.rect.top(), background.rect.top());
            assert_eq!(reroll.rect.bottom(), background.rect.bottom());
            // The fixed footer includes its one-point separator.
            assert_eq!(reroll.rect.height(), crate::layer_toolbar::HEIGHT - 1.0);
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0))
                    .contains_rect(reroll.rect)
            );

            for _ in 0..2 {
                let previous = rows.entity();
                let previous_preview = render(
                    rows.fixture.editor.document(),
                    &rows.achievement,
                    Some((16, 16)),
                    &mut NoAssets,
                )
                .unwrap();
                let revision = rows.fixture.editor.revision();
                let point = reroll.rect.center();
                rows.press(point);
                rows.frame(vec![button(point, false)]);
                rows.frame(vec![]);
                assert!(rows.desktop.entity_drafts.is_empty());
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                assert_eq!(
                    rows.fixture.editor.undo_label(),
                    Some("Re-roll all random layers")
                );
                assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&selected));
                let after = rows.entity();
                let mut expected = previous.clone();
                for (index, key, maximum) in [
                    (2, "particles", u64::from(u32::MAX)),
                    (4, "explosion", u64::from(u32::MAX)),
                    (5, "perlinNoise", i32::MAX as u64),
                    (6, "particles", u64::from(u32::MAX)),
                ] {
                    let seed = &after["icon"]["layers"][index][key]["seed"];
                    assert_ne!(seed, &previous["icon"]["layers"][index][key]["seed"]);
                    assert!(seed.as_u64().unwrap() <= maximum);
                    expected["icon"]["layers"][index][key]["seed"] = seed.clone();
                }
                assert_eq!(after, expected, "Only the four random seeds change");
                assert_eq!(after["icon"]["layers"][1]["maskLayerId"], mask);
                let preview = render(
                    rows.fixture.editor.document(),
                    &rows.achievement,
                    Some((16, 16)),
                    &mut NoAssets,
                )
                .unwrap();
                assert_ne!(preview.achieved, previous_preview.achieved);
                let after = rows.fixture.editor.document().clone();
                rows.fixture.undo();
                assert_eq!(rows.entity(), previous);
                rows.fixture.redo();
                assert_eq!(rows.fixture.editor.document(), &after);
            }
            rows.fixture.undo();
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
        }
    }
}

#[test]
fn reroll_is_disabled_without_random_layers_or_when_the_panel_is_disabled() {
    for random in [false, true] {
        let mut rows = LayerRows::new(1100.0, 1);
        if random {
            install_layers(&mut rows, vec![layer_palette::new_layer("Particle")]);
        }
        rows.input_frame(vec![], !random, true);
        rows.input_frame(vec![], !random, true);
        let reroll = rows.ctx.read_response(Action::Reroll.id()).unwrap();
        assert!(!reroll.enabled(), "random={random}");
        let before = rows.fixture.editor.document().clone();
        let revision = rows.fixture.editor.revision();
        let point = reroll.rect.center();
        for pressed in [true, false] {
            rows.input_frame(
                vec![egui::Event::PointerMoved(point), button(point, pressed)],
                !random,
                true,
            );
        }
        assert_eq!(rows.fixture.editor.document(), &before);
        assert_eq!(rows.fixture.editor.revision(), revision);
    }
}

#[test]
fn reroll_materializes_only_missing_seed_fields_and_preserves_effect_selection() {
    let mut desktop = Desktop::default();
    desktop.selected_layer = Some("solid".into());
    desktop.layer_palette.selected_effect = Some("effect".into());
    let mut icon = json!({"layers":[
        {"id":"solid","kind":"Solid","unrelated":{"seed":99}},
        {"id":"p","kind":"Particle"},
        {"id":"e","kind":"Explosion","explosion":null},
        {"id":"n","kind":"PerlinNoise","perlinNoise":{"frequency":3.0}},
        {"id":"unknown","kind":"FutureLayer","seed":42}
    ]});
    let mut expected = icon.clone();
    desktop.layer_action(layer_palette::LayerAction::Reroll, &mut icon);
    for (index, key) in [(1, "particles"), (2, "explosion"), (3, "perlinNoise")] {
        let seed = icon["layers"][index][key]["seed"].as_u64().unwrap();
        assert_ne!(seed, if index == 3 { 0 } else { 1 });
        if !expected["layers"][index][key].is_object() {
            expected["layers"][index][key] = json!({});
        }
        expected["layers"][index][key]["seed"] = json!(seed);
    }
    assert_eq!(icon, expected);
    assert_eq!(desktop.selected_layer.as_deref(), Some("solid"));
    assert_eq!(
        desktop.layer_palette.selected_effect.as_deref(),
        Some("effect")
    );
}
