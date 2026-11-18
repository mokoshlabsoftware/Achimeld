use crate::Desktop;
use crate::style_picker::Glyph;
use achimeld_application::{EditorPort, EntityKind, Preferences, UiRequest};
use egui::{Align, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::Value;
use std::collections::BTreeMap;

fn api_name(entity: &Value) -> &str {
    entity["apiName"]
        .as_str()
        .or_else(|| entity.pointer("/apiName/value").and_then(Value::as_str))
        .unwrap_or("")
}

pub(crate) fn pending_rename<'a>(
    project: &'a Value,
    entity: &'a Value,
    stat: bool,
) -> Option<&'a str> {
    if entity["pendingApiRename"]["appId"] == project["steamAppId"]
        && let Some(old) = entity["pendingApiRename"]["from"]
            .as_str()
            .filter(|old| *old != api_name(entity))
    {
        return Some(old);
    }
    let kind = if stat { "Stat" } else { "Achievement" };
    let baseline = project["syncBaselines"]
        .as_array()?
        .iter()
        .find(|b| b["entityType"] == kind && b["localId"] == entity["id"])?;
    let old = baseline["apiName"].as_str()?;
    (old != api_name(entity)).then_some(old)
}

pub(crate) fn rename_panel(
    ui: &mut egui::Ui,
    port: &dyn EditorPort,
    entity: &mut Value,
    stat: bool,
) {
    use crate::buttons::UiButtonExt as _;
    let Some(old) =
        pending_rename(port.editor().document().project(), entity, stat).map(str::to_owned)
    else {
        return;
    };
    let fill = crate::theme::color(port.preferences(), "Warning");
    let response = egui::Frame::new()
        .fill(fill)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!(
                    "{} {old}",
                    crate::localization::text(ui.ctx(), "Rename from")
                ))
                .color(egui::Color32::BLACK),
            );
        })
        .response
        .interact(Sense::click());
    crate::buttons::context_menu(&response, |ui| {
        if ui
            .app_button(crate::localization::text(ui.ctx(), "Revert rename"))
            .clicked()
        {
            entity["apiName"] = Value::String(old.clone());
            ui.close();
        }
    });
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

pub(crate) fn achievement_set_id(achievement: &Value) -> u64 {
    let set = &achievement["setId"];
    set.as_u64()
        .or_else(|| set.as_str()?.parse().ok())
        .unwrap_or(0)
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

fn steam_art_mesh(texture: egui::TextureId, rect: Rect, opacity: f32) -> egui::Shape {
    let left = rect.center().x;
    let mut mesh = egui::Mesh::with_texture(texture);
    for (position, uv, alpha) in [
        (Pos2::new(left, rect.top()), Pos2::new(0.0, 0.0), 0),
        (rect.right_top(), Pos2::new(1.0, 0.0), 128),
        (rect.right_bottom(), Pos2::new(1.0, 1.0), 128),
        (Pos2::new(left, rect.bottom()), Pos2::new(0.0, 1.0), 0),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: position,
            uv,
            color: Color32::from_white_alpha((alpha as f32 * opacity).round() as u8),
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    egui::Shape::mesh(mesh)
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
    pub colors: Option<(Color32, Color32)>,
    pub enabled: bool,
}

struct PromptLayout {
    width: f32,
    body_height: f32,
    progress: Option<f32>,
    adjustable: bool,
}

#[derive(Clone, Copy, Default)]
struct ReviewWindow {
    size: Option<Vec2>,
    maximized: bool,
}

fn resize_review(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: Rect,
    minimum: Vec2,
    window: &mut ReviewWindow,
) {
    use egui::ResizeDirection::*;
    let bounds = ui.ctx().content_rect().shrink(3.0);
    for (index, (region, side, cursor)) in
        crate::chrome::resize_regions(rect).into_iter().enumerate()
    {
        let response = ui
            .interact(region, id.with(("review-resize", index)), Sense::drag())
            .on_hover_cursor(cursor);
        if !response.dragged() {
            continue;
        }
        let delta = response.drag_delta();
        let mut next = rect;
        if matches!(side, West | NorthWest | SouthWest) {
            next.min.x = (rect.left() + delta.x).clamp(
                bounds.left().min(rect.right() - minimum.x),
                rect.right() - minimum.x,
            );
        }
        if matches!(side, East | NorthEast | SouthEast) {
            next.max.x = (rect.right() + delta.x).clamp(
                rect.left() + minimum.x,
                bounds.right().max(rect.left() + minimum.x),
            );
        }
        if matches!(side, North | NorthWest | NorthEast) {
            next.min.y = (rect.top() + delta.y).clamp(
                bounds.top().min(rect.bottom() - minimum.y),
                rect.bottom() - minimum.y,
            );
        }
        if matches!(side, South | SouthWest | SouthEast) {
            next.max.y = (rect.bottom() + delta.y).clamp(
                rect.top() + minimum.y,
                bounds.bottom().max(rect.top() + minimum.y),
            );
        }
        window.size = Some(next.size());
        ui.ctx()
            .data_mut(|data| data.insert_temp(id.with("drag-position"), next.min));
        ui.request_repaint();
    }
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
    prompt(
        ctx,
        id,
        title,
        header_icon,
        actions,
        PromptLayout {
            width: 468.0,
            body_height: 420.0,
            progress: None,
            adjustable: false,
        },
        body,
    )
}

pub(crate) fn review_prompt_wide(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    header_icon: egui::Image<'static>,
    actions: &[ReviewAction],
    progress: Option<f32>,
    body: impl FnOnce(&mut egui::Ui),
) -> ReviewChoice {
    prompt(
        ctx,
        id,
        title,
        header_icon,
        actions,
        PromptLayout {
            width: 880.0,
            body_height: 680.0,
            progress,
            adjustable: true,
        },
        body,
    )
}

fn prompt(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    header_icon: egui::Image<'static>,
    actions: &[ReviewAction],
    layout: PromptLayout,
    body: impl FnOnce(&mut egui::Ui),
) -> ReviewChoice {
    let mut choice = None;
    let modal_id = egui::Id::new(id);
    let geometry_id = modal_id.with("review-window");
    let mut window = ctx
        .data(|data| data.get_temp::<ReviewWindow>(geometry_id))
        .unwrap_or_default();
    let modal = crate::modal::dialog(ctx, modal_id);
    let modal = if layout.adjustable && window.maximized {
        egui::Modal::new(modal_id)
    } else {
        modal
    };
    let frame = egui::Frame::new()
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
        ));
    let content = |ui: &mut egui::Ui| {
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
        let title_galley = ui.painter().layout_no_wrap(
            title.into(),
            egui::TextStyle::Heading.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        let header_height = title_galley.size().y.max(24.0) + 24.0;
        let bar = crate::buttons::bar_height(ui, 20.0);
        let chrome_height =
            header_height + bar + 42.0 + if layout.progress.is_some() { 2.0 } else { 0.0 };
        let viewport = ctx.content_rect().size();
        let maximum = (viewport - Vec2::splat(8.0)).max(Vec2::splat(1.0));
        let minimum = Vec2::new(
            total.max(title_galley.size().x + 144.0).max(350.0),
            chrome_height + 100.0,
        )
        .min(maximum);
        let initial = Vec2::new(
            layout.width.max(total).min((viewport.x - 32.0).max(1.0)),
            0.8 * (chrome_height
                + (viewport.y - header_height - bar - 76.0).clamp(120.0, layout.body_height)),
        );
        let size = if layout.adjustable && window.maximized {
            maximum
        } else {
            window.size.unwrap_or(initial).max(minimum).min(maximum)
        };
        let width = if layout.adjustable { size.x } else { initial.x };
        ui.set_width(width);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let (header, _) = ui.allocate_exact_size(Vec2::new(width, header_height), Sense::hover());
        if !layout.adjustable || !window.maximized {
            let mut drag = header;
            if layout.adjustable {
                drag.max.x -= 72.0;
            }
            crate::modal::drag_header(ui, drag);
        }
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
        if layout.adjustable {
            let controls = Rect::from_min_max(
                header.right_top() - Vec2::new(72.0, 0.0),
                header.right_bottom(),
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(controls)
                    .layout(egui::Layout::left_to_right(Align::Center)),
                |ui| {
                    let maximize =
                        crate::buttons::Button::new(if window.maximized { "▣" } else { "□" })
                            .bar()
                            .resting_colors(Color32::BLACK, ui.visuals().text_color())
                            .min_size(Vec2::new(36.0, header_height));
                    if ui
                        .add(maximize)
                        .on_hover_text(if window.maximized {
                            "Restore"
                        } else {
                            "Maximize"
                        })
                        .clicked()
                    {
                        window.maximized = !window.maximized;
                        ctx.request_repaint();
                    }
                    if ui
                        .add(
                            crate::buttons::Button::new("×")
                                .bar()
                                .resting_colors(Color32::BLACK, ui.visuals().text_color())
                                .min_size(Vec2::new(36.0, header_height)),
                        )
                        .on_hover_text("Close")
                        .clicked()
                    {
                        choice = actions.len().checked_sub(1);
                    }
                },
            );
        }
        prompt_divider(ui);
        if let Some(remaining) = layout.progress {
            let (track, _) = ui.allocate_exact_size(
                Vec2::new(width, 2.0 / ctx.pixels_per_point()),
                Sense::hover(),
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    track.min,
                    egui::pos2(
                        track.left() + track.width() * remaining.clamp(0.0, 1.0),
                        track.bottom(),
                    ),
                ),
                0.0,
                ui.visuals().selection.bg_fill,
            );
        }
        let body_height = if layout.adjustable {
            (size.y - chrome_height).max(1.0)
        } else {
            (viewport.y - header.height() - bar - 76.0).clamp(120.0, layout.body_height)
        };
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(24, 20))
            .show(ui, |ui| {
                ui.set_width((width - 48.0).max(1.0));
                if layout.adjustable {
                    ui.set_min_height(body_height);
                }
                egui::ScrollArea::vertical()
                    .id_salt((id, "body"))
                    .max_height(body_height)
                    .auto_shrink([true, !layout.adjustable])
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
                        .add_enabled(
                            action.enabled,
                            crate::buttons::Button::new((
                                action.icon.clone(),
                                crate::localization::text(ctx, action.label),
                            ))
                            .id(egui::Id::new((id, "action", index)))
                            .bar()
                            .resting_colors(
                                action.colors.map_or(ui.visuals().panel_fill, |c| c.0),
                                action.colors.map_or(ui.visuals().text_color(), |c| c.1),
                            )
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
        if layout.adjustable && !window.maximized {
            let rect = Rect::from_min_max(
                header.min,
                egui::pos2(header.right(), ui.min_rect().bottom()),
            );
            resize_review(ui, modal_id, rect, minimum, &mut window);
        }
    };
    let response = modal.frame(frame).show(ctx, content);
    if layout.adjustable {
        ctx.data_mut(|data| data.insert_temp(geometry_id, window));
    }
    let dismissed = response.should_close();
    if let Some(index) = choice {
        ReviewChoice::Action(index)
    } else if dismissed {
        ReviewChoice::Dismissed
    } else if crate::modal::enter(ctx, &response) {
        let preferred = actions
            .iter()
            .position(|action| action.enabled && action.label == "Synchronize now")
            .or_else(|| {
                actions.iter().position(|action| {
                    action.enabled && action.label != "Stop" && action.label != "Start 5 s"
                })
            });
        preferred.map_or(ReviewChoice::Idle, ReviewChoice::Action)
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
            .corner_radius(0)
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
        if Self::steam_operation_active(port) {
            ui.disable();
        }
        let mut filter_changed = false;
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
                        ui.add_space(8.0);
                        let filter = ui.add(
                            egui::TextEdit::singleline(&mut self.steam_filter)
                                .id(egui::Id::new("steam-filter"))
                                .hint_text("Name, API name, or description")
                                .margin(egui::Margin {
                                    left: 22,
                                    right: 4,
                                    top: 2,
                                    bottom: 2,
                                })
                                .desired_width(210.0),
                        );
                        filter_changed = filter.changed();
                        let center = egui::pos2(filter.rect.left() + 11.0, filter.rect.center().y);
                        let top = center - egui::vec2(4.0, 5.0);
                        let bottom = center + egui::vec2(4.0, 5.0);
                        let upper_right = egui::pos2(bottom.x, top.y);
                        let lower_left = egui::pos2(top.x, bottom.y);
                        let stroke = egui::Stroke::new(1.2, ui.visuals().weak_text_color());
                        for segment in [
                            [top, upper_right],
                            [upper_right, lower_left],
                            [lower_left, bottom],
                            [bottom, top],
                        ] {
                            ui.painter().line_segment(segment, stroke);
                        }
                        ui.add_space(8.0);
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
                                    .resting_colors(
                                        crate::theme::mix(
                                            crate::theme::color(port.preferences(), "Panel"),
                                            delete_fill(port.preferences()),
                                            0.18,
                                        ),
                                        ui.visuals().text_color(),
                                    )
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
            let scroll = egui::ScrollArea::vertical()
                .id_salt("steam-configuration")
                .auto_shrink([false, false]);
            let scroll = if filter_changed {
                scroll.vertical_scroll_offset(0.0)
            } else {
                scroll
            };
            scroll.show(ui, |ui| {
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
                    let filter = self.steam_filter.trim().to_lowercase();
                    let localized = localizations(snapshot, &port.preferences().language);
                    let mark_ink = crate::theme::color(port.preferences(), "Primary text");
                    let ring = crate::theme::color(port.preferences(), "Muted text");
                    for (kind, title, name_field, description_field) in [
                        (
                            EntityKind::Achievement,
                            "Achievements",
                            "AchievementDisplayName",
                            Some("AchievementDescription"),
                        ),
                        (EntityKind::Stat, "Stats", "StatDisplayName", None),
                    ] {
                        ui.add_space(12.0);
                        ui.separator();
                        let entities = snapshot[kind.key()]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        ui.heading(format!("{title} ({})", entities.len()));
                        ui.push_id(("steam-mark-actions", kind.key()), |ui| {
                            crate::buttons::bar(ui, 0.0, |ui| {
                                let all_marked = entities.iter().filter(|row| !api_name(row).is_empty()).all(|row| {
                                    self.steam_marks.contains(&(kind, api_name(row).to_owned()))
                                });
                                if ui.add_enabled(!reading && !all_marked,
                                    crate::buttons::Button::new("Mark all").bar())
                                    .on_hover_text(format!("Mark every loaded {title} entry, including entries hidden by the filter."))
                                    .clicked()
                                {
                                    self.steam_marks.extend(entities.iter().filter_map(|row| {
                                        let api = api_name(row);
                                        (!api.is_empty()).then(|| (kind, api.to_owned()))
                                    }));
                                }
                                if ui.add_enabled(!reading && self.steam_marks.iter().any(|(marked_kind, _)| *marked_kind == kind),
                                    crate::buttons::Button::new("Clear marks").bar()).clicked()
                                {
                                    self.steam_marks.retain(|(marked_kind, _)| *marked_kind != kind);
                                }
                            });
                        });
                        let show_sets = kind == EntityKind::Achievement
                            && entities.iter().any(|entity| achievement_set_id(entity) != 0);
                        let mut groups: BTreeMap<u64, Vec<_>> = BTreeMap::new();
                        for item in entities
                            .iter()
                            .enumerate()
                            .filter_map(|(index, entity)| {
                                let api = api_name(entity);
                                let name = localized.get(&(api, name_field)).copied().unwrap_or("");
                                let description = description_field
                                    .and_then(|field| localized.get(&(api, field)).copied())
                                    .unwrap_or("");
                                let matches = filter.is_empty()
                                    || format!("{name} {api}").to_lowercase().contains(&filter)
                                    || description.to_lowercase().contains(&filter);
                                matches.then_some((index, entity, api, name, description))
                            })
                        {
                            let set = if show_sets { achievement_set_id(item.1) } else { 0 };
                            groups.entry(set).or_default().push(item);
                        }
                        let width = (ui.available_width() - 16.0).max(1.0);
                        let column_count =
                            ((width + 12.0) / 372.0).floor().clamp(1.0, 3.0) as usize;
                        ui.scope(|ui| {
                            ui.set_width(width);
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            for (set, visible) in &groups {
                                if show_sets {
                                    ui.add_space(6.0);
                                    if *set == 0 {
                                        ui.strong("Base game");
                                    } else {
                                        ui.strong(format!("Set {set}"));
                                    }
                                }
                            for row in visible.chunks(column_count) {
                                let mut row_tiles = Vec::new();
                                ui.columns(column_count, |columns| {
                                    for (column, &(index, entity, api, name, description)) in
                                        row.iter().enumerate()
                                    {
                                        let ui = &mut columns[column];
                                        let marked = !api.is_empty()
                                            && self.steam_marks.contains(&(kind, api.to_owned()));
                                        ui.push_id((kind.key(), index), |ui| {
                                            let panel =
                                                crate::theme::color(port.preferences(), "Panel");
                                            let artwork = (kind == EntityKind::Achievement)
                                                .then(|| port.steam_icon_preview(api, false))
                                                .flatten()
                                                .and_then(|preview| self.preview_texture(ui.ctx(), &format!("steam:{api}"), preview));
                                            let alt_artwork = (kind == EntityKind::Achievement)
                                                .then(|| port.steam_icon_preview(api, true))
                                                .flatten()
                                                .and_then(|preview| self.preview_texture(ui.ctx(), &format!("steam-alt:{api}"), preview));
                                            let alt_held = ui.ctx().input(|input| input.modifiers.alt);
                                            let alt_opacity = ui.ctx().animate_bool_with_time(
                                                ui.id().with("steam-unachieved-preview"), alt_held, 0.25);
                                            let tile_fill = if marked { crate::theme::mix(panel, fill, 0.18) } else { panel };
                                            let background_shape = ui.painter().add(egui::Shape::Noop);
                                            let mut artwork_shape = None;
                                            let tile =
                                                egui::Frame::new()
                                                    .inner_margin(12)
                                                    .show(ui, |ui| {
                                                        artwork_shape = Some(ui.painter().add(egui::Shape::Noop));
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
                                                                if !description.is_empty() {
                                                                    ui.label(description);
                                                                }
                                                            ui.horizontal_wrapped(|ui| {
                                                                let detail_ink = crate::theme::color(port.preferences(), "Primary text").gamma_multiply(0.5);
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
                                                            ui.colored_label(detail_ink, format!("{label}: {text}"));
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
                                            row_tiles.push((tile.response.rect, api.to_owned(), tile_fill,
                                                background_shape, artwork.zip(artwork_shape), alt_artwork.map(|texture| (texture, alt_opacity)), ui.id().with("select"), marked));
                                        });
                                    }
                                });
                                let bottom = row_tiles.iter().map(|(rect, ..)| rect.bottom()).fold(0.0_f32, f32::max);
                                for (natural, api, tile_fill, background_shape, artwork, alt_artwork, id, marked) in row_tiles {
                                    let rect = Rect::from_min_max(natural.min, Pos2::new(natural.right(), bottom));
                                    ui.painter().set(background_shape, egui::Shape::rect_filled(rect, 0.0, tile_fill));
                                    if marked { ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.5, fill), egui::StrokeKind::Inside); }
                                    if let Some((texture, artwork_shape)) = artwork {
                                        ui.painter().set(artwork_shape, steam_art_mesh(texture, rect, 1.0));
                                    }
                                    if let Some((texture, opacity)) = alt_artwork.filter(|(_, opacity)| *opacity > 0.0) {
                                        ui.painter().add(steam_art_mesh(texture, rect, opacity));
                                    }
                                    if !api.is_empty() && !reading {
                                        let response = ui.interact(rect, id, Sense::click());
                                        if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                                        if response.clicked() {
                                            let key = (kind, api);
                                            if !self.steam_marks.remove(&key) { self.steam_marks.insert(key); }
                                        }
                                    }
                                }
                                ui.add_space(8.0);
                            }
                            }
                        });
                        if groups.is_empty() {
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
            &[
                ReviewAction {
                    label: "Delete on Steam",
                    icon: Glyph::Delete.image(ctx, 20.0),
                    colors: None,
                    enabled: self.steam_panel_available(port)
                        && !port.synchronizing()
                        && !port.reading()
                        && !port.browser_profile_busy(),
                },
                ReviewAction {
                    label: "Cancel",
                    icon: Glyph::Cancel.image(ctx, 20.0),
                    colors: None,
                    enabled: true,
                },
            ],
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
        if !fresh
            && matches!(choice, ReviewChoice::Action(0))
            && self.steam_panel_available(port)
            && !port.synchronizing()
            && !port.reading()
        {
            self.request(UiRequest::DeleteSteam(
                entries
                    .into_iter()
                    .map(|entry| (entry.kind, entry.api))
                    .collect(),
            ));
        }
        if !fresh && !matches!(choice, ReviewChoice::Idle) {
            self.steam_delete_review = false;
        }
    }
}

#[cfg(test)]
mod rename_tests {
    use super::pending_rename;
    use serde_json::json;

    #[test]
    fn rename_panel_uses_persisted_origin_for_both_kinds_and_clears_on_revert() {
        for stat in [false, true] {
            let project = json!({"steamAppId":480,"syncBaselines":[]});
            let mut entity = json!({"id":"local","apiName":"NEW","pendingApiRename":{"from":"OLD","steamId":"1","appId":480}});
            assert_eq!(pending_rename(&project, &entity, stat), Some("OLD"));
            entity["apiName"] = json!("NEWER");
            assert_eq!(pending_rename(&project, &entity, stat), Some("OLD"));
            entity["apiName"] = json!("OLD");
            assert_eq!(pending_rename(&project, &entity, stat), None);
            entity["apiName"] = json!("NEW");
            entity["pendingApiRename"]["appId"] = json!(481);
            assert_eq!(pending_rename(&project, &entity, stat), None);
        }
    }
}
