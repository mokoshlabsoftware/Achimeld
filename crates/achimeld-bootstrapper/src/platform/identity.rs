//! OS-derived process/token identity. Paths are compared as complete components.
use super::handles::{Handle, error};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::Threading::*,
};

/// Compare complete Windows paths with either separator spelling, never prefixes.
/// Manifest verification separately rejects traversal/reparse paths and locks files.
pub fn same_image(a: &Path, b: &Path) -> bool {
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

pub fn elevated() -> Result<bool, String> {
    let mut token = ptr::null_mut();
    // SAFETY: Current-process token output is live; a successful inspection handle transfers to Handle.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(error("cannot inspect launch identity"));
        }
    }
    let token = Handle(token as usize);
    let mut value = TOKEN_ELEVATION::default();
    let mut length = 0;
    // SAFETY: Owned token, aligned TOKEN_ELEVATION output, size and returned-length storage remain live.
    unsafe {
        if GetTokenInformation(
            token.raw(),
            TokenElevation,
            &mut value as *mut _ as *mut c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        ) == 0
        {
            return Err(error("cannot inspect elevation"));
        }
    }
    Ok(value.TokenIsElevated != 0)
}
pub fn user_sid() -> Result<String, String> {
    let mut token = ptr::null_mut();
    // SAFETY: Current-process token output is live; a successful inspection handle transfers to Handle.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(error("cannot inspect user"));
        }
    }
    let token = Handle(token as usize);
    let mut buffer = vec![0usize; 128];
    let mut length = 0;
    // SAFETY: Aligned initialized storage holds TOKEN_USER and SID; successful encoding gives a terminated LocalAlloc string freed after copying.
    unsafe {
        if GetTokenInformation(
            token.raw(),
            TokenUser,
            buffer.as_mut_ptr() as *mut c_void,
            (buffer.len() * size_of::<usize>()) as u32,
            &mut length,
        ) == 0
        {
            return Err(error("cannot inspect user SID"));
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(error("cannot encode user SID"));
        }
        let mut size = 0;
        while *text.add(size) != 0 {
            size += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, size));
        LocalFree(text as HLOCAL);
        Ok(result)
    }
}
pub fn image(pid: u32) -> Result<PathBuf, String> {
    // SAFETY: PID is only used for an inspection handle; the result is checked and owned.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err("process identity unavailable".into());
    }
    let process = Handle(raw as usize);
    query_image(&process)
}

/// An inspected process object, retaining its immutable executable identity until exit.
pub struct MonitoredProcess {
    process: Handle,
    image: PathBuf,
}

impl MonitoredProcess {
    pub fn image(&self) -> &Path {
        &self.image
    }

    pub fn has_exited(&self) -> Result<bool, String> {
        process_exited(&self.process)
    }
}

/// Inspect a Job member once; only a positively signaled process may lack an image.
pub fn monitored_image(pid: u32) -> Result<Option<MonitoredProcess>, String> {
    // SAFETY: PID requests inspection/synchronization rights only; a successful handle is owned.
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if raw.is_null() {
        return Err("process identity unavailable".into());
    }
    let process = Handle(raw as usize);
    Ok(image_if_live(&process)?.map(|image| MonitoredProcess { process, image }))
}

pub(super) fn image_if_live(process: &Handle) -> Result<Option<PathBuf>, String> {
    if process_exited(process)? {
        return Ok(None);
    }
    let started = std::time::Instant::now();
    loop {
        match query_image(process) {
            Ok(path) => return Ok(Some(path)),
            Err(error) => {
                if process_exited_within(process, 10)? {
                    return Ok(None);
                }
                if started.elapsed() >= std::time::Duration::from_millis(500) {
                    return Err(error);
                }
            }
        }
    }
}

fn process_exited(process: &Handle) -> Result<bool, String> {
    process_exited_within(process, 0)
}

fn process_exited_within(process: &Handle, milliseconds: u32) -> Result<bool, String> {
    // SAFETY: The owned process handle has SYNCHRONIZE and stays live during a bounded wait.
    // Windows can deny image inspection during teardown before signaling the
    // process object. Only a signaled owned handle proves departure; the
    // bounded grace period never authorizes a live or unclassifiable process.
    match unsafe { WaitForSingleObject(process.raw(), milliseconds) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err("process liveness unavailable".into()),
    }
}

fn query_image(process: &Handle) -> Result<PathBuf, String> {
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    // SAFETY: Owned process and writable UTF-16 buffer are live; length starts at the buffer capacity.
    unsafe {
        if QueryFullProcessImageNameW(process.raw(), 0, buffer.as_mut_ptr(), &mut length) == 0 {
            return Err(error("process path unavailable"));
        }
    }
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..length as usize],
    )))
}

#[cfg(test)]
mod monitor_tests {
    use super::*;

    #[test]
    fn live_process_without_query_rights_is_not_treated_as_exited() {
        // SAFETY: Current PID is live; a successful inspection handle transfers once to Handle.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, GetCurrentProcessId()) };
        assert!(!raw.is_null());
        assert!(image_if_live(&Handle(raw as usize)).is_err());
        assert!(monitored_image(std::process::id()).unwrap().is_some());
    }

    #[test]
    fn inspected_process_retains_identity_when_later_image_queries_are_denied() {
        let mut inspected = monitored_image(std::process::id()).unwrap().unwrap();
        let expected = inspected.image().to_owned();
        // Simulate query denial after a successful inspection of this same live
        // process object. Production construction always requires the first query.
        // SAFETY: Current PID remains live; the successful synchronization handle is owned once.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, GetCurrentProcessId()) };
        assert!(!raw.is_null());
        inspected.process = Handle(raw as usize);
        assert!(query_image(&inspected.process).is_err());
        assert!(!inspected.has_exited().unwrap());
        assert_eq!(inspected.image(), expected);
    }
}
