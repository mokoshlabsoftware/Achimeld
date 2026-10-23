//! Bounded project inventory and recoverable project/catalog transactions.
#![forbid(unsafe_code)]
pub mod project;
#[cfg(test)]
#[path = "../../../build/test_output.rs"]
mod test_output;
use serde::Serialize;
use serde_json::Value;
use std::{io::Read, path::Path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInventory {
    pub format: String,
    pub schema_version: u32,
    pub stats: usize,
    pub achievements: usize,
    pub has_localization_catalog: bool,
    /// Original bytes remain intact; callers cannot save through this inspector.
    #[serde(skip)]
    pub original_json: Vec<u8>,
}
pub fn inspect_bytes(bytes: &[u8]) -> Result<ProjectInventory, &'static str> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Project JSON exceeds its size limit");
    }
    achimeld_security::wire::check_json_depth(bytes)
        .map_err(|_| "Project JSON exceeds its nesting limit")?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "Invalid project JSON")?;
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .ok_or("Missing project format")?;
    // Legacy format strings are intentionally checked rather than guessed.
    if format != "com.achimeld.project" && format != "com.achiever.project" {
        return Err("Unsupported project format");
    }
    let schema = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or("Missing schema version")?;
    if schema > 1 {
        return Err("Unsupported future project schema; preserve the original file");
    }
    let stats = value
        .get("stats")
        .and_then(Value::as_array)
        .ok_or("Missing stats array")?
        .len();
    let achievements = value
        .get("achievements")
        .and_then(Value::as_array)
        .ok_or("Missing achievements array")?
        .len();
    Ok(ProjectInventory {
        format: format.into(),
        schema_version: schema as u32,
        stats,
        achievements,
        has_localization_catalog: value.pointer("/assets/localizationCatalog").is_some(),
        original_json: bytes.to_vec(),
    })
}
pub fn inspect_file(path: &Path) -> Result<ProjectInventory, &'static str> {
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("achv"))
    {
        return Err("Project must use the .achv extension");
    }
    let file = std::fs::File::open(path).map_err(|_| "Project file unavailable")?;
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Project file unreadable")?;
    inspect_bytes(&bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_preserves_original_and_refuses_future_schema() {
        let bytes = br#"{"format":"com.achimeld.project","schemaVersion":1,"stats":[],"achievements":[],"futureAsset":{"opaque":true}}"#;
        let inventory = inspect_bytes(bytes).unwrap();
        assert_eq!(inventory.original_json, bytes);
        assert!(inspect_bytes(br#"{"format":"com.achimeld.project","schemaVersion":2,"stats":[],"achievements":[]}"#).is_err());
    }
}
