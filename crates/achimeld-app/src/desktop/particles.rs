use super::*;
use achimeld_application::particles::{MAX_PRESET_BYTES, ParticlePreset, PresetTarget};

pub(super) fn save(path: &Path, settings: &Value) -> Result<(), String> {
    let bytes = ParticlePreset::encode(settings)?;
    achimeld_persistence::project::write_atomic(path, &bytes)
}

impl Host {
    fn particle_target_current(&self, target: &PresetTarget) -> bool {
        self.editor.document().project()["projectId"].as_str() == Some(&target.project_id)
            && self.editor.revision() == target.revision
            && self
                .editor
                .document()
                .entity(
                    achimeld_application::EntityKind::Achievement,
                    &target.achievement_id,
                )
                .and_then(|e| e["icon"]["layers"].as_array())
                .is_some_and(|layers| {
                    layers
                        .iter()
                        .any(|layer| layer["id"] == target.layer_id && layer["kind"] == "Particle")
                })
    }
    pub(super) fn load_particle_preset(
        &mut self,
        target: PresetTarget,
        path: &Path,
    ) -> Result<(), String> {
        if !self.particle_target_current(&target) {
            return Err("The particle layer changed while choosing the preset".into());
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PRESET_BYTES {
            return Err("Particle preset exceeds the file size limit".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_PRESET_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let settings = ParticlePreset::decode(&bytes)?;
        self.particle_preset = Some((target, settings));
        Ok(())
    }
    pub(super) fn take_loaded_particle_preset(
        &mut self,
        achievement_id: &str,
        layer_id: &str,
    ) -> Option<Value> {
        let (target, _) = self.particle_preset.as_ref()?;
        if !self.particle_target_current(target) {
            self.particle_preset = None;
            self.status = crate::desktop::logged_status(
                "The particle layer changed before the preset could be applied".into(),
            );
            return None;
        }
        if target.achievement_id != achievement_id || target.layer_id != layer_id {
            return None;
        }
        self.particle_preset.take().map(|(_, settings)| settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_preset_files_are_bounded_and_pinned_to_the_requesting_layer() {
        let mut host = Host::new(egui::Context::default());
        host.editor = Editor::default();
        host.editor
            .execute(
                "Add",
                Command::Add(achimeld_application::EntityKind::Achievement),
                None,
            )
            .unwrap();
        let mut achievement = host
            .editor
            .document()
            .entities(achimeld_application::EntityKind::Achievement)[0]
            .clone();
        let achievement_id = achievement["id"].as_str().unwrap().to_owned();
        let layer_id = new_id();
        achievement["icon"] = json!({"layers":[{"id":layer_id,"kind":"Particle"}]});
        host.editor
            .execute(
                "Recipe",
                Command::ReplaceEntity {
                    kind: achimeld_application::EntityKind::Achievement,
                    id: achievement_id.clone(),
                    value: achievement,
                },
                None,
            )
            .unwrap();
        let target = PresetTarget {
            project_id: host.editor.document().project()["projectId"]
                .as_str()
                .unwrap()
                .into(),
            revision: host.editor.revision(),
            achievement_id: achievement_id.clone(),
            layer_id: layer_id.clone(),
        };
        let folder = crate::test_output::root().join(format!("particle-preset-{}", new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("effect.achparticles");
        let settings = json!(achimeld_application::particles::ParticleSettings {
            different_seed_per_instance: true,
            size_min: 40.0,
            size_max: 100.0,
            softness_min: Some(0.1),
            color_map: Some(achimeld_application::gradient_map::presets().remove(1).1),
            ..Default::default()
        });
        save(&path, &settings).unwrap();
        host.load_particle_preset(target.clone(), &path).unwrap();
        assert!(
            host.take_loaded_particle_preset("another-achievement", &layer_id)
                .is_none()
        );
        assert_eq!(
            host.take_loaded_particle_preset(&achievement_id, &layer_id),
            Some(settings)
        );
        assert!(
            host.take_loaded_particle_preset(&achievement_id, &layer_id)
                .is_none()
        );
        host.editor
            .execute(
                "Change",
                Command::Add(achimeld_application::EntityKind::Achievement),
                None,
            )
            .unwrap();
        assert!(host.load_particle_preset(target, &path).is_err());
        let oversized = folder.join("oversized.achparticles");
        std::fs::write(&oversized, vec![b' '; MAX_PRESET_BYTES as usize + 1]).unwrap();
        let target = PresetTarget {
            revision: host.editor.revision(),
            project_id: host.editor.document().project()["projectId"]
                .as_str()
                .unwrap()
                .into(),
            achievement_id,
            layer_id,
        };
        assert!(
            host.load_particle_preset(target, &oversized)
                .unwrap_err()
                .contains("size limit")
        );
    }
}
