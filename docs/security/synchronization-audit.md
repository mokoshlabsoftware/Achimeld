# Reviewed Steam synchronization implementation audit

The host renders, hashes and retains each available synchronized achievement icon pair as bounded PNG bytes during preparation. The receiver compares those hashes with the saved baseline and fresh Steam icon identity, so an artwork-only edit appears as a separate Icon row. The review shows the frozen action plan; one explicit Synchronize click submits selected rows without a second confirmation. The worker uploads those captured bytes and verifies Steam read-back. A user can edit and save the project during the review and transfer; completion merges independent local and Steam changes. This source and fixture check does not establish live Steam compatibility.

## Live icon conversion correction (2026-11-01)

The reviewed Steam artwork snapshot renders its upload PNG pairs at 256 by 256 pixels, independent of the project's atlas export size. This matches the saved Steam achievement icon dimensions observed in the live waifus6 draft. The worker downloads and hashes each pre-upload Steam icon, then after one authorized POST retries the saved configuration and CDN download until a changed image hash appears. A full snapshot read must report that same hash for the selected achievement's achieved and unachieved images. It never repeats the POST. An unchanged hash is an uncertain stale outcome; a changed hash proves new saved bytes, not their visual meaning. Public preview inspection supplies visual QA when needed.

## Previous deferred icon flow and process cleanup (2026-10-29)

This earlier flow followed Synchronize with default uploads automatically and
paused for review only on conflicts or uncertain-outcome recovery. It removed
ordinary per-row choices. The current reviewed flow described above supersedes
that behavior.

SEC-SYNC-001: optional deferred icon candidates carry no PNG hashes at preparation.
They authorize only the receiver's selected next icon step. The host renders that
pair from the frozen document at execution; the receiver validates bounded PNGs
and pins their hashes before writing and read-back. Legacy prehashed requests
retain exact-byte checks. This changes when deferred bytes are pinned and does not
claim they were reviewed as images before execution. Preparation and imports no
longer render every icon. The locked variant derives exclusively from the current
composed achieved image; legacy locked asset references cannot override it.

SEC-PROCESS-001: cleanup previously waited only for the worker and ignored exit
results. It now terminates the owned Job, checks zero active members and waits on
retained member handles, logging success or a bounded static failure. Unconfirmed
cleanup retains owned handles for bounded automatic background retries; a new
start waits for recovery without a permanent restart latch. The process fixture covers an
already-exited worker with a still-live descendant and immediate reopening after
confirmed cleanup. Installed enforcement and live Steam remain separate scopes.

## Deleted history and failure diagnostics (2026-10-29)

SEC-SYNC-001: verified deletion previously saved a fresh snapshot but retained
baselines. A local name differing from the deleted historical name then became
an implicit rename of an absent identity, preventing review. The host now checks
the matching complete snapshot and exact confirmed batch before reconciling
baselines by entity namespace and name/identity. A fresh prepare also repairs
older historical records only when both names and identity are absent and there
is no explicit pending rename. Explicit intent, surviving identities, incomplete
reads and conflicting names retain refusal behavior. No automatic upload occurs.

The worker maps exact source-owned synchronization validation reasons to closed
error codes; the host logs static specific explanations through its existing
diagnostic sink. Unknown exception text stays private. Fixtures cover recovery
without writes, preserved explicit intent, deletion result validation and
namespace isolation, and safe known/unknown diagnostic classification. These are
source/fixture checks; live Steam draft qualification remains separate.

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

## Steam deletion review (2026-10-29)

Steam deletion uses the fixed `deleteSteam` worker command and a single-use
`SteamDeletionSession`. The host requires current editor-session validation,
matching saved project contents and no competing Steam operation. The in-app
confirmation lists the selected targets and uses the standard gray resting action
buttons with shared hover and keyboard-focus behavior.
Cancellation never approves the batch.

The receiver validates captured target identities and fingerprints from both Read Steam and synchronization snapshots, checks the pinned App ID, then
reads a complete Steam snapshot before each target. Confirmed absence is skipped.
Existing targets must retain their reviewed definition fingerprint and Steam
identity. Achievements precede stats; surviving progress dependencies stop stat
deletion. The browser rechecks the record identity and arms one fixed deletion
POST. Read-back must confirm absence and preserve unrelated data. Failures are
terminal; subsequent attempts reconcile against fresh Steam data instead of
replaying a failed request. A local uncertain-outcome journal remains after failure.

The supervised reader is dropped on every exit. The desktop completion path
requests application focus on success, error, cancellation or worker disconnect.
Successful runs refresh the saved remote snapshot without deleting authored
project entries. Failure invalidates session validation and asks for a refresh.
A successful draft deletion still requires manual Steamworks publication.

Exit cancels an active synchronization and drops its review channels before
the normal local Save/Discard/Cancel close flow. The Rust worker is detached at
this point so a stalled Steam call cannot hold the desktop window open. Its
frozen input cannot mutate the editor after detachment; a draft write already
in flight may have an uncertain outcome. The durable journal and a fresh
Steam read are required before any retry. Completion polling continues while
an application dialog is visible so prompts cannot stall normal cancellation.

Icon upload read-back retries fresh saved-configuration reads and CDN downloads
for a bounded interval until the exact selected icon's content hash differs from
its pre-upload hash. It never repeats the draft POST. The full verified remote
snapshot must contain the resulting hash before the step can complete. Closed
worker codes distinguish a rejected POST, missing saved image, invalid raster,
and unchanged remote hash. Older pixel-difference codes remain in the wire
vocabulary for compatibility but the current writer does no pixel comparison.
Arbitrary portal responses and image bytes do not enter diagnostics.

Fixture evidence covers mixed missing/present batches, dependency order, replay,
failed read-back, confirmation cancellation and validation gating. Live Steam
selectors, installed supervision, and exact-package runtime diagnostics require
separate qualification; these source tests do not establish those claims.

## Deletion route and shutdown recovery (2026-10-29)

The installed 0.2.26.14 runtime log recorded a terminal dynamic-request rejection
during deletion, followed by validation waiting on worker stop. Steam's public
`https://partner.steamgames.com/public/javascript/apps.js` deletion closures use
`/apps/deleteachievement/<app>/<stat>/<bit>` and `/apps/deletestat/<app>/<stat>`.
The adapter and policy previously expected only the app segment. Both now use
one canonical URI bound to the rechecked record identity, with a single-use POST
grant. Other records, apps, queries, navigation and replay remain denied.

Graceful stop now has a five-second host deadline instead of the general
five-minute deadline. The existing owned Job is closed on every stop outcome.
Cleanup failure is logged; completed operation results remain usable only after
cancellation and supervisor health checks. Failed operation results never enter
this success path. No protocol shape or launch recipe changes.

Source policy checks and synthetic Chromium routing cover both record routes and
replay. The Windows pipe fixture covers a stalled reply deadline and process
cleanup. Live account deletion, exact rebuilt desktop runtime logs and installed
acceptance remain separate qualification; these fixtures do not establish them.

## Host workflow diagnostics (2026-10-29)

Host workflow diagnostics now distinguish validation, synchronization and deletion.
Deletion records selected counts, verification/save/cleanup stages, elapsed time,
and the verified deleted/already-absent summary. Synchronization records ordinal
step completion after read-back and journal advancement, and final verification.
All three workflows announce browser cleanup before waiting for stop, so a
shutdown failure cannot be mislabeled as destination navigation. These additions
use the existing bounded host sink with fixed stages and numeric counts/timings;
they add no worker wire fields, raw exceptions, portal contents or request data.
Exact rebuilt desktop runtime emission remains to be checked during a supervised
Steam workflow; source and test output alone do not qualify that delivery scope.

## Pending API renames (2026-10-29)

Local achievement and stat API edits retain the original name and Steam identity
from the verified baseline or latest matching-App-ID snapshot. General properties
show an orange Rename from panel with an undoable Revert rename context action.
Repeated edits preserve the original identity; saving retains the intent. Changing
the destination App ID clears it.

The receiving sync session prepares one upload row for a rename. It rejects missing
or ambiguous identities and occupied destinations before any write. Playwright
opens the original record's Edit control, never New for a rename, rechecks its
Steam identity and saves the new name. Read-back requires the same identity, absence
of the old name, unchanged artwork/localizations and unrelated records, and updated
progress references for a renamed stat. Stat validation includes affected remote
achievements. Successful baseline updates clear the pending intent. An interrupted
save can reconcile the destination by the same unique Steam identity.

Evidence: managed rename fixtures cover both entity kinds, record counts, stat
references, occupied destinations and interrupted-save reconciliation. Rust tests
cover retained origin through repeated edits, persistence, revert and undo.
Live Steam forms, native visual acceptance and exact-build runtime/crash evidence
remain unqualified by these source and synthetic checks.

Validation result for the pending-rename change: workspace Rust tests passed,
including 288 UI tests and the new persistence regression. Strict workspace Clippy
and formatting passed on rerun. Managed policy/synchronization checks passed.
The full offline fixture audit at
`C:\Projects\AchimeldBuild\current\audit-c11198abd78f419b8e3a887abb4a00f8`
is not clean package evidence: its initial format/lint failures were corrected,
its Chromium owned-page startup timed out (BrowserFixtures.cs, CreateOwnedPageAsync),
and concurrent source edits invalidated source stability. No live Steam writes,
native rename-panel screenshot acceptance, installer delivery, or exact-build
runtime/crash qualification was performed for this change.

## Live icon hash readback and timed review

The 0.3.0.5 hidden desktop was run through the supervised MCP connection with
the saved waifus6 project (Steam App ID 4873800). The first review had 233 rows.
Exactly one icon pair, ACHIEVEMENT_013, was selected for upload; all other rows
were Skip. Steamworks reported the draft saved, and an independent fresh review
returned achieved SHA-256
`c7db266af28e08aadb9a1063b74bae3d15c8b8b105a351a94499e7cc5aee44f7`
and unachieved SHA-256
`fbc6023e7953b55290b093ab404e3a618612e9cd13f0c3f774eef763b94134b1`.
Both differ from their pre-upload CDN hashes. The review was cancelled and the
editor exited normally. The exact running build emitted a flushed Public
Documents runtime log. This qualifies one live draft upload and readback; it
does not qualify Steamworks publication or every achievement.

The subsequent 0.3.0.6 UI source moves the review to a square modal with one
scrollable list and a stoppable five-second countdown. Focused UI regressions
cover editing, Stop, resume, timed submission and a changed-destination Cancel
path. Its package build was attempted but failed during Rust compilation when
an expected release rlib disappeared while other Rust work was active. There
is no 0.3.0.6 runtime qualification; the combined later build must supply it.

The 0.3.0.8 hidden desktop used the saved waifus6 project for live stress checks.
Validation cancellation during browser startup left the UI responsive; Achievements
opened through MCP selection, and a later validation succeeded. A duplicate
validation request returned `editor_busy` without interrupting the first check.
Stats and Achievements selection worked after validation and while a fresh Steam
review was preparing. Cancelling that review before the five-second deadline
reported cancellation, left the document clean, and preserved 100 achievements
and 32 stats. Its 233 review rows had one default Import and 232 Skip choices,
so the timed boundary did not select a Steam draft upload. The close request
exited normally, with the exact build's Public Documents log recording the
session and cleanup.

One cancellation raced the automatic submission at the read-only `beginSync`
plan check. The worker was stopped and the editor stayed responsive, but the
error exposed `IPC header unavailable` instead of cancellation. `BeginAsync`
only refreshes and checks the remote snapshot and derives steps; draft writes
start later in `StepAsync`. Working version 0.3.0.9 gives cancellation precedence
to worker transport errors from read-only prepare and begin requests. It does
not mask errors during draft steps, whose outcomes may be uncertain. The focused
regression and the full offline fixture audit passed at
`C:\Projects\AchimeldBuild\current\audit-0855b97d546a445e8beaebdf73a119ab`.
The exact rebuilt runtime remains to be checked.

The exact 0.3.0.9 hidden runtime exposed a second pre-write cancellation race:
rapid Cancel while the dedicated Steam browser was starting left the editor
responsive and Achievements selectable, but the result still displayed
`IPC header unavailable`. Starting validation and immediately requesting normal
close cancelled the operation and exited cleanly; the Public Documents log
retained the active worker cleanup. Working version 0.3.0.10 now tracks whether
any `syncStep` request has started. A cancelled error before that boundary is
reported as cancellation across browser startup, validation, remote read and
review planning. At or after the first draft step, the original error remains
visible because the remote outcome may be uncertain. The focused boundary test
and full offline fixture audit passed at
`C:\Projects\AchimeldBuild\current\audit-9e9df4dc378c442093731a4571cd335d`.
The exact 0.3.0.10 hidden runtime then completed three rapid validation-cancel
cycles on waifus6. Cancellation during browser startup and Steamworks navigation
returned `Steam read cancelled`, with no active operation or dirty project.
Revalidation succeeded, Achievements and Stats navigation remained available,
and MCP status reported the review modal as blocking while the five-second
countdown was active. Cancelling that review before submission left no selected
draft write. Both supervised Steam page reads were cancelled and a subsequent
validation succeeded. A normal close during active validation exited the editor;
the exact package emitted a flushed Public Documents log through exit. The live
run did not force a transport failure after a draft step, and it did not repeat
the narrow timed `beginSync` race; the boundary regression covers both outcomes.

An additional 0.3.0.10 watched endurance run cancelled eight validations at
delays from zero to four seconds. All eight stopped cleanly while the editor
remained alive. The next uncancelled validation authenticated and downloaded
the banner, but explicit browser stop failed with `Owned Steam worker process
enumeration incomplete`. The background cleanup reaper confirmed exit about
half a second later, and a retry validated successfully. The process list was
changing while Windows filled it; it remained within the owned Job's 256-process
cap. Working version 0.3.0.11 accepts an in-bound count mismatch because Job
termination and active-process accounting still gate reopening until zero.
Over-capacity counts and failed process queries or termination still fail.
Focused process-list boundary testing and the full offline fixture audit passed
at `C:\Projects\AchimeldBuild\current\audit-48c9edc4771040f2b5a5849b723969d1`.
The exact 0.3.0.11 watched package completed the same eight staggered
validation cancellations, followed by six successful validations. A subsequent
Steam review opened as a reported modal and cancelled before submission;
Achievements and Stats remained selectable, and the document stayed clean
at 100 achievements and 32 stats. The Public Documents runtime log ended in
a normal exit, with bootstrapper exit code zero. The changing-list branch did
not recur in that live run, so its direct evidence remains the bounded-count
test and the source ownership/accounting checks.

One earlier 0.3.0.10 hidden session disappeared during destination navigation.
Its log ended without a normal-exit record or local dump, and a fresh MCP call
found no editor. The launcher's exit code was not captured in that run. Later
watched sessions completed the delayed and eight-cycle tests and exited with
code zero. The cause of that one exit remains unqualified; forced termination
cannot reliably produce in-process crash evidence.

## Local editing during active progress

Active progress is a movable nonmodal window with stage, elapsed time and an
explicit Cancel button. The review keeps its action bar outside the scrolling rows
and states that it uses a frozen snapshot. Host editing, history and ordinary Save
remain available; project switching, Save As and destination changes are guarded.
The worker never replaces the shared project file while it runs. It writes only
verified content-addressed image bytes to the current project's Art Data Bank.
At completion the host merges the frozen result separately with the latest saved
file and the live editor. Object fields and ID-keyed arrays merge independent
changes; overlapping changes fail visibly and retain the editor. Captured PNG
bytes have a 128 MiB total memory cap. Source tests cover the merge and ownership
contracts; installed and live Steam qualification remain separate.

## Completed operations and delayed browser teardown

The 0.2.26.33 live validation completed authentication, but its explicit stop waited
for Windows process objects to signal and converted slow teardown into validation
failure. Explicit stop now requests termination once and hands the same owned Job,
process handles and capacity permit to the background reaper without polling on the
operation's completion path. It reports pending versus confirmed cleanup accurately.
Cancellation, supervisor failure and native cleanup errors still fail the operation.
New reader connections wait cancellably for cleanup before any worker/browser launch;
no process-name lookup, detached browser or protocol operation is introduced.
The real C# process fixture exercises prompt handoff, eventual process exit, reopening,
and survival of a separate worker. This is not proof of graceful Chromium shutdown
latency or live Steam write correctness.
