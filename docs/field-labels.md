# Field label and abbreviation policy

Abbreviations are allowed only on surfaces whose content width the user can
resize. A fixed, unresizable UI must show complete localized labels, regardless
of **Preferences > App style > Abbreviate field labels**. This applies to field
labels, tab captions, and other descriptive controls. Established domain names
such as **Steam App ID**, **RGB**, and **ID** remain valid; do not mechanically
expand those names.

Judge the actual containing surface. A fixed-width section inside a resizable
window is still fixed. Moving a window, scrolling its body, changing a global
label budget, or dragging a label divider does not qualify as resizing that
surface. Automatic sizing to content also does not qualify.

In fixed forms, measure complete localized labels with the active font. Keep
labels to the left of their fields; wrap long labels within their column and
allow the row/body to grow vertically. Adapt form width or use bounded vertical
scrolling where needed. Do not substitute initials, ellipses, clipping, further
font shrinking, or a hover tooltip for a readable complete label. Preserve minimum control
space and avoid overlap and horizontal scrolling at supported scales.

Resizable editors retain their saved label budgets, optional multiword
abbreviations such as **Bottom height → B. Height** only when the measured full
label exceeds its column after allowing room for an icon. Expanding the column
restores full names immediately. Property labels use regular, non-bold text one
point smaller than body text, measured and painted with the same font. Complete
localized names remain available on hover. Single-word names, acronyms, and axis labels retain their meaning.
The hovered or dragged label divider spans the visible panel height with a
three-point line and three centered circular grip marks.
See [Preferences and layer layout](preferences.md) for budgets and interaction.
Prompts also follow [Application prompt windows](application-prompts.md).

## Implementation and review

The shared [`fields.rs`](../crates/achimeld-ui/src/fields.rs) row, checkbox,
icon-row and slider helpers enforce full labels inside modal, popup, menu,
tooltip and picker ancestry. Wrap other unresizable windows and fixed-width
form sections in `fields::fixed_width`; this scope applies to nested helpers
and removes the global label divider. Review alternate render paths and
localized labels whenever adding a fixed surface. Do not change document,
preference or worker protocol shapes to apply this presentation rule.

Check ordinary and enlarged fonts, display scales, long translations, narrow
available widths, enabled and disabled controls, and the abbreviation preference
both on and off. Verify full painted text without elision, label/field separation,
bounded width, cancellation, validation and existing actions. Keep regression
coverage for resizable editors so their abbreviation setting still works.

## Project audit: 2026-10-19

Reviewed application source under `crates`, `bridge`, `build`, `installer` and
`tools`, including all abbreviation code, shared field callers, fixed/auto-sized
windows, modals, popups, explicit width allocations and the manual inspector-tab
fallback. The native Achimeld UI owns all label abbreviation; bridge/build/tool
code supplies no separate UI abbreviation implementation. Vendored framework
source and generated outputs are not application UI implementations.

| Surface | Labels affected or protected | Resolution |
| --- | --- | --- |
| Sync > Edit Steam App ID | Steam App ID previously became S. A. ID | Modal ancestry now requires the full label and measures its width. |
| Sort, row filling | Achievements per row; Stats per row | Fixed form scope preserves both full labels. |
| Sort, column filling | Achievements per column; Stats per column | Same fixed scope covers the alternate direction. |
| Sort, activation option | Sort by stat activation | Fixed scope also covers the checkbox label, including Polish translations. |
| Movable color and grayscale editors | Red, Green, Blue, Alpha, Gray, Hex | Current labels do not abbreviate; fixed scope protects localized or future multiword labels and prevents budget elision. |
| Save project style / Rename style | Name | Current label does not abbreviate; modal ancestry protects it from narrow label budgets. |

Other prompts have no shared labeled property fields. Preferences, Project
settings, docked/floating Properties and Layers, Gradient/Effect Explorer,
source selection, Art Data Bank details and Style Viewer property sections
permit resizing their content width. Inspector tab fallbacks (Gen, Loc, Ico,
Sty, Img) belong to the resizable Properties surface. Fixed bars and dropdown
rows do not synthesize word abbreviations. Icon-only actions retain their
existing full accessible names and tooltips.

Validation: the complete UI suite passed **237 tests**; **40 native capture tests**
were ignored by that default run. Explicitly ran and visually reviewed the native
Steam App ID and Sort captures; both passed. The focused layout regressions cover
English and Polish, 120/220/420-point forms, 13/24-point fonts, 100/150/200% display
scales and both extreme label budgets. Existing cancellation, validation, sorting,
color-window dragging and resizable-editor abbreviation checks also passed. The
strict workspace/all-target lint gate passed. This is source and local UI evidence;
it does not claim installed-package or live Steam qualification.
