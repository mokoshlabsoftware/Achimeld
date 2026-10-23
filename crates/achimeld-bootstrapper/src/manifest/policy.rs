//! Pure manifest identity, version, inventory-path and network-role policy.
use super::{
    PRODUCT,
    model::{Manifest, ProgramRole, TrustConfig},
};
use std::collections::HashSet;
pub fn version(value: &str) -> Result<[u32; 3], String> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid package version".into());
    }
    let mut result = [0; 3];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return Err("noncanonical package version".into());
        }
        result[index] = part.parse().map_err(|_| "package version overflow")?;
    }
    Ok(result)
}

pub fn valid_path(value: &str) -> bool {
    if value.is_empty() || value.len() > 240 || value.contains('\\') || !value.is_ascii() {
        return false;
    }
    value.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.ends_with(['.', ' '])
            && !part.bytes().any(|b| b < 32 || b"<>:\"|?*".contains(&b))
            && !matches!(
                part.split('.')
                    .next()
                    .unwrap()
                    .to_ascii_uppercase()
                    .as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
    })
}

pub fn validate(manifest: &Manifest, trust: &TrustConfig) -> Result<(), String> {
    let runtime: serde_json::Value =
        serde_json::from_str(include_str!("../../../../build/browser-runtime.lock.json"))
            .map_err(|_| "compiled browser lock rejected")?;
    let browser = runtime["executable"]
        .as_str()
        .ok_or("compiled browser identity absent")?;
    if manifest.network.browser_executable != browser {
        return Err("browser does not match the compiled runtime lock".into());
    }
    if manifest.schema_version != 1
        || manifest.product != PRODUCT
        || manifest.platform != "windows-x64"
        || manifest.mode != trust.mode
        || !matches!(manifest.mode.as_str(), "development" | "official")
        || version(&manifest.version)? < version(&trust.minimum_version)?
        || manifest.files.is_empty()
        || manifest.files.len() > 20_000
        || manifest.network.proxy_port < 1024
        || !(manifest.source_commit.len() == 40 || manifest.source_commit.len() == 64)
        || !manifest
            .source_commit
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || (manifest.mode == "official"
            && (manifest.source_dirty || trust.publisher.as_deref().is_none_or(str::is_empty)))
    {
        return Err("package identity or version policy rejected".into());
    }
    let mut paths = HashSet::new();
    for file in &manifest.files {
        if !valid_path(&file.path)
            || !paths.insert(file.path.to_ascii_lowercase())
            || file.sha256.len() != 64
            || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || file.size > 2 * 1024 * 1024 * 1024
        {
            return Err("unsafe or duplicate inventory path".into());
        }
    }
    for required in [
        "Achimeld.exe",
        "Achimeld.App.exe",
        "Achimeld.SteamWorker.exe",
        "Achimeld.NodeHost.exe",
        ".playwright/node/win32_x64/node.exe",
        browser,
    ] {
        if !paths.contains(&required.to_ascii_lowercase()) {
            return Err("required executable absent".into());
        }
    }
    let mut programs = HashSet::new();
    for program in &manifest.network.programs {
        if !valid_path(&program.path)
            || !paths.contains(&program.path.to_ascii_lowercase())
            || !program.path.to_ascii_lowercase().ends_with(".exe")
            || !programs.insert(program.path.to_ascii_lowercase())
        {
            return Err("invalid network program inventory".into());
        }
        let role = match program.path.as_str() {
            "Achimeld.App.exe" | "Achimeld.NodeHost.exe" => ProgramRole::Offline,
            "Achimeld.SteamWorker.exe" | ".playwright/node/win32_x64/node.exe" => {
                ProgramRole::SteamBroker
            }
            path if path == browser => ProgramRole::SteamBroker,
            _ => return Err("unexpected executable in reader network policy".into()),
        };
        if program.role != role {
            return Err("reader executable network role changed".into());
        }
    }
    for file in &manifest.files {
        if file.path.to_ascii_lowercase().ends_with(".exe")
            && file.path != "Achimeld.exe"
            && !manifest
                .network
                .programs
                .iter()
                .any(|program| program.path == file.path)
        {
            return Err("unexpected executable in reader package".into());
        }
        if file.path.to_ascii_lowercase().ends_with(".exe")
            && file.path != "Achimeld.exe"
            && !programs.contains(&file.path.to_ascii_lowercase())
        {
            return Err("executable missing from network policy".into());
        }
    }
    if !manifest.network.programs.iter().any(|p| {
        p.path == manifest.network.browser_executable && p.role == ProgramRole::SteamBroker
    }) || !manifest
        .network
        .programs
        .iter()
        .any(|p| p.path == "Achimeld.SteamWorker.exe" && p.role == ProgramRole::SteamBroker)
    {
        return Err("supervised browser and worker policy absent".into());
    }
    Ok(())
}
