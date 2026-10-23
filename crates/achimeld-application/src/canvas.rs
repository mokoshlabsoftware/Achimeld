use crate::EntityKind;

// Project canvas coordinates are 20-DIP grid cells, as in the original editor.
// Card dimensions and wrapping must use the same units for load, add and duplicate.
pub const GRID_SIZE: f32 = 20.0;
pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 4.0;

pub fn card_size(kind: EntityKind) -> [f32; 2] {
    match kind {
        EntityKind::Achievement => [336.0, 118.0],
        EntityKind::Stat => [236.0, 178.0],
    }
}

pub fn cell_span(kind: EntityKind) -> [i32; 2] {
    card_size(kind).map(|size| (size / GRID_SIZE).ceil() as i32)
}

pub fn auto_position(kind: EntityKind, index: usize) -> [i32; 2] {
    let span = cell_span(kind);
    [(index % 5) as i32 * span[0], (index / 5) as i32 * span[1]]
}

/// Find the closest unoccupied grid position, including cards outside the viewport/filter.
/// Candidate rows are the requested row and card edges; between edges the blocked
/// horizontal intervals do not change. This avoids scanning every canvas grid cell.
pub fn vacant_position(
    kind: EntityKind,
    entities: &[serde_json::Value],
    near: [i32; 2],
) -> [i32; 2] {
    const LIMIT: i32 = 100_000;
    let near = near.map(|n| n.clamp(-LIMIT, LIMIT));
    let [width, height] = cell_span(kind);
    let occupied: Vec<_> = entities
        .iter()
        .enumerate()
        .map(|(i, entity)| {
            let automatic = auto_position(kind, i);
            [
                entity["canvasColumn"]
                    .as_i64()
                    .unwrap_or(i64::from(automatic[0])) as i32,
                entity["canvasRow"]
                    .as_i64()
                    .unwrap_or(i64::from(automatic[1])) as i32,
            ]
        })
        .collect();
    let mut rows = vec![near[1], -LIMIT, LIMIT];
    for [_, row] in &occupied {
        rows.extend(
            [row.saturating_sub(height), row.saturating_add(height)]
                .map(|n| n.clamp(-LIMIT, LIMIT)),
        );
    }
    rows.sort_unstable();
    rows.dedup();
    rows.sort_unstable_by_key(|row| {
        (
            (i64::from(*row) - i64::from(near[1])).abs(),
            std::cmp::Reverse(*row),
        )
    });
    let mut best = near;
    let mut distance = i64::MAX;
    for row in rows {
        let dy = i64::from(row) - i64::from(near[1]);
        if dy * dy > distance {
            break;
        }
        let mut blocked: Vec<_> = occupied
            .iter()
            .filter(|[_, y]| (i64::from(*y) - i64::from(row)).abs() < i64::from(height))
            .map(|[x, _]| (x.saturating_sub(width - 1), x.saturating_add(width - 1)))
            .collect();
        blocked.sort_unstable();
        let mut column = Some(near[0]);
        let mut merged: Option<(i32, i32)> = None;
        // Merge forbidden integer intervals so adjoining cards cannot hide a collision.
        for (left, right) in blocked
            .into_iter()
            .chain(std::iter::once((i32::MAX, i32::MAX)))
        {
            if let Some((start, end)) = merged {
                if left <= end.saturating_add(1) {
                    merged = Some((start, end.max(right)));
                    continue;
                }
                if (start..=end).contains(&near[0]) {
                    column = [end.saturating_add(1), start.saturating_sub(1)]
                        .into_iter()
                        .filter(|x| (-LIMIT..=LIMIT).contains(x))
                        .min_by_key(|x| (i64::from(*x) - i64::from(near[0])).abs());
                    break;
                }
            }
            merged = Some((left, right));
        }
        let Some(column) = column else {
            continue;
        };
        let dx = i64::from(column) - i64::from(near[0]);
        let next = dx * dx + dy * dy;
        if next < distance {
            distance = next;
            best = [column, row];
            if distance == 0 {
                break;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Editor};
    use serde_json::json;

    fn clear(kind: EntityKind, entities: &[serde_json::Value], at: [i32; 2]) -> bool {
        let [w, h] = cell_span(kind);
        entities.iter().enumerate().all(|(i, entity)| {
            let auto = auto_position(kind, i);
            let x = entity["canvasColumn"]
                .as_i64()
                .unwrap_or(i64::from(auto[0]));
            let y = entity["canvasRow"].as_i64().unwrap_or(i64::from(auto[1]));
            (i64::from(at[0]) - x).abs() >= i64::from(w)
                || (i64::from(at[1]) - y).abs() >= i64::from(h)
        })
    }

    #[test]
    fn placement_matches_nearest_free_grid_point_with_irregular_and_legacy_cards() {
        let entities = vec![
            json!({"canvasColumn":-12,"canvasRow":-4}),
            json!({}),
            json!({"canvasColumn":3,"canvasRow":7}),
            json!({"canvasColumn":-8,"canvasRow":12}),
        ];
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            for near in [[0, 0], [-8, 3], [6, 7], [17, 0], [-30, -18]] {
                let actual = vacant_position(kind, &entities, near);
                assert!(clear(kind, &entities, actual));
                let distance = |at: [i32; 2]| {
                    (i64::from(at[0] - near[0])).pow(2) + (i64::from(at[1] - near[1])).pow(2)
                };
                let brute = (-50..=50)
                    .flat_map(|x| (-50..=50).map(move |y| [x, y]))
                    .filter(|at| clear(kind, &entities, *at))
                    .map(distance)
                    .min()
                    .unwrap();
                assert_eq!(distance(actual), brute, "{kind:?}, {near:?}");
            }
        }
    }

    #[test]
    fn creation_avoids_moved_cards_and_undo_restores_the_whole_edit() {
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            let mut editor = Editor::default();
            editor
                .execute(
                    "Add",
                    Command::AddAt {
                        kind,
                        position: [0, 0],
                    },
                    None,
                )
                .unwrap();
            let first = editor.document().entities(kind)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            editor
                .execute(
                    "Move",
                    Command::Move {
                        kind,
                        id: first,
                        column: 17,
                        row: 0,
                    },
                    None,
                )
                .unwrap();
            let before = editor.document().clone();
            for command in [
                Command::Add(kind),
                Command::AddAt {
                    kind,
                    position: [17, 0],
                },
            ] {
                editor.execute("Create", command, None).unwrap();
                let created = editor.document().entities(kind).last().unwrap();
                let position = [
                    created["canvasColumn"].as_i64().unwrap() as i32,
                    created["canvasRow"].as_i64().unwrap() as i32,
                ];
                assert!(clear(kind, before.entities(kind), position));
                let after = editor.document().clone();
                editor.undo();
                assert_eq!(editor.document(), &before);
                editor.redo();
                assert_eq!(editor.document(), &after);
                editor.undo();
            }
        }
    }

    #[test]
    fn placement_stays_within_storage_bounds_and_skips_fully_occupied_rows() {
        let kind = EntityKind::Achievement;
        let entities: Vec<_> = (-100_000..=100_000)
            .step_by(17)
            .map(|x| json!({"canvasColumn":x,"canvasRow":0}))
            .collect();
        let at = vacant_position(kind, &entities, [0, 0]);
        assert!(clear(kind, &entities, at));
        assert_eq!(at, [0, 6]);
        let corner = vec![json!({"canvasColumn":100_000,"canvasRow":100_000})];
        let at = vacant_position(kind, &corner, [i32::MAX, i32::MAX]);
        assert!(at.iter().all(|n| (-100_000..=100_000).contains(n)));
        assert!(clear(kind, &corner, at));
    }
}
