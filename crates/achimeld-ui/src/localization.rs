use std::collections::BTreeMap;
use std::sync::LazyLock;
const LANGUAGE: &str = "achimeld-interface-language";
fn key(text: &str) -> String {
    text.trim().trim_end_matches(['.', '…']).to_lowercase()
}
static CATALOG: LazyLock<BTreeMap<String, Vec<String>>> = LazyLock::new(|| {
    let records = achimeld_core::csv::records(include_str!("../assets/strings.csv"))
        .expect("validated embedded interface CSV");
    let mut result = BTreeMap::new();
    for row in records.into_iter().skip(1) {
        if row.len() != achimeld_core::INTERFACE_LANGUAGES.len() + 2 {
            continue;
        }
        result.insert(row[0].clone(), row.clone());
        result.entry(key(&row[2])).or_insert(row);
    }
    result
});
pub fn language(ctx: &egui::Context, language: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(LANGUAGE), language.to_owned()));
}
pub fn text(ctx: &egui::Context, source: &str) -> String {
    let language = ctx
        .data(|data| data.get_temp::<String>(egui::Id::new(LANGUAGE)))
        .unwrap_or_else(|| "English".into());
    let index = achimeld_core::INTERFACE_LANGUAGES
        .iter()
        .position(|l| *l == language)
        .unwrap_or(0)
        + 2;
    CATALOG
        .get(source)
        .or_else(|| CATALOG.get(&key(source)))
        .map(|row| {
            if row[index].is_empty() {
                row[2].clone()
            } else {
                row[index].clone()
            }
        })
        .unwrap_or_else(|| source.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interface_language_uses_the_reference_catalog_and_english_fallback() {
        let ctx = egui::Context::default();
        language(&ctx, "Polish");
        assert_eq!(text(&ctx, "File"), "Plik");
        assert_eq!(text(&ctx, "FIOPEN"), "Otwórz");
        assert_eq!(
            text(&ctx, "A new untranslated label"),
            "A new untranslated label"
        );
    }

    #[test]
    fn expanded_interface_catalog_preserves_language_columns_and_fallback() {
        let records = achimeld_core::csv::records(include_str!("../assets/strings.csv")).unwrap();
        assert_eq!(&records[0][2..], achimeld_core::INTERFACE_LANGUAGES);
        assert!(records.iter().all(|row| row.len() == records[0].len()));
        let ctx = egui::Context::default();
        for (name, file) in [
            ("German", "Datei"),
            ("Spanish", "Archivo"),
            ("Italian", "File"),
            ("Portuguese", "Ficheiro"),
            ("Dutch", "Bestand"),
            ("Russian", "Файл"),
            ("Ukrainian", "Файл"),
        ] {
            language(&ctx, name);
            assert_eq!(text(&ctx, "FIMENU"), file);
            assert_eq!(text(&ctx, "An unknown label"), "An unknown label");
            let preferences = achimeld_application::Preferences {
                language: name.into(),
                ..Default::default()
            };
            assert!(preferences.validate().is_ok());
        }
        assert_eq!(achimeld_core::LANGUAGES.len(), 7);
    }
}
