use crate::renderer::{composite, rgba};
use achimeld_application::diagnostics::{Operation, effect_kind};
use image::{Rgba, RgbaImage};
use serde_json::Value;

fn traced<T>(
    effect: &Value,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
    render: impl FnOnce(&Operation) -> Result<T, String>,
) -> Result<T, String> {
    let operation = Operation::begin("render effect");
    operation.checkpoint(&format!(
        "kind={}; canvas={}x{}; width={}; blur={}; opacity={}; affect_outline={}; thread={:?}",
        effect_kind(effect["kind"].as_str().unwrap_or("")),
        width,
        height,
        num(effect, "width", 2.0),
        num(effect, "blur", 2.0),
        num(effect, "opacity", 1.0),
        effect["affectOutline"] == true,
        std::thread::current().id(),
    ));
    let result = render(&operation);
    operation.finish(if result.is_ok() {
        "completed"
    } else if cancelled() {
        "cancelled"
    } else {
        "failed"
    });
    result
}
fn num(value: &Value, key: &str, default: f64) -> f64 {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
pub(crate) fn apply(
    image: RgbaImage,
    layer: &Value,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
    text_outline: Option<&OutlineRenderer<'_>>,
) -> Result<RgbaImage, String> {
    let count = layer["effects"].as_array().map_or(0, Vec::len)
        + usize::from(layer["kind"] == "Text" && layer["text"]["outlineEnabled"] == true);
    let operation = (count > 0).then(|| {
        let operation = Operation::begin("render layer effects");
        operation.checkpoint(&format!(
            "canvas={width}x{height}; effect_count={count}; initialize alpha masks"
        ));
        operation
    });
    let result = apply_inner(
        image,
        layer,
        width,
        height,
        cancelled,
        text_outline,
        operation.as_ref(),
    );
    if let Some(operation) = operation {
        operation.finish(if result.is_ok() {
            "completed"
        } else if cancelled() {
            "cancelled"
        } else {
            "failed"
        });
    }
    result
}

fn apply_inner(
    mut image: RgbaImage,
    layer: &Value,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
    text_outline: Option<&OutlineRenderer<'_>>,
    operation: Option<&Operation>,
) -> Result<RgbaImage, String> {
    let mut original_alpha: Vec<_> = image.pixels().map(|p| p[3]).collect();
    let mut vector_outline_valid = true;
    let mut artwork = None;
    let mut outline_coverage: Option<Vec<u8>> = None;
    let text = &layer["text"];
    let legacy_outline = serde_json::json!({"kind":"Outline","colorHex":text["outlineColorHex"],
        "width":text["outlineWidth"],"placement":text["outlinePlacement"]});
    let legacy =
        (layer["kind"] == "Text" && text["outlineEnabled"] == true).then_some(&legacy_outline);
    for (index, effect) in legacy
        .into_iter()
        .chain(layer["effects"].as_array().into_iter().flatten())
        .enumerate()
    {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        if effect["visible"] == false {
            continue;
        }
        if let Some(operation) = operation {
            operation.checkpoint(&format!(
                "effect_index={index}; kind={}",
                effect_kind(effect["kind"].as_str().unwrap_or(""))
            ));
        }
        if crate::spatial_effects::supports(effect) {
            if crate::spatial_effects::is_identity(effect, height) {
                continue;
            }
            // Keep every coverage plane in the same coordinates for later outlines/inner light.
            image = traced(effect, width, height, cancelled, |_| {
                crate::spatial_effects::apply(image, effect, cancelled)
            })?;
            let warp_alpha = |alpha: &[u8]| -> Result<Vec<u8>, String> {
                let mask = RgbaImage::from_fn(width, height, |x, y| {
                    Rgba([255, 255, 255, alpha[(y * width + x) as usize]])
                });
                Ok(crate::spatial_effects::apply(mask, effect, cancelled)?
                    .pixels()
                    .map(|p| p[3])
                    .collect())
            };
            original_alpha = warp_alpha(&original_alpha)?;
            if let Some(previous) = artwork.take() {
                artwork = Some(crate::spatial_effects::apply(previous, effect, cancelled)?);
            }
            if let Some(coverage) = outline_coverage.take() {
                outline_coverage = Some(warp_alpha(&coverage)?);
            }
            vector_outline_valid = false;
            continue;
        }
        if effect["kind"] == "Outline" {
            let (outlined, coverage) = traced(effect, width, height, cancelled, |operation| {
                operation.checkpoint("retain artwork and inspect vector eligibility");
                artwork.get_or_insert_with(|| image.clone());
                let vector = if vector_outline_valid
                    && image
                        .pixels()
                        .map(|p| p[3])
                        .eq(original_alpha.iter().copied())
                {
                    text_outline
                        .map(|render| {
                            operation.checkpoint("vector outline rasterization");
                            render(effect)
                        })
                        .transpose()?
                } else {
                    None
                };
                outline(image, effect, height, vector, cancelled, operation)
            })?;
            image = outlined;
            if let Some(previous) = &mut outline_coverage {
                for (old, new) in previous.iter_mut().zip(coverage) {
                    *old = 255 - ((u16::from(255 - *old) * u16::from(255 - new)) / 255) as u8;
                }
            } else {
                outline_coverage = Some(coverage);
            }
            continue;
        }
        if let (Some(previous), Some(coverage)) = (&mut artwork, &outline_coverage) {
            let updated = single(previous.clone(), effect, &original_alpha, height, cancelled)?;
            if effect["affectOutline"] != true
                && matches!(effect["kind"].as_str(), Some("BoxBlur" | "InnerLight"))
            {
                replace_artwork(&mut image, previous, &updated, coverage);
                *previous = updated;
                continue;
            }
            *previous = updated;
            if effect["kind"] == "BoxBlur" {
                let alpha = RgbaImage::from_fn(width, height, |x, y| {
                    Rgba([0, 0, 0, coverage[(y * width + x) as usize]])
                });
                let blurred = box_blur(
                    &alpha,
                    (num(effect, "blur", 2.0) * f64::from(height) / 512.0)
                        .clamp(0.0, 128.0)
                        .round() as u32,
                );
                let opacity = num(effect, "opacity", 1.0).clamp(0.0, 1.0);
                outline_coverage = Some(
                    coverage
                        .iter()
                        .zip(blurred.pixels())
                        .map(|(a, b)| {
                            (f64::from(*a) * (1.0 - opacity) + f64::from(b[3]) * opacity).round()
                                as u8
                        })
                        .collect(),
                );
            }
        }
        image = single(image, effect, &original_alpha, height, cancelled)?;
    }
    Ok(image)
}

pub(crate) type OutlineRenderer<'a> = dyn Fn(&Value) -> Result<RgbaImage, String> + 'a;

fn single(
    image: RgbaImage,
    effect: &Value,
    original_alpha: &[u8],
    reference_height: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    traced(
        effect,
        image.width(),
        image.height(),
        cancelled,
        |operation| {
            single_inner(
                image,
                effect,
                original_alpha,
                reference_height,
                cancelled,
                operation,
            )
        },
    )
}

fn single_inner(
    mut image: RgbaImage,
    effect: &Value,
    original_alpha: &[u8],
    reference_height: u32,
    cancelled: &dyn Fn() -> bool,
    operation: &Operation,
) -> Result<RgbaImage, String> {
    operation.checkpoint("validate effect color and blend mode");
    let (width, height) = image.dimensions();
    let opacity = num(effect, "opacity", 1.0).clamp(0.0, 1.0);
    let color = rgba(effect["colorHex"].as_str().unwrap_or("#FF000000"))?;
    let mode = effect["blendMode"].as_str().unwrap_or("Normal");
    crate::renderer::check_blend_mode(mode)?;
    let scale = f64::from(reference_height) / 512.0;
    match effect["kind"].as_str().unwrap_or("") {
        "GradientOverlay" => {
            crate::renderer::gradient_map(&mut image, &effect["gradient"], opacity, mode)?;
        }
        "ColorOverlay" => {
            let alpha: Vec<_> = image.pixels().map(|p| p[3]).collect();
            // Blend color within the existing coverage. Using the original
            // alpha during blending gives translucent edges a different color.
            for pixel in image.pixels_mut() {
                pixel[3] = 255;
            }
            let overlay = RgbaImage::from_pixel(width, height, color);
            composite(&mut image, &overlay, opacity, mode);
            for (p, a) in image.pixels_mut().zip(alpha) {
                p[3] = a;
            }
        }
        "BoxBlur" => {
            let radius = (num(effect, "blur", 2.0) * scale).clamp(0.0, 128.0).round() as u32;
            let blurred = box_blur(&image, radius);
            for (p, b) in image.pixels_mut().zip(blurred.pixels()) {
                for i in 0..4 {
                    p[i] = (f64::from(p[i]) * (1.0 - opacity) + f64::from(b[i]) * opacity).round()
                        as u8;
                }
            }
        }
        "Shadow" => {
            let dx = (num(effect, "offsetX", 2.0) * scale).clamp(-4096.0, 4096.0) as i64;
            let dy = (num(effect, "offsetY", 2.0) * scale).clamp(-4096.0, 4096.0) as i64;
            let mut shadow = RgbaImage::new(width, height);
            for (x, y, p) in shadow.enumerate_pixels_mut() {
                let (sx, sy) = (i64::from(x) - dx, i64::from(y) - dy);
                if sx >= 0 && sy >= 0 && sx < i64::from(width) && sy < i64::from(height) {
                    *p = color;
                    p[3] = (u16::from(color[3])
                        * u16::from(image.get_pixel(sx as u32, sy as u32)[3])
                        / 255) as u8;
                }
            }
            let radius = (num(effect, "blur", 2.0) * scale).clamp(0.0, 128.0).round() as u32;
            let mut shadow = box_blur(&shadow, radius);
            for p in shadow.pixels_mut() {
                p[3] = (f64::from(p[3]) * opacity).round() as u8;
            }
            composite(&mut shadow, &image, 1.0, mode);
            image = shadow;
        }
        "OuterLight" => {
            let spread = (num(effect, "width", 2.0) * scale).clamp(0.0, 64.0).round() as u32;
            let radius = (num(effect, "blur", 2.0) * scale).clamp(0.0, 128.0).round() as u32;
            let padding = spread + radius;
            operation.checkpoint(&format!(
                "outer glow padded mask; spread={spread}; radius={radius}; padded={}x{}",
                width + 2 * padding,
                height + 2 * padding
            ));
            // Transparent padding makes the result independent of canvas-edge clamping.
            // Sliding maxima and separable blur keep work linear in the padded plane.
            let alpha = RgbaImage::from_fn(width + 2 * padding, height + 2 * padding, |x, y| {
                let a = if x >= padding
                    && y >= padding
                    && x < width + padding
                    && y < height + padding
                {
                    image.get_pixel(x - padding, y - padding)[3]
                } else {
                    0
                };
                Rgba([0, 0, 0, a])
            });
            operation.checkpoint("outer glow expand alpha");
            let expanded = max_alpha(&alpha, spread);
            if cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            let spread_mask = RgbaImage::from_fn(alpha.width(), alpha.height(), |x, y| {
                Rgba([0, 0, 0, expanded[(y * alpha.width() + x) as usize]])
            });
            operation.checkpoint("outer glow blur");
            let blurred = box_blur(&spread_mask, radius);
            if cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            operation.checkpoint("outer glow composite");
            let glow = RgbaImage::from_fn(width, height, |x, y| {
                let mut pixel = color;
                let coverage = blurred.get_pixel(x + padding, y + padding)[3]
                    .saturating_sub(image.get_pixel(x, y)[3]);
                pixel[3] = (u16::from(color[3]) * u16::from(coverage) / 255) as u8;
                pixel
            });
            composite(&mut image, &glow, opacity, mode);
        }
        "InnerLight" => {
            let cutout = (num(effect, "cutout", 0.0) * scale)
                .clamp(0.0, 64.0)
                .round() as u32;
            let (pw, ph) = (width + cutout * 2, height + cutout * 2);
            let alpha = RgbaImage::from_fn(pw, ph, |x, y| {
                let a = if x >= cutout && y >= cutout && x < width + cutout && y < height + cutout {
                    original_alpha[((y - cutout) * width + x - cutout) as usize]
                } else {
                    0
                };
                Rgba([0, 0, 0, a])
            });
            let expanded = max_alpha(&alpha, cutout);
            let dx = (num(effect, "offsetX", 2.0) * scale)
                .clamp(-4096.0, 4096.0)
                .round() as i64;
            let dy = (num(effect, "offsetY", 2.0) * scale)
                .clamp(-4096.0, 4096.0)
                .round() as i64;
            let inverse_at = |x: i64, y: i64| {
                let (sx, sy) = (x - dx + i64::from(cutout), y - dy + i64::from(cutout));
                if sx >= 0 && sy >= 0 && sx < i64::from(pw) && sy < i64::from(ph) {
                    255 - expanded[(sy as u32 * pw + sx as u32) as usize]
                } else {
                    255
                }
            };
            let clip: Vec<_> = image
                .pixels()
                .enumerate()
                .map(|(i, p)| {
                    if effect["affectOutline"] == true {
                        1.0
                    } else if p[3] == 0 {
                        0.0
                    } else {
                        (f64::from(original_alpha[i]) / f64::from(p[3])).min(1.0)
                    }
                })
                .collect();
            let region: Vec<_> = (0..height)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .enumerate()
                .map(|(i, (x, y))| {
                    (f64::from(inverse_at(x.into(), y.into())) * clip[i]).round() as u8
                })
                .collect();
            overlay(&mut image, &region, color, opacity, mode)?;
            let radius = (num(effect, "blur", 2.0) * scale).clamp(0.0, 128.0).round() as u32;
            if effect["blurEnabled"] != false && radius > 0 {
                let inverted =
                    RgbaImage::from_fn(width + radius * 2, height + radius * 2, |x, y| {
                        Rgba([
                            0,
                            0,
                            0,
                            inverse_at(
                                i64::from(x) - i64::from(radius),
                                i64::from(y) - i64::from(radius),
                            ),
                        ])
                    });
                if cancelled() {
                    return Err("Icon rendering cancelled".into());
                }
                let blurred = box_blur(&inverted, radius);
                let mask: Vec<_> = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .enumerate()
                    .map(|(i, (x, y))| {
                        (f64::from(blurred.get_pixel(x + radius, y + radius)[3]) * clip[i]).round()
                            as u8
                    })
                    .collect();
                overlay(
                    &mut image,
                    &mask,
                    rgba(effect["blurColorHex"].as_str().unwrap_or("#FFFFFFFF"))?,
                    num(effect, "blurOpacity", 1.0).clamp(0.0, 1.0),
                    effect["blurBlendMode"].as_str().unwrap_or("Screen"),
                )?;
            }
        }
        kind => return Err(format!("Unsupported layer effect: {kind}")),
    }
    Ok(image)
}

fn overlay(
    image: &mut RgbaImage,
    mask: &[u8],
    color: Rgba<u8>,
    opacity: f64,
    mode: &str,
) -> Result<(), String> {
    crate::renderer::check_blend_mode(mode)?;
    for (pixel, coverage) in image.pixels_mut().zip(mask) {
        let amount = f64::from(*coverage) / 255.0 * f64::from(color[3]) / 255.0 * opacity;
        for i in 0..3 {
            let base = f64::from(pixel[i]) / 255.0;
            pixel[i] = ((base * (1.0 - amount)
                + crate::renderer::blend(f64::from(color[i]) / 255.0, base, mode) * amount)
                * 255.0)
                .round() as u8;
        }
    }
    Ok(())
}

fn outline(
    image: RgbaImage,
    effect: &Value,
    height: u32,
    vector: Option<RgbaImage>,
    cancelled: &dyn Fn() -> bool,
    operation: &Operation,
) -> Result<(RgbaImage, Vec<u8>), String> {
    operation.checkpoint("validate outline color and blend mode");
    let opacity = num(effect, "opacity", 1.0).clamp(0.0, 1.0);
    let color = rgba(effect["colorHex"].as_str().unwrap_or("#FF000000"))?;
    crate::renderer::check_blend_mode(effect["blendMode"].as_str().unwrap_or("Normal"))?;
    if let Some(stroke) = vector {
        operation.checkpoint("vector outline composite");
        let coverage = stroke
            .pixels()
            .map(|p| (f64::from(p[3]) * opacity).round() as u8)
            .collect();
        let mut result = image;
        composite(&mut result, &stroke, opacity, "Normal");
        return Ok((result, coverage));
    }
    let radius = (num(effect, "width", 2.0) * f64::from(height) / 512.0)
        .clamp(0.0, 64.0)
        .round() as u32;
    let placement = effect["placement"].as_str().unwrap_or("Center");
    let outside = if placement == "Inward" {
        0
    } else if placement == "Center" {
        radius.div_ceil(2)
    } else {
        radius
    };
    let inside = if placement == "Outward" {
        0
    } else if placement == "Center" {
        radius / 2
    } else {
        radius
    };
    let round = effect["outlineShape"] == "Round" || effect["outlineShape"] == 1;
    operation.checkpoint(&format!(
        "outline expand mask; outside={outside}; inside={inside}; round={round}"
    ));
    let expanded = morph(&image, outside, false, round, cancelled)?;
    operation.checkpoint("outline erode mask");
    let eroded = morph(&image, inside, true, round, cancelled)?;
    operation.checkpoint("outline composite");
    let mut outer = RgbaImage::new(image.width(), image.height());
    let mut inner = Vec::with_capacity(expanded.len());
    let mut coverage = Vec::with_capacity(expanded.len());
    for (i, ((pixel, dst), expanded)) in image
        .pixels()
        .zip(outer.pixels_mut())
        .zip(expanded)
        .enumerate()
    {
        let out = expanded.saturating_sub(pixel[3]);
        let edge = pixel[3].saturating_sub(eroded[i]);
        *dst = color;
        dst[3] = (u16::from(out) * u16::from(color[3]) / 255) as u8;
        inner.push(edge);
        coverage.push(
            (f64::from(out.saturating_add(edge)) * f64::from(color[3]) / 255.0 * opacity).round()
                as u8,
        );
    }
    for pixel in outer.pixels_mut() {
        pixel[3] = (f64::from(pixel[3]) * opacity).round() as u8;
    }
    // Outside coverage belongs behind the artwork, including translucent edges.
    composite(&mut outer, &image, 1.0, "Normal");
    overlay(
        &mut outer,
        &inner,
        color,
        opacity,
        effect["blendMode"].as_str().unwrap_or("Normal"),
    )?;
    Ok((outer, coverage))
}

fn replace_artwork(
    result: &mut RgbaImage,
    previous: &RgbaImage,
    updated: &RgbaImage,
    coverage: &[u8],
) {
    for (((pixel, old), new), coverage) in result
        .pixels_mut()
        .zip(previous.pixels())
        .zip(updated.pixels())
        .zip(coverage)
    {
        let visible = 1.0 - f64::from(*coverage) / 255.0;
        let (a, oa, na) = (
            f64::from(pixel[3]) / 255.0,
            f64::from(old[3]) / 255.0,
            f64::from(new[3]) / 255.0,
        );
        let output = (a + (na - oa) * visible).clamp(0.0, 1.0);
        for i in 0..3 {
            pixel[i] = if output == 0.0 {
                0
            } else {
                ((f64::from(pixel[i]) * a
                    + (f64::from(new[i]) * na - f64::from(old[i]) * oa) * visible)
                    / output)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
        }
        pixel[3] = (output * 255.0).round() as u8;
    }
}

fn morph(
    image: &RgbaImage,
    radius: u32,
    erode: bool,
    round: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    if !round {
        if !erode {
            return Ok(max_alpha(image, radius));
        }
        // Transparent pixels beyond the canvas participate in erosion.
        let inverse = RgbaImage::from_fn(
            image.width() + 2 * radius,
            image.height() + 2 * radius,
            |x, y| {
                let a = if x >= radius
                    && y >= radius
                    && x < image.width() + radius
                    && y < image.height() + radius
                {
                    image.get_pixel(x - radius, y - radius)[3]
                } else {
                    0
                };
                Rgba([0, 0, 0, 255 - a])
            },
        );
        let expanded = max_alpha(&inverse, radius);
        return Ok((0..image.height())
            .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
            .map(|(x, y)| 255 - expanded[((y + radius) * inverse.width() + x + radius) as usize])
            .collect());
    }
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut result = vec![if erode { 255 } else { 0 }; w * h];
    let horizontal = RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let a = image.get_pixel(x, y)[3];
        Rgba([0, 0, 0, if erode { 255 - a } else { a }])
    });
    for dy in -(radius as i64)..=i64::from(radius) {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let span = (f64::from(radius * radius) - (dy * dy) as f64)
            .sqrt()
            .floor() as u32;
        let filtered = horizontal_max(&horizontal, span);
        for y in 0..h {
            let sy = y as i64 + dy;
            for x in 0..w {
                let sample = if sy < 0
                    || sy >= h as i64
                    || (erode && (x < span as usize || x + span as usize >= w))
                {
                    0
                } else {
                    let a = filtered[sy as usize * w + x];
                    if erode { 255 - a } else { a }
                };
                let p = &mut result[y * w + x];
                *p = if erode {
                    (*p).min(sample)
                } else {
                    (*p).max(sample)
                };
            }
        }
    }
    Ok(result)
}

fn horizontal_max(image: &RgbaImage, radius: u32) -> Vec<u8> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut out = vec![0; w * h];
    for y in 0..h {
        let mut queue = std::collections::VecDeque::<usize>::new();
        let mut next = 0;
        for x in 0..w {
            while next < w && next <= x + radius as usize {
                while queue.back().is_some_and(|p| {
                    image.get_pixel(*p as u32, y as u32)[3]
                        <= image.get_pixel(next as u32, y as u32)[3]
                }) {
                    queue.pop_back();
                }
                queue.push_back(next);
                next += 1;
            }
            while queue
                .front()
                .is_some_and(|p| *p < x.saturating_sub(radius as usize))
            {
                queue.pop_front();
            }
            out[y * w + x] = queue
                .front()
                .map_or(0, |p| image.get_pixel(*p as u32, y as u32)[3]);
        }
    }
    out
}
// Separable sliding maxima keep outlines linear in pixel count, independent of radius.
fn max_alpha(image: &RgbaImage, radius: u32) -> Vec<u8> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut first = vec![0; w * h];
    let mut out = first.clone();
    let radius = radius as usize;
    for horizontal in [true, false] {
        let source = if horizontal {
            image.pixels().map(|p| p[3]).collect::<Vec<_>>()
        } else {
            first.clone()
        };
        let target = if horizontal { &mut first } else { &mut out };
        let (lines, length) = if horizontal { (h, w) } else { (w, h) };
        for line in 0..lines {
            let mut queue = std::collections::VecDeque::<usize>::new();
            let index = |p| {
                if horizontal {
                    line * w + p
                } else {
                    p * w + line
                }
            };
            let mut next = 0;
            for position in 0..length {
                while next < length && next <= position.saturating_add(radius) {
                    while queue
                        .back()
                        .is_some_and(|p| source[index(*p)] <= source[index(next)])
                    {
                        queue.pop_back();
                    }
                    queue.push_back(next);
                    next += 1;
                }
                while queue
                    .front()
                    .is_some_and(|p| *p < position.saturating_sub(radius))
                {
                    queue.pop_front();
                }
                target[index(position)] = queue.front().map_or(0, |p| source[index(*p)]);
            }
        }
    }
    out
}
pub(crate) fn box_blur(image: &RgbaImage, radius: u32) -> RgbaImage {
    if radius == 0 {
        return image.clone();
    }
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut intermediate = vec![[0u32; 4]; w * h];
    for y in 0..h {
        let mut prefix = vec![[0u64; 4]; w + 1];
        for x in 0..w {
            let p = image.get_pixel(x as u32, y as u32);
            let a = u64::from(p[3]);
            prefix[x + 1] = std::array::from_fn(|i| {
                prefix[x][i] + if i == 3 { a * 255 } else { u64::from(p[i]) * a }
            });
        }
        for x in 0..w {
            let lo = x.saturating_sub(radius as usize);
            let hi = (x + radius as usize + 1).min(w);
            intermediate[y * w + x] = std::array::from_fn(|i| {
                ((prefix[hi][i] - prefix[lo][i]) / (hi - lo) as u64) as u32
            });
        }
    }
    let mut out = RgbaImage::new(w as u32, h as u32);
    for x in 0..w {
        let mut prefix = vec![[0u64; 4]; h + 1];
        for y in 0..h {
            prefix[y + 1] =
                std::array::from_fn(|i| prefix[y][i] + u64::from(intermediate[y * w + x][i]));
        }
        for y in 0..h {
            let lo = y.saturating_sub(radius as usize);
            let hi = (y + radius as usize + 1).min(h);
            let sum: [u64; 4] =
                std::array::from_fn(|i| (prefix[hi][i] - prefix[lo][i]) / (hi - lo) as u64);
            let a = sum[3];
            let p = if a == 0 {
                Rgba([0; 4])
            } else {
                Rgba([
                    (sum[0] * 255).checked_div(a).unwrap_or(0).min(255) as u8,
                    (sum[1] * 255).checked_div(a).unwrap_or(0).min(255) as u8,
                    (sum[2] * 255).checked_div(a).unwrap_or(0).min(255) as u8,
                    (a / 255).min(255) as u8,
                ])
            };
            out.put_pixel(x as u32, y as u32, p);
        }
    }
    out
}
