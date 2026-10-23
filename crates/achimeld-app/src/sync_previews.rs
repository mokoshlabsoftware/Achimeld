//! Thumbnails of the exact staged upload bytes. One decode job and an 8 MiB cache.
use achimeld_application::Preview;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc;

type PendingDecode = (String, mpsc::Receiver<Result<[Preview; 2], String>>);

#[derive(Default)]
pub(crate) struct IconPreviews {
    pub files: BTreeMap<String, Vec<PathBuf>>,
    cache: BTreeMap<String, ([Preview; 2], u64)>,
    pending: Option<PendingDecode>,
    failed: BTreeSet<String>,
    clock: u64,
}
impl IconPreviews {
    pub fn get(&self, api: &str, locked: bool) -> Option<Preview> {
        self.cache
            .get(api)
            .map(|(pair, _)| pair[usize::from(locked)].clone())
    }
    pub fn poll(&mut self) {
        let Some((api, receiver)) = &self.pending else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Icon preview stopped".into()),
        };
        let api = api.clone();
        self.pending = None;
        match result {
            Ok(pair) => {
                self.clock += 1;
                self.cache.insert(api, (pair, self.clock));
            }
            Err(_) => {
                self.failed.insert(api);
            }
        }
        while self
            .cache
            .values()
            .map(|(pair, _)| pair.iter().map(|p| p.rgba.len()).sum::<usize>())
            .sum::<usize>()
            > 8 * 1024 * 1024
        {
            let Some(oldest) = self
                .cache
                .iter()
                .min_by_key(|(_, (_, tick))| *tick)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.cache.remove(&oldest);
        }
    }
    pub fn request(&mut self, api: &str, ctx: &eframe::egui::Context) {
        if self.cache.contains_key(api) {
            self.clock += 1;
            if let Some((_, tick)) = self.cache.get_mut(api) {
                *tick = self.clock;
            }
            return;
        }
        if self.pending.is_some() || self.failed.contains(api) {
            return;
        }
        let Some(paths) = self
            .files
            .get(api)
            .filter(|paths| paths.len() == 2)
            .cloned()
        else {
            return;
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let mut previews = Vec::new();
                for path in paths {
                    let image = crate::art_bank::decode(&path)?;
                    let source_size = [image.width() as usize, image.height() as usize];
                    let scale = (96.0 / image.width().max(image.height()).max(1) as f64).min(1.0);
                    let thumbnail = image::imageops::resize(
                        &image,
                        (image.width() as f64 * scale).round().max(1.0) as u32,
                        (image.height() as f64 * scale).round().max(1.0) as u32,
                        image::imageops::FilterType::Triangle,
                    );
                    previews.push(Preview {
                        width: thumbnail.width() as usize,
                        height: thumbnail.height() as usize,
                        source_size,
                        rgba: thumbnail.into_raw().into(),
                        fingerprint: path.to_string_lossy().into_owned(),
                    });
                }
                previews
                    .try_into()
                    .map_err(|_| "Expected one generated icon pair".into())
            })();
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.pending = Some((api.into(), receiver));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_staged_pairs_decode_in_background_and_share_cached_pixels() {
        let directory = crate::test_output::root()
            .join(format!("sync-preview-{}", achimeld_application::new_id()));
        std::fs::create_dir(&directory).unwrap();
        let paths = [directory.join("color.png"), directory.join("locked.png")];
        for (path, color) in paths
            .iter()
            .zip([[220, 120, 30, 255], [120, 120, 120, 255]])
        {
            image::RgbaImage::from_pixel(256, 128, image::Rgba(color))
                .save(path)
                .unwrap();
        }
        let mut previews = IconPreviews::default();
        previews.files.insert("COIN".into(), paths.to_vec());
        let ctx = eframe::egui::Context::default();
        previews.request("unreviewed", &ctx);
        assert!(previews.pending.is_none());
        previews.request("COIN", &ctx);
        for _ in 0..200 {
            previews.poll();
            if previews.get("COIN", false).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let color = previews.get("COIN", false).unwrap();
        let locked = previews.get("COIN", true).unwrap();
        assert_eq!([color.width, color.height], [96, 48]);
        assert_eq!(&color.rgba[..4], &[220, 120, 30, 255]);
        assert_eq!(&locked.rgba[..4], &[120, 120, 120, 255]);
        previews.request("COIN", &ctx);
        assert!(previews.pending.is_none());
        assert!(std::sync::Arc::ptr_eq(
            &color.rgba,
            &previews.get("COIN", false).unwrap().rgba
        ));
        for path in paths {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
}
