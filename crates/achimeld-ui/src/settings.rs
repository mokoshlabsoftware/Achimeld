use crate::Desktop;
use crate::buttons::UiButtonExt as _;
use crate::inspector::{choice, integer, number, text};
use achimeld_application::{Command, EditorPort};
impl Desktop {
    pub(crate) fn open_preferences(&mut self, port: &dyn EditorPort) {
        self.preferences = Some(port.preferences().clone());
        self.preference_base = self.preferences.clone();
    }
    pub(crate) fn apply_settings(&mut self, port: &mut dyn EditorPort) -> bool {
        if port.editor().revision() != self.settings_revision {
            self.error = Some("The project changed while settings were open. Reopen settings to apply your changes.".into());
            return false;
        }
        self.apply(
            port,
            "Project settings",
            Command::ProjectSettings {
                name: self.project_name.clone(),
                app_id: self.app_id,
                atlas: Some(self.atlas.clone()),
                assets: None,
            },
            None,
        )
    }
    pub(crate) fn open_settings(&mut self, port: &dyn EditorPort) {
        let p = port.editor().document().project();
        self.project_name = p["displayName"].as_str().unwrap_or("").into();
        self.app_id = port.editor().document().app_id();
        self.atlas = p["atlas"].clone();
        if self.atlas["unachievedIcon"].is_null() {
            self.atlas["unachievedIcon"] =
                serde_json::json!({"mode":"Grayscale","opacity":1.0,"tintHex":"#808080"});
        }
        for key in ["outputWidth", "outputHeight"] {
            if self.atlas[key].is_null() {
                self.atlas[key] = self.atlas["outputSize"]
                    .as_u64()
                    .map_or(serde_json::json!(512), |n| serde_json::json!(n));
            }
        }
        self.settings_revision = port.editor().revision();
        self.settings = true;
    }
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        if !self.settings {
            return;
        }
        let mut open = true;
        egui::Window::new("Project settings")
            .fade_in(false)
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                crate::fields::row(ui, "Project name", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.project_name)
                            .desired_width(ui.available_width()),
                    );
                });
                crate::fields::row(ui, "Steam App ID", |ui| {
                    ui.add(egui::DragValue::new(&mut self.app_id));
                });
                ui.separator();
                ui.heading(crate::localization::text(ui.ctx(), "Atlas and output"));
                for (label, key) in [
                    ("Tile width", "tileWidth"),
                    ("Tile height", "tileHeight"),
                    ("Output width", "outputWidth"),
                    ("Output height", "outputHeight"),
                ] {
                    integer(ui, label, &mut self.atlas, key, 1..=4096);
                }
                for (label, key) in [
                    ("Left offset", "leftOffset"),
                    ("Top offset", "topOffset"),
                    ("Horizontal spacing", "horizontalSpacing"),
                    ("Vertical spacing", "verticalSpacing"),
                ] {
                    integer(ui, label, &mut self.atlas, key, 0..=4096);
                }
                choice(
                    ui,
                    "Traversal",
                    &mut self.atlas,
                    "traversal",
                    &["RowMajor", "ColumnMajor"],
                );
                ui.separator();
                ui.heading("Unachieved icons");
                let unachieved = &mut self.atlas["unachievedIcon"];
                choice(ui, "Mode", unachieved, "mode", &["Grayscale", "Tint"]);
                number(ui, "Opacity", unachieved, "opacity", 0.0..=1.0, 0.01);
                if unachieved["mode"] == "Tint" {
                    text(ui, "Tint", unachieved, "tintHex");
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .app_button(crate::localization::text(ui.ctx(), "Cancel"))
                        .clicked()
                    {
                        self.settings = false;
                    }
                    if ui
                        .app_button(crate::localization::text(ui.ctx(), "Apply"))
                        .clicked()
                        && self.apply_settings(port)
                    {
                        self.settings = false;
                    }
                });
            });
        if !open {
            self.settings = false;
        }
    }
    pub(crate) fn preferences_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let Some(mut preferences) = self.preferences.take() else {
            return;
        };
        let icons = self
            .preference_icons
            .get_or_insert_with(|| {
                [
                    (
                        "general",
                        include_bytes!("../assets/toolbar/preferences-general.rgba").as_slice(),
                    ),
                    (
                        "colors",
                        include_bytes!("../assets/toolbar/preferences-colors.rgba").as_slice(),
                    ),
                    (
                        "app-style",
                        include_bytes!("../assets/toolbar/preferences-app-style.rgba").as_slice(),
                    ),
                    (
                        "naming",
                        include_bytes!("../assets/toolbar/preferences-naming.rgba").as_slice(),
                    ),
                    (
                        "browser",
                        include_bytes!("../assets/toolbar/preferences-browser.rgba").as_slice(),
                    ),
                ]
                .map(|(name, bytes)| {
                    ctx.load_texture(
                        format!("preferences-{name}"),
                        egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                        egui::TextureOptions::LINEAR
                            .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                    )
                })
            })
            .each_ref()
            .map(egui::TextureHandle::id);
        let mut keep = true;
        let panel = crate::theme::color(&preferences, "Panel");
        egui::Window::new(crate::localization::text(ctx, "Preferences"))
            .id(egui::Id::new("preferences-window"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .title_bar(false)
            .collapsible(false)
            .default_size([700.0, 490.0])
            .min_size([400.0, 300.0])
            .frame(egui::Frame::window(&ctx.global_style())
                .fill(panel)
                .corner_radius(egui::CornerRadius::ZERO)
                .inner_margin(0))
            .show(ctx, |ui| {
                // Preferences follows the square application bars, independently
                // of the corner radius being previewed for other controls.
                let visuals = ui.visuals_mut();
                for widget in [
                    &mut visuals.widgets.noninteractive, &mut visuals.widgets.inactive,
                    &mut visuals.widgets.hovered, &mut visuals.widgets.active,
                    &mut visuals.widgets.open,
                ] {
                    widget.corner_radius = egui::CornerRadius::ZERO;
                }
                visuals.window_corner_radius = egui::CornerRadius::ZERO;
                visuals.menu_corner_radius = egui::CornerRadius::ZERO;
                ui.spacing_mut().item_spacing.y = 0.0;
                let row_height = crate::buttons::bar_height(ui, 24.0);
                let header_height = row_height.max(ui.text_style_height(&egui::TextStyle::Heading)
                    + 2.0 * ui.spacing().button_padding.y);
                egui::Panel::top("preferences-header")
                    .exact_size(header_height)
                    .frame(egui::Frame::new().fill(panel))
                    .show(ui, |ui| {
                        crate::buttons::bar_with_height(ui, header_height, |ui| {
                            let rect = ui.max_rect();
                            ui.add_sized([header_height, header_height],
                                egui::Image::new((icons[0], egui::vec2(16.0, 16.0)))
                                    .fit_to_exact_size(egui::vec2(16.0, 16.0))
                                    .tint(ui.visuals().text_color()));
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                                crate::localization::text(ui.ctx(), "Preferences"),
                                egui::TextStyle::Heading.resolve(ui.style()), ui.visuals().text_color());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_sized([header_height, header_height],
                                    crate::buttons::Button::new("×").bar().centered_glyph())
                                    .on_hover_text(crate::localization::text(ui.ctx(), "Close")).clicked() {
                                    keep = false;
                                }
                            });
                        });
                    });
                egui::Panel::bottom("preferences-actions")
                    .exact_size(row_height)
                    .frame(egui::Frame::new().fill(panel))
                    .show(ui, |ui| {
                        crate::buttons::bar_with_height(ui, row_height, |ui| {
                            let width = ui.available_width() / 2.0;
                            if ui.add_sized([width, row_height],
                                crate::buttons::Button::new(crate::localization::text(ui.ctx(), "Cancel")).bar()).clicked() {
                                keep = false;
                            }
                            if ui.add_sized([ui.available_width(), row_height],
                                crate::buttons::Button::new(crate::localization::text(ui.ctx(), "Save")).bar()).clicked() {
                                if self.preference_base.as_ref().is_some_and(|base| base != port.preferences()) {
                                    self.error = Some("Preferences changed while this window was open. Reopen Preferences to apply your changes.".into());
                                } else {
                                    keep = !self.apply(port, "Preferences", Command::Preferences(preferences.clone()), None);
                                }
                            }
                        });
                    });
                let categories = ["General", "Colors", "App style", "Naming", "Browser"]
                    .map(|label| crate::localization::text(ui.ctx(), label));
                let navigation_width = categories.iter().map(|label| ui.painter().layout_no_wrap(
                    label.clone(), egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color()
                ).size().x + 28.0 + 2.0 * ui.spacing().button_padding.x).fold(130.0_f32, f32::max)
                    .min(ui.available_width() * 0.42);
                egui::Panel::left("preferences-navigation")
                    .exact_size(navigation_width)
                    .resizable(false)
                    .frame(egui::Frame::new().fill(panel))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        egui::ScrollArea::vertical().id_salt("preferences-sections").show(ui, |ui| {
                            for (index, label) in categories.iter().enumerate() {
                                if ui.add_sized([ui.available_width(), row_height],
                                    crate::buttons::Button::selectable(self.pref_category == index,
                                        (egui::Image::new((icons[index], egui::Vec2::splat(20.0))), label.as_str()))
                                        .bar().align(egui::Align2::LEFT_CENTER).icon_spacing(8.0)
                                        .image_tint_follows_text_color(true)
                                        .wrap_mode(egui::TextWrapMode::Truncate)).clicked() {
                                    self.pref_category = index;
                                }
                            }
                        });
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(panel).inner_margin(12))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = preferences.inspector_spacing;
                        egui::ScrollArea::vertical().id_salt(("preferences-page", self.pref_category))
                            .auto_shrink([false, false]).show(ui, |ui| {
                                self.preferences_page(ui, port, &mut preferences, &mut keep);
                            });
                    });
            });
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            keep = false;
        }
        if keep {
            self.preferences = Some(preferences);
        } else {
            self.preference_base = None;
        }
    }

    fn preferences_page(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        preferences: &mut achimeld_application::Preferences,
        keep: &mut bool,
    ) {
        match self.pref_category {
            0 => {
                crate::fields::row(ui, "Achimeld folder", |ui| {
                    ui.add(
                        egui::Label::new(
                            port.settings_folder()
                                .map(|path| path.display().to_string())
                                .unwrap_or_else(|| "Default application folder".into()),
                        )
                        .truncate(),
                    );
                });
                if preference_action(ui, "Choose folder…").clicked() {
                    self.request(achimeld_application::UiRequest::ChooseSettingsFolder);
                    *keep = false;
                }
                ui.label("Shared preferences, gradients and icon styles are kept here. Project localization CSVs are saved beside the .achv file.");
                ui.separator();
                crate::fields::row(ui, "Language", |ui| {
                    crate::dropdowns::Dropdown::from_id_salt("language")
                        .width(ui.available_width().min(180.0))
                        .truncate()
                        .selected_text(&preferences.language)
                        .show_ui(ui, |ui| {
                            for language in achimeld_core::LANGUAGES {
                                if ui
                                    .add(
                                        crate::buttons::Button::selectable(
                                            preferences.language == language,
                                            language,
                                        )
                                        .bar(),
                                    )
                                    .clicked()
                                {
                                    preferences.language = language.into();
                                    ui.close();
                                }
                            }
                        });
                });
                ui.separator();
                crate::fields::checkbox(
                    ui,
                    "Layer properties in right sidebar",
                    &mut preferences.layer_properties_in_sidebar,
                );
                ui.weak(crate::localization::text(ui.ctx(),
                    "Use a compact Layers window and edit the selected layer or effect in the right sidebar."));
                crate::fields::checkbox(
                    ui,
                    "Display mini values in FX rows",
                    &mut preferences.show_effect_mini_values,
                );
            }
            1 => {
                let panel = ui.visuals().panel_fill;
                ui.visuals_mut().widgets.inactive.bg_fill = panel;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = panel;
                for (name, color) in &mut preferences.palette {
                    crate::fields::row(ui, name, |ui| {
                        crate::color_picker::edit_button(ui, color);
                    });
                }
                ui.separator();
                if preference_action(ui, "Restore default colors").clicked() {
                    preferences.palette = achimeld_application::Preferences::default().palette;
                }
            }
            2 => {
                field_label_preferences(ui, preferences);
                ui.separator();
                ui.heading("Button styles");
                ui.label("Hover to preview the negative fade.");
                ui.label("Body: 0.15 s · Text / icons: 0.2 s");
                crate::buttons::bar(ui, 20.0, |ui| {
                    for (name, style) in [
                        ("Standard", crate::buttons::ButtonStyle::Standard),
                        ("Accent", crate::buttons::ButtonStyle::Accent),
                        ("Quiet", crate::buttons::ButtonStyle::Quiet),
                    ] {
                        ui.add(
                            crate::buttons::Button::new(name)
                                .style(style)
                                .square_corners(),
                        );
                    }
                });
                crate::buttons::bar(ui, 20.0, |ui| {
                    ui.add(crate::buttons::Button::selectable(true, "Selected tab").bar());
                    ui.add_enabled(false, crate::buttons::Button::new("Disabled").bar());
                    crate::icons::header_button(
                        ui,
                        ui.id().with("icon-preview"),
                        crate::icons::Icon::Layers,
                        "Icon action",
                        true,
                    );
                    crate::chrome::title_menu(ui, "Menu preview", |ui| {
                        preference_action(ui, "Menu action");
                    });
                });
                ui.label("Bar buttons");
                crate::buttons::bar(ui, 20.0, |ui| {
                    ui.add(
                        crate::buttons::Button::new("Wants to be clicked")
                            .bar()
                            .wants_click(true),
                    );
                    ui.add_enabled(
                        false,
                        crate::buttons::Button::new("Disabled cue")
                            .bar()
                            .wants_click(true),
                    );
                });
                let preview_id = ui.id().with("bar-preview");
                let mut selected =
                    ui.data_mut(|data| data.get_temp::<usize>(preview_id).unwrap_or(0));
                crate::buttons::bar(ui, 20.0, |ui| {
                    for (index, label) in ["First tab", "Second tab"].into_iter().enumerate() {
                        if ui
                            .add(crate::buttons::Button::selectable(selected == index, label).bar())
                            .clicked()
                        {
                            selected = index;
                        }
                    }
                    ui.add_enabled(false, crate::buttons::Button::new("Disabled").bar());
                });
                ui.data_mut(|data| data.insert_temp(preview_id, selected));
                ui.separator();
                crate::fields::row(ui, "Control corner radius", |ui| {
                    ui.add(egui::DragValue::new(&mut preferences.control_radius).range(0..=24));
                });
                for (label, size, min, max) in [
                    (
                        "Heading font size",
                        &mut preferences.header_size,
                        10.0,
                        48.0,
                    ),
                    ("Normal font size", &mut preferences.normal_size, 9.0, 32.0),
                    ("Small font size", &mut preferences.small_size, 8.0, 24.0),
                    (
                        "Inspector spacing",
                        &mut preferences.inspector_spacing,
                        0.0,
                        32.0,
                    ),
                ] {
                    crate::fields::row(ui, label, |ui| {
                        ui.add(egui::DragValue::new(size).range(min..=max));
                    });
                }
            }
            3 => {
                crate::fields::row(ui, "Achievement naming convention", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut preferences.naming_convention)
                            .desired_width(ui.available_width()),
                    );
                });
                match preferences.achievement_api_name(1) {
                    Ok(name) => {
                        ui.label(format!("Example: {name}"));
                    }
                    Err(error) => {
                        ui.colored_label(crate::theme::color(preferences, "Error"), error);
                    }
                }
            }
            _ => {
                ui.label("Steam opens in a separate browser when you choose Read Steam.");
                ui.label("Sign in manually if prompted. Your session is kept for next time.");
                ui.label(port.status());
            }
        }
    }
}

fn field_label_preferences(ui: &mut egui::Ui, preferences: &mut achimeld_application::Preferences) {
    ui.heading("Field labels");
    crate::fields::row(ui, "Minimum label width", |ui| {
        ui.add(
            egui::DragValue::new(&mut preferences.field_label_min_width)
                .range(40.0..=240.0)
                .suffix(" px")
                .speed(1.0),
        );
    });
    crate::fields::row(ui, "Label width budget", |ui| {
        ui.add(
            egui::DragValue::new(&mut preferences.field_label_width_percent)
                .range(5.0..=50.0)
                .suffix("%")
                .speed(1.0),
        );
    });
    crate::fields::checkbox(
        ui,
        "Abbreviate field labels",
        &mut preferences.abbreviate_field_labels,
    );
    ui.weak("Abbreviations and label budgets apply to resizable editors. Fixed forms always show full labels. Labels stay to the left; hover to read the full name.");
    crate::fields::configure(ui.ctx(), preferences);
    let preview_id = ui.id().with("field-label-preview");
    let mut preview = ui
        .data(|data| data.get_temp::<f64>(preview_id))
        .unwrap_or(45.0);
    crate::sliders::value_slider(ui, "Bottom height", &mut preview, 0.0..=100.0, "%", 0);
    ui.data_mut(|data| data.insert_temp(preview_id, preview));
}

fn preference_action(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let height = crate::buttons::bar_height(ui, 24.0);
    crate::buttons::bar_with_height(ui, height, |ui| {
        ui.add_sized(
            [ui.available_width(), height],
            crate::buttons::Button::new(crate::localization::text(ui.ctx(), label))
                .bar()
                .align(egui::Align2::LEFT_CENTER),
        )
    })
    .inner
}
