//! SEC-SUPERVISION-001: bounded request -> OS-derived caller -> authorization -> effect.
//! Rejected requests close only that connection. Loss of a mandatory control stops the Job.
use crate::{
    manifest::{ProgramRole, VerifiedPackage},
    platform::{self, Job},
    telemetry::{self, Observation},
    udp_probes::Probes,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Operation {
    Hello,
    Status,
    Report,
    ClearProfile,
    PolicyFailure,
    UdpProbe4,
    UdpProbe6,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    operation: Operation,
    nonce: String,
    #[serde(default)]
    probe_token: Option<String>,
    #[serde(default)]
    failure_code: Option<String>,
}

impl Request {
    fn parse(value: serde_json::Value) -> Result<Self, DispatchError> {
        let request: Self =
            serde_json::from_value(value).map_err(|_| DispatchError::Rejected("request-schema"))?;
        if !hex_token(&request.nonce) {
            return Err(DispatchError::Rejected("request-challenge"));
        }
        match request.operation {
            Operation::UdpProbe4 | Operation::UdpProbe6 => {
                if !request.probe_token.as_deref().is_some_and(hex_token)
                    || request.failure_code.is_some()
                {
                    return Err(DispatchError::Rejected("request-probe"));
                }
            }
            Operation::PolicyFailure => {
                if request.probe_token.is_some()
                    || request.failure_code.as_deref().is_some_and(|code| {
                        code.is_empty()
                            || code.len() > 64
                            || !code
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
                    })
                {
                    return Err(DispatchError::Rejected("request-failure-code"));
                }
            }
            _ if request.probe_token.is_some() || request.failure_code.is_some() => {
                return Err(DispatchError::Rejected("request-extra-argument"));
            }
            _ => {}
        }
        Ok(request)
    }
}
fn hex_token(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caller {
    Editor,
    Worker,
    NodeHost,
}
impl Caller {
    fn for_image(root: &Path, image: &Path) -> Option<Self> {
        [
            ("Achimeld.App.exe", Self::Editor),
            ("Achimeld.SteamWorker.exe", Self::Worker),
            ("Achimeld.NodeHost.exe", Self::NodeHost),
        ]
        .into_iter()
        .find_map(|(name, role)| platform::same_image(&root.join(name), image).then_some(role))
    }
    fn authenticate(root: &Path, job: &Job, pid: u32) -> Result<Self, DispatchError> {
        // SECURITY(SEC-SUPERVISION-001): The PID is obtained from the connected pipe,
        // never from request data. An ACL or claimed process name is insufficient.
        if pid == 0 || !job.contains(pid) {
            return Err(DispatchError::Rejected("caller-job"));
        }
        let image = platform::image(pid).map_err(|_| DispatchError::Rejected("caller-image"))?;
        Self::for_image(root, &image).ok_or(DispatchError::Rejected("caller-role"))
    }
    fn permits(self, operation: Operation) -> bool {
        match operation {
            Operation::ClearProfile => self == Self::Editor,
            Operation::Hello
            | Operation::Status
            | Operation::Report
            | Operation::PolicyFailure
            | Operation::UdpProbe4
            | Operation::UdpProbe6 => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    Rejected(&'static str),
    ControlLost(&'static str),
}
impl DispatchError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Rejected(code) | Self::ControlLost(code) => code,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    nonce: String,
    session_id: String,
    supervisor_pid: u32,
    package_version: String,
    package_mode: String,
    network_enforced: bool,
    monitoring_active: bool,
    monitoring_coverage: &'static str,
    proxy_port: u16,
    browser_executable: PathBuf,
    profile_directory: PathBuf,
    probe_port: u16,
    udp_probe_denied: Option<bool>,
    failure_code: Option<&'static str>,
    report_json: Option<String>,
}

pub struct Dispatcher<'a> {
    pub package: &'a VerifiedPackage,
    pub job: &'a Job,
    pub profile: &'a Path,
    pub session: &'a str,
    pub supervisor_pid: u32,
    pub healthy: &'a AtomicBool,
    pub network_enforced: bool,
    pub observations: &'a Mutex<VecDeque<Observation>>,
    pub probes: Option<&'a mut Probes>,
}
impl Dispatcher<'_> {
    pub fn authorize_peer(&self, pid: u32) -> Result<(), DispatchError> {
        Caller::authenticate(self.package.root(), self.job, pid).map(|_| ())
    }
    pub fn respond(&mut self, pid: u32, value: serde_json::Value) -> Result<Status, DispatchError> {
        let request = Request::parse(value)?;
        let caller = Caller::authenticate(self.package.root(), self.job, pid)?;
        if !caller.permits(request.operation) {
            return Err(DispatchError::Rejected("operation-role"));
        }
        let mut failure_code = None;
        let mut udp_probe_denied = None;
        match request.operation {
            Operation::ClearProfile => {
                if self.browser_busy()? || platform::clear_profile(self.profile).is_err() {
                    failure_code = Some("security.profile.busy");
                }
            }
            Operation::PolicyFailure => {
                // Do not log caller-provided codes even if syntactically bounded.
                self.healthy.store(false, Ordering::Release);
                failure_code = Some("security.policy.probe.failed");
            }
            Operation::UdpProbe4 | Operation::UdpProbe6 => {
                if !self.network_enforced {
                    return Err(DispatchError::Rejected("network-isolation-unavailable"));
                }
                let denied = self
                    .probes
                    .as_mut()
                    .ok_or(DispatchError::ControlLost("udp-probe-unavailable"))?
                    .denied(
                        request.operation == Operation::UdpProbe6,
                        request
                            .probe_token
                            .as_deref()
                            .ok_or(DispatchError::Rejected("request-probe"))?,
                    )
                    .map_err(|_| DispatchError::ControlLost("udp-probe-unavailable"))?;
                udp_probe_denied = Some(denied);
                if !denied {
                    self.healthy.store(false, Ordering::Release);
                    failure_code = Some("security.policy.probe.failed");
                }
            }
            Operation::Hello | Operation::Status | Operation::Report => {}
        }
        let healthy = self.healthy.load(Ordering::Acquire);
        let enforced = self.network_enforced && healthy;
        let report_json = if request.operation == Operation::Report {
            let records = self
                .observations
                .lock()
                .map_err(|_| DispatchError::ControlLost("report-unavailable"))?;
            Some(
                serde_json::to_string(&Report {
                    schema_version: 1,
                    product: "Achimeld",
                    package_version: &self.package.manifest().version,
                    package_mode: &self.package.manifest().mode,
                    network_enforced: enforced,
                    monitoring_coverage: telemetry::COVERAGE,
                    records: &records,
                    limitations: REPORT_LIMITATIONS,
                })
                .map_err(|_| DispatchError::ControlLost("report-unavailable"))?,
            )
        } else {
            None
        };
        let manifest = self.package.manifest();
        Ok(Status {
            nonce: request.nonce,
            session_id: self.session.to_owned(),
            supervisor_pid: self.supervisor_pid,
            package_version: manifest.version.clone(),
            package_mode: manifest.mode.clone(),
            network_enforced: enforced,
            monitoring_active: healthy,
            monitoring_coverage: telemetry::COVERAGE,
            proxy_port: manifest.network.proxy_port,
            browser_executable: self
                .package
                .root()
                .join(&manifest.network.browser_executable),
            profile_directory: self.profile.to_owned(),
            // Retain the existing wire shape and valid endpoint range. User
            // sessions have no UDP probe listener and never claim probe denial.
            probe_port: self.probes.as_ref().map_or(42763, |probes| probes.port),
            udp_probe_denied,
            failure_code,
            report_json,
        })
    }
    fn browser_busy(&self) -> Result<bool, DispatchError> {
        for pid in self
            .job
            .pids()
            .map_err(|_| DispatchError::ControlLost("process-attribution"))?
        {
            let image = match platform::image(pid) {
                Ok(image) => image,
                Err(_) if !self.job.contains(pid) => continue,
                Err(_) => return Err(DispatchError::ControlLost("process-identity")),
            };
            if self
                .package
                .manifest()
                .network
                .programs
                .iter()
                .any(|program| {
                    program.role == ProgramRole::SteamBroker
                        && platform::same_image(&self.package.root().join(&program.path), &image)
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
const REPORT_LIMITATIONS: &str = "No payloads, cookies, URLs, headers or profile contents collected. Destination IP addresses may identify network providers. Records are bounded to the latest 256 samples.";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report<'a> {
    schema_version: u32,
    product: &'static str,
    package_version: &'a str,
    package_mode: &'a str,
    network_enforced: bool,
    monitoring_coverage: &'static str,
    records: &'a VecDeque<Observation>,
    limitations: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn unauthorized_job_peer_is_rejected_before_request_data() {
        let job = Job::create().unwrap();
        assert!(matches!(
            Caller::authenticate(Path::new("C:/package"), &job, std::process::id()),
            Err(DispatchError::Rejected("caller-job"))
        ));
    }
    #[test]
    fn supervision_authorization_matrix_is_exhaustive() {
        for caller in [Caller::Editor, Caller::Worker, Caller::NodeHost] {
            for operation in [
                Operation::Hello,
                Operation::Status,
                Operation::Report,
                Operation::ClearProfile,
                Operation::PolicyFailure,
                Operation::UdpProbe4,
                Operation::UdpProbe6,
            ] {
                assert_eq!(
                    caller.permits(operation),
                    operation != Operation::ClearProfile || caller == Caller::Editor
                );
            }
        }
    }
    #[test]
    fn supervision_rejects_unknown_fields_operations_and_misplaced_arguments() {
        let valid = json!({"operation":"status", "nonce":"a".repeat(64), "probeToken":null, "failureCode":null});
        assert!(Request::parse(valid.clone()).is_ok());
        for (field, value) in [
            ("operation", json!("applyOperation")),
            ("nonce", json!("fake")),
            ("probeToken", json!("a".repeat(64))),
            ("failureCode", json!("x")),
            ("pid", json!(42)),
        ] {
            let mut input = valid.clone();
            input[field] = value;
            assert!(Request::parse(input).is_err());
        }
        for token in ["", "00", &"z".repeat(64)] {
            assert!(
                Request::parse(
                    json!({"operation":"udpProbe4","nonce":"a".repeat(64),"probeToken":token})
                )
                .is_err()
            );
        }
        assert!(
            Request::parse(
                json!({"operation":"udpProbe6","nonce":"a".repeat(64),"probeToken":"A".repeat(64)})
            )
            .is_ok()
        );
        for code in ["", "secret\nvalue", &"a".repeat(65)] {
            assert!(
                Request::parse(
                    json!({"operation":"policyFailure","nonce":"a".repeat(64),"failureCode":code})
                )
                .is_err()
            );
        }
    }
    #[test]
    fn supervision_caller_identity_requires_complete_package_path() {
        let root = Path::new("C:/Program Files/Achimeld/version");
        assert_eq!(
            Caller::for_image(root, &root.join("Achimeld.App.exe")),
            Some(Caller::Editor)
        );
        assert!(Caller::for_image(root, Path::new("C:/Users/test/Achimeld.App.exe")).is_none());
        assert!(Caller::for_image(root, &root.join("Achimeld.App.exe.evil")).is_none());
    }
    #[test]
    fn supervision_reports_have_only_declared_redacted_fields() {
        let records = VecDeque::new();
        let value = serde_json::to_value(Report {
            schema_version: 1,
            product: "Achimeld",
            package_version: "0.1.0",
            package_mode: "development",
            network_enforced: true,
            monitoring_coverage: telemetry::COVERAGE,
            records: &records,
            limitations: REPORT_LIMITATIONS,
        })
        .unwrap();
        assert_eq!(value.as_object().unwrap().len(), 8);
        assert!(value.get("nonce").is_none());
        assert!(value.get("profileDirectory").is_none());
    }
}
