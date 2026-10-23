use super::*;

fn text_rects(output: &egui::FullOutput, label: &str, within: egui::Rect) -> Vec<egui::Rect> {
    output
        .shapes
        .iter()
        .filter_map(|paint| match &paint.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                within.contains_rect(rect).then_some(rect)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn effect_mini_controls_fit_without_hiding_the_type_icon_and_preference_hides_values() {
    for scale in [1.0, 1.25, 2.0] {
        for width in [440.0, 700.0, 1100.0] {
            let mut rows = LayerRows::new(width, 2);
            rows.ctx.set_pixels_per_point(scale);
            rows.desktop
                .layer_palette
                .expanded_effects
                .insert(rows.ids[2].clone());
            rows.frame(vec![]);
            let output = rows.frame(vec![]);
            let parent = rows.row(2);
            let id = rows.entity()["icon"]["layers"][2]["effects"][0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let selector = rows
                .ctx
                .read_response(crate::layer_effects::effect_control_id(
                    &rows.achievement,
                    &rows.ids[2],
                    &id,
                    "row",
                ))
                .unwrap()
                .rect;
            let delete = rows
                .ctx
                .read_response(crate::layer_effects::effect_control_id(
                    &rows.achievement,
                    &rows.ids[2],
                    &id,
                    "delete",
                ))
                .unwrap()
                .rect;
            assert!(
                parent.contains_rect(selector),
                "selector overflow {width}/{scale}: {parent:?} {selector:?}"
            );
            assert!(
                parent.contains_rect(delete),
                "delete overflow {width}/{scale}: {parent:?} {delete:?}"
            );
            let texture = rows
                .ctx
                .data(|data| {
                    data.get_temp::<egui::TextureHandle>(egui::Id::new((
                        "effect-type-icon",
                        "outline",
                    )))
                })
                .unwrap();
            fn has_icon(
                shape: &egui::Shape,
                texture: egui::TextureId,
                selector: egui::Rect,
            ) -> bool {
                match shape {
                    egui::Shape::Rect(rect) => {
                        rect.brush
                            .as_ref()
                            .is_some_and(|brush| brush.fill_texture_id == texture)
                            && selector.contains_rect(rect.rect)
                    }
                    egui::Shape::Mesh(mesh) => {
                        mesh.texture_id == texture && selector.contains_rect(mesh.calc_bounds())
                    }
                    egui::Shape::Vec(shapes) => shapes
                        .iter()
                        .any(|shape| has_icon(shape, texture, selector)),
                    _ => false,
                }
            }
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|paint| has_icon(&paint.shape, texture.id(), selector)),
                "Missing type icon at {width}/{scale}: {selector:?}"
            );
            let before = rows.fixture.editor.document().clone();
            if width == 1100.0 {
                assert_eq!(text_rects(&output, "4.0", parent).len(), 1);
            }
            let mut prefs = rows.fixture.preferences().clone();
            prefs.show_effect_mini_values = false;
            rows.fixture.editor = std::mem::take(&mut rows.fixture.editor).with_preferences(prefs);
            rows.frame(vec![]);
            let output = rows.frame(vec![]);
            assert!(text_rects(&output, "4.0", rows.row(2)).is_empty());
            assert_eq!(rows.fixture.editor.document(), &before);
        }
    }
}

#[test]
fn inline_width_drag_edits_the_effect_without_reordering_and_undo_restores_it() {
    for passes in [1, 2] {
        let mut rows = LayerRows::new(1100.0, passes);
        rows.desktop
            .layer_palette
            .expanded_effects
            .insert(rows.ids[2].clone());
        rows.frame(vec![]);
        let output = rows.frame(vec![]);
        let pos = text_rects(&output, "4.0", rows.row(2))[0].center();
        let before = rows.fixture.editor.document().clone();
        rows.press(pos);
        let to = pos + egui::vec2(24.0, 0.0);
        rows.frame(vec![egui::Event::PointerMoved(to)]);
        rows.frame(vec![button(to, false)]);
        rows.frame(vec![]);
        assert!(
            rows.entity()["icon"]["layers"][2]["effects"][0]["width"]
                .as_f64()
                .unwrap()
                > 4.0
        );
        assert!(rows.desktop.layer_drag.is_none());
        assert!(rows.desktop.layer_palette.effect_drag.is_none());
        rows.fixture.undo();
        assert_eq!(rows.fixture.editor.document(), &before);
    }
}

#[test]
fn every_effect_row_keeps_complete_controls_inside_the_stack_at_large_fonts() {
    for font in [13.0, 24.0] {
        for width in [440.0, 700.0, 1100.0] {
            let mut rows = LayerRows::new(width, 1);
            let mut preferences = rows.fixture.preferences().clone();
            preferences.normal_size = font;
            rows.fixture.editor =
                std::mem::take(&mut rows.fixture.editor).with_preferences(preferences);
            let effects: Vec<_> = [
                "Outline",
                "InnerLight",
                "OuterLight",
                "Shadow",
                "ColorOverlay",
                "GradientOverlay",
                "BoxBlur",
                "Skew",
                "Perspective",
                "Pinch",
                "LensBlur",
            ]
            .into_iter()
            .map(|kind| json!({"id":new_id(),"kind":kind}))
            .collect();
            let mut entity = rows.entity();
            entity["icon"]["layers"][2]["effects"] = json!(effects);
            rows.fixture
                .execute(
                    "Recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: rows.achievement.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            rows.desktop
                .layer_palette
                .expanded_effects
                .insert(rows.ids[2].clone());
            rows.frame(vec![]);
            rows.frame(vec![]);
            let parent = rows.row(2);
            assert!(
                parent.right() <= width * 0.5,
                "Stack grew at {width}/{font}: {parent:?}"
            );
            for effect in effects {
                let id = effect["id"].as_str().unwrap();
                let rect = |control| {
                    rows.ctx
                        .read_response(crate::layer_effects::effect_control_id(
                            &rows.achievement,
                            &rows.ids[2],
                            id,
                            control,
                        ))
                        .unwrap()
                        .rect
                };
                assert!(
                    parent.contains_rect(rect("row")) && parent.contains_rect(rect("delete")),
                    "Effect {} overflow at {width}/{font}",
                    effect["kind"]
                );
            }
        }
    }
}

#[test]
fn add_effect_is_centered_between_fx_and_first_junction() {
    let mut rows = LayerRows::new(1100.0, 1);
    rows.desktop
        .layer_palette
        .expanded_effects
        .insert(rows.ids[2].clone());
    rows.frame(vec![]);
    rows.frame(vec![]);
    let rect = |control| {
        rows.ctx
            .read_response(layer_palette::layer_control_id(
                &rows.achievement,
                &rows.ids[2],
                control,
            ))
            .unwrap()
            .rect
    };
    let fx = rect("fx");
    let add = rect("add");
    let effect = rows.entity()["icon"]["layers"][2]["effects"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let first = rows
        .ctx
        .read_response(crate::layer_effects::effect_control_id(
            &rows.achievement,
            &rows.ids[2],
            &effect,
            "row",
        ))
        .unwrap()
        .rect;
    assert!((add.center().x - fx.center().x).abs() < 0.5);
    assert!((add.center().y - (fx.bottom() + first.center().y) * 0.5).abs() < 1.0);
}

#[test]
#[ignore = "Native FX row preview; dedicated external screenshot output"]
fn native_effect_rows_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        rows: LayerRows,
        frames: usize,
        output: std::path::PathBuf,
    }
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.rows.fixture.preferences());
            if self.frames == 2 {
                self.rows.desktop.selected_layer = Some(self.rows.ids[2].clone());
                self.rows.desktop.layer_palette.selected_effect =
                    self.rows.entity()["icon"]["layers"][2]["effects"][0]["id"]
                        .as_str()
                        .map(str::to_owned);
            }
            self.rows.desktop.layer_editor(ui, &mut self.rows.fixture);
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
            assert!(self.frames < 128, "Native FX screenshot was not received");
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    ))
    .unwrap();
    assert!(output.starts_with(std::path::absolute(crate::test_output::root()).unwrap()));
    let mut rows = LayerRows::new(1100.0, 1);
    let mut entity = rows.entity();
    let effects: Vec<_> = ["Outline", "InnerLight", "OuterLight", "Shadow", "ColorOverlay", "GradientOverlay", "BoxBlur", "Skew", "Perspective", "Pinch", "LensBlur"]
        .into_iter().map(|kind| json!({"id":new_id(),"kind":kind,"visible":true,"width":6.0,"blur":12.0,"colorHex":"#FFB484F1","opacity":1.0})).collect();
    let selected = effects[0]["id"].as_str().unwrap().to_owned();
    entity["icon"]["layers"][2]["effects"] = json!(effects);
    rows.fixture
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: rows.achievement.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    rows.desktop.selected_layer = Some(rows.ids[2].clone());
    rows.desktop.layer_palette.selected_effect = Some(selected);
    rows.desktop
        .layer_palette
        .expanded_effects
        .insert(rows.ids[2].clone());
    eframe::run_native(
        "Achimeld FX rows",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 760.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            // Texture handles belong to the native context, not the headless setup context.
            let mut desktop = Desktop::default();
            desktop.selection = rows.desktop.selection.clone();
            desktop.selected_layer = rows.desktop.selected_layer.clone();
            desktop.layer_palette.selected_effect =
                rows.desktop.layer_palette.selected_effect.clone();
            desktop.layer_palette.expanded_effects =
                rows.desktop.layer_palette.expanded_effects.clone();
            rows.desktop = desktop;
            Ok(Box::new(Window {
                rows,
                frames: 0,
                output,
            }))
        }),
    )
    .unwrap();
}
