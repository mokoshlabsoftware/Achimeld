use crate::Desktop;
use crate::buttons::UiButtonExt as _;
use achimeld_application::{EditorPort, EntityKind, UiRequest};

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
        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(crate::localization::text(ui.ctx(), "Synchronize"));
                self.sync_game_banner(ui, port);
                let project = port.editor().document().project();
                ui.label(format!("{} · Steam App ID {}", project["displayName"].as_str().unwrap_or("Project"), project["steamAppId"].as_u64().unwrap_or(0)));
                ui.label("Existing Steam stats and achievements are imported into the project automatically when you validate.");
                ui.weak("Uploads save to the Steamworks draft. Publish manually in Steamworks.");
                ui.checkbox(&mut self.terminal.show_during_sync, "Show Terminal during validation and synchronization");
                crate::buttons::bar(ui, 24.0, |ui| {
                    if ui.add_enabled(!port.synchronizing() && !port.reading(),
                        crate::buttons::Button::new((crate::style_picker::Glyph::Rename.image(ui.ctx(), 20.0), "Edit Steam App ID…"))
                            .bar().id(egui::Id::new("sync-edit-app-id")).wants_click(self.sync_app_id_wants_click(port)).icon_spacing(5.0).image_tint_follows_text_color(true)).clicked() {
                        self.open_sync_app_id(port);
                    }
                });
                ui.add_space(8.0);
                if let Some(error) = port.synchronization_error() {
                    ui.colored_label(crate::theme::color(port.preferences(), "Error"), error);
                }
                if let Some(review) = port.synchronization_review() {
                    let token = review["token"].as_str().unwrap_or("");
                    let rows = review["rows"].as_array().map(Vec::as_slice).unwrap_or_default();
                    if self.sync_token != token {
                        self.sync_confirmation = None;
                        self.sync_token = token.into();
                        self.sync_filter.clear(); self.sync_page = 0;
                        self.sync_choices = rows.iter().filter_map(|row| Some((row["key"].as_str()?.into(), row["defaultAction"].as_str()?.into()))).collect();
                    }
                    let achievements = review["steamAchievements"].as_u64().unwrap_or(0);
                    ui.strong(format!("Steam entries are in the project · {} stats · {achievements} {}", review["steamStats"].as_u64().unwrap_or(0), if achievements == 1 { "achievement" } else { "achievements" }));
                    if let Some(identity) = self.sync_review_identity(port) {
                        ui.strong(format!("Validated destination · Steam App ID {}", identity.1));
                        ui.add(egui::Label::new(crate::synchronization_target::steamworks_destination(identity.1)).wrap());
                        ui.label("Steamworks configuration loaded successfully. Check the App ID and reviewed entries before synchronizing.");
                        let mut confirmed = self.sync_destination_confirmed(port);
                        if ui.checkbox(&mut confirmed, "I confirm this is the correct Steam App ID.").changed() {
                            self.sync_confirmation = confirmed.then_some(identity);
                        }
                    } else {
                        ui.colored_label(crate::theme::color(port.preferences(), "Warning"), "This validation belongs to another Steam App ID. Cancel and validate again.");
                    }
                    if review["reconciliation"] == true {
                        ui.colored_label(crate::theme::color(port.preferences(), "Warning"), "The previous synchronization was interrupted. This plan uses a fresh Steam read; review the remaining differences.");
                    }
                    ui.separator();
                    if ui.add(egui::TextEdit::singleline(&mut self.sync_filter).hint_text("Filter by API name")).changed() { self.sync_page = 0; }
                    let query = self.sync_filter.to_lowercase();
                    let visible: Vec<_> = rows.iter().filter(|row| row["state"] != "InSync" && row["apiName"].as_str().unwrap_or("").to_lowercase().contains(&query)).collect();
                    let pages = visible.len().div_ceil(50).max(1);
                    self.sync_page = self.sync_page.min(pages - 1);
                    if pages > 1 { ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(self.sync_page > 0, crate::buttons::Button::new("Previous")).clicked() { self.sync_page -= 1; }
                        ui.label(format!("Page {} of {pages}", self.sync_page + 1));
                        if ui.add_enabled(self.sync_page + 1 < pages, crate::buttons::Button::new("Next")).clicked() { self.sync_page += 1; }
                    }); }
                    for row in visible.into_iter().skip(self.sync_page * 50).take(50) {
                        let key = row["key"].as_str().unwrap_or("");
                        ui.push_id(key, |ui| {
                            egui::Frame::new().fill(crate::theme::color(port.preferences(), "Panel")).inner_margin(10).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let mut imported = false;
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(format!("{} · {}", row["kind"].as_str().unwrap_or(""), row["apiName"].as_str().unwrap_or("")));
                                    ui.weak(match row["state"].as_str().unwrap_or("") {
                                        "LocalOnly" => "Only in project", "SteamOnly" => "Only on Steam", "LocalChanged" => "Project changed", "SteamChanged" => "Steam changed",
                                        "Conflict" => "Review both versions", "InSync" => "Up to date", "ArtworkUnavailable" => "Artwork unavailable", "RenamedOnSteam" => "Renamed on Steam · import keeps the old entry as Retired", value => value,
                                    });
                                    let choice = self.sync_choices.entry(key.into()).or_insert_with(|| "Skip".into());
                                    for action in row["actions"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                                        if ui.add(crate::buttons::Button::selectable(choice == action, action)).clicked() { *choice = action.into(); imported = action == "Import"; }
                                    }
                                });
                                if imported { self.sync_choices.insert(format!("Icon:{}", row["apiName"].as_str().unwrap_or("")), "Skip".into()); }
                                if row["kind"] == "Icon" && row["hasArtwork"] == true && ui.is_rect_visible(egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 100.0))) {
                                    let api = row["apiName"].as_str().unwrap_or("");
                                    ui.horizontal_wrapped(|ui| {
                                        for (locked, label) in [(false, "Achieved"), (true, "Locked")] {
                                            ui.vertical(|ui| {
                                                ui.weak(label);
                                                if let Some(preview) = port.synchronization_icon_preview(api, locked) {
                                                    let size = egui::vec2(preview.width as f32, preview.height as f32);
                                                    let size = size * (64.0 / size.max_elem().max(1.0)).min(1.0);
                                                    if let Some(texture) = self.preview_texture(ui.ctx(), &format!("sync:{token}:{api}:{locked}"), preview) { ui.image((texture, size)); }
                                                } else {
                                                    ui.spinner();
                                                    self.request(UiRequest::PreviewSynchronizationIcons(api.into()));
                                                }
                                            });
                                        }
                                    });
                                }
                                if row["state"] == "ArtworkUnavailable" { ui.weak("Artwork could not be generated. Repair its source in the Art Data Bank before uploading icons."); }
                                if let Some(error) = row["uploadError"].as_str() { ui.colored_label(crate::theme::color(port.preferences(), "Warning"), error); }
                                let fields = row["fields"].as_array().map(Vec::as_slice).unwrap_or_default();
                                if !fields.is_empty() {
                                    egui::CollapsingHeader::new(format!("{} field {}", fields.len(), if fields.len() == 1 { "difference" } else { "differences" })).show(ui, |ui| {
                                        for field in fields {
                                            ui.label(field["field"].as_str().unwrap_or(""));
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(format!("Project: {}", field["local"].as_str().unwrap_or("")));
                                                ui.label(format!("Steam: {}", field["steam"].as_str().unwrap_or("")));
                                            });
                                        }
                                    });
                                }
                            });
                        });
                        ui.add_space(4.0);
                    }
                    let selected = self.sync_selected_count(port);
                    crate::buttons::bar(ui, 24.0, |ui| {
                        if ui.add_enabled(self.sync_action_enabled(port), crate::buttons::Button::new(format!("Synchronize {selected} selected {}", if selected == 1 { "change" } else { "changes" })).style(crate::buttons::ButtonStyle::Accent).bar()).clicked() {
                            self.run_sync_action(port);
                        }
                        if ui.add(crate::buttons::Button::new("Cancel").bar()).clicked() { self.sync_confirmation = None; self.request(UiRequest::CancelSynchronization); }
                    });
                } else if port.synchronizing() {
                    ui.horizontal_wrapped(|ui| {
                        ui.spinner(); ui.label(port.status());
                        if ui.app_button("Stop synchronization").clicked() { self.request(UiRequest::CancelSynchronization); }
                    });
                } else {
                    self.sync_confirmation = None;
                    let valid_id = achimeld_core::SteamAppId::new(port.editor().document().app_id()).is_ok();
                    if !valid_id { ui.weak("Set a valid Steam App ID before validating or synchronizing."); }
                    else if port.synchronization_verified() { ui.strong("Validated and synchronized · no pending changes."); }
                    else if self.sync_destination_validated(port) { ui.strong("Steam App ID validated"); ui.weak("Synchronize to read the latest Steamworks configuration and review changes."); }
                    else { ui.weak("Validate to read the Steamworks destination and review its configuration before any changes are applied."); }
                    crate::buttons::bar(ui, 24.0, |ui| {
                        let action = if valid_id {
                            crate::buttons::Button::new((crate::style_picker::Glyph::Validate.image(ui.ctx(), 20.0), self.sync_action_label(port)))
                        } else {
                            crate::buttons::Button::new(self.sync_action_label(port))
                        };
                        if ui.add_enabled(self.sync_action_enabled(port), action.bar().id(egui::Id::new("sync-primary-action")).wants_click(self.sync_validate_wants_click(port)).icon_spacing(5.0).image_tint_follows_text_color(true)).clicked() { self.run_sync_action(port); }
                    });
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
                        } else if !port.synchronization_verified() {
                            ui.weak("Showing saved Steam data. Synchronize to refresh the comparison before uploading.");
                        }
                    }
                }
            });
        });
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
        let validated =
            self.sync_review_identity(port).is_some() || port.synchronization_verified();
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
                ui.image((texture, size));
            } else {
                ui.colored_label(crate::theme::color(port.preferences(), "Error"), "The saved game banner could not be loaded. Repair its image in the Art Data Bank.");
            }
        } else if validated {
            ui.colored_label(
                crate::theme::color(port.preferences(), "Error"),
                "The game banner could not be retrieved from Steam.",
            );
        }
    }
}
