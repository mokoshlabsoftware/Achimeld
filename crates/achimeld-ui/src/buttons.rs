//! The application's finite button styles and interruptible hover animation.
use egui::{AtomLayout, Color32, Frame, IntoAtoms, Response, Sense, TextStyle, Ui, Vec2, Widget};

pub const BODY_SECONDS: f32 = 0.15;
pub const TEXT_SECONDS: f32 = 0.2;

#[derive(Clone, Copy, Default)]
pub enum ButtonStyle {
    #[default]
    Standard,
    Accent,
    Quiet,
}

#[derive(Clone, Copy, Default)]
struct Fade {
    body: f32,
    text: f32,
    time: Option<f64>,
}
impl Fade {
    fn advance(&mut self, target: bool, time: f64) {
        let dt = self.time.map_or(0.0, |last| (time - last).max(0.0) as f32);
        self.time = Some(time);
        let direction = if target { 1.0 } else { -1.0 };
        self.body = (self.body + direction * dt / BODY_SECONDS).clamp(0.0, 1.0);
        self.text = (self.text + direction * dt / TEXT_SECONDS).clamp(0.0, 1.0);
    }
    fn settled(self, target: bool) -> bool {
        let end = if target { 1.0 } else { 0.0 };
        self.body == end && self.text == end
    }
}
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn base_colors(ui: &Ui, style: ButtonStyle) -> (Color32, Color32) {
    let v = ui.visuals();
    let body = if crate::dropdowns::is_popup(ui) {
        v.window_fill
    } else {
        match style {
            ButtonStyle::Standard => v.widgets.inactive.bg_fill,
            ButtonStyle::Accent => v.selection.bg_fill,
            ButtonStyle::Quiet => v.panel_fill,
        }
    };
    (body, v.text_color())
}

/// A bar fits its tallest glyph/control, with no additional outer padding.
pub fn bar_height(ui: &Ui, icon_height: f32) -> f32 {
    let pixels_per_point = ui.ctx().pixels_per_point();
    // Font rows round up when laid out. Reserve the same physical-pixel height
    // before adding the button padding, including at fractional display scales.
    let text_height =
        (ui.text_style_height(&TextStyle::Button) * pixels_per_point).ceil() / pixels_per_point;
    let height = (text_height.max(icon_height) + 2.0 * ui.spacing().button_padding.y)
        .max(ui.spacing().interact_size.y);
    (height * pixels_per_point).ceil() / pixels_per_point
}

pub(crate) fn bar_panel(ui: &Ui, id: &'static str, icon_height: f32) -> egui::Panel {
    // Panel reserves this separator in its outer margin. Its exact height must
    // include that room so the full-height buttons end before the divider.
    let separator = ui.visuals().widgets.noninteractive.bg_stroke.width.round();
    egui::Panel::top(id)
        .exact_size(bar_height(ui, icon_height) + separator)
        .frame(Frame::new().fill(ui.visuals().panel_fill))
}

pub(crate) fn bar<R>(
    ui: &mut Ui,
    icon_height: f32,
    contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let height = bar_height(ui, icon_height);
    bar_with_height(ui, height, contents)
}

pub(crate) fn bar_with_height<R>(
    ui: &mut Ui,
    height: f32,
    contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    Frame::new().fill(ui.visuals().panel_fill).show(ui, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.spacing_mut().interact_size.y = height;
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            egui::Layout::left_to_right(egui::Align::Center),
            contents,
        )
        .inner
    })
}

fn fade_colors(body: Color32, text: Color32, fade: Fade) -> (Color32, Color32) {
    let fill = crate::theme::mix(body, text, smooth(fade.body));
    let mut ink = crate::theme::mix(text, body, smooth(fade.text)).to_srgba_unmultiplied();
    // Swapping two colors along the same RGB line necessarily crosses. Bend the
    // text's path perpendicular to that line near the crossing, using only the
    // original palette pair. Endpoints remain the exact exchanged colors.
    let a = body.to_srgba_unmultiplied();
    let b = text.to_srgba_unmultiplied();
    let d = std::array::from_fn::<_, 3, _>(|i| b[i] as f32 - a[i] as f32);
    let bend = [d[1] - d[2], d[2] - d[0], d[0] - d[1]];
    let bend = if bend.iter().all(|v| v.abs() < 1.0) {
        [d[0], -d[0], 0.0]
    } else {
        bend
    };
    let magnitude = d.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    let bend_scale = magnitude / bend.iter().map(|v| v.abs()).fold(1.0_f32, f32::max);
    let distance = (0..3)
        .map(|i| (ink[i] as f32 - fill.to_srgba_unmultiplied()[i] as f32).abs())
        .fold(0.0_f32, f32::max);
    let weight = (1.0 - distance / 64.0).clamp(0.0, 1.0).powi(2)
        * (std::f32::consts::PI * fade.text).sin()
        * 0.25
        * bend_scale;
    for i in 0..3 {
        ink[i] = (ink[i] as f32 + bend[i] * weight).round().clamp(0.0, 255.0) as u8;
    }
    (
        fill,
        Color32::from_rgba_unmultiplied(ink[0], ink[1], ink[2], ink[3]),
    )
}

/// Also used by custom-painted icons and window controls.
pub fn colors(ui: &Ui, response: &Response, style: ButtonStyle) -> (Color32, Color32) {
    let (body, text) = base_colors(ui, style);
    colors_from(ui, response, body, text)
}

fn colors_from(ui: &Ui, response: &Response, body: Color32, text: Color32) -> (Color32, Color32) {
    let fade = feedback(ui, response);
    if response.enabled() {
        fade_colors(body, text, fade)
    } else {
        (body, crate::theme::mix(body, text, 0.45))
    }
}

/// Subtle overlays keep the row underneath visible and leave glyphs legible.
pub(crate) fn subtle_colors(
    ui: &Ui,
    response: &Response,
    style: ButtonStyle,
    resting_opacity: f32,
) -> (Color32, Color32) {
    let (body, text) = base_colors(ui, style);
    let fade = feedback(ui, response);
    let resting_opacity = resting_opacity.clamp(0.0, 1.0);
    (
        text.gamma_multiply(resting_opacity + (1.0 - resting_opacity) * 0.16 * smooth(fade.body)),
        if response.enabled() {
            text
        } else {
            crate::theme::mix(body, text, 0.45)
        },
    )
}

fn feedback(ui: &Ui, response: &Response) -> Fade {
    let id = response.id.with("button-fade");
    let target = response.enabled()
        && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on());
    let time = ui.input(|i| i.time);
    let mut fade = ui.data_mut(|data| data.get_temp::<Fade>(id).unwrap_or_default());
    if response.enabled() {
        fade.advance(target, time);
    } else {
        fade = Fade {
            time: Some(time),
            ..Default::default()
        };
    }
    ui.data_mut(|data| data.insert_temp(id, fade));
    if !fade.settled(target) {
        ui.ctx().request_repaint();
    }
    fade
}

fn paint_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: std::sync::Arc<egui::Galley>,
    ink: Color32,
) {
    // An override is essential: the theme's text color is already baked into
    // the cached galley, so changing only its fallback leaves white-on-white.
    painter.add(egui::epaint::TextShape::new(pos, galley, ink).with_override_text_color(ink));
}

fn paint_contents(
    mut prepared: egui::AllocatedAtomLayout<'_>,
    ui: &Ui,
    ink: Color32,
    center_glyph: bool,
) -> Response {
    let mut labels = Vec::new();
    let button_id = prepared.response.id;
    let center = prepared.response.rect.center();
    for (index, atom) in prepared.iter_mut().enumerate() {
        if let egui::SizedAtomKind::Text(galley) = &atom.kind {
            let id = atom
                .id
                .unwrap_or_else(|| button_id.with(("button-text", index)));
            labels.push((id, galley.clone()));
            atom.id = Some(id);
            // Keep the measured text cell for layout, and paint its glyphs with
            // the same animated ink used by image and custom-painted buttons.
            atom.kind = egui::SizedAtomKind::Empty {
                size: Some(atom.size),
            };
        }
    }
    let output = prepared.paint(ui);
    for (id, galley) in labels {
        if let Some(rect) = output.rect(id) {
            let position = if center_glyph && galley.mesh_bounds.is_positive() {
                center - galley.mesh_bounds.center().to_vec2()
            } else {
                rect.min
            };
            paint_text(ui.painter(), position, galley, ink);
        }
    }
    output.response
}

pub struct Button<'a> {
    layout: AtomLayout<'a>,
    style: ButtonStyle,
    selected: Option<bool>,
    min_size: Vec2,
    tint_image: bool,
    square_corners: bool,
    primary_outline: bool,
    center_glyph: bool,
    subtle_body_opacity: Option<f32>,
    wants_click: bool,
    resting_colors: Option<(Color32, Color32)>,
}
impl<'a> Button<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>) -> Self {
        Self {
            layout: AtomLayout::new(atoms)
                .align2(egui::Align2::CENTER_CENTER)
                .sense(Sense::click())
                .fallback_font(TextStyle::Button),
            style: ButtonStyle::Standard,
            selected: None,
            min_size: Vec2::ZERO,
            tint_image: false,
            square_corners: false,
            primary_outline: false,
            center_glyph: false,
            subtle_body_opacity: None,
            wants_click: false,
            resting_colors: None,
        }
    }
    /// Resting body and label colors for one destructive action.
    /// Hover still exchanges those two palette colors.
    pub fn resting_colors(mut self, body: Color32, text: Color32) -> Self {
        self.resting_colors = Some((body, text));
        self
    }
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    /// A quiet tutorial cue, independent of the button's style and interaction.
    pub fn wants_click(mut self, wants_click: bool) -> Self {
        self.wants_click = wants_click;
        self
    }
    pub fn selectable(selected: bool, atoms: impl IntoAtoms<'a>) -> Self {
        Self::new(atoms).selected(selected)
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self.style = if selected {
            ButtonStyle::Accent
        } else {
            ButtonStyle::Standard
        };
        self
    }
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }
    pub(crate) fn id(mut self, id: egui::Id) -> Self {
        self.layout = self.layout.id(id);
        self
    }
    pub(crate) fn align(mut self, align: egui::Align2) -> Self {
        self.layout = self.layout.align2(align);
        self
    }
    pub fn wrap_mode(mut self, mode: egui::TextWrapMode) -> Self {
        self.layout = self.layout.wrap_mode(mode);
        self
    }
    /// Center an icon-only text glyph by its visible bounds in the button body.
    pub(crate) fn centered_glyph(mut self) -> Self {
        self.center_glyph = true;
        self
    }
    pub fn image_tint_follows_text_color(mut self, tint: bool) -> Self {
        self.tint_image = tint;
        self
    }
    pub fn icon_spacing(mut self, gap: f32) -> Self {
        self.layout = self.layout.gap(gap);
        self
    }
    /// Special treatment for the primary Steam validation/synchronization action.
    pub(crate) fn primary_outline(mut self) -> Self {
        self.square_corners = true;
        self.primary_outline = true;
        self
    }
    pub(crate) fn square_corners(mut self) -> Self {
        self.square_corners = true;
        self
    }
    pub(crate) fn subtle_body(mut self, resting_opacity: f32) -> Self {
        self.subtle_body_opacity = Some(resting_opacity);
        self
    }
    pub(crate) fn sense(mut self, sense: Sense) -> Self {
        self.layout = self.layout.sense(sense);
        self
    }
    /// Adjacent bar buttons blend into the panel; selected tabs retain Accent.
    pub fn bar(mut self) -> Self {
        self.style = if self.selected == Some(true) {
            ButtonStyle::Accent
        } else {
            ButtonStyle::Quiet
        };
        self.square_corners = true;
        self
    }
}
impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        crate::dropdown_navigation::prepare(ui);
        let mut layout = self.layout;
        let in_popup = crate::dropdowns::is_popup(ui);
        if in_popup {
            layout = layout.align2(egui::Align2::LEFT_CENTER);
        }
        let text = layout.text().map(|text| text.into_owned());
        let frame = Frame::new()
            .inner_margin(ui.spacing().button_padding)
            .corner_radius(if self.square_corners || in_popup {
                egui::CornerRadius::ZERO
            } else {
                ui.visuals().widgets.inactive.corner_radius
            });
        let mut prepared = layout
            .frame(frame)
            .min_size(
                self.min_size
                    .max(
                        if in_popup
                            && !ui.is_sizing_pass()
                            && ui.layout().main_dir() == egui::Direction::TopDown
                        {
                            Vec2::new(ui.available_width(), 0.0)
                        } else {
                            Vec2::ZERO
                        },
                    )
                    .max(Vec2::new(0.0, ui.spacing().interact_size.y)),
            )
            .allocate(ui);
        crate::dropdown_navigation::interact(
            ui,
            &mut prepared.response,
            self.selected == Some(true),
        );
        let resting_colors = self.resting_colors;
        let response = if ui.is_rect_visible(prepared.response.rect) {
            let (body, ink) = if let Some((body, text)) = resting_colors {
                colors_from(ui, &prepared.response, body, text)
            } else if let Some(resting_opacity) = self.subtle_body_opacity.filter(|_| !in_popup) {
                subtle_colors(ui, &prepared.response, self.style, resting_opacity)
            } else {
                colors(ui, &prepared.response, self.style)
            };
            prepared.frame.fill = body;
            if self.wants_click && prepared.response.enabled() && !ui.is_sizing_pass() {
                ui.painter().rect_filled(
                    prepared.response.rect,
                    prepared.frame.corner_radius,
                    body,
                );
                paint_attention(ui, prepared.response.rect, prepared.frame.corner_radius);
                prepared.frame.fill = Color32::TRANSPARENT;
            }
            prepared.fallback_text_color = ink;
            if self.tint_image {
                prepared.map_images(|image| image.tint(ink));
            }
            paint_contents(prepared, ui, ink, self.center_glyph)
        } else {
            prepared.response
        };
        if self.primary_outline && ui.is_rect_visible(response.rect) {
            ui.painter().rect_stroke(
                response.rect,
                0,
                egui::Stroke::new(1.0 / ui.ctx().pixels_per_point(), Color32::WHITE),
                egui::StrokeKind::Inside,
            );
        }
        response.widget_info(|| {
            let label = text.as_deref().unwrap_or("");
            if let Some(selected) = self.selected {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    selected,
                    label,
                )
            } else {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
            }
        });
        response
    }
}

fn paint_attention(ui: &Ui, rect: egui::Rect, radius: egui::CornerRadius) {
    let id = egui::Id::new("button-attention-stripes");
    let texture = ui
        .data_mut(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            // Screen(black, base) = base; Screen(white, base) = white.
            // White at 20% alpha is exactly that black/white screen overlay at 20%.
            let pixels = (0..32)
                .flat_map(|y| {
                    (0..32).map(move |x| {
                        if (x + y) % 32 < 16 {
                            Color32::from_white_alpha(51)
                        } else {
                            Color32::TRANSPARENT
                        }
                    })
                })
                .collect();
            let texture = ui.ctx().load_texture(
                "button-attention-stripes",
                egui::ColorImage::new([32, 32], pixels),
                egui::TextureOptions {
                    wrap_mode: egui::TextureWrapMode::Repeat,
                    ..egui::TextureOptions::LINEAR
                },
            );
            ui.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    let phase = ui.input(|input| (input.time * 0.35).rem_euclid(1.0) as f32);
    let uv = egui::Rect::from_min_size(egui::pos2(phase, 0.0), rect.size() / 32.0);
    ui.painter().add(
        egui::epaint::RectShape::filled(rect, radius, Color32::WHITE)
            .with_texture(texture.id(), uv),
    );
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}

pub trait UiButtonExt {
    fn app_button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> Response;
    fn app_selectable_label<'a>(&mut self, selected: bool, atoms: impl IntoAtoms<'a>) -> Response;
    fn app_selectable_value<'a, T: PartialEq>(
        &mut self,
        value: &mut T,
        choice: T,
        atoms: impl IntoAtoms<'a>,
    ) -> Response;
    fn app_menu_button<'a, R>(
        &mut self,
        atoms: impl IntoAtoms<'a>,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>>;
}
impl UiButtonExt for Ui {
    fn app_button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> Response {
        self.add(Button::new(atoms))
    }
    fn app_selectable_label<'a>(&mut self, selected: bool, atoms: impl IntoAtoms<'a>) -> Response {
        self.add(Button::selectable(selected, atoms))
    }
    fn app_selectable_value<'a, T: PartialEq>(
        &mut self,
        value: &mut T,
        choice: T,
        atoms: impl IntoAtoms<'a>,
    ) -> Response {
        let mut response = self.app_selectable_label(*value == choice, atoms);
        if response.clicked() && *value != choice {
            *value = choice;
            response.mark_changed();
        }
        if response.clicked() && crate::dropdowns::is_popup(self) {
            self.close();
        }
        response
    }
    fn app_menu_button<'a, R>(
        &mut self,
        atoms: impl IntoAtoms<'a>,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        menu(self, Button::new(atoms).style(ButtonStyle::Quiet), content)
    }
}
fn menu_style(style: &mut egui::Style) {
    style.spacing.item_spacing = Vec2::ZERO;
    style.spacing.menu_margin = egui::Margin::ZERO;
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.window_stroke = egui::Stroke::NONE;
    style.visuals.panel_fill = style.visuals.window_fill;
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
        widget.bg_stroke = egui::Stroke::NONE;
    }
}

pub(crate) fn context_menu<R>(
    response: &Response,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<egui::InnerResponse<R>> {
    let mut popup = egui::Popup::context_menu(response);
    if response.secondary_clicked() {
        let frame = response.ctx.cumulative_frame_nr();
        let click_id = popup.get_id().with("secondary-click-frame");
        let already_handled = response.ctx.data_mut(|data| {
            let previous = data.get_temp::<u64>(click_id);
            data.insert_temp(click_id, frame);
            previous == Some(frame)
        });
        // An open menu dismisses at its original anchor. Reopening it here
        // moves it for the closing frame; replaying the click in another
        // layout pass can then reopen it again after dismissal.
        if popup.is_open() || already_handled {
            popup = popup.open_memory(None);
        }
    }
    popup
        .style(menu_style)
        .show(|ui| crate::dropdowns::contents(ui, content))
}

/// A canvas menu opened by its completed click gesture, independently of card hit targets.
pub(crate) fn context_menu_at<R>(
    response: &Response,
    position: egui::Pos2,
    open: &mut bool,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<egui::InnerResponse<R>> {
    let result = egui::Popup::menu(response)
        .at_position(position)
        .open_memory((!*open).then_some(egui::SetOpenCommand::Bool(false)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .style(menu_style)
        .show(|ui| crate::dropdowns::contents(ui, content));
    *open = egui::Popup::is_id_open(&response.ctx, egui::Popup::default_response_id(response));
    result
}

pub fn menu<R>(
    ui: &mut Ui,
    button: Button<'_>,
    content: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let response = ui
        .scope(|ui| {
            let mut font = TextStyle::Button.resolve(&ui.ctx().global_style());
            font.size = (font.size - 1.0).max(1.0);
            ui.style_mut().text_styles.insert(TextStyle::Button, font);
            ui.add(button.bar())
        })
        .inner;
    let inner = menu_popup(ui, &response, content);
    egui::InnerResponse { response, inner }
}

pub(crate) fn menu_popup<R>(
    ui: &mut Ui,
    response: &Response,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    menu_popup_with_behavior(ui, response, None, content)
}

/// Search and other picker controls must remain open when their fields are clicked.
pub(crate) fn menu_popup_with_behavior<R>(
    ui: &mut Ui,
    response: &Response,
    behavior: Option<egui::PopupCloseBehavior>,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let inner = if egui::containers::menu::is_in_menu(ui) {
        egui::containers::menu::SubMenu::new().show(ui, response, |ui| {
            if !response.enabled() {
                ui.disable();
            }
            crate::dropdowns::contents(ui, content)
        })
    } else {
        let mut config = egui::containers::menu::MenuConfig::find(ui).style(menu_style);
        if let Some(behavior) = behavior {
            config = config.close_behavior(behavior);
        }
        egui::Popup::menu(response)
            .close_behavior(config.close_behavior)
            .style(config.style.clone())
            .info(
                egui::UiStackInfo::new(egui::UiKind::Menu)
                    .with_tag_value(egui::containers::menu::MenuConfig::MENU_CONFIG_TAG, config),
            )
            .show(|ui| {
                if !response.enabled() {
                    ui.disable();
                }
                crate::dropdowns::contents(ui, content)
            })
    };
    inner.map(|inner| inner.inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attention_stripes_animate_inside_the_body_and_skip_disabled_and_clipped_buttons() {
        let ctx = egui::Context::default();
        crate::tests::configure_test_fonts(&ctx);
        let id = egui::Id::new("attention-test");
        let draw = |time: f64, enabled: bool, visible: bool| {
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
                    ui.set_style(ui.ctx().global_style());
                    if !visible {
                        ui.set_clip_rect(egui::Rect::NOTHING);
                    }
                    rect = ui
                        .add_enabled(enabled, Button::new("Next step").id(id).wants_click(true))
                        .rect;
                },
            );
            let stripes: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(shape) if shape.brush.is_some() => Some(shape.clone()),
                    _ => None,
                })
                .collect();
            let texture = output
                .textures_delta
                .set
                .values()
                .flatten()
                .find(|delta| delta.image.size() == [32, 32])
                .map(|delta| {
                    let egui::ImageData::Color(image) = &delta.image;
                    image.clone()
                });
            output.textures_delta.clear();
            (rect, stripes, texture)
        };
        let (body, first, textures) = draw(0.0, true, true);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].rect, body);
        assert_eq!(first[0].stroke, egui::Stroke::NONE);
        let image = textures.unwrap();
        assert_eq!(
            image.pixels.iter().filter(|pixel| pixel.a() == 51).count(),
            512
        );
        assert_eq!(
            image.pixels.iter().filter(|pixel| pixel.a() == 0).count(),
            512
        );
        assert_eq!(image.pixels[0], image.pixels[31 * 32 + 1]);
        let (next_body, next, textures) = draw(0.5, true, true);
        assert_eq!(body, next_body);
        assert_ne!(first[0].brush, next[0].brush);
        assert!(textures.is_none(), "All buttons share one stripe texture");
        assert!(draw(1.0, false, true).1.is_empty());
        assert!(draw(1.5, true, false).1.is_empty());
    }
    #[test]
    fn layer_eye_and_fx_bodies_are_translucent_only_while_hovered_or_focused() {
        let ctx = egui::Context::default();
        crate::tests::configure_test_fonts(&ctx);
        let ids = [egui::Id::new("eye-feedback"), egui::Id::new("fx-feedback")];
        let mut time = 0.0;
        let mut frame = |events, enabled| {
            time += 0.05;
            let mut bounds = [egui::Rect::NOTHING; 2];
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::theme::apply(ui.ctx(), &achimeld_application::Preferences::default());
                    ui.set_style(ui.ctx().global_style());
                    ui.add_enabled_ui(enabled, |ui| {
                        ui.horizontal(|ui| {
                            for (index, icon) in
                                [crate::icons::Icon::Eye(true), crate::icons::Icon::Effects]
                                    .into_iter()
                                    .enumerate()
                            {
                                bounds[index] = crate::icons::layer_button(
                                    ui, ids[index], icon, "Control", 1.0, 0.0,
                                )
                                .rect;
                            }
                        });
                    });
                },
            );
            let mut rects = Vec::new();
            for shape in &output.shapes {
                collect_shapes(&shape.shape, &mut Vec::new(), &mut rects);
            }
            let alpha = bounds.map(|bounds| {
                rects
                    .iter()
                    .filter(|body| body.rect == bounds)
                    .map(|body| {
                        assert_eq!(body.stroke, egui::Stroke::NONE);
                        body.fill.a()
                    })
                    .max()
                    .unwrap_or(0)
            });
            output.textures_delta.clear();
            (bounds, alpha)
        };
        frame(vec![], true);
        let (bounds, alpha) = frame(vec![], true);
        assert_eq!(alpha, [0, 0]);
        for index in 0..2 {
            ctx.memory_mut(|memory| memory.surrender_focus(ids[1 - index]));
            frame(
                vec![egui::Event::PointerMoved(bounds[index].center())],
                true,
            );
            for _ in 0..6 {
                frame(vec![], true);
            }
            let (_, alpha) = frame(vec![], true);
            assert!(alpha[index] > 0 && alpha[index] < 64);
            assert_eq!(alpha[1 - index], 0);
            frame(
                vec![egui::Event::PointerMoved(egui::pos2(500.0, 500.0))],
                true,
            );
            for _ in 0..6 {
                frame(vec![], true);
            }
            assert_eq!(frame(vec![], true).1, [0, 0]);
            ctx.memory_mut(|memory| memory.request_focus(ids[index]));
            for _ in 0..6 {
                frame(vec![], true);
            }
            let (_, alpha) = frame(vec![], true);
            assert!(alpha[index] > 0 && alpha[index] < 64);
            assert_eq!(frame(vec![], false).1, [0, 0]);
        }
    }

    #[test]
    fn second_right_click_dismisses_context_menu_without_moving_it() {
        for extra_pass in [false, true] {
            let ctx = egui::Context::default();
            let mut time = 0.0;
            let mut frame = |events| {
                time += 0.02;
                let mut menus = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000.0, 800.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let response =
                            ui.allocate_response(ui.available_size(), egui::Sense::click());
                        if let Some(menu) = context_menu(&response, |ui| {
                            ui.app_button("Menu action");
                        }) {
                            menus.push(menu.response.rect);
                        }
                        if extra_pass && ui.ctx().current_pass_index() == 0 {
                            ui.ctx()
                                .request_discard("Context menu dismissal regression");
                        }
                    },
                );
                output.textures_delta.clear();
                menus
            };
            let pointer = |pos, pressed| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            frame(vec![]);
            let first = egui::pos2(150.0, 150.0);
            frame(pointer(first, true));
            frame(pointer(first, false));
            let original = *frame(vec![]).last().unwrap();
            let second = egui::pos2(650.0, 550.0);
            for pressed in [true, false] {
                for rect in frame(pointer(second, pressed)) {
                    assert_eq!(
                        rect, original,
                        "Menu moved during dismissal, extra_pass={extra_pass}"
                    );
                }
            }
            assert!(!egui::Popup::is_any_open(&ctx));
            assert!(frame(vec![]).is_empty());
            frame(pointer(second, true));
            frame(pointer(second, false));
            assert!(egui::Popup::is_any_open(&ctx));
            assert_eq!(frame(vec![]).last().unwrap().min, second);
        }
    }

    #[test]
    fn canvas_zoom_glyphs_are_visually_centered_at_each_font_size_and_scale() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for font_size in [12.0, 16.0, 24.0] {
                let ctx = egui::Context::default();
                crate::tests::configure_test_fonts(&ctx);
                ctx.set_pixels_per_point(scale);
                let preferences = achimeld_application::Preferences {
                    normal_size: font_size,
                    ..Default::default()
                };
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    crate::theme::apply(ui.ctx(), &preferences);
                    ui.set_style(ui.ctx().global_style());
                    ui.spacing_mut().interact_size.y = 24.0;
                    ui.spacing_mut().button_padding.y = 2.0;
                    ui.horizontal_centered(|ui| {
                        crate::Desktop::default().canvas_controls(ui);
                    });
                });
                let mut texts = Vec::new();
                let mut rects = Vec::new();
                for shape in &output.shapes {
                    collect_shapes(&shape.shape, &mut texts, &mut rects);
                }
                for symbol in ["−", "+"] {
                    let text = texts
                        .iter()
                        .find(|text| text.galley.text() == symbol)
                        .unwrap();
                    let visible = text.galley.mesh_bounds.translate(text.pos.to_vec2());
                    let body = rects
                        .iter()
                        .find(|rect| rect.rect.contains_rect(visible))
                        .unwrap();
                    assert!(
                        visible.center().distance(body.rect.center()) < 0.01,
                        "{symbol}, scale={scale}, font={font_size}: {visible:?}, {:?}",
                        body.rect,
                    );
                }
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn menu_command_glyphs_share_the_left_inset_including_disabled_rows() {
        for scale in [1.0, 1.25, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let mut heading = None;
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(700.0, 400.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        crate::theme::apply(
                            ui.ctx(),
                            &achimeld_application::Preferences::default(),
                        );
                        ui.set_style(ui.ctx().global_style());
                        heading = Some(crate::chrome::title_menu(ui, "Tools", |ui| {
                            ui.app_button("Short");
                            ui.app_button("A much longer menu command");
                            ui.add_enabled(false, Button::new("Disabled"));
                        }));
                    },
                );
                let mut texts = Vec::new();
                for shape in &output.shapes {
                    collect_shapes(&shape.shape, &mut texts, &mut Vec::new());
                }
                output.textures_delta.clear();
                (heading.as_ref().unwrap().clone(), texts)
            };
            frame(vec![]);
            let (response, _) = frame(vec![]);
            let pos = response.rect.center();
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            let (_, texts) = frame(vec![]);
            let x = |label: &str| {
                texts
                    .iter()
                    .find(|text| text.galley.text() == label)
                    .unwrap()
                    .pos
                    .x
            };
            assert!((x("Short") - x("A much longer menu command")).abs() <= 1.0 / scale);
            assert!((x("Disabled") - x("A much longer menu command")).abs() <= 1.0 / scale);
        }
    }
    fn collect_shapes(
        shape: &egui::Shape,
        texts: &mut Vec<egui::epaint::TextShape>,
        rects: &mut Vec<egui::epaint::RectShape>,
    ) {
        match shape {
            egui::Shape::Text(text) => texts.push(text.clone()),
            egui::Shape::Rect(rect) => rects.push(rect.clone()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_shapes(shape, texts, rects);
                }
            }
            _ => {}
        }
    }
    #[test]
    fn rendered_hover_text_keeps_font_and_bounds_while_title_menus_stay_square() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            check_hover_text_at_scale(scale);
        }
    }

    fn check_hover_text_at_scale(scale: f32) {
        let ctx = egui::Context::default();
        crate::tests::configure_test_fonts(&ctx);
        ctx.set_pixels_per_point(scale);
        let preferences = achimeld_application::Preferences::default();
        let primary = crate::theme::color(&preferences, "Primary text");
        let control = crate::theme::color(&preferences, "Control");
        let frame = |time, events| {
            let mut bounds = [egui::Rect::NOTHING; 2];
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 300.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    crate::theme::apply(ui.ctx(), &preferences);
                    ui.set_style(ui.ctx().global_style());
                    ui.horizontal(|ui| {
                        // Explicit light text reproduces the cached galley color
                        // that previously won over the animated fallback color.
                        bounds[0] = ui
                            .add(Button::new(egui::RichText::new("Action").color(primary)))
                            .rect;
                        bounds[1] = crate::chrome::title_menu_bar(ui, ["File"], |_, _| {})[0].rect;
                    });
                },
            );
            let mut texts = Vec::new();
            let mut rects = Vec::new();
            for shape in &output.shapes {
                collect_shapes(&shape.shape, &mut texts, &mut rects);
            }
            output.textures_delta.clear();
            (bounds, texts, rects)
        };
        frame(0.0, vec![]);
        let baseline = frame(0.01, vec![]);
        assert_eq!(
            baseline
                .1
                .iter()
                .filter(|text| text.galley.text() == "Action")
                .count(),
            1
        );
        frame(
            0.02,
            vec![egui::Event::PointerMoved(baseline.0[0].center())],
        );
        let hover = frame(0.43, vec![]);
        assert_eq!(hover.0, baseline.0);
        let labels: Vec<_> = hover
            .1
            .iter()
            .filter(|text| text.galley.text() == "Action")
            .collect();
        assert_eq!(labels.len(), 1, "Settled hover must paint one glyph run");
        let baseline_label = baseline
            .1
            .iter()
            .find(|text| text.galley.text() == "Action")
            .unwrap();
        assert_eq!(labels[0].pos, baseline_label.pos);
        assert_eq!(labels[0].galley.size(), baseline_label.galley.size());
        assert_eq!(
            labels[0].galley.job.sections,
            baseline_label.galley.job.sections
        );
        assert!(
            labels
                .iter()
                .all(|text| text.override_text_color == Some(control))
        );
        assert!(hover.2.iter().any(|rect| rect.rect == hover.0[0]
            && rect.fill == primary
            && rect.corner_radius == egui::CornerRadius::same(preferences.control_radius)));
        assert!(
            hover
                .2
                .iter()
                .any(|rect| rect.rect == hover.0[1]
                    && rect.corner_radius == egui::CornerRadius::ZERO)
        );
        frame(0.44, vec![egui::Event::PointerGone]);
        let resting = frame(0.85, vec![]);
        assert_eq!(resting.0, baseline.0);
        assert_eq!(
            resting
                .1
                .iter()
                .filter(|text| text.galley.text() == "Action")
                .count(),
            1
        );
        frame(
            0.86,
            vec![egui::Event::PointerMoved(baseline.0[1].center())],
        );
        let title = frame(1.27, vec![]);
        assert!(title.2.iter().any(|rect| rect.rect == title.0[1]
            && rect.fill == primary
            && rect.corner_radius == egui::CornerRadius::ZERO));
        let panel = crate::theme::color(&preferences, "Panel");
        assert!(
            title
                .1
                .iter()
                .filter(|text| text.galley.text() == "File")
                .all(|text| text.override_text_color == Some(panel))
        );
    }
    #[test]
    fn hover_reverses_from_current_progress_and_channels_finish_separately() {
        let mut fade = Fade::default();
        fade.advance(true, 0.0);
        fade.advance(true, 0.075);
        assert!((fade.body - 0.5).abs() < 0.001);
        assert!((fade.text - 0.375).abs() < 0.001);
        fade.advance(false, 0.10);
        assert!(fade.body < 0.5 && fade.text < 0.375);
        fade.advance(true, 0.125);
        assert!((fade.body - 0.5).abs() < 0.001);
        fade.advance(false, 1.0);
        assert!(fade.settled(false));
        fade.advance(true, 1.15);
        assert_eq!(fade.body, 1.0);
        assert!(fade.text < 1.0);
        fade.advance(true, 1.2);
        assert!(fade.settled(true));
    }
    #[test]
    fn negative_endpoints_are_exact_and_midpoint_colors_stay_distinct() {
        for (body, text) in [
            (Color32::from_gray(42), Color32::from_gray(243)),
            (Color32::from_gray(243), Color32::from_gray(42)),
            (
                Color32::from_rgb(42, 46, 53),
                Color32::from_rgb(241, 243, 245),
            ),
            (
                Color32::from_rgb(63, 74, 104),
                Color32::from_rgb(241, 243, 245),
            ),
            (
                Color32::from_rgb(32, 35, 41),
                Color32::from_rgb(241, 243, 245),
            ),
        ] {
            assert_eq!(fade_colors(body, text, Fade::default()), (body, text));
            assert_eq!(
                fade_colors(
                    body,
                    text,
                    Fade {
                        body: 1.0,
                        text: 1.0,
                        time: None
                    }
                ),
                (text, body)
            );
            for i in 1..400 {
                let t = i as f32 / 1000.0;
                let (fill, ink) = fade_colors(
                    body,
                    text,
                    Fade {
                        body: (t / BODY_SECONDS).min(1.0),
                        text: (t / TEXT_SECONDS).min(1.0),
                        time: None,
                    },
                );
                assert_ne!(fill, ink, "Colors crossed at {t}");
            }
        }
    }
}
