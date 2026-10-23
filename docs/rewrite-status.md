# Implementation status

Updated 2026-10-14. All application source used by the build is local. The source migration is described in [migration review](security/migration-audit.md); audit qualification starts at [security/README.md](security/README.md). Desktop workflows and outstanding parity are tracked in [desktop rewrite](desktop-rewrite.md).

## Implemented

The Rust workspace contains signed package verification, a Windows network broker and WFP controls, Jobs, authenticated supervision, the constrained read client, canonical hashes, normalized snapshot parsing and an offline project inventory. The C# read worker, Browser/Core/Security libraries, and compatibility oracle build from local source. Package creation requires no installed application or application checkout.

Managed dependencies are pinned in project lockfiles. The Playwright package contains the vendor JavaScript driver and Node runtime; Chromium is separately archive-digest pinned. These third-party runtimes remain security review dependencies, distinct from locally compiled application code.

The dedicated profile is solely under the current user's Achimeld browser directory. No user profile data is copied. The worker has no mutation, upload, caller-supplied download, evaluation or external browser attach commands.

## Pending qualification and product work

Full installed WFP/browser acceptance and an authorized live Steam read are required for each new signed package. Source and synthetic pipe success do not qualify that package's Windows enforcement. Official release identity and independently reproduced builds remain pending.

Native Rust editing, a common undo history, live preference colors, stable layer reordering, background previews/export and recoverable project/catalog saves are implemented and have focused source fixtures. Full Achiever feature parity, synchronization writes and MCP are not complete. Data format compatibility and the committed C# generated golden vectors remain intact. The [roadmap](../ROADMAP.md) is future work, not a release claim.

Exact source identity, selected checks and skipped scopes are recorded in each audit evidence file. No historical installed binary hash qualifies the current source.
