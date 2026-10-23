use super::*;
use achimeld_application::{
    Command, Editor, EditorPort, EntityKind, Preferences, Preview, UiRequest, new_id,
};
use achimeld_imaging::{AssetSource, render};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "art_bank_regressions.rs"]
mod art_bank_regressions;
#[path = "canvas_regressions.rs"]
mod canvas_regressions;
#[path = "dropdown_regressions.rs"]
mod dropdown_regressions;
#[path = "explosion_regressions.rs"]
mod explosion_regressions;
#[path = "gradient_map_regressions.rs"]
mod gradient_map_regressions;
#[path = "help_regressions.rs"]
mod help_regressions;
#[path = "icon_pan_regressions.rs"]
mod icon_pan_regressions;
#[path = "image_drop_regressions.rs"]
mod image_drop_regressions;
#[path = "inspector_panel_regressions.rs"]
mod inspector_panel_regressions;
#[path = "layer_regressions.rs"]
mod layer_regressions;
#[path = "layer_row_regressions.rs"]
mod layer_row_regressions;
#[path = "layer_type_regressions.rs"]
mod layer_type_regressions;
#[path = "node_drag_regressions.rs"]
mod node_drag_regressions;
#[path = "node_navigation_regressions.rs"]
mod node_navigation_regressions;
#[path = "particle_regressions.rs"]
mod particle_regressions;
#[path = "preferences_regressions.rs"]
mod preferences_regressions;
#[path = "sorting_regressions.rs"]
mod sorting_regressions;
#[path = "source_picker_regressions.rs"]
mod source_picker_regressions;
#[path = "style_picker_regressions.rs"]
mod style_picker_regressions;
#[path = "style_viewer_regressions.rs"]
mod style_viewer_regressions;
#[path = "synchronization_regressions.rs"]
mod synchronization_regressions;
#[path = "terminal_regressions.rs"]
mod terminal_regressions;
#[path = "window_layout_regressions.rs"]
mod window_layout_regressions;
struct NoAssets;

fn steam_snapshot() -> Value {
    json!({
        "appId": 0, "isComplete": true, "readAtUtc": "2026-10-15T12:00:00Z",
        "achievements": [
            {"apiName": {"value": "STEAM_FIRST"}, "hidden": false, "setBy": "GameClient", "displayOrder": 0},
            {"apiName": "STEAM_EXPLORER", "hidden": true, "setBy": "GameClient", "displayOrder": 1}
        ],
        "stats": [{"apiName": {"value": "STEAM_DISTANCE"}, "attributes": {"data-type": "Int", "data-set-by": "GameClient", "data-minimum": "0", "data-maximum": "10000", "data-default": "0", "data-increment-only": "true"}}],
        "localizations": [
            {"apiName": {"value": "STEAM_FIRST"}, "field": "AchievementDisplayName", "values": {"English": "First steps on Steam"}},
            {"apiName": "STEAM_FIRST", "field": "AchievementDescription", "values": {"English": "Complete the first chapter."}},
            {"apiName": "STEAM_EXPLORER", "field": "AchievementDisplayName", "values": {"English": "Steam explorer"}},
            {"apiName": "STEAM_DISTANCE", "field": "StatDisplayName", "values": {"English": "Distance on Steam"}}
        ]
    })
}

#[test]
fn steam_sections_share_scroll_and_only_show_remote_values_without_editing() {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    project["lastRemoteSnapshot"] = steam_snapshot();
    fixture.editor = Editor::new(
        achimeld_application::Document::from_parts(
            project,
            fixture.editor.document().catalog().clone(),
        )
        .unwrap(),
    );
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    let mut desktop = Desktop::default();
    desktop.view = desktop::View::Steam;
    fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Text(text) => labels.push((text.galley.text().to_owned(), text.pos)),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, labels);
                }
            }
            _ => {}
        }
    }
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, height, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(760.0, height),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.steam_view(ui, fixture),
        );
        output.textures_delta.clear();
        let mut labels = Vec::new();
        for shape in output.shapes {
            collect(&shape.shape, &mut labels);
        }
        labels
    };
    frame(&mut desktop, &mut fixture, 700.0, vec![]);
    let labels = frame(&mut desktop, &mut fixture, 700.0, vec![]);
    let y = |labels: &[(String, egui::Pos2)], label: &str| {
        labels
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
            .1
            .y
    };
    assert!(y(&labels, "Stats (1)") > y(&labels, "Achievements (2)"));
    assert!(
        labels
            .iter()
            .any(|(text, _)| text == "First steps on Steam")
    );
    assert!(labels.iter().any(|(text, _)| text == "Distance on Steam"));
    assert!(labels.iter().any(|(text, _)| text == "Maximum: 10000"));
    let height = y(&labels, "Stats (1)") + 40.0;
    frame(&mut desktop, &mut fixture, height, vec![]);
    let labels = frame(
        &mut desktop,
        &mut fixture,
        height,
        vec![egui::Event::PointerMoved(egui::pos2(400.0, 170.0))],
    );
    let achievements_y = y(&labels, "Achievements (2)");
    let stats_y = y(&labels, "Stats (1)");
    frame(
        &mut desktop,
        &mut fixture,
        height,
        vec![
            egui::Event::PointerMoved(egui::pos2(400.0, 170.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -60.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let scrolled = frame(&mut desktop, &mut fixture, height, vec![]);
    let delta = y(&scrolled, "Achievements (2)") - achievements_y;
    assert!(delta < 0.0);
    assert!((y(&scrolled, "Stats (1)") - stats_y - delta).abs() < 0.1);
    desktop.filter = "STEAM_DISTANCE".into();
    let filtered = frame(&mut desktop, &mut fixture, 700.0, vec![]);
    assert!(filtered.iter().any(|(text, _)| text == "Distance on Steam"));
    assert!(filtered.iter().any(|(text, _)| text == "Type: Int"));
    assert!(filtered.iter().any(|(text, _)| text == "Maximum: 10000"));
    assert!(
        !filtered
            .iter()
            .any(|(text, _)| text == "First steps on Steam")
    );
    desktop.shortcuts(&ctx, &mut fixture);
    assert_eq!(fixture.editor.document(), &before);
    assert_eq!(fixture.editor.revision(), revision);
    assert_eq!(fixture.requests, 0);
}

pub(crate) fn configure_test_fonts(ctx: &egui::Context) {
    let faces = std::env::var_os("SystemRoot").and_then(|root| {
        let fonts = std::path::PathBuf::from(root).join("Fonts");
        Some([
            std::fs::read(fonts.join("segoeui.ttf")).ok()?,
            std::fs::read(fonts.join("segoeuib.ttf")).ok()?,
        ])
    });
    crate::configure_fonts(ctx, faces);
}

#[test]
fn canvas_wheel_zooms_at_pointer_over_node_bodies_and_icons_in_both_editors() {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        {
            let view = if kind == EntityKind::Stat {
                desktop::View::Stats
            } else {
                desktop::View::Achievements
            };
            for over_art in [false, true] {
                for scroll in [-40.0, 40.0] {
                    let mut fixture = Fixture::new();
                    let mut project = fixture.editor.document().project().clone();
                    project["lastRemoteSnapshot"] = json!({
                        "achievements": project["achievements"],
                        "stats": project["stats"],
                    });
                    fixture.editor = Editor::new(
                        achimeld_application::Document::from_parts(
                            project,
                            fixture.editor.document().catalog().clone(),
                        )
                        .unwrap(),
                    );
                    let id = fixture.editor.document().entities(kind)[0]["id"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    let mut desktop = Desktop::default();
                    desktop.kind = kind;
                    desktop.view = view;
                    let ctx = egui::Context::default();
                    let mut canvas_rect = egui::Rect::NOTHING;
                    let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
                        let preferences = fixture.preferences().clone();
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(1200.0, 900.0),
                                )),
                                events,
                                ..Default::default()
                            },
                            |ui| {
                                canvas_rect = ui.available_rect_before_wrap();
                                desktop.canvas(ui, fixture, &preferences, None);
                            },
                        );
                        output.textures_delta.clear();
                        canvas_rect
                    };
                    frame(&mut desktop, &mut fixture, vec![]);
                    let rect = frame(&mut desktop, &mut fixture, vec![]);
                    let card = ctx
                        .read_response(egui::Id::new(("node", &id)))
                        .unwrap()
                        .rect;
                    let pointer = if over_art {
                        card.min + egui::vec2(40.0, 40.0)
                    } else {
                        card.max - egui::vec2(20.0, 20.0)
                    };
                    frame(
                        &mut desktop,
                        &mut fixture,
                        vec![egui::Event::PointerMoved(pointer)],
                    );
                    let before_zoom = desktop.zoom;
                    let world = (pointer - rect.min - desktop.pan) / before_zoom;
                    frame(
                        &mut desktop,
                        &mut fixture,
                        vec![egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, scroll),
                            phase: egui::TouchPhase::Move,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                    assert!(
                        (desktop.zoom - before_zoom) * scroll > 0.0,
                        "Wheel zoom blocked for {kind:?}, art={over_art}"
                    );
                    let anchored = rect.min + desktop.pan + world * desktop.zoom;
                    assert!(anchored.distance(pointer) < 0.001);
                    assert!(desktop.selection.is_empty());
                    assert!(desktop.node_drag.is_none());
                    assert_eq!(fixture.editor.undo_label(), Some("Canvas camera"));
                }
            }
        }
    }
}

#[test]
fn canvas_wheel_ignores_disabled_outside_and_covered_canvas() {
    for (enabled, covered, pointer) in [
        (false, false, egui::pos2(100.0, 100.0)),
        (true, false, egui::pos2(1000.0, 100.0)),
        (true, true, egui::pos2(100.0, 100.0)),
    ] {
        let mut fixture = Fixture::new();
        let mut desktop = Desktop::default();
        let ctx = egui::Context::default();
        let mut frame = |events| {
            let preferences = fixture.preferences().clone();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 800.0),
                        )),
                        |ui| {
                            if !enabled {
                                ui.disable();
                            }
                            desktop.canvas(ui, &mut fixture, &preferences, None);
                        },
                    );
                    if covered {
                        egui::Area::new(egui::Id::new("canvas-cover"))
                            .order(egui::Order::Foreground)
                            .fixed_pos(egui::Pos2::ZERO)
                            .show(ui.ctx(), |ui| {
                                ui.allocate_exact_size(
                                    egui::vec2(400.0, 400.0),
                                    egui::Sense::click(),
                                );
                            });
                    }
                },
            );
            output.textures_delta.clear();
            (desktop.zoom, desktop.pan)
        };
        frame(vec![]);
        frame(vec![]);
        let before = frame(vec![egui::Event::PointerMoved(pointer)]);
        assert_eq!(
            frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 40.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }]),
            before
        );
    }
}

#[test]
fn legacy_grid_cards_and_camera_restore_render_multiple_rows_without_changing_project() {
    let mut fixture = Fixture::new();
    let mut project = fixture.editor.document().project().clone();
    project["achievements"][0]["canvasColumn"] = json!(0);
    project["achievements"][0]["canvasRow"] = json!(0);
    project["achievements"][1]["canvasColumn"] = json!(17);
    project["achievements"][1]["canvasRow"] = json!(6);
    project["achievementsCamera"] = json!({"zoom":0.31,"offsetX":-100.93,"offsetY":-77.04});
    project["statsCamera"] = json!({"zoom":0.73,"offsetX":-231.31,"offsetY":882.11});
    let document = achimeld_application::Document::from_parts(
        project,
        fixture.editor.document().catalog().clone(),
    )
    .unwrap();
    fixture.editor = Editor::new(document.clone());
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        );
        output.textures_delta.clear();
    };
    frame(&mut desktop, &mut fixture);
    frame(&mut desktop, &mut fixture);
    let cards: Vec<_> = document
        .entities(EntityKind::Achievement)
        .iter()
        .map(|e| {
            ctx.read_response(egui::Id::new((
                "node",
                e["id"].as_str().unwrap().to_owned(),
            )))
            .unwrap()
            .rect
        })
        .collect();
    assert!((cards[1].left() - cards[0].left() - 17.0 * 20.0 * 0.31).abs() < 0.01);
    assert!((cards[1].top() - cards[0].top() - 6.0 * 20.0 * 0.31).abs() < 0.01);
    assert!(!cards[0].intersects(cards[1]));
    assert_eq!(desktop.zoom, 0.31);
    assert_eq!(desktop.pan, egui::vec2(100.93, 77.04));
    desktop.kind = EntityKind::Stat;
    frame(&mut desktop, &mut fixture);
    assert_eq!(desktop.zoom, 0.73);
    assert_eq!(desktop.pan, egui::vec2(231.31, -882.11));
    desktop.kind = EntityKind::Achievement;
    frame(&mut desktop, &mut fixture);
    assert_eq!(desktop.zoom, 0.31);
    assert_eq!(fixture.editor.document(), &document);
    assert!(!fixture.editor.is_dirty());
    assert!(fixture.editor.undo_label().is_none());
}

#[test]
#[ignore = "Requires ACHIMELD_LEGACY_PROJECT with authentic catalog"]
fn real_legacy_project_renders_all_100_achievements_at_their_saved_grid_positions() {
    let mut fixture = Fixture::new();
    let path = std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_LEGACY_PROJECT").expect("Set ACHIMELD_LEGACY_PROJECT"),
    );
    fixture.editor = Editor::new(
        achimeld_persistence::project::ProjectStore::new()
            .load(&path)
            .unwrap(),
    );
    let document = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let size = egui::vec2(5000.0, 5000.0);
    for _ in 0..2 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
    }
    assert_eq!(document.entities(EntityKind::Achievement).len(), 100);
    for entity in document.entities(EntityKind::Achievement) {
        let id = entity["id"].as_str().unwrap().to_owned();
        let response = ctx
            .read_response(egui::Id::new(("node", &id)))
            .unwrap_or_else(|| panic!("Achievement {} was not rendered", entity["apiName"]));
        assert!((response.rect.width() - 336.0 * desktop.zoom).abs() < 0.01);
    }
    assert_eq!(fixture.editor.document(), &document);
    assert!(!fixture.editor.is_dirty());
}

#[test]
fn toolbar_tabs_and_bar_keep_their_bounds_on_hover_focus_and_selection() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for font_size in [12.0, 16.0, 24.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let icon = ctx.load_texture(
                "tab-test",
                egui::ColorImage::from_rgba_unmultiplied(
                    [64, 64],
                    include_bytes!("../assets/toolbar/tab-achievements.rgba"),
                ),
                egui::TextureOptions::LINEAR,
            );
            let image = (icon.id(), egui::vec2(20.0, 20.0));
            let preferences = Preferences {
                normal_size: font_size,
                ..Default::default()
            };
            let mut selected = 0;
            let mut frame = |events| {
                let mut bounds = Vec::new();
                let mut bar = egui::Rect::NOTHING;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1440.0, 920.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        theme::apply(ui.ctx(), &preferences);
                        ui.set_style(ui.ctx().global_style());
                        bar = crate::buttons::bar_panel(ui, "toolbar", 24.0)
                            .show(ui, |ui| {
                                crate::buttons::bar(ui, 24.0, |ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    for (choice, label) in
                                        ["Achievements", "Stats", "Sync", "Steam", "Art Data Bank"]
                                            .into_iter()
                                            .enumerate()
                                    {
                                        bounds.push(
                                            desktop::toolbar_tab(
                                                ui,
                                                &mut selected,
                                                choice,
                                                label,
                                                image,
                                            )
                                            .rect,
                                        );
                                    }
                                    bounds.push(
                                        ui.add(
                                            crate::buttons::Button::new("Following control").bar(),
                                        )
                                        .rect,
                                    );
                                });
                            })
                            .response
                            .rect;
                    },
                );
                output.textures_delta.clear();
                (bar, bounds)
            };
            frame(vec![]);
            let baseline = frame(vec![]);
            assert!(
                baseline.0.height() < 54.0,
                "The bar must lose its outer vertical padding"
            );
            for rect in &baseline.1 {
                assert!((rect.top() - baseline.0.top()).abs() < 0.01);
                assert!(
                    (baseline.0.bottom() - rect.bottom() - 1.0).abs() < 0.01,
                    "scale {scale}, font {font_size}: bar {:?}, button {rect:?}",
                    baseline.0,
                );
            }
            for pair in baseline.1[..5].windows(2) {
                assert!(
                    (pair[0].right() - pair[1].left()).abs() < 0.01,
                    "Tabs must touch"
                );
            }
            for rect in &baseline.1[..5] {
                let pos = rect.center();
                for _ in 0..3 {
                    assert_eq!(frame(vec![egui::Event::PointerMoved(pos)]), baseline);
                }
                for pressed in [true, false] {
                    assert_eq!(
                        frame(vec![egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        }]),
                        baseline
                    );
                }
                assert_eq!(frame(vec![egui::Event::PointerGone]), baseline);
                assert_eq!(frame(vec![]), baseline);
            }
        }
    }
}

impl AssetSource for NoAssets {
    fn image(&mut self, _: &Value) -> Result<std::sync::Arc<image::RgbaImage>, String> {
        Err("No image in this synthetic fixture".into())
    }
    fn font(&mut self, _: &str) -> Option<std::sync::Arc<achimeld_imaging::Font>> {
        None
    }
}
struct Fixture {
    editor: Editor,
    requests: usize,
    last_request: Option<UiRequest>,
    previews: BTreeMap<String, Preview>,
    blend_preview: Option<achimeld_application::ports::LayerBlendPreview>,
    recent_projects: Vec<std::path::PathBuf>,
    styles: Vec<achimeld_application::IconStyle>,
    layout: achimeld_application::LayerLayout,
    viewer: Option<achimeld_application::style_viewer::StyleViewerState>,
}
impl Fixture {
    // Fixtures that render both panes directly choose the optional combined layout.
    fn combined_layers() -> Self {
        let mut fixture = Self::new();
        let preferences = Preferences {
            layer_properties_in_sidebar: false,
            ..fixture.preferences().clone()
        };
        fixture.editor = fixture.editor.with_preferences(preferences);
        fixture
    }

    fn new() -> Self {
        let mut editor = Editor::default();
        editor
            .execute(
                "Fixture",
                Command::Batch(vec![
                    Command::Add(EntityKind::Achievement),
                    Command::Add(EntityKind::Achievement),
                    Command::Add(EntityKind::Stat),
                ]),
                None,
            )
            .unwrap();
        Self {
            editor,
            requests: 0,
            last_request: None,
            previews: BTreeMap::new(),
            blend_preview: None,
            recent_projects: Vec::new(),
            styles: Vec::new(),
            layout: Default::default(),
            viewer: None,
        }
    }
}
impl EditorPort for Fixture {
    fn set_layer_blend_preview(
        &mut self,
        preview: Option<achimeld_application::ports::LayerBlendPreview>,
    ) {
        self.blend_preview = preview;
    }
    fn style_viewer(&self) -> Option<&achimeld_application::style_viewer::StyleViewerState> {
        self.viewer.as_ref()
    }
    fn style_viewer_style_preview(&mut self, style: &str, _edge: usize) -> Option<Preview> {
        self.previews.get(&format!("viewer-style:{style}")).cloned()
    }
    fn editor(&self) -> &Editor {
        &self.editor
    }
    fn execute(
        &mut self,
        label: &str,
        command: Command,
        group: Option<&str>,
    ) -> Result<(), String> {
        self.editor.execute(label, command, group)
    }
    fn finish_group(&mut self) {
        self.editor.finish_group();
    }
    fn undo(&mut self) {
        self.editor.undo();
    }
    fn redo(&mut self) {
        self.editor.redo();
    }
    fn request(&mut self, request: UiRequest) {
        self.requests += 1;
        self.last_request = Some(request);
    }
    fn status(&self) -> &str {
        "Ready"
    }
    fn preferences(&self) -> &Preferences {
        self.editor.preferences()
    }
    fn recent_projects(&self) -> &[std::path::PathBuf] {
        &self.recent_projects
    }
    fn preview(&mut self, id: &str) -> Option<Preview> {
        self.previews.get(id).map(|p| Preview {
            width: p.width,
            height: p.height,
            source_size: p.source_size,
            rgba: p.rgba.clone(),
            fingerprint: p.fingerprint.clone(),
        })
    }
    fn icon_style_preview(
        &mut self,
        style: &str,
        achievement: &str,
        _edge: usize,
    ) -> Option<Preview> {
        self.previews
            .get(&format!("style:{style}:{achievement}"))
            .cloned()
    }
    fn asset_preview(&mut self, atlas: bool, id: Option<&str>) -> Option<Preview> {
        self.previews
            .get(&format!(
                "asset:{}:{}",
                if atlas { "atlas" } else { "image" },
                id.unwrap_or("default")
            ))
            .cloned()
    }
    fn art_file_bytes(&mut self, reference: &Value) -> Option<u64> {
        ["images", "atlases"]
            .into_iter()
            .flat_map(|key| {
                self.editor.document().project()["assets"][key]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .find(|entry| entry["image"] == *reference)
            .and_then(|entry| entry["fileBytes"].as_u64())
    }
    fn layer_preview(&mut self, achievement: &str, layer: &str) -> Option<Preview> {
        let image = achimeld_imaging::render_layer_cancellable(
            self.editor.document(),
            achievement,
            layer,
            &mut NoAssets,
            &|| false,
        )
        .ok()?;
        Some(Preview {
            width: image.width() as usize,
            height: image.height() as usize,
            source_size: [image.width() as usize, image.height() as usize],
            rgba: image.into_raw().into(),
            fingerprint: format!("{achievement}/{layer}/{}", self.editor.revision()),
        })
    }
    fn global_icon_styles(&self) -> &[achimeld_application::IconStyle] {
        &self.styles
    }
    fn layer_layout(&self) -> achimeld_application::LayerLayout {
        self.layout.clone()
    }
    fn save_layer_layout(
        &mut self,
        layout: achimeld_application::LayerLayout,
    ) -> Result<(), String> {
        layout.validate()?;
        self.layout = layout;
        Ok(())
    }
}
#[test]
fn recent_menu_shows_localized_empty_state_and_requests_the_selected_full_path() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let mut initial = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.show(ui, &mut fixture)
    });
    initial.textures_delta.clear();
    crate::localization::language(&ctx, "Polish");
    let frame = |desktop: &mut Desktop, fixture: &Fixture, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    desktop.recent_projects_menu(ui, fixture);
                });
            },
        );
        output.textures_delta.clear();
        output
    };
    let text_position = |output: &egui::FullOutput, label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing menu text: {label}"))
    };
    frame(&mut desktop, &fixture, vec![]);
    let output = frame(&mut desktop, &fixture, vec![]);
    let empty = text_position(&output, "Brak ostatnich plików");
    for pressed in [true, false] {
        frame(
            &mut desktop,
            &fixture,
            vec![egui::Event::PointerButton {
                pos: empty,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    assert!(desktop.requests.is_empty());
    fixture.recent_projects = (0..10)
        .map(|i| std::path::PathBuf::from(format!("C:/Projects/Game-{i}/Game.achv")))
        .collect();
    frame(&mut desktop, &fixture, vec![]);
    let output = frame(&mut desktop, &fixture, vec![]);
    for path in &fixture.recent_projects {
        text_position(&output, &path.to_string_lossy());
    }
    let chosen = fixture.recent_projects[7].clone();
    let pos = text_position(&output, &chosen.to_string_lossy());
    for pressed in [true, false] {
        frame(
            &mut desktop,
            &fixture,
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
    assert!(
        matches!(desktop.requests.as_slice(), [UiRequest::OpenRecentProject(path)] if path == &chosen)
    );
    // The queued action shares the normal invalid-draft gate.
    let catalog_id = fixture
        .editor
        .document()
        .catalog()
        .rows
        .keys()
        .next()
        .unwrap()
        .clone();
    desktop
        .localization_drafts
        .insert((catalog_id, "English".into()), ("bad".into(), "bad".into()));
    let before = fixture.editor.document().clone();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.show(ui, &mut fixture)
    });
    output.textures_delta.clear();
    assert_eq!(fixture.requests, 0);
    assert_eq!(fixture.editor.document(), &before);
    assert!(desktop.error.is_some());
}

#[test]
fn stale_settings_cannot_erase_assets_imported_after_opening_the_window() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    desktop.open_settings(&fixture);
    let mut assets = fixture.editor.document().project()["assets"].clone();
    assets["images"] =
        json!([{"id":new_id(),"image":{"kind":"Absolute","path":"C:\\icons\\source.png"}}]);
    fixture
        .editor
        .execute(
            "Import image",
            Command::ProjectSettings {
                name: "Changed project".into(),
                app_id: 480,
                atlas: None,
                assets: Some(assets.clone()),
            },
            None,
        )
        .unwrap();
    let imported = fixture.editor.document().clone();
    assert!(!desktop.apply_settings(&mut fixture));
    assert_eq!(fixture.editor.document(), &imported);
    desktop.open_settings(&fixture);
    desktop.project_name = "Renamed".into();
    assert!(desktop.apply_settings(&mut fixture));
    assert_eq!(fixture.editor.document().project()["assets"], assets);
    fixture.editor.undo();
    assert_eq!(fixture.editor.document(), &imported);
}
#[test]
fn disabled_editor_ignores_project_and_editing_shortcuts() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let document = fixture.editor.document().clone();
    for key in [egui::Key::O, egui::Key::N, egui::Key::S, egui::Key::Z] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                }],
                ..Default::default()
            },
            |ui| {
                ui.disable();
                desktop.show(ui, &mut fixture);
            },
        );
        output.textures_delta.clear();
        assert_eq!(fixture.requests, 0);
        assert_eq!(fixture.editor.document(), &document);
    }
}

#[test]
fn progressive_textures_use_gpu_mipmaps_and_reuse_the_texture() {
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    let preview = |edge: usize| Preview {
        width: edge,
        height: edge,
        source_size: [512, 512],
        rgba: vec![128; edge * edge * 4].into(),
        fingerprint: edge.to_string(),
    };
    let mut id = None;
    let mut quick = ctx.run_ui(egui::RawInput::default(), |ui| {
        id = desktop.preview_texture(ui.ctx(), "progressive", preview(64));
    });
    let options = quick.textures_delta.set.get(&id.unwrap()).unwrap()[0].options;
    assert_eq!(options.minification, egui::TextureFilter::Linear);
    assert_eq!(options.magnification, egui::TextureFilter::Linear);
    assert_eq!(options.mipmap_mode, Some(egui::TextureFilter::Linear));
    quick.textures_delta.clear();
    let mut refined_id = None;
    let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
        refined_id = desktop.preview_texture(ui.ctx(), "progressive", preview(512));
    });
    assert_eq!(id, refined_id);
    assert!(
        full.textures_delta
            .set
            .get(&id.unwrap())
            .unwrap()
            .iter()
            .any(|delta| delta.image.size() == [512, 512] && delta.options == options)
    );
    full.textures_delta.clear();
    let mut unchanged = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.preview_texture(ui.ctx(), "progressive", preview(512));
    });
    assert!(unchanged.textures_delta.set.is_empty());
    unchanged.textures_delta.clear();
    assert_eq!(super::desktop::mipmap_bytes([512, 512]), 1_398_100);
    assert_eq!(super::desktop::mipmap_bytes([1, 4]), 28);
}

#[test]
fn hundred_canvas_thumbnails_remain_resident_and_resize_without_flashing() {
    let ctx = egui::Context::default();
    let mut desktop = Desktop::default();
    let mut port = Fixture::new();
    let ids: Vec<_> = (0..100).map(|i| format!("canvas-card-{i}")).collect();
    for id in &ids {
        port.previews.insert(
            id.clone(),
            Preview {
                width: 64,
                height: 64,
                source_size: [512, 512],
                rgba: vec![255; 64 * 64 * 4].into(),
                fingerprint: format!("{id}-64"),
            },
        );
    }
    let mut handles = Vec::new();
    for frame in 0..5 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let current: Vec<_> = ids
                .iter()
                .map(|id| {
                    desktop
                        .canvas_texture(ui.ctx(), &mut port, id, 30.0)
                        .unwrap()
                })
                .collect();
            if frame == 0 {
                handles = current;
            } else {
                assert_eq!(current, handles);
            }
        });
        if frame > 0 {
            assert!(
                output.textures_delta.set.is_empty(),
                "Steady canvas must not upload textures"
            );
        }
        output.textures_delta.clear();
    }
    assert_eq!(desktop.textures.len(), 100);
    let id = &ids[0];
    port.previews.remove(id);
    let mut pending = ctx.run_ui(egui::RawInput::default(), |ui| {
        assert_eq!(
            desktop.canvas_texture(ui.ctx(), &mut port, id, 160.0),
            Some(handles[0])
        );
    });
    assert!(pending.textures_delta.set.is_empty());
    pending.textures_delta.clear();
    port.previews.insert(
        id.clone(),
        Preview {
            width: 256,
            height: 256,
            source_size: [512, 512],
            rgba: vec![255; 256 * 256 * 4].into(),
            fingerprint: format!("{id}-256"),
        },
    );
    let mut refined = ctx.run_ui(egui::RawInput::default(), |ui| {
        assert_eq!(
            desktop.canvas_texture(ui.ctx(), &mut port, id, 160.0),
            Some(handles[0])
        );
    });
    assert!(
        refined.textures_delta.set[&handles[0]]
            .iter()
            .any(|delta| delta.image.size() == [256, 256]
                && delta.options.mipmap_mode == Some(egui::TextureFilter::Linear))
    );
    refined.textures_delta.clear();
}

#[test]
fn shifted_shortcuts_trigger_save_as_and_redo_without_becoming_save_and_undo() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let modifiers = egui::Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let input = |key| egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input(egui::Key::S), |ui| desktop.show(ui, &mut fixture));
    output.textures_delta.clear();
    assert!(matches!(
        fixture.last_request,
        Some(UiRequest::SaveProjectAs)
    ));
    assert_eq!(fixture.requests, 1);
    fixture
        .editor
        .execute("Add stat", Command::Add(EntityKind::Stat), None)
        .unwrap();
    let edited = fixture.editor.document().clone();
    fixture.editor.undo();
    let mut output = ctx.run_ui(input(egui::Key::Z), |ui| desktop.show(ui, &mut fixture));
    output.textures_delta.clear();
    assert_eq!(fixture.editor.document(), &edited);
}
#[test]
fn layer_pointer_drag_preserves_model_until_drop_and_commits_one_undo() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let ids: Vec<_> = (0..3).map(|_| new_id()).collect();
    entity["icon"] = json!({"layers":ids.iter().enumerate().map(|(i,id)|json!({"id":id,"displayName":format!("Layer {i}"),"kind":"Solid","solidColorHex":"#FFFFFFFF"})).collect::<Vec<_>>()});
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
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame = |desktop: &mut Desktop, fixture: &mut Fixture, events: Vec<egui::Event>| {
        time += 0.05;
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        );
        output.textures_delta.clear();
    };
    frame(&mut desktop, &mut fixture, vec![]);
    frame(&mut desktop, &mut fixture, vec![]);
    let first = ctx
        .read_response(egui::Id::new(("layer-drag", &id, &ids[2])))
        .unwrap()
        .rect
        .center();
    let last = desktop.layer_palette.rows[&ids[0]].center_bottom() - egui::vec2(0.0, 2.0);
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    frame(
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::PointerMoved(first),
            egui::Event::PointerButton {
                pos: first,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(last)],
    );
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerMoved(last + egui::vec2(0.0, -2.0))],
    );
    assert_eq!(fixture.editor.document(), &before);
    assert_eq!(fixture.editor.revision(), revision);
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerButton {
            pos: last,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(fixture.editor.revision(), revision + 1);
    assert_eq!(fixture.editor.undo_label(), Some("Reorder icon layers"));
    assert_eq!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap()["icon"]["layers"][0]["id"],
        ids[2]
    );
    fixture.editor.undo();
    assert_eq!(fixture.editor.document(), &before);
}
#[test]
fn preference_preview_colors_are_applied_and_cancel_restores_saved_palette() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let mut preview = fixture.preferences().clone();
    preview.palette.insert("Panel".into(), [210, 220, 230, 255]);
    preview
        .palette
        .insert("Window".into(), [240, 241, 242, 255]);
    desktop.preferences = Some(preview.clone());
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.show(ui, &mut fixture)
    });
    output.textures_delta.clear();
    assert_eq!(
        ctx.global_style().visuals.panel_fill,
        theme::color(&preview, "Panel")
    );
    assert!(!ctx.global_style().visuals.dark_mode);
    assert_eq!(fixture.preferences(), &Preferences::default());
    desktop.preferences = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        desktop.show(ui, &mut fixture)
    });
    output.textures_delta.clear();
    assert_eq!(
        ctx.global_style().visuals.panel_fill,
        theme::color(fixture.preferences(), "Panel")
    );
    assert!(ctx.global_style().visuals.dark_mode);
    assert!(fixture.editor.undo_label().is_some());
}
#[test]
fn inspector_keeps_its_width_from_empty_startup_through_selection_and_tab_changes() {
    let mut fixture = Fixture::new();
    let original = fixture.editor.document().clone();
    let id = original.entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    configure_test_fonts(&ctx);
    ctx.set_pixels_per_point(1.25);
    let panel_id = egui::Id::new("inspector");
    let width = || {
        egui::containers::panel::PanelState::load(&ctx, panel_id)
            .unwrap()
            .size()
            .x
    };

    for _ in 0..3 {
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        assert!(
            (width() * 1.25 - 360.0).abs() < 1.0,
            "Empty inspector width: {}",
            width()
        );
    }
    desktop.selection = vec![id];
    for tab in [
        desktop::InspectorTab::General,
        desktop::InspectorTab::Localization,
        desktop::InspectorTab::Icon,
    ] {
        desktop.inspector_tab = tab;
        for _ in 0..3 {
            draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
            assert!(
                (width() - 288.0).abs() <= 1.0,
                "Selected inspector width: {}",
                width()
            );
        }
    }
    // A user-resized width must also survive clearing the selection.
    let mut state = egui::containers::panel::PanelState::load(&ctx, panel_id).unwrap();
    state.outer_rect.min.x = state.outer_rect.max.x - 400.0;
    ctx.data_mut(|data| data.insert_persisted(panel_id, state));
    desktop.selection.clear();
    for _ in 0..3 {
        draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
        assert!(
            (width() - 400.0).abs() < 1.0,
            "Resized inspector width: {}",
            width()
        );
    }
    assert_eq!(fixture.editor.document(), &original);
}

#[test]
fn drawing_inspectors_never_edits_document_or_adds_history() {
    let mut fixture = Fixture::new();
    let original = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    let ctx = egui::Context::default();
    for tab in [
        desktop::InspectorTab::General,
        desktop::InspectorTab::Icon,
        desktop::InspectorTab::Localization,
    ] {
        desktop.inspector_tab = tab;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| desktop.show(ui, &mut fixture));
            output.textures_delta.clear();
        }
    }
    assert_eq!(fixture.editor.document(), &original);
    assert_eq!(fixture.editor.revision(), revision);
    assert_eq!(fixture.requests, 0);
}
#[test]
fn selecting_every_layer_kind_never_normalizes_the_saved_recipe() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let layers: Vec<_> = [
        "Solid",
        "Shape",
        "Curves",
        "Contrast",
        "HueSaturation",
        "SelectiveColor",
        "UnsharpMask",
        "Text",
        "Text3d",
        "Artifact3d",
        "AtlasTile",
        "StandaloneImage",
        "AchievementImage",
        "Gradient",
        "GradientOverlay",
        "SdfVignette",
        "Folder",
    ]
    .into_iter()
    .map(|kind| json!({"id":new_id(),"kind":kind,"displayName":kind,"effects":[]}))
    .collect();
    let ids: Vec<_> = layers
        .iter()
        .map(|l| l["id"].as_str().unwrap().to_owned())
        .collect();
    entity["icon"] = json!({"layers":layers});
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
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    for id in ids {
        let selected_kind = before.entities(EntityKind::Achievement)[0]["icon"]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|layer| layer["id"] == id)
            .unwrap()["kind"]
            .as_str()
            .unwrap();
        desktop.selected_layer = Some(id);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 2200.0),
                )),
                ..Default::default()
            },
            |ui| desktop.show(ui, &mut fixture),
        );
        output.textures_delta.clear();
        assert_eq!(
            fixture.editor.revision(),
            revision,
            "Drawing {selected_kind} must not change stored values"
        );
        assert_eq!(fixture.editor.document(), &before);
        assert!(desktop.entity_drafts.is_empty());
    }
}
#[test]
fn title_bar_controls_preserve_close_checks_and_emit_window_actions() {
    let mut fixture = Fixture::new();
    let mut desktop = Desktop::default();
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut frame =
        |desktop: &mut Desktop, fixture: &mut Fixture, events, (maximized, fullscreen)| {
            time += 0.05;
            let mut input = egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .maximized = Some(maximized);
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .fullscreen = Some(fullscreen);
            let mut output = ctx.run_ui(input, |ui| desktop.show(ui, fixture));
            output.textures_delta.clear();
            output
                .viewport_output
                .remove(&egui::ViewportId::ROOT)
                .unwrap()
                .commands
        };
    frame(&mut desktop, &mut fixture, vec![], (false, false));
    frame(&mut desktop, &mut fixture, vec![], (false, false));
    for (id, state, expected) in [
        (
            "window-minimize",
            (false, false),
            egui::ViewportCommand::Minimized(true),
        ),
        (
            "window-maximize",
            (false, false),
            egui::ViewportCommand::Maximized(true),
        ),
        (
            "window-maximize",
            (true, false),
            egui::ViewportCommand::Maximized(false),
        ),
        (
            "window-maximize",
            (false, true),
            egui::ViewportCommand::Fullscreen(false),
        ),
        ("window-close", (false, true), egui::ViewportCommand::Close),
    ] {
        let pos = ctx.read_response(egui::Id::new(id)).unwrap().rect.center();
        let down = frame(
            &mut desktop,
            &mut fixture,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            state,
        );
        assert!(!down.contains(&egui::ViewportCommand::StartDrag));
        let commands = frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            state,
        );
        if id == "window-close" {
            assert!(matches!(fixture.last_request, Some(UiRequest::Exit)));
            assert!(!commands.contains(&expected));
        } else {
            assert!(commands.contains(&expected), "{id}: {commands:?}");
        }
    }
    let id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let entity = fixture
        .editor
        .document()
        .entity(EntityKind::Achievement, &id)
        .unwrap()
        .clone();
    desktop.entity_drafts.insert(
        id,
        desktop::EntityDraft {
            base: entity.clone(),
            value: entity,
            error: "Invalid draft".into(),
        },
    );
    let requests = fixture.requests;
    let pos = ctx
        .read_response(egui::Id::new("window-close"))
        .unwrap()
        .rect
        .center();
    frame(
        &mut desktop,
        &mut fixture,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        (false, false),
    );
    let commands = frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        (false, false),
    );
    assert_eq!(fixture.requests, requests);
    assert!(desktop.error.is_some());
    assert!(!commands.contains(&egui::ViewportCommand::Close));

    // Let earlier control clicks age out of egui's triple-click interval.
    for _ in 0..14 {
        frame(&mut desktop, &mut fixture, vec![], (false, false));
    }
    let pos = ctx
        .read_response(egui::Id::new("window-drag"))
        .unwrap()
        .rect
        .center();
    let press = |pos| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let release = |pos| {
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    };
    let commands = frame(&mut desktop, &mut fixture, press(pos), (false, false));
    assert!(commands.contains(&egui::ViewportCommand::StartDrag));
    frame(&mut desktop, &mut fixture, release(pos), (false, false));
    frame(&mut desktop, &mut fixture, press(pos), (false, false));
    let commands = frame(&mut desktop, &mut fixture, release(pos), (false, false));
    assert!(commands.contains(&egui::ViewportCommand::Maximized(true)));

    for _ in 0..14 {
        frame(&mut desktop, &mut fixture, vec![], (false, true));
    }
    frame(&mut desktop, &mut fixture, press(pos), (false, true));
    frame(&mut desktop, &mut fixture, release(pos), (false, true));
    frame(&mut desktop, &mut fixture, press(pos), (false, true));
    let commands = frame(&mut desktop, &mut fixture, release(pos), (false, true));
    assert!(commands.contains(&egui::ViewportCommand::Fullscreen(false)));

    let pos = egui::pos2(3.0, 3.0);
    frame(&mut desktop, &mut fixture, vec![], (false, false));
    let commands = frame(&mut desktop, &mut fixture, press(pos), (false, false));
    assert!(commands.contains(&egui::ViewportCommand::BeginResize(
        egui::ResizeDirection::NorthWest
    )));
    frame(&mut desktop, &mut fixture, release(pos), (false, false));
    // Expanded windows have no resize grips.
    for state in [(true, false), (false, true)] {
        frame(&mut desktop, &mut fixture, vec![], state);
        let commands = frame(&mut desktop, &mut fixture, press(pos), state);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::BeginResize(_)))
        );
        frame(&mut desktop, &mut fixture, release(pos), state);
    }
}

#[test]
fn about_dismissal_is_not_an_edit_and_blocks_editor_shortcuts() {
    let mut desktop = Desktop::default();
    let mut fixture = Fixture::new();
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    let ctx = egui::Context::default();
    desktop.open_about();
    let frame = |desktop: &mut Desktop, fixture: &mut Fixture, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 920.0),
                )),
                events,
                ..Default::default()
            },
            |ui| desktop.show(ui, fixture),
        );
        output.textures_delta.clear();
    };
    frame(&mut desktop, &mut fixture, vec![]);
    assert!(desktop.about);
    frame(
        &mut desktop,
        &mut fixture,
        vec![egui::Event::Key {
            key: egui::Key::N,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        }],
    );
    assert_eq!(fixture.requests, 0);
    // Even an unrelated invalid entity draft must not block dismissal.
    let entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    desktop.entity_drafts.insert(
        entity["id"].as_str().unwrap().into(),
        desktop::EntityDraft {
            base: entity.clone(),
            value: entity,
            error: "Invalid".into(),
        },
    );
    for key in [egui::Key::Escape, egui::Key::Enter] {
        frame(
            &mut desktop,
            &mut fixture,
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!desktop.about);
        assert!(!desktop.entity_drafts.is_empty());
        assert!(matches!(
            fixture.last_request,
            Some(UiRequest::AboutDismissed)
        ));
        desktop.open_about();
        frame(&mut desktop, &mut fixture, vec![]);
        assert!(desktop.about);
    }
    assert_eq!(fixture.requests, 2);
    assert_eq!(fixture.editor.document(), &before);
    assert_eq!(fixture.editor.revision(), revision);
}

#[test]
#[ignore = "Native screenshot fixture; use a dedicated output path under the external build output directory"]
fn native_desktop_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native Sort panel screenshot; dedicated external output path"]
fn native_sort_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native color window screenshot fixture; use a dedicated workspace output path"]
fn native_colors_screenshot() {
    native_window(true, false, false, false, false);
}
#[test]
#[ignore = "Native About screenshot fixture; use a dedicated workspace output path"]
fn native_about_screenshot() {
    native_window(false, true, false, false, false);
}
#[test]
#[ignore = "Native change log screenshot fixture; external output path"]
fn native_change_log_screenshot() {
    native_window(false, true, false, false, false);
}
#[test]
#[ignore = "Native root screenshot of the embedded floating fallback; dedicated workspace output path"]
fn native_embedded_layers_screenshot() {
    native_window(false, false, true, false, false);
}
#[test]
#[ignore = "Native overview and connector screenshot; dedicated external output path"]
fn native_canvas_overview_screenshot() {
    native_window(false, false, true, false, false);
}
#[test]
#[ignore = "Native canvas context menu screenshot; dedicated external output path"]
fn native_canvas_context_menu_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native local style achievement screenshot; dedicated external output path"]
fn native_style_achievement_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native reusable art image screenshot; dedicated external output path"]
fn native_art_image_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native App style screenshot fixture; dedicated workspace output path"]
fn native_app_style_screenshot() {
    native_window(false, false, false, true, false);
}

#[test]
#[ignore = "Native Art Data Bank screenshot; dedicated external output path"]
fn native_art_bank_screenshot() {
    native_window(false, false, false, false, false);
}

#[test]
fn art_bank_draws_all_tabs_at_narrow_and_wide_widths_without_edits() {
    let mut fixture = Fixture::new();
    let mut assets = fixture.editor.document().project()["assets"].clone();
    assets["images"] = json!([{"id":"image-fixture","displayName":"Imported artwork","image":{"kind":"RelativeToProject","path":"missing.png"},"kind":"User"}]);
    assets["atlases"] = json!([{"id":"atlas-fixture","displayName":"Atlas artwork","image":{"kind":"RelativeToProject","path":"atlas.png"}}]);
    fixture
        .editor
        .execute(
            "Art",
            Command::ProjectSettings {
                name: "Art fixture".into(),
                app_id: 0,
                atlas: None,
                assets: Some(assets),
            },
            None,
        )
        .unwrap();
    let before = fixture.editor.document().clone();
    let revision = fixture.editor.revision();
    for width in [480.0, 760.0, 1200.0] {
        let ctx = egui::Context::default();
        let mut desktop = Desktop::default();
        desktop.view = desktop::View::Art;
        for tab in [
            crate::art_bank::ArtTab::General,
            crate::art_bank::ArtTab::Icon,
            crate::art_bank::ArtTab::Atlases,
        ] {
            desktop.art_bank.select_tab(tab);
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 820.0),
                        )),
                        ..Default::default()
                    },
                    |ui| desktop.art_bank_view(ui, &mut fixture),
                );
                output.textures_delta.clear();
                assert_eq!(fixture.editor.document(), &before);
                assert_eq!(fixture.editor.revision(), revision);
            }
        }
    }
}
#[test]
#[ignore = "Native Sync view screenshot; dedicated external output path"]
fn native_sync_screenshot() {
    native_window(false, false, false, false, true);
}
#[test]
#[ignore = "Native Steam screenshot; dedicated external output path"]
fn native_steam_screenshot() {
    native_window(false, false, false, false, false);
}
#[test]
#[ignore = "Native hover text and square title-menu screenshot; workspace output path"]
fn native_button_hover_screenshot() {
    use winit::platform::windows::EventLoopBuilderExtWindows as _;
    struct Window {
        output: std::path::PathBuf,
        started: std::time::Instant,
        pointer: Option<egui::Pos2>,
        requested: bool,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
            if let Some(pos) = self.pointer {
                input.events.push(egui::Event::PointerMoved(pos));
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            let preferences = Preferences::default();
            theme::apply(ui.ctx(), &preferences);
            ui.set_style(ui.ctx().global_style());
            ui.painter()
                .rect_filled(ui.max_rect(), 0, theme::color(&preferences, "Window"));
            egui::Frame::new()
                .fill(theme::color(&preferences, "Panel"))
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        self.pointer = Some(
                            crate::chrome::title_menu_bar(
                                ui,
                                ["File", "Edit", "Tools", "Help"],
                                |_, _| {},
                            )[0]
                            .rect
                            .center(),
                        );
                    });
                });
            ui.add_space(24.0);
            ui.heading("Button hover");
            ui.label("Bold, contrasting text on the light fill; square title menus.");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add(crate::buttons::Button::new("Regular"));
                let id = ui.next_auto_id();
                ui.memory_mut(|memory| memory.request_focus(id));
                ui.add(crate::buttons::Button::new("Hover / keyboard focus"));
            });
            if !self.requested && self.started.elapsed().as_secs_f32() > 0.75 {
                self.requested = true;
                ui.ctx()
                    .send_viewport_cmd(
                        egui::ViewportCommand::Screenshot(egui::UserData::default()),
                    );
            }
            for event in ui.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<_> = image
                        .pixels
                        .iter()
                        .flat_map(|p| p.to_srgba_unmultiplied())
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
            }
            assert!(
                self.started.elapsed().as_secs() < 20,
                "Hover screenshot did not arrive"
            );
            ui.ctx().request_repaint();
        }
    }
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
    ))
    .unwrap();
    let boundary = std::path::absolute(crate::test_output::root()).unwrap();
    assert!(output.starts_with(boundary) && output.extension().is_some_and(|s| s == "png"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([680.0, 240.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "Achimeld hover fixture",
        options,
        Box::new(move |_| {
            Ok(Box::new(Window {
                output,
                started: std::time::Instant::now(),
                pointer: None,
                requested: false,
            }))
        }),
    )
    .unwrap();
}
fn native_window(colors: bool, about: bool, floating: bool, app_style: bool, sync: bool) {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Window {
        desktop: Desktop,
        fixture: Fixture,
        output: std::path::PathBuf,
        frames: usize,
        screenshot_viewport: egui::ViewportId,
        capture_frame: usize,
        context_menu: bool,
    }
    impl eframe::App for Window {
        fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
            if self.context_menu {
                input.focused = true;
            }
            if self.context_menu && [2, 3].contains(&self.frames) {
                let entity = &self.fixture.editor.document().entities(self.desktop.kind)[0];
                let id = entity["id"].as_str().unwrap();
                let pos = ctx
                    .read_response(egui::Id::new(("node", id)))
                    .unwrap()
                    .rect
                    .center();
                input.events.extend([
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed: self.frames == 2,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
        }
        fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
            self.desktop.show(ui, &mut self.fixture);
            self.frames += 1;
            if self.frames == self.capture_frame {
                if self.context_menu {
                    assert!(
                        self.desktop.canvas_menu.is_some(),
                        "The context menu must be visible in the native capture"
                    );
                }
                ui.ctx().send_viewport_cmd_to(
                    self.screenshot_viewport,
                    egui::ViewportCommand::Screenshot(egui::UserData::default()),
                );
            }
            if let Some(image) = ui.ctx().input_for(self.screenshot_viewport, |input| {
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
                    .flat_map(|p| p.to_srgba_unmultiplied())
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
                self.frames < 1000,
                "Native screenshot event was not received"
            );
            ui.ctx().request_repaint();
        }
    }
    let mut fixture = Fixture::new();
    let first = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut entity = fixture
        .editor
        .document()
        .entity(EntityKind::Achievement, &first)
        .unwrap()
        .clone();
    entity["icon"] = json!({"layers":[{"id":new_id(),"displayName":"Gradient","kind":"Gradient","visible":true,"opacity":1.0,"gradient":{"kind":"Linear","angle":45.0,"stops":[{"offset":0.0,"colorHex":"#FF4672DE"},{"offset":1.0,"colorHex":"#FF9B70E8"}]}},{"id":new_id(),"displayName":"Shape","kind":"Shape","visible":true,"opacity":0.6,"solidColorHex":"#FFFFFFFF","shape":{"kind":"Sphere","scaleX":0.5,"scaleY":0.5,"colorHex":"#FFFFFFFF"}}]});
    let text_alignment = std::env::var_os("ACHIMELD_SCREENSHOT_TEXT_ALIGNMENT").is_some();
    let text_layer = new_id();
    if text_alignment {
        entity["icon"]["layers"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": text_layer,
                "kind": "Text",
                "displayName": "Text",
                "visible": true,
                "opacity": 1.0,
                "text": {
                    "content": "00", "fontFamily": "Segoe UI", "fontSize": 128.0,
                    "colorHex": "#FFFFC4F3", "alignment": "BottomRight"
                }
            }));
    }
    if floating {
        fixture.layout.floating = true;
        let folder = new_id();
        let mask = new_id();
        let layers = entity["icon"]["layers"].as_array_mut().unwrap();
        layers[1]["parentFolderId"] = json!(folder);
        layers[1]["effects"] = json!([{"id":new_id(),"kind":"Outline","visible":true,"colorHex":"#FFFFFFFF","width":4.0}]);
        layers.push(json!({"id":mask,"kind":"Gradient","visible":false,"gradient":{"kind":"Linear","angle":90.0,"stops":[{"offset":0.0,"colorHex":"#FFFFFFFF"},{"offset":1.0,"colorHex":"#00FFFFFF"}]}}));
        layers.push(json!({"id":folder,"kind":"Folder","displayName":"Artwork","maskLayerId":mask,"visible":true,"opacity":0.9}));
        layers.push(json!({"id":new_id(),"kind":"Curves","displayName":"Curves","visible":true,"curves":{"points":[{"input":0.0,"output":0.0},{"input":0.45,"output":0.6},{"input":1.0,"output":1.0}]}}));
    }
    fixture
        .editor
        .execute(
            "Icon",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: first.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let mut desktop = Desktop::default();
    if about {
        desktop.open_about();
        if std::env::var_os("ACHIMELD_SCREENSHOT_CHANGE_LOG").is_some() {
            desktop.open_change_log();
        }
    }
    if text_alignment {
        desktop.selection = vec![first.clone()];
        desktop.selected_layer = Some(text_layer);
    }
    if std::env::var_os("ACHIMELD_SCREENSHOT_STEAM").is_some() {
        let mut project = fixture.editor.document().project().clone();
        project["lastRemoteSnapshot"] = steam_snapshot();
        fixture.editor = Editor::new(
            achimeld_application::Document::from_parts(
                project,
                fixture.editor.document().catalog().clone(),
            )
            .unwrap(),
        );
        desktop.view = desktop::View::Steam;
    }
    if std::env::var_os("ACHIMELD_SCREENSHOT_ART_BANK").is_some() {
        let atlas =
            std::env::var("ACHIMELD_SCREENSHOT_ART_BANK").is_ok_and(|value| value == "atlases");
        let mut assets = fixture.editor.document().project()["assets"].clone();
        let brand =
            image::load_from_memory(include_bytes!("../../achimeld-app/assets/achimeld.png"))
                .unwrap()
                .to_rgba8();
        let mut entries = Vec::new();
        for (name, size, bytes) in [
            ("Achimeld emblem", [1024, 1024], 428_512_u64),
            ("Achievement badge", [512, 512], 138_240),
            ("Menu artwork", [1920, 1080], 1_965_432),
        ] {
            let id = new_id();
            entries.push(json!({"id":id,"displayName":name,"image":{"kind":"RelativeToProject","path":format!("preview.art-data-bank/{id}.png")},"kind":"User","fileBytes":bytes,"importSource":{"kind":"File","path":format!("C:\\Artwork\\Achimeld\\{name}.png")}}));
            fixture.previews.insert(
                format!("asset:{}:{id}", if atlas { "atlas" } else { "image" }),
                Preview {
                    width: brand.width() as usize,
                    height: brand.height() as usize,
                    source_size: size,
                    rgba: brand.as_raw().clone().into(),
                    fingerprint: id.clone(),
                },
            );
        }
        desktop.view = desktop::View::Art;
        desktop.art_bank.select_tab(if atlas {
            crate::art_bank::ArtTab::Atlases
        } else {
            crate::art_bank::ArtTab::Icon
        });
        desktop.art_bank.select(entries[0]["id"].as_str().unwrap());
        assets[if atlas { "atlases" } else { "images" }] = json!(entries);
        fixture
            .editor
            .execute(
                "Art bank fixture",
                Command::ProjectSettings {
                    name: "Artwork library preview".into(),
                    app_id: 0,
                    atlas: None,
                    assets: Some(assets),
                },
                None,
            )
            .unwrap();
    }
    desktop.selection = vec![first];
    if let Ok(view) = std::env::var("ACHIMELD_SCREENSHOT_SORT") {
        desktop.sort_open = true;
        if view == "stats" {
            desktop.view = desktop::View::Stats;
            desktop.kind = EntityKind::Stat;
            desktop.selection.clear();
        }
        if std::env::var_os("ACHIMELD_SCREENSHOT_SORT_OPTIONS").is_some() {
            let preferences = Preferences {
                sort_nodes_by_icon_style: true,
                new_line_per_icon_style: true,
                fill_nodes_by_columns: true,
                order_selected_nodes_only: true,
                sort_nodes_by_stat_activation: true,
                include_helpers_in_sorting: true,
                ..fixture.preferences().clone()
            };
            fixture
                .editor
                .execute(
                    "Sort preview preferences",
                    Command::Preferences(preferences),
                    None,
                )
                .unwrap();
        }
    }
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let context_menu = std::env::var("ACHIMELD_SCREENSHOT_CONTEXT_MENU").ok();
    if let Some(kind) = &context_menu {
        desktop.inspector_tab = desktop::InspectorTab::General;
        if kind == "stats" {
            desktop.view = desktop::View::Stats;
            desktop.kind = EntityKind::Stat;
            desktop.selection.clear();
        }
    }
    let overview = std::env::var_os("ACHIMELD_SCREENSHOT_OVERVIEW").is_some();
    if overview {
        fixture
            .editor
            .execute(
                "Overview cards",
                Command::Batch(
                    (0..22)
                        .map(|_| Command::Add(EntityKind::Achievement))
                        .collect(),
                ),
                None,
            )
            .unwrap();
        let recipe = fixture.editor.document().entities(EntityKind::Achievement)[0]["icon"].clone();
        let stat = fixture.editor.document().entities(EntityKind::Stat)[0]["id"].clone();
        for (index, mut entity) in fixture
            .editor
            .document()
            .entities(EntityKind::Achievement)
            .to_vec()
            .into_iter()
            .enumerate()
        {
            entity["canvasColumn"] = json!((index % 4) * 18);
            entity["canvasRow"] = json!((index / 4) * 7);
            entity["icon"] = recipe.clone();
            if index >= 4 {
                entity["progressStatId"] = stat.clone();
                entity["unlockValue"] = json!(1);
            }
            fixture
                .editor
                .execute(
                    "Overview recipe",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: entity["id"].as_str().unwrap().into(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
        }
        fixture
            .editor
            .execute(
                "Overview zoom",
                Command::CanvasCamera {
                    kind: EntityKind::Achievement,
                    zoom: 0.43,
                    offset_x: -30.0,
                    offset_y: -30.0,
                },
                None,
            )
            .unwrap();
        let style = achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Shared overview icons".into(),
            recipe,
        };
        fixture.save_icon_style(style.clone()).unwrap();
        desktop.layer_palette.active_style = Some(style.id);
        desktop.layer_palette.achievement = desktop.selection.first().cloned();
        fixture.layout.position = Some([920.0, 130.0]);
        fixture.layout.size = [480.0, 600.0];
    }
    if sync {
        desktop.view = desktop::View::Sync;
    }
    if std::env::var_os("ACHIMELD_SCREENSHOT_LAYER_SIDEBAR").is_some() {
        let preferences = Preferences {
            layer_properties_in_sidebar: true,
            ..fixture.preferences().clone()
        };
        fixture
            .execute("Sidebar layout", Command::Preferences(preferences), None)
            .unwrap();
        fixture.layout.position = Some([220.0, 180.0]);
    }
    let preferences_general = std::env::var_os("ACHIMELD_SCREENSHOT_PREFERENCES_GENERAL").is_some();
    if colors || app_style || preferences_general {
        desktop.open_preferences(&fixture);
        desktop.pref_category = if preferences_general {
            0
        } else if app_style {
            2
        } else {
            1
        };
    }
    let source_picker = std::env::var_os("ACHIMELD_SCREENSHOT_SOURCE_PICKER").is_some();
    if source_picker {
        fixture = source_picker_regressions::native_fixture();
    }
    if let Some(path) = std::env::var_os("ACHIMELD_SCREENSHOT_PROJECT") {
        fixture.editor = Editor::new(
            achimeld_persistence::project::ProjectStore::new()
                .load(&std::path::PathBuf::from(path))
                .unwrap(),
        );
        desktop.selection.clear();
        desktop.inspector_tab = desktop::InspectorTab::General;
    }
    let other_icons = fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|e| e["icon"].is_null())
        .map(|e| {
            let mut e = e.clone();
            e["icon"] =
                json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF4672DE"}]});
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: e["id"].as_str().unwrap().into(),
                value: e,
            }
        })
        .collect();
    fixture
        .editor
        .execute("Synthetic artwork", Command::Batch(other_icons), None)
        .unwrap();
    let art_image = std::env::var_os("ACHIMELD_SCREENSHOT_ART_IMAGE").is_some();
    if art_image || std::env::var_os("ACHIMELD_SCREENSHOT_STYLE_ACHIEVEMENT").is_some() {
        let recipe = fixture.editor.document().entities(EntityKind::Achievement)[0]["icon"].clone();
        fixture
            .editor
            .execute(
                "Local style",
                if art_image {
                    Command::AddArtImageAt { position: [0, 8] }
                } else {
                    Command::AddStyleAchievementAt { position: [0, 8] }
                },
                None,
            )
            .unwrap();
        let mut local = fixture
            .editor
            .document()
            .entities(EntityKind::Achievement)
            .last()
            .unwrap()
            .clone();
        let id = local["id"].as_str().unwrap().to_owned();
        local["icon"] = recipe;
        if !art_image {
            local["unlockValue"] = json!(56.25);
        }
        fixture
            .editor
            .execute(
                "Preview recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: local,
                },
                None,
            )
            .unwrap();
        desktop.selection = vec![id];
        desktop.inspector_tab = if std::env::var_os("ACHIMELD_SCREENSHOT_STYLE_GENERAL").is_some() {
            desktop::InspectorTab::General
        } else {
            desktop::InspectorTab::Icon
        };
    }
    for entity in fixture.editor.document().entities(EntityKind::Achievement) {
        let id = entity["id"].as_str().unwrap().to_owned();
        let Ok(pair) = render(
            fixture.editor.document(),
            &id,
            Some((256, 256)),
            &mut NoAssets,
        ) else {
            continue;
        };
        fixture.previews.insert(
            id.clone(),
            Preview {
                width: 256,
                height: 256,
                source_size: [256, 256],
                rgba: pair.achieved.into_raw().into(),
                fingerprint: id,
            },
        );
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([1440.0, 920.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    let output = std::path::absolute(std::path::PathBuf::from(
        std::env::var_os("ACHIMELD_SCREENSHOT_TO")
            .expect("Set ACHIMELD_SCREENSHOT_TO to a the external build output directory PNG"),
    ))
    .unwrap();
    let boundary = std::path::absolute(crate::test_output::root()).unwrap();
    assert!(output.starts_with(boundary) && output.extension().is_some_and(|s| s == "png"));
    eframe::run_native(
        "Achimeld UI fixture",
        options,
        Box::new(move |cc| {
            configure_test_fonts(&cc.egui_ctx);
            if source_picker {
                let achievement =
                    fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
                        .as_str()
                        .unwrap();
                desktop.open_general_picker(
                    &cc.egui_ctx,
                    achievement,
                    crate::source_picker::SourceKind::Image,
                );
            }
            if floating {
                cc.egui_ctx.set_embed_viewports(true);
            }
            cc.egui_ctx
                .options_mut(|options| options.max_passes = std::num::NonZeroUsize::MIN);
            Ok(Box::new(Window {
                desktop,
                fixture,
                output,
                frames: 0,
                screenshot_viewport: egui::ViewportId::ROOT,
                capture_frame: if overview { 60 } else { 8 },
                context_menu: context_menu.is_some(),
            }))
        }),
    )
    .unwrap();
}

fn draw_editor(
    desktop: &mut Desktop,
    fixture: &mut Fixture,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 920.0),
            )),
            events,
            ..Default::default()
        },
        |ui| desktop.show(ui, fixture),
    );
    output.textures_delta.clear();
}
#[test]
fn icon_style_nodes_keep_their_own_style_identity_when_other_styles_change() {
    let mut fixture = Fixture::new();
    fixture
        .editor
        .execute(
            "Local",
            Command::AddStyleAchievementAt { position: [60, 0] },
            None,
        )
        .unwrap();
    let style = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Shared".into(),
        recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]}),
    };
    fixture.save_icon_style(style.clone()).unwrap();
    for mut entity in fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)
        .to_vec()
    {
        entity["icon"] = style.recipe.clone();
        fixture
            .editor
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: entity["id"].as_str().unwrap().into(),
                    value: entity,
                },
                None,
            )
            .unwrap();
    }
    let local = fixture
        .editor
        .document()
        .entities(EntityKind::Achievement)
        .last()
        .unwrap()
        .clone();
    let local_id = local["id"].as_str().unwrap().to_owned();
    let normal_id = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut desktop = Desktop::default();
    desktop.selection = vec![normal_id];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.layer_palette.active_style = Some(style.id.clone());
    desktop.layer_palette.pending = Some(layer_palette::LayerAction::Add("Shape"));
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    assert_eq!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &local_id),
        Some(&local)
    );
    let before_own_edit = fixture.editor.document().clone();
    desktop.selection = vec![local_id.clone()];
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.layer_palette.active_style = Some(style.id);
    desktop.layer_palette.pending = Some(layer_palette::LayerAction::Add("Shape"));
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    assert!(desktop.layer_palette.active_style.is_none());
    assert_eq!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &local_id)
            .unwrap()["icon"]["layers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for entity in before_own_edit
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| entity["id"] != local_id)
    {
        assert_eq!(
            fixture
                .editor
                .document()
                .entity(EntityKind::Achievement, entity["id"].as_str().unwrap()),
            Some(entity)
        );
    }
    let styles = fixture.editor.document().icon_styles();
    let linked = local["iconStyleId"].as_str().unwrap();
    assert_eq!(
        styles
            .iter()
            .find(|style| style.id == linked)
            .unwrap()
            .recipe,
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &local_id)
            .unwrap()["icon"]
    );
    for style in before_own_edit
        .icon_styles()
        .into_iter()
        .filter(|style| style.id != linked)
    {
        assert!(styles.contains(&style));
    }
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before_own_edit);
}

#[test]
fn layer_delete_keyboard_preserves_achievement_and_undo_restores_folder_mask() {
    let mut fixture = Fixture::new();
    let mut entity = fixture.editor.document().entities(EntityKind::Achievement)[0].clone();
    let achievement = entity["id"].as_str().unwrap().to_owned();
    let folder = new_id();
    let child = new_id();
    let mask = new_id();
    entity["icon"] = json!({"layers":[{"id":mask,"kind":"Solid","visible":false,"solidColorHex":"#FFFFFFFF"},{"id":child,"kind":"Solid","parentFolderId":folder,"solidColorHex":"#FFFFFFFF"},{"id":folder,"kind":"Folder","maskLayerId":mask}]});
    fixture
        .editor
        .execute(
            "Recipe",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: achievement.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![achievement.clone()];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.selected_layer = Some(folder);
    desktop.layer_palette.keyboard_active = true;
    draw_editor(
        &mut desktop,
        &mut fixture,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .is_some()
    );
    assert!(
        fixture
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .unwrap()["icon"]["layers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.editor.undo_label(), Some("Delete icon layer"));
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
}
#[test]
fn copied_folder_retains_internal_links_with_fresh_layer_and_effect_ids() {
    let folder = new_id();
    let child = new_id();
    let mask = new_id();
    let effect = new_id();
    let layers = vec![
        json!({"id":mask,"kind":"Solid","visible":false}),
        json!({"id":child,"kind":"Solid","parentFolderId":folder,"effects":[{"id":effect,"kind":"Outline"}]}),
        json!({"id":folder,"kind":"Folder","maskLayerId":mask}),
    ];
    let bundle = layer_palette::copy_bundle(&layers, &folder);
    let (fresh, root) = layer_palette::fresh_bundle(&bundle, &folder, None);
    assert_eq!(bundle.len(), 3);
    assert_eq!(
        layer_palette::stack(&layers, &Default::default()),
        vec![(folder.clone(), 0), (child.clone(), 1)]
    );
    let f = fresh.iter().find(|l| l["id"] == root).unwrap();
    let c = fresh
        .iter()
        .find(|l| l["kind"] == "Solid" && !l["parentFolderId"].is_null())
        .unwrap();
    assert_eq!(c["parentFolderId"], f["id"]);
    assert!(fresh.iter().any(|l| l["id"] == f["maskLayerId"]));
    assert_ne!(c["effects"][0]["id"], effect);
    assert!(
        fresh
            .iter()
            .all(|l| layers.iter().all(|old| old["id"] != l["id"]))
    );
    assert_eq!(
        achimeld_application::style_signature(&json!({"layers":layers})),
        achimeld_application::style_signature(&json!({"layers":fresh}))
    );
}
#[test]
fn layer_style_edit_updates_matching_icons_as_one_undo_and_leaves_other_icons_alone() {
    let mut fixture = Fixture::new();
    let first = fixture.editor.document().entities(EntityKind::Achievement)[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let recipe = json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]});
    let mut commands = Vec::new();
    for e in fixture.editor.document().entities(EntityKind::Achievement) {
        let mut e = e.clone();
        e["icon"] = recipe.clone();
        commands.push(Command::ReplaceEntity {
            kind: EntityKind::Achievement,
            id: e["id"].as_str().unwrap().into(),
            value: e,
        });
    }
    fixture
        .editor
        .execute("Recipes", Command::Batch(commands), None)
        .unwrap();
    let style = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Red".into(),
        recipe,
    };
    fixture.save_icon_style(style.clone()).unwrap();
    fixture.styles.push(style.clone());
    let before = fixture.editor.document().clone();
    let mut desktop = Desktop::default();
    desktop.selection = vec![first];
    desktop.inspector_tab = desktop::InspectorTab::Icon;
    let ctx = egui::Context::default();
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    desktop.layer_palette.active_style = Some(style.id.clone());
    desktop.layer_palette.pending = Some(layer_palette::LayerAction::Add("Shape"));
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    assert!(
        fixture
            .editor
            .document()
            .entities(EntityKind::Achievement)
            .iter()
            .all(|e| e["icon"]["layers"].as_array().unwrap().len() == 2)
    );
    assert_eq!(fixture.editor.undo_label(), Some("Edit icon style"));
    assert_eq!(
        fixture.icon_styles()[0].recipe["layers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fixture.global_icon_styles(), std::slice::from_ref(&style));
    let after = fixture.editor.document().clone();
    fixture.undo();
    assert_eq!(fixture.editor.document(), &before);
    assert_eq!(fixture.icon_styles(), vec![style.clone()]);
    fixture.redo();
    assert_eq!(fixture.editor.document(), &after);
    assert_eq!(fixture.global_icon_styles(), &[style]);
}
#[test]
fn global_styles_require_import_and_project_copies_are_independent() {
    let mut fixture = Fixture::new();
    let global = achimeld_application::IconStyle {
        preview_png: None,
        id: new_id(),
        name: "Global template".into(),
        recipe: json!({"layers":[{"id":new_id(),"kind":"Solid"}]}),
    };
    fixture.styles.push(global.clone());
    assert!(fixture.icon_styles().is_empty());
    fixture.editor.mark_saved();
    fixture.import_global_icon_style(&global.id).unwrap();
    assert!(fixture.editor.is_dirty());
    let local = fixture.icon_styles()[0].clone();
    assert_ne!(local.id, global.id);
    assert_eq!(local.recipe, global.recipe);
    let mut edited = local.clone();
    edited.name = "Project copy".into();
    fixture.save_icon_style(edited).unwrap();
    assert_eq!(fixture.global_icon_styles(), std::slice::from_ref(&global));
    fixture.delete_icon_style(&local.id).unwrap();
    assert!(fixture.icon_styles().is_empty());
    fixture.undo();
    assert_eq!(fixture.icon_styles()[0].name, "Project copy");
    // Switching documents cannot expose either the old project or global styles.
    fixture.editor = Editor::default();
    assert!(fixture.icon_styles().is_empty());
    assert_eq!(fixture.global_icon_styles(), &[global]);
}
#[test]
fn atlas_picker_respects_offsets_gaps_and_column_major_order() {
    let settings = json!({"tileWidth":16,"tileHeight":12,"leftOffset":3,"topOffset":5,"horizontalSpacing":2,"verticalSpacing":4,"traversal":"ColumnMajor"});
    let size = [55, 45];
    assert_eq!(source_picker::tile_at(size, &settings, [4.0, 6.0]), Some(0));
    assert_eq!(
        source_picker::tile_at(size, &settings, [22.0, 6.0]),
        Some(2)
    );
    assert_eq!(
        source_picker::tile_at(size, &settings, [4.0, 22.0]),
        Some(1)
    );
    assert_eq!(source_picker::tile_at(size, &settings, [19.0, 6.0]), None);
    assert_eq!(source_picker::tile_at(size, &settings, [1.0, 6.0]), None);
}

#[test]
fn inactive_main_window_does_not_split_a_floating_property_gesture() {
    let mut fixture = Fixture::new();
    let initial = fixture.editor.document().clone();
    let mut entity = initial.entities(EntityKind::Achievement)[0].clone();
    let id = entity["id"].as_str().unwrap().to_owned();
    let mut desktop = Desktop::default();
    desktop.selection = vec![id.clone()];
    // Render only the inactive parent. A native child owns its separate pointer
    // input; the embedded fallback would instead render an idle child here.
    desktop.layer_palette.open = false;
    fixture.layout.floating = true;
    let ctx = egui::Context::default();
    // Initialize the project/camera context before the gesture starts.
    draw_editor(&mut desktop, &mut fixture, &ctx, vec![]);
    entity["internalName"] = json!("In progress");
    fixture
        .execute(
            "Edit properties",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity.clone(),
            },
            Some("floating-property"),
        )
        .unwrap();
    let mut output = ctx.run_ui(
        egui::RawInput {
            focused: false,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 920.0),
            )),
            ..Default::default()
        },
        |ui| desktop.show(ui, &mut fixture),
    );
    output.textures_delta.clear();
    entity["internalName"] = json!("Finished");
    fixture
        .execute(
            "Edit properties",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id,
                value: entity,
            },
            Some("floating-property"),
        )
        .unwrap();
    fixture.undo();
    assert_eq!(fixture.editor.document(), &initial);
}

#[path = "general_image_regressions.rs"]
mod general_image_regressions;
#[path = "gradient_regressions.rs"]
mod gradient_regressions;
#[path = "spatial_effects_regressions.rs"]
mod spatial_effects_regressions;
