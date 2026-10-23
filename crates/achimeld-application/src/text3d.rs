//! Portable 3D text recipe settings. Units use the icon's 512-unit design space.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextProjection {
    #[default]
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextShading {
    Unlit,
    #[default]
    Matte,
    Glossy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Text3dOutput {
    #[default]
    FullColor,
    FullMask,
    FaceMask,
    EmbossMask,
    EmbossMaskMinusFace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Text3dSettings {
    pub projection: TextProjection,
    pub camera_distance: f64,
    pub focal_length: f64,
    pub orthographic_size: f64,
    pub extrusion: f64,
    pub rotation_x: f64,
    pub rotation_y: f64,
    pub rotation_z: f64,
    pub position_x: f64,
    pub position_y: f64,
    pub position_z: f64,
    pub shading: TextShading,
    pub side_color_hex: String,
    pub ambient: f64,
    pub light_intensity: f64,
    pub light_azimuth: f64,
    pub light_elevation: f64,
    pub roughness: f64,
    pub push: f64,
    pub emboss: f64,
    pub output: Text3dOutput,
    pub shadow_strength: f64,
    pub smooth_normals: bool,
    pub reflectivity: f64,
}
impl Default for Text3dSettings {
    fn default() -> Self {
        Self {
            projection: TextProjection::Perspective,
            camera_distance: 800.0,
            focal_length: 50.0,
            orthographic_size: 512.0,
            extrusion: 32.0,
            rotation_x: 15.0,
            rotation_y: -25.0,
            rotation_z: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            shading: TextShading::Matte,
            side_color_hex: "#FFB8C4D8".into(),
            ambient: 0.3,
            light_intensity: 0.8,
            light_azimuth: -35.0,
            light_elevation: 45.0,
            roughness: 0.35,
            push: 0.0,
            emboss: 0.0,
            output: Text3dOutput::FullColor,
            shadow_strength: 0.0,
            smooth_normals: false,
            reflectivity: 0.55,
        }
    }
}
impl Text3dSettings {
    pub fn from_value(value: &Value) -> Result<Self, String> {
        if !value.is_null() && !value.is_object() {
            return Err("3D text settings must be an object".into());
        }
        let settings: Self = if value.is_null() {
            Self::default()
        } else {
            serde_json::from_value(value.clone()).map_err(|e| format!("Invalid 3D text: {e}"))?
        };
        for (name, value, min, max) in [
            ("camera distance", settings.camera_distance, 32.0, 8192.0),
            ("focal length", settings.focal_length, 10.0, 300.0),
            (
                "orthographic size",
                settings.orthographic_size,
                32.0,
                8192.0,
            ),
            ("extrusion", settings.extrusion, 0.0, 512.0),
            ("push", settings.push, -64.0, 64.0),
            ("emboss", settings.emboss, 0.0, 64.0),
            ("shadow strength", settings.shadow_strength, 0.0, 1.0),
            ("rotation X", settings.rotation_x, -360.0, 360.0),
            ("rotation Y", settings.rotation_y, -360.0, 360.0),
            ("rotation Z", settings.rotation_z, -360.0, 360.0),
            ("position X", settings.position_x, -4096.0, 4096.0),
            ("position Y", settings.position_y, -4096.0, 4096.0),
            ("position Z", settings.position_z, -4096.0, 4096.0),
            ("ambient light", settings.ambient, 0.0, 1.0),
            ("light intensity", settings.light_intensity, 0.0, 2.0),
            ("light azimuth", settings.light_azimuth, -180.0, 180.0),
            ("light elevation", settings.light_elevation, -90.0, 90.0),
            ("roughness", settings.roughness, 0.0, 1.0),
            ("reflectivity", settings.reflectivity, 0.0, 1.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("3D text {name} must be between {min} and {max}"));
            }
        }
        let color = &settings.side_color_hex;
        if !color.starts_with('#')
            || ![7, 9].contains(&color.len())
            || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err("Invalid 3D text side color".into());
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text3d_defaults_and_recipe_settings_are_portable_and_bounded() {
        let settings = Text3dSettings::default();
        assert_eq!(Text3dSettings::from_value(&Value::Null).unwrap(), settings);
        assert_eq!(Text3dSettings::from_value(&json!({})).unwrap(), settings);
        assert_eq!(
            Text3dSettings::from_value(&serde_json::to_value(&settings).unwrap()).unwrap(),
            settings
        );
        for invalid in [
            json!([]),
            json!({"cameraDistance":0}),
            json!({"focalLength":301}),
            json!({"extrusion":-1}),
            json!({"push":-65}),
            json!({"push":65}),
            json!({"emboss":-1}),
            json!({"emboss":65}),
            json!({"output":"Other"}),
            json!({"shadowStrength":1.1}),
            json!({"smoothNormals":1}),
            json!({"rotationY":361}),
            json!({"positionZ":4097}),
            json!({"projection":"Unknown"}),
            json!({"shading":"Unknown"}),
            json!({"ambient":2}),
            json!({"roughness":-1}),
            json!({"reflectivity":-0.01}),
            json!({"reflectivity":1.01}),
            json!({"lightIntensity":null}),
            json!({"sideColorHex":"bad"}),
            json!({"cameraDistance":"800"}),
        ] {
            assert!(
                Text3dSettings::from_value(&invalid).is_err(),
                "Accepted {invalid}"
            );
        }
    }
}
