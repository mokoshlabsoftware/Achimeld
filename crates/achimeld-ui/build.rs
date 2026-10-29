fn main() {
    println!("cargo:rerun-if-changed=assets/particle-shapes.png");
    let image = image::load_from_memory(include_bytes!("assets/particle-shapes.png"))
        .expect("embedded particle icons")
        .thumbnail(1024, 1024)
        .into_rgba8();
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    std::fs::write(output.join("particle-shapes.rgba"), image.as_raw())
        .expect("write particle icon pixels");
    std::fs::write(
        output.join("particle-shapes.rs"),
        format!(
            "([{}, {}], include_bytes!(concat!(env!(\"OUT_DIR\"), \"/particle-shapes.rgba\")))",
            image.width(),
            image.height()
        ),
    )
    .expect("write particle icon dimensions");
}
