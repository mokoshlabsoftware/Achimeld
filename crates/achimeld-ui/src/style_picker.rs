//! Saved-style previews and an undoable, explicitly approved name edit.
use crate::{Desktop, buttons, icons, layer_palette, localization};
use achimeld_application::{Command, EditorPort, EntityKind, IconStyle, style_signature};
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};
use serde_json::{Value, json};

pub(crate) struct StyleRename {
    project: String,
    achievement: String,
    style: String,
    pub name: String,
    focus: bool,
    error: Option<String>,
    viewer: bool,
    recipe: Option<Value>,
}

pub(crate) struct StyleEditPrompt {
    project: String,
    achievement: String,
    style: String,
}

#[derive(Clone, Copy)]
pub(crate) enum Glyph {
    Rename,
    Approve,
    Cancel,
    Import,
    Save,
    Validate,
    Delete,
}
impl Glyph {
    pub(crate) fn image(self, ctx: &egui::Context, size: f32) -> egui::Image<'static> {
        let (name, bytes): (&str, &[u8]) = match self {
            Self::Rename => (
                "rename",
                include_bytes!("../assets/toolbar/style-rename.rgba"),
            ),
            Self::Approve => (
                "approve",
                include_bytes!("../assets/toolbar/style-approve.rgba"),
            ),
            Self::Cancel => (
                "cancel",
                include_bytes!("../assets/toolbar/style-cancel.rgba"),
            ),
            Self::Import => (
                "import",
                include_bytes!("../assets/toolbar/style-import.rgba"),
            ),
            Self::Save => ("save", include_bytes!("../assets/toolbar/style-save.rgba")),
            Self::Delete => (
                "delete",
                include_bytes!("../assets/toolbar/steam-delete.rgba"),
            ),
            Self::Validate => (
                "validate",
                include_bytes!("../assets/toolbar/sync-validate.rgba"),
            ),
        };
        let id = egui::Id::new(("style-action-glyph", name));
        let texture = ctx
            .data(|data| data.get_temp::<egui::TextureHandle>(id))
            .unwrap_or_else(|| {
                let texture = ctx.load_texture(
                    format!("style-action-{name}"),
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
                texture
            });
        egui::Image::new((texture.id(), Vec2::splat(size)))
    }
}

pub(crate) fn row_id(style: &str, action: &str) -> egui::Id {
    egui::Id::new(("project-style-row", style, action))
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}

fn fit(size: Vec2, bounds: Rect) -> Rect {
    let scale = (bounds.width() / size.x).min(bounds.height() / size.y);
    Rect::from_center_size(bounds.center(), size * scale)
}

pub(crate) fn steam_set_caption(port: &dyn EditorPort, style: Option<&str>) -> String {
    use achimeld_application::style_viewer::SteamAchievementSet;
    let links = style
        .map(|id| port.icon_style_steam_sets(id))
        .unwrap_or_default();
    if links.is_empty() {
        return "Not tied to any set".into();
    }
    let sets: Vec<_> = links
        .iter()
        .map(|link| match &link.set {
            SteamAchievementSet::BaseGame => format!("Base game set (App {})", link.app_id),
            SteamAchievementSet::Custom { name, .. } => format!("{name} set (App {})", link.app_id),
        })
        .collect();
    format!("Tied to: {}", sets.join(", "))
}

impl Desktop {
    // Regular icons use recipe matching. Keep the explicitly chosen identity
    // while recipes match, including when a newly saved style duplicates another.
    pub(crate) fn matching_icon_style<'a>(
        &self,
        port: &dyn EditorPort,
        icon: &Value,
        styles: &'a [IconStyle],
    ) -> Option<&'a IconStyle> {
        let linked = self.linked_icon_style(port);
        let chosen = self
            .selection
            .first()
            .and_then(|id| self.icon_style_choices.get(id));
        let signature = style_signature(icon);
        styles
            .iter()
            .find(|style| Some(&style.id) == linked.as_ref())
            .or_else(|| {
                styles.iter().find(|style| {
                    Some(&style.id) == chosen && style_signature(&style.recipe) == signature
                })
            })
            .or_else(|| {
                styles
                    .iter()
                    .find(|style| style_signature(&style.recipe) == signature)
            })
    }

    pub(crate) fn set_style_edit_mode(&mut self, port: &mut dyn EditorPort, style: Option<String>) {
        port.finish_group();
        self.layer_palette.active_style = style;
    }

    fn remember_icon_style(&mut self, port: &dyn EditorPort, achievement: &str, style: &str) {
        self.icon_style_choices.retain(|id, _| {
            port.editor()
                .document()
                .entity(EntityKind::Achievement, id)
                .is_some()
        });
        self.icon_style_choices
            .insert(achievement.into(), style.into());
    }

    pub(crate) fn style_edit_prompt_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
    ) {
        use crate::steam::{ReviewAction, ReviewChoice, review_prompt};
        let Some(prompt) = self.style_edit_prompt.take() else {
            return;
        };
        if port.editor().document().project()["projectId"].as_str() != Some(&prompt.project)
            || self.selection.first() != Some(&prompt.achievement)
            || !port
                .icon_styles()
                .iter()
                .any(|style| style.id == prompt.style)
            || port
                .editor()
                .document()
                .entity(EntityKind::Achievement, &prompt.achievement)
                .is_none()
        {
            return;
        }
        let choice = review_prompt(
            ctx,
            "created-style-edit-mode",
            &localization::text(ctx, "Style created"),
            Glyph::Save.image(ctx, 24.0),
            &[
                ReviewAction {
                    colors: None,
                    enabled: true,
                    label: "Stay in icon edit mode",
                    icon: Glyph::Cancel.image(ctx, 20.0),
                },
                ReviewAction {
                    colors: None,
                    enabled: true,
                    label: "Edit style",
                    icon: Glyph::Rename.image(ctx, 20.0),
                },
            ],
            |ui| {
                ui.add(egui::Label::new(localization::text(ctx,
                    "The new style is assigned to this icon. Stay in icon edit mode, or edit the style and every icon using it?"))
                    .wrap().halign(egui::Align::Center));
            },
        );
        match choice {
            ReviewChoice::Action(1) => self.set_style_edit_mode(port, Some(prompt.style)),
            ReviewChoice::Idle => self.style_edit_prompt = Some(prompt),
            _ => self.set_style_edit_mode(port, None),
        }
    }

    pub(crate) fn linked_icon_style(&self, port: &dyn EditorPort) -> Option<String> {
        self.selection
            .first()
            .and_then(|id| {
                port.editor()
                    .document()
                    .entity(achimeld_application::EntityKind::Achievement, id)
            })
            .filter(|entity| achimeld_application::style_achievements::is_style_achievement(entity))
            .and_then(|entity| entity["iconStyleId"].as_str().map(str::to_owned))
    }
    fn saved_style_texture(
        &mut self,
        ui: &egui::Ui,
        port: &mut dyn EditorPort,
        style: &str,
        achievement: &str,
        edge: usize,
    ) -> Option<(egui::TextureId, Vec2)> {
        // One GPU handle per recipe/context, shared by the banner and full view.
        let key = format!("style:{style}:{achievement}");
        if let Some(preview) = port.icon_style_preview(style, achievement, edge) {
            self.preview_texture(ui.ctx(), &key, preview);
        }
        self.textures
            .get(&key)
            .map(|(_, texture, _)| (texture.id(), texture.size_vec2()))
    }

    pub(crate) fn open_style_rename(&mut self, port: &dyn EditorPort, style: &IconStyle) {
        let Some(achievement) = self.selection.first() else {
            return;
        };
        self.style_rename = Some(StyleRename {
            project: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            achievement: achievement.clone(),
            style: style.id.clone(),
            name: style.name.clone(),
            focus: true,
            error: None,
            viewer: false,
            recipe: None,
        });
    }
    pub(crate) fn open_viewer_style_rename(&mut self, port: &dyn EditorPort, style: &IconStyle) {
        self.style_rename = Some(StyleRename {
            project: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            achievement: String::new(),
            style: style.id.clone(),
            name: style.name.clone(),
            focus: true,
            error: None,
            viewer: true,
            recipe: None,
        });
    }

    pub(crate) fn open_style_save(&mut self, port: &dyn EditorPort, icon: &Value) {
        let Some(achievement) = self.selection.first() else {
            return;
        };
        self.style_rename = Some(StyleRename {
            project: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            achievement: achievement.clone(),
            style: achimeld_application::new_id(),
            name: String::new(),
            focus: true,
            error: None,
            viewer: false,
            recipe: Some(icon.clone()),
        });
    }

    pub(crate) fn project_style_rows(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        icon: &mut Value,
        styles: &[IconStyle],
    ) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let achievement = self.selection.first().cloned().unwrap_or_default();
        let height = 3.0 * ui.text_style_height(&egui::TextStyle::Small).max(15.0)
            + ui.text_style_height(&egui::TextStyle::Small)
            + 4.0;
        let max_height = (ui.ctx().content_rect().height() * 0.48).max(height);
        let ids: Vec<_> = styles
            .iter()
            .map(|style| row_id(&style.id, "apply"))
            .collect();
        let selected = self
            .matching_icon_style(port, icon, styles)
            .map(|style| row_id(&style.id, "apply"));
        let scroll_to = crate::dropdown_navigation::virtual_rows(ui, &ids, selected);
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("project-style-previews")
            .max_height(max_height)
            .auto_shrink([false, true]);
        if let Some(index) = scroll_to {
            scroll = scroll.vertical_scroll_offset(index as f32 * height);
        }
        scroll.show_rows(ui, height, styles.len(), |ui, visible| {
            for style in &styles[visible] {
                ui.horizontal(|ui| {
                    let action_width = 32.0_f32.max(ui.text_style_height(&egui::TextStyle::Button));
                    let width = (ui.available_width() - 2.0 * action_width).max(1.0);
                    let selected = selected == Some(row_id(&style.id, "apply"));
                    let response = ui.add_sized(
                        [width, height],
                        buttons::Button::selectable(selected, "")
                            .id(row_id(&style.id, "apply"))
                            .bar(),
                    );
                    let texture = if ui.is_rect_visible(response.rect) {
                        self.saved_style_texture(ui, port, &style.id, &achievement, 128)
                    } else {
                        None
                    };
                    crate::style_banner::paint(ui, &response, &style.name, texture, selected);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &style.name)
                    });
                    if response.on_hover_text(&style.name).clicked() {
                        let (layers, _) = layer_palette::fresh_bundle(
                            style.recipe["layers"].as_array().map_or(&[], Vec::as_slice),
                            "",
                            None,
                        );
                        *icon = style.recipe.clone();
                        icon["layers"] = json!(layers);
                        self.layer_palette.active_style = None;
                        self.remember_icon_style(port, &achievement, &style.id);
                        self.layer_palette.edit_label = Some("Apply icon style");
                        ui.close();
                    }
                    let rename = ui.add_sized(
                        [action_width, height],
                        buttons::Button::new("")
                            .id(row_id(&style.id, "rename"))
                            .bar(),
                    );
                    let (_, ink) = buttons::colors(ui, &rename, buttons::ButtonStyle::Quiet);
                    Glyph::Rename.image(ui.ctx(), 18.0).tint(ink).paint_at(
                        ui,
                        Rect::from_center_size(rename.rect.center(), Vec2::splat(18.0)),
                    );
                    rename.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            "Rename project style",
                        )
                    });
                    if rename.on_hover_text("Rename project style").clicked() {
                        self.open_style_rename(port, style);
                        ui.close();
                    }
                    let delete = ui.add_sized(
                        [action_width, height],
                        buttons::Button::new("")
                            .id(row_id(&style.id, "delete"))
                            .bar(),
                    );
                    let (_, ink) = buttons::colors(ui, &delete, buttons::ButtonStyle::Quiet);
                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(delete.rect.center(), Vec2::splat(16.0)),
                        icons::Icon::Delete,
                        ink,
                    );
                    delete.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            "Delete project style",
                        )
                    });
                    if delete.on_hover_text("Delete project style").clicked() {
                        self.open_style_delete(port, &style.id);
                        if self.layer_palette.active_style.as_ref() == Some(&style.id) {
                            self.layer_palette.active_style = None;
                        }
                    }
                });
            }
        });
    }

    pub(crate) fn style_rename_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let Some(mut edit) = self.style_rename.take() else {
            return;
        };
        if port.editor().document().project()["projectId"].as_str() != Some(&edit.project) {
            return;
        }
        let styles = if edit.viewer {
            port.style_viewer()
                .map(|state| state.styles.clone())
                .unwrap_or_default()
        } else {
            port.icon_styles()
        };
        let creating = edit.recipe.is_some();
        let exists = creating || styles.iter().any(|style| style.id == edit.style);
        let header_glyph = if creating { Glyph::Save } else { Glyph::Rename };
        let mut approve = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("rename-project-style"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .fill(Color32::BLACK)
                    .inner_margin(0)
                    .corner_radius(0),
            )
            .show(ctx, |ui| {
                let actions = [
                    (
                        if creating { "Save" } else { "Approve" },
                        if creating {
                            Glyph::Save
                        } else {
                            Glyph::Approve
                        },
                    ),
                    ("Cancel", Glyph::Cancel),
                ];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let natural = actions.map(|(label, _)| {
                    ui.painter()
                        .layout_no_wrap(label.into(), font.clone(), ui.visuals().text_color())
                        .size()
                        .x
                        + 28.0
                        + 2.0 * ui.spacing().button_padding.x
                });
                let total: f32 = natural.iter().sum();
                let width = 468.0_f32
                    .max(total)
                    .min((ctx.content_rect().width() - 32.0).max(1.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let title = ui.painter().layout_no_wrap(
                    localization::text(
                        ctx,
                        if creating {
                            "Save project style"
                        } else {
                            "Rename style"
                        },
                    ),
                    egui::TextStyle::Heading.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
                let (header, _) = ui.allocate_exact_size(
                    Vec2::new(width, title.size().y.max(24.0) + 24.0),
                    Sense::hover(),
                );
                header_glyph
                    .image(ctx, 24.0)
                    .tint(ui.visuals().text_color())
                    .paint_at(
                        ui,
                        Rect::from_center_size(
                            egui::pos2(header.left() + 36.0, header.center().y),
                            Vec2::splat(24.0),
                        ),
                    );
                ui.painter().galley(
                    header.center() - title.size() * 0.5,
                    title,
                    ui.visuals().text_color(),
                );
                divider(ui);
                let action_height = buttons::bar_height(ui, 20.0);
                let body_height =
                    (ctx.content_rect().height() - header.height() - action_height - 76.0)
                        .max(40.0);
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 20))
                    .show(ui, |ui| {
                        ui.set_width((width - 48.0).max(1.0));
                        egui::ScrollArea::vertical()
                            .id_salt("rename-style-body")
                            .max_height(body_height)
                            .show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.add(
                                        egui::Label::new(if creating {
                                            "Save the current icon as a named project style."
                                        } else {
                                            "Choose a name for this style."
                                        })
                                        .halign(egui::Align::Center),
                                    );
                                    ui.add_space(12.0);
                                    let edge = ui
                                        .available_width()
                                        .min(256.0)
                                        .min((body_height - 110.0).max(64.0));
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(edge), Sense::hover());
                                    let preview = if creating {
                                        if let Some(preview) = port.preview(&edit.achievement) {
                                            self.preview_texture(ctx, &edit.achievement, preview);
                                        }
                                        self.textures
                                            .get(&edit.achievement)
                                            .map(|(_, texture, _)| {
                                                (texture.id(), texture.size_vec2())
                                            })
                                            .or_else(|| {
                                                let recipe = edit.recipe.as_ref()?;
                                                let style = styles.iter().find(|style| {
                                                    achimeld_application::style_signature(
                                                        &style.recipe,
                                                    ) == achimeld_application::style_signature(
                                                        recipe,
                                                    )
                                                })?;
                                                self.saved_style_texture(
                                                    ui,
                                                    port,
                                                    &style.id,
                                                    &edit.achievement,
                                                    512,
                                                )
                                            })
                                    } else if edit.viewer {
                                        if let Some(preview) =
                                            port.style_viewer_style_preview(&edit.style, 512)
                                        {
                                            self.preview_texture(
                                                ctx,
                                                &format!("viewer-style:{}", edit.style),
                                                preview,
                                            );
                                        }
                                        self.textures
                                            .get(&format!("viewer-style:{}", edit.style))
                                            .map(|(_, texture, _)| {
                                                (texture.id(), texture.size_vec2())
                                            })
                                    } else {
                                        self.saved_style_texture(
                                            ui,
                                            port,
                                            &edit.style,
                                            &edit.achievement,
                                            512,
                                        )
                                    };
                                    if let Some((texture, size)) = preview {
                                        ui.painter().image(
                                            texture,
                                            fit(size, rect),
                                            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().text(
                                            rect.center(),
                                            Align2::CENTER_CENTER,
                                            "Loading style preview…",
                                            egui::TextStyle::Body.resolve(ui.style()),
                                            ui.visuals().text_color(),
                                        );
                                    }
                                    ui.add_space(16.0);
                                    crate::fields::row(ui, "Name", |ui| {
                                        let field = ui.add(
                                            egui::TextEdit::singleline(&mut edit.name)
                                                .id(egui::Id::new("rename-style-name"))
                                                .desired_width(ui.available_width()),
                                        );
                                        if edit.focus {
                                            field.request_focus();
                                            edit.focus = false;
                                        }
                                        if field.changed() {
                                            edit.error = None;
                                        }
                                    });
                                    let validation = if !exists {
                                        Some("This project style no longer exists.")
                                    } else if edit.name.trim().is_empty() {
                                        Some("Enter a style name.")
                                    } else if edit.name.trim().len() > 256 {
                                        Some("This name is too long.")
                                    } else {
                                        edit.error.as_deref()
                                    };
                                    if let Some(error) = validation {
                                        ui.add_space(8.0);
                                        ui.add(egui::Label::new(error).halign(egui::Align::Center));
                                    }
                                });
                            });
                    });
                divider(ui);
                ui.allocate_ui_with_layout(
                    Vec2::new(width, action_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let start = ui.cursor().left();
                        let split = ((start + width * natural[0] / total) * ctx.pixels_per_point())
                            .round()
                            / ctx.pixels_per_point();
                        for (index, (label, glyph)) in actions.into_iter().enumerate() {
                            let right = if index == 0 { split } else { start + width };
                            let enabled = index == 1
                                || (exists
                                    && !edit.name.trim().is_empty()
                                    && edit.name.trim().len() <= 256);
                            if ui
                                .add_enabled(
                                    enabled,
                                    buttons::Button::new((glyph.image(ctx, 20.0), label))
                                        .id(egui::Id::new(("rename-style-action", index)))
                                        .bar()
                                        .min_size(Vec2::new(
                                            right - ui.cursor().left(),
                                            action_height,
                                        ))
                                        .icon_spacing(8.0)
                                        .image_tint_follows_text_color(true),
                                )
                                .clicked()
                            {
                                if index == 0 {
                                    approve = true;
                                } else {
                                    cancel = true;
                                }
                            }
                        }
                    },
                );
            });
        if cancel || response.should_close() {
            return;
        }
        if approve {
            if let Some(recipe) = edit.recipe.clone() {
                let style = IconStyle {
                    id: edit.style.clone(),
                    name: edit.name.trim().into(),
                    recipe,
                    preview_png: None,
                };
                let result = port
                    .editor()
                    .document()
                    .entity(EntityKind::Achievement, &edit.achievement)
                    .cloned()
                    .ok_or_else(|| "This icon no longer exists.".to_owned())
                    .and_then(|mut entity| {
                        entity["icon"] = style.recipe.clone();
                        if achimeld_application::style_achievements::is_style_achievement(&entity) {
                            entity["iconStyleId"] = json!(style.id);
                        }
                        port.finish_group();
                        port.execute(
                            "Save project icon style",
                            Command::Batch(vec![
                                Command::SaveIconStyle(style),
                                Command::ReplaceEntity {
                                    kind: EntityKind::Achievement,
                                    id: edit.achievement.clone(),
                                    value: entity,
                                },
                            ]),
                            None,
                        )
                    });
                match result {
                    Ok(()) => {
                        self.remember_icon_style(port, &edit.achievement, &edit.style);
                        self.entity_drafts.remove(&edit.achievement);
                        if port
                            .editor()
                            .document()
                            .entity(EntityKind::Achievement, &edit.achievement)
                            .is_some_and(|entity| {
                                !achimeld_application::style_achievements::is_local_icon(entity)
                            })
                        {
                            let was_editing_style = self.layer_palette.active_style.is_some();
                            self.set_style_edit_mode(
                                port,
                                was_editing_style.then(|| edit.style.clone()),
                            );
                            if !was_editing_style {
                                self.style_edit_prompt = Some(StyleEditPrompt {
                                    project: edit.project,
                                    achievement: edit.achievement,
                                    style: edit.style,
                                });
                                ctx.request_repaint();
                            }
                        }
                        return;
                    }
                    Err(error) => edit.error = Some(error),
                }
                self.style_rename = Some(edit);
                return;
            }
            if edit.viewer {
                port.request(achimeld_application::UiRequest::StyleViewer(
                    achimeld_application::style_viewer::StyleViewerAction::RenameStyle {
                        id: edit.style,
                        name: edit.name,
                    },
                ));
                return;
            }
            match port.rename_icon_style(&edit.style, &edit.name) {
                Ok(()) => return,
                Err(error) => edit.error = Some(error),
            }
        }
        self.style_rename = Some(edit);
    }
}
