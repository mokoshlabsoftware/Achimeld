use super::*;
use crate::image_drop::DropZone;
use achimeld_application::ImageImportTarget;
use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};
#[derive(Debug)]
struct File(std::path::PathBuf);
impl egui::DroppedFile for File {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        panic!("Presentation must not read dropped artwork")
    }
}
fn frame(
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    ctx: &egui::Context,
    events: Vec<Event>,
    drop: bool,
) -> egui::FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 920.0))),
        events,
        ..Default::default()
    };
    if drop {
        input
            .dropped_files
            .push(std::sync::Arc::new(File("C:/art/first.png".into())));
    }
    let mut out = ctx.run_ui(input, |ui| desktop.show(ui, fixture));
    out.textures_delta.clear();
    out
}
fn click(desktop: &mut Desktop, fixture: &mut Fixture, ctx: &egui::Context, pos: Pos2) {
    for pressed in [true, false] {
        frame(
            desktop,
            fixture,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
    }
}
fn label(shape: &egui::Shape, name: &str) -> Option<Pos2> {
    match shape {
        egui::Shape::Text(t) if crate::fields::label_matches(t.galley.text(), name) => {
            Some(t.pos + t.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(v) => v.iter().find_map(|s| label(s, name)),
        _ => None,
    }
}
fn button(out: &egui::FullOutput, name: &str) -> Pos2 {
    out.shapes
        .iter()
        .find_map(|s| label(&s.shape, name))
        .unwrap_or_else(|| panic!("Missing {name}"))
}
#[test]
fn right_click_creates_a_local_style_achievement_with_general_and_style_tabs() {
    for sidebar in [true, false] {
        check_local_style_creation(sidebar);
    }
}

fn check_local_style_creation(sidebar: bool) {
    for target in ["background", "card", "icon"] {
        for passes in [1, 2] {
            let mut fixture = if sidebar {
                Fixture::new()
            } else {
                Fixture::combined_layers()
            };
            let mut desktop = Desktop::default();
            let ctx = egui::Context::default();
            ctx.options_mut(|options| options.max_passes = passes.try_into().unwrap());
            frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let pos = match target {
                "card" => {
                    ctx.read_response(egui::Id::new(("node", &id)))
                        .unwrap()
                        .rect
                        .right_center()
                        - Vec2::new(20.0, 0.0)
                }
                "icon" => ctx
                    .read_response(egui::Id::new(("icon-layers", &id)))
                    .unwrap()
                    .rect
                    .center(),
                _ => egui::pos2(300.0, 600.0),
            };
            let before = fixture.editor.document().clone();
            for pressed in [true, false] {
                frame(
                    &mut desktop,
                    &mut fixture,
                    &ctx,
                    vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Secondary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    false,
                );
            }
            let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            click(
                &mut desktop,
                &mut fixture,
                &ctx,
                button(&out, "Create icon style node"),
            );
            let after = fixture.editor.document().clone();
            let local = after.entities(EntityKind::Achievement).last().unwrap();
            assert!(achimeld_application::style_achievements::is_style_achievement(local));
            assert_eq!(desktop.selection, [local["id"].as_str().unwrap()]);
            assert!(desktop.layers_available());
            let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            assert_eq!(desktop.layer_palette.properties_rect.is_some(), sidebar);
            let expected = if sidebar {
                ["General", "Style", "Opacity"]
            } else {
                ["General", "Style", "Show layers panel"]
            };
            for expected in expected {
                button(&out, expected);
            }
            for absent in [
                "Preview inputs",
                "U. Value",
                "Localization",
                "S. API Name",
                "Affect style",
            ] {
                assert!(
                    out.shapes
                        .iter()
                        .all(|shape| label(&shape.shape, absent).is_none())
                );
            }
            click(&mut desktop, &mut fixture, &ctx, button(&out, "General"));
            let general = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            assert_eq!(desktop.inspector_tab, desktop::InspectorTab::General);
            for expected in ["General", "Style", "U. Value", "L. Style"] {
                button(&general, expected);
            }
            frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            assert_eq!(desktop.inspector_tab, desktop::InspectorTab::General);
            click(&mut desktop, &mut fixture, &ctx, button(&general, "Style"));
            assert_eq!(desktop.inspector_tab, desktop::InspectorTab::Icon);
            assert_eq!(fixture.editor.document(), &after);
            fixture.undo();
            assert_eq!(fixture.editor.document(), &before);
            fixture.redo();
            assert_eq!(fixture.editor.document(), &after);
        }
    }
}

#[test]
fn style_node_general_changes_unlock_and_explicit_style_link_with_undo() {
    for sidebar in [true, false] {
        let mut fixture = if sidebar {
            Fixture::new()
        } else {
            Fixture::combined_layers()
        };
        for position in [[0, 12], [20, 12]] {
            fixture
                .execute(
                    "Create node",
                    Command::AddStyleAchievementAt { position },
                    None,
                )
                .unwrap();
        }
        let nodes = fixture.editor.document().entities(EntityKind::Achievement);
        let node = nodes[nodes.len() - 2].clone();
        let other = nodes.last().unwrap().clone();
        let id = node["id"].as_str().unwrap().to_owned();
        let styles = fixture.icon_styles();
        let first = styles.iter().find(|s| node["iconStyleId"] == s.id).unwrap();
        let second = styles
            .iter()
            .find(|s| other["iconStyleId"] == s.id)
            .unwrap();
        let mut desktop = Desktop::default();
        desktop.selection = vec![id.clone()];
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        click(&mut desktop, &mut fixture, &ctx, button(&out, "10.0"));
        let key = |key, modifiers| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![
                key(egui::Key::A, egui::Modifiers::CTRL),
                Event::Text("27.5".into()),
                key(egui::Key::Enter, egui::Modifiers::NONE),
            ],
            false,
        );
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        assert_eq!(
            fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &id)
                .unwrap()["unlockValue"],
            27.5
        );
        let before_link = fixture.editor.document().clone();
        let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        click(&mut desktop, &mut fixture, &ctx, button(&out, &first.name));
        let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        click(&mut desktop, &mut fixture, &ctx, button(&out, &second.name));
        let linked = fixture.editor.document().clone();
        let changed = linked.entity(EntityKind::Achievement, &id).unwrap();
        assert_eq!(changed["iconStyleId"], second.id);
        assert_eq!(changed["icon"], second.recipe);
        assert_eq!(changed["unlockValue"], 27.5);
        assert_eq!(linked.icon_styles(), styles);
        fixture.undo();
        assert_eq!(fixture.editor.document(), &before_link);
        fixture.redo();
        assert_eq!(fixture.editor.document(), &linked);
        assert_eq!(
            achimeld_application::Document::from_parts(
                linked.project().clone(),
                linked.catalog().clone()
            )
            .unwrap(),
            linked
        );
    }
}

#[test]
fn drops_choose_canvas_card_general_and_layers_without_falling_through() {
    for zone in [
        DropZone::Canvas,
        DropZone::Achievement(String::new()),
        DropZone::General(String::new()),
    ] {
        let mut fixture = Fixture::new();
        let mut desktop = Desktop::default();
        let ctx = egui::Context::default();
        let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        desktop.selection = vec![id.clone()];
        let before = fixture.editor.document().clone();
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        let mut pos = desktop
            .image_drop
            .zones
            .iter()
            .find_map(|(_, rect, z, _)| {
                let matches = matches!(
                    (&zone, z),
                    (DropZone::Canvas, DropZone::Canvas)
                        | (DropZone::Achievement(_), DropZone::Achievement(_))
                        | (DropZone::General(_), DropZone::General(_))
                );
                (matches && rect.is_positive()).then_some(if *z == DropZone::Canvas {
                    rect.right_bottom() - Vec2::splat(70.0)
                } else {
                    rect.center()
                })
            })
            .expect("Drop zone must be visible");
        if matches!(zone, DropZone::Achievement(_)) {
            pos = ctx
                .read_response(egui::Id::new(("node", &id)))
                .unwrap()
                .rect
                .right_center()
                - Vec2::new(20.0, 0.0);
        }
        let out = frame(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![Event::PointerMoved(pos)],
            true,
        );
        assert!(
            out.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Focus)
        );
        if zone == DropZone::Canvas {
            assert!(!desktop.image_drop.pending());
            let canvas = desktop
                .image_drop
                .zones
                .iter()
                .find(|(_, _, z, _)| *z == DropZone::Canvas)
                .unwrap()
                .1;
            let expected =
                crate::canvas::canvas_position(pos, canvas.min + desktop.pan, desktop.zoom);
            assert!(
                matches!(&fixture.last_request, Some(UiRequest::ImportDroppedImages { target: ImageImportTarget::NewAchievements { name, position }, .. }) if name == "first" && *position == expected)
            );
        } else if zone == DropZone::General(String::new()) {
            assert!(!desktop.image_drop.pending());
            assert!(
                matches!(&fixture.last_request,Some(UiRequest::ImportDroppedImages { target:ImageImportTarget::General {achievement_id,..},.. }) if achievement_id == &id)
            );
        } else {
            assert_eq!(fixture.requests, 0, "Choice must precede import");
            let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
            let pos = button(&out, "Import");
            click(&mut desktop, &mut fixture, &ctx, pos);
            assert!(
                matches!(&fixture.last_request,Some(UiRequest::ImportDroppedImages { target:ImageImportTarget::Layers {achievement_id,..},.. }) if achievement_id == &id)
            );
        }
        assert_eq!(fixture.editor.document(), &before);
        assert!(desktop.view == desktop::View::Achievements);
    }
}
#[test]
fn icon_mode_requires_layers_and_double_clicks_switch_or_clear_selection() {
    let mut fixture = Fixture::new();
    fixture.layout.position = Some([700.0, 300.0]);
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = fixture.editor.document().clone();
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    let icon = ctx
        .read_response(egui::Id::new(("icon-layers", &id)))
        .unwrap()
        .rect
        .center();
    click(&mut desktop, &mut fixture, &ctx, icon);
    assert!(
        desktop.inspector_tab == desktop::InspectorTab::General,
        "Single icon click must only select"
    );
    click(&mut desktop, &mut fixture, &ctx, icon);
    assert!(desktop.layers_available());
    desktop.layer_palette.open = false;
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    assert!(
        desktop.layers_available(),
        "Icon mode repairs a hidden layers panel"
    );
    let card = ctx
        .read_response(egui::Id::new(("node", &id)))
        .unwrap()
        .rect;
    let body = card.right_center() - Vec2::new(30.0, 0.0);
    for _ in 0..40 {
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    }
    click(&mut desktop, &mut fixture, &ctx, body);
    click(&mut desktop, &mut fixture, &ctx, body);
    assert!(
        desktop.inspector_tab == desktop::InspectorTab::General,
        "Double-click card body must enter General"
    );
    assert_eq!(desktop.selection, vec![id.clone()]);
    let canvas = desktop
        .image_drop
        .zones
        .iter()
        .find(|(_, _, z, _)| *z == DropZone::Canvas)
        .unwrap()
        .1;
    let empty = canvas.left_bottom() + Vec2::new(40.0, -50.0);
    for _ in 0..40 {
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    }
    click(&mut desktop, &mut fixture, &ctx, empty);
    click(&mut desktop, &mut fixture, &ctx, empty);
    assert!(desktop.selection.is_empty());
    assert!(!desktop.layers_available());
    assert_eq!(fixture.editor.document(), &before);
}
#[test]
fn cancelled_drop_and_drop_on_stats_do_not_create_artwork() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap();
    let pos = ctx
        .read_response(egui::Id::new(("node", id)))
        .unwrap()
        .rect
        .right_center()
        - Vec2::new(20.0, 0.0);
    frame(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![Event::PointerMoved(pos)],
        true,
    );
    let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    click(&mut desktop, &mut fixture, &ctx, button(&out, "Cancel"));
    assert_eq!(fixture.requests, 0);
    desktop.view = desktop::View::Stats;
    desktop.kind = EntityKind::Stat;
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    let out = frame(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![Event::PointerMoved(pos)],
        true,
    );
    assert_eq!(fixture.requests, 0);
    assert!(
        !out.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Focus)
    );
}

#[test]
fn narrow_inspectors_keep_all_three_tab_labels_visible_at_larger_fonts() {
    for width in [220.0, 300.0, 640.0] {
        for font_size in [14.0, 22.0, 32.0] {
            let mut fixture = Fixture::new();
            let mut prefs = fixture.editor.preferences().clone();
            prefs.normal_size = font_size;
            fixture
                .editor
                .execute("Preferences", Command::Preferences(prefs), None)
                .unwrap();
            let before = fixture.editor.document().clone();
            let mut desktop = Desktop::default();
            desktop.selection = vec![
                before.entities(EntityKind::Achievement)[0]["id"]
                    .as_str()
                    .unwrap()
                    .into(),
            ];
            let ctx = egui::Context::default();
            configure_test_fonts(&ctx);
            for _ in 0..2 {
                let mut out = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 850.0))),
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), fixture.preferences());
                        ui.set_style(ctx.global_style());
                        desktop.inspector(ui, &mut fixture);
                    },
                );
                out.textures_delta.clear();
                for alternatives in [["General", "Gen"], ["Localization", "Loc"], ["Icon", "Ico"]] {
                    let label = out
                        .shapes
                        .iter()
                        .find_map(|s| alternatives.iter().find_map(|name| label(&s.shape, name)));
                    assert!(
                        label.is_some_and(|p| p.x >= 0.0 && p.x <= width),
                        "Missing tab {alternatives:?} at {width}/{font_size}"
                    );
                }
            }
            assert_eq!(fixture.editor.document(), &before);
        }
    }
}

#[test]
fn short_right_release_creates_near_click_and_deletes_from_both_canvases() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        for target in ["background", "card", "icon"] {
            if kind == EntityKind::Stat && target == "icon" {
                continue;
            }
            for passes in [1, 2] {
                let mut fixture = Fixture::new();
                fixture
                    .execute(
                        "Camera",
                        Command::CanvasCamera {
                            kind,
                            zoom: 0.75,
                            offset_x: -80.0,
                            offset_y: -40.0,
                        },
                        None,
                    )
                    .unwrap();
                let mut desktop = Desktop::default();
                desktop.kind = kind;
                desktop.view = if kind == EntityKind::Achievement {
                    desktop::View::Achievements
                } else {
                    desktop::View::Stats
                };
                let ctx = egui::Context::default();
                ctx.options_mut(|options| options.max_passes = passes.try_into().unwrap());
                frame(&mut desktop, &mut fixture, &ctx, vec![], false);
                frame(&mut desktop, &mut fixture, &ctx, vec![], false);
                let id = fixture.editor.document().entities(kind)[0]["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let pos = match target {
                    "card" => {
                        ctx.read_response(egui::Id::new(("node", &id)))
                            .unwrap()
                            .rect
                            .right_center()
                            - Vec2::new(20.0, 0.0)
                    }
                    "icon" => ctx
                        .read_response(egui::Id::new(("icon-layers", &id)))
                        .unwrap()
                        .rect
                        .center(),
                    _ => egui::pos2(300.0, 600.0),
                };
                let before = fixture.editor.document().clone();
                let pan = desktop.pan;
                for pressed in [true, false] {
                    frame(
                        &mut desktop,
                        &mut fixture,
                        &ctx,
                        vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: PointerButton::Secondary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        false,
                    );
                    assert_eq!(
                        desktop.canvas_menu.is_some(),
                        !pressed,
                        "{kind:?}/{target}/{passes}"
                    );
                }
                assert_eq!(desktop.pan, pan);
                let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
                let create = if kind == EntityKind::Achievement {
                    "Create achievement"
                } else {
                    "Create stat"
                };
                click(&mut desktop, &mut fixture, &ctx, button(&out, create));
                assert!(desktop.canvas_menu.is_none());
                let after = fixture.editor.document().clone();
                assert_eq!(after.entities(kind).len(), before.entities(kind).len() + 1);
                let new_id = after.entities(kind).last().unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                assert_eq!(desktop.selection, vec![new_id.clone()]);
                frame(&mut desktop, &mut fixture, &ctx, vec![], false);
                let card = ctx
                    .read_response(egui::Id::new(("node", &new_id)))
                    .unwrap()
                    .rect;
                assert!(
                    (card.min - pos).length() < 280.0,
                    "New card must stay near the click"
                );
                for old in before.entities(kind) {
                    let old = ctx
                        .read_response(egui::Id::new(("node", old["id"].as_str().unwrap())))
                        .unwrap()
                        .rect;
                    assert!(!card.intersect(old).is_positive());
                }
                fixture.undo();
                assert_eq!(fixture.editor.document(), &before);
                fixture.redo();
                assert_eq!(fixture.editor.document(), &after);
                let pos = card.right_center() - Vec2::new(20.0, 0.0);
                for pressed in [true, false] {
                    frame(
                        &mut desktop,
                        &mut fixture,
                        &ctx,
                        vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: PointerButton::Secondary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        false,
                    );
                }
                let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
                click(
                    &mut desktop,
                    &mut fixture,
                    &ctx,
                    button(&out, "Delete selection"),
                );
                assert_eq!(
                    fixture.editor.document().entities(kind).len(),
                    before.entities(kind).len()
                );
                fixture.undo();
                assert_eq!(fixture.editor.document(), &after);
            }
        }
    }
}

#[test]
fn right_click_preserves_group_and_escape_or_outside_click_dismisses_without_edits() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    let before = fixture.editor.document().clone();
    let ids: Vec<_> = before
        .entities(EntityKind::Achievement)
        .iter()
        .map(|entity| entity["id"].as_str().unwrap().to_owned())
        .collect();
    desktop.selection = ids.clone();
    let pos = ctx
        .read_response(egui::Id::new(("node", &ids[0])))
        .unwrap()
        .rect
        .right_center()
        - Vec2::new(20.0, 0.0);
    for dismiss in ["escape", "outside", "delete"] {
        desktop.selection = ids.clone();
        for pressed in [true, false] {
            frame(
                &mut desktop,
                &mut fixture,
                &ctx,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
        }
        let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
        assert_eq!(desktop.selection, ids);
        assert!(
            desktop.canvas_menu.is_some(),
            "Menu did not reopen for {dismiss}"
        );
        assert!(egui::Popup::is_any_open(&ctx));
        match dismiss {
            "escape" => {
                frame(
                    &mut desktop,
                    &mut fixture,
                    &ctx,
                    vec![Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    false,
                );
            }
            "outside" => click(&mut desktop, &mut fixture, &ctx, egui::pos2(800.0, 700.0)),
            _ => click(
                &mut desktop,
                &mut fixture,
                &ctx,
                button(&out, "Delete selection"),
            ),
        }
        assert!(desktop.canvas_menu.is_none());
        if dismiss == "delete" {
            assert!(
                fixture
                    .editor
                    .document()
                    .entities(EntityKind::Achievement)
                    .is_empty()
            );
            fixture.undo();
        }
        assert_eq!(fixture.editor.document(), &before);
        frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    }
}

#[test]
fn right_click_creates_an_art_image_with_image_controls_and_no_achievement_settings() {
    for sidebar in [true, false] {
        check_art_image_creation(sidebar);
    }
}

fn check_art_image_creation(sidebar: bool) {
    let mut fixture = if sidebar {
        Fixture::new()
    } else {
        Fixture::combined_layers()
    };
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    let before = fixture.editor.document().clone();
    let pos = egui::pos2(300.0, 600.0);
    for pressed in [true, false] {
        frame(
            &mut desktop,
            &mut fixture,
            &ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
    }
    let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    click(
        &mut desktop,
        &mut fixture,
        &ctx,
        button(&out, "Create art image"),
    );
    let after = fixture.editor.document().clone();
    let local = after.entities(EntityKind::Achievement).last().unwrap();
    assert!(achimeld_application::art_images::is_art_image(local));
    assert_eq!(desktop.selection, [local["id"].as_str().unwrap()]);
    assert!(desktop.layers_available());
    let out = frame(&mut desktop, &mut fixture, &ctx, vec![], false);
    assert_eq!(desktop.layer_palette.properties_rect.is_some(), sidebar);
    let expected = if sidebar {
        [
            "Image",
            "Preview inputs",
            "Select a layer to edit its properties.",
        ]
    } else {
        [
            "Image",
            "Edit image layers…",
            "Art image · reusable composition",
        ]
    };
    for expected in expected {
        button(&out, expected);
    }
    for absent in [
        "General",
        "Localization",
        "S. API Name",
        "Affect style",
        "U. Value",
    ] {
        assert!(
            out.shapes
                .iter()
                .all(|shape| label(&shape.shape, absent).is_none())
        );
    }
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
    fixture.redo();
    assert_eq!(fixture.editor.document(), &after);
}
