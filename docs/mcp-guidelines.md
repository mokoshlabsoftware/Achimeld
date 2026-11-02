# Achimeld MCP guidelines

This document defines the editor MCP behavior expected by agents and local clients. The supervised desktop owns the project, its undo history, files, and Steam browser. MCP requests use the same host operations and security checks as the desktop.

## Availability and lifecycle

- MCP must read and edit an open editor while its window is minimized. A minimized window is a normal running state, not a reason to time out, restore the window, or change focus.
- Requests must be dispatched independently of rendering frames. Mutations still execute serially against the editor owner, with the same project, revision, busy-state, cancellation, and undo checks.
- The MCP facade must reconnect after a development package is replaced or an editor restarts. A reconnect identifies the new process and session before sending a mutation. A stale process ID or revision is rejected, never silently retargeted.
- Discovery must tolerate the brief named-pipe replacement after a client disconnects. A handshake during that gap should retry before reporting that no editor is open.
- Normal close and restart actions must preserve Save, Discard, and Cancel behavior. A close request reports whether it closed, is awaiting a prompt, or was refused. Restart must not bypass an unsaved-work prompt or launch an arbitrary executable.
- A health probe must work even if the editor renderer is stalled. It reports listener availability, editor-owner responsiveness, last completed dispatch, and the current process and session identity without requiring a project read.

## Rich initialization handshake

The initialization response should provide a bounded, structured snapshot for each reachable editor:

| Field | Purpose |
| --- | --- |
| Protocol and capability versions | Identify exact tools, limits, and optional features. |
| Compiled four-part working version and package/source identity | Distinguish the build being controlled from the development checkout. |
| Process ID, editor session ID, and uptime | Detect restart and stale connections. |
| Project ID, display name, path, revision, and dirty state | Pin the intended project before edits. Return only the project path already visible to that local user. |
| Achievement, Steam-sync, and stat counts | Let the client confirm it selected the intended editor. |
| Current canvas, active host operation, and busy reason | Explain when a request can proceed or should be retried. |
| Steam App ID, validation state, review state, and saved snapshot age | Distinguish current live evidence from retained Steam data. |
| Runtime log availability and location | Point the client to the current local diagnostic source. |

The handshake is a snapshot, not authorization to skip a fresh `get_scene` before a mutation. It must have a response size limit and omit cookies, credentials, tokens, browser contents, raw requests and responses, arbitrary worker exceptions, and sensitive dump contents. If an editor cannot answer, initialization should report its health and a specific failure instead of silently omitting its project data.

## Request and error contract

- Every mutation is pinned to process ID, project ID, expected revision, and a fresh request ID. An uncertain retry uses the same request ID and identical arguments. A restart or project switch invalidates that pin.
- Return distinct errors for listener unavailable, editor owner paused, editor busy, stale scene, validation failure, transport disconnect, and uncertain completion. Include a safe next action in each error.
- A timed-out request must say whether execution began. If that cannot be proven, mark the outcome uncertain and require a read before retrying. Do not replay a possible Steam write automatically.
- Steam page access remains limited to the selected App ID and approved achievements or stats routes. Draft changes require a fresh validated review, explicit row choices, and read-back. MCP must not expose arbitrary URLs, worker commands, filesystem paths, or unsupervised browser launch.
- Artwork diagnostics may return one selected achievement's validated public Steam CDN preview URLs and content hashes from the current frozen review. Reject absent reviews, mismatched App IDs, unknown achievements, and URLs outside the fixed public image path. Do not return authenticated portal URLs or browser state.
- Emit bounded, safe host diagnostics for connection, dispatch, cancellation, and failure transitions. Do not log project text, artwork bytes, portal content, or credentials.
- `status` and `get_scene` must report the current modal's kind, safe message, and whether it can be acknowledged. Acknowledge by exact modal ID only for a single-action informational notice. Never turn Save, Discard, Delete, or other multi-choice prompts into a generic OK action.
- Named effect presets must be discoverable by stable ID and complete recipe. Applying a preset to selected achievements must use the exact saved recipe, preserve unrelated layers and artwork, and create one undo entry. Preset thumbnails must be tied to the recipe generation that produced their pixels.
- Synchronization captures rendered icon bytes and project fields before review. The plan states that it is a frozen snapshot. Editing and ordinary Save remain available while Steam runs; later edits belong to the next synchronization. Completion merges independent edits and never overwrites the editor after a conflict.

## Acceptance checks

1. Minimize an editor, then call health, scene, a reversible edit, undo, and close. None should require restoring or focusing the window.
2. Replace a development package while its MCP facade is idle. Reconnect to the new build and reject stale process/session/revision pins.
3. Start validation, cancel it, open achievements and stats in turn, cancel each read, and validate again. Check the live runtime log and ensure browser cleanup does not strand the next operation.
4. Change only an achievement name, then only its artwork. Each fresh review must show the exact changed fields; unchanged rows must stay in sync. Submit a selected test draft change, read it back, and verify a later review is clean.
5. Exercise lost replies, disconnects, minimized windows, and editor shutdown. Verify precise error categories and that uncertain Steam operations are never replayed.
6. With a review open, edit and save an icon, then submit the frozen icon pair. Verify the uploaded bytes match the reviewed snapshot and the later edit survives for the next review.
7. Open an informational error and a startup update. Verify status reports each notice and exact-ID acknowledgment works; destructive prompts remain separate.

Keep the implementation evidence and remaining live/installed qualifications in [MCP audit](security/mcp-audit.md) and [synchronization audit](security/synchronization-audit.md). This document states the intended contract; it does not claim every acceptance check has passed.
