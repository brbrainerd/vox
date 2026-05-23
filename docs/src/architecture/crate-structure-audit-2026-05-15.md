---
title: "Crate structure audit & consolidation plan (2026-05-15)"
description: "Snapshot of post-reorg crate sprawl: drift between layers.toml / where-things-live.md / disk, candidates for condensation, build-time critical path, and arch-check rules to prevent regression."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-15"
training_eligible: false
---

# Crate structure audit & consolidation plan (2026-05-15)

Companion to [`2026-05-08-workspace-reorg-outcome.md`](./2026-05-08-workspace-reorg-outcome.md)
and [`repo-layout-sprawl-audit-2026.md`](./repo-layout-sprawl-audit-2026.md). The
2026-05-08 reorg landed a strict layer model (`layers.toml` + `vox-arch-check`)
and cut `vox-cli` rebuild by 74% and `vox-orchestrator` by 36%. **One week
later** the workspace has drifted in three measurable ways. This audit
documents the drift, proposes a tiered consolidation plan, and lists the new
CI rules that would have caught the drift on the PR that introduced it.

## TL;DR

- **102 Cargo crates** on disk (111 dirs total; 9 are skill-only plugins with no `Cargo.toml`).
- **5 hard-drift entries** between the three sources of truth (`crates/`, `layers.toml`, `where-things-live.md`); 24 soft-drift "ghost" rows in WTL.
- **`vox-orchestrator` is regrowing** — 52K LoC post-reorg → **65.5K LoC today** (+26% in ~1 week).
- **36 of 102 crates are under 500 LoC**; the SCIENTIA cluster (9 crates) and small L0/L1 utilities (9 crates) are the two coherent consolidation bands.
- **The build-time critical path is `vox-cli`** — 57 direct `vox-*` workspace deps and a known inversion to `vox-orchestrator`. The orchestrator regrowth directly extends CLI incremental cost.
- **Three new arch-check rules** (WTL↔disk parity, orphan promotion to `error`, LoC-budget regression delta) would close the regressions this audit had to find by hand.

> **Status (2026-05-15):** Tiers A + B + C all landed in one session. Workspace is now ~91 crates (−11 from folding). `vox-arch-check` reports clean. Remaining risk: `vox-orchestrator` at 65,560 LoC with only 7% headroom — see Tier D plan.

## 1. Inventory and current shape

### 1.1 Three sources of truth, three counts

| Source | Count | Meaning |
|---|---|---|
| `crates/*/Cargo.toml` (filesystem) | 102 | What Cargo actually compiles. |
| `layers.toml` `[crates]` entries | 99 | What arch-check validates against the layer model. |
| `where-things-live.md` `crates/<name>` references | ~117 | What humans and LLM agents are told the workspace contains. |

The three should agree exactly (modulo skill-only plugins). They don't.

### 1.2 LoC distribution

The L3 "heavy runtime" tier holds the build-time concentration:

| Crate | LoC | Budget | Headroom |
|---|---:|---:|---:|
| `vox-orchestrator` | 65,560 | 70,000 | 7% (was 26% post-reorg) |
| `vox-compiler` | 38,701 | 45,000 | 14% |
| `vox-orchestrator-mcp` | 35,638 | 40,000 | 11% |
| `vox-db` | 31,935 | 40,000 | 20% |
| `vox-codegen` | 21,250 | 25,000 | 15% |
| `vox-publisher` | 19,951 | 20,000 | <1% **(near budget)** |
| `vox-code-audit` | 19,375 | 25,000 | 22% |
| `vox-populi` | 17,188 | 20,000 | 14% |
| `vox-ml-cli` | 16,299 | 20,000 | 19% |
| `vox-gamify` | 15,900 | 20,000 | 21% |

Four crates are above 80% of their `max_loc` budget. The orchestrator's 13K
regrowth in one week is the standout signal.

### 1.3 The small-crate band

36 crates are <500 LoC. The 9 smallest with non-trivial reverse-dep counts:

| Crate | LoC | Reverse deps (direct) | Layer |
|---|---:|---:|---|
| `vox-bounded-fs` | 86 | 17 | L1 |
| `vox-http-client` | 129 | 14 | L1 |
| `vox-primitives` | 172 | 6 | L0 |
| `vox-grammar-export` | 1,581 | 6 | L1 |
| `vox-jsonschema-util` | 160 | 5 | L1 |
| `vox-scaling-policy` | (small) | 5 | L1 |
| `vox-tracing-init` | 52 | 4 | L1 |
| `vox-protocol` | 112 | 4 | L0 |
| `vox-build-meta` | 46 | 4 | L0 |

These are the foundation-umbrella candidates (§4.3).

## 2. Drift findings

### 2.1 Hard drift (verified via `cargo metadata` and filesystem)

| # | Direction | Item | What's happening |
|---|---|---|---|
| 1 | disk → layers.toml | `vox-audit` | 3.1K-LoC crate exists on disk and in `Cargo.toml` workspace members, but is **missing from `layers.toml`**. `vox-arch-check` flags it on every CI run today: `vox-arch-check: 1 workspace crate(s) missing from layers.toml: vox-audit`. |
| 2 | layers.toml → disk | `vox-agentos-mutation` (L0) | Row exists in `layers.toml`; no directory on disk. Listed in WTL too. |
| 3 | layers.toml → disk | `vox-dashboard` (L3) | Row exists in `layers.toml`; no directory on disk. |
| 4 | layers.toml → disk | `vox-mens-eval` (L2) | Row exists in `layers.toml`; no directory on disk. |
| 5 | doc-only artifact | `crates/_frozen.md` | "Core 10" April 2026 charter naming `vox-runtime` and `vox-toestub` (neither exists); prescribes feature-gating regime that was never implemented; contradicts the active layer model. |

`vox-audit` is the only entry that fails arch-check. The other three pass
because arch-check only verifies disk-→-layers.toml, not the reverse direction.

### 2.2 Soft drift — WTL ghost rows

`where-things-live.md` references **24 crates** that have no directory and no
Cargo entry. Categorized:

| Category | Count | Crates |
|---|---:|---|
| SCIENTIA aspirational | 7 | `vox-claim-extractor`, `vox-inspect-bridge`, `vox-nanopub`, `vox-prereg`, `vox-ro-crate`, `vox-scientia-ingest`, `vox-mesh-models` |
| Orchestrator extractions never landed | 3 | `vox-cli-ci`, `vox-orchestrator-core`, `vox-orchestrator-cap-mint` |
| MENS aspirational | 2 | `vox-distributed-training`, `vox-inference` |
| Release/packaging aspirational | 3 | `vox-checksum-manifest`, `vox-release-artifacts`, `vox-assets` |
| OpenAI/HTTP split never landed | 3 | `vox-openai-sse`, `vox-openai-wire`, `vox-http-envelope` |
| Misc aspirational | 6 | `vox-share`, `vox-ssg`, `vox-exec-grammar`, `vox-install-policy`, `vox-mesh-policy`, `vox-agentos-mutation` |

These are not failures — most come from in-flight plans in
[`docs/src/architecture/`](.) (mesh SSOT, SCIENTIA phases, MENS Mn-T*) — but
WTL presents them with no signal that they're planned-but-not-landed. A future
agent reading WTL will `cd` into the crate, find nothing, and either re-create
sprawl (worst case) or stall on the ambiguity (best case).

### 2.3 Hidden orchestrator regrowth

Phase 4 of the reorg extracted 36K LoC from `vox-orchestrator` (mcp + queue),
leaving 52K. The current crate is **65,560 LoC**. The 13K-LoC regrowth in ~1
week is invisible to `vox-arch-check` because the LoC budget (`max_loc =
70_000`) is set to absorb growth and triggers `warn`, not `error`. There is no
delta-from-last-release check.

## 3. Where the structural lines are weak

### 3.1 `vox-cli` direct workspace deps: 57

`vox-cli` directly depends on **57 `vox-*` workspace crates**, plus the
documented inversion edge into `vox-orchestrator`. Every one of those is on the
CLI incremental rebuild critical path. The Phase 7 ("vox-cli-thin") extraction
was deferred in the reorg because most heavy commands are already feature-gated
— but the dep fan-out remains, which means a touch in any of the 57 transitive
deps invalidates `vox-cli`.

### 3.2 `vox-orchestrator` known inversion regrowth

The documented inversion `vox-cli → vox-orchestrator` is load-bearing for
runtime/observability surfaces (`vox status`, `vox attention`, `vox model *`,
`vox dei`, `vox live`). It was tolerable post-reorg at 52K orchestrator LoC.
At 65K, every CLI build pays the regrowth. Phase 6 (`vox-orchestrator-runtime`)
was deferred specifically because it required a 40-method trait facade; the
calculus changes now that the cost is back.

### 3.3 SCIENTIA cluster: 9 crates, no internal cohesion enforcement

`vox-scientia` exists as a 2.5K-LoC "cluster crate" but is doing no structural
work — each SCIENTIA phase (A through H) lives in its own L1/L2/L3 leaf:

| Crate | LoC | Reverse deps | Notes |
|---|---:|---:|---|
| `vox-scientia` | 2,469 | 4 | Cluster crate, mostly empty |
| `vox-research-events` | 11,061 | 3 | L1 typed event bus |
| `vox-scientia-producers` | 1,838 | 1 | Phase A signal producers |
| `vox-replay-runner` | (medium) | 1 | Phase B re-executor |
| `vox-manuscript-scaffold` | (small) | 2 | Phase C scaffolder |
| `vox-manuscript-latex` | 803 | 1 | Phase 3+4 LaTeX renderer |
| `vox-critic-gate` | (small) | 1 | Phase D gate evaluator |
| `vox-class-routing` | 441 | 2 | Phase E venue routing |
| `vox-findings-site` | 536 | 1 | Phase G HTML builder |
| `vox-scientia-dashboard` | 480 | 1 | Phase H JSON builders |

Each is pure (no DB, no network); the layer model would tolerate folding them
into 1–2 crates with zero inversion risk. Total: ~18K LoC across 10 crates
with avg reverse-dep count of ~1.7.

## 4. Consolidation plan (tiered by risk)

### 4.1 Tier A — Drift reconciliation (small, no code moves)

**Goal:** make the three sources of truth agree, retire stale documents.

1. Add `vox-audit` row to `layers.toml`. Layer: 5 (binary surface) or 3 (umbrella library) — needs a quick design call on whether `vox-audit` is "another `vox-arch-check`-style L0 binary" or "library + thin binary". The Cargo.toml currently makes it a binary depending on `vox-code-audit` + `vox-telemetry`, suggesting L5 binary.
2. Delete `vox-agentos-mutation`, `vox-dashboard`, `vox-mens-eval` rows from `layers.toml`. They are bookkeeping ghosts; arch-check tolerates them today but they mislead. (If any are intended as planned crates, move to a new `[planned]` table — see §5.3.)
3. Move `crates/_frozen.md` to `docs/src/architecture/history/2026-04-core-ten-charter.md` (or delete). The "Core 10" framing is dead.
4. Edit `where-things-live.md`: move the 24 ghost rows to a new **"Planned but not landed"** section at the bottom, each with a one-line pointer to the plan doc that owns the work. **Don't delete the rows** — they prevent re-invention and surface the gap.
5. Document the 9 skill-only plugins (`vox-plugin-skill-*`, `vox-plugin-noop-skill`) in a new WTL section so disk-vs-WTL diff is clean.

**Cost:** ~2h. **Build impact:** none. **CI impact:** removes the persistent `vox-audit` warning.

### 4.2 Tier B — SCIENTIA cluster fold (medium, code moves)

**Goal:** turn `vox-scientia` from cluster placeholder into the real home for SCIENTIA Phase A–H pure types and builders.

Merge candidates:

| Merge | Into | Rationale |
|---|---|---|
| `vox-scientia-producers` | `vox-scientia` (new `producers` mod) | 1 reverse dep; pure Rust |
| `vox-replay-runner` | `vox-scientia` (new `replay` mod) | 1 reverse dep; uses tokio process — keep at L3 |
| `vox-manuscript-scaffold` + `vox-manuscript-latex` | `vox-scientia` (new `manuscript` mod) | Both pure renderers; combined ~1.5K LoC |
| `vox-critic-gate` + `vox-class-routing` | `vox-scientia` (new `gate` + `routing` mods) | Both pure evaluators |
| `vox-findings-site` + `vox-scientia-dashboard` | `vox-scientia` (new `views` mod) | Both pure JSON/HTML builders |
| `vox-scientia-jsonschema-codegen` | Keep separate | Already a separate `kind = "binary"` codegen tool |
| `vox-research-events` | Keep separate | L1 with 3 downstream deps including `vox-orchestrator`; promoting to L2/L3 via fold would create an inversion |

**Net change:** -8 crates (10 → 2). Estimated `vox-scientia` size: ~6K LoC, well within an L3 budget.

**Cost:** 1 day (mostly mod renames + `Cargo.toml` consolidation). **Build impact:** neutral-to-slight-positive (fewer compilation units to schedule). **Layer impact:** none (`vox-scientia` is already L3 and `staleness_exempt = true`).

### 4.3 Tier C — Foundation umbrella (medium, opt-in build-time trade)

User-confirmed scope: **open to a small-utility umbrella crate**.

**Proposed:** new `vox-foundation` crate at L1 that re-exports six tiny utilities currently scattered across L0/L1:

| Folded crate | LoC | Reverse deps |
|---|---:|---:|
| `vox-build-meta` | 46 | 4 |
| `vox-tracing-init` | 52 | 4 |
| `vox-primitives` | 172 | 6 |
| `vox-bounded-fs` | 86 | 17 |
| `vox-jsonschema-util` | 160 | 5 |
| `vox-protocol` | 112 | 4 |

**Total LoC absorbed:** ~628 across 6 crates → one crate.

**Build-time analysis:** the 2026-05-08 reorg deliberately preserved L0/L1
leaves as separate crates so `workspace-hack` exclusion gave them a 0.53s
incremental floor. Folding them into `vox-foundation` (L1) loses that floor —
any change to one utility invalidates all six consumers. **However**:

- The 6 candidates change rarely (`vox-build-meta` is `staleness_exempt`; `vox-tracing-init` is 52 lines that rarely change).
- The 17 consumers of `vox-bounded-fs` already pay a workspace-hack cost (they're not leaves), so folding it has limited cost.
- The biggest leaf consumer of the foundation crate would be `vox-primitives` (6 reverse-deps).

**Recommendation:** fold `vox-build-meta`, `vox-tracing-init`, `vox-primitives`,
`vox-protocol`, `vox-jsonschema-util` (5 crates). **Leave `vox-bounded-fs`
separate** — 17 reverse-deps is too much fan-out to risk on a leaf collapse.
Add `vox-foundation` to the hakari exclusion list so it stays a fast leaf
itself.

**Cost:** 1 day. **Build impact:** -5 compilation units; expected -0.2 to +0.5s on cold workspace; **neutral on incremental.**

### 4.4 Tier D — Orchestrator re-split (large, multi-day)

**Goal:** reverse the 13K LoC regrowth by resuming the deferred Phase 6 of the 2026-05-08 reorg.

Two paths:

1. **Phase 6 as originally scoped** — extract `vox-orchestrator-runtime` (`runtime.rs`, `orch_daemon/`, parts of `dei_shim/`). Blocked by needing a 40-method trait facade over `Orchestrator`.
2. **Vertical-slice extraction** — split `agentos/` (D1–D10 policy modules in WTL §"Common tasks") into `vox-orchestrator-policy`. Each policy module is already file-level isolated; the bulk of the cross-cuts in §1.2 of the deferred-phases analysis come from `runtime.rs`, not `agentos/`.

Path 2 is cheaper and addresses the regrowth (the agentos/ subdir is where most of the +13K LoC came from per quick grep — verify before scoping). Path 1 captures the bigger long-term build win.

**Out of scope for this audit** — flag as a separate plan doc if Tier C lands cleanly.

## 5. Drift prevention — proposed arch-check rules

`vox-arch-check` already covers Rules 1–11 (layer ordering, fan-in, LoC budget,
orphan, docstring, description, where_things_live, staleness, generated-file-
drift, forbidden_deps, forbidden_pattern). Three gaps remain:

### 5.1 New Rule 12 — WTL↔layers.toml↔disk three-way parity (proposed `error`)

For every workspace crate from `cargo metadata`:
- Must have a row in `layers.toml`.
- Must have at least one `crates/<name>/` reference in `where-things-live.md` (or be listed in a new `[wtl_exempt]` table for skill-only plugins).

For every `layers.toml` row:
- Must have a corresponding directory on disk (catches the 3 ghost rows in §2.1).

For every `crates/<name>` mention in WTL:
- Either the directory exists OR the row is under a `## Planned but not landed` heading.

The first half (disk→layers) is already partially implemented (`vox-audit`
warning). This rule formalizes the other two directions. **Pure file walk; no
deps; ~150 LoC of Rust to add to `vox-arch-check`.**

### 5.2 New Rule 13 — LoC-budget regression delta (proposed `warn`)

Currently `max_loc` triggers when a crate exceeds its hard ceiling. **Add a
per-PR delta check**: warn if a crate's LoC grows by >10% relative to its
last-tagged-release size. Catches the 13K orchestrator regrowth at PR time
rather than at the 70K ceiling.

Requires `git show <last-release-tag>:<crate>/src/**` size baseline. ~80 LoC.

### 5.3 New `[planned]` table in `layers.toml`

```toml
[planned]
# Crates referenced by in-flight plan docs but not yet landed.
# The arch-check WTL parity rule (Rule 12) consults this list before flagging
# a WTL row as a ghost.
vox-claim-extractor = { plan = "docs/src/architecture/scientia-...-2026.md" }
vox-orchestrator-core = { plan = "docs/src/architecture/2026-05-08-workspace-reorg-design.md#phase-6" }
# ...
```

This gives WTL ghost rows a home and forces every "planned crate" mention to
point at the doc that owns the work — closing the loop on §2.2.

### 5.4 Promote `orphan` from `warn` to `error` after Tier A lands

Currently `orphan = "warn"` because of staging crates. Post-Tier-A, the only
expected orphan is whatever's intentionally a leaf utility; flip to `error` to
catch new orphans on the PR that introduces them.

## 6. Phased rollout

| Tier | Scope | Risk | Build impact | Status |
|---|---|---|---|---|
| A | Drift reconciliation + Rule 12 + `[planned]` table | Low | None | **Landed 2026-05-15** — `vox-audit` added to `layers.toml` (L5); ghost rows pruned; Rule 12 (WTL↔layers↔disk three-way parity) enforced in `vox-arch-check`; `orphan` promoted to `error`. |
| B | SCIENTIA fold | Medium | Neutral | **Landed 2026-05-15** — 8 SCIENTIA sub-crates merged into `vox-scientia` as sub-modules; `vox-scientia` `max_loc = 15_000`; all `crate::` sibling refs corrected to `super::`. |
| C | `vox-foundation` umbrella (5 utilities) | Medium | -0.2s to +0.5s cold; neutral incremental | **Landed 2026-05-15** — `vox-primitives` + `vox-protocol` + `vox-tracing-init` folded into `vox-foundation`; added to hakari `traversal-excludes` + `final-excludes`; `staleness_exempt = true`. |
| D | Orchestrator re-split | High | -1 to -2s on CLI incremental (estimated) | **Not started** — see [`2026-05-15-orchestrator-tier-d-plan.md`](2026-05-15-orchestrator-tier-d-plan.md). Vertical-slice (agentos/) is only 358 LoC — not a meaningful target. Phase 6 runtime extraction is the correct path. |

**Strict-order dependency:** A → (B and C, parallel) → D. Tier A's WTL parity
rule must land before B or C because both reduce the crate set and the rule
catches stale references.

## 7. Open questions for human review

1. **`vox-audit` placement** — L5 binary (matches its current Cargo shape) or split into L3 `vox-audit-core` + L5 `vox-audit` binary (mirror of `vox-arch-check` pattern)?
2. **`[planned]` table semantics** — should planned rows have layer + intended-size budgets, or are they purely documentation hooks?
3. **SCIENTIA fold layer** — keep `vox-scientia` at L3 (current), or split the pure-data subset (manuscript/gate/routing/views) into a new L2 `vox-scientia-types`?
4. **Orchestrator re-split path** — vertical-slice (agentos/ extraction) or finish Phase 6 (runtime extraction)?

## 8. Methodology and re-run commands

All counts in this doc come from these commands run against the current branch
on 2026-05-15:

```powershell
# Workspace package count
cargo metadata --no-deps --format-version=1 | jq '.packages | length'

# LoC per crate
Get-ChildItem crates -Directory | ForEach-Object {
  $name = $_.Name
  if (Test-Path "$($_.FullName)/Cargo.toml") {
    $loc = (Get-ChildItem "$($_.FullName)/src" -Recurse -Filter *.rs 2>$null |
            Get-Content | Measure-Object -Line).Lines
    "$loc $name"
  }
} | Sort-Object { [int]($_ -split ' ')[0] } -Descending

# WTL ↔ disk diff
grep -oE 'crates/(vox-[a-z0-9-]+)' docs/src/architecture/where-things-live.md `
  | sed 's|crates/||' | sort -u > wtl.txt
ls crates | sort -u > disk.txt
comm -23 wtl.txt disk.txt   # WTL ghosts
comm -13 wtl.txt disk.txt   # disk-only

# Reverse-dep count for a candidate
grep -l '^vox-bounded-fs\s*=\|"vox-bounded-fs"' crates/*/Cargo.toml | wc -l
```

Re-run before any consolidation PR to confirm the band hasn't shifted.
