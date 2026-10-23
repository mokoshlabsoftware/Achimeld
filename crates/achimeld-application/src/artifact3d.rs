//! Portable, bounded collections of editable 3D primitives in 512-unit icon space.
use crate::text3d::{TextProjection, TextShading};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_PARTS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Primitive {
    #[default]
    Cube,
    Cone,
    Torus,
    Cylinder,
    Sphere,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactOutput {
    #[default]
    FullColor,
    AlphaMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ArtifactPart {
    pub id: String,
    pub name: String,
    pub primitive: Primitive,
    pub visible: bool,
    pub size: [f64; 3],
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub color_hex: String,
    pub shading: TextShading,
    pub roughness: f64,
    pub reflectivity: f64,
    /// Zero is sharp; one is an ellipsoid. Small values begin as a chamfer.
    pub rounding: f64,
    pub start_angle: f64,
    pub sweep_angle: f64,
    /// Sphere cuts in normalized local Y, before scaling/rotation.
    pub cut_bottom: f64,
    pub cut_top: f64,
    /// Zero is solid; positive values make a hollow sphere with a closed rim.
    pub wall_thickness: f64,
    /// Torus tube radius relative to the outside ring radius.
    pub tube_radius: f64,
}

impl Default for ArtifactPart {
    fn default() -> Self {
        Self {
            id: "00000000-0000-4000-8000-000000000001".into(),
            name: "Cube".into(),
            primitive: Primitive::Cube,
            visible: true,
            size: [120.0; 3],
            position: [0.0; 3],
            rotation: [0.0; 3],
            color_hex: "#FFD1D9E8".into(),
            shading: TextShading::Glossy,
            roughness: 0.3,
            reflectivity: 0.55,
            rounding: 0.0,
            start_angle: 0.0,
            sweep_angle: 360.0,
            cut_bottom: -1.0,
            cut_top: 1.0,
            wall_thickness: 0.0,
            tube_radius: 0.22,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ArtifactSettings {
    pub parts: Vec<ArtifactPart>,
    pub output: ArtifactOutput,
    pub projection: TextProjection,
    pub camera_distance: f64,
    pub focal_length: f64,
    pub orthographic_size: f64,
    pub rotation: [f64; 3],
    pub position: [f64; 3],
    pub ambient: f64,
    pub light_intensity: f64,
    pub light_azimuth: f64,
    pub light_elevation: f64,
}

impl Default for ArtifactSettings {
    fn default() -> Self {
        Self {
            parts: preset(ArtifactPreset::Person),
            output: ArtifactOutput::FullColor,
            projection: TextProjection::Perspective,
            camera_distance: 800.0,
            focal_length: 50.0,
            orthographic_size: 512.0,
            rotation: [12.0, -20.0, 0.0],
            position: [0.0; 3],
            ambient: 0.3,
            light_intensity: 0.8,
            light_azimuth: -35.0,
            light_elevation: 45.0,
        }
    }
}

impl ArtifactSettings {
    pub fn from_value(value: &Value) -> Result<Self, String> {
        if !value.is_null() && !value.is_object() {
            return Err("3D artifact settings must be an object".into());
        }
        let settings: Self = if value.is_null() {
            Self::default()
        } else {
            serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid 3D artifact: {e}"))?
        };
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), String> {
        bounded(self.camera_distance, 32.0, 8192.0)?;
        bounded(self.focal_length, 10.0, 300.0)?;
        bounded(self.orthographic_size, 32.0, 8192.0)?;
        vector(self.rotation, -360.0, 360.0)?;
        vector(self.position, -4096.0, 4096.0)?;
        bounded(self.ambient, 0.0, 1.0)?;
        bounded(self.light_intensity, 0.0, 2.0)?;
        bounded(self.light_azimuth, -180.0, 180.0)?;
        bounded(self.light_elevation, -90.0, 90.0)?;
        if self.parts.len() > MAX_PARTS {
            return Err("A 3D artifact may contain at most 64 parts".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for part in &self.parts {
            if !crate::document::valid_id(&part.id)
                || part.id == "00000000-0000-0000-0000-000000000000"
                || !ids.insert(part.id.to_ascii_lowercase())
            {
                return Err("Invalid or duplicate 3D artifact part ID".into());
            }
            if part.name.chars().count() > 128 {
                return Err("3D artifact part names may contain at most 128 characters".into());
            }
            vector(part.size, 0.1, 2048.0)?;
            vector(part.position, -4096.0, 4096.0)?;
            vector(part.rotation, -360.0, 360.0)?;
            bounded(part.rounding, 0.0, 1.0)?;
            bounded(part.roughness, 0.0, 1.0)?;
            bounded(part.reflectivity, 0.0, 1.0)?;
            bounded(part.start_angle, -360.0, 360.0)?;
            bounded(part.sweep_angle, 0.5, 360.0)?;
            bounded(part.cut_bottom, -1.0, 1.0)?;
            bounded(part.cut_top, -1.0, 1.0)?;
            if part.cut_top - part.cut_bottom < 0.001 {
                return Err("The sphere's top cut must be above its bottom cut".into());
            }
            bounded(part.wall_thickness, 0.0, 0.95)?;
            bounded(part.tube_radius, 0.01, 0.95)?;
            let color = &part.color_hex;
            if !color.starts_with('#')
                || ![7, 9].contains(&color.len())
                || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err("Invalid 3D artifact material color".into());
            }
        }
        Ok(())
    }
}

fn bounded(value: f64, min: f64, max: f64) -> Result<(), String> {
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!("3D artifact value must be between {min} and {max}"));
    }
    Ok(())
}
fn vector(values: [f64; 3], min: f64, max: f64) -> Result<(), String> {
    for value in values {
        bounded(value, min, max)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPreset {
    Person,
    Cup,
    Shield,
    Sword,
    Potion,
}
pub const PRESETS: [(ArtifactPreset, &str); 5] = [
    (ArtifactPreset::Person, "Person"),
    (ArtifactPreset::Cup, "Cup"),
    (ArtifactPreset::Shield, "Shield"),
    (ArtifactPreset::Sword, "Sword"),
    (ArtifactPreset::Potion, "Potion"),
];

/// Presets are editable recipes, never baked images or hidden geometry.
pub fn preset(preset: ArtifactPreset) -> Vec<ArtifactPart> {
    use Primitive::*;
    let mut parts = Vec::new();
    let mut add = |name: &str, primitive, size, position, color: &str| {
        parts.push(ArtifactPart {
            id: format!("00000000-0000-4000-8000-{:012}", parts.len() + 1),
            name: name.into(),
            primitive,
            size,
            position,
            color_hex: color.into(),
            ..ArtifactPart::default()
        });
    };
    let gold = "#FFE5AF42";
    let steel = "#FFCCD9E8";
    match preset {
        ArtifactPreset::Person => {
            add("Head", Sphere, [82.0; 3], [0.0, 135.0, 0.0], steel);
            add(
                "Torso",
                Cylinder,
                [108.0, 140.0, 66.0],
                [0.0, 25.0, 0.0],
                "#FF4E91CF",
            );
            add(
                "Left hand",
                Cylinder,
                [36.0, 135.0, 36.0],
                [-78.0, 25.0, 0.0],
                steel,
            );
            add(
                "Right hand",
                Cylinder,
                [36.0, 135.0, 36.0],
                [78.0, 25.0, 0.0],
                steel,
            );
            add(
                "Left foot",
                Cylinder,
                [42.0, 136.0, 48.0],
                [-32.0, -108.0, 5.0],
                steel,
            );
            add(
                "Right foot",
                Cylinder,
                [42.0, 136.0, 48.0],
                [32.0, -108.0, 5.0],
                steel,
            );
            for part in &mut parts[1..] {
                part.rounding = 0.8;
            }
            parts[2].rotation[2] = -12.0;
            parts[3].rotation[2] = 12.0;
        }
        ArtifactPreset::Cup => {
            add(
                "Bowl",
                Sphere,
                [220.0, 210.0, 180.0],
                [0.0, 124.0, 0.0],
                gold,
            );
            add(
                "Base",
                Sphere,
                [160.0, 50.0, 130.0],
                [0.0, -146.0, 0.0],
                gold,
            );
            add(
                "Stem",
                Cylinder,
                [36.0, 165.0, 36.0],
                [0.0, -61.5, 0.0],
                gold,
            );
            add(
                "Left handle",
                Torus,
                [108.0, 22.0, 64.0],
                [-85.0, 84.0, 0.0],
                gold,
            );
            add(
                "Right handle",
                Torus,
                [108.0, 22.0, 64.0],
                [85.0, 84.0, 0.0],
                gold,
            );
            parts[0].cut_top = 0.0;
            parts[0].wall_thickness = 0.12;
            parts[1].cut_bottom = 0.0;
            parts[2].rounding = 0.12;
            for (i, start) in [(3, 90.0), (4, -90.0)] {
                parts[i].rotation[0] = 90.0;
                parts[i].start_angle = start;
                parts[i].sweep_angle = 180.0;
            }
        }
        ArtifactPreset::Shield => {
            add(
                "Shield face",
                Sphere,
                [240.0, 290.0, 60.0],
                [0.0, 0.0, 0.0],
                "#FF4178AF",
            );
            add(
                "Central boss",
                Sphere,
                [70.0, 70.0, 50.0],
                [0.0, 0.0, 30.0],
                gold,
            );
            add(
                "Vertical brace",
                Cube,
                [18.0, 230.0, 12.0],
                [0.0, 0.0, 30.0],
                gold,
            );
            add(
                "Horizontal brace",
                Cube,
                [184.0, 18.0, 12.0],
                [0.0, 0.0, 30.0],
                gold,
            );
            parts[2].rounding = 0.15;
            parts[3].rounding = 0.15;
        }
        ArtifactPreset::Sword => {
            add("Blade", Cube, [44.0, 212.0, 15.0], [0.0, 36.0, 0.0], steel);
            add("Tip", Cone, [44.0, 55.0, 15.0], [0.0, 169.5, 0.0], steel);
            add("Guard", Cube, [146.0, 24.0, 34.0], [0.0, -80.0, 0.0], gold);
            add(
                "Grip",
                Cylinder,
                [30.0, 88.0, 30.0],
                [0.0, -133.0, 0.0],
                "#FF715042",
            );
            add("Pommel", Sphere, [48.0; 3], [0.0, -187.0, 0.0], gold);
            parts[0].rounding = 0.1;
            parts[2].rounding = 0.25;
            parts[3].rounding = 0.15;
        }
        ArtifactPreset::Potion => {
            add(
                "Bottle",
                Sphere,
                [210.0, 240.0, 170.0],
                [0.0, -24.0, 0.0],
                "#FF729EC1",
            );
            add(
                "Potion",
                Sphere,
                [184.0, 210.0, 180.0],
                [0.0, -24.0, 5.0],
                "#FF8C48CF",
            );
            add(
                "Neck",
                Cylinder,
                [66.0, 100.0, 66.0],
                [0.0, 111.0, 0.0],
                "#FF729EC1",
            );
            add("Rim", Torus, [82.0, 18.0, 82.0], [0.0, 155.0, 0.0], gold);
            add(
                "Stopper",
                Cylinder,
                [56.0, 38.0, 56.0],
                [0.0, 178.0, 0.0],
                "#FFA87945",
            );
            parts[0].cut_top = 0.78;
            parts[1].cut_top = 0.12;
            parts[2].rounding = 0.1;
            parts[4].rounding = 0.18;
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn artifact3d_presets_defaults_and_bounds() {
        let default = ArtifactSettings::default();
        assert_eq!(ArtifactSettings::from_value(&Value::Null).unwrap(), default);
        assert_eq!(ArtifactSettings::from_value(&json!({})).unwrap(), default);
        for (kind, _) in PRESETS {
            let settings = ArtifactSettings {
                parts: preset(kind),
                ..Default::default()
            };
            let value = json!(settings);
            assert_eq!(ArtifactSettings::from_value(&value).unwrap(), settings);
        }
        let mut invalids = vec![
            json!([]),
            json!({"parts":null}),
            json!({"output":"FaceMask"}),
            json!({"rotation":[0,0]}),
            json!({"cameraDistance":0}),
        ];
        for (key, value) in [
            ("size", json!([0, 120, 120])),
            ("position", json!([0, 0, 4097])),
            ("rotation", json!([361, 0, 0])),
            ("primitive", json!("Other")),
            ("rounding", json!(1.1)),
            ("sweepAngle", json!(0)),
            ("cutTop", json!(-1)),
            ("wallThickness", json!(1)),
            ("tubeRadius", json!(0)),
            ("colorHex", json!("oops")),
            ("id", json!("invalid")),
            ("shading", json!("Other")),
        ] {
            let mut part = json!(ArtifactPart::default());
            part[key] = value;
            invalids.push(json!({"parts":[part]}));
        }
        invalids.push(json!({"parts":vec![ArtifactPart::default();65]}));
        invalids.push(json!({"parts":vec![ArtifactPart::default();2]}));
        for invalid in invalids {
            assert!(
                ArtifactSettings::from_value(&invalid).is_err(),
                "Accepted {invalid}"
            );
        }
        let mut settings = default;
        settings.parts[0].size[0] = f64::NAN;
        assert!(settings.validate().is_err());
    }
}
