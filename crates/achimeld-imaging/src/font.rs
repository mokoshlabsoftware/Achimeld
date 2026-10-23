use image::{Rgba, RgbaImage};
use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
};
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    kurbo::{Affine, BezPath, Cap, Join, Stroke},
};

/// Owned font bytes, parsed lazily through the maintained Fontations implementation.
/// The host supplies the bytes; this type has no filesystem or font-discovery authority.
pub struct Font {
    bytes: Vec<u8>,
}
impl Font {
    /// Centered, unhinted outlines in the icon's design units, with Y pointing up.
    pub(crate) fn text_path(
        &self,
        content: &str,
        font_size: f32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BezPath, String> {
        use vello_cpu::kurbo::Shape as _;
        let font = FontRef::new(&self.bytes).map_err(|e| e.to_string())?;
        let size = Size::new(font_size);
        let location = LocationRef::default();
        let metrics = font.metrics(size, location);
        let glyph_metrics = font.glyph_metrics(size, location);
        let outlines = font.outline_glyphs();
        let charmap = font.charmap();
        let mut path = BezPath::new();
        let mut segments = 0;
        for (line, content) in content.split('\n').enumerate() {
            let glyphs: Vec<_> = content
                .trim_end_matches('\r')
                .chars()
                .map(|c| charmap.map(c).unwrap_or_default())
                .collect();
            let width: f32 = glyphs
                .iter()
                .map(|g| glyph_metrics.advance_width(*g).unwrap_or(0.0))
                .sum();
            let mut x = -width / 2.0;
            let y = -(line as f32) * (metrics.ascent - metrics.descent + metrics.leading);
            for id in glyphs {
                if cancelled() {
                    return Err("Icon rendering cancelled".into());
                }
                if let Some(glyph) = outlines.get(id) {
                    let mut pen = Pen {
                        path: BezPath::new(),
                        segments: &mut segments,
                    };
                    glyph
                        .draw(DrawSettings::unhinted(size, location), &mut pen)
                        .map_err(|e| e.to_string())?;
                    if *pen.segments > 65_536 {
                        return Err("3D text outlines exceed their work budget".into());
                    }
                    pen.path
                        .apply_affine(Affine::translate((f64::from(x), f64::from(y))));
                    path.extend(pen.path.elements().iter().copied());
                }
                x += glyph_metrics.advance_width(id).unwrap_or(0.0);
            }
        }
        if !path.elements().is_empty() {
            let center = path.bounding_box().center();
            path.apply_affine(Affine::translate((-center.x, -center.y)));
        }
        Ok(path)
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Font exceeds its byte limit".into());
        }
        FontRef::new(&bytes).map_err(|e| e.to_string())?;
        Ok(Self { bytes })
    }
    pub(crate) fn text(
        &self,
        content: &str,
        size: f32,
        color: Rgba<u8>,
        alignment: &str,
        dimensions: (u32, u32),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RgbaImage, String> {
        self.draw(content, size, color, alignment, dimensions, cancelled, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn text_outline(
        &self,
        content: &str,
        size: f32,
        color: Rgba<u8>,
        alignment: &str,
        dimensions: (u32, u32),
        cancelled: &dyn Fn() -> bool,
        width: f64,
        placement: &str,
    ) -> Result<RgbaImage, String> {
        let pen_width = if placement == "Center" {
            width
        } else {
            width * 2.0
        };
        let mut outline = self.draw(
            content,
            size,
            color,
            alignment,
            dimensions,
            cancelled,
            Some(pen_width),
        )?;
        if placement != "Center" {
            let fill = self.text(
                content,
                size,
                Rgba([255; 4]),
                alignment,
                dimensions,
                cancelled,
            )?;
            for (pixel, fill) in outline.pixels_mut().zip(fill.pixels()) {
                let clip = if placement == "Inward" {
                    fill[3]
                } else {
                    255 - fill[3]
                };
                pixel[3] = (u16::from(pixel[3]) * u16::from(clip) / 255) as u8;
            }
        }
        Ok(outline)
    }
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        content: &str,
        size: f32,
        color: Rgba<u8>,
        alignment: &str,
        dimensions: (u32, u32),
        cancelled: &dyn Fn() -> bool,
        stroke: Option<f64>,
    ) -> Result<RgbaImage, String> {
        let (width, height) = dimensions;
        let font = FontRef::new(&self.bytes).map_err(|e| e.to_string())?;
        let size = Size::new(size);
        let location = LocationRef::default();
        let metrics = font.metrics(size, location);
        let glyph_metrics = font.glyph_metrics(size, location);
        let charmap = font.charmap();
        let lines: Vec<Vec<_>> = content
            .split('\n')
            .map(|line| {
                line.trim_end_matches('\r')
                    .chars()
                    .map(|c| charmap.map(c).unwrap_or_default())
                    .collect()
            })
            .collect();
        let line_height = metrics.ascent - metrics.descent + metrics.leading;
        let text_height = line_height * lines.len() as f32;
        let pad = (width.min(height) as f32 * 0.06).max(2.0 * height as f32 / 512.0);
        let top = if alignment.starts_with("Top") {
            pad
        } else if alignment.starts_with("Bottom") {
            height as f32 - pad - text_height
        } else {
            (height as f32 - text_height) / 2.0
        };
        let outlines = font.outline_glyphs();
        let mut context = RenderContext::new(width as u16, height as u16);
        context.set_paint(vello_cpu::color::palette::css::WHITE);
        if let Some(width) = stroke {
            context.set_stroke(
                Stroke::new(width)
                    .with_join(Join::Round)
                    .with_caps(Cap::Round),
            );
        }
        let mut segments = 0;
        for (line, glyphs) in lines.into_iter().enumerate() {
            let text_width: f32 = glyphs
                .iter()
                .map(|g| glyph_metrics.advance_width(*g).unwrap_or(0.0))
                .sum();
            let mut pen_x = if alignment.ends_with("Left") {
                pad
            } else if alignment.ends_with("Right") {
                width as f32 - pad - text_width
            } else {
                (width as f32 - text_width) / 2.0
            };
            pen_x = pen_x.max(0.0);
            let baseline = top.max(0.0) + metrics.ascent + line as f32 * line_height;
            for glyph_id in glyphs {
                if cancelled() {
                    return Err("Icon rendering cancelled".into());
                }
                if let Some(glyph) = outlines.get(glyph_id) {
                    let mut path = Pen {
                        path: BezPath::new(),
                        segments: &mut segments,
                    };
                    glyph
                        .draw(DrawSettings::unhinted(size, location), &mut path)
                        .map_err(|e| e.to_string())?;
                    if *path.segments > 65_536 {
                        return Err("Text outlines exceed their work budget".into());
                    }
                    context.set_transform(Affine::new([
                        1.0,
                        0.0,
                        0.0,
                        -1.0,
                        f64::from(pen_x),
                        f64::from(baseline),
                    ]));
                    if stroke.is_some() {
                        context.stroke_path(&path.path);
                    } else {
                        context.fill_path(&path.path);
                    }
                }
                pen_x += glyph_metrics.advance_width(glyph_id).unwrap_or(0.0);
            }
        }
        context.flush();
        let mut pixels = Pixmap::new(width as u16, height as u16);
        context.render(&mut pixels, &mut Resources::new());
        Ok(RgbaImage::from_fn(width, height, |x, y| {
            let coverage = pixels.data()[(y * width + x) as usize].a;
            Rgba([
                color[0],
                color[1],
                color[2],
                (u16::from(coverage) * u16::from(color[3]) / 255) as u8,
            ])
        }))
    }
}
struct Pen<'a> {
    path: BezPath,
    segments: &'a mut usize,
}
impl Pen<'_> {
    fn allowed(&mut self) -> bool {
        *self.segments += 1;
        *self.segments <= 65_536
    }
}
impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.allowed() {
            self.path.move_to((f64::from(x), f64::from(y)));
        }
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if self.allowed() {
            self.path.line_to((f64::from(x), f64::from(y)));
        }
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        if self.allowed() {
            self.path
                .quad_to((f64::from(cx), f64::from(cy)), (f64::from(x), f64::from(y)));
        }
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        if self.allowed() {
            self.path.curve_to(
                (f64::from(ax), f64::from(ay)),
                (f64::from(bx), f64::from(by)),
                (f64::from(x), f64::from(y)),
            );
        }
    }
    fn close(&mut self) {
        if self.allowed() {
            self.path.close_path();
        }
    }
}
