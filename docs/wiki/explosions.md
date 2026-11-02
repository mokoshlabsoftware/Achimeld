# Explosion layers and Explosion Explorer

Add Explosion from the layer picker. The effect builds shapes on a periodic rectangle and maps that rectangle around a circle. The construction's top becomes the outside, and its bottom becomes the center.

New explosion layers start with the bundled boom recipe. Existing layers and saved presets keep their own settings.

Explosion Explorer thumbnails render each saved recipe at 96 by 96 pixels. A tile accepts only pixels from its own current recipe; the full preview can retain its previous image while a new render completes. MCP can list named presets with their exact layer recipes and apply one preset to selected achievements in one undo step.

The construction preview explains the source shapes; the main preview shows the projected result. The construction strip grows with the Properties panel. Open construction editor for a resizable window with the explosion controls, a larger strip, and whole-effect Offset X, Offset Y and Angle. Edits update the selected layer immediately.

## Main controls

| Control | Range and effect |
| --- | --- |
| Objects | Exact count 1-512, or an inclusive range when Count range is enabled |
| Regularity | 0 equal spacing; 1 independent random placement |
| Seed / Randomize | Reproduce or replace sampled geometry |
| Different seed for each achievement | Stable variation per achievement with a shared base seed |
| Overall scale | 1-200 percent for the completed explosion |
| Shape X/Y | Independent 1-10 times object stretch |
| Base brightness | 0-200 percent, multiplied by each object's brightness variation |
| Cover canvas | Radius uses half the canvas diagonal to reach corners; off uses the shorter half-edge |
| Invert colors | Reverses gradient RGB without moving stops or changing alpha |

The action bar also provides masks, presets, Count range and Bottom scaling curve. Hover icon-only actions for their names.

## Shape foldout

New layers use a Deltoid to Ellipse morph: -100 percent curves the sides inward, 0 percent is a kite, and 100 percent is an ellipse. Top height and Bottom height independently sample the distances from the waist. At a kite's zero end height, that end becomes triangular; both heights zero make an empty shape.

Width, Offset, Sharpness and Random scale use ranges. Global scale multiplies width and both heights together. Global/random scale run 1-200 percent; Offset runs -1000 to 1000 percent, with positive values moving the waist inward. Sharpness runs soft 0 to crisp 1.

The sampled construction aspect ratio is between 1:1 and 10:1. Width, heights and shape offset use the rectangle height as their unit. A range-bar drag retains the separation between its ends.

## Fill the center

Fill power adds 0-900 percent width toward the construction bottom, up to 10 times. Fill stretch defines how far upward the change reaches: 50 percent starts halfway, 100 percent starts at the top, and zero disables widening.

Linear, Quadratic and Logarithmic curves reach the same final multiplier. Quadratic concentrates growth near the bottom; Logarithmic starts growing earlier. Widening stays centered on each shape and wraps at the seam.

## Blur and gradients

Radial blur and Angular blur run 0-100 percent before projection. Radial spreads vertically in the strip; Angular spreads horizontally and wraps. These differ from the [Radial blur adjustment](icon-layers.md), which blurs the completed backdrop.

Radial and Linear gradient modes use final canvas coordinates. SDF colors the original shape from its boundary toward its deepest interior. Select a gradient stop and use Stop alpha to set transparency from 0 to 100 percent. Stop transparency multiplies coverage. Reverse gradient mirrors stops and their alpha.

Base brightness, per-object Brightness and gradient colors are separate multipliers. A dark result can come from any of them.

## Whole effect transforms and older recipes

Offset X/Y and Angle move the completed explosion. They are different from the sampled shape Offset.

Older triangle recipes retain their appearance until Use deltoid is selected explicitly. Opening the editor does not convert them. Older stored overall-scale ranges retain their sampled result until that control is edited.

## Presets

Explosion Explorer keeps the live preview visible on the left while the preset library and editing controls scroll on the right. The library displays a grid of rendered effect previews with names below the images, at a default size of 96 by 96 pixels. It supports search, save, duplicate, rename, removal/undo removal and .achexplosions collection import/export.

When exporting a collection to an existing file, confirm Replace to update it, or choose another filename. The existing file stays intact if export fails.

Presets include shapes, gradient stops/alpha, variation ranges, seed behavior, polar fit, transforms, opacity, blend and effects. Applying a preset preserves the target's identity, name, folder and mask links and participates in undo.

Use a matching before/after frame when comparing Fill power, Cover canvas or gradient placement; they change different parts of the construction.


## Preset files (working change 0.2.29.23)

The folder button opens Explosion Explorer directly. The adjacent quick load button shows saved preset icons and applies a choice in one click. Randomize and per-achievement seed complete the shared top bar; explosion-specific actions sit on the bar below. The Explorer starts with an Unsaved changes entry copied from the layer. Save preset creates a new library preset from this entry; Apply to layer applies the selected draft. Icon size has its own knob, and preset panels have square corners. Export selected in the Explorer writes the complete effect, including its embedded gradient, blend, transforms and effects. Particle files use `.achparticles`; explosion files use `.achexplosions`.

**Import presets** in the Explorer opens a Windows picker. Imported presets enter the library; use quick load or Apply to layer to apply one. Importing alone does not replace the layer. Desktop verification and updated screenshots remain pending.
