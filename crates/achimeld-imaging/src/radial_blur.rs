//! Bounded radial/angular sampling and premultiplied construction-strip blur.
use image::{Rgba, RgbaImage};

pub(crate) fn apply(
    image: &RgbaImage,
    radial: f64,
    angular: f64,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    let mut current = image.clone();
    let cx = f64::from(image.width().saturating_sub(1)) * 0.5;
    let cy = f64::from(image.height().saturating_sub(1)) * 0.5;
    for (is_radial, amount) in [(true, radial), (false, angular)] {
        if amount == 0.0 {
            continue;
        }
        let mut output = RgbaImage::new(image.width(), image.height());
        for y in 0..image.height() {
            if cancelled() {
                return Err("Icon rendering cancelled".into());
            }
            for x in 0..image.width() {
                let (dx, dy) = (f64::from(x) - cx, f64::from(y) - cy);
                let mut sum = [0.0; 4];
                for i in -8_i32..=8 {
                    let t = f64::from(i) / 8.0;
                    let (sx, sy) = if is_radial {
                        let factor = 1.0 + t * amount * 0.5;
                        (cx + dx * factor, cy + dy * factor)
                    } else {
                        let (sin, cos) = (t * amount * std::f64::consts::FRAC_PI_4).sin_cos();
                        (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
                    };
                    let pixel = crate::renderer::bilinear(&current, sx, sy);
                    let weight = f64::from(9 - i.abs());
                    sum[3] += f64::from(pixel[3]) * weight;
                    for channel in 0..3 {
                        sum[channel] += f64::from(pixel[channel]) * f64::from(pixel[3]) * weight;
                    }
                }
                let mut pixel = Rgba([0, 0, 0, (sum[3] / 81.0).round() as u8]);
                if pixel[3] > 0 {
                    for channel in 0..3 {
                        pixel[channel] = (sum[channel] / sum[3]).round() as u8;
                    }
                }
                output.put_pixel(x, y, pixel);
            }
        }
        current = output;
    }
    Ok(current)
}

pub(crate) fn mix(a: Rgba<u8>, b: Rgba<u8>, amount: f64, mode: &str) -> Rgba<u8> {
    if amount == 0.0 {
        return a;
    }
    if amount == 1.0 && mode == "Normal" {
        return b;
    }
    let alpha = f64::from(a[3]) * (1.0 - amount) + f64::from(b[3]) * amount;
    let mut result = Rgba([0, 0, 0, alpha.round() as u8]);
    if result[3] > 0 {
        for i in 0..3 {
            let changed =
                crate::renderer::blend(f64::from(b[i]) / 255.0, f64::from(a[i]) / 255.0, mode)
                    * 255.0;
            result[i] = ((f64::from(a[i]) * f64::from(a[3]) * (1.0 - amount)
                + changed * f64::from(b[3]) * amount)
                / alpha)
                .round() as u8;
        }
    }
    result
}

/// One sliding box pass; horizontal construction coordinates wrap at the seam.
pub(crate) fn axis(
    image: &RgbaImage,
    radius: u32,
    horizontal: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    if radius == 0 {
        return Ok(image.clone());
    }
    let (length, lines) = if horizontal {
        (image.width(), image.height())
    } else {
        (image.height(), image.width())
    };
    let mut output = RgbaImage::new(image.width(), image.height());
    if length == 0 {
        return Ok(output);
    }
    let radius = i64::from(radius.min(length));
    let window = (radius * 2 + 1) as u64;
    for line in 0..lines {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        let sample = |index: i64| -> [u64; 4] {
            if !horizontal && !(0..i64::from(length)).contains(&index) {
                return [0; 4];
            }
            let index = index.rem_euclid(i64::from(length)) as u32;
            let pixel = if horizontal {
                image.get_pixel(index, line)
            } else {
                image.get_pixel(line, index)
            };
            let alpha = u64::from(pixel[3]);
            [
                u64::from(pixel[0]) * alpha,
                u64::from(pixel[1]) * alpha,
                u64::from(pixel[2]) * alpha,
                alpha,
            ]
        };
        let mut sum = [0_u64; 4];
        for i in -radius..=radius {
            let pixel = sample(i);
            for c in 0..4 {
                sum[c] += pixel[c];
            }
        }
        for index in 0..length {
            let mut pixel = Rgba([0, 0, 0, ((sum[3] + window / 2) / window) as u8]);
            if pixel[3] > 0 {
                for c in 0..3 {
                    pixel[c] = ((sum[c] + sum[3] / 2) / sum[3]) as u8;
                }
            }
            if horizontal {
                output.put_pixel(index, line, pixel);
            } else {
                output.put_pixel(line, index, pixel);
            }
            let leaving = sample(i64::from(index) - radius);
            let entering = sample(i64::from(index) + radius + 1);
            for c in 0..4 {
                sum[c] = sum[c] - leaving[c] + entering[c];
            }
        }
    }
    Ok(output)
}

pub(crate) fn wrapped_sample(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    if image.width() == 0 || image.height() == 0 {
        return Rgba([0; 4]);
    }
    let (left, top) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let mut sum = [0.0; 4];
    for dy in 0..2 {
        for dx in 0..2 {
            let sy = top + dy;
            if !(0..i64::from(image.height())).contains(&sy) {
                continue;
            }
            let sx = (left + dx).rem_euclid(i64::from(image.width()));
            let pixel = image.get_pixel(sx as u32, sy as u32);
            let weight =
                (if dx == 0 { 1.0 - fx } else { fx }) * if dy == 0 { 1.0 - fy } else { fy };
            sum[3] += f64::from(pixel[3]) * weight;
            for c in 0..3 {
                sum[c] += f64::from(pixel[c]) * f64::from(pixel[3]) * weight;
            }
        }
    }
    let mut result = Rgba([0, 0, 0, sum[3].round() as u8]);
    if result[3] > 0 {
        for c in 0..3 {
            result[c] = (sum[c] / sum[3]).round() as u8;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radial_blur_axes_preserve_transparency_wrap_and_distinct_directions() {
        let mut image = RgbaImage::from_pixel(33, 33, Rgba([255, 0, 0, 0]));
        image.put_pixel(24, 16, Rgba([0, 255, 0, 255]));
        assert_eq!(apply(&image, 0.0, 0.0, &|| false).unwrap(), image);
        let radial = apply(&image, 1.0, 0.0, &|| false).unwrap();
        let angular = apply(&image, 0.0, 1.0, &|| false).unwrap();
        let spread = |image: &RgbaImage| {
            image
                .enumerate_pixels()
                .fold([0.0; 2], |mut spread, (x, y, p)| {
                    spread[0] += (f64::from(x) - 24.0).powi(2) * f64::from(p[3]);
                    spread[1] += (f64::from(y) - 16.0).powi(2) * f64::from(p[3]);
                    spread
                })
        };
        let radial_spread = spread(&radial);
        let angular_spread = spread(&angular);
        assert!(radial_spread[0] > 3.0 * radial_spread[1]);
        assert!(angular_spread[1] > 3.0 * angular_spread[0]);
        assert!(
            radial
                .enumerate_pixels()
                .any(|(x, _, p)| x != 24 && p[3] > 0)
        );
        assert!(
            angular
                .enumerate_pixels()
                .any(|(_, y, p)| y != 16 && p[3] > 0)
        );
        for image in [&radial, &angular] {
            assert!(
                image
                    .pixels()
                    .all(|p| p[3] == 0 || (p[0] == 0 && p[1] == 255 && p[2] == 0))
            );
            assert!(image.pixels().all(|p| p[3] != 0 || p.0 == [0; 4]));
        }
        let mut seam = RgbaImage::new(8, 3);
        seam.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
        let horizontal = axis(&seam, 1, true, &|| false).unwrap();
        let vertical = axis(&seam, 1, false, &|| false).unwrap();
        assert!(horizontal.get_pixel(7, 0)[3] > 0);
        assert_eq!(vertical.get_pixel(0, 2)[3], 0);
        assert_eq!(
            wrapped_sample(&seam, -0.25, 0.0),
            wrapped_sample(&seam, 7.75, 0.0)
        );
        assert!(apply(&image, 1.0, 1.0, &|| true).is_err());
        assert!(axis(&image, 1, true, &|| true).is_err());
    }
}
