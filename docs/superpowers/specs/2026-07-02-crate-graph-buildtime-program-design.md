---
title: "Crate-Graph Build-Time Program (Graphify fusion) — Design"
description: "Three small Graphify/analysis improvements (rebuild-cause diagnostic, what-if simulation, symbol-weighted dep edges), then one measured map run, ending in a ranked crate-restructuring proposal. Proposal-only: no splits executed in this program."
category: "Architecture SSOTs"
---

# Crate-Graph Build-Time Program — Design (2026-07-02)

## Problem

Workspace rebuilds feel far too broad: touching vox-cli (or a shared crate) appears to
rebuild large parts of the ~135-crate network. We do not currently know how much of that
pain is **structural** (fat dependency edges, monolith crates) versus **hygiene**
(feature-set drift between invocations, build-script reruns, env-var changes invalidating
fingerprints). Restructuring decisions made without that distinction — and without
edge-level evidence of what a dependency is actually *used for* — are guesses.

## Decisions taken during brainstorming

- **D — phased program**: tooling → map → restructure, designed now, executed in stages.
- **D — rebuild cause unknown**: Phase 1 must include a rebuild-cause diagnostic; we do
  not assume the pain is structural.
- **A — "search" means the code graph**: improvements target Graphify query/lens
  (`vox-graph-reader`), not the `vox-search` RAG/web crate. vox-search is untouched.
- **A — Phase 3 is proposal-only**: the program ends with a ranked, evidence-backed
  restructuring doc. Each actual split/cut becomes its own future spec→plan cycle.

## What already exists (build on, do not reinvent)

- `scripts/crate-build-audit.vox` — fuses `cargo metadata` adjacency,
  `docs/src/architecture/layers.toml`, and the latest `cargo build --timings` HTML into
  `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`. Stays the extractor.
- `vox-graph-reader::crate_model` — already computes `dependents` (transitive fan-in)
  and `blast_s` (self compile seconds + transitive dependents' seconds), and builds a
  graphify-shaped crate map from `crate-graph.v1.json` + `crate_audit.json`. Already
  degrades to dependents-only ranking when timings are absent.
- `vox graphify` CLI (vox-cli `commands/graphify/`) — has `Status/Ingest/Rebuild/
  Coverage/Index/Refresh/Gc/CrateMap` subcommands. `CrateMap` is the extension point.
- Symbol-level directed call/reference graph (Graphify SP-1, landed) in
  `graphify-out/graph.json`.
- In-flight vox-cli extraction (`vox-cli-core/contracts/ci/share/research/review`),
  with known remaining work (Tier-2 guards, dispatcher move, HeavyGuardHost,
  model/runtime blocked on a vox-db API issue). This program must reconcile against it,
  not duplicate it.

## Architecture

```
cargo metadata ──┐
cargo --timings ─┼→ crate-build-audit.vox → crate_audit.json + crate-graph.v1.json
layers.toml ─────┘                                    │
                                                      ▼
graph.json (symbol/call graph, SP-1) ──→  vox-graph-reader (crate_model + NEW analyses)
                                                      │
CARGO_LOG fingerprint capture (NEW) ──────────────────┤
                                                      ▼
                                    vox graphify crate-map {what-if, edges, why-rebuilt}
                                                      ▼
                             Phase 2 evidence pack → Phase 3 proposal doc
```

### New component 1 — Rebuild-cause diagnostic

CLI-native: `vox graphify why-rebuilt` (no wrapper script — a VoxScript would only
shell back into `vox`; the Rust side sets env vars cross-platform). Parser lives in
`vox-graph-reader::rebuild_causes` (pure text→classification, no I/O), capture +
reporting in the graphify command.

Behavior: `--capture` runs two consecutive `cargo check --workspace --exclude vox-gui`
invocations (**check, not build** — never tries to relink a possibly-running/locked
`vox.exe` on Windows), the second with
`CARGO_LOG=cargo::core::compiler::fingerprint=info`, captures its stderr to
`graphify-out/rebuild_fingerprint.log`, parses it, and classifies every dirty crate
into exactly one cause. `--log <path>` parses a previously captured log instead.

| Class | Meaning |
|---|---|
| `feature_drift` | feature set differs from previous compilation |
| `build_script_rerun` | build script re-ran (rerun-if-changed/env) |
| `env_change` | tracked env var changed |
| `dep_rebuilt` | recompiled only because a dependency was rebuilt (cascade) |
| `config_change` | rustflags / profile / compile-kind / config settings changed |
| `file_dirty` | source file newer than fingerprint (legitimate) |
| `unknown` | unrecognized log shape — raw line preserved verbatim, never guessed |

Output: `graphify-out/rebuild_causes.json` + printed summary table. An idle second build
that recompiles anything with a non-`file_dirty` cause is a hygiene bug and gets fixed
(or ticketed) before structural conclusions are drawn.

### New component 2 — What-if simulation

Pure functions in `vox-graph-reader::crate_model`:

- `what_if_cut(adj, self_s, edge) -> Delta` — remove one dep edge, recompute
  `blast_s`/`dependents` for affected crates, return per-crate and total deltas.
- `what_if_split(adj, self_s, crate, moved_deps) -> Delta` — model splitting a crate in
  two: a new node takes the listed dep edges (and an assumed share of self time is NOT
  modeled — time attribution stays with the original crate; the value is in the
  dependency-graph shape change, and the report says so explicitly).

CLI: `vox graphify crate-map --what-if-cut <from>:<to>` and
`--what-if-split <crate>=<dep1>,<dep2>,…`. Also a `--top-cuts N` mode that evaluates
every existing edge and ranks the N best single-edge cuts by blast_s saved.

### New component 3 — Symbol-weighted dependency edges

Join `graph.json` symbol references against crate dep edges: for each workspace edge
A→B, count distinct B-items referenced from A (`symbols_used`), and list them (capped).

- `symbols_used == 0` → **candidate** unused dependency. Always labeled
  "candidate — verify by removal": macros, trait impls, re-exports, and `#[derive]`
  usage can be invisible to the symbol graph. Verification = remove the dep and build.
- Low `symbols_used` × high `blast_s(B)` → top cut/shim candidates ("A uses 2 functions
  of B; move them or extract a types crate").

CLI: `vox graphify crate-map --edges [--json]`, sorted by (blast contribution, weight).
Output artifact: `graphify-out/edge_weights.json`.

## Phases and deliverables

### Phase 1 — Tooling (each piece independently landable)

| Piece | Input | Output | Done when |
|---|---|---|---|
| why-rebuilt | two instrumented `cargo check` runs | `rebuild_causes.json` + summary | correctly classifies a seeded feature-drift and a seeded env-change on a test crate; idle rebuild classifies clean |
| what-if | existing crate map | ranked deltas | `--what-if-cut` matches hand-computed BFS on a toy graph; `--top-cuts` ranks a constructed graph correctly |
| edge weights | `graph.json` × crate edges | `edge_weights.json`, zero-weight list | a known-thin and a known-heavy edge rank correctly; zero-weight list has no false positives on 3 spot-checks |

### Phase 2 — Map (one session, mostly running things)

1. Fresh cold build with `--timings` (existing `crate_audit.json` may be stale), then
   `crate-build-audit.vox` refresh.
2. `vox graphify refresh` so the symbol graph matches HEAD.
3. Run rebuild-why, `--top-cuts`, `--edges` → evidence pack in `graphify-out/`.
4. Sanity gates before trusting the data:
   - timings cover ≥90% of workspace crates;
   - symbol-graph coverage matches its coverage report;
   - `.claude/worktrees/` and `dist/` are excluded from extraction (known past
     pollution source that inflated the graph ~20%).

### Phase 3 — Proposal (analysis + writing; no execution)

A ranked doc in `docs/src/architecture/` (with required frontmatter). Each
recommendation carries:

- the specific edge/split,
- measured expected blast_s saving (from what-if),
- edge-weight evidence (symbols used, listed),
- risk class: `dep-removal` < `feature-gate` < `crate-split`,
- interaction with the in-flight vox-cli extraction (explicitly reconciled so the doc
  never re-proposes already-planned work).

If Phase 2's rebuild-cause data shows the dominant pain is hygiene, the proposal says
so and leads with hygiene fixes; structural recommendations are still produced but
ranked honestly against that finding.

## Error handling

- Missing/partial timings → analyses run in dependents-only mode and every affected
  number is labeled "no times" (crate_model already supports this degradation).
- Unrecognized fingerprint log lines → `unknown` class with raw line preserved; the
  parser never guesses. A high `unknown` rate (>20% of recompiles) fails the run with
  a message to update the parser for the current cargo version.
- Zero-weight edges are never reported as "unused", only "candidate — verify by
  removal".
- All artifacts are written atomically (write temp + rename) into `graphify-out/`.

## Testing

- Unit tests beside each new `crate_model` function: toy graphs with hand-computed
  expected `blast_s`/deltas, including a cycle (crate_metrics is already cycle-safe;
  what-if must stay so).
- Fingerprint parser: fixture log files checked into the crate's `tests/fixtures/`,
  one per cause class plus one garbage file asserting `unknown`.
- Edge weighting: fixture mini `graph.json` + mini crate graph with known join result.
- No end-to-end CI job — these are run-on-demand analysis tools. Workspace clippy
  stays green (`--exclude vox-gui` as usual); no new `unsafe`.

## Non-goals

- No vox-search changes.
- No new Graphify corpus registration / freshness auto-rerun (SP-2 owns that) and no
  GUI surface.
- No crate splits or dependency removals executed in this program (Phase 3 output is
  the proposal; verification-by-removal for zero-weight candidates is allowed as a
  *check*, reverted, with the result recorded in the proposal).
- No modeling of self-time attribution across a hypothetical split (stated limitation
  in what-if output).
