# Particle layers and Particle Explorer

Add Particles from the layer picker for procedural static artwork. A seed reproduces the layout; this is not an animation system.

For a simple illustrated starting point, follow [Add a simple particle overlay](first-achievement.md#add-a-simple-particle-overlay).

## Count layout and variation

The top controls include Count, Seed, Size, Rotation, Shape X/Y, Opacity, SDF softness and Regularity. Size, Rotation, Opacity, Softness and spawn radius have minimum/maximum controls.

Drag an end independently or drag the range bar to move both ends together. Arrow keys adjust a focused control; Shift increases the step.

| Control | Meaning |
| --- | --- |
| Count | Number of particles, up to 512 |
| Seed / Randomize | Reproduce or replace the sampled layout |
| Shape X/Y | Stretch the composite in its rotated local axes, each 1-10 times |
| Regularity | 0 gives even placement; 1 fully random placement |
| Spawn center | Positions the disk/annulus through diagram or X/Y controls |
| Minimum radius | Creates an inner exclusion circle |
| Maximum radius and distribution | Set outer extent and bias toward inner or outer areas |

Particle diameters use the 512-unit artwork space; spawn center/radii are relative to layer space. The enclosing layer transform moves the completed effect.

The square spawn diagram expands with the width of the right-hand editing tab and stays centered as the tab is resized. Drag in the diagram to move the spawn center, or use Center X and Center Y below it.

## Build the particle shape

Every particle uses the entire ordered composite of up to 16 shape parts. Add a sphere, polygon or star, then edit its scale, position, rotation, sides and star inner-radius range.

The first part starts the shape. Later parts use Union, Subtract or Intersect. A ring can be made from a sphere with a smaller sphere subtracted. Composite normalization keeps Size describing the total particle diameter.

Polygons/stars support up to 16 sides/points. Star inner-radius bounds run from 5 to 100 percent. Changing shape stretch or inner-radius sampling preserves other independent sampled values.

## Color each particle

The MagicDust starting effect uses a 1D radial gradient. You can convert it to a 2D color map, where horizontal position varies between particles and a vertical slice supplies the complete gradient within one particle. Choose Linear or Radial placement and a linear angle where relevant.

Use [Gradients and color maps](gradients.md) for selection, point editing, Power, influence, Test slice and Reverse gradient. Black to white, Aurora, Soft spectrum, and Warm and cool are map presets.

Older effects without a 2D map retain their existing 1D gradient and placement. Use a 2D color map explicitly converts the color source using the previous stop colors.

## Seeds across achievements

Different seed for each achievement derives a stable variation from each achievement identity while retaining a common base seed. Duplicating an achievement gives the copy its own variation.

Randomize changes the base seed. Applying an effect preset preserves the current seed and per-achievement switch; loading a saved effect restores its saved settings. Re-roll all random layers affects the current icon's random sources together.

## Single effects and collections

Before working change 0.2.29.23, Load effect and Save effect used single-effect .achparticles files containing particle settings, component shapes, map points, variation ranges and seed behavior. Common layer transforms, blending, folders and other layer effects remained in the enclosing recipe. For the newer toolbar and complete preset exports, see Preset files below.

Particle Explorer, available from Tools or the layer's folder button, keeps the live preview visible on the left while the preset library and editing controls scroll on the right. The library displays a grid of rendered effect previews with names below the images, at a default size of 96 by 96 pixels. It manages named presets with search, save, duplicate, removal and undo removal. Its .achparticles collections also include layer opacity, blend, transforms and effects. They can import earlier single-effect files.

MagicDust is bundled with the application as a Particle Explorer preset and is the starting effect for newly created particle layers. Its layer uses Screen blending. Existing particle layers keep their saved settings, and existing saved presets remain in the library.

When exporting a collection to an existing file, confirm Replace to update it, or choose another filename. The existing file stays intact if export fails.

Applying an Explorer preset preserves the target layer's identity and folder/mask links and uses normal undo. Single-effect input is limited to 256 KiB; map points to 64. An invalid file is rejected before applying a partial result.

Start with a small count and simple shape. Check the result at icon size before adding fine detail.


## Preset files (working change 0.2.29.23)

The folder button opens Particle Explorer directly. The adjacent quick load button shows saved preset icons and applies a choice in one click. Randomize and per-achievement seed complete the shared top bar. The Explorer starts with an Unsaved changes entry copied from the layer. Save preset creates a new library preset from this entry; Apply to layer applies the selected draft. Icon size has its own knob, and preset panels have square corners. Export selected in the Explorer writes the complete effect, including its embedded gradient, blend, transforms and effects. Particle files use `.achparticles`; explosion files use `.achexplosions`.

**Import presets** in the Explorer opens a Windows picker. Imported presets enter the library; use quick load or Apply to layer to apply one. Older single-effect particle files remain importable. Importing alone does not replace the layer. Particle composite shapes have a Shape Explorer and quick load menu; a shape preset changes the composite only. Desktop verification and updated screenshots remain pending.
