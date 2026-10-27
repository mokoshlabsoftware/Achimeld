# Clean-slate desktop UI recipe

This is a reusable specification for building a different application with Achimeld's visual and interaction style. Replace achievements, stats, Steam and icon recipes with the new application's own objects and tools. Keep the shell, shared controls, selection-driven inspector, semantic colors, predictable menus and immediate visual feedback.

The recipe is self-contained: another app can implement it without this repository. The source map at the end identifies the implementations and design guides used to extract it.

**Source review:** 2026-10-20, against the current working tree, including existing uncommitted work. This was a UI source survey and design extraction, covering the main application modes, shared widgets, specialized editors and host integration. It was not a fresh native visual test or a security qualification. Requirements below describe the reusable target; the final audit notes identify source exceptions that should not become accidental requirements.

## Contents

1. [The visual identity](#1-the-visual-identity)
2. [The application shell](#2-the-application-shell)
3. [The title/menu row](#3-the-titlemenu-row)
4. [Navigation and every other button row](#4-navigation-and-every-other-button-row)
5. [Buttons and reversible color inversion](#5-buttons-and-reversible-color-inversion)
6. [Dropdowns, nested menus and context menus](#6-dropdowns-nested-menus-and-context-menus)
7. [The selection-driven inspector](#7-the-selection-driven-inspector)
8. [Property rows and field labels](#8-property-rows-and-field-labels)
9. [Sliders and range sliders](#9-sliders-and-range-sliders)
10. [Colors, gradients and visual editors](#10-colors-gradients-and-visual-editors)
11. [Style banners and preset libraries](#11-style-banners-and-preset-libraries)
12. [Preferences and the palette](#12-preferences-and-the-palette)
13. [Localization and typography](#13-localization-and-typography)
14. [Prompts, file pickers and pending actions](#14-prompts-file-pickers-and-pending-actions)
15. [Canvas, selection and camera behavior](#15-canvas-selection-and-camera-behavior)
16. [Layers, effects and asset banks](#16-layers-effects-and-asset-banks)
17. [Generated icons and embedded artwork](#17-generated-icons-and-embedded-artwork)
18. [Responsive previews and bounded work](#18-responsive-previews-and-bounded-work)
19. [State, commands, undo and ownership](#19-state-commands-undo-and-ownership)
20. [Implementation order for a new app](#20-implementation-order-for-a-new-app)
21. [Acceptance checklist](#21-acceptance-checklist)
22. [Source map and audit notes](#22-source-map-and-audit-notes)

## 1. The visual identity

The style is a compact desktop workspace: dark neutral surfaces, bright readable text, restrained blue selection, square adjoining controls, visual previews and a property inspector. The workspace gets the available space; actions sit in disciplined bars around it.

The important rules are:

- Use a small shared component set. A new panel inherits the same buttons, bars, fields, menus and preview behavior.
- Make adjacent button bodies touch. Their resting backgrounds merge into the containing bar or popup.
- Use filled color inversion for hover and keyboard focus. Keep labels, icons and geometry stable during animation.
- Represent visual choices visually: color swatches, generated symbols, gradient strips, artwork thumbnails and style banners.
- Keep property labels to the left of their controls. Width pressure changes wrapping, available detail or scrolling without moving labels above fields.
- Make the right inspector reflect the active selection. Avoid separate, competing copies of the same property editor.
- Show a useful preview quickly, then refine it. Retain the last successful image while work continues.
- Localize chrome independently of user-authored content. Theme chrome through named semantic colors.

### Square-edged profile for the new app

Use **zero corner radius** for the new app's buttons, bars, tabs, menus, prompts and panels. Keep slider knobs circular: their shape identifies a draggable handle.

Achimeld already forces square corners on title/menu controls, window actions, bar buttons, dropdowns and prompt action rows. Its ordinary standalone controls can still use the saved radius preference, whose source default is 6. For a consistently square new app, set the general radius to 0 as well. If a radius preference is retained, explain which ordinary controls it affects; structural bars and menus stay square.

### Baseline measurements

All measurements are logical UI points unless explicitly marked physical pixels. Measure text using the active font and scale; these values are starting contracts, not a reason to clip translations.

| Item | Baseline |
| --- | --- |
| Title/menu panel | 24 high; 8 left inset; no right or vertical inset |
| Title menu heading | Minimum body height 20; horizontal padding 12; vertically centered |
| Window controls | 28 wide by 24 high each; no gaps |
| Normal/button font | 13; default Windows face Segoe UI |
| Heading / small font | 18 / 11 |
| Ordinary button padding | 12 horizontal, 7 vertical |
| Ordinary minimum interaction height | 32 |
| Default property-row gap | 7 vertical; shared general horizontal spacing 8 |
| Main toolbar icon | 20 square in a bar that reserves a 24-point icon allowance |
| Main tab icon-to-label gap | 5 |
| Default inspector width | 288; resizable |
| Property label budget | Greater of 90 or 15% of the complete row, subject to available field space |
| Hover/focus animation | Body 0.15 seconds; text/icons 0.20 seconds |

## 2. The application shell

The uppermost row is a **title and command-menu row**. File, Edit and Tools are menu headings, not document tabs. The next row is **view navigation and actions**. The Properties tabs belong to the selected object.

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ Mark │ File Edit Tools Windows Help     drag space     Saved │ _ □ ×      │
├────────────────────────────────────────────────────────────────────────────┤
│ [View A][View B][Asset bank][Style viewer][Undo][Redo] [Filter]  Name [Act] │
├──────────────────────────────────────────────────┬─────────────────────────┤
│                                                  │ Properties              │
│ Main workspace / canvas / active tool             ├─────────────────────────┤
│                                                  │ [General][Text][Visual] │
│ Floating tools may cover this area                ├─────────────────────────┤
│                                                  │ Selected object's fields│
│                                                  │ and nested properties   │
├──────────────────────────────────────────────────┴─────────────────────────┤
│ Zoom out · percentage/reset · zoom in · Fit all │ Status          Counts   │
└────────────────────────────────────────────────────────────────────────────┘
```

The shell stays in place while the central mode changes. Achimeld uses Achievements, Stats, Sync, Steam, Art Data Bank and Style Viewer; a different app supplies its own useful modes. Do not copy integrations solely to preserve the appearance.

The current implementation is a native Rust desktop UI using egui/eframe and a Glow renderer. The recipe is framework-neutral: a different UI stack should reproduce these component/state contracts rather than imitate framework defaults. The source's dependency pins describe this checkout, not a requirement to choose the same toolkit for a new app.

The inspector is present in applicable editing modes. Full-workspace tools such as an asset bank or style viewer can supply their own detail layouts. Style Viewer is a main mode beside the asset bank, rather than a floating utility window.

Separate three kinds of state:

| State | Examples | Expected lifetime |
| --- | --- | --- |
| Document | Objects, properties, layers, style references, owned assets | Saved project and undo history |
| Application preferences | Language, palette, fonts, property layout options | Saved across projects |
| Workspace/session | Active view, selection, popup candidate, inspector placement, open tools | Explicit session/layout policy |

Achimeld saves some camera and preview-input data in the project. Docking the inspector is session state. Make these choices explicit in the new app instead of treating all UI state as project edits.

## 3. The title/menu row

### Layout and window behavior

Use the Panel color across the compact title strip. Place the small transparent application mark, a separator, adjacent menu headings, flexible drag space, a saved/dirty indicator, a separator and window controls in that order.

Show the application name and compiled working version beside the mark at the top
of every main window. Use the same identity in the native Windows title so a
Not Responding window still identifies its build. Derive it from the same embedded
version/history source as About; never hard-code a second version.

The mark is displayed at 18 by 18 and has the application's name as a tooltip. The mark and unused title area are window drag targets. Menu headings, fields and window controls retain their own input.

- Primary press on a drag target starts normal window movement.
- Double-clicking a drag target toggles maximization; fullscreen first exits fullscreen.
- Minimize, Maximize/Restore and Close occupy the right edge without gaps.
- Glyphs are centered inside their actual 28-by-24 bodies. The Maximize symbol changes to Restore when appropriate.
- Close enters the normal exit flow, including invalid-draft checks and Save/Discard/Cancel.
- Preserve native edge/corner resizing while the window is neither maximized nor fullscreen.

For a custom window frame, implement resizing, drag hit regions and maximize state deliberately. Drawing a title row alone does not provide those behaviors.

### Menu interaction state machine

Manage the title headings as one group with one active menu identity.

| Input | Result |
| --- | --- |
| Hover when every title menu is closed | Animate heading feedback; keep menus closed |
| Click or keyboard-activate a heading | Open its menu |
| Move onto another heading while a title menu is open | Switch immediately, including across skipped headings and in either direction |
| Move into the open dropdown | Keep it reachable and open |
| Click the same open heading | Close it |
| Hover a command | Feedback only |
| Activate an enabled command | Execute once; close as that command requires |
| Escape or outside click | Dismiss; subsequent hover alone does not reopen |
| Disabled heading/row | No command or hover switching |

Collect all heading hit responses before drawing any dropdown. Then choose the active heading and render its menu. This avoids a frame where one heading is highlighted over another heading's commands. Release stale keyboard focus when switching.

An unrelated open popup must not make title headings switch on hover. Restore ordinary application padding and interaction height inside the dropdown so compact title-row settings do not shrink menu contents.

### Portable menu organization

| Menu | Suggested contents and responsibility |
| --- | --- |
| File | New, Open…, Open Recent, Save, Save as…, Project settings…, Preferences…, Exit |
| Edit | Undo with action label, Redo with action label, Duplicate selection, Delete selection |
| Tools | Domain tools, asset bank, style viewer, imports/exports, optional diagnostics window |
| Windows | Reset layout |
| Help | About, Change log; other help destinations only if the new app provides them |

Achimeld's Open Recent holds up to ten complete project paths, newest first, with an inert localized placeholder when empty. Recent-file opening uses the same pending-action and unsaved-change flow as Open.

Use ellipses for actions that need further user input, such as choosing a destination. A command that starts a defined operation immediately can omit them. Disable Undo/Redo when their respective actions are unavailable and expose their action labels in tooltips or menu text.

Reset layout restores default tool placements, recovers lost floating windows, clears movable color-editor positions and returns Properties to the right dock. It preserves document content, selections, drafts and canvas framing. About and Change log can share one read-only in-app modal with a bottom page/action bar. Embed version/history at build time if they must work offline.

### Shortcuts and command ownership

Use the same command route for menus, toolbar actions, context menus and shortcuts. Achimeld provides Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+Z, Ctrl+Y/Ctrl+Shift+Z, Ctrl+D and Delete for the corresponding file/history/selection actions.

For the new app, route input to the focused text/editor or modal first, then the focused tool, then application commands. A Delete key in a text field must not delete a document object; Duplicate/Delete in a focused layer stack operate on layers. Display shortcuts in tooltips/menu captions and disable unavailable commands consistently across entry points. Do not infer Alt-menu mnemonics, a command palette or additional shortcuts merely from the custom title-menu appearance; implement and verify any such features explicitly.

## 4. Navigation and every other button row

The main navigation row is the default pattern for **every group of adjacent buttons**: panel toolbars, inspector tabs, miniature banks, file actions and bottom action bars.

1. Reserve a fixed row height for the current font/display scale.
2. Fill the complete row with the containing panel's resting color.
3. Set outer vertical padding and inter-button spacing to zero.
4. Give each button the full content height, with square corners.
5. Make unselected resting bodies exactly match the row, including its alpha.
6. Keep selected tabs in the Accent style.
7. Put text/icon padding inside bodies. Center text fields within the row using a compact inset.
8. Keep bounds unchanged during hover, focus, selection and disabling.

Do not put a second colored backing rectangle behind an unselected tab. A button body must reach both row limits, without a floating capsule or a narrow empty band above or below it.

Calculate the height using physical-pixel rounding:

```text
scale = physical pixels per logical point
textHeight = ceil(buttonFontLineHeight * scale) / scale
contentHeight = max(textHeight, iconAllowance) + 2 * buttonPaddingY
barHeight = ceil(max(contentHeight, minimumInteractionHeight) * scale) / scale
```

The main row uses iconAllowance = 24. With the default padding this produces a 38-point content row when the icon allowance is the largest item. Account separately for its separator; do not subtract separator space from full-height button bodies. Preserve an established row's approved height when extending it.

View tabs combine transparent 20-point artwork with a 5-point label gap. Center the **combined group**. Hover only animates a tab; click changes mode. Cancel in-progress selection/node gestures when changing modes. Achimeld clears canvas selection on a main-tab switch; preserve that rule if selection is scoped to the active mode.

Keep global Undo/Redo consistently placed. Put view-specific actions beside the relevant view. Creation/deletion can live in a contextual canvas menu rather than filling the bar with every command. A right-aligned project name and primary action provide a stable destination for the main workflow. For staged operations, show the next meaningful action and its prerequisites explicitly.

When a row cannot fit, use a deliberate overflow or bounded scroll layout. Do not let changing labels or open menus silently enlarge the window. Fixed, unresizable surfaces retain complete localized descriptive labels.

## 5. Buttons and reversible color inversion

### Exactly three styles

| Style | Resting body | Resting ink | Typical use |
| --- | --- | --- | --- |
| Standard | Control | Primary text | Standalone actions |
| Accent | Blend(Control, Accent, 0.25) | Primary text | Selected tabs and emphasized actions |
| Quiet | Panel | Primary text | Menu headings, window controls, adjoining bar actions |

Inside a popup, **every row rests on the popup's exact background**, including the current value. Dropdown selection does not receive a persistent Accent block. Application tabs still do.

All buttons contain a filled body and text and/or artwork. Button bodies have no outline in any state. Keyboard focus uses the same filled negative state as hover. Structural dividers, canvas-selection markers and focus indicators on other widget types have separate roles; the no-outline rule applies to button bodies.

### Exact negative endpoints

For each button, let B be its own resting body and I its own resting ink:

```text
rest:        body = B, ink = I
hover/focus: body = I, ink = B
disabled:    body = B, ink = mix(B, I, 0.45)
```

Invert that button's colors, including an Accent button's selected resting fill. Do not substitute a generic hover gray, change font weight, resize the button or move its contents.

Keep independent animation state under each stable button ID:

```text
target = enabled && (hovered || keyboardFocused || pressed)
direction = target ? +1 : -1
bodyProgress = clamp(bodyProgress + direction * dt / 0.15, 0, 1)
inkProgress  = clamp(inkProgress  + direction * dt / 0.20, 0, 1)
ease(t) = t * t * (3 - 2 * t)
```

Changing target reverses immediately from the current progress. Leaving halfway through a fade must not complete the old cycle first. Disabled buttons reset interaction progress and stop animating. Request frames only while a transition or visible cue needs them.

**Two different durations do not prevent disappearing text.** Swapping two colors along the same RGB line causes a crossing. Achimeld bends the intermediate ink path perpendicular to that color line near the crossing, with strength fading to zero at the endpoints. Port the contrast-preserving behavior along with the timings. Test dark, light and customized palette pairs; this color-separation technique is not a general guarantee of accessible contrast for arbitrary user-selected colors.

Apply animated ink to the actual rendered glyphs and artwork tint. Cached text layouts may already contain a theme color: override that paint color explicitly. Keep the measured layout and resting font family/weight unchanged. Center an icon-only text glyph using its visible bounds, and center images with aspect ratio preserved inside the actual body.

### Optional next-action cue

Achimeld can place slowly moving 45-degree black/white stripes over a button with Screen blending at 20% opacity. The overlay stays under the text/icon and within the body's clipping/corners. It changes neither action semantics nor layout. Only enabled, visible buttons animate; a matching completed workflow or active operation removes the cue. Include enabled/disabled examples in the component preview page if this feature is adopted.

## 6. Dropdowns, nested menus and context menus

Use one popup infrastructure for ordinary choices, font/language searches, presets, references, style catalogs, nested title commands and right-click menus. In Achimeld these are shared Dropdown and button-menu helpers; direct framework ComboBox construction is guarded against in regressions.

### Appearance

- Square, borderless panel; zero outer padding and zero gaps between rows.
- Full-width rows with internal label/icon insets. Menu labels are left-aligned and vertically centered.
- Closed trigger uses its containing panel background. Popup rows use the popup background, including selected entries.
- Use the same inverse hover/focus animation and disabled ink as buttons.
- Popup Button, Body and Small fonts are each one logical point below their corresponding configured app font. Resolve from the global app font every time so nested menus do not shrink cumulatively. Ordinary collapsed choices use the smaller button font too.
- Keep complete short lists visible together. Achimeld's eight blend modes are one unscrolled list: Normal, Multiply, Screen, Overlay, Darken, Lighten, Add and Difference.
- Long catalogs may scroll and use search/virtualization. Their logical navigation order includes offscreen entries.

### Input ownership

Maintain separate concepts for the committed value, current navigation candidate and input owner.

| Input/state | Required behavior |
| --- | --- |
| Up / Down | Move through enabled entries in logical order, stopping at either end; scroll candidate into view |
| Enter / Space | Activate current candidate exactly once |
| Candidate changes | Feedback or temporary preview only; no document commit or undo entry |
| Pointer position actually changes | Return ownership to the mouse |
| Pointer stays still through keyboard navigation/repaints | Keep keyboard ownership; do not reselect the row under that pointer |
| Escape / outside click | Close without committing the candidate |
| Search/property input | Retain normal typing, clicking and selection behavior |
| Disabled choice | Cannot activate; ordinary disabled pickers close their list |

Display one active candidate, not independent mouse and keyboard highlights. Tab must still reach search fields and auxiliary actions. In a font search, Up/Down can transfer from the search field into results. A title menu that becomes disabled may retain its inert display; it cannot switch headings or execute.

Context menus anchor to the original completed click. Do not reopen or move them when the same event is seen in a repeated layout pass. Distinguish a short right click from a right-button drag before deciding to open a canvas menu.

### Safe live previews

Hovering or keyboard-selecting a blend candidate temporarily composes the selected layer against the current composition. Click/Enter commits through normal validation and undo. Preview every supported layer category, including folders and adjustments; a preview for only image layers is incomplete.

Keep temporary preview state outside the saved recipe. Clear it on dismissal, pointer departure, focus loss, selection or document revision changes, and disabled editing. Use the same cancellable preview worker as ordinary rendering. Do not render hidden catalog rows merely to support keyboard order.

## 7. The selection-driven inspector

The right-hand panel is **Properties**, the inspector for the currently selected object or nested item. It is not a second project browser.

### Selection contract

1. Selection is a set/list of stable object identities.
2. Resolve the active object against the current document every frame or state update.
3. With no selection, show a brief localized instruction instead of stale properties.
4. If a selected object no longer exists, remove the stale selection.
5. Choose tabs and property editors from the selected object's capabilities.
6. If an active tab is invalid for the new type, choose a valid one deterministically.
7. Preserve focused drafts and scroll identity where the same object/editor remains active.

Achimeld edits the **first selected object** in this inspector; multiple selection does not imply aggregate property editing. A new app can add explicit bulk editing, but it must show mixed values and define which objects a commit affects.

Examples from the source:

| Selection | Inspector tabs |
| --- | --- |
| Regular achievement | General, Localization, Icon |
| Stat | General, Localization |
| Local icon-style helper | General, Style |
| Reusable art image | Image |
| Layer or effect in the visual editor | That layer/effect's properties within the selected visual tab |

Use the new app's names, with the same capability-based approach. Inspector tabs follow the flush square bar contract and carry generated type/category icons. In a resizable inspector they can fall back to shorter captions, with complete localized names in tooltips and accessibility metadata.

### Docking and movement

Properties starts on the right at 288 wide. Achimeld's docked minimum is 220 where space permits; maximum width is 60% of the workspace. It can dock left, dock right or float inside the workspace.

- Drag only the full-width Properties header to move it. Editing fields does not move the inspector.
- While dragging near a side, show an Accent destination preview. The source uses a 40-point edge band, capped at 20% of workspace width.
- Release on the preview to dock, elsewhere to float. The canvas takes the freed space.
- Click/keyboard-activate the header for Dock left, Dock right and Float commands.
- Escape, focus loss, disabled workspace and leaving editing cancel an unfinished drag and restore its starting placement.
- Fit floating windows inside the remaining workspace when the main window shrinks.
- Preserve global content identity across placements so drafts, focus, tabs, scroll and asset-drop routing survive.
- A floating inspector blocks pointer and wheel input from reaching the canvas underneath.
- Reset layout returns Properties to the right without changing the document.

Dock placement survives view/project switches during the session in the current source; do not claim it is a saved project setting.

### Nested layer properties

The default application preference places layer/effect properties in the right inspector, while a compact floating Layers window holds the stack. Turning the option off restores the wider combined layer editor. Both arrangements use the same editors, validation and history.

Every ordinary layer property editor begins with exactly three compact pinned rows:

```text
Opacity     [slider] [value]
Blend mode  [mode choice]
[Type icon] Name [single-line editable name........................]
──────────────────────────────────────────────────────────────────
Scrollable type-specific properties
```

Use only the type icon, with its full localized type on hover. Do not add separate Layer type, Layer style or Layer properties headings. Let the name field use the remaining width without wrapping the identity row. Keep this header above the type-specific scroll area, including for folders, adjustments and General Image layers. Effects use their relevant effect properties rather than pretending to be document objects.

Reserve a constant scrollbar gutter even while its bar is hidden/thin. Hovering an expanding scrollbar must not cover or shift controls.

## 8. Property rows and field labels

Use one FieldRow implementation for text, numbers, checkboxes, dropdowns, sliders, swatches and icon/name rows. It owns localization, label widths, tooltips, wrapping, accessibility labels and the optional divider.

### Resizable editors

```text
labelWidth = max(savedMinimum, rowWidth * savedPercent / 100)
labelWidth = min(labelWidth, space remaining after gap and minimum control width)
```

Defaults are 90 points and 15%. Preferences permit minimum width 40–240 and percentage 5–50. The minimum wins until physical space requires shrinking. At narrow widths, the label column may truncate to preserve a usable field without overlapping or introducing horizontal scrolling. It never moves above the field.

Optional multiword abbreviation starts from the full localized label: Bottom height → B. Height. Preserve meaningful acronyms and axis labels. Single-word labels stay intact. Always expose the complete localized label on hover and in accessibility metadata, including for disabled fields.

The gap between label and control can contain an invisible-at-rest global divider. Hover reveals the divider and horizontal-resize cursor. Drag updates the shared ratio live across editors; persist once on release. Achimeld lowers the drag's safety minimum to 40 so a previously saved 90-point minimum cannot prevent narrowing. Inside Preferences, this edits its draft and still respects Save/Cancel.

### Fixed forms and popups

A fixed-width surface always displays complete localized labels, regardless of abbreviation preferences. Measure the full text, keep it to the left, wrap it and grow the row/body vertically. Adapt width or use bounded vertical scrolling when necessary. A tooltip, ellipsis, smaller font or initials do not substitute for the complete fixed-form label.

Being movable, vertically scrollable, auto-sized, inside a resizable parent or equipped with a label divider does not make a fixed-width form resizable. Mark fixed ancestry explicitly, including prompts, menus, pickers and fixed sections inside otherwise resizable tools.

Only place related short fields side by side when every complete label-and-field pair fits. If a slider track and number must stack within their control cell, the row's label still stays on the left.

Checkbox labels occupy the left label column; associate them with the checkbox and allow clicking the label to toggle it. Numeric fields support typed values and horizontal dragging with domain-specific bounds, precision and units. Show validation next to the relevant draft and offer a clear revert path.

## 9. Sliders and range sliders

### Single-value slider

Use a compact horizontal rail, a filled circular knob in Primary text and an editable numeric value in the same property row. The rail and recessed surface derive from Control/Window. Keyboard focus is shown on the rail using Accent; it does not require recoloring the knob.

The shared source track is 12.5 high with a 4-point rail. A compact opacity tile combines a small opacity symbol/caption with this track in a 34-point recessed control. Prefer the shared value-slider row for ordinary properties instead of recreating the track per editor.

The numeric control supports typing and horizontal dragging, shows units such as %, degrees or a multiplier, and uses the same bounds as the track. Reserve number width from the configured format and maximum, rather than the current digits, to avoid jitter. Round/clamp committed values consistently with the property's precision. Keep display units separate from storage units: opacity can display 0–100% while storing 0–1.

When narrow, put track and numeric editor one above the other inside the control column. Keep the left label in place. In the source helper, the ordinary track budget is 24–360 wide when a side-by-side value fits. Disabled sliders ignore interactions and the single-value knob uses subdued ink. Keep keyboard behavior and meaningful slider metadata when custom painting around a framework slider.

### Two-ended range slider

```text
rail ───────●════════════════●─────────────   minimum–maximum units
            low             high
```

This is one interval with three separately selectable parts: low knob, high knob and the selected bar.

| Gesture | Result |
| --- | --- |
| Drag low knob | Change minimum; it cannot cross maximum |
| Drag high knob | Change maximum; it cannot cross minimum |
| Drag the bar between knobs | Translate both ends by the same delta; preserve interval width |
| Reach a domain limit while moving the bar | Clamp the shared delta so interval width remains intact |
| Grab the edge of a knob | Retain the pointer-to-knob offset; no jump on press |
| Click empty rail | Move the nearer endpoint to the click within valid bounds |
| Knobs coincide | Choose a side predictably so either direction can separate them |
| Focused Left/Right | Adjust the selected endpoint or whole bar |
| Disabled control | Ignore knob, bar, rail and keyboard edits |

The source draws 5-point-radius knobs with 7-point hit radii. The base rail is 3 thick; the selected segment is 4, increasing to 6 when its bar is hovered/dragged. Use Grab over the segment, Grabbing during translation and horizontal-resize over endpoints. Focus is an Accent rail indicator.

Capture the starting pointer position and both starting values once per gesture. Derive each frame's delta from that snapshot rather than repeatedly adding displacement. For whole-interval motion:

```text
delta = clamp(pointerDeltaInValueUnits, domainMin - startLow, domainMax - startHigh)
nextLow  = startLow  + delta
nextHigh = startHigh + delta
```

The source's keyboard increment is 1% of domain width, or 10% with Shift. Its regression cases cover endpoint and whole-bar keyboard adjustment. Retain fine/coarse adjustment when porting and check the chosen framework's modifier/event handling.

The current range helper shows a min–max readout; it does not provide two independently typed endpoint boxes. If precise endpoint entry is needed in the new app, add it through the same bounds/validation contract. On narrow layouts the readout can move below the track inside the control column.

Keep a complete drag as one undoable edit group. Test endpoint crossing, nonzero lower domain bounds, coincident knobs, repeated drags, bar limits and narrow layouts.

## 10. Colors, gradients and visual editors

### Color editing

Represent colors in normal properties as swatches over a transparency checker. Clicking opens a movable in-app color window. Keep color encodings out of ordinary labels/tooltips.

The color editor combines a square saturation/brightness area, full-width hue and alpha strips, numeric Red/Green/Blue/Alpha controls, a preview and a Hex field. Keep numeric widths independent of their current digits. Retain hue through neutral gray/black numeric edits so returning to saturation is predictable. Support pointer dragging and arrow-key nudging.

Hex is an explicit exception to visual-only color presentation. Accept RGB or alpha-inclusive input, with an optional leading #. **Achimeld uses #RRGGBB and #AARRGGBB, with alpha first in the eight-digit form.** Do not silently substitute the common web RRGGBBAA convention when porting. Incomplete/invalid text retains the last valid color and remains editable.

The movable color window is fixed-width, so it always uses complete field labels. Its source visual geometry is compacted to 70% of an earlier design; reproduce the current compact geometry, not the historical scaling operation. In mask contexts, use Gray/Alpha plus a grayscale preview and a clear White reveals / Black hides explanation.

Black/white checker or marker artwork and the content being edited have their own visual meaning. Theme changes should not recolor image content or convert every visual datum into the application Accent.

### Gradient and curve patterns

- Show gradients as rendered strips/tiles with visible handles, not a list of stored color strings.
- Double-click empty gradient space to create a stop using the existing sampled gradient color. Select stops to edit their color/position.
- A 2D gradient map uses movable color points, selection-box dragging, group movement and controls below the map. In the source, Shift adds and Ctrl removes map-point selection.
- Use explicit Delete/Reverse/preset actions in a shared bar. Keep point selection and numeric edits distinct from dragging the complete editor.
- Named gradient/effect explorers combine search, visual tiles, create/duplicate/save/delete and import/export. Library edits have a defined lifetime separate from applying a preset to a document.

Achimeld's Gradient Explorer uses plain-click selection, Shift-add and Alt-subtract for **library tiles**, which differs from its map-point modifiers. Do not assume one modifier scheme across all collections. The new app should document or deliberately unify these gestures. Curve and other graphical editors should reuse the same field, focus, clipping and draft rules even when their canvas geometry differs.

## 11. Style banners and preset libraries

Here **style** means a reusable visual recipe applied to domain content. It is separate from the application's theme/palette. Keep those two concepts separate in UI names and data.

A style banner gives the user a recognizable visual choice without requiring a text-only list or displaying a full square image everywhere.

### Three levels of preview

| Surface | Image presentation | Information/actions |
| --- | --- | --- |
| Collapsed current-style banner | Thin central 16% horizontal crop, vertical UV 0.42–0.58, with a strong translucent palette-fill overlay | Current style name and disclosure; secondary scope/link caption |
| Open project-style list | Central 48% crop, UV 0.26–0.74 | Separate name band; adjacent Rename/Delete actions |
| Style Viewer library | Same 48% crop in a larger banner; image height about 0.48 × width | Name band, selected state, context actions; full result in main preview |

Crops are for recognition; the main preview and naming prompt show the full image with its aspect ratio. Keep labels on a readable band rather than relying on arbitrary image contrast. Paint custom labels with the shared animated ink, including the actual glyph override. Fit the caption inside its allocated slot so a long name does not push import/save actions away or resize the tool window.

The collapsed source banner uses a palette-fill overlay with alpha 220/255. Treat that as a reference value, with readable text as the acceptance criterion. Reserve adjacent square, full-height direct actions for **Import into project** and **Save current as style**. Import opens the global template list; Save opens the standard naming prompt with a full preview and Save/Cancel. Do not hide all library operations inside a text-only choice list.

Popup style rows follow popup resting-fill rules; the main Style Viewer can use Accent selection. Show complete names on hover/accessibility metadata when a resizable banner truncates them. Keyboard navigation must work through virtualized style rows and reach auxiliary actions.

### Identity and application semantics

Store style identity separately from its name, recipe and rendered thumbnail. An explicit link takes priority over recipe-equality matching; two styles can have identical recipes and different identities. State whether applying a style copies its recipe or links the object to future style edits. Achimeld uses copies for ordinary application and explicit links for its style-helper nodes.

When copying layered recipes, create fresh internal layer/effect identities and remap references. Otherwise changing one instance can select or alter another accidentally. Rename/delete belong to validated undoable operations. Deleting a linked style should have an explicit policy; the source clears the link while retaining the object's editable recipe.

### Style Viewer as a workspace mode

Provide project styles, app/global styles and portable collections, a common input area, a large full-result preview and an asset bank. Read-only built-in collections should be visibly distinct from editable collections. Browsing a library should not silently apply a style to document objects.

Render each banner from its actual recipe with the same current preview inputs. Do not compare stale stored thumbnails rendered with different source images. Achimeld uses three stable image slots (ID 0, ID 1, ID 2) and a decimal unlock-value input defaulting to 10. A different app can use its own small, representative input set. Clearing a slot leaves the other slot IDs unchanged.

The source adapts from side columns to collapsible/stacked content as width falls: a library side panel starts at 760, the additional bank side panel at 1040, and the input side panel at 460 within the center. These are implementation reference thresholds; choose breakpoints from the new app's content and translations. Keep file actions and preview reachable at narrow sizes.

Collection load/save uses native pickers and host-owned validated storage. Validate a candidate before replacing state; cancellation/failure preserves the current libraries. Portable recipes must carry their needed assets. In Achimeld a collection includes a paired localization CSV and sibling art-data-bank folder; copying only the main file is insufficient. The new app may choose a different portable container.

## 12. Preferences and the palette

### Semantic colors

Use named tokens centrally rather than scattering unrelated colors through components. Store RGBA values in preferences and derive shared visuals once per UI update. Preserve token identities even if their displayed names are localized.

These are the actual source defaults; alpha is 255 for every entry:

| Token | RGB | Responsibility |
| --- | --- | --- |
| Window | 23, 25, 29 | Deep surfaces, editor backgrounds |
| Panel | 32, 35, 41 | Bars, inspectors, Quiet resting buttons |
| Control | 42, 46, 53 | Ordinary control bodies |
| Border | 65, 71, 80 | Structural edges/dividers where required |
| Primary text | 241, 243, 245 | Main text, tintable glyphs, slider knobs |
| Muted text | 167, 173, 183 | Secondary information |
| Accent | 124, 156, 255 | Selection derivation, focus/cursor cues |
| Success | 100, 199, 141 | Positive statuses |
| Warning | 244, 190, 91 | Attention/status meaning |
| Error | 255, 114, 114 | Invalid drafts and failures |
| Background top | 21, 21, 21 | Canvas gradient top |
| Background bottom | 48, 48, 48 | Canvas gradient bottom |
| Grid | 56, 56, 56 | Canvas grid |

Derive selected fill as mix(Control, Accent, 0.25), button disabled ink from the button's own resting pair, and text cursor/focus rail from Accent. Set text, code/edit backgrounds, panel fills, warnings, errors and hyperlinks through the same theme application layer. Custom-painted rows and glyphs must consult that layer too.

Achimeld infers a dark/light framework mode from the Window RGB sum being below 384. This heuristic supports framework behavior; semantic token values still determine actual colors. A missing lookup falls back to Control, while preference validation requires every default token to exist. A new app should validate its complete palette schema and not rely on a silent fallback to conceal misspellings.

**Palette scope:** application chrome is themeable. Document artwork, swatch values, generated content and some visual reference marks are data. Standard prompt header/body use an intentional opaque black exception described below. Do not promise that every painted pixel is a user-controlled theme token.

### Preferences window

Use a resizable in-app window with square edges, a custom header, left section navigation, scrollable page content and a fixed full-width Cancel/Save bottom row. Long settings pages scroll above the actions. Generated white tintable section icons and left-aligned icon/label groups make the categories recognizable. Unselected category rows merge into Panel; selected categories use Accent.

Achimeld's sections are General, Colors, App style, Naming and Browser. For another app, keep General/Colors/App style and replace domain-specific sections. General includes interface language and layout options; Colors exposes each palette token as a swatch; App style is both settings and an interactive component gallery.

The gallery should show Standard, Accent and Quiet; selected/unselected tabs; enabled/disabled buttons; icon-only actions; a menu; adjoining bar buttons; field-label behavior; and optional attention cues. Users must be able to hover and keyboard-focus these samples. A static picture cannot demonstrate the interaction contract.

Appearance controls in the source include:

| Setting | Default | Valid range |
| --- | --- | --- |
| General control radius | 6 in source; use 0 for the square new-app profile | 0–24 |
| Heading size | 18 | 10–48 |
| Normal/button size | 13 | 9–32 |
| Small size | 11 | 8–24 |
| Inspector vertical spacing | 7 | 0–32 |
| Label minimum | 90 | 40–240 |
| Label percentage | 15% | 5–50% |
| Abbreviate resizable field labels | On | Boolean |
| Layer properties in sidebar | On | Boolean |
| Effect-row mini values | On | Boolean |

### Draft, preview, Save and Cancel

1. Open Preferences by cloning committed settings into a draft and retaining its base snapshot.
2. Apply the draft palette, language, fonts and spacing to the visible app for live preview.
3. Validate the complete candidate before saving.
4. Refuse a stale save if committed preferences changed since the window opened; ask the user to reopen/resolve the conflict.
5. Save through the application's command/persistence path.
6. Cancel, close and Escape discard the draft and restore committed appearance.
7. Restore default colors changes the draft until saved.

Supply defaults for missing new fields in older preference files. Keep application preferences separate from project settings such as project name, identifiers or atlas geometry. For the new app, define whether preference commits participate in application history; Achimeld's editor history captures preferences as well as document deltas.

## 13. Localization and typography

### Two catalogs, two jobs

| Catalog | Purpose | Ownership |
| --- | --- | --- |
| Interface strings | Menus, fields, prompts, tooltips, help captions | Embedded application resource |
| Project content strings | Names/descriptions/text authored for exported content | Project data, editable through validated commands |

Do not translate user-chosen object/style names as if they were interface keys. Changing interface language must not rewrite document content or schema identifiers.

Achimeld's embedded interface CSV has nine columns:

```csv
ID,Context,English,Polish,ChineseSimplified,ChineseTraditional,Japanese,Korean,French
```

It is parsed once into a cached catalog. Lookup first tries the stable ID, then a normalized English-source key. English-source normalization trims surrounding space, trailing periods/ellipsis and lowercases. A missing translation falls back to the English cell; an unknown key falls back to its supplied source string. Language preference selects one of the seven declared languages; unknown language state falls back to English.

For a clean-slate implementation, prefer stable namespaced IDs and use English lookup only as a migration convenience. Repeated English words in different contexts can collide in source-based lookup. Store the context for translators, not as a substitute for an explicit message identity.

Route all visible UI text through the catalog: headings, menu commands, disabled placeholders, field labels, tooltips, accessible names, prompt actions, status messages and validation errors. Shared property rows already localize their labels in the source; a literal framework label elsewhere does not become translated automatically. Audit custom painting and alternate/error paths too.

### Layout after translation

- Measure translated text before sizing controls, menus, category columns and prompts.
- Apply abbreviations only after localization, and only in resizable surfaces.
- Fixed prompts/forms wrap complete labels on the left; bars preserve height and use deliberate overflow.
- Reduce popup fonts from the configured global font once, not once per nesting level.
- Give icon-only controls complete localized tooltips and accessible names.
- Keep internal IDs, serialized enum names and palette keys stable across language changes.

The current language set does not establish right-to-left, grammatical pluralization or locale-specific numeric entry support. If the new app needs those, add explicit message-formatting, direction and parsing policies rather than assuming CSV lookup supplies them.

### Project-text translation workflow

Use stable localization IDs referenced by objects, rather than array positions or English text. Achimeld saves its content catalog as a CSV beside the project. Its Localization inspector edits each language through commands, retains invalid text drafts and groups typing for undo.

For translator exchange, export a scoped catalog with ID, Context and English source pinned. Validate all imported rows before applying one transaction: reject unknown IDs, altered source/context and mismatched placeholders. In the source import, empty translated cells mean **no change**; clearing a translation is an explicit editor action. Host operations check that the document did not change while a background import was being read. Adopt these semantics deliberately and show them to translators.

### Fonts and rendering

On Windows, the host supplies Segoe UI regular/bold bytes. The UI embeds Noto Sans CJK regular/bold fallbacks, including a monospace fallback. Language selects regional faces for Japanese, Korean, Simplified Chinese and Traditional Chinese, so mixed text has coverage even when the interface language is English.

Keep actual bold-family text where needed; hovering must not change weight. Check regular, bold and monospace with CJK, accented Latin and mixed text at ordinary and fractional display scales.

Achimeld also has paired local font-rasterizer/Glow patches for RGB subpixel coverage. These are a renderer-specific fidelity detail, not a portable requirement to copy two dependency folders into another stack. If reusing this Rust backend, keep the patches paired and their licenses/provenance intact. Otherwise use the target platform's appropriate text renderer and compare native output. The source path targets horizontal RGB stripe displays and is not equivalent to Windows ClearType or automatic display-order detection.

## 14. Prompts, file pickers and pending actions

### Standard in-app prompt

Use a centered modal over a dimmed backdrop with three full-width sections:

```text
┌──────────────────────────────────────────────┐
│ [icon]             Title                    │ opaque black header
├──────────────────────────────────────────────┤
│                                              │
│            Centered wrapped message          │ opaque black body
│              Relevant context                │
│                                              │
├──────────────────────────────────────────────┤
│ [icon] Action one │ [icon] Action two │ Cancel│ gray flush action row
└──────────────────────────────────────────────┘
```

The icon is on the left, but the heading is centered independently across the complete header. There is no extra framework title bar or close button around this composition. Use square corners, no outer content padding, a thin palette edge and one-physical-pixel horizontal dividers. Black header/body is the deliberate exception to theme-derived surface fills; gray actions, text and interaction colors still derive from the palette.

Reference sizing:

- Start at width 468; expand to fit natural combined action widths; leave 16-point viewport margins.
- Header icon 24 square, inset 24 from the left. Header height is max(title height, 24) + 24.
- Body padding 24 horizontal and 20 vertical, with every wrapped message line centered.
- Action artwork 20 square with an 8-point icon/label gap.
- Actions use the shared bar-height calculation, square bodies and no gaps.
- Divide available width in proportion to each action's natural content width. Round shared boundaries to physical pixels; the last button receives the remainder.
- Long content scrolls within a bound while actions remain reachable.

Use complete fixed-form field labels. Preserve action order and semantics. Escape/dismissal cancels; it must not select a destructive or committing action silently.

### Unsaved work and errors

The reference is Unsaved changes: Save this project before continuing? followed by project name in bold, full saved path in smaller muted text, or Not saved yet. Actions are Save, Discard changes, Cancel in that order. Center/wrap project context and bound unusually long paths.

Capture the pending action before asking: which project to open, whether to exit, or which collection is being replaced. Continue it only after the required successful save or explicit discard. A cancelled picker, failed save or rejected validation must not execute the continuation.

Errors use the same prompt structure, a specific message and meaningful next actions. A prerequisite error should not quietly perform an unrelated action; for example, the current saved-project-required prompt tells the user to save and retry instead of automatically opening Save during validation.

### File and folder selection

Use standard Windows file/folder dialogs for picking paths. Application confirmation, warning, error and acknowledgement windows use the in-app prompt system. Keep native picker startup and I/O in the host and avoid blocking the UI loop while the user chooses. The result is cancellation, a validated selection or a recoverable error. A picker failing is distinct from the user cancelling it.

## 15. Canvas, selection and camera behavior

Use a clipped central canvas with the Background top/bottom gradient and Grid palette tokens. Size artwork by physical display demand, preserve image aspect ratios and separate world coordinates from screen coordinates.

### Pointer-anchored wheel zoom

Let O be the canvas rectangle's screen origin, P the pan in canvas-relative screen units, Z the old zoom and M the screen pointer:

```text
anchor = M - O
worldUnderPointer = (anchor - P) / Z
newZoom = clamp(requestedZoom, minZoom, maxZoom)
newPan = anchor - worldUnderPointer * newZoom
```

Apply this on every step of smoothed wheel input using the actual clamped zoom. It must hold after panning, over cards/artwork and at zoom limits. The source scales wheel input exponentially, exp(scroll × 0.0015).

The +/− buttons anchor at the canvas center; the source steps by ×1.2 or ÷1.2. Reset and Fit all retain their explicit framing rules. They are different anchors and need separate regression cases.

Only an enabled, visible, uncovered canvas receives wheel zoom. Test the clipped interaction rectangle and topmost layer; a floating tool, popup or inspector must intercept input above it. Card hit areas on the same canvas must not accidentally block canvas zoom.

### Selection, movement and contextual creation

- Click to select; provide explicit additive/subtractive and rectangle-selection semantics appropriate to the surface.
- Use stable identities for selected objects and separate selection gestures from object drags.
- A short right click opens a context menu at its original position. Right-button movement pans with slight easing and no overshoot; it must not also open a menu.
- Right-clicking an unselected card selects it. Right-clicking one member of a selected group preserves that group for actions such as Delete selection.
- Create near the click/drop location in the nearest unoccupied grid position, considering filtered/hidden objects for collision avoidance.
- Keep a pan and its settling in one camera undo group if camera edits are undoable. New pointer input, wheel zoom, Escape, focus loss, disabled state, view change or an external camera update cancels remaining glide.
- Keyboard object navigation follows the canvas's spatial model. Delete/Duplicate route to the focused object/layer context and respect text editing and modals.

At low zoom, source achievement labels fade out around a 0.5 threshold to prioritize artwork, while stat cards retain labels. This is an example of reducing detail with scale; choose meaningful detail levels for the new domain. Use preview outlines/connections to make the selected artwork and its Layers tool relationship clear without shifting layout.

## 16. Layers, effects and asset banks

### Layer and effect rows

Provide a visible type symbol or thumbnail, visibility action, name, compact opacity/blend controls and meaningful selection. Use a consistent fixed-height action bar for add, duplicate, delete, mask and other domain actions. Keep tree connectors and indentation legible.

Treat thumbnail selection, row selection, numeric editing, swatch editing, expansion and reordering as distinct hit targets. Grabbing a numeric mini editor must not reorder the effect or its owning layer. A mask/sublayer thumbnail needs its own visible selected outline so the user can identify the active editing target.

Expanded FX rows always retain their generated type icon. When the mini-value preference is on, admit complete icon/value pairs only when they fit. The effect name yields before controls, and parameter labels appear only when their complete group fits. At narrow widths keep the type accessible through a tooltip. Clicking a layer row/thumbnail returns to its properties; unnecessary Back to layer navigation is avoided.

The source also gives layer rows a subtle content-derived backdrop: use an authored color for solids/text/gradients or an alpha-weighted dominant image color, fading into the neutral Control/Panel base. Cache image-color analysis by preview fingerprint; ignore transparent RGB. Tint/selection transitions take about 0.4 seconds. This treatment belongs to content rows, while adjoining action buttons still obey their exact panel-resting-fill rule. It is a useful way to distinguish layers without adding more permanent label colors.

### Asset bank

Imported artwork becomes an owned asset in a central bank. Treat Images, Atlases and other useful categories as bank tabs with the shared flush bar, plus Import/Refresh/reload/resize actions and a detail inspector. Show thumbnails, dimensions, identity/status and repair actions where useful. A source picker can reuse the bank's image/atlas catalogs and highlight the selected atlas tile.

Retain an owned copy for file drops as well as explicit imports. Saved projects use content-addressed files and project-relative references in a sibling asset directory. Reload/resize preserves logical asset identity and retains immutable previous content for undo. A source file disappearing must not make the project depend on an arbitrary original absolute path.

Unavailable artwork stays visible and repairable in the bank. A missing image layer must not prevent other icon layers from rendering. Route imports by the hit destination: empty canvas can create objects, an object's artwork can offer General images vs layers, and dropping into the bank just imports assets. The host validates/decodes/retains files; the UI sends a closed target intent and subsequently works with asset identities.

### Floating tools and recovery

The Layers window can move from passive captions, header gaps, unused body and margins. Enabled buttons, fields, rows, dividers, scrollbars and resize handles keep their gestures. Covered/disabled areas cannot initiate a drag. Ensure a partially offscreen window can be recovered from its visible passive area, and provide Reset layout/Show layers panel recovery commands.

## 17. Generated icons and embedded artwork

Use the **imagegen skill** to create new option-tab, action and prompt artwork that follows this raster icon family. Existing code-drawn primitives such as window-control lines can remain shared primitives. This document records the workflow; no new icons were needed or generated for the documentation task.

### Art direction

Use a crisp, simple off-white monochrome glyph on actual transparency, with consistent medium visual weight, clean geometry, centered visible bounds and a strong silhouette at 20 by 20. Avoid a button background, panel, baked label, decorative frame, shadow or watermark. A 64-by-64 final embedded asset provides scaling headroom; visual consistency matters more than identical source canvas dimensions.

Example reusable prompt:

```text
Use case: logo-brand
Asset type: small desktop toolbar action icon
Primary request: One <meaningful subject for this action> symbol.
Style/medium: crisp flat off-white monochrome raster glyph; medium visual weight;
match the supplied established toolbar icon references.
Composition/framing: one centered symbol; balanced visible bounds; clearly readable
at 20 by 20 logical points.
Constraints: genuinely transparent background; no panel, text, decorative outer
frame, drop shadow or watermark; no extra objects.
```

### Production workflow

1. Read the installed imagegen SKILL.md and inspect representative existing icons.
2. Use the built-in image-generation tool by default. Request transparent output and preserve generated alpha. Label images as references or edit targets explicitly.
3. Generate one asset/variant per built-in call where separate assets are needed. Inspect at intended display size, then refine one specific defect at a time.
4. Preserve the original generated source unchanged, plus the exact generation/refinement prompts and preparation notes in a small manifest.
5. Prepare a centered 64-by-64 tintable copy with white RGB and the selected glyph's alpha. Preserve aspect ratio and antialiased coverage; do not bake the current theme color into the glyph.
6. Save source and final files in the project. In this Rust app, source/final PNGs support review and raw row-major RGBA8 is embedded with include_bytes!, avoiding runtime asset paths or a build-time external application checkout.
7. Load the final texture once/reuse it. Use linear filtering with linear mipmaps for this generated icon family and artwork previews.
8. Tint with the shared animated button ink in enabled, hovered, focused, selected and disabled states. Verify contrast and centering at small/large fonts and fractional scales.
9. Give icon-only actions a complete localized tooltip and accessibility name.

The default built-in skill flow does not require a user API key. If that tool is unavailable, do not silently switch to an API/CLI provider; follow the installed skill's explicit fallback authorization rules. Copy project-bound outputs into the project rather than leaving runtime references to the generator's default output folder.

Keep a provenance manifest per family: purpose, source/final filenames, exact prompts, source of reused artwork, alpha/normalization preparation and intended display size. The source toolbar README distinguishes generated artwork from older reused assets. Do not retroactively claim every legacy glyph was generated.

## 18. Responsive previews and bounded work

The visible result is part of the interaction contract. A property edit should show its first useful image as soon as possible; final-quality work follows asynchronously.

```text
edit → validated state / temporary candidate → prioritize visible preview
     → quick result → display immediately → detailed refinement → replace texture
                         ↑ keep previous successful image while pending
```

- Keep decode/composition work off the UI thread using bounded workers.
- Give visible quick previews priority over detail work. Do not hold completed quick images behind a full batch of expensive renders.
- Tag requests/results with document generation and identity. Cancel obsolete work and refuse stale results.
- Share immutable decoded images and font data across jobs. Lock asset loading briefly; composition can proceed independently.
- Bound pending jobs, completed-result buffering, CPU caches and GPU textures, including mip levels.
- Reuse a texture handle as quality changes; avoid creating a new handle each frame.
- Preserve the last successful image during refinement and meaningful failures; show missing/error status without making all artwork disappear.
- Request rendering only for visible banners/tiles/cards, with deliberate priority for a selected large preview.
- Measure **time to first visible result** separately from **time to final quality**.

Source reference budgets: the main preview worker uses 1–4 threads, normally available CPUs minus one within that range; at most 512 pending jobs; a 128 MiB preview cache; completed-result buffering bounded by worker count. The shared UI artwork cache is bounded by 512 handles or 128 MiB, accounting for mipmaps and evicting least-recently-used entries. Other caches have their own lifetimes; do not interpret a single cache budget as a total-process memory guarantee.

Choose budgets appropriate to the new app, but keep their existence explicit. A large-image stress case should remain interactive, and continuously changing a slider should not build an unbounded queue.

## 19. State, commands, undo and ownership

Visual consistency works best with a consistent state flow:

```text
UI reads document/preferences/selection
  → widget produces a local draft or closed intent
  → application validates a command
  → one transaction updates state/history
  → host handles required I/O/background work
  → fresh state and preview results reach the UI
```

The source presentation crate forbids unsafe code and receives an EditorPort exposing read-only state, validated commands, preview access and a finite set of user requests. The host owns file dialogs, image/file I/O, browser/process integration and persistence. These are source-architecture boundaries inside a shared process, not an OS sandbox for UI code.

Keep the new app's interface explicit: OpenDocument, SaveDocument, ImportAssets(target), ApplyPreset(id), BeginOperation and CancelOperation are reviewable intents. Avoid handing arbitrary executable paths, worker operations or raw browser-launch recipes to presentation controls.

### Drafts and history

- Editing a field may create incomplete/invalid text. Retain the draft and error without overwriting the last valid document value.
- Keep the original base snapshot while a draft is invalid. If live data changed in the meantime, require a revert/resolution before applying that stale draft.
- Mark invalid fields clearly and provide Revert invalid edit/text.
- Group typing and continuous slider/drag updates by the focused control/gesture. Finish on release/focus/context changes. One gesture should not produce hundreds of undo steps.
- Discrete commands such as applying a style, reordering a layer, renaming or importing a group get explicit history labels.
- Immediate-mode/repeated layout passes may observe an activation more than once. Keep side-effecting toggles and context-menu opening idempotent per input event/frame; stable IDs and an activation guard prevent a toggle from changing twice.
- Validate whole batches before mutation; failure preserves existing content.
- Save/open/export/destructive continuations respect invalid drafts. Undo may first revert invalid drafts; Redo is unavailable while drafts remain.
- Derive dirty status from the real save/history state, not whether a window happened to open.
- Browsing, menu candidates, docking and temporary blend previews do not create document history. Library edits and preview-input edits have explicit ownership; some preview inputs are real saved document data.

### Review before applying external changes

The synchronization and remote-snapshot views provide a transferable pattern for an app that connects to another system:

1. Keep the last loaded remote snapshot visibly read-only and timestamped. Do not substitute unsaved local data when presenting it as remote truth.
2. Guide the primary action through destination setup, validation/read, review and application. Show the destination identity alongside the result.
3. Compare local/remote rows with explicit states such as local-only, remote-only, changed, conflict and unavailable artwork. Let users expand field differences and choose only supported per-row actions.
4. Filter/page large reviews; request expensive artwork only for visible rows. Show selected-change counts in the final action label.
5. Require explicit destination confirmation for this workflow. Bind it to the current review identity, destination and document revision; a new review or changed destination invalidates it.
6. Enable Apply only when prerequisites and at least one selected change are satisfied. Provide cancellation and meaningful in-progress status.
7. Treat missing decorative banners separately from a failed data read. An unavailable image does not automatically invalidate the data or confirm its destination.

These are optional integration UI patterns, not a requirement to add remote access to a local-only app. Achimeld's particular validation also reconciles remote records into its project and has its own Steam draft/publish semantics; define such effects explicitly for the new app. UI confirmation is only one part of the operation's contract and does not replace host-side validation.

### Status and diagnostics

Every runnable build must create real-time runtime logs in the current user's
AppData from process entry, before supervision and UI initialization. This includes
development, quick and installed builds. For Achimeld use
`%LOCALAPPDATA%\Achimeld\RustDiagnostics`; compiler/build/test evidence remains
separate. Flush each record, retain bounded previous sessions, and keep writing
independent of whether the Terminal is open. Include compiled version, process and
session identity, safe operation breadcrumbs and responsiveness status. Expose the
actual file location and logging failures in Preferences, retry failed writes and
retain bounded in-memory evidence.

Resolve the runtime location through Windows' current-user Local AppData known
folder, independently of a launcher's redirected environment. Preserve the last
supervised runtime log separately before rotation so repeated startup failures
cannot replace the only useful session evidence.

Provide local crash dumps and safe metadata for Rust panics, unhandled native
exceptions and prolonged active UI hangs. Use a dedicated capture thread or process
and minimal filtered dumps with bounded retention; never dump browser/worker
processes or full memory. Treat dumps as sensitive local artifacts, not sanitized
text, and never upload them automatically. Forced termination and power loss can
prevent capture; preserve flushed logs and report missing evidence honestly.
Verify the exact built executable creates live files and useful artifacts after
controlled failures. A passing logger unit test alone is not runtime qualification.

Keep a compact status bar with operation status, relevant counts and camera controls. Long-running operations expose cancellation where supported and disable conflicting actions, while ordinary input/painting continues.

An optional Tools > Terminal window is a **read-only session log**, with selection, Ctrl+A/Ctrl+C, Copy all and Pause/Resume display. Collection continues while display is paused; reopening retains session output. Achimeld bounds it to 1 MiB/4,000 entries and uses UTC timestamps. It provides no command execution. Validation records logs without automatically opening it, preserving the user's workspace.

## 20. Implementation order for a new app

Build a small component gallery before filling every domain editor. Use this order to make each new feature inherit the style:

| Step | Build | Completion criterion |
| --- | --- | --- |
| 1 | Token palette, fonts, scale conversion and localization lookup | One translated themed window renders correctly at multiple scales |
| 2 | Shared Button and ButtonBar | Exact endpoints, reversal, focus, disabled state and flush geometry work |
| 3 | Dropdown/menu infrastructure | Ordered keyboard navigation, input ownership, cancellation and nested menus work |
| 4 | Shell and command routing | Title dragging, menus, navigation, status, shortcuts and dirty-close flow work |
| 5 | FieldRow, typed drafts, numeric controls and swatches | Left labels, fixed-form wrapping, validation and grouped undo work |
| 6 | Single slider and interval slider | Knob/bar gestures, numeric display, focus and bounds work |
| 7 | Selection service and Properties inspector | No/stale/multiple selection policy and capability tabs are explicit |
| 8 | Asset bank, source picker and preview service | Owned imports, quick/detail results, cancellation and cache budgets work |
| 9 | Style banners/library and full-workspace viewer | Consistent-input previews, identity semantics and portable storage work |
| 10 | Preferences with live component gallery | Language/palette/layout preview and Save/Cancel/stale-save behavior work |
| 11 | Domain tools, layer/effect editors and floating layouts | Existing components reused, input targets isolated, Reset layout works |
| 12 | Acceptance pass | Functional regressions and native visual checks below are recorded |

Suggested framework-neutral component responsibilities:

```text
Theme              semantic palette + font/spacing application
Localization       message IDs + fallback + language state
Button             three styles + geometry + per-ID reversible animation
ButtonBar          fixed rounded-to-pixel height + square gapless children
PopupMenu          panel styling + keyboard/pointer ownership + dismissal
FieldRow           localized left label + width policy + fixed-form scope
ValueSlider        track + typed/dragged value + units + bounds
RangeSlider        low/high/bar gesture state + readout
ColorEditor        swatch + movable visual/numeric editor + valid-color draft
StyleBanner        recipe preview crop + name band + separate action targets
Inspector          active selection + capability tabs + nested editors
WorkspaceLayout    dock/float/recover + clipping and input coverage
PreviewService     generation-aware bounded quick/detail rendering
CommandService     validation + transactions + undo grouping
HostServices       dialogs + owned assets + persistence + background operations
```

These are responsibilities, not a public API that can be imported directly from this repository. Many current helpers are crate-private; port/adapt their behavior or deliberately extract a library. Avoid duplicating them inside every editor.

### Copyable implementation brief

> Build a compact square-edged desktop editor with two upper rows: a 24-point custom title/menu strip and a fixed-height navigation/action bar. Use a shared semantic palette, localized strings, three button styles and exact reversible hover/focus inversion with 0.15-second body and 0.20-second ink transitions. All adjoining bars, tabs and popup rows are square, gapless and match their panel at rest. Provide a selection-driven right Properties inspector with capability tabs, left-aligned property labels, shared scalar/range sliders, visual color editing and grouped validated undo. Represent presets with live rendered style banners and a full-workspace viewer. Preferences previews language, every theme token, typography and layout with Save/Cancel. Import artwork into an owned asset bank. Use generated transparent tintable icons with retained source/prompts. Render quick visible previews before asynchronous refinement, cancel obsolete work and bound queues/caches. Use the standard three-section in-app prompt for confirmations/errors and native Windows pickers for files. Implement the component gallery and acceptance checklist before adding app-specific editors.

## 21. Acceptance checklist

Use the following as the new app's definition of done. Existing source tests are a reference for test ideas, not evidence that a separate implementation passes.

### Geometry and theme

- [ ] Every adjacent button row is square, full-height, gapless and exactly matches its panel at rest.
- [ ] No button outline appears on hover, press, keyboard focus or disabled states.
- [ ] Icon-only content is centered in its actual body at different font sizes/scales.
- [ ] Hover/focus preserve text weight, control widths and layout.
- [ ] Changing each palette token updates its intended surfaces, including custom painters, without recoloring content.
- [ ] Light/dark/custom palettes retain readable endpoints and intermediate text/icon colors.
- [ ] Text, shared boundaries, separators and images remain crisp at 100%, 125%, 150% and 200% scale.

### Buttons and menus

- [ ] Mid-animation exit/re-entry reverses independently for each button.
- [ ] Disabled buttons do not execute or run an attention animation.
- [ ] Closed title headings do not open on hover; an open group switches both ways without stale commands.
- [ ] Escape/outside dismissal leaves title menus closed until explicit activation.
- [ ] Popups show one-point-smaller text without cumulative shrinking in nested menus.
- [ ] Short choice lists fit together; long lists navigate through offscreen enabled entries.
- [ ] A stationary mouse does not steal the keyboard candidate during repaint.
- [ ] Enter/Space commits once; candidate movement and cancellation do not alter history.
- [ ] Search fields/auxiliary actions remain usable; disabled rows cannot activate.
- [ ] Blend/preset temporary previews clear on every relevant dismissal/context change.

### Inspector and values

- [ ] Empty, deleted and changed-type selections never display stale controls.
- [ ] Multi-selection behavior is explicit and does not suggest unsupported bulk edits.
- [ ] Docking/floating preserves drafts, focus, tab/scroll identity and drop targeting.
- [ ] Drag cancellation and Reset layout recover tools; covered controls do not receive canvas input.
- [ ] Ordinary layer headers pin Opacity, Blend mode and icon/Name rows above scrolling properties.
- [ ] Property labels remain left of fields; fixed forms show full translated labels even when abbreviation is enabled.
- [ ] Resizable abbreviation/truncation exposes full names; divider changes commit once.
- [ ] Scalar sliders and typed values agree on units, precision and bounds.
- [ ] Range endpoints cannot cross; bar dragging preserves width at limits and never jumps on grab.
- [ ] Coincident range knobs separate predictably; keyboard and disabled behavior work.
- [ ] Invalid text/hex/numeric drafts preserve the last valid result and can be repaired/reverted.

### Files, previews and localization

- [ ] Save/Discard/Cancel and failed/cancelled pickers preserve the pending-action contract.
- [ ] All prompt paths, including picker errors, follow the same header/body/action layout.
- [ ] Imported/dropped assets remain available after moving/removing their original source file.
- [ ] Missing artwork remains repairable while remaining layers render.
- [ ] Banners compare real recipes using the same inputs; full previews preserve aspect ratio.
- [ ] Quick results appear before full quality, stale work cannot replace current images and memory/work queues remain bounded.
- [ ] Wheel zoom preserves the pointer's world point after pan, over artwork and at limits; +/− preserve canvas center.
- [ ] Every UI/error/tooltip/accessibility string uses message lookup or an intentional user-content path.
- [ ] Fallbacks, missing translations, long names, CJK and large fonts remain usable without changing stored IDs.
- [ ] Preferences Save commits; Cancel/close/Escape restore appearance; stale saves are rejected.
- [ ] Native visual checks and automated state/geometry checks are recorded separately.

## 22. Source map and audit notes

### What was reviewed

The extraction began with repository instructions and the existing UI contracts, then traced shared components and their callers through the native presentation, application and host layers. It surveyed the main modes (canvas, synchronization, Steam, Art Data Bank, Style Viewer), property/layer/effect families, preferences, localization, dialogs, rendering and regression coverage. Source searches across crates, bridge, build, installer and tools located the native UI and distinguished browser automation/build/fixture code from application controls. Generated packages and vendored UI internals were not treated as the application design authority.

The table is for navigation. The recipe above contains the portable behavior so the document remains useful when copied without these files.

| Concern | Primary implementation / contract |
| --- | --- |
| Repository style rules | [AGENTS.md](../AGENTS.md) |
| Title, menus, navigation and style-viewer contract | [Upper application bars](upper-application-bar.md) |
| Shared buttons, color animation, bars, popups | [buttons.rs](../crates/achimeld-ui/src/buttons.rs) |
| Custom title frame, grouped menu switching, resize controls | [chrome.rs](../crates/achimeld-ui/src/chrome.rs) |
| Shell, view state, status, shortcuts and GPU artwork cache | [UI desktop.rs](../crates/achimeld-ui/src/desktop.rs) |
| Popup appearance and input contract | [Popup dropdown guide](popup-dropdowns.md), [dropdowns.rs](../crates/achimeld-ui/src/dropdowns.rs), [dropdown_navigation.rs](../crates/achimeld-ui/src/dropdown_navigation.rs) |
| Layer/effect blend choices and temporary preview | [blend_preview.rs](../crates/achimeld-ui/src/blend_preview.rs), [blend_icons.rs](../crates/achimeld-ui/src/blend_icons.rs) |
| Selection/object editors and inspector docking | [inspector.rs](../crates/achimeld-ui/src/inspector.rs), [inspector_panel.rs](../crates/achimeld-ui/src/inspector_panel.rs), [window_layout.rs](../crates/achimeld-ui/src/window_layout.rs) |
| General-object properties and text/content fields | [achievement_general.rs](../crates/achimeld-ui/src/achievement_general.rs), [text_content.rs](../crates/achimeld-ui/src/text_content.rs), [text_alignment.rs](../crates/achimeld-ui/src/text_alignment.rs), [font_picker.rs](../crates/achimeld-ui/src/font_picker.rs) |
| Shared label layout and fixed/resizable policy | [fields.rs](../crates/achimeld-ui/src/fields.rs), [Field labels](field-labels.md) |
| Scalar/opacity sliders and interval sliders | [opacity.rs](../crates/achimeld-ui/src/opacity.rs), [particle_sliders.rs](../crates/achimeld-ui/src/particle_sliders.rs) |
| Visual color, gradient, map and curve controls | [color_picker.rs](../crates/achimeld-ui/src/color_picker.rs), [gradient.rs](../crates/achimeld-ui/src/gradient.rs), [gradient_map.rs](../crates/achimeld-ui/src/gradient_map.rs), [curves.rs](../crates/achimeld-ui/src/curves.rs), [Gradient Explorer](gradient-explorer.md) |
| Preference UI, schema/defaults and theme application | [settings.rs](../crates/achimeld-ui/src/settings.rs), [application preferences.rs](../crates/achimeld-application/src/preferences.rs), [theme.rs](../crates/achimeld-ui/src/theme.rs), [Preferences guide](preferences.md) |
| Interface language and font fallback | [localization.rs](../crates/achimeld-ui/src/localization.rs), [strings.csv](../crates/achimeld-ui/assets/strings.csv), [typography.rs](../crates/achimeld-ui/src/typography.rs), [language identities](../crates/achimeld-core/src/domain.rs) |
| Project localization import/validation | [translations.rs](../crates/achimeld-application/src/translations.rs), [host translations.rs](../crates/achimeld-app/src/translations.rs) |
| Standard prompt and native file picker flow | [Application prompts](application-prompts.md), [app_dialog.rs](../crates/achimeld-app/src/app_dialog.rs), [about.rs](../crates/achimeld-ui/src/about.rs), [change_log.rs](../crates/achimeld-ui/src/change_log.rs) |
| Canvas geometry, pan, zoom, selection and keyboard navigation | [canvas.rs](../crates/achimeld-ui/src/canvas.rs), [node_navigation.rs](../crates/achimeld-ui/src/node_navigation.rs), [sorting.rs](../crates/achimeld-ui/src/sorting.rs) |
| Layer stack, shared layer properties, effects and masks | [layer_palette.rs](../crates/achimeld-ui/src/layer_palette.rs), [layers.rs](../crates/achimeld-ui/src/layers.rs), [layer_effects.rs](../crates/achimeld-ui/src/layer_effects.rs), [layer_masks.rs](../crates/achimeld-ui/src/layer_masks.rs), [Effect rows](effect-rows.md), [Masks](masks.md) |
| Content-derived layer backgrounds and action-row geometry | [layer_backdrop.rs](../crates/achimeld-ui/src/layer_backdrop.rs), [layer_toolbar.rs](../crates/achimeld-ui/src/layer_toolbar.rs) |
| Specialized property families | [particles.rs](../crates/achimeld-ui/src/particles.rs), [explosion.rs](../crates/achimeld-ui/src/explosion.rs), [artifact3d.rs](../crates/achimeld-ui/src/artifact3d.rs), [text3d.rs](../crates/achimeld-ui/src/text3d.rs), [perlin.rs](../crates/achimeld-ui/src/perlin.rs), [vignette.rs](../crates/achimeld-ui/src/vignette.rs) |
| Style banners, rename/save and full viewer layout | [style_picker.rs](../crates/achimeld-ui/src/style_picker.rs), [style_viewer.rs](../crates/achimeld-ui/src/style_viewer.rs), [viewer layout.rs](../crates/achimeld-ui/src/style_viewer/layout.rs), [Agent style authoring](agent-style-authoring.md) |
| Preset libraries and portable viewer host behavior | [gradient_explorer.rs](../crates/achimeld-ui/src/gradient_explorer.rs), [effect_explorer.rs](../crates/achimeld-ui/src/effect_explorer.rs), [host style_viewer.rs](../crates/achimeld-app/src/desktop/style_viewer.rs) |
| Asset bank, general images, picker and drop routing | [art_bank.rs](../crates/achimeld-ui/src/art_bank.rs), [general_images.rs](../crates/achimeld-ui/src/general_images.rs), [source_picker.rs](../crates/achimeld-ui/src/source_picker.rs), [image_drop.rs](../crates/achimeld-ui/src/image_drop.rs), [Image-drop guide](editor-image-drops.md) |
| Read-only diagnostics window | [terminal.rs](../crates/achimeld-ui/src/terminal.rs), [diagnostics.rs](../crates/achimeld-application/src/diagnostics.rs) |
| Read-only remote state and staged review/confirmation | [steam.rs](../crates/achimeld-ui/src/steam.rs), [synchronization.rs](../crates/achimeld-ui/src/synchronization.rs), [synchronization_target.rs](../crates/achimeld-ui/src/synchronization_target.rs) |
| Generated artwork provenance and shared symbols | [Toolbar artwork README](../crates/achimeld-ui/assets/toolbar/README.md), [icons.rs](../crates/achimeld-ui/src/icons.rs), [layer_icons.rs](../crates/achimeld-ui/src/layer_icons.rs), [effect_icons.rs](../crates/achimeld-ui/src/effect_icons.rs) |
| Progressive rendering and result ownership | [preview_renderer.rs](../crates/achimeld-app/src/preview_renderer.rs), [preview state](../crates/achimeld-application/src/preview.rs), [imaging renderer.rs](../crates/achimeld-imaging/src/renderer.rs) |
| Commands, ports, history and host integration | [ports.rs](../crates/achimeld-application/src/ports.rs), [history.rs](../crates/achimeld-application/src/history.rs), [application lib.rs](../crates/achimeld-application/src/lib.rs), [host desktop.rs](../crates/achimeld-app/src/desktop.rs) |
| Renderer-specific text fidelity | [Paired renderer patch notes](../crates/achimeld-ui/third-party/README.md), [text_raster_regressions.rs](../crates/achimeld-ui/src/text_raster_regressions.rs) |
| Authority qualifications | [Security entry point](security/README.md), [invariant registry](security/invariants.json), [known gaps](security/known-gaps.md) |

### Useful regression families to retain

The UI source includes button tests and focused regression modules for dropdown navigation, preferences, inspector placement, field layout, window recovery, canvas anchors, layers/effects, styles, art banks and typography. The host has prompt and rendering tests. Read the assertions and input paths before using a passing fixture as proof of a behavior.

For a source change in this repository, use the existing Windows/Rust build wrapper and external build tree. The repository-wide offline source/fixture entry point is build/Audit.ps1 -Offline -Fixtures. Native screenshot tests are separate, including native_button_hover_screenshot, native_app_style_screenshot, native_app_prompt_screenshot and native_rgb_subpixel_text; they require their documented environment/output setup. Generated results belong outside source. A new app should have its own equivalent checks rather than inheriting Achimeld's qualification claims.

### Observed exceptions and limits

These were found during extraction and are recorded to distinguish the intended recipe from every existing rendering path. They were not changed by this documentation task.

| Observation | Evidence / implication for the new app |
| --- | --- |
| General controls can still be rounded | Preferences defaults to radius 6; bars/popups override it. Set the clean-slate general default to 0 for the requested square profile. |
| Standard prompt layout is not yet universal | desktop.rs still renders the generic Action could not be completed window through an ordinary framework window; app_dialog.rs has a separately padded picker-error/retry modal. Use the standardized three-section prompt for these paths in the new app. |
| Localization has fallbacks and untranslated literal paths | The CSV includes empty translation cells, and some headings/status/errors bypass lookup. Fallback prevents blank labels but is not complete translation coverage. Add missing-ID/coverage checks in the new app. |
| Specialized controls are not all built from the scalar helper | For example artifact3d.rs contains a direct framework slider. Treat the shared scalar/range behavior as the new-app standard, not every historical per-editor variation. |
| Some older property paths still expose stored color text | Project settings in settings.rs uses a text field for the unachieved-icon Tint value. The new-app standard keeps ordinary color properties visual and reserves encoded entry for the movable color editor. |
| Mask-source header is a specialized exception in source | layers.rs omits opacity/blend for mask-source editing although the general three-row contract applies to ordinary layers. Model mask semantics explicitly; do not accidentally copy that exception to unrelated layers. |
| Range readout is not a pair of editable endpoint fields | particle_sliders.rs paints min–max text. Add typed endpoint editing explicitly if needed. |
| Source gallery is not a complete accessibility qualification | Focus/labels/metadata and framework AccessKit support exist, but this review did not run screen-reader or comprehensive contrast acceptance. Verify the new app's supported assistive workflows. |
| Source survey is not runtime/package acceptance | No native visual tests, full audit, rebuild, installed Windows checks or live Steam flows were run for this documentation-only change. No application behavior was modified. |

### What to carry into the next app

Carry the component contracts, semantic palette, field and menu behavior, selection/inspector model, icon provenance, visual libraries and responsive preview architecture. Replace document types, integrations, storage format, operation names and domain-specific properties deliberately. The reusable deliverable is this UI recipe; Achimeld's Steam authority, wire schema and installed enforcement are separate contracts.
