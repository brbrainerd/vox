# Operation Sequence Mining Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mine recurring tool-call procedures from the `agent_operations` stream into ranked skill `Candidate`s, surfaced via `vox skill suggest`.

**Architecture:** A pure miner in `vox-skill-discovery` (no DB dependency) finds contiguous tool-call n-grams that recur across sessions; a new `vox-db` read feeds it; a `vox-cli` subcommand wires read → map → mine → render. Reuses the existing `Candidate`/`DraftFrontmatter`/`render_*` types.

**Tech Stack:** Rust — `vox-db` (turso), `vox-skill-discovery`, `vox-cli` (clap), `serde_json`.

**Spec:** `docs/superpowers/specs/2026-06-30-operation-sequence-mining-design.md`

**Sub-project 2 of 4** (capture ✅ → **mine** → propose → author/install). Ships the miner + CLI only; no MCP/HITL.

---

## Spec refinement (intentional)

The spec named the `vox-discover` binary for the CLI surface. To keep the
`vox-skill-discovery` library **DB-free** (it would otherwise gain `vox-db` +
`tokio`/turso), the CLI lives in **`vox-cli`** instead (already depends on
`vox-db`, `tokio`, and the `skill` subcommand) as **`vox skill suggest`**. The
miner library stays pure (takes rows, returns candidates); `vox-cli` owns the DB
glue. Same capability, cleaner dependency graph.

## Codebase facts — VERIFIED 2026-06-30

| Fact | Value |
|---|---|
| Read-query idiom | `self.conn.query("SELECT …", ()).await?` → `while let Some(row)=rows.next().await? { row.get::<T>(i) }` (see `list_active_sessions`, `ops_agents.rs`). Reads use `self.conn` directly (no breaker). `StoreError = crate::store::types::StoreError`. |
| Test harness | `VoxDb::connect(DbConfig::Memory).await` (`use crate::{DbConfig, VoxDb};`), `--features local`. |
| agent_operations cols | `ts_ms, session_id, agent_id, tool_name, args_redacted, result_redacted, duration_ms, is_error` (schema v81, on `main`). |
| Candidate types | `Candidate{kind:CandidateKind, members:Vec<String>, score:f32, suggested_action:String, draft_frontmatter:Option<DraftFrontmatter>}`; `DraftFrontmatter{name,description,category,tags}`; `CandidateKind{RepeatedCode,DuplicatesInstalled,SsotDrift,ModelPromptVariant}` (`candidate.rs`). |
| lib exports | `vox-skill-discovery/src/lib.rs` re-exports candidate/catalog/code_miner/options/report items; add `op_miner` there. |
| render | `vox_skill_discovery::{render_json(&[Candidate])->Result<String>, render_terminal(&[Candidate])->String}`. |
| vox-cli skill cmd | `crates/vox-cli/src/commands/extras/skill_cmd.rs` `enum SkillCmd` + `run(cmd)`; handlers in `extras/ars/`. Gated by `--features ars`. vox-cli already deps `vox-db`, `tokio`, `vox-plugin-host`. |
| name rule | Agent Skills: `[a-z0-9-]`, ≤64, no leading/trailing/double hyphen (mirror `vox-plugin-host::user_install::validate_skill_name`). |

## File Structure

- **Modify** `crates/vox-db/src/store/ops_agents.rs` — `OperationRow` + `list_recent_operations`.
- **Modify** `crates/vox-skill-discovery/src/candidate.rs` — add `CandidateKind::RepeatedOperations`.
- **Create** `crates/vox-skill-discovery/src/op_miner.rs` — `MinedOp`, `OpMiningOptions`, `arg_keys`, `mine_repeated_operations` (pure).
- **Modify** `crates/vox-skill-discovery/src/lib.rs` — `pub mod op_miner;` + re-exports.
- **Modify** `crates/vox-cli/Cargo.toml` — add `vox-skill-discovery` dep.
- **Modify** `crates/vox-cli/src/commands/extras/skill_cmd.rs` — `Suggest` variant + dispatch.
- **Create** `crates/vox-cli/src/commands/extras/ars/skill_suggest.rs` — the handler.
- **Modify** `crates/vox-cli/src/commands/extras/ars/mod.rs` — register + re-export the handler.

## Execution notes
- **TDD** is strict for Task 2 (pure miner — the bulk of the logic). Task 1 has a DB round-trip test; Task 3 is glue verified by build + a smoke run.
- Tasks 1 and 2 are independent (parallelizable). Task 3 depends on both. Serialize edits per file.
- Commit after each task. Run `vox-db` tests with `--features local`.

---

## Task 1: `vox-db` read path

**Files:** Modify `crates/vox-db/src/store/ops_agents.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod operation_tests` in `ops_agents.rs`:

```rust
#[tokio::test]
async fn list_recent_operations_orders_and_limits() {
    use crate::{DbConfig, VoxDb};
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    for (i, tool) in ["a", "b", "c"].iter().enumerate() {
        db.record_operation(Some("s1"), None, tool, "{}", Some("ok"), i as i64, false)
            .await
            .expect("record");
    }
    let rows = db.list_recent_operations(2).await.expect("list");
    assert_eq!(rows.len(), 2, "respects limit");
    assert!(rows.iter().all(|r| r.session_id.as_deref() == Some("s1")));
    assert!(rows.iter().any(|r| r.tool_name == "c"), "includes most recent");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-db --features local list_recent_operations_orders_and_limits`
Expected: FAIL — `no method named list_recent_operations` / `cannot find type OperationRow`.

- [ ] **Step 3: Implement `OperationRow` + the read**

In `ops_agents.rs`, add the struct above the `impl crate::VoxDb` block (near the top, after the `use` lines):

```rust
/// One captured operation row (subset used by sequence mining).
#[derive(Debug, Clone)]
pub struct OperationRow {
    pub ts_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tool_name: String,
    pub args_redacted: String,
}
```

Inside the `impl crate::VoxDb` block (next to `record_operation`), add:

```rust
/// Most-recent `limit` captured operations, newest first. Mining regroups by session.
pub async fn list_recent_operations(&self, limit: i64) -> Result<Vec<OperationRow>, StoreError> {
    let mut rows = self
        .conn
        .query(
            "SELECT ts_ms, session_id, agent_id, tool_name, args_redacted
             FROM agent_operations ORDER BY ts_ms DESC, id DESC LIMIT ?1",
            turso::params![limit],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(OperationRow {
            ts_ms: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
            session_id: row.get(1).ok(),
            agent_id: row.get(2).ok(),
            tool_name: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            args_redacted: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
        });
    }
    Ok(out)
}
```

> `turso::params!` is already imported at the top of this file. `row.get::<Option<String>>(i).ok()` yields `None` on SQL NULL.

- [ ] **Step 4: Run the test**

Run: `cargo test -p vox-db --features local list_recent_operations_orders_and_limits`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/store/ops_agents.rs
git commit -m "feat(db): list_recent_operations read over agent_operations"
```

---

## Task 2: pure miner in `vox-skill-discovery`

**Files:**
- Modify `crates/vox-skill-discovery/src/candidate.rs`
- Create `crates/vox-skill-discovery/src/op_miner.rs`
- Modify `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Add the `RepeatedOperations` variant**

In `crates/vox-skill-discovery/src/candidate.rs`, add to `enum CandidateKind`:

```rust
    /// A recurring sequence of tool calls (a procedure) that could become a skill.
    RepeatedOperations,
```

- [ ] **Step 2: Write the failing miner tests**

Create `crates/vox-skill-discovery/src/op_miner.rs` with ONLY the test module first:

```rust
//! Mines recurring tool-call procedures from captured operations. Pure: takes
//! `MinedOp` rows, returns advisory `Candidate`s. No DB / IO dependency.

use crate::candidate::{Candidate, CandidateKind, DraftFrontmatter};
use std::collections::BTreeSet;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn op(session: &str, ts: i64, tool: &str, keys: &[&str]) -> MinedOp {
        MinedOp {
            ts_ms: ts,
            session_id: session.to_string(),
            tool_name: tool.to_string(),
            arg_keys: keys.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn seq(session: &str, base: i64, tools: &[&str]) -> Vec<MinedOp> {
        tools.iter().enumerate().map(|(i, t)| op(session, base + i as i64, t, &[])).collect()
    }

    fn default_opts() -> OpMiningOptions {
        OpMiningOptions { min_len: 2, max_len: 6, min_occurrences: 3, min_distinct_sessions: 2 }
    }

    #[test]
    fn detects_sequence_recurring_across_sessions() {
        let mut ops = Vec::new();
        // A->B->C three times across two sessions (2 in s1, 1 in s2).
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s1", 10, &["a", "b", "c"]));
        ops.extend(seq("s2", 0, &["a", "b", "c"]));
        let cands = mine_repeated_operations(&ops, &default_opts());
        // The full A->B->C 3-gram must be among the candidates.
        let abc = cands.iter().find(|c| {
            c.draft_frontmatter.as_ref().map(|d| d.name.as_str()) == Some("a-b-c")
        });
        assert!(abc.is_some(), "expected a-b-c candidate, got {cands:?}");
        let abc = abc.unwrap();
        assert_eq!(abc.kind, CandidateKind::RepeatedOperations);
        assert!(abc.draft_frontmatter.as_ref().unwrap().description.contains("3"));
    }

    #[test]
    fn excludes_sequence_confined_to_one_session() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s1", 10, &["a", "b", "c"]));
        ops.extend(seq("s1", 20, &["a", "b", "c"])); // 3x but ONE session
        let cands = mine_repeated_operations(&ops, &default_opts());
        assert!(cands.is_empty(), "min_distinct_sessions=2 should exclude single-session, got {cands:?}");
    }

    #[test]
    fn excludes_below_min_occurrences() {
        let mut ops = Vec::new();
        ops.extend(seq("s1", 0, &["a", "b", "c"]));
        ops.extend(seq("s2", 0, &["a", "b", "c"])); // 2x total < 3
        let cands = mine_repeated_operations(&ops, &default_opts());
        assert!(cands.is_empty());
    }

    #[test]
    fn arg_key_shape_distinguishes_ops() {
        // read{path}->write recurs 3× across 3 sessions (passes threshold).
        // read{path,range}->write recurs only 2× — it must NOT merge with the
        // {path} variant to reach 5×. Different arg-key shape ⇒ different op_key.
        let mut ops = Vec::new();
        for s in ["s1", "s2", "s3"] {
            ops.push(op(s, 0, "read", &["path"]));
            ops.push(op(s, 1, "write", &[]));
        }
        for s in ["s4", "s5"] {
            ops.push(op(s, 0, "read", &["path", "range"]));
            ops.push(op(s, 1, "write", &[]));
        }
        let cands = mine_repeated_operations(&ops, &default_opts());
        // Both variants render tool-name "read-write"; only the {path} one (3×/3)
        // passes. If op_keys wrongly merged, the survivor would report 5×.
        let rw: Vec<_> = cands
            .iter()
            .filter(|c| c.draft_frontmatter.as_ref().map(|d| d.name.as_str()) == Some("read-write"))
            .collect();
        assert_eq!(rw.len(), 1, "arg-shapes must not merge; got {cands:?}");
        assert!(
            rw[0].draft_frontmatter.as_ref().unwrap().description.contains("3×"),
            "expected 3× (not merged to 5×); got {:?}",
            rw[0].draft_frontmatter
        );
    }

    #[test]
    fn arg_keys_parses_sorts_dedups() {
        assert_eq!(arg_keys(r#"{"b":1,"a":2}"#), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(arg_keys("not json"), Vec::<String>::new());
        assert_eq!(arg_keys("[1,2]"), Vec::<String>::new());
    }

    #[test]
    fn empty_and_single_op_sessions_yield_nothing() {
        assert!(mine_repeated_operations(&[], &default_opts()).is_empty());
        assert!(mine_repeated_operations(&seq("s1", 0, &["a"]), &default_opts()).is_empty());
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p vox-skill-discovery op_miner`
Expected: FAIL — `cannot find type MinedOp` / `function mine_repeated_operations`.

- [ ] **Step 4: Implement the miner**

Insert ABOVE the `#[cfg(test)] mod tests` block in `op_miner.rs`:

```rust
/// One captured operation the miner reasons over. The caller (vox-cli) maps a
/// `vox-db` row into this; rows with no `session_id` are dropped before mining.
#[derive(Debug, Clone)]
pub struct MinedOp {
    pub ts_ms: i64,
    pub session_id: String,
    pub tool_name: String,
    /// Top-level arg keys, sorted + deduped (values are redacted/ignored).
    pub arg_keys: Vec<String>,
}

/// Tuning for sequence mining.
#[derive(Debug, Clone)]
pub struct OpMiningOptions {
    pub min_len: usize,
    pub max_len: usize,
    pub min_occurrences: usize,
    pub min_distinct_sessions: usize,
}

impl Default for OpMiningOptions {
    fn default() -> Self {
        Self { min_len: 2, max_len: 6, min_occurrences: 3, min_distinct_sessions: 2 }
    }
}

/// Extract sorted, deduped top-level keys from a JSON object string. Non-object
/// or unparseable input → empty (the op still participates by tool_name).
pub fn arg_keys(args_json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(serde_json::Value::Object(map)) => {
            let set: BTreeSet<String> = map.keys().cloned().collect();
            set.into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Stable per-op identity: `tool_name(k1,k2,...)`.
fn op_key(op: &MinedOp) -> String {
    if op.arg_keys.is_empty() {
        op.tool_name.clone()
    } else {
        format!("{}({})", op.tool_name, op.arg_keys.join(","))
    }
}

/// Skill-name-safe slug from an n-gram of tool names (Agent Skills `name` rule).
fn ngram_name(tools: &[String]) -> String {
    let raw = tools.join("-").to_ascii_lowercase();
    let mut s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    let s: String = s.chars().take(64).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "procedure".to_string() } else { s }
}

/// Mine recurring contiguous tool-call sequences into ranked candidates.
pub fn mine_repeated_operations(ops: &[MinedOp], opts: &OpMiningOptions) -> Vec<Candidate> {
    // Group by session, ordered by ts.
    let mut by_session: HashMap<&str, Vec<&MinedOp>> = HashMap::new();
    for o in ops {
        by_session.entry(o.session_id.as_str()).or_default().push(o);
    }
    for v in by_session.values_mut() {
        v.sort_by_key(|o| o.ts_ms);
    }

    // Per n-gram (joined op_keys): total count, distinct sessions, the tool list,
    // and a few provenance anchors.
    struct Agg {
        count: usize,
        sessions: BTreeSet<String>,
        tools: Vec<String>,
        anchors: Vec<String>,
    }
    let mut agg: HashMap<String, Agg> = HashMap::new();

    for (sid, list) in &by_session {
        let keys: Vec<String> = list.iter().map(|o| op_key(o)).collect();
        let tools: Vec<String> = list.iter().map(|o| o.tool_name.clone()).collect();
        let n = keys.len();
        for len in opts.min_len..=opts.max_len {
            if len > n {
                break;
            }
            for start in 0..=(n - len) {
                let gram = keys[start..start + len].join(" -> ");
                let e = agg.entry(gram).or_insert_with(|| Agg {
                    count: 0,
                    sessions: BTreeSet::new(),
                    tools: tools[start..start + len].to_vec(),
                    anchors: Vec::new(),
                });
                e.count += 1;
                e.sessions.insert((*sid).to_string());
                if e.anchors.len() < 20 {
                    e.anchors.push(format!("session:{}@{}", sid, list[start].ts_ms));
                }
            }
        }
    }

    let mut out: Vec<Candidate> = agg
        .into_values()
        .filter(|a| a.count >= opts.min_occurrences && a.sessions.len() >= opts.min_distinct_sessions)
        .map(|a| {
            let arrow = a.tools.join(" → ");
            let name = ngram_name(&a.tools);
            Candidate {
                kind: CandidateKind::RepeatedOperations,
                members: a.anchors,
                score: (a.count * a.tools.len()) as f32,
                suggested_action: "Save recurring procedure as a skill".to_string(),
                draft_frontmatter: Some(DraftFrontmatter {
                    name,
                    description: format!(
                        "Recurring procedure: {arrow} (seen {}× across {} sessions)",
                        a.count,
                        a.sessions.len()
                    ),
                    category: "workflow".to_string(),
                    tags: vec!["auto-discovered".to_string(), "operations".to_string()],
                }),
            }
        })
        .collect();
    // Highest score first; stable tiebreak by name for deterministic output.
    out.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let xn = x.draft_frontmatter.as_ref().map(|d| d.name.as_str()).unwrap_or("");
                let yn = y.draft_frontmatter.as_ref().map(|d| d.name.as_str()).unwrap_or("");
                xn.cmp(yn)
            })
    });
    out
}
```

> VERIFIED: no `match`/`if let` on `CandidateKind` exists in the crate (only constructions + `Debug`-derived rendering), so adding `RepeatedOperations` is safe — no match arms to update.

- [ ] **Step 5: Export from the library**

In `crates/vox-skill-discovery/src/lib.rs`, add after the existing `pub mod` lines and re-exports:

```rust
pub mod op_miner;
pub use op_miner::{MinedOp, OpMiningOptions, arg_keys, mine_repeated_operations};
```

- [ ] **Step 6: Surface `draft_frontmatter` in the terminal report**

VERIFIED gap: `render_terminal` (`crates/vox-skill-discovery/src/report.rs`) prints `kind`/`score`/`action`/`members` but NOT `draft_frontmatter` — so `vox skill suggest` (terminal mode) would hide the actual suggested skill name/description (the whole point). `render_json` already serializes the full struct. Extend `render_terminal`: after the header `push_str` and before the `members:` loop, add the draft block. Locate the existing loop body (it pushes the `[i] {:?} (score …) action: … members:` header) and insert:

```rust
        if let Some(df) = &c.draft_frontmatter {
            out.push_str(&format!("    suggested skill: {} — {}\n", df.name, df.description));
        }
```

Add a test to `report.rs`'s `#[cfg(test)] mod tests` (it already imports `CandidateKind`):

```rust
    #[test]
    fn terminal_report_shows_draft_skill() {
        use crate::candidate::{Candidate, DraftFrontmatter};
        let c = Candidate {
            kind: CandidateKind::RepeatedOperations,
            members: vec!["session:s1@0".into()],
            score: 6.0,
            suggested_action: "Save recurring procedure as a skill".into(),
            draft_frontmatter: Some(DraftFrontmatter {
                name: "a-b-c".into(),
                description: "Recurring procedure: a → b → c (seen 3× across 2 sessions)".into(),
                category: "workflow".into(),
                tags: vec!["auto-discovered".into()],
            }),
        };
        let out = render_terminal(std::slice::from_ref(&c));
        assert!(out.contains("suggested skill: a-b-c"), "got: {out}");
    }
```

> This is backward-compatible: code/installed candidates with `draft_frontmatter: None` print as before.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p vox-skill-discovery`
Expected: PASS (op_miner 6 + report incl. the new `terminal_report_shows_draft_skill`).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-skill-discovery/src/candidate.rs crates/vox-skill-discovery/src/op_miner.rs crates/vox-skill-discovery/src/lib.rs crates/vox-skill-discovery/src/report.rs
git commit -m "feat(skill-discovery): mine_repeated_operations sequence miner + draft in terminal report"
```

---

## Task 3: `vox skill suggest` CLI

**Files:**
- Modify `crates/vox-cli/Cargo.toml`
- Modify `crates/vox-cli/src/commands/extras/skill_cmd.rs`
- Create `crates/vox-cli/src/commands/extras/ars/skill_suggest.rs`
- Modify `crates/vox-cli/src/commands/extras/ars/mod.rs`

- [ ] **Step 1: Add the dependency**

In `crates/vox-cli/Cargo.toml`, under the `ars` feature's deps (where `vox-skills`/`vox-openclaw-runtime` are declared optional), add `vox-skill-discovery`. First add to `[dependencies]`:

```toml
vox-skill-discovery = { workspace = true, optional = true }
```

Then add it to the `ars` feature list (mirroring the existing `ars = ["dep:vox-skills", "dep:vox-openclaw-runtime"]`):

```toml
ars = ["dep:vox-skills", "dep:vox-openclaw-runtime", "dep:vox-skill-discovery"]
```

> VERIFIED: `vox-skill-discovery` is already in the root `[workspace.dependencies]` (`Cargo.toml:182`) — no root change needed.

- [ ] **Step 2: Add the `Suggest` subcommand**

In `crates/vox-cli/src/commands/extras/skill_cmd.rs`, add a variant to `enum SkillCmd`:

```rust
    /// Suggest skills from recurring captured operation sequences (advisory).
    Suggest {
        /// Max recent operations to analyze.
        #[arg(long, default_value_t = 5000)]
        limit: i64,
        /// Output format: terminal | json
        #[arg(long, default_value = "terminal")]
        format: String,
    },
```

And a dispatch arm in `run`:

```rust
        SkillCmd::Suggest { limit, format } => ars::skill_suggest(limit, &format).await,
```

- [ ] **Step 3: Write the handler**

Create `crates/vox-cli/src/commands/extras/ars/skill_suggest.rs`:

```rust
use anyhow::Result;

use vox_skill_discovery::{
    OpMiningOptions, MinedOp, arg_keys, mine_repeated_operations, render_json, render_terminal,
};

/// `vox skill suggest` — mine recurring operation procedures into advisory candidates.
// VERIFIED: `vox_db::Codex` is a type alias for `VoxDb` (vox-db/src/lib.rs:332),
// so `connect_default()` yields a `VoxDb` and `list_recent_operations` is callable
// directly — same pattern the other ars handlers use (registry.rs / eval_promote.rs).
pub async fn skill_suggest(limit: i64, format: &str) -> Result<()> {
    let db = match vox_db::Codex::connect_default().await {
        Ok(db) => db,
        Err(_) => {
            println!("No operations captured yet (operation capture disabled or DB unavailable).");
            return Ok(());
        }
    };
    let rows = db
        .list_recent_operations(limit)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if rows.is_empty() {
        println!("No operations captured yet.");
        return Ok(());
    }
    // Map DB rows -> MinedOp, dropping rows with no session_id.
    let ops: Vec<MinedOp> = rows
        .into_iter()
        .filter_map(|r| {
            r.session_id.map(|sid| MinedOp {
                ts_ms: r.ts_ms,
                session_id: sid,
                tool_name: r.tool_name,
                arg_keys: arg_keys(&r.args_redacted),
            })
        })
        .collect();
    let candidates = mine_repeated_operations(&ops, &OpMiningOptions::default());
    let rendered = match format {
        "json" => render_json(&candidates)?,
        _ => render_terminal(&candidates),
    };
    println!("{rendered}");
    Ok(())
}
```

> VERIFIED: `Codex = VoxDb` (alias), so `connect_default()` returns a `VoxDb` and
> `list_recent_operations` (added in Task 1 on `impl crate::VoxDb`) is callable
> directly — no adjustment needed.

- [ ] **Step 4: Register the handler**

In `crates/vox-cli/src/commands/extras/ars/mod.rs`, add the module + re-export mirroring the sibling handlers (e.g. how `skills_crud` / `eval_promote` are declared):

VERIFIED idiom: `ars/mod.rs` uses `mod <name>;` + `pub use <name>::{...};` (not `pub(crate)`). Add `mod skill_suggest;` to the module block and to the re-export block:

```rust
pub use skill_suggest::skill_suggest;
```

- [ ] **Step 5: Build**

Run: `cargo build -p vox-cli --features ars 2>&1 | tail -3`
Expected: `Finished` (no errors). If the connect type mismatch from Step 3's note appears, fix per the grep and rebuild.

- [ ] **Step 6: Smoke-test**

Run: `cargo run -p vox-cli --features ars -- skill suggest --limit 100`
Expected: either a candidate list or "No operations captured yet." — exits 0 either way (no panic). (A fresh DB with no captured ops prints the empty message.)

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/Cargo.toml crates/vox-cli/src/commands/extras/skill_cmd.rs crates/vox-cli/src/commands/extras/ars/skill_suggest.rs crates/vox-cli/src/commands/extras/ars/mod.rs
git commit -m "feat(cli): vox skill suggest — mine operation sequences into candidates"
```

---

## Final verification

- [ ] **Step 1: Unit + DB tests**

Run: `cargo test -p vox-skill-discovery op_miner` then `cargo test -p vox-db --features local list_recent_operations_orders_and_limits`
Expected: all pass.

- [ ] **Step 2: CLI builds + smoke**

Run: `cargo build -p vox-cli --features ars` then `cargo run -p vox-cli --features ars -- skill suggest --limit 100`
Expected: builds; runs to exit 0.

- [ ] **Step 3: Format + clippy on touched crates**

Run: `cargo fmt -p vox-db -p vox-skill-discovery -p vox-cli` then `cargo clippy -p vox-skill-discovery -- -D warnings`
Expected: clean. (Clippy the pure crate `vox-skill-discovery`; the others transitively pull `vox-telemetry` whose pre-existing `collapsible_if` lint is unrelated — see prior sub-projects.)
