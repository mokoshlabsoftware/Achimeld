use crate::buttons::Button;
use crate::{Desktop, localization::text};
use achimeld_application::UiRequest;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum HelpPage {
    About,
    Changes,
}

impl Desktop {
    pub fn open_about(&mut self) {
        self.about = true;
        self.help_page = HelpPage::About;
    }

    pub fn open_change_log(&mut self) {
        self.about = true;
        self.help_page = HelpPage::Changes;
    }

    pub(crate) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about {
            return;
        }
        let response = egui::Modal::new(egui::Id::new("about-dialog"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .fill(egui::Color32::BLACK)
                    .inner_margin(0)
                    .corner_radius(0),
            )
            .show(ctx, |ui| {
                let width = 468.0_f32.min((ctx.content_rect().width() - 32.0).max(0.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let title = text(ctx, if self.help_page == HelpPage::About { "About" } else { "Change log" });
                let title = ui.painter().layout_no_wrap(
                    title,
                    egui::TextStyle::Heading.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
                let header_height = title.size().y.max(24.0) + 24.0;
                let (header, _) = ui.allocate_exact_size(egui::vec2(width, header_height), egui::Sense::hover());
                let icon_id = egui::Id::new("help-brand-icon");
                let icon = ctx.data(|data| data.get_temp::<egui::TextureHandle>(icon_id)).unwrap_or_else(|| {
                    let icon = ctx.load_texture("help-brand", crate::chrome::brand_image(), egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)));
                    ctx.data_mut(|data| data.insert_temp(icon_id, icon.clone()));
                    icon
                });
                egui::Image::new((icon.id(), egui::Vec2::splat(24.0)))
                    .tint(ui.visuals().text_color())
                    .paint_at(ui, egui::Rect::from_center_size(egui::pos2(header.left() + 36.0, header.center().y), egui::Vec2::splat(24.0)));
                ui.painter().galley(header.center() - title.size() / 2.0, title, ui.visuals().text_color());
                divider(ui);
                let action_height = crate::buttons::bar_height(ui, 24.0);
                let body_height = (ctx.content_rect().height() - header_height - action_height - 76.0).clamp(40.0, 400.0);
                egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 20)).show(ui, |ui| {
                    ui.set_width((width - 48.0).max(0.0));
                    ui.spacing_mut().item_spacing.y = 8.0;
                    egui::ScrollArea::vertical().id_salt(("help-body", self.help_page == HelpPage::Changes)).max_height(body_height).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.heading("Achimeld");
                            centered(ui, format!("{} {}", text(ctx, "Version"), crate::change_log::current_version()));
                            ui.add_space(4.0);
                            if self.help_page == HelpPage::About {
                                centered(ui, text(ctx, "Abt000"));
                                for paragraph in [
                                    "Create and edit achievement and stat definitions, design icons, and review imports or draft uploads for your own Steamworks Partner account. Publish manually in Steamworks.",
                                    "Abt002",
                                    "Steam sign-in stays in the dedicated browser profile. Browser automation can access that authenticated session.",
                                    "Abt004",
                                ] {
                                    ui.add_space(8.0);
                                    centered(ui, text(ctx, paragraph));
                                }
                            } else {
                                centered(ui, text(ctx, "Newest changes first. History starts with version tracking."));
                                for entry in crate::change_log::ENTRIES.iter() {
                                    ui.add_space(12.0);
                                    ui.heading(entry.version);
                                    centered(ui, entry.date);
                                    for change in &entry.changes {
                                        centered(ui, *change);
                                    }
                                }
                            }
                        });
                    });
                });
                divider(ui);
                let labels = [
                    text(ctx, if self.help_page == HelpPage::About { "Change log" } else { "About" }),
                    text(ctx, "Close"),
                ];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let natural = labels.each_ref().map(|label| ui.painter().layout_no_wrap(label.clone(), font.clone(), ui.visuals().text_color()).size().x + 2.0 * ui.spacing().button_padding.x);
                let first_width = (width * natural[0] / (natural[0] + natural[1]) * ctx.pixels_per_point()).round() / ctx.pixels_per_point();
                ui.allocate_ui_with_layout(egui::vec2(width, action_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if ui.add_sized([first_width, action_height], Button::new(labels[0].clone()).bar()).clicked() {
                        self.help_page = if self.help_page == HelpPage::About { HelpPage::Changes } else { HelpPage::About };
                    }
                    ui.add_sized([width - first_width, action_height], Button::new(labels[1].clone()).bar()).clicked()
                }).inner
            });
        let keyboard_close = response.is_top_modal
            && !response.any_popup_open
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            });
        if response.inner || keyboard_close {
            self.about = false;
            self.request(UiRequest::AboutDismissed);
        }
    }
}

fn centered(ui: &mut egui::Ui, message: impl Into<egui::WidgetText>) {
    ui.add(egui::Label::new(message).halign(egui::Align::Center).wrap());
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}
