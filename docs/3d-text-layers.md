# 3D text layers

Choose **Add layer > 3D text** in the icon layer palette. Text, system font, font size,
front color, and the achievement unlock-value link remain editable. The image has a
transparent background and participates in the normal layer opacity, blend mode,
mask, folder, 2D transform, and effects pipeline. It is regenerated from the recipe
for previews and exports; no external renderer or image file is required.

The camera looks down the negative Z axis. Perspective uses camera distance and a
focal length in millimeters with a 36 mm vertical film gate. Increasing the focal
length magnifies the object; increasing camera distance reduces its apparent size.
Orthographic uses view height to set the visible vertical extent, preserving size
when distance or object Z changes. Both projections clip geometry one design unit
in front of the camera.

Extrusion extends equally forward and backward from the text's local XY plane.
Object rotation applies X, then Y, then Z about the center of the glyph bounds;
translation follows rotation. X points right, Y up, and positive Z toward the camera.
Font size, extrusion, camera distance, view height, and position use the icon's
512-unit design space, independently of export resolution and aspect ratio.

**Push** offsets the letter contours by −64 to +64 design units: negative values
thin strokes and enlarge counters; positive values thicken strokes and close gaps.
Sufficient thinning can remove a stroke entirely. **Emboss** adds a 45° beveled rim
outside the pushed face, up to 64 units wide and high, on both ends of the extrusion.
The original flat face stays at its pushed size; the rim adds to the object's width
and depth. Emboss also works with zero extrusion. Its surface uses the front color;
the straight extrusion walls use the side color.

The **Output** selector offers Full color, Full mask, Face mask, Emboss mask, and
Emboss mask − face.
Masks are white geometry on transparency, independent of material colors and
material alpha. Full mask includes every visible surface. Face mask includes the
visible flat letter faces (front or back); Emboss mask includes only the beveled
rim. Other surfaces still occlude those masks, so hidden faces cannot show through.
Emboss mask is empty when Emboss is zero. Normal layer opacity, blending, effects,
and masking still apply to the selected output.
Emboss mask − face removes the flat faces from the **complete embossed object**,
keeping the visible beveled rim and extrusion walls. It corresponds to Full mask
minus Face mask, rather than subtracting the face from the already isolated rim.
With zero Emboss, this output still includes any visible extrusion walls.
The face is excluded per coverage sample before filtering; internal cutouts and
outer silhouettes receive the same antialiasing, without dark or jagged seams.
The saved output value is `EmbossMaskMinusFace`.

Matte and Glossy use an adjustable directional light plus ambient illumination.
Glossy adds a roughness-controlled highlight. Unlit shows the front and side colors
directly. Color controls use swatches; encoded colors are internal recipe data.
Glossy's **Reflectivity** slider controls highlight strength from zero to one,
independently of Roughness. The default of 0.55 preserves previous Glossy rendering.
Reflectivity is ignored by Matte, Unlit, and mask outputs.
**Letter shadows** controls directional self-shadow strength from zero to one;
letters, counters, rims, and walls can cast shadows onto the text's other surfaces.
These are shadows on the text itself, with no added ground plane. Ambient light
remains present in shadow. Unlit and mask outputs ignore lighting and shadows.
**Smooth normals** softens curved walls and interpolates the normals across the rim
to join the flat faces and walls smoothly. It changes shading without changing
silhouettes or mask geometry.

The additive schema-1 layer kind is `Text3d`, with the existing `text` record and
the `text3d` settings record. Rust validation, C# validation, project mapping, and
synchronization recipe fingerprints include the new settings. Existing numeric
layer identities retain their values. Default settings apply when `text3d` is absent.
The additive settings `push`, `emboss`, `output`, `shadowStrength`, `smoothNormals`,
and `reflectivity` default to zero, zero, `FullColor`, zero, false, and 0.55.
Existing recipes retain their
original rendering until one of the shape or lighting options is enabled.

The renderer uses centered font outlines, vector extrusion walls, depth testing,
and projectively sampled antialiased front/back coverage, including letter holes.
Small color images use 2× supersampling. All mask outputs use adaptive subpixel
coverage at silhouettes, letter holes and surface boundaries, at every export
size: 4×4 for thumbnails up to 64 pixels and 8×8 for larger images. Interior mask
pixels use a single sample, keeping quick previews responsive. Text is limited to
256 characters and a 512-unit
font size; outlines, flattened vertices, surface work, and coverage textures have
explicit bounds. Work runs on the existing bounded preview workers, checks
cancellation during geometry and rasterization, and uses the existing quick-first,
last-successful-image preview behavior.

Push, emboss, smoothing, and self shadows use a sampled signed distance field from
the same antialiased font outlines. A separable Euclidean transform handles contour
merges, erosion and counters. Field dimensions are bounded to 1024 per axis; primary
rays take at most 192 steps, shadow rays 64, with a combined 64-million-sample budget
and cancellation during field construction and rasterization. Masks share a
64-million-work budget across center and edge samples, with cancellation checks
during refinement. Lighting, projection,
camera clipping, and supersampling are shared with the original vector path.

Regression tests cover projections, lens and distance, XYZ transforms, back faces,
holes, extrusion, materials, opacity/effects, clipping, cancellation, defaults,
invalid parameters, and project history. The ignored `text3d_contact_sheet` test
writes a six-view visual sample and separate quick/final timings to external build
output.
`text3d_relief_contact_sheet` adds weight, bevel, output mask, normal, and shadow
comparisons. Its timings measure CPU image readiness, not on-screen presentation.
