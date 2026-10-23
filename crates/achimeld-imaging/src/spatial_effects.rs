//! Canvas-centered distortions, sampled in premultiplied alpha by the shared sampler.
use crate::renderer::bilinear;
use image::{Rgba, RgbaImage};
use serde_json::Value;

pub(crate) fn supports(effect: &Value) -> bool {
    matches!(
        effect["kind"].as_str(),
        Some("Skew" | "Perspective" | "Pinch" | "LensBlur")
    )
}

fn number(effect: &Value, key: &str, min: f64, max: f64) -> f64 {
    effect[key]
        .as_f64()
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
        .clamp(min, max)
}

pub(crate) fn is_identity(effect: &Value, height: u32) -> bool {
    if effect["opacity"].as_f64() == Some(0.0) {
        return true;
    }
    match effect["kind"].as_str() {
        Some("Skew") => {
            number(effect, "skewX", -60.0, 60.0) == 0.0
                && number(effect, "skewY", -60.0, 60.0) == 0.0
        }
        Some("Perspective") => {
            number(effect, "perspectiveX", -0.45, 0.45) == 0.0
                && number(effect, "perspectiveY", -0.45, 0.45) == 0.0
        }
        Some("Pinch") => number(effect, "amount", -1.0, 1.0) == 0.0,
        Some("LensBlur") => {
            (number(effect, "blur", 0.0, 128.0) * f64::from(height) / 512.0).round() == 0.0
                && number(effect, "aberration", 0.0, 64.0) == 0.0
        }
        _ => true,
    }
}

pub(crate) fn apply(
    image: RgbaImage,
    effect: &Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let strength = effect["opacity"]
        .as_f64()
        .filter(|v| v.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let kind = effect["kind"].as_str().unwrap_or("");
    let skew_x = (number(effect, "skewX", -60.0, 60.0) * strength)
        .to_radians()
        .tan();
    let skew_y = (number(effect, "skewY", -60.0, 60.0) * strength)
        .to_radians()
        .tan();
    let perspective_x = number(effect, "perspectiveX", -0.45, 0.45) * strength;
    let perspective_y = number(effect, "perspectiveY", -0.45, 0.45) * strength;
    let amount = number(effect, "amount", -1.0, 1.0) * strength;
    let aberration =
        number(effect, "aberration", 0.0, 64.0) * f64::from(image.height()) / 512.0 * strength;
    let blur = (number(effect, "blur", 0.0, 128.0) * f64::from(image.height()) / 512.0)
        .round()
        .clamp(0.0, 128.0) as u32;
    if strength == 0.0
        || match kind {
            "Skew" => skew_x == 0.0 && skew_y == 0.0,
            "Perspective" => perspective_x == 0.0 && perspective_y == 0.0,
            "Pinch" => amount == 0.0,
            "LensBlur" => blur == 0 && aberration == 0.0,
            _ => true,
        }
    {
        return Ok(image);
    }
    let (width, height) = image.dimensions();
    let (cx, cy) = (
        f64::from(width.saturating_sub(1)) / 2.0,
        f64::from(height.saturating_sub(1)) / 2.0,
    );
    let (rx, ry) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let radius = rx.min(ry).max(0.5);
    let blurred = (kind == "LensBlur" && blur > 0).then(|| crate::effects::box_blur(&image, blur));
    let mut output = RgbaImage::new(width, height);
    for y in 0..height {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for x in 0..width {
            let (dx, dy) = (f64::from(x) - cx, f64::from(y) - cy);
            let (sx, sy) = match kind {
                // X shear followed by Y shear has determinant one, even at equal angles.
                "Skew" => {
                    let sy = dy - skew_y * dx;
                    (dx - skew_x * sy + cx, sy + cy)
                }
                "Perspective" => {
                    let (u, v) = (dx / rx, dy / ry);
                    let denominator = 1.0 - perspective_x * u - perspective_y * v;
                    // Both coefficients are <= .45, keeping the horizon outside the canvas.
                    (u / denominator * rx + cx, v / denominator * ry + cy)
                }
                "Pinch" => {
                    let r = dx.hypot(dy) / radius;
                    // Smoothly meets the unchanged outer region with unit slope.
                    let factor = 1.0 - amount * (1.0 - r.min(1.0)).powi(2);
                    (dx * factor + cx, dy * factor + cy)
                }
                "LensBlur" => {
                    let r = dx.hypot(dy);
                    let edge = (r / radius).min(1.0).powi(2);
                    let shift = aberration * edge / r.max(0.5);
                    let sample = |scale: f64| {
                        let (sx, sy) = (dx * scale + cx, dy * scale + cy);
                        let sharp = bilinear(&image, sx, sy);
                        blurred.as_ref().map_or(sharp, |blurred| {
                            mix(sharp, bilinear(blurred, sx, sy), edge * strength)
                        })
                    };
                    let channels = [sample(1.0 - shift), sample(1.0), sample(1.0 + shift)];
                    let alpha = channels.iter().map(|p| p[3]).max().unwrap_or(0);
                    let mut pixel = Rgba([0, 0, 0, alpha]);
                    if alpha > 0 {
                        for i in 0..3 {
                            pixel[i] = (f64::from(channels[i][i]) * f64::from(channels[i][3])
                                / f64::from(alpha))
                            .round() as u8;
                        }
                    }
                    output.put_pixel(x, y, pixel);
                    continue;
                }
                _ => unreachable!(),
            };
            output.put_pixel(x, y, bilinear(&image, sx, sy));
        }
    }
    Ok(output)
}

fn mix(a: Rgba<u8>, b: Rgba<u8>, amount: f64) -> Rgba<u8> {
    let alpha = f64::from(a[3]) * (1.0 - amount) + f64::from(b[3]) * amount;
    let mut result = Rgba([0, 0, 0, alpha.round() as u8]);
    if alpha > 0.0 {
        for i in 0..3 {
            result[i] = ((f64::from(a[i]) * f64::from(a[3]) * (1.0 - amount)
                + f64::from(b[i]) * f64::from(b[3]) * amount)
                / alpha)
                .round() as u8;
        }
    }
    result
}
