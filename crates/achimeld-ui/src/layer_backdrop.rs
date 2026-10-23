use achimeld_application::Preferences;
use egui::{Color32, Id, Pos2, Rect, Shape};
use serde_json::Value;

fn parse_color(value: &Value) -> Option<Color32> {
    let hex = value.as_str()?.strip_prefix('#')?;
    let value = u32::from_str_radix(hex, 16).ok()?;
    let alpha = match hex.len() {
        6 => 255,
        8 => (value >> 24) as u8,
        _ => return None,
    };
    Some(Color32::from_rgba_unmultiplied(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
        alpha,
    ))
}

// Match Achiever's authored-color policy: gradients use visible stops weighted
// by covered span and color strength, rather than a dark interpolated pixel.
pub(crate) fn source_color(layer: &Value) -> Option<Color32> {
    let kind = layer["kind"].as_str()?;
    if kind == "Particle"
        && let Some(points) = layer["particles"]["colorMap"]["points"].as_array()
    {
        return points
            .iter()
            .filter_map(|p| {
                let color = parse_color(&p["colorHex"])?;
                let [r, g, b, a] = color.to_srgba_unmultiplied();
                let high = r.max(g).max(b) as f64;
                let saturation = high - f64::from(r.min(g).min(b));
                let score = f64::from(a)
                    * (high + saturation)
                    * p["power"].as_f64().unwrap_or(1.0)
                    * p["rangeX"].as_f64().unwrap_or(0.35)
                    * p["rangeY"].as_f64().unwrap_or(0.35);
                (a > 0).then_some((score, Color32::from_rgb(r, g, b)))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, c)| c);
    }
    let vignette_gradient = kind == "SdfVignette" && layer["sdfVignette"]["useGradient"] == true;
    let gradient = if kind == "Particle" {
        &layer["particles"]["gradient"]
    } else if kind == "Explosion" {
        &layer["explosion"]["gradient"]
    } else if vignette_gradient {
        &layer["sdfVignette"]["gradient"]
    } else {
        &layer["gradient"]
    };
    let color = match if vignette_gradient || matches!(kind, "Particle" | "Explosion") {
        "Gradient"
    } else {
        kind
    } {
        "Text" | "Text3d" => parse_color(&layer["text"]["colorHex"]),
        "Solid" => parse_color(&layer["solidColorHex"]),
        "Shape" => parse_color(&layer["shape"]["colorHex"])
            .or_else(|| parse_color(&layer["solidColorHex"])),
        "SdfVignette" => parse_color(&layer["sdfVignette"]["colorHex"]),
        "Gradient" | "GradientOverlay" => {
            let mut stops: Vec<_> = gradient["stops"]
                .as_array()?
                .iter()
                .filter_map(|stop| {
                    Some((stop["offset"].as_f64()?, parse_color(&stop["colorHex"])?))
                })
                .collect();
            stops.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winner = None;
            let mut best = -1.0;
            for (index, &(offset, color)) in stops.iter().enumerate() {
                let [r, g, b, a] = color.to_srgba_unmultiplied();
                if a == 0 {
                    continue;
                }
                let left = if index == 0 {
                    0.0
                } else {
                    (stops[index - 1].0 + offset) * 0.5
                };
                let right = if index + 1 == stops.len() {
                    1.0
                } else {
                    (stops[index + 1].0 + offset) * 0.5
                };
                let high = r.max(g).max(b) as f64;
                let low = r.min(g).min(b) as f64;
                let saturation = if high == 0.0 {
                    0.0
                } else {
                    (high - low) / high
                };
                let score = (right - left).max(0.0) * a as f64 * high * (1.0 + saturation);
                if score > best {
                    best = score;
                    winner = Some(color);
                }
            }
            winner
        }
        _ => None,
    }?;
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    Some(Color32::from_rgb(r, g, b))
}

// Quantized alpha-weighted histogram ignores transparent RGB and preserves the
// true hue of the winning bin. Only recomputed when the preview changes.
pub(crate) fn dominant_color(rgba: &[u8]) -> Option<Color32> {
    let mut bins = vec![[0_u64; 4]; 4096];
    for pixel in rgba.as_chunks::<4>().0 {
        let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]];
        let key = ((r as usize >> 4) << 8) | ((g as usize >> 4) << 4) | (b as usize >> 4);
        let bin = &mut bins[key];
        bin[0] += a as u64;
        for (sum, channel) in bin[1..].iter_mut().zip([r, g, b]) {
            *sum += channel as u64 * a as u64;
        }
    }
    let mut winner = [0; 4];
    for bin in bins {
        if bin[0] > winner[0] {
            winner = bin;
        }
    }
    (winner[0] > 0).then(|| {
        Color32::from_rgb(
            (winner[1] / winner[0]) as u8,
            (winner[2] / winner[0]) as u8,
            (winner[3] / winner[0]) as u8,
        )
    })
}

pub(crate) fn paint(
    ui: &egui::Ui,
    id: Id,
    rect: Rect,
    art_bottom: f32,
    source: Color32,
    selected: bool,
    preferences: &Preferences,
) -> Shape {
    let light = ui
        .ctx()
        .animate_bool_with_time(id.with("selection"), selected, 0.4)
        * 0.2;
    let target = source.to_array();
    let tint = std::array::from_fn::<_, 3, _>(|i| {
        ui.ctx()
            .animate_value_with_time(id.with(("tint", i)), target[i] as f32 * 0.2, 0.4)
    });
    let brighten = |value: f32| (value * (1.0 + light)).round().clamp(0.0, 255.0) as u8;
    let tint = Color32::from_rgb(brighten(tint[0]), brighten(tint[1]), brighten(tint[2]));
    let art = crate::theme::color(preferences, "Control").to_array();
    let effects = crate::theme::color(preferences, "Panel").to_array();
    let art = Color32::from_rgb(
        brighten(art[0] as f32),
        brighten(art[1] as f32),
        brighten(art[2] as f32),
    );
    let effects = Color32::from_rgb(
        brighten(effects[0] as f32),
        brighten(effects[1] as f32),
        brighten(effects[2] as f32),
    );
    let mut mesh = egui::epaint::Mesh::default();
    let split = art_bottom.clamp(rect.top(), rect.bottom());
    for (top, bottom, base) in [(rect.top(), split, art), (split, rect.bottom(), effects)] {
        if bottom <= top {
            continue;
        }
        let start = mesh.vertices.len() as u32;
        for y in 0..=8 {
            for x in 0..=24 {
                let pos = Pos2::new(
                    egui::lerp(rect.x_range(), x as f32 / 24.0),
                    egui::lerp(top..=bottom, y as f32 / 8.0),
                );
                // Circular fade from the lower left, radius equal to row width.
                let fade = (1.0 - pos.distance(rect.left_bottom()) / rect.width().max(1.0))
                    .clamp(0.0, 1.0);
                mesh.colored_vertex(pos, crate::theme::mix(base, tint, fade));
            }
        }
        for y in 0..8 {
            for x in 0..24 {
                let a = start + y * 25 + x;
                mesh.add_triangle(a, a + 1, a + 25);
                mesh.add_triangle(a + 1, a + 26, a + 25);
            }
        }
    }
    Shape::mesh(mesh)
}
