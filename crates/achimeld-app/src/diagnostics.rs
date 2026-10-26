//! Host adapter for safe application and worker diagnostics.
use achimeld_application::diagnostics::{self as output, PersistenceStatus};
use std::path::{Path, PathBuf};
mod monitor;
pub use monitor::Monitor;

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // Third-party browser/rendering logs can include URLs or document data.
        metadata.level() <= log::Level::Info && metadata.target().starts_with("achimeld")
    }
    fn log(&self, entry: &log::Record<'_>) {
        if self.enabled(entry.metadata()) {
            achimeld_application::diagnostics::record(
                entry.level().as_str(),
                entry.target(),
                &entry.args().to_string(),
            );
        }
    }
    fn flush(&self) {}
}

pub fn initialize() {
    output::install_sink(persist);
    initialize_persistence();
    static LOGGER: Logger = Logger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

struct PersistentLog {
    file: std::fs::File,
    path: PathBuf,
    bytes: usize,
    // Excludes concurrent instances from rotating an active session.
    _lock: std::fs::File,
}

impl PersistentLog {
    fn open(directory: &std::path::Path) -> std::io::Result<Self> {
        Self::open_slot(directory, 0)
    }

    fn available(directory: &Path) -> std::io::Result<Self> {
        let mut result = Self::open(directory);
        for slot in 1..=4 {
            if result
                .as_ref()
                .is_err_and(|e| matches!(e.raw_os_error(), Some(32 | 33)))
            {
                result = Self::open_slot(directory, slot);
            } else {
                break;
            }
        }
        result
    }

    fn open_slot(directory: &std::path::Path, slot: u8) -> std::io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let stem = if slot == 0 {
            "log".to_owned()
        } else {
            format!("log-instance-{slot}")
        };
        let current = directory.join(format!("{stem}.txt"));
        let backup = directory.join(format!("{stem}_bak.txt"));
        let lock = directory.join(format!("{stem}.lock"));
        achimeld_security::windows::reject_reparse(directory).map_err(std::io::Error::other)?;
        for path in [&current, &backup, &lock] {
            validate_file(path)?;
        }
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(lock)?;
        if current.exists() {
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            std::fs::rename(&current, backup)?;
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(1)
            .open(&current)?;
        Ok(Self {
            file,
            path: current,
            bytes: 0,
            _lock: lock,
        })
    }

    fn write(&mut self, line: &str) -> std::io::Result<()> {
        use std::io::{Seek, Write};
        // Bound disk use while retaining the latest records, including panics.
        if self.bytes + line.len() > 8 * 1024 * 1024 {
            self.file.set_len(0)?;
            self.file.rewind()?;
            let marker = b"Earlier records discarded at the 8 MiB session limit.\n";
            self.file.write_all(marker)?;
            self.bytes = marker.len();
        }
        self.file.write_all(line.as_bytes())?;
        self.file.flush()?;
        self.bytes += line.len();
        Ok(())
    }
}

fn persistent() -> &'static std::sync::Mutex<Option<PersistentLog>> {
    static LOG: std::sync::OnceLock<std::sync::Mutex<Option<PersistentLog>>> =
        std::sync::OnceLock::new();
    LOG.get_or_init(|| std::sync::Mutex::new(None))
}

fn persist(line: &str) {
    let mut log = persistent()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(writer) = log.as_mut()
        && let Err(error) = writer.write(line)
    {
        let status = failure_status(writer.path.parent().map(Path::to_path_buf), "write", &error);
        // Release the failed handle so the independent monitor can retry. The
        // current message still reaches the bounded in-memory buffer afterwards.
        *log = None;
        output::set_persistence_status(status);
    }
}

fn initialize_persistence() {
    if let Some(directory) = diagnostic_directory() {
        output::install_previous_sessions(read_previous(&directory));
    }
    retry_persistence();
    if let Some(message) = output::persistence_status().failure {
        output::record("ERROR", "diagnostics", &message);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Panic payloads may contain document data or browser errors. Retain only
        // source location and a static reason; the previous stderr hook still runs.
        let message = info.location().map_or_else(
            || "Unhandled Rust panic".to_owned(),
            |location| {
                format!(
                    "Unhandled Rust panic at {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            },
        );
        achimeld_application::diagnostics::record("ERROR", "panic", &message);
        previous(info);
    }));
}

fn diagnostic_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Achimeld/RustDiagnostics"))
}

// Check the already-validated directory's direct children without erasing the
// Windows sharing-violation code needed to try another instance slot.
fn validate_file(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_attributes() & 0x400 != 0 || !metadata.is_file() => Err(
            std::io::Error::other("Diagnostic file must be a regular non-reparse file"),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn failure_status(
    directory: Option<PathBuf>,
    stage: &str,
    error: &std::io::Error,
) -> PersistenceStatus {
    PersistenceStatus {
        directory,
        file: None,
        failure: Some(format!(
            "File logging unavailable: {stage}; {:?}; Windows error {}. Logs are retained in memory; automatic retry is enabled.",
            error.kind(),
            error
                .raw_os_error()
                .map_or_else(|| "unavailable".into(), |code| code.to_string())
        )),
    }
}

fn retry_persistence() {
    let mut writer = persistent().lock().unwrap_or_else(|e| e.into_inner());
    if writer.is_some() {
        return;
    }
    let directory = diagnostic_directory();
    let prepared = match crate::storage_directory("RustDiagnostics") {
        Ok(path) => path,
        Err(error) => {
            let error = error
                .downcast::<std::io::Error>()
                .map(|error| *error)
                .unwrap_or_else(|_| {
                    std::io::Error::other("Diagnostic directory validation failed")
                });
            output::set_persistence_status(failure_status(
                directory,
                "validate / create diagnostic directory",
                &error,
            ));
            return;
        }
    };
    let result = PersistentLog::available(&prepared);
    match result {
        Ok(mut log) => {
            // Replay what was retained while file logging was unavailable. Never
            // call record while holding the writer lock (the sink takes it too).
            let snapshot = output::since(0);
            let replay = snapshot.as_ref().map_or("", |s| s.text.as_str());
            if let Err(error) = log.write(replay) {
                output::set_persistence_status(failure_status(directory, "replay", &error));
                return;
            }
            output::set_persistence_status(PersistenceStatus {
                directory,
                file: Some(log.path.clone()),
                failure: None,
            });
            *writer = Some(log);
        }
        Err(error) => {
            output::set_persistence_status(failure_status(directory, "open / rotate", &error))
        }
    }
}

fn read_previous(directory: &Path) -> Vec<output::PreviousSession> {
    use std::io::{Read, Seek};
    if achimeld_security::windows::reject_reparse(directory).is_err() {
        return Vec::new();
    }
    let mut sessions = Vec::new();
    for slot in 0..=4 {
        let stem = if slot == 0 {
            "log".into()
        } else {
            format!("log-instance-{slot}")
        };
        for suffix in [".txt", "_bak.txt"] {
            let name = format!("{stem}{suffix}");
            let path = directory.join(&name);
            if validate_file(&path).is_err() {
                continue;
            }
            let Ok(mut file) = std::fs::File::open(path) else {
                continue;
            };
            let Ok(metadata) = file.metadata() else {
                continue;
            };
            let offset = metadata.len().saturating_sub(128 * 1024);
            if file.seek(std::io::SeekFrom::Start(offset)).is_err() {
                continue;
            }
            let mut bytes = Vec::new();
            if file.take(128 * 1024).read_to_end(&mut bytes).is_err() {
                continue;
            }
            let mut text = String::from_utf8_lossy(&bytes).into_owned();
            if offset > 0 {
                text.insert_str(
                    0,
                    "[Showing the last 128 KiB; the complete log is in the log folder.]\n",
                );
            }
            sessions.push(output::PreviousSession { name, text });
        }
    }
    sessions
}

pub fn open_folder() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let directory =
        crate::storage_directory("RustDiagnostics").map_err(|_| "Log folder unavailable")?;
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Both terminated strings remain live; the shell is asked to open
    // only the host-derived diagnostic directory, with no executable or arguments.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        Err("Windows could not open the log folder".into())
    } else {
        Ok(())
    }
}

/// Per-user, cross-package desktop ownership. The OS releases it on crash/exit;
/// a leftover filename never means an instance is still running.
pub fn desktop_instance() -> Result<Option<std::fs::File>, String> {
    let directory = crate::storage_directory("RustDiagnostics")
        .map_err(|_| "Cannot access the desktop instance lock")?;
    lock_desktop(&directory).map_err(|_| "Cannot establish desktop instance ownership".into())
}

fn lock_desktop(directory: &Path) -> std::io::Result<Option<std::fs::File>> {
    use std::os::windows::fs::OpenOptionsExt;
    achimeld_security::windows::reject_reparse(directory).map_err(std::io::Error::other)?;
    let path = directory.join("desktop-instance.lock");
    // Metadata access itself can encounter an exclusively held Windows handle.
    let result = validate_file(&path).and_then(|()| {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
    });
    match result {
        Ok(file) => Ok(Some(file)),
        Err(error) if matches!(error.raw_os_error(), Some(32 | 33)) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_ownership_rejects_a_second_launch_and_releases_on_exit() {
        let directory = crate::test_output::root()
            .join(format!("desktop-owner-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = lock_desktop(&directory).unwrap().unwrap();
        assert!(lock_desktop(&directory).unwrap().is_none());
        drop(first);
        let next = lock_desktop(&directory).unwrap().unwrap();
        drop(next);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn initialization_and_recovery_are_verified_in_an_isolated_process() {
        let directory = crate::test_output::root()
            .join(format!("log-recovery-{}", achimeld_application::new_id()));
        let logs = directory.join("Achimeld/RustDiagnostics");
        std::fs::create_dir_all(logs.join("log.txt")).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::isolated_recovery_child",
                "--nocapture",
            ])
            .env("LOCALAPPDATA", &directory)
            .env("ACHIMELD_LOG_RECOVERY_TEST", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let text = std::fs::read_to_string(logs.join("log.txt")).unwrap();
        assert!(text.contains("retained during file failure"));
        assert!(text.contains("written after recovery"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn isolated_recovery_child() {
        if std::env::var("ACHIMELD_LOG_RECOVERY_TEST").as_deref() != Ok("1") {
            return;
        }
        initialize();
        assert!(output::persistence_status().failure.is_some());
        output::record("INFO", "test", "retained during file failure");
        std::fs::remove_dir(diagnostic_directory().unwrap().join("log.txt")).unwrap();
        retry_persistence();
        assert!(output::persistence_status().failure.is_none());
        output::record("INFO", "test", "written after recovery");
        assert!(
            std::fs::read_to_string(output::persistence_status().file.unwrap())
                .unwrap()
                .contains("written after recovery")
        );
    }

    #[test]
    fn automatic_slots_preserve_live_writers_and_previous_sessions() {
        let directory = crate::test_output::root()
            .join(format!("log-slots-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut writers = Vec::new();
        for slot in 0..5 {
            let mut writer = PersistentLog::available(&directory).unwrap();
            writer.write(&format!("session {slot}\n")).unwrap();
            assert_eq!(
                std::fs::read_to_string(&writer.path).unwrap(),
                format!("session {slot}\n")
            );
            writers.push(writer);
        }
        assert!(PersistentLog::available(&directory).is_err());
        assert_eq!(read_previous(&directory).len(), 5);
        drop(writers);
        let mut next = PersistentLog::available(&directory).unwrap();
        next.write("new session\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("log_bak.txt")).unwrap(),
            "session 0\n"
        );
        assert!(
            read_previous(&directory)
                .iter()
                .any(|s| s.name == "log_bak.txt" && s.text == "session 0\n")
        );
        drop(next);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn history_reads_are_bounded_and_non_regular_files_are_rejected() {
        let directory = crate::test_output::root()
            .join(format!("log-history-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("log_bak.txt"), "a".repeat(256 * 1024)).unwrap();
        std::fs::create_dir(directory.join("log.txt")).unwrap();
        assert!(PersistentLog::open(&directory).is_err());
        let previous = read_previous(&directory);
        assert_eq!(previous.len(), 1);
        assert!(previous[0].text.starts_with("[Showing the last 128 KiB"));
        assert!(previous[0].text.len() < 129 * 1024);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn persistent_log_survives_close_rotates_and_excludes_concurrent_writers() {
        let directory = crate::test_output::root()
            .join(format!("persistent-log-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut first = PersistentLog::open(&directory).unwrap();
        first.write("first session\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("log.txt")).unwrap(),
            "first session\n"
        );
        assert!(PersistentLog::open(&directory).is_err());
        let mut concurrent = PersistentLog::open_slot(&directory, 1).unwrap();
        concurrent.write("concurrent session\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("log-instance-1.txt")).unwrap(),
            "concurrent session\n"
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("log.txt")).unwrap(),
            "first session\n"
        );
        drop(concurrent);
        drop(first);
        let mut second = PersistentLog::open(&directory).unwrap();
        second.write("second session\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.join("log_bak.txt")).unwrap(),
            "first session\n"
        );
        second.bytes = 8 * 1024 * 1024;
        second.write("panic record\n").unwrap();
        assert!(
            std::fs::read_to_string(directory.join("log.txt"))
                .unwrap()
                .ends_with("panic record\n")
        );
        drop(second);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
