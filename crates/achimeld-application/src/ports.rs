use crate::{Command, Editor};
use std::path::PathBuf;

/// Closed destinations for locally dropped artwork. Files are decoded and owned by the host.
#[derive(Clone, Debug)]
pub enum ImageImportTarget {
    ArtBank,
    NewAchievements {
        name: String,
        position: [i32; 2],
    },
    General {
        achievement_id: String,
        selected_layer_id: Option<String>,
        style_id: Option<String>,
    },
    Layers {
        achievement_id: String,
        selected_layer_id: Option<String>,
        style_id: Option<String>,
    },
}

#[derive(Clone)]
pub struct Preview {
    pub width: usize,
    pub height: usize,
    pub source_size: [usize; 2],
    /// Immutable pixels are shared across frames instead of copied on every paint.
    pub rgba: std::sync::Arc<[u8]>,
    pub fingerprint: String,
}

/// Fixed local user intents. There is no raw worker operation, URL, process or executable path.
pub enum UiRequest {
    NewProject,
    OpenProject,
    /// A project from the host's recent list, pinned across an optional save.
    OpenRecentProject(PathBuf),
    SaveProject,
    SaveProjectAs,
    ReadSteam,
    CancelRead,
    PrepareSynchronization,
    ApplySynchronization(Vec<(String, String)>),
    CancelSynchronization,
    OpenBrowserProfile,
    DeleteBrowserProfile,
    ForgetSteamValidation,
    PreviewSynchronizationIcons(String),
    CancelExport,
    ExportIcons,
    ExportAchievementTranslations,
    ImportAchievementTranslations,
    ImportAtlas,
    ImportImage,
    /// Import files supplied by a desktop drop. The host validates and retains them.
    ImportArtFiles(Vec<PathBuf>),
    ImportDroppedImages {
        paths: Vec<PathBuf>,
        target: ImageImportTarget,
    },
    RefreshArt,
    ReloadArt {
        atlas: bool,
        id: String,
    },
    ResizeArt {
        atlas: bool,
        id: String,
        width: u32,
        height: u32,
    },
    ImportGeneralImage {
        achievement_id: String,
    },
    ImportGeneralFiles {
        achievement_id: String,
        paths: Vec<PathBuf>,
    },
    PasteGeneralImage {
        achievement_id: String,
    },
    CopyGeneralImage {
        source: serde_json::Value,
    },
    ImportLayerImage {
        achievement_id: String,
        layer_id: Option<String>,
    },
    OpenPreferences,
    /// Open only the host-owned diagnostic directory in Windows Explorer.
    OpenLogFolder,
    LoadParticlePreset {
        achievement_id: String,
        layer_id: String,
    },
    SaveParticlePreset(serde_json::Value),
    ImportGradients,
    ExportGradients(Vec<crate::GradientPreset>),
    ImportEffects(crate::effect_presets::EffectKind),
    ExportEffects {
        kind: crate::effect_presets::EffectKind,
        presets: Vec<crate::effect_presets::EffectPreset>,
    },
    ChooseSettingsFolder,
    AboutDismissed,
    StyleViewer(crate::style_viewer::StyleViewerAction),
    Exit,
}

/// A temporary canvas composition. It never changes the document or its history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerBlendPreview {
    pub achievement_id: String,
    pub layer_id: String,
    pub mode: String,
}

/// The presentation crate can inspect state and request commands; the host owns all I/O.
pub trait EditorPort {
    /// The host validates the native file; normal UI history applies its settings.
    fn take_particle_preset(
        &mut self,
        _achievement_id: &str,
        _layer_id: &str,
    ) -> Option<serde_json::Value> {
        None
    }
    /// Installed outline font families, discovered by the host. None while loading.
    fn system_font_families(&self) -> Option<&[String]> {
        Some(&[])
    }
    fn style_viewer(&self) -> Option<&crate::style_viewer::StyleViewerState> {
        None
    }
    fn style_viewer_preview(&mut self) -> Option<Preview> {
        None
    }
    fn style_viewer_style_preview(&mut self, _style: &str, _edge: usize) -> Option<Preview> {
        None
    }
    fn style_viewer_image_preview(&mut self, _index: usize) -> Option<Preview> {
        None
    }
    fn style_viewer_default_image_preview(&mut self) -> Option<Preview> {
        None
    }
    fn editor(&self) -> &Editor;
    fn execute(&mut self, label: &str, command: Command, group: Option<&str>)
    -> Result<(), String>;
    fn finish_group(&mut self);
    fn undo(&mut self);
    fn redo(&mut self);
    fn request(&mut self, request: UiRequest);
    fn status(&self) -> &str;
    fn preview(&mut self, entity_id: &str) -> Option<Preview>;
    /// Preview a hovered layer blend mode until the menu item is left or dismissed.
    fn set_layer_blend_preview(&mut self, _preview: Option<LayerBlendPreview>) {}
    /// Canvas artwork is sized in physical pixels, independently of export and
    /// inspector previews. The host owns asynchronous composition and its cache.
    fn canvas_preview(&mut self, entity_id: &str, _edge: usize) -> Option<Preview> {
        self.preview(entity_id)
    }
    /// Saved recipe rendered with the achievement's General images and linked text.
    /// The host composes a temporary view; neither the style nor achievement is edited.
    fn icon_style_preview(
        &mut self,
        _style_id: &str,
        _achievement_id: &str,
        _edge: usize,
    ) -> Option<Preview> {
        None
    }
    fn layer_preview(&mut self, _entity_id: &str, _layer_id: &str) -> Option<Preview> {
        None
    }
    fn asset_preview(&mut self, _atlas: bool, _asset_id: Option<&str>) -> Option<Preview> {
        None
    }
    fn art_file_bytes(&mut self, _reference: &serde_json::Value) -> Option<u64> {
        None
    }
    fn art_inventory_ready(&self) -> bool {
        true
    }
    fn art_saved_bytes(&self) -> Option<u64> {
        None
    }
    /// Latest completed General import, valid only at its document revision.
    fn general_image_added(&self) -> Option<(String, i64, u64)> {
        None
    }
    fn image_import_selection(&self) -> Option<(String, u64)> {
        None
    }
    fn art_busy(&self) -> bool {
        false
    }
    fn art_pending_preview(&self) -> Option<Preview> {
        None
    }
    fn layer_layout(&self) -> crate::LayerLayout {
        Default::default()
    }
    fn save_layer_layout(&mut self, _layout: crate::LayerLayout) -> Result<(), String> {
        Ok(())
    }
    fn icon_styles(&self) -> Vec<crate::IconStyle> {
        self.editor().document().icon_styles()
    }
    /// Local Steam-set metadata from the open style set, or a saved style-set document.
    fn icon_style_steam_sets(&self, id: &str) -> Vec<crate::style_viewer::SteamAchievementSetLink> {
        if let Some(viewer) = self.style_viewer()
            && viewer.styles.iter().any(|style| style.id == id)
        {
            return viewer
                .steam_sets
                .iter()
                .filter(|link| link.style_id == id && link.validate().is_ok())
                .cloned()
                .collect();
        }
        self.editor()
            .document()
            .project()
            .get("styleViewerSteamSets")
            .and_then(|value| {
                serde_json::from_value::<Vec<crate::style_viewer::SteamAchievementSetLink>>(
                    value.clone(),
                )
                .ok()
            })
            .unwrap_or_default()
            .into_iter()
            .filter(|link| link.style_id == id && link.validate().is_ok())
            .collect()
    }
    /// User library entries are templates, never styles visible to icon layers.
    fn global_icon_styles(&self) -> &[crate::IconStyle] {
        &[]
    }
    fn import_global_icon_style(&mut self, id: &str) -> Result<(), String> {
        let mut style = self
            .global_icon_styles()
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or("Global icon style no longer exists")?;
        style.id = crate::new_id();
        self.save_icon_style(style)
    }
    fn settings_folder(&self) -> Option<&std::path::Path> {
        None
    }
    fn gradients(&self) -> &[crate::GradientPreset] {
        &[]
    }
    fn gradient_maps(&self) -> &[crate::gradient_map::GradientMapPreset] {
        &[]
    }
    fn save_gradient_map(
        &mut self,
        _preset: crate::gradient_map::GradientMapPreset,
    ) -> Result<(), String> {
        Err("2D gradient library unavailable".into())
    }
    fn delete_gradient_map(&mut self, _id: &str) -> Result<(), String> {
        Err("2D gradient library unavailable".into())
    }
    fn effect_presets(&self) -> &[crate::effect_presets::EffectPreset] {
        &[]
    }
    fn save_effect_preset(
        &mut self,
        _preset: crate::effect_presets::EffectPreset,
    ) -> Result<(), String> {
        Err("Effect library unavailable".into())
    }
    fn delete_effect_preset(&mut self, _id: &str) -> Result<(), String> {
        Err("Effect library unavailable".into())
    }
    fn effect_preview(&mut self, _preset: &crate::effect_presets::EffectPreset) -> Option<Preview> {
        None
    }
    fn effect_preview_error(&self) -> Option<&str> {
        None
    }
    /// Host-owned, bounded pre-polar SDF preview; never changes the document.
    fn explosion_construction_preview(
        &mut self,
        _settings: &serde_json::Value,
        _instance: &str,
    ) -> Option<Preview> {
        None
    }
    fn explosion_construction_error(&self) -> Option<&str> {
        None
    }
    fn save_gradient(&mut self, _gradient: crate::GradientPreset) -> Result<(), String> {
        Err("Gradient library unavailable".into())
    }
    fn delete_gradient(&mut self, _id: &str) -> Result<(), String> {
        Err("Gradient library unavailable".into())
    }
    /// One host-owned library transaction for the full selection.
    fn delete_gradients(&mut self, _ids: &[String]) -> Result<(), String> {
        Err("Gradient library unavailable".into())
    }
    fn restore_gradients(&mut self, _gradients: Vec<crate::GradientPreset>) -> Result<(), String> {
        Err("Gradient library unavailable".into())
    }
    fn save_icon_style(&mut self, style: crate::IconStyle) -> Result<(), String> {
        self.execute(
            "Save project icon style",
            Command::SaveIconStyle(style),
            None,
        )
    }
    fn delete_icon_style(&mut self, id: &str) -> Result<(), String> {
        self.execute(
            "Delete project icon style",
            Command::DeleteIconStyle(id.into()),
            None,
        )
    }
    fn rename_icon_style(&mut self, id: &str, name: &str) -> Result<(), String> {
        let mut style = self
            .icon_styles()
            .into_iter()
            .find(|style| style.id == id)
            .ok_or("Project icon style no longer exists")?;
        let name = name.trim();
        if name.is_empty() {
            return Err("Enter a style name".into());
        }
        if name.len() > 256 {
            return Err("The style name is too long (maximum 256 UTF-8 bytes)".into());
        }
        if style.name == name {
            return Ok(());
        }
        style.name = name.into();
        self.finish_group();
        self.execute(
            "Rename project icon style",
            Command::SaveIconStyle(style),
            None,
        )
    }
    fn execute_style_edit(
        &mut self,
        style: crate::IconStyle,
        command: Command,
        group: Option<&str>,
    ) -> Result<(), String> {
        if !self.icon_styles().iter().any(|s| s.id == style.id) {
            return Err("Project icon style no longer exists".into());
        }
        self.execute(
            "Edit icon style",
            Command::Batch(vec![command, Command::SaveIconStyle(style)]),
            group,
        )
    }
    fn preview_error(&self, _entity_id: &str) -> Option<&str> {
        None
    }
    fn preferences(&self) -> &crate::Preferences;
    fn recent_projects(&self) -> &[PathBuf] {
        &[]
    }
    fn reading(&self) -> bool {
        false
    }
    fn browser_profile_busy(&self) -> bool {
        false
    }
    fn synchronization_review(&self) -> Option<&serde_json::Value> {
        None
    }
    /// True only for the unchanged document successfully verified in this session.
    fn synchronization_verified(&self) -> bool {
        false
    }
    fn synchronization_error(&self) -> Option<&str> {
        None
    }
    fn synchronizing(&self) -> bool {
        false
    }
    fn synchronization_icon_preview(&self, _api_name: &str, _locked: bool) -> Option<Preview> {
        None
    }
    fn exporting(&self) -> bool {
        false
    }
}
