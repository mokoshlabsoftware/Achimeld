use crate::buttons::UiButtonExt as _;
use crate::desktop::NodeDrag;
use crate::{Desktop, theme};
use achimeld_application::canvas::{GRID_SIZE, MAX_ZOOM, MIN_ZOOM, auto_position, card_size};
use achimeld_application::{Command, EditorPort, EntityKind, Preferences};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::Value;

// Painting and hit-testing need these small fields, never the artwork recipe.
// Own the strings so commands may mutate the document while cards are drawn.
struct CanvasCard {
    id: String,
    api: String,
    name: String,
    description: String,
    stat_type: String,
    column: i32,
    row: i32,
    focus_art: f32,
    stat_driven: bool,
    status: Option<AchievementStatus>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AchievementStatus {
    Retired,
    Current,
    Outdated,
    Unsynced,
}

fn achievement_status(
    entity: &Value,
    project: &Value,
    verified: bool,
) -> Option<AchievementStatus> {
    if achimeld_application::style_achievements::is_local_icon(entity) {
        return None;
    }
    if entity["steamSyncStatus"] == "Retired" {
        return Some(AchievementStatus::Retired);
    }
    let baseline = project["syncBaselines"].as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            entry["entityType"] == "Achievement"
                && entry["localId"] == entity["id"]
                && entry["apiName"] == entity["apiName"]
        })
    });
    Some(if entity["steamSyncStatus"] == "Unsynced" || !baseline {
        AchievementStatus::Unsynced
    } else if verified {
        AchievementStatus::Current
    } else {
        AchievementStatus::Outdated
    })
}

impl AchievementStatus {
    fn asset(self) -> (&'static str, &'static [u8], &'static str) {
        match self {
            Self::Retired => (
                "retired",
                include_bytes!("../assets/achievement-status/retired.rgba"),
                "Retired",
            ),
            Self::Current => (
                "current",
                include_bytes!("../assets/achievement-status/current.rgba"),
                "Synced and up to date",
            ),
            Self::Outdated => (
                "outdated",
                include_bytes!("../assets/achievement-status/outdated.rgba"),
                "Previously synced · changes or validation needed",
            ),
            Self::Unsynced => (
                "unsynced",
                include_bytes!("../assets/achievement-status/unsynced.rgba"),
                "Not synced yet",
            ),
        }
    }

    fn paint(
        self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        alpha: f32,
        card_id: &str,
    ) {
        if alpha <= 0.0 {
            return;
        }
        let (name, bytes, label) = self.asset();
        let key = Id::new(("achievement-status-texture", name));
        let texture = ui
            .ctx()
            .data(|data| data.get_temp::<egui::TextureHandle>(key))
            .unwrap_or_else(|| {
                let texture = ui.ctx().load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                ui.ctx()
                    .data_mut(|data| data.insert_temp(key, texture.clone()));
                texture
            });
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE.gamma_multiply(alpha),
        );
        ui.interact(
            rect,
            Id::new(("achievement-status", card_id)),
            Sense::hover(),
        )
        .on_hover_text(crate::localization::text(ui.ctx(), label));
    }
}

pub(crate) struct CanvasFocus {
    painter: egui::Painter,
    icons: Vec<(Rect, f32)>,
    opacity: f32,
    color: Color32,
}

#[cfg(test)]
mod achievement_status_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_requires_matching_identity_and_current_verification() {
        let mut entity = json!({"id":"local", "apiName":"ACH", "steamSyncStatus":"Active"});
        let mut project = json!({"syncBaselines":[{"entityType":"Achievement", "localId":"local", "apiName":"ACH"}]});
        assert_eq!(
            achievement_status(&entity, &project, true),
            Some(AchievementStatus::Current)
        );
        assert_eq!(
            achievement_status(&entity, &project, false),
            Some(AchievementStatus::Outdated)
        );
        project["syncBaselines"][0]["apiName"] = json!("OLD");
        assert_eq!(
            achievement_status(&entity, &project, true),
            Some(AchievementStatus::Unsynced)
        );
        entity["steamSyncStatus"] = json!("Retired");
        assert_eq!(
            achievement_status(&entity, &project, true),
            Some(AchievementStatus::Retired)
        );
    }
}

const ICON_PAN_SECONDS: f64 = 0.4;
// A small amount of follow-through, settling without overshooting the pointer.
const CAMERA_DRAG_RESPONSE_SECONDS: f32 = 0.045;

pub(crate) struct IconPan {
    achievement: String,
    from: Vec2,
    started: f64,
    repeat_primary: bool,
    primary_press_frame: Option<u64>,
}

pub(crate) struct CameraDrag {
    button: egui::PointerButton,
    pointer_start: Pos2,
    from: Vec2,
    moved: bool,
    started: f64,
}

pub(crate) struct CameraMotion {
    target: Vec2,
    updated: f64,
}

#[derive(Clone)]
pub(crate) struct CanvasMenu {
    kind: EntityKind,
    pointer: Pos2,
    position: [i32; 2],
    entity: Option<String>,
}

pub(crate) fn canvas_position(pointer: Pos2, origin: Pos2, zoom: f32) -> [i32; 2] {
    let point = (pointer - origin) / (zoom * GRID_SIZE);
    [point.x.round() as i32, point.y.round() as i32]
}

pub(crate) struct SelectionDrag {
    pointer_start: Pos2,
    before: Vec<String>,
    base: Vec<String>,
}

/// The largest axis-aligned canvas rectangle outside the floating Layers window.
pub(crate) fn uncovered_canvas(canvas: Rect, panel: Option<Rect>) -> Option<Rect> {
    let Some(covered) = panel
        .map(|panel| canvas.intersect(panel))
        .filter(Rect::is_positive)
    else {
        return canvas.is_positive().then_some(canvas);
    };
    [
        Rect::from_min_max(canvas.min, Pos2::new(covered.left(), canvas.bottom())),
        Rect::from_min_max(Pos2::new(covered.right(), canvas.top()), canvas.max),
        Rect::from_min_max(canvas.min, Pos2::new(canvas.right(), covered.top())),
        Rect::from_min_max(Pos2::new(canvas.left(), covered.bottom()), canvas.max),
    ]
    .into_iter()
    .filter(Rect::is_positive)
    .reduce(|best, rect| {
        if rect.area() > best.area() {
            rect
        } else {
            best
        }
    })
}

fn achievement_art_rect(min: Pos2, zoom: f32, labels: f32) -> Rect {
    Rect::from_min_size(
        min + Vec2::splat(10.0) * zoom * labels,
        Vec2::splat((118.0 - 22.0 * labels) * zoom),
    )
}

pub(crate) fn connector_segment(icon: Rect, panel: Rect, header_icon: Rect) -> Option<[Pos2; 2]> {
    if icon.intersects(panel) {
        return None;
    }
    fn edge(rect: Rect, toward: Pos2) -> Pos2 {
        let direction = toward - rect.center();
        let t =
            (rect.width() / 2.0 / direction.x.abs()).min(rect.height() / 2.0 / direction.y.abs());
        rect.center() + direction * t
    }
    Some([
        edge(icon, header_icon.center()),
        edge(header_icon, icon.center()),
    ])
}

fn stat_wallpaper(painter: &egui::Painter, card: Rect, zoom: f32, p: &Preferences) {
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(card));
    let cue = theme::mix(theme::color(p, "Warning"), theme::color(p, "Error"), 0.25);
    let mut glow = egui::Mesh::default();
    let center = card.left_bottom();
    glow.colored_vertex(center, cue.gamma_multiply(0.15));
    for i in 0..=16 {
        let angle = i as f32 / 16.0 * std::f32::consts::TAU;
        glow.colored_vertex(
            center + Vec2::angled(angle) * 140.0 * zoom,
            Color32::TRANSPARENT,
        );
        if i > 0 {
            glow.add_triangle(0, i, i + 1);
        }
    }
    painter.add(egui::Shape::mesh(glow));
    let color = cue.gamma_multiply(0.10);
    let step = 56.0 * zoom;
    let mut y = card.top();
    while y < card.bottom() {
        let mut x = card.left();
        while x < card.right() {
            for offset in [Vec2::ZERO, Vec2::splat(28.0)] {
                let origin = Pos2::new(x, y) + offset * zoom;
                let point = |x, y| origin + Vec2::new(x, y) * zoom;
                painter.add(egui::Shape::convex_polygon(
                    vec![point(14.0, 3.1), point(22.0, 11.1), point(6.0, 11.1)],
                    color,
                    Stroke::NONE,
                ));
                painter.rect_filled(
                    Rect::from_min_max(point(10.8, 11.0), point(17.2, 23.9)),
                    0.0,
                    color,
                );
            }
            x += step;
        }
        y += step;
    }
}
pub(crate) enum CanvasAction {
    ZoomOut,
    Reset,
    ZoomIn,
    FitAll,
}

impl Desktop {
    pub(crate) fn paint_layer_connections(&self) {
        let (Some(focus), Some(panel), Some(header)) = (
            &self.canvas_focus,
            self.layer_palette.panel_rect,
            &self.layer_palette.header,
        ) else {
            return;
        };
        if !self.layers_available() {
            return;
        }
        let mut connections = Vec::new();
        for (icon, opacity) in &focus.icons {
            let color = focus.color.gamma_multiply(focus.opacity * opacity);
            if let Some(segment) = connector_segment(*icon, panel, header.icon) {
                let line = egui::Shape::line_segment(
                    segment,
                    Stroke::new(0.6, color.gamma_multiply(0.45)),
                );
                focus.painter.add(line.clone());
                connections.push(line);
            }
            focus.painter.rect_stroke(
                *icon,
                0.0,
                Stroke::new(1.5, color),
                egui::StrokeKind::Outside,
            );
        }
        header
            .painter
            .set(header.connections, egui::Shape::Vec(connections));
    }
    pub(crate) fn sync_canvas_camera(&mut self, port: &mut dyn EditorPort) -> &'static str {
        let camera_key = if self.kind == EntityKind::Achievement {
            "achievementsCamera"
        } else {
            "statsCamera"
        };
        let stored_camera = port.editor().document().project()[camera_key].clone();
        if self
            .camera_state
            .as_ref()
            .is_none_or(|(view, kind, camera)| {
                *view != self.view || *kind != self.kind || camera != &stored_camera
            })
        {
            port.finish_group();
            self.camera_editing = false;
            self.camera_drag = None;
            self.camera_motion = None;
            self.canvas_menu = None;
            self.icon_pan = None;
            self.node_drag = None;
            self.selection_drag = None;
            let (zoom, pan) = if stored_camera.is_object() {
                let finite = |key: &str, default: f32| {
                    stored_camera[key]
                        .as_f64()
                        .map(|v| v as f32)
                        .filter(|v| v.is_finite())
                        .unwrap_or(default)
                };
                (
                    finite("zoom", 1.0).clamp(MIN_ZOOM, MAX_ZOOM),
                    -Vec2::new(finite("offsetX", 0.0), finite("offsetY", 0.0)),
                )
            } else {
                (1.0, Vec2::new(30.0, 30.0))
            };
            self.zoom = zoom;
            self.pan = pan;
            self.camera_state = Some((self.view, self.kind, stored_camera));
        }
        camera_key
    }

    pub(crate) fn canvas_controls(&self, ui: &mut egui::Ui) -> Option<CanvasAction> {
        let mut action = None;
        ui.push_id("canvas-controls", |ui| {
            if ui
                .add(crate::buttons::Button::new("−").centered_glyph())
                .clicked()
            {
                action = Some(CanvasAction::ZoomOut);
            }
            if ui
                .app_button(format!("{:.0}%", self.zoom * 100.0))
                .clicked()
            {
                action = Some(CanvasAction::Reset);
            }
            if ui
                .add(crate::buttons::Button::new("+").centered_glyph())
                .clicked()
            {
                action = Some(CanvasAction::ZoomIn);
            }
            if ui.app_button("Fit all").clicked() {
                action = Some(CanvasAction::FitAll);
            }
        });
        action
    }

    fn zoom_canvas(&mut self, anchor: Vec2, zoom: f32) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if zoom != self.zoom {
            // Anchor and pan are in canvas-relative screen units. Preserve the
            // world point under the pointer (wheel) or camera center (buttons).
            self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
            self.zoom = zoom;
        }
    }

    pub(crate) fn start_icon_pan(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        achievement: String,
    ) {
        if self.icon_pan.is_some() {
            return;
        }
        port.finish_group();
        self.camera_editing = false;
        self.camera_drag = None;
        self.camera_motion = None;
        self.icon_pan = Some(IconPan {
            achievement,
            from: self.pan,
            started: ctx.input(|input| input.time),
            repeat_primary: false,
            primary_press_frame: None,
        });
        ctx.request_repaint();
    }

    fn animate_icon_pan(
        &mut self,
        ui: &egui::Ui,
        port: &mut dyn EditorPort,
        rect: Rect,
        labels: f32,
    ) {
        if !self.layers_available() || !ui.is_enabled() {
            self.icon_pan = None;
            return;
        }
        let now = ui.input(|input| input.time);
        let Some(animation) = &self.icon_pan else {
            return;
        };
        let entities = port.editor().document().entities(EntityKind::Achievement);
        let target = entities.iter().enumerate().find(|(_, entity)| {
            entity["id"].as_str() == Some(animation.achievement.as_str())
                && self.selection.first() == Some(&animation.achievement)
        });
        let (Some((index, entity)), Some(visible)) = (
            target,
            uncovered_canvas(rect, self.layer_palette.panel_rect),
        ) else {
            self.icon_pan = None;
            return;
        };
        let automatic = auto_position(EntityKind::Achievement, index);
        let position = Vec2::new(
            entity["canvasColumn"]
                .as_i64()
                .unwrap_or(i64::from(automatic[0])) as f32,
            entity["canvasRow"]
                .as_i64()
                .unwrap_or(i64::from(automatic[1])) as f32,
        ) * GRID_SIZE;
        // Use exactly the artwork geometry used for painting, including the
        // compact icon at low zoom. Pan is stored in canvas-relative screen units.
        let center = achievement_art_rect(position.to_pos2(), 1.0, labels).center();
        let destination = visible.center() - rect.min - center.to_vec2() * self.zoom;
        let progress = ((now - animation.started) / ICON_PAN_SECONDS).clamp(0.0, 1.0) as f32;
        // Smootherstep keeps velocity and acceleration at zero at both ends,
        // while elapsed time still fixes the complete pan to ICON_PAN_SECONDS.
        let eased = progress * progress * progress * (progress * (6.0 * progress - 15.0) + 10.0);
        self.pan = animation.from + (destination - animation.from) * eased;
        if progress >= 1.0 {
            self.pan = destination;
            self.icon_pan = None;
        } else {
            ui.ctx().request_repaint();
        }
    }

    fn drag_canvas_camera(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        cancel: bool,
    ) -> (bool, Option<Pos2>) {
        if cancel || !ui.is_enabled() {
            self.camera_drag = None;
            self.camera_motion = None;
            return (false, None);
        }
        let now = ui.input(|input| input.time);
        let frame = ui.ctx().cumulative_frame_nr();
        if self.camera_drag.is_none()
            && self.camera_press_frame != Some(frame)
            && ui.input(|input| input.pointer.any_pressed())
        {
            self.camera_motion = None;
        }
        // Cards share the canvas layer. Capture the gesture before their hit
        // targets can consume it, while respecting clipping and floating windows.
        if self.camera_drag.is_none()
            && self.camera_press_frame != Some(frame)
            && self.node_drag.is_none()
            && self.selection_drag.is_none()
            && let Some((button, pointer_start)) = ui.input(|input| {
                if input.pointer.primary_down() {
                    return None;
                }
                [egui::PointerButton::Middle, egui::PointerButton::Secondary]
                    .into_iter()
                    .find_map(|button| {
                        // press_origin is already cleared if a busy frame batches
                        // the press and release. Retain the original event position.
                        input.raw.events.iter().find_map(|event| match event {
                            egui::Event::PointerButton {
                                pos,
                                button: pressed_button,
                                pressed: true,
                                ..
                            } if *pressed_button == button => Some((button, *pos)),
                            _ => None,
                        })
                    })
            })
            && rect.intersect(ui.clip_rect()).contains(pointer_start)
            && ui.ctx().layer_id_at(pointer_start) == Some(ui.layer_id())
        {
            self.camera_press_frame = Some(frame);
            self.camera_drag = Some(CameraDrag {
                button,
                pointer_start,
                from: self.pan,
                moved: false,
                started: now,
            });
            self.camera_motion = Some(CameraMotion {
                target: self.pan,
                updated: now,
            });
        }
        let Some(drag) = &mut self.camera_drag else {
            let moving = self.camera_motion.is_some();
            self.animate_camera_drag(ui, now);
            return (moving, None);
        };
        let options = ui.ctx().options(|options| options.input_options);
        let (pointer, dragging, down) = ui.input(|input| {
            // A busy frame can also contain hover movement after release. End
            // the gesture at its release event, before that later movement.
            let released_at = input.raw.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    ..
                } if *button == drag.button => Some(*pos),
                _ => None,
            });
            (
                released_at.or_else(|| input.pointer.interact_pos()),
                input.pointer.is_decidedly_dragging(),
                input.pointer.button_down(drag.button),
            )
        });
        if (dragging
            || drag.moved
            || now - drag.started > options.max_click_duration
            || pointer.is_some_and(|pointer| {
                pointer.distance(drag.pointer_start) > options.max_click_dist
            }))
            && let Some(pointer) = pointer
        {
            // Absolute displacement is stable across repeated layout passes,
            // stationary frames, and the last movement on button release.
            if let Some(motion) = &mut self.camera_motion {
                motion.target = drag.from + (pointer - drag.pointer_start);
            }
            drag.moved = true;
        }
        let menu = (!down
            && !drag.moved
            && drag.button == egui::PointerButton::Secondary
            && pointer.is_some_and(|pointer| {
                rect.intersect(ui.clip_rect()).contains(pointer)
                    && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id())
            })
            && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Secondary)))
        .then_some(drag.pointer_start);
        if !down {
            self.camera_drag = None;
        }
        self.animate_camera_drag(ui, now);
        (true, menu)
    }

    fn animate_camera_drag(&mut self, ui: &egui::Ui, now: f64) {
        let Some(motion) = &mut self.camera_motion else {
            return;
        };
        // Elapsed time prevents repeated layout passes from advancing twice.
        // A stationary hold can leave the UI idle; keep its first move smooth.
        let elapsed = (now - motion.updated).clamp(0.0, 0.05) as f32;
        motion.updated = now;
        let progress = 1.0 - (-elapsed / CAMERA_DRAG_RESPONSE_SECONDS).exp();
        self.pan += (motion.target - self.pan) * progress;
        if (self.pan - motion.target).length() <= 0.1 {
            self.pan = motion.target;
            if self.camera_drag.is_none() {
                self.camera_motion = None;
            }
        } else {
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        p: &Preferences,
        action: Option<CanvasAction>,
    ) {
        let camera_key = self.sync_canvas_camera(port);
        let before_camera = (self.zoom, self.pan);
        let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        if escape || !ui.is_enabled() || !ui.input(|input| input.focused) {
            self.node_drag = None;
            if let Some(drag) = self.selection_drag.take() {
                self.selection = drag.before;
            }
        }
        // Track the full gesture before painting, including a stationary frame
        // or the final movement delivered together with the button release.
        if let Some(drag) = &mut self.node_drag
            && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
        {
            drag.delta = pointer - drag.pointer_start;
        }
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        if self.kind == EntityKind::Achievement {
            self.image_drop_zone(ui, rect, crate::image_drop::DropZone::Canvas, 0);
        }
        let painter = ui.painter_at(rect);
        let scroll = if ui.is_enabled() && ui.rect_contains_pointer(rect) {
            ui.input(|input| input.smooth_scroll_delta.y)
        } else {
            0.0
        };
        let (panning, menu_click) = self.drag_canvas_camera(
            ui,
            rect,
            escape || action.is_some() || scroll != 0.0 || !ui.input(|input| input.focused),
        );
        if action.is_some() || scroll != 0.0 || panning {
            self.node_drag = None;
            if let Some(drag) = self.selection_drag.take() {
                self.selection = drag.before;
            }
        }
        // Hit-test the last painted icon: another press on the moving target is
        // a repeat activation, not a request to interrupt the camera movement.
        let repeat_icon_press = self
            .icon_pan
            .as_ref()
            .and_then(|animation| {
                ui.ctx()
                    .read_response(Id::new(("icon-layers", &animation.achievement)))
            })
            .is_some_and(|icon| {
                ui.input(|input| {
                    input.pointer.button_pressed(egui::PointerButton::Primary)
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|pointer| icon.rect.contains(pointer))
                })
            });
        if ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
            && let Some(animation) = &mut self.icon_pan
            && animation.primary_press_frame != Some(ui.ctx().cumulative_frame_nr())
        {
            animation.repeat_primary = repeat_icon_press;
            animation.primary_press_frame = Some(ui.ctx().cumulative_frame_nr());
        }
        // Preserve that decision through release even if the icon moves away
        // from the pointer; the same click must not become a background gesture.
        let repeat_icon_gesture = self.icon_pan.as_ref().is_some_and(|pan| pan.repeat_primary);
        let interrupt_pan = escape
            || action.is_some()
            || scroll != 0.0
            || panning
            || (ui.rect_contains_pointer(rect)
                && ui.input(|input| input.pointer.any_pressed())
                && !repeat_icon_gesture);
        if interrupt_pan {
            self.icon_pan = None;
        }
        let mut background = egui::Mesh::default();
        for (pos, color) in [
            (rect.left_top(), theme::color(p, "Background top")),
            (rect.right_top(), theme::color(p, "Background top")),
            (rect.left_bottom(), theme::color(p, "Background bottom")),
            (rect.right_bottom(), theme::color(p, "Background bottom")),
        ] {
            background.colored_vertex(pos, color);
        }
        background.add_triangle(0, 1, 2);
        background.add_triangle(2, 1, 3);
        painter.add(egui::Shape::mesh(background));
        // Nodes share the canvas layer and must not block wheel zoom. Respect
        // clipping and higher layers so covered canvases do not receive it.
        match action {
            Some(CanvasAction::ZoomOut) => self.zoom_canvas(rect.size() * 0.5, self.zoom / 1.2),
            Some(CanvasAction::ZoomIn) => self.zoom_canvas(rect.size() * 0.5, self.zoom * 1.2),
            _ => {}
        }
        if scroll != 0.0
            && let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
        {
            self.zoom_canvas(pointer - rect.min, self.zoom * (scroll * 0.0015).exp());
        }
        if !escape && action.is_none() && scroll == 0.0 && !panning {
            self.navigate_nodes(ui, port, p, rect, response.id);
        }
        let labels = if self.kind == EntityKind::Achievement {
            ui.ctx()
                .animate_bool_with_time(ui.id().with("canvas-labels"), self.zoom > 0.5, 0.4)
        } else {
            1.0
        };
        self.animate_icon_pan(ui, port, rect, labels);
        let cell = Vec2::splat(GRID_SIZE * self.zoom);
        let origin = rect.min + self.pan;
        let grid = theme::color(p, "Grid");
        let step = GRID_SIZE * self.zoom;
        let mut x = rect.left() + (self.pan.x % step + step) % step;
        while x < rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(0.5, grid),
            );
            x += step;
        }
        let mut y = rect.top() + (self.pan.y % step + step) % step;
        while y < rect.bottom() {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(0.5, grid),
            );
            y += step;
        }
        painter.circle_filled(origin, 3.0, theme::color(p, "Accent"));
        let focus_id = ui.id().with("icon-focus-mask");
        let initialized = ui.ctx().data_mut(|data| {
            std::mem::replace(data.get_temp_mut_or_default::<bool>(focus_id), true)
        });
        if !initialized {
            ui.ctx().animate_bool_with_time(focus_id, false, 0.4);
        }
        let focus = ui
            .ctx()
            .animate_bool_with_time(focus_id, self.layers_available(), 0.4);
        // Use the same style membership as commit_style, including general
        // artwork and linked text supplied independently by each achievement.
        let focus_style = self
            .layer_palette
            .active_style
            .as_ref()
            .filter(|_| self.layer_palette.achievement.as_ref() == self.selection.first())
            .and_then(|id| port.icon_styles().into_iter().find(|style| &style.id == id))
            .map(|style| achimeld_application::style_signature(&style.recipe));
        let mut foreground_art = Vec::new();
        let mut focus_icons = Vec::new();
        let cards: Vec<CanvasCard> = {
            let document = port.editor().document();
            let entities = document.entities(self.kind);
            entities
                .iter()
                .enumerate()
                .map(|(index, entity)| {
                    let api = entity["apiName"]
                        .as_str()
                        .or(entity.pointer("/apiName/value").and_then(Value::as_str))
                        .unwrap_or("");
                    let automatic = auto_position(self.kind, index);
                    CanvasCard {
                        id: entity["id"].as_str().unwrap_or(api).to_owned(),
                        api: if achimeld_application::art_images::is_art_image(entity) {
                            crate::localization::text(ui.ctx(), "Art image")
                        } else if achimeld_application::style_achievements::is_local_icon(entity) {
                            crate::localization::text(ui.ctx(), "Icon style node")
                        } else {
                            api.to_owned()
                        },
                        name: document.display_name(entity, &p.language),
                        description: if achimeld_application::art_images::is_art_image(entity) {
                            String::new()
                        } else {
                            document.description(entity, &p.language)
                        },
                        stat_type: entity["type"].as_str().unwrap_or("STAT").to_owned(),
                        column: entity["canvasColumn"]
                            .as_i64()
                            .unwrap_or(i64::from(automatic[0]))
                            as i32,
                        row: entity["canvasRow"]
                            .as_i64()
                            .unwrap_or(i64::from(automatic[1])) as i32,
                        focus_art: if self.selection.first().is_some_and(|selected| {
                            selected == entity["id"].as_str().unwrap_or(api)
                        }) {
                            1.0
                        } else {
                            crate::layer_palette::style_edit_fade(
                                ui.ctx(),
                                ui.id()
                                    .with(("style-focus", entity["id"].as_str().unwrap_or(api))),
                                focus_style.as_ref().is_some_and(|signature| {
                                    !achimeld_application::style_achievements::is_local_icon(entity)
                                        && achimeld_application::style_signature(
                                            &crate::layer_palette::effective_icon(entity),
                                        ) == *signature
                                }),
                            )
                        },
                        stat_driven: !entity["progressStatId"].is_null(),
                        status: (self.kind == EntityKind::Achievement)
                            .then(|| {
                                achievement_status(
                                    entity,
                                    document.project(),
                                    port.synchronization_verified(),
                                )
                            })
                            .flatten(),
                    }
                })
                .collect()
        };
        if cards.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                match self.kind {
                    EntityKind::Achievement => "Add achievement to begin",
                    EntityKind::Stat => "Add stat to begin",
                },
                FontId::proportional(p.header_size),
                theme::color(p, "Muted text"),
            );
        }
        let filter = self.filter.to_lowercase();
        let visible = |entity: &&CanvasCard| {
            filter.is_empty()
                || format!("{} {}", entity.name, entity.api)
                    .to_lowercase()
                    .contains(&filter)
        };
        let size = card_size(self.kind);
        let zoom = self.zoom;
        let card_rect = |entity: &CanvasCard| {
            Rect::from_min_size(
                origin + Vec2::new(entity.column as f32 * cell.x, entity.row as f32 * cell.y),
                Vec2::new(size[0], size[1]) * zoom,
            )
        };
        let can_start_gesture = !escape
            && ui.is_enabled()
            && ui.input(|input| input.focused)
            && action.is_none()
            && scroll == 0.0
            && !panning
            && !repeat_icon_gesture;
        if can_start_gesture
            && self.node_drag.is_none()
            && self.selection_drag.is_none()
            && self.camera_drag.is_none()
            && ui.rect_contains_pointer(rect)
            && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary))
            && let Some(pointer_start) = ui.input(|input| input.pointer.press_origin())
            && !cards
                .iter()
                .filter(visible)
                .any(|card| card_rect(card).contains(pointer_start))
        {
            let before = self.selection.clone();
            let base = if ui.input(|input| input.modifiers.ctrl) || self.layers_available() {
                before.clone()
            } else {
                Vec::new()
            };
            self.selection = base.clone();
            self.selected_layer = None;
            self.selection_drag = Some(SelectionDrag {
                pointer_start,
                before,
                base,
            });
        }
        let selection_rect = self.selection_drag.as_ref().and_then(|drag| {
            ui.input(|input| input.pointer.interact_pos())
                .map(|pointer| Rect::from_two_pos(drag.pointer_start, pointer))
        });
        if let (Some(drag), Some(bounds)) = (&self.selection_drag, selection_rect) {
            self.selection = drag.base.clone();
            if bounds.is_positive() {
                for entity in cards.iter().filter(visible) {
                    if bounds.intersects(card_rect(entity)) && !self.selection.contains(&entity.id)
                    {
                        self.selection.push(entity.id.clone());
                    }
                }
            }
        }
        let mut pointer_on_card = false;
        for entity in &cards {
            let id = entity.id.clone();
            let api = &entity.api;
            let name = &entity.name;
            if !filter.is_empty() && !format!("{name} {api}").to_lowercase().contains(&filter) {
                continue;
            }
            let col = entity.column;
            let row = entity.row;
            let delta = self
                .node_drag
                .as_ref()
                .filter(|d| d.starts.contains_key(&id))
                .map(|d| d.delta)
                .unwrap_or_default();
            let min = origin + Vec2::new(col as f32 * cell.x, row as f32 * cell.y) + delta;
            let size = card_size(self.kind);
            let card = Rect::from_min_size(min, Vec2::new(size[0], size[1]) * self.zoom);
            if !card.intersects(rect) {
                continue;
            }
            let painter = ui.painter_at(card.intersect(rect));
            let selected = self.selection.contains(&id);
            let corner_radius = if self.kind == EntityKind::Achievement {
                0
            } else {
                p.control_radius
            };
            let retired = entity.status == Some(AchievementStatus::Retired);
            painter.rect_filled(
                card,
                corner_radius,
                if retired {
                    Color32::BLACK
                } else {
                    theme::color(p, "Panel")
                },
            );
            if self.kind == EntityKind::Achievement && entity.stat_driven && !retired {
                stat_wallpaper(&painter, card, self.zoom, p);
            }
            if selected || self.kind == EntityKind::Stat {
                painter.rect_stroke(
                    card,
                    corner_radius,
                    Stroke::new(
                        if selected { 2.0 } else { 1.0 },
                        theme::color(p, if selected { "Accent" } else { "Border" }),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            let art = if self.kind == EntityKind::Achievement {
                achievement_art_rect(card.min, self.zoom, labels)
            } else {
                Rect::from_min_size(
                    card.min + Vec2::new(12.0, 12.0) * self.zoom,
                    Vec2::new(212.0, 86.0) * self.zoom,
                )
            };
            if self.kind == EntityKind::Achievement {
                // Keep alpha intact: fading a duplicate image would brighten
                // translucent artwork. Only undo the mask's RGB dimming.
                let brightness = if focus > 0.0 && entity.focus_art > 0.0 {
                    1.0 - (focus * 166.0).round() / 255.0 * (1.0 - entity.focus_art)
                } else {
                    1.0
                };
                let artwork = if let Some(texture) = self.canvas_texture(
                    ui.ctx(),
                    port,
                    &id,
                    art.width() * ui.ctx().pixels_per_point(),
                ) {
                    egui::Shape::image(
                        texture,
                        art,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::from_gray((brightness * 255.0).round() as u8),
                    )
                } else {
                    let color = theme::color(p, "Accent");
                    let [r, g, b, a] = color.to_array();
                    let color = Color32::from_rgba_premultiplied(
                        (f32::from(r) * brightness).round() as u8,
                        (f32::from(g) * brightness).round() as u8,
                        (f32::from(b) * brightness).round() as u8,
                        a,
                    );
                    let galley = painter.layout_no_wrap(
                        "◇".into(),
                        FontId::proportional(64.0 * self.zoom),
                        color,
                    );
                    let pos = Align2::CENTER_CENTER
                        .anchor_size(art.center(), galley.size())
                        .min;
                    egui::Shape::galley(pos, galley, color)
                };
                if focus > 0.0 && entity.focus_art > 0.0 {
                    foreground_art.push((painter.clone(), artwork));
                    focus_icons.push((art, entity.focus_art));
                } else {
                    painter.add(artwork);
                }
            } else {
                painter.text(
                    art.center(),
                    Align2::CENTER_CENTER,
                    &entity.stat_type,
                    FontId::proportional(28.0 * self.zoom),
                    theme::color(p, "Accent"),
                );
            }
            let text_origin = if self.kind == EntityKind::Achievement {
                Vec2::new(116.0, 16.0)
            } else {
                Vec2::new(12.0, 108.0)
            };
            let text_painter = painter.with_clip_rect(
                Rect::from_min_max(
                    card.min + text_origin * self.zoom,
                    card.max
                        - Vec2::new(if entity.status.is_some() { 34.0 } else { 8.0 }, 8.0)
                            * self.zoom,
                )
                .intersect(rect),
            );
            let text_alpha = labels * if retired { 0.55 } else { 1.0 };
            text_painter.text(
                card.min + text_origin * self.zoom,
                Align2::LEFT_TOP,
                name,
                FontId::proportional(p.normal_size * self.zoom),
                (if retired {
                    Color32::WHITE
                } else {
                    theme::color(p, "Primary text")
                })
                .gamma_multiply(text_alpha),
            );
            text_painter.text(
                card.min + (text_origin + Vec2::new(0.0, 24.0)) * self.zoom,
                Align2::LEFT_TOP,
                api,
                FontId::proportional(p.small_size * self.zoom),
                (if retired {
                    Color32::WHITE
                } else {
                    theme::color(p, "Muted text")
                })
                .gamma_multiply(text_alpha),
            );
            if self.kind == EntityKind::Achievement {
                text_painter.text(
                    card.min + (text_origin + Vec2::new(0.0, 48.0)) * self.zoom,
                    Align2::LEFT_TOP,
                    &entity.description,
                    FontId::proportional(p.small_size * self.zoom),
                    (if retired {
                        Color32::WHITE
                    } else {
                        theme::color(p, "Muted text")
                    })
                    .gamma_multiply(text_alpha),
                );
            }
            if let Some(status) = entity.status {
                let badge = Rect::from_min_max(
                    card.max - Vec2::splat(30.0 * self.zoom),
                    card.max - Vec2::splat(6.0 * self.zoom),
                );
                status.paint(ui, &painter, badge, labels, &id);
            }
            let hit = ui.interact(card, Id::new(("node", &id)), Sense::click_and_drag());
            pointer_on_card |= hit.contains_pointer();
            let mut clicked = !repeat_icon_gesture && hit.clicked();
            if self.kind == EntityKind::Achievement {
                self.image_drop_zone(
                    ui,
                    card,
                    crate::image_drop::DropZone::Achievement(id.clone()),
                    1,
                );
                let icon_hit = ui
                    .interact(art, Id::new(("icon-layers", &id)), Sense::click())
                    .on_hover_text("Edit icon layers");
                clicked |= !repeat_icon_gesture && icon_hit.clicked();
                if !repeat_icon_gesture && icon_hit.double_clicked() {
                    if self.layers_available() && self.selection.first() == Some(&id) {
                        self.start_icon_pan(ui.ctx(), port, id.clone());
                    }
                    self.selection = vec![id.clone()];
                    self.open_layers();
                }
            }
            if self.selection_drag.is_none() && clicked {
                if !ui.input(|i| i.modifiers.ctrl) {
                    self.selection.clear();
                }
                if ui.input(|i| i.modifiers.ctrl)
                    && self.selection.contains(&id)
                    && !self.layers_available()
                {
                    self.selection.retain(|selected| selected != &id);
                } else if !self.selection.contains(&id) {
                    self.selection.push(id.clone());
                }
                self.selected_layer = None;
            }
            if !repeat_icon_gesture && hit.double_clicked() {
                self.selection = vec![id.clone()];
                self.close_layers();
            }
            if hit.drag_started_by(egui::PointerButton::Primary)
                && can_start_gesture
                && self.selection_drag.is_none()
                && let Some(pointer_start) = ui.input(|i| i.pointer.press_origin())
            {
                if !self.selection.contains(&id) {
                    self.selection = vec![id.clone()];
                }
                self.node_drag = Some(NodeDrag {
                    starts: cards
                        .iter()
                        .filter(|card| self.selection.contains(&card.id))
                        .map(|card| (card.id.clone(), [card.column, card.row]))
                        .collect(),
                    pointer_start,
                    delta: hit.total_drag_delta().unwrap_or_default(),
                });
                self.icon_pan = None;
                self.selected_layer = None;
                ui.ctx().request_repaint();
            }
        }
        // Let an existing menu handle the outside click before changing its
        // anchor or selection, so its final painted frame cannot jump.
        if let Some(pointer) = menu_click
            && self.canvas_menu.is_none()
        {
            let entity = cards
                .iter()
                .filter(visible)
                .rev()
                .find(|card| card_rect(card).contains(pointer))
                .map(|card| card.id.clone());
            if let Some(id) = &entity
                && !self.selection.contains(id)
            {
                self.selection = vec![id.clone()];
                self.selected_layer = None;
            }
            self.canvas_menu = Some(CanvasMenu {
                kind: self.kind,
                pointer,
                position: canvas_position(pointer, origin, self.zoom),
                entity,
            });
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&response));
        }
        if let Some(menu) = self.canvas_menu.clone() {
            let mut open = menu.kind == self.kind
                && ui.is_enabled()
                && !escape
                && ui.input(|input| input.focused);
            crate::buttons::context_menu_at(&response, menu.pointer, &mut open, |ui| {
                let editable = self.entity_drafts.is_empty() && self.localization_drafts.is_empty();
                let label = if self.kind == EntityKind::Achievement {
                    "Create achievement"
                } else {
                    "Create stat"
                };
                let mut creation = vec![(
                    label,
                    Command::AddAt {
                        kind: self.kind,
                        position: menu.position,
                    },
                )];
                if self.kind == EntityKind::Achievement {
                    creation.push((
                        "Create icon style node",
                        Command::AddStyleAchievementAt {
                            position: menu.position,
                        },
                    ));
                }
                if self.kind == EntityKind::Achievement {
                    creation.push((
                        "Create art image",
                        Command::AddArtImageAt {
                            position: menu.position,
                        },
                    ));
                }
                for (label, command) in creation {
                    if !ui
                        .add_enabled(editable, crate::buttons::Button::new(label))
                        .clicked()
                    {
                        continue;
                    }
                    port.finish_group();
                    if self.apply(port, label, command, None) {
                        self.selection = port
                            .editor()
                            .document()
                            .entities(self.kind)
                            .last()
                            .and_then(|entity| entity["id"].as_str())
                            .map(|id| vec![id.to_owned()])
                            .unwrap_or_default();
                        self.filter.clear();
                        self.selected_layer = None;
                        self.close_layers();
                        if matches!(label, "Create icon style node" | "Create art image") {
                            self.inspector_tab = crate::desktop::InspectorTab::Icon;
                            self.open_layers();
                        }
                    }
                    ui.close();
                }
                if ui
                    .add_enabled(
                        editable && !self.selection.is_empty(),
                        crate::buttons::Button::new("Delete selection"),
                    )
                    .clicked()
                {
                    self.delete(port);
                    ui.close();
                }
                if self.kind == EntityKind::Achievement
                    && let Some(id) = &menu.entity
                    && ui.app_button("Icon layers…").clicked()
                {
                    self.selection = vec![id.clone()];
                    self.open_layers();
                    ui.close();
                }
            });
            if !open {
                self.canvas_menu = None;
            }
        }
        if !repeat_icon_gesture && response.double_clicked() && !pointer_on_card {
            self.close_layers();
            self.selection.clear();
            self.selected_layer = None;
        }
        // A dragged card can leave the clipped canvas. Commit independently of
        // its hit-test so dropping outside still completes exactly one move.
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(drag) = self.node_drag.take()
        {
            let delta = [
                (drag.delta.x / cell.x).round() as i32,
                (drag.delta.y / cell.y).round() as i32,
            ];
            let label = if drag.starts.len() == 1 {
                "Move node"
            } else {
                "Move nodes"
            };
            let commands = drag
                .starts
                .into_iter()
                .map(|(id, start)| Command::Move {
                    kind: self.kind,
                    id,
                    column: start[0].saturating_add(delta[0]),
                    row: start[1].saturating_add(delta[1]),
                })
                .collect();
            if delta != [0, 0] {
                self.apply(port, label, Command::Batch(commands), None);
            }
            ui.ctx().request_repaint();
        }
        if focus > 0.0 {
            painter.rect_filled(
                rect,
                0.0,
                Color32::from_black_alpha((focus * 166.0).round() as u8),
            );
            for (art_painter, artwork) in foreground_art {
                art_painter.add(artwork);
            }
        }
        if let Some(bounds) = selection_rect.filter(Rect::is_positive) {
            let accent = theme::color(p, "Accent");
            painter.rect_filled(bounds, 0.0, accent.gamma_multiply(0.15));
            painter.rect_stroke(
                bounds,
                0.0,
                Stroke::new(1.0, accent),
                egui::StrokeKind::Inside,
            );
        }
        if !ui.input(|input| input.pointer.primary_down()) {
            self.selection_drag = None;
            self.node_drag = None;
        }
        self.canvas_focus = Some(CanvasFocus {
            painter: painter.clone(),
            icons: focus_icons,
            opacity: focus,
            color: theme::color(p, "Primary text"),
        });
        match action {
            Some(CanvasAction::Reset) => {
                self.zoom = 1.0;
                self.pan = Vec2::new(30.0, 30.0);
            }
            Some(CanvasAction::FitAll) => {
                let mut bounds = Rect::NOTHING;
                let size = card_size(self.kind);
                for entity in &cards {
                    bounds = bounds.union(Rect::from_min_size(
                        egui::pos2(
                            entity.column as f32 * GRID_SIZE,
                            entity.row as f32 * GRID_SIZE,
                        ),
                        Vec2::new(size[0], size[1]),
                    ));
                }
                if bounds.is_finite() {
                    let available = (rect.size() - Vec2::splat(60.0)).max(Vec2::splat(1.0));
                    self.zoom = (available.x / bounds.width())
                        .min(available.y / bounds.height())
                        .clamp(MIN_ZOOM, MAX_ZOOM);
                    self.pan = rect.size() * 0.5 - bounds.center().to_vec2() * self.zoom;
                }
            }
            Some(CanvasAction::ZoomOut | CanvasAction::ZoomIn) | None => {}
        }
        if before_camera != (self.zoom, self.pan) {
            // The status bar is drawn first; refresh its zoom readout too.
            ui.ctx().request_repaint();
            if self.apply(
                port,
                "Canvas camera",
                Command::CanvasCamera {
                    kind: self.kind,
                    zoom: f64::from(self.zoom),
                    offset_x: f64::from(-self.pan.x),
                    offset_y: f64::from(-self.pan.y),
                },
                Some("canvas-camera"),
            ) {
                self.camera_state = Some((
                    self.view,
                    self.kind,
                    port.editor().document().project()[camera_key].clone(),
                ));
                self.camera_editing = true;
            } else {
                self.icon_pan = None;
                self.camera_drag = None;
                self.camera_motion = None;
            }
        }
        if self.camera_editing
            && self.icon_pan.is_none()
            && self.camera_motion.is_none()
            && !ui.input(|i| i.pointer.any_down() || i.smooth_scroll_delta.y != 0.0)
        {
            port.finish_group();
            self.camera_editing = false;
        }
    }
}
