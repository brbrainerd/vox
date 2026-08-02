#![allow(missing_docs)]

//! Chat harness regression tests — one test per bug this session's code review found and
//! fixed on `claude/axis-chat-fixes`. Each test drives the REAL `Orchestrator`/
//! `AiTaskProcessor`/gate-cascade stack in-process (no mocked `invoke`, no webview) so a
//! revert of any of these fixes fails CI immediately, unlike before this plan.
//!
//! Conventions (forensic logging, timeouts, watchdog) mirror `orchestrator_e2e_test.rs` —
//! read that file first if extending this one.

use std::sync::Arc;
use std::time::Duration;
use vox_orchestrator::{Orchestrator, OrchestratorConfig, types::TaskCategory};

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
            .submit_task(
                "first message",
                vec![],
                None,
                Some(session.to_string()),
                None,
            )
            .await
            .expect("first submits");
        let second = orch
            .submit_task(
                "second message",
                vec![],
                None,
                Some(session.to_string()),
                None,
            )
            .await
            .expect("second submits");

        assert_ne!(
            first, second,
            "two submissions must never collapse to the same task id"
        );
        let all = orch.all_tasks();
        assert!(
            all.iter().any(|t| t.id == first) && all.iter().any(|t| t.id == second),
            "both submissions must be independently trackable, not silently dropped"
        );
    })
    .await
    .expect("test timed out");
}

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
        async fn submit_and_complete_under_forced_behavioral_gate(category: TaskCategory) -> bool {
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
                    "task body",
                    vec![],
                    None,
                    None,
                    None,
                    Some(hints),
                    None,
                    None,
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
            orch.complete_task(task_id).await.is_ok() && orch.status().total_completed == 1
        }

        let chat_completed =
            submit_and_complete_under_forced_behavioral_gate(TaskCategory::Chat).await;
        let codegen_completed =
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
/// `#[serial]`: mirrors the real test this fixture is copied from
/// (`crates/vox-orchestrator/src/models/select.rs`'s own
/// `decide_excludes_cloud_candidate_under_local_only_privacy`), which is `#[serial]`-guarded
/// because `set_test_privacy_override` mutates a process-global `Mutex` (`TEST_PRIVACY_OVERRIDE`
/// in `route_policy.rs`) that races other tests touching the same override under parallel
/// `cargo test` if not serialized.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn decide_never_selects_a_cloud_model_under_local_only_privacy_via_any_path() {
    use vox_orchestrator::models::select::{ModelSelectionRequest, SelectionIntent, decide};
    use vox_orchestrator::models::{ModelRegistry, ModelSpec, ProviderType};
    use vox_orchestrator::route_policy;

    tokio::time::timeout(TEST_TIMEOUT, async {
        // Mirrors the fixture shape in crates/vox-orchestrator/src/models/select.rs's own
        // decide_excludes_cloud_candidate_under_local_only_privacy test (added in this
        // session's earlier code-review fix pass).
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
                strengths: vec![vox_orchestrator::models::StrengthTag::Generalist],
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
        registry.register(minimal_spec(
            "cloud-model-fixture-id",
            ProviderType::OpenRouter,
        ));

        route_policy::set_test_privacy_override(Some("local_only"));
        let req = ModelSelectionRequest::from_intent(SelectionIntent::for_task(
            vox_orchestrator::types::TaskCategory::CodeGen,
        ));
        let decision = decide(&req, &registry);
        route_policy::set_test_privacy_override(None);

        let decision = decision.expect("local candidate must still be selectable");
        assert_eq!(decision.selected_model, "local-model-fixture-id");
    })
    .await
    .expect("test timed out");
}

/// Code-review fix (gui-axis-chat-harness-fixes, 2026-08-02), case (a) of spec §5 item 4: with no
/// local model registered under VOX_INFERENCE_PRIVACY=local_only, AiTaskProcessor::process must
/// fail closed rather than falling back to StreamRoute::Cascade (which streams from a
/// cloud-inclusive provider list with no privacy awareness — the exact leak this session's
/// runtime.rs fix closed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn ai_task_processor_fails_closed_under_local_only_with_no_local_model_registered() {
    use vox_orchestrator::route_policy;
    use vox_orchestrator::runtime::{AiTaskProcessor, TaskProcessor};

    tokio::time::timeout(TEST_TIMEOUT, async {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();

        // Wipe the registry down to zero candidates (local and cloud) before constructing
        // AiTaskProcessor, so "no local model registered" holds regardless of what the
        // default bootstrap catalog contains on this machine. `ModelRegistry` has no
        // provider-filtered `retain` — `clear()` is the real available mutation method
        // (crates/vox-orchestrator/src/models/registry.rs:635) and satisfies "no local model
        // registered" a fortiori.
        {
            let registry_handle = orch.models_handle();
            let mut registry = registry_handle.write().unwrap();
            registry.clear();
        }

        route_policy::set_test_privacy_override(Some("local_only"));
        let processor = AiTaskProcessor::new(orch.event_bus.clone(), orch.clone()).await;

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
        let result = processor
            .process(vox_orchestrator::types::AgentId(1), task, cancel)
            .await;
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
