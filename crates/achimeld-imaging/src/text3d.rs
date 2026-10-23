//! Extruded font outlines with depth-tested walls and projective front/back faces.
//! No filesystem, graphics context, or additional worker pool is needed here.
use achimeld_application::text3d::{Text3dOutput, Text3dSettings, TextProjection, TextShading};
#[path = "text3d_relief.rs"]
mod relief;
use image::{Rgba, RgbaImage};
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    kurbo::{Affine, BezPath, PathEl, Point, Shape as _, flatten},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Face,
    Side,
    Emboss,
}

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

struct Scene<'a> {
    settings: &'a Text3dSettings,
    basis: [V3; 3],
    position: V3,
    light: V3,
    width: u32,
    height: u32,
}
impl Scene<'_> {
    fn rotate(&self, v: V3) -> V3 {
        add(
            add(mul(self.basis[0], v[0]), mul(self.basis[1], v[1])),
            mul(self.basis[2], v[2]),
        )
    }
    fn inverse(&self, v: V3) -> V3 {
        self.basis.map(|axis| dot(axis, v))
    }
    fn world(&self, v: V3) -> V3 {
        add(self.rotate(v), self.position)
    }
    fn project(&self, v: V3) -> V3 {
        let depth = self.settings.camera_distance - v[2];
        let scale = f64::from(self.height)
            * match self.settings.projection {
                TextProjection::Perspective => self.settings.focal_length / (36.0 * depth),
                TextProjection::Orthographic => 1.0 / self.settings.orthographic_size,
            };
        [
            f64::from(self.width) / 2.0 + v[0] * scale,
            f64::from(self.height) / 2.0 - v[1] * scale,
            depth,
        ]
    }
    fn ray(&self, x: u32, y: u32) -> (V3, V3) {
        self.ray_at(f64::from(x) + 0.5, f64::from(y) + 0.5)
    }
    fn ray_at(&self, x: f64, y: f64) -> (V3, V3) {
        let x = x - f64::from(self.width) / 2.0;
        let y = f64::from(self.height) / 2.0 - y;
        match self.settings.projection {
            TextProjection::Perspective => (
                [0.0, 0.0, self.settings.camera_distance],
                [
                    x * 36.0 / (f64::from(self.height) * self.settings.focal_length),
                    y * 36.0 / (f64::from(self.height) * self.settings.focal_length),
                    -1.0,
                ],
            ),
            TextProjection::Orthographic => (
                [
                    x * self.settings.orthographic_size / f64::from(self.height),
                    y * self.settings.orthographic_size / f64::from(self.height),
                    self.settings.camera_distance,
                ],
                [0.0, 0.0, -1.0],
            ),
        }
    }
    fn paint(
        &self,
        color: Rgba<u8>,
        normal: V3,
        view: V3,
        surface: Surface,
        visibility: f64,
    ) -> Rgba<u8> {
        let selected = match self.settings.output {
            Text3dOutput::FullColor => return self.shade(color, normal, view, visibility),
            Text3dOutput::EmbossMaskMinusFace => surface != Surface::Face,
            Text3dOutput::FullMask => true,
            Text3dOutput::FaceMask => surface == Surface::Face,
            Text3dOutput::EmbossMask => surface == Surface::Emboss,
        };
        Rgba([255, 255, 255, if selected { 255 } else { 0 }])
    }
    fn shade(&self, color: Rgba<u8>, normal: V3, view: V3, visibility: f64) -> Rgba<u8> {
        if self.settings.shading == TextShading::Unlit {
            return color;
        }
        let diffuse = dot(normal, self.light).max(0.0) * self.settings.light_intensity * visibility;
        let specular = if self.settings.shading == TextShading::Glossy {
            dot(normal, unit(add(self.light, unit(view))))
                .max(0.0)
                .powf(4.0 + 124.0 * (1.0 - self.settings.roughness))
                * self.settings.light_intensity
                * self.settings.reflectivity
                * visibility
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

struct Mask {
    alpha: Vec<u8>,
    width: u32,
    height: u32,
    left: f64,
    top: f64,
    scale: f64,
}
impl Mask {
    fn new(path: &BezPath, output_height: u32) -> Self {
        Self::padded(path, output_height, 0.0, 2048)
    }
    fn padded(path: &BezPath, output_height: u32, padding: f64, limit: u32) -> Self {
        let bounds = path.bounding_box().inflate(padding, padding);
        let scale = (f64::from(output_height) / 512.0 * 2.0)
            .clamp(0.25, 8.0)
            .min(f64::from(limit - 4) / bounds.width().max(bounds.height()).max(1.0));
        let width = (bounds.width() * scale).ceil() as u32 + 4;
        let height = (bounds.height() * scale).ceil() as u32 + 4;
        let left = bounds.x0 - 2.0 / scale;
        let top = bounds.y1 + 2.0 / scale;
        let mut context = RenderContext::new(width as u16, height as u16);
        context.set_paint(vello_cpu::color::palette::css::WHITE);
        context.set_transform(Affine::new([
            scale,
            0.0,
            0.0,
            -scale,
            -left * scale,
            top * scale,
        ]));
        context.fill_path(path);
        context.flush();
        let mut pixels = Pixmap::new(width as u16, height as u16);
        context.render(&mut pixels, &mut Resources::new());
        Self {
            alpha: pixels.data().iter().map(|p| p.a).collect(),
            width,
            height,
            left,
            top,
            scale,
        }
    }
    fn sample(&self, x: f64, y: f64) -> f64 {
        let x = (x - self.left) * self.scale - 0.5;
        let y = (self.top - y) * self.scale - 0.5;
        let ix = x.floor() as i64;
        let iy = y.floor() as i64;
        let mut alpha = 0.0;
        for dy in 0..2 {
            for dx in 0..2 {
                let px = ix + dx;
                let py = iy + dy;
                if (0..i64::from(self.width)).contains(&px)
                    && (0..i64::from(self.height)).contains(&py)
                {
                    let wx = if dx == 0 {
                        1.0 - (x - ix as f64)
                    } else {
                        x - ix as f64
                    };
                    let wy = if dy == 0 {
                        1.0 - (y - iy as f64)
                    } else {
                        y - iy as f64
                    };
                    alpha += f64::from(self.alpha[(py as u32 * self.width + px as u32) as usize])
                        * wx
                        * wy
                        / 255.0;
                }
            }
        }
        alpha
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    font: &crate::Font,
    content: &str,
    size: f32,
    color: Rgba<u8>,
    settings: &Text3dSettings,
    dimensions: (u32, u32),
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    if !size.is_finite() || !(1.0..=512.0).contains(&size) || content.chars().count() > 256 {
        return Err("3D text exceeds its font size or character budget".into());
    }
    let path = font.text_path(content, size, cancelled)?;
    if path.elements().is_empty() || path.bounding_box().area() == 0.0 {
        return Ok(RgbaImage::new(dimensions.0, dimensions.1));
    }
    // Masks resolve adaptive subpixel coverage directly at the requested size.
    // Color previews keep their existing supersampling and image filtering.
    let samples =
        if settings.output == Text3dOutput::FullColor && dimensions.0.max(dimensions.1) <= 512 {
            2
        } else {
            1
        };
    let width = dimensions.0 * samples;
    let height = dimensions.1 * samples;
    let rotation = |mut v: V3| {
        for (a, b, angle) in [
            (1, 2, settings.rotation_x),
            (2, 0, settings.rotation_y),
            (0, 1, settings.rotation_z),
        ] {
            let (sin, cos) = angle.to_radians().sin_cos();
            (v[a], v[b]) = (cos * v[a] - sin * v[b], sin * v[a] + cos * v[b]);
        }
        v
    };
    let azimuth = settings.light_azimuth.to_radians();
    let elevation = settings.light_elevation.to_radians();
    let scene = Scene {
        settings,
        basis: [
            rotation([1.0, 0.0, 0.0]),
            rotation([0.0, 1.0, 0.0]),
            rotation([0.0, 0.0, 1.0]),
        ],
        position: [
            settings.position_x,
            settings.position_y,
            settings.position_z,
        ],
        light: [
            azimuth.sin() * elevation.cos(),
            elevation.sin(),
            azimuth.cos() * elevation.cos(),
        ],
        width,
        height,
    };
    let pixels = if settings.output != Text3dOutput::FullColor
        || settings.push != 0.0
        || settings.emboss > 0.0
        || (settings.output == Text3dOutput::FullColor
            && settings.shading != TextShading::Unlit
            && (settings.smooth_normals || settings.shadow_strength > 0.0))
    {
        relief::render(&scene, &path, color, cancelled)?
    } else {
        render_plain(&scene, &path, color, cancelled)?
    };
    if samples == 1 {
        return Ok(pixels);
    }
    // Filter premultiplied color so transparent edges do not acquire a dark halo.
    Ok(RgbaImage::from_fn(dimensions.0, dimensions.1, |x, y| {
        let mut sums = [0u32; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let pixel = pixels.get_pixel(x * 2 + dx, y * 2 + dy);
                for i in 0..3 {
                    sums[i] += u32::from(pixel[i]) * u32::from(pixel[3]);
                }
                sums[3] += u32::from(pixel[3]);
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

fn render_plain(
    scene: &Scene<'_>,
    path: &BezPath,
    color: Rgba<u8>,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let (width, height, settings) = (scene.width, scene.height, scene.settings);
    let mask = Mask::new(path, height);
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    let mut contours: Vec<Vec<Point>> = Vec::new();
    let mut vertices = 0;
    flatten(
        path.iter(),
        (128.0 / f64::from(height)).clamp(0.03, 1.0),
        |element| {
            vertices += 1;
            if vertices > 16_384 {
                return;
            }
            match element {
                PathEl::MoveTo(p) => contours.push(vec![p]),
                PathEl::LineTo(p) => {
                    if let Some(contour) = contours.last_mut() {
                        contour.push(p);
                    }
                }
                _ => {}
            }
        },
    );
    if vertices > 16_384 {
        return Err("3D text geometry exceeds its vertex budget".into());
    }
    // TrueType and CFF use opposite contour orientations. Hole contours naturally
    // run in the opposite direction to their outer contour.
    let area = |points: &Vec<Point>| -> f64 {
        points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum()
    };
    let orientation = contours
        .iter()
        .map(area)
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .unwrap_or(1.0)
        .signum();
    let side = crate::renderer::rgba(&settings.side_color_hex)?;
    let mut pixels = RgbaImage::new(width, height);
    let mut depths = vec![f32::INFINITY; (width * height) as usize];
    let half = settings.extrusion / 2.0;
    let mut work = 0u64;
    if half > 0.0 {
        for contour in &contours {
            for (a, b) in contour
                .iter()
                .zip(contour.iter().cycle().skip(1))
                .take(contour.len())
            {
                if cancelled() {
                    return Err("Icon rendering cancelled".into());
                }
                let normal = unit([(b.y - a.y) * orientation, (a.x - b.x) * orientation, 0.0]);
                if dot(normal, normal) < 0.5 {
                    continue;
                }
                let world_normal = scene.rotate(normal);
                let center = scene.world([(a.x + b.x) / 2.0, (a.y + b.y) / 2.0, 0.0]);
                let view = if settings.projection == TextProjection::Perspective {
                    sub([0.0, 0.0, settings.camera_distance], center)
                } else {
                    [0.0, 0.0, 1.0]
                };
                if dot(world_normal, view) <= 0.0 {
                    continue;
                }
                let quad = [
                    scene.world([a.x, a.y, -half]),
                    scene.world([b.x, b.y, -half]),
                    scene.world([b.x, b.y, half]),
                    scene.world([a.x, a.y, half]),
                ];
                let clipped = clip_near(&quad, settings.camera_distance - 1.0);
                let shade = scene.paint(side, world_normal, view, Surface::Side, 1.0);
                for i in 1..clipped.len().saturating_sub(1) {
                    triangle(
                        scene,
                        [clipped[0], clipped[i], clipped[i + 1]],
                        shade,
                        &mut pixels,
                        &mut depths,
                        &mut work,
                        cancelled,
                    )?;
                }
            }
        }
    }
    for sign in [1.0, -1.0] {
        let normal = scene.rotate([0.0, 0.0, sign]);
        let view = if settings.projection == TextProjection::Perspective {
            sub(
                [0.0, 0.0, settings.camera_distance],
                scene.world([0.0, 0.0, half * sign]),
            )
        } else {
            [0.0, 0.0, 1.0]
        };
        if dot(normal, view) <= 0.0 {
            continue;
        }
        // Restrict ray casting to the clipped face bounds, including the coverage
        // filter's support. Empty canvas and back-facing planes need no samples.
        let bounds = path.bounding_box();
        let margin = 2.0 / mask.scale;
        let quad = [
            (bounds.x0 - margin, bounds.y0 - margin),
            (bounds.x1 + margin, bounds.y0 - margin),
            (bounds.x1 + margin, bounds.y1 + margin),
            (bounds.x0 - margin, bounds.y1 + margin),
        ]
        .map(|(x, y)| scene.world([x, y, half * sign]));
        let clipped = clip_near(&quad, settings.camera_distance - 1.0);
        if clipped.len() < 3 {
            continue;
        }
        let projected: Vec<_> = clipped.into_iter().map(|v| scene.project(v)).collect();
        let bound = |axis: usize, minimum: bool| -> u32 {
            let value = projected
                .iter()
                .map(|v| v[axis])
                .reduce(|a, b| if minimum { a.min(b) } else { a.max(b) })
                .unwrap_or(0.0);
            let value = if minimum {
                value.floor() - 1.0
            } else {
                value.ceil() + 1.0
            };
            value.clamp(0.0, f64::from(if axis == 0 { width } else { height })) as u32
        };
        let (x0, x1, y0, y1) = (
            bound(0, true),
            bound(0, false),
            bound(1, true),
            bound(1, false),
        );
        let uniform_color = (settings.shading != TextShading::Glossy
            || settings.projection == TextProjection::Orthographic)
            .then(|| scene.paint(color, normal, [0.0, 0.0, 1.0], Surface::Face, 1.0));
        for y in y0..y1 {
            if y % 16 == 0 && cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            for x in x0..x1 {
                let (origin, direction) = scene.ray(x, y);
                if dot(normal, direction) >= -1e-10 {
                    continue;
                }
                let local_origin = scene.inverse(sub(origin, scene.position));
                let local_direction = scene.inverse(direction);
                let distance = (half * sign - local_origin[2]) / local_direction[2];
                let index = (y * width + x) as usize;
                if distance < 1.0 || distance >= f64::from(depths[index]) {
                    continue;
                }
                let hit = add(local_origin, mul(local_direction, distance));
                let coverage = mask.sample(hit[0], hit[1]);
                if coverage <= 0.0 {
                    continue;
                }
                let mut shade = uniform_color.unwrap_or_else(|| {
                    scene.paint(color, normal, mul(direction, -1.0), Surface::Face, 1.0)
                });
                shade[3] = (f64::from(shade[3]) * coverage).round() as u8;
                if shade[3] == 0 && settings.output == Text3dOutput::FullColor {
                    continue;
                }
                pixels.put_pixel(x, y, shade);
                depths[index] = distance as f32;
            }
        }
    }
    Ok(pixels)
}

fn clip_near(vertices: &[V3], maximum_z: f64) -> Vec<V3> {
    let mut result = Vec::with_capacity(6);
    for (a, b) in vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
    {
        let a_inside = a[2] <= maximum_z;
        let b_inside = b[2] <= maximum_z;
        if a_inside {
            result.push(*a);
        }
        if a_inside != b_inside {
            result.push(add(
                *a,
                mul(sub(*b, *a), (maximum_z - a[2]) / (b[2] - a[2])),
            ));
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn triangle(
    scene: &Scene<'_>,
    vertices: [V3; 3],
    color: Rgba<u8>,
    pixels: &mut RgbaImage,
    depths: &mut [f32],
    work: &mut u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    if color[3] == 0 && scene.settings.output == Text3dOutput::FullColor {
        return Ok(());
    }
    let [a, b, c] = vertices.map(|v| scene.project(v));
    let edge =
        |a: V3, b: V3, x: f64, y: f64| (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
    let area = edge(a, b, c[0], c[1]);
    if area.abs() < 1e-10 {
        return Ok(());
    }
    let x0 = a[0]
        .min(b[0])
        .min(c[0])
        .floor()
        .clamp(0.0, f64::from(scene.width)) as u32;
    let x1 = a[0]
        .max(b[0])
        .max(c[0])
        .ceil()
        .clamp(0.0, f64::from(scene.width)) as u32;
    let y0 = a[1]
        .min(b[1])
        .min(c[1])
        .floor()
        .clamp(0.0, f64::from(scene.height)) as u32;
    let y1 = a[1]
        .max(b[1])
        .max(c[1])
        .ceil()
        .clamp(0.0, f64::from(scene.height)) as u32;
    *work += u64::from(x1 - x0) * u64::from(y1 - y0);
    if *work > 128_000_000 {
        return Err("3D text exceeds its surface work budget".into());
    }
    for y in y0..y1 {
        if y % 16 == 0 && cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for x in x0..x1 {
            let px = f64::from(x) + 0.5;
            let py = f64::from(y) + 0.5;
            let wa = edge(b, c, px, py) / area;
            let wb = edge(c, a, px, py) / area;
            let wc = 1.0 - wa - wb;
            if wa < -1e-9 || wb < -1e-9 || wc < -1e-9 {
                continue;
            }
            let depth = match scene.settings.projection {
                TextProjection::Perspective => 1.0 / (wa / a[2] + wb / b[2] + wc / c[2]),
                TextProjection::Orthographic => wa * a[2] + wb * b[2] + wc * c[2],
            };
            let index = (y * scene.width + x) as usize;
            if depth >= 1.0 && depth < f64::from(depths[index]) {
                depths[index] = depth as f32;
                pixels.put_pixel(x, y, color);
            }
        }
    }
    Ok(())
}
