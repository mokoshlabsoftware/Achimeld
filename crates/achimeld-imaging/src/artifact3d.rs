//! Bounded triangle rasterization on the existing asynchronous preview workers.
use achimeld_application::{
    artifact3d::{ArtifactOutput, ArtifactPart, ArtifactSettings},
    text3d::{TextProjection, TextShading},
};
use image::{Rgba, RgbaImage};
#[path = "artifact3d_geometry.rs"]
mod geometry;

type V3 = [f64; 3];
fn add(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn mul(a: V3, b: f64) -> V3 {
    a.map(|v| v * b)
}
fn dot(a: V3, b: V3) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn unit(a: V3) -> V3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(1e-12))
}

struct Rotation([V3; 3]);
impl Rotation {
    fn new(angles: V3) -> Self {
        let rotate = |mut v: V3| {
            for (a, b, angle) in [(1, 2, angles[0]), (2, 0, angles[1]), (0, 1, angles[2])] {
                let (sin, cos) = angle.to_radians().sin_cos();
                (v[a], v[b]) = (cos * v[a] - sin * v[b], sin * v[a] + cos * v[b]);
            }
            v
        };
        Self([
            rotate([1.0, 0.0, 0.0]),
            rotate([0.0, 1.0, 0.0]),
            rotate([0.0, 0.0, 1.0]),
        ])
    }
    fn apply(&self, v: V3) -> V3 {
        add(
            add(mul(self.0[0], v[0]), mul(self.0[1], v[1])),
            mul(self.0[2], v[2]),
        )
    }
}

#[derive(Clone, Copy)]
struct Vertex {
    p: V3,
    n: V3,
}

pub(crate) fn render(
    value: &serde_json::Value,
    dimensions: (u32, u32),
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let settings = ArtifactSettings::from_value(value)?;
    if dimensions.0 == 0 || dimensions.1 == 0 || dimensions.0.max(dimensions.1) > 4096 {
        return Err("3D artifact dimensions exceed the render budget".into());
    }
    let samples = if dimensions.0.max(dimensions.1) <= 512 {
        2
    } else {
        1
    };
    let (width, height) = (dimensions.0 * samples, dimensions.1 * samples);
    let mut raster = Raster {
        image: RgbaImage::new(width, height),
        depths: vec![f32::INFINITY; (width * height) as usize],
        settings: &settings,
        work: 0,
        cancelled,
    };
    let global = Rotation::new(settings.rotation);
    let segments = match dimensions.0.max(dimensions.1) {
        0..=96 => 24,
        97..=256 => 40,
        _ => 64,
    };
    for part in settings.parts.iter().filter(|p| p.visible) {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let color = crate::renderer::rgba(&part.color_hex)?;
        if color[3] == 0 {
            continue;
        }
        let local = Rotation::new(part.rotation);
        let mesh = geometry::mesh(part, segments);
        for triangle in mesh {
            let triangle = triangle.map(|v| Vertex {
                p: add(
                    global.apply(add(
                        local.apply(std::array::from_fn(|i| v.p[i] * part.size[i] / 2.0)),
                        part.position,
                    )),
                    settings.position,
                ),
                // Inverse transpose preserves normals under nonuniform scaling.
                n: unit(global.apply(local.apply(std::array::from_fn(|i| v.n[i] / part.size[i])))),
            });
            raster.clipped_triangle(triangle, part, color)?;
        }
    }
    if samples == 1 {
        return Ok(raster.image);
    }
    // Premultiplied downsampling retains material colors at transparent edges.
    Ok(RgbaImage::from_fn(dimensions.0, dimensions.1, |x, y| {
        let mut sums = [0u32; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let p = raster.image.get_pixel(x * 2 + dx, y * 2 + dy);
                for i in 0..3 {
                    sums[i] += u32::from(p[i]) * u32::from(p[3]);
                }
                sums[3] += u32::from(p[3]);
            }
        }
        Rgba([
            (sums[0] / sums[3].max(1)) as u8,
            (sums[1] / sums[3].max(1)) as u8,
            (sums[2] / sums[3].max(1)) as u8,
            ((sums[3] + 2) / 4) as u8,
        ])
    }))
}

struct Raster<'a> {
    image: RgbaImage,
    depths: Vec<f32>,
    settings: &'a ArtifactSettings,
    work: u64,
    cancelled: &'a dyn Fn() -> bool,
}
impl Raster<'_> {
    fn clipped_triangle(
        &mut self,
        triangle: [Vertex; 3],
        part: &ArtifactPart,
        color: Rgba<u8>,
    ) -> Result<(), String> {
        if (self.cancelled)() {
            return Err("Icon rendering cancelled".into());
        }
        // Clip in world space before perspective division; a camera crossing
        // must never generate infinite screen bounds or discard a visible face.
        let limit = self.settings.camera_distance - 1.0;
        let mut polygon = Vec::with_capacity(4);
        for i in 0..3 {
            let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
            if a.p[2] <= limit {
                polygon.push(a);
            }
            if (a.p[2] <= limit) != (b.p[2] <= limit) {
                let t = (limit - a.p[2]) / (b.p[2] - a.p[2]);
                polygon.push(Vertex {
                    p: add(a.p, mul(sub(b.p, a.p), t)),
                    n: add(a.n, mul(sub(b.n, a.n), t)),
                });
            }
        }
        for i in 1..polygon.len().saturating_sub(1) {
            self.triangle([polygon[0], polygon[i], polygon[i + 1]], part, color)?;
        }
        Ok(())
    }
    fn project(&self, p: V3) -> V3 {
        let depth = self.settings.camera_distance - p[2];
        let scale = f64::from(self.image.height())
            * match self.settings.projection {
                TextProjection::Perspective => self.settings.focal_length / (36.0 * depth),
                TextProjection::Orthographic => 1.0 / self.settings.orthographic_size,
            };
        [
            f64::from(self.image.width()) / 2.0 + p[0] * scale,
            f64::from(self.image.height()) / 2.0 - p[1] * scale,
            depth,
        ]
    }
    fn triangle(
        &mut self,
        vertices: [Vertex; 3],
        part: &ArtifactPart,
        color: Rgba<u8>,
    ) -> Result<(), String> {
        let p = vertices.map(|v| self.project(v.p));
        let edge =
            |a: V3, b: V3, x: f64, y: f64| (x - a[0]) * (b[1] - a[1]) - (y - a[1]) * (b[0] - a[0]);
        let area = edge(p[0], p[1], p[2][0], p[2][1]);
        if area.abs() < 1e-9 {
            return Ok(());
        }
        let bounds = |axis: usize, max: u32| {
            let lo = p
                .iter()
                .map(|v| v[axis])
                .fold(f64::INFINITY, f64::min)
                .floor()
                .clamp(0.0, f64::from(max)) as u32;
            let hi = p
                .iter()
                .map(|v| v[axis])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .clamp(0.0, f64::from(max)) as u32;
            (lo, hi)
        };
        let (left, right) = bounds(0, self.image.width());
        let (top, bottom) = bounds(1, self.image.height());
        self.work += u64::from(right - left) * u64::from(bottom - top);
        if self.work > 160_000_000 {
            return Err("3D artifact exceeds its rasterization work budget".into());
        }
        for y in top..bottom {
            if (self.cancelled)() {
                return Err("Icon rendering cancelled".into());
            }
            for x in left..right {
                let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let weights = [
                    edge(p[1], p[2], px, py) / area,
                    edge(p[2], p[0], px, py) / area,
                    edge(p[0], p[1], px, py) / area,
                ];
                if weights.iter().any(|v| *v < -1e-9) {
                    continue;
                }
                let mut w = weights;
                let depth = if self.settings.projection == TextProjection::Perspective {
                    for i in 0..3 {
                        w[i] /= p[i][2];
                    }
                    let sum: f64 = w.iter().sum();
                    for value in &mut w {
                        *value /= sum;
                    }
                    1.0 / sum
                } else {
                    (0..3).map(|i| w[i] * p[i][2]).sum()
                };
                let index = (y * self.image.width() + x) as usize;
                if depth >= f64::from(self.depths[index]) {
                    continue;
                }
                let interpolate = |field: fn(Vertex) -> V3| {
                    (0..3).fold([0.0; 3], |v, i| add(v, mul(field(vertices[i]), w[i])))
                };
                let n = unit(interpolate(|v| v.n));
                let world = interpolate(|v| v.p);
                self.image
                    .put_pixel(x, y, self.shade(color, part, n, world));
                self.depths[index] = depth as f32;
            }
        }
        Ok(())
    }
    fn shade(&self, color: Rgba<u8>, part: &ArtifactPart, normal: V3, world: V3) -> Rgba<u8> {
        if self.settings.output == ArtifactOutput::AlphaMask {
            return Rgba([255, 255, 255, color[3]]);
        }
        if part.shading == TextShading::Unlit {
            return color;
        }
        let (a, e) = (
            self.settings.light_azimuth.to_radians(),
            self.settings.light_elevation.to_radians(),
        );
        let light = [a.sin() * e.cos(), e.sin(), a.cos() * e.cos()];
        let view = match self.settings.projection {
            TextProjection::Perspective => {
                unit(sub([0.0, 0.0, self.settings.camera_distance], world))
            }
            TextProjection::Orthographic => [0.0, 0.0, 1.0],
        };
        let diffuse = dot(normal, light).max(0.0) * self.settings.light_intensity;
        let specular = if part.shading == TextShading::Glossy {
            dot(normal, unit(add(light, view)))
                .max(0.0)
                .powf(4.0 + 124.0 * (1.0 - part.roughness))
                * self.settings.light_intensity
                * part.reflectivity
        } else {
            0.0
        };
        let mut result = color;
        for i in 0..3 {
            let linear = (f64::from(color[i]) / 255.0).powf(2.2);
            result[i] = ((linear * (self.settings.ambient + diffuse) + specular)
                .clamp(0.0, 1.0)
                .powf(1.0 / 2.2)
                * 255.0)
                .round() as u8;
        }
        result
    }
}
