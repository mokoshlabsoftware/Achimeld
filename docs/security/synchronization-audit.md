# Reviewed Steam synchronization implementation audit

Reviewed the working tree based on `5a54640a435178ea9f6f835347feffd24cee8e1d`. The fresh `build/Audit.ps1 -Offline -Fixtures` evidence records the exact source digest; this commit alone does not identify the implementation. Achiever was used as a source reference. All application build inputs remain local to this repository.

Coverage includes the desktop review/actions, saved-project and artwork ownership, typed worker envelopes and receiving state, snapshot consistency, browser request scopes, fixed draft forms, read-back, icon verification, failure/cancellation and local commit. No account sign-in, live draft mutation, publication or installed enforcement qualification was performed by this review.

## Confirmed defects fixed

| Invariant | Trigger and consequence | Fix and reproduction |
| --- | --- | --- |
| SEC-SYNC-001 | A selection was assigned before dependency/numeric validation. A direct caller reusing the session object after an exception could reach a retained step. The production worker already terminated on errors. | [BeginAsync](../../bridge/Achimeld.Browser/SynchronizationSession.cs#L126) activates steps only after preflight. Fixtures reject a step after rejected selection and verify zero portal writes. |
| SEC-DATA-001 / SEC-SYNC-001 | The new managed synchronization reader did not inherit Rust's snapshot identity/ownership checks. Ambiguous names, orphan references or inconsistent fingerprints could enter a plan. | [SynchronizationSnapshotPolicy](../../bridge/Achimeld.Browser/SynchronizationSnapshotPolicy.cs#L8) validates every fresh snapshot. Fixtures cover case aliases and corrupted fingerprints. |
| SEC-SYNC-001 | An image URL could keep its identity while its contents changed between review and import. | [ImportAsync](../../bridge/Achimeld.Browser/SynchronizationSession.cs#L415) compares downloaded content with the reviewed raw hash before returning artwork. Changed bytes make the step terminal; local data is not committed. |
| SEC-SYNC-001 | Nested icon baselines did not match the persisted schema; substituting generated hashes for converted Steam bytes made a new session see false changes. | [Icon baselines](../../bridge/Achimeld.Browser/SynchronizationSession.cs#L400) use the established flat fields and raw remote hashes. Separate pixel verification accepts approved conversion; a fresh-session fixture remains in sync. |
| SEC-SYNC-001 / SEC-READ-002 | A draft scope allowed posts throughout navigation/editing and icon scope also allowed a definition save. | [ArmReviewedPost](../../bridge/Achimeld.Browser/SteamBrowserRequestPolicy.cs#L95) grants one fixed post immediately before the save/upload click. It is consumed once; icons cannot save definitions or navigate to stats. |
| SEC-SYNC-001 | Invalid later definitions or a depleted call budget could be discovered after earlier writes. Stat edits could invalidate unselected Steam achievements. | All selected text, numeric and progress settings, including affected remote thresholds, are checked before activation. Case aliases between local and remote names are refused. The [host](../../crates/achimeld-app/src/synchronization.rs#L215) reserves the remaining call budget, including finish/stop, before dispatch. |
| SEC-STORAGE-001 / SEC-SYNC-001 | Loading the saved snapshot into the editor's storage owner could change its asset origin after Save As, including when review was cancelled. | [synchronization_input](../../crates/achimeld-app/src/desktop.rs#L552) uses a separate storage owner. A filesystem regression verifies both saved and unsaved references still resolve to the same artwork. |
| SEC-DESKTOP-001 | The Steam view expected typed properties, while ordinary refresh preserved normalized attributes. Configuration fields disappeared after refresh. | The view reads both established shapes; rendering regressions cover remote type and numeric fields. |
| SEC-SYNC-001 / SEC-READ-002 | Raw Steam record/editor IDs entered selectors and read-back paths. A size check after an SDK response read did not bound buffering, and a headers-only HTTP timeout did not cover the body. | Canonical decimal record IDs and safe editor prefixes precede selector/path construction. Fixed browser fetches and cookie-free public icon downloads reject redirects and enforce streaming limits; the icon deadline covers the body. Pure stream/ID checks and pinned synthetic Chromium fixtures reproduce refusal. |
| SEC-SYNC-001 | The new-definition allowance could hide text/icon changes in the other entity namespace with the same API name. Progress links and text on remote-only imports were absent from field review. | Unrelated checks retain exact entity ownership. Review includes progress API names and imported translations; same-name mutation and review-field fixtures cover both. |
| SEC-STORAGE-001 / SEC-SYNC-001 | Adopting a saved result after Save As could change the asset base used by undo history. | The storage owner verifies the saved pair, retains the existing asset base and rebases the new result. A renamed-project regression checks artwork after adoption, undo, redo and another save. |
| SEC-STORAGE-001 / SEC-SYNC-001 | Project identity was checked at application time, after the background save. A malformed worker result could also introduce filesystem artwork references. | Before saving, the host pins identity/settings and existing asset records, accepts only appended imported image placeholders, and binds every new reference to bytes it retained. Negative result fixtures cover changed identity, settings, asset paths and unregistered imports. |

Additional review fixes keep import choices independent from icon uploads, retain advanced recipes without older-schema round trips, provide exact staged icon previews through one background decoder and an 8 MiB cache, paginate large reviews, and construct selections without a quadratic row search. Local result application remains one undoable, project/App-ID-pinned transaction. The host journals uncertain outcomes before dispatch and saves verified project/catalog/artwork through the existing recovery mechanism.

## Validation and limits

Reproductions are in `tools/browser-policy-tests/SynchronizationFixtures.cs`, `tools/read-policy-tests/Program.cs`, the application synchronization/history tests, the host Save As/cancellation tests and the UI synchronization regressions. The native preview uses synthetic data and no Steam session. Required reproducible evidence is produced by:

```powershell
./build/Audit.ps1 -Offline -Fixtures
```

Live login dependencies, current Steam form selectors and image conversion tolerance still require acceptance with the exact newly signed package and an authorized test App ID. The browser guard constrains routes and single post opportunities; it does not independently prove the semantics of a compromised Steam page's post body. Steam provides no atomic conditional save, so read-back cannot exclude all concurrent changes between checking and saving. Compromised worker/runtime containment, installed WFP, third-party advisories, publication and remote deletion retain the limitations in [known gaps](known-gaps.md).

## 2026-10-19 follow-up review

The follow-up covers selectable bounded diagnostics, persistent-browser startup,
process teardown, current project artwork metadata, import-first reviews,
achievement retirement, and the Steam editor creation/save/read-back sequence.
All application build inputs remain in this repository.

Confirmed fixes include the missing pinned achievement-count read, one-use
editor-creation POSTs separate from saves, canonical per-record metadata reads
in the matching reviewed scope, and static diagnostics from an owned worker's
exit status when its reply is interrupted. Missing query rights on a live
process still fail closed; a bounded teardown wait requires its owned handle
to become signaled. Current artwork selectors bypass the legacy definition
mapper without changing retained recipes. Retired cards and text are checked
before saving and again before applying editor history. Retired names also
remain excluded when Steam spells their API name with different case.

A unique Steam identity from matching verified history identifies a rename.
Importing the reviewed replacement preserves the original card as Retired,
creates a new local identity and retains the imported artwork as one image
layer. Unsynced means eligible for synchronization. Skipped renames retain their
identity history, and interrupted operations still require a fresh read and
review. No delete or publish route was added.

Source regressions cover nullable artwork fields, immutable retired records and
text, ambiguous or foreign rename history, repeated reviews, one-layer artwork,
forged selections, consumed creation/save permissions, pinned numeric metadata
routes, unknown diagnostic codes and real worker exit status over a Windows
pipe. Live validation and import of the eight existing stats in the authorized
test project passed in a user session. Exact draft export/rename acceptance and
full source/fixture results are recorded in the external per-run evidence;
source fixtures alone do not establish live form compatibility or installed
Windows enforcement. Publication, remote deletion, installed WFP and dependency
advisory qualification remain separate scopes.

Validation now imports remote-only stats and achievements, unique renamed
achievements, and Steam-changed stats whose local definition is unchanged before
presenting upload review. The host selects only declared Import actions and
rejects any returned upload step in that phase. It retains artwork, saves and
adopts each verified import, then prepares a fresh plan. Conflicts and existing
achievement recipe replacement retain explicit review. Up-to-date rows have no
per-entry action controls; the view reports imported entries as a count. A
bounded four-pass refresh prevents an endlessly changing remote configuration
from keeping the import loop alive. Live process image queries are retried
within the existing 500 ms bound; persistent denial still stops supervision.

The authorized draft test exposed a false icon conflict after successful import.
The host now records the accepted rendered PNG pair and the separately verified
Steam icon fingerprint after retaining the imported artwork. Later recipe edits
still change the local fingerprint. Export also honors the separately imported
locked image, rather than regenerating it from the achieved image. Regressions
cover that distinction and post-import recipe changes. The saved comparison
recognizes the persisted `steamAppId` field when displaying remote counts.
