use crate::Desktop;
use crate::buttons::UiButtonExt as _;
use crate::desktop::{EntityDraft, InspectorTab};
use achimeld_application::{Command, EditorPort, EntityKind};
use serde_json::{Value, json};
impl Desktop {
    pub(crate) fn inspector(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().item_spacing.y = spacing;
        let Some(id) = self.selection.first().cloned() else {
            ui.add_space(spacing);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(10, 0))
                .show(ui, |ui| {
                    ui.label(crate::localization::text(
                        ui.ctx(),
                        "Select a card to edit its properties.",
                    ));
                });
            return;
        };
        let Some(live) = port.editor().document().entity(self.kind, &id).cloned() else {
            self.selection.clear();
            return;
        };
        let mut entity = self
            .entity_drafts
            .get(&id)
            .map(|d| d.value.clone())
            .unwrap_or_else(|| live.clone());
        if let Some(draft) = self.entity_drafts.get(&id) {
            ui.colored_label(
                crate::theme::color(port.preferences(), "Error"),
                &draft.error,
            );
            if ui.app_button("Revert invalid edit").clicked() {
                self.entity_drafts.remove(&id);
                entity = live.clone();
            }
        }
        let icons = self.inspector_icons.get_or_insert_with(|| {
            [
                (
                    "inspector-general",
                    include_bytes!("../assets/toolbar/inspector-general.rgba").as_slice(),
                ),
                (
                    "inspector-localization",
                    include_bytes!("../assets/toolbar/inspector-localization.rgba").as_slice(),
                ),
                (
                    "inspector-icon",
                    include_bytes!("../assets/toolbar/inspector-icon.rgba").as_slice(),
                ),
            ]
            .map(|(name, bytes)| {
                ui.ctx().load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                )
            })
        });
        let images = icons
            .each_ref()
            .map(|icon| (icon.id(), egui::Vec2::splat(20.0)));
        let art_image = achimeld_application::art_images::is_art_image(&live);
        let local_style = achimeld_application::style_achievements::is_local_icon(&live);
        if art_image || (local_style && self.inspector_tab == InspectorTab::Localization) {
            self.inspector_tab = InspectorTab::Icon;
        }
        if local_style {
            self.layer_palette.active_style = None;
        }
        let tabs: Vec<_> = [
            (InspectorTab::General, "General"),
            (InspectorTab::Localization, "Localization"),
            (InspectorTab::Icon, "Icon"),
        ]
        .into_iter()
        .zip(images)
        .filter(|((tab, _), _)| self.kind != EntityKind::Stat || *tab != InspectorTab::Icon)
        .filter(|((tab, _), _)| !local_style || *tab != InspectorTab::Localization)
        .filter(|((tab, _), _)| !art_image || *tab == InspectorTab::Icon)
        .map(|((tab, name), image)| {
            (
                tab,
                crate::localization::text(
                    ui.ctx(),
                    if art_image {
                        "Image"
                    } else if local_style && tab == InspectorTab::Icon {
                        "Style"
                    } else {
                        name
                    },
                ),
                image,
            )
        })
        .collect();
        let needed_width: f32 = tabs
            .iter()
            .map(|(_, name, _)| {
                ui.painter()
                    .layout_no_wrap(
                        name.clone(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
                    + 25.0
                    + 2.0 * ui.spacing().button_padding.x
            })
            .sum();
        let full_labels = needed_width <= ui.available_width();
        let short_labels: Vec<_> = tabs
            .iter()
            .map(|(tab, _, _)| match tab {
                InspectorTab::General => "Gen",
                InspectorTab::Localization => "Loc",
                InspectorTab::Icon if art_image => "Img",
                InspectorTab::Icon if local_style => "Sty",
                InspectorTab::Icon => "Ico",
            })
            .collect();
        let short_width: f32 = tabs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                ui.painter()
                    .layout_no_wrap(
                        short_labels[index].into(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
                    + 25.0
                    + 2.0 * ui.spacing().button_padding.x
            })
            .sum();
        let show_icons = full_labels || short_width <= ui.available_width();
        ui.spacing_mut().item_spacing.y = 0.0;
        crate::buttons::bar(ui, 20.0, |ui| {
            if !show_icons {
                let text_width =
                    short_width - tabs.len() as f32 * (25.0 + 2.0 * ui.spacing().button_padding.x);
                ui.spacing_mut().button_padding.x = ui.spacing().button_padding.x.min(
                    ((ui.available_width() - text_width) / (2.0 * tabs.len() as f32)).max(0.0),
                );
            }
            for (index, (tab, name, image)) in tabs.into_iter().enumerate() {
                let label = if full_labels {
                    &name
                } else {
                    short_labels[index]
                };
                let response = if show_icons {
                    crate::desktop::toolbar_tab(ui, &mut self.inspector_tab, tab, label, image)
                } else {
                    let response = ui.add(
                        crate::buttons::Button::selectable(self.inspector_tab == tab, label).bar(),
                    );
                    if response.clicked() {
                        self.inspector_tab = tab;
                    }
                    response
                }
                .on_hover_text(&name);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        self.inspector_tab == tab,
                        &name,
                    )
                });
                if response.clicked() && tab == InspectorTab::Icon {
                    self.open_layers();
                }
            }
        });
        ui.add(egui::Separator::default().spacing(0.0));
        ui.spacing_mut().item_spacing.y = spacing;
        ui.add_space(spacing);
        self.layer_palette.properties_rect = None;
        if self.inspector_tab == InspectorTab::Icon
            && port.preferences().layer_properties_in_sidebar
        {
            self.layer_palette.open = true;
            if art_image {
                let before = entity.clone();
                egui::CollapsingHeader::new(crate::localization::text(ui.ctx(), "Preview inputs"))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .show(ui, |ui| {
                                self.style_achievement(ui, port, &mut entity);
                            });
                    });
                self.commit_entity(ui, port, &id, live, before, entity);
            } else {
                crate::buttons::bar(ui, 20.0, |ui| {
                    if ui
                        .add(crate::buttons::Button::new("Show layers panel").bar())
                        .clicked()
                    {
                        self.recover_layers();
                    }
                });
            }
            self.sidebar_layer_properties(ui, port);
            return;
        }
        let before = entity.clone();
        let tab = self.inspector_tab;
        if tab == InspectorTab::Icon {
            self.layer_palette.open = true;
        }
        let general =
            local_style || (self.kind == EntityKind::Achievement && tab == InspectorTab::General);
        let mut contents = |ui: &mut egui::Ui| {
            if local_style && (tab == InspectorTab::General || art_image) {
                egui::Frame::new().inner_margin(10).show(ui, |ui| {
                    self.style_achievement(ui, port, &mut entity);
                });
                return;
            }
            match tab {
                InspectorTab::General => {
                    if self.kind == EntityKind::Achievement {
                        egui::Frame::new()
                            .inner_margin(10)
                            .show(ui, |ui| self.achievement_general(ui, port, &mut entity));
                    } else {
                        text(ui, "Internal name", &mut entity, "internalName");
                        text(ui, "Steam API name", &mut entity, "apiName");
                        choice(
                            ui,
                            "Set by",
                            &mut entity,
                            "setBy",
                            &["GameClient", "GameServer", "OfficialGameServer"],
                        );
                        integer(ui, "Display order", &mut entity, "displayOrder", 0..=20000);
                        choice(
                            ui,
                            "Stat type",
                            &mut entity,
                            "type",
                            &["Int", "Float", "AverageRate"],
                        );
                        boolean(ui, "Increment only", &mut entity, "incrementOnly");
                        boolean(ui, "Aggregated", &mut entity, "aggregated");
                        for (label, key) in [
                            ("Minimum", "minimum"),
                            ("Maximum", "maximum"),
                            ("Default", "defaultValue"),
                            ("Maximum change", "maximumChange"),
                            ("Average-rate window (seconds)", "averageRateWindowSeconds"),
                        ] {
                            decimal(ui, &id, label, &mut entity, key);
                        }
                    }
                }
                InspectorTab::Localization => {
                    if self.kind == EntityKind::Achievement {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .app_button(crate::localization::text(ui.ctx(), "Export CSV…"))
                                .on_hover_text("Export names and descriptions for all achievements")
                                .clicked()
                            {
                                self.request(
                                    achimeld_application::UiRequest::ExportAchievementTranslations,
                                );
                            }
                            if ui
                                .app_button(crate::localization::text(ui.ctx(), "Import CSV…"))
                                .on_hover_text(
                                    "Import translated achievement names and descriptions",
                                )
                                .clicked()
                            {
                                self.request(
                                    achimeld_application::UiRequest::ImportAchievementTranslations,
                                );
                            }
                        });
                        ui.label(crate::localization::text(
                            ui.ctx(),
                            "Use {0} in names or descriptions to insert the unlock value.",
                        ));
                    }
                    for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
                        if self.kind == EntityKind::Stat && key == "descriptionLocalizationId" {
                            continue;
                        }
                        let row_id = entity[key].as_str().unwrap_or("").to_owned();
                        ui.heading(if key == "displayNameLocalizationId" {
                            "Display name"
                        } else {
                            "Description"
                        });
                        for language in [
                            "English",
                            "Polish",
                            "ChineseSimplified",
                            "ChineseTraditional",
                            "Japanese",
                            "Korean",
                            "French",
                        ] {
                            let draft_key = (row_id.clone(), language.to_owned());
                            let mut content = self
                                .localization_drafts
                                .get(&draft_key)
                                .map(|d| d.0.clone())
                                .unwrap_or_else(|| {
                                    port.editor()
                                        .document()
                                        .catalog()
                                        .rows
                                        .get(&row_id)
                                        .and_then(|r| r.get(language))
                                        .cloned()
                                        .unwrap_or_default()
                                });
                            let response = crate::fields::row(ui, language, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut content)
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY),
                                )
                            });
                            if self.kind == EntityKind::Achievement && content.contains("{0}") {
                                ui.label(format!(
                                    "{}: {}",
                                    crate::localization::text(ui.ctx(), "Preview"),
                                    achimeld_application::document::resolve_achievement_text(
                                        &entity, &content,
                                    ),
                                ));
                            }
                            if response.changed() {
                                match port.execute(
                                    "Edit localization",
                                    Command::SetLocalization {
                                        id: row_id.clone(),
                                        language: language.into(),
                                        text: content.clone(),
                                    },
                                    Some(&format!("{row_id}-{language}")),
                                ) {
                                    Ok(()) => {
                                        self.localization_drafts.remove(&draft_key);
                                    }
                                    Err(error) => {
                                        self.localization_drafts
                                            .insert(draft_key.clone(), (content, error));
                                    }
                                }
                            }
                            if let Some((_, error)) = self.localization_drafts.get(&draft_key) {
                                ui.colored_label(
                                    crate::theme::color(port.preferences(), "Error"),
                                    error,
                                );
                                if ui.small_button("Revert invalid text").clicked() {
                                    self.localization_drafts.remove(&draft_key);
                                }
                            }
                            if response.lost_focus() {
                                port.finish_group();
                            }
                        }
                        ui.separator();
                    }
                }
                InspectorTab::Icon => {
                    if let Some(texture) = self.texture(ui.ctx(), port, &id)
                        && crate::icons::thumbnail(ui, Some(texture), "Icon", 140.0).clicked()
                    {
                        self.open_layers();
                    }
                    if ui.app_button("Show layers panel").clicked() {
                        self.recover_layers();
                    }
                }
            }
        };
        if general {
            contents(ui);
        } else {
            egui::ScrollArea::vertical()
                .id_salt("inspector-properties-scroll")
                .show(ui, contents);
        }
        self.commit_entity(ui, port, &id, live, before, entity);
    }
    pub(crate) fn commit_entity(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        id: &str,
        live: Value,
        before: Value,
        entity: Value,
    ) {
        let edit_label = self.layer_palette.edit_label.take();
        let edit_group = format!("inspector-{id}");
        if entity != before {
            if edit_label.is_some() {
                port.finish_group();
            }
            let focused = ui.ctx().memory(|memory| memory.focused());
            if focused != self.focused_control {
                port.finish_group();
                self.focused_control = focused;
            }
            let conflict = self.entity_drafts.get(id).is_some_and(|d| d.base != live);
            let result = if conflict {
                Err("This entity changed while the edit was invalid. Revert the draft before editing again.".into())
            } else {
                port.execute(
                    edit_label.unwrap_or("Edit properties"),
                    Command::ReplaceEntity {
                        kind: self.kind,
                        id: id.to_owned(),
                        value: entity.clone(),
                    },
                    if edit_label.is_some() {
                        None
                    } else {
                        Some(&edit_group)
                    },
                )
            };
            match result {
                Ok(()) => {
                    self.entity_drafts.remove(id);
                }
                Err(error) => {
                    let base = self
                        .entity_drafts
                        .get(id)
                        .map(|d| d.base.clone())
                        .unwrap_or(live);
                    self.entity_drafts.insert(
                        id.to_owned(),
                        EntityDraft {
                            base,
                            value: entity,
                            error,
                        },
                    );
                }
            }
        }
        if let Some(order) = self.reordered_layers.take() {
            port.finish_group();
            self.apply(
                port,
                "Reorder icon layers",
                Command::ReorderLayers {
                    achievement_id: id.to_owned(),
                    order,
                },
                None,
            );
        }
        if !self.camera_editing
            && self.camera_drag.is_none()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.focused && !i.pointer.any_down())
        {
            port.finish_group();
        }
    }
}
pub(crate) fn field<R>(
    ui: &mut egui::Ui,
    label: &str,
    _control_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    crate::fields::row(ui, label, content)
}
pub(crate) fn text(ui: &mut egui::Ui, label: &str, value: &mut Value, key: &str) {
    let mut content = value[key].as_str().unwrap_or("").to_owned();
    if field(ui, label, 150.0, |ui| {
        ui.add(egui::TextEdit::singleline(&mut content).desired_width(ui.available_width()))
            .changed()
    }) {
        value[key] = json!(content);
    }
}
pub(crate) fn boolean(ui: &mut egui::Ui, label: &str, value: &mut Value, key: &str) {
    let mut flag = value[key].as_bool().unwrap_or(false);
    if crate::fields::checkbox(ui, label, &mut flag).changed() {
        value[key] = json!(flag);
    }
}
pub(crate) fn integer(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<i64>,
) {
    let mut number = value[key].as_i64().unwrap_or(*range.start());
    if field(ui, label, 72.0, |ui| {
        ui.add(egui::DragValue::new(&mut number).range(range))
            .changed()
    }) {
        value[key] = json!(number);
    }
}
pub(crate) fn number(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) {
    let default = match key {
        "opacity" | "scale" => 1.0,
        "fontSize" => 128.0,
        "scaleX" | "scaleY" | "innerRadius" => 0.5,
        "unsharpAmount" => 100.0,
        "unsharpRadius" => 1.0,
        _ => 0.0,
    };
    let mut number = value[key].as_f64().unwrap_or(default);
    if field(ui, label, 72.0, |ui| {
        ui.add(egui::DragValue::new(&mut number).range(range).speed(speed))
            .changed()
    }) {
        value[key] = json!(number);
    }
}
fn decimal(ui: &mut egui::Ui, entity_id: &str, label: &str, value: &mut Value, key: &str) {
    let mut content = {
        if value[key].is_null() {
            String::new()
        } else {
            value[key]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value[key].to_string())
        }
    };
    let response = field(ui, label, 72.0, |ui| {
        ui.push_id((entity_id, key), |ui| {
            ui.add(egui::TextEdit::singleline(&mut content).desired_width(ui.available_width()))
        })
        .inner
    });
    if response.changed() {
        if content.is_empty() {
            value[key] = Value::Null;
        } else if let Ok(v) = serde_json::from_str::<Value>(&content)
            && v.is_number()
        {
            value[key] = v;
        } else {
            value[key] = json!(content);
        }
    }
}
pub(crate) fn choice(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Value,
    key: &str,
    choices: &[&str],
) {
    let mut selected = value[key].as_str().unwrap_or(choices[0]).to_owned();
    let previous = selected.clone();
    field(ui, label, 120.0, |ui| {
        crate::dropdowns::Dropdown::from_id_salt(key)
            .width(ui.available_width().min(180.0))
            .truncate()
            .selected_text(&selected)
            .show_ui(ui, |ui| {
                for choice in choices {
                    ui.app_selectable_value(&mut selected, (*choice).into(), *choice);
                }
            })
    });
    if previous != selected {
        value[key] = json!(selected);
    }
}
