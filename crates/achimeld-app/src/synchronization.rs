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
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

pub(crate) enum Event {
    Banner(Box<Document>),
    Imported(Box<Document>),
    Review(Value),
    IconFiles(BTreeMap<String, Vec<PathBuf>>),
    Progress(String),
    Complete(Result<Document, String>),
}
pub(crate) struct Pending {
    pub cancel: Cancellation,
    pub events: mpsc::Receiver<Event>,
    pub approval: mpsc::SyncSender<Vec<(String, String)>>,
    pub review: Option<Value>,
    pub revision: u64,
    pub path: PathBuf,
    pub thread: Option<std::thread::JoinHandle<()>>,
    pub previews: IconPreviews,
}
impl Pending {
    pub fn start(
        path: PathBuf,
        document: Document,
        revision: u64,
        mut assets: impl AssetSource + Send + 'static,
        ctx: eframe::egui::Context,
    ) -> Self {
        let cancel = Cancellation::default();
        let stop = cancel.clone();
        let (sender, events) = mpsc::sync_channel(8);
        let (approval, approved) = mpsc::sync_channel(1);
        let destination = path.clone();
        let thread = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let mut stage = "checking saved project";
            let progress = |event| -> Result<(), String> {
                if let Event::Progress(message) = &event {
                    achimeld_application::diagnostics::record("INFO", "validation", message);
                }
                sender
                    .send(event)
                    .map_err(|_| "Synchronization view closed".to_owned())?;
                ctx.request_repaint();
                Ok(())
            };
            let result = run(
                &destination,
                document,
                &mut assets,
                &stop,
                &approved,
                &progress,
                &mut stage,
            );
            match &result {
                Ok(_) => achimeld_application::diagnostics::record(
                    "INFO",
                    "synchronization",
                    &format!("Completed in {} ms", started.elapsed().as_millis()),
                ),
                Err(error) => achimeld_application::diagnostics::record(
                    "ERROR",
                    "validation",
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
    let mut generated = Generated::new(&root)?;
    let mut pairs = BTreeMap::new();
    let mut hashes = serde_json::Map::new();
    let mut disk_bytes = 0_u64;
    *stage = "preparing achievement artwork";
    for (index, entity) in document
        .synchronized_entities(EntityKind::Achievement)
        .enumerate()
    {
        stop.check().map_err(|e| e.to_string())?;
        let api = entity["apiName"]
            .as_str()
            .ok_or("Achievement has no API name")?;
        progress(Event::Progress(format!("Preparing artwork · {api}")))?;
        let result = (|| {
            let pair = render_cancellable(
                &document,
                entity["id"].as_str().ok_or("Missing achievement ID")?,
                None,
                assets,
                &|| stop.check().is_err(),
            )?;
            let mut paths = Vec::new();
            let mut digests = Vec::new();
            for (side, image) in [pair.achieved, pair.unachieved].into_iter().enumerate() {
                if image.width() > 4096 || image.height() > 4096 {
                    return Err("Icon upload resolution exceeds 4096 pixels".to_owned());
                }
                let mut bytes = Cursor::new(Vec::new());
                image
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                let bytes = bytes.into_inner();
                if bytes.len() > 8 * 1024 * 1024
                    || disk_bytes + bytes.len() as u64 > 512 * 1024 * 1024
                {
                    return Err("Generated synchronization artwork exceeds its budget".into());
                }
                disk_bytes += bytes.len() as u64;
                let destination = generated.directory.join(format!("{index}-{side}.png"));
                write_new(&destination, &bytes)?;
                generated.files.push(destination.clone());
                paths.push(destination);
                digests.push(format!("{:x}", Sha256::digest(&bytes)));
            }
            Ok::<_, String>((paths, digests))
        })();
        if let Ok((paths, digests)) = result {
            pairs.insert(api.to_owned(), paths);
            hashes.insert(
                api.into(),
                json!({"achieved":digests[0],"unachieved":digests[1]}),
            );
        } else if let Err(error) = result {
            achimeld_application::diagnostics::record(
                "WARN",
                "validation",
                &format!(
                    "Artwork preparation skipped for achievement {} · {error}",
                    index + 1
                ),
            );
        }
    }
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
        progress(Event::IconFiles(pairs.clone()))?;
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
        *stage = "applying a reviewed synchronization step";
        stop.check().map_err(|e| e.to_string())?;
        let api = step["apiName"]
            .as_str()
            .ok_or("Missing synchronization target")?;
        progress(Event::Progress(format!(
            "{} · {api} · {} of {}",
            step["action"].as_str().unwrap_or("Synchronizing"),
            index + 1,
            steps.len()
        )))?;
        let (achieved, unachieved) = if step["needsIcons"] == true {
            let paths = pairs
                .get(api)
                .ok_or("The reviewed artwork is unavailable")?;
            (
                Some(STANDARD.encode(read_bounded(&paths[0], 8 * 1024 * 1024)?)),
                Some(STANDARD.encode(read_bounded(&paths[1], 8 * 1024 * 1024)?)),
            )
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
    }
    *stage = "verifying synchronization results";
    let mut result = reader.finish_sync(token).map_err(|e| e.to_string())?;
    reader.stop().map_err(|e| e.to_string())?;
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
    let synchronized = accept_imported_icons(synchronized, &choices, assets, stop)?;
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

// Import accepts the retained artwork and its rendered project representation.
// Keep both identities: Steam may convert uploads and the project may resize an
// imported raster. A subsequent edit must still compare against these bytes.
fn accept_imported_icons(
    document: Document,
    choices: &[Value],
    assets: &mut dyn AssetSource,
    stop: &Cancellation,
) -> Result<Document, String> {
    use achimeld_core::canonical::{Value as CanonicalValue, compute};
    let mut project = document.project().clone();
    for api in choices
        .iter()
        .filter(|choice| choice["action"] == "Import")
        .filter_map(|choice| choice["key"].as_str()?.strip_prefix("Achievement:"))
    {
        let entity = document
            .synchronized_entities(EntityKind::Achievement)
            .find(|entity| entity["apiName"] == api)
            .ok_or("Imported achievement is missing")?;
        if entity["icon"]["layers"]
            .as_array()
            .is_none_or(|layers| layers.len() != 1 || layers[0]["kind"] != "StandaloneImage")
        {
            continue;
        }
        let remote = project["lastRemoteSnapshot"]["icons"]
            .as_array()
            .and_then(|icons| icons.iter().find(|icon| icon["achievementApiName"] == api))
            .and_then(|icon| icon["contentFingerprint"].as_str())
            .map(str::to_owned)
            .ok_or("Imported icon has no verified Steam identity")?;
        let pair = render_cancellable(
            &document,
            entity["id"]
                .as_str()
                .ok_or("Missing imported achievement identity")?,
            None,
            assets,
            &|| stop.check().is_err(),
        )?;
        let mut fields = Vec::new();
        for (name, image) in [
            ("achievedIdentity", pair.achieved),
            ("unachievedIdentity", pair.unachieved),
        ] {
            let mut bytes = Cursor::new(Vec::new());
            image
                .write_to(&mut bytes, image::ImageFormat::Png)
                .map_err(|error| error.to_string())?;
            fields.push((
                name.into(),
                CanonicalValue::Text(format!("{:x}", Sha256::digest(bytes.get_ref()))),
            ));
        }
        let local = compute(fields).map_err(|error| error.to_string())?.entity;
        let baseline = project["syncBaselines"]
            .as_array_mut()
            .and_then(|baselines| {
                baselines.iter_mut().find(|baseline| {
                    baseline["entityType"] == "Achievement"
                        && baseline["localId"] == entity["id"]
                        && baseline["apiName"] == api
                })
            })
            .ok_or("Imported icon has no verified baseline")?;
        baseline["localIconFingerprint"] = json!(local);
        baseline["remoteIconFingerprint"] = json!(remote);
    }
    Document::from_parts(project, document.catalog().clone())
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
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}
fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > maximum {
        return Err("Synchronization file exceeds its budget".into());
    }
    Ok(bytes)
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
struct Generated {
    directory: PathBuf,
    files: Vec<PathBuf>,
}
impl Generated {
    fn new(root: &Path) -> Result<Self, String> {
        let directory = root.join(format!("generated-{}", new_id()));
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        Ok(Self {
            directory,
            files: Vec::new(),
        })
    }
}
impl Drop for Generated {
    fn drop(&mut self) {
        for path in &self.files {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_icon_baseline_accepts_only_import_choices_and_detects_later_recipe_changes() {
        struct Imported;
        impl AssetSource for Imported {
            fn image(&mut self, _: &Value) -> Result<std::sync::Arc<image::RgbaImage>, String> {
                Ok(std::sync::Arc::new(image::RgbaImage::from_pixel(
                    2,
                    2,
                    image::Rgba([220, 30, 90, 255]),
                )))
            }
            fn font(&mut self, _: &str) -> Option<std::sync::Arc<achimeld_imaging::Font>> {
                None
            }
        }
        let mut editor = achimeld_application::Editor::default();
        editor
            .execute(
                "Add",
                achimeld_application::Command::Add(EntityKind::Achievement),
                None,
            )
            .unwrap();
        let mut project = editor.document().project().clone();
        let id = project["achievements"][0]["id"].clone();
        let api = project["achievements"][0]["apiName"]
            .as_str()
            .unwrap()
            .to_owned();
        let image_id = new_id();
        project["achievements"][0]["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"StandaloneImage","imageId":image_id}]});
        project["assets"]["images"] = json!([{"id":image_id,"kind":"SteamImportedArtwork","displayName":"Imported","image":{"kind":"RelativeToProject","path":"imported.png"}}]);
        project["lastRemoteSnapshot"] =
            json!({"icons":[{"achievementApiName":api,"contentFingerprint":"a".repeat(64)}]});
        project["syncBaselines"] = json!([{"entityType":"Achievement","localId":id,"apiName":api,"localIconFingerprint":null,"remoteIconFingerprint":null}]);
        let document = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        let choice = json!({"key":format!("Achievement:{api}"),"action":"Import"});
        let skipped = accept_imported_icons(
            document.clone(),
            &[json!({"key":format!("Achievement:{api}"),"action":"Skip"})],
            &mut Imported,
            &Cancellation::default(),
        )
        .unwrap();
        assert!(skipped.project()["syncBaselines"][0]["localIconFingerprint"].is_null());
        let accepted = accept_imported_icons(
            document,
            std::slice::from_ref(&choice),
            &mut Imported,
            &Cancellation::default(),
        )
        .unwrap();
        let before = accepted.project()["syncBaselines"][0].clone();
        assert_eq!(before["localIconFingerprint"].as_str().unwrap().len(), 64);
        assert_eq!(before["remoteIconFingerprint"], "a".repeat(64));
        let mut changed = accepted.project().clone();
        changed["achievements"][0]["icon"]["layers"][0]["opacity"] = json!(0.5);
        let changed = Document::from_parts(changed, accepted.catalog().clone()).unwrap();
        let after =
            accept_imported_icons(changed, &[choice], &mut Imported, &Cancellation::default())
                .unwrap();
        assert_ne!(
            before["localIconFingerprint"],
            after.project()["syncBaselines"][0]["localIconFingerprint"]
        );
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
    #[test]
    fn generated_artwork_cleanup_removes_only_its_owned_files() {
        let root = crate::test_output::root();
        let mut stage = Generated::new(&root).unwrap();
        let path = stage.directory.join("icon.png");
        write_new(&path, b"fixture").unwrap();
        stage.files.push(path.clone());
        let directory = stage.directory.clone();
        drop(stage);
        assert!(!path.exists() && !directory.exists());
        assert!(validate_image(b"not an image").is_err());
    }
}
