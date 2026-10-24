# Adversarial audit 1 — Achimeld Steam Partner / Playwright feature

> Historical audit of the pre-migration implementation. External checkout paths have been retired; the findings and binary identities below describe that earlier state. The current source and disposition are in [migration review](docs/security/migration-audit.md). All application libraries now build locally, and no mutation-capable browser driver is shipped.

**Audit type:** read-only source, fixture and package review (no build, no install, no live Steam action)
**Commit inspected:** `1680f85` + working-tree "auditability implementation"
**Primary target:** the "supervised Steam read" path: `Achimeld.exe --read-steam` → Rust app → C# receiving worker → Playwright/Chromium → `partner.steamgames.com`
**Report date:** 2026-10-11

---

## 1. What the "suspicious" feature actually is

The feature is a **Steamworks Partner backend automation**:

- It launches a **visible, persistent Chromium profile** (`%LOCALAPPDATA%\Achimeld\Browser\ChromeProfile`) via Playwright, under a forced loopback proxy.
- The user signs into **`partner.steamgames.com` manually once**; the session (cookies, and by definition the logged-in Steam Partner session) is then persisted on disk in that profile and reused on later runs.
- It navigates only to `https://partner.steamgames.com/apps/achievements/<appId>`, calls the portal's own authenticated JSON endpoints (`/fetchstats`, `/fetchachievements`), downloads achievement icons, and reads the DOM.
- The **retained driver that does this work also contains full write capability** — editing stats/achievements/localization, uploading icon pairs to `/images/uploadachievement`, and deleting achievements (verified in the external reference: `the application browser writer at the time of this audit`, `ApplicationPortalAdapters.cs`).

So the surface is: *an automated tool that holds a standing authenticated Steam Partner session and can mutate partner data if it ever chooses to.* That is exactly why it looks alarming. After tracing every path, the conclusion is that **the shipped Achimeld read path is genuinely read-only at its protocol boundary — but the security of that guarantee rests on a binary the repository cannot verify.**

## 2. Confirmed strengths (calibration)

These are real and were checked, not taken on trust:

- **Receiving boundary is genuinely closed.** `bridge/read-worker/Program.cs:96-110` dispatches only 7 operations; `bridge/read-worker/ReadOnlyPolicy.cs:18-42` enforces argument shapes and pins one App ID per session. Mutation/download/`readDocument`/`evaluate` envelopes are rejected, and the same `ReadOnlyPolicy.cs` is compiled into the negative test suite (`tools/read-policy-tests/ReadPolicyTests.csproj:3`).
- **Process/pipe identity.** Worker pipe is current-user, first-instance, byte-mode, and the client PID must be the launched child (`crates/achimeld-security/src/worker_transport.rs:142-181`). Supervision derives caller PID from the pipe and requires Job membership + exact image (`crates/achimeld-bootstrapper/src/supervision.rs:98-106`).
- **Network containment primitives.** Broker is loopback-only, CONNECT-only, allowlisted hosts, :443, checked public IPs, process-table + Job attribution (`broker/policy.rs`, `broker/transport.rs`). Wire framing is bounded with depth checks (`achimeld-security/src/wire.rs`).
- **No credential capture.** No `GetCookiesAsync`/`StorageStateAsync`/DPAPI/keylogging in the retained source; the transport sanitizer strips `password|cookie|session|token|csrf|…` attributes before the DOM crosses the pipe (reference `PortalDocumentSanitizer.cs`, `RedactionService.cs`).
- The working-tree diff against `1680f85` is **pure hardening** (typed operation enum, continuity pinning, tests) — no check was weakened.

## 3. Findings

| # | Severity | Finding |
|---|---|---|
| F1 | **High (trust/supply chain)** | Read-only is enforced over a binary with write capability and no verifiable source |
| F2 | Medium | Network policy is broader than a read-only task requires |
| F3 | Low | `SEC-PATH-001` ("no lossy decoding") is violated by the bootstrapper |
| F4 | Low | Command lines built with lossy `display()` without argument escaping |
| F5 | Info | Standing authenticated profile + live-site dev probe |
| F6 | Info/Process | Unnecessary retained executables shipped in the package |

### F1 — The read-only guarantee depends on an unverifiable, mutation-capable binary (High)

**Evidence.**

- The worker is compiled against three **external** assemblies copied from an installed reference: `bridge/read-worker/ReadWorker.csproj:19-21` (`Achimeld.Browser.dll`, `Achimeld.Core.dll`, `Achimeld.Security.dll`). Their source is not in this repo. the superseded source baseline points at `the external application checkout at the time of this audit` and explicitly warns "actual contents may include reference working-tree edits." `docs/security/known-gaps.md:8` records the gap.
- The exposed operation set is read-only (`Program.cs:96-110`), **but the object it calls holds `ApplyDraftMutationAsync`** (`reference: PlaywrightChromeDriver.cs:233-275`) and the live writer performs real edits/uploads/deletes. The read path calls `ReadCompleteConfigurationAsync` → `ReadIconContentHashesAsync` → `DownloadIconAsync` (`reference: PlaywrightChromeDriver.cs:137-189`) — all inside the unconstrained binary.
- The project itself concedes the limit: `docs/security/contracts.md:12` ("This does not validate every request made inside the driver") and `docs/security/threat-model.md:35` ("CONNECT … cannot establish read-only portal behavior").
- **Concrete identity anomaly.** The retained browser binary that actually ships is `Achimeld.Browser.dll` SHA-256 `f0fb65e5…` (provenance `.artifacts/read-worker-*/read-worker-provenance.json`; identical in every installed reference under `C:\Program Files\Achimeld-Development\*`). A from-source build of the same reference produces `893f083a…` (app Release) / `b59b9cf5…` (Debug). `f0fb65e5…` matches only the **browser *test* project's Release output** in the reference tree. That is explainable (package assembled from a test output), but it demonstrates the point: the shipped binary is not the reference's primary build artifact, and nothing in this repo binds source→binary.
- The installed reference worker is the **full** retained worker (`Achimeld.SteamWorker.dll` = `690fd8a5…`), i.e. it contains `applyOperation`/`downloadArtwork`. Achimeld replaces only that DLL (`build/BridgePackage.ps1:31`) with its constrained one (`db9c7488…`). If the replacement step were skipped or the reference were substituted, the protocol layer would still be read-only, but the *process* would carry an authenticated partner session and a mutation-capable library — invisible to every test in this repository.

**Consequence.** A malicious or merely divergent `Achimeld.Browser.dll` (side effect in `ReadCompleteConfigurationAsync`/`DownloadIconAsync`, or cookie/session exfiltration) would pass `bridge-package` provenance checks that only compare hashes to a self-supplied reference manifest, and would not be caught by the read-policy tests. The attacker prerequisite is influence over the "verified reference package" or a compromised build/reference machine — which the project already rates as trusted (`threat-model.md:7`).

**Smallest appropriate fix.** Build `Achimeld.Browser/Core/Security` from in-repo source, or (a) require an independently attested source→binary digest for the retained libraries, and (b) reduce the blast radius by moving the read operation into a process/handle that cannot call mutation methods (separate assembly without the writer, or a per-operation WFP/OS restriction). At minimum, replace the self-referential hash check in `tools/bridge-package/Program.cs:28-41` with an external attestation of the retained library.

### F2 — Network allowlist is broader than read-only (Medium)

`crates/achimeld-bootstrapper/src/broker/policy.rs:23-33` permits `store.steampowered.com`, `checkout.steampowered.com`, `login.steampowered.com`, `help.steampowered.com`, `steamcommunity.com`, plus `steamstatic.com`/`steamcontent.com`. The retained `downloadArtwork`/icon path uses the same broad set. A read of `/apps/achievements/<id>` and its CDN icons needs `partner.steamgames.com` + the CDN hosts only. Because CONNECT payloads are opaque (`threat-model.md:35`), the browser/worker can reach purchase- and help-capable origins at the network layer. Recommend trimming the broker allowlist and the driver's allowed-host set to the read task.

### F3 — `SEC-PATH-001` claim vs. bootstrapper implementation (Low)

`docs/security/invariants.json` SEC-PATH-001 claims paths are preserved "without lossy decoding," and `contracts.md:27` says "Both implementations consume shared conformance fixtures." The client does this correctly (`windows.rs:200-203`, `OsString::from_wide`; shared invalid-UTF-16 test at `windows.rs:331-362`). The **bootstrapper does not**: `crates/achimeld-bootstrapper/src/platform/identity.rs:121` builds the process image via `String::from_utf16_lossy`, then compares it in `same_image`. There is no invalid-UTF-16 conformance test for `identity.rs` (only ASCII cases in `platform/tests.rs`).

Impact today is low — a replacement character can never equal an ASCII character, so an impostor cannot collide with `Achimeld.App.exe`/`Achimeld.SteamWorker.exe`/`Achimeld.NodeHost.exe`; the realistic effect is false rejection, not impersonation. But it is a genuine deviation from a stated invariant and a latent hazard if an expected image name ever contains non-ASCII. Fix: use lossless `OsString::from_wide` in `identity::image` and add the shared invalid-surrogate fixture to the bootstrapper.

### F4 — Command lines built from lossy `display()` without escaping (Low)

`crates/achimeld-app/src/node_host.rs:160-164` and `crates/achimeld-bootstrapper/src/platform/process.rs:90-96` build `CreateProcessW` command lines with `format!("\"{}\" …", path.display())`. `Path::display()` is lossy, and there is no Windows argument quoting. A quote or non-UTF-8 unit in the package root would corrupt/inject argv. Exploitation requires control of the install directory name (installer-controlled, `Install-Development.ps1:40`), so this is latent rather than reachable. Fix: escape via `CommandLineToArgvW`-safe quoting and pass wide `OsStr` rather than lossy `String`.

### F5 — Standing authenticated profile and live-site dev probe (Informational)

The persistent profile is the intended session store, is ACL-restricted (`platform/profile.rs:26-42`) and can only be cleared by the editor while no broker-role process is live (`supervision.rs:176-180`). This is defensible, but note the security posture: a valid Steam Partner session persists on disk between runs. Additionally, in development mode the worker performs a **live** authenticated navigation plus an anonymous HTTPS GET to `partner.steamgames.com` (`Program.cs:113-138`) and re-runs the network-denial probe every second (`Program.cs:70-91`). Both are gated on the authenticated `packageMode == "development"`, but they are live-site code paths in a command documented as offline/read-only — worth an explicit scope note.

### F6 — Extra retained executables are copied into the package (Informational/Process)

`build/BridgePackage.ps1:17-28` copies every reference file except a small deny-list. The installed manifests show `mcp/Achimeld.Mcp.exe`, `createdump.exe`, `browser/.../elevation_service.exe`, `browser/.../setup.exe`, `PrintDeps.exe`, etc. all entering the package. They are forced `offline` by `manifest/policy.rs:116-118` or the packaging role mapping, and the bootstrapper only launches `Achimeld.App.exe`. Still, the repository states MCP is "not enabled," yet ships an MCP server binary. Recommend an allow-list copy so the package contains only reviewed components.

## 4. Coverage and exclusions

- **Covered:** all Rust crates; the C# receiving worker and its shared policy tests; build/install/audit scripts; manifest authentication/policy/inventory; supervision, wire, broker, WFP policy, telemetry, UDP probes; snapshot/portal parsing; the external reference browser driver and writer at source level; installed reference manifests and hashes.
- **Not covered (recorded as unqualified):** no build, install, or run was performed; installed WFP and live Steam were not exercised; retained binaries were not decompiled. These remain the project's own `known-gaps.md` items 1, 2, 4, 7, 8.

## 5. Verdict

No backdoor, telemetry, hidden endpoint, credential capture, or arbitrary-execution path was found. The Steam Partner Playwright feature is a **deliberately constrained, read-only reader on top of a much more capable prebuilt automation library**. The material risk is not in the new code — it is that **the read-only property is enforced only by a thin, self-authored protocol layer, while the component that actually touches an authenticated Steam Partner session is an opaque, source-unqualified, mutation-capable binary**, and the evidence binding that binary (F1's hash anomaly) does not come from an independent source. If one thing is to be fixed first, fix F1: own the browser/Core/Security source, or obtain independent attestation of the retained binaries and reduce their runtime capability.
