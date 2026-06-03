# Structured Full Search — Design Spec (2026-06-03)

**Status:** P0 landed (`edb3e1d302`) — the unified `vox-search` surface is wired and live (`vox_search_query` command + `SearchView`, registered in the Track A surface registry). This spec covers the enhancements **P1–P4** that turn it into full search.

## Verified constraints (these shaped the design — see the 2026-06-03 reality-check)

1. **No corpus carries a line number.** No struct in any `vox-search` corpus has a line/range field; chunk/repo snippets have no offsets, and the repo "snippet" is literally the file path. → **Locators are file-only** (open the file, not a line). "Open at exact line" is out of scope (needs upstream line-offset indexing — a separate initiative).
2. **Highlights are feasible only where snippets are real text:** memory, knowledge, chunk, web. Repo snippet = path (nothing to highlight); rrf/symbol lines are synthetic prose.
3. **Corpora override is easy:** `execute_search_plan(ctx, query, &SearchPlan, …)` takes a plan whose `pub corpora: Vec<SearchCorpus>` is authoritative (every corpus branch gates on `plan.corpora.contains(...)`). Tantivy & Qdrant are NOT in `SearchCorpus` (feature-gated, not toggleable that way).
4. **Cross-corpus score comparability is weak** (knowledge hardcoded `0.0`; chunk = `sim*2`; memory = fused BM25+vector). → The UI **groups by source**; do not present a single globally-ranked list as authoritative.
5. **Open needs a new capability:** the Tauri shell ACL is locked to the `vox` sidecar (no arbitrary spawn, no `shell:allow-open`), and there's no clipboard/opener plugin. → Use a **raw `std::process::Command`** (the `daemon.rs` precedent, bypasses ACL) for the editor + the **`open` crate** for URLs.
6. **"Derive lines from hits" is riskier than it looked** (9 distinct formats, some feature-gated, rrf is a pure string-merge with no typed source). → **Adaptation: enrich the structured hit as the GUI primary; leave the existing `_lines` building unchanged** (it is already built from the same typed data; the small duplication is safer than inverting and risking `web_gather`'s parser).

## Backend data model (`vox-search`)

**Extend the existing `UnifiedHit` in place** (the GUI DTO contract comment says "do NOT rename"; renaming buys nothing). Add two fields:

```rust
pub struct MatchSpan { pub start: u32, pub end: u32 } // byte offsets into `snippet`

pub enum OpenLocator {
    File { path: String },     // file-only (no line — see constraint 1)
    Web { url: String },
    Memory { id: String },
    None,
}

// UnifiedHit gains:
//   pub highlights: Vec<MatchSpan>,
//   pub locator: OpenLocator,
```

- `compute_highlights(snippet: &str, query: &str) -> Vec<MatchSpan>` — pure, case-insensitive token match → non-overlapping byte spans. Called at the memory/knowledge/chunk/web capture points (query is already in scope there). Empty for repo/symbol/fused.
- `locator` derived from source+path/url: memory/chunk/repo → `File{path}` (when path is a real file path; chunk `chunk_id`/repo path), web → `Web{url}` (from `h.path` which is the URL), memory record → `Memory{id}` when the path is a `node:`/record id, else `None`.
- The 9 `_lines` arrays and `sort_unified_hits_desc` are **unchanged**.
- Tests: `compute_highlights` (multi-term, overlap, case), locator construction per source, serde round-trip of the new fields.

## Search command (`vox-gui/src/commands/search.rs`)

Extend `vox_search_query` and its DTOs:

```rust
pub struct UnifiedHitDto { /* existing… */ highlights: Vec<MatchSpanDto>, locator: OpenLocatorDto }
pub struct FacetCount { pub value: String, pub count: u32 }
pub struct SearchResponseDto {
    pub hits: Vec<UnifiedHitDto>,
    pub facets_by_source: Vec<FacetCount>,
    pub facets_by_kind: Vec<FacetCount>,
    pub total: usize,
    pub next_cursor: Option<usize>,   // offset of the next page, or None
    pub corpora: Vec<String>,
}
// params: query, scope: Option<Vec<String>>, kinds: Option<Vec<String>>,
//         path_glob: Option<String>, since_ms: Option<i64>,
//         limit: Option<usize>, offset: Option<usize>
```

- **Real scope→corpora (P2):** map scope chip ids → `SearchCorpus` and override the plan. Implementation: add `corpora_override: Option<Vec<SearchCorpus>>` to `run_search_with_verification` (apply to `plan.corpora` after `heuristic_search_plan`), preserving the verification/Tavily/RRF wrapper. Replaces the WIP's post-hoc `source` filter.
- **Filters (P2):** apply `path_glob` (glob match on `hit.path`) and `since_ms` (where a timestamp is in `meta`; skip hits without one only when `since_ms` is set) to the captured hits before paging.
- **Facets (P2):** counts of `source` and `kind` over the *full* filtered hit set (before paging).
- **Pagination (P2):** request `offset + limit` from the engine, slice `[offset .. offset+limit]`; `next_cursor = Some(offset+limit)` when more remain.
- Tests: facet aggregation, scope→corpora mapping, path-glob filter, pagination slice.

## Open-locator command (`vox-gui`)

Add the `open` crate (URL open) to `vox-gui/Cargo.toml`. New command:

```rust
#[tauri::command]
pub async fn open_locator(locator: OpenLocatorDto) -> Result<OpenOutcome, String>
// File{path}  -> std::process::Command::new(editor).arg(&path).spawn()  (editor = $VOX_EDITOR else "code")
// Web{url}    -> open::that(&url)
// Memory{id}  -> Ok(OpenOutcome::RevealMemory { id })  // frontend navigates to MemoryView; no spawn
// None        -> Err("no locator")
```

- `editor_command(editor: &str, path: &str) -> (String, Vec<String>)` is a pure helper (unit-tested); default `code <path>` (file-only — no `-g path:line`, per constraint 1). Clipboard fallback stays client-side (`navigator.clipboard`).
- Register in `main.rs`.

## Frontend

- **SearchView** (extend the live surface): render `<mark>` over `highlights` spans in snippets; a **facet sidebar** (source/kind counts, click to filter); **kind + path-glob + recency** filter inputs; **load-more** via `next_cursor`; each hit's primary click → `invoke('open_locator', { locator })` (Memory → navigate to MemoryView); keyboard nav (↑/↓/↵). Drive scope chips' available set from `response.corpora`. Pure helpers (`renderHighlights`, `aggregateFacets`) get vitest coverage.
- **CommandPalette** (`layout/CommandPalette.tsx`): add a backend-search mode — when the query is non-empty, debounce-call `vox_search_query` with a small limit (~8), show top hits inline; `↵` opens the full Search surface seeded with the query (shared via a tiny store or `localStorage` `vox_search_query`).
- **MemoryView**: repoint `mnemosyne_recall` → `vox_search_query` scoped to `memory,knowledge,chunk`, bridging the three mismatches (field rename `src/line/text`→`source/path/snippet`; scope array vs CSV; vocabulary `proj/docs/chats/rules`→corpus ids). Keep `get_memory_status`/`mnemosyne_reindex`. Render highlights + click-to-open. Retire the dead un-indexed `MemorySearchEngine` path in the `mnemosyne_recall` backend (or delegate it to the shared search helper).

## Phasing (each independently shippable; hand to writing-plans)

- **P1 — Highlighting:** `MatchSpan`/`compute_highlights` + capture + DTO + `<mark>` rendering.
- **P2 — Scope/facets/filters/pagination:** `corpora_override`, filters, facets, cursor; SearchView sidebar + load-more.
- **P3 — Click-to-open:** `OpenLocator` + `open_locator` command (`open` crate + raw `Command`) + SearchView wiring.
- **P4 — Reach:** CommandPalette backend search; MemoryView repoint.

## Out of scope (explicit)

Open-at-exact-line and `line_range` (no data source — needs upstream chunk/repo line-offset indexing); typed capture of tantivy/qdrant/rrf-fused/symbol corpora (feature-gated / no typed source / low value); per-leg `ScoreBreakdown` (not separately available). These are noted as future work, not silently dropped.
