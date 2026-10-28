# Steamworks implementation and next migration steps

The application supports supervised reads and reviewed draft synchronization. All application implementation is in this repository; no installed application libraries or external checkout files are build inputs.

## Audit the actual path

1. `crates/achimeld-bootstrapper/src/commands.rs` validates the read CLI App ID.
2. `crates/achimeld-app` and `crates/achimeld-browser` own the private worker transport and typed read commands.
3. `bridge/read-worker/ReadOnlyPolicy.cs` pins one App ID and rejects mutation, uploads, arbitrary downloads and evaluation.
4. `bridge/Achimeld.Security` authenticates and continuously pins the supervisor and runtime configuration.
5. `bridge/Achimeld.Browser/PlaywrightChromeDriver.cs` launches only the supervised packaged browser, authenticates visibly, and reads fixed configuration endpoints.
6. `bridge/Achimeld.Browser/LiveSteamworksConfiguration.cs` normalizes stats, achievements, localization and icon hashes; `PortalDocumentSanitizer.cs` and `RedactionService.cs` bound the transport disclosure.
7. Rust validates normalized origin, App ID, revision and structure before exporting a snapshot.

Driver requests use the attributed loopback broker. The broker permits approved Steam HTTPS hosts and public IPs. It does not inspect encrypted methods or payloads. Browser authentication remains manual. Redirects for configuration/icon reads are disabled. The application has no generic HTTP, upload or script-evaluation command.

## Build correspondence

`Build-ReadWorker.ps1` uses locked restore and local project references, publishes all managed application libraries, and records source/output hashes. `Setup-Browser.ps1` verifies the Chromium lock against the published Playwright browser manifest, verifies the archive digest, and extracts inside a fresh package. Package signing verifies both provenance inventories against archived source and output bytes.

`Test-Migration.ps1` checks evaluated C# compilation/project-reference paths and compares local oracle outputs with committed vectors. The oracle substitutes only the browser transport; actual parsing, canonical hashing and wire implementations are local.

## Reviewed draft synchronization

`SynchronizationSession.cs` retains the saved project, localization catalog, icon hashes and complete fresh remote snapshot. The Sync tab reviews per-entry upload/import choices and field differences; the host shows thumbnails decoded from the exact staged achieved/locked PNGs. Missing local artwork leaves imports and definition/text changes available. Stat operations precede dependent achievements; missing dependency selections are rejected before writes. Each selected step is consumed before effects, and changed pins require a fresh plan. Interrupted mutations are never replayed.

The locally adapted `LiveSteamworksDraftWriter.cs` uses fixed forms and endpoints. Generated uploads use byte payloads rather than caller file paths. Every field is read back; icon verification compares bounded decoded pixels to the approved PNGs while baselines retain raw Steam content hashes. Imports re-check downloaded bytes against reviewed hashes. The host writes a durable uncertain-outcome journal before dispatch and commits JSON/CSV/artwork using existing recoverable storage. All imported artwork belongs to the Art Data Bank, including first-layer achievement artwork. Newer source recipes and project fields survive the pure legacy schema mapping.

Publication and remote deletion remain unavailable. The Steam page can mark imported entries and list them before a sync, and Sync lists those marks beside planned creations, but neither view sends a deletion. Renaming creates a separately reviewed new definition and retains the previous remote entry. The source/fixture audit does not establish live login/resource compatibility, current form selectors, JPEG conversion acceptance or installed enforcement. Steam has no atomic compare-and-save contract; fresh checks and read-back cannot exclude every concurrent edit between them. Account-dependent acceptance must use the new exact signed package and an authorized test App ID.

## Achievement Sets compatibility review

Reviewed on 2026-10-16. Achievement Sets require a migration of Achimeld's configuration model and browser adapter. The [Style Viewer](upper-application-bar.md#steam-achievement-set-links) can save manually entered, app-scoped style-to-set associations in its own style-set files. The Steam reader, synchronization models, and draft writer do not yet support sets. No worker protocol or network permission changes are part of the local viewer links.

### Using the Steamworks feature

Valve describes Achievement Sets as a beta configured in Steamworks; existing achievement API names and game unlock calls remain valid. Create a set under Manage Achievements, choose its player or DLC restriction, optionally make it developer-only, then save. Assign achievements with In Set or Bulk Move. Publish the draft through Steamworks when ready. Existing achievements start in Base game. Empty sets are invisible. Sets track completion separately, and previously completed sets stay completed when achievements are added. [Valve achievement guide](https://partner.steamgames.com/doc/features/achievements/ach_guide).

Bulk Import/Export supplies definition, achievement localization and set localization CSV files. Definitions include `setid`, `spoiler`, `archived` and progress minimum/maximum fields. Create sets in the editor before referencing their IDs in CSV. [Valve bulk workflow](https://partner.steamgames.com/doc/features/achievements/ach_guide).

The public Web API reference still documents `GetSchemaForGame/v2`; its listed version change concerns XML game names. It does not specify a new grouped response contract or a set-management API. This is insufficient to infer the new private editor's JSON fields, endpoints or save payloads. Achimeld currently reads the partner portal, not this Web API. [Valve Web API reference](https://partner.steamgames.com/doc/webapi/ISteamUserStats#GetSchemaForGame).

### Current implementation gaps

| Boundary | Current source and consequence |
| --- | --- |
| Complete configuration | `bridge/Achimeld.Browser/PlaywrightChromeDriver.cs` reads fixed `fetchstats` and `fetchachievements` routes. `LiveSteamworksConfiguration.cs` expects one top-level `achievements` array, converts `hidden`, and omits set and archive metadata. A changed envelope may fail; extra metadata may instead be silently omitted. The live response shape has not been observed. |
| Snapshot and project | `bridge/Achimeld.Core/DomainModels.cs` models a flat `RemoteAchievement`; `bridge/Achimeld.Browser/ProjectFormat/SchemaDtos.cs` and `V1ProjectMapper.cs` have no set representation. `crates/achimeld-browser/src/snapshot.rs` preserves normalized article attributes, but normalization must retain the data first. |
| Conflict detection | `bridge/Achimeld.Core/CanonicalFingerprint.cs` and the Rust snapshot reader hash the older fields. Set membership, set restrictions, archive state and progress minimum are absent. Changes to those values cannot participate in current synchronization checks. |
| Draft writes | `LiveSteamworksDraftWriter.cs` targets `#achievementTable`, English New Achievement/Edit/Save inputs, legacy row IDs and the `saveachievement` route. It forces progress minimum to zero. These controls and whole-row saves require new-editor verification before compatibility can be claimed. Icon uploads also use legacy rows and forms. |
| Receiving authority | `SteamBrowserRequestPolicy.cs` allows fixed reads and one armed legacy save/upload inside a reviewed scope. It has no set-management scope. `SynchronizationSession.cs` retains reviewed values but cannot review or verify values missing from the model. |

These are confirmed source limitations. They do not establish that Valve removed the old endpoints, that a particular live save loses membership, or that the new editor uses the CSV column names in JSON.

### Proposed API migration

1. Capture redacted configuration responses and relevant editor controls from an authorized test app containing both base-game and custom-set achievements. Record exact read routes and response completeness, including archived and restricted entries. Keep credentials, cookies and session tokens out of fixtures. A CSV export can establish interchange fields, but cannot establish private request routes.
2. Add first-class achievement sets with a local identity separate from the app-scoped Steam set ID. Retain localized names, ordering, restrictions and lifecycle state. Add achievement membership, archive state and progress minimum; explicitly map the existing Hidden property to the Spoiler concept. Treat these as proposed internal names until the adapter contract is captured.
3. Carry these values through project persistence, remote snapshots, Rust/C# validation, canonical fingerprints, baselines, import and review. Distinguish an old project with no grouping information from an explicit request to move achievements into Base game. Reject unsupported restrictions or incomplete snapshots before planning writes.
4. Implement the documented CSV interchange as a separate local workflow, or adapt observed private editor requests within the existing supervised browser. Show membership and lifecycle changes in review, retain their approved values at the receiver, and verify them after saving. Add only exact, app-pinned routes required by that adapter. Publication and deletion remain separate unavailable operations.
5. Preserve legacy wire vectors and add migration fixtures covering flat projects, multiple sets, unknown or duplicate IDs, dangling membership, archive/restriction changes during review, nonzero progress minimum, text/icon-only updates preserving membership, and interrupted saves. Run `build/Audit.ps1 -Offline -Fixtures` after implementation, then qualify live behavior separately for the exact package.

### Verification status

This review compared the official documentation with the source paths above. No signed-in Steamworks tab was available to this chat; authenticated response shapes, new selectors and draft saves remain unverified. Runtime tests and the full source/fixture audit were not run because this change only records the investigation. A redacted response capture from the new editor is the prerequisite for changing the live adapter without guessing its contract.
