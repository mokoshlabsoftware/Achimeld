# Steam configuration view

Steam displays imported achievement and stat configuration. Its availability depends on the project/connection state. If disabled, inspect Sync's App ID and validation guidance first.

## Read configuration

Read Steam starts the supervised browser workflow. Sign in manually when prompted. The dedicated profile retains the session for later use.

A read displays its imported configuration and timestamp; Stop cancels the active read. Starting a new read clears deletion marks. Reading, profile operations and synchronization have guards to prevent overlapping work.

The Achievements and Stats groups display names and API identifiers. Filter by name or API name to inspect a subset. Distinguish this saved/read view from your editable local canvas.

## Marks and remote deletion

Selecting a Steam tile marks it for deletion and displays a circle indicator. Mark all marks every loaded entry of that kind, including entries hidden by the filter. Clear marks removes that kind's marks.

When entries are marked, Delete on Steam opens a scrollable review identifying the achievements/stats and destination. Closing the review retains marks. Review the complete list rather than relying only on the visible filtered tiles.

Remote deletion is separate from deleting a local card or setting its status to Retired. It is an external operation and cannot be treated as ordinary local undo.

## Rename identity

For an imported entry, local API-name edits can retain a Rename from origin in the inspector. Follow this identity-aware workflow and review the resulting synchronization decisions.

A remote rename discovered during synchronization can import a new identity while keeping the old project entry Retired. Display-name translation and API-name renaming are separate operations.

## Browser profile

Open profile folder locates Achimeld's dedicated profile. Delete browser profile asks for confirmation and removes that profile's saved sign-in without deleting the account or project. A running browser operation must finish before another can start.

If browser cleanup reports a failure, follow its recovery guidance rather than repeatedly starting reads.

Use [Sync](sync.md) for comparisons/imports/uploads, and complete draft publication manually in Steamworks.

