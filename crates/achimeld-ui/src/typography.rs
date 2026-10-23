use egui::epaint::text::{HintingTarget, SmoothHinting};
use egui::{Context, FontData, FontDefinitions, FontFamily};

pub const BOLD_FAMILY: &str = "achimeld-ui-bold";
const FONT_STATE: &str = "achimeld-ui-fonts";
const NOTO_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSansCJK-Regular.ttc");
const NOTO_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansCJK-Bold.ttc");
const CJK_REGULAR: &str = "Noto Sans CJK";
const CJK_BOLD: &str = "Noto Sans CJK Bold";
const CJK_MONOSPACE: &str = "Noto Sans Mono CJK";

#[derive(Clone)]
struct FontState {
    definitions: std::sync::Arc<FontDefinitions>,
    cjk_index: u32,
}

// The collections share outline data across regional faces. Select the cmap
// appropriate to the interface language, including localized Han forms.
fn cjk_index(language: &str) -> u32 {
    match language {
        "Japanese" => 0,
        "Korean" => 1,
        "ChineseTraditional" => 3,
        _ => 2, // Simplified Chinese; full CJK coverage in other UI languages too.
    }
}

fn install_cjk(fonts: &mut FontDefinitions, index: u32) {
    for (name, bytes, face) in [
        (CJK_REGULAR, NOTO_REGULAR, index),
        (CJK_BOLD, NOTO_BOLD, index),
        (CJK_MONOSPACE, NOTO_REGULAR, index + 5),
    ] {
        let mut data = FontData::from_static(bytes);
        data.index = face;
        fonts.font_data.insert(name.into(), data.into());
    }
    for (family, fallback) in [
        (FontFamily::Proportional, CJK_REGULAR),
        (FontFamily::Name(BOLD_FAMILY.into()), CJK_BOLD),
        (FontFamily::Monospace, CJK_MONOSPACE),
    ] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(fallback.into());
    }
}

/// Windows faces are supplied by the host; Noto CJK fallbacks are embedded.
/// Presentation never reads font paths or requires installed CJK fonts.
pub fn configure_fonts(ctx: &Context, windows_faces: Option<[Vec<u8>; 2]>) {
    let mut fonts = FontDefinitions::default();
    if let Some([regular, bold]) = windows_faces {
        fonts
            .font_data
            .insert("Segoe UI".into(), FontData::from_owned(regular).into());
        fonts
            .font_data
            .insert("Segoe UI Bold".into(), FontData::from_owned(bold).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "Segoe UI".into());
        let mut bold_family = fonts.families[&FontFamily::Proportional].clone();
        bold_family.insert(0, "Segoe UI Bold".into());
        fonts
            .families
            .insert(FontFamily::Name(BOLD_FAMILY.into()), bold_family);
    } else {
        fonts.families.insert(
            FontFamily::Name(BOLD_FAMILY.into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
    }
    let index = cjk_index("English");
    install_cjk(&mut fonts, index);
    for font in fonts.font_data.values_mut() {
        let tweak = &mut std::sync::Arc::make_mut(font).tweak;
        tweak.hinting = Some(true);
        // Keep fractional advances and fit vertically. Horizontal grid fitting
        // would discard detail before the RGB coverage rasterizer sees it.
        tweak.subpixel_binning = Some(true);
        tweak.hinting_target = HintingTarget::Smooth(SmoothHinting {
            light: true,
            symmetric_rendering: true,
            preserve_linear_metrics: true,
        });
    }
    ctx.set_fonts(fonts.clone());
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(FONT_STATE),
            FontState {
                definitions: fonts.into(),
                cjk_index: index,
            },
        );
    });
}

pub(crate) fn ensure_fonts(ctx: &Context, language: &str) {
    let id = egui::Id::new(FONT_STATE);
    if !ctx.data(|data| data.get_temp::<FontState>(id).is_some()) {
        configure_fonts(ctx, None);
    }
    let index = cjk_index(language);
    let changed = ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_insert_with::<FontState>(id, || unreachable!());
        if state.cjk_index == index {
            return None;
        }
        let definitions = std::sync::Arc::make_mut(&mut state.definitions);
        for (name, face) in [
            (CJK_REGULAR, index),
            (CJK_BOLD, index),
            (CJK_MONOSPACE, index + 5),
        ] {
            std::sync::Arc::make_mut(definitions.font_data.get_mut(name).unwrap()).index = face;
        }
        state.cjk_index = index;
        Some(definitions.clone())
    });
    if let Some(fonts) = changed {
        // egui applies changed font definitions at the next pass.
        ctx.set_fonts(fonts);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, FontId};

    const SAMPLES: [(&str, &str); 4] = [
        ("Korean", "한국어 업적 통계 설정 한글"),
        ("ChineseSimplified", "简体中文 成就 统计 设置 骨 直"),
        ("ChineseTraditional", "繁體中文 成就 統計 設定 骨 直"),
        ("Japanese", "日本語 実績 統計 設定 ひらがな カタカナ 骨 直"),
    ];

    #[test]
    fn bundled_noto_covers_cjk_catalogs_and_mixed_text_in_every_ui_family() {
        let records = achimeld_core::csv::records(include_str!("../assets/strings.csv")).unwrap();
        for with_windows_fonts in [false, true] {
            let ctx = Context::default();
            if with_windows_fonts {
                crate::tests::configure_test_fonts(&ctx);
            } else {
                configure_fonts(&ctx, None);
            }
            for (language, _) in SAMPLES {
                ensure_fonts(&ctx, language);
                let column = achimeld_core::LANGUAGES
                    .iter()
                    .position(|entry| *entry == language)
                    .unwrap()
                    + 2;
                let mut characters = std::collections::BTreeSet::new();
                for row in records.iter().skip(1).filter(|row| row.len() == 9) {
                    characters.extend(
                        row[column]
                            .chars()
                            .filter(|c| !c.is_control() && !c.is_whitespace()),
                    );
                }
                // Localized project text also needs coverage in any UI language.
                for (_, sample) in SAMPLES {
                    characters.extend(sample.chars().filter(|c| !c.is_whitespace()));
                }
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    ui.fonts_mut(|fonts| {
                        assert_eq!(fonts.definitions().font_data[CJK_REGULAR].index, cjk_index(language));
                        for family in [
                            FontFamily::Proportional,
                            FontFamily::Name(BOLD_FAMILY.into()),
                            FontFamily::Monospace,
                        ] {
                            let font = FontId::new(18.0, family);
                            {
                                // has_glyph compares face identities and reports false
                                // for valid glyphs sharing the replacement glyph's face.
                                // Inspect actual cmap coverage instead.
                                let mut family = fonts.fonts.font(&font.family);
                                let available = family.characters();
                                for &c in &characters {
                                    assert!(
                                        available.contains_key(&c),
                                        "Missing {c:?} (U+{:04X}) in {language}/{:?}, Windows={with_windows_fonts}",
                                        c as u32,
                                        font.family,
                                    );
                                }
                            }
                            for (_, sample) in SAMPLES {
                                let galley = fonts.layout_no_wrap(
                                    sample.into(), font.clone(), Color32::WHITE,
                                );
                                assert!(galley.size().x > 0.0 && galley.size().y > 0.0);
                            }
                        }
                    });
                });
                assert!(
                    !output.textures_delta.set.is_empty(),
                    "No glyph atlas for {language}"
                );
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn language_switch_selects_regional_noto_faces_and_reuses_font_bytes() {
        let ctx = Context::default();
        crate::tests::configure_test_fonts(&ctx);
        let initial = ctx
            .data(|data| data.get_temp::<FontState>(egui::Id::new(FONT_STATE)))
            .unwrap();
        for (language, index) in [
            ("Korean", 1),
            ("ChineseTraditional", 3),
            ("Japanese", 0),
            ("ChineseSimplified", 2),
            ("English", 2),
            ("Polish", 2),
            ("Japanese", 0),
        ] {
            ensure_fonts(&ctx, language);
            let state = ctx
                .data(|data| data.get_temp::<FontState>(egui::Id::new(FONT_STATE)))
                .unwrap();
            for (name, expected) in [
                (CJK_REGULAR, index),
                (CJK_BOLD, index),
                (CJK_MONOSPACE, index + 5),
            ] {
                let face = &state.definitions.font_data[name];
                assert_eq!(face.index, expected, "{language}/{name}");
                assert_eq!(
                    face.font.as_ptr(),
                    initial.definitions.font_data[name].font.as_ptr()
                );
                assert!(matches!(face.font, std::borrow::Cow::Borrowed(_)));
            }
            if let Some(original) = initial.definitions.font_data.get("Segoe UI") {
                assert!(std::sync::Arc::ptr_eq(
                    original,
                    &state.definitions.font_data["Segoe UI"]
                ));
                assert_eq!(
                    state.definitions.families[&FontFamily::Proportional][0],
                    "Segoe UI"
                );
            }
            // Reapplying the language each frame must keep the same definitions.
            ensure_fonts(&ctx, language);
            let repeated = ctx
                .data(|data| data.get_temp::<FontState>(egui::Id::new(FONT_STATE)))
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(
                &state.definitions,
                &repeated.definitions
            ));
        }
    }
}
