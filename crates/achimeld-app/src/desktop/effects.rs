use super::*;
use achimeld_application::effect_presets::{self, EffectKind, EffectPreset, MAX_COLLECTION_BYTES};

pub(super) fn export(
    kind: EffectKind,
    presets: &[EffectPreset],
    path: &Path,
) -> Result<(), String> {
    use std::io::Write;
    let bytes = effect_presets::encode(kind, presets)?;
    let temporary = path
        .parent()
        .ok_or("Choose an export folder")?
        .join(format!(".achimeld-effects-{}.tmp", new_id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result
        .and_then(|()| std::fs::hard_link(&temporary, path))
        .map_err(|e| format!("Could not export effects; choose an unused filename: {e}"));
    let _ = std::fs::remove_file(&temporary);
    result
}
impl Host {
    pub(super) fn import_effects(&mut self, kind: EffectKind, path: &Path) -> Result<(), String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_COLLECTION_BYTES as u64 {
            return Err("Effect collection exceeds the file size limit".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_COLLECTION_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        self.editor_state
            .import_effects(effect_presets::decode(kind, &bytes)?)
    }
}
pub(super) struct EffectPreview {
    worker: PreviewWorker,
    document: Document,
    achievement: String,
    key: String,
    layer: Value,
    generation: u64,
}
impl EffectPreview {
    pub(super) fn new(ctx: egui::Context) -> Result<Self, String> {
        let mut editor = Editor::default();
        editor.execute(
            "Preview",
            Command::Add(achimeld_application::EntityKind::Achievement),
            None,
        )?;
        let document = editor.document().clone();
        let achievement = document.entities(achimeld_application::EntityKind::Achievement)[0]["id"]
            .as_str()
            .ok_or("Missing preview identity")?
            .to_owned();
        Ok(Self {
            worker: PreviewWorker::with_threads(ctx, 2),
            document,
            key: format!("canvas:256:{achievement}"),
            achievement,
            layer: Value::Null,
            generation: 0,
        })
    }
    pub(super) fn preview(&mut self, preset: &EffectPreset) -> Option<Preview> {
        if preset.layer != self.layer {
            preset.validate().ok()?;
            let mut project = self.document.project().clone();
            project["achievements"][0]["icon"] = json!({"layers":[preset.layer]});
            let document = Document::from_parts(project, self.document.catalog().clone()).ok()?;
            self.document = document;
            self.layer = preset.layer.clone();
            self.generation += 1;
        }
        self.worker.poll();
        self.worker.achievement_preview(
            &self.achievement,
            Some(256),
            self.generation,
            &self.document,
            None,
        )
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.worker.error(&self.key)
    }
    pub(super) fn construction(&mut self, settings: &Value, instance: &str) -> Option<Preview> {
        if instance.len() > 512 {
            return None;
        }
        let key = format!("construction:{instance}");
        if &self.layer["explosion"] != settings || self.key != key {
            achimeld_application::explosion::ExplosionSettings::from_value(settings).ok()?;
            let layer = json!({"id":self.achievement,"kind":"Explosion","explosion":settings});
            let mut project = self.document.project().clone();
            project["achievements"][0]["icon"] = json!({"layers":[layer]});
            self.document = Document::from_parts(project, self.document.catalog().clone()).ok()?;
            self.layer = layer;
            self.key = key;
            self.generation += 1;
        }
        self.worker.poll();
        self.worker
            .preview(&self.key, self.generation, &self.document, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explosion_construction_preview_refines_latest_settings_without_changing_editor() {
        let mut preview = EffectPreview::new(egui::Context::default()).unwrap();
        let mut settings = achimeld_application::explosion::ExplosionSettings::new_deltoid();
        settings.count = 512;
        settings.different_seed_per_instance = true;
        settings.brightness_min = 0.2;
        settings.brightness_max = 1.0;
        settings.radial_blur = 0.2;
        settings.angular_blur = 0.2;
        settings.deltoid.as_mut().unwrap().ellipse = 0.5;
        preview.construction(&json!(settings), "construction/test");
        settings.brightness = 0.6;
        settings.invert_colors = true;
        settings.bottom_power = 2.0;
        settings.bottom_stretch = 0.75;
        settings.bottom_curve = achimeld_application::explosion::BottomCurve::Quadratic;
        let value = json!(settings);
        let expected = achimeld_imaging::render_explosion_construction(
            &value,
            "construction/test",
            280,
            &|| false,
        )
        .unwrap();
        let started = std::time::Instant::now();
        let mut first = None;
        let result = loop {
            if let Some(image) = preview.construction(&value, "construction/test") {
                first.get_or_insert(started.elapsed());
                if image.width >= 280 {
                    break image;
                }
            }
            assert!(
                started.elapsed().as_secs() < 5,
                "Construction preview failed: {:?}",
                preview.error()
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result.rgba.as_ref(), expected.as_raw());
        println!(
            "Construction preview: first visible {:?}, final {:?}",
            first.unwrap(),
            started.elapsed()
        );
    }
    #[test]
    fn procedural_effect_collections_persist_atomically_and_preview_without_editing_projects() {
        let root = crate::test_output::root().join(format!("effect-library-{}", new_id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("editor.json");
        let mut host = Host::new(egui::Context::default());
        host.editor_state = crate::editor_state::EditorState::open(path.clone()).unwrap();
        let before = host.editor.document().clone();
        for kind in [EffectKind::Explosion, EffectKind::Particle] {
            let mut preset =
                EffectPreset::from_layer(kind, &json!({}), format!("Saved {}", kind.layer_kind()))
                    .unwrap();
            if kind == EffectKind::Explosion {
                preset.layer["explosion"]["gradient"]["kind"] = json!("SDF");
                preset.layer["explosion"]["deltoid"]["offsetMin"] = json!(-1.5);
                preset.layer["explosion"]["count"] = json!(128);
                preset.layer["explosion"]["shapeX"] = json!(1.5);
                preset.layer["explosion"]["shapeY"] = json!(2.0);
                preset.layer["explosion"]["bottomPower"] = json!(3.0);
                preset.layer["explosion"]["bottomStretch"] = json!(0.6);
                preset.layer["explosion"]["bottomCurve"] = json!("Logarithmic");
                preset.layer["explosion"]["deltoid"]["ellipse"] = json!(0.5);
                preset.layer["explosion"]["radialBlur"] = json!(0.1);
                preset.layer["explosion"]["angularBlur"] = json!(0.1);
            } else {
                preset.layer["particles"]["colorMap"] =
                    json!(achimeld_application::gradient_map::GradientMap::default());
            }
            preset.layer["offsetX"] = json!(15.0);
            host.editor_state.effect(preset.clone()).unwrap();
            let exported = root.join(format!("library.{}", kind.extension()));
            export(kind, std::slice::from_ref(&preset), &exported).unwrap();
            let original = std::fs::read(&exported).unwrap();
            assert!(export(kind, &[], &exported).is_err());
            assert_eq!(std::fs::read(&exported).unwrap(), original);
            host.import_effects(kind, &exported).unwrap();
            let imported = host.effect_presets().last().unwrap();
            assert_eq!(imported.layer, preset.layer);
            assert_ne!(imported.id, preset.id);
            let mut preview = EffectPreview::new(egui::Context::default()).unwrap();
            let started = std::time::Instant::now();
            let mut first = None;
            let mut final_image = None;
            while started.elapsed().as_secs_f32() < 5.0 {
                if let Some(image) = preview.preview(&preset) {
                    first.get_or_insert(started.elapsed());
                    if image.width == 256 {
                        final_image = Some(image);
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                final_image.is_some(),
                "Missing effect preview: {:?}",
                preview.error()
            );
            println!(
                "{} preview: first visible {:?}, final {:?}",
                kind.layer_kind(),
                first.unwrap(),
                started.elapsed()
            );
        }
        let restored = crate::editor_state::EditorState::open(path).unwrap();
        assert_eq!(
            restored.state.effect_presets,
            host.editor_state.state.effect_presets
        );
        let state = host.editor_state.state.effect_presets.clone();
        let wrong = root.join("wrong.achexplosions");
        std::fs::write(&wrong, b"{invalid").unwrap();
        assert!(host.import_effects(EffectKind::Explosion, &wrong).is_err());
        let huge = root.join("huge.achexplosions");
        std::fs::File::create(&huge)
            .unwrap()
            .set_len(MAX_COLLECTION_BYTES as u64 + 1)
            .unwrap();
        assert!(host.import_effects(EffectKind::Explosion, &huge).is_err());
        assert_eq!(host.editor_state.state.effect_presets, state);
        assert_eq!(host.editor.document(), &before);
    }
}
