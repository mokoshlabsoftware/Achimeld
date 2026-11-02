use crate::Desktop;
use achimeld_application::{EditorPort, EntityKind, SteamPage, UiRequest};

fn snapshot_time(stamp: &str) -> String {
    if let Some(utc) = stamp
        .strip_suffix('Z')
        .or_else(|| stamp.strip_suffix("+00:00"))
    {
        let seconds = utc.split_once('.').map_or(utc, |(seconds, _)| seconds);
        format!("{} UTC", seconds.replace('T', " "))
    } else {
        stamp.replace('T', " ")
    }
}

impl Desktop {
    pub(crate) fn synchronization_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        crate::buttons::bar_panel(ui, "sync-toolbar", 24.0).show(ui, |ui| {
            let height = ui.available_height();
            egui::ScrollArea::horizontal()
                .id_salt("synchronization-toolbar-scroll")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    crate::buttons::bar_with_height(ui, height, |ui| {
                        if ui
                            .add_enabled(
                                !port.synchronizing() && !port.reading(),
                                crate::buttons::Button::new((
                                    crate::style_picker::Glyph::Rename.image(ui.ctx(), 20.0),
                                    "Edit Steam App ID…",
                                ))
                                .bar()
                                .id(egui::Id::new("sync-edit-app-id"))
                                .wants_click(self.sync_app_id_wants_click(port))
                                .icon_spacing(5.0)
                                .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            self.open_sync_app_id(port);
                        }
                        if ui
                            .add_enabled(
                                !port.synchronizing()
                                    && !port.reading()
                                    && !port.browser_profile_busy(),
                                crate::buttons::Button::new((
                                    crate::style_picker::Glyph::Cancel.image(ui.ctx(), 20.0),
                                    "Forget validation",
                                ))
                                .bar()
                                .id(egui::Id::new("forget-steam-validation"))
                                .icon_spacing(5.0)
                                .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            self.request(UiRequest::ForgetSteamValidation);
                        }
                        self.browser_profile_actions(ui, port);
                    });
                });
        });
        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            let height = ui.available_height().max(100.0);
            egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
                ui.heading(crate::localization::text(ui.ctx(), "Synchronize"));
                self.sync_game_banner(ui, port);
                let project = port.editor().document().project();
                ui.label(format!("{} · Steam App ID {}", project["displayName"].as_str().unwrap_or("Project"), project["steamAppId"].as_u64().unwrap_or(0)));
                ui.label("Validate checks Steam sign-in and downloads the game banner. Synchronize prepares achievement artwork and imports existing Steam entries for review.");
                ui.weak("Uploads save to the Steamworks draft. Publish manually in Steamworks.");
                ui.checkbox(&mut self.terminal.show_during_sync, "Show Terminal during validation and synchronization");
                ui.add_space(8.0);
                if let Some(error) = port.synchronization_error() {
                    ui.colored_label(crate::theme::color(port.preferences(), "Error"), error);
                }
                if port.synchronization_review().is_some() {
                    ui.strong("The reviewed changes are ready in the modal window.");
                } else if port.synchronizing() {
                    ui.spinner();
                    ui.label(port.status());
                } else {
                    self.sync_confirmation = None;
                    let valid_id = achimeld_core::SteamAppId::new(port.editor().document().app_id()).is_ok();
                    if !valid_id { ui.weak("Set a valid Steam App ID before validating or synchronizing."); }
                    else if port.synchronization_verified() { ui.strong("Steam connection validated. Synchronize to prepare a fresh review."); }
                    else if self.steam_access(port) != crate::synchronization_target::SteamAccess::PreviousSession {
                        ui.weak("Validate to check Steam sign-in and access before preparing synchronization.");
                    }
                    let compact_actions = ui.available_width() < 450.0;
                    crate::buttons::bar(ui, 24.0, |ui| {
                        let action = if valid_id {
                            crate::buttons::Button::new((crate::style_picker::Glyph::Validate.image(ui.ctx(), 20.0), self.sync_action_label(port)))
                        } else {
                            crate::buttons::Button::new(self.sync_action_label(port))
                        };
                        if ui.add_enabled(self.sync_action_enabled(port), action.bar().primary_outline().id(egui::Id::new("sync-primary-action")).wants_click(self.sync_validate_wants_click(port)).icon_spacing(5.0).image_tint_follows_text_color(true)).clicked() { self.run_sync_action(port); }
                        if !compact_actions { self.steam_page_buttons(ui, port); }
                    });
                    if compact_actions {
                        crate::buttons::bar(ui, 24.0, |ui| self.steam_page_buttons(ui, port));
                    }
                    if port.reading() { ui.spinner(); ui.label(port.status()); }
                    let snapshot = &project["lastRemoteSnapshot"];
                    let snapshot_id = snapshot["steamAppId"].as_u64().or_else(|| snapshot.pointer("/appId/value").and_then(serde_json::Value::as_u64)).or_else(|| snapshot["appId"].as_u64());
                    let current_snapshot = valid_id && snapshot_id == Some(u64::from(port.editor().document().app_id()));
                    egui::Grid::new("synchronization-counts").show(ui, |ui| {
                        ui.label(""); ui.strong("Project"); ui.strong("Saved Steam"); ui.end_row();
                        for (kind, name) in [(EntityKind::Achievement, "Achievements"), (EntityKind::Stat, "Stats")] {
                            ui.label(name); ui.label(port.editor().document().steam_entities(kind).count().to_string());
                            ui.label(if current_snapshot { snapshot[kind.key()].as_array().map_or_else(|| "Not loaded".into(), |rows| rows.len().to_string()) } else { "Not loaded".into() }); ui.end_row();
                        }
                    });
                    if current_snapshot {
                        ui.strong("Saved Steam snapshot");
                        if let Some(stamp) = project["lastVerifiedAtUtc"].as_str()
                            .filter(|stamp| !stamp.is_empty()) {
                            ui.label(format!("Last successful check: {}", snapshot_time(stamp)));
                        } else if let Some(stamp) = snapshot["readAtUtc"].as_str()
                            .filter(|stamp| !stamp.is_empty()) {
                            ui.label(format!("Snapshot read at: {}", snapshot_time(stamp)));
                        } else {
                            ui.weak("The check time was not recorded in this project.");
                        }
                        if port.synchronization_error().is_some() {
                            ui.weak("Latest validation or synchronization failed; showing previously saved Steam data.");
                        } else if port.synchronizing() || port.reading() {
                            ui.weak("Checking Steam; showing previously saved Steam data until the check completes.");
                        } else if self.steam_access(port)
                            == crate::synchronization_target::SteamAccess::PreviousSession
                        {
                            ui.weak("Showing saved Steam data. Re-validate to confirm the internet connection and the Steam partner connection before synchronizing.");
                        } else if !port.synchronization_verified() {
                            ui.weak("Showing saved Steam data. Synchronize to refresh the comparison before uploading.");
                        }
                    }
                }
            });
        });
    }

    fn steam_page_buttons(&mut self, ui: &mut egui::Ui, port: &dyn EditorPort) {
        let enabled = achimeld_core::SteamAppId::new(port.editor().document().app_id()).is_ok()
            && !port.reading()
            && !port.synchronizing()
            && !port.browser_profile_busy();
        for (label, page) in [
            ("Open achievements", SteamPage::Achievements),
            ("Open stats", SteamPage::Stats),
        ] {
            if ui
                .add_enabled(enabled, crate::buttons::Button::new(label).bar())
                .clicked()
            {
                self.request(UiRequest::OpenSteamPage(page));
            }
        }
    }

    fn sync_game_banner(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let project = port.editor().document().project();
        let app_id = port.editor().document().app_id();
        let banner = project["assets"]["images"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| entry["steamBannerAppId"].as_u64() == Some(u64::from(app_id)))
            .and_then(|entry| entry["id"].as_str())
            .map(str::to_owned);
        let access = self.steam_access(port);
        let previous = access == crate::synchronization_target::SteamAccess::PreviousSession;
        let validated = access == crate::synchronization_target::SteamAccess::CurrentSession;
        let unavailable = validated
            && port
                .synchronization_review()
                .is_some_and(|review| review["gameBannerAvailable"] == false);
        if let Some(id) = banner.filter(|_| !unavailable) {
            let key = format!("sync-banner:{id}");
            let texture = if let Some(preview) = port.asset_preview(false, Some(&id)) {
                self.preview_texture(ui.ctx(), &key, preview)
            } else {
                self.cached_preview_texture(&key)
            };
            if let Some(texture) = texture {
                let size = egui::vec2(462.0, 174.0) * (ui.available_width() / 462.0).min(1.0);
                let tint = if previous {
                    crate::synchronization_target::PREVIOUS_SESSION_BANNER_TINT
                } else {
                    egui::Color32::WHITE
                };
                let response = ui.add(egui::Image::new((texture, size)).tint(tint));
                if previous {
                    paint_previous_session_label(ui, port, response.rect);
                }
            } else {
                ui.colored_label(crate::theme::color(port.preferences(), "Error"), "The saved game banner could not be loaded. Repair its image in the Art Data Bank.");
                if previous {
                    ui.colored_label(
                        crate::theme::color(port.preferences(), "Warning"),
                        "Validated in a previous session",
                    );
                }
            }
        } else if validated {
            ui.colored_label(
                crate::theme::color(port.preferences(), "Error"),
                "The game banner could not be retrieved from Steam.",
            );
        } else if previous {
            ui.colored_label(
                crate::theme::color(port.preferences(), "Warning"),
                "Validated in a previous session",
            );
        }
        if previous {
            ui.weak("Re-validate before synchronizing to confirm the internet connection and the connection to Steam partner.");
        }
    }
}

fn paint_previous_session_label(ui: &mut egui::Ui, port: &dyn EditorPort, banner: egui::Rect) {
    let color = crate::theme::color(port.preferences(), "Warning");
    let width = (banner.width() - 24.0).max(1.0);
    let galley = ui.painter().layout(
        "Validated in a previous session".into(),
        egui::TextStyle::Body.resolve(ui.style()),
        color,
        width,
    );
    let pos = egui::pos2(
        banner.center().x - galley.size().x * 0.5,
        banner.center().y - galley.size().y * 0.5,
    );
    let plate = egui::Rect::from_min_size(pos, galley.size()).expand2(egui::vec2(10.0, 6.0));
    ui.painter()
        .rect_filled(plate, 0.0, egui::Color32::from_black_alpha(176));
    ui.painter().galley(pos, galley, color);
}
