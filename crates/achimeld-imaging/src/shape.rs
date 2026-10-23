use image::{Rgba, RgbaImage};
use serde_json::Value;
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    kurbo::{Affine, BezPath, Ellipse, Rect, Shape},
};

fn num(value: &Value, key: &str, default: f64) -> f64 {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

pub(crate) fn render(layer: &Value, width: u32, height: u32) -> Result<RgbaImage, String> {
    let shape = &layer["shape"];
    let color = crate::renderer::rgba(
        shape["colorHex"]
            .as_str()
            .or(layer["solidColorHex"].as_str())
            .unwrap_or("#FFFFFFFF"),
    )?;
    let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let (rx, ry) = (
        cx * num(shape, "scaleX", 0.5).clamp(0.001, 4.0),
        cy * num(shape, "scaleY", 0.5).clamp(0.001, 4.0),
    );
    // Older projects store shape kinds as numeric enum values.
    let kind = shape["kind"]
        .as_str()
        .unwrap_or(match shape["kind"].as_u64() {
            Some(1) => "Sphere",
            Some(2) => "Polygon",
            Some(3) => "Star",
            _ => "Rectangle",
        });
    let path = match kind {
        "Sphere" => Ellipse::new((cx, cy), (rx, ry), 0.0).to_path(0.05),
        "Polygon" | "Star" => {
            let star = kind == "Star";
            let count = shape["vertexCount"].as_u64().unwrap_or(5).clamp(3, 64) as usize
                * if star { 2 } else { 1 };
            let points: Vec<_> = (0..count)
                .map(|i| {
                    let radius = if star && i % 2 == 1 {
                        num(shape, "innerRadius", 0.5).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    let angle = i as f64 / count as f64 * std::f64::consts::TAU
                        - std::f64::consts::FRAC_PI_2;
                    vello_cpu::kurbo::Point::new(
                        cx + rx * radius * angle.cos(),
                        cy + ry * radius * angle.sin(),
                    )
                })
                .collect();
            let mut path = BezPath::new();
            for i in 0..count {
                let previous = points[(i + count - 1) % count];
                let point = points[i];
                let next = points[(i + 1) % count];
                let rounding = num(
                    shape,
                    if star && i % 2 == 1 {
                        "innerRounding"
                    } else {
                        "outerRounding"
                    },
                    0.0,
                )
                .clamp(0.0, 1.0)
                    / 2.0;
                let entry = point.lerp(previous, rounding);
                let exit = point.lerp(next, rounding);
                if i == 0 {
                    path.move_to(entry);
                } else {
                    path.line_to(entry);
                }
                path.quad_to(point, exit);
            }
            path.close_path();
            path
        }
        _ => Rect::new(cx - rx, cy - ry, cx + rx, cy + ry).to_path(0.05),
    };
    let mut context = RenderContext::new(width as u16, height as u16);
    context.set_paint(vello_cpu::color::palette::css::WHITE);
    // Rotation acts on the already sized shape in canvas coordinates.
    context.set_transform(
        Affine::translate((cx, cy))
            * Affine::rotate(num(shape, "rotation", 0.0).to_radians())
            * Affine::translate((-cx, -cy)),
    );
    context.fill_path(&path);
    context.flush();
    let mut pixels = Pixmap::new(width as u16, height as u16);
    context.render(&mut pixels, &mut Resources::new());
    Ok(RgbaImage::from_fn(width, height, |x, y| {
        Rgba([
            color[0],
            color[1],
            color[2],
            (u16::from(pixels.data()[(y * width + x) as usize].a) * u16::from(color[3]) / 255)
                as u8,
        ])
    }))
}
