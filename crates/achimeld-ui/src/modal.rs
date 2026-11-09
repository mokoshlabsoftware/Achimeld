//! Shared pointer and keyboard behavior for in-app modal panels.

use egui::{Context, Id, Key, Modal, ModalResponse, Order, Rect, Sense, Ui, UiKind};

/// Start centered, then retain the position chosen by dragging the prompt header.
pub fn dialog(ctx: &Context, id: Id) -> Modal {
    let layer = Modal::default_area(id).layer();
    let continuing = ctx.memory(|memory| memory.top_modal_layer() == Some(layer));
    if !continuing {
        ctx.data_mut(|data| data.remove::<egui::Pos2>(id.with("drag-position")));
    }
    let position = ctx.data(|data| data.get_temp::<egui::Pos2>(id.with("drag-position")));
    if let Some(position) = position {
        Modal::new(id).area(
            egui::Area::new(id)
                .kind(UiKind::Modal)
                .order(Order::Foreground)
                .sense(Sense::hover())
                .current_pos(position),
        )
    } else {
        Modal::new(id)
    }
}

/// Make only the prompt header a drag handle, leaving body fields and actions alone.
pub fn drag_header(ui: &mut Ui, header: Rect) {
    let id = ui.layer_id().id;
    let response = ui.interact(header, id.with("drag-header"), Sense::drag());
    if response.dragged()
        && let Some(rect) = ui.ctx().memory(|memory| memory.area_rect(id))
    {
        let bounds = ui.ctx().content_rect();
        let position = rect.min + response.drag_delta();
        let position = egui::pos2(
            position.x.clamp(
                bounds.left(),
                (bounds.right() - rect.width()).max(bounds.left()),
            ),
            position.y.clamp(
                bounds.top(),
                (bounds.bottom() - rect.height()).max(bounds.top()),
            ),
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(id.with("drag-position"), position));
        ui.ctx().request_repaint();
    }
}

/// Consume Enter only for the front modal and never while a popup owns the key.
pub fn enter<T>(ctx: &Context, response: &ModalResponse<T>) -> bool {
    response.is_top_modal
        && !response.any_popup_open
        && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_header_moves_panel_and_enter_stays_with_top_modal() {
        let ctx = Context::default();
        let id = Id::new("movable-modal-test");
        let mut time = 0.0;
        let mut frame = |events| {
            time += 0.05;
            let mut accepted = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_ui| {
                    let response = dialog(&ctx, id).show(&ctx, |ui| {
                        ui.set_width(320.0);
                        let (header, _) =
                            ui.allocate_exact_size(egui::vec2(320.0, 50.0), Sense::hover());
                        drag_header(ui, header);
                        ui.label("Prompt body");
                    });
                    accepted = enter(&ctx, &response);
                },
            );
            output.textures_delta.clear();
            accepted
        };
        frame(vec![]);
        frame(vec![]);
        let original = ctx.memory(|memory| memory.area_rect(id)).unwrap();
        let start = original.left_top() + egui::vec2(60.0, 25.0);
        frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        frame(vec![egui::Event::PointerMoved(
            start + egui::vec2(80.0, 35.0),
        )]);
        frame(vec![egui::Event::PointerButton {
            pos: start + egui::vec2(80.0, 35.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let moved = ctx.memory(|memory| memory.area_rect(id)).unwrap();
        assert!(
            moved.left() > original.left() + 50.0,
            "{original:?} {moved:?}"
        );
        assert!(
            moved.top() > original.top() + 20.0,
            "{original:?} {moved:?}"
        );
        assert!(frame(vec![egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]));
    }
}
