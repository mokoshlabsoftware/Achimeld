# Preferences and layer layout

Preferences uses square panel edges and shared square buttons. The five generated
section icons are embedded as white tintable RGBA artwork; original transparent
PNGs and exact built-in imagegen prompts are retained under
`crates/achimeld-ui/assets/toolbar/preferences-*` and `preferences-icons.json`.
The header mark is 16 UI points. Section rows fill their column, align their icon
and label to the left, and touch vertically. Resting unselected actions use the
panel fill; the selected section retains Accent. Save and Cancel share a full-width
bottom bar. Long pages scroll above that bar.

**General > Layer properties in right sidebar** is an optional saved application
preference, on by default, including for older settings without this option.
When enabled, it narrows the floating Layers window to the stack and its actions,
while the Icon inspector displays the
selected layer or effect controls. **Show layers panel** brings a misplaced stack
back inside the application. Both layouts use the same property editors,
validation, draft handling and undo path. The three common layer rows remain above
the scrolling type-specific controls. Local style and art-image preview inputs
remain available in a collapsible section above the sidebar properties.
An explicitly saved off value retains the combined editor.

The saved floating layout retains its combined width, so turning the option off
restores the wider editor. Existing Save, Cancel, close and Escape semantics apply
to the new preference. No project or worker protocol field is added.

**General > Display mini values in FX rows** is on by default, including for old
settings. Expanded effect rows always retain their generated type icon. Complete
mini editors appear only when they fit: Outline shows width followed by a color
swatch, other effects expose their main value(s). Values support typing and
horizontal dragging; swatches open the movable color editor. Effect names yield
to the controls at narrow widths; parameter labels appear only with spare room.
Hover icons to read their complete names. Turning the option off keeps effect
icons and names without inline values. Edits share normal validation and undo.

The add-effect square is centered beneath FX, halfway to the first tree junction.
Select the layer row or thumbnail to return to its properties; the effects pane
has no Back to layer action. Inner light exposes its actual offsets, cutout,
outline inclusion and independent soft-light color, opacity and blend controls.
New inner lights use white Screen light, a downward offset of 8 and blur of 12.
Outer light uses width/spread and blur outside the artwork, with white Screen
light by default. Existing saved inner-light recipes retain their settings.

**App style > Field labels** configures the shared property-row layout in resizable
editors, subject to the [Field label and abbreviation policy](field-labels.md).
Labels always stay on the left of their controls, including in
narrow panels. The label column defaults to the larger of **90 logical pixels**
and **15% of the complete label-and-field row**. The minimum wins when those two
budgets conflict. At widths too small to hold the minimum plus a control, the
label column shrinks and truncates rather than overflowing or stacking above it.
The minimum width (40–240 px), percentage (5–50%), and label abbreviation option
are saved application preferences. Older settings receive the defaults.

Hover the gap between a resizable editor's property label and its field to reveal a vertical
divider and resize cursor. Drag horizontally to change the shared ratio in every
property editor. The application saves the change once on release; dragging inside
Preferences updates that draft, preserving Save and Cancel. The drag uses a 40 px
safety minimum so the previous 90 px minimum cannot block a narrower label column.
The divider is invisible at rest and uses the existing gap.

Color editors are movable windows with a title-bar drag target and visual controls
at 70% of their previous dimensions. Their **Hex** field accepts `#RRGGBB` and
`#AARRGGBB` (with or without `#`); invalid partial input retains the current color.
This field is the explicit exception to swatch-only color presentation elsewhere.

Abbreviations are enabled by default in resizable editors: for example,
**Bottom height → B. Height**. Fixed, unresizable forms always show complete labels,
even with this preference enabled or a narrow global label budget. They measure
the full label and wrap it on the left when necessary, without a label divider.
Field labels show the complete localized name on hover, including when truncated;
single-word names and axis labels remain intact. App style includes a live sample.
Save applies these settings and Cancel, closing the window, or Escape discards
the preview using the existing preference behavior.
