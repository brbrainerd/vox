---
title: "Vox GUI-Native Language Roadmap (April 2026)"
description: "Executable roadmap for turning Vox into a GUI-native language whose compiler catches correctness invariants that React + TypeScript structurally cannot."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Vox GUI-Native Language Roadmap (April 2026)

> **Phase numbering:** This plan uses the **GUI-native language** phase sequence (Phases 0–8). For the other two sequences, see [phase-numbering-index](phase-numbering-index.md).

> **Document purpose.** This is an executable roadmap for turning Vox from a
> "Rust/React-emitting toolchain with a VS Code extension" into a GUI-native
> language whose compiler catches correctness invariants that React +
> TypeScript structurally cannot. It is written to be executed by a less
> capable LLM (e.g., Gemini 3.1) one task at a time, with enough context per
> task that the executor never needs to infer architecture.
>
> **Provenance.** Derived from a conversation with Bertrand Reyna-Brainerd
> (repo operator) on 2026-04-23 covering: VS Code plugin audit, Axum dashboard
> migration at commit `df1d6919`, Vox GUI authoring layer design, K-complexity
> analysis of the existing primitive set, and a structural plan to surface
> error classes TypeScript cannot catch.
>
> **Scope.** ~30 tasks across 8 phases. Phases are ordered by dependency; tasks
> within a phase are mostly parallelizable. Total estimated calendar time with
> one full-time engineer + agent execution: 5-6 months.

---

## Table of Contents

1. [How to use this document](#how-to-use-this-document)
2. [Mandatory preamble — read before any task](#mandatory-preamble)
3. [Glossary](#glossary)
4. [Phase 0 — Dashboard safety (THIS WEEK)](#phase-0--dashboard-safety)
5. [Phase 1 — Dashboard cleanup](#phase-1--dashboard-cleanup)
6. [Phase 2 — Compiler primitive collapse](#phase-2--compiler-primitive-collapse)
7. [Phase 3 — Grammar unification policy](#phase-3--grammar-unification-policy)
8. [Phase 4 — Compiler primitive expansion](#phase-4--compiler-primitive-expansion)
9. [Phase 5 — Web IR correctness validators](#phase-5--web-ir-correctness-validators)
10. [Phase 6 — Vox GUI authoring DSL](#phase-6--vox-gui-authoring-dsl)
11. [Phase 7 — Dashboard re-author through `vox-codegen-ts`](#phase-7--dashboard-re-author)
12. [Phase 8 — Corpus migration + MENS training](#phase-8--corpus-migration--mens-training)
13. [Appendix A — Common pitfalls](#appendix-a--common-pitfalls)
14. [Appendix B — Verification playbook](#appendix-b--verification-playbook)
15. [Appendix C — Escalation protocol](#appendix-c--escalation-protocol)

---

## How to use this document

1. **Read the preamble first.** The policy invariants there apply to every
   task. Violating them is an automatic rejection.
2. **Pick one task at a time.** Tasks specify their preconditions at the top;
   do not start a task whose preconditions are not met.
3. **Do not improvise architecture.** If the task doesn't specify it, ask the
   operator. The escalation protocol is in Appendix C.
4. **Run the verification commands** listed in each task. All must pass before
   marking the task complete. If any fail and the failure is not covered by
   the task's "Known issues" section, STOP and escalate.
5. **Honor the file-modification lists.** Tasks enumerate the files they may
   modify and create. If a change requires touching a file outside that list,
   STOP and escalate.
6. **Commit granularity.** One commit per task, except where explicitly noted.
   Commit message format: `feat(<crate>): TASK-XX.Y — <short summary>` for
   feature work, `chore(<crate>): TASK-XX.Y — <summary>` for cleanup, `docs:
   TASK-XX.Y — <summary>` for documentation. Include `Co-authored-by: AI
   Assistant` trailer; primary author is the operator.

---

<a id="mandatory-preamble"></a>
## Mandatory preamble — read before any task

### Required reading (in order)

You must read these files before starting *any* task. They contain policy that
applies repo-wide:

1. `AGENTS.md` — the always-loaded policy surface. Non-negotiable rules for
   cross-tool, session-critical work. Pay particular attention to:
   - §Research and Documentation Storage: research goes to
     `docs/src/architecture/`, never to IDE-private knowledge bases.
   - §AI Context Exclusion: `.voxignore` is the SSOT; other ignore files are
     derived via `vox ci sync-ignore-files`.
   - §Secret Management: use vox-secrets (`vox_secrets::resolve_secret`). Do not
     introduce direct `std::env::var` reads for secrets.
   - §Cryptography Policy: use `vox-crypto`. Banned: AEGIS, `ring`, any
     wrapper dragging `cmake` or `nasm`.
   - §VoxScript-First Glue Code: ALL automation must be `.vox` executed via
     `vox run`. No new `.ps1` / `.sh` / `.py` glue scripts.
   - §Retired Surfaces: symbols that MUST NOT be used. Using them is an
     automatic rejection.
   - §Archival Protocol: `archive/` and `docs/src/archive/` are tombstoned.
     Do not read, ingest, or modify them.
2. `CLAUDE.md` — Claude-specific overlay. Treat `.vox` files as Vox, not Rust
   or TS. Honor `// vox:skip` annotations.
3. `README.md` — top-of-stack summary. Skim only; the detail lives in
   `docs/src/`.
4. `docs/src/contributors/contributor-hub.md` — contributor entry point.
5. `docs/src/architecture/architecture-index.md` — architecture map.

### Absolute policy invariants (never break)

| # | Rule | Consequence if broken |
|---|------|----------------------|
| P1 | No new `.ps1` / `.sh` / `.py` glue scripts. Automation is `.vox` via `vox run`. | CI gate failure + rejection. |
| P2 | Use `vox_secrets::resolve_secret(...)` for secrets. No raw `env::var` for sensitive values. | `vox ci secret-env-guard` fails. |
| P3 | Use `vox-crypto` for cryptography. No direct `ring`, AEGIS, cmake/nasm. | CI crypto-policy gate fails. |
| P4 | Edit `.voxignore` only; derived ignore files are regenerated via `vox ci sync-ignore-files`. | `vox ci sync-ignore-files` fails. |
| P5 | Do not use retired symbols (see `AGENTS.md §Retired Surfaces`). | Automatic PR rejection. |
| P6 | Do not read, modify, or ingest `archive/` or `docs/src/archive/`. | Hallucination risk; rejection. |
| P7 | All new `.vox` code blocks in docs must compile via `vox-doc-pipeline`. Use `// vox:skip` only for intentionally invalid snippets. | Doctest gate fails. |
| P8 | Structural limits: blocks >500 LOC or >12 methods, directories with >20 files trip the sprawl detector. Honor it. | `vox ci toestub-scoped` fails. |
| P9 | Do not author research into IDE-private knowledge bases (Antigravity, Gemini, etc.). Write to `docs/src/architecture/` in the repo. | Lost work; policy violation. |
| P10 | Commit author attribution: primary author is the operator (Bertrand); agent is `Co-authored-by:` trailer. | Provenance audit failure. |

### Repo layout cheat sheet (critical paths only)

```
C:\Users\Owner\vox\                        — repo root
├── AGENTS.md                              — MUST READ before any task
├── CLAUDE.md                              — Claude-specific overlay
├── README.md                              — product summary
├── Cargo.toml                             — workspace root
├── vox.tokens.json                        — design token SSOT (tiny today)
├── Vox.toml                               — workspace configuration
├── crates/
│   ├── vox-compiler/                      — monolith: lexer, parser, HIR, typeck, codegen
│   │   └── src/
│   │       ├── hir/nodes/                 — decl.rs, stmt_expr.rs, types.rs
│   │       ├── web_ir/                    — mod.rs, validate.rs, lower.rs, nodes/
│   │       └── codegen_ts/                — TSX emission from Web IR
│   ├── vox-orchestrator/                  — agent dispatch, MCP tools, HTTP gateway
│   │   └── src/mcp_tools/http_gateway/    — Axum gateway
│   ├── vox-cli/                           — `vox` binary entry points
│   │   └── src/commands/                  — CLI subcommand dispatch
│   ├── vox-dashboard/                     — Axum-served SPA (crate added in df1d6919)
│   │   ├── src/                           — Rust: lib.rs, router.rs, assets.rs
│   │   ├── src/App.tsx                    — hand-written React (Phase 7 will retire this)
│   │   ├── src/components/*.tsx           — panel components (Phase 7 will retire)
│   │   ├── package.json                   — Vite 6 + React 19 + Tailwind 3
│   │   └── vite.config.ts
│   ├── vox-lsp/                           — tower-lsp server (editor-agnostic)
│   ├── vox-secrets/                       — secret resolution SSOT
│   ├── vox-crypto/                        — cryptography SSOT
│   ├── vox-skills/                        — skill + MCP tool registry
│   ├── vox-gamify/                        — gamification (formerly vox-ludus)
│   ├── vox-scientia/                      — RAG / knowledge curation
│   ├── vox-db/                            — Codex / Arca Vault / Turso bindings
│   └── vox-actor-runtime/                       — process primitives, telemetry
├── apps/editor/vox-vscode/                            — VS Code extension (to be shrunk)
├── contracts/
│   ├── mcp/tool-registry.canonical.yaml   — MCP tool SSOT (247 tools)
│   ├── operations/catalog.v1.yaml         — operations catalog
│   └── terminal/exec-policy.v1.yaml       — terminal exec policy
├── examples/golden/                       — 44 canonical .vox files
├── scripts/                               — .vox automation scripts
├── docs/
│   └── src/
│       ├── adr/                           — ADRs 001-023 (next will be 024)
│       ├── architecture/                  — research + SSoT docs (ALL new research goes here)
│       ├── contributors/
│       ├── ci/
│       ├── how-to/
│       ├── reference/
│       └── tutorials/
└── tests/                                 — ad-hoc test .vox files
```

### Global verification commands

Run these before committing any task unless the task overrides them:

```bash
# Fast compile check (<30s on warm cache)
cargo check --workspace --all-features

# Full build (first time ~5 min, incremental ~30s)
cargo build --workspace --all-features

# Tests (whole workspace)
cargo test --workspace --all-features

# Repository-wide policy gates
vox ci toestub-scoped --report
vox ci secret-env-guard
vox ci secrets-parity
vox ci sync-ignore-files

# Lint + format
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Doctests for .vox code blocks
vox doc-pipeline --mode check

# VS Code extension (only if you touched apps/editor/vox-vscode/)
cd apps/editor/vox-vscode && npm run compile && npm run lint
```

If a task adds new code, also run:

```bash
cargo test -p <crate-you-touched> -- --nocapture
```

### Escalation triggers (STOP and ask)

STOP and request operator input when:
- A required file is missing or a path listed in this roadmap does not exist.
- A test fails for reasons not listed in the task's "Known issues" section.
- You find a file that appears to already implement the task's work.
- A task's acceptance criteria conflict with another task you've already
  completed.
- You would need to disable a CI gate to make the build pass.
- You would need to introduce a new dependency not already in `Cargo.toml`.
- You would need to touch `archive/`, `docs/src/archive/`, or a retired crate.

Escalation format: see Appendix C.

---

## Glossary

| Term | Meaning |
|------|---------|
| **HIR** | High-level IR. Crate: `vox-compiler`. Path: `crates/vox-compiler/src/hir/`. Sits between parse and typed lowering. |
| **Web IR** | Second-stage IR specific to UI emission. Path: `crates/vox-codegen/src/web_ir/`. Lowers to TSX via `codegen_ts`. |
| **Path B** | Legacy UI model: decorator-on-fn component syntax (Path B). Retired per AGENTS.md but HIR fields still exist. Phase 2 removes them. |
| **Path C** | Current UI model: `component Name() { state; view }`. Replaces Path B. |
| **Secrets** | Secret resolution crate. Path: `crates/vox-secrets/`. Call site: `vox_secrets::resolve_secret(SecretId::...)`. |
| **MENS** | Model training pipeline. Native Rust (Burn + Candle). Trains on `.vox` corpus + golden set. |
| **Populi** | Hardware-aware node mesh. Routes training/inference to capable nodes. |
| **Ludus** | Gamification system. Formerly `vox-ludus` (renamed). Path: `crates/vox-gamify/`. |
| **Orchestrator** | Agent dispatcher. MCP control surface. Path: `crates/vox-orchestrator/`. |
| **Scientia** | RAG + knowledge curation. Path: `crates/vox-scientia/`. |
| **Socrates** | Anti-hallucination guards. Exposed via chat-meta. |
| **Arca Vault** | Durable workflow journal backend. |
| **Codex** | Workspace journey database. Turso/SQLite. |
| **MCP** | Model Context Protocol. The orchestrator exposes ~247 tools over MCP. |
| **LSP** | Language Server Protocol. Vox's `vox-lsp` is editor-agnostic. |
| **HTTP gateway** | Optional Axum-served API + WS surface. Path: `crates/vox-orchestrator/src/mcp_tools/http_gateway/`. |
| **Dashboard** | Local SPA. Path: `crates/vox-dashboard/`. Added in commit `df1d6919`. |
| **TOESTUB** | Detector family for skeletons, god objects, sprawl, dry violations. Run via `vox ci toestub-scoped`. |
| **Island** | Interactive component boundary; generated as hydration island. `@island` decorator. |
| **Token file** | `vox.tokens.json` at repo root. Design tokens SSOT (minimal today). |

---

<a id="phase-0--dashboard-safety"></a>
## Phase 0 — Dashboard safety (THIS WEEK)

> **Rationale.** Commit `df1d6919` shipped a working dashboard but with a
> security model that is unsafe against DNS rebinding and clickjacking,
> a CLI launcher that leaks processes, and client code with hooks-rules
> violations. These are **correctness and safety bugs, not strategic choices**.
> Fix before any further migration work.

### TASK-0.1 — File ADR 024: Dashboard as local Axum-served SPA

**Phase**: 0 (documentation — start here while Phase 0.2+ are being worked).
**Estimated effort**: 2 hours.
**Preconditions**: None.
**Blocks**: Nothing (parallel).

**Why**: The dashboard migration landed as a research note
(`docs/src/architecture/dashboard-migration-research-2026.md`) but a
structural change of this size deserves a formal ADR. Future contributors
will look for the decision record in `docs/src/adr/` and not find it.

**Files to read first**:
- `docs/src/adr/README.md` — ADR template conventions.
- `docs/src/adr/010-tanstack-web-spine.md` — closest sibling ADR; use as a
  style reference.
- `docs/src/adr/index.md` — index that must be updated.
- `docs/src/architecture/dashboard-migration-research-2026.md` — the research
  this ADR ratifies.
- Git log for commit `df1d6919` to understand what actually landed.

**Files to create**:
- `docs/src/adr/024-dashboard-axum-spa.md`

**Files to modify**:
- `docs/src/adr/index.md` — add entry for ADR 024.
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.

**Step-by-step work**:
1. Create `docs/src/adr/024-dashboard-axum-spa.md` with frontmatter matching
   ADR 010's shape (`title`, `description`, `category`, `last_updated`,
   `training_eligible`, `schema_type`).
2. Status: `Accepted`. Date: today (2026-04-23).
3. Sections:
   - **Context** — summarize the options considered (keep in VS Code webview,
     Tauri, Electron, native Rust GUI, local Axum-served SPA) and why the
     Axum SPA path was chosen. Reference the conversation evidence: 237
     `vscode.*` API references across 19 files, 247 MCP tools in the control
     surface, pre-existing Vite/TanStack commitment from ADR 010.
   - **Decision** — `crates/vox-dashboard` is the canonical home for the
     orchestration UI, mounted into `http_gateway` under
     `#[cfg(feature = "dashboard")]`, served at `/dashboard` on the same
     origin as `/v1/*`. Assets are compile-time embedded via `include_dir!`.
     `vox dashboard` is the CLI entry point; optional `--app` flag wraps in
     Chromium `--app=` mode.
   - **Rejected alternatives** — Tauri (>2 min build times hostile to dev
     iteration), Electron (Node runtime contradicts Rust-first direction),
     bundled VS Code (licensing + payload), native Rust GUI (no React reuse).
   - **Consequences** — extension shrinks to LSP + inline + "open dashboard"
     command; LSP-capable editors (Neovim, Helix, Zed, IntelliJ) get language
     support for free; any browser can access the dashboard; no
     runtime-level JS dependency.
   - **References** — link ADR 010, ADR 012, and the research note.
4. Update `docs/src/adr/index.md` to list ADR 024 after ADR 023.
5. Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.

**Verification commands**:
```bash
# Doc linkcheck
vox doc-pipeline --mode linkcheck docs/src/adr/024-dashboard-axum-spa.md

# Markdown lint
markdownlint docs/src/adr/024-dashboard-axum-spa.md
```

**Acceptance criteria**:
- ADR file exists with the sections above.
- `docs/src/adr/index.md` lists ADR 024.
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
- No markdown lint errors.

**Do NOT**:
- Alter existing ADRs.
- Move the research note; it stays as a superseded-by link.

---

### TASK-0.2 — Replace loopback-auto-unauth with localhost token auth

**Phase**: 0.
**Estimated effort**: 1-2 days.
**Preconditions**: None.
**Blocks**: TASK-0.3 depends on this.

**Why**: Current code at
`crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs:208-213` auto-allows
unauthenticated access when bind_host is 127.0.0.1 and `VOX_DASHBOARD_ENABLED=1`.
This is exploitable via DNS rebinding — any website the user visits can
issue `fetch('http://127.0.0.1:3921/v1/tools/call', ...)` and drive `vox_emergency_stop`
or any of the 32 tools in `DEFAULT_ALLOWED_TOOLS` (see `mod.rs:49-82`).
The correct pattern (Jupyter, Docker Desktop, Temporal Web) is a random
token generated at startup and required on every request.

**Files to read first**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs` — entire file,
  especially lines 180-260 (gateway state construction) and 380-420 (auth
  check).
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/status.rs` — to
  understand how `auth_required` is surfaced.
- `crates/vox-secrets/src/spec/` — secret spec shape.
- `crates/vox-dashboard/src/assets.rs` — current asset handler (no token
  injection).
- `crates/vox-dashboard/src/transport.ts` — current fetch / WS code.

**Files to modify**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs`
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/status.rs`
- `crates/vox-dashboard/src/assets.rs`
- `crates/vox-dashboard/src/transport.ts`
- `crates/vox-dashboard/src/App.tsx` (read token from meta tag on boot)

**Files to create**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/token.rs` — token
  generation + persistence.

**Step-by-step work**:

1. In the new `token.rs`:
   - Add a `DashboardToken(pub String)` newtype around a 32-byte URL-safe
     base64 token (use `rand::rngs::OsRng` + `base64::URL_SAFE_NO_PAD`).
   - `DashboardToken::generate_or_load(state_dir: &Path) -> Result<Self>`:
     - Build a path `state_dir.join("dashboard.token")`.
     - If the file exists and is less than 30 days old, read it.
     - Otherwise generate a new 32-byte token, write with mode 0600 (Unix)
       or equivalent ACL (Windows — use `std::fs::OpenOptions` with
       `access_mode(0o600)` on Unix and a separate Windows branch using
       `windows_permissions`). Set atime+mtime to now.
     - Return the token.
   - Cover with unit tests.

2. In `mod.rs`:
   - At the top of `build_gateway_state` (or wherever the GatewayState is
     constructed around line 200), after `bearer_token`/`read_bearer_token`
     resolution, compute a `dashboard_token` via
     `DashboardToken::generate_or_load(&state_dir_for_repo(&repo_id))`.
   - Add `dashboard_token: Option<DashboardToken>` field to `GatewayState`.
   - **Remove** the auto-unauth block at lines 208-213.
   - Update the auth check function (`check_auth`, near line 390) to accept
     the dashboard token as equivalent to `bearer_token`, but ONLY when the
     request arrives on loopback AND an Origin header check passes
     (TASK-0.3 adds the Origin check).
   - Keep the `VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED=1` secret path intact for
     operator-level override; log a WARN when active.

3. In `assets.rs`:
   - Change `serve_asset` so that when the requested path resolves to
     `index.html` (i.e., the SPA shell), it reads the current
     `DashboardToken`, splices a `<meta name="vox-bearer" content="..."/>`
     into the `<head>` via simple string replace on `</head>`.
   - Mark the response `Cache-Control: no-store` for the token-injected
     HTML. Other assets keep no cache headers for now (TASK-1.3 adds ETag).

4. In `transport.ts`:
   - On module load (before `voxTransport.connect()` is ever called), read
     `document.querySelector('meta[name="vox-bearer"]')?.getAttribute('content')`.
     Store in a module-level `BEARER: string | null`.
   - `connect()`: append `?token=${encodeURIComponent(BEARER)}` to `wsUrl`
     if BEARER is non-null. Also send an initial auth frame as first
     message: `JSON.stringify({type: 'auth', token: BEARER})`.
   - `callTool()`: set `Authorization: Bearer ${BEARER}` header when BEARER
     is non-null.
   - Surface a visible banner via a new `authStatus` event when the server
     replies 401.

5. In `mod.rs` websocket handler (around lines 450+, file `ws.rs`):
   - Accept either `Authorization: Bearer ...` header on the upgrade OR a
     `?token=...` query parameter OR a first-message `{"type":"auth",...}`
     frame. Compare against `dashboard_token`. Close with code 4401 on
     mismatch.
   - Do not accept query-string tokens unless on loopback.

6. Update integration tests to use the new token path.

**Verification commands**:
```bash
cargo test -p vox-orchestrator http_gateway
cargo clippy -p vox-orchestrator --all-targets -- -D warnings

# Manual smoke test (record output in PR description):
#   1. export VOX_MCP_HTTP_ENABLED=1 VOX_DASHBOARD_ENABLED=1
#   2. cargo run -p vox-cli --features dashboard -- dashboard --no-open &
#   3. curl -i http://127.0.0.1:3921/v1/info    # expect 401 without token
#   4. TOKEN=$(cat ~/.local/state/vox/dashboard.token)
#   5. curl -i -H "Authorization: Bearer $TOKEN" http://127.0.0.1:3921/v1/info  # expect 200
```

**Acceptance criteria**:
- `GET /v1/info` without credentials on loopback returns 401.
- `GET /v1/info` with the dashboard token returns 200.
- The SPA's `index.html` response contains `<meta name="vox-bearer"`.
- `Cache-Control: no-store` set on index.html.
- Token file created with 0600 perms (Unix) or equivalent ACL (Windows).
- `VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED=1` still works for operator override
  and logs a WARN.
- Unit tests cover token generation, reuse, rotation, and corrupted-file
  recovery.

**Known issues**:
- Windows file ACL setting may require the `windows-acl` or `winapi` crate.
  Keep the dependency isolated to `token.rs` behind `cfg(windows)`.

**Do NOT**:
- Commit the token file or add it to the repo.
- Use `env::var("HOME")` directly; use `directories-next` or similar.

---

### TASK-0.3 — Add strict Origin/Host allowlist middleware

**Phase**: 0.
**Estimated effort**: 4-6 hours.
**Preconditions**: TASK-0.2.
**Blocks**: TASK-0.4.

**Why**: Token auth alone does not stop DNS rebinding — the attacker's
browser still sends the token (stored in a cookie or meta tag) because the
target origin matches `127.0.0.1`. The mitigation is a strict `Origin` /
`Host` header check that rejects anything not matching the server's
configured bind address.

**Files to read first**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs` — where
  middleware would land.
- Axum 0.8 tower middleware docs (workspace already uses axum; check current
  version in `Cargo.toml`).

**Files to modify**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs`

**Files to create**:
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/origin_guard.rs`

**Step-by-step work**:

1. In `origin_guard.rs`:
   - Define `OriginAllowlist { allowed: Vec<HostPort> }`.
   - Construct from `bind_host` + `bind_port`; include both `127.0.0.1:<port>`
     and `localhost:<port>` when bind_host is loopback.
   - Export an `axum::middleware::from_fn_with_state` function
     `origin_guard_middleware` that inspects `Host` and `Origin` headers:
     - If `Origin` is present, it must match one of the allowed host:port
       pairs exactly (scheme ignored for loopback, or both `http://` and
       `https://` allowed).
     - If `Origin` is absent (non-browser client), `Host` header must match.
     - On WebSocket upgrade requests, the `Origin` check is strict; no
       exceptions.
   - Reject with HTTP 403 and a short JSON body `{"error":"origin_denied"}`.

2. In `mod.rs`, install the middleware between the router and the body-limit
   layer around line 272-274:
   ```rust
   let app = app
       .layer(axum::middleware::from_fn_with_state(
           origin_allowlist.clone(),
           origin_guard::origin_guard_middleware,
       ))
       .layer(DefaultBodyLimit::max(256 * 1024))
       .with_state(gateway_state.clone());
   ```

3. Unit tests covering:
   - Origin matches → 200.
   - Origin is `http://evil.com` → 403.
   - No Origin, Host matches → 200.
   - No Origin, Host is `127.0.0.1:9999` (wrong port) → 403.
   - WebSocket upgrade with wrong Origin → upgrade rejected.

**Verification commands**:
```bash
cargo test -p vox-orchestrator http_gateway::origin_guard
cargo clippy -p vox-orchestrator --all-targets -- -D warnings
```

**Acceptance criteria**:
- All unit tests pass.
- The middleware is mounted in the gateway router.
- Non-matching Origin requests return 403 before reaching any tool handler.

**Do NOT**:
- Apply the allowlist to public-eval routes if any are explicitly declared
  public via `VOX_MCP_HTTP_PUBLIC_EVAL_ENABLED` — those are covered by
  rate-limit + sandbox, not origin.

---

### TASK-0.4 — Add CSP, X-Frame-Options, Referrer-Policy, and CORS layer

**Phase**: 0.
**Estimated effort**: 3-4 hours.
**Preconditions**: TASK-0.3.
**Blocks**: Nothing.

**Why**: Without Content-Security-Policy, the SPA can be iframed and
clickjacked to issue `vox_emergency_stop` on behalf of the logged-in user.
Without `X-Frame-Options`, nothing stops the iframe. The CORS feature on
`tower-http` was enabled in the crate's Cargo.toml but no CorsLayer is
actually installed.

**Files to modify**:
- `crates/vox-dashboard/src/assets.rs`
- `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs`

**Step-by-step work**:

1. In `assets.rs`, for HTML responses, add:
   - `Content-Security-Policy`:
     `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ws://127.0.0.1:* wss://127.0.0.1:*; frame-ancestors 'none'; object-src 'none'; base-uri 'self';`
     (Adjust `'unsafe-inline'` only if Tailwind-compiled styles require it;
     the Vite build typically does not.)
   - `X-Frame-Options: DENY`
   - `Referrer-Policy: no-referrer`
   - `X-Content-Type-Options: nosniff`

2. For JS/CSS assets, add only `X-Content-Type-Options: nosniff` and
   `Cache-Control: public, max-age=31536000, immutable` (matches Vite's
   hashed filenames).

3. In `mod.rs`, install a `CorsLayer::new()` with:
   - Allowed origins: loopback only, matching the OriginAllowlist from
     TASK-0.3.
   - Allowed methods: GET, POST, OPTIONS.
   - Allowed headers: `Content-Type`, `Authorization`.
   - Max age: 3600s.
   - `allow_credentials(true)` since we send the bearer via header on
     same-origin fetches.

4. Add integration tests:
   - `GET /dashboard` response has `X-Frame-Options: DENY`.
   - `GET /dashboard` response has `Content-Security-Policy` containing
     `frame-ancestors 'none'`.
   - `OPTIONS /v1/tools/call` responds with CORS headers matching allowlist.

**Verification commands**:
```bash
cargo test -p vox-dashboard assets
cargo test -p vox-orchestrator http_gateway
```

**Acceptance criteria**:
- All three response-header requirements surfaced in integration tests.
- CorsLayer installed and enforces the same origin set as the middleware.
- Browser inspection of `GET /dashboard` shows the expected headers.

---

### TASK-0.5 — Fix `vox dashboard` CLI detachment + readiness polling

**Phase**: 0.
**Estimated effort**: 6-8 hours.
**Preconditions**: None.
**Blocks**: Nothing.

**Why**: Current code at `crates/vox-cli/src/commands/dashboard.rs` has three
bugs: (1) prints `VOX_DASHBOARD_READY` before the child has bound, (2) uses
`tokio::time::sleep(Duration::from_secs(3600))` as a fake detachment, (3) on
Unix the child inherits stdio and dies on SIGHUP; on Windows the child
shares the console.

**Files to read first**:
- `crates/vox-cli/src/commands/dashboard.rs` — full file (59 lines).
- `crates/vox-cli/src/process_supervision.rs` (if exists) — existing
  daemon-spawn helpers.
- `crates/vox-orchestrator-d/src/main.rs` — understand what the orchestrator
  daemon logs when ready.

**Files to modify**:
- `crates/vox-cli/src/commands/dashboard.rs`
- `crates/vox-cli/Cargo.toml` (add `reqwest` with `json`, if not already
  present via workspace).

**Step-by-step work**:

1. Replace the single spawn+sleep with a `DashboardLauncher` struct:

   ```rust
   struct DashboardLauncher {
       port: u16,
       open: bool,
       app_mode: bool,
       daemon_path: PathBuf,
   }
   ```

2. Add `launch()` method:
   a. Spawn the orchestrator daemon with stdio redirected to a file at
      `$VOX_STATE_DIR/dashboard.log`.
   b. Unix: call `setsid()` via a `pre_exec` closure so the child starts a
      new session and won't receive SIGHUP when the CLI exits. (Use
      `std::os::unix::process::CommandExt::pre_exec`.)
   c. Windows: set the `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` flags
      via `std::os::windows::process::CommandExt::creation_flags`. Constants
      are `0x00000200` and `0x00000008`.
   d. Write the child PID to `$VOX_STATE_DIR/dashboard.pid`.
   e. Poll `GET http://127.0.0.1:<port>/health` every 250ms for up to 10s.
      On success, proceed; on timeout, print the last 50 lines of the log
      and error out.
   f. On poll success, if `open`, launch the browser in the appropriate
      mode.

3. Add a companion `vox dashboard stop` subcommand that reads the PID file,
   sends SIGTERM (Unix) or TerminateProcess (Windows), waits 5s, SIGKILLs if
   needed, removes PID file.

4. Add `--foreground` flag that skips detachment and runs the daemon
   in-process (useful for debugging).

**Verification commands**:
```bash
cargo test -p vox-cli commands::dashboard
cargo build -p vox-cli --features dashboard
# Manual test:
#   vox dashboard --no-open
#   vox dashboard stop
```

**Acceptance criteria**:
- `vox dashboard` prints the URL only after `GET /health` returns 200.
- The daemon survives the CLI exiting (verify with `ps` / Task Manager).
- `vox dashboard stop` kills the daemon cleanly.
- PID file cleanup on stop.

---

### TASK-0.6 — Harden `transport.ts`: onerror, backoff, auth refresh

**Phase**: 0.
**Estimated effort**: 3-4 hours.
**Preconditions**: TASK-0.2 (bearer injection).

**Why**: Current `crates/vox-dashboard/src/transport.ts` has no `onerror`
handler, reconnects every 2s with no cap, never attaches `Authorization`, and
can drop `'unknown'`-typed messages silently.

**Files to read first**:
- `crates/vox-dashboard/src/transport.ts`
- `crates/vox-dashboard/src/App.tsx` lines 69-144 (usage site).

**Files to modify**:
- `crates/vox-dashboard/src/transport.ts`
- `crates/vox-dashboard/src/App.tsx` (subscribe to new `authStatus` and
  `connectionStatus` events; render a banner when disconnected or
  unauthorized).

**Step-by-step work**:

1. Add `connectionStatus` and `authStatus` event types to the internal
   listener map.
2. On `onopen`, emit `connectionStatus: 'open'` and reset the backoff
   counter.
3. On `onerror`, emit `connectionStatus: 'error'` with the error message.
4. On `onclose` with code 4401 (unauthorized), emit
   `authStatus: 'unauthorized'` and DO NOT reconnect — surface a banner to
   the user asking them to re-open the dashboard with a fresh token.
5. On other close codes, reconnect with exponential backoff: 250ms, 500ms,
   1s, 2s, 5s, 10s, 30s, 30s, 30s…
6. Track attempt count and surface as `reconnectAttempt: number` on the
   `connectionStatus` events.
7. On every message that fails to match a known type, log to
   `console.warn` AND emit a `transportError` event the UI can surface.
8. `callTool`: always set `Authorization: Bearer ${BEARER}` when BEARER is
   set. On response `status === 401`, emit `authStatus: 'unauthorized'` and
   reject the promise.
9. On module load, if `BEARER` is null, emit `authStatus: 'no_token'` so
   the UI can show a prominent error.

**Verification commands**:
```bash
cd crates/vox-dashboard && pnpm run build
cd crates/vox-dashboard && pnpm run test  # if test config exists; add if not
```

**Acceptance criteria**:
- `transport.ts` has no `any` in event type names (use a discriminated
  union).
- UI renders a banner when `connectionStatus === 'error'` or
  `authStatus !== 'authorized'`.
- Reconnect backoff visible in network tab under induced failure.

---

### TASK-0.7 — Fix `App.tsx` hooks violation + dead imports

**Phase**: 0.
**Estimated effort**: 2 hours.
**Preconditions**: None (but coordinate with TASK-0.6 for merge order).

**Why**: `crates/vox-dashboard/src/App.tsx:70` calls `useVoxTransport()`
inside a `useEffect`, which violates Rules of Hooks. `createRoot` is
imported but never used in App.tsx (it's used in `main.tsx`).
`Math.random().toString(36).substr(2, 9)` uses deprecated `substr`.

**Files to modify**:
- `crates/vox-dashboard/src/App.tsx`
- `crates/vox-dashboard/.eslintrc.json` (create if not present) — enable
  `react-hooks/rules-of-hooks` and `react-hooks/exhaustive-deps`.

**Step-by-step work**:

1. Move `const transport = useVoxTransport();` to the top of `function App()`
   before any hook.
2. Remove the unused `createRoot` import.
3. Replace `substr(2, 9)` with `slice(2, 11)`.
4. Add `eslint-plugin-react-hooks` to `package.json` if not already present.
5. Configure ESLint to fail on violations.
6. Run eslint --fix and commit resulting auto-fixes.

**Verification commands**:
```bash
cd crates/vox-dashboard && pnpm run lint
cd crates/vox-dashboard && pnpm run build
```

**Acceptance criteria**:
- ESLint reports zero errors.
- `useVoxTransport()` is at the top of the component.
- `pnpm run build` succeeds.

---

### TASK-0.8 — Add integration tests for the dashboard crate

**Phase**: 0.
**Estimated effort**: 1 day.
**Preconditions**: TASK-0.2, TASK-0.3, TASK-0.4.

**Why**: `crates/vox-dashboard/` has zero tests. At minimum the security
fixes from 0.2–0.4 need regression coverage.

**Files to create**:
- `crates/vox-dashboard/tests/asset_serving.rs`
- `crates/vox-dashboard/tests/auth.rs`
- `crates/vox-dashboard/tests/origin_guard.rs`

**Step-by-step work**:

1. `asset_serving.rs`:
   - Boot a test router via `dashboard_router()`.
   - Assert `GET /dashboard` returns 200, `Content-Type: text/html`, and
     body contains `<meta name="vox-bearer"`.
   - Assert `GET /dashboard/nonexistent.js` falls back to index.html
     (SPA behavior).
   - Assert `GET /dashboard/` redirects or returns index.html.

2. `auth.rs`:
   - Boot a test gateway + dashboard router.
   - Assert `GET /v1/info` without credentials → 401.
   - Assert `GET /v1/info` with a valid token → 200.
   - Assert `GET /v1/info` with an invalid token → 401.
   - Assert WebSocket upgrade without token → 1008 close.

3. `origin_guard.rs`:
   - Assert request with `Origin: http://evil.com` → 403.
   - Assert request with matching `Origin` → 200.
   - Assert request without `Origin` but matching `Host` → 200.

**Verification commands**:
```bash
cargo test -p vox-dashboard --all-features
```

**Acceptance criteria**:
- All three test files pass.
- Each file has at least 3 independent test cases.

---

<a id="phase-1--dashboard-cleanup"></a>
## Phase 1 — Dashboard cleanup

### TASK-1.1 — Delete the `vscode.ts` shim and rewrite component callsites

**Phase**: 1.
**Estimated effort**: 1 day.
**Preconditions**: Phase 0 complete.

**Why**: `crates/vox-dashboard/src/utils/vscode.ts` is a 44-line shim that
translates legacy `vscode.postMessage({type: '…'})` calls into
`voxTransport.callTool(...)`. It exists only because the ported components
still pretend they're in a VS Code webview. It silently drops unknown
message types (`console.warn`), preserves a meaningless abstraction boundary,
and misleads readers by being named `vscode.ts`.

**Files to read first**:
- `crates/vox-dashboard/src/utils/vscode.ts` — the shim.
- `crates/vox-dashboard/src/App.tsx` — primary caller of `vscode.postMessage`.
- Every file under `crates/vox-dashboard/src/components/` — grep for
  `postMessage` and `vscode.`.

**Files to modify**:
- Every component under `crates/vox-dashboard/src/components/` that calls
  `vscode.postMessage`.
- `crates/vox-dashboard/src/App.tsx`.

**Files to delete**:
- `crates/vox-dashboard/src/utils/vscode.ts`.

**Step-by-step work**:

1. Grep `grep -rn "vscode.postMessage" crates/vox-dashboard/src/` to
   enumerate callsites.
2. For each callsite, replace:
   ```ts
   vscode.postMessage({type: 'agentPause', agentId: id});
   ```
   with:
   ```ts
   voxTransport.callTool('vox_pause_agent', { agent_id: id });
   ```
   using the mapping table from the old shim as the reference.
3. Remove `import { getVsCodeApi } from './utils/vscode'` imports.
4. Remove any `const vscode = getVsCodeApi()` lines.
5. For `pickModel` and other message types that don't have a direct MCP tool,
   introduce a local handler in `App.tsx` that calls the appropriate MCP
   tool or raises a dev-time console error for genuinely unhandled cases.
6. Delete `crates/vox-dashboard/src/utils/vscode.ts`.
7. Update `App.tsx` to remove `getVsCodeApi` / `const vscode = ...` and use
   `voxTransport` directly.

**Verification commands**:
```bash
cd crates/vox-dashboard && pnpm run lint
cd crates/vox-dashboard && pnpm run build
grep -rn "vscode.postMessage\|getVsCodeApi" crates/vox-dashboard/src/ && echo "LEAKS FOUND" || echo "OK"
```

**Acceptance criteria**:
- `grep` finds zero `vscode.postMessage` or `getVsCodeApi` references.
- `pnpm run build` succeeds.
- Every message-type previously handled by the shim now has either a direct
  MCP tool call or an explicit local handler.

**Do NOT**:
- Leave the shim with deprecation notices — delete it cleanly.
- Introduce a new shim to preserve the abstraction.

---

### TASK-1.2 — Fix or delete the `vox-dashboard-d` standalone binary

**Phase**: 1.
**Estimated effort**: 4-8 hours depending on option chosen.
**Preconditions**: None.

**Why**: `crates/vox-dashboard/src/bin/vox_dashboard_d.rs` mounts only
`dashboard_router()`, which serves `/dashboard*` but not `/v1/ws` or
`/v1/tools/call`. The SPA it serves makes requests to `/v1/*` via
`window.location.host`, so if run standalone it results in a blank
non-functional SPA.

**Decision required from operator**: Choose option A or B before starting.

#### Option A — Delete the binary (recommended).

1. Delete `crates/vox-dashboard/src/bin/vox_dashboard_d.rs`.
2. Remove the `[[bin]]` entry from `crates/vox-dashboard/Cargo.toml`.
3. Update any docs references in
   `docs/src/architecture/dashboard-migration-research-2026.md` to remove
   "standalone binary fallback" language.

#### Option B — Make the binary actually work.

1. Add `vox-orchestrator` as an optional dep in
   `crates/vox-dashboard/Cargo.toml` under a new feature `with-orchestrator`.
2. In `vox_dashboard_d.rs`, boot a `ServerState::new_for_daemon(...)` and
   call `spawn_http_gateway_if_enabled(state)` before mounting the
   dashboard router. Reuse the existing `VoxOrchestratorDaemonSocket` story.
3. Expand the test suite from TASK-0.8 to cover the standalone binary path.

**Verification commands** (either option):
```bash
cargo build --workspace --all-features
cargo test --workspace
```

**Acceptance criteria**:
- If Option A: no `bin` entry, no orphan `main.rs`, docs updated.
- If Option B: `vox-dashboard-d` starts, binds, serves both `/dashboard` and
  `/v1/*`, and passes the TASK-0.8 test suite.

---

### TASK-1.3 — Add `build.rs` for `include_dir!` safety + ETag support

**Phase**: 1.
**Estimated effort**: 4 hours.

**Why**: `crates/vox-dashboard/src/assets.rs:9` expands `include_dir!` at
compile time. On a clean clone without `dist/` (i.e., before `pnpm run
build`), the macro fails with a confusing error. Also, serving embedded
assets with no `Cache-Control` or `ETag` means every SPA reload re-downloads
~2MB of bundled JS.

**Files to create**:
- `crates/vox-dashboard/build.rs`
- `crates/vox-dashboard/dist/.gitkeep` (with a placeholder index.html if
  desired — see below).

**Files to modify**:
- `crates/vox-dashboard/Cargo.toml` (add `build = "build.rs"`).
- `crates/vox-dashboard/src/assets.rs` (compute ETag, honor
  `If-None-Match`).

**Step-by-step work**:

1. `build.rs`:
   - If `dist/` does not exist or `dist/index.html` does not exist:
     - Print `cargo:warning=dist/ missing; run `pnpm install && pnpm run
       build` in crates/vox-dashboard before building with
       --features embedded-assets`.
     - Create `dist/index.html` with a minimal placeholder HTML that says
       "Dashboard bundle not built." so the `include_dir!` macro still
       succeeds.

2. `assets.rs`:
   - At the top, compute an `ETAG_PREFIX` constant from `env!("CARGO_PKG_VERSION")`
     and the compile-time hash of the embedded dist (use
     `const_fnv1a_hash::fnv1a_hash_64` over every file's bytes).
   - For each file, compute a per-file ETag as `"<ETAG_PREFIX>-<path>-<size>"`.
   - On each request, check `If-None-Match` header; if matches, return 304
     with no body.
   - Set `Cache-Control`:
     - `/dashboard/` or any path resolving to index.html: `no-store` (because
       of the token meta tag).
     - Everything else: `public, max-age=31536000, immutable`.

**Verification commands**:
```bash
# Clean-clone simulation
rm -rf crates/vox-dashboard/dist
cargo build -p vox-dashboard --features embedded-assets    # expect warning, not error
cargo test -p vox-dashboard
```

**Acceptance criteria**:
- `cargo build` succeeds on a fresh clone without running pnpm first.
- ETag roundtrip works: second request returns 304.
- `Cache-Control` header differs between index.html and hashed assets.

---

### TASK-1.4 — Clean up `index.css` duplication and stale comments

**Phase**: 1.
**Estimated effort**: 3 hours.

**Why**: `crates/vox-dashboard/src/index.css` lines 79-240 reinvent Tailwind
as hand-rolled utility classes. The comment at line 79 says "since we can't
easily add tailwind to esbuild without postcss" — but the crate *does* ship
`tailwind.config.js` and `postcss.config.js`. Ship one or the other, not
both.

**Files to modify**:
- `crates/vox-dashboard/src/index.css`
- Possibly `crates/vox-dashboard/tailwind.config.js`

**Step-by-step work**:

1. Verify Tailwind is actually built: `cd crates/vox-dashboard && pnpm run
   build` then `grep -c "flex-direction" dist/assets/*.css`. Should be
   non-zero.
2. If Tailwind works: delete lines 79-240 of `index.css` (the reinvented
   utility block). Keep the `@tailwind` directives, CSS variables,
   `@keyframes`, and `.chat-markdown` / `.shiki-block` class rules.
3. If Tailwind does not work: STOP and escalate — the build is broken
   and this task exceeds scope.

**Verification commands**:
```bash
cd crates/vox-dashboard && pnpm run build
# Open dist/index.html in a browser (or headlessly via playwright if available)
# and confirm the dashboard renders correctly.
```

**Acceptance criteria**:
- `index.css` no longer contains the "since we can't easily add tailwind"
  comment.
- No hand-rolled `.flex`, `.p-2`, `.text-white` etc. utility rules.
- Dashboard renders identically before and after (visual diff).

---

### TASK-1.5 — Pin workspace dependencies and remove `tsconfig.tsbuildinfo`

**Phase**: 1.
**Estimated effort**: 1 hour.

**Why**: `crates/vox-dashboard/Cargo.toml` pins `tower-http = "0.6.2"`
directly instead of using `workspace = true`. `tsconfig.tsbuildinfo` was
committed; it's a TS incremental-build artifact that belongs in
`.gitignore`.

**Files to modify**:
- `crates/vox-dashboard/Cargo.toml`
- `Cargo.toml` (workspace — verify `tower-http` and `mime_guess` are in
  `[workspace.dependencies]`, add if missing).
- `.gitignore`

**Files to delete**:
- `crates/vox-dashboard/tsconfig.tsbuildinfo`

**Step-by-step work**:

1. Verify or add to workspace `Cargo.toml`:
   ```toml
   [workspace.dependencies]
   tower-http = { version = "0.6.2", features = ["fs", "cors"] }
   mime_guess = "2.0"
   ```
2. Change `crates/vox-dashboard/Cargo.toml`:
   - `tower-http = { workspace = true }`
   - `mime_guess = { workspace = true }`
3. Delete `crates/vox-dashboard/tsconfig.tsbuildinfo`.
4. Add to `.gitignore` (if not already present):
   ```
   **/tsconfig.tsbuildinfo
   crates/vox-dashboard/dist/
   ```
5. Run `cargo check --workspace` to confirm the workspace pin resolves.

**Verification commands**:
```bash
cargo check --workspace --all-features
```

**Acceptance criteria**:
- `tower-http` and `mime_guess` use workspace pins.
- `tsconfig.tsbuildinfo` is deleted and gitignored.
- Workspace builds.

---

<a id="phase-2--compiler-primitive-collapse"></a>
## Phase 2 — Compiler primitive collapse

> **Rationale.** The audit found 43 Decl variants (20 semantic-core + 23
> migration-era), 29 ExprKind variants (several collapsible), three
> overlapping endpoint decorators (`@server`/`@query`/`@mutation`), three
> grammar shapes for routing declarations, and a Path B UI model that AGENTS.md
> lists as retired but the HIR still carries. Phase 2 finishes the cleanup.
>
> **Coordination**: every task in this phase changes the `.vox` parser and/or
> HIR. Migrate the 44 golden files + 107 total .vox corpus in the same PR
> that changes the parser (see Phase 8 on corpus migration). Do NOT land a
> syntax change without also migrating the corpus.

### TASK-2.1 — Delete Path B UI fields from `HirModule`

**Phase**: 2.
**Estimated effort**: 2-3 days.
**Preconditions**: Phase 1 complete.
**Blocks**: TASK-2.2 through TASK-2.6 in any order.

**Why**: AGENTS.md §Retired Surfaces lists Path B decorator-on-fn component syntax as retired
in favor of `component Name() {}`. But `crates/vox-compiler/src/hir/nodes/decl.rs`
still carries nine Path B fields on `HirModule`: `components`,
`v0_components`, `hooks`, `pages`, `contexts`, `client_routes`, `layouts`,
`loadings`, `error_boundaries`, `not_founds`. Plus a `HirLoweringMigrationFlags`
that switches code paths at runtime. This is dead weight that doubles the
pattern-match surface on every compiler pass and pollutes the MENS training
signal.

**Files to read first**:
- `crates/vox-compiler/src/hir/nodes/decl.rs` — `HirModule` definition.
- `crates/vox-compiler/src/hir/lowering/mod.rs` — where the fields are
  populated.
- `crates/vox-codegen-ts/src/` — where they're consumed.
- `crates/vox-codegen/src/web_ir/lower.rs` — the Path C lowering.
- `examples/golden/` — verify no `.vox` file uses Path B syntax.
- `AGENTS.md` §Retired Surfaces — confirmation the symbols are retired.

**Files to modify**:
- `crates/vox-compiler/src/hir/nodes/decl.rs`
- `crates/vox-compiler/src/hir/lowering/mod.rs`
- `crates/vox-codegen-ts/src/` (every file that reads the deleted
  fields)
- Any other crate that reads `HirModule.components`, `.hooks`, etc.

**Step-by-step work**:

1. Grep the workspace: `rg "\.components\b|\.hooks\b|\.pages\b|\.contexts\b|\.client_routes\b|\.layouts\b|\.loadings\b|\.error_boundaries\b|\.not_founds\b|\.v0_components\b" crates/`. Produce a report of every callsite.
2. For each callsite:
   - If it's a read and the read is now dead code, delete the surrounding
     block or function.
   - If it's a read that fed into Path B codegen, delete the codegen branch
     entirely (Path C is canonical now).
3. Delete the fields from `HirModule`:
   - `components: Vec<ComponentDecl>`
   - `v0_components: Vec<V0ComponentDecl>`
   - `hooks: Vec<HookDecl>`
   - `pages: Vec<PageDecl>`
   - `contexts: Vec<ContextDecl>`
   - `client_routes: Vec<ClientRouteDecl>`
   - `layouts: Vec<LayoutDecl>`
   - `loadings: Vec<LoadingDecl>`
   - `error_boundaries: Vec<ErrorBoundaryDecl>`
   - `not_founds: Vec<NotFoundDecl>`
4. Delete the corresponding Decl struct definitions if no longer referenced.
5. Delete `HirLoweringMigrationFlags` and every code path that reads it.
6. Delete `reactive_components` only if confirmed duplicate with
   `components`; otherwise rename `reactive_components` to `components` for
   clarity.
7. Update the parser: reject Path B decorator-on-fn component syntax with a friendly
   "Path B retired; use `component Name() {}` form (see AGENTS.md)" error.
8. Update any tests that referenced Path B; convert test inputs to Path C.

**Verification commands**:
```bash
cargo check --workspace
cargo test -p vox-compiler
cargo test --workspace
vox ci toestub-scoped --report

# Golden-file regeneration (confirms no .vox file uses Path B)
vox build examples/golden/*.vox

# Coverage: confirm no dead references
rg "HirLoweringMigrationFlags|\.components\b|\.hooks\b" crates/ && echo "LEAKS" || echo "OK"
```

**Acceptance criteria**:
- `HirModule` has ≤25 fields (down from 34).
- `HirLoweringMigrationFlags` is deleted.
- Parser rejects Path B with a helpful error.
- All golden files still compile.
- Workspace builds clean.

**Known issues**:
- If `apps/editor/vox-vscode/` references any of the deleted Decl types through MCP
  schema, regenerate `apps/editor/vox-vscode/src/core/mcpToolRegistry.generated.ts` via
  its existing script.

**Do NOT**:
- Resurrect Path B behind a feature flag. The policy decision is already
  made.

---

### TASK-2.2 — Unify `@server` / `@query` / `@mutation` into `@endpoint(kind: …)`

**Phase**: 2.
**Estimated effort**: 3-4 days.
**Preconditions**: TASK-2.1.

**Why**: Three decorators lower to the same RPC-shaped contract with
different constraints. Unification reduces the decorator surface from 14 to
12 and consolidates three HIR buckets (`query_fns`, `mutation_fns`,
`server_fns`) into one.

**Target syntax** (as of April 2026 — superseded):

> This unification shipped as `@endpoint(kind: …)`, which was then removed in
> v0.6.0 and replaced by the bare keywords `query` / `mutation` / `server`.
> The at-prefixed spellings below are hard parse errors as of 2026-06-30
> (`cd7cc96874`). Kept to record the design rationale, not as a target.

```vox
// vox:skip -- superseded design: @endpoint was removed in v0.6.0
@endpoint(kind: query)    fn recent_tasks() to list[Task] { ... }
@endpoint(kind: mutation) fn add_task(t: NewTask) to Id[Task] { ... }
@endpoint(kind: server)   fn privileged_action() to Result[Unit] { ... }
```

**Files to read first**:
- `crates/vox-compiler/src/hir/nodes/decl.rs` — current `query_fns` /
  `mutation_fns` / `server_fns` fields.
- `crates/vox-compiler/src/parser/` — where the three decorators parse.
- `crates/vox-compiler/src/hir/lowering/` — where they lower.
- `crates/vox-codegen/src/web_ir/lower.rs` lines 493-561 — loader / server
  function contract construction.
- `examples/golden/*.vox` — every file using the three decorators.

**Files to modify**:
- Parser (path varies — use grep to locate handlers for `@query`,
  `@mutation`, `@server`).
- HIR lowering.
- `HirModule` declaration (collapse `query_fns`, `mutation_fns`,
  `server_fns` into `endpoint_fns: Vec<EndpointFn>` with a `kind:
  EndpointKind` field).
- `EndpointFn` struct definition (new).
- `web_ir/lower.rs` (construct single contract variant).
- Every consumer of the three buckets.

**Step-by-step work**:

1. Define `EndpointKind { Query, Mutation, Server }` and `EndpointFn {
   kind: EndpointKind, fn_decl: FnDecl, ... }`.
2. Update parser:
   - Keep parsing the three legacy decorators during a migration window.
   - Emit a deprecation warning with auto-fix suggestion: `@query` →
     `@endpoint(kind: query)`.
   - Parse `@endpoint(kind: …)` as the new canonical form.
3. Lowering: produce `EndpointFn` with the right `kind`. Apply the same
   constraints as before (query = read-only, GET mount; mutation =
   transactional, POST mount; server = unconstrained).
4. Downstream consumers iterate `module.endpoint_fns` and dispatch on
   `.kind`.
5. Corpus migration (coordinate with TASK-8.1): rewrite all 44+ golden
   files to use the new decorator.
6. Update documentation: `docs/src/reference/ref-decorators.md` and any
   related how-to pages.

**Verification commands**:
```bash
cargo check --workspace
cargo test -p vox-compiler
cargo test --workspace
vox build examples/golden/*.vox
vox doc-pipeline --mode check
```

**Acceptance criteria**:
- `@endpoint(kind: …)` parses and lowers.
- Legacy decorators emit warnings (not errors) during the migration window.
- `HirModule` has one endpoint bucket, not three.
- All golden files use the new decorator.
- Docs updated.

**Do NOT**:
- Delete legacy decorators in the same PR — break the migration into two:
  first introduce `@endpoint`, migrate corpus, THEN delete legacy in a
  follow-up.

---

### TASK-2.3 — Collapse `HirExpr::DbTableOp` into `MethodCall`

**Phase**: 2.
**Estimated effort**: 2 days.
**Preconditions**: TASK-2.1.

**Why**: `DbTableOp` sub-universe in `HirExpr` has 7 variants (`Insert`,
`Get`, `Delete`, `All`, `FilterRecord`, `Count`, `UnsafeQueryRawClause`) for
what are essentially method calls on a table value. Collapsing them to
`MethodCall(table, "insert", args)` removes 7 variants and makes the
operation set extensible without AST surgery.

**Files to read first**:
- `crates/vox-compiler/src/hir/nodes/stmt_expr.rs` — `HirExpr::DbTableOp`
  definition (around line 117).
- `crates/vox-compiler/src/hir/lowering/` — where DbTableOp variants are
  constructed.
- `crates/vox-codegen-ts/src/` and codegen_rust — consumers.
- `crates/vox-compiler/src/typeck/` — type-checking paths.

**Files to modify**:
- `hir/nodes/stmt_expr.rs`
- `hir/lowering/`
- Type checker (intercept `MethodCall` when receiver is a table).
- Codegen emitters.

**Step-by-step work**:

1. Add a `TableOp` enum (in `typeck` or a new `hir/nodes/table_op.rs`)
   listing the seven operations for type-checker dispatch.
2. In lowering: instead of constructing `HirExpr::DbTableOp(Insert, ...)`,
   construct `HirExpr::MethodCall(table_expr, "insert", args)`.
3. In the type checker: when a `MethodCall` receiver resolves to a table
   type, look up the operation in `TableOp` and apply its signature /
   constraints. Unknown method on a table → compile error.
4. Codegen: receive `MethodCall` and dispatch on method name for tables.
5. Delete `HirExpr::DbTableOp` variant + `DbOp` nested enum.
6. Keep `UnsafeQueryRawClause` as a distinct construct if it carries
   escape-hatch semantics that `MethodCall` cannot express; put it in a
   dedicated `HirExpr::UnsafeRawSql` or `Expr::Raw` gated by a feature flag.

**Verification commands**:
```bash
cargo check --workspace
cargo test -p vox-compiler
vox build examples/golden/*.vox
```

**Acceptance criteria**:
- `HirExpr` variant count drops by at least 6.
- All golden-file queries still compile and produce identical generated SQL
  / TS.
- Type checker rejects `my_table.nonexistent_op(...)` with a clear error.

---

### TASK-2.4 — Resolve `HirExpr::Pipe` vs `Binary(Pipe)` duplication

**Phase**: 2.
**Estimated effort**: 4 hours.
**Preconditions**: TASK-2.1.

**Why**: `HirExpr::Pipe` exists as both a `Binary` operator AND a standalone
`HirExpr::Pipe` variant.

**Files to read first**:
- `crates/vox-compiler/src/hir/nodes/stmt_expr.rs` around lines 117-260.
- Parser grammar rules for `|>`.

**Files to modify**:
- `hir/nodes/stmt_expr.rs`.
- Parser.
- Lowering consumers.

**Step-by-step work**:

1. Decide: keep `Binary(Pipe)`, delete standalone `HirExpr::Pipe`.
2. Update parser to produce `Binary(Pipe)` for `|>`.
3. Delete standalone variant.
4. Update consumers.

**Verification commands**:
```bash
cargo test -p vox-compiler
vox build examples/golden/*.vox
```

**Acceptance criteria**:
- Only one representation for `|>`.
- Golden files still compile.

---

### TASK-2.5 — Retire `http` bare-keyword routing in favor of `routes { }` + `@endpoint`

**Phase**: 2.
**Estimated effort**: 1-2 days.
**Preconditions**: TASK-2.2.

**Why**: Three grammar shapes for routing (`routes { }` bare block,
`@query`/`@mutation`/`@server` decorator on fn, `http get "/path"` bare
keyword) collapse to two (after TASK-2.2) and then to one if `http`
retires. `routes { }` becomes the single grammar for URL-addressable
declarations; endpoint functions auto-mount via `@endpoint`.

**Files to modify**:
- Parser.
- HIR lowering.
- Any `.vox` in corpus using `http get "/…"`.
- Docs.

**Step-by-step work**:

1. Grep `http\s+get\s+"` across `.vox` corpus.
2. Migrate each to `routes { "/…" to fn_or_component }` form.
3. Remove `http` bare-keyword parsing from the grammar.
4. Parser emits error "`http` keyword retired; use `routes { }` block"
   during migration window.
5. Update docs (`docs/src/reference/`, `docs/src/how-to/`).

**Verification commands**:
```bash
cargo test -p vox-compiler
grep -rn "^http " examples/ scripts/ tests/ && echo "LEAKS" || echo "OK"
vox build examples/golden/*.vox
```

**Acceptance criteria**:
- No `.vox` file uses `http get "/…"` form.
- Parser rejects the form.
- Docs updated.

---

### TASK-2.6 — Align `workflow`, `activity`, `actor` (Option 1: keyword-sugar)

**Phase**: 2.
**Estimated effort**: 1 day.
**Preconditions**: TASK-2.1.

**Why**: `workflow`, `activity`, `actor` are bare keywords with distinct
HIR variants. Option 1 preserves them as parser sugar for `@durable fn`,
`@activity fn`, `@actor fn` respectively, reducing AST variant count without
breaking source.

**Files to modify**:
- Parser (normalize keyword forms to decorator + `fn`).
- HIR (unify the three Decl variants into `FnDecl` carrying an
  `Option<DurabilityKind>` field).
- Lowering.

**Step-by-step work**:

1. Add `DurabilityKind { Workflow, Activity, Actor }` and
   `FnDecl.durability: Option<DurabilityKind>`.
2. Parser: when it sees `workflow foo()`, produce `FnDecl { durability:
   Some(Workflow), … }`. Same for `activity`, `actor`.
3. Lowering: every backend that currently special-cases the three keywords
   now reads `fn.durability` and dispatches.
4. Delete the three standalone Decl variants.

**Verification commands**:
```bash
cargo test -p vox-compiler
vox build examples/golden/*.vox
```

**Acceptance criteria**:
- `workflow`, `activity`, `actor` still parse at source level.
- HIR has one fn-shaped decl, not four.
- Golden files compile identically.

---

<a id="phase-3--grammar-unification-policy"></a>
## Phase 3 — Grammar unification policy

### TASK-3.1 — Add grammar unification rule to AGENTS.md

**Phase**: 3.
**Estimated effort**: 1 hour.
**Preconditions**: Phase 2 nearly complete (or commit as intent statement).

**Why**: The current grammar has three top-level declaration shapes
(bare-keyword block, decorator-on-type, decorator-on-fn) with no unifying
principle. Document the rule that Phase 2 enforces.

**Files to modify**:
- `AGENTS.md`

**Step-by-step work**:

1. Add a new section after §VoxScript-First Glue Code:

   ```markdown
   ## Grammar Unification (Vox Source Syntax)

   Vox source follows one rule for top-level declarations:

   > **Bare-keyword blocks declare scope. Decorators modify declarations.**

   Examples of bare-keyword blocks (each opens a scope with its own rules):
   `type`, `fn`, `component`, `state_machine`, `routes`, `module`, `actor`,
   `workflow`, `activity`.

   Examples of decorators (modifiers on a declaration):
   `@table`, `@endpoint`, `@pure`, `@deprecated`, `@require`, `@mcp.tool`,
   `@durable`, `@v0`, `@test`, `@scheduled`.

   Decorators compose with bare-keyword blocks:
   `@table type Task { … }` — decorator on a type declaration.
   `@endpoint(kind: query) fn list_tasks() { … }` — decorator on a function.

   Do NOT introduce new bare keywords for modifiable behavior; use a
   decorator.
   ```

2. Cross-link from `docs/src/architecture/architecture-index.md` and any
   grammar-related docs.

**Verification commands**:
```bash
markdownlint AGENTS.md
```

**Acceptance criteria**:
- Rule documented in AGENTS.md.
- One cross-link from architecture index.

---

<a id="phase-4--compiler-primitive-expansion"></a>
## Phase 4 — Compiler primitive expansion

> Four new primitives that the current compiler cannot express. Each adds
> invariants TypeScript + React cannot catch.

### TASK-4.1 — Add `state_machine` first-class block

**Phase**: 4.
**Estimated effort**: 2-3 weeks.
**Preconditions**: Phase 2 complete.

**Why**: `WorkflowScrubber`, `AgentFlow`, `PipelineView` panels are state
machines pretending to be hook chains. A first-class `state_machine` block
with exhaustiveness enforcement kills an entire bug class.

**Target syntax**:

```vox
partial state_machine AgentLifecycle {
  state Idle
  state Working(task: Task)
  state Paused(reason: str)
  terminal state Retired

  on Assign(t)     from Idle       -> Working(t)
  on Pause(r)      from Working(_) -> Paused(r)
  on Resume        from Paused(_)  -> Working(last_task())
  on Retire        from any        -> Retired
}
```

**Compiler enforces**:
- Every (state, event) pair is handled, explicitly ignored via `ignore`, or
  the machine declares it non-total with `partial` modifier.
- Every terminal state is reachable.
- Every transition's target state's fields are initialized.
- Event payloads type-check against transition arguments.

**Files to create**:
- `crates/vox-compiler/src/hir/nodes/state_machine.rs` — `StateMachineDecl`,
  `StateDecl`, `TransitionDecl`, `EventDecl`.
- `crates/vox-compiler/src/typeck/state_machine_check.rs` — exhaustiveness
  + reachability + coverage analysis.
- `crates/vox-codegen/src/web_ir/lower_state_machine.rs` — lower to
  `BehaviorNode::StateMachine` (new variant).
- `crates/vox-codegen-ts/src/state_machine_emit.rs` — emit a
  typed reducer + hook when embedded in a component.

**Files to modify**:
- Parser.
- `hir/nodes/decl.rs` — add `state_machines: Vec<StateMachineDecl>`.
- `web_ir/nodes/behavior.rs` — add `BehaviorNode::StateMachine` variant.
- `web_ir/validate.rs` — coverage validator.

**Step-by-step work** (high level — this is a multi-week task; break into
sub-PRs):

1. **Parse** the syntax. Add tests for happy path + each error message.
2. **Lower to HIR**. Add `StateMachineDecl` with states, events,
   transitions. Check structural constraints (unique state names, unique
   event names, terminal states don't have outgoing transitions).
3. **Type-check**. Exhaustiveness: for each non-terminal state × each
   event, verify a transition or explicit `ignore` exists. Use pattern
   coverage (similar to `match` exhaustiveness).
4. **Lower to Web IR** when the state_machine is embedded in a component.
   Add `BehaviorNode::StateMachine { states, events, transitions,
   initial }`.
5. **Web IR validator**: verify initial state is declared, no dead
   transitions, all referenced state payloads type-check.
6. **Codegen**: emit a typed React reducer with a discriminated union
   return type, plus a hook `useAgentLifecycle()` returning `{ state,
   dispatch }`.
7. **Authoring tests**: write at least two golden examples in
   `examples/golden/` using state_machine.
8. **Docs**: add `docs/src/how-to/how-to-state-machines.md`,
   `docs/src/reference/ref-state-machine.md`.

**Verification commands**:
```bash
cargo test -p vox-compiler state_machine
vox build examples/golden/agent_lifecycle.vox
vox doc-pipeline --mode check
```

**Acceptance criteria**:
- Missing (state, event) combination → compile error citing the missing
  pair.
- Non-reachable terminal state → warning.
- Emitted TS is well-typed; calling `dispatch(wrong_event)` fails
  TypeScript.
- Two golden examples pass.
- Docs land.

**Known issues**:
- Event payload inference may require coordination with existing pattern
  matching.

---

### TASK-4.2 — Add effect annotations (`uses net, db, mcp(...)`)

**Phase**: 4.
**Estimated effort**: 2-3 weeks.
**Preconditions**: Phase 2 complete. Can run in parallel with TASK-4.1.

**Why**: `@pure` is currently a claim with no enforcement. A positive
effect system lets the compiler build a capability graph and catch the
"this function secretly touches the network" class of bug.

**Target syntax**:

```vox
// vox:skip — illustrative; uses deprecated `->` return syntax and `...` placeholders
// No `uses` clause = pure.
fn total(xs: list[int]) -> int { ... }

// Single effect.
fn fetch_tasks() uses net -> list[Task] { ... }

// Multiple effects, some parameterized.
fn save_task(t: Task) uses db, mcp(vox_notify_ludus) -> Id[Task] { ... }
```

**Effect kinds** (initial set):
- `net` — outbound HTTP / WebSocket.
- `db` — database reads or writes.
- `fs` — filesystem reads or writes.
- `mcp(tool_name)` — parameterized; calls a specific MCP tool.
- `env` — environment variable reads.
- `clock` — reads current time.
- `random` — consumes entropy.
- `spawn` — spawns a subprocess or background task.

**Files to create**:
- `crates/vox-compiler/src/hir/nodes/effect.rs` — `Effect`,
  `EffectSet`, `EffectArg`.
- `crates/vox-compiler/src/typeck/effect_check.rs` — propagation.

**Files to modify**:
- Parser (recognize `uses` clause).
- `hir/nodes/decl.rs` — `FnDecl.effects: EffectSet`.
- Type checker — transitively compute the effect set per call site; error
  if a call exceeds the caller's declared set.
- Intrinsics: annotate stdlib functions with their effects (`http.get` →
  `net`, `db.Task.find` → `db`, etc.).
- Codegen — no runtime change, but emit an internal capability table so
  runtime verification can cross-check at the boundary where Vox code
  calls into unannotated Rust.

**Step-by-step work**:

1. Define `Effect` enum and `EffectSet` (sorted/hashed set with cheap
   subset check).
2. Parser + HIR.
3. Stdlib annotation pass: give every intrinsic an effect set.
4. Propagation: `caller.effects ⊇ callee.effects` for every call. Error
   otherwise.
5. `@pure` becomes sugar for `uses nothing`.
6. Docs: `docs/src/how-to/how-to-effects.md`, add examples.
7. Migrate golden files with explicit effect declarations where applicable.
8. Add a compile-time JSON dump of the capability graph (for the
   orchestrator to cross-check against MCP tool declarations).

**Verification commands**:
```bash
cargo test -p vox-compiler effect
vox build examples/golden/*.vox
# A test that should FAIL:
vox check tests/fixtures/effect-escape.vox   # expect: compile error
```

**Acceptance criteria**:
- Calling `http.get(...)` inside a fn without `uses net` fails compilation
  with a precise error.
- `uses net, db` passes through transitive callers.
- Golden examples declare explicit effects where applicable.
- Capability JSON export works.

---

### TASK-4.3 — Add typed URLs primitive

**Phase**: 4.
**Estimated effort**: 1-2 weeks.
**Preconditions**: TASK-2.2, TASK-2.5 (unified endpoint + routes).

**Why**: Routes are currently string patterns. `<Link to="/foo">` is a
brittle string reference. A typed URL algebra gives the compiler a reachable
graph and compile-time link verification.

**Target syntax**:

```vox
// vox:skip -- illustrative: proposed `url` block primitive and typed-link syntax, not implemented
url Path {
  Home
  Task(id: Id[Task])
  Login(?return_to: Path)
  TaskList(?filter: TaskFilter, ?sort: SortKey)
}

// Use sites:
routes {
  Path.Home          to HomePage
  Path.Task(id)      to TaskDetail(id)
  Path.Login(return_to)  to LoginPage
  Path.TaskList(filter, sort)  to TaskListPage
}

component SomePage() {
  view: (
    <div>
      <link to={Path.Task(id)}>View task</link>
      <link to={Path.TaskList(filter: TaskFilter.Open)}>Open tasks</link>
    </div>
  )
}
```

**Files to create**:
- `crates/vox-compiler/src/hir/nodes/url.rs` — `UrlDecl`, `UrlVariant`,
  `UrlArg`.
- `crates/vox-compiler/src/typeck/url_check.rs`.

**Files to modify**:
- Parser.
- `hir/nodes/decl.rs` — `url_decls: Vec<UrlDecl>`.
- `routes { }` parser — accept `Path.Variant(args)` on the left side.
- `<link to=...>` typeck — require typed URL expression, reject strings
  (unless explicitly opted-in via `raw: true` attribute with a compiler
  warning).
- `web_ir/lower.rs` — emit typed route contracts referencing the `url`
  type.

**Step-by-step work**:

1. Parse the `url` bare-keyword block.
2. Build the variant graph in HIR.
3. Type-check `<link to={...}>` against the variant graph; compile error
   on unknown variant.
4. Emit TypeScript enums + builder functions for the URL type; generated
   TSX uses them.
5. Update existing `.vox` files that have `<link to="/…"` strings to use
   typed URLs.

**Verification commands**:
```bash
cargo test -p vox-compiler url
vox build examples/golden/*.vox
# Induce failure:
vox check tests/fixtures/broken-link.vox   # expect: compile error
```

**Acceptance criteria**:
- Compile error on referencing a deleted URL variant.
- Warning on string `to="/…"` form (not yet error; migration window).
- Emitted TSX is type-safe.
- Golden files use typed URLs.

---

### TASK-4.4 — Add design-token types (compile `vox.tokens.json` into types)

**Phase**: 4.
**Estimated effort**: 1 week.
**Preconditions**: Phase 2 complete.

**Why**: `vox.tokens.json` exists but the compiler doesn't read it. Turn it
into a typed enum per token category (Color, Spacing, Radius, Typography,
Surface), loaded at compile time.

**Target token file shape** (expand beyond today's minimal file):

```json
{
  "version": "1.0",
  "color": {
    "surface.base":    "#09090b",
    "surface.raised":  "#18181b",
    "surface.primary": "#3b82f6",
    "...": "..."
  },
  "spacing": {
    "s0":  "0",
    "s1":  "0.25rem",
    "s2":  "0.5rem",
    "s4":  "1rem",
    "s8":  "2rem"
  },
  "radius": { "none": "0", "sm": "0.25rem", "md": "0.5rem", "lg": "1rem", "full": "9999px" },
  "typography": {
    "body":    { "size": "1rem",    "line": "1.5",  "weight": 400 },
    "heading.l":  { "size": "2rem",    "line": "1.2",  "weight": 700 }
  },
  "surface.pairs": [
    { "name": "primary",   "fg": "surface.primary-fg", "bg": "surface.primary" },
    { "name": "danger",    "fg": "surface.danger-fg",  "bg": "surface.danger" },
    { "name": "muted",     "fg": "text.muted",         "bg": "surface.base" }
  ]
}
```

**Files to create**:
- `crates/vox-compiler/src/tokens/mod.rs` — loader.
- `crates/vox-compiler/src/tokens/validate.rs` — check pair references
  resolve; check pair contrast ratios ≥ 4.5:1 at token-load time (emit
  warning on `body` pairs under 4.5:1, error on `body` pairs under 3:1).

**Files to modify**:
- `vox.tokens.json` — expand schema to the shape above (minimal real values).
- Build pipeline — read tokens.json once per compile, build the type
  tables.
- Web IR `StyleNode` — `TokenRef(String)` now typechecks against the
  loaded token registry.
- Codegen TS — emit tokens as Tailwind-compatible CSS variables or as
  Vanilla Extract theme.

**Step-by-step work**:

1. Parse `vox.tokens.json` at compiler startup; keep in a
   `Arc<TokenRegistry>`.
2. Define a JSON schema (`contracts/tokens/tokens.v1.json`); validate
   `vox.tokens.json` against it on every build.
3. Update Web IR `validate.rs` Style stage: resolve every `TokenRef`
   against registry; unknown name → error.
4. Flag literal `#rrggbb`, `rgb(…)`, or `<n>px` in `StyleDeclarationValue::Raw`
   as warnings (will become errors in Phase 6 with the GUI DSL).
5. Compute contrast ratios for declared pairs; emit warnings.
6. Export the registry as TypeScript for the emitted app:
   `generated/tokens.ts`.

**Verification commands**:
```bash
cargo test -p vox-compiler tokens
vox build examples/golden/*.vox
# Induce failure:
echo 'component X() { style: { .x { color: surface.nonexistent } } }' > tests/fixtures/bad-token.vox
vox check tests/fixtures/bad-token.vox   # expect: error
```

**Acceptance criteria**:
- Unknown token name → compile error with "did you mean?" suggestions.
- Pair contrast computed and warned/errored as specified.
- Golden files reference tokens by name.
- Generated `tokens.ts` used by the emitted app.

---

<a id="phase-5--web-ir-correctness-validators"></a>
## Phase 5 — Web IR correctness validators

> Four validators that extend `validate_web_ir` with the invariants
> TypeScript cannot express.

### TASK-5.1 — Token resolution validator (hardening)

**Phase**: 5.
**Estimated effort**: 2-3 days.
**Preconditions**: TASK-4.4.

**Why**: TASK-4.4 introduced the registry. This task tightens enforcement:
literal CSS values in `Raw` become errors, not warnings. Fallback path is
explicit `raw_css { }` escape hatch.

**Files to modify**:
- `crates/vox-codegen/src/web_ir/validate.rs` (Style stage).

**Step-by-step work**:

1. In validate.rs Style stage, for each `Declaration` whose value is
   `StyleDeclarationValue::Raw`:
   - If the raw value matches a hex color (`#[0-9a-fA-F]{3,8}`), an
     rgb() expression, a named CSS color, or a dimensional literal (`\d+(px|rem|em|%)`):
     emit an error with code `web_ir_validate.style.literal_value`.
   - Exception: inside an explicit `raw_css { }` wrapper element (to be
     added in Phase 6); until then, allow but warn.
2. Add an error-code doc entry.

**Verification**: (same pattern as 4.4).

**Acceptance criteria**:
- Literal hex/px in `.vox` style blocks → compile error.
- Migration guide in docs.

---

### TASK-5.2 — Route reachability validator

**Phase**: 5.
**Estimated effort**: 3-4 days.
**Preconditions**: TASK-4.3.

**Why**: Web IR has the data to verify `<link to={Path.X}>` resolves to a
declared route, that every `routes { }` entry's component exists, and that
there are no dead routes.

**Files to modify**:
- `crates/vox-codegen/src/web_ir/validate.rs` (Routes stage).

**Step-by-step work**:

1. Walk `RouteNode::RouteTree` once; collect the set of `RouteContract.id`
   values.
2. Walk every `DomNode::Element` that is a `<link>` or typed-link: verify
   its URL expression resolves to a route.
3. Walk every `RouteContract.component_name`: verify a corresponding
   `view_root` exists.
4. Warn on routes with no inbound link (unreachable).

**Verification**:

```bash
cargo test -p vox-compiler web_ir::validate::routes
vox build examples/golden/*.vox
```

**Acceptance**: broken link → compile error with the specific URL variant
named.

---

### TASK-5.3 — AriaNode + a11y validator

**Phase**: 5.
**Estimated effort**: 2-3 weeks.
**Preconditions**: Phase 2 complete.

**Why**: A11y entirely absent from Web IR. Svelte's compile-time a11y is
the precedent. Vox should match.

**Files to create**:
- `crates/vox-codegen/src/web_ir/nodes/aria.rs` — `AriaNode`, `Role`,
  `KeyAffordance`.
- `crates/vox-codegen/src/web_ir/validate_a11y.rs`.

**Step-by-step work**:

1. Every `DomNode::Element` carries optional `aria: Option<AriaNode>`.
2. Lowering infers aria from element kind + attributes:
   - `button` / `a[href]` → `role: button | link`; requires accessible
     name (text child, `aria-label`, or `aria-labelledby`).
   - `img` → requires `alt` attribute or `aria-hidden="true"`.
   - Form controls → require associated `<label>` (explicit or implicit).
   - Any element with `role="button"` → requires `keyboard` affordance
     (onClick + onKeyDown or an implicit keyboard handler from the
     primitive).
3. Validator `validate_a11y` walks the tree and emits errors/warnings.
4. Doc: `docs/src/how-to/how-to-accessibility.md` explaining the rules and
   escape hatches (`aria_hidden: true`, `decorative: true`).

**Verification commands**:
```bash
cargo test -p vox-compiler web_ir::validate_a11y
vox build examples/golden/*.vox
```

**Acceptance**:
- `<img src="...">` without `alt` → compile error.
- `<button></button>` without content or label → compile error.
- `<div role="button">` without keyboard handler → compile error.
- Docs include escape-hatch syntax.

---

### TASK-5.4 — v0.dev output validator

**Phase**: 5.
**Estimated effort**: 1-2 weeks.
**Preconditions**: TASK-5.1, TASK-5.2, TASK-5.3 (validators to run against
the parsed output).

**Why**: `vox island generate` currently writes v0.dev output verbatim into
`islands/src/<Name>/`. No compiler pass verifies structure.

**Files to read first**:
- `crates/vox-cli/src/commands/island/actions.rs` lines 19-72.
- `crates/vox-cli/src/commands/island/v0.rs`.

**Files to modify**:
- `crates/vox-cli/src/commands/island/actions.rs`.
- `crates/vox-cli/src/commands/island/v0.rs`.

**Step-by-step work**:

1. After v0.dev returns TSX, parse it via a lightweight TSX parser (use
   `swc_ecma_parser` or similar; it's already likely in the workspace).
2. Extract: imports, exported component name, prop interface, JSX tree.
3. Build a partial Web IR from the extracted data.
4. Run the Web IR validators (5.1, 5.2, 5.3).
5. If any errors, present the user with:
   - The raw output.
   - The violations.
   - Options: (a) reject and re-prompt v0 with a corrective system message,
     (b) accept and manually patch, (c) drop into `raw_tsx { }` escape
     block.
6. If acceptance, write the TSX + island stub as today.

**Verification commands**:
```bash
cargo test -p vox-cli island::v0
```

**Acceptance**:
- v0 output that would violate a11y / tokens / routes is caught before
  being written.
- User experience cleanly handles the three outcomes.

---

<a id="phase-6--vox-gui-authoring-dsl"></a>
## Phase 6 — Vox GUI authoring DSL

> The most ambitious phase. Introduce a view DSL that replaces JSX / raw
> CSS / className strings with typed semantic primitives. HTML becomes an
> emission target, not the authoring surface. Tailwind/VE becomes a
> compiler backend.

### TASK-6.1 — Define the semantic primitive set

**Phase**: 6.
**Estimated effort**: 2 weeks (design + initial grammar + codegen for 50%).
**Preconditions**: Phase 4, Phase 5 complete.

**Why**: First cut of the ~20 layout/semantic primitives that replace JSX
tags.

**Initial primitive set** (grouped):

Layout containers:
`stack`, `row`, `column`, `wrap`, `grid`, `overlay`, `spacer`, `divider`.

Content primitives:
`text`, `heading`, `code`, `icon`, `image`, `link`, `badge`.

Interactive:
`button`, `icon_button`, `field`, `textarea`, `select`, `checkbox`, `toggle`,
`radio`.

Structural:
`panel`, `card`, `rail`, `list`, `list_item`, `table`, `route_outlet`.

Each primitive:
- Has a fixed prop signature (no prop extension, period).
- Declares which HTML tag it emits.
- Declares its accessibility affordances.
- Accepts typed token refs for visual properties.

**Files to create**:
- `crates/vox-codegen/src/web_ir/primitives/mod.rs`
- `crates/vox-codegen/src/web_ir/primitives/<primitive>.rs` — one per
  primitive with its signature and emission rules.

**Files to modify**:
- Parser (recognize primitive names inside `view:` block).
- Lowering (map primitive invocations to Web IR nodes).
- `codegen_ts` (emit the right HTML tag + Tailwind classes).

**Step-by-step work**:

This is a multi-week task. Sequence:

1. Pick the 10 highest-usage primitives first (`stack`, `row`, `column`,
   `text`, `button`, `link`, `panel`, `card`, `list`, `route_outlet`).
2. Grammar + HIR + Web IR lowering + TSX emission for each.
3. Write authoring tests: one golden example per primitive.
4. Ship Tailwind-emission backend first (matches what the dashboard uses).
5. Add the other 10 primitives in a follow-up PR.

**Verification**:
```bash
cargo test -p vox-compiler primitives
vox build examples/golden/primitive-showcase.vox
```

**Acceptance**:
- Every primitive has at least one golden test.
- Emitted TSX is visually identical to a hand-written equivalent.
- Primitive signatures are documented in
  `docs/src/reference/ref-primitives.md`.

---

### TASK-6.2 — Token-ref-only style values (delete raw CSS support at authoring layer)

**Phase**: 6.
**Estimated effort**: 1 week.
**Preconditions**: TASK-6.1, TASK-4.4, TASK-5.1.

**Why**: Once primitives are in place, the `style: { }` block on
`@component` should only accept typed token references. Raw hex / px / rgb
enters only via an explicit `raw_css { }` escape.

**Files to modify**:
- Parser for `style: { }`.
- Web IR validator (already errors on `Raw` with literal values per
  TASK-5.1; tighten to reject ALL `Raw` unless in `raw_css`).

**Step-by-step work**:
1. Remove parser support for raw CSS values outside `raw_css { }`.
2. Add `raw_css { ... }` escape hatch with a warning.
3. Migrate golden files.

**Acceptance**: `.vox` source cannot contain literal CSS values outside
`raw_css`.

---

### TASK-6.3 — Surface pair primitive

**Phase**: 6.
**Estimated effort**: 1 week.
**Preconditions**: TASK-4.4.

**Why**: Authors declare `surface: primary` which binds a typed fg/bg pair,
rather than setting `color` and `background-color` independently. Contrast
guaranteed by construction.

**Target syntax**:

```vox
// vox:skip -- illustrative: proposed `surface:` primitive and `:` call-arg syntax, not implemented
panel(surface: primary) {
  text(size: body) "Hello"
}
```

**Files to modify**:
- Primitive signatures (each visual primitive accepts `surface:
  Option<SurfacePair>`).
- Web IR emission (lowers to two CSS vars: `--fg`, `--bg`).
- Token file schema (already has `surface.pairs`).

**Acceptance**: `surface: nonexistent` → compile error. Inline `color:`
still permitted for typography overrides where sensible; compiler enforces
contrast against the surface's bg.

---

### TASK-6.4 — Overlay block + z-index DAG

**Phase**: 6.
**Estimated effort**: 1-2 weeks.
**Preconditions**: TASK-6.1.

**Why**: Absolute positioning becomes an opt-in escape via `overlay { }`;
inside, the compiler verifies z-index ordering forms a DAG and performs a
rudimentary AABB non-overlap check at declared breakpoints.

**Target syntax**:

```vox
// vox:skip -- illustrative: proposed `overlay` block primitive, not implemented; `...` placeholder body
overlay {
  toast(z: 100, position: top_right) { ... }
  drawer(z: 90, position: left) { ... }
  modal(z: 110, position: center) { ... }
}
```

**Files to create**:
- `crates/vox-codegen/src/web_ir/validate_overlay.rs` — DAG +
  AABB check.

**Acceptance**:
- Overlay children with same z → warning.
- Overlap at any declared breakpoint → warning (becomes error in future
  tightening).

---

### TASK-6.5 — Contrast ratio along ancestor chain

**Phase**: 6.
**Estimated effort**: 1 week.
**Preconditions**: TASK-6.3.

**Why**: Surface pairs are contrast-checked at token load. But nested
surfaces can override; a `text` that inherits fg from an ancestor surface
but sits on a descendant's bg is where contrast bugs hide.

**Files to modify**:
- `crates/vox-codegen/src/web_ir/validate_a11y.rs`.

**Step-by-step work**:
1. Walk the primitive tree once, tracking the current (fg, bg) pair per
   subtree.
2. At every `text` / `heading` / `code` node, compute WCAG ratio and
   error/warn per WCAG 2.1 thresholds (4.5:1 body, 3:1 large text).

**Acceptance**: text on insufficient contrast → compile error with ratio.

---

<a id="phase-7--dashboard-re-author"></a>
## Phase 7 — Dashboard re-author through `vox-codegen-ts`

### TASK-7.1 — Re-author `App.tsx` as `app.vox`

**Phase**: 7.
**Estimated effort**: 1 week.
**Preconditions**: Phase 6 primitives usable.

**Why**: The df1d6919 commit hand-wrote the shell in TSX. Land it in
`.vox` so the dashboard dogfoods the new authoring language.

**Files to create**:
- `crates/vox-dashboard/app/src/app.vox`
- `crates/vox-dashboard/app/src/tabs/speak.vox`
- `crates/vox-dashboard/app/src/tabs/command.vox`
- `crates/vox-dashboard/app/src/tabs/network.vox`
- `crates/vox-dashboard/app/src/tabs/forge.vox`

**Files to delete** (after migration verified):
- `crates/vox-dashboard/src/App.tsx`
- `crates/vox-dashboard/src/components/*.tsx` (one by one as they're
  ported in Task 7.2).

**Step-by-step work**:
1. Write `app.vox` with the outer rail + main + tab switcher using new
   primitives.
2. Update the build pipeline to invoke `vox build --target
   dashboard-spa` which compiles `app/src/*.vox` through
   `vox-codegen-ts` and outputs to `dist/`.
3. Replace the Vite entrypoint to consume the compiled output.
4. Smoke test against the running orchestrator.

**Verification**: visual parity with the current dashboard.

**Acceptance**: `crates/vox-dashboard/src/App.tsx` deleted; `app.vox` is
the source of truth.

---

### TASK-7.2 — Re-author panel components in `.vox`

**Phase**: 7.
**Estimated effort**: 3-4 weeks (one panel at a time).
**Preconditions**: TASK-7.1.

**Why**: The 13 components (`AgentFlow`, `AstView`, `AttentionPanel`,
`CodeBlock`, `ComposerPanel`, `ContextExplorer`, `EngineeringDiagnostics`,
`ErrorBoundary`, `IntentionMatrix`, `MeshTopology`, `PipelineView`,
`UnifiedDashboard`, `WorkflowScrubber`) are hand-written React. Re-author
in `.vox`.

**Strategy**: port one per PR, starting with the ones that benefit most
from the state-machine primitive (`WorkflowScrubber`, `AgentFlow`,
`PipelineView`).

**Acceptance criteria per panel**:
- `.vox` source replaces the `.tsx`.
- Emitted TSX preserves behavior.
- State transitions (where applicable) use `state_machine`.
- A11y validator passes.
- Token validator passes.

---

### TASK-7.3 — Delete the dashboard's parallel Vite/Tailwind setup

**Phase**: 7.
**Estimated effort**: 3-4 hours.
**Preconditions**: TASK-7.1, TASK-7.2 complete.

**Why**: Once the dashboard builds through `vox-codegen-ts`, the
`package.json` + `pnpm-lock.yaml` + `vite.config.ts` +
`tailwind.config.js` are parallel build infrastructure the compiler is
now doing. Delete them.

**Files to delete**:
- `crates/vox-dashboard/package.json`
- `crates/vox-dashboard/pnpm-lock.yaml`
- `crates/vox-dashboard/vite.config.ts`
- `crates/vox-dashboard/tailwind.config.js`
- `crates/vox-dashboard/postcss.config.js`
- `crates/vox-dashboard/tsconfig.json`
- `crates/vox-dashboard/eslint.config.*`

**Files to modify**:
- `crates/vox-dashboard/build.rs` — invoke `vox build --target
  dashboard-spa` instead of `pnpm run build`.
- `crates/vox-dashboard/.gitignore`.

**Acceptance**:
- `ls crates/vox-dashboard/` shows no `package.json` or related.
- `cargo build -p vox-dashboard --features embedded-assets` produces a
  working bundle via `vox build`.

---

<a id="phase-8--corpus-migration--mens-training"></a>
## Phase 8 — Corpus migration + MENS training

### TASK-8.1 — Atomic corpus migration PR

**Phase**: 8 (but blocks parts of Phase 2 and 4).
**Estimated effort**: 2-3 days.
**Preconditions**: Every syntax-changing task in Phase 2 and Phase 4
should either (a) be in the same PR as its corpus migration, or (b) ship
a single follow-up "migrate corpus" PR before the next training run.

**Why**: If the corpus and the compiler diverge, MENS learns the old syntax
and users get outputs that no longer compile. Keep corpus + compiler
atomic.

**Files to create**:
- `scripts/migrate-corpus.vox` — a `.vox` automation script that walks
  `examples/golden/`, `scripts/`, `tests/`, `docs/src/**/*.md` and
  rewrites old syntax to new.

**Step-by-step work**:

1. Identify every syntax change that touches the corpus.
2. Write `migrate-corpus.vox` following the VoxScript-First policy:
   - Uses `vox-compiler` as a library (not regex).
   - For each file, parse with the old-form parser (running in
     compatibility mode), re-emit with the new form.
   - Dry-run mode + write mode.
3. Run in dry-run. Review diff.
4. Run in write mode. Commit atomically with the compiler change.
5. Remove compatibility-mode parsing a release later.

**Verification**:
```bash
vox run scripts/migrate-corpus.vox --dry-run
vox run scripts/migrate-corpus.vox --write
vox build examples/golden/*.vox
```

**Acceptance**: 100% of corpus compiles under the new compiler.

---

### TASK-8.2 — MENS training run on new corpus

**Phase**: 8.
**Estimated effort**: 1 week (mostly compute time).
**Preconditions**: TASK-8.1 complete.

**Why**: Corpus changes that aren't followed by a training run are
invisible to the downstream model.

**Step-by-step work**:

1. Run `vox populi train --config qlora.toml` against the new corpus.
2. Run `vox populi eval --suite golden` against the resulting model.
3. Compare against the previous run's eval scores.
4. If regression > 5% on any golden, STOP and investigate (likely a
   corpus gap or a primitive collapse that lost signal).

**Acceptance**: Eval scores ≥ previous run or within 5%.

---

<a id="phase-9--native-bundler-swap"></a>
## Phase 9 — Native Bundler Swap (Node.js Elimination)

> The final step in achieving a true zero-dependency "single command install" for GUI-native development. Replaces Vite and the Node.js/`pnpm` ecosystem with a native Rust bundler (like Rolldown or Oxc) integrated directly into the `vox` binary.

### TASK-9.1 — Integrate Rolldown core into `vox-compiler`

**Phase**: 9.
**Estimated effort**: 2-3 weeks.
**Preconditions**: Phase 7 complete.

**Why**: `vox build` currently relies on shelling out to `pnpm` and `vite` to process the emitted TSX files. Integrating a native Rust bundler (like `rolldown`) eliminates the Node.js dependency and provides a fully self-contained build step.

**Files to create**:
- `crates/vox-codegen/src/bundler/mod.rs`
- `crates/vox-codegen/src/bundler/rolldown_adapter.rs`

**Files to modify**:
- `crates/vox-compiler/Cargo.toml` (add `rolldown` dependencies).
- `crates/vox-cli/src/commands/build.rs` (invoke internal bundler instead of Node process).

**Step-by-step work**:
1. Add `rolldown` as a workspace dependency.
2. Build an adapter in `crates/vox-codegen/src/bundler/` that takes the in-memory or on-disk emitted TSX and routes it through Rolldown.
3. Replace the `pnpm install` and `vite build` shell execution paths in `vox build` with a direct Rust call to the internal bundler.
4. Migrate Tailwind compilation to a pure Rust equivalent (`lightningcss` or similar) if needed, or emit pre-computed static CSS from Phase 6 design tokens.

**Acceptance**:
- `vox build` completes successfully on a machine with no Node.js installed.
- No `node_modules` directory is generated.

---

### TASK-9.2 — Retire NPM / Vite artifacts

**Phase**: 9.
**Estimated effort**: 1 week.
**Preconditions**: TASK-9.1 complete.

**Why**: Clean up the legacy JavaScript ecosystem files now that the native bundler is operational.

**Step-by-step work**:
1. Remove `package.json` generation logic from `vox init`.
2. Remove Vite config template generation.
3. Update `vox doctor` to no longer require `node` or `pnpm` for the `frontend` target.
4. Drop Node.js and `pnpm` from the required dependency matrix in `README.md`.

**Acceptance**:
- New Vox projects initialize and build purely with `.vox` and `Cargo.toml`.
- Node.js is formally dropped from the required dependency matrix.

---

<a id="appendix-a--common-pitfalls"></a>
## Appendix A — Common pitfalls (specifically for weaker LLM executors)

1. **Do not invent file paths.** Every path in this document was verified
   against the repo on 2026-04-23. If a path is missing when you execute,
   STOP and escalate.
2. **Do not invent API signatures.** If a task references
   `vox_secrets::resolve_secret`, find the signature in the source before
   calling it. Do not guess.
3. **Do not silently skip verification commands.** If `cargo test` fails,
   the task is not done.
4. **Do not ignore compiler warnings.** `cargo clippy -- -D warnings` is
   the standard; warnings are errors in CI.
5. **Do not add new crates without asking.** Check `Cargo.toml`
   `[workspace.dependencies]` first; if the crate isn't there, escalate.
6. **Do not commit generated files.** `mcpToolRegistry.generated.ts`,
   `dist/`, `tsconfig.tsbuildinfo`, `target/`, etc. are build products.
7. **Do not modify `archive/` or `docs/src/archive/`.** Tombstone
   directories.
8. **Do not introduce Python or shell scripts.** Use `.vox` + `vox run`.
9. **Preserve commit provenance.** Primary author is the operator
   (Bertrand Reyna-Brainerd <brbrainerd@gmail.com>). Agent is
   `Co-authored-by: AI Assistant <…>`.
10. **Respect structural limits.** Blocks >500 LOC, >12 methods, >20 files
    per dir trip the sprawl detector. Split before shipping.
11. **Honor `// vox:skip`.** Code blocks in docs with this annotation are
    intentionally invalid; do not try to fix them.
12. **Respect `.voxignore`.** Derived ignore files are regenerated; do
    not edit them by hand.

---

<a id="appendix-b--verification-playbook"></a>
## Appendix B — Verification playbook

Before marking any task complete, run:

```bash
# 1. Quick sanity
cargo check --workspace --all-features

# 2. Full test suite
cargo test --workspace --all-features

# 3. Clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 4. Format
cargo fmt --all -- --check

# 5. TOESTUB gates
vox ci toestub-scoped --report

# 6. Secret gates
vox ci secret-env-guard
vox ci secrets-parity

# 7. Ignore file sync
vox ci sync-ignore-files

# 8. Documentation doctests
vox doc-pipeline --mode check

# 9. Golden-file rebuild
vox build examples/golden/*.vox

# 10. VS Code extension (if touched)
cd apps/editor/vox-vscode && npm run compile && npm run lint
```

If any step fails and the failure is not a pre-existing unrelated issue,
STOP and escalate.

---

<a id="appendix-c--escalation-protocol"></a>
## Appendix C — Escalation protocol

When a task blocks on something unexpected, produce an **Escalation Note**
and stop work.

**Escalation Note template**:

```
## Escalation: TASK-X.Y

**Operator**: Bertrand Reyna-Brainerd <brbrainerd@gmail.com>
**Agent**: <model name + version>
**Date**: <YYYY-MM-DD HH:MM>

### Blocker

<one paragraph describing what you found that wasn't in the task spec>

### Evidence

- File: `<path>` lines `<range>`
- Command: `<command run>`
- Output:
  ```
  <paste actual output, trimmed to relevant portion>
  ```

### Options considered

1. <option and why rejected>
2. <option and why rejected>

### Recommended next step

<specific operator action requested>

### Work done so far (do not lose)

- <list of files already modified and committed>
- <files modified but not committed — attach diffs>
```

Save to `docs/src/architecture/escalations-2026/TASK-X.Y-<date>.md` (create
the directory if needed) and push. Notify the operator via whatever channel
is configured.

---

## Change log

- 2026-04-23 — Initial roadmap written (Claude Opus session with operator
  Bertrand Reyna-Brainerd).

---

**End of roadmap.**
