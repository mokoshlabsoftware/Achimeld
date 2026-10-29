pub(crate) fn configure(ctx: &eframe::egui::Context) {
    // Use the installed Windows UI faces without redistributing system fonts.
    let faces = std::env::var_os("SystemRoot").and_then(|root| {
        let fonts = std::path::PathBuf::from(root).join("Fonts");
        Some([
            std::fs::read(fonts.join("segoeui.ttf")).ok()?,
            std::fs::read(fonts.join("segoeuib.ttf")).ok()?,
        ])
    });
    let console = std::env::var_os("SystemRoot").and_then(|root| {
        std::fs::read(std::path::PathBuf::from(root).join("Fonts/consola.ttf")).ok()
    });
    achimeld_ui::configure_fonts_with_monospace(ctx, faces, console);
}
