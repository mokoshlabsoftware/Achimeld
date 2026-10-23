//! Presentation-only docking for the shared Achievements/Stats inspector.
use crate::Desktop;
use crate::buttons;
use achimeld_application::{EditorPort, EntityKind};
use egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Placement {
    Left,
    #[default]
    Right,
    Floating,
}

struct Drag {
    origin: Pos2,
    rect: Rect,
    placement: Placement,
    floating: Option<Rect>,
    moved: bool,
}

pub(crate) struct InspectorPanel {
    pub placement: Placement,
    pub rect: Option<Rect>,
    pub header: Option<Rect>,
    width: f32,
    floating: Option<Rect>,
    drag: Option<Drag>,
    press_frame: Option<u64>,
    pending: Option<Placement>,
    target: Option<Placement>,
    reset_size: bool,
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self {
            placement: Placement::Right,
            rect: None,
            header: None,
            width: 288.0,
            floating: None,
            drag: None,
            press_frame: None,
            pending: None,
            target: None,
            reset_size: false,
        }
    }
}

fn header_id() -> egui::Id {
    egui::Id::new("inspector-drag-header")
}

fn fit(rect: Rect, bounds: Rect) -> Rect {
    let size = rect.size().min(bounds.size());
    Rect::from_min_size(rect.min.clamp(bounds.min, bounds.max - size), size)
}

fn dock_target(pointer: Pos2, bounds: Rect) -> Option<Placement> {
    if !bounds.contains(pointer) {
        return None;
    }
    let band = 40.0_f32.min(bounds.width() * 0.2);
    if pointer.x <= bounds.left() + band {
        Some(Placement::Left)
    } else if pointer.x >= bounds.right() - band {
        Some(Placement::Right)
    } else {
        None
    }
}

impl InspectorPanel {
    pub(crate) fn reset_layout(&mut self) {
        *self = Self {
            reset_size: true,
            ..Self::default()
        };
    }

    pub(crate) fn cancel_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            self.placement = drag.placement;
            self.floating = drag.floating;
            self.reset_size = true;
        }
        self.target = None;
    }

    fn update(&mut self, ctx: &egui::Context, bounds: Rect, enabled: bool) {
        self.target = None;
        if !enabled || ctx.input(|i| !i.focused || i.key_pressed(egui::Key::Escape)) {
            self.cancel_drag();
            return;
        }
        if let Some(placement) = self.pending.take() {
            self.placement = placement;
            if placement == Placement::Floating && self.floating.is_none() {
                self.floating = self.rect.map(|rect| {
                    fit(
                        Rect::from_min_size(
                            rect.min + Vec2::splat(24.0),
                            egui::vec2(rect.width(), rect.height().min(600.0)),
                        ),
                        bounds,
                    )
                });
            }
            self.reset_size = true;
        }
        let frame = ctx.cumulative_frame_nr();
        // Capture only the header, respecting clipping, disabled state and covering windows.
        // Use absolute displacement and the release event so repeated layout passes and
        // pointer movement after a batched release cannot move or redock the panel twice.
        if self.drag.is_none()
            && self.press_frame != Some(frame)
            && let Some(response) = ctx.read_response(header_id())
            && response.enabled()
            && let Some(origin) = ctx.input(|i| {
                i.raw.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } => Some(*pos),
                    _ => None,
                })
            })
            && response.interact_rect.contains(origin)
            && ctx.layer_id_at(origin) == Some(response.layer_id)
            && let Some(rect) = self.rect
        {
            self.press_frame = Some(frame);
            self.drag = Some(Drag {
                origin,
                rect,
                placement: self.placement,
                floating: self.floating,
                moved: false,
            });
        }
        let Some(drag) = &mut self.drag else { return };
        let (pointer, down) = ctx.input(|i| {
            let release = i.raw.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => Some(*pos),
                _ => None,
            });
            (
                release.or_else(|| i.pointer.interact_pos()),
                i.pointer.primary_down(),
            )
        });
        let Some(pointer) = pointer else {
            self.cancel_drag();
            return;
        };
        let delta = pointer - drag.origin;
        drag.moved |= delta.length() > ctx.options(|o| o.input_options.max_click_dist);
        if drag.moved {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            let size = if drag.placement == Placement::Floating {
                drag.rect.size()
            } else {
                egui::vec2(drag.rect.width(), drag.rect.height().min(600.0))
            };
            self.floating = Some(fit(
                Rect::from_min_size(drag.rect.min + delta, size),
                bounds,
            ));
            self.placement = Placement::Floating;
            self.reset_size = true;
            self.target = dock_target(pointer, bounds);
            ctx.request_repaint();
        }
        if !down {
            if let Some(target) = self.target.take() {
                self.placement = target;
            }
            self.drag = None;
        }
    }
}

impl Desktop {
    pub(crate) fn inspector_panel(&mut self, root: &mut egui::Ui, port: &mut dyn EditorPort) {
        let bounds = root.available_rect_before_wrap();
        self.inspector_panel
            .update(root.ctx(), bounds, root.is_enabled());
        let fill = root.visuals().panel_fill;
        let max_width = (bounds.width() * 0.6).max(1.0);
        let min_width = 220.0_f32.min(max_width);
        let width = self.inspector_panel.width.clamp(min_width, max_width);
        if self.inspector_panel.placement != Placement::Floating {
            let mut panel = match self.inspector_panel.placement {
                Placement::Left => egui::Panel::left("inspector"),
                _ => egui::Panel::right("inspector"),
            }
            .default_size(width)
            .min_size(min_width)
            .max_size(max_width);
            if self.inspector_panel.reset_size {
                // Replace the dock's remembered width on arrival so its actual
                // footprint agrees with the preview and the resized floating panel.
                panel = panel.exact_size(width);
                self.inspector_panel.reset_size = false;
            }
            let response = panel
                .resizable(true)
                .frame(egui::Frame::new().fill(fill))
                .show(root, |ui| {
                    ui.set_min_width(ui.available_width());
                    self.inspector_panel_contents(ui, port);
                });
            self.inspector_panel.rect = Some(response.response.rect);
            self.inspector_panel.width = response.response.rect.width();
        } else {
            let target = fit(
                self.inspector_panel.floating.unwrap_or_else(|| {
                    Rect::from_min_size(
                        bounds.center() - egui::vec2(width * 0.5, 240.0),
                        egui::vec2(width, 480.0),
                    )
                }),
                bounds,
            );
            let mut window = egui::Window::new("Properties")
                .id(egui::Id::new("floating-inspector"))
                .title_bar(false)
                .movable(false)
                .collapsible(false)
                .enabled(root.is_enabled())
                .constrain_to(bounds)
                .current_pos(target.min)
                .default_size(target.size())
                .min_size(egui::vec2(220.0, 180.0).min(bounds.size()))
                .max_size(bounds.size())
                .frame(
                    egui::Frame::new()
                        .fill(fill)
                        .shadow(root.visuals().window_shadow),
                );
            if self.inspector_panel.reset_size {
                window = window.fixed_size(target.size());
            }
            self.inspector_panel.reset_size = false;
            if let Some(response) = window.show(root.ctx(), |ui| {
                ui.set_min_size(ui.available_size());
                self.inspector_panel_contents(ui, port);
            }) {
                self.inspector_panel.rect = Some(response.response.rect);
                self.inspector_panel.floating = Some(response.response.rect);
                self.inspector_panel.width = response.response.rect.width();
            }
        }
    }

    fn inspector_panel_contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        // A global content identity retains drafts, focus and scroll state across docking.
        ui.scope_builder(
            egui::UiBuilder::new().id(egui::Id::new("inspector-content")),
            |ui| {
                let spacing = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;
                buttons::bar(ui, 24.0, |ui| {
                    let response = ui
                        .add(
                            buttons::Button::new(crate::localization::text(ui.ctx(), "Properties"))
                                .bar()
                                .id(header_id())
                                .sense(egui::Sense::click_and_drag())
                                .min_size(egui::vec2(
                                    ui.available_width(),
                                    ui.spacing().interact_size.y,
                                )),
                        )
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text(
                            "Drag to float or dock on either side. Click for placement options.",
                        );
                    self.inspector_panel.header = Some(response.rect);
                    buttons::menu_popup(ui, &response, |ui| {
                        for (placement, label) in [
                            (Placement::Left, "Dock left"),
                            (Placement::Right, "Dock right"),
                            (Placement::Floating, "Float"),
                        ] {
                            if ui
                                .add_enabled(
                                    self.inspector_panel.placement != placement,
                                    buttons::Button::new(crate::localization::text(
                                        ui.ctx(),
                                        label,
                                    )),
                                )
                                .clicked()
                            {
                                self.inspector_panel.pending = Some(placement);
                                ui.ctx().request_repaint();
                                ui.close();
                            }
                        }
                    });
                });
                ui.add(egui::Separator::default().spacing(0.0));
                ui.spacing_mut().item_spacing.y = spacing;
                self.inspector(ui, port);
                if self.kind == EntityKind::Achievement
                    && let Some(id) = self.selection.first().cloned()
                {
                    self.image_drop_zone(
                        ui,
                        ui.max_rect(),
                        crate::image_drop::DropZone::Achievement(id),
                        1,
                    );
                }
            },
        );
    }

    pub(crate) fn inspector_dock_preview(&self, ctx: &egui::Context, bounds: Rect) {
        let Some(target) = self.inspector_panel.target else {
            return;
        };
        let width = self.inspector_panel.width.min(bounds.width() * 0.6);
        let x = if target == Placement::Left {
            bounds.left()
        } else {
            bounds.right() - width
        };
        let rect = Rect::from_min_size(
            egui::pos2(x, bounds.top()),
            egui::vec2(width, bounds.height()),
        );
        let accent = ctx.global_style().visuals.selection.bg_fill;
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("inspector-dock-preview"),
        ));
        painter.rect_filled(rect, 0.0, accent.gamma_multiply(0.22));
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, accent),
            egui::StrokeKind::Inside,
        );
    }
}
