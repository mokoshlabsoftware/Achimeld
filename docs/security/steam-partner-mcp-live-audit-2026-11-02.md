# Steam partner and MCP live audit, 2026-11-02

## Scope and evidence

This audit exercised the installed Achimeld 0.3.2.0 editor against the Steamworks draft for App ID 4873800. The saved local project started with 100 achievements and 32 stats. The test deleted only `Finding "Ancient Map"` (`ACHIEVEMENT_003_MAP`) after explicit user approval, then restored it by synchronization. Steamworks publication was not performed.

Live evidence is in `C:\Users\Public\Documents\Achimeld\Logs\last-runtime-log.txt` for the first editor session and `log.txt` for the reopened session. Offline source and fixture evidence is in `C:\Projects\AchimeldBuild\current\audit-4f558cbd14624f8c97c00a73dc9292fa\evidence.json` and `audit-952711ee5a7e4cd7947d219b449ff3a2\evidence.json`. Both runs captured a dirty, changing source tree; neither is a final-package qualification. The installed 0.3.2.0 editor did not contain the new MCP controls described below.

The audit used MCP for initial discovery, scene and health reads, validation, opening and cancelling Steam pages, and synchronization status. When the MCP transport closed after the editor exited, remaining live work used the desktop UI and direct saved-project/log inspection. No credentials, browser contents, or crash dumps were copied into this report.

## Live test record

| Test | Observed result |
| --- | --- |
| Discover editor and project through MCP | Editor PID 10304 was found; scene reported the saved project, 100 achievements, 32 stats, clean revision 7, and App ID 4873800. |
| Interrupt validation during startup/navigation | One validation stopped with `Steam read cancelled` after 18.3 seconds. Owned browser cleanup was initially pending, then recovered automatically. The editor remained responsive. |
| Revalidate repeatedly | Four validations completed, taking 22.1, 12.9, 8.1, and 7.9 seconds. The latter three occurred after project reopening or recovery steps. |
| Open and interrupt Steam pages | Both Achievements and Stats opened through MCP. Immediate cancellation and cancellation after navigation returned control; owned browser processes exited or were subsequently recovered by the cleanup gate. No local entity counts changed. |
| Delete one Steam achievement | The in-app reviewed deletion reported one deleted, zero already absent, and a verified Steam draft read-back at 08:08:42 UTC. The saved project then had 100 local achievements and a 99-achievement remote snapshot. |
| Restore through synchronization | The first review selected three steps: import remote-only stat `TESTSTAT`, upload `ACHIEVEMENT_003_MAP`, and upload its icon. The worker completed `finishSync` at 08:23:07 UTC. The workflow then opened a second review and waited. After cancelling that empty continuation and undoing the unwanted local stat import, a fresh Steam read confirmed the achievement had returned. |
| Forget and rebuild validation | After explicit approval, Forget validation cleared the saved remote snapshot while retaining 100 local achievements and 32 local stats. Validation completed in 7.9 seconds; Read Steam completed, and the project was saved at 08:33:30 UTC. |
| Saved end state | The project contains 100 local achievements, 32 local stats, a fresh remote snapshot of 100 achievements and 33 stats, and exactly one remote `ACHIEVEMENT_003_MAP` with Steam ID `37_5`. The recreated entry has `data-display-order=99`; its earlier observed order was 2. |

An earlier desktop Forget validation action at 08:20 UTC occurred during an input race while navigating a prompt. It was immediately undone and was not saved. The deliberately approved Forget and rebuild test is the 08:32-08:33 UTC sequence above.

## Findings

### F1. Verified deletion leaves the editor with a stale save identity

**Severity: high for recovery and data consistency.** Immediately after the worker saved the verified 99-entry deletion snapshot, the host logged `Project changed on disk; reload before saving` at 08:08:42 UTC. The editor remained dirty while the saved file contained the verified deletion result. The completion path in `crates/achimeld-app/src/desktop.rs` attempts another save through a `ProjectStore` whose expected file hash predates the worker's save in `crates/achimeld-app/src/synchronization.rs`. The exact live sequence supports this cause; no repaired build has been retested. Recovery required discarding the unsaved editor state and reopening the saved project. Do not treat the post-deletion editor's dirty state as the authoritative snapshot without a fresh disk read.

### F2. Closing the last editor ends the MCP transport

**Severity: medium for agent continuity.** The installed connector exited after the last editor closed; subsequent `list_instances` calls returned `Transport closed` even after Achimeld was relaunched. This is consistent with the deliberate package-lock lifetime policy in `crates/achimeld-mcp/src/lifetime.rs`, but the current client did not automatically start a new facade. The connection appeared stalled during recovery. The new source adds `reload_project` so a saved project can be reset without closing the editor, and makes the bootstrapper exit directly when the last editor is gone even if client output is blocked. A true process restart still requires a new MCP connection. The native blocked-output lifetime fixture passed after the source correction; a rebuilt installed client has not been tested.

### F3. Sync continuation can look stalled after verified draft writes

**Severity: medium for workflow clarity.** The first synchronization completed three draft steps, then the host prepared a second review because an import occurred. That review waited for approval with no selected change matching the restoration target. The UI appeared idle until the review was cancelled at 08:28:35 UTC. The cancellation log says the operation stopped while waiting for review approval, although the previous draft steps had completed. The saved file still held a 99-entry snapshot until a fresh Steam read and save. A client should distinguish a waiting review from a running write and read Steam again after cancellation of a continuation.

### F4. Browser stop acknowledgement sometimes failed while cleanup succeeded

**Severity: low to medium for latency and diagnostics.** Some cancelled or completed page/validation operations logged an unavailable IPC header during worker stop. The owned-process cleanup gate then recovered, and later validation/page operations succeeded. This audit did not show a lingering browser or changed local counts, but the stop acknowledgement is not reliable in every attempt.

## MCP coverage and desktop fallbacks

The installed MCP already had `list_recent_projects`, `open_recent_project`, and `get_synchronization`. The last returns validation, activity, errors, review rows, and token, but the installed tool could not filter the returned review. The MCP session could not delete a Steam entry, forget validation, open a project outside recent history, or restart the editor process.

| Desktop action used in this audit | Why MCP was insufficient at the time |
| --- | --- |
| Mark the target achievement, inspect the deletion prompt, and confirm Delete | No reviewed Steam deletion tool. |
| Discard the dirty editor state, close, launch the installed app, and reopen the saved project | MCP close could request the normal prompt, but the transport ended on exit and could not relaunch or reconnect. |
| Inspect and cancel the second synchronization review, undo the unwanted local stat import | The prior MCP transport was closed; review status could otherwise be read by `get_synchronization`. |
| Filter the review by target API name | The installed MCP returned the complete review only. |
| Forget validation and confirm its prompt | No MCP Forget command. |
| Read Steam, save the rebuilt snapshot, and use File/Open Recent | The new editor was outside the closed MCP connection; project Save is still UI-owned. |

Source working version 0.3.2.4 adds `open_project` for absolute local-drive `.achv` files without reparse points, `reload_project` with required `discardUnsaved: true`, `forget_steam_validation` as an undoable local edit, and optional kind/state/API-name filtering for `get_synchronization`. Filtered reads include `totalRows` and `returnedRows`; applying still requires choices for every row of the complete review. These commands are not in the installed build used for the live audit. MCP Steam deletion, project Save, and process relaunch/reconnection remain gaps.

## Source and fixture verification

The MCP unit suite passed 13 tests with 2 intentionally ignored. The focused editor automation suite passed 14 tests with 1 intentionally ignored, and the added saved-project open/reload regression passed. Strict lint for the affected MCP and app crates passed. The native MCP blocked-output shutdown fixture initially failed twice, then passed after the explicit process-exit correction. The first full offline audit passed contract, storage, version, installer, lint, Rust, receiving-policy, browser-policy, desktop-boundary, dependency, worker-build, Playwright-runtime, migration, and browser-request checks. It failed formatting on a pre-existing UI test change and failed the native blocked-output fixture. The second audit passed format, lint, bridge fixtures including blocked-output shutdown, and the remaining policy and request checks. Its Rust compile failed on two concurrent editor changes outside the MCP implementation, and its source-stability check identified five files changing during the run. A final full offline audit on a stable source tree remains required. Installed Windows enforcement, other accounts, forced termination during a Steam write, publication, and live qualification of the rebuilt 0.3.2.4 package were not tested.

## Recommended follow-up

1. Repair the deletion completion save identity, then repeat the reviewed delete, disk snapshot check, sync restore, and saved-state check on a rebuilt editor.
2. Add a token-pinned, explicit-target MCP Steam deletion review and apply flow after F1 is fixed. Keep the Steamworks publication step manual.
3. Make a completed first sync step and a pending continuation review distinguishable in status and UI, with a clear zero-selection exit.
4. Rebuild and qualify 0.3.2.4 live, rerun the full offline audit, and check its real-time Public Documents runtime log from process entry and after a controlled failure.

## Source remediation after the live audit

The subsequent source changes address F1 by adopting the worker's verified saved project as the editor store's current file identity. A regression recreates the stale-hash deletion handoff, retains a concurrent unsaved edit, and confirms a later Save succeeds. F3 now ends each reviewed synchronization after its first completed submission; further differences require a new explicit review. The worker replies to a normal Stop request before Chromium teardown, so slow browser closure does not consume the acknowledgement deadline; the host still owns and verifies process cleanup. Cancellation can still close the pipe before a Stop reply, and the owned cleanup path remains necessary. F2 remains a connector lifecycle limit for a real editor exit: reload_project avoids an exit for saved-project recovery, while a relaunched editor needs a fresh MCP client connection.

MCP source now includes Save for an existing project, a token-pinned Steam deletion review/apply pair, per-command Preferences permissions with all commands allowed initially, project open/reload/Forget validation, and filtered synchronization status. The deletion pair reuses the existing supervised Steam worker, verifies exact API names against the complete saved snapshot, and requires explicit confirmation. The Steamworks publication step remains manual. These are source and fixture findings; none of the new behaviors was present in the installed 0.3.2.0 build used for the live test. Rebuilt-package qualification, including a repeat of the deletion and restore cycle, remains separate from the historical observations above.

The stable source tree through working version 0.3.2.10 passed `build/Audit.ps1 -Offline -Fixtures` in `C:\Projects\AchimeldBuild\current\audit-505db5a3b8c84bfaa79e422c3fee4f22`. It passed the Rust workspace, strict lint, desktop dependency boundary, managed bridge, browser policy, migration and request fixtures, with source stability confirmed. Offline evidence does not replace a live Steamworks repeat on the rebuilt package.

## Fresh-profile MCP sign-in check

The verified development build with working version 0.3.3.3 exposed `delete_browser_profile` through a fresh local MCP facade. The command accepted an explicit `confirmDeletion: true` for the open editor and reported that deletion started. The dedicated `ChromeProfile` directory was then present and empty. The first editor session exited unexpectedly before its log recorded deletion completion; the bootstrapper reported a rejected supervision connection. No crash metadata or dump was captured for that exit, so its cause remains unqualified.

After restarting the same verified build, the profile was still empty. The saved App ID 4873800 project opened through MCP, and `validate_steam` reached `Waiting for Steam sign-in`. The user signed in manually in the dedicated browser. A later `get_synchronization` returned `verified: true`, `active: false`, no error, and `Steam connection validated`. No Steam draft change or publication was made. The Public Documents `log.txt` showed the exact launched version and live startup records. This demonstrates fresh-profile authentication on the second editor session, while the first-session exit requires separate diagnosis.
