use super::*;
use achimeld_application::gradient_map::{ColorPoint, EDGE_POINTS, GradientMap};
#[path = "gradient_map_capture.rs"]
mod capture;
#[path = "gradient_map_selection_regressions.rs"]
mod selection;

struct MapFixture {
    ctx: egui::Context,
    map: GradientMap,
    rect: egui::Rect,
    labels: Vec<(String, egui::Pos2)>,
    circles: usize,
    bounds: egui::Rect,
    width: f32,
    font: f32,
    time: f64,
    swatch: Option<egui::Rect>,
    delete: Option<egui::Rect>,
    selected: Vec<egui::Pos2>,
    values: Vec<egui::Rect>,
    modifiers: egui::Modifiers,
    enabled: bool,
}
impl MapFixture {
    fn new(scale: f32, width: f32) -> Self {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        Self {
            ctx,
            map: Default::default(),
            rect: egui::Rect::NOTHING,
            labels: Vec::new(),
            circles: 0,
            bounds: egui::Rect::NOTHING,
            width,
            font: 13.0,
            time: 0.0,
            swatch: None,
            delete: None,
            selected: Vec::new(),
            values: Vec::new(),
            modifiers: egui::Modifiers::NONE,
            enabled: true,
        }
    }
    fn frame(&mut self, mut events: Vec<egui::Event>) {
        self.time += 0.03;
        events.insert(0, egui::Event::ModifiersChanged(self.modifiers));
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 1000.0),
                )),
                events,
                time: Some(self.time),
                ..Default::default()
            },
            |ui| {
                theme::apply(
                    ui.ctx(),
                    &Preferences {
                        normal_size: self.font,
                        ..Default::default()
                    },
                );
                ui.set_style(ui.ctx().global_style());
                ui.set_width(self.width);
                let mut editor_id = egui::Id::NULL;
                self.rect = ui
                    .add_enabled_ui(self.enabled, |ui| {
                        editor_id = ui.make_persistent_id("2d-gradient-editor");
                        crate::gradient_map::editor(ui, &mut self.map)
                    })
                    .inner
                    .rect
                    .shrink(5.0);
                self.bounds = ui.min_rect();
                self.swatch = ui
                    .ctx()
                    .read_response(editor_id.with("point-color-swatch"))
                    .map(|r| r.rect);
                self.delete = ui
                    .ctx()
                    .read_response(editor_id.with("delete-points"))
                    .map(|r| r.rect);
                self.values = ["Test slice", "Power", "X influence", "Y influence"]
                    .iter()
                    .filter_map(|label| {
                        ui.data(|data| {
                            data.get_temp::<egui::Rect>(editor_id.with(("value", *label)))
                        })
                    })
                    .collect();
            },
        );
        self.labels.clear();
        self.circles = 0;
        self.selected.clear();
        fn collect(
            shape: &egui::Shape,
            labels: &mut Vec<(String, egui::Pos2)>,
            circles: &mut usize,
            selected: &mut Vec<egui::Pos2>,
            scale: f32,
        ) {
            match shape {
                egui::Shape::Text(t) => labels.push((
                    t.galley.text().to_owned(),
                    t.pos + t.galley.rect.center().to_vec2(),
                )),
                egui::Shape::Vec(v) => {
                    for s in v {
                        collect(s, labels, circles, selected, scale);
                    }
                }
                egui::Shape::Circle(circle) if (circle.radius * scale - 3.0).abs() < 0.01 => {
                    *circles += 1
                }
                egui::Shape::Circle(circle) if (circle.radius * scale - 7.0).abs() < 0.01 => {
                    selected.push(circle.center)
                }
                _ => {}
            }
        }
        for s in &output.shapes {
            collect(
                &s.shape,
                &mut self.labels,
                &mut self.circles,
                &mut self.selected,
                self.ctx.pixels_per_point(),
            );
        }
        output.textures_delta.clear();
    }
    fn at(&self, x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(
            egui::lerp(self.rect.x_range(), x),
            egui::lerp(self.rect.y_range(), y),
        )
    }
    fn pointer(&mut self, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: self.modifiers,
            },
        ]);
    }
    fn click(&mut self, pos: egui::Pos2, button: egui::PointerButton) {
        self.pointer(pos, button, true);
        self.pointer(pos, button, false);
        self.frame(vec![]);
    }
    fn key(&mut self, key: egui::Key) {
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}

#[test]
fn particle_color_map_selected_swatch_opens_the_picker_without_a_color_label_button() {
    for (scale, width) in [(1.0, 400.0), (1.5, 280.0), (2.0, 240.0)] {
        let mut f = MapFixture::new(scale, width);
        f.frame(vec![]);
        f.frame(vec![]);
        f.click(f.at(0.0, 0.0), egui::PointerButton::Primary);
        let original = f.map.clone();
        assert!(!f.labels.iter().any(|(text, _)| text == "Color…"));
        let swatch = f
            .swatch
            .expect("Selected point needs a clickable color swatch");
        f.click(swatch.center(), egui::PointerButton::Primary);
        assert!(f.labels.iter().any(|(text, _)| text == "Point color"));
        assert_eq!(
            f.map, original,
            "Opening the picker must preserve the selected anchor"
        );
        f.key(egui::Key::Escape);
        assert!(!f.labels.iter().any(|(text, _)| text == "Point color"));
    }
}

#[test]
fn particle_color_map_hover_slice_helper_and_gray_contrast() {
    let mut f = MapFixture::new(1.0, 400.0);
    f.frame(vec![]);
    f.frame(vec![]);
    let original = f.map.clone();
    assert_eq!(f.circles, 0, "Vertices must be hidden away from the map");
    f.frame(vec![egui::Event::PointerMoved(f.at(0.4, 0.4))]);
    assert_eq!(f.circles, EDGE_POINTS);
    f.click(f.at(0.4, 0.3), egui::PointerButton::Primary);
    assert!(
        f.labels.iter().any(|(s, _)| s == "40.0" || s == "40"),
        "The slice position must follow X, not Y: {:?}",
        f.labels
    );
    assert_eq!(
        f.map, original,
        "The test slicer must not modify the saved gradient or random particle choices"
    );
    f.frame(vec![egui::Event::PointerGone]);
    assert_eq!(f.circles, 0);
    assert!(
        f.labels
            .iter()
            .any(|(s, _)| crate::fields::label_matches(s, "T. Slice"))
    );
    assert!(!f.labels.iter().any(|(s, _)| s.contains('#')));
    let ink = crate::gradient_map::point_ink([128, 128, 128, 255]);
    assert!(ink.r() > 225 && ink.g() > 225 && ink.b() > 225);
    assert_eq!(
        crate::gradient_map::point_ink([255; 4]),
        egui::Color32::BLACK
    );
    assert_eq!(
        crate::gradient_map::point_ink([0, 0, 0, 255]),
        egui::Color32::WHITE
    );
}

#[test]
fn particle_color_map_double_click_adds_one_sampled_point_and_respects_anchors_and_limit() {
    for scale in [1.0, 1.5, 2.0] {
        let mut f = MapFixture::new(scale, 400.0);
        f.frame(vec![]);
        f.frame(vec![]);
        let original = f.map.clone();
        let field = original.compile().unwrap();
        let pointer = f.at(0.4, 0.6);
        f.click(pointer, egui::PointerButton::Primary);
        assert_eq!(f.map, original, "A single click must remain a test slice");
        f.click(pointer, egui::PointerButton::Primary);
        assert_eq!(f.map.points.len(), EDGE_POINTS + 1);
        let point = &f.map.points[EDGE_POINTS];
        assert!((point.x - 0.4).abs() < 1e-5 && (point.y - 0.6).abs() < 1e-5);
        assert_eq!(
            point.color_hex,
            achimeld_application::gradient_map::encode_color(field.sample(point.x, point.y))
        );
        assert_eq!(&f.map.points[..EDGE_POINTS], original.points.as_slice());
        assert!(f.swatch.is_some());
        f.map.validate().unwrap();
        f.key(egui::Key::Escape);
        f.click(pointer, egui::PointerButton::Primary);
        f.click(pointer, egui::PointerButton::Primary);
        assert_eq!(
            f.map.points.len(),
            EDGE_POINTS + 1,
            "Existing points must not be duplicated"
        );
    }
    let mut f = MapFixture::new(1.0, 400.0);
    f.map.points.extend((0..52).map(|i| ColorPoint {
        x: f64::from(i % 13 + 1) / 14.0,
        y: f64::from(i / 13 + 1) / 5.0,
        ..Default::default()
    }));
    f.map.validate().unwrap();
    f.frame(vec![]);
    f.frame(vec![]);
    let original = f.map.clone();
    let pointer = f.at(0.5, 0.05);
    f.click(pointer, egui::PointerButton::Primary);
    f.click(pointer, egui::PointerButton::Primary);
    assert_eq!(
        f.map, original,
        "Double-clicking must retain the point limit"
    );
}

#[test]
fn particle_color_map_has_context_actions_and_inline_controls() {
    let mut f = MapFixture::new(1.0, 400.0);
    f.frame(vec![]);
    f.frame(vec![]);
    assert!(f.labels.iter().any(|(s, _)| s == "?"));
    assert!(!f.labels.iter().any(|(s, _)| s.contains("Shift-click")));
    f.click(f.at(0.4, 0.6), egui::PointerButton::Secondary);
    assert!(egui::Popup::is_any_open(&f.ctx));
    assert!(f.labels.iter().any(|(s, _)| s == "Clone"));
    assert!(f.labels.iter().any(|(s, _)| s == "Delete"));
    assert!(f.labels.iter().any(|(s, _)| s == "Merge"));
    assert!(
        !f.labels
            .iter()
            .any(|(s, _)| s == "Change color…" || s == "Remove point")
    );
    f.click(f.at(0.0, 0.0), egui::PointerButton::Primary);
    assert!(f.labels.iter().any(|(s, _)| s == "Power"));
    assert!(f.swatch.is_some());
    f.click(f.delete.unwrap().center(), egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), EDGE_POINTS - 1);
    f.map.validate().unwrap();
}

#[test]
fn particle_color_map_context_clone_and_delete_edit_the_selected_point() {
    let mut f = MapFixture::new(1.0, 400.0);
    f.frame(vec![]);
    f.frame(vec![]);
    f.click(f.at(0.0, 0.0), egui::PointerButton::Secondary);
    let command =
        |f: &MapFixture, name: &str| f.labels.iter().find(|(label, _)| label == name).unwrap().1;
    f.click(command(&f, "Clone"), egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), EDGE_POINTS + 1);
    f.map.validate().unwrap();
    f.click(f.at(0.01, 0.01), egui::PointerButton::Secondary);
    f.click(command(&f, "Delete"), egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), EDGE_POINTS);
    f.map.validate().unwrap();
}

#[test]
fn particle_color_map_dragging_and_removal_include_initial_edge_points_at_multiple_scales() {
    for (scale, width, font) in [(1.0, 420.0, 13.0), (1.5, 280.0, 18.0), (2.0, 240.0, 24.0)] {
        let mut f = MapFixture::new(scale, width);
        f.font = font;
        f.map.points.push(ColorPoint {
            x: 0.4,
            y: 0.6,
            ..Default::default()
        });
        f.frame(vec![]);
        f.frame(vec![]);
        let press = f.at(0.4, 0.6);
        f.pointer(press, egui::PointerButton::Primary, true);
        let moved = f.at(0.65, 0.35);
        f.frame(vec![egui::Event::PointerMoved(moved)]);
        f.pointer(moved, egui::PointerButton::Primary, false);
        assert!((f.map.points[EDGE_POINTS].x - 0.65).abs() < 1e-5);
        assert!((f.map.points[EDGE_POINTS].y - 0.35).abs() < 1e-5);
        assert!(
            f.bounds.right() <= f.bounds.left() + f.width + 1.0,
            "Map controls overflow at scale {scale}: {:?}",
            f.bounds
        );
        f.click(f.at(0.65, 0.35), egui::PointerButton::Primary);
        f.key(egui::Key::Delete);
        assert_eq!(f.map.points.len(), EDGE_POINTS);
        let original = f.map.clone();
        f.pointer(f.at(0.0, 0.0), egui::PointerButton::Primary, true);
        f.frame(vec![egui::Event::PointerMoved(f.at(0.2, 0.2))]);
        f.pointer(f.at(0.2, 0.2), egui::PointerButton::Primary, false);
        assert!((f.map.points[0].x - 0.2).abs() < 1e-5 && (f.map.points[0].y - 0.2).abs() < 1e-5);
        f.click(f.at(0.2, 0.2), egui::PointerButton::Primary);
        f.key(egui::Key::Delete);
        assert_eq!(f.map.points.len(), original.points.len() - 1);
        f.map.validate().unwrap();
    }
}
