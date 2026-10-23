//! Achiever's AlignProjectNodesEdit, using the native document and style identity.
//! Sorting changes canvas positions only; IDs, display order and wire shapes stay intact.
use crate::{Command, Document, EntityKind, IconStyle, Preferences, canvas, style_signature};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

struct Node<'a> {
    entity: &'a Value,
    signature: String,
    style_name: String,
    activation: Option<Decimal>,
}

/// Prepare one undoable batch. `selection` is ignored unless selected-only is enabled.
pub fn align_nodes(
    document: &Document,
    kind: EntityKind,
    preferences: &Preferences,
    styles: &[IconStyle],
    selection: &[String],
) -> Vec<Command> {
    let selected: BTreeSet<_> = selection.iter().map(String::as_str).collect();
    if preferences.order_selected_nodes_only && selected.is_empty() {
        return Vec::new();
    }
    let signature = |recipe: &Value| style_signature(recipe).to_string();
    let mut style_names = BTreeMap::new();
    for style in styles {
        style_names
            .entry(signature(&style.recipe))
            .or_insert_with(|| style.name.to_uppercase());
    }
    let mut by_display: Vec<_> = document.entities(kind).iter().collect();
    by_display.sort_by_key(|entity| entity["displayOrder"].as_i64().unwrap_or(0));
    let span = canvas::cell_span(kind).map(i64::from);
    // An off-grid card occupies its whole rectangle, including adjacent candidate slots.
    let occupied: BTreeSet<_> = by_display
        .iter()
        .enumerate()
        .filter(|(_, entity)| {
            (crate::style_achievements::is_local_icon(entity)
                && !preferences.include_helpers_in_sorting)
                || (preferences.order_selected_nodes_only
                    && !selected.contains(entity["id"].as_str().unwrap_or("")))
        })
        .flat_map(|(index, entity)| {
            let automatic = canvas::auto_position(kind, index).map(i64::from);
            let column = entity["canvasColumn"].as_i64().unwrap_or(automatic[0]);
            let row = entity["canvasRow"].as_i64().unwrap_or(automatic[1]);
            // Each stationary rectangle intersects at most four candidate cells.
            let left = column.div_euclid(span[0]);
            let right = (column + span[0] - 1).div_euclid(span[0]);
            let top = row.div_euclid(span[1]);
            let bottom = (row + span[1] - 1).div_euclid(span[1]);
            (left..=right).flat_map(move |x| (top..=bottom).map(move |y| [x, y]))
        })
        .collect();
    let mut ordered: Vec<_> = by_display.into_iter()
        .filter(|entity| (!crate::style_achievements::is_local_icon(entity) || preferences.include_helpers_in_sorting)
            && (!preferences.order_selected_nodes_only || selected.contains(entity["id"].as_str().unwrap_or(""))))
        .map(|entity| {
            let signature = if kind == EntityKind::Achievement {
                let recipe = if entity["icon"].is_object() { entity["icon"].clone() } else {
                    json!({"backgroundColorHex":null,"layers":[{"id":entity["id"],"displayName":"Icon","kind":if entity["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"visible":true,"opacity":1.0,"atlasId":entity["atlasId"],"imageId":entity["standaloneImageId"],"tileIndex":entity["tileIndex"],"effects":[]}]})
                };
                signature(&recipe)
            } else { String::new() };
            let style_name = style_names.get(&signature).cloned().unwrap_or_else(|| "\u{ffff}".into());
            let activation = Decimal::from_str(&entity["unlockValue"].as_str().map_or_else(|| entity["unlockValue"].to_string(), str::to_owned)).ok();
            Node { entity, signature, style_name, activation }
        }).collect();
    if kind == EntityKind::Achievement {
        ordered.sort_by(|a, b| {
            let style = if preferences.sort_nodes_by_icon_style {
                a.style_name
                    .cmp(&b.style_name)
                    .then_with(|| a.signature.cmp(&b.signature))
            } else {
                Ordering::Equal
            };
            let linked = |node: &Node<'_>| node.entity["progressStatId"].as_str().is_some();
            crate::style_achievements::is_local_icon(a.entity)
                .cmp(&crate::style_achievements::is_local_icon(b.entity))
                .then(style)
                .then_with(|| {
                    if preferences.sort_nodes_by_stat_activation {
                        linked(b)
                            .cmp(&linked(a))
                            .then_with(|| {
                                a.entity["progressStatId"]
                                    .as_str()
                                    .unwrap_or("")
                                    .cmp(b.entity["progressStatId"].as_str().unwrap_or(""))
                            })
                            .then_with(|| {
                                a.activation
                                    .filter(|_| linked(a))
                                    .cmp(&b.activation.filter(|_| linked(b)))
                            })
                    } else {
                        Ordering::Equal
                    }
                })
                .then_with(|| {
                    a.entity["internalId"]
                        .as_i64()
                        .unwrap_or(0)
                        .cmp(&b.entity["internalId"].as_i64().unwrap_or(0))
                })
                .then_with(|| {
                    a.entity["displayOrder"]
                        .as_i64()
                        .unwrap_or(0)
                        .cmp(&b.entity["displayOrder"].as_i64().unwrap_or(0))
                })
        });
    }
    let per_line = i64::from(
        if kind == EntityKind::Achievement {
            preferences.achievement_nodes_per_row
        } else {
            preferences.stat_nodes_per_row
        }
        .clamp(1, 10),
    );
    let mut slot = 0_i64;
    let mut previous_style: Option<String> = None;
    let mut commands = Vec::with_capacity(ordered.len());
    for node in ordered {
        if kind == EntityKind::Achievement
            && preferences.sort_nodes_by_icon_style
            && preferences.new_line_per_icon_style
            && previous_style
                .as_ref()
                .is_some_and(|style| style != &node.signature)
        {
            slot = ((slot + per_line - 1) / per_line) * per_line;
        }
        let cell = loop {
            let cell = if preferences.fill_nodes_by_columns {
                [slot / per_line, slot % per_line]
            } else {
                [slot % per_line, slot / per_line]
            };
            slot += 1;
            if !occupied.contains(&cell) {
                break cell;
            }
        };
        previous_style = Some(node.signature);
        let column = cell[0] * span[0];
        let row = cell[1] * span[1];
        if node.entity["canvasColumn"].as_i64() != Some(column)
            || node.entity["canvasRow"].as_i64() != Some(row)
        {
            commands.push(Command::Move {
                kind,
                id: node.entity["id"].as_str().unwrap_or("").into(),
                column: column as i32,
                row: row as i32,
            });
        }
    }
    commands
}

#[cfg(test)]
#[path = "sorting_tests.rs"]
mod tests;
