//! Portable, deterministic particle recipes. Coordinates are relative to layer space;
//! diameters use the editor's 512-unit artwork space. No clock or filesystem input.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[path = "particle_shapes.rs"]
mod shapes;
pub use shapes::ParticleShape;

pub const MAX_PARTICLES: u32 = 512;
pub const MAX_PRESET_BYTES: u64 = 262_144;

#[derive(Clone)]
pub struct PresetTarget {
    pub project_id: String,
    pub revision: u64,
    pub achievement_id: String,
    pub layer_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleSettings {
    pub count: u32,
    pub seed: u32,
    pub different_seed_per_instance: bool,
    pub shape: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<ParticleShape>,
    pub vertex_count: u32,
    pub inner_radius: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_radius_min: Option<f64>,
    /// Fixed multipliers for the complete particle shape, after random sizing.
    pub shape_x: f64,
    pub shape_y: f64,
    /// 0 = evenly distributed; 1 = legacy independent random placement.
    pub regularity: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub radius_min: f64,
    pub edge_bias: f64,
    pub size_min: f64,
    pub size_max: f64,
    pub rotation_min: f64,
    pub rotation_max: f64,
    pub opacity_min: f64,
    pub opacity_max: f64,
    pub softness: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub softness_min: Option<f64>,
    pub color_mode: String,
    pub gradient: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_map: Option<crate::gradient_map::GradientMap>,
}
impl Default for ParticleSettings {
    fn default() -> Self {
        Self {
            count: 48,
            seed: 1,
            different_seed_per_instance: false,
            shape: "Sphere".into(),
            shapes: Vec::new(),
            vertex_count: 6,
            inner_radius: 0.45,
            inner_radius_min: None,
            shape_x: 1.0,
            shape_y: 1.0,
            regularity: 1.0,
            center_x: 0.5,
            center_y: 0.5,
            radius: 0.42,
            radius_min: 0.0,
            edge_bias: 0.5,
            size_min: 8.0,
            size_max: 40.0,
            rotation_min: -180.0,
            rotation_max: 180.0,
            opacity_min: 0.35,
            opacity_max: 1.0,
            softness: 0.3,
            softness_min: None,
            color_mode: "Particle".into(),
            gradient: json!({"kind":"Radial","angle":90.0,"stops":[
                {"offset":0.0,"colorHex":"#FFFFFFFF"},
                {"offset":0.45,"colorHex":"#FFFFCF70"},
                {"offset":1.0,"colorHex":"#00FF7040"}]}),
            color_map: None,
        }
    }
}
impl ParticleSettings {
    pub fn components(&self) -> Vec<ParticleShape> {
        if self.shapes.is_empty() {
            vec![ParticleShape {
                kind: self.shape.clone(),
                vertex_count: self.vertex_count,
                inner_radius: self.inner_radius,
                inner_radius_min: self.inner_radius_min,
                ..Default::default()
            }]
        } else {
            self.shapes.clone()
        }
    }
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let settings: Self = if value.is_null() {
            Self::default()
        } else {
            serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid particle settings: {e}"))?
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(map) = &self.color_map {
            map.validate()?;
        }
        if self.shapes.len() > 16 {
            return Err("A particle may contain at most 16 component shapes".into());
        }
        for shape in &self.shapes {
            shape.validate()?;
        }
        if self.count > MAX_PARTICLES
            || !(3..=16).contains(&self.vertex_count)
            || !matches!(self.shape.as_str(), "Sphere" | "Polygon" | "Star")
            || !matches!(
                self.color_mode.as_str(),
                "Particle" | "Spawn radius" | "Random"
            )
        {
            return Err("Invalid particle count, shape or color placement".into());
        }
        for (name, value, min, max) in [
            ("Center X", self.center_x, 0.0, 1.0),
            ("Center Y", self.center_y, 0.0, 1.0),
            ("Spawn radius", self.radius, 0.0, 1.0),
            ("Edge bias", self.edge_bias, 0.0, 1.0),
            ("Minimum spawn radius", self.radius_min, 0.0, self.radius),
            ("Inner radius", self.inner_radius, 0.05, 1.0),
            (
                "Minimum inner radius",
                self.inner_radius_min.unwrap_or(self.inner_radius),
                0.05,
                self.inner_radius,
            ),
            ("Shape X", self.shape_x, 1.0, 10.0),
            ("Shape Y", self.shape_y, 1.0, 10.0),
            ("Regularity", self.regularity, 0.0, 1.0),
            ("Minimum size", self.size_min, 1.0, 512.0),
            ("Maximum size", self.size_max, 1.0, 512.0),
            ("Minimum rotation", self.rotation_min, -360.0, 360.0),
            ("Maximum rotation", self.rotation_max, -360.0, 360.0),
            ("Minimum opacity", self.opacity_min, 0.0, 1.0),
            ("Maximum opacity", self.opacity_max, 0.0, 1.0),
            ("Softness", self.softness, 0.0, 1.0),
            (
                "Minimum softness",
                self.softness_min.unwrap_or(self.softness),
                0.0,
                self.softness,
            ),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{name} must be between {min} and {max}"));
            }
        }
        if self.size_min > self.size_max
            || self.rotation_min > self.rotation_max
            || self.opacity_min > self.opacity_max
        {
            return Err("Particle minimums must not exceed their maximums".into());
        }
        let stops = self.gradient["stops"]
            .as_array()
            .ok_or("Particle gradient requires color stops")?;
        if !(2..=128).contains(&stops.len())
            || !matches!(
                self.gradient["kind"].as_str(),
                Some("Linear" | "Radial" | "Polygon" | "Star")
            )
        {
            return Err("Invalid particle gradient".into());
        }
        if !self.gradient["angle"].is_null()
            && self.gradient["angle"]
                .as_f64()
                .is_none_or(|v| !v.is_finite() || v.abs() > 36000.0)
        {
            return Err("Invalid particle gradient angle".into());
        }
        for stop in stops {
            if stop["offset"]
                .as_f64()
                .is_none_or(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                || stop["colorHex"].as_str().is_none_or(|s| {
                    !s.starts_with('#')
                        || ![7, 9].contains(&s.len())
                        || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                })
            {
                return Err("Invalid particle gradient stop".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub diameter: f64,
    pub rotation: f64,
    pub opacity: f64,
    pub radial: f64,
    pub color: f64,
    pub softness: f64,
    pub slice: f64,
    shape_seed: u64,
}
impl Particle {
    /// A separate stream varies star profiles without changing placement or color.
    pub fn component_inner_radius(&self, shape: &ParticleShape, index: usize) -> f64 {
        let low = shape.inner_radius_min.unwrap_or(shape.inner_radius);
        let mut state = self.shape_seed.wrapping_add(index as u64);
        low + next_random(&mut state) * (shape.inner_radius - low)
    }
}
/// A specified integer PRNG keeps the same layout across machines and builds.
pub fn layout(settings: &ParticleSettings, aspect: f64) -> Vec<Particle> {
    layout_for_instance(settings, aspect, "")
}

pub fn layout_for_instance(
    settings: &ParticleSettings,
    aspect: f64,
    achievement_id: &str,
) -> Vec<Particle> {
    let mut state = u64::from(settings.seed);
    if settings.different_seed_per_instance {
        state ^= 0xcbf29ce484222325;
        for byte in achievement_id.bytes() {
            state = (state ^ u64::from(byte.to_ascii_lowercase())).wrapping_mul(0x100000001b3);
        }
    }
    // A separate stream keeps legacy geometry unchanged. Stratification guarantees
    // distinct random slices while covering the map even with a small particle count.
    let count = settings.count.min(MAX_PARTICLES) as usize;
    let mut slice_state = state ^ 0x8d58ac26afe12e47;
    let mut strata: Vec<_> = (0..count).collect();
    for i in (1..count).rev() {
        let j = (next_random(&mut slice_state) * (i + 1) as f64) as usize;
        strata.swap(i, j);
    }
    let slices: Vec<_> = strata
        .into_iter()
        .map(|i| (i as f64 + next_random(&mut slice_state)) / count as f64)
        .collect();
    let mut random = || next_random(&mut state);
    (0..settings.count.min(MAX_PARTICLES))
        .map(|index| {
            let random_angle = random() * std::f64::consts::TAU;
            let even_angle = if settings.edge_bias == 1.0 || settings.radius_min == settings.radius
            {
                f64::from(index) * std::f64::consts::TAU / count as f64
            } else {
                (f64::from(index) * std::f64::consts::PI * (3.0 - 5.0_f64.sqrt()))
                    .rem_euclid(std::f64::consts::TAU)
            };
            let angle = if settings.regularity == 1.0 {
                random_angle
            } else {
                let delta = (random_angle - even_angle + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                even_angle + settings.regularity * delta
            };
            // 0.5 is uniform area; zero concentrates at center; one is the circle edge.
            let exponent = if settings.edge_bias <= 0.5 {
                2.0 - 3.0 * settings.edge_bias
            } else {
                1.0 - settings.edge_bias
            };
            let inner = if settings.radius > 0.0 {
                settings.radius_min / settings.radius
            } else {
                0.0
            };
            let random_radius = random();
            let even_radius = (f64::from(index) + 0.5) / count as f64;
            let radius_sample = if settings.regularity == 1.0 {
                random_radius
            } else {
                even_radius + settings.regularity * (random_radius - even_radius)
            };
            let radial =
                (inner * inner + radius_sample.powf(2.0 * exponent) * (1.0 - inner * inner)).sqrt();
            let radius = settings.radius * radial;
            Particle {
                x: settings.center_x + angle.cos() * radius / aspect.max(1.0),
                y: settings.center_y + angle.sin() * radius * aspect.min(1.0),
                diameter: settings.size_min + random() * (settings.size_max - settings.size_min),
                rotation: settings.rotation_min
                    + random() * (settings.rotation_max - settings.rotation_min),
                opacity: settings.opacity_min
                    + random() * (settings.opacity_max - settings.opacity_min),
                radial,
                color: random(),
                softness: settings.softness_min.unwrap_or(settings.softness)
                    + random()
                        * (settings.softness - settings.softness_min.unwrap_or(settings.softness)),
                slice: slices[index as usize],
                shape_seed: slice_state
                    .wrapping_add(u64::from(index))
                    .wrapping_add(0x52b4a9817ac360df),
            }
        })
        .collect()
}

fn next_random(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((z ^ (z >> 31)) >> 11) as f64 / ((1_u64 << 53) as f64)
}

pub fn presets() -> Vec<(&'static str, ParticleSettings)> {
    let glow = ParticleSettings {
        color_map: Some(crate::gradient_map::presets().remove(2).1),
        ..Default::default()
    };
    let stars = ParticleSettings {
        shape: "Star".into(),
        vertex_count: 5,
        inner_radius: 0.4,
        softness: 0.04,
        count: 36,
        size_min: 6.0,
        size_max: 28.0,
        ..glow.clone()
    };
    let confetti = ParticleSettings {
        shape: "Polygon".into(),
        vertex_count: 6,
        softness: 0.0,
        count: 72,
        size_min: 6.0,
        size_max: 20.0,
        color_mode: "Random".into(),
        color_map: Some(crate::gradient_map::presets().remove(3).1),
        gradient: json!({"kind":"Linear","angle":90.0,"stops":[
            {"offset":0.0,"colorHex":"#FFFF5F91"},{"offset":0.5,"colorHex":"#FFFFD966"},
            {"offset":1.0,"colorHex":"#FF50DFD0"}]}),
        ..glow.clone()
    };
    let ring = ParticleSettings {
        edge_bias: 0.95,
        radius: 0.38,
        count: 96,
        size_min: 3.0,
        size_max: 15.0,
        ..glow.clone()
    };
    vec![
        ("Soft glow", glow),
        ("Star field", stars),
        ("Confetti", confetti),
        ("Ember ring", ring),
    ]
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticlePreset {
    pub format: String,
    pub schema_version: u32,
    pub particles: ParticleSettings,
}
impl ParticlePreset {
    pub fn encode(settings: &Value) -> Result<Vec<u8>, String> {
        let preset = Self {
            format: "com.achimeld.particles".into(),
            schema_version: 1,
            particles: ParticleSettings::from_value(settings)?,
        };
        serde_json::to_vec_pretty(&preset).map_err(|e| e.to_string())
    }
    pub fn decode(bytes: &[u8]) -> Result<Value, String> {
        if bytes.len() as u64 > MAX_PRESET_BYTES {
            return Err("Particle preset exceeds the file size limit".into());
        }
        let preset: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if preset.format != "com.achimeld.particles" || preset.schema_version != 1 {
            return Err("Unsupported particle preset format".into());
        }
        preset.particles.validate()?;
        serde_json::to_value(preset.particles).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_layout_is_stable_bounded_and_respects_distribution_and_aspect() {
        let mut settings = ParticleSettings {
            count: 512,
            size_min: 40.0,
            size_max: 100.0,
            ..Default::default()
        };
        let first = layout(&settings, 2.0);
        let again = layout(&settings, 2.0);
        assert_eq!(first.len(), 512);
        for (a, b) in first.iter().zip(&again) {
            assert_eq!(
                (a.x, a.y, a.diameter, a.rotation, a.opacity, a.color),
                (b.x, b.y, b.diameter, b.rotation, b.opacity, b.color)
            );
            assert!((40.0..=100.0).contains(&a.diameter));
            assert!((settings.rotation_min..=settings.rotation_max).contains(&a.rotation));
            assert!((settings.opacity_min..=settings.opacity_max).contains(&a.opacity));
            assert!(((a.x - 0.5) * 2.0).hypot(a.y - 0.5) <= settings.radius + 1e-12);
        }
        settings.seed += 1;
        assert_ne!(first[0].x, layout(&settings, 2.0)[0].x);
        settings.edge_bias = 1.0;
        for p in layout(&settings, 0.5) {
            assert!(((p.x - 0.5).hypot((p.y - 0.5) * 2.0) - settings.radius).abs() < 1e-12);
        }
        settings.edge_bias = 0.0;
        let center = layout(&settings, 1.0).iter().map(|p| p.radial).sum::<f64>();
        settings.edge_bias = 0.5;
        let disk = layout(&settings, 1.0).iter().map(|p| p.radial).sum::<f64>();
        assert!(center < disk * 0.65);
    }

    #[test]
    fn particle_presets_round_trip_and_reject_invalid_unbounded_inputs() {
        for (_, settings) in presets() {
            settings.validate().unwrap();
            let value = json!(settings);
            assert_eq!(
                ParticlePreset::decode(&ParticlePreset::encode(&value).unwrap()).unwrap(),
                value
            );
        }
        for (key, value) in [
            ("count", json!(513)),
            ("seed", json!(-1)),
            ("sizeMin", json!(200)),
            ("softness", json!(2)),
            ("shape", json!("Unknown")),
            ("centerX", json!(-0.1)),
            ("vertexCount", json!(1000)),
            ("opacityMin", json!(2)),
            ("rotationMin", json!(999)),
        ] {
            let mut settings = json!(ParticleSettings::default());
            settings[key] = value;
            assert!(ParticleSettings::from_value(&settings).is_err(), "{key}");
        }
        let valid = ParticlePreset::encode(&json!(ParticleSettings::default())).unwrap();
        let mut malformed = json!(ParticleSettings::default());
        malformed["gradient"]["angle"] = json!("not a number");
        assert!(ParticleSettings::from_value(&malformed).is_err());
        let mut unsupported: Value = serde_json::from_slice(&valid).unwrap();
        unsupported["schemaVersion"] = json!(2);
        assert!(ParticlePreset::decode(&serde_json::to_vec(&unsupported).unwrap()).is_err());
        assert!(ParticlePreset::decode(&vec![b' '; MAX_PRESET_BYTES as usize + 1]).is_err());
    }

    #[test]
    fn particle_axes_regularity_and_star_ranges_round_trip_without_shifting_random_streams() {
        let legacy = ParticleSettings::from_value(&json!({"shape":"Star"})).unwrap();
        assert_eq!(
            (legacy.shape_x, legacy.shape_y, legacy.regularity),
            (1.0, 1.0, 1.0)
        );
        let mut changed = legacy.clone();
        changed.shape_x = 2.0;
        changed.shape_y = 3.0;
        changed.inner_radius_min = Some(0.1);
        let original = layout(&legacy, 1.0);
        let ranged = layout(&changed, 1.0);
        let shape = changed.components().remove(0);
        let radii: Vec<_> = ranged
            .iter()
            .map(|p| p.component_inner_radius(&shape, 0))
            .collect();
        assert!(radii.iter().all(|r| (0.1..0.45).contains(r)));
        assert_ne!(radii[0], radii[1]);
        for (a, b) in original.iter().zip(&ranged) {
            assert_eq!(
                (
                    a.x, a.y, a.diameter, a.rotation, a.opacity, a.slice, a.softness
                ),
                (
                    b.x, b.y, b.diameter, b.rotation, b.opacity, b.slice, b.softness
                )
            );
        }
        changed.regularity = 0.0;
        changed.radius_min = 0.2;
        let regular = layout(&changed, 1.0);
        for (i, p) in regular.iter().enumerate() {
            let expected = (changed.radius_min.powi(2)
                + (i as f64 + 0.5) / regular.len() as f64
                    * (changed.radius.powi(2) - changed.radius_min.powi(2)))
            .sqrt();
            assert!(
                ((p.x - changed.center_x).hypot(p.y - changed.center_y) - expected).abs() < 1e-12
            );
        }
        assert_ne!(original[0].x, regular[0].x);
        assert_eq!(original[0].diameter, regular[0].diameter);
        changed.edge_bias = 1.0;
        for (i, p) in layout(&changed, 1.0).iter().enumerate() {
            let angle = i as f64 * std::f64::consts::TAU / regular.len() as f64;
            assert!((p.x - changed.center_x - angle.cos() * changed.radius).abs() < 1e-12);
            assert!((p.y - changed.center_y - angle.sin() * changed.radius).abs() < 1e-12);
        }
        changed.shapes = vec![shape];
        let value = json!(changed);
        assert_eq!(
            ParticlePreset::decode(&ParticlePreset::encode(&value).unwrap()).unwrap(),
            value
        );
        for (key, invalid) in [
            ("shapeX", json!(0.99)),
            ("shapeY", json!(10.01)),
            ("regularity", json!(-0.01)),
            ("regularity", json!(1.01)),
            ("innerRadiusMin", json!(0.46)),
            ("innerRadiusMin", json!(0.01)),
        ] {
            let mut value = value.clone();
            value[key] = invalid;
            assert!(ParticleSettings::from_value(&value).is_err(), "{key}");
        }
        let mut invalid = value;
        invalid["shapes"][0]["innerRadiusMin"] = json!(0.9);
        assert!(ParticleSettings::from_value(&invalid).is_err());
    }
}
