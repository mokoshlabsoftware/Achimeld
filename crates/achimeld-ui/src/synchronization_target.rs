use crate::{Desktop, buttons, localization, style_picker::Glyph};
use achimeld_application::{Command, EditorPort, UiRequest};
use achimeld_core::SteamAppId;
use egui::{Align, Color32, Rect, Sense, Vec2};

pub(crate) struct AppIdEdit {
    pub(crate) text: String,
    revision: u64,
    focus: bool,
    error: Option<String>,
}

pub(crate) fn parse_app_id(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let id = text.parse().ok()?;
    SteamAppId::new(id).ok().map(SteamAppId::value)
}

pub(crate) fn steamworks_destination(app_id: u32) -> String {
    format!("https://partner.steamgames.com/apps/achievements/{app_id}")
}

impl Desktop {
    pub(crate) fn sync_review_identity(&self, port: &dyn EditorPort) -> Option<(String, u32, u64)> {
        let app_id = SteamAppId::new(port.editor().document().app_id())
            .ok()?
            .value();
        let review = port.synchronization_review()?;
        let token = review["token"].as_str().filter(|token| !token.is_empty())?;
        (review["appId"].as_u64() == Some(u64::from(app_id)))
            .then(|| (token.into(), app_id, port.editor().revision()))
    }

    pub(crate) fn sync_destination_confirmed(&self, port: &dyn EditorPort) -> bool {
        self.sync_confirmation.is_some()
            && self.sync_confirmation == self.sync_review_identity(port)
    }

    pub(crate) fn sync_selected_count(&self, port: &dyn EditorPort) -> usize {
        port.synchronization_review()
            .and_then(|review| review["rows"].as_array())
            .into_iter()
            .flatten()
            .filter(|row| {
                self.sync_choices
                    .get(row["key"].as_str().unwrap_or(""))
                    .is_some_and(|action| action != "Skip")
            })
            .count()
    }

    pub(crate) fn sync_action_label(&self, port: &dyn EditorPort) -> &'static str {
        if SteamAppId::new(port.editor().document().app_id()).is_err() {
            "App ID…"
        } else if port.synchronization_review().is_some() || port.synchronization_verified() {
            "Synchronize"
        } else {
            "Validate"
        }
    }

    pub(crate) fn sync_action_enabled(&self, port: &dyn EditorPort) -> bool {
        if self.sync_app_id_edit.is_some() {
            return false;
        }
        if port.synchronization_verified() && port.synchronization_review().is_none() {
            return false;
        }
        if port.synchronization_review().is_some() {
            self.sync_destination_confirmed(port) && self.sync_selected_count(port) > 0
        } else {
            !port.synchronizing() && !port.reading() && !port.art_busy() && !port.exporting()
        }
    }

    pub(crate) fn sync_wants_click(&self, port: &dyn EditorPort) -> bool {
        self.sync_review_identity(port).is_none()
            && !port.synchronization_verified()
            && !port.synchronizing()
            && !port.reading()
    }

    pub(crate) fn sync_app_id_wants_click(&self, port: &dyn EditorPort) -> bool {
        self.sync_wants_click(port) && SteamAppId::new(port.editor().document().app_id()).is_err()
    }

    pub(crate) fn sync_validate_wants_click(&self, port: &dyn EditorPort) -> bool {
        self.sync_wants_click(port) && SteamAppId::new(port.editor().document().app_id()).is_ok()
    }

    pub(crate) fn run_sync_action(&mut self, port: &dyn EditorPort) {
        if !self.sync_action_enabled(port) {
            return;
        }
        if SteamAppId::new(port.editor().document().app_id()).is_err() {
            self.open_sync_app_id(port);
        } else if let Some(review) = port.synchronization_review() {
            self.request(UiRequest::ApplySynchronization(
                review["rows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|row| {
                        let key = row["key"].as_str()?;
                        Some((key.into(), self.sync_choices.get(key)?.clone()))
                    })
                    .collect(),
            ));
        } else {
            self.sync_confirmation = None;
            achimeld_application::diagnostics::record(
                "INFO",
                "validation",
                "Validate selected · sending request to the application host",
            );
            self.request(UiRequest::PrepareSynchronization);
        }
    }

    pub(crate) fn open_sync_app_id(&mut self, port: &dyn EditorPort) {
        self.sync_app_id_edit = Some(AppIdEdit {
            text: match port.editor().document().app_id() {
                0 => String::new(),
                id => id.to_string(),
            },
            revision: port.editor().revision(),
            focus: true,
            error: None,
        });
    }

    pub(crate) fn sync_app_id_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let Some(mut edit) = self.sync_app_id_edit.take() else {
            return;
        };
        let available = !port.synchronizing() && !port.reading();
        let current = edit.revision == port.editor().revision();
        let mut save = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("sync-app-id-editor"))
            .frame(egui::Frame::new().fill(Color32::BLACK).corner_radius(0)
                .stroke(egui::Stroke::new(1.0, ctx.global_style().visuals.widgets.noninteractive.bg_stroke.color)))
            .show(ctx, |ui| {
                let actions = [("Save", Glyph::Save), ("Cancel", Glyph::Cancel)];
                let labels = actions.map(|(label, _)| localization::text(ctx, label));
                let font = egui::TextStyle::Button.resolve(ui.style());
                let natural = labels.each_ref().map(|label| ui.painter()
                    .layout_no_wrap(label.clone(), font.clone(), ui.visuals().text_color()).size().x
                    + 28.0 + 2.0 * ui.spacing().button_padding.x);
                let total: f32 = natural.iter().sum();
                let width = 468.0_f32.max(total).min((ctx.content_rect().width() - 32.0).max(1.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let title = ui.painter().layout_no_wrap(localization::text(ctx, "Edit Steam App ID"),
                    egui::TextStyle::Heading.resolve(ui.style()), ui.visuals().text_color());
                let (header, _) = ui.allocate_exact_size(Vec2::new(width, title.size().y.max(24.0) + 24.0), Sense::hover());
                Glyph::Rename.image(ctx, 24.0).tint(ui.visuals().text_color()).paint_at(ui,
                    Rect::from_center_size(egui::pos2(header.left() + 36.0, header.center().y), Vec2::splat(24.0)));
                ui.painter().galley(header.center() - title.size() / 2.0, title, ui.visuals().text_color());
                divider(ui);
                let height = buttons::bar_height(ui, 20.0);
                let body_height = (ctx.content_rect().height() - header.height() - height - 76.0).max(40.0);
                egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)).show(ui, |ui| {
                    ui.set_width((width - 48.0).max(1.0));
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 12.0);
                    egui::ScrollArea::vertical().id_salt("sync-app-id-body").max_height(body_height).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(Align::Center), |ui| {
                            ui.add(egui::Label::new("Enter the Steam App ID for the app you want to synchronize with. Validate the destination after saving.").wrap().halign(Align::Center));
                            ui.add(egui::Label::new("Changing the App ID clears the previous Steam comparison history and keeps your project entries and artwork.").wrap().halign(Align::Center));
                            crate::fields::row(ui, "Steam App ID", |ui| {
                                let field = ui.add(egui::TextEdit::singleline(&mut edit.text)
                                    .id(egui::Id::new("sync-app-id-field")).char_limit(32)
                                    .desired_width(ui.available_width()));
                                if edit.focus { field.request_focus(); edit.focus = false; }
                                if field.changed() { edit.error = None; }
                            });
                            let error = if !current { Some("The project changed. Cancel and reopen this editor.") }
                                else if !available { Some("Finish or cancel the current Steam operation first.") }
                                else if parse_app_id(&edit.text).is_none() { Some("Enter a whole-number Steam App ID from 1 to 4294967295.") }
                                else { edit.error.as_deref() };
                            if let Some(error) = error { ui.add(egui::Label::new(error).wrap().halign(Align::Center)); }
                        });
                    });
                });
                divider(ui);
                ui.allocate_ui_with_layout(Vec2::new(width, height), egui::Layout::left_to_right(Align::Center), |ui| {
                    let start = ui.cursor().left();
                    let split = ((start + width * natural[0] / total) * ctx.pixels_per_point()).round() / ctx.pixels_per_point();
                    for (index, (_, glyph)) in actions.into_iter().enumerate() {
                        let right = if index == 0 { split } else { start + width };
                        if ui.add_enabled(index == 1 || (parse_app_id(&edit.text).is_some() && current && available),
                            buttons::Button::new((glyph.image(ctx, 20.0), labels[index].clone())).bar()
                                .min_size(Vec2::new(right - ui.cursor().left(), height)).icon_spacing(8.0)
                                .image_tint_follows_text_color(true)).clicked() {
                            if index == 0 { save = true; } else { cancel = true; }
                        }
                    }
                });
            });
        if cancel || response.should_close() {
            return;
        }
        // Reparse after the field has consumed this frame's text input.
        if save
            && let Some(app_id) = parse_app_id(&edit.text)
            && current
            && available
        {
            let project = port.editor().document().project();
            match port.execute(
                "Change Steam App ID",
                Command::ProjectSettings {
                    name: project["displayName"].as_str().unwrap_or("").into(),
                    app_id,
                    atlas: None,
                    assets: None,
                },
                None,
            ) {
                Ok(()) => {
                    self.sync_confirmation = None;
                    return;
                }
                Err(error) => edit.error = Some(error),
            }
        }
        self.sync_app_id_edit = Some(edit);
    }
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}
