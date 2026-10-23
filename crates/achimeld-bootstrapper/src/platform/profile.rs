//! Dedicated profile creation, access restrictions, and guarded cleanup.
use super::{
    handles::{descriptor, error},
    identity::user_sid,
};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Security::*;

pub fn profile() -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("local user storage unavailable")?;
    let path = PathBuf::from(&local).join("Achimeld/Browser/ChromeProfile");
    crate::manifest::reject_reparse(&path)?;
    std::fs::create_dir_all(&path).map_err(|_| "cannot create dedicated browser profile")?;
    restrict_profile(&path)?;
    Ok(path)
}
pub fn restrict_profile(path: &Path) -> Result<(), String> {
    crate::manifest::reject_reparse(path)?;
    let sd = descriptor(&format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{})", user_sid()?))?;
    let name = achimeld_security::windows_arguments::terminated(path.as_os_str())
        .map_err(|e| e.to_string())?;
    // SAFETY: Terminated path and owned descriptor stay live during the synchronous DACL update.
    unsafe {
        if SetFileSecurityW(
            name.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            sd.0 as *mut c_void,
        ) == 0
        {
            return Err(error("cannot restrict dedicated profile permissions"));
        }
    }
    Ok(())
}

pub fn clear_profile(path: &Path) -> Result<(), String> {
    // Reject every reparse point before recursively removing only this fixed user profile.
    let expected = profile()?;
    if path != expected {
        return Err("profile boundary rejected".into());
    }
    let mut pending = vec![path.to_owned()];
    while let Some(directory) = pending.pop() {
        crate::manifest::reject_reparse(&directory)?;
        for entry in std::fs::read_dir(&directory).map_err(|_| "profile cannot be inspected")? {
            let entry = entry.map_err(|_| "profile entry unavailable")?;
            crate::manifest::reject_reparse(&entry.path())?;
            if entry
                .file_type()
                .map_err(|_| "profile entry unavailable")?
                .is_dir()
            {
                pending.push(entry.path());
            }
        }
    }
    std::fs::remove_dir_all(path)
        .map_err(|_| "profile is in use; close the Steam browser first")?;
    std::fs::create_dir_all(path).map_err(|_| "profile cannot be recreated")?;
    restrict_profile(path)
}
