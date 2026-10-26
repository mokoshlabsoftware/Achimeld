use super::*;
use achimeld_application::style_viewer::{StyleViewerAction, StyleViewerSource, StyleViewerState};

fn setup() -> (Fixture, Desktop, String, String) {
    let mut fixture = Fixture::new();
    let style = achimeld_application::IconStyle {
        id: new_id(),
        name: "Cave painting".into(),
        preview_png: None,
        recipe: json!({"layers":[]}),
    };
    let image = new_id();
    let mut project = fixture.editor.document().project().clone();
    project["assets"]["images"] = json!([{"id":image,"displayName":"Any bank image","image":{"kind":"RelativeToProject","path":"image.png"}}]);
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    fixture.viewer = Some(StyleViewerState {
        styles: vec![style.clone()],
        selected_style: Some(style.id.clone()),
        ..Default::default()
    });
    let mut desktop = Desktop::default();
    desktop.open_style_viewer();
    (fixture, desktop, style.id, image)
}

fn frame(
    ctx: &egui::Context,
    fixture: &mut Fixture,
    desktop: &mut Desktop,
    width: f32,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 850.0),
            )),
            events,
            ..Default::default()
        },
        |ui| desktop.show(ui, fixture),
    );
    output.textures_delta.clear();
}

#[test]
fn viewer_is_canvas_mode_with_flush_file_actions_and_project_bank_selection() {
    let (mut fixture, mut desktop, style, image) = setup();
    desktop.style_viewer_ui.options_open = true;
    let before = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    assert!(desktop.view == desktop::View::Style);
    let banner = ctx
        .read_response(egui::Id::new(("viewer-banner", &style)))
        .expect("Rendered style banner");
    assert!(banner.rect.width() >= 260.0);
    let files: Vec<_> = ["style-json-load", "style-json-save"]
        .map(|name| {
            ctx.read_response(egui::Id::new(("viewer-file", name)))
                .unwrap()
                .rect
        })
        .into_iter()
        .collect();
    for pair in files.windows(2) {
        assert_eq!(pair[0].top(), pair[1].top());
        assert_eq!(pair[0].bottom(), pair[1].bottom());
        assert_eq!(pair[0].right(), pair[1].left());
    }
    let bank = ctx
        .read_response(egui::Id::new((
            "viewer-bank-image",
            false,
            Some(image.clone()),
        )))
        .expect("Bank image");
    let slot = ctx
        .read_response(egui::Id::new(("viewer-slot", 0_usize)))
        .expect("Preview image ID 0");
    let preview = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("viewer-main-preview-rect")))
        .expect("Main preview");
    assert!(banner.rect.right() <= preview.left());
    assert!(slot.rect.is_positive());
    assert!(preview.width() > 256.0);
    assert!(banner.rect.height() < 150.0);
    assert!(bank.rect.is_positive());
    let pos = bank.rect.center();
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1440.0,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1440.0,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        matches!(&fixture.last_request,Some(UiRequest::StyleViewer(StyleViewerAction::SetImageSlot { index:0, source:Some(StyleViewerSource::Image {id}) })) if *id == image)
    );
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn viewer_banner_context_rename_uses_viewer_intent_and_compact_layout_remains_available() {
    let (mut fixture, mut desktop, style, _) = setup();
    let before = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    frame(&ctx, &mut fixture, &mut desktop, 1100.0, vec![]);
    frame(&ctx, &mut fixture, &mut desktop, 1100.0, vec![]);
    let pos = ctx
        .read_response(egui::Id::new(("viewer-banner", &style)))
        .unwrap()
        .rect
        .center();
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    for _ in 0..4 {
        frame(&ctx, &mut fixture, &mut desktop, 1100.0, vec![]);
    }
    let rename = ctx
        .read_response(egui::Id::new("viewer-rename-menu"))
        .expect("Banner context menu")
        .rect
        .center();
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![egui::Event::PointerMoved(rename)],
    );
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![
            egui::Event::PointerMoved(rename),
            egui::Event::PointerButton {
                pos: rename,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![egui::Event::PointerButton {
            pos: rename,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        desktop.style_rename.is_some(),
        "Rename click at {rename:?}: {:?}",
        ctx.read_response(egui::Id::new("viewer-rename-menu"))
    );
    desktop.style_rename.as_mut().unwrap().name = "Handprints".into();
    frame(&ctx, &mut fixture, &mut desktop, 1100.0, vec![]);
    let approve = ctx
        .read_response(egui::Id::new(("rename-style-action", 0)))
        .unwrap()
        .rect
        .center();
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![
            egui::Event::PointerMoved(approve),
            egui::Event::PointerButton {
                pos: approve,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1100.0,
        vec![egui::Event::PointerButton {
            pos: approve,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        matches!(&fixture.last_request, Some(UiRequest::StyleViewer(StyleViewerAction::RenameStyle {id,name})) if *id == style && name == "Handprints")
    );
    assert_eq!(fixture.editor.document(), &before);
    desktop.style_viewer_ui.options_open = true;
    frame(&ctx, &mut fixture, &mut desktop, 1000.0, vec![]);
    assert!(desktop.view == desktop::View::Style);
    assert!(
        ctx.read_response(egui::Id::new("viewer-unlock-value"))
            .is_some()
    );
}

#[test]
fn style_viewer_banners_are_wide_and_touch_without_spacing() {
    let (mut fixture, mut desktop, first, _) = setup();
    let mut second = fixture.viewer.as_ref().unwrap().styles[0].clone();
    second.id = new_id();
    second.name = "Second style".into();
    let second_id = second.id.clone();
    fixture.viewer.as_mut().unwrap().styles.push(second);
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    for _ in 0..2 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    let first = ctx
        .read_response(egui::Id::new(("viewer-banner", &first)))
        .unwrap()
        .rect;
    let second = ctx
        .read_response(egui::Id::new(("viewer-banner", &second_id)))
        .unwrap()
        .rect;
    assert_eq!(first.bottom(), second.top());
    assert_eq!(first.x_range(), second.x_range());
    assert!(first.width() > first.height() * 2.0);
}

#[test]
fn viewer_layers_edit_project_style_and_undo_updates_using_icons() {
    let (mut fixture, mut desktop, _, _) = setup();
    let style = achimeld_application::IconStyle {
        id: new_id(),
        name: "Editable".into(),
        preview_png: None,
        recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","visible":true,"solidColorHex":"#FF123456"}]}),
    };
    fixture
        .execute("Save", Command::SaveIconStyle(style.clone()), None)
        .unwrap();
    fixture.viewer = Some(StyleViewerState {
        styles: vec![style.clone()],
        selected_style: Some(style.id.clone()),
        ..Default::default()
    });
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    for _ in 0..8 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    assert!(desktop.layers_available());
    assert!(desktop.selected_layer.is_none());
    let layer = style.recipe["layers"][0]["id"].as_str().unwrap();
    let pos = ctx
        .read_response(crate::layer_palette::layer_control_id(
            &style.id, layer, "eye",
        ))
        .unwrap()
        .rect
        .center();
    let before = fixture.editor.document().clone();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut fixture,
            &mut desktop,
            1440.0,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert_eq!(
        fixture.editor.document().icon_styles()[0].recipe["layers"][0]["visible"],
        false,
        "eye={:?}, error={:?}, draft={:?}, position={pos:?}",
        ctx.read_response(crate::layer_palette::layer_control_id(
            &style.id, layer, "eye"
        )),
        desktop.error,
        desktop.entity_drafts.get(&style.id).map(|d| &d.error)
    );
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
}

#[test]
fn viewer_folders_fold_and_unrelated_collections_have_no_banners() {
    let (mut fixture, mut desktop, id, _) = setup();
    let other = achimeld_application::IconStyle {
        id: new_id(),
        name: "Unrelated".into(),
        recipe: json!({"layers":[]}),
        preview_png: None,
    };
    let collection = new_id();
    fixture.viewer.as_mut().unwrap().collections.push(
        achimeld_application::style_viewer::StyleViewerCollection {
            id: collection.clone(),
            name: "Other project".into(),
            styles: vec![other.clone()],
            read_only: false,
            dirty: false,
            steam_sets: vec![],
        },
    );
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    for _ in 0..2 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    assert!(
        ctx.read_response(egui::Id::new((
            "viewer-banner",
            format!("{collection}/{}", other.id)
        )))
        .is_none()
    );
    let pos = ctx
        .read_response(egui::Id::new(("viewer-folder", false)))
        .unwrap()
        .rect
        .center();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut fixture,
            &mut desktop,
            1440.0,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    for _ in 0..2 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    assert!(
        ctx.read_response(egui::Id::new(("viewer-banner", id)))
            .is_none()
    );
}

#[test]
fn viewer_options_opens_once_and_name_filter_keeps_only_matching_banners() {
    let (mut fixture, mut desktop, id, _) = setup();
    let mut second = fixture.viewer.as_ref().unwrap().styles[0].clone();
    second.id = new_id();
    second.name = "Second style".into();
    let second_id = second.id.clone();
    fixture.viewer.as_mut().unwrap().styles.push(second);
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    for _ in 0..5 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("viewer-name-filter")));
    frame(
        &ctx,
        &mut fixture,
        &mut desktop,
        1440.0,
        vec![egui::Event::Text("SECOND".into())],
    );
    for _ in 0..2 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    assert!(
        ctx.read_response(egui::Id::new(("viewer-banner", id)))
            .is_none()
    );
    assert!(
        ctx.read_response(egui::Id::new(("viewer-banner", second_id)))
            .is_some()
    );
    let pos = ctx
        .read_response(egui::Id::new("viewer-options-button"))
        .unwrap()
        .rect
        .center();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut fixture,
            &mut desktop,
            1440.0,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    for _ in 0..3 {
        frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    }
    assert!(desktop.style_viewer_ui.options_open);
    assert!(
        ctx.read_response(egui::Id::new("viewer-clone-builtins"))
            .is_some()
    );
}
