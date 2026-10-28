use crate::Desktop;
use crate::style_picker::Glyph;
use achimeld_application::{EditorPort, EntityKind, Preferences, UiRequest};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::Value;
use std::collections::BTreeMap;

fn api_name(entity: &Value) -> &str {
    entity["apiName"]
        .as_str()
        .or_else(|| entity.pointer("/apiName/value").and_then(Value::as_str))
        .unwrap_or("")
}

fn localizations<'a>(snapshot: &'a Value, language: &str) -> BTreeMap<(&'a str, &'a str), &'a str> {
    snapshot["localizations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let field = entry["field"].as_str()?;
            let values = entry["values"].as_object()?;
            let text = values
                .iter()
                .find(|(key, value)| {
                    key.eq_ignore_ascii_case(language)
                        && value.as_str().is_some_and(|text| !text.is_empty())
                })
                .or_else(|| {
                    values
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case("English"))
                })
                .and_then(|(_, value)| value.as_str())?;
            Some(((api_name(entry), field), text))
        })
        .collect()
}

pub(crate) fn glyph_image(
    ctx: &egui::Context,
    name: &str,
    bytes: &'static [u8],
    size: f32,
) -> egui::Image<'static> {
    let id = egui::Id::new(("steam-glyph", name));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture = ctx.load_texture(
                format!("steam-glyph-{name}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), Vec2::splat(size)))
}

fn delete_fill(preferences: &Preferences) -> Color32 {
    crate::theme::mix(
        crate::theme::color(preferences, "Warning"),
        crate::theme::color(preferences, "Error"),
        0.62,
    )
}

fn paint_mark(painter: &egui::Painter, center: Pos2, marked: bool, fill: Color32, ink: Color32) {
    let radius = 11.0;
    if marked {
        painter.circle_filled(center, radius, fill);
        let stroke = Stroke::new(2.0, ink);
        painter.line_segment(
            [center + Vec2::new(-5.0, 0.5), center + Vec2::new(-1.5, 4.0)],
            stroke,
        );
        painter.line_segment(
            [center + Vec2::new(-1.5, 4.0), center + Vec2::new(5.5, -4.0)],
            stroke,
        );
    } else {
        painter.circle_stroke(center, radius - 1.0, Stroke::new(1.5, ink));
    }
}

pub(crate) struct MarkedEntry {
    pub kind: EntityKind,
    pub api: String,
    pub name: String,
}

pub(crate) enum ReviewChoice {
    Idle,
    Action(usize),
    Dismissed,
}

pub(crate) struct ReviewAction {
    pub label: &'static str,
    pub icon: egui::Image<'static>,
}

fn prompt_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}

pub(crate) fn review_prompt(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    header_icon: egui::Image<'static>,
    actions: &[ReviewAction],
    body: impl FnOnce(&mut egui::Ui),
) -> ReviewChoice {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new(id))
        .frame(
            egui::Frame::new()
                .fill(Color32::BLACK)
                .corner_radius(0)
                .stroke(Stroke::new(
                    1.0,
                    ctx.global_style()
                        .visuals
                        .widgets
                        .noninteractive
                        .bg_stroke
                        .color,
                )),
        )
        .show(ctx, |ui| {
            let font = egui::TextStyle::Button.resolve(ui.style());
            let natural: Vec<f32> = actions
                .iter()
                .map(|action| {
                    ui.painter()
                        .layout_no_wrap(
                            crate::localization::text(ctx, action.label),
                            font.clone(),
                            ui.visuals().text_color(),
                        )
                        .size()
                        .x
                        + 28.0
                        + 2.0 * ui.spacing().button_padding.x
                })
                .collect();
            let total = natural.iter().sum::<f32>().max(1.0);
            let width = 468.0_f32
                .max(total)
                .min((ctx.content_rect().width() - 32.0).max(1.0));
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let title_galley = ui.painter().layout_no_wrap(
                title.into(),
                egui::TextStyle::Heading.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            let (header, _) = ui.allocate_exact_size(
                Vec2::new(width, title_galley.size().y.max(24.0) + 24.0),
                Sense::hover(),
            );
            header_icon
                .clone()
                .tint(ui.visuals().text_color())
                .paint_at(
                    ui,
                    Rect::from_center_size(
                        egui::pos2(header.left() + 36.0, header.center().y),
                        Vec2::splat(24.0),
                    ),
                );
            ui.painter().galley(
                header.center() - title_galley.size() / 2.0,
                title_galley,
                ui.visuals().text_color(),
            );
            prompt_divider(ui);
            let bar = crate::buttons::bar_height(ui, 20.0);
            let body_height =
                (ctx.content_rect().height() - header.height() - bar - 76.0).clamp(120.0, 420.0);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(24, 20))
                .show(ui, |ui| {
                    ui.set_width((width - 48.0).max(1.0));
                    egui::ScrollArea::vertical()
                        .id_salt((id, "body"))
                        .max_height(body_height)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            body(ui);
                        });
                });
            prompt_divider(ui);
            ui.allocate_ui_with_layout(
                Vec2::new(width, bar),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let start = ui.cursor().left();
                    let mut used = 0.0;
                    for (index, action) in actions.iter().enumerate() {
                        used += natural[index];
                        let right = if index + 1 == actions.len() {
                            start + width
                        } else {
                            ((start + width * used / total) * ctx.pixels_per_point()).round()
                                / ctx.pixels_per_point()
                        };
                        if ui
                            .add(
                                crate::buttons::Button::new((
                                    action.icon.clone(),
                                    crate::localization::text(ctx, action.label),
                                ))
                                .id(egui::Id::new((id, "action", index)))
                                .bar()
                                .min_size(Vec2::new((right - ui.cursor().left()).max(1.0), bar))
                                .icon_spacing(8.0)
                                .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            choice = Some(index);
                        }
                    }
                },
            );
        });
    if let Some(index) = choice {
        ReviewChoice::Action(index)
    } else if response.should_close() {
        ReviewChoice::Dismissed
    } else {
        ReviewChoice::Idle
    }
}

pub(crate) fn review_section(ui: &mut egui::Ui, title: &str, lines: &[String]) {
    ui.add_space(10.0);
    ui.strong(format!("{title} ({})", lines.len()));
    ui.add_space(4.0);
    if lines.is_empty() {
        ui.weak("None");
        return;
    }
    for line in lines {
        egui::Frame::new()
            .fill(ui.visuals().widgets.inactive.bg_fill)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(line);
            });
        ui.add_space(4.0);
    }
}

impl Desktop {
    pub(crate) fn retain_steam_marks(&mut self, snapshot: &Value) {
        if snapshot.is_null() {
            self.steam_marks.clear();
            self.steam_delete_review = false;
            return;
        }
        self.steam_marks.retain(|(kind, api)| {
            snapshot[kind.key()]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| api_name(row) == api))
        });
        if self.steam_marks.is_empty() {
            self.steam_delete_review = false;
        }
    }

    pub(crate) fn marked_steam_entries(&self, port: &dyn EditorPort) -> Vec<MarkedEntry> {
        let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
        let language = port.preferences().language.clone();
        let localized = localizations(snapshot, &language);
        let mut entries = Vec::new();
        for (kind, field) in [
            (EntityKind::Achievement, "AchievementDisplayName"),
            (EntityKind::Stat, "StatDisplayName"),
        ] {
            let Some(rows) = snapshot[kind.key()].as_array() else {
                continue;
            };
            for entity in rows {
                let api = api_name(entity);
                if api.is_empty() || !self.steam_marks.contains(&(kind, api.to_owned())) {
                    continue;
                }
                entries.push(MarkedEntry {
                    kind,
                    api: api.to_owned(),
                    name: localized
                        .get(&(api, field))
                        .copied()
                        .unwrap_or("")
                        .to_owned(),
                });
            }
        }
        entries
    }

    pub(crate) fn steam_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        crate::buttons::bar_panel(ui, "steam-toolbar", 24.0).show(ui, |ui| {
            let height = ui.available_height();
            let reading = port.reading();
            egui::ScrollArea::horizontal()
                .id_salt("steam-toolbar-scroll")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    crate::buttons::bar_with_height(ui, height, |ui| {
                        self.browser_profile_actions(ui, port);
                        if reading {
                            ui.spinner();
                            ui.label("Reading Steam…");
                            if ui.add(crate::buttons::Button::new("Stop").bar()).clicked() {
                                self.request(UiRequest::CancelRead);
                            }
                        } else if ui
                            .add_enabled(
                                !port.browser_profile_busy() && !port.synchronizing(),
                                crate::buttons::Button::new((
                                    glyph_image(
                                        ui.ctx(),
                                        "steam-read",
                                        include_bytes!("../assets/toolbar/steam-read.rgba"),
                                        20.0,
                                    ),
                                    "Read Steam",
                                ))
                                .bar()
                                .icon_spacing(5.0)
                                .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            self.steam_marks.clear();
                            self.steam_delete_review = false;
                            self.request(UiRequest::ReadSteam);
                        }
                        if !reading
                            && !self.steam_marks.is_empty()
                            && ui
                                .add(
                                    crate::buttons::Button::new((
                                        glyph_image(
                                            ui.ctx(),
                                            "steam-delete",
                                            include_bytes!("../assets/toolbar/steam-delete.rgba"),
                                            20.0,
                                        ),
                                        "Delete on Steam",
                                    ))
                                    .bar()
                                    .icon_spacing(5.0)
                                    .image_tint_follows_text_color(true),
                                )
                                .clicked()
                        {
                            self.sync_plan_token = None;
                            self.steam_delete_fresh = true;
                            self.steam_delete_review = true;
                        }
                    });
                });
        });
        egui::Frame::new().inner_margin(24).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("steam-configuration")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
                    self.retain_steam_marks(snapshot);
                    let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
                    ui.heading("Steam");
                    let fill = delete_fill(port.preferences());
                    let reading = port.reading();
                    ui.label("Imported from Steam.");
                    if let Some(read_at) = snapshot["readAtUtc"].as_str() {
                        ui.weak(format!("Read at {read_at}"));
                    }
                    if snapshot.is_null() {
                        ui.label("Read Steam to view its configuration.");
                    } else if snapshot["isComplete"].as_bool() == Some(false) {
                        ui.colored_label(
                            crate::theme::color(port.preferences(), "Warning"),
                            "This Steam snapshot is incomplete.",
                        );
                    } else {
                        ui.weak("Select an entry to mark it for deletion.");
                    }
                    let filter = self.filter.to_lowercase();
                    let localized = localizations(snapshot, &port.preferences().language);
                    let mark_ink = crate::theme::color(port.preferences(), "Primary text");
                    let ring = crate::theme::color(port.preferences(), "Muted text");
                    for (kind, title, name_field) in [
                        (
                            EntityKind::Achievement,
                            "Achievements",
                            "AchievementDisplayName",
                        ),
                        (EntityKind::Stat, "Stats", "StatDisplayName"),
                    ] {
                        ui.add_space(12.0);
                        ui.separator();
                        let entities = snapshot[kind.key()]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        ui.heading(format!("{title} ({})", entities.len()));
                        let visible: Vec<_> = entities
                            .iter()
                            .enumerate()
                            .filter(|(_, entity)| {
                                let api = api_name(entity);
                                let name = localized.get(&(api, name_field)).copied().unwrap_or("");
                                filter.is_empty()
                                    || format!("{name} {api}").to_lowercase().contains(&filter)
                            })
                            .collect();
                        let width = (ui.available_width() - 16.0).max(1.0);
                        let column_count =
                            ((width + 12.0) / 372.0).floor().clamp(1.0, 3.0) as usize;
                        ui.scope(|ui| {
                            ui.set_width(width);
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            for row in visible.chunks(column_count) {
                                ui.columns(column_count, |columns| {
                                    for (column, &(index, entity)) in row.iter().enumerate() {
                                        let ui = &mut columns[column];
                                        let api = api_name(entity);
                                        let name = localized
                                            .get(&(api, name_field))
                                            .copied()
                                            .unwrap_or("");
                                        let marked = !api.is_empty()
                                            && self.steam_marks.contains(&(kind, api.to_owned()));
                                        ui.push_id((kind.key(), index), |ui| {
                                            let panel =
                                                crate::theme::color(port.preferences(), "Panel");
                                            let tile =
                                                egui::Frame::new()
                                                    .fill(if marked {
                                                        crate::theme::mix(panel, fill, 0.18)
                                                    } else {
                                                        panel
                                                    })
                                                    .stroke(Stroke::new(
                                                        if marked { 1.5 } else { 0.0 },
                                                        fill,
                                                    ))
                                                    .inner_margin(12)
                                                    .show(ui, |ui| {
                                                        ui.set_width(ui.available_width());
                                                        ui.horizontal_top(|ui| {
                                                            let mark_width = 22.0;
                                                            let gap = ui.spacing().item_spacing.x;
                                                            ui.vertical(|ui| {
                                                                ui.set_width(
                                                                    (ui.available_width()
                                                                        - mark_width
                                                                        - gap)
                                                                        .max(1.0),
                                                                );
                                                                ui.strong(if name.is_empty() {
                                                                    api
                                                                } else {
                                                                    name
                                                                });
                                                                if !name.is_empty() {
                                                                    ui.weak(api);
                                                                }
                                                                if kind == EntityKind::Achievement {
                                                                    let description = localized
                                                        .get(&(api, "AchievementDescription"))
                                                        .copied()
                                                        .unwrap_or("");
                                                                    if !description.is_empty() {
                                                                        ui.label(description);
                                                                    }
                                                                }
                                                                ui.horizontal_wrapped(|ui| {
                                                    for (key, label, attribute) in [
                                                        ("type", "Type", "data-type"),
                                                        ("setBy", "Set by", "data-set-by"),
                                                        ("hidden", "Hidden", "data-hidden"),
                                                        (
                                                            "progressStatApiName",
                                                            "Progress stat",
                                                            "data-progress-stat",
                                                        ),
                                                        (
                                                            "unlockValue",
                                                            "Unlock value",
                                                            "data-unlock-value",
                                                        ),
                                                        (
                                                            "incrementOnly",
                                                            "Increment only",
                                                            "data-increment-only",
                                                        ),
                                                        ("minimum", "Minimum", "data-minimum"),
                                                        ("maximum", "Maximum", "data-maximum"),
                                                        ("defaultValue", "Default", "data-default"),
                                                        (
                                                            "maximumChange",
                                                            "Maximum change",
                                                            "data-maximum-change",
                                                        ),
                                                        (
                                                            "aggregated",
                                                            "Aggregated",
                                                            "data-aggregated",
                                                        ),
                                                        (
                                                            "averageRateWindowSeconds",
                                                            "Rate window (s)",
                                                            "data-average-window",
                                                        ),
                                                        (
                                                            "displayOrder",
                                                            "Display order",
                                                            "data-display-order",
                                                        ),
                                                    ] {
                                                        let value = entity
                                                            .get(key)
                                                            .filter(|v| !v.is_null())
                                                            .or_else(|| {
                                                                entity["attributes"].get(attribute)
                                                            })
                                                            .unwrap_or(&Value::Null);
                                                        let value =
                                                            value.get("value").unwrap_or(value);
                                                        if !value.is_null()
                                                            && value.as_str() != Some("")
                                                        {
                                                            let text = match value {
                                                                Value::String(text)
                                                                    if text == "true" =>
                                                                {
                                                                    "Yes".into()
                                                                }
                                                                Value::String(text)
                                                                    if text == "false" =>
                                                                {
                                                                    "No".into()
                                                                }
                                                                Value::String(text) => text.clone(),
                                                                Value::Bool(value) => if *value {
                                                                    "Yes"
                                                                } else {
                                                                    "No"
                                                                }
                                                                .into(),
                                                                _ => value.to_string(),
                                                            };
                                                            ui.label(format!("{label}: {text}"));
                                                        }
                                                    }
                                                });
                                                            });
                                                            let (mark, _) = ui.allocate_exact_size(
                                                                Vec2::splat(22.0),
                                                                Sense::hover(),
                                                            );
                                                            if !api.is_empty() {
                                                                paint_mark(
                                                                    ui.painter(),
                                                                    mark.center(),
                                                                    marked,
                                                                    fill,
                                                                    if marked {
                                                                        mark_ink
                                                                    } else {
                                                                        ring
                                                                    },
                                                                );
                                                            }
                                                        });
                                                    });
                                            if !api.is_empty() && !reading {
                                                let response = ui.interact(
                                                    tile.response.rect,
                                                    ui.id().with("select"),
                                                    Sense::click(),
                                                );
                                                if response.hovered() {
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }
                                                if response.clicked() {
                                                    let key = (kind, api.to_owned());
                                                    if !self.steam_marks.remove(&key) {
                                                        self.steam_marks.insert(key);
                                                    }
                                                }
                                            }
                                        });
                                    }
                                });
                                ui.add_space(8.0);
                            }
                        });
                        if visible.is_empty() {
                            ui.weak(if snapshot.is_null() {
                                "Not loaded"
                            } else if entities.is_empty() {
                                "None in the loaded Steam configuration"
                            } else {
                                "No matches"
                            });
                        }
                    }
                });
        });
    }

    pub(crate) fn steam_delete_window(&mut self, ctx: &egui::Context, port: &dyn EditorPort) {
        if !self.steam_delete_review {
            return;
        }
        {
            let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
            self.retain_steam_marks(snapshot);
        }
        if !self.steam_delete_review {
            return;
        }
        let entries = self.marked_steam_entries(port);
        let count = entries.len();
        let intro = if count == 1 {
            "1 entry is marked for deletion.".to_owned()
        } else {
            format!("{count} entries are marked for deletion.")
        };
        let choice = review_prompt(
            ctx,
            "steam-delete-review",
            "Delete on Steam",
            glyph_image(
                ctx,
                "steam-delete",
                include_bytes!("../assets/toolbar/steam-delete.rgba"),
                24.0,
            ),
            &[ReviewAction {
                label: "Close",
                icon: Glyph::Cancel.image(ctx, 20.0),
            }],
            |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(intro).wrap().halign(egui::Align::Center));
                });
                for (kind, title) in [
                    (EntityKind::Achievement, "Achievements"),
                    (EntityKind::Stat, "Stats"),
                ] {
                    let lines: Vec<String> = entries
                        .iter()
                        .filter(|entry| entry.kind == kind)
                        .map(|entry| {
                            if entry.name.is_empty() {
                                entry.api.clone()
                            } else {
                                format!("{} · {}", entry.name, entry.api)
                            }
                        })
                        .collect();
                    if lines.is_empty() {
                        continue;
                    }
                    review_section(ui, title, &lines);
                }
            },
        );
        let fresh = std::mem::take(&mut self.steam_delete_fresh);
        if !fresh && !matches!(choice, ReviewChoice::Idle) {
            self.steam_delete_review = false;
        }
    }
}
