use super::*;
use crate::style_picker::{row_id, steam_set_caption};

fn setup() -> (Fixture, Desktop, Vec<achimeld_application::IconStyle>) {
    let mut fixture = Fixture::new();
    let achievement = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut styles = Vec::new();
    for (name, color) in [
        ("Moonlit silver", "#FFB8CCD9"),
        ("Ember and gold", "#FFE09F3E"),
        ("Ocean glass", "#FF3EA6AD"),
    ] {
        let style = achimeld_application::IconStyle {
            id: new_id(),
            name: name.into(),
            preview_png: None,
            recipe: json!({"layers":[
                {"id":new_id(),"kind":"Gradient","gradient":{"kind":"Linear","angle":45.0,"stops":[{"offset":0.0,"colorHex":"#FF142039"},{"offset":1.0,"colorHex":color}]}},
                {"id":new_id(),"kind":"Shape","shape":{"kind":"Star","scaleX":0.65,"scaleY":0.65,"colorHex":"#FFFFFFFF"}}
            ]}),
        };
        fixture.save_icon_style(style.clone()).unwrap();
        let mut project = fixture.editor.document().project().clone();
        project["achievements"][0]["icon"] = style.recipe.clone();
        let document = achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap();
        let image = render(&document, &achievement, Some((256, 256)), &mut NoAssets)
            .unwrap()
            .achieved;
        fixture.previews.insert(
            format!("style:{}:{achievement}", style.id),
            Preview {
                width: 256,
                height: 256,
                source_size: [256, 256],
                rgba: image.into_raw().into(),
                fingerprint: style.id.clone(),
            },
        );
        styles.push(style);
    }
    fixture.editor.mark_saved();
    let mut desktop = Desktop::default();
    desktop.selection = vec![achievement];
    (fixture, desktop, styles)
}

fn frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    icon: &mut Value,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            crate::theme::apply(ui.ctx(), fixture.preferences());
            ui.set_width(380.0);
            let styles = fixture.icon_styles();
            desktop.project_style_rows(ui, fixture, icon, &styles);
            desktop.style_edit_prompt_window(ui.ctx(), fixture);
            desktop.style_rename_window(ui.ctx(), fixture);
        },
    );
    output.textures_delta.clear();
}

fn click(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    icon: &mut Value,
    id: egui::Id,
) {
    // Modal areas size themselves before accepting input, especially after a
    // scale change or a handoff from the naming prompt.
    for _ in 0..4 {
        frame(ctx, desktop, fixture, icon, vec![]);
    }
    let pos = ctx
        .read_response(id)
        .expect("Rendered control")
        .rect
        .center();
    for pressed in [true, false] {
        frame(
            ctx,
            desktop,
            fixture,
            icon,
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
    frame(ctx, desktop, fixture, icon, vec![]);
}

#[test]
fn save_style_prompt_requires_a_name_and_keeps_cancel_and_undo() {
    let (mut fixture, mut desktop, styles) = setup();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut icon = styles[0].recipe.clone();
    let before = fixture.editor.document().clone();
    desktop.open_style_save(&fixture, &icon);
    frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
    let save = egui::Id::new(("rename-style-action", 0));
    assert!(!ctx.read_response(save).unwrap().enabled());
    desktop.style_rename.as_mut().unwrap().name = "Saved icon".into();
    frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut fixture,
        &mut icon,
        egui::Id::new(("rename-style-action", 1)),
    );
    assert!(desktop.style_rename.is_none());
    assert_eq!(fixture.editor.document(), &before);
    desktop.open_style_save(&fixture, &icon);
    desktop.style_rename.as_mut().unwrap().name = "Saved icon".into();
    frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
    click(&ctx, &mut desktop, &mut fixture, &mut icon, save);
    assert!(desktop.style_rename.is_none());
    let saved = fixture.icon_styles().pop().unwrap();
    assert_eq!(saved.name, "Saved icon");
    assert_eq!(saved.recipe, icon);
    assert_ne!(saved.id, styles[0].id);
    assert_eq!(
        fixture.editor.document().entities(EntityKind::Achievement)[0]["icon"],
        icon
    );
    assert!(desktop.style_edit_prompt.is_some());
    assert!(desktop.layer_palette.active_style.is_none());
    assert_eq!(
        desktop
            .matching_icon_style(&fixture, &icon, &fixture.icon_styles())
            .unwrap()
            .id,
        saved.id
    );
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
    frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
    assert!(desktop.style_edit_prompt.is_none());
    fixture.redo();
    assert_eq!(
        desktop
            .matching_icon_style(&fixture, &icon, &fixture.icon_styles())
            .unwrap()
            .id,
        saved.id
    );
}

#[test]
fn created_style_choice_preserves_the_layer_and_switches_only_on_request() {
    for (choice, scale) in [(0, 1.0), (1, 1.5), (2, 2.0)] {
        let (mut fixture, mut desktop, styles) = setup();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut icon = styles[0].recipe.clone();
        desktop.selected_layer = icon["layers"][1]["id"].as_str().map(str::to_owned);
        let layer = desktop.selected_layer.clone();
        desktop.open_style_save(&fixture, &icon);
        desktop.style_rename.as_mut().unwrap().name = "New style".into();
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        click(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            egui::Id::new(("rename-style-action", 0)),
        );
        let saved = fixture.icon_styles().pop().unwrap();
        assert_eq!(saved.name, "New style", "scale {scale}");
        let document = fixture.editor.document().clone();
        let revision = fixture.editor.revision();
        assert!(desktop.style_edit_prompt.is_some());
        for _ in 0..4 {
            frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        }
        let stay = ctx
            .read_response(egui::Id::new(("created-style-edit-mode", "action", 0usize)))
            .unwrap()
            .rect;
        let edit = ctx
            .read_response(egui::Id::new(("created-style-edit-mode", "action", 1usize)))
            .unwrap()
            .rect;
        assert_eq!(stay.y_range(), edit.y_range());
        assert!((stay.right() - edit.left()).abs() < 0.1);
        let response = ctx
            .read_response(egui::Id::new(("created-style-edit-mode", "action", 0usize)))
            .unwrap();
        assert!(response.enabled());
        assert_eq!(
            ctx.layer_id_at(response.rect.center()),
            Some(response.layer_id)
        );
        if choice < 2 {
            click(
                &ctx,
                &mut desktop,
                &mut fixture,
                &mut icon,
                egui::Id::new(("created-style-edit-mode", "action", choice as usize)),
            );
        } else {
            frame(
                &ctx,
                &mut desktop,
                &mut fixture,
                &mut icon,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(
            desktop.style_edit_prompt.is_none(),
            "choice {choice}, scale {scale}"
        );
        assert_eq!(
            desktop.layer_palette.active_style,
            (choice == 1).then_some(saved.id)
        );
        assert_eq!(desktop.selected_layer, layer);
        assert_eq!(fixture.editor.document(), &document);
        assert_eq!(fixture.editor.revision(), revision);
    }
}

#[test]
fn failed_style_creation_never_assigns_or_offers_an_edit_mode() {
    let (mut fixture, mut desktop, styles) = setup();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut icon = styles[0].recipe.clone();
    icon["layers"][0]["opacity"] = json!(-1.0);
    let before = fixture.editor.document().clone();
    desktop.open_style_save(&fixture, &icon);
    desktop.style_rename.as_mut().unwrap().name = "Invalid style".into();
    frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut fixture,
        &mut icon,
        egui::Id::new(("rename-style-action", 0)),
    );
    assert_eq!(fixture.editor.document(), &before);
    assert!(desktop.style_rename.is_some());
    assert!(desktop.style_edit_prompt.is_none());
    assert!(desktop.icon_style_choices.is_empty());
}

#[test]
fn style_rows_preview_three_banner_heights_and_rename_only_after_approval() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let (mut fixture, mut desktop, styles) = setup();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let mut icon = styles[2].recipe.clone();
        let before = fixture.editor.document().clone();
        for _ in 0..2 {
            frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        }
        let apply = ctx
            .read_response(row_id(&styles[0].id, "apply"))
            .unwrap()
            .rect;
        let rename = ctx
            .read_response(row_id(&styles[0].id, "rename"))
            .unwrap()
            .rect;
        let delete = ctx
            .read_response(row_id(&styles[0].id, "delete"))
            .unwrap()
            .rect;
        assert!(apply.height() >= 3.0 * 15.0 + 15.0);
        assert_eq!(apply.y_range(), rename.y_range());
        assert_eq!(rename.y_range(), delete.y_range());
        assert!((apply.right() - rename.left()).abs() < 0.1);
        assert!((rename.right() - delete.left()).abs() < 0.1);
        click(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            row_id(&styles[0].id, "rename"),
        );
        assert!(desktop.style_rename.is_some());
        assert_eq!(icon, styles[2].recipe);
        desktop.style_rename.as_mut().unwrap().name = "Custom style — Żółć".into();
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        assert_eq!(fixture.editor.document(), &before);
        click(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            egui::Id::new(("rename-style-action", 0)),
        );
        assert!(desktop.style_rename.is_none());
        let renamed = fixture.icon_styles()[0].clone();
        assert_eq!(renamed.name, "Custom style — Żółć");
        assert_eq!(renamed.id, styles[0].id);
        assert_eq!(renamed.recipe, styles[0].recipe);
        assert_eq!(
            fixture.editor.document().entities(EntityKind::Achievement),
            before.entities(EntityKind::Achievement)
        );
        fixture.undo();
        assert_eq!(fixture.editor.document(), &before);
        desktop.open_style_rename(&fixture, &styles[0]);
        desktop.style_rename.as_mut().unwrap().name = "Discard this".into();
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        click(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            egui::Id::new(("rename-style-action", 1)),
        );
        assert!(desktop.style_rename.is_none());
        assert_eq!(fixture.editor.document(), &before);
        desktop.open_style_rename(&fixture, &styles[0]);
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        frame(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(desktop.style_rename.is_none());
        assert_eq!(fixture.editor.document(), &before);
    }
}

#[test]
fn rename_rejects_blank_long_and_deleted_styles_and_preserves_preview_and_links() {
    let (mut fixture, _, styles) = setup();
    let mut project = fixture.editor.document().project().clone();
    project["iconStyles"][0]["previewPng"] = json!("saved default PNG");
    project["styleViewerSteamSets"] = json!([
        {"styleId":styles[0].id,"appId":480,"set":{"kind":"Custom","id":"expansion","name":"Expansion"}},
        {"styleId":styles[1].id,"appId":481,"set":{"kind":"BaseGame"}}
    ]);
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    assert_eq!(
        steam_set_caption(&fixture, Some(&styles[0].id)),
        "Tied to: Expansion set (App 480)"
    );
    assert_eq!(
        steam_set_caption(&fixture, Some(&styles[1].id)),
        "Tied to: Base game set (App 481)"
    );
    assert_eq!(
        steam_set_caption(&fixture, Some(&styles[2].id)),
        "Not tied to any set"
    );
    assert_eq!(steam_set_caption(&fixture, None), "Not tied to any set");
    let before = fixture.editor.document().clone();
    assert!(fixture.rename_icon_style(&styles[0].id, "  ").is_err());
    assert!(
        fixture
            .rename_icon_style(&styles[0].id, &"a".repeat(257))
            .is_err()
    );
    assert_eq!(fixture.editor.document(), &before);
    fixture
        .rename_icon_style(&styles[0].id, " Renamed ")
        .unwrap();
    assert_eq!(fixture.icon_styles()[0].name, "Renamed");
    assert_eq!(
        fixture.icon_styles()[0].preview_png.as_deref(),
        Some("saved default PNG")
    );
    assert_eq!(
        fixture.editor.document().project()["styleViewerSteamSets"],
        before.project()["styleViewerSteamSets"]
    );
    let mut edited = fixture.icon_styles()[0].clone();
    edited.recipe["layers"][0]["visible"] = json!(false);
    fixture.save_icon_style(edited).unwrap();
    assert!(fixture.icon_styles()[0].preview_png.is_none());
    fixture.undo();
    assert_eq!(
        fixture.icon_styles()[0].preview_png.as_deref(),
        Some("saved default PNG")
    );
    fixture.delete_icon_style(&styles[0].id).unwrap();
    assert!(
        fixture
            .rename_icon_style(&styles[0].id, "Resurrect")
            .is_err()
    );
}

#[test]
#[ignore = "Native style dropdown and rename prompt; external output directory"]
fn native_style_picker_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        icon: Value,
        frames: usize,
        rename: bool,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            crate::theme::apply(ui.ctx(), self.fixture.preferences());
            ui.set_width(420.0);
            let styles = self.fixture.icon_styles();
            let banner = self
                .desktop
                .icon_style_banner(ui, &mut self.fixture, &mut self.icon);
            if !self.rename && self.frames == 1 && self.desktop.style_edit_prompt.is_none() {
                egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&banner));
            }
            if self.rename && self.frames == 1 {
                if std::env::var_os("ACHIMELD_SCREENSHOT_SAVE_STYLE").is_some() {
                    self.desktop.open_style_save(&self.fixture, &self.icon);
                    self.desktop.style_rename.as_mut().unwrap().name = "Moonlit silver copy".into();
                } else {
                    self.desktop.open_style_rename(&self.fixture, &styles[0]);
                }
            }
            self.desktop
                .style_edit_prompt_window(ui.ctx(), &mut self.fixture);
            self.desktop
                .style_rename_window(ui.ctx(), &mut self.fixture);
            self.frames += 1;
            if self.frames == 8 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            if let Some(image) = ui.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            }) {
                let rgba: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                image::save_buffer(
                    &self.output,
                    &rgba,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            assert!(self.frames < 500);
            ui.ctx().request_repaint();
        }
    }
    let (mut fixture, mut desktop, styles) = setup();
    if std::env::var_os("ACHIMELD_SCREENSHOT_CREATED_STYLE").is_some() {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut icon = styles[0].recipe.clone();
        desktop.open_style_save(&fixture, &icon);
        desktop.style_rename.as_mut().unwrap().name = "Moonlit silver copy".into();
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        frame(&ctx, &mut desktop, &mut fixture, &mut icon, vec![]);
        click(
            &ctx,
            &mut desktop,
            &mut fixture,
            &mut icon,
            egui::Id::new(("rename-style-action", 0)),
        );
        assert!(desktop.style_edit_prompt.is_some());
        desktop.textures.clear();
    }
    let output = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set external output path"),
    );
    assert!(output.starts_with(crate::test_output::root()));
    let rename = std::env::var_os("ACHIMELD_SCREENSHOT_RENAME_STYLE").is_some()
        || std::env::var_os("ACHIMELD_SCREENSHOT_SAVE_STYLE").is_some();
    let scale = std::env::var("ACHIMELD_SCREENSHOT_SCALE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 700.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "Style picker review",
        options,
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(scale);
            Ok(Box::new(Window {
                desktop,
                fixture,
                icon: styles[0].recipe.clone(),
                frames: 0,
                rename,
                output,
            }))
        }),
    )
    .unwrap();
}
