# Upper application bars and Style Viewer

This is the interaction and appearance contract for Achimeld's two upper rows. It complements [Application prompt windows](application-prompts.md) and the shared button rules in [AGENTS.md](../AGENTS.md). Sizes below are logical UI points, scaled to physical pixels by the display scale. Preserve the existing row heights.

All dropdown contents also follow [Popup dropdown guide](popup-dropdowns.md): one-point-smaller popup text, square gapless rows with the exact popup resting fill, and ordered keyboard navigation that retains ownership until the pointer moves.

## Title and menu row

The compact upper row has a 24-point panel height, with an 8-point left inset and no right or vertical inset. Menu headings use a 20-point minimum body height, centered vertically, and 12 points of horizontal button padding, with no extra vertical button padding. Window controls fill the row's height and sit flush against the top and right window edges. The title panel uses the application's **Panel** color.

From left to right it contains:

1. The white, transparent 18-by-18-point Achimeld mark, with an Achimeld tooltip. The mark is the leftmost content and also acts as a window drag target. All in-app identity surfaces, including About and prompts, share `crates/achimeld-ui/assets/achimeld-logo.rgba`, a 128-by-128 derivative of the supplied alpha glyph. The display PNG is capped at 256-by-256. Windows application and shortcut icons use the transparent orange glyph instead.
2. Five adjacent text headings: **File**, **Edit**, **Tools**, **Windows**, **Help**. There are no horizontal gaps between heading bodies. Each heading has square corners, uses the shared Quiet button, and rests on exactly the panel background.
3. Flexible empty space that acts as the main window drag target.
   When rejected entity/localization edits or a failed effect preset save need recovery,
   show **Revert to working version** in a temporary shared action bar immediately
   below the title row, keeping identity and window controls reachable at narrow
   widths. It discards pending failed
   edits and retains accepted project changes, including unsaved changes. Save and
   Save As remain available and save the accepted document; pending invalid drafts
   are not included. Recovery does not reload the last saved file or undo accepted work.
4. The application name and compiled four-part working version, right-aligned against the separator before the window controls. This identity is a drag target and must remain visible. The native Windows title uses the same identity, including when Windows shows a Not Responding title. The title row does not show Saved or Unsaved changes.
5. A separator, then **Minimize**, **Maximize/Restore**, and **Close** at the right edge. The controls are allocated from right to left, producing that visual order. Each has a 28-by-24-point body and a centered glyph, with no gaps between bodies. Maximize changes to Restore while maximized or fullscreen.

Pressing the primary mouse button on a drag target starts the normal window drag. Double-clicking a drag target toggles maximization; in fullscreen it first leaves fullscreen. Menu headings and window controls must not initiate a drag. Close requests the editor's normal exit flow, including validation and unsaved-change checks. Resizable window edges remain available outside maximized/fullscreen mode.

### Menu opening, switching, and dismissal

The bottom of **Tools** has a separator followed by **Terminal**. Terminal is a
resizable application window showing the current session's application statuses
and validation/worker stages with UTC timestamps. Output is read-only and supports
mouse selection, Ctrl+A, Ctrl+C, and **Copy all**. **Pause output** freezes the
display for selection while collection continues; **Resume output** catches up.
Closing and reopening retains the session history. The buffer retains at most
1 MiB and 4,000 entries and reports discarded older entries. It offers no command
execution or additional filesystem authority.
Selecting **Validate** records its diagnostics without opening Terminal or changing
its visibility. Open **Tools > Terminal** explicitly to view validation output,
including blocked requests and early startup failures.

**Preferences > General** shows the host-provided log folder and current file-logging
status, with gapless **Open log folder** and **View logs** actions. Terminal's session
selector switches between current output and read-only snapshots of saved session
logs from before this launch. Copy uses the displayed session; Clear and Pause are
disabled for historical snapshots. Clearing current output never deletes log files.

Terminal has a visible vertical scrollbar and wraps long lines. Scrolling upward
retains the reading position as output arrives. **Clear** empties only the current
in-memory log, including paused output; collection continues. The Sync view offers
an unchecked **Show Terminal during validation and synchronization** checkbox.
It automatically closes only a Terminal it opened. Opening Terminal from Tools
before or during the operation takes manual ownership and prevents automatic
closing. Successful verification in the current session retains the **Synchronize** label without
attention stripes until the document changes; with no pending review it is
disabled. An App ID with no remembered connection uses the primary **Validate**
action. A connection remembered from a previous session uses the primary
**Re-validate** action. Neither state adds a second button. Validation and synchronization failures remain
visible in the Sync view using the Error palette color, independently of general
status updates, until another validation starts. Errors belong to the current
editor session and Steam App ID and do not appear in another project.

| Input or state | Required result |
| --- | --- |
| Hover a heading while all title menus are closed | Animate the heading's hover feedback. Do not open a menu. |
| Click or keyboard-activate a closed heading | Open its dropdown. |
| Move onto a different heading while a title menu is open | Switch to that heading's menu immediately, without another click. This works in both directions and when skipping headings. |
| Switch headings | Stop displaying the previous dropdown before drawing the new one. Its commands must never remain under another heading's hover feedback. Release stale focus on the previous heading. |
| Move from a heading into its dropdown | Keep the dropdown open so its commands can be reached. |
| Hover a command | Show its hover feedback; do not execute it. |
| Activate an enabled command | Run that command once and close the menu as requested by the action. |
| Click the open heading again | Toggle its dropdown closed. |
| Press Escape or click outside | Dismiss the dropdown. After dismissal, hovering headings alone must not reopen it. |
| Hover disabled controls or a disabled title row | Do not switch menus or run commands. |

The five headings form one group. An unrelated popup elsewhere in the application must not enable title-menu hover switching. All heading responses are collected before any title dropdown is drawn, making switching independent of the headings' drawing order.

Dropdown panels, nested menus, and context menus have square corners, no border stroke, no outer padding, and no gaps between their button rows. Menu rows stretch to the dropdown's width. **Command labels are left-aligned and vertically centered**, including disabled commands and submenu triggers; their starting inset is the shared button padding. The compact title row must not shrink dropdown controls: menu contents restore the normal application button padding and interaction height.

### Menu contents

The following lists describe the English labels and their order. Existing localization applies where available.

| Menu | Commands in order |
| --- | --- |
| File | New project; Open project…; Open Recent; Save; Save as…; Project settings…; Preferences…; Exit |
| Edit | Undo, optionally followed by its history label; Redo, optionally followed by its history label; Duplicate selection; Delete selection |
| Tools | Gradient Explorer…; Style Viewer…; Art Data Bank; Import atlas…; Import image…; Export achievement icons…; Export achievement translations…; Import achievement translations… |
| Windows | Set window size…; Reset layout |
| Help | About; Change log |

Windows > Reset layout restores every floating window's default placement, including closed tools and movable color editors. It recenters Layers and clears its saved position, and returns Properties to its default right dock. Window contents, selections, drafts, open tools, layer split and canvas framing are preserved.

Windows > Set window size… opens a movable panel with editable width and height in logical pixels. Apply restores a maximized or fullscreen main window before setting its inner size; Cancel, Escape and the close button discard the draft. Width and height respect the main window's 720-by-650 minimum. Terminal may be dragged partially beyond the application bounds like Layers.

Layers can be moved by dragging passive captions, header gaps, unused body space and frame margins. Enabled buttons, editable fields, layer/effect rows, dividers, scrollbars and resize handles retain their own gestures. Disabled or covered panels cannot start a drag. A partially offscreen panel remains movable from its visible unused surfaces; Reset layout also recovers it without requiring a selection.

About displays the compiled four-part working version. Change log opens the same read-only Help modal on its dated history page, newest first; the bottom bar switches between About and Change log and closes the modal. Both pages follow the shared black header/body and full-width gray action-row prompt layout. Root `CHANGELOG.md` supplies both the version and history at compile time; rebuilding updates them. The fourth number advances for each completed agent change; a new commit advances the third and resets the fourth. See the version rules in `AGENTS.md` and `build/Update-Version.ps1`.

Open Recent is a nested menu containing up to ten full project paths, newest first. When empty, it shows a localized disabled placeholder. Selecting a path retains the normal validation and Save/Discard/Cancel flow. Undo and Redo are disabled when their corresponding history is unavailable. Existing draft checks continue to govern editing and destructive commands.

## Navigation and action row

The second row is a full-width **Panel** strip. Its fixed content height is calculated once per layout from the larger of the button font's pixel-rounded line height and a 24-point icon allowance, plus the shared top/bottom button padding, with a minimum of the shared interaction height. A single separator line is included in the panel's total height. Changing hover, focus, selection, or enabled state must never change that height or the controls' positions.

The leftmost tabs appear in this order:

| Tab | Icon and action |
| --- | --- |
| Achievements | Trophy; select the achievement canvas. |
| Stats | Stats artwork; select the stat canvas. |
| Sync | Circular arrows; select the synchronization view. |
| Steam | Steam artwork; select the Steam view. |
| Art Data Bank | Stacked image artwork; select the artwork bank. |
| Style Viewer | Generated cave handprint; select the style preview canvas. |

Tabs use embedded transparent artwork displayed at 20 by 20 points, followed by a 5-point icon-to-label gap. Center the combined icon and label group in its body. Preserve icon aspect ratios. The selected tab uses Accent; other resting tabs use exactly the bar's Panel color. Adjacent bodies touch, filling the content height from top to bottom without outer vertical padding. Selecting a different tab clears the canvas selection and pending selection/node drags; entering Achievements or Stats also selects that entity kind. Hovering a navigation tab only changes feedback; it does not switch views. When the label is hidden, that hover also shows the tab name.

After the tabs, the row contains Undo and Redo icon actions. Their tooltips include the available history action and Ctrl+Z/Ctrl+Y. Toolbar Undo also permits reverting invalid drafts. Redo requires a redo entry and no invalid drafts.

Achievements and Stats additionally show Sort, which opens the grid arrangement tool. Creation and deletion live in the canvas right-click menu. Release a short right click on the background, a card, or its artwork to open it, including when a busy startup frame receives the press and release together. Holding the right button and moving drags the camera with slight inertia, easing toward the pointer and settling without overshoot after release; it never opens a menu. Keep the drag and its settling in one camera undo step. New pointer input, zoom, Escape, focus loss, disabled canvas, view changes, and externally changed cameras cancel the remaining glide. The menu creates the current entity kind near the original click, choosing the nearest unoccupied grid position across all cards, including filtered cards. Right-clicking an unselected card selects it; right-clicking a selected card preserves the current group for Delete selection. Achievements also offer Icon layers for the clicked card. Dropping images onto empty achievement canvas creates achievements near the drop position through the host-owned Art Data Bank import, with no creation prompt and no overlapping cards. Dropping onto existing artwork retains the General images/Icon layers destination choice. Achievements, Stats, and Steam show the **Filter by name or API name** field. This field is intentionally inset inside the row; the flush-body rule applies to buttons and tabs. Its preferred width is 210 points. As the window narrows, the field shrinks down to 96 points before any neighbor is allowed to cover it. Narrower than that, the navigation tabs and the staged action keep their icons and drop their labels; hover shows the full label. The project name then shrinks with an ellipsis. The main window's minimum inner size is 720 by 650 points.

At the right, the row shows the project display name and a staged action: **App ID… → Validate → Re-validate → Synchronize**. Without a valid App ID, **App ID…** opens Sync, clears selection and opens the Steam App ID editor. **Validate** has no ellipsis and uses the generated magnifying-glass/checkmark icon, matching the Validate button inside Sync. It opens Sync and prepares a fresh Steamworks comparison without applying changes. **Re-validate** is the same preparation after a previous session already connected: it confirms the internet connection and the Steam partner connection before synchronization, and it uses the same Validate icon. **Synchronize** retains the circular-arrows icon. Active background operations disable preparation. Once a reviewed plan is available, **Synchronize** remains disabled until the destination is confirmed and at least one change is selected. Choosing it opens a review of the planned actions before anything is applied. That review lists new achievements that will be created, entries marked for deletion on the Steam page, and the other selected uploads and imports. Marked deletions are shown in the review and are not sent to Steam. Cancel or Escape closes the review without applying. Synchronize in that window applies the already selected plan. A **Stop** action is available during a read. These are actions, not hover-activated menus.

The Steam page lists the imported Steam achievements and stats as selectable tiles. **Read Steam** imports that configuration through the existing read and clears deletion marks. Selecting a tile marks it for deletion, shown with a circle on the tile. When one or more entries are marked, **Delete on Steam** appears in an orange-red body and opens a scrollable review of those achievements and stats. Closing the review keeps the marks. Read Steam and Delete on Steam use generated toolbar icons. The review lists the marked entries; it does not add a remote deletion command.

Sync includes **Edit Steam App ID…**, which opens a separate standard in-app Save/Cancel prompt. It accepts only a positive 32-bit decimal App ID, commits through project history, and refuses stale drafts. Changing the destination clears its old remote snapshot, comparison baselines and deletion markers while preserving local entries and artwork; undo restores the previous state. Finish or cancel a Steam operation before editing the ID. Validation uses the existing receiver-owned complete read and reviewed plan. Show the matching App ID, fixed Steamworks achievements address and reviewed entries, then require explicit destination confirmation before applying any selections. Confirmation is bound to the review token, App ID and editor revision; a fresh plan requires confirmation again.

Before this session has a live Steam connection, the **Sync** navigation tab uses the shared “wants to be clicked” cue. Inside Sync, the cue guides **Edit Steam App ID…** while no valid ID is present, then **Validate** for an ID that has never connected, or **Re-validate** when a previous session already connected. Clearing the ID or undoing its entry moves the cue back to the editor. Alternating black/white stripes at 45 degrees move slowly over the button body with Screen blending at 20% opacity. The overlay sits below the label/icon, follows the body's corners and clipping, and preserves layout, style and interaction. Disabled and invisible buttons do not animate. Cues stop during an operation, when a matching review is available, and after the current session verifies the document. Preferences > App style previews the cue and its disabled state.

After the authenticated configuration read, validation also attempts to fetch the game's [462 × 174 small capsule](https://partner.steamgames.com/doc/store/assets). The receiving worker discovers the current content-addressed capsule through public Steam metadata for the pinned App ID, derives its double-resolution image and falls back to the legacy CDN route; the existing `prepareSync` response adds optional `gameBanner` bytes. The host bounds and decodes them, retains an ordinary **Steam game banner** image in the Art Data Bank, and saves it with project-relative content-addressed artwork. Sync shows this image with its aspect ratio preserved, including after reopening offline. Refresh keeps the artwork identity and previous immutable versions for undo. Cancelling the Steam review retains the downloaded banner. Missing or invalid artwork displays an unavailable message and does not invalidate a successful configuration read or confirm the destination automatically. Steam access has three layers. No remembered connection for the current App ID is unvalidated. A saved connection from a previous session is its own layer: the partner banner is drawn at half brightness (each color channel multiplied by 128), with a yellow **Validated in a previous session** label centered on the banner. Re-validate before synchronizing. A matching review or a successful verification in the current session draws the banner at full color and uses **Synchronize**. No worker operation or process recipe is added; live Steam qualification remains separate.

## Movable properties sidebar

Achievements and Stats share a **Properties** sidebar. Drag its full-width header to float it inside the application; release within 40 logical points of the left or right workspace edge to dock on that side. An Accent preview marks the destination before release. Release elsewhere to keep it floating. The canvas expands into the freed space. Docked widths and floating window sizes remain resizable; floating panels stay inside the workspace when the main window shrinks.

Click or keyboard-activate the header for **Dock left**, **Dock right**, and **Float** placement commands. The header follows the shared fixed-height, square, gapless bar/button contract, and the placement list follows the popup guide. Escape, focus loss, disabling the workspace, or leaving the editor cancels an unfinished drag. Only the header initiates movement; property controls retain their normal input. The same selection, invalid drafts, tabs, scroll identities, layer properties and host-owned artwork drop routing remain available in every placement. Placement survives canvas/view and project switches during the session and does not alter project history. A floating inspector covers the canvas for pointer and wheel input.

## Icon style nodes

The Achievements canvas right-click menu also offers **Create icon style node** near the original click. These cards use the same vacant placement, selection, dragging, duplicate/delete, undo and owned Art Data Bank artwork as ordinary achievements. The existing local `styleAchievement: true` field keeps its compatible storage name. Creation atomically adds a distinct project style and stores its identity in the optional local `iconStyleId` field. The initial recipe displays General image ID 0 and uses an unlock value of 10. Duplicating a node creates another distinct style, even when the recipes match. The node's banner resolves this explicit identity before comparing recipes, and marks only its linked style selected.

An icon style node has **General** and **Style** inspector tabs. General contains the displayed decimal unlock value, preview text, General images and a **Linked style** selector. Choosing a project style changes the explicit link and adopts that style's recipe without overwriting the chosen style; **No style** clears the link while retaining the editable recipe. These edits save with the project and support undo/redo. Style contains the recipe/layers and layer properties. Both tabs reuse the generated General/Icon inspector artwork and the shared fixed-height bar. The node has no Steam configuration or localization editing tab and cannot enable **Affect style**. Node recipe edits update its linked project style in the same validated undo step; editing that exact saved style updates every explicitly linked node. Other helper cards remain independent even when their recipes match. Choosing a saved style in the Layers recipe picker copies its recipe into the node's linked style. Deleting a saved style clears its node link while retaining the editable card and artwork; undo restores both. Older nodes without a link retain their independent recipes. No worker or wire field is added.

The icon banner selects among project styles. **Import global style into project** and **Save current icon as a project style** are adjacent direct icon buttons beside it, using embedded generated folder/arrow and disk artwork. Their square, gapless bodies fill the existing compact header height, rest on exactly Panel, and use the shared tint, focus and hover feedback. Import opens the global template list directly. Save opens the standard in-app naming prompt with the icon preview and Save/Cancel actions; validation, cancellation and undo retain their existing semantics.

Sorting leaves these cards at their saved positions, including selected-only sorting, and reserves their full rectangles when arranging other cards. The host excludes their records and generated artwork from Steam synchronization, restores their exact records and text before saving the result, and rejects returned identity collisions. The application also rejects synchronization transactions that change local style cards or their text. No worker command or wire shape is added. Installed/live Steam qualification remains a separate scope.

## Shared visual feedback

Use `crates/achimeld-ui/src/buttons.rs` for button bodies, glyph colors, focus feedback, and animation. Use only Standard, Accent, and Quiet.

The default appearance is a dark charcoal Panel with off-white Primary text and a blue Accent. On Windows the host supplies Segoe UI; the normal/button font starts at 13 points and follows Preferences. The shared default button padding is 12 points horizontally and 7 points vertically. Palette and font preferences change these controls together; the title row retains its compact override.

- Standard rests on **Control**, Quiet on **Panel**, and Accent on the palette's selected fill (the Control/Accent blend). Text and tinted artwork use **Primary text**.
- Hover or keyboard focus exchanges the button's own resting fill and ink exactly. The fill eases over 0.15 seconds; text/icons ease over 0.2 seconds. The text's intermediate color path bends to avoid crossing the fill color.
- Every button tracks its own animation progress. Leaving and re-entering reverses from its current progress immediately. Font weight, bounds, and content placement stay unchanged.
- Disabled controls keep their resting body and use subdued ink; they do not animate or activate.
- Title headings, window controls, dropdown rows, and navigation/action bar buttons have square corners and no outline in any state. Other buttons retain the configured radius.
- Keyboard focus uses filled feedback. The animated ink must be applied to the rendered glyphs, including cached text and tinted images.

Preview shared states interactively under Preferences > App style. Application color controls display swatches and visual controls, never color hex values.

## Style Viewer

**Style Viewer** is a main canvas mode after Art Data Bank. Its list contains exactly two collapsible folders: **Project** (styles in the open project) and **Built-in** (read-only shipped recipes). No collection subheadings, saved app library, or remembered foreign project libraries appear. A compact name filter searches both folders without changing selection or fold state. The selected style name appears on its banner, not above the preview; rename is a right-click action. Project recipes refresh on every project revision, including creation, editing, undo, redo and switching projects. The built-in 3D object style is excluded; shipped names do not expose legacy product names.

Banners are gapless, wide, centered crops in full color. Only the name's tight envelope receives the shared button fill and animated text colors; artwork outside the envelope remains unmodified. The main preview shows the complete icon, supports drag-to-pan and pointer-anchored wheel zoom from 10% to 1600%, and has a compact **Center** action at its upper-left edge that recenters and resets zoom. Disabled or covered canvas areas do not take gestures. Camera changes are presentation state, separate from achievement canvas framing and document history.

The generated **Style options** action opens a movable, resizable window. It contains banner height (48–240 points, default 64), resolution (64/128/256/512 pixels, default 512), decimal unlock value (default 10), preview General image IDs 0–2, and a miniature Art Data Bank with All/General/Icons/Atlases filters. Clear affects only its chosen image ID. Preview inputs are saved in project metadata and support undo without changing achievement artwork. A very small **Clone all built-ins** button creates independent editable project copies in one undo step. It does not apply them to icons.

Layers is available alongside this canvas. The right sidebar is empty until a layer/effect is selected, then shows the shared layer/effect properties. Project recipe edits update using icons and explicitly linked style nodes in one validated project-history transaction. Built-ins are read-only; duplicate one to edit it. Context actions duplicate any displayed style into the project, rename project styles, or delete them. Deletion uses the standard in-app prompt: preserve current icon appearance and break the link, or adopt a chosen remaining project style. Cancel changes nothing; undo restores the style and affected icons together. Independent local helper cards are never inferred to be linked merely because their recipes match.

### Style JSON and preview ownership

Style options provides **Load styles…** and **Save styles…**, using standard Windows `.json` pickers. The versioned JSON contains layer recipes, not project or image files. General image IDs make a recipe portable. Direct image/atlas identities require matching artwork in the receiving project's Art Data Bank; filesystem paths in recipes are rejected. Files are bounded to 16 MiB and 128 styles, validated completely before additive import, and receive fresh style/layer/effect identities. Import is undoable. Export uses an unused destination and never overwrites unrelated data. Cancelled pickers and invalid files preserve project state.

The supervised host owns the independent preview document, asset store, renderer and file access. Composition retains bounded asynchronous quick/detail passes, cancellation, shared decoding and linear mipmap filtering. Visible banners request previews; last successful textures remain during refinement. A missing image layer leaves other layers renderable. Blend-mode hover is temporary and never alters the saved recipe. The embedded cup artwork remains an ordinary PNG fallback, not a built-in editable 3D style.

Legacy `.achv` collection authoring, exports and local Steam-set metadata remain supported by the explicit collection tools; they are not additional libraries in the viewer. Their existing validation and host storage ownership remain in force. This UI change adds no worker operation or Steam authority.

## Implementation and acceptance

| Responsibility | Source |
| --- | --- |
| Upper-row contents and navigation | [`desktop.rs`](../crates/achimeld-ui/src/desktop.rs) |
| Grouped heading responses, hover switching, and window controls | [`chrome.rs`](../crates/achimeld-ui/src/chrome.rs) |
| Shared buttons, left-aligned menus, and popup styling | [`buttons.rs`](../crates/achimeld-ui/src/buttons.rs) |
| Viewer presentation | [`style_viewer.rs`](../crates/achimeld-ui/src/style_viewer.rs) |
| Closed viewer intents and set-link data | [`application/style_viewer.rs`](../crates/achimeld-application/src/style_viewer.rs) |
| Viewer isolation, rendering, files, and image imports | [`host/style_viewer.rs`](../crates/achimeld-app/src/desktop/style_viewer.rs) |

Check both hover-switch directions, switching across multiple headings, pointer movement into a dropdown, command activation, Escape/outside-click dismissal, disabled states, left-aligned enabled/disabled command labels, and existing nested menus. Check unchanged geometry at fractional display scales and configured fonts. For the viewer, check canvas/history isolation, image ownership, portable save/load, cancellation, invalid-file preservation, set-link conflicts and cross-game identity. Headless tests are evidence for those paths; native visual acceptance and live Steam qualification are separate checks and must be reported accurately.

## Reusable art images

The Achievements canvas context menu offers **Create art image**, using the same vacant placement, selection, dragging, duplicate/delete and undo as local style cards. An art image exposes only an **Image** inspector: its name, independent layers/style and General images. It has no achievement progress, Steam configuration or localization tab. Shared style mutation skips it. Automatic sorting skips helper cards by default; the Sort panel's **Include helpers at the end** toggle includes icon style nodes and art images after all regular sorted nodes, respecting selection-only sorting and row/column filling. The toggle is saved with the other grid preferences and defaults to off for existing settings.

An art image’s composed transparent output appears among **Images** in the existing source picker, labeled **Art image**. Choose it from General images > Choose from Art Data Bank, then use that General image ID in any icon layer; choosing it directly as a layer source creates the General image slot automatically. These are live identity links: editing the source updates every consumer through the existing quick/detail background preview passes. Art images can reuse other art images; cycles and chains beyond sixteen links are rejected. Deleting a source empties its General image slots without renumbering the remaining slots or removing layer effects, and undo restores the links.

Project schema version 1 retains an optional local `artImage: true` marker on the shared canvas record and an optional `artImageId` on General image entries. Older records keep their existing meaning. The host excludes art images from Steam preparation and restores their exact records after synchronization. Existing host rendering resolves the links before upload/export; no worker operation or protocol shape is added. Achievement icon and translation exports omit art image records. Imported input files remain owned by the Art Data Bank and save as project-relative, content-addressed files beside the project.

Steam and Sync utility actions use a single fixed-height, gapless bar immediately below navigation, with horizontal scrolling at narrow widths. Resting actions match the panel fill. The primary Validate, Re-validate and Synchronize actions are the explicit exception to borderless buttons: square bodies with a one-physical-pixel white outline, retaining shared hover, focus and disabled behavior.
