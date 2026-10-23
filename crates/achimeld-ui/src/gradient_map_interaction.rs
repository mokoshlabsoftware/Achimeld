use super::{ColorField, ColorPoint, GradientMap, MAX_POINTS, encode_color, hit, position, screen};
use egui::{Pos2, Rect};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
enum SelectionMode {
    Replace,
    Add,
    Remove,
}

impl SelectionMode {
    fn from_modifiers(modifiers: egui::Modifiers) -> Self {
        if modifiers.ctrl || modifiers.command {
            Self::Remove
        } else if modifiers.shift {
            Self::Add
        } else {
            Self::Replace
        }
    }
}

#[derive(Clone)]
enum Gesture {
    Move {
        origin: Pos2,
        points: Vec<(usize, [f64; 2])>,
        expected: Vec<ColorPoint>,
    },
    Box {
        origin: Pos2,
        base: BTreeSet<usize>,
        mode: SelectionMode,
        rect: Option<Rect>,
    },
    Ignore,
}

#[derive(Clone)]
pub(super) struct State {
    pub selected: BTreeSet<usize>,
    pub active: Option<usize>,
    gesture: Option<Gesture>,
    pub slice: f64,
    pub color: bool,
    pub picker: Option<(usize, [u8; 4], egui::ecolor::Hsva)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            selected: BTreeSet::new(),
            active: None,
            gesture: None,
            slice: 0.5,
            color: false,
            picker: None,
        }
    }
}

impl State {
    pub fn reset_gesture(&mut self) {
        self.gesture = None;
        self.color = false;
        self.picker = None;
    }

    pub fn retain(&mut self, count: usize) {
        self.selected.retain(|&i| i < count);
        self.active = self
            .active
            .filter(|i| self.selected.contains(i))
            .or_else(|| self.selected.last().copied());
    }

    pub fn snapshots(&self, map: &GradientMap) -> Vec<(usize, [f64; 2])> {
        self.selected
            .iter()
            .map(|&i| (i, [map.points[i].x, map.points[i].y]))
            .collect()
    }

    pub fn moving(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Move { .. }))
    }

    pub fn marquee(&self) -> Option<Rect> {
        match self.gesture {
            Some(Gesture::Box { rect, .. }) => rect,
            _ => None,
        }
    }

    pub fn delete(&mut self, map: &mut GradientMap) {
        let mut index = 0;
        map.points.retain(|_| {
            let keep = !self.selected.contains(&index);
            index += 1;
            keep
        });
        self.selected.clear();
        self.active = None;
        self.reset_gesture();
    }

    fn point_selection(&mut self, index: usize, mode: SelectionMode) {
        match mode {
            SelectionMode::Remove => {
                self.selected.remove(&index);
            }
            SelectionMode::Add => {
                self.selected.insert(index);
            }
            SelectionMode::Replace => {
                if !self.selected.contains(&index) {
                    self.selected.clear();
                    self.selected.insert(index);
                }
            }
        }
        self.active = self
            .selected
            .contains(&index)
            .then_some(index)
            .or_else(|| self.selected.last().copied());
        self.color = false;
        self.picker = None;
    }

    fn press(&mut self, map: &GradientMap, rect: Rect, origin: Pos2, mode: SelectionMode) {
        self.gesture = Some(if let Some(index) = hit(map, rect, origin) {
            self.point_selection(index, mode);
            if matches!(mode, SelectionMode::Remove) {
                Gesture::Ignore
            } else {
                Gesture::Move {
                    origin,
                    points: self.snapshots(map),
                    expected: map.points.clone(),
                }
            }
        } else {
            Gesture::Box {
                origin,
                base: self.selected.clone(),
                mode,
                rect: None,
            }
        });
    }
}

/// Translate the complete selection atomically, preserving spacing at bounds and collisions.
pub(super) fn translate(map: &mut GradientMap, points: &[(usize, [f64; 2])], delta: [f64; 2]) {
    if points.is_empty()
        || !delta.iter().all(|v| v.is_finite())
        || points.iter().any(|(index, _)| *index >= map.points.len())
    {
        return;
    }
    let delta: [f64; 2] = std::array::from_fn(|axis| {
        let low = points
            .iter()
            .map(|(_, p)| -p[axis])
            .fold(-1.0_f64, f64::max);
        let high = points
            .iter()
            .map(|(_, p)| 1.0 - p[axis])
            .fold(1.0_f64, f64::min);
        delta[axis].clamp(low, high)
    });
    let mut positions: Vec<_> = map.points.iter().map(|p| [p.x, p.y]).collect();
    for &(index, p) in points {
        positions[index] = [
            (p[0] + delta[0]).clamp(0.0, 1.0),
            (p[1] + delta[1]).clamp(0.0, 1.0),
        ];
    }
    for (i, p) in positions.iter().enumerate() {
        if positions[..i]
            .iter()
            .any(|q| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) < 1e-8)
        {
            return;
        }
    }
    for (point, p) in map.points.iter_mut().zip(positions) {
        point.x = p[0];
        point.y = p[1];
    }
}

pub(super) fn handle(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: Rect,
    map: &mut GradientMap,
    field: &ColorField,
    state: &mut State,
) {
    if !response.enabled() {
        state.gesture = None;
        return;
    }
    let primary = egui::PointerButton::Primary;
    let mode = SelectionMode::from_modifiers(ui.input(|i| i.modifiers));
    if let Some(Gesture::Move { expected, .. }) = &state.gesture
        && expected != &map.points
    {
        state.gesture = Some(Gesture::Ignore);
    }
    if ((response.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_pressed(primary)))
        || (state.gesture.is_none() && response.drag_started_by(primary)))
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        state.press(map, rect, origin, mode);
        response.request_focus();
    }
    if (response.dragged_by(primary) || response.drag_stopped_by(primary))
        && let Some(pointer) = response.interact_pointer_pos()
    {
        match &mut state.gesture {
            Some(Gesture::Move {
                origin,
                points,
                expected,
            }) => {
                let delta = pointer - *origin;
                translate(
                    map,
                    points,
                    [
                        f64::from(delta.x / rect.width()),
                        f64::from(delta.y / rect.height()),
                    ],
                );
                *expected = map.points.clone();
            }
            Some(Gesture::Box {
                origin,
                base,
                mode,
                rect: selection,
            }) => {
                let area = Rect::from_two_pos(*origin, pointer);
                *selection = Some(area.intersect(rect));
                state.selected = if matches!(mode, SelectionMode::Replace) {
                    BTreeSet::new()
                } else {
                    base.clone()
                };
                for (i, point) in map.points.iter().enumerate() {
                    if area.contains(screen(rect, point)) {
                        if matches!(mode, SelectionMode::Remove) {
                            state.selected.remove(&i);
                        } else {
                            state.selected.insert(i);
                        }
                    }
                }
                state.active = state
                    .active
                    .filter(|i| state.selected.contains(i))
                    .or_else(|| state.selected.last().copied());
                state.color = false;
            }
            _ => {}
        }
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        let gesture = state.gesture.take();
        let ignore = matches!(gesture.as_ref(), Some(Gesture::Ignore));
        if !ignore
            && response.double_clicked_by(primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && hit(map, rect, pointer).is_none()
            && map.points.len() < MAX_POINTS
        {
            let [x, y] = position(rect, pointer);
            if !map
                .points
                .iter()
                .any(|p| (p.x - x).powi(2) + (p.y - y).powi(2) < 1e-8)
            {
                let index = map.points.len();
                map.points.push(ColorPoint {
                    x,
                    y,
                    color_hex: encode_color(field.sample(x, y)),
                    ..Default::default()
                });
                state.selected.clear();
                state.point_selection(index, SelectionMode::Replace);
            }
        } else if !ignore
            && response.clicked_by(primary)
            && let Some(pointer) = response.interact_pointer_pos()
        {
            if let Some(index) = hit(map, rect, pointer) {
                if gesture.is_none() {
                    state.point_selection(index, mode);
                }
            } else if matches!(mode, SelectionMode::Replace) {
                state.slice = position(rect, pointer)[0];
                state.selected.clear();
                state.active = None;
                state.color = false;
            }
        }
    }
    if response.has_focus() {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
            state.delete(map);
        } else {
            let (dx, dy, step) = ui.input(|i| {
                (
                    i32::from(i.key_pressed(egui::Key::ArrowRight))
                        - i32::from(i.key_pressed(egui::Key::ArrowLeft)),
                    i32::from(i.key_pressed(egui::Key::ArrowDown))
                        - i32::from(i.key_pressed(egui::Key::ArrowUp)),
                    if i.modifiers.shift { 0.05 } else { 0.005 },
                )
            });
            if dx != 0 || dy != 0 {
                translate(
                    map,
                    &state.snapshots(map),
                    [f64::from(dx) * step, f64::from(dy) * step],
                );
            }
        }
    }
}
