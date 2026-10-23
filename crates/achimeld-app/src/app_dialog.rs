//! Themed prompts. Filesystem enumeration stays in the supervised host.
use achimeld_application::UiRequest;
use achimeld_ui::buttons::{Button, ButtonStyle, UiButtonExt as _};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Clone, Copy)]
pub(crate) enum SaveChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone)]
pub(crate) enum ViewerNext {
    #[cfg(test)]
    Load,
    Exit,
    Unload(String),
}

pub(crate) enum Dialog {
    Discard(UiRequest),
    SavedProjectRequired,
    Save(Option<UiRequest>),
    Open,
    ParticleLoad(achimeld_application::particles::PresetTarget),
    ParticleSave(serde_json::Value),
    GradientImport,
    GradientExport(Vec<achimeld_application::GradientPreset>),
    EffectsImport(achimeld_application::effect_presets::EffectKind),
    EffectsExport(
        achimeld_application::effect_presets::EffectKind,
        Vec<achimeld_application::effect_presets::EffectPreset>,
    ),
    Import(bool),
    ReloadArt {
        atlas: bool,
        id: String,
    },
    ImportLayer {
        project_id: String,
        achievement_id: String,
        layer_id: Option<String>,
        general: bool,
    },
    Export,
    ExportTranslations,
    ImportTranslations,
    SettingsFolder,
    ViewerLoad,
    ViewerSave {
        then: Option<ViewerNext>,
        collection: Option<String>,
    },
    ViewerImage,
    ViewerDiscard(ViewerNext),
}
pub(crate) enum DialogResult {
    Discard(UiRequest, SaveChoice),
    Acknowledged,
    Save(Option<UiRequest>, Option<PathBuf>),
    Open(Option<PathBuf>),
    ParticleLoad(
        achimeld_application::particles::PresetTarget,
        Option<PathBuf>,
    ),
    ParticleSave(serde_json::Value, Option<PathBuf>),
    GradientImport(Option<PathBuf>),
    GradientExport(Vec<achimeld_application::GradientPreset>, Option<PathBuf>),
    EffectsImport(
        achimeld_application::effect_presets::EffectKind,
        Option<PathBuf>,
    ),
    EffectsExport(
        achimeld_application::effect_presets::EffectKind,
        Vec<achimeld_application::effect_presets::EffectPreset>,
        Option<PathBuf>,
    ),
    Import(bool, Option<PathBuf>),
    ReloadArt {
        atlas: bool,
        id: String,
        path: Option<PathBuf>,
    },
    ImportLayer {
        project_id: String,
        achievement_id: String,
        layer_id: Option<String>,
        general: bool,
        path: Option<PathBuf>,
    },
    Export(Option<PathBuf>),
    ExportTranslations(Option<PathBuf>),
    ImportTranslations(Option<PathBuf>),
    SettingsFolder(Option<PathBuf>),
    ViewerLoad(Option<PathBuf>),
    ViewerSave {
        path: Option<PathBuf>,
        then: Option<ViewerNext>,
        collection: Option<String>,
    },
    ViewerImage(Option<PathBuf>),
    ViewerDiscard(SaveChoice, ViewerNext),
}
impl Dialog {
    fn title(&self) -> &'static str {
        match self {
            Self::Discard(_) => "Unsaved changes",
            Self::SavedProjectRequired => "Saved project required",
            Self::Save(_) => "Save project",
            Self::Open => "Open project",
            Self::ParticleLoad(_) => "Load particle effect",
            Self::ParticleSave(_) => "Save particle effect",
            Self::GradientImport => "Import Photoshop gradients",
            Self::EffectsImport(_) => "Import effect collection",
            Self::EffectsExport(..) => "Export effect collection",
            Self::GradientExport(_) => "Export Photoshop gradients",
            Self::Import(_) | Self::ImportLayer { .. } => "Import image",
            Self::ReloadArt { .. } => "Reload artwork from file",
            Self::Export => "Choose export folder",
            Self::ExportTranslations => "Export achievement translations",
            Self::ImportTranslations => "Import achievement translations",
            Self::SettingsFolder => "Choose Achimeld folder",
            Self::ViewerLoad => "Load style collection",
            Self::ViewerSave { .. } => "Save style collection",
            Self::ViewerImage => "Add Style Viewer image",
            Self::ViewerDiscard(ViewerNext::Unload(_)) => "Unsaved style collection",
            Self::ViewerDiscard(_) => "Unsaved style libraries",
        }
    }
    fn folders_only(&self) -> bool {
        matches!(self, Self::Export | Self::SettingsFolder)
    }
    fn allows(&self, path: &Path) -> bool {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match self {
            Self::GradientImport | Self::GradientExport(_) => extension.eq_ignore_ascii_case("grd"),
            Self::EffectsImport(kind) | Self::EffectsExport(kind, _) => {
                extension.eq_ignore_ascii_case(kind.extension())
            }
            Self::ParticleLoad(_) | Self::ParticleSave(_) => {
                extension.eq_ignore_ascii_case("achparticles")
            }
            Self::Save(_) | Self::Open | Self::ViewerLoad | Self::ViewerSave { .. } => {
                extension.eq_ignore_ascii_case("achv")
            }
            Self::ExportTranslations | Self::ImportTranslations => {
                extension.eq_ignore_ascii_case("csv")
            }
            Self::Import(_)
            | Self::ImportLayer { .. }
            | Self::ReloadArt { .. }
            | Self::ViewerImage => ["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff"]
                .iter()
                .any(|e| extension.eq_ignore_ascii_case(e)),
            _ => false,
        }
    }
    fn finish(self, path: Option<PathBuf>, choice: SaveChoice) -> DialogResult {
        match self {
            Self::Discard(request) => DialogResult::Discard(request, choice),
            Self::SavedProjectRequired => DialogResult::Acknowledged,
            Self::Save(then) => DialogResult::Save(then, path),
            Self::Open => DialogResult::Open(path),
            Self::ParticleLoad(target) => DialogResult::ParticleLoad(target, path),
            Self::ParticleSave(settings) => DialogResult::ParticleSave(settings, path),
            Self::GradientImport => DialogResult::GradientImport(path),
            Self::GradientExport(gradients) => DialogResult::GradientExport(gradients, path),
            Self::EffectsImport(kind) => DialogResult::EffectsImport(kind, path),
            Self::EffectsExport(kind, presets) => DialogResult::EffectsExport(kind, presets, path),
            Self::Import(atlas) => DialogResult::Import(atlas, path),
            Self::ReloadArt { atlas, id } => DialogResult::ReloadArt { atlas, id, path },
            Self::ImportLayer {
                project_id,
                achievement_id,
                layer_id,
                general,
            } => DialogResult::ImportLayer {
                project_id,
                achievement_id,
                layer_id,
                general,
                path,
            },
            Self::Export => DialogResult::Export(path),
            Self::ExportTranslations => DialogResult::ExportTranslations(path),
            Self::ImportTranslations => DialogResult::ImportTranslations(path),
            Self::SettingsFolder => DialogResult::SettingsFolder(path),
            Self::ViewerLoad => DialogResult::ViewerLoad(path),
            Self::ViewerSave { then, collection } => DialogResult::ViewerSave {
                path,
                then,
                collection,
            },
            Self::ViewerImage => DialogResult::ViewerImage(path),
            Self::ViewerDiscard(then) => DialogResult::ViewerDiscard(choice, then),
        }
    }
}

// The Windows picker only selects a path. The supervised host still validates
// it and performs all project, catalog and artwork I/O after completion.
pub(crate) struct AppDialog {
    kind: Option<Dialog>,
    project: Option<PromptProject>,
    folder: PathBuf,
    picker: rfd::FileDialog,
    pending: Option<mpsc::Receiver<Option<PathBuf>>>,
    error: Option<String>,
}
impl AppDialog {
    pub(crate) fn new(kind: Dialog, folder: PathBuf, picker: rfd::FileDialog) -> Self {
        Self {
            kind: Some(kind),
            project: None,
            folder,
            picker,
            pending: None,
            error: None,
        }
    }
    pub(crate) fn with_project(mut self, name: &str, path: Option<&Path>) -> Self {
        self.project = Some(PromptProject {
            name: if name.trim().is_empty() {
                "Untitled project".into()
            } else {
                name.into()
            },
            path: path.map(Path::to_path_buf),
        });
        self
    }
    fn start_picker(&mut self, kind: &Dialog, ctx: &egui::Context) {
        let mut picker = self
            .picker
            .clone()
            .set_title(kind.title())
            .set_directory(&self.folder);
        let save = matches!(
            kind,
            Dialog::Save(_)
                | Dialog::ExportTranslations
                | Dialog::ViewerSave { .. }
                | Dialog::ParticleSave(_)
                | Dialog::GradientExport(_)
                | Dialog::EffectsExport(..)
        );
        let folders = kind.folders_only();
        picker = match kind {
            Dialog::EffectsImport(kind) | Dialog::EffectsExport(kind, _) => {
                picker.add_filter("Achimeld effect collection", &[kind.extension()])
            }
            Dialog::GradientImport | Dialog::GradientExport(_) => {
                picker.add_filter("Photoshop gradients", &["grd"])
            }
            Dialog::ParticleLoad(_) | Dialog::ParticleSave(_) => {
                picker.add_filter("Achimeld particle effects", &["achparticles"])
            }
            Dialog::Save(_) | Dialog::Open => picker.add_filter("Achimeld projects", &["achv"]),
            Dialog::ViewerLoad | Dialog::ViewerSave { .. } => {
                picker.add_filter("Achimeld style sets", &["achv"])
            }
            Dialog::ExportTranslations | Dialog::ImportTranslations => {
                picker.add_filter("Achievement translations", &["csv"])
            }
            Dialog::Import(_)
            | Dialog::ImportLayer { .. }
            | Dialog::ReloadArt { .. }
            | Dialog::ViewerImage => picker.add_filter(
                "Images",
                &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff"],
            ),
            _ => picker,
        };
        picker = match kind {
            Dialog::EffectsExport(kind, _) => {
                picker.set_file_name(format!("{}.{}", kind.layer_kind(), kind.extension()))
            }
            Dialog::GradientExport(_) => picker.set_file_name("Gradients.grd"),
            Dialog::ParticleSave(_) => picker.set_file_name("Particle effect.achparticles"),
            Dialog::Save(_) => picker.set_file_name("Project.achv"),
            Dialog::ExportTranslations => picker.set_file_name("achievements.translations.csv"),
            _ => picker,
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        // One native picker at a time. Keeping its modal loop off the rendering
        // thread lets the editor continue repainting and checking supervision.
        match std::thread::Builder::new()
            .name("file-picker".into())
            .spawn(move || {
                let selected = if folders {
                    picker.pick_folder()
                } else if save {
                    picker.save_file()
                } else {
                    picker.pick_file()
                };
                let _ = sender.send(selected);
                ctx.request_repaint();
            }) {
            Ok(_) => self.pending = Some(receiver),
            Err(error) => self.error = Some(format!("Could not open the file dialog: {error}")),
        }
    }
    pub(crate) fn show(&mut self, ctx: &egui::Context) -> Option<DialogResult> {
        let kind = self.kind.take()?;
        if !matches!(
            kind,
            Dialog::Discard(_) | Dialog::ViewerDiscard(_) | Dialog::SavedProjectRequired
        ) {
            if let Some(pending) = &self.pending {
                match pending.try_recv() {
                    Ok(None) => return Some(kind.finish(None, SaveChoice::Cancel)),
                    Ok(Some(path)) => {
                        self.pending = None;
                        match validate_target(&kind, path) {
                            Ok(path) => return Some(kind.finish(Some(path), SaveChoice::Save)),
                            Err(error) => self.error = Some(error),
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.pending = None;
                        self.error = Some("The file dialog closed unexpectedly".into());
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            } else if self.error.is_none() {
                self.start_picker(&kind, ctx);
            }
            if self.error.is_none() {
                self.kind = Some(kind);
                return None;
            }
        }
        let mut choice = None;
        let frame = if self.error.is_some() {
            egui::Frame::popup(&ctx.global_style()).inner_margin(24)
        } else {
            egui::Frame::popup(&ctx.global_style())
                .fill(egui::Color32::BLACK)
                .inner_margin(0)
                .corner_radius(0)
        };
        let response = egui::Modal::new(egui::Id::new("app-dialog"))
            .frame(frame)
            .show(ctx, |ui| {
                if let Some(error) = &self.error {
                    ui.set_width(420.0);
                    ui.heading(kind.title());
                    ui.add_space(8.0);
                    ui.label(error);
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.app_button("Cancel").clicked() {
                            choice = Some(SaveChoice::Cancel);
                        }
                        if ui
                            .add(Button::new("Choose again…").style(ButtonStyle::Accent))
                            .clicked()
                        {
                            self.error = None;
                        }
                    });
                } else if matches!(kind, Dialog::SavedProjectRequired) {
                    choice = app_prompt(
                        ui,
                        kind.title(),
                        "You need to save your project before validating the Steamworks destination. Use File > Save, then select Validate again.",
                        None,
                        &[("Close", PromptIcon::Cancel, SaveChoice::Cancel)],
                    );
                } else {
                    choice = unsaved_prompt(ui, kind.title(), self.project.as_ref());
                }
            });
        if choice.is_none() && response.should_close() {
            choice = Some(SaveChoice::Cancel);
        }
        if let Some(choice) = choice {
            Some(kind.finish(None, choice))
        } else {
            self.kind = Some(kind);
            None
        }
    }
}

#[derive(Clone, Copy)]
enum PromptIcon {
    Save,
    Discard,
    Cancel,
}
impl PromptIcon {
    fn image(self, ctx: &egui::Context, size: f32) -> egui::Image<'static> {
        let (name, bytes): (&str, &[u8]) = match self {
            Self::Save => ("save", include_bytes!("../assets/prompts/save.rgba")),
            Self::Discard => ("discard", include_bytes!("../assets/prompts/discard.rgba")),
            Self::Cancel => ("cancel", include_bytes!("../assets/prompts/cancel.rgba")),
        };
        let id = egui::Id::new(("prompt-icon", name));
        let texture = ctx
            .data(|data| data.get_temp::<egui::TextureHandle>(id))
            .unwrap_or_else(|| {
                let texture = ctx.load_texture(
                    format!("prompt-{name}"),
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                );
                ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
                texture
            });
        egui::Image::new((texture.id(), egui::Vec2::splat(size)))
    }
}

fn prompt_divider(ui: &mut egui::Ui) {
    // Reserve exactly one physical pixel, including at fractional display scales.
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0 / ui.ctx().pixels_per_point()),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(
        rect,
        0.0,
        ui.visuals().widgets.noninteractive.bg_stroke.color,
    );
}

struct PromptProject {
    name: String,
    path: Option<PathBuf>,
}

fn unsaved_prompt(
    ui: &mut egui::Ui,
    title: &str,
    project: Option<&PromptProject>,
) -> Option<SaveChoice> {
    let message = if title == "Unsaved style collection" {
        "Save this collection before unloading it?"
    } else if title == "Unsaved style libraries" {
        "Save changes to your style libraries before closing?"
    } else {
        "Save this project before continuing?"
    };
    let actions = [
        ("Save", PromptIcon::Save, SaveChoice::Save),
        ("Discard changes", PromptIcon::Discard, SaveChoice::Discard),
        ("Cancel", PromptIcon::Cancel, SaveChoice::Cancel),
    ];
    app_prompt(ui, title, message, project, &actions)
}

fn app_prompt(
    ui: &mut egui::Ui,
    title: &str,
    message: &str,
    project: Option<&PromptProject>,
    actions: &[(&str, PromptIcon, SaveChoice)],
) -> Option<SaveChoice> {
    const ICON: f32 = 20.0;
    const GAP: f32 = 8.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let natural_widths: Vec<_> = actions
        .iter()
        .map(|(label, _, _)| {
            ui.painter()
                .layout_no_wrap((*label).into(), font.clone(), ui.visuals().text_color())
                .size()
                .x
                + ICON
                + GAP
                + 2.0 * ui.spacing().button_padding.x
        })
        .collect();
    let natural_total: f32 = natural_widths.iter().sum();
    let width = 468.0_f32
        .max(natural_total)
        .min((ui.ctx().content_rect().width() - 32.0).max(0.0));
    ui.set_width(width);
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    let title = ui.painter().layout_no_wrap(
        title.into(),
        egui::TextStyle::Heading.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    let header_height = title.size().y.max(24.0) + 24.0;
    let (header, _) =
        ui.allocate_exact_size(egui::vec2(width, header_height), egui::Sense::hover());
    PromptIcon::Save
        .image(ui.ctx(), 24.0)
        .tint(ui.visuals().text_color())
        .paint_at(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(header.left() + 24.0 + 12.0, header.center().y),
                egui::Vec2::splat(24.0),
            ),
        );
    ui.painter().galley(
        header.center() - title.size() / 2.0,
        title,
        ui.visuals().text_color(),
    );
    prompt_divider(ui);
    let height = achimeld_ui::buttons::bar_height(ui, ICON);
    let body_height = (ui.ctx().content_rect().height() - header_height - height - 74.0).max(0.0);
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(24, 20))
        .show(ui, |ui| {
            ui.set_width((width - 48.0).max(0.0));
            egui::ScrollArea::vertical()
                .max_height(body_height)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(message).wrap().halign(egui::Align::Center));
                        if let Some(project) = project {
                            ui.add_space(12.0);
                            ui.add(
                                egui::Label::new(egui::RichText::new(&project.name).family(
                                    egui::FontFamily::Name(achimeld_ui::BOLD_FAMILY.into()),
                                ))
                                .wrap()
                                .halign(egui::Align::Center),
                            );
                            ui.add_space(4.0);
                            let path = project.path.as_ref().map_or_else(
                                || "Not saved yet".into(),
                                |path| path.display().to_string(),
                            );
                            ui.add(
                                egui::Label::new(egui::RichText::new(path).small().weak())
                                    .wrap()
                                    .halign(egui::Align::Center),
                            );
                        }
                    });
                });
        });
    prompt_divider(ui);

    let mut choice = None;
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let start = ui.cursor().left();
            let mut cumulative = 0.0;
            for (index, (label, icon, answer)) in actions.iter().copied().enumerate() {
                cumulative += natural_widths[index];
                // Round shared boundaries, then give the last button the remainder.
                let right = if index == actions.len() - 1 {
                    start + width
                } else {
                    let pixels_per_point = ui.ctx().pixels_per_point();
                    ((start + width * cumulative / natural_total) * pixels_per_point).round()
                        / pixels_per_point
                };
                let button_width = right - ui.cursor().left();
                if ui
                    .add_sized(
                        [button_width, height],
                        Button::new((icon.image(ui.ctx(), ICON), label))
                            .bar()
                            .icon_spacing(GAP)
                            .image_tint_follows_text_color(true),
                    )
                    .clicked()
                {
                    choice = Some(answer);
                }
            }
        },
    );
    choice
}

fn validate_target(kind: &Dialog, mut path: PathBuf) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Choose an absolute file or folder path".into());
    }
    if kind.folders_only() {
        return if path.is_dir() {
            Ok(path)
        } else {
            Err("Choose an existing folder".into())
        };
    }
    if matches!(kind, Dialog::Save(_) | Dialog::ViewerSave { .. }) && path.extension().is_none() {
        path.set_extension("achv");
    }
    if matches!(kind, Dialog::ExportTranslations) && path.extension().is_none() {
        path.set_extension("csv");
    }
    if matches!(kind, Dialog::ParticleSave(_)) && path.extension().is_none() {
        path.set_extension("achparticles");
    }
    if let Dialog::EffectsExport(kind, _) = kind
        && path.extension().is_none()
    {
        path.set_extension(kind.extension());
    }
    if matches!(kind, Dialog::GradientExport(_)) && path.extension().is_none() {
        path.set_extension("grd");
    }
    if !kind.allows(&path) {
        return Err("Choose a file with the supported extension".into());
    }
    if matches!(
        kind,
        Dialog::Save(_)
            | Dialog::ExportTranslations
            | Dialog::ViewerSave { .. }
            | Dialog::ParticleSave(_)
            | Dialog::GradientExport(_)
            | Dialog::EffectsExport(..)
    ) {
        if !path.parent().is_some_and(Path::is_dir) {
            return Err("Choose an existing destination folder".into());
        }
        if path.exists() && !matches!(kind, Dialog::ParticleSave(_)) {
            return Err(
                "Choose an unused filename. Save the open project with Save to update it.".into(),
            );
        }
        if matches!(kind, Dialog::Save(_) | Dialog::ViewerSave { .. }) {
            let csv = path.with_file_name(format!(
                "{}.achimeld.localization.csv",
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid project filename")?
            ));
            if csv.exists() {
                return Err("The matching localization filename is already in use. Choose another project filename.".into());
            }
        } else if path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".achimeld.localization.csv")
        }) {
            return Err(
                "Choose a separate translation filename, such as achievements.translations.csv"
                    .into(),
            );
        }
    } else if !path.is_file() {
        return Err("Choose an existing file".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_project_prompt_closes_without_starting_a_picker() {
        fn text_rect(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_rect(shape, label)),
                _ => None,
            }
        }
        for scale in [1.0, 1.5] {
            for escape in [false, true] {
                let ctx = egui::Context::default();
                ctx.set_zoom_factor(scale);
                let mut dialog = AppDialog::new(
                    Dialog::SavedProjectRequired,
                    crate::test_output::root(),
                    rfd::FileDialog::new(),
                );
                let mut result = None;
                let mut frame = |events| {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1100.0, 700.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| result = dialog.show(ui.ctx()),
                    );
                    output.textures_delta.clear();
                    output
                };
                let _ = frame(vec![]);
                let output = frame(vec![]);
                assert!(
                    output.shapes.iter().any(|shape| {
                        text_rect(&shape.shape, "Saved project required").is_some()
                    })
                );
                let close = output
                    .shapes
                    .iter()
                    .find_map(|shape| text_rect(&shape.shape, "Close"))
                    .unwrap();
                assert!(ctx.content_rect().contains_rect(close));
                if escape {
                    let _ = frame(vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }]);
                } else {
                    for pressed in [true, false] {
                        let _ = frame(vec![
                            egui::Event::PointerMoved(close.center()),
                            egui::Event::PointerButton {
                                pos: close.center(),
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ]);
                    }
                }
                assert!(matches!(result, Some(DialogResult::Acknowledged)));
                assert!(dialog.pending.is_none());
                assert!(dialog.kind.is_none());
            }
        }
    }

    #[test]
    fn picker_rejects_missing_wrong_extension_and_existing_save_targets() {
        let folder =
            crate::test_output::root().join(format!("dialog-{}", achimeld_application::new_id()));
        std::fs::create_dir(&folder).unwrap();
        let target = |kind: &Dialog, filename: &str| validate_target(kind, folder.join(filename));
        assert_eq!(
            target(&Dialog::Save(None), "Project").unwrap(),
            folder.join("Project.achv")
        );
        assert!(target(&Dialog::Open, "missing.achv").is_err());
        assert!(target(&Dialog::Save(None), "wrong.txt").is_err());
        std::fs::write(folder.join("used.achv"), b"preserve").unwrap();
        assert!(target(&Dialog::Save(None), "used.achv").is_err());
        assert!(target(&Dialog::Open, "used.achv").is_ok());
        std::fs::write(folder.join("unused.achimeld.localization.csv"), b"preserve").unwrap();
        assert!(target(&Dialog::Save(None), "unused.achv").is_err());
        assert_eq!(
            std::fs::read(folder.join("used.achv")).unwrap(),
            b"preserve"
        );
        assert_eq!(
            target(&Dialog::ExportTranslations, "translations").unwrap(),
            folder.join("translations.csv")
        );
        assert!(target(&Dialog::ImportTranslations, "translations").is_err());
        assert!(
            target(
                &Dialog::ExportTranslations,
                "unused.achimeld.localization.csv"
            )
            .is_err()
        );
        assert!(target(&Dialog::ExportTranslations, "new.achimeld.localization.csv").is_err());
        assert!(target(&Dialog::ImportTranslations, "used.achv").is_err());
        std::fs::write(folder.join("translated.csv"), b"preserve").unwrap();
        assert!(target(&Dialog::ImportTranslations, "translated.csv").is_ok());
        assert!(target(&Dialog::ExportTranslations, "translated.csv").is_err());
        assert!(validate_target(&Dialog::Export, folder.clone()).is_ok());
        assert!(target(&Dialog::SettingsFolder, "missing").is_err());
        for name in [
            "used.achv",
            "translated.csv",
            "unused.achimeld.localization.csv",
        ] {
            std::fs::remove_file(folder.join(name)).unwrap();
        }
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn cancelling_native_save_keeps_the_pending_action_cancelled() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut dialog = AppDialog::new(
            Dialog::Save(Some(UiRequest::OpenProject)),
            crate::test_output::root(),
            rfd::FileDialog::new(),
        );
        dialog.pending = Some(receiver);
        sender.send(None).unwrap();
        assert!(matches!(
            dialog.show(&egui::Context::default()),
            Some(DialogResult::Save(Some(UiRequest::OpenProject), None))
        ));
        assert!(dialog.show(&egui::Context::default()).is_none());
    }
}
