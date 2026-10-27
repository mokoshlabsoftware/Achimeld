use crate::buttons::UiButtonExt as _;
use crate::desktop::InspectorTab;
use crate::icons::{self, Icon};
use crate::{Desktop, buttons, theme};
use achimeld_application::{
    Command, EditorPort, EntityKind, LayerLayout, ReorderGesture, UiRequest, new_id,
    style_signature,
};
use egui::{Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn layers_panel_id() -> egui::Id {
    egui::Id::new("icon-layers-panel")
}

const STYLE_EDIT_SECONDS: f32 = 0.2;

#[derive(Clone, Copy, PartialEq)]
enum LayerPane {
    Combined,
    Stack,
    Properties,
}

pub(crate) fn style_edit_fade(ctx: &egui::Context, id: egui::Id, active: bool) -> f32 {
    let initialized = ctx.data_mut(|data| {
        std::mem::replace(
            data.get_temp_mut_or_default::<bool>(id.with("initialized")),
            true,
        )
    });
    if !initialized {
        ctx.animate_bool_with_time(id, false, STYLE_EDIT_SECONDS);
    }
    let progress = ctx.animate_bool_with_time(id, active, STYLE_EDIT_SECONDS);
    if progress != if active { 1.0 } else { 0.0 } {
        ctx.request_repaint();
    }
    progress * progress * (3.0 - 2.0 * progress)
}

pub(crate) fn layer_control_id(achievement: &str, layer: &str, control: &str) -> egui::Id {
    egui::Id::new(("layer-control", achievement, layer, control))
}

pub(crate) struct LayerPalette {
    pub open: bool,
    pub keyboard_active: bool,
    pub pending: Option<LayerAction>,
    pub collapsed: BTreeSet<String>,
    pub expanded_effects: BTreeSet<String>,
    pub effect_drag: Option<crate::layer_effects::EffectDrag>,
    pub clipboard: Option<Vec<Value>>,
    pub clipboard_root: Option<String>,
    pub background: Option<Value>,
    pub transform: Option<Value>,
    pub selected_effect: Option<String>,
    pub achievement: Option<String>,
    pub split: f32,
    pub rows: BTreeMap<String, Rect>,
    pub drag_folder: Option<String>,
    pub edit_label: Option<&'static str>,
    pub layout: LayerLayout,
    pub layout_loaded: bool,
    pub last_layout: LayerLayout,
    // Last visible panel rectangle, in application coordinates.
    pub panel_rect: Option<Rect>,
    pub properties_rect: Option<Rect>,
    sidebar_layout: bool,
    pub header: Option<PaletteHeader>,
    // Panel and pointer origins stay fixed for the duration of a surface drag.
    panel_drag: Option<(Pos2, Pos2)>,
    pub focus_requested: bool,
    pub recover_requested: bool,
    pub artwork_colors: BTreeMap<String, (String, Option<egui::Color32>)>,
    pub active_style: Option<String>,
    pub shape_transform: Option<Value>,
    pub curve_drag: Option<usize>,
    visibility_stroke: Option<VisibilityStroke>,
    visibility_press_frame: Option<u64>,
    row_drag_candidate: Option<String>,
}

pub(crate) struct PaletteHeader {
    pub icon: Rect,
    pub painter: egui::Painter,
    pub connections: egui::layers::ShapeIdx,
}

struct VisibilityStroke {
    visible: bool,
    visited: BTreeSet<String>,
    pointer: Option<Pos2>,
}

impl VisibilityStroke {
    fn paint(&mut self, from: Pos2, to: Pos2, eyes: &[(String, Rect)], layers: &mut [Value]) {
        let direction = (to - from).normalized();
        for (id, rect) in eyes {
            if rect.is_positive()
                && (rect.contains(to)
                    || (from != to
                        && rect.intersects(Rect::from_two_pos(from, to))
                        && rect.intersects_ray(from, direction)
                        && rect.intersects_ray(to, -direction)))
                && self.visited.insert(id.clone())
                && let Some(layer) = layers.iter_mut().find(|layer| layer["id"] == *id)
            {
                layer["visible"] = json!(self.visible);
            }
        }
    }
}
impl Default for LayerPalette {
    fn default() -> Self {
        Self {
            open: true,
            keyboard_active: false,
            pending: None,
            collapsed: BTreeSet::new(),
            expanded_effects: BTreeSet::new(),
            effect_drag: None,
            clipboard: None,
            clipboard_root: None,
            background: None,
            transform: None,
            selected_effect: None,
            achievement: None,
            split: 0.5,
            rows: BTreeMap::new(),
            drag_folder: None,
            edit_label: None,
            layout: Default::default(),
            layout_loaded: false,
            last_layout: Default::default(),
            panel_rect: None,
            properties_rect: None,
            sidebar_layout: false,
            header: None,
            panel_drag: None,
            focus_requested: false,
            recover_requested: false,
            artwork_colors: BTreeMap::new(),
            active_style: None,
            shape_transform: None,
            curve_drag: None,
            visibility_stroke: None,
            visibility_press_frame: None,
            row_drag_candidate: None,
        }
    }
}
impl LayerPalette {
    fn drag_position(&mut self, ctx: &egui::Context, enabled: bool) -> Option<Pos2> {
        if !enabled || ctx.input(|i| !i.focused || i.key_pressed(egui::Key::Escape)) {
            self.panel_drag = None;
            return None;
        }
        let id = layers_panel_id();
        let surfaces = [id.with("header"), id.with("background")];
        if self.panel_drag.is_none()
            && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
            && let Some(origin) = ctx.input(|i| i.pointer.press_origin())
            && let Some(surface) = surfaces.iter().find_map(|id| {
                ctx.read_response(*id).filter(|response| {
                    response.enabled()
                        && response.interact_rect.contains(origin)
                        && response.dragged_by(egui::PointerButton::Primary)
                        && ctx.layer_id_at(origin) == Some(response.layer_id)
                })
            })
        {
            // A click-only button can share a drag hit with its background.
            // Reserve every enabled interactive child, including fields, rows,
            // scrollbars and resize handles, using the actual clipped hit areas.
            let hits = ctx.interaction_snapshot(|snapshot| {
                snapshot
                    .contains_pointer
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
            });
            let control = hits.into_iter().any(|hit| {
                !surfaces.contains(&hit)
                    && hit != id.with("move")
                    && ctx.read_response(hit).is_some_and(|response| {
                        response.layer_id == surface.layer_id
                            && response.enabled()
                            && (response.sense.senses_click() || response.sense.senses_drag())
                            && response.interact_rect.contains(origin)
                    })
            });
            if !control && let Some(rect) = self.panel_rect {
                self.panel_drag = Some((rect.min, origin));
            }
        }
        let release = ctx.input(|i| {
            i.raw.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => Some(*pos),
                _ => None,
            })
        });
        let position = self
            .panel_drag
            .zip(release.or_else(|| ctx.pointer_interact_pos()))
            .map(|((position, origin), pointer)| position + (pointer - origin));
        if release.is_some() || !ctx.input(|i| i.pointer.primary_down()) {
            self.panel_drag = None;
        }
        position
    }

    fn default_rect(&self, bounds: Rect) -> Rect {
        let mut size = Vec2::from(self.layout.size);
        if self.sidebar_layout {
            size.x = (size.x * self.split).max(360.0);
        }
        let size = size.min((bounds.size() - Vec2::splat(48.0)).max(egui::vec2(360.0, 180.0)));
        let position = bounds.min + ((bounds.size() - size) * 0.5).max(Vec2::splat(24.0));
        Rect::from_min_size(position, size)
    }
}
#[derive(Clone)]
pub(crate) enum LayerAction {
    Add(&'static str),
    Delete,
    Duplicate,
    Copy,
    Paste,
    Move(i32),
    MoveInto(Option<String>),
    RemoveFromFolder,
    RemoveMask { owner: String, delete_source: bool },
    AddMask { owner: String, kind: &'static str },
    ReferenceMask { owner: String, source: String },
    InvertMask { owner: String },
    GrayscaleMask { owner: String },
    Reroll,
}

fn random_layer_settings(kind: &str) -> Option<(&'static str, u32, u32)> {
    match kind {
        "Particle" => Some(("particles", u32::MAX, 1)),
        "Explosion" => Some(("explosion", u32::MAX, 1)),
        "PerlinNoise" => Some(("perlinNoise", i32::MAX as u32, 0)),
        _ => None,
    }
}

fn reroll_random_layers(layers: &mut [Value]) {
    // Folders and their masks reference this same flat layer list. Re-roll every
    // random source, including hidden layers, independently of the selection.
    for layer in layers {
        let Some((key, maximum, default)) =
            random_layer_settings(layer["kind"].as_str().unwrap_or(""))
        else {
            continue;
        };
        let previous = layer[key]["seed"].as_u64().unwrap_or(u64::from(default));
        let mut seed = u32::from_str_radix(&new_id()[..8], 16).unwrap_or(1) & maximum;
        if u64::from(seed) == previous {
            seed = seed.wrapping_add(1) & maximum;
        }
        if !layer[key].is_object() {
            layer[key] = json!({});
        }
        layer[key]["seed"] = json!(seed);
    }
}

pub(crate) fn is_adjustment(kind: &str) -> bool {
    crate::layer_icons::ADJUSTMENTS.contains(&kind)
}
pub(crate) fn label(kind: &str) -> &str {
    match kind {
        "AtlasTile" => "Atlas tile",
        "Text3d" => "3D text",
        "Artifact3d" => "3D artifact",
        "StandaloneImage" => "Image",
        "AchievementImage" => "General image",
        "HueSaturation" => "Hue / saturation",
        "SelectiveColor" => "Selective color",
        "UnsharpMask" => "Unsharp mask",
        "GradientOverlay" => "Gradient overlay",
        "ColorOverlay" => "Color overlay",
        "InnerLight" => "Inner light",
        "OuterLight" => "Outer light",
        "BoxBlur" => "Box blur",
        "LensBlur" => "Lens blur",
        "RadialBlur" => "Radial blur",
        "SdfVignette" => "SDF vignette",
        "Particle" => "Particles",
        "Explosion" => "Explosion",
        "PerlinNoise" => "Perlin noise",
        "MaskReference" => "Reference mask",
        other => other,
    }
}
pub(crate) fn effective_icon(entity: &Value) -> Value {
    if entity["icon"].is_object() {
        entity["icon"].clone()
    } else {
        json!({"backgroundColorHex":null,"layers":[{"id":entity["id"],"displayName":"Icon","kind":if entity["standaloneImageId"].is_null(){"AtlasTile"}else{"StandaloneImage"},"visible":true,"opacity":1.0,"atlasId":entity["atlasId"],"imageId":entity["standaloneImageId"],"tileIndex":entity["tileIndex"],"effects":[]}]})
    }
}
pub(crate) fn descendants(layers: &[Value], id: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::from([id.to_owned()]);
    loop {
        let count = ids.len();
        for l in layers {
            if l["parentFolderId"]
                .as_str()
                .is_some_and(|p| ids.contains(p))
                && let Some(id) = l["id"].as_str()
            {
                ids.insert(id.into());
            }
        }
        if ids.len() == count {
            return ids;
        }
    }
}
/// A source must not depend on the owner through folders or other masks.
/// Compute the reverse dependency closure once per frame, rather than cloning
/// and validating the entire project for every item in the mask picker.
pub(crate) fn blocked_mask_sources(layers: &[Value], owner: &str) -> BTreeSet<String> {
    mask_dependencies(layers, owner, true)
}
/// A target must not be a dependency of the source mask's owner.
pub(crate) fn blocked_mask_targets(layers: &[Value], owner: &str) -> BTreeSet<String> {
    mask_dependencies(layers, owner, false)
}
fn mask_dependencies(layers: &[Value], owner: &str, reverse: bool) -> BTreeSet<String> {
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut edge = |from, to| {
        let (from, to) = if reverse { (to, from) } else { (from, to) };
        graph.entry(from).or_default().push(to);
    };
    for layer in layers {
        let Some(id) = layer["id"].as_str() else {
            continue;
        };
        if let Some(mask) = layer["maskLayerId"].as_str() {
            edge(id, mask);
        }
        if layer["kind"] == "MaskReference"
            && let Some(source) = layer["maskSourceLayerId"].as_str()
        {
            edge(id, source);
        }
        if layer["visible"] != false
            && let Some(parent) = layer["parentFolderId"].as_str()
        {
            edge(parent, id);
        }
    }
    let mut blocked = BTreeSet::new();
    let mut pending = vec![owner];
    while let Some(id) = pending.pop() {
        if blocked.insert(id.to_owned())
            && let Some(dependents) = graph.get(id)
        {
            pending.extend(dependents);
        }
    }
    blocked
}
// The painter's order is bottom-to-top. The palette is top-to-bottom, with
// children adjacent to their folder; mask roots have their own nested entry.
pub(crate) fn stack(layers: &[Value], collapsed: &BTreeSet<String>) -> Vec<(String, usize)> {
    fn visit(
        layers: &[Value],
        parent: Option<&str>,
        depth: usize,
        masks: &BTreeSet<&str>,
        collapsed: &BTreeSet<String>,
        out: &mut Vec<(String, usize)>,
    ) {
        if depth > 64 {
            return;
        }
        for layer in layers
            .iter()
            .rev()
            .filter(|l| l["parentFolderId"].as_str() == parent)
        {
            let Some(id) = layer["id"].as_str() else {
                continue;
            };
            if masks.contains(id) || out.iter().any(|(shown, _)| shown == id) {
                continue;
            }
            out.push((id.into(), depth));
            if let Some(mask) = layer["maskLayerId"].as_str()
                && !collapsed.contains(mask)
                && layers
                    .iter()
                    .any(|l| l["id"] == mask && l["kind"] == "Folder")
            {
                visit(layers, Some(mask), depth + 2, masks, collapsed, out);
            }
            if layer["kind"] == "Folder" && !collapsed.contains(id) {
                visit(layers, Some(id), depth + 1, masks, collapsed, out);
            }
        }
    }
    let masks = layers
        .iter()
        .filter_map(|l| l["maskLayerId"].as_str())
        .collect();
    let mut out = Vec::new();
    visit(layers, None, 0, &masks, collapsed, &mut out);
    out
}
pub(crate) fn copy_bundle(layers: &[Value], id: &str) -> Vec<Value> {
    let mut ids = descendants(layers, id);
    loop {
        let count = ids.len();
        for l in layers {
            if l["id"].as_str().is_some_and(|id| ids.contains(id))
                && let Some(mask) = l["maskLayerId"].as_str()
            {
                ids.extend(descendants(layers, mask));
            }
        }
        if count == ids.len() {
            break;
        }
    }
    layers
        .iter()
        .filter(|l| l["id"].as_str().is_some_and(|id| ids.contains(id)))
        .cloned()
        .collect()
}
pub(crate) fn fresh_bundle(
    source: &[Value],
    root: &str,
    parent: Option<&str>,
) -> (Vec<Value>, String) {
    let ids: BTreeMap<_, _> = source
        .iter()
        .filter_map(|l| Some((l["id"].as_str()?.to_owned(), new_id())))
        .collect();
    let mut output = source.to_vec();
    for l in &mut output {
        let original = l["id"].as_str().unwrap_or("").to_owned();
        for key in ["id", "parentFolderId", "maskLayerId", "maskSourceLayerId"] {
            if let Some(id) = l[key].as_str() {
                l[key] = ids.get(id).map_or_else(
                    || {
                        if key == "maskSourceLayerId" {
                            json!(id)
                        } else {
                            Value::Null
                        }
                    },
                    |id| json!(id),
                );
            }
        }
        if original == root {
            l["parentFolderId"] = parent.map_or(Value::Null, |id| json!(id));
        }
        if let Some(effects) = l["effects"].as_array_mut() {
            for effect in effects {
                effect["id"] = json!(new_id());
            }
        }
    }
    (output, ids.get(root).cloned().unwrap_or_default())
}
pub(crate) fn remove_layer(layers: &mut Vec<Value>, id: &str) {
    let mut removed = descendants(layers, id);
    // A folder's private mask belongs to the folder, but shared masks survive.
    let masks: Vec<_> = layers
        .iter()
        .filter(|l| l["id"].as_str().is_some_and(|id| removed.contains(id)))
        .filter_map(|l| l["maskLayerId"].as_str().map(str::to_owned))
        .collect();
    for mask in masks {
        if !layers.iter().any(|l| {
            l["maskLayerId"] == mask && l["id"].as_str().is_some_and(|id| !removed.contains(id))
        }) {
            removed.extend(descendants(layers, &mask));
        }
    }
    layers.retain(|l| !l["id"].as_str().is_some_and(|id| removed.contains(id)));
    for l in layers {
        if l["maskLayerId"]
            .as_str()
            .is_some_and(|id| removed.contains(id))
        {
            l["maskLayerId"] = Value::Null;
        }
        if l["maskSourceLayerId"]
            .as_str()
            .is_some_and(|id| removed.contains(id))
        {
            l["maskSourceLayerId"] = Value::Null;
        }
    }
}

pub(crate) fn retain_mask_references(bundle: &mut [Value], destination: &[Value]) {
    let ids: BTreeSet<_> = destination
        .iter()
        .chain(bundle.iter())
        .filter_map(|layer| layer["id"].as_str().map(str::to_owned))
        .collect();
    for layer in bundle {
        if layer["maskSourceLayerId"]
            .as_str()
            .is_some_and(|id| !ids.contains(id))
        {
            layer["maskSourceLayerId"] = Value::Null;
        }
    }
}

impl Desktop {
    pub(crate) fn open_layers(&mut self) {
        self.layer_palette.open = true;
        self.layer_palette.focus_requested = true;
        self.inspector_tab = InspectorTab::Icon;
    }
    pub(crate) fn recover_layers(&mut self) {
        self.open_layers();
        self.layer_palette.recover_requested = true;
    }
    pub(crate) fn close_layers(&mut self) {
        self.layer_palette.open = false;
        self.layer_palette.effect_drag = None;
        self.layer_palette.keyboard_active = false;
        self.layer_palette.panel_rect = None;
        self.inspector_tab = InspectorTab::General;
    }
    pub(crate) fn layers_available(&self) -> bool {
        if self.view == crate::desktop::View::Style {
            return self.layer_palette.open;
        }
        self.layer_palette.open
            && self.view.is_editor()
            && self.kind == EntityKind::Achievement
            && self.inspector_tab == InspectorTab::Icon
            && !self.selection.is_empty()
    }
    fn load_layer_layout(&mut self, port: &dyn EditorPort) {
        if !self.layer_palette.layout_loaded {
            self.layer_palette.layout = port.layer_layout();
            self.layer_palette.last_layout = self.layer_palette.layout.clone();
            // Retain the stored shape for compatibility with older settings;
            // Layers is a floating panel inside the application.
            self.layer_palette.layout.floating = true;
            self.layer_palette.split = self.layer_palette.layout.split;
            self.layer_palette.layout_loaded = true;
        }
    }
    pub(crate) fn reset_layers_position(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        self.load_layer_layout(port);
        self.layer_palette.layout.position = None;
        self.layer_palette.panel_rect = None;
        self.layer_palette.header = None;
        self.layer_palette.panel_drag = None;
        self.layer_palette.recover_requested = true;
        self.save_palette_layout(ctx, port);
    }
    pub(crate) fn layers_window(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        enabled: bool,
    ) {
        self.load_layer_layout(port);
        if self.inspector_tab == InspectorTab::Icon || self.view == crate::desktop::View::Style {
            self.layer_palette.open = true;
        }
        if !self.layers_available()
            || (self.view == crate::desktop::View::Style
                && port.style_viewer().and_then(|s| s.selected()).is_none())
        {
            self.layer_palette.keyboard_active = false;
            self.layer_palette.panel_rect = None;
            self.layer_palette.header = None;
            self.layer_palette.panel_drag = None;
            self.layer_palette.properties_rect = None;
            return;
        }
        let sidebar = self.view == crate::desktop::View::Style
            || port.preferences().layer_properties_in_sidebar;
        let mode_changed = sidebar != self.layer_palette.sidebar_layout;
        self.layer_palette.sidebar_layout = sidebar;
        let id = layers_panel_id();
        let bounds = ctx.content_rect();
        let remembered = (!mode_changed)
            .then(|| ctx.memory(|memory| memory.area_rect(id)))
            .flatten();
        let saved = self.layer_palette.layout.position.map(|position| {
            let mut size = Vec2::from(self.layer_palette.layout.size);
            if sidebar {
                size.x = (size.x * self.layer_palette.split).max(360.0);
            }
            Rect::from_min_size(position.into(), size)
        });
        let initial = self.layer_palette.panel_rect.is_none() || mode_changed;
        let drag_position = self.layer_palette.drag_position(ctx, enabled);
        // Position before laying out the window so paint and hit testing agree.
        // Absolute pointer displacement also makes repeated layout passes idempotent.
        let rect = if let Some(position) = drag_position {
            Some(Rect::from_min_size(
                position,
                remembered
                    .or(saved)
                    .unwrap_or_else(|| self.layer_palette.default_rect(bounds))
                    .size(),
            ))
        } else if initial {
            saved.or(remembered)
        } else {
            remembered.or(saved)
        };
        let recovering = self.layer_palette.recover_requested
            || rect.is_some_and(|rect| !bounds.intersect(rect).is_positive());
        let target = if recovering {
            self.layer_palette.default_rect(bounds)
        } else {
            rect.unwrap_or_else(|| self.layer_palette.default_rect(bounds))
        };
        let mut window = egui::Window::new("Layers")
            .id(id)
            .order(egui::Order::Foreground)
            .enabled(enabled)
            .movable(false)
            .title_bar(false)
            .collapsible(false)
            .constrain(false)
            .default_rect(target)
            .min_size([360.0, 180.0])
            .max_size(bounds.size().max(egui::vec2(360.0, 180.0)))
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(theme::color(port.preferences(), "Panel"))
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin {
                        top: 0,
                        ..ctx.global_style().spacing.window_margin
                    })
                    .stroke(Stroke::NONE),
            );
        if initial || recovering {
            window = window.current_pos(target.min).fixed_size(target.size());
        } else if drag_position.is_some() {
            window = window.current_pos(target.min);
        }
        self.layer_palette.header = None;
        if let Some(response) = window.show(ctx, |ui| {
            // Passive captions are window surfaces; editable fields keep their own input.
            ui.style_mut().interaction.selectable_labels = false;
            ui.interact(
                ui.max_rect() + ui.spacing().window_margin,
                id.with("background"),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
            self.layer_editor(ui, port);
        }) {
            let rect = response.response.rect;
            if self.view != crate::desktop::View::Style
                && let Some(id) = self.selection.first().cloned()
            {
                let layer = egui::LayerId::new(egui::Order::Foreground, layers_panel_id());
                self.image_drop.zones.push((
                    layer,
                    rect,
                    crate::image_drop::DropZone::Achievement(id),
                    2,
                ));
            }
            self.layer_palette.panel_rect = Some(rect);
            self.layer_palette.layout.position = Some(rect.min.into());
            self.layer_palette.layout.size = rect.size().into();
            if sidebar {
                // Store the combined width so switching back restores the full editor.
                self.layer_palette.layout.size[0] /= self.layer_palette.split;
            }
        }
        self.layer_palette.focus_requested = false;
        self.layer_palette.recover_requested = false;
        self.save_palette_layout(ctx, port);
    }
    fn save_palette_layout(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        self.layer_palette.layout.floating = true;
        self.layer_palette.layout.split = self.layer_palette.split;
        if self.layer_palette.layout != self.layer_palette.last_layout
            && !ctx.input(|i| i.pointer.any_down())
        {
            self.layer_palette.last_layout = self.layer_palette.layout.clone();
            if let Err(error) = port.save_layer_layout(self.layer_palette.layout.clone()) {
                self.error = Some(error);
            }
        }
    }
    fn palette_header(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        achievement: &str,
        icon: &mut Value,
    ) {
        let editing_style = self.layer_palette.active_style.is_some();
        let thumbnail_opacity = 1.0
            - style_edit_fade(
                ui.ctx(),
                layers_panel_id().with(("style-header", achievement)),
                editing_style,
            );
        let scale = ui.ctx().pixels_per_point();
        let line_height =
            ((ui.text_style_height(&egui::TextStyle::Small) * scale).ceil() / scale).max(15.0);
        let header_height = (line_height * 2.0 + 3.0).max(icons::CLOSE_BUTTON_SIZE);
        let header_rect = Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(ui.available_width(), header_height),
        ) + egui::Margin {
            top: 0,
            bottom: 0,
            ..ui.spacing().window_margin
        };
        // Register the background before its controls, including the window's
        // top/side margins and the empty space around the subtitle and buttons.
        ui.interact(header_rect, layers_panel_id().with("header"), Sense::drag())
            .on_hover_text("Drag to move layers");
        let painter = ui
            .painter()
            .with_clip_rect(header_rect.intersect(ui.ctx().content_rect()));
        // Reserve paint below the header controls; fill it after the canvas and
        // window both have their current frame geometry.
        let connections = painter.add(egui::Shape::Noop);
        let mut header_icon = Rect::NOTHING;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // Keep the header height stable while the thumbnail's slot collapses.
            // The banner immediately follows it, as in the original editor.
            let slot_width = icons::CLOSE_BUTTON_SIZE * thumbnail_opacity;
            let slot = Rect::from_min_size(
                ui.cursor().min,
                Vec2::new(slot_width, icons::CLOSE_BUTTON_SIZE),
            );
            header_icon =
                Rect::from_center_size(slot.center(), Vec2::splat(24.0 * thumbnail_opacity));
            if thumbnail_opacity > 0.0 {
                let mut thumbnail = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("header-thumbnail")
                        .max_rect(Rect::from_min_size(
                            slot.min,
                            Vec2::splat(icons::CLOSE_BUTTON_SIZE),
                        ))
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );
                thumbnail.shrink_clip_rect(slot);
                thumbnail.multiply_opacity(thumbnail_opacity);
                let texture = self.texture(ui.ctx(), port, achievement);
                let response = icons::thumbnail_button(
                    &mut thumbnail,
                    layers_panel_id().with("center-icon"),
                    texture,
                    "Center edited icon on visible canvas",
                );
                header_icon = response.rect.shrink(4.0).intersect(slot);
                if !header_icon.is_positive() {
                    header_icon = Rect::from_center_size(slot.left_center(), Vec2::ZERO);
                }
                if response.clicked() && !editing_style {
                    self.start_icon_pan(ui.ctx(), port, achievement.to_owned());
                }
            }
            ui.allocate_exact_size(slot.size(), Sense::hover());
            let style_width = ((ui.available_width() - 3.0 * icons::CLOSE_BUTTON_SIZE) * 0.5)
                .clamp(184.0, 424.0)
                .min((ui.available_width() - 3.0 * icons::CLOSE_BUTTON_SIZE - 64.0).max(32.0));
            ui.allocate_ui_with_layout(
                Vec2::new(style_width, header_height),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| self.style_banner(ui, port, icon),
            );
            ui.scope(|ui| {
                let copy = icons::header_button(
                    ui,
                    layers_panel_id().with("copy"),
                    Icon::Copy,
                    "Copy recipe",
                    true,
                );
                if copy.clicked() {
                    self.recipe_clipboard = Some(icon.clone());
                }
                let paste = icons::header_button(
                    ui,
                    layers_panel_id().with("paste"),
                    Icon::Paste,
                    "Paste recipe",
                    self.recipe_clipboard.is_some(),
                );
                if paste.clicked()
                    && let Some(recipe) = self.recipe_clipboard.clone()
                {
                    let source = recipe["layers"].as_array().cloned().unwrap_or_default();
                    let (layers, _) = fresh_bundle(&source, "", None);
                    *icon = recipe;
                    icon["layers"] = json!(layers);
                }
            });
            let title_width = (ui.available_width() - icons::CLOSE_BUTTON_SIZE).max(64.0);
            ui.add_sized(
                Vec2::new(title_width, 32.0),
                egui::Label::new(egui::RichText::new("Layers").strong()).selectable(false),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let close =
                    icons::close_button(ui, layers_panel_id().with("close"), "Close layers");
                if close.clicked() {
                    self.close_layers();
                }
            });
        });
        self.layer_palette.header = Some(PaletteHeader {
            icon: header_icon,
            painter,
            connections,
        });
        ui.separator();
    }
    fn style_banner(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort, icon: &mut Value) {
        let signature = style_signature(icon);
        let local_style = self
            .selection
            .first()
            .and_then(|id| port.editor().document().entity(EntityKind::Achievement, id))
            .is_some_and(achimeld_application::style_achievements::is_local_icon);
        let project_styles = port.icon_styles();
        let linked = self.linked_icon_style(port);
        let matching = project_styles
            .iter()
            .find(|style| Some(style.id.as_str()) == linked.as_deref())
            .or_else(|| {
                project_styles
                    .iter()
                    .find(|style| style_signature(&style.recipe) == signature)
            })
            .cloned();
        let active = self
            .layer_palette
            .active_style
            .as_ref()
            .and_then(|id| project_styles.iter().find(|s| &s.id == id))
            .cloned();
        if self.layer_palette.active_style.is_some() && active.is_none() {
            self.layer_palette.active_style = None;
            self.layer_palette.curve_drag = None;
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let banner_width = (ui.available_width() - icons::CLOSE_BUTTON_SIZE).max(1.0);
            ui.allocate_ui_with_layout(
                Vec2::new(banner_width, 32.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.spacing_mut().interact_size.y = 15.0;
                    ui.spacing_mut().button_padding = Vec2::ZERO;
                    let scope = if self.linked_icon_style(port).is_some() {
                        "Editing linked project style"
                    } else if self.layer_palette.active_style.is_some() {
                        "Editing all matching icons"
                    } else {
                        "Editing this icon"
                    };
                    self.icon_style_banner_with_scope(ui, port, icon, Some(scope));
                },
            );
            if local_style {
                return;
            }
            let mut affect = self.layer_palette.active_style.is_some();
            let style = icons::header_button(
                ui,
                layers_panel_id().with("affect-style"),
                Icon::Star(affect),
                "Affect style",
                !local_style && (matching.is_some() || active.is_some()),
            )
            .on_hover_text("Edit the project style and every icon using it in this project");
            if style.clicked() {
                affect = !affect;
                port.finish_group();
                self.layer_palette.active_style = if affect {
                    matching.as_ref().map(|s| s.id.clone())
                } else {
                    None
                };
            }
        });
    }
    pub(crate) fn icon_style_banner(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        icon: &mut Value,
    ) -> egui::Response {
        self.icon_style_banner_with_scope(ui, port, icon, None)
    }
    fn icon_style_banner_with_scope(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        icon: &mut Value,
        scope: Option<&str>,
    ) -> egui::Response {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let edge = icons::CLOSE_BUTTON_SIZE;
            let scale = ui.ctx().pixels_per_point();
            let line =
                ((ui.text_style_height(&egui::TextStyle::Small) * scale).ceil() / scale).max(15.0);
            let height = (line * 2.0 + 3.0).max(edge);
            let width = (ui.available_width() - 2.0 * edge).max(1.0);
            // Keep long/truncated captions from pushing the action bodies or
            // expanding the floating Layers window after a popup opens.
            let slot = Rect::from_min_size(ui.cursor().min, Vec2::new(width, height));
            let mut caption = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("style-caption")
                    .max_rect(slot)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            caption.spacing_mut().item_spacing.y = 3.0;
            caption.spacing_mut().interact_size.y = line;
            caption.shrink_clip_rect(slot);
            let response = self.icon_style_banner_caption(&mut caption, port, icon, scope);
            ui.allocate_exact_size(slot.size(), Sense::hover());
            for (action, glyph, label) in [
                (
                    "import",
                    crate::style_picker::Glyph::Import,
                    "GnGlobalStyle",
                ),
                ("save", crate::style_picker::Glyph::Save, "GnSaveStyle"),
            ] {
                let id = response.id.with(action);
                let label = crate::localization::text(ui.ctx(), label);
                let button = crate::buttons::Button::new(glyph.image(ui.ctx(), 20.0))
                    .id(id)
                    .bar()
                    .min_size(Vec2::new(edge, height))
                    .image_tint_follows_text_color(true);
                let action_response = ui
                    .scope(|ui| {
                        ui.spacing_mut().button_padding = Vec2::ZERO;
                        ui.spacing_mut().interact_size = Vec2::new(edge, height);
                        if action == "import" {
                            crate::buttons::menu(ui, button, |ui| {
                                let globals = port.global_icon_styles().to_vec();
                                if globals.is_empty() {
                                    ui.label(crate::localization::text(ui.ctx(), "GnNoGlobal"));
                                }
                                for style in globals {
                                    if ui.app_button(&style.name).clicked() {
                                        if let Err(error) = port.import_global_icon_style(&style.id)
                                        {
                                            self.error = Some(error);
                                        } else {
                                            ui.close();
                                        }
                                    }
                                }
                            })
                            .response
                        } else {
                            let enabled = icon["layers"]
                                .as_array()
                                .is_some_and(|layers| !layers.is_empty());
                            ui.add_enabled(enabled, button)
                        }
                    })
                    .inner;
                action_response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        action_response.enabled(),
                        &label,
                    )
                });
                if action_response.on_hover_text(label).clicked() && action == "save" {
                    self.open_style_save(port, icon);
                }
            }
            response
        })
        .inner
    }
    fn icon_style_banner_caption(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        icon: &mut Value,
        scope: Option<&str>,
    ) -> egui::Response {
        let project_styles = port.icon_styles();
        let signature = style_signature(icon);
        let linked = self.linked_icon_style(port);
        let matching = project_styles
            .iter()
            .find(|s| Some(s.id.as_str()) == linked.as_deref())
            .or_else(|| {
                project_styles
                    .iter()
                    .find(|s| style_signature(&s.recipe) == signature)
            });
        let active = self
            .layer_palette
            .active_style
            .as_ref()
            .and_then(|id| project_styles.iter().find(|s| &s.id == id));
        let name = active.or(matching).map_or_else(
            || crate::localization::text(ui.ctx(), "GnNoSt"),
            |s| s.name.clone(),
        );
        let tied = crate::style_picker::steam_set_caption(
            port,
            active.or(matching).map(|s| s.id.as_str()),
        );
        let banner_width = ui.available_width();
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size.y = 15.0;
                ui.spacing_mut().button_padding = Vec2::ZERO;
                let small = egui::TextStyle::Small.resolve(ui.style());
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, small);
                let response = crate::buttons::menu(
                    ui,
                    crate::buttons::Button::new("")
                        .id(
                            if ui.layer_id()
                                == egui::LayerId::new(egui::Order::Foreground, layers_panel_id())
                            {
                                layers_panel_id().with("style-banner")
                            } else {
                                // General and Layers can both draw while a double-click
                                // opens the floating window in this same frame.
                                ui.make_persistent_id("style-banner")
                            },
                        )
                        .align(egui::Align2::LEFT_CENTER)
                        .square_corners()
                        .min_size(Vec2::new(banner_width, 15.0))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                    |ui| {
                        ui.set_width(
                            banner_width
                                .max(320.0)
                                .min((ui.ctx().content_rect().width() - 32.0).max(1.0)),
                        );
                        ui.label(crate::localization::text(ui.ctx(), "GnProjectStyles"));
                        self.project_style_rows(ui, port, icon, &project_styles);
                    },
                )
                .response;

                let (fill, ink) =
                    crate::buttons::colors(ui, &response, crate::buttons::ButtonStyle::Quiet);
                let painter = ui.painter().with_clip_rect(response.rect);
                if let Some(id) = self.selection.first().cloned()
                    && let Some(texture) = self.texture(ui.ctx(), port, &id)
                {
                    painter.image(
                        texture,
                        response.rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.42), Pos2::new(1.0, 0.58)),
                        egui::Color32::WHITE,
                    );
                    painter.rect_filled(
                        response.rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 220),
                    );
                }
                let title = name.clone();
                let mut job = egui::text::LayoutJob::simple(
                    title,
                    egui::TextStyle::Small.resolve(ui.style()),
                    ink,
                    (response.rect.width() - 22.0).max(1.0),
                );
                job.wrap.max_rows = 1;
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(
                    Pos2::new(
                        response.rect.left() + 4.0,
                        response.rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    ink,
                );
                icons::paint(
                    &painter,
                    Rect::from_center_size(
                        Pos2::new(response.rect.right() - 8.0, response.rect.center().y),
                        Vec2::splat(10.0),
                    ),
                    Icon::Atlas,
                    ink,
                );
                response.on_hover_text(name)
            })
            .inner;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let scope = scope.map(|scope| crate::localization::text(ui.ctx(), scope));
            let scope_width = scope.as_ref().map_or(0.0, |scope| {
                ui.painter()
                    .layout_no_wrap(
                        scope.clone(),
                        egui::TextStyle::Small.resolve(ui.style()),
                        ui.visuals().weak_text_color(),
                    )
                    .size()
                    .x
                    .min((ui.available_width() - 11.0).max(0.0) * 0.6)
            });
            let tied_width = ui
                .painter()
                .layout_no_wrap(
                    tied.clone(),
                    egui::TextStyle::Small.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                )
                .size()
                .x
                .min(
                    (ui.available_width() - scope.as_ref().map_or(0.0, |_| scope_width + 11.0))
                        .max(1.0),
                );
            ui.add_sized(
                Vec2::new(tied_width, ui.text_style_height(&egui::TextStyle::Small)),
                egui::Label::new(egui::RichText::new(&tied).small().weak())
                    .selectable(false)
                    .truncate(),
            )
            .on_hover_text(&tied);
            if let Some(scope) = scope {
                ui.add(egui::Separator::default().spacing(1.0));
                ui.add_sized(
                    Vec2::new(scope_width, ui.text_style_height(&egui::TextStyle::Small)),
                    egui::Label::new(egui::RichText::new(&scope).small().weak())
                        .selectable(false)
                        .truncate(),
                )
                .on_hover_text(scope);
            }
        });
        response
    }
    fn commit_style(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        id: &str,
        live: Value,
        entity: Value,
    ) {
        let Some(mut style) = self
            .layer_palette
            .active_style
            .as_ref()
            .and_then(|id| port.icon_styles().into_iter().find(|s| &s.id == id))
        else {
            return;
        };
        if self.entity_drafts.get(id).is_some_and(|d| d.base != live) {
            self.error=Some("This icon changed while its edit was invalid. Revert the draft before editing the style.".into());
            return;
        }
        let old = style_signature(&style.recipe);
        style.recipe = entity["icon"].clone();
        let commands = port
            .editor()
            .document()
            .entities(EntityKind::Achievement)
            .iter()
            .filter(|e| !achimeld_application::style_achievements::is_local_icon(e))
            .filter(|e| e["id"] == id || style_signature(&effective_icon(e)) == old)
            .map(|e| {
                let mut value = e.clone();
                if e["id"] == id {
                    value = entity.clone();
                } else {
                    let mut recipe = style.recipe.clone();
                    let (layers, _) = fresh_bundle(
                        recipe["layers"].as_array().map_or(&[], Vec::as_slice),
                        "",
                        None,
                    );
                    recipe["layers"] = json!(layers);
                    value["icon"] = recipe;
                }
                Command::ReplaceEntity {
                    kind: EntityKind::Achievement,
                    id: e["id"].as_str().unwrap_or("").to_owned(),
                    value,
                }
            })
            .collect();
        let label = self.layer_palette.edit_label.take();
        if label.is_some() {
            port.finish_group();
        }
        let group = format!("style-{}", style.id);
        match port.execute_style_edit(
            style,
            Command::Batch(commands),
            if label.is_some() { None } else { Some(&group) },
        ) {
            Ok(()) => {
                self.entity_drafts.remove(id);
            }
            Err(error) => {
                self.entity_drafts.insert(
                    id.into(),
                    crate::desktop::EntityDraft {
                        base: live,
                        value: entity,
                        error,
                    },
                );
            }
        }
        if !ui.ctx().egui_wants_keyboard_input() && !ui.input(|i| i.pointer.any_down()) {
            port.finish_group();
        }
    }
    pub(crate) fn layer_edit_target(&self, port: &dyn EditorPort) -> Option<String> {
        if self.view == crate::desktop::View::Style {
            port.style_viewer()?.selected_style.clone()
        } else {
            self.selection.first().cloned()
        }
    }
    pub(crate) fn layer_edit_entity(&self, port: &dyn EditorPort, id: &str) -> Option<Value> {
        port.editor()
            .document()
            .entity(EntityKind::Achievement, id)
            .cloned()
            .or_else(|| {
                port.icon_styles()
                    .into_iter()
                    .find(|s| s.id == id)
                    .map(|s| json!({"id":s.id,"icon":s.recipe}))
            })
    }
    pub(crate) fn layer_editor(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort) {
        let pane = if self.view == crate::desktop::View::Style
            || port.preferences().layer_properties_in_sidebar
        {
            LayerPane::Stack
        } else {
            LayerPane::Combined
        };
        self.layer_editor_pane(ui, port, pane);
    }
    pub(crate) fn sidebar_layer_properties(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
    ) {
        self.layer_editor_pane(ui, port, LayerPane::Properties);
    }
    fn layer_editor_pane(&mut self, ui: &mut egui::Ui, port: &mut dyn EditorPort, pane: LayerPane) {
        let viewer_style = if self.view == crate::desktop::View::Style {
            port.style_viewer()
                .and_then(|state| state.selected())
                .map(|style| {
                    if style.id.contains('/') {
                        style
                    } else {
                        port.icon_styles()
                            .into_iter()
                            .find(|s| s.id == style.id)
                            .unwrap_or(style)
                    }
                })
        } else {
            None
        };
        let (id, live) = if self.view == crate::desktop::View::Style {
            let Some(style) = &viewer_style else {
                return;
            };
            (
                style.id.clone(),
                json!({"id":style.id,"icon":style.recipe,"unlockValue":port.style_viewer().and_then(|s| s.preview.unlock_number().ok()),"styleAchievement":true}),
            )
        } else {
            let Some(id) = self.selection.first().cloned() else {
                return;
            };
            let Some(live) = port
                .editor()
                .document()
                .entity(EntityKind::Achievement, &id)
                .cloned()
            else {
                return;
            };
            (id, live)
        };
        let readonly = viewer_style.as_ref().is_some_and(|s| s.id.contains('/'));
        if readonly {
            ui.disable();
        }
        if self.layer_palette.achievement.as_ref() != Some(&id) {
            self.layer_palette.achievement = Some(id.clone());
            self.layer_drag = None;
            self.layer_palette.effect_drag = None;
            self.layer_palette.visibility_stroke = None;
            self.layer_palette.selected_effect = None;
            self.layer_palette.pending = None;
            self.layer_palette.keyboard_active = false;
            self.layer_palette.active_style = None;
            if viewer_style.is_some() {
                self.selected_layer = None;
            }
        }
        let mut entity = self
            .entity_drafts
            .get(&id)
            .map(|d| d.value.clone())
            .unwrap_or_else(|| live.clone());
        let before = entity.clone();
        if achimeld_application::style_achievements::is_local_icon(&live) {
            self.layer_palette.active_style = None;
        }
        let mut icon = effective_icon(&entity);
        let initial = icon.clone();
        if pane != LayerPane::Properties {
            if viewer_style.is_some() {
                buttons::bar(ui, 24.0, |ui| {
                    ui.label("Layers");
                    if readonly {
                        ui.weak("Built-in · duplicate to edit");
                    }
                });
            } else {
                self.palette_header(ui, port, &id, &mut icon);
            }
        }
        if !icon["layers"].as_array().is_some_and(|layers| {
            layers
                .iter()
                .any(|l| l["id"].as_str() == self.selected_layer.as_deref())
        }) {
            self.selected_layer = if viewer_style.is_some() {
                None
            } else {
                icon["layers"].as_array().and_then(|layers| {
                    stack(layers, &self.layer_palette.collapsed)
                        .first()
                        .map(|(id, _)| id.clone())
                })
            };
        }
        if pane != LayerPane::Stack
            && let Some(error) = port.preview_error(&id)
        {
            egui::Panel::bottom(ui.id().with("preview-status"))
                .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("Preview could not update: {error}"))
                                .color(theme::color(port.preferences(), "Warning")),
                        )
                        .wrap(),
                    );
                });
        }
        let area = ui.available_rect_before_wrap();
        if pane == LayerPane::Properties {
            self.layer_palette.properties_rect = Some(area);
        } else if pane == LayerPane::Combined {
            self.layer_palette.properties_rect = None;
        }
        if ui.input(|i| i.pointer.any_pressed()) {
            self.layer_palette.keyboard_active = ui.input(|i| {
                i.pointer.interact_pos().is_some_and(|p| {
                    area.contains(p)
                        || self
                            .layer_palette
                            .properties_rect
                            .is_some_and(|rect| rect.contains(p))
                })
            });
        }
        if let Some(draft) = self.entity_drafts.get(&id) {
            ui.colored_label(theme::color(port.preferences(), "Error"), &draft.error);
            if ui.app_button("Revert invalid edit").clicked() {
                self.entity_drafts.remove(&id);
                return;
            }
        }
        let area = ui.available_rect_before_wrap();
        let (left, right) = if pane == LayerPane::Combined {
            let split_width = (area.width() * self.layer_palette.split)
                .clamp(172.0, (area.width() - 157.0).max(172.0));
            let divider = Rect::from_min_size(
                Pos2::new(area.left() + split_width, area.top()),
                Vec2::new(7.0, area.height()),
            );
            let drag = ui
                .interact(divider, ui.id().with("layer-split"), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if drag.dragged()
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            {
                self.layer_palette.split = ((pos.x - area.left()) / area.width()).clamp(0.25, 0.75);
            }
            ui.painter().line_segment(
                [divider.center_top(), divider.center_bottom()],
                Stroke::new(1.0, theme::color(port.preferences(), "Border")),
            );
            let left = Rect::from_min_max(area.min, Pos2::new(divider.left() - 4.0, area.bottom()));
            let right = Rect::from_min_max(Pos2::new(divider.right() + 4.0, area.top()), area.max);
            (left, right)
        } else {
            (area, area)
        };
        if pane != LayerPane::Properties {
            ui.scope_builder(
                egui::UiBuilder::new().id_salt("layer-stack").max_rect(left),
                |ui| {
                    ui.shrink_clip_rect(left);
                    egui::Panel::bottom("layer-footer")
                        .exact_size(crate::layer_toolbar::HEIGHT)
                        .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
                        .show(ui, |ui| self.layer_footer(ui, &icon));
                    let height = ui.available_height().max(48.0);
                    egui::ScrollArea::vertical()
                        .id_salt("layer-list-scroll")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(layers) = icon["layers"].as_array_mut() {
                                self.layer_rows(ui, port, &id, layers);
                            }
                        });
                },
            );
        }
        if pane != LayerPane::Stack {
            let grayscale = icon["layers"].as_array().is_some_and(|layers| {
                self.selected_layer
                    .as_deref()
                    .is_some_and(|id| achimeld_application::masks::content_ids(layers).contains(id))
            });
            let source = icon["layers"]
                .as_array()
                .and_then(|layers| {
                    layers
                        .iter()
                        .find(|layer| layer["id"].as_str() == self.selected_layer.as_deref())
                })
                .map(|layer| self.layer_artwork(ui.ctx(), port, &id, layer).1)
                .unwrap_or_else(|| theme::color(port.preferences(), "Accent"));
            ui.painter().add(crate::layer_backdrop::paint(
                ui,
                egui::Id::new(("layer-property-backdrop", &id)),
                right,
                right.top(),
                source,
                false,
                port.preferences(),
            ));
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("layer-properties")
                    .ui_stack_info(if grayscale {
                        egui::UiStackInfo::default().with_tag(crate::color_picker::GRAYSCALE)
                    } else {
                        egui::UiStackInfo::default()
                    })
                    .max_rect(right.shrink(10.0)),
                |ui| {
                    if self.selected_layer.is_none() {
                        if viewer_style.is_none() {
                            ui.weak("Select a layer to edit its properties.");
                        }
                    } else if self.layer_palette.selected_effect.is_some() {
                        self.effect_properties(ui, port, &mut icon);
                    } else if let Some(layers) = icon["layers"].as_array_mut() {
                        self.layer_properties(ui, port, layers, &mut entity);
                    }
                },
            );
        }
        if self.layer_palette.keyboard_active && !ui.ctx().egui_wants_keyboard_input() {
            for (key, action) in [
                (egui::Key::C, LayerAction::Copy),
                (egui::Key::V, LayerAction::Paste),
            ] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key)) {
                    self.layer_palette.pending = Some(action);
                }
            }
        }
        if let Some(action) = self.layer_palette.pending.take() {
            self.layer_action(action, &mut icon);
            port.finish_group();
        }
        if let Some(background) = self.layer_palette.background.take() {
            icon["backgroundColorHex"] = background;
        }
        let icon_changed = icon != initial;
        if icon_changed {
            entity["icon"] = icon;
        }
        if let Some(mut style) = viewer_style {
            if icon_changed && !readonly {
                style.recipe = entity["icon"].clone();
                style.preview_png = None;
                let group = format!("viewer-style-{}", style.id);
                match achimeld_application::style_management::edit(port.editor().document(), style)
                    .and_then(|command| port.execute("Edit project style", command, Some(&group)))
                {
                    Ok(()) => {
                        self.entity_drafts.remove(&id);
                    }
                    Err(error) => {
                        self.entity_drafts.insert(
                            id.clone(),
                            crate::desktop::EntityDraft {
                                base: live,
                                value: entity,
                                error,
                            },
                        );
                    }
                }
                if !ui.ctx().egui_wants_keyboard_input() && !ui.input(|i| i.pointer.any_down()) {
                    port.finish_group();
                }
            }
        } else if icon_changed && self.layer_palette.active_style.is_some() {
            self.commit_style(ui, port, &id, live, entity);
        } else {
            self.commit_entity(ui, port, &id, live, before, entity);
        }
        ui.allocate_rect(area, Sense::hover());
    }
    fn layer_artwork(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        achievement: &str,
        layer: &Value,
    ) -> (Option<egui::TextureId>, egui::Color32) {
        let id = layer["id"].as_str().unwrap_or("");
        let key = format!("{achievement}/{id}");
        let authored = crate::layer_backdrop::source_color(layer);
        // Adjustments show their type symbol, so they never request artwork
        // thumbnails or sample the adjusted composition for their row color.
        if is_adjustment(layer["kind"].as_str().unwrap_or("")) {
            return (
                None,
                authored.unwrap_or_else(|| theme::color(port.preferences(), "Accent")),
            );
        }
        let Some(preview) = port.layer_preview(achievement, id) else {
            return (
                None,
                authored.unwrap_or_else(|| theme::color(port.preferences(), "Accent")),
            );
        };
        if self
            .layer_palette
            .artwork_colors
            .get(&key)
            .is_none_or(|(fingerprint, _)| fingerprint != &preview.fingerprint)
        {
            if self.layer_palette.artwork_colors.len() >= 512 {
                self.layer_palette.artwork_colors.clear();
            }
            self.layer_palette.artwork_colors.insert(
                key.clone(),
                (
                    preview.fingerprint.clone(),
                    crate::layer_backdrop::dominant_color(&preview.rgba),
                ),
            );
        }
        let source = authored
            .or(self.layer_palette.artwork_colors[&key].1)
            .unwrap_or_else(|| theme::color(port.preferences(), "Accent"));
        (self.preview_texture(ctx, &key, preview), source)
    }
    fn layer_rows(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        achievement: &str,
        layers: &mut [Value],
    ) {
        let rows = stack(layers, &self.layer_palette.collapsed);
        let mut centers = Vec::new();
        let mut started = None;
        let mut stopped = false;
        let mut eyes = Vec::new();
        let mut tree_nodes = Vec::new();
        let mut selection_previews = Vec::new();
        let mut stroke_started = false;
        let mut drag_preview = None;
        let drag_enabled =
            ui.is_enabled() && ui.input(|i| i.focused && !i.key_pressed(egui::Key::Escape));
        if ui.input(|i| i.pointer.primary_pressed()) {
            self.layer_palette.row_drag_candidate = None;
        }
        if !drag_enabled {
            self.layer_drag = None;
            self.layer_palette.row_drag_candidate = None;
        }
        if !drag_enabled
            || ui.input(|i| !i.pointer.primary_down() && !i.pointer.primary_released())
            || self.layer_palette.effect_drag.as_ref().is_some_and(|drag| {
                !rows.iter().any(|(id, _)| id == &drag.layer)
                    || !self.layer_palette.expanded_effects.contains(&drag.layer)
            })
        {
            self.layer_palette.effect_drag = None;
        }
        if !ui.is_enabled()
            || ui.input(|i| {
                !i.focused
                    || i.key_pressed(egui::Key::Escape)
                    || (!i.pointer.primary_down() && !i.pointer.primary_released())
            })
        {
            self.layer_palette.visibility_stroke = None;
        }
        self.layer_palette.rows.clear();
        let mask_layers = crate::layer_masks::menu_layers(layers);
        let folders: Vec<_> = layers
            .iter()
            .filter(|l| l["kind"] == "Folder")
            .filter_map(|l| {
                Some((
                    l["id"].as_str()?.to_owned(),
                    l["displayName"].as_str().unwrap_or("Folder").to_owned(),
                ))
            })
            .collect();
        self.layer_palette.drag_folder = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        for (id, depth) in &rows {
            let Some(layer) = layers.iter_mut().find(|l| l["id"].as_str() == Some(id)) else {
                continue;
            };
            let kind = layer["kind"].as_str().unwrap_or("Layer").to_owned();
            let mask_source = achimeld_application::masks::is_source(&mask_layers, id);
            let selected = self.selected_layer.as_ref() == Some(id);
            let (texture, source) = self.layer_artwork(ui.ctx(), port, achievement, layer);
            let background = ui.painter().add(egui::Shape::Noop);
            let mut art_bottom = ui.cursor().top();
            let mut row_hit = None;
            let mut eye_rect = Rect::NOTHING;
            let frame = ui
                // The row sits beneath its children and catches unused space.
                .scope_builder(
                    egui::UiBuilder::new()
                        .id_salt(id)
                        .sense(Sense::click_and_drag()),
                    |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 6,
                                right: 6,
                                top: 4,
                                bottom: 4,
                            })
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                ui.spacing_mut().interact_size.y = 24.0;
                                ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
                                let atlas = kind == "AtlasTile";
                                let font_size = egui::TextStyle::Body.resolve(ui.style()).size;
                                let accessory_width = if atlas { 30.0 } else { 0.0 };
                                let compact_controls = 28.0
                                    + if kind == "Folder" { 28.0 } else { 0.0 }
                                    + accessory_width
                                    + 56.0;
                                // Deep folders must give up indentation before pushing controls out of the pane.
                                let indent = (*depth as f32 * 22.0).min(
                                    (ui.available_width()
                                        - compact_controls
                                        - (font_size * 3.0).max(32.0))
                                    .max(0.0),
                                );
                                let mut effects_origin = Pos2::ZERO;
                                ui.allocate_ui_with_layout(
                                    Vec2::new(ui.available_width(), 42.0),
                                    egui::Layout::left_to_right(egui::Align::Min),
                                    |ui| {
                                        ui.add_space(indent);
                                        let visible = layer["visible"].as_bool().unwrap_or(true);
                                        // The two controls share the thumbnail's height, with the eye
                                        // at its top and the smaller fx button at its bottom.
                                        let visibility = ui
                                            .allocate_ui_with_layout(
                                                Vec2::new(22.0, 42.0),
                                                egui::Layout::top_down(egui::Align::Center),
                                                |ui| {
                                                    let eye = icons::layer_button(
                                                        ui,
                                                        layer_control_id(achievement, id, "eye"),
                                                        Icon::Eye(visible),
                                                        "Layer visibility",
                                                        1.0,
                                                        0.0,
                                                    );
                                                    if !mask_source && !is_adjustment(&kind) {
                                                        ui.add_space(
                                                            42.0 - eye.rect.height()
                                                                - icons::layer_control_size(
                                                                    Icon::Effects,
                                                                ),
                                                        );
                                                        let has_effects = layer["effects"]
                                                            .as_array()
                                                            .is_some_and(|effects| {
                                                                !effects.is_empty()
                                                            });
                                                        let expanded = self
                                                            .layer_palette
                                                            .expanded_effects
                                                            .contains(id);
                                                        let effects = icons::layer_button(
                                                            ui,
                                                            layer_control_id(achievement, id, "fx"),
                                                            Icon::Effects,
                                                            if has_effects || expanded {
                                                                "Expand / collapse effects"
                                                            } else {
                                                                "Add effect"
                                                            },
                                                            if has_effects { 1.0 } else { 0.6 },
                                                            if has_effects { 0.15 } else { 0.0 },
                                                        );
                                                        if effects.clicked() {
                                                            achimeld_application::diagnostics::record(
                                                                "INFO", "effects", "Layer FX control activated",
                                                            );
                                                            self.selected_layer = Some(id.clone());
                                                            self.layer_palette.selected_effect =
                                                                None;
                                                            if expanded {
                                                                self.layer_palette
                                                                    .expanded_effects
                                                                    .remove(id);
                                                            } else if has_effects {
                                                                self.layer_palette
                                                                    .expanded_effects
                                                                    .insert(id.clone());
                                                            }
                                                        }
                                                        if !has_effects && !expanded {
                                                            let operation = effects.clicked().then(|| achimeld_application::diagnostics::Operation::begin("draw effect menu"));
                                                            crate::buttons::menu_popup(
                                                                ui,
                                                                &effects,
                                                                |ui| {
                                                                    if let Some(effect) =
                                                                        effect_menu(ui, layer)
                                                                    {
                                                                        self.selected_layer =
                                                                            Some(id.clone());
                                                                        self.layer_palette
                                                                            .selected_effect =
                                                                            Some(effect);
                                                                        self.layer_palette
                                                                            .expanded_effects
                                                                            .insert(id.clone());
                                                                    }
                                                                },
                                                            );
                                                            if let Some(operation) = operation { operation.finish("completed"); }
                                                        }
                                                        effects_origin =
                                                            effects.rect.center_bottom();
                                                    }
                                                    eye
                                                },
                                            )
                                            .inner;
                                        eye_rect = visibility.rect;
                                        eyes.push((
                                            id.clone(),
                                            visibility.rect.intersect(ui.clip_rect()),
                                        ));
                                        if visibility.enabled()
                                            && self.layer_palette.visibility_press_frame
                                                != Some(ui.ctx().cumulative_frame_nr())
                                            && ui
                                                .input(|i| i.focused && i.pointer.primary_pressed())
                                            && (visibility.is_pointer_button_down_on()
                                                || visibility
                                                    .clicked_by(egui::PointerButton::Primary))
                                        {
                                            // A discarded layout pass replays the same input against
                                            // the already-updated document. Start each stroke once.
                                            self.layer_palette.visibility_press_frame =
                                                Some(ui.ctx().cumulative_frame_nr());
                                            port.finish_group();
                                            self.layer_drag = None;
                                            self.layer_palette.visibility_stroke =
                                                Some(VisibilityStroke {
                                                    visible: !visible,
                                                    visited: BTreeSet::new(),
                                                    pointer: None,
                                                });
                                            stroke_started = true;
                                        } else if visibility.clicked()
                                            && !visibility.clicked_by(egui::PointerButton::Primary)
                                            && self.layer_palette.visibility_press_frame
                                                != Some(ui.ctx().cumulative_frame_nr())
                                        {
                                            self.layer_palette.visibility_press_frame =
                                                Some(ui.ctx().cumulative_frame_nr());
                                            port.finish_group();
                                            layer["visible"] = json!(!visible);
                                        }
                                        if kind == "Folder" {
                                            let collapsed =
                                                self.layer_palette.collapsed.contains(id);
                                            if icons::sized_button(
                                                ui,
                                                Icon::Chevron(collapsed),
                                                "Expand / collapse folder",
                                                true,
                                                24.0,
                                            )
                                            .clicked()
                                            {
                                                if collapsed {
                                                    self.layer_palette.collapsed.remove(id);
                                                } else {
                                                    self.layer_palette.collapsed.insert(id.clone());
                                                    self.selected_layer = Some(id.clone());
                                                    self.layer_palette.selected_effect = None;
                                                }
                                            }
                                        }
                                        let preview_width = 48.0;
                                        let show_previews = ui.available_width()
                                            >= preview_width
                                                + 6.0
                                                + (font_size * 6.0).max(72.0)
                                                + accessory_width
                                                + 56.0;
                                        if show_previews {
                                            let thumbnail = icons::draggable_thumbnail(
                                                ui, texture, &kind, 42.0,
                                            );
                                            selection_previews.push((id.clone(), thumbnail.rect));
                                            if thumbnail.drag_started() {
                                                started = Some(id.clone());
                                            }
                                            if thumbnail.drag_stopped() {
                                                stopped = true;
                                            }
                                            if thumbnail.clicked() {
                                                self.selected_layer = Some(id.clone());
                                                self.layer_palette.selected_effect = None;
                                            }
                                            // Together with item spacing, leave 10 px before the two-line label.
                                            ui.add_space(4.0);
                                        }
                                        let reserve = (font_size * 9.0).max(120.0) * 0.7;
                                        let slider_scale = 0.8;
                                        let minimum_slider =
                                            (font_size * 7.0).max(96.0) * slider_scale;
                                        let blend_width = [
                                            "Normal",
                                            "Multiply",
                                            "Screen",
                                            "Overlay",
                                            "Darken",
                                            "Lighten",
                                            "Add",
                                            "Difference",
                                        ]
                                        .iter()
                                        .map(|mode| {
                                            ui.painter()
                                                .layout_no_wrap(
                                                    crate::localization::text(ui.ctx(), mode),
                                                    egui::TextStyle::Button.resolve(ui.style()),
                                                    ui.visuals().text_color(),
                                                )
                                                .size()
                                                .x
                                        })
                                        .fold(0.0_f32, f32::max)
                                        .ceil()
                                            + 48.0;
                                        let available =
                                            ui.available_width() - accessory_width - reserve - 12.0;
                                        let show_slider = available >= minimum_slider + 24.0 + 12.0;
                                        let show_blend = show_slider
                                            && available >= minimum_slider + blend_width + 12.0;
                                        let blend_width = if mask_source {
                                            0.0
                                        } else if show_blend {
                                            blend_width
                                        } else {
                                            24.0
                                        };
                                        let opacity_width = if mask_source {
                                            0.0
                                        } else if show_slider {
                                            ((available - blend_width) * 0.65 * slider_scale).clamp(
                                                minimum_slider,
                                                minimum_slider.max(180.0 * slider_scale),
                                            )
                                        } else {
                                            24.0
                                        };
                                        let name_width = (ui.available_width()
                                            - accessory_width
                                            - opacity_width
                                            - blend_width
                                            - 12.0)
                                            .max(16.0);
                                        let name =
                                            layer["displayName"].as_str().unwrap_or(label(&kind));
                                        let name_rect = ui
                                            .allocate_ui_with_layout(
                                                Vec2::new(name_width, 42.0),
                                                egui::Layout::top_down(egui::Align::Min),
                                                |ui| {
                                                    ui.set_width(name_width);
                                                    ui.add_space(3.0);
                                                    let bold = egui::FontFamily::Name(
                                                        crate::typography::BOLD_FAMILY.into(),
                                                    );
                                                    let family = if ui.fonts(|fonts| {
                                                        fonts.families().contains(&bold)
                                                    }) {
                                                        bold
                                                    } else {
                                                        egui::FontFamily::Proportional
                                                    };
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(
                                                                crate::localization::text(
                                                                    ui.ctx(),
                                                                    name,
                                                                ),
                                                            )
                                                            .family(family),
                                                        )
                                                        .truncate(),
                                                    );
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(layer_summary(
                                                                layer,
                                                            ))
                                                            .small()
                                                            .weak(),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            )
                                            .response
                                            .rect;
                                        // Keep the label's explicit target alongside the row background.
                                        let hit = ui
                                            .interact(
                                                name_rect,
                                                egui::Id::new(("layer-drag", achievement, id)),
                                                Sense::click_and_drag(),
                                            )
                                            .on_hover_text(
                                                "Drag to reorder; drop on a folder to group",
                                            );
                                        if hit.drag_started() {
                                            started = Some(id.clone());
                                        }
                                        if hit.drag_stopped() {
                                            stopped = true;
                                        }
                                        row_hit = Some(hit);
                                        if atlas
                                            && icons::sized_button(
                                                ui,
                                                Icon::Atlas,
                                                "Choose atlas tile",
                                                true,
                                                24.0,
                                            )
                                            .clicked()
                                        {
                                            self.selected_layer = Some(id.clone());
                                            self.layer_palette.selected_effect = None;
                                            self.open_source_picker(
                                                ui.ctx(),
                                                port,
                                                id,
                                                crate::source_picker::SourceKind::Atlas,
                                            );
                                        }
                                        if !mask_source {
                                            row_opacity(ui, layer, show_slider, opacity_width);
                                            self.row_blend(
                                                ui,
                                                port,
                                                achievement,
                                                layer,
                                                show_blend,
                                                blend_width,
                                            );
                                        }
                                    },
                                );
                                art_bottom = ui.min_rect().bottom();
                                if !mask_source
                                    && !is_adjustment(&kind)
                                    && self.layer_palette.expanded_effects.contains(id)
                                {
                                    // Own FX are rendered before the next stack entry (including folder children).
                                    self.layer_effects(
                                        ui,
                                        port,
                                        achievement,
                                        layer,
                                        indent,
                                        effects_origin,
                                    );
                                }
                            })
                    },
                );
            if let Some(preview) =
                self.mask_entry(ui, port, achievement, layer, &mask_layers, *depth)
            {
                selection_previews.push(preview);
            }
            let rect = frame.inner.response.rect;
            let row = frame.response;
            if drag_enabled
                && row.enabled()
                && ui.input(|i| i.pointer.primary_pressed())
                && row.is_pointer_button_down_on()
            {
                // Click-only controls can pass a drag through to a parent in egui.
                // Only arm background dragging when no child owns the initial click.
                let hovered = ui.ctx().interaction_snapshot(|i| i.hovered.clone());
                let child_clicked = hovered.iter().any(|&widget| {
                    widget != row.id
                        && ui
                            .ctx()
                            .read_response(widget)
                            .is_some_and(|hit| hit.sense.senses_click())
                });
                if !child_clicked {
                    self.layer_palette.row_drag_candidate = Some(id.clone());
                    self.selected_layer = Some(id.clone());
                    self.layer_palette.selected_effect = None;
                }
            }
            if row.drag_started() && self.layer_palette.row_drag_candidate.as_ref() == Some(id) {
                started = Some(id.clone());
            }
            stopped |= row.drag_stopped();
            tree_nodes.push((
                id.clone(),
                layer["parentFolderId"].as_str().map(str::to_owned),
                eye_rect,
                rect,
                kind == "Folder",
            ));
            ui.painter().set(
                background,
                crate::layer_backdrop::paint(
                    ui,
                    egui::Id::new(("layer-backdrop", achievement, id)),
                    rect,
                    art_bottom,
                    source,
                    selected,
                    port.preferences(),
                ),
            );
            if selected {
                ui.painter().rect_filled(
                    Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                    0.0,
                    theme::color(port.preferences(), "Accent"),
                );
            }
            // One physical pixel separates adjacent layer blocks, independent of UI scale.
            let pixel = 1.0 / ui.ctx().pixels_per_point();
            let (divider, _) =
                ui.allocate_exact_size(Vec2::new(rect.width(), pixel), Sense::hover());
            ui.painter()
                .rect_filled(divider, 0.0, theme::color(port.preferences(), "Border"));
            self.layer_palette.rows.insert(id.clone(), rect);
            centers.push(rect.center().y);
            let hit = row_hit.expect("layer row includes its name") | row;
            if hit.clicked() {
                self.selected_layer = Some(id.clone());
                self.layer_palette.selected_effect = None;
            }
            crate::buttons::context_menu(&hit, |ui| {
                self.selected_layer = Some(id.clone());
                for (title, action) in [
                    ("Delete layer", LayerAction::Delete),
                    ("Clone layer", LayerAction::Duplicate),
                    ("Copy layer", LayerAction::Copy),
                    ("Paste layer", LayerAction::Paste),
                ] {
                    if ui
                        .add_enabled(
                            !matches!(action, LayerAction::Paste)
                                || self.layer_palette.clipboard.is_some(),
                            crate::buttons::Button::new(title),
                        )
                        .clicked()
                    {
                        self.layer_palette.pending = Some(action);
                        ui.close();
                    }
                }
                if layer["parentFolderId"].is_string()
                    && ui.app_button("Remove from folder").clicked()
                {
                    self.layer_palette.pending = Some(LayerAction::RemoveFromFolder);
                    ui.close();
                }
                ui.app_menu_button("Move into folder", |ui| {
                    if ui.app_button("Root").clicked() {
                        self.layer_palette.pending = Some(LayerAction::MoveInto(None));
                        ui.close();
                    }
                    for (folder, name) in &folders {
                        if folder != id && ui.app_button(name).clicked() {
                            self.layer_palette.pending =
                                Some(LayerAction::MoveInto(Some(folder.clone())));
                            ui.close();
                        }
                    }
                });
                if ui
                    .add_enabled(
                        self.view != crate::desktop::View::Style,
                        buttons::Button::new("Replace from disk…"),
                    )
                    .clicked()
                {
                    self.request(UiRequest::ImportLayerImage {
                        achievement_id: achievement.into(),
                        layer_id: Some(id.clone()),
                    });
                    ui.close();
                }
                if ui.app_button("Edit source / atlas tile").clicked() {
                    self.layer_palette.selected_effect = None;
                    self.open_source_picker(
                        ui.ctx(),
                        port,
                        id,
                        if kind == "AtlasTile" {
                            crate::source_picker::SourceKind::Atlas
                        } else {
                            crate::source_picker::SourceKind::Image
                        },
                    );
                    ui.close();
                }
                self.mask_commands(ui, layer, &mask_layers);
                if !mask_source {
                    ui.app_menu_button("Add effect", |ui| {
                        if let Some(effect) = effect_menu(ui, layer) {
                            self.layer_palette.expanded_effects.insert(id.clone());
                            self.layer_palette.selected_effect = Some(effect);
                        }
                    });
                }
            });
            if started.as_ref() == Some(id) || (self.layer_drag.is_some() && selected) {
                // Reuse this row's paint commands without creating ghost widgets,
                // fetching artwork again, or changing the list's hit targets.
                drag_preview = ui.ctx().graphics(|graphics| {
                    graphics.get(ui.layer_id()).map(|list| {
                        (
                            rect,
                            list.all_entries()
                                .skip(background.0)
                                .map(|paint| paint.shape.clone())
                                .collect::<Vec<_>>(),
                        )
                    })
                });
            }
            if kind == "Folder"
                && self.layer_drag.is_some()
                && ui.input(|i| {
                    i.pointer
                        .interact_pos()
                        .is_some_and(|p| rect.shrink2(Vec2::new(0.0, 8.0)).contains(p))
                })
                && self
                    .selected_layer
                    .as_deref()
                    .is_some_and(|selected| !descendants(layers, selected).contains(id))
            {
                self.layer_palette.drag_folder = Some(id.clone());
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(2.0, theme::color(port.preferences(), "Accent")),
                    egui::StrokeKind::Inside,
                );
            }
        }
        // Resolve selection after every hit target, including a mask clicked
        // below its owner, so the old and new previews never both appear active.
        if self.layer_palette.selected_effect.is_none()
            && let Some((_, rect)) = selection_previews
                .iter()
                .find(|(id, _)| self.selected_layer.as_ref() == Some(id))
        {
            ui.painter().rect_stroke(
                *rect,
                0.0,
                Stroke::new(2.0, theme::color(port.preferences(), "Primary text")),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_stroke(
                rect.shrink(2.0),
                0.0,
                Stroke::new(1.0, theme::color(port.preferences(), "Window")),
                egui::StrokeKind::Inside,
            );
        }
        // Paint after row backgrounds so the folder spine continues through
        // child blocks, including their expanded effects and nested folders.
        let tree_stroke = Stroke::new(
            1.0 / ui.ctx().pixels_per_point(),
            ui.visuals().weak_text_color().gamma_multiply(0.6),
        );
        let mut folder_spines = BTreeMap::new();
        for (id, parent, eye, row, folder) in tree_nodes {
            if let Some(spine) = parent
                .as_ref()
                .and_then(|parent| folder_spines.get_mut(parent))
            {
                let spine: &mut Pos2 = spine;
                let branch_end = eye.left() - 4.0;
                if branch_end > spine.x {
                    let junction = Pos2::new(spine.x, eye.center().y);
                    ui.painter().line_segment([*spine, junction], tree_stroke);
                    ui.painter()
                        .line_segment([junction, Pos2::new(branch_end, junction.y)], tree_stroke);
                    *spine = junction;
                }
            }
            if folder {
                folder_spines.insert(id, Pos2::new(eye.center().x, row.bottom()));
            }
        }
        if let Some(mut stroke) = self.layer_palette.visibility_stroke.take() {
            let mut ended = false;
            for event in ui.input(|i| i.events.clone()) {
                if stroke_started {
                    if let egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } = event
                    {
                        stroke.paint(pos, pos, &eyes, layers);
                        stroke.pointer = Some(pos);
                        stroke_started = false;
                    }
                    continue;
                }
                match event {
                    egui::Event::PointerMoved(pos) => {
                        stroke.paint(stroke.pointer.unwrap_or(pos), pos, &eyes, layers);
                        stroke.pointer = Some(pos);
                    }
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        stroke.paint(stroke.pointer.unwrap_or(pos), pos, &eyes, layers);
                        ended = true;
                        break;
                    }
                    egui::Event::PointerGone => stroke.pointer = None,
                    _ => {}
                }
            }
            if !ended && ui.input(|i| i.pointer.primary_down()) {
                // Also handle an eye entering under the pointer while scrolling.
                if let Some(pos) = stroke.pointer {
                    stroke.paint(pos, pos, &eyes, layers);
                }
                self.layer_palette.visibility_stroke = Some(stroke);
            }
            ui.ctx().request_repaint();
        }
        let ids: Vec<_> = rows.into_iter().map(|(id, _)| id).collect();
        if drag_enabled && let Some(id) = started {
            self.layer_palette.effect_drag = None;
            self.layer_drag = ReorderGesture::begin(ids.clone(), centers.clone(), &id);
            self.selected_layer = Some(id);
            self.layer_palette.selected_effect = None;
        }
        if let Some(drag) = self.layer_drag.as_mut() {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                drag.update(pos.y);
            }
            let slot = drag.slot();
            let y = if slot == 0 {
                ids.first()
                    .and_then(|id| self.layer_palette.rows.get(id))
                    .map(|rect| rect.top())
            } else {
                ids.get(slot - 1)
                    .and_then(|id| self.layer_palette.rows.get(id))
                    .map(|rect| rect.bottom())
            }
            .unwrap_or(ui.cursor().top());
            ui.painter().line_segment(
                [
                    Pos2::new(ui.min_rect().left(), y),
                    Pos2::new(ui.min_rect().right(), y),
                ],
                Stroke::new(2.0, theme::color(port.preferences(), "Accent")),
            );
        }
        if !drag_enabled {
            self.layer_drag = None;
        }
        if stopped && let Some(drag) = self.layer_drag.take() {
            match drag.finish(&ids) {
                Ok(order) => {
                    if let Some(folder) = self.layer_palette.drag_folder.take() {
                        self.layer_palette.pending = Some(LayerAction::MoveInto(Some(folder)));
                    } else if let Some(selected) = self.selected_layer.as_deref() {
                        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                            let parent = layer_drop_parent(
                                layers,
                                selected,
                                &ids,
                                &self.layer_palette.rows,
                                pointer,
                            );
                            if let Some(layer) = layers.iter_mut().find(|l| l["id"] == selected)
                                && layer["parentFolderId"].as_str() != parent.as_deref()
                            {
                                layer["parentFolderId"] =
                                    parent.map_or(Value::Null, |id| json!(id));
                            }
                        }
                        reorder_visual(layers, selected, &order);
                        self.layer_palette.edit_label = Some("Reorder icon layers");
                    }
                }
                Err(error) => self.error = Some(error),
            }
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.layer_drag = None;
            self.layer_palette.row_drag_candidate = None;
        }
        if self.layer_drag.is_some()
            && let Some((rect, shapes)) = drag_preview
            && let Some((origin, pointer)) =
                ui.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()))
        {
            let delta = pointer - origin;
            let mut painter = ui
                .ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new(("layer-drag-preview", achievement)),
                ))
                .with_clip_rect(rect.translate(delta).intersect(ui.ctx().content_rect()));
            painter.set_opacity(0.72);
            for mut shape in shapes {
                shape.translate(delta);
                painter.add(shape);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            ui.ctx().request_repaint();
        }
    }
    fn layer_footer(&mut self, ui: &mut egui::Ui, icon: &Value) {
        use crate::layer_toolbar::{ACTION_COUNT, Action};
        let height = ui.available_height().min(crate::layer_toolbar::HEIGHT);
        // Keep the group at the left edge. Narrow stacks share their width
        // evenly; glyphs remain square and centered in full-height bodies.
        let width = (ui.available_width() / ACTION_COUNT as f32).min(height);
        let size = Vec2::new(width, height);
        crate::buttons::bar_with_height(ui, height, |ui| {
            let create = Action::Create.button(ui, size, true);
            crate::buttons::menu_popup(ui, &create, |ui| {
                if let Some(kind) = layer_type_menu(ui) {
                    self.layer_palette.pending = Some(LayerAction::Add(kind));
                }
            });
            if Action::Folder.button(ui, size, true).clicked() {
                self.layer_palette.pending = Some(LayerAction::Add("Folder"));
            }
            let selected = self.selected_layer.is_some();
            for (symbol, action) in [
                (Action::Up, LayerAction::Move(1)),
                (Action::Down, LayerAction::Move(-1)),
                (Action::Delete, LayerAction::Delete),
            ] {
                if symbol.button(ui, size, selected).clicked() {
                    self.layer_palette.pending = Some(action);
                }
            }
            if Action::Import
                .button(ui, size, self.view != crate::desktop::View::Style)
                .clicked()
                && let Some(achievement) = self.selection.first()
            {
                self.request(UiRequest::ImportLayerImage {
                    achievement_id: achievement.clone(),
                    layer_id: None,
                });
            }
            let background = Action::Background.button(ui, size, true);
            crate::buttons::menu_popup(ui, &background, |ui| {
                let mut value = icon.clone();
                crate::layers::color(ui, "Color", &mut value, "backgroundColorHex");
                if value != *icon {
                    self.layer_palette.background = Some(value["backgroundColorHex"].clone());
                }
            });
            let random_layers = icon["layers"].as_array().is_some_and(|layers| {
                layers.iter().any(|layer| {
                    random_layer_settings(layer["kind"].as_str().unwrap_or("")).is_some()
                })
            });
            if Action::Reroll.button(ui, size, random_layers).clicked() {
                self.layer_palette.pending = Some(LayerAction::Reroll);
            }
        });
    }
    pub(crate) fn layer_action(&mut self, action: LayerAction, icon: &mut Value) {
        self.layer_palette.edit_label = Some(match &action {
            LayerAction::Add(_) => "Add icon layer",
            LayerAction::AddMask { .. } => "Add layer mask",
            LayerAction::ReferenceMask { .. } => "Reference layer mask",
            LayerAction::InvertMask { .. } => "Invert layer mask",
            LayerAction::GrayscaleMask { .. } => "Use grayscale mask",
            LayerAction::Delete => "Delete icon layer",
            LayerAction::Duplicate => "Clone icon layer",
            LayerAction::Copy => "Copy icon layer",
            LayerAction::Paste => "Paste icon layer",
            LayerAction::Reroll => "Re-roll all random layers",
            LayerAction::Move(_) => "Reorder icon layers",
            LayerAction::MoveInto(_) => "Group icon layer",
            LayerAction::RemoveFromFolder => "Remove layer from folder",
            LayerAction::RemoveMask {
                delete_source: true,
                ..
            } => "Delete mask",
            LayerAction::RemoveMask {
                delete_source: false,
                ..
            } => "Remove mask",
        });
        let Some(layers) = icon["layers"].as_array_mut() else {
            return;
        };
        let selected = self.selected_layer.clone();
        let current = selected
            .as_deref()
            .and_then(|id| layers.iter().find(|l| l["id"].as_str() == Some(id)))
            .cloned();
        let parent = current
            .as_ref()
            .and_then(|l| {
                if l["kind"] == "Folder" {
                    l["id"].as_str()
                } else {
                    l["parentFolderId"].as_str()
                }
            })
            .map(str::to_owned);
        match action {
            LayerAction::Reroll => {
                reroll_random_layers(layers);
                return;
            }
            LayerAction::AddMask { owner, kind } => {
                if !achimeld_application::masks::is_source(layers, &owner)
                    && let Some(index) = layers.iter().position(|l| l["id"] == owner)
                {
                    let mask = crate::layer_masks::new_mask(kind);
                    layers[index]["maskLayerId"] = mask["id"].clone();
                    layers[index]["maskMode"] = json!("Luminance");
                    layers[index]["maskInvert"] = json!(false);
                    self.selected_layer = mask["id"].as_str().map(str::to_owned);
                    layers.push(mask);
                }
            }
            LayerAction::ReferenceMask { owner, source } => {
                if achimeld_application::masks::is_source(layers, &owner)
                    || achimeld_application::masks::is_source(layers, &source)
                    || blocked_mask_sources(layers, &owner).contains(&source)
                    || !layers
                        .iter()
                        .any(|l| l["id"] == source && l["maskLayerId"].is_string())
                {
                    return;
                }
                if let Some(index) = layers.iter().position(|l| l["id"] == owner) {
                    let mut mask = crate::layer_masks::new_mask("MaskReference");
                    mask["maskSourceLayerId"] = json!(source);
                    layers[index]["maskLayerId"] = mask["id"].clone();
                    layers[index]["maskMode"] = json!("Luminance");
                    layers[index]["maskInvert"] = json!(false);
                    self.selected_layer = mask["id"].as_str().map(str::to_owned);
                    layers.push(mask);
                }
            }
            LayerAction::InvertMask { owner } => {
                if let Some(layer) = layers
                    .iter_mut()
                    .find(|l| l["id"] == owner && l["maskLayerId"].is_string())
                {
                    layer["maskInvert"] = json!(!layer["maskInvert"].as_bool().unwrap_or(false));
                }
            }
            LayerAction::GrayscaleMask { owner } => {
                if let Some(layer) = layers
                    .iter_mut()
                    .find(|l| l["id"] == owner && l["maskLayerId"].is_string())
                {
                    layer["maskMode"] = json!("Luminance");
                }
            }
            LayerAction::Add(kind) => {
                let mut layer = new_layer(kind);
                layer["parentFolderId"] = parent.map_or(Value::Null, |id| json!(id));
                self.selected_layer = layer["id"].as_str().map(str::to_owned);
                layers.push(layer);
            }
            LayerAction::Delete => {
                if let Some(id) = selected {
                    remove_layer(layers, &id);
                    self.selected_layer = stack(layers, &self.layer_palette.collapsed)
                        .first()
                        .map(|(id, _)| id.clone());
                }
            }
            LayerAction::Copy => {
                if let Some(id) = selected {
                    self.layer_palette.clipboard = Some(copy_bundle(layers, &id));
                    self.layer_palette.clipboard_root = Some(id);
                }
            }
            LayerAction::Duplicate => {
                if let Some(id) = selected {
                    let source = copy_bundle(layers, &id);
                    let parent = current.as_ref().and_then(|l| l["parentFolderId"].as_str());
                    let (bundle, id) = fresh_bundle(&source, &id, parent);
                    layers.extend(bundle);
                    self.selected_layer = Some(id);
                }
            }
            LayerAction::Paste => {
                if let Some(source) = &self.layer_palette.clipboard {
                    let root = self.layer_palette.clipboard_root.as_deref().unwrap_or("");
                    let (mut bundle, id) = fresh_bundle(source, root, parent.as_deref());
                    retain_mask_references(&mut bundle, layers);
                    layers.extend(bundle);
                    self.selected_layer = Some(id);
                }
            }
            LayerAction::Move(delta) => {
                if let Some(id) = selected {
                    let parent = current.as_ref().and_then(|l| l["parentFolderId"].as_str());
                    let mut siblings: Vec<_> = layers
                        .iter()
                        .filter(|l| l["parentFolderId"].as_str() == parent)
                        .filter_map(|l| l["id"].as_str().map(str::to_owned))
                        .collect();
                    if let Some(index) = siblings.iter().position(|s| s == &id) {
                        let next = index as i32 + delta;
                        if next >= 0 && (next as usize) < siblings.len() {
                            siblings.swap(index, next as usize);
                            let mut ordered = siblings.into_iter();
                            let mut replacements = BTreeMap::new();
                            for l in layers.iter() {
                                replacements
                                    .insert(l["id"].as_str().unwrap_or("").to_owned(), l.clone());
                            }
                            for layer in layers
                                .iter_mut()
                                .filter(|l| l["parentFolderId"].as_str() == parent)
                            {
                                if let Some(id) = ordered.next() {
                                    *layer = replacements.remove(&id).expect("sibling exists");
                                }
                            }
                        }
                    }
                }
            }
            LayerAction::MoveInto(folder) => {
                if let Some(id) = selected {
                    let forbidden = descendants(layers, &id);
                    if folder.as_ref().is_some_and(|id| forbidden.contains(id)) {
                        return;
                    }
                    if let Some(layer) = layers.iter_mut().find(|l| l["id"].as_str() == Some(&id)) {
                        layer["parentFolderId"] = folder.map_or(Value::Null, |id| json!(id));
                    }
                }
            }
            LayerAction::RemoveFromFolder => {
                if let Some(id) = selected
                    && let Some(parent) =
                        current.as_ref().and_then(|l| l["parentFolderId"].as_str())
                {
                    let grandparent = layers
                        .iter()
                        .find(|l| l["id"] == parent)
                        .map(|l| l["parentFolderId"].clone())
                        .unwrap_or(Value::Null);
                    if let Some(layer) = layers.iter_mut().find(|l| l["id"] == id) {
                        layer["parentFolderId"] = grandparent;
                    }
                }
            }
            LayerAction::RemoveMask {
                owner,
                delete_source,
            } => {
                if let Some(layer) = layers.iter_mut().find(|l| l["id"] == owner)
                    && let Some(mask) = layer["maskLayerId"].as_str().map(str::to_owned)
                {
                    layer["maskLayerId"] = Value::Null;
                    if delete_source {
                        remove_layer(layers, &mask);
                    }
                    self.selected_layer = Some(owner);
                }
            }
        }
        self.layer_palette.selected_effect = None;
    }
    fn effect_properties(
        &mut self,
        ui: &mut egui::Ui,
        port: &mut dyn EditorPort,
        icon: &mut Value,
    ) {
        if icon["layers"].as_array().is_some_and(|layers| {
            self.selected_layer
                .as_deref()
                .is_some_and(|id| achimeld_application::masks::is_source(layers, id))
        }) {
            self.layer_palette.selected_effect = None;
            return;
        }
        let Some(layer) = icon["layers"].as_array_mut().and_then(|layers| {
            layers
                .iter_mut()
                .find(|l| l["id"].as_str() == self.selected_layer.as_deref())
        }) else {
            return;
        };
        let Some(effect) = layer["effects"].as_array_mut().and_then(|effects| {
            effects
                .iter_mut()
                .find(|e| e["id"].as_str() == self.layer_palette.selected_effect.as_deref())
        }) else {
            self.layer_palette.selected_effect = None;
            return;
        };
        let operation = crate::effect_diagnostics::opening(ui, "open effect properties", effect);
        let stored_effect = effect;
        let mut effective = stored_effect.clone();
        let grayscale = crate::color_picker::grayscale_scope(ui);
        if grayscale {
            achimeld_application::masks::grayscale_colors(&mut effective);
        }
        let initial = effective.clone();
        let effect = &mut effective;
        ui.push_id(
            (
                "effect-opacity",
                &self.selected_layer,
                &self.layer_palette.selected_effect,
            ),
            |ui| {
                crate::layers::effect_blending(ui, effect);
            },
        );
        crate::layers::property_scroll_area(ui)
            .id_salt((
                "layer-property-scroll",
                &self.selected_layer,
                &self.layer_palette.selected_effect,
            ))
            .show(ui, |ui| {
                crate::layers::effect_property_fields(ui, effect);
                if effect["kind"] == "GradientOverlay" {
                    crate::layers::gradient_presets(ui, port, &mut effect["gradient"]);
                }
            });
        if *effect != initial {
            if grayscale {
                achimeld_application::masks::grayscale_colors(effect);
            }
            *stored_effect = effect.clone();
        }
        crate::effect_diagnostics::completed(operation);
    }
}
fn layer_summary(layer: &Value) -> String {
    let kind = layer["kind"].as_str().unwrap_or("Layer");
    match kind {
        "AtlasTile" => format!(
            "{} · #{} · {}×",
            label(kind),
            layer["tileIndex"].as_u64().unwrap_or(0),
            layer["scale"].as_f64().unwrap_or(1.0)
        ),
        "Text" | "Text3d" => format!(
            "{} · {}",
            label(kind),
            layer["text"]["content"].as_str().unwrap_or("")
        ),
        "Shape" => format!(
            "{} · {}",
            label(kind),
            layer["shape"]["kind"].as_str().unwrap_or("Rectangle")
        ),
        _ => label(kind).to_owned(),
    }
}

fn paint_menu_icon(ui: &egui::Ui, response: &egui::Response, icon: Icon) {
    let (_, ink) = crate::buttons::colors(ui, response, crate::buttons::ButtonStyle::Quiet);
    icons::paint(
        ui.painter(),
        Rect::from_min_size(
            Pos2::new(response.rect.left() + 4.0, response.rect.center().y - 8.0),
            Vec2::splat(16.0),
        ),
        icon,
        ink,
    );
}

fn row_opacity(ui: &mut egui::Ui, layer: &mut Value, slider: bool, width: f32) {
    let mut opacity = layer["opacity"].as_f64().unwrap_or(1.0) * 100.0;
    let mut changed = false;
    if slider {
        changed = crate::opacity::slider(ui, &mut opacity, width).changed();
    } else {
        let response = crate::buttons::menu(
            ui,
            crate::buttons::Button::new("").min_size(Vec2::splat(24.0)),
            |ui| {
                ui.horizontal(|ui| {
                    changed |= crate::opacity::slider(ui, &mut opacity, 120.0).changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut opacity)
                                .range(0.0..=100.0)
                                .suffix("%"),
                        )
                        .changed();
                });
            },
        )
        .response
        .on_hover_text(format!("Opacity: {opacity:.0}%"));
        paint_menu_icon(ui, &response, Icon::Opacity);
    }
    if changed {
        layer["opacity"] = json!(opacity / 100.0);
    }
}

impl Desktop {
    fn row_blend(
        &mut self,
        ui: &mut egui::Ui,
        port: &dyn EditorPort,
        achievement: &str,
        layer: &mut Value,
        expanded: bool,
        width: f32,
    ) {
        let control_id = layer_control_id(achievement, layer["id"].as_str().unwrap_or(""), "blend");
        let mode = layer["blendMode"].as_str().unwrap_or("Normal");
        let button = crate::blend_preview::button(ui, mode, width, expanded).id(control_id);
        self.layer_blend_menu(ui, port, achievement, layer, button)
            .on_hover_text("Blend mode");
    }
}

pub(crate) fn mask_type_menu(ui: &mut egui::Ui) -> Option<&'static str> {
    let mut selected = None;
    for kind in ["Folder", "MaskReference"] {
        if let Some(texture) = crate::layer_icons::texture(ui.ctx(), kind) {
            let text = crate::localization::text(ui.ctx(), label(kind));
            if ui
                .add(
                    crate::buttons::Button::new((
                        egui::Image::new((texture, Vec2::splat(20.0))),
                        text,
                    ))
                    .bar()
                    .align(egui::Align2::LEFT_CENTER)
                    .min_size(Vec2::new(ui.available_width(), 28.0))
                    .image_tint_follows_text_color(true),
                )
                .clicked()
            {
                selected = Some(kind);
                ui.close();
            }
        }
    }
    layer_type_menu(ui).or(selected)
}

pub(crate) fn layer_type_menu(ui: &mut egui::Ui) -> Option<&'static str> {
    let column_width = crate::layer_icons::LAYERS
        .iter()
        .chain(crate::layer_icons::ADJUSTMENTS.iter())
        .map(|kind| {
            ui.painter()
                .layout_no_wrap(
                    crate::localization::text(ui.ctx(), label(kind)),
                    egui::TextStyle::Button.resolve(ui.style()),
                    ui.visuals().text_color(),
                )
                .size()
                .x
                + 44.0
        })
        .fold(224.0_f32, f32::max);
    let width = (2.0 * column_width).min((ui.ctx().content_rect().width() - 16.0).max(0.0));
    ui.set_width(width);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
    ui.spacing_mut().interact_size.y = 28.0;
    egui::Frame::new()
        .fill(ui.visuals().panel_fill)
        .show(ui, |ui| {
            let mut selected = None;
            // Keep adjustments beside the artwork list, including when that
            // longer list needs scrolling. Neither column can hide the other.
            ui.columns(2, |columns| {
                for (ui, (heading, kinds)) in columns.iter_mut().zip([
                    ("Layers", crate::layer_icons::LAYERS.as_slice()),
                    ("Adjustments", crate::layer_icons::ADJUSTMENTS.as_slice()),
                ]) {
                    ui.add_sized(
                        [ui.available_width(), 24.0],
                        egui::Label::new(
                            egui::RichText::new(crate::localization::text(ui.ctx(), heading))
                                .small()
                                .weak(),
                        )
                        .halign(egui::Align::LEFT)
                        .truncate(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt(("layer-types", heading))
                        .max_height((ui.ctx().content_rect().height() - 104.0).max(28.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            for &kind in kinds {
                                let texture = crate::layer_icons::texture(ui.ctx(), kind)
                                    .expect("Every selectable layer kind has an embedded symbol");
                                let text = crate::localization::text(ui.ctx(), label(kind));
                                let response = ui
                                    .push_id(kind, |ui| {
                                        ui.add(
                                            crate::buttons::Button::new((
                                                egui::Image::new((texture, Vec2::splat(20.0))),
                                                text.clone(),
                                            ))
                                            .bar()
                                            .align(egui::Align2::LEFT_CENTER)
                                            .wrap_mode(egui::TextWrapMode::Truncate)
                                            .min_size(Vec2::new(ui.available_width(), 28.0))
                                            .icon_spacing(8.0)
                                            .image_tint_follows_text_color(true),
                                        )
                                    })
                                    .inner
                                    .on_hover_text(text);
                                if response.clicked() {
                                    selected = Some(kind);
                                    ui.close();
                                }
                            }
                        });
                }
            });
            selected
        })
        .inner
}

pub(crate) fn effect_menu(ui: &mut egui::Ui, layer: &mut Value) -> Option<String> {
    let operation = crate::effect_diagnostics::opening(ui, "open effect menu", layer);
    let mut added = None;
    for kind in [
        "GradientOverlay",
        "ColorOverlay",
        "Outline",
        "Shadow",
        "InnerLight",
        "OuterLight",
        "BoxBlur",
        "Skew",
        "Perspective",
        "Pinch",
        "LensBlur",
    ] {
        if ui
            .add(
                buttons::Button::new((
                    crate::effect_icons::image(ui.ctx(), kind, 16.0),
                    crate::localization::text(ui.ctx(), label(kind)),
                ))
                .image_tint_follows_text_color(true)
                .bar(),
            )
            .clicked()
        {
            achimeld_application::diagnostics::record(
                "INFO",
                "effects",
                &format!("Add effect requested: {kind}"),
            );
            if !layer["effects"].is_array() {
                layer["effects"] = json!([]);
            }
            let id = new_id();
            added = Some(id.clone());
            let mut effect = json!({"id":id,"kind":kind,"visible":true,"colorHex":"#FF000000","opacity":1.0,"width":2.0,"blur":2.0,"placement":"Center","gradient":{"kind":"Linear","angle":90.0,"stops":[{"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]}});
            match kind {
                "InnerLight" => {
                    effect["colorHex"] = json!("#FFFFFFFF");
                    effect["blendMode"] = json!("Screen");
                    effect["offsetX"] = json!(0.0);
                    effect["offsetY"] = json!(8.0);
                    effect["blur"] = json!(12.0);
                }
                "OuterLight" => {
                    effect["colorHex"] = json!("#FFFFFFFF");
                    effect["blendMode"] = json!("Screen");
                    effect["width"] = json!(4.0);
                    effect["blur"] = json!(12.0);
                }
                "Skew" => {
                    effect["skewX"] = json!(0.0);
                    effect["skewY"] = json!(0.0);
                }
                "Perspective" => {
                    effect["perspectiveX"] = json!(0.0);
                    effect["perspectiveY"] = json!(0.0);
                }
                "Pinch" => effect["amount"] = json!(0.5),
                "LensBlur" => {
                    effect["blur"] = json!(4.0);
                    effect["aberration"] = json!(8.0);
                }
                _ => {}
            }
            layer["effects"]
                .as_array_mut()
                .expect("effects initialized")
                .push(effect);
            achimeld_application::diagnostics::record(
                "INFO",
                "effects",
                &format!("Effect added to layer draft: {kind}"),
            );
            ui.close();
        }
    }
    crate::effect_diagnostics::completed(operation);
    added
}
pub(crate) fn new_layer(kind: &str) -> Value {
    let mut layer = json!({"id":new_id(),"displayName":label(kind),"kind":kind,"visible":true,"opacity":1.0,"blendMode":"Normal","scale":1.0,"rotation":0.0,"offsetX":0.0,"offsetY":0.0,"tileIndex":0,"effects":[],"solidColorHex":"#FFFFFFFF","text":{"content":"Text","fontFamily":"Segoe UI","fontSize":128.0,"colorHex":"#FFFFFFFF","alignment":"Center"},"shape":{"kind":"Rectangle","colorHex":"#FFFFFFFF","scaleX":0.5,"scaleY":0.5,"vertexCount":5,"innerRadius":0.5},"curves":{"points":[{"input":0.0,"output":0.0},{"input":1.0,"output":1.0}]},"gradient":{"kind":"Linear","angle":90.0,"stops":[{"offset":0.0,"colorHex":"#FF000000"},{"offset":1.0,"colorHex":"#FFFFFFFF"}]},"adjustment":{}});
    if kind == "Artifact3d" {
        layer["artifact3d"] = json!(achimeld_application::artifact3d::ArtifactSettings::default());
    }
    if kind == "Text3d" {
        layer["text3d"] =
            serde_json::to_value(achimeld_application::text3d::Text3dSettings::default())
                .expect("Finite 3D text defaults");
    }
    if kind == "PerlinNoise" {
        layer["perlinNoise"] = crate::perlin::defaults();
    }
    if kind == "Particle" {
        let settings = achimeld_application::particles::ParticleSettings {
            seed: u32::from_str_radix(&new_id()[..8], 16).unwrap_or(1),
            color_map: Some(Default::default()),
            ..Default::default()
        };
        layer["particles"] = json!(settings);
    }
    if kind == "Explosion" {
        layer["explosion"] = json!(achimeld_application::explosion::ExplosionSettings {
            seed: u32::from_str_radix(&new_id()[..8], 16).unwrap_or(1),
            ..achimeld_application::explosion::ExplosionSettings::new_deltoid()
        });
    }
    layer
}
fn layer_drop_parent(
    layers: &[Value],
    selected: &str,
    order: &[String],
    rects: &BTreeMap<String, Rect>,
    pointer: Pos2,
) -> Option<String> {
    let current = layers.iter().find(|l| l["id"] == selected)?;
    let original = current["parentFolderId"].as_str().map(str::to_owned);
    let first = order.first().and_then(|id| rects.get(id))?;
    let last = order.last().and_then(|id| rects.get(id))?;
    // Above and below the whole tree are explicit root-level drop targets.
    if pointer.y < first.top() || pointer.y > last.bottom() {
        return None;
    }
    let forbidden = descendants(layers, selected);
    let target = order.iter().find(|id| {
        rects
            .get(*id)
            .is_some_and(|rect| pointer.y <= rect.bottom())
    });
    if let Some(target) = target
        && !forbidden.contains(target)
        && let Some(layer) = layers.iter().find(|l| l["id"] == *target)
    {
        let parent = layer["parentFolderId"].as_str().map(str::to_owned);
        if parent.as_ref().is_none_or(|id| !forbidden.contains(id)) {
            return parent;
        }
    }
    original
}

fn reorder_visual(layers: &mut [Value], selected: &str, order: &[String]) {
    let Some(layer) = layers.iter().find(|l| l["id"].as_str() == Some(selected)) else {
        return;
    };
    let parent = layer["parentFolderId"].as_str().map(str::to_owned);
    let siblings: BTreeSet<_> = layers
        .iter()
        .filter(|l| l["parentFolderId"].as_str() == parent.as_deref())
        .filter_map(|l| l["id"].as_str().map(str::to_owned))
        .collect();
    let mut ordered = order.iter().rev().filter(|id| siblings.contains(*id));
    let original: BTreeMap<_, _> = layers
        .iter()
        .filter_map(|l| Some((l["id"].as_str()?.to_owned(), l.clone())))
        .collect();
    for layer in layers.iter_mut().filter(|l| {
        l["parentFolderId"].as_str() == parent.as_deref()
            && l["id"]
                .as_str()
                .is_some_and(|id| order.iter().any(|s| s == id))
    }) {
        if let Some(id) = ordered.next() {
            *layer = original[id].clone();
        }
    }
}
