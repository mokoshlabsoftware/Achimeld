use super::*;

impl LayerRows {
    fn set_layers(&mut self, layers: Value) {
        let mut entity = self.entity();
        entity["icon"] = json!({"layers": layers});
        self.fixture
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: self.achievement.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        self.fixture.finish_group();
        self.frame(vec![]);
        self.frame(vec![]);
    }

    pub(super) fn entity(&self) -> Value {
        self.fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &self.achievement)
            .unwrap()
            .clone()
    }

    fn right_click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
        self.frame(vec![egui::Event::PointerMoved(pos), secondary(pos, true)]);
        self.frame(vec![secondary(pos, false)]);
        self.frame(vec![])
    }

    fn submenu(&mut self, output: &egui::FullOutput, label: &str) -> egui::FullOutput {
        let pos = menu_position(output, label).unwrap_or_else(|| panic!("Missing {label}"));
        self.press(pos);
        self.frame(vec![button(pos, false)]);
        self.frame(vec![])
    }

    fn activate(&mut self, output: &egui::FullOutput, label: &str) {
        let pos = menu_position(output, label).unwrap_or_else(|| panic!("Missing {label}"));
        self.press(pos);
        self.frame(vec![button(pos, false)]);
        self.frame(vec![]);
        assert!(!egui::Popup::is_any_open(&self.ctx));
        assert!(self.desktop.entity_drafts.is_empty());
    }
}

fn secondary(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn all_layer_types_create_masks_from_context_menus_with_one_undo() {
    for passes in [1, 2] {
        for kind in crate::layer_icons::LAYERS
            .into_iter()
            .chain(crate::layer_icons::ADJUSTMENTS)
            .chain(["Folder"])
        {
            let mut rows = LayerRows::new(1100.0, passes);
            rows.repeat_layout = true;
            let owner = rows.ids[0].clone();
            let mut layer = layer_palette::new_layer(kind);
            layer["id"] = json!(owner);
            rows.set_layers(json!([layer]));
            rows.desktop.selected_layer = Some(owner.clone());
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let pos = rows.row(0).left_top() + egui::vec2(2.0, 2.0);
            let menu = rows.right_click(pos);
            assert!(menu_position(&menu, "Add mask").is_some(), "{kind}");
            let menu = rows.submenu(&menu, "Add mask");
            rows.activate(&menu, "Solid");
            let entity = rows.entity();
            let layers = entity["icon"]["layers"].as_array().unwrap();
            assert_eq!(layers.len(), 2, "{kind}");
            let mask = &layers[1];
            assert_eq!(mask["kind"], "Solid");
            assert_eq!(layers[0]["maskLayerId"], mask["id"]);
            for field in ["opacity", "blendMode", "effects", "maskLayerId"] {
                assert!(mask[field].is_null(), "{kind}: {field}");
            }
            assert_eq!(rows.fixture.editor.revision(), revision + 1);
            assert_eq!(rows.fixture.editor.undo_label(), Some("Add layer mask"));
            assert!(
                rows.ctx
                    .read_response(layer_palette::layer_control_id(
                        &rows.achievement,
                        &owner,
                        "mask-entry"
                    ))
                    .is_some()
            );
            let after = rows.fixture.editor.document().clone();
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
            rows.fixture.redo();
            assert_eq!(rows.fixture.editor.document(), &after);
            let mut icon = rows.entity()["icon"].clone();
            let original = icon.clone();
            rows.desktop.layer_action(
                layer_palette::LayerAction::AddMask {
                    owner: mask["id"].as_str().unwrap().into(),
                    kind: "Solid",
                },
                &mut icon,
            );
            assert_eq!(icon, original, "Mask accepted its own mask");
        }
    }
}

#[test]
fn mask_context_picker_creates_any_source_kind_and_undo_restores_previous_source() {
    for passes in [1, 2] {
        for kind in [
            "Solid",
            "Gradient",
            "PerlinNoise",
            "Shape",
            "Text",
            "Artifact3d",
            "Folder",
            "Contrast",
            "MaskReference",
        ] {
            let mut rows = LayerRows::new(1100.0, passes);
            let owner = rows.ids[0].clone();
            let previous = rows.ids[1].clone();
            rows.set_layers(json!([
                {"id":previous,"kind":"Solid","visible":false,"solidColorHex":"#FFFFFFFF"},
                {"id":owner,"kind":"Folder","displayName":"Owner","maskLayerId":previous}
            ]));
            rows.desktop.selected_layer = Some(owner.clone());
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let pos = rows.row(0).left_top() + egui::vec2(2.0, 2.0);
            let output = rows.right_click(pos);
            let mut menu = rows.submenu(&output, "Replace mask");
            assert!(menu_position(&menu, "Text").is_some());
            assert!(menu_position(&menu, "Folder").is_some());
            let label = crate::localization::text(&rows.ctx, crate::layer_palette::label(kind));
            // The artwork column can need scrolling near the bottom of
            // properties. Adjustment rows have their own adjacent column.
            for _ in 0..4 {
                if menu_position(&menu, &label).is_some() {
                    break;
                }
                let pos = menu_position(&menu, "Perlin noise")
                    .or_else(|| menu_position(&menu, "Gradient"))
                    .or_else(|| menu_position(&menu, "Text"))
                    .expect("Picker has a visible scroll target");
                rows.frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -240.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                menu = rows.frame(vec![]);
            }
            rows.activate(&menu, &label);
            let entity = rows.entity();
            let layers = entity["icon"]["layers"].as_array().unwrap();
            assert_eq!(
                layers.len(),
                3,
                "Mask picker failed for {kind}, {passes} passes"
            );
            let source = layers
                .iter()
                .find(|l| l["kind"] == kind && l["id"] != owner && l["id"] != previous)
                .unwrap();
            let owner = layers.iter().find(|l| l["id"] == owner).unwrap();
            assert_eq!(owner["maskLayerId"], source["id"]);
            assert_eq!(owner["maskMode"], "Luminance");
            assert_eq!(
                rows.desktop.selected_layer.as_deref(),
                source["id"].as_str()
            );
            assert_eq!(rows.fixture.editor.revision(), revision + 1);
            assert_eq!(rows.fixture.editor.undo_label(), Some("Add layer mask"));
            let after = rows.fixture.editor.document().clone();
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
            rows.fixture.redo();
            assert_eq!(rows.fixture.editor.document(), &after);
        }
    }
}

#[test]
fn mask_picker_excludes_sources_that_depend_on_the_owner() {
    let owner = new_id();
    let folder = new_id();
    let linked = new_id();
    let noise = new_id();
    let layers = vec![
        json!({"id":folder,"kind":"Folder"}),
        json!({"id":owner,"kind":"Solid","parentFolderId":folder}),
        json!({"id":linked,"kind":"Shape","maskLayerId":folder}),
        json!({"id":noise,"kind":"PerlinNoise"}),
    ];
    let blocked = layer_palette::blocked_mask_sources(&layers, &owner);
    assert!(blocked.contains(&owner));
    assert!(blocked.contains(&folder));
    assert!(blocked.contains(&linked));
    assert!(!blocked.contains(&noise));
    let targets = layer_palette::blocked_mask_targets(&layers, &linked);
    assert!(targets.contains(&linked));
    assert!(targets.contains(&folder));
    assert!(targets.contains(&owner));
    assert!(!targets.contains(&noise));
}

#[test]
fn right_click_mask_references_layers_and_folders_with_one_undo() {
    for passes in [1, 2] {
        for kind in ["Solid", "Folder"] {
            for from_target in [false, true] {
                let mut rows = LayerRows::new(1100.0, passes);
                rows.repeat_layout = true;
                let source = rows.ids[0].clone();
                let target = rows.ids[1].clone();
                let mask = rows.ids[2].clone();
                rows.set_layers(json!([
                    {"id":mask,"kind":"Solid","displayName":"Shared mask","solidColorHex":"#FF808080","visible":false},
                    {"id":source,"kind":"Shape","displayName":"Source artwork","maskLayerId":mask,"maskMode":"Luminance"},
                    {"id":target,"kind":kind,"displayName":"Target artwork"}
                ]));
                let before = rows.fixture.editor.document().clone();
                let revision = rows.fixture.editor.revision();
                let pos = if from_target {
                    rows.row(1).left_top() + egui::vec2(2.0, 2.0)
                } else {
                    rows.ctx
                        .read_response(layer_palette::layer_control_id(
                            &rows.achievement,
                            &source,
                            "mask-entry",
                        ))
                        .unwrap()
                        .rect
                        .center()
                };
                let menu = rows.right_click(pos);
                let menu = rows.submenu(
                    &menu,
                    if from_target {
                        "Use mask from"
                    } else {
                        "Reference mask to"
                    },
                );
                rows.activate(
                    &menu,
                    if from_target {
                        "Source artwork"
                    } else {
                        "Target artwork"
                    },
                );
                let entity = rows.entity();
                let layers = entity["icon"]["layers"].as_array().unwrap();
                assert_eq!(layers.len(), 4);
                let reference = layers
                    .iter()
                    .find(|l| l["kind"] == "MaskReference")
                    .unwrap();
                assert_eq!(reference["maskSourceLayerId"], source);
                assert_eq!(
                    layers.iter().find(|l| l["id"] == target).unwrap()["maskLayerId"],
                    reference["id"]
                );
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                assert_eq!(
                    rows.fixture.editor.undo_label(),
                    Some("Reference layer mask")
                );
                let after = rows.fixture.editor.document().clone();
                rows.fixture.undo();
                assert_eq!(rows.fixture.editor.document(), &before);
                rows.fixture.redo();
                assert_eq!(rows.fixture.editor.document(), &after);
            }
        }
    }
}

#[test]
fn mask_inversion_is_a_single_undoable_context_action() {
    let mut rows = LayerRows::new(1100.0, 2);
    rows.repeat_layout = true;
    let owner = rows.ids[0].clone();
    rows.set_layers(json!([
        {"id":rows.ids[2],"kind":"Solid","displayName":"Coverage","solidColorHex":"#FF808080"},
        {"id":owner,"kind":"Solid","maskLayerId":rows.ids[2],"maskMode":"Luminance"}
    ]));
    let before = rows.fixture.editor.document().clone();
    let pos = rows
        .ctx
        .read_response(layer_palette::layer_control_id(
            &rows.achievement,
            &owner,
            "mask-entry",
        ))
        .unwrap()
        .rect
        .center();
    let menu = rows.right_click(pos);
    rows.activate(&menu, "Invert mask");
    assert_eq!(rows.entity()["icon"]["layers"][1]["maskInvert"], true);
    assert_eq!(rows.fixture.editor.undo_label(), Some("Invert layer mask"));
    let after = rows.fixture.editor.document().clone();
    rows.fixture.undo();
    assert_eq!(rows.fixture.editor.document(), &before);
    rows.fixture.redo();
    assert_eq!(rows.fixture.editor.document(), &after);
}

#[test]
fn mask_reference_actions_reject_cycles_and_mask_targets_without_editing() {
    let mut rows = LayerRows::new(1100.0, 1);
    let owner = rows.ids[0].clone();
    let child = rows.ids[1].clone();
    let mask = rows.ids[2].clone();
    rows.set_layers(json!([
        {"id":mask,"kind":"Solid","displayName":"Coverage"},
        {"id":child,"kind":"Solid","displayName":"Cyclic child","parentFolderId":owner},
        {"id":owner,"kind":"Folder","displayName":"Folder","maskLayerId":mask}
    ]));
    let original = rows.entity()["icon"].clone();
    for target in [&child, &owner, &mask] {
        let mut icon = original.clone();
        rows.desktop.layer_action(
            layer_palette::LayerAction::ReferenceMask {
                owner: target.clone(),
                source: owner.clone(),
            },
            &mut icon,
        );
        assert_eq!(icon, original);
    }
    let pos = rows
        .ctx
        .read_response(layer_palette::layer_control_id(
            &rows.achievement,
            &owner,
            "mask-entry",
        ))
        .unwrap()
        .rect
        .center();
    let menu = rows.right_click(pos);
    assert!(menu_position(&menu, "Reference mask to").is_none());
}

#[test]
fn properties_omit_folder_and_mask_management_and_reduce_mask_sources() {
    let mut rows = LayerRows::new(1100.0, 1);
    let owner = rows.ids[0].clone();
    let mask = rows.ids[2].clone();
    for kind in ["Solid", "Folder", "Contrast", "MaskReference"] {
        let mut layers = vec![
            json!({"id":owner,"kind":"Folder","displayName":"Owner","maskLayerId":mask}),
            json!({"id":mask,"kind":kind,"displayName":"Coverage","opacity":0.5,"blendMode":"Multiply", "effects":[{"id":new_id(),"kind":"Shadow"}]}),
        ];
        for selected in [&owner, &mask] {
            rows.desktop.selected_layer = Some(selected.clone());
            let before = layers.clone();
            let mut output = rows.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 1800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), rows.fixture.preferences());
                    rows.desktop.layer_properties(
                        ui,
                        &mut rows.fixture,
                        &mut layers,
                        &mut json!({}),
                    );
                },
            );
            output.textures_delta.clear();
            for forbidden in [
                "Folder",
                "Use mask",
                "U. Mask",
                "Mask source",
                "M. Source",
                "Invert mask",
            ] {
                assert!(
                    menu_position(&output, forbidden).is_none(),
                    "{kind}: {forbidden}"
                );
            }
            if selected == &mask {
                for forbidden in ["Opacity", "Effects", "Add effect", "Multiply"] {
                    assert!(
                        menu_position(&output, forbidden).is_none(),
                        "{kind}: {forbidden}"
                    );
                }
            }
            assert_eq!(
                layers, before,
                "Inspecting properties must not rewrite legacy masks"
            );
        }
    }
}

#[test]
fn folder_masks_expose_their_contents_and_nested_entries_stay_inside_narrow_panes() {
    for width in [440.0, 1100.0] {
        let mut rows = LayerRows::new(width, 2);
        let owner = rows.ids[0].clone();
        let child = rows.ids[1].clone();
        let mask = rows.ids[2].clone();
        rows.set_layers(json!([
            {"id":mask,"kind":"Folder","displayName":"A long folder mask name that should stay within the layer pane"},
            {"id":child,"kind":"Shape","parentFolderId":mask},
            {"id":owner,"kind":"Solid","maskLayerId":mask}
        ]));
        let stack = layer_palette::stack(
            rows.entity()["icon"]["layers"].as_array().unwrap(),
            &Default::default(),
        );
        assert_eq!(stack, vec![(owner.clone(), 0), (child.clone(), 2)]);
        let rect = rows
            .ctx
            .read_response(layer_palette::layer_control_id(
                &rows.achievement,
                &owner,
                "mask-entry",
            ))
            .unwrap()
            .rect;
        assert!(
            rect.right() <= rows.row(0).right() + 1.0,
            "{width}: {rect:?}"
        );
        rows.press(rect.center());
        rows.frame(vec![button(rect.center(), false)]);
        assert_eq!(rows.desktop.selected_layer.as_deref(), Some(mask.as_str()));
        rows.desktop.layer_palette.pending = Some(layer_palette::LayerAction::Add("Solid"));
        rows.frame(vec![]);
        rows.frame(vec![]);
        let entity = rows.entity();
        let layers = entity["icon"]["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[3]["parentFolderId"], mask);
        assert!(
            layer_palette::stack(layers, &Default::default())
                .iter()
                .any(|(id, depth)| layers[3]["id"] == *id && *depth == 2)
        );
    }
}

#[test]
fn selected_artwork_or_mask_preview_has_one_outline_and_selection_does_not_edit() {
    for width in [800.0, 1100.0] {
        for scale in [1.0, 1.5] {
            let mut rows = LayerRows::new(width, 2);
            rows.ctx.set_pixels_per_point(scale);
            let owner = rows.ids[0].clone();
            let mask = rows.ids[2].clone();
            let effect = new_id();
            rows.set_layers(json!([
                {"id":mask,"kind":"Solid","displayName":"Coverage","solidColorHex":"#FF808080"},
                {"id":owner,"kind":"Shape","displayName":"Artwork","maskLayerId":mask,
                    "effects":[{"id":effect,"kind":"Shadow"}]}
            ]));
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let outline_color = theme::color(rows.fixture.preferences(), "Primary text");
            let outlines = |output: &egui::FullOutput| -> Vec<egui::Rect> {
                output
                    .shapes
                    .iter()
                    .filter_map(|paint| match &paint.shape {
                        egui::Shape::Rect(shape)
                            if shape.stroke == egui::Stroke::new(2.0, outline_color)
                                && shape.fill == egui::Color32::TRANSPARENT =>
                        {
                            Some(shape.rect)
                        }
                        _ => None,
                    })
                    .collect()
            };
            rows.desktop.selected_layer = Some(owner.clone());
            let artwork = outlines(&rows.frame(vec![]));
            assert_eq!(artwork.len(), 1, "Artwork at {width}/{scale}");
            assert!(rows.row(0).contains_rect(artwork[0]));
            let button_rect = rows
                .ctx
                .read_response(layer_palette::layer_control_id(
                    &rows.achievement,
                    &owner,
                    "mask-entry-preview",
                ))
                .unwrap()
                .rect;
            rows.press(button_rect.center());
            let output = rows.frame(vec![button(button_rect.center(), false)]);
            assert_eq!(rows.desktop.selected_layer.as_deref(), Some(mask.as_str()));
            let selected = outlines(&output);
            assert_eq!(selected.len(), 1, "Only mask on the switching frame");
            assert!(button_rect.contains_rect(selected[0]));
            assert!(!selected[0].intersects(artwork[0]));
            rows.press(artwork[0].center());
            let output = rows.frame(vec![button(artwork[0].center(), false)]);
            assert_eq!(rows.desktop.selected_layer.as_deref(), Some(owner.as_str()));
            assert_eq!(
                outlines(&output),
                artwork,
                "Only artwork on the switching frame"
            );
            rows.desktop.layer_palette.selected_effect = Some(effect);
            assert!(
                outlines(&rows.frame(vec![])).is_empty(),
                "An effect selection is not an artwork selection"
            );
            assert_eq!(rows.fixture.editor.document(), &before);
            assert_eq!(rows.fixture.editor.revision(), revision);
        }
    }
}

#[test]
fn reference_mask_bundles_remap_dependencies_and_deletion_clears_links() {
    let mask = new_id();
    let source = new_id();
    let reference = new_id();
    let owner = new_id();
    let mut layers = vec![
        json!({"id":mask,"kind":"Solid","solidColorHex":"#FFFFFFFF"}),
        json!({"id":source,"kind":"Folder","maskLayerId":mask}),
        json!({"id":reference,"kind":"MaskReference","maskSourceLayerId":source}),
        json!({"id":owner,"kind":"Folder","maskLayerId":reference}),
    ];
    assert!(layer_palette::blocked_mask_sources(&layers, &reference).contains(&owner));
    let bundle = layer_palette::copy_bundle(&layers, &owner);
    // Duplicating a masked folder keeps its live source; it must not duplicate
    // the source's visible artwork as a new root layer.
    assert_eq!(bundle.len(), 2);
    let (mut fresh, root) = layer_palette::fresh_bundle(&bundle, &owner, None);
    assert_eq!(fresh[1]["id"], root);
    assert_eq!(fresh[1]["maskLayerId"], fresh[0]["id"]);
    assert_eq!(fresh[0]["maskSourceLayerId"], source);
    layer_palette::retain_mask_references(&mut fresh, &layers);
    assert_eq!(fresh[0]["maskSourceLayerId"], source);
    layer_palette::retain_mask_references(&mut fresh, &[]);
    assert!(fresh[0]["maskSourceLayerId"].is_null());
    let (whole, _) = layer_palette::fresh_bundle(&layers, &owner, None);
    assert_eq!(whole[2]["maskSourceLayerId"], whole[1]["id"]);
    assert_eq!(whole[1]["maskLayerId"], whole[0]["id"]);
    layer_palette::remove_layer(&mut layers, &source);
    assert_eq!(layers.len(), 2);
    assert!(layers.iter().find(|l| l["id"] == reference).unwrap()["maskSourceLayerId"].is_null());
}

#[test]
#[ignore = "Native mask properties screenshot; writes to the external build output"]
fn native_masks_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        rows: LayerRows,
        frames: usize,
        output: std::path::PathBuf,
        reference: bool,
        color: egui::ecolor::Hsva,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.rows.fixture.preferences());
            self.rows.desktop.layer_editor(ui, &mut self.rows.fixture);
            if !self.reference {
                ui.scope_builder(
                    egui::UiBuilder::new().ui_stack_info(
                        egui::UiStackInfo::default().with_tag(crate::color_picker::GRAYSCALE),
                    ),
                    |ui| {
                        crate::color_picker::window(
                            ui,
                            egui::Id::new("mask-native-color"),
                            "Mask color",
                            egui::Rect::from_min_size(
                                egui::pos2(48.0, 210.0),
                                egui::vec2(40.0, 30.0),
                            ),
                            &mut true,
                            &mut self.color,
                        );
                    },
                );
            }
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
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            assert!(self.frames < 128, "Native mask screenshot was not received");
            ui.ctx().request_repaint();
        }
    }
    let reference_mode = std::env::var_os("ACHIMELD_MASK_REFERENCE").is_some();
    let output = crate::test_output::root().join(if reference_mode {
        "masks-reference.png"
    } else {
        "masks-grayscale.png"
    });
    let mut rows = LayerRows::new(1100.0, 1);
    let mask = rows.ids[0].clone();
    let source = rows.ids[1].clone();
    let reference = rows.ids[2].clone();
    rows.set_layers(json!([
        {"id":mask,"kind":"Solid","displayName":"Solid mask","solidColorHex":"#FF808080","visible":false},
        {"id":source,"kind":"Folder","displayName":"Artwork","maskLayerId":mask,"maskMode":"Luminance"},
        {"id":reference,"kind":"MaskReference","displayName":"Shared artwork mask","maskSourceLayerId":source,"visible":false},
        {"id":new_id(),"kind":"Folder","displayName":"Highlights","maskLayerId":reference,"maskMode":"Luminance"}
    ]));
    rows.desktop.selected_layer = Some(if reference_mode { reference } else { mask });
    eframe::run_native(
        "Achimeld masks",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 760.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            let mut desktop = Desktop::default();
            desktop.selection = rows.desktop.selection.clone();
            desktop.selected_layer = rows.desktop.selected_layer.clone();
            rows.desktop = desktop;
            Ok(Box::new(Window {
                rows,
                frames: 0,
                output,
                reference: reference_mode,
                color: egui::ecolor::Hsva::from_srgba_unmultiplied([128, 128, 128, 255]),
            }))
        }),
    )
    .unwrap();
}

fn menu_position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    // Popup labels are painted after the matching type labels underneath them.
    output
        .shapes
        .iter()
        .rev()
        .find_map(|paint| match &paint.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let center = text.pos + text.galley.rect.center().to_vec2();
                paint.clip_rect.contains(center).then_some(center)
            }
            _ => None,
        })
}

#[test]
fn mask_preview_menu_detaches_or_deletes_shared_sources_with_one_undo() {
    for passes in [1, 2] {
        for control in ["mask-entry", "mask-entry-preview"] {
            for shared in [false, true] {
                for delete in [false, true] {
                    let mut rows = LayerRows::new(1100.0, passes);
                    let owner = rows.ids[0].clone();
                    let mask = rows.ids[2].clone();
                    let other = new_id();
                    let mut layers = vec![
                        json!({"id":mask,"kind":"Solid","displayName":"Mask source","visible":false,
                            "solidColorHex":"#FFFFFFFF"}),
                        json!({"id":rows.ids[1],"kind":"Solid","displayName":"Child","parentFolderId":owner}),
                        json!({"id":owner,"kind":"Folder","displayName":"Folder","maskLayerId":mask}),
                    ];
                    if shared {
                        layers.push(json!({"id":other,"kind":"Folder","displayName":"Other folder","maskLayerId":mask}));
                    }
                    rows.set_layers(json!(layers));
                    rows.desktop.selected_layer = Some(if control == "mask-entry" {
                        owner.clone()
                    } else {
                        rows.ids[1].clone()
                    });
                    rows.frame(vec![]);
                    rows.frame(vec![]);
                    let pos = rows
                        .ctx
                        .read_response(layer_palette::layer_control_id(
                            &rows.achievement,
                            &owner,
                            control,
                        ))
                        .unwrap()
                        .rect
                        .center();
                    let before = rows.fixture.editor.document().clone();
                    let revision = rows.fixture.editor.revision();
                    let output = rows.right_click(pos);
                    assert!(menu_position(&output, "Remove mask").is_some());
                    assert!(menu_position(&output, "Delete mask").is_some());
                    assert!(menu_position(&output, "Delete layer").is_none());
                    assert_eq!(rows.fixture.editor.document(), &before);
                    let label = if delete { "Delete mask" } else { "Remove mask" };
                    rows.activate(&output, label);
                    let mut expected = before
                        .entity(EntityKind::Achievement, &rows.achievement)
                        .unwrap()
                        .clone();
                    let layers = expected["icon"]["layers"].as_array_mut().unwrap();
                    if delete {
                        layers.retain(|layer| layer["id"] != mask);
                    }
                    for layer in layers {
                        if layer["id"] == owner || (delete && layer["maskLayerId"] == mask) {
                            layer["maskLayerId"] = Value::Null;
                        }
                    }
                    assert_eq!(
                        rows.entity(),
                        expected,
                        "{control}, shared={shared}, delete={delete}, passes={passes}"
                    );
                    assert_eq!(rows.fixture.editor.revision(), revision + 1);
                    assert_eq!(rows.fixture.editor.undo_label(), Some(label));
                    assert_eq!(rows.desktop.selected_layer.as_ref(), Some(&owner));
                    let after = rows.fixture.editor.document().clone();
                    rows.fixture.undo();
                    assert_eq!(rows.fixture.editor.document(), &before);
                    rows.fixture.redo();
                    assert_eq!(rows.fixture.editor.document(), &after);
                }
            }
        }
    }
}

#[test]
fn remove_from_folder_menu_moves_one_level_and_preserves_children_with_one_undo() {
    for width in [440.0, 1100.0] {
        for passes in [1, 2] {
            for selected in [1, 2] {
                let mut rows = LayerRows::new(width, passes);
                rows.set_layers(json!([
                    {"id":rows.ids[2],"kind":"Solid","displayName":"Child","parentFolderId":rows.ids[1]},
                    {"id":rows.ids[1],"kind":"Folder","displayName":"Inner folder","parentFolderId":rows.ids[0]},
                    {"id":rows.ids[0],"kind":"Folder","displayName":"Outer folder"}
                ]));
                let before = rows.fixture.editor.document().clone();
                let revision = rows.fixture.editor.revision();
                let pos = rows.row(selected).left_top() + egui::vec2(2.0, 2.0);
                let output = rows.right_click(pos);
                rows.activate(&output, "Remove from folder");
                let mut expected = before
                    .entity(EntityKind::Achievement, &rows.achievement)
                    .unwrap()
                    .clone();
                let layer = expected["icon"]["layers"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|l| l["id"] == rows.ids[selected])
                    .unwrap();
                layer["parentFolderId"] = if selected == 1 {
                    Value::Null
                } else {
                    json!(rows.ids[0])
                };
                assert_eq!(rows.entity(), expected);
                assert_eq!(rows.fixture.editor.revision(), revision + 1);
                assert_eq!(
                    rows.fixture.editor.undo_label(),
                    Some("Remove layer from folder")
                );
                rows.fixture.undo();
                assert_eq!(rows.fixture.editor.document(), &before);
            }
        }
    }
}

#[test]
fn dragging_out_of_a_folder_changes_parent_only_on_drop_and_undo_restores_it() {
    for passes in [1, 2] {
        for target in ["above", "below", "parent-edge", "other-root"] {
            let mut rows = LayerRows::new(1100.0, passes);
            let nested = target != "other-root";
            rows.set_layers(json!([
                {"id":rows.ids[2],"kind":"Solid","displayName":"Child","parentFolderId":rows.ids[1]},
                {"id":rows.ids[1],"kind":"Folder","displayName":"Inner folder",
                    "parentFolderId":if nested {json!(rows.ids[0])} else {Value::Null}},
                {"id":rows.ids[0],"kind":if nested {"Folder"} else {"Solid"},"displayName":"Root"}
            ]));
            let before = rows.fixture.editor.document().clone();
            let revision = rows.fixture.editor.revision();
            let pos = match target {
                "above" => rows.row(0).center_top() - egui::vec2(0.0, 5.0),
                "below" => rows.row(2).center_bottom() + egui::vec2(0.0, 6.0),
                "parent-edge" => rows.row(1).center_top() + egui::vec2(0.0, 1.0),
                _ => rows.row(0).center_bottom() - egui::vec2(0.0, 2.0),
            };
            let start = rows.row(2).left_top() + egui::vec2(2.0, 2.0);
            rows.press(start);
            rows.frame(vec![egui::Event::PointerMoved(pos)]);
            assert!(rows.desktop.layer_drag.is_some());
            assert_eq!(rows.fixture.editor.document(), &before);
            rows.frame(vec![button(pos, false)]);
            rows.frame(vec![]);
            assert!(rows.desktop.entity_drafts.is_empty());
            let entity = rows.entity();
            let layer = entity["icon"]["layers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["id"] == rows.ids[2])
                .unwrap();
            assert_eq!(
                layer["parentFolderId"],
                if target == "parent-edge" {
                    json!(rows.ids[0])
                } else {
                    Value::Null
                },
                "{target}"
            );
            assert_eq!(rows.fixture.editor.revision(), revision + 1);
            assert_eq!(
                rows.fixture.editor.undo_label(),
                Some("Reorder icon layers")
            );
            rows.fixture.undo();
            assert_eq!(rows.fixture.editor.document(), &before);
        }
    }
}
