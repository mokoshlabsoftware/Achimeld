//! Typed read and reviewed synchronization bridge to the supervised C# worker.
use crate::{
    protocol::{ManagedPageDocument, ReadCommand, WorkerResponse},
    snapshot::Snapshot,
};
use achimeld_core::SteamAppId;
use achimeld_security::{
    SecurityError,
    cancellation::{self, Cancellation},
    supervision::Session,
};
use rand::RngCore;
use serde::{Deserialize, de::DeserializeOwned};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AuthenticationState {
    Unknown,
    SignedOut,
    AuthenticationRequired,
    PermissionDenied,
    Authenticated,
}

fn request_id() -> String {
    let mut random = [0; 16];
    rand::thread_rng().fill_bytes(&mut random);
    random[6] = (random[6] & 0x0f) | 0x40;
    random[8] = (random[8] & 0x3f) | 0x80;
    let value = hex::encode(random);
    format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    )
}

#[cfg(windows)]
pub struct SteamReader {
    session: Session,
    transport: Transport,
    cancellation: Cancellation,
    calls: usize,
}
// An open connection always owns both resources; Closed cannot retain a live child.
#[cfg(windows)]
enum Transport {
    Open {
        pipe: achimeld_security::worker_transport::PrivatePipe,
        child: achimeld_security::worker_transport::WorkerChild,
    },
    Closed,
}
const MAXIMUM_READ_CALLS: usize = 9_000;
const WORKER_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const WORKER_CALL_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(windows)]
impl SteamReader {
    pub fn connect(
        mut session: Session,
        cancellation: Cancellation,
    ) -> Result<Self, SecurityError> {
        use achimeld_security::worker_transport::{PrivatePipe, WorkerChild};
        let started = Instant::now();
        log::info!("Connecting supervised Steam worker");
        cancellation.check()?;
        cancellation::check_console()?;
        session.require_healthy()?;
        if session.status().network_enforced {
            session.check_network_denial()?;
        }
        let name = format!("Achimeld.Steam.{}", request_id().replace('-', ""));
        let mut pipe = PrivatePipe::create(&name)?;
        let child = WorkerChild::spawn(session.package_root(), &name)?;
        let mut last_health = Instant::now() - Duration::from_secs(2);
        let mut check = || {
            cancellation.check()?;
            cancellation::check_console()?;
            child.require_alive()?;
            if last_health.elapsed() >= Duration::from_secs(1) {
                session.require_healthy()?;
                last_health = Instant::now();
            }
            Ok(())
        };
        // A failure drops the child Job before returning; no retry/restart is attempted.
        pipe.accept(child.pid(), WORKER_CONNECT_TIMEOUT, &mut check)?;
        session.require_healthy()?;
        log::info!(
            "Steam worker connected in {} ms",
            started.elapsed().as_millis()
        );
        Ok(Self {
            session,
            transport: Transport::Open { pipe, child },
            cancellation,
            calls: 0,
        })
    }
    fn fail_closed(&mut self) {
        self.transport = Transport::Closed;
    }
    fn call<T: DeserializeOwned>(&mut self, command: ReadCommand) -> Result<T, SecurityError> {
        let started = Instant::now();
        let request = command.request(request_id());
        let operation = request.operation.as_str();
        // Polling uses the separate authentication state-change log below.
        let report = operation != "authenticationState";
        if report {
            log::info!(
                "Worker {operation}: started · call {} · transport deadline {} ms",
                self.calls + 1,
                WORKER_CALL_TIMEOUT.as_millis()
            );
        }
        if matches!(self.transport, Transport::Closed) || self.calls >= MAXIMUM_READ_CALLS {
            self.fail_closed();
            return Err(SecurityError("Steam read session is closed"));
        }
        let result = (|| {
            self.cancellation.check()?;
            cancellation::check_console()?;
            self.session.require_healthy()?;
            let Transport::Open { pipe, child } = &mut self.transport else {
                return Err(SecurityError("Steam read session is closed"));
            };
            child.require_alive()?;
            let mut last_health = Instant::now();
            let mut last_progress = Instant::now();
            let mut check = || {
                self.cancellation.check()?;
                cancellation::check_console()?;
                child.require_alive()?;
                if last_health.elapsed() >= Duration::from_secs(1) {
                    self.session.require_healthy()?;
                    last_health = Instant::now();
                }
                if report && last_progress.elapsed() >= Duration::from_secs(5) {
                    log::info!(
                        "Worker {operation}: awaiting response · elapsed {} ms · worker alive · supervisor healthy",
                        started.elapsed().as_millis()
                    );
                    last_progress = Instant::now();
                }
                Ok(())
            };
            let response: WorkerResponse =
                pipe.exchange(&request, WORKER_CALL_TIMEOUT, &mut check)?;
            if let Some(details) = response.failure_details(&request.request_id) {
                log::error!("{details}");
            }
            let data = response
                .accept(&request.request_id)
                .map_err(SecurityError)?;
            self.session.require_healthy()?;
            serde_json::from_value(data)
                .map_err(|_| SecurityError("Steam worker returned an invalid result"))
        })();
        let result = result.map_err(|error| {
            if let Transport::Open { child, .. } = &self.transport
                && let Some(message) = child
                    .diagnostic_exit_code()
                    .and_then(crate::protocol::exit_diagnostic)
            {
                return SecurityError(message);
            }
            error
        });
        self.calls += 1;
        match &result {
            Ok(_) if report => log::info!(
                "Worker {operation}: completed in {} ms",
                started.elapsed().as_millis()
            ),
            Err(error) => log::error!(
                "Worker {operation}: failed after {} ms · {error}",
                started.elapsed().as_millis()
            ),
            _ => {}
        }
        if result.is_err() {
            self.fail_closed();
        }
        result
    }
    fn boolean(&mut self, command: ReadCommand) -> Result<(), SecurityError> {
        if !self.call::<bool>(command)? {
            self.fail_closed();
            return Err(SecurityError(
                "Steam worker did not confirm the read command",
            ));
        }
        Ok(())
    }
    pub fn start(&mut self) -> Result<(), SecurityError> {
        self.boolean(ReadCommand::Start)
    }
    pub fn authenticate(&mut self) -> Result<(), SecurityError> {
        self.boolean(ReadCommand::Authenticate)
    }
    pub fn authentication_state(&mut self) -> Result<AuthenticationState, SecurityError> {
        self.call(ReadCommand::AuthenticationState)
    }
    pub fn open_app(&mut self, app_id: SteamAppId) -> Result<(), SecurityError> {
        self.boolean(ReadCommand::Navigate(app_id))
    }
    pub fn read_snapshot(&mut self, app_id: SteamAppId) -> Result<Snapshot, SecurityError> {
        let document: ManagedPageDocument = self.call(ReadCommand::ReadConfiguration(app_id))?;
        let result = crate::portal::parse_document(&document, app_id).map_err(SecurityError);
        if result.is_err() {
            self.fail_closed();
        }
        result
    }
    pub fn prepare_sync(
        &mut self,
        project: serde_json::Value,
        catalog: serde_json::Value,
        icons: serde_json::Value,
    ) -> Result<serde_json::Value, SecurityError> {
        self.call(ReadCommand::PrepareSync {
            project,
            catalog,
            icons,
        })
    }
    pub fn begin_sync(
        &mut self,
        token: String,
        choices: serde_json::Value,
    ) -> Result<serde_json::Value, SecurityError> {
        self.call(ReadCommand::BeginSync { token, choices })
    }
    pub fn sync_step(
        &mut self,
        token: String,
        key: String,
        achieved: Option<String>,
        unachieved: Option<String>,
    ) -> Result<serde_json::Value, SecurityError> {
        self.call(ReadCommand::SyncStep {
            token,
            key,
            achieved,
            unachieved,
        })
    }
    pub fn finish_sync(&mut self, token: String) -> Result<serde_json::Value, SecurityError> {
        self.call(ReadCommand::FinishSync { token })
    }
    pub fn require_sync_budget(&mut self, steps: usize) -> Result<(), SecurityError> {
        // Reserve finish + stop before the first mutation, including time spent
        // polling manual authentication. Never discover this budget mid-plan.
        if steps
            .checked_add(2)
            .and_then(|count| self.calls.checked_add(count))
            .is_none_or(|total| total > MAXIMUM_READ_CALLS)
        {
            self.fail_closed();
            return Err(SecurityError(
                "This synchronization selection exceeds the session budget; select fewer entries in a fresh plan",
            ));
        }
        Ok(())
    }
    pub fn wait_for_authentication(&mut self, timeout: Duration) -> Result<(), SecurityError> {
        let deadline = Instant::now() + timeout;
        let mut previous = None;
        loop {
            let state = self.authentication_state()?;
            if previous != Some(state) {
                log::info!("Steam authentication: {state:?}");
                previous = Some(state);
            }
            match state {
                AuthenticationState::Authenticated => return Ok(()),
                AuthenticationState::PermissionDenied => {
                    self.fail_closed();
                    return Err(SecurityError(
                        "Steam account does not have access to this app",
                    ));
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                self.fail_closed();
                return Err(SecurityError("Manual Steam sign-in timed out"));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    pub fn stop(&mut self) -> Result<(), SecurityError> {
        let result = if matches!(self.transport, Transport::Closed) {
            Err(SecurityError("Steam read session is closed"))
        } else {
            self.boolean(ReadCommand::Stop)
        };
        self.fail_closed();
        result
    }
    pub fn session_id(&self) -> &str {
        &self.session.status().session_id
    }
    pub fn development_network_test(&mut self, hold: bool) -> Result<(), SecurityError> {
        if self.session.status().package_mode != "development"
            || !self.session.status().network_enforced
        {
            return Err(SecurityError("Development network test unavailable"));
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct ResultData {
            passed: bool,
            stage: String,
        }
        let result: ResultData = self.call(ReadCommand::NetworkSelfTest { hold })?;
        if !result.passed || result.stage != "private-browser-host" {
            self.fail_closed();
            return Err(SecurityError("Worker/browser network acceptance failed"));
        }
        Ok(())
    }
    pub fn hold_healthy(&mut self, timeout: Duration) -> Result<(), SecurityError> {
        let result = (|| {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                self.cancellation.check()?;
                cancellation::check_console()?;
                self.session.require_healthy()?;
                let Transport::Open { child, .. } = &self.transport else {
                    return Err(SecurityError("Steam read session is closed"));
                };
                child.require_alive()?;
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        })();
        if result.is_err() {
            self.fail_closed();
        }
        result
    }
}
#[cfg(windows)]
impl Drop for SteamReader {
    fn drop(&mut self) {
        self.fail_closed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_ids_are_distinct_nonempty_canonical_v4_guids() {
        let a = request_id();
        let b = request_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
        assert!("89ab".contains(&a[19..20]));
    }
}
