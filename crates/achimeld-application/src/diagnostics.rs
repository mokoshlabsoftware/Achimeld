//! Bounded, process-local application diagnostics. No file or worker authority.
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_LINES: usize = 4_000;
const MAX_MESSAGE: usize = 4_096;

#[derive(Default)]
struct Buffer {
    lines: VecDeque<String>,
    bytes: usize,
    revision: u64,
    discarded: u64,
}

impl Buffer {
    fn clear(&mut self) -> Snapshot {
        self.lines.clear();
        self.bytes = 0;
        self.discarded = 0;
        self.revision += 1;
        self.snapshot()
    }
    fn push(&mut self, line: String) {
        self.bytes += line.len();
        self.lines.push_back(line);
        while self.bytes > MAX_BYTES || self.lines.len() > MAX_LINES {
            if let Some(line) = self.lines.pop_front() {
                self.bytes -= line.len();
                self.discarded += 1;
            }
        }
        self.revision += 1;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            text: self.lines.iter().map(String::as_str).collect(),
            discarded: self.discarded,
        }
    }
}

fn buffer() -> &'static Mutex<Buffer> {
    static BUFFER: OnceLock<Mutex<Buffer>> = OnceLock::new();
    BUFFER.get_or_init(Mutex::default)
}

static SINK: OnceLock<fn(&str)> = OnceLock::new();

/// Host-supplied status. Presentation reads it without acquiring filesystem authority.
#[derive(Clone, Default)]
pub struct PersistenceStatus {
    pub directory: Option<std::path::PathBuf>,
    pub file: Option<std::path::PathBuf>,
    pub failure: Option<String>,
}

fn persistence() -> &'static Mutex<PersistenceStatus> {
    static STATUS: OnceLock<Mutex<PersistenceStatus>> = OnceLock::new();
    STATUS.get_or_init(Mutex::default)
}

pub fn set_persistence_status(status: PersistenceStatus) {
    *persistence().lock().unwrap_or_else(|e| e.into_inner()) = status;
}

pub fn persistence_status() -> PersistenceStatus {
    persistence()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Bounded snapshots of fixed diagnostic files loaded by the host at startup.
pub struct PreviousSession {
    pub name: String,
    pub text: String,
}
static PREVIOUS: OnceLock<Vec<PreviousSession>> = OnceLock::new();

pub fn install_previous_sessions(sessions: Vec<PreviousSession>) {
    let _ = PREVIOUS.set(sessions.into_iter().take(10).collect());
}

pub fn previous_sessions() -> &'static [PreviousSession] {
    PREVIOUS.get().map_or(&[], Vec::as_slice)
}

/// Safe operation breadcrumbs: only static names/outcomes, no document payloads.
pub struct Operation {
    id: u64,
    name: &'static str,
    started: std::time::Instant,
    finished: bool,
}
impl Operation {
    pub fn begin(name: &'static str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        record(
            "INFO",
            "operation",
            &format!(
                "#{id} {name}: started; thread={:?}",
                std::thread::current().id()
            ),
        );
        Self {
            id,
            name,
            started: std::time::Instant::now(),
            finished: false,
        }
    }
    pub fn finish(mut self, outcome: &'static str) {
        self.finished = true;
        record(
            if outcome == "failed" { "ERROR" } else { "INFO" },
            "operation",
            &format!(
                "#{} {}: {outcome}; elapsed={} ms",
                self.id,
                self.name,
                self.started.elapsed().as_millis()
            ),
        );
    }
    /// Only static stages or explicitly selected numeric/allowlisted context.
    /// Never pass document text, paths, serialized recipes or raw error messages.
    pub fn checkpoint(&self, context: &str) {
        record(
            "INFO",
            "operation",
            &format!(
                "#{} {}: {context}; elapsed={} ms",
                self.id,
                self.name,
                self.started.elapsed().as_millis()
            ),
        );
    }
}

/// Return a static label even when a malformed recipe supplies arbitrary text.
pub fn effect_kind(kind: &str) -> &'static str {
    match kind {
        "Outline" => "Outline",
        "OuterLight" => "OuterLight",
        "InnerLight" => "InnerLight",
        "Shadow" => "Shadow",
        "BoxBlur" => "BoxBlur",
        "GradientOverlay" => "GradientOverlay",
        "ColorOverlay" => "ColorOverlay",
        "Skew" => "Skew",
        "Perspective" => "Perspective",
        "Pinch" => "Pinch",
        "LensBlur" => "LensBlur",
        _ => "Unknown",
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        if !self.finished {
            record(
                "ERROR",
                "operation",
                &format!(
                    "#{} {}: interrupted; elapsed={} ms",
                    self.id,
                    self.name,
                    self.started.elapsed().as_millis()
                ),
            );
        }
    }
}

/// The supervised host may retain already bounded diagnostic lines.
pub fn install_sink(sink: fn(&str)) {
    let _ = SINK.set(sink);
}

#[derive(Default)]
pub struct Snapshot {
    pub revision: u64,
    pub text: String,
    pub discarded: u64,
}

/// Copies at most 1 MiB, only when records have changed since the caller's view.
pub fn since(revision: u64) -> Option<Snapshot> {
    let buffer = buffer().lock().unwrap_or_else(|error| error.into_inner());
    (buffer.revision != revision).then(|| buffer.snapshot())
}

/// Clears only this process's in-memory session output. Collection continues.
pub fn clear() -> Snapshot {
    buffer()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear()
}

/// Only pass host-owned status text or static diagnostics, never raw browser
/// errors, request arguments, portal contents, tokens or authentication data.
pub fn record(level: &str, source: &str, message: &str) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = stamp.as_secs() % 86_400;
    let message = bounded_message(message);
    let line = format!(
        "{:02}:{:02}:{:02}.{:03}Z {level:5} [{source}] {message}\n",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        stamp.subsec_millis(),
    );
    if let Some(sink) = SINK.get() {
        sink(&line);
    }
    buffer()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(line);
}

fn bounded_message(message: &str) -> String {
    let mut output = String::new();
    for character in message.chars() {
        let escaped: String = if character.is_control() {
            character.escape_default().collect()
        } else {
            character.to_string()
        };
        if output.len() + escaped.len() > MAX_MESSAGE {
            output.push_str("… [truncated]");
            break;
        }
        output.push_str(&escaped);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_output_advances_revision_and_collection_continues() {
        let mut buffer = Buffer::default();
        buffer.push("old\n".into());
        let before = buffer.revision;
        let empty = buffer.clear();
        assert!(empty.text.is_empty());
        assert!(empty.revision > before);
        assert_eq!(empty.discarded, 0);
        buffer.push("new\n".into());
        assert_eq!(buffer.snapshot().text, "new\n");
    }

    #[test]
    fn diagnostics_bound_memory_and_retain_newest_complete_records() {
        let mut buffer = Buffer::default();
        for index in 0..MAX_LINES + 7 {
            buffer.push(format!("record {index}\n"));
        }
        assert_eq!(buffer.discarded, 7);
        assert!(buffer.snapshot().text.starts_with("record 7\n"));
        for _ in 0..500 {
            buffer.push(format!("{}\n", "ą".repeat(2048)));
        }
        assert!(buffer.bytes <= MAX_BYTES);
        assert!(buffer.lines.len() <= MAX_LINES);
        assert!(buffer.snapshot().text.ends_with('\n'));
    }

    #[test]
    fn diagnostics_escape_control_characters_and_truncate_on_unicode_boundaries() {
        assert_eq!(bounded_message("one\r\ntwo\u{1b}"), "one\\r\\ntwo\\u{1b}");
        let message = bounded_message(&"界".repeat(MAX_MESSAGE));
        assert!(message.ends_with("… [truncated]"));
        assert!(message.len() <= MAX_MESSAGE + "… [truncated]".len());
    }
}
