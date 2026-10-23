//! Native selection and gradient file I/O belong exclusively to the supervised host.
use super::*;
use achimeld_application::{GradientPreset, gradient_file};

pub(super) fn export(path: &Path, gradients: &[GradientPreset]) -> Result<(), String> {
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("grd"))
    {
        return Err("Choose a Photoshop .grd file".into());
    }
    let bytes = gradient_file::encode(gradients)?;
    use std::io::Write;
    let temporary = path
        .parent()
        .ok_or("Choose an export folder")?
        .join(format!(".achimeld-gradients-{}.tmp", new_id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result
        .and_then(|()| std::fs::hard_link(&temporary, path))
        .map_err(|e| format!("Could not export gradients; choose an unused filename: {e}"));
    let _ = std::fs::remove_file(&temporary);
    result
}

impl Host {
    pub(super) fn import_gradients(&mut self, path: &Path) -> Result<(), String> {
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("grd"))
        {
            return Err("Choose a Photoshop .grd file".into());
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(gradient_file::MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let gradients = gradient_file::decode(&bytes)?;
        // Validate and persist the whole batch once; a malformed/oversized set changes nothing.
        self.editor_state.import_gradients(gradients)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photoshop_gradient_files_import_atomically_and_exports_preserve_existing_files() {
        let folder = crate::test_output::root().join(format!("gradient-files-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut host = Host::new(egui::Context::default());
        let state_path = folder.join("editor.json");
        host.editor_state = crate::editor_state::EditorState::open(state_path.clone()).unwrap();
        let preset = GradientPreset {
            id: new_id(),
            name: "Twilight".into(),
            stops: vec![
                achimeld_application::GradientStop {
                    offset: 0.0,
                    color_hex: "#00123456".into(),
                },
                achimeld_application::GradientStop {
                    offset: 1.0,
                    color_hex: "#FF987654".into(),
                },
            ],
        };
        let path = folder.join("Twilight.grd");
        export(&path, &[preset]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        host.import_gradients(&path).unwrap();
        assert_eq!(host.editor_state.state.gradients.len(), 1);
        assert_eq!(
            crate::editor_state::EditorState::open(state_path)
                .unwrap()
                .state
                .gradients
                .len(),
            1
        );
        let original = host.editor_state.state.gradients.clone();
        assert!(export(&path, &original).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let broken = folder.join("broken.grd");
        std::fs::write(&broken, &bytes[..bytes.len() - 1]).unwrap();
        assert!(host.import_gradients(&broken).is_err());
        assert_eq!(host.editor_state.state.gradients, original);
        let batch = vec![original[0].clone(); 512];
        assert!(host.editor_state.import_gradients(batch).is_err());
        assert_eq!(host.editor_state.state.gradients, original);
    }
}
