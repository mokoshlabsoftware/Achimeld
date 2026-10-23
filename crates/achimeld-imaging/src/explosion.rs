//! Inverse polar sampling of a periodic SDF strip; no oversized intermediate bitmap.
use achimeld_application::explosion::{BottomCurve, ExplosionSettings, Layout, Triangle, layout};
use image::{Rgba, RgbaImage};
use serde_json::Value;

const BUCKETS: usize = 256;
struct CompiledTriangle {
    brightness: f64,
    pair: usize,
    join_pair: bool,
    x: f64,
    base: f64,
    direction: f64,
    half_width: f64,
    height: f64,
    edge_squared: f64,
    inradius: f64,
    feather: f64,
}
struct CompiledDeltoid {
    brightness: f64,
    x: f64,
    waist: f64,
    half_width: f64,
    top: f64,
    bottom: f64,
    center_shift: f64,
    radius_top: f64,
    radius_bottom: f64,
    power: f64,
    top_edge_squared: f64,
    bottom_edge_squared: f64,
    inradius: f64,
    feather: f64,
}
impl CompiledDeltoid {
    fn sample(
        &self,
        x: f64,
        y: f64,
        aspect: f64,
        width_scale: f64,
        sdf_gradient: bool,
    ) -> (f64, f64) {
        let px = periodic_delta(x - self.x, aspect).abs() / width_scale;
        let py = y - self.waist;
        if px > self.half_width + self.feather
            || py < -self.top - self.feather
            || py > self.bottom + self.feather
        {
            return (0.0, 0.0);
        }
        let distance = self.distance(px, py);
        let alpha = (0.5 - distance / (2.0 * self.feather)).clamp(0.0, 1.0);
        (
            alpha * alpha * (3.0 - 2.0 * alpha),
            if sdf_gradient {
                sdf_input(distance, self.inradius)
            } else {
                0.0
            },
        )
    }
    fn distance(&self, px: f64, py: f64) -> f64 {
        if self.power > 1.0 {
            // The p-norm outline rounds continuously from a kite to an ellipse.
            // Normalize its implicit field by the local gradient for an SDF
            // approximation in strip units, independent of edge feathering.
            let y = py - self.center_shift;
            let rx = self.half_width.max(f64::EPSILON);
            let ry = (if y < 0.0 {
                self.radius_top
            } else {
                self.radius_bottom
            })
            .max(f64::EPSILON);
            let u = px / rx;
            let v = y.abs() / ry;
            let norm = (u.powf(self.power) + v.powf(self.power)).powf(1.0 / self.power);
            if norm < f64::EPSILON {
                return -self.inradius;
            }
            let gradient = (u.powf(self.power - 1.0) / rx).hypot(v.powf(self.power - 1.0) / ry);
            return norm.powf(self.power - 1.0) * (norm - 1.0) / gradient.max(f64::EPSILON);
        }
        // Reflection around X=0 reduces the kite's four perimeter edges to two.
        // The shared waist is inside the shape, never an antialiased seam.
        let side_distance = |height: f64, squared: f64| {
            let along = (((px - self.half_width) * -self.half_width + py * height) / squared)
                .clamp(0.0, 1.0);
            (px - self.half_width + self.half_width * along).hypot(py - height * along)
        };
        let distance = side_distance(-self.top, self.top_edge_squared)
            .min(side_distance(self.bottom, self.bottom_edge_squared));
        let height = if py < 0.0 { self.top } else { self.bottom };
        let inside = if py == 0.0 {
            px <= self.half_width
        } else {
            height > 0.0 && py.abs() <= height && px <= self.half_width * (1.0 - py.abs() / height)
        };
        distance * if inside { -1.0 } else { 1.0 }
    }
}
struct Strip {
    brightness: f64,
    invert_colors: bool,
    layout: Layout,
    triangles: Vec<CompiledTriangle>,
    buckets: Vec<Vec<usize>>,
    deltoids: Vec<CompiledDeltoid>,
    deltoid_buckets: Vec<Vec<usize>>,
    radius: f64,
    radial_blur: f64,
    angular_blur: f64,
    bottom_power: f64,
    bottom_stretch: f64,
    bottom_curve: BottomCurve,
}
impl Strip {
    fn new(settings: &ExplosionSettings, instance: &str, radius: f64) -> Self {
        let layout = layout(settings, instance);
        let mut triangles = Vec::with_capacity(layout.pairs.len() * 2);
        let mut add = |pair: usize, x: f64, t: &Triangle, outward: bool, join_pair: bool| {
            if t.width == 0.0 || t.height == 0.0 {
                return;
            }
            let half_width = t.width / 2.0;
            let (base, direction) = t.placement(outward);
            triangles.push(CompiledTriangle {
                brightness: layout.pairs[pair].brightness,
                pair,
                join_pair,
                x,
                base,
                direction,
                half_width,
                height: t.height,
                edge_squared: half_width * half_width + t.height * t.height,
                inradius: half_width * t.height / (half_width + half_width.hypot(t.height)),
                feather: ((1.0 - t.sharpness) * t.width.min(t.height) * 0.3)
                    .max(0.75 / radius.max(0.5)),
            });
        };
        for (index, pair) in layout.pairs.iter().enumerate() {
            let (out_base, out_direction) = pair.out.placement(true);
            let (in_base, in_direction) = pair.r#in.placement(false);
            let join_pair = (out_base - in_base).abs() < 1e-12 && out_direction != in_direction;
            if !settings.disable_out {
                add(index, pair.x, &pair.out, true, join_pair);
            }
            if !settings.disable_in {
                add(index, pair.x, &pair.r#in, false, join_pair);
            }
        }
        let deltoids: Vec<_> = layout
            .deltoids
            .iter()
            .filter(|d| d.width > 0.0 && d.top_height + d.bottom_height > 0.0)
            .map(|d| {
                let (center_shift, radius_top, radius_bottom, power) = d.rounded_profile();
                let deltoid_radius = d.width / 2.0 * (d.top_height + d.bottom_height)
                    / ((d.width / 2.0).hypot(d.top_height)
                        + (d.width / 2.0).hypot(d.bottom_height));
                CompiledDeltoid {
                    brightness: d.brightness,
                    x: d.x,
                    waist: 0.5 + d.offset,
                    half_width: d.width / 2.0,
                    top: d.top_height,
                    bottom: d.bottom_height,
                    center_shift,
                    radius_top,
                    radius_bottom,
                    power,
                    top_edge_squared: (d.width / 2.0).powi(2) + d.top_height.powi(2),
                    bottom_edge_squared: (d.width / 2.0).powi(2) + d.bottom_height.powi(2),
                    inradius: deltoid_radius * (1.0 - d.ellipse)
                        + (d.width / 2.0).min((d.top_height + d.bottom_height) / 2.0) * d.ellipse,
                    feather: ((1.0 - d.sharpness)
                        * d.width.min(d.top_height + d.bottom_height)
                        * 0.3)
                        .max(0.75 / radius.max(0.5)),
                }
            })
            .collect();
        // Include the deformed footprint in periodic culling and work accounting.
        let max_width_scale = settings.bottom_width_scale(1.0);
        let buckets = angular_buckets(
            layout.aspect,
            triangles.iter().map(|t| {
                (
                    t.x,
                    t.half_width * max_width_scale,
                    t.feather * max_width_scale,
                )
            }),
        );
        let deltoid_buckets = angular_buckets(
            layout.aspect,
            deltoids.iter().map(|d| {
                (
                    d.x,
                    d.half_width * max_width_scale,
                    d.feather * max_width_scale,
                )
            }),
        );
        Self {
            brightness: settings.brightness,
            invert_colors: settings.invert_colors,
            layout,
            triangles,
            buckets,
            deltoids,
            deltoid_buckets,
            radius,
            radial_blur: settings.radial_blur,
            angular_blur: settings.angular_blur,
            bottom_power: settings.bottom_power,
            bottom_stretch: settings.bottom_stretch,
            bottom_curve: settings.bottom_curve,
        }
    }
    fn has_blur(&self) -> bool {
        self.radial_blur > 0.0 || self.angular_blur > 0.0
    }
    fn raster_size(&self) -> (u32, u32) {
        let mut height = self.radius.ceil().clamp(8.0, 512.0);
        let mut width = (height * self.layout.aspect).ceil().clamp(8.0, 2048.0);
        let downscale = (262_144.0 / (width * height)).sqrt().min(1.0);
        width *= downscale;
        height *= downscale;
        (
            width.floor().max(1.0) as u32,
            height.floor().max(1.0) as u32,
        )
    }
    fn blurred(
        &self,
        stops: Option<&[(f64, Rgba<u8>)]>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RgbaImage, String> {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let (width, height) = self.raster_size();
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            if cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            for x in 0..width {
                let (coverage, input, brightness) = self.sample_brightness(
                    (f64::from(x) + 0.5) / f64::from(width) * self.layout.aspect,
                    (f64::from(y) + 0.5) / f64::from(height),
                    stops.is_some(),
                );
                let mut pixel = stops.map_or(Rgba([255; 4]), |stops| {
                    crate::renderer::sample_gradient(stops, input)
                });
                if stops.is_some() {
                    self.color(&mut pixel, brightness);
                } else {
                    for channel in 0..3 {
                        pixel[channel] = (brightness * 127.5).round() as u8;
                    }
                }
                pixel[3] = (f64::from(pixel[3]) * coverage).round() as u8;
                if pixel[3] == 0 {
                    pixel = Rgba([0; 4]);
                }
                image.put_pixel(x, y, pixel);
            }
        }
        let horizontal = (self.angular_blur * f64::from(width) / 24.0).ceil() as u32;
        let vertical = (self.radial_blur * f64::from(height) / 6.0).ceil() as u32;
        for _ in 0..3 {
            if horizontal > 0 {
                image = crate::radial_blur::axis(&image, horizontal, true, cancelled)?;
            }
            if vertical > 0 {
                image = crate::radial_blur::axis(&image, vertical, false, cancelled)?;
            }
        }
        Ok(image)
    }
    fn work(&self, width: u32, height: u32) -> Result<u64, String> {
        let sample_work = if self.bottom_power == 0.0 || self.bottom_stretch == 0.0 {
            1
        } else {
            match self.bottom_curve {
                BottomCurve::Linear => 3,
                BottomCurve::Quadratic => 4,
                BottomCurve::Logarithmic => 8,
            }
        };
        let candidate_work = self
            .buckets
            .iter()
            .zip(&self.deltoid_buckets)
            .map(|(triangles, deltoids)| {
                triangles.len() as u64 * 3
                    + deltoids
                        .iter()
                        .map(|&index| {
                            if self.deltoids[index].power > 1.0 {
                                8
                            } else {
                                3
                            }
                        })
                        .sum::<u64>()
            })
            .max()
            .unwrap_or(0);
        // Charge the rounded p-norm evaluations more than straight-edge SDFs.
        let pixels = u64::from(width).checked_mul(u64::from(height));
        let work = if self.has_blur() {
            let (w, h) = self.raster_size();
            let axes = u64::from(self.radial_blur > 0.0) + u64::from(self.angular_blur > 0.0);
            let strip_work =
                u64::from(w) * u64::from(h) * (sample_work + candidate_work + axes * 3 * 8);
            pixels
                .and_then(|pixels| pixels.checked_mul(8))
                .and_then(|work| work.checked_add(strip_work))
        } else {
            pixels.and_then(|pixels| pixels.checked_mul(sample_work + candidate_work))
        };
        if width == 0 || height == 0 || work.is_none_or(|work| work > 128_000_000) {
            return Err("Explosion exceeds the render work budget; reduce count, shape width or output resolution".into());
        }
        Ok(work.expect("checked work budget"))
    }
    #[cfg(test)]
    fn coverage(&self, x: f64, y: f64) -> f64 {
        self.sample(x, y, false).0
    }
    #[cfg(test)]
    fn sample(&self, x: f64, y: f64, sdf_gradient: bool) -> (f64, f64) {
        let (coverage, input, _) = self.sample_brightness(x, y, sdf_gradient);
        (coverage, input)
    }
    fn color(&self, pixel: &mut Rgba<u8>, brightness: f64) {
        for channel in 0..3 {
            let value = if self.invert_colors {
                255 - pixel[channel]
            } else {
                pixel[channel]
            };
            pixel[channel] = (f64::from(value) * self.brightness * brightness)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    fn sample_brightness(&self, x: f64, y: f64, sdf_gradient: bool) -> (f64, f64, f64) {
        let width_scale = self
            .bottom_curve
            .width_scale(self.bottom_power, self.bottom_stretch, y);
        let bucket = ((x / self.layout.aspect * BUCKETS as f64) as usize).min(BUCKETS - 1);
        let mut coverage = 0.0_f64;
        let mut gradient = 0.0_f64;
        let mut brightness = 1.0;
        let mut current_pair = usize::MAX;
        let mut pair_coverage = 0.0;
        for &index in &self.buckets[bucket] {
            let t = &self.triangles[index];
            if current_pair != t.pair {
                current_pair = t.pair;
                pair_coverage = 0.0;
            }
            let px = periodic_delta(x - t.x, self.layout.aspect).abs() / width_scale;
            let py = (y - t.base) * t.direction;
            if px > t.half_width + t.feather || py < -t.feather || py > t.height + t.feather {
                continue;
            }
            // Exact signed distance to the symmetric isosceles triangle.
            let base = (px - t.half_width).max(0.0).hypot(py);
            let along = (((px - t.half_width) * -t.half_width + py * t.height) / t.edge_squared)
                .clamp(0.0, 1.0);
            let side = (px - t.half_width + t.half_width * along).hypot(py - t.height * along);
            let inside = py >= 0.0 && py <= t.height && px <= t.half_width * (1.0 - py / t.height);
            let distance = base.min(side) * if inside { -1.0 } else { 1.0 };
            if sdf_gradient {
                gradient = gradient.max(sdf_input(distance, t.inradius));
            }
            let alpha = (0.5 - distance / (2.0 * t.feather)).clamp(0.0, 1.0);
            let alpha = alpha * alpha * (3.0 - 2.0 * alpha);
            // Close a shared base only for opposite-facing halves. Overlapping
            // halves (including flipped ones) remain an SDF union, not additive.
            pair_coverage = if t.join_pair {
                pair_coverage + alpha
            } else {
                pair_coverage.max(alpha)
            };
            if pair_coverage.min(1.0) > coverage {
                brightness = t.brightness;
            }
            coverage = coverage.max(pair_coverage.min(1.0));
        }
        for &index in &self.deltoid_buckets[bucket] {
            let (alpha, input) =
                self.deltoids[index].sample(x, y, self.layout.aspect, width_scale, sdf_gradient);
            if alpha > coverage {
                brightness = self.deltoids[index].brightness;
            }
            coverage = coverage.max(alpha);
            gradient = gradient.max(input);
        }
        (coverage, gradient, brightness)
    }
}
fn sdf_input(distance: f64, inradius: f64) -> f64 {
    (-distance / inradius.max(f64::EPSILON)).clamp(0.0, 1.0)
}
fn angular_buckets(aspect: f64, shapes: impl Iterator<Item = (f64, f64, f64)>) -> Vec<Vec<usize>> {
    let mut buckets = vec![Vec::new(); BUCKETS];
    for (index, (x, half_width, feather)) in shapes.enumerate() {
        for (bucket, entries) in buckets.iter_mut().enumerate() {
            let center = (bucket as f64 + 0.5) / BUCKETS as f64 * aspect;
            if periodic_delta(center - x, aspect).abs()
                <= half_width + feather + aspect / (2.0 * BUCKETS as f64)
            {
                entries.push(index);
            }
        }
    }
    buckets
}
fn periodic_delta(delta: f64, period: f64) -> f64 {
    (delta + period * 0.5).rem_euclid(period) - period * 0.5
}
fn polar_radius(settings: &ExplosionSettings, width: u32, height: u32, scale: f64) -> f64 {
    let radius = if settings.cover_canvas {
        f64::from(width).hypot(f64::from(height)) * 0.5
    } else {
        f64::from(width.min(height)) * 0.5
    };
    radius * scale
}

pub(crate) fn estimate(
    value: &Value,
    instance: &str,
    width: u32,
    height: u32,
) -> Result<u64, String> {
    let settings = ExplosionSettings::from_value(value)?;
    let scale = layout(&settings, instance).scale;
    Strip::new(
        &settings,
        instance,
        polar_radius(&settings, width, height, scale),
    )
    .work(width, height)
}
pub(crate) fn render(
    value: &Value,
    instance: &str,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let settings = ExplosionSettings::from_value(value)?;
    if width == 0 || height == 0 || cancelled() {
        return Err("Icon rendering cancelled or empty".into());
    }
    let radius = polar_radius(&settings, width, height, layout(&settings, instance).scale);
    let strip = Strip::new(&settings, instance, radius);
    strip.work(width, height)?;
    let sdf_gradient = settings.gradient["kind"] == "SDF";
    let stops = if sdf_gradient {
        Some(crate::renderer::gradient_stops(&settings.gradient)?)
    } else {
        None
    };
    let blurred = if strip.has_blur() {
        Some(strip.blurred(stops.as_deref(), cancelled)?)
    } else {
        None
    };
    // Spatial gradients use final canvas coordinates; SDF colors follow the
    // strip-space shape distance through the same polar projection as its mask.
    let mut image = if sdf_gradient {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        RgbaImage::new(width, height)
    } else {
        crate::renderer::gradient_image(&settings.gradient, width, height, cancelled)?
    };
    for (y, mut row) in image.enumerate_rows_mut() {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for (x, _, pixel) in &mut row {
            let dx = f64::from(x) + 0.5 - f64::from(width) * 0.5;
            let dy = f64::from(y) + 0.5 - f64::from(height) * 0.5;
            let distance = dx.hypot(dy);
            let radial = distance / radius;
            let outer_coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
            if outer_coverage == 0.0 {
                *pixel = Rgba([0; 4]);
                continue;
            }
            let angular = (dy.atan2(dx) / std::f64::consts::TAU + 0.25).rem_euclid(1.0);
            if let Some(blurred) = &blurred {
                let sample = crate::radial_blur::wrapped_sample(
                    blurred,
                    angular * f64::from(blurred.width()) - 0.5,
                    (1.0 - radial) * f64::from(blurred.height()) - 0.5,
                );
                if sdf_gradient {
                    *pixel = sample;
                    pixel[3] = (f64::from(pixel[3]) * outer_coverage).round() as u8;
                } else {
                    let intensity = if settings.brightness_min == settings.brightness_max {
                        settings.brightness_min
                    } else {
                        f64::from(sample[0]) / 127.5
                    };
                    strip.color(pixel, intensity);
                    pixel[3] = (f64::from(pixel[3]) * f64::from(sample[3]) / 255.0 * outer_coverage)
                        .round() as u8;
                }
                if pixel[3] == 0 {
                    *pixel = Rgba([0; 4]);
                }
                continue;
            }
            // Top of the strip maps outward, bottom inward; X wraps continuously.
            let (coverage, input, brightness) =
                strip.sample_brightness(angular * strip.layout.aspect, 1.0 - radial, sdf_gradient);
            let coverage = coverage * outer_coverage;
            if let Some(stops) = &stops {
                *pixel = crate::renderer::sample_gradient(stops, input);
            }
            strip.color(pixel, brightness);
            pixel[3] = (f64::from(pixel[3]) * coverage).round() as u8;
            if pixel[3] == 0 {
                *pixel = Rgba([0; 4]);
            }
        }
    }
    Ok(image)
}

/// The same SDF sampling and pre-polar blur used by the final explosion.
pub fn construction(
    value: &Value,
    instance: &str,
    width: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if !(8..=512).contains(&width) || cancelled() {
        return Err("Invalid or cancelled construction preview".into());
    }
    let settings = ExplosionSettings::from_value(value)?;
    let aspect = layout(&settings, instance).aspect;
    let height = (f64::from(width) / aspect).ceil().max(8.0) as u32;
    let strip = Strip::new(&settings, instance, f64::from(height));
    strip.work(width, height)?;
    let sdf = settings.gradient["kind"] == "SDF";
    let stops = sdf
        .then(|| crate::renderer::gradient_stops(&settings.gradient))
        .transpose()?;
    let mut image = strip.blurred(stops.as_deref(), cancelled)?;
    if !sdf {
        for pixel in image.pixels_mut() {
            for channel in 0..3 {
                pixel[channel] = (u16::from(pixel[channel]) * 2).min(255) as u8;
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explosion_brightness_inverts_rgb_preserves_alpha_layout_and_construction_softness() {
        let mut settings = ExplosionSettings::new_deltoid();
        settings.gradient = serde_json::json!({"kind":"SDF","stops":[{"offset":0,"colorHex":"#80406080"},{"offset":1,"colorHex":"#80406080"}]});
        let original_layout = layout(&settings, "brightness");
        let original = render(&serde_json::json!(settings), "brightness", 96, 96, &|| {
            false
        })
        .unwrap();
        settings.brightness = 0.5;
        settings.invert_colors = true;
        let changed = render(&serde_json::json!(settings), "brightness", 96, 96, &|| {
            false
        })
        .unwrap();
        for (before, after) in original.pixels().zip(changed.pixels()) {
            assert_eq!(before[3], after[3]);
            if before[3] > 0 {
                assert_eq!(&after.0[..3], &[96, 80, 64]);
            }
        }
        settings.brightness_min = 0.2;
        settings.brightness_max = 1.5;
        let varied = layout(&settings, "brightness");
        assert_eq!(varied, layout(&settings, "brightness"));
        assert!(
            varied
                .deltoids
                .windows(2)
                .any(|pair| pair[0].brightness != pair[1].brightness)
        );
        for (a, b) in original_layout.deltoids.iter().zip(&varied.deltoids) {
            assert_eq!(
                (
                    a.x,
                    a.width,
                    a.top_height,
                    a.bottom_height,
                    a.offset,
                    a.sharpness
                ),
                (
                    b.x,
                    b.width,
                    b.top_height,
                    b.bottom_height,
                    b.offset,
                    b.sharpness
                )
            );
        }
        settings.deltoid.as_mut().unwrap().sharpness_min = 0.0;
        settings.deltoid.as_mut().unwrap().sharpness_max = 0.0;
        let soft =
            construction(&serde_json::json!(settings), "brightness", 280, &|| false).unwrap();
        assert!(soft.pixels().any(|p| p[3] > 0 && p[3] < 128));
        assert!(soft.pixels().any(|p| p[3] == 0));
        settings.angular_blur = 0.4;
        assert_ne!(
            soft,
            construction(&serde_json::json!(settings), "brightness", 280, &|| false).unwrap()
        );
        assert!(construction(&serde_json::json!(settings), "brightness", 280, &|| true).is_err());
        settings.brightness_min = 1.6;
        settings.brightness_max = 1.0;
        assert!(settings.validate().is_err());
    }
    #[test]
    fn explosion_construction_blur_has_independent_axes_seam_and_zero_identity() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            aspect_min: 3.0,
            aspect_max: 3.0,
            ..ExplosionSettings::new_deltoid()
        };
        let original = render(&serde_json::json!(settings), "blur", 96, 96, &|| false).unwrap();
        settings.radial_blur = 0.6;
        let radial = render(&serde_json::json!(settings), "blur", 96, 96, &|| false).unwrap();
        settings.radial_blur = 0.0;
        settings.angular_blur = 0.6;
        let angular = render(&serde_json::json!(settings), "blur", 96, 96, &|| false).unwrap();
        assert_ne!(radial, original);
        assert_ne!(angular, original);
        assert_ne!(radial, angular);
        settings.radial_blur = 0.4;
        settings.gradient["kind"] = serde_json::json!("SDF");
        let strip = Strip::new(&settings, "blur", 128.0);
        let stops = crate::renderer::gradient_stops(&settings.gradient).unwrap();
        let texture = strip.blurred(Some(&stops), &|| false).unwrap();
        assert!(texture.width() * texture.height() <= 262_144);
        for y in [0.0, 20.25, 60.5] {
            assert_eq!(
                crate::radial_blur::wrapped_sample(&texture, -0.25, y),
                crate::radial_blur::wrapped_sample(&texture, f64::from(texture.width()) - 0.25, y)
            );
        }
        let image = render(&serde_json::json!(settings), "blur", 96, 96, &|| false).unwrap();
        assert!(image.pixels().any(|p| p[3] > 0));
        assert!(strip.blurred(Some(&stops), &|| true).is_err());
        settings.radial_blur = 0.0;
        settings.angular_blur = 0.0;
        settings.gradient["kind"] = serde_json::json!("Radial");
        assert_eq!(
            render(&serde_json::json!(settings), "blur", 96, 96, &|| false).unwrap(),
            original
        );
    }
    #[test]
    fn explosion_morph_outline_matches_distance_field_and_ellipse_bounds() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            aspect_min: 10.0,
            aspect_max: 10.0,
            shape_x: 2.0,
            shape_y: 3.0,
            ..ExplosionSettings::new_deltoid()
        };
        for (top, bottom) in [(0.3, 0.3), (0.1, 0.6), (0.0, 0.4)] {
            let shape = settings.deltoid.as_mut().unwrap();
            shape.width_min = 0.4;
            shape.width_max = 0.4;
            shape.top_height_min = top;
            shape.top_height_max = top;
            shape.bottom_height_min = bottom;
            shape.bottom_height_max = bottom;
            shape.sharpness_min = 1.0;
            shape.sharpness_max = 1.0;
            for morph in [0.0, 0.25, 0.5, 0.75, 1.0] {
                settings.deltoid.as_mut().unwrap().ellipse = morph;
                let strip = Strip::new(&settings, "shape", 1024.0);
                let sampled = &strip.layout.deltoids[0];
                let shape = &strip.deltoids[0];
                for [x, y] in sampled.outline() {
                    let distance = shape.distance((x - sampled.x).abs(), y - shape.waist);
                    assert!(
                        distance.is_finite() && distance.abs() < 1e-10,
                        "Morph {morph} outline diverged: {distance}"
                    );
                }
                for x in [0.0, 0.1, 0.3] {
                    for y in [-0.4, -0.1, 0.0, 0.2, 0.5] {
                        let left = shape.sample(
                            shape.x - x,
                            shape.waist + y,
                            strip.layout.aspect,
                            1.0,
                            true,
                        );
                        let right = shape.sample(
                            shape.x + x,
                            shape.waist + y,
                            strip.layout.aspect,
                            1.0,
                            true,
                        );
                        assert!(
                            (left.0 - right.0).abs() < 1e-10 && (left.1 - right.1).abs() < 1e-10
                        );
                        assert!(left.0.is_finite() && left.1.is_finite());
                    }
                }
                assert!(shape.distance(0.0, (shape.bottom - shape.top) * 0.5) < 0.0);
                if morph == 1.0 {
                    assert!((shape.radius_top - shape.radius_bottom).abs() < 1e-12);
                    let (alpha, input) = shape.sample(
                        shape.x,
                        shape.waist + shape.center_shift,
                        strip.layout.aspect,
                        1.0,
                        true,
                    );
                    assert_eq!((alpha, input), (1.0, 1.0));
                }
            }
        }
    }
    #[test]
    fn explosion_512_rounded_objects_render_and_keep_work_and_cancellation_limits() {
        let mut settings = ExplosionSettings {
            count: 512,
            ..ExplosionSettings::new_deltoid()
        };
        let straight = estimate(&serde_json::json!(settings), "512", 128, 128).unwrap();
        settings.deltoid.as_mut().unwrap().ellipse = 1.0;
        let rounded = estimate(&serde_json::json!(settings), "512", 128, 128).unwrap();
        assert!(rounded > straight);
        let image = render(&serde_json::json!(settings), "512", 128, 128, &|| false).unwrap();
        assert!(image.pixels().any(|pixel| pixel[3] > 0));
        assert!(render(&serde_json::json!(settings), "512", 128, 128, &|| true).is_err());
        settings.shape_x = 10.0;
        settings.deltoid.as_mut().unwrap().width_min = 2.0;
        settings.deltoid.as_mut().unwrap().width_max = 2.0;
        assert!(
            estimate(&serde_json::json!(settings), "512", 512, 512)
                .unwrap_err()
                .contains("budget")
        );
    }
    #[test]
    fn explosion_sdf_gradient_uses_interior_distance_and_survives_shape_scaling() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            ..ExplosionSettings::new_deltoid()
        };
        let shape = settings.deltoid.as_mut().unwrap();
        shape.width_min = 0.6;
        shape.width_max = 0.6;
        shape.top_height_min = 0.3;
        shape.top_height_max = 0.3;
        shape.bottom_height_min = 0.3;
        shape.bottom_height_max = 0.3;
        shape.sharpness_min = 1.0;
        shape.sharpness_max = 1.0;
        for scale in [0.5, 1.0, 1.5] {
            settings.deltoid.as_mut().unwrap().scale = scale;
            let strip = Strip::new(&settings, "sdf", 1024.0);
            let shape = &strip.deltoids[0];
            let (center_alpha, center) = strip.sample(shape.x, shape.waist, true);
            let (middle_alpha, middle) =
                strip.sample(shape.x + shape.half_width * 0.5, shape.waist, true);
            let (_, edge) = strip.sample(shape.x + shape.half_width, shape.waist, true);
            assert_eq!(center_alpha, 1.0);
            assert_eq!(middle_alpha, 1.0);
            assert!((center - 1.0).abs() < 1e-12);
            assert!((middle - 0.5).abs() < 1e-12);
            assert!(edge < 1e-12);
            settings.deltoid.as_mut().unwrap().sharpness_min = 0.0;
            settings.deltoid.as_mut().unwrap().sharpness_max = 0.0;
            let soft = Strip::new(&settings, "sdf", 1024.0);
            assert_eq!(
                strip
                    .sample(shape.x + shape.half_width * 0.5, shape.waist, true)
                    .1,
                soft.sample(shape.x + shape.half_width * 0.5, shape.waist, true)
                    .1
            );
            settings.deltoid.as_mut().unwrap().sharpness_min = 1.0;
            settings.deltoid.as_mut().unwrap().sharpness_max = 1.0;
        }
    }
    #[test]
    fn explosion_default_polar_circle_covers_square_and_rectangular_canvas_corners() {
        let settings = ExplosionSettings::new_deltoid();
        assert_eq!((settings.scale_min, settings.scale_max), (1.0, 1.0));
        for (width, height) in [(128, 128), (320, 80), (80, 320)] {
            let radius = polar_radius(&settings, width, height, 1.0);
            let corner = (f64::from(width) / 2.0).hypot(f64::from(height) / 2.0);
            assert_eq!(radius, corner);
            let pixel_corner = (f64::from(width) / 2.0 - 0.5).hypot(f64::from(height) / 2.0 - 0.5);
            assert_eq!((radius + 0.5 - pixel_corner).clamp(0.0, 1.0), 1.0);
            assert_eq!(polar_radius(&settings, width, height, 0.5), radius * 0.5);
            assert_eq!(
                polar_radius(&ExplosionSettings::default(), width, height, 0.9),
                f64::from(width.min(height)) * 0.45
            );
        }
    }
    #[test]
    fn explosion_deltoid_sdf_has_one_symmetric_perimeter_and_independent_tips() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            ..ExplosionSettings::new_deltoid()
        };
        let shape = settings.deltoid.as_mut().unwrap();
        shape.width_min = 0.4;
        shape.width_max = 0.4;
        shape.top_height_min = 0.2;
        shape.top_height_max = 0.2;
        shape.bottom_height_min = 0.35;
        shape.bottom_height_max = 0.35;
        shape.offset_min = 0.05;
        shape.offset_max = 0.05;
        shape.sharpness_min = 1.0;
        shape.sharpness_max = 1.0;
        let strip = Strip::new(&settings, "kite", 1024.0);
        assert!(strip.triangles.is_empty());
        let d = &strip.deltoids[0];
        assert_eq!(strip.coverage(d.x, d.waist), 1.0);
        for y in [
            d.waist - 0.15,
            d.waist - 1e-8,
            d.waist,
            d.waist + 1e-8,
            d.waist + 0.2,
        ] {
            for dx in [0.0, 0.05, 0.1, 0.15, 0.2] {
                assert!((strip.coverage(d.x - dx, y) - strip.coverage(d.x + dx, y)).abs() < 1e-10);
            }
        }
        assert_eq!(strip.coverage(d.x, d.waist - 0.21), 0.0);
        assert_eq!(strip.coverage(d.x, d.waist + 0.36), 0.0);
        let x = d.x;
        settings.deltoid.as_mut().unwrap().top_height_min = 0.0;
        settings.deltoid.as_mut().unwrap().top_height_max = 0.0;
        let flat_top = Strip::new(&settings, "kite", 1024.0);
        assert_eq!(flat_top.coverage(x, 0.45), 0.0);
        assert_eq!(flat_top.coverage(x, 0.8), strip.coverage(x, 0.8));
        assert!((flat_top.coverage(x + 0.1, 0.55) - 0.5).abs() < 1e-12);
        settings.deltoid.as_mut().unwrap().bottom_height_min = 0.0;
        settings.deltoid.as_mut().unwrap().bottom_height_max = 0.0;
        let empty = render(&serde_json::json!(settings), "empty", 64, 64, &|| false).unwrap();
        assert!(empty.pixels().all(|p| p[3] == 0));
    }
    #[test]
    fn explosion_flips_reflect_in_place_for_both_triangle_sets() {
        for outward in [true, false] {
            let mut settings = ExplosionSettings {
                count: 1,
                regularity: 0.0,
                disable_out: !outward,
                disable_in: outward,
                ..Default::default()
            };
            let triangle = if outward {
                &mut settings.out
            } else {
                &mut settings.r#in
            };
            triangle.offset_min = 0.1;
            triangle.offset_max = 0.2;
            triangle.scale = 0.8;
            triangle.scale_min = 0.6;
            triangle.scale_max = 1.4;
            let plain = Strip::new(&settings, "flip", 128.0);
            if outward {
                settings.out.flip = true;
            } else {
                settings.r#in.flip = true;
            }
            let flipped = Strip::new(&settings, "flip", 128.0);
            let triangle = &plain.triangles[0];
            let tip = triangle.base + triangle.direction * triangle.height;
            assert_eq!(flipped.triangles[0].base, tip);
            assert_eq!(flipped.triangles[0].direction, -triangle.direction);
            for along in [-0.2, 0.0, 0.1, 0.5, 0.9, 1.0, 1.2] {
                for across in [-1.2, -0.8, 0.0, 0.8, 1.2] {
                    let x = triangle.x + across * triangle.half_width;
                    let y = triangle.base + triangle.direction * triangle.height * along;
                    let reflected_y = triangle.base + tip - y;
                    assert!(
                        (plain.coverage(x, y) - flipped.coverage(x, reflected_y)).abs() < 1e-12
                    );
                }
            }
            // A wide section at the original base becomes narrow after flipping.
            let x = triangle.x + triangle.half_width * 0.7;
            let y = triangle.base + triangle.direction * triangle.height * 0.1;
            assert!(plain.coverage(x, y) > flipped.coverage(x, y));
        }
    }
    #[test]
    fn explosion_overlapping_halves_do_not_add_opacity() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            ..Default::default()
        };
        settings.out.flip = true;
        settings.out.width_min = 0.3;
        settings.out.width_max = 0.3;
        settings.out.height_min = 0.3;
        settings.out.height_max = 0.3;
        settings.out.sharpness_min = 0.0;
        settings.out.sharpness_max = 0.0;
        settings.r#in = settings.out.clone();
        settings.r#in.flip = false;
        // Align the inward base with the flipped outward base to test an exact overlap.
        settings.r#in.offset_min = -0.3;
        settings.r#in.offset_max = -0.3;
        let both = render(&serde_json::json!(settings), "overlap", 128, 128, &|| false).unwrap();
        settings.disable_out = true;
        let single = render(&serde_json::json!(settings), "overlap", 128, 128, &|| false).unwrap();
        assert!(
            both == single,
            "Identical overlapping triangles must be a union, including soft edges"
        );
        settings.disable_out = false;
        settings.out.flip = false;
        settings.out.offset_min = -0.1;
        settings.out.offset_max = -0.1;
        settings.r#in.offset_min = -0.1;
        settings.r#in.offset_max = -0.1;
        let both = render(&serde_json::json!(settings), "overlap", 128, 128, &|| false).unwrap();
        settings.disable_out = true;
        let inward = render(&serde_json::json!(settings), "overlap", 128, 128, &|| false).unwrap();
        settings.disable_out = false;
        settings.disable_in = true;
        let outward = render(&serde_json::json!(settings), "overlap", 128, 128, &|| false).unwrap();
        for ((both, inward), outward) in both.pixels().zip(inward.pixels()).zip(outward.pixels()) {
            assert_eq!(both[3], inward[3].max(outward[3]));
        }
    }
    #[test]
    fn explosion_rejects_overflowing_work_and_cancels_during_gradient_generation() {
        let settings = serde_json::json!(ExplosionSettings::default());
        assert!(
            estimate(&settings, "budget", u32::MAX, u32::MAX)
                .unwrap_err()
                .contains("budget")
        );
        assert!(estimate(&settings, "budget", 0, 512).is_err());
        let calls = std::cell::Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= 4
        };
        assert!(
            render(&settings, "gradient", 512, 512, &cancelled)
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(calls.get(), 4);
    }
    #[test]
    fn explosion_scaled_triangles_antialias_the_outer_polar_boundary() {
        let mut settings = ExplosionSettings {
            count: 1,
            regularity: 0.0,
            aspect_min: 1.0,
            aspect_max: 1.0,
            disable_in: true,
            ..Default::default()
        };
        settings.out.width_min = 2.0;
        settings.out.width_max = 2.0;
        settings.out.height_min = 1.0;
        settings.out.height_max = 1.0;
        settings.out.scale = 2.0;
        let image = render(&serde_json::json!(settings), "clip", 128, 128, &|| false).unwrap();
        let radius = 128.0 * 0.5 * 0.9;
        let mut partial = 0;
        for (x, y, pixel) in image.enumerate_pixels() {
            let distance = (f64::from(x) + 0.5 - 64.0).hypot(f64::from(y) + 0.5 - 64.0);
            if distance > radius + 0.5 {
                assert_eq!(pixel[3], 0);
            }
            if (distance - radius).abs() < 0.5 && pixel[3] > 0 && pixel[3] < 255 {
                partial += 1;
            }
        }
        assert!(
            partial > 10,
            "The clipped circumference needs partial coverage pixels"
        );
    }
    #[test]
    fn explosion_strip_closes_paired_bases_and_wraps_the_polar_seam() {
        let settings = ExplosionSettings {
            count: 1,
            aspect_min: 1.0,
            aspect_max: 1.0,
            regularity: 0.0,
            ..Default::default()
        };
        let strip = Strip::new(&settings, "seam", 128.0);
        let x = strip.layout.pairs[0].x;
        assert_eq!(strip.coverage(x, 0.5), 1.0);
        let settings = ExplosionSettings {
            count: 100,
            regularity: 1.0,
            aspect_min: 1.0,
            aspect_max: 1.0,
            ..Default::default()
        };
        let strip = Strip::new(&settings, "seam", 128.0);
        for y in [0.3, 0.45, 0.5, 0.55, 0.7] {
            assert!((strip.coverage(1e-10, y) - strip.coverage(1.0 - 1e-10, y)).abs() < 1e-6);
        }
    }
    #[test]
    fn explosion_bottom_scaling_widens_each_shape_preserves_top_and_wraps_seam() {
        for ellipse in [None, Some(0.0), Some(0.5), Some(1.0)] {
            let mut settings = ExplosionSettings {
                count: 1,
                regularity: 0.0,
                aspect_min: 2.0,
                aspect_max: 2.0,
                ..ExplosionSettings::default()
            };
            settings.out.width_min = 0.4;
            settings.out.width_max = 0.4;
            settings.r#in = settings.out.clone();
            settings.r#in.height_min = 0.8;
            settings.r#in.height_max = 0.8;
            if let Some(ellipse) = ellipse {
                settings.deltoid = Some(achimeld_application::explosion::DeltoidSettings {
                    ellipse,
                    width_min: 0.4,
                    width_max: 0.4,
                    top_height_min: 0.5,
                    top_height_max: 0.5,
                    bottom_height_min: 0.8,
                    bottom_height_max: 0.8,
                    sharpness_min: 1.0,
                    sharpness_max: 1.0,
                    ..Default::default()
                });
            }
            let original = Strip::new(&settings, "fill", 256.0);
            settings.bottom_power = 9.0;
            for curve in [
                BottomCurve::Linear,
                BottomCurve::Quadratic,
                BottomCurve::Logarithmic,
            ] {
                settings.bottom_curve = curve;
                let filled = Strip::new(&settings, "fill", 256.0);
                let mut all = Strip::new(&settings, "fill", 256.0);
                for bucket in &mut all.buckets {
                    *bucket = (0..all.triangles.len()).collect();
                }
                for bucket in &mut all.deltoid_buckets {
                    *bucket = (0..all.deltoids.len()).collect();
                }
                for y in [0.1, 0.3, 0.5] {
                    for x in 0..100 {
                        let x = f64::from(x) / 50.0;
                        assert_eq!(original.sample(x, y, true), filled.sample(x, y, true));
                    }
                }
                assert_eq!(original.coverage(1.4, 0.85), 0.0);
                assert_eq!(filled.coverage(1.4, 0.85), 1.0, "{ellipse:?}, {curve:?}");
                assert!(filled.work(128, 128).unwrap() >= original.work(128, 128).unwrap());
                for y in [0.55, 0.7, 0.95] {
                    assert!(
                        (filled.coverage(1e-10, y) - filled.coverage(2.0 - 1e-10, y)).abs() < 1e-6
                    );
                    for x in 0..100 {
                        // Compare bucketed samples with testing every object: widened
                        // shapes must never disappear outside their original bucket.
                        let x = f64::from(x) / 50.0;
                        assert_eq!(filled.sample(x, y, true), all.sample(x, y, true));
                    }
                }
            }
        }
        let mut dense = ExplosionSettings {
            count: 512,
            regularity: 0.0,
            aspect_min: 8.0,
            aspect_max: 8.0,
            deltoid: Some(achimeld_application::explosion::DeltoidSettings {
                width_min: 0.1,
                width_max: 0.1,
                ..Default::default()
            }),
            ..ExplosionSettings::new_deltoid()
        };
        assert!(estimate(&serde_json::json!(dense), "budget", 1024, 1024).is_ok());
        dense.bottom_power = 9.0;
        dense.bottom_curve = BottomCurve::Logarithmic;
        assert!(
            estimate(&serde_json::json!(dense), "budget", 1024, 1024)
                .unwrap_err()
                .contains("budget")
        );
    }
}
