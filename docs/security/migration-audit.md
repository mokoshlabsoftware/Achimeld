# Local source migration review

Updated 2026-10-11. This repository now contains every application source input required by the current build, compatibility checks and security review. The browser adapter is visible in `bridge/Achimeld.Browser`; its domain and supervision dependencies are in `bridge/Achimeld.Core` and `bridge/Achimeld.Security`.

## Migration and audit fixes

| Boundary | Result |
| --- | --- |
| Managed application libraries | Local project references replace prebuilt application assembly inputs. The worker is freshly published as a self-contained Windows x64 application with its own apphost/dependency configuration. |
| Compatibility tool | Uses local source. Evaluated compilation/project inputs must be regular files inside this repository; regenerated snapshots, canonical hashes and framed wire bytes must equal committed vectors. |
| Application browser capabilities | Only authentication, fixed configuration reads and content icon hashing are deployed. Draft mutation, uploads, generated-image file reads, arbitrary downloads, external browser attach and general caller evaluation have no driver implementation. |
| Security continuity | Both Rust and C# pin session, supervisor, version, mode, browser/profile paths, ports and monitoring coverage across replies. C# supervision frames have a separate 64 KiB budget. |
| Runtime launch | Playwright pins the packaged Rust Node host and removes Node injection/TLS overrides. Browser executable, profile and proxy come solely from authenticated supervision. Driver calls require healthy supervision. |
| Remote data | Configuration/icon HTTP reads disable redirects. Icons use approved content hosts without query/fragment/credentials. Configuration parsing is size checked; snapshots are normalized from fixed JSON endpoints rather than returning arbitrary page HTML. |
| Profile storage | Only the dedicated Achimeld profile is permitted. No existing profile or credentials are moved into source or packages. |
| Dependencies and signing | NuGet locks pin managed dependencies and the vendor Playwright/Node distribution. The .NET self-contained runtime version is explicitly pinned in bridge build properties. Chromium is version/revision/archive SHA-256 pinned and checked against the published Playwright manifest. Signing checks exact managed source/output and browser content hashes against archived source. |
| Audit workflow | Managed publication, browser/security policy and local dependency/compatibility checks are required gates. Missing caches fail offline publication. Omitted installed/live/advisory scopes are recorded explicitly. |

Package construction uses `build/BridgePackage.ps1 -Development`; prepared caches permit `-Offline`. No installed application is a build input. `build/RecordBaseline.ps1` records local managed source hashes. Browser/runtime provisioning does not launch a browser.

## Additional audit dispositions

- F1: Application Browser/Core/Security source is local and compiled here; no writer or upload implementation is deployed.
- F2: Purchase/help hosts are removed. Store/login/community hosts remain at the broker for authentication. The local guard now restricts login routes, removes login authority during reads, pins configuration routes to one App ID, restricts icon image locations and rejects unexpected page/resource/WebSocket traffic. CONNECT cannot constrain encrypted content or a compromised worker. Live compatibility remains unqualified.
- F3: Bootstrapper process image construction and Windows path APIs now preserve original UTF-16 units. The bootstrapper consumes shared identity vectors, including distinct invalid surrogates.
- F4: Editor, worker and Node launches share lossless Windows quoting and environment construction. Native Windows argument parsing regressions cover spaces, quotes, backslashes, empty arguments, invalid UTF-16 and oversized/NUL rejection.
- F5: A persistent authenticated dedicated profile is intentional. Development network acceptance explicitly performs anonymous HTTPS and visible authentication navigation; the source audit command never invokes that live probe.
- F6: No external MCP server or application binaries are copied into a package. Local MCP stdio is compiled into the existing verifier's `--mcp` mode; its receiver is compiled into the supervised Rust desktop. Only six required executables are accepted: verifier, Rust app, Rust Node host, worker, vendor Node and Chromium. Browser installer/elevation/PWA/notification helpers and the .NET standalone dump tool are omitted.

## Follow-up audit of the migrated boundary

The old repository's driver was inspected as source evidence only, including configuration/icon reads and `ApplyDraftMutationAsync`. The current compilation graph, driver, receiver/callers, runtime launch policy, package provenance and native manifest acceptance were reviewed here. No external application binaries or checkout are build inputs. The old writer, uploads, generated-icon file reads and public mutation dispatch are absent from the migrated driver. Core mutation descriptions are data models without portal write effects.

| Confirmed issue | Input-to-effect path and correction | Evidence |
| --- | --- | --- |
| Read command collapsed to one argument | Validated `--read-steam 480` became one quoted argument, so the app rejected it. Validation now returns two tokens preserved by the launch recipe. | Real child fixture accepts only the two-token command; malformed CLI checks remain intact. |
| Icon policy allowed unrelated authenticated Partner paths | A remote icon field could cause a cookie-bearing GET to an unrelated Partner route. Icons now require approved image prefixes/extensions, without query, fragment, credentials or redirects. | Negative icon-route tests and direct request phase checks. |
| Page traffic bypassed receiving read-only dispatch | Page scripts/manual interactions could send edits or navigate elsewhere on approved hosts. Context routing now applies monotonic authentication/read policy; direct API reads are checked separately. | Pure phase/route tests and synthetic Chromium attempts for writes, GET mutation routes, changed App IDs, login redirects, first popup requests, service workers and WebSocket interception. |
| Release source and native roles were looser than build claims | Input inspection used default configuration; native policy allowed additional offline executables/arbitrary browser paths. Release inputs now reject external/generated sources and explicit assembly references. The verifier enforces six executables, roles and its compiled browser lock. | Migration gate and signed-manifest negatives for MCP additions, Node role changes and browser substitution. |

Receiving/supervision wire shapes and committed snapshot/hash/wire vectors remain unchanged. Build `.targets` join provenance. Exact source/run identities and skipped scopes are recorded by the audit command; prior packages do not qualify new source. The pinned SDK's graceful intercepted-WebSocket close failure is recorded in [known gaps](known-gaps.md); the fixture verifies the terminal process/Job path used by the worker monitor.

## Remaining qualification

The production network broker remains the authority for destination attribution and public IP checks. HTTPS content is opaque to it. Configuration fetches and public icon downloads now enforce byte limits while streaming; fixed configuration fetches reject redirects, and icon downloads use the pinned broker proxy without cookies. Playwright still buffers page resources and save acknowledgements internally; application parsing limits do not bound all vendor-runtime buffering. Application-owned source is reviewable here; vendor runtime source, independent reproducibility and dependency advisory freshness require their own assessment.

Run `build/Audit.ps1 -Offline -Fixtures -DependencyAudit` after provisioning caches and the RustSec scanner/database. Exact source identity and gate evidence are recorded per run. A fresh signed candidate must subsequently pass protected installation, normal-user Windows enforcement and an authorized live read; local source/fixture/package checks do not establish those guarantees. See [known gaps](known-gaps.md).

Existing project-file identifiers remain data compatibility literals. There are no application checkout paths or binary application dependencies associated with them.
