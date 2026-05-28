---
title: "Crate Organization Follow-up — SSOT, Naming, and Sprawl"
description: "Follow-up to the 2026-05-08 workspace reorg: closes SSOT/discoverability drift, picks up deferred Phase 3/6/7 extractions, and adds enforcement so the gains stick."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; transient artifact."
---

# Crate Organization Follow-up — SSOT, Naming, and Sprawl

Companion to:
- [2026-05-08-workspace-reorg-design.md](./2026-05-08-workspace-reorg-design.md) — original 10-phase plan
- [2026-05-08-workspace-reorg-outcome.md](./2026-05-08-workspace-reorg-outcome.md) — what landed (Phases 0/1/2/4/5/9)
- [2026-05-08-naming-and-guards-design.md](./2026-05-08-naming-and-guards-design.md) — naming-and-anti-entanglement-guards series

This document specifies the next round. It is the source-of-truth spec for the implementation plan that follows.

> **Status (2026-05-15):** All six PRs landed. Track A (SSOT + A4 WTL coverage + A5 orphan → error), Track B (19 description rewrites + binary-layer note), C1 (vox-mcp-meta merge), C2 (vox-package-types split), C4 (ops_ludus → vox-gamify), C5 side-quest (mcp-server feature gate), PR6 (arch-check lints: `description_present`, `where_things_live_coverage`, docstring L0–L2 strict) — all clean. C3 (vox-cli-ci) and C5/Tier D (vox-orchestrator-core) are deferred with dedicated plan docs: [`2026-05-15-cli-ci-extraction-plan.md`](2026-05-15-cli-ci-extraction-plan.md) and [`2026-05-15-orchestrator-tier-d-plan.md`](2026-05-15-orchestrator-tier-d-plan.md).

## Problem statement

The 2026-05-08 reorg landed substantial wins (orchestrator −36%, CLI −74% on incremental builds, 79 crates layer-checked) but stopped short on three axes:

1. **SSOT drift between authoritative docs and live crates.** A crate that was renamed (`vox-ludus` → `vox-gamify`) still appears in 4 live policy surfaces as the *canonical* form. Documents reference `vox-pm`, `vox-mens`, `vox-dei` though those crates no longer exist or have been renamed. `where-things-live.md` covers ~30 of 83 crates.
2. **Naming inconsistency.** Sibling concepts use different suffixes (`vox-skills` vs. `vox-skill-runtime`). Crate `description =` fields are missing or stale on 19 of the heavy crates. Two crates (`vox-mcp-meta` ↔ `vox-mcp-registry`) are functional duplicates.
3. **Deferred extractions still on the table.** Phases 3/6/7/8 of the original reorg were deferred with documented cost-benefit. The audit (below) finds five higher-value moves than the originally-deferred shapes, ordered by build-time-impact ÷ disruption-cost.

The combined effect is that **future LLM tool calls cannot intuitively reason about the structure**: a name lookup hits stale aliases, a "where does this live?" question hits a half-populated lookup table, and a build-time question still hits the 52K-LoC `vox-orchestrator` post-Phase-5.

## Goals

1. Every live policy surface (AGENTS.md, CLAUDE.md, `.cursor/rules/`, `.github/`, `contracts/proximity/`, `docs/src/{architecture,reference,contributors}/`) names crates by the canonical form they have in `crates/<name>/Cargo.toml`.
2. `where-things-live.md` covers all 83 workspace members in one flat lookup table.
3. Every `crates/vox-*/Cargo.toml` has a one-sentence `description =` field that matches its actual scope.
4. Five extractions and one merge land, ranked by impact ÷ disruption.
5. `vox-arch-check` gains two new lints (`description_present`, `where_things_live_coverage`) to prevent regression.

## Non-goals

- Renaming `vox-eval`, `vox-corpus`, or `vox-tensor`. Their domain is non-obvious from the name, but a thorough one-line `where-things-live.md` row is cheaper than 14+ caller updates.
- Renaming `vox-skills` to `vox-skill-registry`. The audit found this would touch ~14 callers; the marginal clarity is not worth the diff. Document the boundary instead (skills = marketplace/lifecycle; skill-runtime = sandbox dispatch).
- Tackling original Phase 6 (`vox-orchestrator-runtime`) in its full form. The audit found a smaller wedge (`vox-orchestrator-core`, 12.3K LoC) that delivers most of the build-time benefit without the 40-method trait facade.
- Touching `crates/_frozen.md` or `docs/src/archive/` references — those are tombstoned per AGENTS.md §Archival Protocol.

## Audit findings (consolidated)

### Track C — Build-time / sprawl (ranked by impact ÷ disruption)

Numbers are post-Phase 5. Source: parallel audit of heavy-crate LoC, top-level module shape, and Cargo.toml internal-dep counts.

| # | Move | LoC | Why | Touches |
|---|---|---|---|---|
| **C1** | Merge `vox-mcp-meta` (62 LoC of re-exports + 4 `&[&str]` constants) → `vox-mcp-registry` | 62 | Pure friction; one less node in every consumer's graph | low |
| **C2** | Split `vox-package` → `vox-package-types` (L1) + `vox-package-build` (L3) | 2.4K | Removes both documented layer inversions (`vox-package` → `vox-compiler`, `vox-package` → `vox-db`) | low — small crate, ~2 callers |
| **C3** | Extract `vox-cli/src/commands/ci/` → `vox-cli-ci` | 17.3K | 27% of vox-cli; tightest internal cohesion of any subdir; CI commands rebuild the entire 63K binary surface today | medium — vox-cli + feature gating |
| **C4** | Move `vox-db/src/store/ops_ludus/` → `vox-gamify` | 3.2K | Consolidates the gamify domain (gamify already has its own `db/`); relieves vox-db budget pressure (32K → 29K under 40K cap) | medium — db ↔ gamify boundary |
| **C5** | Extract `vox-orchestrator/src/orchestrator/` → `vox-orchestrator-core` | 12.3K | Densest remaining node post-Phase-5 (24% of the crate); smaller wedge than the deferred Phase-6 runtime split | high — orchestrator core |

**Side-quest (no LoC moved):** `vox-cli` pulls `vox-orchestrator-mcp` unconditionally; the `mcp-server` feature only gates axum/rmcp. Move the `vox-orchestrator-mcp` dep behind that feature. Estimated 1–3s incremental win on the common (no-mcp) CLI build.

**Cross-cutting:** `vox-orchestrator` and `vox-orchestrator-mcp` are at 88% and 85% of their `max_loc` budgets respectively. C5 is also pressure relief, not just a perf move.

### Track B — Naming changes (taken)

| Action | Before | After | Reason |
|---|---|---|---|
| Merge | `vox-mcp-meta` + `vox-mcp-registry` | `vox-mcp-registry` | meta is 62 LoC of re-exports (also C1) |
| Rewrite `description =` | (missing) on 19 crates | one-sentence canonical description per crate (table below) | Cargo.toml is the first thing `cargo metadata` and rustdoc surface |
| Rewrite `description =` | "Multi-agent file-affinity queue system…" on `vox-orchestrator` | reflects post-Phase-5 reality (queue extracted, MCP extracted) | live drift |
| Rewrite `lib.rs` `//!` docstring | narrow on `vox-corpus` | matches the actual module set (training + mcp_meta + synthetic_search_gen + tool_workflow_corpus) | live drift |
| Document | binary-layer rule (binaries at L0/L3 are tools; only product-shipped binaries are L5) | one-paragraph note in `layers.toml` header | future readers will hit this question |

**Description rewrites** (track-B mechanical work — full list, suggested text):

| Crate | Suggested description |
|---|---|
| `vox-orchestrator` | Slim coordinator for the multi-agent file-affinity router; queue/lock/oplog live in vox-orchestrator-queue, MCP tools in vox-orchestrator-mcp. |
| `vox-orchestrator-types` | Pure-data L0 leaf for vox-orchestrator: agent/task IDs, file affinity, switch actions, provider catalogs. |
| `vox-package` | Vox package manager: Vox.toml manifests, vox.lock, content-addressed artifact cache, registry client, dependency resolver. |
| `vox-actor-runtime` | Process-oriented runtime: actors, mailboxes, supervision, scheduling, LLM/Mens activity primitives. |
| `vox-cli-core` | Shared internals for the vox CLI binary (argv parsing helpers, exit-code policy). |
| `vox-crypto` | Pure-Rust crypto primitives (chacha20poly1305 AEAD, ed25519, x25519); sole crypto SSOT per AGENTS.md §Cryptography Policy. |
| `vox-db` | Codex / VoxDb facade: schema migrations, store ops, Turso/libSQL access for the Vox workspace. |
| `vox-db-types` | Pure-data L0 leaf for vox-db: row types, IDs, schema descriptors. |
| `vox-doc-pipeline` | Doc generator: regenerates SUMMARY.md, architecture-index.md, feed.xml from frontmatter. |
| `vox-eval` | Vox expression evaluator (interpreter for `vox run --interp`). |
| `vox-grammar-export` | Exports the Vox grammar artifact for downstream tooling. |
| `vox-identity` | Identity primitives: signing keys, trust ledger entries. |
| `vox-integration-tests` | Cross-crate integration test harness (test-only L5). |
| `vox-lsp` | Vox Language Server (stdio JSON-RPC). |
| `vox-protocol` | Daemon wire-protocol pure-data types. |
| `vox-scientia-ingest` | Scientia corpus ingestion. |
| `vox-ssg` | Static site generator for the Vox docs surface. |
| `vox-tensor` | Pure-CPU JSONL data loaders / training-pair types (Burn extracted 2026-05-08). |
| `vox-test-harness` | Shared test fixtures and harness primitives. |

The implementing agent must verify each suggested description against the crate's actual `lib.rs` first 30 lines before applying — if scope has drifted further, prefer accuracy to this table.

### Track A — SSOT / discoverability (every drift to fix)

#### A1. Inverted `vox-gamify` ↔ `vox-ludus` directives

**Decision:** `vox-gamify` is canonical. `vox-ludus` is retired. Every live doc claiming the reverse is drift. Fix forward.

| Path:Line | Current | Replace with |
|---|---|---|
| `AGENTS.md:195` | row `\| vox-gamify \| vox-ludus \|` in §Retired Surfaces table | row `\| vox-ludus \| vox-gamify \|` |
| `.cursor/rules/retired-surfaces.mdc:13` | row `\| vox-gamify \| vox-ludus \|` | row `\| vox-ludus \| vox-gamify \|` |
| `.github/copilot-instructions.md:18` | `Use vox-ludus, NOT vox-gamify.` | `Use vox-gamify, NOT vox-ludus.` |
| `.github/PULL_REQUEST_TEMPLATE.md:5` | header `## Ludus / gamify (if applicable)` | `## Gamify (if applicable)` |
| `.github/PULL_REQUEST_TEMPLATE.md:7` | refs `agent-event-kind-ludus-matrix.md` and Ludus event terminology | rephrase around `vox-gamify` |
| `.github/PULL_REQUEST_TEMPLATE.md:10` | "Ludus section if new `VOX_LUDUS_*`" | "Gamify section if new `VOX_LUDUS_*`" (env-var prefix unchanged) |
| `.github/PULL_REQUEST_TEMPLATE.md:14` | `cargo test -p vox-ludus` | `cargo test -p vox-gamify` |
| `contracts/proximity/retired-surfaces.v1.json:14-17` | `"retired_symbol": "vox-gamify", "canonical_replacement": "vox-ludus"` | `"retired_symbol": "vox-ludus", "canonical_replacement": "vox-gamify"` |
| `crates/vox-gamify/README.md:1` | `# vox-ludus` | `# vox-gamify` |
| `crates/vox-gamify/README.md` (body) | every other `vox-ludus` reference | `vox-gamify`. CLI verb `vox ludus` is a separate question — leave verb alone unless explicitly verified retired. |
| `docs/src/contributors/toestub-contributor-guide.md:202` | row `\| vox-gamify \| vox-ludus \|` | row `\| vox-ludus \| vox-gamify \|` |
| `docs/src/reference/agent-quick-reference.md:40` | row `\| vox-gamify \| vox-ludus \|` | row `\| vox-ludus \| vox-gamify \|` |
| `docs/agents/database-nomenclature.md:36,112,113` | `vox-ludus` references | `vox-gamify` (and update `crates/vox-ludus/...` paths to `crates/vox-gamify/...`) |
| `docs/src/reference/clavis-ssot.md:26` | `vox-ludus` | `vox-gamify` |
| `docs/src/reference/env-vars.md:62,66,69,73` | `crates/vox-ludus/src/...` paths | `crates/vox-gamify/src/...` |
| `docs/src/reference/cli.md:525` | `vox-ludus` | `vox-gamify` |
| `docs/src/reference/hitl-and-doubt.md:39` | `vox-ludus` | `vox-gamify` |
| `docs/src/reference/mens-serving-ssot.md:19` | `vox-ludus` | `vox-gamify` |

**Generated artifacts that will resolve themselves on next regeneration** (do NOT hand-edit per [feedback_auto_generated_docs](../../../README.md)):
- `docs/agents/doc-inventory.json`, `doc-inventory-index.json` — regenerate
- `contracts/reports/*.json` — regenerated by their respective audits
- `docs/src/archive/...` — tombstoned, do NOT touch

#### A2. Other retired-form directives

| Path:Line | Current | Replace with |
|---|---|---|
| `docs/src/contributors/coding-agents.md:20` | "`vox-dei` is now a small HITL crate, not the orchestrator" | "`vox-dei` was retired; orchestrator is `vox-orchestrator`." |
| `docs/src/reference/ref-decorators.md:17-30` | prescribes `@server`, `@query`, `@mutation` as separate decorators | rewrite as `@endpoint(kind: server\|query\|mutation)` per AGENTS.md:198 |
| `docs/src/reference/ref-syntax.md:174,178` | code uses `@query fn …` / `@mutation fn …` | rewrite as `@endpoint(kind: query) fn …` / `@endpoint(kind: mutation) fn …` |
| `docs/src/reference/ref-type-system.md:87` | `@server fn update_task(…) -> Result[…]` | `@endpoint(kind: server) fn update_task(…) to Result[…]` |
| `docs/src/reference/vox-db-language-surface.md:19-21` | three rows recommend `@query fn`, `@mutation fn`, `@server fn` | unify behind `@endpoint(kind: …)` |
| `docs/src/reference/vox-fullstack-artifacts.md:20,33` | "Browser client for `@server fn`" | "Browser client for `@endpoint(kind: server)`" |
| `docs/src/reference/orchestration-unified.md:25` | "**DeI planning on the daemon:** … `vox-dei-d`" | drop `vox-dei-d`; daemon is `vox-orchestrator-d` |
| `docs/src/reference/hitl-and-doubt.md:32` | "`ResolutionAgent` (from the `vox-dei` crate)" | point to actual home (`vox-orchestrator::dei_shim`) — verify |
| `docs/src/.well-known/llms-full.txt:38` | "`vox-ars` → replaced by `vox-skills`" | "`vox-ars` → `vox-ars-runtime` (now `vox-openclaw-runtime`)" — match AGENTS.md:194 |

#### A3. Crate-name drift

| Path:Line | Found | Replace with |
|---|---|---|
| `docs/agents/cli-toolchain.md:18,20,22,24,46,48` | `vox-pm/`, `vox-toestub/`, `vox train --native`, `vox mens corpus` references; conflated `vox-compiler` (codegen + SSG) | split rows: `vox-package`, `vox-code-audit`, `vox-codegen`, `vox-ssg`. Drop the `vox train --native` row (Burn removed). Verify the `vox mens corpus` subcommand against `vox-ml-cli`. |
| `docs/agents/database-nomenclature.md:21` | row `\| vox-pm \| crates/vox-pm \|` | `vox-package` / `crates/vox-package` |
| `docs/agents/orchestrator.md:60` | "`vox-populi` / **`vox-mens`** shim" | "`vox-populi` / `vox-ml-cli` shim" |
| `docs/agents/script-registry.json:58` | `"replacement": "vox ci no-vox-dei-import"` | confirm CI subcommand exists; if renamed, update |
| `docs/src/reference/env-vars.md:15,34` | "Codex / Turso (vox-db, vox-pm)" header; "vox-pm exposes" | `vox-package` |
| `docs/src/reference/cli.md:24,33,48,85,223,566-570` | `vox-mens` (binary), `vox-pm` references | binary is `vox-ml-cli`; package crate is `vox-package`. CLI verb `vox mens` may stay if intentional. |
| `docs/src/reference/changelog.md:46` | `vox-pm/README.md` | `vox-package/README.md` |
| `docs/src/reference/mens-serving-ssot.md:19` | refers to `vox-runtime` (not in `crates/`) | `vox-actor-runtime` / `vox-workflow-runtime` |
| `docs/src/reference/vox-portability-ssot.md:40,115` | `vox-pm` | `vox-package` |
| `README.md:71` | `vox-mens` plugin row | rename binary to `vox-ml-cli`; if external plugin name `vox-mens` is intentional, document the binary↔plugin mapping |
| `crates/vox-db/README.md:3` | `Wraps vox-pm::CodeStore` | `Wraps vox-package::CodeStore` |
| `Cargo.toml:135` | `known_inversions` reason mentions "`vox dei`" CLI subcommand | factual at runtime; verify `vox dei` exists or rename in the comment |

ADR files (`docs/src/adr/004-…`, `docs/src/adr/015-…`) reference `vox-pm` historically — leave the ADR text alone but add a one-line "(now `vox-package`)" footnote since their `status: current` frontmatter implies they should still resolve.

#### A4. `where-things-live.md` coverage gap

`where-things-live.md` currently lists ~30 of 83 crates. Expand it to cover all 83 in one flat table grouped by layer. Missing crates (40 total):

**Subsystem section gaps (32):** `vox-capability-registry`, `vox-checksum-manifest`, `vox-cli-core`, `vox-constrained-gen`, `vox-container`, `vox-corpus`, `vox-deploy-codegen`, `vox-doc-inventory`, `vox-eval`, `vox-exec-grammar`, `vox-forge`, `vox-git`, `vox-grammar-export`, `vox-install-policy`, `vox-integration-tests`, `vox-jsonschema-util`, `vox-mcp-meta`, `vox-mcp-registry`, `vox-openai-sse`, `vox-openai-wire`, `vox-primitives`, `vox-project-scaffold`, `vox-publisher`, `vox-repository`, `vox-reqwest-defaults`, `vox-scaling-policy`, `vox-scientia-ingest`, `vox-search`, `vox-tensor`, `vox-test-harness`, `vox-wasm-engine`, `vox-webhook`.

(Note: after C1 lands, `vox-mcp-meta` is removed from the list — handled in PR1 sequencing.)

**Plugins section gaps (9):** `vox-plugin-browser`, `vox-plugin-catalog`, `vox-plugin-cloud`, `vox-plugin-grammar-export`, `vox-plugin-nvml-probe`, `vox-plugin-populi-mesh`, `vox-plugin-publication`, `vox-plugin-script-execution`, `vox-plugin-webhook`.

#### A5. Other point fixes

- `vox-arch-check` is in `crates/` and `layers.toml:39` but **missing from `Cargo.toml [workspace.dependencies]`**. Add it. (`where-things-live.md` line 70 says both must be updated when adding a workspace crate; vox-arch-check is currently violating its own rule.)
- `2026-05-08-workspace-reorg-outcome.md:9,96` and `2026-05-08-naming-and-guards-design.md:59,118` use the old name `vox-layer-check`. These are *historical narration* documenting a tool that was renamed during the same series — leave the prose alone but add a one-line note at the top of each file that the tool is now `vox-arch-check`.
- AGENTS.md §Retired Surfaces: after fixing the inverted gamify/ludus row, audit the rest of the table for any other crates that no longer match reality. (Spot-check: `vox-dei`/`vox-ars`/`vox-mens`/`vox-lexer`/`vox-parser`/`vox-hir`/`vox-typeck` are all confirmed retired by `Cargo.toml`/`layers.toml`/`crates/` — leave those rows alone.)

## Plan: 5 PRs + arch-check enhancement

PRs are ordered to minimize churn: A and B-merge first (fix the names everyone reads), then small C extractions, then C5 last. Each PR is self-contained with explicit verification commands.

### PR1 — Track A (SSOT) + C1 (vox-mcp-meta merge) + B-doc

**Scope:** All A1, A2, A3, A4, A5 fixes; merge `vox-mcp-meta` into `vox-mcp-registry`; rewrite all 19 stale `description =` fields; update `vox-corpus` `lib.rs` docstring; add binary-layer paragraph to `layers.toml` header.

**Verification:**
- `cargo run -p vox-arch-check` clean
- `cargo build --workspace` clean
- `cargo run -p vox-doc-pipeline` then `cargo run -p vox-doc-pipeline -- --check` clean
- `vox ci sync-ignore-files` no-op
- Manual: every line in §Track A above shows the canonical text

**Estimated diff:** ~30 files changed, ~250 lines net (mostly docs).

### PR2 — Track C2 (split `vox-package` to remove inversions)

**Scope:** New crate `vox-package-types` (L1 leaf — manifest, lockfile, package_kind, basic resolver types). `vox-package` keeps the L3 build/registry/cache/compiler-driver. Remove both entries in `[[known_inversions]]` block of `layers.toml`.

**Verification:**
- `cargo run -p vox-arch-check` clean (no inversions remain on `vox-package`)
- `cargo build --workspace` clean
- `cargo test -p vox-package -p vox-package-types` clean

**Estimated diff:** new crate + ~5 files in `vox-package/`, ~2 callers updated.

### PR3 — Track C3 (extract `vox-cli-ci`)

**Scope:** Move `crates/vox-cli/src/commands/ci/` (17.3K LoC) into a new `vox-cli-ci` library crate. `vox-cli` re-exports the public dispatcher. Add `vox-cli-ci` to `layers.toml` at L5 (or L3 if surveyable as a library). Verify the `vox ci` subcommand surface unchanged.

**Verification:**
- `cargo run -p vox-arch-check` clean
- `cargo build --workspace` clean
- `cargo test -p vox-cli-ci` clean
- Smoke test: `vox ci --help`, `vox ci sync-ignore-files`, `vox ci secret-env-guard` all behave identically

**Side-quest in same PR:** Move `vox-orchestrator-mcp` dep behind the `mcp-server` feature in `vox-cli/Cargo.toml`. Verify the no-`mcp-server` build still succeeds.

**Estimated diff:** new crate + ~15 files updated in vox-cli (mostly registration).

### PR4 — Track C4 (move `ops_ludus` from `vox-db` into `vox-gamify`)

**Scope:** Move `crates/vox-db/src/store/ops_ludus/` (3.2K LoC) into `crates/vox-gamify/src/db/`. If callers of those ops live in `vox-orchestrator` or `vox-cli`, route them through `vox-gamify` directly (gamify already depends on `vox-db`). If `vox-db` needs to expose the schema migrations from those ops, keep migrations in `vox-db/src/schema/` and only move the typed ops API.

**Verification:**
- `cargo run -p vox-arch-check` clean
- `cargo build --workspace` clean
- `cargo test -p vox-gamify -p vox-db` clean
- Manual: a fresh DB migration run still creates the gamify-domain tables

**Estimated diff:** ~20 files moved across two crates.

### PR5 — Track C5 (extract `vox-orchestrator-core`)

**Scope:** Move `crates/vox-orchestrator/src/orchestrator/` (12.3K LoC, the densest subdir) into a new `vox-orchestrator-core` crate at L3. `vox-orchestrator` keeps the dei_shim, planning, services, runtime glue. Adjust `max_loc` budgets in `layers.toml` to match the new sizes.

**Verification:**
- `cargo run -p vox-arch-check` clean
- `cargo build --workspace` clean
- `cargo build -p vox-orchestrator` (warm) measured pre/post; expected ≥15% incremental win on edits inside `orchestrator/`
- `cargo test -p vox-orchestrator -p vox-orchestrator-core` clean

**Estimated diff:** new crate + ~50 files moved + Cargo.toml dep updates in mcp/cli/d.

### PR6 — `vox-arch-check` enhancements (lock in the gains)

**Scope:** Two new lints + tightening of one existing lint.

1. **`description_present` (new, warn-by-default):** for every workspace member at L1+, fail (or warn per `[guards]`) if `Cargo.toml` `[package].description` is missing or shorter than 40 chars.
2. **`where_things_live_coverage` (new, warn-by-default):** parse `docs/src/architecture/where-things-live.md`; for every workspace crate not listed in either the subsystem or plugins section, warn. Driven by a single regex on `crates/vox-*` substrings.
3. **Tighten `docstring`** from warn → strict for crates at L0–L2 only. L3+ stays warn (the heavy crates carry historic debt that is out of scope here).

**Verification:**
- `cargo run -p vox-arch-check --warn-only` produces no surprises
- `cargo run -p vox-arch-check` (strict) clean after PR1–PR5 land
- Add a regression test: introduce an empty description in a fixture crate and assert the lint fires

**Estimated diff:** ~200 lines in `crates/vox-arch-check/src/main.rs` + a fixture.

## Sequencing & rationale

```
PR1 (A + C1 + B-desc)  → PR2 (C2: vox-package split)
                       → PR3 (C3: vox-cli-ci)
                       → PR4 (C4: ops_ludus)
                       → PR5 (C5: vox-orchestrator-core)
                       → PR6 (arch-check lints)
```

- **PR1 first** because every later PR's verification reads docs that PR1 fixes; doing names → moves avoids double-touching the same files.
- **PR2 before PR5** because `vox-package` split removes inversions, which is a precondition for clean strict layer-check after PR5 lands more cross-edges.
- **PR6 last** because the new lints would otherwise fail PR1's intermediate states.
- Each PR is independently mergeable and revertable.

**`where-things-live.md` cascading:** PR1 expands the table to cover all 83 current crates. PRs 2/3/5 each add one new crate (`vox-package-types`, `vox-cli-ci`, `vox-orchestrator-core`) — every such PR MUST add the corresponding row in the same diff (the existing instruction in `where-things-live.md:70` already requires this and PR6's `where_things_live_coverage` lint will enforce it once landed).

## Risks

| Risk | Mitigation |
|---|---|
| Renaming docs (A) is large but every line is a leaf edit; hard to cause runtime breakage. | Apply mechanically; rely on `cargo build` + `vox-doc-pipeline --check` to catch broken refs. |
| `vox-package` split (C2) might miss a circular import. | Resolver lives at L3 (depends on compiler); manifest/lockfile/package_kind types are pure-data L1. If a type can't move without dragging a runtime dep, leave it in L3 and document why. |
| `vox-cli-ci` extract (C3) risks subcommand registration drift. | Add a smoke test that runs `vox ci --help` and asserts the command list matches a checked-in golden file. |
| Moving `ops_ludus` (C4) might break existing migrations. | Keep migration SQL in `vox-db/schema/`; only move the typed ops surface. |
| `vox-orchestrator-core` (C5) extraction may surface hidden cross-deps. | Reference Phase-4 method (extract subdirectory whole, fix imports incrementally, test after each compile). |
| Arch-check `description_present` rule might fail on fixture crates. | Set `kind = "test-only"` on fixtures and exempt that kind from the lint. |

## Open questions for the executing agent

1. **CLI verb `vox ludus` vs. `vox gamify`.** The crate is canonical `vox-gamify`. Is the user-visible CLI verb `vox ludus` also retired, or is it preserved as a Latin alias? Default: leave the CLI verb alone unless the user signals otherwise; rename only crate references.
2. **AGENTS.md row for `vox-gamify ↔ vox-ludus`.** Two interpretations: (a) keep the row but flip direction (ludus retired → gamify canonical) so future readers see the retired form; (b) delete the row entirely since gamify was the original form. Recommendation: **(a) flip**. It's still useful information for someone who finds an old `vox-ludus` reference.
3. **`crates/vox-arch-check` and `Cargo.toml [workspace.dependencies]`.** Default: **leave it absent**, since it's a binary tool consumed only via `cargo run -p vox-arch-check` in CI and never as a library dependency. Document this exception in `where-things-live.md:70` ("Add a new workspace crate" instruction): "binary-only tools at L0/L3 don't need a `[workspace.dependencies]` entry." Override only if the user signals otherwise.
4. **Phase 6 (`vox-orchestrator-runtime`).** After C5 extracts `orchestrator-core`, is there still appetite for the original Phase-6 split, or is the C5 wedge sufficient? Default: **defer**, revisit if `vox-orchestrator` re-bloats past 60K LoC.

---

End of spec.
