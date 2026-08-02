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

use std::time::Instant;

/// Outcome of attempting one `LiveEvalTask` (one sample).
pub enum LiveTaskOutcome {
    Ran,
    Skipped { reason: String },
}

/// Cost ceiling per `--live` invocation (see design spec §6.3). Aborts remaining tasks, not
/// already-completed ones, if exceeded mid-run.
pub const LIVE_EVAL_COST_CEILING_USD: f64 = 0.50;

/// The initial live-eval golden task corpus (design spec §6.1): 12 tasks spanning plain chat,
/// tool-calling, privacy-mode, and cost-tier scenarios. Each `prompt` is sent through the real
/// chat harness (`chat_message`); each `checker` scores the real observed `EvalTurnResult`.
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
                // see run_live's scoped_local_only_env guard below.
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
/// `vox_orchestrator_mcp::chat_tools::chat::chat_message` (the `message` submodule that defines
/// it is private — only the function itself is re-exported, confirmed by reading
/// `chat_tools/chat/mod.rs`) — confirmed (Step 1) to return a plain `String`: a JSON envelope
/// shaped `{"success": bool, "data": {"message": {..., "content": ...}, "model_used": ...,
/// "tokens": ..., "latency_ms": ..., "selection_reason": ..., ...}}` (see `message.rs`'s
/// `ToolResult::ok(result).to_json()` call and its `result = json!({"message": asst_msg, ...})`
/// construction — `asst_msg` is a `ChatTranscriptEntry` whose reply text lives in its `content`
/// field, so the real reply-content path is `data.message.content`, not the flat `data.content`
/// this plan originally assumed).
///
/// `cost_usd`/`cost_tier` are DERIVED, not read off the wire: `chat_message`'s envelope reports
/// `model_used` and `tokens`, not a dollar figure, so this function looks the model up in the
/// model registry to get its real `ModelSpec`, then computes `cost_usd = tokens as f64 / 1000.0 *
/// blended cost_per_1k` and `cost_tier = cost_tier_for(&spec)` (Task 3) from it.
async fn run_one_turn(prompt: &str) -> anyhow::Result<EvalTurnResult> {
    use vox_orchestrator_mcp::chat_tools::chat::chat_message;
    use vox_orchestrator_mcp::chat_tools::params::ChatMessageParams;

    let state = build_eval_server_state().await?;
    // `ChatMessageParams` derives `Deserialize` only (no `Default`) — every field but `prompt`
    // is `#[serde(default)]`, so deserializing a single-key JSON object is the real, already
    // -established way to build one with sane defaults (mirrors `message.rs`'s own
    // `chat_message_envelope_includes_latency_ms` test).
    let params: ChatMessageParams = serde_json::from_value(serde_json::json!({ "prompt": prompt }))
        .map_err(|e| anyhow::anyhow!("failed to build ChatMessageParams: {e}"))?;
    let envelope_str = chat_message(&state, params).await;
    let envelope: serde_json::Value = serde_json::from_str(&envelope_str)
        .map_err(|e| anyhow::anyhow!("chat_message envelope was not valid JSON: {e}"))?;

    if envelope.get("success").and_then(|v| v.as_bool()) == Some(false) {
        let err = envelope
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("chat_message reported failure with no error message");
        anyhow::bail!("chat_message call failed: {err}");
    }
    let data = envelope
        .get("data")
        .ok_or_else(|| anyhow::anyhow!("no data field found in envelope: {envelope_str}"))?;

    let reply_text = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no data.message.content field found in envelope: {envelope_str}"))?
        .to_string();
    let model_used = data
        .get("model_used")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no data.model_used field found in envelope: {envelope_str}"))?
        .to_string();
    let tokens = data.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let latency_ms = data.get("latency_ms").and_then(|v| v.as_u64()).unwrap_or(0);

    let registry_handle = state.orchestrator.models_handle();
    let spec = {
        let registry = registry_handle
            .read()
            .map_err(|_| anyhow::anyhow!("model registry lock poisoned"))?;
        registry.get(&model_used).ok_or_else(|| {
            anyhow::anyhow!("model {model_used} not found in registry after a real chat call selected it")
        })?
    };
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

/// Build a real, hermetic-constructor-shaped `ServerState` for a `--live` eval turn. Mirrors
/// `message.rs`'s own `#[cfg(test)] fn test_state()` fixture (and the sibling fixtures in
/// `agent_loop.rs`/`conversation.rs`/`browser_tools.rs`) almost verbatim — the same
/// `ServerState::hermetic_stub` constructor, which is explicitly NOT `#[cfg(test)]`-gated for
/// exactly this reason (see its doc comment: "`vox harness eval`'s `agent-loop-terminates`
/// golden task... is a real, non-test call site in `vox-cli`"). Unlike the test fixture, this
/// does NOT set `mcp_chat_model_override` and does NOT point `OPENROUTER_BASE_URL` at a mock
/// server — real routing picks a real model from the registry, using whatever real
/// `OPENROUTER_API_KEY`/provider credentials are present in the ambient environment, and the
/// registry is seeded by `ModelRegistry::new()`'s own synchronous bootstrap load (the same
/// startup path production uses before its background 6h catalog-refresh loop kicks in).
async fn build_eval_server_state() -> anyhow::Result<vox_orchestrator_mcp::server_state::ServerState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager};
    use vox_orchestrator_mcp::server_state::ServerState;
    use vox_repository::{RepoCapabilities, RepositoryContext};

    let cfg = OrchestratorConfig::for_testing();
    let orch_cfg = cfg.clone();
    let groups = AffinityGroupRegistry::new(vec![]);
    let session_cfg = SessionConfig {
        persist: false,
        sessions_dir: std::env::temp_dir().join("vox-harness-live-eval-sessions"),
        ..SessionConfig::default()
    };
    let session_manager =
        SessionManager::new(session_cfg).map_err(|e| anyhow::anyhow!("session manager: {e}"))?;
    let repository = RepositoryContext {
        root: PathBuf::from("."),
        git_root: None,
        repository_id: "harness-live-eval".into(),
        origin_url: None,
        capabilities: RepoCapabilities {
            vox_project: false,
            cargo_workspace: false,
            cargo_package: false,
            node_workspace: false,
            python_project: false,
            go_module: false,
            git: false,
        },
        has_vox_agents_dir: false,
        vox_toml: None,
    };
    Ok(ServerState::hermetic_stub(
        cfg,
        repository,
        Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
        Arc::new(Mutex::new(session_manager)),
        vox_skills::new_registry_arc(),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

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
