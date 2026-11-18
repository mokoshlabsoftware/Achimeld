//! Exact envelope for the existing C# Playwright worker, with response correlation.
use achimeld_core::SteamAppId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Closed read and reviewed synchronization commands. The receiver independently
/// enforces SEC-READ-001 and SEC-SYNC-001; generic mutations remain absent.
pub(crate) enum ReadCommand {
    CheckPublicVersion,
    Start,
    Authenticate,
    AuthenticationState,
    Navigate(SteamAppId, bool),
    BrowserOpen,
    ReadConfiguration(SteamAppId),
    ReadGameBanner(SteamAppId),
    DeleteSteam {
        project: Value,
        targets: Value,
    },
    PrepareSync {
        project: Value,
        catalog: Value,
        icons: Value,
    },
    BeginSync {
        token: String,
        choices: Value,
    },
    SyncStep {
        token: String,
        key: String,
        achieved: Option<String>,
        unachieved: Option<String>,
    },
    FinishSync {
        token: String,
    },
    Stop,
    NetworkSelfTest {
        hold: bool,
    },
}
impl ReadCommand {
    pub(crate) fn request(self, request_id: String) -> WorkerRequest {
        let (operation, arguments) = match self {
            Self::CheckPublicVersion => ("checkPublicVersion", Value::Null),
            Self::DeleteSteam { project, targets } => (
                "deleteSteam",
                serde_json::json!({"project":project,"targets":targets}),
            ),
            Self::Start => ("start", Value::Null),
            Self::Authenticate => ("authenticate", Value::Null),
            Self::AuthenticationState => ("authenticationState", Value::Null),
            Self::Navigate(id, stats) => (
                "navigate",
                Value::String(format!(
                    "https://partner.steamgames.com/apps/{}/{}",
                    if stats { "stats" } else { "achievements" },
                    id.value()
                )),
            ),
            Self::BrowserOpen => ("browserOpen", Value::Null),
            Self::ReadConfiguration(id) => {
                ("readConfiguration", serde_json::json!({"value":id.value()}))
            }
            Self::ReadGameBanner(id) => ("readGameBanner", serde_json::json!({"value":id.value()})),
            Self::PrepareSync {
                project,
                catalog,
                icons,
            } => (
                "prepareSync",
                serde_json::json!({"project":project,"catalog":catalog,"icons":icons}),
            ),
            Self::BeginSync { token, choices } => (
                "beginSync",
                serde_json::json!({"token":token,"choices":choices}),
            ),
            Self::SyncStep {
                token,
                key,
                achieved,
                unachieved,
            } => (
                "syncStep",
                serde_json::json!({"token":token,"key":key,"achieved":achieved,"unachieved":unachieved}),
            ),
            Self::FinishSync { token } => ("finishSync", serde_json::json!({"token":token})),
            Self::Stop => ("stop", Value::Null),
            Self::NetworkSelfTest { hold } => ("networkSelfTest", Value::Bool(hold)),
        };
        WorkerRequest {
            request_id,
            operation: operation.to_owned(),
            arguments,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerRequest {
    pub request_id: String,
    pub operation: String,
    pub arguments: Value,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerResponse {
    pub request_id: String,
    pub success: bool,
    pub data: Value,
    pub error_code: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedPageDocument {
    pub uri: String,
    pub title: String,
    pub html: String,
}

impl WorkerResponse {
    pub(crate) fn failure_details(&self, request_id: &str) -> Option<String> {
        if self.request_id != request_id || self.success {
            return None;
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Details {
            stage: String,
            kind: String,
            elapsed_ms: u64,
            #[serde(default)]
            request: Option<RequestDetails>,
            #[serde(default)]
            sync: Option<SyncDetails>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct SyncDetails {
            phase: String,
            phase_elapsed_ms: u64,
            endpoint: String,
            write_state: String,
            response_class: String,
            #[serde(default)]
            schema_mask: u32,
            #[serde(default)]
            schema_kind: String,
            #[serde(default)]
            schema_items: u32,
            #[serde(default)]
            schema_named: u32,
            #[serde(default)]
            schema_objects: u32,
            #[serde(default)]
            schema_largest_array: u32,
            #[serde(default)]
            schema_types: u32,
            #[serde(default)]
            schema_container_mask: u32,
            #[serde(default)]
            schema_container_objects: u32,
            #[serde(default)]
            schema_container_arrays: u32,
            #[serde(default)]
            dom_flags: u32,
            #[serde(default)]
            dom_tables: u32,
            #[serde(default)]
            dom_table_rows: u32,
            #[serde(default)]
            dom_grid_rows: u32,
            #[serde(default)]
            dom_frames: u32,
            #[serde(default)]
            dom_api_tokens: u32,
            #[serde(default)]
            dom_api_leaves: u32,
            #[serde(default)]
            dom_achievement_classes: u32,
            #[serde(default)]
            dom_images: u32,
            #[serde(default)]
            dom_buttons: u32,
            #[serde(default)]
            dom_list_items: u32,
            #[serde(default)]
            dom_single_token_cards: u32,
            #[serde(default)]
            dom_cards_with_two_images: u32,
            #[serde(default)]
            dom_cards_with_buttons: u32,
            #[serde(default)]
            dom_cards_with_id_text: u32,
            #[serde(default)]
            dom_cards_with_inputs: u32,
            #[serde(default)]
            dom_largest_card_depth: u32,
            #[serde(default)]
            dom_card_line_pattern: String,
            #[serde(default)]
            dom_card_pattern_matches: u32,
            #[serde(default)]
            html_api_tokens: u32,
            #[serde(default)]
            html_scripts: u32,
            #[serde(default)]
            html_field_mask: u32,
            #[serde(default)]
            app_info_api_tokens: u32,
            #[serde(default)]
            app_info_field_mask: u32,
            #[serde(default)]
            dom_export_mask: u32,
            #[serde(default)]
            export_definitions_rows: u32,
            #[serde(default)]
            export_localization_rows: u32,
            #[serde(default)]
            export_definitions_header_mask: u32,
            #[serde(default)]
            export_localization_header_mask: u32,
            #[serde(default)]
            export_transport_mask: u32,
            #[serde(default)]
            dom_icon_order_evidence: u32,
            #[serde(default)]
            dom_link_action: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RequestDetails {
            method: String,
            host: String,
            path: String,
            extension: String,
            query: bool,
            navigation: bool,
            resource: String,
            failure: String,
        }
        let details: Details = serde_json::from_value(self.data.clone()).ok()?;
        if !matches!(
            details.stage.as_str(),
            "operation"
                | "idle"
                | "navigation.health"
                | "navigation.route"
                | "navigation.popup-check"
                | "navigation.page"
                | "navigation.dom-content-loaded"
                | "navigation.final-popup-check"
                | "navigation.login-prompt"
                | "navigation.complete"
        ) || !matches!(
            details.kind.as_str(),
            "cancelled"
                | "timeout"
                | "playwright"
                | "portal-policy"
                | "invalid-data"
                | "invalid-state"
                | "windows"
                | "io"
                | "access-denied"
                | "other"
        ) || details.elapsed_ms > 300_000
        {
            return None;
        }
        let mut message = format!(
            "Worker rejection details · stage={} · category={} · worker elapsed={} ms",
            details.stage, details.kind, details.elapsed_ms
        );
        if let Some(sync) = details.sync {
            if !matches!(
                sync.phase.as_str(),
                "none"
                    | "prepare"
                    | "review-precheck"
                    | "step-precheck"
                    | "write-precheck"
                    | "write-apply"
                    | "write-readback"
                    | "step-complete"
                    | "finish-precheck"
            ) || sync.phase_elapsed_ms > 300_000
                || !matches!(
                    sync.endpoint.as_str(),
                    "none"
                        | "fetchstats"
                        | "fetchachievements"
                        | "ajaxgetstatsschema"
                        | "ajaxgetappinfo"
                        | "ajaxcreateorupdateachievement"
                        | "savestat"
                        | "saveachievement"
                        | "deletestat"
                        | "deleteachievement"
                        | "uploadachievement"
                )
                || !(matches!(
                    sync.write_state.as_str(),
                    "none"
                        | "read-stats"
                        | "read-achievements-array"
                        | "read-achievements-other"
                        | "read-achievements-html"
                        | "read-achievements-non-json"
                        | "read-icons"
                        | "read-status"
                        | "read-url"
                        | "read-body"
                        | "read-size"
                        | "read-other"
                        | "modern-schema-probed"
                        | "modern-app-info-probed"
                        | "modern-export-probed"
                        | "modern-export-reading"
                        | "modern-export-adapted"
                        | "modern-management-restored"
                        | "modern-editor-select"
                        | "modern-editor-field"
                        | "modern-editor-caption-missing"
                        | "modern-editor-caption-unbound"
                        | "modern-editor-save-control"
                        | "modern-editor-save-probe"
                        | "modern-parse-csv"
                        | "modern-check-counts"
                        | "modern-check-definitions"
                        | "modern-check-localizations"
                        | "modern-check-english"
                        | "modern-check-archived"
                        | "modern-check-set-id"
                        | "modern-check-permission"
                        | "modern-check-spoiler"
                        | "modern-check-progress"
                        | "modern-progress-name"
                        | "modern-progress-min-parse"
                        | "modern-progress-min-value"
                        | "modern-progress-max-parse"
                        | "modern-progress-max-value"
                        | "modern-progress-orphan-range"
                        | "modern-serialize"
                        | "save-armed"
                        | "save-confirmed"
                        | "delete-armed"
                        | "delete-confirmed"
                        | "readback-verified"
                ) || sync
                    .write_state
                    .strip_prefix("modern-editor-shape-")
                    .and_then(|value| value.parse::<u32>().ok())
                    .is_some_and(|mask| mask <= 134_217_727)
                    || sync
                        .write_state
                        .strip_prefix("modern-editor-structure-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 32_767)
                    || sync
                        .write_state
                        .strip_prefix("modern-editor-actions-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some_and(|mask| mask <= 262_143)
                    || sync
                        .write_state
                        .strip_prefix("modern-card-shape-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 32_767)
                    || sync
                        .write_state
                        .strip_prefix("modern-write-shape-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 16_383)
                    || sync
                        .write_state
                        .strip_prefix("modern-write-structure-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-route-shape-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-identity-shape-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 511)
                    || sync
                        .write_state
                        .strip_prefix("modern-id-parts-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-part-sequence-")
                        .and_then(|value| value.parse::<u64>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-import-shape-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 255)
                    || sync
                        .write_state
                        .strip_prefix("modern-import-actions-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 255)
                    || sync
                        .write_state
                        .strip_prefix("modern-preview-shape-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 255)
                    || sync
                        .write_state
                        .strip_prefix("modern-save-scope-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-preview-scope-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-target-scope-")
                        .and_then(|value| value.parse::<u32>().ok())
                        .is_some()
                    || sync
                        .write_state
                        .strip_prefix("modern-probe-gate-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 255)
                    || sync
                        .write_state
                        .strip_prefix("modern-after-save-")
                        .and_then(|value| value.parse::<u16>().ok())
                        .is_some_and(|mask| mask <= 255))
                || !matches!(
                    sync.response_class.as_str(),
                    "none" | "2xx" | "3xx" | "4xx" | "5xx" | "other"
                )
                || !matches!(
                    sync.schema_kind.as_str(),
                    "" | "missing" | "object" | "array" | "string" | "null" | "other"
                )
                || sync.schema_items > 10_001
                || sync.schema_named > 10_001
                || sync.schema_objects > 10_001
                || sync.schema_largest_array > 10_001
                || sync.schema_types > 63
                || sync.schema_container_mask > 65_535
                || sync.schema_container_objects > 128
                || sync.schema_container_arrays > 128
                || sync.dom_flags > 16_383
                || sync.dom_tables > 10_000
                || sync.dom_table_rows > 10_000
                || sync.dom_grid_rows > 10_000
                || sync.dom_frames > 10_000
                || sync.dom_api_tokens > 10_000
                || sync.dom_api_leaves > 10_000
                || sync.dom_achievement_classes > 10_000
                || sync.dom_images > 10_000
                || sync.dom_buttons > 10_000
                || sync.dom_list_items > 10_000
                || sync.dom_single_token_cards > 10_000
                || sync.dom_cards_with_two_images > 10_000
                || sync.dom_cards_with_buttons > 10_000
                || sync.dom_cards_with_id_text > 10_000
                || sync.dom_cards_with_inputs > 10_000
                || sync.dom_largest_card_depth > 8
                || sync.dom_card_line_pattern.len() > 32
                || !sync
                    .dom_card_line_pattern
                    .bytes()
                    .all(|byte| b"AIPCVSNT".contains(&byte))
                || sync.dom_card_pattern_matches > 10_000
                || sync.html_api_tokens > 10_000
                || sync.html_scripts > 10_000
                || sync.html_field_mask > 63
                || sync.app_info_api_tokens > 10_000
                || sync.app_info_field_mask > 1023
                || sync.dom_export_mask > 2047
                || sync.export_definitions_rows > 5000
                || sync.export_localization_rows > 5000
                || sync.export_definitions_header_mask > 255
                || sync.export_localization_header_mask > 255
                || sync.export_transport_mask > 255
                || sync.dom_icon_order_evidence > 10_000
                || !(sync.dom_link_action.is_empty()
                    || sync.dom_link_action == "none"
                    || (sync.dom_link_action.len() <= 32
                        && sync
                            .dom_link_action
                            .bytes()
                            .all(|byte| byte.is_ascii_alphabetic())))
            {
                return None;
            }
            message.push_str(&format!(
                " · sync-phase={} · phase elapsed={} ms · endpoint={} · write-state={} · response-class={} · schema-mask={} · schema-kind={} · schema-items={} · schema-named={} · schema-objects={} · schema-largest-array={} · schema-types={} · schema-container-mask={} · schema-container-objects={} · schema-container-arrays={} · dom-flags={} · dom-tables={} · dom-table-rows={} · dom-grid-rows={} · dom-frames={} · dom-api-tokens={} · dom-api-leaves={} · dom-achievement-classes={} · dom-images={} · dom-buttons={} · dom-list-items={} · dom-single-token-cards={} · dom-cards-with-two-images={} · dom-cards-with-buttons={} · dom-cards-with-id-text={} · dom-cards-with-inputs={} · dom-largest-card-depth={} · dom-card-line-pattern={} · dom-card-pattern-matches={} · html-api-tokens={} · html-scripts={} · html-field-mask={} · app-info-api-tokens={} · app-info-field-mask={} · dom-export-mask={} · export-definitions-rows={} · export-localization-rows={} · export-definitions-header-mask={} · export-localization-header-mask={} · export-transport-mask={} · dom-icon-order-evidence={} · dom-link-action={}",
                sync.phase, sync.phase_elapsed_ms, sync.endpoint, sync.write_state, sync.response_class,
                sync.schema_mask, sync.schema_kind, sync.schema_items, sync.schema_named,
                sync.schema_objects, sync.schema_largest_array, sync.schema_types,
                sync.schema_container_mask, sync.schema_container_objects, sync.schema_container_arrays,
                sync.dom_flags, sync.dom_tables, sync.dom_table_rows, sync.dom_grid_rows,
                sync.dom_frames, sync.dom_api_tokens, sync.dom_api_leaves,
                sync.dom_achievement_classes, sync.dom_images, sync.dom_buttons,
                sync.dom_list_items, sync.dom_single_token_cards,
                sync.dom_cards_with_two_images, sync.dom_cards_with_buttons,
                sync.dom_cards_with_id_text, sync.dom_cards_with_inputs,
                sync.dom_largest_card_depth, sync.dom_card_line_pattern,
                sync.dom_card_pattern_matches, sync.html_api_tokens,
                sync.html_scripts, sync.html_field_mask, sync.app_info_api_tokens,
                sync.app_info_field_mask, sync.dom_export_mask,
                sync.export_definitions_rows, sync.export_localization_rows,
                sync.export_definitions_header_mask, sync.export_localization_header_mask,
                sync.export_transport_mask, sync.dom_icon_order_evidence,
                sync.dom_link_action
            ));
        }
        if let Some(request) = details.request {
            // The worker reduces /achievements/ajax<action>/<app-id> to a
            // bounded action category. Keep the same shape here so a new
            // Steamworks Ajax read can be diagnosed without a URL or query.
            let achievement_ajax_action = request.host == "partner"
                && matches!(request.resource.as_str(), "xhr" | "fetch")
                && !request.navigation
                && request
                    .path
                    .strip_prefix("achievements-ajax")
                    .is_some_and(|action| {
                        (1..=48).contains(&action.len())
                            && action.bytes().all(|byte| byte.is_ascii_lowercase())
                    });
            let um_action = request.host == "partner"
                && matches!(request.resource.as_str(), "xhr" | "fetch")
                && !request.navigation
                && request
                    .path
                    .strip_prefix("um-action-")
                    .is_some_and(|parts| {
                        parts.split_once('-').is_some_and(|(service, action)| {
                            (1..=32).contains(&service.len())
                                && (1..=48).contains(&action.len())
                                && service.bytes().all(|byte| byte.is_ascii_alphabetic())
                                && action.bytes().all(|byte| byte.is_ascii_alphabetic())
                        })
                    });
            if !matches!(
                request.method.as_str(),
                "GET" | "POST" | "HEAD" | "PUT" | "DELETE" | "other"
            ) || !matches!(
                request.host.as_str(),
                "partner" | "login" | "api" | "store" | "community" | "content" | "other"
            ) || !(achievement_ajax_action
                || um_action
                || matches!(
                    request.path.as_str(),
                    "public"
                        | "apps"
                        | "login"
                        | "authentication-api"
                        | "csp-report"
                        | "achievements-ajaxgetstatsschema"
                        | "achievements-ajaxgetlanguagelist"
                        | "achievements-ajaxgetlanguageoptionslist"
                        | "achievements-other"
                        | "player-game-achievements"
                        | "player-global-percentages"
                        | "player-other"
                        | "um-other"
                        | "other"
                ))
                || !matches!(
                    request.resource.as_str(),
                    "document"
                        | "media"
                        | "script"
                        | "stylesheet"
                        | "image"
                        | "font"
                        | "xhr"
                        | "fetch"
                        | "ping"
                        | "manifest"
                        | "eventsource"
                        | "cspreport"
                        | "other"
                )
                || !matches!(
                    request.failure.as_str(),
                    "policy"
                        | "tunnel"
                        | "proxy"
                        | "dns"
                        | "timeout"
                        | "reset"
                        | "refused"
                        | "closed"
                        | "disconnected"
                        | "changed"
                        | "aborted"
                        | "blocked"
                        | "failed"
                        | "certificate"
                        | "other"
                )
                || !matches!(
                    request.extension.as_str(),
                    ".mp3"
                        | ".ogg"
                        | ".wav"
                        | ".m4a"
                        | ".aac"
                        | ".mp4"
                        | ".webm"
                        | ".ogv"
                        | ".js"
                        | ".css"
                        | ".png"
                        | ".jpg"
                        | ".woff"
                        | ".woff2"
                        | "other"
                )
            {
                return None;
            }
            message.push_str(&format!(
                " · method={} · host-category={} · path-category={} · extension={} · query-present={} · navigation={} · resource={} · request-failure={}",
                request.method, request.host, request.path, request.extension, request.query, request.navigation, request.resource, request.failure
            ));
        }
        Some(message)
    }
    pub fn accept(self, request_id: &str) -> Result<Value, &'static str> {
        if self.request_id != request_id {
            return Err("Worker response belongs to another request");
        }
        if !self.success || self.error_code.is_some() {
            // Map only declared codes to static text. Never echo an unknown code
            // or serialize worker exceptions/response payloads into diagnostics.
            if let Some(message) = match self.error_code.as_deref() {
                Some("steam.request.other-page") => {
                    Some("Browser request policy blocked a request from an unowned page")
                }
                Some("steam.request.ping") => {
                    Some("Browser request policy blocked a ping or beacon request")
                }
                Some("steam.request.media") => {
                    Some("Browser request policy blocked a media request")
                }
                Some("steam.request.manifest") => {
                    Some("Browser request policy blocked a web manifest request")
                }
                Some("steam.request.eventsource") => {
                    Some("Browser request policy blocked an event stream request")
                }
                Some("steam.request.frame-unavailable") => {
                    Some("Browser request policy could not verify the requesting frame")
                }
                Some("steam.request.home") => {
                    Some("Browser request policy blocked the Steamworks home redirect")
                }
                Some("steam.request.dashboard") => {
                    Some("Browser request policy blocked the Steamworks dashboard redirect")
                }
                Some("steam.request.navigation") => {
                    Some("Browser request policy blocked a navigation request")
                }
                Some("steam.request.script") => {
                    Some("Browser request policy blocked a script resource")
                }
                Some("steam.request.stylesheet") => {
                    Some("Browser request policy blocked a stylesheet resource")
                }
                Some("steam.request.image") => {
                    Some("Browser request policy blocked an image resource")
                }
                Some("steam.request.font") => {
                    Some("Browser request policy blocked a font resource")
                }
                Some("steam.request.dynamic") => {
                    Some("Browser request policy blocked a dynamic page request")
                }
                Some("steam.request.websocket") => {
                    Some("Browser request policy blocked a WebSocket connection")
                }
                Some("steam.request.rejected") => {
                    Some("Browser request policy blocked an unsupported request")
                }
                Some("steam.network.tunnel") => {
                    Some("The supervised Steam HTTPS tunnel could not be established")
                }
                Some("steam.network.proxy") => {
                    Some("The dedicated browser could not connect to the supervised Steam broker")
                }
                Some("steam.network.certificate") => {
                    Some("The dedicated browser could not verify the Steam HTTPS certificate")
                }
                Some("steam.network.dns") => {
                    Some("The dedicated browser could not resolve a required network name")
                }
                Some("steam.network.connection") => Some(
                    "The Steam connection was closed, reset or refused; check the connection and validate again",
                ),
                Some("steam.network.timeout") => {
                    Some("Chromium reported net::ERR_TIMED_OUT while opening Steam")
                }
                Some("steam.network.changed") => {
                    Some("Chromium reported net::ERR_NETWORK_CHANGED while opening Steam")
                }
                Some("steam.network.disconnected") => {
                    Some("Chromium reported net::ERR_INTERNET_DISCONNECTED while opening Steam")
                }
                Some("steam.network.failed") => {
                    Some("Chromium reported net::ERR_FAILED while opening Steam")
                }
                Some("steam.network.http") => Some(
                    "Steam could not serve the requested page; check Steamworks access and validate again",
                ),
                Some("steam.navigation.interrupted") => Some(
                    "Steam page navigation was interrupted; validate again to start a fresh session",
                ),
                Some("steam.browser.closed") => Some(
                    "The dedicated Steam browser or its page closed during the operation; validate again to start a fresh session",
                ),
                Some("steam.navigation.failed") => Some(
                    "The dedicated browser could not open the Steamworks page; validate again to start a fresh session",
                ),
                Some("security.operation.interrupted") => {
                    Some("Steam operation was interrupted; validate again before continuing")
                }
                Some("steam.operation.timeout") => Some(
                    "Steam operation timed out; check the dedicated browser and validate again",
                ),
                Some("steam.runtime.missing") => Some(
                    "A required Steam browser runtime file or directory is missing; rebuild or repair the application package",
                ),
                Some("steam.runtime.access-denied") => {
                    Some("Access to a required Steam browser runtime resource was denied")
                }
                Some("steam.portal.rejected") => {
                    Some("Steamworks access or a browser request was rejected by the safety policy")
                }
                Some("steam.portal.unsupported") => {
                    Some("The Steamworks page structure is not supported by this application build")
                }
                Some("steam.sync.snapshot-invalid") => {
                    Some("Steam snapshot identities, ownership or fingerprints are inconsistent.")
                }
                Some("steam.sync.command-invalid") => Some(
                    "Synchronization request failed project, catalog, icon or session validation.",
                ),
                Some("steam.sync.rename-conflict") => Some(
                    "A pending API rename conflicts with the current Steam record identity. Reconcile or revert the rename before synchronizing.",
                ),
                Some("steam.sync.name-case-conflict") => Some(
                    "Project and Steam API names differ only by case. Align their spelling before synchronizing.",
                ),
                Some("steam.sync.review-changed") => {
                    Some("Steam changed after review. Prepare a new synchronization plan.")
                }
                Some("steam.sync.remote-changed") => {
                    Some("Steam changed during synchronization. Prepare a fresh plan.")
                }
                Some("steam.sync.readback-mismatch") => {
                    Some("Steam draft read-back did not match the approved change.")
                }
                Some("steam.sync.save-unconfirmed") => Some(
                    "Steamworks did not confirm the reviewed draft save. Read Steam again before retrying.",
                ),
                Some("steam.sync.delete-unconfirmed") => Some(
                    "Steamworks did not confirm the reviewed draft deletion. Read Steam again before retrying.",
                ),
                Some("steam.sync.icon-post-rejected") => Some(
                    "Steamworks rejected the reviewed icon upload. Read Steam again before retrying.",
                ),
                Some("steam.sync.icon-not-saved") => Some(
                    "Steamworks did not report a saved icon. Read Steam again before retrying.",
                ),
                Some("steam.sync.icon-verify-failed") => Some(
                    "The saved Steam icon did not match the reviewed artwork after read-back retries. Read Steam again before retrying.",
                ),
                Some("steam.sync.icon-stale") => Some(
                    "Steam still serves the pre-upload icon hash after the draft write. The outcome is uncertain; read Steam again before retrying.",
                ),
                Some("steam.sync.icon-format") => Some(
                    "Steamworks returned an icon with unexpected image dimensions or format. The draft outcome is uncertain; read Steam again before retrying.",
                ),
                Some("steam.sync.icon-pixels-near") => Some(
                    "Steamworks converted the icon but its pixels fell outside the strict artwork threshold. The draft outcome is uncertain; read Steam again before retrying.",
                ),
                Some("steam.sync.icon-pixels-different") => Some(
                    "Steamworks saved icon pixels substantially different from the reviewed artwork. The draft outcome is uncertain; read Steam again before retrying.",
                ),
                Some("steam.sync.rename-identity") => {
                    Some("Steam did not preserve the renamed record identity.")
                }
                Some("steam.sync.localization-missing") => {
                    Some("A required project localization row is missing.")
                }
                Some("steam.sync.progress-selection") => {
                    Some("Select the progress stat before its dependent achievement.")
                }
                Some("steam.sync.icon-selection") => {
                    Some("An icon upload requires its achievement definition.")
                }
                Some("steam.sync.unrelated-change") => {
                    Some("An unrelated Steam value changed during the draft save.")
                }
                Some("steam.sync.artwork-changed") => {
                    Some("Steam artwork changed after review. Prepare a fresh plan.")
                }
                Some("steam.sync.artwork-size") => {
                    Some("Imported artwork exceeds the 8 MiB limit.")
                }
                Some("steam.sync.progress-import") => {
                    Some("Import the progress stat before its dependent achievement.")
                }
                Some("steam.project.invalid") => Some(
                    "Achimeld could not read this project's synchronization settings. Retrying Steam sign-in will not resolve this error.",
                ),
                Some("steam.snapshot.incomplete") => Some(
                    "Steamworks returned an incomplete configuration; validate again before continuing",
                ),
                Some("steam.browser.start.failed") => Some(
                    "The dedicated Steam browser could not start; check that its profile is not already in use and the application package is intact",
                ),
                _ => None,
            } {
                return Err(message);
            }
            if self.error_code.as_deref() == Some("steam.sync.failed") {
                return Err(
                    "Synchronization could not finish. Check the Terminal for details and review any unverified Steam changes before retrying.",
                );
            }
            return Err("Steam worker failed; uncertain operations must not be replayed");
        }
        Ok(self.data)
    }
}

pub(crate) fn exit_diagnostic(code: u32) -> Option<&'static str> {
    let code = match code {
        201 => "steam.request.other-page",
        211 => "steam.request.rejected",
        212 => "steam.request.ping",
        213 => "steam.request.media",
        214 => "steam.request.manifest",
        215 => "steam.request.eventsource",
        216 => "steam.request.frame-unavailable",
        202 => "steam.request.navigation",
        203 => "steam.request.home",
        204 => "steam.request.dashboard",
        205 => "steam.request.script",
        206 => "steam.request.stylesheet",
        207 => "steam.request.image",
        208 => "steam.request.font",
        209 => "steam.request.dynamic",
        210 => "steam.request.websocket",
        _ => return None,
    };
    WorkerResponse {
        request_id: String::new(),
        success: false,
        data: Value::Null,
        error_code: Some(code.into()),
    }
    .accept("")
    .err()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_exit_diagnostics_accept_only_the_declared_codes() {
        for code in 201..=216 {
            assert!(
                exit_diagnostic(code)
                    .unwrap()
                    .starts_with("Browser request policy")
            );
        }
        for code in [0, 1, 125, 200, 217, u32::MAX] {
            assert!(exit_diagnostic(code).is_none());
        }
    }
    #[test]
    fn old_worker_envelope_and_replay_guard() {
        let request = WorkerRequest {
            request_id: "11111111-2222-3333-4444-555555555555".into(),
            operation: "readConfiguration".into(),
            // The old client passes SteamAppId directly, not nested under appId.
            arguments: serde_json::json!({"value":480}),
        };
        let mut wire = Vec::new();
        achimeld_security::wire::write_frame(&mut wire, &request, 32 * 1024 * 1024).unwrap();
        let decoded: WorkerRequest =
            achimeld_security::wire::read_frame(&mut wire.as_slice(), 32 * 1024 * 1024).unwrap();
        assert_eq!(decoded.arguments["value"], 480);
        let response: WorkerResponse = serde_json::from_str(
            r#"{"requestId":"different","success":true,"data":{},"errorCode":null}"#,
        )
        .unwrap();
        assert!(response.accept(&request.request_id).is_err());
        assert!(
            serde_json::from_str::<WorkerResponse>(
                r#"{"requestId":"x","success":true,"data":{},"unexpected":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn failure_details_accept_only_correlated_bounded_closed_diagnostics() {
        let mut response = WorkerResponse {
            request_id: "request".into(),
            success: false,
            data: serde_json::json!({"stage":"navigation.dom-content-loaded","kind":"playwright","elapsedMs":19899}),
            error_code: Some("steam.navigation.failed".into()),
        };
        assert!(
            response
                .failure_details("request")
                .unwrap()
                .contains("19899 ms")
        );
        assert!(response.failure_details("another").is_none());
        response.data["stage"] = serde_json::json!("https://private.invalid/?token=secret");
        assert!(response.failure_details("request").is_none());
        response.data["stage"] = serde_json::json!("navigation.page");
        response.data["kind"] = serde_json::json!("secret");
        assert!(response.failure_details("request").is_none());
        response.data["kind"] = serde_json::json!("playwright");
        response.data["elapsedMs"] = serde_json::json!(300001);
        assert!(response.failure_details("request").is_none());
        response.data = serde_json::json!(false);
        assert!(
            response.failure_details("request").is_none(),
            "Legacy failure data stays compatible"
        );
    }
    #[test]
    fn typed_read_commands_keep_the_legacy_wire_and_exact_app_route() {
        let id = SteamAppId::new(480).unwrap();
        let read = ReadCommand::ReadConfiguration(id).request("request".into());
        assert_eq!(read.operation, "readConfiguration");
        assert_eq!(read.arguments, serde_json::json!({"value":480}));
        let banner = ReadCommand::ReadGameBanner(id).request("request".into());
        assert_eq!(banner.operation, "readGameBanner");
        assert_eq!(banner.arguments, serde_json::json!({"value":480}));
        assert_eq!(
            ReadCommand::Navigate(id, false)
                .request("request".into())
                .arguments,
            "https://partner.steamgames.com/apps/achievements/480"
        );
        assert_eq!(
            ReadCommand::Navigate(id, true)
                .request("request".into())
                .arguments,
            "https://partner.steamgames.com/apps/stats/480"
        );
    }

    #[test]
    fn request_diagnostics_reject_arbitrary_worker_text() {
        let details = serde_json::json!({
            "stage": "navigation.complete", "kind": "invalid-data", "elapsedMs": 20,
            "request": {"method":"GET", "host":"content", "path":"public", "extension":".mp3",
                "query":true, "navigation":false, "resource":"media", "failure":"policy"}
        });
        let mut response = WorkerResponse {
            request_id: "request".into(),
            success: false,
            data: details.clone(),
            error_code: Some("steam.request.media".into()),
        };
        assert!(
            response
                .failure_details("request")
                .unwrap()
                .contains("extension=.mp3")
        );
        for path in [
            "achievements-ajaxgetstatsschema",
            "achievements-ajaxgetlanguagelist",
            "achievements-ajaxgetlanguageoptionslist",
            "achievements-ajaxgetnewread",
        ] {
            response.data = details.clone();
            response.data["request"]["host"] = serde_json::json!("partner");
            response.data["request"]["resource"] = serde_json::json!("xhr");
            response.data["request"]["path"] = serde_json::json!(path);
            assert!(response.failure_details("request").unwrap().contains(path));
        }
        response.data["request"]["host"] = serde_json::json!("other");
        assert!(response.failure_details("request").is_none());
        response.data["request"]["host"] = serde_json::json!("partner");
        response.data["request"]["path"] =
            serde_json::json!(format!("achievements-ajax{}", "a".repeat(49)));
        assert!(response.failure_details("request").is_none());
        for path in [
            "achievements-other",
            "player-game-achievements",
            "player-global-percentages",
            "player-other",
            "um-other",
        ] {
            response.data = details.clone();
            response.data["request"]["path"] = serde_json::json!(path);
            assert!(response.failure_details("request").unwrap().contains(path));
        }
        for field in ["method", "host", "path", "extension", "resource", "failure"] {
            response.data = details.clone();
            response.data["request"][field] = serde_json::json!("secret-marker");
            assert!(response.failure_details("request").is_none());
        }
        response.data = details;
        response.data["request"]["url"] = serde_json::json!("secret-marker");
        assert!(response.failure_details("request").is_none());
    }
    #[test]
    fn synchronization_diagnostics_accept_only_closed_phase_and_status() {
        let details = serde_json::json!({
            "stage":"operation", "kind":"timeout", "elapsedMs":35604,
            "sync":{"phase":"write-precheck", "phaseElapsedMs":30010,
                "endpoint":"fetchachievements", "writeState":"none", "responseClass":"none"}
        });
        let mut response = WorkerResponse {
            request_id: "request".into(),
            success: false,
            data: details.clone(),
            error_code: Some("steam.operation.timeout".into()),
        };
        let logged = response.failure_details("request").unwrap();
        assert!(logged.contains("sync-phase=write-precheck"));
        assert!(logged.contains("endpoint=fetchachievements"));
        assert!(logged.contains("write-state=none"));
        response.data = details.clone();
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-shape-511");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-shape-134217728");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-structure-32767");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-structure-32768");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-actions-262143");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-editor-actions-262144");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-card-shape-32767");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-card-shape-32768");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-write-shape-16383");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-write-shape-16384");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-write-structure-65535");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-write-structure-65536");
        assert!(response.failure_details("request").is_none());
        response.data = details.clone();
        response.data["sync"]["endpoint"] = serde_json::json!("ajaxcreateorupdateachievement");
        response.data["sync"]["writeState"] = serde_json::json!("modern-route-shape-4294967295");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-route-shape-4294967296");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-identity-shape-511");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-identity-shape-512");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-id-parts-4294967295");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-id-parts-4294967296");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] =
            serde_json::json!("modern-part-sequence-18446744073709551615");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] =
            serde_json::json!("modern-part-sequence-18446744073709551616");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-import-shape-255");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-import-shape-256");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-import-actions-255");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-import-actions-256");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-preview-shape-255");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-preview-shape-256");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-save-scope-4294967295");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-save-scope-4294967296");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-preview-scope-4294967295");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-preview-scope-4294967296");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-target-scope-4294967295");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-target-scope-4294967296");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-probe-gate-255");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-probe-gate-256");
        assert!(response.failure_details("request").is_none());
        response.data["sync"]["writeState"] = serde_json::json!("modern-after-save-255");
        assert!(response.failure_details("request").is_some());
        response.data["sync"]["writeState"] = serde_json::json!("modern-after-save-256");
        assert!(response.failure_details("request").is_none());
        for field in ["phase", "endpoint", "writeState", "responseClass"] {
            response.data = details.clone();
            response.data["sync"][field] = serde_json::json!("secret-marker");
            assert!(response.failure_details("request").is_none());
        }
        response.data = details;
        response.data["sync"]["url"] = serde_json::json!("private");
        assert!(response.failure_details("request").is_none());
        response.data["sync"].as_object_mut().unwrap().remove("url");
        response.data["sync"]["domLinkAction"] = serde_json::json!("private/path?token=secret");
        assert!(response.failure_details("request").is_none());
    }

    #[test]
    fn worker_diagnostics_use_declared_static_messages_and_never_echo_unknown_codes() {
        for (code, expected) in [
            (
                "steam.network.connection",
                "The Steam connection was closed, reset or refused; check the connection and validate again",
            ),
            (
                "steam.network.http",
                "Steam could not serve the requested page; check Steamworks access and validate again",
            ),
            (
                "steam.navigation.interrupted",
                "Steam page navigation was interrupted; validate again to start a fresh session",
            ),
            (
                "steam.browser.closed",
                "The dedicated Steam browser or its page closed during the operation; validate again to start a fresh session",
            ),
            (
                "steam.navigation.failed",
                "The dedicated browser could not open the Steamworks page; validate again to start a fresh session",
            ),
        ] {
            let response = WorkerResponse {
                request_id: "same".into(),
                success: false,
                data: serde_json::json!({"password":"hidden"}),
                error_code: Some(code.into()),
            };
            assert_eq!(response.accept("same"), Err(expected));
        }
        for (code, expected) in [
            (
                "steam.sync.snapshot-invalid",
                "Steam snapshot identities, ownership or fingerprints are inconsistent.",
            ),
            (
                "steam.sync.command-invalid",
                "Synchronization request failed project, catalog, icon or session validation.",
            ),
            (
                "steam.sync.rename-conflict",
                "A pending API rename conflicts with the current Steam record identity. Reconcile or revert the rename before synchronizing.",
            ),
            (
                "steam.sync.name-case-conflict",
                "Project and Steam API names differ only by case. Align their spelling before synchronizing.",
            ),
            (
                "steam.sync.review-changed",
                "Steam changed after review. Prepare a new synchronization plan.",
            ),
            (
                "steam.sync.remote-changed",
                "Steam changed during synchronization. Prepare a fresh plan.",
            ),
            (
                "steam.sync.readback-mismatch",
                "Steam draft read-back did not match the approved change.",
            ),
            (
                "steam.sync.save-unconfirmed",
                "Steamworks did not confirm the reviewed draft save. Read Steam again before retrying.",
            ),
            (
                "steam.sync.delete-unconfirmed",
                "Steamworks did not confirm the reviewed draft deletion. Read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-post-rejected",
                "Steamworks rejected the reviewed icon upload. Read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-not-saved",
                "Steamworks did not report a saved icon. Read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-verify-failed",
                "The saved Steam icon did not match the reviewed artwork after read-back retries. Read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-stale",
                "Steam still serves the pre-upload icon hash after the draft write. The outcome is uncertain; read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-format",
                "Steamworks returned an icon with unexpected image dimensions or format. The draft outcome is uncertain; read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-pixels-near",
                "Steamworks converted the icon but its pixels fell outside the strict artwork threshold. The draft outcome is uncertain; read Steam again before retrying.",
            ),
            (
                "steam.sync.icon-pixels-different",
                "Steamworks saved icon pixels substantially different from the reviewed artwork. The draft outcome is uncertain; read Steam again before retrying.",
            ),
            (
                "steam.sync.rename-identity",
                "Steam did not preserve the renamed record identity.",
            ),
            (
                "steam.sync.localization-missing",
                "A required project localization row is missing.",
            ),
            (
                "steam.sync.progress-selection",
                "Select the progress stat before its dependent achievement.",
            ),
            (
                "steam.sync.icon-selection",
                "An icon upload requires its achievement definition.",
            ),
            (
                "steam.sync.unrelated-change",
                "An unrelated Steam value changed during the draft save.",
            ),
            (
                "steam.sync.artwork-changed",
                "Steam artwork changed after review. Prepare a fresh plan.",
            ),
            (
                "steam.sync.artwork-size",
                "Imported artwork exceeds the 8 MiB limit.",
            ),
            (
                "steam.sync.progress-import",
                "Import the progress stat before its dependent achievement.",
            ),
        ] {
            let response = WorkerResponse {
                request_id: "same".into(),
                success: false,
                data: serde_json::json!({"password":"hidden"}),
                error_code: Some(code.into()),
            };
            assert_eq!(response.accept("same"), Err(expected));
        }
        for code in [
            "steam.sync.snapshot-invalid",
            "steam.sync.command-invalid",
            "steam.sync.rename-conflict",
            "steam.sync.name-case-conflict",
            "steam.sync.review-changed",
            "steam.sync.remote-changed",
            "steam.sync.readback-mismatch",
            "steam.sync.save-unconfirmed",
            "steam.sync.delete-unconfirmed",
            "steam.sync.icon-post-rejected",
            "steam.sync.icon-not-saved",
            "steam.sync.icon-verify-failed",
            "steam.sync.icon-stale",
            "steam.sync.icon-format",
            "steam.sync.icon-pixels-near",
            "steam.sync.icon-pixels-different",
            "steam.sync.rename-identity",
            "steam.sync.localization-missing",
            "steam.sync.progress-selection",
            "steam.sync.icon-selection",
            "steam.sync.unrelated-change",
            "steam.sync.artwork-changed",
            "steam.sync.artwork-size",
            "steam.sync.progress-import",
            "security.operation.interrupted",
            "steam.operation.timeout",
            "steam.runtime.missing",
            "steam.runtime.access-denied",
            "steam.portal.rejected",
            "steam.portal.unsupported",
            "steam.project.invalid",
            "steam.snapshot.incomplete",
            "steam.browser.start.failed",
            "steam.network.connection",
            "steam.network.http",
            "steam.navigation.interrupted",
            "steam.browser.closed",
            "steam.navigation.failed",
            "steam.sync.failed",
            "steam.read.failed",
            "https://secret.invalid/?token=hidden",
        ] {
            let response = WorkerResponse {
                request_id: "same".into(),
                success: false,
                data: serde_json::json!({"password":"hidden"}),
                error_code: Some(code.into()),
            };
            let message = response.accept("same").unwrap_err();
            assert!(!message.contains("hidden") && !message.contains("secret.invalid"));
        }
        let response = WorkerResponse {
            request_id: "other".into(),
            success: false,
            data: Value::Null,
            error_code: Some("steam.runtime.missing".into()),
        };
        assert_eq!(
            response.accept("same"),
            Err("Worker response belongs to another request")
        );
    }
}
