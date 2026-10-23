use std::process::Command;

#[test]
fn playwright_node_host_refuses_unsupervised_and_forged_launches() {
    for endpoint in ["", "Achimeld.Security.forged"] {
        let result = Command::new(env!("CARGO_BIN_EXE_achimeld-node-host"))
            .env("ACHIMELD_SUPERVISION_PIPE", endpoint)
            .env_remove("ACHIMELD_PACKAGE_ROOT")
            .args(["arbitrary.js", "run-driver"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(125));
    }
}

#[test]
fn editor_refuses_unsupervised_startup() {
    let output = Command::new(env!("CARGO_BIN_EXE_achimeld-app"))
        .env_remove("ACHIMELD_SUPERVISION_PIPE")
        .env_remove("ACHIMELD_PACKAGE_ROOT")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("bootstrapper"));
}
#[test]
fn forged_supervision_endpoint_does_not_enable_a_bypass() {
    let output = Command::new(env!("CARGO_BIN_EXE_achimeld-app"))
        .env("ACHIMELD_SUPERVISION_PIPE", "Achimeld.Security.forged")
        .env("ACHIMELD_PACKAGE_ROOT", ".")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid supervision endpoint"));
}
#[test]
fn offline_inspector_reads_the_committed_steam_snapshot() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/steamworks/normalized-2026-09.html");
    let output = Command::new(env!("CARGO_BIN_EXE_achimeld-inspect"))
        .arg("snapshot")
        .arg(fixture)
        .arg("480")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document["contentFingerprint"],
        "54e49cf1369a6912d1b9d43c3727793eca3763887d270c717777962d2776acdb"
    );
}
