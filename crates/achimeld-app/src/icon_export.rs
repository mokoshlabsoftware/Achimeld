use achimeld_application::{Document, EntityKind, new_id};
use achimeld_imaging::{AssetSource, render_cancellable};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

struct Export {
    directory: PathBuf,
    staged: Vec<PathBuf>,
    published: Vec<(PathBuf, [u8; 32])>,
    complete: bool,
}
impl Drop for Export {
    fn drop(&mut self) {
        if !self.complete {
            for (path, digest) in &self.published {
                if file_hash(path).is_ok_and(|actual| actual == *digest) {
                    let _ = fs::remove_file(path);
                }
            }
        }
        for path in &self.staged {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir(&self.directory);
    }
}
pub fn export(
    document: &Document,
    directory: &Path,
    assets: &mut dyn AssetSource,
    cancelled: impl Fn() -> bool,
) -> Result<(), String> {
    document.validate()?;
    let mut names = std::collections::BTreeSet::new();
    let mut destinations = Vec::new();
    for entity in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| !achimeld_application::art_images::is_art_image(entity))
    {
        let api = entity["apiName"].as_str().ok_or("Missing API name")?;
        if api.contains(':')
            || api.starts_with('.')
            || api.ends_with('.')
            || matches!(
                api.split('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_uppercase()
                    .as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
        {
            return Err(format!("{api} cannot be exported as a Windows filename"));
        }
        for name in [format!("{api}.png"), format!("{api}-unachieved.png")] {
            if !names.insert(name.to_ascii_lowercase()) || directory.join(&name).exists() {
                return Err("Icon export filenames collide or already exist. Choose an empty output folder.".into());
            }
            destinations.push(directory.join(name));
        }
    }
    let stage = directory.join(format!(".achimeld-export-{}", new_id()));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let mut transaction = Export {
        directory: stage,
        staged: Vec::new(),
        published: Vec::new(),
        complete: false,
    };
    // Render and encode one achievement at a time. No image list grows with the project.
    for (entity, pair) in document
        .entities(EntityKind::Achievement)
        .iter()
        .filter(|entity| !achimeld_application::art_images::is_art_image(entity))
        .zip(destinations.chunks(2))
    {
        if cancelled() {
            return Err("Icon export cancelled".into());
        }
        let icons = render_cancellable(
            document,
            entity["id"].as_str().ok_or("Missing achievement ID")?,
            None,
            assets,
            &cancelled,
        )?;
        for (image, destination) in [(&icons.achieved, &pair[0]), (&icons.unachieved, &pair[1])] {
            let path = transaction
                .directory
                .join(destination.file_name().ok_or("Invalid export filename")?);
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            transaction.staged.push(path);
            let mut writer = BufWriter::new(file);
            image
                .write_to(&mut writer, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            writer.flush().map_err(|e| e.to_string())?;
            writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        }
    }
    for (staged, destination) in transaction.staged.iter().zip(destinations) {
        if cancelled() {
            return Err("Icon export cancelled".into());
        }
        let digest = file_hash(staged)?;
        // Both paths are on the same volume. Linking publishes a complete synced
        // file atomically, refuses existing targets, and leaves our stage for cleanup.
        fs::hard_link(staged, &destination).map_err(|e| e.to_string())?;
        transaction.published.push((destination, digest));
    }
    transaction.complete = true;
    Ok(())
}
fn file_hash(path: &Path) -> Result<[u8; 32], String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
        return Err("Invalid export file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Linked export path rejected".into());
        }
    }
    let mut file = fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(128 * 1024 * 1024 + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    let mut bytes = 0;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes += count;
        if bytes > 128 * 1024 * 1024 {
            return Err("Export file grew beyond its limit".into());
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().into())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Assets;
    impl AssetSource for Assets {
        fn image(
            &mut self,
            _: &serde_json::Value,
        ) -> Result<std::sync::Arc<image::RgbaImage>, String> {
            Err("No assets".into())
        }
        fn font(&mut self, _: &str) -> Option<std::sync::Arc<achimeld_imaging::Font>> {
            None
        }
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = crate::test_output::root().join(format!("export-test-{}", new_id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn document() -> Document {
        let mut editor = achimeld_application::Editor::default();
        editor
            .execute(
                "Fixture",
                achimeld_application::Command::Batch(vec![
                    achimeld_application::Command::Add(EntityKind::Achievement),
                    achimeld_application::Command::Add(EntityKind::Achievement),
                ]),
                None,
            )
            .unwrap();
        let replacements=editor.document().entities(EntityKind::Achievement).iter().map(|entity| {
            let mut entity=entity.clone();entity["icon"]=serde_json::json!({"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF"}]});
            achimeld_application::Command::ReplaceEntity{kind:EntityKind::Achievement,id:entity["id"].as_str().unwrap().into(),value:entity}
        }).collect();
        editor
            .execute(
                "Synthetic artwork",
                achimeld_application::Command::Batch(replacements),
                None,
            )
            .unwrap();
        editor.document().clone()
    }
    #[test]
    fn collision_never_overwrites_existing_file() {
        let directory = Directory::new();
        let path = directory.0.join("ACHIEVEMENT_001.png");
        fs::write(&path, b"keep").unwrap();
        assert!(export(&document(), &directory.0, &mut Assets, || false).is_err());
        assert_eq!(fs::read(path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
    #[test]
    fn cancellation_after_first_publication_removes_only_own_exports() {
        let directory = Directory::new();
        let keep = directory.0.join("keep.txt");
        fs::write(&keep, b"keep").unwrap();
        assert!(
            export(&document(), &directory.0, &mut Assets, || {
                directory.0.join("ACHIEVEMENT_001.png").exists()
            })
            .is_err()
        );
        assert_eq!(fs::read(keep).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
    #[test]
    fn cancellation_preserves_an_export_modified_by_another_writer() {
        let directory = Directory::new();
        let published = directory.0.join("ACHIEVEMENT_001.png");
        assert!(
            export(&document(), &directory.0, &mut Assets, || {
                if published.exists() {
                    fs::write(&published, b"external edit").unwrap();
                    true
                } else {
                    false
                }
            })
            .is_err()
        );
        assert_eq!(fs::read(published).unwrap(), b"external edit");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
    #[test]
    fn failed_legacy_image_render_publishes_no_blank_files() {
        let directory = Directory::new();
        let document = document();
        let mut project = document.project().clone();
        project["achievements"][0]["icon"] = serde_json::Value::Null;
        let document = Document::from_parts(project, document.catalog().clone()).unwrap();
        assert!(export(&document, &directory.0, &mut Assets, || false).is_err());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
}
