use super::*;

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn rotary_drag_keys_fine_wheel_reset_and_disabled_controls() {
    let ctx = egui::Context::default();
    let preferences = Preferences::default();
    let parameter = Parameter::new("Brightness", "brightness", 0.0..=2.0, 0);
    let mut value: f64 = 1.0;
    let mut id = egui::Id::NULL;
    let rect = Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::splat(45.0));
    let mut frame = |mut events: Vec<egui::Event>,
                     modifiers: egui::Modifiers,
                     enabled: bool,
                     focused: bool,
                     value: &mut f64| {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                if !enabled {
                    ui.disable();
                }
                if focused {
                    ui.memory_mut(|m| m.request_focus(id));
                }
                let response = rotary(
                    ui,
                    &preferences,
                    rect,
                    "brightness",
                    "Brightness",
                    value,
                    1.0,
                    &parameter,
                    parameter.bounds.clone(),
                );
                id = response.id;
            },
        );
        output.textures_delta.clear();
    };
    frame(vec![], egui::Modifiers::NONE, true, false, &mut value);
    frame(
        vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    assert!((value - 1.02).abs() < 1e-9);
    frame(
        vec![key(egui::Key::ArrowUp, egui::Modifiers::SHIFT)],
        egui::Modifiers::SHIFT,
        true,
        true,
        &mut value,
    );
    assert!((value - 1.022).abs() < 1e-9);
    frame(
        vec![key(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    assert_eq!(value, 2.0);
    frame(
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    assert_eq!(value, 1.0);
    frame(
        vec![key(egui::Key::Home, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        false,
        true,
        &mut value,
    );
    assert_eq!(value, 1.0);
    // The same pointer delta changes the value immediately, with no eased data.
    frame(vec![], egui::Modifiers::NONE, true, false, &mut value);
    let point = rect.center();
    frame(
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    frame(
        vec![egui::Event::PointerMoved(
            rect.center() + Vec2::new(20.0, -20.0),
        )],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    assert!(value > 1.0);
    frame(
        vec![egui::Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    value = 1.0;
    frame(
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    let wheel = || egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, 20.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        vec![egui::Event::PointerMoved(rect.center()), wheel()],
        egui::Modifiers::NONE,
        true,
        true,
        &mut value,
    );
    assert!(value > 1.0);
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let before = value;
    frame(
        vec![wheel()],
        egui::Modifiers::NONE,
        true,
        false,
        &mut value,
    );
    assert_eq!(
        value, before,
        "Unfocused wheel scrolling must not edit a knob"
    );
}

#[test]
fn range_endpoints_are_editable_cannot_cross_and_rendering_preserves_data() {
    let ctx = egui::Context::default();
    let p = Preferences::default();
    let defaults = json!({"low": 3.0, "high": 5.0});
    let mut data = defaults.clone();
    let params = [Parameter::new("Aspect ratio", "low", 1.0..=10.0, 9)
        .pair("high")
        .ratio()];
    let frame = |events: Vec<egui::Event>, focus: Option<egui::Id>, data: &mut Value| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                ui.set_width(320.0);
                if let Some(id) = focus {
                    ui.memory_mut(|m| m.request_focus(id));
                }
                bank(ui, &p, data, &defaults, &params);
            },
        );
        for paint in &output.shapes {
            if let egui::Shape::Text(text) = &paint.shape {
                assert!(!text.galley.text().contains("equal spacing"));
            }
        }
        output.textures_delta.clear();
    };
    frame(vec![], None, &mut data);
    assert_eq!(data, defaults);
    // IDs belong to the tile, independent of row/column placement.
    let mut low_id = None;
    let mut high_id = None;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt("low"));
        low_id = Some(child.id().with(("low", "dial")));
        high_id = Some(child.id().with(("high", "dial")));
        child.set_width(320.0);
    });
    output.textures_delta.clear();
    frame(
        vec![key(egui::Key::End, egui::Modifiers::NONE)],
        low_id,
        &mut data,
    );
    assert_eq!(data["low"], 5.0);
    frame(
        vec![key(egui::Key::Home, egui::Modifiers::NONE)],
        high_id,
        &mut data,
    );
    assert_eq!(data["high"], 5.0);
    frame(
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        low_id,
        &mut data,
    );
    assert_eq!(data["low"], 3.0);
}

#[test]
fn banks_fit_font_sizes_translations_scales_and_restore_no_recipe_fields() {
    for font in [10.0, 24.0] {
        for width in [140.0, 180.0, 320.0, 660.0] {
            for scale in [1.0, 2.0] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(scale);
                localization::language(&ctx, "Polish");
                let p = Preferences {
                    knob_label_size: font,
                    knob_value_size: font,
                    ..Default::default()
                };
                let mut data = json!({"low": -0.4, "high": 1.3, "objects": 512});
                let before = data.clone();
                let mut width_used = 0.0;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(width, 2000.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.set_width(width);
                        bank(
                            ui,
                            &p,
                            &mut data,
                            &before,
                            &[
                                Parameter::new("Bottom height", "low", -1.5..=1.5, 14)
                                    .pair("high")
                                    .percent(),
                                Parameter::new("Objects", "objects", 1.0..=512.0, 1).integer(),
                            ],
                        );
                        width_used = ui.min_rect().width();
                    },
                );
                output.textures_delta.clear();
                assert!(width_used <= width + 0.1, "{width_used} > {width}");
                for paint in &output.shapes {
                    if let egui::Shape::Text(text) = &paint.shape {
                        assert!(
                            text.pos.x + text.galley.size().x <= width + 0.1,
                            "Text overflow: {}",
                            text.galley.text()
                        );
                    }
                }
                output.textures_delta.clear();
                assert_eq!(data, before);
            }
        }
    }
}

#[test]
fn font_preferences_migrate_round_trip_and_reject_nonfinite_sizes() {
    let mut old: Preferences = serde_json::from_value(json!({"normalSize": 15.0})).unwrap();
    assert_eq!(old.knob_label_size, 10.0);
    assert_eq!(old.knob_value_size, 10.0);
    let encoded = serde_json::to_value(&old).unwrap();
    assert!(encoded.get("knobLabelSize").is_none());
    assert!(encoded.get("knobValueSize").is_none());
    old.knob_label_size = 12.0;
    old.knob_value_size = 14.0;
    old.validate().unwrap();
    assert_eq!(
        serde_json::from_value::<Preferences>(serde_json::to_value(&old).unwrap()).unwrap(),
        old
    );
    old.knob_label_size = f32::NAN;
    assert!(old.validate().is_err());
}

#[test]
fn resizing_the_right_panel_reflows_columns_and_preserves_focus_and_values() {
    let ctx = egui::Context::default();
    let p = Preferences::default();
    let mut data = json!({"a": 0.5, "b": 0.4, "c": 0.3, "low": 0.2, "high": 0.8});
    let before = data.clone();
    let params = [
        Parameter::new("Brightness", "a", 0.0..=1.0, 0),
        Parameter::new("Objects", "b", 0.0..=1.0, 1),
        Parameter::new("Scale", "c", 0.0..=1.0, 2),
        Parameter::new("Aspect ratio", "low", 0.0..=1.0, 9).pair("high"),
    ];
    let mut ids = None;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        let child = ui.new_child(egui::UiBuilder::new().id_salt("low"));
        let first = ui.new_child(egui::UiBuilder::new().id_salt("a"));
        ids = Some((
            first.id().with(("a", "dial")),
            child.id().with(("low", "dial")),
        ));
    });
    output.textures_delta.clear();
    let (first, low) = ids.unwrap();
    for (width, wraps) in [(660.0, false), (320.0, true), (180.0, true), (660.0, false)] {
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 2000.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                ui.set_width(width);
                ui.memory_mut(|m| m.request_focus(low));
                bank(ui, &p, &mut data, &before, &params);
            });
            output.textures_delta.clear();
        }
        let a = ctx.read_response(first).unwrap().rect;
        let b = ctx.read_response(low).unwrap().rect;
        if wraps {
            assert!(b.top() > a.top());
        } else {
            assert!(
                b.top() >= a.top() && b.top() - a.top() < 30.0,
                "Panel {width}: first={a:?}, range={b:?}"
            );
        }
        assert_eq!(ctx.memory(|m| m.focused()), Some(low));
        assert_eq!(data, before);
    }
}

#[test]
fn gradient_faces_have_a_one_physical_pixel_antialias_fringe() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        let center = Pos2::new(40.0, 40.0);
        let radius = 15.5;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            face(
                ui,
                center,
                radius,
                Color32::from_gray(30),
                Color32::from_gray(65),
                true,
            );
        });
        output.textures_delta.clear();
        let mesh = output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Mesh(mesh) = &s.shape {
                    Some(mesh)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(mesh.is_valid());
        let outside: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.color == Color32::TRANSPARENT)
            .collect();
        assert!(outside.len() >= 96);
        for vertex in outside {
            assert!(((vertex.pos.distance(center) - radius) * scale - 0.5).abs() < 0.01);
        }
        let inside = mesh
            .vertices
            .iter()
            .filter(|v| v.color.a() > 0)
            .map(|v| v.pos.distance(center))
            .fold(0.0, f32::max);
        assert!(((radius - inside) * scale - 0.5).abs() < 0.01);
    }
}
