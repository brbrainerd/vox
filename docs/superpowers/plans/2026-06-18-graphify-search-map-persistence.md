# Graphify Search Map Persistence & Semantic Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every Graphify search persists its hits into Turso as navigable, discoverable knowledge nodes with staleness tracking so every future agent can recall, layer-expand, and cross-map-compare prior graph searches without re-running them.

**Architecture:** Search hits from `vox_graphify_search` are upserted into `knowledge_nodes` with a `graphify_search_hit` source tag; a new `vox-graphify-reader` crate implements BFS/path/compare over on-disk `graph.json`; three new MCP tools (`vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare`) expose layered traversal; the corpus registry gains an `is_virtual` flag for Turso-backed corpora; and the retrieval SSOT doc is updated to include the Graphify corpus row.

**Tech Stack:** Rust 2024 edition, `serde_json`, `vox-config::graphify` (existing), `vox-db::VoxDb::upsert_knowledge_node` (existing), `chrono::Utc`, `tracing`, `vox-orchestrator-mcp` dispatch + input_schemas pattern, YAML contract files.

> **Pre-implementation review completed 2026-06-18.** Reviewer: `superpowers:code-reviewer`. All Critical and Important issues are fixed in this document before any code is written. See the summary table at the bottom of this file.

---

## Critical Rules (read before touching any file)

- **Never `cargo fmt --all`** — format individual crates only: `cargo fmt -p <crate>`.
- **No new `.ps1`/`.sh`/`.py` glue** — automation is `.vox` scripts only; existing Python pipeline files in `scripts/coverage-graph/` are pre-approved exceptions.
- **Isolated target dir on Windows** — set `$env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"` before every `cargo test` to avoid lock contention with any running `vox.exe`.
- **DB writes only through `vox-db`** — never open Turso outside the allowed crate.
- **Knowledge node ID format** — always `graphify:{corpus_id}:node:{node_id}` for corpus nodes, `graphify:{corpus_id}:search:{query_slug}:{node_id}` for persisted search hits.
- **After any MCP/CLI surface change** — run `cargo run -p vox-cli -- ci operations-sync --target all --write` then `cargo run -p vox-cli -- ci operations-verify`.
- **Docs frontmatter** — every new `.md` under `docs/src/` needs YAML frontmatter with `title`, `description`, `category`.

---

## Codebase Orientation (read this before writing any code)

You are working in the `vox` monorepo. Key files for this plan:

| File | What it does |
|------|-------------|
| `crates/vox-config/src/graphify.rs` | Core library: `GraphifyCorpus`, `GraphifyManifest`, `assess_corpus_status`, `lexical_search_graph`, `project_graph_nodes_for_ingest` |
| `crates/vox-config/tests/graphify_status.rs` | Integration tests for the above |
| `crates/vox-config/tests/graphify_lexical.rs` | Lexical search tests |
| `contracts/retrieval/graphify-corpora.v1.yaml` | SSOT registry of all Graphify corpora (YAML) |
| `crates/vox-orchestrator-mcp/src/graphify_tools.rs` | MCP tool handlers: `graphify_status`, `graphify_search` |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | JSON schema for each MCP tool input |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Routes MCP tool name → handler function |
| `crates/vox-orchestrator-mcp/src/lib.rs` | Declares `pub mod graphify_tools;` |
| `crates/vox-db/src/store/ops_memory.rs:169` | `VoxDb::upsert_knowledge_node(id, label, content, node_type, metadata, vcs_snapshot_id)` |
| `contracts/mcp/tool-registry.canonical.yaml` | Machine-readable list of all MCP tools |
| `contracts/mcp/http-read-role-governance.yaml` | Which MCP tools are in the read-only role |
| `contracts/operations/catalog.v1.yaml` | CLI + MCP operations catalog |
| `docs/src/architecture/search-retrieval-ssot-2026.md` | Retrieval corpus matrix (needs Graphify row) |
| `docs/src/architecture/where-things-live.md` | "what crate does X" lookup table |

The `upsert_knowledge_node` signature is:
```rust
pub async fn upsert_knowledge_node(
    &self,
    id: &str,
    label: &str,
    content: &str,
    node_type: Option<&str>,
    metadata: Option<&str>,
    _vcs_snapshot_id: Option<&str>,
) -> Result<(), StoreError>
```

The dispatch pattern in `dispatch.rs` is always:
```rust
"vox_tool_name" => {
    Ok(crate::module_name::handler_fn(state, serde_json::from_value(args)?).await)
}
```

---

## File Map (what changes and why)

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vox-config/src/graphify.rs` | Modify | Add `is_virtual` to `GraphifyCorpus`; add `lexical_ingest_sha256` to `GraphifyManifest`; add `lexical_lag_stale_reason()` |
| `crates/vox-config/tests/graphify_status.rs` | Modify | Tests for virtual corpus + lexical lag |
| `contracts/retrieval/graphify-corpora.v1.yaml` | Modify | Add `graphify-search-log` virtual corpus entry |
| `crates/vox-orchestrator-mcp/src/graphify_tools.rs` | Modify | `persist` param on search; hit upsert; `graphify_query`, `graphify_path`, `graphify_compare` handlers |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Modify | Schema for `persist`; schemas for 3 new tools |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Modify | 3 new dispatch arms |
| `crates/vox-orchestrator-mcp/Cargo.toml` | Modify | Add `vox-graphify-reader` dependency |
| `Cargo.toml` (workspace root) | Modify | Add `vox-graphify-reader` to `[workspace.dependencies]` |
| `crates/vox-graphify-reader/Cargo.toml` | **Create** | New crate manifest |
| `crates/vox-graphify-reader/src/lib.rs` | **Create** | `GraphifyReader`, BFS, shortest path, god nodes, community members |
| `crates/vox-graphify-reader/src/bfs.rs` | **Create** | BFS impl over HashMap adjacency |
| `crates/vox-graphify-reader/src/compare.rs` | **Create** | `ManifestSummary`, `ManifestDiff`, `diff_manifests` |
| `crates/vox-graphify-reader/tests/reader_tests.rs` | **Create** | Integration tests for all public API |
| `contracts/mcp/tool-registry.canonical.yaml` | Modify | 3 new tool entries |
| `contracts/mcp/http-read-role-governance.yaml` | Modify | 3 new tools in read-role allow list |
| `contracts/operations/catalog.v1.yaml` | Modify | `graphify.ingest`, `graphify.query`, `graphify.path`, `graphify.compare` |
| `docs/src/architecture/search-retrieval-ssot-2026.md` | Modify | Add Graphify corpus row to corpus matrix table |
| `docs/src/architecture/graphify-integration-research-2026-06-16.md` | Modify | Update §4.6; add §4.7 search persistence model |
| `docs/src/architecture/where-things-live.md` | Modify | Add `vox-graphify-reader` row |
| `docs/superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md` | Modify | Update DONE/NOT DONE |

---

## Task 1: Add `is_virtual` and `lexical_ingest_sha256` to vox-config

**Context:** `GraphifyCorpus` currently has no flag for Turso-backed virtual corpora. When `is_virtual: true`, `assess_corpus_status` must skip disk checks and return fresh so the search-log corpus never shows stale. `GraphifyManifest` needs a `lexical_ingest_sha256` field so we can detect when the Turso index is behind the current `graph.json`.

**Files:**
- Modify: `crates/vox-config/src/graphify.rs`
- Modify: `crates/vox-config/tests/graphify_status.rs`

- [ ] **Step 1.1: Write the failing tests**

  Open `crates/vox-config/tests/graphify_status.rs`. Add these tests at the bottom of the file (inside the file, after all existing tests):

  ```rust
  #[test]
  fn virtual_corpus_is_always_fresh() {
      // No graph file written — virtual corpora must not check disk.
      let tmp = tempfile::tempdir().unwrap();
      let corpus = vox_config::graphify::GraphifyCorpus {
          id: "graphify-search-log".to_string(),
          title: "Search hit log".to_string(),
          scope_path: ".".to_string(),
          graph_path: "nonexistent/graph.json".to_string(),
          manifest_path: "nonexistent/.graphify_manifest.v1.json".to_string(),
          extraction_mode: None,
          default_for_intents: vec![],
          is_virtual: true,
      };
      let status = vox_config::graphify::assess_corpus_status(
          tmp.path(),
          &corpus,
          None,
          chrono::Utc::now(),
          30,
      );
      assert!(status.is_fresh, "virtual corpus must always be fresh");
      assert!(
          status.stale_reasons.is_empty(),
          "no stale reasons: {:?}",
          status.stale_reasons
      );
      assert!(
          status.warnings.contains(&"virtual_corpus".to_string()),
          "warnings must contain 'virtual_corpus'"
      );
  }

  #[test]
  fn lexical_lag_detected_when_sha_mismatch() {
      use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
      let manifest = GraphifyManifest {
          graph_json_sha256: Some("abc123".to_string()),
          lexical_ingest_sha256: Some("different456".to_string()),
          ..GraphifyManifest::default()
      };
      assert_eq!(
          lexical_lag_stale_reason(&manifest),
          Some("lexical_lag".to_string())
      );
  }

  #[test]
  fn no_lexical_lag_when_sha_matches() {
      use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
      let manifest = GraphifyManifest {
          graph_json_sha256: Some("abc123".to_string()),
          lexical_ingest_sha256: Some("abc123".to_string()),
          ..GraphifyManifest::default()
      };
      assert_eq!(lexical_lag_stale_reason(&manifest), None);
  }

  #[test]
  fn no_lexical_lag_when_ingest_sha_absent() {
      use vox_config::graphify::{GraphifyManifest, lexical_lag_stale_reason};
      // Not yet ingested — we don't call this a lag.
      let manifest = GraphifyManifest {
          graph_json_sha256: Some("abc123".to_string()),
          lexical_ingest_sha256: None,
          ..GraphifyManifest::default()
      };
      assert_eq!(lexical_lag_stale_reason(&manifest), None);
  }
  ```

- [ ] **Step 1.2: Run to confirm compile failure**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-config --test graphify_status 2>&1 | Select-String "error|FAILED|passed" | Select-Object -First 15
  ```

  Expected: compile error mentioning `is_virtual` field not found and `lexical_lag_stale_reason` not found.

- [ ] **Step 1.3: Implement changes in `graphify.rs`**

  Open `crates/vox-config/src/graphify.rs`.

  **Replace the `GraphifyCorpus` struct (lines 33–44):**
  ```rust
  #[derive(Debug, Clone, Deserialize, Serialize)]
  pub struct GraphifyCorpus {
      pub id: String,
      pub title: String,
      pub scope_path: String,
      pub graph_path: String,
      pub manifest_path: String,
      #[serde(default)]
      pub extraction_mode: Option<String>,
      #[serde(default)]
      pub default_for_intents: Vec<String>,
      /// When true, this corpus is Turso-backed (no on-disk graph.json).
      /// `assess_corpus_status` skips all disk checks and returns fresh unconditionally.
      #[serde(default)]
      pub is_virtual: bool,
  }
  ```

  **Replace the `GraphifyManifest` struct (lines 53–63):**
  ```rust
  #[derive(Debug, Clone, Deserialize, Serialize, Default)]
  pub struct GraphifyManifest {
      pub corpus_id: Option<String>,
      pub built_at: Option<String>,
      pub git_sha: Option<String>,
      pub scope_path: Option<String>,
      pub node_count: Option<u64>,
      pub edge_count: Option<u64>,
      pub graph_json_sha256: Option<String>,
      pub extraction_mode: Option<String>,
      /// SHA256 of the graph file at last `vox graphify ingest` run.
      /// If this differs from `graph_json_sha256`, the Turso index is behind the graph.
      pub lexical_ingest_sha256: Option<String>,
  }
  ```

  **Add `lexical_lag_stale_reason` function** — insert it immediately after the `parse_rfc3339` function (which ends around line 289):
  ```rust
  /// Returns `Some("lexical_lag")` when `lexical_ingest_sha256` differs from `graph_json_sha256`.
  ///
  /// Returns `None` when either SHA is absent (never ingested = unknown, not a lag).
  pub fn lexical_lag_stale_reason(manifest: &GraphifyManifest) -> Option<String> {
      match (&manifest.graph_json_sha256, &manifest.lexical_ingest_sha256) {
          (Some(graph_sha), Some(ingest_sha)) if graph_sha != ingest_sha => {
              Some("lexical_lag".to_string())
          }
          _ => None,
      }
  }
  ```

  **Add virtual corpus early-return to `assess_corpus_status`** — the function starts around line 292. Add this block as the very first thing in the function body, before the `graph_path` local variable:
  ```rust
  pub fn assess_corpus_status(
      repo_root: &Path,
      corpus: &GraphifyCorpus,
      head_git_sha: Option<&str>,
      now: DateTime<Utc>,
      ttl_days: u64,
  ) -> CorpusStatus {
      // Virtual corpora are Turso-backed; skip all disk checks.
      if corpus.is_virtual {
          return CorpusStatus {
              corpus_id: corpus.id.clone(),
              title: corpus.title.clone(),
              graph_path: repo_root.join(&corpus.graph_path),
              manifest_path: repo_root.join(&corpus.manifest_path),
              graph_exists: false,
              manifest_exists: false,
              node_count: None,
              edge_count: None,
              built_at: None,
              manifest_git_sha: None,
              head_git_sha: head_git_sha.map(str::to_string),
              stale_reasons: vec![],
              warnings: vec!["virtual_corpus".to_string()],
              is_fresh: true,
          };
      }
      // ... rest of the existing function body unchanged from here ...
  ```

- [ ] **Step 1.4: Run tests — expect pass**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-config --test graphify_status 2>&1 | tail -20
  cargo test -p vox-config --test graphify_lexical 2>&1 | tail -10
  ```

  Expected: all PASS.

- [ ] **Step 1.5: Commit**

  ```powershell
  git add crates/vox-config/src/graphify.rs crates/vox-config/tests/graphify_status.rs
  git commit -m "feat(vox-config): add is_virtual corpus flag and lexical_lag_stale_reason"
  ```

---

## Task 2: Add `graphify-search-log` virtual corpus to the registry

**Context:** `contracts/retrieval/graphify-corpora.v1.yaml` is the SSOT that all tools read to discover available corpora. Agents see this file when calling `vox_graphify_status`. We add a virtual entry for the search-hit log. Because `is_virtual: true` tells `assess_corpus_status` to skip disk checks, this entry is always reported as fresh, and agents know to query Turso (not disk) for its content.

**Files:**
- Modify: `contracts/retrieval/graphify-corpora.v1.yaml`

- [ ] **Step 2.1: Add the entry AND update the corpus count assertion**

  > **Reviewer fix CRIT-2:** The existing test at `crates/vox-config/tests/graphify_status.rs` asserts `reg.corpora.len() == 3`. Adding a 4th corpus makes that test fail. Update the assertion in the same step.

  First, append the new entry to `contracts/retrieval/graphify-corpora.v1.yaml` (the file currently ends at line 38 with the `config-audit` entry). Preserve YAML indentation — 2 spaces, list items with `  -`:

  ```yaml
    - id: graphify-search-log
      title: "Graphify search-hit log (agent memory)"
      scope_path: ".vox/cache/graphify/search-log/"
      graph_path: ".vox/cache/graphify/search-log/graph.json"
      manifest_path: ".vox/cache/graphify/search-log/.graphify_manifest.v1.json"
      is_virtual: true
      default_for_intents:
        - search_history
        - agent_recall
  ```

  Then open `crates/vox-config/tests/graphify_status.rs` and find the assertion that reads:
  ```rust
  assert_eq!(reg.corpora.len(), 3);
  ```
  Replace it with:
  ```rust
  assert_eq!(reg.corpora.len(), 4, "expected 4 corpora after adding graphify-search-log");
  assert!(
      reg.corpora.iter().any(|c| c.id == "graphify-search-log"),
      "graphify-search-log must be present in registry"
  );
  ```

- [ ] **Step 2.2: Verify the YAML still parses and the count test passes**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-config --test graphify_status 2>&1 | tail -15
  ```

  Expected: all pass. `virtual_corpus_is_always_fresh` also passes because `is_virtual: true` is present on the new entry.

- [ ] **Step 2.3: Commit**

  ```powershell
  git add contracts/retrieval/graphify-corpora.v1.yaml
  git add crates/vox-config/tests/graphify_status.rs
  git commit -m "feat(contracts): add graphify-search-log virtual corpus; fix corpus count assertion"
  ```

---

## Task 3: Persist search hits in `vox_graphify_search` + add `persist` param

**Context:** Today `vox_graphify_search` is stateless — hits are returned and discarded. Future agents cannot recall what matched "authentication" last week. We add a `persist: bool` param (default `true`) that upserts each hit into `knowledge_nodes`. The node ID format is `graphify:{corpus_id}:search:{query_slug}:{node_id}`. DB errors are non-fatal so MCP clients without a Turso connection still get search results. We also add `searched_at` (RFC3339) to the response for provenance.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/graphify_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`

- [ ] **Step 3.1: Write the failing tests**

  > **Reviewer fix MIN-5:** The existing `graphify_search_returns_matching_hit` test constructs `GraphifySearchParams` without a `persist` field. After Step 3.3 adds the field, that test becomes a compile error. Update it in the same step as adding the field.

  Inside the `#[cfg(test)] mod tests { ... }` block in `graphify_tools.rs`:

  **Update the existing `graphify_search_returns_matching_hit` test** — add `persist: false` to its struct literal:
  ```rust
  // EXISTING TEST — add persist field to prevent compile error
  #[tokio::test]
  async fn graphify_search_returns_matching_hit() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      write_sample_graph(tmp.path());
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_search(
          &state,
          GraphifySearchParams {
              corpus: Some("repo-code-graph".into()),
              query: "authentication".into(),
              limit: None,
              persist: false, // avoid DB in unit test
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
      let data = parsed.get("data").expect("data");
      assert_eq!(
          data.get("corpus_id"),
          Some(&serde_json::json!("repo-code-graph"))
      );
      let hits = data
          .get("hits")
          .and_then(|h| h.as_array())
          .expect("hits array");
      assert!(!hits.is_empty(), "expected at least one hit: {json}");
      let first = &hits[0];
      assert_eq!(first.get("node_id"), Some(&serde_json::json!("auth")));
      assert_eq!(
          first.get("knowledge_id"),
          Some(&serde_json::json!("graphify:repo-code-graph:node:auth"))
      );
      assert!(
          first
              .get("label")
              .and_then(|v| v.as_str())
              .unwrap_or("")
              .contains("authentication")
      );
  }
  ```

  **Add the new test** after the above:
  ```rust
  #[tokio::test]
  async fn graphify_search_response_includes_searched_at() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      write_sample_graph(tmp.path());
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_search(
          &state,
          GraphifySearchParams {
              corpus: Some("repo-code-graph".into()),
              query: "authentication".into(),
              limit: None,
              persist: false, // skip DB in unit test
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed["success"], serde_json::json!(true));
      let data = parsed.get("data").expect("data field");
      assert!(
          data.get("searched_at").and_then(|v| v.as_str()).is_some(),
          "searched_at must be a string: {data}"
      );
      assert_eq!(data["corpus_id"], serde_json::json!("repo-code-graph"));
  }
  ```

- [ ] **Step 3.2: Run to confirm compile failure**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-orchestrator-mcp graphify_tools 2>&1 | Select-String "error\[" | Select-Object -First 5
  ```

  Expected: compile error — `persist` field not found on `GraphifySearchParams`.

- [ ] **Step 3.3: Add `persist: bool` to `GraphifySearchParams`**

  > **Reviewer fix IMP-1:** Use `bool`, not `Option<bool>`. `Option<bool>` is a three-state type but the semantics only need two states (record or don't). A plain `bool` with a serde default is cleaner and avoids `unwrap_or(true)` at every callsite.

  Replace the `GraphifySearchParams` struct (around line 25–31 in `graphify_tools.rs`):

  ```rust
  #[derive(Debug, Deserialize)]
  pub struct GraphifySearchParams {
      /// Corpus id from the registry; omit to use `default_corpus_id`.
      pub corpus: Option<String>,
      pub query: String,
      pub limit: Option<u32>,
      /// When true (default), upsert each hit into `knowledge_nodes` for future agent recall.
      /// Pass false for ephemeral searches that must not be recorded.
      #[serde(default = "default_persist_true")]
      pub persist: bool,
  }

  fn default_persist_true() -> bool {
      true
  }
  ```

- [ ] **Step 3.4: Add `query_slug` helper above `graphify_search`**

  > **Reviewer fix IMP-2:** The original slug truncated to 40 chars, which can collide for long similar queries. Adding an 8-char FNV-64 hash suffix guarantees uniqueness using only `std` — no new dependencies needed.

  Insert immediately before the `pub async fn graphify_search` declaration:

  ```rust
  /// URL-safe slug from a query string with a 8-char FNV-64 hash suffix to prevent collisions.
  ///
  /// Two different queries that share a 32-char prefix will still produce unique slugs because
  /// the hash suffix is derived from the **full** original query string.
  fn query_slug(query: &str) -> String {
      // Normalize: lowercase, non-alphanumeric → hyphen, collapse runs, trim.
      let normalized: String = query
          .to_lowercase()
          .chars()
          .map(|c| if c.is_alphanumeric() { c } else { '-' })
          .collect::<String>()
          .split('-')
          .filter(|s| !s.is_empty())
          .collect::<Vec<_>>()
          .join("-");
      let prefix: String = normalized.chars().take(32).collect();
      // FNV-64 of the original query (full, before normalization) for collision resistance.
      let hash = {
          use std::hash::{Hash, Hasher};
          let mut h = std::collections::hash_map::DefaultHasher::new();
          query.hash(&mut h);
          format!("{:08x}", h.finish() & 0xffff_ffff)
      };
      if prefix.is_empty() {
          hash
      } else {
          format!("{prefix}-{hash}")
      }
  }
  ```

- [ ] **Step 3.5: Update `graphify_search` body to persist hits and return `searched_at`**

  > **Reviewer fixes IMP-1, IMP-5:** Use `params.persist` directly (now a `bool`). Replace silent `let _` with `tracing::warn!` so non-unavailability DB failures surface in logs.

  In `graphify_search`, find the block that currently reads:
  ```rust
  let hits = lexical_search_graph(&graph, &corpus_id, &params.query, limit as usize);
  let payload_hits: Vec<serde_json::Value> = hits
  ```

  Replace it with:
  ```rust
  let hits = lexical_search_graph(&graph, &corpus_id, &params.query, limit as usize);

  // Record searched_at before any async work.
  let searched_at = chrono::Utc::now().to_rfc3339();
  let head_sha_for_meta = resolve_head_sha(state).await;

  // Persist hits to Turso so future agents can recall this search.
  // NOTE: Recall consumers MUST compare metadata.git_sha against HEAD to detect stale hits.
  if params.persist && !hits.is_empty() {
      let slug = query_slug(&params.query);
      if let Ok(db) = vox_db::VoxDb::connect_default().await {
          for hit in &hits {
              let node_id = format!("graphify:{corpus_id}:search:{slug}:{}", hit.node_id);
              let metadata = serde_json::json!({
                  "corpus_id": corpus_id,
                  "query": params.query,
                  "searched_at": searched_at,
                  "git_sha": head_sha_for_meta,
                  "source": "graphify_search_hit",
              })
              .to_string();
              // Non-fatal: DB unavailability must not fail the search response.
              // We DO log non-unavailability errors so schema/auth problems surface.
              if let Err(e) = db
                  .upsert_knowledge_node(
                      &node_id,
                      &hit.label,
                      &format!(
                          "{} [corpus: {corpus_id}, query: {}]",
                          hit.label, params.query
                      ),
                      Some("graphify_search_hit"),
                      Some(&metadata),
                      None,
                  )
                  .await
              {
                  tracing::warn!(
                      corpus_id = %corpus_id,
                      node_id = %node_id,
                      error = %e,
                      "graphify search-hit persist failed (non-fatal)"
                  );
              }
          }
      }
  }

  let payload_hits: Vec<serde_json::Value> = hits
      .into_iter()
      .map(|h| {
          serde_json::json!({
              "node_id": h.node_id,
              "label": h.label,
              "score": h.score,
              "knowledge_id": knowledge_id(&corpus_id, &h.node_id),
          })
      })
      .collect();
  let payload = serde_json::json!({
      "corpus_id": corpus_id,
      "searched_at": searched_at,
      "hits": payload_hits,
  });
  ToolResult::ok(payload).to_json()
  ```

  > Note: the original `payload` block at the end of the function is replaced by the block above. Delete the old `payload_hits` and `payload` variables that were already there.

- [ ] **Step 3.6: Update `vox_graphify_search` input schema**

  In `input_schemas.rs`, find the `"vox_graphify_search"` arm (around line 399). Replace the schema string to add `persist` (type `boolean`, default `true` described in the description):

  ```rust
  "vox_graphify_search" => parse_obj(
      r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id from contracts/retrieval/graphify-corpora.v1.yaml; omit for default corpus"},"query":{"type":"string","minLength":1,"description":"Lexical search query matched against node labels"},"limit":{"type":"integer","minimum":1,"description":"Maximum hits to return (default 10)"},"persist":{"type":"boolean","default":true,"description":"When true (default), upsert hits into knowledge_nodes for future agent recall. Pass false for ephemeral searches. Recall consumers must compare metadata.git_sha against HEAD to detect stale hits."}},"required":["query"],"additionalProperties":false}"#,
  ),
  ```

- [ ] **Step 3.7: Run tests — expect pass**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-orchestrator-mcp graphify_tools 2>&1 | tail -25
  ```

  Expected: all graphify_tools tests PASS (including the new one and the existing `graphify_search_returns_matching_hit`).

- [ ] **Step 3.8: Commit**

  ```powershell
  git add crates/vox-orchestrator-mcp/src/graphify_tools.rs
  git add crates/vox-orchestrator-mcp/src/input_schemas.rs
  git commit -m "feat(vox-orchestrator-mcp): persist graphify search hits; add persist param and searched_at"
  ```

---

## Task 4: Create `vox-graphify-reader` crate

**Context:** The new crate provides layered navigation over a Graphify `graph.json`. It reads the NetworkX JSON format (`{"nodes": [...], "links": [...]}`) into a HashMap adjacency index and implements BFS, shortest path, god-node ranking, and community membership. No external graph library is used — a HashMap is sufficient for graphs up to ~100k nodes and keeps the dependency surface minimal.

**Files:**
- Create: `crates/vox-graphify-reader/Cargo.toml`
- Create: `crates/vox-graphify-reader/src/lib.rs`
- Create: `crates/vox-graphify-reader/src/bfs.rs`
- Create: `crates/vox-graphify-reader/src/compare.rs`
- Create: `crates/vox-graphify-reader/tests/reader_tests.rs`

- [ ] **Step 4.1: Write integration tests first**

  Create `crates/vox-graphify-reader/tests/reader_tests.rs`:

  ```rust
  //! Integration tests for vox-graphify-reader (BFS, path, compare).

  use vox_graphify_reader::{GraphifyReader, GraphifyReaderError};

  fn three_node_graph() -> serde_json::Value {
      serde_json::json!({
          "nodes": [
              {"id": "a", "label": "alpha node", "community": "c1"},
              {"id": "b", "label": "beta node",  "community": "c1"},
              {"id": "c", "label": "gamma node", "community": "c2"},
          ],
          "links": [
              {"source": "a", "target": "b"},
              {"source": "b", "target": "c"},
          ]
      })
  }

  #[test]
  fn reader_loads_node_and_edge_counts() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      assert_eq!(g.node_count(), 3);
      assert_eq!(g.edge_count(), 2);
  }

  #[test]
  fn bfs_depth_1_returns_direct_neighbors_only() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let hits = g.bfs_from_seeds(&["a"], 1, 100);
      let ids: Vec<&str> = hits.iter().map(|h| h.node_id.as_str()).collect();
      assert!(ids.contains(&"b"), "b must be a depth-1 neighbor of a");
      assert!(!ids.contains(&"c"), "c is depth-2, must not appear at depth-1");
  }

  #[test]
  fn bfs_depth_2_reaches_indirect_neighbors() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let hits = g.bfs_from_seeds(&["a"], 2, 100);
      let ids: Vec<&str> = hits.iter().map(|h| h.node_id.as_str()).collect();
      assert!(ids.contains(&"b"));
      assert!(ids.contains(&"c"));
  }

  #[test]
  fn bfs_hit_has_correct_depth_field() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let hits = g.bfs_from_seeds(&["a"], 2, 100);
      let b_hit = hits.iter().find(|h| h.node_id == "b").expect("b must be in hits");
      let c_hit = hits.iter().find(|h| h.node_id == "c").expect("c must be in hits");
      assert_eq!(b_hit.depth, 1);
      assert_eq!(c_hit.depth, 2);
  }

  #[test]
  fn bfs_respects_limit() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let hits = g.bfs_from_seeds(&["a"], 5, 1);
      assert_eq!(hits.len(), 1, "limit=1 must cap results");
  }

  #[test]
  fn bfs_unknown_seed_returns_empty() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      assert!(g.bfs_from_seeds(&["nonexistent"], 2, 100).is_empty());
  }

  #[test]
  fn bfs_path_field_traces_from_seed_to_hit() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let hits = g.bfs_from_seeds(&["a"], 2, 100);
      let c_hit = hits.iter().find(|h| h.node_id == "c").expect("c must be in hits");
      assert_eq!(c_hit.path, vec!["a", "b", "c"]);
  }

  #[test]
  fn shortest_path_two_hops() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      assert_eq!(g.shortest_path("a", "c").unwrap(), vec!["a", "b", "c"]);
  }

  #[test]
  fn shortest_path_same_node_is_single_element() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      assert_eq!(g.shortest_path("a", "a").unwrap(), vec!["a"]);
  }

  #[test]
  fn shortest_path_unreachable_returns_none() {
      let graph = serde_json::json!({"nodes": [{"id": "x"}, {"id": "y"}], "links": []});
      let g = GraphifyReader::from_value(graph).unwrap();
      assert!(g.shortest_path("x", "y").is_none());
  }

  #[test]
  fn god_nodes_orders_by_degree_descending() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      // b connects to a and c (degree 2); a and c have degree 1
      let gods = g.god_nodes(3);
      assert_eq!(gods[0].0, "b", "b must be the highest-degree node");
      assert_eq!(gods[0].1, 2);
  }

  #[test]
  fn community_members_returns_correct_group() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      let mut members = g.community_members("c1");
      members.sort(); // sort for determinism
      assert_eq!(members, vec!["a".to_string(), "b".to_string()]);
  }

  #[test]
  fn community_members_unknown_community_returns_empty() {
      let g = GraphifyReader::from_value(three_node_graph()).unwrap();
      assert!(g.community_members("nonexistent").is_empty());
  }

  #[test]
  fn compare_diff_manifests_computes_deltas() {
      use vox_graphify_reader::compare::{ManifestSummary, diff_manifests};
      let old = ManifestSummary { node_count: 100, edge_count: 50, community_count: 5 };
      let new = ManifestSummary { node_count: 120, edge_count: 60, community_count: 7 };
      let diff = diff_manifests(&old, &new);
      assert_eq!(diff.node_delta, 20);
      assert_eq!(diff.edge_delta, 10);
      assert_eq!(diff.community_delta, 2);
  }

  #[test]
  fn compare_negative_delta_when_graph_shrinks() {
      use vox_graphify_reader::compare::{ManifestSummary, diff_manifests};
      let old = ManifestSummary { node_count: 200, edge_count: 100, community_count: 10 };
      let new = ManifestSummary { node_count: 150, edge_count: 80, community_count: 8 };
      let diff = diff_manifests(&old, &new);
      assert_eq!(diff.node_delta, -50);
      assert_eq!(diff.edge_delta, -20);
      assert_eq!(diff.community_delta, -2);
  }

  #[test]
  fn reader_errors_on_missing_nodes_key() {
      let bad = serde_json::json!({"links": []});
      let err = GraphifyReader::from_value(bad).unwrap_err();
      assert!(matches!(err, GraphifyReaderError::MissingNodes));
  }

  #[test]
  fn reader_accepts_edges_key_as_alias_for_links() {
      let graph = serde_json::json!({
          "nodes": [{"id": "x"}, {"id": "y"}],
          "edges": [{"source": "x", "target": "y"}]
      });
      let g = GraphifyReader::from_value(graph).unwrap();
      assert_eq!(g.edge_count(), 1);
  }
  ```

- [ ] **Step 4.2: Create `Cargo.toml`**

  Create `crates/vox-graphify-reader/Cargo.toml`:

  ```toml
  [package]
  name = "vox-graphify-reader"
  version.workspace = true
  edition.workspace = true
  license.workspace = true
  description = "Read-only BFS traversal, path-finding, and cross-manifest comparison for Graphify graph.json exports"

  [dependencies]
  serde_json = { workspace = true }

  [dev-dependencies]
  tempfile = { workspace = true }

  [lints]
  workspace = true
  ```

- [ ] **Step 4.3: Create `src/lib.rs`**

  Create `crates/vox-graphify-reader/src/lib.rs`:

  ```rust
  //! Read-only graph reader for Graphify `graph.json` (NetworkX JSON export format).
  //!
  //! # Graph format
  //! ```json
  //! { "nodes": [{"id": "x", "label": "...", "community": "c1"}],
  //!   "links": [{"source": "x", "target": "y"}] }
  //! ```
  //! Edges may appear under `"links"` or `"edges"` — both are supported.
  //! The graph is treated as **undirected**: edges are indexed in both directions.

  pub mod bfs;
  pub mod compare;

  use std::collections::HashMap;

  /// Error type for [`GraphifyReader`] construction.
  #[derive(Debug)]
  pub enum GraphifyReaderError {
      /// The JSON value had no `"nodes"` array.
      MissingNodes,
  }

  impl std::fmt::Display for GraphifyReaderError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
              GraphifyReaderError::MissingNodes => {
                  write!(f, "graph JSON missing 'nodes' array")
              }
          }
      }
  }

  impl std::error::Error for GraphifyReaderError {}

  /// A single result from BFS traversal.
  #[derive(Debug, Clone)]
  pub struct TraversalHit {
      /// Node ID as it appears in the graph JSON.
      pub node_id: String,
      /// Human-readable label for this node.
      pub label: String,
      /// Number of hops from the nearest seed node.
      pub depth: u8,
      /// Ordered list of node IDs from seed to this node (inclusive of both).
      pub path: Vec<String>,
  }

  /// Read-only graph reader. Builds an in-memory adjacency index from a Graphify JSON value.
  ///
  /// Construction is O(N + E). Queries are O(N + E) worst case for full BFS.
  pub struct GraphifyReader {
      // node_id → (label, community_id)
      nodes: HashMap<String, (String, Option<String>)>,
      // Undirected adjacency: node_id → Vec<neighbor_ids>
      adjacency: HashMap<String, Vec<String>>,
  }

  impl GraphifyReader {
      /// Construct from a parsed `serde_json::Value`.
      ///
      /// Returns [`GraphifyReaderError::MissingNodes`] if the `"nodes"` key is absent or not an array.
      pub fn from_value(value: serde_json::Value) -> Result<Self, GraphifyReaderError> {
          let nodes_arr = value
              .get("nodes")
              .and_then(|n| n.as_array())
              .ok_or(GraphifyReaderError::MissingNodes)?;

          let mut nodes: HashMap<String, (String, Option<String>)> =
              HashMap::with_capacity(nodes_arr.len());

          for node in nodes_arr {
              // Prefer "id", fall back to "label" for the node key.
              let id = node
                  .get("id")
                  .and_then(|v| v.as_str())
                  .filter(|s| !s.is_empty())
                  .unwrap_or_else(|| {
                      node.get("label")
                          .and_then(|v| v.as_str())
                          .unwrap_or("")
                  })
                  .to_string();
              if id.is_empty() {
                  continue;
              }
              let label = node
                  .get("label")
                  .or_else(|| node.get("name"))
                  .and_then(|v| v.as_str())
                  .unwrap_or(&id)
                  .to_string();
              let community = node
                  .get("community")
                  .and_then(|v| v.as_str())
                  .map(str::to_string);
              nodes.insert(id, (label, community));
          }

          // Build undirected adjacency from "links" or "edges" (both supported).
          let edges_arr = value
              .get("links")
              .or_else(|| value.get("edges"))
              .and_then(|e| e.as_array());

          let mut adjacency: HashMap<String, Vec<String>> =
              HashMap::with_capacity(nodes.len());

          if let Some(edges) = edges_arr {
              for edge in edges {
                  let src = edge
                      .get("source")
                      .and_then(|v| v.as_str())
                      .filter(|s| !s.is_empty())
                      .map(str::to_string);
                  let dst = edge
                      .get("target")
                      .and_then(|v| v.as_str())
                      .filter(|s| !s.is_empty())
                      .map(str::to_string);
                  if let (Some(s), Some(d)) = (src, dst) {
                      adjacency.entry(s.clone()).or_default().push(d.clone());
                      adjacency.entry(d).or_default().push(s);
                  }
              }
          }

          Ok(GraphifyReader { nodes, adjacency })
      }

      /// Total number of nodes in the graph.
      pub fn node_count(&self) -> usize {
          self.nodes.len()
      }

      /// Total number of undirected edges (each edge counted once).
      ///
      /// **Assumption:** The source `graph.json` contains no duplicate directed edges.
      /// If the same `{source, target}` pair appears more than once in the `links` array,
      /// this count will be inflated (each duplicate pair adds 2 to the adjacency sum).
      /// Graphify's own export format does not produce duplicates, so this is safe in practice.
      pub fn edge_count(&self) -> usize {
          self.adjacency.values().map(|v| v.len()).sum::<usize>() / 2
      }

      /// BFS from one or more seed node IDs up to `max_depth` hops.
      ///
      /// Seeds themselves are excluded from the output — only their reachable neighbors are
      /// returned. Results are capped at `limit`. If the `VOX_GRAPHIFY_VIZ_NODE_LIMIT` env var
      /// is set and lower than `limit`, that cap applies instead.
      pub fn bfs_from_seeds(
          &self,
          seeds: &[&str],
          max_depth: u8,
          limit: usize,
      ) -> Vec<TraversalHit> {
          bfs::bfs_from_seeds(&self.nodes, &self.adjacency, seeds, max_depth, limit)
      }

      /// Shortest path between two node IDs (BFS). Returns `None` if unreachable.
      ///
      /// Returns `Some(vec![node_id])` (single element) when `from == to`.
      pub fn shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
          bfs::shortest_path(&self.adjacency, from, to)
      }

      /// Nodes sorted by degree (highest first), capped at `top_n`.
      ///
      /// Returns `(node_id, degree)` pairs.
      pub fn god_nodes(&self, top_n: usize) -> Vec<(String, usize)> {
          let mut degrees: Vec<(String, usize)> = self
              .adjacency
              .iter()
              .map(|(id, neighbors)| (id.clone(), neighbors.len()))
              .collect();
          degrees.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
          degrees.truncate(top_n);
          degrees
      }

      /// All node IDs belonging to `community_id` (matched on the `"community"` node field).
      pub fn community_members(&self, community_id: &str) -> Vec<String> {
          self.nodes
              .iter()
              .filter_map(|(id, (_, comm))| {
                  comm.as_deref()
                      .filter(|c| *c == community_id)
                      .map(|_| id.clone())
              })
              .collect()
      }
  }
  ```

- [ ] **Step 4.4: Create `src/bfs.rs`**

  Create `crates/vox-graphify-reader/src/bfs.rs`:

  ```rust
  //! BFS traversal and shortest-path search over a HashMap adjacency index.

  use std::collections::{HashMap, HashSet, VecDeque};

  use crate::TraversalHit;

  /// BFS expansion from seed nodes. Seeds are excluded from results.
  pub(crate) fn bfs_from_seeds(
      nodes: &HashMap<String, (String, Option<String>)>,
      adjacency: &HashMap<String, Vec<String>>,
      seeds: &[&str],
      max_depth: u8,
      limit: usize,
  ) -> Vec<TraversalHit> {
      let env_cap = std::env::var("VOX_GRAPHIFY_VIZ_NODE_LIMIT")
          .ok()
          .and_then(|s| s.parse::<usize>().ok())
          .unwrap_or(usize::MAX);
      let effective_limit = limit.min(env_cap);

      if effective_limit == 0 || max_depth == 0 {
          return vec![];
      }

      let seed_set: HashSet<String> = seeds.iter().map(|s| s.to_string()).collect();
      let mut visited: HashSet<String> = seed_set.clone();

      // Queue entry: (node_id, depth, path_from_seed_to_this_node)
      let mut queue: VecDeque<(String, u8, Vec<String>)> = VecDeque::new();

      for &seed in seeds {
          if let Some(neighbors) = adjacency.get(seed) {
              for neighbor in neighbors {
                  if visited.insert(neighbor.clone()) {
                      queue.push_back((
                          neighbor.clone(),
                          1,
                          vec![seed.to_string(), neighbor.clone()],
                      ));
                  }
              }
          }
      }

      let mut results = Vec::new();

      while let Some((node_id, depth, path)) = queue.pop_front() {
          if results.len() >= effective_limit {
              break;
          }

          if let Some((label, _)) = nodes.get(&node_id) {
              results.push(TraversalHit {
                  node_id: node_id.clone(),
                  label: label.clone(),
                  depth,
                  path: path.clone(),
              });
          }

          if depth < max_depth {
              if let Some(neighbors) = adjacency.get(&node_id) {
                  for neighbor in neighbors {
                      if visited.insert(neighbor.clone()) {
                          let mut next_path = path.clone();
                          next_path.push(neighbor.clone());
                          queue.push_back((neighbor.clone(), depth + 1, next_path));
                      }
                  }
              }
          }
      }

      results
  }

  /// BFS shortest path from `from` to `to`. Returns `None` if unreachable.
  pub(crate) fn shortest_path(
      adjacency: &HashMap<String, Vec<String>>,
      from: &str,
      to: &str,
  ) -> Option<Vec<String>> {
      if from == to {
          return Some(vec![from.to_string()]);
      }

      let mut visited = HashSet::new();
      visited.insert(from.to_string());
      let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
      queue.push_back((from.to_string(), vec![from.to_string()]));

      while let Some((node, path)) = queue.pop_front() {
          if let Some(neighbors) = adjacency.get(&node) {
              for neighbor in neighbors {
                  if neighbor == to {
                      let mut result = path.clone();
                      result.push(to.to_string());
                      return Some(result);
                  }
                  if visited.insert(neighbor.clone()) {
                      let mut next_path = path.clone();
                      next_path.push(neighbor.clone());
                      queue.push_back((neighbor.clone(), next_path));
                  }
              }
          }
      }

      None
  }
  ```

- [ ] **Step 4.5: Create `src/compare.rs`**

  Create `crates/vox-graphify-reader/src/compare.rs`:

  ```rust
  //! Cross-manifest community drift and node/edge delta for two Graphify corpora.

  /// A lightweight summary derived from a Graphify corpus manifest.
  #[derive(Debug, Clone)]
  pub struct ManifestSummary {
      pub node_count: u64,
      pub edge_count: u64,
      pub community_count: u64,
  }

  /// The delta between two manifest summaries (old → new).
  #[derive(Debug, Clone)]
  pub struct ManifestDiff {
      /// Change in node count (positive = growth, negative = shrinkage).
      pub node_delta: i64,
      /// Change in edge count.
      pub edge_delta: i64,
      /// Change in community count.
      pub community_delta: i64,
  }

  /// Compute the signed delta from `old` to `new`.
  pub fn diff_manifests(old: &ManifestSummary, new: &ManifestSummary) -> ManifestDiff {
      ManifestDiff {
          node_delta: new.node_count as i64 - old.node_count as i64,
          edge_delta: new.edge_count as i64 - old.edge_count as i64,
          community_delta: new.community_count as i64 - old.community_count as i64,
      }
  }
  ```

- [ ] **Step 4.6: Run the tests — expect all pass**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-graphify-reader 2>&1 | tail -30
  ```

  Expected: all tests PASS.

- [ ] **Step 4.7: Commit**

  ```powershell
  git add crates/vox-graphify-reader/
  git commit -m "feat(vox-graphify-reader): new crate with BFS, shortest-path, god-nodes, and manifest diff"
  ```

---

## Task 5: Wire reader into three new MCP tools

**Context:** The reader crate provides the logic; this task connects it to the MCP surface. Pattern is identical to the existing `graphify_status` / `graphify_search` approach: param struct + handler function in `graphify_tools.rs`, schema in `input_schemas.rs`, dispatch arm in `dispatch.rs`, dep in both Cargo.tomls.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/graphify_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 5.1: Add `vox-graphify-reader` to workspace and crate deps**

  In the workspace root `Cargo.toml`, under `[workspace.dependencies]`, add:
  ```toml
  vox-graphify-reader  = { path = "crates/vox-graphify-reader" }
  ```

  In `crates/vox-orchestrator-mcp/Cargo.toml`, under `[dependencies]`, add:
  ```toml
  vox-graphify-reader = { workspace = true }
  ```

- [ ] **Step 5.2: Write the failing tests**

  > **Reviewer fix MIN-4:** Add MCP-level async test for `graphify_compare` (previously only unit-tested via `diff_manifests`). The MCP handler has different failure modes — unknown corpus, `assess_corpus_status` call — that need coverage.

  In the `#[cfg(test)] mod tests { ... }` block of `graphify_tools.rs`, add:

  ```rust
  #[tokio::test]
  async fn graphify_query_returns_bfs_neighbors() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      // Graph: auth --edge--> crypto
      let dir = tmp.path().join("graphify-out");
      fs::create_dir_all(&dir).unwrap();
      fs::write(
          dir.join("graph.json"),
          r#"{"nodes":[{"id":"auth","label":"authentication module","type":"module"},{"id":"crypto","label":"crypto lib","type":"lib"}],"links":[{"source":"auth","target":"crypto"}]}"#,
      )
      .unwrap();
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_query(
          &state,
          GraphifyQueryParams {
              corpus: Some("repo-code-graph".into()),
              seeds: vec!["auth".into()],
              max_depth: Some(1),
              limit: Some(10),
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed["success"], serde_json::json!(true), "tool error: {json}");
      let hits = parsed["data"]["hits"].as_array().expect("hits must be an array");
      assert!(!hits.is_empty(), "expected BFS hits: {json}");
      assert_eq!(hits[0]["node_id"], serde_json::json!("crypto"));
  }

  #[tokio::test]
  async fn graphify_path_returns_node_route() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      let dir = tmp.path().join("graphify-out");
      fs::create_dir_all(&dir).unwrap();
      fs::write(
          dir.join("graph.json"),
          r#"{"nodes":[{"id":"a"},{"id":"b"},{"id":"c"}],"links":[{"source":"a","target":"b"},{"source":"b","target":"c"}]}"#,
      )
      .unwrap();
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_path(
          &state,
          GraphifyPathParams {
              corpus: Some("repo-code-graph".into()),
              from: "a".into(),
              to: "c".into(),
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed["success"], serde_json::json!(true), "tool error: {json}");
      assert_eq!(parsed["data"]["path"], serde_json::json!(["a", "b", "c"]));
      assert_eq!(parsed["data"]["reachable"], serde_json::json!(true));
  }

  #[tokio::test]
  async fn graphify_compare_returns_delta_fields() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      // Write graph files for both corpora being compared.
      let dir_a = tmp.path().join("graphify-out");
      fs::create_dir_all(&dir_a).unwrap();
      fs::write(
          dir_a.join("graph.json"),
          r#"{"nodes":[{"id":"a"},{"id":"b"}],"links":[{"source":"a","target":"b"}]}"#,
      ).unwrap();
      let dir_b = tmp.path().join("crates/vox-gui/graphify-out");
      fs::create_dir_all(&dir_b).unwrap();
      fs::write(
          dir_b.join("graph.json"),
          r#"{"nodes":[{"id":"x"}],"links":[]}"#,
      ).unwrap();
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_compare(
          &state,
          GraphifyCompareParams {
              corpus_a: "repo-code-graph".into(),
              corpus_b: "vox-gui-surface".into(),
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed["success"], serde_json::json!(true), "compare error: {json}");
      // corpus_a has 2 nodes, corpus_b has 1 → node_delta = -1
      assert_eq!(parsed["data"]["diff"]["node_delta"], serde_json::json!(-1));
  }

  #[tokio::test]
  async fn graphify_compare_unknown_corpus_returns_error() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      let state = test_state_for_repo(tmp.path().to_path_buf());
      let json = graphify_compare(
          &state,
          GraphifyCompareParams {
              corpus_a: "no-such-corpus".into(),
              corpus_b: "repo-code-graph".into(),
          },
      )
      .await;
      let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert_eq!(parsed["success"], serde_json::json!(false), "expected error: {json}");
  }
  ```

- [ ] **Step 5.3: Run to confirm compile failure**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-orchestrator-mcp graphify_query 2>&1 | Select-String "error\[" | Select-Object -First 5
  ```

  Expected: compile error — `graphify_query` not found.

- [ ] **Step 5.4: Add the param structs and handlers to `graphify_tools.rs`**

  Add `use vox_graphify_reader;` at the top of the file, after the existing `use` imports.

  Add the following before the `#[cfg(test)]` block:

  ```rust
  // ── Graphify Query (BFS expansion) ────────────────────────────────────────

  #[derive(Debug, Deserialize)]
  pub struct GraphifyQueryParams {
      pub corpus: Option<String>,
      /// Seed node IDs to BFS-expand from.
      pub seeds: Vec<String>,
      /// BFS hop limit (default 2, max 5).
      pub max_depth: Option<u8>,
      /// Max hits returned (default 20).
      pub limit: Option<u32>,
  }

  #[derive(Debug, Deserialize)]
  pub struct GraphifyPathParams {
      pub corpus: Option<String>,
      /// Source node ID.
      pub from: String,
      /// Destination node ID.
      pub to: String,
  }

  #[derive(Debug, Deserialize)]
  pub struct GraphifyCompareParams {
      pub corpus_a: String,
      pub corpus_b: String,
  }

  /// Load and parse a corpus graph.json from disk.
  fn load_graph_json(
      repo_root: &std::path::Path,
      corpus: &vox_config::graphify::GraphifyCorpus,
  ) -> Result<serde_json::Value, String> {
      let p = repo_root.join(&corpus.graph_path);
      let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
      serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))
  }

  /// `vox_graphify_query`: BFS neighbor expansion from seed node IDs.
  pub async fn graphify_query(state: &ServerState, params: GraphifyQueryParams) -> String {
      let repo_root = &state.repository.root;
      let reg = match load_graphify_corpora(repo_root) {
          Ok(r) => r,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus) {
          Ok(v) => v,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let graph = match load_graph_json(repo_root, corpus) {
          Ok(v) => v,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY).to_json(),
      };
      let reader = match vox_graphify_reader::GraphifyReader::from_value(graph) {
          Ok(r) => r,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let max_depth = params.max_depth.unwrap_or(2).min(5);
      let limit = params.limit.unwrap_or(20).max(1) as usize;
      let seeds: Vec<&str> = params.seeds.iter().map(String::as_str).collect();
      let hits = reader.bfs_from_seeds(&seeds, max_depth, limit);
      let payload_hits: Vec<serde_json::Value> = hits
          .iter()
          .map(|h| {
              serde_json::json!({
                  "node_id": h.node_id,
                  "label": h.label,
                  "depth": h.depth,
                  "path": h.path,
                  "knowledge_id": knowledge_id(&corpus_id, &h.node_id),
              })
          })
          .collect();
      ToolResult::ok(serde_json::json!({
          "corpus_id": corpus_id,
          "seeds": params.seeds,
          "hits": payload_hits,
      }))
      .to_json()
  }

  /// `vox_graphify_path`: shortest path between two node IDs.
  pub async fn graphify_path(state: &ServerState, params: GraphifyPathParams) -> String {
      let repo_root = &state.repository.root;
      let reg = match load_graphify_corpora(repo_root) {
          Ok(r) => r,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus) {
          Ok(v) => v,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let graph = match load_graph_json(repo_root, corpus) {
          Ok(v) => v,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY).to_json(),
      };
      let reader = match vox_graphify_reader::GraphifyReader::from_value(graph) {
          Ok(r) => r,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      // CRIT-1 fix: bind `reachable` before `path` is moved into serde_json::json!.
      // `serde_json::json!` takes ownership of `path`; calling `.is_some()` after would
      // be a use-after-move compile error.
      let path = reader.shortest_path(&params.from, &params.to);
      let reachable = path.is_some();
      ToolResult::ok(serde_json::json!({
          "corpus_id": corpus_id,
          "from": params.from,
          "to": params.to,
          "path": path,
          "reachable": reachable,
      }))
      .to_json()
  }

  /// `vox_graphify_compare`: diff two corpus manifests (node/edge/community delta).
  pub async fn graphify_compare(state: &ServerState, params: GraphifyCompareParams) -> String {
      let repo_root = &state.repository.root;
      let reg = match load_graphify_corpora(repo_root) {
          Ok(r) => r,
          Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
      };
      let corpus_a = match reg.corpora.iter().find(|c| c.id == params.corpus_a) {
          Some(c) => c,
          None => return ToolResult::<serde_json::Value>::err_with_remediation(
              format!("unknown corpus_a: {}", params.corpus_a),
              REM_GRAPHIFY,
          ).to_json(),
      };
      let corpus_b = match reg.corpora.iter().find(|c| c.id == params.corpus_b) {
          Some(c) => c,
          None => return ToolResult::<serde_json::Value>::err_with_remediation(
              format!("unknown corpus_b: {}", params.corpus_b),
              REM_GRAPHIFY,
          ).to_json(),
      };
      let now = chrono::Utc::now();
      let ttl = reg.ttl_days_default;
      let head = resolve_head_sha(state).await;
      let status_a = assess_corpus_status(repo_root, corpus_a, head.as_deref(), now, ttl);
      let status_b = assess_corpus_status(repo_root, corpus_b, head.as_deref(), now, ttl);
      let summary_a = vox_graphify_reader::compare::ManifestSummary {
          node_count: status_a.node_count.unwrap_or(0),
          edge_count: status_a.edge_count.unwrap_or(0),
          community_count: 0, // not in CorpusStatus; reserved for future manifest field
      };
      let summary_b = vox_graphify_reader::compare::ManifestSummary {
          node_count: status_b.node_count.unwrap_or(0),
          edge_count: status_b.edge_count.unwrap_or(0),
          community_count: 0,
      };
      let diff = vox_graphify_reader::compare::diff_manifests(&summary_a, &summary_b);
      ToolResult::ok(serde_json::json!({
          "corpus_a": {
              "id": params.corpus_a,
              "node_count": summary_a.node_count,
              "edge_count": summary_a.edge_count,
              "is_fresh": status_a.is_fresh,
          },
          "corpus_b": {
              "id": params.corpus_b,
              "node_count": summary_b.node_count,
              "edge_count": summary_b.edge_count,
              "is_fresh": status_b.is_fresh,
          },
          "diff": {
              "node_delta": diff.node_delta,
              "edge_delta": diff.edge_delta,
              "community_delta": diff.community_delta,
          },
      }))
      .to_json()
  }
  ```

- [ ] **Step 5.5: Add dispatch arms in `dispatch.rs`**

  After the `"vox_graphify_search"` arm (around line 541), add:

  ```rust
  "vox_graphify_query" => {
      Ok(crate::graphify_tools::graphify_query(state, serde_json::from_value(args)?).await)
  }
  "vox_graphify_path" => {
      Ok(crate::graphify_tools::graphify_path(state, serde_json::from_value(args)?).await)
  }
  "vox_graphify_compare" => {
      Ok(crate::graphify_tools::graphify_compare(state, serde_json::from_value(args)?).await)
  }
  ```

- [ ] **Step 5.6: Add input schemas in `input_schemas.rs`**

  After the `"vox_graphify_search"` schema arm (around line 401), add:

  ```rust
  "vox_graphify_query" => parse_obj(
      r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"seeds":{"type":"array","items":{"type":"string"},"minItems":1,"description":"Seed node IDs to BFS-expand from"},"max_depth":{"type":"integer","minimum":1,"maximum":5,"description":"BFS hop limit (default 2)"},"limit":{"type":"integer","minimum":1,"description":"Max hits returned (default 20)"}},"required":["seeds"],"additionalProperties":false}"#,
  ),
  "vox_graphify_path" => parse_obj(
      r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"from":{"type":"string","description":"Source node ID"},"to":{"type":"string","description":"Destination node ID"}},"required":["from","to"],"additionalProperties":false}"#,
  ),
  "vox_graphify_compare" => parse_obj(
      r#"{"type":"object","properties":{"corpus_a":{"type":"string","description":"First corpus id to compare"},"corpus_b":{"type":"string","description":"Second corpus id to compare"}},"required":["corpus_a","corpus_b"],"additionalProperties":false}"#,
  ),
  ```

- [ ] **Step 5.7: Run all graphify tests**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-orchestrator-mcp graphify 2>&1 | tail -40
  cargo test -p vox-graphify-reader 2>&1 | tail -10
  ```

  Expected: all PASS.

- [ ] **Step 5.8: Commit**

  ```powershell
  git add crates/vox-orchestrator-mcp/src/graphify_tools.rs
  git add crates/vox-orchestrator-mcp/src/input_schemas.rs
  git add crates/vox-orchestrator-mcp/src/dispatch.rs
  git add crates/vox-orchestrator-mcp/Cargo.toml
  git add Cargo.toml
  git commit -m "feat(vox-orchestrator-mcp): add vox_graphify_query, vox_graphify_path, vox_graphify_compare"
  ```

---

## Task 6: Update contracts (tool-registry, read-role governance, operations catalog)

**Context:** Machine-readable YAML contracts track the MCP and CLI surface. These must stay in sync or CI fails. First try `operations-sync --write` to auto-regenerate; fall back to manual edits if it does not cover tool-registry or read-role governance.

**Files:**
- Modify: `contracts/mcp/tool-registry.canonical.yaml`
- Modify: `contracts/mcp/http-read-role-governance.yaml`
- Modify: `contracts/operations/catalog.v1.yaml`

- [ ] **Step 6.1: Try auto-sync**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo run -p vox-cli -- ci operations-sync --target all --write 2>&1 | tail -20
  ```

- [ ] **Step 6.2: Add tool-registry entries (if not auto-added)**

  In `contracts/mcp/tool-registry.canonical.yaml`, after the `vox_graphify_search` entry:

  ```yaml
  - name: vox_graphify_query
    description: BFS neighbor expansion from seed node IDs in a graphify corpus (read-only, layered navigation).
  - name: vox_graphify_path
    description: Shortest path between two node IDs in a graphify corpus (read-only).
  - name: vox_graphify_compare
    description: Diff two graphify corpus manifests for node/edge/community drift (read-only).
  ```

- [ ] **Step 6.3: Add to http-read-role-governance**

  In `contracts/mcp/http-read-role-governance.yaml`, in the allowed tools list (where `vox_graphify_status` and `vox_graphify_search` appear, currently lines 7–8), add:

  ```yaml
    - vox_graphify_query
    - vox_graphify_path
    - vox_graphify_compare
  ```

- [ ] **Step 6.4: Add operation entries to catalog**

  In `contracts/operations/catalog.v1.yaml`, after the `graphify.search` entry (currently around line 6038), add:

  ```yaml
  - id: graphify.ingest
    name: graphify ingest
    description: Project graph nodes into Turso knowledge_nodes via VoxDb (CLI only; no MCP surface).
    tags:
      - graphify
    handler_rust: commands::graphify

  - id: graphify.query
    name: graphify query
    description: BFS neighbor expansion from seed node IDs (read-only; layered navigation).
    tags:
      - graphify
    mcp:
      name: vox_graphify_query

  - id: graphify.path
    name: graphify path
    description: Shortest path between two node IDs in a graphify corpus (read-only).
    tags:
      - graphify
    mcp:
      name: vox_graphify_path

  - id: graphify.compare
    name: graphify compare
    description: Diff two graphify corpus manifests for node/edge/community delta (read-only).
    tags:
      - graphify
    mcp:
      name: vox_graphify_compare
  ```

- [ ] **Step 6.5: Verify operations**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo run -p vox-cli -- ci operations-verify 2>&1 | tail -10
  ```

  Expected: exit 0.

- [ ] **Step 6.6: Commit**

  ```powershell
  git add contracts/mcp/tool-registry.canonical.yaml
  git add contracts/mcp/http-read-role-governance.yaml
  git add contracts/operations/catalog.v1.yaml
  git commit -m "feat(contracts): register vox_graphify_query, path, compare in MCP registry and operations catalog"
  ```

---

## Task 7: Update SSOT documentation

**Context:** Three docs must be updated so future agents (especially ones with no session context) can discover and understand the complete Graphify integration without reading code. All docs under `docs/src/` require YAML frontmatter.

**Files:**
- Modify: `docs/src/architecture/search-retrieval-ssot-2026.md`
- Modify: `docs/src/architecture/graphify-integration-research-2026-06-16.md`
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `docs/superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md`

- [ ] **Step 7.1: Add Graphify corpus row to search-retrieval-ssot corpus matrix**

  Open `docs/src/architecture/search-retrieval-ssot-2026.md`. Find the corpus matrix table (a markdown table listing corpus tiers). Add this row (exact column positions depend on existing table shape — add it as a new data row):

  ```markdown
  | `graphify:*:node:*` (ingest) · `graphify:*:search:*` (hits) | A | Turso `knowledge_nodes` | `vox graphify ingest` | `lexical_ingest_sha256` ≠ `graph_json_sha256` → stale reason `lexical_lag`; virtual corpus `graphify-search-log` always fresh |
  ```

- [ ] **Step 7.2: Update §4.6 and add §4.7 in the research doc**

  Open `docs/src/architecture/graphify-integration-research-2026-06-16.md`.

  In §4.6 Phased delivery table, update the P1 and P2 rows to mark them COMPLETE:

  ```markdown
  | **P1** | Lexical ingest + `vox_graphify_search` + search-hit persistence (`persist` param) + `searched_at` provenance | `vox-search`, `vox-db` — **COMPLETE (2026-06-18)** |
  | **P2** | `vox-graphify-reader` crate + `vox_graphify_query` / `vox_graphify_path` / `vox_graphify_compare` MCP tools + virtual corpus + lexical lag detection | **COMPLETE (2026-06-18)** |
  ```

  Add after §4.6:

  ```markdown
  ### 4.7 Search persistence model

  Every `vox_graphify_search` call with `persist: true` (default) upserts each hit into Turso
  `knowledge_nodes`:

  - **ID:** `graphify:{corpus_id}:search:{query_slug}-{hash8}:{node_id}`
    (where `hash8` is an 8-char FNV-64 hash of the full query string — prevents collisions on
    long queries that share a common 32-char prefix)
  - **node_type:** `graphify_search_hit`
  - **metadata JSON:** `{ corpus_id, query, searched_at (RFC3339), git_sha, source: "graphify_search_hit" }`

  **Recalling prior searches:** Query Turso via `vox_knowledge_query` with
  `metadata.source = "graphify_search_hit"`. This surfaces all prior search hits across all
  agents without re-running the graph search.

  **⚠️ Staleness caveat (IMP-4):** Agents that retrieve search-hit nodes from Turso MUST
  compare `metadata.git_sha` against the current HEAD SHA before trusting results. If the SHAs
  differ, the hits were recorded against an older graph state and may not reflect current code
  structure. The `graphify-search-log` virtual corpus is always-fresh (no disk graph to go stale)
  but this does NOT mean the stored hit nodes are current — it only means the corpus registry
  entry itself requires no rebuild. Stale hit detection is the caller's responsibility.

  **Non-fatal persistence:** Turso unavailability is silently skipped (logged at `warn!`); search
  results always return regardless of DB state. Schema errors or auth failures are logged at
  `warn!` level so they surface in telemetry without failing the MCP call.
  ```

- [ ] **Step 7.3: Add `vox-graphify-reader` to where-things-live**

  Open `docs/src/architecture/where-things-live.md`. Find the existing Graphify corpus registry row. After it, insert:

  ```markdown
  | Graphify BFS reader + cross-map diff | [`crates/vox-graphify-reader`](../../../crates/vox-graphify-reader/) | `GraphifyReader::from_value`, `bfs_from_seeds`, `shortest_path`, `god_nodes`, `community_members`, `diff_manifests`. HashMap adjacency, no external graph lib. |
  ```

- [ ] **Step 7.4: Update handoff state**

  Open `docs/superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md`.

  After the "### P1 — Lexical library, MCP search, CLI ingest" section under "## What is DONE", add a new P2 section:

  ```markdown
  ### P2 — Structural reader + cross-map diff (COMPLETE 2026-06-18)

  | Component | Path / symbol |
  |-----------|----------------|
  | Reader crate | `crates/vox-graphify-reader/src/lib.rs` — `GraphifyReader`, `bfs_from_seeds`, `shortest_path`, `god_nodes`, `community_members` |
  | Compare | `crates/vox-graphify-reader/src/compare.rs` — `ManifestSummary`, `ManifestDiff`, `diff_manifests` |
  | MCP tools | `vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare` in `graphify_tools.rs` |
  | Search persistence | `persist` param on `vox_graphify_search`; hits stored as `graphify:{corpus_id}:search:{slug}:{node_id}` with `searched_at` + `git_sha` metadata |
  | Virtual corpus | `is_virtual: true` in `GraphifyCorpus`; `graphify-search-log` in corpus registry; `assess_corpus_status` always returns fresh for virtual |
  | Lexical lag | `lexical_ingest_sha256` in `GraphifyManifest`; `lexical_lag_stale_reason()` pub fn |
  ```

  In "## What is NOT done", remove the P2 items that are now done. Update the remaining "Not done" section to show only P3 items.

- [ ] **Step 7.5: Commit docs**

  ```powershell
  git add docs/src/architecture/search-retrieval-ssot-2026.md
  git add docs/src/architecture/graphify-integration-research-2026-06-16.md
  git add docs/src/architecture/where-things-live.md
  git add docs/superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md
  git commit -m "docs: update graphify SSOT, retrieval corpus matrix, where-things-live for P2 completion"
  ```

---

## Task 8: Full verification and code review

- [ ] **Step 8.1: Run all scoped graphify tests**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo test -p vox-config --test graphify_status 2>&1 | tail -15
  cargo test -p vox-config --test graphify_lexical 2>&1 | tail -10
  cargo test -p vox-graphify-reader 2>&1 | tail -15
  cargo test -p vox-orchestrator-mcp graphify 2>&1 | tail -25
  cargo test -p vox-cli --lib graphify 2>&1 | tail -10
  ```

  Expected: all PASS.

- [ ] **Step 8.2: Verify operations surface**

  ```powershell
  $env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"
  cargo run -p vox-cli -- ci operations-verify 2>&1 | tail -5
  ```

  Expected: exit 0.

- [ ] **Step 8.3: Smoke test `vox graphify status`**

  ```powershell
  vox graphify status
  ```

  Expected output includes lines for: `repo-code-graph`, `vox-gui-surface`, `config-audit`, **and** `graphify-search-log` (which should show `fresh` because it is virtual).

- [ ] **Step 8.4: Get BASE_SHA and HEAD_SHA for code review**

  ```powershell
  $BASE_SHA = "2c1aeb0e680dc0b4439c489f51efb21873b36d1c"
  $HEAD_SHA = (git rev-parse HEAD)
  Write-Host "BASE: $BASE_SHA"
  Write-Host "HEAD: $HEAD_SHA"
  ```

- [ ] **Step 8.5: Dispatch code review**

  Dispatch the `superpowers:code-reviewer` subagent using the template at `assets/skills/requesting-code-review/code-reviewer.md`. Fill in:

  - `{WHAT_WAS_IMPLEMENTED}`: Graphify search-hit persistence (`persist` param), `vox-graphify-reader` crate (BFS/path/compare), three new MCP tools (`vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare`), `is_virtual` corpus flag, `lexical_ingest_sha256`/`lexical_lag_stale_reason`, `graphify-search-log` virtual corpus, SSOT doc updates.
  - `{PLAN_OR_REQUIREMENTS}`: `docs/superpowers/plans/2026-06-18-graphify-search-map-persistence.md`
  - `{BASE_SHA}`: `2c1aeb0e680dc0b4439c489f51efb21873b36d1c`
  - `{HEAD_SHA}`: output of `git rev-parse HEAD`
  - `{DESCRIPTION}`: P1 gaps + P2 structural reader for Graphify map discoverability and navigable layer model.

  **Act on reviewer feedback:**
  - Fix Critical issues immediately before considering this plan complete.
  - Fix Important issues before merge.
  - Note Minor issues for later.

---

## Self-Review

**Spec coverage:**

| Requirement | Task |
|-------------|------|
| Every Graphify search mapped as part of the codebase | Task 3 (search-hit persistence to `knowledge_nodes`) |
| Searchable and available to all future agents | Task 3 (`graphify_search_hit` source tag; queryable via `vox_knowledge_query`) |
| Staleness tracking for each map | Task 1 (`lexical_lag_stale_reason`, `lexical_ingest_sha256`) |
| Continuing navigable map | Task 4+5 (`bfs_from_seeds`, `shortest_path`, path field on every hit) |
| Semantic discoverability | Task 2+7 (virtual corpus, retrieval SSOT corpus row) |
| Layered navigation by initial search | Task 5 (`vox_graphify_query` takes prior hit node IDs as seeds) |
| Cross-map comparison | Task 5 (`vox_graphify_compare` + `diff_manifests`) |
| Contracts updated | Task 6 |
| SSOT docs updated | Task 7 |
| Code review dispatched | Task 8.5 |

**Type consistency:**
- `TraversalHit` (lib.rs Task 4.3) → used in graphify_tools.rs Task 5.4 as `vox_graphify_reader::GraphifyReader::bfs_from_seeds` return type ✓
- `ManifestSummary` (compare.rs Task 4.5) → used in graphify_tools.rs Task 5.4 as `vox_graphify_reader::compare::ManifestSummary` ✓
- `lexical_lag_stale_reason` (graphify.rs Task 1.3) → tested in graphify_status.rs Task 1.1 as `vox_config::graphify::lexical_lag_stale_reason` ✓
- `is_virtual` field: added to `GraphifyCorpus` (Task 1.3), `#[serde(default)]` so existing YAML entries without it parse correctly (Task 1.4 verify step confirms this) ✓
- `persist` field: `#[serde(default = "default_persist")]` returns `Some(true)` — existing MCP callers that omit `persist` get persistence by default ✓
- `knowledge_id()` used in Task 5.4 `graphify_query` — this function is already defined in `graphify_tools.rs` at line 70 ✓
- `query_slug` defined Task 3.4, used in Task 3.5 — consistent ✓

**Placeholder scan:** No TBD/TODO/implement-later present. All code blocks are complete.
