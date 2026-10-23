//! Drop routing uses current, clipped UI geometry; the host owns decoding and storage.
use crate::{
    Desktop,
    buttons::{self, Button},
};
use achimeld_application::{EditorPort, ImageImportTarget, UiRequest};
use egui::{Color32, LayerId, Rect, Vec2};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DropZone {
    Canvas,
    Achievement(String),
    General(String),
    ArtBank,
}
#[derive(Default)]
pub(crate) struct ImageDrop {
    pub pointer: Option<egui::Pos2>,
    pub zones: Vec<(LayerId, Rect, DropZone, u8)>,
    pending: Option<Choice>,
    pub selected_revision: Option<u64>,
}
impl ImageDrop {
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
}
struct Choice {
    project: String,
    paths: Vec<PathBuf>,
    zone: DropZone,
    general: bool,
    layer: Option<String>,
    style: Option<String>,
}
pub(crate) fn supported(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp", "psd", "psb",
        ]
        .iter()
        .any(|e| s.eq_ignore_ascii_case(e))
    })
}

fn import_glyph(ctx: &egui::Context, cancel: bool, size: f32) -> egui::Image<'static> {
    let id = egui::Id::new(("drop-prompt-glyph", cancel));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let bytes = if cancel {
                include_bytes!("../assets/toolbar/style-cancel.rgba").as_slice()
            } else {
                include_bytes!("../assets/toolbar/menu-import-images.rgba").as_slice()
            };
            let texture = ctx.load_texture(
                format!("drop-prompt-{cancel}"),
                egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                egui::TextureOptions::LINEAR,
            );
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    egui::Image::new((texture.id(), Vec2::splat(size)))
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        egui::Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, 0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}
impl Desktop {
    /// The Windows OLE drag loop does not deliver ordinary cursor movement.
    /// Only the supervised host samples the native cursor; presentation gets coordinates.
    pub fn file_drop_position(&mut self, position: Option<[f32; 2]>) {
        self.image_drop.pointer = position.map(Into::into);
    }
    pub(crate) fn image_drop_zone(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        zone: DropZone,
        priority: u8,
    ) {
        if ui.is_enabled() {
            self.image_drop.zones.push((
                ui.layer_id(),
                rect.intersect(ui.clip_rect()),
                zone,
                priority,
            ));
        }
    }
    pub(crate) fn dropped_art(&mut self, ctx: &egui::Context, port: &dyn EditorPort) {
        if self.about
            || self.settings
            || self.preferences.is_some()
            || self.source_picker.is_some()
            || self.error.is_some()
            || self.image_drop.pending.is_some()
        {
            return;
        }
        let active = ctx.input(|i| {
            !i.raw.dropped_files.is_empty()
                || i.raw
                    .hovered_files
                    .iter()
                    .any(|f| f.path.as_deref().is_some_and(supported))
        });
        if !active {
            return;
        }
        let pointer = self
            .image_drop
            .pointer
            .or_else(|| ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos())));
        let target = pointer.and_then(|p| {
            let layer = ctx.layer_id_at(p);
            self.image_drop
                .zones
                .iter()
                .filter(|(l, rect, _, _)| Some(*l) == layer && rect.contains(p))
                .max_by_key(|(_, _, _, priority)| priority)
                .cloned()
        });
        if let Some((layer, rect, zone, _)) = &target {
            let phase = ctx.input(|i| i.time as f32 * 4.0).sin() * 0.15 + 0.85;
            ctx.layer_painter(*layer).rect_stroke(
                *rect,
                0.0,
                egui::Stroke::new(
                    2.0,
                    ctx.global_style()
                        .visuals
                        .selection
                        .bg_fill
                        .gamma_multiply(phase),
                ),
                egui::StrokeKind::Inside,
            );
            let text = match zone {
                DropZone::Canvas => "Create achievements from images",
                DropZone::General(_) => "Add to General images",
                DropZone::Achievement(_) => "Import General images or Icon layers",
                DropZone::ArtBank => "Add to Art Data Bank",
            };
            if let Some(p) = pointer {
                ctx.layer_painter(LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("image-drop-feedback"),
                ))
                .text(
                    p + egui::vec2(16.0, 24.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional(14.0),
                    ctx.global_style().visuals.text_color(),
                );
            }
            ctx.request_repaint();
        }
        let paths: Vec<_> = ctx
            .input_mut(|i| std::mem::take(&mut i.raw.dropped_files))
            .into_iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| supported(p))
            .collect();
        if paths.is_empty() {
            return;
        }
        if paths.len() > 32 {
            self.error = Some("Drop at most 32 artwork files at a time".into());
            return;
        }
        let Some((_, rect, zone, _)) = target else {
            return;
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        let layer = self.selected_layer.clone();
        let style = match &zone {
            DropZone::Achievement(id) | DropZone::General(id)
                if self.selection.first() == Some(id)
                    && self.layer_palette.achievement.as_ref() == Some(id) =>
            {
                self.layer_palette.active_style.clone()
            }
            _ => None,
        };
        match zone {
            DropZone::Canvas => self.request(UiRequest::ImportDroppedImages {
                target: ImageImportTarget::NewAchievements {
                    name: paths[0]
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Achievement")
                        .into(),
                    position: crate::canvas::canvas_position(
                        pointer.expect("A drop target requires a pointer"),
                        rect.min + self.pan,
                        self.zoom,
                    ),
                },
                paths,
            }),
            DropZone::General(achievement_id) => self.request(UiRequest::ImportDroppedImages {
                paths,
                target: ImageImportTarget::General {
                    achievement_id,
                    selected_layer_id: layer,
                    style_id: style,
                },
            }),
            DropZone::ArtBank => self.request(UiRequest::ImportDroppedImages {
                paths,
                target: ImageImportTarget::ArtBank,
            }),
            _ => {
                self.image_drop.pending = Some(Choice {
                    project: port.editor().document().project()["projectId"]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                    paths,
                    zone,
                    general: false,
                    layer,
                    style,
                });
            }
        }
    }
    pub(crate) fn image_drop_dialog(&mut self, ctx: &egui::Context, port: &dyn EditorPort) {
        let Some(mut choice) = self.image_drop.pending.take() else {
            return;
        };
        if port.editor().document().project()["projectId"] != choice.project {
            return;
        }
        let mut accept = false;
        let response = egui::Modal::new(egui::Id::new("image-drop-choice"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .fill(Color32::BLACK)
                    .inner_margin(0)
                    .corner_radius(0),
            )
            .show(ctx, |ui| {
                let font = egui::TextStyle::Button.resolve(ui.style());
                let natural = ["Import", "Cancel"].map(|label| {
                    ui.painter()
                        .layout_no_wrap(label.into(), font.clone(), ui.visuals().text_color())
                        .size()
                        .x
                        + 28.0
                        + 2.0 * ui.spacing().button_padding.x
                });
                let total: f32 = natural.iter().sum();
                let width = 468.0_f32
                    .max(total)
                    .min((ctx.content_rect().width() - 32.0).max(1.0));
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let title = ui.painter().layout_no_wrap(
                    "Import images".into(),
                    egui::TextStyle::Heading.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
                let (header, _) = ui.allocate_exact_size(
                    Vec2::new(width, title.size().y.max(24.0) + 24.0),
                    egui::Sense::hover(),
                );
                import_glyph(ctx, false, 24.0)
                    .tint(ui.visuals().text_color())
                    .paint_at(
                        ui,
                        Rect::from_center_size(
                            egui::pos2(header.left() + 36.0, header.center().y),
                            Vec2::splat(24.0),
                        ),
                    );
                ui.painter().galley(
                    header.center() - title.size() * 0.5,
                    title,
                    ui.visuals().text_color(),
                );
                divider(ui);
                let height = buttons::bar_height(ui, 20.0);
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 20))
                    .show(ui, |ui| {
                        ui.set_width((width - 48.0).max(1.0));
                        egui::ScrollArea::vertical()
                            .max_height(
                                (ctx.content_rect().height() - header.height() - height - 76.0)
                                    .max(40.0),
                            )
                            .show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.add(
                                        egui::Label::new(format!(
                                            "{} image(s)\nWhere should the artwork be imported?",
                                            choice.paths.len()
                                        ))
                                        .halign(egui::Align::Center),
                                    );
                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        let half = ui.available_width() * 0.5;
                                        for (label, general) in
                                            [("General images", true), ("Icon layers", false)]
                                        {
                                            if ui
                                                .add_sized(
                                                    [half, height],
                                                    Button::selectable(
                                                        choice.general == general,
                                                        label,
                                                    )
                                                    .bar(),
                                                )
                                                .clicked()
                                            {
                                                choice.general = general;
                                            }
                                        }
                                    });
                                });
                            });
                    });
                divider(ui);
                ui.allocate_ui_with_layout(
                    Vec2::new(width, height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let start = ui.cursor().left();
                        let split = ((start + width * natural[0] / total) * ctx.pixels_per_point())
                            .round()
                            / ctx.pixels_per_point();
                        for (index, label) in ["Import", "Cancel"].into_iter().enumerate() {
                            let right = if index == 0 { split } else { start + width };
                            if ui
                                .add_sized(
                                    [right - ui.cursor().left(), height],
                                    Button::new((import_glyph(ctx, index == 1, 20.0), label))
                                        .bar()
                                        .icon_spacing(8.0)
                                        .image_tint_follows_text_color(true),
                                )
                                .clicked()
                            {
                                accept = index == 0;
                                ui.close();
                            }
                        }
                    },
                );
            });
        if accept {
            let target = match choice.zone {
                DropZone::Achievement(achievement_id) if choice.general => {
                    ImageImportTarget::General {
                        achievement_id,
                        selected_layer_id: choice.layer,
                        style_id: choice.style,
                    }
                }
                DropZone::Achievement(achievement_id) => ImageImportTarget::Layers {
                    achievement_id,
                    selected_layer_id: choice.layer,
                    style_id: choice.style,
                },
                _ => return,
            };
            self.request(UiRequest::ImportDroppedImages {
                paths: choice.paths,
                target,
            });
        } else if !response.should_close() {
            self.image_drop.pending = Some(choice);
        }
    }
}
