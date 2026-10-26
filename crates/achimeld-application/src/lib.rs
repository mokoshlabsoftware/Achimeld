//! Application-owned documents and commands. No filesystem, process, GUI or network authority.
#![forbid(unsafe_code)]
pub mod art_bank;
pub mod art_images;
pub mod artifact3d;
pub mod automation;
pub mod canvas;
mod delta;
pub mod diagnostics;
pub mod document;
mod editor_state;
pub mod effect_presets;
pub mod explosion;
pub mod gradient_file;
pub mod gradient_map;
pub mod history;
pub mod masks;
pub mod particles;
#[cfg(test)]
mod perlin_tests;
pub mod ports;
pub mod preferences;
pub mod preview;
pub mod reorder;
pub mod sorting;
#[cfg(test)]
mod spatial_effects_tests;
pub mod style_achievements;
pub mod style_collections;
pub mod style_management;
pub mod style_viewer;
#[cfg(test)]
mod synchronization_tests;
pub mod text3d;
pub mod translations;
mod validation;
pub use document::{Catalog, Command, Document, EntityKind, new_id};
pub use editor_state::{GradientPreset, GradientStop};
pub use editor_state::{IconStyle, LayerLayout, style_signature};
pub use history::Editor;
pub use ports::{EditorPort, ImageImportTarget, Preview, UiRequest};
pub use preferences::Preferences;
pub use reorder::ReorderGesture;

mod general_images;
pub use general_images::{
    append_general_image, general_image_entries, store_general_image_entries,
};
