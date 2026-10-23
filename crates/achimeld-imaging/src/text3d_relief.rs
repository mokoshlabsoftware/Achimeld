//! Bounded distance-field relief, with genuine silhouettes, bevels and self shadows.
//! The field retains glyph counters and handles contour merges/erosion without
//! introducing self-intersecting offset polygons. All work stays on the caller's worker.
use super::*;
#[path = "text3d_masks.rs"]
mod masks;

struct Field {
    mask: Mask,
    distance: Vec<f32>,
}
impl Field {
    fn new(
        path: &BezPath,
        scene: &Scene<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let mask = Mask::padded(
            path,
            if scene.settings.output == Text3dOutput::FullColor {
                scene.height
            } else {
                scene.height.saturating_mul(2)
            },
            scene.settings.push.max(0.0) + scene.settings.emboss + 4.0,
            1024,
        );
        let mut inside = Vec::with_capacity(mask.alpha.len());
        let mut outside = Vec::with_capacity(mask.alpha.len());
        for &alpha in &mask.alpha {
            let a = f32::from(alpha) / 255.0;
            inside.push(if alpha == 255 {
                1e20
            } else {
                (a - 0.5).max(0.0).powi(2)
            });
            outside.push(if alpha == 0 {
                1e20
            } else {
                (0.5 - a).max(0.0).powi(2)
            });
        }
        edt(
            &mut inside,
            mask.width as usize,
            mask.height as usize,
            cancelled,
        )?;
        edt(
            &mut outside,
            mask.width as usize,
            mask.height as usize,
            cancelled,
        )?;
        for (a, b) in inside.iter_mut().zip(outside) {
            *a = (a.sqrt() - b.sqrt()) / mask.scale as f32;
        }
        Ok(Self {
            mask,
            distance: inside,
        })
    }
    fn grid(&self, x: usize, y: usize) -> f64 {
        f64::from(self.distance[y * self.mask.width as usize + x])
    }
    fn coordinates(&self, x: f64, y: f64) -> (usize, usize, f64, f64, f64) {
        let gx = (x - self.mask.left) * self.mask.scale - 0.5;
        let gy = (self.mask.top - y) * self.mask.scale - 0.5;
        let cx = gx.clamp(0.0, f64::from(self.mask.width - 1) - 1e-6);
        let cy = gy.clamp(0.0, f64::from(self.mask.height - 1) - 1e-6);
        (
            cx as usize,
            cy as usize,
            cx.fract(),
            cy.fract(),
            (gx - cx).hypot(gy - cy) / self.mask.scale,
        )
    }
    fn sample(&self, x: f64, y: f64) -> f64 {
        let (ix, iy, fx, fy, beyond) = self.coordinates(x, y);
        let a = self.grid(ix, iy);
        let b = self.grid(ix + 1, iy);
        let c = self.grid(ix, iy + 1);
        let d = self.grid(ix + 1, iy + 1);
        // Piecewise linear interpolation has a well-defined flat normal per
        // triangle; smooth shading averages neighboring field samples.
        let value = if fx + fy <= 1.0 {
            a + (b - a) * fx + (c - a) * fy
        } else {
            d + (c - d) * (1.0 - fx) + (b - d) * (1.0 - fy)
        };
        value - beyond
    }
    fn gradient(&self, x: f64, y: f64, smooth: bool) -> V3 {
        if smooth {
            let step = 1.0 / self.mask.scale;
            return unit([
                self.sample(x - step, y) - self.sample(x + step, y),
                self.sample(x, y - step) - self.sample(x, y + step),
                0.0,
            ]);
        }
        let (ix, iy, fx, fy, _) = self.coordinates(x, y);
        let (dx, dy) = if fx + fy <= 1.0 {
            (
                self.grid(ix + 1, iy) - self.grid(ix, iy),
                self.grid(ix, iy + 1) - self.grid(ix, iy),
            )
        } else {
            (
                self.grid(ix + 1, iy + 1) - self.grid(ix, iy + 1),
                self.grid(ix + 1, iy + 1) - self.grid(ix + 1, iy),
            )
        };
        [-dx * self.mask.scale, dy * self.mask.scale, 0.0]
    }
}

// Separable squared Euclidean distance transform. Each row/column uses the
// lower envelope of parabolas: linear work and bounded scratch storage.
fn edt(
    data: &mut [f32],
    width: usize,
    height: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let length = width.max(height);
    let mut input = vec![0.0; length];
    let mut output = vec![0.0; length];
    let mut sites = vec![0usize; length];
    let mut boundaries = vec![0.0; length + 1];
    for vertical in [false, true] {
        let (count, size) = if vertical {
            (width, height)
        } else {
            (height, width)
        };
        for row in 0..count {
            if row % 16 == 0 && cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            let index = |i| {
                if vertical {
                    i * width + row
                } else {
                    row * width + i
                }
            };
            for i in 0..size {
                input[i] = f64::from(data[index(i)]);
            }
            let mut k = 0;
            sites[0] = 0;
            boundaries[0] = f64::NEG_INFINITY;
            boundaries[1] = f64::INFINITY;
            for q in 1..size {
                let mut intersection;
                loop {
                    let p = sites[k];
                    intersection =
                        ((input[q] - input[p]) + (q * q - p * p) as f64) / (2.0 * (q - p) as f64);
                    if intersection > boundaries[k] {
                        break;
                    }
                    k -= 1;
                }
                k += 1;
                sites[k] = q;
                boundaries[k] = intersection;
                boundaries[k + 1] = f64::INFINITY;
            }
            k = 0;
            for (q, value) in output.iter_mut().take(size).enumerate() {
                while boundaries[k + 1] < q as f64 {
                    k += 1;
                }
                *value = (q as f64 - sites[k] as f64).powi(2) + input[sites[k]];
            }
            for i in 0..size {
                data[index(i)] = output[i] as f32;
            }
        }
    }
    Ok(())
}

struct Model<'a> {
    field: Field,
    settings: &'a Text3dSettings,
    minimum: V3,
    maximum: V3,
}
impl Model<'_> {
    fn distance(&self, p: V3) -> (f64, Surface) {
        let d = -self.field.sample(p[0], p[1]) - self.settings.push;
        let wall = d - self.settings.emboss;
        let face = p[2].abs() - self.settings.extrusion / 2.0 - self.settings.emboss;
        if self.settings.emboss == 0.0 {
            return if face >= wall {
                (face, Surface::Face)
            } else {
                (wall, Surface::Side)
            };
        }
        let bevel = (d + p[2].abs() - self.settings.extrusion / 2.0 - self.settings.emboss)
            * std::f64::consts::FRAC_1_SQRT_2;
        if face >= wall && face >= bevel {
            (face, Surface::Face)
        } else if wall >= bevel {
            (wall, Surface::Side)
        } else {
            (bevel, Surface::Emboss)
        }
    }
    fn normal(&self, p: V3, surface: Surface, smooth: bool) -> V3 {
        if surface == Surface::Face {
            return [0.0, 0.0, p[2].signum()];
        }
        let mut normal = self.field.gradient(p[0], p[1], smooth);
        if surface == Surface::Emboss {
            if smooth {
                // Interpolate the rim's endpoint normals, joining the vertical
                // wall and flat face smoothly without changing any geometry.
                let d = -self.field.sample(p[0], p[1]) - self.settings.push;
                let angle = ((self.settings.emboss - d) / self.settings.emboss.max(1e-9))
                    .clamp(0.0, 1.0)
                    * std::f64::consts::FRAC_PI_2;
                let (sin, cos) = angle.sin_cos();
                normal = [normal[0] * cos, normal[1] * cos, p[2].signum() * sin];
            } else {
                normal[2] = p[2].signum();
            }
        }
        unit(normal)
    }
    fn interval(&self, origin: V3, direction: V3) -> Option<(f64, f64)> {
        let mut near: f64 = 0.0;
        let mut far = f64::INFINITY;
        for i in 0..3 {
            if direction[i].abs() < 1e-12 {
                if origin[i] < self.minimum[i] || origin[i] > self.maximum[i] {
                    return None;
                }
            } else {
                let a = (self.minimum[i] - origin[i]) / direction[i];
                let b = (self.maximum[i] - origin[i]) / direction[i];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
        }
        (far >= near).then_some((near, far))
    }
    fn visibility(&self, p: V3, normal: V3, light: V3, epsilon: f64, work: &mut u64) -> f64 {
        if dot(normal, light) <= 0.0 {
            return 1.0;
        }
        let origin = add(p, mul(normal, epsilon * 4.0));
        let Some((_, far)) = self.interval(origin, light) else {
            return 1.0;
        };
        let mut t = epsilon * 4.0;
        for _ in 0..64 {
            if t > far {
                return 1.0;
            }
            *work += 1;
            let d = self.distance(add(origin, mul(light, t))).0;
            if d < epsilon {
                return 1.0 - self.settings.shadow_strength;
            }
            t += (d * 0.7).max(epsilon);
        }
        1.0
    }
}

pub(super) fn render(
    scene: &Scene<'_>,
    path: &BezPath,
    color: Rgba<u8>,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if scene.settings.output == Text3dOutput::EmbossMask && scene.settings.emboss == 0.0 {
        return Ok(RgbaImage::new(scene.width, scene.height));
    }
    let field = Field::new(path, scene, cancelled)?;
    let padding = scene.settings.push.max(0.0) + scene.settings.emboss + 2.0 / field.mask.scale;
    let bounds = path.bounding_box().inflate(padding, padding);
    let half = scene.settings.extrusion / 2.0 + scene.settings.emboss;
    let model = Model {
        field,
        settings: scene.settings,
        minimum: [bounds.x0, bounds.y0, -half - 1e-4],
        maximum: [bounds.x1, bounds.y1, half + 1e-4],
    };
    if scene.settings.output != Text3dOutput::FullColor {
        return masks::render(scene, &model, cancelled);
    }
    let side = crate::renderer::rgba(&scene.settings.side_color_hex)?;
    let mut pixels = RgbaImage::new(scene.width, scene.height);
    let light = scene.inverse(scene.light);
    let mut work = 0u64;
    for y in 0..scene.height {
        if y % 4 == 0 && cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for x in 0..scene.width {
            let (origin, direction) = scene.ray(x, y);
            let origin = scene.inverse(sub(origin, scene.position));
            let length = dot(direction, direction).sqrt();
            let direction = scene.inverse(mul(direction, 1.0 / length));
            let Some((near, far)) = model.interval(origin, direction) else {
                continue;
            };
            let mut t = near.max(length); // The same one-unit camera clipping plane as vector text.
            for _ in 0..192 {
                if t > far {
                    break;
                }
                work += 1;
                let p = add(origin, mul(direction, t));
                let (d, surface) = model.distance(p);
                let pixel_size = match scene.settings.projection {
                    TextProjection::Orthographic => {
                        scene.settings.orthographic_size / f64::from(scene.height)
                    }
                    TextProjection::Perspective => {
                        (t / length) * 36.0
                            / (f64::from(scene.height) * scene.settings.focal_length)
                    }
                };
                let epsilon = (pixel_size * 0.08).clamp(0.01, 0.35 / model.field.mask.scale);
                if d.abs() <= epsilon {
                    let geometric_normal = model.normal(p, surface, false);
                    // Clipping can start inside a letter: an exiting back face is
                    // not a visible replacement for the missing near-clipped face.
                    if dot(geometric_normal, direction) < -1e-6 {
                        let normal = model.normal(p, surface, scene.settings.smooth_normals);
                        let material = if surface == Surface::Side {
                            side
                        } else {
                            color
                        };
                        let visibility = if scene.settings.output == Text3dOutput::FullColor
                            && scene.settings.shading != TextShading::Unlit
                            && scene.settings.shadow_strength > 0.0
                        {
                            model.visibility(
                                p,
                                geometric_normal,
                                light,
                                epsilon.max(0.2 / model.field.mask.scale),
                                &mut work,
                            )
                        } else {
                            1.0
                        };
                        let shade = scene.paint(
                            material,
                            scene.rotate(normal),
                            scene.rotate(mul(direction, -1.0)),
                            surface,
                            visibility,
                        );
                        pixels.put_pixel(x, y, shade);
                    }
                    break;
                }
                // Triangulated field interpolation and the max of the three
                // half-spaces are conservative distance estimates, with a safety
                // factor for sampled curves. Both rays and shadow rays are capped.
                t += (d.abs() * 0.7).max(epsilon * 0.5);
            }
            if work > 64_000_000 {
                return Err("3D text exceeds its relief work budget".into());
            }
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_transform_is_euclidean_and_cancellable() {
        let mut data = vec![1e20; 25];
        data[12] = 0.0;
        edt(&mut data, 5, 5, &|| false).unwrap();
        assert_eq!(data[0], 8.0);
        assert_eq!(data[7], 1.0);
        assert_eq!(data[12], 0.0);
        assert!(edt(&mut data, 5, 5, &|| true).is_err());
    }
}
