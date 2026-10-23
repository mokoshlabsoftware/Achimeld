//! Effective installation access checks under the actual application token.
use super::handles::{Descriptor, Handle, error};
use std::{ffi::c_void, path::Path, ptr};
use windows_sys::Win32::{
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Threading::*,
};

pub fn require_protected_package(root: &Path) -> Result<(), String> {
    let mut token = ptr::null_mut();
    // SAFETY: Current token output is live; successful query/duplicate handle transfers to Handle.
    unsafe {
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut token,
        ) == 0
        {
            return Err(error("cannot inspect installation permissions"));
        }
    }
    let token = Handle(token as usize);
    let mut impersonation = ptr::null_mut();
    // SAFETY: Owned token and output storage are live; the successful impersonation token is independently owned.
    unsafe {
        if DuplicateToken(token.raw(), SecurityImpersonation, &mut impersonation) == 0 {
            return Err(error("cannot inspect installation permissions"));
        }
    }
    let impersonation = Handle(impersonation as usize);
    let mut paths = vec![root.to_owned()];
    while let Some(path) = paths.pop() {
        let name = achimeld_security::windows_arguments::terminated(path.as_os_str())
            .map_err(|e| e.to_string())?;
        let mut sd = ptr::null_mut();
        // SAFETY: Terminated path and LocalAlloc descriptor owner are live; AccessCheck uses aligned privilege storage and a valid token/mapping.
        unsafe {
            let code = GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut sd,
            );
            if code != 0 {
                return Err("installation permissions unavailable".into());
            }
            let descriptor = Descriptor(sd as usize);
            let mapping = GENERIC_MAPPING {
                GenericRead: FILE_GENERIC_READ,
                GenericWrite: FILE_GENERIC_WRITE,
                GenericExecute: FILE_GENERIC_EXECUTE,
                GenericAll: FILE_ALL_ACCESS,
            };
            let mut privileges = vec![0usize; 256];
            let mut length = (privileges.len() * size_of::<usize>()) as u32;
            let mut granted = 0;
            let mut allowed = 0;
            if AccessCheck(
                descriptor.0 as *mut c_void,
                impersonation.raw(),
                0x02000000, /* MAXIMUM_ALLOWED */
                &mapping,
                privileges.as_mut_ptr() as *mut PRIVILEGE_SET,
                &mut length,
                &mut granted,
                &mut allowed,
            ) == 0
            {
                return Err(error("installation access check unavailable"));
            }
            if granted
                & (FILE_WRITE_DATA
                    | FILE_APPEND_DATA
                    | FILE_WRITE_EA
                    | FILE_WRITE_ATTRIBUTES
                    | DELETE
                    | WRITE_DAC
                    | WRITE_OWNER)
                != 0
            {
                return Err("Installation is writable by the app user. Install the development package in its protected versioned directory before starting Achimeld.".into());
            }
        }
        if path.is_dir() {
            for entry in std::fs::read_dir(&path)
                .map_err(|_| "installation permission inventory unavailable")?
            {
                paths.push(
                    entry
                        .map_err(|_| "installation permission inventory unavailable")?
                        .path(),
                );
            }
        }
    }
    Ok(())
}
