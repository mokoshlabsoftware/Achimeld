//! A normalized configuration is accepted only from the exact Steam partner app URL.
use crate::{
    protocol::ManagedPageDocument,
    snapshot::{self, Snapshot},
};
use achimeld_core::SteamAppId;

pub fn validate_configuration_url(uri: &str, app_id: SteamAppId) -> Result<(), &'static str> {
    // The local worker normalizer emits this one canonical URL. Accepting fewer
    // routes here is intentional: login, publication and arbitrary navigation cannot
    // masquerade as a readConfiguration result. No URL decoding or substring matching.
    let expected = format!(
        "https://partner.steamgames.com/apps/achievements/{}",
        app_id.value()
    );
    if uri != expected {
        return Err("Configuration document did not match the approved Steam origin and App ID");
    }
    Ok(())
}
pub fn parse_document(
    document: &ManagedPageDocument,
    app_id: SteamAppId,
) -> Result<Snapshot, &'static str> {
    validate_configuration_url(&document.uri, app_id)?;
    let content = format!("{}\n{}", document.title, document.html).to_ascii_lowercase();
    for marker in [
        "steam guard",
        "auth challenge",
        "captcha",
        "two-factor",
        "name=\"password\"",
        "type=\"password\"",
        "sign in",
        "login_form",
        "permission denied",
        "access denied",
        "not authorized",
        "insufficient privilege",
    ] {
        if content.contains(marker) {
            return Err("Steam configuration requires manual authentication or permission review");
        }
    }
    snapshot::parse(&document.html, app_id)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_spoofed_origin_app_redirect_and_publication_urls() {
        let id = SteamAppId::new(480).unwrap();
        assert!(
            validate_configuration_url("https://partner.steamgames.com/apps/achievements/480", id)
                .is_ok()
        );
        for uri in [
            "http://partner.steamgames.com/apps/achievements/480",
            "https://partner.steamgames.com.evil/apps/achievements/480",
            "https://user@partner.steamgames.com/apps/achievements/480",
            "https://partner.steamgames.com/apps/achievements/481",
            "https://partner.steamgames.com/apps/publishing/480",
            "https://partner.steamgames.com/login/",
            "https://partner.steamgames.com/apps/achievements/480?appid=481",
        ] {
            assert!(validate_configuration_url(uri, id).is_err());
        }
    }
    #[test]
    fn rejects_authentication_and_missing_sections_in_worker_documents() {
        let id = SteamAppId::new(480).unwrap();
        let mut document = ManagedPageDocument {
            uri: "https://partner.steamgames.com/apps/achievements/480".into(),
            title: "Stats".into(),
            html: include_str!("../../../tests/fixtures/steamworks/normalized-2026-09.html").into(),
        };
        assert!(parse_document(&document, id).is_ok());
        document.title = "Steam Guard".into();
        assert!(parse_document(&document, id).is_err());
    }
}
