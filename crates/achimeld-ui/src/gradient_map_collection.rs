//! Host-persisted 2D gradient templates; applying one copies its points into a particle.
use crate::{
    Desktop,
    buttons::{self, Button},
};
use achimeld_application::{
    EditorPort,
    gradient_map::{GradientMap, GradientMapPreset},
};

const OPEN: &str = "open-2d-gradient-collection";

pub(crate) fn request(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(OPEN), true));
}

fn thumbnail(ui: &egui::Ui, preset: &GradientMapPreset) -> Option<egui::Image<'static>> {
    let id = egui::Id::new(("2d-gradient-collection-thumbnail", &preset.id));
    let cached = ui.data(|data| data.get_temp::<(GradientMap, egui::TextureHandle)>(id));
    let texture = if let Some((map, texture)) = cached
        && map == preset.map
    {
        texture
    } else {
        let pixels = preset.map.compile().ok()?.raster(48, 24, &|| false)?;
        let bytes: Vec<u8> = pixels.into_iter().flatten().collect();
        let image = egui::ColorImage::from_rgba_unmultiplied([48, 24], &bytes);
        let texture = ui.ctx().load_texture(
            format!("2D gradient {}", preset.id),
            image,
            egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
        );
        ui.data_mut(|data| data.insert_temp(id, (preset.map.clone(), texture.clone())));
        texture
    };
    Some(egui::Image::new((texture.id(), egui::vec2(72.0, 36.0))))
}

#[derive(Default)]
pub(crate) struct Collection {
    pub open: bool,
    selected: Option<String>,
    draft: Option<GradientMapPreset>,
    error: Option<String>,
    filter: String,
}

impl Collection {
    fn save(&mut self, port: &mut dyn EditorPort) -> bool {
        let Some(draft) = &self.draft else {
            return true;
        };
        if port.gradient_maps().iter().any(|saved| saved == draft) {
            return true;
        }
        if let Err(error) = port.save_gradient_map(draft.clone()) {
            self.error = Some(error);
            false
        } else {
            self.error = None;
            true
        }
    }

    fn new_map(&mut self) {
        let id = achimeld_application::new_id();
        self.selected = Some(id.clone());
        self.draft = Some(GradientMapPreset {
            id,
            name: "New 2D gradient".into(),
            map: GradientMap::default(),
        });
        self.error = None;
    }

    fn contents(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        buttons::bar(ui, 24.0, |ui| {
            if ui.add(Button::new("New").bar()).clicked() && self.save(port) {
                self.new_map();
            }
            if ui
                .add_enabled(self.draft.is_some(), Button::new("Duplicate").bar())
                .clicked()
                && self.save(port)
                && let Some(draft) = &mut self.draft
            {
                draft.id = achimeld_application::new_id();
                draft.name = format!("{} copy", draft.name.chars().take(60).collect::<String>());
                self.selected = Some(draft.id.clone());
            }
            if ui
                .add_enabled(self.selected.is_some(), Button::new("Delete").bar())
                .clicked()
                && let Some(id) = &self.selected
            {
                match port.delete_gradient_map(id) {
                    Ok(()) => {
                        self.selected = None;
                        self.draft = None;
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Find 2D gradients")
                .desired_width(ui.available_width()),
        );
        let matches: Vec<_> = port
            .gradient_maps()
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&self.filter.to_lowercase()))
            .cloned()
            .collect();
        egui::ScrollArea::vertical()
            .id_salt("2d-gradient-collection-list")
            .max_height(160.0)
            .show_rows(ui, 48.0, matches.len(), |ui, rows| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for preset in matches.iter().skip(rows.start).take(rows.len()) {
                    let selected = self.selected.as_deref() == Some(&preset.id);
                    let button = if let Some(image) = thumbnail(ui, preset) {
                        Button::selectable(selected, (image, preset.name.as_str()))
                    } else {
                        Button::selectable(selected, preset.name.as_str())
                    };
                    if ui
                        .add(
                            button
                                .bar()
                                .min_size(egui::vec2(ui.available_width(), 48.0)),
                        )
                        .clicked()
                        && self.save(port)
                    {
                        self.selected = Some(preset.id.clone());
                        self.draft = Some(preset.clone());
                    }
                }
            });
        if let Some(draft) = &mut self.draft {
            crate::inspector::field(ui, "Name", 90.0, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .char_limit(256)
                        .desired_width(ui.available_width()),
                );
            });
            ui.push_id(&draft.id, |ui| {
                crate::gradient_map::editor(ui, &mut draft.map)
            });
            if ui.add(Button::new("Save 2D gradient")).clicked() {
                self.save(port);
            }
        } else {
            ui.weak("Create a 2D gradient to add it to this collection.");
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.weak("Choose saved maps in a particle layer's gradient picker.");
    }
}

impl Desktop {
    pub(crate) fn gradient_map_collection_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        if ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new(OPEN)))
            .unwrap_or(false)
        {
            self.gradient_map_collection.open = true;
        }
        if !self.gradient_map_collection.open {
            return;
        }
        let mut collection = std::mem::take(&mut self.gradient_map_collection);
        if collection.draft.is_none() && collection.selected.is_none() {
            if let Some(preset) = port.gradient_maps().first() {
                collection.selected = Some(preset.id.clone());
                collection.draft = Some(preset.clone());
            } else {
                collection.new_map();
            }
        }
        let mut open = true;
        egui::Window::new("2D Gradient Collection")
            .id(egui::Id::new("2d-gradient-collection-window"))
            .open(&mut open)
            .default_size([640.0, 620.0])
            .min_width(480.0)
            .collapsible(false)
            .enabled(enabled)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("2d-gradient-collection-body")
                    .show(ui, |ui| collection.contents(ui, port));
            });
        collection.open = open || !collection.save(port);
        self.gradient_map_collection = collection;
    }
}
