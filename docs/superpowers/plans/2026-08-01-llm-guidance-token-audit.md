# LLM Guidance File Token Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix every stale/contradictory reference in Vox's LLM guidance files (`AGENTS.md`, `CLAUDE.md`, `.cursor/rules/*.mdc`, and related docs), collapse standing duplication into single-source pointers, trim non-load-bearing prose from `AGENTS.md`, and add a small CI drift-guard so this class of staleness can't silently recur.

**Architecture:** This is a docs/config content pass, not a code feature. Each task is an independent, small, git-committable edit to one or a few files, verified by `grep` (for stale-reference removal) or a read-through (for trims). The one real code change (Task 13) extends an existing, already-wired Rust CI check with a regression test.

**Tech Stack:** Markdown, `.mdc` (Cursor rules), Rust (`crates/vox-cli`), existing `vox ci` tooling.

---

## Before you start

All file paths below were verified directly against the repo on 2026-08-01 (not taken on faith from an earlier audit pass — one earlier claim, that `.cursor/rules/retired-surfaces.mdc` had "no drift," turned out to be wrong; see Task 7). If any `grep` in a verification step finds unexpected extra hits, stop and re-check rather than force the edit through.

Do all work in this worktree: `C:\Users\Owner\vox\.claude\worktrees\llm-guidance-token-audit-b7e2aa`. Design spec: [`docs/superpowers/specs/2026-08-01-llm-guidance-token-audit-design.md`](../specs/2026-08-01-llm-guidance-token-audit-design.md).

**Shell note:** every verification command in this plan (`grep`, `ls | wc -l`, `test -f`, `cargo test -- --nocapture`, etc.) uses POSIX/bash syntax. Run them through a POSIX-capable shell or tool — this repo's Bash tool, WSL, or Git Bash — not native PowerShell, which has no built-in `grep`/`wc`/POSIX `test` and will fail most of these commands with "not recognized" errors.

**Scope refinements found during verification** (the spec explicitly asked for these to be confirmed at implementation time — this is that confirmation, not a deviation to re-approve):
- `.cursor/rules/data-storage-policy.mdc` is **not** touched. It looked like a 4th copy of the secrets rule in the initial audit, but its content is Tier A/B/C/D data-storage policy with one contextual secrets mention — genuinely unique, not a restatement.
- `.cursor/rules/secrets-policy.mdc` gets only the stale-path fix (Task 1). At 11 lines it's already tight; there's nothing left to reduce.
- `docs/src/contributors/coding-agents.md` is **not** touched for the retired-crate table. It already only cites one example (`vox-dei` → `vox-orchestrator`) and points to `AGENTS.md` for the full table — it was never a full restatement.
- `.cursor/rules/retired-surfaces.mdc` gets a **content fix**, not the "reduce to pointer" originally scoped — verification found two of its rows actively contradict `AGENTS.md` (see Task 7), which is a more important problem than the table being merely duplicative.
- `docs/src/contributors/toestub-contributor-guide.md`'s `security/hardcoded-secret` rule entry (lines 156-171) is **not** reduced to a pointer, despite the spec listing it as a Secrets pointer-reduction target. Task 1 only fixes the stale `spec.rs` path string inside it. The rest of that entry is fix-it guidance (how to route through `vox_secrets::resolve_secret`, false-positive suppression) that this file's own stated purpose is to provide — not a restatement of policy, so left as-is. Its separate `arch/god_object` rule entry (lines 40-43) *is* a pure numeric-threshold restatement and is trimmed in Task 9.
- `docs/src/contributors/continuation-prompt-engineering.md` also restates the secrets rule and is listed in the spec's §2 table as a pointer-reduction target for it — but the spec's own Non-goals section says not to touch this file ("already reviewed and confirmed well-scoped"). The spec contradicts itself here; this plan follows Non-goals and never touches the file.
- Task 11's fix to the Grammar Unification contradiction (see Task 11) goes beyond spec §5's literal scope, which said to trim only the ADR-028/ADR-041 historical-narrative paragraph and keep "the current rule (decorators vs. bare keywords)" unchanged. Verification found the bare-keyword/decorator reserved-lists themselves were the stale half of an active contradiction (labeled "Reserved, not yet implemented" while the same section's narrative says they're stable) — leaving them unedited would have kept the plan's own target of that fix self-contradictory, so both the lists and the narrative are corrected.
- Task 1's own verification grep (Step 8) and Task 14's final sweep are scoped to the 6 files Task 1 fixes plus the specific directories those files live in — not all of `docs/`. `docs/src/architecture/` holds ~7 more implementation-plan/spec docs that also still say `crates/vox-secrets/src/spec.rs` in historical/context-setting prose; like the codebase's own `vox-ml-cli` precedent (`docs.rs:306-307`: implementation-plan docs legitimately reference real-but-narrow-audience paths), these are intentionally out of scope for this guidance-tier pass, not missed.

## Parallel-safe execution groups

Tasks 1-14 can be dispatched as concurrent sub-agents where their file sets are disjoint; several must run strictly in order because they edit the same file (mostly `AGENTS.md`) or depend on an earlier task's fix already being committed. Three tasks below involve a judgment call unsuited to unattended background execution — route those to a human or a foreground session instead.

**Wave 1 (parallel, with one internal ordering note below):** Task 1, Task 2, Task 6, Task 8, Task 9 — disjoint files (Task 1: `AGENTS.md` + `.cursor/rules/secrets-policy.mdc` + `docs/src/reference/agent-quick-reference.md` + `docs/src/contributors/toestub-contributor-guide.md` + `crates/vox-code-audit/.../env_secret_shape.rs` + `crates/vox-cli/.../guards.rs`; Task 2: `docs/agents/orchestrator.md`; Task 6: `CLAUDE.md`; Task 8: `.cursor/rules/voxscript-first-automation.mdc`; Task 9: `docs/agents/cli-toolchain.md` + `docs/src/contributors/coding-agents.md` + `docs/src/contributors/toestub-contributor-guide.md`). Note Task 1 and Task 9 both touch `toestub-contributor-guide.md` (different, non-overlapping line ranges — Task 1 fixes line 167, Task 9 fixes lines 40-43); if dispatching as separate sub-agents, still serialize Task 9 after Task 1 to avoid a same-file write race.

**Wave 2 (after Wave 1 is committed):** Task 3, Task 4, Task 7 — run in parallel with each other. Task 3 touches `docs/agents/governance.md` + `docs/agents/cli-toolchain.md` + `docs/agents/orchestrator.md` + `docs/src/reference/cli.md`, so it must follow Task 2 and Task 9 (both edit files Task 3 also edits). Task 4 edits `AGENTS.md` and must follow Task 1 (also edits `AGENTS.md`). Task 7 edits `.cursor/rules/retired-surfaces.mdc` + `docs/src/reference/agent-quick-reference.md` and must follow Task 1 (also edits `agent-quick-reference.md`). Task 3, Task 4, and Task 7 share no files with each other.

**Wave 3 (after Wave 2 is committed):** Task 5, Task 13 — run in parallel with each other. Task 5 edits `AGENTS.md` and must follow Task 4 (same file). Task 13 edits only `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` and must follow Tasks 1-4 (its new tests and its Step 5 live-repo check assert those 4 stale-reference fixes are already committed).

**Wave 4 (strictly sequential, starts once Wave 3's Task 5 is committed):** Task 10 → Task 11 → Task 12, one at a time — each edits `AGENTS.md` and must wait for the previous to commit before starting.

**Full `AGENTS.md`-touching chain (all touch `AGENTS.md`, must run one at a time in this order):** Task 1 → Task 4 → Task 5 → Task 10 → Task 11 → Task 12. Each of these must be fully committed before the next starts, regardless of what else runs in parallel around them.

**Must run last:** Task 14 depends on every other task (1-13) being committed — it's a repo-wide grep sweep plus a full `vox ci pre-push --complete` run, and is only meaningful (and only avoids false failures) once every prior edit has landed.

**Route to a human/foreground session, not a background sub-agent:**
- **Task 6, Step 2** — verifying the `@AGENTS.md` import actually loads requires an interactive Claude Code session (`/context`); it cannot be scripted or run by an unattended sub-agent.
- **Task 13, Step 5** — if the live `vox ci ssot-drift` check fails, the plan's own remediation ("go back and fix it before proceeding") requires diagnosing *which* earlier task's fix was incomplete and reopening it — a judgment call that risks an incorrect follow-up edit if made without foreground review.
- **Task 14** — the final sweep requires judging which grep hits are expected historical noise (`docs/superpowers/plans/`, `docs/src/archive/`, `CHANGELOG.md`) versus a real regression from Tasks 1-13, and Step 5 explicitly calls for manually opening a file to confirm an anchor slug. A wrongly-dismissed "expected" hit could mask a genuine regression.

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

Run: `grep -n "vox-secrets/src/spec.rs" AGENTS.md .cursor/rules/secrets-policy.mdc docs/src/reference/agent-quick-reference.md docs/src/contributors/toestub-contributor-guide.md crates/vox-code-audit/src/detectors/env_secret_shape.rs crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`
Expected: no output (zero matches). (Scoped to exactly the 6 files this task fixes — a repo-wide sweep for any *other* file that still mentions the stale path happens in Task 14, which has to exclude historical/point-in-time docs that legitimately still reference it; running that exclusion-aware sweep here too early would either miss files outside this narrower list or wrongly flag historical docs as regressions.)

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

Run: `grep -n "mcp_tools/tools/mod.rs" docs/agents/orchestrator.md`
Expected: no output. (Scoped to the one file this task fixes — a repo-wide `grep -r docs/` at this point also matches this plan document itself, which quotes the old string verbatim as Old/New diff text; that repo-wide sweep, with the necessary exclusions, happens in Task 14.)

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

Run: `grep -ln "scripts/quality/toestub_scoped.sh" docs/agents/governance.md docs/agents/cli-toolchain.md docs/agents/orchestrator.md docs/src/reference/cli.md`
Expected: no output. (Scoped to exactly the 4 files this task fixes — a repo-wide `grep -r docs/` at this point also matches `docs/src/archive/research-2026-q1/script-surface-audit.md` and this plan document's own Old/New quotes, both expected to still mention the retired script; that exclusion-aware repo-wide sweep happens in Task 14.)

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

Verified against Claude Code's official memory docs (`code.claude.com/docs/en/memory`, fetched live): `CLAUDE.md` files support `@path/to/import` syntax, expanded and loaded into context at launch. The docs' generic `### AGENTS.md` section describes exactly this repo's situation (a repo that already uses `AGENTS.md` for other tools) and gives this illustrative example of the import syntax — it is not written about Vox specifically (`src/billing/` and "plan mode" are the docs' own generic filler, not anything that exists in this repo), but the import mechanism it documents is what Task 6 applies here:

```markdown
@AGENTS.md

## Claude Code

Use plan mode for changes under `src/billing/`.
```

This replaces the current unenforced prose ("This project uses AGENTS.md ... required reading first") with an actual import, and removes an inline restatement of 4 AGENTS.md policies (VoxScript-only automation, frontmatter, the banned whole-workspace `rustfmt` invocation, where-things-live) that CLAUDE.md was independently repeating.

**Files:**
- Modify: `CLAUDE.md` (full rewrite, 21 lines → 13 lines)

- [ ] **Step 1: Rewrite `CLAUDE.md`**

Old (full current file — quoted verbatim for diff accuracy; the fmt-related line here only *describes* an existing ban documented in `AGENTS.md`, not an instruction to run anything):
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

This can only be verified interactively in a live Claude Code session (not scriptable): open a fresh session in this worktree, run `/context`, and confirm `AGENTS.md` (via the `@AGENTS.md` import in `CLAUDE.md`) appears under **Memory files**. Note this as a manual verification step in the task's completion notes if run non-interactively — do not skip recording that it wasn't machine-verified. This does **not** block Step 3: if this task is executed non-interactively (e.g. by a background sub-agent), record the note and proceed to commit — do not leave the checkbox unresolved or wait for an interactive session before committing.

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

Memory-write APIs are not a simple retirement pair: for writing facts, use `MemoryManager::persist_fact`; `sync_to_db()` bulk-syncs `MEMORY.md` → DB only and is **not** a drop-in replacement for `persist_fact`.
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

Memory-write APIs are not a simple retirement pair: for writing facts, use `MemoryManager::persist_fact`; `sync_to_db()` bulk-syncs `MEMORY.md` → DB only and is **not** a drop-in replacement for `persist_fact`.
```

- [ ] **Step 3: Verify no lingering wrong claims**

Run each (fixed-string match, `-F`, so no regex/escaping pitfalls):
```bash
grep -Fn '| `recall()` | `recall_async()` |' .cursor/rules/retired-surfaces.mdc
grep -Fn '| `persist_fact()` | `sync_to_db()` |' .cursor/rules/retired-surfaces.mdc
grep -Fn '| Sync recall API | `recall_async()` |' docs/src/reference/agent-quick-reference.md
grep -Fn '| Persist-fact API | `sync_to_db()` |' docs/src/reference/agent-quick-reference.md
```
Expected: no output from any of the four (each checks for the exact wrong row this task removed; note the *fixed* files still legitimately contain the bare substrings `recall_async()` and `sync_to_db` elsewhere, so a plain substring grep for those alone would false-positive against the corrected content — these four patterns instead match only the specific wrong claim each old row made).

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

### Task 9: Trim duplicated god-object thresholds in `cli-toolchain.md`, `coding-agents.md`, and `toestub-contributor-guide.md`

`governance.md` §God Object Limit is the multi-tier SSOT (300 soft / 400 warn / 500 error lines, or 12 methods). Three other currently-loaded files restate the numbers instead of pointing at it: `cli-toolchain.md` has a flattened single-tier version that's less accurate (drops the 300/400 tiers); `coding-agents.md`'s structural-limits table and `toestub-contributor-guide.md`'s `arch/god_object` fix-guide entry both restate the exact multi-tier numbers, which is the drift risk `governance.md`'s own SSOT comment exists to prevent. (`toestub-contributor-guide.md`'s separate `security/hardcoded-secret` entry is *not* touched here — see "Scope refinements" at the top of this plan for why.)

**Files:**
- Modify: `docs/agents/cli-toolchain.md:86-90`
- Modify: `docs/src/contributors/coding-agents.md:25-29`
- Modify: `docs/src/contributors/toestub-contributor-guide.md:40-43`

- [ ] **Step 1: Replace the restated thresholds in `cli-toolchain.md`**

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

- [ ] **Step 2: Replace the restated thresholds in `coding-agents.md`**

Old:
```markdown
| Limit | Value | Rule ID |
| --- | --- | --- |
| Max file length (non-blank lines) | 500 | `arch/god_object` |
| Max methods per struct/impl | 12 | `arch/god_object` |
| Max files per directory | 20 | `arch/sprawl` |
```
New:
```markdown
| Limit | Value | Rule ID |
| --- | --- | --- |
| God-object / sprawl thresholds | see [`governance.md` §God Object Limit](../../agents/governance.md#god-object-limit-multi-tier) (multi-tier, do not restate here) | `arch/god_object`, `arch/sprawl` |
```

- [ ] **Step 3: Replace the restated thresholds in `toestub-contributor-guide.md`**

Old:
```markdown
**Triggers:** A `.rs` file exceeds 500 non-blank lines, or a struct/impl has
more than 12 methods. Thresholds: 300 lines = Info, 400 = Warning, 500 = Error.
```
New:
```markdown
**Triggers:** A `.rs` file exceeds 500 non-blank lines, or a struct/impl has
more than 12 methods. Multi-tier thresholds (300/400/500 lines): see
[`governance.md` §God Object Limit](../../agents/governance.md#god-object-limit-multi-tier) — do not restate the numbers here, they drift.
```

- [ ] **Step 4: Verify the anchor resolves**

Run: `grep -n "## God Object Limit" docs/agents/governance.md`
Expected: one match (`## God Object Limit (Multi-Tier)`), confirming the link target exists.

- [ ] **Step 5: Commit**

```bash
git add docs/agents/cli-toolchain.md docs/src/contributors/coding-agents.md docs/src/contributors/toestub-contributor-guide.md
git commit -m "docs: point cli-toolchain.md, coding-agents.md, toestub-contributor-guide.md god-object thresholds at governance.md SSOT"
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

- [ ] **Step 2: Verify the cut content is still discoverable where the spec claims**

Run: `grep -n "build-dependencies\|emit()\|VOX_BUILD_NUMBER\|VOX_GIT_HASH" crates/vox-build-meta/src/lib.rs`
Expected: at least one match for each of the four terms, confirming the cut "Build metadata injection" paragraph's content (add to `[build-dependencies]`, call `vox_build_meta::emit()`, `VOX_BUILD_NUMBER`/`VOX_GIT_HASH` via `env!()`) is genuinely documented in the crate itself, not just discarded.

- [ ] **Step 3: Commit**

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
**Implementation status.** `actor`/`workflow`/`activity` and `@durable`/`@scheduled` are stable, backed by a durable runtime for the supported subset (ADR-041 supersedes the old ADR-028 reservation gate — out-of-subset behavior is now policed by the determinism lint, not a reservation gate). Contract: [ADR-019](docs/src/adr/019-durable-workflow-journal-contract-v1.md), [ADR-021](docs/src/adr/021-generated-workflow-durability-parity.md), [ADR-041](docs/src/adr/041-durable-functions-completion-2026.md). Drift between this section and `pipeline.rs` is checked by the [`docs-reality-audit-program`](docs/src/contributors/docs-reality-audit-program.md).
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "fix(docs): resolve stale 'reserved, not implemented' vs ADR-041 contradiction in AGENTS.md"
```

---

### Task 12: Trim `AGENTS.md` §PR & Review Discipline and §Local CI Gate Tiers

Keep all 4 actionable §PR & Review Discipline rules and the one-line takeaway verbatim; cut the rate-limit-number rationale paragraph and the "Repo policy (enforced by...)" explanatory paragraph down to one clause each.

Also trim §Local CI Gate Tiers's 7-row tier table and its "Slow-test partition" paragraph to a pointer. `docs/src/contributors/local-ci-pre-push.md` already documents 6 of the 7 tiers (Fast/Complete/Full/Full+cov/Full+since/Full+cov+since) in its own "Profiles" table with per-flag detail plus wall-clock targets, and its "Extended `--full` flags" table already names the identical three `--include-slow` tests AGENTS.md separately lists — this is a genuine, zero-new-authoring pointer-reduction (unlike the earlier assumption in this plan's first verification pass, which incorrectly claimed `local-ci-pre-push.md` "doesn't currently have that detail"; it does, confirmed by direct read).

**Files:**
- Modify: `AGENTS.md` §PR & Review Discipline (~26 lines → ~14 lines)
- Modify: `AGENTS.md` §Local CI Gate Tiers (~26 lines → ~14 lines)

- [ ] **Step 1: Replace the §PR & Review Discipline section**

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

- [ ] **Step 2: Replace the §Local CI Gate Tiers section**

Old (full existing section):
```markdown
## Local CI Gate Tiers (SSOT)

> **Canonical budgets:** `contracts/budgets/test-tier-budgets.v1.yaml`
> **Full tier spec:** `docs/superpowers/specs/2026-05-27-test-suite-perf-and-gate-tiers-design.md §4`
> **Per-flag details:** `docs/src/contributors/local-ci-pre-push.md`

**Run CI locally first — do NOT use GitHub Actions as your primary feedback loop (Required).**
GitHub-hosted CI is slow (minutes-to-tens-of-minutes per push) and burns runner
minutes on iteration noise. Before every push, reproduce the relevant gates locally
and only push once they are green. **Local-first runner policy:** CI jobs default to
the self-hosted Docker fleet; GitHub-hosted `runs-on` requires a registered exception
([`docs/src/ci/github-hosted-exceptions.md`](docs/src/ci/github-hosted-exceptions.md)).
Enforced gate: `vox ci runner-policy-check` runs `--strict` inside `ssot-drift`. Both CI and
the fast pre-push tier run `ssot-drift`, so an unregistered GitHub-hosted `runs-on` hard-fails
both — but **CI is authoritative** (pre-push can be `--no-verify`-skipped). The required gate
(`ci-summary`) runs hosted so a fleet outage cannot block merges (see runner-contract.md
break-glass). Local is for speed, not cost — vox is public, hosted minutes are free.
See [`docs/src/ci/runner-contract.md`](docs/src/ci/runner-contract.md) §Local-first CI.
We have Docker available, so the full GitHub workflow suite can be run locally with `act`:

- **Reproduce the actual GitHub jobs in Docker:** `vox ci pre-push --act` runs the
  workflow jobs via [nektos/act](https://github.com/nektos/act) in containers that
  mirror the CI runner image (secrets come from the git-ignored `.secrets` file).
  Use this to catch container/runner-only failures (e.g. `lychee` check-links,
  arch-check evidence-ledger) before they ever reach GitHub.
- **Faster inner loop (no Docker):** `vox ci pre-push --full` for the native gate
  tiers below; scope to changed crates with `--since <ref>`.
- **Per-job spot-checks:** run the exact command a failing job runs (e.g.
  `cargo run -q -p vox-arch-check`, `cargo run -q -p vox-cli -- ci check-links`)
  rather than re-pushing to see if it passes.

Push only after the local equivalent of the gates you expect to run is green. Treat
a red GitHub check whose local equivalent passes as a runner/environment difference
to reproduce locally (via `--act`), not as something to fix by repeated pushes.

Use `vox ci pre-push` to run any tier locally. Install the hook once with `cargo run -q -p vox-cli -- ci install-hooks`.

| Tier | Command | What runs | Target wall-clock |
|---|---|---|---:|
| **fast** (default / hook) | `vox ci pre-push` | fmt, line-endings, ssot-drift, runner-policy-check, workflow-concurrency-guard, scoped doc lint + doctest, drift-check | ≤60s |
| **complete** | `vox ci pre-push --complete` | fast + full doc lint, doc-inventory, clippy, scoped TOESTUB | ≤180s |
| **full** | `vox ci pre-push --full` | complete + nextest workspace (slow excluded) | ≤120s |
| **full+cov** | `vox ci pre-push --full --with-coverage` | full but llvm-cov nextest; emits lcov + HTML | ≤260s |
| **full+since** | `vox ci pre-push --full --since <ref>` | full, nextest for impacted crates only | ≤20s (1–3 crate edit) |
| **full+cov+since** | `vox ci pre-push --full --with-coverage --since <ref>` | combination of full+cov + since | ≤30s typical |
| **ci-equivalent** | `vox ci pre-push --full --with-coverage --include-slow` | full+cov + slow `#[ignore]` partition | ≤480s |

**Slow-test partition** (`--include-slow`): runs three `#[ignore = "slow; ..."]` tests that are excluded by default. CI always sets this flag. The 3 tests are: `arch_check_live_workspace_smoke_and_description_rule`, `timeout_kills_long_running_child`, `generated_ai_fixture_bundle_passes_cargo_check`.

**Budget enforcement:** `--enforce-budgets` compares total elapsed against `contracts/budgets/test-tier-budgets.v1.yaml` (warn at 1.2×, fail at 1.5× measured baseline). No-op if the budgets file is absent. CI also runs `vox ci tier-budget-check --junit target/nextest/ci/junit.xml --profile full` after each nextest run.
```
New:
```markdown
## Local CI Gate Tiers (SSOT)

> **Canonical budgets:** `contracts/budgets/test-tier-budgets.v1.yaml`
> **Full tier spec:** `docs/superpowers/specs/2026-05-27-test-suite-perf-and-gate-tiers-design.md §4`
> **Full tier table + per-flag details:** `docs/src/contributors/local-ci-pre-push.md` — do not restate the tier table here, it drifts.

**Run CI locally first — do NOT use GitHub Actions as your primary feedback loop (Required).**
GitHub-hosted CI is slow (minutes-to-tens-of-minutes per push) and burns runner
minutes on iteration noise. Before every push, reproduce the relevant gates locally
and only push once they are green. **Local-first runner policy:** CI jobs default to
the self-hosted Docker fleet; GitHub-hosted `runs-on` requires a registered exception
([`docs/src/ci/github-hosted-exceptions.md`](docs/src/ci/github-hosted-exceptions.md)).
Enforced gate: `vox ci runner-policy-check` runs `--strict` inside `ssot-drift`. Both CI and
the fast pre-push tier run `ssot-drift`, so an unregistered GitHub-hosted `runs-on` hard-fails
both — but **CI is authoritative** (pre-push can be `--no-verify`-skipped). The required gate
(`ci-summary`) runs hosted so a fleet outage cannot block merges (see runner-contract.md
break-glass). Local is for speed, not cost — vox is public, hosted minutes are free.
See [`docs/src/ci/runner-contract.md`](docs/src/ci/runner-contract.md) §Local-first CI.
We have Docker available, so the full GitHub workflow suite can be run locally with `act`:

- **Reproduce the actual GitHub jobs in Docker:** `vox ci pre-push --act` runs the
  workflow jobs via [nektos/act](https://github.com/nektos/act) in containers that
  mirror the CI runner image (secrets come from the git-ignored `.secrets` file).
  Use this to catch container/runner-only failures (e.g. `lychee` check-links,
  arch-check evidence-ledger) before they ever reach GitHub.
- **Faster inner loop (no Docker):** `vox ci pre-push --full` for the native gate
  tiers below; scope to changed crates with `--since <ref>`.
- **Per-job spot-checks:** run the exact command a failing job runs (e.g.
  `cargo run -q -p vox-arch-check`, `cargo run -q -p vox-cli -- ci check-links`)
  rather than re-pushing to see if it passes.

Push only after the local equivalent of the gates you expect to run is green. Treat
a red GitHub check whose local equivalent passes as a runner/environment difference
to reproduce locally (via `--act`), not as something to fix by repeated pushes.

Use `vox ci pre-push` to run any tier locally (default = **fast**, ≤60s: fmt, line-endings,
ssot-drift, runner-policy-check, workflow-concurrency-guard, scoped doc lint + doctest,
drift-check). Install the hook once with `cargo run -q -p vox-cli -- ci install-hooks`. The
full tier list (complete / full / full+cov / full+since / full+cov+since / ci-equivalent),
their exact flags, and the `--include-slow` slow-test names live in
[`local-ci-pre-push.md`](docs/src/contributors/local-ci-pre-push.md) — not restated here.

**Budget enforcement:** `--enforce-budgets` compares total elapsed against `contracts/budgets/test-tier-budgets.v1.yaml` (warn at 1.2×, fail at 1.5× measured baseline). No-op if the budgets file is absent. CI also runs `vox ci tier-budget-check --junit target/nextest/ci/junit.xml --profile full` after each nextest run.
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: trim AGENTS.md PR & Review Discipline and Local CI Gate Tiers, point tiers at local-ci-pre-push.md"
```

---

### Task 13: Add a CI drift-guard for the 4 fixed stale references

`crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` already has a live, wired-in mechanism for exactly this: `check_stale_doc_and_workflow_refs` (called from `check_docs_ssot`, part of the `ssot-drift` fast-tier gate) scans `docs/**` + root `AGENTS.md`/`README.md`/`CONTRIBUTING.md` for banned substrings. Extend its existing `DOC_BANNED` array with the 4 strings fixed in Tasks 1-4, so if any of them regresses, `vox ci pre-push` (fast tier, ≤60s) catches it immediately instead of waiting for another audit.

Verification against the live repo found the guard's current scan scope has two gaps that this task also closes, or the new banned strings would either false-positive against this plan's own document or fail to protect 4 of the 9 files Tasks 1-4 fixed:

1. **Historical docs false-positive.** `collect_text_files_under` walks all of `docs/` with no exclusion for `docs/superpowers/` or `docs/src/archive/` — both contain point-in-time plan/spec/research documents that legitimately quote retired paths verbatim (before/after diff text, historical audit findings), including this plan document itself. Without an exclusion, extending `DOC_BANNED` with these 4 strings makes `vox ci ssot-drift` fail today, before this task even lands, against files nobody asked to change. Task 14's own final sweep already treats `docs/superpowers/plans/`, `docs/src/archive/`, and `CHANGELOG.md` as expected exceptions for exactly this reason — this task applies the same exemption inside the CI check itself.
2. **Coverage gap.** The scan only ever looks at `.md`/`.yml`/`.yaml` files under `docs/` plus 3 named root files — it never covers `.cursor/rules/*.mdc` or `.rs` source files, so 3 of the 6 files Task 1 fixed (`.cursor/rules/secrets-policy.mdc` and the 2 Rust files) and `GEMINI.md`/`CLAUDE.md`/`.github/copilot-instructions.md` (named in the design spec's own Context section and already swept manually in Task 14) get no automated protection at all. This task adds those specific files as explicit extra scan targets, the same way `README.md`/`AGENTS.md`/`CONTRIBUTING.md` already are.

This remains a **static substring ban**, not a computed check (e.g. counting `.cursor/rules/*.mdc` and comparing against a hardcoded count elsewhere) — it catches regression back to the exact strings Tasks 1-4 fixed, but would not catch, say, the `.mdc` count going stale again to a *different* wrong number after a 10th rule file is added. That's an accepted, narrower scope than the fullest version of the "can't silently recur" goal, not a defect in this task.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:301-386`
- Test: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` (new `#[cfg(test)] mod tests` at end of file)

- [ ] **Step 1: Write the failing tests**

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
    fn flags_stale_tool_registry_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "See `crates/vox-orchestrator/src/mcp_tools/tools/mod.rs` for TOOL_REGISTRY.\n",
        );
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale mcp_tools/tools/mod.rs path must be flagged");
        assert!(err.to_string().contains("mcp_tools/tools/mod.rs"));
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

    #[test]
    fn ignores_historical_docs_dirs() {
        for rel in ["docs/superpowers/plans/example-plan.md", "docs/src/archive/example.md"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            let target = tmp.path().join(rel);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(&target, "Define secrets in `crates/vox-secrets/src/spec.rs`.\n")
                .expect("write file");
            check_stale_doc_and_workflow_refs(tmp.path()).unwrap_or_else(|e| {
                panic!("historical docs under {rel} must be exempt from the stale-ref scan: {e}")
            });
        }
    }

    #[test]
    fn flags_stale_path_in_newly_covered_non_doc_files() {
        for rel in [
            ".cursor/rules/secrets-policy.mdc",
            "crates/vox-code-audit/src/detectors/env_secret_shape.rs",
            "crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs",
        ] {
            let tmp = tempfile::tempdir().expect("tempdir");
            fs::create_dir_all(tmp.path().join("docs")).expect("create docs dir");
            let target = tmp.path().join(rel);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(&target, "See crates/vox-secrets/src/spec.rs for the registry.\n")
                .expect("write file");
            let err = check_stale_doc_and_workflow_refs(tmp.path()).expect_err(
                "stale reference in newly-covered non-doc file must be flagged",
            );
            assert!(
                err.to_string().contains("vox-secrets/src/spec.rs"),
                "unexpected error for {rel}: {err}"
            );
        }
    }

    #[test]
    fn flags_stale_path_in_each_newly_covered_root_file() {
        for filename in ["CLAUDE.md", "GEMINI.md", ".github/copilot-instructions.md"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            fs::create_dir_all(tmp.path().join("docs")).expect("create docs dir");
            let target = tmp.path().join(filename);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(&target, "Define secrets in `crates/vox-secrets/src/spec.rs`.\n")
                .expect("write file");
            let err = check_stale_doc_and_workflow_refs(tmp.path())
                .expect_err("stale reference in newly-covered root file must be flagged");
            assert!(
                err.to_string().contains("vox-secrets/src/spec.rs"),
                "unexpected error for {filename}: {err}"
            );
        }
    }

    #[test]
    fn skips_root_files_when_docs_dir_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("AGENTS.md"),
            "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
        )
        .expect("write AGENTS.md");
        // No docs/ dir created: the whole root-file scan (including AGENTS.md) is
        // gated behind `docs_dir.is_dir()` and is silently skipped when docs/ is
        // absent -- this documents that existing behavior rather than testing a
        // new fix (already true before and after this task's changes).
        check_stale_doc_and_workflow_refs(tmp.path()).expect(
            "without docs/, the stale-ref guard does not scan root AGENTS.md at all (documented gap)",
        );
    }
}
```

- [ ] **Step 2: Run the new tests to confirm they fail**

Run: `cargo test -p vox-cli stale_ref_guard_tests -- --nocapture`
Expected **FAIL** (6 tests — the new strings/files aren't covered yet): `flags_stale_secrets_spec_rs_path`, `flags_stale_tool_registry_path`, `flags_stale_toestub_scoped_sh`, `flags_stale_mdc_count_phrase`, `flags_stale_path_in_newly_covered_non_doc_files`, `flags_stale_path_in_each_newly_covered_root_file`.
Expected **PASS** (3 tests — passing before Step 3 doesn't mean they're vacuous, see the third one): `clean_agents_md_passes`, `skips_root_files_when_docs_dir_absent`, and `ignores_historical_docs_dirs` (passes pre-Step-3 only because `crates/vox-secrets/src/spec.rs` isn't in `DOC_BANNED` yet, so the scan never reaches the historical-dir exemption logic at all; it passes post-Step-3 for the real reason — `EXEMPT_DOC_DIRS` skips the file before the now-banned string would otherwise be flagged. Step 4's re-run after Step 3 is the actual regression check for the exemption logic, not this first run).

- [ ] **Step 3: Extend `DOC_BANNED` and close the scan-scope gaps**

Old (`docs.rs:302-386`, full function):
```rust
fn check_stale_doc_and_workflow_refs(root: &Path) -> Result<()> {
    const WORKFLOW_BANNED: &[&str] = &["verify_doc_inventory_fresh.py", "populi_release_gate.sh"];
    const DOC_BANNED: &[&str] = &["verify_doc_inventory_fresh.py", "populi_release_gate.sh"];
    // Retired crate paths / broken SSOT links — see `docs/src/archive/research-2026-q1/nomenclature-migration-map.md`.
    // Note: "crates/vox-ml-cli/" is intentionally NOT banned here — vox-ml-cli is a
    // grandfathered real crate and implementation plan docs legitimately reference its file paths.
    const NOMENCLATURE_DOC_BANNED: &[&str] = &[
        "reference/mens.md",
        "reference/mens-ssot.md",
        "crates/vox-codex-api/",
    ];
    const DOC_PATH_BANNED: &[&str] = &["docs/how-to-ai-agents.md", "docs/src/how-to-ai-agents.md"];

    let wf_dir = root.join(".github/workflows");
    if wf_dir.is_dir() {
        for entry in fs::read_dir(&wf_dir).with_context(|| format!("read {}", wf_dir.display()))? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) != Some("yml")
                && p.extension().and_then(|x| x.to_str()) != Some("yaml")
            {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in WORKFLOW_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale or retired reference {:?} (use `vox ci` guards; see docs/src/ci/doc-inventory-ssot.md)",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        let mut files = Vec::new();
        collect_text_files_under(&docs_dir, &mut files)?;
        for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"] {
            let p = root.join(rel);
            if p.is_file() {
                files.push(p);
            }
        }
        for p in files {
            let ext = p.extension().and_then(|x| x.to_str());
            if ext != Some("md") && ext != Some("yml") && ext != Some("yaml") {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale reference {:?} — removed from tree; update docs",
                        p.display(),
                        b
                    ));
                }
            }
            for b in NOMENCLATURE_DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: nomenclature drift {:?} — use canonical crate paths (see docs/src/archive/research-2026-q1/nomenclature-migration-map.md)",
                        p.display(),
                        b
                    ));
                }
            }
            for b in DOC_PATH_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale docs path {:?} — link the canonical mdBook path instead",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    println!("stale doc/workflow ref scan OK");
    Ok(())
}
```
New:
```rust
fn check_stale_doc_and_workflow_refs(root: &Path) -> Result<()> {
    const WORKFLOW_BANNED: &[&str] = &["verify_doc_inventory_fresh.py", "populi_release_gate.sh"];
    const DOC_BANNED: &[&str] = &[
        "verify_doc_inventory_fresh.py",
        "populi_release_gate.sh",
        "crates/vox-secrets/src/spec.rs",
        "crates/vox-orchestrator/src/mcp_tools/tools/mod.rs",
        "scripts/quality/toestub_scoped.sh",
        "four `.mdc` rule files",
    ];
    // Retired crate paths / broken SSOT links — see `docs/src/archive/research-2026-q1/nomenclature-migration-map.md`.
    // Note: "crates/vox-ml-cli/" is intentionally NOT banned here — vox-ml-cli is a
    // grandfathered real crate and implementation plan docs legitimately reference its file paths.
    const NOMENCLATURE_DOC_BANNED: &[&str] = &[
        "reference/mens.md",
        "reference/mens-ssot.md",
        "crates/vox-codex-api/",
    ];
    const DOC_PATH_BANNED: &[&str] = &["docs/how-to-ai-agents.md", "docs/src/how-to-ai-agents.md"];
    // Root-level and non-doc-tree files fixed in Tasks 1-4 that `collect_text_files_under`
    // never reaches (it only walks `docs/`) -- pushed explicitly, same pattern as
    // README.md/AGENTS.md/CONTRIBUTING.md below.
    const EXTRA_BANNED_SCAN_TARGETS: &[&str] = &[
        "GEMINI.md",
        "CLAUDE.md",
        ".github/copilot-instructions.md",
        ".cursor/rules/secrets-policy.mdc",
        "crates/vox-code-audit/src/detectors/env_secret_shape.rs",
        "crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs",
    ];
    // Historical/point-in-time docs -- implementation plans and specs under
    // `docs/superpowers/` and archived research under `docs/src/archive/` legitimately
    // quote retired paths verbatim (before/after diff text, historical audit findings).
    // They are not living guidance, so they're exempt from this scan (Task 14's final
    // sweep applies the same exemption for the same reason).
    const EXEMPT_DOC_DIRS: &[&str] = &["docs/superpowers", "docs/src/archive"];

    let wf_dir = root.join(".github/workflows");
    if wf_dir.is_dir() {
        for entry in fs::read_dir(&wf_dir).with_context(|| format!("read {}", wf_dir.display()))? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) != Some("yml")
                && p.extension().and_then(|x| x.to_str()) != Some("yaml")
            {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in WORKFLOW_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale or retired reference {:?} (use `vox ci` guards; see docs/src/ci/doc-inventory-ssot.md)",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        let mut files = Vec::new();
        collect_text_files_under(&docs_dir, &mut files)?;
        for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"] {
            let p = root.join(rel);
            if p.is_file() {
                files.push(p);
            }
        }
        for rel in EXTRA_BANNED_SCAN_TARGETS {
            let p = root.join(rel);
            if p.is_file() {
                files.push(p);
            }
        }
        for p in files {
            let rel = p.strip_prefix(root).unwrap_or(&p);
            if EXEMPT_DOC_DIRS.iter().any(|d| rel.starts_with(Path::new(d))) {
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str());
            if ext != Some("md")
                && ext != Some("yml")
                && ext != Some("yaml")
                && ext != Some("mdc")
                && ext != Some("rs")
            {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale reference {:?} — removed from tree; update docs",
                        p.display(),
                        b
                    ));
                }
            }
            for b in NOMENCLATURE_DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: nomenclature drift {:?} — use canonical crate paths (see docs/src/archive/research-2026-q1/nomenclature-migration-map.md)",
                        p.display(),
                        b
                    ));
                }
            }
            for b in DOC_PATH_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale docs path {:?} — link the canonical mdBook path instead",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    println!("stale doc/workflow ref scan OK");
    Ok(())
}
```

- [ ] **Step 4: Run the tests again to confirm they pass**

Run: `cargo test -p vox-cli stale_ref_guard_tests -- --nocapture`
Expected: all 9 tests **PASS**.

- [ ] **Step 5: Run the real check against the live repo**

Run: `cargo run -p vox-cli -- ci ssot-drift`
Expected: passes (exits 0) — Tasks 1-4 already removed every occurrence of these 4 strings from every file this check now scans, and Step 3's `EXEMPT_DOC_DIRS` exclusion means the plan/spec documents under `docs/superpowers/` (which legitimately quote the old strings as diff text) are not scanned. If this fails on a file *outside* `docs/superpowers/` or `docs/src/archive/`, one of Tasks 1-4's `grep` verification steps missed a hit; go back and fix it before proceeding. If it fails on a file *inside* those directories, `EXEMPT_DOC_DIRS` in Step 3 was applied incorrectly — re-check the `rel.starts_with(...)` logic before proceeding.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs
git commit -m "test(vox-cli): ban the 4 fixed stale doc references from regressing, close scan-scope gaps"
```

---

### Task 14: Final verification sweep

- [ ] **Step 1: Re-confirm zero stale-reference hits across the whole repo (not just docs/)**

Run each and confirm no unexpected output (historical files under `docs/superpowers/plans/`, `docs/src/archive/`, and `CHANGELOG.md` are expected/allowed to still mention old paths — they're point-in-time historical records, not living guidance):

```bash
grep -rn "crates/vox-secrets/src/spec.rs" AGENTS.md CLAUDE.md GEMINI.md .cursor .github docs/agents docs/src/reference docs/src/contributors crates/vox-code-audit crates/vox-cli/src/commands/ci --exclude=docs.rs
```
(`--exclude=docs.rs` skips `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` itself — after Task 13, that file legitimately contains this exact string as a `DOC_BANNED` array literal and in its own test fixtures; grepping it here would be a self-match, not a regression.)
```bash
grep -rn "mcp_tools/tools/mod.rs" docs/agents docs/src
```
```bash
grep -rln "scripts/quality/toestub_scoped.sh" docs/
```
```bash
grep -n "four .mdc" AGENTS.md
```
```bash
grep -Fn '| `recall()` | `recall_async()` |' .cursor/rules/retired-surfaces.mdc
grep -Fn '| `persist_fact()` | `sync_to_db()` |' .cursor/rules/retired-surfaces.mdc
grep -Fn '| Sync recall API | `recall_async()` |' docs/src/reference/agent-quick-reference.md
grep -Fn '| Persist-fact API | `sync_to_db()` |' docs/src/reference/agent-quick-reference.md
```
(re-confirms Task 7's contradiction fix — `recall()`/`recall_async()` and `persist_fact()`/`sync_to_db()` — is still in place after Tasks 8-12 touched nearby content; not otherwise re-checked by anything above)

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
