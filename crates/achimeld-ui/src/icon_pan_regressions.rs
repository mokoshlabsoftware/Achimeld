use super::*;
use egui::{Pos2, Rect, Vec2};

#[test]
fn visible_canvas_uses_largest_uncovered_rectangle_on_every_side() {
    let rect =
        |x, y, width, height| Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height));
    let canvas = rect(100.0, 80.0, 1000.0, 800.0);
    for (panel, expected) in [
        (
            rect(850.0, 0.0, 500.0, 1000.0),
            rect(100.0, 80.0, 750.0, 800.0),
        ),
        (
            rect(0.0, 0.0, 350.0, 1000.0),
            rect(350.0, 80.0, 750.0, 800.0),
        ),
        (
            rect(0.0, 80.0, 1200.0, 200.0),
            rect(100.0, 280.0, 1000.0, 600.0),
        ),
        (
            rect(0.0, 680.0, 1200.0, 300.0),
            rect(100.0, 80.0, 1000.0, 600.0),
        ),
        (
            rect(450.0, 280.0, 200.0, 400.0),
            rect(650.0, 80.0, 450.0, 800.0),
        ),
        (
            rect(1050.0, 830.0, 500.0, 500.0),
            rect(100.0, 80.0, 950.0, 800.0),
        ),
        (rect(1100.0, 80.0, 300.0, 800.0), canvas),
    ] {
        let visible = canvas::uncovered_canvas(canvas, Some(panel)).unwrap();
        assert_eq!(visible, expected);
        assert!(!visible.intersect(panel).is_positive());
        assert!(canvas.contains_rect(visible));
    }
    assert_eq!(canvas::uncovered_canvas(canvas, None), Some(canvas));
    assert_eq!(canvas::uncovered_canvas(canvas, Some(canvas)), None);
}

fn canvas_frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    time: f64,
    events: Vec<egui::Event>,
    action: Option<canvas::CanvasAction>,
) -> Rect {
    let preferences = fixture.preferences().clone();
    let mut rect = Rect::NOTHING;
    let mut action = action;
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            events,
            ..Default::default()
        },
        |ui| {
            rect = ui.available_rect_before_wrap();
            desktop.canvas(ui, fixture, &preferences, action.take());
            ui.ctx().request_discard("Icon pan layout regression");
        },
    );
    output.textures_delta.clear();
    rect
}

#[test]
fn icon_pan_centers_artwork_in_four_tenths_at_current_zoom_and_undoes_once() {
    for zoom in [0.25, 1.6] {
        let mut fixture = Fixture::new();
        let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        entity["canvasColumn"] = json!(120);
        entity["canvasRow"] = json!(-80);
        fixture
            .execute(
                "Position",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        fixture
            .execute(
                "Zoom",
                Command::CanvasCamera {
                    kind: EntityKind::Achievement,
                    zoom,
                    offset_x: -30.0,
                    offset_y: -30.0,
                },
                None,
            )
            .unwrap();
        let ctx = egui::Context::default();
        let mut desktop = Desktop::default();
        desktop.selection = vec![id.clone()];
        desktop.open_layers();
        desktop.layer_palette.panel_rect = Some(Rect::from_min_size(
            egui::pos2(900.0, 100.0),
            egui::vec2(500.0, 700.0),
        ));
        canvas_frame(&ctx, &mut desktop, &mut fixture, 1.0, vec![], None);
        let rect = canvas_frame(&ctx, &mut desktop, &mut fixture, 1.5, vec![], None);
        let visible = canvas::uncovered_canvas(rect, desktop.layer_palette.panel_rect).unwrap();
        let center = egui::vec2(120.0 * 20.0, -80.0 * 20.0)
            + Vec2::splat(if zoom > 0.5 { 58.0 } else { 59.0 });
        let destination = visible.center() - rect.min - center * zoom as f32;
        let before = fixture.editor.document().clone();
        let start = desktop.pan;
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.0, vec![], None);
        desktop.start_icon_pan(&ctx, &mut fixture, id.clone());
        assert_eq!(desktop.pan, start, "Animation must start without jumping");
        assert!(desktop.automation_busy());
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.02, vec![], None);
        let travel = (destination - start).length();
        assert!(
            (desktop.pan - start).length() < travel * 0.002,
            "The first 5% of the duration must ease gently out of rest"
        );
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.1, vec![], None);
        assert!((desktop.pan - start).length() < travel * 0.125);
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.2, vec![], None);
        assert!((desktop.pan - (start + (destination - start) * 0.5)).length() < 0.01);
        assert!(desktop.camera_editing && desktop.icon_pan.is_some());
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.3, vec![], None);
        assert!((desktop.pan - destination).length() < travel * 0.125);
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.38, vec![], None);
        assert!(
            (desktop.pan - destination).length() < travel * 0.002,
            "The last 5% of the duration must ease gently into rest"
        );
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.399, vec![], None);
        assert!(desktop.icon_pan.is_some());
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.401, vec![], None);
        assert!((desktop.pan - destination).length() < 0.01);
        assert_eq!(desktop.zoom, zoom as f32);
        assert!(desktop.icon_pan.is_none() && !desktop.camera_editing);
        // read_response observes the previous layout pass; settle once more to
        // inspect the hit target at the final painted camera position.
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.42, vec![], None);
        let art = ctx
            .read_response(egui::Id::new(("icon-layers", &id)))
            .unwrap()
            .rect;
        assert!(
            art.center().distance(visible.center()) < 0.01,
            "zoom={zoom}, art={art:?}, visible={visible:?}, pan={:?}",
            desktop.pan
        );
        assert!(visible.contains_rect(art));
        let after = fixture.editor.document().clone();
        fixture.editor.undo();
        assert_eq!(
            fixture.editor.document(),
            &before,
            "One undo must restore the complete pan"
        );
        fixture.editor.redo();
        assert_eq!(fixture.editor.document(), &after);
    }
}

#[test]
fn selected_icon_double_click_centers_once_and_ignores_repeat_activations_until_done() {
    for max_passes in [1, 2] {
        let mut fixture = Fixture::new();
        let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = fixture.editor.document().clone();
        let ctx = egui::Context::default();
        ctx.options_mut(|options| options.max_passes = max_passes.try_into().unwrap());
        let mut desktop = Desktop::default();
        desktop.selection = vec![id.clone()];
        desktop.open_layers();
        for time in [1.0, 1.5] {
            canvas_frame(&ctx, &mut desktop, &mut fixture, time, vec![], None);
        }
        let icon_id = egui::Id::new(("icon-layers", &id));
        {
            let mut click = |desktop: &mut Desktop, time| {
                let pos = ctx.read_response(icon_id).unwrap().rect.center();
                for (time, pressed) in [(time, true), (time + 0.01, false)] {
                    canvas_frame(
                        &ctx,
                        desktop,
                        &mut fixture,
                        time,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        None,
                    );
                }
            };
            click(&mut desktop, 2.0);
            assert!(
                desktop.icon_pan.is_none(),
                "A single click must only select"
            );
            click(&mut desktop, 2.05);
            assert!(
                desktop.icon_pan.is_some(),
                "Double-click must start centering"
            );
            // Follow the moving target and click again during the same animation.
            for time in [2.1, 2.15, 2.2] {
                click(&mut desktop, time);
                assert!(
                    desktop.icon_pan.is_some(),
                    "Repeat clicks must not cancel the pan"
                );
            }
        }
        // All callers, including the Layers header thumbnail, share the same guard.
        desktop.start_icon_pan(&ctx, &mut fixture, id.clone());
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.459, vec![], None);
        assert!(desktop.icon_pan.is_some());
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.461, vec![], None);
        assert!(
            desktop.icon_pan.is_none(),
            "Repeats must not extend the original deadline"
        );
        assert!(desktop.layers_available());
        assert_eq!(desktop.selection, vec![id.clone()]);
        fixture.editor.undo();
        assert_eq!(
            fixture.editor.document(),
            &before,
            "Repeated activations must keep one undo step"
        );
        fixture.editor.redo();
        canvas_frame(&ctx, &mut desktop, &mut fixture, 3.0, vec![], None);
        desktop.start_icon_pan(&ctx, &mut fixture, id);
        assert!(
            desktop.icon_pan.is_some(),
            "Centering must be available again after completion"
        );
    }
}

#[test]
fn icon_pan_stops_for_user_navigation_selection_and_external_camera_changes() {
    for interrupt in ["escape", "zoom", "pointer", "selection", "camera", "close"] {
        let mut fixture = Fixture::new();
        let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let ctx = egui::Context::default();
        let mut desktop = Desktop::default();
        desktop.selection = vec![id.clone()];
        desktop.open_layers();
        canvas_frame(&ctx, &mut desktop, &mut fixture, 1.0, vec![], None);
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.0, vec![], None);
        desktop.start_icon_pan(&ctx, &mut fixture, id);
        let rect = canvas_frame(&ctx, &mut desktop, &mut fixture, 2.1, vec![], None);
        let pan = desktop.pan;
        let mut events = vec![];
        let mut action = None;
        match interrupt {
            "escape" => events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }),
            "zoom" => action = Some(canvas::CanvasAction::ZoomIn),
            "pointer" => events.extend([
                egui::Event::PointerMoved(egui::pos2(400.0, 700.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(400.0, 700.0),
                    button: egui::PointerButton::Middle,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]),
            "selection" => desktop.selection.clear(),
            "close" => desktop.close_layers(),
            "camera" => {
                fixture
                    .execute(
                        "External camera",
                        Command::CanvasCamera {
                            kind: EntityKind::Achievement,
                            zoom: 1.0,
                            offset_x: 123.0,
                            offset_y: 456.0,
                        },
                        None,
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        canvas_frame(&ctx, &mut desktop, &mut fixture, 2.2, events, action);
        assert!(
            desktop.icon_pan.is_none(),
            "{interrupt} must stop animation"
        );
        let expected = if interrupt == "camera" {
            egui::vec2(-123.0, -456.0)
        } else if interrupt == "zoom" {
            // Zoom cancels the animation and keeps the camera center fixed.
            let center = rect.size() * 0.5;
            center - (center - pan) * 1.2
        } else {
            pan
        };
        assert_eq!(desktop.pan, expected);
        canvas_frame(&ctx, &mut desktop, &mut fixture, 3.0, vec![], None);
        assert_eq!(desktop.pan, expected, "Cancelled pan must not restart");
    }
}

#[test]
fn layers_thumbnail_has_even_framing_and_click_starts_pan() {
    let mut fixture = Fixture::new();
    fixture.layout.position = Some([900.0, 100.0]);
    fixture.layout.size = [450.0, 600.0];
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    fixture.previews.insert(
        id.clone(),
        Preview {
            width: 1,
            height: 1,
            source_size: [1, 1],
            rgba: vec![255, 0, 255, 255].into(),
            fingerprint: "header-preview".into(),
        },
    );
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    desktop.open_layers();
    let ctx = egui::Context::default();
    let mut frame = |desktop: &mut Desktop, time, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 1000.0))),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
        output
    };
    frame(&mut desktop, 1.0, vec![]);
    let output = frame(&mut desktop, 1.1, vec![]);
    let button = ctx
        .read_response(layer_palette::layers_panel_id().with("center-icon"))
        .unwrap()
        .rect;
    let texture = desktop.textures[&id].1.id();
    let art = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh)
                if mesh.texture_id == texture && button.contains_rect(mesh.calc_bounds()) =>
            {
                Some(mesh.calc_bounds())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(art.center(), button.center());
    assert_eq!(art.size(), button.shrink(4.0).size());
    assert_eq!(button.size(), Vec2::splat(icons::CLOSE_BUTTON_SIZE));
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if rect.rect == button && rect.stroke.width > 0.0)));
    let pan = desktop.pan;
    for (time, pressed) in [(2.0, true), (2.02, false)] {
        frame(
            &mut desktop,
            time,
            vec![
                egui::Event::PointerMoved(button.center()),
                egui::Event::PointerButton {
                    pos: button.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(desktop.icon_pan.is_some());
    frame(&mut desktop, 2.04, vec![]);
    for (time, pressed) in [(2.1, true), (2.12, false), (2.18, true), (2.2, false)] {
        frame(
            &mut desktop,
            time,
            vec![
                egui::Event::PointerMoved(button.center()),
                egui::Event::PointerButton {
                    pos: button.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(desktop.icon_pan.is_some());
    }
    frame(&mut desktop, 2.24, vec![]);
    assert_ne!(desktop.pan, pan);
    frame(&mut desktop, 2.421, vec![]);
    assert!(desktop.icon_pan.is_none());
    frame(&mut desktop, 2.46, vec![]);
    let canvas = desktop
        .image_drop
        .zones
        .iter()
        .find(|(_, _, zone, _)| *zone == image_drop::DropZone::Canvas)
        .unwrap()
        .1;
    let visible = canvas::uncovered_canvas(canvas, desktop.layer_palette.panel_rect).unwrap();
    let art = ctx
        .read_response(egui::Id::new(("icon-layers", &id)))
        .unwrap()
        .rect;
    assert!(art.center().distance(visible.center()) < 0.01);
}

#[test]
fn style_header_fades_and_slides_in_two_tenths_without_changing_height() {
    for max_passes in [1, 2] {
        let mut fixture = Fixture::new();
        fixture.layout.position = Some([700.0, 100.0]);
        fixture.layout.size = [600.0, 600.0];
        let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let recipe = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]});
        entity["icon"] = recipe.clone();
        fixture
            .editor
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        let style = achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Shared".into(),
            recipe,
        };
        fixture.save_icon_style(style.clone()).unwrap();
        fixture.previews.insert(
            id.clone(),
            Preview {
                width: 1,
                height: 1,
                source_size: [1, 1],
                rgba: vec![255, 0, 0, 128].into(),
                fingerprint: id.clone(),
            },
        );
        let before = fixture.editor.document().clone();
        let mut desktop = Desktop::default();
        desktop.selection = vec![id.clone()];
        desktop.open_layers();
        let ctx = egui::Context::default();
        ctx.options_mut(|options| options.max_passes = max_passes.try_into().unwrap());
        let mut time = 0.0;
        let mut frame = |desktop: &mut Desktop| {
            time += 0.02;
            let mut rectangles = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 1000.0))),
                    ..Default::default()
                },
                |ui| {
                    desktop.show(ui, &mut fixture);
                    // Read current geometry before end_pass swaps widget maps.
                    rectangles = Some((
                        ctx.read_response(layer_palette::layers_panel_id().with("style-banner"))
                            .unwrap()
                            .rect,
                        ctx.read_response(layer_palette::layers_panel_id().with("header"))
                            .unwrap()
                            .rect,
                    ));
                },
            );
            output.textures_delta.clear();
            let (banner, header) = rectangles.unwrap();
            let texture = desktop.textures[&id].1.id();
            let alpha = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh)
                    if mesh.texture_id == texture
                        && (mesh.calc_bounds().height() - (icons::CLOSE_BUTTON_SIZE - 8.0))
                            .abs()
                            < 0.1 =>
                {
                    Some(mesh.vertices[0].color.a())
                }
                _ => None,
            });
            (banner, header, alpha)
        };
        for _ in 0..24 {
            frame(&mut desktop);
        }
        let rest = frame(&mut desktop);
        let thumbnail_left = ctx
            .read_response(layer_palette::layers_panel_id().with("center-icon"))
            .unwrap()
            .rect
            .left();
        assert_eq!(rest.2, Some(255));
        desktop.layer_palette.active_style = Some(style.id.clone());
        let first = frame(&mut desktop);
        assert!(
            desktop.layer_palette.active_style.is_some(),
            "Style mode was cleared"
        );
        assert!(
            first.0.left() < rest.0.left() && first.0.left() > thumbnail_left,
            "passes={max_passes}: rest={rest:?}, first={first:?}, thumbnail_left={thumbnail_left}"
        );
        assert!(first.2.unwrap() < 255 && first.2.unwrap() > 0);
        for _ in 0..4 {
            frame(&mut desktop);
        }
        let middle = frame(&mut desktop);
        assert!(middle.0.left() < first.0.left());
        assert!(middle.2.unwrap() < first.2.unwrap());
        // Reverse while moving, then finish entering style mode.
        desktop.layer_palette.active_style = None;
        let reverse = frame(&mut desktop);
        assert!(reverse.0.left() > middle.0.left() && reverse.0.left() < rest.0.left());
        assert!(reverse.2.unwrap() > middle.2.unwrap());
        desktop.layer_palette.active_style = Some(style.id);
        for _ in 0..11 {
            frame(&mut desktop);
        }
        let active = frame(&mut desktop);
        assert!((active.0.left() - thumbnail_left).abs() < 0.1);
        assert_eq!(active.2, None);
        assert_eq!(active.1.top(), rest.1.top());
        assert_eq!(active.1.height(), rest.1.height());
        desktop.layer_palette.active_style = None;
        let leaving = frame(&mut desktop);
        assert!(leaving.0.left() > active.0.left() && leaving.0.left() < rest.0.left());
        assert!(leaving.2.unwrap() > 0 && leaving.2.unwrap() < 255);
        for _ in 0..10 {
            frame(&mut desktop);
        }
        let restored = frame(&mut desktop);
        assert_eq!(restored.0, rest.0);
        assert_eq!(restored.2, Some(255));
        assert_eq!(fixture.editor.document(), &before);
    }
}
