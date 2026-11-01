# Wiki maintenance and version coverage

[CHANGELOG.md](../CHANGELOG.md) is the authoritative software change list and four-part application version source. [Update notes](update-notes.md) describes the existing exporter for changes between versions. This register maps that history to wiki updates; it does not duplicate release notes.

## Workflow for every software change

1. Review impacts on concepts, controls, workflows, screenshots, examples, translations and troubleshooting.
2. Run the existing Change version step in AGENTS.md exactly once per coherent completed change. Record the actual assigned version, never a guessed future version.
3. Add a row below with affected stable article IDs and required work, or a specific no-impact reason. Commit-only entries may reference the rows for their included changes.
4. Update articles when available; leave unavailable work Pending. Recording impact does not authorize website deployment.
5. Verify affected behavior and article content against the intended application build/source. Record the method, evidence and exact version. Source review does not prove runtime or live Steam acceptance.
6. Mark Verified when ready for publication; mark Published only after confirming the deployed documentation revision. Retain historical rows when later changes create new work.

Impact states: Pending, In progress, Verified, Published, Not needed.
Content states: Planned, Draft, Verified, Published.

## Change impact register

| Application version | Affected article IDs | Required work or no-impact reason | Status | Evidence |
| --- | --- | --- | --- | --- |
| 0.2.34.0 | See 0.2.33.1-23 | Commit-only version for the included changes; article work remains tracked in the corresponding change rows | Pending | Source commit and release verification pending |
| 0.2.33.23 | None | No wiki article impact: the requested installer delivery route changes release publication, while the current wiki drafts describe in-app use and local installation behavior | Not needed | Publishing boundary fixtures and route check passed; release delivery pending GitHub authorization |
| 0.2.33.22 | explosions, gradients | Document deltoid morph -100% to 100%, offset -1000% to 1000%, Brightness label and selected gradient stop alpha; refresh control screenshots | Pending | Source and wiki draft updated; focused imaging, application, UI and managed fixtures passed; full offline audit passed before the subsequent publishing route edit; rebuilt desktop screenshot and runtime logs pending |
| 0.2.33.21 | particles, explosions, gradients, icon-layers | Show square corners on Particle, Explosion, Gradient and Shape Explorer windows and the 2D Gradient Collection; refresh affected screenshots | Pending | Source reviewed against popup and button contracts; strict UI Clippy passed. Native desktop appearance and runtime logs unverified |
| 0.2.33.20 | explosions, icon-layers | Describe boom as the new explosion layer default and refresh default screenshots | Pending | Supplied preset embedded unchanged by SHA-256; wiki drafts updated; focused Rust test blocked by access denied on the shared Cargo build lock; rebuilt desktop and runtime logs unverified |
| 0.2.33.19 | particles, explosions, gradients, icon-layers | Describe shared effect opener and quick load, Unsaved changes entries, icon size knobs, Shape Explorer for Shape layers and particle composites, and revised gradient presets; refresh control and Explorer screenshots | Pending | Draft articles updated; effect Explorer 2 focused UI tests, Gradient Explorer 3 focused UI tests, shape recipe test and app source check passed. Full UI suite: 310 passed, 4 failed (2 related failures fixed and retested, 2 unrelated fixture failures), 46 ignored. App persistence test stopped by full disk during test link; rebuilt desktop and runtime logs unverified |
| 0.2.33.18 | explosions, icon-layers | Explain the resizable construction strip and larger Explosion construction editor with all geometry and whole-effect offsets; refresh Properties screenshots | Pending | Source and wiki draft reviewed; UI and test source checks passed; focused runtime test stopped by full disk while linking; rebuilt desktop, runtime logs and screenshots unverified |
| 0.2.33.17 | exports, getting-started | Document File icon export window, saved selection/format/naming/folder, and refresh workflow screenshots | Pending | Export article draft updated; affected-crate Rust check and all 5 icon-export tests passed; rebuilt desktop and runtime logs unverified |
| 0.2.33.16 | particles, icon-layers | Document MagicDust as the bundled Particle Explorer preset and new-layer starting recipe; refresh affected screenshots | Pending | Supplied collection embedded unchanged; source and wiki draft reviewed; focused Rust test blocked by access denied on shared Cargo build lock; rebuilt desktop and runtime logs unverified |
| 0.2.33.15 | particles, icon-layers | Describe the small range-mode icon beside particle range labels and refresh the affected control screenshot | Pending | Source and slider standard updated; focused Rust regression blocked by an unrelated compile error in gradient_explorer.rs; rebuilt desktop and runtime logs unverified |
| 0.2.33.14 | particles, icon-layers | Describe the responsive, centered square spawn diagram and refresh its screenshots | Pending | Particle wiki draft updated; width and centering regression passed at three panel widths and two artwork aspects; drag regression rerun blocked by an unrelated compile error in gradient_explorer.rs; rebuilt desktop and runtime logs unverified |
| 0.2.33.13 | sync, steam, troubleshooting | Explain that Validate downloads the pinned game's banner into the Art Data Bank, that a missing banner does not fail sign-in, and refresh the validation screenshot | Pending | Receiving-policy fixture (46 cases), Rust browser tests (16) and managed worker build passed; desktop compile blocked by an unrelated in-progress gradient editor type error; full offline audit stopped after unrelated format differences; rebuilt desktop and live Steam verification pending |
| 0.2.33.12 | explosions, icon-layers | Describe the supplied Anime Effect as the new explosion layer default and refresh default screenshots | Pending | Bundled preset matches supplied file by SHA-256; wiki explosion draft updated; focused Rust test blocked by access denied on shared Cargo build lock; rebuilt desktop and runtime logs unverified |
| 0.2.33.11 | icon-layers, artifacts-3d | Explain that new 3D artifact creation is hidden and the default 3D object style preset is removed; existing layers remain editable | Pending | Source and article review; focused Rust test blocked by access denied on the shared Cargo build lock; rebuilt desktop review pending |
| 0.2.33.10 | explosions, particles, exports | Describe replacing an existing effect collection and refresh export prompt screenshots | Pending | Picker validation and atomic replacement regression passed; prompt source reviewed; rebuilt desktop appearance and runtime logs not verified |
| 0.2.33.9 | automation, sync | Review draft MCP validation, token-pinned synchronization review and apply, cancellation, and top title menu instructions in a rebuilt desktop | Pending | Draft articles updated; offline Rust check and strict lint for affected crates passed; MCP tests: 13 passed, 2 native checks ignored; focused editor guard test passed. Full audit stopped after unrelated format/bootstrapper lint failures; live Steam, desktop menus and runtime logs remain unverified |
| 0.2.33.8 | sync | Document Open achievements and Open stats beside Sync actions, browser lifetime and project App ID scope; refresh Sync action screenshot | Pending | Rust application check, 21 Sync UI tests, 39 receiving-policy cases, browser worker build and browser fixtures passed; live Steam, rebuilt desktop and runtime-log verification pending |
| 0.2.33.7 | explosions, particles | Describe the fixed left preview and scrollable right controls; refresh both Explorer screenshots | Pending | Draft articles updated; focused UI test passed at compact and wide sizes; native Explosion and Particle Explorer screenshots reviewed; rebuilt desktop runtime and log verification pending |
| 0.2.33.6 | sync, steam | Document selectable review for every proposed change and artwork-only icon updates; refresh Sync screenshots | Pending | Managed synchronization fixtures: 260 passed; focused UI icon selection test passed; Rust sync tests passed in broad run, with three unrelated preview timeout failures under concurrent builds; live Steam and rebuilt desktop review pending |
| 0.2.33.5 | None | No article impact: remove the layer panel hover tooltip; dragging and documented workflows are unchanged | Not needed | Source reviewed; focused compile check blocked by access to the shared Cargo build lock |
| 0.2.33.4 | getting-started, troubleshooting | Explain the online installer SHA-256 comparison with the published source record, matching GitLab source commit, Retry and offline failure; refresh setup screenshots | Pending | Production installer syntax, non-installing preview and source publishing fixtures passed; live GitHub response, installed acceptance and screenshot review pending |
| 0.2.33.3 | getting-started | Refresh the startup screenshot to show the four-part working version at 50% alpha below the logo | Pending | Bootstrapper source reviewed and offline Windows Rust check passed; rebuilt loading-screen appearance and runtime log evidence pending |
| 0.2.33.2 | getting-started | Document Help > Achimeld Wiki opening https://achimeld.com/wiki/ in the default browser | Pending | Fixed host-owned URL reviewed; offline application compilation, version and whitespace checks passed; rebuilt desktop and runtime verification not performed |
| 0.2.33.1 | getting-started, troubleshooting | Refresh welcome updates screenshot for visible scrollbar and separators only between dates. Supplied logo file is entirely white; replacement pending usable artwork | Pending | Source reviewed; focused welcome layout/dismissal check passed at normal and increased scale; rebuilt desktop and controlled-failure runtime evidence not performed |
| 0.2.32.1 | getting-started, localization | Installer now includes Hungarian and Turkish, matching all 16 application languages | Pending | Generator, publishing boundaries and non-installing language fixtures and preview compilation passed; installed acceptance pending |

| 0.2.31.1 | particles, explosions, masks, localization | Refresh narrow mask row screenshots; controls and workflows remain the same. Repair empty translation row and nested menu pointer ownership | Pending | UI suite: 313 passed, 46 native checks ignored; rebuilt desktop visual and runtime evidence pending |

| 0.2.30.1 | Build tooling | No article impact: source formatting only, no application behavior or controls changed | Not needed | Audited installer format failure corrected with cargo fmt |

| 0.2.29.27 | Website wiki renderer, all illustrated articles | Reserve screenshot dimensions before lazy loading to stabilize section navigation | Verified locally; publication pending | Route/asset checks passed; desktop and 390-pixel browser review; no version promotion |
| 0.2.29.26 | all-sections, particles, explosions | Add phase 5 readiness audit and acceptance queue; reconcile article hashes and check every capture manifest | In progress | Nine automated website checks passed; partial desktop observation in 0.2.29.23; full runtime and publication pending |
| 0.2.29.25 | icon-editor, preferences | Refresh Properties screenshots without the heading; describe the narrow docking grip and stable divider-controlled width | Pending | 16 focused inspector checks passed, including layer/tab width stability, divider resizing and docking; 4 native screenshot checks ignored; changed-file formatting and version checks passed; workspace formatting check blocked by existing formatting differences in other edited files; rebuilt desktop visual and runtime log evidence not performed |
| 0.2.29.24 | getting-started, troubleshooting | Refresh startup notes description and screenshot for full logo, version/date header, flat change bullets and Ok | Pending | Focused UI layout and dismissal regression passed at normal and increased scale; desktop visual and runtime log verification pending |
| 0.2.29.23 | particles, explosions | Document matching preset toolbar file controls, complete-effect exports and library imports; refresh toolbar screenshots | Pending | Draft articles updated; both-kind save-button regression passed; preset persistence check passed; compact explorer check failed (360-point panel expanded to 425.4375); native screenshot check skipped; rebuilt desktop and controlled-failure runtime evidence not performed |
| 0.2.29.22 | explosions, icon-layers | Document the supplied anime recipe as the default for new explosion layers and refresh the default preview | Pending | Embedded supplied Explosion.achexplosions; source reviewed; 7 explosion regression checks passed; native screenshot check ignored; rebuilt desktop appearance and runtime log evidence not performed |
| 0.2.29.21 | icon-editor, particles, icon-layers | Document the 1/2 range-slider toggle, midpoint selection and restoration of the previous interval; refresh affected screenshots | Pending | Shared control source reviewed; 10 focused slider interaction checks passed, including mode restoration; rebuilt desktop appearance and runtime log evidence not performed |
| 0.2.29.20 | icon-editor, particles, explosions, preferences | Refresh selected rotary control screenshots for the 10% Accent cell fill replacing the dial focus ring | Pending | Shared control source reviewed; 11 focused knob checks passed; rebuilt desktop visual review and controlled-failure runtime log evidence not performed |
| 0.2.29.19 | gradients, icon-editor | Refresh color selector screenshots for square corners and continuous pointer outline; color editing workflow unchanged | Pending | Offline UI compilation passed; rebuilt desktop visual review and runtime log evidence not performed |
| 0.2.29.18 | first-achievement | Add supplied General tab screenshot to Supply a General Image; identify standalone input ID 0 | Verified locally; publication pending | User-provided screenshot; application version unknown; article target and full-runtime verification unchanged |
| 0.2.29.17 | first-achievement, particles | Add illustrated Create layer > Particles workflow, layer order and Screen blending | Drafted; local checks pending; publication pending | User-provided example; screenshot application version unknown |
| 0.2.29.16 | particles, icon-layers | Document the supplied Light start recipe as the default for new particle layers | Pending | Embedded user preset; source reviewed; rebuilt desktop verification and website publication pending |
| 0.2.29.15 | particles | Update toolbar and screenshots for grouped count/size, shape and Center X/Center Y/Edge knobs | Pending | Focused particle and shared knob tests passed; desktop runtime and controlled-failure log evidence not performed |
| 0.2.29.14 | first-achievement | Illustrate right-click > Create achievement with the user-provided context menu screenshot | Verified locally; publication pending | User-confirmed workflow; screenshot application version unknown |
| 0.2.29.13 | masks, first-achievement, icon-editor | Illustrate artwork inside a folder with a shared star mask; link the practical example from introductory guides | Verified locally; publication pending | User-provided screenshot and source review; screenshot application version unknown; full workflow verification pending |
| 0.2.29.12 | first-achievement | Add user-provided image-drop screenshot; explain Properties > Icon and the existing General Image layer | Verified locally; publication pending | User-confirmed workflow; screenshot application version unknown, not promoted to target or verified version |
| 0.2.29.11 | troubleshooting, icon-effects, masks | No control or workflow changes. Fix accessibility lock conflict when opening effect rows, including masked folders | Not needed | Runtime log identifies UI hang after adding OuterLight; source fix and accessibility-enabled regression coverage; rebuilt desktop launch and controlled-failure runtime evidence not performed |
| 0.2.29.10 | icon-layers | Document shape Scale beside dimensions and the compact X/Y Size group; update shape screenshots | Pending | Offline UI compilation passed; runtime appearance and website update pending |
| 0.2.29.9 | masks | Update channel-control descriptions and screenshots for square colored R/G/B/A buttons and the invert icon | Pending | UI source reviewed; offline compilation passed; runtime appearance and website update pending |
| Recorded history through 0.2.29.0 | All sections | Inventory current functionality and reconcile historical changes through the initial wiki target | Pending | No wiki baseline verified |
| 0.2.29.8 | first-achievement | Explain the Create achievement action and dropping one or more image files onto empty canvas; distinguish drops onto an existing card | Verified | User correction and docs/editor-image-drops.md; website rebuilt and local preview checked |
| 0.2.29.7 | README wiki link | Publish the README Wiki link to https://achimeld.com/wiki/ alongside the deployed wiki in phase 5 | Pending | README.md contains the link; public target returned 404 on 2026-10-30; GitHub unchanged |
| 0.2.29.6 | Website wiki integration | Build 24 English wiki articles with navigation, search, version labels and language-page links; deployment remains pending | Verified | wiki/phase-4-website.md; eight automated checks and desktop/mobile browser checks |
| 0.2.29.5 | Phase 3 reference articles | Add 18 local draft references and model/source coverage; verify individual controls and illustrations before publication | In progress | wiki/reference-coverage.json, wiki/source-evidence-phase3.json |
| 0.2.29.4 | getting-started, first-achievement, achievements-general, general-images, icon-editor, projects | Add six local draft guides and research/capture records; complete runtime acceptance and publish in later phases | In progress | wiki/README.md, wiki/articles.json, wiki/screenshots/0.2.29.0/manifest.json |
| 0.2.29.3 | getting-started, troubleshooting | Document startup change notes, acknowledgement state, skipped versions, unknown history and downgrade behavior | Pending | Source implementation and regression checks; wiki articles remain planned |
| 0.2.29.2 | wiki-maintenance | Development documentation establishes impact tracking and version coverage; no existing user article changes | Not needed | AGENTS.md and this register |

Do not close the initial all-sections row until a detailed coverage inventory accounts for every view, control and workflow. Reference CHANGELOG versions instead of copying complete improvement lists.

## Wiki authoring progress

The [wiki authoring workspace](wiki/README.md) contains the [phase 1 inventory](wiki/phase-1-inventory.md), [screenshot checklist](wiki/screenshot-checklist.md), source discovery register, versioned captures, six phase 2 core guides and 18 phase 3 reference drafts. Source baseline: 0.2.29.3. Runtime captures: 0.2.29.0. Full runtime acceptance and historical impact reconciliation remain pending. Article verification versions remain unknown until their complete scope is checked.

See [Phase 4 website integration](wiki/phase-4-website.md) for the local website implementation and publication handoff.

## Article register

| Stable article ID | Scope | Last verified version | Evidence | Content state |
| --- | --- | --- | --- | --- |
| 0.2.33.1 | getting-started, troubleshooting | Refresh welcome updates screenshot for visible scrollbar and separators only between dates. Supplied logo file is entirely white; replacement pending usable artwork | Pending | Source reviewed; focused welcome layout/dismissal check passed at normal and increased scale; rebuilt desktop and controlled-failure runtime evidence not performed |
| 0.2.29.18 | first-achievement | Add supplied General tab screenshot to Supply a General Image; identify standalone input ID 0 | Verified locally; publication pending | User-provided screenshot; application version unknown; article target and full-runtime verification unchanged |
| 0.2.29.17 | first-achievement, particles | Add illustrated Create layer > Particles workflow, layer order and Screen blending | Drafted; local checks pending; publication pending | User-provided example; screenshot application version unknown |
| 0.2.29.14 | first-achievement | Illustrate right-click > Create achievement with the user-provided context menu screenshot | Verified locally; publication pending | User-confirmed workflow; screenshot application version unknown |
| 0.2.29.13 | masks, first-achievement, icon-editor | Illustrate artwork inside a folder with a shared star mask; link the practical example from introductory guides | Verified locally; publication pending | User-provided screenshot and source review; screenshot application version unknown; full workflow verification pending |
| 0.2.29.12 | first-achievement | Add user-provided image-drop screenshot; explain Properties > Icon and the existing General Image layer | Verified locally; publication pending | User-confirmed workflow; screenshot application version unknown, not promoted to target or verified version |
| getting-started | Concepts and first project | None | Draft in wiki/; source review and scoped 0.2.29.0 observations | Draft |
| achievements-general | Identity, text, unlock settings, stat links, Steam participation | None | Draft in wiki/; source review and scoped 0.2.29.0 observations | Draft |
| general-images | Artwork inputs and recipe slots | None | Draft in wiki/; source review and scoped 0.2.29.0 observations | Draft |
| icon-editor | Composition and shared controls | None | Draft in wiki/; source review and scoped 0.2.29.0 observations | Draft |
| icon-layers | Every layer type and parameter | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| icon-effects | Every effect and parameter | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| masks | Grayscale, alpha, folders and references | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| styles | Reusable recipes, collections and Style Viewer | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| art-data-bank | Images, atlases, compositions and asset repair | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| stats | Types, numeric settings and achievement links | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| sync | Differences, conflicts, imports, uploads and recovery | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| steam | Game association, sign-in, reads and Steam workflows | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| localization | Languages, templates and CSV exchange | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| exports | Icons, translations and collections | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| projects | Files, saving, moving and recovery | None | Draft in wiki/; source review and scoped 0.2.29.0 observations | Draft |
| preferences | Appearance and workspace settings | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| troubleshooting | Failures, diagnostics and recovery | None | Draft in wiki/; source-evidence-phase3.json | Draft |
| first-achievement | First project through save/reopen/export tutorial | None | Source-reviewed draft in wiki/; end-to-end runtime check pending | Draft |
| gradients | Detailed art reference | None | Source-reviewed draft in wiki/; runtime and illustrations pending | Draft |
| particles | Detailed art reference | None | Source-reviewed draft in wiki/; runtime and illustrations pending | Draft |
| explosions | Detailed art reference | None | Source-reviewed draft in wiki/; runtime and illustrations pending | Draft |
| text-3d | Detailed art reference | None | Source-reviewed draft in wiki/; runtime and illustrations pending | Draft |
| artifacts-3d | Detailed art reference | None | Source-reviewed draft in wiki/; runtime and illustrations pending | Draft |
| automation | Local MCP and desktop prerequisites | None | Draft in wiki/; source-evidence-phase3.json | Draft |

Split large sections into child articles as investigation proceeds. These IDs are proposed destinations, not existing URLs. Maintain redirects for renamed published routes. Each translation has independent verification and publication state and records its source article revision.

## Required wiki metadata

Each article must retain id, locale, documentedVersion, lastVerifiedVersion, verificationMethod and evidence, application sourceRevision, lastReviewed ISO date, and documentation contentRevision. Record screenshotVersion per asset when screenshots differ. Translations also retain translationSourceRevision. Identify dirty source snapshots where relevant.

Use the four-part application working version, never website, Cargo or installer versions. Record compiled versions for runtime evidence and screenshots. Planned or unverified articles have explicit unknown values and stay outside published navigation. Record feature introduced/changed/removed versions where useful without assuming compatibility with every intermediate version.

The article displays Covers Achimeld followed by documentedVersion, plus Last verified in followed by lastVerifiedVersion when different. Never stamp articles with the newest application version automatically.

## Publication record

- Publication state: Not published.
- Target application version: 0.2.29.3 for the initial draft; no published coverage claimed.
- Change history reviewed through: Not assigned.
- Published documentation revision and URL: None recorded.

The wiki landing page shows its target application version; individual articles retain their own version labels. Flag articles with unresolved applicable changes at or below the target as awaiting update and identify those changes. An older verification number alone does not prove an article is outdated.

Reconcile every change between the previous reviewed-through version and target using build/Get-UpdateNotes.ps1 and the range rules in update-notes.md. The lower bound is excluded and target included. For an unknown lower bound, explicitly review the full recorded history. Every returned version needs an impact decision, including no-impact and commit-only references.

Advance reviewed-through only after all changes in that interval are triaged; it measures review coverage, not article completion. Advance publication identity only after deployment is confirmed. List remaining Pending and In progress work without implying full version coverage.

Before publication, check routes, links, assets, search entries, article metadata and visible version labels. Preserve documentation revision history. A version selector and archived wikis are separate future work.

## Wiki implementation in five phases

| Phase | Work | Deliverable and completion condition | Status |
| --- | --- | --- | --- |
| 1. Inventory and investigation | Inspect every view, menu, context menu, shortcut, dialog, conditional control and error state; map source and runtime evidence | Coverage register assigns every discovered feature an article ID and verification task; select the target application version | In progress; inventory and initial captures ready |
| 2. Core concepts and first workflow | Explain projects, achievement General settings, identity/text/unlock values, stat links, General Images, icon recipes and reusable styles | A consistent example takes a newcomer from a new project through an achievement, icon, save/reopen and export | Draft guides ready; end-to-end verification pending |
| 3. Complete feature reference | Cover all layers/effects/masks, Art Data Bank, Style Viewer, Stats, localization, exports, Preferences, Steam, Sync, automation and troubleshooting | Every inventoried control and workflow has reviewed instructions, examples and evidence; unresolved verification stays visible | Draft references ready; control/runtime and illustration checks pending |
| 4. Website integration | Add /wiki/ routes, main navigation/footer links, responsive article templates, section navigation, breadcrumbs, contents, local search and article version metadata | Local website build serves accessible articles and direct URLs; route/link/asset/search checks pass; language fallback is explicit | Implemented locally; checks passed; publication pending phase 5 |
| 5. Verification publication and upkeep | Audit coverage, verify representative workflows, review desktop/mobile layouts, publish reviewed content and reconcile future change impacts | Confirm deployed revision and version labels; retain remaining tasks and update this register for each software change | In progress; first verification pass recorded; runtime acceptance and publication pending |

Run phases in order; findings can create follow-up work in earlier phases. No phase is
complete solely because its pages have been written. Phase 4 extends the existing
static website build and hosting. Publication remains separate from local authoring.
