//! Embedded, tintable layer symbols shared by the picker and adjustment rows.
use egui::{TextureHandle, TextureId};

pub(crate) const LAYERS: [&str; 13] = [
    "AtlasTile",
    "AchievementImage",
    "StandaloneImage",
    "Text",
    "Text3d",
    "Artifact3d",
    "Solid",
    "Shape",
    "Particle",
    "Explosion",
    "SdfVignette",
    "Gradient",
    "PerlinNoise",
];
pub(crate) const ADJUSTMENTS: [&str; 7] = [
    "Curves",
    "Contrast",
    "HueSaturation",
    "SelectiveColor",
    "UnsharpMask",
    "RadialBlur",
    "GradientOverlay",
];

pub(crate) fn texture(ctx: &egui::Context, kind: &str) -> Option<TextureId> {
    let bytes: &[u8] = match kind {
        "Folder" => include_bytes!("../assets/toolbar/layer-folder.rgba"),
        "MaskReference" => include_bytes!("../assets/toolbar/layer-mask.rgba"),
        "SteamImportedArtwork" => include_bytes!("../assets/toolbar/art-icon-images.rgba"),
        "AtlasTile" => include_bytes!("../assets/toolbar/art-atlases.rgba"),
        "AchievementImage" => include_bytes!("../assets/toolbar/art-general-images.rgba"),
        "StandaloneImage" => include_bytes!("../assets/toolbar/art-icon-images.rgba"),
        "Text" => include_bytes!("../assets/toolbar/layer-text.rgba"),
        "Text3d" => include_bytes!("../assets/toolbar/layer-text3d.rgba"),
        "Artifact3d" => include_bytes!("../assets/toolbar/layer-artifact3d.rgba"),
        "Solid" => include_bytes!("../assets/toolbar/layer-solid.rgba"),
        "Shape" => include_bytes!("../assets/toolbar/layer-shape.rgba"),
        "Particle" => include_bytes!("../assets/toolbar/layer-particle.rgba"),
        "Explosion" => include_bytes!("../assets/toolbar/layer-explosion.rgba"),
        "SdfVignette" => include_bytes!("../assets/toolbar/layer-vignette.rgba"),
        "Gradient" => include_bytes!("../assets/toolbar/layer-gradient.rgba"),
        "PerlinNoise" => include_bytes!("../assets/toolbar/layer-perlin-noise.rgba"),
        "Curves" => include_bytes!("../assets/toolbar/layer-curves.rgba"),
        "Contrast" => include_bytes!("../assets/toolbar/layer-contrast.rgba"),
        "HueSaturation" => include_bytes!("../assets/toolbar/layer-hue-saturation.rgba"),
        "SelectiveColor" => include_bytes!("../assets/toolbar/layer-selective-color.rgba"),
        "UnsharpMask" => include_bytes!("../assets/toolbar/layer-unsharp-mask.rgba"),
        "RadialBlur" => include_bytes!("../assets/toolbar/layer-radial-blur.rgba"),
        "GradientOverlay" => include_bytes!("../assets/toolbar/layer-gradient-overlay.rgba"),
        _ => return None,
    };
    // Only compiled-in layer kinds can populate this bounded context cache.
    let id = egui::Id::new(("layer-type-icon", kind));
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(texture.id());
    }
    let texture = ctx.load_texture(
        format!("layer-type-{kind}"),
        egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    let texture_id = texture.id();
    ctx.data_mut(|data| data.insert_temp(id, texture));
    Some(texture_id)
}
