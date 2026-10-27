//! Presentation only. All document mutation and I/O go through the application port.
#![forbid(unsafe_code)]
mod about;
mod achievement_general;
mod art_bank;
mod artifact3d;
mod blend_icons;
mod blend_preview;
pub mod buttons;
mod canvas;
mod change_log;
mod chrome;
mod color_picker;
mod curves;
pub mod desktop;
mod dropdown_navigation;
mod dropdowns;
mod effect_diagnostics;
mod effect_explorer;
mod effect_icons;
mod effect_mini;
mod explosion;
mod fields;
mod font_picker;
mod general_images;
mod gradient;
mod gradient_explorer;
mod gradient_map;
mod gradient_map_collection;
mod icons;
mod image_drop;
mod inspector;
mod inspector_panel;
mod layer_backdrop;
mod layer_effects;
mod layer_icons;
mod layer_masks;
mod layer_palette;
mod layer_toolbar;
mod layers;
mod localization;
mod node_navigation;
mod opacity;
mod particles;
mod perlin;
mod settings;
mod sliders;
mod sorting;
mod source_picker;
mod steam;
mod style_banner;
mod style_picker;
mod style_viewer;
mod synchronization;
mod synchronization_target;
mod terminal;
#[cfg(test)]
#[path = "../../../build/test_output.rs"]
mod test_output;
mod text3d;
mod text_alignment;
mod text_content;
mod theme;
mod typography;
mod vignette;
mod window_layout;
pub use change_log::window_title;
pub use desktop::Desktop;
pub use theme::apply as apply_theme;
pub use typography::{BOLD_FAMILY, configure_fonts};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_raster_regressions;
