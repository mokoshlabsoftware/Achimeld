//! Self-contained procedural effect libraries. Filesystem access belongs to the host.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_PRESETS: usize = 128;
pub const MAX_COLLECTION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    #[default]
    Explosion,
    Particle,
}
impl EffectKind {
    pub fn layer_kind(self) -> &'static str {
        match self {
            Self::Explosion => "Explosion",
            Self::Particle => "Particle",
        }
    }
    pub fn settings_key(self) -> &'static str {
        match self {
            Self::Explosion => "explosion",
            Self::Particle => "particles",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Explosion => "Explosion Explorer",
            Self::Particle => "Particle Explorer",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Explosion => "achexplosions",
            Self::Particle => "achparticles",
        }
    }
}
const LAYER_FIELDS: &[&str] = &[
    "kind",
    "visible",
    "opacity",
    "blendMode",
    "scale",
    "rotation",
    "offsetX",
    "offsetY",
    "effects",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectPreset {
    pub id: String,
    pub name: String,
    pub kind: EffectKind,
    pub layer: Value,
}
impl EffectPreset {
    pub fn from_layer(kind: EffectKind, layer: &Value, name: String) -> Result<Self, String> {
        let mut value = json!({"id":crate::new_id(), "kind":kind.layer_kind(), "visible":true,
            "opacity":1.0,"blendMode":"Normal","scale":1.0,"rotation":0.0,"offsetX":0.0,"offsetY":0.0,"effects":[]});
        for key in LAYER_FIELDS
            .iter()
            .copied()
            .chain(std::iter::once(kind.settings_key()))
        {
            if let Some(field) = layer.get(key).filter(|value| !value.is_null()) {
                value[key] = field.clone();
            }
        }
        value[kind.settings_key()] = match kind {
            EffectKind::Explosion => json!(crate::explosion::ExplosionSettings::from_value(
                &layer["explosion"]
            )?),
            EffectKind::Particle => json!(crate::particles::ParticleSettings::from_value(
                &layer["particles"]
            )?),
        };
        let preset = Self {
            id: crate::new_id(),
            name,
            kind,
            layer: value,
        };
        preset.validate()?;
        Ok(preset)
    }
    pub fn validate(&self) -> Result<(), String> {
        let fields = self
            .layer
            .as_object()
            .ok_or("Effect preset requires a layer")?;
        if self.layer["kind"] != self.kind.layer_kind()
            || fields.keys().any(|key| {
                key != "id"
                    && key != self.kind.settings_key()
                    && !LAYER_FIELDS.contains(&key.as_str())
            })
        {
            return Err(
                "Effect presets contain one procedural layer without external references".into(),
            );
        }
        crate::IconStyle {
            id: self.id.clone(),
            name: self.name.clone(),
            recipe: json!({"layers":[self.layer]}),
            preview_png: None,
        }
        .validate()
    }
    pub fn apply_to(&self, layer: &mut Value) -> Result<(), String> {
        self.validate()?;
        if layer["kind"] != self.kind.layer_kind() {
            return Err("Choose a matching effect layer".into());
        }
        let target = layer.as_object_mut().ok_or("Invalid target layer")?;
        for key in LAYER_FIELDS
            .iter()
            .copied()
            .filter(|key| *key != "kind")
            .chain(std::iter::once(self.kind.settings_key()))
        {
            target.remove(key);
            if let Some(value) = self.layer.get(key) {
                target.insert(key.into(), value.clone());
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Collection {
    format: String,
    schema_version: u32,
    kind: EffectKind,
    presets: Vec<EffectPreset>,
}
pub fn validate_library(presets: &[EffectPreset]) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    for kind in [EffectKind::Explosion, EffectKind::Particle] {
        if presets.iter().filter(|p| p.kind == kind).count() > MAX_PRESETS {
            return Err("Too many effect presets".into());
        }
    }
    for preset in presets {
        preset.validate()?;
        if !ids.insert(&preset.id) {
            return Err("Duplicate effect preset identity".into());
        }
    }
    Ok(())
}
pub fn encode(kind: EffectKind, presets: &[EffectPreset]) -> Result<Vec<u8>, String> {
    validate_library(presets)?;
    if presets.iter().any(|p| p.kind != kind) {
        return Err("Effect collection kinds must match".into());
    }
    let bytes = serde_json::to_vec_pretty(&Collection {
        format: "com.achimeld.effects".into(),
        schema_version: 1,
        kind,
        presets: presets.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_COLLECTION_BYTES {
        return Err("Effect collection exceeds the file size limit".into());
    }
    Ok(bytes)
}
pub fn decode(kind: EffectKind, bytes: &[u8]) -> Result<Vec<EffectPreset>, String> {
    if bytes.len() > MAX_COLLECTION_BYTES {
        return Err("Effect collection exceeds the file size limit".into());
    }
    if kind == EffectKind::Particle {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if value["format"] == "com.achimeld.particles" {
            let particles = crate::particles::ParticlePreset::decode(bytes)?;
            return Ok(vec![EffectPreset::from_layer(
                kind,
                &json!({"particles":particles}),
                "Imported particle".into(),
            )?]);
        }
    }
    let collection: Collection = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if collection.format != "com.achimeld.effects"
        || collection.schema_version != 1
        || collection.kind != kind
        || collection.presets.iter().any(|p| p.kind != kind)
    {
        return Err("Unsupported effect collection format or kind".into());
    }
    validate_library(&collection.presets)?;
    Ok(collection.presets)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn procedural_collections_embed_gradients_maps_shapes_and_layer_properties() {
        for kind in [EffectKind::Explosion, EffectKind::Particle] {
            let mut layer = json!({"id":crate::new_id(),"kind":kind.layer_kind(),"opacity":0.6,"blendMode":"Screen",
                "offsetX":31.0,"offsetY":-12.0,"rotation":37.0,"scale":1.25,"effects":[]});
            let mut settings = match kind {
                EffectKind::Explosion => json!(crate::explosion::ExplosionSettings::new_deltoid()),
                EffectKind::Particle => json!(crate::particles::ParticleSettings {
                    color_map: Some(crate::gradient_map::presets().remove(1).1),
                    ..Default::default()
                }),
            };
            settings["seed"] = json!(321);
            settings["gradient"]["stops"] = json!([{"offset":0.0,"colorHex":"#30FF0000"},{"offset":0.3,"colorHex":"#A000FF00"},{"offset":1.0,"colorHex":"#FF0000FF"}]);
            if kind == EffectKind::Explosion {
                settings["gradient"]["kind"] = json!("SDF");
                settings["deltoid"]["offsetMin"] = json!(-1.5);
                settings["deltoid"]["offsetMax"] = json!(1.5);
            }
            layer[kind.settings_key()] = settings.clone();
            let preset = EffectPreset::from_layer(kind, &layer, "Ember".into()).unwrap();
            layer[kind.settings_key()]["gradient"]["stops"][0]["colorHex"] = json!("#FFFFFFFF");
            assert_eq!(preset.layer[kind.settings_key()], settings);
            let restored =
                decode(kind, &encode(kind, std::slice::from_ref(&preset)).unwrap()).unwrap();
            assert_eq!(restored, vec![preset.clone()]);
            let target_id = crate::new_id();
            let mask = crate::new_id();
            let parent = crate::new_id();
            let mut target = json!({"id":target_id,"kind":kind.layer_kind(),"displayName":"Keep name","maskLayerId":mask,"parentFolderId":parent});
            restored[0].apply_to(&mut target).unwrap();
            assert_eq!(target["id"], target_id);
            assert_eq!(target["maskLayerId"], mask);
            assert_eq!(target["parentFolderId"], parent);
            assert_eq!(target["displayName"], "Keep name");
            assert_eq!(target[kind.settings_key()], settings);
            for key in [
                "opacity",
                "blendMode",
                "offsetX",
                "offsetY",
                "rotation",
                "scale",
                "effects",
            ] {
                assert_eq!(target[key], preset.layer[key]);
            }
            let other = if kind == EffectKind::Explosion {
                EffectKind::Particle
            } else {
                EffectKind::Explosion
            };
            assert!(decode(other, &encode(kind, &[preset]).unwrap()).is_err());
        }
    }
    #[test]
    fn procedural_collection_validation_rejects_external_references_limits_and_partial_payloads() {
        let preset =
            EffectPreset::from_layer(EffectKind::Explosion, &json!({}), "Test".into()).unwrap();
        let bytes = encode(EffectKind::Explosion, std::slice::from_ref(&preset)).unwrap();
        for field in ["schemaVersion", "format", "kind"] {
            let mut value: Value = serde_json::from_slice(&bytes).unwrap();
            value[field] = json!("unsupported");
            assert!(decode(EffectKind::Explosion, &serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut bad = preset.clone();
        bad.layer["imageId"] = json!(crate::new_id());
        assert!(bad.validate().is_err());
        let mut bad = preset.clone();
        bad.layer["explosion"]["count"] = json!(513);
        assert!(bad.validate().is_err());
        assert!(validate_library(&[preset.clone(), preset.clone()]).is_err());
        assert!(validate_library(&vec![preset; MAX_PRESETS + 1]).is_err());
        assert!(decode(EffectKind::Explosion, &vec![b' '; MAX_COLLECTION_BYTES + 1]).is_err());
        assert!(decode(EffectKind::Explosion, &bytes[..bytes.len() / 2]).is_err());
        let legacy = crate::particles::ParticlePreset::encode(&json!(
            crate::particles::ParticleSettings::default()
        ))
        .unwrap();
        assert_eq!(decode(EffectKind::Particle, &legacy).unwrap().len(), 1);
        assert!(decode(EffectKind::Explosion, &legacy).is_err());
    }
}
