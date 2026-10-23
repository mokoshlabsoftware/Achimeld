//! Reset presentation geometry without touching document, draft or tool state.
use crate::Desktop;
use achimeld_application::EditorPort;

impl Desktop {
    pub(crate) fn reset_window_layout(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        // Includes closed windows and dynamically identified color editors, so
        // they also use their default placement when next opened.
        ctx.memory_mut(|memory| memory.reset_areas());
        self.inspector_panel.reset_layout();
        // Layers also has host-owned saved geometry, independent of egui memory.
        self.reset_layers_position(ctx, port);
        ctx.request_repaint();
    }
}
