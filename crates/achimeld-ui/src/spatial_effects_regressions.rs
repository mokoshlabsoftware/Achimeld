use super::*;

fn text_positions(output: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
    output
        .shapes
        .iter()
        .filter_map(|paint| match &paint.shape {
            egui::Shape::Text(text) => Some((
                text.galley.text().to_owned(),
                text.pos + text.galley.rect.center().to_vec2(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn spatial_effect_menu_adds_each_effect_and_idle_properties_preserve_recipes() {
    for (kind, label, fields) in [
        ("Skew", "Skew", vec!["Horizontal", "Vertical"]),
        ("Perspective", "Perspective", vec!["Horizontal", "Vertical"]),
        ("Pinch", "Pinch", vec!["Amount"]),
        ("LensBlur", "Lens blur", vec!["E. Blur", "C. Aberration"]),
    ] {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut layer = json!({"id":new_id(),"kind":"Solid"});
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::layer_palette::effect_menu(ui, &mut layer);
                },
            );
            output.textures_delta.clear();
            output
        };
        let output = frame(vec![]);
        let pos = text_positions(&output)
            .into_iter()
            .find(|(text, _)| text == label)
            .unwrap()
            .1;
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        let effects = layer["effects"].as_array().unwrap();
        assert_eq!(effects.len(), 1);
        let mut effect = effects[0].clone();
        assert_eq!(effect["kind"], kind);
        assert!(effect["visible"].as_bool().unwrap());
        for width in [180.0, 360.0, 720.0] {
            let original = effect.clone();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    crate::layers::effect_properties(ui, &mut effect);
                },
            );
            output.textures_delta.clear();
            assert_eq!(
                effect, original,
                "Inspecting {kind} at {width} changed its recipe"
            );
            let labels: Vec<_> = text_positions(&output)
                .into_iter()
                .map(|(label, _)| label)
                .collect();
            for field in &fields {
                assert!(
                    labels
                        .iter()
                        .any(|text| crate::fields::label_matches(text, field)),
                    "Missing {field} for {kind} at {width}"
                );
            }
            assert!(
                !labels
                    .iter()
                    .any(|label| label == "Color" || label == "Blend mode")
            );
        }
    }
}
