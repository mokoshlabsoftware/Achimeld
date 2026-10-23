//! Bounded PSD/PSB raster extraction, using Adobe's public file format specification.
//! The saved composite is authoritative for blend modes and effects. Individual
//! raster layers retain transparency and document coordinates, as in the editor.
use image::{Rgba, RgbaImage};

const LIMIT: usize = 128 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;
pub struct Frame {
    pub name: String,
    pub pixels: RgbaImage,
}
struct Reader<'a> {
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let bytes = self.bytes.get(..n).ok_or("Truncated Photoshop artwork")?;
        self.bytes = &self.bytes[n..];
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| "Invalid Photoshop integer")?,
        ))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| "Invalid Photoshop integer")?,
        ))
    }
    fn length(&mut self, large: bool) -> Result<usize> {
        let n = if large {
            u64::from_be_bytes(
                self.take(8)?
                    .try_into()
                    .map_err(|_| "Invalid Photoshop length")?,
            )
        } else {
            u64::from(self.u32()?)
        };
        usize::try_from(n).map_err(|_| "Photoshop section is too large".into())
    }
    fn section(&mut self, large: bool) -> Result<Reader<'a>> {
        let n = self.length(large)?;
        Ok(Reader {
            bytes: self.take(n)?,
        })
    }
}
struct Layer {
    name: String,
    top: i32,
    left: i32,
    width: u32,
    height: u32,
    channels: Vec<(i16, usize)>,
}
fn dimensions(width: u32, height: u32) -> Result<usize> {
    let pixels = u64::from(width) * u64::from(height);
    if width > 16384 || height > 16384 || pixels * 4 > LIMIT as u64 {
        return Err("Photoshop artwork exceeds the 128 MiB raster limit".into());
    }
    Ok(pixels as usize)
}
pub fn read(bytes: &[u8], cancelled: &dyn Fn() -> bool) -> Result<Vec<Frame>> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Photoshop file exceeds 64 MiB".into());
    }
    let mut r = Reader { bytes };
    if r.take(4)? != b"8BPS" {
        return Err("Invalid Photoshop signature".into());
    }
    let large = match r.u16()? {
        1 => false,
        2 => true,
        _ => return Err("Unsupported Photoshop version".into()),
    };
    if r.take(6)? != [0; 6] {
        return Err("Invalid Photoshop header".into());
    }
    let channels = usize::from(r.u16()?);
    let height = r.u32()?;
    let width = r.u32()?;
    let pixels = dimensions(width, height)?;
    if pixels == 0 || !(1..=56).contains(&channels) {
        return Err("Invalid Photoshop dimensions or channels".into());
    }
    let depth = r.u16()?;
    if ![8, 16].contains(&depth) {
        return Err("Photoshop import supports 8-bit and 16-bit artwork. Export other bit depths to PNG first.".into());
    }
    let colors = match r.u16()? { 1 => 1, 3 => 3, _ => return Err("Photoshop import supports RGB and grayscale artwork. Convert other color modes to RGB first.".into()) };
    if channels < colors {
        return Err("Photoshop color channels are missing".into());
    }
    r.section(false)?;
    r.section(false)?;
    let mut masks = r.section(large)?;
    let mut layers = Vec::new();
    let mut transparent = channels > colors && masks.bytes.is_empty();
    let mut data = if masks.bytes.is_empty() {
        Reader { bytes: &[] }
    } else {
        masks.section(large)?
    };
    if !data.bytes.is_empty() {
        let signed = data.u16()? as i16;
        transparent = signed < 0;
        let count = signed.unsigned_abs() as usize;
        if count > 128 {
            return Err("Import at most 128 Photoshop layers".into());
        }
        for index in 0..count {
            let top = data.u32()? as i32;
            let left = data.u32()? as i32;
            let bottom = data.u32()? as i32;
            let right = data.u32()? as i32;
            let w = u32::try_from(i64::from(right) - i64::from(left))
                .map_err(|_| "Invalid Photoshop layer bounds")?;
            let h = u32::try_from(i64::from(bottom) - i64::from(top))
                .map_err(|_| "Invalid Photoshop layer bounds")?;
            dimensions(w, h)?;
            let count = data.u16()?;
            if count > 56 {
                return Err("Too many Photoshop layer channels".into());
            }
            let mut channel_info = Vec::new();
            for _ in 0..count {
                channel_info.push((data.u16()? as i16, data.length(large)?));
            }
            if data.take(4)? != b"8BIM" {
                return Err("Invalid Photoshop layer signature".into());
            }
            data.take(8)?; // blend key, opacity, clipping, flags, reserved
            let mut extra = data.section(false)?;
            extra.section(false)?;
            extra.section(false)?;
            let len = usize::from(extra.u8()?);
            let mut name = String::from_utf8_lossy(extra.take(len)?).into_owned();
            extra.take((4 - (len + 1) % 4) % 4)?;
            while extra.bytes.len() >= 12 {
                let signature = extra.take(4)?;
                let key = extra.take(4)?;
                if signature != b"8BIM" && signature != b"8B64" {
                    break;
                }
                let long = large
                    && [
                        b"LMsk", b"Lr16", b"Lr32", b"Layr", b"Mt16", b"Mt32", b"Mtrn", b"Alph",
                        b"FMsk", b"lnk2", b"FEid", b"FXid", b"PxSD",
                    ]
                    .contains(&key.try_into().map_err(|_| "Invalid Photoshop key")?);
                let mut block = extra.section(long)?;
                let block_len = block.bytes.len();
                if key == b"luni" {
                    let count = block.u32()? as usize;
                    if count > 4096 {
                        return Err("Photoshop layer name is too long".into());
                    }
                    let units = block
                        .take(count * 2)?
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| u16::from_be_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>();
                    name = String::from_utf16_lossy(&units);
                }
                if block_len % 2 != 0 && !extra.bytes.is_empty() {
                    extra.take(1)?;
                }
            }
            if name.is_empty() {
                name = format!("Layer {}", index + 1);
            }
            layers.push(Layer {
                name,
                top,
                left,
                width: w,
                height: h,
                channels: channel_info,
            });
        }
    }
    let mut frames = Vec::new();
    let mut total = pixels * 4;
    for layer in layers {
        if cancelled() {
            return Err("Artwork processing cancelled".into());
        }
        let n = dimensions(layer.width, layer.height)?;
        let mut rgba = RgbaImage::from_pixel(layer.width, layer.height, Rgba([0, 0, 0, 255]));
        let mut has_color = false;
        for (id, length) in layer.channels {
            let bytes = data.take(length)?;
            if n == 0 || (id != -1 && !(0..colors as i16).contains(&id)) {
                continue;
            }
            let plane = decode(
                bytes,
                layer.width as usize,
                layer.height as usize,
                1,
                depth,
                large,
            )?;
            has_color |= id >= 0;
            for (pixel, value) in rgba
                .pixels_mut()
                .zip(plane.chunks_exact(usize::from(depth / 8)))
            {
                if id == -1 {
                    pixel[3] = value[0];
                } else if colors == 1 {
                    pixel[0] = value[0];
                    pixel[1] = value[0];
                    pixel[2] = value[0];
                } else {
                    pixel[id as usize] = value[0];
                }
            }
        }
        if n == 0 || !has_color {
            continue;
        }
        total = total.saturating_add(pixels * 4);
        if total > LIMIT {
            return Err("Photoshop layer previews exceed the 128 MiB batch limit. Export selected layers to PNG first.".into());
        }
        let mut canvas = RgbaImage::new(width, height);
        image::imageops::overlay(
            &mut canvas,
            &rgba,
            i64::from(layer.left),
            i64::from(layer.top),
        );
        frames.push(Frame {
            name: layer.name,
            pixels: canvas,
        });
    }
    if cancelled() {
        return Err("Artwork processing cancelled".into());
    }
    let plane = decode(
        r.bytes,
        width as usize,
        height as usize,
        channels,
        depth,
        large,
    )?;
    let sample = usize::from(depth / 8);
    let mut composite = RgbaImage::new(width, height);
    for (index, pixel) in composite.pixels_mut().enumerate() {
        for channel in 0..3 {
            pixel[channel] =
                plane[((if colors == 1 { 0 } else { channel }) * pixels + index) * sample];
        }
        pixel[3] = if transparent && channels > colors {
            plane[(colors * pixels + index) * sample]
        } else {
            255
        };
    }
    frames.insert(
        0,
        Frame {
            name: "Entire image".into(),
            pixels: composite,
        },
    );
    Ok(frames)
}
fn decode(
    bytes: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    depth: u16,
    large: bool,
) -> Result<Vec<u8>> {
    let row = width
        .checked_mul(usize::from(depth / 8))
        .ok_or("Photoshop row is too large")?;
    let expected = row
        .checked_mul(height)
        .and_then(|n| n.checked_mul(channels))
        .filter(|n| *n <= LIMIT)
        .ok_or("Photoshop channels exceed 128 MiB")?;
    let mut r = Reader { bytes };
    let compression = r.u16()?;
    let mut output = Vec::with_capacity(expected);
    match compression {
        0 => output.extend_from_slice(r.take(expected)?),
        1 => {
            let mut lengths = Vec::with_capacity(height * channels);
            for _ in 0..height * channels {
                lengths.push(if large {
                    r.u32()? as usize
                } else {
                    usize::from(r.u16()?)
                });
            }
            for len in lengths {
                let mut line = Reader {
                    bytes: r.take(len)?,
                };
                let end = output.len() + row;
                while !line.bytes.is_empty() {
                    let code = line.u8()? as i8;
                    match code {
                        0..=127 => {
                            let count = code as usize + 1;
                            if output.len() + count > end {
                                return Err("Invalid Photoshop RLE row".into());
                            }
                            output.extend_from_slice(line.take(count)?);
                        }
                        -127..=-1 => {
                            let count = (1 - i16::from(code)) as usize;
                            if output.len() + count > end {
                                return Err("Invalid Photoshop RLE run".into());
                            }
                            let value = line.u8()?;
                            output.resize(output.len() + count, value);
                        }
                        _ => {}
                    }
                }
                if output.len() != end {
                    return Err("Truncated Photoshop RLE row".into());
                }
            }
        }
        2 | 3 => {
            let decoder = flate2::read::ZlibDecoder::new(r.bytes);
            let mut bounded = std::io::Read::take(decoder, expected as u64 + 1);
            std::io::Read::read_to_end(&mut bounded, &mut output).map_err(|e| e.to_string())?;
            if output.len() != expected {
                return Err("Invalid Photoshop ZIP channel length".into());
            }
            if compression == 3 {
                for line in output.chunks_exact_mut(row) {
                    if depth == 8 {
                        for i in 1..line.len() {
                            line[i] = line[i].wrapping_add(line[i - 1]);
                        }
                    } else {
                        for i in (2..line.len()).step_by(2) {
                            let prior = u16::from_be_bytes([line[i - 2], line[i - 1]]);
                            let value = u16::from_be_bytes([line[i], line[i + 1]])
                                .wrapping_add(prior)
                                .to_be_bytes();
                            line[i..i + 2].copy_from_slice(&value);
                        }
                    }
                }
            }
        }
        _ => return Err("Unsupported Photoshop compression".into()),
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn length(n: usize, large: bool) -> Vec<u8> {
        if large {
            (n as u64).to_be_bytes().to_vec()
        } else {
            (n as u32).to_be_bytes().to_vec()
        }
    }
    fn fixture(large: bool) -> Vec<u8> {
        let mut bytes = b"8BPS".to_vec();
        bytes.extend(if large { 2u16 } else { 1u16 }.to_be_bytes());
        bytes.extend([0; 6]);
        bytes.extend(4u16.to_be_bytes());
        bytes.extend(1u32.to_be_bytes());
        bytes.extend(2u32.to_be_bytes());
        bytes.extend(8u16.to_be_bytes());
        bytes.extend(3u16.to_be_bytes());
        bytes.extend([0; 8]); // color data and image resources
        let mut info = (-1i16).to_be_bytes().to_vec();
        for coordinate in [0i32, 1, 1, 2] {
            info.extend(coordinate.to_be_bytes());
        }
        info.extend(4u16.to_be_bytes());
        for id in [0i16, 1, 2, -1] {
            info.extend(id.to_be_bytes());
            info.extend(length(3, large));
        }
        info.extend(b"8BIMnorm");
        info.extend([255, 0, 0, 0]);
        let extra = [0, 0, 0, 0, 0, 0, 0, 0, 3, b'A', b'r', b't'];
        info.extend((extra.len() as u32).to_be_bytes());
        info.extend(extra);
        for value in [80, 120, 160, 128] {
            info.extend([0, 0, value]);
        }
        let mut mask = length(info.len(), large);
        mask.extend(info);
        mask.extend([0; 4]);
        bytes.extend(length(mask.len(), large));
        bytes.extend(mask);
        bytes.extend([0, 0, 10, 20, 30, 40, 50, 60, 70, 80]);
        bytes
    }
    #[test]
    fn psd_and_psb_preserve_composite_alpha_and_layer_document_offsets() {
        for large in [false, true] {
            let bytes = fixture(large);
            let frames = read(&bytes, &|| false).unwrap();
            assert_eq!(frames.len(), 2);
            assert_eq!(frames[1].name, "Art");
            assert_eq!(frames[0].pixels.get_pixel(0, 0).0, [10, 30, 50, 70]);
            assert_eq!(frames[0].pixels.get_pixel(1, 0).0, [20, 40, 60, 80]);
            assert_eq!(frames[1].pixels.get_pixel(0, 0).0, [0, 0, 0, 0]);
            assert_eq!(frames[1].pixels.get_pixel(1, 0).0, [80, 120, 160, 128]);
            for end in 0..bytes.len() {
                assert!(
                    read(&bytes[..end], &|| false).is_err(),
                    "Accepted truncation at {end}"
                );
            }
            assert!(read(&bytes, &|| true).is_err());
        }
    }
    #[test]
    fn photoshop_rle_zip_and_prediction_are_bounded_and_decode_exact_samples() {
        assert_eq!(
            decode(&[0, 1, 0, 4, 2, 10, 20, 30], 3, 1, 1, 8, false).unwrap(),
            [10, 20, 30]
        );
        assert!(decode(&[0, 1, 0, 2, 254, 9], 2, 1, 1, 8, false).is_err());
        assert!(decode(&[0, 1, 0, 2, 0, 9], 2, 1, 1, 8, false).is_err());
        for prediction in [false, true] {
            let mut zip =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            std::io::Write::write_all(
                &mut zip,
                if prediction {
                    &[10, 10, 10]
                } else {
                    &[10, 20, 30]
                },
            )
            .unwrap();
            let mut bytes = if prediction { 3u16 } else { 2u16 }.to_be_bytes().to_vec();
            bytes.extend(zip.finish().unwrap());
            assert_eq!(decode(&bytes, 3, 1, 1, 8, false).unwrap(), [10, 20, 30]);
            assert!(decode(&bytes, 2, 1, 1, 8, false).is_err());
        }
        let mut bytes = fixture(false);
        bytes[14..18].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(read(&bytes, &|| false).is_err());
    }
}
