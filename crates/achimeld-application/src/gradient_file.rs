//! Photoshop 6+ GRD v5: bounded ActionDescriptors, solid RGB stops and opacity.
//! See docs/gradient-explorer.md for format references and qualification limits.
use crate::{
    GradientPreset, GradientStop,
    gradient_map::{decode_color, encode_color},
    new_id,
};
use std::collections::BTreeMap;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_GRADIENTS: usize = 512;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug)]
enum Value {
    Object(String, BTreeMap<String, Value>),
    List(Vec<Value>),
    Text(String),
    Number(f64),
    Unit(String, f64),
    Enum(String, String),
    Other,
}
impl Value {
    fn object(&self) -> Result<(&str, &BTreeMap<String, Value>)> {
        if let Self::Object(class, fields) = self {
            Ok((class, fields))
        } else {
            Err("Expected a Photoshop descriptor".into())
        }
    }
    fn list(&self) -> Result<&[Value]> {
        if let Self::List(list) = self {
            Ok(list)
        } else {
            Err("Expected a Photoshop stop list".into())
        }
    }
    fn number(&self) -> Result<f64> {
        if let Self::Number(n) = self {
            Ok(*n)
        } else {
            Err("Expected a Photoshop number".into())
        }
    }
}
fn field<'a>(fields: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a Value> {
    fields
        .get(key)
        .ok_or_else(|| format!("Missing Photoshop gradient field: {key}"))
}
fn range(n: f64, min: f64, max: f64) -> Result<f64> {
    if n.is_finite() && (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err("Photoshop gradient contains an out-of-range value".into())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or("Invalid Photoshop data length")?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or("Truncated Photoshop gradient file")?;
        self.pos = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| "Invalid integer")?,
        ))
    }
    fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n > max {
            return Err("Photoshop gradient exceeds its item limit".into());
        }
        Ok(n)
    }
    fn id(&mut self) -> Result<String> {
        let n = self.count(1024)?;
        let bytes = self.take(if n == 0 { 4 } else { n })?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "Invalid Photoshop identifier".into())
    }
    fn text(&mut self) -> Result<String> {
        let n = self.count(4096)?;
        let units = self
            .take(n * 2)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units)
            .map(|s| s.trim_end_matches('\0').to_owned())
            .map_err(|_| "Invalid Photoshop Unicode name".into())
    }
    fn double(&mut self) -> Result<f64> {
        let n = f64::from_be_bytes(self.take(8)?.try_into().map_err(|_| "Invalid number")?);
        if !n.is_finite() {
            return Err("Non-finite Photoshop value".into());
        }
        Ok(n)
    }
    fn node(&mut self, depth: usize) -> Result<()> {
        self.nodes += 1;
        if depth > 16 || self.nodes > 400_000 {
            return Err("Photoshop descriptor exceeds its complexity limit".into());
        }
        Ok(())
    }
    fn object(&mut self, depth: usize) -> Result<Value> {
        self.node(depth)?;
        self.text()?;
        let class = self.id()?;
        let count = self.count(64)?;
        let mut fields = BTreeMap::new();
        for _ in 0..count {
            let key = self.id()?;
            let value = self.value(depth + 1)?;
            if fields.insert(key, value).is_some() {
                return Err("Duplicate Photoshop descriptor key".into());
            }
        }
        Ok(Value::Object(class, fields))
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        self.node(depth)?;
        Ok(match self.take(4)? {
            b"Objc" | b"GlbO" => self.object(depth + 1)?,
            b"VlLs" => {
                let n = self.count(1024)?;
                Value::List(
                    (0..n)
                        .map(|_| self.value(depth + 1))
                        .collect::<Result<Vec<_>>>()?,
                )
            }
            b"TEXT" => Value::Text(self.text()?),
            b"long" => Value::Number(self.u32()? as i32 as f64),
            b"doub" => Value::Number(self.double()?),
            b"UntF" => {
                let unit = String::from_utf8(self.take(4)?.to_vec())
                    .map_err(|_| "Invalid Photoshop units")?;
                Value::Unit(unit, self.double()?)
            }
            b"enum" => Value::Enum(self.id()?, self.id()?),
            b"bool" => {
                self.take(1)?;
                Value::Other
            }
            b"tdta" | b"alis" => {
                let n = self.count(MAX_BYTES)?;
                self.take(n)?;
                Value::Other
            }
            b"type" | b"GlbC" => {
                self.text()?;
                self.id()?;
                Value::Other
            }
            _ => return Err("Unsupported Photoshop descriptor type".into()),
        })
    }
}

#[derive(Clone)]
struct Stop {
    location: f64,
    midpoint: f64,
    color: [f64; 4],
}
fn location(fields: &BTreeMap<String, Value>) -> Result<(f64, f64)> {
    Ok((
        range(field(fields, "Lctn")?.number()?, 0.0, 4096.0)? / 4096.0,
        range(field(fields, "Mdpn")?.number()?, 0.0, 100.0)? / 100.0,
    ))
}
fn sample(stops: &[Stop], t: f64) -> [f64; 4] {
    let left = stops.iter().rfind(|s| s.location <= t).unwrap_or(&stops[0]);
    let right = stops.iter().find(|s| s.location > t).unwrap_or(left);
    let mut f = if right.location > left.location {
        ((t - left.location) / (right.location - left.location)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mid = left.midpoint.clamp(0.000001, 0.999999);
    f = if f <= mid {
        0.5 * f / mid
    } else {
        0.5 + 0.5 * (f - mid) / (1.0 - mid)
    };
    std::array::from_fn(|i| left.color[i] + (right.color[i] - left.color[i]) * f)
}
fn gradient(value: &Value) -> Result<GradientPreset> {
    let (_, wrapper) = value.object()?;
    let (_, fields) = field(wrapper, "Grad")?.object()?;
    if !matches!(field(fields, "GrdF")?, Value::Enum(kind, value) if kind == "GrdF" && value == "CstS")
    {
        return Err("Photoshop noise gradients are not supported; use a solid RGB gradient".into());
    }
    let name = match field(fields, "Nm  ")? {
        Value::Text(s) => s.clone(),
        _ => return Err("Invalid Photoshop gradient name".into()),
    };
    let mut colors = Vec::new();
    for value in field(fields, "Clrs")?.list()? {
        let (_, stop) = value.object()?;
        if !matches!(field(stop, "Type")?, Value::Enum(kind, value) if kind == "Clry" && value == "UsrS")
        {
            return Err(
                "Foreground/background gradients need fixed colors before importing".into(),
            );
        }
        let (class, color) = field(stop, "Clr ")?.object()?;
        if class != "RGBC" {
            return Err("Convert Photoshop gradient colors to RGB before importing".into());
        }
        let rgb = ["Rd  ", "Grn ", "Bl  "].map(|key| {
            field(color, key)
                .and_then(Value::number)
                .and_then(|n| range(n, 0.0, 255.0))
        });
        let [r, g, b] = rgb;
        let (location, midpoint) = location(stop)?;
        colors.push(Stop {
            location,
            midpoint,
            color: [r?, g?, b?, 255.0],
        });
    }
    let mut opacity = Vec::new();
    for value in field(fields, "Trns")?.list()? {
        let (_, stop) = value.object()?;
        let alpha = match field(stop, "Opct")? {
            Value::Unit(unit, n) if unit == "#Prc" => range(*n, 0.0, 100.0)? * 255.0 / 100.0,
            _ => return Err("Invalid Photoshop opacity units".into()),
        };
        let (location, midpoint) = location(stop)?;
        opacity.push(Stop {
            location,
            midpoint,
            color: [alpha; 4],
        });
    }
    if colors.is_empty() || colors.len() > 128 || opacity.is_empty() || opacity.len() > 128 {
        return Err("Photoshop gradients require 1–128 color and opacity stops".into());
    }
    colors.sort_by(|a, b| a.location.total_cmp(&b.location));
    opacity.sort_by(|a, b| a.location.total_cmp(&b.location));
    range(field(fields, "Intr")?.number()?, 0.0, 4096.0)?;
    // Union both tracks and midpoint knots, retaining both sides of hard edges.
    let mut positions = vec![0.0, 1.0];
    for track in [&colors, &opacity] {
        for stop in track {
            positions.push(stop.location);
        }
        for pair in track.windows(2) {
            if pair[0].location != pair[1].location && pair[0].midpoint != 0.5 {
                positions.push(
                    pair[0].location
                        + (pair[1].location - pair[0].location)
                            * pair[0].midpoint.clamp(0.000001, 0.999999),
                );
            }
        }
    }
    positions.sort_by(f64::total_cmp);
    positions.dedup();
    let positions: Vec<_> = positions
        .into_iter()
        .flat_map(|offset| {
            let hard = [&colors, &opacity]
                .iter()
                .any(|track| track.iter().filter(|s| s.location == offset).count() > 1);
            if hard {
                vec![(offset, true), (offset, false)]
            } else {
                vec![(offset, false)]
            }
        })
        .collect();
    if positions.len() > 128 {
        return Err("Combined Photoshop color and opacity tracks exceed 128 stops".into());
    }
    let at = |track: &[Stop], offset: f64, left: bool| {
        if left && let Some(stop) = track.iter().find(|s| s.location == offset) {
            stop.color
        } else {
            sample(track, offset)
        }
    };
    let preset = GradientPreset {
        id: new_id(),
        name,
        stops: positions
            .into_iter()
            .map(|(offset, left)| {
                let rgb = at(&colors, offset, left);
                GradientStop {
                    offset,
                    color_hex: encode_color([
                        rgb[0].round() as u8,
                        rgb[1].round() as u8,
                        rgb[2].round() as u8,
                        at(&opacity, offset, left)[0].round() as u8,
                    ]),
                }
            })
            .collect(),
    };
    preset.validate()?;
    Ok(preset)
}

pub fn decode(bytes: &[u8]) -> Result<Vec<GradientPreset>> {
    if bytes.len() > MAX_BYTES {
        return Err("Photoshop gradient file exceeds 8 MiB".into());
    }
    let mut reader = Reader {
        bytes,
        pos: 0,
        nodes: 0,
    };
    if reader.take(4)? != b"8BGR" || reader.take(2)? != [0, 5] || reader.u32()? != 16 {
        return Err("Use a Photoshop 6 or newer .grd gradient file (version 5)".into());
    }
    let root = reader.object(0)?;
    let (_, fields) = root.object()?;
    let list = field(fields, "GrdL")?.list()?;
    if list.is_empty() || list.len() > MAX_GRADIENTS {
        return Err("Import between 1 and 512 Photoshop gradients".into());
    }
    // Modern Photoshop may append preset-folder metadata; it is not application authority.
    if reader.pos != bytes.len() && !bytes[reader.pos..].starts_with(b"8BIMphry") {
        return Err("Unexpected trailing Photoshop gradient data".into());
    }
    list.iter().map(gradient).collect()
}

fn id(bytes: &mut Vec<u8>, name: &str) {
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(name.as_bytes());
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    let units: Vec<_> = value.encode_utf16().chain([0]).collect();
    bytes.extend_from_slice(&(units.len() as u32).to_be_bytes());
    for unit in units {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
}
fn object(bytes: &mut Vec<u8>, class: &str, count: u32) {
    text(bytes, "");
    id(bytes, class);
    bytes.extend_from_slice(&count.to_be_bytes());
}
fn key(bytes: &mut Vec<u8>, name: &str, kind: &[u8; 4]) {
    id(bytes, name);
    bytes.extend_from_slice(kind);
}
fn number(bytes: &mut Vec<u8>, name: &str, value: f64) {
    key(bytes, name, b"doub");
    bytes.extend_from_slice(&value.to_be_bytes());
}
fn integer(bytes: &mut Vec<u8>, name: &str, value: u32) {
    key(bytes, name, b"long");
    bytes.extend_from_slice(&value.to_be_bytes());
}
fn enumeration(bytes: &mut Vec<u8>, name: &str, kind: &str, value: &str) {
    key(bytes, name, b"enum");
    id(bytes, kind);
    id(bytes, value);
}

pub fn encode(gradients: &[GradientPreset]) -> Result<Vec<u8>> {
    if gradients.is_empty() || gradients.len() > MAX_GRADIENTS {
        return Err("Export between 1 and 512 gradients".into());
    }
    let mut bytes = b"8BGR\0\x05\0\0\0\x10".to_vec();
    object(&mut bytes, "null", 1);
    key(&mut bytes, "GrdL", b"VlLs");
    bytes.extend_from_slice(&(gradients.len() as u32).to_be_bytes());
    for preset in gradients {
        preset.validate()?;
        bytes.extend_from_slice(b"Objc");
        object(&mut bytes, "Grdn", 1);
        key(&mut bytes, "Grad", b"Objc");
        object(&mut bytes, "Grdn", 5);
        key(&mut bytes, "Nm  ", b"TEXT");
        text(&mut bytes, &preset.name);
        enumeration(&mut bytes, "GrdF", "GrdF", "CstS");
        number(&mut bytes, "Intr", 4096.0);
        let mut stops = preset.stops.clone();
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        for (list, class) in [("Clrs", "Clrt"), ("Trns", "TrnS")] {
            key(&mut bytes, list, b"VlLs");
            bytes.extend_from_slice(&(stops.len() as u32).to_be_bytes());
            for stop in &stops {
                let color = decode_color(&stop.color_hex).ok_or("Invalid gradient color")?;
                bytes.extend_from_slice(b"Objc");
                object(&mut bytes, class, if list == "Clrs" { 4 } else { 3 });
                if list == "Clrs" {
                    key(&mut bytes, "Clr ", b"Objc");
                    object(&mut bytes, "RGBC", 3);
                    for (name, value) in ["Rd  ", "Grn ", "Bl  "].into_iter().zip(color) {
                        number(&mut bytes, name, f64::from(value));
                    }
                    enumeration(&mut bytes, "Type", "Clry", "UsrS");
                } else {
                    key(&mut bytes, "Opct", b"UntF");
                    bytes.extend_from_slice(b"#Prc");
                    bytes.extend_from_slice(&(f64::from(color[3]) * 100.0 / 255.0).to_be_bytes());
                }
                integer(&mut bytes, "Lctn", (stop.offset * 4096.0).round() as u32);
                integer(&mut bytes, "Mdpn", 50);
            }
        }
    }
    if bytes.len() > MAX_BYTES {
        return Err("Export exceeds the Photoshop gradient file limit".into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "gradient_file_tests.rs"]
mod tests;
