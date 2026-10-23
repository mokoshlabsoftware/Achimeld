# Grayscale masks

Every artwork, adjustment and folder layer can own a mask. Right-click a layer
and choose **Add mask** to create any supported source type. Its mask appears as
a compact, indented entry directly below the owner in Layers. Click the entry
to edit its name, grayscale content and transform. The selected artwork or mask
thumbnail has a contrasting outline that follows the active sublayer. A folder
mask exposes its ordinary child layers in the tree. Mask roots have no opacity, blend mode,
effects or nested-mask controls. Procedural presets change their content without
adding those controls. Existing saved values retain their rendering behavior.

Right-click a layer or its mask entry to edit, replace, invert, remove or delete
the mask. **Remove mask** detaches it; **Delete mask** deletes the source and
clears direct users. **Use grayscale mask** explicitly converts a legacy Alpha
mask to luminance. Mask and folder management are absent from the bottom of
layer properties; folder moves remain available through the tree and context menu.

New masks use luminance multiplied by transparency: white reveals, black hides,
and gray partially reveals. Imported artwork is converted to grayscale by the
compositor; original Art Data Bank assets retain their identities and colors.
Mask color editors expose gray and alpha, including grayscale conversion of pasted
hex colors and nested preset colors. Normal artwork keeps its color editor.

**Reference mask to** on a mask entry assigns a live reference to another layer
or folder. **Use mask from** on a destination layer assigns the same reference
from the other direction. Both menus omit mask roots and dependencies that would
create a cycle. References follow the source owner's current mask, including
replacement and inversion, without copying its visible artwork. Creation,
replacement, referencing, inversion, removal and deletion each use one undo step
and support redo. Opening or dismissing a menu does not modify the recipe.

Reference mask is also available as a mask source type. Its **Source layer**
picker can reassign the reference. An unassigned reference, or a source without
a mask, fully reveals. Removing a source layer clears the reference. The source
layer's artwork, visibility and opacity do not become mask coverage.

Saved recipes add the `MaskReference` layer kind and optional `maskSourceLayerId`
identity. The kind is appended to the managed enum; existing numeric identities
are unchanged. Rust validation and the managed model reject missing identities
and cycles through mask, reference and visible folder dependencies. Rendering
retains shared depth, pixel-work and cancellation limits. These are recipe fields;
host file ownership, supervision operations and process recipes are unchanged.

Copying a layer bundle keeps a live reference to its existing source without
duplicating the source's visible artwork. Pasting into a recipe without that source
clears the reference for reassignment. References inside a copied whole recipe,
achievement duplication, style application and style signatures remap identities.
Project JSON and the managed version-one mapper retain the field.
Existing Alpha masks keep their coverage on loading; Use grayscale mask explicitly
converts one to luminance. New masks and source replacements use luminance.

Regression coverage includes creation on every layer type, reduced mask properties,
right-click references to layers and folders, cycle rejection, narrow nested rows,
folder-mask contents, preset control isolation, grayscale color editing and thumbnails, reference
coverage and source changes, missing/cyclic references, bundle remapping, deletion,
undo and redo, and managed model/DTO compatibility. Native screenshot fixtures
write grayscale and reference property previews to the external current build tree.

Earlier verification on October 18, 2026 (before the context-menu workflow): all 84 application tests and 90 compositor tests
passed; ten unrelated compositor visual fixtures remained ignored. Eleven mask UI
regressions and all four color-editor regressions passed. Both native mask captures
were run explicitly and inspected. The final copy/paste reference regression was
rerun after its adjustment. Managed browser/model and synchronization fixtures,
Rust formatting, and the workspace/all-targets strict lint gate passed.

The shared UI test executable was held by another running test process, so mask
UI checks used a separately named executable in the same external build tree.
The full `Audit.ps1 -Offline -Fixtures` run, installed enforcement and live Steam
qualification were not run for this editing change.
