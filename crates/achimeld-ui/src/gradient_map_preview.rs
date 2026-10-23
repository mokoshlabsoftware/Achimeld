//! One bounded refinement worker for color-map thumbnails; every edit paints a quick result first.
use achimeld_application::gradient_map::{ColorField, GradientMap};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};

type Pixels = Vec<[u8; 4]>;

struct Job {
    field: ColorField,
    cancelled: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Pixels>>>,
    context: egui::Context,
}

#[derive(Clone)]
struct Cache {
    map: GradientMap,
    texture: egui::TextureHandle,
    cancelled: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Pixels>>>,
    queued: bool,
    finished: bool,
}

const QUICK: [usize; 2] = [64, 32];
const FINAL: [usize; 2] = [256, 128];

fn worker() -> Option<&'static SyncSender<Job>> {
    static WORKER: OnceLock<Option<SyncSender<Job>>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel::<Job>(1);
            std::thread::Builder::new()
                .name("color-map-preview".into())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        if let Some(pixels) = job.field.raster(FINAL[0], FINAL[1], &|| {
                            job.cancelled.load(Ordering::Relaxed)
                        }) && !job.cancelled.load(Ordering::Relaxed)
                            && let Ok(mut result) = job.result.lock()
                        {
                            *result = Some(pixels);
                            job.context.request_repaint();
                        }
                    }
                })
                .ok()?;
            Some(sender)
        })
        .as_ref()
}

fn image(size: [usize; 2], pixels: &[[u8; 4]]) -> egui::ColorImage {
    egui::ColorImage::new(
        size,
        pixels
            .iter()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
    )
}

pub(super) fn texture(ui: &egui::Ui, map: &GradientMap, field: &ColorField) -> egui::TextureHandle {
    // A single cache per application context bounds memory even when visiting many layers.
    let id = egui::Id::new("color-map-preview-cache");
    let options = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
    let mut cache = ui.data_mut(|data| data.get_temp::<Cache>(id));
    if cache.as_ref().is_none_or(|c| &c.map != map) {
        if let Some(previous) = &cache {
            previous.cancelled.store(true, Ordering::Relaxed);
        }
        let quick = image(
            QUICK,
            &field
                .raster(QUICK[0], QUICK[1], &|| false)
                .unwrap_or_default(),
        );
        let texture = if let Some(previous) = &mut cache {
            previous.texture.set(quick, options);
            previous.texture.clone()
        } else {
            ui.ctx().load_texture("2D gradient", quick, options)
        };
        cache = Some(Cache {
            map: map.clone(),
            texture,
            cancelled: Arc::new(AtomicBool::new(false)),
            result: Arc::new(Mutex::new(None)),
            queued: false,
            finished: false,
        });
    }
    let mut cache = cache.expect("A valid map always has a quick preview");
    if let Ok(mut result) = cache.result.lock()
        && let Some(pixels) = result.take()
    {
        cache.texture.set(image(FINAL, &pixels), options);
        cache.finished = true;
    }
    if !cache.queued
        && !cache.finished
        && let Some(worker) = worker()
        && let Ok(field) = map.compile()
    {
        cache.queued = worker
            .try_send(Job {
                field,
                cancelled: cache.cancelled.clone(),
                result: cache.result.clone(),
                context: ui.ctx().clone(),
            })
            .is_ok();
        if !cache.queued {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
    let texture = cache.texture.clone();
    ui.data_mut(|data| data.insert_temp(id, cache));
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_color_map_preview_refines_and_discards_obsolete_work() {
        let ctx = egui::Context::default();
        let mut map = GradientMap::default();
        let start = std::time::Instant::now();
        let frame = |map: &GradientMap| {
            let field = map.compile().unwrap();
            let mut size = [0, 0];
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                size = texture(ui, map, &field).size();
            });
            output.textures_delta.clear();
            size
        };
        assert_eq!(frame(&map), QUICK);
        let first = start.elapsed();
        map.points[0].color_hex = "#FF40A0FF".into();
        assert_eq!(frame(&map), QUICK);
        while frame(&map) != FINAL {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(10),
                "Gradient refinement failed to finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let cache = ctx
            .data_mut(|data| data.get_temp::<Cache>(egui::Id::new("color-map-preview-cache")))
            .unwrap();
        assert_eq!(cache.map, map);
        assert!(cache.finished);
        eprintln!(
            "2D gradient preview first visible: {:.2} ms; final after replacement: {:.2} ms",
            first.as_secs_f64() * 1000.0,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
