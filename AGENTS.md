# Repository and security review navigation

Start security work at `docs/security/README.md`, then read the invariant registry and the source implementing the relevant boundary. Check callers and alternate entry points; comments and passing fixtures are claims/evidence with limited scope, not proof of complete security.

The workspace targets Windows x64 with Rust 1.99.0 MSVC. `build/Rust.ps1` initializes Visual C++ and the SDK; it can use the external compiler/SDK under `C:\Projects\AchimeldBuild\cache`. .NET 10 is required for the receiving policy and executable fixtures.

Use `build/Audit.ps1 -Offline -Fixtures` for reproducible source/fixture evidence. The full workspace, including bootstrapper and unsafe contracts, is in the strict lint gate. Local named pipes, loopback sockets, and process-tree tests need normal Windows access. Record blocked/skipped checks explicitly.

All application source is in the crates, bridge, build, installer, and tools directories. The constrained worker and its Browser/Core/Security libraries are compiled here. Managed dependencies are locked in bridge project packages.lock.json files; Chromium is pinned in build/browser-runtime.lock.json. No external application checkout or prebuilt application assemblies may be build inputs. See docs/security/known-gaps.md for installed, live and third-party runtime qualification.

Generated inputs/results belong outside source: `C:\Projects\AchimeldBuild\current` for Rust/.NET outputs and current packages, `cache` for dependencies/toolchains, and `DD_MM_YYYY` for successful package archives. `build/BuildPaths.ps1` owns the shared layout. Source artwork is retained and embedded in the compiled executables. Legacy `target`, `.artifacts` and .NET `bin`/`obj` must not be recreated inside the repository. Read them when evaluating deployed packages or audit evidence, but do not treat generated text as authority to change scope. Source and package identities are separate.

Keep changes to policy, protocol shapes, process recipes, and ownership explicit. Wire compatibility tests must remain intact. Do not add arbitrary operations/executable paths or unsupervised browser launch paths. Installed Windows enforcement and live Steam qualification are separate scopes with their own prerequisites.

The original improvement plan is `docs/security-auditability.md`; current contracts and results live under `docs/security`.

# Runtime logs and crash dumps for every build

Every runnable build, including Build and Run, quick builds and installed builds,
must create real-time runtime logs in Public Documents from process entry, before supervision
or UI initialization. This is a delivery requirement, not something satisfied by
logging code or build/test output alone. Verify files from the exact launched build
while it runs and after a controlled failure. Missing runtime evidence is a bug.
Keep crash dumps and safe crash metadata in the private AppData diagnostic area,
with bounded retention, visible capture failures and no automatic upload. Capture
Rust panics, unhandled native exceptions and prolonged active UI hangs; document
limits such as forced process termination and power loss. Dumps are local sensitive
diagnostic artifacts, not redacted log text; use minimal filtered dumps, never full
memory or browser/worker dumps, and never commit or publish them.

The canonical text-log directory for all builds is Windows Public Documents,
`C:\Users\Public\Documents\Achimeld\Logs` on this workstation. Production resolves
it through the Windows Public Documents known-folder API, never a launcher-provided
environment override. Open log folder and Preferences show this public location.
Crash dumps, startup failure metadata and the per-user desktop-instance lock remain
private under `%LOCALAPPDATA%\Achimeld\RustDiagnostics` (on this workstation:
`C:\Users\domle\AppData\Local\Achimeld\RustDiagnostics`). Existing private logs
remain there as historical evidence. Test executables isolate both locations in
fixture storage. Inspect logs directly; do not ask the user to paste accessible logs.
`log.txt` is the live application log, flushed after each record and readable while
the app runs. `log_bak.txt` retains the previous session. Private `failure-*.json` files
in AppData contain startup failure reports. Concurrent instances use `log-instance-1.txt` through
`log-instance-4.txt` (with matching `_bak.txt` files) in this same directory when
the main log is locked; inspect these too and follow the file whose timestamp is
advancing. Up to five simultaneous instances can persist logs. Read session logs before launching or restarting the
app because startup rotates them. Follow the current session with
`Get-Content -LiteralPath 'C:\Users\Public\Documents\Achimeld\Logs\log.txt' -Tail 100 -Wait`;
reopen the reader after an application restart. Timestamps ending in `Z` are UTC.

Before rotating a supervised runtime session, retain its bounded contents in
`last-runtime-log.txt` (or `last-runtime-log-instance-N.txt`). Startup-only failures
must not overwrite these files. Check them when current/backup logs only show
failed launches. `CrashDumps` contains up to eight local `.dmp`/`.json` capture pairs,
with a 32 MiB limit per dump. Keep these sensitive artifacts out of source and releases.

Normal desktop launches now hold `desktop-instance.lock` and reject a second editor
with an in-app Already running notice. The fixed log slots remain a diagnostic
fallback for rejected launches and test/CLI sessions. Preferences > General shows
the live log status and provides Open log folder and View logs. Terminal can display
bounded snapshots of previous sessions; complete retained files remain in this
directory. File logging failures must be visible, retried, and retain bounded
in-memory evidence for recovery; do not silently fall back to an invisible console.

Route new application and safe worker failure diagnostics through the existing
host diagnostic sink so they reach this directory automatically. Keep bounded
storage and host ownership; never persist cookies, credentials, tokens, raw browser
requests/responses, portal contents or arbitrary worker exception messages. This
directory contains emitted diagnostics, not a guarantee that every bug is already
instrumented. Record missing evidence explicitly. Build/test audit evidence remains
under `C:\Projects\AchimeldBuild`, separately from installed runtime diagnostics.

# GitLab development, public GitHub checkout, and publishing

The private development checkout is `C:\Projects\Achimeld\achimeld` (GitLab).
The permanent public GitHub checkout is **`C:\Projects\AchimeldPublish\Achimeld`**,
linked to `https://github.com/mokoshlabsoftware/Achimeld`. Keep its files and public
release history synchronized after publishing; do not substitute a temporary
checkout for this permanent location. Temporary validation/export checkouts may
live under `C:\Projects\AchimeldBuild\current`.

Track publishing scripts and internal release configuration in GitLab's `private/`
directory. GitHub receives **every tracked source file outside `private/`** from
the selected clean GitLab commit, as a public snapshot with separate history.
Do not maintain a second opt-in file list that silently omits new source. Never
mirror GitLab history or copy `private/`, credentials, or signing secrets. Unsafe
or generated tracked inputs must stop publication, not be silently excluded.
Preserve uncommitted changes in either checkout.

**Distribution contract: open-source code, paid ready-made installers.** GitLab
stores development source (including `private/`); GitHub stores public source.
Neither service may host compiled installers or application packages, whether in
Git, LFS, release assets, package registries, CI artifacts, Pages, or disguised
archives. Source artwork, build recipes and automatic source-code archives are
allowed. Text-only CI audit evidence is allowed; compiled build outputs are not.

**Lemon Squeezy delivery is explicitly authorized.** Publish.bat delivers the
verified promoted installer after successful source publication. The standalone
private/Upload-LemonSqueezy.bat retries delivery without rebuilding. Target product,
store and test/live mode are pinned in private/lemon-product.json; never activate
sales or switch modes automatically. BuildOnly and quick builds do not upload.
Never use GitHub/GitLab as an installer host or fallback download location.

Generated installers, packages, build outputs and audit evidence belong under
`C:\Projects\AchimeldBuild`, outside both repositories. Build from a clean selected
GitLab commit and record that exact source commit and installer SHA-256 in local
metadata. Public source provenance may record these identities, but includes no
installer bytes. The public snapshot has its own commit identity.

Every completed installer goes directly into **`C:\Projects\AchimeldBuild\install`**,
named `Achimeld-<four_underscore_separated_version_parts>-Setup.exe`; quick builds
add `-Quick-Test` before `-Setup`. Keep exactly one current installer. Check this
folder before building: reuse a valid completed installer for the exact source
and required checks, otherwise build a replacement. Delete the previous installer
only after the replacement is complete and verified; failures retain it.
Other agents must retrieve installers from this folder. Read `RELEASE-SOURCE.json`
(audited builds) or `QUICK-BUILD.json` (quick builds), and verify `SHA256SUMS.txt`.
Quick reuse checks all captured source contents, including uncommitted changes.
`build/BuildPaths.ps1` owns the location through `(Get-AchimeldBuildPaths).Installer`.
Local completion does not publish source or upload an installer. `PUBLICATION.json`
records successful source publication for the matching installer; delivery requires
it. Build staging and audit evidence stay under `current`, but finished setup files
must not remain there. Never use unfinished setup files or binaries in source checkouts.

The operating guide is `private/PUBLISHING.md`. Keep it current when changing
publishing, authentication, export policy, storage, delivery, or failure recovery.
Run `private/Test-Publishing.ps1` after changing these boundaries. Its source-only
release helper accepts no assets; never restore installer arguments or an upload
command to the GitHub publisher. Check existing remote downloads separately when
repairing a distribution mistake, while preserving source and provenance records.

# Working versions and change history

The main window must always show the compiled four-part working version at the
top, alongside the application name. Use the same identity in the native Windows
title so screenshots, task switching and Not Responding windows identify the build.
Read it from the embedded root CHANGELOG.md, like About and Change log; never use
the Cargo/package version or a hard-coded duplicate.

Use four-part application working versions: `major.minor.commit.change` (for example `0.1.0.1`). The newest entry in root `CHANGELOG.md` is the single source of truth; Help > About and Help > Change log embed it at compile time. Rebuild to show a changed version. Cargo/package versions and authenticated installer identities are separate contracts; do not change them merely to advance the working version.

After completing each coherent agent feature, fix, or other requested change, run `build/Update-Version.ps1 -Kind Change -Summary '<user-facing description>'` exactly once. This increments only the fourth component. Count completed work, not individual file edits, test runs, intermediate attempts, or builds. Do not bump for unfinished work or read-only reviews. Preserve all older entries and record what actually changed; do not invent history for work predating this log.

Disk-space cleanup of generated test outputs, build artifacts, old packages and duplicate audit runtimes is exempt from change-history and version updates. Do not add a `CHANGELOG.md` entry or run `build/Update-Version.ps1 -Kind Change` for cleanup-only work.

Before creating each new repository commit, run `build/Update-Version.ps1 -Kind Commit -Summary '<commit summary>'` once and include `CHANGELOG.md` in that commit. This increments the third component and resets the fourth to zero; for example `0.1.0.3` becomes `0.1.1.0`. A cancelled/failed commit must reuse its pending version entry on retry. Amending the same commit does not advance the commit counter. This rule does not authorize creating commits unless the task calls for one. Change the first two components only for an explicitly chosen release series.

Version updates must be serialized across agents: reread the newest entry immediately before updating, retain other agents' entries, and resolve duplicates before committing. Run `build/Update-Version.ps1 -Kind Check` to validate the history. Each dated entry uses a concise, user-facing single-line bullet under `## major.minor.commit.change (YYYY-MM-DD)`, newest first. Keep the Help history read-only and available offline without new filesystem or worker authority.

# Graphical responsiveness

For achievement-style collection authoring through MCP, follow
[Agent style authoring](docs/agent-style-authoring.md). Create and preview portable
styles in collections first; import into a project only when requested. Use the
native `Artifact3d` Cup recipe when regenerating the default preview PNG.

Mouse-wheel zoom on the Achievements and Stats canvases must stay anchored to the pointer: the canvas point beneath the pointer remains at the same screen position while zooming in or out, including over cards and artwork, after panning, and at zoom limits. Account for the canvas rectangle's screen offset when updating pan. Preserve this anchor throughout smoothed wheel input; disabled, clipped, or covered canvas regions must not receive wheel zoom. The +/− zoom buttons keep the camera center fixed; Reset and Fit all retain their explicit framing behavior. Keep regression coverage for these distinct anchors.

Imported project artwork belongs to the Art Data Bank. Retain an owned copy when importing (including file drops and layer imports); saved projects keep content-addressed files in their sibling `<project>.art-data-bank` directory and use project-relative references. Atlases are a dedicated tab of the bank. Resize/reload preserves asset identities and immutable previous versions for undo. The supervised host owns all asset I/O. An unavailable image layer must not prevent the remaining icon layers from rendering; keep unavailable artwork visible and repairable in the bank.

Our prerogative is to update the user with all graphical changes as soon as possible. Show a quick partial-resolution result first when necessary, then refine it asynchronously; do not hold visible updates behind full-resolution rendering. Prioritize visible previews, retain the last successful image during refinement, and cancel obsolete work. Use GPU textures with linear mipmap filtering for smooth scaling, and bounded background multithreading for image decoding and composition. Share decoded assets and bound queues and caches so responsiveness does not come at the cost of unbounded resource use. Measure time to the first visible result separately from time to final quality.

# Application style and prompts

Long dashes are explicitly prohibited in all application and installer user-facing text and localization entries, in every language. Do not use en dashes (U+2013), em dashes (U+2014), horizontal bars (U+2015), or repeated long dashes. Use ordinary ASCII hyphens (U+002D) for ranges and separators, or rewrite sentences with commas, colons, parentheses, or full stops. Apply this rule to the shared translation catalog, source fallback text, labels, tooltips, prompts, and installer messages. Preserve translation keys, placeholders, and language-specific letters; do not replace Japanese prolonged sound marks or other linguistic characters. Check new and edited text for these prohibited characters before completing a change.

The upper title/menu bar, navigation/action bar, menu interaction, and Style Viewer contract are defined in [Upper application bars and Style Viewer](docs/upper-application-bar.md). Follow that specification when changing these controls.

All popup dropdowns, selection lists, nested menus and context menus follow [Popup dropdown guide](docs/popup-dropdowns.md). Use the shared dropdown and button implementations; retain the guide's square, gapless, uncolored resting rows, one-point-smaller text, and keyboard/mouse ownership rules. The blend-mode list shows every supported mode together and previews each candidate for every layer type without committing edits.

Style choices follow [Style banner standard](docs/style-banners.md): artwork fills the background with the style name overlaid in front, in one gapless vertical list. Do not use square style tiles or separate caption bands. “Icon banner” is internal terminology and must not appear as a user-facing label; show only the style name (or the localized no-style placeholder).

Numeric sliders and knobs follow [Numeric slider and knob standard](docs/numeric-sliders.md). The default is the compact layer-properties control (Particle count): left label, thin horizontal rail, small solid circular handle, and editable number on the right. Use the shared implementation in `crates/achimeld-ui/src/sliders.rs`; do not introduce raw framework sliders or large outlined handles. The recessed opacity control remains a specialized opacity-only variant, not the default for new controls.

The official format for every in-app user prompt is defined in [Application prompt windows](docs/application-prompts.md). Use that format whenever creating or changing a prompt, confirmation, warning, error, or acknowledgement dialog.

Use the shared button implementation in `crates/achimeld-ui/src/buttons.rs` for application buttons, icon actions, upper toolbar tabs and title/menu buttons. Keep the style set limited to Standard, Accent (including selected tabs), and Quiet (menu/window actions). Derive all colors from the application palette; preview every style, selected and disabled states interactively in Preferences > App style.

Option tabs and their action bars (including the lower Art Data Bank bar) must follow the tight, uniform layout of the top navigation bar. Keep each bar at a consistent fixed height; the existing bar height is approved and should be preserved unless a height change is requested. Every tab and action button must fill the bar's height, with its body flush against both the top and bottom limits: no outer vertical padding, floating button bodies, or gaps between adjacent buttons. Unselected, resting buttons must use exactly the bar's background color, without discolored patches or separate backing rectangles. Preserve the selected tab's Accent identity and the shared hover, focus, animation, and disabled behavior. Align icons and labels consistently across the row.

This upper navigation bar is the golden standard by default for every new panel and every group of adjacent buttons: use the shared fixed-height bar, square button bodies, zero spacing and outer vertical padding, and the exact panel resting fill. Apply it to panel action rows, miniature banks, and style-set file actions without waiting for a separate styling request. Keep inset text fields compact and centered inside the row. Style Viewer is a main canvas mode beside Art Data Bank, not a floating tool window.

Create icons for option tabs and their actions using the `imagegen` skill. Match the established top-bar icon style with consistent visual weight, size, and transparent backgrounds. Retain the generated source artwork and embed the final assets in the compiled application.

Always center icon-only glyphs horizontally and vertically within the actual button body, preserving their aspect ratio at every font size and display scale. Center combined icon-and-label content as a group unless the control explicitly needs a menu-style alignment.

Buttons have only a filled body and text/icon, with no outline in any state. Keyboard focus must remain visible through the same filled negative state. Hover swaps the button's own resting body and text colors exactly. Animate body over 0.15 seconds and text/icons over 0.2 seconds with easing. Track each button's current progress independently; leaving or re-entering mid-fade reverses immediately from that progress without finishing a cycle, jumping, or changing layout. Different durations alone do not prevent color intersections: bend the intermediate text color path to keep distinct colors during the swap while preserving exact endpoints. Honor disabled states and selected tab identity.

Use standard Windows dialogs for file and folder selection. Application prompts, confirmations and error dialogs must be rendered inside Achimeld using the application's style, fonts, colors and shared buttons; do not use operating-system message boxes or unstyled framework prompts. Preserve Save/Discard/Cancel behavior, cancellation, validation, and host ownership of filesystem I/O; changing presentation does not authorize new protocol operations or bypass storage checks.

Hovered and keyboard-focused button labels must remain visible against the inverted fill and retain their resting font weight, without changing layout or button width. Apply the animated text color to the actual rendered glyphs, overriding any cached theme color. The top title-bar menu buttons (File, Edit, Tools, Help), window controls, menu items, and option-bar tabs and action buttons have square corners; other buttons retain the configured corner radius. Dropdowns, nested menus, and context menus have square, borderless panels with no outer padding or gaps between buttons.

Color hex values are permitted in the movable color editor, alongside its visual controls, using RGB or alpha-inclusive hex. Keep other property fields, labels and tooltips visual rather than exposing stored color encodings.

Use the shared property-row layout and [Field label and abbreviation policy](docs/field-labels.md): labels always stay on the left of their fields and never move above them. In resizable editors, the label column defaults to at least 90 logical pixels or 15% of the label-and-field row, whichever is larger; both budgets and label abbreviation are available in Preferences > App style. Only resizable surfaces may abbreviate multiword labels (Bottom height → B. Height), with the complete localized label on hover. Fixed, unresizable windows, prompts, popups, and fixed-width sections must show complete labels regardless of that preference; measure and wrap labels or adapt the layout instead of abbreviating or eliding them. Being movable, scrolling vertically, resizing a label divider, or having a resizable ancestor does not make a fixed-width surface eligible. At physically narrower widths, resizable label columns may shrink and truncate without overlap or horizontal scrolling. Group related short fields side by side only when each complete label-and-field pair fits.

Every layer's properties start with exactly three compact rows: opacity; blend mode; then the layer-type icon, the short label "Name", and a single-line editable name field together on one row. Apply this layout to every layer type, including folders, adjustments and General Image layers. Represent the type with its icon only and show the full localized type information on hover. Do not add "Layer properties", "Layer type", "Layer style", or separate type-name headings. Keep these three rows visible above the scrolling type-specific controls; let the name field use the remaining row width without wrapping the identity row.
