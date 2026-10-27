//! Bounded icon composition. The host supplies image/font bytes; this crate has no I/O.
#![forbid(unsafe_code)]
mod artifact3d;
#[cfg(test)]
mod artifact3d_tests;
#[cfg(test)]
mod effect_diagnostics_tests;
mod effects;
mod explosion;
#[cfg(test)]
mod light_effects_tests;
pub use explosion::construction as render_explosion_construction;
#[cfg(test)]
mod explosion_tests;
mod font;
#[cfg(test)]
mod particle_tests;
mod particles;
mod perlin;
pub mod photoshop;
mod radial_blur;
#[cfg(test)]
mod radial_blur_tests;
mod shape;
mod spatial_effects;
mod text3d;
#[cfg(test)]
mod text3d_tests;
pub use font::Font;
pub mod renderer;
pub use renderer::{
    AssetSource, IconPair, render, render_achieved_cancellable, render_cancellable,
    render_layer_cancellable,
};
#[cfg(test)]
mod art_image_tests;
#[cfg(test)]
mod perlin_tests;
#[cfg(test)]
mod spatial_effects_tests;
#[cfg(test)]
mod tests;
