//! Ordered navigation for shared popup buttons, including virtualized catalogs.
use egui::{Id, LayerId, Pos2, Rect, Response, Ui};

#[derive(Clone, Default)]
struct Navigation {
    frame: u64,
    rows: Vec<(Id, Rect)>,
    active: Option<Id>,
    keyboard: bool,
    pointer: Option<Pos2>,
    activate: Option<Id>,
    moved: bool,
    virtual_order: bool,
}

fn state_id(layer: LayerId) -> Id {
    layer.id.with("dropdown-navigation")
}

pub(crate) fn keyboard_row(ctx: &egui::Context, layer: LayerId) -> Option<Id> {
    ctx.data(|data| data.get_temp::<Navigation>(state_id(layer)))
        .filter(|state| state.keyboard)
        .and_then(|state| state.active)
}

pub(crate) fn prepare(ui: &Ui) {
    if !ui
        .stack()
        .iter()
        .any(|stack| matches!(stack.kind(), Some(egui::UiKind::Popup | egui::UiKind::Menu)))
    {
        return;
    }
    let id = state_id(ui.layer_id());
    let frame = ui.ctx().cumulative_frame_nr();
    let previous = ui.data(|data| data.get_temp::<Navigation>(id));
    if previous.as_ref().is_some_and(|state| state.frame == frame) {
        return;
    }
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let mut state = previous
        .filter(|state| state.frame + 1 == frame)
        .unwrap_or_else(|| Navigation {
            pointer,
            ..Default::default()
        });
    state.activate = None;
    state.moved = false;
    if ui.ctx().text_edit_focused() || !ui.input(|input| input.focused) {
        state.keyboard = false;
    }
    let accepts_keys = ui.is_enabled()
        && ui.input(|input| input.focused)
        && !ui.ctx().text_edit_focused()
        && ui.memory(|memory| memory.areas().top_layer_id(ui.layer_id().order))
            == Some(ui.layer_id());
    let events = ui.input(|input| input.events.clone());
    let mut pointer_changed = false;
    let mut consumed = Vec::new();
    // Apply modality changes in event order, including multiple repeats and a
    // mouse move after a key in the same input batch. Repeated identical pointer
    // positions do not interrupt keyboard ownership.
    for event in events {
        match event {
            egui::Event::PointerMoved(pos) if state.pointer != Some(pos) => {
                state.pointer = Some(pos);
                state.keyboard = false;
                pointer_changed = true;
                state.active = (ui.ctx().layer_id_at(pos) == Some(ui.layer_id()))
                    .then(|| state.rows.iter().find(|(_, rect)| rect.contains(pos)))
                    .flatten()
                    .map(|(id, _)| *id);
            }
            egui::Event::PointerGone => {
                state.pointer = None;
                state.keyboard = false;
                pointer_changed = true;
                state.active = None;
            }
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                ..
            } => {
                // Let Tab reach search fields and auxiliary actions normally.
                state.keyboard = false;
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
                ..
            } if accepts_keys
                && matches!(key, egui::Key::ArrowDown | egui::Key::ArrowUp)
                && !state.rows.is_empty() =>
            {
                let current = state
                    .active
                    .and_then(|active| state.rows.iter().position(|(id, _)| *id == active));
                let next = match (current, key) {
                    (Some(index), egui::Key::ArrowDown) => (index + 1).min(state.rows.len() - 1),
                    (Some(index), _) => index.saturating_sub(1),
                    (None, egui::Key::ArrowDown) => 0,
                    (None, _) => state.rows.len() - 1,
                };
                state.active = Some(state.rows[next].0);
                state.keyboard = true;
                state.moved = true;
                consumed.push(key);
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
                ..
            } if accepts_keys
                && state.keyboard
                && matches!(key, egui::Key::Enter | egui::Key::Space) =>
            {
                state.activate = state.active;
                consumed.push(key);
            }
            _ => {}
        }
    }
    ui.input_mut(|input| {
        for key in consumed {
            input.consume_key(egui::Modifiers::NONE, key);
        }
    });
    if state.keyboard && accepts_keys {
        ui.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            if let Some(active) = state.active {
                memory.request_focus(active);
            }
        });
    } else if pointer_changed
        && let Some(focused) = ui.memory(|memory| memory.focused())
        && state.rows.iter().any(|(id, _)| *id == focused)
    {
        ui.memory_mut(|memory| memory.surrender_focus(focused));
    }
    state.frame = frame;
    state.rows.clear();
    state.virtual_order = false;
    ui.data_mut(|data| data.insert_temp(id, state));
}

pub(crate) fn interact(ui: &Ui, response: &mut Response, selected: bool) {
    let id = state_id(ui.layer_id());
    let Some(mut state) = ui.data(|data| data.get_temp::<Navigation>(id)) else {
        return;
    };
    if state.frame != ui.ctx().cumulative_frame_nr() {
        return;
    }
    if response.enabled() {
        if let Some((_, rect)) = state.rows.iter_mut().find(|(id, _)| *id == response.id) {
            *rect = response.rect.intersect(ui.clip_rect());
        } else if !state.virtual_order {
            state.rows.push((response.id, response.rect));
        }
        if state.active.is_none() && selected {
            state.active = Some(response.id);
        }
        if state.keyboard {
            response.flags.set(egui::response::Flags::HOVERED, false);
            response.flags.set(
                egui::response::Flags::FAKE_PRIMARY_CLICKED,
                state.activate == Some(response.id),
            );
            if state.active == Some(response.id) {
                response.request_focus();
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }
    }
    ui.data_mut(|data| data.insert_temp(id, state));
}

/// Give virtualized lists their full logical order without rendering hidden rows.
/// Returns a scroll target only when the keyboard moves to a different item.
pub(crate) fn virtual_rows(ui: &Ui, ids: &[Id], selected: Option<Id>) -> Option<usize> {
    prepare(ui);
    let id = state_id(ui.layer_id());
    let mut state = ui.data(|data| data.get_temp::<Navigation>(id))?;
    state.virtual_order = true;
    let old: std::collections::HashMap<_, _> = state.rows.drain(..).collect();
    state.rows = ids
        .iter()
        .map(|id| (*id, old.get(id).copied().unwrap_or(Rect::NOTHING)))
        .collect();
    if state.active.is_none_or(|id| !ids.contains(&id)) {
        state.active = selected.filter(|id| ids.contains(id));
    }
    let scroll = (state.keyboard && state.moved)
        .then(|| {
            state
                .active
                .and_then(|active| ids.iter().position(|id| *id == active))
        })
        .flatten();
    ui.data_mut(|data| data.insert_temp(id, state));
    scroll
}
