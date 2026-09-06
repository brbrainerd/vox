---
title: "Graphify Graph Run Lifecycle Implementation Plan"
description: "Implementation plan for corpus path migration, lexical-lag surfacing, TTL env var, VoxScript auto-refresh, and CI freshness gate."
category: "superpowers"
status: "current"
---

# Graphify Graph Run Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the foundational graph-run lifecycle so that every Graphify corpus has a canonical storage path, accurate freshness signals (including lexical lag), a user-configurable TTL, an auto-rebuild script, and a CI freshness gate that fails the build when corpora drift.

**Architecture:** The existing `assess_corpus_status()` detects `git_drift`, `ttl_expired`, `graph_missing`, and `graph_corrupt`, but (a) corpus paths still point at the legacy `graphify-out/` directory instead of `.vox/cache/graphify/<corpus_id>/`, (b) `lexical_lag_stale_reason()` exists but is never called from the status path, (c) the TTL is hardcoded and cannot be overridden by env var, (d) there is no auto-rebuild VoxScript, and (e) `--strict` exists in the CLI but is not wired into CI. This plan closes all five gaps in order, with each task independently testable.

**Tech Stack:** Rust (`vox-config`, `vox-cli`, `vox-orchestrator-mcp`), VoxScript (`.vox`), YAML contracts, GitHub Actions CI.

---
## Background reading (read before you start)

| File | Why you care |
|------|-------------|
| `crates/vox-config/src/graphify.rs` | Core logic: `assess_corpus_status`, `lexical_lag_stale_reason`, `CorpusStatus` struct |
| `contracts/retrieval/graphify-corpora.v1.yaml` | Registry: 4 corpora, each with `graph_path` and `manifest_path` |
| `crates/vox-cli/src/commands/graphify/mod.rs` | CLI: `vox graphify status --strict` and `vox graphify ingest` |
| `crates/vox-orchestrator-mcp/src/graphify_tools.rs` | MCP `vox_graphify_status` tool |
| `scripts/coverage-graph/manifest_writer.py` | Python manifest writer hook (deprecated in Task 5) |
| `crates/vox-config/src/paths.rs` | Exports `REPO_CACHE_DIR` and `REPO_GRAPHIFY_CACHE_SUBDIR = "graphify"` |
| `.github/workflows/ci.yml` | CI pipeline (Task 4 adds a freshness gate step here) |
| `contracts/config/env-vars.v1.yaml` | Contract for registered env vars (Task 3 adds entry here) |

### The `CorpusStatus` struct (from `graphify.rs` lines 72-88)

```rust
pub struct CorpusStatus {
    pub corpus_id: String,
    pub title: String,
    pub graph_path: PathBuf,
    pub manifest_path: PathBuf,
    pub graph_exists: bool,
    pub manifest_exists: bool,
    pub node_count: Option<u64>,
    pub edge_count: Option<u64>,
    pub built_at: Option<String>,
    pub manifest_git_sha: Option<String>,
    pub head_git_sha: Option<String>,
    pub stale_reasons: Vec<String>,   // "git_drift"|"ttl_expired"|"graph_missing"|"graph_corrupt"
    pub warnings: Vec<String>,        // "manifest_missing"|"node_count_drift"|"edge_count_drift"
    pub is_fresh: bool,
}
```

### What "virtual" means

`graphify-search-log` has `is_virtual: true`. `assess_corpus_status` returns `is_fresh: true`
immediately for virtual corpora — no disk checks. Path-migration tasks must skip virtual corpora.

### Commit policy

Conventional commit prefixes: `feat:`, `fix:`, `test:`, `refactor:`, `chore:`, `docs:`.
Commits go on the current branch (`refactor/vox-db-maintainability`). Do not open a PR.

### How to build and test

```powershell
# Build a single crate (never `cargo fmt --all` — see AGENTS.md)
cargo build -p vox-config

# Test a single crate
cargo test -p vox-config

# Test one specific test by name
cargo test -p vox-config graphify::tests::some_test_name

# Format a single crate
cargo fmt -p vox-config
```

---

## File map

| Action | File | Responsibility |
|--------|------|---------------|
| **MODIFY** | `contracts/retrieval/graphify-corpora.v1.yaml` | Migrate `repo-code-graph`, `vox-gui-surface`, `config-audit` paths from `graphify-out/` → `.vox/cache/graphify/<id>/` |
| **MODIFY** | `crates/vox-config/src/graphify.rs` | Wire `lexical_lag_stale_reason()` into `assess_corpus_status()`; add `resolve_ttl_days()` |
| **MODIFY** | `crates/vox-config/tests/graphify_status.rs` | TDD tests for lexical-lag and TTL env override |
| **MODIFY** | `crates/vox-cli/src/commands/graphify/mod.rs` | Call `resolve_ttl_days`; update test `graph_dir` path |
| **MODIFY** | `crates/vox-orchestrator-mcp/src/graphify_tools.rs` | Call `resolve_ttl_days` |
| **MODIFY** | `contracts/config/env-vars.v1.yaml` | Register `VOX_GRAPHIFY_TTL_DAYS` |
| **MODIFY** | `.github/workflows/ci.yml` | Add `vox graphify status --strict` step |
| **CREATE** | `scripts/graphify-refresh.vox` | VoxScript: check status, surface rebuild instructions |
| **MODIFY** | `scripts/coverage-graph/manifest_writer.py` | Add deprecation warning |

---

## Task 1: Migrate corpus paths from `graphify-out/` to `.vox/cache/graphify/<id>/`

**Files:**
- Modify: `contracts/retrieval/graphify-corpora.v1.yaml`
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (test helper at line 261)

- [ ] **Step 1.1: Write a failing test asserting the new path**

  Open `crates/vox-cli/src/commands/graphify/mod.rs`. Add this test inside the `#[cfg(test)] mod tests` block, above the existing `ingest_graph_corpus_projects_minimal_graph_nodes` test (around line 258):

  ```rust
  #[test]
  fn ingest_corpus_resolves_cache_dir_path() {
      let tmp = tempfile::tempdir().unwrap();
      write_registry(tmp.path());
      // After path migration, repo-code-graph lives at .vox/cache/graphify/repo-code-graph/
      let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
      fs::create_dir_all(&graph_dir).unwrap();
      fs::write(
          graph_dir.join("graph.json"),
          r#"{"nodes":[{"id":"n1","label":"some module","type":"module"}]}"#,
      )
      .unwrap();
      let nodes = ingest_graph_corpus(tmp.path(), "repo-code-graph").unwrap();
      assert_eq!(nodes.len(), 1);
      assert_eq!(nodes[0].id, "graphify:repo-code-graph:node:n1");
  }
  ```

- [ ] **Step 1.2: Run the test to confirm it fails**

  ```powershell
  cargo test -p vox-cli ingest_corpus_resolves_cache_dir_path
  ```

  Expected: **FAIL** — registry still points at `graphify-out/graph.json`, so `ingest_graph_corpus` reads the wrong path and finds no graph there.

- [ ] **Step 1.3: Migrate the three disk corpora in the registry**

  Replace the full content of `contracts/retrieval/graphify-corpora.v1.yaml` with:

  ```yaml
  x-vox-version: 1
  schema_version: 1

  # Named Graphify knowledge-graph corpora for agent retrieval and freshness gates.
  # See docs/src/architecture/graphify-integration-research-2026-06-16.md

  default_corpus_id: repo-code-graph
  ttl_days_default: 30

  corpora:
    - id: repo-code-graph
      title: Repository code graph
      scope_path: "."
      graph_path: ".vox/cache/graphify/repo-code-graph/graph.json"
      manifest_path: ".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json"
      extraction_mode: structural
      default_for_intents:
        - code_navigation
        - repo_structure

    - id: vox-gui-surface
      title: vox-gui surface map
      scope_path: crates/vox-gui
      graph_path: ".vox/cache/graphify/vox-gui-surface/graph.json"
      manifest_path: ".vox/cache/graphify/vox-gui-surface/.graphify_manifest.v1.json"
      extraction_mode: structural
      default_for_intents:
        - gui_surface

    - id: config-audit
      title: Config hardcoded-values audit graph
      scope_path: "."
      graph_path: ".vox/cache/graphify/config-audit/graph.json"
      manifest_path: ".vox/cache/graphify/config-audit/.graphify_manifest.v1.json"
      extraction_mode: audit
      default_for_intents:
        - config_audit

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

- [ ] **Step 1.4: Update the existing test's `graph_dir` to use the new path**

  In `crates/vox-cli/src/commands/graphify/mod.rs`, find `ingest_graph_corpus_projects_minimal_graph_nodes` (around line 258). Change:

  ```rust
  let graph_dir = tmp.path().join("graphify-out");
  ```

  to:

  ```rust
  let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
  ```

  Everything else in that test stays the same.

- [ ] **Step 1.5: Run both tests**

  ```powershell
  cargo test -p vox-cli ingest_corpus_resolves_cache_dir_path
  cargo test -p vox-cli ingest_graph_corpus_projects_minimal_graph_nodes
  ```

  Expected: both **PASS**.

- [ ] **Step 1.6: Run all CLI tests for regressions**

  ```powershell
  cargo test -p vox-cli
  ```

  Expected: all pass. The `status_strict_fails_when_graph_missing` test uses an empty tempdir — neither old nor new paths have a graph file, so `graph_missing` fires regardless. That test should still pass.

- [ ] **Step 1.7: Commit**

  ```powershell
  git add contracts/retrieval/graphify-corpora.v1.yaml
  git add crates/vox-cli/src/commands/graphify/mod.rs
  git commit -m "refactor: migrate graphify corpus paths from graphify-out/ to .vox/cache/graphify/<id>/"
  ```

---

## Task 2: Surface `lexical_lag` in `assess_corpus_status` stale_reasons

**Why:** `lexical_lag_stale_reason()` exists at line 301 of `graphify.rs` but is never called. This task wires it into `assess_corpus_status` so that `vox graphify status` and `vox_graphify_status` both surface it when the Turso index is behind the graph.

**Files:**
- Modify: `crates/vox-config/src/graphify.rs`
- Modify: `crates/vox-config/tests/graphify_status.rs`

- [ ] **Step 2.1: Write a failing test for `lexical_lag` in `stale_reasons`**

  Open `crates/vox-config/tests/graphify_status.rs`. Add this test at the end of the file:

  ```rust
  #[test]
  fn lexical_lag_appears_in_stale_reasons_when_sha_mismatch() {
      use vox_config::graphify::{GraphifyCorpus, assess_corpus_status};
      use chrono::Utc;
      use std::fs;

      let tmp = tempfile::tempdir().unwrap();
      let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
      fs::create_dir_all(&graph_dir).unwrap();
      let graph_bytes = br#"{"nodes":[{"id":"x","label":"x"}],"links":[]}"#;
      fs::write(graph_dir.join("graph.json"), graph_bytes).unwrap();

      // lexical_ingest_sha256 != graph_json_sha256 => lag
      let manifest = serde_json::json!({
          "corpus_id": "repo-code-graph",
          "built_at": "2026-01-01T00:00:00Z",
          "git_sha": "abc123",
          "node_count": 1,
          "edge_count": 0,
          "graph_json_sha256": "aaaa",
          "lexical_ingest_sha256": "bbbb"
      });
      fs::write(
          graph_dir.join(".graphify_manifest.v1.json"),
          serde_json::to_string(&manifest).unwrap(),
      ).unwrap();

      let corpus = GraphifyCorpus {
          id: "repo-code-graph".into(),
          title: "Test".into(),
          scope_path: ".".into(),
          graph_path: ".vox/cache/graphify/repo-code-graph/graph.json".into(),
          manifest_path: ".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json".into(),
          extraction_mode: None,
          default_for_intents: vec![],
          is_virtual: false,
      };

      let status = assess_corpus_status(
          tmp.path(),
          &corpus,
          Some("abc123"),  // matches manifest git_sha => no git_drift
          Utc::now(),
          30,
      );

      assert!(
          status.stale_reasons.contains(&"lexical_lag".to_string()),
          "expected lexical_lag in stale_reasons, got: {:?}",
          status.stale_reasons
      );
      assert!(!status.stale_reasons.contains(&"git_drift".to_string()));
  }
  ```

- [ ] **Step 2.2: Run the failing test**

  ```powershell
  cargo test -p vox-config lexical_lag_appears_in_stale_reasons
  ```

  Expected: **FAIL** — `lexical_lag` is not yet added to `stale_reasons`.

- [ ] **Step 2.3: Wire `lexical_lag_stale_reason()` into `assess_corpus_status`**

  Open `crates/vox-config/src/graphify.rs`. Find the block that checks `node_count_drift` and `edge_count_drift` (lines 404-415). Immediately **after** that block, and **before** `let is_fresh = stale_reasons.is_empty();` (line 417), insert:

  ```rust
  // Turso lexical index is behind the current graph — run `vox graphify ingest`.
  if let Some(ref m) = manifest {
      if let Some(reason) = lexical_lag_stale_reason(m) {
          stale_reasons.push(reason);
      }
  }
  ```

- [ ] **Step 2.4: Run the new test to verify it passes**

  ```powershell
  cargo test -p vox-config lexical_lag_appears_in_stale_reasons
  ```

  Expected: **PASS**.

- [ ] **Step 2.5: Add the complementary "no lag when SHAs match" test**

  In `crates/vox-config/tests/graphify_status.rs`, add:

  ```rust
  #[test]
  fn no_lexical_lag_when_sha_matches() {
      use vox_config::graphify::{GraphifyCorpus, assess_corpus_status};
      use chrono::Utc;
      use std::fs;

      let tmp = tempfile::tempdir().unwrap();
      let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
      fs::create_dir_all(&graph_dir).unwrap();
      fs::write(
          graph_dir.join("graph.json"),
          br#"{"nodes":[{"id":"x","label":"x"}],"links":[]}"#,
      ).unwrap();

      let sha = "deadbeef";
      let manifest = serde_json::json!({
          "corpus_id": "repo-code-graph",
          "built_at": "2026-01-01T00:00:00Z",
          "git_sha": sha,
          "node_count": 1,
          "edge_count": 0,
          "graph_json_sha256": sha,
          "lexical_ingest_sha256": sha  // same => no lag
      });
      fs::write(
          graph_dir.join(".graphify_manifest.v1.json"),
          serde_json::to_string(&manifest).unwrap(),
      ).unwrap();

      let corpus = GraphifyCorpus {
          id: "repo-code-graph".into(),
          title: "Test".into(),
          scope_path: ".".into(),
          graph_path: ".vox/cache/graphify/repo-code-graph/graph.json".into(),
          manifest_path: ".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json".into(),
          extraction_mode: None,
          default_for_intents: vec![],
          is_virtual: false,
      };

      let status = assess_corpus_status(tmp.path(), &corpus, Some(sha), Utc::now(), 30);
      assert!(!status.stale_reasons.contains(&"lexical_lag".to_string()));
      assert!(status.is_fresh, "expected fresh when SHAs match and no drift");
  }
  ```

- [ ] **Step 2.6: Run all `vox-config` tests**

  ```powershell
  cargo test -p vox-config
  ```

  Expected: all pass.

- [ ] **Step 2.7: Commit**

  ```powershell
  git add crates/vox-config/src/graphify.rs
  git add crates/vox-config/tests/graphify_status.rs
  git commit -m "feat: surface lexical_lag in assess_corpus_status stale_reasons"
  ```

---

## Task 3: Add `VOX_GRAPHIFY_TTL_DAYS` env var and `resolve_ttl_days()` helper

**Why:** TTL is hardcoded at 30 days everywhere. CI and developers need to override it. This task adds a public helper that reads the env var, registers it in the contract, and wires it into both the CLI and MCP assess paths.

**Files:**
- Modify: `crates/vox-config/src/graphify.rs`
- Modify: `crates/vox-config/tests/graphify_status.rs`
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs`
- Modify: `crates/vox-orchestrator-mcp/src/graphify_tools.rs`
- Modify: `contracts/config/env-vars.v1.yaml`

- [ ] **Step 3.1: Write a failing test for TTL env override**

  In `crates/vox-config/tests/graphify_status.rs`, add:

  ```rust
  #[test]
  fn ttl_env_var_overrides_default() {
      use vox_config::graphify::resolve_ttl_days;

      std::env::remove_var("VOX_GRAPHIFY_TTL_DAYS");
      assert_eq!(resolve_ttl_days(30), 30, "no env: use registry default");

      std::env::set_var("VOX_GRAPHIFY_TTL_DAYS", "7");
      assert_eq!(resolve_ttl_days(30), 7, "env=7: override registry default");

      std::env::set_var("VOX_GRAPHIFY_TTL_DAYS", "notanumber");
      assert_eq!(resolve_ttl_days(30), 30, "non-numeric: fall back to default");

      std::env::remove_var("VOX_GRAPHIFY_TTL_DAYS");
  }
  ```

- [ ] **Step 3.2: Run the failing test (expected: compile error)**

  ```powershell
  cargo test -p vox-config ttl_env_var_overrides_default
  ```

  Expected: **compile error** — `resolve_ttl_days` does not exist yet.

- [ ] **Step 3.3: Add `GRAPHIFY_TTL_DAYS_ENV` constant and `resolve_ttl_days()` to `graphify.rs`**

  Open `crates/vox-config/src/graphify.rs`. After the `fn default_ttl_days() -> u64` function (line 29-31), add:

  ```rust
  /// Environment variable name to override `ttl_days_default` from the corpus registry.
  pub const GRAPHIFY_TTL_DAYS_ENV: &str = "VOX_GRAPHIFY_TTL_DAYS";

  /// Resolve effective TTL: `VOX_GRAPHIFY_TTL_DAYS` env var wins over `registry_default`.
  ///
  /// Non-numeric or absent env falls back to `registry_default` silently.
  pub fn resolve_ttl_days(registry_default: u64) -> u64 {
      std::env::var(GRAPHIFY_TTL_DAYS_ENV)
          .ok()
          .and_then(|v| v.parse::<u64>().ok())
          .unwrap_or(registry_default)
  }
  ```

- [ ] **Step 3.4: Run the test — verify it passes**

  ```powershell
  cargo test -p vox-config ttl_env_var_overrides_default
  ```

  Expected: **PASS**.

  > **If you see test flakiness** because parallel tests mutate `VOX_GRAPHIFY_TTL_DAYS`: add `serial_test = "2"` to `[dev-dependencies]` in `crates/vox-config/Cargo.toml` and annotate the test with `#[serial_test::serial]`.

- [ ] **Step 3.5: Wire `resolve_ttl_days` into the CLI `assess_all`**

  Open `crates/vox-cli/src/commands/graphify/mod.rs`. Update the import at line 6:

  ```rust
  use vox_config::graphify::{
      CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
      assess_corpus_status, load_graphify_corpora, project_graph_nodes_for_ingest,
      resolve_ttl_days,
  };
  ```

  Then find `assess_all` (line 72). Change `let ttl = reg.ttl_days_default;` to:

  ```rust
  let ttl = resolve_ttl_days(reg.ttl_days_default);
  ```

- [ ] **Step 3.6: Wire `resolve_ttl_days` into the MCP `assess_all`**

  Open `crates/vox-orchestrator-mcp/src/graphify_tools.rs`. Find `assess_all` (around line 105). Change `let ttl = reg.ttl_days_default;` to:

  ```rust
  let ttl = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
  ```

- [ ] **Step 3.7: Register `VOX_GRAPHIFY_TTL_DAYS` in the env-vars contract**

  Open `contracts/config/env-vars.v1.yaml`. Search for any existing `VOX_` entry to understand the list format (each entry is a YAML mapping). Add this entry in the appropriate section (e.g. near other `graphify` or `corpus` related entries, or at the end of the list):

  ```yaml
  - name: VOX_GRAPHIFY_TTL_DAYS
    description: >-
      Override the Graphify corpus TTL (days) from the registry default (30 days).
      When set, all corpus freshness assessments use this value instead of
      `ttl_days_default` in `contracts/retrieval/graphify-corpora.v1.yaml`.
      Non-numeric values are silently ignored and fall back to the registry default.
    type: optional
    default: "30"
    scope: graphify
  ```

- [ ] **Step 3.8: Build all affected crates**

  ```powershell
  cargo build -p vox-config
  cargo build -p vox-cli
  cargo build -p vox-orchestrator-mcp
  ```

  Expected: all build without errors.

- [ ] **Step 3.9: Run tests**

  ```powershell
  cargo test -p vox-config
  cargo test -p vox-cli
  ```

  Expected: all pass.

- [ ] **Step 3.10: Commit**

  ```powershell
  git add crates/vox-config/src/graphify.rs
  git add crates/vox-config/tests/graphify_status.rs
  git add crates/vox-cli/src/commands/graphify/mod.rs
  git add crates/vox-orchestrator-mcp/src/graphify_tools.rs
  git add contracts/config/env-vars.v1.yaml
  git commit -m "feat: add VOX_GRAPHIFY_TTL_DAYS env var and resolve_ttl_days() helper"
  ```

---

## Task 4: Wire `--strict` into the CI pipeline as a freshness warning gate

**Why:** `vox graphify status --strict` already exits non-zero on staleness (CLI `mod.rs` line 191) but is not called from CI. This task adds a warning-gate step.

**Note:** This is `continue-on-error: true` because graph files are Tier D artifacts not committed to git. CI checkouts won't have them — the corpora will show `graph_missing`. The gate becomes blocking once the rebuild pipeline runs in CI.

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 4.1: Find the right job in `.github/workflows/ci.yml`**

  The CI file already references `graphify-out/` in lines 923, 932-934 (build-bench artifact steps). Find the job that contains those steps — it likely relates to audit or quality gates. Note the job name.

  ```powershell
  Select-String -Path ".github/workflows/ci.yml" -Pattern "graphify" -Context 3,3
  ```

  Identify whether there is a `freshness-gates` job or a logical place to add a new step after the existing graphify artifact upload.

- [ ] **Step 4.2: Add the freshness gate step**

  In the job you found (or in the `test` / main CI job if no specific job exists), add this step after the last graphify-related step:

  ```yaml
        - name: Graphify corpus freshness gate (warning)
          run: cargo run --quiet -p vox-cli -- graphify status --strict
          env:
            VOX_GRAPHIFY_TTL_DAYS: "7"
          continue-on-error: true
  ```

  > `continue-on-error: true` makes this a visible warning that does not fail CI while graph artifacts are not yet built in CI. Remove `continue-on-error` once the rebuild pipeline is integrated (future plan).

- [ ] **Step 4.3: Commit**

  ```powershell
  git add .github/workflows/ci.yml
  git commit -m "ci: add graphify corpus freshness warning gate (continue-on-error until rebuild pipeline)"
  ```

---

## Task 5: Create `scripts/graphify-refresh.vox` and deprecate `manifest_writer.py`

**Why:** Per `AGENTS.md §VoxScript-First Glue Code`, all project automation must be `.vox` files. This VoxScript checks freshness for all corpora and surfaces rebuild instructions for stale ones. It also runs `vox graphify ingest` when `--ingest` flag is passed and `lexical_lag` is the only reason for staleness.

**Files:**
- Create: `scripts/graphify-refresh.vox`
- Modify: `scripts/coverage-graph/manifest_writer.py`

- [ ] **Step 5.1: Read a nearby VoxScript for syntax reference**

  ```powershell
  Get-Content scripts/fmt.vox
  ```

  Note the syntax: `let`, `for`, `if`, `run(cmd, args)`, `print(msg)`, `exit(code)`, `env.args()`. Use this as your reference.

- [ ] **Step 5.2: Check that `vox run --interp` is available**

  ```powershell
  cargo run -p vox-cli -- run --help
  ```

  Expected: help text showing `--interp` flag. If not available, use `cargo run -p vox-cli -- check <file>` for validation.

- [ ] **Step 5.3: Create `scripts/graphify-refresh.vox`**

  Create the file at `scripts/graphify-refresh.vox` with this content — adapt syntax to match `scripts/fmt.vox` if the exact keywords differ:

  ```vox
  // graphify-refresh.vox
  // Check freshness of all registered graphify corpora and surface rebuild instructions.
  //
  // Usage:
  //   vox run scripts/graphify-refresh.vox               # status check only
  //   vox run scripts/graphify-refresh.vox -- --ingest   # also run ingest for lexical_lag corpora
  //
  // SSOT: contracts/retrieval/graphify-corpora.v1.yaml
  // Policy: AGENTS.md §VoxScript-First Glue Code

  let args = env.args()
  let do_ingest = args.contains("--ingest")

  print("=== Graphify Corpus Freshness Check ===")

  let status_out = run("cargo", ["run", "--quiet", "-p", "vox-cli", "--",
                                 "graphify", "status", "--json"])
  let statuses = json.parse(status_out.stdout)

  let any_stale = false

  for corpus in statuses {
      let id = corpus.corpus_id
      let reasons = corpus.stale_reasons

      if corpus.is_fresh {
          print("  [ok]    " + id)
      } else {
          any_stale = true
          print("  [stale] " + id + " — " + reasons.join(", "))

          if reasons.contains("graph_missing") || reasons.contains("git_drift") {
              print("          → Rebuild graph: python -m graphify --config graphify.toml --corpus " + id)
              print("            Then write manifest: python scripts/coverage-graph/manifest_writer.py")
          }

          if reasons.contains("lexical_lag") {
              print("          → Re-ingest to Turso: vox graphify ingest --corpus " + id)
              if do_ingest {
                  run("cargo", ["run", "--quiet", "-p", "vox-cli", "--",
                                "graphify", "ingest", "--corpus", id])
                  print("            Ingest complete.")
              }
          }

          if reasons.contains("ttl_expired") {
              print("          → TTL expired. Rebuild corpus or raise VOX_GRAPHIFY_TTL_DAYS.")
          }
      }
  }

  if any_stale {
      print("")
      print("One or more corpora are stale. Run the rebuild commands shown above.")
      exit(1)
  } else {
      print("All registered corpora are fresh.")
  }
  ```

- [ ] **Step 5.4: Validate the script**

  ```powershell
  cargo run -p vox-cli -- check scripts/graphify-refresh.vox
  ```

  Expected: `ok` or equivalent. If `check` is not supported for scripts, run:

  ```powershell
  cargo run -p vox-cli -- run --interp scripts/graphify-refresh.vox
  ```

  Expected: prints corpus status (most will be stale with `graph_missing`), exits non-zero (correct — corpora are missing in dev env).

- [ ] **Step 5.5: Add a deprecation notice to `manifest_writer.py`**

  Open `scripts/coverage-graph/manifest_writer.py`. After the closing `"""` of the module docstring (line 6), add:

  ```python
  import warnings
  warnings.warn(
      "manifest_writer.py is deprecated. Use `vox run scripts/graphify-refresh.vox` instead. "
      "See AGENTS.md §VoxScript-First Glue Code.",
      DeprecationWarning,
      stacklevel=2,
  )
  ```

- [ ] **Step 5.6: Commit**

  ```powershell
  git add scripts/graphify-refresh.vox
  git add scripts/coverage-graph/manifest_writer.py
  git commit -m "feat: add scripts/graphify-refresh.vox VoxScript; deprecate manifest_writer.py"
  ```

---

## Task 6: Update SSOT documentation

**Files:**
- Modify: `docs/src/architecture/graphify-integration-research-2026-06-16.md`
- Modify: `docs/src/architecture/where-things-live.md` (only if `graphify-out/` appears there)

- [ ] **Step 6.1: Update corpus paths in the research doc**

  Open `docs/src/architecture/graphify-integration-research-2026-06-16.md`. Find all occurrences of `graphify-out/` and replace:

  | Old path | New path |
  |----------|----------|
  | `graphify-out/graph.json` | `.vox/cache/graphify/repo-code-graph/graph.json` |
  | `graphify-out/config-audit-graph/graph.json` | `.vox/cache/graphify/config-audit/graph.json` |
  | `crates/vox-gui/graphify-out/graph.json` | `.vox/cache/graphify/vox-gui-surface/graph.json` |

  Then add a new subsection under the staleness section (wherever `git_drift`, `ttl_expired`, etc. are described):

  ```markdown
  #### Lexical lag (`lexical_lag`)

  Added in Graph Run Lifecycle plan (2026-06-18). `assess_corpus_status()` now pushes
  `"lexical_lag"` into `stale_reasons` when `manifest.lexical_ingest_sha256 !=
  manifest.graph_json_sha256`. This indicates the Turso `knowledge_nodes` index was built
  from an older version of `graph.json` than the current on-disk file.

  **Fix:** `vox graphify ingest --corpus <id>` or `vox run scripts/graphify-refresh.vox -- --ingest`.
  ```

- [ ] **Step 6.2: Frontmatter on updated docs**

Set valid frontmatter on the new page (`title`, `description`, `category`, `status`).
Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).

- [ ] **Step 6.3: Check `where-things-live.md` for stale paths**

  ```powershell
  Select-String -Path "docs/src/architecture/where-things-live.md" -Pattern "graphify-out"
  ```

  If any matches: update them to `.vox/cache/graphify/<id>/`.

- [ ] **Step 6.4: Commit**

  ```powershell
  git add docs/src/architecture/graphify-integration-research-2026-06-16.md
  git add docs/src/architecture/where-things-live.md
  git commit -m "docs: update graphify SSOT — new corpus paths, lexical_lag, TTL env var"
  ```

---

## Task 7: Final integration verification

- [ ] **Step 7.1: Build all affected crates**

  ```powershell
  cargo build -p vox-config
  cargo build -p vox-cli
  cargo build -p vox-orchestrator-mcp
  ```

  Expected: all build cleanly.

- [ ] **Step 7.2: Run all test suites**

  ```powershell
  cargo test -p vox-config
  cargo test -p vox-cli
  cargo test -p vox-orchestrator-mcp
  ```

  Expected: all pass.

- [ ] **Step 7.3: Smoke-test `vox graphify status`**

  ```powershell
  cargo run -p vox-cli -- graphify status
  ```

  Expected (approximate):

  ```
  # head <sha>
  repo-code-graph      stale  nodes=-  edges=-  graph=.vox/cache/graphify/repo-code-graph/graph.json
    stale: graph_missing
  vox-gui-surface      stale  nodes=-  edges=-  graph=.vox/cache/graphify/vox-gui-surface/graph.json
    stale: graph_missing
  config-audit         stale  nodes=-  edges=-  graph=.vox/cache/graphify/config-audit/graph.json
    stale: graph_missing
  graphify-search-log  fresh  nodes=-  edges=-  graph=.vox/cache/graphify/search-log/graph.json
    warn:  virtual_corpus
  ```

  Paths must show `.vox/cache/graphify/...` — not `graphify-out/`.

- [ ] **Step 7.4: Smoke-test `--strict` exit code**

  ```powershell
  cargo run -p vox-cli -- graphify status --strict
  $LASTEXITCODE
  ```

  Expected: exit code `1` (stale corpora present).

- [ ] **Step 7.5: Smoke-test TTL env override**

  ```powershell
  $env:VOX_GRAPHIFY_TTL_DAYS = "0"
  cargo run -p vox-cli -- graphify status
  Remove-Item Env:VOX_GRAPHIFY_TTL_DAYS
  ```

  Expected: corpora with a `built_at` in the manifest would show `ttl_expired`; without a manifest, `graph_missing` takes precedence. Either is correct.

- [ ] **Step 7.6: Format modified crates**

  ```powershell
  cargo fmt -p vox-config
  cargo fmt -p vox-cli
  cargo fmt -p vox-orchestrator-mcp
  ```

- [ ] **Step 7.7: Final commit if formatter made changes**

  ```powershell
  git add -u
  git status
  # Only commit if there are changes
  git commit -m "chore: fmt after graphify run-lifecycle implementation"
  ```

---

## Self-review checklist

| Requirement | Task |
|-------------|------|
| Corpus paths migrated `graphify-out/` → `.vox/cache/graphify/<id>/` | Task 1 |
| `lexical_lag` surfaced in `stale_reasons` (TDD) | Task 2 |
| `VOX_GRAPHIFY_TTL_DAYS` env var + `resolve_ttl_days()` (TDD) | Task 3 |
| CI freshness gate (`--strict` + `continue-on-error`) | Task 4 |
| VoxScript auto-refresh script | Task 5 |
| `manifest_writer.py` deprecation notice | Task 5.5 |
| SSOT docs updated with new paths + lexical_lag section | Task 6 |
| All code shown in full (no placeholders) | Verified |
| Type/method names consistent across tasks: `resolve_ttl_days`, `assess_corpus_status`, `lexical_lag_stale_reason` | Verified |
| Frequent commits (one per task) | Verified |

> **Out of scope — separate plans required:**
> - Sub-project B: GUI surface (corpus health panel, graph explorer in `vox-gui`)
> - Sub-project C: Search fusion (`vox_memory_search` routing to graphify corpora)
> - Sub-project D: Rust ecosystem uplift (Kodegraf/CodeGraph/Octocode evaluation)
> - Spool events (`graphify.lifecycle` Tier B events via `vox-spool`)
> - Full Python graphify pipeline migration to VoxScript
> - Removal of the committed `graphify-out/` directory files (C1 blocker, separate cleanup PR)
