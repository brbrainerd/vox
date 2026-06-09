---
title: "Semantic Test-Coverage Graph Strategy"
description: "Strategy for a searchable, regenerable map of what concepts are proven vs unproven by the test suite — overlaid on the graphify code graph. Distinguishes reached/targeted/proven coverage from line coverage."
category: "Architecture SSOTs"
status: "proposed"
training_eligible: false
---

# Semantic Test-Coverage Graph Strategy (2026-06-07)

## 1. Problem

We have ~3,261 Rust source files across 116 crates, 1,410 files with `#[test]`,
968 `#[cfg(test)]` modules, 91 `tests/` dirs, 1,472 `.vox` test files, and 629
goldens. We already have **line coverage**: `cargo llvm-cov` +
[`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml) +
`vox ci coverage-gates` (workspace floor 50%, per-crate floors).

Line coverage answers *"did this line execute during some test?"* — which is
exactly the signal the maintainer **does not** want to optimize for ("tests that
touch a lot of files uselessly"). It cannot distinguish a function that is
*incidentally executed* from one whose *behavior is actually asserted*.

The gap: there is no map of **what behavior is proven vs merely reached**, and no
way to search it ("show every public symbol in `vox-compiler` that is reached but
never asserted on").

## 2. The three strengths of a coverage claim

Every (test → symbol) relationship carries one of three `proof_strength` values:

| Strength    | Meaning                                                                 | Source signal                          |
|-------------|-------------------------------------------------------------------------|----------------------------------------|
| `reached`   | Symbol's code ran during *some* test.                                    | `cargo llvm-cov ... --json` (we have it)|
| `targeted`  | A test *names* the symbol / its module as its subject (intent to test).  | Static: imports + call graph in tests  |
| `proven`    | A test makes an **assertion on the symbol's observable behavior** (return value or effect, including error/edge paths) — not just "it didn't panic." | Static assertion-target analysis + LLM |

**Semantically covered vs not** = the gap between `reached` and `proven`. A
function can be 100% line-covered and 0% proven. That delta is the deliverable.

## 3. Architecture: a derived layer on the graphify graph

The map's home is `graphify-out/graph.json` (already built: 15,333 AST nodes,
29k edges, community-clustered). We **overlay** new node and edge types rather
than building a separate store, so it is queryable today via `/graphify query`
and the HTML viz.

### New node types
- **`Test`** — one per `#[test]` / golden / `.vox` case. Fields: `id`, `test_kind`
  (`unit` | `integration` | `golden` | `vox`), `source_location`, `crate`.
- **`Behavior`** — a single claim a test proves, in plain language
  (e.g. *"qlora LoRA delta is computed in activation dtype, not F32"*). Carries
  `confidence` and the EXTRACTED / INFERRED / AMBIGUOUS honesty tag.

### New edge types (each carries `proof_strength`)
- `Test --reaches--> Symbol`   (deterministic, from llvm-cov)
- `Test --targets--> Symbol`   (deterministic, from static analysis)
- `Test --proves--> Symbol`    (assertion-target analysis; LLM-confirmed)
- `Test --proves--> Behavior` and `Behavior --about--> Symbol`
- Derived view: a Symbol with inbound `reaches`/`targets` but **no** `proves`
  is flagged `unproven`.

### Searchable queries this unlocks
- "Public symbols in `<crate>` that are `reached` but never `proven`."
- "Behaviors asserted in the qlora training path; which edge cases are missing."
- "Assertion-free tests (have `reaches` edges, emit no `proves`) — candidate
  no-op tests."
- "Symbols changed in the last commit with zero `proves` edges."

## 4. Phased build (each phase independently valuable; stop after any one)

> Status: **proposal only.** No code in this document; build begins on go.

### Phase 0 — Ingest existing llvm-cov into the graph (deterministic, no LLM)
Parse the `cargo llvm-cov ... --json --summary-only` export already produced in
CI ([`ci.yml`](../../../.github/workflows/ci.yml) line ~566) and attach per-symbol
`reaches` edges. Gives the "touched" baseline to subtract *against*. Cheap.

### Phase 1 — Static test → subject + assertion mapping (deterministic)
A pass (new `vox-test-audit`-style analyzer reusing `vox-ast`, or a graphify
extraction pass) that, per test, resolves imported/called symbols (`targets`) and
traces the value flowing into each `assert*!` / `expect` / golden-compare back to
its originating symbol (first-pass `proves`). Runs over the whole repo today.

### Phase 2 — Semantic `Behavior` extraction (LLM — graphify's core job)
Fan-out agents read each test body + the symbols it asserts on, emit `Behavior`
nodes and `proves` edges with confidence + honesty tags. This turns "`foo` is
tested" into "the *empty-input error path* of `foo` is proven; its *overflow*
case is not." **Per-crate** — run frozen-core crates first (`vox-compiler`,
`vox-db`, `vox-actor-runtime`, `vox-orchestrator`), expand outward. Only
token-cost phase.

### Phase 3 — Map surface + optional gate
Reports/queries (proven-coverage by crate, unproven public API, assertion-free
tests). Optionally a `vox ci semantic-coverage` gate that flags new public
symbols landing with `reaches` but no `proves` — the semantic analog of the
line-coverage gate. Mirrors [`vox-effort-audit`](../../../crates/vox-effort-audit)
shape if promoted to a checked-in crate later.

## 5. Why incremental works here
- The code graph is already built and cached — Phases 0 and 1 are deterministic
  and run over the whole repo with no token cost.
- Phase 2 is naturally per-crate, so coverage of the *map itself* grows crate by
  crate, prioritized by where proof matters most (frozen core first).
- The honesty model (EXTRACTED / INFERRED / AMBIGUOUS) is inherited from
  graphify, so the map never silently invents a `proves` edge.

## 6. Open decisions (for the implementation cycle)
- Promote to a checked-in `vox-test-audit` crate + CI gate, or keep the map
  purely in the graphify graph as an exploration surface? (Current choice: graph
  first; crate/gate deferred.)
- Assertion-target tracing depth in Phase 1 (direct-arg only vs intra-function
  dataflow).
- Behavior granularity in Phase 2 (one per assertion vs one per logical claim).

## 7. Related
- [`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml) — the
  line-coverage gate this strategy complements (not replaces).
- [`crates/vox-effort-audit`](../../../crates/vox-effort-audit) — shape precedent
  for an audit crate + CLI subcommand, if Phase 3 is promoted.
