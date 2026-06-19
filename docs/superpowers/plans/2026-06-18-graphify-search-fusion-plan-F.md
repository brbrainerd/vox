# Plan F — Graphify Search Fusion (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan B recommended** (multi-corpus). Works standalone but is most useful with B's corpora.

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed.** Crash between tasks → compiling, tested tree.
2. **Verify-before-use.** First step is an `rg`/read confirming exact symbols. Differs → STOP.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note. No looping.
5. **Parallel dispatch.** Honor tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all`; automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs.
7. **Verification ritual** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Make graphify reachable from intent-driven search by activating the dormant `default_for_intents` routing: a search with an `intent` and no explicit corpus resolves to the corpus registered for that intent.

**Architecture:** The `GraphifyCorpus.default_for_intents` field is loaded from YAML but **read nowhere**. (F1) add a pure `select_corpus_for_intent(registry, intent)` to `vox-config` (unit + bundled-registry tests). (F2) add an optional `intent` to the `vox_graphify_search` MCP tool and route through F1 when no explicit `corpus` is given.

**Tech Stack:** Rust; `serde`; `vox-config::graphify`; `vox-orchestrator-mcp` (`#[tokio::test]`).

> **Scope note (deferred, not placeholder):** the larger fusion — adding graphify hits as a source inside `run_retrieval_bundle` (memory_tools/retrieval.rs:288) and RRF-merging them with knowledge/memory/chunks — is a separate, bigger task touching a 120-line function. The recipe (load intent-matched corpora, `lexical_search_graph`, format as `[graphify:corpus:node]` lines, `rrf_merge_line_lists`) is recorded in the suite index for a follow-on plan. Plan F ships the airtight routing primitive + its activation in the existing graphify search tool.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-config/src/graphify.rs` | `select_corpus_for_intent` | Modify (F1) |
| `crates/vox-orchestrator-mcp/src/graphify_tools.rs` | `intent` param + routing | Modify (F2) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "default_for_intents" crates/` — confirm the field is read NOWHERE except its definition (the gap F1 closes).
- `rg -n "pub struct GraphifySearchParams|pub async fn graphify_search|default_corpus_id|let corpus_id" crates/vox-orchestrator-mcp/src/graphify_tools.rs` — note the `GraphifySearchParams` struct, the `graphify_search` handler, and the EXACT line that resolves `corpus_id` (the registry binding name + the `unwrap_or` to `default_corpus_id`). F2 replaces that one expression.
- `rg -n "fn write_registry|fn write_sample_graph|fn test_state_for_repo|#\[tokio::test\]" crates/vox-orchestrator-mcp/src/graphify_tools.rs` — confirm the test harness helpers.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task F1 `[SEQUENTIAL]`: Pure intent → corpus routing

**Files:**
- Modify: `crates/vox-config/src/graphify.rs`
- Test: inline `#[cfg(test)]` in `crates/vox-config/src/graphify.rs`

- [ ] **Step 1 (verify-before-use):** Run the first two Pre-flight lines. Confirm `default_for_intents` is read nowhere and that `GraphifyCorporaRegistry { default_corpus_id, ttl_days_default, corpora }` is the registry type. Differs → STOP.

- [ ] **Step 2: Write the failing tests.** In `graphify.rs` `mod tests`:

```rust
#[test]
fn intent_routing_picks_first_matching_corpus() {
    // synthetic registry
    let mk = |id: &str, intents: &[&str]| GraphifyCorpus {
        id: id.into(), title: id.into(), scope_path: ".".into(),
        graph_path: "g".into(), manifest_path: "m".into(),
        extraction_mode: None, default_for_intents: intents.iter().map(|s| s.to_string()).collect(),
        is_virtual: false,
        // If Plan B landed, add: source_root: None,
    };
    let reg = GraphifyCorporaRegistry {
        default_corpus_id: "a".into(), ttl_days_default: 30,
        corpora: vec![mk("a", &["code_navigation"]), mk("b", &["gui_surface"])],
    };
    assert_eq!(select_corpus_for_intent(&reg, "gui_surface").as_deref(), Some("b"));
    assert_eq!(select_corpus_for_intent(&reg, "code_navigation").as_deref(), Some("a"));
    assert_eq!(select_corpus_for_intent(&reg, "nonexistent"), None);
}

#[test]
fn intent_routing_against_bundled_registry() {
    // Exercises the real contract data (repo-code-graph↔code_navigation, vox-gui-surface↔gui_surface).
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("contracts/retrieval");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("graphify-corpora.v1.yaml"),
        include_str!("../../../contracts/retrieval/graphify-corpora.v1.yaml"),
    )
    .unwrap();
    let reg = load_graphify_corpora(tmp.path()).unwrap();
    assert_eq!(select_corpus_for_intent(&reg, "code_navigation").as_deref(), Some("repo-code-graph"));
    assert_eq!(select_corpus_for_intent(&reg, "gui_surface").as_deref(), Some("vox-gui-surface"));
}
```

> The `include_str!` path is relative to `graphify.rs` (i.e. `crates/vox-config/src/`). Confirm the depth with `rg -n "include_str" crates/vox-config/src/` if an existing test already embeds this file, and copy its exact relative path.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config intent_routing_picks_first_matching_corpus intent_routing_against_bundled_registry` → FAIL (`select_corpus_for_intent` missing).

- [ ] **Step 4: Implement** in `graphify.rs` (after `load_graphify_corpora`):

```rust
/// First corpus id whose `default_for_intents` contains `intent`, if any.
/// Activates the otherwise-dormant intent-routing field.
pub fn select_corpus_for_intent(reg: &GraphifyCorporaRegistry, intent: &str) -> Option<String> {
    reg.corpora
        .iter()
        .find(|c| c.default_for_intents.iter().any(|i| i == intent))
        .map(|c| c.id.clone())
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-config intent_routing_picks_first_matching_corpus intent_routing_against_bundled_registry` → PASS.

- [ ] **Step 6: Verify (Rule 7) + commit.**

```bash
git add crates/vox-config/src/graphify.rs
git commit -m "feat(graphify): select_corpus_for_intent — activate default_for_intents routing"
```

---

## Task F2 `[SEQUENTIAL]`: Wire `intent` into `vox_graphify_search`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/graphify_tools.rs`
- Test: inline `#[cfg(test)]` in `crates/vox-orchestrator-mcp/src/graphify_tools.rs`

- [ ] **Step 1 (verify-before-use):** Run the second + third Pre-flight lines. Quote the exact `corpus_id` resolution line (e.g. `let corpus_id = params.corpus.clone().unwrap_or_else(|| reg.default_corpus_id.clone());`) and the registry binding name (call it `reg` below — adapt if different). Confirm the harness helpers + `#[tokio::test]`. Differs → STOP.

- [ ] **Step 2: Write the failing test.** In `graphify_tools.rs` `#[cfg(test)]` module (mirroring `graphify_search_returns_matching_hit`):

```rust
#[tokio::test]
async fn graphify_search_routes_by_intent_when_no_corpus() {
    let tmp = tempfile::tempdir().unwrap();
    write_registry(tmp.path());
    write_sample_graph(tmp.path()); // sample graph for repo-code-graph
    let state = test_state_for_repo(tmp.path().to_path_buf());
    let json = graphify_search(
        &state,
        GraphifySearchParams {
            corpus: None,
            intent: Some("code_navigation".into()), // → repo-code-graph
            query: "authentication".into(),
            limit: None,
            persist: false,
        },
    )
    .await;
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(parsed.get("corpus_id"), Some(&serde_json::json!("repo-code-graph")));
}
```

> If the response key is not `corpus_id`, use the key the existing `graphify_search_returns_matching_hit` test asserts on (confirm in Step 1) and assert it equals `"repo-code-graph"`.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator-mcp graphify_search_routes_by_intent_when_no_corpus` → FAIL to compile (`intent` field missing on `GraphifySearchParams`).

- [ ] **Step 4: Add the `intent` field.** In `GraphifySearchParams`, add (after `corpus`):

```rust
    /// Optional intent; when `corpus` is omitted, routes to the corpus registered for this
    /// intent (`default_for_intents`) before falling back to `default_corpus_id`.
    #[serde(default)]
    pub intent: Option<String>,
```

- [ ] **Step 5: Route through F1.** Replace the existing `corpus_id` resolution expression (from Step 1) with intent-aware resolution. Using the registry binding `reg`:

```rust
    let corpus_id = params
        .corpus
        .clone()
        .or_else(|| {
            params
                .intent
                .as_deref()
                .and_then(|i| vox_config::graphify::select_corpus_for_intent(&reg, i))
        })
        .unwrap_or_else(|| reg.default_corpus_id.clone());
```

> Confirm `reg` is loaded BEFORE this line (it must be, since the old code already read `reg.default_corpus_id`). If the registry binding has a different name, use it verbatim.

- [ ] **Step 6: Fix other construction sites.** Adding a field breaks any `GraphifySearchParams { .. }` literal lacking `intent`. Run `rg -n "GraphifySearchParams \{" crates/` and add `intent: None,` to each (e.g. the existing `graphify_search_returns_matching_hit` test). `cargo build -p vox-orchestrator-mcp` must be clean.

- [ ] **Step 7: Run → PASS.** `cargo test -p vox-orchestrator-mcp graphify_search_routes_by_intent_when_no_corpus graphify_search_returns_matching_hit` → PASS.

- [ ] **Step 8: Verify (Rule 7) + arch-check + commit.**

```bash
git add crates/vox-orchestrator-mcp/src/graphify_tools.rs
git commit -m "feat(graphify): intent routing in vox_graphify_search (corpus inferred from intent)"
```

---

## Parallelization summary
- **F1 → F2 SEQUENTIAL** (F2's routing calls F1's `select_corpus_for_intent`).

## Self-Review
- **Spec coverage:** "integrate with Vox's search capabilities" — graphify search is now intent-routable (the field that was meant to drive this was dead). The deeper unified-retrieval fusion is explicitly DEFERRED with a recorded recipe — scoped down, not stubbed.
- **Placeholder scan:** none. The one uncertain anchor (the exact `corpus_id` line + response key) is gated by a verify-before-use step that quotes it before editing.
- **Type consistency:** `select_corpus_for_intent(reg, intent) -> Option<String>` identical across F1 + F2; `intent: Option<String>` added once and back-filled to all literals; the synthetic test corpus literal carries `source_root: None` only if Plan B landed (noted inline).
- **Antigravity fit:** atomic+green+commit; the routing CORRECTNESS is proven by pure + bundled-registry tests (deterministic), while the async handler change is proven by one `#[tokio::test]` against the known harness + a field-backfill compile guard.
