use super::*;
use achimeld_application::{IconStyle, style_viewer::StyleViewerSource};

impl Desktop {
    pub(crate) fn style_viewer_view(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let Some(state) = port.style_viewer().cloned() else {
            ui.spinner();
            ui.label("Opening Style Viewer…");
            port.request(UiRequest::StyleViewer(Action::Open));
            return;
        };
        let mut viewer = std::mem::take(&mut self.style_viewer_ui);
        if viewer.saved_unlock != state.preview.unlock_value {
            viewer.unlock = state.preview.unlock_value.clone();
            viewer.saved_unlock = state.preview.unlock_value.clone();
        }
        let mut actions = Vec::new();
        buttons::bar_panel(ui, "style-viewer-toolbar", 24.0).show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("style-viewer-toolbar-scroll")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    buttons::bar(ui, 24.0, |ui| {
                        for (label, action, name, bytes) in [
                            (
                                "Load collection…",
                                Action::LoadSet,
                                "style-set-load",
                                include_bytes!("../../assets/toolbar/style-set-load.rgba")
                                    .as_slice(),
                            ),
                            (
                                "Save collection",
                                Action::SaveSet,
                                "style-set-save",
                                include_bytes!("../../assets/toolbar/style-set-save.rgba")
                                    .as_slice(),
                            ),
                            (
                                "Save collection as…",
                                Action::SaveSetAs,
                                "style-set-save-as",
                                include_bytes!("../../assets/toolbar/style-set-save.rgba")
                                    .as_slice(),
                            ),
                            (
                                "New collection",
                                Action::NewCollection,
                                "style-collection-new",
                                include_bytes!("../../assets/toolbar/tab-art-bank.rgba").as_slice(),
                            ),
                        ] {
                            if ui
                                .add_enabled(
                                    !state.busy,
                                    buttons::Button::new((glyph(ui.ctx(), name, bytes), label))
                                        .id(egui::Id::new(("viewer-file", name)))
                                        .bar()
                                        .icon_spacing(5.0)
                                        .image_tint_follows_text_color(true)
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .clicked()
                            {
                                actions.push(action);
                            }
                        }
                        ui.add_space(12.0);
                        let collection = state
                            .collections
                            .iter()
                            .find(|item| Some(&item.id) == state.active_collection.as_ref());
                        ui.weak(collection.map_or("Project styles", |item| item.name.as_str()));
                        if state.dirty || state.collections.iter().any(|item| item.dirty) {
                            ui.weak(" · Unsaved changes");
                        }
                    });
                });
        });
        let wide = ui.available_width() >= 1040.0;
        let library_side = ui.available_width() >= 760.0;
        if library_side {
            egui::Panel::left("style-viewer-library")
                .default_size(300.0)
                .min_size(220.0)
                .max_size(380.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
                .show(ui, |ui| {
                    self.viewer_libraries(ui, port, &state, &mut actions);
                });
        }
        if wide {
            egui::Panel::right("style-viewer-bank")
                .default_size(300.0)
                .min_size(220.0)
                .max_size(380.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
                .show(ui, |ui| {
                    self.viewer_bank(ui, port, &state, &mut viewer, &mut actions);
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                if !library_side {
                    egui::CollapsingHeader::new("Project and app styles")
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.allocate_ui(egui::vec2(ui.available_width(), 260.0), |ui| {
                                self.viewer_libraries(ui, port, &state, &mut actions)
                            });
                        });
                }
                if !wide {
                    egui::CollapsingHeader::new("Preview artwork · Art Data Bank").show(ui, |ui| {
                        ui.allocate_ui(egui::vec2(ui.available_width(), 240.0), |ui| {
                            self.viewer_bank(ui, port, &state, &mut viewer, &mut actions)
                        });
                    });
                }
                if ui.available_width() >= 460.0 {
                    egui::Panel::left("style-viewer-inputs")
                        .default_size(220.0)
                        .min_size(180.0)
                        .max_size(300.0)
                        .resizable(true)
                        .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
                        .show(ui, |ui| {
                            self.viewer_inputs(ui, port, &state, &mut viewer, &mut actions);
                        });
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().inner_margin(12))
                        .show(ui, |ui| {
                            self.viewer_main_preview(ui, port, &state, &mut actions)
                        });
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("viewer-narrow-center")
                        .show(ui, |ui| {
                            self.viewer_inputs(ui, port, &state, &mut viewer, &mut actions);
                            self.viewer_main_preview(ui, port, &state, &mut actions);
                        });
                }
            });
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
        actions: &mut Vec<Action>,
    ) {
        let height = ui.available_height().max(120.0);
        let top = height * 0.45;
        ui.allocate_ui(egui::vec2(ui.available_width(), top), |ui| {
            buttons::bar(ui, 24.0, |ui| {
                if ui
                    .add_sized(
                        [ui.available_width(), ui.spacing().interact_size.y],
                        buttons::Button::selectable(
                            state.active_collection.is_none(),
                            "Project styles",
                        )
                        .bar()
                        .id(egui::Id::new("viewer-project-library")),
                    )
                    .clicked()
                {
                    actions.push(Action::SelectCollection(None));
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("viewer-project-styles-scroll")
                .max_height((top - buttons::bar_height(ui, 24.0)).max(40.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if state.styles.is_empty() {
                        ui.weak("Styles saved in this project appear here.");
                    }
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    for style in &state.styles {
                        self.viewer_banner(ui, port, state, style, actions);
                    }
                });
        });
        ui.separator();
        buttons::bar(ui, 24.0, |ui| {
            ui.add_space(8.0);
            ui.strong("App styles & collections");
        });
        egui::ScrollArea::vertical()
            .id_salt("viewer-app-styles-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for collection in &state.collections {
                    buttons::bar(ui, 24.0, |ui| {
                        let label = format!(
                            "{}{} · {}",
                            collection.name,
                            if collection.dirty { " *" } else { "" },
                            collection.styles.len()
                        );
                        let unload_width = ui
                            .painter()
                            .layout_no_wrap(
                                "Unload".into(),
                                egui::TextStyle::Button.resolve(ui.style()),
                                ui.visuals().text_color(),
                            )
                            .size()
                            .x
                            + ui.spacing().button_padding.x * 2.0;
                        let width = (ui.available_width()
                            - if collection.read_only {
                                0.0
                            } else {
                                unload_width
                            })
                        .max(1.0);
                        if ui
                            .add_sized(
                                [width, ui.spacing().interact_size.y],
                                buttons::Button::selectable(
                                    state.active_collection.as_ref() == Some(&collection.id),
                                    label,
                                )
                                .bar()
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                        {
                            actions.push(Action::SelectCollection(Some(collection.id.clone())));
                        }
                        if !collection.read_only
                            && ui
                                .add_enabled(!state.busy, buttons::Button::new("Unload").bar())
                                .on_hover_text("Unload this collection from the app library.")
                                .clicked()
                        {
                            actions.push(Action::UnloadCollection(collection.id.clone()));
                        }
                    });
                    if collection.styles.is_empty() {
                        ui.weak("Right-click a style to copy it into this collection.");
                    }
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        for style in &collection.styles {
                            let mut style = style.clone();
                            style.id = format!("{}/{}", collection.id, style.id);
                            self.viewer_banner(ui, port, state, &style, actions);
                        }
                    });
                }
            });
    }

    fn viewer_banner(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        style: &IconStyle,
        actions: &mut Vec<Action>,
    ) {
        let selected = state.selected_style.as_deref() == Some(&style.id);
        let width = ui.available_width();
        let band = ui.text_style_height(&egui::TextStyle::Button) + 12.0;
        let response = ui.add_sized(
            [width, width * 0.48 + band],
            buttons::Button::selectable(selected, "")
                .bar()
                .id(egui::Id::new(("viewer-banner", &style.id))),
        );
        let (fill, ink) = buttons::colors(
            ui,
            &response,
            if selected {
                buttons::ButtonStyle::Accent
            } else {
                buttons::ButtonStyle::Quiet
            },
        );
        let image =
            egui::Rect::from_min_max(response.rect.min, response.rect.max - egui::vec2(0.0, band));
        if ui.is_rect_visible(response.rect)
            && let Some((texture, _)) =
                self.viewer_texture(ui, port, &style.id, if selected { 512 } else { 256 })
        {
            ui.painter().image(
                texture,
                image,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.26), egui::pos2(1.0, 0.74)),
                egui::Color32::WHITE,
            );
        }
        let label = egui::Rect::from_min_max(
            egui::pos2(response.rect.left(), image.bottom()),
            response.rect.max,
        );
        ui.painter().rect_filled(label, 0, fill);
        let mut job = egui::text::LayoutJob::simple(
            style.name.clone(),
            egui::TextStyle::Button.resolve(ui.style()),
            ink,
            (width - 16.0).max(1.0),
        );
        job.wrap.max_rows = 1;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        ui.painter().with_clip_rect(label).add(
            egui::epaint::TextShape::new(label.center() - galley.size() * 0.5, galley, ink)
                .with_override_text_color(ink),
        );
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

    fn viewer_inputs(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        state: &StyleViewerState,
        viewer: &mut ViewerUi,
        actions: &mut Vec<Action>,
    ) {
        egui::ScrollArea::vertical().id_salt("viewer-inputs-scroll").auto_shrink([false, true]).show(ui, |ui| {
            egui::Frame::new().inner_margin(10).show(ui, |ui| {
                ui.strong("Preview inputs");
                ui.weak("Select a slot, then choose artwork on the right. These IDs drive every style preview.");
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
        actions: &mut Vec<Action>,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("style-viewer-canvas")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let style = state
                    .styles
                    .iter()
                    .find(|style| Some(&style.id) == state.selected_style.as_ref())
                    .cloned()
                    .or_else(|| {
                        let (collection, id) = state.selected_style.as_ref()?.split_once('/')?;
                        let mut style = state
                            .collections
                            .iter()
                            .find(|item| item.id == collection)?
                            .styles
                            .iter()
                            .find(|style| style.id == id)?
                            .clone();
                        style.id = format!("{collection}/{id}");
                        Some(style)
                    });
                if let Some(style) = style {
                    ui.vertical_centered(|ui| {
                        ui.heading(&style.name);
                        let edge = ui
                            .available_width()
                            .min((ui.available_height() - 130.0).max(160.0))
                            .min(680.0);
                        let (rect, response) =
                            ui.allocate_exact_size(egui::Vec2::splat(edge), egui::Sense::click());
                        ui.ctx().data_mut(|data| {
                            data.insert_temp(egui::Id::new("viewer-main-preview-rect"), rect)
                        });
                        if let Some((texture, size)) = self.viewer_texture(ui, port, &style.id, 512)
                        {
                            ui.painter().image(
                                texture,
                                fit(size, rect),
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Rendering preview…",
                                egui::TextStyle::Body.resolve(ui.style()),
                                ui.visuals().weak_text_color(),
                            );
                        }
                        self.viewer_style_menu(&response, port, &style, state, state.busy, actions);
                    });
                } else {
                    ui.weak("Choose a style from the library on the left.");
                }
                ui.add_space(12.0);
                let collection = state
                    .selected_style
                    .as_deref()
                    .and_then(|id| id.split_once('/'))
                    .and_then(|(id, _)| state.collections.iter().find(|item| item.id == id));
                if state.selected_style.is_some() && collection.is_none_or(|item| !item.read_only) {
                    egui::CollapsingHeader::new("Steam Achievement Set links").show(ui, |ui| {
                        let mut links = state.clone();
                        if let Some(collection) = collection {
                            links.steam_sets = collection
                                .steam_sets
                                .iter()
                                .cloned()
                                .map(|mut link| {
                                    link.style_id = format!("{}/{}", collection.id, link.style_id);
                                    link
                                })
                                .collect();
                        }
                        self.style_viewer_link.show(
                            ui,
                            &links,
                            port.editor().document().app_id(),
                            actions,
                        )
                    });
                }
                if !state.status.is_empty() {
                    ui.label(&state.status);
                }
            });
    }
}
