# Local editor automation with MCP

Achimeld's MCP connection lets a compatible client inspect and edit an already open editor. It shares local project undo history. Save in the desktop to keep project changes.

## Connect

Open Achimeld normally, then use Help > Connect MCP for the executable path and configuration. The command is the verified Achimeld.exe with argument --mcp, running under the same Windows account as the desktop.

The connector does not open the desktop. Validation and synchronization use the editor's supervised Steam browser workflow. Reconnect after reopening the editor. It exits shortly after no editor remains, so a stale connector may need restarting.

## Read before editing

1. Call list_instances and choose its processId.
2. Call get_scene for projectId, revision, nodes and selection.
3. Use get_node, style-listing or translation reads for the details needed.
4. Supply projectId, expectedRevision and a fresh GUID requestId for an edit.
5. Use the returned revision, or reread after manual changes.

The status includes uptime, project, canvas-node count, Steam-sync achievement count and stat count. Local style/art cards and Retired entries can make those counts differ.

## Available work

| Task | Tools |
| --- | --- |
| Inspect | list_instances, status, get_scene, get_node |
| Create records | create_achievement, create_stat, create_stats |
| Edit/organize | update_node, move_nodes, select_node |
| Stat configuration | configure_stat, link_achievement_stat |
| Artwork/styles | add_general_image, list_icon_styles, apply_icon_style |
| Translation | get/export/import_achievement_translations |
| Icon output | export_achievement_icons |
| History | undo, redo |
| Style collections | list_style_collections, get_icon_style, create_style_collection, create_icon_style, export_style_preview, export_style_collection, import_icon_style |
| Steam review | get_synchronization, validate_steam, prepare_synchronization, apply_synchronization, cancel_synchronization |
| Title menus | open_menu, close_menu |

create_stats accepts 1-1000 names/objects or supported delimited text, validates the whole batch and adds it in one undo step. move_nodes accepts up to 1000 unique node placements.

update_node preserves omitted fields. configure_stat instead replaces configuration: omitted numbers become null, flags false, and Set by defaults to GameClient. For linking, provide both statId/unlockValue or omit both to unlink.

add_general_image accepts exactly one existing image, atlas/tile or absolute local file. Required style input slots must be attached before applying a style.

## Stale busy or uncertain results

stale_scene means reread the scene. editor_busy can mean active dialogs, invalid inspector drafts, artwork work or synchronization.

Retry an identical successful request with exactly the same ID and arguments when appropriate. Reusing that ID with different arguments returns request_id_reused. Retry caching is bounded and ends on restart/reopening/eviction; inspect the scene before issuing a new creation after uncertain completion.

Undo/redo include manual history, not an isolated automation stack.

## Validate and synchronize

Save the project in the desktop, then call validate_steam. Poll get_synchronization until verified or an error appears. Call prepare_synchronization and poll until the review appears. Inspect its Steam App ID, rows and available actions. Apply by supplying the exact review token and one `{key,action}` choice for every row, including rows left unchanged. Poll again for completion or an error. cancel_synchronization cancels an active check or review. These calls use the same supervised Steam workflow as the Sync screen; uploads save to the Steamworks draft and publication is still manual. Live Steam acceptance remains pending.

open_menu accepts File, Edit, Tools, Windows or Help. close_menu dismisses the active popup. A nested menu is opened through its parent in the desktop.

## Styles and output

Create/preview portable styles in collections before explicitly importing them into a project. Collection creation/editing is separate from project Save; export a collection to persist it. Project import creates independent identities without automatically applying the style.

Style preview export writes one PNG, default 512 square and at most 1024 per axis, to an unused absolute local filename. A collection export uses an unused .achv destination with its companions.

Icon export differs from desktop export in formats, names and replacement behavior; see [Exports](exports.md). Translation import has the identity/template safeguards described in [Localization](localization.md).

## Desktop boundaries

The connector does not expose project open/save or remote deletion. File destinations must follow the selected tool's local-path rules; do not invent unsupported operations to bypass them.

Connected local clients can edit the open project. Connect only clients you intend to give that local editing access.

