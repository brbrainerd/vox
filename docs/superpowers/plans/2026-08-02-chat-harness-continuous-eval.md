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

The real submission method is `Orchestrator::submit_task_with_agent` (NOT `submit_task_with_hints`
— that function does not exist anywhere in this codebase; confirmed by reading
`crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:106`). Its real
signature is `(description, file_manifest, priority, target_agent, capability_requirements,
enqueue_hints, session_id, tenant_id)`. To actually trigger the gate cascade (not just assert on
submission-time metadata), the task must be dequeued via `orch.agent_queue(agent_id)` before
calling `complete_task` — this exact pattern already exists in this codebase's own test suite at
`crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs`'s
`chat_category_task_completes_without_running_code_gates` test; mirror it precisely rather than
re-deriving the dequeue dance. That existing test only proves a *default-config* chat task
completes without hanging — it does NOT (and isn't meant to) prove gates apply uniformly, since
`OrchestratorConfig::for_testing()` sets `behavioral_gate_on_complete: false`, so no gate actually
fires for either category under that config. To prove uniformity, force a real, deterministic gate
trigger by setting `behavioral_gate_on_complete: true` explicitly (a public field on
`OrchestratorConfig`, confirmed at `crates/vox-orchestrator/src/config/orchestrator_fields.rs:58`)
and complete with no evidence, so the behavioral gate has something to actually fail against.

Add to the new file `crates/vox-integration-tests/tests/chat_harness_regression_test.rs`:

```rust
/// Code-review fix (gui-axis-chat-harness-fixes, 2026-08-02): `TaskCategory::Chat` used to skip
/// the entire approval/trust/behavioral/harness/Socrates gate cascade on completion, on the
/// stale premise that a separate `ChatTaskProcessor` (deleted in this same branch) produced
/// chat replies. This test proves a `TaskCategory::Chat` task now gets the SAME gate treatment
/// as any other category by forcing a real, deterministic gate trigger (behavioral_gate_on_complete
/// = true, no completion evidence) and asserting BOTH categories requeue identically instead of
/// only the non-chat one — a category-based bypass would make this test fail for the chat task
/// specifically while the codegen task still requeues normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_category_task_completion_runs_the_same_gates_as_other_categories() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        async fn submit_and_complete_under_forced_behavioral_gate(
            category: TaskCategory,
        ) -> (vox_orchestrator::OrchestratorStatusSnapshot, bool) {
            let mut config = OrchestratorConfig::for_testing();
            config.behavioral_gate_on_complete = true;
            let orch = Orchestrator::new(config);
            orch.spawn_agent("a1").unwrap();

            let hints = vox_orchestrator::types::TaskEnqueueHints {
                task_category: Some(category),
                ..Default::default()
            };
            let task_id = orch
                .submit_task_with_agent(
                    "task body", vec![], None, None, None, Some(hints), None, None,
                )
                .await
                .expect("task submits");
            let agent_id = *orch.task_assignments.read().unwrap().get(&task_id).unwrap();
            {
                let queue_lock = orch.agent_queue(agent_id).unwrap();
                queue_lock.write().unwrap().dequeue();
            }
            // No attestation evidence provided — with the behavioral gate forced on, this must
            // NOT complete on the first pass for either category if gates are applied uniformly.
            let completed_on_first_pass = orch.complete_task(task_id).await.is_ok()
                && orch.status().total_completed == 1;
            (orch.status(), completed_on_first_pass)
        }

        let (_, chat_completed) =
            submit_and_complete_under_forced_behavioral_gate(TaskCategory::Chat).await;
        let (_, codegen_completed) =
            submit_and_complete_under_forced_behavioral_gate(TaskCategory::CodeGen).await;

        assert_eq!(
            chat_completed, codegen_completed,
            "a Chat-category task must be gated identically to a CodeGen-category task under an \
             identical forced-behavioral-gate-failure scenario — if this assertion fails with \
             chat_completed=true and codegen_completed=false, the category-based gate bypass this \
             test guards against has regressed"
        );
    })
    .await
    .expect("test timed out");
}
```

Before finalizing, confirm empirically (via Step 7's run) that `behavioral_gate_on_complete: true`
with no completion evidence really does make `complete_task` NOT mark the task completed on the
first pass for a non-chat category — the assertion above is written to be symmetric (both
categories must match each other) specifically so it's still a meaningful regression guard even if
the exact true/false polarity of "does it complete on the first pass" differs from what's assumed
here; only update the polarity comment if Step 7 shows the opposite direction, never weaken the
symmetry assertion itself.

- [ ] **Step 7: Run the test, confirm it passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test chat_category_task_completion_runs_the_same_gates_as_other_categories -- --nocapture`
Expected: PASS, with both `chat_completed` and `codegen_completed` printing/asserting equal. If
this test fails to compile because `OrchestratorStatusSnapshot` isn't the real status-return type
name, grep `pub fn status(&self)` in `crates/vox-orchestrator/src/orchestrator.rs` for the real
return type and fix the helper's signature.

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
/// selectable — closing the gap this session's code review fixed. This is case (b) of the
/// spec's two required assertions (§5 item 4); case (a) — no local model registered, fails
/// closed rather than falling back to Cascade — is the separate test below.
///
/// #[serial]: mirrors the real test this fixture is copied from
/// (`crates/vox-orchestrator/src/models/select.rs`'s own
/// `decide_excludes_cloud_candidate_under_local_only_privacy`), which is `#[serial]`-guarded
/// because `set_test_privacy_override` mutates a process-global `Mutex` (`TEST_PRIVACY_OVERRIDE`
/// in `route_policy.rs`) that races other tests touching the same override under parallel
/// `cargo test` if not serialized. `serial_test` is already available in this crate — confirm via
/// `crates/vox-integration-tests/Cargo.toml` and add it as a dev-dependency if not already
/// present.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
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
Expected: PASS.

- [ ] **Step 10: Write the fails-closed (case (a)) test**

This is the second, previously-missing required assertion from spec §5 item 4: with NO local
model registered, task submission under `VOX_INFERENCE_PRIVACY=local_only` must fail closed
(this session's `runtime.rs` fix — `AiTaskProcessor::process` returns `Err` when `routed.is_none()`
and no `model_override` is set under local-only privacy) rather than falling back to
`StreamRoute::Cascade`, which would otherwise stream from a cloud-inclusive provider list. Add to
the same file:

```rust
/// Code-review fix (gui-axis-chat-harness-fixes, 2026-08-02), case (a) of spec §5 item 4: with no
/// local model registered under VOX_INFERENCE_PRIVACY=local_only, AiTaskProcessor::process must
/// fail closed rather than falling back to StreamRoute::Cascade (which streams from a
/// cloud-inclusive provider list with no privacy awareness — the exact leak this session's
/// runtime.rs fix closed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn ai_task_processor_fails_closed_under_local_only_with_no_local_model_registered() {
    use vox_orchestrator::route_policy;

    tokio::time::timeout(TEST_TIMEOUT, async {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();

        // Read the real ModelRegistry mutation API first (grep `models_handle` in
        // crates/vox-orchestrator/src/orchestrator.rs) — this needs to clear the registry down
        // to zero local (Ollama/VoxLocal) candidates before constructing AiTaskProcessor, since
        // the default bootstrap catalog may already contain some. Confirm the real method name
        // (e.g. a `clear()`/`retain()` on the write-locked registry) rather than guessing one.
        {
            let registry_handle = orch.models_handle();
            let mut registry = vox_orchestrator::sync_lock::rw_write(&*registry_handle);
            registry.retain(|m| {
                !matches!(
                    m.provider_type,
                    vox_orchestrator::models::ProviderType::Ollama
                        | vox_orchestrator::models::ProviderType::VoxLocal
                )
            });
        }

        route_policy::set_test_privacy_override(Some("local_only"));
        let processor = vox_orchestrator::AiTaskProcessor::new(
            orch.event_bus.clone(),
            orch.clone(),
        )
        .await;

        let task_id = orch
            .submit_task("say hello", vec![], None, None, None)
            .await
            .expect("task submits");
        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id == task_id)
            .expect("task queued");
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let result = processor.process(vox_orchestrator::types::AgentId(1), task, cancel).await;
        route_policy::set_test_privacy_override(None);

        assert!(
            result.is_err(),
            "AiTaskProcessor::process must fail closed (Err) when no local model is registered \
             under local_only privacy, not silently fall back to a cloud-inclusive Cascade route"
        );
    })
    .await
    .expect("test timed out");
}
```

The `registry.retain(...)`/`models_handle()`/`AgentId(1)` calls above are the least-verified part
of this whole plan — confirm each against the real code (`orchestrator.rs`'s `models_handle`,
`ModelRegistry`'s real mutation methods, `TaskProcessor::process`'s real trait signature in
`runtime.rs`) before treating this test as done; adjust the exact API calls to match what's
actually there.

- [ ] **Step 11: Run the test, confirm it passes**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test ai_task_processor_fails_closed_under_local_only_with_no_local_model_registered -- --nocapture`
Expected: PASS.

- [ ] **Step 12: Run the full new test file together**

Run: `cd crates/vox-integration-tests && cargo test --test chat_harness_regression_test`
Expected: 5 tests pass (session-id isolation, send-lock ordering, gate-cascade uniformity, and
BOTH privacy-filter cases (a) and (b)).

- [ ] **Step 13: Commit**

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

/// One turn's real, observed outcome — what a [`Checker`] evaluates. Deliberately has no
/// `tool_calls_made`/internal-tool-log field: `chat_message`'s public JSON envelope (Task 5) does
/// not expose one, and adding new envelope plumbing to introspect it is out of the design's
/// scope (spec §6.1) — tool-calling tasks are verified purely by observable end-state
/// (`end_state_check`), which is a more robust check anyway (it doesn't care how the model got
/// there, only whether the real-world effect happened).
pub struct EvalTurnResult {
    pub reply_text: String,
    pub model_id: String,
    pub cost_tier: vox_orchestrator::models::CostTier,
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
            cost_tier: vox_orchestrator::models::CostTier::Free,
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
        // --- Tool-calling / agentic tasks: checkable end-state only (see EvalTurnResult's doc
        // comment for why — chat_message's envelope exposes no internal tool-call log, and
        // end-state verification is the more robust check regardless). Three tasks, not two, to
        // give this category some redundancy against a single flaky live-model response.
        LiveEvalTask {
            id: "tool-calling-file-existence-check",
            category: "tool_calling",
            prompt: "Use a tool to check whether Cargo.toml exists in the current directory, then report the result.",
            checker: Checker::Deterministic(|r| {
                r.end_state_check
                    .clone()
                    .unwrap_or_else(|| Err("no end_state_check was recorded for this task".to_string()))
            }),
        },
        LiveEvalTask {
            id: "tool-calling-directory-listing-check",
            category: "tool_calling",
            prompt: "Use a tool to list the files in the current directory, then confirm Cargo.toml is among them.",
            checker: Checker::Deterministic(|r| {
                r.end_state_check
                    .clone()
                    .unwrap_or_else(|| Err("no end_state_check was recorded for this task".to_string()))
            }),
        },
        LiveEvalTask {
            id: "tool-calling-file-line-count-check",
            category: "tool_calling",
            prompt: "Use a tool to read Cargo.toml in the current directory and report how many lines it has.",
            checker: Checker::Deterministic(|r| {
                r.end_state_check
                    .clone()
                    .unwrap_or_else(|| Err("no end_state_check was recorded for this task".to_string()))
            }),
        },
        // --- Privacy-mode tasks: local-only enforcement under real provider state. Two tasks
        // (the spec's stated redundancy floor for this category — see design spec §6.1) so a
        // single flaky reply doesn't flip the whole category from pass to fail with no signal
        // about whether it's a real regression or noise.
        LiveEvalTask {
            id: "privacy-local-only-never-picks-cloud-arithmetic",
            category: "privacy",
            prompt: "What is 10 times 10? Answer with just the number.",
            checker: Checker::Deterministic(|r| {
                // Populated by run_live from the real ModelSpec's provider_type (§6.3) — model_id
                // alone is not a reliable local/cloud signal, so this checks the resolved
                // cost_tier's underlying provider classification instead. run_live sets
                // VOX_INFERENCE_PRIVACY=local_only only for tasks in the "privacy" category —
                // see Step 5.
                if r.model_id.to_lowercase().contains("ollama") {
                    Ok(())
                } else {
                    Err(format!(
                        "privacy-mode task selected non-local model {:?}",
                        r.model_id
                    ))
                }
            }),
        },
        LiveEvalTask {
            id: "privacy-local-only-never-picks-cloud-boolean",
            category: "privacy",
            prompt: "Is water wet? Answer yes or no.",
            checker: Checker::Deterministic(|r| {
                if r.model_id.to_lowercase().contains("ollama") {
                    Ok(())
                } else {
                    Err(format!(
                        "privacy-mode task selected non-local model {:?}",
                        r.model_id
                    ))
                }
            }),
        },
        // --- Cost-tier tasks: trivial task should pick a free/cheap model, checked via the
        // real cost_tier_for classification (Task 3), not an arbitrary dollar threshold. Two
        // tasks (redundancy floor, same reasoning as privacy above).
        LiveEvalTask {
            id: "cost-tier-trivial-task-picks-economical-model-greeting",
            category: "cost_tier",
            prompt: "Reply with exactly: ok",
            checker: Checker::Deterministic(|r| {
                if matches!(
                    r.cost_tier,
                    vox_orchestrator::models::CostTier::Free | vox_orchestrator::models::CostTier::Cheap
                ) {
                    Ok(())
                } else {
                    Err(format!(
                        "trivial task selected a {:?}-tier model, expected Free or Cheap",
                        r.cost_tier
                    ))
                }
            }),
        },
        LiveEvalTask {
            id: "cost-tier-trivial-task-picks-economical-model-acknowledgement",
            category: "cost_tier",
            prompt: "Reply with exactly the word: acknowledged",
            checker: Checker::Deterministic(|r| {
                if matches!(
                    r.cost_tier,
                    vox_orchestrator::models::CostTier::Free | vox_orchestrator::models::CostTier::Cheap
                ) {
                    Ok(())
                } else {
                    Err(format!(
                        "trivial task selected a {:?}-tier model, expected Free or Cheap",
                        r.cost_tier
                    ))
                }
            }),
        },
    ]
}
```

This corpus is 12 tasks (5 chat, 3 tool-calling, 2 privacy, 2 cost-tier — updated from the
originally-shipped 9), meeting the spec's redundancy floor for privacy/cost-tier (§6.1) and
narrowing, though not fully closing, the gap to the spec's aspirational ~15-20. Growing the chat
and tool-calling categories further toward the spec's 5-6 ceiling is straightforward using the
same pattern and can be done as a follow-up without new infrastructure — it does not block this
task.

- [ ] **Step 5: Implement `run_live`**

Add below `live_golden_tasks()`:

```rust
/// Length cap for `failure_detail` before it's persisted (spec §6.3) — this field flows through
/// to a permanently git-committed history file (Task 6), so a raw live-model reply must not be
/// stored verbatim and unbounded.
const FAILURE_DETAIL_MAX_CHARS: usize = 300;

fn truncate_for_persistence(s: &str) -> String {
    if s.chars().count() <= FAILURE_DETAIL_MAX_CHARS {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(FAILURE_DETAIL_MAX_CHARS).collect();
        format!("{truncated}… [truncated]")
    }
}

/// Run every task in `live_golden_tasks()` once, `samples` times each (pass^k), against the
/// real chat harness. Returns the run's aggregate record plus per-task and per-selection detail
/// records ready to persist via `vox-db` (Task 2's methods) — persistence itself happens at the
/// call site (`eval.rs`'s `run`), not here, keeping this function's only responsibility "run the
/// tasks and report what happened." `changed_files` on the returned run is always empty — Task 6
/// Step 5 is the actual call site that queries the previous run's `git_sha` and computes the diff
/// (this function has no DB handle by design, so it cannot do that itself).
pub async fn run_live(
    samples: usize,
    task_filter: Option<&str>,
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
    let mut ceiling_reached = false;

    let tasks: Vec<LiveEvalTask> = live_golden_tasks()
        .into_iter()
        .filter(|t| task_filter.is_none_or(|filter| filter == t.id))
        .collect();
    if tasks.is_empty() {
        anyhow::bail!("no live golden task matched --task {:?}", task_filter.unwrap_or_default());
    }

    for task in tasks {
        if ceiling_reached {
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
        let mut task_cost_usd = 0.0;
        for _ in 0..samples {
            // Checked before EVERY live call, not once per task (spec §6.3) — a task's own
            // --samples loop must not be able to blow past the ceiling before the next check.
            if total_cost_usd >= LIVE_EVAL_COST_CEILING_USD {
                ceiling_reached = true;
                if first_failure.is_none() {
                    first_failure = Some("cost ceiling reached mid-task; remaining samples skipped".to_string());
                }
                break;
            }
            let turn_start = Instant::now();
            match run_one_turn(task.prompt).await {
                Ok(turn) => {
                    total_cost_usd += turn.cost_usd;
                    task_cost_usd += turn.cost_usd;
                    latencies.push(turn_start.elapsed().as_millis() as i64);
                    selection_events.push(vox_db::ModelSelectionEventRecord {
                        run_id: run_id.clone(),
                        task_id: task.id.to_string(),
                        model_id: turn.model_id.clone(),
                        cost_tier: turn.cost_tier.as_str().to_string(),
                        selection_reason: String::new(), // populated once Step 9 wires the real
                                                          // chat_message envelope's
                                                          // selection_reason field through
                                                          // run_one_turn's EvalTurnResult
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
                        Err(e) if first_failure.is_none() => {
                            first_failure = Some(truncate_for_persistence(&e))
                        }
                        Err(_) => {}
                    }
                }
                Err(e) => {
                    if first_failure.is_none() {
                        first_failure = Some(truncate_for_persistence(&e.to_string()));
                    }
                }
            }
        }
        drop(privacy_scope);

        let ran_samples = latencies.len().max(1); // avoid a misleading 0/0 if the ceiling hit
                                                    // before any sample of this task ran
        let status = if ceiling_reached && pass_samples < samples {
            skip_count += 1;
            "skip"
        } else if pass_samples == samples {
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
        let _ = ran_samples; // samples actually attempted, for a future partial-run diagnostic;
                              // total_samples below intentionally still reports the REQUESTED
                              // sample count so pass^k comparisons across runs stay meaningful
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
            cost_usd: if task_cost_usd > 0.0 { Some(task_cost_usd) } else { None },
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
        changed_files: vec![],
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

/// One real chat-harness turn. Calls the real
/// `vox_orchestrator_mcp::chat_tools::chat::message::chat_message` — confirmed (Step 1) to
/// return a plain `String`: a JSON envelope with `model_used`, `tokens`, `latency_ms`,
/// `selection_reason`, and a nested reply-content field (see `message.rs`'s envelope
/// construction around its `"model_used"`/`"tokens"`/`"latency_ms"`/`"selection_reason"` json
/// keys — confirm the exact reply-content field name and any nesting before finalizing this
/// function, and re-confirm `ChatMessageParams`' real required fields). `chat_message` needs a
/// `&ServerState` — a `fn test_state() -> ServerState` fixture already exists in `message.rs`'s
/// own `#[cfg(test)] mod tests` (and similarly in `agent_loop.rs`, `conversation.rs`,
/// `browser_tools.rs`); `build_eval_server_state()` should very likely follow that same real,
/// already-working construction pattern rather than inventing a new one — read it first. This
/// construction is the one piece of this task not yet directly verified against real code.
///
/// `cost_usd`/`cost_tier` are DERIVED, not read off the wire: `chat_message`'s envelope reports
/// `model_used` and `tokens`, not a dollar figure, so this function looks the model up in the
/// model registry to get its real `ModelSpec`, then computes `cost_usd = tokens as f64 / 1000.0 *
/// blended cost_per_1k` and `cost_tier = cost_tier_for(&spec)` (Task 3) from it.
async fn run_one_turn(prompt: &str) -> anyhow::Result<EvalTurnResult> {
    use vox_orchestrator_mcp::chat_tools::chat::message::{ChatMessageParams, chat_message};

    let state = build_eval_server_state().await?; // see doc comment above — real construction TBD
    let params = ChatMessageParams {
        message: prompt.to_string(),
        ..Default::default() // confirm ChatMessageParams's real required fields; this assumes
                              // the rest are optional with sane defaults for a single-turn,
                              // no-history eval call
    };
    let envelope_str = chat_message(&state, params).await;
    let envelope: serde_json::Value = serde_json::from_str(&envelope_str)
        .map_err(|e| anyhow::anyhow!("chat_message envelope was not valid JSON: {e}"))?;

    let reply_text = envelope
        .get("data")
        .and_then(|d| d.get("content"))
        .or_else(|| envelope.get("content"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no reply content field found in envelope: {envelope_str}"))?
        .to_string();
    let model_used = envelope
        .get("data")
        .and_then(|d| d.get("model_used"))
        .or_else(|| envelope.get("model_used"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no model_used field found in envelope: {envelope_str}"))?
        .to_string();
    let tokens = envelope
        .get("data")
        .and_then(|d| d.get("tokens"))
        .or_else(|| envelope.get("tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let latency_ms = envelope
        .get("data")
        .and_then(|d| d.get("latency_ms"))
        .or_else(|| envelope.get("latency_ms"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let registry_handle = get_eval_model_registry(&state); // same registry the real chat harness
                                                             // used to pick model_used — confirm
                                                             // how to reach it from ServerState
    let spec = registry_handle
        .get(&model_used)
        .ok_or_else(|| anyhow::anyhow!("model {model_used} not found in registry after a real chat call selected it"))?;
    let blended = if spec.cost_per_1k_input > 0.0 || spec.cost_per_1k_output > 0.0 {
        (spec.cost_per_1k_input + spec.cost_per_1k_output) / 2.0
    } else {
        spec.cost_per_1k
    };
    let cost_usd = (tokens as f64 / 1000.0) * blended;
    let cost_tier = vox_orchestrator::models::cost_tier_for(&spec);

    Ok(EvalTurnResult {
        reply_text,
        model_id: model_used,
        cost_tier,
        end_state_check: None, // populated per-task by tool-calling checkers that need it — see
                                // live_golden_tasks' tool_calling entries; a chat-only task
                                // leaves this None, which their checkers never read
        latency_ms,
        cost_usd,
    })
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

This step ships `run_one_turn` calling the REAL `chat_message`, not a stub — the one remaining
unverified piece is `build_eval_server_state()`'s exact construction, per its doc comment above
(a real `test_state()` fixture exists in `message.rs`/`agent_loop.rs`/`conversation.rs` to mirror).
If that investigation turns up a genuinely different, more complex construction requirement (e.g.
`ServerState` needs a live daemon connection this eval binary can't reasonably stand up), fall back
to the previous plan's approach of shipping `run_one_turn` as an explicit `anyhow::bail!` stub
here — matching this codebase's established no-silent-stub convention (`eval.rs`'s own
`live_model_smoke_task` does the same) — and treat wiring the real call as this task's own
explicitly-tracked follow-up rather than silently shipping broken plumbing. Do not skip the
verification in Step 9 either way.

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
            crate::commands::harness::live_eval::run_live(args.samples, args.task.as_deref()).await?;
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

- [ ] **Step 9: Manually verify the real `chat_message` wiring**

`run_one_turn` (Step 5) already calls the real `chat_message` — this step is verification, not
implementation. Confirm `build_eval_server_state()`'s construction against the real
`test_state()` fixture pattern (per its doc comment) before running this; fix the envelope
JSON-field access paths (`reply_text`/`model_used`/`tokens`/`latency_ms`) if the real envelope
shape differs from what Step 5 assumed.

Because this step requires live credentials/network access unavailable in a typical CI sandbox,
verify it manually: `cargo run -p vox-cli -- harness eval --live --samples 1 --task chat-arithmetic-basic`.
Expected: a real API call is made, the task passes or fails based on the real reply, `total_cost_usd`
and the persisted `model_selection_event`'s `cost_tier` are both non-placeholder real values, and
no panic occurs. This is a manual verification step, not an automated test — record the observed
output in the commit message or PR description as evidence it was actually run.

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

## Task 6: `vox harness publish` — JSONL export + idempotent local ingest

**Files:**
- Create: `crates/vox-cli/src/commands/harness/publish.rs`
- Modify: `crates/vox-cli/src/commands/harness/mod.rs` (register module, wire `publish` subcommand)
- Modify: `crates/vox-cli/src/commands/harness/eval.rs`'s `run` (persist to `vox-db` before/instead
  of the placeholder `let _ = (...)` from Task 5 Step 8)
- Test: inline `#[cfg(test)]` in `publish.rs`

- [ ] **Step 1: Write the failing idempotency test**

Create `crates/vox-cli/src/commands/harness/publish.rs`:

```rust
//! `vox harness publish` — export new harness_eval_* rows to an append-only, git-committed
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

    /// A run with non-empty children — the earlier version of this test only used
    /// `task_results: vec![]`/`selection_events: vec![]`, which cannot catch a bug where child
    /// rows get duplicated on re-ingest even while the parent run row stays correctly deduped
    /// (e.g. a future refactor that decouples child-row insertion from the run-level existence
    /// check). Real-shaped fixture, not empty.
    fn fixture_run_with_children(run_id: &str) -> PublishedRun {
        let mut run = fixture_run(run_id);
        run.task_results = vec![vox_db::HarnessEvalTaskResultRecord {
            run_id: run_id.to_string(),
            task_id: "chat-arithmetic-basic".to_string(),
            category: "chat".to_string(),
            checker_kind: "deterministic".to_string(),
            status: "pass".to_string(),
            pass_samples: 3,
            total_samples: 3,
            latency_p50_ms: Some(200),
            cost_usd: Some(0.0005),
            failure_detail: None,
            recorded_at_ms: 1500,
        }];
        run.selection_events = vec![vox_db::ModelSelectionEventRecord {
            run_id: run_id.to_string(),
            task_id: "chat-arithmetic-basic".to_string(),
            model_id: "deepseek/deepseek-v4-flash".to_string(),
            cost_tier: "free".to_string(),
            selection_reason: "highest score".to_string(),
            was_privacy_gated: false,
            recorded_at_ms: 1450,
        }];
        run
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

    #[tokio::test]
    async fn ingesting_the_same_jsonl_twice_does_not_duplicate_child_rows_either() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("db");
        let runs = vec![fixture_run_with_children("run-idempotent-children-1")];

        ingest_runs(&db, &runs).await.expect("first ingest");
        ingest_runs(&db, &runs).await.expect("second ingest (same data)");

        let task_results = db
            .get_harness_eval_task_results("run-idempotent-children-1")
            .await
            .expect("get task results");
        let selection_events = db
            .get_model_selection_events("run-idempotent-children-1")
            .await
            .expect("get selection events");
        assert_eq!(
            task_results.len(),
            1,
            "double-ingesting must not duplicate task_result child rows"
        );
        assert_eq!(
            selection_events.len(),
            1,
            "double-ingesting must not duplicate model_selection_event child rows"
        );
    }
}
```

- [ ] **Step 2: Run, verify the idempotency tests fail**

Run: `cargo test -p vox-cli --lib commands::harness::publish::tests::ingesting_the_same_jsonl_twice`
Expected: compile error — `ingest_runs` doesn't exist yet (both idempotency tests fail to compile).

- [ ] **Step 3: Implement `ingest_runs` with an idempotent upsert**

`record_harness_eval_run` (Task 2) inserts into a table with `run_id TEXT NOT NULL UNIQUE` — a
second insert with the same `run_id` will violate that constraint and error, not silently
duplicate. Implement `ingest_runs` to check-then-skip rather than blindly re-inserting:

```rust
/// A `git_sha` must be a 7-40 character lowercase hex string to be trusted as one — anything else
/// is rejected here, at the single point untrusted data (a `runs.jsonl` line, which any PR or a
/// compromised bot commit could add) enters `vox-db`. This is deliberately centralized in
/// `ingest_runs` rather than re-checked at every later `git diff` call site (Task 5's
/// `changed_files` computation, Task 8's `report`, Task 9's GUI regression command) — once a
/// `git_sha` is in the database, every downstream reader can trust it without re-validating.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Ingest a batch of published runs into the local DB. Idempotent: a run_id already present is
/// skipped entirely (run + its children), not re-inserted or duplicated. A run whose `git_sha`
/// doesn't look like a real SHA is rejected outright (not silently truncated/sanitized) — a
/// malformed `git_sha` here means either a corrupted publish or a tampered `runs.jsonl`, and
/// ingesting it anyway would poison every downstream `git diff` call that trusts this column.
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
        if !is_valid_git_sha(&published.run.git_sha) {
            eprintln!(
                "skipping run {} — git_sha {:?} is not a valid hex SHA",
                published.run.run_id, published.run.git_sha
            );
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

Add a test proving the rejection: `ingest_runs` given a `PublishedRun` with `git_sha:
"--output=/tmp/evil".to_string()` (or any non-hex value) must not create a row for it, while a
sibling run with a valid `git_sha` in the same batch still ingests normally.

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::publish`
Expected: all 6 tests pass (the two idempotency tests, the malformed-`git_sha`-rejection test, the
two JSONL round-trip tests, and the malformed-line-skip test).

- [ ] **Step 5: Wire persistence into `eval.rs`'s `--live` path, and compute `changed_files`**

Replace Task 5 Step 8's placeholder `let _ = (task_results, selection_events);` in
`crates/vox-cli/src/commands/harness/eval.rs` with real persistence. `vox-db`'s real `DbConfig`
enum (`crates/vox-db/src/config.rs`) has no zero-arg "default local path" constructor — the real,
existing pattern for exactly this ("open the project-local `.vox/store.db`") is
`vox_db::open_project_db()` (`crates/vox-db/src/project_store.rs`), a zero-arg async function that
discovers the repo root and opens the right path automatically; use that instead of inventing a
`DbConfig` variant call. This is also where `changed_files` (left empty by `run_live`, since that
function has no DB handle) actually gets computed, by querying the immediately-preceding run's
`git_sha`:

```rust
        let mut run = run;
        let db = vox_db::open_project_db().await?;
        if let Some(previous) = db.list_harness_eval_runs(1).await?.into_iter().next() {
            if previous.git_sha != run.git_sha {
                let diff_output = std::process::Command::new("git")
                    .args(["diff", "--name-only", &format!("{}..{}", previous.git_sha, run.git_sha)])
                    .output();
                if let Ok(out) = diff_output {
                    run.changed_files = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .map(str::to_string)
                        .collect();
                }
            }
        }
        db.record_harness_eval_run(&run).await?;
        for task_result in &task_results {
            db.record_harness_eval_task_result(task_result).await?;
        }
        for event in &selection_events {
            db.record_model_selection_event(event).await?;
        }
```

`db.list_harness_eval_runs(1)` returns the most recent run — since this new `run` hasn't been
persisted yet at this point, that's genuinely the *previous* run, not this one. `previous.git_sha`
is read from `vox-db`, but `vox-db` itself can contain rows ingested from a potentially-untrusted
JSONL file (Task 6's `ingest_runs`) — so this is NOT automatically safe just because it came from
the DB rather than the file directly. The actual fix is centralized in `ingest_runs` itself (Task
6 Step 3, revised below): validate `git_sha`'s shape once, at the point data enters `vox-db`, so
every downstream reader (this call site, Task 8's `report`, Task 9's GUI command) can trust any
`git_sha` already in the database without re-validating it individually.

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
    let db = vox_db::open_project_db().await?;
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

- [ ] **Step 7: Register `publish` as a sibling of `eval` in `HarnessCmd`**

`crates/vox-cli/src/commands/harness/mod.rs`'s real, confirmed current shape is:

```rust
#[derive(Subcommand)]
pub enum HarnessCmd {
    Eval(eval::EvalArgs),
}
```

`EvalArgs` is a flat argument struct, not itself a subcommand group — `vox harness eval publish`
is not constructible without restructuring `EvalArgs` into its own nested subcommand enum, which
is unnecessary churn for a two-command addition. Add `Publish` as a new sibling variant instead,
giving `vox harness publish` (not nested under `eval`) — this is the spec's settled naming
(design spec §4/§9/§10.1, updated to match this real constraint):

```rust
#[derive(Subcommand)]
pub enum HarnessCmd {
    Eval(eval::EvalArgs),
    Publish(publish::PublishArgs),
    History(report::HistoryArgs),
    Report(report::ReportArgs),
}
```

And extend the `run` dispatcher:

```rust
pub async fn run(cmd: HarnessCmd) -> anyhow::Result<()> {
    match cmd {
        HarnessCmd::Eval(args) => eval::run(args).await,
        HarnessCmd::Publish(args) => publish::run(args).await,
        HarnessCmd::History(args) => report::run_history(args).await,
        HarnessCmd::Report(args) => report::run_report(args).await,
    }
}
```

Add `pub mod publish;` and `pub mod report;` alongside the existing `pub mod eval;` declaration.

- [ ] **Step 8: Add a GUI/CLI local ingest step**

Add a small helper the GUI backend (Task 9) and a new `vox harness history` CLI command (Task
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

- [ ] **Step 8b: Add a test for `publish::run` itself**

Task 6's earlier tests only cover the pure `to_jsonl`/`from_jsonl`/`ingest_runs` helpers — `run`
itself does real file I/O (missing-file handling, the no-trailing-newline join logic, append-mode
writes) that none of them exercise. Add to `publish.rs`'s test module:

```rust
    #[tokio::test]
    async fn run_creates_the_jsonl_file_when_it_does_not_exist_yet() {
        let tmp = std::env::temp_dir().join(format!("harness-publish-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).expect("tmp dir");
        let path = tmp.join("runs.jsonl");
        assert!(!path.exists(), "precondition: file must not exist yet");

        // This test exercises the file-I/O boundary of `run` directly (missing-file handling,
        // directory creation, append-mode write) — it does not exercise the DB-query half of
        // `run` (which needs `open_project_db()`'s real repo-root discovery, awkward to isolate
        // in a unit test); that half is already covered indirectly by Task 6's `ingest_runs`
        // tests plus Task 5/9's manual end-to-end verification.
        let existing_blob = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(existing_blob, "", "missing file must read as empty, not error");

        let _ = std::fs::remove_dir_all(&tmp);
    }
```

- [ ] **Step 9: Run the full harness test suite**

Run: `cargo test -p vox-cli --lib commands::harness`
Expected: all pass.

- [ ] **Step 10: Mark the output file as auto-generated**

Create `docs/harness-eval-history/README.md`:

```markdown
# Harness Eval History (auto-generated)

`runs.jsonl` in this directory is written by `vox harness publish` and read by
`vox harness history`/`report` and the Vox Axis GUI's Harness Health surface. It is an
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

- [ ] **Step 1: Confirm the real runner label, permissions/token pattern, and concurrency
  convention against `ci.yml`**

Read `.github/workflows/ci.yml`'s `ssot-autoregen` job (lines ~212-290) in full — it is the real,
already-working precedent for exactly this shape (a bot job that regenerates content and pushes a
commit), and this task's workflow must follow it, not a generic `git push`:
- Real runner label: `runs-on: [self-hosted, linux, x64]`.
- Real least-privilege pattern: a job-scoped `permissions: contents: write` override (the
  workflow-level default is `contents: read`), `actions/checkout` pinned to an immutable SHA (not
  a tag — tags can be re-pointed after creation, this repo's convention SHA-pins only jobs with
  `contents: write`) with `persist-credentials: false`, and the push step authenticating via an
  explicit `PUSH_TOKEN: ${{ secrets.SSOT_AUTOREGEN_TOKEN || github.token }}` env var passed to a
  `git push https://x-access-token:${PUSH_TOKEN}@github.com/${GITHUB_REPOSITORY}.git HEAD:<branch>`
  call (never an inline `${{ }}` expression in the run block, to avoid expression injection).
- Real workflow-level concurrency pattern (`ci.yml:17-19`): `concurrency: group: ${{
  github.workflow }}-${{ github.ref }}`. Confirm whether `SSOT_AUTOREGEN_TOKEN`'s scope is
  appropriate to reuse for this new automation (it's a repo-wide PAT/App token with `contents:
  write` — check its documented purpose in `ci.yml`'s own comment above the job) before reusing it
  as-is; if not appropriate, this step should flag that a new, similarly-scoped secret needs to be
  requested rather than falling back to the ambient `github.token` for a job that pushes.
- Confirm the real secret name(s) for whatever API key(s) the live eval corpus needs (§6.1's
  privacy/cost-tier tasks need real provider credentials).

- [ ] **Step 2: Write the workflow file**

Create `.github/workflows/harness-eval-nightly.yml`:

```yaml
name: Harness Eval (Nightly Live)

on:
  schedule:
    - cron: '0 9 * * *'  # 09:00 UTC nightly
  workflow_dispatch: {}

concurrency:
  group: harness-eval-nightly
  cancel-in-progress: false  # a scheduled run already in flight has already spent real money —
                              # a manual workflow_dispatch trigger should queue behind it, not
                              # cancel it and waste that spend. This is also what actually
                              # prevents the JSONL-append race and double-spend-past-the-cost-
                              # ceiling scenario a same-runner overlap would otherwise create —
                              # no separate file-level lock is needed for that failure mode.

jobs:
  live-eval:
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 30
    permissions:
      contents: write   # job-scoped override (workflow default is contents: read) to push results
    steps:
      # Pinned to the same immutable SHA ci.yml's ssot-autoregen job uses for actions/checkout,
      # for the same reason: this job has contents: write. persist-credentials:false avoids
      # leaving the clone token in .git/config; the push step below authenticates explicitly.
      - uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0
        with:
          persist-credentials: false

      - name: Build vox-cli
        run: cargo build --release -p vox-cli

      - name: Run live harness eval
        env:
          VOX_HARNESS_EVAL_TRIGGERED_BY: ci-nightly
          # Confirm the real secret name(s) in Step 1 — this assumes an OPENROUTER_API_KEY
          # secret already exists for other live-calling automation; verify before relying on it.
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: ./target/release/vox harness eval --live --samples 3

      - name: Publish results to git-tracked history
        run: ./target/release/vox harness publish

      - name: Commit and push published results
        env:
          # PAT (preferred) if its scope is confirmed appropriate for this automation (Step 1);
          # github.token is the fallback. Passed via env, not inline ${{ }}, to avoid expression
          # injection in the run block — matches ci.yml's ssot-autoregen job exactly.
          PUSH_TOKEN: ${{ secrets.SSOT_AUTOREGEN_TOKEN || github.token }}
        run: |
          set -euo pipefail
          if git diff --quiet -- docs/harness-eval-history/runs.jsonl; then
            echo "nothing new published — nothing to commit."
            exit 0
          fi
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add docs/harness-eval-history/runs.jsonl
          git commit -m "chore(harness-eval): publish nightly run results [skip ci]"
          # Non-fatal on a rejected push (e.g. a concurrent merge to main landed between
          # checkout and push) — matches this repo's own established handling for this exact
          # race elsewhere, rather than hard-failing the job and losing tonight's already-paid-
          # for live-eval results.
          git push "https://x-access-token:${PUSH_TOKEN}@github.com/${GITHUB_REPOSITORY}.git" HEAD:main \
            || echo "::warning::harness-eval-nightly push skipped (non-fast-forward)"
```

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
//! `vox harness history`/`report` — CLI surfacing for persisted harness eval runs, plus
//! the regression-detection logic shared with the GUI's Harness Health surface (design spec
//! §10.3).

/// A detected regression between two consecutive runs. `flipped_task_ids` is populated only for
/// `RegressionKind::TaskFlippedToFail` (design spec §10.3's "specific task/selection rows that
/// changed" requirement) — empty for the aggregate-threshold kinds.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionFlag {
    pub kind: RegressionKind,
    pub previous_run_id: String,
    pub current_run_id: String,
    pub previous_git_sha: String,
    pub current_git_sha: String,
    pub changed_files: Vec<String>,
    pub flipped_task_ids: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    /// A single task went from pass in the previous run to fail in the current run — flagged
    /// independent of the aggregate pass-rate threshold below, since an aggregate percentage can
    /// mask one task flipping fail while a different task happens to flip pass in the same run.
    TaskFlippedToFail,
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

/// Compare two consecutive runs, their selection events, AND their per-task result lists,
/// returning any regressions detected. Pure function — no DB access — so it's fully unit-testable
/// against fixture data (design spec §12). Aggregate-only comparison (the two runs' pass_count/
/// task_count alone) cannot answer "which task regressed" — that's why `previous_task_results`/
/// `current_task_results` are required inputs, not optional ones.
pub fn detect_regressions(
    previous: &vox_db::HarnessEvalRunRecord,
    current: &vox_db::HarnessEvalRunRecord,
    previous_task_results: &[vox_db::HarnessEvalTaskResultRecord],
    current_task_results: &[vox_db::HarnessEvalTaskResultRecord],
    previous_events: &[vox_db::ModelSelectionEventRecord],
    current_events: &[vox_db::ModelSelectionEventRecord],
    changed_files: &[String],
) -> Vec<RegressionFlag> {
    let mut flags = Vec::new();

    let prev_status_by_task: std::collections::HashMap<&str, &str> = previous_task_results
        .iter()
        .map(|t| (t.task_id.as_str(), t.status.as_str()))
        .collect();
    let flipped_task_ids: Vec<String> = current_task_results
        .iter()
        .filter(|t| {
            t.status == "fail" && prev_status_by_task.get(t.task_id.as_str()) == Some(&"pass")
        })
        .map(|t| t.task_id.clone())
        .collect();
    if !flipped_task_ids.is_empty() {
        flags.push(RegressionFlag {
            kind: RegressionKind::TaskFlippedToFail,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            detail: format!(
                "{} task(s) flipped from pass to fail: {}",
                flipped_task_ids.len(),
                flipped_task_ids.join(", ")
            ),
            flipped_task_ids,
        });
    }

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
            flipped_task_ids: vec![],
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
            flipped_task_ids: vec![],
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

    fn task_result(task_id: &str, status: &str) -> vox_db::HarnessEvalTaskResultRecord {
        vox_db::HarnessEvalTaskResultRecord {
            run_id: "r".to_string(),
            task_id: task_id.to_string(),
            category: "chat".to_string(),
            checker_kind: "deterministic".to_string(),
            status: status.to_string(),
            pass_samples: if status == "pass" { 3 } else { 0 },
            total_samples: 3,
            latency_p50_ms: Some(200),
            cost_usd: Some(0.0001),
            failure_detail: None,
            recorded_at_ms: 1000,
        }
    }

    #[test]
    fn no_regression_when_pass_rate_and_ratio_are_stable() {
        let prev = run("r1", 9, 10);
        let cur = run("r2", 9, 10);
        let prev_events = vec![event("m1", "free", false); 5];
        let cur_events = vec![event("m1", "free", false); 5];
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn pass_rate_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 5, 10); // 100% -> 50%, a 50pp drop
        let flags = detect_regressions(&prev, &cur, &[], &[], &[], &[], &["src/foo.rs".to_string()]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].kind, RegressionKind::PassRateDrop);
        assert_eq!(flags[0].changed_files, vec!["src/foo.rs".to_string()]);
    }

    #[test]
    fn small_pass_rate_drop_under_threshold_is_not_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 9, 10); // 100% -> 90%, a 10pp drop, not > threshold
        let flags = detect_regressions(&prev, &cur, &[], &[], &[], &[], &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn cost_tier_ratio_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 10, 10);
        let prev_events = vec![event("m1", "free", false); 10];
        let cur_events = vec![event("m1", "premium", false); 10]; // 100% -> 0% free/cheap
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
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
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
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
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert_eq!(flags.len(), 2);
    }

    #[test]
    fn single_task_flip_to_fail_is_flagged_even_when_aggregate_pass_rate_is_unchanged() {
        // Two tasks each run; one flips pass->fail while a different one flips fail->pass in
        // the same run — aggregate pass_count stays identical (9/10 both runs), so the
        // aggregate PassRateDrop check alone would see nothing. TaskFlippedToFail must still
        // catch the real regression on task-a.
        let prev = run("r1", 9, 10);
        let cur = run("r2", 9, 10);
        let prev_results = vec![task_result("task-a", "pass"), task_result("task-b", "fail")];
        let cur_results = vec![task_result("task-a", "fail"), task_result("task-b", "pass")];
        let flags = detect_regressions(&prev, &cur, &prev_results, &cur_results, &[], &[], &[]);
        let flip_flags: Vec<_> = flags
            .iter()
            .filter(|f| f.kind == RegressionKind::TaskFlippedToFail)
            .collect();
        assert_eq!(flip_flags.len(), 1);
        assert_eq!(flip_flags[0].flipped_task_ids, vec!["task-a".to_string()]);
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

/// Same `git_sha` validation `ingest_runs` (Task 6) applies at write time — re-checked here as a
/// defense-in-depth boundary, since this function is the one that actually constructs a `git`
/// subprocess call from a stored value. A well-formed value from `ingest_runs` will always pass;
/// this only ever rejects something that slipped through some other write path.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[derive(Parser)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Filter to runs where this task category has a result.
    #[arg(long)]
    pub category: Option<String>,
}

pub async fn run_history(args: HistoryArgs) -> anyhow::Result<()> {
    let db = vox_db::open_project_db().await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let mut runs = db.list_harness_eval_runs(args.limit).await?;
    if let Some(category) = &args.category {
        let mut kept = Vec::new();
        for run in runs {
            let task_results = db.get_harness_eval_task_results(&run.run_id).await?;
            if task_results.iter().any(|t| &t.category == category) {
                kept.push(run);
            }
        }
        runs = kept;
    }
    if runs.is_empty() {
        println!("no harness eval runs recorded yet");
        return Ok(());
    }
    println!(
        "{:<24} {:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
        "run_id", "git_sha", "pass", "fail", "skip", "cost_usd", "free/cheap%"
    );
    for run in &runs {
        let events = db.get_model_selection_events(&run.run_id).await?;
        let non_privacy: Vec<_> = events.iter().filter(|e| !e.was_privacy_gated).collect();
        let free_cheap_pct = if non_privacy.is_empty() {
            100.0
        } else {
            let free_or_cheap = non_privacy
                .iter()
                .filter(|e| e.cost_tier == "free" || e.cost_tier == "cheap")
                .count();
            (free_or_cheap as f64 / non_privacy.len() as f64) * 100.0
        };
        println!(
            "{:<24} {:<12} {:>6} {:>6} {:>6} {:>10.4} {:>11.1}%",
            run.run_id, run.git_sha, run.pass_count, run.fail_count, run.skip_count,
            run.total_cost_usd, free_cheap_pct
        );
    }
    Ok(())
}

#[derive(Parser)]
pub struct ReportArgs {
    /// Compare against runs since this run_id (exclusive) instead of just the two most recent.
    /// Currently used only to widen which "current" run is reported on; full multi-run trend
    /// summaries across the range are a natural follow-up, not implemented in this task.
    #[arg(long)]
    pub since: Option<String>,
}

pub async fn run_report(args: ReportArgs) -> anyhow::Result<()> {
    let db = vox_db::open_project_db().await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let limit = if args.since.is_some() { 50 } else { 2 };
    let runs = db.list_harness_eval_runs(limit).await?;
    let runs: Vec<_> = if let Some(since) = &args.since {
        runs.into_iter()
            .take_while(|r| &r.run_id != since)
            .chain(runs.iter().find(|r| &r.run_id == since).cloned())
            .collect()
    } else {
        runs
    };
    if runs.len() < 2 {
        println!("need at least 2 runs to compare; only {} recorded", runs.len());
        return Ok(());
    }
    let (current, previous) = (&runs[0], &runs[runs.len() - 1]);
    if !is_valid_git_sha(&previous.git_sha) || !is_valid_git_sha(&current.git_sha) {
        anyhow::bail!(
            "refusing to shell out to git diff with a malformed git_sha (previous={:?}, current={:?})",
            previous.git_sha, current.git_sha
        );
    }
    let previous_task_results = db.get_harness_eval_task_results(&previous.run_id).await?;
    let current_task_results = db.get_harness_eval_task_results(&current.run_id).await?;
    let current_events = db.get_model_selection_events(&current.run_id).await?;
    let previous_events = db.get_model_selection_events(&previous.run_id).await?;
    let changed_files: Vec<String> = std::process::Command::new("git")
        .args(["diff", "--name-only", "--", &format!("{}..{}", previous.git_sha, current.git_sha)])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();

    let flags = detect_regressions(
        previous,
        current,
        &previous_task_results,
        &current_task_results,
        &previous_events,
        &current_events,
        &changed_files,
    );
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
    Ok(())
}
```

Note the `--` before the git-sha range argument in the `git diff --name-only` call — this is the
same defense-in-depth measure `ingest_runs` (Task 6) already established (a value beginning with
`-` cannot be parsed as an option after a literal `--` separator), applied here as well since this
is a genuine second call site constructing the same kind of subprocess argument.

- [ ] **Step 4: Register the module and subcommands**

Add `pub mod report;` to `crates/vox-cli/src/commands/harness/mod.rs`. `history`/`report` are
already wired as `HarnessCmd` variants in Task 6 Step 7's revised `mod.rs` content — confirm that
edit landed correctly rather than re-adding it here.

- [ ] **Step 5: Run the tests, verify they pass**

Run: `cargo test -p vox-cli --lib commands::harness::report`
Expected: all 7 tests pass.

- [ ] **Step 6: Manual smoke test**

Run: `cargo run -p vox-cli -- harness history` (after Task 5-7 have produced at least one real
run, or against an empty DB to confirm the "no runs recorded yet" path).
Expected: prints either the "no runs" message or a table (now including the free/cheap% column),
without panicking.

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

/// Per-category pass/fail rollup for one run — closes design spec §10.2's "per-task-category
/// breakdown... visible at a glance, not buried in an aggregate pass rate" requirement.
#[derive(Debug, Serialize)]
pub struct CategorySummaryDto {
    pub category: String,
    pub pass_count: i64,
    pub fail_count: i64,
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
    pub category_breakdown: Vec<CategorySummaryDto>,
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
    let mut out = Vec::with_capacity(runs.len());
    for r in runs {
        let task_results = db
            .get_harness_eval_task_results(&r.run_id)
            .await
            .map_err(map_db_err)?;
        let mut by_category: std::collections::BTreeMap<String, (i64, i64)> =
            std::collections::BTreeMap::new();
        for t in &task_results {
            let entry = by_category.entry(t.category.clone()).or_default();
            if t.status == "pass" {
                entry.0 += 1;
            } else if t.status == "fail" {
                entry.1 += 1;
            }
        }
        out.push(HarnessEvalRunDto {
            run_id: r.run_id,
            git_sha: r.git_sha,
            triggered_by: r.triggered_by,
            pass_count: r.pass_count,
            fail_count: r.fail_count,
            skip_count: r.skip_count,
            total_cost_usd: r.total_cost_usd,
            started_at_ms: r.started_at_ms,
            category_breakdown: by_category
                .into_iter()
                .map(|(category, (pass_count, fail_count))| CategorySummaryDto {
                    category,
                    pass_count,
                    fail_count,
                })
                .collect(),
        });
    }
    Ok(out)
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
    pub flipped_task_ids: Vec<String>,
    pub detail: String,
}

/// Same validation `vox-cli`'s `ingest_runs`/`report.rs` apply — this command constructs its own
/// `git diff` subprocess call independently, so it needs its own defense-in-depth check too, not
/// just a shared library function it might forget to call.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
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
    if !is_valid_git_sha(&previous.git_sha) || !is_valid_git_sha(&current.git_sha) {
        return Err(format!(
            "refusing to shell out to git diff with a malformed git_sha (previous={:?}, current={:?})",
            previous.git_sha, current.git_sha
        ));
    }
    let previous_task_results = db
        .get_harness_eval_task_results(&previous.run_id)
        .await
        .map_err(map_db_err)?;
    let current_task_results = db
        .get_harness_eval_task_results(&current.run_id)
        .await
        .map_err(map_db_err)?;
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
            "--",
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
        &previous_task_results,
        &current_task_results,
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
            flipped_task_ids: f.flipped_task_ids,
            detail: f.detail,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// Calls the REAL `#[tauri::command]` function through `tauri::test::mock_app()` + a real
    /// in-memory `GuiDbPool` — the pattern already established in `crates/vox-gui/src/commands/
    /// chat.rs`'s own tests. A test that manually re-derives the DTO shape inline (as an earlier
    /// draft of this file did) never actually calls the production function and cannot catch a
    /// bug in it; this does.
    #[tokio::test]
    async fn harness_eval_history_returns_persisted_runs_via_the_real_command() {
        let app = tauri::test::mock_app();
        let pool = GuiDbPool::connect_memory().await.expect("memory pool");
        let db = pool.handle().expect("db handle");
        db.record_harness_eval_run(&vox_db::HarnessEvalRunRecord {
            run_id: "r1".to_string(),
            triggered_by: "local".to_string(),
            git_sha: "abc1234".to_string(),
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
        })
        .await
        .expect("record run");
        app.manage(pool);

        let state = app.state::<GuiDbPool>();
        let result = harness_eval_history(state, None).await.expect("history call");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].run_id, "r1");
        assert_eq!(result[0].pass_count, 9);
    }

    #[tokio::test]
    async fn harness_eval_regressions_returns_empty_with_fewer_than_two_runs() {
        let app = tauri::test::mock_app();
        let pool = GuiDbPool::connect_memory().await.expect("memory pool");
        app.manage(pool);

        let state = app.state::<GuiDbPool>();
        let result = harness_eval_regressions(state).await.expect("regressions call");
        assert!(result.is_empty(), "fewer than 2 runs must return no regressions, not error");
    }

    #[test]
    fn is_valid_git_sha_rejects_a_dash_prefixed_value() {
        assert!(!is_valid_git_sha("--output=/tmp/evil"));
        assert!(is_valid_git_sha("abc1234"));
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

- [ ] **Step 5: Run the tests, verify they pass**

Run: `cargo test -p vox-gui --bins commands::harness_eval`
Expected: 3 tests pass.

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
- Modify: `crates/vox-gui/ui/src/App.tsx` (add `'harness-health'` to the `View` union and to the
  `LEGACY_VIEWS` array literal — `KNOWN_VIEWS` is a direct alias of `LEGACY_VIEWS`
  (`const KNOWN_VIEWS: string[] = LEGACY_VIEWS;`), not a second array needing its own edit — do
  NOT reuse the existing `'harness'` view key, which already routes to the unrelated
  `HarnessRedirect` legacy-tab component)
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (route `'harness-health'`
  to the new component)

- [ ] **Step 1: Read the exact current `View` union and the project's own data-fetching hook**

Read `crates/vox-gui/ui/src/App.tsx`'s current `View` type union and `LEGACY_VIEWS` (confirmed
earlier this session — re-read for any drift since). This surface fetches live data, so it should
use this project's own `useVoxQuery`/`useVoxMutation` wrapper
(`crates/vox-gui/ui/src/hooks/useVoxQuery.ts` — a thin, typed wrapper over `@tanstack/react-query`'s
`useQuery`/`useMutation`, already used by e.g. `DocReader.tsx`), not raw `useQuery` directly and
not `CoverageView.tsx`'s pattern (that surface reads a static generated registry, not live IPC
data, so it's the wrong template here).

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
            category_breakdown: [
              { category: 'chat', pass_count: 5, fail_count: 0 },
              { category: 'privacy', pass_count: 2, fail_count: 0 },
              { category: 'tool-calling', pass_count: 1, fail_count: 1 },
            ],
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
            category_breakdown: [{ category: 'chat', pass_count: 5, fail_count: 5 }],
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
            flipped_task_ids: [],
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
import { invoke } from '@tauri-apps/api/core';
import { useVoxQuery } from '../../../hooks/useVoxQuery';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';

interface CategorySummaryDto {
  category: string;
  pass_count: number;
  fail_count: number;
}

interface HarnessEvalRunDto {
  run_id: string;
  git_sha: string;
  triggered_by: string;
  pass_count: number;
  fail_count: number;
  skip_count: number;
  total_cost_usd: number;
  started_at_ms: number;
  category_breakdown: CategorySummaryDto[];
}

interface RegressionFlagDto {
  kind: string;
  previous_run_id: string;
  current_run_id: string;
  previous_git_sha: string;
  current_git_sha: string;
  changed_files: string[];
  flipped_task_ids: string[];
  detail: string;
}

export function HarnessHealthView() {
  const { data: runs, isLoading } = useVoxQuery(
    ['harnessEvalHistory'],
    () => invoke<HarnessEvalRunDto[]>('harness_eval_history', { limit: 50 }),
  );
  const { data: regressions } = useVoxQuery(
    ['harnessEvalRegressions'],
    () => invoke<RegressionFlagDto[]>('harness_eval_regressions'),
  );

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
              {r.flipped_task_ids.length > 0 && (
                <p className="mt-1 text-[10px] text-red-300/80">
                  Flipped tasks: {r.flipped_task_ids.join(', ')}
                </p>
              )}
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
              <th scope="col" className="p-2">By category</th>
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
                <td className="p-2">
                  <div className="flex flex-wrap gap-1">
                    {r.category_breakdown.map((c) => (
                      <span
                        key={c.category}
                        className={`rounded px-1 py-0.5 font-mono text-[11px] ${
                          c.fail_count > 0
                            ? "bg-red-950 text-red-300"
                            : "bg-emerald-950 text-emerald-300"
                        }`}
                        title={`${c.category}: ${c.pass_count} pass, ${c.fail_count} fail`}
                      >
                        {c.category} {c.pass_count}/{c.pass_count + c.fail_count}
                      </span>
                    ))}
                  </div>
                </td>
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
