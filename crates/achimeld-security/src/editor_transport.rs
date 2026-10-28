//! Local editor automation. Separate from the authenticated Steam worker protocol.
use crate::{
    SecurityError, wire,
    worker_transport::{CheckedIo, current_user_sid, descriptor},
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::*,
    System::Pipes::*,
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
};

pub fn name(pid: u32) -> Result<String, SecurityError> {
    if pid == 0 {
        return Err(SecurityError("Invalid editor process"));
    }
    let hash = hex::encode_upper(Sha256::digest(current_user_sid()?.as_bytes()));
    Ok(format!("Achimeld.RustEditor.{}.{pid}", &hash[..16]))
}

pub struct Pipe {
    file: File,
}
impl Pipe {
    pub fn connected(&self) -> bool {
        // SAFETY: File owns this pipe; a peek with no output pointers only checks connection state.
        unsafe {
            PeekNamedPipe(
                self.file.as_raw_handle(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ) != 0
        }
    }
    pub fn create() -> Result<Self, SecurityError> {
        let path: Vec<u16> = format!("\\\\.\\pipe\\{}", name(std::process::id())?)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let sd = descriptor()?;
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        // SAFETY: Terminated path and allocated descriptor remain live for pipe creation; the handle is non-inheritable.
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
        // SAFETY: Windows allocated this descriptor and pipe creation has finished using it.
        unsafe {
            LocalFree(sd);
        }
        Self::from_handle(handle)
    }
    fn from_handle(handle: HANDLE) -> Result<Self, SecurityError> {
        if handle == INVALID_HANDLE_VALUE {
            return Err(SecurityError("Editor pipe unavailable"));
        }
        Ok(Self {
            // SAFETY: The successfully opened handle transfers exactly once to File ownership.
            file: unsafe { File::from_raw_handle(handle) },
        })
    }
    pub fn connect(pid: u32) -> Result<Self, SecurityError> {
        let path: Vec<u16> = format!("\\\\.\\pipe\\{}", name(pid)?)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let deadline = Instant::now() + Duration::from_secs(3);
        let pipe = loop {
            // SAFETY: Terminated path is live; no security/template pointers are supplied and handles cannot be inherited.
            let handle = unsafe {
                CreateFileW(
                    path.as_ptr(),
                    0xC0000000,
                    0,
                    ptr::null(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                break Self::from_handle(handle)?;
            }
            let error = std::io::Error::last_os_error();
            if !matches!(error.raw_os_error(), Some(2 | 231)) || Instant::now() >= deadline {
                return Err(SecurityError(
                    "Cannot connect to editor; call list_instances again",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut actual = 0;
        let mode = PIPE_NOWAIT;
        let handle = pipe.file.as_raw_handle();
        // SAFETY: File owns the connected pipe and output storage remains live during both queries.
        if unsafe { GetNamedPipeServerProcessId(handle, &mut actual) } == 0 || actual != pid {
            return Err(SecurityError("Editor endpoint belongs to another process"));
        }
        // SAFETY: Connected pipe and mode storage remain live; optional arguments are null.
        if unsafe { SetNamedPipeHandleState(handle, &mode, ptr::null(), ptr::null()) } == 0 {
            return Err(SecurityError("Editor pipe mode unavailable"));
        }
        Ok(pipe)
    }
    pub fn accept(
        &mut self,
        check: &mut impl FnMut() -> Result<(), SecurityError>,
    ) -> Result<(), SecurityError> {
        loop {
            check()?;
            // SAFETY: File owns this nonblocking pipe; no asynchronous OVERLAPPED is required.
            let connected =
                unsafe { ConnectNamedPipe(self.file.as_raw_handle(), ptr::null_mut()) } != 0;
            let error = if connected {
                0
            } else {
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32
            };
            if error == ERROR_PIPE_CONNECTED {
                return Ok(());
            }
            if error == ERROR_NO_DATA {
                return Err(SecurityError(
                    "Editor client disconnected before acceptance",
                ));
            }
            if !connected && !matches!(error, ERROR_PIPE_LISTENING | ERROR_NO_DATA) {
                return Err(SecurityError("Editor connection failed"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn read<T: DeserializeOwned>(
        &mut self,
        timeout: Duration,
        check: &mut impl FnMut() -> Result<(), SecurityError>,
    ) -> Result<T, SecurityError> {
        wire::read_frame(
            &mut CheckedIo {
                file: &mut self.file,
                deadline: Instant::now() + timeout,
                check,
            },
            16 * 1024 * 1024,
        )
    }
    pub fn write<T: Serialize>(
        &mut self,
        value: &T,
        timeout: Duration,
        check: &mut impl FnMut() -> Result<(), SecurityError>,
    ) -> Result<(), SecurityError> {
        wire::write_frame(
            &mut CheckedIo {
                file: &mut self.file,
                deadline: Instant::now() + timeout,
                check,
            },
            value,
            16 * 1024 * 1024,
        )
    }
}

pub fn instances() -> Result<serde_json::Value, SecurityError> {
    let prefix = name(1)?.trim_end_matches('1').to_owned();
    let mut result = Vec::new();
    for entry in std::fs::read_dir(r"\\.\pipe\")
        .map_err(|_| SecurityError("Editor discovery unavailable"))?
        .flatten()
    {
        if let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.strip_prefix(&prefix))
            .and_then(|s| s.parse::<u32>().ok())
        {
            result.push(serde_json::json!({"processId":pid,"title":"Achimeld"}));
        }
    }
    Ok(serde_json::json!(result))
}

/// Creation time of a process this account can inspect. Used for editor uptime only.
pub fn process_uptime(pid: u32) -> Option<Duration> {
    use std::time::{SystemTime, UNIX_EPOCH};
    if pid == 0 {
        return None;
    }
    // SAFETY: Opens a query-only handle for one discovered editor PID. The handle is closed
    // on every path below, including when that process has already exited.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    // SAFETY: `handle` is a live process handle, and the four FILETIME outputs stay live for the call.
    let ok = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    // SAFETY: The query handle is closed exactly once.
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 {
        return None;
    }
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    // FILETIME counts 100 ns ticks since 1601-01-01. Unix epoch is 11,644,473,600 seconds later.
    const WINDOWS_TO_UNIX_100NS: u64 = 11_644_473_600 * 10_000_000;
    let unix_ticks = ticks.checked_sub(WINDOWS_TO_UNIX_100NS)?;
    let started = UNIX_EPOCH + Duration::from_secs(unix_ticks / 10_000_000);
    SystemTime::now().duration_since(started).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_uptime_matches_this_process() {
        let uptime = process_uptime(std::process::id()).expect("current process uptime");
        assert!(uptime.as_secs() < 7 * 24 * 60 * 60);
    }
}
