# Achimeld Rust rewrite roadmap

Status: implementation started on 2026-10-10. See [current implementation status](docs/rewrite-status.md) for completed work and limits. Baseline inspected on 2026-10-10.

This roadmap describes intended Rust editor work. Unchecked tasks and acceptance criteria are not implemented or qualified functionality. All current application source and build inputs live in this repository.

## Local implementation references

| Area | Local source | Next work |
| --- | --- | --- |
| Domain and synchronization | `crates/achimeld-core`, `bridge/Achimeld.Core` | Complete validation, ordering and conflict semantics |
| Project storage | `crates/achimeld-persistence` | Schemas, transactions, CSVs and owned assets |
| Desktop interaction and imaging | `crates/achimeld-app`, `crates/achimeld-imaging` | Editor UI and recipe renderer |
| Steamworks | `crates/achimeld-browser`, `bridge/Achimeld.Browser`, `bridge/read-worker` | Qualify reads, then reviewed writes |
| Security | `crates/achimeld-bootstrapper`, `crates/achimeld-security`, `bridge/Achimeld.Security` | Installed acceptance and continuous audit |
| Distribution | `build`, `installer`, `tools/bridge-package` | Independent release identity and reproducibility |

Compatibility vectors are committed under `tests/fixtures` and regenerated from local application source. Source migration and dependency provenance are documented in [migration review](docs/security/migration-audit.md). Official signing and production updates remain release prerequisites.

## Contracts to preserve

- **Projects:** retain `.achv`, schema versions 0 and 1, stable IDs, referenced localization CSVs, atlas/image references, node positions, cameras, recipes, remote snapshots, tombstones, and synchronization baselines. Read `com.achiever.project` and `com.achimeld.project`; write the Achimeld identity.
- **Durability:** preserve JSON/CSV consistency, catalog hashes, interrupted transaction recovery, safe Save As behavior, and legacy transaction/journal identifiers. Deleting imported artwork occurs only after the corresponding project save commits, within the project's ownership boundary.
- **Numeric and hash behavior:** preserve decimal values and the existing canonical encoding. Rust byte lengths, enum names, float formatting, or iteration order must not silently change stored fingerprints.
- **Editor:** keep undo/redo, atomic edits, dirty-state prompts, filtering, selection, duplication, grid snapping, pan/zoom, and independent atlas assignment, canvas position, and Steam display order. Camera navigation is persisted without creating edit history or a dirty prompt by itself.
- **Localization and appearance:** retain application/project catalog separation, stable localization IDs, English fallback, live language/theme preview, and complete Cancel restoration. UI colors come from the configured palette and central derivations; artwork retains its authored colors.
- **Icons:** retain stable tile assignment, folder/mask/link semantics, shared styles, adjustments, transforms, and achieved/locked pairs. Steam export automatically includes grayscale locked artwork. Preserve imported achieved/unachieved assets as paired owned data.
- **Steam:** pull, compare, approve selected operations, apply, and read back. Preserve the user-triggered Synchronize action for ordinary conflict-free exports and the separate Import action. Validate origin, App ID, page shape, and expected previous values. An interrupted or unverified write is uncertain, never successful. Port current achievement deletion/rename behavior; add explicit destructive-operation review as a documented safety improvement.
- **Security:** retain the supervised launch boundary, isolated worker/browser, dedicated profile, authenticated bounded IPC, signed inventory, and Windows-enforced network restrictions. Login and Steam Guard stay in the visible Steam browser. Publication remains manual, and deletion runs only through an approved typed operation.
- **MCP:** target the live editor session, share its commands and undo history, check project/revision pins, and deduplicate identical request retries. Preserve the existing local tool scope; Steam writes stay in the reviewed UI workflow.

## Proposed Rust architecture

Use a Cargo workspace with boundaries matching the responsibilities already separated in the current domain contracts. Names and crate count can be refined during the foundation milestone.

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
crates/
  achimeld-core/          domain, validation, fingerprints, diff and sync plans
  achimeld-application/   document session, commands, history, use cases, ports
  achimeld-persistence/   JSON/CSV, preferences, journals, recovery, asset ownership
  achimeld-imaging/       decoding, composition, previews, export
  achimeld-browser/       worker-side Steamworks adapter and browser lifecycle
  achimeld-security/      supervision contracts, IPC, profile and policy checks
  achimeld-app/           desktop shell and adapter composition
  achimeld-steam-worker/  isolated browser and Steam artwork operations
  achimeld-bootstrapper/  adapted existing Rust verifier, supervisor and broker
  achimeld-mcp/           stdio server and editor connection
browser-driver/          bundled JavaScript Playwright recipes and locked dependencies
tools/
  achimeld-package/       inventory and package construction
tests/
  fixtures/              sanitized projects, CSVs, images, portal pages, IPC cases
  acceptance/            Windows, packaging, and authorized live workflows
docs/
  decisions/             architecture decision records
  parity.md              legacy behavior checklist and verification evidence
  migration.md           user data, settings, recovery, and rollback instructions
```

Dependency rules:

1. `core` owns behavior and has no desktop, browser, or Windows dependencies.
2. `application` depends on `core` and defines infrastructure ports. Manual edits and MCP edits use the same command/session layer.
3. Persistence, imaging, and browser adapters implement those ports. Their concrete library types stay out of domain models and UI state.
4. The desktop composes local services and a typed Steam-worker client. Browser automation and Steam network operations execute only in the supervised worker/driver tree. The Rust worker controls a private JavaScript Playwright child; the desktop has no generic browser-control API.
5. Windows APIs and unsafe/FFI code stay in narrow infrastructure modules with explicit safety and lifetime contracts. The bootstrapper retains its independent verification role.
6. Background work is bounded and cancellable. The editor owns document mutation; revision/generation checks prevent outdated background results from replacing current state.

## Milestones

The sequence is dependency-driven, not a calendar estimate. Set estimates after the UI/image spikes and the working-browser bridge proof. Prove read/write synchronization through a small Rust harness early, before waiting for the complete editor or new icon renderer. The Steam workstream follows S0–S4 in [the detailed plan](docs/steamworks-migration.md). Each milestone ends with a demonstrable result and recorded evidence in `docs/parity.md`.

| Milestone | Outcome | Prerequisites |
| --- | --- | --- |
| M0 | Verified working Steam baseline, parity inventory, UI/image decisions | None |
| M1 | Cargo workspace, checks, and supervised application skeleton | M0 |
| M2 | Existing projects load/save/recover safely | M1 |
| M3 | Usable achievement/stat editor and localized preferences | M2 |
| M4 | Complete offline icon authoring and export | M3 |
| M5 | Working-worker bridge, Rust reads, then JavaScript Playwright driver parity | M1, M2, M0 baseline |
| M6 | Early draft-sync harness, complete UI synchronization and reconciliation | M2 plus M5 bridge stage for harness; M3/M4 plus final M5 driver for editor parity |
| M7 | Rust MCP server with existing editor tool parity | M3, M4 |
| M8 | Qualified installed package and release prerequisites | M1–M7 |
| M9 | Migration rehearsal and replacement of the compatible project reader | M8 |

M5's initial bridge and M6's guarded writer are developed together as S1. The early read/write proof uses the local C# worker; it does not wait for M5's final JavaScript driver or the complete desktop UI.

### M0 — Inventory and feasibility

- [ ] Record a legacy source revision and any relevant working-tree changes. Identify representative projects, screenshots, exports, and workflow recordings.
- [ ] Build `docs/parity.md` from source and tests. Mark each behavior required, intentionally changed, or deferred, with a reason and an acceptance case.
- [ ] Compare Rust UI candidates in a small executable. Exercise a node canvas, docked/floating layers, image textures, inspector forms, Unicode text, file dialogs, clipboard, numeric scrubbing, keyboard focus, DPI, and accessibility.
- [ ] Prove an image backend using PNG/JPEG, PSD/PSB composites and layer extraction, text, masks, gradients, blending, and transparent resizing. Record any native-library dependency and licensing obligations.
- [ ] Freeze and reproduce the working Playwright read/create/update/text flow, including the asynchronous progress dropdown and confirmed save/read-back. Specify exact recipes and serialized worker operations using the detailed Steam plan.
- [ ] Commit to the existing C# worker as an initial bridge and JavaScript Playwright as the final bundled driver. Verify the matched runtime/browser and private transport; evaluate another automation mechanism only against a demonstrated failure and the same acceptance suite.
- [ ] Investigate the legacy installed-worker acceptance failure. Separate an existing policy/runtime defect from any new Rust adapter defect.
- [ ] Record UI, imaging, browser transport, async execution, numeric representation, and platform decisions in `docs/decisions/`. Pin a tested toolchain and dependency set after these spikes.

Exit: the working synchronization baseline is recorded, the chosen worker/driver migration is concrete, and the UI/image spikes remove blocking assumptions. The temporary C# worker has a parity/removal gate. Live validation uses an authorized test App ID when one is available; fixture tests remain credential-free.

### M1 — Workspace and launch foundation

- [x] Create the workspace and initial executable entry points with a lockfile and toolchain pin. A signed development bridge builder now exists; final Rust packaging and release qualification remain pending.
- [ ] Add Windows CI for formatting, linting, unit/integration tests, and release builds. Add dependency/license checks and a repeatable local build command.
- [ ] Adapt the existing Rust bootstrapper instead of replacing proven verification logic without cause. Review manifests, roles, paths, executable names, process ownership, and helper inventory for the new package.
- [ ] Implement Rust supervision/IPC contracts and a minimal editor launched through the bootstrapper. Preserve restricted named pipes, message bounds, authentication, expiry, replay rejection, and kill-on-close Jobs.
- [ ] Build the minimal Rust Steam harness and adapt the local worker's executable/peer-role assumptions inside a verified development package. Preserve the existing framed wire contract before changing transport.
- [ ] Establish sanitized logging, error categories, cancellation, progress, and development package diagnostics.

Exit: a development package launches the Rust shell through its supervisor, detects an altered package, rejects forged supervision, and closes supervised children on supervisor loss. Passing source tests alone does not qualify installed network enforcement.

Current evidence: the Rust read harness, private transport and signed package are implemented. App/worker detached launch and a signed offline Node adapter pass process/stdio regression tests. Protected installation, signature/policy verification and effective write denial have passed. Installed runs exposed inherited Windows executable-separator and accepted-socket mode bugs; both have corrections and regression coverage. The private browser starts, but full network acceptance and a successful read of authorized App ID 4873800 still require proof with the corrected installed package; see the status document.

### M2 — Domain, compatibility, and persistence

- [ ] Port achievements, INT/FLOAT/AVGRATE stats, dependencies, localization ownership, atlas geometry, recipes, validation, sync states, and three-way planning.
- [ ] Port DTOs for schema 0/1, defaults and aliases, unsupported-version handling, input-size limits, and reference validation. Reject unsupported fields/recipes that cannot be preserved rather than silently dropping them.
- [ ] Preserve canonical fingerprints with golden vectors for .NET decimal/float/date formatting, legacy enum type names, ordinal ordering, NFC/newline normalization, and UTF-16 length prefixes. Hash encoded text as UTF-8, matching the legacy implementation.
- [ ] Implement JSON/CSV hash validation and staged transactional writes, including recovery at every interruption point. Preserve compatible legacy transactions and sync journals.
- [ ] Port settings locations, preferences, recent projects, icon-style/gradient libraries, and legacy data aliases. Reuse dedicated profile locations only through the existing profile policy; do not copy session material.
- [ ] Preserve relative/absolute references and imported artwork ownership for both `.achiever-assets` and `.achimeld-assets`. Test Save As, missing assets, containment, reparse points, and deletion after commit.

Exit: the Rust loader opens the representative legacy corpus without losing fields or IDs. Rust-saved schema-1 copies can be read by the compatible project reader where compatibility is promised. Crash-injection tests recover a matching JSON/CSV pair and preserve the last durable artwork. Canonical vectors and plans match the baseline.

### M3 — Editor, localization, and preferences

- [ ] Build the Working/Steam shell, achievement/stat canvases, selection, filters, inspector, menus, keyboard commands, status, and progress surfaces.
- [ ] Port create/update/duplicate/delete, stat linking, validation, shared command execution, undo/redo, dirty state, New/Open/Save/Save As, and save-on-exit prompts.
- [ ] Preserve grid snapping, negative positions, multi-selection, pan/zoom, and layout persistence. Moving nodes must not alter atlas assignments or Steam display order.
- [ ] Port app and project CSV catalogs, IDs, placeholders, fallback, and live switching. Retain longer-translation and font-size layout checks.
- [ ] Port the full implemented palette and shared derived states, rounded controls, numeric text editing/scrubbing, and Preferences category behavior. Save persists the working copy; Cancel or close restores the original state.
- [ ] Keep invalid input visible and prevent partial inspector edits from corrupting the document. Background previews and reads must leave the UI responsive.

Exit: users can open a legacy project, edit achievements/stats/localization, arrange nodes, undo/redo, save, and reopen with equivalent data. Keyboard, DPI, theme, preference cancellation, and dirty-state workflows pass acceptance checks.

### M4 — Icon pipeline and full offline authoring

- [ ] Implement atlas slicing and stable tile assignment; standalone images; PSD/PSB workflows established in M0; General images; and imported Steam artwork.
- [ ] Port recipes: raster/text/solid/gradient/shape layers, achievement image links, folders, masks, opacity, blending, transforms, and ordered effects/adjustments.
- [ ] Preserve folder-scoped curves, Catmull–Rom interpolation, gradient stops and hexadecimal lists, contrast, hue/saturation, selective color, gradient overlay, sharpening, outlines, shadows, blur, and inner lighting.
- [ ] Port shared styles and independent layer/effect IDs when cloning. Preserve live updates, cancellation restoration, layer ordering, and docked/floating layer editing.
- [ ] Implement progressive shared-style previews: drafts at most 64 pixels, all drafts settled before full renders, two shared render slots, cancellation, and stale-result rejection. Scale pixel parameters from the original canvas; never change project dimensions to obtain a draft.
- [ ] Implement achieved/locked export pairs, alpha-aware resizing, cache keys, output formats/dimensions, sanitized filenames, and per-icon export errors.
- [ ] Compare representative outputs with the old renderer. Use exact comparisons for deterministic geometry/metadata and documented pixel tolerances for rasterization or text differences.

Exit: the agreed recipe/asset corpus renders and exports correctly, including nested folders, masks, transparency, rectangular canvases, linked text, shared styles, and locked artwork. Performance is measured against the same baseline hardware/projects; no unsupported recipe is silently flattened on save.

This completes the offline editor milestone. Steam development can continue without blocking local authoring acceptance.

### M5 — Preserve Playwright and migrate the worker

- [ ] Complete S1: reuse the existing C# Playwright worker through Rust IPC, with Rust snapshot parsing and the early read/plan/execute harness. Do not replace portal interaction while also rewriting its planner.
- [ ] Complete S2: translate the real `PlaywrightChromeDriver`/`LiveSteamworksDraftWriter` recipes to bundled JavaScript Playwright under a Rust worker. Keep profile/proxy settings, field mappings, readiness waits, and save responses equivalent.
- [ ] Preserve the actual `fetchstats`/`fetchachievements` read path and `statTable`/`achievementTable` editor path. Synthetic fixture selectors alone are insufficient browser parity.
- [ ] Bundle and inventory Node, Playwright, scripts, and the matched browser; keep the dedicated visible profile and private driver pipes.
- [ ] Enforce typed operations, approved origins/hosts, App ID and page validation, redirect handling, bounded transport, sanitized snapshots, and worker shutdown.
- [ ] Port snapshot parsing, achievements/stats/localization/icon metadata, completeness checks, imported artwork, and fixture coverage.
- [ ] Expose connection/authentication/reading/error states, cancellation, stale snapshot presentation, profile reset, and current enforcement status.
- [ ] Exercise real installed IPv4/IPv6 TCP/UDP restrictions and permitted broker traffic for the changed process tree, including browser/driver children. Reproduce and close the legacy worker acceptance gap.
- [ ] Run opt-in, visible, serial live reads with an authorized test account/App ID. Manual sign-in and Steam Guard remain user actions.

Exit: Rust can drive the working workflow through the old worker, then the JavaScript driver passes the same reads and selected draft-write cases before the old helper is retired. The installed package authenticates manually, imports a complete snapshot/artwork, handles browser closure, and rejects invalid context/destinations. Record functional behavior and installed enforcement separately.

### M6 — Reviewed synchronization and recovery

- [ ] Port field-level comparison, baselines, conflicts, imports, tombstones, dependency ordering, and explicit inclusion/exclusion of plan operations.
- [ ] Prove stat/linked-achievement/text synchronization early with the S1 harness. Use approved pre-generated icons for transport/upload checks before the new renderer is complete.
- [ ] Bind approval to project revision, App ID, local data, icon sources, and remote baseline. Changed pins or remote values require reconciliation and a fresh review.
- [ ] Apply stats before dependent achievements, followed by localization and icon pairs. Save portal drafts and read back every affected record/content before committing verified baselines.
- [ ] Persist the sync journal before writes and advance it only with appropriate evidence. Distinguish applied, verified, failed, and uncertain operations.
- [ ] Preserve no-automatic-replay behavior after timeout, worker loss, cancellation during a write, or supervisor failure. Resume by reading remote state and rebuilding the plan.
- [ ] Port project-authoritative achievement deletion and rename recreation from current source, including replacement dependencies and verified absence. Preserve unsupported stat-deletion limits; show destructive operations for explicit inclusion. Publication remains manual.
- [ ] Preserve per-operation draft saving, the legacy creation-default fingerprint handling, and read-only verification retries. Avoid inventing a second global Save or blindly repeating a timed-out click.
- [ ] Exercise fixture failures and authorized live cases: stale approval, wrong App ID, changed page, partial snapshot, duplicate prevention, icon mismatch, interrupted save, and verification failure.

Exit: a reviewed plan converges on the test App ID, a repeated refresh shows the verified result, and interrupted execution reconciles without duplicate records or false success. Changes reaching Steam are never treated as a reversible local rollback.

### M7 — Rust MCP parity

- [ ] Implement a stdio MCP server using current-user named-pipe editor access, instance selection, and server/process identity checks. Keep stdout reserved for protocol traffic.
- [ ] Port the existing tools: instance/scene/node reads, icon export, achievement/stat creation, style listing/application, General images, stat configuration/linking, node updates/moves/selection, and undo/redo.
- [ ] Preserve project ID/revision envelopes, atomic batch edits, `stale_scene`/`editor_busy` behavior, request-ID deduplication, retry semantics, bounds, and timeouts.
- [ ] Route mutations through the same application commands as manual edits. Keep the documented limits on project open/save, deletion, individual layer mutation, and Steam writes unless a separate scope decision expands them.
- [ ] Run a real MCP client against the Rust app for the legacy ability-achievement workflow, including styles, imported images, linked stats, manual edits, and shared undo history.

Exit: existing client workflows work with documented configuration changes, identical retries do not duplicate edits, and invalid/stale batches leave the document unchanged.

### M8 — Packaging, enforcement, and release qualification

- [ ] Port package construction, inventory generation, verification, notices, protected versioned installation, repair, and uninstall for the final Rust process inventory.
- [ ] Verify signatures, rollback/version rules, file locks, effective ACLs, policy persistence, supervision loss, broker ownership, denial probes, and redacted reports on installed systems.
- [ ] Qualify clean-machine install, normal-user execution, reboot, sleep/resume, browser login/CDNs, policy loss, and interrupted-write recovery. Include unsupported/absent GPU and missing dependency behavior.
- [ ] Measure startup, memory, large canvases, shared-style refresh, full export, and Steam snapshot parsing against the recorded legacy baseline. Set release thresholds from those measurements.
- [ ] Review every native/browser dependency, license, advisory, and unsafe boundary. Archive source revision, lockfiles, toolchain, inventory, hashes, and acceptance results.
- [ ] Resolve license, publisher identity, trusted release keys, official channel, vulnerability-reporting route, and expected-publisher signature verification before enabling official mode. Keep update activation disabled until its trust and rollback workflow is qualified.
- [ ] Complete keyboard, accessibility, DPI, language, theme, error recovery, and user documentation checks. Obtain independent review before making public security enforcement guarantees.

Exit: a release candidate can install and perform the complete workflow on qualified Windows systems with documented limits and evidence. Development signing and passing unit tests are not substitutes for release identity or installed acceptance.

### M9 — Migration rehearsal and cutover

- [ ] Document backup, installation, settings/library compatibility, schema behavior, profile handling, recovery, and return-to-legacy instructions.
- [ ] Rehearse on copies of representative projects and assets. Run legacy-to-Rust-to-legacy round trips while schema compatibility is promised; compare semantic data, exports, and synchronization plans.
- [ ] Keep caches and transient sessions separated; never allow both editors to write the same project concurrently. Reconcile any existing uncertain sync journal before new writes.
- [ ] Complete the parity checklist or explicitly document accepted omissions. Remove the temporary C# worker after JavaScript driver parity; retain the documented Node/Playwright/browser dependency boundary.
- [ ] Confirm the installed production app does not depend on the legacy checkout or .NET application. Keep the old repository available for reference until the migration is accepted.
- [ ] Update the README with actual build/run instructions, supported platforms, migration guidance, and known limitations. Tag a verified release only after M8 prerequisites pass.

Exit: Achimeld can replace the old app for the agreed workflows, existing data is preserved, and users have a tested recovery path. Returning to the old editor does not undo remote Steam changes; those require a new pull and reviewed reconciliation.

## Validation strategy

Translate tests by responsibility, keeping behavior assertions rather than WPF implementation details. Legacy tests and sample outputs provide an oracle; this roadmap does not assert that the old suite was rerun during drafting.

| Boundary | Required evidence |
| --- | --- |
| Domain | Validation, IDs, decimals, canonical vectors, dependency rules, three-way plans |
| Storage | Legacy/new identifiers, schemas, CSV hashes, corruption, fault injection, asset ownership, round trips |
| Editor | Shared commands/history, no-op edits, dirty-state rules, selection/camera/layout, preferences cancellation |
| Imaging | Golden outputs, metadata, alpha/color behavior, effects, preview ordering/cancellation, export pairs |
| Browser and sync | Sanitized fixtures, page/context rejection, stale plans, complete reads, uncertain writes, verified convergence |
| MCP | Real client protocol, pipe access/identity, pins, deduplication, atomic edits, manual/agent concurrency |
| Installed security | Real policy/denial probes, complete supervised process tree, tamper, crash, reboot, repair/uninstall |
| Release | Clean-system workflow, performance budgets, native dependencies, signatures, notices, migration rehearsal |

Normal CI uses synthetic or sanitized fixtures and needs no Steam credentials. Live tests are explicit, visible, and serial against an authorized test App ID. Record failures and skipped qualification; do not count skipped live cases as release evidence.

## Main risks and decisions

| Risk | Response and decision point |
| --- | --- |
| UI framework cannot support the established canvas/layers/accessibility behavior | Prove representative interactions in M0 before committing to the shell |
| PSD/PSB, fonts, or image effects differ across libraries | Select the backend using a fixture corpus; document acceptable output differences before M4 acceptance |
| Rust numeric/string serialization changes synchronization fingerprints | Preserve explicit legacy encoding and golden vectors in M2; version any intentional future change |
| JavaScript driver changes working browser behavior | Reuse the existing worker first, preserve Playwright, then prove recipe/response/transport parity before switching |
| Existing installed-worker security defect is copied into the new app | Track the legacy acceptance failure independently and require installed evidence in M5/M8 |
| Partial migration drops unsupported project fields or artwork | Preserve or reject explicitly; use copy-based rehearsal and crash/round-trip tests |
| Portal changes invalidate reads/writes | Keep selectors/parsers isolated, refresh sanitized fixtures, and stop on unknown page shapes |
| Temporary bridge becomes a permanent hidden dependency | Record its owner, removal gate, and inventory; require removal or an explicit accepted scope change before cutover |
| Scope expands before parity is usable | Finish the offline editor and verified draft synchronization first; track enhancements separately |

## First implementation backlog

1. Freeze the working Steam build and record its exact read/stat/achievement/text recipes, protocol fixtures, and outcomes (S0).
2. Scaffold the Rust workspace and adapt the bootstrapper plus typed IPC so a small Rust harness can call the existing worker (S1).
3. Port snapshots, canonical values, planner/mapper, pins, journal, and guarded writer; prove read/import and one stat/linked-achievement/text draft flow before the full UI.
4. Complete UI/image spikes and compatible transactional storage while the known-working browser worker remains available.
5. Translate Playwright recipes to the JavaScript driver; compare and prove them under Rust supervision, then remove the C# helper (S2/S3).
6. Deliver the editor workflow: open, edit, undo/redo, render, save/reopen, synchronize, and verify; complete icons/deletion/rename parity.
7. Qualify the final installed process tree, diagnose enforcement gaps, and apply measured improvements only after the working behavior is preserved (S4).

Defer cross-platform releases, arbitrary browser attachment, automated publication, new translation services, team collaboration, and a paid launcher until the Rust parity release is qualified. Achievement deletion/rename are existing source behaviors to port with explicit destructive-operation review, not features to silently drop.
