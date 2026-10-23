use super::*;

fn frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 920.0),
            )),
            events,
            ..Default::default()
        },
        |ui| desktop.show(ui, fixture),
    );
    output.textures_delta.clear();
    output
}

fn text_rect(output: &egui::FullOutput, label: &str, within: egui::Rect) -> egui::Rect {
    fn visit(shape: &egui::Shape, label: &str, within: egui::Rect) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                within.contains_rect(rect).then_some(rect)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| visit(shape, label, within)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| visit(&shape.shape, label, within))
        .unwrap_or_else(|| panic!("Missing visible label {label}"))
}

fn click(ctx: &egui::Context, desktop: &mut Desktop, fixture: &mut Fixture, pos: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            desktop,
            fixture,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn sidebar_preference_defaults_on_and_explicit_choices_round_trip() {
    assert!(Preferences::default().layer_properties_in_sidebar);
    let old: Preferences = serde_json::from_value(json!({"normalSize": 15.0})).unwrap();
    assert!(old.layer_properties_in_sidebar);
    assert!(
        serde_json::to_value(&old)
            .unwrap()
            .get("layerPropertiesInSidebar")
            .is_none()
    );
    for enabled in [false, true] {
        let preferences = Preferences {
            layer_properties_in_sidebar: enabled,
            ..old.clone()
        };
        preferences.validate().unwrap();
        let encoded = serde_json::to_value(&preferences).unwrap();
        if !enabled {
            assert_eq!(encoded["layerPropertiesInSidebar"], false);
        }
        assert_eq!(
            serde_json::from_value::<Preferences>(encoded).unwrap(),
            preferences
        );
        let explicit: Preferences =
            serde_json::from_value(json!({"layerPropertiesInSidebar": enabled})).unwrap();
        assert_eq!(explicit.layer_properties_in_sidebar, enabled);
    }
}

#[test]
fn effect_mini_values_default_on_and_saved_off_round_trips() {
    let old: Preferences = serde_json::from_value(json!({"normalSize": 15.0})).unwrap();
    assert!(old.show_effect_mini_values);
    assert!(
        serde_json::to_value(&old)
            .unwrap()
            .get("showEffectMiniValues")
            .is_none()
    );
    let off: Preferences = serde_json::from_value(json!({"showEffectMiniValues": false})).unwrap();
    assert!(!off.show_effect_mini_values);
    assert_eq!(
        serde_json::from_value::<Preferences>(serde_json::to_value(&off).unwrap()).unwrap(),
        off
    );
}

#[test]
fn sidebar_edits_selected_layer_and_restores_combined_window() {
    let mut fixture = Fixture::new();
    fixture.layout.position = Some([140.0, 180.0]);
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let layer = new_id();
    let effect = new_id();
    entity["icon"] = json!({"layers":[{"id":layer,"kind":"Solid","displayName":"Selected artwork","opacity":1.0,"visible":true,"solidColorHex":"#FFFFFFFF","effects":[{"id":effect,"kind":"Outline","visible":true,"width":3.0,"colorHex":"#FFFFFFFF"}]}]});
    fixture
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    desktop.selected_layer = Some(layer.clone());
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    let default_stack = desktop.layer_palette.panel_rect.unwrap();
    let default_properties = desktop.layer_palette.properties_rect.unwrap();
    assert!(default_properties.left() > default_stack.right());
    let output = frame(&ctx, &mut desktop, &mut fixture, vec![]);
    text_rect(&output, "Opacity", default_properties);
    text_rect(&output, "Name", default_properties);
    let preferences = Preferences {
        layer_properties_in_sidebar: false,
        ..fixture.preferences().clone()
    };
    fixture
        .execute("Combined layout", Command::Preferences(preferences), None)
        .unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    let combined = desktop.layer_palette.panel_rect.unwrap();
    assert!(desktop.layer_palette.properties_rect.is_none());
    let preferences = Preferences {
        layer_properties_in_sidebar: true,
        ..fixture.preferences().clone()
    };
    fixture
        .execute("Layout", Command::Preferences(preferences), None)
        .unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    let compact = desktop.layer_palette.panel_rect.unwrap();
    let properties = desktop.layer_palette.properties_rect.unwrap();
    assert!(
        compact.width() < combined.width() * 0.7,
        "{compact:?} vs {combined:?}"
    );
    assert!(properties.left() > compact.right());
    let output = frame(&ctx, &mut desktop, &mut fixture, vec![]);
    let name = text_rect(&output, "Selected artwork", properties).center();
    click(&ctx, &mut desktop, &mut fixture, name);
    frame(
        &ctx,
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text("Sidebar edit".into()),
        ],
    );
    assert_eq!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap()["icon"]["layers"][0]["displayName"],
        "Sidebar edit"
    );
    fixture.finish_group();
    assert!(fixture.editor.undo());
    assert_eq!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap()["icon"]["layers"][0]["displayName"],
        "Selected artwork"
    );
    desktop.layer_palette.selected_effect = Some(effect);
    let output = frame(&ctx, &mut desktop, &mut fixture, vec![]);
    text_rect(
        &output,
        "Width",
        desktop.layer_palette.properties_rect.unwrap(),
    );
    desktop.layer_palette.selected_effect = None;
    let preferences = Preferences {
        layer_properties_in_sidebar: false,
        ..fixture.preferences().clone()
    };
    fixture
        .execute("Combined layout", Command::Preferences(preferences), None)
        .unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    assert!(desktop.layer_palette.properties_rect.is_none());
    assert!((desktop.layer_palette.panel_rect.unwrap().width() - combined.width()).abs() < 4.0);
    assert_eq!(desktop.selected_layer.as_deref(), Some(layer.as_str()));
}

#[test]
fn preferences_cancel_preserves_the_saved_layout_option() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    desktop.open_preferences(&fixture);
    desktop
        .preferences
        .as_mut()
        .unwrap()
        .layer_properties_in_sidebar = false;
    let draft = desktop.preferences.as_mut().unwrap();
    draft.field_label_min_width = 120.0;
    draft.field_label_width_percent = 25.0;
    draft.abbreviate_field_labels = false;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    let output = frame(&ctx, &mut desktop, &mut fixture, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut fixture,
        text_rect(&output, "Cancel", egui::Rect::EVERYTHING).center(),
    );
    assert!(desktop.preferences.is_none());
    assert!(fixture.preferences().layer_properties_in_sidebar);
    assert_eq!(fixture.preferences().field_label_min_width, 90.0);
    assert_eq!(fixture.preferences().field_label_width_percent, 15.0);
    assert!(fixture.preferences().abbreviate_field_labels);
}

#[test]
fn preferences_save_can_disable_the_default_sidebar_option() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    desktop.open_preferences(&fixture);
    desktop
        .preferences
        .as_mut()
        .unwrap()
        .layer_properties_in_sidebar = false;
    let draft = desktop.preferences.as_mut().unwrap();
    draft.field_label_min_width = 120.0;
    draft.field_label_width_percent = 25.0;
    draft.abbreviate_field_labels = false;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&ctx, &mut desktop, &mut fixture, vec![]);
    }
    let output = frame(&ctx, &mut desktop, &mut fixture, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut fixture,
        text_rect(&output, "Save", egui::Rect::EVERYTHING).center(),
    );
    assert!(desktop.preferences.is_none());
    assert!(!fixture.preferences().layer_properties_in_sidebar);
    assert_eq!(fixture.preferences().field_label_min_width, 120.0);
    assert_eq!(fixture.preferences().field_label_width_percent, 25.0);
    assert!(!fixture.preferences().abbreviate_field_labels);
}

#[test]
fn field_label_preferences_default_round_trip_and_reject_invalid_budgets() {
    let old: Preferences = serde_json::from_value(json!({"normalSize": 15.0})).unwrap();
    assert_eq!(old.field_label_min_width, 90.0);
    assert_eq!(old.field_label_width_percent, 15.0);
    assert!(old.abbreviate_field_labels);
    let defaults = serde_json::to_value(&old).unwrap();
    for key in [
        "fieldLabelMinWidth",
        "fieldLabelWidthPercent",
        "abbreviateFieldLabels",
    ] {
        assert!(
            defaults.get(key).is_none(),
            "Default rules do not inflate existing settings or history"
        );
    }
    let custom = Preferences {
        field_label_min_width: 120.0,
        field_label_width_percent: 25.0,
        abbreviate_field_labels: false,
        ..old
    };
    custom.validate().unwrap();
    assert_eq!(
        serde_json::from_value::<Preferences>(serde_json::to_value(&custom).unwrap()).unwrap(),
        custom
    );
    for invalid in [0.0, 241.0, f32::NAN, f32::INFINITY] {
        assert!(
            Preferences {
                field_label_min_width: invalid,
                ..custom.clone()
            }
            .validate()
            .is_err()
        );
    }
    for invalid in [0.0, 51.0, f32::NAN, f32::INFINITY] {
        assert!(
            Preferences {
                field_label_width_percent: invalid,
                ..custom.clone()
            }
            .validate()
            .is_err()
        );
    }
}
