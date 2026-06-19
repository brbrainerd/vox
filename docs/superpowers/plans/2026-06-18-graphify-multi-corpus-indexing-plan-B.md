# Plan B — Graphify Multi-Corpus & Target-Repo Indexing (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Engineered against those modes. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan A** (`RebuildMeta`, qualified `module::symbol` ids). Plan A must be fully landed and green first.

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A signature/field change fixes every caller/literal in the SAME task. A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** Each task's first step is an `rg`/read confirming the symbols it touches. Reality differs → STOP, do not invent.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Verification fails twice → STOP + handoff note (what failed, last good SHA). No looping.
5. **Parallel dispatch.** Honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`-p <crate>`); automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs.
7. **Verification ritual before commit** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD` to last green; re-attempt the single task.
9. **Skills:** design → `brainstorming`; parallel → `dispatching-parallel-agents`; isolation → `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Let `vox graphify` index arbitrary external target repositories and build multiple corpora over one source along different semantic lines, via a dynamic registration overlay and a pluggable semantic lens.

**Architecture:** (B1) `source_root` on `GraphifyCorpus` points a corpus's source at an external repo while its graph stays under the Vox repo's `.vox/cache/graphify/<id>/`; freshness uses the corpus's own source-repo HEAD. (B2) `vox graphify index <path>` writes a corpus into a runtime overlay (`.vox/cache/graphify/registered.v1.json`); `load_all_corpora` merges YAML (canonical, wins collisions) + overlay, and builds it. (B3) a semantic lens selected by `extraction_mode` post-processes the structural graph; ship the `modules` lens (collapse `module::symbol` → one node per module, weighted inter-module edges) — a distinct semantic line and the large-repo data-size escape hatch.

**Tech Stack:** Rust; `serde`/`serde_json`/`serde_yaml`; `clap`; `chrono`; `vox-graphify-reader`.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-config/src/graphify.rs` | Registry + overlay merge | Modify (B1, B2) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | `index` cmd, path/head resolution | Modify (B1, B2) |
| `crates/vox-graphify-reader/src/lens.rs` | Semantic lenses | Create (B3) |
| `crates/vox-graphify-reader/src/lib.rs` | Module registration | Modify (B3) |
| `crates/vox-graphify-reader/src/rebuild.rs` | Apply lens by mode | Modify (B3) |
| `crates/vox-graphify-reader/tests/lens_tests.rs` | Lens tests | Create (B3) |
| `crates/vox-graphify-reader/tests/rebuild_tests.rs` | Lens-in-rebuild test | Modify (B3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "GraphifyCorpus \{" crates/` — enumerate EVERY `GraphifyCorpus` struct literal; B1 adds `source_root: None,` to each (including Plan A's A4 test literal). A missed literal = compile error.
- `rg -n "load_graphify_corpora|fn run\(" crates/vox-cli/src/commands/graphify/mod.rs` — note the three `load_graphify_corpora` call sites in `run()` (Status/Ingest/Rebuild) that B2 switches to `load_all_corpora`.
- `rg -n "use std::collections::HashSet|serde_yaml" crates/vox-config/src/graphify.rs` — confirm `HashSet` and `serde_yaml` are in scope (they are).
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task B1 `[SEQUENTIAL]`: External `source_root` + per-corpus freshness head

Adds the `source_root` field (all literals fixed in-task) and routes rebuild/freshness through it.

**Files:**
- Modify: `crates/vox-config/src/graphify.rs` (struct)
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (`resolve_source_dir`, `resolve_head_sha_in`, `assess_all`, `Rebuild` arm)
- Modify: every other `GraphifyCorpus { .. }` literal flagged by Pre-flight
- Test: inline `#[cfg(test)]` in `crates/vox-cli/src/commands/graphify/mod.rs`

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight `rg -n "GraphifyCorpus \{" crates/` and `rg -n "pub struct GraphifyCorpus" -A14 crates/vox-config/src/graphify.rs`. List every literal to patch. If a literal is outside `crates/`, STOP and report.

- [ ] **Step 2: Write the failing test.** Inside the existing `#[cfg(test)] mod tests` in `mod.rs`:

```rust
#[test]
fn source_root_overrides_repo_root_for_source_dir() {
    use vox_config::graphify::GraphifyCorpus;
    let repo = std::path::Path::new("/repo");
    let ext = GraphifyCorpus {
        id: "ext".into(), title: "ext".into(), scope_path: "src".into(),
        graph_path: ".vox/cache/graphify/ext/graph.json".into(),
        manifest_path: ".vox/cache/graphify/ext/.graphify_manifest.v1.json".into(),
        extraction_mode: Some("structural".into()), default_for_intents: vec![],
        is_virtual: false, source_root: Some("/other/target".into()),
    };
    assert_eq!(resolve_source_dir(repo, &ext), std::path::Path::new("/other/target").join("src"));
    let local = GraphifyCorpus { source_root: None, ..ext };
    assert_eq!(resolve_source_dir(repo, &local), repo.join("src"));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-cli source_root_overrides_repo_root_for_source_dir` → FAIL to compile (`source_root` + `resolve_source_dir` missing).

- [ ] **Step 4: Add the field.** In `graphify.rs`, after `is_virtual` in `GraphifyCorpus`:

```rust
    /// Absolute path to an external source repository to index. `None` = the Vox repo root.
    /// The graph is stored under the Vox repo's `.vox/cache/graphify/<id>/` regardless.
    #[serde(default)]
    pub source_root: Option<String>,
```

- [ ] **Step 5: Fix every flagged literal.** Add `source_root: None,` to each `GraphifyCorpus { .. }` from Step 1 (e.g. Plan A's `crates/vox-cli/tests/graphify_rebuild.rs` literal). Run `cargo build -p vox-cli -p vox-config` and patch any remaining compile errors.

- [ ] **Step 6: Add helpers + per-corpus head.** In `mod.rs`, near `resolve_head_sha`:

```rust
pub(crate) fn resolve_source_dir(
    repo_root: &std::path::Path,
    corpus: &GraphifyCorpus,
) -> std::path::PathBuf {
    corpus
        .source_root
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_root.to_path_buf())
        .join(&corpus.scope_path)
}

/// `git -C <dir> rev-parse HEAD`, or Ok(None) if not a git repo.
fn resolve_head_sha_in(dir: &std::path::Path) -> anyhow::Result<Option<String>> {
    let output = std::process::Command::new("git")
        .arg("-C").arg(dir).args(["rev-parse", "HEAD"])
        .output().context("git rev-parse HEAD")?;
    if !output.status.success() { return Ok(None); }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!sha.is_empty()).then_some(sha))
}
```

Replace `assess_all`'s body so freshness uses each corpus's own source-repo HEAD (external corpora track a different repo than the Vox HEAD — comparing against Vox HEAD would always report `git_drift`):

```rust
fn assess_all(
    repo_root: &std::path::Path,
    reg: &GraphifyCorporaRegistry,
    corpus: &Option<String>,
    vox_head: Option<&str>,
) -> Result<Vec<CorpusStatus>, GraphifyError> {
    let now = Utc::now();
    let ttl = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
    selected_corpora(reg, corpus)?
        .into_iter()
        .map(|c| {
            let head: Option<String> = match &c.source_root {
                Some(root) => resolve_head_sha_in(std::path::Path::new(root)).unwrap_or(None),
                None => vox_head.map(str::to_string),
            };
            Ok(assess_corpus_status(repo_root, c, head.as_deref(), now, ttl))
        })
        .collect()
}
```

In the `Rebuild` arm, change `let source_dir = repo_root.join(&corpus.scope_path);` to `let source_dir = resolve_source_dir(repo_root, corpus);`.

- [ ] **Step 7: Run → PASS + build.** `cargo test -p vox-cli source_root_overrides_repo_root_for_source_dir` → PASS; `cargo build -p vox-cli -p vox-config` → clean.

- [ ] **Step 8: Verify (Rule 7) + commit.**

```bash
git add crates/vox-config/src/graphify.rs crates/vox-cli/src/commands/graphify/mod.rs crates/vox-cli/tests/graphify_rebuild.rs
git commit -m "feat(graphify): external source_root for target-repo corpora + per-corpus freshness head"
```

---

## Task B2 `[SEQUENTIAL]`: `vox graphify index <path>` dynamic registration

**Files:**
- Modify: `crates/vox-config/src/graphify.rs` (overlay registry)
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (`Index` arm; switch `run()` loads to `load_all_corpora`)
- Test: inline `#[cfg(test)]` in `crates/vox-config/src/graphify.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub fn load_graphify_corpora|struct CorporaFile|REGISTERED_REL_PATH" crates/vox-config/src/graphify.rs`. Confirm `load_graphify_corpora` exists and `REGISTERED_REL_PATH` does NOT yet. If it does, STOP.

- [ ] **Step 2: Write failing tests** in `graphify.rs` `mod tests`:

```rust
fn sample_corpus(id: &str) -> GraphifyCorpus {
    GraphifyCorpus {
        id: id.into(), title: "t".into(), scope_path: ".".into(),
        graph_path: format!(".vox/cache/graphify/{id}/graph.json"),
        manifest_path: format!(".vox/cache/graphify/{id}/.graphify_manifest.v1.json"),
        extraction_mode: Some("structural".into()), default_for_intents: vec![],
        is_virtual: false, source_root: Some("/tmp/target".into()),
    }
}
fn write_min_registry(repo: &std::path::Path) {
    let dir = repo.join("contracts/retrieval");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("graphify-corpora.v1.yaml"),
        "default_corpus_id: repo-code-graph\nttl_days_default: 30\ncorpora:\n  - id: repo-code-graph\n    title: Repo\n    scope_path: \".\"\n    graph_path: \"g\"\n    manifest_path: \"m\"\n").unwrap();
}
#[test]
fn upsert_then_load_all_includes_registered() {
    let tmp = tempfile::tempdir().unwrap();
    write_min_registry(tmp.path());
    upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
    let reg = load_all_corpora(tmp.path()).unwrap();
    assert!(reg.corpora.iter().any(|c| c.id == "ext-a"));
    assert!(reg.corpora.iter().any(|c| c.id == "repo-code-graph"));
}
#[test]
fn upsert_idempotent_by_id() {
    let tmp = tempfile::tempdir().unwrap();
    write_min_registry(tmp.path());
    upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
    upsert_registered_corpus(tmp.path(), &sample_corpus("ext-a")).unwrap();
    assert_eq!(load_registered_corpora(tmp.path()).iter().filter(|c| c.id == "ext-a").count(), 1);
}
#[test]
fn yaml_wins_id_collision() {
    let tmp = tempfile::tempdir().unwrap();
    write_min_registry(tmp.path());
    let mut collide = sample_corpus("repo-code-graph");
    collide.title = "HIJACKED".into();
    upsert_registered_corpus(tmp.path(), &collide).unwrap();
    let reg = load_all_corpora(tmp.path()).unwrap();
    let c = reg.corpora.iter().find(|c| c.id == "repo-code-graph").unwrap();
    assert_eq!(c.title, "Repo");
    assert_eq!(reg.corpora.iter().filter(|c| c.id == "repo-code-graph").count(), 1);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config upsert_then_load_all_includes_registered yaml_wins_id_collision upsert_idempotent_by_id`.

- [ ] **Step 4: Implement the overlay in `graphify.rs`.** Add after `CORPORA_REL_PATH`:

```rust
/// Runtime registration overlay (corpora created by `vox graphify index`).
pub const REGISTERED_REL_PATH: &str = ".vox/cache/graphify/registered.v1.json";
```

Add a struct near the others:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RegisteredCorporaFile {
    #[serde(default)]
    corpora: Vec<GraphifyCorpus>,
}
```

Add three functions after `load_graphify_corpora`:

```rust
/// Load runtime-registered corpora (empty if the overlay file is absent/unparseable).
pub fn load_registered_corpora(repo_root: &Path) -> Vec<GraphifyCorpus> {
    let path = repo_root.join(REGISTERED_REL_PATH);
    let Ok(raw) = fs::read_to_string(&path) else { return Vec::new(); };
    serde_json::from_str::<RegisteredCorporaFile>(&raw).map(|f| f.corpora).unwrap_or_default()
}

/// Insert-or-replace a corpus (by `id`) in the overlay.
pub fn upsert_registered_corpus(repo_root: &Path, corpus: &GraphifyCorpus) -> std::io::Result<()> {
    let path = repo_root.join(REGISTERED_REL_PATH);
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let mut corpora = load_registered_corpora(repo_root);
    corpora.retain(|c| c.id != corpus.id);
    corpora.push(corpus.clone());
    let raw = serde_json::to_string_pretty(&RegisteredCorporaFile { corpora })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, raw)
}

/// Canonical YAML corpora + runtime-registered corpora. YAML wins id collisions.
pub fn load_all_corpora(repo_root: &Path) -> Result<GraphifyCorporaRegistry, GraphifyError> {
    let mut reg = load_graphify_corpora(repo_root)?;
    let existing: HashSet<String> = reg.corpora.iter().map(|c| c.id.clone()).collect();
    for c in load_registered_corpora(repo_root) {
        if !existing.contains(&c.id) { reg.corpora.push(c); }
    }
    Ok(reg)
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-config upsert_then_load_all_includes_registered yaml_wins_id_collision upsert_idempotent_by_id` → PASS.

- [ ] **Step 6: Add the `Index` subcommand + switch `run()` loads.** In `mod.rs`, add to `GraphifyCmd` after `Rebuild`:

```rust
    /// Register an external target repository as a corpus and build it.
    Index {
        /// Path to the target repository (or subdirectory) to index.
        path: String,
        /// Corpus id (default: sanitized final path component).
        #[arg(long)]
        id: Option<String>,
        /// Extraction mode / semantic lens ("structural", "modules").
        #[arg(long, default_value = "structural")]
        mode: String,
    },
```

Update the import to add the overlay fn:

```rust
use vox_config::graphify::{
    CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError, GraphifyKnowledgeNode,
    assess_corpus_status, load_all_corpora, load_graphify_corpora, project_graph_nodes_for_ingest,
    upsert_registered_corpus,
};
```

In `run()`, change the `load_graphify_corpora(repo_root)` calls in the `Status`, `Ingest`, and `Rebuild` arms to `load_all_corpora(repo_root)` (leave the standalone `ingest_graph_corpus` helper). Add the `Index` arm:

```rust
        GraphifyCmd::Index { path, id, mode } => {
            let abs = std::fs::canonicalize(&path)
                .with_context(|| format!("canonicalize target path {path}"))?;
            // NOTE (Windows): canonicalize yields a verbatim `\\?\` prefix; it round-trips
            // through PathBuf/join/git -C fine. Do not strip it manually.
            let corpus_id = id
                .unwrap_or_else(|| abs.file_name().map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "target".to_string()))
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
                .collect::<String>();
            let corpus = GraphifyCorpus {
                id: corpus_id.clone(),
                title: format!("Indexed target: {}", abs.display()),
                scope_path: ".".to_string(),
                graph_path: format!(".vox/cache/graphify/{corpus_id}/graph.json"),
                manifest_path: format!(".vox/cache/graphify/{corpus_id}/.graphify_manifest.v1.json"),
                extraction_mode: Some(mode),
                default_for_intents: vec![],
                is_virtual: false,
                source_root: Some(abs.to_string_lossy().to_string()),
            };
            upsert_registered_corpus(repo_root, &corpus)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let source_dir = resolve_source_dir(repo_root, &corpus);
            let output_file = repo_root.join(&corpus.graph_path);
            let cache_dir = output_file
                .parent().ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                .join("file_cache");
            let meta = vox_graphify_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha_in(&abs).ok().flatten(),
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
            };
            println!("Indexing '{}' as corpus '{}'...", abs.display(), corpus_id);
            vox_graphify_reader::rebuild::rebuild_graph(repo_root, &source_dir, &output_file, &cache_dir, &meta)
                .map_err(|e| anyhow::anyhow!("Index rebuild failed: {}", e))?;
            println!("Corpus '{corpus_id}' registered and built.");
        }
```

- [ ] **Step 7: Build + smoke test.** `cargo build -p vox-cli` → clean. Then:

```bash
cargo run -p vox-cli -- graphify index crates/vox-graphify-reader --id self-reader --mode structural
cargo run -p vox-cli -- graphify status --corpus self-reader
```

Expected: "Corpus 'self-reader' registered and built."; `.vox/cache/graphify/self-reader/graph.json` and `.vox/cache/graphify/registered.v1.json` exist; status lists `self-reader`.

- [ ] **Step 8: Verify (Rule 7) + commit.**

```bash
git add crates/vox-config/src/graphify.rs crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): vox graphify index registers + builds external target repos"
```

---

## Task B3 `[SEQUENTIAL]` (shares rebuild.rs with Plan A — after A): `modules` semantic lens

**Files:**
- Create: `crates/vox-graphify-reader/src/lens.rs`
- Modify: `crates/vox-graphify-reader/src/lib.rs`
- Modify: `crates/vox-graphify-reader/src/rebuild.rs`
- Test: `crates/vox-graphify-reader/tests/lens_tests.rs`, `crates/vox-graphify-reader/tests/rebuild_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub mod cluster|let final_graph|\"node_count\"" crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/src/rebuild.rs`. Confirm Plan A's A1 manifest block (`let final_graph = serde_json::json!({` and `"node_count": nodes_val.len(),`) exists. If A1 not landed, STOP — apply Plan A first.

- [ ] **Step 2: Write failing tests.** Create `crates/vox-graphify-reader/tests/lens_tests.rs`:

```rust
use serde_json::json;
use vox_graphify_reader::lens::collapse_to_modules;

#[test]
fn collapses_to_modules_with_weighted_edges() {
    let g = json!({
        "nodes": [{"id":"a.rs::f","label":"f"},{"id":"b.rs::g","label":"g"},{"id":"b.rs::h","label":"h"}],
        "links": [{"source":"a.rs::f","target":"b.rs::g"},{"source":"a.rs::f","target":"b.rs::h"}]
    });
    let c = collapse_to_modules(&g);
    let ids: Vec<&str> = c["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["a.rs","b.rs"]);
    assert!(c["nodes"].as_array().unwrap().iter().all(|n| n["kind"]=="module"));
    let links = c["links"].as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["source"], "a.rs");
    assert_eq!(links[0]["target"], "b.rs");
    assert_eq!(links[0]["weight"], 2);
}
#[test]
fn intra_module_edges_dropped() {
    let g = json!({"nodes":[{"id":"a.rs::f","label":"f"},{"id":"a.rs::g","label":"g"}],
        "links":[{"source":"a.rs::f","target":"a.rs::g"}]});
    let c = collapse_to_modules(&g);
    assert_eq!(c["links"].as_array().unwrap().len(), 0);
    assert_eq!(c["nodes"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test lens_tests` → FAIL (no `lens` module).

- [ ] **Step 4: Create `lens.rs`.**

```rust
//! Semantic lenses: post-process a structural graph into a different "semantic line".
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn module_of(id: &str) -> &str {
    id.rsplit_once("::").map(|(m, _)| m).unwrap_or(id)
}

/// Collapse a `module::symbol` graph into a module-level graph: one node per module, one
/// weighted edge per inter-module call relationship. Intra-module edges drop. Both a
/// distinct semantic line and the coarse view that keeps very large repos navigable.
pub fn collapse_to_modules(graph: &Value) -> Value {
    let empty = vec![];
    let nodes = graph.get("nodes").and_then(|n| n.as_array()).unwrap_or(&empty);
    let links = graph.get("links").or_else(|| graph.get("edges"))
        .and_then(|n| n.as_array()).unwrap_or(&empty);

    let mut modules: HashSet<String> = HashSet::new();
    for n in nodes {
        if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
            modules.insert(module_of(id).to_string());
        }
    }
    let mut weights: HashMap<(String, String), u64> = HashMap::new();
    for l in links {
        if let (Some(s), Some(t)) = (
            l.get("source").and_then(|v| v.as_str()),
            l.get("target").and_then(|v| v.as_str()),
        ) {
            let (sm, tm) = (module_of(s).to_string(), module_of(t).to_string());
            if sm != tm { *weights.entry((sm, tm)).or_insert(0) += 1; }
        }
    }
    let mut module_list: Vec<String> = modules.into_iter().collect();
    module_list.sort();
    let nodes_val: Vec<Value> = module_list.into_iter()
        .map(|id| json!({"id": id, "label": id, "kind": "module", "community": "c_0"})).collect();
    let mut edge_list: Vec<((String, String), u64)> = weights.into_iter().collect();
    edge_list.sort();
    let links_val: Vec<Value> = edge_list.into_iter()
        .map(|((s, t), w)| json!({"source": s, "target": t, "weight": w})).collect();
    json!({"nodes": nodes_val, "links": links_val})
}
```

- [ ] **Step 5: Register module.** In `crates/vox-graphify-reader/src/lib.rs`, after `pub mod cluster;` add `pub mod lens;`.

- [ ] **Step 6: Apply the lens in `rebuild.rs` + count from final graph.** Replace Plan A's `let final_graph = serde_json::json!({ "nodes": nodes_val, "links": links_val });` with:

```rust
    let structural_graph = serde_json::json!({ "nodes": nodes_val, "links": links_val });
    let final_graph = if meta.extraction_mode.as_deref() == Some("modules") {
        super::lens::collapse_to_modules(&structural_graph)
    } else {
        structural_graph
    };
    let node_count = final_graph["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
    let edge_count = final_graph["links"].as_array().map(|a| a.len()).unwrap_or(0);
```

In the manifest `json!` (from A1), change the two count lines to `"node_count": node_count,` and `"edge_count": edge_count,`. (The `graph_bytes`/digest/write logic already serializes `final_graph`.)

- [ ] **Step 7: Add a rebuild-with-lens test.** Append to `crates/vox-graphify-reader/tests/rebuild_tests.rs`:

```rust
#[test]
fn modules_mode_produces_module_graph() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn f() { g(); }").unwrap();
    fs::write(src.join("b.rs"), "fn g() {}").unwrap();
    let out = tmp.path().join("out/graph.json");
    let mut meta = RebuildMeta::default();
    meta.extraction_mode = Some("modules".to_string());
    rebuild_graph(tmp.path(), &src, &out, &tmp.path().join("out/fc"), &meta).unwrap();
    let g = read_graph(&out);
    assert!(g["nodes"].as_array().unwrap().iter().all(|n| n["kind"] == "module"));
}
```

- [ ] **Step 8: Run → PASS + Rule 7 + arch-check + commit.** `cargo test -p vox-graphify-reader`; `cargo run -p vox-arch-check`; then:

```bash
git add crates/vox-graphify-reader/src/lens.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/lens_tests.rs crates/vox-graphify-reader/tests/rebuild_tests.rs
git commit -m "feat(graphify): modules semantic lens for coarse, large-repo-safe graphs"
```

---

## Parallelization summary

- **B1 → B2 SEQUENTIAL** (both edit `graphify.rs` + `mod.rs`).
- **B3 SEQUENTIAL after Plan A** (shares `rebuild.rs`). Disjoint from B1/B2's files, but B3 depends on Plan A's manifest block, not on B1/B2 — it MAY run in parallel with B1/B2 ONLY if a separate agent owns `vox-graphify-reader` and no B1/B2 task touches it (they do not). Safe to parallelize B3 ∥ (B1→B2) on two agents.

## Self-Review

- **Spec coverage:** external target repos (B1+B2), multiple corpora per source along semantic lines (B3 lens by `extraction_mode`; registry already allows N corpora per scope), "automatically index" via one command (B2), data-size foothold (`modules` lens).
- **Placeholder scan:** none. The Windows `canonicalize` UNC caveat is documented inline (evidence-based add). Every literal-breaking field add is fixed in-task (Step 5).
- **Type consistency:** `source_root: Option<String>` in every literal; `resolve_source_dir(repo_root, corpus)` identical across tasks; overlay fns (`load_all_corpora`/`upsert_registered_corpus`/`load_registered_corpora`) identical in graphify.rs and the CLI import; lens edits the exact A1 `final_graph` block.
- **Antigravity fit:** atomic+green+commit per task; verify-before-use first; parallel tags; the field-add task explicitly enumerates and fixes all literals (a classic fast-model compile-break trap).
- **Deferred:** auto-rerun/cost-gate = Plan C; retention/GC of indexed snapshots = Plan D; GUI surfacing of registered corpora = Plan E.
