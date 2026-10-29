# Invalid edit recovery review, 2026-10-29

Scope: current working tree, including pre-existing uncommitted changes. This is
source and fixture evidence, not a guarantee that every possible UI error has been
eliminated. The earlier runtime warning does not identify a field or prove NaN.

Relevant contracts: SEC-DOCUMENT-001 and SEC-DESKTOP-001. History applies commands
to a candidate and validates before accepting it. Host save writes that accepted
document and marks it saved only after storage succeeds. No new storage authority
or protocol operation is introduced.

Confirmed findings and changes:

- Desktop request dispatch blocked Save and Save As whenever any entity or
  localization draft existed. These requests now reach the existing host save
  path. Drafts remain available for correction and are excluded from saved data.
- Menu/window close and native close could leave users repeatedly confronting a
  generic validation message. A conditional upper-bar recovery action discards
  failed drafts without altering accepted document/history. Closing still uses
  the normal unsaved-project flow after recovery.
- Effect explorer save failure was an independent native-close blocker. Recovery
  restores its saved preset, or drops an unsaved failed preset draft, when its
  error is present. Menu and title Exit now use the same close path as native close.
- Shared numeric inputs lacked explicit finite checks before JSON assignment.
  Numeric inspector, layer/effect, miniature effect, 3D, opacity, slider and 2D
  gradient paths now retain previous values when a non-finite edit is produced.
- Particle range painting divided by a potentially zero interval. The denominator
  is guarded, and non-finite range candidates cannot overwrite settings.
- Particle control candidates now validate before replacing layer settings. A
  rejected candidate retains previous settings and emits a bounded generic host
  diagnostic rather than creating another invalid project draft.

Reviewed entry points include entity inspector, layer/style edit failures,
localization drafts, keyboard/menu save, title/native close, Undo, effect explorer,
host save, and history validation. Decimal text fields deliberately retain invalid
text as repairable drafts; silently coercing user text is not appropriate.
Core validation remains necessary for imports and automation. No panic assertion
was added for ordinary invalid user input.

Regression coverage includes preserving accepted unsaved changes during recovery,
allowing both save intents while retaining drafts, reaching normal exit after
recovery, and finite paint geometry for collapsed particle ranges.

Qualification: the first UI suite run reported 289 passes, 2 layout failures and
45 ignored tests. Failures were compact layer expansion and miniature effect icon
visibility. The offline audit also found the pre-existing production image
dependency rejected by the desktop boundary gate. Its first format check failed;
formatting was subsequently applied. That audit run overlapped continued fixes
and is not final-source certification. A later test link encountered a locked UI
test executable. The later broad UI run was interrupted to release that executable;
its modal-dismissal regression was corrected and passed on retry. Final focused
checks passed for recovery (including both save intents, accepted unsaved changes,
and a reachable action at 720 logical pixels), collapsed-range paint geometry,
and title controls/close/modal dismissal. The browser-request fixture failed with
a timeout in BrowserFixtures.cs:78. The offline audit recorded source instability
as other work and these fixes changed the shared tree during the run.

Final strict UI lint passed (`clippy -p achimeld-ui --all-targets -- -D warnings`).
The bounded particle regression run passed 26 tests with 2 native screenshots
ignored. Version history check and diff whitespace check passed.

Native runtime, controlled failure diagnostics, installed
enforcement, and live Steam are not established by these unit checks.
