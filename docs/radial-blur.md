# Radial blur adjustment

Add **Radial blur** from the adjustment-layer picker. Its generated icon depicts streaks spreading from a central opening. Like other adjustment layers, it changes the composited layers below it and supports opacity, blend mode, masks, history and project persistence.

**Radial blur** spreads artwork toward and away from the canvas center. **Angular blur** sweeps it around that center. Both are fixed sliders from 0–100%; zero on both is an exact identity. These controls do not use a random seed. The center is the center of the output canvas.

Each enabled direction uses 17 tent-weighted bilinear samples. At 100%, radial sampling spans half to one-and-a-half times the distance from the center; angular sampling spans ±45 degrees. Radial runs before angular when both are enabled. Sampling, interpolation, opacity and masks preserve premultiplied color so transparent pixels do not introduce color fringes. Outside the canvas is transparent.

Each direction is charged 68 image planes to the shared compositor work budget before allocation. Rendering checks cancellation before allocation and once per row. Extreme dimensions or repeated adjustments remain subject to existing image-size and composition limits. Layer kind `RadialBlur` is appended as ID 21; older kind IDs and worker operations are unchanged. Amounts must be finite and within 0–100 in both Rust and managed validation.

Explosion layers expose their own [Radial blur and Angular blur](explosion-layers.md) controls. Those soften the bounded construction rectangle before polar projection, keeping the blur aligned with the generated explosion. The adjustment layer instead operates on the already-composited canvas.

Executable evidence: `radial_blur_axes_preserve_transparency_wrap_and_distinct_directions` covers directions, transparent colors, seam behavior and cancellation; `radial_blur_adjustment_round_trips_masks_opacity_and_history_with_bounded_work` covers rendering, serialization, undo/redo, masks, opacity, invalid input and the work cap. `RadialBlurFixtures` covers managed mapping, validation, enum identities and fingerprints. These are source and synthetic fixture checks, not installed or live-runtime qualification.
