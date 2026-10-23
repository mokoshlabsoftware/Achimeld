//! Bounded legacy IPC and authenticated Windows supervision.
pub mod cancellation;
#[cfg(windows)]
pub mod editor_transport;
pub mod storage;
pub mod supervision;
#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub mod windows_arguments;
pub mod wire;
#[cfg(windows)]
pub mod worker_transport;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityError(pub &'static str);
impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for SecurityError {}
