use achimeld_application::{Document, EntityKind};
use image::{Rgba, RgbaImage};
use serde_json::Value;
use std::sync::Arc;
pub struct IconPair {
    pub achieved: RgbaImage,
    pub unachieved: RgbaImage,
}
pub trait AssetSource {
    fn image(&mut self, reference: &Value) -> Result<Arc<RgbaImage>, String>;
    fn font(&mut self, family: &str) -> Option<Arc<crate::Font>>;
}
fn num(v: &Value, key: &str, default: f64) -> f64 {
    v[key].as_f64().filter(|v| v.is_finite()).unwrap_or(default)
}
pub fn rgba(text: &str) -> Result<Rgba<u8>, String> {
    let text = text.strip_prefix('#').ok_or("Colors must begin with #")?;
    let bytes = hex_color(text)?;
    match bytes.as_slice() {
        [r, g, b] => Ok(Rgba([*r, *g, *b, 255])),
        [a, r, g, b] => Ok(Rgba([*r, *g, *b, *a])),
        _ => Err("Colors require RGB or ARGB hexadecimal values".into()),
    }
}
fn hex_color(value: &str) -> Result<Vec<u8>, String> {
    if ![6, 8].contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid color length".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "Invalid color".into()))
        .collect()
}
pub fn render(
    document: &Document,
    id: &str,
    size: Option<(u32, u32)>,
    assets: &mut dyn AssetSource,
) -> Result<IconPair, String> {
    render_cancellable(document, id, size, assets, &|| false)
}
pub fn render_cancellable(
    document: &Document,
    id: &str,
    size: Option<(u32, u32)>,
    assets: &mut dyn AssetSource,
    cancelled: &dyn Fn() -> bool,
) -> Result<IconPair, String> {
    let mut work = Work {
        cancelled,
        allow_missing_art: false,
        planes: 0,
        pixels: 0,
        art_stack: Vec::new(),
        mask_ids: Default::default(),
    };
    let achieved = compose_achieved(document, id, size, assets, &mut work)?;
    work.plane(achieved.width(), achieved.height())?;
    if let Some(locked_id) = document
        .entity(EntityKind::Achievement, id)
        .and_then(|entity| entity["unachievedImageId"].as_str())
    {
        let reference = document.project()["assets"]["images"]
            .as_array()
            .and_then(|images| images.iter().find(|image| image["id"] == locked_id))
            .map(|image| &image["image"])
            .ok_or("Locked icon image asset is missing")?;
        let source = assets.image(reference)?;
        work.check()?;
        let unachieved = image::imageops::resize(
            source.as_ref(),
            achieved.width(),
            achieved.height(),
            image::imageops::FilterType::Lanczos3,
        );
        return Ok(IconPair {
            achieved,
            unachieved,
        });
    }
    let settings = &document.project()["atlas"]["unachievedIcon"];
    let tint = match settings["mode"].as_str().unwrap_or("Grayscale") {
        "Grayscale" => None,
        "Tint" => Some(rgba(settings["tintHex"].as_str().unwrap_or("#808080"))?),
        mode => return Err(format!("Unsupported unachieved icon mode: {mode}")),
    };
    let opacity = num(settings, "opacity", 1.0).clamp(0.0, 1.0);
    let mut unachieved = achieved.clone();
    for pixel in unachieved.pixels_mut() {
        let grey = (0.2126 * f64::from(pixel[0])
            + 0.7152 * f64::from(pixel[1])
            + 0.0722 * f64::from(pixel[2]))
        .round() as u8;
        for i in 0..3 {
            pixel[i] = tint.map_or(grey, |tint| {
                ((f64::from(grey) + f64::from(tint[i])) / 2.0).round() as u8
            });
        }
        pixel[3] = (f64::from(pixel[3]) * opacity).round() as u8;
    }
    Ok(IconPair {
        achieved,
        unachieved,
    })
}
/// Compose only the displayed icon; export still produces both variants.
pub fn render_achieved_cancellable(
    document: &Document,
    id: &str,
    size: Option<(u32, u32)>,
    assets: &mut dyn AssetSource,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    compose_achieved(
        document,
        id,
        size,
        assets,
        &mut Work {
            cancelled,
            allow_missing_art: true,
            planes: 0,
            pixels: 0,
            art_stack: Vec::new(),
            mask_ids: Default::default(),
        },
    )
}
fn compose_achieved(
    document: &Document,
    id: &str,
    size: Option<(u32, u32)>,
    assets: &mut dyn AssetSource,
    work: &mut Work<'_>,
) -> Result<RgbaImage, String> {
    work.check()?;
    let project = document.project();
    let achievement = document
        .entity(EntityKind::Achievement, id)
        .ok_or("Achievement no longer exists")?;
    let settings = &project["atlas"];
    let default = settings["outputSize"].as_u64().unwrap_or(512) as u32;
    let (width, height) = size.unwrap_or((
        settings["outputWidth"].as_u64().unwrap_or(default.into()) as u32,
        settings["outputHeight"].as_u64().unwrap_or(default.into()) as u32,
    ));
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("Icon dimensions must be between 1 and 4096 pixels".into());
    }
    compose_icon(project, achievement, width, height, 0, assets, work)
}
fn compose_icon(
    project: &Value,
    achievement: &Value,
    width: u32,
    height: u32,
    depth: usize,
    assets: &mut dyn AssetSource,
    work: &mut Work<'_>,
) -> Result<RgbaImage, String> {
    let id = achievement["id"].as_str().ok_or("Missing icon identity")?;
    if work.art_stack.len() > achimeld_application::art_images::MAX_LINK_DEPTH
        || work.art_stack.iter().any(|active| active == id)
    {
        return Err("Art image link cycle or nesting limit exceeded".into());
    }
    work.art_stack.push(id.to_owned());
    let result = compose_recipe(project, achievement, width, height, depth, assets, work);
    work.art_stack.pop();
    result
}
fn compose_recipe(
    project: &Value,
    achievement: &Value,
    width: u32,
    height: u32,
    depth: usize,
    assets: &mut dyn AssetSource,
    work: &mut Work<'_>,
) -> Result<RgbaImage, String> {
    work.plane(width, height)?;

    let mut achieved = RgbaImage::new(width, height);
    let recipe = &achievement["icon"];
    if let Some(background) = recipe["backgroundColorHex"].as_str() {
        let c = rgba(background)?;
        for p in achieved.pixels_mut() {
            *p = c;
        }
    }
    if let Some(layers) = recipe["layers"].as_array() {
        if layers.len() > 512 {
            return Err("Icon has too many layers".into());
        }
        work.mask_ids.extend(
            achimeld_application::masks::content_ids(layers)
                .into_iter()
                .map(str::to_owned),
        );
        group(
            &mut achieved,
            layers,
            None,
            achievement,
            project,
            assets,
            depth,
            work,
        )?;
    } else {
        let layer = serde_json::json!({"kind":if achievement["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"atlasId":achievement["atlasId"],"imageId":achievement["standaloneImageId"],"tileIndex":achievement["tileIndex"]});
        let plane = layer_image(
            &layer,
            achievement,
            project,
            width,
            height,
            assets,
            work.cancelled,
            work.allow_missing_art,
        )?;
        composite(&mut achieved, &plane, 1.0, "Normal");
    }
    Ok(achieved)
}
/// Render one layer and its folder/mask dependencies with the same budgets as
/// icon composition. Hidden layers still have artwork in the layer palette.
pub fn render_layer_cancellable(
    document: &Document,
    achievement_id: &str,
    layer_id: &str,
    assets: &mut dyn AssetSource,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    let achievement = document
        .entity(EntityKind::Achievement, achievement_id)
        .ok_or("Achievement is missing")?;
    let layers = achievement["icon"]["layers"].as_array();
    let fallback = serde_json::json!({"id":achievement_id,"kind":if achievement["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"atlasId":achievement["atlasId"],"imageId":achievement["standaloneImageId"],"tileIndex":achievement["tileIndex"]});
    let fallback_layers = [fallback];
    let layers = layers.map_or(fallback_layers.as_slice(), Vec::as_slice);
    let layer = layers
        .iter()
        .find(|l| l["id"].as_str() == Some(layer_id))
        .ok_or("Layer is missing")?;
    // Only adjustment sources need a backdrop. Ordinary noise/image masks keep
    // the direct thumbnail path instead of recomposing the stack for each row.
    let mut source = layer;
    let mut needs_backdrop = false;
    for _ in 0..=64 {
        if is_adjustment(source) {
            needs_backdrop = true;
            break;
        }
        let next = if source["kind"] == "MaskReference" {
            source["maskSourceLayerId"]
                .as_str()
                .and_then(|id| layers.iter().find(|layer| layer["id"].as_str() == Some(id)))
                .and_then(|owner| owner["maskLayerId"].as_str())
        } else {
            source["maskLayerId"].as_str()
        };
        let Some(mask) = next else {
            break;
        };
        let Some(next) = layers.iter().find(|l| l["id"].as_str() == Some(mask)) else {
            break;
        };
        source = next;
    }
    if needs_backdrop {
        let index = layers
            .iter()
            .position(|l| l["id"].as_str() == Some(layer_id))
            .ok_or("Layer is missing")?;
        let mut input = layers.to_vec();
        for (i, l) in input.iter_mut().enumerate() {
            if i >= index && l["parentFolderId"] == layer["parentFolderId"] {
                l["visible"] = serde_json::json!(false);
            }
        }
        let mut image = RgbaImage::new(128, 128);
        let mut work = Work {
            cancelled,
            allow_missing_art: true,
            planes: 0,
            pixels: 0,
            art_stack: Vec::new(),
            mask_ids: achimeld_application::masks::content_ids(layers)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        group(
            &mut image,
            &input,
            layer["parentFolderId"].as_str(),
            achievement,
            document.project(),
            assets,
            0,
            &mut work,
        )?;
        return plane(
            layer,
            layers,
            achievement,
            document.project(),
            128,
            128,
            assets,
            0,
            &mut work,
            Some(&image),
        );
    }
    let mut work = Work {
        cancelled,
        allow_missing_art: true,
        planes: 0,
        pixels: 0,
        art_stack: Vec::new(),
        mask_ids: achimeld_application::masks::content_ids(layers)
            .into_iter()
            .map(str::to_owned)
            .collect(),
    };
    plane(
        layer,
        layers,
        achievement,
        document.project(),
        128,
        128,
        assets,
        0,
        &mut work,
        None,
    )
}
struct Work<'a> {
    cancelled: &'a dyn Fn() -> bool,
    allow_missing_art: bool,
    planes: u64,
    pixels: u64,
    art_stack: Vec<String>,
    mask_ids: std::collections::BTreeSet<String>,
}
impl Work<'_> {
    fn check(&self) -> Result<(), String> {
        if (self.cancelled)() {
            Err("Icon rendering cancelled".into())
        } else {
            Ok(())
        }
    }
    fn plane(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.check()?;
        self.planes += 1;
        self.pixels += u64::from(width) * u64::from(height);
        if self.planes > 2048 || self.pixels > 512 * 1024 * 1024 {
            return Err("Icon exceeds the render work budget".into());
        }
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
fn group(
    target: &mut RgbaImage,
    layers: &[Value],
    parent: Option<&str>,
    achievement: &Value,
    project: &Value,
    assets: &mut dyn AssetSource,
    depth: usize,
    work: &mut Work<'_>,
) -> Result<(), String> {
    let masks: std::collections::BTreeSet<_> = layers
        .iter()
        .filter_map(|l| l["maskLayerId"].as_str())
        .collect();
    for layer in layers {
        work.check()?;
        if layer["visible"] == false
            || layer["parentFolderId"].as_str() != parent
            || layer["id"].as_str().is_some_and(|id| masks.contains(id))
        {
            continue;
        }
        check_blend_mode(layer["blendMode"].as_str().unwrap_or("Normal"))?;
        if matches!(
            layer["kind"].as_str(),
            Some(
                "Curves"
                    | "Contrast"
                    | "HueSaturation"
                    | "SelectiveColor"
                    | "UnsharpMask"
                    | "RadialBlur"
                    | "GradientOverlay"
            )
        ) {
            work.plane(target.width(), target.height())?;
            let original = layer["maskLayerId"].is_string().then(|| target.clone());
            if layer["kind"] == "GradientOverlay" {
                gradient_map(
                    target,
                    &layer["gradient"],
                    num(layer, "opacity", 1.0),
                    layer["blendMode"].as_str().unwrap_or("Normal"),
                )?;
            } else if layer["kind"] == "RadialBlur" {
                radial_adjustment(target, layer, work)?;
            } else {
                adjust(target, layer);
            }
            if let Some(original) = original {
                let mask_id = layer["maskLayerId"].as_str().ok_or("Invalid mask ID")?;
                let source = layers
                    .iter()
                    .find(|l| l["id"].as_str() == Some(mask_id))
                    .ok_or("Mask layer is missing")?;
                let mask = plane(
                    source,
                    layers,
                    achievement,
                    project,
                    target.width(),
                    target.height(),
                    assets,
                    depth + 1,
                    work,
                    Some(&original),
                )?;
                for ((pixel, before), mask) in target
                    .pixels_mut()
                    .zip(original.pixels())
                    .zip(mask.pixels())
                {
                    let strength = mask_strength(layer, source, mask);
                    if layer["kind"] == "RadialBlur" {
                        *pixel = crate::radial_blur::mix(*before, *pixel, strength, "Normal");
                        continue;
                    }
                    for channel in 0..4 {
                        pixel[channel] = (f64::from(before[channel]) * (1.0 - strength)
                            + f64::from(pixel[channel]) * strength)
                            .round() as u8;
                    }
                }
            }
            continue;
        }
        let plane = plane(
            layer,
            layers,
            achievement,
            project,
            target.width(),
            target.height(),
            assets,
            depth,
            work,
            Some(target),
        )?;
        composite(
            target,
            &plane,
            num(layer, "opacity", 1.0).clamp(0.0, 1.0),
            layer["blendMode"].as_str().unwrap_or("Normal"),
        );
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn plane(
    layer: &Value,
    layers: &[Value],
    achievement: &Value,
    project: &Value,
    width: u32,
    height: u32,
    assets: &mut dyn AssetSource,
    depth: usize,
    work: &mut Work<'_>,
    backdrop: Option<&RgbaImage>,
) -> Result<RgbaImage, String> {
    work.plane(width, height)?;
    if layer["kind"] == "Explosion" {
        work.pixels += crate::explosion::estimate(
            &layer["explosion"],
            achievement["id"].as_str().unwrap_or(""),
            width,
            height,
        )?;
        if work.pixels > 512 * 1024 * 1024 {
            return Err("Icon exceeds the render work budget".into());
        }
    }
    if layer["kind"] == "Particle" {
        work.pixels += crate::particles::estimate(
            &layer["particles"],
            achievement["id"].as_str().unwrap_or(""),
            width,
            height,
        )?;
        if work.pixels > 512 * 1024 * 1024 {
            return Err("Icon exceeds the render work budget".into());
        }
    }
    if depth > 64
        || u64::from(width) * u64::from(height) * 4 * (depth as u64 + 4) > 256 * 1024 * 1024
    {
        return Err("Icon nesting exceeds the render memory budget".into());
    }
    let mut image = if layer["kind"] == "MaskReference" {
        // Copy coverage, never the referenced layer's artwork or opacity.
        let owner = layer["maskSourceLayerId"].as_str().and_then(|id| {
            layers
                .iter()
                .find(|source| source["id"].as_str() == Some(id))
        });
        let mask = owner
            .and_then(|owner| owner["maskLayerId"].as_str())
            .and_then(|id| {
                layers
                    .iter()
                    .find(|source| source["id"].as_str() == Some(id))
            });
        let image = if let (Some(owner), Some(source)) = (owner, mask) {
            let mut image = plane(
                source,
                layers,
                achievement,
                project,
                width,
                height,
                assets,
                depth + 1,
                work,
                backdrop,
            )?;
            for row in image.rows_mut() {
                work.check()?;
                for pixel in row {
                    let gray = (255.0 * mask_strength(owner, source, pixel)).round() as u8;
                    *pixel = Rgba([gray, gray, gray, 255]);
                }
            }
            image
        } else {
            // An unassigned reference or a layer without a mask fully reveals.
            RgbaImage::from_pixel(width, height, Rgba([255; 4]))
        };
        transform(image, layer, width, height)
    } else if layer["kind"] == "Folder" {
        let mut image = RgbaImage::new(width, height);
        group(
            &mut image,
            layers,
            Some(layer["id"].as_str().ok_or("Missing folder ID")?),
            achievement,
            project,
            assets,
            depth + 1,
            work,
        )?;
        transform(image, layer, width, height)
    } else if let Some(id) = achimeld_application::art_images::layer_source(achievement, layer) {
        let source = project["achievements"].as_array().and_then(|entities| {
            entities.iter().find(|entity| {
                entity["id"].as_str() == Some(id)
                    && achimeld_application::art_images::is_art_image(entity)
            })
        });
        match source {
            Some(source) => transform(
                compose_icon(project, source, width, height, depth + 1, assets, work)?,
                layer,
                width,
                height,
            ),
            None if work.allow_missing_art => RgbaImage::new(width, height),
            None => return Err("Linked art image is missing".into()),
        }
    } else if is_adjustment(layer) {
        // Adjustment sources use the owner's current composited backdrop.
        // Passing this immutable snapshot avoids implicit graph dependencies.
        let mut image = backdrop
            .cloned()
            .unwrap_or_else(|| RgbaImage::new(width, height));
        let mut settings = layer.clone();
        settings["opacity"] = serde_json::json!(1.0);
        if layer["kind"] == "GradientOverlay" {
            gradient_map(
                &mut image,
                &layer["gradient"],
                1.0,
                layer["blendMode"].as_str().unwrap_or("Normal"),
            )?;
        } else if layer["kind"] == "RadialBlur" {
            radial_adjustment(&mut image, &settings, work)?;
        } else {
            adjust(&mut image, &settings);
        }
        transform(image, layer, width, height)
    } else {
        if layer["kind"] == "PerlinNoise" {
            // Charge octave sampling against the shared, bounded render budget.
            for _ in 0..layer["perlinNoise"]["octaves"]
                .as_u64()
                .unwrap_or(6)
                .clamp(1, 12)
            {
                work.plane(width, height)?;
            }
        }
        layer_image(
            layer,
            achievement,
            project,
            width,
            height,
            assets,
            work.cancelled,
            work.allow_missing_art,
        )?
    };
    if let Some(id) = layer["maskLayerId"].as_str() {
        let mask = layers
            .iter()
            .find(|l| l["id"].as_str() == Some(id))
            .ok_or("Mask layer is missing")?;
        let source = mask;
        let mask = plane(
            mask,
            layers,
            achievement,
            project,
            width,
            height,
            assets,
            depth + 1,
            work,
            backdrop,
        )?;
        for (pixel, mask) in image.pixels_mut().zip(mask.pixels()) {
            pixel[3] = (f64::from(pixel[3]) * mask_strength(layer, source, mask)).round() as u8;
        }
    }
    work.check()?;
    for _effect in layer["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|effect| effect["visible"] != false)
    {
        work.plane(width, height)?;
    }
    if layer["kind"] == "Text" && layer["text"]["outlineEnabled"] == true {
        work.plane(width, height)?;
    }
    let font = if layer["kind"] == "Text" {
        assets.font(layer["text"]["fontFamily"].as_str().unwrap_or("Segoe UI"))
    } else {
        None
    };
    let outline = |effect: &Value| {
        let font = font.as_ref().ok_or("Font is unavailable")?;
        let text = &layer["text"];
        let image = font.text_outline(
            &text_content(text, achievement),
            (num(text, "fontSize", 128.0) * f64::from(height) / 512.0) as f32,
            rgba(effect["colorHex"].as_str().unwrap_or("#FF000000"))?,
            text["alignment"].as_str().unwrap_or("BottomRight"),
            (width, height),
            work.cancelled,
            (num(effect, "width", 2.0) * f64::from(height) / 512.0).clamp(0.0, 64.0),
            effect["placement"].as_str().unwrap_or("Center"),
        )?;
        Ok(transform(image, layer, width, height))
    };
    let mut image = crate::effects::apply(
        image,
        layer,
        width,
        height,
        work.cancelled,
        font.as_ref()
            .map(|_| &outline as &crate::effects::OutlineRenderer<'_>),
    )?;
    if layer["id"]
        .as_str()
        .is_some_and(|id| work.mask_ids.contains(id))
    {
        for row in image.rows_mut() {
            work.check()?;
            for pixel in row {
                *pixel = Rgba(achimeld_application::masks::grayscale(pixel.0));
            }
        }
    }
    Ok(image)
}
fn text_content(text: &Value, achievement: &Value) -> String {
    if text["linkToAchievementValue"] == true {
        achievement["unlockValue"]
            .as_number()
            .map(ToString::to_string)
            .unwrap_or_default()
    } else {
        text["content"].as_str().unwrap_or("").to_owned()
    }
}
#[test]
fn value_linked_text_is_blank_until_an_unlock_value_is_assigned() {
    use serde_json::json;
    let linked = json!({"linkToAchievementValue":true,"content":"Template"});
    assert_eq!(text_content(&linked, &json!({"unlockValue":null})), "");
    assert_eq!(text_content(&linked, &json!({"unlockValue":12.5})), "12.5");
    assert_eq!(
        text_content(&json!({"content":"Template"}), &json!({"unlockValue":null})),
        "Template"
    );
}
fn is_adjustment(layer: &Value) -> bool {
    matches!(
        layer["kind"].as_str(),
        Some(
            "Curves"
                | "Contrast"
                | "HueSaturation"
                | "SelectiveColor"
                | "UnsharpMask"
                | "RadialBlur"
                | "GradientOverlay"
        )
    )
}

fn mask_strength(owner: &Value, source: &Value, pixel: &Rgba<u8>) -> f64 {
    let alpha = f64::from(pixel[3]) / 255.0;
    let coverage = if owner["maskMode"] == "Luminance" {
        alpha
            * (0.2126 * f64::from(pixel[0])
                + 0.7152 * f64::from(pixel[1])
                + 0.0722 * f64::from(pixel[2]))
            / 255.0
    } else {
        alpha
    };
    let coverage = if owner["maskInvert"] == true {
        1.0 - coverage
    } else {
        coverage
    };
    coverage * num(source, "opacity", 1.0).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn layer_image(
    layer: &Value,
    achievement: &Value,
    project: &Value,
    width: u32,
    height: u32,
    assets: &mut dyn AssetSource,
    cancelled: &dyn Fn() -> bool,
    allow_missing_art: bool,
) -> Result<RgbaImage, String> {
    let mut image = RgbaImage::new(width, height);
    let kind = layer["kind"].as_str().unwrap_or("");
    match kind {
        "Solid" => {
            let color = rgba(layer["solidColorHex"].as_str().unwrap_or("#FFFFFFFF"))?;
            for p in image.pixels_mut() {
                *p = color;
            }
        }
        "PerlinNoise" => {
            image = crate::perlin::render(&layer["perlinNoise"], width, height, cancelled)?;
        }
        "Gradient" | "GradientOverlay" => {
            image = gradient_image(&layer["gradient"], width, height, cancelled)?
        }
        "Text" => {
            let text = &layer["text"];
            let content = text_content(text, achievement);
            let font = assets
                .font(text["fontFamily"].as_str().unwrap_or("Segoe UI"))
                .ok_or("Font is unavailable")?;
            let size = (num(text, "fontSize", 128.0) * f64::from(height) / 512.0) as f32;
            let color = rgba(text["colorHex"].as_str().unwrap_or("#FFFFFFFF"))?;
            if size > 4096.0 || content.chars().count() > 1024 {
                return Err("Icon text exceeds the rasterization budget".into());
            }
            let alignment = text["alignment"].as_str().unwrap_or("BottomRight");
            image = font.text(&content, size, color, alignment, (width, height), cancelled)?;
        }
        "Artifact3d" => {
            image = crate::artifact3d::render(&layer["artifact3d"], (width, height), cancelled)?;
        }
        "Text3d" => {
            let text = &layer["text"];
            let content = text_content(text, achievement);
            let font = assets
                .font(text["fontFamily"].as_str().unwrap_or("Segoe UI"))
                .ok_or("Font is unavailable")?;
            let settings =
                achimeld_application::text3d::Text3dSettings::from_value(&layer["text3d"])?;
            image = crate::text3d::render(
                &font,
                &content,
                num(text, "fontSize", 128.0) as f32,
                rgba(text["colorHex"].as_str().unwrap_or("#FFFFFFFF"))?,
                &settings,
                (width, height),
                cancelled,
            )?;
        }
        "Shape" => {
            image = crate::shape::render(layer, width, height)?;
        }
        "Particle" => {
            image = crate::particles::render(
                &layer["particles"],
                achievement["id"].as_str().unwrap_or(""),
                width,
                height,
                cancelled,
            )?;
        }
        "Explosion" => {
            image = crate::explosion::render(
                &layer["explosion"],
                achievement["id"].as_str().unwrap_or(""),
                width,
                height,
                cancelled,
            )?;
        }
        "SdfVignette" => {
            let settings = &layer["sdfVignette"];
            let invert = settings["invert"] == true;
            let colors = (settings["useGradient"] == true)
                .then(|| gradient_lookup(&settings["gradient"]))
                .transpose()?;
            let color = if colors.is_some() {
                Rgba([0; 4])
            } else {
                rgba(settings["colorHex"].as_str().unwrap_or("#FF000000"))?
            };
            let unit = f64::from(width.min(height)) / 2.0;
            let inset = num(settings, "inset", 0.15).clamp(0.0, 0.95) * unit;
            let hx = f64::from(width) / 2.0 - inset;
            let hy = f64::from(height) / 2.0 - inset;
            let radius = num(settings, "roundness", 0.5).clamp(0.0, 1.0) * hx.min(hy);
            let feather = (num(settings, "softness", 0.2).clamp(0.0, 1.0) * unit)
                .max(f64::from(height) / 512.0);
            for (y, mut row) in image.enumerate_rows_mut() {
                if cancelled() {
                    return Err("Icon rendering cancelled".into());
                }
                for (x, _, p) in &mut row {
                    let qx = (f64::from(x) + 0.5 - f64::from(width) / 2.0).abs() - hx + radius;
                    let qy = (f64::from(y) + 0.5 - f64::from(height) / 2.0).abs() - hy + radius;
                    let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
                    let t = ((distance + feather) / feather).clamp(0.0, 1.0);
                    let coverage = t * t * (3.0 - 2.0 * t);
                    let alpha = if invert { 1.0 - coverage } else { coverage };
                    let color = colors
                        .as_ref()
                        .map_or(color, |colors| colors[(alpha * 255.0).round() as usize]);
                    *p = Rgba([
                        color[0],
                        color[1],
                        color[2],
                        (f64::from(color[3]) * alpha).round() as u8,
                    ]);
                }
            }
        }
        "AtlasTile" | "StandaloneImage" | "SteamImportedArtwork" | "AchievementImage" => {
            let mut effective = layer.clone();
            if kind == "AtlasTile" && layer["linkAtlasToGeneral"] == true {
                effective["atlasId"] = achievement["atlasId"].clone();
                effective["tileIndex"] = achievement["tileIndex"].clone();
            }
            if kind == "AchievementImage" {
                if let Some(entry) =
                    achievement["generalImageEntries"]
                        .as_array()
                        .and_then(|entries| {
                            entries
                                .iter()
                                .find(|e| e["id"] == layer["generalImageIndex"])
                        })
                {
                    effective["imageId"] = entry["imageId"].clone();
                    effective["atlasId"] = entry["atlasId"].clone();
                    effective["tileIndex"] = entry["tileIndex"].clone();
                } else {
                    effective["imageId"] = achievement["generalImageId"].clone();
                    effective["atlasId"] = achievement["generalAtlasId"].clone();
                    effective["tileIndex"] = achievement["generalTileIndex"].clone();
                }
            }
            if kind != "AtlasTile"
                && effective["imageId"].is_null()
                && effective["atlasId"].is_null()
            {
                return Ok(image);
            }
            let default_general_atlas = kind == "AchievementImage"
                && effective["atlasId"] == "00000000-0000-0000-0000-000000000000";
            if default_general_atlas {
                effective["atlasId"] = Value::Null;
            }
            let standalone =
                kind != "AtlasTile" && !default_general_atlas && effective["atlasId"].is_null();
            let asset = if standalone {
                project["assets"]["images"]
                    .as_array()
                    .and_then(|a| a.iter().find(|a| a["id"] == effective["imageId"]))
                    .map(|a| &a["image"])
            } else {
                project["assets"]["atlases"]
                    .as_array()
                    .and_then(|a| a.iter().find(|a| a["id"] == effective["atlasId"]))
                    .map(|a| &a["image"])
                    .or_else(|| {
                        effective["atlasId"]
                            .is_null()
                            .then_some(&project["assets"]["atlas"])
                    })
            };
            // An unavailable image contributes transparent pixels. Other layers,
            // folders and masks still compose; structural/budget errors still fail.
            let Some(asset) = asset else {
                return if allow_missing_art {
                    Ok(image)
                } else {
                    Err("Layer image asset is missing".into())
                };
            };
            let mut source = match assets.image(asset) {
                Ok(source) => source,
                Err(error) => {
                    if cancelled() {
                        return Err("Icon rendering cancelled".into());
                    }
                    return if allow_missing_art {
                        Ok(image)
                    } else {
                        Err(error)
                    };
                }
            };
            if !standalone {
                let settings = if project["styleViewerVersion"] == 1 {
                    project["assets"]["atlases"]
                        .as_array()
                        .and_then(|atlases| {
                            atlases
                                .iter()
                                .find(|atlas| atlas["id"] == effective["atlasId"])
                        })
                        .and_then(|atlas| atlas.get("styleViewerAtlasGrid"))
                        .unwrap_or(&project["atlas"])
                } else {
                    &project["atlas"]
                };
                let tw = settings["tileWidth"].as_u64().unwrap_or(512) as u32;
                let th = settings["tileHeight"].as_u64().unwrap_or(512) as u32;
                let left = settings["leftOffset"].as_u64().unwrap_or(0) as u32;
                let top = settings["topOffset"].as_u64().unwrap_or(0) as u32;
                let dx = tw
                    .checked_add(
                        settings["horizontalSpacing"]
                            .as_u64()
                            .unwrap_or(0)
                            .try_into()
                            .map_err(|_| "Atlas spacing is too large")?,
                    )
                    .ok_or("Atlas grid overflow")?;
                let dy = th
                    .checked_add(
                        settings["verticalSpacing"]
                            .as_u64()
                            .unwrap_or(0)
                            .try_into()
                            .map_err(|_| "Atlas spacing is too large")?,
                    )
                    .ok_or("Atlas grid overflow")?;
                if tw == 0 || th == 0 || dx == 0 || dy == 0 {
                    return Err("Invalid atlas grid".into());
                }
                let columns = source
                    .width()
                    .saturating_sub(left)
                    .saturating_sub(tw)
                    .checked_div(dx)
                    .unwrap_or(0)
                    + u32::from(source.width().saturating_sub(left) >= tw);
                let rows = source
                    .height()
                    .saturating_sub(top)
                    .saturating_sub(th)
                    .checked_div(dy)
                    .unwrap_or(0)
                    + u32::from(source.height().saturating_sub(top) >= th);
                let tile = effective["tileIndex"].as_u64().unwrap_or(0) as u32;
                if columns == 0 || rows == 0 || tile >= columns * rows {
                    if allow_missing_art {
                        return Ok(image);
                    }
                    return Err("Atlas tile is outside the image".into());
                }
                let (x, y) = if settings["traversal"] == "ColumnMajor" {
                    (tile / rows, tile % rows)
                } else {
                    (tile % columns, tile / columns)
                };
                source = Arc::new(
                    image::imageops::crop_imm(source.as_ref(), left + x * dx, top + y * dy, tw, th)
                        .to_image(),
                );
            }
            image = image::imageops::resize(
                source.as_ref(),
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            );
        }
        "Curves" | "Contrast" | "HueSaturation" | "SelectiveColor" | "UnsharpMask"
        | "RadialBlur" => {
            return Err("Adjustment layers require the composited background".into());
        }
        _ => return Err(format!("Unsupported icon layer: {kind}")),
    }
    Ok(transform(image, layer, width, height))
}
fn transform(mut image: RgbaImage, layer: &Value, width: u32, height: u32) -> RgbaImage {
    let scale = num(layer, "scale", 1.0).max(0.001);
    let angle = num(layer, "rotation", 0.0).to_radians();
    let ox = num(layer, "offsetX", 0.0) * f64::from(width) / 512.0;
    let oy = num(layer, "offsetY", 0.0) * f64::from(height) / 512.0;
    if scale != 1.0 || angle != 0.0 || ox != 0.0 || oy != 0.0 {
        let mut result = RgbaImage::new(width, height);
        // Pixel centers run from 0 through dimension - 1. Using dimension / 2
        // shifts a 180-degree rotation by a pixel and clips opposite edges.
        let (cx, cy) = (f64::from(width - 1) / 2.0, f64::from(height - 1) / 2.0);
        let (sin, cos) = angle.sin_cos();
        for (x, y, p) in result.enumerate_pixels_mut() {
            let dx = (f64::from(x) - cx - ox) / scale;
            let dy = (f64::from(y) - cy - oy) / scale;
            *p = bilinear(&image, dx * cos + dy * sin + cx, -dx * sin + dy * cos + cy);
        }
        image = result;
    }
    image
}
pub(crate) fn bilinear(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (left, top) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let mut alpha = 0.0;
    let mut rgb = [0.0; 3];
    for dy in 0..2 {
        for dx in 0..2 {
            let (sx, sy) = (left + dx, top + dy);
            if sx < 0 || sy < 0 || sx >= i64::from(image.width()) || sy >= i64::from(image.height())
            {
                continue;
            }
            let pixel = image.get_pixel(sx as u32, sy as u32);
            let weight = if dx == 0 { 1.0 - fx } else { fx } * if dy == 0 { 1.0 - fy } else { fy };
            let coverage = f64::from(pixel[3]) * weight;
            alpha += coverage;
            for i in 0..3 {
                rgb[i] += f64::from(pixel[i]) * coverage;
            }
        }
    }
    if alpha <= 0.0 {
        return Rgba([0; 4]);
    }
    Rgba([
        (rgb[0] / alpha).round() as u8,
        (rgb[1] / alpha).round() as u8,
        (rgb[2] / alpha).round() as u8,
        alpha.round() as u8,
    ])
}
pub(crate) fn sample_gradient(stops: &[(f64, Rgba<u8>)], t: f64) -> Rgba<u8> {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        if t <= pair[1].0 {
            let p = ((t - pair[0].0) / (pair[1].0 - pair[0].0).max(f64::EPSILON)).clamp(0.0, 1.0);
            return Rgba(std::array::from_fn(|i| {
                (f64::from(pair[0].1[i]) + (f64::from(pair[1].1[i]) - f64::from(pair[0].1[i])) * p)
                    .round() as u8
            }));
        }
    }
    stops.last().map(|s| s.1).unwrap_or(Rgba([0; 4]))
}
pub(crate) fn composite(target: &mut RgbaImage, source: &RgbaImage, opacity: f64, mode: &str) {
    for (dst, src) in target.pixels_mut().zip(source.pixels()) {
        let a = f64::from(src[3]) / 255.0 * opacity;
        let b = f64::from(dst[3]) / 255.0;
        let out = a + b * (1.0 - a);
        if out <= 0.0 {
            continue;
        }
        for i in 0..3 {
            let s = f64::from(src[i]) / 255.0;
            let d = f64::from(dst[i]) / 255.0;
            let blend = blend(s, d, mode);
            dst[i] = (((s * (1.0 - b) + blend * b) * a + d * b * (1.0 - a)) / out * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        dst[3] = (out * 255.0).round() as u8;
    }
}
pub(crate) fn check_blend_mode(mode: &str) -> Result<(), String> {
    if matches!(
        mode,
        "Normal" | "Multiply" | "Screen" | "Overlay" | "Darken" | "Lighten" | "Add" | "Difference"
    ) {
        Ok(())
    } else {
        Err(format!("Unsupported blend mode: {mode}"))
    }
}
pub(crate) fn blend(s: f64, d: f64, mode: &str) -> f64 {
    match mode {
        "Multiply" => s * d,
        "Screen" => 1.0 - (1.0 - s) * (1.0 - d),
        "Overlay" => {
            if d < 0.5 {
                2.0 * s * d
            } else {
                1.0 - 2.0 * (1.0 - s) * (1.0 - d)
            }
        }
        "Darken" => s.min(d),
        "Lighten" => s.max(d),
        "Add" => (s + d).min(1.0),
        "Difference" => (s - d).abs(),
        _ => s,
    }
}

fn radial_adjustment(
    image: &mut RgbaImage,
    layer: &Value,
    work: &mut Work<'_>,
) -> Result<(), String> {
    let radial = num(&layer["adjustment"], "radialBlur", 0.0).clamp(0.0, 100.0) / 100.0;
    let angular = num(&layer["adjustment"], "angularBlur", 0.0).clamp(0.0, 100.0) / 100.0;
    let opacity = num(layer, "opacity", 1.0).clamp(0.0, 1.0);
    if opacity == 0.0 || (radial == 0.0 && angular == 0.0) {
        return Ok(());
    }
    for _ in 0..(u32::from(radial > 0.0) + u32::from(angular > 0.0)) * 17 * 4 {
        work.plane(image.width(), image.height())?;
    }
    let blurred = crate::radial_blur::apply(image, radial, angular, work.cancelled)?;
    let mode = layer["blendMode"].as_str().unwrap_or("Normal");
    for (pixel, changed) in image.pixels_mut().zip(blurred.pixels()) {
        *pixel = crate::radial_blur::mix(*pixel, *changed, opacity, mode);
    }
    Ok(())
}

fn adjust(image: &mut RgbaImage, layer: &Value) {
    let kind = layer["kind"].as_str().unwrap_or("");
    let settings = &layer["adjustment"];
    if kind == "UnsharpMask" {
        let blurred = image::imageops::blur(
            image,
            (num(settings, "unsharpRadius", 1.0) * f64::from(image.height()) / 512.0)
                .clamp(0.1, 32.0) as f32,
        );
        let amount = num(settings, "unsharpAmount", 100.0) / 100.0;
        let threshold = num(settings, "unsharpThreshold", 0.0);
        for (pixel, blur) in image.pixels_mut().zip(blurred.pixels()) {
            for i in 0..3 {
                let delta = f64::from(pixel[i]) - f64::from(blur[i]);
                if delta.abs() >= threshold {
                    let original = f64::from(pixel[i]);
                    let changed = ((original + delta * amount) / 255.0).clamp(0.0, 1.0);
                    let changed = blend(
                        changed,
                        original / 255.0,
                        layer["blendMode"].as_str().unwrap_or("Normal"),
                    );
                    let opacity = num(layer, "opacity", 1.0).clamp(0.0, 1.0);
                    pixel[i] = (original * (1.0 - opacity) + changed * 255.0 * opacity)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
        return;
    }
    let mut points: Vec<(f64, f64)> = layer["curves"]["points"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| (num(p, "input", 0.0), num(p, "output", 0.0)))
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| a.0 == b.0);
    let lookup: [f64; 256] = std::array::from_fn(|i| {
        if points.is_empty() {
            i as f64 / 255.0
        } else {
            curve(&points, i as f64 / 255.0)
        }
    });
    let mut gradient: Vec<_> = layer["gradient"]["stops"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((num(s, "offset", 0.0), rgba(s["colorHex"].as_str()?).ok()?)))
        .collect();
    gradient.sort_by(|a, b| a.0.total_cmp(&b.0));
    let fallback = [settings.clone()];
    let corrections = settings["selectiveColors"]
        .as_array()
        .map_or(fallback.as_slice(), Vec::as_slice);
    for pixel in image.pixels_mut() {
        let original = *pixel;
        let r = f64::from(pixel[0]) / 255.0;
        let g = f64::from(pixel[1]) / 255.0;
        let b = f64::from(pixel[2]) / 255.0;
        let (mut hue, mut saturation, mut lightness) = hsl(r, g, b);
        hue = (hue + num(settings, "hue", 0.0) / 360.0).rem_euclid(1.0);
        saturation = (saturation + num(settings, "saturation", 0.0) / 100.0).clamp(0.0, 1.0);
        lightness = (lightness + num(settings, "lightness", 0.0) / 100.0).clamp(0.0, 1.0);
        let shifted = rgb(hue, saturation, lightness);
        for i in 0..3 {
            let mut value = f64::from(pixel[i]) / 255.0;
            match kind {
                "Curves" => {
                    if !points.is_empty() {
                        value = lookup[pixel[i] as usize];
                    }
                }
                "Contrast" => {
                    let contrast = num(settings, "contrast", 0.0) / 100.0;
                    let factor = if contrast >= 0.0 {
                        1.0 / (1.0 - contrast).max(0.01)
                    } else {
                        1.0 + contrast
                    };
                    value = (value - 0.5) * factor + 0.5 + num(settings, "brightness", 0.0) / 100.0;
                }
                "HueSaturation" => {
                    value = shifted[i];
                }
                "SelectiveColor" => {
                    let (hue, saturation, lightness) = hsl(r, g, b);
                    let min = r.min(g).min(b);
                    let max = r.max(g).max(b);
                    for correction in corrections {
                        let weight = match correction["range"]
                            .as_str()
                            .or(correction["colorRange"].as_str())
                            .unwrap_or("Reds")
                        {
                            "Whites" => ((min - 0.5) * 2.0).clamp(0.0, 1.0),
                            "Blacks" => ((0.5 - max) * 2.0).clamp(0.0, 1.0),
                            "Neutrals" => {
                                (1.0 - saturation) * (1.0 - (2.0 * lightness - 1.0).abs())
                            }
                            name => {
                                let center = match name {
                                    "Yellows" => 60.0,
                                    "Greens" => 120.0,
                                    "Cyans" => 180.0,
                                    "Blues" => 240.0,
                                    "Magentas" => 300.0,
                                    _ => 0.0,
                                };
                                let distance = (hue * 360.0 - center).abs();
                                let distance = distance.min(360.0 - distance);
                                (1.0 - distance / 60.0).clamp(0.0, 1.0) * saturation
                            }
                        };
                        let channel = match i {
                            0 => "cyan",
                            1 => "magenta",
                            _ => "yellow",
                        };
                        value -= weight
                            * (num(correction, channel, 0.0) / 100.0 * f64::from(original[i])
                                / 255.0
                                + num(correction, "black", 0.0) / 100.0);
                    }
                }
                "GradientOverlay" if !gradient.is_empty() => {
                    let channel = layer["gradient"]["inputChannel"].as_str().unwrap_or("RGB");
                    let input = match channel {
                        "Red" => r,
                        "Green" => g,
                        "Blue" => b,
                        _ => 0.2126 * r + 0.7152 * g + 0.0722 * b,
                    };
                    value = f64::from(sample_gradient(&gradient, input)[i]) / 255.0;
                }
                _ => {}
            }
            let opacity = num(layer, "opacity", 1.0).clamp(0.0, 1.0);
            value = blend(
                value.clamp(0.0, 1.0),
                f64::from(original[i]) / 255.0,
                layer["blendMode"].as_str().unwrap_or("Normal"),
            );
            pixel[i] = (f64::from(original[i]) * (1.0 - opacity)
                + value.clamp(0.0, 1.0) * 255.0 * opacity)
                .round() as u8;
        }
    }
}
fn hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let light = (max + min) / 2.0;
    if delta == 0.0 {
        return (0.0, 0.0, light);
    }
    let hue = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    (hue / 6.0, delta / (1.0 - (2.0 * light - 1.0).abs()), light)
}
fn rgb(h: f64, s: f64, l: f64) -> [f64; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let base = match (h * 6.0) as u8 {
        0 => [c, x, 0.0],
        1 => [x, c, 0.0],
        2 => [0.0, c, x],
        3 => [0.0, x, c],
        4 => [x, 0.0, c],
        _ => [c, 0.0, x],
    };
    base.map(|v| v + m)
}
fn curve(points: &[(f64, f64)], input: f64) -> f64 {
    if input <= points[0].0 {
        return points[0].1.clamp(0.0, 1.0);
    }
    for i in 1..points.len() {
        if input <= points[i].0 {
            let p0 = points[i.saturating_sub(2)];
            let p1 = points[i - 1];
            let p2 = points[i];
            let p3 = points[(i + 1).min(points.len() - 1)];
            let span = p2.0 - p1.0;
            let t = (input - p1.0) / span;
            let m1 = (p2.1 - p0.1) / (p2.0 - p0.0) * span;
            let m2 = (p3.1 - p1.1) / (p3.0 - p1.0) * span;
            return ((2.0 * t * t * t - 3.0 * t * t + 1.0) * p1.1
                + (t * t * t - 2.0 * t * t + t) * m1
                + (-2.0 * t * t * t + 3.0 * t * t) * p2.1
                + (t * t * t - t * t) * m2)
                .clamp(0.0, 1.0);
        }
    }
    points.last().map(|p| p.1).unwrap_or(input).clamp(0.0, 1.0)
}

pub(crate) fn gradient_image(
    gradient: &Value,
    width: u32,
    height: u32,
    cancelled: &dyn Fn() -> bool,
) -> Result<RgbaImage, String> {
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    let mut image = RgbaImage::new(width, height);
    let angle = num(gradient, "angle", 90.0).to_radians();
    let colors = gradient_stops(gradient)?;
    for (y, mut row) in image.enumerate_rows_mut() {
        if cancelled() {
            return Err("Icon rendering cancelled".into());
        }
        for (x, _, p) in &mut row {
            let nx = f64::from(x) / f64::from(width) - 0.5;
            let ny = f64::from(y) / f64::from(height) - 0.5;
            let t = if gradient["kind"] == "Radial" {
                (nx * nx + ny * ny).sqrt() * 2.0
            } else {
                nx * angle.cos() + ny * angle.sin() + 0.5
            };
            *p = sample_gradient(&colors, t.clamp(0.0, 1.0));
        }
    }
    Ok(image)
}

pub(crate) fn gradient_stops(gradient: &Value) -> Result<Vec<(f64, Rgba<u8>)>, String> {
    let mut stops = gradient["stops"]
        .as_array()
        .ok_or("Gradient has no stops")?
        .iter()
        .map(|s| {
            Ok((
                num(s, "offset", 0.0),
                rgba(s["colorHex"].as_str().unwrap_or("#FFFFFFFF"))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if stops.is_empty() {
        return Err("Gradient has no stops".into());
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(stops)
}

pub(crate) fn gradient_map(
    image: &mut RgbaImage,
    gradient: &Value,
    opacity: f64,
    mode: &str,
) -> Result<(), String> {
    check_blend_mode(mode)?;
    let lookup = gradient_lookup(gradient)?;
    for pixel in image.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let input = match gradient["inputChannel"].as_str().unwrap_or("RGB") {
            "Red" => f64::from(pixel[0]),
            "Green" => f64::from(pixel[1]),
            "Blue" => f64::from(pixel[2]),
            _ => {
                0.2126 * f64::from(pixel[0])
                    + 0.7152 * f64::from(pixel[1])
                    + 0.0722 * f64::from(pixel[2])
            }
        };
        let mapped = lookup[input.round().clamp(0.0, 255.0) as usize];
        let amount = opacity.clamp(0.0, 1.0) * f64::from(mapped[3]) / 255.0;
        for i in 0..3 {
            let base = f64::from(pixel[i]) / 255.0;
            pixel[i] = ((base * (1.0 - amount)
                + blend(f64::from(mapped[i]) / 255.0, base, mode) * amount)
                * 255.0)
                .round() as u8;
        }
    }
    Ok(())
}

pub(crate) fn gradient_lookup(gradient: &Value) -> Result<[Rgba<u8>; 256], String> {
    let stops = gradient_stops(gradient)?;
    Ok(std::array::from_fn(|i| {
        sample_gradient(&stops, i as f64 / 255.0)
    }))
}
