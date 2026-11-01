# Icon effect reference

Expand a layer's FX section and use Add effect. Select an effect row to edit its properties; select the layer row or thumbnail to return to layer properties. Drag the effect's selector to change its order.

Effects run in their displayed order. Enable/visibility, opacity and removal affect that effect. Editing a mini value or color swatch is separate from dragging the selector.

## Appearance effects

| Effect | Controls exposed by the property editor | Result |
| --- | --- | --- |
| Color overlay | Color, opacity, blend mode | Colors the layer's coverage |
| Outline | Color; Width 0-64; Placement Center/Inward/Outward; opacity and blend | Adds an edge around coverage |
| Shadow | Color; Offset X/Y -512 to 512; Blur 0-128; opacity and blend | Adds an offset soft silhouette |
| Inner light | Color; Offset X/Y -4096 to 4096; Cutout 0-256; Affect outline; Soft light and its controls | Adds directional light inside coverage |
| Outer light | Color; Width 0-64; Blur 0-128; opacity and blend | Spreads and softens light outside coverage |
| Box blur | Blur 0-128; opacity and blend | Softens the layer |
| Gradient overlay | Input channel RGB/Red/Green/Blue; gradient placement, angle, stops and presets; opacity and blend | Applies a gradient treatment to the layer |

These are the editor's ranges. Do not interpret a broader file-format limit as an additional UI slider range.

Inner light's Soft light switch enables Blur, Light color, Light opacity (0-100 percent) and Light blend. These settings are separate from the effect's main color, opacity and blend mode. New inner lights start with white Screen lighting, downward offset 8 and blur 12; existing saved recipes retain their values.

Outer light appears outside the source; increasing its width or blur cannot expand the output canvas. Leave room around artwork when you want its glow retained.

## Spatial effects

Spatial effects have opacity to reduce their strength and omit the ordinary blend-mode control.

| Effect | Controls | Behavior |
| --- | --- | --- |
| Skew | Horizontal/vertical -60 to 60 degrees | Applies centered horizontal then vertical shear |
| Perspective | Horizontal/vertical -45 to 45 percent | Tilts around the canvas center |
| Pinch | Amount -100 to 100 percent | Positive bulges the center; negative pinches inward |
| Lens blur | Edge blur 0-128; Chromatic aberration 0-64 | Keeps the center clearer while softening and separating red/blue at edges |

Skew and Perspective start at zero, Pinch at 50 percent, and Lens blur at blur 4/aberration 8. Distortion leaves canvas dimensions fixed and samples outside it as transparency. Reduce distortion, move or scale the artwork if important details clip.

## Similar names with different scopes

A **Gradient layer** paints its own image. A **Gradient overlay adjustment** maps the backdrop below its stack position. A **Gradient overlay effect** belongs to its owning layer.

**Box blur** softens a layer. **Lens blur** increases edge softening and color separation. The **Radial blur adjustment** acts on the composited canvas; Explosion's radial/angular blur acts on the construction before projection.

## Mini controls

Preferences > General > Display mini values in FX rows controls compact row editors. A narrow panel can hide mini values or names without removing the full properties. Hover the effect icon for its name.

For a useful comparison, keep the source and framing fixed, capture the effect disabled, then enabled. Change order separately when comparing two effects.

