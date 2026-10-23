//! Deterministic strip-space deltoids, subsequently mapped into polar space.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_OBJECTS: u32 = 512;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottomCurve {
    #[default]
    Linear,
    Quadratic,
    Logarithmic,
}
impl BottomCurve {
    pub fn width_scale(self, power: f64, stretch: f64, y: f64) -> f64 {
        if power == 0.0 || stretch == 0.0 {
            return 1.0;
        }
        let depth = (1.0 - (1.0 - y) / stretch).clamp(0.0, 1.0);
        1.0 + power * self.sample(depth)
    }

    pub fn sample(self, depth: f64) -> f64 {
        let depth = depth.clamp(0.0, 1.0);
        match self {
            Self::Linear => depth,
            Self::Quadratic => depth * depth,
            Self::Logarithmic => (9.0 * depth).ln_1p() / 10.0_f64.ln(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct DeltoidSettings {
    /// 0 = straight deltoid edges; 1 = an ellipse with the same overall bounds.
    pub ellipse: f64,
    pub scale: f64,
    pub scale_min: f64,
    pub scale_max: f64,
    pub width_min: f64,
    pub width_max: f64,
    pub top_height_min: f64,
    pub top_height_max: f64,
    pub bottom_height_min: f64,
    pub bottom_height_max: f64,
    pub offset_min: f64,
    pub offset_max: f64,
    pub sharpness_min: f64,
    pub sharpness_max: f64,
}
impl Default for DeltoidSettings {
    fn default() -> Self {
        Self {
            ellipse: 0.0,
            scale: 1.0,
            scale_min: 1.0,
            scale_max: 1.0,
            width_min: 0.12,
            width_max: 0.35,
            top_height_min: 0.2,
            top_height_max: 0.45,
            bottom_height_min: 0.2,
            bottom_height_max: 0.45,
            offset_min: 0.0,
            offset_max: 0.0,
            sharpness_min: 0.8,
            sharpness_max: 1.0,
        }
    }
}
impl DeltoidSettings {
    fn validate(&self) -> Result<(), String> {
        for (name, low, high, min, max) in [
            ("Deltoid to ellipse", self.ellipse, self.ellipse, 0.0, 1.0),
            ("Deltoid global scale", self.scale, self.scale, 0.01, 2.0),
            (
                "Deltoid random scale",
                self.scale_min,
                self.scale_max,
                0.01,
                2.0,
            ),
            ("Deltoid width", self.width_min, self.width_max, 0.0, 2.0),
            (
                "Top vertex height",
                self.top_height_min,
                self.top_height_max,
                0.0,
                1.0,
            ),
            (
                "Bottom vertex height",
                self.bottom_height_min,
                self.bottom_height_max,
                0.0,
                1.0,
            ),
            (
                "Deltoid offset",
                self.offset_min,
                self.offset_max,
                -1.5,
                1.5,
            ),
            (
                "Deltoid sharpness",
                self.sharpness_min,
                self.sharpness_max,
                0.0,
                1.0,
            ),
        ] {
            validate_range(name, low, high, min, max)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct TriangleSettings {
    pub flip: bool,
    pub scale: f64,
    pub scale_min: f64,
    pub scale_max: f64,
    pub width_min: f64,
    pub width_max: f64,
    pub height_min: f64,
    pub height_max: f64,
    pub offset_min: f64,
    pub offset_max: f64,
    pub sharpness_min: f64,
    pub sharpness_max: f64,
}
impl Default for TriangleSettings {
    fn default() -> Self {
        Self {
            flip: false,
            scale: 1.0,
            scale_min: 1.0,
            scale_max: 1.0,
            width_min: 0.12,
            width_max: 0.35,
            height_min: 0.2,
            height_max: 0.45,
            offset_min: 0.0,
            offset_max: 0.0,
            sharpness_min: 0.8,
            sharpness_max: 1.0,
        }
    }
}
impl TriangleSettings {
    fn validate(&self) -> Result<(), String> {
        for (name, low, high, min, max) in [
            ("Triangle global scale", self.scale, self.scale, 0.01, 2.0),
            (
                "Triangle random scale",
                self.scale_min,
                self.scale_max,
                0.01,
                2.0,
            ),
            ("Triangle width", self.width_min, self.width_max, 0.0, 2.0),
            (
                "Triangle height",
                self.height_min,
                self.height_max,
                0.0,
                1.0,
            ),
            (
                "Triangle offset",
                self.offset_min,
                self.offset_max,
                -0.5,
                0.5,
            ),
            (
                "Triangle sharpness",
                self.sharpness_min,
                self.sharpness_max,
                0.0,
                1.0,
            ),
        ] {
            validate_range(name, low, high, min, max)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ExplosionSettings {
    /// Exact count, or the inclusive upper bound when count_min is present.
    pub count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_min: Option<u32>,
    pub seed: u32,
    pub different_seed_per_instance: bool,
    /// 0 = equidistant; 1 = independent uniform placement.
    pub regularity: f64,
    pub aspect_min: f64,
    pub aspect_max: f64,
    pub scale_min: f64,
    pub scale_max: f64,
    /// Fixed multipliers applied after each object's random shape sizing.
    pub shape_x: f64,
    pub shape_y: f64,
    /// Extra horizontal scale at the bottom; zero keeps the original geometry.
    pub bottom_power: f64,
    /// Fraction of the construction height affected, measured upward from the bottom.
    pub bottom_stretch: f64,
    pub bottom_curve: BottomCurve,
    pub radial_blur: f64,
    pub angular_blur: f64,
    pub brightness: f64,
    pub brightness_min: f64,
    pub brightness_max: f64,
    pub invert_colors: bool,
    /// Absent in older recipes, which retain their original triangle geometry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deltoid: Option<DeltoidSettings>,
    /// Use the canvas half-diagonal as the polar radius instead of its shorter half-edge.
    pub cover_canvas: bool,
    pub disable_out: bool,
    pub disable_in: bool,
    pub out: TriangleSettings,
    pub r#in: TriangleSettings,
    pub gradient: Value,
}
impl Default for ExplosionSettings {
    fn default() -> Self {
        Self {
            count: 9,
            count_min: None,
            seed: 1,
            different_seed_per_instance: false,
            regularity: 0.35,
            aspect_min: 3.0,
            aspect_max: 5.0,
            scale_min: 0.9,
            scale_max: 0.9,
            shape_x: 1.0,
            shape_y: 1.0,
            bottom_power: 0.0,
            bottom_stretch: 0.5,
            bottom_curve: BottomCurve::Linear,
            radial_blur: 0.0,
            angular_blur: 0.0,
            brightness: 1.0,
            brightness_min: 1.0,
            brightness_max: 1.0,
            invert_colors: false,
            deltoid: None,
            cover_canvas: false,
            disable_out: false,
            disable_in: false,
            out: TriangleSettings::default(),
            r#in: TriangleSettings::default(),
            gradient: json!({"kind":"Radial","angle":90.0,"stops":[
                {"offset":0.0,"colorHex":"#FF000000"},
                {"offset":1.0,"colorHex":"#FFFFFFFF"}]}),
        }
    }
}
fn validate_range(name: &str, low: f64, high: f64, min: f64, max: f64) -> Result<(), String> {
    if !low.is_finite() || !high.is_finite() || low < min || high > max || low > high {
        return Err(format!(
            "{name} requires ordered bounds between {min} and {max}"
        ));
    }
    Ok(())
}
impl ExplosionSettings {
    pub fn bottom_width_scale(&self, y: f64) -> f64 {
        self.bottom_curve
            .width_scale(self.bottom_power, self.bottom_stretch, y)
    }

    pub fn new_deltoid() -> Self {
        Self {
            deltoid: Some(DeltoidSettings::default()),
            cover_canvas: true,
            scale_min: 1.0,
            scale_max: 1.0,
            ..Self::default()
        }
    }
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let settings: Self = if value.is_null() {
            Self::new_deltoid()
        } else {
            serde_json::from_value(value.clone())
                .map_err(|e| format!("Invalid explosion settings: {e}"))?
        };
        settings.validate()?;
        Ok(settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_OBJECTS).contains(&self.count)
            || self
                .count_min
                .is_some_and(|min| !(1..=self.count).contains(&min))
        {
            return Err(
                "Explosion object count must be between 1 and 512, with ordered bounds".into(),
            );
        }
        validate_range("Aspect ratio", self.aspect_min, self.aspect_max, 1.0, 10.0)?;
        validate_range("Overall scale", self.scale_min, self.scale_max, 0.01, 2.0)?;
        validate_range("Shape X", self.shape_x, self.shape_x, 1.0, 10.0)?;
        validate_range("Shape Y", self.shape_y, self.shape_y, 1.0, 10.0)?;
        validate_range(
            "Bottom power",
            self.bottom_power,
            self.bottom_power,
            0.0,
            9.0,
        )?;
        validate_range(
            "Bottom stretch",
            self.bottom_stretch,
            self.bottom_stretch,
            0.0,
            1.0,
        )?;
        validate_range("Radial blur", self.radial_blur, self.radial_blur, 0.0, 1.0)?;
        validate_range(
            "Angular blur",
            self.angular_blur,
            self.angular_blur,
            0.0,
            1.0,
        )?;
        validate_range("Regularity", self.regularity, self.regularity, 0.0, 1.0)?;
        validate_range(
            "Base brightness",
            self.brightness,
            self.brightness,
            0.0,
            2.0,
        )?;
        validate_range(
            "Brightness variation",
            self.brightness_min,
            self.brightness_max,
            0.0,
            2.0,
        )?;
        self.out.validate()?;
        self.r#in.validate()?;
        if let Some(shape) = &self.deltoid {
            shape.validate()?;
        }
        if !matches!(
            self.gradient["kind"].as_str(),
            Some("Linear" | "Radial" | "SDF")
        ) {
            return Err("Explosion gradient must be Linear, Radial or SDF".into());
        }
        // Reuse the shared procedural gradient contract, including stop colors/alpha.
        let mut gradient = self.gradient.clone();
        if gradient["kind"] == "SDF" {
            gradient["kind"] = json!("Radial");
        }
        crate::particles::ParticleSettings {
            gradient,
            ..Default::default()
        }
        .validate()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Triangle {
    pub flip: bool,
    pub width: f64,
    pub height: f64,
    pub offset: f64,
    pub sharpness: f64,
}
impl Triangle {
    /// Return the wide base and direction to the tip. Flip swaps the two ends
    /// within the triangle's existing vertical bounds.
    pub fn placement(&self, outward: bool) -> (f64, f64) {
        let direction = if outward { -1.0 } else { 1.0 };
        let base = 0.5 + direction * self.offset;
        if self.flip {
            (base + direction * self.height, -direction)
        } else {
            (base, direction)
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Pair {
    pub brightness: f64,
    /// Position along the strip in strip-height units, wrapped at aspect.
    pub x: f64,
    pub out: Triangle,
    pub r#in: Triangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Deltoid {
    pub brightness: f64,
    pub ellipse: f64,
    pub x: f64,
    pub width: f64,
    pub top_height: f64,
    pub bottom_height: f64,
    pub offset: f64,
    pub sharpness: f64,
}
impl Deltoid {
    /// The same rounded profile is used by the strip diagram and the SDF.
    pub fn rounded_profile(&self) -> (f64, f64, f64, f64) {
        let shift = (self.bottom_height - self.top_height) * 0.5 * self.ellipse;
        (
            shift,
            self.top_height + shift,
            self.bottom_height - shift,
            1.0 + self.ellipse,
        )
    }
    pub fn outline(&self) -> Vec<[f64; 2]> {
        if self.ellipse == 0.0 {
            return self.vertices().to_vec();
        }
        let (shift, top, bottom, power) = self.rounded_profile();
        (0..32)
            .map(|i| {
                let angle =
                    -std::f64::consts::FRAC_PI_2 + f64::from(i) / 32.0 * std::f64::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                [
                    self.x + self.width * 0.5 * cos.signum() * cos.abs().powf(2.0 / power),
                    0.5 + self.offset
                        + shift
                        + if sin < 0.0 { top } else { bottom }
                            * sin.signum()
                            * sin.abs().powf(2.0 / power),
                ]
            })
            .collect()
    }
    pub fn vertices(&self) -> [[f64; 2]; 4] {
        let waist = 0.5 + self.offset;
        [
            [self.x, waist - self.top_height],
            [self.x + self.width / 2.0, waist],
            [self.x, waist + self.bottom_height],
            [self.x - self.width / 2.0, waist],
        ]
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub aspect: f64,
    pub scale: f64,
    pub pairs: Vec<Pair>,
    pub deltoids: Vec<Deltoid>,
}

/// One layout is shared by the construction diagram and the final compositor.
pub fn layout(settings: &ExplosionSettings, instance: &str) -> Layout {
    let mut state = u64::from(settings.seed);
    if settings.different_seed_per_instance {
        state ^= 0xcbf29ce484222325;
        for byte in instance.bytes() {
            state = (state ^ u64::from(byte.to_ascii_lowercase())).wrapping_mul(0x100000001b3);
        }
    }
    let mut random = || {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 11) as f64 / ((1_u64 << 53) as f64)
    };
    let aspect = settings.aspect_min + random() * (settings.aspect_max - settings.aspect_min);
    let scale = settings.scale_min + random() * (settings.scale_max - settings.scale_min);
    let min = settings.count_min.unwrap_or(settings.count);
    let count = min + (random() * f64::from(settings.count - min + 1)) as u32;
    if let Some(shape) = &settings.deltoid {
        let mut sample = |low, high| low + random() * (high - low);
        let mut deltoids: Vec<_> = (0..count)
            .map(|_| Deltoid {
                brightness: 1.0,
                ellipse: shape.ellipse,
                x: 0.0,
                width: sample(shape.width_min, shape.width_max),
                top_height: sample(shape.top_height_min, shape.top_height_max),
                bottom_height: sample(shape.bottom_height_min, shape.bottom_height_max),
                offset: sample(shape.offset_min, shape.offset_max),
                sharpness: sample(shape.sharpness_min, shape.sharpness_max),
            })
            .collect();
        for (i, deltoid) in deltoids.iter_mut().enumerate() {
            let uniform = (i as f64 + 0.5) / f64::from(count);
            deltoid.x = (uniform + settings.regularity * (random() - 0.5)).rem_euclid(1.0) * aspect;
        }
        for deltoid in &mut deltoids {
            let factor =
                shape.scale * (shape.scale_min + random() * (shape.scale_max - shape.scale_min));
            deltoid.width *= factor * settings.shape_x;
            deltoid.top_height *= factor * settings.shape_y;
            deltoid.bottom_height *= factor * settings.shape_y;
        }
        for deltoid in &mut deltoids {
            deltoid.brightness = settings.brightness_min
                + random() * (settings.brightness_max - settings.brightness_min);
        }
        return Layout {
            aspect,
            scale,
            pairs: Vec::new(),
            deltoids,
        };
    }
    let mut triangle = |s: &TriangleSettings| Triangle {
        flip: s.flip,
        width: s.width_min + random() * (s.width_max - s.width_min),
        height: s.height_min + random() * (s.height_max - s.height_min),
        offset: s.offset_min + random() * (s.offset_max - s.offset_min),
        sharpness: s.sharpness_min + random() * (s.sharpness_max - s.sharpness_min),
    };
    // Sample geometry before positions so changing spacing never changes the shapes.
    let mut pairs: Vec<_> = (0..count)
        .map(|_| Pair {
            brightness: 1.0,
            x: 0.0,
            out: triangle(&settings.out),
            r#in: triangle(&settings.r#in),
        })
        .collect();
    for (i, pair) in pairs.iter_mut().enumerate() {
        let uniform = (i as f64 + 0.5) / f64::from(count);
        let jitter = random() - 0.5;
        pair.x = (uniform + settings.regularity * jitter).rem_euclid(1.0) * aspect;
    }
    // Append scale sampling so recipes saved before these controls retain their
    // existing dimensions/positions. Both halves sample independently per pair.
    for pair in &mut pairs {
        for (t, s) in [
            (&mut pair.out, &settings.out),
            (&mut pair.r#in, &settings.r#in),
        ] {
            let scale = s.scale * (s.scale_min + random() * (s.scale_max - s.scale_min));
            t.width *= scale * settings.shape_x;
            t.height *= scale * settings.shape_y;
        }
    }
    for pair in &mut pairs {
        pair.brightness = settings.brightness_min
            + random() * (settings.brightness_max - settings.brightness_min);
    }
    Layout {
        aspect,
        scale,
        pairs,
        deltoids: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explosion_shape_axes_preserve_random_samples_and_maximum_count_round_trips() {
        for deltoid in [false, true] {
            let mut settings = if deltoid {
                ExplosionSettings::new_deltoid()
            } else {
                ExplosionSettings::default()
            };
            settings.count = MAX_OBJECTS;
            if let Some(shape) = &mut settings.deltoid {
                shape.ellipse = 0.65;
                shape.scale_min = 0.5;
                shape.scale_max = 1.5;
            }
            let before = layout(&settings, "shape axes");
            settings.shape_x = 10.0;
            settings.shape_y = 3.0;
            let after = layout(&settings, "shape axes");
            assert_eq!(before.aspect, after.aspect);
            assert_eq!(before.scale, after.scale);
            assert_eq!(before.deltoids.len() + before.pairs.len(), 512);
            for (a, b) in before.deltoids.iter().zip(&after.deltoids) {
                assert_eq!(
                    (a.x, a.offset, a.sharpness, a.ellipse),
                    (b.x, b.offset, b.sharpness, b.ellipse)
                );
                assert!((b.width - a.width * 10.0).abs() < 1e-12);
                assert!((b.top_height - a.top_height * 3.0).abs() < 1e-12);
                assert!((b.bottom_height - a.bottom_height * 3.0).abs() < 1e-12);
            }
            for (a, b) in before.pairs.iter().zip(&after.pairs) {
                assert_eq!(a.x, b.x);
                for (a, b) in [(&a.out, &b.out), (&a.r#in, &b.r#in)] {
                    assert_eq!(
                        (a.offset, a.sharpness, a.flip),
                        (b.offset, b.sharpness, b.flip)
                    );
                    assert!((b.width - a.width * 10.0).abs() < 1e-12);
                    assert!((b.height - a.height * 3.0).abs() < 1e-12);
                }
            }
            assert_eq!(
                ExplosionSettings::from_value(&json!(settings)).unwrap(),
                settings
            );
            settings.count_min = Some(500);
            settings.validate().unwrap();
            let sampled = layout(&settings, "count range");
            assert!((500..=512).contains(&(sampled.deltoids.len() + sampled.pairs.len())));
        }
        let legacy = ExplosionSettings::from_value(&json!({"deltoid":{}})).unwrap();
        assert_eq!(
            (
                legacy.shape_x,
                legacy.shape_y,
                legacy.deltoid.unwrap().ellipse
            ),
            (1.0, 1.0, 0.0)
        );
        for ellipse in [-0.1, 1.1] {
            assert!(
                ExplosionSettings::from_value(&json!({"deltoid":{"ellipse":ellipse}})).is_err()
            );
        }
    }
    #[test]
    fn explosion_bottom_scaling_curves_stretch_bounds_and_seed_compatibility() {
        let legacy = ExplosionSettings::from_value(&json!({})).unwrap();
        assert_eq!(
            (
                legacy.bottom_power,
                legacy.bottom_stretch,
                legacy.bottom_curve
            ),
            (0.0, 0.5, BottomCurve::Linear)
        );
        for deltoid in [false, true] {
            let mut settings = if deltoid {
                ExplosionSettings::new_deltoid()
            } else {
                legacy.clone()
            };
            let original = layout(&settings, "bottom fill");
            settings.bottom_power = 9.0;
            for curve in [
                BottomCurve::Linear,
                BottomCurve::Quadratic,
                BottomCurve::Logarithmic,
            ] {
                settings.bottom_curve = curve;
                assert_eq!(layout(&settings, "bottom fill"), original);
                for y in [-1.0, 0.0, 0.25, 0.5] {
                    assert_eq!(settings.bottom_width_scale(y), 1.0);
                }
                assert!((settings.bottom_width_scale(1.0) - 10.0).abs() < 1e-12);
                let values: Vec<_> = (0..=100)
                    .map(|y| settings.bottom_width_scale(f64::from(y) / 100.0))
                    .collect();
                assert!(values.windows(2).all(|p| p[1] >= p[0]));
                assert_eq!(
                    ExplosionSettings::from_value(&json!(settings)).unwrap(),
                    settings
                );
            }
            settings.bottom_curve = BottomCurve::Linear;
            assert_eq!(settings.bottom_width_scale(0.75), 5.5);
            settings.bottom_stretch = 1.0;
            assert_eq!(settings.bottom_width_scale(0.5), 5.5);
            settings.bottom_stretch = 0.25;
            assert_eq!(settings.bottom_width_scale(0.75), 1.0);
            settings.bottom_stretch = 0.0;
            assert_eq!(settings.bottom_width_scale(1.0), 1.0);
            settings.bottom_stretch = f64::MIN_POSITIVE;
            assert_eq!(settings.bottom_width_scale(1.0), 10.0);
            for invalid in [-0.1, 9.1, f64::NAN, f64::INFINITY] {
                settings.bottom_power = invalid;
                assert!(settings.validate().is_err());
            }
            settings.bottom_power = 1.0;
            for invalid in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
                settings.bottom_stretch = invalid;
                assert!(settings.validate().is_err());
            }
        }
        assert!(BottomCurve::Quadratic.sample(0.5) < BottomCurve::Linear.sample(0.5));
        assert!(BottomCurve::Logarithmic.sample(0.5) > BottomCurve::Linear.sample(0.5));
        for bad in [json!("Unknown"), json!(null), json!(1)] {
            assert!(ExplosionSettings::from_value(&json!({"bottomCurve":bad})).is_err());
        }
    }
    #[test]
    fn explosion_accepts_sdf_gradient_with_valid_stops_and_preserves_old_polar_fit() {
        let mut settings = ExplosionSettings::new_deltoid();
        assert!(settings.cover_canvas);
        assert_eq!((settings.scale_min, settings.scale_max), (1.0, 1.0));
        assert!(
            !ExplosionSettings::from_value(&json!({}))
                .unwrap()
                .cover_canvas
        );
        for kind in ["Radial", "Linear", "SDF"] {
            settings.gradient["kind"] = json!(kind);
            assert_eq!(
                ExplosionSettings::from_value(&json!(settings)).unwrap(),
                settings
            );
        }
        let saved = json!(settings);
        for (key, invalid) in [("stops", json!([])), ("angle", json!("invalid"))] {
            let mut value = saved.clone();
            value["gradient"][key] = invalid;
            assert!(ExplosionSettings::from_value(&value).is_err());
        }
        let mut value = saved.clone();
        value["gradient"]["stops"][0]["offset"] = json!(1.1);
        assert!(ExplosionSettings::from_value(&value).is_err());
        let mut value = saved.clone();
        value["gradient"]["stops"][0]["colorHex"] = json!("invalid");
        assert!(ExplosionSettings::from_value(&value).is_err());
        let mut value = saved;
        value["coverCanvas"] = json!("true");
        assert!(ExplosionSettings::from_value(&value).is_err());
    }
    #[test]
    fn explosion_deltoids_share_width_scale_and_waist_with_independent_vertex_heights() {
        let mut settings = ExplosionSettings {
            count: 12,
            regularity: 0.0,
            ..ExplosionSettings::new_deltoid()
        };
        let normal = layout(&settings, "deltoid");
        assert!(normal.pairs.is_empty());
        assert_eq!(normal.deltoids.len(), 12);
        let shape = settings.deltoid.as_mut().unwrap();
        shape.scale = 1.5;
        shape.scale_min = 0.5;
        shape.scale_max = 1.5;
        let varied = layout(&settings, "deltoid");
        assert_eq!(varied, layout(&settings, "deltoid"));
        let mut factors = Vec::new();
        for (original, scaled) in normal.deltoids.iter().zip(&varied.deltoids) {
            let factor = scaled.width / original.width;
            assert!((0.75..=2.25).contains(&factor));
            assert!((scaled.top_height / original.top_height - factor).abs() < 1e-12);
            assert!((scaled.bottom_height / original.bottom_height - factor).abs() < 1e-12);
            assert_eq!(original.x, scaled.x);
            assert_eq!(original.offset, scaled.offset);
            let [top, right, bottom, left] = scaled.vertices();
            assert_eq!(top[0], bottom[0]);
            assert!((right[0] + left[0] - 2.0 * top[0]).abs() < 1e-12);
            assert_eq!(left[1], right[1]);
            assert!(top[1] < right[1] && bottom[1] > right[1]);
            factors.push(factor);
        }
        assert!(factors.windows(2).any(|p| p[0] != p[1]));
        assert!(
            normal
                .deltoids
                .iter()
                .any(|d| d.top_height != d.bottom_height)
        );
        for pair in normal.deltoids.windows(2) {
            assert!((pair[1].x - pair[0].x - normal.aspect / 12.0).abs() < 1e-12);
        }
        let saved = json!(settings);
        assert_eq!(ExplosionSettings::from_value(&saved).unwrap(), settings);
        assert!(
            ExplosionSettings::from_value(&json!({}))
                .unwrap()
                .deltoid
                .is_none()
        );
        assert!(
            ExplosionSettings::from_value(&Value::Null)
                .unwrap()
                .deltoid
                .is_some()
        );
        for (key, invalid) in [
            ("topHeightMin", json!(-0.1)),
            ("topHeightMax", json!(1.1)),
            ("bottomHeightMin", json!(0.5)),
            ("bottomHeightMax", json!(-0.1)),
            ("scale", json!(0)),
            ("scaleMin", json!(1.6)),
            ("scaleMax", json!(2.1)),
            ("widthMin", json!(-0.1)),
            ("offsetMax", json!(1.6)),
            ("sharpnessMax", json!(1.1)),
            ("unknown", json!(true)),
        ] {
            let mut invalid_recipe = saved.clone();
            invalid_recipe["deltoid"][key] = invalid;
            assert!(
                ExplosionSettings::from_value(&invalid_recipe).is_err(),
                "{key}"
            );
        }
    }
    #[test]
    fn explosion_triangle_scale_samples_are_independent_and_preserve_legacy_layouts() {
        let mut settings = ExplosionSettings::default();
        let legacy = layout(&settings, "scale");
        assert!((legacy.aspect - 4.133123150344562).abs() < 1e-12);
        assert!((legacy.pairs[0].x - 0.18707202877294724).abs() < 1e-12);
        assert!((legacy.pairs[0].out.width - 0.22220261992282758).abs() < 1e-12);
        settings.out.scale = 1.5;
        settings.out.scale_min = 0.5;
        settings.out.scale_max = 1.5;
        settings.out.flip = true;
        settings.r#in.scale = 0.75;
        settings.r#in.scale_min = 0.5;
        settings.r#in.scale_max = 1.5;
        let varied = layout(&settings, "scale");
        assert_eq!(varied, layout(&settings, "scale"));
        let mut factors = Vec::new();
        for (before, after) in legacy.pairs.iter().zip(&varied.pairs) {
            assert_eq!(before.x, after.x);
            let out = after.out.width / before.out.width;
            let inward = after.r#in.width / before.r#in.width;
            assert!((0.75..=2.25).contains(&out));
            assert!((0.375..=1.125).contains(&inward));
            assert!((out - after.out.height / before.out.height).abs() < 1e-12);
            assert!((inward - after.r#in.height / before.r#in.height).abs() < 1e-12);
            assert_eq!(before.out.offset, after.out.offset);
            assert!(after.out.flip);
            assert_ne!(out / 1.5, inward / 0.75);
            factors.push(out);
        }
        assert!(factors.windows(2).any(|p| p[0] != p[1]));
        let mut sparse = json!(ExplosionSettings::default());
        for key in ["out", "in"] {
            for new_field in ["flip", "scale", "scaleMin", "scaleMax"] {
                sparse[key].as_object_mut().unwrap().remove(new_field);
            }
        }
        assert_eq!(
            legacy,
            layout(&ExplosionSettings::from_value(&sparse).unwrap(), "scale")
        );
    }
    #[test]
    fn explosion_flip_swaps_wide_and_pointed_ends_within_existing_bounds() {
        for outward in [true, false] {
            for height in [0.0, 0.3, 1.5] {
                for offset in [-0.2, 0.0, 0.25] {
                    let mut triangle = Triangle {
                        flip: false,
                        width: 1.2,
                        height,
                        offset,
                        sharpness: 0.8,
                    };
                    let (base, direction) = triangle.placement(outward);
                    let tip = base + direction * height;
                    triangle.flip = true;
                    let (flipped_base, flipped_direction) = triangle.placement(outward);
                    let flipped_tip = flipped_base + flipped_direction * height;
                    assert_eq!(flipped_base, tip);
                    assert_eq!(flipped_direction, -direction);
                    assert!((flipped_tip - base).abs() < 1e-12);
                    assert!((flipped_base.min(flipped_tip) - base.min(tip)).abs() < 1e-12);
                    assert!((flipped_base.max(flipped_tip) - base.max(tip)).abs() < 1e-12);
                }
            }
        }
    }
    #[test]
    fn explosion_layout_ranges_spacing_and_instance_identity() {
        let mut s = ExplosionSettings {
            regularity: 0.0,
            count_min: Some(3),
            ..Default::default()
        };
        let a = layout(&s, "A");
        assert_eq!(a, layout(&s, "B"));
        assert!((3..=9).contains(&a.pairs.len()));
        for adjacent in a.pairs.windows(2) {
            assert!(
                (adjacent[1].x - adjacent[0].x - a.aspect / a.pairs.len() as f64).abs() < 1e-12
            );
        }
        assert!((s.aspect_min..=s.aspect_max).contains(&a.aspect));
        assert_eq!(a.scale, 0.9);
        s.different_seed_per_instance = true;
        assert_ne!(layout(&s, "A"), layout(&s, "B"));
        assert_eq!(layout(&s, "ABC"), layout(&s, "abc"));
        s.regularity = 1.0;
        let random = layout(&s, "A");
        assert!(
            random
                .pairs
                .iter()
                .all(|p| (0.0..random.aspect).contains(&p.x))
        );
    }
    #[test]
    fn explosion_rejects_invalid_ranges_and_round_trips() {
        let s = ExplosionSettings::default();
        assert_eq!(ExplosionSettings::from_value(&json!(s)).unwrap(), s);
        for (key, value) in [
            ("count", json!(0)),
            ("count", json!(513)),
            ("countMin", json!(10)),
            ("aspectMin", json!(0.9)),
            ("aspectMax", json!(11)),
            ("scaleMin", json!(0)),
            ("scaleMax", json!(0.8)),
            ("regularity", json!(1.01)),
            ("shapeX", json!(0.99)),
            ("shapeX", json!(10.01)),
            ("shapeY", json!(0.99)),
            ("shapeY", json!(10.01)),
            ("radialBlur", json!(-0.1)),
            ("radialBlur", json!(1.1)),
            ("angularBlur", json!(-0.1)),
            ("angularBlur", json!(1.1)),
            ("disableIn", json!(1)),
            ("unknown", json!(true)),
        ] {
            let mut value_s = json!(s);
            value_s[key] = value;
            assert!(ExplosionSettings::from_value(&value_s).is_err(), "{key}");
        }
        for key in ["widthMin", "heightMin", "offsetMin", "sharpnessMin"] {
            let mut value = json!(s);
            value["out"][key] = json!(3);
            assert!(ExplosionSettings::from_value(&value).is_err(), "{key}");
        }
        for (key, invalid) in [
            ("scale", json!(0)),
            ("scale", json!(2.1)),
            ("scaleMin", json!(0)),
            ("scaleMin", json!(1.1)),
            ("scaleMax", json!(0.9)),
            ("flip", json!("yes")),
        ] {
            let mut value = json!(s);
            value["in"][key] = invalid;
            assert!(ExplosionSettings::from_value(&value).is_err(), "{key}");
        }
    }
}
