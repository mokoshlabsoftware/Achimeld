//! Windows-only transport for the retained bootstrapper. No ambient browser access.
use crate::{
    SecurityError,
    supervision::{Operation, Request, Status, validate_continuity, validate_response},
    wire,
};
use rand::RngCore;
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::{fs::MetadataExt, io::FromRawHandle},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
    System::{
        Pipes::{GetNamedPipeServerProcessId, PIPE_NOWAIT, SetNamedPipeHandleState},
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
};

pub fn same_path(a: &Path, b: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    if !a.is_absolute() || !b.is_absolute() {
        return false;
    }
    let mut a = a.components();
    let mut b = b.components();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(a), Some(b))
                if a.as_os_str()
                    .encode_wide()
                    .map(ascii_fold)
                    .eq(b.as_os_str().encode_wide().map(ascii_fold)) => {}
            _ => return false,
        }
    }
}
fn ascii_fold(unit: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&unit) {
        unit + 32
    } else {
        unit
    }
}

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(Some(0)).collect()
}

struct DeadlinePipe {
    file: File,
    deadline: Instant,
}
impl DeadlinePipe {
    fn retry(&self, error: &std::io::Error) -> bool {
        if matches!(error.raw_os_error(), Some(232 | 536)) && Instant::now() < self.deadline {
            std::thread::sleep(Duration::from_millis(5));
            true
        } else {
            false
        }
    }
}
impl Read for DeadlinePipe {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if Instant::now() >= self.deadline {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            match self.file.read(buffer) {
                Ok(0) if !buffer.is_empty() => std::thread::sleep(Duration::from_millis(5)),
                Err(e) if self.retry(&e) => continue,
                result => return result,
            }
        }
    }
}
impl Write for DeadlinePipe {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        loop {
            if Instant::now() >= self.deadline {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            match self.file.write(buffer) {
                Ok(0) => std::thread::sleep(Duration::from_millis(5)),
                Err(e) if self.retry(&e) => continue,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn exchange(
    name: &str,
    root: &Path,
    operation: Operation,
    token: Option<&str>,
) -> Result<Status, SecurityError> {
    super::supervision::validate_pipe_name(name)?;
    let path = wide(std::ffi::OsStr::new(&format!("\\\\.\\pipe\\{name}")));
    let deadline = Instant::now() + Duration::from_secs(5);
    let handle = loop {
        // SAFETY: terminated UTF-16 path, no borrowed security structure or template handle.
        // SAFETY: Terminated UTF-16 pipe path stays live; null optional security/template arguments are permitted.
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                0xC0000000,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            break handle;
        }
        let error = std::io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(2 | 231)) || Instant::now() >= deadline {
            return Err(SecurityError("Supervisor connection unavailable"));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // SAFETY: the successfully opened handle transfers exactly once to File's ownership.
    // SAFETY: Successful opened handle transfers exactly once into File ownership.
    let file = unsafe { File::from_raw_handle(handle) };
    let mut pid = 0;
    let mode = PIPE_NOWAIT;
    // SAFETY: handle is owned by file; output pointers remain live for both calls.
    // SAFETY: Owned pipe handle and PID output are live for the synchronous query.
    if unsafe { GetNamedPipeServerProcessId(handle, &mut pid) } == 0
        // SAFETY: Owned pipe and mode storage are live; null optional count/timeout arguments are permitted.
        || unsafe { SetNamedPipeHandleState(handle, &mode, std::ptr::null(), std::ptr::null()) }
            == 0
    {
        return Err(SecurityError("Supervisor identity unavailable"));
    }
    let expected = root
        .join("Achimeld.exe")
        .canonicalize()
        .map_err(|_| SecurityError("Supervisor executable unavailable"))?;
    let actual = process_image(pid)?
        .canonicalize()
        .map_err(|_| SecurityError("Supervisor executable unavailable"))?;
    if !same_path(&expected, &actual) {
        return Err(SecurityError("Unexpected supervision server"));
    }
    let mut random = [0; 32];
    rand::thread_rng().fill_bytes(&mut random);
    let nonce = hex::encode_upper(random);
    let request = Request {
        operation,
        nonce: nonce.clone(),
        probe_token: token.map(str::to_owned),
        failure_code: None,
    };
    let mut pipe = DeadlinePipe { file, deadline };
    wire::write_frame(&mut pipe, &request, 65536)?;
    let status: Status = wire::read_frame(&mut pipe, 65536)?;
    validate_response(
        &status,
        &nonce,
        pid,
        name.strip_prefix("Achimeld.Security."),
    )?;
    pipe.write_all(b"ACK1")
        .map_err(|_| SecurityError("Supervision acknowledgement failed"))?;
    Ok(status)
}

fn process_image(pid: u32) -> Result<PathBuf, SecurityError> {
    // SAFETY: opens only an inspection handle for the authenticated pipe's reported PID.
    // SAFETY: PID is used only for an inspection handle; successful result is checked before use.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return Err(SecurityError("Supervisor identity unavailable"));
    }
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    // SAFETY: writable buffer is length UTF-16 units; handle is valid and closed below.
    // SAFETY: Writable UTF-16 buffer is length units long; successful process handle remains live during the query.
    let result = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
    // SAFETY: The successfully opened inspection handle is closed once after its query, including failure paths.
    unsafe {
        CloseHandle(handle);
    }
    if result == 0 {
        return Err(SecurityError("Supervisor identity unavailable"));
    }
    use std::os::windows::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length as usize],
    )))
}

pub fn reject_reparse(path: &Path) -> Result<(), SecurityError> {
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)
            .map_err(|_| SecurityError("Security path unavailable"))?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(SecurityError(
                "Security paths cannot contain reparse points",
            ));
        }
    }
    Ok(())
}
pub fn validate_paths(root: &Path, status: &Status) -> Result<(), SecurityError> {
    reject_reparse(&status.browser_executable)?;
    reject_reparse(&status.profile_directory)?;
    let browser = status
        .browser_executable
        .canonicalize()
        .map_err(|_| SecurityError("Browser identity unavailable"))?;
    let browser_root = root
        .join("browser")
        .canonicalize()
        .map_err(|_| SecurityError("Browser package unavailable"))?;
    if !browser.starts_with(&browser_root) || browser == browser_root || !browser.is_file() {
        return Err(SecurityError("Browser outside verified package"));
    }
    let local = PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or(SecurityError("Dedicated profile unavailable"))?,
    );
    if !dedicated_profile(&local, &status.profile_directory) {
        return Err(SecurityError("Unexpected browser profile"));
    }
    Ok(())
}

fn dedicated_profile(local: &Path, candidate: &Path) -> bool {
    same_path(&local.join("Achimeld/Browser/ChromeProfile"), candidate)
}

pub fn check_network_denial(pipe: &str, root: &Path, status: &Status) -> Result<(), SecurityError> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, UdpSocket};
    fn access_denied(e: &std::io::Error) -> bool {
        e.raw_os_error() == Some(10013)
    }
    for address in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        match TcpStream::connect_timeout(&SocketAddr::new(address, 42762), Duration::from_secs(3)) {
            Err(e) if access_denied(&e) => {}
            _ => return Err(SecurityError("TCP denial probe did not prove enforcement")),
        }
        let any = if address.is_ipv6() {
            IpAddr::V6(Ipv6Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        };
        let socket = UdpSocket::bind(SocketAddr::new(any, 0))
            .map_err(|_| SecurityError("UDP denial probe unavailable"))?;
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|_| SecurityError("UDP denial probe unavailable"))?;
        match socket.connect(SocketAddr::new(address, status.probe_port)) {
            Err(e) if access_denied(&e) => continue,
            Err(_) => return Err(SecurityError("UDP denial probe unavailable")),
            Ok(()) => {}
        }
        let mut token = [0; 32];
        rand::thread_rng().fill_bytes(&mut token);
        match socket.send(&token) {
            Err(e) if access_denied(&e) => continue,
            Ok(32) => {}
            _ => return Err(SecurityError("UDP denial probe unavailable")),
        }
        let next = exchange(
            pipe,
            root,
            if address.is_ipv6() {
                Operation::UdpProbe6
            } else {
                Operation::UdpProbe4
            },
            Some(&hex::encode_upper(token)),
        )?;
        validate_response(
            &next,
            &next.nonce,
            status.supervisor_pid,
            Some(&status.session_id),
        )?;
        validate_continuity(status, &next)?;
        if next.udp_probe_denied != Some(true) {
            return Err(SecurityError("UDP denial probe did not prove enforcement"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_current_dedicated_profile_is_allowed() {
        let local = Path::new("C:/Users/test/AppData/Local");
        assert!(dedicated_profile(
            local,
            &local.join("Achimeld/Browser/ChromeProfile")
        ));
        for relative in [
            "Google/Chrome/User Data",
            "OtherProduct/Browser/ChromeProfile",
            "Achimeld/Browser/ChromeProfile/Default",
        ] {
            assert!(!dedicated_profile(local, &local.join(relative)));
        }
    }

    #[test]
    fn path_identity_conforms_to_shared_vectors_and_preserves_invalid_utf16() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/security/path-identity.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                same_path(
                    Path::new(case["a"].as_str().unwrap()),
                    Path::new(case["b"].as_str().unwrap())
                ),
                case["equal"].as_bool().unwrap()
            );
        }
        let prefix: Vec<_> = "C:/Users/".encode_utf16().collect();
        let path = |unit| {
            let mut units = prefix.clone();
            units.push(unit);
            units.extend("/Achimeld.exe".encode_utf16());
            OsString::from_wide(&units)
        };
        assert!(!same_path(
            Path::new(&path(0xd800)),
            Path::new(&path(0xd801))
        ));
        assert!(!same_path(
            Path::new(&path(0xd800)),
            Path::new(&path(0xfffd))
        ));
    }
}
