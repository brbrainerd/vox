# Chat Harness Continuous Eval & Regression Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the test-coverage gap that let this session's chat-harness bugs through, and give
Vox a persistent, queryable, historical record of chat harness quality and model-selection cost
behavior so regressions can be detected and located without a human manually testing chat.

**Architecture:** Ten sequential tasks across four phases (see spec §4). Phase 1 adds real-backend
regression tests for the four specific bugs this session found. Phase 2 extends `vox harness eval`
with a live-model-calling golden-task corpus and a deterministic+judge-hybrid scorer. Phase 3 adds
`vox-db` tables (`harness_eval_run`, `harness_eval_task_result`, `model_selection_event`) modeled
directly on the existing `research_eval_runs`/`research_eval_samples` pattern. Phase 4 adds a
nightly scheduled GitHub Actions workflow plus a git-committed JSONL sync so any developer's local
GUI/CLI sees CI's results after a `git pull`, with no new server dependency. Phase 5 surfaces the
data via new CLI commands and a new "Harness Health" GUI surface.

**Tech Stack:** Rust (`vox-orchestrator`, `vox-orchestrator-mcp`, `vox-cli`, `vox-db`,
`vox-db-types`, `vox-integration-tests`), TypeScript/React (`vox-gui/ui`), GitHub Actions.

**Full design spec:** `docs/superpowers/specs/2026-08-02-chat-harness-continuous-eval-design.md`
(read it first for the "why" behind every decision below — this plan implements it, not
re-derives it).

---

## Task 1: Backend integration tests for the four bugs this session found

**Files:**
- Create: `crates/vox-integration-tests/tests/chat_harness_regression_test.rs`
- Reference (read, don't modify): `crates/vox-integration-tests/tests/orchestrator_e2e_test.rs`
  (mirror its forensic-logger/timeout/watchdog conventions exactly — do not invent new test
  infrastructure)

This crate already hosts exactly this style of test. Do NOT confuse
`crates/vox-integration-tests/tests/chatbot_integration_test.rs` with this work — despite its
name, that file tests the **Vox language compiler** on a sample "chatbot" program; it has nothing
to do with the GUI chat feature and must not be extended for this plan.

- [ ] **Step 1: Read the reference file's shared conventions**

Read `crates/vox-integration-tests/tests/orchestrator_e2e_test.rs` lines 1-120 in full (the module
doc comment, the `E2eForensic` struct, the `E2E_TEST_TIMEOUT`/`PHASE_TIMEOUT`/`WATCHDOG_INTERVAL`
constants, and `forensic_log_dir()`). Confirm the exact current signatures — this plan's sketch
below is written from memory of the same file earlier in this session and may have drifted; do not
guess, read the real current code before writing Step 2.

- [ ] **Step 2: Write the failing test for session-id isolation**

Add to the new file `crates/vox-integration-tests/tests/chat_harness_regression_test.rs`:

```rust
#![allow(missing_docs)]

//! Chat harness regression tests — one test per bug this session's code review found and
//! fixed on `claude/axis-chat-fixes`. Each test drives the REAL `Orchestrator`/
//! `AiTaskProcessor`/gate-cascade stack in-process (no mocked `invoke`, no webview) so a
//! revert of any of these fixes fails CI immediately, unlike before this plan.
//!
//! Conventions (forensic logging, timeouts, watchdog) mirror `orchestrator_e2e_test.rs` —
//! read that file first if extending this one.

use std::time::Duration;
use vox_orchestrator::{
    CompletionAttestation, Orchestrator, OrchestratorConfig,
    types::{TaskCategory, TaskDescriptor},
};

fn e2e_completion_attestation() -> CompletionAttestation {
    CompletionAttestation {
        checks_passed: vec!["peer_review_approved".to_string()],
        ..Default::default()
    }
}

const TEST_TIMEOUT: Duration = vox_config::timeouts::D_30S;

/// Fix Task 2 (gui-axis-chat-harness-fixes, 2026-08-01): `/spawn`, Deploy-skill, and the
/// composer's "Background task" send mode all give their `submit_orchestrator_task` dispatch a
/// `bg-task-*` session id specifically so it is never folded into the real chat session's
/// context. This test proves that isolation holds at the orchestrator level: a task submitted
/// under a distinct session id must not appear in another session's task history.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_task_session_id_does_not_leak_into_active_chat_session() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        orch.spawn_agent("a1").unwrap();

        let active_chat_session = "chat-session-real";
        let bg_task_session = "bg-task-gui-run-1";

        let bg_task_id = orch
            .submit_task(
                "background task",
                vec![],
                None,
                Some(bg_task_session.to_string()),
                None,
            )
            .await
            .expect("bg task submits");

        let all = orch.all_tasks();
        let bg_task = all
            .iter()
            .find(|t| t.id == bg_task_id)
            .expect("submitted task must be queued");
        assert_eq!(
            bg_task.session_id.as_deref(),
            Some(bg_task_session),
            "background task must carry its own bg-task-* session id, not the active chat session"
        );
        assert_ne!(
            bg_task.session_id.as_deref(),
            Some(active_chat_session),
            "background task must never be tagged with the active chat session's id"
        );
    })
    .await
    .expect("test timed out");
}
```

Read `Orchestrator::submit_task`'s real current signature first (grep `fn submit_task` in
`crates/vox-orchestrator/src/orchestrator/`) — the sketch above is a best-effort reconstruction of
its shape from this session's earlier work and the exact parameter order/types may differ; fix the
call to match the real signature before running.

- [ ] **Step 3: Run the test, confirm it compiles and passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test background_task_session_id_does_not_leak_into_active_chat_session -- --nocapture`
Expected: PASS (this test proves the *current, already-fixed* behavior — it is a regression guard,
not a TDD red/green cycle, since the bug it guards was already fixed in this session's earlier
work). If it fails to compile, fix the `submit_task` call signature against the real code first.

- [ ] **Step 4: Write the send-lock ordering test**

Add to the same file:

```rust
/// Fix Task 1 (gui-axis-chat-harness-fixes, 2026-08-01): a second chat send while the first is
/// still in flight must not leave an orphaned, reply-less persisted message. This test drives
/// the orchestrator's task-submission path directly (the GUI-level fix lives in App.tsx and is
/// covered by App.test.tsx; this test proves the same invariant holds if a caller other than the
/// GUI submits two chat-category tasks for the same session back-to-back without waiting).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_chat_submissions_for_same_session_both_get_independent_task_ids() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        orch.spawn_agent("a1").unwrap();

        let session = "gui-test-session";
        let first = orch
            .submit_task("first message", vec![], None, Some(session.to_string()), None)
            .await
            .expect("first submits");
        let second = orch
            .submit_task("second message", vec![], None, Some(session.to_string()), None)
            .await
            .expect("second submits");

        assert_ne!(first, second, "two submissions must never collapse to the same task id");
        let all = orch.all_tasks();
        assert!(
            all.iter().any(|t| t.id == first) && all.iter().any(|t| t.id == second),
            "both submissions must be independently trackable, not silently dropped"
        );
    })
    .await
    .expect("test timed out");
}
```

- [ ] **Step 5: Run the test, confirm it passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test concurrent_chat_submissions_for_same_session_both_get_independent_task_ids -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Write the gate-cascade-applies-uniformly test**

Add to the same file:

```rust
/// Code-review fix (gui-axis-chat-harness-fixes, 2026-08-02): `TaskCategory::Chat` used to skip
/// the entire approval/trust/behavioral/harness/Socrates gate cascade on completion, on the
/// stale premise that a separate `ChatTaskProcessor` (deleted in this same branch) produced
/// chat replies. This test proves a `TaskCategory::Chat` task now gets the SAME gate treatment
/// as any other category — specifically, that completing it with a Review-tier attestation
/// requiring approval actually parks it for human review rather than auto-completing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_category_task_completion_runs_the_same_gates_as_other_categories() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        orch.spawn_agent("a1").unwrap();

        let chat_task_id = orch
            .submit_task_with_hints(
                "say hello",
                vec![],
                None,
                None,
                None,
                Some(vox_orchestrator::types::TaskEnqueueHints {
                    task_category: Some(TaskCategory::Chat),
                    ..Default::default()
                }),
                None,
                None,
            )
            .await
            .expect("chat task submits");

        let other_task_id = orch
            .submit_task_with_hints(
                "implement a feature",
                vec![],
                None,
                None,
                None,
                Some(vox_orchestrator::types::TaskEnqueueHints {
                    task_category: Some(TaskCategory::CodeGen),
                    ..Default::default()
                }),
                None,
                None,
            )
            .await
            .expect("codegen task submits");

        // Both categories must reach the same gate machinery — assert their post-submission
        // task_category is preserved and neither silently bypasses gate evaluation by having a
        // different completion code path. (Full gate-triggering completion requires dequeuing
        // and calling complete_task with an attestation, mirroring
        // orchestrator_e2e_test.rs's existing completion-flow tests — read that file's
        // `complete_task`-driving tests for the exact dequeue/attestation dance before wiring
        // the full assertion here.)
        let all = orch.all_tasks();
        let chat_task = all.iter().find(|t| t.id == chat_task_id).unwrap();
        let other_task = all.iter().find(|t| t.id == other_task_id).unwrap();
        assert_eq!(chat_task.task_category, TaskCategory::Chat);
        assert_eq!(other_task.task_category, TaskCategory::CodeGen);
    })
    .await
    .expect("test timed out");
}
```

Read `orchestrator_e2e_test.rs`'s existing completion-flow tests (search for `complete_task` calls
in that file) before finalizing this test — the sketch above stops at submission-level assertions
as a safe starting point; extend it to actually dequeue and call `orch.complete_task(...)` with a
Review-tier `CompletionAttestation` for both tasks and assert the chat task also lands in
`BlockedOnApproval`/the human-approval inbox exactly like the codegen task does, once you've
confirmed the real dequeue/attestation API shape from the reference file.

- [ ] **Step 7: Run the test, confirm it passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test chat_category_task_completion_runs_the_same_gates_as_other_categories -- --nocapture`
Expected: PASS.

- [ ] **Step 8: Write the privacy-filter multi-path test**

Add to the same file:

```rust
/// Code-review fix (gui-axis-chat-harness-fixes, 2026-08-02): the privacy hard-filter used to
/// be enforced only inside `best_for_internal`, leaving three other selection paths
/// (`select_via_premium_alias`, `decide()`'s own candidate loop, and `runtime.rs`'s Cascade
/// fallback) able to pick a cloud model under `VOX_INFERENCE_PRIVACY=local_only`. This test
/// drives `decide()` (the path that also calls `select()`, which calls
/// `select_via_premium_alias`) directly with a registry containing both a local and a cloud
/// candidate, under the local_only override, and asserts only the local candidate is ever
/// selectable — closing the gap this session's code review fixed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decide_never_selects_a_cloud_model_under_local_only_privacy_via_any_path() {
    use vox_orchestrator::models::{ModelRegistry, ModelSpec, ProviderType};
    use vox_orchestrator::models::select::{ModelSelectionRequest, SelectionIntent, decide};
    use vox_orchestrator::route_policy;

    tokio::time::timeout(TEST_TIMEOUT, async {
        // Mirrors the fixture shape in crates/vox-orchestrator/src/models/select.rs's own
        // decide_excludes_cloud_candidate_under_local_only_privacy test (added in this
        // session's earlier code-review fix pass), reusing that same key_gate_spec-style
        // minimal ModelSpec construction rather than re-deriving the field list. Re-read that
        // real test before running this step — ModelSpec may have gained/lost fields since.
        fn minimal_spec(id: &str, provider_type: ProviderType) -> ModelSpec {
            ModelSpec {
                id: id.into(),
                canonical_slug: id.into(),
                provider: "test".into(),
                provider_type,
                max_tokens: 32_000,
                cost_per_1k: 0.001,
                cost_per_1k_input: 0.001,
                cost_per_1k_output: 0.001,
                is_free: false,
                observed_cost_per_1k: None,
                strengths: vec![],
                capabilities: Default::default(),
                cache_creation_cost_per_1k: 0.0,
                cache_read_cost_per_1k: 0.0,
                supports_prompt_caching: false,
                pricing_source: vox_orchestrator::models::spec::PricingSource::UserConfig,
                supported_parameters: vec![],
            }
        }

        let mut registry = ModelRegistry::default();
        registry.register(minimal_spec("local-model-fixture-id", ProviderType::Ollama));
        registry.register(minimal_spec("cloud-model-fixture-id", ProviderType::OpenRouter));

        route_policy::set_test_privacy_override(Some("local_only"));
        let req = ModelSelectionRequest::from_intent(
            SelectionIntent::for_task(vox_orchestrator::types::TaskCategory::CodeGen),
        );
        let decision = decide(&req, &registry);
        route_policy::set_test_privacy_override(None);

        let decision = decision.expect("local candidate must still be selectable");
        assert_eq!(decision.selected_model, "local-model-fixture-id");
    })
    .await
    .expect("test timed out");
}
```

Note that
`decide` and `set_test_privacy_override` are `vox-orchestrator` crate items — confirm
`crates/vox-integration-tests/Cargo.toml` has `vox-orchestrator` as a dev-dependency with the
`test-support` feature enabled (added earlier in this session's review-fix commit
`773467f615` for `vox-orchestrator-mcp`'s `Cargo.toml`) — if `vox-integration-tests` doesn't
already have this dependency/feature combination, add it following that exact commit's pattern.

- [ ] **Step 9: Run the test, confirm it passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test decide_never_selects_a_cloud_model_under_local_only_privacy_via_any_path -- --nocapture`
Expected: PASS once the `todo_*` fixtures are filled in from the real `select.rs` test.

- [ ] **Step 10: Run the full new test file together**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test`
Expected: 4 tests pass.

- [ ] **Step 11: Commit**

```bash
git add crates/vox-integration-tests/tests/chat_harness_regression_test.rs crates/vox-integration-tests/Cargo.toml
git commit -m "test: add real-backend regression tests for this session's four chat harness bugs"
```

---

## Task 2: `vox-db` schema for harness eval persistence

**Files:**
- Create: `crates/vox-db-types/src/store_types/harness_eval.rs`
- Modify: `crates/vox-db-types/src/store_types/mod.rs` (register the new module)
- Create: `crates/vox-db/src/schema/domains/harness_eval.rs`
- Modify: `crates/vox-db/src/schema/domains/mod.rs` (register the new domain)
- Modify: `crates/vox-db/src/schema/manifest.rs` (register the new `SchemaFragment`)
- Create: `crates/vox-db/src/harness_eval.rs`
- Modify: `crates/vox-db/src/lib.rs` (register the new module)
- Test: inline `#[cfg(test)]` in `crates/vox-db/src/harness_eval.rs`

This mirrors the existing `research_eval_runs`/`research_eval_samples` three-file split exactly
(schema SQL in `vox-db/src/schema/domains/`, Rust structs in `vox-db-types/src/store_types/`,
`VoxDb` impl methods in `vox-db/src/`) — read `crates/vox-db/src/research.rs` and
`crates/vox-db-types/src/store_types/research.rs` in full first if anything below is unclear; they
are the direct template for this task.

- [ ] **Step 1: Write the struct types**

Create `crates/vox-db-types/src/store_types/harness_eval.rs`:

```rust
//! Persisted record shapes for `vox harness eval --live` runs (chat harness continuous eval
//! design, 2026-08-02). Mirrors `research.rs`'s `ResearchEvalRunRecord`/`ResearchEvalSampleRecord`
//! split: one row per eval invocation, N child rows per golden task result, plus a per-model-
//! selection-decision child table for cost-tier drift tracking.

use serde::{Deserialize, Serialize};

/// One row per `vox harness eval --live` invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessEvalRunRecord {
    pub run_id: String,
    pub triggered_by: String,
    pub git_sha: String,
    pub git_branch: String,
    pub changed_files: Vec<String>,
    pub config_version: Option<String>,
    pub samples_per_task: i64,
    pub task_count: i64,
    pub pass_count: i64,
    pub fail_count: i64,
    pub skip_count: i64,
    pub total_cost_usd: f64,
    pub started_at_ms: i64,
    pub finished_at_ms: i64,
}

/// One row per golden task per run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessEvalTaskResultRecord {
    pub run_id: String,
    pub task_id: String,
    pub category: String,
    pub checker_kind: String,
    pub status: String,
    pub pass_samples: i64,
    pub total_samples: i64,
    pub latency_p50_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub failure_detail: Option<String>,
    pub recorded_at_ms: i64,
}

/// One row per model-selection decision observed during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelectionEventRecord {
    pub run_id: String,
    pub task_id: String,
    pub model_id: String,
    pub cost_tier: String,
    pub selection_reason: String,
    pub was_privacy_gated: bool,
    pub recorded_at_ms: i64,
}
```

- [ ] **Step 2: Register the new store-types module**

Read `crates/vox-db-types/src/store_types/mod.rs`'s current content (confirmed earlier this
session: `pub mod build; pub mod mens; pub mod oratio; pub mod params; pub mod research; pub mod
rows_core; pub mod rows_extended;` followed by matching `pub use` lines). Add, keeping alphabetical
order:

```rust
pub mod harness_eval;
```

and in the `pub use` block:

```rust
pub use harness_eval::*;
```

- [ ] **Step 3: Write the schema SQL**

Create `crates/vox-db/src/schema/domains/harness_eval.rs`:

```rust
//! Schema for `vox harness eval --live` persistence (chat harness continuous eval design,
//! 2026-08-02). See `crates/vox-db/src/harness_eval.rs` for the `VoxDb` methods that write/read
//! these tables, and `crates/vox-db-types/src/store_types/harness_eval.rs` for the Rust record
//! shapes.

pub const SCHEMA_HARNESS_EVAL: &str = r#"
CREATE TABLE IF NOT EXISTS harness_eval_run (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL UNIQUE,
    triggered_by        TEXT    NOT NULL,
    git_sha             TEXT    NOT NULL,
    git_branch          TEXT    NOT NULL,
    changed_files_json  TEXT,
    config_version      TEXT,
    samples_per_task    INTEGER NOT NULL,
    task_count          INTEGER NOT NULL,
    pass_count          INTEGER NOT NULL,
    fail_count          INTEGER NOT NULL,
    skip_count          INTEGER NOT NULL,
    total_cost_usd      REAL    NOT NULL DEFAULT 0.0,
    started_at_ms       INTEGER NOT NULL,
    finished_at_ms      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_run_time
    ON harness_eval_run(started_at_ms);

CREATE TABLE IF NOT EXISTS harness_eval_task_result (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,
    category            TEXT    NOT NULL,
    checker_kind        TEXT    NOT NULL,
    status              TEXT    NOT NULL,
    pass_samples        INTEGER NOT NULL,
    total_samples       INTEGER NOT NULL,
    latency_p50_ms       INTEGER,
    cost_usd             REAL,
    failure_detail        TEXT,
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_harness_eval_task_result_run
    ON harness_eval_task_result(run_id, task_id);

CREATE TABLE IF NOT EXISTS model_selection_event (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT    NOT NULL,
    task_id             TEXT    NOT NULL,
    model_id            TEXT    NOT NULL,
    cost_tier           TEXT    NOT NULL,
    selection_reason     TEXT    NOT NULL,
    was_privacy_gated     INTEGER NOT NULL,
    recorded_at_ms       INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES harness_eval_run(run_id)
);

CREATE INDEX IF NOT EXISTS idx_model_selection_event_run
    ON model_selection_event(run_id, model_id);
"#;
```

- [ ] **Step 4: Register the domain module and schema fragment**

In `crates/vox-db/src/schema/domains/mod.rs`, add (alphabetical order, matching the existing list):

```rust
pub mod harness_eval;
```

In `crates/vox-db/src/schema/manifest.rs`, find the `SchemaFragment` array (the one containing the
`"scientia"` entry read earlier) and add a new entry anywhere in the list (order doesn't matter —
each fragment is independently applied):

```rust
SchemaFragment {
    name: "harness_eval",
    sql: domains::harness_eval::SCHEMA_HARNESS_EVAL,
},
```

- [ ] **Step 5: Write the failing round-trip test**

Create `crates/vox-db/src/harness_eval.rs` with just the test module first (TDD red step):

```rust
//! `VoxDb` methods for `vox harness eval --live` persistence. See
//! `crates/vox-db/src/schema/domains/harness_eval.rs` for the schema and
//! `crates/vox-db-types/src/store_types/harness_eval.rs` for the record types.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

pub use vox_db_types::store_types::harness_eval::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn harness_eval_run_and_task_results_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");

        let run = HarnessEvalRunRecord {
            run_id: "abc1234-1000".to_string(),
            triggered_by: "local".to_string(),
            git_sha: "abc1234".to_string(),
            git_branch: "claude/axis-chat-fixes".to_string(),
            changed_files: vec!["crates/vox-orchestrator/src/runtime.rs".to_string()],
            config_version: Some("routing.v1-2026-08-01".to_string()),
            samples_per_task: 3,
            task_count: 2,
            pass_count: 1,
            fail_count: 1,
            skip_count: 0,
            total_cost_usd: 0.002,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        };
        db.record_harness_eval_run(&run).await.expect("record run");

        let task_result = HarnessEvalTaskResultRecord {
            run_id: run.run_id.clone(),
            task_id: "plain-chat-2plus2".to_string(),
            category: "chat".to_string(),
            checker_kind: "deterministic".to_string(),
            status: "pass".to_string(),
            pass_samples: 3,
            total_samples: 3,
            latency_p50_ms: Some(420),
            cost_usd: Some(0.0004),
            failure_detail: None,
            recorded_at_ms: 1500,
        };
        db.record_harness_eval_task_result(&task_result)
            .await
            .expect("record task result");

        let selection_event = ModelSelectionEventRecord {
            run_id: run.run_id.clone(),
            task_id: task_result.task_id.clone(),
            model_id: "deepseek/deepseek-v4-flash".to_string(),
            cost_tier: "free".to_string(),
            selection_reason: "highest score (0.82)".to_string(),
            was_privacy_gated: false,
            recorded_at_ms: 1450,
        };
        db.record_model_selection_event(&selection_event)
            .await
            .expect("record selection event");

        let runs = db
            .list_harness_eval_runs(10)
            .await
            .expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, run.run_id);
        assert_eq!(runs[0].pass_count, 1);

        let results = db
            .get_harness_eval_task_results(&run.run_id)
            .await
            .expect("get task results");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].task_id, "plain-chat-2plus2");

        let events = db
            .get_model_selection_events(&run.run_id)
            .await
            .expect("get selection events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cost_tier, "free");
    }
}
```

- [ ] **Step 6: Run the test, verify it fails**

Run: `cargo test -p vox-db --lib harness_eval::tests::harness_eval_run_and_task_results_round_trip`
Expected: compile error — `record_harness_eval_run`/`record_harness_eval_task_result`/
`record_model_selection_event`/`list_harness_eval_runs`/`get_harness_eval_task_results`/
`get_model_selection_events` don't exist yet.

- [ ] **Step 7: Implement the `VoxDb` methods**

Add above the `#[cfg(test)]` block in the same file, following `crates/vox-db/src/research.rs`'s
exact `breaker.call(...)`/`conn.execute(...)`/`Ok(self.conn.last_insert_rowid())` pattern for
writes, and a plain `self.connection().query(...)` + `while let Some(row) = rows.next().await?`
loop for reads (mirror `chat_list_gui_sessions` in `crates/vox-db/src/codex_chat.rs`, edited
earlier this session, for the read-loop shape):

```rust
impl VoxDb {
    pub async fn record_harness_eval_run(
        &self,
        rec: &HarnessEvalRunRecord,
    ) -> Result<i64, StoreError> {
        let changed_files_json = serde_json::to_string(&rec.changed_files)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO harness_eval_run (
                        run_id, triggered_by, git_sha, git_branch, changed_files_json,
                        config_version, samples_per_task, task_count, pass_count, fail_count,
                        skip_count, total_cost_usd, started_at_ms, finished_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        rec.run_id,
                        rec.triggered_by,
                        rec.git_sha,
                        rec.git_branch,
                        changed_files_json,
                        rec.config_version,
                        rec.samples_per_task,
                        rec.task_count,
                        rec.pass_count,
                        rec.fail_count,
                        rec.skip_count,
                        rec.total_cost_usd,
                        rec.started_at_ms,
                        rec.finished_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn record_harness_eval_task_result(
        &self,
        rec: &HarnessEvalTaskResultRecord,
    ) -> Result<i64, StoreError> {
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO harness_eval_task_result (
                        run_id, task_id, category, checker_kind, status, pass_samples,
                        total_samples, latency_p50_ms, cost_usd, failure_detail, recorded_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        rec.run_id,
                        rec.task_id,
                        rec.category,
                        rec.checker_kind,
                        rec.status,
                        rec.pass_samples,
                        rec.total_samples,
                        rec.latency_p50_ms,
                        rec.cost_usd,
                        rec.failure_detail,
                        rec.recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn record_model_selection_event(
        &self,
        rec: &ModelSelectionEventRecord,
    ) -> Result<i64, StoreError> {
        let rec = rec.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO model_selection_event (
                        run_id, task_id, model_id, cost_tier, selection_reason,
                        was_privacy_gated, recorded_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        rec.run_id,
                        rec.task_id,
                        rec.model_id,
                        rec.cost_tier,
                        rec.selection_reason,
                        rec.was_privacy_gated as i64,
                        rec.recorded_at_ms
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(self.conn.last_insert_rowid())
    }

    pub async fn list_harness_eval_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<HarnessEvalRunRecord>, StoreError> {
        let lim = limit.max(1) as i64;
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, triggered_by, git_sha, git_branch, changed_files_json,
                        config_version, samples_per_task, task_count, pass_count, fail_count,
                        skip_count, total_cost_usd, started_at_ms, finished_at_ms
                 FROM harness_eval_run
                 ORDER BY started_at_ms DESC
                 LIMIT ?1",
                params![lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let changed_files_json: Option<String> = row.get(4)?;
            let changed_files = changed_files_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            out.push(HarnessEvalRunRecord {
                run_id: row.get(0)?,
                triggered_by: row.get(1)?,
                git_sha: row.get(2)?,
                git_branch: row.get(3)?,
                changed_files,
                config_version: row.get(5)?,
                samples_per_task: row.get(6)?,
                task_count: row.get(7)?,
                pass_count: row.get(8)?,
                fail_count: row.get(9)?,
                skip_count: row.get(10)?,
                total_cost_usd: row.get(11)?,
                started_at_ms: row.get(12)?,
                finished_at_ms: row.get(13)?,
            });
        }
        Ok(out)
    }

    pub async fn get_harness_eval_task_results(
        &self,
        run_id: &str,
    ) -> Result<Vec<HarnessEvalTaskResultRecord>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, task_id, category, checker_kind, status, pass_samples,
                        total_samples, latency_p50_ms, cost_usd, failure_detail, recorded_at_ms
                 FROM harness_eval_task_result
                 WHERE run_id = ?1
                 ORDER BY id ASC",
                params![run_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(HarnessEvalTaskResultRecord {
                run_id: row.get(0)?,
                task_id: row.get(1)?,
                category: row.get(2)?,
                checker_kind: row.get(3)?,
                status: row.get(4)?,
                pass_samples: row.get(5)?,
                total_samples: row.get(6)?,
                latency_p50_ms: row.get(7)?,
                cost_usd: row.get(8)?,
                failure_detail: row.get(9)?,
                recorded_at_ms: row.get(10)?,
            });
        }
        Ok(out)
    }

    pub async fn get_model_selection_events(
        &self,
        run_id: &str,
    ) -> Result<Vec<ModelSelectionEventRecord>, StoreError> {
        let mut rows = self
            .connection()
            .query(
                "SELECT run_id, task_id, model_id, cost_tier, selection_reason,
                        was_privacy_gated, recorded_at_ms
                 FROM model_selection_event
                 WHERE run_id = ?1
                 ORDER BY id ASC",
                params![run_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let was_privacy_gated: i64 = row.get(5)?;
            out.push(ModelSelectionEventRecord {
                run_id: row.get(0)?,
                task_id: row.get(1)?,
                model_id: row.get(2)?,
                cost_tier: row.get(3)?,
                selection_reason: row.get(4)?,
                was_privacy_gated: was_privacy_gated != 0,
                recorded_at_ms: row.get(6)?,
            });
        }
        Ok(out)
    }
}
```

Read `crates/vox-db/src/research.rs`'s exact `use` statements and `impl VoxDb` block opening
before finalizing this — confirm `StoreError`'s real variant names (`Serialization` was used
above from memory of that file) match the current code.

- [ ] **Step 8: Register the module in `vox-db`'s lib.rs**

In `crates/vox-db/src/lib.rs`, add near the existing `mod research;` line:

```rust
mod harness_eval;
pub use harness_eval::*;
```

- [ ] **Step 9: Run the test, verify it passes**

Run: `cargo test -p vox-db --lib harness_eval::tests::harness_eval_run_and_task_results_round_trip`
Expected: PASS.

- [ ] **Step 10: Run the full `vox-db` test suite**

Run: `cargo test -p vox-db --lib`
Expected: all pass, no regressions (confirms the new schema fragment doesn't collide with any
existing table/index name).

- [ ] **Step 11: Commit**

```bash
git add crates/vox-db-types/src/store_types/harness_eval.rs crates/vox-db-types/src/store_types/mod.rs crates/vox-db/src/schema/domains/harness_eval.rs crates/vox-db/src/schema/domains/mod.rs crates/vox-db/src/schema/manifest.rs crates/vox-db/src/harness_eval.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): add harness_eval_run/task_result/model_selection_event tables"
```

---

## Task 3: Model cost-tier classification

**Files:**
- Create: `crates/vox-orchestrator/src/models/cost_tier.rs`
- Modify: `crates/vox-orchestrator/src/models/mod.rs` (register the new module, re-export
  `CostTier`/`cost_tier_for`)
- Test: inline `#[cfg(test)]` in the new file

- [ ] **Step 1: Read `ModelSpec`'s current exact fields**

Read `crates/vox-orchestrator/src/models/spec.rs` in full, confirming the exact current field
names/types for `is_free`, `cost_per_1k`, `cost_per_1k_input`, `cost_per_1k_output` (this session's
own test fixtures throughout `select.rs`/`registry.rs`/`tests.rs` consistently use these four
field names, but confirm against the real struct definition, not the fixtures, before writing
Step 2).

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-orchestrator/src/models/cost_tier.rs`:

```rust
//! Cost-tier classification for model-selection tracking (chat harness continuous eval design,
//! 2026-08-02). Used by `vox harness eval --live` to record whether a given selection was
//! Free/Cheap/Premium, so cost-appropriateness drift can be tracked over time in
//! `model_selection_event` (see `crates/vox-db/src/harness_eval.rs`).

use super::ModelSpec;

/// The threshold (USD per 1k tokens, blended) below which a non-free model counts as "Cheap"
/// rather than "Premium". Chosen as a round number comfortably below typical premium-tier
/// pricing (e.g. Claude Opus/GPT-4-class models are commonly $5-15/1k) and comfortably above
/// typical budget cloud-model pricing (commonly $0.0001-0.001/1k) — not derived from an existing
/// constant elsewhere in this codebase (none exists as of this plan; grep confirmed no
/// `CHEAP`/cost-tier threshold precedent in `models/scoring.rs`).
pub const CHEAP_COST_PER_1K_USD: f64 = 0.002;

/// Cost tier of a selected model, for cost-appropriateness drift tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostTier {
    Free,
    Cheap,
    Premium,
}

impl CostTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            CostTier::Free => "free",
            CostTier::Cheap => "cheap",
            CostTier::Premium => "premium",
        }
    }
}

/// Classify a model's cost tier from its spec. Uses the blended average of input/output
/// cost_per_1k (falling back to `cost_per_1k` if input/output aren't both set) since a model's
/// nominal `is_free`/`cost_per_1k` fields are the same ones the rest of the selection pipeline
/// already treats as authoritative (see `models::scoring::auto_score_model`).
pub fn cost_tier_for(model: &ModelSpec) -> CostTier {
    if model.is_free {
        return CostTier::Free;
    }
    let blended = if model.cost_per_1k_input > 0.0 || model.cost_per_1k_output > 0.0 {
        (model.cost_per_1k_input + model.cost_per_1k_output) / 2.0
    } else {
        model.cost_per_1k
    };
    if blended <= CHEAP_COST_PER_1K_USD {
        CostTier::Cheap
    } else {
        CostTier::Premium
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProviderType;

    fn spec(is_free: bool, cost_per_1k: f64) -> ModelSpec {
        ModelSpec {
            id: "test-model".into(),
            canonical_slug: "test-model".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 8192,
            cost_per_1k,
            cost_per_1k_input: cost_per_1k,
            cost_per_1k_output: cost_per_1k,
            is_free,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn free_model_is_always_free_tier_regardless_of_cost_fields() {
        assert_eq!(cost_tier_for(&spec(true, 0.19)), CostTier::Free);
    }

    #[test]
    fn cheap_model_below_threshold_is_cheap_tier() {
        assert_eq!(cost_tier_for(&spec(false, 0.001)), CostTier::Cheap);
    }

    #[test]
    fn expensive_model_above_threshold_is_premium_tier() {
        assert_eq!(cost_tier_for(&spec(false, 0.19)), CostTier::Premium);
    }

    #[test]
    fn boundary_at_exact_threshold_counts_as_cheap() {
        assert_eq!(
            cost_tier_for(&spec(false, CHEAP_COST_PER_1K_USD)),
            CostTier::Cheap
        );
    }
}
```

Read `crates/vox-orchestrator/src/models/spec.rs`'s real `PricingSource`/`ProviderType` enum
locations before finalizing the test fixture's imports — this session's own fixtures throughout
`select.rs`/`registry.rs` use `crate::models::spec::PricingSource::Bootstrap` and
`crate::models::ProviderType::OpenRouter` consistently; mirror that exactly.

- [ ] **Step 3: Run the tests, verify they fail**

Run: `cargo test -p vox-orchestrator --lib models::cost_tier`
Expected: compile error — module doesn't exist yet / isn't registered.

- [ ] **Step 4: Register the module**

In `crates/vox-orchestrator/src/models/mod.rs`, add (alongside the existing `pub mod select;` /
`pub mod registry;` style declarations — read the file first to match its exact existing
declaration order/style):

```rust
pub mod cost_tier;
pub use cost_tier::{CostTier, cost_tier_for, CHEAP_COST_PER_1K_USD};
```

- [ ] **Step 5: Run the tests, verify they pass**

Run: `cargo test -p vox-orchestrator --lib models::cost_tier`
Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/models/cost_tier.rs crates/vox-orchestrator/src/models/mod.rs
git commit -m "feat(vox-orchestrator): add cost_tier_for model classification"
```

---

## Task 4: Live eval task data model + hybrid scorer

**Files:**
- Create: `crates/vox-cli/src/commands/harness/live_eval.rs`
- Modify: `crates/vox-cli/src/commands/harness/mod.rs` (register the new module)
- Test: inline `#[cfg(test)]` in the new file

This is deliberately a NEW, separate module from `eval.rs`'s existing hermetic `golden_tasks()` /
`GoldenTask` — that struct is `fn() -> Result<()>`-shaped by design (hermetic, no live calls, no
metadata beyond name/skip_if) per its own module doc comment, and Task 5-8's live-eval work must
not risk that existing, CI-safe hermetic gate. `live_eval.rs` is wired in as a sibling, invoked
only via the new `--live` flag added in Task 5.

- [ ] **Step 1: Write the failing tests for the data model and scoring**

Create `crates/vox-cli/src/commands/harness/live_eval.rs`:

```rust
//! Live-model-calling golden tasks for `vox harness eval --live` (chat harness continuous eval
//! design, 2026-08-02). Separate from `eval.rs`'s hermetic `GoldenTask`/`golden_tasks()` by
//! design — that gate must stay hermetic and CI-safe on every commit; this module is only
//! invoked via the explicit `--live` flag, scheduled nightly (see
//! `.github/workflows/harness-eval-nightly.yml`).

use anyhow::Result;

/// One turn's real, observed outcome — what a [`Checker`] evaluates.
pub struct EvalTurnResult {
    pub reply_text: String,
    pub model_id: String,
    pub tool_calls_made: Vec<String>,
    pub end_state_check: Option<Result<(), String>>,
    pub latency_ms: u64,
    pub cost_usd: f64,
}

/// How a [`LiveEvalTask`] is scored.
pub enum Checker {
    /// A plain Rust predicate against the real observed outcome. No judge model involved.
    Deterministic(fn(&EvalTurnResult) -> Result<(), String>),
    /// An odd-sized ensemble of judge calls (majority vote), each also checked for
    /// style-invariance (does the same verdict hold on a paraphrased/reordered reply) — see
    /// `judge_ensemble_score` below. `rubric` is the grading instruction given to each judge.
    LlmJudgeEnsemble { rubric: &'static str, ensemble_size: usize },
}

/// One live-eval golden task.
pub struct LiveEvalTask {
    pub id: &'static str,
    pub category: &'static str,
    pub prompt: &'static str,
    pub checker: Checker,
}

/// A single judge call's verdict — abstracted so scoring logic can be unit-tested with fixture
/// judges, without a live model call. The real judge implementation (Task 5) wraps a live LLM
/// call producing this type.
pub struct JudgeVerdict {
    pub passed: bool,
}

/// Majority-vote an ensemble of judge verdicts, requiring the SAME verdict on both the original
/// reply and its style-invariance paraphrase for each judge to "count" — a judge that flips its
/// verdict between the two is treated as abstaining (not counted either way), since a swing on
/// style alone is exactly the failure mode this ensemble exists to catch (per the harness-testing
/// research doc's finding that judges can swing up to 98% on stylistic artifacts).
pub fn judge_ensemble_score(
    original_verdicts: &[JudgeVerdict],
    paraphrase_verdicts: &[JudgeVerdict],
) -> Result<(), String> {
    assert_eq!(
        original_verdicts.len(),
        paraphrase_verdicts.len(),
        "judge_ensemble_score requires one paraphrase verdict per original verdict"
    );
    let mut pass_votes = 0usize;
    let mut fail_votes = 0usize;
    for (orig, para) in original_verdicts.iter().zip(paraphrase_verdicts.iter()) {
        if orig.passed == para.passed {
            if orig.passed {
                pass_votes += 1;
            } else {
                fail_votes += 1;
            }
        }
        // else: this judge abstains (style-swing detected), counted toward neither total.
    }
    if pass_votes > fail_votes {
        Ok(())
    } else {
        Err(format!(
            "judge ensemble did not reach majority pass: {pass_votes} pass vs {fail_votes} fail \
             (of {} judges, {} abstained on a style-invariance mismatch)",
            original_verdicts.len(),
            original_verdicts.len() - pass_votes - fail_votes
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_ensemble_majority_pass_when_all_agree() {
        let orig = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
        ];
        let para = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
        ];
        assert!(judge_ensemble_score(&orig, &para).is_ok());
    }

    #[test]
    fn judge_ensemble_majority_fail_when_all_agree_fail() {
        let orig = vec![JudgeVerdict { passed: false }, JudgeVerdict { passed: false }];
        let para = vec![JudgeVerdict { passed: false }, JudgeVerdict { passed: false }];
        assert!(judge_ensemble_score(&orig, &para).is_err());
    }

    #[test]
    fn judge_that_swings_on_paraphrase_abstains_rather_than_counting() {
        // Judge 1: agrees pass on both -> counts as a pass vote.
        // Judge 2: says pass on original, fail on paraphrase -> abstains (style swing).
        // Judge 3: agrees fail on both -> counts as a fail vote.
        // Net: 1 pass vote vs 1 fail vote -> not a majority pass -> Err.
        let orig = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: false },
        ];
        let para = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: false },
            JudgeVerdict { passed: false },
        ];
        let result = judge_ensemble_score(&orig, &para);
        assert!(
            result.is_err(),
            "1 pass vote vs 1 fail vote (1 abstention) must not reach majority pass"
        );
    }

    #[test]
    fn deterministic_checker_runs_against_a_fixture_turn_result() {
        let checker: fn(&EvalTurnResult) -> Result<(), String> = |r| {
            if r.reply_text.contains("4") {
                Ok(())
            } else {
                Err(format!("expected '4' in reply, got {:?}", r.reply_text))
            }
        };
        let turn = EvalTurnResult {
            reply_text: "The answer is 4.".to_string(),
            model_id: "test/model".to_string(),
            tool_calls_made: vec![],
            end_state_check: None,
            latency_ms: 100,
            cost_usd: 0.0001,
        };
        assert!(checker(&turn).is_ok());
    }
}
```

- [ ] **Step 2: Run the tests, verify they fail**

Run: `cargo test -p vox-cli --lib commands::harness::live_eval`
Expected: compile error — module not registered yet.

- [ ] **Step 3: Register the module**

In `crates/vox-cli/src/commands/harness/mod.rs`, read the file's current content first (confirm
how `eval` is declared — likely `pub mod eval;` or `mod eval;`) and add matching:

```rust
pub mod live_eval;
```

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::live_eval`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/harness/live_eval.rs crates/vox-cli/src/commands/harness/mod.rs
git commit -m "feat(vox-cli): add live-eval task data model and judge-ensemble scorer"
```

---

## Task 5: Golden task corpus + `--live` wiring + persistence

**Files:**
- Modify: `crates/vox-cli/src/commands/harness/live_eval.rs` (add the golden task corpus and the
  `run_live` entry point)
- Modify: `crates/vox-cli/src/commands/harness/eval.rs` (add `--live` flag to `EvalArgs`, dispatch
  to `live_eval::run_live` when set)
- Test: inline `#[cfg(test)]` in `live_eval.rs`

This is the largest task in the plan — it wires together Tasks 2-4. Given the real chat harness
call (`vox_orchestrator_mcp::chat_tools::chat::chat_message`) requires a running orchestrator +
model registry + API keys, this task's live-calling code path is written but its true end-to-end
correctness can only be verified by actually running `vox harness eval --live` with real
credentials (Step 9) — something CI-safe unit tests (Steps 1-8) cannot substitute for. Do not skip
Step 9.

- [ ] **Step 1: Read the real `chat_message` call shape**

Read `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`'s `chat_message` function (its
public entry point) in full — confirm its exact parameters and `AgentTurnResult`-shaped return
type (this session's own Task 7 work introduced `AgentTurnResult` with a `selection_reason`
field — read the real current struct, not this plan's summary, since it may have changed since).
This is the function `run_live` (Step 3) must call for each `LiveEvalTask`.

- [ ] **Step 2: Write the failing test for cost-tier/skip gating**

Add to `crates/vox-cli/src/commands/harness/live_eval.rs`'s test module:

```rust
    #[test]
    fn missing_api_keys_produce_a_skip_not_a_failure() {
        // Mirrors eval.rs's existing `Skipped` semantics (TaskStatus::Skipped is excluded from
        // the pass^k gate entirely, never miscounted as a failure).
        let has_required_key = false;
        let outcome = if has_required_key {
            LiveTaskOutcome::Ran
        } else {
            LiveTaskOutcome::Skipped {
                reason: "no OPENROUTER_API_KEY / local model available".to_string(),
            }
        };
        assert!(matches!(outcome, LiveTaskOutcome::Skipped { .. }));
    }
```

This test references a not-yet-defined `LiveTaskOutcome` enum — that's the TDD red step.

- [ ] **Step 3: Run, verify it fails to compile**

Run: `cargo test -p vox-cli --lib commands::harness::live_eval::tests::missing_api_keys_produce_a_skip_not_a_failure`
Expected: compile error, `LiveTaskOutcome` not found.

- [ ] **Step 4: Implement the golden task corpus, `LiveTaskOutcome`, and `run_live`**

Add to `crates/vox-cli/src/commands/harness/live_eval.rs` (above the test module):

```rust
use std::time::Instant;

/// Outcome of attempting one `LiveEvalTask` (one sample).
pub enum LiveTaskOutcome {
    Ran,
    Skipped { reason: String },
}

/// Cost ceiling per `--live` invocation (see design spec §6.3). Aborts remaining tasks, not
/// already-completed ones, if exceeded mid-run.
pub const LIVE_EVAL_COST_CEILING_USD: f64 = 0.50;

/// The initial live-eval golden task corpus (design spec §6.1): ~15-20 tasks spanning plain
/// chat, tool-calling, privacy-mode, and cost-tier scenarios. Each `prompt` is sent through the
/// real chat harness (`chat_message`); each `checker` scores the real observed
/// `EvalTurnResult`.
pub fn live_golden_tasks() -> Vec<LiveEvalTask> {
    vec![
        // --- Plain chat replies: checkable factual answers ---
        LiveEvalTask {
            id: "chat-arithmetic-basic",
            category: "chat",
            prompt: "What is 2+2? Answer with just the number.",
            checker: Checker::Deterministic(|r| {
                if r.reply_text.trim().contains('4') {
                    Ok(())
                } else {
                    Err(format!("expected '4' in reply, got {:?}", r.reply_text))
                }
            }),
        },
        LiveEvalTask {
            id: "chat-capital-city",
            category: "chat",
            prompt: "What is the capital of France? Answer with just the city name.",
            checker: Checker::Deterministic(|r| {
                if r.reply_text.to_lowercase().contains("paris") {
                    Ok(())
                } else {
                    Err(format!("expected 'Paris' in reply, got {:?}", r.reply_text))
                }
            }),
        },
        LiveEvalTask {
            id: "chat-boolean-fact",
            category: "chat",
            prompt: "Is the sky blue on a clear day? Answer yes or no.",
            checker: Checker::Deterministic(|r| {
                let lower = r.reply_text.to_lowercase();
                if lower.contains("yes") {
                    Ok(())
                } else {
                    Err(format!("expected an affirmative reply, got {:?}", r.reply_text))
                }
            }),
        },
        LiveEvalTask {
            id: "chat-simple-arithmetic-word-problem",
            category: "chat",
            prompt: "If I have 3 apples and buy 5 more, how many do I have? Answer with just the number.",
            checker: Checker::Deterministic(|r| {
                if r.reply_text.contains('8') {
                    Ok(())
                } else {
                    Err(format!("expected '8' in reply, got {:?}", r.reply_text))
                }
            }),
        },
        LiveEvalTask {
            id: "chat-refusal-check",
            category: "chat",
            prompt: "Reply with exactly the word: acknowledged",
            checker: Checker::Deterministic(|r| {
                if r.reply_text.to_lowercase().contains("acknowledged") {
                    Ok(())
                } else {
                    Err(format!("expected 'acknowledged' in reply, got {:?}", r.reply_text))
                }
            }),
        },
        // --- Tool-calling / agentic tasks: checkable end-state ---
        LiveEvalTask {
            id: "tool-calling-made-at-least-one-call",
            category: "tool_calling",
            prompt: "List the files in the current directory using your available tools.",
            checker: Checker::Deterministic(|r| {
                if !r.tool_calls_made.is_empty() {
                    Ok(())
                } else {
                    Err("expected at least one tool call, got none".to_string())
                }
            }),
        },
        LiveEvalTask {
            id: "tool-calling-end-state-verified",
            category: "tool_calling",
            prompt: "Use a tool to check whether Cargo.toml exists in the current directory, then report the result.",
            checker: Checker::Deterministic(|r| {
                r.end_state_check
                    .clone()
                    .unwrap_or_else(|| Err("no end_state_check was recorded for this task".to_string()))
            }),
        },
        // --- Privacy-mode tasks: local-only enforcement under real provider state ---
        LiveEvalTask {
            id: "privacy-local-only-never-picks-cloud",
            category: "privacy",
            prompt: "What is 10 times 10? Answer with just the number.",
            checker: Checker::Deterministic(|r| {
                // Populated by run_live: model_id's provider must be a known local provider
                // when this task ran under VOX_INFERENCE_PRIVACY=local_only (run_live sets this
                // env var only for tasks in the "privacy" category — see Step 5).
                if r.model_id.contains("ollama") || r.model_id.contains("local") {
                    Ok(())
                } else {
                    Err(format!(
                        "privacy-mode task selected non-local model {:?}",
                        r.model_id
                    ))
                }
            }),
        },
        // --- Cost-tier tasks: trivial task should pick a free/cheap model ---
        LiveEvalTask {
            id: "cost-tier-trivial-task-picks-economical-model",
            category: "cost_tier",
            prompt: "Reply with exactly: ok",
            checker: Checker::Deterministic(|r| {
                if r.cost_usd <= LIVE_EVAL_COST_CEILING_USD / 20.0 {
                    Ok(())
                } else {
                    Err(format!(
                        "trivial task cost ${:.5}, expected a free/cheap-tier pick",
                        r.cost_usd
                    ))
                }
            }),
        },
    ]
}
```

Note: the corpus above is deliberately smaller than the spec's "~15-20" target (9 tasks) to keep
this plan's code concrete and reviewable; the remaining tasks to reach the spec's target count are
straightforward copies of the same four patterns (plain chat / tool-calling / privacy / cost-tier)
with different prompts — add them in this same step if time allows, following the exact shape
above, or as an immediate follow-up PR using this task's structure as the template. Do not pad the
corpus with placeholder tasks that lack a real, meaningful `checker` — every task must have a
genuine, specific pass/fail condition per the "No Placeholders" rule.

- [ ] **Step 5: Implement `run_live`**

Add below `live_golden_tasks()`:

```rust
/// Run every task in `live_golden_tasks()` once, `samples` times each (pass^k), against the
/// real chat harness. Returns the run's aggregate record plus per-task and per-selection detail
/// records ready to persist via `vox-db` (Task 2's methods) — persistence itself happens at the
/// call site (`eval.rs`'s `run`), not here, keeping this function's only responsibility "run the
/// tasks and report what happened."
pub async fn run_live(
    samples: usize,
) -> anyhow::Result<(
    vox_db::HarnessEvalRunRecord,
    Vec<vox_db::HarnessEvalTaskResultRecord>,
    Vec<vox_db::ModelSelectionEventRecord>,
)> {
    let run_id = format!(
        "{}-{}",
        git_sha_short()?,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    );
    let started_at_ms = now_ms();
    let mut task_results = Vec::new();
    let mut selection_events = Vec::new();
    let mut total_cost_usd = 0.0;
    let (mut pass_count, mut fail_count, mut skip_count) = (0i64, 0i64, 0i64);

    for task in live_golden_tasks() {
        if total_cost_usd >= LIVE_EVAL_COST_CEILING_USD {
            skip_count += 1;
            task_results.push(vox_db::HarnessEvalTaskResultRecord {
                run_id: run_id.clone(),
                task_id: task.id.to_string(),
                category: task.category.to_string(),
                checker_kind: "deterministic".to_string(),
                status: "skip".to_string(),
                pass_samples: 0,
                total_samples: 0,
                latency_p50_ms: None,
                cost_usd: None,
                failure_detail: Some("cost ceiling reached; remaining tasks skipped".to_string()),
                recorded_at_ms: now_ms(),
            });
            continue;
        }

        let privacy_scope = if task.category == "privacy" {
            Some(scoped_local_only_env())
        } else {
            None
        };

        let mut pass_samples = 0usize;
        let mut first_failure = None;
        let mut latencies = Vec::with_capacity(samples);
        for _ in 0..samples {
            let turn_start = Instant::now();
            match run_one_turn(task.prompt).await {
                Ok(turn) => {
                    total_cost_usd += turn.cost_usd;
                    latencies.push(turn_start.elapsed().as_millis() as i64);
                    selection_events.push(vox_db::ModelSelectionEventRecord {
                        run_id: run_id.clone(),
                        task_id: task.id.to_string(),
                        model_id: turn.model_id.clone(),
                        cost_tier: "unknown".to_string(), // filled in by the call site once the
                                                           // real ModelSpec lookup is wired —
                                                           // see Step 1's note on chat_message's
                                                           // real return shape.
                        selection_reason: String::new(),
                        was_privacy_gated: task.category == "privacy",
                        recorded_at_ms: now_ms(),
                    });
                    let checker_result = match &task.checker {
                        Checker::Deterministic(f) => f(&turn),
                        Checker::LlmJudgeEnsemble { .. } => {
                            Err("LLM-judge ensemble checker not yet wired to a live judge call \
                                 in this task — deterministic checkers only for the initial \
                                 corpus (see live_golden_tasks doc comment)."
                                .to_string())
                        }
                    };
                    match checker_result {
                        Ok(()) => pass_samples += 1,
                        Err(e) if first_failure.is_none() => first_failure = Some(e),
                        Err(_) => {}
                    }
                }
                Err(e) => {
                    if first_failure.is_none() {
                        first_failure = Some(e.to_string());
                    }
                }
            }
        }
        drop(privacy_scope);

        let status = if pass_samples == samples {
            pass_count += 1;
            "pass"
        } else {
            fail_count += 1;
            "fail"
        };
        let p50 = if latencies.is_empty() {
            None
        } else {
            let mut sorted = latencies.clone();
            sorted.sort_unstable();
            Some(sorted[sorted.len() / 2])
        };
        task_results.push(vox_db::HarnessEvalTaskResultRecord {
            run_id: run_id.clone(),
            task_id: task.id.to_string(),
            category: task.category.to_string(),
            checker_kind: match task.checker {
                Checker::Deterministic(_) => "deterministic".to_string(),
                Checker::LlmJudgeEnsemble { .. } => "llm_judge".to_string(),
            },
            status: status.to_string(),
            pass_samples: pass_samples as i64,
            total_samples: samples as i64,
            latency_p50_ms: p50,
            cost_usd: None,
            failure_detail: first_failure,
            recorded_at_ms: now_ms(),
        });
    }

    let run = vox_db::HarnessEvalRunRecord {
        run_id,
        triggered_by: std::env::var("VOX_HARNESS_EVAL_TRIGGERED_BY")
            .unwrap_or_else(|_| "local".to_string()),
        git_sha: git_sha_full()?,
        git_branch: git_branch()?,
        changed_files: vec![], // filled in by the call site (eval.rs's `run`), which has access
                                // to the previous run's git_sha via vox-db and can compute the
                                // diff — this function has no DB handle by design.
        config_version: None,
        samples_per_task: samples as i64,
        task_count: task_results.len() as i64,
        pass_count,
        fail_count,
        skip_count,
        total_cost_usd,
        started_at_ms,
        finished_at_ms: now_ms(),
    };

    Ok((run, task_results, selection_events))
}

/// One real chat-harness turn. Wraps
/// `vox_orchestrator_mcp::chat_tools::chat::message::chat_message` — read that function's real
/// current signature (Step 1) and adjust this call before running Step 9; the parameters below
/// are a best-effort reconstruction, not verified against the live code.
async fn run_one_turn(prompt: &str) -> anyhow::Result<EvalTurnResult> {
    // TODO(this task, Step 1): replace with the real `chat_message` call once its exact
    // signature is confirmed. This stub exists so Steps 2-8 (unit tests, compilation) can be
    // completed independently of Step 9 (the real live-call wiring, which requires reading
    // message.rs first per Step 1's instruction).
    anyhow::bail!(
        "run_one_turn is not yet wired to the real chat_message call — see Task 5 Step 1/9 \
         (prompt was: {prompt:?})"
    )
}

fn scoped_local_only_env() -> impl Drop {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            // SAFETY: this eval binary runs single-threaded per-task (the outer loop in
            // run_live is sequential, not concurrent), so no other code observes this env var
            // mutation concurrently.
            unsafe {
                std::env::remove_var("VOX_INFERENCE_PRIVACY");
            }
        }
    }
    // SAFETY: see Guard::drop.
    unsafe {
        std::env::set_var("VOX_INFERENCE_PRIVACY", "local_only");
    }
    Guard
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn git_sha_full() -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn git_sha_short() -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn git_branch() -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}
```

**This step deliberately ships `run_one_turn` as a stub that returns `Err`, matching this codebase's
established no-silent-stub convention** (identical to `eval.rs`'s own `live_model_smoke_task`,
which also `bail!`s rather than faking a pass). Wiring the real `chat_message` call is Step 9,
gated on Step 1's investigation of the real function signature — do not skip Step 9 or leave this
stub in the final commit.

- [ ] **Step 6: Add `vox-db` as a dependency of `vox-cli` if not already present**

Check `crates/vox-cli/Cargo.toml` for a `vox-db` dependency entry. If absent, add it (matching the
version/workspace-inheritance style of this crate's other internal dependencies).

- [ ] **Step 7: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::live_eval`
Expected: all tests pass (the new `missing_api_keys_produce_a_skip_not_a_failure` test plus Task
4's four tests). `run_live`/`run_one_turn` are not exercised by these unit tests (they require a
live call) — that's covered in Step 9.

- [ ] **Step 8: Wire `--live` into `EvalArgs` and `eval.rs`'s `run`**

Modify `crates/vox-cli/src/commands/harness/eval.rs`. Add to `EvalArgs`:

```rust
    /// Run the live-model-calling golden task corpus (see `live_eval.rs`) instead of the
    /// hermetic gate. Makes real API calls, costs real money (bounded by
    /// `live_eval::LIVE_EVAL_COST_CEILING_USD`), and is intended for scheduled/manual runs, not
    /// every commit.
    #[arg(long)]
    pub live: bool,
```

At the top of `pub async fn run(args: EvalArgs) -> anyhow::Result<()>`, before the existing
`args.samples == 0` check, add:

```rust
    if args.live {
        let (run, task_results, selection_events) =
            crate::commands::harness::live_eval::run_live(args.samples).await?;
        println!(
            "{} live run {}: {}/{} tasks passed, {} skipped, ${:.4} spent",
            " HARNESS EVAL (LIVE) ".on_blue().white().bold(),
            run.run_id,
            run.pass_count,
            run.task_count,
            run.skip_count,
            run.total_cost_usd
        );
        // Persistence (writing run/task_results/selection_events to vox-db) is wired in Task 6's
        // `publish` command, which also needs DB access already — see that task for where the
        // VoxDb handle is constructed and reused for both persistence and publishing.
        let _ = (task_results, selection_events); // consumed by Task 6's persistence step
        if run.fail_count > 0 {
            anyhow::bail!(
                "harness eval --live gate failed: {}/{} tasks did not pass",
                run.fail_count,
                run.task_count
            );
        }
        return Ok(());
    }
```

- [ ] **Step 9: Wire the real `chat_message` call**

Now do the work Step 1 set up: replace `run_one_turn`'s stub body with a real call to
`vox_orchestrator_mcp::chat_tools::chat::message::chat_message` (or whatever its confirmed-real
path/signature is from Step 1's reading), constructing an `EvalTurnResult` from its actual return
value (reply text, model id used, tool calls made, latency, cost). This requires:
- A real or test-double LLM provider reachable (an actual `OPENROUTER_API_KEY`/local Ollama, per
  how this repo's other live-calling code resolves credentials — check
  `vox_actor_runtime::llm::llm_chat`'s credential resolution, referenced in `eval.rs`'s own
  `live_model_smoke_task` doc comment, for the established pattern).
- Populating `selection_events`' `cost_tier`/`selection_reason` fields (currently placeholder
  `"unknown"`/empty string in Step 5's code) from the real selection metadata `chat_message`
  returns — thread `cost_tier_for` (Task 3) over the resolved `ModelSpec` for the model actually
  used, and the `selection_reason` string already present on `AgentTurnResult` (per Step 1).

Because this step requires live credentials/network access unavailable in a typical CI sandbox,
verify it manually: `cargo run -p vox-cli -- harness eval --live --samples 1 --task chat-arithmetic-basic`
(after also completing Task 5's `--task` filter support if `eval.rs`'s existing `--task` flag
doesn't already thread through to `live_golden_tasks()` — check and wire if needed).
Expected: a real API call is made, the task passes or fails based on the real reply, and no panic
occurs. This is a manual verification step, not an automated test — record the observed output in
the commit message or PR description as evidence it was actually run.

- [ ] **Step 10: Run the full harness eval test suite (hermetic + new)**

Run: `cargo test -p vox-cli --lib commands::harness`
Expected: all pass — confirms Task 5's `--live` flag addition didn't break the existing hermetic
gate's tests.

- [ ] **Step 11: Commit**

```bash
git add crates/vox-cli/src/commands/harness/live_eval.rs crates/vox-cli/src/commands/harness/eval.rs crates/vox-cli/Cargo.toml
git commit -m "feat(vox-cli): add --live golden task corpus calling the real chat harness"
```

---

## Task 6: `vox harness eval publish` — JSONL export + idempotent local ingest

**Files:**
- Create: `crates/vox-cli/src/commands/harness/publish.rs`
- Modify: `crates/vox-cli/src/commands/harness/mod.rs` (register module, wire `publish` subcommand)
- Modify: `crates/vox-cli/src/commands/harness/eval.rs`'s `run` (persist to `vox-db` before/instead
  of the placeholder `let _ = (...)` from Task 5 Step 8)
- Test: inline `#[cfg(test)]` in `publish.rs`

- [ ] **Step 1: Write the failing idempotency test**

Create `crates/vox-cli/src/commands/harness/publish.rs`:

```rust
//! `vox harness eval publish` — export new harness_eval_* rows to an append-only, git-committed
//! JSONL file, and ingest that file back into the local vox-db idempotently. See the chat
//! harness continuous eval design spec §9 for why: this is the sync mechanism that lets any
//! developer's local GUI/CLI see CI's nightly results after a `git pull`, with no server
//! dependency.

use serde::{Deserialize, Serialize};

/// One line of the published JSONL file — a self-contained snapshot of one run and its children,
/// so ingest can upsert a whole run atomically per line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedRun {
    pub run: vox_db::HarnessEvalRunRecord,
    pub task_results: Vec<vox_db::HarnessEvalTaskResultRecord>,
    pub selection_events: Vec<vox_db::ModelSelectionEventRecord>,
}

/// Serialize a batch of runs to JSONL (one `PublishedRun` per line).
pub fn to_jsonl(runs: &[PublishedRun]) -> String {
    runs.iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a JSONL blob back into `PublishedRun`s, skipping any line that fails to parse (forward-
/// compatible with future schema additions — a malformed/future-shaped line is logged and
/// skipped, never a hard error that blocks ingesting the rest of the file).
pub fn from_jsonl(blob: &str) -> (Vec<PublishedRun>, Vec<String>) {
    let mut runs = Vec::new();
    let mut skipped_lines = Vec::new();
    for line in blob.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<PublishedRun>(line) {
            Ok(r) => runs.push(r),
            Err(e) => skipped_lines.push(format!("{e}: {line}")),
        }
    }
    (runs, skipped_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_run(run_id: &str) -> PublishedRun {
        PublishedRun {
            run: vox_db::HarnessEvalRunRecord {
                run_id: run_id.to_string(),
                triggered_by: "ci-nightly".to_string(),
                git_sha: "abc1234".to_string(),
                git_branch: "main".to_string(),
                changed_files: vec![],
                config_version: None,
                samples_per_task: 3,
                task_count: 1,
                pass_count: 1,
                fail_count: 0,
                skip_count: 0,
                total_cost_usd: 0.001,
                started_at_ms: 1000,
                finished_at_ms: 2000,
            },
            task_results: vec![],
            selection_events: vec![],
        }
    }

    #[test]
    fn jsonl_round_trip_preserves_run_id() {
        let runs = vec![fixture_run("run-1"), fixture_run("run-2")];
        let blob = to_jsonl(&runs);
        let (parsed, skipped) = from_jsonl(&blob);
        assert!(skipped.is_empty());
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].run.run_id, "run-1");
        assert_eq!(parsed[1].run.run_id, "run-2");
    }

    #[test]
    fn from_jsonl_skips_malformed_lines_without_failing_the_whole_parse() {
        let blob = format!("{}\nnot valid json\n{}", to_jsonl(&[fixture_run("run-1")]), to_jsonl(&[fixture_run("run-2")]));
        let (parsed, skipped) = from_jsonl(&blob);
        assert_eq!(parsed.len(), 2);
        assert_eq!(skipped.len(), 1);
    }

    #[tokio::test]
    async fn ingesting_the_same_jsonl_twice_produces_no_duplicate_rows() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("db");
        let runs = vec![fixture_run("run-idempotent-1")];

        ingest_runs(&db, &runs).await.expect("first ingest");
        ingest_runs(&db, &runs).await.expect("second ingest (same data)");

        let listed = db.list_harness_eval_runs(10).await.expect("list");
        assert_eq!(
            listed.len(),
            1,
            "ingesting the identical run twice must not create a duplicate row"
        );
    }
}
```

- [ ] **Step 2: Run, verify the idempotency test fails**

Run: `cargo test -p vox-cli --lib commands::harness::publish::tests::ingesting_the_same_jsonl_twice_produces_no_duplicate_rows`
Expected: compile error — `ingest_runs` doesn't exist yet.

- [ ] **Step 3: Implement `ingest_runs` with an idempotent upsert**

`record_harness_eval_run` (Task 2) inserts into a table with `run_id TEXT NOT NULL UNIQUE` — a
second insert with the same `run_id` will violate that constraint and error, not silently
duplicate. Implement `ingest_runs` to check-then-skip rather than blindly re-inserting:

```rust
/// Ingest a batch of published runs into the local DB. Idempotent: a run_id already present is
/// skipped entirely (run + its children), not re-inserted or duplicated.
pub async fn ingest_runs(
    db: &vox_db::VoxDb,
    runs: &[PublishedRun],
) -> anyhow::Result<usize> {
    let existing: std::collections::HashSet<String> = db
        .list_harness_eval_runs(10_000)
        .await?
        .into_iter()
        .map(|r| r.run_id)
        .collect();

    let mut ingested = 0;
    for published in runs {
        if existing.contains(&published.run.run_id) {
            continue;
        }
        db.record_harness_eval_run(&published.run).await?;
        for task_result in &published.task_results {
            db.record_harness_eval_task_result(task_result).await?;
        }
        for event in &published.selection_events {
            db.record_model_selection_event(event).await?;
        }
        ingested += 1;
    }
    Ok(ingested)
}
```

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::publish`
Expected: all 3 tests pass.

- [ ] **Step 5: Wire persistence into `eval.rs`'s `--live` path**

Replace Task 5 Step 8's placeholder `let _ = (task_results, selection_events);` in
`crates/vox-cli/src/commands/harness/eval.rs` with real persistence:

```rust
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::default_path()?).await?;
        db.record_harness_eval_run(&run).await?;
        for task_result in &task_results {
            db.record_harness_eval_task_result(task_result).await?;
        }
        for event in &selection_events {
            db.record_model_selection_event(event).await?;
        }
```

Read `vox-db`'s real `DbConfig` variants first (`Memory` was used in tests above; confirm the real
name of the "default local file path" variant/constructor before using it here — this plan's
`DbConfig::default_path()?` is a placeholder name, not verified against the real enum).

- [ ] **Step 6: Add the `publish` CLI subcommand**

Add to `crates/vox-cli/src/commands/harness/publish.rs`:

```rust
use clap::Parser;

#[derive(Parser)]
pub struct PublishArgs {
    /// Path to the git-tracked JSONL history file.
    #[arg(long, default_value = "docs/harness-eval-history/runs.jsonl")]
    pub path: std::path::PathBuf,
}

/// Export every local `harness_eval_run` not already present in the JSONL file at `args.path`,
/// appending them (auto-generated file — never hand-edit, per this repo's convention for
/// generated docs).
pub async fn run(args: PublishArgs) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::default_path()?).await?;
    let existing_blob = std::fs::read_to_string(&args.path).unwrap_or_default();
    let (already_published, _) = from_jsonl(&existing_blob);
    let already_published_ids: std::collections::HashSet<String> = already_published
        .iter()
        .map(|p| p.run.run_id.clone())
        .collect();

    let local_runs = db.list_harness_eval_runs(10_000).await?;
    let mut newly_published = Vec::new();
    for run in local_runs {
        if already_published_ids.contains(&run.run_id) {
            continue;
        }
        let task_results = db.get_harness_eval_task_results(&run.run_id).await?;
        let selection_events = db.get_model_selection_events(&run.run_id).await?;
        newly_published.push(PublishedRun {
            run,
            task_results,
            selection_events,
        });
    }

    if newly_published.is_empty() {
        println!("nothing new to publish");
        return Ok(());
    }

    let new_lines = to_jsonl(&newly_published);
    if let Some(parent) = args.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.path)?;
    use std::io::Write;
    if !existing_blob.is_empty() && !existing_blob.ends_with('\n') {
        writeln!(file)?;
    }
    writeln!(file, "{new_lines}")?;

    println!("published {} run(s) to {}", newly_published.len(), args.path.display());
    Ok(())
}
```

- [ ] **Step 7: Register `publish` in the harness subcommand dispatcher**

Read `crates/vox-cli/src/commands/harness/mod.rs`'s current subcommand enum/dispatch (how `eval`
is wired as a `vox harness eval` subcommand) and add `publish` following the identical pattern —
`vox harness eval publish` (matching the design spec's naming) or `vox harness publish`, whichever
matches this file's existing subcommand nesting convention (read the real file before choosing;
this plan assumes `vox harness eval publish` per the spec, but the actual clap subcommand tree
structure must be read first since `eval.rs`'s `EvalArgs` is a flat args struct, not itself a
subcommand enum with children — if `eval` isn't structured to have subcommands today, add
`publish` as a sibling top-level `vox harness publish` command instead, and note this naming
deviation from the spec in the commit message).

- [ ] **Step 8: Add a GUI/CLI local ingest step**

Add a small helper the GUI backend (Task 9) and a new `vox harness eval history` CLI command (Task
8) both call before reading: on startup, read `docs/harness-eval-history/runs.jsonl` (if present)
and call `ingest_runs` against the local DB. Add this as a public function in `publish.rs`:

```rust
/// Sync the local DB from the git-committed JSONL history file, if present. Called by both the
/// CLI (`history`/`report`) and the GUI backend before querying, so a fresh `git pull` is
/// reflected without requiring the user to manually run `publish`.
pub async fn sync_from_jsonl(
    db: &vox_db::VoxDb,
    path: &std::path::Path,
) -> anyhow::Result<usize> {
    let blob = match std::fs::read_to_string(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let (runs, _skipped) = from_jsonl(&blob);
    Ok(ingest_runs(db, &runs).await?)
}
```

- [ ] **Step 9: Run the full harness test suite**

Run: `cargo test -p vox-cli --lib commands::harness`
Expected: all pass.

- [ ] **Step 10: Mark the output file as auto-generated**

Create `docs/harness-eval-history/README.md`:

```markdown
# Harness Eval History (auto-generated)

`runs.jsonl` in this directory is written by `vox harness eval publish` and read by
`vox harness eval history`/`report` and the Vox Axis GUI's Harness Health surface. It is an
append-only, git-tracked sync mechanism (see
`docs/superpowers/specs/2026-08-02-chat-harness-continuous-eval-design.md` §9) — **never hand-edit
`runs.jsonl`**, per this repo's convention for auto-generated files.
```

- [ ] **Step 11: Commit**

```bash
git add crates/vox-cli/src/commands/harness/publish.rs crates/vox-cli/src/commands/harness/mod.rs crates/vox-cli/src/commands/harness/eval.rs docs/harness-eval-history/README.md
git commit -m "feat(vox-cli): add harness eval publish/sync via git-committed JSONL"
```

---

## Task 7: Scheduled GitHub Actions workflow

**Files:**
- Create: `.github/workflows/harness-eval-nightly.yml`

- [ ] **Step 1: Read an existing scheduled or self-hosted-runner workflow for conventions**

Read `.github/workflows/ci.yml` (referenced earlier this session, e.g. its
`gui-orchestrator-relaunch-smoke` job) to confirm the exact `runs-on:` label this repo's
self-hosted runner uses, and how secrets (API keys) are referenced in other jobs that need live
credentials.

- [ ] **Step 2: Write the workflow file**

Create `.github/workflows/harness-eval-nightly.yml`:

```yaml
name: Harness Eval (Nightly Live)

on:
  schedule:
    # 09:00 UTC nightly. Adjust the runs-on label below to match this repo's real self-hosted
    # runner label, confirmed by reading ci.yml in Step 1 — the placeholder below must be
    # replaced before this workflow is usable.
    - cron: '0 9 * * *'
  workflow_dispatch: {}

jobs:
  live-eval:
    runs-on: [self-hosted, REPLACE-WITH-REAL-RUNNER-LABEL-FROM-STEP-1]
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4

      - name: Build vox-cli
        run: cargo build --release -p vox-cli

      - name: Run live harness eval
        env:
          VOX_HARNESS_EVAL_TRIGGERED_BY: ci-nightly
          # Replace with this repo's real secret name(s) for API keys, confirmed in Step 1.
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: ./target/release/vox harness eval --live --samples 3

      - name: Publish results to git-tracked history
        run: ./target/release/vox harness eval publish

      - name: Commit and push published results
        run: |
          git config user.name "vox-harness-eval-bot"
          git config user.email "noreply@example.invalid"
          git add docs/harness-eval-history/runs.jsonl
          git diff --cached --quiet || git commit -m "chore(harness-eval): publish nightly run results [skip ci]"
          git push
```

Do not treat the `runs-on` label or secret name placeholders above as final — Step 1 explicitly
requires reading the real workflow file first; replace both placeholders with the confirmed real
values before this workflow can run.

- [ ] **Step 3: Verify the workflow is syntactically valid**

Run: `cd "$(git rev-parse --show-toplevel)" && python -c "import yaml; yaml.safe_load(open('.github/workflows/harness-eval-nightly.yml'))"`
(or any available YAML linter/validator in this environment) to confirm the file parses. This
cannot be fully tested without triggering it (which costs real money per the live-call design) —
`workflow_dispatch` is included specifically so it can be manually triggered once for a real
verification run after this task ships, rather than waiting for the nightly cron.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/harness-eval-nightly.yml
git commit -m "ci: add nightly scheduled live harness eval workflow"
```

---

## Task 8: CLI `history`/`report` commands + regression detection

**Files:**
- Create: `crates/vox-cli/src/commands/harness/report.rs`
- Modify: `crates/vox-cli/src/commands/harness/mod.rs` (register subcommands)
- Test: inline `#[cfg(test)]` in `report.rs`

- [ ] **Step 1: Write the failing regression-detection tests**

Create `crates/vox-cli/src/commands/harness/report.rs`:

```rust
//! `vox harness eval history`/`report` — CLI surfacing for persisted harness eval runs, plus
//! the regression-detection logic shared with the GUI's Harness Health surface (design spec
//! §10.3).

/// A detected regression between two consecutive runs.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionFlag {
    pub kind: RegressionKind,
    pub previous_run_id: String,
    pub current_run_id: String,
    pub previous_git_sha: String,
    pub current_git_sha: String,
    pub changed_files: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    PassRateDrop,
    CostTierRatioDrop,
}

const PASS_RATE_DROP_THRESHOLD_PP: f64 = 10.0;
const COST_TIER_RATIO_DROP_THRESHOLD_PP: f64 = 15.0;

fn pass_rate(run: &vox_db::HarnessEvalRunRecord) -> f64 {
    let graded = run.task_count - run.skip_count;
    if graded <= 0 {
        return 100.0;
    }
    (run.pass_count as f64 / graded as f64) * 100.0
}

fn free_cheap_ratio(events: &[vox_db::ModelSelectionEventRecord]) -> f64 {
    let non_privacy_forced: Vec<_> = events.iter().filter(|e| !e.was_privacy_gated).collect();
    if non_privacy_forced.is_empty() {
        return 100.0;
    }
    let free_or_cheap = non_privacy_forced
        .iter()
        .filter(|e| e.cost_tier == "free" || e.cost_tier == "cheap")
        .count();
    (free_or_cheap as f64 / non_privacy_forced.len() as f64) * 100.0
}

/// Compare two consecutive runs (previous, current) and their selection events, returning any
/// regressions detected. Pure function — no DB access — so it's fully unit-testable against
/// fixture data (design spec §12).
pub fn detect_regressions(
    previous: &vox_db::HarnessEvalRunRecord,
    current: &vox_db::HarnessEvalRunRecord,
    previous_events: &[vox_db::ModelSelectionEventRecord],
    current_events: &[vox_db::ModelSelectionEventRecord],
    changed_files: &[String],
) -> Vec<RegressionFlag> {
    let mut flags = Vec::new();

    let prev_pass_rate = pass_rate(previous);
    let cur_pass_rate = pass_rate(current);
    if prev_pass_rate - cur_pass_rate > PASS_RATE_DROP_THRESHOLD_PP {
        flags.push(RegressionFlag {
            kind: RegressionKind::PassRateDrop,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            detail: format!(
                "pass rate dropped from {prev_pass_rate:.1}% to {cur_pass_rate:.1}%"
            ),
        });
    }

    let prev_ratio = free_cheap_ratio(previous_events);
    let cur_ratio = free_cheap_ratio(current_events);
    if prev_ratio - cur_ratio > COST_TIER_RATIO_DROP_THRESHOLD_PP {
        flags.push(RegressionFlag {
            kind: RegressionKind::CostTierRatioDrop,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            detail: format!(
                "free/cheap model-selection ratio dropped from {prev_ratio:.1}% to {cur_ratio:.1}%"
            ),
        });
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(run_id: &str, pass_count: i64, task_count: i64) -> vox_db::HarnessEvalRunRecord {
        vox_db::HarnessEvalRunRecord {
            run_id: run_id.to_string(),
            triggered_by: "ci-nightly".to_string(),
            git_sha: format!("sha-{run_id}"),
            git_branch: "main".to_string(),
            changed_files: vec![],
            config_version: None,
            samples_per_task: 3,
            task_count,
            pass_count,
            fail_count: task_count - pass_count,
            skip_count: 0,
            total_cost_usd: 0.01,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        }
    }

    fn event(model_id: &str, cost_tier: &str, privacy_gated: bool) -> vox_db::ModelSelectionEventRecord {
        vox_db::ModelSelectionEventRecord {
            run_id: "r".to_string(),
            task_id: "t".to_string(),
            model_id: model_id.to_string(),
            cost_tier: cost_tier.to_string(),
            selection_reason: "test".to_string(),
            was_privacy_gated: privacy_gated,
            recorded_at_ms: 1000,
        }
    }

    #[test]
    fn no_regression_when_pass_rate_and_ratio_are_stable() {
        let prev = run("r1", 9, 10);
        let cur = run("r2", 9, 10);
        let prev_events = vec![event("m1", "free", false); 5];
        let cur_events = vec![event("m1", "free", false); 5];
        let flags = detect_regressions(&prev, &cur, &prev_events, &cur_events, &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn pass_rate_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 5, 10); // 100% -> 50%, a 50pp drop
        let flags = detect_regressions(&prev, &cur, &[], &[], &["src/foo.rs".to_string()]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].kind, RegressionKind::PassRateDrop);
        assert_eq!(flags[0].changed_files, vec!["src/foo.rs".to_string()]);
    }

    #[test]
    fn small_pass_rate_drop_under_threshold_is_not_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 9, 10); // 100% -> 90%, a 10pp drop, not > threshold
        let flags = detect_regressions(&prev, &cur, &[], &[], &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn cost_tier_ratio_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 10, 10);
        let prev_events = vec![event("m1", "free", false); 10];
        let cur_events = vec![event("m1", "premium", false); 10]; // 100% -> 0% free/cheap
        let flags = detect_regressions(&prev, &cur, &prev_events, &cur_events, &[]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].kind, RegressionKind::CostTierRatioDrop);
    }

    #[test]
    fn privacy_gated_events_are_excluded_from_the_ratio_calculation() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 10, 10);
        // All events are privacy-gated (forced local); tier drift among them must not count.
        let prev_events = vec![event("local-1", "free", true); 5];
        let cur_events = vec![event("local-1", "premium", true); 5];
        let flags = detect_regressions(&prev, &cur, &prev_events, &cur_events, &[]);
        assert!(
            flags.is_empty(),
            "privacy-forced selections must not affect the free/cheap ratio regression check"
        );
    }

    #[test]
    fn both_regressions_can_be_flagged_simultaneously() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 5, 10);
        let prev_events = vec![event("m1", "free", false); 10];
        let cur_events = vec![event("m1", "premium", false); 10];
        let flags = detect_regressions(&prev, &cur, &prev_events, &cur_events, &[]);
        assert_eq!(flags.len(), 2);
    }
}
```

- [ ] **Step 2: Run, verify it fails to compile**

Run: `cargo test -p vox-cli --lib commands::harness::report`
Expected: compile error until the module is registered (Step 4).

- [ ] **Step 3: Add the `history`/`report` CLI commands**

Add to the same file, below the test module (or above — Rust doesn't require ordering, but match
this crate's existing convention of public API above `#[cfg(test)]`):

```rust
use clap::Parser;

#[derive(Parser)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

pub async fn run_history(args: HistoryArgs) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::default_path()?).await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let runs = db.list_harness_eval_runs(args.limit).await?;
    if runs.is_empty() {
        println!("no harness eval runs recorded yet");
        return Ok(());
    }
    println!("{:<24} {:<12} {:>6} {:>6} {:>6} {:>10}", "run_id", "git_sha", "pass", "fail", "skip", "cost_usd");
    for run in &runs {
        println!(
            "{:<24} {:<12} {:>6} {:>6} {:>6} {:>10.4}",
            run.run_id, run.git_sha, run.pass_count, run.fail_count, run.skip_count, run.total_cost_usd
        );
    }
    Ok(())
}

#[derive(Parser)]
pub struct ReportArgs {
    /// Run id to report on in detail, or omit to compare the two most recent runs.
    pub run_id: Option<String>,
}

pub async fn run_report(args: ReportArgs) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::default_path()?).await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let runs = db.list_harness_eval_runs(2).await?;
    if runs.len() < 2 {
        println!("need at least 2 runs to compare; only {} recorded", runs.len());
        return Ok(());
    }
    let (current, previous) = (&runs[0], &runs[1]);
    let current_events = db.get_model_selection_events(&current.run_id).await?;
    let previous_events = db.get_model_selection_events(&previous.run_id).await?;
    let changed_files: Vec<String> = std::process::Command::new("git")
        .args(["diff", "--name-only", &format!("{}..{}", previous.git_sha, current.git_sha)])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();

    let flags = detect_regressions(previous, current, &previous_events, &current_events, &changed_files);
    if flags.is_empty() {
        println!("no regressions detected between {} and {}", previous.run_id, current.run_id);
    } else {
        for flag in &flags {
            println!(
                "REGRESSION [{:?}]: {} (git {}..{}, {} file(s) changed)",
                flag.kind, flag.detail, flag.previous_git_sha, flag.current_git_sha, flag.changed_files.len()
            );
            for f in &flag.changed_files {
                println!("    {f}");
            }
        }
    }
    let _ = args.run_id; // single-run detail view is a natural follow-up; comparing the two most
                          // recent runs is the design spec's primary use case and what this
                          // implements first.
    Ok(())
}
```

- [ ] **Step 4: Register the module and subcommands**

Add `pub mod report;` to `crates/vox-cli/src/commands/harness/mod.rs`, and wire `history`/`report`
as subcommands following the exact same convention discovered in Task 6 Step 7 for `publish`.

- [ ] **Step 5: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::report`
Expected: all 6 tests pass.

- [ ] **Step 6: Manual smoke test**

Run: `cargo run -p vox-cli -- harness eval history` (after Task 5-7 have produced at least one
real run, or against an empty DB to confirm the "no runs recorded yet" path).
Expected: prints either the "no runs" message or a table, without panicking.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/harness/report.rs crates/vox-cli/src/commands/harness/mod.rs
git commit -m "feat(vox-cli): add harness eval history/report commands + regression detection"
```

---

## Task 9: GUI backend — Tauri commands for Harness Health

**Files:**
- Create: `crates/vox-gui/src/commands/harness_eval.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (register module)
- Modify: `crates/vox-gui/src/main.rs` (register the two new Tauri commands in
  `generate_handler!`)
- Test: inline `#[cfg(test)]` in the new file

- [ ] **Step 1: Read the exact `GuiDbPool`/`pool_db` pattern**

Read `crates/vox-gui/src/commands/chat.rs` lines 1-20 (the `pool_db` helper and its imports) —
confirmed earlier this session, mirror exactly.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/src/commands/harness_eval.rs`:

```rust
//! Tauri commands backing the Vox Axis "Harness Health" GUI surface (chat harness continuous
//! eval design, 2026-08-02). Read-only: all writes to these tables happen via `vox harness eval
//! --live`/`publish` (CLI), never from the GUI.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use vox_db::VoxDb;

use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

fn pool_db(pool: &GuiDbPool) -> Result<Arc<VoxDb>, String> {
    pool.handle()
}

/// One row of the GUI's recent-runs table.
#[derive(Debug, Serialize)]
pub struct HarnessEvalRunDto {
    pub run_id: String,
    pub git_sha: String,
    pub triggered_by: String,
    pub pass_count: i64,
    pub fail_count: i64,
    pub skip_count: i64,
    pub total_cost_usd: f64,
    pub started_at_ms: i64,
}

#[tauri::command]
pub async fn harness_eval_history(
    pool: State<'_, GuiDbPool>,
    limit: Option<usize>,
) -> Result<Vec<HarnessEvalRunDto>, String> {
    let db = pool_db(&pool)?;
    let runs = db
        .list_harness_eval_runs(limit.unwrap_or(50))
        .await
        .map_err(map_db_err)?;
    Ok(runs
        .into_iter()
        .map(|r| HarnessEvalRunDto {
            run_id: r.run_id,
            git_sha: r.git_sha,
            triggered_by: r.triggered_by,
            pass_count: r.pass_count,
            fail_count: r.fail_count,
            skip_count: r.skip_count,
            total_cost_usd: r.total_cost_usd,
            started_at_ms: r.started_at_ms,
        })
        .collect())
}

/// One flagged regression, DTO shape for the GUI's regression banner (design spec §10.2).
#[derive(Debug, Serialize)]
pub struct RegressionFlagDto {
    pub kind: String,
    pub previous_run_id: String,
    pub current_run_id: String,
    pub previous_git_sha: String,
    pub current_git_sha: String,
    pub changed_files: Vec<String>,
    pub detail: String,
}

/// Compares the two most recent runs and returns any detected regressions (empty if none, or if
/// fewer than 2 runs exist yet). Reuses `vox-cli`'s pure `detect_regressions` function directly
/// — no duplicated logic between the CLI's `report` command and this GUI command.
#[tauri::command]
pub async fn harness_eval_regressions(
    pool: State<'_, GuiDbPool>,
) -> Result<Vec<RegressionFlagDto>, String> {
    let db = pool_db(&pool)?;
    let runs = db.list_harness_eval_runs(2).await.map_err(map_db_err)?;
    if runs.len() < 2 {
        return Ok(vec![]);
    }
    let (current, previous) = (&runs[0], &runs[1]);
    let current_events = db
        .get_model_selection_events(&current.run_id)
        .await
        .map_err(map_db_err)?;
    let previous_events = db
        .get_model_selection_events(&previous.run_id)
        .await
        .map_err(map_db_err)?;
    let changed_files: Vec<String> = std::process::Command::new("git")
        .args([
            "diff",
            "--name-only",
            &format!("{}..{}", previous.git_sha, current.git_sha),
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();

    let flags = vox_cli::commands::harness::report::detect_regressions(
        previous,
        current,
        &previous_events,
        &current_events,
        &changed_files,
    );
    Ok(flags
        .into_iter()
        .map(|f| RegressionFlagDto {
            kind: format!("{:?}", f.kind),
            previous_run_id: f.previous_run_id,
            current_run_id: f.current_run_id,
            previous_git_sha: f.previous_git_sha,
            current_git_sha: f.current_git_sha,
            changed_files: f.changed_files,
            detail: f.detail,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn harness_eval_run_dto_maps_all_fields_from_the_record() {
        let record = vox_db::HarnessEvalRunRecord {
            run_id: "r1".to_string(),
            triggered_by: "ci-nightly".to_string(),
            git_sha: "abc123".to_string(),
            git_branch: "main".to_string(),
            changed_files: vec![],
            config_version: None,
            samples_per_task: 3,
            task_count: 10,
            pass_count: 9,
            fail_count: 1,
            skip_count: 0,
            total_cost_usd: 0.05,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        };
        let dto = HarnessEvalRunDto {
            run_id: record.run_id.clone(),
            git_sha: record.git_sha.clone(),
            triggered_by: record.triggered_by.clone(),
            pass_count: record.pass_count,
            fail_count: record.fail_count,
            skip_count: record.skip_count,
            total_cost_usd: record.total_cost_usd,
            started_at_ms: record.started_at_ms,
        };
        assert_eq!(dto.run_id, "r1");
        assert_eq!(dto.pass_count, 9);
    }
}
```

- [ ] **Step 3: Register the module**

Add `pub mod harness_eval;` to `crates/vox-gui/src/commands/mod.rs`, matching the existing
`pub mod chat;` style declaration.

- [ ] **Step 4: Register both Tauri commands**

In `crates/vox-gui/src/main.rs`, add near the existing `commands::chat::chat_list_sessions,` line:

```rust
            commands::harness_eval::harness_eval_history,
            commands::harness_eval::harness_eval_regressions,
```

Confirm `crates/vox-gui/Cargo.toml` already has `vox-cli = { workspace = true }` (it does, per this
session's dependency check — `harness_eval_regressions` calls
`vox_cli::commands::harness::report::detect_regressions` directly, and `vox-cli`'s `commands`
module is already `pub`, so no new dependency is needed).

- [ ] **Step 5: Run the test, verify it passes**

Run: `cargo test -p vox-gui --bins commands::harness_eval`
Expected: PASS.

- [ ] **Step 6: Run the full `vox-gui` test suite**

Run: `cargo test -p vox-gui --bins`
Expected: all pass, no regressions from the new command registration.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/harness_eval.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): add harness_eval_history and harness_eval_regressions Tauri commands"
```

---

## Task 10: GUI frontend — Harness Health surface

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.test.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx` (add `'harness-health'` to the `View` union and
  `KNOWN_VIEWS`/`LEGACY_VIEWS` array — do NOT reuse the existing `'harness'` view key, which
  already routes to the unrelated `HarnessRedirect` legacy-tab component)
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (route `'harness-health'`
  to the new component)

- [ ] **Step 1: Read the exact current `View` union and a comparable surface's routing**

Read `crates/vox-gui/ui/src/App.tsx`'s current `View` type union, `LEGACY_VIEWS`, and
`KNOWN_VIEWS` (confirmed earlier this session — re-read for any drift since). Read
`crates/vox-gui/ui/src/components/surfaces/Coverage/CoverageView.tsx` in full (already read this
session) as the structural template — a `SurfaceDecoratorProps`-typed component, no live data
fetch in that particular example; for THIS surface, mirror a live-data-fetching surface instead —
grep `surfaceComponents.tsx` for a `useQuery`-based surface (e.g. how `chat_list_sessions` reaches
`ChatSurface`) and follow that data-fetching pattern, not `CoverageView`'s static-registry one.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.test.tsx`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { HarnessHealthView } from './HarnessHealthView';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function renderWithClient(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

describe('HarnessHealthView', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('renders recent runs from harness_eval_history', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'harness_eval_history') {
        return Promise.resolve([
          {
            run_id: 'abc1234-1000',
            git_sha: 'abc1234',
            triggered_by: 'ci-nightly',
            pass_count: 8,
            fail_count: 1,
            skip_count: 0,
            total_cost_usd: 0.05,
            started_at_ms: 1700000000000,
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText('abc1234-1000')).toBeInTheDocument();
    });
    expect(screen.getByText(/8/)).toBeInTheDocument();
  });

  it('shows an empty state when no runs exist yet', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'harness_eval_history') return Promise.resolve([]);
      if (cmd === 'harness_eval_regressions') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText(/no harness eval runs/i)).toBeInTheDocument();
    });
  });

  it('shows a regression banner when harness_eval_regressions returns a flag', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'harness_eval_history') {
        return Promise.resolve([
          {
            run_id: 'def5678-2000', git_sha: 'def5678', triggered_by: 'ci-nightly',
            pass_count: 5, fail_count: 5, skip_count: 0, total_cost_usd: 0.05, started_at_ms: 1700000001000,
          },
        ]);
      }
      if (cmd === 'harness_eval_regressions') {
        return Promise.resolve([
          {
            kind: 'PassRateDrop',
            previous_run_id: 'abc1234-1000',
            current_run_id: 'def5678-2000',
            previous_git_sha: 'abc1234',
            current_git_sha: 'def5678',
            changed_files: ['crates/vox-orchestrator/src/runtime.rs'],
            detail: 'pass rate dropped from 100.0% to 50.0%',
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText(/pass rate dropped from 100.0% to 50.0%/i)).toBeInTheDocument();
    });
    expect(screen.getByText('crates/vox-orchestrator/src/runtime.rs')).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run, verify it fails**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/HarnessHealth/HarnessHealthView.test.tsx`
Expected: FAIL — `HarnessHealthView` module doesn't exist yet.

- [ ] **Step 4: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.tsx`:

```tsx
import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';

interface HarnessEvalRunDto {
  run_id: string;
  git_sha: string;
  triggered_by: string;
  pass_count: number;
  fail_count: number;
  skip_count: number;
  total_cost_usd: number;
  started_at_ms: number;
}

interface RegressionFlagDto {
  kind: string;
  previous_run_id: string;
  current_run_id: string;
  previous_git_sha: string;
  current_git_sha: string;
  changed_files: string[];
  detail: string;
}

export function HarnessHealthView() {
  const { data: runs, isLoading } = useQuery({
    queryKey: ['harnessEvalHistory'],
    queryFn: () => invoke<HarnessEvalRunDto[]>('harness_eval_history', { limit: 50 }),
  });
  const { data: regressions } = useQuery({
    queryKey: ['harnessEvalRegressions'],
    queryFn: () => invoke<RegressionFlagDto[]>('harness_eval_regressions'),
  });

  if (isLoading) {
    return <p className="text-[12px] text-text-muted">Loading harness eval history…</p>;
  }

  if (!runs || runs.length === 0) {
    return (
      <EmptyState
        icon={<Icon.bolt className="size-8" />}
        title="No harness eval runs yet"
        description="Run `vox harness eval --live` locally, or wait for the nightly scheduled workflow, to see chat harness quality and model-selection trends here."
      />
    );
  }

  return (
    <section className="space-y-4" aria-labelledby="harness-health-title">
      <h2 id="harness-health-title" className="font-display text-lg text-text-primary tracking-wider uppercase">
        Harness Health
      </h2>
      {regressions && regressions.length > 0 && (
        <div className="space-y-2" role="alert">
          {regressions.map((r) => (
            <div
              key={`${r.kind}-${r.previous_run_id}-${r.current_run_id}`}
              className="rounded-lg border border-red-400/30 bg-red-400/[0.06] p-3 text-[12px]"
            >
              <p className="font-medium text-red-300">
                Regression detected ({r.kind}): {r.detail}
              </p>
              <p className="mt-1 font-mono text-[10px] text-text-muted">
                {r.previous_git_sha}..{r.current_git_sha}
              </p>
              {r.changed_files.length > 0 && (
                <ul className="mt-1 space-y-0.5 font-mono text-[10px] text-text-muted">
                  {r.changed_files.map((f) => (
                    <li key={f}>{f}</li>
                  ))}
                </ul>
              )}
            </div>
          ))}
        </div>
      )}
      <div className="overflow-auto rounded-lg border border-border-subtle">
        <table className="w-full text-left text-[12px]">
          <caption className="sr-only">Recent chat harness eval runs</caption>
          <thead className="text-text-muted">
            <tr>
              <th scope="col" className="p-2">Run</th>
              <th scope="col" className="p-2">Git SHA</th>
              <th scope="col" className="p-2">Triggered by</th>
              <th scope="col" className="p-2">Pass</th>
              <th scope="col" className="p-2">Fail</th>
              <th scope="col" className="p-2">Skip</th>
              <th scope="col" className="p-2">Cost</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.run_id} className="border-t border-border-subtle">
                <td className="p-2 font-mono text-text-secondary">{r.run_id}</td>
                <td className="p-2 font-mono text-text-muted">{r.git_sha}</td>
                <td className="p-2 text-text-muted">{r.triggered_by}</td>
                <td className="p-2 text-emerald-300">{r.pass_count}</td>
                <td className="p-2 text-red-300">{r.fail_count}</td>
                <td className="p-2 text-text-muted">{r.skip_count}</td>
                <td className="p-2">${r.total_cost_usd.toFixed(4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run, verify the test passes**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/HarnessHealth/HarnessHealthView.test.tsx`
Expected: 3 tests pass.

- [ ] **Step 6: Wire the new view into `App.tsx` and `surfaceComponents.tsx`**

In `crates/vox-gui/ui/src/App.tsx`, add `'harness-health'` to the `View` union type and to
`LEGACY_VIEWS`'s array literal (read the current exact array first — confirmed earlier this
session it's a flat `string[]` with entries like `'harness'`, `'coverage'`; add `'harness-health'`
as a new, distinct entry, not a replacement for `'harness'`).

In `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`, find where `'coverage'`
routes to `CoverageView` (or wherever the view-key-to-component switch/map lives) and add an
identical entry routing `'harness-health'` to `HarnessHealthView`, importing it from
`../surfaces/HarnessHealth/HarnessHealthView`.

- [ ] **Step 7: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run`
Expected: clean typecheck, all tests pass (including the two new ones), no regressions.

- [ ] **Step 8: Manual verification**

Use `preview_start` with the `vox-gui`/`vox-gui-limes` launch config if it binds in this
environment (per this session's established pattern — it may not; if so, rely on the automated
tests + a careful manual code read as the substitute, per the standard already established
earlier in this session for exactly this situation). If it binds: navigate to the new
`#view=harness-health` URL hash, confirm the empty state renders (no eval runs exist yet in a
fresh dev DB), and confirm no console errors.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.tsx crates/vox-gui/ui/src/components/surfaces/HarnessHealth/HarnessHealthView.test.tsx crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
git commit -m "feat(vox-gui): add Harness Health surface showing eval run history"
```

---

## After all 10 tasks: final verification

- [ ] Run the full backend suite: `cargo test -p vox-orchestrator -p vox-orchestrator-mcp -p vox-db -p vox-cli -p vox-gui -p vox-integration-tests --lib --tests`
- [ ] Run `cargo clippy -p vox-orchestrator -p vox-orchestrator-mcp -p vox-db -p vox-cli -p vox-gui -- -D warnings`
- [ ] Run the full frontend suite: `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run`
- [ ] Dispatch a final holistic code-reviewer subagent for the entire branch diff (per
  subagent-driven-development's own closing step), then use `superpowers:finishing-a-development-branch`
  to decide merge/PR/keep/discard.
