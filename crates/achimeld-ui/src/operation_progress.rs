//! Presentation-only operation timing, independent of changing stage messages.
use crate::Desktop;
use crate::steam::{ReviewAction, ReviewChoice, progress_prompt};
use crate::style_picker::Glyph;
use achimeld_application::{EditorPort, UiRequest};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct OperationClock {
    active: Option<(String, Instant)>,
}

impl OperationClock {
    fn observe(&mut self, activity: Option<&str>, now: Instant) -> Duration {
        let Some(activity) = activity else {
            self.active = None;
            return Duration::ZERO;
        };
        let (previous, started) = self.active.get_or_insert_with(|| (activity.into(), now));
        if previous != activity {
            *previous = activity.into();
            *started = now;
        }
        now.saturating_duration_since(*started)
    }
}

fn elapsed_text(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

impl Desktop {
    pub(crate) fn steam_operation_active(port: &dyn EditorPort) -> bool {
        port.browser_profile_busy()
            || port.reading()
            || (port.synchronizing() && port.synchronization_review().is_none())
    }

    pub(crate) fn operation_progress(&mut self, ctx: &egui::Context, port: &dyn EditorPort) {
        let (activity, cancel) = if port.browser_profile_busy() {
            (Some("Deleting browser profile"), None)
        } else if port.synchronizing() && port.synchronization_review().is_none() {
            (
                Some(port.synchronization_activity()),
                Some(UiRequest::CancelSynchronization),
            )
        } else if port.reading() {
            (Some("Reading Steam"), Some(UiRequest::CancelRead))
        } else {
            (None, None)
        };
        let elapsed = self.operation_clock.observe(activity, Instant::now());
        let Some(activity) = activity else { return };
        ctx.request_repaint_after(Duration::from_millis(100));
        let actions = if cancel.is_some() {
            vec![ReviewAction {
                label: "Cancel",
                icon: Glyph::Cancel.image(ctx, 20.0),
                colors: None,
                enabled: true,
            }]
        } else {
            Vec::new()
        };
        // Consume Escape once so it cannot also reach another floating control.
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let choice = progress_prompt(
            ctx,
            &crate::localization::text(ctx, activity),
            Glyph::Validate.image(ctx, 24.0),
            &actions,
            |ui| {
                let body_size = egui::TextStyle::Body.resolve(ui.style()).size;
                ui.vertical_centered(|ui| {
                    ui.add(egui::Spinner::new().size(32.0));
                    ui.add_space(16.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(port.status()).size((body_size * 1.35).max(18.0)),
                        )
                        .wrap()
                        .halign(egui::Align::Center),
                    );
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(crate::localization::text(ctx, "Elapsed time"))
                            .size(body_size.max(16.0)),
                    );
                    ui.label(
                        egui::RichText::new(elapsed_text(elapsed))
                            .monospace()
                            .size((body_size * 2.0).max(28.0)),
                    );
                    ui.add_space(8.0);
                });
            },
        );
        // Outside clicks never hide active work. Escape retains cancellation semantics.
        if (matches!(choice, ReviewChoice::Action(0)) || escape)
            && let Some(cancel) = cancel
        {
            self.request(cancel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Port {
        editor: achimeld_application::Editor,
        profile: bool,
    }
    impl EditorPort for Port {
        fn editor(&self) -> &achimeld_application::Editor {
            &self.editor
        }
        fn execute(
            &mut self,
            _: &str,
            _: achimeld_application::Command,
            _: Option<&str>,
        ) -> Result<(), String> {
            Ok(())
        }
        fn finish_group(&mut self) {}
        fn undo(&mut self) {}
        fn redo(&mut self) {}
        fn request(&mut self, _: UiRequest) {}
        fn status(&self) -> &str {
            "Opening Steamworks destination · App ID 4670140"
        }
        fn preferences(&self) -> &achimeld_application::Preferences {
            self.editor.preferences()
        }
        fn preview(&mut self, _: &str) -> Option<achimeld_application::Preview> {
            None
        }
        fn browser_profile_busy(&self) -> bool {
            self.profile
        }
        fn synchronizing(&self) -> bool {
            !self.profile
        }
        fn synchronization_activity(&self) -> &str {
            "Validating Steam connection"
        }
    }

    #[test]
    fn floating_progress_leaves_background_clickable_and_can_be_dragged() {
        let port = Port {
            editor: Default::default(),
            profile: false,
        };
        let mut desktop = Desktop::default();
        let ctx = egui::Context::default();
        let mut clicked = false;
        let mut button = egui::Rect::NOTHING;
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = ui.button("Edit artwork");
                    button = response.rect;
                    clicked |= response.clicked();
                    desktop.operation_progress(&ctx, &port);
                },
            );
            output.textures_delta.clear();
            button
        };
        frame(vec![]);
        let button = frame(vec![]).center();
        let pointer = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(vec![
            egui::Event::PointerMoved(button),
            pointer(button, true),
        ]);
        frame(vec![pointer(button, false)]);
        let id = egui::Id::new("steam-operation-progress");
        let before = ctx.memory(|m| m.area_rect(id).unwrap());
        let header = before.min + egui::vec2(100.0, 20.0);
        frame(vec![
            egui::Event::PointerMoved(header),
            pointer(header, true),
        ]);
        let moved = header + egui::vec2(90.0, 45.0);
        frame(vec![egui::Event::PointerMoved(moved)]);
        frame(vec![pointer(moved, false)]);
        let after = ctx.memory(|m| m.area_rect(id).unwrap());
        assert!(
            after.min.distance(before.min) > 50.0,
            "{before:?} -> {after:?}"
        );
        assert!(clicked);
        assert!(Desktop::steam_operation_active(&port));
    }

    #[test]
    fn operation_progress_escape_cancels_only_cancellable_work() {
        for profile in [false, true] {
            let port = Port {
                editor: Default::default(),
                profile,
            };
            let mut desktop = Desktop::default();
            let ctx = egui::Context::default();
            for frame in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000.0, 800.0),
                        )),
                        events: if frame == 2 {
                            vec![egui::Event::Key {
                                key: egui::Key::Escape,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: Default::default(),
                            }]
                        } else {
                            vec![]
                        },
                        ..Default::default()
                    },
                    |_| desktop.operation_progress(&ctx, &port),
                );
                output.textures_delta.clear();
            }
            assert_eq!(desktop.requests.len(), usize::from(!profile));
            if !profile {
                assert!(matches!(
                    desktop.requests[0],
                    UiRequest::CancelSynchronization
                ));
            }
            assert!(desktop.operation_clock.active.is_some());
        }
    }

    #[test]
    #[ignore = "Native centered progress preview"]
    fn native_operation_progress_preview() {
        use winit::platform::windows::EventLoopBuilderExtWindows as _;
        struct Preview {
            desktop: Desktop,
            port: Port,
            frame: usize,
        }
        impl eframe::App for Preview {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                self.frame += 1;
                crate::apply_theme(ui.ctx(), self.port.preferences());
                self.desktop.show(ui, &mut self.port);
                if self.frame == 6 {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                            egui::UserData::default(),
                        ));
                }
                for event in ui.input(|i| i.raw.events.clone()) {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let bytes: Vec<_> =
                            image.pixels.iter().flat_map(|p| p.to_array()).collect();
                        image::save_buffer(
                            crate::test_output::root().join("operation-progress.png"),
                            &bytes,
                            image.width() as u32,
                            image.height() as u32,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                assert!(self.frame < 300, "Progress screenshot did not arrive");
                ui.ctx().request_repaint();
            }
        }
        eframe::run_native(
            "Achimeld progress preview",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]),
                event_loop_builder: Some(Box::new(|builder| {
                    builder.with_any_thread(true);
                })),
                ..Default::default()
            },
            Box::new(|cc| {
                crate::configure_fonts(&cc.egui_ctx, None);
                Ok(Box::new(Preview {
                    desktop: Desktop::default(),
                    port: Port {
                        editor: Default::default(),
                        profile: false,
                    },
                    frame: 0,
                }))
            }),
        )
        .unwrap();
    }

    #[test]
    fn operation_clock_retains_elapsed_and_resets_for_review_and_new_work() {
        let now = Instant::now();
        let mut clock = OperationClock::default();
        assert_eq!(clock.observe(Some("Validating"), now), Duration::ZERO);
        assert_eq!(
            clock.observe(Some("Validating"), now + Duration::from_secs(65)),
            Duration::from_secs(65)
        );
        assert_eq!(
            clock.observe(None, now + Duration::from_secs(70)),
            Duration::ZERO
        );
        assert_eq!(
            clock.observe(Some("Synchronizing"), now + Duration::from_secs(90)),
            Duration::ZERO
        );
        assert_eq!(
            clock.observe(Some("Deleting"), now + Duration::from_secs(95)),
            Duration::ZERO
        );
        assert_eq!(elapsed_text(Duration::from_secs(3661)), "01:01:01");
    }
}
