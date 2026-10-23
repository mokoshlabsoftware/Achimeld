//! Offline diagnostic utility: no browser worker or live website actions.
use achimeld_core::SteamAppId;
use std::{io::Read, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [kind, path] if kind == "project" => println!(
            "{}",
            serde_json::to_string_pretty(&achimeld_persistence::inspect_file(Path::new(path))?)?
        ),
        [kind, path, app_id] if kind == "snapshot" => {
            let file = std::fs::File::open(path)?;
            let mut text = String::new();
            file.take(16 * 1024 * 1024 + 1).read_to_string(&mut text)?;
            let snapshot =
                achimeld_browser::snapshot::parse(&text, SteamAppId::new(app_id.parse()?)?)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        _ => {
            return Err(
                "Usage: achimeld-inspect project FILE.achv | snapshot NORMALIZED.html APP_ID"
                    .into(),
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Inspection failed: {error}");
        std::process::exit(1);
    }
}
