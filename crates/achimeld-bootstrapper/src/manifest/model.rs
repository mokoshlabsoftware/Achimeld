//! Strict manifest wire types; these are unverified data until authentication.
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustConfig {
    pub mode: String,
    pub minimum_version: String,
    pub publisher: Option<String>,
    pub keys: Vec<TrustKey>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustKey {
    pub key_id: String,
    pub public_key: String,
    pub revoked: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub product: String,
    pub platform: String,
    pub version: String,
    pub source_commit: String,
    pub source_dirty: bool,
    pub mode: String,
    pub key_id: String,
    pub created_utc: String,
    pub files: Vec<InventoryFile>,
    pub network: NetworkPolicy,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventoryFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkPolicy {
    pub proxy_port: u16,
    pub browser_executable: String,
    pub programs: Vec<NetworkProgram>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkProgram {
    pub path: String,
    pub role: ProgramRole,
}
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProgramRole {
    Offline,
    SteamBroker,
}
