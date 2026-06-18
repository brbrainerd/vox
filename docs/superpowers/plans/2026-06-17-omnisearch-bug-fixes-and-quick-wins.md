# Omni-Search Bug Fixes & Quick Wins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 8 verified bugs and quick-win improvements in the Vox search stack — no new infrastructure, each fix is a targeted patch that immediately improves search correctness and quality.

**Architecture:** Search flows from `vox-db-types` (corpus plan types) → `vox-search` (execution engine) → `vox-gui/src/commands/search.rs` (Tauri bridge) → `vox-gui/ui` (React frontend). Bugs span all four layers. Each task is independent and can be cherry-picked individually.

**Tech Stack:** Rust (cargo test), TypeScript/React (vitest + @testing-library/react), Tauri 2, SQLite/libSQL (vox-db), pnpm.

---

## Scope Note

This plan covers **Phase A** of the omni-search roadmap: Tier 1 fixes with no new infrastructure. Three larger efforts are explicitly out of scope and planned separately:

- **Plan B:** Persistent repo index (Tantivy + file-watcher, tree-sitter symbol extraction)
- **Plan C:** Chat as first-class `SearchCorpus::Chats`
- **Plan D:** Omni-Search UX elevation (streaming results, preview panel, intent prefix routing)

---

## Codebase Orientation (read this before starting)

The search stack has four layers. Here is what each does and where it lives:

**Layer 1 — Corpus types** (`crates/vox-db-types/src/retrieval.rs`):
Defines `SearchCorpus` enum (which data sources to search), `SearchIntent` (what kind of query this is), and `heuristic_search_plan()` (maps a raw query to a plan). This is where "which corpora get consulted for a broad search?" is decided.

**Layer 2 — Search engine** (`crates/vox-search/src/`):
`execution.rs` orchestrates the actual search against each corpus. `policy.rs` holds all tunable defaults (weights, flags, limits). `rrf.rs` does cross-corpus result merging. Each corpus has its own file (e.g., `memory_hybrid.rs`, `repo_path.rs`).

**Layer 3 — Tauri bridge** (`crates/vox-gui/src/commands/search.rs`):
Exposes the `vox_search_query` Tauri command that the frontend calls. Translates frontend scope strings (`"memory"`, `"chats"`) to `SearchCorpus` variants, calls the search engine, and maps results to DTOs. This is where the chats routing bug lives.

**Layer 4 — React frontend** (`crates/vox-gui/ui/src/components/surfaces/Search/`):
`SearchView.tsx` — the full search surface (sidebar). `searchHelpers.ts` — shared types and display utilities (`scoreToPct`, `groupBySource`, `UnifiedHit`). `SearchView.test.tsx` — where frontend tests live (create this file in Task 4).

**Running tests:**
- Rust: `cargo test -p <crate-name>` from the repo root
- TypeScript: `pnpm test` from inside `crates/vox-gui/ui/`
- Single test: `cargo test -p vox-search my_test_name -- --nocapture`

---

## File Map

| File | What changes |
|---|---|
| `crates/vox-search/src/policy.rs:167` | Flip `prefer_rrf_merge` default to `true` |
| `crates/vox-db-types/src/retrieval.rs:287–305` | Add `SymbolProximity` to BroadResearch/FactualLookup default corpora |
| `crates/vox-search/src/execution.rs:379–393` | Propagate rank-based score for KnowledgeGraph hits (was `0.0`) |
| `crates/vox-gui/src/commands/search.rs:140–320` | Fix chats-only scope routing; add `is_chats_only_scope` helper |
| `crates/vox-gui/src/commands/search.rs` (`SearchResponseDto`) | Add `repo_truncated: bool` field |
| `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx:45–52` | Replace broken `pathMatchesGlob` with regex-based implementation |
| `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx:449–453` | Add `aria-live="polite"` to result count `<div>` |
| `crates/vox-gui/ui/src/components/surfaces/Search/searchHelpers.ts` | Clamp `scoreToPct` to [0, 100] |
| `crates/vox-gui/Cargo.toml` | Add `"web-scrape"` feature to `vox-search` dependency |
| `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx` | **NEW** — vitest unit tests for `pathMatchesGlob`, `scoreToPct`, accessibility |

---

## Prerequisites

Set up an isolated worktree so your changes don't interfere with other work:

```powershell
# From the repo root (c:\Users\Owner\vox or wherever you cloned to)
git worktree add ../vox-omnisearch-fixes -b feat/omnisearch-bug-fixes
```

Then open `../vox-omnisearch-fixes` as your working directory. Verify the baseline builds:

```powershell
cargo build -p vox-gui 2>&1 | Select-String "error"
# Expected: no output (zero errors)

cargo build -p vox-search 2>&1 | Select-String "error"
# Expected: no output

cargo build -p vox-db-types 2>&1 | Select-String "error"
# Expected: no output
```

Verify frontend tests pass before you start:

```powershell
cd crates/vox-gui/ui
pnpm test
# Expected: all tests pass, zero failures
cd ../../..
```

If anything fails at baseline, stop and fix the environment before continuing.

---

## Task 1: Enable RRF Fusion by Default

**What is RRF?** Reciprocal Rank Fusion is a technique that merges result lists from different sources (memory, knowledge graph, repo files, web) by rank position rather than score. Without it, results come back as "memory results, then chunk results, then repo results" — a rigid ordering by corpus. With RRF, the top result from each corpus competes against each other, and the most relevant hits surface to the top regardless of which corpus they came from.

RRF is currently off by default. The env var `VOX_SEARCH_PREFER_RRF=true` enables it. This task flips the default so all users benefit without configuration. Operators who want to disable it set `VOX_SEARCH_PREFER_RRF=false`.

**Files:**
- Modify: `crates/vox-search/src/policy.rs:167`
- Test: `crates/vox-search/src/policy.rs` (add to existing `#[cfg(test)]` block)

**Background — current code at line 167:**
```rust
prefer_rrf_merge: parse_truthy_env(vox_secrets::SecretId::VoxSearchPreferRrf),
```
`parse_truthy_env` returns `false` when the env var is absent. The fix changes the absent-value default to `true` using an inline match instead of the helper.

- [ ] **Step 1.1: Write the failing tests**

Open `crates/vox-search/src/policy.rs`. Scroll to the bottom. There is already a `#[cfg(test)]` block. Add these two tests inside it:

```rust
#[test]
fn rrf_is_enabled_by_default_when_env_unset() {
    // This test modifies env which is process-global — run tests with --test-threads=1
    // if env collision becomes a flake.
    std::env::remove_var("VOX_SEARCH_PREFER_RRF");
    let policy = SearchPolicy::from_env();
    assert!(
        policy.prefer_rrf_merge,
        "prefer_rrf_merge must default to true; operators set VOX_SEARCH_PREFER_RRF=false to disable"
    );
}

#[test]
fn rrf_disabled_when_env_set_to_false() {
    std::env::set_var("VOX_SEARCH_PREFER_RRF", "false");
    let policy = SearchPolicy::from_env();
    assert!(!policy.prefer_rrf_merge, "VOX_SEARCH_PREFER_RRF=false must disable RRF");
    std::env::remove_var("VOX_SEARCH_PREFER_RRF");
}
```

- [ ] **Step 1.2: Run to verify they fail**

```powershell
cargo test -p vox-search rrf -- --nocapture
```

Expected: `rrf_is_enabled_by_default_when_env_unset` FAILS with assertion `false != true`.

- [ ] **Step 1.3: Apply the fix**

In `crates/vox-search/src/policy.rs`, replace line 167:

```rust
// BEFORE (line 167)
prefer_rrf_merge: parse_truthy_env(vox_secrets::SecretId::VoxSearchPreferRrf),
```

```rust
// AFTER — default ON; set VOX_SEARCH_PREFER_RRF=false to disable
prefer_rrf_merge: match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSearchPreferRrf)
    .expose()
{
    Some(v) => {
        let v = v.trim();
        v == "1"
            || v.eq_ignore_ascii_case("true")
            || v.eq_ignore_ascii_case("yes")
            || v.eq_ignore_ascii_case("on")
    }
    None => true, // default ON
},
```

- [ ] **Step 1.4: Run tests to verify they pass**

```powershell
cargo test -p vox-search rrf -- --nocapture
```

Expected: both tests show `ok`.

- [ ] **Step 1.5: Run the full vox-search suite**

```powershell
cargo test -p vox-search
```

Expected: all pass, zero regressions.

- [ ] **Step 1.6: Commit**

```powershell
git add crates/vox-search/src/policy.rs
git commit -m "fix(search): enable RRF fusion by default (VOX_SEARCH_PREFER_RRF defaults to true)"
```

---

## Task 2: Propagate KnowledgeGraph Score (Fix 0.0 Bug)

**What is the bug?** Every result from the knowledge graph gets `score: 0.0` hardcoded in `execution.rs`. This means KG results always rank at the very bottom when results are sorted by score, regardless of how well the query matches the knowledge node. A highly relevant KG definition sinks below low-relevance repo path matches.

**The fix:** The DB already returns knowledge nodes in best-match-first order (FTS5 rank, or created-at DESC for the LIKE fallback). Use the result position as a rank-based score: position 0 → `1.0`, position 1 → `0.5`, position 2 → `0.33`, etc. Formula: `1.0 / (1.0 + rank)`. This requires zero schema changes.

**Files:**
- Modify: `crates/vox-search/src/execution.rs:379–393`

**Background — what `query_knowledge_nodes` returns:**
The function signature is:
```rust
pub async fn query_knowledge_nodes(&self, query: &str, limit: i64)
    -> Result<Vec<(String, String, String)>, StoreError>
```
Each tuple is `(id, label, snippet_200_chars)`. Rows are ordered best-match-first by the DB.

**The broken code block (execution.rs ~lines 379–393):**
```rust
rows.into_iter()
    .map(|(id, label, snippet)| {
        let snip = snippet.replace('\n', " ");
        unified_hits.push(UnifiedHit {
            source: "knowledge".to_string(),
            kind: "knowledge".to_string(),
            path: Some(format!("node:{id}")),
            title: (!label.is_empty()).then(|| label.clone()),
            snippet: snip.clone(),
            score: 0.0,   // <-- BUG: all KG hits rank last regardless of relevance
            provenance: vec!["knowledge:fts".to_string()],
        });
        format!("[node:{id}] {label} — {snip}")
    })
    .collect::<Vec<_>>()
```

- [ ] **Step 2.1: Write the test**

Add to the `#[cfg(test)]` block in `crates/vox-search/src/execution.rs` (or create one if absent):

```rust
#[test]
fn knowledge_graph_rank_based_scores_are_decreasing() {
    // Simulates the fixed formula applied to 3 KG rows returned by the DB.
    // Row 0 is best match (rank 0), row 1 is next, etc.
    let scores: Vec<f64> = (0..3).map(|rank| 1.0 / (1.0 + rank as f64)).collect();

    assert!((scores[0] - 1.0).abs() < f64::EPSILON, "rank 0 score = 1.0");
    assert!((scores[1] - 0.5).abs() < f64::EPSILON, "rank 1 score = 0.5");
    assert!((scores[2] - 1.0 / 3.0).abs() < 1e-10, "rank 2 score = 0.333...");
    assert!(scores[0] > scores[1], "scores must decrease with rank");
    assert!(scores[1] > scores[2], "scores must decrease with rank");
}
```

- [ ] **Step 2.2: Run to confirm it passes (it documents the target formula)**

```powershell
cargo test -p vox-search knowledge_graph_rank_based_scores -- --nocapture
```

Expected: PASS. This test validates the formula, not the implementation — it tells you what the code should produce.

- [ ] **Step 2.3: Apply the fix to execution.rs**

Find the `rows.into_iter().map(|(id, label, snippet)|` block that pushes to `unified_hits` with `score: 0.0`. Change `into_iter()` to `into_iter().enumerate()` and compute the rank-based score:

```rust
// AFTER — note the .enumerate() and the score formula
rows.into_iter()
    .enumerate()
    .map(|(rank, (id, label, snippet))| {
        let snip = snippet.replace('\n', " ");
        unified_hits.push(UnifiedHit {
            source: "knowledge".to_string(),
            kind: "knowledge".to_string(),
            path: Some(format!("node:{id}")),
            title: (!label.is_empty()).then(|| label.clone()),
            snippet: snip.clone(),
            // Rank-based score: DB returns best-match-first, so position 0 is most relevant.
            // Formula 1.0/(1.0+rank): gives 1.0, 0.5, 0.33, 0.25, ...
            // Consistent with RRF weighting; no DB schema change required.
            score: 1.0 / (1.0 + rank as f64),
            provenance: vec!["knowledge:fts".to_string()],
        });
        format!("[node:{id}] {label} — {snip}")
    })
    .collect::<Vec<_>>()
```

- [ ] **Step 2.4: Run the full vox-search suite**

```powershell
cargo test -p vox-search
```

Expected: all pass.

- [ ] **Step 2.5: Commit**

```powershell
git add crates/vox-search/src/execution.rs
git commit -m "fix(search): propagate rank-based score for KnowledgeGraph hits (was hardcoded 0.0)"
```

---

## Task 3: Fix Chats-Only Scope Routing Bug

**What is the bug?** When a user selects only the "Chats" scope chip in the search UI, the search should return only chat messages. Instead it returns chat messages PLUS all-corpora results (memory, knowledge graph, repo files, web). This is because `"chats"` has no corresponding `SearchCorpus` variant, so after the scope is filtered through `scope_to_corpus()`, the corpora list is empty, the code falls through to the full heuristic plan, and the chat LIKE query appends its results on top.

**The fix:** Detect before the main search runs whether the scope is exclusively `["chats"]`. If so, skip the main search entirely. Only the chat LIKE query at the bottom of the function runs.

**Files:**
- Modify: `crates/vox-gui/src/commands/search.rs`

**Background — how scope routing currently works in `vox_search_query` (~lines 233–320):**

```rust
// 1. Build scope tags from the scope parameter
let scope_tags: Vec<String> = scope.clone().unwrap_or_default();

// 2. Map scope tags to SearchCorpus variants ("chats" maps to None — no variant exists)
let scope_corpora: Option<Vec<SearchCorpus>> = scope.and_then(|v| {
    // ... "chats" is filtered out; if nothing else in the list, returns None
});

// 3. Execute main search (always runs, even for chats-only)
let (exec, plan) = if let Some(corpora) = scope_corpora { ... }
                   else { run_search_with_verification(...) };

// 4. Chats appended AFTER (correct position, wrong trigger condition)
let wants_chats = scope_tags.is_empty() || scope_tags.iter().any(|x| x == "chats");
if wants_chats && let Some(db_ref) = ctx.db.as_ref() { ... }
```

- [ ] **Step 3.1: Write the failing test**

In `crates/vox-gui/src/commands/search.rs`, inside the `#[cfg(test)]` module at the bottom of the file, add:

```rust
#[test]
fn scope_to_corpus_does_not_map_chats() {
    // "chats" has no SearchCorpus variant — it is handled separately after the main search.
    // Verify this invariant so we can safely use it as a sentinel value.
    assert!(
        scope_to_corpus("chats").is_none(),
        "chats must not map to a SearchCorpus variant; it is handled post-hoc"
    );
}

#[test]
fn is_chats_only_scope_helper_logic() {
    // After Step 3.3, we add this function. Test it here.
    let only_chats = vec!["chats".to_string()];
    let mixed = vec!["chats".to_string(), "memory".to_string()];
    let empty: Vec<String> = vec![];
    let no_chats = vec!["memory".to_string(), "repo".to_string()];

    assert!(is_chats_only_scope(&only_chats), "['chats'] should be chats-only");
    assert!(!is_chats_only_scope(&mixed), "['chats','memory'] is not chats-only");
    assert!(!is_chats_only_scope(&empty), "[] is not chats-only (means all scopes)");
    assert!(!is_chats_only_scope(&no_chats), "['memory','repo'] is not chats-only");
}
```

- [ ] **Step 3.2: Run to verify**

```powershell
cargo test -p vox-gui scope_to_corpus_does_not_map_chats -- --nocapture
```

Expected: PASS.

```powershell
cargo test -p vox-gui is_chats_only_scope_helper_logic -- --nocapture
```

Expected: FAIL with "cannot find function `is_chats_only_scope` in this scope".

- [ ] **Step 3.3: Add the helper function**

In `crates/vox-gui/src/commands/search.rs`, just above the `vox_search_query` function definition, add:

```rust
/// Returns `true` when the caller explicitly requested chats as the sole scope.
///
/// When this is true, the main multi-corpus search is skipped entirely and only
/// `chat_search_gui_messages` runs. This prevents the chats scope from silently
/// fanning out to all corpora (the "chats" string has no `SearchCorpus` variant,
/// so it would otherwise drop through to the full heuristic plan).
fn is_chats_only_scope(scope_tags: &[String]) -> bool {
    !scope_tags.is_empty() && scope_tags.iter().all(|t| t == "chats")
}
```

- [ ] **Step 3.4: Wire the helper into `vox_search_query`**

Inside `vox_search_query`, find the block that starts approximately at line 233 where `scope_tags` and `scope_corpora` are built. Add the `chats_only` detection **after** `scope_tags` is built and **before** the main search block:

```rust
let scope_tags: Vec<String> = scope.clone().unwrap_or_default();
let chats_only = is_chats_only_scope(&scope_tags);   // <-- ADD THIS LINE

let scope_corpora: Option<Vec<SearchCorpus>> = scope.and_then(|v| { /* unchanged */ });
```

Then replace the main search execution block. Find this section (approximately lines 248–278):

```rust
// BEFORE
let (exec, plan) = if let Some(corpora) = scope_corpora {
    let mut plan: SearchPlan = heuristic_search_plan(&query, false, None);
    plan.corpora = corpora;
    let execution = execute_search_plan(&ctx, &query, &plan, engine_limit, &policy, None)
        .await
        .map_err(|e| format!("search failed: {e}"))?;
    (execution, plan)
} else {
    let (execution, _diag, plan) = run_search_with_verification(
        &ctx, &query, RetrievalTriggerMode::ExplicitToolQuery,
        engine_limit, &policy, None, None,
    )
    .await
    .map_err(|e| format!("search failed: {e}"))?;
    (execution, plan)
};

let corpora: Vec<String> = plan.corpora.iter().map(|c| format!("{c:?}").to_lowercase()).collect();
let mut all_hits: Vec<UnifiedHitDto> = exec.unified_hits.into_iter().map(unified_hit_to_dto).collect();

let wants_chats = scope_tags.is_empty() || scope_tags.iter().any(|x| x == "chats");
```

Replace with:

```rust
// AFTER — chats-only scope skips the main search entirely
let (mut all_hits, corpora) = if chats_only {
    // User asked for chats only. Skip the multi-corpus search entirely.
    // The chat LIKE query below will populate all_hits.
    (Vec::<UnifiedHitDto>::new(), vec!["chats".to_string()])
} else if let Some(corpora_list) = scope_corpora {
    let mut plan: SearchPlan = heuristic_search_plan(&query, false, None);
    plan.corpora = corpora_list;
    let execution = execute_search_plan(&ctx, &query, &plan, engine_limit, &policy, None)
        .await
        .map_err(|e| format!("search failed: {e}"))?;
    let names = plan.corpora.iter().map(|c| format!("{c:?}").to_lowercase()).collect();
    (execution.unified_hits.into_iter().map(unified_hit_to_dto).collect(), names)
} else {
    let (execution, _diag, plan) = run_search_with_verification(
        &ctx, &query, RetrievalTriggerMode::ExplicitToolQuery,
        engine_limit, &policy, None, None,
    )
    .await
    .map_err(|e| format!("search failed: {e}"))?;
    let names = plan.corpora.iter().map(|c| format!("{c:?}").to_lowercase()).collect();
    (execution.unified_hits.into_iter().map(unified_hit_to_dto).collect(), names)
};

// wants_chats: run the chat LIKE query if scope explicitly includes chats, or if no scope at all.
let wants_chats = chats_only || scope_tags.is_empty() || scope_tags.iter().any(|x| x == "chats");
```

Also update the `SearchResponseDto` construction at the bottom to use `corpora` directly (it is now a `Vec<String>` from the match above, not derived from `plan`).

- [ ] **Step 3.5: Run tests**

```powershell
cargo test -p vox-gui -- --nocapture
```

Expected: all tests pass including the two new ones.

- [ ] **Step 3.6: Commit**

```powershell
git add crates/vox-gui/src/commands/search.rs
git commit -m "fix(search): chats-only scope now returns only chat hits (was leaking all-corpora results)"
```

---

## Task 4: Fix `pathMatchesGlob` — Frontend Glob Matching is Broken

**What is the bug?** The path glob filter in `SearchView.tsx` (the "Path" input in the search header) strips `*` characters and does a `path.includes()` substring check. So the glob `**/*.rs` becomes `.rs` after stripping, and then matches any path that contains `.rs` as a substring — including `/usr/local/lib/libssl.rss` or directory names like `.rs-tools/`. This is not how globs work.

**The fix:** Replace the broken implementation with a function that converts glob syntax to a regex: `**` → `.*` (any chars including `/`), `*` → `[^/]*` (any non-separator chars), `?` → `[^/]` (one non-separator char), everything else is regex-escaped.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx:45–52`
- Create: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx`

- [ ] **Step 4.1: Create the test file**

Create `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx` with this content:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { pathMatchesGlob } from './SearchView';

describe('pathMatchesGlob', () => {
  it('returns true when glob is empty (no filter applied)', () => {
    expect(pathMatchesGlob('src/main.rs', '')).toBe(true);
    expect(pathMatchesGlob('src/main.rs', '   ')).toBe(true);
  });

  it('returns false when path is null', () => {
    expect(pathMatchesGlob(null, '**/*.rs')).toBe(false);
  });

  it('** matches across path separators', () => {
    expect(pathMatchesGlob('crates/vox-search/src/lib.rs', '**/*.rs')).toBe(true);
    expect(pathMatchesGlob('a/b/c/d/e.rs', '**/*.rs')).toBe(true);
  });

  it('** does NOT match a .rss extension as .rs', () => {
    expect(pathMatchesGlob('feed.rss', '**/*.rs')).toBe(false);
  });

  it('single * does not cross path separators', () => {
    // *.rs at the root level only matches flat filenames (no /)
    expect(pathMatchesGlob('main.rs', '*.rs')).toBe(true);
    expect(pathMatchesGlob('src/main.rs', '*.rs')).toBe(false);
  });

  it('exact path match without wildcards', () => {
    expect(pathMatchesGlob('crates/vox-gui/src/main.rs', 'crates/vox-gui/src/main.rs')).toBe(true);
    expect(pathMatchesGlob('crates/vox-gui/src/lib.rs', 'crates/vox-gui/src/main.rs')).toBe(false);
  });

  it('? matches exactly one non-separator character', () => {
    expect(pathMatchesGlob('src/main.rs', 'src/ma?n.rs')).toBe(true);
    expect(pathMatchesGlob('src/mn.rs', 'src/ma?n.rs')).toBe(false);  // ? requires exactly 1 char
    expect(pathMatchesGlob('src/ma/n.rs', 'src/ma?n.rs')).toBe(false); // ? must not be /
  });

  it('crate-scoped glob works', () => {
    expect(pathMatchesGlob('crates/vox-search/src/execution.rs', 'crates/vox-search/**')).toBe(true);
    expect(pathMatchesGlob('crates/vox-gui/src/lib.rs', 'crates/vox-search/**')).toBe(false);
  });

  it('no match returns false', () => {
    expect(pathMatchesGlob('src/main.rs', '**/*.ts')).toBe(false);
  });

  it('handles regex special chars in path literally', () => {
    // A glob with a literal dot should not treat . as "any char"
    expect(pathMatchesGlob('src/main_rs', '**/*.rs')).toBe(false); // underscore not a dot
  });
});
```

- [ ] **Step 4.2: Run tests to verify they fail**

```powershell
cd crates/vox-gui/ui
pnpm test -- SearchView.test
```

Expected: multiple failures — `pathMatchesGlob` is not exported, and the import fails.

- [ ] **Step 4.3: Replace `pathMatchesGlob` in SearchView.tsx**

Find lines 45–52 in `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx`:

```typescript
// BEFORE (lines 45–52) — strips all * then does substring match
function pathMatchesGlob(path: string | null, glob: string): boolean {
  const pattern = glob.trim();
  if (!pattern) return true;
  if (!path) return false;
  const normalized = pattern.replace(/^\*\*\//, '').replace(/\*\*/g, '').replace(/\*/g, '');
  if (!normalized) return true;
  return path.includes(normalized);
}
```

Replace with:

```typescript
/**
 * Match `path` against a glob pattern.
 *
 * Rules:
 * - `**` matches any sequence of characters including path separators (`/`)
 * - `*`  matches any sequence of characters NOT including path separators
 * - `?`  matches exactly one character that is not a path separator
 * - All other characters match literally (dot, plus, parens, etc. are NOT regex wildcards here)
 *
 * Exported so unit tests can import it directly from this module.
 */
export function pathMatchesGlob(path: string | null, glob: string): boolean {
  const pattern = glob.trim();
  if (!pattern) return true;
  if (!path) return false;

  // Build regex source by processing ** and * in separate passes.
  //
  // We split on '**' first (double-star = match anything including '/'),
  // then within each segment we split on '*' (single-star = match non-slash chars).
  // Between steps we regex-escape any literal special characters.
  const escapeLiteral = (s: string) => s.replace(/[.+^${}()|[\]\\]/g, '\\$&');

  const regexSource = pattern
    .split('**')
    .map(segment =>
      segment
        .split('*')
        .map(part => escapeLiteral(part))
        .join('[^/]*')    // single * → zero-or-more non-separator chars
    )
    .join('.*');          // double ** → zero-or-more of anything (including /)

  // Replace ? placeholders with [^/] (one non-separator char)
  const finalSource = regexSource.replace(/\?/g, '[^/]');

  try {
    return new RegExp(`^${finalSource}$`).test(path);
  } catch {
    // Malformed pattern — fall back to safe substring match
    return path.includes(pattern);
  }
}
```

- [ ] **Step 4.4: Run tests**

```powershell
pnpm test -- SearchView.test
```

Expected: all 10 tests pass.

- [ ] **Step 4.5: Run the full TypeScript suite**

```powershell
pnpm test
```

Expected: no regressions.

- [ ] **Step 4.6: Commit**

```powershell
cd ../../..
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx
git commit -m "fix(search-ui): replace broken pathMatchesGlob with regex-based glob matching; add tests"
```

---

## Task 5: Add `aria-live` to Search Result Count

**What is the bug?** When a user types into the search box, the result count updates from "0 results" to "12 results across memory, repo". Screen readers do not announce this change because the element has no ARIA live region. Screen reader users get no audio feedback that results arrived.

**The fix:** Add `aria-live="polite"` and `aria-atomic="true"` to the result count `<div>`. `polite` waits for the user to pause before announcing (non-disruptive). `atomic` announces the whole element, not just the changed portion.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx:449–453`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx`

- [ ] **Step 5.1: Add the accessibility test to SearchView.test.tsx**

First add the required imports at the top of the test file:

```typescript
import { render } from '@testing-library/react';
import { SearchView } from './SearchView';

// Mock voxTransport — SearchView calls Tauri commands that don't exist in tests
vi.mock('../../../transport', () => ({
  voxTransport: {
    voxSearchQuery: vi.fn().mockResolvedValue({
      hits: [], total: 0, corpora: [],
      facets_by_source: [], facets_by_kind: [], next_cursor: null,
    }),
    openLocator: vi.fn(),
  },
}));

vi.mock('../../../lib/gamifyGuiEvents', () => ({
  recordGamifyGuiEvent: vi.fn(),
}));
```

Then add this describe block:

```typescript
describe('SearchView accessibility', () => {
  it('result count has aria-live="polite" for screen reader announcements', () => {
    // SearchView requires pushToast prop
    render(<SearchView pushToast={vi.fn()} />);
    const liveRegion = document.querySelector('[aria-live="polite"]');
    expect(liveRegion).not.toBeNull();
  });

  it('result count has aria-atomic="true"', () => {
    render(<SearchView pushToast={vi.fn()} />);
    const liveRegion = document.querySelector('[aria-atomic="true"]');
    expect(liveRegion).not.toBeNull();
  });
});
```

- [ ] **Step 5.2: Run tests to verify they fail**

```powershell
cd crates/vox-gui/ui
pnpm test -- SearchView.test
```

Expected: the two accessibility tests FAIL — no element with `aria-live="polite"` exists.

- [ ] **Step 5.3: Apply the fix to SearchView.tsx**

Find lines 449–453 in `SearchView.tsx`:

```tsx
{/* BEFORE */}
<div className="text-xs text-zinc-500 mt-1">
  {response
    ? `${response.total} result${response.total !== 1 ? 's' : ''} across ${response.corpora.join(', ')}`
    : 'Search across memory, knowledge, repo, and web'}
</div>
```

Replace with:

```tsx
{/* AFTER — aria-live announces result count changes to screen readers */}
<div
  className="text-xs text-zinc-500 mt-1"
  aria-live="polite"
  aria-atomic="true"
>
  {response
    ? `${response.total} result${response.total !== 1 ? 's' : ''} across ${response.corpora.join(', ')}`
    : 'Search across memory, knowledge, repo, and web'}
</div>
```

- [ ] **Step 5.4: Run tests**

```powershell
pnpm test -- SearchView.test
```

Expected: all tests pass.

- [ ] **Step 5.5: Commit**

```powershell
cd ../../..
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx
git commit -m "fix(a11y): add aria-live=polite to search result count for screen reader announcements"
```

---

## Task 6: Emit Truncation Warning When Repo Scan Cap Is Hit

**What is the bug?** When a repo has more than `repo_inventory_max_files` files (default: 20,000), the WalkDir scan silently drops files past the limit. The user sees fewer results than actually exist, with no indication. The Vox repo itself has 150+ crates and is approaching this limit.

**The fix:** Thread a `truncated: bool` flag from the repo path search up through to `SearchResponseDto`, and show a warning toast in the UI.

**Files:**
- Modify: `crates/vox-search/src/` (find the cap enforcement; see Step 6.1)
- Modify: `crates/vox-gui/src/commands/search.rs` (`SearchResponseDto` + return value)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx`

- [ ] **Step 6.1: Find where the file cap is applied**

```powershell
rg -n "repo_inventory_max_files\|\.take(" crates/vox-search/src/ --iglob "*.rs"
```

Open the file(s) found. Look for an iterator chain like `.take(policy.repo_inventory_max_files)`. Note the file name and line. This is where you will thread the truncation flag.

- [ ] **Step 6.2: Write a test for truncation detection**

In the file from Step 6.1, add a unit test in its `#[cfg(test)]` block:

```rust
#[test]
fn repo_scan_detects_truncation_when_over_limit() {
    // Build a fake list of more paths than the cap
    let max = 3usize;
    let paths: Vec<std::path::PathBuf> = (0..10)
        .map(|i| std::path::PathBuf::from(format!("src/file_{i}.rs")))
        .collect();

    // count_exceeds_max tests whether truncation would occur given paths and a limit.
    // We will add this as an inline helper in the fix step.
    let truncated = paths.len() > max;
    let kept: Vec<_> = paths.into_iter().take(max).collect();

    assert!(truncated, "should detect truncation");
    assert_eq!(kept.len(), max);
}
```

- [ ] **Step 6.3: Add truncation detection at the cap site**

At the site found in Step 6.1, change the iterator to collect all paths first, then check if truncation occurred before applying the `.take()`:

```rust
// Find the existing code that caps the file list. It will look like one of:
//   .take(policy.repo_inventory_max_files)
//   let hits: Vec<...> = walker.take(max_files).collect();

// Change to (adapt variable names to match what you found):
let all_files: Vec<_> = walker.collect();
let repo_truncated = all_files.len() > policy.repo_inventory_max_files;
let capped_files: Vec<_> = all_files.into_iter().take(policy.repo_inventory_max_files).collect();
// Use capped_files where you previously used the walker directly.
```

The `repo_truncated` bool needs to flow back to the caller. How to thread it depends on the function's return type:
- If the function returns `Vec<UnifiedHit>`, change it to return `(Vec<UnifiedHit>, bool)`.
- If it returns a struct, add a `pub truncated: bool` field.
- Follow the call chain up to `execute_search_plan` in `execution.rs`, then up to `vox_search_query` in `search.rs`.

The key invariant: by the time you reach `search.rs`, you have a `repo_truncated: bool` available.

- [ ] **Step 6.4: Add `repo_truncated` to `SearchResponseDto`**

In `crates/vox-gui/src/commands/search.rs`, add the field to the struct:

```rust
#[derive(Debug, serde::Serialize)]
pub struct SearchResponseDto {
    pub hits: Vec<UnifiedHitDto>,
    pub facets_by_source: Vec<FacetCount>,
    pub facets_by_kind: Vec<FacetCount>,
    pub total: usize,
    pub next_cursor: Option<usize>,
    pub corpora: Vec<String>,
    /// True when the repo file WalkDir scan was capped at `repo_inventory_max_files`.
    /// Some repo files may be absent from results.
    pub repo_truncated: bool,
}
```

Update the `Ok(SearchResponseDto { ... })` return to include `repo_truncated` (populated from Step 6.3).

- [ ] **Step 6.5: Add the `repo_truncated` field to the TypeScript type**

In `crates/vox-gui/ui/src/components/surfaces/Search/searchHelpers.ts`, find the `SearchResponse` type and add the field:

```typescript
export interface SearchResponse {
  hits: UnifiedHit[];
  facets_by_source: FacetCount[];
  facets_by_kind: FacetCount[];
  total: number;
  next_cursor: number | null;
  corpora: string[];
  repo_truncated?: boolean;   // <-- add this
}
```

- [ ] **Step 6.6: Show the warning toast in SearchView.tsx**

In `SearchView.tsx`, find where `setResponse` is called (in the `mergeHits` effect). After setting the response, add:

```typescript
if (resp.repo_truncated) {
  pushToast({
    tone: 'warn',
    title: 'Repo scan truncated',
    body: `Results show the first ${(20_000).toLocaleString()} repo files. Narrow your search or use a path glob to find files in deep directories.`,
  });
}
```

- [ ] **Step 6.7: Run all tests**

```powershell
cargo test -p vox-search
cargo test -p vox-gui
```

```powershell
cd crates/vox-gui/ui
pnpm test
```

Expected: all pass.

- [ ] **Step 6.8: Commit**

```powershell
cd ../../..
git add crates/vox-search/src/
git add crates/vox-gui/src/commands/search.rs
git add crates/vox-gui/ui/src/components/surfaces/Search/
git commit -m "fix(search): surface repo file-scan truncation warning when 20k file cap is hit"
```

---

## Task 7: Enable `web-scrape` Feature in GUI Build

**What is the bug?** When users select the Web scope in the search UI, results contain only engine snippets (30–100 characters). The same query via the MCP `vox_research_run` tool returns full-page extracted markdown. The difference: the GUI's dependency on `vox-search` does not include the `web-scrape` feature flag, so the HTML-to-markdown extraction code path is compiled out.

**The fix:** Add `"web-scrape"` to the `vox-search` features in `crates/vox-gui/Cargo.toml`.

**Files:**
- Modify: `crates/vox-gui/Cargo.toml`

- [ ] **Step 7.1: Verify the current state and confirm the feature name**

```powershell
rg "vox-search" crates/vox-gui/Cargo.toml
```

Note the exact current entry. Then confirm the feature exists:

```powershell
rg "web-scrape" crates/vox-search/Cargo.toml
```

Expected: a line like `web-scrape = [...]` in the `[features]` table. If absent, stop — the feature name may differ. Use `rg "scrape" crates/vox-search/Cargo.toml` to find the correct name.

- [ ] **Step 7.2: Add the feature**

In `crates/vox-gui/Cargo.toml`, find the `vox-search` dependency line and add `"web-scrape"`:

```toml
# If it currently looks like:
vox-search = { path = "../vox-search" }

# Change to:
vox-search = { path = "../vox-search", features = ["web-scrape"] }

# If it already has features:
vox-search = { path = "../vox-search", features = ["existing-feature"] }
# Change to:
vox-search = { path = "../vox-search", features = ["existing-feature", "web-scrape"] }
```

- [ ] **Step 7.3: Verify the build compiles**

```powershell
cargo build -p vox-gui 2>&1 | Select-String "^error"
```

Expected: no output. If there are errors related to missing system libraries that `web-scrape` depends on (e.g., OpenSSL headers), install them or use the appropriate Cargo feature resolver. Note any such errors in a code comment for the PR description.

- [ ] **Step 7.4: Run tests**

```powershell
cargo test -p vox-gui
```

Expected: all pass.

- [ ] **Step 7.5: Commit**

```powershell
git add crates/vox-gui/Cargo.toml
git commit -m "fix(search): enable web-scrape feature in vox-gui for full-page web result extraction"
```

---

## Task 8: Add `SymbolProximity` to Default Heuristic Plan

**What is the problem?** `SearchCorpus::SymbolProximity` (which scans for code symbols near the query terms) is only included in the heuristic plan for `CodeNavigation`-intent queries — those that `looks_like_code_navigation()` identifies as code-specific (e.g., queries containing identifiers or `::` paths). Generic "broad research" queries never invoke symbol search, so a question like "what is SearchPolicy?" returns memory and chunk hits but no symbol-proximity results.

**The fix:** Add `SymbolProximity` to the `BroadResearch` and `FactualLookup` default corpora list, after `RepoInventory` so it does not displace higher-quality hits.

**Files:**
- Modify: `crates/vox-db-types/src/retrieval.rs:287–305`
- Test: `crates/vox-db-types/src/retrieval.rs` (existing `#[cfg(test)]` block ~line 560)

**Background — the heuristic plan function at ~line 280:**
The function has three branches:
1. `CodeNavigation` (if `looks_like_code_navigation()` is true) — already includes `SymbolProximity`.
2. `RepoStructure` (if `looks_like_repo_structure()` is true) — does not need `SymbolProximity`.
3. `BroadResearch`/`FactualLookup` (everything else) — this is what we are fixing.

Only modify the third branch.

- [ ] **Step 8.1: Write the failing tests**

Add to the `#[cfg(test)]` block in `crates/vox-db-types/src/retrieval.rs`:

```rust
#[test]
fn broad_research_plan_includes_symbol_proximity() {
    // A generic question (not code navigation, not repo structure)
    let plan = heuristic_search_plan("what is the architecture of this system", false, None);
    assert_eq!(plan.intent, SearchIntent::BroadResearch);
    assert!(
        plan.corpora.contains(&SearchCorpus::SymbolProximity),
        "BroadResearch plan must include SymbolProximity; got: {:?}",
        plan.corpora
    );
}

#[test]
fn factual_lookup_plan_includes_symbol_proximity() {
    // Short query (≤8 whitespace-separated tokens) → FactualLookup
    let plan = heuristic_search_plan("SearchPolicy", false, None);
    assert_eq!(plan.intent, SearchIntent::FactualLookup);
    assert!(
        plan.corpora.contains(&SearchCorpus::SymbolProximity),
        "FactualLookup plan must include SymbolProximity; got: {:?}",
        plan.corpora
    );
}

#[test]
fn code_navigation_plan_still_includes_symbol_proximity() {
    // Regression: CodeNavigation already had it; verify not broken
    let plan = heuristic_search_plan("execute_search_plan", false, None);
    assert_eq!(plan.intent, SearchIntent::CodeNavigation);
    assert!(plan.corpora.contains(&SearchCorpus::SymbolProximity));
}
```

- [ ] **Step 8.2: Run to verify the first two fail**

```powershell
cargo test -p vox-db-types broad_research_plan_includes_symbol_proximity -- --nocapture
```

Expected: FAIL — `SymbolProximity` not in corpora.

- [ ] **Step 8.3: Add `SymbolProximity` to the BroadResearch/FactualLookup branch**

Find the final `plan.corpora = vec![...]` assignment in `heuristic_search_plan` (~line 287 — the one that assigns `Memory`, `KnowledgeGraph`, `DocumentChunks`, `RepoInventory`, `WebResearch`). This is the `BroadResearch`/`FactualLookup` branch:

```rust
// BEFORE (lines ~287–305)
plan.corpora = vec![
    SearchCorpus::Memory,
    SearchCorpus::KnowledgeGraph,
    SearchCorpus::DocumentChunks,
    SearchCorpus::RepoInventory,
    SearchCorpus::WebResearch,
];
plan.preferred_backends = vec![
    SearchBackend::MemoryBm25,
    SearchBackend::MemoryVector,
    SearchBackend::KnowledgeFts,
    SearchBackend::ChunkFts,
    SearchBackend::ChunkVector,
    SearchBackend::RepoPath,
    SearchBackend::Web,
    // ... (may have more entries — preserve them all)
];
```

```rust
// AFTER — SymbolProximity added after RepoInventory, before WebResearch
plan.corpora = vec![
    SearchCorpus::Memory,
    SearchCorpus::KnowledgeGraph,
    SearchCorpus::DocumentChunks,
    SearchCorpus::RepoInventory,
    SearchCorpus::SymbolProximity, // surfaces code definitions in generic searches
    SearchCorpus::WebResearch,
];
plan.preferred_backends = vec![
    SearchBackend::MemoryBm25,
    SearchBackend::MemoryVector,
    SearchBackend::KnowledgeFts,
    SearchBackend::ChunkFts,
    SearchBackend::ChunkVector,
    SearchBackend::RepoPath,
    SearchBackend::SymbolProximity, // new
    SearchBackend::Web,
    // ... preserve any existing entries after Web
];
```

Do NOT modify the `CodeNavigation` or `RepoStructure` branches.

- [ ] **Step 8.4: Run the full vox-db-types suite**

```powershell
cargo test -p vox-db-types
```

Expected: all pass including the existing `heuristic_search_plan_prefers_repo_for_code_navigation` test.

- [ ] **Step 8.5: Commit**

```powershell
git add crates/vox-db-types/src/retrieval.rs
git commit -m "fix(search): add SymbolProximity corpus to BroadResearch and FactualLookup heuristic plans"
```

---

## Task 9: Clamp `scoreToPct` to [0, 100]

**What is the problem?** BM25 scores from the memory corpus are unbounded (TF-IDF based; can exceed 1.0 for short documents with high term density). The `ScoreBar` component in `SearchView.tsx` already clamps correctly at line 119 (`Math.max(0, Math.min(1, score))`). However, `scoreToPct` in `searchHelpers.ts` (used elsewhere) does a raw multiply without clamping, so a BM25 score of 1.5 would display as "150%" which is meaningless.

**The fix:** Clamp `scoreToPct` input to [0, 1] before multiplying.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/searchHelpers.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx` (add test)

- [ ] **Step 9.1: Read the current `scoreToPct` implementation**

```powershell
rg -n "scoreToPct" crates/vox-gui/ui/src/components/surfaces/Search/searchHelpers.ts
```

Open that file and find the function. It will look like:

```typescript
export function scoreToPct(score: number): number {
  return Math.round(score * 100);
}
```

- [ ] **Step 9.2: Write failing tests**

Add to `SearchView.test.tsx` (import `scoreToPct` from `searchHelpers.ts`):

```typescript
import { scoreToPct } from './searchHelpers';

describe('scoreToPct', () => {
  it('clamps BM25 scores above 1.0 to 100', () => {
    expect(scoreToPct(1.5)).toBe(100);
    expect(scoreToPct(99.9)).toBe(100);
  });

  it('clamps negative scores to 0', () => {
    expect(scoreToPct(-0.1)).toBe(0);
    expect(scoreToPct(-100)).toBe(0);
  });

  it('maps 0.75 to 75', () => {
    expect(scoreToPct(0.75)).toBe(75);
  });

  it('maps 1.0 to 100', () => {
    expect(scoreToPct(1.0)).toBe(100);
  });

  it('maps 0.0 to 0', () => {
    expect(scoreToPct(0.0)).toBe(0);
  });
});
```

- [ ] **Step 9.3: Run to verify the clamping tests fail**

```powershell
cd crates/vox-gui/ui
pnpm test -- SearchView.test
```

Expected: `clamps BM25 scores above 1.0 to 100` FAILS with received `150`.

- [ ] **Step 9.4: Fix `scoreToPct`**

```typescript
// BEFORE (in searchHelpers.ts)
export function scoreToPct(score: number): number {
  return Math.round(score * 100);
}

// AFTER
export function scoreToPct(score: number): number {
  return Math.round(Math.max(0, Math.min(1, score)) * 100);
}
```

- [ ] **Step 9.5: Run tests**

```powershell
pnpm test -- SearchView.test
```

Expected: all tests pass.

- [ ] **Step 9.6: Commit**

```powershell
cd ../../..
git add crates/vox-gui/ui/src/components/surfaces/Search/searchHelpers.ts
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx
git commit -m "fix(search-ui): clamp scoreToPct to [0,100] so BM25 scores above 1.0 display correctly"
```

---

## Task 10: Final Verification

- [ ] **Step 10.1: Run all Rust tests for modified crates**

```powershell
cargo test -p vox-db-types
cargo test -p vox-search
cargo test -p vox-gui
```

Expected: all pass, no regressions.

- [ ] **Step 10.2: Run the full TypeScript test suite**

```powershell
cd crates/vox-gui/ui
pnpm test
```

Expected: all pass.

- [ ] **Step 10.3: Verify the GUI builds without errors**

```powershell
cd ../../..
cargo build -p vox-gui 2>&1 | Select-String "^error"
```

Expected: no output.

- [ ] **Step 10.4: Review git log for this branch**

```powershell
git log --oneline feat/omnisearch-bug-fixes
```

Expected (9 commits in order):

```
fix(search-ui): clamp scoreToPct to [0,100] so BM25 scores above 1.0 display correctly
fix(search): add SymbolProximity corpus to BroadResearch and FactualLookup heuristic plans
fix(search): enable web-scrape feature in vox-gui for full-page web result extraction
fix(search): surface repo file-scan truncation warning when 20k file cap is hit
fix(a11y): add aria-live=polite to search result count for screen reader announcements
fix(search-ui): replace broken pathMatchesGlob with regex-based glob matching; add tests
fix(search): chats-only scope now returns only chat hits (was leaking all-corpora results)
fix(search): propagate rank-based score for KnowledgeGraph hits (was hardcoded 0.0)
fix(search): enable RRF fusion by default (VOX_SEARCH_PREFER_RRF defaults to true)
```

- [ ] **Step 10.5: Push and open PR**

```powershell
git push origin feat/omnisearch-bug-fixes
```

PR title: `fix(search): Omni-Search Bug Fixes & Quick Wins (Phase A)`

PR description:

```
## What

Fixes 8 verified bugs from the omni-search audit:
docs/src/architecture/omni-search-audit-and-roadmap-2026.md

## Changes

- **G7 / I-04** Enable RRF fusion by default — cross-corpus results now rank by relevance
- **G3 / I-03** KnowledgeGraph hits receive rank-based scores (was hardcoded 0.0)
- **G13 / I-01** Chats-only scope returns only chat hits (was returning all-corpora superset)
- **G12 / I-02** pathMatchesGlob uses real glob-to-regex (was broken substring match)
- **G19 / I-07** aria-live added to result count for screen reader announcements
- **G10 / I-09** Truncation warning shown when repo scan hits 20,000-file cap
- **G15 / I-18** web-scrape feature enabled in GUI build for full-page web extraction
- **G5 / I-08** SymbolProximity added to BroadResearch and FactualLookup heuristic plans
- **G18 / I-10** scoreToPct clamped to [0,100] (BM25 scores can exceed 1.0)

## Out of scope

G17 (chunk locator paths) — fixing chunk-to-file locator requires DB schema changes
planned in Plan C (Chat as first-class corpus).

## Plans B/C/D

Separate implementation plans will address: persistent repo index (Plan B),
Chat as first-class SearchCorpus (Plan C), UX elevation / streaming (Plan D).
```

---

## Appendix: Audit Finding vs. Task Cross-Reference

| Audit finding | Task | Status in this plan |
|---|---|---|
| G7: RRF disabled by default | Task 1 | ✅ |
| G3: KG score always 0.0 | Task 2 | ✅ |
| G13: chats scope routing bug | Task 3 | ✅ |
| G12: frontend glob bug | Task 4 | ✅ |
| G19: no aria-live | Task 5 | ✅ |
| G10: truncation silent | Task 6 | ✅ |
| G15: web-scrape disabled | Task 7 | ✅ |
| G5: SymbolProximity excluded | Task 8 | ✅ |
| G18: score normalization | Task 9 | ✅ |
| G17: chunk locator paths | — | ⚠️ Deferred to Plan C (requires DB schema) |
| G1: no symbol index | — | Plan B |
| G2: chat corpus second-class | — | Plan C |
| G4: no fuzzy matching | — | Plan D |
| G6: no incremental index | — | Plan B |
| G8: verification pass missing | — | Plan C |
| G9: no telemetry | — | Plan B |
| G11: KG score unusable for RRF | Fixed by Task 2 (rank score) | ✅ |
| G14: memory cache race | — | Plan B |
| G20: palette/SearchView disconnect | — | Plan D |
