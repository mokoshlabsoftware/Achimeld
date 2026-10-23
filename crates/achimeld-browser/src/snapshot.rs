//! Port of the normalized document reader, not a scraper for arbitrary portal HTML.
use achimeld_core::{
    Decimal, LANGUAGES, LocalizationField, ProjectEntityType, SetBy, StatType, SteamApiName,
    SteamAppId,
    canonical::{self, Value},
};
use roxmltree::Node;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const FIXTURE_REVISION: &str = "2026-09-sanitized-fixture";
pub const LIVE_REVISION: &str = "steamworks-configuration-json-v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntity {
    pub api_name: SteamApiName,
    pub steam_id: Option<String>,
    /// Normalized source attributes are preserved verbatim for the next planner increment.
    pub attributes: BTreeMap<String, String>,
    pub content_fingerprint: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteLocalization {
    pub entity_type: ProjectEntityType,
    pub api_name: SteamApiName,
    pub field: LocalizationField,
    pub values: BTreeMap<String, String>,
    pub content_fingerprint: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub app_id: SteamAppId,
    pub revision: String,
    pub portal_revision_fingerprint: String,
    pub supports_display_order: bool,
    pub stats: Vec<RemoteEntity>,
    pub achievements: Vec<RemoteEntity>,
    pub localizations: Vec<RemoteLocalization>,
    pub icons: Vec<RemoteEntity>,
    pub content_fingerprint: String,
}
type Result<T> = std::result::Result<T, &'static str>;
fn required<'a>(node: Node<'a, 'a>, name: &str) -> Result<&'a str> {
    node.attribute(name)
        .filter(|s| !s.is_empty())
        .ok_or("Required snapshot attribute missing")
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn optional_text(value: Option<&str>) -> Value {
    value.map(text).unwrap_or(Value::Null)
}
fn boolean(node: Node<'_, '_>, name: &str) -> Result<Value> {
    match required(node, name)?.to_ascii_lowercase().as_str() {
        "true" => Ok(Value::Boolean(true)),
        "false" => Ok(Value::Boolean(false)),
        _ => Err("Invalid snapshot Boolean"),
    }
}
fn number(node: Node<'_, '_>, name: &str) -> Result<Value> {
    match node.attribute(name).filter(|v| !v.trim().is_empty()) {
        None => Ok(Value::Null),
        Some(value) => Decimal::from_str_exact(value.trim())
            .map(Value::Decimal)
            .map_err(|_| "Invalid snapshot decimal"),
    }
}
fn fingerprint(fields: Vec<(String, Value)>) -> Result<String> {
    canonical::compute(fields)
        .map(|f| f.entity)
        .map_err(|_| "Invalid canonical fields")
}
fn enumeration(type_name: &'static str, name: &'static str) -> Value {
    Value::Enumeration { type_name, name }
}
fn entity(node: Node<'_, '_>, fingerprint: String) -> Result<RemoteEntity> {
    Ok(RemoteEntity {
        api_name: SteamApiName::new(required(node, "data-api-name")?)
            .map_err(|_| "Invalid snapshot API name")?,
        steam_id: node.attribute("data-steam-id").map(str::to_owned),
        attributes: node
            .attributes()
            .map(|a| (a.name().to_owned(), a.value().to_owned()))
            .collect(),
        content_fingerprint: fingerprint,
    })
}
fn display_order(node: Node<'_, '_>, supported: bool) -> Result<Value> {
    let order: i32 = required(node, "data-display-order")?
        .parse()
        .map_err(|_| "Invalid snapshot display order")?;
    Ok(if supported {
        Value::Integer(order.into())
    } else {
        Value::Null
    })
}
fn parse_stat(node: Node<'_, '_>, supported: bool) -> Result<RemoteEntity> {
    let api = SteamApiName::new(required(node, "data-api-name")?)
        .map_err(|_| "Invalid snapshot API name")?;
    let kind: StatType = required(node, "data-type")?
        .parse()
        .map_err(|_| "Invalid stat type")?;
    let set_by: SetBy = required(node, "data-set-by")?
        .parse()
        .map_err(|_| "Invalid stat authority")?;
    let fields = vec![
        ("aggregated".into(), boolean(node, "data-aggregated")?),
        ("apiName".into(), text(api.as_str())),
        (
            "averageRateWindowSeconds".into(),
            number(node, "data-average-window")?,
        ),
        ("defaultValue".into(), number(node, "data-default")?),
        ("displayOrder".into(), display_order(node, supported)?),
        (
            "incrementOnly".into(),
            boolean(node, "data-increment-only")?,
        ),
        ("maximum".into(), number(node, "data-maximum")?),
        ("maximumChange".into(), number(node, "data-maximum-change")?),
        ("minimum".into(), number(node, "data-minimum")?),
        (
            "setBy".into(),
            enumeration(SetBy::legacy_type(), set_by.legacy_name()),
        ),
        (
            "type".into(),
            enumeration(StatType::legacy_type(), kind.legacy_name()),
        ),
    ];
    entity(node, fingerprint(fields)?)
}
fn parse_achievement(node: Node<'_, '_>, supported: bool) -> Result<RemoteEntity> {
    let api = SteamApiName::new(required(node, "data-api-name")?)
        .map_err(|_| "Invalid snapshot API name")?;
    let set_by: SetBy = required(node, "data-set-by")?
        .parse()
        .map_err(|_| "Invalid achievement authority")?;
    let progress = node
        .attribute("data-progress-stat")
        .filter(|s| !s.is_empty())
        .map(SteamApiName::new)
        .transpose()
        .map_err(|_| "Invalid progress stat API name")?;
    let fields = vec![
        ("apiName".into(), text(api.as_str())),
        ("displayOrder".into(), display_order(node, supported)?),
        ("hidden".into(), boolean(node, "data-hidden")?),
        (
            "progressStatApiName".into(),
            optional_text(progress.as_ref().map(SteamApiName::as_str)),
        ),
        (
            "setBy".into(),
            enumeration(SetBy::legacy_type(), set_by.legacy_name()),
        ),
        ("unlockValue".into(), number(node, "data-unlock-value")?),
    ];
    entity(node, fingerprint(fields)?)
}
fn parse_localization(node: Node<'_, '_>) -> Result<RemoteLocalization> {
    let entity_type: ProjectEntityType = required(node, "data-entity-type")?
        .parse()
        .map_err(|_| "Invalid localization owner type")?;
    let field: LocalizationField = required(node, "data-field")?
        .parse()
        .map_err(|_| "Invalid localization field")?;
    let api_name = SteamApiName::new(required(node, "data-api-name")?)
        .map_err(|_| "Invalid localization API name")?;
    let mut values: BTreeMap<String, String> = LANGUAGES
        .into_iter()
        .map(|l| (l.into(), String::new()))
        .collect();
    let mut seen = BTreeSet::new();
    for span in node.children().filter(|n| n.has_tag_name("span")) {
        let language = required(span, "data-language")?;
        if !LANGUAGES.contains(&language) || !seen.insert(language.to_owned()) {
            return Err("Unsupported or duplicate localization language");
        }
        values.insert(
            language.into(),
            span.descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect::<String>(),
        );
    }
    if values["English"].trim().is_empty() {
        return Err("English localization is required");
    }
    let content_fingerprint =
        fingerprint(values.iter().map(|(k, v)| (k.clone(), text(v))).collect())?;
    Ok(RemoteLocalization {
        entity_type,
        api_name,
        field,
        values,
        content_fingerprint,
    })
}
fn parse_icon(node: Node<'_, '_>) -> Result<RemoteEntity> {
    // Preview URLs are metadata only; this parser never fetches them.
    for name in ["data-achieved-preview", "data-unachieved-preview"] {
        if let Some(url) = node.attribute(name).filter(|s| !s.trim().is_empty())
            && (!url.starts_with("https://")
                || url[8..].split('/').next().is_none_or(str::is_empty)
                || url.chars().any(char::is_whitespace))
        {
            return Err("Icon preview must be an absolute HTTPS URL");
        }
    }
    let hash = fingerprint(vec![
        (
            "achievedIdentity".into(),
            optional_text(node.attribute("data-achieved-identity")),
        ),
        (
            "unachievedIdentity".into(),
            optional_text(node.attribute("data-unachieved-identity")),
        ),
    ])?;
    entity(node, hash)
}
fn unique_entities(items: &[RemoteEntity]) -> Result<()> {
    let mut names = BTreeSet::new();
    for item in items {
        if !names.insert(item.api_name.as_str()) {
            return Err("Duplicate entity API name");
        }
    }
    Ok(())
}

pub fn parse(html: &str, app_id: SteamAppId) -> Result<Snapshot> {
    if html.len() > 16 * 1024 * 1024 {
        return Err("Normalized snapshot exceeds size limit");
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: 200_000,
        ..Default::default()
    };
    // The committed legacy fixture has the inert HTML doctype. Accept exactly that
    // declaration at the start; retain the parser's prohibition on all actual DTDs.
    let body = html.trim_start_matches('\u{feff}').trim_start();
    let body = body.strip_prefix("<!DOCTYPE html>").unwrap_or(body);
    let document = roxmltree::Document::parse_with_options(body, options)
        .map_err(|_| "Snapshot must be sanitized well-formed XML")?;
    let root = document.root_element();
    if !root.has_tag_name("html") || required(root, "data-steamworks-page")? != "stats-achievements"
    {
        return Err("Unexpected portal page");
    }
    if required(root, "data-appid")? != app_id.value().to_string() {
        return Err("Snapshot belongs to another Steam app");
    }
    let revision = required(root, "data-steamworks-revision")?;
    if revision != FIXTURE_REVISION && revision != LIVE_REVISION {
        return Err("Unsupported snapshot revision");
    }
    let supported = revision != LIVE_REVISION;
    let mut sections = BTreeMap::new();
    for id in ["stats", "achievements", "localizations", "icons"] {
        let matches: Vec<_> = root
            .descendants()
            .filter(|n| n.has_tag_name("section") && n.attribute("id") == Some(id))
            .collect();
        if matches.len() != 1 {
            return Err("Snapshot section missing or duplicated");
        }
        sections.insert(id, matches[0]);
    }
    let articles = |id| {
        sections[id]
            .children()
            .filter(|n| n.has_tag_name("article"))
    };
    let mut stats = articles("stats")
        .map(|n| parse_stat(n, supported))
        .collect::<Result<Vec<_>>>()?;
    let mut achievements = articles("achievements")
        .map(|n| parse_achievement(n, supported))
        .collect::<Result<Vec<_>>>()?;
    let mut localizations = articles("localizations")
        .map(parse_localization)
        .collect::<Result<Vec<_>>>()?;
    let mut icons = articles("icons")
        .map(parse_icon)
        .collect::<Result<Vec<_>>>()?;
    for items in [&stats, &achievements, &icons] {
        unique_entities(items)?;
    }
    let stat_names: BTreeSet<_> = stats.iter().map(|e| e.api_name.as_str()).collect();
    let achievement_names: BTreeSet<_> = achievements.iter().map(|e| e.api_name.as_str()).collect();
    for achievement in &achievements {
        if let Some(progress) = achievement
            .attributes
            .get("data-progress-stat")
            .filter(|s| !s.is_empty())
        {
            let api = SteamApiName::new(progress).map_err(|_| "Invalid progress link")?;
            if !stat_names.contains(api.as_str()) {
                return Err("Progress link references a missing stat");
            }
        }
    }
    let mut localization_keys = BTreeSet::new();
    for l in &localizations {
        if !localization_keys.insert((l.entity_type, l.api_name.as_str(), l.field)) {
            return Err("Duplicate localization field");
        }
        let valid = match (l.entity_type, l.field) {
            (ProjectEntityType::Stat, LocalizationField::StatDisplayName) => {
                stat_names.contains(l.api_name.as_str())
            }
            (
                ProjectEntityType::Achievement,
                LocalizationField::AchievementDisplayName
                | LocalizationField::AchievementDescription,
            ) => achievement_names.contains(l.api_name.as_str()),
            _ => false,
        };
        if !valid {
            return Err("Localization owner is missing or mismatched");
        }
    }
    if icons
        .iter()
        .any(|i| !achievement_names.contains(i.api_name.as_str()))
    {
        return Err("Icon owner is missing");
    }
    // Stable canonical indices are API-name sorted, irrespective of displayed row order.
    stats.sort_by(|a, b| a.api_name.cmp(&b.api_name));
    achievements.sort_by(|a, b| a.api_name.cmp(&b.api_name));
    localizations.sort_by(|a, b| {
        (a.entity_type, &a.api_name, a.field).cmp(&(b.entity_type, &b.api_name, b.field))
    });
    icons.sort_by(|a, b| a.api_name.cmp(&b.api_name));
    let portal_revision_fingerprint = canonical::value_hash(&text(revision));
    let mut fields = vec![
        ("appId".into(), Value::Integer(app_id.value().into())),
        ("portalRevision".into(), text(&portal_revision_fingerprint)),
        ("supportsDisplayOrder".into(), Value::Boolean(supported)),
    ];
    for (label, items) in [
        ("stat", &stats),
        ("achievement", &achievements),
        ("icon", &icons),
    ] {
        for (i, e) in items.iter().enumerate() {
            fields.push((
                format!("{label}:{i}:{}", e.api_name.as_str()),
                text(&e.content_fingerprint),
            ));
        }
    }
    for (i, l) in localizations.iter().enumerate() {
        fields.push((
            format!(
                "localization:{i}:{}:{}:{}",
                l.entity_type.legacy_name(),
                l.api_name.as_str(),
                l.field.legacy_name()
            ),
            text(&l.content_fingerprint),
        ));
    }
    let content_fingerprint = fingerprint(fields)?;
    Ok(Snapshot {
        app_id,
        revision: revision.into(),
        portal_revision_fingerprint,
        supports_display_order: supported,
        stats,
        achievements,
        localizations,
        icons,
        content_fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str =
        include_str!("../../../tests/fixtures/steamworks/normalized-2026-09.html");
    #[test]
    fn bounded_mutations_preserve_snapshot_identity_or_reject() {
        let id = SteamAppId::new(480).unwrap();
        for position in (0..FIXTURE.len()).step_by(11) {
            for replacement in [0, b'<', b'>', b'&', b'"', b'0', 0xff] {
                let mut bytes = FIXTURE.as_bytes().to_vec();
                bytes[position] = replacement;
                if let Ok(input) = std::str::from_utf8(&bytes)
                    && let Ok(snapshot) = parse(input, id)
                {
                    assert_eq!(snapshot.app_id, id);
                    assert_eq!(snapshot.revision, FIXTURE_REVISION);
                    assert_eq!(snapshot.content_fingerprint.len(), 64);
                }
            }
        }
    }
    #[test]
    fn reads_complete_fixture_and_preserves_languages_and_links() {
        let s = parse(FIXTURE, SteamAppId::new(480).unwrap()).unwrap();
        assert_eq!(
            (
                s.stats.len(),
                s.achievements.len(),
                s.localizations.len(),
                s.icons.len()
            ),
            (1, 1, 3, 1)
        );
        assert_eq!(s.localizations[0].values["Polish"], "Pierwsze kroki");
        assert_eq!(s.localizations[0].values["Japanese"], "");
        assert_eq!(
            s.achievements[0].attributes["data-progress-stat"],
            "STAT_SCORE"
        );
        assert!(s.supports_display_order);
    }
    #[test]
    fn unsafe_incomplete_wrong_app_and_unknown_structures_stop() {
        let id = SteamAppId::new(480).unwrap();
        assert!(parse(FIXTURE, SteamAppId::new(481).unwrap()).is_err());
        assert!(
            parse(
                &FIXTURE.replace(
                    "<!DOCTYPE html>",
                    "<!DOCTYPE html [<!ENTITY injected 'bad'>]>"
                ),
                id
            )
            .is_err()
        );
        for (old, new) in [
            (FIXTURE_REVISION, "future"),
            ("id=\"stats\"", "id=\"missing\""),
            (
                "data-progress-stat=\"STAT_SCORE\"",
                "data-progress-stat=\"MISSING\"",
            ),
            ("data-type=\"Int\"", "data-type=\"Unknown\""),
        ] {
            assert!(parse(&FIXTURE.replace(old, new), id).is_err());
        }
    }
    #[test]
    fn live_revision_does_not_fingerprint_unavailable_order() {
        let live = FIXTURE.replace(FIXTURE_REVISION, LIVE_REVISION);
        let first = parse(&live, SteamAppId::new(480).unwrap()).unwrap();
        let second = parse(
            &live.replace("data-display-order=\"0\"", "data-display-order=\"999\""),
            SteamAppId::new(480).unwrap(),
        )
        .unwrap();
        assert!(!first.supports_display_order);
        assert_eq!(first.content_fingerprint, second.content_fingerprint);
    }
}
