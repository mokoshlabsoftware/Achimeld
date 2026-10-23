use super::*;

pub(super) fn tile_id(id: &str) -> egui::Id {
    egui::Id::new(("gradient-preset-tile", id))
}

fn tile(ui: &mut egui::Ui, preset: &GradientPreset, selected: bool, size: Vec2) -> egui::Response {
    let response = ui.add_sized(
        size,
        Button::selectable(selected, "")
            .bar()
            .id(tile_id(&preset.id)),
    );
    if ui.is_rect_visible(response.rect) {
        let (_, ink) = buttons::colors(
            ui,
            &response,
            if selected {
                ButtonStyle::Accent
            } else {
                ButtonStyle::Quiet
            },
        );
        let content = response.rect.shrink(8.0);
        let line_height = ui.text_style_height(&egui::TextStyle::Button);
        let ramp = Rect::from_min_max(
            content.min,
            egui::pos2(content.right(), content.bottom() - line_height * 2.0 - 6.0),
        );
        let stops = json!(preset.stops);
        crate::gradient::strip(ui, ramp, stops.as_array().map_or(&[], Vec::as_slice));
        let label =
            Rect::from_min_max(egui::pos2(content.left(), ramp.bottom() + 6.0), content.max);
        let galley = ui.painter().layout(
            preset.name.clone(),
            egui::TextStyle::Button.resolve(ui.style()),
            ink,
            label.width(),
        );
        let position = egui::pos2(label.center().x - galley.size().x * 0.5, label.top());
        ui.painter()
            .with_clip_rect(label)
            .galley(position, galley, ink);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            &preset.name,
        )
    });
    response.on_hover_text(&preset.name)
}

impl Explorer {
    pub(super) fn choose(
        &mut self,
        port: &mut dyn EditorPort,
        preset: GradientPreset,
        shift: bool,
        alt: bool,
    ) {
        if !self.save(port) {
            return;
        }
        if alt {
            self.selection.remove(&preset.id);
            if self.selected.as_deref() == Some(&preset.id) {
                self.draft = port
                    .gradients()
                    .iter()
                    .find(|g| self.selection.contains(&g.id))
                    .cloned();
                self.selected = self.draft.as_ref().map(|g| g.id.clone());
            }
        } else {
            if !shift {
                self.selection.clear();
            }
            self.selection.insert(preset.id.clone());
            self.selected = Some(preset.id.clone());
            self.draft = Some(preset);
        }
    }

    pub(super) fn delete_selection(&mut self, port: &mut dyn EditorPort) {
        let originals: Vec<_> = port
            .gradients()
            .iter()
            .filter(|g| self.selection.contains(&g.id))
            .cloned()
            .collect();
        let ids: Vec<_> = self.selection.iter().cloned().collect();
        if !originals.is_empty()
            && let Err(error) = port.delete_gradients(&ids)
        {
            self.error = Some(error);
            return;
        }
        if !originals.is_empty() {
            self.removed = originals;
        }
        self.selection.clear();
        self.selected = None;
        self.draft = None;
        self.error = None;
    }

    pub(super) fn undo_delete(&mut self, port: &mut dyn EditorPort) {
        if let Err(error) = port.restore_gradients(self.removed.clone()) {
            self.error = Some(error);
            return;
        }
        self.selection = self.removed.iter().map(|g| g.id.clone()).collect();
        self.draft = self.removed.first().cloned();
        self.selected = self.draft.as_ref().map(|g| g.id.clone());
        self.removed.clear();
        self.error = None;
    }

    pub(super) fn tiles(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let needle = self.filter.to_lowercase();
        let presets: Vec<_> = port
            .gradients()
            .iter()
            .filter(|g| g.name.to_lowercase().contains(&needle))
            .collect();
        let order: Vec<_> = presets.iter().map(|g| g.id.clone()).collect();
        self.selection.retain(|id| {
            order.contains(id)
                || self
                    .draft
                    .as_ref()
                    .is_some_and(|g| &g.id == id && !port.gradients().iter().any(|p| &p.id == id))
        });
        let width = (ui.available_width() - 16.0).max(1.0);
        let minimum = (ui.text_style_height(&egui::TextStyle::Button) * 8.0).max(140.0);
        let columns = (width / minimum).floor().max(1.0) as usize;
        let size = Vec2::new(
            width / columns as f32,
            80.0 + ui.text_style_height(&egui::TextStyle::Button) * 2.0,
        );
        let mut choose = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            egui::ScrollArea::vertical()
                .id_salt("gradient-library")
                .max_height(size.y * 2.0)
                .auto_shrink([false, false])
                .show_rows(ui, size.y, presets.len().div_ceil(columns), |ui, rows| {
                    for row in rows {
                        ui.horizontal(|ui| {
                            for preset in presets.iter().skip(row * columns).take(columns) {
                                let response =
                                    tile(ui, preset, self.selection.contains(&preset.id), size);
                                if response.clicked() {
                                    response.request_focus();
                                    choose = Some(((*preset).clone(), ui.input(|i| i.modifiers)));
                                }
                            }
                        });
                    }
                });
        });
        if presets.is_empty() {
            ui.weak(if port.gradients().is_empty() {
                "Create a gradient or import a Photoshop .grd set."
            } else {
                "No gradients match this search."
            });
        }
        if let Some((preset, modifiers)) = choose {
            self.choose(port, preset, modifiers.shift, modifiers.alt);
        }
        let focused = ui.ctx().memory(|m| m.focused());
        if ui.is_enabled()
            && !self.selection.is_empty()
            && order.iter().any(|id| focused == Some(tile_id(id)))
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
        {
            self.delete_selection(port);
        }
    }
}
