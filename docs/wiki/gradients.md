# Gradients color maps and Gradient Explorer

Gradient presets use a folder button to open Gradient Explorer and an adjacent quick load menu with visual swatches. Explorer keeps an Unsaved changes entry from the current gradient; Save gradient creates a new preset from it, and Apply to layer uses the selected draft. The Icon size knob adjusts the preset cards. Explorer and 2D Gradient Collection panels have square corners. Rebuilt desktop screenshots and runtime verification are pending.

A gradient defines colors and transparency across a range. Its placement depends on the tool using it: a linear/radial image, an intensity map, a particle's color slice or a shape's distance field.

## Edit a one dimensional gradient

Drag a stop along the ramp to move it. Click it to edit its color/opacity and position. Double-click empty ramp space to add a stop sampled from the existing gradient; right-click creation is also available. Delete a selected stop through its controls. The final stop is protected.

The shared editor allows up to 128 stops. Reverse gradient mirrors positions and transparency.

The ordinary Gradient layer editor offers Linear or Radial and Angle -360 to 360 degrees. A gradient-overlay adjustment/effect additionally chooses RGB, Red, Green or Blue as its input channel. A vignette map follows fade strength and has no spatial-angle control.

## Gradient Explorer

Open Tools > Gradient Explorer, or use the folder button in a gradient layer.

Browse, search, create, duplicate, edit and save named gradients. Plain-click selects; Shift-click adds a tile; Alt-click subtracts it. Delete selected or Delete with tile focus removes the group. Undo delete restores the removed group. Search keeps selections that remain visible.

Switching presets, starting a file operation or closing saves the current valid library draft. Repair invalid names before leaving it. Library edits are separate from project undo; choosing a preset for a layer applies it through project history.

## Import and export

Import reads supported Photoshop .grd sets. Export selected and Export all write .grd files, preserving names, colors and transparency. Choose an unused destination.

The importer supports solid RGB gradient sets, not legacy version-3 files, noise gradients, dynamic foreground/background stops or non-RGB spaces. The limits are 8 MiB, 512 presets and 128 combined stops per gradient. Unsupported files produce errors without a partial library import.

Export quantizes positions to the format's 1/4096 steps. Achimeld's interpolation does not promise identical Photoshop pixels.

## Particle two dimensional color map

A particle map's vertical axis describes the gradient along a particle; its horizontal axis varies between particles. Each particle takes a stable seeded vertical slice.

Click a point to select it, Shift-click to add, Ctrl-click to subtract. Drag empty map space for a selection box; drag selected points to move the group. Arrow keys nudge; Shift gives a larger step. Delete removes selected points. Double-click empty space adds a sampled point.

Selected-point controls include color, position, Power and X/Y influence. Group position changes retain spacing. All initial points can move or be deleted; an empty map is transparent. The limit is 64 points.

Click-release on empty space or the Test slice knob positions a helper guide without changing the effect. Reverse gradient flips the vertical direction while retaining horizontal variation. See [Particles](particles.md).

## Color window

Click a swatch to open the movable color editor and drag its title bar to reposition it. Use visual controls, opacity and RGBA values. Hex accepts RGB or alpha-inclusive forms such as #RRGGBB and #AARRGGBB, with or without #. Invalid partial text keeps the previous valid color.

