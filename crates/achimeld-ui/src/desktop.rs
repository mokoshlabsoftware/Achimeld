use crate::buttons::UiButtonExt as _;
use crate::theme;
use achimeld_application::{
    Command, EditorPort, EntityKind, Preferences, ReorderGesture, UiRequest,
};
use egui::Vec2;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn mipmap_bytes([mut width, mut height]: [usize; 2]) -> usize {
    let mut bytes = 0;
    loop {
        bytes += width * height * 4;
        if width <= 1 && height <= 1 {
            return bytes;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
}

pub const MIN_WINDOW_INNER_SIZE: [f32; 2] = [720.0, 650.0];
const TOOLBAR_FILTER_PREFERRED: f32 = 210.0;
const TOOLBAR_FILTER_FLOOR: f32 = 96.0;
/// egui's default separator spacing. The toolbar zeroes item spacing, and the
/// separator still reserves this width.
const TOOLBAR_SEPARATOR: f32 = 6.0;

pub(crate) fn toolbar_tab<T: PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    choice: T,
    label: &str,
    image: (egui::TextureId, Vec2),
) -> egui::Response {
    toolbar_tab_attention(ui, value, choice, label, image, false, true)
}

fn toolbar_tab_attention<T: PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    choice: T,
    label: &str,
    image: (egui::TextureId, Vec2),
    wants_click: bool,
    show_label: bool,
) -> egui::Response {
    let selected = *value == choice;
    let button = if show_label {
        crate::buttons::Button::selectable(selected, (egui::Image::new(image), label))
    } else {
        crate::buttons::Button::selectable(selected, egui::Image::new(image).alt_text(label))
    };
    // Padding is identical in all states; animation never changes the layout.
    let mut response = ui.add(
        button
            .bar()
            .wants_click(wants_click)
            .icon_spacing(5.0)
            .image_tint_follows_text_color(true)
            .wrap_mode(egui::TextWrapMode::Extend),
    );
    if !show_label {
        response = response.on_hover_text(label);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
        });
    }
    if response.clicked() && !selected {
        *value = choice;
        response.mark_changed();
    }
    response
}

fn toolbar_text_width(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let font = style.resolve(ui.style());
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
            .rect
            .width()
    })
}

fn toolbar_button_width(ui: &egui::Ui, icon: f32, label: Option<&str>) -> f32 {
    let text = label.map_or(0.0, |label| {
        5.0 + toolbar_text_width(ui, label, egui::TextStyle::Button)
    });
    ui.spacing().button_padding.x * 2.0 + icon + text
}

fn toolbar_text_button_width(ui: &egui::Ui, label: &str) -> f32 {
    ui.spacing().button_padding.x * 2.0 + toolbar_text_width(ui, label, egui::TextStyle::Button)
}

fn filter_leading_space(ui: &egui::Ui) -> f32 {
    let margin = egui::Margin::symmetric(4, 2);
    let pixels_per_point = ui.ctx().pixels_per_point();
    let text_height =
        (ui.text_style_height(&egui::TextStyle::Body) * pixels_per_point).ceil() / pixels_per_point;
    let filter_height = text_height + ui.spacing().extra_text_line_spacing + margin.sum().y;
    let inset = (ui.spacing().interact_size.y - filter_height) * 0.5;
    (inset - TOOLBAR_SEPARATOR * 0.5).max(0.0)
}

struct ToolbarPlan {
    icons_only: bool,
    filter_width: f32,
    project_width: f32,
    right_width: f32,
}

#[derive(Clone, Copy)]
struct ToolbarContents<'a> {
    show_filter: bool,
    show_sort: bool,
    sync_label: &'a str,
    sync_icon: f32,
    show_stop: bool,
    project: &'a str,
}

/// Shrink the filter before hiding labels. Hide labels before letting the
/// project name or the staged action cover the field.
fn plan_toolbar(ui: &egui::Ui, available: f32, request: ToolbarContents<'_>) -> ToolbarPlan {
    let ToolbarContents {
        show_filter,
        show_sort,
        sync_label,
        sync_icon,
        show_stop,
        project,
    } = request;
    let tabs = |icons_only: bool| {
        [
            "Achievements",
            "Stats",
            "Sync",
            "Steam",
            "Art Data Bank",
            "Style Viewer",
        ]
        .into_iter()
        .map(|label| toolbar_button_width(ui, 20.0, (!icons_only).then_some(label)))
        .sum::<f32>()
    };
    let history = toolbar_button_width(ui, 24.0, None) * 2.0
        + if show_sort {
            toolbar_button_width(ui, 24.0, None)
        } else {
            0.0
        };
    let separators = TOOLBAR_SEPARATOR * (1.0 + f32::from(show_sort) + f32::from(show_filter));
    let filter_lead = if show_filter {
        filter_leading_space(ui)
    } else {
        0.0
    };
    let project_full = toolbar_text_width(ui, project, egui::TextStyle::Body);
    let stop = if show_stop {
        toolbar_text_button_width(ui, "Stop") + 1.0
    } else {
        0.0
    };
    let sync = |icons_only: bool| {
        toolbar_button_width(ui, sync_icon, (!icons_only).then_some(sync_label)) + 1.0
    };
    // One extra point absorbs font and pixel rounding so the next column cannot cross the filter.
    let chrome = |icons_only: bool, filter: f32, project_width: f32| {
        let gap = if project_width > 0.5 { 12.0 } else { 0.0 };
        tabs(icons_only)
            + history
            + separators
            + filter_lead
            + filter
            + 1.0
            + gap
            + project_width
            + stop
            + sync(icons_only)
    };
    let fit = |icons_only: bool| -> (f32, f32) {
        if !show_filter {
            if chrome(icons_only, 0.0, project_full) <= available {
                return (0.0, project_full);
            }
            let project_width = (available - chrome(icons_only, 0.0, 0.0)).clamp(0.0, project_full);
            return (0.0, project_width);
        }
        if chrome(icons_only, TOOLBAR_FILTER_PREFERRED, project_full) <= available {
            return (TOOLBAR_FILTER_PREFERRED, project_full);
        }
        let room = (available - chrome(icons_only, 0.0, 0.0)).max(0.0);
        if room >= TOOLBAR_FILTER_FLOOR + project_full {
            let filter =
                (room - project_full).clamp(TOOLBAR_FILTER_FLOOR, TOOLBAR_FILTER_PREFERRED);
            (filter, project_full)
        } else if room >= TOOLBAR_FILTER_FLOOR {
            (
                TOOLBAR_FILTER_FLOOR,
                (room - TOOLBAR_FILTER_FLOOR).min(project_full),
            )
        } else {
            (room, 0.0)
        }
    };
    let (filter, project_width, icons_only) = if show_filter {
        let (filter, project_width) = fit(false);
        if filter + 0.01 >= TOOLBAR_FILTER_FLOOR && project_width + 0.5 >= project_full {
            (filter, project_width, false)
        } else {
            let (filter, project_width) = fit(true);
            (filter, project_width, true)
        }
    } else if chrome(false, 0.0, project_full) <= available + 0.1 {
        (0.0, project_full, false)
    } else {
        let (_, project_width) = fit(true);
        (0.0, project_width, true)
    };
    let gap = if project_width > 0.5 { 12.0 } else { 0.0 };
    ToolbarPlan {
        icons_only,
        filter_width: filter,
        project_width,
        right_width: (gap + project_width + stop + sync(icons_only)).min(available.max(0.0)),
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum View {
    Achievements,
    Stats,
    Steam,
    Sync,
    Art,
    Style,
}
impl View {
    pub(crate) fn is_editor(self) -> bool {
        matches!(self, Self::Achievements | Self::Stats)
    }
}
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum InspectorTab {
    General,
    Localization,
    Icon,
}
pub(crate) struct NodeDrag {
    pub starts: BTreeMap<String, [i32; 2]>,
    pub pointer_start: egui::Pos2,
    pub delta: Vec2,
}
pub(crate) struct EntityDraft {
    pub base: Value,
    pub value: Value,
    pub error: String,
}

pub(crate) struct WindowSizeDraft {
    width: u32,
    height: u32,
}

pub struct Desktop {
    pub(crate) terminal: crate::terminal::Terminal,
    pub(crate) window_size: Option<WindowSizeDraft>,
    pub(crate) sync_token: String,
    pub(crate) sync_choices: BTreeMap<String, String>,
    pub(crate) sync_filter: String,
    pub(crate) sync_page: usize,
    pub(crate) sync_confirmation: Option<(String, u32, u64)>,
    pub(crate) sync_plan_token: Option<String>,
    pub(crate) sync_plan_fresh: bool,
    pub(crate) steam_marks: BTreeSet<(EntityKind, String)>,
    pub(crate) steam_delete_review: bool,
    pub(crate) steam_delete_fresh: bool,
    pub(crate) sync_app_id_edit: Option<crate::synchronization_target::AppIdEdit>,
    brand_icon: Option<egui::TextureHandle>,
    pub(crate) preference_icons: Option<[egui::TextureHandle; 5]>,
    history_icons: Option<[egui::TextureHandle; 2]>,
    tab_icons: Option<[egui::TextureHandle; 6]>,
    pub(crate) inspector_icons: Option<[egui::TextureHandle; 3]>,
    pub(crate) about: bool,
    pub(crate) help_page: crate::about::HelpPage,
    pub(crate) view: View,
    pub(crate) kind: EntityKind,
    pub(crate) selection: Vec<String>,
    pub(crate) filter: String,
    pub(crate) inspector_tab: InspectorTab,
    pub(crate) inspector_panel: crate::inspector_panel::InspectorPanel,
    pub(crate) layout_reset_requested: bool,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) camera_state: Option<(View, EntityKind, Value)>,
    pub(crate) camera_editing: bool,
    pub(crate) camera_drag: Option<crate::canvas::CameraDrag>,
    pub(crate) camera_press_frame: Option<u64>,
    pub(crate) camera_motion: Option<crate::canvas::CameraMotion>,
    pub(crate) canvas_menu: Option<crate::canvas::CanvasMenu>,
    pub(crate) icon_pan: Option<crate::canvas::IconPan>,
    pub(crate) node_drag: Option<NodeDrag>,
    pub(crate) selection_drag: Option<crate::canvas::SelectionDrag>,
    pub(crate) canvas_focus: Option<crate::canvas::CanvasFocus>,
    pub(crate) layer_drag: Option<ReorderGesture>,
    pub(crate) textures: BTreeMap<String, (String, egui::TextureHandle, u64)>,
    texture_clock: u64,
    pub(crate) texture_bytes: usize,
    pub(crate) settings: bool,
    pub(crate) sort_open: bool,
    pub(crate) gradient_explorer: crate::gradient_explorer::Explorer,
    pub(crate) gradient_map_collection: crate::gradient_map_collection::Collection,
    pub(crate) effect_explorer: crate::effect_explorer::Explorer,
    pub(crate) style_viewer_ui: crate::style_viewer::ViewerUi,
    pub(crate) preferences: Option<Preferences>,
    pub(crate) preference_base: Option<Preferences>,
    pub(crate) pref_category: usize,
    pub(crate) project_name: String,
    pub(crate) app_id: u32,
    pub(crate) atlas: Value,
    pub(crate) settings_revision: u64,
    pub(crate) error: Option<String>,
    pub(crate) selected_layer: Option<String>,
    pub(crate) blend_hover: Option<crate::blend_preview::BlendHover>,
    pub(crate) entity_drafts: BTreeMap<String, EntityDraft>,
    pub(crate) localization_drafts: BTreeMap<(String, String), (String, String)>,
    pub(crate) requests: Vec<UiRequest>,
    pub(crate) reordered_layers: Option<Vec<String>>,
    pub(crate) recipe_clipboard: Option<Value>,
    pub(crate) layer_palette: crate::layer_palette::LayerPalette,
    pub(crate) source_picker: Option<crate::source_picker::SourcePicker>,
    pub(crate) style_rename: Option<crate::style_picker::StyleRename>,
    pub(crate) style_edit_prompt: Option<crate::style_picker::StyleEditPrompt>,
    pub(crate) icon_style_choices: BTreeMap<String, String>,
    project_id: Option<String>,
    pub(crate) focused_control: Option<egui::Id>,
    pub(crate) art_bank: crate::art_bank::ArtBank,
    pub(crate) general_images: crate::general_images::GeneralImages,
    pub(crate) manual_stat_link: Option<String>,
    pub(crate) image_drop: crate::image_drop::ImageDrop,
}
impl Default for Desktop {
    fn default() -> Self {
        Self {
            terminal: Default::default(),
            window_size: None,
            sync_token: String::new(),
            sync_choices: BTreeMap::new(),
            sync_filter: String::new(),
            sync_page: 0,
            sync_confirmation: None,
            sync_plan_token: None,
            sync_plan_fresh: false,
            steam_marks: BTreeSet::new(),
            steam_delete_review: false,
            steam_delete_fresh: false,
            sync_app_id_edit: None,
            brand_icon: None,
            preference_icons: None,
            history_icons: None,
            tab_icons: None,
            inspector_icons: None,
            about: false,
            help_page: crate::about::HelpPage::About,
            view: View::Achievements,
            kind: EntityKind::Achievement,
            selection: Vec::new(),
            filter: String::new(),
            inspector_tab: InspectorTab::General,
            inspector_panel: Default::default(),
            layout_reset_requested: false,
            zoom: 1.0,
            pan: Vec2::new(30.0, 30.0),
            camera_state: None,
            camera_editing: false,
            camera_drag: None,
            camera_press_frame: None,
            camera_motion: None,
            canvas_menu: None,
            icon_pan: None,
            node_drag: None,
            selection_drag: None,
            canvas_focus: None,
            layer_drag: None,
            textures: BTreeMap::new(),
            texture_clock: 0,
            texture_bytes: 0,
            settings: false,
            sort_open: false,
            gradient_explorer: Default::default(),
            gradient_map_collection: Default::default(),
            effect_explorer: Default::default(),
            style_viewer_ui: Default::default(),
            preferences: None,
            preference_base: None,
            pref_category: 1,
            project_name: String::new(),
            app_id: 0,
            atlas: Value::Null,
            settings_revision: 0,
            error: None,
            selected_layer: None,
            blend_hover: None,
            entity_drafts: BTreeMap::new(),
            localization_drafts: BTreeMap::new(),
            requests: Vec::new(),
            reordered_layers: None,
            recipe_clipboard: None,
            layer_palette: Default::default(),
            source_picker: None,
            style_rename: None,
            style_edit_prompt: None,
            icon_style_choices: BTreeMap::new(),
            project_id: None,
            focused_control: None,
            art_bank: Default::default(),
            general_images: Default::default(),
            manual_stat_link: None,
            image_drop: Default::default(),
        }
    }
}
impl Desktop {
    fn window_size_panel(&mut self, ctx: &egui::Context, enabled: bool) {
        let Some(draft) = &mut self.window_size else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(crate::localization::text(ctx, "Set window size"))
            .id(egui::Id::new("set-window-size"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .enabled(enabled)
            .default_width(300.0)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(0))
            .show(ctx, |ui| {
                egui::Frame::new().inner_margin(12).show(ui, |ui| {
                    ui.label(crate::localization::text(
                        ctx,
                        "Window size in logical pixels",
                    ));
                    crate::fields::row(ui, &crate::localization::text(ctx, "Width"), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut draft.width)
                                .range(MIN_WINDOW_INNER_SIZE[0] as u32..=8192),
                        );
                    });
                    crate::fields::row(ui, &crate::localization::text(ctx, "Height"), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut draft.height)
                                .range(MIN_WINDOW_INNER_SIZE[1] as u32..=8192),
                        );
                    });
                });
                crate::buttons::bar(ui, 0.0, |ui| {
                    let width = ui.available_width() / 2.0;
                    let height = ui.spacing().interact_size.y;
                    if ui
                        .add_sized(
                            [width, height],
                            crate::buttons::Button::new(crate::localization::text(ctx, "Apply"))
                                .bar(),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui
                        .add_sized(
                            [ui.available_width(), height],
                            crate::buttons::Button::new(crate::localization::text(ctx, "Cancel"))
                                .bar(),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });
        if enabled && apply {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                draft.width as f32,
                draft.height as f32,
            )));
        }
        if !open || cancel || apply || (enabled && ctx.input(|i| i.key_pressed(egui::Key::Escape)))
        {
            self.window_size = None;
        }
    }

    /// Read-only navigation exposed to the supervised editor automation host.
    pub fn automation_scene(&self) -> Value {
        serde_json::json!({"activeCanvas":match self.view { View::Achievements=>"Achievements",View::Stats=>"Stats",_=>"Other" },"selection":self.selection,"filter":self.filter,"camera":{"zoom":self.zoom,"offsetX":self.pan.x,"offsetY":self.pan.y}})
    }
    pub fn automation_busy(&self) -> bool {
        self.about
            || self.style_rename.is_some()
            || self.style_edit_prompt.is_some()
            || self.view == View::Style
            || self.image_drop.pending()
            || self.art_bank.pending()
            || self.settings
            || self.sync_app_id_edit.is_some()
            || self.sort_open
            || self.gradient_explorer.open
            || self.effect_explorer.open
            || self.preferences.is_some()
            || self.source_picker.is_some()
            || !self.entity_drafts.is_empty()
            || !self.localization_drafts.is_empty()
            || self.node_drag.is_some()
            || self.selection_drag.is_some()
            || self.layer_drag.is_some()
            || self.layer_palette.effect_drag.is_some()
            || self.camera_editing
            || self.camera_drag.is_some()
            || self.camera_motion.is_some()
            || self.canvas_menu.is_some()
            || self.icon_pan.is_some()
    }
    pub fn automation_select(&mut self, kind: EntityKind, id: String, port: &mut dyn EditorPort) {
        self.view = if kind == EntityKind::Achievement {
            View::Achievements
        } else {
            View::Stats
        };
        self.kind = kind;
        self.selection = vec![id];
        self.filter.clear();
        self.camera_state = None;
        self.layer_palette.active_style = None;
        // Include the destination canvas camera in the returned automation revision.
        self.sync_canvas_camera(port);
    }
    pub(crate) fn picker_project_id(&self) -> String {
        self.project_id.clone().unwrap_or_default()
    }
    pub fn show(&mut self, root: &mut egui::Ui, port: &mut dyn EditorPort) {
        self.canvas_focus = None;
        self.image_drop.zones.clear();
        let project_id = port.editor().document().project()["projectId"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        if self
            .project_id
            .as_ref()
            .is_some_and(|old| old != &project_id)
        {
            let clipboard = self.recipe_clipboard.take();
            let mut palette = std::mem::take(&mut self.layer_palette);
            palette.achievement = None;
            palette.pending = None;
            palette.selected_effect = None;
            palette.active_style = None;
            palette.keyboard_active = false;
            let about = self.about;
            let help_page = self.help_page;
            let mut inspector_panel = std::mem::take(&mut self.inspector_panel);
            inspector_panel.cancel_drag();
            let viewer_active = self.view == View::Style;
            let terminal = std::mem::take(&mut self.terminal);
            let window_size = self.window_size.take();
            *self = Self::default();
            self.terminal = terminal;
            self.window_size = window_size;
            self.recipe_clipboard = clipboard;
            self.layer_palette = palette;
            self.about = about;
            self.help_page = help_page;
            self.inspector_panel = inspector_panel;
            if viewer_active {
                self.view = View::Style;
            }
        }
        self.project_id = Some(project_id);
        if std::mem::take(&mut self.layout_reset_requested) {
            self.reset_window_layout(root.ctx(), port);
        }
        self.sync_blend_preview(root.ctx(), port, root.is_enabled());
        if let Some((id, revision)) = port.image_import_selection()
            && revision == port.editor().revision()
            && self.image_drop.selected_revision != Some(revision)
        {
            self.image_drop.selected_revision = Some(revision);
            self.view = View::Achievements;
            self.kind = EntityKind::Achievement;
            self.selection = vec![id];
            self.filter.clear();
            self.inspector_tab = InspectorTab::General;
        }
        let ctx = root.ctx().clone();
        if !self.view.is_editor() {
            self.canvas_menu = None;
        }
        let window_rect = root.max_rect();
        let focused = ctx.memory(|memory| memory.focused());
        if ctx.input(|i| i.focused) && focused != self.focused_control {
            port.finish_group();
            self.focused_control = focused;
        }
        self.entity_drafts.retain(|id, _| {
            port.editor()
                .document()
                .entity(EntityKind::Achievement, id)
                .is_some()
                || port
                    .editor()
                    .document()
                    .entity(EntityKind::Stat, id)
                    .is_some()
        });
        self.localization_drafts
            .retain(|(id, _), _| port.editor().document().catalog().rows.contains_key(id));
        let preferences = self
            .preferences
            .as_ref()
            .unwrap_or(port.preferences())
            .clone();
        crate::localization::language(&ctx, &preferences.language);
        theme::apply(&ctx, &preferences);
        root.set_style(ctx.global_style());
        let brand_icon = self.brand_icon.get_or_insert_with(|| {
            ctx.load_texture(
                "achimeld-title-brand-icon",
                crate::chrome::title_brand_image(),
                egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
            )
        });
        let brand_image = (brand_icon.id(), egui::vec2(18.0, 18.0));
        let history_icons = self.history_icons.get_or_insert_with(|| {
            [
                (
                    "undo",
                    include_bytes!("../assets/toolbar/undo.rgba").as_slice(),
                ),
                (
                    "redo",
                    include_bytes!("../assets/toolbar/redo.rgba").as_slice(),
                ),
            ]
            .map(|(name, bytes)| {
                ctx.load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR,
                )
            })
        });
        let history_images = history_icons
            .each_ref()
            .map(|icon| (icon.id(), egui::vec2(24.0, 24.0)));
        let tab_icons = self.tab_icons.get_or_insert_with(|| {
            [
                (
                    "tab-achievements",
                    include_bytes!("../assets/toolbar/tab-achievements.rgba").as_slice(),
                ),
                (
                    "tab-stats",
                    include_bytes!("../assets/toolbar/tab-stats.rgba").as_slice(),
                ),
                (
                    "tab-sync",
                    include_bytes!("../assets/toolbar/tab-sync.rgba").as_slice(),
                ),
                (
                    "tab-steam",
                    include_bytes!("../assets/toolbar/tab-steam.rgba").as_slice(),
                ),
                (
                    "tab-art-bank",
                    include_bytes!("../assets/toolbar/tab-art-bank.rgba").as_slice(),
                ),
                (
                    "tab-style-viewer",
                    include_bytes!("../assets/toolbar/tab-style-viewer.rgba").as_slice(),
                ),
            ]
            .map(|(name, bytes)| {
                ctx.load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied([64, 64], bytes),
                    egui::TextureOptions::LINEAR
                        .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                )
            })
        });
        let tab_images = tab_icons
            .each_ref()
            .map(|icon| (icon.id(), Vec2::splat(20.0)));
        if root.is_enabled() {
            self.shortcuts(&ctx, port);
        }
        egui::Panel::top("title-bar")
            .exact_size(24.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::color(&preferences, "Panel"))
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(root, |ui| {
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().button_padding = egui::vec2(12.0, 0.0);
                ui.horizontal_centered(|ui| {
                    let brand = ui
                        .add(
                            egui::Image::new(brand_image)
                                .alt_text("Achimeld")
                                .sense(egui::Sense::click_and_drag()),
                        )
                        .on_hover_text("Achimeld");
                    crate::chrome::drag_window(&brand);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let help = crate::localization::text(ui.ctx(), "Help");
                    let windows = crate::localization::text(ui.ctx(), "Windows");
                    crate::chrome::title_menu_bar(
                        ui,
                        ["File", "Edit", "Tools", &windows, &help],
                        |index, ui| match index {
                            0 => {
                                for (name, request) in [
                                    ("New project", UiRequest::NewProject),
                                    ("Open project…", UiRequest::OpenProject),
                                ] {
                                    if ui
                                        .app_button(crate::localization::text(ui.ctx(), name))
                                        .clicked()
                                    {
                                        self.request(request);
                                        ui.close();
                                    }
                                }
                                ui.app_menu_button(
                                    crate::localization::text(ui.ctx(), "Open Recent"),
                                    |ui| {
                                        self.recent_projects_menu(ui, port);
                                    },
                                );
                                for (name, request) in [
                                    ("Save", UiRequest::SaveProject),
                                    ("Save as…", UiRequest::SaveProjectAs),
                                ] {
                                    if ui
                                        .app_button(crate::localization::text(ui.ctx(), name))
                                        .clicked()
                                    {
                                        self.request(request);
                                        ui.close();
                                    }
                                }
                                if ui
                                    .app_button(crate::localization::text(
                                        ui.ctx(),
                                        "Project settings…",
                                    ))
                                    .clicked()
                                {
                                    self.open_settings(port);
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Preferences…"))
                                    .clicked()
                                {
                                    self.open_preferences(port);
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Exit"))
                                    .clicked()
                                {
                                    self.request(UiRequest::Exit);
                                    ui.close();
                                }
                            }
                            1 => {
                                let undo = port
                                    .editor()
                                    .undo_label()
                                    .map(|s| format!("Undo {s}"))
                                    .unwrap_or_else(|| "Undo".into());
                                if ui
                                    .add_enabled(
                                        port.editor().undo_label().is_some(),
                                        crate::buttons::Button::new(undo),
                                    )
                                    .clicked()
                                {
                                    self.undo(port);
                                    ui.close();
                                }
                                let redo = port
                                    .editor()
                                    .redo_label()
                                    .map(|s| format!("Redo {s}"))
                                    .unwrap_or_else(|| "Redo".into());
                                if ui
                                    .add_enabled(
                                        port.editor().redo_label().is_some(),
                                        crate::buttons::Button::new(redo),
                                    )
                                    .clicked()
                                {
                                    if self.entity_drafts.is_empty()
                                        && self.localization_drafts.is_empty()
                                    {
                                        port.redo();
                                    }
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(
                                        ui.ctx(),
                                        "Duplicate selection",
                                    ))
                                    .clicked()
                                {
                                    self.duplicate(port);
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(
                                        ui.ctx(),
                                        "Delete selection",
                                    ))
                                    .clicked()
                                {
                                    self.delete(port);
                                    ui.close();
                                }
                            }
                            2 => {
                                if ui.app_button("Gradient Explorer…").clicked() {
                                    self.gradient_explorer.open = true;
                                    ui.close();
                                }
                                for kind in [
                                    achimeld_application::effect_presets::EffectKind::Explosion,
                                    achimeld_application::effect_presets::EffectKind::Particle,
                                ] {
                                    if ui.app_button(format!("{}…", kind.title())).clicked() {
                                        crate::effect_explorer::request(ui.ctx(), kind, None);
                                        ui.close();
                                    }
                                }
                                if ui.app_button("Style Viewer…").clicked() {
                                    self.open_style_viewer();
                                    ui.close();
                                }
                                if ui.app_button("Art Data Bank").clicked() {
                                    self.view = View::Art;
                                    ui.close();
                                }
                                for (label, request) in [
                                    ("Import atlas…", UiRequest::ImportAtlas),
                                    ("Import image…", UiRequest::ImportImage),
                                    ("Export achievement icons…", UiRequest::ExportIcons),
                                    (
                                        "Export achievement translations…",
                                        UiRequest::ExportAchievementTranslations,
                                    ),
                                    (
                                        "Import achievement translations…",
                                        UiRequest::ImportAchievementTranslations,
                                    ),
                                ] {
                                    if ui
                                        .app_button(crate::localization::text(ui.ctx(), label))
                                        .clicked()
                                    {
                                        self.request(request);
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Terminal"))
                                    .clicked()
                                {
                                    self.open_terminal();
                                    ui.close();
                                }
                            }
                            3 => {
                                if ui
                                    .app_button(crate::localization::text(
                                        ui.ctx(),
                                        "Set window size…",
                                    ))
                                    .clicked()
                                {
                                    let fallback = ui.ctx().content_rect();
                                    let size = ui.ctx().input(|input| {
                                        input.viewport().inner_rect.unwrap_or(fallback).size()
                                    });
                                    self.window_size = Some(WindowSizeDraft {
                                        width: size
                                            .x
                                            .round()
                                            .clamp(MIN_WINDOW_INNER_SIZE[0], 8192.0)
                                            as u32,
                                        height: size
                                            .y
                                            .round()
                                            .clamp(MIN_WINDOW_INNER_SIZE[1], 8192.0)
                                            as u32,
                                    });
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Reset layout"))
                                    .clicked()
                                {
                                    // Reset before next frame's windows are laid out.
                                    self.layout_reset_requested = true;
                                    ui.ctx().request_repaint();
                                    ui.close();
                                }
                            }
                            _ => {
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "About"))
                                    .clicked()
                                {
                                    self.open_about();
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Change log"))
                                    .clicked()
                                {
                                    self.open_change_log();
                                    ui.close();
                                }
                                if ui
                                    .app_button(crate::localization::text(ui.ctx(), "Connect MCP"))
                                    .clicked()
                                {
                                    self.open_connect_mcp();
                                    ui.close();
                                }
                            }
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::chrome::window_controls(ui) {
                            self.request(UiRequest::Exit);
                        }
                        ui.separator();
                        // Save state stays hidden. The working identity uses this right-aligned slot.
                        let identity = crate::chrome::title_identity(ui);
                        crate::chrome::drag_window(&identity);
                        let (drag_rect, _) =
                            ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                        let drag = ui.interact(
                            drag_rect,
                            egui::Id::new("window-drag"),
                            egui::Sense::click_and_drag(),
                        );
                        crate::chrome::drag_window(&drag);
                    });
                });
            });
        crate::buttons::bar_panel(root, "toolbar", 24.0).show(root, |ui| {
            crate::buttons::bar(ui, 24.0, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let width = ui.available_width();
                let height = ui.available_height();
                let project_name = port.editor().document().project()["displayName"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                let sync_label = self.sync_action_label(port);
                let validate_icon = port.synchronization_review().is_none()
                    && achimeld_core::SteamAppId::new(port.editor().document().app_id()).is_ok();
                let plan = plan_toolbar(
                    ui,
                    width,
                    ToolbarContents {
                        show_filter: self.view.is_editor() || self.view == View::Steam,
                        show_sort: self.view.is_editor(),
                        sync_label,
                        sync_icon: if validate_icon { 24.0 } else { 20.0 },
                        show_stop: port.reading(),
                        project: &project_name,
                    },
                );
                let right = plan.right_width.min(width);
                let left = (width - right).max(0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(left, height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_clip_rect(ui.max_rect());
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for ((view, label), image) in [
                                (View::Achievements, "Achievements"),
                                (View::Stats, "Stats"),
                                (View::Sync, "Sync"),
                                (View::Steam, "Steam"),
                                (View::Art, "Art Data Bank"),
                                (View::Style, "Style Viewer"),
                            ]
                            .into_iter()
                            .zip(tab_images)
                            {
                                let wants_click = view == View::Sync && self.sync_wants_click(port);
                                if toolbar_tab_attention(
                                    ui,
                                    &mut self.view,
                                    view,
                                    label,
                                    image,
                                    wants_click,
                                    !plan.icons_only,
                                )
                                .changed()
                                {
                                    self.selection.clear();
                                    self.node_drag = None;
                                    self.selection_drag = None;
                                    self.canvas_menu = None;
                                    self.layer_palette.keyboard_active = false;
                                    if view == View::Style {
                                        self.open_style_viewer();
                                    }
                                    if view.is_editor() {
                                        self.kind = if view == View::Stats {
                                            EntityKind::Stat
                                        } else {
                                            EntityKind::Achievement
                                        };
                                    }
                                }
                            }
                        });
                        ui.separator();
                        let undo_label = port
                            .editor()
                            .undo_label()
                            .map(|label| format!("Undo {label} (Ctrl+Z)"))
                            .unwrap_or_else(|| "Undo (Ctrl+Z)".into());
                        if ui
                            .add_enabled(
                                port.editor().undo_label().is_some()
                                    || !self.entity_drafts.is_empty()
                                    || !self.localization_drafts.is_empty(),
                                crate::buttons::Button::new(
                                    egui::Image::new(history_images[0]).alt_text("Undo"),
                                )
                                .bar()
                                .image_tint_follows_text_color(true),
                            )
                            .on_hover_text(undo_label)
                            .clicked()
                        {
                            self.undo(port);
                        }
                        let redo_label = port
                            .editor()
                            .redo_label()
                            .map(|label| format!("Redo {label} (Ctrl+Y)"))
                            .unwrap_or_else(|| "Redo (Ctrl+Y)".into());
                        if ui
                            .add_enabled(
                                port.editor().redo_label().is_some()
                                    && self.entity_drafts.is_empty()
                                    && self.localization_drafts.is_empty(),
                                crate::buttons::Button::new(
                                    egui::Image::new(history_images[1]).alt_text("Redo"),
                                )
                                .bar()
                                .image_tint_follows_text_color(true),
                            )
                            .on_hover_text(redo_label)
                            .clicked()
                        {
                            port.redo();
                        }
                        if self.view.is_editor() {
                            ui.separator();
                            self.sort_button(ui, port);
                        }
                        if self.view.is_editor() || self.view == View::Steam {
                            let separator = ui.separator();
                            let margin = egui::Margin::symmetric(4, 2);
                            // The separator already reserves half its width on this side.
                            ui.add_space(
                                (filter_leading_space(ui) + TOOLBAR_SEPARATOR * 0.5
                                    - separator.rect.width() * 0.5)
                                    .max(0.0),
                            );
                            let filter_width = plan.filter_width.min(ui.available_width());
                            if filter_width >= 1.0 {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.filter)
                                        .id(egui::Id::new("toolbar-filter"))
                                        .hint_text("Filter by name or API name")
                                        .margin(margin)
                                        .desired_width(filter_width),
                                );
                            }
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(right, height),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.set_clip_rect(ui.max_rect());
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let sync_image = if validate_icon {
                            crate::style_picker::Glyph::Validate.image(ui.ctx(), 24.0)
                        } else {
                            egui::Image::new(tab_images[2])
                        };
                        let button = if plan.icons_only {
                            crate::buttons::Button::new(sync_image.alt_text(sync_label))
                        } else {
                            crate::buttons::Button::new((sync_image, sync_label))
                        };
                        let mut response = ui.add_enabled(
                            self.sync_action_enabled(port),
                            button
                                .id(egui::Id::new("toolbar-sync"))
                                .icon_spacing(5.0)
                                .bar()
                                .image_tint_follows_text_color(true)
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        if plan.icons_only {
                            response = response.on_hover_text(sync_label);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    response.enabled(),
                                    sync_label,
                                )
                            });
                        }
                        if response.clicked() {
                            self.view = View::Sync;
                            self.selection.clear();
                            self.run_sync_action(port);
                        }
                        if port.reading()
                            && ui.add(crate::buttons::Button::new("Stop").bar()).clicked()
                        {
                            self.request(UiRequest::CancelRead);
                        }
                        if plan.project_width > 0.5 && !project_name.is_empty() {
                            ui.add_space(12.0);
                            let full = toolbar_text_width(ui, &project_name, egui::TextStyle::Body);
                            let mut label =
                                egui::Label::new(project_name.as_str()).selectable(false);
                            let truncated = plan.project_width + 0.5 < full;
                            if truncated {
                                label = label.truncate();
                            }
                            let response =
                                ui.add_sized([plan.project_width, ui.available_height()], label);
                            if truncated {
                                response.on_hover_text(project_name.as_str());
                            }
                        }
                    },
                );
            });
        });
        if self.view.is_editor() {
            self.sync_canvas_camera(port);
        } else {
            self.camera_drag = None;
            self.camera_motion = None;
            self.canvas_menu = None;
            if self.icon_pan.take().is_some() || self.camera_editing {
                port.finish_group();
                self.camera_editing = false;
            }
        }
        let mut canvas_action = None;
        egui::Panel::bottom("status")
            .default_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::color(&preferences, "Panel"))
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(root, |ui| {
                ui.spacing_mut().interact_size.y = 24.0;
                ui.spacing_mut().button_padding.y = 2.0;
                ui.horizontal_centered(|ui| {
                    if self.view.is_editor() {
                        canvas_action = self.canvas_controls(ui);
                        ui.separator();
                    }
                    ui.label(port.status());
                    if port.exporting() && ui.app_button("Cancel export").clicked() {
                        self.request(UiRequest::CancelExport);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("App ID {}", port.editor().document().app_id()));
                        ui.label(format!(
                            "{} achievements · {} stats",
                            port.editor()
                                .document()
                                .entities(EntityKind::Achievement)
                                .len(),
                            port.editor().document().entities(EntityKind::Stat).len()
                        ));
                    });
                });
            });
        let inspector_bounds = root.available_rect_before_wrap();
        if self.view.is_editor() {
            self.inspector_panel(root, port);
        } else {
            self.inspector_panel.cancel_drag();
            self.inspector_panel.header = None;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::color(&preferences, "Background top")))
            .show(root, |ui| {
                if self.view == View::Art {
                    self.art_bank_view(ui, port);
                    self.image_drop_zone(
                        ui,
                        ui.max_rect(),
                        crate::image_drop::DropZone::ArtBank,
                        0,
                    );
                } else if self.view == View::Style {
                    self.style_viewer_view(ui, port);
                } else if self.view == View::Sync {
                    self.synchronization_view(ui, port);
                } else if self.view == View::Steam {
                    self.steam_view(ui, port);
                } else {
                    self.canvas(ui, port, &preferences, canvas_action);
                }
            });
        self.inspector_dock_preview(&ctx, inspector_bounds);
        self.layers_window(&ctx, port, root.is_enabled());
        self.gradient_explorer_window(&ctx, port, root.is_enabled());
        self.gradient_map_collection_window(&ctx, port, root.is_enabled());
        self.effect_explorer_window(&ctx, port, root.is_enabled());
        self.terminal
            .observe_sync(port.synchronizing() && port.synchronization_review().is_none());
        self.terminal.show(&ctx, root.is_enabled());
        self.window_size_panel(&ctx, root.is_enabled());
        // The floating window has its current frame position now. Connecting
        // visible icon rectangles takes only CPU geometry, with no render jobs.
        self.paint_layer_connections();
        self.source_picker_window(&ctx, port);
        self.settings_window(&ctx, port);
        self.sync_app_id_window(&ctx, port);
        self.sync_plan_window(&ctx, port);
        self.steam_delete_window(&ctx, port);
        self.sort_window(&ctx, port);
        self.preferences_window(&ctx, port);
        self.about_window(&ctx);
        if root.is_enabled() {
            self.dropped_art(&ctx, port);
        }
        self.image_drop_dialog(&ctx, port);
        self.style_edit_prompt_window(&ctx, port);
        self.style_rename_window(&ctx, port);
        self.style_delete_window(&ctx, port);
        self.sync_blend_preview(&ctx, port, root.is_enabled());
        if let Some((minimum, ratio)) = crate::fields::take_change(&ctx) {
            if let Some(draft) = &mut self.preferences {
                draft.field_label_min_width = minimum;
                draft.field_label_width_percent = ratio;
            } else {
                let mut next = port.preferences().clone();
                next.field_label_min_width = minimum;
                next.field_label_width_percent = ratio;
                self.apply(
                    port,
                    "Resize property labels",
                    Command::Preferences(next),
                    None,
                );
            }
        }
        crate::chrome::resize_window(root, window_rect);
        for request in std::mem::take(&mut self.requests) {
            if (!self.entity_drafts.is_empty() || !self.localization_drafts.is_empty())
                && !matches!(
                    request,
                    UiRequest::CancelRead | UiRequest::CancelExport | UiRequest::AboutDismissed
                )
            {
                self.error = Some(
                    "Finish or revert the highlighted invalid edits before continuing.".into(),
                );
                achimeld_application::diagnostics::record(
                    "WARN",
                    "editor",
                    self.error.as_deref().unwrap_or_default(),
                );
            } else {
                port.request(request);
            }
        }
        if let Some(message) = self.error.clone() {
            egui::Window::new("Action could not be completed")
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label(message);
                    if ui
                        .app_button(crate::localization::text(ui.ctx(), "OK"))
                        .clicked()
                    {
                        self.error = None;
                    }
                });
        }
    }
    pub(crate) fn apply(
        &mut self,
        port: &mut dyn EditorPort,
        label: &str,
        command: Command,
        group: Option<&str>,
    ) -> bool {
        if let Err(error) = port.execute(label, command, group) {
            achimeld_application::diagnostics::record("ERROR", "editor", &error);
            self.error = Some(error);
            false
        } else {
            true
        }
    }
    pub(crate) fn request(&mut self, request: UiRequest) {
        self.requests.push(request);
    }
    pub(crate) fn recent_projects_menu(&mut self, ui: &mut egui::Ui, port: &dyn EditorPort) {
        if port.recent_projects().is_empty() {
            ui.add_enabled(
                false,
                crate::buttons::Button::new(crate::localization::text(ui.ctx(), "No recent files")),
            );
        } else {
            for path in port.recent_projects() {
                if ui.app_button(path.to_string_lossy()).clicked() {
                    self.request(UiRequest::OpenRecentProject(path.clone()));
                    ui.close();
                }
            }
        }
    }
    pub fn close(&mut self, port: &mut dyn EditorPort) {
        if self.effect_explorer.open && !self.effect_explorer.save(port) {
            return;
        }
        if self.entity_drafts.is_empty() && self.localization_drafts.is_empty() {
            port.request(UiRequest::Exit);
        } else {
            self.error =
                Some("Finish or revert the highlighted invalid edits before closing.".into());
        }
    }
    pub fn open_style_viewer(&mut self) {
        self.view = View::Style;
        self.selection.clear();
        self.node_drag = None;
        self.selection_drag = None;
        self.canvas_menu = None;
        self.layer_palette.keyboard_active = false;
        self.request(UiRequest::StyleViewer(
            achimeld_application::style_viewer::StyleViewerAction::Open,
        ));
    }
    pub(crate) fn undo(&mut self, port: &mut dyn EditorPort) {
        self.layer_palette.active_style = None;
        if self.entity_drafts.is_empty() && self.localization_drafts.is_empty() {
            port.undo();
        } else {
            self.entity_drafts.clear();
            self.localization_drafts.clear();
        }
        self.layer_drag = None;
        self.layer_palette.effect_drag = None;
        self.node_drag = None;
        self.selection_drag = None;
    }
    pub(crate) fn shortcuts(&mut self, ctx: &egui::Context, port: &mut dyn EditorPort) {
        let dialog_open = self.about
            || self.gradient_explorer.open
            || self.art_bank.pending()
            || self.settings
            || self.sort_open
            || self.preferences.is_some()
            || self.source_picker.is_some()
            || self.style_rename.is_some()
            || self.style_edit_prompt.is_some()
            || self.style_viewer_ui.deletion.is_some();
        let editing_text = ctx.egui_wants_keyboard_input();
        let actions = ctx.input_mut(|i| {
            let mut actions = Vec::new();
            for (key, shift, action) in [
                (egui::Key::N, false, 0),
                (egui::Key::O, false, 1),
                (egui::Key::S, true, 3),
                (egui::Key::S, false, 2),
                (egui::Key::Z, true, 5),
                (egui::Key::Z, false, 4),
                (egui::Key::Y, false, 5),
                (egui::Key::D, false, 6),
            ] {
                if dialog_open
                    || (self.view == View::Style
                        && action == 6
                        && !self.layer_palette.keyboard_active)
                    || (action == 6 && editing_text)
                {
                    continue;
                }
                if i.consume_key(
                    egui::Modifiers {
                        ctrl: true,
                        shift,
                        ..Default::default()
                    },
                    key,
                ) {
                    actions.push(action);
                }
            }
            actions
        });
        for action in actions {
            match action {
                0 => self.request(UiRequest::NewProject),
                1 => self.request(UiRequest::OpenProject),
                2 => self.request(UiRequest::SaveProject),
                3 => self.request(UiRequest::SaveProjectAs),
                4 => self.undo(port),
                5 => {
                    if self.entity_drafts.is_empty() && self.localization_drafts.is_empty() {
                        port.redo();
                    }
                }
                6 if self.layer_palette.keyboard_active => {
                    self.layer_palette.pending = Some(crate::layer_palette::LayerAction::Duplicate)
                }
                6 => self.duplicate(port),
                _ => {}
            }
        }
        if !dialog_open
            && !ctx.egui_wants_keyboard_input()
            && !self
                .general_images
                .focus
                .is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
        {
            if self.layer_palette.keyboard_active {
                self.layer_palette.pending = Some(crate::layer_palette::LayerAction::Delete);
            } else {
                self.delete(port);
            }
        }
    }
    pub(crate) fn delete(&mut self, port: &mut dyn EditorPort) {
        if self.view.is_editor() && !self.selection.is_empty() {
            self.apply(
                port,
                "Delete selection",
                Command::Delete {
                    kind: self.kind,
                    ids: self.selection.clone(),
                },
                None,
            );
            self.selection.clear();
        }
    }
    fn duplicate(&mut self, port: &mut dyn EditorPort) {
        if self.view.is_editor() {
            let commands = self
                .selection
                .iter()
                .map(|id| Command::Duplicate {
                    kind: self.kind,
                    id: id.clone(),
                })
                .collect();
            self.apply(port, "Duplicate selection", Command::Batch(commands), None);
        }
    }
    pub(crate) fn texture(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        id: &str,
    ) -> Option<egui::TextureId> {
        if let Some(preview) = port.preview(id) {
            self.preview_texture(ctx, id, preview)
        } else {
            self.texture_clock += 1;
            self.textures.get_mut(id).map(|(_, texture, used)| {
                *used = self.texture_clock;
                texture.id()
            })
        }
    }
    pub(crate) fn canvas_texture(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        id: &str,
        physical_edge: f32,
    ) -> Option<egui::TextureId> {
        let edge = (physical_edge.ceil() as usize)
            .clamp(64, 512)
            .next_power_of_two();
        // One GPU handle per card is reused as resolution changes. A pending
        // size request keeps the previous successful texture visible.
        let key = format!("canvas:{id}");
        if let Some(preview) = port.canvas_preview(id, edge) {
            self.preview_texture(ctx, &key, preview)
        } else {
            self.texture_clock += 1;
            self.textures.get_mut(&key).map(|(_, texture, used)| {
                *used = self.texture_clock;
                texture.id()
            })
        }
    }
    pub(crate) fn layer_texture(
        &mut self,
        ctx: &egui::Context,
        port: &mut dyn EditorPort,
        achievement: &str,
        layer: &str,
    ) -> Option<egui::TextureId> {
        let preview = port.layer_preview(achievement, layer)?;
        self.preview_texture(ctx, &format!("{achievement}/{layer}"), preview)
    }
    pub(crate) fn preview_texture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        preview: achimeld_application::Preview,
    ) -> Option<egui::TextureId> {
        self.texture_clock += 1;
        let options =
            egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
        if self
            .textures
            .get(id)
            .is_none_or(|(fingerprint, _, _)| fingerprint != &preview.fingerprint)
        {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [preview.width, preview.height],
                &preview.rgba,
            );
            if let Some((fingerprint, handle, _)) = self.textures.get_mut(id) {
                self.texture_bytes -= mipmap_bytes(handle.size());
                handle.set(image, options);
                *fingerprint = preview.fingerprint;
            } else {
                let handle = ctx.load_texture(id, image, options);
                self.textures
                    .insert(id.into(), (preview.fingerprint, handle, self.texture_clock));
            }
            self.texture_bytes += mipmap_bytes([preview.width, preview.height]);
        }
        if let Some((_, _, used)) = self.textures.get_mut(id) {
            *used = self.texture_clock;
        }
        // Include every mip level in the budget; evict least recently displayed artwork.
        while self.textures.len() > 512 || self.texture_bytes > 128 * 1024 * 1024 {
            let Some(old) = self
                .textures
                .iter()
                .filter(|(key, _)| key.as_str() != id)
                .min_by_key(|(_, (_, _, used))| *used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((_, handle, _)) = self.textures.remove(&old) {
                self.texture_bytes -= mipmap_bytes(handle.size());
            }
        }
        self.textures.get(id).map(|(_, t, _)| t.id())
    }

    pub(crate) fn cached_preview_texture(&mut self, id: &str) -> Option<egui::TextureId> {
        self.texture_clock += 1;
        self.textures.get_mut(id).map(|(_, texture, used)| {
            *used = self.texture_clock;
            texture.id()
        })
    }
}
