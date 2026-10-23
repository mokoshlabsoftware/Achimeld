//! Opt-in, read-only benchmark of a real project's existing load/render paths.
use super::*;
use achimeld_application::{Command, Editor, EditorPort, EntityKind, Preferences, UiRequest};
use achimeld_persistence::project::{parse_catalog, resolve_reference};
use std::hint::black_box;
use std::time::{Duration, Instant};

struct TimedAssets {
    files: Files,
    image_time: Duration,
    font_time: Duration,
    images: usize,
}
impl TimedAssets {
    fn new(path: &std::path::Path) -> Self {
        let mut files = Files::new();
        files.project = Some(path.to_owned());
        Self {
            files,
            image_time: Duration::ZERO,
            font_time: Duration::ZERO,
            images: 0,
        }
    }
}
impl AssetSource for TimedAssets {
    fn image(&mut self, reference: &serde_json::Value) -> Result<Arc<image::RgbaImage>, String> {
        let start = Instant::now();
        let result = self.files.image(reference);
        self.image_time += start.elapsed();
        self.images += 1;
        result
    }
    fn font(&mut self, family: &str) -> Option<Arc<achimeld_imaging::Font>> {
        let start = Instant::now();
        let result = self.files.font(family);
        self.font_time += start.elapsed();
        result
    }
}

struct BenchPort {
    editor: Editor,
    previews: BTreeMap<String, Preview>,
    requested: usize,
    canvas_previews: BTreeMap<(String, usize), Preview>,
}
impl EditorPort for BenchPort {
    fn editor(&self) -> &Editor {
        &self.editor
    }
    fn preferences(&self) -> &Preferences {
        self.editor.preferences()
    }
    fn execute(
        &mut self,
        label: &str,
        command: Command,
        group: Option<&str>,
    ) -> Result<(), String> {
        self.editor.execute(label, command, group)
    }
    fn finish_group(&mut self) {
        self.editor.finish_group();
    }
    fn undo(&mut self) {
        self.editor.undo();
    }
    fn redo(&mut self) {
        self.editor.redo();
    }
    fn request(&mut self, _: UiRequest) {
        panic!("Benchmark must not issue host requests");
    }
    fn status(&self) -> &str {
        "Benchmark"
    }
    fn preview(&mut self, id: &str) -> Option<Preview> {
        self.requested += 1;
        self.previews.get(id).cloned()
    }
    fn canvas_preview(&mut self, id: &str, edge: usize) -> Option<Preview> {
        self.requested += 1;
        let key = (id.to_owned(), edge);
        if let Some(preview) = self.canvas_previews.get(&key) {
            return Some(preview.clone());
        }
        let source = self.previews.get(id)?;
        let ratio = (edge as f64 / source.width.max(source.height) as f64).min(1.0);
        let image = image::RgbaImage::from_raw(
            source.width as u32,
            source.height as u32,
            source.rgba.to_vec(),
        )
        .unwrap();
        let image = image::imageops::resize(
            &image,
            (source.width as f64 * ratio).round().max(1.0) as u32,
            (source.height as f64 * ratio).round().max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        );
        let preview = Preview {
            width: image.width() as usize,
            height: image.height() as usize,
            source_size: source.source_size,
            rgba: image.into_raw().into(),
            fingerprint: format!("{}-{edge}", source.fingerprint),
        };
        self.canvas_previews.insert(key, preview.clone());
        Some(preview)
    }
}

fn measure(label: &str, count: usize, mut action: impl FnMut()) {
    let mut times = Vec::with_capacity(count);
    for _ in 0..count {
        let start = Instant::now();
        action();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "BENCH {label}: mean={:.3}ms p50={:.3}ms p95={:.3}ms max={:.3}ms n={count}",
        times.iter().sum::<f64>() / count as f64,
        times[count / 2],
        times[(count * 95 / 100).min(count - 1)],
        times[count - 1]
    );
}

#[test]
#[ignore = "Set ACHIMELD_BENCH_PROJECT and run explicitly in release mode"]
fn real_project_performance() {
    let path =
        PathBuf::from(std::env::var_os("ACHIMELD_BENCH_PROJECT").expect("ACHIMELD_BENCH_PROJECT"));
    // Parse read-only: ProjectStore::load also owns recovery/lock files, which are
    // deliberately excluded so this benchmark never changes the user's project.
    let start = Instant::now();
    let bytes = std::fs::read(&path).unwrap();
    let project: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let csv =
        std::fs::read(resolve_reference(&project["assets"]["localizationCatalog"], &path).unwrap())
            .unwrap();
    let document = Document::from_parts(project, parse_catalog(&csv).unwrap()).unwrap();
    println!(
        "BENCH read_parse_validate={:.3}ms bytes={} achievements={}",
        start.elapsed().as_secs_f64() * 1000.0,
        bytes.len(),
        document.entities(EntityKind::Achievement).len()
    );
    measure(
        "clone all achievement records (including drop)",
        150,
        || {
            black_box(document.entities(EntityKind::Achievement).to_vec());
        },
    );
    measure("clone document (including drop)", 100, || {
        black_box(document.clone());
    });
    measure("validate document", 100, || {
        document.validate().unwrap();
    });
    let mut editor = Editor::new(document.clone());
    measure("saved is_dirty", 150, || {
        black_box(editor.is_dirty());
    });
    let mut position = 0.0;
    measure("coalesced camera command", 100, || {
        position += 0.1;
        editor
            .execute(
                "Camera",
                Command::CanvasCamera {
                    kind: EntityKind::Achievement,
                    zoom: 0.3138428,
                    offset_x: position,
                    offset_y: -77.0,
                },
                Some("camera"),
            )
            .unwrap();
    });

    let ids: Vec<String> = document
        .entities(EntityKind::Achievement)
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect();
    let mut full = BTreeMap::new();
    for pass in [Pass::Quick, Pass::Detail] {
        let mut assets = TimedAssets::new(&path);
        let start = Instant::now();
        let mut first = None;
        let mut errors = 0;
        for id in &ids {
            match render_job(
                &document,
                &Job {
                    id: id.clone(),
                    pass,
                    cancelled: None,
                },
                1,
                &mut assets,
                &|| false,
            ) {
                Ok(preview) => {
                    first.get_or_insert_with(|| start.elapsed());
                    if pass == Pass::Detail {
                        full.insert(id.clone(), preview);
                    }
                }
                Err(error) => {
                    errors += 1;
                    if errors < 4 {
                        println!("BENCH render error: {error}");
                    }
                }
            }
        }
        println!(
            "BENCH serial {pass:?}: first={first:?} total={:?} image_io_decode={:?} font_io={:?} image_calls={} errors={errors}",
            start.elapsed(),
            assets.image_time,
            assets.font_time,
            assets.images
        );
    }
    println!(
        "BENCH renderable={} failed={} (failed artwork is excluded from texture costs)",
        full.len(),
        ids.len() - full.len()
    );
    assert!(
        !full.is_empty(),
        "No artwork rendered; texture measurements would be invalid"
    );

    for count in [1, 4] {
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), count);
        let start = Instant::now();
        for id in &ids {
            worker.preview(id, 1, &document, Some(&path));
        }
        let mut first = None;
        let mut initial = None;
        loop {
            worker.poll();
            if worker.states.values().any(|s| s.output().is_some()) {
                first.get_or_insert_with(|| start.elapsed());
            }
            if ids.iter().all(|id| {
                worker.states[id].output().is_some() || worker.states[id].error().is_some()
            }) {
                initial.get_or_insert_with(|| start.elapsed());
            }
            if ids.iter().all(|id| {
                worker.states[id].error().is_some()
                    || worker.states[id]
                        .output()
                        .is_some_and(|p| p.fingerprint.ends_with("Detail"))
            }) {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(180),
                "Benchmark timeout"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        println!(
            "BENCH progressive {count} workers: first={first:?} all_initial_or_error={initial:?} all_detail_or_error={:?} errors={}",
            start.elapsed(),
            worker
                .states
                .values()
                .filter(|s| s.error().is_some())
                .count()
        );
    }

    {
        let mut worker = PreviewWorker::with_threads(egui::Context::default(), 4);
        let keys: Vec<_> = ids.iter().map(|id| format!("canvas:64:{id}")).collect();
        let start = Instant::now();
        for key in &keys {
            worker.preview(key, 1, &document, Some(&path));
        }
        let mut first = None;
        loop {
            worker.poll();
            if worker.states.values().any(|s| s.output().is_some()) {
                first.get_or_insert_with(|| start.elapsed());
            }
            if keys.iter().all(|key| {
                worker.states[key].output().is_some() || worker.states[key].error().is_some()
            }) {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(180),
                "Canvas benchmark timeout"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let bytes: usize = worker
            .states
            .values()
            .filter_map(|s| s.output())
            .map(|p| p.rgba.len())
            .sum();
        println!(
            "BENCH actual canvas 64px 4 workers: first={first:?} all={:?} cached_MiB={:.2}",
            start.elapsed(),
            bytes as f64 / 1048576.0
        );
    }

    let small: BTreeMap<_, _> = full
        .iter()
        .map(|(id, preview)| {
            let source = image::RgbaImage::from_raw(
                preview.width as u32,
                preview.height as u32,
                preview.rgba.to_vec(),
            )
            .unwrap();
            let pixels =
                image::imageops::resize(&source, 256, 256, image::imageops::FilterType::Triangle);
            (
                id.clone(),
                Preview {
                    width: 256,
                    height: 256,
                    source_size: preview.source_size,
                    rgba: pixels.into_raw().into(),
                    fingerprint: format!("{id}-256"),
                },
            )
        })
        .collect();
    for (label, previews, fit_all) in [
        ("saved camera no textures", BTreeMap::new(), false),
        ("saved camera 512px", full.clone(), false),
        ("all 100 visible 512px", full, true),
        ("all 100 visible 256px", small, true),
    ] {
        let ctx = egui::Context::default();
        let mut desktop = achimeld_ui::Desktop::default();
        let mut ui_document = document.clone();
        if fit_all {
            let mut project = document.project().clone();
            project["achievementsCamera"] =
                serde_json::json!({"zoom":0.20,"offsetX":-20.0,"offsetY":-20.0});
            ui_document = Document::from_parts(project, document.catalog().clone()).unwrap();
        }
        let mut port = BenchPort {
            editor: Editor::new(ui_document),
            previews,
            requested: 0,
            canvas_previews: BTreeMap::new(),
        };
        let mut time = 0.0;
        let mut frame = |scroll: bool| {
            time += 1.0 / 60.0;
            let mut events = vec![egui::Event::PointerMoved(egui::pos2(700.0, 400.0))];
            if scroll {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(
                        0.0,
                        if ((time * 60.0) as usize).is_multiple_of(2) {
                            2.0
                        } else {
                            -2.0
                        },
                    ),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 920.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    desktop.show(ui, &mut port);
                },
            );
            let updates = output
                .textures_delta
                .set
                .values()
                .map(|deltas| deltas.len())
                .sum::<usize>();
            let bytes: usize = output
                .textures_delta
                .set
                .values()
                .flatten()
                .map(|delta| {
                    let [w, h] = delta.image.size();
                    w * h * 4
                })
                .sum();
            black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
            (updates, bytes)
        };
        for _ in 0..10 {
            frame(false);
        }
        let mut uploads = (0, 0);
        measure(&format!("UI idle {label}"), 120, || {
            let (count, bytes) = frame(false);
            uploads.0 += count;
            uploads.1 += bytes;
        });
        println!(
            "BENCH idle texture updates/frame={:.1} base_level_MiB/frame={:.2}",
            uploads.0 as f64 / 120.0,
            uploads.1 as f64 / 120.0 / 1048576.0
        );
        measure(&format!("UI zoom {label}"), 120, || {
            black_box(frame(true));
        });
        println!(
            "BENCH {label}: preview requests across 250 frames={}",
            port.requested
        );
    }
}
