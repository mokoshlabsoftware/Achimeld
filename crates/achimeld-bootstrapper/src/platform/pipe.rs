//! Local first-instance supervision pipe with bounded byte framing and receipt.
use super::{
    handles::{Handle, descriptor, error, wide},
    identity::user_sid,
};
use std::{
    ffi::c_void,
    fs::File,
    io::{Read, Write},
    os::windows::io::FromRawHandle,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    Storage::FileSystem::*,
    System::{Pipes::*, Threading::*},
};

pub struct Pipe {
    pub handle: Handle,
}
impl Pipe {
    pub fn create(name: &str) -> Result<Self, String> {
        let name = wide(&format!("\\\\.\\pipe\\{name}"));
        let sd = descriptor(&format!("D:P(A;;GA;;;SY)(A;;GA;;;{})", user_sid()?))?;
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0 as *mut c_void,
            bInheritHandle: 0,
        };
        // SAFETY: Terminated name, owned descriptor and security attributes stay live; successful non-inheritable pipe transfers to Handle.
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65536,
                65536,
                5000,
                &security,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(error("cannot establish private supervision endpoint"));
        }
        Ok(Self {
            handle: Handle(handle as usize),
        })
    }
    pub fn client(&self) -> Result<Option<u32>, String> {
        // SAFETY: Pipe handle and PID output are live; synchronous nonblocking connection has no OVERLAPPED storage.
        unsafe {
            let result = ConnectNamedPipe(self.handle.raw(), ptr::null_mut());
            // PIPE_NOWAIT success means the instance became available, not that
            // a peer connected. Only ERROR_PIPE_CONNECTED permits a PID query.
            // https://learn.microsoft.com/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe
            if result != 0 {
                return Ok(None);
            }
            match GetLastError() {
                ERROR_PIPE_CONNECTED => {}
                ERROR_PIPE_LISTENING => return Ok(None),
                ERROR_NO_DATA => {
                    self.disconnect();
                    return Ok(None);
                }
                _ => return Err(error("supervision pipe connection unavailable")),
            }
            let mut pid = 0;
            if GetNamedPipeClientProcessId(self.handle.raw(), &mut pid) == 0 {
                let code = GetLastError();
                let failure = format!("supervision peer identity unavailable (Windows {code})");
                self.disconnect();
                if matches!(
                    code,
                    ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED | ERROR_BROKEN_PIPE
                ) {
                    return Ok(None);
                }
                return Err(failure);
            }
            Ok(Some(pid))
        }
    }
    pub fn exchange(
        &self,
        respond: impl FnOnce(serde_json::Value) -> Result<serde_json::Value, String>,
    ) -> Result<(), String> {
        // Duplicate the handle for File ownership without closing the reserved pipe instance.
        let mut duplicate = ptr::null_mut();
        // SAFETY: Borrowed source handle stays live; successful non-inheritable duplicate has a distinct owner.
        unsafe {
            if DuplicateHandle(
                GetCurrentProcess(),
                self.handle.raw(),
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            ) == 0
            {
                return Err(error("pipe handle unavailable"));
            }
        }
        // SAFETY: Successful duplicated handle transfers exactly once into File, leaving the reserved endpoint owned separately.
        let mut file = unsafe { File::from_raw_handle(duplicate) };
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut header = [0u8; 4];
        read_deadline(&mut file, &mut header, deadline)?;
        let length = u32::from_le_bytes(header) as usize;
        if length == 0 || length > 65536 {
            return Err("IPC request limit exceeded".into());
        }
        let mut bytes = vec![0; length];
        read_deadline(&mut file, &mut bytes, deadline)?;
        let request = serde_json::from_slice(&bytes).map_err(|_| "invalid IPC request")?;
        let response =
            serde_json::to_vec(&respond(request)?).map_err(|_| "invalid IPC response")?;
        if response.len() > 65536 {
            return Err("IPC response limit exceeded".into());
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        write_deadline(&mut file, &(response.len() as u32).to_le_bytes(), deadline)?;
        write_deadline(&mut file, &response, deadline)?;
        // DisconnectNamedPipe discards unread output. Wait for a bounded receipt
        // acknowledgement rather than blocking indefinitely in FlushFileBuffers.
        let mut receipt = [0; 4];
        read_deadline(
            &mut file,
            &mut receipt,
            Instant::now() + Duration::from_secs(3),
        )?;
        if receipt != *b"ACK1" {
            return Err("invalid IPC receipt".into());
        }
        Ok(())
    }
    pub fn disconnect(&self) {
        // SAFETY: Pipe handle remains owned while disconnecting; outstanding writes were bounded by receipt handling.
        unsafe {
            DisconnectNamedPipe(self.handle.raw());
        }
    }
}
fn write_deadline(file: &mut File, mut bytes: &[u8], deadline: Instant) -> Result<(), String> {
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err("IPC client timeout".into());
        }
        match file.write(bytes) {
            // PIPE_NOWAIT can successfully write zero bytes when the client's
            // receive buffer is full. Wait for space without losing partial writes.
            Ok(0) => std::thread::sleep(Duration::from_millis(5)),
            Ok(count) => {
                bytes = &bytes[count..];
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::Interrupted =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(_) => return Err("IPC client disconnected".into()),
        }
    }
    Ok(())
}
fn read_deadline(file: &mut File, mut bytes: &mut [u8], deadline: Instant) -> Result<(), String> {
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err("IPC client timeout".into());
        }
        match file.read(bytes) {
            // PIPE_NOWAIT returns zero bytes while a connected client is still
            // preparing its request. Broken clients return a pipe error instead.
            Ok(0) => std::thread::sleep(Duration::from_millis(5)),
            Ok(count) => {
                bytes = &mut bytes[count..];
            }
            Err(error)
                if error.raw_os_error() == Some(232)
                    || error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(_) => return Err("IPC transport failed".into()),
        }
    }
    Ok(())
}
