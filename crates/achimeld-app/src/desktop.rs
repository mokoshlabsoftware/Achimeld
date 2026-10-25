use crate::app_dialog::{AppDialog, Dialog, DialogResult, SaveChoice, ViewerNext};
use crate::preview_renderer::PreviewWorker;
use crate::recent_projects::RecentProjects;
use achimeld_application::{
    Command, Document, Editor, EditorPort, Preferences, Preview, UiRequest, new_id,
};
use achimeld_browser::worker::SteamReader;
use achimeld_core::SteamAppId;
use achimeld_imaging::AssetSource;
use achimeld_persistence::project::{ProjectStore, resolve_reference};
use achimeld_security::{cancellation::Cancellation, supervision::Session};
use eframe::egui;
use image::RgbaImage;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
mod automation;
mod effects;
mod gradients;
mod image_drop;
mod particles;
mod style_viewer;

fn logged_status(message: String) -> String {
    achimeld_application::diagnostics::record("INFO", "application", &message);
    message
}

pub fn run(mut session: Session) -> Result<(), Box<dyn std::error::Error>> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/achimeld.png"))?;
    session.require_healthy()?;
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    let (health_tx, health_rx) = mpsc::sync_channel(1);
    let monitor = std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            if session.require_healthy().is_err() {
                let _ = health_tx.try_send(());
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_decorations(false)
            .with_maximized(true)
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1000.0, 650.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Achimeld",
        options,
        Box::new(move |cc| Ok(Box::new(NativeApp::new(cc, health_rx)))),
    );
    stopped.store(true, Ordering::Release);
    let _ = monitor.join();
    result.map_err(Into::into)
}

struct NativeApp {
    automation: Option<automation::Server>,
    automation_state: automation::Engine,
    ui: achimeld_ui::Desktop,
    host: Host,
    health: mpsc::Receiver<()>,
    about_pending: bool,
}
impl NativeApp {
    fn new(cc: &eframe::CreationContext<'_>, health: mpsc::Receiver<()>) -> Self {
        crate::ui_fonts::configure(&cc.egui_ctx);
        let mut host = Host::new(cc.egui_ctx.clone());
        host.file_dialog = rfd::FileDialog::new().set_parent(cc);
        let automation = match automation::Server::start(cc.egui_ctx.clone()) {
            Ok(server) => Some(server),
            Err(error) => {
                host.status = crate::desktop::logged_status(format!(
                    "Editor automation unavailable: {error}"
                ));
                None
            }
        };
        if host
            .settings
            .as_ref()
            .is_none_or(|location| location.needs_choice)
        {
            host.start_dialog(Dialog::SettingsFolder);
        }
        let mut ui = achimeld_ui::Desktop::default();
        let about_pending = !host.about_seen && host.dialog.is_some();
        if !host.about_seen && !about_pending {
            ui.open_about();
        }
        Self {
            automation,
            automation_state: Default::default(),
            ui,
            host,
            health,
            about_pending,
        }
    }
}
impl eframe::App for NativeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ui
            .file_drop_position(image_drop::native_drop_position(ui.ctx(), frame));
        if matches!(
            self.health.try_recv(),
            Ok(()) | Err(mpsc::TryRecvError::Disconnected)
        ) {
            std::process::exit(125);
        }
        let dialog_open = self.host.dialog.is_some() || self.host.drop_picker_open();
        if !dialog_open && self.about_pending {
            self.ui.open_about();
            self.about_pending = false;
        }
        if !dialog_open {
            self.host.poll_read();
            self.host.poll_synchronization();
            self.host.poll_export();
            self.host.poll_translations();
            self.host.poll_art();
            self.host.poll_image_drop();
        }
        self.host.previews.poll();
        self.host.poll_viewer();
        if dialog_open {
            ui.disable();
        }
        self.ui.show(ui, &mut self.host);
        self.host.show_dialog(ui.ctx());
        self.host.show_drop_picker(ui.ctx());
        if let Some(server) = &self.automation {
            self.automation_state
                .poll(server, &mut self.host, &mut self.ui);
        }
        if ui.ctx().input(|i| i.viewport().close_requested()) && !self.host.exit_allowed {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ui.close(&mut self.host);
        }
        ui.ctx().request_repaint_after(Duration::from_millis(250));
    }
}

struct PendingRead {
    cancel: Cancellation,
    receiver: mpsc::Receiver<Result<Value, String>>,
    project_id: String,
    app_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}
struct PendingExport {
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<(), String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
struct Host {
    editor_session_id: String,
    settings: Option<crate::settings_location::SettingsLocation>,
    editor: Editor,
    store: ProjectStore,
    status: String,
    ctx: egui::Context,
    read: Option<PendingRead>,
    synchronization: Option<crate::synchronization::Pending>,
    verified_sync: Option<(String, u64)>,
    sync_error: Option<(String, u32, String)>,
    previews: PreviewWorker,
    render_generation: u64,
    assets: Files,
    exit_allowed: bool,
    export: Option<PendingExport>,
    translations: Option<crate::translations::Pending>,
    about_seen: bool,
    dialog: Option<AppDialog>,
    file_dialog: rfd::FileDialog,
    recent_projects: RecentProjects,
    editor_state: crate::editor_state::EditorState,
    art: Option<crate::art_bank::Pending>,
    art_queue: std::collections::VecDeque<(PathBuf, Option<String>)>,
    general_clipboard: Option<(String, String, Value)>,
    general_image_added: Option<(String, i64, u64)>,
    particle_preset: Option<(achimeld_application::particles::PresetTarget, Value)>,
    effect_preview: Option<effects::EffectPreview>,
    construction_preview: Option<effects::EffectPreview>,
    art_stats: crate::art_bank::Stats,
    art_stats_epoch: u64,
    image_drop: Option<image_drop::Pending>,
    image_import_selection: Option<(String, u64)>,
    style_viewer: Option<style_viewer::Viewer>,
}
impl Host {
    fn new(ctx: egui::Context) -> Self {
        crate::system_fonts::prepare(ctx.clone());
        #[cfg(not(test))]
        let settings_result = crate::settings_location::SettingsLocation::discover();
        // Test hosts must never discover or update the desktop's settings/history.
        #[cfg(test)]
        let settings_result: Result<_, String> = {
            let folder = crate::test_output::root().join(format!("host-settings-{}", new_id()));
            Ok(crate::settings_location::SettingsLocation::test_location(
                folder.clone(),
                folder.join("locator.json"),
            ))
        };
        let mut settings_error = settings_result.as_ref().err().cloned();
        let settings = settings_result.ok();
        let preferences = settings
            .as_ref()
            .map(|location| load_preferences_at(&location.folder))
            .transpose()
            .unwrap_or_else(|error| {
                settings_error = Some(error);
                None
            })
            .unwrap_or_default();
        let editor_state = settings
            .as_ref()
            .map(|location| {
                crate::editor_state::EditorState::open(location.path("rust-editor-state.json"))
            })
            .transpose()
            .unwrap_or_else(|error| {
                settings_error = Some(error);
                None
            })
            .unwrap_or_else(|| crate::editor_state::EditorState::new(None));
        let previews = PreviewWorker::new(ctx.clone());
        Self {
            editor_session_id: new_id(),
            recent_projects: RecentProjects::for_user(
                settings
                    .as_ref()
                    .map(|location| location.path("rust-recent-projects.json")),
            ),
            editor_state,
            settings,
            editor: Editor::default().with_preferences(preferences),
            store: ProjectStore::new(),
            status: logged_status(
                settings_error
                    .map(|error| format!("Could not load shared settings: {error}"))
                    .unwrap_or_else(|| "Ready · supervised editor".into()),
            ),
            ctx,
            read: None,
            synchronization: None,
            verified_sync: None,
            sync_error: None,
            previews,
            render_generation: 1,
            assets: Files::new(),
            exit_allowed: false,
            export: None,
            translations: None,
            about_seen: load_about_seen().unwrap_or(false),
            dialog: None,
            file_dialog: rfd::FileDialog::new(),
            art: None,
            art_queue: Default::default(),
            general_clipboard: None,
            general_image_added: None,
            particle_preset: None,
            effect_preview: None,
            construction_preview: None,
            art_stats: Default::default(),
            art_stats_epoch: 0,
            image_drop: None,
            image_import_selection: None,
            style_viewer: None,
        }
    }
    fn set_sync_error(&mut self, error: String) {
        self.sync_error = Some((
            self.editor_session_id.clone(),
            self.editor.document().app_id(),
            error.clone(),
        ));
        self.status = crate::desktop::logged_status(error);
    }
    fn report(&mut self, result: Result<(), String>, success: &str) {
        self.status = crate::desktop::logged_status(match result {
            Ok(()) => success.into(),
            Err(error) => error,
        });
    }
    fn save(&mut self, save_as: bool, then: Option<UiRequest>) {
        let path = if !save_as {
            self.store.path().map(Path::to_path_buf)
        } else {
            None
        };
        if let Some(path) = path {
            self.complete_dialog(DialogResult::Save(then, Some(path)));
        } else {
            self.start_dialog(Dialog::Save(then));
        }
    }
    fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.store.save(self.editor.document(), path)?;
        self.art_stats_epoch += 1;
        self.editor.mark_saved();
        self.assets.project = self.store.asset_base().map(Path::to_path_buf);
        self.remember_project("Project saved");
        Ok(())
    }
    fn remember_project(&mut self, success: &str) {
        self.status = crate::desktop::logged_status(success.into());
        if let Some(path) = self.store.path()
            && let Err(error) = self.recent_projects.remember(path)
        {
            self.status = crate::desktop::logged_status(format!(
                "{success} · Could not remember recent projects: {error}"
            ));
        }
    }
    fn start_dialog(&mut self, dialog: Dialog) {
        if self.dialog.is_some() {
            return;
        }
        let source = match &dialog {
            Dialog::ReloadArt { atlas, id } => self.art_source_file(*atlas, id),
            _ => None,
        };
        let mut picker = self.file_dialog.clone();
        if let Some(name) = source
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
        {
            picker = picker.set_file_name(name);
        }
        let folder = source
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| {
                self.store
                    .path()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            })
            .or_else(|| self.settings_folder().map(Path::to_path_buf))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let is_project_prompt = matches!(dialog, Dialog::Discard(_));
        let mut dialog = AppDialog::new(dialog, folder, picker);
        if is_project_prompt {
            dialog = dialog.with_project(
                self.editor.document().project()["displayName"]
                    .as_str()
                    .unwrap_or("Untitled project"),
                self.store.path(),
            );
        }
        self.dialog = Some(dialog);
    }
    fn art_source_file(&self, atlas: bool, id: &str) -> Option<PathBuf> {
        let assets = &self.editor.document().project()["assets"];
        let source = if atlas && id == "default" {
            &assets["atlasImportSource"]
        } else {
            &assets[if atlas { "atlases" } else { "images" }]
                .as_array()?
                .iter()
                .find(|entry| entry["id"].as_str() == Some(id))?["importSource"]
        };
        if source["kind"] != "File" {
            return None;
        }
        let path = PathBuf::from(source["path"].as_str()?);
        // Metadata only seeds a user-requested picker; it is never an image
        // reference or permission to read a file. Avoid network path hints.
        use std::path::{Component, Prefix};
        (path.is_absolute()
            && matches!(path.components().next(), Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))))
        .then_some(path)
    }
    fn show_dialog(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.dialog.as_mut().and_then(|dialog| dialog.show(ctx)) {
            self.dialog = None;
            self.complete_dialog(result);
        }
    }
    fn complete_dialog(&mut self, result: DialogResult) {
        match result {
            DialogResult::EffectsImport(kind, Some(path)) => {
                let result = self.import_effects(kind, &path);
                self.report(result, "Effect collection imported");
            }
            DialogResult::EffectsExport(kind, presets, Some(path)) => {
                let result = effects::export(kind, &presets, &path);
                self.report(result, "Effect collection exported");
            }
            DialogResult::GradientImport(Some(path)) => {
                let result = self.import_gradients(&path);
                self.report(result, "Photoshop gradients imported");
            }
            DialogResult::GradientExport(gradients, Some(path)) => {
                let result = gradients::export(&path, &gradients);
                self.report(result, "Photoshop gradients exported");
            }
            DialogResult::ParticleLoad(target, Some(path)) => {
                let result = self.load_particle_preset(target, &path);
                self.report(result, "Particle effect loaded");
            }
            DialogResult::ParticleSave(settings, Some(path)) => {
                let result = particles::save(&path, &settings);
                self.report(result, "Particle effect saved");
            }
            result @ (DialogResult::ViewerLoad(_)
            | DialogResult::ViewerSave { .. }
            | DialogResult::ViewerImage(_)
            | DialogResult::ViewerDiscard(_, _)) => self.viewer_dialog(result),
            DialogResult::Discard(request, SaveChoice::Save) => {
                self.save(false, Some(request));
            }
            DialogResult::Discard(request, SaveChoice::Discard) => {
                self.continue_request(request);
            }
            DialogResult::Save(then, Some(path)) => match self.save_to(&path) {
                Ok(()) => {
                    if let Some(request) = then {
                        self.continue_request(request);
                    }
                }
                Err(error) => {
                    if matches!(then, Some(UiRequest::PrepareSynchronization)) {
                        self.set_sync_error(error);
                    } else {
                        self.status = crate::desktop::logged_status(error);
                    }
                }
            },
            DialogResult::Open(Some(path)) => {
                if let Err(error) = self.open(&path) {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            DialogResult::SettingsFolder(Some(path)) => {
                let result = self.change_settings_folder(&path);
                self.report(result, "Achimeld folder changed");
            }
            DialogResult::Import(atlas, Some(path)) => {
                let result = self.start_art(path, crate::art_bank::Target::Import(atlas), None);
                if let Err(error) = result {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            DialogResult::ReloadArt {
                atlas,
                id,
                path: Some(path),
            } => {
                let result = self.replace_art(atlas, &id, Some(path), None);
                if let Err(error) = result {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            DialogResult::Export(Some(path)) => {
                if let Err(error) = self.export_icons(path) {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            DialogResult::ExportTranslations(Some(path)) => self.start_translations(path, false),
            DialogResult::ImportTranslations(Some(path)) => self.start_translations(path, true),
            DialogResult::ImportLayer {
                project_id,
                achievement_id,
                layer_id,
                general,
                path: Some(path),
            } => {
                let result = if self.editor.document().project()["projectId"].as_str()
                    != Some(&project_id)
                {
                    Err("The project changed while importing the image".into())
                } else {
                    self.start_art(
                        path,
                        if general {
                            crate::art_bank::Target::General(achievement_id)
                        } else {
                            crate::art_bank::Target::Layer {
                                achievement: achievement_id,
                                layer: layer_id,
                            }
                        },
                        None,
                    )
                };
                if let Err(error) = result {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            // Cancel is terminal, including Save before Open/New/Exit.
            _ => {}
        }
    }
    fn continue_request(&mut self, request: UiRequest) {
        match request {
            UiRequest::PrepareSynchronization => {
                if let Err(error) = self.start_synchronization() {
                    self.set_sync_error(error);
                }
            }
            UiRequest::NewProject => {
                if let Some(job) = &mut self.image_drop {
                    job.cancel();
                }
                if let Some(job) = &self.art {
                    job.cancelled.store(true, Ordering::Release);
                }
                self.art_queue.clear();
                if let Some(read) = &self.read {
                    read.cancel.cancel();
                }
                let preferences = self.editor.preferences().clone();
                self.editor = Editor::new(Document::new()).with_preferences(preferences);
                self.editor_session_id = new_id();
                self.store = ProjectStore::new();
                self.assets = Files::new();
                self.render_generation += 1;
                self.status = crate::desktop::logged_status("New project".into());
            }
            UiRequest::OpenProject => self.start_dialog(Dialog::Open),
            UiRequest::OpenRecentProject(path) => {
                if let Err(error) = self.open(&path) {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            UiRequest::Exit => {
                if self
                    .style_viewer
                    .as_ref()
                    .is_some_and(|viewer| viewer.state.busy)
                {
                    self.status = crate::desktop::logged_status(
                        "Wait for the Style Viewer image import to finish before closing.".into(),
                    );
                    return;
                }
                if self.style_viewer.as_ref().is_some_and(|viewer| {
                    viewer.state.dirty
                        || viewer
                            .state
                            .collections
                            .iter()
                            .any(|collection| collection.dirty)
                }) {
                    self.start_dialog(Dialog::ViewerDiscard(ViewerNext::Exit));
                    return;
                }
                self.exit_allowed = true;
                if let Some(read) = &self.read {
                    read.cancel.cancel();
                }
                if let Some(export) = &self.export {
                    export.cancel.store(true, Ordering::Release);
                }
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
    }
    fn open(&mut self, path: &Path) -> Result<(), String> {
        let mut store = ProjectStore::new();
        let document = store.load(path)?;
        if !document.project()["styleViewerVersion"].is_null() {
            return Err("Open style sets with Tools > Style Viewer > Load style set.".into());
        }
        if let Some(job) = &mut self.image_drop {
            job.cancel();
        }
        let preferences = self.editor.preferences().clone();
        if let Some(read) = &self.read {
            read.cancel.cancel();
        }
        if let Some(job) = &self.art {
            job.cancelled.store(true, Ordering::Release);
        }
        self.art_queue.clear();
        self.editor = Editor::new(document).with_preferences(preferences);
        self.editor_session_id = new_id();
        self.store = store;
        self.assets = Files::new();
        self.assets.project = Some(path.to_path_buf());
        self.render_generation += 1;
        self.remember_project("Project opened");
        Ok(())
    }
    fn change_settings_folder(&mut self, folder: &Path) -> Result<(), String> {
        crate::settings_location::validate_folder(folder)?;
        let mut state =
            crate::editor_state::EditorState::open(folder.join("rust-editor-state.json"))?;
        if ![
            "rust-editor-state.json",
            "user.gradients.json",
            "user.icon-styles.json",
        ]
        .iter()
        .any(|name| folder.join(name).exists())
        {
            state.state = self.editor_state.state.clone();
        }
        let preferences = if ["rust-preferences.json", "preferences.json"]
            .iter()
            .any(|name| folder.join(name).exists())
        {
            load_preferences_at(folder)?
        } else {
            self.editor.preferences().clone()
        };
        let mut settings = self.settings.clone().map_or_else(
            || {
                crate::settings_location::SettingsLocation::discover()
                    .or_else(|_| crate::settings_location::SettingsLocation::default_location())
            },
            Ok,
        )?;
        // All candidate libraries and preferences are validated before activation.
        // Existing C# library files are read only; Rust uses distinct filenames.
        state.persist()?;
        let mut remembered = false;
        self.editor.execute_with(
            "Preferences",
            Command::Preferences(preferences.clone()),
            None,
            |_, after| {
                save_preferences_at(after, &folder.join("rust-preferences.json"))?;
                settings.remember(folder)?;
                remembered = true;
                Ok(())
            },
        )?;
        // execute_with skips its callback for unchanged preferences.
        if !remembered {
            save_preferences_at(&preferences, &folder.join("rust-preferences.json"))?;
            settings.remember(folder)?;
        }
        settings.folder = folder.to_owned();
        settings.needs_choice = false;
        self.recent_projects =
            RecentProjects::for_user(Some(settings.path("rust-recent-projects.json")));
        self.editor_state = state;
        self.settings = Some(settings);
        Ok(())
    }
    fn start_read(&mut self) -> Result<(), String> {
        if self.read.is_some() || self.synchronization.is_some() {
            return Err("A Steam read is already running".into());
        }
        let app_id = SteamAppId::new(self.editor.document().app_id()).map_err(|e| e.to_string())?;
        let project_id = self.editor.document().project()["projectId"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let cancel = Cancellation::default();
        let worker_cancel = cancel.clone();
        let ctx = self.ctx.clone();
        let (tx, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let result = (|| {
                let session = Session::connect().map_err(|e| e.to_string())?;
                let mut reader =
                    SteamReader::connect(session, worker_cancel).map_err(|e| e.to_string())?;
                reader.start().map_err(|e| e.to_string())?;
                reader.open_app(app_id).map_err(|e| e.to_string())?;
                reader
                    .wait_for_authentication(Duration::from_secs(600))
                    .map_err(|e| e.to_string())?;
                reader.open_app(app_id).map_err(|e| e.to_string())?;
                let snapshot = reader.read_snapshot(app_id).map_err(|e| e.to_string())?;
                reader.stop().map_err(|e| e.to_string())?;
                serde_json::to_value(snapshot)
                    .map_err(|_| "Steam snapshot serialization failed".to_owned())
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.read = Some(PendingRead {
            cancel,
            receiver,
            project_id,
            app_id: app_id.value(),
            thread: Some(thread),
        });
        self.status = crate::desktop::logged_status(
            "Reading Steam · sign in manually in the dedicated browser if requested".into(),
        );
        Ok(())
    }
    fn start_synchronization(&mut self) -> Result<(), String> {
        if self.synchronization.is_some()
            || self.image_drop.is_some()
            || self.read.is_some()
            || self.art.is_some()
            || !self.art_queue.is_empty()
            || self.export.is_some()
        {
            return Err("Finish or cancel the current background operation first".into());
        }
        let (path, document) = self.synchronization_input()?;
        self.verified_sync = None;
        self.sync_error = None;
        let mut assets = Files::new();
        assets.project = Some(path.clone());
        self.synchronization = Some(crate::synchronization::Pending::start(
            path,
            document,
            self.editor.revision(),
            assets,
            self.ctx.clone(),
        ));
        self.status = crate::desktop::logged_status("Preparing synchronization…".into());
        Ok(())
    }
    fn synchronization_input(&self) -> Result<(PathBuf, Document), String> {
        SteamAppId::new(self.editor.document().app_id()).map_err(|e| e.to_string())?;
        let path = self
            .store
            .path()
            .ok_or("Save the project before synchronizing")?
            .to_path_buf();
        // Use the exact saved project, including content-addressed art references
        // and the catalog hash written by the transaction immediately before this.
        let mut saved_store = ProjectStore::new();
        let document = saved_store.load(&path)?;
        if document.project()["projectId"] != self.editor.document().project()["projectId"]
            || document.app_id() != self.editor.document().app_id()
        {
            return Err(
                "The saved project identity changed; reopen it before synchronizing".into(),
            );
        }
        // Keep the editor's original asset origin intact if review is cancelled
        // after Save As; only the background snapshot uses the new saved paths.
        Ok((path, document))
    }
    fn poll_synchronization(&mut self) {
        let mut finished = None;
        if let Some(pending) = &mut self.synchronization {
            pending.previews.poll();
            if pending.revision != self.editor.revision() {
                pending.cancel.cancel();
            }
            loop {
                match pending.events.try_recv() {
                    Ok(crate::synchronization::Event::Banner(document)) => {
                        if pending.revision != self.editor.revision() {
                            pending.cancel.cancel();
                            finished = Some(Err("The project changed while its game banner was downloaded; reopen the saved result".into()));
                            break;
                        }
                        let result = self
                            .store
                            .adopt_saved_result(&pending.path, &document)
                            .and_then(|document| {
                                self.editor.execute(
                                    "Download Steam game banner",
                                    Command::ImportArtwork(Box::new(document)),
                                    None,
                                )
                            });
                        if let Err(error) = result {
                            pending.cancel.cancel();
                            finished = Some(Err(error));
                            break;
                        }
                        self.editor.mark_saved();
                        pending.revision = self.editor.revision();
                        self.assets = Files::new();
                        self.assets.project = self.store.asset_base().map(Path::to_path_buf);
                        self.render_generation += 1;
                        self.art_stats_epoch += 1;
                    }
                    Ok(crate::synchronization::Event::Review(review)) => {
                        pending.review = Some(review);
                        self.status = crate::desktop::logged_status(
                            "Steam entries are in the project · review remaining uploads or conflicts".into(),
                        );
                    }
                    Ok(crate::synchronization::Event::Imported(document)) => {
                        let result = self
                            .store
                            .adopt_saved_result(&pending.path, &document)
                            .and_then(|document| {
                                self.editor.execute(
                                    "Import Steam entries",
                                    Command::ApplySynchronization(Box::new(document)),
                                    None,
                                )
                            });
                        if let Err(error) = result {
                            pending.cancel.cancel();
                            finished = Some(Err(error));
                            break;
                        }
                        self.editor.mark_saved();
                        pending.revision = self.editor.revision();
                        self.assets = Files::new();
                        self.assets.project = self.store.asset_base().map(Path::to_path_buf);
                        self.render_generation += 1;
                        self.art_stats_epoch += 1;
                    }
                    Ok(crate::synchronization::Event::IconFiles(files)) => {
                        pending.previews.files = files
                    }
                    Ok(crate::synchronization::Event::Progress(message)) => {
                        // The producer records every stage before enqueueing it.
                        self.status = message
                    }
                    Ok(crate::synchronization::Event::Complete(result)) => {
                        finished = Some(result);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished = Some(Err(
                            "Synchronization stopped; prepare a fresh plan to reconcile Steam"
                                .into(),
                        ));
                        break;
                    }
                }
            }
        }
        if let Some(result) = finished
            && let Some(mut pending) = self.synchronization.take()
        {
            if let Some(thread) = pending.thread.take() {
                let _ = thread.join();
            }
            let result = result.and_then(|document| {
                if pending.revision != self.editor.revision() {
                    return Err(
                        "The project changed during synchronization; reopen its saved result"
                            .into(),
                    );
                }
                // Refresh storage identities after the background transaction. The
                // command retains one undo entry for local changes; Steam is external.
                let document = self.store.adopt_saved_result(&pending.path, &document)?;
                self.execute(
                    "Apply local synchronization result",
                    Command::ApplySynchronization(Box::new(document)),
                    None,
                )?;
                self.editor.mark_saved();
                self.assets = Files::new();
                self.assets.project = self.store.asset_base().map(Path::to_path_buf);
                self.render_generation += 1;
                self.art_stats_epoch += 1;
                Ok(())
            });
            self.verified_sync = result
                .as_ref()
                .ok()
                .map(|()| (self.editor_session_id.clone(), self.editor.revision()));
            if let Err(error) = &result {
                self.set_sync_error(error.clone());
            } else {
                self.sync_error = None;
            }
            self.report(result, "Synchronization completed and saved · Steam draft verified · publish manually in Steamworks");
        }
    }
    fn poll_read(&mut self) {
        let result = self
            .read
            .as_ref()
            .and_then(|read| match read.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Steam reader stopped".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result
            && let Some(mut read) = self.read.take()
        {
            if let Some(thread) = read.thread.take() {
                let _ = thread.join();
            }
            if read.cancel.check().is_err() {
                self.status = crate::desktop::logged_status("Steam read cancelled".into());
                return;
            }
            if self.editor.document().project()["projectId"].as_str() != Some(&read.project_id)
                || self.editor.document().app_id() != read.app_id
            {
                self.status = crate::desktop::logged_status(
                    "Read completed for a different project; result was discarded".into(),
                );
                return;
            }
            let result = result.and_then(|snapshot| {
                self.execute("Read Steam", Command::SetRemoteSnapshot(snapshot), None)
            });
            self.report(result, "Steam configuration read successfully");
        }
    }
    fn import_asset(&mut self, atlas: bool, art: crate::art_bank::Prepared) -> Result<(), String> {
        let (command, _) = self.import_asset_command(atlas, art)?;
        self.execute(
            if atlas {
                "Import atlas"
            } else {
                "Import image"
            },
            command,
            None,
        )
    }
    fn import_asset_command(
        &mut self,
        atlas: bool,
        art: crate::art_bank::Prepared,
    ) -> Result<(Command, String), String> {
        let mut assets = self.editor.document().project()["assets"].clone();
        let id = new_id();
        let reference = self.store.retain_art(&art.bytes)?;
        let name = art.name;
        let key = if atlas { "atlases" } else { "images" };
        let list = assets[key].as_array_mut().ok_or("Invalid image library")?;
        let mut entry = if atlas {
            json!({"id":id,"displayName":name,"image":reference,"columns":0,"rows":0})
        } else {
            json!({"id":id,"displayName":name,"image":reference,"kind":"User"})
        };
        entry["pixelWidth"] = json!(art.dimensions[0]);
        entry["pixelHeight"] = json!(art.dimensions[1]);
        entry["fileBytes"] = json!(art.bytes.len());
        entry["importSource"] = json!(art.source);
        list.push(entry);
        if atlas {
            assets["atlas"] = reference;
            assets["atlasImportSource"] = json!(art.source);
        }
        let project = self.editor.document().project();
        Ok((
            Command::ProjectSettings {
                name: project["displayName"].as_str().unwrap_or("").into(),
                app_id: self.editor.document().app_id(),
                atlas: None,
                assets: Some(assets),
            },
            id,
        ))
    }
    fn import_layer_image(
        &mut self,
        achievement: &str,
        layer_id: Option<&str>,
        art: crate::art_bank::Prepared,
    ) -> Result<(), String> {
        let mut entity = self
            .editor
            .document()
            .entity(achimeld_application::EntityKind::Achievement, achievement)
            .cloned()
            .ok_or("Achievement no longer exists")?;
        let name = art.name.clone();
        let (asset, image_id) = self.import_asset_command(false, art)?;
        if entity["icon"].is_null() {
            entity["icon"] = json!({"layers":[{"id":new_id(),"kind":if entity["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"atlasId":entity["atlasId"],"imageId":entity["standaloneImageId"],"tileIndex":entity["tileIndex"],"displayName":"Icon","visible":true,"effects":[]}]});
        }
        let layers = entity["icon"]["layers"]
            .as_array_mut()
            .ok_or("Invalid icon stack")?;
        if let Some(id) = layer_id {
            let index = layers
                .iter()
                .position(|l| l["id"].as_str() == Some(id))
                .ok_or("Layer no longer exists")?;
            if layers[index]["kind"] == "Folder" {
                let mask = new_id();
                layers[index]["maskLayerId"] = json!(mask);
                layers.push(json!({"id":mask,"displayName":"Folder mask","kind":"StandaloneImage","imageId":image_id,"visible":false,"opacity":1.0,"effects":[]}));
            } else {
                let layer = &mut layers[index];
                layer["kind"] = json!("StandaloneImage");
                layer["imageId"] = json!(image_id);
                layer["atlasId"] = Value::Null;
                layer["linkAtlasToGeneral"] = json!(false);
            }
        } else {
            layers.push(json!({"id":new_id(),"displayName":name,"kind":"StandaloneImage","imageId":image_id,"visible":true,"opacity":1.0,"effects":[]}));
        }
        self.execute(
            "Import layer image",
            Command::Batch(vec![
                asset,
                Command::ReplaceEntity {
                    kind: achimeld_application::EntityKind::Achievement,
                    id: achievement.into(),
                    value: entity,
                },
            ]),
            None,
        )
    }
    fn import_general_image(
        &mut self,
        achievement: &str,
        art: crate::art_bank::Prepared,
    ) -> Result<(), String> {
        let mut entity = self
            .editor
            .document()
            .entity(achimeld_application::EntityKind::Achievement, achievement)
            .cloned()
            .ok_or("Achievement no longer exists")?;
        let (asset, image_id) = self.import_asset_command(false, art)?;
        achimeld_application::append_general_image(
            &mut entity,
            json!({"imageId":image_id,"atlasId":null,"tileIndex":0}),
        );
        self.finish_group();
        let result = self.execute(
            "Import general image",
            Command::Batch(vec![
                asset,
                Command::ReplaceEntity {
                    kind: achimeld_application::EntityKind::Achievement,
                    id: achievement.into(),
                    value: entity,
                },
            ]),
            None,
        );
        if result.is_ok() {
            self.remember_general_image(achievement);
        }
        result
    }
    fn remember_general_image(&mut self, achievement: &str) {
        if let Some(entity) = self
            .editor
            .document()
            .entity(achimeld_application::EntityKind::Achievement, achievement)
        {
            let index = achimeld_application::general_image_entries(entity).len() as i64 - 1;
            self.general_image_added = Some((achievement.into(), index, self.editor.revision()));
        }
    }
    fn paste_general_image(&mut self, achievement: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        if let Some((project, token, source)) = &self.general_clipboard
            && self.editor.document().project()["projectId"] == project.as_str()
            && clipboard.get_text().ok().as_ref() == Some(token)
        {
            let mut entity = self
                .editor
                .document()
                .entity(achimeld_application::EntityKind::Achievement, achievement)
                .cloned()
                .ok_or("Achievement no longer exists")?;
            achimeld_application::append_general_image(&mut entity, source.clone());
            self.finish_group();
            let result = self.execute(
                "Paste general image",
                Command::ReplaceEntity {
                    kind: achimeld_application::EntityKind::Achievement,
                    id: achievement.into(),
                    value: entity,
                },
                None,
            );
            if result.is_ok() {
                self.remember_general_image(achievement);
            }
            return result;
        }
        if self.art.is_some() {
            return Err("Artwork is still being processed".into());
        }
        let project = self.editor.document().project()["projectId"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let (tx, receiver) = mpsc::sync_channel(1);
        let (_preview_tx, preview_receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
                let image = clipboard.get_image().map_err(|e| e.to_string())?;
                let width = u32::try_from(image.width).map_err(|e| e.to_string())?;
                let height = u32::try_from(image.height).map_err(|e| e.to_string())?;
                if width == 0
                    || height == 0
                    || width > 16384
                    || height > 16384
                    || image.bytes.len() > 64 * 1024 * 1024
                    || stop.load(Ordering::Acquire)
                {
                    return Err("Clipboard image exceeds the artwork limits".into());
                }
                let image = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
                    .ok_or("Invalid clipboard image")?;
                let mut bytes = std::io::Cursor::new(Vec::new());
                image
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                Ok(crate::art_bank::Prepared {
                    name: "Clipboard".into(),
                    bytes: bytes.into_inner(),
                    dimensions: [width, height],
                    source: crate::art_bank::ImportSource::Clipboard,
                })
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.art = Some(crate::art_bank::Pending {
            project,
            target: crate::art_bank::Target::General(achievement.into()),
            receiver,
            preview_receiver,
            preview: None,
            cancelled,
        });
        Ok(())
    }
    fn start_art(
        &mut self,
        path: PathBuf,
        target: crate::art_bank::Target,
        size: Option<[u32; 2]>,
    ) -> Result<(), String> {
        if self.art.is_some() || self.image_drop.is_some() {
            return Err("Artwork is still being processed".into());
        }
        let project = self.editor.document().project()["projectId"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let (tx, receiver) = mpsc::sync_channel(1);
        let (preview_tx, preview_receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = crate::art_bank::prepare_progressive(
                path,
                size,
                |preview| {
                    let _ = preview_tx.try_send(preview);
                    ctx.request_repaint();
                },
                &|| worker_cancelled.load(Ordering::Acquire),
            );
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.art = Some(crate::art_bank::Pending {
            project,
            target,
            receiver,
            preview_receiver,
            preview: None,
            cancelled,
        });
        self.status = crate::desktop::logged_status("Processing artwork…".into());
        Ok(())
    }
    fn replace_art(
        &mut self,
        atlas: bool,
        id: &str,
        path: Option<PathBuf>,
        size: Option<[u32; 2]>,
    ) -> Result<(), String> {
        let key = if atlas { "atlases" } else { "images" };
        let expected = if atlas && id == "default" {
            self.editor.document().project()["assets"]["atlas"].clone()
        } else {
            let entry = self.editor.document().project()["assets"][key]
                .as_array()
                .and_then(|items| items.iter().find(|entry| entry["id"].as_str() == Some(id)))
                .ok_or("Artwork no longer exists")?;
            entry["image"].clone()
        };
        let reload = path.is_some();
        let path = path.map_or_else(
            || {
                let base = self.store.asset_base().map(Path::to_path_buf).unwrap_or(
                    std::env::current_dir()
                        .map_err(|e| e.to_string())?
                        .join("Untitled.achv"),
                );
                resolve_reference(&expected, &base)
            },
            Ok,
        )?;
        self.start_art(
            path,
            crate::art_bank::Target::Replace {
                atlas,
                id: id.into(),
                expected,
                reload,
            },
            size,
        )
    }
    fn poll_art(&mut self) {
        if let Some(job) = &mut self.art
            && let Ok(preview) = job.preview_receiver.try_recv()
        {
            job.preview = Some(preview);
        }
        let result = self
            .art
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Artwork processing stopped".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result
            && let Some(job) = self.art.take()
        {
            let result = if self.editor.document().project()["projectId"].as_str()
                != Some(&job.project)
            {
                self.art_queue.clear();
                Err("Artwork completed for a different project; result discarded".into())
            } else {
                result.and_then(|art| match job.target {
                    crate::art_bank::Target::Import(atlas) => self.import_asset(atlas, art),
                    crate::art_bank::Target::General(achievement) => {
                        self.import_general_image(&achievement, art)
                    }
                    crate::art_bank::Target::Layer { achievement, layer } => {
                        self.import_layer_image(&achievement, layer.as_deref(), art)
                    }
                    crate::art_bank::Target::Replace {
                        atlas,
                        id,
                        expected,
                        reload,
                    } => {
                        let mut assets = self.editor.document().project()["assets"].clone();
                        let key = if atlas { "atlases" } else { "images" };
                        if atlas && id == "default" {
                            if assets["atlas"] != expected {
                                return Err(
                                    "Artwork changed during processing; result discarded".into()
                                );
                            }
                            assets["atlas"] = self.store.retain_art(&art.bytes)?;
                            if reload {
                                assets["atlasImportSource"] = json!(art.source);
                            }
                        } else {
                            let entry = assets[key]
                                .as_array_mut()
                                .and_then(|items| {
                                    items
                                        .iter_mut()
                                        .find(|entry| entry["id"].as_str() == Some(&id))
                                })
                                .ok_or("Artwork no longer exists")?;
                            if entry["image"] != expected {
                                return Err(
                                    "Artwork changed during processing; result discarded".into()
                                );
                            }
                            let reference = self.store.retain_art(&art.bytes)?;
                            entry["image"] = reference.clone();
                            entry["pixelWidth"] = json!(art.dimensions[0]);
                            entry["pixelHeight"] = json!(art.dimensions[1]);
                            entry["fileBytes"] = json!(art.bytes.len());
                            if reload {
                                entry["importSource"] = json!(art.source);
                            }
                            if atlas && assets["atlas"] == expected {
                                assets["atlas"] = reference;
                                if reload {
                                    assets["atlasImportSource"] = json!(art.source);
                                }
                            }
                        }
                        let project = self.editor.document().project();
                        self.execute(
                            "Update artwork",
                            Command::ProjectSettings {
                                name: project["displayName"].as_str().unwrap_or("").into(),
                                app_id: self.editor.document().app_id(),
                                atlas: None,
                                assets: Some(assets),
                            },
                            None,
                        )
                    }
                })
            };
            if result.is_err() {
                self.art_queue.clear();
            }
            self.report(result, "Art Data Bank updated");
        }
        if self.art.is_none()
            && let Some((path, achievement)) = self.art_queue.pop_front()
        {
            let target = achievement.map_or(
                crate::art_bank::Target::Import(false),
                crate::art_bank::Target::General,
            );
            let result = self.start_art(path, target, None);
            if let Err(error) = result {
                self.status = crate::desktop::logged_status(error);
            }
        }
    }
    fn export_icons(&mut self, directory: PathBuf) -> Result<(), String> {
        if self.export.is_some() {
            return Err("An icon export is already running".into());
        }
        let document = self.editor.document().clone();
        let project = self.store.asset_base().map(Path::to_path_buf);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let ctx = self.ctx.clone();
        let (tx, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let mut assets = Files::new();
            assets.project = project;
            let result = crate::icon_export::export(&document, &directory, &mut assets, || {
                stop.load(Ordering::Acquire)
            });
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.export = Some(PendingExport {
            cancel,
            receiver,
            thread: Some(thread),
        });
        self.status =
            crate::desktop::logged_status("Exporting achievement and grayscale icons…".into());
        Ok(())
    }
    fn poll_export(&mut self) {
        let result = self
            .export
            .as_ref()
            .and_then(|export| match export.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Icon exporter stopped".into())),
            });
        if let Some(result) = result
            && let Some(mut export) = self.export.take()
        {
            if let Some(thread) = export.thread.take() {
                let _ = thread.join();
            }
            self.report(result, "Achievement and grayscale icons exported");
        }
    }
    fn start_translations(&mut self, path: PathBuf, import: bool) {
        if self.translations.is_some() || self.synchronization.is_some() {
            self.status = crate::desktop::logged_status(
                "Finish the current translation or synchronization operation first".into(),
            );
            return;
        }
        self.editor.finish_group();
        self.translations = Some(crate::translations::Pending::start(
            self.editor.document().clone(),
            self.editor.revision(),
            path,
            import,
            self.ctx.clone(),
        ));
        self.status = crate::desktop::logged_status(
            if import {
                "Reading achievement translations…"
            } else {
                "Exporting achievement translations…"
            }
            .into(),
        );
    }
    fn poll_translations(&mut self) {
        let result =
            self.translations
                .as_ref()
                .and_then(|pending| match pending.receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Translation operation stopped".into()))
                    }
                });
        if let Some(result) = result
            && let Some(pending) = self.translations.take()
        {
            match result {
                Ok(crate::translations::Outcome::Exported(path)) => {
                    self.status = crate::desktop::logged_status(format!(
                        "Achievement translations exported to {}",
                        path.display()
                    ));
                }
                Ok(crate::translations::Outcome::Imported(command, count)) => {
                    if self.editor.revision() != pending.revision
                        || self.editor.document().project()["projectId"].as_str()
                            != Some(&pending.project_id)
                    {
                        self.status = crate::desktop::logged_status(
                            "Project changed while reading translations; import the CSV again"
                                .into(),
                        );
                    } else if count == 0 {
                        self.status = crate::desktop::logged_status(
                            "No achievement translations changed".into(),
                        );
                    } else {
                        let result = self.execute("Import achievement translations", command, None);
                        self.report(
                            result,
                            &format!(
                                "Imported {count} translated cells · save the project to keep them"
                            ),
                        );
                    }
                }
                Err(error) => self.status = crate::desktop::logged_status(error),
            }
        }
    }
}
impl EditorPort for Host {
    fn take_particle_preset(&mut self, achievement_id: &str, layer_id: &str) -> Option<Value> {
        self.take_loaded_particle_preset(achievement_id, layer_id)
    }
    fn system_font_families(&self) -> Option<&[String]> {
        crate::system_fonts::families()
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
        if self.synchronization.is_some() {
            return Err("Finish or cancel synchronization before editing the project".into());
        }
        let revision = self.editor.preview_revision();
        let preference_path = self
            .settings
            .as_ref()
            .map(|location| location.path("rust-preferences.json"));
        self.editor
            .execute_with(label, command, group, |before, after| {
                persist_preferences_at(before, after, preference_path.as_deref())
            })?;
        if self.editor.preview_revision() != revision {
            self.render_generation += 1;
        }
        Ok(())
    }
    fn finish_group(&mut self) {
        self.editor.finish_group();
    }
    fn undo(&mut self) {
        if self.synchronization.is_some() {
            return;
        }
        let revision = self.editor.preview_revision();
        let path = self
            .settings
            .as_ref()
            .map(|location| location.path("rust-preferences.json"));
        match self
            .editor
            .undo_with(|before, after| persist_preferences_at(before, after, path.as_deref()))
        {
            Ok(true) => {
                if self.editor.preview_revision() != revision {
                    self.render_generation += 1;
                }
                self.status = crate::desktop::logged_status("Undo completed".into());
            }
            Ok(false) => {}
            Err(error) => self.status = crate::desktop::logged_status(error),
        }
    }
    fn redo(&mut self) {
        if self.synchronization.is_some() {
            return;
        }
        let revision = self.editor.preview_revision();
        let path = self
            .settings
            .as_ref()
            .map(|location| location.path("rust-preferences.json"));
        match self
            .editor
            .redo_with(|before, after| persist_preferences_at(before, after, path.as_deref()))
        {
            Ok(true) => {
                if self.editor.preview_revision() != revision {
                    self.render_generation += 1;
                }
                self.status = crate::desktop::logged_status("Redo completed".into());
            }
            Ok(false) => {}
            Err(error) => self.status = crate::desktop::logged_status(error),
        }
    }
    fn request(&mut self, request: UiRequest) {
        if matches!(request, UiRequest::PrepareSynchronization) {
            achimeld_application::diagnostics::record(
                "INFO",
                "validation",
                &format!(
                    "Validate requested · Steam App ID {} · saved project: {}",
                    self.editor.document().app_id(),
                    self.store.path().is_some(),
                ),
            );
        }
        if self.dialog.is_some() {
            if matches!(request, UiRequest::PrepareSynchronization) {
                achimeld_application::diagnostics::record(
                    "WARN",
                    "validation",
                    "Blocked: close the current dialog before validating",
                );
            }
            return;
        }
        if self.synchronization.is_some()
            && !matches!(
                &request,
                UiRequest::ApplySynchronization(_)
                    | UiRequest::CancelSynchronization
                    | UiRequest::PreviewSynchronizationIcons(_)
            )
        {
            if matches!(request, UiRequest::Exit) {
                if let Some(pending) = &self.synchronization {
                    pending.cancel.cancel();
                }
                self.status = crate::desktop::logged_status(
                    "Cancelling synchronization · close again after it stops".into(),
                );
            } else {
                self.status = crate::desktop::logged_status(
                    "Finish or cancel synchronization before editing the project".into(),
                );
            }
            return;
        }
        // Validate membership before saving can reorder/evict the chosen entry.
        if let UiRequest::OpenRecentProject(path) = &request
            && !self.recent_projects.paths.contains(path)
        {
            self.status = crate::desktop::logged_status(
                "This project is no longer in the recent list".into(),
            );
            return;
        }
        match request {
            UiRequest::PrepareSynchronization => {
                self.sync_error = None;
                if SteamAppId::new(self.editor.document().app_id()).is_err() {
                    self.set_sync_error(
                        "Set a valid Steam App ID before validating or synchronizing".into(),
                    );
                } else if self.read.is_some()
                    || self.art.is_some()
                    || self.export.is_some()
                    || self.translations.is_some()
                {
                    self.set_sync_error(
                        "Finish or cancel the current background operation first".into(),
                    );
                } else if self.store.path().is_none() {
                    self.set_sync_error(
                        "Save the project before validating the Steamworks destination".into(),
                    );
                    self.start_dialog(Dialog::SavedProjectRequired);
                } else {
                    self.save(false, Some(UiRequest::PrepareSynchronization));
                }
            }
            UiRequest::ApplySynchronization(choices) => {
                if let Some(pending) = &mut self.synchronization
                    && pending.review.is_some()
                {
                    match pending.approval.try_send(choices) {
                        Ok(()) => {
                            pending.review = None;
                            self.status = crate::desktop::logged_status(
                                "Applying reviewed synchronization…".into(),
                            );
                        }
                        Err(_) => self.set_sync_error(
                            "Synchronization approval could not be delivered".into(),
                        ),
                    }
                }
            }
            UiRequest::PreviewSynchronizationIcons(api) => {
                if let Some(pending) = &mut self.synchronization
                    && pending.review.is_some()
                {
                    pending.previews.request(&api, &self.ctx);
                }
            }
            UiRequest::CancelSynchronization => {
                if let Some(pending) = &mut self.synchronization {
                    pending.cancel.cancel();
                    pending.review = None;
                }
                self.status = crate::desktop::logged_status("Cancelling synchronization · interrupted draft changes will be reconciled by a fresh read".into());
            }
            UiRequest::NewProject
            | UiRequest::OpenProject
            | UiRequest::OpenRecentProject(_)
            | UiRequest::Exit => {
                if self.editor.is_dirty() {
                    self.start_dialog(Dialog::Discard(request));
                } else {
                    self.continue_request(request);
                }
            }
            UiRequest::SaveProject | UiRequest::SaveProjectAs => {
                self.save(matches!(request, UiRequest::SaveProjectAs), None);
            }
            UiRequest::ReadSteam => {
                if let Err(e) = self.start_read() {
                    self.status = crate::desktop::logged_status(e);
                }
            }
            UiRequest::CancelRead => {
                if let Some(read) = &self.read {
                    read.cancel.cancel();
                    self.status = crate::desktop::logged_status("Cancelling Steam read".into());
                }
            }
            UiRequest::CancelExport => {
                if let Some(export) = &self.export {
                    export.cancel.store(true, Ordering::Release);
                    self.status = crate::desktop::logged_status("Cancelling icon export…".into());
                }
            }
            UiRequest::ImportAtlas | UiRequest::ImportImage => {
                self.start_dialog(Dialog::Import(matches!(request, UiRequest::ImportAtlas)));
            }
            UiRequest::ImportArtFiles(paths) => {
                if self.art_busy() {
                    self.status = crate::desktop::logged_status(
                        "Wait for the current artwork import to finish".into(),
                    );
                } else if paths.len() > 32 {
                    self.status = crate::desktop::logged_status(
                        "Drop at most 32 artwork files at a time".into(),
                    );
                } else {
                    self.art_queue
                        .extend(paths.into_iter().map(|path| (path, None)));
                    self.poll_art();
                }
            }
            UiRequest::ImportDroppedImages { paths, target } => {
                if let Err(error) = self.start_image_drop(paths, target) {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            UiRequest::ReloadArt { atlas, id } => {
                self.start_dialog(Dialog::ReloadArt { atlas, id })
            }
            UiRequest::RefreshArt => {
                self.render_generation += 1;
            }
            UiRequest::ResizeArt {
                atlas,
                id,
                width,
                height,
            } => {
                let result = self.replace_art(atlas, &id, None, Some([width, height]));
                if let Err(error) = result {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            UiRequest::ImportGeneralImage { achievement_id } => {
                self.start_dialog(Dialog::ImportLayer {
                    project_id: self.editor.document().project()["projectId"]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                    achievement_id,
                    layer_id: None,
                    general: true,
                })
            }
            UiRequest::ImportGeneralFiles {
                achievement_id,
                paths,
            } => {
                if !self.art_busy() && paths.len() <= 32 {
                    self.art_queue
                        .extend(paths.into_iter().map(|p| (p, Some(achievement_id.clone()))));
                    self.poll_art();
                }
            }
            UiRequest::CopyGeneralImage { source } => {
                let token = new_id();
                match arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(token.clone()))
                {
                    Ok(()) => {
                        self.general_clipboard = Some((
                            self.editor.document().project()["projectId"]
                                .as_str()
                                .unwrap_or("")
                                .into(),
                            token,
                            source,
                        ))
                    }
                    Err(error) => self.status = crate::desktop::logged_status(error.to_string()),
                }
            }
            UiRequest::PasteGeneralImage { achievement_id } => {
                if let Err(error) = self.paste_general_image(&achievement_id) {
                    self.status = crate::desktop::logged_status(error);
                }
            }
            UiRequest::ImportLayerImage {
                achievement_id,
                layer_id,
            } => self.start_dialog(Dialog::ImportLayer {
                project_id: self.editor.document().project()["projectId"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
                achievement_id,
                layer_id,
                general: false,
            }),
            UiRequest::ExportIcons => {
                if self.export.is_some() {
                    self.status =
                        crate::desktop::logged_status("An icon export is already running".into());
                } else {
                    self.start_dialog(Dialog::Export);
                }
            }
            UiRequest::ExportAchievementTranslations | UiRequest::ImportAchievementTranslations => {
                if self.translations.is_some() {
                    self.status = crate::desktop::logged_status(
                        "A translation operation is already running".into(),
                    );
                } else {
                    self.start_dialog(
                        if matches!(request, UiRequest::ImportAchievementTranslations) {
                            Dialog::ImportTranslations
                        } else {
                            Dialog::ExportTranslations
                        },
                    );
                }
            }
            UiRequest::LoadParticlePreset {
                achievement_id,
                layer_id,
            } => {
                self.start_dialog(Dialog::ParticleLoad(
                    achimeld_application::particles::PresetTarget {
                        project_id: self.editor.document().project()["projectId"]
                            .as_str()
                            .unwrap_or("")
                            .into(),
                        revision: self.editor.revision(),
                        achievement_id,
                        layer_id,
                    },
                ));
            }
            UiRequest::SaveParticlePreset(settings) => {
                match achimeld_application::particles::ParticleSettings::from_value(&settings) {
                    Ok(_) => self.start_dialog(Dialog::ParticleSave(settings)),
                    Err(error) => self.status = crate::desktop::logged_status(error),
                }
            }
            UiRequest::ImportGradients => self.start_dialog(Dialog::GradientImport),
            UiRequest::ImportEffects(kind) => self.start_dialog(Dialog::EffectsImport(kind)),
            UiRequest::ExportEffects { kind, presets } => {
                match achimeld_application::effect_presets::encode(kind, &presets) {
                    Ok(_) => self.start_dialog(Dialog::EffectsExport(kind, presets)),
                    Err(error) => self.status = crate::desktop::logged_status(error),
                }
            }
            UiRequest::ExportGradients(gradients) => {
                match achimeld_application::gradient_file::encode(&gradients) {
                    Ok(_) => self.start_dialog(Dialog::GradientExport(gradients)),
                    Err(error) => self.status = crate::desktop::logged_status(error),
                }
            }
            UiRequest::OpenPreferences => {}
            UiRequest::StyleViewer(action) => self.viewer_action(action),
            UiRequest::ChooseSettingsFolder => self.start_dialog(Dialog::SettingsFolder),
            UiRequest::AboutDismissed => {
                if !self.about_seen {
                    match about_state_path().and_then(|path| save_about_seen(&path)) {
                        Ok(()) => self.about_seen = true,
                        Err(error) => {
                            self.status = crate::desktop::logged_status(format!(
                                "Could not remember About dismissal: {error}"
                            ))
                        }
                    }
                }
            }
        }
    }
    fn status(&self) -> &str {
        &self.status
    }
    fn style_viewer(&self) -> Option<&achimeld_application::style_viewer::StyleViewerState> {
        self.style_viewer.as_ref().map(|viewer| &viewer.state)
    }
    fn style_viewer_preview(&mut self) -> Option<Preview> {
        self.viewer_preview()
    }
    fn style_viewer_style_preview(&mut self, style: &str, edge: usize) -> Option<Preview> {
        self.viewer_style_preview(style, edge)
    }
    fn style_viewer_image_preview(&mut self, index: usize) -> Option<Preview> {
        self.viewer_image_preview(index)
    }
    fn style_viewer_default_image_preview(&mut self) -> Option<Preview> {
        self.viewer_default_image_preview()
    }
    fn preview(&mut self, id: &str) -> Option<Preview> {
        self.previews.achievement_preview(
            id,
            None,
            self.render_generation,
            self.editor.document(),
            self.store.asset_base(),
        )
    }
    fn canvas_preview(&mut self, id: &str, edge: usize) -> Option<Preview> {
        self.previews.achievement_preview(
            id,
            Some(edge.clamp(64, 512).next_power_of_two()),
            self.render_generation,
            self.editor.document(),
            self.store.asset_base(),
        )
    }
    fn set_layer_blend_preview(
        &mut self,
        preview: Option<achimeld_application::ports::LayerBlendPreview>,
    ) {
        self.previews.set_blend_preview(preview);
    }
    fn icon_style_preview(
        &mut self,
        style: &str,
        achievement: &str,
        edge: usize,
    ) -> Option<Preview> {
        self.previews.preview(
            &format!(
                "style:{}:{style}:{achievement}",
                edge.clamp(64, 512).next_power_of_two()
            ),
            self.render_generation,
            self.editor.document(),
            self.store.asset_base(),
        )
    }
    fn preferences(&self) -> &Preferences {
        self.editor.preferences()
    }
    fn settings_folder(&self) -> Option<&Path> {
        self.settings
            .as_ref()
            .map(|location| location.folder.as_path())
    }
    fn gradients(&self) -> &[achimeld_application::GradientPreset] {
        &self.editor_state.state.gradients
    }
    fn gradient_maps(&self) -> &[achimeld_application::gradient_map::GradientMapPreset] {
        &self.editor_state.state.gradient_maps
    }
    fn save_gradient_map(
        &mut self,
        preset: achimeld_application::gradient_map::GradientMapPreset,
    ) -> Result<(), String> {
        self.editor_state.gradient_map(preset)
    }
    fn delete_gradient_map(&mut self, id: &str) -> Result<(), String> {
        self.editor_state.delete_gradient_map(id)
    }
    fn effect_presets(&self) -> &[achimeld_application::effect_presets::EffectPreset] {
        &self.editor_state.state.effect_presets
    }
    fn save_effect_preset(
        &mut self,
        preset: achimeld_application::effect_presets::EffectPreset,
    ) -> Result<(), String> {
        self.editor_state.effect(preset)
    }
    fn delete_effect_preset(&mut self, id: &str) -> Result<(), String> {
        self.editor_state.delete_effect(id)
    }
    fn effect_preview(
        &mut self,
        preset: &achimeld_application::effect_presets::EffectPreset,
    ) -> Option<Preview> {
        if self.effect_preview.is_none() {
            self.effect_preview = effects::EffectPreview::new(self.ctx.clone()).ok();
        }
        self.effect_preview.as_mut()?.preview(preset)
    }
    fn effect_preview_error(&self) -> Option<&str> {
        self.effect_preview
            .as_ref()
            .and_then(effects::EffectPreview::error)
    }
    fn explosion_construction_preview(
        &mut self,
        settings: &Value,
        instance: &str,
    ) -> Option<Preview> {
        if self.construction_preview.is_none() {
            self.construction_preview = effects::EffectPreview::new(self.ctx.clone()).ok();
        }
        self.construction_preview
            .as_mut()?
            .construction(settings, instance)
    }
    fn explosion_construction_error(&self) -> Option<&str> {
        self.construction_preview
            .as_ref()
            .and_then(effects::EffectPreview::error)
    }
    fn save_gradient(
        &mut self,
        gradient: achimeld_application::GradientPreset,
    ) -> Result<(), String> {
        self.editor_state.gradient(gradient)
    }
    fn delete_gradient(&mut self, id: &str) -> Result<(), String> {
        self.editor_state.delete_gradient(id)
    }
    fn delete_gradients(&mut self, ids: &[String]) -> Result<(), String> {
        self.editor_state.delete_gradients(ids)
    }
    fn restore_gradients(
        &mut self,
        gradients: Vec<achimeld_application::GradientPreset>,
    ) -> Result<(), String> {
        self.editor_state.import_gradients(gradients)
    }
    fn layer_preview(&mut self, entity_id: &str, layer_id: &str) -> Option<Preview> {
        self.previews.preview(
            &format!("{entity_id}/{layer_id}"),
            self.render_generation,
            self.editor.document(),
            self.store.asset_base(),
        )
    }
    fn asset_preview(&mut self, atlas: bool, asset_id: Option<&str>) -> Option<Preview> {
        self.previews.preview(
            &format!(
                "asset:{}:{}",
                if atlas { "atlas" } else { "image" },
                asset_id.unwrap_or("default")
            ),
            self.render_generation,
            self.editor.document(),
            self.store.asset_base(),
        )
    }
    fn art_file_bytes(&mut self, reference: &Value) -> Option<u64> {
        self.art_stats.get(
            self.render_generation + self.art_stats_epoch,
            &self.editor.document().project()["assets"],
            self.store.asset_base(),
            self.store.path(),
            reference,
            &self.ctx,
        )
    }
    fn art_inventory_ready(&self) -> bool {
        self.art_stats
            .ready(self.render_generation + self.art_stats_epoch)
    }
    fn art_saved_bytes(&self) -> Option<u64> {
        self.art_stats.saved_bytes()
    }
    fn general_image_added(&self) -> Option<(String, i64, u64)> {
        self.general_image_added.clone()
    }
    fn image_import_selection(&self) -> Option<(String, u64)> {
        self.image_import_selection.clone()
    }
    fn art_busy(&self) -> bool {
        self.art.is_some() || !self.art_queue.is_empty() || self.image_drop.is_some()
    }
    fn art_pending_preview(&self) -> Option<Preview> {
        self.art
            .as_ref()
            .filter(|job| {
                self.editor.document().project()["projectId"].as_str() == Some(&job.project)
            })
            .and_then(|job| job.preview.clone())
            .or_else(|| self.drop_preview())
    }
    fn layer_layout(&self) -> achimeld_application::LayerLayout {
        self.editor_state.state.layout.clone()
    }
    fn save_layer_layout(
        &mut self,
        layout: achimeld_application::LayerLayout,
    ) -> Result<(), String> {
        self.editor_state.layout(layout)
    }
    fn global_icon_styles(&self) -> &[achimeld_application::IconStyle] {
        &self.editor_state.state.styles
    }
    fn recent_projects(&self) -> &[PathBuf] {
        &self.recent_projects.paths
    }
    fn preview_error(&self, id: &str) -> Option<&str> {
        self.previews.error(id)
    }
    fn reading(&self) -> bool {
        self.read.is_some()
    }
    fn synchronization_review(&self) -> Option<&Value> {
        self.synchronization
            .as_ref()
            .and_then(|pending| pending.review.as_ref())
    }
    fn synchronization_verified(&self) -> bool {
        self.verified_sync
            .as_ref()
            .is_some_and(|(session, revision)| {
                session == &self.editor_session_id && *revision == self.editor.revision()
            })
    }
    fn synchronization_error(&self) -> Option<&str> {
        self.sync_error
            .as_ref()
            .and_then(|(session, app_id, error)| {
                (session == &self.editor_session_id && *app_id == self.editor.document().app_id())
                    .then_some(error.as_str())
            })
    }
    fn synchronizing(&self) -> bool {
        self.synchronization.is_some()
    }
    fn synchronization_icon_preview(&self, api: &str, locked: bool) -> Option<Preview> {
        self.synchronization
            .as_ref()
            .and_then(|pending| pending.previews.get(api, locked))
    }
    fn exporting(&self) -> bool {
        self.export.is_some()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut pending) = self.synchronization.take() {
            pending.cancel.cancel();
            drop(pending.events);
            drop(pending.approval);
            if let Some(thread) = pending.thread.take() {
                let _ = thread.join();
            }
        }
        if let Some(mut export) = self.export.take() {
            export.cancel.store(true, Ordering::Release);
            if let Some(thread) = export.thread.take() {
                let _ = thread.join();
            }
        }
        if let Some(mut read) = self.read.take() {
            read.cancel.cancel();
            if let Some(thread) = read.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

pub(crate) struct Files {
    pub(crate) project: Option<PathBuf>,
    images: BTreeMap<PathBuf, (Arc<RgbaImage>, u64, Option<std::time::SystemTime>)>,
    fonts: BTreeMap<String, (Arc<achimeld_imaging::Font>, usize)>,
}
impl Files {
    pub(crate) fn new() -> Self {
        Self {
            project: None,
            images: BTreeMap::new(),
            fonts: BTreeMap::new(),
        }
    }
    fn cache(&mut self, path: PathBuf, image: Arc<RgbaImage>) -> Result<(), String> {
        let bytes = image.as_raw().len();
        if bytes > 128 * 1024 * 1024 {
            return Err("Decoded image exceeds 128 MiB".into());
        }
        while !self.images.is_empty()
            && self
                .images
                .values()
                .map(|i| i.0.as_raw().len())
                .sum::<usize>()
                + bytes
                > 128 * 1024 * 1024
        {
            if let Some(key) = self.images.keys().next().cloned() {
                self.images.remove(&key);
            }
        }
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        self.images
            .insert(path, (image, metadata.len(), metadata.modified().ok()));
        Ok(())
    }
}
impl AssetSource for Files {
    fn image(&mut self, reference: &Value) -> Result<Arc<RgbaImage>, String> {
        let project = self.project.clone().unwrap_or(
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join("Untitled.achv"),
        );
        let path = resolve_reference(reference, &project)?;
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if self
            .images
            .get(&path)
            .is_none_or(|i| i.1 != metadata.len() || i.2 != metadata.modified().ok())
        {
            self.cache(path.clone(), Arc::new(load_image(&path)?))?;
        }
        self.images
            .get(&path)
            .map(|i| i.0.clone())
            .ok_or_else(|| "Image unavailable".into())
    }
    fn font(&mut self, family: &str) -> Option<Arc<achimeld_imaging::Font>> {
        let family_key = family.to_lowercase();
        if let Some((font, _)) = self.fonts.get(&family_key) {
            return Some(font.clone());
        }
        let bytes = crate::system_fonts::load(family)?;
        let size = bytes.len();
        let font = Arc::new(achimeld_imaging::Font::from_bytes(bytes).ok()?);
        while self.fonts.values().map(|(_, size)| size).sum::<usize>() + size > 64 * 1024 * 1024 {
            let key = self.fonts.keys().next()?.clone();
            self.fonts.remove(&key);
        }
        self.fonts.insert(family_key, (font.clone(), size));
        Some(font)
    }
}
fn load_image(path: &Path) -> Result<RgbaImage, String> {
    crate::art_bank::decode(path)
}
fn preferences_path() -> Result<PathBuf, String> {
    let root = PathBuf::from(
        std::env::var_os("APPDATA").ok_or("Application settings directory unavailable")?,
    )
    .join("Achimeld");
    Ok(root.join("rust-preferences.json"))
}

// First-launch acknowledgement is session/application state, outside editable
// preferences and project undo. Preferences resets and undo cannot clear it.
fn about_state_path() -> Result<PathBuf, String> {
    Ok(preferences_path()?.with_file_name("rust-about-seen.json"))
}
fn load_about_seen() -> Result<bool, String> {
    load_about_seen_at(&about_state_path()?)
}
fn load_about_seen_at(path: &Path) -> Result<bool, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(65)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 64 {
        return Err("About state is too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}
fn save_about_seen(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Settings directory unavailable")?)
        .map_err(|error| error.to_string())?;
    achimeld_persistence::project::write_atomic(path, b"true\n")
}
fn load_preferences_at(folder: &Path) -> Result<Preferences, String> {
    let path = folder.join("rust-preferences.json");
    if !path.exists() {
        let legacy = folder.join("preferences.json");
        if legacy.exists() {
            return crate::legacy_settings::preferences(&crate::settings_location::read_json(
                &legacy,
                64 * 1024,
            )?);
        }
        return Ok(Preferences::default());
    }
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 {
        return Err("Preferences file is too large".into());
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("Preferences file is too large".into());
    }
    let preferences: Preferences = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    preferences.validate()?;
    Ok(preferences)
}
fn save_preferences_at(preferences: &Preferences, path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Settings directory unavailable")?)
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|e| e.to_string())?;
    achimeld_persistence::project::write_atomic(path, &bytes)
}
fn persist_preferences_at(
    before: &Preferences,
    after: &Preferences,
    path: Option<&Path>,
) -> Result<(), String> {
    if before != after {
        save_preferences_at(after, path.ok_or("Settings directory unavailable")?)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod about_tests {
    use super::*;

    #[test]
    #[ignore = "Requires an explicitly selected local project and artwork"]
    fn selected_project_icon_reference() {
        let path = PathBuf::from(std::env::var_os("ACHIMELD_ICON_PROJECT").unwrap());
        let output = PathBuf::from(std::env::var_os("ACHIMELD_ICON_TO").unwrap());
        let boundary = std::path::absolute(crate::test_output::root()).unwrap();
        assert!(output.is_absolute() && output.starts_with(boundary));
        let document = ProjectStore::new().load(&path).unwrap();
        let internal_id: u64 = std::env::var("ACHIMELD_ICON_INTERNAL_ID")
            .unwrap()
            .parse()
            .unwrap();
        let entity = document
            .entities(achimeld_application::EntityKind::Achievement)
            .iter()
            .find(|entity| entity["internalId"].as_u64() == Some(internal_id))
            .unwrap();
        let id = entity["id"].as_str().unwrap();
        let mut files = Files::new();
        files.project = Some(path);
        let started = std::time::Instant::now();
        achimeld_imaging::render_achieved_cancellable(
            &document,
            id,
            Some((128, 128)),
            &mut files,
            &|| false,
        )
        .unwrap();
        let quick = started.elapsed();
        let pair = achimeld_imaging::render(&document, id, None, &mut files).unwrap();
        let detail = started.elapsed();
        let preview =
            achimeld_imaging::render_achieved_cancellable(&document, id, None, &mut files, &|| {
                false
            })
            .unwrap();
        assert_eq!(preview, pair.achieved);
        preview.save(output).unwrap();
        eprintln!("First visible preview: {quick:?}; final pair ready: {detail:?}");
    }

    #[test]
    fn first_launch_acknowledgement_survives_restart_and_rejects_invalid_state() {
        let root = crate::test_output::root().join(format!("about-state-{}", new_id()));
        let path = root.join("rust-about-seen.json");
        assert!(!load_about_seen_at(&path).unwrap());
        save_about_seen(&path).unwrap();
        assert!(load_about_seen_at(&path).unwrap());
        // Appearance preferences use a separate file; replacing them does not
        // undo the first-launch acknowledgement.
        std::fs::write(root.join("rust-preferences.json"), b"{}").unwrap();
        assert!(load_about_seen_at(&path).unwrap());
        std::fs::write(&path, b"invalid").unwrap();
        assert!(load_about_seen_at(&path).is_err());
        std::fs::write(&path, [b' '; 65]).unwrap();
        assert!(load_about_seen_at(&path).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root.join("rust-preferences.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)]
mod dialog_tests {
    #[test]
    fn verified_sync_is_scoped_to_the_unchanged_editor_session() {
        use super::*;
        let mut host = Host::new(egui::Context::default());
        assert!(!host.synchronization_verified());
        host.verified_sync = Some((host.editor_session_id.clone(), host.editor.revision()));
        assert!(host.synchronization_verified());
        host.editor
            .execute(
                "Change destination",
                Command::ProjectSettings {
                    name: "Changed".into(),
                    app_id: 480,
                    atlas: None,
                    assets: None,
                },
                None,
            )
            .unwrap();
        assert!(!host.synchronization_verified());
        host.verified_sync = Some(("another-session".into(), host.editor.revision()));
        assert!(!host.synchronization_verified());
    }

    #[test]
    fn synchronization_errors_survive_status_changes_but_not_a_different_session() {
        let mut host = Host::new(egui::Context::default());
        host.set_sync_error("Steam navigation timed out".into());
        host.status = "Project saved".into();
        assert_eq!(
            host.synchronization_error(),
            Some("Steam navigation timed out")
        );
        host.editor_session_id = new_id();
        assert_eq!(host.synchronization_error(), None);
        host.continue_request(UiRequest::PrepareSynchronization);
        assert!(host.synchronization_error().is_some());
        assert!(!host.synchronization_verified());
    }
    use super::*;
    use achimeld_application::EntityKind;
    #[test]
    #[ignore = "Native themed prompt screenshot; dedicated workspace output path"]
    fn native_app_prompt_screenshot() {
        use winit::platform::windows::EventLoopBuilderExtWindows as _;
        struct Window {
            desktop: achimeld_ui::Desktop,
            host: Host,
            output: PathBuf,
            frames: usize,
        }
        impl eframe::App for Window {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                ui.disable();
                self.desktop.show(ui, &mut self.host);
                self.host.show_dialog(ui.ctx());
                self.frames += 1;
                if self.frames == 8 {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                            egui::UserData::default(),
                        ));
                }
                for event in ui.input(|i| i.raw.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let rgba: Vec<u8> = image
                            .pixels
                            .iter()
                            .flat_map(egui::Color32::to_array)
                            .collect();
                        image::save_buffer(
                            &self.output,
                            &rgba,
                            image.size[0] as u32,
                            image.size[1] as u32,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                assert!(self.frames < 1000, "Screenshot did not arrive");
                ui.ctx().request_repaint();
            }
        }
        let output = std::path::absolute(PathBuf::from(
            std::env::var_os("ACHIMELD_SCREENSHOT_TO").expect("Set ACHIMELD_SCREENSHOT_TO"),
        ))
        .unwrap();
        let boundary = std::path::absolute(crate::test_output::root()).unwrap();
        assert!(output.starts_with(&boundary) && output.extension().is_some_and(|s| s == "png"));
        let picker = std::env::var_os("ACHIMELD_SCREENSHOT_PICKER").is_some();
        let unsaved = std::env::var_os("ACHIMELD_SCREENSHOT_UNSAVED").is_some();
        let validation = std::env::var_os("ACHIMELD_SCREENSHOT_VALIDATION").is_some();
        let scale: f32 = std::env::var("ACHIMELD_SCREENSHOT_SCALE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1.0);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size([1100.0, 700.0]),
            event_loop_builder: Some(Box::new(|builder| {
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };
        eframe::run_native(
            "Achimeld prompt fixture",
            options,
            Box::new(move |cc| {
                let mut host = dirty_host();
                host.ctx = cc.egui_ctx.clone();
                crate::ui_fonts::configure(&host.ctx);
                host.ctx.set_zoom_factor(scale);
                host.execute(
                    "Name project",
                    Command::ProjectSettings {
                        name: "Aurora".into(),
                        app_id: if validation { 480 } else { 0 },
                        atlas: None,
                        assets: None,
                    },
                    None,
                )
                .unwrap();
                if !unsaved && !validation {
                    let folder = boundary.join(format!("prompt-preview-{}", new_id()));
                    std::fs::create_dir(&folder).unwrap();
                    host.store
                        .save(host.editor.document(), &folder.join("Aurora.achv"))
                        .unwrap();
                }
                host.file_dialog = rfd::FileDialog::new().set_parent(cc);
                if validation {
                    host.request(UiRequest::PrepareSynchronization);
                } else {
                    host.start_dialog(if picker {
                        Dialog::Open
                    } else {
                        Dialog::Discard(UiRequest::Exit)
                    });
                }
                Ok(Box::new(Window {
                    desktop: achimeld_ui::Desktop::default(),
                    host,
                    output,
                    frames: 0,
                }))
            }),
        )
        .unwrap();
    }
    #[test]
    fn changing_shared_folder_preserves_project_and_carries_presets_to_an_empty_destination() {
        let root = std::path::absolute(
            crate::test_output::root().join(format!("shared-folder-{}", new_id())),
        )
        .unwrap();
        let old = root.join("old");
        let chosen = root.join("chosen");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir(&chosen).unwrap();
        let mut host = dirty_host();
        host.settings = Some(crate::settings_location::SettingsLocation::test_location(
            old.clone(),
            root.join("locator.json"),
        ));
        host.editor_state =
            crate::editor_state::EditorState::open(old.join("rust-editor-state.json")).unwrap();
        host.editor_state
            .gradient(achimeld_application::GradientPreset {
                id: new_id(),
                name: "Shared gradient".into(),
                stops: vec![achimeld_application::GradientStop {
                    offset: 0.0,
                    color_hex: "#FF112233".into(),
                }],
            })
            .unwrap();
        let project = host.editor.document().clone();
        let preferences = host.editor.preferences().clone();
        host.change_settings_folder(&chosen).unwrap();
        assert_eq!(host.settings_folder(), Some(chosen.as_path()));
        assert_eq!(host.gradients().len(), 1);
        assert_eq!(host.editor.document(), &project);
        assert_eq!(load_preferences_at(&chosen).unwrap(), preferences);
        assert_eq!(
            crate::editor_state::EditorState::open(chosen.join("rust-editor-state.json"))
                .unwrap()
                .state
                .gradients
                .len(),
            1
        );
        assert!(!chosen.join("localization.csv").exists());
        std::fs::write(chosen.join("rust-editor-state.json"), b"invalid").unwrap();
        assert!(host.change_settings_folder(&chosen).is_err());
        assert_eq!(
            std::fs::read(chosen.join("rust-editor-state.json")).unwrap(),
            b"invalid"
        );
        assert_eq!(host.editor.document(), &project);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preparing_a_saved_sync_snapshot_preserves_cancelled_save_as_asset_origins() {
        let root = crate::test_output::root().join(format!("sync-origin-{}", new_id()));
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .save(first.join("atlas.png"))
            .unwrap();
        let mut host = dirty_host();
        host.execute(
            "Steam ID",
            Command::ProjectSettings {
                name: "Sync origin".into(),
                app_id: 480,
                atlas: None,
                assets: None,
            },
            None,
        )
        .unwrap();
        let original_path = first.join("project.achv");
        host.store
            .save(host.editor.document(), &original_path)
            .unwrap();
        let loaded = host.store.load(&original_path).unwrap();
        host.editor = Editor::new(loaded);
        let moved = second.join("renamed.achv");
        host.store.save(host.editor.document(), &moved).unwrap();
        let origin = host.store.asset_base().unwrap().to_path_buf();
        let before = host.editor.document().clone();
        let (saved_path, saved) = host.synchronization_input().unwrap();
        assert_eq!(host.store.asset_base(), Some(origin.as_path()));
        assert_eq!(host.editor.document(), &before);
        let old_art = resolve_reference(&before.project()["assets"]["atlas"], &origin)
            .unwrap()
            .canonicalize()
            .unwrap();
        let new_art = resolve_reference(&saved.project()["assets"]["atlas"], &saved_path)
            .unwrap()
            .canonicalize()
            .unwrap();
        // Save As retains a separate immutable copy in the new project's bank.
        // Cancellation preserves the editor's original base and identical bytes.
        assert_eq!(
            std::fs::read(&old_art).unwrap(),
            std::fs::read(&new_art).unwrap()
        );
        let adopted = host.store.adopt_saved_result(&saved_path, &saved).unwrap();
        host.execute(
            "Synchronization",
            Command::ApplySynchronization(Box::new(adopted)),
            None,
        )
        .unwrap();
        let check_art = |host: &Host| {
            let art = resolve_reference(
                &host.editor.document().project()["assets"]["atlas"],
                host.store.asset_base().unwrap(),
            )
            .unwrap();
            assert_eq!(
                std::fs::read(art).unwrap(),
                std::fs::read(&old_art).unwrap()
            );
        };
        check_art(&host);
        assert!(host.editor.undo());
        check_art(&host);
        assert!(host.editor.redo());
        check_art(&host);
        host.store
            .save(host.editor.document(), &saved_path)
            .unwrap();
        drop(host);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_active_sync_rejects_editor_mutations_and_cancel_is_terminal_to_review() {
        let mut host = dirty_host();
        let before = host.editor.document().clone();
        let (_sender, events) = mpsc::sync_channel(1);
        let (approval, receiver) = mpsc::sync_channel(1);
        host.synchronization = Some(crate::synchronization::Pending {
            cancel: Cancellation::default(),
            events,
            approval,
            review: Some(json!({"token":"review"})),
            revision: host.editor.revision(),
            path: crate::test_output::root().join("unused-sync.achv"),
            thread: None,
            previews: Default::default(),
        });
        assert!(
            host.execute("Blocked edit", Command::Add(EntityKind::Stat), None)
                .is_err()
        );
        host.undo();
        host.redo();
        host.request(UiRequest::NewProject);
        assert_eq!(host.editor.document(), &before);
        host.request(UiRequest::ApplySynchronization(vec![(
            "Stat:SCORE".into(),
            "Upload".into(),
        )]));
        assert_eq!(receiver.try_recv().unwrap().len(), 1);
        assert!(host.synchronization.as_ref().unwrap().review.is_none());
        host.request(UiRequest::CancelSynchronization);
        assert!(
            host.synchronization
                .as_ref()
                .unwrap()
                .cancel
                .check()
                .is_err()
        );
        assert_eq!(host.editor.document(), &before);
    }

    #[test]
    fn banner_handoff_advances_the_review_revision_preserves_undo_and_rejects_stale_edits() {
        for stale in [false, true] {
            let mut host = dirty_host();
            let root = crate::test_output::root().join(format!("banner-handoff-{}", new_id()));
            std::fs::create_dir(&root).unwrap();
            let path = root.join("game.achv");
            host.store.save(host.editor.document(), &path).unwrap();
            host.editor.mark_saved();
            let before = host.editor.document().clone();
            let mut background = ProjectStore::new();
            background.load(&path).unwrap();
            let reference = background
                .retain_art(b"host-owned-handoff-fixture")
                .unwrap();
            let mut project = before.project().clone();
            project["assets"]["images"].as_array_mut().unwrap().push(json!({"id":new_id(),"kind":"User","steamBannerAppId":before.app_id(),"displayName":"Game banner","image":reference}));
            background
                .save(
                    &Document::from_parts(project, before.catalog().clone()).unwrap(),
                    &path,
                )
                .unwrap();
            let saved = background.load(&path).unwrap();
            let (sender, events) = mpsc::sync_channel(4);
            let (approval, _receiver) = mpsc::sync_channel(1);
            let revision = host.editor.revision();
            host.synchronization = Some(crate::synchronization::Pending {
                cancel: Cancellation::default(),
                events,
                approval,
                review: None,
                revision,
                path: path.clone(),
                thread: None,
                previews: Default::default(),
            });
            if stale {
                host.editor
                    .execute("Intervening edit", Command::Add(EntityKind::Stat), None)
                    .unwrap();
            }
            let edited = host.editor.document().clone();
            sender
                .send(crate::synchronization::Event::Banner(Box::new(
                    saved.clone(),
                )))
                .unwrap();
            host.poll_synchronization();
            if stale {
                assert!(host.synchronization.is_none());
                assert_eq!(host.editor.document(), &edited);
            } else {
                assert_eq!(host.editor.document(), &saved);
                assert!(!host.editor.is_dirty());
                let pending = host.synchronization.as_ref().unwrap();
                assert_eq!(pending.revision, host.editor.revision());
                assert!(pending.revision > revision);
                assert!(pending.cancel.check().is_ok());
                host.poll_synchronization();
                assert!(
                    host.synchronization
                        .as_ref()
                        .unwrap()
                        .cancel
                        .check()
                        .is_ok()
                );
                sender
                    .send(crate::synchronization::Event::Complete(Err(
                        "Cancelled fixture review".into(),
                    )))
                    .unwrap();
                host.poll_synchronization();
                assert_eq!(ProjectStore::new().load(&path).unwrap(), saved);
                host.undo();
                assert_eq!(host.editor.document(), &before);
            }
            drop(host);
            drop(background);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn missing_app_id_rejects_sync_before_any_save_or_worker_is_started() {
        let mut host = dirty_host();
        let before = host.editor.document().clone();
        host.request(UiRequest::PrepareSynchronization);
        assert!(host.dialog.is_none());
        assert!(host.synchronization.is_none());
        assert!(host.status.contains("valid Steam App ID"));
        assert_eq!(host.editor.document(), &before);
    }

    #[test]
    fn validation_of_a_project_without_a_saved_path_requires_an_explicit_save() {
        for dirty in [false, true] {
            let mut host = dirty_host();
            host.execute(
                "Steam ID",
                Command::ProjectSettings {
                    name: "Unsaved project".into(),
                    app_id: 480,
                    atlas: None,
                    assets: None,
                },
                None,
            )
            .unwrap();
            if !dirty {
                host.editor.mark_saved();
            }
            let before = host.editor.document().clone();
            let revision = host.editor.revision();
            host.request(UiRequest::PrepareSynchronization);
            assert!(host.dialog.is_some());
            assert!(host.synchronization.is_none());
            assert!(host.store.path().is_none());
            assert!(host.status.contains("Save the project before validating"));
            // Dismissing the error never resumes validation or opens Save As.
            host.dialog = None;
            host.complete_dialog(DialogResult::Acknowledged);
            assert!(host.dialog.is_none());
            assert!(host.synchronization.is_none());
            assert!(host.store.path().is_none());
            assert_eq!(host.editor.document(), &before);
            assert_eq!(host.editor.revision(), revision);
            assert_eq!(host.editor.is_dirty(), dirty);
            host.undo();
            host.redo();
            assert_eq!(host.editor.document(), &before);
        }
    }

    fn dirty_host() -> Host {
        let mut host = Host::new(egui::Context::default());
        host.execute(
            "Add achievement",
            Command::Add(EntityKind::Achievement),
            None,
        )
        .unwrap();
        host
    }

    #[test]
    fn test_hosts_keep_recent_history_in_separate_settings_folders() {
        let mut first = dirty_host();
        let second = dirty_host();
        let folder = first.settings_folder().unwrap().to_owned();
        assert!(folder.starts_with(crate::test_output::root()));
        assert_ne!(first.settings_folder(), second.settings_folder());
        assert!(first.recent_projects.paths.is_empty());
        assert!(second.recent_projects.paths.is_empty());
        let project = folder.join("test.achv");
        first.recent_projects.remember(&project).unwrap();
        assert_eq!(first.recent_projects.paths, vec![project.clone()]);
        assert!(second.recent_projects.paths.is_empty());
        assert_eq!(
            RecentProjects::new(Some(folder.join("rust-recent-projects.json"))).paths,
            vec![project]
        );
        std::fs::remove_file(folder.join("rust-recent-projects.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn translation_handoff_is_pinned_undoable_and_saved_through_the_project_pair() {
        fn finish(host: &mut Host) {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while host.translations.is_some() {
                host.poll_translations();
                assert!(
                    std::time::Instant::now() < deadline,
                    "Translation job timed out"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let root = crate::test_output::root().join(format!("translation-host-{}", new_id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("handoff.csv");
        let mut host = dirty_host();
        host.editor.mark_saved();
        let before = host.editor.document().clone();
        host.complete_dialog(DialogResult::ExportTranslations(Some(path.clone())));
        host.request(UiRequest::ImportAchievementTranslations);
        assert!(host.dialog.is_none());
        finish(&mut host);
        assert_eq!(host.editor.document(), &before);
        assert!(!host.editor.is_dirty());
        let mut copy =
            achimeld_persistence::project::parse_catalog(&std::fs::read(&path).unwrap()).unwrap();
        let id = copy.rows.keys().next().unwrap().clone();
        copy.rows
            .get_mut(&id)
            .unwrap()
            .insert("French".into(), "Texte traduit".into());
        std::fs::write(
            &path,
            achimeld_persistence::project::write_catalog(&copy).unwrap(),
        )
        .unwrap();
        host.complete_dialog(DialogResult::ImportTranslations(Some(path.clone())));
        host.request(UiRequest::PrepareSynchronization);
        assert!(host.dialog.is_none());
        host.execute("New edit", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let newer = host.editor.document().clone();
        finish(&mut host);
        assert!(host.status.contains("Project changed"));
        assert_eq!(host.editor.document(), &newer);
        host.complete_dialog(DialogResult::ImportTranslations(Some(path)));
        finish(&mut host);
        assert!(host.status.contains("Imported 1 translated cells"));
        assert_eq!(
            host.editor.document().catalog().rows[&id]["French"],
            "Texte traduit"
        );
        host.undo();
        assert_eq!(host.editor.document(), &newer);
        host.redo();
        let project = root.join("translated.achv");
        host.save_to(&project).unwrap();
        let loaded = ProjectStore::new().load(&project).unwrap();
        assert_eq!(loaded.catalog(), host.editor.document().catalog());
        assert!(!host.editor.is_dirty());
        drop(host);
        assert!(root.is_absolute() && root.parent() == Some(crate::test_output::root().as_path()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn owned_art_survives_source_deletion_and_resize_reload_undo() {
        let root = crate::test_output::root().join(format!("art-bank-{}", new_id()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.png");
        RgbaImage::from_pixel(16, 8, image::Rgba([240, 80, 20, 255]))
            .save(&source)
            .unwrap();
        let mut host = dirty_host();
        let achievement = host
            .editor
            .document()
            .entities(achimeld_application::EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let prepared = crate::art_bank::prepare(source.clone(), None).unwrap();
        host.import_layer_image(&achievement, None, prepared)
            .unwrap();
        let original = host.editor.document().project()["assets"]["images"][0].clone();
        let id = original["id"].as_str().unwrap().to_owned();
        assert_eq!(
            original["importSource"],
            json!({"kind":"File","path":source})
        );
        assert_eq!(host.art_source_file(false, &id), Some(source.clone()));
        std::fs::remove_file(&source).unwrap();
        let retained = host.assets.image(&original["image"]).unwrap();
        assert_eq!(retained.dimensions(), (16, 8));
        host.replace_art(false, &id, None, Some([8, 4])).unwrap();
        wait_art(&mut host);
        let resized = host.editor.document().project()["assets"]["images"][0].clone();
        assert_eq!(resized["id"], original["id"]);
        assert_eq!(resized["pixelWidth"], 8);
        assert_eq!(resized["pixelHeight"], 4);
        assert_eq!(resized["importSource"], original["importSource"]);
        host.undo();
        assert_eq!(
            host.editor.document().project()["assets"]["images"][0],
            original
        );
        host.redo();
        assert_eq!(
            host.editor.document().project()["assets"]["images"][0],
            resized
        );
        let replacement = root.join("replacement.png");
        RgbaImage::from_pixel(4, 2, image::Rgba([0, 255, 60, 255]))
            .save(&replacement)
            .unwrap();
        host.replace_art(false, &id, Some(replacement.clone()), None)
            .unwrap();
        wait_art(&mut host);
        assert_eq!(
            host.editor.document().project()["assets"]["images"][0]["id"],
            id
        );
        assert_eq!(host.art_source_file(false, &id), Some(replacement));
        let saved = root.join("source-metadata.achv");
        host.save_to(&saved).unwrap();
        let loaded = ProjectStore::new().load(&saved).unwrap();
        assert_eq!(
            loaded.project()["assets"]["images"][0]["importSource"],
            host.editor.document().project()["assets"]["images"][0]["importSource"]
        );
        host.undo();
        assert_eq!(
            host.editor.document().project()["assets"]["images"][0],
            resized
        );
        assert_eq!(host.art_source_file(false, &id), Some(source));
        host.replace_art(false, &id, None, Some([16384, 16384]))
            .unwrap();
        wait_art(&mut host);
        assert_eq!(
            host.editor.document().project()["assets"]["images"][0],
            resized
        );
        assert!(host.status.contains("memory limit"));
    }
    #[test]
    fn atlas_source_survives_resize_and_default_atlas_reload_is_undoable() {
        let root = crate::test_output::root().join(format!("atlas-source-{}", new_id()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("atlas.png");
        RgbaImage::from_pixel(16, 8, image::Rgba([0, 255, 60, 255]))
            .save(&source)
            .unwrap();
        let mut host = dirty_host();
        host.import_asset(
            true,
            crate::art_bank::prepare(source.clone(), None).unwrap(),
        )
        .unwrap();
        let entry = host.editor.document().project()["assets"]["atlases"][0].clone();
        let id = entry["id"].as_str().unwrap();
        assert_eq!(host.art_source_file(true, id), Some(source.clone()));
        assert_eq!(host.art_source_file(true, "default"), Some(source.clone()));
        host.replace_art(true, id, None, Some([8, 4])).unwrap();
        wait_art(&mut host);
        assert_eq!(host.art_source_file(true, "default"), Some(source));

        // A legacy default atlas has no library entry or provenance.
        let mut project = host.editor.document().project().clone();
        project["assets"]["atlases"] = json!([]);
        project["assets"]
            .as_object_mut()
            .unwrap()
            .remove("atlasImportSource");
        host.editor = Editor::new(
            Document::from_parts(project, host.editor.document().catalog().clone()).unwrap(),
        );
        assert_eq!(host.art_source_file(true, "default"), None);
        host.replace_art(true, "default", None, Some([4, 2]))
            .unwrap();
        wait_art(&mut host);
        assert_eq!(host.art_source_file(true, "default"), None);
        host.replace_art(true, "default", Some(root.join("atlas.png")), None)
            .unwrap();
        wait_art(&mut host);
        assert_eq!(
            host.art_source_file(true, "default"),
            Some(root.join("atlas.png"))
        );
        host.undo();
        assert_eq!(host.art_source_file(true, "default"), None);
    }
    #[test]
    fn general_import_retains_owned_art_and_is_one_undo_step() {
        let mut host = dirty_host();
        let before = host.editor.document().clone();
        let achievement = before.entities(EntityKind::Achievement)[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut bytes = std::io::Cursor::new(Vec::new());
        RgbaImage::from_pixel(16, 8, image::Rgba([240, 80, 20, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        host.import_general_image(
            &achievement,
            crate::art_bank::Prepared {
                name: "General".into(),
                bytes: bytes.into_inner(),
                dimensions: [16, 8],
                source: crate::art_bank::ImportSource::Clipboard,
            },
        )
        .unwrap();
        let entity = host
            .editor
            .document()
            .entity(EntityKind::Achievement, &achievement)
            .unwrap();
        assert_eq!(entity["generalImageEntries"][0]["id"], 0);
        assert_eq!(
            entity["icon"],
            before
                .entity(EntityKind::Achievement, &achievement)
                .unwrap()["icon"]
        );
        let image = host.editor.document().project()["assets"]["images"][0].clone();
        assert_eq!(image["importSource"], json!({"kind":"Clipboard"}));
        assert_eq!(entity["generalImageEntries"][0]["imageId"], image["id"]);
        assert_eq!(
            host.assets.image(&image["image"]).unwrap().dimensions(),
            (16, 8)
        );
        let after = host.editor.document().clone();
        host.undo();
        assert_eq!(host.editor.document(), &before);
        host.redo();
        assert_eq!(host.editor.document(), &after);
    }
    fn wait_art(host: &mut Host) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while host.art_busy() {
            host.poll_art();
            assert!(
                std::time::Instant::now() < deadline,
                "Artwork did not complete"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn canvas_navigation_and_placement_keep_full_resolution_previews_through_history() {
        let mut host = dirty_host();
        let mut entity = host.editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        entity["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF4672DE"}]});
        host.execute(
            "Artwork",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let full = loop {
            host.previews.poll();
            if let Some(preview) = host.preview(&id)
                && preview.fingerprint.ends_with("Detail")
            {
                break preview;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Full preview did not complete"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!([full.width, full.height], [512, 512]);
        let generation = host.render_generation;
        let revision = host.editor.revision();
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            for zoom in [0.25, 1.0, 4.0] {
                host.execute(
                    "Camera",
                    Command::CanvasCamera {
                        kind,
                        zoom,
                        offset_x: 120.0 * zoom,
                        offset_y: -80.0 * zoom,
                    },
                    Some("camera"),
                )
                .unwrap();
            }
            host.finish_group();
        }
        host.execute(
            "Move",
            Command::Batch(vec![Command::Move {
                kind: EntityKind::Achievement,
                id: id.clone(),
                column: 5,
                row: -2,
            }]),
            None,
        )
        .unwrap();
        // Entity replacement is another entry point for placement changes.
        let mut entity = host.editor.document().entities(EntityKind::Achievement)[0].clone();
        entity["canvasColumn"] = json!(10);
        host.execute(
            "Placement",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
        assert!(host.editor.revision() > revision);
        for _ in 0..4 {
            host.undo();
        }
        for _ in 0..4 {
            host.redo();
        }
        assert_eq!(host.render_generation, generation);
        host.previews.poll();
        let after = host.preview(&id).unwrap();
        assert_eq!(after.fingerprint, full.fingerprint);
        assert!(Arc::ptr_eq(&after.rgba, &full.rgba));

        // Actual artwork still invalidates, including undo and redo.
        let mut entity = host.editor.document().entities(EntityKind::Achievement)[0].clone();
        entity["icon"]["layers"][0]["solidColorHex"] = json!("#FFFFFFFF");
        host.execute(
            "Artwork",
            Command::ReplaceEntity {
                kind: EntityKind::Achievement,
                id: id.clone(),
                value: entity,
            },
            None,
        )
        .unwrap();
        assert_eq!(host.render_generation, generation + 1);
        host.undo();
        assert_eq!(host.render_generation, generation + 2);
        host.redo();
        assert_eq!(host.render_generation, generation + 3);
    }

    fn assert_unchanged(host: &Host, document: &Document, revision: u64, generation: u64) {
        assert_eq!(host.editor.document(), document);
        assert_eq!(host.editor.revision(), revision);
        assert!(host.editor.is_dirty());
        assert_eq!(host.render_generation, generation);
        assert!(!host.exit_allowed);
        assert!(host.dialog.is_none());
        assert!(host.store.path().is_none());
    }

    #[test]
    fn cancelling_open_and_unsaved_prompts_preserves_the_document_and_undo() {
        let mut host = dirty_host();
        let document = host.editor.document().clone();
        let revision = host.editor.revision();
        let generation = host.render_generation;
        for result in [
            DialogResult::Open(None),
            DialogResult::SettingsFolder(None),
            DialogResult::Discard(UiRequest::OpenProject, SaveChoice::Cancel),
            DialogResult::Discard(
                UiRequest::OpenRecentProject(PathBuf::from("recent.achv")),
                SaveChoice::Cancel,
            ),
            DialogResult::Discard(UiRequest::NewProject, SaveChoice::Cancel),
            DialogResult::Discard(UiRequest::Exit, SaveChoice::Cancel),
            DialogResult::Save(Some(UiRequest::OpenProject), None),
            DialogResult::Save(
                Some(UiRequest::OpenRecentProject(PathBuf::from("recent.achv"))),
                None,
            ),
            DialogResult::Save(Some(UiRequest::NewProject), None),
            DialogResult::Save(Some(UiRequest::Exit), None),
            DialogResult::Save(None, None),
            DialogResult::Import(false, None),
            DialogResult::Import(true, None),
            DialogResult::Export(None),
            DialogResult::ExportTranslations(None),
            DialogResult::ImportTranslations(None),
        ] {
            host.complete_dialog(result);
            assert_unchanged(&host, &document, revision, generation);
        }
        host.undo();
        assert!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .is_empty()
        );
        host.redo();
        assert_eq!(host.editor.document(), &document);
    }

    #[test]
    fn pending_app_prompt_blocks_replacement_requests_and_preserves_document() {
        let mut host = dirty_host();
        crate::ui_fonts::configure(&host.ctx);
        let document = host.editor.document().clone();
        let revision = host.editor.revision();
        let generation = host.render_generation;
        host.start_dialog(Dialog::Discard(UiRequest::OpenProject));
        host.request(UiRequest::NewProject);
        host.request(UiRequest::OpenProject);
        host.request(UiRequest::OpenRecentProject(PathBuf::from("recent.achv")));
        host.request(UiRequest::Exit);
        assert!(host.dialog.is_some());
        assert_eq!(host.editor.document(), &document);
        assert!(!host.exit_allowed);
        for _ in 0..2 {
            let mut output = host
                .ctx
                .clone()
                .run_ui(egui::RawInput::default(), |ui| host.show_dialog(ui.ctx()));
            output.textures_delta.clear();
        }
        let mut result = host.ctx.clone().run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| host.show_dialog(ui.ctx()),
        );
        result.textures_delta.clear();
        assert!(host.dialog.is_none());
        assert_unchanged(&host, &document, revision, generation);
    }

    #[test]
    fn failed_save_before_open_does_not_continue_or_discard_unsaved_changes() {
        let mut host = dirty_host();
        let document = host.editor.document().clone();
        let revision = host.editor.revision();
        let generation = host.render_generation;
        host.complete_dialog(DialogResult::Save(
            Some(UiRequest::OpenProject),
            Some(PathBuf::from("not-a-project.txt")),
        ));
        assert_unchanged(&host, &document, revision, generation);
        assert_ne!(host.status, "Project saved");
    }

    #[test]
    fn recent_open_and_save_use_validated_storage_and_pin_selection_across_save() {
        let root = crate::test_output::root().join(format!("recent-host-{}", new_id()));
        let root = std::path::absolute(root).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        let state = root.join("recent.json");
        let chosen = root.join("chosen.achv");
        ProjectStore::new().save(&Document::new(), &chosen).unwrap();
        let mut host = dirty_host();
        host.recent_projects = RecentProjects::new(Some(state.clone()));
        host.recent_projects.remember(&chosen).unwrap();
        let saved = root.join("saved.achv");
        // Save before switching moves 'saved' ahead of 'chosen'; the continuation
        // must still reopen the originally selected project.
        host.complete_dialog(DialogResult::Save(
            Some(UiRequest::OpenRecentProject(chosen.clone())),
            Some(saved.clone()),
        ));
        assert_eq!(host.store.path(), Some(chosen.as_path()));
        assert_eq!(host.status, "Project opened");
        assert!(!host.editor.is_dirty());
        assert_eq!(
            host.recent_projects.paths,
            vec![chosen.clone(), saved.clone()]
        );
        assert_eq!(
            RecentProjects::new(Some(state)).paths,
            host.recent_projects.paths
        );
        assert!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .is_empty()
        );
        assert!(host.editor.undo_label().is_none());

        host.execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let before = host.editor.document().clone();
        let history = host.recent_projects.paths.clone();
        // Missing or corrupt inputs preserve the edited document and navigation history.
        assert!(host.open(&root.join("missing.achv")).is_err());
        std::fs::write(&saved, b"invalid").unwrap();
        host.complete_dialog(DialogResult::Discard(
            UiRequest::OpenRecentProject(saved.clone()),
            SaveChoice::Discard,
        ));
        assert_eq!(host.editor.document(), &before);
        assert!(host.editor.is_dirty());
        assert_eq!(host.recent_projects.paths, history);
        host.request(UiRequest::OpenRecentProject(root.join("unlisted.achv")));
        assert!(host.dialog.is_none());
        assert_eq!(host.editor.document(), &before);
        assert_eq!(host.status, "This project is no longer in the recent list");
        // A failed history write does not turn a successful project save into a failure.
        host.recent_projects = RecentProjects::new(Some(root.clone()));
        host.save_to(&chosen).unwrap();
        assert!(!host.editor.is_dirty());
        assert!(
            host.status
                .starts_with("Project saved · Could not remember")
        );
        assert_eq!(host.recent_projects.paths, vec![chosen]);
        drop(host);
        for file in std::fs::read_dir(&root).unwrap() {
            std::fs::remove_file(file.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
