# Auditability implementation and review

Baseline: `1680f857584f2b23fcce94a92c43e3cf28b62ae3`. This implementation is a working-tree change; use the audit command's complete source digest and file inventory, rather than treating that baseline commit as the identity of the new code. Review date: 2026-10-11. This is local implementation review, not an independent security certification.

## Implemented structure

- The [security entry point](README.md), [threat model](threat-model.md), [invariant registry](invariants.json), [boundary contracts](contracts.md), root repository navigation and [known gaps](known-gaps.md) give auditors a short path from each claim to enforcement and evidence.
- Manifest authentication, pure policy and locked filesystem inventory are separate modules. `VerifiedPackage` exposes read-only accessors and owns the session's replacement locks. Windows handle/identity/ACL/profile/process/pipe adapters and broker policy/transport are separate review units.
- Supervision parses a closed operation enum, derives roles from OS peer identity and Job membership, authorizes each operation before effects, and distinguishes malformed requests from loss of mandatory controls. Only the editor can clear profiles; an uninspectable live child prevents clearing.
- Rust worker commands are typed, and transport ownership has one open/closed state. The new local C# receiver enforces a read/lifecycle allowlist, exact argument shapes and one canonical App ID. It has no mutation, download or arbitrary evaluation dispatch. Development network probes require the authenticated package mode.
- Every Rust crate inherits unsafe-contract lints. Strict Clippy includes the bootstrapper. FFI safety explanations state ownership, storage and cleanup obligations; they remain reviewable claims, not automatic proof.
- Shared Windows path fixtures, negative receiving-policy tests, real pipe/process/stdio fixtures and bounded deterministic parser mutation tests challenge the boundaries. They preserve the existing legacy wire and snapshot compatibility vectors.
- `build/Audit.ps1` runs selected gates, fails required failures, records skipped scopes explicitly, hashes logs and exact source, and detects source changes during the run. CI invokes the same source/fixture entry point and preserves its artifacts.
- Development package assembly compiles the receiver and application libraries from local project references, checks source/runtime/DLL provenance before signing, and includes exact local source, public trust configuration and provenance in the signed inventory. Native policy independently enforces the six reader executables, their roles and its compiled browser lock.

## Defects found and fixed during review

| Finding | Consequence and correction | Evidence |
| --- | --- | --- |
| Lossy Windows process path comparison | Different malformed UTF-16 names could compare equal after replacement-character decoding. Both identity implementations now compare complete absolute path components using UTF-16 code units with ASCII case folding. | Shared vectors plus distinct invalid-surrogate regressions in both implementations. |
| Inherited WFP rejection diagnostics inspected rejected nested pointers | A malformed native condition/blob could reach unsafe slicing during diagnostics. Rejected filters now produce a fixed message; comparison validates optional blob pointers/data/size before slicing. The native allocation lifetime contract remains explicit. | Synthetic valid/null-condition/null-blob filter regression; installed BFE behavior is separate. |
| Public-IP policy missed the newer IPv6 documentation range | The inherited global-unicast approximation accepted `3fff::/20`, which [IANA classifies as documentation and not globally reachable](https://www.iana.org/assignments/iana-ipv6-special-registry). The broker now rejects that entire prefix. | Lower/upper prefix boundary regressions alongside private/mapped/transition destination checks. |
| Native connection-table storage and decoder were too permissive | Byte storage did not state native alignment, short headers could panic, and a generic safe decoder could admit invalid Rust representations. Native calls now use aligned storage; short/count-invalid tables are rejected; decoding is restricted to the four integer-only row types. | Short-header/count regressions and strict unsafe lint. |
| Health response configuration could drift | Subsequent replies could change session/runtime configuration. Rust clients now pin supervisor/session/version/mode/profile/browser/proxy/probe/coverage fields across replies. | Mutated session/configuration regressions. |
| Receiver capability depended on the caller API | The inherited worker exposed broader commands than the Rust reader. Packaging now substitutes a local closed receiving implementation, sharing its exact policy with negative tests. | 29 policy cases; actual synthetic pipe attempts for mutation/download/arbitrary-read/evaluation envelopes. |
| Initial elevated verifier lacked a required identity pin | Self-verifying supplied executables could run before their identity was independently selected. The installer now requires the separately retained trusted-build SHA-256, copies pinned bytes to administrator-only staging, and executes only that verifier there. Untrusted inventory copies are confined to staging as regular files; full signature/inventory verification and identity consistency precede normal-user readability. | Pure digest/stream-reset/staging-ACL tests and script syntax checks. Full privileged installation/ACL propagation remains unqualified. |
| Application/library build provenance was incomplete | All managed application libraries now compile from local source and project references. The signer binds every published runtime output and application source hash to the archive; Chromium is independently archive-digest pinned. | Required managed publication, local dependency/compatibility gate and signed package review; see [migration review](migration-audit.md). |

## Validation and evidence boundaries

Run the final source/fixture/package-build/advisory gates with:

```powershell
./build/Audit.ps1 -Offline -Fixtures -DependencyAudit 
./build/BridgePackage.ps1 -Development 
```

Local validation passed 54 ordinary Rust tests, all six selected Windows pipe/process/stdio fixtures, 29 C# receiving-policy cases, full strict lint and formatting, navigation/script/installer checks, matched receiver compilation and the scoped Rust advisory gate. The initial complete run exposed the wrapper environment failure described above; it was repaired and the whole selected gate rerun. Package integrity validation is separate from installed acceptance.

Each run emits a new ignored `.artifacts/audit-*/evidence.json`; inspect `selectedChecksPassed`, the exact source digest, gate logs and `not-run` reasons. The source identity includes uncommitted contents and deleted tracked files. The bridge builder reports a fresh candidate path and verifier digest. Ordinary direct app/worker startup must continue to refuse unsupervised execution. No administrator installation or live Steam operation is part of this source command.

The RustSec scan uses official `cargo-audit` 0.22.2 with pinned archive/executable digests. The reviewed database revision is `7eebec69c352c7191b1f13eb95dd510eeca5d1de` (commit 2026-10-09). It reports one raw finding, [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), concerning private-key timing with no patched version available. The [exception](dependency-exceptions.json) applies only to the current public-verification-only production use, expires 2027-01-09 and is printed rather than hidden with scanner ignore flags. This is a use-specific disposition, not a claim that the dependency has been patched. Other/new findings and expiry fail the gate.

The implementation has been reviewed across entry points, caller and receiver dispatch, session pinning, extraction/ownership, FFI storage, build provenance, installer execution and evidence reporting. Source/fixture success cannot prove effective installed WFP rules, deployed browser behavior, independent third-party runtime source/build correspondence, independent release identity, or freedom from every vulnerability. These remaining scopes and sustained fuzzing requirements are listed in [known gaps](known-gaps.md).
