---
title: "Semantic Proximity, Split-Brain Detection, and Safe Symbol Surfacing (Research 2026)"
description: "How to programmatically detect conceptually related but divergently named code, surface proximity hints without causing LLM hallucination, and build a discovery layer for semantic drift in AI-native codebases."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-17"
training_eligible: false
training_rationale: "Foundational for the MENS corpus quality flywheel and vox-search symbol-proximity layer. Directly reduces LLM hallucination risk from identifier-conflation and split-brain naming drift."
see_also:
  - rag-and-research-architecture-2026.md
  - research-ts-hallucination-k-complexity-2026.md
  - research-cl-oracle-semantic-drift-2026.md
  - research-language-alias-canonicalization-2026.md
  - multi-repo-context-isolation-research-2026.md
  - research-llm-output-mediation-validation-2026.md
  - orphan-surface-inventory.md
  - nomenclature-migration-map.md
  - language-surface-ssot.md
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Semantic Proximity, Split-Brain Detection, and Safe Symbol Surfacing (Research 2026)

## 1. Problem Statement

Modern codebases — especially ones developed with agentic AI assistance — accumulate a specific class of structural debt: **split-brain naming divergence**. This is the state where two or more functions, types, or concepts:

- Are **semantically related** (same concept, overlapping intent),
- Have **divergent names** that do not obviously signal their relationship,
- Were created independently by different agents or contributors at different times, and
- **Neither LLM agents nor human reviewers** routinely notice the duplication because no single diff ever showed both sides.

Classic examples from the Vox/FableForge corpus (documented in prior sessions):

| Left (older) | Right (newer) | Relationship |
|---|---|---|
| `resolveArenaRound` | `combatRoundResolver` | Functionally equivalent entry points |
| `recall()` | `recall_async()` | Deprecated sync vs. current async |
| `persist_fact()` | `sync_to_db()` | Renamed during migration, both exist |
| `vox-dei` | `vox-orchestrator` | Retired crate vs. canonical replacement |
| `ClassRegistry` (FableForge) | `classRegistry` (camelCase variant) | JS module fragmentation |

The hazard compounds with LLMs: an agent asked to "call the function that starts combat" may hallucinate a third name that sounds plausible but maps to neither, or — worse — may assume both names refer to the same entity and write code that calls both, causing double-execution bugs.

This document synthesizes 2025–2026 research on:
1. **Detecting** split-brain naming drift automatically,
2. **Surfacing** semantic proximity hints safely (without triggering conflation),
3. **Resolving** confirmed duplicates through principled canonicalization, and
4. **Architecturally grounding** these capabilities in Vox's existing `vox-search`, `vox-compiler`, and MENS pipelines.

---

## 2. Taxonomy of the Problem Space

### 2.1 Clone Types (Industry Classification)

The code-clone detection literature uses a four-type taxonomy:

| Type | Description | Detection Difficulty | Vox Relevance |
|---|---|---|---|
| **Type 1** | Exact copy, possibly different whitespace/comments | Low | Low (CI catches these) |
| **Type 2** | Renamed identifiers only; structure identical | Medium | Medium (renames across refactors) |
| **Type 3** | Modified copies: added/deleted statements | High | High (split agents diverge incrementally) |
| **Type 4** | Semantically equivalent, syntactically distinct | Very High | **Critical** (our primary concern) |

Vox's split-brain problem is primarily **Type 2 through Type 4**. Type-1 is caught by existing linters and the `arch/duplication` TOESTUB gate.

### 2.2 The "Knowledge Conflating Hallucination" (KCH) Risk

Research from 2025 (arxiv, ACL Anthology) identifies a distinct LLM failure mode called **Knowledge Conflating Hallucination** (KCH): when two similar-but-different entities exist in training data or context, an LLM may:

- Blend properties of both into a single hallucinated description,
- Invoke one while believing it called the other,
- Generate code that imports/calls a name that combines features of both (non-existent).

This is distinct from the more commonly studied "factual hallucination" and is specifically triggered by **near-duplicate names in the same semantic neighborhood**. The risk is highest when:
- Names share 2+ morphemes (e.g., `resolveArena` vs. `arenaResolver`),
- Docstrings or comments describe overlapping behaviors,
- Both appear in the same context window.

### 2.3 The Discovery Gap

The inverse risk — the **discovery gap** — occurs when genuinely related functions are named so differently that an LLM agent fails to find existing work and writes redundant code. This is the "new ticket, new wheel" problem:

> An agent implementing `CharacterPersistence.saveProgress()` does not find the existing `DiscoveryRegistry.serializeAreaCompletion()` because the names share no tokens, yet both write character state to the database.

Both risks (conflation and invisibility) stem from the same root cause: **symbol names are not semantically indexed relative to each other**.

archived_date: 2026-04-18
---

## 3. Detection Techniques — State of the Art (2025–2026)

### 3.1 AST + Structural Embedding (Hybrid Approach)

The current industry standard is a two-stage pipeline:

**Stage 1 — Structural Filtering (fast, O(n) over symbol table):**
- Parse all source files with a grammar-aware parser (Tree-sitter is preferred in 2026 for Rust + TypeScript simultaneously).
- Extract a normalized **function signature vector**: `(arity, param_types, return_type, depth, complexity_estimate)`.
- Cluster via LSH (Locality-Sensitive Hashing) or MinHash to find structurally proximate candidates in sub-linear time.

**Stage 2 — Semantic Reranking (precise, O(k²) over cluster members):**
- For each cluster from Stage 1, embed the function body using a code-specialized model (e.g., Jina Code Embeddings, a Qwen-based code embedder, or a fine-tuned CodeBERT derivative).
- Use cosine similarity to surface pairs with similarity in [0.65, 0.95] — the "proximity zone."
- The upper bound (< 1.0) is crucial: pairs with similarity ≥ 0.96 are likely exact duplicates (handled separately). The lower bound (> 0.65) prevents false positives from accidentally similar signatures.

**Safe Surfacing Rule (Critical for Anti-Hallucination):**
Results must always be framed as **candidates for human/agent review**, never as assertions of identity:

```
PROXIMITY HINT [confidence=0.81]:
  `resolveArenaRound` (combat/resolver.ts:42) and
  `combatRoundResolver` (engine/round.ts:17)
  share high semantic overlap. They may be:
    (a) Intended duplicates — consider consolidating,
    (b) Related but distinct — consider adding cross-references in docs,
    (c) False positive — verify and dismiss.
  DO NOT assume these are the same function without reading both implementations.
```

This framing prevents KCH by explicitly prohibiting conflation while still promoting discovery.

### 3.2 Identifier-Morpheme Distance

A lightweight complement to embedding-based search. Instead of embedding entire functions, compute **edit-distance and morpheme-overlap** between bare symbol names:

```
Levenshtein("resolveArenaRound", "arenaRoundResolver") = 6
Token overlap (camelCase split): {"resolve","arena","round"} ∩ {"arena","round","resolver"} = 2/3
```

This catches renamed-and-reshuffled identifiers that pure semantic embedding may miss (because embedding aggregates body meaning, not name meaning). Best paired with the structural embedding approach above.

**Key implementation note:** Normalize identifiers by:
- Splitting on camelCase, snake_case, and kebab-case boundaries,
- Stemming each token (e.g., "resolver" → "resolve"),
- Computing Jaccard similarity on the resulting token sets.

### 3.3 Call-Graph and Dependency Proximity

Two functions that are **never called from the same callsite** but share callers at depth ≤ 2 are likely related. Building a lightweight call-graph index (feasible with Tree-sitter or rust-analyzer for Rust) enables:

- **Sibling detection**: functions with the same parent callers,
- **Interface surface matching**: functions that appear in the same trait/interface across implementations,
- **Import co-occurrence**: TypeScript modules that are consistently imported together.

This graph-structural signal is low-noise (structural, not probabilistic) and can be used as a gating filter before running expensive embedding comparisons.

### 3.4 Doc-Comment and Changelog Mining

Most well-written functions carry a docstring or changelog entry mentioning what they *replaced* or what they are *similar to*. Mining this prose with lightweight NLP (keyword extraction, not LLM inference) surfaces explicit relationships the code itself doesn't encode:

```
/// Like `persist_fact()` but async-safe. Prefer this over the deprecated synchronous version.
pub async fn sync_to_db(fact: &Fact) -> Result<()> { ... }
```

The phrase "Like `X`" is a structured proximity signal. A simple regex pass over all doc comments can build an explicit **similarity edge list** that costs zero embedding inference.

---

## 4. Safe Surfacing Architecture — The "Proximity Hint" Layer

The goal is to surface candidate relationships to LLM agents and human developers **without causing the agent to assume identity**. The key design constraint is:

> **A proximity hint must be structured so that treating two items as identical is never the lowest-effort action.**

### 4.1 Proposed Data Model

```rust
/// A candidate pair of semantically proximate symbols.
/// NOT a claim of functional equivalence — investigation is required.
struct ProximityCandidate {
    /// Fully qualified path of the first symbol.
    left: SymbolRef,
    /// Fully qualified path of the second symbol.
    right: SymbolRef,
    /// Combined confidence score [0.0, 1.0]. Scores below 0.65 are not surfaced.
    confidence: f64,
    /// Which detection signals contributed to this score.
    signals: Vec<ProximitySignal>,
    /// Conservative label; never claims identity.
    verdict: ProximityVerdict,
}

enum ProximitySignal {
    SemanticEmbedding { cosine_sim: f64 },
    IdentifierMorpheme { jaccard: f64 },
    CallGraphSibling { shared_callers: usize },
    DocCommentReference { snippet: String },
    StructuralSignature { score: f64 },
}

enum ProximityVerdict {
    /// Likely the same concept under two names — investigate consolidation.
    PotentialDuplicate,
    /// Related but probably distinct — add cross-references.
    RelatedSurfaces,
    /// Similar structure, different domain — low action priority.
    StructuralCousin,
}
```

### 4.2 Surfacing Channels

Results should be surfaced through multiple channels with different urgency levels:

| Channel | Trigger | Audience | Action |
|---|---|---|---|
| `vox check --proximity` | Manual or CI | Agent + Human | Review list; decide verdict |
| Socrates annotation | LLM context injection | Agent only | Prefixes symbol mention with disambiguation note |
| LSP hover hint | IDE integration | Human only | Tooltip: "See also: ..." |
| MENS DPO negative lane | Training data | Model | Teaches model NOT to conflate these pairs |
| PR review annotation | On symbol-touching diffs | Human + CodeRabbit | Flag when diff touches only one of a known pair |

### 4.3 The Socrates Disambiguation Annotation

When an agent queries for a symbol that has known proximity candidates, the Socrates policy layer should **prepend a structured disambiguation note** to the context before the agent generates code:

```
[SOCRATES DISAMBIGUATION]
You referenced `resolveArenaRound`. Note: a semantically related symbol exists:
  → `combatRoundResolver` at engine/round.ts:17 (proximity=0.81, verdict=PotentialDuplicate)
These are NOT confirmed to be the same function. Read both before calling either.
Do NOT use the name of one as a substitute for the other in generated code.
```

This pattern prevents KCH by explicitly deactivating the low-effort "they're the same" shortcut before the generation window opens.

archived_date: 2026-04-18
---

## 5. Resolution — What to Do With Confirmed Duplicates

Detecting proximity is the discovery phase. Resolution requires a principled decision tree:

```
Confirmed pair detected
        │
        ├── [Identical behavior, different names]
        │       → Deprecate one; alias to canonical in the other
        │       → Add entry to nomenclature-migration-map.md
        │       → Add deprecated symbol to AGENTS.md "Retired Surfaces" table
        │
        ├── [Related but distinct behavior]
        │       → Add mutual `see_also` doc-comments to both
        │       → Register as a "related pair" in the proximity registry
        │       → Consider extracting shared sub-logic into a named abstraction
        │
        ├── [Similar structure, different domain]
        │       → Low priority; add a cross-domain note
        │       → Consider whether a shared trait/interface would be beneficial
        │
        └── [False positive]
                → Dismiss with explicit annotation:
                  `// PROXIMITY-DISMISS: not related to <other_symbol> despite name similarity`
                → This annotation suppresses future false-positive surfacing
```

### 5.1 The Canonicalization Chain

When deprecating one symbol in a confirmed duplicate pair:

1. **Mark the retired symbol** in `AGENTS.md § Retired Surfaces` with its canonical replacement.
2. **Add a migration entry** to `docs/src/architecture/nomenclature-migration-map.md`.
3. **Generate a negative DPO pair** for the MENS training corpus: `(prompt that uses old name, bad completion using old name, good completion using new name)`.
4. **Run `vox ci clavis-parity`** if the deprecated surface touches secrets.
5. **Add `PROXIMITY-RESOLVED` annotation** at the retired symbol's call site to suppress future alerts.

---

## 6. Integration with Vox's Existing Architecture

### 6.1 `vox-search` — New `SymbolProximity` Corpus

The existing `vox-search` RAG pipeline already manages multiple corpora (Memory, KnowledgeGraph, DocumentChunks, TantivyDocs, Qdrant). The proximity detection capability maps cleanly onto this architecture as a new corpus type:

| New Corpus | Backend | Feature Gate | Source Crate |
|---|---|---|---|
| `SymbolProximity` | Tantivy (morpheme index) + Qdrant (code embeddings) | `symbol-proximity` feature | `vox-search/symbol_proximity.rs` [PLANNED] |

The corpus is queried when:
- A symbol name appears in an agent query,
- A diff touches a file containing a known high-confidence proximity pair,
- `vox check --proximity` is invoked manually.

### 6.2 `vox-compiler` — Proximity Annotations in HIR

The Vox compiler's HIR (High-level IR) phase is the right place to attach proximity metadata as a compiler-time annotation, analogous to deprecation warnings:

```
warning[P001]: symbol `resolveArenaRound` has a high-proximity sibling
  --> combat/resolver.ts:42
  |
  | fn resolveArenaRound(...) { ... }
  |
  = note: `combatRoundResolver` at engine/round.ts:17 (proximity=0.81)
  = help: verify these are distinct before using both; use `// PROXIMITY-DISMISS` to suppress
```

This is a **non-fatal warning** in `vox check` mode and a **silent annotation** in normal compilation, preventing friction for well-understood code while surfacing drift for new code.

### 6.3 MENS Corpus — Anti-Conflation DPO Pairs

Proximity-detected pairs feed directly into the MENS continual learning flywheel as **negative DPO examples**:

```jsonc
// Bad completion (conflates two distinct functions)
{
  "prompt": "Call the function that starts a combat round",
  "chosen": "// These are distinct. Use resolveArenaRound for arena-mode combat\n// and combatRoundResolver for the unified round pipeline.\nresolveArenaRound(ctx);",
  "rejected": "// Just use either one, they do the same thing\ncombatRoundResolver(ctx);"
}
```

This creates a training signal that explicitly teaches the model to **treat proximity hints as disambiguation prompts, not as permission to conflate**.

See [MENS Synthetic Corpus Limitations](mens-synthetic-corpus-limitations-research-2026.md) §4.2 for the anti-conflation lane design.

### 6.4 `vox ci` — New `proximity-drift` Gate

A new CI gate `vox ci proximity-drift` should:
1. Run the full symbol-proximity scan on the current working tree.
2. Compare against the last committed proximity report (stored at `contracts/proximity/snapshot.v1.json`).
3. Fail if **new high-confidence pairs** (confidence ≥ 0.80) have appeared that are not yet triaged.
4. Warn (non-failing) for medium-confidence pairs (confidence ∈ [0.65, 0.80)).

This creates a **drift ratchet**: the number of untriaged high-confidence proximity pairs can only decrease over time.

archived_date: 2026-04-18
---

## 7. Vox-Specific Gaps Identified

Cross-referencing this research against the existing Vox codebase and documentation reveals the following gaps:

### Gap 1 — No Symbol Proximity Corpus in `vox-search` [HIGH]

The `rag-and-research-architecture-2026.md` documents 8 corpora; none targets intra-codebase symbol proximity. The `RepoInventory` corpus does path scanning but not semantic symbol comparison.

**Action:** Add `vox-search/symbol_proximity.rs` — see §6.1.

### Gap 2 — `nomenclature-migration-map.md` Is Manually Maintained [HIGH]

The existing [nomenclature migration map](nomenclature-migration-map.md) tracks retired symbol→canonical pairs, but it is populated manually. There is no automated scan to detect new pairs that should be added.

**Action:** `vox ci proximity-drift` should produce candidate additions to this map.

### Gap 3 — Socrates Has No Disambiguation Annotation Path [MEDIUM]

The Socrates policy (`vox-socrates-policy`) handles confidence, abstention, and research escalation, but has no mechanism to prepend disambiguation notes when an agent query touches a known proximity pair.

**Action:** Add a `DisambiguationHint` variant to `RiskDecision` that is emitted when a queried symbol has proximity candidates above threshold.

### Gap 4 — AGENTS.md "Retired Surfaces" Is Not Machine-Readable [MEDIUM]

The AGENTS.md retired-surfaces table is human-readable markdown. There is no machine-readable contract file that `vox-search` or `vox-compiler` can consume to suppress proximity warnings for already-canonicalized pairs.

**Action:** Add `contracts/proximity/retired-surfaces.v1.json` as the machine-readable companion to the AGENTS.md table. Gate with `vox ci clavis-parity`.

### Gap 5 — No MENS Anti-Conflation DPO Lane [MEDIUM]

The MENS corpus pipeline (documented in [mens-corpus-implementation-plan-2026.md](mens-corpus-implementation-plan-2026.md)) has a DPO lane but no lane specifically targeting KCH (Knowledge Conflating Hallucination) pairs.

**Action:** Add a `lane_kch_anticonflation` mix-config entry that auto-generates negative DPO pairs from the proximity snapshot.

### Gap 6 — `orphan-surface-inventory.md` Has No Proximity Dimension [LOW]

The [orphan surface inventory](orphan-surface-inventory.md) tracks surfaces with no callers or docs. It does not cross-reference proximate but disjoint surfaces (callers in non-overlapping call graphs).

**Action:** Merge proximity signals into the orphan inventory as an additional column.

---

## 8. Factual Error Audit — Existing Documentation

The following potential factual errors or stale claims were identified by cross-referencing this research with current web sources and existing docs:

### 8.1 `rag-and-research-architecture-2026.md` §2.1 — `KnowledgeGraph` corpus

**Claim:** "KnowledgeGraph — SQLite FTS5 node queries"

**Issue:** The document implies the KnowledgeGraph corpus uses semantic graph traversal, but FTS5 is purely lexical. For symbol proximity, this corpus is insufficient — it will miss Type-3 and Type-4 clones entirely.

**Correction:** The KnowledgeGraph corpus provides structural traversal (what depends on what) but does NOT provide semantic proximity. Any proximity detection must layer Qdrant vector search on top, not rely on FTS5 alone. The document should clarify this distinction to prevent agents from over-trusting the KnowledgeGraph corpus for semantic tasks.

### 8.2 `research-ts-hallucination-k-complexity-2026.md` — Identifier Bias Claim

No factual error found, but this document should explicitly cross-reference the **KCH (Knowledge Conflating Hallucination)** taxonomy documented here, as identifier bias is one of its primary mechanistic drivers.

### 8.3 `nomenclature-migration-map.md` — Completeness

This document contains migration entries for known retired symbols but does not document the **detection process** by which new pairs are identified. Without a documented process, the map will fall behind the codebase. This is a **process gap** rather than a factual error but should be noted.

### 8.4 `AGENTS.md` § Retired Surfaces Table — `recall()` Entry

**Claim:** `recall()` → `recall_async()` (retired/canonical pair)

**Issue:** The table does not document the **call signature differences** between the two. If an agent finds `recall_async()` in search results, it may construct an incorrect call signature by analogy with `recall()`. The table entry should include the canonical call signature.

archived_date: 2026-04-18
---

## 9. Implementation Roadmap

### Wave 0 — Machine-Readable Contracts (1–2 days)

- [ ] Create `contracts/proximity/retired-surfaces.v1.json` (machine-readable companion to AGENTS.md table).
- [ ] Create `contracts/proximity/snapshot.v1.json` (empty initial proximity snapshot).
- [ ] Add schema definitions to `contracts/` alongside existing schemas.

### Wave 1 — Identifier-Morpheme Scanner (3–5 days)

- [ ] Implement `vox-search/symbol_proximity.rs` — morpheme-distance pass only (no embeddings yet).
- [ ] Add `vox check --proximity` CLI subcommand that runs the scanner and outputs candidates.
- [ ] Wire `vox ci proximity-drift` gate against the snapshot contract.

### Wave 2 — Semantic Embedding Layer (1 week)

- [ ] Add code-embedding pass to the proximity corpus using the existing Qdrant integration.
- [ ] Tune the `[0.65, 0.95]` proximity zone against a golden set of known Vox duplicate pairs.
- [ ] Emit `ProximityCandidate` structs with `signals: Vec<ProximitySignal>`.

### Wave 3 — Socrates Disambiguation Integration (3–5 days)

- [ ] Add `DisambiguationHint` to `vox-socrates-policy` `RiskDecision` enum.
- [ ] Wire proximity lookup into the Socrates pre-response annotation path.
- [ ] Write test fixtures for KCH pair disambiguation prompts.

### Wave 4 — MENS DPO Anti-Conflation Lane (1 week)

- [ ] Add `lane_kch_anticonflation` to the MENS corpus mix config.
- [ ] Implement generator that produces `(prompt, chosen, rejected)` from confirmed proximity pairs.
- [ ] Validate lane output against the `MENS PR checklist` at `mens-llm-pr-checklist.md`.

### Wave 5 — IDE and LSP Integration (2 weeks)

- [ ] Wire proximity candidates into the VS Code extension's hover provider.
- [ ] Add CodeRabbit annotation hook for PRs that touch only one member of a known pair.

---

## 10. Key Findings Summary

| Finding | Confidence | Implication |
|---|---|---|
| Split-brain naming drift is primarily Type-3/Type-4 clones | High | Lexical dedup (CI) is insufficient; semantic embedding is required |
| KCH (conflation hallucination) is specifically triggered by near-duplicate context | High | Disambiguation must be injected before generation, not after |
| Identifier-morpheme distance is a cheap, high-signal pre-filter | High | Implement before embedding for O(n) scaling |
| The `[0.65, 0.95]` cosine similarity zone is the productive "proximity zone" | Medium | Below 0.65 = noise; above 0.95 = exact duplicates (different tooling) |
| Proximity hints must frame candidates as "investigate", never "same as" | High | All surfacing UI must enforce this anti-conflation framing |
| Doc-comment mining (`// Like X`) is zero-cost and high-recall for explicit cross-refs | High | Add to Wave 1 scanner before embedding |
| Vox's existing `vox-search` RRF pipeline is the right integration host | High | `SymbolProximity` as a new corpus in the existing architecture |
| A CI proximity ratchet (`vox ci proximity-drift`) prevents accumulation of untriaged pairs | High | Implement alongside the scanner in Wave 1 |

archived_date: 2026-04-18
---

## 11. Works Cited and Cross-References

### Internal (Vox docs)
- [RAG and Research Architecture 2026](rag-and-research-architecture-2026.md)
- [Nomenclature Migration Map](nomenclature-migration-map.md)
- [Orphan Surface Inventory](orphan-surface-inventory.md)
- [Language Alias Canonicalization Research 2026](research-language-alias-canonicalization-2026.md)
- [LLM Hallucination K-Complexity Strategies](research-ts-hallucination-k-complexity-2026.md)
- [The Compile-Pass Oracle and Semantic Drift](research-cl-oracle-semantic-drift-2026.md)
- [MENS Synthetic Corpus Limitations 2026](mens-synthetic-corpus-limitations-research-2026.md)
- [MENS Corpus Implementation Plan 2026](mens-corpus-implementation-plan-2026.md)
- [LLM Output Mediation and Validation 2026](research-llm-output-mediation-validation-2026.md)
- [Multi-Repo Context Isolation 2026](multi-repo-context-isolation-research-2026.md)
- [MENS LLM PR Checklist](mens-llm-pr-checklist.md)

### External Research (2025–2026)
- *HyClone*: Two-stage hybrid clone detection (LLM screening + execution-based verification). ResearchGate, 2025.
- *SeqCoBench*: Benchmark for functional equivalence across semantic transformations. ACL Anthology, 2025.
- *ClassEval-Obf*: Identifier-obfuscation benchmark revealing LLM identifier bias. 2025.
- Matryoshka Representation Learning (MRL) for variable-length code embeddings. Jina AI, 2025.
- Qdrant hybrid search (dense + sparse, native RRF fusion). [qdrant.tech](https://qdrant.tech), 2025.
- Tantivy + LanceDB for embedded (in-process) hybrid search in Rust. 2025.
- Sourcegraph Cody: Hybrid LSP + semantic code search for deterministic enumeration. [sourcegraph.com](https://sourcegraph.com), 2025.
- "Lost-in-distance" phenomenon in graph-aware LLM attention. OpenReview, 2025.
- VibeDrift behavioral pattern detection; Archcodex living registry pattern. 2025–2026.


