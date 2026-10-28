//! Exact envelope for the existing C# Playwright worker, with response correlation.
use achimeld_core::SteamAppId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Closed read and reviewed synchronization commands. The receiver independently
/// enforces SEC-READ-001 and SEC-SYNC-001; generic mutations remain absent.
pub(crate) enum ReadCommand {
    Start,
    Authenticate,
    AuthenticationState,
    Navigate(SteamAppId),
    ReadConfiguration(SteamAppId),
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
            Self::Start => ("start", Value::Null),
            Self::Authenticate => ("authenticate", Value::Null),
            Self::AuthenticationState => ("authenticationState", Value::Null),
            Self::Navigate(id) => (
                "navigate",
                Value::String(format!(
                    "https://partner.steamgames.com/apps/achievements/{}",
                    id.value()
                )),
            ),
            Self::ReadConfiguration(id) => {
                ("readConfiguration", serde_json::json!({"value":id.value()}))
            }
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
        if let Some(request) = details.request {
            if !matches!(
                request.method.as_str(),
                "GET" | "POST" | "HEAD" | "PUT" | "DELETE" | "other"
            ) || !matches!(
                request.host.as_str(),
                "partner" | "login" | "api" | "store" | "community" | "content" | "other"
            ) || !matches!(
                request.path.as_str(),
                "public" | "apps" | "login" | "authentication-api" | "csp-report" | "other"
            ) || !matches!(
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
            ) || !matches!(
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
            ) || !matches!(
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
            ) {
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
                Some("steam.project.invalid") => Some(
                    "The Steam worker could not read this saved project format; check artwork and synchronization settings",
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
                    "Synchronization stopped. Steam may have changed or a selected value could not be verified. Prepare a fresh plan before continuing.",
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
        assert_eq!(
            ReadCommand::Navigate(id)
                .request("request".into())
                .arguments,
            "https://partner.steamgames.com/apps/achievements/480"
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
        for code in [
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
