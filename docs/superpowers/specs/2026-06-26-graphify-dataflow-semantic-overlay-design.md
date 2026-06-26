---
category: "Architecture SSOTs"
title: "Graphify Data-Flow / Def-Use Layer + Semantic Overlay — Design"
date: 2026-06-26
status: design
---

# Graphify Data-Flow / Def-Use Layer + Semantic Overlay — Design

> **Status:** design (not executed). This is the spec the
> [general-enhancement blueprint](2026-06-03-structured-full-search-design.md)
> and `2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md`
> explicitly deferred ("the semantic overlay gets its own spec later"; "no
> data-flow modeling in the structural core"). Both layers are now **in scope**.

## 0. Context and the bug class the call graph cannot see

The enhanced structural Graphify (`crates/vox-graphify-reader/`) is a **call /
composition / boundary graph**: `ExtractedNode{ id, label, kind }` +
`ExtractedEdge{ source, target, confidence }` with `confidence ∈ {resolved,
heuristic, declared}`, plus `cmd:` / `tool:` / `surface:` registry nodes and a
`coverage.rs` classifier (`Surfaced` / `OrphanBackend` / `DeadEnd`). It answers
"who calls / composes / dispatches-to whom."

It **cannot** see the defect in
`docs/superpowers/plans/2026-06-26-frontend-emit-validation-gate.md` (main repo):

> `generate_with_options` *populates*
> `ReactiveViewBridgeStats::reactive_view_emit_failures` (a `Vec<WebIrDiagnostic>`
> error-accumulator) per reactive component, but **never reads that field back**
> to gate the return — it stores the stats into `CodegenOutput` and returns `Ok`
> regardless (`emitter.rs:624`). The collected errors are silently swallowed.

This is a **data-flow defect**: a field is *written* (def) and only ever *read*
to be serialized into the output struct — it never flows into a `Result`, a
branch condition, or an early return. The call graph sees
`generate_with_options → record_failure` and `generate_with_options →
CodegenOutput{…}` as two perfectly ordinary edges. Nothing is structurally
"missing." The defect lives in the *absence* of a read-for-control-flow, which a
call graph has no vocabulary for.

Two complementary layers close two different gaps:

| Layer | Question | Determinism | Catches |
|---|---|---|---|
| **1. Data-flow / def-use** | "is this written value ever *used to decide* anything?" | deterministic (AST def-use, intra-procedural) | dead signals: ignored `Result`, write-only fields, collected-but-unconsumed accumulators (the frontend-emit bug) |
| **2. Semantic overlay** | "what is *related* to X / what is the auth flow?" | non-deterministic (embeddings + LLM labels), **separate overlay graph** | fuzzy relations the structural graph can't express |

The **two-layer principle** is non-negotiable: the deterministic structural core
is never mutated by either new layer. Data-flow adds new deterministic edge kinds
to the same graph family (still honest, still confidence-labeled). The semantic
overlay is a **physically separate** graph, queried separately, and every result
is tagged with its source layer so an agent always knows whether it is reading
ground truth or a guess.

---

## 1. Data-flow / def-use layer

### 1.1 Scope decision — the tractable first cut

Full inter-procedural dataflow (points-to, alias analysis, MIR-level
reaching-definitions across crate boundaries) is a multi-quarter effort and
fundamentally at odds with Graphify's "cheap, never-wrong-by-construction"
character. **We do not attempt it.** The first cut is deliberately narrow:

> **Intra-function (intra-procedural) def-use**, plus a small set of
> **pattern detectors** that fire only on high-confidence, locally-decidable
> shapes — chief among them the *error-accumulator-written-but-never-read-for-
> control-flow* detector that catches the frontend-emit case.

Honesty rule (inherited): if the analysis can't decide a use locally, it **drops
the claim** (no finding) rather than guessing. A missed defect is acceptable; a
false "dead signal" that sends an agent on a wrong hunt is not. Every finding
carries a `confidence` label.

This is implemented as a new module `crates/vox-graphify-reader/src/dataflow.rs`,
re-using the existing `syn` visitor pattern (Rust) and the tree-sitter walk (TS).
It runs **per function** — no whole-program fixpoint.

### 1.2 New nodes and edges

The structural core already emits `fn` and `struct` nodes. Data-flow adds:

**New node kinds** (joined to existing `fn`/`struct` ids; same `graph.json`):

- `field:<Struct>::<field>` — a struct field that participates in def-use
  (created lazily, only for fields the analysis actually touches).
- `binding:<fn-id>::<name>@<n>` — a local binding inside a function (SSA-ish:
  `@n` disambiguates shadowing/re-assignment; intra-function only).

**New deterministic edge kinds** (carry `confidence`, same honesty firewall):

- `def-write` : `<fn-id>` → `field:…` / `binding:…` — this function assigns to
  the field/binding (`x = …`, `self.x = …`, `x.push(…)`, `x.0 = …`).
- `use-read` : `<fn-id>` → `field:…` / `binding:…` — this function reads the
  value **for a non-trivial purpose**, sub-typed by `read_kind`:
  - `read_kind: "control"` — read feeds a branch / loop / `match` scrutinee /
    `if` condition / `?` / early `return cond`. **This is the load-bearing one.**
  - `read_kind: "consume"` — read passed to a function call or `return`ed (value
    leaves the function meaningfully).
  - `read_kind: "store"` — read only to copy the value into *another* aggregate
    (e.g. `CodegenOutput { reactive_stats }`). **A `store`-only read does NOT
    count as "consumed for control flow."** This distinction is the entire crux.

Edges are confidence-labeled exactly as today: a write/read we can pin to a
concrete binding is `resolved`; one reached through a method we can't model
(e.g. interior mutability via a trait object) is dropped, not downgraded.

### 1.3 The dead-signal detectors (pattern layer)

On top of the def-use edges, `dataflow.rs` runs a fixed set of **local
detectors**. Each emits a `DeadSignal` finding only when it can decide locally.

1. **`ignored_result`** — a call returning `Result<_,_>` / `Option<_>` whose
   value is dropped (statement-expression, `let _ =`, or `;`-terminated) with no
   `?`, `.unwrap()`, `.expect()`, `match`, `if let`, or assignment. (Complements
   clippy's `unused_must_use` but is graph-queryable and cross-references the
   call edge.) Confidence `heuristic` (we can't always see the return type
   without resolution → drop when the callee type is unknown).

2. **`write_only_field`** — a `field:` node with ≥1 `def-write` edge and **zero
   `use-read` edges of any kind** across the whole graph. Confidence `resolved`
   (this one is whole-graph but purely structural: count edges).

3. **`accumulator_never_gates`** (the frontend-emit detector) — a `field:` or
   `binding:` node that:
   - has ≥1 `def-write` edge whose write-site is an **accumulation** op
     (`.push`, `.extend`, `.insert`, `+=`, `*_or_default().push`), AND
   - has **zero `use-read` edges with `read_kind: "control"`**, AND
   - has ≥1 `use-read` edge with `read_kind: "store"` or `"consume"`.

   In English: *"this thing collects errors/items, and that collection flows out
   to a return value or another struct, but is never read to decide a branch or
   early-return."* Confidence `heuristic` — the canonical shape of a swallowed
   error-accumulator. This is the bug class the call graph is blind to.

A `DeadSignal` finding carries:

```rust
pub struct DeadSignal {
    pub detector: DeadSignalKind,   // IgnoredResult | WriteOnlyField | AccumulatorNeverGates
    pub node_id: String,            // field:/binding: id
    pub label: String,
    pub write_sites: Vec<String>,   // fn-ids that def-write it
    pub read_kinds: Vec<String>,    // distinct read_kind values observed ("store", …)
    pub confidence: String,         // resolved | heuristic
    pub rationale: String,          // human-readable: why it fired
}
```

### 1.4 New coverage / lint output: "dead signals"

`coverage.rs` gains a sibling report in `dataflow.rs`:

```rust
pub struct DeadSignalReport { pub findings: Vec<DeadSignal> }
pub fn compute_dead_signals(graph: &Value) -> DeadSignalReport;
```

It mirrors `compute_coverage`'s honesty firewall: it reports **what the def-use
edges say**, makes no judgment about whether the swallow is intentional, and
labels confidence so an agent (or a CI gate) can choose its threshold. A future
`vox graphify dead-signals --corpus <c>` CLI subcommand and a non-blocking CI
advisory (mirroring the existing coverage advisory) surface it; a team can later
promote `AccumulatorNeverGates @ resolved` to blocking once the false-positive
rate is measured. (This is exactly the gate the frontend-emit plan adds *by hand*
for one field — the dead-signal report would have *found* it.)

### 1.5 Frontend-emit walkthrough — would the detector flag it? **YES.**

Trace `accumulator_never_gates` over `generate_with_options`:

1. **Field node created.** `ReactiveViewBridgeStats` is a `struct` node;
   `reactive_view_emit_failures: Vec<WebIrDiagnostic>` → `field:ReactiveViewBridgeStats::reactive_view_emit_failures`.

2. **`def-write` (accumulation).** Inside the per-component loop, the production
   emitter records each blocking diagnostic via
   `reactive_stats.reactive_view_emit_failures.push(diag)` (the `reactive/view.rs`
   path that populates the field). The visitor sees a `.push` on a path rooted at
   the local `reactive_stats` binding whose type is the struct →
   `def-write` edge, write-site = `generate_with_options`, op = accumulation. ✔

3. **`use-read` classification.** The only other mention of the field in the
   function is at the `Ok(CodegenOutput { … reactive_stats … })` construction
   (`emitter.rs:624`): the binding is **moved into another aggregate**. The
   visitor classifies this as `read_kind: "store"` (value copied into a struct
   literal), **not** `"control"`. ✔

4. **Control-read check.** Scan all `use-read` edges for this node with
   `read_kind == "control"`. There are **none** — the field is never the
   scrutinee of an `if` / `match`, never `?`-propagated, never compared to
   `is_empty()` to gate the return. ✔

5. **Detector fires.** Has accumulation `def-write` ✔, zero `control` reads ✔, has
   a `store` read ✔ → **`AccumulatorNeverGates`**, confidence `heuristic`,
   rationale: *"reactive_view_emit_failures is populated via .push and flows only
   into CodegenOutput (store); never read to gate a branch or return."*

**Verdict: the def-use detector flags `reactive_view_emit_failures` as
written-but-never-read-for-control-flow.** It is the textbook positive case for
`accumulator_never_gates`. The frontend-emit plan fixes exactly this field by
hand; the dead-signal report finds the *class* — every other "collected and
swallowed" accumulator in the codebase surfaces the same way.

**Honest limits of the first cut** (stated, not hidden):
- It is **intra-procedural**. If the `.push` happened in a *callee* and the field
  flowed back through a return, the local visitor would not connect them →
  dropped (no false finding, but a possible miss). The frontend-emit case is
  safe because both the writes and the store are textually inside (or inlined
  into) the same function's reactive loop + return.
- Interior mutability through a trait object or a macro-generated setter is not
  modeled → dropped.
- `read_kind` classification is conservative: anything we can't confidently call
  `control` is **not** counted as control (so we never *miss-clear* a real
  dead signal by mistakenly seeing a control read).

---

## 2. Semantic overlay (separate layer)

### 2.1 Principle — a distinct overlay graph, never mutating the core

The semantic layer answers fuzzy questions the structural graph cannot:
"find things related to X", "what is the auth flow", "which surfaces are *about*
billing." It is **embeddings + LLM-labeled relations**, both non-deterministic.

It is stored as a **separate artifact** — `semantic-overlay.json` alongside
`graph.json`, never merged in. Every overlay node/edge references structural node
ids but lives in its own file and its own reader. An agent querying the overlay
gets results explicitly stamped `layer: "semantic"` with a `source` and a
`similarity`/`confidence`; a query against the structural core is stamped
`layer: "structural"`. The contract: **an agent always knows which layer a result
came from.** LLM-guessed edges are *never* written back into `graph.json`.

### 2.2 Embeddings — reuse Vox Search, do not duplicate

The repo already has an embedding primitive (`vox-actor-runtime/src/llm/embed.rs:
llm_embed`) and a live unified search surface (`vox-search`, the
`2026-06-03-structured-full-search-design.md` corpus model: `SearchCorpus`,
`UnifiedHit`, `execute_search_plan`). The overlay **does not build its own
embedding stack.** Coordination with the Vox-Search fusion design:

- **Corpus = graph nodes + their docs.** Add a new `SearchCorpus::GraphifyNodes`
  (or feed nodes as documents into the existing chunk/knowledge corpus) whose
  unit-of-retrieval is a node's `{label, kind, module path, doc-comment,
  surrounding identifiers}`. Embeddings are computed via the *same* `llm_embed`
  pipeline and stored in the *same* vector store (Qdrant, where present) under a
  distinct collection — so "semantic graphify" rides on Vox-Search's infra, not a
  parallel one.
- **`vox_graphify_semantic_related` is a thin adapter** over
  `execute_search_plan` with `corpora = [GraphifyNodes]`, mapping `UnifiedHit`
  back to structural node ids. It inherits Vox-Search's honesty constraints
  (group-by-source, weak cross-corpus comparability → present similarity, never a
  single authoritative rank).
- **No line locators** (per the search design's constraint 1) — semantic results
  resolve to a node id → file, not a line.

### 2.3 LLM-labeled relations — the overlay graph

Beyond similarity, the overlay can carry **typed, LLM-asserted relations**
("`AuthView` *implements* the login flow described in `auth.md`"; "`billing_cmd`
and `invoice_cmd` are *alternatives*"). These are produced offline, batched, and:

- written **only** to `semantic-overlay.json`, each as
  `{ source, target, relation, model, confidence, evidence }`;
- **never** promoted into the structural graph;
- regenerated, not incrementally trusted — a stale overlay is dropped on rebuild
  (it carries the structural `graph_json_sha256` it was built against; mismatch →
  the overlay is marked stale and queries warn, mirroring the existing freshness
  model in `vox-config` / `lexical_lag`).

### 2.4 The capability

- *"Find things semantically related to X"* → `vox_graphify_semantic_related`
  (embedding kNN over node corpus).
- *"What's the auth flow?"* → embedding retrieval seeds + 1–2-hop structural
  expansion (`bfs_from_seeds`) to ground the fuzzy hit in real call/composition
  edges, returned as a **mixed** result with each edge stamped by its layer
  (structural edges = ground truth; semantic seed = guess). This is the honest
  realization of "relate any feature to any other": the *seeds* are semantic, the
  *connective tissue* is the deterministic core.

---

## 3. Tool surface (reserved names + schemas)

Existing tools (in `vox-orchestrator-mcp/src/dispatch.rs` +
`graphify_tools.rs`): `vox_graphify_status`, `vox_graphify_search`,
`vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare`. The new layers
**reserve** these names (coordinate final wiring with the agent-tool-surface
design — separate agent):

| Tool | Layer | Input (sketch) | Output | Notes |
|---|---|---|---|---|
| `vox_graphify_dataflow` | data-flow | `{ corpus, node_id }` (a `fn`/`field`/`binding` id) | def-use edges (`def-write` / `use-read` w/ `read_kind`) incident to the node | deterministic; `layer: "structural"` |
| `vox_graphify_dead_signals` | data-flow | `{ corpus, detector?: "ignored_result"\|"write_only_field"\|"accumulator_never_gates", min_confidence?: "heuristic"\|"resolved" }` | `DeadSignalReport.findings[]` | the lint output; the frontend-emit bug shows here |
| `vox_graphify_semantic_related` | semantic | `{ corpus, query \| node_id, k?, min_similarity? }` | ranked node ids + `similarity` + `source` | `layer: "semantic"`; thin adapter over `execute_search_plan` |

Every response object carries `layer` (`"structural"` | `"semantic"`) and, for
semantic, a `stale: bool` from the overlay-vs-core sha check. Input schemas land
in `vox-orchestrator-mcp/src/input_schemas.rs` next to the existing graphify
entries. CLI mirrors: `vox graphify dataflow`, `vox graphify dead-signals`,
`vox graphify semantic-related` (the dead-signals one also runs as a non-blocking
CI advisory, like coverage today).

---

## 4. Sequencing

**Recommendation: data-flow first, semantic second.** Rationale:

1. **Data-flow is concrete and self-contained.** It catches real, shippable bugs
   (the frontend-emit swallow and its whole class) using infra already in the
   crate (`syn` visitor, tree-sitter walk, the `coverage.rs` report pattern). No
   new external dependency, fully deterministic, testable with the same fixture
   style as `coverage.rs`. It extends the structural core's value immediately and
   keeps the "never hallucinates" trust intact.

2. **Semantic has a hard upstream dependency.** It should ride on the Vox-Search
   embedding stack (`llm_embed` + Qdrant + `execute_search_plan`) rather than
   spawning a parallel one. That stack's enhancements (P1–P4 in the
   `2026-06-03` search design — highlights, locators, facets) are the natural
   place to add the `GraphifyNodes` corpus. Building semantic *before* the
   `GraphifyNodes` corpus exists in Vox-Search means duplicating embedding infra
   — exactly the anti-goal. So semantic waits for (a) the data-flow layer to
   prove the two-layer discipline on something deterministic, and (b) the
   Vox-Search corpus hook.

**Order:**

1. **Data-flow layer** (`dataflow.rs`: def-use edges + 3 detectors + `compute_dead_signals` + `vox_graphify_dataflow` / `vox_graphify_dead_signals` tools + CI advisory). Validate against the frontend-emit fixture end-to-end.
2. **Vox-Search `GraphifyNodes` corpus** (coordinate with the structured-full-search spec) — the embedding seam.
3. **Semantic overlay** (`semantic-overlay.json` writer/reader, freshness sha, `vox_graphify_semantic_related` adapter, mixed seed-then-structural-expand query). Built only after 2.

### Key forks (decisions for ratification)

- **Tractable-dataflow scope.** Intra-procedural def-use + 3 local detectors,
  honesty-drop on anything non-local. **Fork:** accept the known miss (writes that
  happen in a callee and flow back via return are not connected) in exchange for
  zero false "dead signal" findings and a cheap, `syn`-only implementation —
  versus a heavier inter-procedural pass (rejected for the first cut). The
  frontend-emit case is caught under the cheap scope.
- **Semantic now vs after-search.** Build the semantic overlay **after** the
  Vox-Search `GraphifyNodes` corpus exists, reusing `llm_embed`/Qdrant — versus
  standing up a standalone embedding stack now (rejected: duplicates infra,
  violates "coordinate with the Vox-Search fusion design"). Cost: semantic lands
  later; benefit: one embedding pipeline, one honesty model.
