//! Fixed public release lookup and per-user acknowledgement.
use super::*;

pub(super) fn start(ctx: egui::Context) -> mpsc::Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let _ = std::thread::Builder::new()
        .name("public-version-check".into())
        .spawn(move || {
            let result = (|| {
                let session = Session::connect().map_err(|error| error.to_string())?;
                let mut worker = SteamReader::connect(session, Cancellation::default())
                    .map_err(|error| error.to_string())?;
                let version = worker
                    .check_public_version()
                    .map_err(|error| error.to_string())?;
                worker.stop().map_err(|error| error.to_string())?;
                Ok(version)
            })();
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    receiver
}

fn components(value: &str) -> Option<[u32; 4]> {
    let parts = value
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    parts.try_into().ok()
}

pub(super) fn is_newer(candidate: &str) -> bool {
    components(candidate)
        .zip(components(achimeld_ui::current_version()))
        .is_some_and(|(candidate, current)| candidate > current)
}

fn state_path() -> Result<PathBuf, String> {
    Ok(preferences_path()?.with_file_name("rust-public-release-seen.json"))
}

pub(super) fn has_seen(version: &str) -> Result<bool, String> {
    has_seen_at(&state_path()?, version)
}

fn has_seen_at(path: &Path, version: &str) -> Result<bool, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(257)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 256 {
        return Err("Public release state is too large".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let seen = value
        .get("lastAcknowledgedVersion")
        .and_then(Value::as_str)
        .and_then(components)
        .ok_or("Invalid public release state")?;
    Ok(components(version).is_some_and(|version| seen >= version))
}

pub(super) fn acknowledge(version: &str) -> Result<(), String> {
    acknowledge_at(&state_path()?, version)
}

pub(super) fn open_releases() -> Result<(), String> {
    #[cfg(windows)]
    {
        let url: Vec<u16> = "https://github.com/mokoshlabsoftware/Achimeld/releases"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: Live terminated strings identify a fixed public HTTPS URL.
        // No caller-provided executable, URL, or arguments enter this call.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("Windows could not open GitHub releases".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Opening GitHub releases requires Windows".into())
    }
}

fn acknowledge_at(path: &Path, version: &str) -> Result<(), String> {
    if components(version).is_none() {
        return Err("Invalid public release version".into());
    }
    std::fs::create_dir_all(path.parent().ok_or("Settings directory unavailable")?)
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&json!({ "lastAcknowledgedVersion": version }))
        .map_err(|error| error.to_string())?;
    achimeld_persistence::project::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_comparison_uses_all_four_parts() {
        assert!(!is_newer(achimeld_ui::current_version()));
        assert!(is_newer("999.0.0.0"));
        assert!(!is_newer("0.0.0.0"));
        assert!(!is_newer("invalid"));
    }
    #[test]
    fn acknowledgement_is_once_per_newer_public_version() {
        let folder = crate::test_output::root().join(format!("public-release-{}", new_id()));
        let path = folder.join("state.json");
        assert!(!has_seen_at(&path, "1.2.3.4").unwrap());
        acknowledge_at(&path, "1.2.3.4").unwrap();
        assert!(has_seen_at(&path, "1.2.3.4").unwrap());
        assert!(has_seen_at(&path, "1.2.3.3").unwrap());
        assert!(!has_seen_at(&path, "1.2.3.5").unwrap());
        acknowledge_at(&path, "1.2.3.5").unwrap();
        assert!(has_seen_at(&path, "1.2.3.5").unwrap());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
