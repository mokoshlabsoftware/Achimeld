use std::{env, fs, path::PathBuf};
#[path = "../../build/icon_resource.rs"]
mod icon_resource;

fn main() {
    icon_resource::link_icon("Achimeld");
    println!("cargo:rerun-if-env-changed=ACHIMELD_TRUST_CONFIG");
    let content = if let Ok(path) = env::var("ACHIMELD_TRUST_CONFIG") {
        println!("cargo:rerun-if-changed={path}");
        fs::read_to_string(path).expect("read trusted verification configuration")
    } else {
        // An unconfigured binary builds for tests but cannot authenticate or launch a package.
        r#"{"mode":"unconfigured","minimumVersion":"0.1.0","publisher":null,"keys":[]}"#.to_owned()
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("trusted.json"),
        content,
    )
    .unwrap();
}
