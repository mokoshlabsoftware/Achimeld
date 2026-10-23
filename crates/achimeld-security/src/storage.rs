//! Narrow platform adapter for same-volume file replacement.
use std::path::Path;

pub fn replace_file(staged: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let encode = |path: &Path| -> std::io::Result<Vec<u16>> {
            let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
            if units.contains(&0) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "File path contains NUL",
                ));
            }
            units.push(0);
            Ok(units)
        };
        let source = encode(staged)?;
        let destination = encode(target)?;
        // SAFETY: Both NUL-terminated UTF-16 buffers remain live through the call;
        // the API retains no pointers. Only same-volume replacement is requested.
        let result = unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(staged, target)
    }
}
