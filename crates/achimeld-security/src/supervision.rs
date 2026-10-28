use crate::SecurityError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Wire spelling is shared with the retained C# client; no arbitrary operation strings.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    Hello,
    Status,
    ClearProfile,
    UdpProbe4,
    UdpProbe6,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub operation: Operation,
    pub nonce: String,
    pub probe_token: Option<String>,
    pub failure_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Status {
    pub nonce: String,
    pub session_id: String,
    pub supervisor_pid: u32,
    pub package_version: String,
    pub package_mode: String,
    pub network_enforced: bool,
    pub monitoring_active: bool,
    pub monitoring_coverage: String,
    pub proxy_port: u16,
    pub browser_executable: PathBuf,
    pub profile_directory: PathBuf,
    pub probe_port: u16,
    pub udp_probe_denied: Option<bool>,
    pub failure_code: Option<String>,
    pub report_json: Option<String>,
}

pub fn validate_pipe_name(name: &str) -> Result<(), SecurityError> {
    let session = name
        .strip_prefix("Achimeld.Security.")
        .ok_or(SecurityError("Start Achimeld through its bootstrapper"))?;
    if session.len() != 64 || !session.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SecurityError("Invalid supervision endpoint"));
    }
    Ok(())
}

pub fn validate_response(
    status: &Status,
    nonce: &str,
    server_pid: u32,
    expected_session: Option<&str>,
) -> Result<(), SecurityError> {
    if status.nonce != nonce || status.supervisor_pid != server_pid || server_pid == 0 {
        return Err(SecurityError("Unauthenticated supervision response"));
    }
    if status.session_id.len() != 64
        || !status.session_id.bytes().all(|b| b.is_ascii_hexdigit())
        || expected_session.is_some_and(|id| id != status.session_id)
    {
        return Err(SecurityError("Supervision session changed"));
    }
    if !status.monitoring_active
        || status.failure_code.is_some()
        || status.proxy_port < 1024
        || status.probe_port < 1024
    {
        return Err(SecurityError("Mandatory security controls unavailable"));
    }
    if !matches!(status.package_mode.as_str(), "development" | "official") {
        return Err(SecurityError("Unknown package mode"));
    }
    Ok(())
}

/// SEC-SUPERVISION-001: A health response cannot change the authenticated configuration.
pub fn validate_continuity(previous: &Status, next: &Status) -> Result<(), SecurityError> {
    if previous.session_id != next.session_id
        || previous.supervisor_pid != next.supervisor_pid
        || previous.package_version != next.package_version
        || previous.package_mode != next.package_mode
        || previous.network_enforced != next.network_enforced
        || previous.proxy_port != next.proxy_port
        || previous.probe_port != next.probe_port
        || previous.browser_executable != next.browser_executable
        || previous.profile_directory != next.profile_directory
        || previous.monitoring_coverage != next.monitoring_coverage
    {
        return Err(SecurityError("Supervision configuration changed"));
    }
    Ok(())
}

pub struct Session {
    pipe_name: String,
    package_root: PathBuf,
    status: Status,
}

impl Session {
    pub fn connect() -> Result<Self, SecurityError> {
        #[cfg(windows)]
        {
            let pipe_name = std::env::var("ACHIMELD_SUPERVISION_PIPE")
                .map_err(|_| SecurityError("Start Achimeld through its bootstrapper"))?;
            validate_pipe_name(&pipe_name)?;
            let executable = std::env::current_exe()
                .map_err(|_| SecurityError("Application identity unavailable"))?;
            let launch_root = executable
                .parent()
                .ok_or(SecurityError("Package root unavailable"))?
                .to_path_buf();
            let package_root = launch_root
                .canonicalize()
                .map_err(|_| SecurityError("Package root unavailable"))?;
            let configured = std::env::var_os("ACHIMELD_PACKAGE_ROOT")
                .ok_or(SecurityError("Package root missing"))?;
            let configured = PathBuf::from(configured)
                .canonicalize()
                .map_err(|_| SecurityError("Package root unavailable"))?;
            if !crate::windows::same_path(&package_root, &configured) {
                return Err(SecurityError("Supervision belongs to another package"));
            }
            let status =
                crate::windows::exchange(&pipe_name, &package_root, Operation::Hello, None)?;
            validate_response(
                &status,
                &status.nonce,
                status.supervisor_pid,
                pipe_name.strip_prefix("Achimeld.Security."),
            )?;
            crate::windows::validate_paths(&package_root, &status)?;
            Ok(Self {
                pipe_name,
                // Keep the normal Win32 spelling for the C# peer's AppContext identity check.
                package_root: launch_root,
                status,
            })
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError("Supervision requires Windows"))
        }
    }

    pub fn package_root(&self) -> &Path {
        &self.package_root
    }
    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn require_healthy(&mut self) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            let next = crate::windows::exchange(
                &self.pipe_name,
                &self.package_root,
                Operation::Status,
                None,
            )?;
            validate_response(
                &next,
                &next.nonce,
                self.status.supervisor_pid,
                Some(&self.status.session_id),
            )?;
            crate::windows::validate_paths(&self.package_root, &next)?;
            validate_continuity(&self.status, &next)?;
            self.status = next;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError("Supervision requires Windows"))
        }
    }

    /// The supervisor owns the fixed path and refuses deletion while its browser is active.
    pub fn clear_profile(&mut self) -> Result<(), SecurityError> {
        self.require_healthy()?;
        #[cfg(windows)]
        {
            let next = crate::windows::exchange(
                &self.pipe_name,
                &self.package_root,
                Operation::ClearProfile,
                None,
            )?;
            crate::windows::validate_paths(&self.package_root, &next)?;
            validate_continuity(&self.status, &next)?;
            self.status = next;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError("Profile deletion requires Windows"))
        }
    }

    pub fn check_network_denial(&mut self) -> Result<(), SecurityError> {
        if !self.status.network_enforced {
            return Err(SecurityError(
                "Windows network isolation is not enabled for this session",
            ));
        }
        #[cfg(windows)]
        {
            crate::windows::check_network_denial(&self.pipe_name, &self.package_root, &self.status)
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError("Network enforcement requires Windows"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_deletion_uses_the_existing_fixed_wire_operation_without_a_path() {
        let value = serde_json::to_value(Request {
            operation: Operation::ClearProfile,
            nonce: "challenge".into(),
            probe_token: None,
            failure_code: None,
        })
        .unwrap();
        assert_eq!(value["operation"], "clearProfile");
        assert!(value.get("path").is_none());
        assert_eq!(value.as_object().unwrap().len(), 4);
    }
    fn healthy() -> Status {
        serde_json::from_value(serde_json::json!({"nonce":"challenge", "sessionId":"ab".repeat(32), "supervisorPid":123,
            "packageVersion":"0.1.0", "packageMode":"development", "networkEnforced":true, "monitoringActive":true,
            "monitoringCoverage":"polling", "proxyPort":42761, "browserExecutable":"browser/chrome.exe",
            "profileDirectory":"dedicated", "probePort":42763, "udpProbeDenied":null, "failureCode":null, "reportJson":null})).unwrap()
    }
    #[test]
    fn forged_stale_and_unhealthy_sessions_fail_closed() {
        let mut status = healthy();
        assert!(validate_response(&status, "challenge", 123, Some(&"ab".repeat(32))).is_ok());
        assert!(validate_response(&status, "wrong", 123, None).is_err());
        assert!(validate_response(&status, "challenge", 124, None).is_err());
        assert!(validate_response(&status, "challenge", 123, Some(&"cd".repeat(32))).is_err());
        status.network_enforced = false;
        // User sessions truthfully advertise no Windows network enforcement.
        assert!(validate_response(&status, "challenge", 123, None).is_ok());
        assert!(validate_continuity(&healthy(), &status).is_err());
        assert!(validate_continuity(&status, &healthy()).is_err());
        status.monitoring_active = false;
        assert!(validate_response(&status, "challenge", 123, None).is_err());
        status.monitoring_active = true;
        status.network_enforced = true;
        status.failure_code = Some("control-loss".into());
        assert!(validate_response(&status, "challenge", 123, None).is_err());
    }
    #[test]
    fn endpoint_is_a_fixed_local_supervision_name() {
        assert!(validate_pipe_name(&format!("Achimeld.Security.{}", "a".repeat(64))).is_ok());
        for name in [
            "",
            "Achimeld.Security.fake",
            "\\\\remote\\pipe\\Achimeld.Security.fake",
        ] {
            assert!(validate_pipe_name(name).is_err());
        }
    }
    #[test]
    fn health_response_cannot_change_session_or_configuration() {
        let previous = healthy();
        assert!(validate_continuity(&previous, &healthy()).is_ok());
        for field in [
            "sessionId",
            "packageVersion",
            "packageMode",
            "browserExecutable",
            "profileDirectory",
            "monitoringCoverage",
        ] {
            let mut value = serde_json::json!({"nonce":"challenge", "sessionId":"ab".repeat(32), "supervisorPid":123,
                "packageVersion":"0.1.0", "packageMode":"development", "networkEnforced":true, "monitoringActive":true,
                "monitoringCoverage":"polling", "proxyPort":42761, "browserExecutable":"browser/chrome.exe",
                "profileDirectory":"dedicated", "probePort":42763});
            value[field] = "changed".into();
            let next: Status = serde_json::from_value(value).unwrap();
            assert!(validate_continuity(&previous, &next).is_err(), "{field}");
        }
        let mut next = healthy();
        next.proxy_port += 1;
        assert!(validate_continuity(&previous, &next).is_err());
        let mut next = healthy();
        next.probe_port += 1;
        assert!(validate_continuity(&previous, &next).is_err());
    }
}
