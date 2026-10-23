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
    let before = fixture.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    frame(&ctx, &mut fixture, &mut desktop, 1440.0, vec![]);
    assert!(desktop.view == desktop::View::Style);
    let banner = ctx
        .read_response(egui::Id::new(("viewer-banner", &style)))
        .expect("Rendered style banner");
    assert!(banner.rect.width() >= 280.0);
    let files: Vec<_> = ["style-set-load", "style-set-save", "style-set-save-as"]
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
    assert!(banner.rect.right() <= slot.rect.left());
    assert!(slot.rect.right() <= preview.left());
    assert!(preview.right() <= bank.rect.left());
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
    frame(&ctx, &mut fixture, &mut desktop, 560.0, vec![]);
    assert!(desktop.view == desktop::View::Style);
    assert!(
        ctx.read_response(egui::Id::new("viewer-unlock-value"))
            .is_some()
    );
}
