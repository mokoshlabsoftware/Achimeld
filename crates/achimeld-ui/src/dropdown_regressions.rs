use super::*;
use crate::buttons::UiButtonExt as _;

#[test]
fn dropdown_audit_keeps_framework_controls_out_of_application_sources() {
    let mut directories = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    let forbidden = ["ComboBox", "::"].concat();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !source.contains(&forbidden),
                    "Use the shared dropdown contract: {}",
                    path.display()
                );
            }
        }
    }
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

struct List {
    ctx: egui::Context,
    selected: usize,
    rows: Vec<egui::Response>,
    trigger: Option<egui::Response>,
    time: f64,
    count: usize,
    enabled: bool,
}
impl List {
    fn new(count: usize) -> Self {
        let ctx = egui::Context::default();
        configure_test_fonts(&ctx);
        Self {
            ctx,
            selected: 0,
            rows: Vec::new(),
            trigger: None,
            time: 0.0,
            count,
            enabled: true,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.25;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480.0, 700.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                theme::apply(ui.ctx(), &Preferences::default());
                ui.set_style(ui.ctx().global_style());
                if !self.enabled {
                    ui.disable();
                }
                self.rows.clear();
                self.trigger = Some(
                    crate::dropdowns::Dropdown::from_id_salt("shape-test")
                        .width(220.0)
                        .selected_text(format!("Item {}", self.selected))
                        .show_ui(ui, |ui| {
                            for index in 0..self.count {
                                let response = ui
                                    .add_enabled_ui(index != 2, |ui| {
                                        ui.app_selectable_value(
                                            &mut self.selected,
                                            index,
                                            format!("Item {index}"),
                                        )
                                    })
                                    .inner;
                                self.rows.push(response);
                            }
                        })
                        .response,
                );
            },
        );
        output.textures_delta.clear();
        output
    }
    fn open(&mut self) {
        self.frame(vec![]);
        egui::Popup::open_id(
            &self.ctx,
            egui::Popup::default_response_id(self.trigger.as_ref().unwrap()),
        );
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn active(&self) -> Option<usize> {
        self.rows
            .first()
            .and_then(|row| crate::dropdown_navigation::keyboard_row(&self.ctx, row.layer_id))
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
    }
}

#[test]
fn dropdown_keys_skip_disabled_rows_and_stationary_mouse_cannot_reclaim_selection() {
    let mut list = List::new(6);
    list.open();
    let pointer = list.rows[1].rect.center();
    list.frame(vec![egui::Event::PointerMoved(pointer)]);
    list.frame(vec![key(egui::Key::ArrowDown)]);
    assert_eq!(list.active(), Some(3));
    for _ in 0..4 {
        list.frame(vec![egui::Event::PointerMoved(pointer)]);
    }
    assert_eq!(list.active(), Some(3));
    assert_eq!(list.selected, 0);
    let moved = list.rows[4].rect.center();
    list.frame(vec![egui::Event::PointerMoved(moved)]);
    assert_eq!(list.active(), None);
    list.frame(vec![key(egui::Key::ArrowUp)]);
    assert_eq!(list.active(), Some(3));
    list.frame(vec![key(egui::Key::Enter)]);
    assert_eq!(list.selected, 3);
    assert!(!egui::Popup::is_any_open(&list.ctx));
}

#[test]
fn dropdown_keyboard_scrolls_to_last_entry_and_escape_preserves_value() {
    let mut list = List::new(40);
    list.open();
    for _ in 0..45 {
        list.frame(vec![key(egui::Key::ArrowDown)]);
    }
    list.frame(vec![]);
    list.frame(vec![]);
    assert_eq!(list.active(), Some(39));
    let last = list.rows[39].rect;
    assert!(last.top() >= 0.0 && last.bottom() < 700.0, "{last:?}");
    list.frame(vec![key(egui::Key::Escape)]);
    assert_eq!(list.selected, 0);
    assert!(!egui::Popup::is_any_open(&list.ctx));
    list.open();
    list.enabled = false;
    list.frame(vec![key(egui::Key::ArrowDown)]);
    assert_eq!(list.selected, 0);
    assert!(!egui::Popup::is_any_open(&list.ctx));
}

#[test]
fn dropdown_obeys_the_last_actual_input_when_mouse_and_keys_arrive_together() {
    let mut list = List::new(6);
    list.open();
    let first = list.rows[1].rect.center();
    let second = list.rows[4].rect.center();
    list.frame(vec![
        egui::Event::PointerMoved(first),
        key(egui::Key::ArrowDown),
    ]);
    assert_eq!(list.active(), Some(3));
    list.frame(vec![
        key(egui::Key::ArrowUp),
        egui::Event::PointerMoved(second),
    ]);
    assert_eq!(list.active(), None);
    list.frame(vec![key(egui::Key::ArrowUp)]);
    assert_eq!(list.active(), Some(3));
}

#[test]
fn dropdown_rows_are_square_gapless_uncolored_and_one_point_smaller() {
    for scale in [1.0, 1.5, 2.0] {
        let mut list = List::new(4);
        list.ctx.set_pixels_per_point(scale);
        list.open();
        let output = list.frame(vec![]);
        let arrow = list
            .ctx
            .data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::new("dropdown-disclosure")))
            .expect("Geometric disclosure texture")
            .id();
        let trigger = list.trigger.as_ref().unwrap().rect;
        assert!(
            output.shapes.iter().any(|paint| match &paint.shape {
                egui::Shape::Mesh(mesh) =>
                    mesh.texture_id == arrow && trigger.contains_rect(mesh.calc_bounds()),
                egui::Shape::Rect(rect) =>
                    rect.brush
                        .as_ref()
                        .is_some_and(|brush| brush.fill_texture_id == arrow)
                        && trigger.contains_rect(rect.rect),
                _ => false,
            }),
            "Disclosure must render as an image in every font/scale"
        );
        let preferences = Preferences::default();
        let background = theme::color(&preferences, "Window");
        for row in &list.rows {
            let body = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect == row.rect => Some(rect),
                    _ => None,
                })
                .expect("Shared button body");
            assert_eq!(body.corner_radius, egui::CornerRadius::ZERO);
            assert_eq!(body.stroke, egui::Stroke::NONE);
            if row.enabled() {
                assert_eq!(body.fill, background);
            }
        }
        for adjacent in list.rows.windows(2) {
            assert!((adjacent[0].rect.bottom() - adjacent[1].rect.top()).abs() < 0.01);
            assert_eq!(adjacent[0].rect.x_range(), adjacent[1].rect.x_range());
        }
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text().starts_with("Item ")
            {
                assert_eq!(
                    text.galley.job.sections[0].format.font_id.size,
                    preferences.normal_size - 1.0
                );
            }
        }
    }
}

#[test]
#[ignore = "Native shared-dropdown visual review; external screenshot output"]
fn native_dropdown_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        layer: Value,
        frames: usize,
        blend: bool,
        popup: Option<egui::Id>,
        output: std::path::PathBuf,
        started: std::time::Instant,
        requested: bool,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
            input.focused = true;
            if self.frames == 4 {
                if let Some(rect) = self
                    .popup
                    .and_then(|id| ctx.memory(|memory| memory.area_rect(id)))
                {
                    let height = rect.height() / if self.blend { 8.0 } else { 4.0 };
                    input.events.push(egui::Event::PointerMoved(
                        rect.min + egui::vec2(30.0, height * 1.5),
                    ));
                }
            } else if self.frames == 5 {
                input.events.push(key(egui::Key::ArrowDown));
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            theme::apply(ui.ctx(), self.fixture.preferences());
            ui.set_style(ui.ctx().global_style());
            ui.painter()
                .rect_filled(ui.max_rect(), 0, ui.visuals().panel_fill);
            ui.set_width(350.0);
            let response = if self.blend {
                crate::blend_preview::property_row(ui, |ui| {
                    let mode = self.layer["blendMode"].as_str().unwrap_or("Normal");
                    let button = crate::blend_preview::button(ui, mode, 220.0, true);
                    self.desktop.layer_blend_menu(
                        ui,
                        &self.fixture,
                        "review",
                        &mut self.layer,
                        button,
                    )
                })
            } else {
                crate::fields::row(ui, "Shape", |ui| {
                    let mut selected = self.layer["kind"]
                        .as_str()
                        .unwrap_or("Rectangle")
                        .to_owned();
                    let response = crate::dropdowns::Dropdown::from_id_salt("review-shape")
                        .width(220.0)
                        .selected_text(selected.clone())
                        .show_ui(ui, |ui| {
                            for shape in ["Rectangle", "Sphere", "Polygon", "Star"] {
                                ui.app_selectable_value(&mut selected, shape.into(), shape);
                            }
                        })
                        .response;
                    self.layer["kind"] = json!(selected);
                    response
                })
            };
            let popup = egui::Popup::default_response_id(&response);
            self.popup = Some(popup);
            if self.frames == 1 {
                egui::Popup::open_id(ui.ctx(), popup);
            }
            self.frames += 1;
            if self.frames > 6 && !self.requested && self.started.elapsed().as_secs_f32() > 1.0 {
                assert!(egui::Popup::is_id_open(ui.ctx(), popup));
                self.requested = true;
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
            assert!(self.started.elapsed().as_secs() < 15);
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("External screenshot path"),
    );
    assert!(output.starts_with(crate::test_output::root()));
    let blend = std::env::var_os("ACHIMELD_DROPDOWN_REVIEW_BLEND").is_some();
    let scale = std::env::var("ACHIMELD_SCREENSHOT_SCALE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.0);
    eframe::run_native(
        "Achimeld dropdown review",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([420.0, if blend { 350.0 } else { 200.0 }]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_pixels_per_point(scale);
            Ok(Box::new(Window {
                desktop: Desktop::default(),
                fixture: Fixture::new(),
                layer: json!({"id":new_id(),"kind":"Rectangle","blendMode":"Normal"}),
                frames: 0,
                blend,
                popup: None,
                output,
                started: std::time::Instant::now(),
                requested: false,
            }))
        }),
    )
    .unwrap();
}
