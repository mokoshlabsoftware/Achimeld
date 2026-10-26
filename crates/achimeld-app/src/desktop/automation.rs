//! Requests are serialized on the UI thread; image decoding/export stay on bounded background workers.
use super::*;
use achimeld_application::{EntityKind, automation as edits};
use achimeld_mcp::{Request, failure, success};
use achimeld_security::{SecurityError, editor_transport::Pipe};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, time::Instant};

struct Pending {
    cancelled: Arc<AtomicBool>,
    request: Request,
    artwork: Option<crate::art_bank::Prepared>,
    translation: Option<(Command, usize)>,
    reply: mpsc::SyncSender<Value>,
    deadline: Instant,
}
pub(super) struct Server {
    receiver: mpsc::Receiver<Pending>,
    stopped: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub(super) fn start(ctx: egui::Context) -> Result<Self, String> {
        let first = Pipe::create().map_err(|e| e.to_string())?;
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let mut first = Some(first);
            while !stop.load(Ordering::Acquire) {
                let mut pipe = match first.take().map(Ok).unwrap_or_else(Pipe::create) {
                    Ok(pipe) => pipe,
                    Err(_) => break,
                };
                let mut check = || {
                    if stop.load(Ordering::Acquire) {
                        Err(SecurityError("Editor stopped"))
                    } else {
                        Ok(())
                    }
                };
                if pipe.accept(&mut check).is_err() {
                    continue;
                }
                let request: Request = match pipe.read(Duration::from_secs(15), &mut check) {
                    Ok(request) => request,
                    Err(_) => continue,
                };
                let timeout =
                    Duration::from_secs(if request.operation == "export_achievement_icons" {
                        600
                    } else {
                        20
                    });
                let deadline = Instant::now() + timeout;
                let prepared = validate(&request);
                let response = match prepared {
                    Err(error) => failure("invalid_request", &error),
                    Ok(()) => {
                        let (reply, replies) = mpsc::sync_channel(1);
                        let cancelled = Arc::new(AtomicBool::new(false));
                        if sender
                            .try_send(Pending {
                                cancelled: cancelled.clone(),
                                request,
                                artwork: None,
                                translation: None,
                                reply,
                                deadline,
                            })
                            .is_err()
                        {
                            continue;
                        }
                        ctx.request_repaint();
                        loop {
                            if stop.load(Ordering::Acquire)
                                || Instant::now() >= deadline
                                || !pipe.connected()
                            {
                                cancelled.store(true, Ordering::Release);
                                break failure(
                                    "timeout",
                                    "Read the scene; retry only with the identical requestId and arguments",
                                );
                            }
                            match replies.recv_timeout(Duration::from_millis(10)) {
                                Ok(result) => break result,
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(_) => break failure("editor_closed", "Editor unavailable"),
                            }
                        }
                    }
                };
                let response = if serde_json::to_vec(&response)
                    .map_or(true, |bytes| bytes.len() > achimeld_mcp::MAXIMUM_BYTES)
                {
                    failure(
                        "response_too_large",
                        "Narrow the request; response exceeds 16 MiB",
                    )
                } else {
                    response
                };
                let _ = pipe.write(&response, Duration::from_secs(5), &mut check);
                // Keep the server handle alive until the client has consumed the response.
                let _: Result<String, _> = pipe.read(Duration::from_secs(2), &mut check);
            }
        });
        Ok(Self {
            receiver,
            stopped,
            thread: Some(thread),
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn validate(request: &Request) -> Result<(), String> {
    if request.operation == "list_instances" {
        return Err("Discovery belongs to the MCP facade".into());
    }
    let mut args = request.arguments.clone();
    let map = args.as_object_mut().ok_or("Arguments must be an object")?;
    if ["processId", "projectId", "expectedRevision", "requestId"]
        .iter()
        .any(|key| map.contains_key(*key))
    {
        return Err("Envelope fields cannot appear in operation arguments".into());
    }
    map.insert("processId".into(), json!(std::process::id()));
    if !read_only(&request.operation) {
        for (key, id) in [
            ("projectId", &request.project_id),
            ("requestId", &request.request_id),
        ] {
            let id = id.as_deref().ok_or("Mutation identity missing")?;
            if !valid_id(id) {
                return Err("Mutation identities must be nonzero GUIDs".into());
            }
            map.insert(key.into(), json!(id));
        }
        map.insert(
            "expectedRevision".into(),
            json!(request.expected_revision.ok_or("Revision missing")?),
        );
    }
    achimeld_mcp::validate_arguments(&request.operation, &args)
}
fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id != "00000000-0000-0000-0000-000000000000"
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn read_only(operation: &str) -> bool {
    matches!(
        operation,
        "get_scene"
            | "get_node"
            | "list_icon_styles"
            | "get_achievement_translations"
            | "list_style_collections"
            | "get_icon_style"
    )
}

fn translation_operation(operation: &str) -> bool {
    matches!(
        operation,
        "get_achievement_translations"
            | "export_achievement_translations"
            | "import_achievement_translations"
    )
}

struct Cached {
    id: String,
    hash: [u8; 32],
    response: Value,
    bytes: usize,
}
enum Output {
    Artwork(crate::art_bank::Prepared),
    Export(Value),
    TranslationImport(Box<Command>, usize),
}
struct Working {
    pending: Pending,
    receiver: mpsc::Receiver<Result<Output, String>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    envelope: Value,
    session: String,
}
impl Drop for Working {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[derive(Default)]
pub(super) struct Engine {
    observed: Option<(String, u64, Value, u64)>,
    revision: u64,
    cache: VecDeque<Cached>,
    bytes: usize,
    work: Option<Working>,
}
impl Engine {
    fn observe(&mut self, host: &Host, ui: &achimeld_ui::Desktop) {
        let current = (
            host.editor_session_id.clone(),
            host.editor.revision(),
            ui.automation_scene(),
            host.style_viewer
                .as_ref()
                .map_or(0, super::style_viewer::Viewer::library_revision),
        );
        if self
            .observed
            .as_ref()
            .is_some_and(|old| old.0 != current.0 || old.1 > current.1)
        {
            self.cache.clear();
            self.bytes = 0;
        }
        if self.observed.as_ref() != Some(&current) {
            self.revision += 1;
            self.observed = Some(current);
        }
    }
    fn envelope(&self, host: &Host, result: Value) -> Value {
        json!({"projectId":host.editor.document().project()["projectId"],"revision":self.revision,"result":result})
    }
    pub(super) fn poll(&mut self, server: &Server, host: &mut Host, ui: &mut achimeld_ui::Desktop) {
        self.observe(host, ui);
        if let Some(work) = &self.work {
            match work.receiver.try_recv() {
                Ok(result) => {
                    let mut work = self.work.take().unwrap();
                    let response = match result {
                        Ok(Output::Export(data)) => {
                            work.envelope["result"] = data;
                            let response = success(work.envelope.clone());
                            if work.session == host.editor_session_id {
                                self.cache(&work.pending.request, response.clone());
                            }
                            response
                        }
                        Ok(Output::Artwork(art)) => {
                            work.pending.artwork = Some(art);
                            self.complete_prepared(&work, host, ui)
                        }
                        Ok(Output::TranslationImport(command, count)) => {
                            work.pending.translation = Some((*command, count));
                            self.complete_prepared(&work, host, ui)
                        }
                        Err(e) => failure("operation_failed", &e),
                    };
                    let _ = work.pending.reply.try_send(response);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(_) => {
                    if let Some(work) = self.work.take() {
                        let _ = work
                            .pending
                            .reply
                            .try_send(failure("operation_failed", "Artwork worker stopped"));
                    }
                }
            }
        }
        if let Ok(pending) = server.receiver.try_recv() {
            if Instant::now() >= pending.deadline || pending.cancelled.load(Ordering::Acquire) {
                return;
            }
            let request = &pending.request;
            if let Some(response) = self.preflight(request, host, ui) {
                let _ = pending.reply.try_send(response);
                return;
            }
            if translation_operation(&request.operation)
                || matches!(
                    request.operation.as_str(),
                    "export_achievement_icons" | "export_style_preview"
                )
                || (request.operation == "add_general_image"
                    && !request.arguments["filePath"].is_null())
            {
                let mut document = host.editor.document().clone();
                let mut style_achievement = None;
                if request.operation == "export_style_preview" {
                    let prepared = (|| {
                        host.ensure_style_viewer()?;
                        host.style_viewer
                            .as_ref()
                            .ok_or("Style Viewer unavailable")?
                            .style_export_document(
                                request.arguments["collectionId"]
                                    .as_str()
                                    .ok_or("Missing collectionId")?,
                                request.arguments["styleId"]
                                    .as_str()
                                    .ok_or("Missing styleId")?,
                            )
                    })();
                    match prepared {
                        Ok((preview, achievement)) => {
                            document = preview;
                            style_achievement = Some(achievement);
                        }
                        Err(error) => {
                            let _ = pending.reply.try_send(failure("invalid_request", &error));
                            return;
                        }
                    }
                }
                let args = request.arguments.clone();
                let base = host.assets.project.clone();
                let stop = Arc::new(AtomicBool::new(false));
                let cancelled = stop.clone();
                let request_cancelled = pending.cancelled.clone();
                let deadline = pending.deadline;
                let ctx = host.ctx.clone();
                let (tx, receiver) = mpsc::sync_channel(1);
                let export = request.operation == "export_achievement_icons";
                let operation = request.operation.clone();
                self.observe(host, ui);
                let thread = std::thread::spawn(move || {
                    let cancelled = || {
                        cancelled.load(Ordering::Acquire)
                            || request_cancelled.load(Ordering::Acquire)
                            || Instant::now() >= deadline
                    };
                    let result = if translation_operation(&operation) {
                        prepare_translations(&document, &operation, &args, &cancelled)
                    } else if let Some(achievement) = style_achievement {
                        export_style_preview(&document, &achievement, &args, base, &cancelled)
                            .map(Output::Export)
                    } else if export {
                        export_icons(&document, &args, base, &cancelled).map(Output::Export)
                    } else {
                        prepare_art(&args, &cancelled).map(Output::Artwork)
                    };
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
                self.work = Some(Working {
                    envelope: self.envelope(host, Value::Null),
                    session: host.editor_session_id.clone(),
                    pending,
                    receiver,
                    stop,
                    thread: Some(thread),
                });
                return;
            }
            let response = self.complete(&pending, host, ui);
            let _ = pending.reply.try_send(response);
        }
    }
    fn complete_prepared(
        &mut self,
        work: &Working,
        host: &mut Host,
        ui: &mut achimeld_ui::Desktop,
    ) -> Value {
        if Instant::now() >= work.pending.deadline || work.pending.cancelled.load(Ordering::Acquire)
        {
            failure("timeout", "Request expired without editing")
        } else if work.session != host.editor_session_id {
            failure("stale_scene", "The editor project session changed")
        } else if let Some(response) = self.preflight(&work.pending.request, host, ui) {
            response
        } else {
            self.complete(&work.pending, host, ui)
        }
    }
    fn complete(
        &mut self,
        pending: &Pending,
        host: &mut Host,
        ui: &mut achimeld_ui::Desktop,
    ) -> Value {
        if Instant::now() >= pending.deadline || pending.cancelled.load(Ordering::Acquire) {
            return failure("timeout", "Request expired without editing");
        }
        match self.apply(pending, host, ui) {
            Ok(result) => {
                self.observe(host, ui);
                let response = success(self.envelope(host, result));
                if !read_only(&pending.request.operation) {
                    self.cache(&pending.request, response.clone());
                }
                host.ctx.request_repaint();
                response
            }
            Err(error) => failure("invalid_request", &error),
        }
    }
    fn preflight(
        &self,
        request: &Request,
        host: &Host,
        ui: &achimeld_ui::Desktop,
    ) -> Option<Value> {
        if let Err(error) = validate(request) {
            return Some(failure("invalid_request", &error));
        }
        if request.operation == "get_achievement_translations" && self.work.is_some() {
            return Some(failure(
                "editor_busy",
                "Finish the active editor operation before retrying",
            ));
        }
        if read_only(&request.operation) {
            return None;
        }
        let hash: [u8; 32] = Sha256::digest(serde_json::to_vec(request).unwrap_or_default()).into();
        if let Some(cached) = self
            .cache
            .iter()
            .find(|entry| Some(entry.id.as_str()) == request.request_id.as_deref())
        {
            return Some(if cached.hash == hash {
                cached.response.clone()
            } else {
                failure(
                    "request_id_reused",
                    "Use a new requestId for different arguments",
                )
            });
        }
        if request.project_id.as_deref() != host.editor.document().project()["projectId"].as_str()
            || request.expected_revision != Some(self.revision)
        {
            return Some(failure(
                "stale_scene",
                "Read get_scene again and use its projectId and revision",
            ));
        }
        if host.dialog.is_some()
            || host.synchronization.is_some()
            || host.read.is_some()
            || host.art.is_some()
            || host.image_drop.is_some()
            || !host.art_queue.is_empty()
            || host.export.is_some()
            || host.translations.is_some()
            || self.work.is_some()
            || ui.automation_busy()
            || host.style_viewer.as_ref().is_some_and(|v| v.state.busy)
        {
            return Some(failure(
                "editor_busy",
                "Finish the active editor operation before retrying",
            ));
        }
        None
    }
    fn cache(&mut self, request: &Request, response: Value) {
        let Some(id) = request.request_id.clone() else {
            return;
        };
        let bytes = serde_json::to_vec(&response).map_or(0, |b| b.len());
        let hash = Sha256::digest(serde_json::to_vec(request).unwrap_or_default()).into();
        self.cache.push_back(Cached {
            id,
            hash,
            response,
            bytes,
        });
        self.bytes += bytes;
        while self.cache.len() > 256 || self.bytes > 16 * 1024 * 1024 {
            if let Some(old) = self.cache.pop_front() {
                self.bytes -= old.bytes;
            }
        }
    }
    fn apply(
        &mut self,
        pending: &Pending,
        host: &mut Host,
        ui: &mut achimeld_ui::Desktop,
    ) -> Result<Value, String> {
        let request = &pending.request;
        let args = &request.arguments;
        match request.operation.as_str() {
            "list_style_collections"
            | "get_icon_style"
            | "create_style_collection"
            | "create_icon_style"
            | "export_style_collection" => {
                host.ensure_style_viewer()?;
                let result = host
                    .style_viewer
                    .as_mut()
                    .ok_or("Style Viewer unavailable")?
                    .collection_operation(&request.operation, args)?;
                if request.operation == "export_style_collection" {
                    host.persist_viewer_collections();
                }
                Ok(result)
            }
            "import_icon_style" => host.import_collection_style(
                args["collectionId"]
                    .as_str()
                    .ok_or("Missing collectionId")?,
                args["styleId"].as_str().ok_or("Missing styleId")?,
            ),
            "import_achievement_translations" => {
                let (command, count) = pending
                    .translation
                    .as_ref()
                    .ok_or("Translation import has not been prepared")?;
                host.finish_group();
                host.execute(
                    "MCP: import achievement translations",
                    command.clone(),
                    None,
                )?;
                host.status = crate::desktop::logged_status(format!(
                    "Imported {count} translated cells · save the project to keep them"
                ));
                Ok(json!({"changedCells":count,"saved":false}))
            }
            "get_scene" => {
                let document = host.editor.document();
                Ok(
                    json!({"project":{"id":document.project()["projectId"],"displayName":document.project()["displayName"],"steamAppId":document.app_id()},"canvas":ui.automation_scene(),"achievements":edits::summaries(document,EntityKind::Achievement),"stats":edits::summaries(document,EntityKind::Stat),"assets":document.project()["assets"],"isDirty":host.editor.is_dirty(),"canUndo":host.editor.undo_label().is_some(),"canRedo":host.editor.redo_label().is_some(),"undoLabel":host.editor.undo_label(),"redoLabel":host.editor.redo_label()}),
                )
            }
            "get_node" => Ok(edits::node(
                host.editor.document(),
                args["nodeId"].as_str().ok_or("Missing nodeId")?,
            )?
            .1),
            "list_icon_styles" => {
                let mut styles = host.editor.document().icon_styles();
                for style in host.global_icon_styles() {
                    if !styles.iter().any(|s| s.id == style.id) {
                        styles.push(style.clone());
                    }
                }
                Ok(json!(styles))
            }
            "select_node" => {
                let id = args["nodeId"].as_str().ok_or("Missing nodeId")?;
                let (kind, _) = edits::node(host.editor.document(), id)?;
                host.finish_group();
                ui.automation_select(kind, id.into(), host);
                Ok(json!({"nodeId":id}))
            }
            "undo" | "redo" => {
                host.finish_group();
                let path = host
                    .settings
                    .as_ref()
                    .map(|l| l.path("rust-preferences.json"));
                let before = host.editor.preview_revision();
                let persist = |a: &Preferences, b: &Preferences| {
                    super::persist_preferences_at(a, b, path.as_deref())
                };
                let changed = if request.operation == "undo" {
                    host.editor.undo_with(persist)?
                } else {
                    host.editor.redo_with(persist)?
                };
                if before != host.editor.preview_revision() {
                    host.render_generation += 1;
                }
                Ok(json!({"changed":changed}))
            }
            "add_general_image" if pending.artwork.is_some() => {
                let id = args["achievementId"]
                    .as_str()
                    .ok_or("Missing achievementId")?;
                let mut entity = host
                    .editor
                    .document()
                    .entity(EntityKind::Achievement, id)
                    .ok_or("Unknown achievement ID")?
                    .clone();
                let art = pending.artwork.as_ref().unwrap();
                let retained = host.store.retain_art(&art.bytes)?;
                let existing = host.editor.document().project()["assets"]["images"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|entry| entry["image"] == retained)
                    .and_then(|entry| entry["id"].as_str())
                    .map(str::to_owned);
                let (asset, image) = if let Some(image) = existing {
                    (None, image)
                } else {
                    let (command, id) = host.import_asset_command(
                        false,
                        crate::art_bank::Prepared {
                            name: args["displayName"].as_str().unwrap_or(&art.name).into(),
                            bytes: art.bytes.clone(),
                            dimensions: art.dimensions,
                            source: art.source.clone(),
                        },
                    )?;
                    (Some(command), id)
                };
                let general = achimeld_application::append_general_image(
                    &mut entity,
                    json!({"imageId":image,"atlasId":null,"tileIndex":0}),
                );
                host.finish_group();
                let mut commands: Vec<_> = asset.into_iter().collect();
                commands.push(Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.into(),
                    value: entity,
                });
                host.execute("MCP: add general image", Command::Batch(commands), None)?;
                Ok(
                    json!({"nodeId":id,"generalImageId":general,"imageAssetId":image,"atlasId":null}),
                )
            }
            operation => {
                let prepared_style = if operation == "apply_icon_style"
                    && !host
                        .editor
                        .document()
                        .icon_styles()
                        .iter()
                        .any(|style| Some(style.id.as_str()) == args["styleId"].as_str())
                {
                    let style = host
                        .global_icon_styles()
                        .iter()
                        .find(|style| Some(style.id.as_str()) == args["styleId"].as_str())
                        .cloned()
                        .ok_or("Unknown icon style")?;
                    let mut candidate = Editor::new(host.editor.document().clone());
                    candidate.execute("Import saved style", Command::SaveIconStyle(style), None)?;
                    Some(candidate)
                } else {
                    None
                };
                let document = prepared_style
                    .as_ref()
                    .map_or(host.editor.document(), Editor::document);
                let edit = edits::prepare(document, operation, args)?;
                host.finish_group();
                host.execute(
                    &format!("MCP: {operation}"),
                    Command::ApplyAutomation(Box::new(edit.document)),
                    None,
                )?;
                if let Some((kind, id)) = edit.selection {
                    ui.automation_select(kind, id, host);
                }
                Ok(edit.result)
            }
        }
    }
}

fn prepare_translations(
    document: &Document,
    operation: &str,
    args: &Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<Output, String> {
    let absolute_path = |key: &str| -> Result<PathBuf, String> {
        let path = PathBuf::from(args[key].as_str().ok_or("Missing CSV path")?);
        if !local_path(&path) {
            return Err("Translation CSV paths must use an absolute local drive".into());
        }
        let existing = if path.exists() {
            path.as_path()
        } else {
            path.parent().ok_or("Missing CSV parent folder")?
        };
        achimeld_security::windows::reject_reparse(existing).map_err(|e| e.to_string())?;
        Ok(path)
    };
    if cancelled() {
        return Err("Translation operation cancelled".into());
    }
    match operation {
        "get_achievement_translations" => {
            let bytes = crate::translations::csv_bytes(document)?;
            if bytes.len() > achimeld_mcp::MAXIMUM_INLINE_TRANSLATION_BYTES {
                return Err("Translation CSV exceeds 2 MiB inline; use export_achievement_translations to a file".into());
            }
            let csv = String::from_utf8(bytes).map_err(|error| error.to_string())?;
            Ok(Output::Export(
                json!({"csv":csv,"filename":"achievements.translations.csv","encoding":"UTF-8","rowCount":achimeld_application::translations::achievement_catalog(document)?.rows.len(),"languages":achimeld_core::LANGUAGES}),
            ))
        }
        "export_achievement_translations" => {
            let path = absolute_path("destinationFilePath")?;
            crate::translations::export_cancellable(document, &path, cancelled)?;
            Ok(Output::Export(
                json!({"filePath":path,"encoding":"UTF-8 with BOM","rowCount":achimeld_application::translations::achievement_catalog(document)?.rows.len()}),
            ))
        }
        "import_achievement_translations" => {
            if args["csv"].is_string() == args["filePath"].is_string() {
                return Err("Supply exactly one of csv or filePath".into());
            }
            let (command, count) = if let Some(csv) = args["csv"].as_str() {
                if csv.len() > achimeld_mcp::MAXIMUM_INLINE_TRANSLATION_BYTES {
                    return Err("Inline translation CSV exceeds 2 MiB; use filePath".into());
                }
                let catalog = achimeld_persistence::project::parse_catalog(csv.as_bytes())?;
                achimeld_application::translations::import_achievements(document, &catalog)?
            } else {
                crate::translations::read(document, &absolute_path("filePath")?)?
            };
            if cancelled() {
                return Err("Translation import cancelled without editing".into());
            }
            Ok(Output::TranslationImport(Box::new(command), count))
        }
        _ => Err("Unknown translation operation".into()),
    }
}

fn export_style_preview(
    document: &Document,
    achievement: &str,
    args: &Value,
    base: Option<PathBuf>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, String> {
    use std::io::Write;
    let path = PathBuf::from(
        args["destinationFilePath"]
            .as_str()
            .ok_or("Missing destinationFilePath")?,
    );
    if !local_path(&path)
        || path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("png"))
    {
        return Err(
            "Style previews require an unused absolute local-drive .png destination".into(),
        );
    }
    achimeld_security::windows::reject_reparse(path.parent().ok_or("Missing destination folder")?)
        .map_err(|e| e.to_string())?;
    if path.exists() {
        return Err("Style preview destination already exists".into());
    }
    let width = args["width"].as_u64().unwrap_or(512);
    let height = args["height"].as_u64().unwrap_or(512);
    if !(1..=1024).contains(&width) || !(1..=1024).contains(&height) {
        return Err("Style preview dimensions must be 1–1024".into());
    }
    let mut assets = Files::new();
    assets.project = base;
    let rendered = achimeld_imaging::render_cancellable(
        document,
        achievement,
        Some((width as u32, height as u32)),
        &mut assets,
        cancelled,
    )?
    .achieved;
    let mut png = std::io::Cursor::new(Vec::new());
    rendered
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    if cancelled() {
        return Err("Style preview export cancelled".into());
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    output
        .write_all(png.get_ref())
        .and_then(|_| output.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(json!({"filePath":path,"width":width,"height":height}))
}

fn prepare_art(
    args: &Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<crate::art_bank::Prepared, String> {
    let supplied = ["imageAssetId", "atlasId", "filePath"]
        .iter()
        .filter(|key| !args[**key].is_null())
        .count();
    if supplied != 1 {
        return Err("Supply exactly one artwork source".into());
    }
    let path = PathBuf::from(args["filePath"].as_str().ok_or("Missing filePath")?);
    if !local_path(&path) {
        return Err("filePath must be absolute".into());
    }
    achimeld_security::windows::reject_reparse(&path).map_err(|e| e.to_string())?;
    crate::art_bank::prepare_progressive(path, None, |_| {}, cancelled)
}
pub(super) fn local_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.is_absolute()
        && matches!(path.components().next(),Some(Component::Prefix(prefix))if matches!(prefix.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
}
fn export_icons(
    document: &Document,
    args: &Value,
    base: Option<PathBuf>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, String> {
    let directory = PathBuf::from(
        args["destinationDirectory"]
            .as_str()
            .ok_or("Missing destinationDirectory")?,
    );
    if !local_path(&directory) || !directory.is_dir() {
        return Err("Destination must be an existing absolute directory".into());
    }
    achimeld_security::windows::reject_reparse(&directory).map_err(|e| e.to_string())?;
    let format = args["format"].as_str().unwrap_or("Png");
    let (encoding, extension) = match format {
        "Png" => (image::ImageFormat::Png, "png"),
        "Jpeg" => (image::ImageFormat::Jpeg, "jpg"),
        "Bmp" => (image::ImageFormat::Bmp, "bmp"),
        "Tiff" => (image::ImageFormat::Tiff, "tiff"),
        _ => return Err("Unsupported icon format".into()),
    };
    let width = args["width"].as_u64().unwrap_or(256) as u32;
    let height = args["height"].as_u64().unwrap_or(256) as u32;
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        return Err("Icon dimensions must be 1–4096".into());
    }
    let mut assets = Files::new();
    assets.project = base;
    let mut names = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut failures = Vec::new();
    let mut count = 0;
    // Preflight all names before writing: gray pairs must never collide with another API name.
    let mut outputs = Vec::new();
    let mut reply_bytes = 0;
    for entity in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| !achimeld_application::art_images::is_art_image(entity))
    {
        let api = entity["apiName"].as_str().ok_or("Missing API name")?;
        let stem: String = api
            .chars()
            .map(|c| {
                if c.is_control() || "<>:\"/\\|?*".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let stem = stem.trim_end_matches(['.', ' ']);
        let device = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
        let stem = if stem.is_empty()
            || [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ]
            .contains(&device.as_str())
        {
            format!("_{stem}")
        } else {
            stem.to_owned()
        };
        let pair = [
            directory.join(format!("{stem}.{extension}")),
            directory.join(format!("{stem}_gray.{extension}")),
        ];
        for path in &pair {
            if !names.insert(path.to_string_lossy().to_lowercase()) {
                return Err("Icon export filenames collide".into());
            }
            if path.exists() {
                achimeld_security::windows::reject_reparse(path).map_err(|e| e.to_string())?;
            }
        }
        reply_bytes += serde_json::to_vec(&pair).map_err(|e| e.to_string())?.len() + 512;
        if reply_bytes > 8 * 1024 * 1024 {
            return Err("Export file inventory exceeds the response budget; use a shorter destination path or a smaller project".into());
        }
        outputs.push(pair);
    }
    for (entity, pair) in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| !achimeld_application::art_images::is_art_image(entity))
        .zip(outputs)
    {
        if cancelled() {
            return Err("Export cancelled; some complete files may already exist".into());
        }
        let result = (|| {
            let achieved = achimeld_imaging::render_cancellable(
                document,
                entity["id"].as_str().ok_or("Missing ID")?,
                Some((width, height)),
                &mut assets,
                cancelled,
            )?
            .achieved;
            let mut gray = achieved.clone();
            for pixel in gray.pixels_mut() {
                let value = (0.2126 * f64::from(pixel[0])
                    + 0.7152 * f64::from(pixel[1])
                    + 0.0722 * f64::from(pixel[2]))
                .round() as u8;
                pixel[0] = value;
                pixel[1] = value;
                pixel[2] = value;
            }
            for (image, path) in [(&achieved, &pair[0]), (&gray, &pair[1])] {
                if cancelled() {
                    return Err("Export cancelled".into());
                }
                let temporary = directory.join(format!(".achimeld-mcp-{}.tmp", new_id()));
                let saved = (|| {
                    let file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temporary)
                        .map_err(|e| e.to_string())?;
                    let mut writer = std::io::BufWriter::new(file);
                    let output = if encoding == image::ImageFormat::Jpeg {
                        let rgb = image::RgbImage::from_fn(width, height, |x, y| {
                            let p = image.get_pixel(x, y);
                            image::Rgb([
                                (u16::from(p[0]) * u16::from(p[3]) / 255) as u8,
                                (u16::from(p[1]) * u16::from(p[3]) / 255) as u8,
                                (u16::from(p[2]) * u16::from(p[3]) / 255) as u8,
                            ])
                        });
                        image::DynamicImage::ImageRgb8(rgb)
                    } else {
                        image::DynamicImage::ImageRgba8(image.clone())
                    };
                    output
                        .write_to(&mut writer, encoding)
                        .map_err(|e| e.to_string())?;
                    use std::io::Write;
                    writer.flush().map_err(|e| e.to_string())?;
                    writer.get_ref().sync_all().map_err(|e| e.to_string())?;
                    drop(writer);
                    if path.exists() {
                        achimeld_security::windows::reject_reparse(path)
                            .map_err(|e| e.to_string())?;
                    }
                    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
                })();
                if saved.is_err() {
                    let _ = std::fs::remove_file(&temporary);
                }
                saved?;
                files.push(path.to_string_lossy().into_owned());
            }
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => count += 1,
            Err(error) => failures
                .push(json!({"nodeId":entity["id"],"apiName":entity["apiName"],"error":error})),
        }
    }
    Ok(
        json!({"destinationDirectory":directory,"format":format,"width":width,"height":height,"exportedCount":count,"files":files,"failures":failures}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_returns_the_destination_camera_and_global_style_import_is_one_undo_step() {
        let (mut host, mut ui, mut engine) = fixture();
        host.editor
            .execute(
                "Stat camera",
                Command::CanvasCamera {
                    kind: EntityKind::Stat,
                    zoom: 2.0,
                    offset_x: 20.0,
                    offset_y: -10.0,
                },
                None,
            )
            .unwrap();
        engine.observe(&host, &ui);
        let stat = request(
            &host,
            &engine,
            "create_stat",
            json!({"apiName":"COUNT","displayName":"Count","type":"Int","column":0,"row":0}),
        );
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, stat)["success"],
            true
        );
        assert_eq!(ui.automation_scene()["camera"]["zoom"], 2.0);
        let revision = engine.revision;
        engine.observe(&host, &ui);
        assert_eq!(engine.revision, revision);
        let create = request(
            &host,
            &engine,
            "create_achievement",
            json!({"apiName":"STYLE","displayName":"Styled","description":"Saved template","column":0,"row":0}),
        );
        let id = dispatch(&mut engine, &mut host, &mut ui, create)["data"]["result"]["nodeId"]
            .as_str()
            .unwrap()
            .to_owned();
        let style = achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Global fixture".into(),
            recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF"}]}),
        };
        host.editor_state.state.styles.push(style.clone());
        let apply = request(
            &host,
            &engine,
            "apply_icon_style",
            json!({"achievementId":id,"styleId":style.id}),
        );
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, apply)["success"],
            true
        );
        assert_eq!(host.editor.document().icon_styles().len(), 1);
        assert!(host.editor.undo());
        assert!(host.editor.document().icon_styles().is_empty());
    }
    #[test]
    fn export_result_remains_bound_to_its_origin_after_a_project_switch() {
        let (mut host, mut ui, mut engine) = fixture();
        engine.observe(&host, &ui);
        let envelope = engine.envelope(&host, Value::Null);
        let origin = envelope["projectId"].clone();
        let mut job = pending(request(
            &host,
            &engine,
            "export_achievement_icons",
            json!({"destinationDirectory":r"C:\fixture"}),
        ));
        let (reply, responses) = mpsc::sync_channel(1);
        job.reply = reply;
        let (tx, receiver) = mpsc::sync_channel(1);
        tx.send(Ok(Output::Export(json!({"exportedCount":1}))))
            .unwrap();
        engine.work = Some(Working {
            pending: job,
            receiver,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            envelope,
            session: host.editor_session_id.clone(),
        });
        host.editor = Editor::default();
        host.editor_session_id = new_id();
        let (_, receiver) = mpsc::sync_channel(1);
        let server = Server {
            receiver,
            stopped: Arc::new(AtomicBool::new(false)),
            thread: None,
        };
        engine.poll(&server, &mut host, &mut ui);
        assert_eq!(responses.recv().unwrap()["data"]["projectId"], origin);
        assert!(engine.cache.is_empty());
    }
    #[test]
    fn cancelled_and_expired_queued_requests_never_mutate_the_document() {
        let (mut host, mut ui, mut engine) = fixture();
        engine.observe(&host, &ui);
        let (tx, receiver) = mpsc::sync_channel(1);
        let server = Server {
            receiver,
            stopped: Arc::new(AtomicBool::new(false)),
            thread: None,
        };
        for expired in [true, false] {
            let mut job = pending(request(
                &host,
                &engine,
                "create_achievement",
                json!({"apiName":"CANCELLED","displayName":"Cancelled","description":"Never created","column":0,"row":0}),
            ));
            if expired {
                job.deadline = Instant::now() - Duration::from_secs(1);
            } else {
                job.cancelled.store(true, Ordering::Release);
            }
            tx.send(job).unwrap();
            engine.poll(&server, &mut host, &mut ui);
        }
        assert!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .is_empty()
        );
        assert!(host.editor.undo_label().is_none());
        assert!(!local_path(Path::new(r"\\server\share\art.png")));
        assert!(!local_path(Path::new(r"\\.\pipe\endpoint")));
    }
    fn fixture() -> (Host, achimeld_ui::Desktop, Engine) {
        let mut host = Host::new(egui::Context::default());
        host.settings = None;
        host.dialog = None;
        host.editor = Editor::default();
        (host, achimeld_ui::Desktop::default(), Engine::default())
    }
    fn request(host: &Host, engine: &Engine, operation: &str, args: Value) -> Request {
        Request {
            operation: operation.into(),
            arguments: args,
            project_id: host.editor.document().project()["projectId"]
                .as_str()
                .map(str::to_owned),
            expected_revision: Some(engine.revision),
            request_id: Some(new_id()),
        }
    }
    fn pending(request: Request) -> Pending {
        let (reply, _) = mpsc::sync_channel(1);
        Pending {
            cancelled: Arc::new(AtomicBool::new(false)),
            request,
            artwork: None,
            translation: None,
            reply,
            deadline: Instant::now() + Duration::from_secs(20),
        }
    }
    fn dispatch(
        engine: &mut Engine,
        host: &mut Host,
        ui: &mut achimeld_ui::Desktop,
        request: Request,
    ) -> Value {
        engine.observe(host, ui);
        if let Some(response) = engine.preflight(&request, host, ui) {
            response
        } else {
            engine.complete(&pending(request), host, ui)
        }
    }
    #[test]
    fn retry_staleness_busy_and_same_project_reopen_use_the_live_editor_session() {
        let (mut host, mut ui, mut engine) = fixture();
        engine.observe(&host, &ui);
        let create = request(
            &host,
            &engine,
            "create_achievement",
            json!({"apiName":"FIRST","displayName":"First","description":"Win once","column":2,"row":3}),
        );
        let result = dispatch(&mut engine, &mut host, &mut ui, create.clone());
        assert_eq!(result["success"], true);
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, create.clone()),
            result
        );
        assert_eq!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .len(),
            1
        );
        let mut reused = create.clone();
        reused.arguments["apiName"] = json!("OTHER");
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, reused)["errorCode"],
            "request_id_reused"
        );
        let stale = request(&host, &engine, "undo", json!({}));
        host.editor
            .execute("Manual", Command::Add(EntityKind::Stat), None)
            .unwrap();
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, stale)["errorCode"],
            "stale_scene"
        );
        let busy = request(&host, &engine, "undo", json!({}));
        host.start_dialog(Dialog::Open);
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, busy)["errorCode"],
            "editor_busy"
        );
        host.dialog = None;
        let document = host.editor.document().clone();
        host.editor = Editor::new(document);
        host.editor_session_id = new_id();
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, create)["errorCode"],
            "stale_scene"
        );
    }
    #[test]
    fn file_artwork_is_owned_deduplicated_and_retries_need_no_source_file() {
        let (mut host, mut ui, mut engine) = fixture();
        engine.observe(&host, &ui);
        let create = request(
            &host,
            &engine,
            "create_achievement",
            json!({"apiName":"ART","displayName":"Art","description":"Artwork","column":0,"row":0}),
        );
        let id = dispatch(&mut engine, &mut host, &mut ui, create)["data"]["result"]["nodeId"]
            .as_str()
            .unwrap()
            .to_owned();
        let source = crate::test_output::root().join(format!("mcp-source-{}.png", new_id()));
        image::RgbaImage::from_pixel(4, 4, image::Rgba([240, 40, 10, 255]))
            .save(&source)
            .unwrap();
        let import = request(
            &host,
            &engine,
            "add_general_image",
            json!({"achievementId":id,"filePath":source}),
        );
        let mut job = pending(import.clone());
        job.artwork = Some(prepare_art(&import.arguments, &|| false).unwrap());
        let result = engine.complete(&job, &mut host, &mut ui);
        assert_eq!(result["success"], true);
        std::fs::remove_file(&source).unwrap();
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, import.clone()),
            result
        );
        let mut second = import;
        second.request_id = Some(new_id());
        second.expected_revision = Some(engine.revision);
        job.request = second;
        assert_eq!(engine.complete(&job, &mut host, &mut ui)["success"], true);
        assert_eq!(
            host.editor.document().project()["assets"]["images"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let saved = host.editor.document().clone();
        host.editor.undo();
        assert_eq!(
            achimeld_application::general_image_entries(
                host.editor
                    .document()
                    .entity(EntityKind::Achievement, &id)
                    .unwrap()
            )
            .len(),
            1
        );
        host.editor.redo();
        assert_eq!(*host.editor.document(), saved);
    }
    #[test]
    fn icon_exports_have_requested_dimensions_grayscale_formats_and_failures() {
        let (mut host, _, _) = fixture();
        host.editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = host.editor.document().entities(EntityKind::Achievement)[0].clone();
        entity["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFEE5511"}]});
        host.editor
            .execute(
                "Artwork",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: entity["id"].as_str().unwrap().into(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        let directory = crate::test_output::root().join(format!("mcp-export-{}", new_id()));
        std::fs::create_dir(&directory).unwrap();
        for format in ["Png", "Jpeg", "Bmp", "Tiff"] {
            let result = export_icons(
                host.editor.document(),
                &json!({"destinationDirectory":directory,"format":format,"width":24,"height":12}),
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(result["exportedCount"], 1);
            assert_eq!(result["failures"], json!([]));
            for path in result["files"].as_array().unwrap() {
                let path = path.as_str().unwrap();
                let image = image::open(path).unwrap().to_rgb8();
                assert_eq!(image.dimensions(), (24, 12));
                if path.contains("_gray") {
                    assert!(image.pixels().all(|p| p[0] == p[1] && p[1] == p[2]));
                }
            }
        }
        assert_eq!(
            export_icons(
                host.editor.document(),
                &json!({"destinationDirectory":directory}),
                None,
                &|| false
            )
            .unwrap()["exportedCount"],
            1
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
    #[test]
    fn prepared_translation_import_rechecks_late_edits_and_cancellation() {
        let (mut host, mut ui, mut engine) = fixture();
        host.execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        engine.observe(&host, &ui);
        let mut copy =
            achimeld_application::translations::achievement_catalog(host.editor.document())
                .unwrap();
        copy.rows
            .values_mut()
            .next()
            .unwrap()
            .insert("French".into(), "Traduction".into());
        let csv = String::from_utf8(achimeld_persistence::project::write_catalog(&copy).unwrap())
            .unwrap();
        let request = request(
            &host,
            &engine,
            "import_achievement_translations",
            json!({"csv":csv}),
        );
        let Output::TranslationImport(command, count) = prepare_translations(
            host.editor.document(),
            &request.operation,
            &request.arguments,
            &|| false,
        )
        .unwrap() else {
            panic!("Expected an import plan")
        };
        let mut pending = pending(request);
        pending.translation = Some((*command, count));
        let (_sender, receiver) = mpsc::sync_channel(1);
        let work = Working {
            pending,
            receiver,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            envelope: Value::Null,
            session: host.editor_session_id.clone(),
        };
        host.execute("Manual edit", Command::Add(EntityKind::Stat), None)
            .unwrap();
        engine.observe(&host, &ui);
        let before = host.editor.document().clone();
        assert_eq!(
            engine.complete_prepared(&work, &mut host, &mut ui)["errorCode"],
            "stale_scene"
        );
        work.pending.cancelled.store(true, Ordering::Release);
        assert_eq!(
            engine.complete_prepared(&work, &mut host, &mut ui)["errorCode"],
            "timeout"
        );
        assert_eq!(host.editor.document(), &before);
        assert_eq!(host.editor.undo_label(), Some("Manual edit"));
    }

    #[test]
    fn collection_edits_are_revision_pinned_and_import_is_one_undo_step() {
        let (mut host, mut ui, mut engine) = fixture();
        engine.observe(&host, &ui);
        let before = host.editor.document().clone();
        let list = request(&host, &engine, "list_style_collections", json!({}));
        let libraries = dispatch(&mut engine, &mut host, &mut ui, list);
        assert_eq!(libraries["success"], true, "{libraries}");
        let source = libraries["data"]["result"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "c13dc47a-5afd-4bf3-a2bb-e5456020a001")
            .unwrap();
        let source_id = source["id"].clone();
        let style_id = source["styles"][0]["id"].clone();
        let create = request(
            &host,
            &engine,
            "create_style_collection",
            json!({"name":"Agent styles"}),
        );
        let created = dispatch(&mut engine, &mut host, &mut ui, create.clone());
        assert_eq!(created["success"], true, "{created}");
        assert_eq!(dispatch(&mut engine, &mut host, &mut ui, create), created);
        let collection = created["data"]["result"]["collectionId"].clone();
        let copy = request(
            &host,
            &engine,
            "create_icon_style",
            json!({"collectionId":collection,"sourceCollectionId":source_id,"sourceStyleId":style_id,"name":"Teal shine","edits":[{"path":"/layers/0/gradient/stops/0/colorHex","value":"#FF082F49"}]}),
        );
        let stale = request(
            &host,
            &engine,
            "create_style_collection",
            json!({"name":"Stale"}),
        );
        let copied = dispatch(&mut engine, &mut host, &mut ui, copy.clone());
        assert_eq!(copied["success"], true, "{copied}");
        assert_eq!(dispatch(&mut engine, &mut host, &mut ui, copy), copied);
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, stale)["errorCode"],
            "stale_scene"
        );
        assert_eq!(host.editor.document(), &before);
        assert!(host.editor.undo_label().is_none());
        let import = request(
            &host,
            &engine,
            "import_icon_style",
            json!({"collectionId":collection,"styleId":copied["data"]["result"]["style"]["id"]}),
        );
        let imported = dispatch(&mut engine, &mut host, &mut ui, import);
        assert_eq!(imported["success"], true, "{imported}");
        assert_eq!(host.editor.document().icon_styles().len(), 1);
        assert!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .is_empty()
        );
        assert!(host.editor.undo());
        assert_eq!(host.editor.document(), &before);
        let invalid = request(
            &host,
            &engine,
            "create_icon_style",
            json!({"collectionId":source_id,"sourceCollectionId":source_id,"sourceStyleId":style_id,"name":"Cannot edit presets"}),
        );
        // Undo changes the observed revision; refresh before preparing the mutation.
        engine.observe(&host, &ui);
        let mut invalid = invalid;
        invalid.expected_revision = Some(engine.revision);
        assert_eq!(
            dispatch(&mut engine, &mut host, &mut ui, invalid)["success"],
            false
        );
    }

    /// Developer authoring command: creates the shipped variants through JSON-RPC and the real editor pipe.
    #[test]
    #[ignore = "Writes a new external authoring result; run explicitly to regenerate the app variants"]
    fn author_default_styles_through_mcp() {
        let (mut host, mut ui, mut engine) = fixture();
        let original = host.editor.document().clone();
        let server = Server::start(host.ctx.clone()).unwrap();
        let process = std::process::id();
        let client = std::thread::spawn(move || {
            let mut trace = Vec::new();
            let mut rpc = |name: &str, arguments: Value| -> Value {
                let input = [json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}), json!({"jsonrpc":"2.0","method":"notifications/initialized"}), json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}})].iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n";
                let mut output = Vec::new();
                achimeld_mcp::serve(
                    &mut std::io::Cursor::new(input),
                    &mut output,
                    achimeld_mcp::call_editor,
                )
                .unwrap();
                let response: Value = serde_json::from_str(
                    std::str::from_utf8(&output)
                        .unwrap()
                        .lines()
                        .last()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(response["result"]["isError"], false, "{response}");
                let result: Value = serde_json::from_str(
                    response["result"]["content"][0]["text"].as_str().unwrap(),
                )
                .unwrap();
                trace.push(json!({"tool":name,"arguments":arguments,"result":result}));
                result
            };
            let libraries = rpc("list_style_collections", json!({"processId":process}));
            let scene = rpc("get_scene", json!({"processId":process}));
            let mut pins = scene;
            let mut mutate =
                |name: &str, mut arguments: Value, rpc: &mut dyn FnMut(&str, Value) -> Value| {
                    arguments["processId"] = json!(process);
                    arguments["projectId"] = pins["projectId"].clone();
                    arguments["expectedRevision"] = pins["revision"].clone();
                    arguments["requestId"] = json!(new_id());
                    let result = rpc(name, arguments);
                    pins = result.clone();
                    result
                };
            let collection = mutate(
                "create_style_collection",
                json!({"name":"Color studies"}),
                &mut rpc,
            )["result"]["collectionId"]
                .clone();
            let names = [
                "Mint shine",
                "Citrus prism",
                "Violet dusk",
                "Bronze focus",
                "Lagoon ribbon",
                "Citrus neon",
                "Azure shine",
                "Lime pulse",
            ];
            let mut styles = Vec::new();
            for library in libraries["result"].as_array().unwrap().iter().filter(|c| {
                matches!(
                    c["id"].as_str(),
                    Some(
                        "c13dc47a-5afd-4bf3-a2bb-e5456020a001"
                            | "c13dc47a-5afd-4bf3-a2bb-e5456020a002"
                    )
                )
            }) {
                for source in library["styles"].as_array().unwrap() {
                    let original = rpc(
                        "get_icon_style",
                        json!({"processId":process,"collectionId":library["id"],"styleId":source["id"]}),
                    );
                    let mut edits = Vec::new();
                    fn colors(value: &Value, path: &str, edits: &mut Vec<Value>, index: usize) {
                        match value {
                            Value::Object(object) => {
                                for (key, value) in object {
                                    let next = format!(
                                        "{path}/{}",
                                        key.replace('~', "~0").replace('/', "~1")
                                    );
                                    if (key.ends_with("ColorHex") || key == "colorHex")
                                        && let Some(hex) = value.as_str()
                                    {
                                        let start = if hex.len() == 9 { 3 } else { 1 };
                                        let r =
                                            u8::from_str_radix(&hex[start..start + 2], 16).unwrap();
                                        let g = u8::from_str_radix(&hex[start + 2..start + 4], 16)
                                            .unwrap();
                                        let b = u8::from_str_radix(&hex[start + 4..start + 6], 16)
                                            .unwrap();
                                        if r.max(g).max(b) - r.min(g).min(b) > 16 {
                                            let rgb = if index == 6 {
                                                [g, r, b]
                                            } else if index.is_multiple_of(2) {
                                                [g, b, r]
                                            } else {
                                                [b, r, g]
                                            };
                                            edits.push(json!({"path":next,"value":format!("{}{:02X}{:02X}{:02X}", &hex[..start],rgb[0],rgb[1],rgb[2])}));
                                        }
                                    } else {
                                        colors(value, &next, edits, index);
                                    }
                                }
                            }
                            Value::Array(array) => {
                                for (i, value) in array.iter().enumerate() {
                                    colors(value, &format!("{path}/{i}"), edits, index);
                                }
                            }
                            _ => {}
                        }
                    }
                    colors(&original["result"]["recipe"], "", &mut edits, styles.len());
                    for (index, layer) in original["result"]["recipe"]["layers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                    {
                        if layer["kind"] == "AchievementImage"
                            && layer["scale"].as_f64().is_some_and(|scale| scale > 1.5)
                        {
                            edits.push(
                                json!({"path":format!("/layers/{index}/scale"),"value":0.95}),
                            );
                            for key in ["offsetX", "offsetY"] {
                                if layer[key].is_number() {
                                    edits.push(
                                        json!({"path":format!("/layers/{index}/{key}"),"value":0}),
                                    );
                                }
                            }
                        }
                    }
                    if let Some((index, layer)) = original["result"]["recipe"]["layers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .find(|(_, l)| l["kind"] == "Shape" && l["opacity"].is_number())
                    {
                        edits.push(json!({"path":format!("/layers/{index}/opacity"),"value":layer["opacity"].as_f64().unwrap()*0.88}));
                    }
                    let made = mutate(
                        "create_icon_style",
                        json!({"collectionId":collection,"name":names[styles.len()],"sourceCollectionId":library["id"],"sourceStyleId":source["id"],"edits":edits}),
                        &mut rpc,
                    );
                    styles.push(made["result"]["style"].clone());
                }
            }
            assert_eq!(styles.len(), 8);
            let folder =
                crate::test_output::root().join(format!("mcp-style-authoring-{}", new_id()));
            std::fs::create_dir_all(&folder).unwrap();
            let objects = mutate(
                "create_style_collection",
                json!({"name":"Preview objects"}),
                &mut rpc,
            )["result"]["collectionId"]
                .clone();
            use achimeld_application::artifact3d::{ArtifactPreset, ArtifactSettings, preset};
            let settings = ArtifactSettings {
                parts: preset(ArtifactPreset::Cup),
                rotation: [12.0, -18.0, 0.0],
                ambient: 0.36,
                light_intensity: 0.9,
                ..Default::default()
            };
            let cup = mutate("create_icon_style", json!({"collectionId":objects,"name":"Achievement cup · 3D object","recipe":{"layers":[{"id":new_id(),"displayName":"Achievement cup","kind":"Artifact3d","visible":true,"opacity":1.0,"artifact3d":settings}]}}), &mut rpc)["result"]["style"].clone();
            mutate(
                "export_style_preview",
                json!({"collectionId":objects,"styleId":cup["id"],"destinationFilePath":folder.join("achievement-cup.png"),"width":512,"height":512}),
                &mut rpc,
            );
            let png = image::open(folder.join("achievement-cup.png"))
                .unwrap()
                .to_rgba8();
            assert!(png.pixels().any(|p| p[3] == 0));
            assert!(png.pixels().any(|p| p[3] > 0));
            std::fs::write(folder.join("preview-objects.json"), serde_json::to_vec_pretty(&json!([{"id":"c13dc47a-5afd-4bf3-a2bb-e5456020a004","name":"Preview objects","styles":[cup]}])).unwrap()).unwrap();
            mutate(
                "export_style_collection",
                json!({"collectionId":collection,"destinationFilePath":folder.join("Color studies.achv")}),
                &mut rpc,
            );
            let loaded = ProjectStore::new()
                .load(&folder.join("Color studies.achv"))
                .unwrap();
            assert_eq!(loaded.icon_styles().len(), 8);
            let mut overview = image::RgbaImage::new(1024, 512);
            for (index, style) in styles.iter().enumerate() {
                let path = folder.join(format!("style-{}.png", index + 1));
                mutate(
                    "export_style_preview",
                    json!({"collectionId":collection,"styleId":style["id"],"destinationFilePath":path,"width":256,"height":256}),
                    &mut rpc,
                );
                let image = image::open(&path).unwrap().to_rgba8();
                assert!(image.pixels().any(|p| p[3] > 0));
                image::imageops::replace(
                    &mut overview,
                    &image,
                    ((index % 4) * 256) as i64,
                    ((index / 4) * 256) as i64,
                );
            }
            overview.save(folder.join("style-overview.png")).unwrap();
            let output = json!([{"id":"c13dc47a-5afd-4bf3-a2bb-e5456020a003","name":"Color studies","styles":styles}]);
            std::fs::write(
                folder.join("variants.json"),
                serde_json::to_vec_pretty(&output).unwrap(),
            )
            .unwrap();
            std::fs::write(
                folder.join("mcp-transcript.json"),
                serde_json::to_vec_pretty(&trace).unwrap(),
            )
            .unwrap();
            println!("MCP_STYLE_AUTHORING={}", folder.display());
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        while !client.is_finished() {
            engine.poll(&server, &mut host, &mut ui);
            assert!(Instant::now() < deadline, "MCP style authoring timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        client.join().unwrap();
        assert_eq!(host.editor.document(), &original);
        assert!(host.editor.undo_label().is_none());
    }

    #[test]
    fn real_windows_pipe_serves_the_stdio_facade_and_survives_disconnection() {
        let (mut host, mut ui, mut engine) = fixture();
        let server = Server::start(host.ctx.clone()).unwrap();
        let process = std::process::id();
        let (tx, rx) = mpsc::sync_channel(1);
        let client = std::thread::spawn(move || {
            {
                let mut pipe = Pipe::connect(process).unwrap();
                pipe.write(
                    &json!({"invalid":true}),
                    Duration::from_secs(2),
                    &mut || Ok(()),
                )
                .unwrap();
            }
            let input=[json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),json!({"jsonrpc":"2.0","method":"notifications/initialized"}),json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_scene","arguments":{"processId":process}}})].iter().map(Value::to_string).collect::<Vec<_>>().join("\n")+"\n";
            let mut output = Vec::new();
            achimeld_mcp::serve(
                &mut std::io::Cursor::new(input),
                &mut output,
                achimeld_mcp::call_editor,
            )
            .unwrap();
            let responses: Vec<Value> = std::str::from_utf8(&output)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(responses[1]["result"]["isError"], false);
            let scene: Value = serde_json::from_str(
                responses[1]["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            let args = json!({"processId":process,"projectId":scene["projectId"],"expectedRevision":scene["revision"],"requestId":new_id(),"apiName":"PIPE_TEST","displayName":"Pipe test","description":"Created through the real Windows pipe","column":-3,"row":2});
            let created = achimeld_mcp::call_editor("create_achievement", &args).unwrap();
            assert_eq!(
                achimeld_mcp::call_editor("create_achievement", &args).unwrap(),
                created
            );
            let node = achimeld_mcp::call_editor(
                "get_node",
                &json!({"processId":process,"nodeId":created["result"]["nodeId"]}),
            )
            .unwrap();
            assert_eq!(node["result"]["definition"]["apiName"], "PIPE_TEST");
            let csv = achimeld_mcp::call_editor(
                "get_achievement_translations",
                &json!({"processId":process}),
            )
            .unwrap();
            assert_eq!(csv["result"]["rowCount"], 2);
            let mut catalog = achimeld_persistence::project::parse_catalog(
                csv["result"]["csv"].as_str().unwrap().as_bytes(),
            )
            .unwrap();
            let row = node["result"]["definition"]["displayNameLocalizationId"]
                .as_str()
                .unwrap();
            catalog
                .rows
                .get_mut(row)
                .unwrap()
                .insert("Polish".into(), "Próba tłumaczenia".into());
            let translated =
                String::from_utf8(achimeld_persistence::project::write_catalog(&catalog).unwrap())
                    .unwrap();
            let import = json!({"processId":process,"projectId":csv["projectId"],"expectedRevision":csv["revision"],"requestId":new_id(),"csv":translated});
            let imported =
                achimeld_mcp::call_editor("import_achievement_translations", &import).unwrap();
            assert_eq!(imported["result"]["changedCells"], 1);
            assert_eq!(
                achimeld_mcp::call_editor("import_achievement_translations", &import).unwrap(),
                imported
            );
            let mut stale = import.clone();
            stale["requestId"] = json!(new_id());
            assert!(
                achimeld_mcp::call_editor("import_achievement_translations", &stale)
                    .unwrap_err()
                    .contains("stale_scene")
            );
            let path =
                crate::test_output::root().join(format!("mcp-translations-{}.csv", new_id()));
            let export = json!({"processId":process,"projectId":imported["projectId"],"expectedRevision":imported["revision"],"requestId":new_id(),"destinationFilePath":path});
            let exported =
                achimeld_mcp::call_editor("export_achievement_translations", &export).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                achimeld_persistence::project::parse_catalog(&bytes)
                    .unwrap()
                    .rows[row]["Polish"],
                "Próba tłumaczenia"
            );
            let mut collision = export.clone();
            collision["requestId"] = json!(new_id());
            assert!(
                achimeld_mcp::call_editor("export_achievement_translations", &collision).is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            let from_file = json!({"processId":process,"projectId":exported["projectId"],"expectedRevision":exported["revision"],"requestId":new_id(),"filePath":path});
            let unchanged =
                achimeld_mcp::call_editor("import_achievement_translations", &from_file).unwrap();
            assert_eq!(unchanged["result"]["changedCells"], 0);
            let undo = json!({"processId":process,"projectId":unchanged["projectId"],"expectedRevision":unchanged["revision"],"requestId":new_id()});
            achimeld_mcp::call_editor("undo", &undo).unwrap();
            let after = achimeld_mcp::call_editor(
                "get_achievement_translations",
                &json!({"processId":process}),
            )
            .unwrap();
            assert_eq!(
                achimeld_persistence::project::parse_catalog(
                    after["result"]["csv"].as_str().unwrap().as_bytes()
                )
                .unwrap()
                .rows[row]["Polish"],
                ""
            );
            std::fs::remove_file(path).unwrap();
            let _ = tx.send(());
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            engine.poll(&server, &mut host, &mut ui);
            if rx.try_recv().is_ok() || client.is_finished() {
                break;
            }
            assert!(Instant::now() < deadline, "Windows pipe fixture timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        client.join().unwrap();
        assert_eq!(
            host.editor
                .document()
                .entities(EntityKind::Achievement)
                .len(),
            1
        );
    }
}
