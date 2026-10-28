use super::*;

impl Host {
    pub(super) fn require_browser_idle(&self) -> Result<(), String> {
        if self.read.is_some() || self.synchronization.is_some() || self.browser_profile.is_some() {
            Err("Finish or stop the active Steam operation first".into())
        } else {
            Ok(())
        }
    }

    pub(super) fn delete_browser_profile(&mut self) -> Result<(), String> {
        self.require_browser_idle()?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let ctx = self.ctx.clone();
        std::thread::Builder::new().name("browser-profile".into()).spawn(move || {
            let result = Session::connect().and_then(|mut session| session.clear_profile())
                .map_err(|_| "Could not delete the browser profile. Close the dedicated Steam browser and try again.".to_owned());
            let _ = sender.send(result);
            ctx.request_repaint();
        }).map_err(|_| "Could not start browser profile deletion")?;
        self.browser_profile = Some(receiver);
        self.verified_sync = None;
        self.status = logged_status("Deleting the dedicated browser profile…".into());
        Ok(())
    }

    pub(super) fn poll_browser_profile(&mut self) {
        let result = self
            .browser_profile
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Browser profile deletion stopped".into()))
                }
            });
        if let Some(result) = result {
            self.browser_profile = None;
            self.report(
                result,
                "Browser profile deleted. Validate to sign in again.",
            );
        }
    }

    pub(super) fn forget_steam_validation(&mut self) -> Result<(), String> {
        self.require_browser_idle()?;
        let document = forgotten(self.editor.document())?;
        self.execute(
            "Forget Steam validation",
            Command::ImportArtwork(Box::new(document)),
            None,
        )?;
        self.verified_sync = None;
        self.sync_error = None;
        self.art_stats_epoch += 1;
        Ok(())
    }
}

// Keep immutable files for undo. A banner used by authored content becomes ordinary
// artwork; only its validation role is removed. Never clear synchronization baselines
// or the host's uncertain-write recovery journal as a side effect of forgetting access.
fn forgotten(document: &Document) -> Result<Document, String> {
    let mut project = document.project().clone();
    project["lastRemoteSnapshot"] = Value::Null;
    project["lastVerifiedAtUtc"] = Value::Null;
    let images = project["assets"]["images"].take();
    let mut retained = Vec::new();
    for mut entry in images.as_array().into_iter().flatten().cloned() {
        if entry["steamBannerAppId"] == document.app_id() {
            let used = entry["id"]
                .as_str()
                .is_some_and(|id| contains_id(&project, id));
            if !used {
                continue;
            }
            if let Some(object) = entry.as_object_mut() {
                object.remove("steamBannerAppId");
            }
        }
        retained.push(entry);
    }
    project["assets"]["images"] = json!(retained);
    Document::from_parts(project, document.catalog().clone())
}

fn contains_id(value: &Value, id: &str) -> bool {
    match value {
        Value::String(text) => text == id,
        Value::Array(items) => items.iter().any(|item| contains_id(item, id)),
        Value::Object(fields) => fields.values().any(|item| contains_id(item, id)),
        _ => false,
    }
}

pub(super) fn open_folder() -> Result<(), String> {
    let session = Session::connect().map_err(|error| error.to_string())?;
    let directory = &session.status().profile_directory;
    #[cfg(windows)]
    {
        achimeld_security::windows::reject_reparse(directory).map_err(|error| error.to_string())?;
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: Live terminated strings identify only the supervisor-validated
        // fixed profile directory. No executable, URL, or arguments are accepted.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("Windows could not open the browser profile folder".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Opening the browser profile requires Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forgetting_validation_preserves_authored_content_used_artwork_and_undo() {
        let mut host = Host::new(egui::Context::default());
        host.editor
            .execute(
                "Achievement",
                Command::Add(achimeld_application::EntityKind::Achievement),
                None,
            )
            .unwrap();
        let unused = new_id();
        let used = new_id();
        let ordinary = new_id();
        let mut project = host.editor.document().project().clone();
        project["steamAppId"] = json!(480);
        project["lastRemoteSnapshot"] = json!({"appId":480,"isComplete":true});
        project["lastVerifiedAtUtc"] = json!("2026-10-28T12:00:00Z");
        project["achievements"][0]["generalImageId"] = json!(used);
        project["assets"]["images"] = json!([
            {"id":unused,"steamBannerAppId":480,"displayName":"Banner","image":{"kind":"RelativeToProject","path":"banner.png"}},
            {"id":used,"steamBannerAppId":480,"displayName":"Used banner","image":{"kind":"RelativeToProject","path":"used.png"}},
            {"id":ordinary,"displayName":"User art","image":{"kind":"RelativeToProject","path":"user.png"}}
        ]);
        let before =
            Document::from_parts(project, host.editor.document().catalog().clone()).unwrap();
        host.editor = Editor::new(before.clone());
        host.verified_sync = Some((host.editor_session_id.clone(), host.editor.revision()));
        host.forget_steam_validation().unwrap();
        let after = host.editor.document().project();
        assert!(after["lastRemoteSnapshot"].is_null());
        assert!(after["lastVerifiedAtUtc"].is_null());
        let images = after["assets"]["images"].as_array().unwrap();
        assert_eq!(images.len(), 2);
        assert!(
            images
                .iter()
                .all(|entry| entry["steamBannerAppId"].is_null())
        );
        assert!(images.iter().any(|entry| entry["id"] == used));
        assert!(images.iter().any(|entry| entry["id"] == ordinary));
        for key in ["achievements", "stats", "syncBaselines", "tombstones"] {
            assert_eq!(after[key], before.project()[key]);
        }
        assert!(host.verified_sync.is_none());
        assert!(host.editor.undo());
        assert_eq!(host.editor.document(), &before);
        assert!(!host.synchronization_verified());
    }

    #[test]
    fn profile_deletion_blocks_other_steam_operations_and_reset_prompts() {
        let mut host = Host::new(egui::Context::default());
        let (_sender, receiver) = mpsc::sync_channel(1);
        host.browser_profile = Some(receiver);
        assert!(host.start_read().is_err());
        assert!(host.start_synchronization().is_err());
        host.request(UiRequest::DeleteBrowserProfile);
        assert!(host.dialog.is_none());
        host.request(UiRequest::ForgetSteamValidation);
        assert!(host.dialog.is_none());
    }
}
