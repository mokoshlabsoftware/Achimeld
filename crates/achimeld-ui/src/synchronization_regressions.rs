use super::*;

struct ReviewPort {
    base: Fixture,
    review: Value,
    show_review: bool,
    banner_ready: bool,
    verified: bool,
    error: Option<String>,
}
impl ReviewPort {
    fn new() -> Self {
        let mut base = Fixture::new();
        base.editor
            .execute(
                "Preview project",
                Command::ProjectSettings {
                    name: "Steam synchronization preview".into(),
                    app_id: 480,
                    atlas: None,
                    assets: None,
                },
                None,
            )
            .unwrap();
        base.editor.mark_saved();
        Self {
            base,
            show_review: true,
            banner_ready: false,
            verified: false,
            error: None,
            review: json!({"token":"fixture-review","appId":480,"rows":[
                {"key":"Stat:SCORE","kind":"Stat","apiName":"SCORE","state":"LocalOnly","defaultAction":"Upload","actions":["Skip","Upload"],"fields":[{"field":"Maximum","local":"100","steam":"—"}]},
                {"key":"Achievement:STEAM_FIRST","kind":"Achievement","apiName":"STEAM_FIRST","state":"SteamOnly","defaultAction":"Skip","actions":["Skip","Import"],"fields":[]},
                {"key":"Icon:LOCAL_WIN","kind":"Icon","apiName":"LOCAL_WIN","state":"ArtworkUnavailable","defaultAction":"Skip","actions":["Skip"],"fields":[]},
                {"key":"Icon:COIN","kind":"Icon","apiName":"COIN","state":"InSync","defaultAction":"Skip","actions":["Skip","Upload"],"hasArtwork":true,"fields":[]}
            ]}),
        }
    }
}
impl EditorPort for ReviewPort {
    fn editor(&self) -> &Editor {
        self.base.editor()
    }
    fn execute(
        &mut self,
        label: &str,
        command: Command,
        group: Option<&str>,
    ) -> Result<(), String> {
        self.base.execute(label, command, group)
    }
    fn finish_group(&mut self) {
        self.base.finish_group();
    }
    fn undo(&mut self) {
        self.base.undo();
    }
    fn redo(&mut self) {
        self.base.redo();
    }
    fn request(&mut self, request: UiRequest) {
        self.base.request(request);
    }
    fn status(&self) -> &str {
        "Review the synchronization plan"
    }
    fn preferences(&self) -> &Preferences {
        self.base.preferences()
    }
    fn preview(&mut self, id: &str) -> Option<Preview> {
        self.base.preview(id)
    }
    fn synchronization_review(&self) -> Option<&Value> {
        self.show_review.then_some(&self.review)
    }
    fn synchronization_verified(&self) -> bool {
        self.verified
    }
    fn synchronization_error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn synchronizing(&self) -> bool {
        self.show_review
    }
    fn asset_preview(&mut self, _atlas: bool, id: Option<&str>) -> Option<Preview> {
        (self.banner_ready && id == Some("fixture-banner")).then(|| Preview {
            width: 462,
            height: 174,
            source_size: [462, 174],
            rgba: [40, 70, 110, 255].repeat(462 * 174).into(),
            fingerprint: "fixture-banner".into(),
        })
    }
    fn synchronization_icon_preview(&self, _api: &str, locked: bool) -> Option<Preview> {
        let mut pixels = Vec::new();
        for y in 0_i32..64 {
            for x in 0_i32..64 {
                let inside = (x - 32).pow(2) + (y - 32).pow(2) < 25 * 25;
                let rgb = if inside {
                    if locked {
                        [175, 175, 175]
                    } else {
                        [240, 190, 55]
                    }
                } else {
                    [32, 42, 58]
                };
                pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        Some(Preview {
            width: 64,
            height: 64,
            source_size: [512, 512],
            rgba: pixels.into(),
            fingerprint: format!("fixture-icon-{locked}"),
        })
    }
}

#[test]
fn saved_destination_validation_survives_reopen_but_is_bound_to_app_id() {
    for (app_id, validated) in [(480, true), (481, false)] {
        let mut port = ReviewPort::new();
        port.show_review = false;
        let mut project = port.base.editor.document().project().clone();
        project["lastRemoteSnapshot"] = json!({"steamAppId":app_id});
        project["lastVerifiedAtUtc"] = json!("2026-10-20T15:37:29Z");
        port.base.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                port.base.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let desktop = Desktop::default();
        assert_eq!(desktop.sync_destination_validated(&port), validated);
        assert_eq!(desktop.sync_wants_click(&port), !validated);
        assert_eq!(desktop.sync_validate_wants_click(&port), !validated);
        assert_eq!(
            desktop.sync_action_label(&port),
            if validated { "Synchronize" } else { "Validate" }
        );
        assert!(desktop.sync_action_enabled(&port));
    }
}

#[test]
fn completed_validation_keeps_synchronize_label_without_attention_animation() {
    let mut port = ReviewPort::new();
    port.show_review = false;
    port.verified = true;
    let desktop = Desktop::default();
    assert_eq!(desktop.sync_action_label(&port), "Synchronize");
    assert!(!desktop.sync_validate_wants_click(&port));
    assert!(
        !desktop.sync_action_enabled(&port),
        "There is no pending reviewed work to replay"
    );
    port.verified = false;
    assert_eq!(desktop.sync_action_label(&port), "Validate");
    assert!(desktop.sync_validate_wants_click(&port));
}
fn texts(shape: &egui::Shape, output: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => output.push((
            text.galley.text().into(),
            egui::Rect::from_min_size(text.pos, text.galley.size()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                texts(shape, output);
            }
        }
        _ => {}
    }
}
fn frame(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    port: &mut dyn EditorPort,
    width: f32,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 1100.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            desktop.synchronization_view(ui, port);
            desktop.sync_app_id_window(ui.ctx(), port);
        },
    );
    let mut labels = Vec::new();
    for shape in &output.shapes {
        texts(&shape.shape, &mut labels);
    }
    output.textures_delta.clear();
    labels
}
fn click(
    ctx: &egui::Context,
    desktop: &mut Desktop,
    port: &mut dyn EditorPort,
    position: egui::Pos2,
) {
    for pressed in [true, false] {
        frame(
            ctx,
            desktop,
            port,
            1000.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn review_is_side_effect_free_and_only_selected_prepared_rows_are_submitted() {
    let mut port = ReviewPort::new();
    let original = port.base.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert_eq!(desktop.sync_choices["Stat:SCORE"], "Upload");
    assert_eq!(desktop.sync_choices["Achievement:STEAM_FIRST"], "Skip");
    assert_eq!(port.base.editor.document(), &original);
    assert!(desktop.requests.is_empty());
    assert!(
        !labels
            .iter()
            .any(|(label, _)| label == "Import available Steam entries")
    );
    let import = labels
        .iter()
        .find(|(label, _)| label == "Import")
        .unwrap()
        .1
        .center();
    click(&ctx, &mut desktop, &mut port, import);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert_eq!(desktop.sync_choices["Achievement:STEAM_FIRST"], "Import");
    let apply = labels
        .iter()
        .find(|(label, _)| label == "Synchronize 2 selected changes")
        .unwrap()
        .1
        .center();
    click(&ctx, &mut desktop, &mut port, apply);
    assert!(
        desktop.requests.is_empty(),
        "Destination must be confirmed before applying"
    );
    let confirm = labels
        .iter()
        .find(|(label, _)| label == "I confirm this is the correct Steam App ID.")
        .unwrap()
        .1
        .center();
    click(&ctx, &mut desktop, &mut port, confirm);
    click(&ctx, &mut desktop, &mut port, apply);
    let request = desktop.requests.pop().unwrap();
    let UiRequest::ApplySynchronization(choices) = request else {
        panic!("Expected typed synchronization selection");
    };
    assert_eq!(choices.len(), 4);
    assert!(choices.contains(&("Stat:SCORE".into(), "Upload".into())));
    assert!(choices.contains(&("Achievement:STEAM_FIRST".into(), "Import".into())));
    assert_eq!(port.base.editor.document(), &original);
}

#[test]
fn saved_snapshot_counts_use_the_project_steam_app_id_and_reject_another_destination() {
    for (app_id, current) in [(480, true), (481, false)] {
        let mut port = ReviewPort::new();
        port.show_review = false;
        let mut project = port.base.editor.document().project().clone();
        project["lastRemoteSnapshot"] = json!({"steamAppId":app_id,"stats":[null,null,null,null,null,null,null,null],"achievements":[null]});
        port.base.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                port.base.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
        let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
        assert_eq!(
            labels.iter().any(|(label, _)| label == "Not loaded"),
            !current
        );
        assert_eq!(
            labels
                .iter()
                .any(|(label, _)| label == "Saved Steam snapshot"),
            current
        );
    }
}

#[test]
fn failed_retry_keeps_saved_snapshot_time_and_distinguishes_it_from_live_validation() {
    for (verified_at, read_at, expected) in [
        (
            Some("2026-10-23T11:45:00.1234567+00:00"),
            Some("2026-10-22T10:00:00Z"),
            "Last successful check: 2026-10-23 11:45:00 UTC",
        ),
        (
            None,
            Some("2026-10-22T10:00:00Z"),
            "Snapshot read at: 2026-10-22 10:00:00 UTC",
        ),
        (
            None,
            None,
            "The check time was not recorded in this project.",
        ),
    ] {
        let mut port = ReviewPort::new();
        port.show_review = false;
        let mut project = port.base.editor.document().project().clone();
        project["lastRemoteSnapshot"] =
            json!({"steamAppId":480,"stats":[null],"achievements":[null],"readAtUtc":read_at});
        project["lastVerifiedAtUtc"] = json!(verified_at);
        port.base.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                port.base.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        let original = port.base.editor.document().clone();
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut desktop = Desktop::default();
        for failed in [false, true] {
            port.error = failed
                .then(|| "Steam operation stopped while opening the Steamworks destination".into());
            frame(&ctx, &mut desktop, &mut port, 480.0, vec![]);
            let labels = frame(&ctx, &mut desktop, &mut port, 480.0, vec![]);
            assert!(labels.iter().any(|(text, _)| text == expected));
            assert!(
                labels
                    .iter()
                    .any(|(text, _)| text == "Saved Steam snapshot")
            );
            assert_eq!(labels.iter().any(|(text, _)| text == "Latest validation or synchronization failed; showing previously saved Steam data."), failed);
            assert!(
                !labels
                    .iter()
                    .any(|(text, _)| text == "Validated and synchronized · no pending changes.")
            );
            assert_eq!(port.base.editor.document(), &original);
            for (text, rect) in labels {
                assert!(rect.right() <= 480.0, "Snapshot status clipped: {text}");
            }
        }
    }
}

#[test]
fn a_new_review_resets_choices_and_narrow_rows_wrap() {
    let mut port = ReviewPort::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 420.0, vec![]);
    desktop
        .sync_choices
        .insert("Stat:SCORE".into(), "Skip".into());
    desktop.sync_confirmation = desktop.sync_review_identity(&port);
    port.review["token"] = json!("fresh-review");
    let labels = frame(&ctx, &mut desktop, &mut port, 420.0, vec![]);
    assert_eq!(desktop.sync_choices["Stat:SCORE"], "Upload");
    assert!(!desktop.sync_destination_confirmed(&port));
    for (label, rect) in labels {
        assert!(
            rect.max.x <= 420.0,
            "Clipped synchronization label {label}: {rect:?}"
        );
    }
}

fn set_app_id(port: &mut Fixture, app_id: u32) {
    port.execute(
        "App ID",
        Command::ProjectSettings {
            name: "Sync test".into(),
            app_id,
            atlas: None,
            assets: None,
        },
        None,
    )
    .unwrap();
}

#[test]
fn synchronization_counts_only_real_achievements_even_after_local_icons_are_renamed() {
    let mut port = Fixture::new();
    port.editor = Editor::default();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    for command in [
        Command::AddStyleAchievementAt { position: [0, 0] },
        Command::AddArtImageAt { position: [20, 0] },
    ] {
        port.execute("Local icon", command, None).unwrap();
        let local = port
            .editor
            .document()
            .entities(EntityKind::Achievement)
            .last()
            .unwrap();
        let id = local["displayNameLocalizationId"].as_str().unwrap().into();
        port.execute(
            "Rename",
            Command::SetLocalization {
                id,
                language: "English".into(),
                text: "Renamed canvas icon".into(),
            },
            None,
        )
        .unwrap();
    }
    for expected in [0, 1] {
        if expected == 1 {
            port.execute("Achievement", Command::Add(EntityKind::Achievement), None)
                .unwrap();
        }
        frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
        let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
        let row = labels
            .iter()
            .find(|(text, _)| text == "Achievements")
            .unwrap()
            .1;
        let count = labels
            .iter()
            .find(|(_, rect)| {
                rect.left() > row.right() && (rect.center().y - row.center().y).abs() < 1.0
            })
            .unwrap();
        assert_eq!(count.0, expected.to_string());
    }
}

fn label_center(labels: &[(String, egui::Rect)], target: &str) -> egui::Pos2 {
    labels
        .iter()
        .find(|(label, _)| label == target)
        .unwrap_or_else(|| panic!("Missing {target}: {labels:?}"))
        .1
        .center()
}

#[test]
fn entering_an_app_id_moves_the_animated_cue_to_validate_and_undo_restores_it() {
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut port = Fixture::new();
    let mut desktop = Desktop::default();
    let draw = |desktop: &mut Desktop, port: &mut Fixture| {
        frame(&ctx, desktop, port, 1000.0, vec![]);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            desktop.synchronization_view(ui, port)
        });
        let stripes = ctx
            .data(|data| {
                data.get_temp::<egui::TextureHandle>(egui::Id::new("button-attention-stripes"))
            })
            .unwrap()
            .id();
        let highlighted: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect
                        .brush
                        .as_ref()
                        .is_some_and(|brush| brush.fill_texture_id == stripes) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect();
        let mut labels = Vec::new();
        for shape in &output.shapes {
            texts(&shape.shape, &mut labels);
        }
        output.textures_delta.clear();
        let edit = ctx
            .read_response(egui::Id::new("sync-edit-app-id"))
            .unwrap()
            .rect;
        let action = ctx
            .read_response(egui::Id::new("sync-primary-action"))
            .unwrap()
            .rect;
        (
            highlighted.contains(&edit),
            highlighted.contains(&action),
            labels,
        )
    };
    let (edit, validate, _) = draw(&mut desktop, &mut port);
    assert!(
        edit && !validate,
        "Only the App ID editor is guided before an ID is present"
    );
    set_app_id(&mut port, 480);
    let (edit, validate, labels) = draw(&mut desktop, &mut port);
    assert!(
        !edit && validate,
        "The rendered stripes must move to Validate after entering an ID"
    );
    assert!(labels.iter().any(|(text, _)| text == "Validate"));
    assert!(
        !labels
            .iter()
            .any(|(text, _)| text == "Validate…" || text == "Validate...")
    );
    port.undo();
    let (edit, validate, _) = draw(&mut desktop, &mut port);
    assert!(edit && !validate);
}

#[test]
fn missing_id_opens_the_app_id_editor_and_a_valid_id_only_prepares_a_review() {
    let mut port = Fixture::new();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut port,
        label_center(&labels, "App ID…"),
    );
    assert!(desktop.sync_app_id_edit.is_some());
    assert!(desktop.requests.is_empty());
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut port,
        label_center(&labels, "Cancel"),
    );
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert!(!labels.iter().any(|(label, _)| label == "Validate again"));
    assert!(desktop.requests.is_empty());
    assert!(desktop.sync_action_enabled(&port));
    assert!(desktop.sync_wants_click(&port));
    set_app_id(&mut port, 480);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert_eq!(desktop.sync_action_label(&port), "Validate");
    click(
        &ctx,
        &mut desktop,
        &mut port,
        label_center(&labels, "Validate"),
    );
    assert!(matches!(
        desktop.requests.as_slice(),
        [UiRequest::PrepareSynchronization]
    ));
    assert!(!desktop.terminal.open, "Validate must keep Terminal closed");
}

#[test]
fn toolbar_app_id_stage_opens_sync_and_editor_then_validate_prepares_the_review() {
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut port = Fixture::new();
    let mut desktop = Desktop::default();
    let draw = |desktop: &mut Desktop, port: &mut Fixture, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1900.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, port),
        );
        let mut labels = Vec::new();
        output.textures_delta.clear();
        for shape in output.shapes {
            texts(&shape.shape, &mut labels);
        }
        labels
    };
    draw(&mut desktop, &mut port, vec![]);
    let labels = draw(&mut desktop, &mut port, vec![]);
    let target = label_center(&labels, "App ID…");
    for pressed in [true, false] {
        draw(
            &mut desktop,
            &mut port,
            vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert_eq!(port.requests, 0);
    assert!(desktop.view == desktop::View::Sync);
    assert!(desktop.sync_app_id_edit.is_some());
    desktop.sync_app_id_edit = None;
    set_app_id(&mut port, 480);
    let labels = draw(&mut desktop, &mut port, vec![]);
    let target = label_center(&labels, "Validate");
    for pressed in [true, false] {
        draw(
            &mut desktop,
            &mut port,
            vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert!(desktop.view == desktop::View::Sync);
    assert!(matches!(
        port.last_request,
        Some(UiRequest::PrepareSynchronization)
    ));
    assert!(
        !desktop.terminal.open,
        "Toolbar validation must keep Terminal closed"
    );
}

#[test]
fn confirmation_is_bound_to_the_app_id_review_and_revision() {
    let mut port = ReviewPort::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    desktop.sync_confirmation = desktop.sync_review_identity(&port);
    assert!(desktop.sync_action_enabled(&port));
    assert!(!desktop.sync_wants_click(&port));
    assert!(!desktop.sync_app_id_wants_click(&port));
    assert!(!desktop.sync_validate_wants_click(&port));
    port.review["appId"] = json!(481);
    assert!(!desktop.sync_action_enabled(&port));
    desktop.run_sync_action(&port);
    assert!(desktop.requests.is_empty());
    port.review["appId"] = json!(480);
    port.base
        .execute("Edit", Command::Add(EntityKind::Stat), None)
        .unwrap();
    assert!(!desktop.sync_action_enabled(&port));
    desktop.sync_confirmation = desktop.sync_review_identity(&port);
    port.review["token"] = json!("new-review");
    assert!(!desktop.sync_action_enabled(&port));
    port.review["appId"] = json!(0);
    set_app_id(&mut port.base, 0);
    assert!(desktop.sync_review_identity(&port).is_none());
}

#[test]
fn validated_review_without_a_banner_explains_the_missing_artwork() {
    let mut port = ReviewPort::new();
    port.review["gameBannerAvailable"] = json!(false);
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 420.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 420.0, vec![]);
    assert!(
        labels
            .iter()
            .any(|(label, _)| label.contains("game banner could not be retrieved"))
    );
    assert!(!desktop.sync_wants_click(&port));
    assert!(
        !desktop.sync_action_enabled(&port),
        "A missing banner never confirms a destination"
    );
}

#[test]
fn failed_validation_shows_the_error_and_only_the_primary_validate_action() {
    let mut port = ReviewPort::new();
    port.show_review = false;
    port.error = Some("Steam navigation timed out while opening the destination".into());
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert!(
        labels
            .iter()
            .any(|(label, _)| label.contains("Steam navigation timed out"))
    );
    assert!(!labels.iter().any(|(label, _)| label == "Validate again"));
    assert_eq!(desktop.sync_action_label(&port), "Validate");
    assert!(desktop.sync_action_enabled(&port));
    port.error = None;
    port.verified = true;
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    assert!(!labels.iter().any(
        |(label, _)| label.contains("Steam navigation timed out") || label == "Validate again"
    ));
    assert_eq!(desktop.sync_action_label(&port), "Synchronize");
    assert!(!desktop.sync_validate_wants_click(&port));
}

#[test]
fn sync_banner_uses_only_the_matching_app_and_retains_its_texture_during_refinement() {
    let mut port = ReviewPort::new();
    let mut assets = port.base.editor.document().project()["assets"].clone();
    assets["images"].as_array_mut().unwrap().push(json!({"id":"fixture-banner","kind":"User","steamBannerAppId":480,"displayName":"Game banner","image":{"kind":"RelativeToProject","path":"fixture.png"}}));
    port.base
        .execute(
            "Banner",
            Command::ProjectSettings {
                name: "Banner fixture".into(),
                app_id: 480,
                atlas: None,
                assets: Some(assets),
            },
            None,
        )
        .unwrap();
    port.banner_ready = true;
    port.review["gameBannerAvailable"] = json!(true);
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    frame(&ctx, &mut desktop, &mut port, 420.0, vec![]);
    assert!(desktop.textures.contains_key("sync-banner:fixture-banner"));
    let texture = desktop.textures["sync-banner:fixture-banner"].1.id();
    let has_texture = |shape: &egui::Shape| match shape {
        egui::Shape::Mesh(mesh) => mesh.texture_id == texture,
        egui::Shape::Rect(rect) => rect
            .brush
            .as_ref()
            .is_some_and(|brush| brush.fill_texture_id == texture),
        _ => false,
    };
    port.banner_ready = false;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.synchronization_view(ui, &mut port)
    });
    assert!(output.shapes.iter().any(|shape| has_texture(&shape.shape)));
    output.textures_delta.clear();
    set_app_id(&mut port.base, 481);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.synchronization_view(ui, &mut port)
    });
    assert!(
        !output.shapes.iter().any(|shape| has_texture(&shape.shape)),
        "Another App ID must not display the old game's banner"
    );
    output.textures_delta.clear();
}

#[test]
fn app_id_editor_validates_cancels_saves_and_rejects_stale_edits() {
    use crate::synchronization_target::parse_app_id;
    for invalid in ["", "0", "-1", "+480", "4.80", "abc", "4294967296", "４８０"] {
        assert!(parse_app_id(invalid).is_none(), "Accepted {invalid}");
    }
    assert_eq!(parse_app_id(" 480 "), Some(480));
    assert_eq!(parse_app_id("4294967295"), Some(u32::MAX));
    let mut port = Fixture::new();
    let original = port.editor.document().clone();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    desktop.open_sync_app_id(&port);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![])
        .into_iter()
        .chain(labels)
        .collect::<Vec<_>>();
    assert!(labels.iter().any(|(label, _)| label == "Steam App ID"));
    assert!(!labels.iter().any(|(label, _)| label == "S. A. ID"));
    let field = ctx
        .read_response(egui::Id::new("sync-app-id-field"))
        .unwrap();
    let label = labels
        .iter()
        .find(|(label, _)| label == "Steam App ID")
        .unwrap()
        .1;
    assert!(label.right() < field.rect.left());
    click(&ctx, &mut desktop, &mut port, label_center(&labels, "Save"));
    assert_eq!(port.editor.document(), &original);
    assert!(desktop.sync_app_id_edit.is_some());
    desktop.sync_app_id_edit.as_mut().unwrap().text = "480".into();
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    click(
        &ctx,
        &mut desktop,
        &mut port,
        label_center(&labels, "Cancel"),
    );
    assert!(desktop.sync_app_id_edit.is_none());
    assert_eq!(port.editor.document(), &original);
    desktop.open_sync_app_id(&port);
    desktop.sync_app_id_edit.as_mut().unwrap().text = "480".into();
    frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    click(&ctx, &mut desktop, &mut port, label_center(&labels, "Save"));
    assert!(desktop.sync_app_id_edit.is_none());
    assert_eq!(port.editor.document().app_id(), 480);
    port.undo();
    assert_eq!(port.editor.document(), &original);
    desktop.open_sync_app_id(&port);
    desktop.sync_app_id_edit.as_mut().unwrap().text = "481".into();
    port.execute("Concurrent edit", Command::Add(EntityKind::Stat), None)
        .unwrap();
    let labels = frame(&ctx, &mut desktop, &mut port, 1000.0, vec![]);
    click(&ctx, &mut desktop, &mut port, label_center(&labels, "Save"));
    assert_eq!(port.editor.document().app_id(), 0);
    assert!(desktop.sync_app_id_edit.is_some());
}

#[test]
#[ignore = "Native synchronization preview; writes a screenshot under the external fixture directory"]
fn native_synchronization_review_preview() {
    native_preview(false, false, false, false);
}

#[test]
#[ignore = "Native Steam App ID editor preview; writes a screenshot under the external fixture directory"]
fn native_synchronization_app_id_preview() {
    native_preview(true, false, false, false);
}

#[test]
#[ignore = "Native synchronization guidance preview; writes a screenshot under the external fixture directory"]
fn native_synchronization_guidance_preview() {
    native_preview(false, true, false, false);
}

#[test]
#[ignore = "Native validation guidance preview; writes a screenshot under the external fixture directory"]
fn native_synchronization_validate_preview() {
    native_preview(false, true, true, false);
}

#[test]
#[ignore = "Native saved snapshot preview; writes a screenshot under the external fixture directory"]
fn native_saved_snapshot_failure_preview() {
    native_preview(false, false, false, true);
}

fn native_preview(edit_app_id: bool, guidance: bool, has_app_id: bool, saved_failure: bool) {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        port: ReviewPort,
        desktop: Desktop,
        frame: usize,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.frame += 1;
            self.desktop.show(ui, &mut self.port);
            if self.frame == 6 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            for event in ui.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    image::save_buffer(
                        &self.output,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            assert!(
                self.frame < 300,
                "Synchronization screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    let output = crate::test_output::root().join(if saved_failure {
        "synchronization-saved-snapshot-failure.png"
    } else if has_app_id {
        "synchronization-validate.png"
    } else if guidance {
        "synchronization-guidance.png"
    } else if edit_app_id {
        "synchronization-app-id.png"
    } else {
        "synchronization-review.png"
    });
    eframe::run_native(
        "Achimeld synchronization preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 850.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            let mut desktop = Desktop::default();
            desktop.view = desktop::View::Sync;
            let mut port = ReviewPort::new();
            if saved_failure {
                port.show_review = false;
                port.error = Some("Steam operation stopped while opening the Steamworks destination: Browser request policy blocked an unsupported request".into());
                let mut project = port.base.editor.document().project().clone();
                project["lastRemoteSnapshot"] = json!({"steamAppId":480,"stats":[null,null,null,null,null,null,null,null],"achievements":[null]});
                project["lastVerifiedAtUtc"] = json!("2026-10-23T11:45:00Z");
                port.base.editor = Editor::new(achimeld_application::Document::from_parts(project, port.base.editor.document().catalog().clone()).unwrap());
            }
            if edit_app_id || guidance {
                port.show_review = false;
                set_app_id(&mut port.base, if has_app_id { 4_670_140 } else { 0 });
            }
            if edit_app_id {
                desktop.open_sync_app_id(&port);
            }
            Ok(Box::new(Window {
                port,
                desktop,
                frame: 0,
                output,
            }))
        }),
    )
    .unwrap();
}
