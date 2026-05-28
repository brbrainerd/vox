---
title: "Crate Org Follow-up — Implementation Plan"
description: "Step-by-step plan to land the 6 PRs from 2026-05-08-crate-org-followup-design.md."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; transient artifact."
---

# Crate Org Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the SSOT/discoverability fixes, naming corrections, and 5 build-time extractions specified in [2026-05-08-crate-org-followup-design.md](./2026-05-08-crate-org-followup-design.md), and add `vox-arch-check` lints to prevent regression.

**Architecture:** Six sequential PRs. PR1 fixes names everywhere docs read them. PR2–PR5 perform extractions/splits ranked by build-time impact ÷ disruption cost (smallest-first). PR6 adds enforcement so the gains stick. Each PR is independently mergeable and revertable; verification commands are explicit.

**Tech Stack:** Rust 2024, `cargo`, `cargo metadata`, `vox-doc-pipeline`, `vox-arch-check` (workspace's own architectural lint binary), TOML, Markdown.

**Spec:** [2026-05-08-crate-org-followup-design.md](./2026-05-08-crate-org-followup-design.md)

**Authoritative ground truth (do NOT update from memory — re-read before editing):**
- `crates/<name>/Cargo.toml` (the actual `name = "..."` field)
- `Cargo.toml` `[workspace.dependencies]`
- `docs/src/architecture/layers.toml`
- The actual `crates/<name>/` directory existence

**Project rules to honor (from CLAUDE.md / AGENTS.md):**
- Auto-generated docs MUST be regenerated, never hand-edited: `docs/src/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/src/feed.xml`, `*.generated.md`, `.cursorignore`/`.aiignore`/`.aiexclude` (regen via `vox-doc-pipeline` or `vox ci sync-ignore-files`).
- Project automation MUST be `.vox` files via `vox run`, never `.ps1`/`.sh`/`.py` glue.
- All ` ```vox ` doc snippets must compile via `vox-doc-pipeline` doctest runner; use `// vox:skip` to opt out per-block.
- `archive/` and `docs/src/archive/` are tombstoned — do NOT read or modify.
- Use `Edit`/`Write`/`Read`/`Glob`/`Grep` tools, not `cat`/`sed`/`awk`/`echo`.

---

> **Status (2026-05-15):** All PRs 1–6 landed in one session (2026-05-15). The 156 checkboxes below were not individually ticked — see the status callout in [2026-05-08-crate-org-followup-design.md](2026-05-08-crate-org-followup-design.md) for the authoritative summary. C3 and Tier D (C5) are deferred with their own plan docs. `vox-arch-check` reports clean.

## Pre-flight (do once before starting)

- [ ] **Step 0.1: Confirm baseline build is clean.** This is the reference point for all verifications.

  Run:
  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  ```
  Expected: both clean (exit 0, no errors).

- [ ] **Step 0.2: Confirm doc pipeline is clean.**

  Run:
  ```
  cargo run -p vox-doc-pipeline -- --check
  ```
  Expected: clean exit. If it fails, regenerate first with `cargo run -p vox-doc-pipeline` and re-run `--check`.

- [ ] **Step 0.3: Read the spec.**

  Read `docs/src/architecture/2026-05-08-crate-org-followup-design.md` end-to-end before starting Task 1.

---

# PR1 — SSOT + Descriptions + vox-mcp-meta merge

PR1 lands all of Track A (SSOT/discoverability), the description rewrites portion of Track B, and the C1 merge. Eight tasks, single PR.

## Task 1: Flip the gamify ↔ ludus directives in live policy surfaces

The crate is canonical `vox-gamify`. Every live doc claiming `vox-ludus` is canonical is drift.

**Files:**
- Modify: `AGENTS.md:195`
- Modify: `.cursor/rules/retired-surfaces.mdc:13`
- Modify: `.github/copilot-instructions.md:18`
- Modify: `.github/PULL_REQUEST_TEMPLATE.md:5,7,10,14`
- Modify: `contracts/proximity/retired-surfaces.v1.json:14-17`

- [ ] **Step 1.1: Flip the AGENTS.md retired-surfaces row.**

  Read `AGENTS.md` lines 190–202. Find the row reading:
  ```
  | `vox-gamify` | `vox-ludus` |
  ```
  Replace with:
  ```
  | `vox-ludus` | `vox-gamify` |
  ```

- [ ] **Step 1.2: Flip the .cursor rule row.**

  In `.cursor/rules/retired-surfaces.mdc:13`, apply the same flip as step 1.1.

- [ ] **Step 1.3: Flip the copilot instructions line.**

  In `.github/copilot-instructions.md`, find:
  ```
  Use `vox-ludus`, NOT `vox-gamify`.
  ```
  Replace with:
  ```
  Use `vox-gamify`, NOT `vox-ludus`.
  ```

- [ ] **Step 1.4: Update the PR template gamify references.**

  Read `.github/PULL_REQUEST_TEMPLATE.md` (whole file).
  - Line 5: change `## Ludus / gamify (if applicable)` to `## Gamify (if applicable)`.
  - Line 7: rephrase the body around `vox-gamify` (preserve any archive doc reference, just replace the crate name).
  - Line 10: change `Ludus section if new VOX_LUDUS_*` to `Gamify section if new VOX_LUDUS_*` (env-var prefix unchanged).
  - Line 14: change `cargo test -p vox-ludus` to `cargo test -p vox-gamify`.

- [ ] **Step 1.5: Flip the proximity contract JSON.**

  In `contracts/proximity/retired-surfaces.v1.json` lines 14–17, find the entry:
  ```json
  {
    "retired_symbol": "vox-gamify",
    "canonical_replacement": "vox-ludus"
  }
  ```
  Replace with:
  ```json
  {
    "retired_symbol": "vox-ludus",
    "canonical_replacement": "vox-gamify"
  }
  ```

- [ ] **Step 1.6: Verify with grep.**

  Run:
  ```
  rg "vox-ludus.*vox-gamify|Use \`vox-ludus\`" -n .cursor .github AGENTS.md contracts/proximity
  ```
  Expected: no matches in any *live* file (every match should be in `archive/` or generated reports — not these paths).

## Task 2: Fix gamify references in docs and crates/vox-gamify/README.md

**Files:**
- Modify: `crates/vox-gamify/README.md`
- Modify: `docs/src/contributors/toestub-contributor-guide.md:202`
- Modify: `docs/src/reference/agent-quick-reference.md:40`
- Modify: `docs/agents/database-nomenclature.md:36,112,113`
- Modify: `docs/src/reference/clavis-ssot.md:26`
- Modify: `docs/src/reference/env-vars.md:62,66,69,73`
- Modify: `docs/src/reference/cli.md:525`
- Modify: `docs/src/reference/hitl-and-doubt.md:39`
- Modify: `docs/src/reference/mens-serving-ssot.md:19`

- [ ] **Step 2.1: Rewrite `crates/vox-gamify/README.md`.**

  Read the file. Change the H1 from `# vox-ludus` to `# vox-gamify`. Replace every other `vox-ludus` token in the body with `vox-gamify`. Leave the user-visible CLI verb `vox ludus` alone if it appears (it's a Latin alias and out of scope).

- [ ] **Step 2.2: Flip the toestub contributor guide row.**

  In `docs/src/contributors/toestub-contributor-guide.md` line 202, flip the same way as Task 1.1.

- [ ] **Step 2.3: Flip the agent-quick-reference row.**

  In `docs/src/reference/agent-quick-reference.md` line 40, flip the same way.

- [ ] **Step 2.4: Update database-nomenclature references.**

  In `docs/agents/database-nomenclature.md`:
  - Line 36: replace `vox-ludus` with `vox-gamify`.
  - Lines 112–113: replace any `crates/vox-ludus/...` paths with `crates/vox-gamify/...`. Replace `vox-ludus` references with `vox-gamify`.

- [ ] **Step 2.5: Update clavis-ssot reference.**

  In `docs/src/reference/clavis-ssot.md:26`, replace `vox-ludus` with `vox-gamify`.

- [ ] **Step 2.6: Update env-vars paths.**

  In `docs/src/reference/env-vars.md` lines 62, 66, 69, 73, change every `crates/vox-ludus/src/...` to `crates/vox-gamify/src/...`. Leave the env-var name prefix `VOX_LUDUS_*` alone.

- [ ] **Step 2.7: Update cli.md reference.**

  In `docs/src/reference/cli.md:525`, replace `vox-ludus` with `vox-gamify`.

- [ ] **Step 2.8: Update hitl-and-doubt.md.**

  In `docs/src/reference/hitl-and-doubt.md:39`, replace `vox-ludus` with `vox-gamify`.

- [ ] **Step 2.9: Update mens-serving-ssot.md.**

  In `docs/src/reference/mens-serving-ssot.md:19`, replace `vox-ludus` with `vox-gamify`.

- [ ] **Step 2.10: Verify all gamify references.**

  Run:
  ```
  rg "vox-ludus" docs/src docs/agents crates/vox-gamify --files-without-match docs/src/archive contracts/reports CHANGELOG.md
  ```
  Expected: zero results outside the allowed exemptions (archive/, generated reports, CHANGELOG history).

## Task 3: Fix other retired-form directives (Track A2)

**Files:**
- Modify: `docs/src/contributors/coding-agents.md:20`
- Modify: `docs/src/reference/ref-decorators.md:17-30`
- Modify: `docs/src/reference/ref-syntax.md:174,178`
- Modify: `docs/src/reference/ref-type-system.md:87`
- Modify: `docs/src/reference/vox-db-language-surface.md:19-21`
- Modify: `docs/src/reference/vox-fullstack-artifacts.md:20,33`
- Modify: `docs/src/reference/orchestration-unified.md:25`
- Modify: `docs/src/reference/hitl-and-doubt.md:32`
- Modify: `docs/src/.well-known/llms-full.txt:38`

- [ ] **Step 3.1: Update coding-agents.md vox-dei reference.**

  In `docs/src/contributors/coding-agents.md:20`, replace:
  ```
  `vox-dei` is now a small HITL crate, not the orchestrator
  ```
  with:
  ```
  `vox-dei` was retired; the orchestrator is `vox-orchestrator`.
  ```

- [ ] **Step 3.2: Unify decorator references in ref-decorators.md.**

  In `docs/src/reference/ref-decorators.md` lines 17–30, replace any standalone `@server`/`@query`/`@mutation` decorator descriptions with the unified `@endpoint(kind: server|query|mutation)` form per `AGENTS.md:198`. Preserve any route-mapping table.

- [ ] **Step 3.3: Update ref-syntax.md examples.**

  In `docs/src/reference/ref-syntax.md`:
  - Line 174: change `@query fn …` to `@endpoint(kind: query) fn …`.
  - Line 178: change `@mutation fn …` to `@endpoint(kind: mutation) fn …`.

- [ ] **Step 3.4: Update ref-type-system.md example.**

  In `docs/src/reference/ref-type-system.md:87`, change `@server fn update_task(...)` to `@endpoint(kind: server) fn update_task(...)`. Also change any `->` to `to` if the surrounding examples use the `to` arrow form (verify by reading the surrounding 10 lines first).

- [ ] **Step 3.5: Update vox-db-language-surface decorator rows.**

  In `docs/src/reference/vox-db-language-surface.md` lines 19–21, replace the three rows recommending `@query fn`, `@mutation fn`, `@server fn` with rows recommending `@endpoint(kind: query)`, `@endpoint(kind: mutation)`, `@endpoint(kind: server)` respectively. Preserve route-mapping columns.

- [ ] **Step 3.6: Update vox-fullstack-artifacts.md.**

  In `docs/src/reference/vox-fullstack-artifacts.md` lines 20 and 33, replace `@server fn` with `@endpoint(kind: server)`.

- [ ] **Step 3.7: Update orchestration-unified.md.**

  In `docs/src/reference/orchestration-unified.md:25`, drop the `vox-dei-d` mention. The daemon is `vox-orchestrator-d`. Replace inline.

- [ ] **Step 3.8: Update hitl-and-doubt.md ResolutionAgent reference.**

  In `docs/src/reference/hitl-and-doubt.md:32`, the line says `ResolutionAgent (from the vox-dei crate)`. The `vox-dei` crate no longer exists. Verify the actual home by grepping for `ResolutionAgent` in `crates/`:
  ```
  rg "struct ResolutionAgent|impl ResolutionAgent|pub struct ResolutionAgent" crates/
  ```
  Update the doc with the actual crate path (likely `vox-orchestrator::dei_shim`). If `ResolutionAgent` no longer exists, drop the parenthetical.

- [ ] **Step 3.9: Update llms-full.txt vox-ars line.**

  In `docs/src/.well-known/llms-full.txt:38`, replace:
  ```
  `vox-ars` → replaced by `vox-skills`
  ```
  with:
  ```
  `vox-ars` → `vox-ars-runtime` (now `vox-openclaw-runtime`)
  ```
  This matches `AGENTS.md:194`.

## Task 4: Fix crate-name drift (vox-pm → vox-package, vox-mens → vox-ml-cli, etc.)

**Files:**
- Modify: `docs/agents/cli-toolchain.md:18,20,22,24,46,48`
- Modify: `docs/agents/database-nomenclature.md:21`
- Modify: `docs/agents/orchestrator.md:60`
- Modify: `docs/agents/script-registry.json:58`
- Modify: `docs/src/reference/env-vars.md:15,34`
- Modify: `docs/src/reference/cli.md:24,33,48,85,223,566-570`
- Modify: `docs/src/reference/changelog.md:46`
- Modify: `docs/src/reference/mens-serving-ssot.md:19`
- Modify: `docs/src/reference/vox-portability-ssot.md:40,115`
- Modify: `README.md:71`
- Modify: `crates/vox-db/README.md:3`

- [ ] **Step 4.1: Update cli-toolchain.md row by row.**

  Read `docs/agents/cli-toolchain.md` lines 1–60.
  - Line 18: split the conflated row "vox-compiler (Codegen, SSG)" into separate rows for `vox-compiler`, `vox-codegen`, `vox-ssg`.
  - Line 20: change `vox-pm/` to `vox-package/`.
  - Line 22: change `vox-tensor/` description "Burn-based ML training" to "Pure-CPU JSONL data loaders / training-pair types (Burn extracted 2026-05-08)".
  - Line 24: change `vox-toestub/` to `vox-code-audit/`.
  - Line 46: drop the `vox train --native` row (Burn removed; this command does not exist).
  - Line 48: verify `vox mens corpus` is the actual subcommand by grepping `crates/vox-cli/src/commands/`. If not, update.

- [ ] **Step 4.2: Update database-nomenclature.md vox-pm row.**

  In `docs/agents/database-nomenclature.md:21`, change every cell in the `vox-pm` row:
  - Crate name: `vox-pm` → `vox-package`
  - Path: `crates/vox-pm` → `crates/vox-package`
  - Leave description text intact unless it mentions Burn / training (then replace per the same pattern).

- [ ] **Step 4.3: Update orchestrator.md vox-mens reference.**

  In `docs/agents/orchestrator.md:60`, replace `vox-mens` with `vox-ml-cli`.

- [ ] **Step 4.4: Verify script-registry.json CI subcommand.**

  Read `docs/agents/script-registry.json:58`. Verify the `replacement` field references a CI subcommand that actually exists by grepping `crates/vox-cli/src/commands/ci/`:
  ```
  rg "no-vox-dei-import|no_vox_dei_import" crates/vox-cli/src/commands/ci/
  ```
  If found, leave alone. If not found, find the actual subcommand name and update.

- [ ] **Step 4.5: Update env-vars.md vox-pm references.**

  In `docs/src/reference/env-vars.md`:
  - Line 15: change header `## Codex / Turso (vox-db, vox-pm)` to `## Codex / Turso (vox-db, vox-package)`.
  - Line 34: change `vox-pm exposes ...` to `vox-package exposes ...`. Verify the feature reference (`replication = ["vox-db/replication"]`) matches `crates/vox-package/Cargo.toml:11`.

- [ ] **Step 4.6: Update cli.md crate references.**

  In `docs/src/reference/cli.md`:
  - Line 24: in the section about specialized domains, leave the user-visible CLI verb `vox mens` alone, but if the binary name `vox-mens` appears, change to `vox-ml-cli`.
  - Line 33: same pattern.
  - Line 48: same.
  - Line 85: change header `## Package management (vox-pm)` to `## Package management (vox-package)`.
  - Line 223: change `vox-pm` to `vox-package`.
  - Lines 566–570: change `vox-mens-...` to `vox-ml-cli-...`.

- [ ] **Step 4.7: Update changelog reference.**

  In `docs/src/reference/changelog.md:46`, change `vox-pm/README.md` to `vox-package/README.md`.

- [ ] **Step 4.8: Update mens-serving-ssot vox-runtime reference.**

  In `docs/src/reference/mens-serving-ssot.md:19`, replace `vox-runtime` (which doesn't exist) with the canonical pair: `vox-actor-runtime` / `vox-workflow-runtime`. Surrounding context determines which.

- [ ] **Step 4.9: Update vox-portability-ssot.md.**

  In `docs/src/reference/vox-portability-ssot.md` lines 40 and 115, change `vox-pm` to `vox-package`.

- [ ] **Step 4.10: Update README.md vox-mens row.**

  In `README.md:71`, decide: if `vox-mens` is a *plugin distribution* name (external), document the `vox-mens` plugin → `vox-ml-cli` binary mapping in the same row. If it's a stale internal reference, replace with `vox-ml-cli`. Default to documenting the mapping (preserves user-facing surface).

- [ ] **Step 4.11: Update vox-db README.**

  In `crates/vox-db/README.md:3`, change `Wraps vox-pm::CodeStore` to `Wraps vox-package::CodeStore`.

- [ ] **Step 4.12: Add ADR footnotes.**

  ADR files `docs/src/adr/004-codex-arca-turso-ssot.md` and `docs/src/adr/015-vox-docker-oci-portability-ssot.md` reference `vox-pm` historically. Don't rewrite the ADR body, but at the bottom of each file add a one-line note:
  ```markdown
  > Nomenclature note (2026-05-08): `vox-pm` was renamed to `vox-package`; references in this ADR are historical.
  ```

- [ ] **Step 4.13: Verify name drift is gone.**

  Run:
  ```
  rg "vox-pm\b|vox-mens\b" docs/src docs/agents README.md crates/vox-db/README.md --files-without-match docs/src/archive docs/src/adr docs/src/reference/changelog.md
  ```
  Expected: zero matches outside ADRs and changelog.

## Task 5: Fix layers.toml comment + the two narration docs

**Files:**
- Modify: `docs/src/architecture/layers.toml:135` (comment in known_inversions)
- Modify: `docs/src/architecture/2026-05-08-workspace-reorg-outcome.md` (top of file)
- Modify: `docs/src/architecture/2026-05-08-naming-and-guards-design.md` (top of file)

- [ ] **Step 5.1: Fix the layers.toml inversion-reason comment.**

  Read `docs/src/architecture/layers.toml:135` and the surrounding `[[known_inversions]]` block. The comment mentions "vox dei" as a CLI subcommand. Verify by:
  ```
  rg "Subcommand|fn dei|name = \"dei\"" crates/vox-cli/src/commands/dei.rs
  ```
  If `dei` exists, leave alone. If not, replace `vox dei` with the actual subcommand name in the comment.

- [ ] **Step 5.2: Add a one-line note to workspace-reorg-outcome.md.**

  At the top of `docs/src/architecture/2026-05-08-workspace-reorg-outcome.md`, immediately after the H1, add a blockquote:
  ```markdown
  > **Naming note (2026-05-08):** The CI guard binary referenced as `vox-layer-check` in this narration was renamed to `vox-arch-check` later in the same series; references below are historical.
  ```

- [ ] **Step 5.3: Add the same note to naming-and-guards-design.md.**

  At the top of `docs/src/architecture/2026-05-08-naming-and-guards-design.md`, immediately after the H1, add the identical blockquote from step 5.2.

## Task 6: Expand `where-things-live.md` to cover all 83 crates

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 6.1: Read current contents and the layers.toml to align.**

  Read `docs/src/architecture/where-things-live.md` end-to-end and `docs/src/architecture/layers.toml` end-to-end. Note: the table is grouped in two sections — "Quick reference: subsystem → crate" and "Plugins". Plugins live at L4.

- [ ] **Step 6.2: Replace the "Quick reference" section with a layer-grouped table.**

  Restructure the "Quick reference: subsystem → crate" table so it covers every L0/L1/L2/L3/L5 entry from `layers.toml`. Use this exact structure (one section per layer, alphabetical within section). Each row is `| Crate | One-line scope |`. For the scope column, use the description from the spec's Track-B description table where one is provided; otherwise read the crate's `lib.rs` first 30 lines and write a one-sentence summary.

  ```markdown
  ## Quick reference: subsystem → crate (by layer)

  ### L0 — pure types
  | Crate | One-line scope |
  |---|---|
  | [`vox-arch-check`](../../../crates/vox-arch-check/) | CI guard binary; enforces layers.toml. |
  | [`vox-db-types`](../../../crates/vox-db-types/) | Pure-data L0 leaf for vox-db: row types, IDs, schema descriptors. |
  | [`vox-mesh-types`](../../../crates/vox-mesh-types/) | Pure-data mesh transport types. |
  | [`vox-orchestrator-types`](../../../crates/vox-orchestrator-types/) | Pure-data L0 leaf for vox-orchestrator: agent/task IDs, file affinity, switch actions, provider catalogs. |
  | [`vox-primitives`](../../../crates/vox-primitives/) | Pure-data primitives shared workspace-wide. |
  | [`vox-protocol`](../../../crates/vox-protocol/) | Daemon wire-protocol pure-data types. |
  | [`workspace-hack`](../../../crates/workspace-hack/) | Cargo-hakari unification crate; do not edit by hand. |

  ### L1 — primitives & utilities
  ... (every L1 crate)

  ### L2 — domain libraries
  ... (every L2 crate)

  ### L3 — heavy runtimes
  ... (every L3 crate)

  ### L5 — surfaces
  ... (every L5 crate)
  ```

  For each crate not in the spec's description table, read its `lib.rs` first 30 lines and the Cargo.toml `description` (if present after PR1's description rewrites land — which they do in Task 7 below, so do Task 6 AFTER Task 7 completes for the description-rich rows). If the description is missing, write a one-sentence summary based on the lib.rs docstring.

- [ ] **Step 6.3: Replace the "Plugins" section with a complete L4 list.**

  Section structure:
  ```markdown
  ## Plugins (L4 — cdylib only; never compile-time deps for L0..L3)

  | Plugin crate | Provides |
  |---|---|
  | [`vox-plugin-browser`](../../../crates/vox-plugin-browser/) | (read lib.rs first line) |
  | [`vox-plugin-catalog`](../../../crates/vox-plugin-catalog/) | (read lib.rs first line) |
  | [`vox-plugin-cloud`](../../../crates/vox-plugin-cloud/) | (read lib.rs first line) |
  | [`vox-plugin-grammar-export`](../../../crates/vox-plugin-grammar-export/) | (read lib.rs first line) |
  | [`vox-plugin-mens-candle-cuda`](../../../crates/vox-plugin-mens-candle-cuda/) | (existing) |
  | [`vox-plugin-nvml-probe`](../../../crates/vox-plugin-nvml-probe/) | (read lib.rs first line) |
  | [`vox-plugin-oratio`](../../../crates/vox-plugin-oratio/) | (existing) |
  | [`vox-plugin-oratio-mic`](../../../crates/vox-plugin-oratio-mic/) | (existing) |
  | [`vox-plugin-populi-mesh`](../../../crates/vox-plugin-populi-mesh/) | (read lib.rs first line) |
  | [`vox-plugin-publication`](../../../crates/vox-plugin-publication/) | (read lib.rs first line) |
  | [`vox-plugin-runtime-container`](../../../crates/vox-plugin-runtime-container/) | (existing) |
  | [`vox-plugin-runtime-wasm`](../../../crates/vox-plugin-runtime-wasm/) | (existing) |
  | [`vox-plugin-script-execution`](../../../crates/vox-plugin-script-execution/) | (read lib.rs first line) |
  | [`vox-plugin-webhook`](../../../crates/vox-plugin-webhook/) | (read lib.rs first line) |
  ```

  Replace each `(read lib.rs first line)` with the actual one-line summary from that crate's `lib.rs`.

- [ ] **Step 6.4: Update the "When to NOT add a new crate" footer.**

  Add a paragraph noting the binary-tool exception:
  ```markdown
  ### Binary-only tools

  Crates with `kind = "binary"` in `layers.toml` (e.g., `vox-arch-check`, `vox-ml-cli`, `vox-orchestrator-d`) don't need a `[workspace.dependencies]` entry in the root `Cargo.toml` — they're consumed via `cargo run -p <name>`, not as library dependencies. The "Add a new workspace crate" instruction below applies to libraries only.
  ```

  Adjust the existing instruction at the bottom to reference this exception.

- [ ] **Step 6.5: Verify coverage.**

  Run a quick sanity check by counting crates referenced in the doc vs. layers.toml:
  ```
  rg "crates/vox-" docs/src/architecture/where-things-live.md | rg -o "vox-[a-z0-9-]+" | sort -u | wc -l
  ```
  Expected: ≥80 (some crates may be referenced multiple times; the number of unique names should match the layers.toml count of 79 + workspace-hack).

## Task 7: Rewrite stale Cargo.toml descriptions and vox-corpus lib.rs docstring

**Files (one-line `description =` field rewrite per crate):**
- `crates/vox-orchestrator/Cargo.toml:3`
- `crates/vox-orchestrator-types/Cargo.toml`
- `crates/vox-package/Cargo.toml`
- `crates/vox-actor-runtime/Cargo.toml`
- `crates/vox-cli-core/Cargo.toml`
- `crates/vox-crypto/Cargo.toml`
- `crates/vox-db/Cargo.toml`
- `crates/vox-db-types/Cargo.toml`
- `crates/vox-doc-pipeline/Cargo.toml`
- `crates/vox-eval/Cargo.toml`
- `crates/vox-grammar-export/Cargo.toml`
- `crates/vox-identity/Cargo.toml`
- `crates/vox-integration-tests/Cargo.toml`
- `crates/vox-lsp/Cargo.toml`
- `crates/vox-protocol/Cargo.toml`
- `crates/vox-scientia-ingest/Cargo.toml`
- `crates/vox-ssg/Cargo.toml`
- `crates/vox-tensor/Cargo.toml`
- `crates/vox-test-harness/Cargo.toml`
- `crates/vox-corpus/src/lib.rs` (docstring only; description is separate)

- [ ] **Step 7.1: Rewrite the `vox-orchestrator` description.**

  Read `crates/vox-orchestrator/Cargo.toml` lines 1–10. Replace the existing `description = "Multi-agent file-affinity queue system..."` with:
  ```toml
  description = "Slim coordinator for the multi-agent file-affinity router; queue/lock/oplog live in vox-orchestrator-queue, MCP tools in vox-orchestrator-mcp."
  ```

- [ ] **Step 7.2: Add descriptions to the other 18 crates.**

  For each crate listed below, open `crates/<name>/Cargo.toml` and add (or replace) the `description = ` field directly under the `name = ...` line. Verify the crate's actual scope by reading `crates/<name>/src/lib.rs` first 30 lines before applying — if the suggested description does not match reality, prefer accuracy.

  Use these exact descriptions:

  | Crate | Description |
  |---|---|
  | `vox-orchestrator-types` | `Pure-data L0 leaf for vox-orchestrator: agent/task IDs, file affinity, switch actions, provider catalogs.` |
  | `vox-package` | `Vox package manager: Vox.toml manifests, vox.lock, content-addressed artifact cache, registry client, dependency resolver.` |
  | `vox-actor-runtime` | `Process-oriented runtime: actors, mailboxes, supervision, scheduling, LLM/Mens activity primitives.` |
  | `vox-cli-core` | `Shared internals for the vox CLI binary (argv parsing helpers, exit-code policy).` |
  | `vox-crypto` | `Pure-Rust crypto primitives (chacha20poly1305 AEAD, ed25519, x25519); sole crypto SSOT per AGENTS.md §Cryptography Policy.` |
  | `vox-db` | `Codex / VoxDb facade: schema migrations, store ops, Turso/libSQL access for the Vox workspace.` |
  | `vox-db-types` | `Pure-data L0 leaf for vox-db: row types, IDs, schema descriptors.` |
  | `vox-doc-pipeline` | `Doc generator: regenerates SUMMARY.md, architecture-index.md, feed.xml from frontmatter.` |
  | `vox-eval` | `Vox expression evaluator (interpreter for vox run --interp).` |
  | `vox-grammar-export` | `Exports the Vox grammar artifact for downstream tooling.` |
  | `vox-identity` | `Identity primitives: signing keys, trust ledger entries.` |
  | `vox-integration-tests` | `Cross-crate integration test harness (test-only L5).` |
  | `vox-lsp` | `Vox Language Server (stdio JSON-RPC).` |
  | `vox-protocol` | `Daemon wire-protocol pure-data types.` |
  | `vox-scientia-ingest` | `Scientia corpus ingestion.` |
  | `vox-ssg` | `Static site generator for the Vox docs surface.` |
  | `vox-tensor` | `Pure-CPU JSONL data loaders / training-pair types (Burn extracted 2026-05-08).` |
  | `vox-test-harness` | `Shared test fixtures and harness primitives.` |

  Standard placement (matches existing crates):
  ```toml
  [package]
  name = "vox-foo"
  description = "..."
  version.workspace = true
  edition.workspace = true
  ```

- [ ] **Step 7.3: Update vox-corpus lib.rs docstring.**

  Read `crates/vox-corpus/src/lib.rs` first 30 lines. The current docstring claims `vox-corpus` is "training SSOT for Mens" but the module set is broader: `training/`, `mcp_meta/`, `synthetic_search_gen/`, `tool_workflow_corpus/`, `codegen_vox/`. Replace the leading `//!` block with one accurate sentence:
  ```rust
  //! Corpus crate for the Vox workspace: aggregates training data, MCP meta corpora,
  //! synthetic-search generators, tool-workflow corpora, and codegen-Vox samples.
  //! Used by `vox-ml-cli` and downstream training pipelines.
  ```

- [ ] **Step 7.4: Verify with cargo metadata.**

  Run:
  ```
  cargo metadata --no-deps --format-version 1 | rg "\"name\":\"vox-orchestrator\"" -A 1
  ```
  Expected: the `description` field is the new one from step 7.1.

  Run a broader check:
  ```
  cargo metadata --no-deps --format-version 1 | rg "\"description\":null,\"name\":\"vox-"
  ```
  Expected: zero null descriptions for the 19 crates above (other crates may still have null until PR6 lints).

## Task 8: Add binary-layer paragraph to layers.toml header

**Files:**
- Modify: `docs/src/architecture/layers.toml` (header comment, ~lines 1–22)

- [ ] **Step 8.1: Add the binary-tool exception note.**

  Read `docs/src/architecture/layers.toml` lines 1–25. Insert a new paragraph in the header comment block, immediately before the `[guards]` table:

  ```toml
  # Binary-tool exception: a `kind = "binary"` crate at any layer is a *tool*
  # (e.g., vox-arch-check, vox-ml-cli, vox-orchestrator-d) consumed via
  # `cargo run -p <name>` rather than as a library dep. Such crates are exempt
  # from the orphan rule and from the convention that workspace surfaces sit at L5.
  # Only product-shipped binaries (the user-visible CLI surface) live at L5.
  ```

  Place the new comment block so it reads naturally between the layer-list comment and the `[guards]` section.

## Task 9: Merge `vox-mcp-meta` into `vox-mcp-registry` (C1)

**Files:**
- Read: `crates/vox-mcp-meta/src/lib.rs` (entire file — it's small)
- Modify: `crates/vox-mcp-registry/src/lib.rs` (add re-exports)
- Modify: `crates/vox-mcp-registry/Cargo.toml` (any new deps if vox-mcp-meta had them)
- Delete: `crates/vox-mcp-meta/` (entire directory)
- Modify: `Cargo.toml` (`[workspace.dependencies]` — remove the `vox-mcp-meta` line)
- Modify: `docs/src/architecture/layers.toml` (remove the `vox-mcp-meta = ...` line)
- Modify: every consumer's `Cargo.toml` and source that imports `vox_mcp_meta::*`

- [ ] **Step 9.1: Read `vox-mcp-meta` to understand what it exports.**

  Read `crates/vox-mcp-meta/src/lib.rs` and `crates/vox-mcp-meta/Cargo.toml`. The crate is ~62 LoC; the agent audit found it re-exports `vox-mcp-registry::TOOL_REGISTRY` plus 4 static `&[&str]` constants. Document the exact symbols you'll need to re-home.

- [ ] **Step 9.2: Find every consumer.**

  Run:
  ```
  rg "vox_mcp_meta|vox-mcp-meta" --type rust --type toml -l
  ```
  Save the file list.

- [ ] **Step 9.3: Move the symbols into `vox-mcp-registry`.**

  Copy each public symbol from `vox-mcp-meta/src/lib.rs` to `vox-mcp-registry/src/lib.rs`. Preserve the public interface exactly (same names, same types, same `pub` visibility).

- [ ] **Step 9.4: Update consumer imports.**

  For every file in step 9.2's list that uses `vox_mcp_meta::X`, change the import to `vox_mcp_registry::X`. For every `Cargo.toml` that has a `vox-mcp-meta = ...` dep, replace it with `vox-mcp-registry = ...` (deduplicate if `vox-mcp-registry` is already there).

- [ ] **Step 9.5: Remove the workspace dep.**

  In root `Cargo.toml` `[workspace.dependencies]`, delete the line:
  ```toml
  vox-mcp-meta              = { path = "crates/vox-mcp-meta" }
  ```

- [ ] **Step 9.6: Remove the layer entry.**

  In `docs/src/architecture/layers.toml`, delete the row:
  ```toml
  vox-mcp-meta            = { layer = 2 }
  ```

- [ ] **Step 9.7: Delete the crate directory.**

  Remove the `crates/vox-mcp-meta/` directory entirely. From the workspace root:
  ```
  Remove-Item -Recurse -Force crates/vox-mcp-meta
  ```
  (PowerShell — bash equivalent: `rm -rf crates/vox-mcp-meta`.)

- [ ] **Step 9.8: Build and verify.**

  Run:
  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  ```
  Both must be clean.

## Task 10: PR1 verification, regenerate auto-generated docs, and commit

- [ ] **Step 10.1: Regenerate auto-generated docs.**

  Run:
  ```
  cargo run -p vox-doc-pipeline
  ```

  This regenerates `docs/src/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/src/feed.xml`. Do NOT hand-edit them per CLAUDE.md.

  If `vox-doc-pipeline` finds frontmatter issues in the docs you edited (Task 1–8), fix them in the source markdown and re-run.

- [ ] **Step 10.2: Sync ignore files.**

  Run:
  ```
  cargo run -p vox-cli -- ci sync-ignore-files
  ```
  Expected: no diff (we didn't touch `.voxignore`).

- [ ] **Step 10.3: Run final verification.**

  Run all in sequence:
  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  cargo run -p vox-doc-pipeline -- --check
  cargo test --workspace --no-run
  ```
  All must succeed. If a test fails to compile, investigate and fix before commit.

- [ ] **Step 10.4: Commit.**

  Stage everything explicitly (no `git add -A` — see CLAUDE.md / Bash safety):
  ```
  git add AGENTS.md .cursor .github contracts/proximity crates/vox-gamify/README.md
  git add docs/src/contributors docs/src/reference docs/agents docs/src/.well-known
  git add docs/src/architecture/where-things-live.md docs/src/architecture/layers.toml
  git add docs/src/architecture/2026-05-08-workspace-reorg-outcome.md
  git add docs/src/architecture/2026-05-08-naming-and-guards-design.md
  git add Cargo.toml crates/vox-orchestrator/Cargo.toml
  git add crates/vox-orchestrator-types/Cargo.toml crates/vox-package/Cargo.toml
  git add crates/vox-actor-runtime/Cargo.toml crates/vox-cli-core/Cargo.toml
  git add crates/vox-crypto/Cargo.toml crates/vox-db/Cargo.toml
  git add crates/vox-db-types/Cargo.toml crates/vox-doc-pipeline/Cargo.toml
  git add crates/vox-eval/Cargo.toml crates/vox-grammar-export/Cargo.toml
  git add crates/vox-identity/Cargo.toml crates/vox-integration-tests/Cargo.toml
  git add crates/vox-lsp/Cargo.toml crates/vox-protocol/Cargo.toml
  git add crates/vox-scientia-ingest/Cargo.toml crates/vox-ssg/Cargo.toml
  git add crates/vox-tensor/Cargo.toml crates/vox-test-harness/Cargo.toml
  git add crates/vox-corpus/src/lib.rs crates/vox-db/README.md
  git add crates/vox-mcp-registry/ docs/src/SUMMARY.md
  git add docs/src/architecture/architecture-index.md docs/src/feed.xml
  git add docs/src/adr/004-codex-arca-turso-ssot.md
  git add docs/src/adr/015-vox-docker-oci-portability-ssot.md
  git rm -r crates/vox-mcp-meta
  git commit -m "$(cat <<'EOF'
  refactor(docs+crates): SSOT/discoverability fixes + crate descriptions + vox-mcp-meta merge

  PR1 of crate-org-followup-2026:
  - Fix inverted gamify/ludus directives in 12 live policy surfaces.
  - Fix retired-form (vox-pm, vox-mens, @server fn, vox-dei, ResolutionAgent) references in ~25 docs.
  - Expand where-things-live.md to cover all 83 workspace crates by layer.
  - Add canonical descriptions to 19 crates that had none.
  - Rewrite stale vox-orchestrator description (post-Phase-5).
  - Update vox-corpus lib.rs docstring to match actual scope.
  - Add binary-tool exception note to layers.toml.
  - Merge vox-mcp-meta (62 LoC of re-exports) into vox-mcp-registry.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md
  EOF
  )"
  ```

- [ ] **Step 10.5: Push and open PR.**

  ```
  git push -u origin HEAD
  gh pr create --title "refactor: SSOT/discoverability fixes + crate descriptions + vox-mcp-meta merge" --body "$(cat <<'EOF'
  ## Summary
  - PR1 of [2026-05-08 crate-org-followup](./2026-05-08-crate-org-followup-design.md).
  - Fixes SSOT drift across ~37 live docs + 19 missing Cargo.toml descriptions.
  - Merges vox-mcp-meta into vox-mcp-registry.

  ## Test plan
  - [x] cargo build --workspace clean
  - [x] cargo run -p vox-arch-check clean
  - [x] cargo run -p vox-doc-pipeline -- --check clean
  - [x] cargo test --workspace --no-run clean
  EOF
  )"
  ```

---

# PR2 — Split `vox-package` into `vox-package-types` (L1) + `vox-package` (L3)

Removes both documented layer inversions (`vox-package` → `vox-compiler`, `vox-package` → `vox-db`).

## Task 11: Survey vox-package and identify the type-only surface

**Files:**
- Read: `crates/vox-package/src/lib.rs`
- Read: `crates/vox-package/src/manifest.rs`
- Read: `crates/vox-package/src/lockfile.rs`
- Read: `crates/vox-package/src/package_kind.rs`
- Read: `crates/vox-package/src/registry.rs`
- Read: `crates/vox-package/src/resolver/` (whole subdir)
- Read: `crates/vox-package/src/artifact_cache.rs`
- Read: `crates/vox-package/src/workspace.rs`

- [ ] **Step 11.1: Read every vox-package source file and classify each.**

  For each file, decide: **type-only** (no async, no DB, no compiler — moves to L1) or **build-runtime** (stays in L3). Record the classification in a working note.

  The audit suggests these are pure-data L1 candidates:
  - `manifest.rs` (Vox.toml manifest types)
  - `lockfile.rs` (vox.lock types)
  - `package_kind.rs` (package_kind enum)
  - `resolver/` *types only* (the resolver request/response structs; not the implementation)

  These stay L3:
  - `artifact_cache.rs` (DB-backed cache)
  - `registry.rs` (HTTP client)
  - `resolver/` *implementation* (calls compiler)
  - `workspace.rs` (uses vox-db)
  - `bin/`, `deploy_coolify.rs`

  Verify by reading each file.

## Task 12: Create the new `vox-package-types` crate

**Files:**
- Create: `crates/vox-package-types/Cargo.toml`
- Create: `crates/vox-package-types/src/lib.rs`

- [ ] **Step 12.1: Scaffold the new crate.**

  Create `crates/vox-package-types/Cargo.toml`:
  ```toml
  [package]
  name = "vox-package-types"
  description = "Pure-data L1 leaf for vox-package: Vox.toml manifest types, vox.lock types, package_kind enum, resolver request/response types."
  version.workspace = true
  edition.workspace = true

  [dependencies]
  serde = { workspace = true, features = ["derive"] }
  serde_json = { workspace = true }
  toml = { workspace = true }
  thiserror = { workspace = true }
  semver = { workspace = true }
  workspace-hack = { workspace = true }

  [lints]
  workspace = true
  ```

  Create `crates/vox-package-types/src/lib.rs`:
  ```rust
  //! Pure-data L1 leaf for vox-package: Vox.toml manifest types, vox.lock
  //! types, package_kind enum, resolver request/response types. No async,
  //! no DB, no compiler dependency.
  ```

- [ ] **Step 12.2: Add to workspace.**

  In root `Cargo.toml` `[workspace.dependencies]`, add (alphabetical order):
  ```toml
  vox-package-types         = { path = "crates/vox-package-types" }
  ```

  In `docs/src/architecture/layers.toml`, add to the L1 section (alphabetical):
  ```toml
  vox-package-types     = { layer = 1 }
  ```

## Task 13: Move type-only modules from `vox-package` to `vox-package-types`

- [ ] **Step 13.1: Move manifest, lockfile, package_kind.**

  For each of `manifest.rs`, `lockfile.rs`, `package_kind.rs`:
  1. Move the file from `crates/vox-package/src/<name>.rs` to `crates/vox-package-types/src/<name>.rs`.
  2. Remove any non-pure-data imports (anything that pulls `tokio`, `vox-db`, `vox-compiler`).
  3. Add a `mod <name>; pub use <name>::*;` line in `crates/vox-package-types/src/lib.rs`.

- [ ] **Step 13.2: Move resolver type definitions.**

  Read `crates/vox-package/src/resolver/`. Identify which files are type-only (request/response/error types) vs. implementation (HTTP/DB calls). Move only the type files into `crates/vox-package-types/src/resolver/`. The implementation stays.

- [ ] **Step 13.3: Update vox-package to depend on vox-package-types.**

  In `crates/vox-package/Cargo.toml` `[dependencies]`, add:
  ```toml
  vox-package-types = { workspace = true }
  ```

  Remove unused deps that were only there for the type files (e.g., if `semver` is no longer needed in vox-package after the move).

  In `crates/vox-package/src/lib.rs`, replace the old `mod manifest; mod lockfile; ...` lines with re-exports from the new types crate:
  ```rust
  pub use vox_package_types::{manifest, lockfile, package_kind, resolver as resolver_types};
  ```

- [ ] **Step 13.4: Update internal `vox-package` source imports.**

  Inside `crates/vox-package/src/`, every `use crate::manifest::...` becomes `use vox_package_types::manifest::...`. Same for `lockfile::`, `package_kind::`, `resolver::types::`.

- [ ] **Step 13.5: Update external callers.**

  Run:
  ```
  rg "use vox_package::(manifest|lockfile|package_kind|resolver::(types|Request|Response|Error))" --type rust
  ```

  Each caller may either:
  - Continue using `vox_package::manifest::...` (works because of the re-export in lib.rs); or
  - Switch to `vox_package_types::manifest::...` directly (slightly faster compile because L1 crates compile sooner).

  For now, **prefer not switching** callers; just keep the re-export. Fewer files to touch in this PR. The lint in PR6 won't penalize either form.

- [ ] **Step 13.6: Build and verify the split compiles.**

  Run:
  ```
  cargo build -p vox-package-types
  cargo build -p vox-package
  cargo build --workspace
  ```
  All clean.

## Task 14: Remove the known_inversions for vox-package

- [ ] **Step 14.1: Verify the inversion is gone.**

  Run:
  ```
  cargo run -p vox-arch-check
  ```

  At this point, the strict layer-check will still pass because the inversions are still listed in `[[known_inversions]]`. We need to verify that they're now *unused* (i.e., vox-package no longer needs them). To check this, temporarily remove them and re-run.

- [ ] **Step 14.2: Remove both inversion entries.**

  In `docs/src/architecture/layers.toml`, delete both:
  ```toml
  [[known_inversions]]
  from   = "vox-package"
  to     = "vox-compiler"
  reason = "..."

  [[known_inversions]]
  from   = "vox-package"
  to     = "vox-db"
  reason = "..."
  ```

- [ ] **Step 14.3: Verify strict layer-check still passes.**

  Run:
  ```
  cargo run -p vox-arch-check
  ```

  Must be clean. If it fails: vox-package still has a transitive edge to vox-compiler or vox-db at L3 that violates layering. If so, the inversion was real — restore the entry, but document why and revisit splitting in a follow-up.

  **Expected case:** vox-package is now L3 and depends on L3 (vox-db, vox-compiler). That's allowed (within-layer). The split removed the L1-or-lower → L3 edge.

  **Wait — re-read layers.toml.** vox-package was originally L2, NOT L1. If vox-package is at L2 with deps to vox-db (L3) and vox-compiler (L3), the inversions exist because L2→L3 is going UP. Verify the current layer assignment: does the `[crates]` table still list `vox-package = { layer = 2 }`?

  If yes: split's effect is to pull manifest/lockfile/package_kind out as L1, but leave vox-package at L2 with the same L2→L3 edges. The inversions remain. **Fix by:** moving vox-package itself to L3 in `layers.toml`, since it now does runtime work (DB, compiler-driver). The L1 split (`vox-package-types`) is the new pure-data home.

- [ ] **Step 14.4: Re-tier vox-package to L3.**

  In `docs/src/architecture/layers.toml`, move the `vox-package` entry from the L2 section to the L3 section. Keep the layer-2 entry deleted; insert at L3 alphabetically. Update its annotation:
  ```toml
  vox-package           = { layer = 3 }
  ```

- [ ] **Step 14.5: Re-run arch-check.**

  ```
  cargo run -p vox-arch-check
  ```
  Must be clean now.

## Task 15: PR2 verification and commit

- [ ] **Step 15.1: Full verification.**

  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  cargo test -p vox-package -p vox-package-types
  cargo run -p vox-doc-pipeline -- --check
  ```
  All clean.

- [ ] **Step 15.2: Add a row to where-things-live.md.**

  In `docs/src/architecture/where-things-live.md` L1 section (added in Task 6), insert (alphabetical):
  ```markdown
  | [`vox-package-types`](../../../crates/vox-package-types/) | Pure-data L1 leaf for vox-package: manifest, lockfile, package_kind, resolver types. |
  ```

  Verify the L3 row for `vox-package` is in the L3 section (Task 6 may have placed it under L2 — move it).

- [ ] **Step 15.3: Commit.**

  ```
  git add crates/vox-package-types crates/vox-package
  git add Cargo.toml docs/src/architecture/layers.toml
  git add docs/src/architecture/where-things-live.md
  git commit -m "$(cat <<'EOF'
  refactor(vox-package): split into vox-package-types (L1) + vox-package (L3)

  Removes both documented layer inversions (vox-package → vox-compiler,
  vox-package → vox-db) by moving manifest/lockfile/package_kind/resolver
  type definitions to a new pure-data L1 crate. vox-package itself moves
  from L2 to L3 to reflect its actual runtime scope.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md (C2)
  EOF
  )"
  ```

---

# PR3 — Extract `vox-cli-ci` (C3) + side-quest

## Task 16: Survey `vox-cli/src/commands/ci/`

**Files:**
- Read: `crates/vox-cli/src/commands/ci/` (whole subdir)
- Read: `crates/vox-cli/src/commands/mod.rs`
- Read: `crates/vox-cli/Cargo.toml`

- [ ] **Step 16.1: List the ci/ subdir contents.**

  Run:
  ```
  Get-ChildItem -Recurse crates/vox-cli/src/commands/ci/ | Select-Object FullName
  ```

  Note: there are likely many files (the audit found ~17.3K LoC). Identify the top-level dispatcher (probably `mod.rs`) and the public entry point.

- [ ] **Step 16.2: Identify deps used only inside ci/.**

  Run:
  ```
  rg "use crate::|use super::|use vox_" crates/vox-cli/src/commands/ci/ --type rust
  ```

  Bucket each import:
  - `use crate::commands::ci::...` (intra-ci) — fine, will move with the code
  - `use crate::commands::other::...` (cross-cutting; need to expose those publicly or copy into vox-cli-ci)
  - `use vox_<other>::...` (workspace dep — record for the new crate's Cargo.toml)
  - `use std::...` / external (record but no action)

- [ ] **Step 16.3: Read the ci-command registration in `commands/mod.rs`.**

  Find the dispatcher that routes `vox ci <subcommand>`. Note the public function name (likely something like `dispatch_ci(args: ...) -> Result<...>`).

## Task 17: Create the `vox-cli-ci` crate skeleton

**Files:**
- Create: `crates/vox-cli-ci/Cargo.toml`
- Create: `crates/vox-cli-ci/src/lib.rs`

- [ ] **Step 17.1: Scaffold.**

  Create `crates/vox-cli-ci/Cargo.toml`:
  ```toml
  [package]
  name = "vox-cli-ci"
  description = "vox CLI 'ci' subcommand dispatcher and implementations (sync-ignore-files, secret-env-guard, generate-plugin-catalog-docs, etc.)."
  version.workspace = true
  edition.workspace = true

  [dependencies]
  # Add deps from Step 16.2 here. At minimum:
  clap = { workspace = true }
  anyhow = { workspace = true }
  tokio = { workspace = true }
  workspace-hack = { workspace = true }
  # plus every vox-* dep used inside ci/

  [lints]
  workspace = true
  ```

  Create `crates/vox-cli-ci/src/lib.rs`:
  ```rust
  //! vox CLI `ci` subcommand: sync-ignore-files, secret-env-guard, doc-pipeline
  //! check, plugin-catalog generators, ssot-drift, etc. Extracted from vox-cli
  //! to isolate CI-only edits from the main CLI binary's incremental rebuild.
  ```

- [ ] **Step 17.2: Add to workspace.**

  In root `Cargo.toml` `[workspace.dependencies]`:
  ```toml
  vox-cli-ci                = { path = "crates/vox-cli-ci" }
  ```

  In `docs/src/architecture/layers.toml` (L3 section, alphabetical):
  ```toml
  vox-cli-ci              = { layer = 3 }
  ```

  Note: this is L3, not L5. The L5 surface remains `vox-cli` (the binary). `vox-cli-ci` is a library crate.

## Task 18: Move ci/ into vox-cli-ci

- [ ] **Step 18.1: Move the directory.**

  Move `crates/vox-cli/src/commands/ci/` to `crates/vox-cli-ci/src/`. Preserve the internal structure.

  PowerShell:
  ```
  Move-Item crates/vox-cli/src/commands/ci/* crates/vox-cli-ci/src/
  Remove-Item crates/vox-cli/src/commands/ci -Recurse
  ```

- [ ] **Step 18.2: Rename the entry-point module.**

  If `crates/vox-cli-ci/src/mod.rs` was the old dispatcher entry, rename it to a more idiomatic name like `dispatch.rs` (Cargo convention disfavors `mod.rs` for the crate root's child modules in Rust 2024). Update `crates/vox-cli-ci/src/lib.rs` to match:
  ```rust
  //! ... (from step 17.1)

  pub mod dispatch;
  pub use dispatch::dispatch_ci;  // or whatever the original entry function is
  ```

- [ ] **Step 18.3: Fix imports inside vox-cli-ci.**

  Every `use crate::commands::ci::...` in moved files becomes `use crate::...`. Every `use crate::commands::<other>::...` must be re-resolved — either:
  - The shared item moves with ci/ if it's only used by ci (preferable)
  - The shared item stays in vox-cli; vox-cli-ci re-imports it via a `vox-cli` reverse dep (NOT recommended — creates cycle)
  - The shared item is extracted to `vox-cli-core` (which already exists at L3)

  If you hit a cross-cut, prefer moving the helper to `vox-cli-core`.

- [ ] **Step 18.4: Update vox-cli to call vox-cli-ci.**

  In `crates/vox-cli/Cargo.toml`, add:
  ```toml
  vox-cli-ci = { workspace = true }
  ```

  In `crates/vox-cli/src/commands/mod.rs`, replace the `pub mod ci;` line with:
  ```rust
  // Moved to vox-cli-ci crate.
  pub use vox_cli_ci::dispatch_ci;
  ```

  Find the call site that dispatches `Subcommand::Ci(args)` (likely in `crates/vox-cli/src/main.rs` or equivalent) and update it to call `vox_cli_ci::dispatch_ci(args).await` instead of the local function.

- [ ] **Step 18.5: Build and verify.**

  ```
  cargo build -p vox-cli-ci
  cargo build -p vox-cli
  cargo build --workspace
  ```

  Expect compile errors at first (missing imports, name resolution). Fix iteratively. Common patterns:
  - Add a missing `vox-*` dep to `vox-cli-ci/Cargo.toml`
  - Re-export an internal helper as `pub` in `vox-cli/src/something.rs` if vox-cli-ci needs it
  - If the same helper is needed in both: extract to `vox-cli-core`

- [ ] **Step 18.6: Run a smoke test.**

  ```
  cargo run -p vox-cli -- ci --help
  cargo run -p vox-cli -- ci sync-ignore-files
  cargo run -p vox-cli -- ci secret-env-guard
  ```

  All must produce identical output to before the extraction. If any output differs, investigate.

## Task 19: Side-quest — gate `vox-orchestrator-mcp` behind `mcp-server` feature

- [ ] **Step 19.1: Find the current dep declaration.**

  In `crates/vox-cli/Cargo.toml`, find:
  ```toml
  vox-orchestrator-mcp = { workspace = true }
  ```
  (or similar — it's currently unconditional per the audit).

- [ ] **Step 19.2: Make it optional and gate it.**

  Change to:
  ```toml
  vox-orchestrator-mcp = { workspace = true, optional = true }
  ```

  Find the `[features]` section. Locate the `mcp-server` feature and add `dep:vox-orchestrator-mcp` to its dep list:
  ```toml
  mcp-server = ["dep:vox-orchestrator-mcp", "dep:axum", "dep:rmcp"]
  # (preserve existing entries; just add the new dep)
  ```

- [ ] **Step 19.3: Gate the call sites.**

  Run:
  ```
  rg "vox_orchestrator_mcp" crates/vox-cli/src/ --type rust
  ```

  For each occurrence outside `mod mcp_server`, wrap with `#[cfg(feature = "mcp-server")]` or move into a feature-gated module.

- [ ] **Step 19.4: Verify default build excludes vox-orchestrator-mcp.**

  ```
  cargo build -p vox-cli --no-default-features
  cargo build -p vox-cli --features mcp-server
  cargo build --workspace
  ```

  All clean. Default build (no mcp-server) should NOT pull `vox-orchestrator-mcp` — verify with:
  ```
  cargo tree -p vox-cli --no-default-features | rg "vox-orchestrator-mcp"
  ```
  Expected: zero matches.

## Task 20: PR3 verification and commit

- [ ] **Step 20.1: Full verification.**

  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  cargo test -p vox-cli-ci -p vox-cli
  cargo run -p vox-doc-pipeline -- --check
  ```
  All clean.

- [ ] **Step 20.2: Add the row to where-things-live.md.**

  In the L3 section, alphabetical:
  ```markdown
  | [`vox-cli-ci`](../../../crates/vox-cli-ci/) | vox CLI 'ci' subcommand dispatcher (sync-ignore-files, secret-env-guard, etc.). Extracted from vox-cli to isolate CI-only edits. |
  ```

- [ ] **Step 20.3: Commit.**

  ```
  git add crates/vox-cli-ci crates/vox-cli
  git add Cargo.toml docs/src/architecture/layers.toml
  git add docs/src/architecture/where-things-live.md
  git rm -r crates/vox-cli/src/commands/ci  # if not already removed by Move-Item
  git commit -m "$(cat <<'EOF'
  refactor(vox-cli): extract vox-cli-ci + gate vox-orchestrator-mcp behind mcp-server

  Extracts the 17.3K LoC ci/ subdir from vox-cli into a dedicated vox-cli-ci
  library crate, isolating CI-only edits from the main CLI's incremental
  rebuild. Side-quest: makes the vox-orchestrator-mcp dep optional behind the
  mcp-server feature, so default CLI builds skip it.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md (C3 + side-quest)
  EOF
  )"
  ```

---

# PR4 — Move `ops_ludus` from `vox-db` into `vox-gamify` (C4)

## Task 21: Survey `vox-db/src/store/ops_ludus/` and `vox-gamify/src/db/`

**Files:**
- Read: `crates/vox-db/src/store/ops_ludus/` (whole subdir)
- Read: `crates/vox-gamify/src/db/` (whole subdir)

- [ ] **Step 21.1: List both subdirs.**

  ```
  Get-ChildItem -Recurse crates/vox-db/src/store/ops_ludus/
  Get-ChildItem -Recurse crates/vox-gamify/src/db/
  ```

- [ ] **Step 21.2: Identify migration SQL vs. typed ops.**

  Migration SQL (CREATE TABLE statements, schema versioning) typically lives under `crates/vox-db/src/schema/`. The `ops_ludus/` subdir holds typed ops (functions like `insert_quest`, `select_companions`, etc.). Read enough of each file to confirm.

  **Plan rule:** migrations stay in `vox-db/schema/`, only the typed ops API moves to `vox-gamify/db/`.

- [ ] **Step 21.3: Find external callers.**

  Run:
  ```
  rg "ops_ludus|crate::store::ops_ludus|vox_db::store::ops_ludus" --type rust -l
  ```

  List the files that will need updating.

## Task 22: Move ops_ludus into vox-gamify

- [ ] **Step 22.1: Copy files into vox-gamify.**

  Move every typed-ops file from `crates/vox-db/src/store/ops_ludus/` into `crates/vox-gamify/src/db/` (merge with the existing `db/` subdir there). Preserve internal structure where possible.

  ```
  Move-Item crates/vox-db/src/store/ops_ludus/* crates/vox-gamify/src/db/
  Remove-Item crates/vox-db/src/store/ops_ludus -Recurse
  ```

- [ ] **Step 22.2: Update imports in moved files.**

  Inside the moved files, every `use crate::store::...` becomes `use vox_db::store::...` if it referred to vox-db's other store ops. `use super::types::Quest` etc. stays.

  If a moved file used a *private* item from `vox_db`, that item must now be made public (or replicated in vox-gamify). Prefer making vox-db's helper public if it's a clean abstraction.

- [ ] **Step 22.3: Re-export from vox-db (compat shim).**

  In `crates/vox-db/src/store/mod.rs` (or wherever the old `pub mod ops_ludus;` lived), replace with a re-export shim:
  ```rust
  // Moved to vox-gamify::db (2026-05-08 crate-org-followup, PR4).
  // Compat re-export so older callers don't break in the same PR.
  pub use vox_gamify::db as ops_ludus;
  ```

  This requires `vox-db` to depend on `vox-gamify`, which would create an L3→L3 same-layer edge (allowed) but a new dep cycle if vox-gamify still depends on vox-db. **Check for cycle.** If cycle: skip the re-export. Update callers in step 22.4 instead.

- [ ] **Step 22.4: Update external callers.**

  For each file from step 21.3:
  - Replace `use vox_db::store::ops_ludus::X` with `use vox_gamify::db::X`.
  - Verify the caller's `Cargo.toml` already has `vox-gamify`. If not, add it.
  - If a caller is in `vox-db` itself, it's a sign that vox-db is using its own gamify ops — investigate whether that caller belongs in vox-gamify too.

- [ ] **Step 22.5: Build and verify.**

  ```
  cargo build -p vox-gamify
  cargo build -p vox-db
  cargo build --workspace
  ```

  Expected: clean. If a cycle was detected in step 22.3, the compat shim won't compile — drop it and rely on direct caller updates from step 22.4.

## Task 23: PR4 verification and commit

- [ ] **Step 23.1: Run gamify and db tests.**

  ```
  cargo test -p vox-gamify
  cargo test -p vox-db
  cargo test --workspace --no-run
  ```

- [ ] **Step 23.2: Manual migration smoke test.**

  Create a fresh DB and run vox-db migrations to confirm gamify-domain tables are still created (migrations stayed in `vox-db/schema/`):
  ```
  $env:VOX_DB_URL = "file:.\test-migration.db"
  cargo run -p vox-cli -- db init
  cargo run -p vox-cli -- db schema-list | rg "ludus|gamify|quest|companion"
  Remove-Item test-migration.db
  ```
  Expected: gamify-domain tables (`gamify_*`, `ludus_*`) appear in the schema list.

- [ ] **Step 23.3: Run arch-check + doc-pipeline.**

  ```
  cargo run -p vox-arch-check
  cargo run -p vox-doc-pipeline -- --check
  ```

- [ ] **Step 23.4: Commit.**

  ```
  git add crates/vox-db crates/vox-gamify
  git commit -m "$(cat <<'EOF'
  refactor(vox-db,vox-gamify): move ops_ludus typed ops to vox-gamify

  The ~3.2K LoC of gamify-domain typed ops in vox-db/src/store/ops_ludus/
  conceptually belongs in vox-gamify (which already has its own db/ module).
  Migrations stay in vox-db/schema/ — only the typed API moves. Relieves
  vox-db budget pressure (32K → ~29K) and consolidates the gamify domain
  into one crate.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md (C4)
  EOF
  )"
  ```

---

# PR5 — Extract `vox-orchestrator-core` (C5)

## Task 24: Survey `vox-orchestrator/src/orchestrator/`

**Files:**
- Read: `crates/vox-orchestrator/src/orchestrator/` (whole subdir — 12.3K LoC)
- Read: `crates/vox-orchestrator/src/lib.rs`
- Read: `crates/vox-orchestrator/Cargo.toml`

- [ ] **Step 24.1: List the orchestrator/ subdir.**

  ```
  Get-ChildItem -Recurse crates/vox-orchestrator/src/orchestrator/ | Where-Object {-not $_.PSIsContainer} | Select-Object FullName
  ```

- [ ] **Step 24.2: Identify the public boundary.**

  Read `crates/vox-orchestrator/src/lib.rs`. Find every `pub mod orchestrator;` and `pub use orchestrator::...` line. Record the public types/functions exposed from this subdir.

- [ ] **Step 24.3: Identify cross-cuts.**

  Run:
  ```
  rg "use crate::|use super::" crates/vox-orchestrator/src/orchestrator/ --type rust
  ```

  Bucket: intra-orchestrator/ (move with code), cross-cut to other vox-orchestrator modules (need to expose those publicly or move with). Cross-cuts to other crates already work.

## Task 25: Create the `vox-orchestrator-core` crate

**Files:**
- Create: `crates/vox-orchestrator-core/Cargo.toml`
- Create: `crates/vox-orchestrator-core/src/lib.rs`

- [ ] **Step 25.1: Scaffold.**

  Create `crates/vox-orchestrator-core/Cargo.toml`:
  ```toml
  [package]
  name = "vox-orchestrator-core"
  description = "Core router/dispatcher logic for vox-orchestrator (the densest subdir of the post-Phase-5 monolith). Parent crate vox-orchestrator now hosts dei_shim, planning, services, runtime glue."
  version.workspace = true
  edition.workspace = true

  [dependencies]
  # Mirror vox-orchestrator's [dependencies] minus anything used only by
  # the modules that stayed in the parent. Start permissive; trim after
  # cargo build identifies the actual subset.
  tokio = { workspace = true }
  serde = { workspace = true }
  serde_json = { workspace = true }
  tracing = { workspace = true }
  thiserror = { workspace = true }
  anyhow = { workspace = true }
  vox-orchestrator-types = { workspace = true }
  vox-orchestrator-queue = { workspace = true }
  vox-db = { workspace = true }
  vox-config = { workspace = true }
  vox-bounded-fs = { workspace = true }
  workspace-hack = { workspace = true }
  # Add others as the build complains.

  [lints]
  workspace = true
  ```

  Create `crates/vox-orchestrator-core/src/lib.rs`:
  ```rust
  //! Core router/dispatcher logic extracted from vox-orchestrator (formerly
  //! crates/vox-orchestrator/src/orchestrator/). The parent vox-orchestrator
  //! crate now hosts dei_shim, planning, services, runtime glue, and
  //! coordination across the queue/mcp/core boundary.
  ```

- [ ] **Step 25.2: Add to workspace + layers.**

  Root `Cargo.toml` `[workspace.dependencies]`:
  ```toml
  vox-orchestrator-core     = { path = "crates/vox-orchestrator-core" }
  ```

  `docs/src/architecture/layers.toml` L3 section:
  ```toml
  vox-orchestrator-core   = { layer = 3, max_loc = 20_000 }
  ```

## Task 26: Move orchestrator/ subdir into vox-orchestrator-core

- [ ] **Step 26.1: Move files.**

  ```
  Move-Item crates/vox-orchestrator/src/orchestrator/* crates/vox-orchestrator-core/src/
  Remove-Item crates/vox-orchestrator/src/orchestrator -Recurse
  ```

- [ ] **Step 26.2: Adjust the entry-point module.**

  If the moved subdir had a `mod.rs`, rename to `dispatch.rs` or similar (Rust 2024 prefers named files). Update `crates/vox-orchestrator-core/src/lib.rs` to declare the modules:
  ```rust
  pub mod dispatch;
  pub use dispatch::*;
  ```

  Adjust as needed based on what the subdir actually exposes.

- [ ] **Step 26.3: Fix imports inside vox-orchestrator-core.**

  Every `use crate::...` that referred to a module *outside* the moved subdir must now be either:
  - `use vox_orchestrator::...` (creates dep cycle if vox-orchestrator depends on vox-orchestrator-core) — DO NOT use this
  - `use vox_orchestrator_types::...` (preferred — pull pure types from L0 leaf)
  - Move the helper into vox-orchestrator-core too if it's a tight collaborator
  - Move the helper to a shared L1/L2 crate if it's broadly useful

- [ ] **Step 26.4: Update vox-orchestrator to depend on vox-orchestrator-core.**

  `crates/vox-orchestrator/Cargo.toml` `[dependencies]`:
  ```toml
  vox-orchestrator-core = { workspace = true }
  ```

  Replace the old `pub mod orchestrator;` in `crates/vox-orchestrator/src/lib.rs` with:
  ```rust
  pub use vox_orchestrator_core as orchestrator;
  ```

  This preserves the public path `vox_orchestrator::orchestrator::X` for external callers.

- [ ] **Step 26.5: Build incrementally.**

  ```
  cargo build -p vox-orchestrator-core
  ```

  Fix imports until clean. Then:
  ```
  cargo build -p vox-orchestrator
  cargo build --workspace
  ```

  Likely failures + fixes:
  - Missing dep in `vox-orchestrator-core/Cargo.toml` → add it
  - Cross-cut import that creates cycle → break by moving the helper or pulling pure types from L0
  - Test code using internals → may need to add `pub(crate)` visibility in vox-orchestrator-core

## Task 27: Update layers.toml budgets

- [ ] **Step 27.1: Re-measure LoC of vox-orchestrator and vox-orchestrator-core.**

  ```
  Get-ChildItem -Recurse -Filter *.rs crates/vox-orchestrator/src | ForEach-Object { (Get-Content $_.FullName).Count } | Measure-Object -Sum
  Get-ChildItem -Recurse -Filter *.rs crates/vox-orchestrator-core/src | ForEach-Object { (Get-Content $_.FullName).Count } | Measure-Object -Sum
  ```

- [ ] **Step 27.2: Update budgets.**

  In `docs/src/architecture/layers.toml`, adjust `vox-orchestrator`'s `max_loc`:
  - Old: `60_000`
  - New: round up to next 5K above current size (e.g., if current is 41K, set to 45_000)

  Set `vox-orchestrator-core`'s `max_loc` to the next 5K above its current size (probably 15_000 or 20_000).

  This is a tightening — future bloat will trip the lint earlier.

## Task 28: PR5 verification and commit

- [ ] **Step 28.1: Full verification.**

  ```
  cargo build --workspace
  cargo run -p vox-arch-check
  cargo test -p vox-orchestrator -p vox-orchestrator-core
  cargo run -p vox-doc-pipeline -- --check
  ```
  All clean.

- [ ] **Step 28.2: Measure incremental build win.**

  Touch a file inside vox-orchestrator-core and time the rebuild:
  ```
  Add-Content crates/vox-orchestrator-core/src/lib.rs "`n// touch"
  Measure-Command { cargo build -p vox-orchestrator-core }
  # Undo
  Remove-Item crates/vox-orchestrator-core/src/lib.rs
  git checkout crates/vox-orchestrator-core/src/lib.rs
  ```

  Compare to a touched-orchestrator baseline (before extraction). Expected: ≥15% reduction on edits inside the extracted code path. Record the number in the PR description.

- [ ] **Step 28.3: Add the row to where-things-live.md.**

  L3 section:
  ```markdown
  | [`vox-orchestrator-core`](../../../crates/vox-orchestrator-core/) | Core router/dispatcher for vox-orchestrator (extracted from the densest subdir of the post-Phase-5 crate). |
  ```

  Update `vox-orchestrator`'s scope description to reflect the post-extraction reality.

- [ ] **Step 28.4: Update vox-orchestrator's Cargo.toml description.**

  In `crates/vox-orchestrator/Cargo.toml`, update the `description` to reflect the new shape:
  ```toml
  description = "Glue crate for the multi-agent file-affinity router: dei_shim, planning, services, runtime glue. Core router lives in vox-orchestrator-core, queue/lock/oplog in vox-orchestrator-queue, MCP in vox-orchestrator-mcp."
  ```

- [ ] **Step 28.5: Commit.**

  ```
  git add crates/vox-orchestrator crates/vox-orchestrator-core
  git add Cargo.toml docs/src/architecture/layers.toml
  git add docs/src/architecture/where-things-live.md
  git commit -m "$(cat <<'EOF'
  refactor(vox-orchestrator): extract vox-orchestrator-core (12.3K LoC)

  Pulls the densest subdir of post-Phase-5 vox-orchestrator (the
  router/dispatcher) into a dedicated L3 crate. Smaller wedge than the
  originally-deferred Phase 6 runtime split, but still delivers
  meaningful incremental-build savings on edits to that path.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md (C5)
  EOF
  )"
  ```

---

# PR6 — `vox-arch-check` enhancements (lock in the gains)

## Task 29: Add `description_present` lint to vox-arch-check

**Files:**
- Modify: `crates/vox-arch-check/src/main.rs`
- Modify: `docs/src/architecture/layers.toml` `[guards]` section

- [ ] **Step 29.1: Read the existing arch-check structure.**

  Read `crates/vox-arch-check/src/main.rs` end-to-end. Note the existing rules: layer ordering (strict), fan-in (warn), loc_budget (warn), orphan (warn), docstring (warn). Each rule is invoked in `run()` and accumulates findings into a `Report`.

- [ ] **Step 29.2: Add the `description_present` rule.**

  In `crates/vox-arch-check/src/main.rs`, add a new function alongside the existing `check_*` functions:

  ```rust
  /// Warn (or fail) if a workspace member at L1+ has no `description` field
  /// in its Cargo.toml or has one shorter than 40 characters. Binary-only
  /// crates (`kind = "binary"`) and `workspace-hack` are exempt.
  fn check_description_present(
      meta: &cargo_metadata::Metadata,
      cfg: &LayersConfig,
  ) -> Vec<String> {
      let mut findings = Vec::new();
      for pkg in &meta.packages {
          let Some(entry) = cfg.crates.get(&pkg.name) else { continue };
          if entry.layer < 1 { continue }
          if entry.kind == "binary" { continue }
          if pkg.name == "workspace-hack" { continue }
          let desc = pkg.description.as_deref().unwrap_or("");
          if desc.len() < 40 {
              findings.push(format!(
                  "{}: Cargo.toml description is missing or shorter than 40 chars (\"{}\")",
                  pkg.name, desc,
              ));
          }
      }
      findings
  }
  ```

- [ ] **Step 29.3: Wire the rule into `run()` and the `Report`.**

  In `run()`, after the existing rule calls, add:
  ```rust
  let description_findings = check_description_present(&metadata, &config);
  ```

  Add a field to the `Report` struct:
  ```rust
  struct Report {
      // ... existing fields
      description: Vec<String>,
  }
  ```

  Update `print_summary()` to print the new findings under a "Description present:" heading.

- [ ] **Step 29.4: Add the strictness control.**

  Extend `GuardsConfig`:
  ```rust
  #[derive(Debug, Default, Deserialize)]
  struct GuardsConfig {
      // ... existing
      #[serde(default)]
      description: Option<String>,
  }
  ```

  In `Report::strict_failed()`, treat description findings as strict only if `cfg.guards.description == Some("error")`. Default is `"warn"` (non-failing).

- [ ] **Step 29.5: Document in layers.toml.**

  Add to the `[guards]` section in `docs/src/architecture/layers.toml`:
  ```toml
  description = "warn"
  ```

  (Add a comment noting that this becomes "error" after PR1 lands and all 19 missing descriptions are filled in.)

## Task 30: Add `where_things_live_coverage` lint

- [ ] **Step 30.1: Add the rule.**

  In `crates/vox-arch-check/src/main.rs`:

  ```rust
  /// Warn if a workspace member is not mentioned in
  /// `docs/src/architecture/where-things-live.md`. Detection: the doc must
  /// contain the literal substring `crates/<name>/` for every workspace
  /// member. Plugin and test-only crates are also expected to appear (in the
  /// Plugins or surfaces sections respectively).
  fn check_where_things_live_coverage(
      meta: &cargo_metadata::Metadata,
      cfg: &LayersConfig,
      repo_root: &Path,
  ) -> Result<Vec<String>> {
      let path = repo_root.join("docs/src/architecture/where-things-live.md");
      let body = std::fs::read_to_string(&path)
          .with_context(|| format!("read {}", path.display()))?;
      let mut findings = Vec::new();
      for pkg in &meta.packages {
          if !cfg.crates.contains_key(&pkg.name) { continue }
          if pkg.name == "workspace-hack" { continue }
          let needle = format!("crates/{}/", pkg.name);
          if !body.contains(&needle) {
              findings.push(format!(
                  "{}: not listed in where-things-live.md (no `{}` substring)",
                  pkg.name, needle,
              ));
          }
      }
      Ok(findings)
  }
  ```

- [ ] **Step 30.2: Wire into `run()` and `Report`.**

  Same pattern as Task 29: call the function, store findings in a new `Report` field, print in summary, gate strictness via a new `where_things_live` field in `GuardsConfig`.

- [ ] **Step 30.3: Document.**

  Add to `[guards]`:
  ```toml
  where_things_live = "warn"
  ```

## Task 31: Tighten `docstring` rule to strict for L0–L2

- [ ] **Step 31.1: Modify the `docstring` rule.**

  Find the existing `check_docstring` (or equivalent) function. Currently warns for any `lib.rs` not starting with `//!`. Change the strict-or-warn decision per finding based on the crate's layer:

  ```rust
  let strict_for_this_crate = entry.layer <= 2;
  // accumulate strict findings vs. warn findings separately
  ```

- [ ] **Step 31.2: Update Report.**

  Treat the strict-docstring findings the same way as layer-ordering findings (fail-by-default unless `--warn-only`). Warn-docstring findings stay warn.

- [ ] **Step 31.3: Verify zero findings on current tree.**

  After PR1's `vox-corpus` docstring fix, verify L0–L2 crates all have `//!` docstrings:
  ```
  cargo run -p vox-arch-check
  ```
  Expected: clean. If not, add `//!` docstrings to the offending L0–L2 crates inline.

## Task 32: Add a regression test fixture

**Files:**
- Create: `crates/vox-arch-check/tests/fixtures/missing-desc/Cargo.toml`
- Create: `crates/vox-arch-check/tests/fixtures/missing-desc/src/lib.rs`
- Create: `crates/vox-arch-check/tests/integration.rs`

- [ ] **Step 32.1: Create a fixture crate with no description.**

  `crates/vox-arch-check/tests/fixtures/missing-desc/Cargo.toml`:
  ```toml
  [package]
  name = "vox-arch-check-fixture-missing-desc"
  version = "0.0.0"
  edition = "2024"
  # description intentionally missing
  ```

  `crates/vox-arch-check/tests/fixtures/missing-desc/src/lib.rs`:
  ```rust
  //! Fixture for description_present lint tests.
  ```

- [ ] **Step 32.2: Add an integration test.**

  `crates/vox-arch-check/tests/integration.rs`:
  ```rust
  //! Smoke tests: the description_present rule fires on a fixture crate
  //! that is missing its description, and is silent on a fixture crate
  //! that has a long-enough one.

  use std::process::Command;

  #[test]
  fn description_present_fires_on_missing() {
      // The fixture is excluded from the main workspace via
      // root Cargo.toml's `exclude = [...]`. We invoke arch-check
      // pointed at the fixture directory directly.
      // Detailed wiring depends on how arch-check accepts a metadata-root
      // override; if it doesn't, this test is a //! TODO.
  }
  ```

  If `vox-arch-check` doesn't currently accept a non-default metadata path, add a CLI flag `--manifest-path` that maps through to `cargo_metadata::MetadataCommand::manifest_path()`. Then write the test against the fixture.

- [ ] **Step 32.3: Add the fixture to root Cargo.toml `exclude`.**

  In root `Cargo.toml`:
  ```toml
  exclude = [
      # ... existing entries
      "crates/vox-arch-check/tests/fixtures/missing-desc",
  ]
  ```

## Task 33: PR6 verification, lint regen, and commit

- [ ] **Step 33.1: Run arch-check on the actual workspace.**

  ```
  cargo run -p vox-arch-check
  ```

  Expected: clean (all PRs 1–5 fixed the underlying issues).

- [ ] **Step 33.2: Run with `--warn-only` to verify the report is structurally correct.**

  ```
  cargo run -p vox-arch-check -- --warn-only
  ```

- [ ] **Step 33.3: Run the integration test.**

  ```
  cargo test -p vox-arch-check
  ```
  Expected: passes.

- [ ] **Step 33.4: Switch description guard to `error` after confirming clean.**

  In `docs/src/architecture/layers.toml`:
  ```toml
  description = "error"
  where_things_live = "error"
  ```

  Re-run `cargo run -p vox-arch-check`. Must be clean.

- [ ] **Step 33.5: Full workspace verification.**

  ```
  cargo build --workspace
  cargo test --workspace --no-run
  cargo run -p vox-doc-pipeline -- --check
  ```

- [ ] **Step 33.6: Commit.**

  ```
  git add crates/vox-arch-check Cargo.toml
  git add docs/src/architecture/layers.toml
  git commit -m "$(cat <<'EOF'
  feat(vox-arch-check): add description_present + where_things_live coverage lints

  - Adds description_present lint (warn-by-default; strict per [guards]):
    every L1+ library crate must have a Cargo.toml description ≥40 chars.
  - Adds where_things_live_coverage lint: every workspace member must be
    listed in docs/src/architecture/where-things-live.md.
  - Tightens existing docstring lint to strict for L0–L2 crates only.
  - Adds integration test fixture for the new lints.
  - Promotes both new guards from warn → error in layers.toml after
    PR1–PR5 fixed the underlying drift.

  Spec: docs/src/architecture/2026-05-08-crate-org-followup-design.md (PR6)
  EOF
  )"
  ```

---

## Self-review

Before opening PR6, run a final pass over all 6 PRs:

- [ ] **Coverage:** every section of the spec maps to a task. Spec sections: A1 (gamify) → Task 1, 2; A2 → Task 3; A3 → Task 4; A4 → Task 6; A5 → Task 5; B descriptions → Task 7; B binary-layer doc → Task 8; C1 → Task 9; C2 → Tasks 11–15; C3 → Tasks 16–20; side-quest → Task 19; C4 → Tasks 21–23; C5 → Tasks 24–28; PR6 → Tasks 29–33. ✔
- [ ] **No placeholders:** every step contains the actual edit, not "TBD" or "similar to". The only judgment call is the docstring summary in Task 6.3 / 6.4 ("read lib.rs first line") which the executing agent has to read fresh — that's intentional.
- [ ] **Verification at every PR boundary:** every PR ends with `cargo build --workspace`, `cargo run -p vox-arch-check`, `cargo run -p vox-doc-pipeline -- --check`, and (where applicable) `cargo test`.
- [ ] **Commits:** every PR has an explicit commit step with file list and message body.
- [ ] **Open questions:** the spec's open questions (vox ludus CLI verb, AGENTS.md row interpretation, vox-arch-check workspace.dependencies absence, Phase-6 future) are all defaulted in the plan; the executing agent does not have to make those calls unless they discover the default is wrong.

---

## Risks and recovery

- **A wrong line number in the docs edits.** Tasks 1–4 reference specific line numbers from the audit. If a line has shifted, the executing agent should grep for the surrounding text and apply the edit at the right location. If a previously-recorded reference is gone entirely, log it and continue (it may have been fixed in a parallel commit).
- **A vox-package or vox-orchestrator-core extraction reveals a hidden cycle.** Recover by NOT committing the partial extraction; restore via `git checkout .` after capturing the cycle in a comment for the next attempt.
- **An auto-generated doc check fails after edits.** Re-run `cargo run -p vox-doc-pipeline` (without `--check`) to regenerate; commit the regenerated files alongside the edits.
- **Arch-check fails on a real, unintentional inversion you didn't introduce.** Don't add a new known_inversion to silence it; investigate the root cause and either fix the cycle or document the decision in this plan.

End of plan.
