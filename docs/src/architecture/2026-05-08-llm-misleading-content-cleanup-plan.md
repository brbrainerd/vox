---
title: "LLM-Misleading-Content Cleanup Plan (2026-05-08)"
description: "Comprehensive plan to eliminate stale syntax, retired-feature mentions, and dual-brain documentation that mislead future LLM tool calls. Converges every surface on single-source-of-truth current syntax via positive-example framing."
category: "Architecture SSOTs"
status: roadmap
last_updated: "2026-05-08"
training_eligible: false
audience: contributors
---

# LLM-Misleading-Content Cleanup Plan

> **For agentic workers:** This plan is structured for one-task-at-a-time execution by Sonnet 4.6 or equivalent. Each phase is self-contained — read only the files the phase names, follow the verification commands, commit, then proceed. Do **not** load the whole plan into working context; read only the active phase.
>
> **Companion plan:** [`2026-05-02-codebase-cleanup-and-signal-improvement.md`](2026-05-02-codebase-cleanup-and-signal-improvement.md) — that plan is older and treats `@island` as "active code on a known decommission timeline." `@island` retirement landed 2026-05-03, so its Phase 5 is no longer a deferral. **This plan supersedes that plan's Phase 5 section** and adds findings not covered there.

---

## Goal

Eliminate content across code, docs, ADRs, fixtures, snapshots, and instruction surfaces that would mislead a future LLM tool call into:
- using a retired decorator (`@island`, `@server`, `@query`, `@mutation`) as if current,
- writing imports against a renamed crate path (`crates/vox-clavis/...` instead of `crates/vox-secrets/...`),
- thinking a completed phase is in-flight or a future phase is shipped,
- treating a single-mode (fullstack-only) build as the only mode,
- following a superseded plan because its supersession was never declared,
- learning from a negative example ("don't use X") when a positive example of the current way is available.

## Principles

1. **Two positive examples beat one positive + one negative.** When a doc-comment says "do not use X", rewrite it to show the current way as a positive example. Keep retirement notices as historical reference, not as primary content.
2. **Single source of truth.** When two documents cover the same area, one is canonical and the other has a SUPERSEDED banner pointing to it. No "dual brains."
3. **Archive, don't delete.** History is fine in `docs/src/archive/`. Stale narrative in *active* directories is the problem.
4. **Don't hand-edit auto-generated files.** `SUMMARY.md`, `architecture-index.md`, `feed.xml`, `*.generated.md`, `.cursorignore`, `.aiignore`, `.aiexclude` are tool output. Always run the generator.
5. **Verify before edit.** Every phase begins by reading the files it changes. The inventory below is a guide, not a guarantee — code moves.
6. **Decisions belong to the operator.** Where a phase has a judgment call, pause and ask before acting. (The `clavis` brand rename decision was made 2026-05-08: full rename.)

## Inventory Snapshot (2026-05-08)

This is the *summary* of findings. The phase-by-phase steps below have the operational detail.

| Surface | Finding | Phase |
|---|---|---|
| `AGENTS.md` §Secret Management, `.github/copilot-instructions.md`, `.cursor/rules/secrets-policy.mdc`, `.cursor/rules/data-storage-policy.mdc`, `crates/_frozen.md`, `vox-schema.json`, `contracts/toestub/suppressions.v1.json`, `.voxindexingignore`, `docs/agents/*-allowlist.txt` | Referenced `crates/vox-clavis/...` paths; the directory is now `crates/vox-secrets/`. Operator chose full rename (2026-05-08); all `clavis` brand surfaces are being updated. | **A** |
| `AGENTS.md:178`, `GEMINI.md:29`, `GEMINI.md:47` | Cross-refs to `docs/src/architecture/{terminal-exec-policy-research-findings-2026.md, vox-as-glue-research-2026.md}`; both files now in `docs/src/archive/research-2026-q1/`. | **B** |
| `docs/src/reference/ref-decorators.md:17–30` (and adjacent) | `@server`, `@query`, `@mutation` documented as current — they are retired in favor of `@endpoint(kind: ...)`. | **C** |
| `docs/src/reference/ref-decorators.md` (the `@island` section, ~92–94) | Already correctly marked retired ✅ — verify only. | **C** |
| `docs/src/adr/027-dual-track-ui-surfaces.md:5` | Frontmatter `status: "current"` while body declares supersession. | **D** |
| `docs/src/adr/012-internal-web-ir-strategy.md` | Body marks superseded; `training_eligible` flag may need to be `false`. Verify. | **D** |
| `crates/vox-compiler/tests/snapshots/reactive_smoke_test__island_mount_ast_z.snap`, `…__parity_page_tsx_data_prop_label_op_s166.snap` | Snapshots assert `data-vox-island=...` as canonical emit. Verify whether the producing tests are intentional regression-pins (keep with rename) or stale. | **E** |
| `crates/vox-compiler/tests/web_ir_lower_emit_test.rs:954` | "post-@island retirement" comment — keep, but verify the test still has a clear positive purpose. | **E** |
| `crates/vox-orchestrator/src/queue/priority.rs:233`, `…/snapshot.rs:21`, `…/workspace.rs:44`, `crates/vox-orchestrator-queue/src/lib.rs:4`, `crates/vox-orchestrator-types/src/agent_types/file_affinity.rs:1`, `crates/vox-integration-tests/tests/pipeline/includes/include_01.rs:502`, `crates/vox-cli/src/commands/ci/nomenclature_guard.rs:16,40,49–50` | Comments label sections "Phase 5", "Phase 5F", "Phase 5.1" as if active phase numbers. The work is done; phase numbers should be replaced with feature names. | **F** |
| `crates/vox-cli/src/cli_args.rs:9`, `crates/vox-config/src/config/gamify_web.rs:55–69`, `crates/vox-codegen/src/codegen_ts/emitter.rs:149` | "(existing default)" / "Express server only when explicitly requested" framing — implies a single-mode legacy build model. Rewrite to neutral two-mode language. | **G** |
| `crates/vox-actor-runtime/src/builtins/mod.rs` (vox_hash_fast docstring), `crates/vox-config/src/env_parse.rs` (multiple "do NOT use this for secrets" comments) | Negative-only docstrings. Rewrite as positive examples that name the right tool first. | **H** |
| `docs/src/archive/phase5-react-interop-spec-2026.md`, `docs/src/archive/research-2026-q1/react-interop-implementation-plan-2026.md` | Superseded plans missing prominent banner pointing to current `external-frontend-interop-plan-2026.md`. | **I** |
| `docs/src/archive/how-to-islands-and-pages.md:20–27` | Internal prose says `@island` is "scheduled for retirement" — the file's own frontmatter says it was retired 2026-05-03. Reconcile. | **I** |
| `docs/src/architecture/gui-native-roadmap-status-2026.md` (TASK-7.3, TASK-8.2 verdicts) | Future-tense language for tasks that are complete-or-deferred (mixed). | **J** |
| `docs/src/architecture/dashboard-migration-research-2026.md` (~line 50) | "future compiler roadmap will expose..." — flip to "deferred to Phase 9 (vox-gui-native-roadmap-2026.md)". | **J** |
| `docs/superpowers/plans/ci/2026-05-03-vox-dashboard-claude-design-port.md:5–6, 656` | Treats `import react` Phase 5 bridges as if working today. Add Phase-5-blocking note. | **K** |
| `docs/src/SUMMARY.md`, `docs/agents/doc-inventory.json`, `docs/agents/doc-inventory-index.json`, `architecture-index.md`, `feed.xml`, `.cursorignore`, `.aiignore`, `.aiexclude` | Auto-generated; potentially stale post-retirement. Re-run generators (do not hand-edit). | **L** |
| (repo-wide) Three independent phase numbering systems | Frontend interop (Phases 1–5), GUI-native (Phases 0–9), workspace reorg (Phases 1–10). LLMs conflate them. | **M** |
| `docs/src/architecture/2026-05-02-codebase-cleanup-and-signal-improvement.md:33` | Calls `@island` "active code on a known decommission timeline" — but it was decommissioned 2026-05-03. Tasks reference `v0_shadcn_island.vox` golden, also stale. | **N** |
| `docs/src/api/DOC_GAPS.md:6,16–17` | `last_updated: 2026-04-06` while file documents 2026-05-03 retirement; gap entries for `workflow`/`activity` may conflict with their retirement in interop plan Phase 4. | **N** |
| (verification) | Final repo-wide grep for missed items. | **O** |

---

## Pre-Flight (run once before starting)

Before any phase, confirm the working tree is clean and verify the inventory has not drifted.

```pwsh
# 1. Working tree clean
git status

# 2. Branch from main (or current working branch — operator's call)
git rev-parse --abbrev-ref HEAD

# 3. Quick rot probes — counts only, to confirm inventory is still current
#    (use the Grep tool for these, not raw rg, to inherit project ignores)
#    Expected counts as of 2026-05-08 in parens; treat large drifts as a flag to re-inventory.
#    - "vox-clavis" or "vox_clavis"   ~30+ hits across instruction surfaces and config (now being renamed to vox-secrets / vox_secrets)
#    - "@island"                      ~43 files (most are archived/correct retirement notices)
#    - "data-vox-island"              2 snapshot files
#    - "(existing default)"           in vox-config gamify_web.rs
#    - "Phase 5F" or "Phase 5.1"      a handful of code comments

# 4. Confirm the canonical interop plan exists
git log --oneline -1 -- docs/src/architecture/external-frontend-interop-plan-2026.md
```

If any count drifts wildly, **stop and re-run the inventory** before continuing — the plan's targets may be stale.

---

## Phase A — Crate-path drift: `crates/vox-clavis/...` → `crates/vox-secrets/...`

**Goal.** Update every doc and rule that points at the old crate directory path. Do *not* rename env vars or the CLI verb yet — that's a separate decision.

### A.0 Decision: Full rename chosen (2026-05-08)

The operator decision has been made: **full rename** including env vars and CLI verbs. The `clavis` brand is being retired across all active surfaces. Archive documents retain the old name for historical context.

Previously open questions:

- Env vars: `VOX_CLAVIS_*` → `VOX_SECRETS_*`. Defined in `crates/vox-config/src/operator_registry.rs`. (Being updated by parallel agents.)
- CLI: `vox clavis doctor`, `vox clavis set`, `vox clavis login` → `vox secrets doctor`, `vox secrets set`, `vox secrets login`. (Being updated by parallel agents.)
- Module file: `crates/vox-config/src/clavis.rs` → `crates/vox-config/src/secrets.rs`. (Being updated by parallel agents.)
- CI workflow: `.github/workflows/ci.yml` `VOX_CLAVIS_*` exports. (Being updated by parallel agents.)

### A.1 Full rename targets

For each file below, read it first, then update `crates/vox-clavis/...` → `crates/vox-secrets/...`, `vox_clavis::` → `vox_secrets::`, `vox clavis` CLI verbs → `vox secrets`, and brand references `Clavis`/`clavis` → `Secrets`/`secrets`.

Files (verified as of 2026-05-08):
1. [`AGENTS.md`](../../../AGENTS.md) — lines 75, 76, 77, 88, 89, 90 (the §Secret Management section). **Caveat:** if A.0 chose "full rename", also update `vox_clavis::resolve_secret(...)` → `vox_secrets::resolve_secret(...)` here. Otherwise leave the `vox_clavis::` Rust import path intact and let it be resolved by the Rust crate-rename in a separate dedicated PR.
2. [`.github/copilot-instructions.md`](../../../.github/copilot-instructions.md) — line 27.
3. [`.cursor/rules/secrets-policy.mdc`](../../../.cursor/rules/secrets-policy.mdc) — lines 8, 9.
4. [`.cursor/rules/data-storage-policy.mdc`](../../../.cursor/rules/data-storage-policy.mdc) — lines 11, 13.
5. [`.voxindexingignore`](../../../.voxindexingignore) — line 59.
6. [`vox-schema.json`](../../../vox-schema.json) — `"vox-clavis"` key on line ~50 and `path_pattern` on line ~52. **Caveat:** this is a JSON schema that may be consumed by tooling — search for consumers of `"vox-clavis"` as a key before renaming the key. If consumers exist, leave the key and only update `path_pattern`.
7. [`contracts/toestub/suppressions.v1.json`](../../../contracts/toestub/suppressions.v1.json) — `path_glob` on line 7.
8. [`crates/_frozen.md`](../../../crates/_frozen.md) — line 13.
9. [`docs/agents/turso-import-allowlist.txt`](../../agents/turso-import-allowlist.txt) — line 8.
10. [`docs/agents/sql-connection-api-allowlist.txt`](../../agents/sql-connection-api-allowlist.txt) — line 8.
11. [`docs/agents/query-all-allowlist.txt`](../../agents/query-all-allowlist.txt) — line 7.

### A.2 Verification

```pwsh
# Should return zero hits in the files above (use Grep tool, not raw rg)
# After: Grep for "crates/vox-clavis" in repo, output_mode=files_with_matches
# Expect: only matches in CHANGELOG.md (intentional history) and any *.archive/* docs
```

If hits remain in non-archive files, recheck.

### A.3 Commit

```
docs(secrets): full clavis → secrets brand rename in docs and instructions

Operator chose full rename (2026-05-08). Crate paths, Rust import
paths, CLI verbs, env-var refs, and brand mentions updated from
clavis/Clavis to secrets/Secrets across instruction surfaces,
architecture docs, contributor docs, and rule files.
```

---

## Phase B — Broken doc cross-references

**Goal.** Three known broken links pointing into `docs/src/architecture/` for files that now live in `docs/src/archive/research-2026-q1/`.

### B.1 Reads first

- [`AGENTS.md`](../../../AGENTS.md) — read context around line 178.
- [`GEMINI.md`](../../../GEMINI.md) — read context around lines 29, 47.

### B.2 Edits

For each, change the path and append `(archived)` if the surrounding link list uses that convention.

| File | Line | Old path | New path |
|---|---|---|---|
| AGENTS.md | ~178 | `docs/src/architecture/terminal-exec-policy-research-findings-2026.md` | `docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md` |
| GEMINI.md | ~29 | `docs/src/architecture/vox-as-glue-research-2026.md` | `docs/src/archive/research-2026-q1/vox-as-glue-research-2026.md` |
| GEMINI.md | ~47 | `docs/src/architecture/terminal-exec-policy-research-findings-2026.md` | `docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md` |

### B.3 Sweep for siblings

There may be other docs linking to the same archived files. Run:

```
# Use Grep tool with pattern: docs/src/architecture/(terminal-exec-policy-research-findings-2026|vox-as-glue-research-2026)
# output_mode=content, -n=true
```

Update any extra hits the same way.

### B.4 Verification

After edits, the same Grep should show no hits in active (non-archive, non-CHANGELOG) docs.

### B.5 Commit

```
docs: retarget archived research links to docs/src/archive/research-2026-q1/

Two research docs were archived in Q1 2026 reorg; AGENTS.md and
GEMINI.md still pointed at the old architecture/ path. Updates
inline link targets and standardizes on (archived) labeling.
```

---

## Phase C — Reference docs: retired decorators

**Goal.** `docs/src/reference/ref-decorators.md` documents `@server`, `@query`, `@mutation` as if current. They were replaced by `@endpoint(kind: server|query|mutation)`. Add a positive-example deprecation block — show the current way first, mention the retired form as a one-line "replaced by".

### C.1 Reads first

- [`docs/src/reference/ref-decorators.md`](../reference/ref-decorators.md) — read in full (~250 lines).
- [`docs/src/architecture/external-frontend-interop-plan-2026.md`](external-frontend-interop-plan-2026.md) — check the §Phase 3 / @endpoint section so the rewritten reference matches the canonical syntax exactly.

### C.2 Verify the inventory finding

Before editing, **confirm** lines 17–30 actually describe `@server`, `@query`, `@mutation` as current. The inventory was drawn from agent excerpts; trust but verify.

### C.3 Edit pattern

For each retired decorator section in `ref-decorators.md`:

1. Replace the section header with a "Retired" subsection placed **after** the corresponding `@endpoint(kind: ...)` section, not before.
2. Format:
   ```markdown
   ### `@endpoint(kind: query)` (current syntax)

   <full positive-example block: 1–2 short examples showing the current way>

   #### Replaced: `@query` (retired YYYY-MM-DD)

   `@query` is no longer recognized. The two snippets below are equivalent — use the lower form.

   <one example pair: old → new>
   ```
3. Do not delete the retired form's documentation entirely — leave a one-line "replaced by" so historical search engines and old tutorials still resolve.
4. Keep the `@island` section already correctly marked retired (verify it follows the same template; if it deviates, normalize).

### C.4 Verification

```
# Use Grep tool: pattern "^### `@(server|query|mutation)`"  type=md  path=docs/src/reference
# Expect: zero hits at level "### `@server`" without the "(retired" suffix.
```

Run the docs build (`mdbook build docs` or `cargo run -p vox-doc-pipeline`, whichever is canonical for this repo — check `AGENTS.md`) to ensure no link checker breaks.

### C.5 Commit

```
docs(reference): mark @server / @query / @mutation as retired

ref-decorators.md still listed the old endpoint decorators as
current. Each section is now a "Replaced" subsection under the
corresponding @endpoint(kind: ...) form, framed as positive
examples of the current syntax with the retired form as a one-line
pointer for legacy search.
```

---

## Phase D — ADR frontmatter alignment

**Goal.** ADRs whose frontmatter `status:` contradicts their own body need to be reconciled. Two known cases.

### D.1 Reads first

- [`docs/src/adr/027-dual-track-ui-surfaces.md`](../adr/027-dual-track-ui-surfaces.md) — read in full. Body on lines 24, 27, 39, 50 narrates `@island` as a current Track B surface; frontmatter at line 5 says `status: "current"`.
- [`docs/src/adr/012-internal-web-ir-strategy.md`](../adr/012-internal-web-ir-strategy.md) — read in full. Body marks "Superseded (2026-05-03)"; frontmatter `training_eligible` may be `true` (problem) or already `false` (no action).
- [`docs/src/adr/002-diataxis-doc-architecture.md`](../adr/002-diataxis-doc-architecture.md):60–62 — confirm the canonical status enum: `current|experimental|legacy|research|roadmap|deprecated`.

### D.2 Edits

**ADR 027:**
- Frontmatter `status: "current"` → `status: "deprecated"` (per the body's "Superseded (2026-05-03)" line).
- Add a top-of-body banner immediately under the H1, before any other content:
  ```markdown
  > **Superseded 2026-05-03** by [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). The Track B `@island` surface described below is retired. This ADR is retained for historical context.
  ```
- Set frontmatter `training_eligible: false` if not already.

**ADR 012:**
- Verify body's supersession date and successor.
- Set frontmatter `status: "deprecated"` and `training_eligible: false`.
- Add the equivalent banner at top of body.

### D.3 Verification

```
# Use Grep on docs/src/adr for: ^status: "current"
# Read each match's body for "Superseded" — none should remain.
```

### D.4 Commit

```
docs(adr): reconcile ADR-012 / ADR-027 frontmatter with superseded body

Both ADRs body-text declared supersession on 2026-05-03 but kept
status: current and training_eligible: true in frontmatter. Updates
status to deprecated, training_eligible to false, and adds a
top-of-body banner pointing to the canonical successor.
```

---

## Phase E — Snapshot tests for `@island` emit

**Goal.** Decide whether the two `data-vox-island` snapshots are intentional regression-pins (keep, and clarify) or genuine stale.

### E.1 Reads first

- [`crates/vox-compiler/tests/web_ir_lower_emit_test.rs`](../../../crates/vox-compiler/tests/web_ir_lower_emit_test.rs) — read in full. Look for the test functions that produce the two snapshots:
  - `reactive_smoke_test__island_mount_ast_z`
  - `reactive_smoke_test__parity_page_tsx_data_prop_label_op_s166`
- [`crates/vox-compiler/tests/snapshots/reactive_smoke_test__counter_tsx_reactive_smoke.snap`](../../../crates/vox-compiler/tests/snapshots/reactive_smoke_test__counter_tsx_reactive_smoke.snap)
- [`crates/vox-compiler/tests/snapshots/reactive_smoke_test__clicky_tsx_classname_onclick.snap`](../../../crates/vox-compiler/tests/snapshots/reactive_smoke_test__clicky_tsx_classname_onclick.snap)
- The doc-comment at `web_ir_lower_emit_test.rs:954` says "Parity chain fixture (post-@island retirement)" — that suggests these are intentional regression pins.

### E.2 Decision tree

- **If the producing test is alive and explicitly asserts post-retirement behavior** (i.e., the test exists to lock in that the retired decorator no longer emits anything specific or is rejected at parse time): keep the test, but rename its function and the snapshot to make the regression intent explicit. Pattern: `…__rejects_island_mount_emit` instead of `…__island_mount_ast_z`. Update both the test function name and run `cargo insta accept` (or repo's snapshot tool) to regenerate the renamed snapshot.
- **If the producing test is dead** (function gone, snapshot orphaned): delete both `.snap` files and run the snapshot tool to confirm no test references them.
- **If the test is alive but its assertion still emits `data-vox-island`**: that's a genuine bug — the retirement landed but the emitter still produces the attribute. **Stop and report this to the operator** — it's outside this plan's scope.

### E.3 Verification

```
cargo test -p vox-compiler --test web_ir_lower_emit_test
# After any rename, cargo insta review (or equivalent)
```

### E.4 Commit

```
test(vox-compiler): clarify @island regression-pin snapshots

The two reactive_smoke_test snapshots that contain data-vox-island
output are post-retirement regression pins. Renames the test
functions and snapshot files to make the intent explicit
(rejects_island_mount instead of island_mount_ast_z) so future
readers do not infer the attributes are a current emit target.
```

(Or if deleted: `test(vox-compiler): remove orphaned @island snapshots`.)

---

## Phase F — Phase-N marker hygiene in code

**Goal.** Strip "Phase 5", "Phase 5F", "Phase 5.1", "Phase 3 grandfather" markers from code comments where they refer to *completed* phases. Phase numbers are calendar artifacts; once the work lands, the comment should describe the *feature*, not the phase.

**Keep** phase markers when they describe *deferred* work or genuinely-still-in-flight phases (rare in this codebase).

### F.1 Reads first (one file at a time)

For each file, read the surrounding ~30 lines. The inventory cited specific lines, but adjacent code may have related phase prose worth normalizing in the same edit.

1. [`crates/vox-orchestrator/src/queue/priority.rs`](../../../crates/vox-orchestrator/src/queue/priority.rs) — line ~233 ("Phase 5.1 additions").
2. [`crates/vox-orchestrator/src/snapshot.rs`](../../../crates/vox-orchestrator/src/snapshot.rs) — line ~21 (Phase 5 reorg note).
3. [`crates/vox-orchestrator/src/workspace.rs`](../../../crates/vox-orchestrator/src/workspace.rs) — line ~44 (Phase 5 reorg note).
4. [`crates/vox-orchestrator-queue/src/lib.rs`](../../../crates/vox-orchestrator-queue/src/lib.rs) — line ~4 (Phase 5 extraction).
5. [`crates/vox-orchestrator-types/src/agent_types/file_affinity.rs`](../../../crates/vox-orchestrator-types/src/agent_types/file_affinity.rs) — line ~1 (Phase 5 extraction).
6. [`crates/vox-integration-tests/tests/pipeline/includes/include_01.rs`](../../../crates/vox-integration-tests/tests/pipeline/includes/include_01.rs) — line ~502 ("Phase 5F").
7. [`crates/vox-cli/src/commands/ci/nomenclature_guard.rs`](../../../crates/vox-cli/src/commands/ci/nomenclature_guard.rs) — lines ~16, 40, 49–50 (Phase 2/3/5 markers in allowlist comments).

### F.2 Edit pattern

For each: replace `Phase X` with the feature/extraction it represents. Keep the date if it's already present (`2026-05-08`) — dates are durable, phase numbers are not.

Examples:

```rust
// Before:
// Identifiers moved to `vox-orchestrator-types` in 2026-05-08 reorg Phase 5.

// After:
// Identifiers extracted to `vox-orchestrator-types` (2026-05-08).
```

```rust
// Before:
// ── Phase 5.1 additions ──────────────────────────────────────────────

// After:
// ── Deduplication queue (enqueue_dedup) ─────────────────────────────
```

```rust
// Before:
// --- Phase 5F: Full-stack dashboard integration test ---

// After:
// --- Dashboard full-pipeline integration test (OP-S206) ---
```

For `nomenclature_guard.rs`, "grandfathered (Phase 3)" type comments: rewrite to `(grandfathered, migration in progress)` if migration is ongoing, or strip the phase number entirely if it's done. Read the surrounding allowlist to judge.

### F.3 Verification

```
# Grep tool: pattern "Phase [0-9]" --type=rust path=crates
# Expect: only hits in legitimate phase-number contexts (e.g., enum names, Phase{N}State types
# referring to actual runtime state machines, not calendar phases).
```

### F.4 Per-file commit (or one bundled commit if small)

```
chore(comments): replace calendar phase markers with feature names

Phase 5 / 5.1 / 5F markers in completed code referred to the
2026-05-08 reorg checkpoint. Replaces with feature-named comments
so future LLM tool calls don't infer ongoing-phase work where
none exists.
```

---

## Phase G — Build-mode terminology

**Goal.** Several comments imply the build model is single-mode with `fullstack` as the legacy default. The current model is two-mode (fullstack + server-only) per [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md). Rewrite to neutral two-mode framing.

### G.1 Reads first

- [`crates/vox-cli/src/cli_args.rs`](../../../crates/vox-cli/src/cli_args.rs) — line ~9.
- [`crates/vox-config/src/config/gamify_web.rs`](../../../crates/vox-config/src/config/gamify_web.rs) — lines 55–69 (the `BuildTarget` enum doc).
- [`crates/vox-codegen/src/codegen_ts/emitter.rs`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs) — line ~149 (Express server gate).

### G.2 Edits

**`cli_args.rs:9`**
- Old: `/// fullstack` is the default — existing projects are unaffected.`
- New: `/// fullstack is the default. Use --target=server for Rust-only (Axum), or --target=client for the TS SDK (`vox-client.ts`, `openapi.json`, …).`

**`gamify_web.rs:62`** (and adjacent)
- Old: ` ... and Axum Rust backend (existing default).`
- New: ` ... and Axum Rust backend (default mode).`
- Sweep the rest of the enum doc for "existing" / "legacy" / "old" framing and neutralize.

**`emitter.rs:149`**
- Old: `// Generate Express server only when explicitly requested (Axum + api.ts is canonical).`
- New: `// Legacy Express server emission (deprecated; Axum + typed vox-client.ts is canonical). Gated on VOX_EMIT_EXPRESS_SERVER=1.`
- Add a `#[deprecated]` annotation to the Express-emit function if it has a clear function boundary; otherwise just the comment.

### G.3 Verification

```
# Grep: pattern "(existing default|existing projects are unaffected)"
# Expect: zero hits.
```

```
cargo check -p vox-cli -p vox-config -p vox-codegen
```

### G.4 Commit

```
docs(comments): drop "existing default" framing for build modes

Two-mode build model (fullstack + server-only) has been the design
since the 2026-05-03 retirement; comments still implied a legacy
single-mode framing. Rewrites to neutral two-mode language and
flags the Express emit path as deprecated explicitly.
```

---

## Phase H — Negative→positive doc-comment rewrites

**Goal.** Two known instances; sweep for more during execution. Convert "do NOT use this for X" to a positive example that names the right tool first.

### H.1 Reads first

- [`crates/vox-actor-runtime/src/builtins/mod.rs`](../../../crates/vox-actor-runtime/src/builtins/mod.rs) — find the `vox_hash_fast` doc-comment.
- [`crates/vox-config/src/env_parse.rs`](../../../crates/vox-config/src/env_parse.rs) — read in full (it has multiple "Do NOT use this for secrets" comments).

### H.2 Edit pattern

Two-positive-example rule: name the **right** tool first, then mention the wrong tool only as a one-liner.

**Before:**
```rust
//! **Secrets:** do not use this for API keys — resolve via vox_secrets::resolve_secret at the callsite.
```

**After:**
```rust
//! For non-secret config (timeouts, operator flags, feature gates), parse with
//! the helpers below. For API keys and other sensitive values, use
//! `vox_secrets::resolve_secret(...)` at the callsite — this module deliberately
//! does not handle secrets.
```

**Before:**
```rust
/// ⚠ NOT cryptographic — do not use for stored provenance hashes.
```

**After:**
```rust
/// Fast non-cryptographic hash (FxHash). Use for object identity, dedup keys,
/// and other ephemeral keying. For provenance, signatures, or any
/// security-sensitive hash, use `vox_hash_secure` (BLAKE3-based).
```

### H.3 Sweep

```
# Grep on crates/**: "do not use this", "Do NOT use", "NOT cryptographic", "do not call this"
# Read each hit and judge whether a positive-first rewrite applies.
```

Some negative-only doc-comments are appropriate (e.g., `unsafe` invariants where the *only* signal is "don't"). Use judgment; not every negative needs a flip.

### H.4 Verification

```
cargo doc --workspace --no-deps
# Open the rewritten items in the generated rustdoc and confirm they read positively.
```

### H.5 Commit

```
docs(rust): flip negative-only doc-comments to positive examples

Doc-comments saying "do not use this for X" are weaker LLM signals
than "use Y for the right case; this module does Z". Rewrites
two known instances (env_parse, vox_hash_fast) and sweeps for
related negative-only patterns.
```

---

## Phase I — Superseded plans: banners

**Goal.** Three archived plan documents and one in-archive how-to lack a prominent SUPERSEDED banner pointing to the canonical successor.

### I.1 Reads first (each in full)

- [`docs/src/archive/phase5-react-interop-spec-2026.md`](../archive/phase5-react-interop-spec-2026.md) — has weak banner on ~line 11. Strengthen.
- [`docs/src/archive/research-2026-q1/react-interop-implementation-plan-2026.md`](../archive/research-2026-q1/react-interop-implementation-plan-2026.md) — no banner.
- [`docs/src/archive/how-to-islands-and-pages.md`](../archive/how-to-islands-and-pages.md) — has frontmatter `archived_date: 2026-05-03` but body line ~20 says "scheduled for retirement" (future tense).

### I.2 Standard banner template

Place immediately after the H1 heading, before any other prose:

```markdown
> **⚠ Superseded — archived YYYY-MM-DD.**
> The current plan for this area is [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).
> Retained for historical context only. Do not implement against this document.
```

For the how-to (different shape — it's user-facing teaching material, not a plan):

```markdown
> **⚠ Archived YYYY-MM-DD.**
> The `@island` decorator was retired YYYY-MM-DD. For the current way to compose UI surfaces and bridge React, see [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).
> The contents below describe a retired pattern and are kept for migration reference.
```

### I.3 In-body fixes

For the how-to specifically, also reconcile internal prose:
- Line ~20 ("scheduled for retirement") → "retired 2026-05-03".
- Any future-tense language → past-tense.

### I.4 Verification

```
# Grep on docs/src/archive: pattern "scheduled for retirement"
# Expect: zero hits (or only inside literal code-block quotes).
```

```
mdbook build docs   # link checker
```

### I.5 Commit

```
docs(archive): add SUPERSEDED banners on retired interop plans

Three archive docs — phase5-react-interop-spec-2026, the Q1
react-interop-implementation-plan, and how-to-islands-and-pages
— were missing or had only weak supersession notices. Adds a
standard banner pointing to external-frontend-interop-plan-2026
and reconciles "scheduled for retirement" prose with the
2026-05-03 retirement event.
```

---

## Phase J — Stale prose in completed-phase docs

**Goal.** Two architecture docs use future tense for tasks that are complete-or-deferred (mixed). Reword.

### J.1 Reads first

- [`docs/src/architecture/gui-native-roadmap-status-2026.md`](gui-native-roadmap-status-2026.md) — read in full. Focus on TASK-7.3, TASK-8.2, and any cells with future-tense verbs (`will be`, `is going to`, `is scheduled to`).
- [`docs/src/architecture/dashboard-migration-research-2026.md`](dashboard-migration-research-2026.md) — find the line (~50) using "future compiler roadmap will expose primitives".

### J.2 Edit pattern

Each task verdict should match its check state:
- `✅ Done` → past tense ("Implemented in `crates/...`").
- `⏳ Pending — external dependency` → present tense with explicit blocker ("Awaiting operator MENS training run").
- `🔲 Deferred to Phase X` → present tense pointer ("Deferred to Phase 9; see vox-gui-native-roadmap-2026.md §...").

For `dashboard-migration-research-2026.md`:
- Old: `"deferred to Phase 9, the future compiler roadmap will expose primitives..."`
- New: `"Deferred to Phase 9 of the GUI-native roadmap (vox-gui-native-roadmap-2026.md §Phase 9). Until that lands, dashboards rely on hand-authored TSX components."`

### J.3 Verification

```
# Grep on docs/src/architecture/gui-native-roadmap-status-2026.md:
#   pattern "(will be|is going to|is scheduled to)"
# Expect: ≤ 1 deliberate hit (e.g., a true future commitment); each remaining hit justified inline.
```

### J.4 Commit

```
docs(architecture): align tense with task status in roadmap docs

gui-native-roadmap-status-2026 and dashboard-migration-research
mixed future-tense prose with ✅ Done check states. Updates each
task verdict to match its actual status (done / pending external
dep / deferred to specific later phase) so LLMs can read the
status field as authoritative.
```

---

## Phase K — Dashboard plan: Phase-5 hedge

**Goal.** The dashboard port plan describes `import react` Phase 5 bridges as if working today. Add a hedge.

### K.1 Reads first

- [`docs/superpowers/plans/ci/2026-05-03-vox-dashboard-claude-design-port.md`](../../superpowers/plans/ci/2026-05-03-vox-dashboard-claude-design-port.md) — read in full (it's long; chunk by H2).

### K.2 Edits

1. Add a status banner under the H1:
   ```markdown
   > **Phase 5 dependency.** Sections describing capitalized React-component imports (e.g. `CodeEditor(path=active_path)` calling into a React-authored TSX file) depend on Phase 5 of [external-frontend-interop-plan-2026](../../src/architecture/external-frontend-interop-plan-2026.md), which is in-plan as of 2026-05-08. Until Phase 5 lands, those surfaces require a hand-authored compat layer (TBD).
   ```

2. Around line ~656 (the `CodeEditor` example), add an inline note matching the banner:
   ```markdown
   <!-- Phase 5: requires the React-component import bridge from external-frontend-interop-plan-2026. Until that lands, mount manually via TBD compat layer. -->
   ```

### K.3 Verification

Read the doc top-to-bottom and confirm any other "import react" or "capitalized component call pattern" mention has either a Phase-5 hedge or a Phase-5 footnote pointing to the banner.

### K.4 Commit

```
docs(plans): hedge Phase-5 React-bridge usage in dashboard port plan

The 2026-05-03 dashboard port plan describes capitalized React
component imports (CodeEditor, etc.) as the current pattern. That
bridge is Phase 5 of external-frontend-interop-plan-2026 and not
shipped. Adds a top-of-doc Phase-5 dependency banner and inline
notes at each affected example.
```

---

## Phase L — Regenerate auto-generated docs

**Goal.** Re-run generators so that `SUMMARY.md`, doc-inventory, ignore-files, and others reflect post-retirement state. **Do not hand-edit any of these.**

### L.1 Reads first

- [`AGENTS.md`](../../../AGENTS.md) — find the §Doc Generation / §Ignore-File Generation sections to confirm canonical commands.

Likely commands (verify in AGENTS.md before running):
- `cargo run -p vox-doc-pipeline` (regenerates `SUMMARY.md`, `architecture-index.md`, `feed.xml`, `*.generated.md`).
- `vox ci sync-ignore-files` or `cargo run -p vox-cli -- ci sync-ignore-files` (regenerates `.cursorignore`, `.aiignore`, `.aiexclude` from `.voxignore`).
- `cargo run -p vox-cli -- ci doc-inventory` or similar (regenerates `docs/agents/doc-inventory*.json`).

### L.2 Run

```pwsh
# Run each generator. Inspect git diff after each, do not commit corrupted output.
cargo run -p vox-doc-pipeline
git diff -- docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml '*.generated.md'

cargo run -p vox-cli -- ci sync-ignore-files
git diff -- .cursorignore .aiignore .aiexclude

# (substitute correct command for doc-inventory)
git diff -- docs/agents/doc-inventory.json docs/agents/doc-inventory-index.json
```

### L.3 If a generator fails or leaves fragments

The 2026-05-02 cleanup plan flagged corrupted comment fragments (lines ~6–11) in the ignore files. If those reappear:
1. **Stop** — do not commit.
2. File a separate task: "Generator leaves stub `#` fragments in `.cursorignore`."
3. Hand-edit only as a temporary patch *if explicitly authorized by the operator*.

### L.4 Verification

```
git diff --stat
# Expect changes only in known generated outputs.
```

```
mdbook build docs    # confirm SUMMARY.md is internally valid
```

### L.5 Commit

```
chore(generated): regenerate docs index, ignore files, and doc inventory

Regenerates SUMMARY.md / architecture-index.md / feed.xml,
.cursorignore / .aiignore / .aiexclude, and docs/agents/doc-
inventory*.json after the @island retirement and surrounding
plan-banner updates. No hand edits.
```

---

## Phase M — Phase-numbering clarification doc (NEW)

**Goal.** Three independent phase numbering systems coexist; LLMs and humans both conflate them. One short index doc disambiguates.

### M.1 New file

[`docs/src/architecture/phase-numbering-index.md`](phase-numbering-index.md)

```markdown
---
title: "Phase Numbering Index (2026-05-08)"
description: "Disambiguates the three independent phase sequences used in vox plans. When a plan or commit says \"Phase 5\", look here first."
category: "Architecture SSOTs"
status: current
last_updated: "2026-05-08"
training_eligible: true
audience: contributors
---

# Phase Numbering Index

The vox project uses **three independent phase sequences**. They are not aligned. When a code comment, plan doc, or commit message says "Phase N", check which sequence it belongs to.

| Sequence | Range | Topic | Canonical plan | Status |
|---|---|---|---|---|
| **Frontend interop** | Phases 1–5 | Build target split, TS-emit, HTTP ergonomics, schema, React bidirectional interop | [external-frontend-interop-plan-2026](external-frontend-interop-plan-2026.md) | Phases 1–4 done; Phase 5 in plan |
| **GUI-native language** | Phases 0–9 | Vox compiler primitives for native UI | [vox-gui-native-roadmap-2026](vox-gui-native-roadmap-2026.md) | Phases 0–7 done; 8–9 in plan / deferred |
| **Workspace reorg** | Phases 1–10 | Crate extraction, layer enforcement, dead-crate burn | [2026-05-08-workspace-reorg-design](2026-05-08-workspace-reorg-design.md), [2026-05-08-workspace-reorg-outcome](2026-05-08-workspace-reorg-outcome.md) | Mostly complete; Phases 3, 6, 7 deferred |

## How to disambiguate at a glance

- "Phase 5" alone → **frontend interop** (most common usage).
- "GUI Phase N" or context mentions VUV/native → **GUI-native**.
- "reorg Phase N" or context mentions crate extraction → **workspace reorg**.

## When writing new code or comments

Prefer feature names over phase numbers in code comments. Phase numbers are calendar-relative; feature names age better. If you must use a phase number, qualify it: `// Frontend interop Phase 5: ...`.

## Cross-references

This index is referenced from:
- `AGENTS.md` §Phase Numbering
- `CLAUDE.md` (top-level pointer)
- Each of the three canonical plans (mutual link in their headers)
```

### M.2 Add cross-references

After creating the index, add a one-line reference at the top of each canonical plan, immediately after frontmatter:

```markdown
> **Phase numbering:** This plan uses the **{frontend interop | GUI-native | workspace reorg}** phase sequence. See [phase-numbering-index](phase-numbering-index.md) for the other two sequences.
```

Add to:
- [`docs/src/architecture/external-frontend-interop-plan-2026.md`](external-frontend-interop-plan-2026.md)
- [`docs/src/architecture/vox-gui-native-roadmap-2026.md`](vox-gui-native-roadmap-2026.md)
- [`docs/src/architecture/2026-05-08-workspace-reorg-design.md`](2026-05-08-workspace-reorg-design.md)

Add a `Phase Numbering` link to:
- [`AGENTS.md`](../../../AGENTS.md) — under the architecture-index reference, one line.

### M.3 Verification

```
mdbook build docs
```

### M.4 Commit

```
docs(architecture): add phase-numbering-index to disambiguate three sequences

Frontend interop (1–5), GUI-native (0–9), and workspace reorg
(1–10) phases coexist and routinely get conflated in commits and
agent prompts. Adds a single short index doc and links it from
each canonical plan plus AGENTS.md.
```

---

## Phase N — Reconcile the 2026-05-02 cleanup plan

**Goal.** That earlier plan deferred all `@island`-related cleanup to "Phase 5 of the interop plan". Phase 5 of the interop plan retired `@island` on 2026-05-03. So the 2026-05-02 plan's Phase 5 work is now actionable (or already done in this very plan).

### N.1 Reads first

- [`docs/src/architecture/2026-05-02-codebase-cleanup-and-signal-improvement.md`](2026-05-02-codebase-cleanup-and-signal-improvement.md) — read in full.

### N.2 Edits

1. Update the §Source Findings entry at line ~33:
   - Old: `"@island infrastructure ... is *active code on a known decommission timeline*"`
   - New: `"@island was retired 2026-05-03 (see external-frontend-interop-plan-2026 §Phase 5). Cleanup of remaining @island artifacts in CLI commands, island_emit.rs, templates, golden files, and the how-to is now actionable. See 2026-05-08-llm-misleading-content-cleanup-plan.md for the consolidated cleanup."`

2. Update the four golden examples bullet (line ~31): `v0_shadcn_island.vox` is itself a stale fixture — verify whether it's still used by any test. If unused, plan its deletion in this phase. If used, schedule a separate task to update it to current syntax.

3. Add a top-of-body banner:
   ```markdown
   > **Partially superseded 2026-05-08.** Phase 5 (@island cleanup) is now consolidated under [2026-05-08-llm-misleading-content-cleanup-plan](2026-05-08-llm-misleading-content-cleanup-plan.md). Remaining phases (1–4, 6) are still actionable as written.
   ```

4. Update the file's `last_updated:` frontmatter to `2026-05-08`.

### N.3 Reads + edits — DOC_GAPS.md

- [`docs/src/api/DOC_GAPS.md`](../api/DOC_GAPS.md) — update `last_updated:` to `2026-05-08` if the file's content has been touched by anything in this plan.
- Also reconcile the `workflow` and `activity` gap entries at lines ~16–17:
  - Read the relevant section of [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md) for the actual status of those keywords.
  - If retired: mark the gaps "deferred — keyword retired post-Phase-4".
  - If kept: leave the gap entries but cross-reference the plan section that describes them.

### N.4 Verification

```
mdbook build docs
```

### N.5 Commit

```
docs(plans): mark 2026-05-02 cleanup plan Phase 5 as superseded

Phase 5 of the 2026-05-02 plan deferred @island cleanup behind
external-frontend-interop-plan-2026 Phase 5. That phase landed
2026-05-03; the cleanup is now consolidated in the 2026-05-08
LLM-misleading-content cleanup plan. Adds a partial-supersession
banner. Also reconciles DOC_GAPS.md last_updated and workflow/
activity entries with the plan's Phase 4 retirement decisions.
```

---

## Phase O — Final verification sweep

**Goal.** After all preceding phases, run repo-wide probes to catch anything missed.

### O.1 Probes

Run each Grep and read every hit. **Most hits will be in `docs/src/archive/**` or `CHANGELOG.md` — those are intentional history. Flag only hits in active surfaces.**

```
# 1. Retired decorator forms outside reference docs and archive
Grep pattern: "@island\b|@server\b|@query\b|@mutation\b"
type: md
glob: docs/!(archive)/**/*.md

# 2. Stale crate paths
Grep pattern: "crates/vox-clavis"
type: md
(also check json, txt, mdc)
Excluding: CHANGELOG.md, docs/src/archive/

# 3. Phase markers in code
Grep pattern: "Phase [0-9]"
type: rust
glob: crates/**

# 4. "(existing default)" / future-as-current framing
Grep pattern: "existing default|will be exposed|will be retired|scheduled for retirement"

# 5. ADR frontmatter integrity
Grep pattern: '^status: "current"'
path: docs/src/adr
# For each hit, confirm body has no "Superseded" line.

# 6. Non-.vox automation scripts at repo root
Glob: *.ps1, *.sh, *.py
# scripts/vox-dev.sh is the one allowed bootstrap launcher.
```

### O.2 Build + test gates

```
cargo check --workspace
cargo test --workspace --no-run
mdbook build docs
cargo run -p vox-arch-check
```

If any check fails, **stop and diagnose** — do not proceed to merge.

### O.3 Memory-update prompt

Independently of code/doc changes, update auto-memory at `C:\Users\Owner\.claude\projects\C--Users-Owner-vox\memory\`:

- Update `project_vox_interop_positioning.md` (or whatever the current memory file is) with the 2026-05-03 retirement landing date and a pointer to this cleanup plan.
- Add a new memory entry pointing at [phase-numbering-index.md](phase-numbering-index.md) so future sessions resolve "Phase N" correctly.

### O.4 Commit

(Each phase commits independently. Phase O has no edits of its own; it gates the merge.)

---

## Done Criteria

This plan is complete when **every** item below holds:

- [ ] **Phase A:** Full rename across instruction surfaces. `Grep` for `crates/vox-clavis`, `vox_clavis`, and `vox clavis` returns hits only in CHANGELOG and `docs/src/archive/`.
- [ ] **Phase B:** Three known broken cross-refs in AGENTS.md and GEMINI.md fixed; sweep for siblings clean.
- [ ] **Phase C:** `ref-decorators.md` shows `@server`/`@query`/`@mutation` only as "Replaced" subsections under `@endpoint(kind: ...)`.
- [ ] **Phase D:** ADR 027 and ADR 012 frontmatter `status` matches body. Both have a top-of-body SUPERSEDED banner.
- [ ] **Phase E:** Two `data-vox-island` snapshots either renamed to make regression-pin intent explicit, or deleted, or genuine bug filed.
- [ ] **Phase F:** No "Phase 5"/"Phase 5F"/"Phase 5.1" calendar markers in non-archive Rust source. Hits only in legitimate phase-typed APIs.
- [ ] **Phase G:** No "(existing default)" / "existing projects are unaffected" framing. Express emit gated and labeled deprecated.
- [ ] **Phase H:** `vox_hash_fast` and `env_parse` doc-comments lead with positive examples. Sweep clean.
- [ ] **Phase I:** Three archived plans + the islands how-to have standard SUPERSEDED banners. Internal "scheduled for retirement" prose reconciled.
- [ ] **Phase J:** Future-tense in `gui-native-roadmap-status-2026.md` and `dashboard-migration-research-2026.md` reconciled with task status.
- [ ] **Phase K:** Dashboard port plan has top-of-doc Phase-5 banner and per-example notes.
- [ ] **Phase L:** Auto-generated docs regenerated cleanly. No hand edits.
- [ ] **Phase M:** `phase-numbering-index.md` exists; cross-refs added to three plans + AGENTS.md.
- [ ] **Phase N:** 2026-05-02 cleanup plan has partial-supersession banner; DOC_GAPS reconciled.
- [ ] **Phase O:** All probes clean (excluding archive/CHANGELOG); workspace builds; mdbook builds; vox-arch-check passes; auto-memory updated.

## Out of Scope (deliberately deferred)

- ~~Renaming the `clavis` brand for env vars and CLI verb.~~ **Done (2026-05-08).** Operator chose full rename; parallel agents are executing it across the codebase.
- ~~Renaming `vox_clavis::` Rust import paths.~~ **In progress (2026-05-08).** Being renamed to `vox_secrets::` by parallel agents.
- Rewriting the canonical interop plan. This plan ships *around* it, not over it.
- Removing `@island` runtime code (`island_emit.rs`, `templates/islands.rs`, `vox-cli` island commands). Those are tracked under the 2026-05-02 cleanup plan; this plan only stops them being *referenced* as if current.
- Touching `crates/vox-actor-runtime/src/llm/types.rs` env-var twiddling — that's runtime behavior, not misleading-content rot.

## Appendix: How Sonnet 4.6 should execute this plan

1. **One phase per session is ideal.** Each phase is sized to fit comfortably in working context with the inventory snapshot above as the only pre-loaded reference.
2. **Read before edit.** Every phase's first step is `Reads first`. Do those before any `Edit` call.
3. **Verify before commit.** Every phase has a verification block. Run it; if it fails, fix and re-verify, do not commit and "fix later".
4. **Use the Edit tool, not Write, on existing files.** Only Phase M creates a new file (`phase-numbering-index.md`).
5. **Stop and ask if reality differs from inventory.** The inventory is a snapshot; if you read a file and the cited line says something else, stop. Re-read surroundings, judge whether the rot has already been fixed, and report.
6. **Do not chain multiple phases into one commit.** Each phase has its own commit message template; keep them separate so the merge story is reviewable.
7. **`@island` references inside `docs/src/archive/**` are intentional.** Do not chase them.
8. **Memory updates (Phase O.3) must use auto-memory tooling**, not raw filesystem writes from inside the repo worktree.
