# LLM Guidance File Token Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix every stale/contradictory reference in Vox's LLM guidance files (`AGENTS.md`, `CLAUDE.md`, `.cursor/rules/*.mdc`, and related docs), collapse standing duplication into single-source pointers, trim non-load-bearing prose from `AGENTS.md`, and add a small CI drift-guard so this class of staleness can't silently recur.

**Architecture:** This is a docs/config content pass, not a code feature. Each task is an independent, small, git-committable edit to one or a few files, verified by `grep` (for stale-reference removal) or a read-through (for trims). The one real code change (Task 13) extends an existing, already-wired Rust CI check with a regression test.

**Tech Stack:** Markdown, `.mdc` (Cursor rules), Rust (`crates/vox-cli`), existing `vox ci` tooling.

---

## Before you start

All file paths below were verified directly against the repo on 2026-08-01 (not taken on faith from an earlier audit pass — one earlier claim, that `.cursor/rules/retired-surfaces.mdc` had "no drift," turned out to be wrong; see Task 7). If any `grep` in a verification step finds unexpected extra hits, stop and re-check rather than force the edit through.

Do all work in this worktree: `C:\Users\Owner\vox\.claude\worktrees\llm-guidance-token-audit-b7e2aa`. Design spec: [`docs/superpowers/specs/2026-08-01-llm-guidance-token-audit-design.md`](../specs/2026-08-01-llm-guidance-token-audit-design.md).

**Scope refinements found during verification** (the spec explicitly asked for these to be confirmed at implementation time — this is that confirmation, not a deviation to re-approve):
- `.cursor/rules/data-storage-policy.mdc` is **not** touched. It looked like a 4th copy of the secrets rule in the initial audit, but its content is Tier A/B/C/D data-storage policy with one contextual secrets mention — genuinely unique, not a restatement.
- `.cursor/rules/secrets-policy.mdc` gets only the stale-path fix (Task 1). At 11 lines it's already tight; there's nothing left to reduce.
- `docs/src/contributors/coding-agents.md` is **not** touched for the retired-crate table. It already only cites one example (`vox-dei` → `vox-orchestrator`) and points to `AGENTS.md` for the full table — it was never a full restatement.
- `AGENTS.md`'s §Local CI Gate Tiers table is **not** trimmed. The spec proposed moving 5 of its 7 rows to `docs/src/contributors/local-ci-pre-push.md`, but that file doesn't currently have that detail — trimming without first authoring it there would delete information, not move it. Left as-is; a genuine reference table, not restated bloat.
- `.cursor/rules/retired-surfaces.mdc` gets a **content fix**, not the "reduce to pointer" originally scoped — verification found two of its rows actively contradict `AGENTS.md` (see Task 7), which is a more important problem than the table being merely duplicative.

---

### Task 1: Fix stale `crates/vox-secrets/src/spec.rs` path

`crates/vox-secrets/src/spec.rs` no longer exists — it was restructured into a `spec/` module (`spec/mod.rs`, `spec/ids.rs`, `spec/types.rs`, `spec/registry/*.rs`). 6 currently-loaded guidance/source files still tell agents to declare secrets there.

**Files:**
- Modify: `AGENTS.md:120`, `AGENTS.md:133`
- Modify: `.cursor/rules/secrets-policy.mdc:9`
- Modify: `docs/src/reference/agent-quick-reference.md:26`
- Modify: `docs/src/contributors/toestub-contributor-guide.md:167`
- Modify: `crates/vox-code-audit/src/detectors/env_secret_shape.rs:95`
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs:604`

- [ ] **Step 1: Fix `AGENTS.md:120`**

Old:
```
- Define and maintain secret metadata in `crates/vox-secrets/src/spec.rs`.
```
New:
```
- Define and maintain secret metadata in `crates/vox-secrets/src/spec/` (the `SecretId` enum lives in `spec/ids.rs`; entries live under `spec/registry/`).
```

- [ ] **Step 2: Fix `AGENTS.md:133`**

Old:
```
1. Add `SecretId` and `SecretSpec` entries in `crates/vox-secrets/src/spec.rs`.
```
New:
```
1. Add `SecretId` and `SecretSpec` entries in `crates/vox-secrets/src/spec/` (`spec/ids.rs` for the enum, `spec/registry/` for entries).
```

- [ ] **Step 3: Fix `.cursor/rules/secrets-policy.mdc:9`**

Old:
```
- Add new secrets to `crates/vox-secrets/src/spec.rs` with `SecretId` + `SecretSpec`
```
New:
```
- Add new secrets to `crates/vox-secrets/src/spec/` (`ids.rs` + `registry/`) with `SecretId` + `SecretSpec`
```

- [ ] **Step 4: Fix `docs/src/reference/agent-quick-reference.md:26`**

Old:
```
Never read `std::env::var("SECRET")`; exclusively employ `vox_secrets::resolve_secret(...)` and declare it in `crates/vox-secrets/src/spec.rs`.
```
New:
```
Never read `std::env::var("SECRET")`; exclusively employ `vox_secrets::resolve_secret(...)` and declare it in `crates/vox-secrets/src/spec/`.
```

- [ ] **Step 5: Fix `docs/src/contributors/toestub-contributor-guide.md:167`**

Old:
```
Declare the `SecretId` variant in `crates/vox-secrets/src/spec.rs`. See
```
New:
```
Declare the `SecretId` variant in `crates/vox-secrets/src/spec/` (`ids.rs` + `registry/`). See
```

- [ ] **Step 6: Fix `crates/vox-code-audit/src/detectors/env_secret_shape.rs:95`**

Old (lines 94-98):
```rust
            alternatives: vec![
                "Add a SecretSpec entry in crates/vox-secrets/src/spec.rs, then call \
                 vox_secrets.resolve(SecretId::YourKey)"
                    .to_string(),
            ],
```
New:
```rust
            alternatives: vec![
                "Add a SecretSpec entry in crates/vox-secrets/src/spec/ (ids.rs + registry/), then call \
                 vox_secrets.resolve(SecretId::YourKey)"
                    .to_string(),
            ],
```

- [ ] **Step 7: Fix `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs:604`**

Old:
```rust
            "operator-env-guard: found {} usage(s) of unregistered environment variables:\n{}\n\nRegister in `crates/vox-secrets/src/spec.rs` (secrets) or `crates/vox-config/src/operator_registry.rs` (tuning).",
```
New:
```rust
            "operator-env-guard: found {} usage(s) of unregistered environment variables:\n{}\n\nRegister in `crates/vox-secrets/src/spec/` (secrets) or `crates/vox-config/src/operator_registry.rs` (tuning).",
```

- [ ] **Step 8: Verify no stale references remain**

Run: `grep -rn "vox-secrets/src/spec.rs" --include="*.md" --include="*.mdc" --include="*.rs" AGENTS.md CLAUDE.md GEMINI.md .cursor crates/vox-code-audit crates/vox-cli docs/src/reference/agent-quick-reference.md docs/src/contributors/toestub-contributor-guide.md`
Expected: no output (zero matches).

- [ ] **Step 9: Compile-check the two Rust edits**

Run: `cargo check -p vox-code-audit -p vox-cli`
Expected: exits 0, no errors (these are string-literal-only edits, should be a no-op for the type system).

- [ ] **Step 10: Commit**

```bash
git add AGENTS.md .cursor/rules/secrets-policy.mdc docs/src/reference/agent-quick-reference.md docs/src/contributors/toestub-contributor-guide.md crates/vox-code-audit/src/detectors/env_secret_shape.rs crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs
git commit -m "docs: fix stale crates/vox-secrets/src/spec.rs path (now spec/ module)"
```

---

### Task 2: Fix stale orchestrator `TOOL_REGISTRY` path

`docs/agents/orchestrator.md` claims the authoritative `TOOL_REGISTRY` lives at `crates/vox-orchestrator/src/mcp_tools/tools/mod.rs` — that nested path doesn't exist (the crate's modules are flat under `src/`, as the same file's own "Crate layout" table documents). Verified current truth: `TOOL_REGISTRY` is generated by `crates/vox-mcp-registry/build.rs` and re-exported at `crates/vox-orchestrator-mcp/src/lib.rs:186` (`pub use vox_mcp_registry::TOOL_REGISTRY;`).

**Files:**
- Modify: `docs/agents/orchestrator.md:18`, `docs/agents/orchestrator.md:90`

- [ ] **Step 1: Fix `docs/agents/orchestrator.md:18`**

Old:
```
**Authoritative MCP tool names + descriptions:** `crates/vox-orchestrator/src/mcp_tools/tools/mod.rs` → `TOOL_REGISTRY`. The grouped lists below are for humans and may lag that array when new tools land.
```
New:
```
**Authoritative MCP tool names + descriptions:** `TOOL_REGISTRY`, generated by `crates/vox-mcp-registry/build.rs` and re-exported at `crates/vox-orchestrator-mcp/src/lib.rs`. The grouped lists below are for humans and may lag that array when new tools land.
```

- [ ] **Step 2: Fix `docs/agents/orchestrator.md:90`**

Old:
```
Grouped for readability only — **names and descriptions** must match `TOOL_REGISTRY` in `vox-mcp`.
```
New:
```
Grouped for readability only — **names and descriptions** must match `TOOL_REGISTRY` (`crates/vox-mcp-registry`, re-exported from `vox-orchestrator-mcp`).
```

- [ ] **Step 3: Verify the corrected path is real**

Run: `grep -n "pub use vox_mcp_registry::TOOL_REGISTRY" crates/vox-orchestrator-mcp/src/lib.rs`
Expected: one match at the line cited above (confirms the fix points at a real, current symbol).

- [ ] **Step 4: Verify no stale nested path remains**

Run: `grep -rn "mcp_tools/tools/mod.rs" docs/`
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add docs/agents/orchestrator.md
git commit -m "docs: fix stale TOOL_REGISTRY path in orchestrator.md"
```

---

### Task 3: Fix stale `scripts/quality/toestub_scoped.sh` references

The script no longer exists on disk (confirmed: `test -f scripts/quality/toestub_scoped.sh` fails). Four currently-loaded docs still tell agents to invoke it. The canonical replacement, already used correctly as the primary recommendation in `docs/src/reference/cli.md`, is `vox ci toestub-scoped`.

**Files:**
- Modify: `docs/agents/governance.md:14-19`
- Modify: `docs/agents/cli-toolchain.md:76-78`
- Modify: `docs/agents/orchestrator.md:59`
- Modify: `docs/src/reference/cli.md:622`

- [ ] **Step 1: Fix `docs/agents/governance.md` (lines 14-19)**

Old:
```
**CI / agents (canonical)** — no `vox` feature gate; calls the `toestub` binary directly:

```bash
bash scripts/quality/toestub_scoped.sh                    # default root: crates/vox-repository
cargo run -p vox-code-audit --bin toestub -- <PATH>         # explicit scan root
```
```
New:
```
**CI / agents (canonical)** — no `vox` feature gate; calls the `toestub` binary directly:

```bash
vox ci toestub-scoped                                        # default root: crates/vox-repository
cargo run -p vox-code-audit --bin toestub -- <PATH>         # explicit scan root
```
```

Also fix the reference to the retired script a few lines below (governance.md line 30):

Old:
```
GitHub CI runs the **scoped** TOESTUB pass above (`toestub_scoped.sh`). When you run **`vox stub-check`**, it exits non-zero on error/critical findings for the configured scan (see CLI flags in [`ref-cli.md`](../src/ref-cli.md)).
```
New:
```
GitHub CI runs the **scoped** TOESTUB pass above (`vox ci toestub-scoped`). When you run **`vox stub-check`**, it exits non-zero on error/critical findings for the configured scan (see CLI flags in [`ref-cli.md`](../src/ref-cli.md)).
```

- [ ] **Step 2: Fix `docs/agents/cli-toolchain.md:78`**

Old:
```
Canonical CI/agents path: **`bash scripts/quality/toestub_scoped.sh`** (or `cargo run -p vox-code-audit --bin toestub -- <PATH>`).
```
New:
```
Canonical CI/agents path: **`vox ci toestub-scoped`** (or `cargo run -p vox-code-audit --bin toestub -- <PATH>`).
```

- [ ] **Step 3: Fix `docs/agents/orchestrator.md:59`**

Old:
```
- TOESTUB analysis → `bash scripts/quality/toestub_scoped.sh` or `cargo run -p vox-code-audit --bin toestub -- <PATH>`; optional `vox stub-check` when built with **`--features stub-check`** (see `docs/src/reference/cli.md`).
```
New:
```
- TOESTUB analysis → `vox ci toestub-scoped` or `cargo run -p vox-code-audit --bin toestub -- <PATH>`; optional `vox stub-check` when built with **`--features stub-check`** (see `docs/src/reference/cli.md`).
```

- [ ] **Step 4: Fix `docs/src/reference/cli.md:622`**

Old:
```
**CI / parity:** prefer **`vox ci toestub-scoped`** (default scan root `crates/vox-repository`) — same policy surface as GitHub Actions. Use **`vox stub-check …`** for interactive or repo-wide scans when you need clap flags (format, baselines, Ludus, etc.). Optional thin shell: `scripts/quality/toestub_scoped.sh` delegates to `vox ci toestub-scoped`; the standalone **`toestub`** crate binary remains available for advanced tooling.
```
New:
```
**CI / parity:** prefer **`vox ci toestub-scoped`** (default scan root `crates/vox-repository`) — same policy surface as GitHub Actions. Use **`vox stub-check …`** for interactive or repo-wide scans when you need clap flags (format, baselines, Ludus, etc.). The standalone **`toestub`** crate binary remains available for advanced tooling.
```

- [ ] **Step 5: Verify no stale references remain**

Run: `grep -rln "scripts/quality/toestub_scoped.sh" docs/`
Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add docs/agents/governance.md docs/agents/cli-toolchain.md docs/agents/orchestrator.md docs/src/reference/cli.md
git commit -m "docs: retired scripts/quality/toestub_scoped.sh -- use vox ci toestub-scoped"
```

---

### Task 4: Fix stale `.cursor/rules` file count in `AGENTS.md`

`AGENTS.md:276` claims "four `.mdc` rule files." There are nine: `build-environment.mdc`, `ci-runner-convention.mdc`, `cli-command-registry.mdc`, `cross-platform-source-hygiene.mdc`, `data-storage-policy.mdc`, `documentation-policy.mdc`, `retired-surfaces.mdc`, `secrets-policy.mdc`, `voxscript-first-automation.mdc` (verified via `Glob .cursor/rules/*.mdc`).

**Files:**
- Modify: `AGENTS.md:276`

- [ ] **Step 1: Fix the line**

Old:
```
For Cursor-specific rules see [`.cursor/rules/`](.cursor/rules/) — four `.mdc` rule files control build environment, CI runner conventions, CLI registry, and source hygiene.
```
New:
```
For Cursor-specific rules see [`.cursor/rules/`](.cursor/rules/) — nine `.mdc` rule files covering build environment, CI runner conventions, CLI command registry, cross-platform source hygiene, data storage policy, documentation policy, retired surfaces, secrets policy, and VoxScript-first automation.
```

- [ ] **Step 2: Verify the count matches reality**

Run: `ls .cursor/rules/*.mdc | wc -l`
Expected: `9`

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: fix stale .cursor/rules file count (four -> nine) in AGENTS.md"
```

---

### Task 5: Add missing Perennial Bug Pattern (parallel-agent fmt drift)

Git history mining (8 occurrences: "rustfmt cleanup accumulated across parallel subagent commits," "rustfmt cleanup left uncommitted by concurrent agent work," etc.) found a recurring mistake class not yet in `AGENTS.md`'s Perennial Bug Patterns section.

**Files:**
- Modify: `AGENTS.md` (end of §Perennial Bug Patterns, before the "Coverage of these classes..." closing line)

- [ ] **Step 1: Insert the new bullet**

Old (the last bullet + the closing paragraph, exact existing text):
```
- **`vox-gui` sidecar missing in a fresh worktree.** `cargo build`/`test -p vox-gui` fails inside `tauri-build` ("resource path ... doesn't exist") the first time ANY `git worktree add` builds it — each worktree gets its own `target/` (per-worktree by design, see `.cargo/config.toml`), so the release `vox` binary Tauri bundles as an `externalBin` sidecar doesn't exist yet. `crates/vox-gui/build.rs` and `vox doctor` both name the missing path and the fix: run `vox run scripts/gui-build.vox` (or `cargo build -p vox-cli --release` then copy `target/release/vox[.exe]` to the triple-suffixed sidecar path) once per worktree before building `vox-gui`.

Coverage of these classes by detector + severity + enforcement point (and the still-open gaps) is tracked in [`detector-coverage-ledger.md`](docs/src/contributors/detector-coverage-ledger.md) — add a row when you add a detector.
```
New:
```
- **`vox-gui` sidecar missing in a fresh worktree.** `cargo build`/`test -p vox-gui` fails inside `tauri-build` ("resource path ... doesn't exist") the first time ANY `git worktree add` builds it — each worktree gets its own `target/` (per-worktree by design, see `.cargo/config.toml`), so the release `vox` binary Tauri bundles as an `externalBin` sidecar doesn't exist yet. `crates/vox-gui/build.rs` and `vox doctor` both name the missing path and the fix: run `vox run scripts/gui-build.vox` (or `cargo build -p vox-cli --release` then copy `target/release/vox[.exe]` to the triple-suffixed sidecar path) once per worktree before building `vox-gui`.
- **Parallel-agent fmt drift.** When multiple agents/worktrees touch overlapping crates concurrently, `rustfmt` drift from one session's edits routinely lands unformatted in another's commit. Before merging work assembled from parallel sessions, run `vox run scripts/fmt.vox` (or `VOX_FMT_CHECK=1 vox run scripts/fmt.vox` to check only).

Coverage of these classes by detector + severity + enforcement point (and the still-open gaps) is tracked in [`detector-coverage-ledger.md`](docs/src/contributors/detector-coverage-ledger.md) — add a row when you add a detector.
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs: add parallel-agent fmt drift to AGENTS.md Perennial Bug Patterns"
```

---

### Task 6: Switch `CLAUDE.md` to the native `@AGENTS.md` import

Verified against Claude Code's official memory docs (`code.claude.com/docs/en/memory`, fetched live): `CLAUDE.md` files support `@path/to/import` syntax, expanded and loaded into context at launch. The docs give this exact repo's situation as the canonical example:

```markdown
@AGENTS.md

## Claude Code

Use plan mode for changes under `src/billing/`.
```

This replaces the current unenforced prose ("This project uses AGENTS.md ... required reading first") with an actual import, and removes an inline restatement of 4 AGENTS.md policies (VoxScript-only automation, frontmatter, `cargo fmt --all` ban, where-things-live) that CLAUDE.md was independently repeating.

**Files:**
- Modify: `CLAUDE.md` (full rewrite, 21 lines → 13 lines)

- [ ] **Step 1: Rewrite `CLAUDE.md`**

Old (full current file):
```markdown
# Claude Code Overlay

This project uses `AGENTS.md` as the cross-tool policy surface (required reading first).

## Claude-specific additions

These are behaviors specific to Claude Code. **All cross-tool rules live in [`AGENTS.md`](AGENTS.md) — read it first.** In particular AGENTS.md is normative for: the "where does this code go" lookup (`where-things-live.md`), required Markdown frontmatter under `docs/src/`, VoxScript-only automation (no new `.ps1`/`.sh`/`.py`), and the `cargo fmt --all` ban (use `vox run scripts/fmt.vox` / `cargo fmt -p <crate>`).

- If you open a `.vox` file, treat it as Vox language source — not Rust, not TypeScript.
- Honor `// vox:skip` annotations in code blocks; do not validate those against the compiler. (Prefer making fenced `vox` blocks compile; use `// vox:skip` + a reason only for genuine out-of-file excerpts.)
- Do not store project-specific research in your IDE/agent memory; write it to `docs/src/architecture/` instead (with frontmatter).

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
```
New (full replacement file):
```markdown
@AGENTS.md

## Claude Code

These are behaviors specific to Claude Code, in addition to the imported `AGENTS.md` above.

- If you open a `.vox` file, treat it as Vox language source — not Rust, not TypeScript.
- Honor `// vox:skip` annotations in code blocks; do not validate those against the compiler. (Prefer making fenced `vox` blocks compile; use `// vox:skip` + a reason only for genuine out-of-file excerpts.)
- Do not store project-specific research in your IDE/agent memory; write it to `docs/src/architecture/` instead (with frontmatter).

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
```

- [ ] **Step 2: Verify the import loads**

This can only be verified interactively in a live Claude Code session (not scriptable): open a fresh session in this worktree, run `/context`, and confirm `AGENTS.md` (via the `@AGENTS.md` import in `CLAUDE.md`) appears under **Memory files**. Note this as a manual verification step in the task's completion notes if run non-interactively — do not skip recording that it wasn't machine-verified.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): use native @AGENTS.md import instead of prose pointer"
```

---

### Task 7: Fix active contradiction in the retired-surfaces "memory API" rows

`AGENTS.md`'s canonical Retired Surfaces table says:
- `recall()` / `recall_async()` (deprecated memory reads) → `MemoryManager::lookup_fact_by_key` (async) or RAG / retrieval bundle
- `sync_to_db()` bulk-syncs MEMORY.md → DB only — **not** a drop-in replacement for `MemoryManager::persist_fact`

Two other currently-loaded files say the exact opposite: that `recall_async()` is the canonical replacement for `recall()`, and that `sync_to_db()` is the canonical replacement for `persist_fact()`. An agent that reads only one of these files (a realistic scenario for `.cursor/rules/*.mdc`, which Cursor may load without also loading root `AGENTS.md`) would call a deprecated/wrong API. This is real, present drift, not just duplication — the crate-name rows in both tables are correct and unaffected; only the two memory-API rows need fixing.

**Files:**
- Modify: `.cursor/rules/retired-surfaces.mdc:17-18`
- Modify: `docs/src/reference/agent-quick-reference.md:43-44`

- [ ] **Step 1: Fix `.cursor/rules/retired-surfaces.mdc`**

Old (full file):
```markdown
---
description: Prevent use of retired Vox crates, env vars, and API symbols
alwaysApply: true
---
# Retired surfaces (LLM Guard)

NEVER use these symbols — they cause broken integration:

| Retired | Use instead |
|---|---|
| `vox-dei` (orchestrator crate) | `vox-orchestrator` |
| `vox-ars` | `vox-openclaw-runtime` |
| `vox-ludus` | `vox-gamify` |
| `vox-lexer`, `vox-parser`, `vox-hir`, `vox-typeck` | `vox-compiler` |
| `@component fn Name()` | `component Name() {}` |
| `TURSO_URL` / `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| `recall()` | `recall_async()` |
| `persist_fact()` | `sync_to_db()` |
```
New (full file):
```markdown
---
description: Prevent use of retired Vox crates, env vars, and API symbols
alwaysApply: true
---
# Retired surfaces (LLM Guard)

NEVER use these symbols — they cause broken integration:

| Retired | Use instead |
|---|---|
| `vox-dei` (orchestrator crate) | `vox-orchestrator` |
| `vox-ars` | `vox-openclaw-runtime` |
| `vox-ludus` | `vox-gamify` |
| `vox-lexer`, `vox-parser`, `vox-hir`, `vox-typeck` | `vox-compiler` |
| `@component fn Name()` | `component Name() {}` |
| `TURSO_URL` / `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| `recall()` / `recall_async()` (deprecated memory reads) | `MemoryManager::lookup_fact_by_key` (async) or RAG/retrieval bundle |

Memory-write APIs (`persist_fact` vs `sync_to_db`) are not a simple retirement pair — see `AGENTS.md` §Retired Surfaces for the current distinction before touching memory-write code.
```

- [ ] **Step 2: Fix `docs/src/reference/agent-quick-reference.md`**

Old (lines 33-45, full table):
```markdown
## Retired Surfaces Quick Map

| Retired / Deprecated | Canonical Replacement (Use Instead) |
|---|---|
| Legacy orchestrator packaging | `vox-orchestrator` |
| Legacy ARS/OpenClaw predecessor crate | `vox-openclaw-runtime` |
| Legacy gamification crate label | `vox-gamify` |
| Legacy split compiler crates | `vox-compiler` |
| Legacy React-interop component decorator | `component Name() {}` |
| Legacy Turso-prefixed DB env aliases | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| Sync recall API | `recall_async()` |
| Persist-fact API | `sync_to_db()` |
```
New:
```markdown
## Retired Surfaces Quick Map

| Retired / Deprecated | Canonical Replacement (Use Instead) |
|---|---|
| Legacy orchestrator packaging | `vox-orchestrator` |
| Legacy ARS/OpenClaw predecessor crate | `vox-openclaw-runtime` |
| Legacy gamification crate label | `vox-gamify` |
| Legacy split compiler crates | `vox-compiler` |
| Legacy React-interop component decorator | `component Name() {}` |
| Legacy Turso-prefixed DB env aliases | `VOX_DB_URL` / `VOX_DB_TOKEN` |
| `recall()` / `recall_async()` (deprecated memory reads) | `MemoryManager::lookup_fact_by_key` (async) or RAG/retrieval bundle |

Memory-write APIs (`persist_fact` vs `sync_to_db`) are not a simple retirement pair — see `AGENTS.md` §Retired Surfaces before touching memory-write code.
```

- [ ] **Step 3: Verify no lingering wrong claims**

Run: `grep -rn "recall_async()\` |\`sync_to_db()\`" .cursor/rules/retired-surfaces.mdc docs/src/reference/agent-quick-reference.md`
Expected: no output (both files no longer assert `recall_async()`/`sync_to_db()` as bare "canonical replacement" table rows).

- [ ] **Step 4: Commit**

```bash
git add .cursor/rules/retired-surfaces.mdc docs/src/reference/agent-quick-reference.md
git commit -m "fix(docs): correct contradictory recall/persist_fact retirement claims"
```

---

### Task 8: Trim duplicated VoxScript execution-tier table in `.cursor/rules/voxscript-first-automation.mdc`

This file already cites `AGENTS.md §VoxScript-First Glue Code` as "authoritative policy" in its own References section, but still restates the full 3-row execution-tier table above it. Keep the correct/wrong code examples (genuinely useful standalone context for a Cursor session) and the bootstrap/security sections (not duplicated elsewhere); replace only the redundant table.

**Files:**
- Modify: `.cursor/rules/voxscript-first-automation.mdc:26-32`

- [ ] **Step 1: Replace the Execution tiers section**

Old:
```markdown
## Execution tiers

| Need | Command |
|---|---|
| Pure computation | `vox run --interp` |
| File I/O or subprocess | `vox run` (native, cached) |
| Untrusted execution | `vox run --isolation wasm` |
```
New:
```markdown
## Execution tiers

Full tier table (interp / native / wasm-isolated, and when to use each): `AGENTS.md §VoxScript-First Glue Code` — authoritative, do not restate here.
```

- [ ] **Step 2: Commit**

```bash
git add .cursor/rules/voxscript-first-automation.mdc
git commit -m "docs(cursor): trim duplicated VoxScript tier table to a pointer"
```

---

### Task 9: Trim duplicated god-object table in `docs/agents/cli-toolchain.md`

`governance.md` §God Object Limit is the multi-tier SSOT (300 soft / 400 warn / 500 error lines, or 12 methods). `cli-toolchain.md` restates a flattened single-tier version that's less accurate (drops the 300/400 tiers).

**Files:**
- Modify: `docs/agents/cli-toolchain.md:86-90`

- [ ] **Step 1: Replace the restated thresholds**

Old:
```markdown
See `docs/src/reference/cli.md` for flags (`--suggest-fixes`, not `--fix`).
God-object thresholds (from `vox-schema.json`):
- Files > 500 lines → warning
- Structs > 12 methods → warning
- Directories > 20 files → warning
```
New:
```markdown
See `docs/src/reference/cli.md` for flags (`--suggest-fixes`, not `--fix`).
God-object / sprawl thresholds: see [`governance.md` §God Object Limit](governance.md#god-object-limit-multi-tier) (multi-tier: 300 soft / 400 warn / 500 error lines, or 12 methods; sprawl: 20 files/dir) — do not restate the numbers here, they drift.
```

- [ ] **Step 2: Verify the anchor resolves**

Run: `grep -n "## God Object Limit" docs/agents/governance.md`
Expected: one match (`## God Object Limit (Multi-Tier)`), confirming the link target exists.

- [ ] **Step 3: Commit**

```bash
git add docs/agents/cli-toolchain.md
git commit -m "docs: point cli-toolchain.md god-object thresholds at governance.md SSOT"
```

---

### Task 10: Trim `AGENTS.md` §Versioning Policy

Cut the "Build metadata injection" paragraph (dev-reference for authoring a new version-displaying binary — rare, and discoverable from the `vox-build-meta` crate itself when actually needed) and collapse the version-scheme table into one inline sentence. Keep every agent-actionable rule: single source of truth, don't hand-bump PATCH, update CHANGELOG after a MAJOR.MINOR bump (and why), don't maintain a separate doc version.

**Files:**
- Modify: `AGENTS.md` §Versioning Policy (~26 lines → ~11 lines)

- [ ] **Step 1: Replace the section**

Old (full existing section):
```markdown
## Versioning Policy (SSOT)

The workspace uses a **single source of truth** for all crate versions:

```toml
# Cargo.toml [workspace.package]
version = "0.6.0"
```

All first-party crates inherit this via `version.workspace = true`. Plugin crates (`vox-plugin-*`) maintain independent versions on their own release cadence.

**Version scheme:** `MAJOR.MINOR.PATCH+build.N (GITHASH)`

| Component | Owner | How it changes |
|---|---|---|
| `MAJOR.MINOR` | Human (manual bump in `Cargo.toml`) | Breaking or feature releases |
| `PATCH` | Human (manual bump) or git-cliff on tag | Bugfix releases |
| `+build.N` | Automatic (`vox-build-meta` in `build.rs`) | Every merged commit |
| `(GITHASH)` | Automatic (`vox-build-meta`) | Every commit, diagnostics only |

**After bumping `MAJOR.MINOR`:** update `CHANGELOG.md` with a `## [X.Y.Z] - YYYY-MM-DD` entry. This date is the threshold used by `vox-arch-check` Rule 6 (staleness check) to flag crates that haven't changed in the new release cycle.

**Documentation version = compiler version.** Do not maintain a separate "doc version" (e.g., 0.8). All release notes live under `docs/news/YYYY-MM-DD-vX.Y.Z-release.md` and reference the same `X.Y.Z` as `Cargo.toml`.

**Build metadata injection:** Add `vox-build-meta` to `[build-dependencies]` and call `vox_build_meta::emit()` from `build.rs` in any binary that displays its version. The `VOX_BUILD_NUMBER` and `VOX_GIT_HASH` env vars are then available via `env!()`.

**Staleness check:** `vox-arch-check` Rule 6 warns when a crate has had no commits since the last `CHANGELOG.md` release date. Use `staleness_exempt = true` in `layers.toml` for crates that are intentionally stable (e.g., `workspace-hack`, build helpers).
```
New:
```markdown
## Versioning Policy (SSOT)

Single source of truth: `Cargo.toml [workspace.package] version`. All first-party crates inherit it via `version.workspace = true`; plugin crates (`vox-plugin-*`) version independently.

**Version scheme:** `MAJOR.MINOR.PATCH+build.N (GITHASH)` — `MAJOR.MINOR`/`PATCH` are human-bumped (or git-cliff on tag for `PATCH`); `+build.N`/`(GITHASH)` are injected automatically by `vox-build-meta` in `build.rs` on every commit.

**After bumping `MAJOR.MINOR`:** update `CHANGELOG.md` with a `## [X.Y.Z] - YYYY-MM-DD` entry — `vox-arch-check` Rule 6 uses this date to flag crates that haven't changed in the new release cycle (mark intentionally-stable crates `staleness_exempt = true` in `layers.toml`).

**Documentation version = compiler version.** Do not maintain a separate "doc version." Release notes live under `docs/news/YYYY-MM-DD-vX.Y.Z-release.md`, same `X.Y.Z` as `Cargo.toml`.
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs: trim AGENTS.md Versioning Policy to agent-actionable rules"
```

---

### Task 11: Fix internal contradiction + trim `AGENTS.md` §Grammar Unification

Reading the full section closely surfaces a real self-contradiction: lines 232 and 237 label `workflow`/`activity`/`@durable`/`@scheduled` as "Reserved (ADR-028, not yet implemented)," but the "Implementation status" paragraph a few lines later says ADR-041 superseded ADR-028 and these are now "stable public-grammar features" with the reservation gate removed. The more detailed, ADR-cited paragraph is treated as current; the earlier "Reserved... not yet implemented" labels are the stale half and are corrected to match. The implementation-status paragraph itself is also trimmed (drop the meta-commentary about which doc-audit tool catches drift here — not an agent-actionable rule).

**Files:**
- Modify: `AGENTS.md` §Grammar Unification (lines ~230-238, ~253-261)

- [ ] **Step 1: Fix the bare-keyword and decorator reserved-lists**

Old:
```markdown
**Bare-keyword blocks** (each opens a scope with its own rules):
`type`, `fn`, `component`, `state_machine`, `routes`, `module`, `actor`.
**Reserved (ADR-028, not yet implemented):** `workflow`, `activity`.

**Decorators** (modifiers composed on top of a declaration):
`@table`, `@query`, `@mutation`, `@server`, `@pure`, `@deprecated`, `@require`, `@mcp.tool`,
`@v0`, `@test`.
**Reserved (ADR-028, not yet implemented):** `@durable`, `@scheduled`.
**Removed in v0.6.0:** `@endpoint` (see §Retired Surfaces).
```
New:
```markdown
**Bare-keyword blocks** (each opens a scope with its own rules):
`type`, `fn`, `component`, `state_machine`, `routes`, `module`, `actor`, `workflow`, `activity`
(the last two are stable per ADR-041, superseding the old ADR-028 reservation gate — see Implementation status below).

**Decorators** (modifiers composed on top of a declaration):
`@table`, `@query`, `@mutation`, `@server`, `@pure`, `@deprecated`, `@require`, `@mcp.tool`,
`@v0`, `@test`, `@durable`, `@scheduled` (the last two stable per ADR-041 — see Implementation status below).
**Removed in v0.6.0:** `@endpoint` (see §Retired Surfaces).
```

- [ ] **Step 2: Trim the Implementation status paragraph**

Old:
```markdown
**Implementation status (ADR-041 supersedes ADR-028).** `actor`, `workflow`, `activity`,
`@durable`, and `@scheduled` are **stable public-grammar features** backed by a durable runtime
for the supported subset; the old `E028` reservation gate is **removed**, and out-of-subset
behavior is now policed by the determinism lint, not a reservation gate. Supported subset +
contract: [ADR-019](docs/src/adr/019-durable-workflow-journal-contract-v1.md),
[ADR-021](docs/src/adr/021-generated-workflow-durability-parity.md),
[ADR-041](docs/src/adr/041-durable-functions-completion-2026.md). Drift between this section and
`pipeline.rs` is caught by the [`docs-reality-audit-program`](docs/src/contributors/docs-reality-audit-program.md)
(and the planned `vox ci retirement-audit` gate, [CR-L6](docs/src/architecture/v1-llm-target-implementation-plan-2026.md) P1.3).
```
New:
```markdown
**Implementation status.** `actor`/`workflow`/`activity` and `@durable`/`@scheduled` are stable, backed by a durable runtime for the supported subset (ADR-041 supersedes the old ADR-028 reservation gate — out-of-subset behavior is now policed by the determinism lint, not a reservation gate). Contract: [ADR-019](docs/src/adr/019-durable-workflow-journal-contract-v1.md), [ADR-021](docs/src/adr/021-generated-workflow-durability-parity.md), [ADR-041](docs/src/adr/041-durable-functions-completion-2026.md).
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "fix(docs): resolve stale 'reserved, not implemented' vs ADR-041 contradiction in AGENTS.md"
```

---

### Task 12: Trim `AGENTS.md` §PR & Review Discipline

Keep all 4 actionable rules and the one-line takeaway verbatim; cut the rate-limit-number rationale paragraph and the "Repo policy (enforced by...)" explanatory paragraph down to one clause each.

**Files:**
- Modify: `AGENTS.md` §PR & Review Discipline (~26 lines → ~14 lines)

- [ ] **Step 1: Replace the section**

Old (full existing section):
```markdown
## PR & Review Discipline (Required, Cross-Tool)

> **Canonical config:** `.coderabbit.yaml` (repo root; CodeRabbit reads it from the **default branch**).

Automated PR review (CodeRabbit) is **rate-limited and shared**: every branch, every
Claude Code tab/worktree, and every IDE you use pushes as the **same GitHub identity**,
so they all draw from **one** per-developer review allowance (Pro tier: ~5 PR reviews/hour,
refilling over time, throttled further under sustained bursts). Treating every `git push`
as a review request drains that allowance in minutes and stalls all your other work.

**Repo policy (enforced by `.coderabbit.yaml`):** `auto_review.auto_incremental_review:
false` — CodeRabbit reviews a PR **once when it opens** and does **not** auto-review
subsequent pushes. Re-review is **on demand only**.

Therefore, across **all** branches/tabs/IDEs:

- **Batch commits; push once when the PR is review-ready** — not after every commit. (This
  is the same "don't re-push to iterate" rule as the CI gate tiers above, applied to review.)
- **Request re-review explicitly** by commenting **`@coderabbitai review`** on the PR when
  you actually want fresh eyes — never by pushing repeatedly.
- **Don't open a PR before the work is review-ready.** If you must push early, keep the PR a
  **Draft** (drafts are not auto-reviewed; `auto_review.drafts: false`).
- The `vox ci pre-push` hook prints an **advisory** reminder when you re-push a branch that
  already has an upstream (the proxy for an open PR). It never blocks the push.

One-line takeaway: **one deliberate review per ready PR**, not one per push.
```
New:
```markdown
## PR & Review Discipline (Required, Cross-Tool)

> **Canonical config:** `.coderabbit.yaml` (repo root; CodeRabbit reads it from the **default branch**).

Automated PR review (CodeRabbit) is **rate-limited and shared** across every branch, worktree, and IDE (same GitHub identity, one per-developer allowance) — and `.coderabbit.yaml` sets `auto_review.auto_incremental_review: false`, so a PR is reviewed **once on open**, never automatically on later pushes.

Therefore, across **all** branches/tabs/IDEs:

- **Batch commits; push once when the PR is review-ready** — not after every commit.
- **Request re-review explicitly** by commenting **`@coderabbitai review`** — never by pushing repeatedly.
- **Don't open a PR before it's review-ready.** If you must push early, keep it a **Draft** (`auto_review.drafts: false`).
- `vox ci pre-push` prints an **advisory** reminder on re-push to a branch with an upstream; it never blocks.

One-line takeaway: **one deliberate review per ready PR**, not one per push.
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs: trim AGENTS.md PR & Review Discipline rationale, keep all rules"
```

---

### Task 13: Add a CI drift-guard for the 4 fixed stale references

`crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` already has a live, wired-in mechanism for exactly this: `check_stale_doc_and_workflow_refs` (called from `check_docs_ssot`, part of the `ssot-drift` fast-tier gate) scans `docs/**` + root `AGENTS.md`/`README.md`/`CONTRIBUTING.md` for banned substrings. Extend its existing `DOC_BANNED` array with the 4 strings fixed in Tasks 1-4, so if any of them regresses, `vox ci pre-push` (fast tier, ≤60s) catches it immediately instead of waiting for another audit.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:304`
- Test: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` (new `#[cfg(test)] mod tests` at end of file)

- [ ] **Step 1: Write the failing test**

Add at the end of `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` (after the final `}` closing `check_codex_ssot`):

```rust

#[cfg(test)]
mod stale_ref_guard_tests {
    use super::*;
    use std::fs;

    fn write_agents_md(root: &Path, body: &str) {
        // check_stale_doc_and_workflow_refs only scans the root AGENTS.md/README.md/
        // CONTRIBUTING.md branch when `docs/` exists (see the `if docs_dir.is_dir()`
        // guard around that whole block) -- an empty docs/ dir is enough to enter it.
        fs::create_dir_all(root.join("docs")).expect("create docs dir");
        fs::write(root.join("AGENTS.md"), body).expect("write AGENTS.md");
    }

    #[test]
    fn flags_stale_secrets_spec_rs_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
        );
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale spec.rs path must be flagged");
        assert!(err.to_string().contains("vox-secrets/src/spec.rs"));
    }

    #[test]
    fn flags_stale_toestub_scoped_sh() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(tmp.path(), "Run `scripts/quality/toestub_scoped.sh`.\n");
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale toestub_scoped.sh reference must be flagged");
        assert!(err.to_string().contains("toestub_scoped.sh"));
    }

    #[test]
    fn flags_stale_mdc_count_phrase() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(tmp.path(), "There are four `.mdc` rule files here.\n");
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale 'four .mdc rule files' phrase must be flagged");
        assert!(err.to_string().contains("four `.mdc` rule files"));
    }

    #[test]
    fn clean_agents_md_passes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "Define secrets in `crates/vox-secrets/src/spec/`. Run `vox ci toestub-scoped`. Nine `.mdc` rule files.\n",
        );
        check_stale_doc_and_workflow_refs(tmp.path())
            .expect("clean doc content must pass the stale-ref guard");
    }
}
```

- [ ] **Step 2: Run the new tests to confirm they fail**

Run: `cargo test -p vox-cli stale_ref_guard_tests -- --nocapture`
Expected: `flags_stale_secrets_spec_rs_path`, `flags_stale_toestub_scoped_sh`, and `flags_stale_mdc_count_phrase` **FAIL** (the strings aren't banned yet); `clean_agents_md_passes` should already pass.

- [ ] **Step 3: Extend `DOC_BANNED` with the 4 new entries**

Old (`docs.rs:304`):
```rust
    const DOC_BANNED: &[&str] = &["verify_doc_inventory_fresh.py", "populi_release_gate.sh"];
```
New:
```rust
    const DOC_BANNED: &[&str] = &[
        "verify_doc_inventory_fresh.py",
        "populi_release_gate.sh",
        "crates/vox-secrets/src/spec.rs",
        "crates/vox-orchestrator/src/mcp_tools/tools/mod.rs",
        "scripts/quality/toestub_scoped.sh",
        "four `.mdc` rule files",
    ];
```

- [ ] **Step 4: Run the tests again to confirm they pass**

Run: `cargo test -p vox-cli stale_ref_guard_tests -- --nocapture`
Expected: all 4 tests **PASS**.

- [ ] **Step 5: Run the real check against the live repo**

Run: `cargo run -p vox-cli -- ci ssot-drift`
Expected: passes (exits 0) — Tasks 1-4 already removed every occurrence of these 4 strings from `docs/` and root `AGENTS.md`. If this fails, one of Tasks 1-4's `grep` verification steps missed a hit; go back and fix it before proceeding.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs
git commit -m "test(vox-cli): ban the 4 fixed stale doc references from regressing"
```

---

### Task 14: Final verification sweep

- [ ] **Step 1: Re-confirm zero stale-reference hits across the whole repo (not just docs/)**

Run each and confirm no unexpected output (historical files under `docs/superpowers/plans/`, `docs/src/archive/`, and `CHANGELOG.md` are expected/allowed to still mention old paths — they're point-in-time historical records, not living guidance):

```bash
grep -rn "crates/vox-secrets/src/spec.rs" AGENTS.md CLAUDE.md GEMINI.md .cursor .github docs/agents docs/src/reference docs/src/contributors crates/vox-code-audit crates/vox-cli/src/commands/ci
```
```bash
grep -rn "mcp_tools/tools/mod.rs" docs/agents docs/src
```
```bash
grep -rln "scripts/quality/toestub_scoped.sh" docs/
```
```bash
grep -n "four .mdc" AGENTS.md
```

- [ ] **Step 2: Measure the AGENTS.md size reduction**

Run: `wc -l -w AGENTS.md`
Record the before (556 lines / 5952 words, from the audit) vs. after count in the final task summary — this is the headline number for the "save tokens" goal.

- [ ] **Step 3: Run the fast pre-push gate**

Run: `cargo run -p vox-cli -- ci pre-push`
Expected: passes. This exercises `ssot-drift` (which now includes Task 13's new checks), fmt, line-endings, and scoped doc lint.

- [ ] **Step 4: Run the complete tier (clippy + full doc lint) since Rust files were touched**

Run: `cargo run -p vox-cli -- ci pre-push --complete`
Expected: passes.

- [ ] **Step 5: Spot-check every new cross-reference link resolves**

Manually open (or `grep -n "^##"`) `docs/agents/governance.md` to confirm the `#god-object-limit-multi-tier` anchor slug matches what Task 9 linked, and confirm `docs/src/contributors/agent-instruction-architecture.md` (referenced by the unchanged bottom line of `CLAUDE.md`) still exists.

- [ ] **Step 6: No commit for this task** — it's verification-only. If any step fails, return to the relevant task, fix, and re-commit there (do not accumulate fixes into a new "final fixes" commit; keep history attributable per-task).
