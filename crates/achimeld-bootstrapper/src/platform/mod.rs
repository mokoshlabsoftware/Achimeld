//! Narrow Windows adapters; policy and supervision dispatch live outside this module.
mod handles;
mod identity;
mod package_access;
mod pipe;
mod process;
mod profile;

pub use handles::descriptor;
pub use identity::{MonitoredProcess, elevated, image, monitored_image, same_image};
pub use package_access::require_protected_package;
pub use pipe::Pipe;
pub use process::Job;
pub use profile::{clear_profile, profile};

#[cfg(test)]
mod tests;
