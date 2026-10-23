# Editor MCP implementation audit

Scope: the original Achiever MCP workflow translated into the Rust editor, signed launcher's `--mcp` mode, Windows transport, UI-thread dispatch, owned artwork and background export. This is a local source/fixture review, not independent attestation or installed/live acceptance.

The collection-authoring extension uses the same closed registry and pins.
`collection_edits_are_revision_pinned_and_import_is_one_undo_step` checks collection
isolation, stale and repeated requests, read-only templates and atomic project
import. `style_tools_bound_patches_and_keep_file_and_project_authority_explicit`
checks recipe/patch schemas and preview dimensions. The explicit developer fixture
`author_default_styles_through_mcp` creates eight variants and a native Artifact3d
cup through actual JSON-RPC/Windows pipe calls, renders the cup PNG, exports and
reopens a collection, and asserts the canvas/history did not change. It is ignored
in the routine suite because it creates curated authoring outputs; its external
transcript digest is recorded in the embedded assets' `authoring.json`.

| Finding | Fix and regression evidence |
| --- | --- |
| File import retries depended on the source remaining available. | Retry lookup precedes file access; artwork is owned and deduplicated. `file_artwork_is_owned_deduplicated_and_retries_need_no_source_file`. |
| Reopening the same project could retain previous retry records. | Each host editor session has a separate identity. `retry_staleness_busy_and_same_project_reopen_use_the_live_editor_session`. |
| Early client disconnect could stall acceptance; closing immediately after a response could discard unread bytes. | Recreate disconnected instances and retain server handles until acknowledgment/timeout. `real_windows_pipe_serves_the_stdio_facade_and_survives_disconnection`. |
| Synchronous dispatch could not receive cancellation while awaiting the editor. | Bounded input thread receives cancellations; disconnected/expired queued work cannot start editing. `cancellation_is_received_while_an_editor_call_is_in_progress`, `cancelled_and_expired_queued_requests_never_mutate_the_document`. |
| Style validation checked the wrong general-image index field. | Check the renderer's actual `generalImageIndex` before committing. `style_checks_the_actual_general_image_index_before_committing`. |
| Global templates were absent from the initial project-only style list. | Merge project/global libraries and retain applied templates in the same document transaction. |
| Export could report/cache against a newly opened project. | Pin the originating envelope and cache only within that editor session. |
| Selecting a different canvas could return a revision before its saved camera was restored, making the next request unexpectedly stale. | Restore the destination camera before observing the response revision. `selection_returns_the_destination_camera_and_global_style_import_is_one_undo_step`. |
| Export path, filename and response limits needed explicit handling. | Restrict local-drive paths, reject reparse points, sanitize names, preflight pair collisions/result budget, publish complete temporary files. `icon_exports_have_requested_dimensions_grayscale_formats_and_failures`. |
| Generic arguments could acquire unintended authority. | Closed registry/schema is revalidated by the receiver. `schema_rejects_extra_authority_malformed_batches_and_invalid_enums` and existing unsupervised-startup negatives. |
| Translation CSV access needed the same source, ownership, history and concurrency checks as the desktop. | Shared bounded CSV routines and import planner; unchanged IDs/context/English and placeholders; shared-row rejection; local-drive CSV paths; no overwrite; post-preparation session/revision/cancellation checks. `prepared_translation_import_rechecks_late_edits_and_cancellation`, `translation_tools_require_one_bounded_input_and_mutation_pins`, and the translation round trip in `real_windows_pipe_serves_the_stdio_facade_and_survives_disconnection`. |

Application tests cover atomic creation/history, localization preservation, invalid batches, stat/link ranges and independent style graph identities. The Windows fixture runs the stdio facade through a real pipe into the UI-thread dispatcher, retries a creation and inspects its node. Export fixtures inspect PNG/JPEG/BMP/TIFF dimensions and grayscale pixels. Wire compatibility vectors and the six-executable package allowlist remain intact; no external application assembly/source or extra MCP executable is a build input.

`build/Audit.ps1 -Offline -Fixtures` records exact source identity, per-check logs, outcomes and skipped scopes outside source. Its report establishes which checks passed for a given tree. Protected installation/WFP, different-account ACL enforcement, external MCP client UI acceptance, live Steam, sustained fuzzing, CPU/RSS and hostile same-user filesystem races remain separate scopes. The finite retry cache is not an exactly-once guarantee across restart/eviction. Explicit exports may overwrite matching images and interruption may leave earlier complete files. MCP exposes no remote writes or project save/open.
