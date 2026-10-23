//! Owned Windows handles and LocalFree security descriptors.
use std::ptr;
use windows_sys::Win32::{Foundation::*, Security::Authorization::*};

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
pub struct Handle(pub usize);
impl Handle {
    pub fn raw(&self) -> HANDLE {
        self.0 as HANDLE
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Handle owns a successful Windows handle; invalid sentinel handles are excluded before closing once.
        unsafe {
            if self.0 != 0 && self.raw() != INVALID_HANDLE_VALUE {
                CloseHandle(self.raw());
            }
        }
    }
}
pub(super) fn error(message: &str) -> String {
    // SAFETY: GetLastError reads thread-local error state and has no pointer/ownership requirements.
    format!("{message} (Windows {})", unsafe { GetLastError() })
}
pub struct Descriptor(pub usize);
impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: Descriptor exclusively owns a LocalAlloc security descriptor, released once with LocalFree.
        unsafe {
            LocalFree(self.0 as HLOCAL);
        }
    }
}
pub fn descriptor(sddl: &str) -> Result<Descriptor, String> {
    let text = wide(sddl);
    let mut sd = ptr::null_mut();
    // SAFETY: Terminated SDDL and output storage are live; a successful LocalAlloc result transfers to Descriptor.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut sd,
            ptr::null_mut(),
        ) == 0
        {
            return Err(error("cannot create restricted permissions"));
        }
    }
    Ok(Descriptor(sd as usize))
}
