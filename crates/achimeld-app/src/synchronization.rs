//! Supervised host orchestration. One job, one reviewed session, one image pair in
//! memory at a time. The worker receives bytes and identities, never local paths.
use crate::sync_previews::IconPreviews;
use achimeld_application::{Catalog, Document, EntityKind, new_id};
use achimeld_browser::worker::SteamReader;
use achimeld_core::SteamAppId;
use achimeld_imaging::{AssetSource, render_cancellable};
use achimeld_persistence::project::ProjectStore;
use achimeld_security::{cancellation::Cancellation, supervision::Session};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

// Only reconcile the confirmed batch, in its own entity namespace. A renamed
// identity still present remotely is not a confirmed deletion.
fn reconcile_deleted_history(
    project: &mut Value,
    result: &Value,
    targets: &Value,
) -> Result<(), String> {
    let invalid = || "Invalid verified deletion history".to_owned();
    let snapshot = &result["snapshot"];
    if snapshot["steamAppId"] != project["steamAppId"] || snapshot["isComplete"] != true {
        return Err(invalid());
    }
    let targets = targets.as_array().ok_or_else(invalid)?;
    let results = result["results"].as_array().ok_or_else(invalid)?;
    if results.len() != targets.len() {
        return Err(invalid());
    }
    let mut deleted = Vec::new();
    for target in targets {
        let kind = target["kind"].as_str().ok_or_else(invalid)?;
        let name = target["apiName"].as_str().ok_or_else(invalid)?;
        let key = match kind {
            "Stat" => "stats",
            "Achievement" => "achievements",
            _ => return Err(invalid()),
        };
        if results
            .iter()
            .filter(|r| {
                r["kind"] == kind
                    && r["apiName"] == name
                    && matches!(r["status"].as_str(), Some("Deleted" | "AlreadyAbsent"))
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
        let previous = project["lastRemoteSnapshot"][key]
            .as_array()
            .ok_or_else(invalid)?;
        let scalar_name = |r: &Value| {
            r["apiName"]
                .as_str()
                .or_else(|| r["apiName"]["value"].as_str())
                .map(str::to_owned)
        };
        let old: Vec<_> = previous
            .iter()
            .filter(|r| scalar_name(r).as_deref() == Some(name))
            .collect();
        if old.len() != 1 {
            return Err(invalid());
        }
        let identity = old[0]["steamId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid)?;
        let current = snapshot[key].as_array().ok_or_else(invalid)?;
        if current.iter().any(|r| {
            scalar_name(r).is_some_and(|n| n.eq_ignore_ascii_case(name)) || r["steamId"] == identity
        }) {
            return Err(invalid());
        }
        deleted.push((kind.to_owned(), key, name.to_owned(), identity.to_owned()));
    }
    for (kind, key, name, identity) in deleted {
        project["syncBaselines"]
            .as_array_mut()
            .ok_or_else(invalid)?
            .retain(|b| {
                b["entityType"] != kind || (b["apiName"] != name && b["steamId"] != identity)
            });
        for entity in project[key].as_array_mut().ok_or_else(invalid)? {
            let pending = &entity["pendingApiRename"];
            if pending["appId"] == snapshot["steamAppId"]
                && pending["from"] == name
                && pending["steamId"] == identity
            {
                entity
                    .as_object_mut()
                    .ok_or_else(invalid)?
                    .remove("pendingApiRename");
            }
        }
    }
    Ok(())
}

pub(crate) enum Event {
    Summary(String),
    Banner(Box<Document>),
    Imported(Box<Document>),
    Review(Value),
    Progress(String),
    Complete(Result<Document, String>),
}

/// Combine independently edited fields. Arrays are atomic: ambiguous concurrent
/// changes (including reordered/deleted layers) fail instead of losing artwork.
pub(crate) fn preserve_local_edits(
    baseline: &Document,
    local: &Document,
    remote: &Document,
) -> Result<Document, String> {
    fn merge(
        base: Option<&Value>,
        local: Option<&Value>,
        remote: Option<&Value>,
    ) -> Result<Option<Value>, String> {
        if local == base || local == remote {
            return Ok(remote.cloned());
        }
        if remote == base {
            return Ok(local.cloned());
        }
        if let (Some(Value::Object(b)), Some(Value::Object(l)), Some(Value::Object(r))) =
            (base, local, remote)
        {
            let keys: std::collections::BTreeSet<_> =
                b.keys().chain(l.keys()).chain(r.keys()).collect();
            let mut result = serde_json::Map::new();
            for key in keys {
                if let Some(value) = merge(b.get(key), l.get(key), r.get(key))? {
                    result.insert(key.clone(), value);
                }
            }
            return Ok(Some(Value::Object(result)));
        }
        Err("Steam completed with conflicting project changes. Your edits are retained; save the project and prepare a fresh synchronization review.".into())
    }
    let project = merge(
        Some(baseline.project()),
        Some(local.project()),
        Some(remote.project()),
    )?
    .ok_or("Missing merged project")?;
    let base = serde_json::to_value(baseline.catalog()).map_err(|e| e.to_string())?;
    let current = serde_json::to_value(local.catalog()).map_err(|e| e.to_string())?;
    let result = serde_json::to_value(remote.catalog()).map_err(|e| e.to_string())?;
    let catalog = serde_json::from_value(
        merge(Some(&base), Some(&current), Some(&result))?.ok_or("Missing merged catalog")?,
    )
    .map_err(|e| e.to_string())?;
    Document::from_parts(project, catalog)
}
pub(crate) struct Pending {
    pub baseline: Document,
    pub activity: &'static str,
    pub completion_message: Option<String>,
    pub handshake_only: bool,
    pub cancel: Cancellation,
    pub events: mpsc::Receiver<Event>,
    pub approval: mpsc::SyncSender<Vec<(String, String)>>,
    pub review: Option<Value>,
    pub revision: u64,
    pub path: PathBuf,
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub previews: IconPreviews,
}

#[cfg(test)]
mod concurrent_edit_tests {
    use super::*;

    #[test]
    fn independent_changes_merge_and_overlapping_changes_fail_without_mutating_local() {
        let base = Document::new();
        let mut local = base.project().clone();
        local["name"] = json!("Local artwork session");
        let local = Document::from_parts(local, base.catalog().clone()).unwrap();
        let mut remote = base.project().clone();
        remote["lastRemoteSnapshot"] = json!({"steamAppId":base.app_id()});
        let remote = Document::from_parts(remote, base.catalog().clone()).unwrap();
        let merged = preserve_local_edits(&base, &local, &remote).unwrap();
        assert_eq!(merged.project()["name"], local.project()["name"]);
        assert_eq!(
            merged.project()["lastRemoteSnapshot"],
            remote.project()["lastRemoteSnapshot"]
        );
        let mut conflict = remote.project().clone();
        conflict["name"] = json!("Conflicting name");
        let conflict = Document::from_parts(conflict, base.catalog().clone()).unwrap();
        assert!(preserve_local_edits(&base, &local, &conflict).is_err());
        assert_eq!(local.project()["name"], "Local artwork session");
    }
}
impl Pending {
    pub fn start_deletion(
        path: PathBuf,
        document: Document,
        revision: u64,
        targets: Vec<(EntityKind, String)>,
        ctx: eframe::egui::Context,
    ) -> Self {
        let baseline = document.clone();
        let cancel = Cancellation::default();
        let stop = cancel.clone();
        let (sender, events) = mpsc::sync_channel(8);
        let (approval, _) = mpsc::sync_channel(1);
        let destination = path.clone();
        let thread = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let mut stage = "checking deletion selection";
            let achievements = targets
                .iter()
                .filter(|(kind, _)| *kind == EntityKind::Achievement)
                .count();
            let stats = targets
                .iter()
                .filter(|(kind, _)| *kind == EntityKind::Stat)
                .count();
            achimeld_application::diagnostics::record(
                "INFO",
                "steam-deletion",
                &format!("Started reviewed deletion: {achievements} achievements, {stats} stats"),
            );
            let progress = |event| -> Result<(), String> {
                if let Event::Progress(message) | Event::Summary(message) = &event {
                    achimeld_application::diagnostics::record("INFO", "steam-deletion", message);
                }
                sender
                    .send(event)
                    .map_err(|_| "Steam deletion view closed".to_owned())?;
                ctx.request_repaint();
                Ok(())
            };
            let result = (|| {
                let app_id = SteamAppId::new(document.app_id()).map_err(|e| e.to_string())?;
                let mut store = ProjectStore::new();
                if store.load(&destination)? != document {
                    return Err("Save the project before deleting Steam entries".into());
                }
                let mut reader = connect_steam(app_id, &stop, &progress, &mut stage)?;
                progress(Event::Progress(
                    "Deleting reviewed Steam entries; missing entries will be skipped".into(),
                ))?;
                let targets: Vec<_> = targets.into_iter().map(|(kind, api)| json!({"kind":if kind == EntityKind::Stat {"Stat"} else {"Achievement"}, "apiName":api})).collect();
                stage = "saving the deletion recovery journal";
                let journal_root = runtime_root()?;
                fs::create_dir_all(&journal_root).map_err(|e| e.to_string())?;
                let journal = journal_root.join(format!("delete-{}.json", app_id.value()));
                save_journal(
                    &journal,
                    &json!({"appId": app_id.value(), "targets": targets, "state": "outcome-uncertain"}),
                )?;
                stage = "deleting and verifying reviewed Steam entries";
                let result = reader
                    .delete_steam(document.project().clone(), json!(targets))
                    .map_err(|_| "Steam deletion stopped. Some entries may already have been deleted. Validate and refresh Steam before retrying.".to_owned())?;
                close_steam(&mut reader, &progress, &mut stage)?;
                stop.check().map_err(|e| e.to_string())?;
                stage = "checking verified deletion results";
                let mut project = document.project().clone();
                if !result["snapshot"].is_object() {
                    return Err("Invalid deletion result snapshot".into());
                }
                reconcile_deleted_history(&mut project, &result, &json!(targets))?;
                project["lastRemoteSnapshot"] = result["snapshot"].clone();
                let updated = Document::from_parts(project, document.catalog().clone())?;
                if store.load(&destination)? != document {
                    return Err(
                        "Project changed during Steam deletion; refresh Steam before continuing"
                            .into(),
                    );
                }
                stage = "saving verified deletion results";
                progress(Event::Progress("Saving verified deletion results…".into()))?;
                store.save(&updated, &destination)?;
                stage = "removing the completed deletion recovery journal";
                fs::remove_file(&journal).map_err(|e| e.to_string())?;
                stage = "summarizing verified deletion results";
                let results = result["results"]
                    .as_array()
                    .ok_or("Invalid deletion results")?;
                let deleted = results.iter().filter(|r| r["status"] == "Deleted").count();
                let absent = results
                    .iter()
                    .filter(|r| r["status"] == "AlreadyAbsent")
                    .count();
                progress(Event::Summary(format!(
                    "Steam deletion completed: {deleted} deleted, {absent} already absent. Steam draft verified; publish manually in Steamworks."
                )))?;
                stage = "reloading saved deletion results";
                store.load(&destination)
            })();
            if result.is_ok() {
                achimeld_application::diagnostics::record(
                    "INFO",
                    "steam-deletion",
                    &format!(
                        "Completed and saved in {} ms",
                        started.elapsed().as_millis()
                    ),
                );
            } else {
                achimeld_application::diagnostics::record(
                    "ERROR",
                    "steam-deletion",
                    &format!(
                        "Stopped while {stage} after {} ms; refresh Steam to reconcile completed and unattempted entries",
                        started.elapsed().as_millis()
                    ),
                );
            }
            let _ = progress(Event::Complete(result));
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
        });
        Self {
            baseline,
            activity: "Deleting Steam entries",
            completion_message: None,
            handshake_only: false,
            cancel,
            events,
            approval,
            review: None,
            revision,
            path,
            thread: Some(thread),
            previews: IconPreviews::default(),
        }
    }

    pub fn start(
        path: PathBuf,
        document: Document,
        revision: u64,
        mut assets: impl AssetSource + Send + 'static,
        ctx: eframe::egui::Context,
        handshake_only: bool,
    ) -> Self {
        let baseline = document.clone();
        let cancel = Cancellation::default();
        let stop = cancel.clone();
        let (sender, events) = mpsc::sync_channel(8);
        let (approval, approved) = mpsc::sync_channel(1);
        let destination = path.clone();
        let thread = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let mut stage = "checking saved project";
            let component = if handshake_only {
                "validation"
            } else {
                "synchronization"
            };
            achimeld_application::diagnostics::record("INFO", component, "Started");
            let progress = |event| -> Result<(), String> {
                if let Event::Progress(message) = &event {
                    achimeld_application::diagnostics::record("INFO", component, message);
                }
                sender
                    .send(event)
                    .map_err(|_| "Synchronization view closed".to_owned())?;
                ctx.request_repaint();
                Ok(())
            };
            let result = if handshake_only {
                handshake(&document, &stop, &progress, &mut stage).map(|()| document)
            } else {
                run(
                    &destination,
                    document,
                    &mut assets,
                    &stop,
                    &approved,
                    &progress,
                    &mut stage,
                )
            };
            match &result {
                Ok(_) => achimeld_application::diagnostics::record(
                    "INFO",
                    component,
                    &format!("Completed in {} ms", started.elapsed().as_millis()),
                ),
                Err(error) => achimeld_application::diagnostics::record(
                    "ERROR",
                    component,
                    &format!(
                        "Stopped while {stage} after {} ms · {error}",
                        started.elapsed().as_millis()
                    ),
                ),
            }
            let result =
                result.map_err(|error| format!("Steam operation stopped while {stage}: {error}"));
            let _ = progress(Event::Complete(result));
        });
        Self {
            baseline,
            activity: if handshake_only {
                "Validating Steam connection"
            } else {
                "Synchronizing Steam"
            },
            completion_message: None,
            handshake_only,
            cancel,
            events,
            approval,
            review: None,
            revision,
            path,
            thread: Some(thread),
            previews: IconPreviews::default(),
        }
    }
}

fn connect_steam(
    app_id: SteamAppId,
    stop: &Cancellation,
    progress: &dyn Fn(Event) -> Result<(), String>,
    stage: &mut &'static str,
) -> Result<SteamReader, String> {
    progress(Event::Progress(
        "Opening Steam · sign in in the dedicated browser if requested".into(),
    ))?;
    *stage = "connecting to the supervisor";
    progress(Event::Progress("Connecting to the supervisor…".into()))?;
    let session = Session::connect().map_err(|e| e.to_string())?;
    *stage = "starting the Steam worker";
    progress(Event::Progress(
        "Starting the supervised Steam worker…".into(),
    ))?;
    let mut reader = SteamReader::connect(session, stop.clone()).map_err(|e| e.to_string())?;
    *stage = "starting the dedicated browser";
    progress(Event::Progress(
        "Starting the dedicated Steam browser…".into(),
    ))?;
    reader.start().map_err(|e| e.to_string())?;
    *stage = "opening the Steamworks destination";
    progress(Event::Progress(format!(
        "Opening Steamworks destination · App ID {}",
        app_id.value()
    )))?;
    reader.open_app(app_id).map_err(|e| e.to_string())?;
    *stage = "waiting for Steam sign-in";
    progress(Event::Progress(
        "Waiting for Steam sign-in · complete sign-in in the dedicated browser".into(),
    ))?;
    reader
        .wait_for_authentication(Duration::from_secs(600))
        .map_err(|e| e.to_string())?;
    *stage = "opening the Steamworks destination";
    progress(Event::Progress(format!(
        "Opening Steamworks destination · App ID {}",
        app_id.value()
    )))?;
    reader.open_app(app_id).map_err(|e| e.to_string())?;
    reader
        .wait_for_authentication(Duration::from_secs(30))
        .map_err(|e| e.to_string())?;
    Ok(reader)
}

fn handshake(
    document: &Document,
    stop: &Cancellation,
    progress: &dyn Fn(Event) -> Result<(), String>,
    stage: &mut &'static str,
) -> Result<(), String> {
    let app_id = SteamAppId::new(document.app_id()).map_err(|e| e.to_string())?;
    let mut reader = connect_steam(app_id, stop, progress, stage)?;
    stop.check().map_err(|e| e.to_string())?;
    close_steam(&mut reader, progress, stage)?;
    progress(Event::Progress(
        "Steam connection validated. Choose Synchronize to prepare a review.".into(),
    ))
}

fn close_steam(
    reader: &mut SteamReader,
    progress: &dyn Fn(Event) -> Result<(), String>,
    stage: &mut &'static str,
) -> Result<(), String> {
    *stage = "closing the dedicated Steam browser";
    progress(Event::Progress(
        "Closing the dedicated Steam browser…".into(),
    ))?;
    let cleanup_complete = reader.stop().map_err(|e| e.to_string())?;
    progress(Event::Progress(
        if cleanup_complete {
            "Dedicated Steam browser cleanup completed"
        } else {
            "Steam operation completed. Browser cleanup continues in the background."
        }
        .into(),
    ))
}

fn run(
    path: &Path,
    document: Document,
    assets: &mut dyn AssetSource,
    stop: &Cancellation,
    approved: &mpsc::Receiver<Vec<(String, String)>>,
    progress: &dyn Fn(Event) -> Result<(), String>,
    stage: &mut &'static str,
) -> Result<Document, String> {
    let mut document = document;
    for _ in 0..4 {
        let (next, imported) = run_once(path, document, assets, stop, approved, progress, stage)?;
        if !imported {
            return Ok(next);
        }
        progress(Event::Imported(Box::new(next.clone())))?;
        document = next;
    }
    Err("Steam kept changing during import; validate again".into())
}

fn automatic_imports(review: &Value) -> Vec<(String, String)> {
    review["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| {
            matches!(row["kind"].as_str(), Some("Stat" | "Achievement"))
                && (matches!(row["state"].as_str(), Some("SteamOnly" | "RenamedOnSteam"))
                    || row["kind"] == "Stat" && row["state"] == "SteamChanged")
                && row["actions"]
                    .as_array()
                    .is_some_and(|actions| actions.iter().any(|action| action == "Import"))
        })
        .filter_map(|row| Some((row["key"].as_str()?.into(), "Import".into())))
        .collect()
}

fn automatic_upload_choices(review: &Value, reconciliation: bool) -> Option<Vec<(String, String)>> {
    let rows = review["rows"].as_array()?;
    if reconciliation
        || rows.iter().any(|row| {
            matches!(
                row["state"].as_str(),
                Some("Conflict" | "ArtworkUnavailable")
            ) || row["uploadError"].as_str().is_some()
        })
    {
        return None;
    }
    rows.iter()
        .map(|row| {
            let action = row["defaultAction"].as_str()?;
            if !row["actions"]
                .as_array()?
                .iter()
                .any(|allowed| allowed == action)
            {
                return None;
            }
            Some((row["key"].as_str()?.to_owned(), action.to_owned()))
        })
        .collect()
}

fn run_once(
    path: &Path,
    document: Document,
    assets: &mut dyn AssetSource,
    stop: &Cancellation,
    approved: &mpsc::Receiver<Vec<(String, String)>>,
    progress: &dyn Fn(Event) -> Result<(), String>,
    stage: &mut &'static str,
) -> Result<(Document, bool), String> {
    progress(Event::Progress("Checking saved project…".into()))?;
    let mut store = ProjectStore::new();
    if store.load(path)? != document {
        return Err("The project changed on disk; reopen it before synchronizing".into());
    }
    let app_id = SteamAppId::new(document.app_id()).map_err(|e| e.to_string())?;
    *stage = "preparing local validation storage";
    let root = runtime_root()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let project_key = format!(
        "{:x}",
        Sha256::digest(
            document.project()["projectId"]
                .as_str()
                .unwrap_or("")
                .as_bytes()
        )
    );
    let journal_path = root.join(format!("{project_key}.json"));
    let reconciliation = journal_path.exists();
    // Declare icon candidates without rendering. The frozen document is rendered
    // only when the receiver reaches that icon's upload step.
    let hashes: serde_json::Map<String, Value> = document
        .synchronized_entities(EntityKind::Achievement)
        .filter_map(|entity| {
            Some((
                entity["apiName"].as_str()?.to_owned(),
                json!({"achieved":"", "unachieved":"", "deferred":true}),
            ))
        })
        .collect();
    let mut reader = connect_steam(app_id, stop, progress, stage)?;
    *stage = "reading Steamworks configuration";
    progress(Event::Progress(
        "Reading Steam and comparing project fields…".into(),
    ))?;
    let mut review = reader
        .prepare_sync(
            achimeld_application::style_achievements::synchronization_project(&document),
            serde_json::to_value(&document.catalog().rows).map_err(|e| e.to_string())?,
            Value::Object(hashes),
        )
        .map_err(|e| e.to_string())?;
    if review["appId"].as_u64() != Some(u64::from(app_id.value())) {
        return Err(
            "Steam validation returned another App ID; validate the destination again".into(),
        );
    }
    stop.check().map_err(|e| e.to_string())?;
    *stage = "retaining the game banner";
    let banner = retain_game_banner(&mut store, path, &document, review["gameBanner"].take())?;
    review
        .as_object_mut()
        .ok_or("Invalid synchronization review")?
        .remove("gameBanner");
    review["gameBannerAvailable"] = json!(banner.is_some());
    if let Some(document) = &banner {
        progress(Event::Banner(Box::new(document.clone())))?;
    }
    review["reconciliation"] = json!(reconciliation);
    if reconciliation && let Some(rows) = review["rows"].as_array_mut() {
        for row in rows {
            row["defaultAction"] = json!("Skip");
        }
    }
    let token = review["token"]
        .as_str()
        .ok_or("Invalid synchronization review")?
        .to_owned();
    let imports = automatic_imports(&review);
    let imported = !imports.is_empty();
    let has_changes = review["rows"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["state"] != "InSync"));
    let choices = if imported {
        progress(Event::Progress(format!(
            "Importing {} Steam entries into the project…",
            imports.len()
        )))?;
        imports
    } else if !has_changes {
        Vec::new()
    } else if let Some(choices) = automatic_upload_choices(&review, reconciliation) {
        choices
    } else {
        for (kind, field) in [("Stat", "steamStats"), ("Achievement", "steamAchievements")] {
            review[field] = json!(
                review["rows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|row| row["kind"] == kind && row["state"] != "LocalOnly")
                    .count()
            );
        }

        progress(Event::Review(review))?;
        *stage = "waiting for review approval";
        progress(Event::Progress(
            "Steam entries are in the project · review remaining uploads or conflicts".into(),
        ))?;
        loop {
            stop.check().map_err(|e| e.to_string())?;
            match approved.recv_timeout(Duration::from_millis(250)) {
                Ok(choices) => break choices,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Synchronization review closed".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => reader
                    .hold_healthy(Duration::from_millis(1))
                    .map_err(|e| e.to_string())?,
            }
        }
    };
    let choices: Vec<_> = choices
        .into_iter()
        .map(|(key, action)| json!({"key":key,"action":action}))
        .collect();
    *stage = "starting reviewed synchronization";
    let steps = reader
        .begin_sync(token.clone(), json!(choices))
        .map_err(|e| e.to_string())?;
    let steps = steps.as_array().ok_or("Invalid synchronization steps")?;
    if imported && steps.iter().any(|step| step["action"] != "Import") {
        return Err("Automatic Steam import returned an upload step".into());
    }
    reader
        .require_sync_budget(steps.len())
        .map_err(|e| e.to_string())?;
    let project_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&document).map_err(|e| e.to_string())?)
    );
    let mut journal = json!({"schemaVersion":1,"projectId":document.project()["projectId"],"projectFingerprint":project_hash,"appId":app_id.value(),"token":token,"steps":steps,"next":0,"state":"reviewed"});
    save_journal(&journal_path, &journal)?;
    let mut references = BTreeMap::new();
    for (index, step) in steps.iter().enumerate() {
        let step_started = std::time::Instant::now();
        *stage = "applying a reviewed synchronization step";
        stop.check().map_err(|e| e.to_string())?;
        let api = step["apiName"]
            .as_str()
            .ok_or("Missing synchronization target")?;
        progress(Event::Progress(format!(
            "{} · {} · {api} · {} of {}",
            step["action"].as_str().unwrap_or("Synchronizing"),
            step["key"]
                .as_str()
                .and_then(|key| key.split_once(':'))
                .map_or("Entry", |(kind, _)| kind),
            index + 1,
            steps.len()
        )))?;
        let (achieved, unachieved) = if step["needsIcons"] == true {
            *stage = "preparing the current icon upload";
            progress(Event::Progress(format!(
                "Preparing icon · {api} · {} of {}",
                index + 1,
                steps.len()
            )))?;
            let entity = document
                .synchronized_entities(EntityKind::Achievement)
                .find(|entity| entity["apiName"] == api)
                .ok_or("Icon upload target is missing")?;
            let pair = render_cancellable(
                &document,
                entity["id"].as_str().ok_or("Icon identity is missing")?,
                None,
                assets,
                &|| stop.check().is_err(),
            )?;
            let encode = |image: image::RgbaImage| -> Result<String, String> {
                let mut bytes = Cursor::new(Vec::new());
                image
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                if bytes.get_ref().len() > 8 * 1024 * 1024 {
                    return Err("Icon upload exceeds 8 MiB".into());
                }
                Ok(STANDARD.encode(bytes.into_inner()))
            };
            let bytes = (Some(encode(pair.achieved)?), Some(encode(pair.unachieved)?));
            progress(Event::Progress(format!(
                "Uploading icon · {api} · {} of {}",
                index + 1,
                steps.len()
            )))?;
            bytes
        } else {
            (None, None)
        };
        journal["next"] = json!(index);
        journal["state"] = json!("outcome-uncertain");
        save_journal(&journal_path, &journal)?;
        let result = reader
            .sync_step(
                token.clone(),
                step["key"]
                    .as_str()
                    .ok_or("Missing synchronization step")?
                    .into(),
                achieved,
                unachieved,
            )
            .map_err(|e| e.to_string())?;
        let artwork = result["artwork"]
            .as_array()
            .ok_or("Invalid imported artwork")?;
        if artwork.len() > 2
            || (!artwork.is_empty()
                && (step["action"] != "Import"
                    || !step["key"]
                        .as_str()
                        .is_some_and(|key| key.starts_with("Achievement:"))))
        {
            return Err("Artwork was returned outside an achievement import".into());
        }
        for artwork in artwork {
            let id = artwork["id"]
                .as_str()
                .ok_or("Missing imported artwork identity")?
                .to_owned();
            let encoded = artwork["bytes"]
                .as_str()
                .ok_or("Missing imported artwork")?;
            if references.contains_key(&id) {
                return Err("Imported artwork identity was repeated".into());
            }
            if encoded.len() > 12 * 1024 * 1024 {
                return Err("Imported artwork exceeds its transport budget".into());
            }
            let bytes = STANDARD
                .decode(encoded)
                .map_err(|_| "Invalid imported artwork encoding")?;
            validate_image(&bytes)?;
            references.insert(id, store.retain_art(&bytes)?);
        }
        journal["state"] = json!("verified");
        journal["next"] = json!(index + 1);
        save_journal(&journal_path, &journal)?;
        achimeld_application::diagnostics::record(
            "INFO",
            "synchronization",
            &format!(
                "Reviewed step {} of {} verified and recovery journal advanced in {} ms",
                index + 1,
                steps.len(),
                step_started.elapsed().as_millis()
            ),
        );
    }
    *stage = "verifying synchronization results";
    progress(Event::Progress(
        "Verifying final Steam synchronization results…".into(),
    ))?;
    let mut result = reader.finish_sync(token).map_err(|e| e.to_string())?;
    close_steam(&mut reader, progress, stage)?;
    *stage = "checking verified synchronization results";
    bind_result_artwork(&document, &mut result["project"], &mut references)?;
    if let Some(banner) = &banner {
        merge_game_banner(&mut result["project"], banner)?;
    }
    let mut catalog = Catalog {
        rows: serde_json::from_value(result["catalog"].take())
            .map_err(|_| "Invalid synchronized localization catalog")?,
    };
    achimeld_application::style_achievements::restore(
        &document,
        &mut result["project"],
        &mut catalog,
    )?;
    let synchronized = Document::from_parts(result["project"].take(), catalog)?;
    achimeld_application::style_achievements::require_preserved(&document, &synchronized)?;
    stop.check().map_err(|e| e.to_string())?;
    progress(Event::Progress(
        "Saving verified synchronization results…".into(),
    ))?;
    *stage = "saving synchronization results";
    store.save(&synchronized, path)?;
    // Saving may rebase staged artwork references into the sibling Art Data Bank.
    let synchronized = store.load(path)?;
    let _ = fs::remove_file(journal_path);
    if imported {
        progress(Event::Progress(format!(
            "Imported and saved {} Steam entries in the project",
            choices
                .iter()
                .filter(|choice| choice["action"] == "Import")
                .count()
        )))?;
    }
    Ok((synchronized, imported))
}

// The worker supplies bounded bytes only. The supervised host decodes, owns and
// saves the capsule as ordinary project-relative Art Data Bank artwork, even if
// the user subsequently cancels the reviewed Steam changes.
fn retain_game_banner(
    store: &mut ProjectStore,
    path: &Path,
    original: &Document,
    encoded: Value,
) -> Result<Option<Document>, String> {
    let Some(encoded) = encoded
        .as_str()
        .filter(|text| text.len() <= 3 * 1024 * 1024)
    else {
        return Ok(None);
    };
    let Ok(bytes) = STANDARD.decode(encoded) else {
        return Ok(None);
    };
    if bytes.len() > 2 * 1024 * 1024 {
        return Ok(None);
    }
    let Ok(mut reader) = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format() else {
        return Ok(None);
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(462);
    limits.max_image_height = Some(174);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let Ok(image) = reader.decode() else {
        return Ok(None);
    };
    if image.width() != 462 || image.height() != 174 {
        return Ok(None);
    }
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let reference = store.retain_art(png.get_ref())?;
    let mut project = original.project().clone();
    let images = project["assets"]["images"]
        .as_array_mut()
        .ok_or("Missing project artwork inventory")?;
    if let Some(entry) = images
        .iter_mut()
        .find(|entry| entry["steamBannerAppId"] == original.app_id())
    {
        entry["image"] = reference;
    } else {
        images.push(json!({"id":new_id(),"kind":"User","displayName":format!("Steam game banner · App {}", original.app_id()),"steamBannerAppId":original.app_id(),"image":reference}));
    }
    let document = Document::from_parts(project, original.catalog().clone())?;
    store.save(&document, path)?;
    store.load(path).map(Some)
}

fn merge_game_banner(project: &mut Value, validated: &Document) -> Result<(), String> {
    let banner = validated.project()["assets"]["images"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["steamBannerAppId"] == validated.app_id())
        .ok_or("Validated game banner is missing")?;
    let images = project["assets"]["images"]
        .as_array_mut()
        .ok_or("Invalid synchronized artwork")?;
    if let Some(entry) = images.iter_mut().find(|entry| entry["id"] == banner["id"]) {
        *entry = banner.clone();
    } else {
        images.push(banner.clone());
    }
    Ok(())
}

// A constrained worker returns data, never filesystem authority. Keep project
// identity, settings and all existing asset references pinned before any save.
// Only host-retained imported bytes may introduce a new image reference.
fn bind_result_artwork(
    original: &Document,
    project: &mut Value,
    references: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    let mut pinned = original.project().clone();
    let mut returned = project.clone();
    for value in [&mut pinned, &mut returned] {
        let object = value
            .as_object_mut()
            .ok_or("Invalid synchronized project")?;
        for key in [
            "stats",
            "achievements",
            "syncBaselines",
            "lastRemoteSnapshot",
            "lastVerifiedAtUtc",
            "nextAchievementInternalId",
            "assets",
        ] {
            object.remove(key);
        }
    }
    if pinned != returned {
        return Err("Synchronization changed the project identity or settings".into());
    }
    let mut pinned_assets = original.project()["assets"].clone();
    let mut returned_assets = project["assets"].clone();
    for value in [&mut pinned_assets, &mut returned_assets] {
        value
            .as_object_mut()
            .ok_or("Invalid synchronized assets")?
            .remove("images");
    }
    if pinned_assets != returned_assets {
        return Err("Synchronization changed existing asset references".into());
    }
    let original_images = original.project()["assets"]["images"]
        .as_array()
        .ok_or("Missing original image inventory")?;
    let images = project["assets"]["images"]
        .as_array_mut()
        .ok_or("Invalid synchronized artwork inventory")?;
    if !images.starts_with(original_images)
        || images.len() - original_images.len() != references.len()
    {
        return Err("Synchronization changed existing artwork or imported identities".into());
    }
    for asset in &mut images[original_images.len()..] {
        let object = asset.as_object().ok_or("Invalid imported artwork")?;
        if object.len() != 4
            || !object.contains_key("image")
            || asset["kind"] != "SteamImportedArtwork"
            || !asset["image"].is_null()
            || !asset["displayName"].is_string()
        {
            return Err("Synchronization returned an unexpected artwork reference".into());
        }
        let id = asset["id"]
            .as_str()
            .ok_or("Missing imported artwork identity")?;
        let reference = references
            .remove(id)
            .ok_or("Imported artwork identity was not retained by the host")?;
        asset["image"] = reference;
    }
    if !references.is_empty() {
        return Err("Imported artwork identities did not match the final project".into());
    }
    Ok(())
}

fn runtime_root() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Achimeld/RustSync"))
        .ok_or("Local application data is unavailable".into())
}
fn save_journal(path: &Path, journal: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(journal).map_err(|e| e.to_string())?;
    achimeld_persistence::project::write_atomic(path, &bytes)
}
fn validate_image(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Imported artwork exceeds 8 MiB".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_upload_uses_only_allowed_defaults_and_pauses_for_conflicts_or_recovery() {
        let mut review = json!({"rows":[{"key":"Stat:SCORE","state":"LocalOnly","defaultAction":"Upload","actions":["Skip","Upload"]}]});
        assert_eq!(
            automatic_upload_choices(&review, false),
            Some(vec![("Stat:SCORE".into(), "Upload".into())])
        );
        assert!(automatic_upload_choices(&review, true).is_none());
        for state in ["Conflict", "ArtworkUnavailable"] {
            review["rows"][0]["state"] = json!(state);
            assert!(automatic_upload_choices(&review, false).is_none());
        }
        review["rows"][0]["state"] = json!("LocalOnly");
        review["rows"][0]["actions"] = json!(["Skip"]);
        assert!(automatic_upload_choices(&review, false).is_none());
    }

    #[test]
    fn verified_deletion_clears_only_matching_history_and_pending_rename() {
        for kind in ["Achievement", "Stat"] {
            for status in ["Deleted", "AlreadyAbsent"] {
                let key = if kind == "Stat" {
                    "stats"
                } else {
                    "achievements"
                };
                let other = if kind == "Stat" {
                    "Achievement"
                } else {
                    "Stat"
                };
                let mut project = json!({"steamAppId":480,"stats":[],"achievements":[],
                    "lastRemoteSnapshot":{"stats":[],"achievements":[]},
                    "syncBaselines":[{"entityType":kind,"apiName":"OLD","steamId":"9"},
                        {"entityType":kind,"apiName":"RENAMED_HISTORY","steamId":"9"},
                        {"entityType":other,"apiName":"OLD","steamId":"9"},
                        {"entityType":kind,"apiName":"KEEP","steamId":"10"}]});
                project["lastRemoteSnapshot"][key] = json!([{"apiName":"OLD","steamId":"9"}]);
                project[key] = json!([{"apiName":"NEW","pendingApiRename":{"from":"OLD","steamId":"9","appId":480}}]);
                let result = json!({"snapshot":{"steamAppId":480,"isComplete":true,"stats":[],"achievements":[]},
                    "results":[{"kind":kind,"apiName":"OLD","status":status}]});
                let targets = json!([{"kind":kind,"apiName":"OLD"}]);
                for fault in 0..4 {
                    let mut bad = result.clone();
                    match fault {
                        0 => bad["snapshot"]["isComplete"] = json!(false),
                        1 => bad["snapshot"]["steamAppId"] = json!(481),
                        2 => bad["results"][0]["status"] = json!("Failed"),
                        _ => bad["snapshot"][key] = json!([{"apiName":"RENAMED","steamId":"9"}]),
                    }
                    let mut unchanged = project.clone();
                    assert!(reconcile_deleted_history(&mut unchanged, &bad, &targets).is_err());
                    assert_eq!(unchanged, project);
                }
                reconcile_deleted_history(&mut project, &result, &targets).unwrap();
                assert_eq!(project["syncBaselines"].as_array().unwrap().len(), 2);
                assert_eq!(project["syncBaselines"][0]["entityType"], other);
                assert_eq!(project[key][0]["apiName"], "NEW");
                assert!(project[key][0].get("pendingApiRename").is_none());
            }
        }
    }

    #[test]
    fn automatic_import_never_selects_uploads_conflicts_or_icons() {
        let review = json!({"rows":[
            {"kind":"Stat","key":"Stat:NEW","state":"SteamOnly","actions":["Skip","Import"]},
            {"kind":"Achievement","key":"Achievement:RENAMED","state":"RenamedOnSteam","actions":["Skip","Import"]},
            {"kind":"Stat","key":"Stat:UPDATED","state":"SteamChanged","actions":["Skip","Import","Upload"]},
            {"kind":"Stat","key":"Stat:CONFLICT","state":"Conflict","actions":["Skip","Import","Upload"]},
            {"kind":"Stat","key":"Stat:LOCAL","state":"LocalChanged","actions":["Skip","Import","Upload"]},
            {"kind":"Icon","key":"Icon:NEW","state":"SteamChanged","actions":["Skip","Upload"]},
            {"kind":"Stat","key":"Stat:FORGED","state":"SteamOnly","actions":["Skip","Upload"]}
        ]});
        assert_eq!(
            automatic_imports(&review),
            vec![
                ("Stat:NEW".into(), "Import".into()),
                ("Achievement:RENAMED".into(), "Import".into()),
                ("Stat:UPDATED".into(), "Import".into())
            ]
        );
    }
    #[test]
    fn validated_banner_is_owned_portable_and_preserves_identity_when_refreshed() {
        let root = crate::test_output::root().join(format!("sync-banner-{}", new_id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("banner.achv");
        let mut store = ProjectStore::new();
        let mut editor = achimeld_application::Editor::new(Document::new());
        editor
            .execute(
                "Set app",
                achimeld_application::Command::ProjectSettings {
                    name: "Banner fixture".into(),
                    app_id: 480,
                    atlas: None,
                    assets: None,
                },
                None,
            )
            .unwrap();
        store.save(editor.document(), &path).unwrap();
        let original = store.load(&path).unwrap();
        let encoded = |width, height, color| {
            let image = image::RgbaImage::from_pixel(width, height, image::Rgba(color));
            let mut png = Cursor::new(Vec::new());
            image.write_to(&mut png, image::ImageFormat::Png).unwrap();
            json!(STANDARD.encode(png.get_ref()))
        };
        for invalid in [
            Value::Null,
            json!("not base64"),
            json!(STANDARD.encode(b"bad raster")),
            encoded(461, 174, [0, 0, 0, 255]),
        ] {
            assert!(
                retain_game_banner(&mut store, &path, &original, invalid)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(store.load(&path).unwrap(), original);
        }
        let first = retain_game_banner(
            &mut store,
            &path,
            &original,
            encoded(462, 174, [20, 40, 60, 255]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            store.load(&path).unwrap(),
            first,
            "Validation retains artwork before applying Steam changes"
        );
        let entry = first.project()["assets"]["images"][0].clone();
        assert_eq!(entry["steamBannerAppId"], 480);
        assert_eq!(
            entry["kind"], "User",
            "Keep the managed asset-kind contract intact"
        );
        assert_eq!(entry["image"]["kind"], "RelativeToProject");
        let art_path = root.join(entry["image"]["path"].as_str().unwrap());
        assert_eq!(image::open(&art_path).unwrap().width(), 462);
        let refreshed = retain_game_banner(
            &mut store,
            &path,
            &first,
            encoded(462, 174, [70, 80, 90, 255]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            refreshed.project()["assets"]["images"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            refreshed.project()["assets"]["images"][0]["id"],
            entry["id"]
        );
        assert_ne!(
            refreshed.project()["assets"]["images"][0]["image"],
            entry["image"]
        );
        assert!(
            art_path.exists(),
            "Previous immutable artwork survives refresh for undo"
        );
        let mut final_project = original.project().clone();
        merge_game_banner(&mut final_project, &refreshed).unwrap();
        assert_eq!(
            final_project["assets"]["images"],
            refreshed.project()["assets"]["images"]
        );
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn worker_result_cannot_change_identity_settings_or_choose_artwork_paths() {
        let original = Document::new();
        let mut project = original.project().clone();
        assert!(bind_result_artwork(&original, &mut project, &mut BTreeMap::new()).is_ok());
        for key in ["projectId", "steamAppId", "displayName", "atlas"] {
            let mut changed = original.project().clone();
            changed[key] = json!("Changed by worker");
            assert!(bind_result_artwork(&original, &mut changed, &mut BTreeMap::new()).is_err());
        }
        let mut changed = original.project().clone();
        changed["assets"]["atlas"] = json!({"kind":"Absolute","path":"C:/outside.png"});
        assert!(bind_result_artwork(&original, &mut changed, &mut BTreeMap::new()).is_err());
        let id = new_id();
        let imported = json!({"id":id,"displayName":"Imported icon","kind":"SteamImportedArtwork","image":null});
        let mut valid = original.project().clone();
        valid["assets"]["images"]
            .as_array_mut()
            .unwrap()
            .push(imported);
        assert!(bind_result_artwork(&original, &mut valid.clone(), &mut BTreeMap::new()).is_err());
        let retained = json!({"kind":"Absolute","path":"C:/host-retained.png"});
        let mut references = BTreeMap::from([(id.clone(), retained.clone())]);
        let mut forged = valid.clone();
        forged["assets"]["images"][0]["image"] = json!({"kind":"Absolute","path":"C:/outside.png"});
        assert!(bind_result_artwork(&original, &mut forged, &mut references.clone()).is_err());
        assert!(bind_result_artwork(&original, &mut valid, &mut references).is_ok());
        assert_eq!(valid["assets"]["images"][0]["image"], retained);
        assert!(references.is_empty());
    }
    #[test]
    fn synchronization_journal_is_durable_and_replaced_before_a_step() {
        let directory = crate::test_output::root().join(format!("sync-journal-{}", new_id()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("journal.json");
        save_journal(&path, &json!({"state":"reviewed","next":0})).unwrap();
        save_journal(&path, &json!({"state":"outcome-uncertain","next":1})).unwrap();
        let data: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(data["state"], "outcome-uncertain");
        assert_eq!(data["next"], 1);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
