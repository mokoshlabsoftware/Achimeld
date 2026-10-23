//! Host adapter for safe application and worker diagnostics.
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
    initialize_persistence();
    static LOGGER: Logger = Logger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

struct PersistentLog {
    file: std::fs::File,
    bytes: usize,
    // Excludes concurrent instances from rotating an active session.
    _lock: std::fs::File,
}

impl PersistentLog {
    fn open(directory: &std::path::Path) -> std::io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let current = directory.join("log.txt");
        let backup = directory.join("log_bak.txt");
        let lock = directory.join("log.lock");
        for path in [&current, &backup, &lock] {
            if path.exists() {
                achimeld_security::windows::reject_reparse(path).map_err(std::io::Error::other)?;
            }
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
            .open(current)?;
        Ok(Self {
            file,
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
            self.file
                .write_all(b"Earlier records discarded at the 8 MiB session limit.\n")?;
            self.bytes = 64;
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
    if let Some(log) = log.as_mut()
        && log.write(line).is_err()
    {
        eprintln!("Achimeld could not write its diagnostic log");
    }
}

fn initialize_persistence() {
    let result = crate::storage_directory("RustDiagnostics")
        .map_err(|error| std::io::Error::other(error.to_string()))
        .and_then(|directory| PersistentLog::open(&directory));
    match result {
        Ok(log) => {
            *persistent()
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(log);
            achimeld_application::diagnostics::install_sink(persist);
        }
        Err(_) => eprintln!("Achimeld could not initialize its diagnostic log"),
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

#[cfg(test)]
mod tests {
    use super::*;

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
