//! Thumbnails of the exact frozen upload bytes. One decode job and an 8 MiB cache.
use achimeld_application::Preview;
use base64::Engine as _;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, mpsc};

pub(crate) type FrozenIcons = BTreeMap<String, (Vec<u8>, Vec<u8>)>;
type PendingDecode = (String, mpsc::Receiver<Result<[Preview; 2], String>>);

#[derive(Default)]
pub(crate) struct IconPreviews {
    pub frozen: Arc<FrozenIcons>,
    cache: BTreeMap<String, ([Preview; 2], u64)>,
    remote: BTreeMap<(String, bool), Preview>,
    pending: Option<PendingDecode>,
    failed: BTreeSet<String>,
    clock: u64,
}
impl IconPreviews {
    pub fn remote(&self, api: &str, unachieved: bool) -> Option<Preview> {
        self.remote.get(&(api.to_owned(), unachieved)).cloned()
    }
    pub fn receive_remote(&mut self, review: &mut Value) {
        self.remote.clear();
        let Some(Value::Array(entries)) = review
            .as_object_mut()
            .and_then(|object| object.remove("remoteIconPreviews"))
        else {
            return;
        };
        let mut decoded_bytes = 0_usize;
        for entry in entries.into_iter().take(100) {
            let Some(api) = entry["apiName"]
                .as_str()
                .filter(|api| !api.is_empty() && api.len() <= 128)
            else {
                continue;
            };
            for (field, unachieved) in [("achieved", false), ("unachieved", true)] {
                let Some(encoded) = entry[field].as_str().filter(|text| text.len() <= 350_000)
                else {
                    continue;
                };
                let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
                    continue;
                };
                if bytes.is_empty() || bytes.len() > 256 * 1024 {
                    continue;
                }
                let Ok(mut reader) =
                    image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()
                else {
                    continue;
                };
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(1024);
                limits.max_image_height = Some(1024);
                limits.max_alloc = Some(8 * 1024 * 1024);
                reader.limits(limits);
                let Ok(image) = reader.decode() else {
                    continue;
                };
                let source_size = [image.width() as usize, image.height() as usize];
                let image = image
                    .thumbnail(image.width().min(128), image.height().min(128))
                    .to_rgba8();
                let width = image.width() as usize;
                let height = image.height() as usize;
                let rgba = image.into_raw();
                if rgba.len() > 8 * 1024 * 1024 - decoded_bytes {
                    continue;
                }
                decoded_bytes += rgba.len();
                self.remote.insert(
                    (api.to_owned(), unachieved),
                    Preview {
                        width,
                        height,
                        source_size,
                        rgba: rgba.into(),
                        fingerprint: format!("review:{api}:{field}:{:x}", Sha256::digest(&bytes)),
                    },
                );
            }
        }
    }
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
                achimeld_application::diagnostics::record(
                    "WARN",
                    "synchronization",
                    "Could not decode a frozen project icon preview",
                );
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
        if !self.frozen.contains_key(api) {
            return;
        }
        let frozen = Arc::clone(&self.frozen);
        let name = api.to_owned();
        let (sender, receiver) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let (achieved, unachieved) = &frozen[&name];
                let mut previews = Vec::new();
                for bytes in [achieved, unachieved] {
                    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                        .map_err(|error| error.to_string())?
                        .into_rgba8();
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
                        fingerprint: format!("frozen:{:x}", Sha256::digest(bytes)),
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
    use serde_json::json;

    #[test]
    fn remote_icon_bytes_are_bounded_session_previews_and_removed_from_review() {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([30, 90, 150, 255]),
        ));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(png.get_ref());
        let mut review = json!({"token":"review", "remoteIconPreviews":[
            {"apiName":"COIN", "achieved":encoded, "unachieved":encoded},
            {"apiName":"INVALID", "achieved":"not-base64", "unachieved":null}
        ]});
        let mut previews = IconPreviews::default();
        previews.receive_remote(&mut review);
        assert!(review.get("remoteIconPreviews").is_none());
        assert_eq!(review["token"], "review");
        let achieved = previews.remote("COIN", false).unwrap();
        assert_eq!([achieved.width, achieved.height], [2, 2]);
        assert_eq!(&achieved.rgba[..4], &[30, 90, 150, 255]);
        assert!(previews.remote("COIN", true).is_some());
        assert!(previews.remote("INVALID", false).is_none());
        previews.receive_remote(&mut json!({"token":"next"}));
        assert!(previews.remote("COIN", false).is_none());
    }
    #[test]
    fn exact_staged_pairs_decode_in_background_and_share_cached_pixels() {
        let png = |color| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::RgbaImage::from_pixel(256, 128, image::Rgba(color))
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            bytes.into_inner()
        };
        let frozen = Arc::new(BTreeMap::from([(
            "COIN".into(),
            (png([220, 120, 30, 255]), png([120, 120, 120, 255])),
        )]));
        let mut previews = IconPreviews {
            frozen: Arc::clone(&frozen),
            ..Default::default()
        };
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
        assert!(Arc::ptr_eq(&previews.frozen, &frozen));
    }
}
