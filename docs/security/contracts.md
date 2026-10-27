# Security boundary contracts

## Achievement translation handoff

The supervised editor host owns translation file I/O. Desktop and local MCP operations share the catalog serializer, bounded parser and application import planner. The fixed MCP tools are `get_achievement_translations`, `export_achievement_translations`, and `import_achievement_translations`; they add no Steam worker operations or process recipes. Inline CSV is at most 2 MiB of UTF-8 so nested JSON escaping remains below the 16 MiB MCP envelope budget. Dedicated CSV files remain bounded to 64 MiB; MCP file paths must be absolute and use the `.csv` extension. Export publishes a synced, complete file without replacing any destination, rejects the project catalog's reserved suffix, and checks cancellation before publication.

Imports require unchanged ID, generated API/field context and English source, supported language columns, and matching `{0}` placeholders. Unknown, duplicate, stale or ambiguously shared rows reject the entire plan. Blank cells and omitted rows preserve existing translations. The MCP host rechecks project/session identity, scene revision, cancellation/deadline and busy state after background preparation, then commits one application transaction using the shared undo history. Successful identical requests use the existing bounded retry cache. Import does not save or rewrite the project pair; the normal project Save retains ownership, hashes and recovery checks. Source and synthetic Windows pipe fixtures cover this extension; installed and live Steam enforcement remain separate scopes.

## Protocols and budgets

Local icon style nodes (`styleAchievement`) and art images (`artImage`) remain host-owned canvas records regardless of their editable names. Achievement naming, numbering and synchronization counts exclude them. The host excludes their definitions, icon staging and associated baseline/tombstone records from the synchronization request, then restores their exact records and text before applying the result. The receiving `prepareSync` boundary rejects local markers before the legacy mapper can discard them; only absent or boolean-false markers can enter a review. This tightens input validation without adding protocol operations or changing process/storage ownership.

| Boundary | Contract | Failure outcome |
| --- | --- | --- |
| Worker pipe | Current-user local first-instance byte pipe; actual launched child PID; 4-byte little-endian nonzero UTF-8 JSON length; 32 MiB maximum, JSON depth 64; strict envelope, correlated request ID; no replay. | Bad transport/result closes Rust reader and drops its child Job. Receiving policy/operation failure sends only a static failure code and ends its loop. |
| Worker calls | 15-second connection deadline; 300-second exchange deadline; 9,000 total calls; transport cancellation/health polling. | Terminal closure; no automatic restart/retry. |
| Receiving operations | Existing read/lifecycle envelopes remain compatible. `prepareSync` retains project/catalog/icon hashes for the pinned App ID; `beginSync` selects retained row IDs; `syncStep` consumes the next row with optional hash-pinned PNG bytes; `finishSync` re-reads and returns local data. Synchronization DTOs reject unknown fields. | Generic mutation/download/evaluation operations, arbitrary URLs/paths, changed App IDs, stale tokens/snapshots and repeated failed steps are rejected. Failure and stop are terminal. |
| Synchronization budgets | At most 10,000 local or remote definitions, 8 MiB per uploaded/imported image, generated PNG dimensions at most 4096, 512 MiB generated disk staging, one background thumbnail decode and 8 MiB thumbnail cache. The host reserves the remaining 9,000-call budget before the first selected step. | Unavailable local artwork disables only its icon-upload choice. Oversized/changed data or an insufficient call budget stops the plan before mutation. A durable journal records uncertain outcomes; a new session reads fresh and defaults all choices to Skip. |
| Validation banner | `prepareSync` optionally returns `gameBanner` image bytes discovered through the fixed `store.steampowered.com/api/appdetails?appids={pinnedAppId}&filters=basic` public metadata route (512 KiB limit, JSON depth 32). Only the matching successful app entry and its `capsule_image` are considered. The resolver accepts exact Steam shared Fastly/Akamai or legacy Akamai hosts, the matching app path, a 40-hex content hash, and the small capsule filename; only an optional numeric `t` query is allowed and is stripped. It derives `capsule_231x87_2x.jpg` and retains the old fixed CDN route as an optional fallback. The cookie-free broker-proxied client disables redirects, limits each image body to 2 MiB and covers headers/body with a five-second deadline per request (at most three requests). The host accepts only a decoded 462 × 174 raster with a 4 MiB allocation limit and retains a re-encoded PNG in the sibling project Art Data Bank as an ordinary `User` image with optional `steamBannerAppId` metadata. | Missing, malformed or oversized artwork does not grant validation or write authority. The host saves the banner after a matching authenticated review, retains it on review cancellation, and merges only its own retained banner into a verified final result. No caller-selected URL/path, generic operation, network role or process recipe is added. |
| Supervision pipe | 64 KiB maximum; client overall exchange deadline 5 seconds; server request, response and ACK1 receipt have distinct 3-second phase budgets. Strict typed operations; 64-hex nonce and UDP probe token; operation-specific argument presence. | Malformed or unauthorized connection rejected/disconnected; at most 16 static rejection messages. Control/attribution/probe loss stops Job; policyFailure/failed UDP denial sets health false. |
| Supervision continuity | Nonce/actual server PID/session authenticated; package mode, version, Windows network-enforcement flag, browser/profile paths, ports and coverage pinned across Rust and C# health/probe responses. | Configuration change rejected, and caller closes reader/Node Job. The local C# client applies the same continuity contract. |
| Snapshot | 16 MiB normalized XML, 200,000 nodes, DTD disabled except inert initial HTML doctype; exact origin/App ID; fixed revisions/sections; uniqueness and owner/link checks. | Result rejected; reader closed. This does not validate every request made inside the driver. |
| Project inventory | 32 MiB bounded file read; JSON depth 64; known format/schema 0/1; original bytes retained. | Inventory error; no migration/save capability. Full project/CSV/asset validation is pending. |
| Broker | Loopback only; process-table attribution and Job/role checks; CONNECT only, approved DNS names, :443, 8 KiB header, 64 active clients; checked address connected directly. | Invalid client/request rejected; listener failure stops session. Tunnel lifecycle and total deadlines are reviewed separately from hostname parsing. |
| Diagnostics | Static failure messages/stages, bounded supervisor records and broker diagnostics; process-table destinations are metadata, not payloads. | No arbitrary worker/portal errors included in persisted failure/report data. Local compiler/test logs are separate source evidence. |

The desktop host persists bounded application diagnostic lines immediately under `%LOCALAPPDATA%\Achimeld\RustDiagnostics\log.txt`; startup rotates the previous session to `log_bak.txt`. This applies to development, quick and installed builds. A Windows exclusive lock prevents concurrent sessions from rotating an active log. Each session file is capped at 8 MiB (older current-session records are discarded at the cap). The application library only supplies a host-installed line callback and gains no filesystem authority. Panic records retain source locations, never arbitrary panic payloads. Reparse checks cover the storage directory and fixed filenames; hostile same-user filesystem races and power-loss durability remain unqualified. Clearing the UI output does not delete persisted diagnostics. Startup records identify the executable, compiled version and PID; independent ten-second runtime heartbeats and FX menu breadcrumbs give evidence without opening Terminal.

Effect diagnostics use the same bounded host sink. Opening effect menus, rows and
properties records paired operations on entry or selection changes, not every
repaint. First-use effect-icon loading and each effect-render operation include
correlated operation IDs and thread IDs. Outline vector rasterization, mask
expansion/erosion and composition, and outer-glow padding, expansion, blur and
composition emit checkpoints before work. Numeric dimensions/settings and static
effect names are allowed; arbitrary kind names become `Unknown`. Document text,
asset paths, recipe JSON and raw render errors are not recorded. Completion,
failure, cancellation and unwinding are distinguished; abrupt termination can
leave only a start/checkpoint. This helps locate unfinished work but does not by
itself establish the cause of a crash. Controlled subprocess fixtures render both
effects and check live and post-failure log evidence for panic, native exception
and active UI hang; installed reproduction remains a separate qualification.

The editor initializes a dedicated crash-capture thread before supervision/UI work.
Production diagnostic paths use Windows' current-user Local AppData known folder;
only test builds honor a fixture environment override. Before log rotation, a
supervised session is copied into the corresponding fixed `last-runtime-log*.txt`
file (five slots, 8 MiB each). Startup-only failures cannot replace this evidence.
Rust panics and unhandled native exceptions request synchronous capture; the UI
monitor requests at most one hang dump per session after five seconds of unfinished
active UI work. Capture targets only the current Rust editor process, adds no child
process or worker operation, and serializes DbgHelp calls. Native exception storage
stays live while its thread waits; a 15-second timeout terminates only the failing
process rather than allowing dangling exception pointers. No browser, worker, full
memory, handle or token-data dump is requested. `MiniDumpFilterMemory`,
`MiniDumpWithoutOptionalData` and `MiniDumpWithThreadInfo` retain filtered stacks
and thread information. Filtering is not a guarantee that a dump contains no
private data: dumps are sensitive local files, never automatic uploads or public
audit artifacts.

`RustDiagnostics\CrashDumps` holds at most eight generated capture pairs (`.dmp`
and `.json`); a custom DbgHelp I/O callback rejects writes beyond 32 MiB per dump.
A fixed exclusive capture lock serializes retention across editors. Only validated
generated filenames are pruned, never unrelated files. Partial failed dumps are
removed and safe capture status is retained in JSON when storage permits. Failures
surface in the host warning and runtime log. Forced process termination, fail-fast
paths that bypass exception filters, exhausted resources, corrupted process state
and power loss can prevent capture. This is not a guarantee for all native crashes.
Controlled subprocess panic, native-exception, active-hang, retention and size-limit
fixtures are separate from exact-package runtime qualification.

Concurrent desktop instances that encounter a sharing violation on the main log
use four fixed fallback slots in the same diagnostic directory:
`log-instance-1.txt` through `log-instance-4.txt`, each with its own lock and
`_bak.txt` previous-session file. This supports five concurrent persisted sessions
with at most 80 MiB of session text. Exhausted slots or other initialization errors
produce a visible in-app warning and a bounded diagnostic status (stage, error kind,
native Windows code when available). The independent desktop diagnostic monitor
retries file initialization every ten seconds and replays retained in-memory output
after recovery. The log callback is installed even when the initial file open fails.
Preferences > General exposes this status, the fixed log directory and an explicit
Open log folder action. Only that host-derived, reparse-checked directory is passed
to the Windows shell; no arbitrary executable, path or worker operation is accepted.
Terminal offers read-only startup snapshots of the ten fixed current/backup files,
at most the last 128 KiB per file. Clear and Pause affect only the current UI output.

Normal desktop sessions hold an exclusive `desktop-instance.lock` in this directory
before constructing editor state. A concurrent desktop launch displays the themed
Already running notice, then closes on acknowledgement without loading another
editor or changing the first session. The OS releases ownership on process exit;
a stale filename does not block relaunch. CLI/self-test entry points retain their
existing behavior, and log slots remain available for these and rejected launches.
This is a per-user convenience guard, not an authentication boundary.

Operation breadcrumbs use static categories, monotonic operation numbers and elapsed
times for editor transactions and quick/detail preview jobs. Effect additions record
only the closed-list effect type. A separate thread records active UI work exceeding
five seconds, including its static phase, and subsequent recovery. Idle windows are
not hangs. This does not capture all event-loop, GPU, OS or logger-I/O stalls, nor does
it reconstruct uninstrumented past sessions. Installed runtime qualification remains
separate from source/fixture evidence. No arbitrary paths or worker file authority
are introduced. Request rejection diagnostics distinguish unowned pages,
ping/beacon, media, manifest, event stream and unavailable-frame cases through
declared codes and owned-process exit statuses (201 and 212–216); older codes and
wire envelopes remain accepted. These diagnostics do not authorize any new traffic
and contain no request URLs or payloads.

## Process recipes and ownership

Persistent-browser startup snapshots the restored page list, installs the request
guard and creates one owned blank page before closing the snapshot's tabs. The
context remains offline until ownership and popup monitoring are installed. This
avoids mutating Playwright's live page collection during enumeration and retains
a live tab while replacing restored tabs. The isolated persistent-context fixture
uses a fresh test profile, fixed runtimes and a rejecting loopback proxy.
The fixed Chromium launch arguments block direct DNS with `MAP * ~NOTFOUND`
and exclude only the literal `127.0.0.1` broker endpoint. Without that exclusion,
Chromium also rejects resolution of its proxy IP. The production recipe is shared
with the fixture, which requires an actual connection to the rejecting proxy;
synthetic route fulfillment alone does not exercise this connection.

The in-app Tools > Terminal is a process-local, bounded log view (1 MiB / 4,000
entries, messages limited to 4,096 UTF-8 bytes before a truncation marker). It
collects host statuses and declared worker stages, not raw Playwright logs,
request arguments, portal data, tokens or authentication contents. Existing worker
response envelopes now use a closed set of additional failure codes for timeout,
missing runtime resources, access denial, rejected/unsupported portal data,
incomplete snapshots and browser startup failure. The Rust receiver maps these
to static text, correlates responses first and treats unknown codes as generic
failure. Failures still terminate the session without replay. No operations,
process recipes, persisted diagnostic payloads or worker capabilities are added.
Request-policy failures retain only their first category (navigation, fixed
home/dashboard redirect, resource type, or WebSocket). Network failures map
recognized Chromium proxy/tunnel/DNS/certificate markers to static text. No
request URL, query, header, body, unknown code or raw exception is retained. The
logging facade adapter belongs to the desktop host; the application model keeps
only its dependency-free bounded in-memory buffer.

Navigation failures also distinguish an interrupted navigation, a closed browser,
a connection reset/closure/refusal and an HTTP page-load failure using static
messages. Unclassified navigation failures have a navigation-specific fallback.
The receiver still ends the session on every failure; no navigation or uncertain
write is automatically replayed, and raw browser errors never enter diagnostics.

Failure responses may use the existing `data` slot for a bounded diagnostic object:
`stage`, `kind`, and `elapsedMs`. The host accepts only correlated failures, a closed
stage/category vocabulary, and durations up to 300,000 ms; unknown fields/values
are discarded. Legacy boolean failure data remains accepted. No envelope or
operation authority changes. Waiting operations report elapsed time every five
seconds while checking worker lifetime and supervisor health. Chromium timeout,
network-change, disconnected and generic failure markers map to static text.

The default desktop and `--read-steam APP_ID` run as the current user from the signed package without installation. `--protected [--read-steam APP_ID]` explicitly requires protected package ACLs and verified persistent WFP before launch. Existing `--self-test`/`--self-test-hold` always select protected enforcement. There is no fallback from a failed protected launch. `--user-self-test` is development-only and exercises the app/worker handshake and stop without browser startup or live Steam traffic.

The supervision wire shape is unchanged. `networkEnforced=false, monitoringActive=true` describes an active user session with no claimed OS socket denial. It still requires authenticated peer/Job/image identity, package locks, attributed proxy, request policy and health monitoring. A protected session reports `networkEnforced=true`; either direction of flag change during a session is rejected. Rust/C# direct-denial probes remain restricted to protected sessions and refuse to report success in user sessions. The receiving worker's network self-test is enabled only for protected development sessions. User sessions do not create UDP denial listeners. The existing probePort field retains a valid reserved value for wire compatibility, without representing a live listener.

The browser request guard starts dormant, then permits declared login families during authentication. Reading pins one App ID, removes login authority and permits fixed GET configuration routes and declared static resources. A receiver-selected draft scope arms one fixed post immediately before each save/upload click, then consumes it. Icon scope cannot save definitions. This route/one-post check does not independently validate a compromised page's post body. Configuration reads use a fixed same-origin browser fetch with redirects disabled and a 16 MiB streaming limit. Public icon reads use a cookie-free HTTP client through the pinned broker proxy, with phase/image checks, redirects disabled, a 16 MiB decompressed streaming limit and an end-to-end deadline. Icons require image locations without query/fragment. Context routing denies other writes, unexpected navigation/resources, other-page requests and WebSocket server connections. Rejection latches a terminal failure checked on every operation and by the one-second worker monitor. Package provenance identifies this capability as `reviewed-sync-v1`; the signing gate rejects the former read-only policy identity for a new package.

Browser initialization requests Offline=true and blocked service workers. Routes are installed before restored pages close and networking is enabled for the owned page. This is SDK/context ordering, not an OS guarantee about earliest Chromium startup or internal/background traffic. Synthetic fixtures use pinned binaries in a fixture-owned Job, synthetic responses and a loopback proxy rejecting every connection; they never load the production profile or exercise installed WFP/live authentication. Socket denial ends the fixture process/Job, matching the worker monitor. Graceful intercepted-socket teardown is unqualified.

The validated read CLI stays two arguments through CreateProcessW. Native package policy accepts six reader executables with fixed roles; the browser path matches the lock compiled into the verifier. The migration gate evaluates Release inputs and rejects external/generated application sources and explicit assembly references.

| Recipe | Required ordering / inherited authority | Cleanup |
| --- | --- | --- |
| Bootstrapper editor | Fixed packaged executable; allowlisted/rebuilt environment; suspended, detached console; no inherited handles; assign to Job before resume. The bootstrapper owns a separate startup image window outside the Job and hides only its own console for desktop launches; CLI commands retain console behavior. | Kill-on-close Job; mandatory control loss terminates tree. The startup image closes after the editor window becomes visible; startup failure is shown in that window. |
| Rust worker launch | Fixed packaged worker apphost; supervision/package values from parent; signed Node-host override; suspended, detached; no inherited handles/breakaway; nested Job assignment before resume. | Rust reader owns both pipe and child in one open state; closed state retains neither. |
| Node host | Authenticate supervisor; packaged CLI and Node; explicit inherited stdio list; attribute/handle-list storage live through CreateProcess; suspended/detached Node assigned before resume. | Owned attribute/stdio/process/Job resources; health loss returns and closes nested Job. |

Worker publication applies `build/PlaywrightRuntime.ps1` only to the exact SHA-256-pinned Playwright 1.62.0 driver bundle. Its Windows forced-close fallback calls the existing browser child's native `kill("SIGKILL")` instead of spawning `cmd.exe` and `taskkill`. Browser descendants remain owned by the Node host's kill-on-close Job until driver exit; failed worker operations are terminal and the reader also owns an enclosing Job. No system executable is added to the authenticated process allowlist. Publication records upstream/patched digests before inventory/signing and binds the patch script to the archived source. Unknown or already modified bundles fail publication. The real Node-host fixture exercises forced cleanup and health loss with a descendant, rejects helper-process cleanup, and checks both process handles reach exit. Browser startup, installed enforcement and live Steam validation still require their separate qualification.

Package authentication checks the exact received manifest bytes with the embedded trusted key, then pure schema/identity/role policy. Inventory hashing, reparse checks and session-long file/directory locks apply to both launch modes; effective administrator-owned ACL checks additionally apply to protected sessions. `VerifiedPackage` has private fields and read-only accessors; it cannot be freely constructed/mutated outside its module.

Complete-path comparisons use Windows components and ASCII case folding over original UTF-16 units. They accept separator/case variants but do not lossy-decode invalid names, resolve aliases, prove file identity or replace protected-directory/file-lock checks. Both implementations consume shared conformance fixtures.

Windows resources document local `SAFETY` obligations and allocator-specific release APIs. Workspace lints deny undocumented unsafe blocks, missing public unsafe safety contracts, and implicit unsafe operations in unsafe functions. A lint-passing comment is an obligation to verify, not a soundness proof.

Windows launch construction preserves original UTF-16 executable, argument and environment units. One shared quoting helper supplies editor, worker and Node argv; native parser tests cover spaces, quotes, trailing backslashes, empty arguments and invalid surrogates. NUL and oversized command lines are rejected. The dedicated profile is solely the Achimeld profile under current-user local application data.

Development network self-tests contact the live Steam origin anonymously and perform visible authentication navigation. They are a separately invoked acceptance scope; the source/fixture audit never invokes them. Persistent authenticated profile storage is intentional and does not imply filesystem sandboxing.

Validation first opens the pinned app's achievements route, waits for manual
authentication if needed, then reopens that same route before configuration
reads, matching the existing read flow. Static asset names permit the URI
unreserved `~` used by Steamworks bundle filenames; host, method, resource-type,
extension and phase checks still apply. This does not add an endpoint or host.

The achievements page also reads `GET /apps/getachievementcounts/{pinnedAppId}`.
The request guard permits that exact same-origin background read during the
pinned authentication/read phases, with no query string or navigation authority.
It is not a configuration serializer endpoint, and no worker operation is added.
Other App IDs, trailing paths, queries and POST remain denied. The route and
method were observed on the existing Mosaica achievements page on 2026-10-19;
source and synthetic Chromium fixtures cover the narrow exception.

The supervisor initially inspects each sampled Job member through one owned
process handle with query and synchronization rights. After approving its image,
the monitor retains that handle and immutable image identity while the process
remains live, avoiding repeated path queries during Chromium teardown. The cache
contains only sampled Job members (at most 256). A positively signaled cached
handle is removed before reopening its PID, so it cannot authenticate a reused
PID. A newly inspected process can be skipped before image inspection, or after
an image query races its exit. A failed first query retries image inspection and
checks exit at 10 ms intervals for at most 500 ms on that same handle. Windows
can temporarily deny image queries during startup or teardown. A live
unclassified process still requires an approved image; query denial is not
treated as departure. If opening/querying a sampled PID fails, a fresh native Job
enumeration must prove that the PID has departed before it can be skipped. The
fixture retains a terminated child's handle to exercise the exited-object case,
and a live synchronize-only handle verifies that missing query rights fail closed.

Draft definition creation separately arms one fixed `newachievement/{appId}` or
`newstat/{appId}` POST immediately before opening the editor. It cannot replace
the separately armed save, is consumed once, and is unavailable in read or icon
scope. Steam's individual `fetchachievement/{appId}/{statId}/{bitId}` and
`fetchstat/{appId}/{statId}` metadata reads use POST in the legacy editor. Only
matching reviewed scopes, pinned App IDs, canonical numeric record IDs and
XHR/fetch requests can use these routes; queries and navigation are denied.
No delete or publish route is permitted. Fresh Steam-only entries default to
Import in the review so existing definitions are brought into a new project
first. Reconciliation after an uncertain outcome still resets every choice to
Skip and requires a fresh review.

If the worker monitor exits before its active reply, exit codes 201 through 211
carry only declared request-rejection categories. The host reads that status
from its owned worker process handle with a bounded 250 ms wait after transport
failure and maps known values to static text. Unknown codes retain the original
failure. Request URLs, bodies, headers and browser exceptions are never logged.

Ordinary achievements have an optional `steamSyncStatus`: absent or `Active`
participates in synchronization. `Unsynced` means ready to synchronize and also
participates; a verified definition step marks it `Active`. `Retired` remains an
editable local card excluded from synchronization. Both receiving validation and host result application preserve inactive
cards and their localized text. No definition or icon review rows exist for them,
including when Steam has the same API name, so forged selections are rejected.
A unique Steam ID from verified snapshot/baseline history can identify a remote
API-name change. Importing its reviewed new row retains the old card as
`Retired` and creates a new local identity with imported artwork. Skipping that
import never restores upload authority to the old name. Ambiguous identities
do not establish a rename. No delete or publish operation is introduced.

The receiver maps definition fields without host artwork selectors or recipes;
their complete original JSON stays in the retained result. This allows current
nullable general-image tile fields without older DTO conversions. Imported
assets use the shared `SteamImportedArtwork` identity so later reads remain
compatible. The host checks retired cards, local helper cards and their text
before saving as well as when applying a result to editor history.

## Local editor MCP

### Portable style collections

The explicit local tools `list_style_collections`, `get_icon_style`,
`create_style_collection`, `create_icon_style`, `import_icon_style`,
`export_style_collection`, and `export_style_preview` operate through the same
receiving schema, current-account pipe and revision/request/session checks. The
revision observation includes manual and MCP collection changes. Collections are
separate from canvas history. Presets are embedded, read-only templates and are
never inserted into a new project automatically. Explicit import clones a portable
recipe into one project undo transaction, leaving all achievements unchanged.

Portable authoring/import requires the existing validated style recipe, at most
1 MiB, 512 layers and 32 effects per layer. General artwork uses achievement slots;
non-null image/atlas IDs and filesystem references are rejected. Optional edits
replace at most 128 existing scalar properties by JSON pointer; layer identities,
types and relationship fields require a complete validated recipe. Clones allocate
new style/layer/effect IDs and preserve internal mask/folder relationships. Each
collection has at most 128 styles; the open library budget remains 128 MiB.

Collection export uses the existing host-owned `.achv`/CSV/art-bank transaction
and refuses existing destinations. Preview export renders a pinned viewer snapshot
on the existing background-operation slot, through the native renderer, with a
maximum 1024 × 1024 output and 20-second response budget. It encodes before
publication, checks cancellation, and opens the destination PNG with `create_new`.
Both exports require absolute local-drive paths and existing reparse-point checks.
They add no project open/save, arbitrary executable dispatch, network, worker or
Steam authority. Filesystem races against a hostile same-user process remain outside
this local trust claim. Built-in preview artwork is retained by the viewer's
independent store and is not implicitly imported into project assets.

The existing signed verifier accepts exactly `--mcp` after package verification and refuses an elevated facade. It launches no editor/worker/browser. The desktop receiver starts only after the existing supervised app entry point succeeds. The facade's main thread polls current-user editor endpoint discovery every 100 ms and ends the process after two continuous seconds without an endpoint (including startup without an editor), or on discovery failure. Blocking client stdio runs on background threads and cannot retain the process or verified package locks after this shutdown. Discovery is a lifetime hint, not new authentication or mutation authority; existing receiver checks still apply. Multiple open editors keep the facade available. No executable role, supervision wire shape or Steam operation is added. See [MCP usage](../mcp-server.md) and [audit scope](mcp-audit.md).

The account-hashed, PID-bound local Windows pipe is non-inheritable, rejects remote clients and uses an ACL for the current account/SYSTEM. The client checks the kernel-reported server PID and requests identification-level security quality of service. A connection carries one 16-MiB/depth-64 request/response pair plus acknowledgment before server handle closure. Invalid/disconnected instances are discarded. The receiving UI thread revalidates the closed schema, session/project/revision, request ID and busy state before preparing/committing commands. Successful results are cached before delivery, limited to 256 entries and 16 MiB; project reopening clears them. The stdio input queue holds eight messages, editor queue one, and one background decode/export operation is allowed. Overload terminates the facade connection. Ordinary requests expire after 20 seconds, export after ten minutes; cancellation/disconnect can prevent queued edits, while executing transactions finish atomically.

Artwork is decoded in the host background worker and retained in the Art Data Bank. Styles remap cloned layer/effect identities and validate required general image indices. Export is an explicit local filesystem operation against a pinned document snapshot, with fixed supported image formats/dimensions, sanitized names and a bounded preflight inventory. Matching image files may be overwritten; complete earlier files can remain after interruption. Local-drive paths and existing reparse checks do not prove safety against hostile same-user filesystem races. Remote writes, project open/save, arbitrary executable/URL dispatch and browser launch are absent from the MCP registry.
