# Local editor automation with MCP

Achimeld's MCP connection lets an agent inspect and edit a supervised project, with or without an editor window. Save changes before ending the connection.

## Connect

Setup MCP, below Launch Achimeld on the installer finish screen, provides a copyable agent prompt and exact installed configuration. By default the agent launches the visible editor and connects using --mcp. Only on your explicit request, use --protected --mcp-headless for an installed windowless host, or --mcp-headless for a development package. Run as your normal Windows user. The connection owns its host and stops it on disconnect. Use save_project with an unused absolute local-drive .achv path for the first save, then omit path for later saves.

For visual editing, an agent can launch the verified Achimeld.exe with --protected for an installed package. Help > Connect MCP describes the separate --mcp desktop connector. Save and close the current session before launching the other mode, then reopen the project; unsaved state is not shared. Desktop and headless sessions cannot run together because they use the same supervised network port. The --mcp connector does not open the desktop. Validation and synchronization use the editor's supervised Steam browser workflow. Reconnect after reopening the editor. It exits shortly after no editor remains, so a stale connector may need restarting.

## Read before editing

1. Call list_instances and choose its processId.
2. Call get_scene for projectId, revision, nodes and selection.
3. Use get_node, style-listing or translation reads for the details needed.
4. Supply projectId, expectedRevision and a fresh GUID requestId for an edit.
5. Use the returned revision, or reread after manual changes.

The initialization handshake includes structured editor facts. Status includes uptime, working version, project, counts, validation and synchronization state, and any modal that blocks editing. `get_editor_health` remains available when the window is minimized. Local style/art cards and Retired entries can make counts differ.

## Available work

| Task | Tools |
| --- | --- |
| Inspect | list_instances, status, get_editor_health, get_scene, get_node, list_recent_projects |
| Create records | create_achievement, create_stat, create_stats |
| Batch image import | review_achievement_image_folder, import_achievement_images |
| Project settings | set_project_settings, save_project |
| Edit/organize | update_node, move_nodes, select_node |
| Stat configuration | configure_stat, link_achievement_stat |
| Artwork/styles | add_general_image, list_icon_styles, apply_icon_style, list_effect_presets, apply_effect_preset, close_effect_explorer |
| Translation | get/export/import_achievement_translations |
| Icon output | export_achievement_icons |
| History | undo, redo |
| Style collections | list_style_collections, get_icon_style, create_style_collection, create_icon_style, export_style_preview, export_style_collection, import_icon_style |
| Steam review | get_synchronization, get_remote_icon_metadata, validate_steam, prepare_synchronization, apply_synchronization, cancel_synchronization, forget_steam_validation, delete_browser_profile, review_steam_deletion, apply_steam_deletion |
| Title menus | open_menu, close_menu |
| Notices | acknowledge_modal |
| Editor lifecycle and supervised pages | open_recent_project, open_project, reload_project, save_project, close_editor, open_steam_page, cancel_steam_read |

set_project_settings changes the project name, Steam App ID or both in one undoable edit. A changed App ID clears the old validation and synchronization baseline. Save afterward. For a batch of images, review_achievement_image_folder lists supported files directly in one local-drive folder and returns a token. Import with that token and one fileName, name and description for every listed image. The folder and editor revision must still match. Achimeld copies the artwork into the Art Data Bank and creates all achievements in one undoable edit. Poll status and get_scene for completion. A batch is limited to 128 images and 128 MiB; subfolders are not traversed.

create_stats accepts 1-1000 names/objects or supported delimited text, validates the whole batch and adds it in one undo step. move_nodes accepts up to 1000 unique node placements.

update_node preserves omitted fields. configure_stat instead replaces configuration: omitted numbers become null, flags false, and Set by defaults to GameClient. For linking, provide both statId/unlockValue or omit both to unlink.

add_general_image accepts exactly one existing image, atlas/tile or absolute local file. Required style input slots must be attached before applying a style.

## Stale busy or uncertain results

stale_scene means reread the scene. editor_busy can mean active dialogs, invalid inspector drafts or artwork work. An active synchronization snapshot allows ordinary edits and Save.

Status and scene report the current informational notice with a `modalId`. `acknowledge_modal` accepts that exact ID for a single-action OK notice, including startup updates and errors. It never confirms Save, Discard or Delete prompts.

`close_editor` cancels an active synchronization and continues into the editor's usual Save/Discard/Cancel choice for unsaved local edits. An interrupted Steam draft needs a fresh read before another submission.

After the last editor closes, the MCP connector exits. Restart the connection after relaunching Achimeld. For a saved-project reset that keeps MCP connected, use reload_project with discardUnsaved true.

Retry an identical successful request with exactly the same ID and arguments when appropriate. Reusing that ID with different arguments returns request_id_reused. Retry caching is bounded and ends on restart/reopening/eviction; inspect the scene before issuing a new creation after uncertain completion.

Undo/redo include manual history, not an isolated automation stack.

## Validate and synchronize

Save the project in the desktop or with save_project, then call validate_steam. Poll get_synchronization until verified or an error appears. Call prepare_synchronization and poll until the review appears. Inspect its Steam App ID, rows and available actions. The review captures artwork and project fields before it appears; later edits remain for the next sync. Apply by supplying the exact review token and one `{key,action}` choice for every row, including rows left unchanged. Poll again for completion or an error. cancel_synchronization cancels an active check or review. These calls use the same supervised Steam workflow as the Sync screen; uploads save to the Steamworks draft and publication is still manual.

The desktop shows a square review modal with a five-second countdown and Stop control. Status reports this as a review modal that cannot be acknowledged with acknowledge_modal. MCP apply_synchronization remains an explicit call with all row choices; the desktop countdown does not submit on behalf of an MCP client.

During a prepared review, get_remote_icon_metadata returns one named achievement's validated public Steam image links and content hashes. Use it to compare the current draft art with the frozen project art after an uncertain upload. The links are public image references, not access to Steamworks pages.

The general get_synchronization response keeps the review rows and token compact. Optional kind, state and case-insensitive API-name query arguments filter the returned rows; totalRows and returnedRows show the complete and filtered counts. Applying the review still requires choices for every row in the complete review. Ask for one achievement's remote image pair only when needed. forget_steam_validation clears the saved Steam snapshot and downloaded game banner as an undoable local edit, leaving authored entries and sync baselines intact; save_project persists it.

delete_browser_profile requires confirmDeletion=true and removes Achimeld's dedicated Chromium profile, including its saved Steam sign-in. It leaves the Steam account and project intact. The command starts the supervised deletion; poll get_synchronization.browserProfileDeletionActive and status for completion. Finish active Steam work before deleting the profile, then validate again to sign in.

For a deliberate draft deletion, call review_steam_deletion with exact Achievement or Stat API names. It requires a verified, complete Steam read and returns the matched rows and a token bound to the editor session and snapshot. Inspect those rows before calling apply_steam_deletion with the same targets, token and confirmDeletion true. The supervised worker deletes only those entries, reads Steam again and saves the verified project. Steamworks publication remains manual. A changed snapshot invalidates the review token.

Synchronization performs one reviewed submission at a time. If an import exposes further differences, prepare a new review explicitly after completion.

open_menu accepts File, Edit, Tools, Windows or Help. close_menu dismisses the active popup. A nested menu is opened through its parent in the desktop.

## Styles and output

Create/preview portable styles in collections before explicitly importing them into a project. Collection creation/editing is separate from project Save; export a collection to persist it. Project import creates independent identities without automatically applying the style.

Style preview export writes one PNG, default 512 square and at most 1024 per axis, to an unused absolute local filename. A collection export uses an unused .achv destination with its companions.

Icon export differs from desktop export in formats, names and replacement behavior; see [Exports](exports.md). Translation import has the identity/template safeguards described in [Localization](localization.md).

## Desktop boundaries

The connector can list the editor's recent projects, open one of those paths, or open another absolute local-drive .achv project without reparse points when the current document is clean. reload_project reopens the current saved path and explicitly discards unsaved changes when discardUnsaved is true; it resets the project session without closing the editor. save_project writes the current existing project path, or accepts an unused absolute local-drive .achv path for the first save. Save As remains in the desktop. File destinations must follow the selected tool's local-path rules.

Preferences > MCP lists every command in themed folders. Each is allowed by default. A folder switch enables or disables every command in that folder, including commands hidden by the current filter; individual switches remain available. The red Sensitive folder contains browser profile deletion, forgetting Steam validation, and the reviewed Steam deletion commands. Allow all resets the list. Save Preferences to apply changes. The editor enforces this policy for reads, writes, discovery and health. A blocked command returns a permission error. Connected local clients can edit the open project. Connect only clients you intend to give that local editing access.

