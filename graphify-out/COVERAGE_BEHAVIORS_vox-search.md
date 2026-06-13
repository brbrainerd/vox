# vox-search — Semantic Behavior Map

Derived from 32 extracted Behavior claims, deduped to 13 distinct behaviors across 9 symbols.

## Summary

The vox-search test surface concentrates proof on two well-covered areas: the SearXNG policy validators (`normalize_searxng_engines_csv`, `normalize_searxng_language_tag`), which are the only symbols with explicit **error/rejection** tests, and a small core of ranking/serialization utilities (`rrf_merge_line_lists`, `UnifiedHit`, `normalized_char_similarity`, `run_multi_hop_web_research`) that carry **invariant** or **edge** proofs. The semantically richer surfaces — CRAG query expansion, the hybrid memory search engine with its contradiction detector, and the symbol-proximity contract loader — are proven **happy-path only**, leaving their failure, empty, and conflict modes unverified. These are the most actionable holes because their contracts (a conditional expander, a flagging detector, and a file/contract reader) all have obvious negative/error modes.

## Per-symbol behaviors

### `CragRouter::expand_queries_from_partial_evidence` (crag.rs)
- Expands weak evidence (score <= 0.2) into queries containing "primary source evidence" and "independent corroborating sources". *(happy)*
- Expands contradictory evidence (`potential_contradiction = true`) into a "conflicting evidence source comparison" query. *(happy)*
- **Error path:** none. **Edge/invariant:** none. No test that strong, non-contradictory evidence produces no expansion.

### `MemorySearchEngine::search` (memory_hybrid.rs)
- Returns non-empty hits for indexed content with provenance tags prefixed `bm25:`. *(happy)*
- Flags hits `potential_contradiction = true` when multiple indexed files share overlapping topic keywords; returns exactly 2 hits in the overlapping-topic case. *(happy)*
- **Error path:** none. **Edge/invariant:** none. No empty-corpus/no-match case, and contradiction detection is never tested for false positives.

### `normalize_searxng_engines_csv` (policy.rs)
- Accepts `google,bing,ddg` and returns it unchanged. *(happy)*
- Rejects malformed input with invalid characters (e.g. semicolon injection). *(error)*
- **Error path:** yes. **Edge/invariant:** none (empty list, whitespace, duplicates untested).

### `normalize_searxng_language_tag` (policy.rs)
- Accepts valid BCP 47 hyphen tags like `en-US`. *(happy)*
- Rejects underscore form `en_US`. *(error)*
- **Error path:** yes. **Edge/invariant:** none.

### `run_multi_hop_web_research` (research.rs)
- Returns empty result when the initial queries array is empty, regardless of other params. *(edge)*
- **Error path:** none. **Edge/invariant:** edge only. No non-empty happy path or hop-termination/error proof.

### `rrf_merge_line_lists` (rrf.rs)
- Merges ranking lists via reciprocal rank fusion; items in multiple lists rank above single-list items. *(happy + invariant)*
- **Error path:** none. **Edge/invariant:** invariant (multi-list > single-list ordering).

### `rrf_dedup_key` (rrf.rs)
- Extracts stable dedup keys from bracketed prefixes: `[repo:crates/foo]` → `repo:crates/foo`; `[node:abc]` → `node:abc`. *(happy)*
- **Error path:** none. **Edge/invariant:** none (malformed/unbracketed input untested).

### `normalized_char_similarity` (symbol_proximity.rs)
- Returns exactly 1.0 for identical strings. *(invariant)*
- **Error path:** none. **Edge/invariant:** identity invariant only; no disjoint (0.0) or range proof.

### `scan_symbol_proximity` (symbol_proximity.rs)
- Loads retired-symbol contract from `contracts/proximity/retired-surfaces.v1.json` at repo root and returns mappings including `legacy-split-parser` → `vox-compiler` (both retired symbol and canonical replacement present). *(happy)*
- **Error path:** none. **Edge/invariant:** none (missing/malformed contract untested).

### `UnifiedHit` / `sort_unified_hits_desc` (unified.rs)
- `UnifiedHit`: serde round-trips with all fields (score, provenance, path, title, snippet) preserved. *(invariant)*
- `sort_unified_hits_desc`: sorts descending by score, e.g. `[0.1, 0.9, 0.5]` → `[0.9, 0.5, 0.1]`. *(happy)*
- **Error path:** none. **Edge/invariant:** serde round-trip invariant; sort has no stability/empty/tie/NaN proof.

## Semantic gaps

Symbols proven on the happy path only whose contracts have a clear failure/empty/conflict mode:

1. **`CragRouter::expand_queries_from_partial_evidence` — conditional expander, no negative case.** Both proven behaviors are positive expansions. There is no test that *strong* evidence (score > 0.2 and no contradiction) yields **no** spurious expansion. A router that always expands would pass every current test.
2. **`MemorySearchEngine::search` — detector with no false-positive guard, no empty path.** The contradiction flag is only proven to fire on overlapping topics; nothing proves it *stays false* for unrelated documents. Also missing: empty-corpus and zero-match behavior. This is an integrity/relevance surface — the most consequential gap.
3. **`scan_symbol_proximity` — contract/file loader with no failure path.** Reads `contracts/proximity/*.json` from repo root but is never tested against a missing, empty, or malformed contract, nor an absent repo root. A loader silently returning empty/garbage would pass.
4. **`rrf_dedup_key` — parser with no malformed-input proof.** Only well-formed `[prefix]` strings are tested; the fallback for unbracketed or malformed input (which governs dedup correctness) is unverified.
5. **`sort_unified_hits_desc` — comparator with no edge proof.** No stability, empty-slice, equal-score tie, or NaN-score behavior is pinned, despite floating-point score sorting being a known footgun.
6. **`normalized_char_similarity` — only the identity end of the range proven.** The 0.0 (fully disjoint) bound and general range/ordering invariants are untested.