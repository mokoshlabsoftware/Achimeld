use crate::buttons::{self, Button};
use serde_json::{Value, json};

pub(crate) fn show(
    ui: &mut egui::Ui,
    families: Option<&[String]>,
    text: &mut Value,
) -> egui::Response {
    let selected = text["fontFamily"].as_str().unwrap_or("Segoe UI").to_owned();
    let response = crate::inspector::field(ui, "Font family", 150.0, |ui| {
        let width = ui.available_width();
        let button = ui.add_sized(
            [width, ui.spacing().interact_size.y],
            Button::new((
                egui::RichText::new(selected.as_str()).size(
                    (egui::TextStyle::Button
                        .resolve(&ui.ctx().global_style())
                        .size
                        - 1.0)
                        .max(1.0),
                ),
                crate::dropdowns::disclosure(ui),
            ))
            .id(ui.id().with("system-font-picker"))
            .bar()
            .image_tint_follows_text_color(true)
            .wrap_mode(egui::TextWrapMode::Truncate),
        );
        let popup = egui::Popup::default_response_id(&button);
        if !button.enabled() {
            egui::Popup::close_id(ui.ctx(), popup);
        }
        let query_id = button.id.with("font-search");
        let mut query = ui
            .ctx()
            .data_mut(|data| data.get_temp::<String>(query_id))
            .unwrap_or_default();
        if button.clicked() && !egui::Popup::is_id_open(ui.ctx(), popup) {
            query.clear();
        }
        buttons::menu_popup_with_behavior(
            ui,
            &button,
            Some(egui::PopupCloseBehavior::CloseOnClickOutside),
            |ui| {
                ui.set_width(width.max(210.0).min(ui.ctx().content_rect().width() - 16.0));
                let search = egui::Frame::new()
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut query)
                                .id(query_id)
                                .char_limit(128)
                                .hint_text(crate::localization::text(
                                    ui.ctx(),
                                    "Search system fonts…",
                                ))
                                .desired_width(ui.available_width()),
                        )
                    })
                    .inner;
                if button.clicked() {
                    search.request_focus();
                }
                if search.has_focus()
                    && ui.input(|input| {
                        input.key_pressed(egui::Key::ArrowDown)
                            || input.key_pressed(egui::Key::ArrowUp)
                    })
                {
                    search.surrender_focus();
                }
                match families {
                    None => {
                        ui.label(crate::localization::text(ui.ctx(), "Loading system fonts…"));
                    }
                    Some(families) => {
                        let query = query.trim().to_lowercase();
                        let matches: Vec<_> = families
                            .iter()
                            .filter(|name| name.to_lowercase().contains(&query))
                            .collect();
                        if matches.is_empty() {
                            ui.label(crate::localization::text(
                                ui.ctx(),
                                if families.is_empty() {
                                    "No system fonts available."
                                } else {
                                    "No matching fonts."
                                },
                            ));
                        }
                        let height = buttons::bar_height(ui, 0.0);
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        let ids: Vec<_> = matches
                            .iter()
                            .map(|family| query_id.with(family.as_str()))
                            .collect();
                        let scroll_to = crate::dropdown_navigation::virtual_rows(
                            ui,
                            &ids,
                            matches
                                .iter()
                                .position(|family| selected.eq_ignore_ascii_case(family))
                                .map(|index| ids[index]),
                        );
                        let mut scroll = egui::ScrollArea::vertical()
                            .id_salt("system-font-list")
                            .max_height((ui.ctx().content_rect().height() * 0.4).min(320.0))
                            .auto_shrink([false, true]);
                        if let Some(index) = scroll_to {
                            scroll = scroll.vertical_scroll_offset(index as f32 * height);
                        }
                        scroll.show_rows(ui, height, matches.len(), |ui, rows| {
                            for family in &matches[rows] {
                                if ui
                                    .add_sized(
                                        [ui.available_width(), height],
                                        Button::selectable(
                                            selected.to_lowercase() == family.to_lowercase(),
                                            family.as_str(),
                                        )
                                        .id(query_id.with(family.as_str()))
                                        .bar(),
                                    )
                                    .clicked()
                                {
                                    text["fontFamily"] = json!(family);
                                    ui.close();
                                }
                            }
                        });
                    }
                }
            },
        );
        ui.ctx().data_mut(|data| data.insert_temp(query_id, query));
        button
    });
    if families.is_some_and(|fonts| {
        !fonts.is_empty()
            && !fonts
                .iter()
                .any(|name| name.to_lowercase() == selected.to_lowercase())
    }) {
        ui.label(
            egui::RichText::new(crate::localization::text(
                ui.ctx(),
                "This font is unavailable on this computer. Choose a system font to replace it.",
            ))
            .weak(),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_picker_keyboard_reaches_virtualized_results_and_commits_only_on_enter() {
        let ctx = egui::Context::default();
        crate::tests::configure_test_fonts(&ctx);
        let families: Vec<_> = (0..80).map(|index| format!("Family {index:02}")).collect();
        let mut text = json!({"fontFamily":"Family 00","content":"Keep"});
        let mut time = 0.0;
        let mut render = |text: &mut Value, events| {
            time += 0.05;
            let mut response = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(450.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    response = Some(show(ui, Some(&families), text));
                },
            );
            output.textures_delta.clear();
            (response.unwrap(), output)
        };
        let (trigger, _) = render(&mut text, vec![]);
        egui::Popup::open_id(&ctx, egui::Popup::default_response_id(&trigger));
        render(&mut text, vec![]);
        render(&mut text, vec![]);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        for _ in 0..85 {
            render(&mut text, vec![key(egui::Key::ArrowDown)]);
        }
        assert_eq!(text["fontFamily"], "Family 00");
        let (_, output) = render(&mut text, vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text() == "Family 79")));
        render(&mut text, vec![key(egui::Key::Enter)]);
        assert_eq!(text, json!({"fontFamily":"Family 79","content":"Keep"}));
        assert!(!egui::Popup::is_any_open(&ctx));
    }

    fn frame(
        ctx: &egui::Context,
        text: &mut Value,
        events: Vec<egui::Event>,
        width: f32,
    ) -> Vec<(String, egui::Pos2)> {
        fn labels(shape: &egui::Shape, found: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => found.push((
                    t.galley.text().to_owned(),
                    t.pos + t.galley.rect.center().to_vec2(),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, found);
                    }
                }
                _ => {}
            }
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                show(
                    ui,
                    Some(&["Arial".into(), "Cambria".into(), "Segoe UI".into()]),
                    text,
                );
            },
        );
        let mut found = Vec::new();
        for shape in &output.shapes {
            labels(&shape.shape, &mut found);
        }
        output.textures_delta.clear();
        found
    }

    fn click(ctx: &egui::Context, text: &mut Value, pos: egui::Pos2) -> Vec<(String, egui::Pos2)> {
        for pressed in [true, false] {
            frame(
                ctx,
                text,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                450.0,
            );
        }
        frame(ctx, text, vec![], 450.0)
    }

    fn position(labels: &[(String, egui::Pos2)], name: &str) -> egui::Pos2 {
        labels
            .iter()
            .find(|(label, _)| label == name)
            .unwrap_or_else(|| panic!("Missing {name}: {labels:?}"))
            .1
    }

    #[test]
    fn font_picker_search_commits_only_a_selected_system_font_and_escape_preserves_it() {
        let ctx = egui::Context::default();
        let mut text =
            json!({"fontFamily":"Missing family", "content":"Keep this", "fontSize":128});
        frame(&ctx, &mut text, vec![], 450.0);
        let initial = frame(&ctx, &mut text, vec![], 450.0);
        assert!(
            initial
                .iter()
                .any(|(label, _)| label.contains("unavailable"))
        );
        let opened = click(&ctx, &mut text, position(&initial, "Missing family"));
        position(&opened, "Arial");
        position(&opened, "Cambria");
        // Clicking inside the search field must not dismiss the picker.
        click(&ctx, &mut text, position(&opened, "Search system fonts…"));
        assert!(egui::Popup::is_any_open(&ctx));
        assert_eq!(text["fontFamily"], "Missing family");
        let filtered = frame(
            &ctx,
            &mut text,
            vec![egui::Event::Text("CaM".into())],
            450.0,
        );
        assert!(!filtered.iter().any(|(label, _)| label == "Arial"));
        assert_eq!(text["fontFamily"], "Missing family");
        click(&ctx, &mut text, position(&filtered, "Cambria"));
        assert_eq!(
            text,
            json!({"fontFamily":"Cambria", "content":"Keep this", "fontSize":128})
        );
        assert!(!egui::Popup::is_any_open(&ctx));
        let selected = frame(&ctx, &mut text, vec![], 450.0);
        let reopened = click(&ctx, &mut text, position(&selected, "Cambria"));
        position(&reopened, "Arial");
        let empty = frame(
            &ctx,
            &mut text,
            vec![egui::Event::Text("No such font".into())],
            450.0,
        );
        position(&empty, "No matching fonts.");
        frame(
            &ctx,
            &mut text,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            450.0,
        );
        assert!(!egui::Popup::is_any_open(&ctx));
        assert_eq!(text["fontFamily"], "Cambria");
        let selected = frame(&ctx, &mut text, vec![], 450.0);
        click(&ctx, &mut text, position(&selected, "Cambria"));
        assert!(egui::Popup::is_any_open(&ctx));
        let before = text.clone();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.disable();
            show(ui, Some(&["Arial".into(), "Cambria".into()]), &mut text);
        });
        output.textures_delta.clear();
        assert!(!egui::Popup::is_any_open(&ctx));
        assert_eq!(text, before);
    }

    #[test]
    fn font_picker_preserves_saved_names_while_loading_or_unavailable() {
        for families in [None, Some([].as_slice())] {
            let ctx = egui::Context::default();
            let original = json!({"fontFamily":"Saved font from another computer"});
            let mut text = original.clone();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                show(ui, families, &mut text);
            });
            output.textures_delta.clear();
            assert_eq!(text, original);
        }
    }

    #[test]
    fn font_picker_fits_narrow_properties_and_large_application_fonts() {
        for width in [180.0, 250.0, 450.0] {
            for size in [13.0, 24.0] {
                let ctx = egui::Context::default();
                crate::typography::configure_fonts(&ctx, None);
                let mut text = json!({"fontFamily":"A very long family name"});
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 700.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        crate::theme::apply(
                            ui.ctx(),
                            &achimeld_application::Preferences {
                                normal_size: size,
                                ..Default::default()
                            },
                        );
                        ui.set_style(ui.ctx().global_style());
                        show(ui, Some(&["Arial".into()]), &mut text);
                        assert!(
                            ui.min_rect().right() <= ui.max_rect().right() + 0.1,
                            "Overflow at {width}/{size}: {:?}",
                            ui.min_rect()
                        );
                    },
                );
                output.textures_delta.clear();
                assert_eq!(text["fontFamily"], "A very long family name");
            }
        }
    }
}
