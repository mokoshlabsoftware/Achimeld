//! Bounded shape recipes shared by shape layers and particle composites.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Layer,
    Particle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShapePreset {
    pub id: String,
    pub name: String,
    pub kind: ShapeKind,
    pub shape: Value,
}
impl ShapePreset {
    pub fn from_layer(kind: ShapeKind, layer: &Value, name: String) -> Result<Self, String> {
        let shape = match kind {
            ShapeKind::Layer => layer["shape"].clone(),
            ShapeKind::Particle => json!(
                crate::particles::ParticleSettings::from_value(&layer["particles"])?.components()
            ),
        };
        let preset = Self {
            id: crate::new_id(),
            name,
            kind,
            shape,
        };
        preset.validate()?;
        Ok(preset)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !crate::document::valid_id(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 128
        {
            return Err("Invalid shape preset identity or name".into());
        }
        match self.kind {
            ShapeKind::Layer => {
                let shape = self
                    .shape
                    .as_object()
                    .ok_or("Shape preset requires settings")?;
                if shape.keys().any(|key| {
                    ![
                        "kind",
                        "colorHex",
                        "scaleX",
                        "scaleY",
                        "vertexCount",
                        "innerRadius",
                        "rotation",
                    ]
                    .contains(&key.as_str())
                }) {
                    return Err("Shape preset contains unsupported fields".into());
                }
                let kind = self.shape["kind"]
                    .as_str()
                    .ok_or("Shape kind is required")?;
                if !["Rectangle", "Sphere", "Polygon", "Star"].contains(&kind) {
                    return Err("Unsupported shape kind".into());
                }
                for (key, min, max) in [
                    ("scaleX", 0.01, 2.0),
                    ("scaleY", 0.01, 2.0),
                    ("innerRadius", 0.01, 2.0),
                    ("rotation", -360.0, 360.0),
                ] {
                    if let Some(value) = self.shape.get(key)
                        && !value
                            .as_f64()
                            .is_some_and(|v| v.is_finite() && (min..=max).contains(&v))
                    {
                        return Err("Invalid shape value".into());
                    }
                }
                if let Some(vertices) = self.shape.get("vertexCount")
                    && !vertices.as_u64().is_some_and(|n| (3..=64).contains(&n))
                {
                    return Err("Invalid shape vertices".into());
                }
                if let Some(color) = self.shape.get("colorHex") {
                    let hex = color
                        .as_str()
                        .and_then(|s| s.strip_prefix('#'))
                        .ok_or("Invalid shape color")?;
                    if ![6, 8].contains(&hex.len()) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err("Invalid shape color".into());
                    }
                }
                if self.shape.to_string().len() > 4096 {
                    return Err("Shape preset is too large".into());
                }
            }
            ShapeKind::Particle => {
                let shapes: Vec<crate::particles::ParticleShape> =
                    serde_json::from_value(self.shape.clone()).map_err(|e| e.to_string())?;
                if shapes.is_empty() {
                    return Err("Shape preset requires a component".into());
                }
                let settings = crate::particles::ParticleSettings {
                    shapes,
                    ..Default::default()
                };
                settings.validate()?;
            }
        }
        Ok(())
    }
    pub fn apply_to(&self, layer: &mut Value) -> Result<(), String> {
        self.validate()?;
        match self.kind {
            ShapeKind::Layer if layer["kind"] == "Shape" => layer["shape"] = self.shape.clone(),
            ShapeKind::Particle if layer["kind"] == "Particle" => {
                let mut settings =
                    crate::particles::ParticleSettings::from_value(&layer["particles"])?;
                settings.shapes =
                    serde_json::from_value(self.shape.clone()).map_err(|e| e.to_string())?;
                settings.validate()?;
                layer["particles"] = json!(settings);
            }
            _ => return Err("Choose a matching shape layer".into()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shape_presets_apply_only_matching_shape_fields() {
        let mut shape = json!({"id":crate::new_id(),"kind":"Shape","shape":{"kind":"Star","colorHex":"#FFAA4400","scaleX":0.4,"scaleY":0.7,"vertexCount":5,"innerRadius":0.5},"opacity":0.6});
        let preset = ShapePreset::from_layer(ShapeKind::Layer, &shape, "Star".into()).unwrap();
        shape["shape"]["kind"] = json!("Sphere");
        preset.apply_to(&mut shape).unwrap();
        assert_eq!(shape["shape"], preset.shape);
        assert_eq!(shape["opacity"], 0.6);
        let mut particle = json!({"kind":"Particle","particles":json!(crate::particles::ParticleSettings::default())});
        assert!(preset.apply_to(&mut particle).is_err());
        let source =
            ShapePreset::from_layer(ShapeKind::Particle, &particle, "Composite".into()).unwrap();
        particle["particles"]["count"] = json!(123);
        source.apply_to(&mut particle).unwrap();
        assert_eq!(particle["particles"]["count"], 123);
        assert_eq!(particle["particles"]["shapes"], source.shape);
        let mut invalid = source.clone();
        invalid.shape = json!([]);
        assert!(invalid.validate().is_err());
    }
}
