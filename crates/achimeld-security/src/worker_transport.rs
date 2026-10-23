//! Current-user byte pipe and suspended child in a nested kill-on-close Job.
//! No shell, external browser attachment, job breakaway, or automatic replay.
use crate::{SecurityError, wire};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    ffi::c_void,
    fs::File,
    io::{Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::Path,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{JobObjects::*, Pipes::*, Threading::*},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: Handle owns the successful Windows handle exclusively; null/invalid sentinels are excluded.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub fn validate_name(name: &str) -> Result<(), SecurityError> {
    let suffix = name
        .strip_prefix("Achimeld.Steam.")
        .ok_or(SecurityError("Invalid worker endpoint"))?;
    if suffix.len() != 32 || !suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SecurityError("Invalid worker endpoint"));
    }
    Ok(())
}
pub(super) fn current_user_sid() -> Result<String, SecurityError> {
    let mut token = ptr::null_mut();
    // SAFETY: inspect only the current process token. Aligned storage holds TOKEN_USER.
    // SAFETY: Current-process token output is live; successful result transfers to Handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(SecurityError("Worker pipe identity unavailable"));
    }
    let token = Handle(token);
    let mut buffer = vec![0usize; 4096];
    let mut length = 0;
    // SAFETY: Owned token and aligned initialized TOKEN_USER storage are live; buffer size matches allocated bytes.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * size_of::<usize>()) as u32,
            &mut length,
        )
    } == 0
    {
        return Err(SecurityError("Worker pipe identity unavailable"));
    }
    // SAFETY: Successful token query filled aligned storage including TOKEN_USER; its SID is borrowed while the buffer stays live.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = ptr::null_mut();
    // SAFETY: Borrowed valid token SID and string output are live; successful result is a terminated LocalAlloc string.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
        return Err(SecurityError("Worker pipe identity unavailable"));
    }
    let mut size = 0;
    // SAFETY: SID string is the successfully returned terminated Windows allocation and stays live through this scan.
    while unsafe { *sid.add(size) } != 0 {
        size += 1;
    }
    // SAFETY: Scanned size is within the terminated SID allocation; the borrowed UTF-16 slice is copied before free.
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, size) });
    // SAFETY: Successful SID string allocation is released exactly once with LocalFree.
    unsafe {
        LocalFree(sid.cast());
    }
    Ok(text)
}
pub(super) fn descriptor() -> Result<HLOCAL, SecurityError> {
    let text = current_user_sid()?;
    let sddl = wide(&format!("O:{text}D:P(A;;GA;;;SY)(A;;GA;;;{text})"));
    let mut result = ptr::null_mut();
    // SAFETY: Terminated SDDL and output storage are live; successful descriptor allocation is owned until pipe creation.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut result,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(SecurityError("Restricted worker pipe unavailable"));
    }
    Ok(result)
}

pub struct PrivatePipe {
    file: File,
}
impl PrivatePipe {
    pub fn create(name: &str) -> Result<Self, SecurityError> {
        validate_name(name)?;
        let path = wide(&format!("\\\\.\\pipe\\{name}"));
        let sd = descriptor()?;
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        // SAFETY: live security descriptor/path; exclusively owned non-inheritable local pipe.
        // SAFETY: Live terminated path/security descriptor; successful local first-instance pipe is exclusively owned and non-inheritable.
        let handle = unsafe {
            CreateNamedPipeW(
                path.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65536,
                65536,
                5000,
                &security,
            )
        };
        // SAFETY: Descriptor was allocated by Windows and is released after the synchronous pipe-creation call.
        unsafe {
            LocalFree(sd);
        }
        if handle == INVALID_HANDLE_VALUE {
            return Err(SecurityError("Restricted worker pipe unavailable"));
        }
        Ok(Self {
            // SAFETY: Successful pipe handle transfers exactly once into File ownership.
            file: unsafe { File::from_raw_handle(handle) },
        })
    }
    pub fn accept(
        &mut self,
        expected_pid: u32,
        timeout: Duration,
        check: &mut impl FnMut() -> Result<(), SecurityError>,
    ) -> Result<(), SecurityError> {
        let deadline = Instant::now() + timeout;
        loop {
            check()?;
            let handle = self.file.as_raw_handle();
            // SAFETY: File owns the pipe; synchronous nonblocking connection needs no OVERLAPPED storage.
            let connected = unsafe { ConnectNamedPipe(handle, ptr::null_mut()) } != 0;
            let error = if connected {
                0
            } else {
                // SAFETY: GetLastError reads thread-local error state immediately after failed connection.
                unsafe { GetLastError() }
            };
            if connected || error == ERROR_PIPE_CONNECTED {
                let mut pid = 0;
                // SAFETY: Owned connected pipe and writable PID output remain live during identity query.
                if unsafe { GetNamedPipeClientProcessId(handle, &mut pid) } == 0
                    || pid != expected_pid
                    || pid == 0
                {
                    return Err(SecurityError(
                        "Worker pipe client is not the launched child",
                    ));
                }
                return Ok(());
            }
            if !matches!(error, ERROR_PIPE_LISTENING | ERROR_NO_DATA) {
                return Err(SecurityError("Worker connection failed"));
            }
            if Instant::now() >= deadline {
                return Err(SecurityError("Worker connection timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn exchange<Q: Serialize, R: DeserializeOwned>(
        &mut self,
        request: &Q,
        timeout: Duration,
        check: &mut impl FnMut() -> Result<(), SecurityError>,
    ) -> Result<R, SecurityError> {
        let mut io = CheckedIo {
            file: &mut self.file,
            deadline: Instant::now() + timeout,
            check,
        };
        wire::write_frame(&mut io, request, wire::MAXIMUM_MESSAGE_BYTES)?;
        wire::read_frame(&mut io, wire::MAXIMUM_MESSAGE_BYTES)
    }
}
pub(super) struct CheckedIo<'a, F> {
    pub(super) file: &'a mut File,
    pub(super) deadline: Instant,
    pub(super) check: &'a mut F,
}
impl<F: FnMut() -> Result<(), SecurityError>> CheckedIo<'_, F> {
    fn tick(&mut self) -> std::io::Result<()> {
        // read_exact/write_all retry Interrupted; use a terminal error for cancellation/health loss.
        (self.check)().map_err(|_| std::io::Error::from(std::io::ErrorKind::ConnectionAborted))?;
        if Instant::now() >= self.deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
    fn temporary(error: &std::io::Error) -> bool {
        matches!(error.raw_os_error(), Some(232 | 536))
    }
}
impl<F: FnMut() -> Result<(), SecurityError>> Read for CheckedIo<'_, F> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            self.tick()?;
            match self.file.read(buffer) {
                Ok(0) if !buffer.is_empty() => std::thread::sleep(Duration::from_millis(5)),
                Err(e) if Self::temporary(&e) => std::thread::sleep(Duration::from_millis(5)),
                result => return result,
            }
        }
    }
}
impl<F: FnMut() -> Result<(), SecurityError>> Write for CheckedIo<'_, F> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        loop {
            self.tick()?;
            match self.file.write(buffer) {
                Ok(0) => std::thread::sleep(Duration::from_millis(5)),
                Err(e) if Self::temporary(&e) => std::thread::sleep(Duration::from_millis(5)),
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct WorkerChild {
    process: Handle,
    job: Handle,
    pid: u32,
}
impl WorkerChild {
    pub fn spawn(root: &Path, name: &str) -> Result<Self, SecurityError> {
        validate_name(name)?;
        let executable = root.join("Achimeld.SteamWorker.exe");
        crate::windows::reject_reparse(&executable)?;
        Self::spawn_executable(root, &executable, name)
    }
    fn spawn_executable(root: &Path, executable: &Path, name: &str) -> Result<Self, SecurityError> {
        // SAFETY: Null name/security request an unnamed non-inheritable Job; result is checked before use.
        let job = Handle(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        if job.0.is_null() {
            return Err(SecurityError("Worker lifetime job unavailable"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 256;
        // SAFETY: Owned Job and initialized limits storage remain live with the supplied correct size.
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(SecurityError("Worker lifetime cannot be enforced"));
        }
        let path = crate::windows_arguments::terminated(executable.as_os_str())?;
        let mut command = crate::windows_arguments::command_line(
            executable.as_os_str(),
            &[std::ffi::OsStr::new(name)],
        )?;
        let directory = crate::windows_arguments::terminated(root.as_os_str())?;
        let mut environment: Vec<_> = std::env::vars_os()
            .filter(|(key, _)| {
                matches!(
                    key.to_str().unwrap_or("").to_ascii_uppercase().as_str(),
                    "SYSTEMROOT"
                        | "WINDIR"
                        | "LOCALAPPDATA"
                        | "APPDATA"
                        | "TEMP"
                        | "TMP"
                        | "USERPROFILE"
                        | "HOMEDRIVE"
                        | "HOMEPATH"
                        | "USERNAME"
                        | "USERDOMAIN"
                        | "PROGRAMDATA"
                        | "PROGRAMFILES"
                        | "PROGRAMFILES(X86)"
                        | "PATH"
                        | "PLAYWRIGHT_BROWSERS_PATH"
                        | "ACHIMELD_PACKAGE_ROOT"
                        | "ACHIMELD_SUPERVISION_PIPE"
                )
            })
            .collect();
        environment.sort_by_key(|(key, _)| key.to_ascii_uppercase());
        // Override caller state with the signed GUI-subsystem host. The existing
        // .NET driver otherwise starts Node with CREATE_NO_WINDOW and adds conhost.
        environment.push((
            "PLAYWRIGHT_NODEJS_PATH".into(),
            root.join("Achimeld.NodeHost.exe").into_os_string(),
        ));
        environment.sort_by_key(|(key, _)| key.to_ascii_uppercase());
        let mut block = Vec::new();
        for (key, value) in environment {
            let mut entry = key;
            entry.push("=");
            entry.push(value);
            block.extend(crate::windows_arguments::terminated(&entry)?);
        }
        block.push(0);
        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        // SAFETY: exclusively owned buffers; child starts suspended, cannot inherit handles
        // or break away from the bootstrapper Job, and is assigned before its first instruction.
        // SAFETY: Owned UTF-16/environment buffers remain live; child is suspended and cannot inherit handles or break away before Job assignment.
        if unsafe {
            CreateProcessW(
                path.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                CREATE_SUSPENDED | DETACHED_PROCESS | CREATE_UNICODE_ENVIRONMENT,
                block.as_ptr().cast::<c_void>(),
                directory.as_ptr(),
                &startup,
                &mut info,
            )
        } == 0
        {
            return Err(SecurityError("Verified Steam worker could not start"));
        }
        let process = Handle(info.hProcess);
        let thread = Handle(info.hThread);
        // SAFETY: Owned Job/process handles are live; the child has not resumed.
        if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
            // SAFETY: Owned suspended-process handle stays live for termination and bounded cleanup wait.
            unsafe {
                TerminateProcess(process.0, 125);
                WaitForSingleObject(process.0, 5000);
            }
            return Err(SecurityError(
                "Steam worker could not join its lifetime job",
            ));
        }
        let child = Self {
            process,
            job,
            pid: info.dwProcessId,
        };
        // SAFETY: Owned thread is resumed only after successful lifetime-Job assignment.
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            return Err(SecurityError("Supervised Steam worker could not resume"));
        }
        Ok(child)
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn require_alive(&self) -> Result<(), SecurityError> {
        let mut code = 0;
        // SAFETY: Owned process and exit-code output stay live during the query.
        if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0
            || code != STILL_ACTIVE as u32
        {
            return Err(SecurityError("Steam worker stopped"));
        }
        Ok(())
    }
    pub fn diagnostic_exit_code(&self) -> Option<u32> {
        // SAFETY: The owned worker handle has SYNCHRONIZE and remains live
        // through this bounded wait and exit-code query after transport failure.
        unsafe {
            if WaitForSingleObject(self.process.0, 250) != WAIT_OBJECT_0 {
                return None;
            }
            let mut code = 0;
            (GetExitCodeProcess(self.process.0, &mut code) != 0).then_some(code)
        }
    }
}
impl Drop for WorkerChild {
    fn drop(&mut self) {
        // SAFETY: Owned Job/process handles stay live through tree termination and the bounded cleanup wait.
        unsafe {
            TerminateJobObject(self.job.0, 125);
            WaitForSingleObject(self.process.0, 5000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    fn name() -> String {
        let mut value = [0; 16];
        rand::thread_rng().fill_bytes(&mut value);
        format!("Achimeld.Steam.{}", hex::encode(value))
    }
    #[test]
    fn local_pipe_first_instance_and_peer_identity_are_enforced() {
        let name = name();
        let mut server = PrivatePipe::create(&name).unwrap();
        assert!(PrivatePipe::create(&name).is_err());
        let path = format!("\\\\.\\pipe\\{name}");
        let client = std::thread::spawn(move || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap()
        });
        assert!(
            server
                .accept(
                    // SAFETY: GetCurrentProcessId only queries process identity; this test uses a different PID.
                    unsafe { GetCurrentProcessId() } + 1,
                    Duration::from_secs(1),
                    &mut || Ok(())
                )
                .is_err()
        );
        drop(client.join().unwrap());
    }
    #[test]
    fn cancellation_interrupts_waiting_for_a_worker() {
        let mut server = PrivatePipe::create(&name()).unwrap();
        let started = Instant::now();
        assert!(
            server
                .accept(123, Duration::from_secs(15), &mut || Err(SecurityError(
                    "cancelled"
                )))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    fn fixture() -> (PrivatePipe, WorkerChild) {
        let executable = std::path::PathBuf::from(
            std::env::var_os("ACHIMELD_TEST_WORKER")
                .expect("Build tools/worker-fixture and set ACHIMELD_TEST_WORKER"),
        );
        let name = name();
        let mut pipe = PrivatePipe::create(&name).unwrap();
        let child = WorkerChild::spawn_executable(executable.parent().unwrap(), &executable, &name)
            .unwrap();
        pipe.accept(child.pid(), Duration::from_secs(10), &mut || {
            child.require_alive()
        })
        .unwrap();
        (pipe, child)
    }
    fn request(operation: &str) -> serde_json::Value {
        serde_json::json!({"requestId":"11111111-2222-4333-8444-555555555555","operation":operation,"arguments":null})
    }
    #[test]
    #[ignore = "Requires the built C# pipe fixture; build/Test-Bridge.ps1 runs this gate"]
    fn csharp_fragmented_large_unicode_frame_and_descendant_cleanup() {
        let (mut pipe, child) = fixture();
        let response: serde_json::Value = pipe
            .exchange(&request("large"), Duration::from_secs(10), &mut || {
                child.require_alive()
            })
            .unwrap();
        assert_eq!(response["requestId"], request("large")["requestId"]);
        assert_eq!(
            response["data"]["text"].as_str().unwrap(),
            "Żółw🦀".repeat(40000)
        );
        drop(pipe);
        drop(child);
        let (mut pipe, child) = fixture();
        let response: serde_json::Value = pipe
            .exchange(&request("child"), Duration::from_secs(10), &mut || {
                child.require_alive()
            })
            .unwrap();
        let descendant = response["data"]["childPid"].as_u64().unwrap() as u32;
        // SAFETY: Test PID is used only to obtain a synchronization handle whose successful result is checked.
        let handle = Handle(unsafe { OpenProcess(SYNCHRONIZE, 0, descendant) });
        assert!(!handle.0.is_null());
        drop(pipe);
        drop(child);
        assert_eq!(
            // SAFETY: Checked owned synchronization handle remains live for the test wait.
            unsafe { WaitForSingleObject(handle.0, 5000) },
            WAIT_OBJECT_0
        );
    }
    #[test]
    #[ignore = "Requires the built C# pipe fixture; build/Test-Bridge.ps1 runs this gate"]
    fn csharp_stalled_reply_is_cancelled_and_health_loss_is_terminal() {
        for reason in ["cancelled", "supervisor lost"] {
            let (mut pipe, child) = fixture();
            let started = Instant::now();
            let result: Result<serde_json::Value, _> =
                pipe.exchange(&request("hang"), Duration::from_secs(300), &mut || {
                    child.require_alive()?;
                    if started.elapsed() > Duration::from_millis(100) {
                        Err(SecurityError(reason))
                    } else {
                        Ok(())
                    }
                });
            assert!(result.is_err());
            assert!(started.elapsed() < Duration::from_secs(2));
            // SAFETY: Test child PID is used only to obtain a synchronization handle whose result is checked.
            let handle = Handle(unsafe { OpenProcess(SYNCHRONIZE, 0, child.pid()) });
            assert!(!handle.0.is_null());
            drop(pipe);
            drop(child);
            assert_eq!(
                // SAFETY: Checked owned synchronization handle remains live for the test wait.
                unsafe { WaitForSingleObject(handle.0, 5000) },
                WAIT_OBJECT_0
            );
        }
    }
    #[test]
    #[ignore = "Requires the built C# pipe fixture; build/Test-Bridge.ps1 runs this gate"]
    fn csharp_oversize_reply_is_rejected_before_payload_allocation() {
        let (mut pipe, child) = fixture();
        let result: Result<serde_json::Value, _> =
            pipe.exchange(&request("oversize"), Duration::from_secs(10), &mut || {
                child.require_alive()
            });
        assert!(result.is_err());
    }
    #[test]
    #[ignore = "Requires the built C# pipe fixture; build/Test-Bridge.ps1 runs this gate"]
    fn csharp_owned_worker_exit_status_survives_a_missing_reply() {
        let (mut pipe, child) = fixture();
        assert_eq!(child.diagnostic_exit_code(), None);
        let result: Result<serde_json::Value, _> = pipe.exchange(
            &request("diagnostic-exit"),
            Duration::from_secs(10),
            &mut || child.require_alive(),
        );
        assert!(result.is_err());
        assert_eq!(child.diagnostic_exit_code(), Some(209));
    }
    #[test]
    #[ignore = "Requires the built C# pipe fixture; build/Test-Bridge.ps1 runs this gate"]
    fn csharp_receiving_read_policy_rejects_direct_mutation_envelopes() {
        for operation in [
            "applyOperation",
            "downloadArtwork",
            "readDocument",
            "evaluate",
        ] {
            let (mut pipe, child) = fixture();
            let response: serde_json::Value = pipe
                .exchange(&request(operation), Duration::from_secs(10), &mut || {
                    child.require_alive()
                })
                .unwrap();
            assert_eq!(response["success"], false);
            assert_eq!(response["errorCode"], "steam.read.failed");
        }
    }
}
