//! Local, filtered minidumps on one dedicated thread, never a browser/worker dump.
use std::{
    ffi::c_void,
    fs::File,
    io::{self, Write},
    os::windows::{fs::FileExt, io::AsRawHandle},
    path::Path,
    ptr,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::{Diagnostics::Debug::*, Threading::*},
};

pub(super) const PANIC: u32 = 1;
const NATIVE: u32 = 2;
pub(super) const HANG: u32 = 3;
const MAX_DUMP_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REPORTS: usize = 8;
const TIMEOUT_MS: u32 = 15_000;
static STATE: OnceLock<Arc<State>> = OnceLock::new();
static ERROR: AtomicU32 = AtomicU32::new(0);

struct Event(usize);
impl Event {
    fn new() -> io::Result<Self> {
        // SAFETY: Unnamed auto-reset event, no inherited handle or borrowed pointers.
        let handle = unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) };
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle as usize))
        }
    }
    fn raw(&self) -> HANDLE {
        self.0 as HANDLE
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: This wrapper exclusively owns the event until its final drop.
        unsafe {
            CloseHandle(self.raw());
        }
    }
}
struct State {
    request: Event,
    done: Event,
    busy: AtomicBool,
    timed_out: AtomicBool,
    reason: AtomicU32,
    thread: AtomicU32,
    exception: AtomicUsize,
}

pub(super) fn initialize() {
    let result = (|| -> io::Result<()> {
        let state = Arc::new(State {
            request: Event::new()?,
            done: Event::new()?,
            busy: AtomicBool::new(false),
            timed_out: AtomicBool::new(false),
            reason: AtomicU32::new(0),
            thread: AtomicU32::new(0),
            exception: AtomicUsize::new(0),
        });
        let worker = state.clone();
        std::thread::Builder::new()
            .name("crash-capture".into())
            .spawn(move || {
                loop {
                    // SAFETY: Worker owns an Arc retaining both event handles for its lifetime.
                    if unsafe { WaitForSingleObject(worker.request.raw(), INFINITE) }
                        != WAIT_OBJECT_0
                    {
                        break;
                    }
                    let result = write_report(&worker);
                    ERROR.store(
                        if worker.timed_out.load(Ordering::Acquire) {
                            1460
                        } else {
                            result
                                .err()
                                .map_or(0, |e| e.raw_os_error().unwrap_or(1).unsigned_abs())
                        },
                        Ordering::Release,
                    );
                    // SAFETY: The completion event remains owned by worker and STATE.
                    unsafe {
                        SetEvent(worker.done.raw());
                    }
                }
            })?;
        let _ = STATE.set(state);
        // SAFETY: Process-lifetime callback uses only initialized events and atomics.
        unsafe {
            SetUnhandledExceptionFilter(Some(exception_filter));
        }
        Ok(())
    })();
    if let Err(error) = result {
        ERROR.store(
            error.raw_os_error().unwrap_or(1).unsigned_abs(),
            Ordering::Release,
        );
        super::output::record(
            "ERROR",
            "crash-capture",
            "Could not initialize local crash capture",
        );
    } else {
        super::output::record(
            "INFO",
            "crash-capture",
            "Local filtered crash and hang dump capture initialized",
        );
    }
}

pub(super) fn failure() -> Option<String> {
    let code = ERROR.load(Ordering::Acquire);
    (code != 0).then(|| format!("Local crash dump capture failed; Windows error {code}. Check the runtime diagnostic folder."))
}

pub(super) fn capture(reason: u32) {
    let _ = request(reason, ptr::null());
}

fn request(reason: u32, exception: *const EXCEPTION_POINTERS) -> bool {
    let Some(state) = STATE.get() else {
        return false;
    };
    if state
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    state.reason.store(reason, Ordering::Relaxed);
    // SAFETY: Gets the current thread's numeric identifier without borrowing memory.
    state
        .thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    state.exception.store(exception as usize, Ordering::Release);
    // SAFETY: Events remain live; native exception pointers are retained by the waiting callback.
    let completed = unsafe {
        SetEvent(state.request.raw());
        WaitForSingleObject(state.done.raw(), TIMEOUT_MS) == WAIT_OBJECT_0
    };
    if completed {
        state.busy.store(false, Ordering::Release);
    } else {
        state.timed_out.store(true, Ordering::Release);
        ERROR.store(1460, Ordering::Release);
    }
    completed
}

unsafe extern "system" fn exception_filter(exception: *const EXCEPTION_POINTERS) -> i32 {
    if !request(NATIVE, exception) {
        // SAFETY: End only this failing process. It must not resume/unwind exception storage
        // while the capture thread could still be reading the native exception pointers.
        unsafe {
            TerminateProcess(GetCurrentProcess(), 0xc0000409);
        }
    }
    EXCEPTION_EXECUTE_HANDLER
}

fn write_report(state: &State) -> io::Result<()> {
    let parent =
        super::directory().map_err(|_| io::Error::other("Diagnostic storage unavailable"))?;
    let directory = parent.join("CrashDumps");
    match std::fs::create_dir(&directory) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    achimeld_security::windows::reject_reparse(&directory).map_err(io::Error::other)?;
    // Serialize retention/capture across simultaneous editors without touching their logs.
    use std::os::windows::fs::OpenOptionsExt;
    let lock_path = directory.join("capture.lock");
    super::validate_file(&lock_path)?;
    let _lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(lock_path)?;
    retain_reports(&directory)?;
    let reason = match state.reason.load(Ordering::Acquire) {
        PANIC => "panic",
        NATIVE => "native",
        _ => "hang",
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stem = format!("capture-{stamp}-{}", std::process::id());
    let dump_path = directory.join(format!("{stem}.dmp"));
    let dump = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dump_path)?;
    let exception = state.exception.load(Ordering::Acquire) as *mut EXCEPTION_POINTERS;
    let info = MINIDUMP_EXCEPTION_INFORMATION {
        ThreadId: state.thread.load(Ordering::Acquire),
        ExceptionPointers: exception,
        ClientPointers: 0,
    };
    let callback = MINIDUMP_CALLBACK_INFORMATION {
        CallbackRoutine: Some(write_callback),
        CallbackParam: (&dump as *const File).cast_mut().cast(),
    };
    // SAFETY: Only this dedicated thread calls DbgHelp. The file, callback parameter and
    // exception descriptor remain live synchronously; native faulting thread waits for completion.
    let succeeded = unsafe {
        MiniDumpWriteDump(
            GetCurrentProcess(),
            std::process::id(),
            dump.as_raw_handle(),
            MiniDumpFilterMemory | MiniDumpWithoutOptionalData | MiniDumpWithThreadInfo,
            if exception.is_null() {
                ptr::null()
            } else {
                &info
            },
            ptr::null(),
            &callback,
        )
    } != 0;
    let mut error = if succeeded {
        None
    } else {
        Some(io::Error::last_os_error())
    };
    if let Err(flush_error) = dump.sync_all() {
        error = Some(flush_error);
    }
    drop(dump);
    let succeeded = error.is_none();
    if !succeeded {
        std::fs::remove_file(&dump_path)?;
    }
    let mut metadata = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("{stem}.json")))?;
    let report = serde_json::json!({
        "application": achimeld_ui::window_title(), "pid": std::process::id(), "unixNanoseconds": stamp.to_string(),
        "reason": reason, "threadId": state.thread.load(Ordering::Acquire), "dumpWritten": succeeded,
        "windowsError": error.as_ref().and_then(io::Error::raw_os_error), "filtered": true,
        "sensitiveLocalArtifact": true,
    });
    metadata.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
    metadata.sync_all()?;
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn retain_reports(directory: &Path) -> io::Result<()> {
    let mut stems = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("dmp" | "json")
        ) && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem.strip_prefix("capture-").is_some_and(|tail| {
                tail.split_once('-').is_some_and(|(stamp, pid)| {
                    !stamp.is_empty()
                        && !pid.is_empty()
                        && stamp.bytes().chain(pid.bytes()).all(|b| b.is_ascii_digit())
                })
            })
        {
            super::validate_file(&path)?;
            stems.insert(stem.to_owned());
        }
    }
    let remove = stems.len().saturating_sub(MAX_REPORTS - 1);
    for stem in stems.into_iter().take(remove) {
        for extension in ["dmp", "json"] {
            let path = directory.join(format!("{stem}.{extension}"));
            super::validate_file(&path)?;
            match std::fs::remove_file(path) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

// DbgHelp's custom I/O callback enforces the size limit before writing any bytes.
unsafe extern "system" fn write_callback(
    parameter: *mut c_void,
    input: *const MINIDUMP_CALLBACK_INPUT,
    output: *mut MINIDUMP_CALLBACK_OUTPUT,
) -> i32 {
    if parameter.is_null() || input.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: DbgHelp passes valid callback structures and our live File pointer for
    // this synchronous invocation. Packed fields are copied without creating references.
    unsafe {
        let kind = (*input).CallbackType as i32;
        if kind == IoStartCallback {
            (*output).Anonymous.Status = 1;
        } else if kind == IoFinishCallback {
            (*output).Anonymous.Status = 0;
        } else if kind == IoWriteAllCallback {
            let io = (*input).Anonymous.Io;
            let valid = io
                .Offset
                .checked_add(u64::from(io.BufferBytes))
                .is_some_and(|end| end <= MAX_DUMP_BYTES);
            if !valid || io.Buffer.is_null() {
                (*output).Anonymous.Status = 0x80070070u32 as i32;
                return 1;
            }
            let bytes = std::slice::from_raw_parts(io.Buffer.cast::<u8>(), io.BufferBytes as usize);
            let file = &*parameter.cast::<File>();
            (*output).Anonymous.Status = if file
                .seek_write(bytes, io.Offset)
                .is_ok_and(|count| count == bytes.len())
            {
                0
            } else {
                0x80004005u32 as i32
            };
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn real_process_failures_leave_live_logs_and_valid_bounded_minidumps() {
        for mode in ["panic", "native", "hang"] {
            let root = crate::test_output::root().join(format!(
                "runtime-capture-{}",
                achimeld_application::new_id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "diagnostics::crash::tests::capture_child",
                    "--nocapture",
                ])
                .env("ACHIMELD_CAPTURE_TEST", mode)
                .env("LOCALAPPDATA", &root)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let logs = root.join("Achimeld/Logs");
            let private = root.join("Achimeld/RustDiagnostics");
            let mut saw_live_log = false;
            loop {
                saw_live_log |= std::fs::read_to_string(logs.join("log.txt")).is_ok_and(|s| {
                    s.contains("fixture ready before failure") && s.contains("outer glow composite")
                });
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!(
                        "Capture child timed out: {mode}; evidence: {}",
                        root.display()
                    );
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            assert!(saw_live_log, "Log was not readable while child ran: {mode}");
            let text = std::fs::read_to_string(logs.join("log.txt")).unwrap();
            assert!(text.contains("fixture ready before failure"));
            for expected in [
                "kind=Outline",
                "outline expand mask",
                "kind=OuterLight",
                "outer glow blur",
                "render effect: completed",
            ] {
                assert!(
                    text.contains(expected),
                    "Missing effect evidence after {mode}: {expected}"
                );
            }
            assert!(!text.contains("PRIVATE_PANIC_PAYLOAD"));
            if mode == "panic" {
                assert!(text.contains("Unhandled Rust panic at"));
            }
            if mode == "hang" {
                assert!(text.contains("UI work has not completed"));
            }
            assert!(!logs.join("CrashDumps").exists());
            assert!(!private.join("log.txt").exists());
            let paths: Vec<_> = std::fs::read_dir(private.join("CrashDumps"))
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            let dump = paths
                .iter()
                .find(|p| p.extension().is_some_and(|s| s == "dmp"))
                .expect("A real minidump must exist");
            let bytes = std::fs::read(dump).unwrap();
            assert!(bytes.starts_with(b"MDMP"));
            assert!(bytes.len() as u64 <= MAX_DUMP_BYTES);
            // A real directory and thread stream are present, not just a magic header.
            assert!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()) >= 3);
            let metadata = paths
                .iter()
                .find(|p| p.extension().is_some_and(|s| s == "json"))
                .unwrap();
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(metadata).unwrap()).unwrap();
            assert_eq!(report["reason"], mode);
            assert_eq!(report["dumpWritten"], true);
            assert_eq!(report["application"], achimeld_ui::window_title());
        }
    }

    #[test]
    fn capture_child() {
        let Ok(mode) = std::env::var("ACHIMELD_CAPTURE_TEST") else {
            return;
        };
        assert_ne!(
            super::super::windows_local_app_data().unwrap(),
            std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()),
            "Production AppData resolution must ignore the redirected fixture environment"
        );
        crate::diagnostics::initialize();
        // Exercise the real renderer and host sink before the controlled failure.
        // Only this test executable redirects AppData, never the production app.
        let mut editor = achimeld_application::Editor::default();
        editor
            .execute(
                "Fixture",
                achimeld_application::Command::Add(achimeld_application::EntityKind::Achievement),
                None,
            )
            .unwrap();
        let mut project = editor.document().project().clone();
        let id = project["achievements"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        project["achievements"][0]["icon"] = serde_json::json!({"layers":[{
            "id":achimeld_application::new_id(), "kind":"Shape",
            "shape":{"kind":"Rectangle", "scaleX":0.5, "scaleY":0.5},
            "effects":[
                {"id":achimeld_application::new_id(), "kind":"Outline", "width":16},
                {"id":achimeld_application::new_id(), "kind":"OuterLight", "width":16, "blur":32}
            ]
        }]});
        let document = achimeld_application::Document::from_parts(
            project,
            editor.document().catalog().clone(),
        )
        .unwrap();
        achimeld_imaging::render_achieved_cancellable(
            &document,
            &id,
            Some((64, 64)),
            &mut crate::desktop::Files::new(),
            &|| false,
        )
        .unwrap();
        super::super::output::record("INFO", "test", "fixture ready before failure");
        std::thread::sleep(Duration::from_millis(300));
        match mode.as_str() {
            "panic" => panic!("PRIVATE_PANIC_PAYLOAD"),
            "native" => {
                // SAFETY: Isolated, explicitly selected test child. Deliberately raises a
                // noncontinuable exception with no pointers or external process effects.
                unsafe {
                    RaiseException(0xe0424d01, 1, 0, ptr::null());
                }
            }
            "hang" => {
                let monitor = crate::diagnostics::Monitor::new();
                let frame = monitor.frame();
                frame.phase("synthetic blocked UI");
                std::thread::sleep(Duration::from_secs(7));
                drop(frame);
                drop(monitor);
                assert!(failure().is_none());
            }
            _ => panic!("Unknown fixture mode"),
        }
    }

    #[test]
    fn retention_preserves_unrelated_files_and_callback_refuses_oversized_dump() {
        let root = crate::test_output::root().join(format!(
            "capture-retention-{}",
            achimeld_application::new_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for n in 100..110 {
            for ext in ["dmp", "json"] {
                std::fs::write(root.join(format!("capture-{n}-1.{ext}")), b"fixture").unwrap();
            }
        }
        std::fs::write(root.join("unrelated.dmp"), b"keep").unwrap();
        retain_reports(&root).unwrap();
        assert!(root.join("unrelated.dmp").is_file());
        assert_eq!(
            std::fs::read_dir(&root).unwrap().count(),
            (MAX_REPORTS - 1) * 2 + 1
        );
        let file = File::create(root.join("bounded.dmp")).unwrap();
        let mut byte = 1u8;
        let input = MINIDUMP_CALLBACK_INPUT {
            CallbackType: IoWriteAllCallback as u32,
            Anonymous: MINIDUMP_CALLBACK_INPUT_0 {
                Io: MINIDUMP_IO_CALLBACK {
                    Handle: file.as_raw_handle(),
                    Offset: MAX_DUMP_BYTES,
                    Buffer: (&mut byte as *mut u8).cast(),
                    BufferBytes: 1,
                },
            },
            ..Default::default()
        };
        let mut output = MINIDUMP_CALLBACK_OUTPUT::default();
        // SAFETY: Valid live fixture storage matches the I/O callback type and file parameter.
        unsafe {
            write_callback(
                (&file as *const File).cast_mut().cast(),
                &input,
                &mut output,
            );
            assert!(output.Anonymous.Status < 0);
        }
        assert_eq!(file.metadata().unwrap().len(), 0);
    }
}
