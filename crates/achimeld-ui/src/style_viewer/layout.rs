use super::*;
use achimeld_application::{IconStyle, style_viewer::StyleViewerSource};

impl Desktop {
    pub(crate) fn style_viewer_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let Some(state) = port.style_viewer().cloned() else {
            ui.spinner();
            port.request(UiRequest::StyleViewer(Action::Open));
            return;
        };
        let mut viewer = std::mem::take(&mut self.style_viewer_ui);
        if viewer.saved_unlock != state.preview.unlock_value {
            viewer.unlock = state.preview.unlock_value.clone();
            viewer.saved_unlock = state.preview.unlock_value.clone();
        }
        let mut actions = Vec::new();
        egui::Panel::left("style-viewer-library")
            .default_size(280.0)
            .min_size(180.0)
            .max_size(380.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                self.viewer_libraries(ui, port, &state, &mut viewer, &mut actions);
            });
        egui::Panel::right("style-viewer-properties")
            .default_size(300.0)
            .min_size(220.0)
            .max_size(480.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                self.sidebar_layer_properties(ui, port);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                self.viewer_main_preview(ui, port, &state, &mut viewer, &mut actions);
            });
        self.viewer_options(
            ui.ctx(),
            port,
            &state,
            &mut viewer,
            &mut actions,
            ui.is_enabled(),
        );
        // Context menus can open a deletion prompt while the presentation state is borrowed.
        viewer.deletion = self.style_viewer_ui.deletion.take().or(viewer.deletion);
        self.style_viewer_ui = viewer;
        for action in actions {
            port.request(UiRequest::StyleViewer(action));
        }
    }

    fn viewer_libraries(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        buttons::bar(ui, 24.0, |ui| {
            let icon = glyph(
                ui.ctx(),
                "style-options",
                include_bytes!("../../assets/toolbar/style-options.rgba"),
            );
            let response = ui.add(
                buttons::Button::new((icon, "Style options"))
                    .id(egui::Id::new("viewer-options-button"))
                    .bar()
                    .image_tint_follows_text_color(true),
            );
            if clicked_once(&response) {
                viewer.options_open = !viewer.options_open;
                if viewer.options_open {
                    ui.ctx().move_to_top(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("style-options-window"),
                    ));
                }
            }
        });
        buttons::bar(ui, 24.0, |ui| {
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut viewer.name_filter)
                    .id(egui::Id::new("viewer-name-filter"))
                    .hint_text("Filter styles by name")
                    .desired_width((ui.available_width() - 8.0).max(1.0)),
            );
        });
        let search = viewer.name_filter.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("viewer-libraries-scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            for builtin in [false, true] {
                let open = if builtin { &mut viewer.builtins_open } else { &mut viewer.project_open };
                let label = if builtin { "Built-in" } else { "Project" };
                buttons::bar(ui, 24.0, |ui| {
                    let response = ui.add_sized([ui.available_width(), ui.spacing().interact_size.y],
                        buttons::Button::new("").id(egui::Id::new(("viewer-folder",builtin))).bar());
                    let (_,ink) = buttons::colors(ui,&response,buttons::ButtonStyle::Quiet);
                    let center = response.rect.left_center();
                    icons::paint(ui.painter(),egui::Rect::from_center_size(center+egui::vec2(12.0,0.0),egui::Vec2::splat(12.0)),icons::Icon::Chevron(!*open),ink);
                    icons::paint(ui.painter(),egui::Rect::from_center_size(center+egui::vec2(32.0,0.0),egui::Vec2::splat(18.0)),icons::Icon::Folder,ink);
                    ui.painter().text(center+egui::vec2(48.0,0.0),egui::Align2::LEFT_CENTER,label,egui::TextStyle::Button.resolve(ui.style()),ink);
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button,response.enabled(),label));
                    if clicked_once(&response) { *open = !*open; }
                });
                if !*open { continue; }
                if builtin {
                    for collection in state.collections.iter().filter(|c| c.read_only && c.name == "Built-in") {
                        for style in collection.styles.iter().filter(|s| s.name.to_lowercase().contains(&search)) {
                            let mut style = style.clone();
                            style.id = format!("{}/{}", collection.id, style.id);
                            self.viewer_banner(ui, port, state, &style, viewer.banner_height, actions);
                        }
                    }
                } else {
                    for style in state.styles.iter().filter(|s| s.name.to_lowercase().contains(&search)) { self.viewer_banner(ui, port, state, style, viewer.banner_height, actions); }
                    if state.styles.is_empty() { ui.weak("No project styles yet. Duplicate a built-in style or load styles in Style options."); }
                }
            }
        });
    }

    fn viewer_options(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
        enabled: bool,
    ) {
        let mut open = viewer.options_open;
        egui::Window::new("Style options")
            .id(egui::Id::new("style-options-window"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .enabled(enabled)
            .default_width(430.0)
            .default_height(600.0)
            .min_width(330.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                buttons::bar(ui, 24.0, |ui| {
                    for (label, action, name, bytes) in [
                        (
                            "Load styles…",
                            Action::LoadJson,
                            "style-json-load",
                            include_bytes!("../../assets/toolbar/style-set-load.rgba").as_slice(),
                        ),
                        (
                            "Save styles…",
                            Action::SaveJson,
                            "style-json-save",
                            include_bytes!("../../assets/toolbar/style-set-save.rgba").as_slice(),
                        ),
                    ] {
                        if ui
                            .add_enabled(
                                !state.busy,
                                buttons::Button::new((glyph(ui.ctx(), name, bytes), label))
                                    .id(egui::Id::new(("viewer-file", name)))
                                    .bar()
                                    .image_tint_follows_text_color(true),
                            )
                            .clicked()
                        {
                            actions.push(action);
                        }
                    }
                });
                ui.scope(|ui| {
                    let small = egui::TextStyle::Small.resolve(ui.style());
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, small);
                    ui.spacing_mut().interact_size.y = 20.0;
                    ui.spacing_mut().button_padding = egui::vec2(5.0, 2.0);
                    if ui
                        .add_enabled(
                            !state.busy,
                            buttons::Button::new("Clone all built-ins")
                                .id(egui::Id::new("viewer-clone-builtins"))
                                .bar(),
                        )
                        .on_hover_text(
                            "Create editable copies of every built-in style in the current project",
                        )
                        .clicked()
                    {
                        actions.push(Action::CloneBuiltins);
                    }
                });
                let mut height = f64::from(viewer.banner_height);
                if crate::sliders::value_slider(
                    ui,
                    "Banner height",
                    &mut height,
                    48.0..=240.0,
                    " pt",
                    0,
                ) {
                    viewer.banner_height = height as f32;
                }
                let mut resolution = viewer.resolution_power - 6;
                if crate::sliders::discrete_slider(
                    ui,
                    "Resolution",
                    &mut resolution,
                    &[64, 128, 256, 512],
                    " px",
                ) {
                    viewer.resolution_power = resolution + 6;
                }
                ui.weak("JSON stores layer recipes. Image and atlas references use the current project’s Art Data Bank.");
                self.viewer_sidebar(ui, port, state, viewer, actions);
            });
        viewer.options_open = open;
    }

    fn viewer_banner(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        style: &IconStyle,
        banner_height: f32,
        actions: &mut Vec<Action>,
    ) {
        let selected = state.selected_style.as_deref() == Some(&style.id);
        let width = ui.available_width();
        let height = banner_height
            .min(width * 0.65)
            .max(ui.text_style_height(&egui::TextStyle::Button) + 12.0);
        let response = ui.add_sized(
            [width, height],
            buttons::Button::selectable(selected, "")
                .bar()
                .id(egui::Id::new(("viewer-banner", &style.id))),
        );
        let texture = if ui.is_rect_visible(response.rect) {
            self.viewer_texture(ui, port, &style.id, 256)
        } else {
            None
        };
        crate::style_banner::paint_viewer(ui, &response, &style.name, texture, selected);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                response.enabled(),
                selected,
                &style.name,
            )
        });
        if response.clicked() {
            actions.push(Action::SelectStyle(style.id.clone()));
        }
        self.viewer_style_menu(&response, port, style, state, state.busy, actions);
    }

    fn viewer_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        egui::CollapsingHeader::new("Preview inputs")
            .default_open(true)
            .show(ui, |ui| {
                self.viewer_inputs(ui, port, state, viewer, actions);
            });
        self.viewer_bank(ui, port, state, viewer, actions);
    }

    fn viewer_inputs(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        egui::ScrollArea::vertical().id_salt("viewer-inputs-scroll").max_height((ui.available_height() * 0.45).clamp(100.0, 300.0)).auto_shrink([false, true]).show(ui, |ui| {
            egui::Frame::new().inner_margin(10).show(ui, |ui| {
                ui.weak("Select a slot, then choose artwork below. These IDs drive every style preview.");
                crate::fields::row(ui, "Unlock value", |ui| {
                    let field = ui.add(egui::TextEdit::singleline(&mut viewer.unlock).id(egui::Id::new("viewer-unlock-value")).desired_width(74.0).char_limit(64));
                    let mut candidate = state.preview.clone(); candidate.unlock_value = viewer.unlock.trim().into();
                    if field.changed() && candidate.validate().is_ok() { actions.push(Action::SetUnlockValue(candidate.unlock_value.clone())); }
                    if candidate.validate().is_err() { ui.weak("Enter a number"); }
                });
            });
            for index in 0..3 {
                ui.push_id(("viewer-slot-card", index), |ui| {
                    buttons::bar(ui, 24.0, |ui| {
                        let clear_width = ui.painter().layout_no_wrap("Clear".into(), egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color()).size().x + ui.spacing().button_padding.x * 2.0;
                        let width = (ui.available_width() - clear_width).max(1.0);
                        if ui.add_sized([width, ui.spacing().interact_size.y], buttons::Button::selectable(viewer.slot == index, format!("Image ID {index}")).bar().wrap_mode(egui::TextWrapMode::Truncate).id(egui::Id::new(("viewer-slot", index)))).clicked() { viewer.slot = index; }
                        if ui.add_enabled((state.preview.slot(index).is_some() || state.preview.uses_default(index)) && !state.busy, buttons::Button::new("Clear").bar()).clicked() {
                            actions.push(Action::SetImageSlot { index, source:None });
                        }
                    });
                    let response = ui.add_sized([ui.available_width(), 116.0], buttons::Button::selectable(viewer.slot == index, "").bar());
                    if response.clicked() { viewer.slot = index; }
                    let mut caption = "Choose artwork".to_owned();
                    if let Some(source) = state.preview.slot(index) {
                        let (atlas, id) = match source { StyleViewerSource::Image { id } => (false, Some(id.as_str())), StyleViewerSource::Atlas { id, .. } => (true, id.as_deref()) };
                        caption = if let StyleViewerSource::Atlas { tile, .. } = source { format!("Atlas tile {tile}") } else { "Image".into() };
                        if let Some(preview) = port.asset_preview(atlas, id) {
                            let source_size = preview.source_size;
                            let mut size = egui::vec2(preview.width as f32, preview.height as f32);
                            let uv = if let StyleViewerSource::Atlas { tile, .. } = source {
                                crate::source_picker::tile_uv(source_size, &port.editor().document().project()["atlas"], *tile)
                            } else { None }.unwrap_or_else(|| egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)));
                            size = size * uv.size();
                            if let Some(texture) = self.preview_texture(ui.ctx(), &format!("viewer-slot-art:{atlas}:{}", id.unwrap_or("default")), preview) {
                                ui.painter().image(texture, fit(size, response.rect.shrink(8.0)), uv, egui::Color32::WHITE);
                                caption.clear();
                            }
                        } else { caption = "Artwork unavailable · repair in Art Data Bank".into(); }
                    } else if state.preview.uses_default(index) {
                        caption = "Default achievement cup".into();
                        if let Some(preview) = port.style_viewer_default_image_preview() {
                            let size = egui::vec2(preview.width as f32, preview.height as f32);
                            if let Some(texture) = self.preview_texture(ui.ctx(), "viewer-default-cup", preview) {
                                ui.painter().image(texture, fit(size, response.rect.shrink(8.0)), egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                                caption.clear();
                            }
                        }
                    }
                    if !caption.is_empty() {
                        let (_, ink) = buttons::colors(ui, &response, if viewer.slot == index { buttons::ButtonStyle::Accent } else { buttons::ButtonStyle::Quiet });
                        let mut job = egui::text::LayoutJob::simple(caption, egui::TextStyle::Small.resolve(ui.style()), ink, (response.rect.width() - 16.0).max(1.0));
                        job.halign = egui::Align::Center; job.wrap.max_rows = 3;
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                        ui.painter().with_clip_rect(response.rect).add(egui::epaint::TextShape::new(egui::pos2(response.rect.center().x, response.rect.center().y - galley.size().y * 0.5), galley, ink).with_override_text_color(ink));
                    }
                });
            }
        });
    }

    fn viewer_main_preview(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        let style = state.selected();
        buttons::bar(ui, 20.0, |ui| {
            if ui
                .add(
                    buttons::Button::new("Center")
                        .id(egui::Id::new("viewer-center"))
                        .bar(),
                )
                .on_hover_text("Center the icon and reset zoom")
                .clicked()
            {
                viewer.pan = egui::Vec2::ZERO;
                viewer.zoom = 1.0;
            }
        });
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size().max(egui::Vec2::splat(1.0)),
            egui::Sense::click_and_drag(),
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new("viewer-main-preview-rect"), rect));
        if response.dragged() && ui.is_enabled() {
            viewer.pan += ui.input(|i| i.pointer.delta());
        }
        if response.hovered()
            && ui.is_enabled()
            && let Some(pointer) = response.hover_pos()
        {
            let scroll = ui.input_mut(|i| {
                let scroll = i.smooth_scroll_delta.y;
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                scroll
            });
            if scroll != 0.0 {
                viewer.zoom_at(rect, pointer, (scroll * 0.002).exp());
            }
        }
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if let Some(style) = style {
            let edge = rect.width().min(rect.height()).min(512.0) * 0.75 * viewer.zoom;
            let image_rect =
                egui::Rect::from_center_size(rect.center() + viewer.pan, egui::Vec2::splat(edge));
            if let Some((texture, size)) =
                self.viewer_texture(ui, port, &style.id, 1usize << viewer.resolution_power)
            {
                painter.image(
                    texture,
                    fit(size, image_rect),
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                painter.text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Rendering preview…",
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
            }
            self.viewer_style_menu(&response, port, &style, state, state.busy, actions);
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Choose a style",
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
        if !state.status.is_empty() {
            painter.text(
                rect.left_bottom() - egui::vec2(-8.0, 8.0),
                egui::Align2::LEFT_BOTTOM,
                &state.status,
                egui::TextStyle::Small.resolve(ui.style()),
                ui.visuals().weak_text_color(),
            );
        }
    }
}

impl ViewerUi {
    pub(crate) fn zoom_at(&mut self, rect: egui::Rect, pointer: egui::Pos2, factor: f32) {
        let previous = self.zoom;
        self.zoom = (previous * factor).clamp(0.1, 16.0);
        let anchor = pointer - rect.center();
        self.pan = anchor - (anchor - self.pan) * (self.zoom / previous);
    }
}

#[cfg(test)]
mod camera_tests {
    use super::*;
    #[test]
    fn wheel_anchor_survives_offset_panning_smoothing_and_limits() {
        let rect = egui::Rect::from_min_size(egui::pos2(280.0, 110.0), egui::vec2(750.0, 650.0));
        let pointer = rect.min + egui::vec2(183.0, 223.0);
        let mut viewer = ViewerUi {
            pan: egui::vec2(-173.0, 86.0),
            ..Default::default()
        };
        let world = (pointer - rect.center() - viewer.pan) / viewer.zoom;
        for factor in [1.1, 1.04, 1.02, 0.83, 0.97, 1000.0, 2.0, 0.00001, 0.5] {
            viewer.zoom_at(rect, pointer, factor);
            let current = rect.center() + viewer.pan + world * viewer.zoom;
            assert!(current.distance(pointer) < 0.003, "{current:?} {pointer:?}");
        }
    }
}
