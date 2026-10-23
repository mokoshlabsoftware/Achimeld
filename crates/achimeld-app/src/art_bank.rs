//! One bounded background artwork operation; presentation never reads files.
use image::RgbaImage;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::mpsc;

pub(crate) enum Target {
    Import(bool),
    General(String),
    Layer {
        achievement: String,
        layer: Option<String>,
    },
    Replace {
        atlas: bool,
        id: String,
        expected: serde_json::Value,
        reload: bool,
    },
}
#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub(crate) enum ImportSource {
    File { path: String },
    Clipboard,
}
pub(crate) struct Prepared {
    pub name: String,
    pub bytes: Vec<u8>,
    pub dimensions: [u32; 2],
    pub source: ImportSource,
}
pub(crate) struct Pending {
    pub project: String,
    pub target: Target,
    pub receiver: mpsc::Receiver<Result<Prepared, String>>,
    pub preview_receiver: mpsc::Receiver<achimeld_application::Preview>,
    pub preview: Option<achimeld_application::Preview>,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

type Inventory = std::collections::BTreeMap<String, Option<u64>>;
struct InventoryResult {
    files: Inventory,
    saved_bytes: Option<u64>,
}
#[derive(Default)]
pub(crate) struct Stats {
    generation: Option<u64>,
    pending: Option<(u64, mpsc::Receiver<InventoryResult>)>,
    files: Inventory,
    saved_bytes: Option<u64>,
}
impl Stats {
    pub fn ready(&self, generation: u64) -> bool {
        self.generation == Some(generation)
    }
    pub fn saved_bytes(&self) -> Option<u64> {
        self.saved_bytes
    }
    #[allow(clippy::too_many_arguments)]
    pub fn get(
        &mut self,
        generation: u64,
        assets: &serde_json::Value,
        project: Option<&std::path::Path>,
        saved_project: Option<&std::path::Path>,
        reference: &serde_json::Value,
        ctx: &eframe::egui::Context,
    ) -> Option<u64> {
        if let Some((version, receiver)) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.files = result.files;
                    self.saved_bytes = result.saved_bytes;
                    self.generation = Some(*version);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.generation != Some(generation) && self.pending.is_none() {
            let assets = assets.clone();
            let project = project
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("Untitled.achv"));
            let ctx = ctx.clone();
            let saved_directory = saved_project.and_then(|p| {
                Some(
                    p.parent()?
                        .join(format!("{}.art-data-bank", p.file_stem()?.to_str()?)),
                )
            });
            let (tx, receiver) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let mut files = Inventory::new();
                for reference in std::iter::once(&assets["atlas"]).chain(
                    ["images", "atlases"].into_iter().flat_map(|key| {
                        assets[key]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|entry| &entry["image"])
                    }),
                ) {
                    let key = reference.to_string();
                    files.entry(key).or_insert_with(|| {
                        achimeld_persistence::project::resolve_reference(reference, &project)
                            .ok()
                            .and_then(|path| std::fs::metadata(path).ok())
                            .filter(|m| m.is_file())
                            .map(|m| m.len())
                    });
                }
                let saved_bytes = saved_directory.and_then(|directory| {
                    if !directory.exists() {
                        return Some(0);
                    }
                    let mut total = 0_u64;
                    for entry in std::fs::read_dir(directory).ok()? {
                        let entry = entry.ok()?;
                        if entry.file_type().ok()?.is_file() {
                            total = total.checked_add(entry.metadata().ok()?.len())?;
                        }
                    }
                    Some(total)
                });
                let _ = tx.send(InventoryResult { files, saved_bytes });
                ctx.request_repaint();
            });
            self.pending = Some((generation, receiver));
        }
        self.ready(generation)
            .then(|| self.files.get(&reference.to_string()).copied().flatten())
            .flatten()
    }
}

#[cfg(test)]
pub(crate) fn prepare(path: PathBuf, size: Option<[u32; 2]>) -> Result<Prepared, String> {
    prepare_progressive(path, size, |_| {}, &|| false)
}
pub(crate) fn prepare_progressive(
    path: PathBuf,
    size: Option<[u32; 2]>,
    quick: impl FnOnce(achimeld_application::Preview),
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, String> {
    if cancelled() {
        return Err("Artwork processing cancelled".into());
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Image")
        .to_owned();
    let mut image = decode(&path)?;
    if cancelled() {
        return Err("Artwork processing cancelled".into());
    }
    let ratio = (64.0 / image.width().max(image.height()).max(1) as f64).min(1.0);
    let preview = image::imageops::resize(
        &image,
        (image.width() as f64 * ratio).round().max(1.0) as u32,
        (image.height() as f64 * ratio).round().max(1.0) as u32,
        image::imageops::FilterType::Triangle,
    );
    quick(achimeld_application::Preview {
        width: preview.width() as usize,
        height: preview.height() as usize,
        source_size: [image.width() as usize, image.height() as usize],
        rgba: preview.into_raw().into(),
        fingerprint: achimeld_application::new_id(),
    });
    if cancelled() {
        return Err("Artwork processing cancelled".into());
    }
    if let Some([width, height]) = size {
        if width == 0
            || height == 0
            || width > 16384
            || height > 16384
            || u64::from(width) * u64::from(height) * 4 > 128 * 1024 * 1024
        {
            return Err("Resolution exceeds the artwork memory limit (128 MiB)".into());
        }
        image =
            image::imageops::resize(&image, width, height, image::imageops::FilterType::Lanczos3);
    }
    if cancelled() {
        return Err("Artwork processing cancelled".into());
    }
    let source = ImportSource::File {
        path: std::path::absolute(path)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Invalid artwork source path")?
            .to_owned(),
    };
    encode(name, image, source)
}
pub(crate) fn encode(
    name: String,
    image: RgbaImage,
    source: ImportSource,
) -> Result<Prepared, String> {
    let dimensions = [image.width(), image.height()];
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let bytes = output.into_inner();
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Encoded artwork exceeds 64 MiB".into());
    }
    Ok(Prepared {
        name,
        bytes,
        dimensions,
        source,
    })
}

pub(crate) fn decode(path: &std::path::Path) -> Result<RgbaImage, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Image file exceeds 64 MiB".into());
    }
    let (width, height) = image::ImageReader::new(Cursor::new(bytes.as_slice()))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if u64::from(width) * u64::from(height) * 4 > 128 * 1024 * 1024 {
        return Err("Decoded image exceeds 128 MiB".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    use image::ImageDecoder as _;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    Ok(image.to_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preliminary_pixels_precede_final_encoding_and_obsolete_work_cancels() {
        let path = crate::test_output::root().join(format!(
            "art-preview-{}.png",
            achimeld_application::new_id()
        ));
        RgbaImage::from_pixel(256, 128, image::Rgba([240, 80, 20, 255]))
            .save(&path)
            .unwrap();
        let started = std::time::Instant::now();
        // The callback fires before the high-quality resize and PNG encoding.
        let first = std::cell::Cell::new(None);
        let art = prepare_progressive(
            path.clone(),
            Some([32, 16]),
            |preview| {
                assert_eq!([preview.width, preview.height], [64, 32]);
                first.set(Some(started.elapsed()));
            },
            &|| false,
        )
        .unwrap();
        let first_pixels = first.get().expect("Initial preview must publish");
        let finished = started.elapsed();
        assert_eq!(art.dimensions, [32, 16]);
        assert!(first_pixels <= finished);
        eprintln!(
            "Artwork preview pixels ready: {first_pixels:?}; final artwork ready: {finished:?}"
        );
        let cancelled = std::cell::Cell::new(false);
        assert!(
            prepare_progressive(
                path.clone(),
                Some([32, 16]),
                |_| cancelled.set(true),
                &|| cancelled.get()
            )
            .is_err()
        );
        std::fs::remove_file(path).unwrap();
    }
}
