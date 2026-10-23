# Threat model and authority map

## Assets and assumptions

Protect package integrity and execution identity, Steam profile/authentication data, remote app configuration, user project/snapshot data, and signing authority. Project bytes, portal/worker result bytes, IPC request fields, CLI inputs and unrelated local peers require validation even when their producer is expected to be legitimate.

The current model covers malicious input, malformed/replayed IPC, unsupervised local clients attempting to use the broker/supervisor, loss of mandatory controls, and accidental authority expansion during migration. It assumes a trusted Windows kernel, compiler/build environment and selected initial development verifier/key. Normal user sessions run directly from a verified user-owned package without elevation. Explicit protected sessions additionally assume administrator installation and persistent WFP policy. Development trust is locally selected; it does not establish an independent publisher identity.

User sessions intentionally have no administrator-owned installation ACL requirement and no Windows socket-denial guarantee. Signature/inventory verification and session-long replacement locks still apply. Browser requests and proxy destinations remain constrained in application code, but compromised code can bypass those settings and open sockets directly. Protected sessions check installation ACLs and WFP before any child starts, continuously verify WFP, and never downgrade on failure. The authenticated `networkEnforced` status is false for user sessions and is pinned across health replies in both clients.

A same-user process with injection/debug access, compromised privileged installer, or arbitrary code execution inside any supervised process is not defeated by the present design. Jobs and WFP do not make a general filesystem/process sandbox. Malicious code inside the app could attempt alternate packaged Node/browser launch paths. The receiving policy constrains commands arriving at its protocol boundary, including arbitrary requests from a caller, but does not establish containment of a compromised process or code that bypasses that boundary.

## Data and authority flows

```text
package + CLI -> bootstrapper -> signed inventory/locks/Job checks
                              -> optional explicit protected ACL/WFP checks
                              -> supervised Rust app
app -> private pipe -> authenticated receiving worker -> Node host -> Node -> Chromium
worker/Node/Chromium -> attributed loopback broker -> approved public Steam HTTPS destinations
worker reply -> bounded frame/envelope -> pinned App ID/origin/shape -> snapshot -> local export
offline inspector -> bounded local file parsing only
```

| Authority | Privilege and capabilities | Launch/lifetime owner |
| --- | --- | --- |
| Installer | Administrator; copies a selected development package into a protected version directory and installs/removes package-specific WFP policy. Initial verifier identity must be selected independently. | Developer/Windows administrator approval; not launched by source audit command. |
| Build/signing tools | Normal user; compile/copy executable code, choose embedded development public trust and sign an inventory with an ephemeral in-memory key. Source/compiler/locked third-party runtime must be trusted inputs. | Developer build process. |
| Bootstrapper | Normal user during sessions; reads and locks package contents, manages Jobs, supervisor IPC and the broker. Explicit protected sessions also verify effective installation ACLs/WFP. Broker itself reaches permitted remote destinations. | User CLI; package startup refuses elevation. |
| Rust app | Normal user; declared offline network role (OS-enforced only in protected sessions), supervised IPC, local desktop document commands, user-selected project/catalog/assets/export files, per-user preferences and bounded snapshot diagnostics. The UI/application split is an in-process architecture boundary. | Bootstrapper Job. |
| Receiving C# worker | Normal user; broker network role; constrained read/lifecycle requests, receiver-owned reviewed synchronization and a development-only network probe. Accesses the dedicated browser profile. Imports return approved image bytes; project/artwork storage stays in the Rust host. | Rust app's nested kill-on-close Job inside bootstrapper Job. |
| Node host | Normal user; offline network role; only packaged `cli.js run-driver`; only three explicit stdio handles inherited. | Receiving worker/Playwright, supervised; owns nested Node Job. |
| Node/Chromium | Normal user; broker network roles; signed runtime, fixed driver/browser and dedicated profile. Third-party runtime code has broader internal capabilities than the request interface. | Supervised child tree and Node-host/bootstrapper Jobs. |
| Offline inspector | Normal user; local bounded project/snapshot reads; no worker launch or website operations. Its crate shares build dependencies with the app. | Direct user invocation. |

Worker pipe ACL/current-user checks supplement actual peer PID checks. Supervisor clients are authorized using OS-reported pipe PID, Job membership and complete package executable identity, never a PID/name supplied in JSON. Only the editor may request profile clearing; it must wait until every broker-role process is absent. Inspection failures for still-live children prevent clearing and stop mandatory supervision.

The broker authenticates the local process/role and limits CONNECT targets to approved hostnames, port 443 and checked public IPs. TLS endpoint verification is performed by the browser/HTTP runtime. CONNECT does not inspect encrypted methods, paths or payloads, and cannot establish read-only portal behavior.

Reviewed Steam draft mutation is available through a receiver-owned synchronization session. The desktop saves and pins its project before preparation, renders bounded icon pairs in the host, and presents field differences with explicit upload/import choices. The worker retains the project, catalog, icon hashes, one App ID and the complete reviewed remote fingerprint. Execution selects retained row identities, checks dependency closure, consumes each step once and re-reads before and after mutations. A durable host journal marks a step uncertain before dispatch; interruption requires a fresh remote read and new review, with every choice initially skipped. The host retains imported artwork in the Art Data Bank and commits project/catalog updates through existing recoverable transactions. Publication, remote deletion, arbitrary paths/URLs/scripts and generic write envelopes remain unavailable. Official release trust and exact-package installed/live acceptance remain separate.

The local browser guard constrains trusted code and page scripts: authentication has declared login routes; reading loses login authority and pins the App ID. A reviewed mutation temporarily allows only the matching stat/achievement save route or achievement image upload, plus fixed read-back routes. Outside that scope, writes are denied. Unexpected navigation/resources, other-page requests and WebSocket server connections are denied throughout. Configuration reads use a fixed same-origin streaming fetch under interception. Public icons use a cookie-free HTTP client through the pinned broker proxy, with phase/image checks, redirects disabled, a decompressed byte limit and a deadline covering the entire body. This reduces accidental/page-driven authority; it does not contain code that bypasses the guard inside a compromised worker. Live authentication/resource compatibility remains unqualified. Steam provides no atomic conditional save, so external changes between the final check and save cannot be excluded; read-back stops on a mismatched target or unrelated changed value.
