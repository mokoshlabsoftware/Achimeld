# Synchronization review and recovery

Sync connects a saved local project with a Steamworks destination. Validate checks sign-in and access. Synchronize prepares fresh information, artwork and a review.

Uploads save to a Steamworks draft. Publishing remains a manual Steamworks step.

![Previously validated saved snapshot](screenshots/0.2.29.0/05-sync-saved-state.jpg)

## Destination and connection

Check the project name and Steam App ID. Edit Steam App ID changes the intended association. Save the project before validating; an unsaved project receives guidance to save first.

Open achievements and Open stats beside the validation or synchronization action open the selected project's pages in the dedicated Steam browser. Close that browser window when finished. These shortcuts do not validate the connection or change the project.

A banner marked Validated in a previous session and a saved timestamp describe cached information. Re-validate to confirm the current connection before synchronizing. Matching counts alone do not establish matching fields or icons.

Forget validation clears saved validation/snapshot/banner data after its confirmation while retaining authored entries and used artwork. Save persists that local edit.

Open profile folder and Delete browser profile manage the dedicated Steam browser profile. Deleting it removes saved sign-in, not your Steam account or project.

## Review differences

The review identifies the validated App ID and lists every proposed definition or icon change before Steamworks is updated. Artwork-only edits appear as Icon changes. Filter by API name; large reviews paginate 50 entries at a time.

| State | What it asks you to inspect |
| --- | --- |
| Only in project | A local definition absent from the read |
| Only on Steam | A remote entry needing an import decision |
| Project changed | Local changes relative to the baseline |
| Steam changed | Remote changes relative to the baseline |
| Review both versions | Conflicting changes |
| Up to date | No current difference; omitted from the differences list |
| Artwork unavailable | Repair its source before an icon upload |
| Renamed on Steam | Import preserves the old entry as Retired |

Use project, Use Steam and Leave unchanged appear according to the entry and review state. Choose Leave unchanged to unselect a proposed upload or import; you can select it again before synchronizing. These choices do not delete remote entries. Do not assume every row offers all choices.

Expand field differences and inspect the actual values. An import choice can change the associated icon choice so incompatible import/upload combinations are not retained.

## Apply selected changes

Check the destination, field choices and selected-change count. Use Synchronize selected changes when the review enables it. Confirmation and revision checks bind the operation to the reviewed project and destination.

An MCP client can perform the same supervised workflow for a saved project. It validates first, prepares a fresh review, then supplies the review token and an explicit action for every row. The client can poll the current review, activity and errors, or cancel active work. See [Local editor automation](automation.md) for the tool sequence. Steamworks publication remains manual.

Icon previews and rendering are part of this workflow where required; a cached icon picture alone does not prove the upload pair is ready.

Progress shows stage and elapsed time. Cancel uses the active operation's cancellation path. A movable progress panel can leave artwork accessible, while review and save/project-switching guards retain destination identity.

## Interrupted work

An interruption can leave uncertain remote draft results. Reconcile using a fresh Steam read and review the remaining differences. Do not blindly repeat the previous plan.

Local edits remain dirty when appropriate after a merge. Conflicting changes preserve local work and require a fresh review. Read completion/error guidance and use [Troubleshooting](troubleshooting.md) for diagnostics.

Fresh live validation, draft writes and interruption recovery still require controlled acceptance before these draft instructions can be marked fully runtime verified.

