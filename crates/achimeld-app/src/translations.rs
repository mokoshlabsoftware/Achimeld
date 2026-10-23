//! Bounded translator-file I/O in the supervised host, away from the paint thread.
use achimeld_application::{Command, Document, new_id, translations};
use achimeld_persistence::project::{parse_catalog, write_catalog};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

const LIMIT: u64 = 64 * 1024 * 1024;

pub(crate) enum Outcome {
    Exported(PathBuf),
    Imported(Command, usize),
}
pub(crate) struct Pending {
    pub project_id: String,
    pub revision: u64,
    pub receiver: mpsc::Receiver<Result<Outcome, String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Pending {
    pub fn start(
        document: Document,
        revision: u64,
        path: PathBuf,
        import: bool,
        ctx: eframe::egui::Context,
    ) -> Self {
        let project_id = document.project()["projectId"]
            .as_str()
            .unwrap_or("")
            .into();
        let (sender, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let result = if import {
                read(&document, &path).map(|(command, count)| Outcome::Imported(command, count))
            } else {
                export(&document, &path).map(|()| Outcome::Exported(path))
            };
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        Self {
            project_id,
            revision,
            receiver,
            thread: Some(thread),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn csv_path(path: &Path) -> Result<(), String> {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    {
        return Err("Choose a CSV file".into());
    }
    Ok(())
}

pub(crate) fn read(document: &Document, path: &Path) -> Result<(Command, usize), String> {
    csv_path(path)?;
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err("Translation CSV must be a file of at most 64 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    translations::import_achievements(document, &parse_catalog(&bytes)?)
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(crate) fn csv_bytes(document: &Document) -> Result<Vec<u8>, String> {
    document.validate()?;
    let bytes = write_catalog(&translations::achievement_catalog(document)?)?;
    if bytes.len() as u64 > LIMIT - 3 {
        return Err("Translation CSV exceeds 64 MiB".into());
    }
    Ok(bytes)
}

fn export(document: &Document, path: &Path) -> Result<(), String> {
    export_cancellable(document, path, &|| false)
}

pub(crate) fn export_cancellable(
    document: &Document,
    path: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    csv_path(path)?;
    if path.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".achimeld.localization.csv")
    }) {
        return Err(
            "Choose a separate translation filename, such as achievements.translations.csv".into(),
        );
    }
    if cancelled() {
        return Err("Translation export cancelled".into());
    }
    let bytes = csv_bytes(document)?;
    let parent = path.parent().ok_or("Choose an export folder")?;
    let temporary = parent.join(format!(".achimeld-translations-{}.tmp", new_id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let _stage = Stage(temporary.clone());
    // A UTF-8 BOM allows spreadsheet editors to recognize all supported scripts.
    file.write_all(&[0xef, 0xbb, 0xbf])
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    if cancelled() {
        return Err("Translation export cancelled".into());
    }
    // Publish complete bytes and refuse existing files, including a target created
    // after the picker validated it. The project-owned catalog is never replaced.
    fs::hard_link(&temporary, path)
        .map_err(|error| format!("Could not export CSV; choose an unused filename: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use achimeld_application::{Editor, EntityKind};

    #[test]
    fn csv_round_trip_handles_unicode_quotes_lines_and_never_overwrites() {
        let root = crate::test_output::root().join(format!("translations-{}", new_id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("achievements.csv");
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let id =
            editor.document().entities(EntityKind::Achievement)[0]["descriptionLocalizationId"]
                .as_str()
                .unwrap()
                .to_owned();
        editor
            .execute(
                "Source",
                Command::SetLocalization {
                    id: id.clone(),
                    language: "English".into(),
                    text: "Find {0}, \"now\"\nPlease".into(),
                },
                None,
            )
            .unwrap();
        export(editor.document(), &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        assert_eq!(read(editor.document(), &path).unwrap().1, 0);
        assert!(export(editor.document(), &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let cancelled = root.join("cancelled.csv");
        let checks = std::cell::Cell::new(0);
        assert!(
            export_cancellable(editor.document(), &cancelled, &|| {
                checks.set(checks.get() + 1);
                checks.get() >= 2
            })
            .is_err()
        );
        assert!(!cancelled.exists());
        assert!(
            export(
                editor.document(),
                &root.join("project.achimeld.localization.csv")
            )
            .is_err()
        );
        let mut copy = parse_catalog(&bytes).unwrap();
        copy.rows
            .get_mut(&id)
            .unwrap()
            .insert("Polish".into(), "Znajdź {0}, \"teraz\"\nProszę".into());
        copy.rows
            .get_mut(&id)
            .unwrap()
            .insert("Japanese".into(), "今すぐ{0}個".into());
        fs::write(&path, write_catalog(&copy).unwrap()).unwrap();
        let (command, count) = read(editor.document(), &path).unwrap();
        assert_eq!(count, 2);
        editor.execute("Import", command, None).unwrap();
        assert_eq!(
            editor.document().catalog().rows[&id]["Polish"],
            copy.rows[&id]["Polish"]
        );
        let row = "aBc123,context,English,,,,,,\r\n";
        fs::write(
            &path,
            format!(
                "{}\r\n{row}{row}",
                achimeld_persistence::project::COLUMNS.join(",")
            ),
        )
        .unwrap();
        assert!(read(editor.document(), &path).is_err());
        fs::write(&path, b"wrong,header\n").unwrap();
        assert!(read(editor.document(), &path).is_err());
        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(read(editor.document(), &path).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
