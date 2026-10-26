use achimeld_browser::worker::SteamReader;
use achimeld_core::SteamAppId;
use achimeld_security::cancellation::{self, Cancellation};
use achimeld_security::supervision::Session;
use std::time::Duration;
mod app_dialog;
mod art_bank;
mod desktop;
mod diagnostics;
mod editor_state;
mod icon_export;
mod legacy_settings;
mod preview_renderer;
mod recent_projects;
mod settings_location;
mod sync_previews;
mod synchronization;
mod system_fonts;
#[cfg(test)]
#[path = "../../../build/test_output.rs"]
mod test_output;
mod translations;
mod ui_fonts;
fn run(stage: &mut &'static str) -> Result<(), Box<dyn std::error::Error>> {
    let session = Session::connect()?;
    achimeld_application::diagnostics::record(
        "INFO",
        "application",
        "Supervised session connected",
    );
    *stage = "console-control";
    cancellation::install_console_handler()?;
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [command] if command == "--security-user-self-test" => {
            if session.status().network_enforced || session.status().package_mode != "development" {
                return Err(
                    "User-session test requires an unprivileged development session".into(),
                );
            }
            *stage = "user-worker-connect";
            let mut reader = SteamReader::connect(session, Cancellation::default())?;
            // Stop exercises the real authenticated receiving worker without
            // starting Chromium, opening Steam, or claiming OS network denial.
            *stage = "user-worker-stop";
            reader.stop()?;
        }
        [command, app_id] if command == "--read-steam" => {
            let app_id = SteamAppId::new(app_id.parse()?)?;
            *stage = "worker-connect";
            let mut reader = SteamReader::connect(session, Cancellation::default())?;
            *stage = "browser-start";
            reader.start()?;
            *stage = "app-navigation";
            reader.open_app(app_id)?;
            println!(
                "Sign into Steam in the dedicated browser if requested. This sign-in persists in Achimeld's dedicated profile. Reading app {app_id}; no draft changes will be made."
            );
            *stage = "manual-authentication";
            reader.wait_for_authentication(Duration::from_secs(600))?;
            // A login redirect can finish on the dashboard. Return to the approved app route.
            reader.open_app(app_id)?;
            *stage = "configuration-read";
            let snapshot = reader.read_snapshot(app_id)?;
            let session_id = reader.session_id().to_owned();
            reader.stop()?;
            *stage = "snapshot-save";
            let output = save_read(app_id, &session_id, &snapshot)?;
            println!(
                "Read {} stats and {} achievements. Snapshot saved to {}",
                snapshot.stats.len(),
                snapshot.achievements.len(),
                output.display()
            );
        }
        [command]
            if matches!(
                command.as_str(),
                "--security-self-test" | "--security-self-test-hold"
            ) =>
        {
            let hold = command == "--security-self-test-hold";
            *stage = "worker-connect";
            let mut reader = SteamReader::connect(session, Cancellation::default())?;
            *stage = "worker-network-self-test";
            reader.development_network_test(hold)?;
            if hold {
                *stage = "held-supervision";
                reader.hold_healthy(Duration::from_secs(120))?;
            }
            reader.stop()?;
        }
        [] => {
            *stage = "desktop-window";
            desktop::run(session)?;
        }
        _ => return Err("Unsupported supervised editor command".into()),
    }
    Ok(())
}

fn save_read(
    app_id: SteamAppId,
    session_id: &str,
    snapshot: &achimeld_browser::snapshot::Snapshot,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    use std::io::Write;
    if session_id.len() != 64 || !session_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid read session identity".into());
    }
    let directory = storage_directory("RustReads")?;
    let output = directory.join(format!("{}-{session_id}.json", app_id.value()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    let mut json = serde_json::to_vec_pretty(snapshot)?;
    json.push(b'\n');
    file.write_all(&json)?;
    file.sync_all()?;
    Ok(output)
}
fn storage_directory(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let local = std::path::PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or("Local application storage unavailable")?,
    );
    achimeld_security::windows::reject_reparse(&local)?;
    let product = local.join("Achimeld");
    if product.exists() {
        achimeld_security::windows::reject_reparse(&product)?;
    } else {
        std::fs::create_dir(&product)?;
    }
    let directory = product.join(name);
    if directory.exists() {
        achimeld_security::windows::reject_reparse(&directory)?;
    } else {
        std::fs::create_dir(&directory)?;
    }
    achimeld_security::windows::reject_reparse(&directory)?;
    Ok(directory)
}
fn record_failure(
    stage: &str,
    error: &(dyn std::error::Error + 'static),
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    // Only static Rust security messages are retained. Never serialize arbitrary
    // worker/Playwright exceptions, DOM, URLs, credentials or authentication state.
    let message = error
        .downcast_ref::<achimeld_security::SecurityError>()
        .map(|error| error.0)
        .unwrap_or("Application operation failed");
    let directory = storage_directory("RustDiagnostics")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let output = directory.join(format!("failure-{}-{stamp}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let json = serde_json::to_vec_pretty(&serde_json::json!({"stage":stage,"message":message}))?;
    file.write_all(&json)?;
    file.sync_all()?;
    Ok(())
}
fn main() {
    diagnostics::initialize();
    let version = include_str!("../../../CHANGELOG.md")
        .lines()
        .find_map(|line| {
            line.strip_prefix("## ")
                .and_then(|line| line.split_whitespace().next())
        })
        .unwrap_or("unknown");
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    achimeld_application::diagnostics::record(
        "INFO",
        "application",
        &format!(
            "Starting Achimeld; version={version}; pid={}; UTC unix time={started}",
            std::process::id()
        ),
    );
    let mut stage = "supervised-startup";
    if let Err(error) = run(&mut stage) {
        achimeld_application::diagnostics::record(
            "ERROR",
            "application",
            &format!("Application stopped during {stage}"),
        );
        eprintln!("Achimeld cannot start: {error}");
        if stage != "supervised-startup" {
            let _ = record_failure(stage, error.as_ref());
        }
        std::process::exit(1);
    }
    achimeld_application::diagnostics::record("INFO", "application", "Achimeld exited normally");
}
