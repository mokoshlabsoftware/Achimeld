//! SEC-PACKAGE-001: authenticate exact bytes, validate policy, inventory/lock files.
mod authenticate;
mod filesystem;
mod model;
pub mod policy;
pub use authenticate::parse_and_authenticate;
pub use filesystem::{reject_reparse, verify};
pub use model::{Manifest, ProgramRole, TrustConfig};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
pub const PRODUCT: &str = "Achimeld";
pub const MAX_MANIFEST: usize = 4 * 1024 * 1024;
pub struct VerifiedPackage {
    root: PathBuf,
    manifest: Manifest,
    // Deny replacement of verified files and directories throughout the supervised session.
    _locks: Vec<File>,
}

impl VerifiedPackage {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use super::model::{InventoryFile, NetworkPolicy, NetworkProgram, TrustKey};
    use super::policy::{valid_path, version};
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use rsa::RsaPublicKey;
    use sha2::Sha256;

    #[test]
    fn authenticated_manifest_rejects_tampering_revocation_and_rollback() {
        use rsa::{
            RsaPrivateKey,
            pkcs8::EncodePublicKey,
            pss::SigningKey,
            signature::{RandomizedSigner, SignatureEncoding},
        };
        let mut rng = rand::thread_rng();
        let private = RsaPrivateKey::new(&mut rng, 3072).unwrap();
        let public = RsaPublicKey::from(&private).to_public_key_der().unwrap();
        let mut trust = TrustConfig {
            mode: "development".into(),
            minimum_version: "1.2.0".into(),
            publisher: None,
            keys: vec![TrustKey {
                key_id: "test".into(),
                public_key: STANDARD.encode(public.as_bytes()),
                revoked: false,
            }],
        };
        let runtime: serde_json::Value =
            serde_json::from_str(include_str!("../../../../build/browser-runtime.lock.json"))
                .unwrap();
        let browser = runtime["executable"].as_str().unwrap();
        let programs = vec![
            NetworkProgram {
                path: "Achimeld.App.exe".into(),
                role: ProgramRole::Offline,
            },
            NetworkProgram {
                path: "Achimeld.SteamWorker.exe".into(),
                role: ProgramRole::SteamBroker,
            },
            NetworkProgram {
                path: browser.into(),
                role: ProgramRole::SteamBroker,
            },
            NetworkProgram {
                path: "Achimeld.NodeHost.exe".into(),
                role: ProgramRole::Offline,
            },
            NetworkProgram {
                path: ".playwright/node/win32_x64/node.exe".into(),
                role: ProgramRole::SteamBroker,
            },
        ];
        let mut manifest = Manifest {
            schema_version: 1,
            product: "Achimeld".into(),
            platform: "windows-x64".into(),
            version: "1.2.3".into(),
            source_commit: "a".repeat(40),
            source_dirty: true,
            mode: "development".into(),
            key_id: "test".into(),
            created_utc: "2026-10-09T00:00:00Z".into(),
            files: [
                "Achimeld.exe",
                "Achimeld.App.exe",
                "Achimeld.SteamWorker.exe",
                browser,
                "Achimeld.NodeHost.exe",
                ".playwright/node/win32_x64/node.exe",
            ]
            .iter()
            .map(|path| InventoryFile {
                path: (*path).into(),
                size: 1,
                sha256: "0".repeat(64),
            })
            .collect(),
            network: NetworkPolicy {
                proxy_port: 42761,
                browser_executable: browser.into(),
                programs,
            },
        };
        let signing = SigningKey::<Sha256>::new(private);
        let sign = |value: &Manifest| {
            let bytes = serde_json::to_vec(value).unwrap();
            let signature = signing
                .sign_with_rng(&mut rand::thread_rng(), &bytes)
                .to_bytes();
            (bytes, signature)
        };
        let (bytes, signature) = sign(&manifest);
        assert!(parse_and_authenticate(&bytes, &signature, &trust).is_ok());
        let mut changed = bytes.clone();
        changed[0] = b' ';
        assert!(parse_and_authenticate(&changed, &signature, &trust).is_err());
        let mut broken = signature.to_vec();
        broken[0] ^= 1;
        assert!(parse_and_authenticate(&bytes, &broken, &trust).is_err());
        trust.keys[0].revoked = true;
        assert!(parse_and_authenticate(&bytes, &signature, &trust).is_err());
        trust.keys[0].revoked = false;
        manifest.version = "1.1.9".into();
        let (bytes, signature) = sign(&manifest);
        assert!(parse_and_authenticate(&bytes, &signature, &trust).is_err());
        manifest.version = "1.2.3".into();
        manifest.network.programs[0].role = ProgramRole::SteamBroker;
        let (bytes, signature) = sign(&manifest);
        assert!(parse_and_authenticate(&bytes, &signature, &trust).is_err());
        manifest.network.programs[0].role = ProgramRole::Offline;
        for (index, role) in [(3, ProgramRole::SteamBroker), (4, ProgramRole::Offline)] {
            let original = manifest.network.programs[index].role;
            manifest.network.programs[index].role = role;
            assert!(policy::validate(&manifest, &trust).is_err());
            manifest.network.programs[index].role = original;
        }
        manifest.files.push(InventoryFile {
            path: "mcp/Achimeld.Mcp.exe".into(),
            size: 1,
            sha256: "0".repeat(64),
        });
        manifest.network.programs.push(NetworkProgram {
            path: "mcp/Achimeld.Mcp.exe".into(),
            role: ProgramRole::Offline,
        });
        assert!(policy::validate(&manifest, &trust).is_err());
        manifest.files.pop();
        manifest.network.programs.pop();
        manifest.network.browser_executable = "browser/other/chrome.exe".into();
        assert!(policy::validate(&manifest, &trust).is_err());
        manifest.network.browser_executable = browser.into();
        manifest.files.push(manifest.files[0].clone());
        let (bytes, signature) = sign(&manifest);
        assert!(parse_and_authenticate(&bytes, &signature, &trust).is_err());
    }
    #[test]
    fn unsafe_paths_are_rejected() {
        for bad in [
            "../secret",
            "/secret",
            "x\\y",
            "x//y",
            "x/../y",
            "C:/app.exe",
            "x:a",
            "NUL.txt",
            "x.",
            "x ",
            "a\0b",
            "com1/dll",
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
        for good in [
            "Achimeld.App.exe",
            ".playwright/node/win32_x64/node.exe",
            "browser/chrome.dll",
        ] {
            assert!(valid_path(good));
        }
    }
    #[test]
    fn downgrade_comparison_is_numeric() {
        assert!(version("0.10.0").unwrap() > version("0.9.9").unwrap());
        for bad in ["0.01.0", "1.2", "1.2.3.4", "1.2.3-beta", "1.-2.3"] {
            assert!(version(bad).is_err());
        }
    }
}
