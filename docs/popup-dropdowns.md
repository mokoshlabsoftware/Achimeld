# Popup dropdown guide

This is the appearance and interaction contract for every Achimeld dropdown. It applies to property choices (including Shape), preset lists, language and font selection, layer menus, saved-style menus, title menus, nested menus and context menus. It complements [Upper application bars and Style Viewer](upper-application-bar.md), [Application prompt windows](application-prompts.md), and [AGENTS.md](../AGENTS.md).

## Appearance

- Use `dropdowns::Dropdown` for ordinary value/preset lists and `buttons::menu`, `menu_popup`, or `context_menu` for custom contents. Do not introduce direct framework ComboBox controls or private popup styles.
- Popup panels and every button body have square corners, no outline, no outer padding, and zero spacing between adjacent button bodies. Rows fill the list width. Insets belong inside a button or search field. Preserve established bar heights.
- A resting row, including the current value, uses exactly the popup background. A closed trigger uses exactly its containing panel background. Do not add a selected Accent rectangle to dropdown items; selection remains available as widget metadata and the collapsed value. Accent identity on application tabs is unchanged.
- Hover and keyboard focus exchange the row's own body and text colors. Use the shared reversible 0.15-second body and 0.2-second ink animation, its contrast-preserving path, and disabled treatment. Actual glyphs and icon tint must use the animated ink without changing font weight, width, or layout.
- Button, body and caption text inside a popup are one logical point smaller than their corresponding configured application fonts. Resolve from the application font each time; nested menus must not reduce it again. Ordinary collapsed choices also use button text one point smaller.
- Menu labels align left and center vertically. Center icon-only glyphs within their bodies; retain icon aspect ratios. Where a control has generated artwork, use the embedded asset and tint it with the same ink as its label. The blend property has its own icon, and every supported blend mode has a distinct icon.
- Show complete short lists together. The eight blend modes always appear in one list with no ScrollArea. Long catalogs (system fonts, styles, languages, project references) may scroll to stay inside the viewport. Preserve searches and virtualized preview lists; do not request previews for hidden catalog rows merely to support navigation.

## Input ownership and dismissal

Up and Down move through enabled entries in logical list order, including entries outside the visible area, and bring the candidate into view. Stop at the first/last entry. Enter or Space activates the candidate once. Moving the candidate alone does not commit a value or create undo history. Disabled controls cannot activate. Disabled ordinary value/font pickers close their list; an already open title menu retains its existing inert display without hover switching. Tab can still reach search fields and auxiliary style actions.

Actual pointer position changes return control to the mouse. With a stationary pointer, keyboard selection remains authoritative through repaints, repeated pointer events at the same position, and repeated layout passes. Do not continually select the row beneath an unmoved pointer. Moving outside the list clears mouse preview. Keyboard focus and pointer hover must not display two simultaneously active rows.

Escape and outside clicks dismiss without applying a candidate. Preserve each menu's existing opening, hover-switching, explicit command closing, nested-menu and cancellation semantics. Clicking or typing in an embedded search/property field must retain that field's normal input; in the font search, Up/Down transfers to its results. Application prompts and standard Windows file/folder dialogs keep their separate contracts.

## Blend preview

Both the layer-list control and the pinned property-row control use `blend_preview`. This applies to images, solids, shapes, text/3D, particles, explosions, gradients, noise, folders and adjustments. Hovering or keyboard-selecting another mode temporarily renders that layer with the candidate blend mode against the current composition. Click or Enter commits through the ordinary editor and undo path. Escape, outside dismissal, pointer departure, focus loss, disabled editing, selection changes and document revisions clear the temporary preview.

Effect blend properties, including the light/blur blend choice, reuse the same complete icon list and appearance. Their existing defaults remain intact.

Keep previews outside the saved recipe. Reuse the host's bounded quick/detail renderer, cancel obsolete work, retain the last successful visible image while its replacement is pending, and preserve asset I/O ownership. No protocol operations, policy authority or new filesystem paths are needed for this presentation behavior.

## Project inventory and migration

The October 18, 2026 source audit covered application UI sources under `crates`, and checked `bridge` for application dropdown controls. Browser automation popup checks in the constrained bridge describe remote-browser enforcement, rather than an Achimeld UI dropdown.

Eleven direct framework dropdown construction sites were replaced: ten now use the ordinary shared Dropdown, and the blend property uses the shared icon/preview menu. Existing custom dropdown, nested-menu and context-menu families now inherit the same appearance and navigation. A regression guard scans the UI source tree for direct framework dropdown construction.

| Surface | Source / shared implementation |
| --- | --- |
| Generic properties: Shape, transforms, gradients, adjustments, effects, sorting and preferences | `inspector::choice` → `dropdowns::Dropdown` |
| Layer/folder/mask references | `layers::reference_choice` → `dropdowns::Dropdown` |
| 3D artifact output, presets and part/primitive choices | `artifact3d` → `dropdowns::Dropdown`; shared command menus |
| 3D text output | `text3d` → `dropdowns::Dropdown` |
| Particle effects and composite shape presets | `particles`, `particle_shapes` → `dropdowns::Dropdown` |
| Color-map presets | `gradient_map` → `dropdowns::Dropdown` |
| Language | `settings` → `dropdowns::Dropdown` |
| Layer-list and property blend choices | `blend_preview`, `blend_icons` → shared menu/buttons, all eight rows |
| Font search/results | `font_picker` → shared popup/buttons and full logical order for virtualized rows |
| Project style previews and actions / global style submenu | `style_picker`, `layer_palette` → shared menus, virtualized logical order |
| Achievement general choices | `achievement_general` → shared menus/buttons |
| Explosion curve, effect presets, gradient presets, layer/effect creation and actions | `explosion`, `effect_explorer`, `layers`, `layer_palette`, `layer_effects` → shared menus/buttons |
| Title headings and nested commands | `chrome`, `desktop` → shared menus/buttons |
| Canvas, artwork bank, General images, gradient-stop and Style Viewer context actions | `canvas`, `art_bank`, `general_images`, `gradient`, `style_viewer` → shared context menus/buttons |

The shared popup style lives in `buttons.rs`; typography and ordinary dropdown construction live in `dropdowns.rs`; ordered navigation lives in `dropdown_navigation.rs`. Custom-painted menu rows must obtain their colors through `buttons::colors` so they obey the same resting and animated fills.

## Verification

Keep regression coverage for square/borderless gapless geometry, equal resting fills (including the current entry), one-point font reduction, enabled/disabled entries, long-list keyboard scrolling, keyboard-to-mouse transitions, stationary mouse ownership, Enter commit, Escape cancellation, and blend previews for every layer type. Check fractional display scales and larger configured fonts. Existing title-menu switching, nested-menu and command tests remain required.

Use the external build tree for logs and native screenshots. Headless geometry/input regressions and native visual checks provide different evidence; record unrun or blocked checks explicitly. A passing source test does not qualify an installed package or live Steam behavior.
