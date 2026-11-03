//! Windowless owner for the same supervised editor and serialized MCP engine.
use super::*;

pub fn run(mut session: Session) -> Result<(), Box<dyn std::error::Error>> {
    session.require_healthy()?;
    let ctx = egui::Context::default();
    crate::ui_fonts::configure(&ctx);
    let mut host = Host::new(ctx.clone());
    host.headless = true;
    let server = automation::Server::start(ctx.clone(), host.mcp_health_allowed.clone())?;
    let mut engine = automation::Engine::default();
    let mut ui = achimeld_ui::Desktop::default();
    let diagnostics = crate::diagnostics::Monitor::new();
    achimeld_application::diagnostics::record("INFO", "application", "Headless MCP host ready");
    while !host.exit_allowed {
        session.require_healthy()?;
        let activity = diagnostics.frame();
        // Advance egui's bounded frame state without creating a native window,
        // painting panels or invoking file pickers.
        let _ = ctx.run_ui(Default::default(), |_| {
            host.poll_synchronization();
            host.poll_read();
            host.poll_browser_profile();
            host.poll_export();
            host.poll_translations();
            host.poll_art();
            host.previews.poll();
            host.poll_viewer();
            engine.poll(&server, &mut host, &mut ui);
        });
        server.note_logic_tick();
        drop(activity);
        std::thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
