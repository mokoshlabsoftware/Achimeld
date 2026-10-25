//! Bounded, portable two-dimensional color fields shared by the editor and renderer.
use serde::{Deserialize, Serialize};

pub const EDGE_POINTS: usize = 12;
pub const MAX_POINTS: usize = 64;

/// Initial positions: four points on each edge, with shared corners.
pub fn anchor_position(index: usize) -> (f64, f64) {
    match index {
        0..=3 => (index as f64 / 3.0, 0.0),
        4..=7 => ((index - 4) as f64 / 3.0, 1.0),
        8..=9 => (0.0, (index - 7) as f64 / 3.0),
        10..=11 => (1.0, (index - 9) as f64 / 3.0),
        _ => (0.5, 0.5),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradientMap {
    pub points: Vec<ColorPoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradientMapPreset {
    pub id: String,
    pub name: String,
    pub map: GradientMap,
}

impl GradientMapPreset {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 128
            || self.name.trim().is_empty()
            || self.name.len() > 256
        {
            return Err("Invalid 2D gradient name or identity".into());
        }
        self.map.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorPoint {
    pub x: f64,
    pub y: f64,
    pub color_hex: String,
    pub power: f64,
    pub range_x: f64,
    pub range_y: f64,
}

impl Default for ColorPoint {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.5,
            color_hex: "#FFFFFFFF".into(),
            power: 1.0,
            range_x: 0.35,
            range_y: 0.35,
        }
    }
}

impl Default for GradientMap {
    fn default() -> Self {
        Self {
            points: (0..EDGE_POINTS)
                .map(|i| {
                    let (x, y) = anchor_position(i);
                    ColorPoint {
                        x,
                        y,
                        color_hex: {
                            let shade = (y * 255.0).round() as u8;
                            encode_color([shade, shade, shade, 255])
                        },
                        ..Default::default()
                    }
                })
                .collect(),
        }
    }
}

impl GradientMap {
    /// Reverse each particle's gradient while keeping the variation axis.
    pub fn reverse_gradient(&mut self) -> Result<(), String> {
        self.validate()?;
        let mut reversed = self.clone();
        // Preserve the original ordering of untouched legacy edge points.
        let original_edges = self.points.len() >= EDGE_POINTS
            && self
                .points
                .iter()
                .take(EDGE_POINTS)
                .enumerate()
                .all(|(i, p)| (p.x, p.y) == anchor_position(i));
        for (index, point) in self.points.iter().enumerate() {
            if original_edges && index < EDGE_POINTS {
                let source = self.points[..EDGE_POINTS]
                    .iter()
                    .find(|p| (p.x - point.x).abs() < 1e-9 && (p.y - (1.0 - point.y)).abs() < 1e-9)
                    .ok_or("Missing mirrored edge anchor")?;
                reversed.points[index] = source.clone();
                reversed.points[index].x = point.x;
                reversed.points[index].y = point.y;
            } else {
                reversed.points[index].y = 1.0 - point.y;
            }
        }
        reversed.validate()?;
        *self = reversed;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.points.len() > MAX_POINTS {
            return Err("A 2D gradient may contain at most 64 points".into());
        }
        for (i, point) in self.points.iter().enumerate() {
            if !point.x.is_finite()
                || !point.y.is_finite()
                || !(0.0..=1.0).contains(&point.x)
                || !(0.0..=1.0).contains(&point.y)
                || decode_color(&point.color_hex).is_none()
                || ![
                    (point.power, 0.05, 8.0),
                    (point.range_x, 0.02, 2.0),
                    (point.range_y, 0.02, 2.0),
                ]
                .iter()
                .all(|&(v, min, max)| v.is_finite() && (min..=max).contains(&v))
            {
                return Err("Invalid 2D gradient point or influence".into());
            }
            if self.points[..i]
                .iter()
                .any(|other| (point.x - other.x).powi(2) + (point.y - other.y).powi(2) < 1e-8)
            {
                return Err("2D gradient points must have distinct positions".into());
            }
        }
        Ok(())
    }

    pub fn compile(&self) -> Result<ColorField, String> {
        self.validate()?;
        Ok(ColorField {
            points: self
                .points
                .iter()
                .map(|p| {
                    let c = decode_color(&p.color_hex).unwrap_or([0; 4]);
                    let alpha = f64::from(c[3]) / 255.0;
                    FieldPoint {
                        x: p.x,
                        y: p.y,
                        power: p.power,
                        inverse_x: p.range_x.recip(),
                        inverse_y: p.range_y.recip(),
                        color: [
                            f64::from(c[0]) * alpha,
                            f64::from(c[1]) * alpha,
                            f64::from(c[2]) * alpha,
                            alpha,
                        ],
                    }
                })
                .collect(),
        })
    }

    /// Seed a new field with colors from an existing 1D gradient.
    pub fn from_stops(stops: &[(f64, [u8; 4])]) -> Self {
        let sample = |x: f64| {
            let left = stops.iter().rfind(|p| p.0 <= x).or(stops.first());
            let right = stops.iter().find(|p| p.0 > x).or(left);
            match (left, right) {
                (Some(a), Some(b)) => {
                    let t = if b.0 > a.0 {
                        ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    std::array::from_fn(|i| {
                        (f64::from(a.1[i]) * (1.0 - t) + f64::from(b.1[i]) * t).round() as u8
                    })
                }
                _ => [255; 4],
            }
        };
        let mut map = Self::default();
        for p in &mut map.points {
            p.color_hex = encode_color(sample(p.y));
        }
        for &(y, color) in stops.iter().take(MAX_POINTS - EDGE_POINTS) {
            if y > 0.0001
                && y < 0.9999
                && !map
                    .points
                    .iter()
                    .any(|p| (p.y - y).abs() < 0.0001 && p.x == 0.5)
            {
                map.points.push(ColorPoint {
                    x: 0.5,
                    y,
                    color_hex: encode_color(color),
                    ..Default::default()
                });
            }
        }
        map
    }
}

struct FieldPoint {
    x: f64,
    y: f64,
    power: f64,
    inverse_x: f64,
    inverse_y: f64,
    color: [f64; 4],
}

pub struct ColorField {
    points: Vec<FieldPoint>,
}

impl ColorField {
    /// Anisotropic Shepard interpolation: all points negotiate a smooth, distance-weighted field.
    /// Premultiplied alpha prevents transparent points from adding dark color fringes.
    pub fn sample(&self, x: f64, y: f64) -> [u8; 4] {
        if self.points.is_empty() {
            return [0; 4];
        }
        let (x, y) = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        let mut sum = [0.0; 4];
        let mut total = 0.0;
        for p in &self.points {
            let distance = ((x - p.x) * p.inverse_x).powi(2) + ((y - p.y) * p.inverse_y).powi(2);
            if distance < 1e-20 {
                return unpremultiply(p.color);
            }
            let weight = p.power / (distance * distance);
            for (sum, value) in sum.iter_mut().zip(p.color) {
                *sum += weight * value;
            }
            total += weight;
        }
        unpremultiply(sum.map(|v| v / total))
    }

    /// A vertical, top-to-bottom gradient at a chosen horizontal position.
    pub fn slice(&self, x: f64, height: usize) -> Vec<[u8; 4]> {
        (0..height)
            .map(|y| self.sample(x, y as f64 / height.saturating_sub(1).max(1) as f64))
            .collect()
    }

    pub fn raster(
        &self,
        width: usize,
        height: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<Vec<[u8; 4]>> {
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            if cancelled() {
                return None;
            }
            let y = y as f64 / height.saturating_sub(1).max(1) as f64;
            pixels.extend(
                (0..width)
                    .map(|x| self.sample(x as f64 / width.saturating_sub(1).max(1) as f64, y)),
            );
        }
        Some(pixels)
    }
}

fn unpremultiply(color: [f64; 4]) -> [u8; 4] {
    if color[3] <= 0.0 {
        return [0; 4];
    }
    [
        (color[0] / color[3]).round().clamp(0.0, 255.0) as u8,
        (color[1] / color[3]).round().clamp(0.0, 255.0) as u8,
        (color[2] / color[3]).round().clamp(0.0, 255.0) as u8,
        (color[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

pub fn decode_color(hex: &str) -> Option<[u8; 4]> {
    let text = hex.strip_prefix('#')?;
    if ![6, 8].contains(&text.len()) || !text.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let n = u32::from_str_radix(text, 16).ok()?;
    Some([
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
        if text.len() == 8 {
            (n >> 24) as u8
        } else {
            255
        },
    ])
}

pub fn encode_color([r, g, b, a]: [u8; 4]) -> String {
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

pub fn presets() -> Vec<(&'static str, GradientMap)> {
    let mut aurora = GradientMap::default();
    for p in &mut aurora.points {
        let top = [60.0 + p.x * 120.0, 235.0 - p.x * 140.0, 255.0];
        let bottom = [255.0, 100.0 + p.x * 125.0, 70.0 + p.x * 20.0];
        p.color_hex = encode_color([0, 1, 2, 3].map(|i| {
            if i == 3 {
                255
            } else {
                (top[i] * (1.0 - p.y) + bottom[i] * p.y).round() as u8
            }
        }));
    }
    let mut glow = aurora.clone();
    for p in &mut glow.points {
        let color = decode_color(&p.color_hex).unwrap_or([255; 4]);
        p.color_hex = encode_color([
            color[0],
            color[1],
            color[2],
            ((1.0 - p.y) * 255.0).round() as u8,
        ]);
    }
    let mut rainbow = aurora.clone();
    for p in &mut rainbow.points {
        p.color_hex = encode_color(if p.y < 0.5 {
            [
                (255.0 * (1.0 - p.x)).round() as u8,
                (180.0 * p.x).round() as u8,
                255,
                255,
            ]
        } else {
            [255, (80.0 + p.x * 175.0).round() as u8, 40, 255]
        });
    }
    vec![
        ("Black to white", GradientMap::default()),
        ("Aurora", aurora),
        ("Soft spectrum", glow),
        ("Warm and cool", rainbow),
    ]
}

#[cfg(test)]
#[path = "gradient_map_tests.rs"]
mod tests;
