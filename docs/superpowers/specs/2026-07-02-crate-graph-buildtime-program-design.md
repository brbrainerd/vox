---
title: "Crate-Graph Build-Time Program (Graphify fusion) — Design"
description: "Graphify/analysis improvements (rebuild-cause diagnostic, what-if simulation, symbol-weighted dep edges, extraction hygiene, resolver precision), then one measured map run, ending in a ranked crate-restructuring proposal. Proposal-only: no splits executed in this program."
category: "Architecture SSOTs"
---

# Crate-Graph Build-Time Program — Design (2026-07-02, rev 2 adversarially audited)

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

## Rev-2 adversarial audit — verified facts this design is built on

All measured against the working tree on 2026-07-02:

- **The live symbol corpus is `.vox/cache/graphify/repo-code-graph/graph.json`**
  (registry-resolved), NOT the stale `graphify-out/graph.json` (which covers only 3
  crates and uses an older node schema). The live corpus: 29,185 nodes / 29,730 edges,
  covering **126 workspace crates** with 4,462 cross-crate edges (99.5% `resolved`,
  0.5% `dangling`). Native schema: node `{id: "<path>::<symbol>", label, kind, community}`
  (kind ∈ fn/struct/command/tool), edge `{source, target, confidence}` — **no
  `relation`, no `source_file`**; crate attribution comes from the id's path prefix
  (`crates/<name>/…`).
- **`workspace-hack` is a dependency of 71 of 121 crates** in
  `contracts/ci/crate-graph.v1.json`. It is deliberate feature-unification; any cut/edge
  analysis that doesn't exclude it by name ranks ~dozens of nonsense "cut
  workspace-hack" items first.
- **16 workspace crates have <10 extracted symbols** in the corpus; 111 of 593 dep
  edges touch such a crate. Zero-weight on those edges is invisibility, not unusedness.
- **Edge resolution is bare-name based** (`rebuild.rs::resolve_edges`): same-module
  preference, else unique-global fallback, else dropped. Consequences measured:
  all 4,462 surviving cross-crate edges point at globally-unique names (ambiguous ones
  were dropped) — so `symbols_used` systematically **undercounts** (deflation), and a
  bare local call can be misattributed to another crate when the name is globally
  unique elsewhere (inflation, rarer). Both directions are handled by labeling, never
  by trusting the count as ground truth.
- **The extraction walker does not exclude `dist`/`web-dist`/`.claude`** — 816 nodes in
  the live corpus are minified-bundle functions (e.g.
  `apps/vox-mental-tracker/web-dist/assets/index-*.js::createLaneMap`). This pollutes
  every Graphify consumer, not just this program.
- `vox-graph-reader::crate_model::crate_metrics` (dependents + blast_s, cycle-safe) is
  reused as-is; `vox graphify crate-map` already loads `crate-graph.v1.json` +
  `graphify-out/crate_audit.json` and has the `--ingest` path into Turso knowledge
  nodes.

## What already exists (build on, do not reinvent)

- `scripts/crate-build-audit.vox` — fuses `cargo metadata` adjacency,
  `docs/src/architecture/layers.toml`, and the latest `cargo build --timings` HTML into
  `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`. Stays the extractor.
- `vox-graph-reader::crate_model` — `dependents` / `blast_s` metrics and the
  graphify-shaped crate map; degrades to dependents-only ranking when timings are
  absent.
- `vox graphify` CLI (vox-cli `commands/graphify/`) — `Status/Ingest/Rebuild/Coverage/
  Index/Refresh/Gc/CrateMap`. `CrateMap` is the extension point.
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
repo-code-graph corpus (native schema) ──→  vox-graph-reader (crate_model + NEW analyses)
  ▲ (extraction hygiene + resolver                     │
     precision improvements land here)                 │
CARGO_LOG fingerprint capture (NEW) ──────────────────┤
                                                      ▼
                    vox graphify crate-map {what-if, edges} + vox graphify why-rebuilt
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

The parser handles both `package_id` span formats cargo has emitted: legacy
`package_id=<name> <version>` and modern PackageIdSpec
(`path+file:///…/name#0.1.0` / `…#name@ver`, cargo ≥1.77 — this repo runs 1.96), so
real captures yield crate names, never URLs.

Stated limitations (printed in output): the diagnostic observes **check units**, not
build/link units — pain that only exists at link time (e.g. relinking the vox binary)
is invisible here; and the per-crate collapse keeps the first specific cause, so a
crate with two distinct hygiene causes shows one (full line counts remain in the
summary).

### New component 2 — What-if simulation

Pure functions in `vox-graph-reader::what_if`:

- `what_if_cut(adj, self_s, edge) -> Delta` — remove one dep edge, recompute
  `blast_s`/`dependents` for affected crates, return per-crate and total deltas.
- `what_if_split(adj, self_s, crate, moved_deps) -> Delta` — model splitting: a new
  leaf node takes the listed dep edges. Self-time attribution is NOT modeled (stays
  with the original crate); the output states that split savings are an **upper
  bound** on the dependency-shape win.
- `top_cuts(adj, self_s, n, exclude_targets)` — evaluate every existing edge, rank the
  N best single-edge cuts by total blast_s saved. **`workspace-hack` is excluded by
  default** (deliberate feature-unification; cutting it is an anti-goal), via an
  exclude list so future deliberate-coupling crates can be added.

CLI: `vox graphify crate-map --what-if-cut <from>:<to>`,
`--what-if-split <crate>=<dep1>,<dep2>,…`, `--top-cuts N`.

### New component 3 — Symbol-weighted dependency edges

Join the **native-schema** repo-code-graph corpus against crate dep edges: for each
workspace edge A→B, count distinct B symbols referenced from A (`symbols_used`,
resolved-confidence edges only), and list them (capped at 20).

False-positive controls (each measured against the real corpus):

- `workspace-hack` edges are excluded from candidate reporting (they'd be 71 instant
  false candidates).
- Rows where either endpoint crate has **<10 extracted symbols** get
  `"low_visibility": true` and are excluded from the candidate list (still present in
  the full table). Today that quarantines 111 of 593 edges.
- `symbols_used == 0` on a well-covered edge → **candidate** unused dependency, always
  labeled "candidate — verify by removal": macros, trait impls, derives, re-exports,
  and resolver-dropped ambiguous calls are invisible. Verification = remove the dep
  and `cargo check -p <consumer> --all-targets` (dev/test/bench targets included).
- Output `meta` reports corpus coverage stats and BOTH cross-crate ref counters
  (`refs_in_dep_graph`, `refs_not_in_dep_graph`) so the noise ratio is computable —
  above ~0.20 undeclared, the whole table is labeled low-confidence.

Low `symbols_used` × high `blast_s(B)` → top cut/shim candidates ("A uses 2 items of
B; move them or extract a types crate").

CLI: `vox graphify crate-map --edges`. Output: `graphify-out/edge_weights.json`.

### New component 4 — Graphify extraction hygiene (engine improvement)

`walk_source_files` additionally excludes `dist`, `web-dist`, and `.claude`
directories. Removes the 816 minified-bundle nodes from every future corpus rebuild —
this improves Graphify for **all** consumers (agents querying the code graph stop
retrieving bundle noise), independent of this program. `patches/` (vendored patched
deps) stays included: it is source, and crate attribution already ignores non-`crates/`
paths for this program's analyses.

### New component 5 — Resolver same-crate preference (engine improvement)

`resolve_edges` gains a middle preference tier: same-module → **same-crate unique** →
global-unique → drop. Today an ambiguous bare call whose definitions live in the same
crate as the caller is dropped entirely; the new tier recovers those edges (recall up)
and can only reduce, never add, false cross-crate attributions. Behavior change is
covered by unit tests on all four tiers.

## AI-first output contracts

Vox's tooling is agent-facing first. Every artifact this program emits obeys:

1. **Pure-stdout JSON**: when an analysis flag is used, stdout carries exactly one JSON
   document; all notes/warnings go to stderr. `vox graphify crate-map --top-cuts 20 >
   file.json` must always yield valid JSON. Enforced structurally: the analysis flags
   are mutually exclusive at the clap level, so two documents can never interleave.
2. **`schema_version` + provenance** on every artifact:
   `{"schema_version": 1, "provenance": {"generated_by": "<argv>", "git_sha": "...",
   "corpus_digest": "..."}}` — an agent (or a future run) can tell exactly what
   produced a file and whether it is stale.
3. **Atomic writes** (temp + rename) into `graphify-out/`.
4. **Recallable**: the crate map already ingests into Turso knowledge nodes
   (`crate-map --ingest`); analysis artifacts live in `graphify-out/` with stable
   names so agents can re-read them across sessions. (Ingesting analyses themselves is
   deferred — YAGNI until an agent actually needs recall-time access.)
5. The Phase 3 proposal doc ends with a **machine-readable appendix**: the ranked
   recommendations duplicated as a fenced JSON block, so future agents consume the
   proposal without prose-parsing.
6. **One entry point**: Phase 2 ends by writing
   `graphify-out/evidence_index.v1.json` — a manifest of every artifact, the sanity
   gate results, and the dep-kind semantics, so an agent starts from one file instead
   of tribal knowledge of six filenames.

## Phases and deliverables

### Phase 1 — Tooling (each piece independently landable)

| Piece | Input | Output | Done when |
|---|---|---|---|
| why-rebuilt | two instrumented `cargo check` runs | `rebuild_causes.json` + summary | correctly classifies fixture lines for all six specific classes; garbage → `unknown`; idle rebuild classifies clean |
| what-if | crate map inputs | ranked deltas | matches hand-computed BFS on a toy graph; `top_cuts` excludes `workspace-hack` by default; cycle-safe |
| edge weights | native corpus × crate edges | `edge_weights.json` | counts distinct resolved cross-crate symbols on a fixture; flags low-visibility rows; excludes workspace-hack from candidates; zero-weight rows labeled candidate-only |
| walker hygiene | — | cleaner corpus | `dist`/`web-dist`/`.claude` files excluded, unit-tested |
| resolver tier | — | higher-recall edges | four-tier preference unit-tested, incl. the recovered same-crate-ambiguous case |

### Phase 2 — Map (one session, mostly running things)

1. Regenerate the dependency graph: `vox ci affected-crates --regen` (the committed
   `crate-graph.v1.json` may be stale).
2. Fresh cold build with `--timings`, then `crate-build-audit.vox` refresh.
3. **Force-rebuild** the repo-code-graph corpus (`vox graphify rebuild`) so the walker
   hygiene + resolver improvements are actually in the data (a freshness-based
   `refresh --auto` would skip a "fresh" corpus built with the old extractor).
4. Run why-rebuilt capture, `--top-cuts`, `--edges` → evidence pack in `graphify-out/`.
5. Sanity gates before trusting the data:
   - timings cover ≥90% of workspace crates;
   - corpus covers ≥90% of workspace crates (post-rebuild; today 126/121-ish by id
     prefix — the corpus also sees non-workspace dirs);
   - zero nodes under `dist/`, `web-dist/`, `.claude/`;
   - `refs_not_in_dep_graph` ratio reported and eyeballed.
6. Zero-weight verification (checks only, reverted): for up to 5 well-covered
   zero-weight candidates, remove the dep, `cargo check -p <consumer> --all-targets`,
   record PASS/FAIL + blind-spot reason, revert.

### Phase 3 — Proposal (analysis + writing; no execution)

A ranked doc in `docs/src/architecture/` (with required frontmatter). Ordering rule:
**hygiene findings first** — if the why-rebuilt data shows feature drift / env churn /
build-script reruns, those fixes are cheaper than any restructuring and benefit every
build; structural recommendations are ranked honestly below them.

Each structural recommendation carries:

- the specific edge/split,
- measured expected blast_s saving (from what-if),
- edge-weight evidence (symbols used, listed; visibility caveats),
- risk class: `dep-removal` < `feature-gate` < `crate-split`,
- verification status (for zero-weight candidates checked in Phase 2),
- interaction with the in-flight vox-cli extraction (explicitly reconciled so the doc
  never re-proposes already-planned work).

Plus the machine-readable JSON appendix (AI-first contract #5).

## Execution model (process, not code)

- **Parallel reads, sequential writes.** Read-heavy steps (Phase 2 evidence analysis,
  Phase 3 reconciliation against the vox-cli split specs) fan out to parallel
  read-only subagents; all file writes and commits happen in the main session
  (dispatched agents are write-denied in this environment).
- **TDD for every Phase 1 behavior**: failing test first, minimal implementation,
  one commit per green step. Run-book phases (2/3) are not TDD — their rigor comes
  from the sanity gates and verification-by-removal.
- **Optional workflow-assisted verification** (requires explicit user opt-in per
  orchestration policy): Phase 3 recommendations can each be adversarially verified by
  independent skeptic subagents ("try to refute: cutting edge X saves Ys") before the
  doc is finalized. Without opt-in, the main session verifies serially.

## Error handling

- Missing/partial timings → analyses run in dependents-only mode; every affected
  number is labeled "no times" (crate_model already supports this degradation);
  warning goes to **stderr**.
- Unrecognized fingerprint log lines → `unknown` class with raw line preserved; the
  parser never guesses. Unknown rate >20% fails the run (after writing the artifact)
  with instructions to extend the classifier from the preserved raw lines.
- Zero-weight edges are never reported as "unused", only "candidate — verify by
  removal"; low-visibility rows are never candidates at all.
- All artifacts are written atomically (temp + rename).

## Testing

- Unit tests beside each new `vox-graph-reader` function: toy graphs with
  hand-computed expectations, including a dependency cycle (what-if must stay
  cycle-safe), the workspace-hack exclusion, and low-visibility flagging.
- Fingerprint parser: fixture log file checked into `tests/fixtures/` covering all six
  specific classes + garbage lines asserting `unknown`; Phase 2 feeds real captured
  lines back into the fixture when they classify as `unknown`.
- Edge weighting: fixture mini corpus in the **native schema** with known join result.
- Walker + resolver: unit tests for the new filters and the four-tier resolution.
- No end-to-end CI job — these are run-on-demand analysis tools. Workspace clippy
  stays green (`--exclude vox-gui` as usual); no new `unsafe`.

## Non-goals

- No vox-search changes.
- No new Graphify corpus registration / freshness auto-rerun (SP-2 owns that) and no
  GUI surface.
- No crate splits or dependency removals executed in this program (Phase 3 output is
  the proposal; verification-by-removal is a *check*, reverted, with the result
  recorded).
- No modeling of self-time attribution across a hypothetical split (stated limitation
  in what-if output).
- No ingest of analysis artifacts into Turso (deferred until an agent needs
  recall-time access; the crate map itself already ingests).

## Dependency-graph semantics (verified 2026-07-02)

`crate-graph.v1.json` is produced by `vox ci affected-crates --regen`
(`vox-cli-ci/src/affected_cmd.rs::graph_from_metadata`): it reads
`cargo metadata` `resolve.nodes[].deps` and keeps **all dependency kinds** — normal,
build, AND dev — without distinction (it never inspects `dep_kinds`), filtered to
workspace members. Consequences the analyses and proposal must state:

- blast_s is **CI-shaped**: it models what rebuilds under `--all-targets` CI, where
  dev-deps do trigger rebuilds. A dev-only edge does NOT cost production/check builds.
- Cutting a dev-only edge is cheaper and lower-risk than a normal-dep cut; the
  proposal's risk classes account for this by checking the consumer's `Cargo.toml`
  section for each recommended cut.
- Splitting the SSOT by dep kind is a possible future regenerator improvement but is
  out of scope here (the file is a committed contract consumed by affected/parity CI
  gates).
