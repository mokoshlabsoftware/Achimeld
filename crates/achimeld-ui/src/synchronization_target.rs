use crate::{Desktop, buttons, localization, steam, style_picker::Glyph};
use achimeld_application::{Command, EditorPort, UiRequest};
use achimeld_core::SteamAppId;
use egui::{Align, Color32, Rect, Sense, Vec2};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(crate) struct AppIdEdit {
    pub(crate) text: String,
    pub(crate) project_name: String,
    revision: u64,
    focus: bool,
    error: Option<String>,
}

pub(crate) struct SyncWarning {
    token: String,
    choices: Vec<(String, String)>,
    delete_achievements: usize,
    delete_stats: usize,
    projected_achievements: u64,
    above_default_limit: bool,
}

pub(crate) struct ValidationFollowup {
    pub project_id: String,
    pub app_id: u32,
    pub saw_validation: bool,
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

pub(crate) fn sync_review_group(row: &serde_json::Value) -> u8 {
    if row["state"] == "InSync" {
        2
    } else if row["fields"]
        .as_array()
        .is_some_and(|fields| !fields.is_empty())
    {
        0
    } else {
        1
    }
}

/// How far the open project has confirmed access to its Steam partner destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SteamAccess {
    /// This App ID has no remembered partner connection.
    Unvalidated,
    /// A previous session connected. This session still needs a live check.
    PreviousSession,
    /// This session has a matching review or a completed verification.
    CurrentSession,
}

/// Half brightness so a remembered partner banner stays recognizable and reads as inactive.
pub(crate) const PREVIOUS_SESSION_BANNER_TINT: egui::Color32 =
    egui::Color32::from_rgb(128, 128, 128);

impl Desktop {
    pub(crate) fn steam_panel_available(&self, port: &dyn EditorPort) -> bool {
        self.steam_access(port) == SteamAccess::CurrentSession
            && port.synchronization_error().is_none()
    }

    pub(crate) fn browser_profile_actions(&mut self, ui: &mut egui::Ui, port: &dyn EditorPort) {
        for (label, glyph, request) in [
            (
                "Open profile folder",
                Glyph::Import,
                UiRequest::OpenBrowserProfile,
            ),
            (
                "Delete browser profile",
                Glyph::Delete,
                UiRequest::DeleteBrowserProfile,
            ),
        ] {
            if ui
                .add_enabled(
                    !port.synchronizing() && !port.reading() && !port.browser_profile_busy(),
                    buttons::Button::new((glyph.image(ui.ctx(), 20.0), label))
                        .bar()
                        .icon_spacing(5.0)
                        .image_tint_follows_text_color(true),
                )
                .clicked()
            {
                self.request(request);
            }
        }
    }

    pub(crate) fn steam_access(&self, port: &dyn EditorPort) -> SteamAccess {
        if self.sync_review_identity(port).is_some() || port.synchronization_verified() {
            SteamAccess::CurrentSession
        } else if self.sync_destination_validated(port) {
            SteamAccess::PreviousSession
        } else {
            SteamAccess::Unvalidated
        }
    }

    pub(crate) fn sync_destination_validated(&self, port: &dyn EditorPort) -> bool {
        let project = port.editor().document().project();
        let snapshot = &project["lastRemoteSnapshot"];
        let snapshot_id = snapshot["steamAppId"]
            .as_u64()
            .or_else(|| {
                snapshot
                    .pointer("/appId/value")
                    .and_then(serde_json::Value::as_u64)
            })
            .or_else(|| snapshot["appId"].as_u64());
        SteamAppId::new(port.editor().document().app_id()).is_ok()
            && snapshot_id == Some(u64::from(port.editor().document().app_id()))
            && project["lastVerifiedAtUtc"]
                .as_str()
                .is_some_and(|stamp| !stamp.is_empty())
    }

    pub(crate) fn sync_review_identity(&self, port: &dyn EditorPort) -> Option<(String, u32)> {
        let app_id = SteamAppId::new(port.editor().document().app_id())
            .ok()?
            .value();
        let review = port.synchronization_review()?;
        let token = review["token"].as_str().filter(|token| !token.is_empty())?;
        (review["appId"].as_u64() == Some(u64::from(app_id))).then(|| (token.into(), app_id))
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
        } else if self.steam_access(port) == SteamAccess::PreviousSession {
            "Re-validate"
        } else {
            "Validate"
        }
    }

    pub(crate) fn sync_action_enabled(&self, port: &dyn EditorPort) -> bool {
        if self.sync_app_id_edit.is_some() {
            return false;
        }
        if port.synchronization_review().is_some() {
            self.sync_destination_confirmed(port) && self.sync_selected_count(port) > 0
        } else {
            !port.synchronizing()
                && !port.reading()
                && !port.browser_profile_busy()
                && !port.art_busy()
                && !port.exporting()
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
        } else if let Some(token) = port
            .synchronization_review()
            .and_then(|review| review["token"].as_str())
            .filter(|token| !token.is_empty())
        {
            self.steam_delete_review = false;
            self.sync_plan_token = Some(token.to_owned());
            // The review modal owns the five-second countdown and explicit
            // Stop action before the frozen choices reach the host.
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

    pub(crate) fn validate_and_synchronize(&mut self, port: &dyn EditorPort) {
        if !self.sync_action_enabled(port)
            || port.synchronization_verified()
            || port.synchronization_review().is_some()
            || SteamAppId::new(port.editor().document().app_id()).is_err()
        {
            return;
        }
        self.validation_followup = Some(ValidationFollowup {
            project_id: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
            app_id: port.editor().document().app_id(),
            saw_validation: false,
        });
        self.run_sync_action(port);
    }

    pub(crate) fn continue_validation_followup(&mut self, port: &dyn EditorPort) {
        let Some(mut followup) = self.validation_followup.take() else {
            return;
        };
        if port.editor().document().project()["projectId"].as_str()
            != Some(followup.project_id.as_str())
            || port.editor().document().app_id() != followup.app_id
            || port.synchronization_error().is_some()
        {
            return;
        }
        if port.synchronizing() {
            followup.saw_validation = true;
            self.validation_followup = Some(followup);
        } else if followup.saw_validation && port.synchronization_verified() {
            self.sync_confirmation = None;
            self.request(UiRequest::PrepareSynchronization);
        } else if !followup.saw_validation {
            self.validation_followup = Some(followup);
        }
    }

    pub(crate) fn confirm_sync_plan(&mut self, port: &dyn EditorPort) {
        let Some(review) = port.synchronization_review() else {
            self.sync_plan_token = None;
            return;
        };
        if self.sync_plan_token.as_deref() != review["token"].as_str()
            || !self.sync_action_enabled(port)
        {
            self.sync_plan_token = None;
            return;
        }
        let choices: Vec<(String, String)> = review["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| {
                let key = row["key"].as_str()?;
                Some((key.into(), self.sync_choices.get(key)?.clone()))
            })
            .collect();
        let (delete_achievements, delete_stats, projected_achievements) =
            self.sync_warning_counts(review);
        let above_default_limit = projected_achievements > 100;
        if delete_achievements + delete_stats > 0 || above_default_limit {
            self.sync_review_started = None;
            self.sync_review_paused = true;
            self.sync_warning = Some(SyncWarning {
                token: review["token"].as_str().unwrap_or("").into(),
                choices,
                delete_achievements,
                delete_stats,
                projected_achievements,
                above_default_limit,
            });
            return;
        }
        self.sync_plan_token = None;
        self.request(UiRequest::ApplySynchronization {
            choices,
            confirm_deletion: false,
            allow_above_100: false,
        });
    }

    fn sync_warning_counts(&self, review: &serde_json::Value) -> (usize, usize, u64) {
        let mut deleted_achievements = 0;
        let mut deleted_stats = 0;
        let mut created_achievements = 0;
        for row in review["rows"].as_array().into_iter().flatten() {
            let key = row["key"].as_str().unwrap_or("");
            let action = self
                .sync_choices
                .get(key)
                .map(String::as_str)
                .unwrap_or("Skip");
            if action == "Delete" {
                if row["kind"] == "Achievement" {
                    deleted_achievements += 1;
                }
                if row["kind"] == "Stat" {
                    deleted_stats += 1;
                }
            } else if action == "Upload"
                && row["kind"] == "Achievement"
                && row["state"] == "LocalOnly"
            {
                created_achievements += 1;
            }
        }
        let remote = review["remoteAchievementCount"].as_u64().unwrap_or(0);
        (
            deleted_achievements,
            deleted_stats,
            remote.saturating_sub(deleted_achievements as u64) + created_achievements,
        )
    }

    pub(crate) fn planned_sync_lines(
        &self,
        port: &dyn EditorPort,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let rows = port
            .synchronization_review()
            .and_then(|review| review["rows"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let choice = |key: &str| {
            self.sync_choices
                .get(key)
                .map(String::as_str)
                .unwrap_or("Skip")
        };
        let mut creations = Vec::new();
        let mut created = BTreeSet::new();
        for row in rows {
            let key = row["key"].as_str().unwrap_or("");
            if row["kind"] == "Achievement"
                && row["state"] == "LocalOnly"
                && choice(key) == "Upload"
            {
                let api = row["apiName"].as_str().unwrap_or("");
                let icon = choice(&format!("Icon:{api}")) == "Upload";
                creations.push(if icon {
                    format!("Create · {api} · icon included")
                } else {
                    format!("Create · {api}")
                });
                created.insert(api.to_owned());
            }
        }
        let deletions = rows
            .iter()
            .filter(|row| choice(row["key"].as_str().unwrap_or("")) == "Delete")
            .map(|row| {
                format!(
                    "Delete · {} · {}",
                    row["kind"].as_str().unwrap_or(""),
                    row["apiName"].as_str().unwrap_or("")
                )
            })
            .collect();
        let mut other = Vec::new();
        for row in rows {
            let key = row["key"].as_str().unwrap_or("");
            let action = choice(key);
            if action == "Skip" || action == "Delete" {
                continue;
            }
            let api = row["apiName"].as_str().unwrap_or("");
            let kind = row["kind"].as_str().unwrap_or("");
            if created.contains(api)
                && action == "Upload"
                && (kind == "Achievement" || kind == "Icon")
            {
                continue;
            }
            other.push(format!("{action} · {kind} · {api}"));
        }
        (creations, deletions, other)
    }

    pub(crate) fn sync_plan_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let Some(review) = port.synchronization_review() else {
            self.sync_plan_token = None;
            self.sync_review_started = None;
            return;
        };
        let Some((token, app_id)) = self.sync_review_identity(port) else {
            self.sync_plan_token = review["token"].as_str().map(str::to_owned);
            self.sync_review_started = None;
            let choice = steam::review_prompt(
                ctx,
                "sync-plan-destination-changed",
                "Review synchronization",
                steam::glyph_image(
                    ctx,
                    "tab-sync",
                    include_bytes!("../assets/toolbar/tab-sync.rgba"),
                    24.0,
                ),
                &[steam::ReviewAction {
                    colors: None,
                    enabled: true,
                    label: "Cancel review",
                    icon: Glyph::Cancel.image(ctx, 20.0),
                }],
                |ui| {
                    ui.label("The Steam App ID changed after validation. Cancel this review and validate the destination again.");
                },
            );
            if matches!(
                choice,
                steam::ReviewChoice::Action(_) | steam::ReviewChoice::Dismissed
            ) {
                self.sync_plan_token = None;
                self.sync_confirmation = None;
                self.request(UiRequest::CancelSynchronization);
            }
            return;
        };
        if self.error.is_some() || self.about || self.sync_app_id_edit.is_some() {
            self.sync_review_started = None;
            self.sync_review_elapsed = Duration::ZERO;
            return;
        }
        let rows = review["rows"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let differences = rows.iter().filter(|row| row["state"] != "InSync").count();
        let achievement_differences = rows
            .iter()
            .filter(|row| row["kind"] == "Achievement" && row["state"] != "InSync")
            .count();
        let stat_differences = rows
            .iter()
            .filter(|row| row["kind"] == "Stat" && row["state"] != "InSync")
            .count();
        let icon_differences = rows
            .iter()
            .filter(|row| row["kind"] == "Icon" && row["state"] != "InSync")
            .count();
        if self.sync_token != token {
            self.sync_token = token.clone();
            self.sync_warning = None;
            self.sync_plan_token = Some(token.clone());
            self.sync_confirmation = Some((token.clone(), app_id));
            self.sync_filter.clear();
            self.sync_choices = rows
                .iter()
                .filter_map(|row| {
                    Some((
                        row["key"].as_str()?.into(),
                        row["defaultAction"].as_str()?.into(),
                    ))
                })
                .collect();
            for row in rows {
                let kind = match row["kind"].as_str() {
                    Some("Achievement") => Some(achimeld_application::EntityKind::Achievement),
                    Some("Stat") => Some(achimeld_application::EntityKind::Stat),
                    _ => None,
                };
                if row["state"] == "SteamOnly"
                    && row["actions"]
                        .as_array()
                        .is_some_and(|actions| actions.iter().any(|action| action == "Delete"))
                    && kind
                        .zip(row["apiName"].as_str())
                        .is_some_and(|(kind, api)| {
                            self.steam_marks.contains(&(kind, api.to_owned()))
                        })
                    && let Some(key) = row["key"].as_str()
                {
                    self.sync_choices.insert(key.into(), "Delete".into());
                }
            }
            if review["reconciliation"] == true {
                for row in rows {
                    if row["state"] != "InSync"
                        && row["actions"]
                            .as_array()
                            .is_some_and(|actions| actions.iter().any(|action| action == "Upload"))
                        && let Some(key) = row["key"].as_str()
                    {
                        self.sync_choices.insert(key.into(), "Upload".into());
                    }
                }
            }
            self.sync_review_elapsed = Duration::ZERO;
            self.sync_review_paused = review["reconciliation"] == true;
            self.sync_review_started =
                (!self.sync_review_paused && self.sync_selected_count(port) > 0).then(Instant::now);
        }
        if let Some(warning) = self.sync_warning.take() {
            let same_choices = warning.token == token
                && warning
                    .choices
                    .iter()
                    .all(|(key, action)| self.sync_choices.get(key) == Some(action));
            if same_choices {
                let decision = steam::review_prompt(
                    ctx,
                    "sync-plan-warning",
                    "Confirm Steam draft changes",
                    if warning.delete_achievements + warning.delete_stats > 0 {
                        Glyph::Delete.image(ctx, 24.0)
                    } else {
                        Glyph::Validate.image(ctx, 24.0)
                    },
                    &[
                        steam::ReviewAction {
                            colors: None,
                            enabled: true,
                            label: "Continue",
                            icon: Glyph::Approve.image(ctx, 20.0),
                        },
                        steam::ReviewAction {
                            colors: None,
                            enabled: true,
                            label: "Back to review",
                            icon: Glyph::Cancel.image(ctx, 20.0),
                        },
                    ],
                    |ui| {
                        if warning.delete_achievements + warning.delete_stats > 0 {
                            ui.label(format!("Synchronization will delete {} Steam achievements and {} Steam stats before other changes. These draft deletions can affect players after you publish in Steamworks.",
                                warning.delete_achievements, warning.delete_stats));
                        }
                        if warning.above_default_limit {
                            ui.label(format!("The reviewed plan would leave {} Steam achievements. Apps that have not been vetted are limited to 100. Continue only if this app can exceed that limit.",
                                warning.projected_achievements));
                        }
                    },
                );
                match decision {
                    steam::ReviewChoice::Action(0) => {
                        self.sync_plan_token = None;
                        self.request(UiRequest::ApplySynchronization {
                            choices: warning.choices,
                            confirm_deletion: warning.delete_achievements + warning.delete_stats
                                > 0,
                            allow_above_100: warning.above_default_limit,
                        });
                    }
                    steam::ReviewChoice::Idle => self.sync_warning = Some(warning),
                    _ => {}
                }
            }
            return;
        }
        {
            let snapshot = &port.editor().document().project()["lastRemoteSnapshot"];
            self.retain_steam_marks(snapshot);
        }
        let (creations, deletions, other) = self.planned_sync_lines(port);
        self.sync_plan_token = Some(token);
        let selected = self.sync_selected_count(port);
        let (delete_achievements, delete_stats, projected_achievements) =
            self.sync_warning_counts(review);
        let requires_explicit =
            delete_achievements + delete_stats > 0 || projected_achievements > 100;
        if requires_explicit {
            self.sync_review_started = None;
            self.sync_review_paused = true;
        }
        if selected > 0 && !self.sync_review_paused && self.sync_review_started.is_none() {
            self.sync_review_started = Some(Instant::now());
        }
        let elapsed = self.sync_review_elapsed
            + self
                .sync_review_started
                .map_or(Duration::ZERO, |started| started.elapsed());
        let remaining = Duration::from_secs(5).saturating_sub(elapsed);
        let running =
            selected > 0 && !self.sync_review_paused && self.sync_review_started.is_some();
        let progress = (selected > 0).then(|| remaining.as_secs_f32() / 5.0);
        let mut changed = false;
        let choice = steam::review_prompt_wide(
            ctx,
            "sync-plan-review",
            "Review synchronization",
            steam::glyph_image(
                ctx,
                "tab-sync",
                include_bytes!("../assets/toolbar/tab-sync.rgba"),
                24.0,
            ),
            &[
                steam::ReviewAction {
                    colors: None,
                    enabled: !requires_explicit && (running || selected > 0),
                    label: if requires_explicit {
                        "Confirmation required"
                    } else if running {
                        "Stop"
                    } else {
                        "Start 5 s"
                    },
                    icon: Glyph::Cancel.image(ctx, 20.0),
                },
                steam::ReviewAction {
                    colors: None,
                    enabled: selected > 0 && self.sync_destination_confirmed(port),
                    label: "Synchronize now",
                    icon: steam::glyph_image(
                        ctx,
                        "tab-sync",
                        include_bytes!("../assets/toolbar/tab-sync.rgba"),
                        20.0,
                    ),
                },
                steam::ReviewAction {
                    colors: None,
                    enabled: true,
                    label: "Cancel",
                    icon: Glyph::Cancel.image(ctx, 20.0),
                },
            ],
            progress,
            |ui| {
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.corner_radius = egui::CornerRadius::ZERO;
                widgets.hovered.corner_radius = egui::CornerRadius::ZERO;
                widgets.active.corner_radius = egui::CornerRadius::ZERO;
                ui.label(format!("Validated destination: Steam App ID {app_id}"));
                ui.label(steamworks_destination(app_id));
                ui.strong(format!(
                    "Differences found: {differences} (achievements: {achievement_differences}, stats: {stat_differences}, icons: {icon_differences})"
                ));
                ui.label(format!(
                    "{selected} selected {}. {}",
                    if selected == 1 { "change" } else { "changes" },
                    if running {
                        format!(
                            "Synchronization starts in {:.1} s. Press Stop to edit the plan.",
                            remaining.as_secs_f32()
                        )
                    } else if requires_explicit {
                        "Select Synchronize now to review the deletion or achievement limit warning."
                            .into()
                    } else if self.sync_review_paused {
                        "Countdown stopped. Edit the plan, then start it again or synchronize now."
                            .into()
                    } else {
                        "Select changes to start the five-second countdown.".into()
                    }
                ));
                if review["reconciliation"] == true {
                    ui.colored_label(
                        crate::theme::color(port.preferences(), "Warning"),
                        "The previous synchronization was interrupted. A fresh Steam read prepared project uploads where available. Review the choices below before starting synchronization.",
                    );
                }
                ui.label("Artwork for this upload is cached. You can keep editing the project after synchronization starts.");
                if selected > 0 {
                    ui.strong(format!(
                        "Selected plan: new achievements: {}, deletions: {}, other changes: {}",
                        creations.len(),
                        deletions.len(),
                        other.len()
                    ));
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.sync_filter)
                        .hint_text("Filter by API name"),
                );
                let query = self.sync_filter.to_lowercase();
                let mut visible_rows = rows
                    .iter()
                    .filter(|row| {
                        row["apiName"]
                            .as_str()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                    })
                    .collect::<Vec<_>>();
                visible_rows.sort_by_key(|row| sync_review_group(row));
                let mut group = None;
                for row in visible_rows {
                    let next_group = sync_review_group(row);
                    if group != Some(next_group) {
                        ui.add_space(8.0);
                        ui.strong(match next_group {
                            0 => "Field differences",
                            1 => "Other differences",
                            _ => "Up to date",
                        });
                        group = Some(next_group);
                    }
                    let key = row["key"].as_str().unwrap_or("");
                    ui.push_id(key, |ui| {
                        egui::Frame::new()
                            .fill(crate::theme::color(port.preferences(), "Panel"))
                            .corner_radius(0)
                            .inner_margin(10)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let mut imported = false;
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(format!(
                                        "{} · {}",
                                        row["kind"].as_str().unwrap_or(""),
                                        row["apiName"].as_str().unwrap_or("")
                                    ));
                                    ui.add_space(8.0);
                                    ui.weak(match row["state"].as_str().unwrap_or("") {
                                        "LocalOnly" => "Only in project",
                                        "SteamOnly" => "Only on Steam",
                                        "LocalChanged" => "Project changed",
                                        "SteamChanged" => "Steam changed",
                                        "Conflict" => "Review both versions",
                                        "InSync" => "Up to date",
                                        "ArtworkUnavailable" => "Artwork unavailable",
                                        "RenamedOnSteam" => "Renamed on Steam · import keeps the old entry as Retired",
                                        value => value,
                                    });
                                    ui.add_space(8.0);
                                    let entry = self.sync_choices.entry(key.into()).or_insert_with(|| "Skip".into());
                                    if let Some(default) = row["defaultAction"]
                                        .as_str()
                                        .filter(|action| *action != "Skip")
                                    {
                                        let mut included = entry != "Skip";
                                        if ui.checkbox(&mut included, "Include").changed() {
                                            *entry = if included { default } else { "Skip" }.into();
                                            imported = included && default == "Import";
                                            changed = true;
                                        }
                                        ui.add_space(8.0);
                                    }
                                    for action in row["actions"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .filter_map(|value| value.as_str())
                                    {
                                        let label = match action {
                                            "Upload" => "Use project",
                                            "Skip" => "Leave unchanged",
                                            "Import" => "Use Steam",
                                            "Delete" => "Delete on Steam",
                                            _ => action,
                                        };
                                        if ui.add(buttons::Button::selectable(entry == action, label).bar()).clicked() {
                                            *entry = action.into();
                                            imported = action == "Import";
                                            changed = true;
                                        }
                                    }
                                });
                                if imported {
                                    self.sync_choices.insert(
                                        format!("Icon:{}", row["apiName"].as_str().unwrap_or("")),
                                        "Skip".into(),
                                    );
                                }
                                if row["state"] == "ArtworkUnavailable" {
                                    ui.weak("Artwork could not be generated. Repair its source in the Art Data Bank before uploading icons.");
                                }
                                if let Some(error) = row["uploadError"].as_str() {
                                    ui.colored_label(crate::theme::color(port.preferences(), "Warning"), error);
                                }
                                let fields = row["fields"].as_array().map(Vec::as_slice).unwrap_or_default();
                                if !fields.is_empty() {
                                    egui::CollapsingHeader::new(format!(
                                        "{} field {}",
                                        fields.len(),
                                        if fields.len() == 1 { "difference" } else { "differences" }
                                    ))
                                    .show(ui, |ui| {
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
            },
        );
        if changed && !self.sync_review_paused {
            self.sync_review_elapsed = Duration::ZERO;
            self.sync_review_started = (self.sync_selected_count(port) > 0).then(Instant::now);
        }
        match choice {
            steam::ReviewChoice::Action(0) if running => {
                self.sync_review_elapsed = elapsed;
                self.sync_review_started = None;
                self.sync_review_paused = true;
            }
            steam::ReviewChoice::Action(0) => {
                self.sync_review_elapsed = Duration::ZERO;
                self.sync_review_started = Some(Instant::now());
                self.sync_review_paused = false;
            }
            steam::ReviewChoice::Action(1) => self.confirm_sync_plan(port),
            steam::ReviewChoice::Action(2) => {
                self.sync_plan_token = None;
                self.sync_review_started = None;
                self.sync_confirmation = None;
                self.request(UiRequest::CancelSynchronization);
            }
            steam::ReviewChoice::Dismissed => {
                self.sync_review_elapsed = elapsed;
                self.sync_review_started = None;
                self.sync_review_paused = true;
            }
            steam::ReviewChoice::Idle if running && remaining.is_zero() && !changed => {
                self.confirm_sync_plan(port);
            }
            steam::ReviewChoice::Idle => {}
            steam::ReviewChoice::Action(_) => {}
        }
        if running {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    pub(crate) fn open_sync_app_id(&mut self, port: &dyn EditorPort) {
        self.sync_app_id_edit = Some(AppIdEdit {
            text: match port.editor().document().app_id() {
                0 => String::new(),
                id => id.to_string(),
            },
            project_name: port.editor().document().project()["displayName"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
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
        let response = crate::modal::dialog(ctx, egui::Id::new("sync-app-id-editor"))
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
                crate::modal::drag_header(ui, header);
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
                            ui.add(egui::Label::new("Edit the project name and Steam App ID for the app you want to synchronize with. Validate the destination after saving.").wrap().halign(Align::Center));
                            ui.add(egui::Label::new("Changing the App ID clears the previous Steam comparison history and keeps your project entries and artwork.").wrap().halign(Align::Center));
                            crate::fields::row(ui, "Project name", |ui| {
                                if ui.add(egui::TextEdit::singleline(&mut edit.project_name)
                                    .id(egui::Id::new("sync-project-name-field"))
                                    .char_limit(1024).desired_width(ui.available_width())).changed() {
                                    edit.error = None;
                                }
                            });
                            crate::fields::row(ui, "Steam App ID", |ui| {
                                let field = ui.add(egui::TextEdit::singleline(&mut edit.text)
                                    .font(egui::TextStyle::Monospace)
                                    .id(egui::Id::new("sync-app-id-field")).char_limit(32)
                                    .desired_width(ui.available_width()));
                                if edit.focus { field.request_focus(); edit.focus = false; }
                                if field.changed() { edit.error = None; }
                            });
                            let error = if !current { Some("The project changed. Cancel and reopen this editor.") }
                                else if !available { Some("Finish or cancel the current Steam operation first.") }
                                else if edit.project_name.trim().is_empty() || edit.project_name.len() > 1024 { Some("Enter a project name.") }
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
                        if ui.add_enabled(index == 1 || (parse_app_id(&edit.text).is_some() && !edit.project_name.trim().is_empty() && edit.project_name.len() <= 1024 && current && available),
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
        if (save || crate::modal::enter(ctx, &response))
            && let Some(app_id) = parse_app_id(&edit.text)
            && !edit.project_name.trim().is_empty()
            && edit.project_name.len() <= 1024
            && current
            && available
        {
            match port.execute(
                "Edit project and Steam App ID",
                Command::ProjectSettings {
                    name: edit.project_name.trim().to_owned(),
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
