use crate::buttons::UiButtonExt as _;
use crate::{Desktop, buttons, icons, localization, theme};
use achimeld_application::{Command, EditorPort, EntityKind};
use egui::{Pos2, Rect, Stroke, Vec2};
use serde_json::{Value, json};
const FIELD_HEIGHT: f32 = 38.0;
const NAME_FIELD_HEIGHT: f32 = 28.0;
pub(crate) fn full_text(
    ui: &mut egui::Ui,
    text: &mut String,
    height: f32,
    multiline: bool,
) -> egui::Response {
    let edit = if multiline {
        egui::TextEdit::multiline(text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(text)
    };
    let vertical_margin =
        ((height - ui.text_style_height(&egui::TextStyle::Body)) / 2.0).clamp(0.0, 8.0);
    ui.add_sized(
        [ui.available_width(), height],
        edit.background_color(ui.visuals().widgets.inactive.bg_fill)
            .margin(Vec2::new(16.0, vertical_margin))
            .desired_width(f32::INFINITY),
    )
}
/// Original 56 x 56 staggered-arrow wallpaper, clipped to the name header.
pub(crate) fn stat_wallpaper(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let cue = theme::mix(
        ui.visuals().warn_fg_color,
        ui.visuals().error_fg_color,
        0.25,
    );
    let ink = theme::mix(ui.visuals().panel_fill, cue, 0.10);
    for y in 0..(rect.height() / 56.0).ceil() as i32 + 1 {
        for x in 0..(rect.width() / 56.0).ceil() as i32 + 1 {
            for (ox, oy) in [(0.0, 0.0), (28.0, 28.0)] {
                let origin = rect.min + Vec2::new(x as f32 * 56.0 + ox, y as f32 * 56.0 + oy);
                painter.add(egui::Shape::convex_polygon(
                    [(14.0, 3.1), (22.0, 11.1), (6.0, 11.1)]
                        .iter()
                        .map(|&(x, y)| origin + Vec2::new(x, y))
                        .collect(),
                    ink,
                    Stroke::NONE,
                ));
                painter.rect_filled(
                    Rect::from_min_max(
                        origin + Vec2::new(10.8, 11.1),
                        origin + Vec2::new(17.2, 23.9),
                    ),
                    0.0,
                    ink,
                );
            }
        }
    }
}
impl Desktop {
    pub(crate) fn style_achievement(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        entity: &mut Value,
    ) {
        let art_image = achimeld_application::art_images::is_art_image(entity);
        ui.weak(localization::text(
            ui.ctx(),
            if art_image {
                "Art image · reusable composition"
            } else {
                "Icon style node · excluded from Steam"
            },
        ));
        ui.add_space(8.0);
        let id = entity["id"].as_str().unwrap_or("").to_owned();
        let texture = self.texture(ui.ctx(), port, &id);
        if icons::thumbnail(
            ui,
            texture,
            if art_image {
                "Edit image layers…"
            } else {
                "Edit this icon style node"
            },
            140.0,
        )
        .clicked()
        {
            self.recover_layers();
        }
        buttons::bar(ui, 20.0, |ui| {
            if ui
                .add(
                    buttons::Button::new(if art_image {
                        "Edit image layers…"
                    } else {
                        "Edit style…"
                    })
                    .bar(),
                )
                .clicked()
            {
                self.recover_layers();
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("style-achievement-inputs")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !art_image {
                    crate::inspector::field(ui, "Unlock value", 100.0, |ui| {
                        let mut value = entity["unlockValue"].as_f64().unwrap_or(10.0);
                        if ui
                            .push_id(("style-node-unlock", &id), |ui| {
                                ui.add(
                                    egui::DragValue::new(&mut value)
                                        .speed(0.1)
                                        .max_decimals(8)
                                        .range(f64::from(i32::MIN)..=f64::from(i32::MAX)),
                                )
                            })
                            .inner
                            .changed()
                        {
                            entity["unlockValue"] = json!(value);
                        }
                    });
                    ui.add_space(8.0);
                    self.style_node_link(ui, port, entity);
                    ui.add_space(8.0);
                }
                crate::fields::row(ui, if art_image { "Name" } else { "InDisp" }, |ui| {
                    self.english_field(
                        ui,
                        port,
                        entity,
                        "displayNameLocalizationId",
                        NAME_FIELD_HEIGHT,
                        false,
                    )
                });
                ui.add_space(8.0);
                if !art_image {
                    crate::fields::row(ui, "InDesc", |ui| {
                        self.english_field(
                            ui,
                            port,
                            entity,
                            "descriptionLocalizationId",
                            54.0,
                            true,
                        )
                    });
                }
                ui.add_space(8.0);
                if art_image {
                    let mut icon = crate::layer_palette::effective_icon(entity);
                    let before = icon.clone();
                    self.icon_style_banner(ui, port, &mut icon);
                    if icon != before {
                        entity["icon"] = icon;
                    }
                }
                ui.add_space(8.0);
                self.general_explorer(ui, port, entity, None);
            });
    }
    fn style_node_link(&mut self, ui: &mut egui::Ui, port: &dyn EditorPort, entity: &mut Value) {
        let styles = port.icon_styles();
        let mut selected = entity["iconStyleId"].as_str().map(str::to_owned);
        let before = selected.clone();
        let caption = styles
            .iter()
            .find(|style| Some(&style.id) == selected.as_ref())
            .map(|style| style.name.clone())
            .unwrap_or_else(|| localization::text(ui.ctx(), "No style"));
        crate::fields::row(ui, "Linked style", |ui| {
            crate::dropdowns::Dropdown::from_id_salt("style-node-link")
                .width(ui.available_width())
                .selected_text(caption)
                .show_ui(ui, |ui| {
                    ui.app_selectable_value(
                        &mut selected,
                        None,
                        localization::text(ui.ctx(), "No style"),
                    );
                    for style in &styles {
                        ui.app_selectable_value(&mut selected, Some(style.id.clone()), &style.name);
                    }
                });
        });
        if selected != before {
            if let Some(style) = styles
                .iter()
                .find(|style| Some(&style.id) == selected.as_ref())
            {
                entity["iconStyleId"] = json!(style.id);
            } else {
                entity
                    .as_object_mut()
                    .expect("Icon style node")
                    .remove("iconStyleId");
            }
            self.selected_layer = None;
            self.layer_palette.edit_label = Some("Change linked icon style");
        }
    }
    fn english_field(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        entity: &Value,
        key: &str,
        height: f32,
        multiline: bool,
    ) -> String {
        let row = entity[key].as_str().unwrap_or("").to_owned();
        let draft_key = (row.clone(), "English".into());
        let mut text = self
            .localization_drafts
            .get(&draft_key)
            .map(|d| d.0.clone())
            .unwrap_or_else(|| {
                port.editor()
                    .document()
                    .catalog()
                    .text(&row, "English")
                    .to_owned()
            });
        let response = ui
            .push_id(key, |ui| full_text(ui, &mut text, height, multiline))
            .inner;
        if response.changed() {
            match port.execute(
                "Edit localization",
                Command::SetLocalization {
                    id: row.clone(),
                    language: "English".into(),
                    text: text.clone(),
                },
                Some(&format!("{row}-English")),
            ) {
                Ok(()) => {
                    self.localization_drafts.remove(&draft_key);
                }
                Err(error) => {
                    self.localization_drafts
                        .insert(draft_key.clone(), (text.clone(), error));
                }
            }
        }
        if let Some((_, error)) = self.localization_drafts.get(&draft_key) {
            ui.add(
                egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color))
                    .wrap(),
            );
        }
        if response.lost_focus() {
            port.finish_group();
        }
        text
    }
    pub(crate) fn achievement_general(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        entity: &mut Value,
    ) {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Panel::bottom("achievement-steam-footer")
            .exact_size(18.0)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let remote =
                    port.editor().document().project()["lastRemoteSnapshot"]["achievements"]
                        .as_array()
                        .and_then(|a| {
                            a.iter().find(|a| {
                                a["apiName"] == entity["apiName"]
                                    || a["apiName"]["value"] == entity["apiName"]
                            })
                        });
                let steam = remote
                    .and_then(|r| r["steamId"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| localization::text(ui.ctx(), "InNoId"));
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {steam}",
                        localization::text(ui.ctx(), "InStId")
                    ))
                    .small()
                    .weak(),
                );
            });
        let header = Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 74.0));
        let mut tied = !entity["progressStatId"].is_null()
            || self.manual_stat_link.as_deref() == entity["id"].as_str();
        if tied {
            stat_wallpaper(ui, header);
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let id = entity["id"].as_str().unwrap_or("");
            let texture = self.texture(ui.ctx(), port, id);
            if icons::thumbnail(ui, texture, "Icon", 64.0).clicked() {
                self.open_layers();
            }
            ui.vertical(|ui| {
                let template = crate::fields::row(ui, "InDisp", |ui| {
                    self.english_field(
                        ui,
                        port,
                        entity,
                        "displayNameLocalizationId",
                        NAME_FIELD_HEIGHT,
                        false,
                    )
                });
                if template.contains("{0}") {
                    ui.add_space(4.0);
                    let name =
                        achimeld_application::document::resolve_achievement_text(entity, &template);
                    ui.add(
                        egui::Label::new(egui::RichText::new(name).family(egui::FontFamily::Name(
                            crate::typography::BOLD_FAMILY.into(),
                        )))
                        .wrap(),
                    )
                    .on_hover_text(localization::text(ui.ctx(), "Name exported to Steam"));
                }
            });
        });
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        egui::ScrollArea::vertical()
            .id_salt("achievement-general-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut status = entity["steamSyncStatus"].as_str().unwrap_or("Active").to_owned();
                crate::fields::row(ui, "Steam sync", |ui| {
                    crate::dropdowns::Dropdown::from_id_salt("achievement-steam-sync")
                        .width(ui.available_width())
                        .selected_text(localization::text(ui.ctx(), &status))
                        .show_ui(ui, |ui| {
                            for value in ["Active", "Unsynced", "Retired"] {
                                ui.app_selectable_value(&mut status, value.into(), localization::text(ui.ctx(), value));
                            }
                        }).response.on_hover_text(localization::text(ui.ctx(), "Unsynced entries are ready to synchronize. Active entries participate in synchronization. Retired entries stay in this project and are excluded from synchronization."));
                });
                if status != entity["steamSyncStatus"].as_str().unwrap_or("Active") {
                    entity["steamSyncStatus"] = json!(status);
                }
                ui.add_space(8.0);
                crate::fields::row(ui, "InDesc", |ui| {
                    self.english_field(ui, port, entity, "descriptionLocalizationId", 54.0, true)
                });
                ui.add_space(8.0);
                let mut api = entity["apiName"].as_str().unwrap_or("").to_owned();
                if crate::fields::row(ui, "InApiN", |ui| {
                    full_text(ui, &mut api, FIELD_HEIGHT, false)
                })
                .changed()
                {
                    entity["apiName"] = json!(api);
                }
                crate::steam::rename_panel(ui, port, entity, false);
                ui.add_space(8.0);
                let mut icon = crate::layer_palette::effective_icon(entity);
                let before = icon.clone();
                self.icon_style_banner(ui, port, &mut icon);
                if icon != before {
                    entity["icon"] = icon;
                }
                ui.add_space(6.0);
                self.general_explorer(ui, port, entity, None);
                ui.add_space(8.0);
                let stats: Vec<_> = port
                    .editor()
                    .document()
                    .entities(EntityKind::Stat)
                    .iter()
                    .map(|s| {
                        (
                            s["id"].as_str().unwrap_or("").to_owned(),
                            s["apiName"].as_str().unwrap_or("").to_owned(),
                            s["type"].as_str().unwrap_or("Int").to_owned(),
                        )
                    })
                    .collect();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let width = ui.available_width();
                    let small = ((width - 24.0) / 4.0).max(40.0);
                    let large = (width - small * 2.0 - 16.0).max(56.0);
                    let old = entity["setBy"].as_str().unwrap_or("GameClient").to_owned();
                    dropdown(ui, "InSetB", "", Some(icons::Icon::Monitor), small, |ui| {
                        for (value, key) in [
                            ("GameClient", "GnClient"),
                            ("GameServer", "GnServer"),
                            ("OfficialGameServer", "GnOfficial"),
                        ] {
                            if ui
                                .app_selectable_label(
                                    old == value,
                                    localization::text(ui.ctx(), key),
                                )
                                .clicked()
                            {
                                entity["setBy"] = json!(value);
                                ui.close();
                            }
                        }
                    });
                    dropdown(
                        ui,
                        "InATyp",
                        "",
                        Some(icons::Icon::Star(false)),
                        small,
                        |ui| {
                            for (value, key) in [(false, "InManu"), (true, "InTied")] {
                                if ui
                                    .app_selectable_label(
                                        tied == value,
                                        localization::text(ui.ctx(), key),
                                    )
                                    .clicked()
                                {
                                    tied = value;
                                    self.manual_stat_link = if value {
                                        Some(entity["id"].as_str().unwrap_or("").into())
                                    } else {
                                        None
                                    };
                                    entity["progressStatId"] = if value {
                                        stats.first().map_or(Value::Null, |s| json!(s.0))
                                    } else {
                                        Value::Null
                                    };
                                    ui.close();
                                }
                            }
                        },
                    );
                    if tied {
                        let selected = entity["progressStatId"].as_str().unwrap_or("").to_owned();
                        let title = stats.iter().find(|s| s.0 == selected).map_or_else(
                            || localization::text(ui.ctx(), "InNone"),
                            |s| s.1.clone(),
                        );
                        dropdown(ui, "InProg", &title, None, large, |ui| {
                            for (id, name, _) in &stats {
                                if ui.app_selectable_label(id == &selected, name).clicked() {
                                    entity["progressStatId"] = json!(id);
                                    self.manual_stat_link = None;
                                    ui.close();
                                }
                            }
                        });
                        // Choosing a stat derives the numeric kind, as in the original.
                        if let Some(stat) = stats.iter().find(|s| s.0 == selected) {
                            entity["unlockValueKind"] =
                                json!(if stat.2 == "Int" { "Integer" } else { "Float" });
                        }
                    } else {
                        let selected = entity["unlockValueKind"]
                            .as_str()
                            .unwrap_or("Integer")
                            .to_owned();
                        dropdown(
                            ui,
                            "InUTyp",
                            &localization::text(
                                ui.ctx(),
                                if selected == "Integer" {
                                    "InInte"
                                } else {
                                    "InFloa"
                                },
                            ),
                            None,
                            large,
                            |ui| {
                                for (kind, key) in [("Integer", "InInte"), ("Float", "InFloa")] {
                                    if ui
                                        .app_selectable_label(
                                            selected == kind,
                                            localization::text(ui.ctx(), key),
                                        )
                                        .clicked()
                                    {
                                        entity["unlockValueKind"] = json!(kind);
                                        ui.close();
                                    }
                                }
                            },
                        );
                    }
                });
                ui.add_space(6.0);
                if tied {
                    ui.label(
                        egui::RichText::new(localization::text(ui.ctx(), "InUFol"))
                            .small()
                            .weak(),
                    );
                    ui.add_space(8.0);
                }
                let integer = entity["unlockValueKind"] != "Float";
                crate::fields::row(ui, "InUnlk", |ui| {
                    full_number(ui, entity, "unlockValue", integer)
                });
                ui.add_space(8.0);
                crate::fields::row(ui, "InOrdr", |ui| {
                    full_number(ui, entity, "displayOrder", true)
                });
                ui.add_space(8.0);
                if entity["hidden"] == true {
                    let mut hidden = true;
                    if crate::fields::checkbox(ui, "InHidd", &mut hidden).changed() {
                        entity["hidden"] = json!(hidden);
                    }
                }
            });
    }
}
fn full_number(ui: &mut egui::Ui, entity: &mut Value, key: &str, integer: bool) {
    let mut value = entity[key]
        .as_f64()
        .or_else(|| entity[key].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.0);
    let previous = value;
    let range = if key == "displayOrder" {
        0.0..=20000.0
    } else {
        f64::from(i32::MIN)..=f64::from(i32::MAX)
    };
    ui.add_sized(
        [ui.available_width(), FIELD_HEIGHT],
        egui::DragValue::new(&mut value)
            .speed(if integer { 1.0 } else { 0.1 })
            .range(range)
            .min_decimals(0)
            .max_decimals(if integer { 0 } else { 8 }),
    );
    if value != previous {
        entity[key] = if integer {
            json!(value.round() as i64)
        } else {
            json!(value)
        };
    }
}
fn dropdown(
    ui: &mut egui::Ui,
    key: &str,
    text: &str,
    icon: Option<icons::Icon>,
    width: f32,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let title = localization::text(ui.ctx(), key);
    let response = ui
        .push_id(key, |ui| {
            buttons::menu(
                ui,
                buttons::Button::new("").min_size(Vec2::new(width, FIELD_HEIGHT)),
                |ui| {
                    ui.set_min_width(180.0);
                    ui.label(egui::RichText::new(&title).weak());
                    ui.separator();
                    content(ui);
                },
            )
            .response
        })
        .inner;
    let (_, ink) = buttons::colors(ui, &response, buttons::ButtonStyle::Quiet);
    let rect = response.rect;
    if let Some(icon) = icon {
        icons::paint(
            ui.painter(),
            Rect::from_center_size(rect.center() - Vec2::new(6.0, 0.0), Vec2::splat(18.0)),
            icon,
            ink,
        );
    } else {
        let mut font = egui::TextStyle::Button.resolve(&ui.ctx().global_style());
        font.size = (font.size - 1.0).max(1.0);
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            font,
            ink,
            (rect.width() - 32.0).max(1.0),
        );
        job.wrap.max_rows = 1;
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(
            Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(
            Pos2::new(rect.right() - 12.0, rect.center().y),
            Vec2::splat(8.0),
        ),
        icons::Icon::Chevron(false),
        ink,
    );
    response.on_hover_text(title)
}
