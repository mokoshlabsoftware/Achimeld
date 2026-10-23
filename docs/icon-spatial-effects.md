# Spatial layer effects

Add **Skew**, **Perspective**, **Pinch**, or **Lens blur** from a layer's Effects menu. They also work on folders and saved styles. Effects run in their displayed stack order; visibility, opacity, reordering, removal and undo use the existing effects workflow. Opacity reduces distortion strength. Lens blur opacity reduces both edge softening and color separation.

| Effect | Controls | Behavior |
| --- | --- | --- |
| Skew | Horizontal/vertical angles, −60° to 60° | Centered horizontal shear followed by vertical shear. The two shears remain invertible at every permitted combination. |
| Perspective | Horizontal/vertical, −45% to 45% | Centered projective tilt; the restricted coefficients keep the horizon outside the source canvas. |
| Pinch | Amount, −100% to 100% | Positive values create a fisheye bulge; negative values pinch inward. The circular distortion smoothly meets the unchanged outer region. |
| Lens blur | Edge blur, 0–128; chromatic aberration, 0–64 | Leaves the center clear and increases softening and red/blue separation toward the edges. Blur and separation distances use the existing 512-pixel reference height. |

Skew and Perspective start at zero. Pinch starts at 50%; Lens blur starts with blur 4 and aberration 8. Samples outside the canvas are transparent, and the canvas dimensions stay fixed, so enlarged artwork can be clipped. Sampling uses premultiplied alpha to preserve translucent edges. The existing bounded preview workers perform the rendering, check cancellation between rows, and retain their usual preview/refinement scheduling.

Recipes retain the optional fields `skewX`, `skewY`, `perspectiveX`, `perspectiveY`, `amount`, and `aberration`; Lens blur also uses `blur`. Rust command validation and managed model validation enforce the same ranges. Synchronization retains raw recipes and uses the existing host-generated icon hashes. Existing effect enum values retain their numeric identities; the four new kinds are appended. Project storage and synchronization preserve the new settings without adding operations or filesystem authority. Old application versions may reject these new effect kinds.

Source checks cover neutral/invisible/zero-opacity effects, both transform axes, equal-angle shear stability, perspective limits, pinch direction, lens center/fringes, transparency, effect order and lighting/outline coverage, small canvases, cancellation, parameter rejection, undo/redo, project persistence and managed recipe retention. These checks do not qualify installed Windows enforcement or live Steam behavior.

## Perlin noise layers and masks

**Perlin noise** is a procedural layer in the layer picker. Its seed reproduces the same field; frequency controls feature size, octaves (1–12) control the number of frequency bands, detail strength controls amplitude falloff, and frequency spacing controls the ratio between bands. Individual octave switches retain their settings when the octave count is reduced and restored. Disabled octaves do not renumber the remaining frequencies or seeds. With every octave disabled the result is neutral grey. **Gradient colors** maps low-to-high noise values through the shared gradient editor, including transparent stops and saved presets.

An empty **Mask** slot has a **+** picker for any layer type, including folders, images, text, adjustments, gradients and Perlin noise. The properties picker can also reuse an existing layer. Creating a mask creates an ordinary editable layer and attaches it in one undoable edit. Referenced mask sources do not also draw independently in the stack. Their visibility switch does not disable the mask; remove the mask link to detach it. Sources can be shared, transformed, styled and masked themselves. Circular folder/mask dependencies are rejected.

**Alpha** uses source transparency; **Luminance** multiplies transparency by brightness (white reveals, black hides). **Invert mask** reverses coverage before source opacity. New noise, solid and gradient masks start with luminance; image, text, shape and folder masks start with alpha. Existing projects retain alpha masking by default. Adjustment recipients interpolate between the original and adjusted backdrop using mask coverage; adjustment mask sources process an immutable copy of the recipient's current backdrop without adding implicit render dependencies.

Version-one recipes add `kind: "PerlinNoise"` with a `perlinNoise` settings object (`seed`, `frequency`, `octaves`, `persistence`, `lacunarity`, `enabledOctaves`, `useGradient`, `gradient`). Other layers keep the existing `maskLayerId`, with optional `maskMode` (`Alpha`/`Luminance`) and `maskInvert`. Rust and managed validation bound noise parameters and gradient stops. Rendering uses the existing cancellable preview workers and charges octave work to the shared render budget. Generated layer artwork and its generation prompt are retained under `crates/achimeld-ui/assets/toolbar/layer-perlin-noise*` and the final glyph is compiled into the executable. No protocol operations or filesystem authority are added.
