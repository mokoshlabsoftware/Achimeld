//! RGB blend functions from https://www.w3.org/TR/compositing-1/#blending.
use image::Rgba;

fn lum(c: [f64; 3]) -> f64 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}
fn sat(c: [f64; 3]) -> f64 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}
fn set_lum(c: [f64; 3], l: f64) -> [f64; 3] {
    let delta = l - lum(c);
    let mut c = c.map(|v| v + delta);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    if n < 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}
fn set_sat(c: [f64; 3], s: f64) -> [f64; 3] {
    let min = c[0].min(c[1]).min(c[2]);
    let range = sat(c);
    if range == 0.0 {
        [0.0; 3]
    } else {
        c.map(|v| (v - min) * s / range)
    }
}
pub(crate) fn rgb(s: [f64; 3], d: [f64; 3], mode: &str) -> [f64; 3] {
    match mode {
        "Hue" => set_lum(set_sat(s, sat(d)), lum(d)),
        "Saturation" => set_lum(set_sat(d, sat(s)), lum(d)),
        "Color" => set_lum(s, lum(d)),
        "Luminosity" => set_lum(d, lum(s)),
        _ => std::array::from_fn(|i| crate::renderer::blend(s[i], d[i], mode)),
    }
}
pub(crate) fn pixels(s: Rgba<u8>, d: Rgba<u8>, mode: &str) -> [f64; 3] {
    rgb(
        std::array::from_fn(|i| f64::from(s[i]) / 255.0),
        std::array::from_fn(|i| f64::from(d[i]) / 255.0),
        mode,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expanded_modes_handle_transparency_and_boundary_values() {
        use crate::renderer::{blend, check_blend_mode, composite};
        for mode in [
            "Color",
            "Hue",
            "Saturation",
            "Luminosity",
            "Soft Light",
            "Hard Light",
            "Color Dodge",
            "Color Burn",
            "Exclusion",
        ] {
            check_blend_mode(mode).unwrap();
            for s in [[0.0; 3], [1.0; 3], [1.0, 0.0, 0.5]] {
                for d in [[0.0; 3], [1.0; 3], [0.1, 0.8, 0.3]] {
                    assert!(
                        rgb(s, d, mode)
                            .iter()
                            .all(|v| v.is_finite() && *v >= -1e-12 && *v <= 1.0 + 1e-12),
                        "{mode}"
                    );
                }
            }
            let source = image::RgbaImage::from_pixel(1, 1, Rgba([210, 60, 100, 128]));
            let mut target = image::RgbaImage::new(1, 1);
            composite(&mut target, &source, 1.0, mode);
            assert_eq!(target, source, "{mode}: transparent backdrop");
            let before = target.clone();
            composite(&mut target, &source, 0.0, mode);
            assert_eq!(target, before, "{mode}: zero opacity");
        }
        assert_eq!(blend(1.0, 0.0, "Color Dodge"), 0.0);
        assert_eq!(blend(0.0, 1.0, "Color Burn"), 1.0);
        assert!((blend(0.5, 0.3, "Soft Light") - 0.3).abs() < 1e-12);
        assert!((blend(0.8, 0.25, "Hard Light") - 0.7).abs() < 1e-12);
        assert!((blend(0.8, 0.25, "Exclusion") - 0.65).abs() < 1e-12);
    }
    #[test]
    fn color_keeps_backdrop_luminance_and_gray_removes_chroma() {
        let d = [0.2, 0.6, 0.8];
        for s in [[1.0, 0.0, 0.0], [0.0, 1.0, 1.0], [0.4; 3]] {
            let c = rgb(s, d, "Color");
            assert!((lum(c) - lum(d)).abs() < 1e-12);
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        let c = rgb([0.4; 3], d, "Color");
        assert!(sat(c) < 1e-12);
    }
    #[test]
    fn luminosity_is_color_with_sources_exchanged() {
        let s = [0.8, 0.3, 0.1];
        let d = [0.1, 0.5, 0.7];
        assert_eq!(rgb(s, d, "Luminosity"), rgb(d, s, "Color"));
        assert!((sat(rgb(s, d, "Saturation")) - sat(s)).abs() < 1e-12);
    }
}
