//! Seeded gradient noise with resolution-independent coordinates and bounded work.
use image::{Rgba, RgbaImage};
use serde_json::Value;

pub(crate) const MAX_OCTAVES: usize = 12;

fn number(value: &Value, key: &str, default: f64) -> f64 {
    value[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}

fn hash(x: i64, y: i64, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

fn dot(x: i64, y: i64, dx: f64, dy: f64, seed: u32) -> f64 {
    let d = std::f64::consts::FRAC_1_SQRT_2;
    match hash(x, y, seed) & 7 {
        0 => dx,
        1 => -dx,
        2 => dy,
        3 => -dy,
        4 => (dx + dy) * d,
        5 => (dx - dy) * d,
        6 => (-dx + dy) * d,
        _ => (-dx - dy) * d,
    }
}

fn noise(x: f64, y: f64, seed: u32) -> f64 {
    let (ix, iy) = (x.floor() as i64, y.floor() as i64);
    let (x, y) = (x - x.floor(), y - y.floor());
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    lerp(
        lerp(
            dot(ix, iy, x, y, seed),
            dot(ix + 1, iy, x - 1.0, y, seed),
            fade(x),
        ),
        lerp(
            dot(ix, iy + 1, x, y - 1.0, seed),
            dot(ix + 1, iy + 1, x - 1.0, y - 1.0, seed),
            fade(x),
        ),
        fade(y),
    )
}

pub(crate) fn render(
    settings: &Value,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let seed = settings["seed"].as_u64().unwrap_or(0).min(i32::MAX as u64) as u32;
    let count = settings["octaves"]
        .as_u64()
        .unwrap_or(6)
        .clamp(1, MAX_OCTAVES as u64);
    let persistence = number(settings, "persistence", 0.5).clamp(0.0, 1.0);
    let lacunarity = number(settings, "lacunarity", 2.0).clamp(1.0, 4.0);
    let mut frequency = number(settings, "frequency", 4.0).clamp(0.1, 128.0);
    let mut amplitude = 1.0;
    let mut octaves = Vec::with_capacity(MAX_OCTAVES);
    let mut total = 0.0;
    for i in 0..count as usize {
        if settings["enabledOctaves"].get(i).and_then(Value::as_bool) != Some(false)
            && amplitude > 0.0
        {
            octaves.push((
                frequency,
                amplitude,
                seed.wrapping_add((i as u32).wrapping_mul(0x9e37_79b9)),
            ));
            total += amplitude;
        }
        frequency *= lacunarity;
        amplitude *= persistence;
    }
    let colors = if settings["useGradient"] == true {
        Some(super::renderer::gradient_lookup(&settings["gradient"])?)
    } else {
        None
    };
    let mut image = RgbaImage::new(width, height);
    // Rectangular outputs keep isotropic features. All preview sizes sample the
    // same continuous field in project-relative coordinates.
    let unit = f64::from(width.min(height).max(1));
    for (y, mut row) in image.enumerate_rows_mut() {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for (x, _, pixel) in &mut row {
            let (x, y) = ((f64::from(x) + 0.5) / unit, (f64::from(y) + 0.5) / unit);
            let sum: f64 = octaves
                .iter()
                .map(|&(f, a, s)| noise(x * f + 0.37, y * f + 0.73, s) * a)
                .sum();
            // Disabling every octave gives neutral grey without dividing by zero.
            let value = if total > 0.0 {
                0.5 + sum / total * std::f64::consts::FRAC_1_SQRT_2
            } else {
                0.5
            };
            let index = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            *pixel = colors
                .as_ref()
                .map_or(Rgba([index, index, index, 255]), |colors| {
                    colors[index as usize]
                });
        }
    }
    Ok(image)
}
