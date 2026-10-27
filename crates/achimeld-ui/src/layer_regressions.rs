use super::*;

#[test]
fn layers_match_only_project_styles_and_import_global_templates_from_separate_menu() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let recipe = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF123456"}]});
    entity["icon"] = recipe.clone();
    fixture
        .execute(
            "Icon",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let global = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Global recipe".into(),
        recipe,
    };
    fixture.styles.push(global.clone());
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => result.push((
                    t.galley.text().to_owned(),
                    t.pos + t.galley.rect.center().to_vec2(),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, result);
                    }
                }
                _ => {}
            }
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        );
        let mut result = Vec::new();
        for shape in &output.shapes {
            labels(&shape.shape, &mut result);
        }
        output.textures_delta.clear();
        result
    };
    let click = |desktop: &mut Desktop, fixture: &mut Fixture, pos| {
        for pressed in [true, false] {
            frame(
                desktop,
                fixture,
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
        frame(desktop, fixture, vec![])
    };
    let position = |labels: &[(String, egui::Pos2)], label: &str| {
        labels
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
            .1
    };
    frame(&mut desktop, &mut fixture, vec![]);
    let initial = frame(&mut desktop, &mut fixture, vec![]);
    assert!(!initial.iter().any(|(text, _)| text == &global.name));
    let dropdown = position(&initial, "No style");
    let header = ctx
        .read_response(layer_palette::layers_panel_id().with("header"))
        .unwrap()
        .rect;
    assert!((header.top()..=header.bottom()).contains(&dropdown.y));
    assert!(header.contains(dropdown));
    let panel = desktop.layer_palette.panel_rect;
    let menu = click(&mut desktop, &mut fixture, dropdown);
    assert_eq!(desktop.layer_palette.panel_rect, panel);
    assert!(menu.iter().any(|(text, _)| text == "Project styles"));
    assert!(
        !menu
            .iter()
            .any(|(text, _)| text == "Import global style into project")
    );
    let banner_id = layer_palette::layers_panel_id().with("style-banner");
    let import_body = ctx.read_response(banner_id.with("import")).unwrap().rect;
    let save_body = ctx.read_response(banner_id.with("save")).unwrap().rect;
    assert_eq!(import_body.y_range(), save_body.y_range());
    assert_eq!(import_body.width(), icons::CLOSE_BUTTON_SIZE);
    assert!(import_body.height() >= icons::CLOSE_BUTTON_SIZE);
    assert!((import_body.right() - save_body.left()).abs() < 0.1);
    assert!(!menu.iter().any(|(text, _)| text == &global.name));
    click(&mut desktop, &mut fixture, dropdown);
    let import = ctx
        .read_response(
            layer_palette::layers_panel_id()
                .with("style-banner")
                .with("import"),
        )
        .expect("Direct import icon action")
        .rect
        .center();
    let submenu = click(&mut desktop, &mut fixture, import);
    let before = fixture.editor.document().clone();
    click(&mut desktop, &mut fixture, position(&submenu, &global.name));
    assert_eq!(fixture.icon_styles().len(), 1);
    assert_ne!(fixture.icon_styles()[0].id, global.id);
    assert_eq!(fixture.icon_styles()[0].recipe, global.recipe);
    assert_eq!(
        fixture.editor.document().entities(EntityKind::Achievement),
        before.entities(EntityKind::Achievement)
    );
    assert_eq!(fixture.global_icon_styles(), &[global]);
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
    click(&mut desktop, &mut fixture, save_body.center());
    assert!(
        desktop.style_rename.is_some(),
        "Direct save action opens the naming prompt"
    );
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn compact_layers_start_collapsed_and_keep_one_pixel_dividers_at_each_scale() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for width in [700.0, 1100.0] {
            let mut fixture = Fixture::combined_layers();
            let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
            let achievement = entity["id"].as_str().unwrap().to_owned();
            let top = new_id();
            let bottom = new_id();
            let effect = new_id();
            entity["icon"] = json!({"layers":[
                {"id":bottom,"displayName":"Lower artwork","kind":"Solid","solidColorHex":"#FFFFFFFF"},
                {"id":top,"displayName":"Upper artwork","kind":"Solid","solidColorHex":"#FFFFFFFF",
                    "effects":[{"id":effect,"kind":"Outline","visible":true,"colorHex":"#FFFFFFFF","width":2.0}]}
            ]});
            fixture
                .execute(
                    "Recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            let before = fixture.editor.document().clone();
            let mut desktop = Desktop::default();
            desktop.selection = vec![achievement.clone()];
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            ctx.set_pixels_per_point(scale);
            let mut frame = |desktop: &mut Desktop, events| {
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
                        theme::apply(ui.ctx(), fixture.preferences());
                        desktop.layer_editor(ui, &mut fixture);
                    },
                );
                output.textures_delta.clear();
                output
            };
            frame(&mut desktop, vec![]);
            let collapsed = frame(&mut desktop, vec![]);
            assert!(desktop.layer_palette.expanded_effects.is_empty());
            assert!(!collapsed.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == "Outline")));
            assert!((desktop.layer_palette.rows[&top].height() - 50.0).abs() < 1.0);
            let fx = ctx
                .read_response(layer_palette::layer_control_id(&achievement, &top, "fx"))
                .unwrap()
                .rect
                .center();
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    vec![
                        egui::Event::PointerMoved(fx),
                        egui::Event::PointerButton {
                            pos: fx,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            let output = frame(&mut desktop, vec![]);
            let a = desktop.layer_palette.rows[&top];
            let b = desktop.layer_palette.rows[&bottom];
            assert!(
                ((b.top() - a.bottom()) * scale - 1.0).abs() < 0.15,
                "Expected one physical pixel at {scale}: {a:?}, {b:?}"
            );
            assert!(a.width() <= width * 0.5, "Layer row overflowed its pane");
            assert!(
                (a.height() - b.height() - 30.0 - icons::layer_control_size(icons::Icon::Effects))
                    .abs()
                    < 1.0
            );
            assert!(
                (b.height() - 50.0).abs() < 1.0,
                "Compact artwork row: {b:?}"
            );
            let effect_pos = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Outline" => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .expect("Expanding FX must show the existing effect");
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == "Effects" && a.contains(text.pos))));
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    vec![
                        egui::Event::PointerMoved(effect_pos),
                        egui::Event::PointerButton {
                            pos: effect_pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(desktop.selected_layer.as_ref(), Some(&top));
            assert_eq!(
                desktop.layer_palette.selected_effect.as_ref(),
                Some(&effect)
            );
            assert_eq!(fixture.editor.document(), &before);
        }
    }
}

#[test]
fn layer_controls_fit_thumbnail_height_scale_fx_and_use_filled_hover() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for font_size in [14.0, 22.0] {
            let mut fixture = Fixture::combined_layers();
            let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
            let achievement = entity["id"].as_str().unwrap().to_owned();
            let layer = new_id();
            entity["icon"] =
                json!({"layers":[{"id":layer,"kind":"Gradient","displayName":"Gradient"}]});
            fixture
                .execute(
                    "Recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            let mut desktop = Desktop::default();
            desktop.selection = vec![achievement.clone()];
            desktop.layer_palette.expanded_effects.insert(layer.clone());
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            ctx.set_pixels_per_point(scale);
            let frame = |desktop: &mut Desktop, fixture: &mut Fixture, time, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1100.0, 700.0),
                        )),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), fixture.preferences());
                        ui.set_style(ctx.global_style());
                        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                            ui.style_mut().text_styles.get_mut(&style).unwrap().size = font_size;
                        }
                        desktop.layer_editor(ui, fixture);
                    },
                );
                output.textures_delta.clear();
                output
            };
            let controls = || {
                ["eye", "fx", "add"].map(|control| {
                    ctx.read_response(layer_palette::layer_control_id(
                        &achievement,
                        &layer,
                        control,
                    ))
                    .unwrap()
                    .rect
                })
            };
            frame(&mut desktop, &mut fixture, 0.0, vec![]);
            // A display-scale change first resolves the root's point dimensions.
            frame(&mut desktop, &mut fixture, 0.005, vec![]);
            let rest = frame(&mut desktop, &mut fixture, 0.01, vec![]);
            let bounds = controls();
            let [eye, fx, add] = bounds;
            assert!((eye.center().x - fx.center().x).abs() * scale < 0.6);
            assert!(
                (fx.bottom() - eye.top() - 42.0).abs() * scale < 1.1,
                "Thumbnail alignment: {bounds:?}"
            );
            assert!((fx.width() - eye.width() * 0.85).abs() * scale < 1.1);
            assert!(eye.bottom() <= fx.top(), "Controls overlap: {bounds:?}");
            assert!(
                (add.top() - fx.bottom()).abs() * scale < 1.1
                    && (add.center().x - fx.center().x).abs() * scale < 1.1
                    && (add.size() - fx.size()).length() * scale < 1.1,
                "Add belongs directly below and centered on FX at {scale}/{font_size}: {bounds:?}"
            );
            let row = desktop.layer_palette.rows[&layer];
            assert!(
                (row.bottom() - add.bottom() - 4.0).abs() * scale < 1.1,
                "An empty FX area must only reserve the add square and row margin: {row:?}, {add:?}"
            );
            assert!(
                row.width() < 550.0,
                "Row overflow at {scale}/{font_size}: {row:?}; controls {bounds:?}"
            );
            assert!((eye.left() - row.left() - 6.0).abs() * scale < 1.1);
            let glyph = rest
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "fx" => Some(text),
                    _ => None,
                })
                .unwrap();
            assert!(
                (glyph.pos + glyph.galley.mesh_bounds.center().to_vec2()).distance(fx.center())
                    * scale
                    < 0.6
            );
            assert!(
                glyph
                    .galley
                    .job
                    .sections
                    .iter()
                    .all(|section| section.format.color
                        == theme::color(fixture.preferences(), "Primary text").gamma_multiply(0.6))
            );
            for (index, bound) in bounds.into_iter().enumerate() {
                if index < 2 {
                    assert!(!rest.shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.rect.min.distance(bound.min) * scale < 1.0 && rect.rect.max.distance(bound.max) * scale < 1.0 && rect.fill.a() != 0)), "Resting body at {scale}/{font_size}: {bound:?}");
                } else {
                    assert!(rest.shapes.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Rect(rect) if rect.rect.min.distance(bound.min) * scale < 1.0 && rect.rect.max.distance(bound.max) * scale < 1.0 && rect.fill == theme::color(fixture.preferences(), "Panel") && rect.stroke.color.a() == 0)));
                }
            }
            // Both strokes of the + meet at the actual button center.
            let plus_lines: Vec<_> = rest
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, .. }
                        if points.iter().all(|point| add.contains(*point)) =>
                    {
                        Some(points)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(plus_lines.len(), 2);
            for points in plus_lines {
                assert!(
                    egui::Rect::from_two_pos(points[0], points[1])
                        .center()
                        .distance(add.center())
                        * scale
                        < 0.6
                );
            }
            let mut time = 0.02;
            for (index, bound) in bounds.into_iter().enumerate() {
                frame(
                    &mut desktop,
                    &mut fixture,
                    time,
                    vec![egui::Event::PointerMoved(bound.center())],
                );
                let hover = frame(&mut desktop, &mut fixture, time + 0.3, vec![]);
                assert_eq!(controls(), bounds, "Hover changed the layout");
                let fill = theme::color(fixture.preferences(), "Primary text")
                    .gamma_multiply(if index < 2 { 0.16 } else { 1.0 });
                assert!(hover.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.rect.min.distance(bound.min) * scale < 1.0 && rect.rect.max.distance(bound.max) * scale < 1.0 && rect.fill == fill && rect.stroke.color.a() == 0)));
                time += 0.4;
            }
            frame(
                &mut desktop,
                &mut fixture,
                time,
                vec![egui::Event::PointerGone],
            );
            let idle = frame(&mut desktop, &mut fixture, time + 0.3, vec![]);
            for bound in [eye, fx] {
                assert!(!idle.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.rect.min.distance(bound.min) * scale < 1.0 && rect.rect.max.distance(bound.max) * scale < 1.0 && rect.fill.a() != 0)));
            }
            ctx.memory_mut(|memory| {
                memory.request_focus(layer_palette::layer_control_id(&achievement, &layer, "fx"))
            });
            frame(&mut desktop, &mut fixture, time + 0.4, vec![]);
            let focused = frame(&mut desktop, &mut fixture, time + 0.7, vec![]);
            assert!(focused.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(rect) if rect.rect.min.distance(fx.min) * scale < 1.0 && rect.rect.max.distance(fx.max) * scale < 1.0 && rect.fill == theme::color(fixture.preferences(), "Primary text").gamma_multiply(0.16) && rect.stroke.color.a() == 0)));
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    &mut fixture,
                    time + 0.8,
                    vec![
                        egui::Event::PointerMoved(fx.center()),
                        egui::Event::PointerButton {
                            pos: fx.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            let collapsed = frame(&mut desktop, &mut fixture, time + 0.9, vec![]);
            assert!(!desktop.layer_palette.expanded_effects.contains(&layer));
            assert!(desktop.layer_palette.rows[&layer].height() < row.height());
            // An unrendered response may survive one frame in egui's response cache.
            frame(&mut desktop, &mut fixture, time + 1.0, vec![]);
            assert!(
                ctx.read_response(layer_palette::layer_control_id(&achievement, &layer, "add"))
                    .is_none()
            );
            assert!(!collapsed.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, .. } if points.iter().all(|point| add.contains(*point)))));
        }
    }
}

#[test]
fn eye_drag_paints_initial_visibility_in_both_directions_and_undoes_as_one_edit() {
    for initial in [true, false] {
        for downward in [true, false] {
            let mut fixture = Fixture::new();
            let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
            let achievement = entity["id"].as_str().unwrap().to_owned();
            let ids: Vec<_> = (0..5).map(|_| new_id()).collect();
            let (start, end) = if downward { (3, 1) } else { (1, 3) };
            entity["icon"] = json!({"layers": ids.iter().enumerate().map(|(index, id)| json!({
                "id":id, "kind":"Solid", "displayName":format!("Layer {index}"),
                "visible":if index == start { initial } else { index % 2 == 0 },
                "solidColorHex":"#FFFFFFFF"
            })).collect::<Vec<_>>()});
            fixture
                .execute(
                    "Recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            fixture.finish_group();
            let before = fixture.editor.document().clone();
            let mut desktop = Desktop::default();
            desktop.selection = vec![achievement.clone()];
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1100.0, 800.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), fixture.preferences());
                        desktop.layer_editor(ui, fixture);
                    },
                );
                output.textures_delta.clear();
            };
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let visible = |fixture: &Fixture, index: usize| {
                fixture
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, &achievement)
                    .unwrap()["icon"]["layers"][index]["visible"]
                    .as_bool()
                    .unwrap()
            };
            frame(&mut desktop, &mut fixture, vec![]);
            frame(&mut desktop, &mut fixture, vec![]);
            let eyes: Vec<_> = ids
                .iter()
                .map(|id| {
                    ctx.read_response(layer_palette::layer_control_id(&achievement, id, "eye"))
                        .unwrap()
                        .rect
                        .center()
                })
                .collect();
            frame(
                &mut desktop,
                &mut fixture,
                vec![
                    egui::Event::PointerMoved(eyes[start]),
                    button(eyes[start], true),
                ],
            );
            assert_eq!(
                visible(&fixture, start),
                !initial,
                "The initial press must update immediately"
            );
            // A single fast movement crosses the middle eye without landing on it.
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(eyes[end])],
            );
            for index in 1..=3 {
                assert_eq!(visible(&fixture, index), !initial);
            }
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(eyes[start])],
            );
            for index in 1..=3 {
                assert_eq!(
                    visible(&fixture, index),
                    !initial,
                    "Retracing must not toggle twice"
                );
            }
            // Crossing another layer's name does not paint its visibility.
            let away = eyes[start] + egui::vec2(130.0, 0.0);
            let other_name = egui::pos2(away.x, eyes[0].y);
            frame(
                &mut desktop,
                &mut fixture,
                vec![
                    egui::Event::PointerMoved(away),
                    egui::Event::PointerMoved(other_name),
                    button(other_name, false),
                ],
            );
            assert!(desktop.layer_drag.is_none());
            assert!(visible(&fixture, 0));
            assert!(visible(&fixture, 4));
            let after = fixture.editor.document().clone();
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(eyes[0])],
            );
            assert_eq!(
                fixture.editor.document(),
                &after,
                "Release must end the stroke"
            );
            fixture.undo();
            assert_eq!(fixture.editor.document(), &before);
            fixture.redo();
            assert_eq!(fixture.editor.document(), &after);
            // A normal click remains a single toggle and a separate undo step.
            frame(&mut desktop, &mut fixture, vec![button(eyes[0], true)]);
            frame(&mut desktop, &mut fixture, vec![button(eyes[0], false)]);
            assert!(!visible(&fixture, 0));
            fixture.undo();
            assert_eq!(fixture.editor.document(), &after);
            // A modal disabling the editor ends a held stroke immediately.
            frame(&mut desktop, &mut fixture, vec![button(eyes[0], true)]);
            let held = fixture.editor.document().clone();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 800.0),
                    )),
                    events: vec![egui::Event::PointerMoved(eyes[4])],
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), fixture.preferences());
                    ui.disable();
                    desktop.layer_editor(ui, &mut fixture);
                },
            );
            output.textures_delta.clear();
            assert_eq!(fixture.editor.document(), &held);
            frame(
                &mut desktop,
                &mut fixture,
                vec![egui::Event::PointerMoved(eyes[4]), button(eyes[4], false)],
            );
            assert_eq!(
                fixture.editor.document(),
                &held,
                "Re-enabling must not resume the canceled stroke"
            );
        }
    }
}

#[test]
fn icon_tab_reopens_layers_even_when_already_selected() {
    for floating in [false, true] {
        icon_tab_reopens_layers(floating);
    }
}

fn icon_tab_reopens_layers(floating: bool) {
    let mut fixture = Fixture::new();
    fixture.layout.floating = floating;
    let document = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![
        document.entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .into(),
    ];
    let ctx = egui::Context::default();
    let mut frame = |desktop: &mut Desktop, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
        output
    };
    for _ in 0..2 {
        desktop.layer_palette.open = false;
        desktop.layer_palette.focus_requested = false;
        frame(&mut desktop, vec![]);
        let output = frame(&mut desktop, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if ["Icon", "Ico"].contains(&text.galley.job.text.as_str()) =>
                {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            frame(
                &mut desktop,
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
        let output = frame(&mut desktop, vec![]);
        assert!(desktop.layer_palette.open);
        assert!(desktop.layer_palette.layout.floating);
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Layers"
        )));
    }
    assert_eq!(fixture.editor.document(), &document);
}

#[test]
fn card_icon_reopens_floating_layers_even_with_legacy_docked_settings() {
    let mut fixture = Fixture::new();
    let document = fixture.editor.document().clone();
    let id = document.entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    desktop.layer_palette.open = false;
    fixture.layout.floating = false;
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    let pos = ctx
        .read_response(egui::Id::new(("icon-layers", &id)))
        .unwrap()
        .rect
        .center();
    for pressed in [true, false, true, false] {
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
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
    assert_eq!(desktop.selection, vec![id]);
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    assert!(desktop.layer_palette.open && desktop.layer_palette.layout.floating);
    assert!(desktop.layer_palette.achievement.is_some());
    assert_eq!(fixture.editor.document(), &document);
}

#[test]
fn layers_follow_icon_tab_and_ignore_legacy_docking() {
    assert!(achimeld_application::LayerLayout::default().floating);
    for floating in [false, true] {
        let mut fixture = Fixture::new();
        fixture.layout.floating = floating;
        let document = fixture.editor.document().clone();
        let mut desktop = Desktop::default();
        desktop.selection = vec![
            document.entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .into(),
        ];
        let ctx = egui::Context::default();
        for tab in [
            desktop::InspectorTab::General,
            desktop::InspectorTab::Icon,
            desktop::InspectorTab::Localization,
            desktop::InspectorTab::Icon,
        ] {
            desktop.inspector_tab = tab;
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 920.0),
                    )),
                    ..Default::default()
                },
                |ui| desktop.show(ui, &mut fixture),
            );
            output.textures_delta.clear();
            assert_eq!(
                output.shapes.iter().any(|shape| matches!(
                    &shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Layers"
                )),
                tab == desktop::InspectorTab::Icon
            );
            assert!(desktop.layer_palette.layout.floating);
        }
        assert_eq!(fixture.editor.document(), &document);
    }
}

#[test]
fn icon_button_recovers_a_lost_window_and_sync_is_an_upper_view() {
    let mut fixture = Fixture::new();
    fixture.layout.position = Some([60000.0, -60000.0]);
    let mut desktop = Desktop::default();
    desktop.selection = vec![
        fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .into(),
    ];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    desktop.layer_palette.open = false;
    let ctx = egui::Context::default();
    let parent = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(1440.0, 920.0));
    let mut frame = |desktop: &mut Desktop, events| {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, parent.size())),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .outer_rect = Some(parent);
        let mut output = ctx.run_ui(input, |ui| desktop.show(ui, &mut fixture));
        output.textures_delta.clear();
        output
    };
    frame(&mut desktop, vec![]);
    let labels = |output: &egui::FullOutput, label: &str| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let output = frame(&mut desktop, vec![]);
    assert_eq!(
        labels(&output, "Sync").len(),
        1,
        "Sync belongs only in the upper toolbar"
    );
    let button = labels(&output, "Show layers panel")[0];
    for pressed in [true, false] {
        frame(
            &mut desktop,
            vec![
                egui::Event::PointerMoved(button),
                egui::Event::PointerButton {
                    pos: button,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(desktop.layer_palette.open);
    let position = desktop.layer_palette.layout.position.unwrap();
    assert!(parent.contains(position.into()));
    assert!(parent.contains(egui::pos2(
        position[0] + desktop.layer_palette.layout.size[0],
        position[1] + desktop.layer_palette.layout.size[1]
    )));
    desktop.layer_palette.open = false;
    frame(&mut desktop, vec![]);
    let output = frame(&mut desktop, vec![]);
    let sync = labels(&output, "Sync")[0];
    for pressed in [true, false] {
        frame(
            &mut desktop,
            vec![
                egui::Event::PointerMoved(sync),
                egui::Event::PointerButton {
                    pos: sync,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let output = frame(&mut desktop, vec![]);
    assert!(desktop.view == desktop::View::Sync);
    assert!(labels(&output, "Inspector").is_empty());
    assert!(!labels(&output, "Untitled project · Steam App ID 0").is_empty());
}

#[test]
fn layers_panel_recovers_only_when_entirely_outside_the_app() {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 920.0));
    for (position, recovered) in [([30000.0, -30000.0], true), ([-300.0, 100.0], false)] {
        let mut fixture = Fixture::new();
        fixture.layout.position = Some(position);
        let mut desktop = Desktop::default();
        desktop.selection = vec![
            fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .into(),
        ];
        desktop.inspector_tab = desktop::InspectorTab::Icon;
        let ctx = egui::Context::default();
        // Production disables embedding. Layers must still use only the main viewport.
        ctx.set_embed_viewports(false);
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                },
                |ui| desktop.show(ui, &mut fixture),
            );
            assert_eq!(output.viewport_output.len(), 1);
            assert!(output.viewport_output.contains_key(&egui::ViewportId::ROOT));
            output.textures_delta.clear();
        }
        let rect = desktop.layer_palette.panel_rect.unwrap();
        if recovered {
            assert!(bounds.contains_rect(rect));
            assert!(rect.center().distance(bounds.center()) < 1.0);
        } else {
            assert_eq!(rect.min, egui::Pos2::from(position));
        }
        assert_eq!(fixture.layout.position, Some(rect.min.into()));
    }
}

#[test]
fn layers_panel_stays_above_other_app_windows_and_below_dialogs() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    desktop.selection = vec![
        fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .into(),
    ];
    desktop.open_layers();
    desktop.open_settings(&fixture);
    let ctx = egui::Context::default();
    let panel = egui::LayerId::new(egui::Order::Foreground, layer_palette::layers_panel_id());
    let settings = egui::LayerId::new(egui::Order::Middle, egui::Id::new("Project settings"));
    for _ in 0..2 {
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    }
    for _ in 0..4 {
        ctx.move_to_top(settings);
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        let center = desktop.layer_palette.panel_rect.unwrap().center();
        assert_eq!(ctx.layer_id_at(center), Some(panel));
    }
    desktop.open_about();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    let modal = ctx.memory(|m| m.top_modal_layer()).unwrap();
    assert_eq!(
        ctx.layer_id_at(desktop.layer_palette.panel_rect.unwrap().center()),
        Some(modal)
    );
}

#[test]
fn layers_panel_drag_tracks_pointer_without_flicker_during_pause_reverse_and_release() {
    for max_passes in [1, 2] {
        for target in [
            "top",
            "left",
            "right",
            "bottom",
            "subtitle",
            "title",
            "disabled-paste",
            "disabled-affect-style",
            "body-blank",
            "body-margin",
            "separator",
        ] {
            check_layers_panel_drag(max_passes, target);
        }
    }
}

fn check_layers_panel_drag(max_passes: usize, target: &str) {
    let mut fixture = Fixture::new();
    let document = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![
        document.entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .into(),
    ];
    desktop.open_layers();
    let ctx = egui::Context::default();
    ctx.options_mut(|options| {
        options.max_passes = std::num::NonZeroUsize::new(max_passes).unwrap()
    });
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        let mut header = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                desktop.show(ui, fixture);
                // Read before end_pass swaps the widget registries.
                header = ctx
                    .read_response(layer_palette::layers_panel_id().with("header"))
                    .map(|response| response.rect);
            },
        );
        output.textures_delta.clear();
        header.unwrap()
    };
    for _ in 0..3 {
        frame(&mut desktop, &mut fixture, vec![]);
    }
    let before = desktop.layer_palette.panel_rect.unwrap();
    let header_before = frame(&mut desktop, &mut fixture, vec![]);
    // The compact actions now occupy the header's top edge. Drag its empty title area.
    let title_x = (ctx
        .read_response(layer_palette::layers_panel_id().with("paste"))
        .unwrap()
        .rect
        .right()
        + ctx
            .read_response(layer_palette::layers_panel_id().with("close"))
            .unwrap()
            .rect
            .left())
        * 0.5;
    let press = match target {
        "top" => egui::pos2(title_x, header_before.top() + 2.0),
        "left" => header_before.left_center() + egui::vec2(2.0, 0.0),
        "right" => header_before.right_center() - egui::vec2(5.0, 0.0),
        "bottom" => egui::pos2(title_x, header_before.bottom() - 2.0),
        "body-blank" => before.left_bottom() + egui::vec2(45.0, -90.0),
        "body-margin" => before.left_center() + egui::vec2(2.0, 0.0),
        "separator" => egui::pos2(title_x, header_before.bottom() + 3.0),
        "subtitle" => {
            ctx.read_response(layer_palette::layers_panel_id().with("style-banner"))
                .unwrap()
                .rect
                .center_bottom()
                + egui::vec2(0.0, 10.0)
        }
        "disabled-paste" | "disabled-affect-style" => {
            let control = target.strip_prefix("disabled-").unwrap();
            let response = ctx
                .read_response(layer_palette::layers_panel_id().with(control))
                .unwrap();
            assert!(!response.enabled());
            response.rect.center()
        }
        _ => egui::pos2(title_x, header_before.center().y),
    };
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(press), button(press, true)],
    );
    for delta in [
        egui::vec2(25.0, 15.0),
        egui::vec2(100.0, 60.0),
        egui::vec2(50.0, 30.0),
        egui::Vec2::ZERO,
    ] {
        let mut header = frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerMoved(press + delta)],
        );
        for _ in 0..4 {
            let rect = desktop.layer_palette.panel_rect.unwrap();
            assert!(
                rect.min.distance(before.min + delta) < 1.0,
                "Layers jumped during {target} drag (passes={max_passes}, press={press:?}, header={header_before:?}): {rect:?}, expected {:?}",
                before.translate(delta)
            );
            assert_eq!(rect.size(), before.size());
            assert_eq!(header.center(), header_before.center() + delta);
            header = frame(&mut desktop, &mut fixture, vec![]);
        }
    }
    // Include new movement in the release frame, then ensure the saved geometry is stable.
    let delta = egui::vec2(75.0, 45.0);
    frame(
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::PointerMoved(press + delta),
            button(press + delta, false),
        ],
    );
    for _ in 0..4 {
        let rect = desktop.layer_palette.panel_rect.unwrap();
        assert!(rect.min.distance(before.min + delta) < 1.0);
        assert_eq!(fixture.layout.position, Some(rect.min.into()));
        frame(&mut desktop, &mut fixture, vec![]);
    }
    assert_eq!(fixture.editor.document(), &document);
}

#[test]
fn layers_header_controls_keep_drag_gestures_out_of_the_window_background() {
    for control in [
        "center-icon",
        "style-banner",
        "style-import",
        "style-save",
        "copy",
        "paste",
        "close",
        "layer-row",
        "property-blend",
    ] {
        let mut fixture = Fixture::new();
        let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        let achievement = entity["id"].as_str().unwrap().to_owned();
        entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Solid","visible":true,"solidColorHex":"#FFFFFFFF"}]});
        fixture
            .editor
            .execute(
                "Icon",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        fixture
            .editor
            .execute(
                "Layout preference",
                Command::Preferences(Preferences {
                    layer_properties_in_sidebar: false,
                    ..fixture.preferences().clone()
                }),
                None,
            )
            .unwrap();
        let mut desktop = Desktop::default();
        desktop.selection = vec![achievement];
        desktop.open_layers();
        desktop.recipe_clipboard = Some(json!({"layers": []}));
        let ctx = egui::Context::default();
        for _ in 0..3 {
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        }
        let before = desktop.layer_palette.panel_rect.unwrap();
        let id = match control {
            "style-import" => layer_palette::layers_panel_id()
                .with("style-banner")
                .with("import"),
            "style-save" => layer_palette::layers_panel_id()
                .with("style-banner")
                .with("save"),
            _ => layer_palette::layers_panel_id().with(control),
        };
        let press = if control == "layer-row" {
            desktop.layer_palette.rows[desktop.selected_layer.as_ref().unwrap()].center()
        } else {
            let id = if control == "property-blend" {
                layer_palette::layer_control_id(
                    &desktop.selection[0],
                    desktop.selected_layer.as_ref().unwrap(),
                    "property-blend",
                )
            } else {
                id
            };
            let response = ctx.read_response(id).unwrap();
            assert!(response.enabled(), "The fixture must enable {control}");
            response.rect.center()
        };
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![
                egui::Event::PointerMoved(press),
                egui::Event::PointerButton {
                    pos: press,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![egui::Event::PointerMoved(press + egui::vec2(80.0, 60.0))],
        );
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        assert_eq!(
            desktop.layer_palette.panel_rect,
            Some(before),
            "Dragged {control}"
        );
    }
}

#[test]
fn layers_can_be_recovered_by_dragging_visible_body_when_header_is_offscreen() {
    for (position, delta) in [
        ([1380.0, 150.0], egui::vec2(-1040.0, -40.0)),
        ([300.0, -160.0], egui::vec2(40.0, 300.0)),
    ] {
        let mut fixture = Fixture::new();
        fixture.layout.position = Some(position);
        let mut desktop = Desktop::default();
        desktop.selection = vec![
            fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        ];
        desktop.open_layers();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        }
        let before = desktop.layer_palette.panel_rect.unwrap();
        let press = if position[0] > 1000.0 {
            before.left_center() + egui::vec2(2.0, 0.0)
        } else {
            before.left_bottom() + egui::vec2(45.0, -90.0)
        };
        assert!(ctx.content_rect().contains(press));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![egui::Event::PointerMoved(press), button(press, true)],
        );
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![egui::Event::PointerMoved(press + delta)],
        );
        // The release coordinate owns the final position, even if another
        // pointer movement was batched after it in the same frame.
        draw_editor(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![
                button(press + delta, false),
                egui::Event::PointerMoved(press),
            ],
        );
        for _ in 0..3 {
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        }
        let after = desktop.layer_palette.panel_rect.unwrap();
        assert!(
            after.min.distance(before.min + delta) < 1.0,
            "{before:?} -> {after:?}"
        );
        assert!(ctx.content_rect().contains_rect(after));
        assert_eq!(fixture.layout.position, Some(after.min.into()));
    }
}

#[test]
fn disabled_covered_and_secondary_layer_surface_presses_do_not_move_window() {
    for mode in ["disabled", "covered", "secondary"] {
        let mut fixture = Fixture::new();
        let mut desktop = Desktop::default();
        desktop.selection = vec![
            fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        ];
        desktop.open_layers();
        let ctx = egui::Context::default();
        let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events, enabled: bool| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 920.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    if !enabled {
                        ui.disable();
                    }
                    desktop.show(ui, fixture);
                },
            );
            output.textures_delta.clear();
        };
        for _ in 0..3 {
            frame(&mut desktop, &mut fixture, vec![], true);
        }
        let before = desktop.layer_palette.panel_rect.unwrap();
        if mode == "covered" {
            desktop.open_about();
        }
        let enabled = mode != "disabled";
        for _ in 0..2 {
            frame(&mut desktop, &mut fixture, vec![], enabled);
        }
        let press = before.left_bottom() + egui::vec2(45.0, -90.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: if mode == "secondary" {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            },
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerMoved(press), button(press, true)],
            enabled,
        );
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerMoved(press + egui::vec2(80.0, 60.0))],
            enabled,
        );
        frame(
            &mut desktop,
            &mut fixture,
            vec![button(press + egui::vec2(80.0, 60.0), false)],
            enabled,
        );
        frame(&mut desktop, &mut fixture, vec![], enabled);
        assert_eq!(desktop.layer_palette.panel_rect, Some(before), "{mode}");
    }
}

#[test]
fn layers_panel_drag_reopen_and_app_resize_preserve_reachable_geometry() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    desktop.selection = vec![
        fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .into(),
    ];
    desktop.open_layers();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    }
    let before = desktop.layer_palette.panel_rect.unwrap();
    let title = ctx
        .read_response(layer_palette::layers_panel_id().with("header"))
        .unwrap()
        .rect
        .right_center()
        - egui::vec2(120.0, 0.0);
    let delta = egui::vec2(80.0, 40.0);
    draw_editor(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![
            egui::Event::PointerMoved(title),
            egui::Event::PointerButton {
                pos: title,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    draw_editor(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![egui::Event::PointerMoved(title + delta)],
    );
    draw_editor(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: title + delta,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let moved = desktop.layer_palette.panel_rect.unwrap();
    assert!(
        (moved.min - before.min - delta).length() < 1.0,
        "Panel did not follow title drag: {before:?} -> {moved:?}"
    );
    desktop.layer_palette.open = false;
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.open_layers();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    assert_eq!(desktop.layer_palette.panel_rect.unwrap().min, moved.min);
    assert_eq!(
        desktop.layer_palette.panel_rect.unwrap().size(),
        moved.size()
    );
    // Shrink the app so the saved panel lies entirely beyond its right edge.
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 600.0));
    for _ in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
    }
    assert!(bounds.contains_rect(desktop.layer_palette.panel_rect.unwrap()));
}
#[test]
fn layer_tint_uses_visible_authored_colors_and_alpha_weighted_artwork() {
    let mut layer = json!({"kind":"Gradient","gradient":{"stops":[
        {"offset":0.0,"colorHex":"#FF442A65"}, {"offset":1.0,"colorHex":"#FFE47742"}
    ]}});
    assert_eq!(
        layer_backdrop::source_color(&layer),
        Some(egui::Color32::from_rgb(228, 119, 66))
    );
    layer["gradient"]["stops"][1]["colorHex"] = json!("#00FFFFFF");
    assert_eq!(
        layer_backdrop::source_color(&layer),
        Some(egui::Color32::from_rgb(68, 42, 101))
    );
    assert_eq!(
        layer_backdrop::dominant_color(&[0, 0, 255, 0, 0, 0, 255, 10, 200, 40, 20, 255]),
        Some(egui::Color32::from_rgb(200, 40, 20))
    );
    assert_eq!(layer_backdrop::dominant_color(&[255, 255, 255, 0]), None);
    let mut vignette = json!({"kind":"SdfVignette","sdfVignette":{
        "colorHex":"#FF000000","useGradient":true,"gradient":layer["gradient"]}});
    assert_eq!(
        layer_backdrop::source_color(&vignette),
        layer_backdrop::source_color(&layer)
    );
    vignette["sdfVignette"]["useGradient"] = json!(false);
    assert_eq!(
        layer_backdrop::source_color(&vignette),
        Some(egui::Color32::BLACK)
    );
}

#[test]
fn sdf_vignette_controls_preserve_legacy_defaults_and_both_fills_at_narrow_widths() {
    fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Text(t) => result.push((
                t.galley.text().to_owned(),
                t.pos + t.galley.rect.center().to_vec2(),
            )),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    labels(shape, result);
                }
            }
            _ => {}
        }
    }
    for width in [180.0, 320.0] {
        for scale in [1.0, 1.5, 2.0] {
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            ctx.set_pixels_per_point(scale);
            let mut fixture = Fixture::new();
            let original =
                json!({"colorHex":"#FF12AB34","roundness":0.84,"inset":0.0,"softness":1.0});
            let mut settings = original.clone();
            let frame = |fixture: &mut Fixture, settings: &mut Value, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1200.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), fixture.preferences());
                        crate::vignette::properties(ui, fixture, settings);
                    },
                );
                let mut result = Vec::new();
                for shape in &output.shapes {
                    labels(&shape.shape, &mut result);
                }
                output.textures_delta.clear();
                result
            };
            let click = |fixture: &mut Fixture, settings: &mut Value, pos| {
                for pressed in [true, false] {
                    frame(
                        fixture,
                        settings,
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
                frame(fixture, settings, vec![])
            };
            let position = |labels: &[(String, egui::Pos2)], text: &str| {
                labels
                    .iter()
                    .find(|(s, _)| s == text)
                    .unwrap_or_else(|| panic!("Missing {text}: {labels:?}"))
                    .1
            };
            frame(&mut fixture, &mut settings, vec![]);
            let initial = frame(&mut fixture, &mut settings, vec![]);
            assert_eq!(
                settings, original,
                "Painting must not rewrite legacy settings"
            );
            click(
                &mut fixture,
                &mut settings,
                position(&initial, "Invert vignette"),
            );
            assert_eq!(settings["invert"], true);
            let menu = click(&mut fixture, &mut settings, position(&initial, "Color"));
            let mapped = click(&mut fixture, &mut settings, position(&menu, "Gradient map"));
            assert_eq!(settings["useGradient"], true);
            let gradient = settings["gradient"].clone();
            assert_eq!(gradient["stops"].as_array().unwrap().len(), 3);
            assert!(
                !mapped
                    .iter()
                    .any(|(s, _)| s == "Angle" || s == "Gradient kind" || s.starts_with('#'))
            );
            let menu = click(
                &mut fixture,
                &mut settings,
                position(&mapped, "Gradient map"),
            );
            let color_option = menu.iter().rfind(|(text, _)| text == "Color").unwrap().1;
            let solid = click(&mut fixture, &mut settings, color_option);
            assert_eq!(settings["useGradient"], false);
            assert_eq!(settings["colorHex"], original["colorHex"]);
            assert_eq!(settings["gradient"], gradient);
            let menu = click(&mut fixture, &mut settings, position(&solid, "Color"));
            click(&mut fixture, &mut settings, position(&menu, "Gradient map"));
            assert_eq!(settings["gradient"], gradient);
        }
    }
}

#[test]
#[ignore = "Native screenshot of the in-app Layers panel; dedicated external output path"]
fn native_layers_properties_screenshot() {
    layers_window_fixture(false, false, false);
}

#[test]
#[ignore = "Native screenshot of SDF vignette controls; dedicated external output path"]
fn native_sdf_vignette_screenshot() {
    layers_window_fixture(true, false, false);
}

#[test]
#[ignore = "Native screenshot of the restored gradient editor; dedicated external output path"]
fn native_gradient_editor_screenshot() {
    layers_window_fixture(false, true, false);
}

#[test]
#[ignore = "Native first-click Text FX screenshot; dedicated external output path"]
fn native_text_fx_screenshot() {
    layers_window_fixture(false, false, true);
}

fn layers_window_fixture(vignette: bool, gradient_edit: bool, text_fx: bool) {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        frames: usize,
        output: std::path::PathBuf,
        text_fx: Option<(String, String)>,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
            if let Some((achievement, layer)) = &self.text_fx {
                input.focused = true;
                if [2, 3].contains(&self.frames) {
                    let pos = ctx
                        .read_response(layer_palette::layer_control_id(achievement, layer, "fx"))
                        .expect("Text FX control")
                        .rect
                        .center();
                    input.events.extend([
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: self.frames == 2,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]);
                }
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.desktop.show(ui, &mut self.fixture);
            self.frames += 1;
            if self.frames == 8 {
                if let Some((achievement, layer)) = &self.text_fx {
                    let response = ui
                        .ctx()
                        .read_response(layer_palette::layer_control_id(achievement, layer, "fx"))
                        .unwrap();
                    assert!(
                        egui::Popup::is_id_open(
                            ui.ctx(),
                            egui::Popup::default_response_id(&response)
                        ),
                        "Text FX must remain responsive with its menu open"
                    );
                }
                ui.ctx().send_viewport_cmd_to(
                    egui::ViewportId::ROOT,
                    egui::ViewportCommand::Screenshot(egui::UserData::default()),
                );
            }
            if let Some(image) = ui.ctx().input_for(egui::ViewportId::ROOT, |input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let bytes: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    &self.output,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                ui.ctx()
                    .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
            assert!(
                self.frames < 128,
                "Native Layers screenshot was not received"
            );
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
    }
    let mut fixture = Fixture::new();
    fixture.layout.floating = true;
    if gradient_edit {
        fixture.layout.size[1] = 720.0;
    }
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let achievement = entity["id"].as_str().unwrap().to_owned();
    let gradient = new_id();
    let shape = new_id();
    let folder = new_id();
    let vignette_id = new_id();
    entity["icon"] = json!({"layers":[
        {"id":gradient,"displayName":"Twilight gradient","kind":"Gradient","visible":true,"opacity":1.0,
            "gradient":{"kind":"Linear","angle":45.0,"stops":[{"offset":0.0,"colorHex":"#FF4672DE"},{"offset":1.0,"colorHex":"#FF9B70E8"}]}},
        {"id":shape,"displayName":"Golden emblem","kind":"Shape","parentFolderId":folder,"visible":true,"opacity":1.0,
            "shape":{"kind":"Star","scaleX":0.65,"scaleY":0.65,"vertexCount":5,"innerRadius":0.5,"colorHex":"#FFECA44B"},
            "effects":[
                {"id":new_id(),"kind":"Outline","visible":true,"colorHex":"#FFFFFFFF","width":4.0},
                {"id":new_id(),"kind":"InnerLight","visible":true,"colorHex":"#FFFFFFFF","opacity":0.6,"width":3.0,"blur":2.0}]},
        {"id":folder,"displayName":"Emblem folder","kind":"Folder","visible":true,"opacity":1.0,
            "effects":[{"id":new_id(),"kind":"Shadow","visible":true,"colorHex":"#FF000000","opacity":0.6,"blur":4.0}]}
    ]});
    if vignette {
        entity["icon"]["layers"].as_array_mut().unwrap().push(json!({
            "id":vignette_id,"displayName":"Colorful vignette","kind":"SdfVignette","visible":true,
            "sdfVignette":{"useGradient":true,"invert":false,"roundness":0.84,"inset":0.15,"softness":1.0,
                "gradient":{"kind":"Linear","angle":90.0,"stops":[
                    {"offset":0.0,"colorHex":"#FF36D1DC"},{"offset":0.5,"colorHex":"#FF7C4DFF"},{"offset":1.0,"colorHex":"#FFFF4FA3"}]}}
        }));
    }
    if text_fx {
        entity["icon"]["layers"] = json!([{
            "id":shape,"displayName":"Text","kind":"Text","visible":true,"opacity":1.0,
            "text":{"content":"Text","fontFamily":"Segoe UI","fontSize":36.0},"effects":[]
        }]);
    }
    fixture
        .execute(
            "Icon",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: achievement.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    if vignette || gradient_edit {
        let image = achimeld_imaging::render(
            fixture.editor.document(),
            &achievement,
            Some((512, 512)),
            &mut NoAssets,
        )
        .unwrap()
        .achieved;
        fixture.previews.insert(
            achievement.clone(),
            achimeld_application::Preview {
                width: 512,
                height: 512,
                source_size: [512, 512],
                rgba: image.into_raw().into(),
                fingerprint: "vignette-fixture".into(),
            },
        );
    }
    let mut desktop = Desktop::default();
    desktop.selection = vec![achievement.clone()];
    desktop.selected_layer = Some(if vignette {
        vignette_id
    } else if gradient_edit {
        gradient
    } else {
        shape.clone()
    });
    if !vignette && !text_fx {
        desktop
            .layer_palette
            .expanded_effects
            .extend([folder, shape.clone()]);
    }
    desktop.open_layers();
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    ))
    .unwrap();
    let boundary = std::path::absolute(crate::test_output::root()).unwrap();
    assert!(output.starts_with(boundary) && output.extension().is_some_and(|ext| ext == "png"));
    eframe::run_native(
        "Achimeld Layers fixture",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_title(crate::window_title())
                .with_inner_size([1440.0, 920.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_embed_viewports(false);
            cc.egui_ctx
                .options_mut(|options| options.max_passes = std::num::NonZeroUsize::MIN);
            Ok(Box::new(Window {
                desktop,
                fixture,
                frames: 0,
                output,
                text_fx: text_fx.then_some((achievement, shape)),
            }))
        }),
    )
    .unwrap();
}
