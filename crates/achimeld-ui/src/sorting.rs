//! Achiever's grid controls, presented with the shared Achimeld buttons.
use crate::{Desktop, buttons, icons, localization};
use achimeld_application::{Command, EditorPort, sorting::align_nodes};
use egui::{TextureHandle, TextureId, Vec2};

fn texture(ctx: &egui::Context, name: &'static str) -> TextureId {
    let id = egui::Id::new(("sort-icon", name));
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture.id();
    }
    let bytes: &[u8] = match name {
        "sort" => include_bytes!("../assets/toolbar/sort.rgba"),
        "style" => include_bytes!("../assets/toolbar/sort-style.rgba"),
        "new-line" => include_bytes!("../assets/toolbar/sort-new-line.rgba"),
        "rows" => include_bytes!("../assets/toolbar/sort-rows.rgba"),
        "selection" => include_bytes!("../assets/toolbar/sort-selection.rgba"),
        "helpers" => include_bytes!("../assets/toolbar/sort-helpers.rgba"),
        _ => unreachable!("closed sort icon set"),
    };
    let texture = ctx.load_texture(
        format!("sort-{name}"),
        egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    );
    let texture_id = texture.id();
    ctx.data_mut(|data| data.insert_temp(id, texture));
    texture_id
}

fn option(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    icon: Option<(&'static str, f32)>,
    enabled: bool,
) -> bool {
    let id = egui::Id::new(("sort-option", icon.map_or("id", |(name, _)| name)));
    let label = localization::text(ui.ctx(), label);
    let button = if let Some((name, angle)) = icon {
        buttons::Button::new(
            egui::Image::new((texture(ui.ctx(), name), Vec2::splat(22.0)))
                .rotate(angle, Vec2::splat(0.5))
                .alt_text(&label),
        )
        .image_tint_follows_text_color(true)
    } else {
        buttons::Button::new(egui::RichText::new("ID").strong())
    };
    let response = ui.add_enabled(
        enabled,
        button
            .id(id)
            .selected(selected)
            .bar()
            .min_size(egui::vec2(40.0, 0.0)),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
    response.on_hover_text(label).clicked()
}

impl Desktop {
    pub(crate) fn sort_button(&mut self, ui: &mut egui::Ui, port: &dyn EditorPort) {
        let label = localization::text(ui.ctx(), "Sort");
        let response = ui.add_enabled(
            !port.synchronizing(),
            buttons::Button::new(
                egui::Image::new((texture(ui.ctx(), "sort"), Vec2::splat(24.0))).alt_text(&label),
            )
            .id(egui::Id::new("sort-toolbar"))
            .selected(self.sort_open)
            .bar()
            .image_tint_follows_text_color(true),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
        });
        if response.on_hover_text(label).clicked() {
            self.sort_open = true;
            ui.ctx().move_to_top(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("node-grid-settings"),
            ));
        }
    }

    pub(crate) fn align_nodes(&mut self, port: &mut dyn EditorPort) {
        if !self.view.is_editor() || port.synchronizing() {
            return;
        }
        if !self.entity_drafts.is_empty() || !self.localization_drafts.is_empty() {
            self.error =
                Some("Finish or revert the highlighted invalid edits before sorting.".into());
            return;
        }
        let mut styles = port.icon_styles();
        styles.extend_from_slice(port.global_icon_styles());
        let commands = align_nodes(
            port.editor().document(),
            self.kind,
            port.preferences(),
            &styles,
            &self.selection,
        );
        if !commands.is_empty() {
            port.finish_group();
            self.node_drag = None;
            self.apply(port, "Align nodes to grid", Command::Batch(commands), None);
        }
    }

    pub(crate) fn sort_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        if !self.sort_open || !self.view.is_editor() {
            return;
        }
        let mut preferences = port.preferences().clone();
        let mut align = false;
        egui::Window::new(localization::text(ctx, "WkGrid"))
            .id(egui::Id::new("node-grid-settings"))
            .order(egui::Order::Foreground)
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fade_in(false)
            .default_width(300.0)
            .default_pos(ctx.content_rect().center() - egui::vec2(150.0, 120.0))
            .default_pos(ctx.content_rect().center() - egui::vec2(150.0, 120.0))
            .show(ctx, |ui| {
                crate::fields::fixed_width(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(localization::text(ctx, "WkGrid"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icons::close_button(ui, egui::Id::new("sort-close"), "Close")
                                .clicked()
                            {
                                self.sort_open = false;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.scope(|ui| {
                        let columns = preferences.fill_nodes_by_columns;
                        for (key, count) in [
                            (
                                if columns { "WkAcCl" } else { "WkAcRw" },
                                &mut preferences.achievement_nodes_per_row,
                            ),
                            (
                                if columns { "WkStCl" } else { "WkStRw" },
                                &mut preferences.stat_nodes_per_row,
                            ),
                        ] {
                            crate::fields::row(ui, key, |ui| {
                                ui.add(egui::DragValue::new(count).range(1..=10).speed(0.1));
                            });
                        }
                    });
                    ui.add_space(10.0);
                    buttons::bar(ui, 22.0, |ui| {
                        if option(
                            ui,
                            "WkSoId",
                            !preferences.sort_nodes_by_icon_style,
                            None,
                            true,
                        ) {
                            preferences.sort_nodes_by_icon_style = false;
                        }
                        if option(
                            ui,
                            "WkSoSt",
                            preferences.sort_nodes_by_icon_style,
                            Some(("style", 0.0)),
                            true,
                        ) {
                            preferences.sort_nodes_by_icon_style = true;
                        }
                        if option(
                            ui,
                            "WkNeLn",
                            preferences.new_line_per_icon_style,
                            Some(("new-line", 0.0)),
                            preferences.sort_nodes_by_icon_style,
                        ) {
                            preferences.new_line_per_icon_style =
                                !preferences.new_line_per_icon_style;
                        }
                        let columns = preferences.fill_nodes_by_columns;
                        if option(
                            ui,
                            if columns { "WkCols" } else { "WkRows" },
                            columns,
                            Some((
                                "rows",
                                if columns {
                                    std::f32::consts::FRAC_PI_2
                                } else {
                                    0.0
                                },
                            )),
                            true,
                        ) {
                            preferences.fill_nodes_by_columns = !columns;
                        }
                        let selected = preferences.order_selected_nodes_only;
                        if option(
                            ui,
                            if selected { "WkSelI" } else { "WkAllI" },
                            selected,
                            Some(("selection", 0.0)),
                            true,
                        ) {
                            preferences.order_selected_nodes_only = !selected;
                        }
                        if option(
                            ui,
                            "Include helpers at the end",
                            preferences.include_helpers_in_sorting,
                            Some(("helpers", 0.0)),
                            self.kind == achimeld_application::EntityKind::Achievement,
                        ) {
                            preferences.include_helpers_in_sorting =
                                !preferences.include_helpers_in_sorting;
                        }
                    });
                    ui.add_space(6.0);
                    crate::fields::checkbox(
                        ui,
                        "WkSoAc",
                        &mut preferences.sort_nodes_by_stat_activation,
                    );
                    ui.add_space(6.0);
                    align = ui
                        .add_enabled(
                            !port.synchronizing(),
                            buttons::Button::new(localization::text(ctx, "WkAlgn"))
                                .id(egui::Id::new("sort-align"))
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                        )
                        .clicked();
                })
            });
        // Controls persist immediately; Align remains an explicit action.
        if preferences != *port.preferences()
            && !self.apply(
                port,
                "Node grid settings",
                Command::Preferences(preferences),
                Some("node-grid-settings"),
            )
        {
            return;
        }
        if align {
            self.align_nodes(port);
        }
    }
}
