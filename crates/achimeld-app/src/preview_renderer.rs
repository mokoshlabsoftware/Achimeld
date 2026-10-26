use crate::desktop::Files;
use achimeld_application::preview::PreviewState;
use achimeld_application::{Document, Preview, ports::LayerBlendPreview};
use achimeld_imaging::{AssetSource, render_achieved_cancellable, render_layer_cancellable};
use eframe::egui;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};

const MAX_PENDING: usize = 512;
const CACHE_BYTES: usize = 128 * 1024 * 1024;

#[cfg(test)]
#[path = "performance_benchmark.rs"]
mod performance_benchmark;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    Quick,
    Detail,
    Single,
}
struct Job {
    id: String,
    pass: Pass,
    cancelled: Option<Arc<AtomicBool>>,
}
struct Batch {
    generation: u64,
    document: Arc<Document>,
    assets: Arc<Mutex<Files>>,
    quick: VecDeque<Job>,
    detail: VecDeque<Job>,
    active_quick: usize,
    stop: bool,
    cancelled: Arc<AtomicBool>,
}
impl Batch {
    fn take(&mut self) -> Option<Job> {
        if let Some(job) = self.quick.pop_front() {
            self.active_quick += 1;
            Some(job)
        } else if self.active_quick == 0 {
            self.detail.pop_front()
        } else {
            None
        }
    }
}
// Only asset loading holds this lock. Composition proceeds independently on
// each worker, sharing immutable decoded pixels and avoiding duplicate decodes.
struct SharedAssets(Arc<Mutex<Files>>);
impl AssetSource for SharedAssets {
    fn image(&mut self, reference: &serde_json::Value) -> Result<Arc<image::RgbaImage>, String> {
        self.0
            .lock()
            .map_err(|_| "Image cache unavailable".to_owned())?
            .image(reference)
    }
    fn font(&mut self, family: &str) -> Option<Arc<achimeld_imaging::Font>> {
        self.0.lock().ok()?.font(family)
    }
}
struct Completed {
    id: String,
    pass: Pass,
    generation: u64,
    result: Result<Preview, String>,
    cancelled: Option<Arc<AtomicBool>>,
}
pub struct PreviewWorker {
    queue: Arc<(Mutex<Batch>, Condvar)>,
    receiver: mpsc::Receiver<Completed>,
    states: BTreeMap<String, PreviewState<Preview>>,
    threads: Vec<std::thread::JoinHandle<()>>,
    usage: BTreeMap<String, u64>,
    clock: u64,
    generation: u64,
    source_generation: Option<u64>,
    identity: Option<(String, Option<PathBuf>)>,
    blend_preview: Option<LayerBlendPreview>,
    blend_cancelled: Arc<AtomicBool>,
}
impl PreviewWorker {
    pub fn new(ctx: egui::Context) -> Self {
        // Leave CPU capacity for input/painting and cap concurrent render memory.
        let count = std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1).clamp(1, 4));
        Self::with_threads(ctx, count)
    }
    pub(crate) fn with_threads(ctx: egui::Context, count: usize) -> Self {
        let queue = Arc::new((
            Mutex::new(Batch {
                generation: 0,
                document: Arc::new(Document::new()),
                assets: Arc::new(Mutex::new(Files::new())),
                quick: VecDeque::new(),
                detail: VecDeque::new(),
                active_quick: 0,
                stop: false,
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
            Condvar::new(),
        ));
        // Full-resolution images may be large; never buffer hundreds of them.
        let (tx, receiver) = mpsc::sync_channel(count);
        let mut threads = Vec::new();
        for _ in 0..count {
            let shared = queue.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            threads.push(std::thread::spawn(move || {
                loop {
                    let (generation, document, assets, job, cancelled) = {
                        let (lock, wake) = &*shared;
                        let Ok(mut batch) = lock.lock() else {
                            return;
                        };
                        let job = loop {
                            if batch.stop {
                                return;
                            }
                            if let Some(job) = batch.take() {
                                break job;
                            }
                            let Ok(next) = wake.wait(batch) else {
                                return;
                            };
                            batch = next;
                        };
                        (
                            batch.generation,
                            batch.document.clone(),
                            batch.assets.clone(),
                            job,
                            batch.cancelled.clone(),
                        )
                    };
                    let is_cancelled = || {
                        cancelled.load(Ordering::Acquire)
                            || job
                                .cancelled
                                .as_ref()
                                .is_some_and(|c| c.load(Ordering::Acquire))
                    };
                    let operation =
                        achimeld_application::diagnostics::Operation::begin(match job.pass {
                            Pass::Quick => "quick preview render",
                            Pass::Detail => "detail preview render",
                            Pass::Single => "single preview render",
                        });
                    let result = render_job(
                        &document,
                        &job,
                        generation,
                        &mut SharedAssets(assets),
                        &is_cancelled,
                    );
                    operation.finish(if is_cancelled() {
                        "cancelled"
                    } else if result.is_ok() {
                        "completed"
                    } else {
                        "failed"
                    });
                    // Publish before releasing the quick-pass barrier. Refinement is
                    // admitted only by poll(), so an initial result reaches the UI first.
                    let mut completed = Completed {
                        id: job.id,
                        pass: job.pass,
                        generation,
                        result,
                        cancelled: job.cancelled.clone(),
                    };
                    loop {
                        if is_cancelled() {
                            break;
                        }
                        match tx.try_send(completed) {
                            Ok(()) => {
                                ctx.request_repaint();
                                break;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => return,
                            Err(mpsc::TrySendError::Full(value)) => {
                                completed = value;
                                std::thread::sleep(std::time::Duration::from_millis(2));
                            }
                        }
                    }
                    let (lock, wake) = &*shared;
                    let Ok(mut batch) = lock.lock() else {
                        return;
                    };
                    if batch.generation == generation && job.pass != Pass::Detail {
                        batch.active_quick -= 1;
                    }
                    wake.notify_all();
                }
            }));
        }
        Self {
            queue,
            receiver,
            states: BTreeMap::new(),
            threads,
            usage: BTreeMap::new(),
            clock: 0,
            generation: 0,
            source_generation: None,
            identity: None,
            blend_preview: None,
            blend_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn poll(&mut self) {
        let mut refine = Vec::new();
        while let Ok(completed) = self.receiver.try_recv() {
            if completed
                .cancelled
                .as_ref()
                .is_some_and(|c| c.load(Ordering::Acquire))
            {
                continue;
            }
            // An evicted entry can be requested again while its previous detail
            // job is still in flight. Never replace current detail with quick pixels.
            if completed.pass == Pass::Quick
                && self.states.get(&completed.id).is_some_and(|state| {
                    state.generation() == Some(completed.generation)
                        && state
                            .output()
                            .is_some_and(|p| p.fingerprint.ends_with("Detail"))
                })
            {
                continue;
            }
            if let Some(state) = self.states.get_mut(&completed.id)
                && state.complete(completed.generation, completed.result)
                && completed.pass == Pass::Quick
            {
                refine.push((completed.generation, completed.id, completed.cancelled));
            }
        }
        let (lock, wake) = &*self.queue;
        if let Ok(mut batch) = lock.lock() {
            for (generation, id, cancelled) in refine {
                if batch.generation == generation && batch.detail.len() < MAX_PENDING {
                    batch.detail.push_back(Job {
                        id,
                        pass: Pass::Detail,
                        cancelled,
                    });
                }
            }
            wake.notify_all();
        }
        while self
            .states
            .values()
            .filter_map(PreviewState::output)
            .map(|p| p.rgba.len())
            .sum::<usize>()
            > CACHE_BYTES
        {
            let Some(old) = self
                .usage
                .iter()
                .filter(|(id, _)| self.states.get(*id).is_some_and(|s| s.output().is_some()))
                .min_by_key(|(_, used)| *used)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            self.states.remove(&old);
            self.usage.remove(&old);
        }
    }
    pub fn error(&self, id: &str) -> Option<&str> {
        self.states.get(id).and_then(PreviewState::error)
    }
    pub fn set_blend_preview(&mut self, preview: Option<LayerBlendPreview>) {
        if self.blend_preview == preview {
            return;
        }
        self.blend_cancelled.store(true, Ordering::Release);
        self.blend_cancelled = Arc::new(AtomicBool::new(false));
        self.blend_preview = preview;
        self.states.retain(|id, _| !id.starts_with("blend:"));
        self.usage.retain(|id, _| !id.starts_with("blend:"));
        let (lock, wake) = &*self.queue;
        if let Ok(mut batch) = lock.lock() {
            batch.quick.retain(|job| !job.id.starts_with("blend:"));
            batch.detail.retain(|job| !job.id.starts_with("blend:"));
            wake.notify_all();
        }
    }
    pub fn achievement_preview(
        &mut self,
        id: &str,
        edge: Option<usize>,
        generation: u64,
        document: &Document,
        project: Option<&std::path::Path>,
    ) -> Option<Preview> {
        let key = if let Some(hover) = &self.blend_preview
            && hover.achievement_id == id
        {
            let edge = edge.unwrap_or(512);
            format!("blend:{edge}:{}:{}:{id}", hover.mode, hover.layer_id)
        } else if let Some(edge) = edge {
            format!("canvas:{edge}:{id}")
        } else {
            id.to_owned()
        };
        self.preview(&key, generation, document, project)
    }
    pub fn preview(
        &mut self,
        id: &str,
        generation: u64,
        document: &Document,
        project: Option<&std::path::Path>,
    ) -> Option<Preview> {
        let identity = (
            document.project()["projectId"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
            project.map(std::path::Path::to_path_buf),
        );
        let changed = self.identity.as_ref() != Some(&identity);
        if changed || self.source_generation != Some(generation) {
            if changed {
                self.states.clear();
                self.usage.clear();
            }
            self.generation += 1;
            self.source_generation = Some(generation);
            self.identity = Some(identity);
            let (lock, wake) = &*self.queue;
            if let Ok(mut batch) = lock.lock() {
                batch.cancelled.store(true, Ordering::Release);
                batch.cancelled = Arc::new(AtomicBool::new(false));
                batch.generation = self.generation;
                batch.document = Arc::new(document.clone());
                batch.quick.clear();
                batch.detail.clear();
                batch.active_quick = 0;
                if changed {
                    let mut files = Files::new();
                    files.project = project.map(std::path::Path::to_path_buf);
                    batch.assets = Arc::new(Mutex::new(files));
                }
                wake.notify_all();
            }
        }
        if !self.states.contains_key(id)
            && self.states.len() >= MAX_PENDING
            && let Some(old) = self
                .usage
                .iter()
                .min_by_key(|(_, used)| *used)
                .map(|(id, _)| id.clone())
        {
            self.states.remove(&old);
            self.usage.remove(&old);
        }
        self.clock += 1;
        self.usage.insert(id.into(), self.clock);
        let state = self.states.entry(id.into()).or_default();
        if state.request(self.generation) {
            let (lock, wake) = &*self.queue;
            let mut admitted = false;
            if let Ok(mut batch) = lock.lock()
                && batch.quick.len() < MAX_PENDING
            {
                let pass = if id.starts_with("asset:")
                    || (id.contains('/')
                        && !id.starts_with("viewer-style:")
                        && !id.starts_with("construction:"))
                    || id.starts_with("canvas:64:")
                    || id.starts_with("style:64:")
                    || id.starts_with("blend:64:")
                {
                    Pass::Single
                } else {
                    Pass::Quick
                };
                let job = Job {
                    id: id.into(),
                    pass,
                    cancelled: id
                        .starts_with("blend:")
                        .then(|| self.blend_cancelled.clone()),
                };
                if id.starts_with("blend:") {
                    batch.quick.push_front(job);
                } else {
                    batch.quick.push_back(job);
                }
                admitted = true;
                wake.notify_all();
            }
            if !admitted {
                state.defer(self.generation);
            }
        }
        state.output().cloned()
    }
}
fn render_job(
    document: &Document,
    job: &Job,
    generation: u64,
    assets: &mut dyn AssetSource,
    cancelled: &dyn Fn() -> bool,
) -> Result<Preview, String> {
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    if let Some(instance) = job.id.strip_prefix("construction:") {
        let settings = &document.project()["achievements"][0]["icon"]["layers"][0]["explosion"];
        let image = achimeld_imaging::render_explosion_construction(
            settings,
            instance,
            if job.pass == Pass::Quick { 64 } else { 280 },
            cancelled,
        )?;
        return Ok(Preview {
            width: image.width() as usize,
            height: image.height() as usize,
            source_size: [image.width() as usize, image.height() as usize],
            rgba: image.into_raw().into(),
            fingerprint: format!("{}-{generation}-{:?}", job.id, job.pass),
        });
    }
    // Reuse the bounded canvas passes with a private document view. Saved styles
    // can be previewed without applying them or borrowing another icon's recipe.
    let styled_document;
    let render_id;
    let viewer_style = job.id.starts_with("viewer-style:");
    let (document, id) = if let Some(style) = job
        .id
        .strip_prefix("style:")
        .or_else(|| job.id.strip_prefix("viewer-style:"))
    {
        let mut parts = style.splitn(3, ':');
        let edge = parts.next().ok_or("Missing style preview size")?;
        let style = parts.next().ok_or("Missing preview style")?;
        let achievement = parts.next().ok_or("Missing preview achievement")?;
        let saved = document
            .icon_styles()
            .into_iter()
            .find(|s| s.id == style)
            .or_else(|| {
                let (collection, id) = style.split_once('/')?;
                document.project()["styleViewerLibrary"][collection]
                    .as_array()?
                    .iter()
                    .find_map(|value| {
                        let saved: achimeld_application::IconStyle =
                            serde_json::from_value(value.clone()).ok()?;
                        (saved.id == id).then_some(saved)
                    })
            })
            .ok_or("Project icon style no longer exists")?;
        if !viewer_style
            && !saved.recipe["layers"].as_array().is_some_and(|layers| {
                layers.iter().any(|layer| {
                    (layer["kind"] == "Particle"
                        && layer["particles"]["differentSeedPerInstance"] == true)
                        || (layer["kind"] == "Explosion"
                            && layer["explosion"]["differentSeedPerInstance"] == true)
                })
            })
            && let Some(png) = &saved.preview_png
            && let Ok(image) = saved_style_image(png, edge, job.pass)
        {
            let source_size = [image.1[0] as usize, image.1[1] as usize];
            return Ok(Preview {
                width: image.0.width() as usize,
                height: image.0.height() as usize,
                source_size,
                rgba: image.0.into_raw().into(),
                fingerprint: format!("{}-{generation}-{:?}", job.id, job.pass),
            });
        }
        let mut project = document.project().clone();
        let entity = project["achievements"]
            .as_array_mut()
            .and_then(|entities| {
                entities
                    .iter_mut()
                    .find(|entity| entity["id"] == achievement)
            })
            .ok_or("Achievement no longer exists")?;
        entity["icon"] = saved.recipe;
        if viewer_style && let Some((collection, _)) = style.split_once('/') {
            entity["generalImageEntries"] =
                document.project()["styleViewerLibraryImages"][collection].clone();
        }
        if viewer_style {
            let preview = achimeld_application::style_viewer::StyleViewerPreview::from_project(
                document.project(),
            );
            preview.apply_images(
                entity,
                document.project()["styleViewerDefaultImages"]
                    .as_array()
                    .map_or(&[], Vec::as_slice),
            );
        }
        styled_document = Document::from_parts(project, document.catalog().clone())?;
        render_id = format!("canvas:{edge}:{achievement}");
        (&styled_document, render_id.as_str())
    } else if let Some(blend) = job.id.strip_prefix("blend:") {
        let mut parts = blend.splitn(4, ':');
        let edge = parts.next().ok_or("Missing blend preview size")?;
        let mode = parts.next().ok_or("Missing blend preview mode")?;
        let layer_id = parts.next().ok_or("Missing blend preview layer")?;
        let achievement = parts.next().ok_or("Missing blend preview achievement")?;
        let mut project = document.project().clone();
        let entity = project["achievements"]
            .as_array_mut()
            .and_then(|entities| entities.iter_mut().find(|e| e["id"] == achievement))
            .ok_or("Achievement no longer exists")?;
        if !entity["icon"].is_object() {
            entity["icon"] = serde_json::json!({"layers":[{
                "id":entity["id"],
                "kind":if entity["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},
                "atlasId":entity["atlasId"],"imageId":entity["standaloneImageId"],"tileIndex":entity["tileIndex"]
            }]});
        }
        let layer = entity["icon"]["layers"]
            .as_array_mut()
            .and_then(|layers| layers.iter_mut().find(|l| l["id"] == layer_id))
            .ok_or("Layer no longer exists")?;
        layer["blendMode"] = serde_json::json!(mode);
        styled_document = Document::from_parts(project, document.catalog().clone())?;
        render_id = format!("canvas:{edge}:{achievement}");
        (&styled_document, render_id.as_str())
    } else {
        (document, job.id.as_str())
    };
    let mut source_size = None;
    let image = if let Some(asset) = id.strip_prefix("asset:") {
        let (kind, key) = asset.split_once(':').unwrap_or(("", ""));
        let reference = if kind == "atlas" && key == "default" {
            Some(&document.project()["assets"]["atlas"])
        } else {
            document.project()["assets"][if kind == "atlas" { "atlases" } else { "images" }]
                .as_array()
                .and_then(|assets| assets.iter().find(|a| a["id"].as_str() == Some(key)))
                .map(|a| &a["image"])
        };
        let image = assets.image(reference.ok_or("Image asset is missing")?)?;
        source_size = Some([image.width() as usize, image.height() as usize]);
        let ratio = (512.0 / image.width().max(image.height()) as f64).min(1.0);
        image::imageops::resize(
            image.as_ref(),
            (image.width() as f64 * ratio).round().max(1.0) as u32,
            (image.height() as f64 * ratio).round().max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else if let Some((achievement, layer)) = id.split_once('/') {
        render_layer_cancellable(document, achievement, layer, assets, cancelled)?
    } else {
        let (id, canvas_edge) = if let Some(canvas) = id.strip_prefix("canvas:") {
            let (edge, id) = canvas.split_once(':').ok_or("Invalid canvas preview")?;
            let edge = edge
                .parse::<u32>()
                .map_err(|_| "Invalid canvas preview size")?;
            if !(64..=512).contains(&edge) || !edge.is_power_of_two() {
                return Err("Invalid canvas preview size".into());
            }
            (id, Some(edge))
        } else {
            (id, None)
        };
        let settings = &document.project()["atlas"];
        let default = settings["outputSize"].as_u64().unwrap_or(512);
        let width = settings["outputWidth"].as_u64().unwrap_or(default);
        let height = settings["outputHeight"].as_u64().unwrap_or(default);
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("Icon dimensions must be between 1 and 4096 pixels".into());
        }
        let edge = if job.pass == Pass::Quick {
            64
        } else {
            canvas_edge.unwrap_or(4096)
        };
        let ratio = (f64::from(edge) / width.max(height) as f64).min(1.0);
        source_size = Some([width as usize, height as usize]);
        render_achieved_cancellable(
            document,
            id,
            if job.pass == Pass::Quick || canvas_edge.is_some() {
                Some((
                    (width as f64 * ratio).round().max(1.0) as u32,
                    (height as f64 * ratio).round().max(1.0) as u32,
                ))
            } else {
                None
            },
            assets,
            cancelled,
        )?
    };
    if cancelled() {
        return Err("Icon rendering cancelled".into());
    }
    Ok(Preview {
        width: image.width() as usize,
        height: image.height() as usize,
        source_size: source_size.unwrap_or([image.width() as usize, image.height() as usize]),
        rgba: image.into_raw().into(),
        fingerprint: format!("{}-{generation}-{:?}", job.id, job.pass),
    })
}

fn saved_style_image(
    png: &str,
    edge: &str,
    pass: Pass,
) -> Result<(image::RgbaImage, [u32; 2]), String> {
    use base64::Engine as _;
    let edge = edge
        .parse::<u32>()
        .map_err(|_| "Invalid style preview size")?;
    if !(64..=512).contains(&edge) || !edge.is_power_of_two() || png.len() > 4 * 1024 * 1024 {
        return Err("Invalid style preview size".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png)
        .map_err(|error| error.to_string())?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let source_size = [image.width(), image.height()];
    let edge = if pass == Pass::Quick { 64 } else { edge };
    let ratio = (f64::from(edge) / image.width().max(image.height()).max(1) as f64).min(1.0);
    Ok((
        image::imageops::resize(
            &image,
            (image.width() as f64 * ratio).round().max(1.0) as u32,
            (image.height() as f64 * ratio).round().max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        ),
        source_size,
    ))
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        if let Ok(mut batch) = lock.lock() {
            batch.stop = true;
            batch.cancelled.store(true, Ordering::Release);
            batch.quick.clear();
            batch.detail.clear();
            wake.notify_all();
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use achimeld_application::{Command, Editor, EntityKind, new_id};
    use serde_json::json;
    use std::time::{Duration, Instant};

    #[test]
    fn blend_hover_renders_only_the_target_and_cancels_without_editing() {
        let (document, ids) = fixture(2);
        let mut project = document.project().clone();
        let layer = new_id();
        project["achievements"][0]["icon"] = json!({"layers":[
            {"id":new_id(),"kind":"Solid","solidColorHex":"#FF804020"},
            {"id":layer,"kind":"Solid","solidColorHex":"#FF4080C0","blendMode":"Normal"}
        ]});
        let document = Document::from_parts(project, document.catalog().clone()).unwrap();
        let before = document.clone();
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        worker.achievement_preview(&ids[0], Some(256), 1, &document, None);
        let normal_key = format!("canvas:256:{}", ids[0]);
        wait_until(|| {
            worker.poll();
            worker.states[&normal_key]
                .output()
                .is_some_and(|p| p.width == 256)
        });
        let normal = worker.states[&normal_key].output().unwrap().clone();
        assert_eq!(&normal.rgba[..4], &[64, 128, 192, 255]);
        let request = |mode: &str| {
            Some(LayerBlendPreview {
                achievement_id: ids[0].clone(),
                layer_id: layer.clone(),
                mode: mode.into(),
            })
        };
        let started = Instant::now();
        worker.set_blend_preview(request("Multiply"));
        assert!(
            worker
                .achievement_preview(&ids[0], Some(256), 1, &document, None)
                .is_none()
        );
        let multiply_key = format!("blend:256:Multiply:{layer}:{}", ids[0]);
        // Quick pixels are published before refinement is admitted by poll().
        wait_until(|| {
            let batch = worker.queue.0.lock().unwrap();
            batch.quick.is_empty() && batch.active_quick == 0
        });
        worker.poll();
        let first = started.elapsed();
        let quick = worker.states[&multiply_key].output().unwrap();
        assert_eq!([quick.width, quick.height], [64, 64]);
        assert!(quick.fingerprint.ends_with("Quick"));
        assert_eq!(&quick.rgba[..4], &[32, 32, 24, 255]);
        wait_until(|| {
            worker.poll();
            worker.states[&multiply_key]
                .output()
                .is_some_and(|p| p.width == 256)
        });
        eprintln!(
            "Blend hover quick preview ready: {first:?}; final quality ready: {:?}",
            started.elapsed()
        );
        let old_token = worker.blend_cancelled.clone();
        worker.set_blend_preview(request("Screen"));
        assert!(old_token.load(Ordering::Acquire));
        assert!(!worker.states.contains_key(&multiply_key));
        worker.achievement_preview(&ids[0], Some(256), 1, &document, None);
        let screen_token = worker.blend_cancelled.clone();
        worker.set_blend_preview(request("Difference"));
        assert!(screen_token.load(Ordering::Acquire));
        worker.achievement_preview(&ids[0], Some(256), 1, &document, None);
        worker.achievement_preview(&ids[1], Some(64), 1, &document, None);
        let difference_key = format!("blend:256:Difference:{layer}:{}", ids[0]);
        wait_until(|| {
            worker.poll();
            worker.states[&difference_key]
                .output()
                .is_some_and(|p| p.width == 256)
                && worker.states[&format!("canvas:64:{}", ids[1])]
                    .output()
                    .is_some()
        });
        assert_eq!(
            &worker.states[&difference_key].output().unwrap().rgba[..4],
            &[64, 64, 160, 255]
        );
        let token = worker.blend_cancelled.clone();
        worker.set_blend_preview(None);
        assert!(token.load(Ordering::Acquire));
        let restored = worker
            .achievement_preview(&ids[0], Some(256), 1, &document, None)
            .unwrap();
        assert!(Arc::ptr_eq(&normal.rgba, &restored.rgba));
        assert!(worker.states.keys().all(|id| !id.starts_with("blend:")));
        assert_eq!(document, before);
    }

    #[test]
    fn style_previews_use_saved_json_png_and_fall_back_to_recipe_without_editing() {
        use base64::Engine as _;
        let (document, ids) = fixture(1);
        let mut project = document.project().clone();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(160, 80, image::Rgba([11, 22, 33, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let style = new_id();
        project["iconStyles"] = json!([{"id":style,"name":"Default preview",
            "recipe":{"layers":[{"id":new_id(),"kind":"Solid","solidColorHex":"#FFFF0000"}]},
            "previewPng":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}]);
        let document = Document::from_parts(project, document.catalog().clone()).unwrap();
        let before = document.project().clone();
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        let key = format!("style:128:{style}:{}", ids[0]);
        worker.preview(&key, 1, &document, None);
        wait_until(|| {
            worker.poll();
            worker.states[&key]
                .output()
                .is_some_and(|p| p.fingerprint.ends_with("Detail"))
        });
        let preview = worker.states[&key].output().unwrap();
        assert_eq!([preview.width, preview.height], [128, 64]);
        assert_eq!(preview.source_size, [160, 80]);
        assert_eq!(&preview.rgba[..4], &[11, 22, 33, 255]);
        assert_eq!(document.project(), &before);
        let mut project = before.clone();
        project["iconStyles"][0]["previewPng"] = json!("invalid PNG");
        let fallback = Document::from_parts(project, document.catalog().clone()).unwrap();
        worker.preview(&key, 2, &fallback, None);
        wait_until(|| {
            worker.poll();
            worker.states[&key].output().is_some_and(|p| {
                p.fingerprint.ends_with("Detail") && p.rgba[..4] == [255, 0, 0, 255]
            })
        });
        assert_eq!(worker.states[&key].output().unwrap().width, 128);
        assert_eq!(fallback.project()["achievements"], before["achievements"]);
    }

    #[test]
    fn particle_instance_style_previews_ignore_a_shared_saved_png() {
        use base64::Engine as _;
        let (document, ids) = fixture(2);
        let mut project = document.project().clone();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(64, 64, image::Rgba([11, 22, 33, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let style = new_id();
        let settings = achimeld_application::particles::ParticleSettings {
            different_seed_per_instance: true,
            size_min: 40.0,
            size_max: 100.0,
            shapes: vec![
                Default::default(),
                achimeld_application::particles::ParticleShape {
                    operation: "Subtract".into(),
                    scale: 0.6,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        project["iconStyles"] = json!([{"id":style,"name":"Particle style","recipe":{"layers":[{"id":new_id(),"kind":"Particle","particles":settings}]},
            "previewPng":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}]);
        let document = Document::from_parts(project, document.catalog().clone()).unwrap();
        let before = document.clone();
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        let keys: Vec<_> = ids
            .iter()
            .map(|id| format!("style:512:{style}:{id}"))
            .collect();
        let started = Instant::now();
        let mut first_visible = None;
        for key in &keys {
            worker.preview(key, 1, &document, None);
        }
        wait_until(|| {
            worker.poll();
            if first_visible.is_none() && worker.states[&keys[0]].output().is_some() {
                first_visible = Some(started.elapsed());
            }
            keys.iter().all(|key| {
                worker.states[key]
                    .output()
                    .is_some_and(|p| p.fingerprint.ends_with("Detail"))
            })
        });
        eprintln!(
            "Particle preview, two 512px ring instances: first visible {:?}, final quality {:?}",
            first_visible.unwrap(),
            started.elapsed()
        );
        let first = worker.states[&keys[0]].output().unwrap();
        let second = worker.states[&keys[1]].output().unwrap();
        assert_ne!(first.rgba, second.rgba);
        assert_ne!(&first.rgba[..4], &[11, 22, 33, 255]);
        assert_eq!(document, before);
    }

    #[test]
    fn explosion_instance_style_previews_refine_without_using_a_shared_saved_png() {
        use base64::Engine as _;
        let (document, ids) = fixture(2);
        let mut project = document.project().clone();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(64, 64, image::Rgba([11, 22, 33, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let style = new_id();
        let mut settings = achimeld_application::explosion::ExplosionSettings {
            different_seed_per_instance: true,
            deltoid: Some(achimeld_application::explosion::DeltoidSettings {
                scale: 1.25,
                scale_min: 0.5,
                scale_max: 1.5,
                top_height_min: 0.1,
                top_height_max: 0.25,
                bottom_height_min: 0.25,
                bottom_height_max: 0.5,
                ..Default::default()
            }),
            ..Default::default()
        };
        settings.cover_canvas = true;
        settings.gradient["kind"] = json!("SDF");
        project["iconStyles"] = json!([{"id":style,"name":"Explosion style","recipe":{"layers":[{"id":new_id(),"kind":"Explosion","explosion":settings}]},
            "previewPng":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}]);
        let document = Document::from_parts(project, document.catalog().clone()).unwrap();
        let before = document.clone();
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        let keys: Vec<_> = ids
            .iter()
            .map(|id| format!("style:512:{style}:{id}"))
            .collect();
        let started = Instant::now();
        let mut first_visible = None;
        for key in &keys {
            worker.preview(key, 1, &document, None);
        }
        wait_until(|| {
            worker.poll();
            if first_visible.is_none() && worker.states[&keys[0]].output().is_some() {
                first_visible = Some(started.elapsed());
            }
            keys.iter().all(|key| {
                worker.states[key]
                    .output()
                    .is_some_and(|p| p.fingerprint.ends_with("Detail"))
            })
        });
        eprintln!(
            "Explosion preview, two 512px instances: first visible {:?}, final quality {:?}",
            first_visible.unwrap(),
            started.elapsed()
        );
        let first = worker.states[&keys[0]].output().unwrap();
        let second = worker.states[&keys[1]].output().unwrap();
        assert_ne!(first.rgba, second.rgba);
        assert_ne!(&first.rgba[..4], &[11, 22, 33, 255]);
        assert_eq!(document, before);
    }

    #[test]
    fn canvas_previews_are_bounded_and_do_not_refine_small_cards_to_export_size() {
        let (document, ids) = fixture(1);
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        let small = format!("canvas:64:{}", ids[0]);
        worker.preview(&small, 1, &document, None);
        wait_until(|| {
            worker.poll();
            worker.states[&small].output().is_some()
        });
        let preview = worker.states[&small].output().unwrap();
        assert_eq!([preview.width, preview.height], [64, 64]);
        assert_eq!(preview.source_size, [512, 512]);
        assert!(preview.fingerprint.ends_with("Single"));
        let large = format!("canvas:256:{}", ids[0]);
        worker.preview(&large, 1, &document, None);
        wait_until(|| {
            worker.poll();
            worker.states[&large]
                .output()
                .is_some_and(|p| p.fingerprint.ends_with("Detail"))
        });
        let preview = worker.states[&large].output().unwrap();
        assert_eq!([preview.width, preview.height], [256, 256]);
        assert_eq!(worker.states[&small].output().unwrap().width, 64);
        worker.preview(&ids[0], 1, &document, None);
        wait_until(|| {
            worker.poll();
            worker.states[&ids[0]]
                .output()
                .is_some_and(|p| p.fingerprint.ends_with("Detail"))
        });
        assert_eq!(worker.states[&ids[0]].output().unwrap().width, 512);
    }

    fn fixture(count: usize) -> (Document, Vec<String>) {
        let mut editor = Editor::default();
        let mut ids = Vec::new();
        for _ in 0..count {
            editor
                .execute("Add", Command::Add(EntityKind::Achievement), None)
                .unwrap();
            let mut entity = editor
                .document()
                .entities(EntityKind::Achievement)
                .last()
                .unwrap()
                .clone();
            let id = entity["id"].as_str().unwrap().to_owned();
            entity["icon"] = json!({"layers":[
                {"id":new_id(),"kind":"Gradient","gradient":{"kind":"Linear","angle":45.0,"stops":[{"offset":0.0,"colorHex":"#FF4672DE"},{"offset":1.0,"colorHex":"#FF9B70E8"}]}},
                {"id":new_id(),"kind":"Shape","shape":{"kind":"Sphere","scaleX":0.5,"scaleY":0.5,"colorHex":"#FFFFFFFF"},"effects":[{"kind":"BoxBlur","blur":4.0}]}
            ]});
            editor
                .execute(
                    "Artwork",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.clone(),
                        value: entity,
                    },
                    None,
                )
                .unwrap();
            ids.push(id);
        }
        (editor.document().clone(), ids)
    }
    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !condition() {
            assert!(Instant::now() < deadline, "Preview did not complete");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn quick_results_reach_ui_before_full_resolution_and_pixels_are_shared() {
        let (document, ids) = fixture(2);
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 2);
        for id in &ids {
            assert!(worker.preview(id, 0, &document, None).is_none());
        }
        // Both quick results fit in the channel. No refinement may start before poll.
        wait_until(|| {
            let batch = worker.queue.0.lock().unwrap();
            batch.quick.is_empty() && batch.active_quick == 0
        });
        worker.poll();
        for id in &ids {
            let quick = worker.preview(id, 0, &document, None).unwrap();
            assert_eq!([quick.width, quick.height], [64, 64]);
            assert!(Arc::ptr_eq(
                &quick.rgba,
                &worker.preview(id, 0, &document, None).unwrap().rgba
            ));
        }
        wait_until(|| {
            worker.poll();
            ids.iter()
                .all(|id| worker.states[id].output().is_some_and(|p| p.width == 512))
        });
        for id in &ids {
            let full = worker.preview(id, 0, &document, None).unwrap();
            assert_eq!([full.width, full.height], [512, 512]);
            assert!(full.fingerprint.ends_with("Detail"));
        }
    }
    #[test]
    fn in_flight_quick_work_blocks_refinement() {
        let worker = PreviewWorker::with_threads(egui::Context::default(), 1);
        let mut batch = worker.queue.0.lock().unwrap();
        batch.quick.push_back(Job {
            id: "quick".into(),
            pass: Pass::Quick,
            cancelled: None,
        });
        batch.detail.push_back(Job {
            id: "detail".into(),
            pass: Pass::Detail,
            cancelled: None,
        });
        assert_eq!(batch.take().unwrap().pass, Pass::Quick);
        assert!(batch.take().is_none());
        batch.active_quick -= 1;
        assert_eq!(batch.take().unwrap().pass, Pass::Detail);
    }
    #[test]
    fn project_changes_reject_late_results_with_reused_source_generation() {
        let (first, ids) = fixture(1);
        let (second, _) = fixture(1);
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 1);
        worker.preview(&ids[0], 0, &first, None);
        let old = worker.generation;
        let cancellation = worker.queue.0.lock().unwrap().cancelled.clone();
        worker.preview(&ids[0], 0, &second, None);
        assert!(cancellation.load(Ordering::Acquire));
        assert_ne!(old, worker.generation);
        assert!(
            !worker
                .states
                .get_mut(&ids[0])
                .unwrap()
                .complete(old, Err("obsolete".into()))
        );
        assert!(worker.error(&ids[0]).is_none());
    }
    #[test]
    fn shutdown_cancels_workers_with_a_full_result_channel() {
        let (document, ids) = fixture(3);
        let (done, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut worker = PreviewWorker::with_threads(egui::Context::default(), 1);
            for id in ids {
                worker.preview(&id, 0, &document, None);
            }
            std::thread::sleep(Duration::from_millis(100));
            drop(worker);
            done.send(()).unwrap();
        });
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    #[test]
    #[ignore = "Synthetic rendering timings; run explicitly in release mode"]
    fn progressive_preview_timings() {
        let (document, ids) = fixture(80);
        for count in [1, 4] {
            let mut worker = PreviewWorker::with_threads(egui::Context::default(), count);
            let start = Instant::now();
            for id in &ids {
                worker.preview(id, 1, &document, None);
            }
            let mut first = None;
            let mut quick_time = None;
            wait_until(|| {
                worker.poll();
                if worker.states.values().any(|s| s.output().is_some()) {
                    first.get_or_insert_with(|| start.elapsed());
                }
                if ids.iter().all(|id| worker.states[id].output().is_some()) {
                    quick_time.get_or_insert_with(|| start.elapsed());
                }
                ids.iter()
                    .all(|id| worker.states[id].output().is_some_and(|p| p.width == 512))
            });
            println!(
                "{count} workers, 80 layered icons: first {:?}, all initial {:?}, all full {:?}",
                first.unwrap(),
                quick_time.unwrap(),
                start.elapsed()
            );
        }
        let start = Instant::now();
        let mut assets = Files::new();
        let mut first = None;
        for id in &ids {
            achimeld_imaging::render(&document, id, Some((256, 256)), &mut assets).unwrap();
            first.get_or_insert_with(|| start.elapsed());
        }
        println!(
            "Previous serial 256x256 pair rendering: first {:?}, all {:?}",
            first.unwrap(),
            start.elapsed()
        );
    }
}
