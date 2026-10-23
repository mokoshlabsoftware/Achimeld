# Local editor MCP

Achimeld implements the original Achiever MCP workflow against its open Rust editor. The original checkout is a behavioral reference, never a build dependency.

Build a fresh verified package with `build-and-run.bat --offline --build-only`, then open Achimeld normally. Configure an MCP client with the absolute verified package path:

```json
{"mcpServers":{"achimeld":{"command":"C:\\Projects\\AchimeldBuild\\current\\package\\Achimeld.exe","args":["--mcp"]}}}
```

The MCP process verifies and retains locks on its signed package. It does not start the desktop or browser. Run both as the same normal Windows account. Direct startup of `achimeld-app.exe` still requires supervision. Protection applies to the separately launched editor; `--protected --mcp` is unsupported.

The tools are `list_instances`, `get_scene`, `get_node`, `list_icon_styles`, `get_achievement_translations`, `export_achievement_translations`, `import_achievement_translations`, `create_achievement`, `create_stat`, `update_node`, `move_nodes`, `select_node`, `configure_stat`, `link_achievement_stat`, `add_general_image`, `apply_icon_style`, `export_achievement_icons`, `undo`, and `redo`.

Style authoring additionally provides `list_style_collections`, `get_icon_style`,
`create_style_collection`, `create_icon_style`, `export_style_preview`,
`export_style_collection`, and `import_icon_style`. Follow the
[agent style-authoring guide](agent-style-authoring.md) for the complete workflow,
recipe examples and native 3D cup rendering. Collection creation/cloning stays
outside project history. Explicit import adds one independently identified project
style in one undo step. Collection edits participate in MCP revision pins and retry
caching; they are persisted by collection export, independently of project Save.
App defaults remain read-only opt-in collections in Style Viewer.

Style preview export uses the app's own renderer on a background snapshot and
writes one PNG, default 512 × 512, at most 1024 per dimension. Its destination must
be an unused absolute local-drive `.png` filename. Collection export requires an
unused `.achv` filename and uses the existing owned-artwork/catalog transaction.
Neither tool saves the open project. Both use normal mutation pins; style preview
export has the ordinary 20-second budget. Source projects are not needed at runtime.

1. Call `list_instances` and choose a `processId`.
2. Call `get_scene` to obtain `projectId`, `revision`, and `result`.
3. Supply that `projectId`, `expectedRevision`, and a fresh GUID `requestId` with each edit.
4. Use the returned revision next, or read again after manual edits. Save through the UI.

Stale state returns `stale_scene`. Active dialogs, inspector drafts, artwork operations or synchronization return `editor_busy`. An identical successful request can be retried with exactly the same arguments and ID. Changed arguments with a cached ID return `request_id_reused`. The last 256 successful responses are cached with a combined 16-MiB cap. Project reopening, restart or eviction ends that guarantee; inspect the scene before issuing new creations. Revisions are local to the editor session and include manual document/navigation changes. Undo/redo share manual history.

Creation accepts English text and integer grid `column`/`row`. Occupied creation positions resolve to the nearest unoccupied grid location; inspect the returned node or scene for the final coordinates. Stat types are `Int`, `Float`, and `AverageRate`. Each edit is one atomic undo step. `move_nodes` accepts 1–1000 unique `{nodeId,column,row}` entries. Node patches preserve omitted values and other languages; description/hidden apply only to achievements. `configure_stat` replaces numeric settings: omitted numbers become null, flags false, and `setBy` defaults to `GameClient`. Link with both `statId` and `unlockValue`, or omit both to unlink. Integer thresholds must be whole numbers within stat bounds; invalidating linked thresholds rejects the edit.

`add_general_image` requires exactly one `imageAssetId`, `atlasId` with optional `tileIndex`, or absolute local-drive `filePath`. Files are decoded off the UI thread and copied into the Art Data Bank. Identical owned bytes reuse a project asset. Returned `generalImageId` indices correspond to style layers' `generalImageIndex`. Styles include project entries and saved global templates. Applying a global style retains it in the project in the same undo step; layer/effect IDs are independently cloned and folder/mask links remapped. Required general images must already be attached.

Icon export accepts an existing absolute local-drive `destinationDirectory`, optional `format` (`Png`, `Jpeg`, `Bmp`, `Tiff`), and width/height 1–4096 (default 256). Color and fully grayscale `_gray` pairs use sanitized API names. JPEG flattens alpha onto black. Matching image files are replaced. Pair collisions and excessive result inventories reject before writes. Per-achievement failures are returned; interruption can leave earlier complete files. Background export uses a pinned snapshot without changing project settings/history.

For localization, call `get_achievement_translations` with `processId` to receive `result.csv`, a suggested filename, language metadata and row count. It returns two rows per achievement: display name and description, with stable `ID`, API/field `Context`, English source templates and the supported language columns. Keep ID, Context, English and column headers unchanged, and retain `{0}` placeholders in translations. To create a handoff file, call `export_achievement_translations` with an absolute local-drive `destinationFilePath` ending in `.csv` and the usual mutation pins. Export requires an unused filename, writes UTF-8 with a BOM, and rejects the reserved `.achimeld.localization.csv` suffix.

Return translated content through `import_achievement_translations`, supplying exactly one of `csv` or absolute local-drive `filePath`, plus the current project/revision/request identity. File operations reject UNC/device paths and reparse points. Inline CSV is limited to 2 MiB of UTF-8; use file export/import for larger catalogs (up to 64 MiB). Rows can be reordered or omitted. Blank cells preserve current translations. Unknown/duplicate/shared IDs, changed English/context, malformed CSV and broken placeholders reject the complete import. Preparation runs in the background and rechecks the editor session, revision, busy state and cancellation before one undoable edit. The result reports `changedCells`; save through the desktop to persist the project/catalog pair. The project-owned companion CSV should not be edited directly. See [translation boundary contracts](security/contracts.md#achievement-translation-handoff).

Stdout contains only newline-delimited JSON-RPC; diagnostics use stderr. Protocol versions 2024-11-05, 2025-03-26, 2025-06-18 and 2025-11-25 are supported. Lines/pipe frames are capped at 16 MiB and nesting at 64. The stdio queue holds eight messages and the editor queue one. Overload closes the connection. Operations have a 20-second response budget, including translation CSV work; icon exports have ten minutes. Cancellation notifications/stdin closure interrupt a waiting facade. Disconnect/expiry cancels queued work; edits already executing complete atomically. Decode/filter primitives are not preemptible. Inspect or retry identically after uncertain completion.

The local pipe name includes the current Windows account hash and editor PID. A first-instance, non-inheritable ACL allows that account and SYSTEM; remote clients are rejected. The facade verifies the kernel-reported server PID. One connection carries a request, response and client acknowledgment before server handle closure. Connected local clients are trusted to edit the open project. This is not protection against compromised same-user processes. The registry has no executable/URL dispatch, browser launch, project open/save, deletion or Steam writes. Steam changes stay in the existing reviewed UI workflow.

The implementation follows the official [stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle), and [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) contracts. Run `build/Audit.ps1 -Offline -Fixtures` and inspect its exact source evidence; see the [MCP audit](security/mcp-audit.md) for scope and limits.
