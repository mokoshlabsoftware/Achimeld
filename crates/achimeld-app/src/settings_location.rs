use std::io::Read;
use std::path::{Path, PathBuf};

// The locator stays in APPDATA so a selected folder can be found after restart.
// Shared settings never determine a project's localization-catalog location.
#[derive(Clone)]
pub(crate) struct SettingsLocation {
    pub folder: PathBuf,
    pub needs_choice: bool,
    locator: PathBuf,
}
impl SettingsLocation {
    #[cfg(test)]
    pub(crate) fn test_location(folder: PathBuf, locator: PathBuf) -> Self {
        Self {
            folder,
            locator,
            needs_choice: false,
        }
    }
    pub fn discover() -> Result<Self, String> {
        let default =
            PathBuf::from(std::env::var_os("APPDATA").ok_or("Settings directory unavailable")?)
                .join("Achimeld");
        let locator = default.join("rust-settings-folder.json");
        let legacy = legacy_folder();
        let needs_choice = !locator.exists() && legacy.is_none();
        let folder = if locator.exists() {
            serde_json::from_slice::<PathBuf>(&read_json(&locator, 128 * 1024)?)
                .map_err(|e| format!("Invalid settings folder: {e}"))?
        } else {
            legacy.unwrap_or(default)
        };
        if !folder.is_absolute() {
            return Err("Settings folder must be absolute".into());
        }
        Ok(Self {
            folder,
            locator,
            needs_choice,
        })
    }
    pub fn default_location() -> Result<Self, String> {
        let folder =
            PathBuf::from(std::env::var_os("APPDATA").ok_or("Settings directory unavailable")?)
                .join("Achimeld");
        Ok(Self {
            locator: folder.join("rust-settings-folder.json"),
            folder,
            needs_choice: true,
        })
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.folder.join(name)
    }
    pub fn remember(&self, folder: &Path) -> Result<(), String> {
        validate_folder(folder)?;
        std::fs::create_dir_all(
            self.locator
                .parent()
                .ok_or("Settings directory unavailable")?,
        )
        .map_err(|e| e.to_string())?;
        achimeld_persistence::project::write_atomic(
            &self.locator,
            &serde_json::to_vec(folder).map_err(|e| e.to_string())?,
        )
    }
}

pub(crate) fn validate_folder(folder: &Path) -> Result<(), String> {
    if !folder.is_absolute() || !folder.is_dir() {
        return Err("Choose an existing absolute settings folder".into());
    }
    achimeld_security::windows::reject_reparse(folder).map_err(|e| e.to_string())?;
    let probe = folder.join(format!(".achimeld-{}.tmp", achimeld_application::new_id()));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| format!("Settings folder is not writable: {e}"))?;
    drop(file);
    std::fs::remove_file(probe).map_err(|e| e.to_string())
}

pub(crate) fn read_json(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    achimeld_security::windows::reject_reparse(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Settings file exceeds its size limit".into());
    }
    achimeld_security::wire::check_json_depth(&bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(windows)]
fn legacy_folder() -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    for key in ["Software\\Achimeld", "Software\\Achiever"] {
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let name: Vec<u16> = "SettingsFolder".encode_utf16().chain(Some(0)).collect();
        let mut data = vec![0u16; 32768];
        let mut size = (data.len() * 2) as u32;
        // SAFETY: key/name are terminated UTF-16; data has size bytes of writable
        // storage. The call only reads this fixed HKCU value and retains no pointers.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status == 0 && size >= 2 && size as usize <= data.len() * 2 && size.is_multiple_of(2) {
            data.truncate(size as usize / 2);
            if data.last() == Some(&0) {
                data.pop();
            }
            if let Ok(text) = String::from_utf16(&data) {
                let path = PathBuf::from(text);
                if path.is_absolute() && path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
#[cfg(not(windows))]
fn legacy_folder() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chosen_folder_survives_restart_and_cannot_redirect_project_catalogs() {
        let root = crate::test_output::root()
            .join(format!("folder-test-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(root.join("chosen")).unwrap();
        let root = std::path::absolute(root).unwrap();
        let location = SettingsLocation {
            folder: root.clone(),
            locator: root.join("locator.json"),
            needs_choice: false,
        };
        let chosen = root.join("chosen");
        location.remember(&chosen).unwrap();
        let restored: PathBuf =
            serde_json::from_slice(&read_json(&location.locator, 128 * 1024).unwrap()).unwrap();
        assert_eq!(restored, chosen);
        assert!(!chosen.join("localization.csv").exists());
        assert!(location.remember(Path::new("relative")).is_err());
        assert_eq!(std::fs::read_dir(&chosen).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
