//! Generated effect and parameter symbols; only these compiled assets enter the cache.
pub(crate) fn image(ctx: &egui::Context, kind: &str, size: f32) -> egui::Image<'static> {
    let (name, bytes): (&str, &[u8]) = match kind {
        "ColorOverlay" => (
            "color-overlay",
            include_bytes!("../assets/toolbar/effect-color-overlay.rgba"),
        ),
        "GradientOverlay" => (
            "gradient-overlay",
            include_bytes!("../assets/toolbar/effect-gradient-overlay.rgba"),
        ),
        "InnerLight" => (
            "inner-light",
            include_bytes!("../assets/toolbar/effect-inner-light.rgba"),
        ),
        "OuterLight" => (
            "outer-light",
            include_bytes!("../assets/toolbar/effect-outer-light.rgba"),
        ),
        "Shadow" => (
            "shadow",
            include_bytes!("../assets/toolbar/effect-shadow.rgba"),
        ),
        "BoxBlur" => (
            "box-blur",
            include_bytes!("../assets/toolbar/effect-box-blur.rgba"),
        ),
        "Skew" => ("skew", include_bytes!("../assets/toolbar/effect-skew.rgba")),
        "Perspective" => (
            "perspective",
            include_bytes!("../assets/toolbar/effect-perspective.rgba"),
        ),
        "Pinch" => (
            "pinch",
            include_bytes!("../assets/toolbar/effect-pinch.rgba"),
        ),
        "LensBlur" => (
            "lens-blur",
            include_bytes!("../assets/toolbar/effect-lens-blur.rgba"),
        ),
        "Width" => (
            "width",
            include_bytes!("../assets/toolbar/effect-width.rgba"),
        ),
        "Color" => (
            "color",
            include_bytes!("../assets/toolbar/effect-color.rgba"),
        ),
        _ => (
            "outline",
            include_bytes!("../assets/toolbar/effect-outline.rgba"),
        ),
    };
    let id = egui::Id::new(("effect-type-icon", name));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let operation = achimeld_application::diagnostics::Operation::begin("load effect icon");
            operation.checkpoint(name);
            let texture = ctx.load_texture(
                format!("effect-{name}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            operation.finish("completed");
            texture
        });
    egui::Image::new((texture.id(), egui::Vec2::splat(size)))
}
