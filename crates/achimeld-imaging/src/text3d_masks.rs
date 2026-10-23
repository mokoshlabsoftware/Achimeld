//! Resolve coverage at actual pixel footprints, including silhouettes, holes and
//! boundaries between faces, bevels and walls. Interior pixels need one ray;
//! boundary pixels share a bounded subpixel grid for every mask selection.
use super::*;

const NEAR_EDGE: u8 = 4;
const CLASS: u8 = 3;

fn selected(output: Text3dOutput, sample: u8) -> bool {
    match output {
        Text3dOutput::FullColor => false,
        Text3dOutput::FullMask => sample & CLASS != 0,
        Text3dOutput::FaceMask => sample & CLASS == 1,
        Text3dOutput::EmbossMask => sample & CLASS == 3,
        Text3dOutput::EmbossMaskMinusFace => matches!(sample & CLASS, 2 | 3),
    }
}

fn pixel_size(scene: &Scene<'_>, t: f64, ray_length: f64) -> f64 {
    match scene.settings.projection {
        TextProjection::Orthographic => scene.settings.orthographic_size / f64::from(scene.height),
        TextProjection::Perspective => {
            (t / ray_length) * 36.0 / (f64::from(scene.height) * scene.settings.focal_length)
        }
    }
}

fn trace(
    scene: &Scene<'_>,
    model: &Model<'_>,
    x: f64,
    y: f64,
    edge_hint: bool,
    work: &mut u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<u8, String> {
    *work += 1;
    if *work > 64_000_000 {
        return Err("3D text exceeds its mask coverage work budget".into());
    }
    if *work & 1023 == 0 && cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    let (origin, direction) = scene.ray_at(x, y);
    let origin = scene.inverse(sub(origin, scene.position));
    let length = dot(direction, direction).sqrt();
    let direction = scene.inverse(mul(direction, 1.0 / length));
    let Some((near, far)) = model.interval(origin, direction) else {
        return Ok(0);
    };
    let mut t = near.max(length);
    // The facing cap plane bounds the entire solid in local Z. A point inside
    // its glyph footprint is the first visible surface, without marching.
    if direction[2].abs() > 1e-10 {
        let half = scene.settings.extrusion / 2.0 + scene.settings.emboss;
        let cap = (-direction[2].signum() * half - origin[2]) / direction[2];
        if cap >= t - 1e-8 && cap <= far + 1e-8 {
            let p = add(origin, mul(direction, cap));
            let inside = model.field.sample(p[0], p[1]) + scene.settings.push;
            if inside >= 0.0 {
                let footprint = 1.5 * pixel_size(scene, cap, length) / direction[2].abs().max(0.05);
                return Ok(1 | if edge_hint && inside <= footprint {
                    NEAR_EDGE
                } else {
                    0
                });
            }
        }
    }
    let mut closest = f64::INFINITY;
    for _ in 0..192 {
        if t > far {
            break;
        }
        *work += 1;
        if *work > 64_000_000 {
            return Err("3D text exceeds its mask coverage work budget".into());
        }
        if *work & 1023 == 0 && cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let p = add(origin, mul(direction, t));
        let (d, surface) = model.distance(p);
        let pixel = pixel_size(scene, t, length);
        let epsilon = (pixel * 0.01).clamp(0.0001, 0.05 / model.field.mask.scale);
        if edge_hint && d > 0.0 {
            closest = closest.min(d / pixel.max(1e-9));
        }
        if d.abs() <= epsilon {
            let normal = model.normal(p, surface, false);
            let facing = -dot(normal, direction);
            if facing <= 1e-6 {
                return Ok(if facing.abs() < 0.15 { NEAR_EDGE } else { 0 });
            }
            let class = match surface {
                Surface::Face => 1,
                Surface::Side => 2,
                Surface::Emboss => 3,
            };
            if !edge_hint {
                return Ok(class);
            }
            // Flag thin bands even if no neighboring pixel center lands on
            // them. Foreshortened faces have a larger local pixel footprint.
            let footprint = 1.5 * pixel / facing.max(0.05);
            let edge = match surface {
                Surface::Face => model.field.sample(p[0], p[1]) + scene.settings.push <= footprint,
                Surface::Side => (p[2].abs() - scene.settings.extrusion / 2.0).abs() <= footprint,
                Surface::Emboss => {
                    let outside = -model.field.sample(p[0], p[1]) - scene.settings.push;
                    outside.min(scene.settings.emboss - outside) <= footprint
                }
            };
            return Ok(class | if edge || facing < 0.15 { NEAR_EDGE } else { 0 });
        }
        t += (d.abs() * 0.7).max(epsilon * 0.5);
    }
    Ok(if edge_hint && closest < 1.5 {
        NEAR_EDGE
    } else {
        0
    })
}

pub(super) fn render(
    scene: &Scene<'_>,
    model: &Model<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let width = scene.width;
    let height = scene.height;
    let grid = if width.max(height) <= 64 { 4 } else { 8 };
    let samples = grid * grid;
    let mut centers = vec![0u8; (u64::from(width) * u64::from(height)) as usize];
    let mut work = 0;
    for y in 0..height {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for x in 0..width {
            centers[(y * width + x) as usize] = trace(
                scene,
                model,
                f64::from(x) + 0.5,
                f64::from(y) + 0.5,
                true,
                &mut work,
                cancelled,
            )?;
        }
    }
    let mut pixels = RgbaImage::new(width, height);
    for y in 0..height {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for x in 0..width {
            let center = centers[(y * width + x) as usize];
            let mut edge = center & NEAR_EDGE != 0;
            if !edge {
                'neighbors: for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                    for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                        if centers[(ny * width + nx) as usize] & CLASS != center & CLASS {
                            edge = true;
                            break 'neighbors;
                        }
                    }
                }
            }
            let alpha = if edge {
                let mut covered = 0u32;
                for sy in 0..grid {
                    for sx in 0..grid {
                        let sample = trace(
                            scene,
                            model,
                            f64::from(x) + (f64::from(sx) + 0.5) / f64::from(grid),
                            f64::from(y) + (f64::from(sy) + 0.5) / f64::from(grid),
                            false,
                            &mut work,
                            cancelled,
                        )?;
                        covered += u32::from(selected(scene.settings.output, sample));
                    }
                }
                ((covered * 255 + samples / 2) / samples) as u8
            } else if selected(scene.settings.output, center) {
                255
            } else {
                0
            };
            pixels.put_pixel(x, y, Rgba([255, 255, 255, alpha]));
        }
    }
    Ok(pixels)
}
