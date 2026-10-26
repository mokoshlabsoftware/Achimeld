use super::*;
use crate::buttons::UiButtonExt as _;

impl Desktop {
    pub(crate) fn open_style_delete(&mut self, port: &dyn EditorPort, style: &str) {
        self.style_viewer_ui.deletion = Some(DeleteStyle {
            project: port.editor().document().project()["projectId"]
                .as_str()
                .unwrap_or("")
                .into(),
            style: style.into(),
            replacement: None,
        });
    }

    pub(crate) fn style_delete_window(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let Some(mut draft) = self.style_viewer_ui.deletion.take() else {
            return;
        };
        if port.editor().document().project()["projectId"].as_str() != Some(&draft.project) {
            return;
        }
        let styles = port.icon_styles();
        let Some(style) = styles.iter().find(|s| s.id == draft.style) else {
            return;
        };
        let count = port
            .editor()
            .document()
            .entities(achimeld_application::EntityKind::Achievement)
            .iter()
            .filter(|e| achimeld_application::style_management::uses_style(e, style))
            .count();
        let mut approve = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("delete-project-style"))
            .frame(egui::Frame::popup(&ctx.global_style()).fill(egui::Color32::BLACK).inner_margin(0).corner_radius(0))
            .show(ctx, |ui| {
                let width = 520.0_f32.min((ctx.content_rect().width()-32.0).max(1.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let font = egui::TextStyle::Heading.resolve(ui.style());
                let title = ui.painter().layout_no_wrap("Delete project style".into(), font, ui.visuals().text_color());
                let (header, _) = ui.allocate_exact_size(egui::vec2(width, title.size().y.max(24.0)+24.0), egui::Sense::hover());
                icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(header.left()+36.0,header.center().y),egui::Vec2::splat(24.0)), icons::Icon::Delete, ui.visuals().text_color());
                ui.painter().galley(header.center()-title.size()*0.5,title,ui.visuals().text_color());
                divider(ui);
                egui::Frame::new().inner_margin(egui::Margin::symmetric(24,20)).show(ui, |ui| {
                    crate::fields::fixed_width(ui, |ui| {
                        egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-220.0).max(80.0)).show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(&style.name).strong());
                                ui.add(egui::Label::new(format!("{count} icons use this style. Keep their current appearance and break the link, or choose a replacement style.")).wrap().halign(egui::Align::Center));
                            });
                            ui.add_space(16.0);
                            crate::fields::row(ui,"Using icons",|ui| {
                                let label = draft.replacement.as_ref().and_then(|id| styles.iter().find(|s| &s.id == id)).map_or("Keep appearance · break link",|s| s.name.as_str());
                                crate::dropdowns::Dropdown::from_id_salt("delete-style-replacement").selected_text(label).show_ui(ui,|ui| {
                                    ui.app_selectable_value(&mut draft.replacement,None,"Keep appearance · break link");
                                    for other in styles.iter().filter(|s| s.id != style.id) {
                                        ui.app_selectable_value(&mut draft.replacement,Some(other.id.clone()),&other.name);
                                    }
                                });
                            });
                        });
                    });
                });
                divider(ui);
                buttons::bar(ui,20.0,|ui| {
                    let height = ui.spacing().interact_size.y;
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let natural = ["Delete style", "Cancel"].map(|label| ui.painter().layout_no_wrap(label.into(), font.clone(), ui.visuals().text_color()).size().x + 28.0 + 2.0*ui.spacing().button_padding.x);
                    let total = ui.available_width();
                    let width = (total * natural[0] / (natural[0]+natural[1]) * ui.ctx().pixels_per_point()).round()/ui.ctx().pixels_per_point();
                    approve = ui.add_sized([width,height],buttons::Button::new((glyph(ui.ctx(),"delete-style",include_bytes!("../../assets/toolbar/layer-action-delete.rgba")),"Delete style")).id(egui::Id::new("delete-style-approve")).bar().icon_spacing(8.0).image_tint_follows_text_color(true)).clicked();
                    cancel = ui.add_sized([total-width,height],buttons::Button::new((glyph(ui.ctx(),"cancel-style-delete",include_bytes!("../../assets/toolbar/style-cancel.rgba")),"Cancel")).id(egui::Id::new("delete-style-cancel")).bar().icon_spacing(8.0).image_tint_follows_text_color(true)).clicked();
                });
            });
        if approve {
            match achimeld_application::style_management::remove(
                port.editor().document(),
                &draft.style,
                draft.replacement.as_deref(),
            )
            .and_then(|command| {
                port.finish_group();
                port.execute("Delete project style", command, None)
            }) {
                Ok(()) => {
                    self.selected_layer = None;
                }
                Err(error) => {
                    self.error = Some(error);
                    self.style_viewer_ui.deletion = Some(draft);
                }
            }
        } else if !cancel && !response.should_close() {
            self.style_viewer_ui.deletion = Some(draft);
        }
    }
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}
