# Phase 1 inventory and findings

The initial discovery baseline is application source version 0.2.29.3, inspected on 2026-10-30. Runtime observations and screenshots use the user's open example project in compiled version 0.2.29.0.

## Source coverage

[source-inventory.json](source-inventory.json) indexes 176 files, 301 type declarations, 23 icon layer kinds, 11 effect kinds and 264 change-log entries. Its 8,915 lexical string candidates include controls, tooltip text, option values, property keys and internal identifiers.

Each indexed source and candidate has article destinations and a follow-up verification task. History entries have suggested article destinations and pending human review; these suggestions do not advance the reviewed-through history version.

The register records a SHA-256 for each input and an aggregate snapshot identity. It covers the listed feature sources and top-level guides, not every repository file or all executable code paths.

## Main view map

| Surface | Article IDs | Next verification |
| --- | --- | --- |
| File/Edit/Tools/Windows/Help | projects, getting-started, exports, preferences, automation | Walk all submenus, dialogs and enabled/disabled states |
| Achievements and inspector | achievements-general, general-images, localization, icon-editor | Creation, direct/stat-linked modes, sync statuses, rename and hidden conditions |
| Stats | stats | Int/Float/AverageRate, optional numeric values, validation and linked achievements |
| Sync and Steam | sync, steam | Fresh connection, review states, conflicts, cancellation and controlled draft actions |
| Art Data Bank | art-data-bank | All tabs, import, atlas grids, references, resize, reload and unavailable sources |
| Style Viewer | styles | Actual selected preview, project/collection ownership, apply/import/save and Affect style |
| Layers and type properties | icon-editor, icon-layers, icon-effects, masks | Every picker entry, transform, effect, folder, mask and context action |
| Explorers, Preferences and Terminal | icon-layers, preferences, troubleshooting | Presets, import/export, all preference sections, logs and recovery |
| MCP | automation | Inspect available server operations against the running build and document limits |

## Runtime observations

The example has 21 canvas achievement records and 17 stats. Fit all reveals a separate style helper node; Sync shows 20 ordinary achievements in the saved project/Steam counts. Counts should be explained by item kind and synchronization scope rather than assumed identical.

The selected ULTRA_CLICK_100 achievement has a template name and description containing {0}, a value of 100, a General Image input at ID 0 and a linked UltraClickUsedTimes stat. Its Text layer uses the unlock-value source, while its General Image layer uses an 80 percent scale.

General settings, Localization, stat settings, Art Data Bank, a cached Sync state and the icon layer editor were inspected. Style Viewer banners were captured, but a complete selected-style preview was not established. Steam is disabled in the observed connection state. No fresh Steam validation, upload, import, publication, save/reopen or export acceptance was exercised.

The MCP discovery call initially failed with Transport closed. Computer Use later reached the running editor and supplied these scoped observations.

## Picker and model distinctions

The layer picker declares 13 ordinary source/procedural kinds and the adjustment picker declares seven kinds. The model has 23 kinds overall. Folder, MaskReference and SteamImportedArtwork require their own workflow investigation rather than being assumed ordinary layer-picker choices.

The Hidden control is conditional in General settings. A stat link changes numeric-kind behavior. Missing artwork, retired records, rename state, style helper nodes, art-image nodes and active Steam operations also require separate documentation states.

## Remaining phase 1 work

- Classify and deduplicate lexical candidates into individual control and workflow records.
- Check empty projects, alternate stat types, hidden/retired/renamed items and all context menus.
- Complete the screenshot checklist, including visual before/after examples.
- Reconcile all historical changes with reviewed impact decisions.
- Verify the appropriate compiled version for any behavior introduced after 0.2.29.0.
- Exercise the tutorial end to end in a disposable example project and retain save/reopen/export evidence.

Phase 1 remains in progress. The current inventory is sufficient to start core-guide drafts, while unresolved verification stays explicit.

