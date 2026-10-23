//! Host-only Windows font discovery and bounded outline-font loading.
//! Names are passed to GDI, never interpreted as filesystem paths.
use std::collections::BTreeSet;
use std::sync::{Once, OnceLock};
use windows_sys::Win32::Graphics::Gdi::*;

const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;
static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();
static DISCOVERY: Once = Once::new();

pub(crate) fn prepare(ctx: eframe::egui::Context) {
    DISCOVERY.call_once(|| {
        let repaint = ctx.clone();
        if std::thread::Builder::new()
            .name("system-fonts".into())
            .spawn(move || {
                let _ = FAMILIES.set(enumerate());
                repaint.request_repaint();
            })
            .is_err()
        {
            let _ = FAMILIES.set(Vec::new());
            ctx.request_repaint();
        }
    });
}

pub(crate) fn families() -> Option<&'static [String]> {
    FAMILIES.get().map(Vec::as_slice)
}

struct DeviceContext(HDC);
impl DeviceContext {
    fn new() -> Option<Self> {
        // SAFETY: A null source creates a memory DC owned by this guard/thread.
        let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        (!dc.is_null()).then_some(Self(dc))
    }
}
impl Drop for DeviceContext {
    fn drop(&mut self) {
        // SAFETY: This guard owns the memory DC; selected font guards drop first.
        unsafe { DeleteDC(self.0) };
    }
}

struct SelectedFont<'dc> {
    dc: &'dc DeviceContext,
    font: HFONT,
    previous: HGDIOBJ,
}
impl<'dc> SelectedFont<'dc> {
    fn new(dc: &'dc DeviceContext, description: &LOGFONTW) -> Option<Self> {
        // SAFETY: The initialized description is valid for this synchronous call.
        let font = unsafe { CreateFontIndirectW(description) };
        if font.is_null() {
            return None;
        }
        // SAFETY: Both objects are live, locally owned GDI handles.
        let previous = unsafe { SelectObject(dc.0, font) };
        if previous.is_null() || previous as isize == -1 {
            // SAFETY: The font was not selected and is owned here.
            unsafe { DeleteObject(font) };
            return None;
        }
        Some(Self { dc, font, previous })
    }
}
impl Drop for SelectedFont<'_> {
    fn drop(&mut self) {
        // SAFETY: The DC outlives this guard. Restore before deleting our font.
        unsafe {
            SelectObject(self.dc.0, self.previous);
            DeleteObject(self.font);
        }
    }
}

fn enumerate() -> Vec<String> {
    let Some(dc) = DeviceContext::new() else {
        return Vec::new();
    };
    let description = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    let mut families = BTreeSet::<String>::new();
    // SAFETY: GDI calls collect synchronously with valid LOGFONTW data. The unique
    // mutable set remains alive until enumeration returns; no pointer is retained.
    unsafe {
        EnumFontFamiliesExW(
            dc.0,
            &description,
            Some(collect),
            &mut families as *mut BTreeSet<String> as isize,
            0,
        );
    }
    // GDI's enumeration names have a 31-unit limit. Resolve the full selected
    // names, and offer only faces whose tables fit the renderer's byte limit.
    // Discovery inspects directories, never decodes every font's full contents.
    let mut families: Vec<_> = families
        .into_iter()
        .filter_map(|family| {
            with_font(&family, |dc, actual| {
                let short =
                    String::from_utf16_lossy(&actual.encode_utf16().take(31).collect::<Vec<_>>());
                if actual.to_lowercase() != family.to_lowercase()
                    && short.to_lowercase() != family.to_lowercase()
                {
                    return None;
                }
                font_directory(dc)?;
                Some(actual.to_owned())
            })
        })
        .collect();
    families.sort_by_cached_key(|name| name.to_lowercase());
    families.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
    families
}

// SAFETY: Called only by EnumFontFamiliesExW above; pointers have the lifetimes
// documented at that call. No pointer escapes and no user code is invoked.
unsafe extern "system" fn collect(
    font: *const LOGFONTW,
    _: *const TEXTMETRICW,
    kind: u32,
    data: isize,
) -> i32 {
    if font.is_null() || data == 0 || kind & TRUETYPE_FONTTYPE == 0 {
        return 1;
    }
    // SAFETY: The callback contract supplies a readable LOGFONTW for this call.
    let name = unsafe { &(*font).lfFaceName };
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    let name = String::from_utf16_lossy(&name[..end]);
    // SAFETY: data is the exclusively borrowed set from enumerate above.
    let families = unsafe { &mut *(data as *mut BTreeSet<String>) };
    if !name.is_empty() && !name.starts_with('@') {
        families.insert(name);
    }
    i32::from(families.len() < 4096)
}

pub(crate) fn load(family: &str) -> Option<Vec<u8>> {
    with_font(family, |dc, actual| {
        // Reject Windows' silent substitution for unknown recipe fonts.
        if actual.to_lowercase() != family.to_lowercase() {
            return None;
        }
        standalone_font(dc)
    })
}

fn with_font<T>(
    family: &str,
    use_font: impl FnOnce(&DeviceContext, &str) -> Option<T>,
) -> Option<T> {
    if family.is_empty() || family.contains('\0') || family.encode_utf16().count() > 127 {
        return None;
    }
    let name: Vec<_> = family.encode_utf16().take(31).collect();
    let mut description = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        lfOutPrecision: OUT_TT_ONLY_PRECIS,
        // Preserve the original icon renderer's Segoe UI semibold appearance.
        lfWeight: if family.eq_ignore_ascii_case("Segoe UI") {
            FW_SEMIBOLD
        } else {
            FW_NORMAL
        } as i32,
        ..Default::default()
    };
    description.lfFaceName[..name.len()].copy_from_slice(&name);
    let dc = DeviceContext::new()?;
    let _font = SelectedFont::new(&dc, &description)?;
    let mut actual = [0u16; 128];
    // SAFETY: The DC has a selected font and actual is writable for 128 UTF-16 units.
    if unsafe { GetTextFaceW(dc.0, actual.len() as i32, actual.as_mut_ptr()) } <= 0 {
        return None;
    }
    let end = actual.iter().position(|c| *c == 0)?;
    use_font(&dc, &String::from_utf16_lossy(&actual[..end]))
}

fn font_data(dc: &DeviceContext, tag: u32, bytes: &mut [u8]) -> Option<()> {
    // SAFETY: The live DC has a selected font; the buffer and size match exactly.
    let read = unsafe { GetFontData(dc.0, tag, 0, bytes.as_mut_ptr().cast(), bytes.len() as u32) };
    (read as usize == bytes.len()).then_some(())
}

fn font_directory(dc: &DeviceContext) -> Option<Vec<u8>> {
    let mut header = [0u8; 12];
    font_data(dc, 0, &mut header)?;
    if !matches!(&header[..4], b"\0\x01\0\0" | b"OTTO" | b"true") {
        return None;
    }
    let tables = u16::from_be_bytes([header[4], header[5]]) as usize;
    if !(1..=256).contains(&tables) {
        return None;
    }
    let mut bytes = vec![0; 12 + tables * 16];
    font_data(dc, 0, &mut bytes)?;
    let mut total = bytes.len();
    for table in bytes[12..].as_chunks::<16>().0 {
        let length = u32::from_be_bytes(table[12..16].try_into().ok()?) as usize;
        total = ((total + 3) & !3).checked_add(length)?;
        if total > MAX_FONT_BYTES {
            return None;
        }
    }
    Some(bytes)
}

fn standalone_font(dc: &DeviceContext) -> Option<Vec<u8>> {
    let mut bytes = font_directory(dc)?;
    let tables = (bytes.len() - 12) / 16;
    // Collection faces use offsets in the original TTC file. Rebuild the SFNT
    // directory from GDI's selected-face tables so Cambria/CJK collections work
    // without exposing font file paths or parsing a different collection face.
    for index in 0..tables {
        let record = 12 + index * 16;
        let tag = u32::from_le_bytes(bytes[record..record + 4].try_into().ok()?);
        let length = u32::from_be_bytes(bytes[record + 12..record + 16].try_into().ok()?) as usize;
        let offset = (bytes.len() + 3) & !3;
        let end = offset.checked_add(length)?;
        if end > MAX_FONT_BYTES {
            return None;
        }
        bytes.resize(end, 0);
        if length > 0 {
            font_data(dc, tag, &mut bytes[offset..end])?;
        }
        bytes[record + 8..record + 12].copy_from_slice(&(offset as u32).to_be_bytes());
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use achimeld_imaging::AssetSource;

    #[test]
    fn system_font_catalog_is_sorted_unique_and_loads_installed_outline_faces() {
        let start = std::time::Instant::now();
        let families = enumerate();
        eprintln!(
            "Discovered {} system font families in {:?}",
            families.len(),
            start.elapsed()
        );
        assert!(families.iter().any(|name| name == "Segoe UI"));
        assert!(families.len() > 8);
        assert!(
            families
                .windows(2)
                .all(|pair| pair[0].to_lowercase() < pair[1].to_lowercase())
        );
        for family in &families {
            assert!(!family.starts_with('@'));
            let bytes =
                load(family).unwrap_or_else(|| panic!("Cannot load installed font {family}"));
            assert!(bytes.len() <= MAX_FONT_BYTES);
            achimeld_imaging::Font::from_bytes(bytes)
                .unwrap_or_else(|error| panic!("Cannot parse {family}: {error}"));
        }
    }

    #[test]
    fn system_font_loader_rejects_missing_names_paths_and_embedded_nuls() {
        for name in [
            "",
            "Achimeld nonexistent font",
            "C:\\Windows\\Fonts\\arial.ttf",
            "Arial\0suffix",
            "This name exceeds the GDI face name length limit",
        ] {
            assert!(
                load(name).is_none(),
                "Unexpected font substitution for {name:?}"
            );
        }
    }

    #[test]
    fn system_font_selection_renders_distinct_faces_including_collections() {
        use achimeld_application::{Command, Editor, EntityKind, new_id};
        use serde_json::json;
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let mut assets = crate::desktop::Files::new();
        let mut results = Vec::new();
        for family in ["Segoe UI", "Arial", "Cambria", "Cambria Math"] {
            // Cambria and Cambria Math share a TTC, but use different face tables.
            assert!(assets.font(family).is_some(), "Missing {family}");
            entity["icon"] = json!({"layers":[{"id":new_id(),"kind":"Text","text":{
                "content":"Aa Żółć 123", "fontFamily":family, "fontSize":72,
                "colorHex":"#FFFFFFFF", "alignment":"Center"
            }}]});
            editor
                .execute(
                    "Font",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.clone(),
                        value: entity.clone(),
                    },
                    None,
                )
                .unwrap();
            let image =
                achimeld_imaging::render(editor.document(), &id, Some((256, 128)), &mut assets)
                    .unwrap()
                    .achieved;
            assert!(image.pixels().any(|pixel| pixel[3] > 0), "Blank {family}");
            results.push(image);
        }
        assert_ne!(results[0], results[1]);
        assert_ne!(results[1], results[2]);
        assert_ne!(results[2], results[3]);
        assert!(assets.font("ARIAL").is_some());
    }
}
