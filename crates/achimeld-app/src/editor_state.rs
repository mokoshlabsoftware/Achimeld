use achimeld_application::{GradientPreset, IconStyle, LayerLayout};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct State {
    pub layout: LayerLayout,
    pub styles: Vec<IconStyle>,
    pub style_collections: Vec<PathBuf>,
    pub gradients: Vec<GradientPreset>,
    pub effect_presets: Vec<achimeld_application::effect_presets::EffectPreset>,
}
impl State {
    fn validate(&self) -> Result<(), String> {
        self.layout.validate()?;
        achimeld_application::effect_presets::validate_library(&self.effect_presets)?;
        if self.gradients.len() > 512 {
            return Err("Too many gradient presets".into());
        }
        let mut gradients = std::collections::BTreeSet::new();
        for gradient in &self.gradients {
            gradient.validate()?;
            if !gradients.insert(&gradient.id) {
                return Err("Duplicate gradient identity".into());
            }
        }
        if self.styles.len() > 128 {
            return Err("Too many saved icon styles".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for style in &self.styles {
            style.validate()?;
            if !ids.insert(&style.id) {
                return Err("Duplicate style identity".into());
            }
        }
        Ok(())
    }
}
// Local palette navigation and the user library are separate from project history.
// The host picks one fixed state path; presentation cannot supply a filesystem path.
pub(crate) struct EditorState {
    pub state: State,
    path: Option<PathBuf>,
    load_error: Option<String>,
}
impl EditorState {
    pub fn style_collections(&mut self, paths: Vec<PathBuf>) -> Result<(), String> {
        let mut state = self.state.clone();
        state.style_collections = paths;
        self.save(state)
    }
    pub fn persist(&mut self) -> Result<(), String> {
        self.save(self.state.clone())
    }
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let mut state = if path.exists() {
            load(&path)?
        } else {
            State::default()
        };
        if !path.exists() {
            let folder = path.parent().ok_or("Settings directory unavailable")?;
            let gradients = folder.join("user.gradients.json");
            if gradients.exists() {
                state.gradients = serde_json::from_slice(&crate::settings_location::read_json(
                    &gradients, MAX_BYTES,
                )?)
                .map_err(|e| e.to_string())?;
            }
            let styles = folder.join("user.icon-styles.json");
            if styles.exists() {
                state.styles = crate::legacy_settings::icon_styles(
                    &crate::settings_location::read_json(&styles, MAX_BYTES)?,
                )?;
            }
        }
        // Earlier Rust migrations retained recipes but dropped the optional
        // default PNG. Recover only matching identities/recipes from the original
        // library; saved Rust names, recipes and existing previews stay authoritative.
        if path.exists()
            && let Some(folder) = path.parent()
            && let Ok(bytes) = crate::settings_location::read_json(
                &folder.join("user.icon-styles.json"),
                MAX_BYTES,
            )
            && let Ok(original) = crate::legacy_settings::icon_styles(&bytes)
        {
            for style in &mut state.styles {
                if style.preview_png.is_none()
                    && let Some(source) = original.iter().find(|source| {
                        source.id == style.id
                            && achimeld_application::style_signature(&source.recipe)
                                == achimeld_application::style_signature(&style.recipe)
                    })
                {
                    style.preview_png = source.preview_png.clone();
                }
            }
        }
        state.validate()?;
        Ok(Self {
            state,
            path: Some(path),
            load_error: None,
        })
    }
    pub fn new(path: Option<PathBuf>) -> Self {
        let loaded = path.as_deref().map_or_else(|| Ok(State::default()), load);
        match loaded {
            Ok(state) => Self {
                state,
                path,
                load_error: None,
            },
            Err(error) => Self {
                state: State::default(),
                path,
                load_error: Some(error),
            },
        }
    }
    fn save(&mut self, state: State) -> Result<(), String> {
        if let Some(error) = &self.load_error {
            return Err(format!(
                "Saved editor state needs recovery; the original file was preserved: {error}"
            ));
        }
        state.validate()?;
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Editor state is too large".into());
        }
        let path = self
            .path
            .as_deref()
            .ok_or("Settings directory unavailable")?;
        std::fs::create_dir_all(path.parent().ok_or("Settings directory unavailable")?)
            .map_err(|e| e.to_string())?;
        achimeld_persistence::project::write_atomic(path, &bytes)?;
        self.state = state;
        Ok(())
    }
    pub fn layout(&mut self, layout: LayerLayout) -> Result<(), String> {
        if layout == self.state.layout {
            return Ok(());
        }
        let mut state = self.state.clone();
        state.layout = layout;
        self.save(state)
    }
    pub fn gradient(&mut self, gradient: GradientPreset) -> Result<(), String> {
        gradient.validate()?;
        let mut state = self.state.clone();
        if let Some(old) = state.gradients.iter_mut().find(|g| g.id == gradient.id) {
            *old = gradient;
        } else {
            state.gradients.push(gradient);
        }
        self.save(state)
    }
    pub fn effect(
        &mut self,
        preset: achimeld_application::effect_presets::EffectPreset,
    ) -> Result<(), String> {
        preset.validate()?;
        let mut state = self.state.clone();
        if let Some(old) = state.effect_presets.iter_mut().find(|p| p.id == preset.id) {
            *old = preset;
        } else {
            state.effect_presets.push(preset);
        }
        self.save(state)
    }
    pub fn import_effects(
        &mut self,
        mut presets: Vec<achimeld_application::effect_presets::EffectPreset>,
    ) -> Result<(), String> {
        let mut state = self.state.clone();
        for preset in &mut presets {
            preset.id = achimeld_application::new_id();
        }
        state.effect_presets.extend(presets);
        self.save(state)
    }
    pub fn delete_effect(&mut self, id: &str) -> Result<(), String> {
        let mut state = self.state.clone();
        state.effect_presets.retain(|preset| preset.id != id);
        self.save(state)
    }
    pub fn import_gradients(&mut self, gradients: Vec<GradientPreset>) -> Result<(), String> {
        let mut state = self.state.clone();
        state.gradients.extend(gradients);
        self.save(state)
    }
    pub fn delete_gradient(&mut self, id: &str) -> Result<(), String> {
        self.delete_gradients(&[id.to_owned()])
    }
    pub fn delete_gradients(&mut self, ids: &[String]) -> Result<(), String> {
        let mut state = self.state.clone();
        state.gradients.retain(|g| !ids.contains(&g.id));
        self.save(state)
    }
    #[cfg(test)]
    pub fn style(&mut self, style: IconStyle) -> Result<(), String> {
        style.validate()?;
        let mut state = self.state.clone();
        if let Some(old) = state.styles.iter_mut().find(|s| s.id == style.id) {
            *old = style;
        } else {
            state.styles.push(style);
        }
        self.save(state)
    }
}
fn load(path: &Path) -> Result<State, String> {
    let mut bytes = Vec::new();
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(error) => return Err(error.to_string()),
    };
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Editor state is too large".into());
    }
    let state: State = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    state.validate()?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_gradients_and_styles_migrate_to_distinct_rust_state_without_changing_originals() {
        let root = root();
        let gradient = serde_json::json!([{"Id":achimeld_application::new_id(),"Name":"Gradient","Stops":[{"Offset":0.0,"ColorHex":"#FF112233"},{"Offset":1.0,"ColorHex":"#FF445566"}]}]);
        let gradient_bytes = serde_json::to_vec(&gradient).unwrap();
        std::fs::write(root.join("user.gradients.json"), &gradient_bytes).unwrap();
        let style_bytes=serde_json::to_vec(&serde_json::json!([{"Id":achimeld_application::new_id(),"Name":"Legacy style","Recipe":{"Layers":[{"Id":{"Value":achimeld_application::new_id()},"Kind":5,"Gradient":{"Kind":0,"Stops":gradient[0]["Stops"]},"Effects":[]}]}}])).unwrap();
        std::fs::write(root.join("user.icon-styles.json"), &style_bytes).unwrap();
        let path = root.join("rust-editor-state.json");
        let mut state = EditorState::open(path.clone()).unwrap();
        assert_eq!(state.state.styles.len(), 1);
        assert_eq!(state.state.gradients.len(), 1);
        state.persist().unwrap();
        let restored = EditorState::open(path.clone()).unwrap();
        assert_eq!(restored.state.gradients, state.state.gradients);
        assert_eq!(restored.state.styles, state.state.styles);
        let mut original: serde_json::Value = serde_json::from_slice(&style_bytes).unwrap();
        original[0]["PreviewPng"] = serde_json::json!("iVBORw0KGgo=");
        let with_preview = serde_json::to_vec(&original).unwrap();
        std::fs::write(root.join("user.icon-styles.json"), &with_preview).unwrap();
        let recovered = EditorState::open(path).unwrap();
        assert_eq!(
            recovered.state.styles[0].preview_png.as_deref(),
            Some("iVBORw0KGgo=")
        );
        assert_eq!(
            recovered.state.styles[0].recipe,
            state.state.styles[0].recipe
        );
        assert_eq!(recovered.state.styles[0].name, state.state.styles[0].name);
        assert_eq!(
            std::fs::read(root.join("user.gradients.json")).unwrap(),
            gradient_bytes
        );
        assert_eq!(
            std::fs::read(root.join("user.icon-styles.json")).unwrap(),
            with_preview
        );
        assert!(!root.join("localization.csv").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "Requires ACHIMELD_LEGACY_SETTINGS pointing to an audit copy of the original folder"]
    fn actual_original_style_library_and_preferences_load() {
        let folder = PathBuf::from(
            std::env::var_os("ACHIMELD_LEGACY_SETTINGS").expect("Set ACHIMELD_LEGACY_SETTINGS"),
        );
        let state = EditorState::open(folder.join("rust-editor-state.json")).unwrap();
        let bytes = std::fs::read(folder.join("user.icon-styles.json")).unwrap();
        assert_eq!(
            state.state.styles.len(),
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .unwrap()
                .as_array()
                .unwrap()
                .len()
        );
        assert!(
            state
                .state
                .styles
                .iter()
                .any(|style| style.name == "Shiny Icon")
        );
        assert_eq!(
            crate::legacy_settings::preferences(
                &std::fs::read(folder.join("preferences.json")).unwrap()
            )
            .unwrap()
            .naming_convention,
            "ACHIEVEMENT_{0:000}"
        );
    }
    fn root() -> PathBuf {
        let path = crate::test_output::root().join(format!(
            "editor-state-test-{}",
            achimeld_application::new_id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn style() -> IconStyle {
        IconStyle {
            preview_png: None,
            id: achimeld_application::new_id(),
            name: "Style".into(),
            recipe: serde_json::json!({"layers":[{"id":achimeld_application::new_id(),"kind":"Solid","solidColorHex":"#FFFFFFFF"}]}),
        }
    }
    #[test]
    fn saved_style_and_layer_geometry_survive_restart() {
        let root = root();
        let path = root.join("state.json");
        let mut state = EditorState::new(Some(path.clone()));
        let style = style();
        state.style(style.clone()).unwrap();
        let layout = LayerLayout {
            floating: true,
            position: Some([-760.0, 40.0]),
            size: [900.0, 680.0],
            ..Default::default()
        };
        state.layout(layout.clone()).unwrap();
        let loaded = EditorState::new(Some(path));
        assert_eq!(loaded.state.styles, vec![style]);
        assert_eq!(loaded.state.layout, layout);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_library_and_failed_write_preserve_the_original_and_state() {
        let root = root();
        let path = root.join("state.json");
        std::fs::write(&path, b"corrupt").unwrap();
        let mut state = EditorState::new(Some(path.clone()));
        assert!(state.style(style()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"corrupt");
        let blocked = root.join("file");
        std::fs::write(&blocked, b"original").unwrap();
        let mut state = EditorState::new(Some(blocked.join("state.json")));
        assert!(state.style(style()).is_err());
        assert!(state.state.styles.is_empty());
        assert_eq!(std::fs::read(blocked).unwrap(), b"original");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn gradient_batch_deletion_and_restore_persist_together_and_failed_writes_preserve_state() {
        let root = root();
        let path = root.join("gradient-state.json");
        let mut state = EditorState::open(path.clone()).unwrap();
        let gradients: Vec<_> = (0..3)
            .map(|i| GradientPreset {
                id: achimeld_application::new_id(),
                name: format!("Gradient {i}"),
                stops: vec![achimeld_application::GradientStop {
                    offset: 0.0,
                    color_hex: "#FF123456".into(),
                }],
            })
            .collect();
        state.import_gradients(gradients.clone()).unwrap();
        state
            .delete_gradients(&[gradients[0].id.clone(), gradients[2].id.clone()])
            .unwrap();
        assert_eq!(state.state.gradients, vec![gradients[1].clone()]);
        assert_eq!(
            EditorState::open(path.clone()).unwrap().state.gradients,
            state.state.gradients
        );
        state
            .import_gradients(vec![gradients[0].clone(), gradients[2].clone()])
            .unwrap();
        let restored = state.state.gradients.clone();
        let bytes = std::fs::read(&path).unwrap();
        let blocked = root.join("blocked");
        std::fs::write(&blocked, b"preserve").unwrap();
        state.path = Some(blocked.join("state.json"));
        assert!(
            state
                .delete_gradients(&gradients.iter().map(|g| g.id.clone()).collect::<Vec<_>>())
                .is_err()
        );
        assert_eq!(state.state.gradients, restored);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read(&blocked).unwrap(), b"preserve");
    }
}
