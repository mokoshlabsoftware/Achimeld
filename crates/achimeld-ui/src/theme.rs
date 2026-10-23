use achimeld_application::Preferences;
use egui::{Color32, FontId, Stroke, TextStyle};
pub fn color(preferences: &Preferences, name: &str) -> Color32 {
    let c = preferences
        .palette
        .get(name)
        .copied()
        .unwrap_or(preferences.palette["Control"]);
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let a = a.to_srgba_unmultiplied();
    let b = b.to_srgba_unmultiplied();
    let c = std::array::from_fn::<_, 4, _>(|i| {
        (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)).round() as u8
    });
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}
pub fn apply(ctx: &egui::Context, p: &Preferences) {
    crate::fields::configure(ctx, p);
    crate::typography::ensure_fonts(ctx, &p.language);
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, p.inspector_spacing);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(p.header_size));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(p.normal_size));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(p.normal_size));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(p.small_size));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(p.normal_size));
    let v = &mut style.visuals;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.text_options.font_hinting = true;
    v.text_options.subpixel_binning = true;
    let window = color(p, "Window").to_array();
    v.dark_mode = u16::from(window[0]) + u16::from(window[1]) + u16::from(window[2]) < 384;
    v.override_text_color = Some(color(p, "Primary text"));
    v.weak_text_color = Some(color(p, "Muted text"));
    v.text_edit_bg_color = Some(color(p, "Window"));
    v.code_bg_color = color(p, "Control");
    v.text_cursor.stroke = Stroke::new(2.0, color(p, "Accent"));
    v.window_corner_radius = egui::CornerRadius::same(p.control_radius);
    v.window_shadow.color = color(p, "Window").gamma_multiply(0.4);
    v.popup_shadow.color = color(p, "Window").gamma_multiply(0.4);
    v.panel_fill = color(p, "Panel");
    v.window_fill = color(p, "Window");
    v.extreme_bg_color = color(p, "Window");
    v.faint_bg_color = color(p, "Control");
    v.window_stroke = Stroke::new(1.0, color(p, "Border"));
    v.selection.bg_fill = mix(color(p, "Control"), color(p, "Accent"), 0.25);
    v.selection.stroke = Stroke::new(1.0, color(p, "Accent"));
    v.warn_fg_color = color(p, "Warning");
    v.error_fg_color = color(p, "Error");
    v.hyperlink_color = color(p, "Accent");
    let configure = |widget: &mut egui::style::WidgetVisuals, t: f32| {
        widget.bg_fill = mix(color(p, "Control"), color(p, "Accent"), t);
        widget.weak_bg_fill = widget.bg_fill;
        widget.bg_stroke = Stroke::new(1.0, color(p, "Border"));
        widget.fg_stroke = Stroke::new(1.0, color(p, "Primary text"));
        widget.corner_radius = egui::CornerRadius::same(p.control_radius);
    };
    configure(&mut v.widgets.noninteractive, 0.0);
    configure(&mut v.widgets.inactive, 0.0);
    configure(&mut v.widgets.hovered, 0.15);
    configure(&mut v.widgets.active, 0.30);
    configure(&mut v.widgets.open, 0.20);
    ctx.set_global_style(style);
}
