use super::*;

#[test]
fn fixed_sort_form_shows_all_full_labels_in_both_directions_and_languages() {
    for language in ["English", "Polish"] {
        for columns in [false, true] {
            for font in [13.0, 24.0] {
                let mut fixture = Fixture::new();
                fixture.editor = fixture.editor.with_preferences(Preferences {
                    fill_nodes_by_columns: columns,
                    normal_size: font,
                    language: language.into(),
                    field_label_min_width: 40.0,
                    field_label_width_percent: 5.0,
                    ..Default::default()
                });
                let mut desktop = Desktop::default();
                desktop.sort_open = true;
                let ctx = egui::Context::default();
                configure_test_fonts(&ctx);
                for frame in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1100.0, 900.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            crate::theme::apply(ui.ctx(), fixture.preferences());
                            crate::localization::language(ui.ctx(), language);
                            desktop.sort_window(ui.ctx(), &mut fixture);
                        },
                    );
                    output.textures_delta.clear();
                    if frame == 0 {
                        continue; // Window measurement precedes its first painted frame.
                    }
                    for key in [
                        if columns { "WkAcCl" } else { "WkAcRw" },
                        if columns { "WkStCl" } else { "WkStRw" },
                        "WkSoAc",
                    ] {
                        let full = crate::localization::text(&ctx, key);
                        let (shape, text) = output
                            .shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Text(text) if text.galley.text() == full => {
                                    Some((shape, text))
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| panic!("Missing {full}"));
                        assert!(!text.galley.elided, "Elided {full}");
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            shape.clip_rect.expand(1.0).contains_rect(rect),
                            "Clipped {full}"
                        );
                    }
                }
            }
        }
    }
}

fn click(desktop: &mut Desktop, fixture: &mut Fixture, ctx: &egui::Context, id: egui::Id) {
    let response = ctx
        .read_response(id)
        .unwrap_or_else(|| panic!("Missing control {id:?}"));
    assert!(
        response.enabled(),
        "Disabled sort control {id:?}: {response:?}"
    );
    let pos = response.rect.center();
    for pressed in [true, false] {
        draw_editor(
            desktop,
            fixture,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    for _ in 0..3 {
        draw_editor(desktop, fixture, ctx, vec![]);
    }
}

#[test]
fn sort_toolbar_opens_original_controls_and_aligns_selected_nodes_in_both_views() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        let mut fixture = Fixture::new();
        if kind == EntityKind::Stat {
            fixture
                .editor
                .execute("Add", Command::Add(kind), None)
                .unwrap();
        }
        let mut project = fixture.editor.document().project().clone();
        project[kind.key()][0]["canvasColumn"] = json!(1);
        project[kind.key()][0]["canvasRow"] = json!(1);
        let before = achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap();
        let selected = before.entities(kind)[1]["id"].as_str().unwrap().to_owned();
        fixture.editor = Editor::new(before.clone());
        let mut desktop = Desktop::default();
        desktop.kind = kind;
        desktop.view = if kind == EntityKind::Stat {
            desktop::View::Stats
        } else {
            desktop::View::Achievements
        };
        desktop.selection = vec![selected.clone()];
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        for _ in 0..2 {
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        }
        let sort = ctx.read_response(egui::Id::new("sort-toolbar")).unwrap();
        assert!(sort.rect.height() >= 24.0);
        click(&mut desktop, &mut fixture, &ctx, sort.id);
        assert!(desktop.sort_open);
        assert!(desktop.automation_busy());
        let option = |name| egui::Id::new(("sort-option", name));
        assert!(!ctx.read_response(option("new-line")).unwrap().enabled());
        click(&mut desktop, &mut fixture, &ctx, option("style"));
        assert!(fixture.preferences().sort_nodes_by_icon_style);
        assert!(ctx.read_response(option("new-line")).unwrap().enabled());
        click(&mut desktop, &mut fixture, &ctx, option("new-line"));
        click(&mut desktop, &mut fixture, &ctx, option("rows"));
        click(&mut desktop, &mut fixture, &ctx, option("selection"));
        assert!(fixture.preferences().new_line_per_icon_style);
        assert!(fixture.preferences().fill_nodes_by_columns);
        assert!(fixture.preferences().order_selected_nodes_only);
        let helpers = ctx.read_response(option("helpers")).unwrap();
        assert_eq!(helpers.enabled(), kind == EntityKind::Achievement);
        if kind == EntityKind::Achievement {
            click(&mut desktop, &mut fixture, &ctx, option("helpers"));
            assert!(fixture.preferences().include_helpers_in_sorting);
        }
        assert_eq!(fixture.editor.document(), &before);
        click(
            &mut desktop,
            &mut fixture,
            &ctx,
            egui::Id::new("sort-align"),
        );
        assert_eq!(fixture.editor.undo_label(), Some("Align nodes to grid"));
        let moved = fixture.editor.document().entity(kind, &selected).unwrap();
        assert_eq!(moved["canvasColumn"], 0);
        assert_eq!(
            moved["canvasRow"],
            achimeld_application::canvas::cell_span(kind)[1] * 2
        );
        assert_eq!(desktop.selection, vec![selected]);
        fixture.undo();
        assert_eq!(fixture.editor.document(), &before);
        click(
            &mut desktop,
            &mut fixture,
            &ctx,
            egui::Id::new("sort-close"),
        );
        assert!(!desktop.sort_open);
        desktop.sort_open = true;
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        assert!(fixture.preferences().fill_nodes_by_columns);
    }
}
