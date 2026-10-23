//! SDF particles rasterize only their clipped bounds, with cancellation and a work cap.
use achimeld_application::particles::{
    Particle, ParticleSettings, ParticleShape, layout_for_instance,
};
use image::{Rgba, RgbaImage};
use serde_json::Value;

pub(crate) fn render(
    value: &Value,
    instance: &str,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let settings = ParticleSettings::from_value(value)?;
    let colors = crate::renderer::gradient_lookup(&settings.gradient)?;
    let field = settings
        .color_map
        .as_ref()
        .map(|map| map.compile())
        .transpose()?;
    let components = settings.components();
    let compiled: Vec<_> = components
        .iter()
        .map(|shape| {
            (
                shape,
                vertices(shape, shape.inner_radius),
                shape.rotation.to_radians().sin_cos(),
            )
        })
        .collect();
    let shape_extent = components
        .iter()
        .enumerate()
        .filter(|(i, s)| *i == 0 || s.operation == "Union")
        .map(|(_, s)| s.offset_x.hypot(s.offset_y) + s.scale)
        .fold(0.05_f64, f64::max);
    estimate(value, instance, width, height)?;
    let mut image = RgbaImage::new(width, height);
    let scale = f64::from(width.min(height)) / 512.0;
    let angle = settings.gradient["angle"]
        .as_f64()
        .unwrap_or(90.0)
        .to_radians();
    let (gs, gc) = angle.sin_cos();
    for particle in layout_for_instance(&settings, f64::from(width) / f64::from(height), instance) {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let varied: Vec<_> = components
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                if shape.kind == "Star"
                    && shape
                        .inner_radius_min
                        .is_some_and(|low| low != shape.inner_radius)
                {
                    Some(vertices(
                        shape,
                        particle.component_inner_radius(shape, index),
                    ))
                } else {
                    None
                }
            })
            .collect();
        let slice = field.as_ref().map(|field| {
            field
                .slice(particle.slice, 256)
                .into_iter()
                .map(Rgba)
                .collect::<Vec<_>>()
        });
        let colors = slice.as_deref().unwrap_or(&colors);
        let (radius, feather, extent_x, extent_y) = footprint(&settings, &particle, scale);
        let (cx, cy) = (
            particle.x * f64::from(width),
            particle.y * f64::from(height),
        );
        let x0 = (cx - extent_x).floor().clamp(0.0, f64::from(width)) as u32;
        let x1 = (cx + extent_x).ceil().clamp(0.0, f64::from(width)) as u32;
        let y0 = (cy - extent_y).floor().clamp(0.0, f64::from(height)) as u32;
        let y1 = (cy + extent_y).ceil().clamp(0.0, f64::from(height)) as u32;
        let (s, c) = particle.rotation.to_radians().sin_cos();
        for y in y0..y1 {
            if cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            for x in x0..x1 {
                let (dx, dy) = (f64::from(x) + 0.5 - cx, f64::from(y) + 0.5 - cy);
                let (px, py) = (
                    (dx * c + dy * s) / (radius * settings.shape_x),
                    (-dx * s + dy * c) / (radius * settings.shape_y),
                );
                let mut distance = 0.0_f64;
                for (index, (shape, vertices, (ss, sc))) in compiled.iter().enumerate() {
                    let (dx, dy) = (
                        px * shape_extent - shape.offset_x,
                        py * shape_extent - shape.offset_y,
                    );
                    let (sx, sy) = (
                        (dx * sc + dy * ss) / shape.scale,
                        (-dx * ss + dy * sc) / shape.scale,
                    );
                    let vertices = varied[index].as_ref().unwrap_or(vertices);
                    let part = if vertices.is_empty() {
                        sx.hypot(sy) - 1.0
                    } else {
                        polygon_distance(sx, sy, vertices)
                    } * shape.scale
                        / shape_extent;
                    distance = if index == 0 {
                        part
                    } else {
                        match shape.operation.as_str() {
                            "Subtract" => distance.max(-part),
                            "Intersect" => distance.max(part),
                            _ => distance.min(part),
                        }
                    };
                }
                let distance = distance * radius * settings.shape_x.min(settings.shape_y);
                let t = (0.5 - distance / (2.0 * feather)).clamp(0.0, 1.0);
                let coverage = t * t * (3.0 - 2.0 * t);
                if coverage == 0.0 {
                    continue;
                }
                let local_offset = || {
                    if settings.gradient["kind"] == "Linear" {
                        (px * gc + py * gs) / 2.0 + 0.5
                    } else {
                        px.hypot(py)
                    }
                };
                let offset = if field.is_some() {
                    local_offset()
                } else {
                    match settings.color_mode.as_str() {
                        "Random" => particle.color,
                        "Spawn radius" => particle.radial,
                        _ => local_offset(),
                    }
                };
                let color = colors[(offset.clamp(0.0, 1.0) * 255.0).round() as usize];
                let alpha = f64::from(color[3]) / 255.0 * particle.opacity * coverage;
                let destination = image.get_pixel_mut(x, y);
                let da = f64::from(destination[3]) / 255.0;
                let out = alpha + da * (1.0 - alpha);
                if out > 0.0 {
                    for i in 0..3 {
                        destination[i] = ((f64::from(color[i]) * alpha
                            + f64::from(destination[i]) * da * (1.0 - alpha))
                            / out)
                            .round() as u8;
                    }
                    destination[3] = (out * 255.0).round() as u8;
                } else {
                    *destination = Rgba([0; 4]);
                }
            }
        }
    }
    Ok(image)
}

pub(crate) fn estimate(
    value: &Value,
    instance: &str,
    width: u32,
    height: u32,
) -> Result<u64, String> {
    let settings = ParticleSettings::from_value(value)?;
    let complexity: u64 = settings
        .components()
        .iter()
        .map(|shape| match shape.kind.as_str() {
            "Sphere" => 1,
            "Star" => u64::from(shape.vertex_count) * 2,
            _ => u64::from(shape.vertex_count),
        })
        .sum();
    let scale = f64::from(width.min(height)) / 512.0;
    let slice_work = settings.color_map.as_ref().map_or(0, |m| {
        m.points.len() as u64 * u64::from(settings.count) * 256
    });
    let work = slice_work
        + layout_for_instance(&settings, f64::from(width) / f64::from(height), instance)
            .iter()
            .map(|p| {
                let (_, _, extent_x, extent_y) = footprint(&settings, p, scale);
                let bounds = |center: f64, extent: f64, limit: u32| {
                    (center + extent).ceil().clamp(0.0, f64::from(limit)) as u64
                        - (center - extent).floor().clamp(0.0, f64::from(limit)) as u64
                };
                bounds(p.x * f64::from(width), extent_x, width)
                    * bounds(p.y * f64::from(height), extent_y, height)
                    * complexity
            })
            .sum::<u64>();
    if work > 128_000_000 {
        return Err(
            "Particles exceed the render work budget; reduce count, size or output resolution"
                .into(),
        );
    }
    Ok(work)
}

// Rendering and work admission use the same conservative rotated shape bounds.
fn footprint(settings: &ParticleSettings, particle: &Particle, scale: f64) -> (f64, f64, f64, f64) {
    let radius = particle.diameter * scale / 2.0;
    let feather = (radius * particle.softness).max(0.5);
    if settings.shape_x == 1.0 && settings.shape_y == 1.0 {
        return (radius, feather, radius + feather, radius + feather);
    }
    let (s, c) = particle.rotation.to_radians().sin_cos();
    let outer = radius + feather / settings.shape_x.min(settings.shape_y);
    (
        radius,
        feather,
        outer * (settings.shape_x * c).hypot(settings.shape_y * s),
        outer * (settings.shape_x * s).hypot(settings.shape_y * c),
    )
}

fn vertices(settings: &ParticleShape, inner_radius: f64) -> Vec<(f64, f64)> {
    if settings.kind == "Sphere" {
        return Vec::new();
    }
    let star = settings.kind == "Star";
    let count = settings.vertex_count * if star { 2 } else { 1 };
    (0..count)
        .map(|i| {
            let angle = f64::from(i) / f64::from(count) * std::f64::consts::TAU
                - std::f64::consts::FRAC_PI_2;
            let radius = if star && i % 2 == 1 {
                inner_radius
            } else {
                1.0
            };
            (angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

// Closest segment distance with even/odd winding also handles concave stars.
fn polygon_distance(x: f64, y: f64, vertices: &[(f64, f64)]) -> f64 {
    let mut distance = f64::INFINITY;
    let mut inside = false;
    for (i, &(ax, ay)) in vertices.iter().enumerate() {
        let (bx, by) = vertices[(i + 1) % vertices.len()];
        let (ex, ey) = (bx - ax, by - ay);
        let t = (((x - ax) * ex + (y - ay) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        distance = distance.min((x - ax - ex * t).hypot(y - ay - ey * t));
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    if inside { -distance } else { distance }
}
