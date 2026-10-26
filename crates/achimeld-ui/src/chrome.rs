use egui::{CursorIcon, PointerButton, Rect, ResizeDirection, Sense, Stroke, ViewportCommand};

pub(crate) fn brand_image() -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [128, 128],
        include_bytes!("../assets/achimeld-logo.rgba"),
    )
}

// Only the title bar uses the white mark; other surfaces retain the app colors.
pub(crate) fn title_brand_image() -> egui::ColorImage {
    egui::ColorImage::new(
        [128, 128],
        include_bytes!("../assets/achimeld-title-mark.rgba")
            .chunks_exact(4)
            .map(|pixel| egui::Color32::from_white_alpha(pixel[3]))
            .collect(),
    )
}

pub(crate) fn title_menu(
    ui: &mut egui::Ui,
    label: &str,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let mut content = Some(content);
    let [response] = title_menu_bar(ui, [label], |_, ui| {
        if let Some(content) = content.take() {
            content(ui);
        }
    });
    response
}

pub(crate) fn title_menu_bar<const N: usize>(
    ui: &mut egui::Ui,
    labels: [&str; N],
    mut content: impl FnMut(usize, &mut egui::Ui),
) -> [egui::Response; N] {
    // Allocate every heading before showing any popup: switching must work in
    // either direction without painting the previous menu for another frame.
    let responses = labels.map(|label| {
        ui.add(
            crate::buttons::Button::new(label)
                .style(crate::buttons::ButtonStyle::Quiet)
                .square_corners(),
        )
    });
    let open = responses.iter().position(|response| {
        egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(response))
    });
    if let Some(open) = open
        && let Some(hovered) = responses.iter().position(|response| response.hovered())
        && open != hovered
        && responses[hovered].enabled()
    {
        responses[open].surrender_focus();
        egui::Popup::open_id(
            ui.ctx(),
            egui::Popup::default_response_id(&responses[hovered]),
        );
    }
    for (index, response) in responses.iter().enumerate() {
        crate::buttons::menu_popup(ui, response, |ui| {
            // The compact title bar must not shrink the popup's controls.
            ui.spacing_mut().button_padding = ui.ctx().global_style().spacing.button_padding;
            ui.spacing_mut().interact_size = ui.ctx().global_style().spacing.interact_size;
            content(index, ui);
        });
    }
    responses
}

pub(crate) fn drag_window(response: &egui::Response) {
    if response.double_clicked() {
        toggle_maximized(&response.ctx);
    } else if response.is_pointer_button_down_on()
        && response
            .ctx
            .input(|i| i.pointer.button_pressed(PointerButton::Primary))
    {
        response.ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let command = ctx.input(|i| {
        if i.viewport().fullscreen.unwrap_or(false) {
            ViewportCommand::Fullscreen(false)
        } else {
            ViewportCommand::Maximized(!i.viewport().maximized.unwrap_or(false))
        }
    });
    ctx.send_viewport_cmd(command);
}

// Return close intent to the editor so its draft and save checks stay in charge.
pub(crate) fn window_controls(ui: &mut egui::Ui) -> bool {
    let expanded = ui.ctx().input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    });
    let mut close = false;
    for (id, label) in [
        ("window-close", "Close"),
        (
            "window-maximize",
            if expanded { "Restore" } else { "Maximize" },
        ),
        ("window-minimize", "Minimize"),
    ] {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 24.0), Sense::hover());
        let response = ui
            .interact(rect, egui::Id::new(id), Sense::click())
            .on_hover_text(label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        let (body, ink) = crate::buttons::colors(ui, &response, crate::buttons::ButtonStyle::Quiet);
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, body);
        let stroke = Stroke::new(1.5, ink);
        let center = rect.center();
        let icon = Rect::from_center_size(center, egui::vec2(10.0, 10.0));
        match id {
            "window-close" => {
                ui.painter()
                    .line_segment([icon.left_top(), icon.right_bottom()], stroke);
                ui.painter()
                    .line_segment([icon.right_top(), icon.left_bottom()], stroke);
                close = response.clicked();
            }
            "window-maximize" => {
                if expanded {
                    let back = icon.translate(egui::vec2(2.0, -2.0));
                    ui.painter()
                        .line_segment([back.left_top(), back.right_top()], stroke);
                    ui.painter()
                        .line_segment([back.right_top(), back.right_bottom()], stroke);
                }
                ui.painter()
                    .rect_stroke(icon, 0, stroke, egui::StrokeKind::Inside);
                if response.clicked() {
                    toggle_maximized(ui.ctx());
                }
            }
            _ => {
                ui.painter().line_segment(
                    [center - egui::vec2(5.0, 0.0), center + egui::vec2(5.0, 0.0)],
                    stroke,
                );
                if response.clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                }
            }
        }
    }
    close
}

pub(crate) fn resize_window(ui: &mut egui::Ui, rect: Rect) {
    if ui.ctx().input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let edge = 5.0;
    let corner = 12.0;
    let regions = [
        (
            Rect::from_min_max(rect.min, rect.min + egui::vec2(corner, corner)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                rect.right_top() - egui::vec2(corner, 0.0),
                rect.right_top() + egui::vec2(0.0, corner),
            ),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(
                rect.left_bottom() - egui::vec2(0.0, corner),
                rect.left_bottom() + egui::vec2(corner, 0.0),
            ),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(rect.max - egui::vec2(corner, corner), rect.max),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                rect.min + egui::vec2(corner, 0.0),
                rect.right_top() + egui::vec2(-corner, edge),
            ),
            ResizeDirection::North,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                rect.left_bottom() + egui::vec2(corner, -edge),
                rect.max - egui::vec2(corner, 0.0),
            ),
            ResizeDirection::South,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                rect.min + egui::vec2(0.0, corner),
                rect.left_bottom() + egui::vec2(edge, -corner),
            ),
            ResizeDirection::West,
            CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(
                rect.right_top() + egui::vec2(-edge, corner),
                rect.max - egui::vec2(0.0, corner),
            ),
            ResizeDirection::East,
            CursorIcon::ResizeHorizontal,
        ),
    ];
    for (index, (rect, direction, cursor)) in regions.into_iter().enumerate() {
        let response = ui
            .interact(rect, ui.id().with(("window-resize", index)), Sense::drag())
            .on_hover_cursor(cursor);
        if response.drag_started_by(PointerButton::Primary) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_title_menu_follows_hover_in_both_directions_and_dismisses() {
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut frame = |events: Vec<egui::Event>, enabled| {
            time += 0.05;
            let mut headings = Vec::new();
            let mut shown = Vec::new();
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
                |ui| {
                    headings.clear();
                    shown.clear();
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.horizontal(|ui| {
                            headings.extend(title_menu_bar(
                                ui,
                                ["File", "Edit", "Tools", "Windows", "Help"],
                                |index, ui| {
                                    shown.push(index);
                                    ui.label(
                                        [
                                            "File commands",
                                            "Edit commands",
                                            "Tools commands",
                                            "Windows commands",
                                            "Help commands",
                                        ][index],
                                    );
                                },
                            ));
                        });
                    });
                },
            );
            output.textures_delta.clear();
            (headings, shown)
        };
        frame(vec![], true);
        let (headings, _) = frame(vec![], true);
        let point = |index: usize| headings[index].rect.center();
        let pointer = |index: usize, pressed| egui::Event::PointerButton {
            pos: point(index),
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(
            frame(vec![egui::Event::PointerMoved(point(1))], true)
                .1
                .is_empty()
        );
        frame(vec![pointer(1, true)], true);
        assert_eq!(frame(vec![pointer(1, false)], true).1, [1]);
        for index in [0, 2, 4, 3, 1, 0] {
            assert_eq!(
                frame(vec![egui::Event::PointerMoved(point(index))], true).1,
                [index]
            );
        }
        // Moving into the open menu must not close it or change its heading.
        assert_eq!(
            frame(
                vec![egui::Event::PointerMoved(point(0) + egui::vec2(0.0, 30.0))],
                true
            )
            .1,
            [0]
        );
        frame(
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            true,
        );
        assert!(
            frame(vec![egui::Event::PointerMoved(point(2))], true)
                .1
                .is_empty()
        );
        frame(vec![pointer(2, true)], true);
        frame(vec![pointer(2, false)], true);
        assert_eq!(
            frame(vec![egui::Event::PointerMoved(point(0))], false).1,
            [2]
        );
    }
}
