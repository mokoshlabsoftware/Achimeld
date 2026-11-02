# Export icons translations styles and presets

Export creates an output for use elsewhere. Save the project separately to retain editable data.

## Desktop achievement icons

Choose File > Export achievement icons (also available in Tools). The export window lists every ordinary achievement icon, selected by default. Select all, select none, or choose any subset. Choose PNG or JPG, an API-name or numbered API-name filename, and an output folder. These choices and the folder are saved with the project when you save it. Check output width/height, 50%-100% desaturation, optional gradient map (up to 50% strength), and opacity for unachieved icons in Project settings.

The desktop path produces achieved and unachieved pairs in the selected format:

| Output | Example |
| --- | --- |
| Achieved | ULTRA_CLICK_100.png |
| Unachieved | ULTRA_CLICK_100-unachieved.png |

Numbered API names add a four-digit sequence to each selected icon, for example `0001-ULTRA_CLICK_100.png`. JPG uses `.jpg` and flattens transparency. The folder must exist. Export refuses an existing destination, so choose an empty folder or remove earlier output before exporting again.

Output dimensions follow the project settings, which expose 1-4096 per axis. API names must produce valid Windows filenames. Existing targets, case-insensitive collisions and reserved names can reject the export.

Files are staged before publication. Cancellation or ordinary publication failure has rollback handling; crash recovery for an external export is not a guarantee. Inspect completion status and the resulting pair.

Open both images and check dimensions, transparency, text at intended display size and the unachieved treatment.

## Other desktop exchanges

| Output | Route and format |
| --- | --- |
| Achievement translations | Localization or Tools export/import CSV; see [Localization](localization.md) |
| Style recipes | Style options Load/Save styles, .json; see [Styles](styles.md) |
| Gradient library | Gradient Explorer, .grd; see [Gradients](gradients.md) |
| Particle effect or library | Single effect and Explorer .achparticles workflows; see [Particles](particles.md) |
| Explosion library | Explorer .achexplosions; see [Explosions](explosions.md) |
| Legacy style collection | Supported .achv collection workflow, separate from style JSON |

Respect unused-destination rules shown by the chosen exporter. Sharing a style recipe does not automatically package every referenced project image.

## MCP icon export is a different path

The automation tool export_achievement_icons uses an existing absolute directory, optional Png/Jpeg/Bmp/Tiff, and width/height 1-4096 (default 256). It writes color and grayscale _gray pairs with sanitized names; JPEG flattens transparency on black.

That tool replaces matching output images and can leave earlier completed files if interrupted. It uses a pinned snapshot and does not change project settings or save the project. These behaviors differ from the desktop PNG exporter; choose and document the route explicitly.

MCP style preview export produces one PNG with its own dimension/destination limits. See [Automation](automation.md).

