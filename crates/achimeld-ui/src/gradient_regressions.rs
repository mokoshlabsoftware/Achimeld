use super::*;

struct GradientFixture {
    ctx: egui::Context,
    id: egui::Id,
    gradient: Value,
    width: f32,
    font: f32,
    time: f64,
    labels: Vec<(String, egui::Pos2)>,
}

impl GradientFixture {
    fn new(gradient: Value) -> Self {
        Self {
            ctx: egui::Context::default(),
            id: egui::Id::NULL,
            gradient,
            width: 320.0,
            font: 13.0,
            time: 0.0,
            labels: Vec::new(),
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::Rect {
        fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => result.push((
                    t.galley.text().to_owned(),
                    t.pos + t.galley.rect.center().to_vec2(),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, result);
                    }
                }
                _ => {}
            }
        }
        self.time += 0.02;
        let mut bounds = egui::Rect::NOTHING;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 800.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                let preferences = Preferences {
                    normal_size: self.font,
                    ..Default::default()
                };
                theme::apply(ui.ctx(), &preferences);
                ui.set_style(ui.ctx().global_style());
                ui.set_width(self.width);
                self.id = ui.make_persistent_id("gradient-editor");
                let start = ui.cursor().min;
                crate::layers::gradient_stops_ui(ui, &mut self.gradient);
                bounds = egui::Rect::from_min_max(start, ui.min_rect().max);
            },
        );
        self.labels.clear();
        for shape in &output.shapes {
            labels(&shape.shape, &mut self.labels);
        }
        output.textures_delta.clear();
        bounds
    }

    fn control(&self, name: &str) -> egui::Response {
        self.ctx.read_response(self.id.with(name)).unwrap()
    }

    fn ramp(&self) -> egui::Rect {
        let rect = self.control("ramp").rect;
        egui::Rect::from_min_max(
            rect.min + egui::Vec2::splat(8.0),
            egui::pos2(rect.right() - 8.0, rect.top() + 48.0),
        )
    }

    fn handle(&self, index: usize) -> egui::Pos2 {
        let ramp = self.ramp();
        egui::pos2(
            egui::lerp(
                ramp.x_range(),
                self.gradient["stops"][index]["offset"].as_f64().unwrap() as f32,
            ),
            ramp.bottom() + 16.0,
        )
    }

    fn pointer(&mut self, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }

    fn click(&mut self, pos: egui::Pos2, button: egui::PointerButton) {
        self.pointer(pos, button, true);
        self.pointer(pos, button, false);
        self.frame(vec![]);
    }

    fn label(&self, text: &str) -> egui::Pos2 {
        self.labels
            .iter()
            .find(|(s, _)| s == text)
            .unwrap_or_else(|| panic!("Missing {text}: {:?}", self.labels))
            .1
    }
}

fn black_white() -> Value {
    json!({"kind":"Radial","angle":47.123456789,"inputChannel":"Green","stops":[
        {"offset":0.0,"colorHex":"#00000000"}, {"offset":1.0,"colorHex":"#FFFFFFFF"}
    ]})
}

#[test]
fn gradient_reverse_icon_mirrors_all_stops_and_preserves_colors_opacity_and_other_settings() {
    for (width, font) in [(180.0, 13.0), (320.0, 22.0)] {
        let mut value = black_white();
        value["stops"] = json!([{"offset":0.0,"colorHex":"#30FF0000"},{"offset":0.25,"colorHex":"#A000FF00"},{"offset":1.0,"colorHex":"#FF0000FF"}]);
        let mut fixture = GradientFixture::new(value.clone());
        fixture.width = width;
        fixture.font = font;
        fixture.frame(vec![]);
        fixture.frame(vec![]);
        let flip = fixture.control("flip").rect.center();
        fixture.click(flip, egui::PointerButton::Primary);
        assert_eq!(
            fixture.gradient["stops"],
            json!([{"offset":0.0,"colorHex":"#FF0000FF"},{"offset":0.75,"colorHex":"#A000FF00"},{"offset":1.0,"colorHex":"#30FF0000"}])
        );
        for key in ["kind", "angle", "inputChannel"] {
            assert_eq!(fixture.gradient[key], value[key]);
        }
        fixture.click(
            fixture.control("flip").rect.center(),
            egui::PointerButton::Primary,
        );
        assert_eq!(fixture.gradient, value);
    }
}

#[test]
fn gradient_editor_adds_sampled_colors_and_preserves_recipe_fields() {
    let mut fixture = GradientFixture::new(black_white());
    fixture.frame(vec![]);
    fixture.frame(vec![]);
    let original = fixture.gradient.clone();
    assert_eq!(original, black_white());
    let ramp = fixture.ramp();
    let pointer = egui::pos2(egui::lerp(ramp.x_range(), 0.75), ramp.center().y);
    fixture.click(pointer, egui::PointerButton::Secondary);
    fixture.click(
        fixture.label("Create color stop"),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        fixture.gradient["stops"][2],
        json!({"offset":0.75,"colorHex":"#BFBFBFBF"})
    );
    assert_eq!(fixture.gradient["kind"], original["kind"]);
    assert_eq!(fixture.gradient["angle"], original["angle"]);
    assert_eq!(fixture.gradient["inputChannel"], original["inputChannel"]);
    fixture.click(
        fixture.control("add-stop").rect.center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        fixture.gradient["stops"][3],
        json!({"offset":0.5,"colorHex":"#7F7F7F7F"})
    );
}

#[test]
fn gradient_editor_double_click_creates_one_sampled_stop_at_each_display_scale() {
    for scale in [1.0, 1.5, 2.0] {
        let mut fixture = GradientFixture::new(black_white());
        fixture.ctx.set_pixels_per_point(scale);
        fixture.frame(vec![]);
        fixture.frame(vec![]);
        let ramp = fixture.ramp();
        let pointer = egui::pos2(egui::lerp(ramp.x_range(), 0.75), ramp.center().y);
        fixture.click(pointer, egui::PointerButton::Primary);
        assert_eq!(
            fixture.gradient,
            black_white(),
            "A single click must not create a stop"
        );
        fixture.click(pointer, egui::PointerButton::Primary);
        let mut expected = black_white();
        expected["stops"]
            .as_array_mut()
            .unwrap()
            .push(json!({"offset":0.75,"colorHex":"#BFBFBFBF"}));
        assert_eq!(fixture.gradient, expected);
        assert!(!fixture.labels.iter().any(|(s, _)| s == "Color stop"));
        let handle = fixture.handle(2);
        fixture.click(handle, egui::PointerButton::Primary);
        fixture.click(handle, egui::PointerButton::Primary);
        assert_eq!(
            fixture.gradient, expected,
            "Existing handles must not create duplicate stops"
        );
    }
}

#[test]
fn gradient_editor_drags_across_other_stops_and_clamps_without_reordering() {
    let mut fixture = GradientFixture::new(black_white());
    fixture.frame(vec![]);
    fixture.frame(vec![]);
    let press = fixture.handle(0) + egui::vec2(2.0, 0.0);
    fixture.pointer(press, egui::PointerButton::Primary, true);
    let ramp = fixture.ramp();
    let moved = egui::pos2(egui::lerp(ramp.x_range(), 0.65) + 2.0, press.y);
    fixture.frame(vec![egui::Event::PointerMoved(moved)]);
    assert!((fixture.gradient["stops"][0]["offset"].as_f64().unwrap() - 0.65).abs() < 0.00001);
    let outside = egui::pos2(ramp.right() + 90.0, press.y);
    fixture.frame(vec![egui::Event::PointerMoved(outside)]);
    fixture.pointer(outside, egui::PointerButton::Primary, false);
    assert_eq!(fixture.gradient["stops"][0]["offset"], 1.0);
    assert_eq!(fixture.gradient["stops"][0]["colorHex"], "#00000000");
    assert_eq!(fixture.gradient["stops"][1]["colorHex"], "#FFFFFFFF");
    assert!(!fixture.labels.iter().any(|(s, _)| s == "Color stop"));
    let press = fixture.handle(0);
    fixture.pointer(press, egui::PointerButton::Primary, true);
    let moved = egui::pos2(ramp.left() - 100.0, press.y);
    fixture.frame(vec![egui::Event::PointerMoved(moved)]);
    fixture.pointer(moved, egui::PointerButton::Primary, false);
    assert_eq!(fixture.gradient["stops"][0]["offset"], 0.0);
}

#[test]
fn gradient_editor_opens_visual_color_picker_and_protects_the_last_stop() {
    let mut fixture = GradientFixture::new(black_white());
    fixture.frame(vec![]);
    fixture.frame(vec![]);
    fixture.click(fixture.handle(1), egui::PointerButton::Primary);
    fixture.label("Color stop");
    fixture.label("Hex");
    assert_eq!(
        fixture.gradient,
        black_white(),
        "Selecting a stop must not rewrite its colors"
    );
    let red = fixture
        .labels
        .iter()
        .find(|(text, _)| text == "Red")
        .expect("Visual red control")
        .1
        + egui::vec2(150.0, 0.0);
    fixture.pointer(red, egui::PointerButton::Primary, true);
    let moved = red - egui::vec2(40.0, 0.0);
    fixture.frame(vec![egui::Event::PointerMoved(moved)]);
    fixture.pointer(moved, egui::PointerButton::Primary, false);
    assert_ne!(fixture.gradient["stops"][1]["colorHex"], "#FFFFFFFF");
    let changed = fixture.gradient["stops"][1]["colorHex"].as_str().unwrap();
    assert!(
        changed.starts_with("#FF") && changed.ends_with("FFFF"),
        "Alpha and other channels must survive color editing: {changed}"
    );
    fixture.click(
        fixture.control("delete-stop").rect.center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(fixture.gradient["stops"].as_array().unwrap().len(), 1);
    assert!(!egui::Popup::is_id_open(
        &fixture.ctx,
        fixture.id.with("color")
    ));
    fixture.click(
        fixture.control("delete-stop").rect.center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(fixture.gradient["stops"].as_array().unwrap().len(), 1);
    fixture.click(fixture.handle(0), egui::PointerButton::Secondary);
    fixture.label("Delete color stop");
    fixture.click(
        fixture.label("Delete color stop"),
        egui::PointerButton::Primary,
    );
    assert_eq!(fixture.gradient["stops"].as_array().unwrap().len(), 1);
}

#[test]
fn gradient_editor_keeps_sparse_data_and_compact_bar_geometry_at_font_and_display_scales() {
    for width in [200.0, 280.0, 400.0] {
        for font in [13.0, 22.0] {
            for scale in [1.0, 1.5, 2.0] {
                let mut fixture = GradientFixture::new(json!({"inputChannel":"Blue"}));
                fixture.width = width;
                fixture.font = font;
                fixture.ctx.set_pixels_per_point(scale);
                fixture.frame(vec![]);
                fixture.frame(vec![]);
                assert_eq!(fixture.gradient, json!({"inputChannel":"Blue"}));
                let ramp = fixture.control("ramp").rect;
                let add = fixture.control("add-stop").rect;
                let delete = fixture.control("delete-stop").rect;
                assert!(
                    (ramp.width() - width).abs() < 1.0,
                    "{width}, {font}, {scale}: {ramp:?}"
                );
                assert!((add.top() - delete.top()).abs() < 0.1);
                assert!((add.bottom() - delete.bottom()).abs() < 0.1);
                assert!((add.right() - delete.left()).abs() < 0.1);
                assert!(
                    delete.right() <= ramp.right() + 1.0,
                    "{width}, {font}, {scale}: {add:?} {delete:?}"
                );
                assert!(!fixture.labels.iter().any(|(s, _)| s.contains('#')));
            }
        }
    }
}

#[test]
fn gradient_editor_enforces_the_existing_stop_limit() {
    let mut value = black_white();
    value["stops"] = json!(
        (0..128)
            .map(|i| json!({"offset":f64::from(i)/127.0,"colorHex":"#FFFFFFFF"}))
            .collect::<Vec<_>>()
    );
    let mut fixture = GradientFixture::new(value);
    fixture.frame(vec![]);
    fixture.frame(vec![]);
    fixture.click(
        fixture.control("add-stop").rect.center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(fixture.gradient["stops"].as_array().unwrap().len(), 128);
    let pointer = fixture.ramp().center();
    fixture.click(pointer, egui::PointerButton::Primary);
    fixture.click(pointer, egui::PointerButton::Primary);
    assert_eq!(fixture.gradient["stops"].as_array().unwrap().len(), 128);
}

#[test]
fn gradient_editor_live_layers_effects_and_vignette_drags_undo_as_one_edit() {
    for kind in ["Gradient", "GradientOverlay", "Effect", "SdfVignette"] {
        let mut fixture = Fixture::combined_layers();
        let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        let achievement = entity["id"].as_str().unwrap().to_owned();
        let layer = new_id();
        let effect = new_id();
        let mut value = json!({"id":layer,"displayName":"Gradient editing","kind":kind,"visible":true,"opacity":1.0});
        if kind == "Effect" {
            value["kind"] = json!("Solid");
            value["solidColorHex"] = json!("#FFFFFFFF");
            value["effects"] = json!([{"id":effect,"kind":"GradientOverlay","visible":true,"gradient":black_white()}]);
        } else if kind == "SdfVignette" {
            value["sdfVignette"] = json!({"useGradient":true,"gradient":black_white()});
        } else {
            value["gradient"] = black_white();
        }
        entity["icon"] = json!({"layers":[value]});
        fixture
            .execute(
                "Prepare gradient",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        fixture.finish_group();
        let initial = fixture.editor.document().clone();
        let mut desktop = Desktop::default();
        desktop.selection = vec![achievement.clone()];
        desktop.selected_layer = Some(layer);
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
            fn ramp(shape: &egui::Shape) -> Option<egui::Rect> {
                match shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::default() => {
                        let rect = mesh.calc_bounds();
                        ((rect.height() - 40.0).abs() < 0.1 && rect.width() > 100.0).then_some(rect)
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(ramp),
                    _ => None,
                }
            }
            time += 0.02;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 1000.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), fixture.preferences());
                    ui.set_style(ui.ctx().global_style());
                    desktop.layer_editor(ui, fixture);
                },
            );
            let rect = output.shapes.iter().find_map(|shape| ramp(&shape.shape));
            output.textures_delta.clear();
            rect
        };
        frame(&mut desktop, &mut fixture, vec![]);
        if kind == "Effect" {
            desktop.layer_palette.selected_effect = Some(effect);
        }
        let ramp = frame(&mut desktop, &mut fixture, vec![])
            .unwrap_or_else(|| panic!("Missing live {kind} gradient strip"));
        assert_eq!(
            fixture.editor.document(),
            &initial,
            "Opening {kind} changed sparse data"
        );
        let press = egui::pos2(ramp.left(), ramp.bottom() + 16.0);
        frame(
            &mut desktop,
            &mut fixture,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for offset in [0.3, 0.7] {
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(egui::pos2(
                    egui::lerp(ramp.x_range(), offset),
                    press.y,
                ))],
            );
        }
        let release = egui::pos2(egui::lerp(ramp.x_range(), 0.7), press.y);
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerButton {
                pos: release,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let edited = fixture.editor.document().clone();
        assert_ne!(edited, initial, "{kind} did not update the live recipe");
        assert!(
            desktop.entity_drafts.is_empty(),
            "{kind} produced an invalid recipe"
        );
        fixture.undo();
        assert_eq!(
            fixture.editor.document(),
            &initial,
            "{kind} drag was split into multiple undo steps"
        );
        fixture.redo();
        assert_eq!(fixture.editor.document(), &edited);
    }
}
