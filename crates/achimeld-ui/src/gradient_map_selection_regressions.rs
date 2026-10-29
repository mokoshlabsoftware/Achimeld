use super::*;

impl MapFixture {
    fn modified_click(&mut self, pos: egui::Pos2, modifiers: egui::Modifiers) {
        self.modifiers = modifiers;
        self.click(pos, egui::PointerButton::Primary);
        self.modifiers = egui::Modifiers::NONE;
        self.frame(vec![]);
    }

    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.pointer(from, egui::PointerButton::Primary, true);
        self.frame(vec![egui::Event::PointerMoved(to)]);
        self.pointer(to, egui::PointerButton::Primary, false);
        self.frame(vec![]);
    }

    fn slice_is(&self, percent: &str) {
        assert!(
            self.labels
                .iter()
                .any(|(s, _)| s == percent.trim_end_matches('%')),
            "Missing slice {percent}: {:?}",
            self.labels
        );
    }
}

#[test]
fn particle_color_map_release_click_guides_slice_and_empty_drag_selects_without_moving_it() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.frame(vec![]);
    f.frame(vec![]);
    let before = f.map.clone();
    let point = f.at(0.4, 0.4);
    f.pointer(point, egui::PointerButton::Primary, true);
    f.slice_is("50.0%");
    f.pointer(point, egui::PointerButton::Primary, false);
    f.frame(vec![]);
    f.slice_is("40.0%");
    f.drag(f.at(0.2, 0.2), f.at(0.8, 0.8));
    f.slice_is("40.0%");
    assert_eq!(
        f.map, before,
        "The marquee and slice helper cannot edit saved colors"
    );
}

#[test]
fn ctrl_drag_on_a_point_sets_slice_without_moving_or_selecting_it() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.frame(vec![]);
    f.frame(vec![]);
    let before = f.map.clone();
    f.modifiers = egui::Modifiers::CTRL;
    f.drag(f.at(1.0 / 3.0, 0.0), f.at(0.8, 0.6));
    f.modifiers = egui::Modifiers::NONE;
    f.frame(vec![]);
    f.slice_is("33.3%");
    assert_eq!(f.map, before);
    assert!(f.selected.is_empty());
}

#[test]
fn multiple_points_offer_unify_color_then_shared_color_edit_and_merge() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.map.points[1].color_hex = "#FFFF0000".into();
    f.frame(vec![]);
    f.frame(vec![]);
    f.click(f.at(0.0, 0.0), egui::PointerButton::Primary);
    f.modified_click(f.at(1.0 / 3.0, 0.0), egui::Modifiers::SHIFT);
    assert!(f.labels.iter().any(|(s, _)| s == "Unify color"));
    assert!(
        !f.labels
            .iter()
            .any(|(s, _)| s == "Power" || s == "Point X" || s == "Point Y")
    );
    let unify = f.labels.iter().find(|(s, _)| s == "Unify color").unwrap().1;
    f.click(unify, egui::PointerButton::Primary);
    assert_eq!(f.map.points[0].color_hex, f.map.points[1].color_hex);
    assert!(f.labels.iter().any(|(s, _)| s == "Point color"));
    f.key(egui::Key::Escape);
    let merge = f.labels.iter().find(|(s, _)| s == "Merge").unwrap().1;
    f.click(merge, egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), EDGE_POINTS - 1);
    assert!(f.labels.iter().any(|(s, _)| s == "Point settings"));
    assert!(f.labels.iter().any(|(s, _)| s == "Power"));
    f.map.validate().unwrap();
}

#[test]
fn merging_the_last_two_points_keeps_the_selection_panel_valid() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.map.points.truncate(2);
    f.frame(vec![]);
    f.frame(vec![]);
    f.click(f.at(0.0, 0.0), egui::PointerButton::Primary);
    f.modified_click(f.at(1.0 / 3.0, 0.0), egui::Modifiers::SHIFT);
    let merge = f.labels.iter().find(|(s, _)| s == "Merge").unwrap().1;
    f.click(merge, egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), 1);
    assert!(f.labels.iter().any(|(s, _)| s == "Point settings"));
    f.map.validate().unwrap();
}

#[test]
fn particle_color_map_marquee_shift_alt_and_group_drag_work_at_multiple_scales() {
    for (scale, width, font) in [(1.0, 420.0, 13.0), (1.5, 280.0, 18.0), (2.0, 240.0, 24.0)] {
        let mut f = MapFixture::new(scale, width);
        f.font = font;
        f.map.points.extend(
            [(0.25, 0.3), (0.45, 0.55), (0.75, 0.6)]
                .into_iter()
                .map(|(x, y)| ColorPoint {
                    x,
                    y,
                    ..Default::default()
                }),
        );
        f.frame(vec![]);
        f.frame(vec![]);
        f.drag(f.at(0.15, 0.15), f.at(0.55, 0.7));
        assert_eq!(f.selected.len(), 2, "Box selection at {scale}");
        f.drag(f.at(0.25, 0.3), f.at(0.35, 0.4));
        for (i, (x, y)) in [(0.35, 0.4), (0.55, 0.65)].into_iter().enumerate() {
            let p = &f.map.points[EDGE_POINTS + i];
            assert!(
                (p.x - x).abs() < 1e-5 && (p.y - y).abs() < 1e-5,
                "Group drag must preserve point offsets: {p:?}"
            );
        }
        f.slice_is("50.0%");
        f.modified_click(f.at(0.75, 0.6), egui::Modifiers::SHIFT);
        assert_eq!(f.selected.len(), 3);
        f.modified_click(f.at(0.55, 0.65), egui::Modifiers::CTRL);
        f.slice_is("55.0%");
        assert_eq!(f.selected.len(), 3);
        f.modified_click(f.at(0.55, 0.65), egui::Modifiers::ALT);
        assert_eq!(f.selected.len(), 2);
        f.modified_click(f.at(0.75, 0.6), egui::Modifiers::SHIFT);
        assert_eq!(
            f.selected.len(),
            2,
            "Shift adds without toggling an existing selection"
        );
        assert_eq!(
            f.map.points.len(),
            EDGE_POINTS + 3,
            "Alt deselects, without deleting a saved point"
        );
        assert_eq!(
            f.values.len(),
            1,
            "Multi-selection shows only the test slice slider"
        );
        assert!(f.labels.iter().any(|(s, _)| s == "Merge"));
        assert!(
            f.bounds.right() <= f.bounds.left() + width + 1.0,
            "Controls overflow at {scale}: {:?}",
            f.bounds
        );
        f.click(f.delete.unwrap().center(), egui::PointerButton::Primary);
        assert_eq!(f.map.points.len(), EDGE_POINTS + 1);
        assert!(f.selected.is_empty());
        assert!(
            f.map
                .points
                .iter()
                .any(|p| (p.x - 0.55).abs() < 1e-5 && (p.y - 0.65).abs() < 1e-5)
        );
        f.map.validate().unwrap();
    }
}

#[test]
fn particle_color_map_group_motion_clamps_together_and_rejects_collisions_atomically() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.map.points = [(0.1, 0.2), (0.4, 0.6), (0.5, 0.5)]
        .into_iter()
        .map(|(x, y)| ColorPoint {
            x,
            y,
            ..Default::default()
        })
        .collect();
    f.frame(vec![]);
    f.frame(vec![]);
    f.drag(f.at(0.05, 0.1), f.at(0.45, 0.8));
    assert_eq!(f.selected.len(), 2);
    f.drag(f.at(0.1, 0.2), f.at(-0.5, -0.5));
    assert!(f.map.points[0].x.abs() < 1e-5 && f.map.points[0].y.abs() < 1e-5);
    assert!((f.map.points[1].x - 0.3).abs() < 1e-5 && (f.map.points[1].y - 0.4).abs() < 1e-5);
    let before = f.map.clone();
    f.drag(f.at(0.0, 0.0), f.at(0.2, 0.1));
    assert_eq!(
        f.map, before,
        "One collision must not partially move or collapse the selection"
    );
    f.map.validate().unwrap();
    f.pointer(f.at(0.0, 0.0), egui::PointerButton::Primary, true);
    f.map.points.truncate(1);
    let remaining = f.map.clone();
    let moved = f.at(0.2, 0.1);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.pointer(moved, egui::PointerButton::Primary, false);
    assert_eq!(
        f.map, remaining,
        "A replaced point set must invalidate an obsolete group drag"
    );
    f.pointer(f.at(0.0, 0.0), egui::PointerButton::Primary, true);
    f.map.points[0].x = 0.3;
    f.map.points[0].color_hex = "#FFFF0000".into();
    let replacement = f.map.clone();
    let moved = f.at(0.5, 0.5);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.pointer(moved, egui::PointerButton::Primary, false);
    assert_eq!(
        f.map, replacement,
        "Same-length replacements must cancel an obsolete drag too"
    );
}

#[test]
fn particle_color_map_can_delete_every_point_and_create_a_new_one_in_the_empty_map() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.frame(vec![]);
    f.frame(vec![]);
    for point in f.map.points.clone() {
        f.modified_click(f.at(point.x as f32, point.y as f32), egui::Modifiers::SHIFT);
    }
    assert_eq!(f.selected.len(), EDGE_POINTS);
    f.key(egui::Key::Delete);
    assert!(f.map.points.is_empty());
    assert_eq!(f.map.compile().unwrap().sample(0.4, 0.4), [0; 4]);
    f.time += 1.0;
    let point = f.at(0.4, 0.4);
    f.click(point, egui::PointerButton::Primary);
    f.click(point, egui::PointerButton::Primary);
    assert_eq!(f.map.points.len(), 1);
    f.map.validate().unwrap();
}

#[test]
fn particle_color_map_disabled_editor_ignores_drag_selection_and_delete() {
    let mut f = MapFixture::new(1.0, 420.0);
    f.frame(vec![]);
    f.frame(vec![]);
    f.click(f.at(0.0, 0.0), egui::PointerButton::Primary);
    let original = f.map.clone();
    f.enabled = false;
    f.drag(f.at(0.0, 0.0), f.at(0.2, 0.2));
    f.key(egui::Key::Delete);
    assert_eq!(f.map, original);
}
