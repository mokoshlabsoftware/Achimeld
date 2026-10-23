# Pinned egui text renderer patches

`epaint` and `egui_glow` are paired source dependencies, selected by the root
`[patch.crates-io]` table. Both are the published 0.36.2 sources from egui commit
`49682f8baa058bf49e011035cfbd6e825f88a5ef`. Upstream is
<https://github.com/emilk/egui>. Each directory includes the upstream MIT and
Apache-2.0 licenses. The published archive SHA-256 identities were:

| Crate | Published archive SHA-256 |
| --- | --- |
| epaint | `a0ec0308dc23130fc1a7d4623a8534c2ed069c6dc84e258ee4f3de99615aa1a3` |
| egui_glow | `606ed4167daf341e5753d703285c4c1d562324b2c07aa8f405cd7d066fe58f3b` |

Local changes are limited to `epaint/src/text/font.rs`,
`egui_glow/src/painter.rs`, `egui_glow/src/shader/fragment.glsl`, and the license
include paths in their manifests. Other source is unchanged upstream code.
These library dependencies retain their upstream lint configuration and are
excluded from workspace membership. The application workspace still runs its
strict lint gate; the regression tests for this integration are compiled and
run in `achimeld-ui/src/text_raster_regressions.rs`.

The rasterizer hints at the physical font size, then samples glyph coverage at
three times the horizontal resolution. A color-balanced five-tap LCD filter
reduces edge fringing. Three coverage channels occupy the existing RGBA atlas;
the alpha channel contains their average. Glyph advances and layout metrics do
not change as a consequence of the coverage conversion. Scratch raster storage
is three times the normal width for one glyph; persistent atlas dimensions and
the existing bounded cache policy are unchanged.

The Glow renderer draws font-atlas meshes once per color channel, using that
channel's coverage for both source color and source alpha. A fourth pass writes
scalar framebuffer alpha. This preserves primitive ordering for overlapping
glyphs without requiring dual-source blending or a framebuffer readback.
Images retain the original single-pass renderer. White atlas texels and
pre-rasterized discs have equal RGB coverage, so ordinary filled geometry keeps
its existing appearance. Uniforms and color masks are reset before returning
to other meshes or paint callbacks. The extra passes trade draw submission cost
for compatibility with the existing OpenGL backend.

This path targets horizontally striped RGB displays at the final native pixel
resolution. It is not Windows DirectWrite/ClearType and does not detect RGB/BGR
panel order or non-stripe OLED layouts. Scaled copies of a screenshot cannot
represent the display's physical subpixels. Icon export continues through the
separate imaging renderer, which does not use this font atlas.

Update both patches together when upgrading egui. Do not use this atlas with
an unpatched renderer: ordinary scalar alpha blending cannot composite the
three coverages correctly.

Validation: run the normal `achimeld-ui` tests. For actual GPU validation, set
`ACHIMELD_SCREENSHOT_TO` to a PNG under the external current build directory
and run `native_rgb_subpixel_text` with `--ignored --test-threads=1`.
`ACHIMELD_TEXT_SCALE` accepts `1.0`, `1.25`, `1.5`, and `2.0`. The fixture checks
RGB edges on both light and dark backgrounds, image alpha blending, and
restoration of ordinary atlas geometry after an image draw. The existing
`native_button_hover_screenshot` and `native_app_style_screenshot` fixtures cover
the shared button presentation.
