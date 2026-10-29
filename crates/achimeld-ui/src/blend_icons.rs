//! Generated blend symbols, embedded and tinted with the shared button ink.
pub(crate) fn image(ctx: &egui::Context, mode: &str, size: f32) -> egui::Image<'static> {
    let mode = if crate::blend_preview::MODES.contains(&mode) {
        mode
    } else {
        "control"
    };
    let bytes: &[u8] = match mode {
        "Normal" => include_bytes!("../assets/toolbar/blend-normal.rgba"),
        "Multiply" => include_bytes!("../assets/toolbar/blend-multiply.rgba"),
        "Screen" => include_bytes!("../assets/toolbar/blend-screen.rgba"),
        "Overlay" => include_bytes!("../assets/toolbar/blend-overlay.rgba"),
        "Darken" => include_bytes!("../assets/toolbar/blend-darken.rgba"),
        "Lighten" => include_bytes!("../assets/toolbar/blend-lighten.rgba"),
        "Add" => include_bytes!("../assets/toolbar/blend-add.rgba"),
        "Difference" => include_bytes!("../assets/toolbar/blend-difference.rgba"),
        "Color" => include_bytes!("../assets/toolbar/blend-color.rgba"),
        "Hue" => include_bytes!("../assets/toolbar/blend-hue.rgba"),
        "Saturation" => include_bytes!("../assets/toolbar/blend-saturation.rgba"),
        "Luminosity" => include_bytes!("../assets/toolbar/blend-luminosity.rgba"),
        "Soft Light" => include_bytes!("../assets/toolbar/blend-soft-light.rgba"),
        "Hard Light" => include_bytes!("../assets/toolbar/blend-hard-light.rgba"),
        "Color Dodge" => include_bytes!("../assets/toolbar/blend-color-dodge.rgba"),
        "Color Burn" => include_bytes!("../assets/toolbar/blend-color-burn.rgba"),
        "Exclusion" => include_bytes!("../assets/toolbar/blend-exclusion.rgba"),
        _ => include_bytes!("../assets/toolbar/blend-control.rgba"),
    };
    let id = egui::Id::new(("blend-mode-icon", mode));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture = ctx.load_texture(
                format!("blend-mode-{mode}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), egui::Vec2::splat(size)))
}
