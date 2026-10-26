//! Selectable session output; the host owns every operation described here.
use crate::{Desktop, buttons, localization::text};
use achimeld_application::diagnostics;

#[derive(Default)]
pub(crate) struct Terminal {
    pub(crate) open: bool,
    paused: bool,
    pub(crate) show_during_sync: bool,
    auto_opened: bool,
    was_syncing: bool,
    output: diagnostics::Snapshot,
    previous: Option<usize>,
    #[cfg(test)]
    scroll: Option<(egui::Id, egui::Rect, f32)>,
}

impl Terminal {
    pub(crate) fn observe_sync(&mut self, syncing: bool) {
        if syncing && !self.was_syncing && self.show_during_sync && !self.open {
            self.open = true;
            self.auto_opened = true;
        }
        if !syncing && self.was_syncing && self.auto_opened {
            self.open = false;
            self.auto_opened = false;
        }
        self.was_syncing = syncing;
    }
    pub(crate) fn show(&mut self, ctx: &egui::Context, enabled: bool) {
        if !self.open {
            return;
        }
        if !self.paused
            && let Some(output) = diagnostics::since(self.output.revision)
        {
            self.output = output;
        }
        let mut open = self.open;
        egui::Window::new(text(ctx, "Terminal"))
            .id(egui::Id::new("application-terminal"))
            .open(&mut open)
            .default_pos([24.0, 80.0])
            .default_size([900.0, 440.0])
            .min_size([360.0, 200.0])
            .collapsible(false)
            .constrain(false)
            .enabled(enabled)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(0))
            .show(ctx, |ui| {
                if !enabled { ui.disable(); }
                buttons::bar(ui, 24.0, |ui| {
                    crate::dropdowns::Dropdown::from_id_salt("log-session")
                        .selected_text(self.previous.and_then(|i| diagnostics::previous_sessions().get(i))
                            .map_or("Current session", |s| s.name.as_str()))
                        .show_ui(ui, |ui| {
                            if ui.add(buttons::Button::selectable(self.previous.is_none(), "Current session").bar()).clicked() {
                                self.previous = None;
                                ui.close();
                            }
                            for (index, session) in diagnostics::previous_sessions().iter().enumerate() {
                                if ui.add(buttons::Button::selectable(self.previous == Some(index), &session.name).bar()).clicked() {
                                    self.previous = Some(index);
                                    ui.close();
                                }
                            }
                        });
                    if ui.add(buttons::Button::new(text(ctx, "Copy all")).bar()).clicked() {
                        ctx.copy_text(self.visible_text().into());
                    }
                    if ui.add_enabled(self.previous.is_none(), buttons::Button::new(text(ctx, "Clear")).bar()).clicked() {
                        self.output = diagnostics::clear();
                    }
                    if ui.add_enabled(self.previous.is_none(), buttons::Button::selectable(self.paused,
                        text(ctx, if self.paused { "Resume output" } else { "Pause output" })).bar()).clicked() {
                        self.paused = !self.paused;
                    }
                    ui.allocate_space(egui::vec2(ui.available_width(), 0.0));
                });
                egui::Frame::new().inner_margin(8).show(ui, |ui| {
                    if let Some(failure) = diagnostics::persistence_status().failure {
                        ui.label(failure);
                    }
                    if self.previous.is_some() {
                        ui.weak("Saved log snapshot from before this launch. Open the log folder for complete files.");
                    }
                    ui.weak(text(ctx, "Select any part of the output and press Ctrl+C to copy. Ctrl+A selects all. Times are UTC."));
                    if self.previous.is_none() && self.output.discarded > 0 {
                        ui.weak(format!("{} older log entries discarded (session limit: 1 MiB / 4,000 entries).", self.output.discarded));
                    }
                    let scroll = egui::ScrollArea::vertical()
                        .id_salt("terminal-scroll")
                        .auto_shrink([false, false])
                        .max_height(ui.available_height().max(80.0))
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                        .stick_to_bottom(!self.paused && self.previous.is_none())
                        .show(ui, |ui| { output(ui, self.visible_text()); });
                    #[cfg(test)]
                    { self.scroll = Some((scroll.id, scroll.inner_rect, scroll.state.offset.y)); }
                    #[cfg(not(test))]
                    let _ = scroll;
                });
            });
        self.open = open;
        if !open {
            self.auto_opened = false;
        }
    }

    fn visible_text(&self) -> &str {
        self.previous
            .and_then(|i| diagnostics::previous_sessions().get(i))
            .map_or(&self.output.text, |s| s.text.as_str())
    }
}

fn output(ui: &mut egui::Ui, text: &str) -> egui::text_edit::TextEditOutput {
    // An immutable TextBuffer keeps native text selection/copy shortcuts enabled
    // while rejecting typing, cut, paste and other edits to the log history.
    let mut read_only = text;
    egui::TextEdit::multiline(&mut read_only)
        .id(egui::Id::new("terminal-output"))
        .font(egui::TextStyle::Monospace)
        .desired_width(f32::INFINITY)
        .desired_rows(20)
        .frame(egui::Frame::NONE)
        .show(ui)
}

impl Desktop {
    pub fn open_terminal(&mut self) {
        self.terminal.open = true;
        self.terminal.auto_opened = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_terminal_visibility_respects_manual_ownership() {
        let mut desktop = Desktop::default();
        desktop.terminal.observe_sync(true);
        assert!(!desktop.terminal.open);
        desktop.terminal.observe_sync(false);
        desktop.terminal.show_during_sync = true;
        desktop.terminal.observe_sync(true);
        assert!(desktop.terminal.open);
        desktop.terminal.observe_sync(false);
        assert!(!desktop.terminal.open);
        desktop.terminal.observe_sync(true);
        desktop.open_terminal();
        desktop.terminal.observe_sync(false);
        assert!(
            desktop.terminal.open,
            "Manual opening during work takes ownership"
        );
        desktop.terminal.observe_sync(true);
        desktop.terminal.observe_sync(false);
        assert!(
            desktop.terminal.open,
            "A previously open Terminal stays open"
        );
    }

    #[test]
    fn terminal_can_extend_past_application_bounds() {
        let ctx = egui::Context::default();
        let mut terminal = Terminal {
            open: true,
            ..Default::default()
        };
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 400.0),
                    )),
                    ..Default::default()
                },
                |_ui| terminal.show(&ctx, true),
            );
            output.textures_delta.clear();
        }
        let rect = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("application-terminal")))
            .unwrap();
        assert!(
            rect.right() > 600.0,
            "Terminal must remain partially offscreen: {rect:?}"
        );
    }

    #[test]
    fn long_terminal_output_scrolls_up_and_keeps_the_reading_position() {
        let ctx = egui::Context::default();
        let mut terminal = Terminal {
            open: true,
            paused: true,
            ..Default::default()
        };
        terminal.output.text = (0..200).map(|i| format!("Log entry {i}\n")).collect();
        let draw = |terminal: &mut Terminal, events| {
            let mut result = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_ui| terminal.show(&ctx, true),
            );
            result.textures_delta.clear();
        };
        for _ in 0..4 {
            draw(&mut terminal, vec![]);
        }
        let (id, rect, _) = terminal.scroll.unwrap();
        let mut saved = egui::scroll_area::State::load(&ctx, id).unwrap();
        saved.offset.y = 600.0;
        saved.store(&ctx, id);
        draw(
            &mut terminal,
            vec![egui::Event::PointerMoved(rect.center())],
        );
        let before = terminal.scroll.unwrap().2;
        for _ in 0..5 {
            draw(
                &mut terminal,
                vec![
                    egui::Event::PointerMoved(rect.center()),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 80.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(
            terminal.scroll.unwrap().2 < before - 10.0,
            "Wheel input must scroll older output into view"
        );
        terminal.paused = false;
        terminal.output.revision = diagnostics::since(0).map_or(0, |s| s.revision);
        let previous = terminal.scroll.unwrap().2;
        terminal.output.text.push_str("New output\n");
        draw(&mut terminal, vec![]);
        assert!(
            terminal.scroll.unwrap().2 <= previous + 1.0,
            "New output must not pull the reader to the bottom"
        );
    }

    #[test]
    fn terminal_supports_partial_selection_copy_and_rejects_edits() {
        let ctx = egui::Context::default();
        let text = "first stage\nsecond stage\nvalidation failed";
        let id = egui::Id::new("terminal-output");
        let mut initial = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut edit = output(ui, text);
            edit.response.request_focus();
            edit.state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(12),
                    egui::text::CCursor::new(24),
                )));
            edit.state.store(ui.ctx(), id);
        });
        initial.textures_delta.clear();
        let mut result = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Copy],
                ..Default::default()
            },
            |ui| {
                output(ui, text);
            },
        );
        result.textures_delta.clear();
        assert!(result.platform_output.commands.iter().any(|command|
            matches!(command, egui::OutputCommand::CopyText(value) if value == "second stage")));
        let mut edited = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::Text("overwrite".into()),
                    egui::Event::Paste("paste".into()),
                ],
                ..Default::default()
            },
            |ui| {
                let edit = output(ui, text);
                assert_eq!(edit.galley.text(), text);
            },
        );
        edited.textures_delta.clear();
        assert_eq!(text, "first stage\nsecond stage\nvalidation failed");
    }
}
