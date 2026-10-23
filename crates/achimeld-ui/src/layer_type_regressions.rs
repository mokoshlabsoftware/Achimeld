use super::*;

#[derive(Default)]
struct PickerPaint {
    rows: Vec<egui::epaint::RectShape>,
    icons: Vec<(egui::Rect, egui::TextureId, egui::Color32)>,
    labels: Vec<egui::epaint::TextShape>,
}
impl PickerPaint {
    fn collect(&mut self, shape: &egui::Shape) {
        match shape {
            egui::Shape::Rect(rect) if rect.brush.is_some() => self.icons.push((
                rect.rect,
                rect.brush.as_ref().unwrap().fill_texture_id,
                rect.fill,
            )),
            egui::Shape::Rect(rect) if rect.rect.height() == 28.0 => self.rows.push(rect.clone()),
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                self.icons
                    .push((mesh.calc_bounds(), mesh.texture_id, mesh.vertices[0].color));
            }
            egui::Shape::Text(text) => self.labels.push(text.clone()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    self.collect(shape);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn layer_picker_rows_fill_width_and_tint_leading_icons_with_hovered_labels() {
    for scale in [1.0, 1.5, 2.0] {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let preferences = Preferences::default();
        let frame = |time, events| {
            let mut selected = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 530.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    theme::apply(ui.ctx(), &preferences);
                    ui.set_style(ui.ctx().global_style());
                    selected = layer_palette::layer_type_menu(ui);
                },
            );
            let mut paint = PickerPaint::default();
            for shape in &output.shapes {
                paint.collect(&shape.shape);
            }
            output.textures_delta.clear();
            (paint, selected)
        };
        frame(0.0, vec![]);
        let (rest, _) = frame(0.01, vec![]);
        let first_adjustment = crate::layer_icons::LAYERS.len();
        let kinds = first_adjustment + crate::layer_icons::ADJUSTMENTS.len();
        assert_eq!(rest.rows.len(), kinds);
        assert_eq!(rest.icons.len(), kinds);
        let panel = theme::color(&preferences, "Panel");
        let ink = theme::color(&preferences, "Primary text");
        let first = rest.rows[0].rect;
        let adjustment = rest.rows[first_adjustment].rect;
        assert_eq!(first.top(), adjustment.top());
        assert_eq!(first.right(), adjustment.left());
        for (index, row) in rest.rows.iter().enumerate() {
            assert_eq!(
                row.rect.x_range(),
                if index < first_adjustment {
                    first
                } else {
                    adjustment
                }
                .x_range()
            );
            assert_eq!(row.fill, panel);
            assert_eq!(row.stroke, egui::Stroke::NONE);
            assert_eq!(row.corner_radius, egui::CornerRadius::ZERO);
            let icon = &rest.icons[index];
            let label = rest
                .labels
                .iter()
                .find(|text| {
                    row.rect.contains(text.pos)
                        && text.galley.text()
                            == crate::localization::text(
                                &ctx,
                                layer_palette::label(
                                    crate::layer_icons::LAYERS
                                        .iter()
                                        .chain(crate::layer_icons::ADJUSTMENTS.iter())
                                        .nth(index)
                                        .unwrap(),
                                ),
                            )
                })
                .unwrap();
            assert!(
                icon.0.right() < label.pos.x,
                "Each symbol precedes its label"
            );
            if index != 0 && index != first_adjustment {
                assert_eq!(rest.rows[index - 1].rect.bottom(), row.rect.top());
            }
        }
        let target = rest.rows[first_adjustment].rect;
        let point = egui::pos2(target.right() - 2.0, target.center().y);
        frame(0.02, vec![egui::Event::PointerMoved(point)]);
        let (hover, _) = frame(0.43, vec![]);
        assert_eq!(hover.rows[first_adjustment].rect, target);
        assert_eq!(hover.rows[first_adjustment].fill, ink);
        assert_eq!(hover.icons[first_adjustment].2, panel);
        assert!(
            hover
                .labels
                .iter()
                .filter(|text| text.galley.text() == "Curves")
                .all(|text| text.override_text_color == Some(panel))
        );
        for pressed in [true, false] {
            let (_, selected) = frame(
                0.44,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if !pressed {
                assert_eq!(
                    selected,
                    Some("Curves"),
                    "The far edge of the row selects the layer"
                );
            }
        }
    }
}

#[test]
fn create_popup_keeps_every_adjustment_reachable_and_undoable() {
    fn frame(
        ctx: &egui::Context,
        desktop: &mut Desktop,
        fixture: &mut Fixture,
        size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> PickerPaint {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), fixture.preferences());
                ui.set_style(ui.ctx().global_style());
                desktop.layer_editor(ui, fixture);
            },
        );
        let mut paint = PickerPaint::default();
        for shape in &output.shapes {
            paint.collect(&shape.shape);
        }
        output.textures_delta.clear();
        paint
    }
    fn click(
        ctx: &egui::Context,
        desktop: &mut Desktop,
        fixture: &mut Fixture,
        size: egui::Vec2,
        pos: egui::Pos2,
    ) {
        for pressed in [true, false] {
            frame(
                ctx,
                desktop,
                fixture,
                size,
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
    }
    for size in [egui::vec2(760.0, 530.0), egui::vec2(360.0, 360.0)] {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        let mut fixture = Fixture::new();
        let achievement = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut desktop = Desktop::default();
        desktop.selection = vec![achievement.clone()];
        frame(&ctx, &mut desktop, &mut fixture, size, vec![]);
        frame(&ctx, &mut desktop, &mut fixture, size, vec![]);
        let before = fixture.editor.document().clone();
        let initial_icon = layer_palette::effective_icon(
            before
                .entity(EntityKind::Achievement, &achievement)
                .unwrap(),
        );
        let initial_layers = initial_icon["layers"].as_array().unwrap().len();
        for kind in crate::layer_icons::ADJUSTMENTS {
            let create = ctx
                .read_response(crate::layer_toolbar::Action::Create.id())
                .unwrap();
            let popup = egui::Popup::default_response_id(&create);
            click(&ctx, &mut desktop, &mut fixture, size, create.rect.center());
            let paint = frame(&ctx, &mut desktop, &mut fixture, size, vec![]);
            assert!(egui::Popup::is_id_open(&ctx, popup));
            let text = crate::localization::text(&ctx, layer_palette::label(kind));
            let label = paint
                .labels
                .iter()
                .find(|label| label.galley.text() == text)
                .unwrap_or_else(|| panic!("Missing {kind} at {size:?}"));
            let point = label.pos + label.galley.rect.center().to_vec2();
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains(point));
            click(&ctx, &mut desktop, &mut fixture, size, point);
            assert!(!egui::Popup::is_id_open(&ctx, popup));
            let entity = fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .unwrap();
            let layers = entity["icon"]["layers"].as_array().unwrap();
            assert_eq!(layers.len(), initial_layers + 1);
            let layer = layers.last().unwrap();
            assert_eq!(layer["kind"], kind);
            assert_eq!(layer["id"].as_str(), desktop.selected_layer.as_deref());
            assert_eq!(fixture.requests, 0);
            fixture.undo();
            assert_eq!(fixture.editor.document(), &before);
            frame(&ctx, &mut desktop, &mut fixture, size, vec![]);
        }
    }
}

#[test]
fn adjustment_thumbnails_ignore_artwork_and_keep_distinct_draggable_symbols() {
    let ctx = egui::Context::default();
    let artwork = egui::TextureId::User(999);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        for kind in crate::layer_icons::ADJUSTMENTS {
            let response = icons::draggable_thumbnail(ui, Some(artwork), kind, 42.0);
            assert!(response.sense.senses_click() && response.sense.senses_drag());
        }
    });
    output.textures_delta.clear();
    let mut paint = PickerPaint::default();
    for shape in &output.shapes {
        paint.collect(&shape.shape);
        assert!(
            !matches!(&shape.shape, egui::Shape::Rect(rect) if rect.brush.is_none()),
            "Symbols have no artwork checkerboard or frame"
        );
    }
    assert_eq!(paint.icons.len(), crate::layer_icons::ADJUSTMENTS.len());
    let textures: std::collections::HashSet<_> = paint.icons.iter().map(|icon| icon.1).collect();
    assert_eq!(textures.len(), crate::layer_icons::ADJUSTMENTS.len());
    assert!(!textures.contains(&artwork));
}

#[test]
#[ignore = "Native layer picker and adjustment symbol preview; external output path"]
fn native_layer_types_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        output: std::path::PathBuf,
        started: std::time::Instant,
        requested: bool,
        hover: bool,
        pointer: Option<egui::Pos2>,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
            if self.hover
                && let Some(pointer) = self.pointer
            {
                input.events.push(egui::Event::PointerMoved(pointer));
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), &Preferences::default());
            ui.set_style(ui.ctx().global_style());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    let origin = ui.cursor().min;
                    layer_palette::layer_type_menu(ui);
                    self.pointer = Some(origin + egui::vec2(400.0, 24.0 + 14.0));
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.heading("Adjustment layers");
                    for kind in crate::layer_icons::ADJUSTMENTS {
                        ui.horizontal(|ui| {
                            icons::draggable_thumbnail(ui, None, kind, 42.0);
                            ui.label(layer_palette::label(kind));
                        });
                    }
                });
            });
            if !self.requested && self.started.elapsed().as_secs_f32() > 0.5 {
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
                self.requested = true;
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
            assert!(
                self.started.elapsed().as_secs() < 20,
                "Layer picker screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    ))
    .unwrap();
    assert!(output.starts_with(std::path::absolute(crate::test_output::root()).unwrap()));
    eframe::run_native(
        "Achimeld layer types",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([740.0, 530.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            Ok(Box::new(Window {
                output,
                started: std::time::Instant::now(),
                requested: false,
                hover: std::env::var_os("ACHIMELD_LAYER_ICON_HOVER").is_some(),
                pointer: None,
            }))
        }),
    )
    .unwrap();
}
