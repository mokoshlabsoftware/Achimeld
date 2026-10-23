//! Spatial keyboard selection uses project coordinates, independent of zoom/pan.
use crate::{Desktop, canvas::uncovered_canvas};
use achimeld_application::canvas::{GRID_SIZE, auto_position, card_size};
use achimeld_application::{EditorPort, Preferences};
use egui::{Id, Key, Rect, Vec2};
use serde_json::Value;

#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::ArrowLeft => Some(Self::Left),
            Key::ArrowRight => Some(Self::Right),
            Key::ArrowUp => Some(Self::Up),
            Key::ArrowDown => Some(Self::Down),
            _ => None,
        }
    }

    fn rank(self, from: [f64; 2], to: [f64; 2]) -> Option<(f64, f64)> {
        let [dx, dy] = [to[0] - from[0], to[1] - from[1]];
        let (forward, sideways) = match self {
            Self::Left => (-dx, dy.abs()),
            Self::Right => (dx, dy.abs()),
            Self::Up => (-dy, dx.abs()),
            Self::Down => (dy, dx.abs()),
        };
        // A 90-degree cone, including its diagonal edges. Double the lateral
        // distance before measuring proximity: favour the same row/column,
        // without allowing a distant aligned card to win at any distance.
        (forward > 0.0 && sideways <= forward)
            .then_some((forward * forward + 4.0 * sideways * sideways, sideways))
    }
}

struct Node {
    id: String,
    center: [f64; 2],
}

fn destination(
    nodes: &[Node],
    selected: &[String],
    direction: Direction,
    center: [f64; 2],
) -> Option<usize> {
    let anchor = selected
        .iter()
        .find_map(|id| nodes.iter().position(|node| &node.id == id));
    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let rank = if let Some(anchor) = anchor {
                if selected.contains(&node.id) {
                    return None;
                }
                direction.rank(nodes[anchor].center, node.center)?
            } else {
                // Start at the card nearest the visible canvas centre if there is
                // no selection (or the old selection has been filtered/deleted).
                let dx = node.center[0] - center[0];
                let dy = node.center[1] - center[1];
                (dx * dx + dy * dy, 0.0)
            };
            Some((index, rank))
        })
        .min_by(|(a, rank_a), (b, rank_b)| {
            rank_a
                .0
                .total_cmp(&rank_b.0)
                .then_with(|| rank_a.1.total_cmp(&rank_b.1))
                .then_with(|| nodes[*a].id.cmp(&nodes[*b].id))
        })
        .map(|(index, _)| index)
}

impl Desktop {
    pub(crate) fn navigate_nodes(
        &mut self,
        ui: &egui::Ui,
        port: &mut dyn EditorPort,
        p: &Preferences,
        rect: Rect,
        canvas_id: Id,
    ) {
        let ctx = ui.ctx();
        if !self.view.is_editor()
            || !ui.is_enabled()
            || self.about
            || self.settings
            || self.sort_open
            || self.preferences.is_some()
            || self.source_picker.is_some()
            || self.style_rename.is_some()
            || self.error.is_some()
            || self.image_drop.pending()
            || self.art_bank.pending()
            || !self.entity_drafts.is_empty()
            || !self.localization_drafts.is_empty()
            || self.node_drag.is_some()
            || self.selection_drag.is_some()
            || self.camera_drag.is_some()
            || self.layer_drag.is_some()
            || self.layer_palette.keyboard_active
            || egui::Popup::is_any_open(ctx)
            || ctx.memory(|memory| memory.top_modal_layer().is_some())
            || ui.input(|input| {
                !input.focused || input.pointer.any_down() || input.pointer.any_pressed()
            })
        {
            return;
        }
        let entities = port.editor().document().entities(self.kind);
        if let Some(focused) = ctx.memory(|memory| memory.focused())
            && focused != canvas_id
            && !entities.iter().any(|entity| {
                entity["id"].as_str().is_some_and(|id| {
                    focused == Id::new(("node", id)) || focused == Id::new(("icon-layers", id))
                })
            })
        {
            return;
        }
        let directions = ui.input_mut(|input| {
            let mut directions = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                    && !modifiers.any()
                    && let Some(direction) = Direction::from_key(*key)
                {
                    directions.push(direction);
                    false
                } else {
                    true
                }
            });
            directions
        });
        if directions.is_empty() {
            return;
        }
        // egui has already scheduled its own directional widget focus at the
        // start of the pass. Cancel it even at a dead end, keeping arrows on
        // the canvas instead of unexpectedly focusing an inspector field.
        ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        let filter = self.filter.to_lowercase();
        let size = card_size(self.kind);
        let nodes: Vec<_> = entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                let api = entity["apiName"]
                    .as_str()
                    .or(entity.pointer("/apiName/value").and_then(Value::as_str))
                    .unwrap_or("");
                if !filter.is_empty() {
                    let name = port.editor().document().display_name(entity, &p.language);
                    if !format!("{name} {api}").to_lowercase().contains(&filter) {
                        return None;
                    }
                }
                let automatic = auto_position(self.kind, index);
                Some(Node {
                    id: entity["id"].as_str().unwrap_or(api).to_owned(),
                    center: std::array::from_fn(|axis| {
                        let coordinate = entity[["canvasColumn", "canvasRow"][axis]]
                            .as_i64()
                            .unwrap_or(i64::from(automatic[axis]))
                            as i32;
                        f64::from(coordinate) * f64::from(GRID_SIZE) + f64::from(size[axis]) / 2.0
                    }),
                })
            })
            .collect();
        let Some(visible) = uncovered_canvas(
            rect,
            self.layer_palette
                .panel_rect
                .filter(|_| self.layers_available()),
        ) else {
            return;
        };
        for direction in directions {
            let center = (visible.center() - rect.min - self.pan) / self.zoom;
            let Some(index) = destination(
                &nodes,
                &self.selection,
                direction,
                [f64::from(center.x), f64::from(center.y)],
            ) else {
                continue;
            };
            let node = &nodes[index];
            port.finish_group();
            self.selection = vec![node.id.clone()];
            self.selected_layer = None;
            self.layer_palette.active_style = None;
            self.icon_pan = None;
            if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(focused));
            }
            // Reveal immediately, moving the camera only as far as necessary.
            let card = Rect::from_center_size(
                rect.min
                    + self.pan
                    + Vec2::new(node.center[0] as f32, node.center[1] as f32) * self.zoom,
                Vec2::new(size[0], size[1]) * self.zoom,
            );
            let margin = 20.0_f32.min(visible.width().min(visible.height()) / 4.0);
            let inset = visible.shrink(margin);
            let shift = |min: f32, max: f32, low: f32, high: f32| {
                if max - min > high - low {
                    (low + high - min - max) / 2.0
                } else {
                    (low - min).max(0.0) + (high - max).min(0.0)
                }
            };
            self.pan += Vec2::new(
                shift(card.left(), card.right(), inset.left(), inset.right()),
                shift(card.top(), card.bottom(), inset.top(), inset.bottom()),
            );
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cone_rejects_behind_sideways_and_coincident_nodes_in_every_direction() {
        for (direction, axis) in [
            (Direction::Right, [1.0, 0.0]),
            (Direction::Left, [-1.0, 0.0]),
            (Direction::Down, [0.0, 1.0]),
            (Direction::Up, [0.0, -1.0]),
        ] {
            let point = |forward, side| {
                [
                    axis[0] * forward - axis[1] * side,
                    axis[1] * forward + axis[0] * side,
                ]
            };
            for (forward, side, eligible) in [
                (0.0, 0.0, false),
                (-10.0, 0.0, false),
                (0.0, 10.0, false),
                (10.0, 10.1, false),
                (10.0, 10.0, true),
                (10.0, -10.0, true),
                (10.0, 0.0, true),
            ] {
                assert_eq!(
                    direction.rank([0.0; 2], point(forward, side)).is_some(),
                    eligible
                );
            }
        }
    }

    #[test]
    fn proximity_and_alignment_balance_without_distant_straight_line_jumps() {
        let nodes = [[0.0, 0.0], [10.0, 9.0], [20.0, 0.0], [100.0, 0.0]]
            .into_iter()
            .enumerate()
            .map(|(index, center)| Node {
                id: index.to_string(),
                center,
            })
            .collect::<Vec<_>>();
        let selected = ["0".to_owned()];
        assert_eq!(
            destination(&nodes, &selected, Direction::Right, [0.0; 2]),
            Some(2)
        );
        assert_eq!(
            destination(&nodes[..2], &selected, Direction::Right, [0.0; 2]),
            Some(1)
        );
        let without_near_straight = nodes
            .into_iter()
            .enumerate()
            .filter(|(index, _)| *index != 2)
            .map(|(_, node)| node)
            .collect::<Vec<_>>();
        assert_eq!(
            destination(
                &without_near_straight,
                &selected,
                Direction::Right,
                [0.0; 2]
            ),
            Some(1)
        );
    }

    #[test]
    fn initial_selection_and_geometric_ties_are_predictable() {
        let mut nodes = vec![
            Node {
                id: "anchor".into(),
                center: [0.0, 0.0],
            },
            Node {
                id: "b".into(),
                center: [10.0, 2.0],
            },
            Node {
                id: "a".into(),
                center: [10.0, -2.0],
            },
        ];
        let selected = ["anchor".into()];
        let pick = |nodes: &[Node], selection: &[String], center| {
            destination(nodes, selection, Direction::Right, center)
                .map(|index| nodes[index].id.clone())
        };
        assert_eq!(pick(&nodes, &selected, [0.0; 2]).as_deref(), Some("a"));
        nodes.reverse();
        assert_eq!(pick(&nodes, &selected, [0.0; 2]).as_deref(), Some("a"));
        assert_eq!(pick(&nodes, &[], [10.0, 2.0]).as_deref(), Some("b"));
        assert!(destination(&[], &[], Direction::Right, [0.0; 2]).is_none());
    }
}
