# Style banner standard

The compact style selector in the General view is the visual reference.
A style is represented by a wide rectangular **banner**, with its rendered art
in the background and its style name in front. “Icon banner” is internal
implementation terminology, not a caption for users.

## Presentation

- Show only the style name, or the localized **No style** placeholder.
  Never prefix it with “Icon Banner”, including translated equivalents.
- Fill the entire banner with artwork using a centered crop that preserves
  aspect ratio. Do not shrink the icon into a square thumbnail inside the row.
- Overlay the name on the art, left-aligned and vertically centered.
  Use a translucent application-palette fill behind the foreground text to keep
  it readable, preserving the shared hover, focus and selected-state colors.
  Do not create a separate name band below the image.
- Stack style banners in a single vertical list with **zero spacing**:
  adjacent bodies touch, with square corners and no padding between rows.
  Collection headings and the explicitly requested built-in divider remain
  structural boundaries, not blank spacers between styles.
- Style Viewer retains its banner-height control. The default is 64 logical
  points; height is capped relative to list width so banners remain wide.
  Changing height changes how much background artwork is visible.
- The main selected-style preview shows the complete image. The artwork picker
  contains rectangular image tiles; it is not a style list.

## Shared implementation and behavior

[style_banner.rs](../crates/achimeld-ui/src/style_banner.rs) paints the common
artwork/name treatment for Style Viewer and the project-style dropdown.
The compact General-view selector retains its existing selector affordance and
shows the unprefixed name.

Use the shared button implementation for hit testing, keyboard focus, hover,
selection and context menus. Visible rows request previews before selection;
keep asynchronous rendering, bounded caches and last successful previews.
Names must remain legible while a preview is missing or loading. Preserve style
identity, collection ownership, rename/import/apply actions and undo semantics.
Banner presentation does not authorize importing or applying a style.

Regression checks should cover adjacent row boundaries, artwork behind the name,
absence of the internal prefix, readable hover/selection states and display scales.
