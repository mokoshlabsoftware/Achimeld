The direct project-style actions use the generated `style-import` folder/arrow
and `style-save` disk. Original transparent `*-source.png` artwork is retained
unchanged. Exact built-in imagegen prompts and 64 x 64 tintable RGBA preparation
are recorded in `style-library-actions.json`; the embedded icons follow the
shared button ink and use linear mipmap filtering.

The Sync and upper-toolbar **Validate** actions use the generated magnifying
glass/checkmark in `sync-validate-source.png`. Both original generation outputs
are retained unchanged. `sync-validate.json` records the built-in imagegen prompts
and 64 x 64 white tintable PNG/RGBA preparation. The icon follows the shared
animated button ink and uses linear mipmap filtering.

The eleven effect symbols and the Width/Color parameter symbols were generated
with the built-in imagegen tool through the imagegen skill. `effect-icons.json`
retains the exact prompt set; `effect-*-source.png` retains the unchanged originals.
Centered 64 x 64 white tintable PNG/RGBA copies preserve generated transparency
and are embedded with linear mipmap filtering in the effect menu and FX rows.

The Explosion layer uses the generated nine-arm starburst in `layer-explosion-source.png`.
Its bottom-scaling curve dropdown uses generated `explosion-curve-linear`,
`explosion-curve-quadratic`, and `explosion-curve-logarithmic` icons. Original
transparent PNGs, exact built-in imagegen prompts, and preparation notes are
retained in `explosion-curve-icons.json`; centered 64 x 64 tintable RGBA glyphs
are embedded with linear mipmap filtering.
Its count-range action uses `explosion-range-source.png`. Exact prompts are retained
in `explosion-icons.json`; centered 64 x 64 white tintable RGBA assets preserve the
generated transparency and use linear mipmap filtering.

Its compact action bar adds generated `layer-mask`, `explosion-seed-per-achievement`
and `explosion-cover-canvas` icons. Original PNG sources, exact built-in imagegen
prompts, reused artwork and 64 x 64 embedding notes are recorded in
`explosion-toolbar-icons.json`. The white tintable RGBA assets preserve generated
alpha and follow the shared button palette and linear mipmap filtering.

The two PNG icons are copied unchanged from the Achiever repository:
`src/Achimeld.App/Assets/Toolbar/add-achievement.png` and `add-stat.png`.

The matching `.rgba` files contain their decoded 64 x 64, row-major RGBA8 pixels
for embedding, using the same format as the UI's brand icon. The UI uses these
local assets without an image decoder or an external checkout at build time.

The Perlin noise layer icon was generated with the built-in imagegen tool.
`layer-perlin-noise-generated.png` retains the source, `layer-perlin-noise.png`
previews the embedded 64 x 64 glyph, and `layer-perlin-noise.rgba` contains its
tintable pixels with generated alpha preserved. The exact prompt and provider
are recorded in `layer-perlin-noise.json`.

The Radial blur adjustment icon was generated through the imagegen skill with
the built-in Image Generation tool. `layer-radial-blur-source.png` retains the
original transparent source; its exact prompt and preparation are recorded in
`layer-radial-blur.json`. The centered 64 x 64 white tintable RGBA asset preserves
the resized generated alpha and is embedded with linear mipmap filtering.

The five `sort*.png` icons were created with the built-in Image Generation tool
through the imagegen skill for the shared Achievements/Stats Sort control and
the original grid panel's icon-style, new-line, fill-direction and selection controls.
Exact prompts and preparation are retained in `sort-icons.json`. The source
PNGs are retained unchanged; white tintable 64 x 64 `.rgba` copies retain the
generated alpha, center the visible bounds at a consistent size, and are embedded
with linear mipmap filtering.

The five `tab-*.png` source icons were created with the built-in Image Generation
tool. Their exact prompts are retained in `tab-icons.json`. The corresponding
64 x 64 `.rgba` files are white tintable glyphs retaining the generated alpha,
embedded in the application. The Sync tab and Synchronize action share one icon.

The four `art-*.png` icons were created with the built-in Image Generation tool
for General images, Icon images, Atlases and Refresh. Exact prompts are retained
in `art-icons.json`. Their 64 x 64 `.rgba` copies retain the generated alpha and
use white pixels so the shared buttons can animate their tint with the labels.

The additional `art-import.png` icon was created with the built-in Image
Generation tool through the imagegen skill for Import artwork and Import atlas.
Its exact prompt and embedding notes are retained in `art-import.json`. The
original PNG is retained unchanged; the 64 x 64 `art-import.rgba` contains white
tintable pixels with the resized generated alpha.

The three inspector-*.png icons were created with the built-in Image Generation
tool for General properties, Localization and Icon artwork. Their generation
and refinement prompts are retained in inspector-icons.json. The 64 x 64 .rgba
copies retain their generated alpha and use white pixels for animated tinting.

The `layer-eye-source.png` stencil was created with the built-in Image Generation
tool, inspired by Achiever's smooth outlined visibility eye. Its exact prompt
and asset preparation are retained in `layer-eye.json`. `layer-eye.png` and the
embedded 64 x 64 `layer-eye.rgba` use the generated stencil's luminance as alpha,
with white tintable pixels and centered visible bounds. The shared icon button
centers a fixed square glyph within the actual button body at every font size;
the visibility button is centered beside the layer thumbnail.

The eleven `layer-*.png` icons were created with the built-in Image Generation
tool for the layer picker and adjustment-layer symbols. Exact prompts and the
three reused Art Data Bank icons are recorded in `layer-icons.json`. The source
PNGs are retained unchanged. Their 64 x 64 white `.rgba` copies preserve the
generated alpha for palette tinting and linear mipmap filtering. Adjustment
rows use these symbols instead of artwork thumbnails.

The right-hand layer properties pane uses these same generated type symbols.
The additional Folder symbol was created through the imagegen skill using the
built-in Image Generation tool. Its original source is retained in
`layer-folder-generated.png`, with the exact prompt and embedding notes in
`layer-properties-icons.json`. The generated alpha is preserved in its 64 x 64
white tintable `layer-folder.rgba` asset. Imported artwork uses the generated
Icon images symbol.

The five `text-align-*.png` symbols were created through the imagegen skill with
the built-in Image Generation tool for the text layer's four corner anchors and
center. Exact prompts are retained in `text-alignment-icons.json`. Original PNGs
are retained unchanged; embedded 64 x 64 white `.rgba` glyphs preserve generated
alpha and normalize visible bounds for palette tinting and linear mipmap filtering.

The six `menu-*.png` symbols were created through the imagegen skill with the
built-in Image Generation tool for the General images context menu. Exact prompts
and preparation are retained in `general-image-menu-icons.json`. Original PNGs
are retained unchanged. Their embedded 64 x 64 white tintable `.rgba` copies
preserve the resized generated alpha. The menu uses a shared leading icon column
and the same animated tint for icons and labels, including disabled actions.

The `style-rename.png` and `style-approve.png` symbols were generated through the
imagegen skill with the built-in Image Generation tool for the style dropdown
and Rename style prompt. Exact prompts and preparation are retained in
`style-picker-icons.json`. Original PNGs are retained unchanged; the embedded
64 x 64 white tintable `.rgba` copies preserve the generated alpha. The Cancel
symbol reuses the application's generated prompt artwork.

The Style Viewer mode uses the generated cave-handprint `tab-style-viewer.png`.
`style-set-load.png` and `style-set-save.png` are the matching file actions.
Original PNGs, exact prompts, and preparation notes are retained in
`style-viewer-icons.json`; embedded 64 x 64 white RGBA glyphs preserve their
generated alpha and follow the shared animated ink with linear mipmap filtering.

The 3D text layer uses the generated extruded T in `layer-text3d-source.png`.
Its built-in imagegen prompt and preparation are in `layer-text3d.json`; the
embedded 64 x 64 white `layer-text3d.rgba` preserves the generated transparency.

The 3D artifact layer uses the generated primitive assembly in
`layer-artifact3d-source.png`. The exact built-in imagegen prompt and preparation
are in `layer-artifact3d.json`; its 64 x 64 white `layer-artifact3d.rgba` preserves
generated transparency and follows the shared animated ink.

The Sort panel's helper inclusion action uses `sort-helpers-source.png`, generated
through the imagegen skill with the built-in Image Generation tool. The exact
prompt is retained in `sort-helpers.json`. Its centered 64 x 64 white tintable
`sort-helpers.rgba` preserves the generated alpha and uses linear mipmap filtering.

The gradient Reverse action uses the built-in imagegen source in
`gradient-flip-source.png`. Its exact prompt and embedding are recorded in
`gradient-flip.json`. The dominant visible glyph is cropped and centered in a
64 x 64 white tintable RGBA asset, preserving its generated alpha.

The five Preferences section icons use built-in imagegen artwork. Original
transparent sources, exact prompts and 64 x 64 white tintable RGBA preparation
are retained in preferences-icons.json and preferences-*-source.png.

The blend control and its eight modes use built-in imagegen artwork. Exact
prompts and preparation are retained in `blend-icons.json`; `blend-*-source.png`
retains each original. Embedded 64 x 64 white RGBA glyphs preserve generated
alpha, follow animated button ink, and use linear mipmap filtering.

The final layer-bar action uses the generated five-pip die in
`layer-action-reroll-source.png`. Its exact built-in imagegen prompt and
preparation are retained in `layer-action-reroll.json`; the centered 64 x 64
white tintable glyph preserves the generated alpha and uses linear mipmaps.

The trailing text unlock-value toggle uses the generated chain links in
`text-unlock-link-source.png`. Its exact built-in imagegen prompt and preparation
are retained in `text-unlock-link.json`; the centered 64 x 64 white tintable
glyph preserves the generated alpha and follows the shared button ink.

Toolbar PNGs are limited to 128 x 128 by `build/Resize-ToolbarIcons.ps1`,
invoked by `build/Rust.ps1` before build/run/check/test/clippy. It preserves
aspect ratio and transparency using high-quality bicubic interpolation and
leaves smaller images untouched. Run it directly after adding artwork.
Original artwork retained before introducing this limit lives in
`../toolbar-originals`; historical preparation notes above describe those originals.
Raw .rgba embedding assets retain their existing 64 x 64 contract.
Direct Cargo invocations bypass this PowerShell preflight.
