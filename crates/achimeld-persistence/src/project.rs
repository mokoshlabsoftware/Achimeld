use achimeld_application::{Catalog, Document, new_id};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, String>;
pub const COLUMNS: [&str; 9] = [
    "ID",
    "Context",
    "English",
    "Polish",
    "ChineseSimplified",
    "ChineseTraditional",
    "Japanese",
    "Korean",
    "French",
];
const MAX_PROJECT: u64 = 32 * 1024 * 1024;
const MAX_CATALOG: u64 = 64 * 1024 * 1024;

pub struct ProjectStore {
    path: Option<PathBuf>,
    origin: Option<PathBuf>,
    catalog_path: Option<PathBuf>,
    expected: Option<(String, String)>,
    art_staging: Option<PathBuf>,
    staged_art_files: std::collections::BTreeSet<PathBuf>,
}
impl Default for ProjectStore {
    fn default() -> Self {
        Self::new()
    }
}
impl ProjectStore {
    pub fn new() -> Self {
        Self {
            path: None,
            origin: None,
            catalog_path: None,
            expected: None,
            art_staging: None,
            staged_art_files: Default::default(),
        }
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn asset_base(&self) -> Option<&Path> {
        self.origin.as_deref().or(self.path.as_deref())
    }
    /// Retain immutable imported bytes immediately, including before the first save.
    /// History may still reference an older version after resize/reload.
    pub fn retain_art(&mut self, bytes: &[u8]) -> Result<Value> {
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Artwork exceeds 64 MiB".into());
        }
        if self.art_staging.is_none() {
            let directory = std::env::temp_dir().join(format!("achimeld-art-{}", new_id()));
            reject_reparse(&directory)?;
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
            self.art_staging = Some(directory);
        }
        let directory = self
            .art_staging
            .as_deref()
            .ok_or("Art staging unavailable")?;
        let path = directory.join(format!("{}.png", hash(bytes)));
        retain_immutable(&path, bytes)?;
        self.staged_art_files.insert(path.clone());
        Ok(json!({"kind":"Absolute","path":path.to_str().ok_or("Invalid art bank path")?}))
    }
    pub fn load(&mut self, path: &Path) -> Result<Document> {
        let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
        let path = absolute.as_path();
        check_extension(path)?;
        let (_lock, needs_recovery) = load_guard(path)?;
        if needs_recovery {
            recover_locked(path)?;
        }
        let bytes = read_limited(path, MAX_PROJECT)?;
        achimeld_security::wire::check_json_depth(&bytes).map_err(|e| e.to_string())?;
        let project: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !matches!(project["schemaVersion"].as_u64(), Some(0 | 1)) {
            return Err("Unsupported project version; original files were preserved".into());
        }
        let catalog_path = resolve_reference(&project["assets"]["localizationCatalog"], path)?;
        let csv = read_limited(&catalog_path, MAX_CATALOG).map_err(|error|format!("Cannot load project localization CSV '{}': {error}. Keep the matching CSV with this project; the current document was preserved.",catalog_path.display()))?;
        if project["schemaVersion"] == 1
            && project["localizationCatalogSha256"]
                .as_str()
                .is_none_or(|v| !v.eq_ignore_ascii_case(&hash(&csv)))
        {
            return Err("Project localization hash does not match its CSV".into());
        }
        let document = Document::from_parts(project, parse_catalog(&csv)?)?;
        self.path = Some(path.to_owned());
        self.origin = Some(path.to_owned());
        self.catalog_path = Some(catalog_path);
        self.expected = Some((hash(&bytes), hash(&csv)));
        Ok(document)
    }
    /// Adopt a verified background save while retaining the asset base used by
    /// existing editor history. New references are rebased to that same base.
    pub fn adopt_saved_result(&mut self, path: &Path, expected: &Document) -> Result<Document> {
        let mut saved_store = Self::new();
        let saved = saved_store.load(path)?;
        if &saved != expected {
            return Err("The saved synchronization result changed on disk".into());
        }
        let mut project = saved.project().clone();
        if let Some(origin) = &self.origin {
            rebase_references(&mut project, path, origin)?;
        }
        let document = Document::from_parts(project, saved.catalog().clone())?;
        self.path = saved_store.path.take();
        self.catalog_path = saved_store.catalog_path.take();
        self.expected = saved_store.expected.take();
        if self.origin.is_none() {
            self.origin = saved_store.origin.take();
        }
        Ok(document)
    }
    pub fn save(&mut self, document: &Document, path: &Path) -> Result<()> {
        let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
        let path = absolute.as_path();
        document.validate()?;
        check_extension(path)?;
        let _lock = project_lock(path)?;
        recover_locked(path)?;
        let parent = path.parent().ok_or("Project directory unavailable")?;
        reject_reparse(parent)?;
        let stem = path
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or("Invalid project filename")?;
        let csv_name = format!("{stem}.achimeld.localization.csv");
        let csv_path = parent.join(&csv_name);
        let owned = self.path.as_deref().is_some_and(|old| same_path(old, path));
        if !owned && (path.exists() || csv_path.exists()) {
            return Err("Save as requires an unused project and localization filename".into());
        }
        if csv_path.exists()
            && !self
                .catalog_path
                .as_deref()
                .is_some_and(|old| same_path(old, &csv_path))
        {
            return Err("The destination localization CSV belongs to another file; choose an unused project filename".into());
        }
        if owned && let Some((json_hash, csv_hash)) = &self.expected {
            if hash(&read_limited(path, MAX_PROJECT)?) != *json_hash {
                return Err("Project changed on disk; reload before saving".into());
            }
            let old_csv = self
                .catalog_path
                .as_deref()
                .ok_or("Loaded catalog location unavailable")?;
            if hash(&read_limited(old_csv, MAX_CATALOG)?) != *csv_hash {
                return Err("Localization CSV changed on disk; reload before saving".into());
            }
        }
        let csv = write_catalog(document.catalog())?;
        let mut project = document.project().clone();
        if let Some(old) = &self.origin {
            rebase_references(&mut project, old, path)?;
        }
        own_art_references(&mut project, path, &self.staged_art_files)?;
        project["format"] = json!("com.achimeld.project");
        project["schemaVersion"] = json!(1);
        project["assets"]["localizationCatalog"] =
            json!({"kind":"RelativeToProject","path":csv_name});
        project["localizationCatalogSha256"] = json!(hash(&csv));
        let mut bytes = serde_json::to_vec_pretty(&project).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if bytes.len() as u64 > MAX_PROJECT {
            return Err("Project exceeds its size limit".into());
        }
        // Stage both new files and both originals before publishing the recovery record.
        // Recovery restores the entire previous pair unless commit was recorded.
        let token = new_id();
        let files = [path.to_owned(), csv_path.clone()];
        let contents = [bytes.as_slice(), csv.as_slice()];
        let mut entries = Vec::new();
        let mut staged_files = StagedFiles::default();
        for (target, content) in files.iter().zip(contents) {
            reject_reparse(target)?;
            let name = target
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("Invalid filename")?;
            let staged = format!(".{name}.{token}.new");
            let backup = format!(".{name}.{token}.old");
            let existed = target.exists();
            let original = if existed {
                Some(read_limited(target, MAX_CATALOG)?)
            } else {
                None
            };
            if let Some(bytes) = &original {
                write_new(&parent.join(&backup), bytes)?;
                staged_files.paths.push(parent.join(&backup));
            }
            write_new(&parent.join(&staged), content)?;
            staged_files.paths.push(parent.join(&staged));
            entries.push(json!({"target":name,"staged":staged,"backup":backup,"existed":existed,"oldSha256":original.as_ref().map(|b|hash(b)),"newSha256":hash(content)}));
        }
        let journal = transaction_path(path);
        write_new(&journal,&serde_json::to_vec(&json!({"format":"com.achimeld.rust-project-transaction","state":"prepared","project":path.file_name().and_then(|s|s.to_str()),"files":entries})).map_err(|e|e.to_string())?)?;
        staged_files.recovery_owns_files = true;
        for entry in &entries {
            replace(
                &parent.join(entry["staged"].as_str().unwrap_or("")),
                &parent.join(entry["target"].as_str().unwrap_or("")),
            )?;
        }
        // A synced marker is separate: no torn in-place journal rewrite can erase rollback information.
        write_new(&commit_path(path), b"committed\n")?;
        self.path = Some(path.to_owned());
        if self.origin.is_none() {
            self.origin = Some(path.to_owned());
        }
        self.catalog_path = Some(csv_path);
        self.expected = Some((hash(&bytes), hash(&csv)));
        // Record the durable pair even if cleanup fails, so retrying does not mistake
        // our committed bytes for an external edit.
        recover_locked(path)?;
        Ok(())
    }
}

impl Drop for ProjectStore {
    fn drop(&mut self) {
        // Only our freshly created flat staging directory is eligible for cleanup.
        if let Some(directory) = &self.art_staging {
            if reject_reparse(directory).is_err() {
                return;
            }
            for path in &self.staged_art_files {
                if path.parent() == Some(directory.as_path()) && reject_reparse(path).is_ok() {
                    let _ = fs::remove_file(path);
                }
            }
            let _ = fs::remove_dir(directory);
        }
    }
}

fn retain_immutable(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_reparse(path)?;
    if path.exists() {
        if read_limited(path, 64 * 1024 * 1024)? != bytes {
            return Err("Art bank content changed; existing files were preserved".into());
        }
        Ok(())
    } else {
        write_new(path, bytes)
    }
}

fn own_art_references(
    project: &mut Value,
    path: &Path,
    staged: &std::collections::BTreeSet<PathBuf>,
) -> Result<()> {
    fn own(
        reference: &mut Value,
        path: &Path,
        staged: &std::collections::BTreeSet<PathBuf>,
    ) -> Result<()> {
        if reference.is_null() {
            return Ok(());
        }
        let source = resolve_reference(reference, path)?;
        // Legacy broken references stay repairable. New imports are retained before
        // entering the document, so they never depend on the original source file.
        if !source.exists() {
            if staged.contains(&source) {
                return Err(
                    "Retained artwork is unavailable; reload it in Art Data Bank before saving"
                        .into(),
                );
            }
            return Ok(());
        }
        let bytes = read_limited(&source, 64 * 1024 * 1024)?;
        let stem_hash = source.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let managed = staged.contains(&source)
            || (source
                .parent()
                .and_then(Path::file_name)
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.ends_with(".art-data-bank"))
                && stem_hash.len() == 64
                && stem_hash.bytes().all(|b| b.is_ascii_hexdigit()));
        if managed && !stem_hash.eq_ignore_ascii_case(&hash(&bytes)) {
            return Err(
                "Retained artwork changed on disk; reload it in Art Data Bank before saving".into(),
            );
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid project name")?;
        let parent = path.parent().ok_or("Project directory unavailable")?;
        let directory = parent.join(format!("{stem}.art-data-bank"));
        reject_reparse(&directory)?;
        if !directory.exists() {
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
        }
        // Filename is derived solely from content and a closed extension list.
        let extension = source
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let extension = match extension.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "webp" => extension.as_str(),
            _ => "image",
        };
        let target = directory.join(format!("{}.{extension}", hash(&bytes)));
        retain_immutable(&target, &bytes)?;
        *reference = json!({"kind":"RelativeToProject","path":target.strip_prefix(parent).map_err(|e| e.to_string())?.to_str().ok_or("Invalid art bank path")?});
        Ok(())
    }
    own(&mut project["assets"]["atlas"], path, staged)?;
    for key in ["images", "atlases"] {
        if let Some(items) = project["assets"][key].as_array_mut() {
            for item in items {
                own(&mut item["image"], path, staged)?;
            }
        }
    }
    Ok(())
}

fn check_extension(path: &Path) -> Result<()> {
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("achv"))
    {
        return Err("Project files must use the .achv extension".into());
    }
    Ok(())
}
pub fn resolve_reference(reference: &Value, project: &Path) -> Result<PathBuf> {
    let text = reference["path"].as_str().ok_or("Missing asset path")?;
    let path = PathBuf::from(text);
    match reference["kind"].as_str() {
        Some("RelativeToProject")
            if !path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                )
            }) =>
        {
            std::path::absolute(
                project
                    .parent()
                    .ok_or("Project directory unavailable")?
                    .join(path),
            )
            .map_err(|e| e.to_string())
        }
        Some("Absolute") if path.is_absolute() => {
            std::path::absolute(path).map_err(|e| e.to_string())
        }
        _ => Err("Invalid project path reference".into()),
    }
}
fn same_path(first: &Path, second: &Path) -> bool {
    if first == second {
        return true;
    }
    #[cfg(windows)]
    if let (Some(first), Some(second)) = (first.to_str(), second.to_str()) {
        return first.eq_ignore_ascii_case(second);
    }
    false
}
fn rebase_references(value: &mut Value, old: &Path, new: &Path) -> Result<()> {
    fn rebase(value: &mut Value, old: &Path, new: &Path) -> Result<()> {
        if value.is_null() {
            return Ok(());
        }
        let resolved = resolve_reference(value, old)?;
        let absolute = std::path::absolute(resolved).map_err(|e| e.to_string())?;
        if let Ok(relative) =
            absolute.strip_prefix(new.parent().ok_or("Project directory unavailable")?)
        {
            value["kind"] = json!("RelativeToProject");
            value["path"] = json!(
                relative
                    .to_str()
                    .ok_or("Asset path cannot be represented in project JSON")?
            );
        } else {
            value["kind"] = json!("Absolute");
            value["path"] = json!(
                absolute
                    .to_str()
                    .ok_or("Asset path cannot be represented in project JSON")?
            );
        }
        Ok(())
    }
    // Only schema-defined references are ours to rewrite. Future metadata is opaque.
    if let Some(assets) = value.get_mut("assets") {
        for key in ["atlas", "localizationCatalog"] {
            if let Some(reference) = assets.get_mut(key) {
                rebase(reference, old, new)?;
            }
        }
        for key in ["images", "atlases"] {
            if let Some(items) = assets.get_mut(key).and_then(Value::as_array_mut) {
                for item in items {
                    if let Some(reference) = item.get_mut("image") {
                        rebase(reference, old, new)?;
                    }
                }
            }
        }
    }
    Ok(())
}
fn project_lock(path: &Path) -> Result<fs::File> {
    check_extension(path)?;
    let lock_path = path.with_extension("achv.rust-lock");
    reject_reparse(&lock_path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options
        .open(lock_path)
        .map_err(|e| format!("Project is locked or unavailable: {e}"))
}
fn load_guard(path: &Path) -> Result<(fs::File, bool)> {
    if transaction_path(path).exists() || commit_path(path).exists() {
        return project_lock(path).map(|file| (file, true));
    }
    let lock_path = path.with_extension("achv.rust-lock");
    reject_reparse(&lock_path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    match options.open(lock_path) {
        Ok(file) => Ok((file, false)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // An untouched project need not create a lock in a read-only directory.
            // Pin the JSON against write/delete while reading the hash-bound CSV.
            reject_reparse(path)?;
            let mut options = OpenOptions::new();
            options.read(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(1);
            }
            options
                .open(path)
                .map(|file| (file, false))
                .map_err(|e| e.to_string())
        }
        Err(error) => Err(format!("Project is locked or unavailable: {error}")),
    }
}
fn transaction_path(path: &Path) -> PathBuf {
    path.with_extension("achv.rust-transaction.json")
}
fn commit_path(path: &Path) -> PathBuf {
    path.with_extension("achv.rust-transaction.committed")
}
pub fn recover(path: &Path) -> Result<()> {
    let _lock = project_lock(path)?;
    recover_locked(path)
}
fn recover_locked(path: &Path) -> Result<()> {
    let journal = transaction_path(path);
    if !journal.exists() {
        let marker = commit_path(path);
        reject_reparse(&marker)?;
        if marker.exists() {
            fs::remove_file(marker).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let data: Value = serde_json::from_slice(&read_limited(&journal, 64 * 1024)?)
        .map_err(|_| "Invalid project recovery record")?;
    if data["format"] != "com.achimeld.rust-project-transaction"
        || data["project"].as_str() != path.file_name().and_then(|s| s.to_str())
        || data["state"] != "prepared"
    {
        return Err("Recovery record identity rejected".into());
    }
    let parent = path.parent().ok_or("Project directory unavailable")?;
    let entries = data["files"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or("Invalid recovery inventory")?;
    let project_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid filename")?;
    let csv_name = format!(
        "{}.achimeld.localization.csv",
        path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?
    );
    let marker = commit_path(path);
    reject_reparse(&marker)?;
    let committed = marker.exists();
    if committed
        && !matches!(
            read_limited(&marker, 32)?.as_slice(),
            b"committed\n" | b"rolled-back\n"
        )
    {
        return Err("Invalid recovery commit marker".into());
    }
    let mut cleanup = Vec::new();
    let mut rollback = Vec::new();
    let mut transaction_token: Option<String> = None;
    for (i, entry) in entries.iter().enumerate() {
        let expected = if i == 0 { project_name } else { &csv_name };
        if entry["target"] != expected {
            return Err("Recovery target rejected".into());
        }
        let existed = entry["existed"]
            .as_bool()
            .ok_or("Invalid recovery file state")?;
        let valid_hash = |v: &Value| {
            v.as_str()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        };
        if !valid_hash(&entry["newSha256"])
            || (existed && !valid_hash(&entry["oldSha256"]))
            || (!existed && !entry["oldSha256"].is_null())
        {
            return Err("Invalid recovery file hash".into());
        }
        let target = parent.join(expected);
        let mut auxiliary = Vec::new();
        for key in ["staged", "backup"] {
            let name = entry[key].as_str().ok_or("Missing recovery path")?;
            let token = name
                .strip_prefix(&format!(".{expected}."))
                .and_then(|s| s.strip_suffix(if key == "staged" { ".new" } else { ".old" }))
                .ok_or("Recovery path rejected")?;
            if token.len() != 36
                || !token.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
            {
                return Err("Recovery path rejected".into());
            }
            if transaction_token.as_deref().is_some_and(|v| v != token) {
                return Err("Recovery files belong to different transactions".into());
            }
            transaction_token.get_or_insert_with(|| token.to_owned());
            let aux = parent.join(name);
            reject_reparse(&aux)?;
            if aux.exists() {
                let expected_hash = if key == "staged" {
                    &entry["newSha256"]
                } else {
                    &entry["oldSha256"]
                };
                if expected_hash.as_str() != Some(&hash(&read_limited(&aux, MAX_CATALOG)?)) {
                    return Err("Recovery auxiliary file changed; files were preserved".into());
                }
            }
            auxiliary.push(aux);
        }
        reject_reparse(&target)?;
        if !committed {
            if target.exists() {
                let actual = hash(&read_limited(&target, MAX_CATALOG)?);
                if entry["oldSha256"].as_str() != Some(&actual)
                    && entry["newSha256"].as_str() != Some(&actual)
                {
                    return Err("Project changed during recovery; files were preserved".into());
                }
            }
            if existed {
                if !auxiliary[1].exists() {
                    return Err("Project recovery backup missing".into());
                }
                let bytes = read_limited(&auxiliary[1], MAX_CATALOG)?;
                if entry["oldSha256"].as_str() != Some(&hash(&bytes)) {
                    return Err("Project recovery backup hash differs; files were preserved".into());
                }
                rollback.push((target, Some(bytes)));
            } else {
                rollback.push((target, None));
            }
        }
        cleanup.extend(auxiliary);
    }
    // Validate the entire inventory before changing either member of the pair.
    for (target, bytes) in rollback {
        if let Some(bytes) = bytes {
            write_atomic(&target, &bytes)?;
        } else if target.exists() {
            fs::remove_file(target).map_err(|e| e.to_string())?;
        }
    }
    // Keep every backup until the entire rollback is durable. Interrupted recovery
    // can then retry; interrupted cleanup needs no backups once this marker exists.
    if !committed {
        write_new(&commit_path(path), b"rolled-back\n")?;
    }
    for aux in cleanup {
        if aux.exists() {
            fs::remove_file(aux).map_err(|e| e.to_string())?;
        }
    }
    fs::remove_file(journal).map_err(|e| e.to_string())?;
    let marker = commit_path(path);
    if marker.exists() {
        fs::remove_file(marker).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>> {
    reject_reparse(path)?;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > max {
        return Err("File exceeds its size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("File exceeds its size limit".into());
    }
    Ok(bytes)
}
fn reject_reparse(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if !ancestor.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Linked project paths are not supported".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Reparse project paths are not supported".into());
            }
        }
    }
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
#[derive(Default)]
struct StagedFiles {
    paths: Vec<PathBuf>,
    recovery_owns_files: bool,
}
impl Drop for StagedFiles {
    fn drop(&mut self) {
        if !self.recovery_owns_files {
            for path in &self.paths {
                let _ = fs::remove_file(path);
            }
        }
    }
}
fn replace(source: &Path, target: &Path) -> Result<()> {
    achimeld_security::storage::replace_file(source, target).map_err(|e| e.to_string())
}
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("File directory unavailable")?;
    reject_reparse(parent)?;
    reject_reparse(path)?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid filename")?;
    let staged = parent.join(format!(".{name}.{}.tmp", new_id()));
    write_new(&staged, bytes)?;
    let result = replace(&staged, path);
    if result.is_err() && staged.exists() {
        let _ = fs::remove_file(staged);
    }
    result
}
fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn parse_catalog(bytes: &[u8]) -> Result<Catalog> {
    if bytes.len() as u64 > MAX_CATALOG {
        return Err("Catalog is too large".into());
    }
    let text = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
        .map_err(|_| "Catalog must be UTF-8")?;
    let records = achimeld_core::csv::records(text)?;
    if records
        .first()
        .is_none_or(|v| v.iter().map(String::as_str).ne(COLUMNS))
    {
        return Err("Unsupported localization CSV header".into());
    }
    let mut catalog = Catalog::default();
    for row in records.into_iter().skip(1) {
        if row.len() != 9 {
            return Err("Incorrect localization column count".into());
        }
        let id = row[0].clone();
        let fields: BTreeMap<_, _> = COLUMNS
            .into_iter()
            .skip(1)
            .zip(row.into_iter().skip(1))
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        if catalog.rows.insert(id, fields).is_some() {
            return Err("Duplicate localization ID".into());
        }
    }
    Ok(catalog)
}
pub fn write_catalog(catalog: &Catalog) -> Result<Vec<u8>> {
    fn field(value: &str) -> String {
        if value.contains([',', '"', '\r', '\n']) {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.into()
        }
    }
    let mut text = format!("{}\r\n", COLUMNS.join(","));
    for (id, row) in &catalog.rows {
        let mut values = vec![id.clone()];
        values.extend(
            COLUMNS
                .iter()
                .skip(1)
                .map(|k| row.get(*k).cloned().unwrap_or_default()),
        );
        text.push_str(
            &values
                .iter()
                .map(|v| field(v))
                .collect::<Vec<_>>()
                .join(","),
        );
        text.push_str("\r\n");
    }
    if text.len() as u64 > MAX_CATALOG {
        return Err("Catalog is too large".into());
    }
    Ok(text.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_style_library_survives_save_as_and_restart_without_user_settings() {
        let directory = Directory::new();
        let first = directory.0.join("styles.achv");
        let second = directory.0.join("copy.achv");
        let style = achimeld_application::IconStyle {
            preview_png: None,
            id: new_id(),
            name: "Project only".into(),
            recipe: json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FF123456"}]}),
        };
        let mut editor = Editor::default();
        editor
            .execute("Style", Command::SaveIconStyle(style.clone()), None)
            .unwrap();
        let mut store = ProjectStore::new();
        store.save(editor.document(), &first).unwrap();
        let loaded = ProjectStore::new().load(&first).unwrap();
        assert_eq!(loaded.icon_styles(), vec![style.clone()]);
        store.save(&loaded, &second).unwrap();
        assert_eq!(
            ProjectStore::new().load(&second).unwrap().icon_styles(),
            vec![style]
        );
        assert!(Document::new().icon_styles().is_empty());
    }
    #[test]
    fn art_is_owned_deduplicated_and_portable_after_staging_is_gone() {
        let directory = Directory::new();
        let first = directory.0.join("first.achv");
        let copy_directory = directory.0.join("copy");
        fs::create_dir(&copy_directory).unwrap();
        let second = copy_directory.join("second.achv");
        let mut store = ProjectStore::new();
        let reference = store.retain_art(b"immutable-art-fixture").unwrap();
        assert_eq!(
            store.retain_art(b"immutable-art-fixture").unwrap(),
            reference
        );
        let mut project = Document::new().project().clone();
        project["assets"]["images"] =
            json!([{"id":new_id(),"displayName":"Art","image":reference}]);
        let document = Document::from_parts(project, Catalog::default()).unwrap();
        store.save(&document, &first).unwrap();
        let loaded = ProjectStore::new().load(&first).unwrap();
        let owned =
            resolve_reference(&loaded.project()["assets"]["images"][0]["image"], &first).unwrap();
        assert!(owned.starts_with(directory.0.join("first.art-data-bank")));
        store.save(&document, &second).unwrap();
        store.save(&document, &second).unwrap();
        drop(store);
        assert!(!Path::new(reference["path"].as_str().unwrap()).exists());
        let copy = ProjectStore::new().load(&second).unwrap();
        let copy_art =
            resolve_reference(&copy.project()["assets"]["images"][0]["image"], &second).unwrap();
        assert!(copy_art.starts_with(copy_directory.join("second.art-data-bank")));
        assert_eq!(fs::read(copy_art).unwrap(), b"immutable-art-fixture");
        assert_eq!(
            fs::read_dir(copy_directory.join("second.art-data-bank"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn art_content_collision_preserves_existing_bytes() {
        let mut store = ProjectStore::new();
        let reference = store.retain_art(b"original").unwrap();
        let path = Path::new(reference["path"].as_str().unwrap());
        fs::write(path, b"changed outside the app").unwrap();
        assert!(store.retain_art(b"original").is_err());
        assert_eq!(fs::read(path).unwrap(), b"changed outside the app");
    }
    #[test]
    fn local_style_achievement_and_general_artwork_survive_portable_save() {
        use achimeld_application::{Command, Editor, EntityKind};
        let directory = Directory::new();
        let path = directory.0.join("style.achv");
        let mut store = ProjectStore::new();
        let reference = store.retain_art(b"local-style-image").unwrap();
        let image_id = new_id();
        let mut editor = Editor::default();
        editor
            .execute(
                "Local style",
                Command::AddStyleAchievementAt { position: [3, -7] },
                None,
            )
            .unwrap();
        let mut project = editor.document().project().clone();
        project["assets"]["images"] = json!([{"id":image_id,"displayName":"General artwork","image":reference,"kind":"User"}]);
        let local = &mut project["achievements"][0];
        local["unlockValue"] = json!(42.125);
        local["generalImageEntries"] =
            json!([{"id":0,"kind":"StandaloneImage","imageId":image_id}]);
        local["icon"] = json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0,"visible":true}]});
        let document = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        store.save(&document, &path).unwrap();
        drop(store);
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(
            loaded.entities(EntityKind::Achievement),
            document.entities(EntityKind::Achievement)
        );
        assert_eq!(loaded.catalog(), document.catalog());
        assert_eq!(loaded.icon_styles(), document.icon_styles());
        assert_eq!(
            loaded.entities(EntityKind::Achievement)[0]["iconStyleId"],
            loaded.icon_styles()[0].id
        );
        let owned =
            resolve_reference(&loaded.project()["assets"]["images"][0]["image"], &path).unwrap();
        assert!(owned.starts_with(directory.0.join("style.art-data-bank")));
        assert_eq!(fs::read(owned).unwrap(), b"local-style-image");
    }
    #[test]
    fn editable_art_image_and_general_links_survive_owned_portable_save() {
        let directory = Directory::new();
        let path = directory.0.join("composed.achv");
        let mut store = ProjectStore::new();
        let reference = store.retain_art(b"owned-art-input").unwrap();
        let mut editor = achimeld_application::Editor::default();
        editor
            .execute(
                "Art and achievement",
                achimeld_application::Command::Batch(vec![
                    achimeld_application::Command::AddArtImageAt { position: [0, 0] },
                    achimeld_application::Command::Add(EntityKind::Achievement),
                ]),
                None,
            )
            .unwrap();
        let mut project = editor.document().project().clone();
        let image = new_id();
        project["assets"]["images"] =
            json!([{"id":image,"displayName":"Art input","image":reference}]);
        let art = project["achievements"][0]["id"].clone();
        project["achievements"][0]["generalImageEntries"] = json!([{"id":0,"imageId":image}]);
        project["achievements"][0]["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}]});
        project["achievements"][1]["generalImageEntries"] = json!([{"id":0,"artImageId":art}]);
        project["achievements"][1]["icon"] =
            json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":0}]});
        let document = Document::from_parts(project, editor.document().catalog().clone()).unwrap();
        store.save(&document, &path).unwrap();
        drop(store);
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(
            loaded.entities(EntityKind::Achievement),
            document.entities(EntityKind::Achievement)
        );
        let owned =
            resolve_reference(&loaded.project()["assets"]["images"][0]["image"], &path).unwrap();
        assert!(owned.starts_with(directory.0.join("composed.art-data-bank")));
        assert_eq!(fs::read(owned).unwrap(), b"owned-art-input");
    }
    #[test]
    fn lost_or_changed_staged_art_cannot_be_published_as_a_saved_project() {
        let directory = Directory::new();
        let destination = directory.0.join("art.achv");
        let mut store = ProjectStore::new();
        let reference = store.retain_art(b"original-art").unwrap();
        let mut project = Document::new().project().clone();
        project["assets"]["images"] =
            json!([{"id":new_id(),"displayName":"Art","image":reference}]);
        let document = Document::from_parts(project, Catalog::default()).unwrap();
        let staged = Path::new(reference["path"].as_str().unwrap());
        fs::write(staged, b"changed-art").unwrap();
        assert!(store.save(&document, &destination).is_err());
        assert!(!destination.exists());
        fs::remove_file(staged).unwrap();
        assert!(store.save(&document, &destination).is_err());
        assert!(!destination.exists());
    }
    use achimeld_application::{Command, Editor, EntityKind};
    #[test]
    fn saved_unlock_templates_remain_editable_after_reload() {
        let directory = Directory::new();
        let path = directory.0.join("unlock-template.achv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let name = entity["displayNameLocalizationId"]
            .as_str()
            .unwrap()
            .to_owned();
        entity["unlockValue"] = json!(5);
        editor
            .execute(
                "Text",
                Command::Batch(vec![
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: entity["id"].as_str().unwrap().into(),
                        value: entity,
                    },
                    Command::SetLocalization {
                        id: name.clone(),
                        language: "English".into(),
                        text: "Find {0} sticks".into(),
                    },
                ]),
                None,
            )
            .unwrap();
        ProjectStore::new().save(editor.document(), &path).unwrap();
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(loaded.catalog().text(&name, "English"), "Find {0} sticks");
        assert_eq!(
            loaded.display_name(&loaded.entities(EntityKind::Achievement)[0], "English"),
            "Find 5 sticks"
        );
    }

    #[test]
    fn text3d_recipe_survives_save_reload_and_history() {
        let directory = Directory::new();
        let path = directory.0.join("text3d.achv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let settings = achimeld_application::text3d::Text3dSettings {
            projection: achimeld_application::text3d::TextProjection::Orthographic,
            extrusion: 90.0,
            push: -2.5,
            emboss: 6.0,
            output: achimeld_application::text3d::Text3dOutput::EmbossMaskMinusFace,
            reflectivity: 0.8,
            shadow_strength: 0.75,
            smooth_normals: true,
            rotation_y: 110.0,
            position_z: 75.0,
            shading: achimeld_application::text3d::TextShading::Glossy,
            ..Default::default()
        };
        let recipe = json!({"layers":[{"id":new_id(),"kind":"Text3d","text":{"content":"3D text","fontSize":192.0,"fontFamily":"Segoe UI","colorHex":"#FFFFCC55"},"text3d":settings}]});
        entity["icon"] = recipe.clone();
        editor
            .execute(
                "3D text",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        assert!(editor.undo());
        assert!(editor.redo());
        ProjectStore::new().save(editor.document(), &path).unwrap();
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(
            loaded.entity(EntityKind::Achievement, &id).unwrap()["icon"],
            recipe
        );
    }

    #[test]
    fn artifact3d_recipe_survives_save_reload_and_history() {
        use achimeld_application::artifact3d::*;
        let directory = Directory::new();
        let path = directory.0.join("artifact3d.achv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let mut settings = ArtifactSettings {
            parts: preset(ArtifactPreset::Cup),
            output: ArtifactOutput::AlphaMask,
            ..Default::default()
        };
        settings.parts[0].size[0] = 243.0;
        settings.parts[3].sweep_angle = 145.0;
        let recipe = json!({"layers":[{"id":new_id(),"kind":"Artifact3d","artifact3d":settings}]});
        entity["icon"] = recipe.clone();
        editor
            .execute(
                "3D artifact",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        assert!(editor.undo());
        assert!(editor.redo());
        ProjectStore::new().save(editor.document(), &path).unwrap();
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(
            loaded.entity(EntityKind::Achievement, &id).unwrap()["icon"],
            recipe
        );
    }

    #[test]
    fn explosion_project_round_trip_retains_ranges_and_single_value_transform() {
        let directory = Directory::new();
        let path = directory.0.join("explosion.achv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let mut settings = achimeld_application::explosion::ExplosionSettings {
            count_min: Some(2),
            count: 100,
            scale_min: 0.4,
            scale_max: 1.2,
            deltoid: Some(achimeld_application::explosion::DeltoidSettings {
                scale: 1.5,
                scale_min: 0.5,
                scale_max: 1.25,
                top_height_min: 0.1,
                top_height_max: 0.4,
                bottom_height_min: 0.3,
                bottom_height_max: 0.6,
                ..Default::default()
            }),
            ..Default::default()
        };
        settings.cover_canvas = true;
        settings.gradient["kind"] = json!("SDF");
        let recipe = json!({"layers":[{"id":new_id(),"kind":"Explosion","explosion":settings,
            "offsetX":32.0,"offsetY":-14.0,"rotation":20.0}]});
        entity["icon"] = recipe.clone();
        editor
            .execute(
                "Explosion",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: id.clone(),
                    value: entity,
                },
                None,
            )
            .unwrap();
        ProjectStore::new().save(editor.document(), &path).unwrap();
        let loaded = ProjectStore::new().load(&path).unwrap();
        assert_eq!(
            loaded.entity(EntityKind::Achievement, &id).unwrap()["icon"],
            recipe
        );
    }

    #[test]
    #[ignore = "Requires ACHIMELD_LEGACY_PROJECT pointing to a local copy with its authentic catalog"]
    fn legacy_project_preserves_every_entity_recipe_translation_and_camera_on_save_as() {
        let path = PathBuf::from(
            std::env::var_os("ACHIMELD_LEGACY_PROJECT").expect("Set ACHIMELD_LEGACY_PROJECT"),
        );
        let original_bytes = fs::read(&path).unwrap();
        let original: Value = serde_json::from_slice(&original_bytes).unwrap();
        let catalog_path =
            resolve_reference(&original["assets"]["localizationCatalog"], &path).unwrap();
        let original_csv = fs::read(&catalog_path).unwrap();
        let mut store = ProjectStore::new();
        let loaded = store.load(&path).unwrap();
        assert_eq!(loaded.entities(EntityKind::Achievement).len(), 100);
        assert_eq!(loaded.entities(EntityKind::Stat).len(), 32);
        let mut compatible = loaded.project().clone();
        if original.get("iconStyles").is_none() {
            compatible.as_object_mut().unwrap().remove("iconStyles");
        }
        assert_eq!(compatible, original);
        assert_eq!(loaded.catalog(), &parse_catalog(&original_csv).unwrap());
        let directory = Directory::new();
        let saved = directory.0.join("copy.achv");
        store.save(&loaded, &saved).unwrap();
        let reloaded = ProjectStore::new().load(&saved).unwrap();
        for key in [
            "achievements",
            "stats",
            "achievementsCamera",
            "statsCamera",
            "tombstones",
            "syncBaselines",
            "lastRemoteSnapshot",
        ] {
            assert_eq!(
                reloaded.project()[key],
                original[key],
                "Legacy field {key} changed"
            );
        }
        assert_eq!(reloaded.catalog(), loaded.catalog());
        assert_eq!(fs::read(&path).unwrap(), original_bytes);
        assert_eq!(fs::read(catalog_path).unwrap(), original_csv);
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let root = crate::test_output::root().join(format!("project-test-{}", new_id()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            if self.0.starts_with(crate::test_output::root()) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    fn interrupted(path: &Path) -> Value {
        let parent = path.parent().unwrap();
        let token = new_id();
        let mut entries = Vec::new();
        for (name, old, new) in [
            ("test.achv", b"old-json".as_slice(), b"new-json".as_slice()),
            (
                "test.achimeld.localization.csv",
                b"old-csv".as_slice(),
                b"new-csv".as_slice(),
            ),
        ] {
            let staged = format!(".{name}.{token}.new");
            let backup = format!(".{name}.{token}.old");
            write_new(&parent.join(&backup), old).unwrap();
            write_new(&parent.join(&staged), new).unwrap();
            write_new(
                &parent.join(name),
                if name == "test.achv" { new } else { old },
            )
            .unwrap();
            entries.push(json!({"target":name,"staged":staged,"backup":backup,"existed":true,"oldSha256":hash(old),"newSha256":hash(new)}));
        }
        json!({"format":"com.achimeld.rust-project-transaction","state":"prepared","project":"test.achv","files":entries})
    }
    #[test]
    fn interrupted_save_recovers_both_files_and_recovery_is_idempotent() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        let journal = interrupted(&path);
        write_new(
            &transaction_path(&path),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        recover(&path).unwrap();
        recover(&path).unwrap();
        assert_eq!(fs::read(path).unwrap(), b"old-json");
        assert_eq!(
            fs::read(directory.0.join("test.achimeld.localization.csv")).unwrap(),
            b"old-csv"
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 3); // Pair and persistent handle-lock file.
    }
    #[test]
    fn invalid_second_recovery_member_cannot_change_the_first() {
        for corrupt_backup in [false, true] {
            let directory = Directory::new();
            let path = directory.0.join("test.achv");
            let mut journal = interrupted(&path);
            if corrupt_backup {
                fs::write(
                    directory
                        .0
                        .join(journal["files"][1]["backup"].as_str().unwrap()),
                    b"changed-backup",
                )
                .unwrap();
            } else {
                journal["files"][1]["target"] = json!("other.csv");
            }
            write_new(
                &transaction_path(&path),
                &serde_json::to_vec(&journal).unwrap(),
            )
            .unwrap();
            assert!(recover(&path).is_err());
            assert_eq!(fs::read(path).unwrap(), b"new-json");
            assert_eq!(
                fs::read(directory.0.join("test.achimeld.localization.csv")).unwrap(),
                b"old-csv"
            );
        }
    }
    #[test]
    fn save_as_preserves_assets_across_repeated_saves_and_detects_external_edits() {
        let directory = Directory::new();
        let first = directory.0.join("test.achv");
        let subdirectory = directory.0.join("copy");
        fs::create_dir(&subdirectory).unwrap();
        let second = subdirectory.join("copy.achv");
        let mut store = ProjectStore::new();
        store.save(&Document::new(), &first).unwrap();
        let document = store.load(&first).unwrap();
        store.save(&document, &second).unwrap();
        store.save(&document, &second).unwrap();
        let loaded = ProjectStore::new().load(&second).unwrap();
        assert_eq!(
            resolve_reference(&loaded.project()["assets"]["atlas"], &second).unwrap(),
            std::path::absolute(directory.0.join("atlas.png")).unwrap()
        );
        fs::write(
            subdirectory.join("copy.achimeld.localization.csv"),
            b"external edit",
        )
        .unwrap();
        assert!(store.save(&document, &second).is_err());
        assert_eq!(
            fs::read(subdirectory.join("copy.achimeld.localization.csv")).unwrap(),
            b"external edit"
        );
    }
    #[test]
    fn project_and_multiline_catalog_round_trip_preserve_opaque_fields() {
        let root = crate::test_output::root().join(format!("project-test-{}", new_id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("test.achv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut opaque = editor.document().entities(EntityKind::Achievement)[0].clone();
        let entity_id = opaque["id"].as_str().unwrap().to_owned();
        opaque["futureEditorMetadata"] = json!({"nested":[1,2,{"text":"retain me"}],"kind":"RelativeToProject","path":"do-not-rewrite"});
        opaque["icon"] = json!({"layers":[{"id":new_id(),"kind":"SdfVignette",
            "sdfVignette":{"colorHex":"#FF123456","invert":true,"useGradient":true,
                "roundness":0.84,"inset":0.0,"softness":1.0,"gradient":{
                    "kind":"Linear","angle":90.0,"stops":[
                        {"offset":0.0,"colorHex":"#FF36D1DC"},
                        {"offset":0.5,"colorHex":"#807C4DFF"},
                        {"offset":1.0,"colorHex":"#FFFF4FA3"}]}},"effects":[
                            {"id":new_id(),"kind":"Skew","skewX":24.5,"skewY":-10.0},
                            {"id":new_id(),"kind":"Perspective","perspectiveX":0.25,"perspectiveY":-0.125},
                            {"id":new_id(),"kind":"Pinch","amount":0.75},
                            {"id":new_id(),"kind":"LensBlur","blur":8.0,"aberration":12.0}]}]});
        editor
            .execute(
                "Metadata",
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: entity_id,
                    value: opaque,
                },
                None,
            )
            .unwrap();
        let id =
            editor.document().entities(EntityKind::Achievement)[0]["displayNameLocalizationId"]
                .as_str()
                .unwrap()
                .to_owned();
        editor
            .execute(
                "Text",
                Command::SetLocalization {
                    id,
                    language: "English".into(),
                    text: "Comma, quote \" and\nnewline".into(),
                },
                None,
            )
            .unwrap();
        let mut store = ProjectStore::new();
        store.save(editor.document(), &path).unwrap();
        let loaded = store.load(&path).unwrap();
        assert_eq!(loaded.catalog(), editor.document().catalog());
        assert_eq!(
            loaded.entities(EntityKind::Achievement),
            editor.document().entities(EntityKind::Achievement)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn separate_catalog_cannot_overwrite_an_unrelated_sibling() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        let sibling = directory.0.join("test.achimeld.localization.csv");
        let external = directory.0.join("other.csv");
        let mut store = ProjectStore::new();
        store.save(&Document::new(), &path).unwrap();
        fs::copy(&sibling, &external).unwrap();
        let mut project: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        project["assets"]["localizationCatalog"]["path"] = json!("other.csv");
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        fs::write(&sibling, b"unrelated data").unwrap();
        let document = store.load(&path).unwrap();
        assert!(store.save(&document, &path).is_err());
        assert_eq!(fs::read(&sibling).unwrap(), b"unrelated data");
        assert_eq!(
            fs::read(&external).unwrap(),
            write_catalog(document.catalog()).unwrap()
        );
    }
    #[cfg(windows)]
    #[test]
    fn project_handle_lock_prevents_concurrent_save_and_reopens_after_release() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        let lock = project_lock(&path).unwrap();
        assert!(ProjectStore::new().save(&Document::new(), &path).is_err());
        assert!(!path.exists());
        drop(lock);
        ProjectStore::new().save(&Document::new(), &path).unwrap();
    }
    #[test]
    fn committed_recovery_preserves_modified_auxiliary_files() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        let journal = interrupted(&path);
        let backup = directory
            .0
            .join(journal["files"][1]["backup"].as_str().unwrap());
        fs::write(&backup, b"external edit").unwrap();
        write_new(
            &transaction_path(&path),
            &serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        write_new(&commit_path(&path), b"committed\n").unwrap();
        assert!(recover(&path).is_err());
        assert_eq!(fs::read(backup).unwrap(), b"external edit");
        assert!(transaction_path(&path).exists());
    }
    #[cfg(windows)]
    #[test]
    fn relative_references_reject_drive_relative_and_rooted_paths() {
        for text in ["C:asset.png", "\\asset.png", "C:\\asset.png"] {
            assert!(
                resolve_reference(
                    &json!({"kind":"RelativeToProject","path":text}),
                    Path::new("C:\\projects\\test.achv")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn dot_relative_catalog_paths_remain_owned_on_save() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        let mut store = ProjectStore::new();
        store.save(&Document::new(), &path).unwrap();
        let mut project: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        project["assets"]["localizationCatalog"]["path"] =
            json!("./test.achimeld.localization.csv");
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        let document = store.load(&path).unwrap();
        store
            .save(&document, &directory.0.join(".").join("test.achv"))
            .unwrap();
        #[cfg(windows)]
        store
            .save(&document, &directory.0.join("TEST.achv"))
            .unwrap();
        assert_eq!(
            ProjectStore::new().load(&path).unwrap().catalog(),
            document.catalog()
        );
    }
    #[test]
    fn loading_without_recovery_creates_no_sidecar() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        ProjectStore::new().save(&Document::new(), &path).unwrap();
        fs::remove_file(path.with_extension("achv.rust-lock")).unwrap();
        ProjectStore::new().load(&path).unwrap();
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
    #[cfg(windows)]
    #[test]
    fn read_only_lock_still_allows_loading_without_recovery() {
        let directory = Directory::new();
        let path = directory.0.join("test.achv");
        ProjectStore::new().save(&Document::new(), &path).unwrap();
        let lock_path = path.with_extension("achv.rust-lock");
        let original_permissions = fs::metadata(&lock_path).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&lock_path, permissions).unwrap();
        let loaded = ProjectStore::new().load(&path);
        fs::set_permissions(&lock_path, original_permissions).unwrap();
        assert!(loaded.is_ok());
    }
}
