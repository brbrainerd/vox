---
title: "Orchestrator Companion Audit — Non-Routing Surface Critique & Improvement Plan"
description: "Full-system audit of crates/vox-orchestrator and surrounding surfaces excluding model-routing (covered by model-orchestration-ssot-audit-2026.md). ~280 numbered improvements across 27 surface clusters (A..AB). Four-axis tagged: risk/capability/hygiene/perf × P0–P3 × S/M/L effort."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-01"
training_eligible: true
training_rationale: "Core orchestrator quality reference. Names every file with known issues, proposes mechanical operations, and provides grep-able success criteria. Companion to model-orchestration-ssot-audit-2026.md."
authored: "2026-05-01"
---

# Orchestrator Companion Audit — Non-Routing Surface Critique & Improvement Plan

## Part 1 — Scope & Boundaries

**What this document covers.** Everything in the `vox-orchestrator` system *except* model selection, model catalog, and vox-secrets secret distribution — those are covered by [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md) (FIX-01..75) and [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) (P0–P3 gaps). Items here that *touch* routing are labelled `Prior audit ref: FIX-NN` and are net-new extensions, not duplicates.

**In scope:**
- `crates/vox-orchestrator/src/` — all modules except `models/` and `catalog.rs`
- `crates/vox-orchestrator/src/mcp_tools/` — all eight tool subdirectories
- `crates/vox-db/src/store/ops_orchestrator.rs`
- `contracts/communication/orchestrator-persistence-outbox.schema.json`
- `.vox/agents/orchestrator.md`, `.vox/agents/vox-orchestrator.md`, `docs/agents/orchestrator.md`, `crates/vox-skills/skills/orchestrator.skill.md`
- `crates/vox-integration-tests/tests/orchestrator_*.rs`
- `docs/src/adr/022-orchestrator-bootstrap-and-daemon-boundaries.md`

**Out of scope for this document:** `crates/vox-orchestrator/src/models/`, `catalog.rs`, `scoring.rs`, `key_guard.rs`, `crates/vox-secrets/`, Populi mesh transport (covered separately by `vox-populi` architecture docs).

---

## Part 2 — Concern Index

Each row is one FIX item. Columns: `ID | surface | axis | concern | P | effort | title`.

| ID | Surface | Axis | Concern | P | Effort | Title |
|----|---------|------|---------|---|--------|-------|
| FIX-A-01 | orchestrator-core | hygiene | modularity | P2 | M | Flatten Arc<RwLock<T>> nesting with typed guard wrappers |
| FIX-A-02 | orchestrator-core | risk | correctness | P1 | M | Guard every db.read().as_ref() dereference against None |
| FIX-A-03 | orchestrator-core | hygiene | modularity | P2 | L | Split Orchestrator struct into domain-bounded sub-stores |
| FIX-A-04 | orchestrator-core | risk | correctness | P1 | S | Enforce feature-contract: runtime feature ↔ runtime fields |
| FIX-A-05 | orchestrator-core | hygiene | contracts | P3 | S | Document Orchestrator state invariants in a module-level doc-comment |
| FIX-A-06 | orchestrator-core | risk | correctness | P1 | M | Add state-machine guard for agent lifecycle transitions |
| FIX-B-01 | runtime | risk | correctness | P0 | M | Extract AiTaskProcessor::process() into focused sub-functions |
| FIX-B-02 | runtime | risk | correctness | P1 | S | Replace UUID split().next().unwrap() with validated parse |
| FIX-B-03 | runtime | risk | correctness | P1 | S | Upgrade AtomicUsize::fetch_add to Ordering::AcqRel for spawn counter |
| FIX-B-04 | runtime | hygiene | modularity | P2 | M | Split ActorAgent::handle_command ProcessQueue branch into helper |
| FIX-B-05 | runtime | hygiene | modularity | P2 | M | Extract queue-lock-find-update pattern into orchestrator helper |
| FIX-B-06 | runtime | risk | correctness | P1 | M | Audit Mutex lock inside reconciled_cost closure for deadlock |
| FIX-B-07 | runtime | risk | correctness | P1 | S | Add structured error type; remove bare String errors in process() |
| FIX-B-08 | runtime | capability | observability | P2 | S | Emit structured trace span at start of each TaskPhase transition |
| FIX-B-09 | runtime | perf | perf | P2 | M | Deduplicate provider match arm (10-branch) into a method |
| FIX-B-10 | runtime | risk | correctness | P1 | M | Verify async cancel safety in handle_command under task abort |
| FIX-B-11 | runtime | capability | capability | P1 | L | Implement doom-loop detector (cost_delta/progress_delta watchdog) |
| FIX-B-12 | runtime | risk | correctness | P2 | S | Check_scaling: guard AgentFleet span line 657 with overflow-safe arithmetic |
| FIX-C-01 | handoff | hygiene | modularity | P3 | S | Extract JSON find+parse into a single parse_metadata_key() helper |
| FIX-C-02 | handoff | risk | correctness | P2 | S | Aggregate transcript-bleed errors instead of three separate calls |
| FIX-C-03 | handoff | risk | correctness | P2 | S | Fix double-parse of HARNESS_SPEC_JSON_METADATA_KEY when no envelope |
| FIX-C-04 | handoff | hygiene | contracts | P2 | M | Write a JSON Schema for ContextEnvelope and validate at handoff boundary |
| FIX-C-05 | handoff | risk | testing | P1 | M | Add tests: handoff with no verification_criteria, with bleed, with bad harness JSON |
| FIX-C-06 | handoff | capability | observability | P2 | S | Emit handoff telemetry event with invariant check results |
| FIX-C-07 | handoff | risk | correctness | P2 | S | Validate harness spec side-effects before accepting as invariant |
| FIX-D-01 | events | risk | correctness | P1 | S | Replace fetch_add(Relaxed) with AcqRel for event ID ordering |
| FIX-D-02 | events | risk | correctness | P1 | S | Log dropped events when broadcast channel is at capacity |
| FIX-D-03 | events | hygiene | modularity | P2 | L | Split AgentEventKind (50+ variants) into TaskEvent/AgentEvent/SystemEvent sub-enums |
| FIX-D-04 | events | hygiene | contracts | P2 | M | Replace string IDs in ActivityStarted/Completed/Retried with AgentId/TaskId |
| FIX-D-05 | events | hygiene | modularity | P3 | M | Extract TaskStarted/Failed/Completed shared fields into a TaskEventCore struct |
| FIX-D-06 | events | capability | observability | P2 | S | Add subscriber_count telemetry metric for event bus health |
| FIX-D-07 | events | risk | correctness | P2 | M | Define and enforce event channel capacity; document lossy vs lossless lanes |
| FIX-D-08 | events | capability | capability | P2 | M | Add lock-propagation event variants for multi-agent schema-mutation coordination |
| FIX-E-01 | grounding | risk | correctness | P1 | S | Replace expect("char boundary") at line 235 with safe UTF-8 boundary check |
| FIX-E-02 | grounding | hygiene | modularity | P2 | M | Decompose split_summary_into_claim_segments() 132-line function into named steps |
| FIX-E-03 | grounding | hygiene | modularity | P3 | M | Externalize English phrase list in classify_line_claim_kind into config/YAML |
| FIX-E-04 | grounding | risk | testing | P2 | M | Add tests: emoji input, surrogate pairs, German mixed-script, empty summary |
| FIX-E-05 | grounding | hygiene | modularity | P3 | S | Deduplicate str/strasse suffix checks in locale detection helper |
| FIX-E-06 | grounding | capability | capability | P2 | L | Add entropy/confidence scoring from token probabilities to ground truth check |
| FIX-F-01 | usage-finops | risk | perf | P0 | M | Replace global static Mutex in record_call_detailed (line 256) with async channel |
| FIX-F-02 | usage-finops | risk | correctness | P1 | S | Validate JSON shape before as_u64()/as_f64() fallbacks (lines 265-268) |
| FIX-F-03 | usage-finops | hygiene | modularity | P3 | S | Replace custom today() date calc with chrono::Utc::now().date_naive() |
| FIX-F-04 | usage-finops | hygiene | modularity | P3 | S | Remove record_call() wrapper that just calls record_call_detailed() with constants |
| FIX-F-05 | usage-finops | capability | capability | P1 | L | Add per-tenant and per-fleet-segment budget caps alongside global cap |
| FIX-F-06 | usage-finops | capability | observability | P2 | S | Expose real-time budget utilization via a /v1/budget/status HTTP endpoint |
| FIX-F-07 | usage-finops | risk | correctness | P2 | M | Write unit tests: budget exhaustion halts task routing; tenant cap isolation |
| FIX-F-08 | usage-finops | capability | capability | P2 | M | Implement pre-execution token estimation using tool output length prediction |
| FIX-G-01 | mcp-dispatch | risk | correctness | P1 | S | Add per-tool execution timeout in handle_tool_call_inner (no timeout today) |
| FIX-G-02 | mcp-dispatch | risk | correctness | P1 | M | Normalize 960-line match into a tool registry (name → handler fn ptr) |
| FIX-G-03 | mcp-dispatch | risk | security | P1 | S | Move hardcoded sentinel strings (SYSTEM_INTERVENTION, etc.) to typed constants |
| FIX-G-04 | mcp-dispatch | risk | correctness | P2 | S | Replace parse::<u64>().unwrap_or(0) AgentId/TaskId fallbacks with explicit error |
| FIX-G-05 | mcp-dispatch | capability | observability | P2 | S | Emit dispatch latency span per tool call with tool_name attribute |
| FIX-G-06 | mcp-dispatch | capability | capability | P2 | M | Add per-tool rate limiter configurable from Vox.toml [orchestrator.tool_limits] |
| FIX-G-07 | mcp-dispatch | hygiene | modularity | P3 | L | Split dispatch.rs into per-tool-group modules (chat, scientia, llm, memory, task, dei) |
| FIX-G-08 | mcp-dispatch | risk | correctness | P2 | S | Add backpressure: reject tool call when agent queue exceeds configurable high-watermark |
| FIX-G-09 | mcp-dispatch | hygiene | contracts | P2 | M | Define a ToolResult error taxonomy enum (Budget/Auth/Validation/Internal/Timeout) |
| FIX-H-01 | chat-tools | risk | correctness | P1 | S | Fix .expect("telemetry JSON must parse") at mod.rs:176 — return error instead |
| FIX-H-02 | chat-tools | risk | correctness | P1 | S | Fix .expect("search_refinement field") at mod.rs:214 — guard with if-let |
| FIX-H-03 | chat-tools | risk | correctness | P1 | S | Fix .unwrap() on static Regex::new() in mentions.rs:9 — use expect with message |
| FIX-H-04 | chat-tools | hygiene | modularity | P1 | L | Decompose plan_goal() (514 lines, plan.rs:186-699) into planning pipeline stages |
| FIX-H-05 | chat-tools | hygiene | modularity | P2 | M | Decompose maybe_refine_plan() (337 lines, plan_loop.rs:151-487) |
| FIX-H-06 | chat-tools | hygiene | contracts | P2 | S | Move hardcoded token caps (3072/4096/8192) in plan.rs:55-60 to Vox.toml |
| FIX-H-07 | chat-tools | hygiene | contracts | P2 | S | Make max_tasks default (plan.rs:187) and max_refine_rounds cap (plan_loop.rs:119) configurable |
| FIX-H-08 | chat-tools | hygiene | contracts | P2 | S | Move reserve_tokens=4096 and refine_budget=18000 constants to config |
| FIX-H-09 | chat-tools | hygiene | contracts | P2 | S | Make refine threshold 0.28 (plan_loop.rs:162) and inadequacy defaults configurable |
| FIX-H-10 | chat-tools | risk | correctness | P2 | M | Add retry limit enforcement in plan_loop refinement; guard against infinite refine |
| FIX-H-11 | chat-tools | capability | observability | P2 | S | Emit plan generation span with task_count, depth, token_budget attributes |
| FIX-H-12 | chat-tools | risk | testing | P2 | M | Add tests: plan with zero tasks, plan exceeding budget, replan with empty base |
| FIX-H-13 | chat-tools | hygiene | modularity | P3 | S | Remove .unwrap_or(None).unwrap_or(0) chained option at plan.rs:495 |
| FIX-I-01 | scientia-tools | hygiene | contracts | P2 | S | Move hardcoded score thresholds (0.85, 0.62, 0.58) to config/YAML |
| FIX-I-02 | scientia-tools | hygiene | contracts | P2 | S | Move common.rs:87 evidence threshold (0.85) to Vox.toml [orchestrator.scientia] |
| FIX-I-03 | scientia-tools | risk | correctness | P2 | M | Add retry limit and timeout guard on external.rs job submission/replay loops |
| FIX-I-04 | scientia-tools | capability | observability | P2 | S | Emit scholar lifecycle events (submit, discover, publish) as typed AgentEventKind variants |
| FIX-I-05 | scientia-tools | risk | testing | P2 | M | Add tests: novelty check with empty corpus, discovery with no results, lifecycle failure paths |
| FIX-I-06 | scientia-tools | hygiene | modularity | P2 | L | Break novelty.rs and scholar.rs functions >100 lines into named computation steps |
| FIX-I-07 | scientia-tools | risk | correctness | P2 | S | Add backpressure on discovery refresh calls: enforce min_refresh_interval |
| FIX-J-01 | llm-bridge | risk | correctness | P0 | M | Add explicit HTTP timeout Duration to all provider HTTP clients in llm_bridge/ |
| FIX-J-02 | llm-bridge | hygiene | contracts | P1 | M | Replace Result<T, String> error type in infer.rs with typed ProviderError enum |
| FIX-J-03 | llm-bridge | hygiene | modularity | P1 | L | Decompose mcp_infer_tool_completion() (390 lines, infer.rs:210-599) |
| FIX-J-04 | llm-bridge | hygiene | contracts | P2 | S | Replace hardcoded vision token estimate (1000) with per-model config lookup |
| FIX-J-05 | llm-bridge | risk | correctness | P2 | M | Add exponential backoff on HTTP 429/408/504 (detected but no backoff today) |
| FIX-J-06 | llm-bridge | capability | observability | P2 | S | Emit provider attempt span with status_code, attempt_number, provider_name |
| FIX-J-07 | llm-bridge | capability | capability | P2 | M | Add schema-aware message translation layer for Anthropic tool-call alternation |
| FIX-J-08 | llm-bridge | risk | correctness | P2 | S | Validate provider response schema before silent .unwrap_or_default() fallbacks |
| FIX-J-09 | llm-bridge | hygiene | modularity | P3 | M | Consolidate per-provider adapters behind a single ProviderAdapter trait impl |
| FIX-J-10 | llm-bridge | capability | capability | P2 | M | Add PII-sensitive routing: AgentTask.sensitivity_marker gates provider selection |
| FIX-K-01 | http-gateway | risk | security | P0 | S | Remove DEBUG println! at mod.rs:176-177 (env var leak to logs) |
| FIX-K-02 | http-gateway | risk | security | P0 | S | Fix origin_guard.rs starts_with URL matching (prefix bypass: 127.0.0.1.attacker.com) |
| FIX-K-03 | http-gateway | risk | security | P1 | M | Add request body size limit on eval endpoint; reject EvalRequest.code > 64KiB |
| FIX-K-04 | http-gateway | risk | security | P1 | M | Move WebSocket auth-on-first-message to connection handshake; reject unauthenticated upgrades |
| FIX-K-05 | http-gateway | risk | security | P1 | S | Remove query-param token fallback in ws.rs (?token=, ?bearer=); tokens in URLs appear in logs |
| FIX-K-06 | http-gateway | risk | security | P1 | S | Fix constant_time_eq: ensure max(a,b) branch doesn't leak token length via timing |
| FIX-K-07 | http-gateway | risk | security | P1 | S | Audit bearer-token-to-dashboard-token downgrade (mod.rs:227); document role mapping |
| FIX-K-08 | http-gateway | risk | correctness | P1 | S | Add SIGTERM/SIGINT graceful shutdown handler to the daemon and gateway |
| FIX-K-09 | http-gateway | capability | ops | P1 | S | Add /healthz and /readyz endpoints for k8s-style probe routing |
| FIX-K-10 | http-gateway | risk | correctness | P1 | S | Replace fragile unwrap() after guarded branch in ws.rs:87 with pattern-match |
| FIX-K-11 | http-gateway | hygiene | modularity | P2 | M | Decompose spawn_http_gateway_if_enabled (124 lines) into named setup phases |
| FIX-K-12 | http-gateway | hygiene | contracts | P2 | S | Make dashboard token TTL (30 days, token.rs:19) configurable via Vox.toml |
| FIX-K-13 | http-gateway | risk | correctness | P2 | S | Validate token.rs file parse failure: log and require re-auth rather than silently regenerate |
| FIX-K-14 | http-gateway | risk | security | P2 | S | Enforce Windows ACL semantics for token file (token.rs platform path); match Unix mode 0o600 |
| FIX-K-15 | http-gateway | capability | ops | P2 | M | Add per-IP rate limiting to eval endpoint (CPU DoS risk, eval.rs:87) |
| FIX-K-16 | http-gateway | hygiene | contracts | P2 | S | Centralise auth role-decision logic; remove cross-layer auth in eval.rs vs origin_guard.rs |
| FIX-K-17 | http-gateway | hygiene | modularity | P3 | M | Pre-compute role-to-tool permission matrix at startup instead of per-call check |
| FIX-K-18 | http-gateway | capability | observability | P2 | S | Add structured access log with trace_id, role, tool_name on every gateway request |
| FIX-L-01 | memory-tools | hygiene | contracts | P3 | S | Move hardcoded item limits (20, 10) and confidence defaults (0.5, 1.0) to config |
| FIX-L-02 | memory-tools | capability | observability | P3 | S | Propagate REM_* remediation constants to all handler error paths consistently |
| FIX-L-03 | memory-tools | risk | correctness | P2 | M | Add rate limiting on memory store/recall operations per agent session |
| FIX-L-04 | memory-tools | risk | testing | P2 | M | Add tests: store near capacity, concurrent reads/writes, session isolation |
| FIX-L-05 | memory-tools | hygiene | modularity | P3 | M | Document memory isolation guarantees: can agent A read agent B's scratchpad? |
| FIX-M-01 | task-tools | hygiene | modularity | P2 | M | Deduplicate companion lookup/upsert pattern (3× in lifecycle.rs) into a helper |
| FIX-M-02 | task-tools | risk | correctness | P1 | S | Log errors from fire-and-forget companion gamification async tasks |
| FIX-M-03 | task-tools | risk | correctness | P2 | S | Validate priority string at parse time; reject unknown priority rather than defaulting |
| FIX-M-04 | task-tools | risk | testing | P2 | M | Add tests: task priority parsing, companion update failure, lifecycle abort |
| FIX-M-05 | task-tools | capability | observability | P2 | S | Emit task lifecycle events (submit/start/complete/cancel) as typed events via EventBus |
| FIX-N-01 | dei-shim | hygiene | modularity | P1 | L | Plan and execute dei_shim (2769 LOC) retirement: audit callers, migrate, gate with feature flag |
| FIX-N-02 | dei-shim | hygiene | modularity | P2 | M | Audit dei_tools/ for callers of shim functions; migrate each to canonical module |
| FIX-N-03 | dei-shim | hygiene | docs | P2 | S | Add #[deprecated] attribute to all dei_shim public symbols with migration path |
| FIX-N-04 | dei-shim | hygiene | modularity | P2 | M | Confirm orchestrator_snapshot.rs in dei_tools/ still needed; extract to dei-independent path |
| FIX-N-05 | dei-shim | hygiene | ops | P3 | S | Add CI lint: fail if any new non-test file imports from dei_shim |
| FIX-O-01 | daemon | risk | correctness | P1 | M | Add SIGTERM/SIGINT handler with graceful orchestrator flush in vox_orchestrator_d.rs |
| FIX-O-02 | daemon | risk | correctness | P1 | S | Treat init_db failure (line 75) as fatal with structured error; do not continue |
| FIX-O-03 | daemon | risk | correctness | P1 | S | Return Result from SessionManager initialization (line 120); eliminate panic! |
| FIX-O-04 | daemon | capability | ops | P1 | S | Add /healthz readiness probe (FIX-K-09 provides HTTP; daemon needs to wire it) |
| FIX-O-05 | daemon | hygiene | docs | P2 | M | Update ADR-022 with Phase B TCP/stdio RPC flag matrix current status |
| FIX-O-06 | daemon | risk | correctness | P2 | S | Log and alarm if spawn_http_gateway_if_enabled returns Err; daemon should not silently continue |
| FIX-O-07 | daemon | capability | ops | P2 | M | Add --dry-run flag to daemon startup: validates config + DB without binding sockets |
| FIX-O-08 | daemon | capability | ops | P2 | M | Emit structured startup metrics (uptime, build_sha, feature_flags) on boot |
| FIX-O-09 | daemon | hygiene | docs | P3 | S | Write a new ADR for daemon split-plane RPC flag matrix (Phase B from ADR-022) |
| FIX-P-01 | persistence-outbox | risk | correctness | P1 | M | Guard fence token increment in ops_orchestrator.rs for i64::MAX overflow |
| FIX-P-02 | persistence-outbox | risk | correctness | P1 | M | Add clock-skew tolerance (±5s window) to distributed lock TTL comparison |
| FIX-P-03 | persistence-outbox | hygiene | contracts | P2 | M | Add schema version field to orchestrator-persistence-outbox.schema.json |
| FIX-P-04 | persistence-outbox | hygiene | contracts | P2 | S | Replace additionalProperties:true in replayPayload with explicit discriminated union |
| FIX-P-05 | persistence-outbox | risk | correctness | P2 | M | Write idempotence tests for every outbox replay_op (all 6 operations) |
| FIX-P-06 | persistence-outbox | capability | observability | P2 | S | Add lifecycle tick observability: emit metric for queued/pruned/retried/replayed counts |
| FIX-P-07 | persistence-outbox | risk | correctness | P2 | M | Clarify dual Result<Result<T, String>, StoreError> return in acquire_distributed_lock |
| FIX-P-08 | persistence-outbox | hygiene | docs | P3 | S | Write a mini-ADR for degraded-mode persistence outbox semantics and replay ordering |
| FIX-P-09 | persistence-outbox | risk | correctness | P2 | M | Add circuit-breaker recovery test: simulate DB unavailability then reconnect |
| FIX-P-10 | persistence-outbox | capability | observability | P2 | S | Track outbox queue depth as a Prometheus-compatible gauge exposed via /metrics |
| FIX-Q-01 | planning | hygiene | contracts | P2 | S | Replace string-based policy selection (policy.rs:6-10) with a typed PlanPolicy enum |
| FIX-Q-02 | planning | risk | correctness | P2 | M | Add maximum recursion depth guard to router to prevent infinite replan loops |
| FIX-Q-03 | planning | risk | testing | P2 | M | Add tests: plan synthesis with no nodes, replan after partial completion, policy conflict |
| FIX-Q-04 | planning | capability | observability | P2 | S | Emit planning decision telemetry: chosen policy, node_count, synthesis_latency_ms |
| FIX-Q-05 | planning | hygiene | modularity | P3 | M | Document planning module's relationship to chat_tools/plan.rs; separate responsibilities |
| FIX-R-01 | scaling-services | hygiene | modularity | P2 | M | Extract services/ embeddings, routing-policy, gateway helpers into typed service traits |
| FIX-R-02 | scaling-services | risk | correctness | P2 | M | Add invariant test: ScalingAction decisions are idempotent under concurrent check_scaling |
| FIX-R-03 | scaling-services | capability | observability | P2 | S | Emit scaling decision events (scale_up/scale_down/no_op) with agent_count, load metrics |
| FIX-R-04 | scaling-services | hygiene | contracts | P3 | S | Document scaling cooldown logic; make cooldown_secs configurable via Vox.toml |
| FIX-R-05 | scaling-services | risk | correctness | P2 | S | Verify check_scaling arithmetic is overflow-safe for large agent counts (line 657) |
| FIX-S-01 | agent-prompts | hygiene | agent-prompts | P2 | M | Reconcile MCP tool lists across .vox/agents/orchestrator.md and docs/agents/orchestrator.md |
| FIX-S-02 | agent-prompts | hygiene | agent-prompts | P2 | M | Audit .vox/agents/orchestrator.md tool categories against dispatch.rs tool registry |
| FIX-S-03 | agent-prompts | hygiene | agent-prompts | P2 | S | Add explicit version/date stamp to each agent prompt so staleness is detectable |
| FIX-S-04 | agent-prompts | hygiene | agent-prompts | P2 | M | Consolidate Socrates gate policy between .vox/agents/orchestrator.md and docs/agents/orchestrator.md |
| FIX-S-05 | agent-prompts | hygiene | agent-prompts | P3 | S | Define canonical read vs. write tool categories in vox-orchestrator.md scope declaration |
| FIX-S-06 | agent-prompts | hygiene | agent-prompts | P3 | M | Review orchestrator.skill.md for drift from crate layout (agent lifecycle, handoff, budget) |
| FIX-S-07 | agent-prompts | capability | agent-prompts | P2 | M | Add doom-loop intervention policy to orchestrator.md (cost/progress watchdog instructions) |
| FIX-S-08 | agent-prompts | capability | agent-prompts | P2 | M | Add lock-propagation awareness: when agent performs schema mutation, instruct broadcast |
| FIX-S-09 | agent-prompts | hygiene | docs | P3 | S | Link each agent prompt to its governing ADR and SSOT section |
| FIX-T-01 | tests | risk | testing | P0 | M | Add tests: Socrates gate enforce mode (task blocked), shadow mode (task proceeds but logged) |
| FIX-T-02 | tests | risk | testing | P1 | M | Add tests: trust-gate-relax path (agent reliability below threshold) |
| FIX-T-03 | tests | risk | testing | P1 | M | Add tests: handoff invariant failures (each of the 6 invariants violated independently) |
| FIX-T-04 | tests | risk | testing | P1 | M | Add tests: conflict resolution between concurrent agents modifying same file |
| FIX-T-05 | tests | risk | testing | P1 | L | Add tests: outbox degraded-mode round-trip (DB offline → queue → reconnect → replay) |
| FIX-T-06 | tests | risk | testing | P1 | M | Add tests: doom-loop detector triggers under simulated runaway cost |
| FIX-T-07 | tests | risk | testing | P1 | M | Add tests: distributed lock expiry and foreign-holder rejection |
| FIX-T-08 | tests | risk | testing | P2 | M | Add tests: event bus capacity reached; verify dropped events are logged |
| FIX-T-09 | tests | risk | testing | P2 | M | Add tests: multi-agent task fan-out with budget exhaustion mid-flight |
| FIX-T-10 | tests | hygiene | testing | P2 | M | Unify MCP tool tests from inline #[cfg(test)] into a coherent per-tool test module |
| FIX-T-11 | tests | risk | testing | P2 | M | Add tests: daemon graceful shutdown drains outbox before exiting |
| FIX-T-12 | tests | risk | testing | P2 | M | Add tests: gateway origin guard — loopback pass, external reject, prefix-bypass blocked |
| FIX-T-13 | tests | hygiene | testing | P3 | S | Add deterministic seeds to e2e tests (orchestrator_e2e_test.rs) for reproducibility |
| FIX-T-14 | tests | hygiene | testing | P3 | S | Add bootstrap parity test for all Orchestrator fields, not only repository_id and shard paths |
| FIX-U-01 | contracts | hygiene | contracts | P2 | M | Define contracts/communication/handoff-envelope.v1.schema.json; validate in handoff.rs |
| FIX-U-02 | contracts | hygiene | contracts | P2 | M | Define contracts/communication/agent-event.v1.schema.json aligned with AgentEventKind |
| FIX-U-03 | contracts | hygiene | contracts | P2 | M | Define contracts/communication/mcp-dispatch-error.v1.schema.json for ToolResult errors |
| FIX-U-04 | contracts | hygiene | contracts | P3 | S | Add schema version field to all orchestrator contract JSON files |
| FIX-U-05 | contracts | hygiene | contracts | P3 | S | Add vox ci contracts-validate guard checking all contracts/ JSON against their schemas |
| FIX-V-01 | adrs-docs | hygiene | docs | P2 | M | Update ADR-022 Phase B section: document current RPC flag matrix and remaining gaps |
| FIX-V-02 | adrs-docs | hygiene | docs | P2 | S | Write ADR-023: event bus semantics (lossy vs lossless lanes, channel capacity policy) |
| FIX-V-03 | adrs-docs | hygiene | docs | P2 | S | Write ADR-024: distributed lock fence-token discipline and clock-skew policy |
| FIX-V-04 | adrs-docs | hygiene | docs | P3 | S | Add "related ADRs" footer to each orchestrator ADR linking to others |
| FIX-V-05 | adrs-docs | hygiene | docs | P3 | S | Update docs/agents/orchestrator.md to reflect dei_shim retirement plan (FIX-N-01) |
| FIX-X-01 | error-handling | risk | correctness | P1 | L | Audit all unwrap()/expect() in crates/vox-orchestrator/src/**/*.rs; replace panic paths |
| FIX-X-02 | error-handling | hygiene | modularity | P1 | M | Define OrchestratorError enum with variants per subsystem; propagate through public API |
| FIX-X-03 | error-handling | hygiene | contracts | P2 | M | Standardize on err_with_remediation() pattern across all MCP tool handlers |
| FIX-X-04 | error-handling | hygiene | modularity | P2 | M | Add error context (file, agent_id, task_id) to every error propagation in the call stack |
| FIX-X-05 | error-handling | risk | correctness | P2 | S | Ban bare String as error type in Result<_, String>; enforce with a clippy lint |
| FIX-X-06 | error-handling | hygiene | modularity | P3 | S | Centralise all REM_* remediation string constants into one remediation.rs module |
| FIX-Y-01 | async-cancel | risk | correctness | P1 | M | Audit all async fn holding RwLock guard across .await boundaries; replace with scope guard |
| FIX-Y-02 | async-cancel | risk | correctness | P1 | M | Audit tokio::select! arms for dropped futures holding locks; ensure cleanup on cancel |
| FIX-Y-03 | async-cancel | risk | correctness | P2 | M | Add cancel-safety documentation to every async fn in runtime.rs and orchestrator.rs |
| FIX-Y-04 | async-cancel | risk | testing | P2 | M | Add test: task abort mid-execution leaves no orphan lock or outbox entry |
| FIX-Z-01 | observability | capability | observability | P1 | M | Add slow-operation detector: emit WARN if any tool handler exceeds 5s wall-clock |
| FIX-Z-02 | observability | capability | observability | P2 | S | Enforce tracing span names follow vox_orchestrator::<module> convention (no vox_dei::) |
| FIX-Z-03 | observability | capability | observability | P2 | M | Add log-level audit: DEBUG in production code; demote or gate behind --verbose flag |
| FIX-Z-04 | observability | capability | observability | P2 | S | Add metric: active_agents gauge, queue_depth gauge, handoff_count counter |
| FIX-Z-05 | observability | capability | observability | P3 | M | Emit structured startup audit log listing feature flags, config sources, DB connection state |
| FIX-Z-06 | observability | hygiene | docs | P3 | S | Document metric naming convention in docs/src/reference/observability.md |
| FIX-AA-01 | security | risk | security | P0 | S | Remove DEBUG println! (already filed as FIX-K-01 — cross-reference only) |
| FIX-AA-02 | security | risk | security | P1 | M | Install tracing redaction middleware across all orchestrator spans (see prior FIX-53) |
| FIX-AA-03 | security | risk | security | P1 | M | Verify no SecretSpec-managed value reaches a tool handler parameter or log line |
| FIX-AA-04 | security | risk | security | P2 | M | Add replay-attack protection to A2A delivery: reject duplicate jwe_payload nonces |
| FIX-AA-05 | security | risk | security | P2 | M | Add SSRF guard in eval endpoint: block outbound network from eval subprocess |
| FIX-AA-06 | security | risk | security | P2 | S | Document authenticated surface area in a threat model table in docs/src/security/ |
| FIX-AA-07 | security | hygiene | security | P3 | M | Run cargo-audit and cargo-deny on vox-orchestrator in CI; gate on advisories |
| FIX-AB-01 | lifecycle | risk | correctness | P1 | M | Implement Drop for Orchestrator: flush outbox, drain event bus, close DB handle |
| FIX-AB-02 | lifecycle | risk | correctness | P1 | M | Ensure shutdown ordering: stop accepting tasks → drain queue → flush outbox → close DB |
| FIX-AB-03 | lifecycle | risk | correctness | P2 | M | Add RAII guard for distributed lock: auto-release on drop even if holder panics |
| FIX-AB-04 | lifecycle | risk | correctness | P2 | M | Document and test restart determinism: same repo → same shard paths and memory layout |
| FIX-AB-05 | lifecycle | risk | testing | P2 | M | Add test: orchestrator restart after crash recovers from outbox replay without duplicates |

---

## Part 3 — Surface Clusters

Each cluster has a Design Note (architectural context) followed by its FIX items (pulled from Part 2 above by ID prefix). Use the concern index for cross-referencing.

---

### Cluster A — Core `orchestrator.rs` and Submodule Tree

**Design Note.** The `Orchestrator` struct (`orchestrator.rs:53-128`) is a 75-field god object where every major subsystem — affinity map, lock manager, context store, budget, event bus, agent registry, JJ oplog, conflict manager, snapshot, runtime handles, DB handle — is held behind `Arc<RwLock<T>>` without consistency invariants between fields. The optional `db: Option<Arc<VoxDb>>` at line 98 is consumed across ~20 downstream sites without a unified access pattern, creating silent None-dereference risk. The submodule split into `core`, `agent_lifecycle`, `scaling`, `vcs_ops` helps but the struct root remains an anti-pattern against domain cohesion. The goal is to move toward bounded sub-stores with typed accessors, deferring a full God-object split to Cluster B (runtime) and Cluster R (scaling).

**FIX-A-01** `[OPEN]`
`surface: orchestrator-core` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: M`
- *Problem.* Every field in `Orchestrator` is wrapped `Arc<RwLock<T>>` with no shared consistency boundary. Write to two related fields (e.g., agent registry + task assignments) requires locking each separately; no atomic update across the pair.
- *Operation.* Introduce typed guard wrappers: `AgentRegistryGuard`, `TaskQueueGuard`. Each wraps a coarse `RwLock` covering all related fields. Replace raw `Arc<RwLock<T>>` at each related field cluster with a single shared guard type.
- *Success.* `rg 'Arc<RwLock' crates/vox-orchestrator/src/orchestrator.rs` returns ≤10 hits (from ~40 today). Existing tests green.

**FIX-A-02** `[OPEN]`
`surface: orchestrator-core` | `axis: risk` | `concern: correctness` | `P1` | `effort: M`
- *Problem.* `self.db.read().unwrap().as_ref()` dereference pattern at numerous call sites. If `db` is `None` (offline mode) the call site must handle it; inconsistent handling produces silent no-ops or panics.
- *Operation.* Add `fn db_required(&self) -> Result<Arc<VoxDb>, OrchestratorError>` and `fn db_optional(&self) -> Option<Arc<VoxDb>>`. Replace all direct field reads. Callers that require DB explicitly propagate `OrchestratorError::DatabaseUnavailable`.
- *Success.* `rg '\.db\.read\(\)' crates/vox-orchestrator/src` returns zero hits outside the new accessor.

**FIX-A-03** `[OPEN]`
`surface: orchestrator-core` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: L`
- *Problem.* 75-field struct resists reading and modification. Adding a new subsystem requires editing the god object.
- *Operation.* Group fields into four domain structs: `AgentDomain { agents, groups, task_assignments, task_traces }`, `VcsDomain { snapshot, oplog, conflict_manager, workspace_manager }`, `BudgetDomain { budget, usage }`, `InfraDomain { db, event_bus, runtime_handle }`. Expose via `fn agents(&self) -> &AgentDomain`, etc.
- *Success.* `Orchestrator` struct body ≤20 fields; domain struct fields are its own concern.

**FIX-A-04** `[OPEN]`
`surface: orchestrator-core` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* Runtime handles at `orchestrator.rs:84-86` are `#[cfg(feature = "runtime")]` but DB at line 98 is unconditional. A build without the runtime feature still requires DB, producing asymmetric feature contracts.
- *Operation.* Audit all feature-gated fields. Either gate DB access behind the same feature or document explicitly why they differ (add a `// INVARIANT:` comment).
- *Success.* `cargo check --no-default-features -p vox-orchestrator` produces zero unexpected type errors.

**FIX-A-05** `[OPEN]`
`surface: orchestrator-core` | `axis: hygiene` | `concern: contracts` | `P3` | `effort: S`
- *Problem.* No invariant documentation exists on the struct. A developer reading it cannot know valid state transitions.
- *Operation.* Add a `//! # State invariants` block at the top of `orchestrator.rs` listing: (1) db can be None only in offline mode, (2) agents map keys match task_assignments keys, (3) event_bus is always initialized.
- *Success.* Block exists; reviewed in code review.

**FIX-A-06** `[OPEN]`
`surface: orchestrator-core` | `axis: risk` | `concern: correctness` | `P1` | `effort: M`
- *Problem.* Agent lifecycle transitions (idle → assigned → running → completed) have no state-machine guard. Callers can set contradictory states.
- *Operation.* Define `AgentState` enum; add `transition(&mut self, to: AgentState) -> Result<()>` that validates the transition table. Reject invalid transitions with `OrchestratorError::InvalidStateTransition`.
- *Success.* Test: attempt idle→completed transition returns Err; idle→assigned→running→completed returns Ok.

---

### Cluster B — Runtime Supervisor (`runtime.rs`)

**Design Note.** The `AiTaskProcessor::process()` function at `runtime.rs:139-417` (279 lines) combines model routing, phase-loop execution, drift detection, cost reconciliation, and transcript persistence in a single method. This is the most complex function in the codebase and the highest-risk change target. The phase loop at lines 266-349 has embedded `match TaskPhase::*` control flow across ~80 lines. `ActorAgent::handle_command` (83 lines, lines 467-549) contains a `ProcessQueue` branch worth 60 lines of nested conditionals. Scaling check at lines 657-771 is independently well-factored. The panic at line 733 (UUID format assumption) is the only hard-crash path found.

**FIX-B-01** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P0` | `effort: M`
- *Problem.* `AiTaskProcessor::process()` (279 lines) violates SRP: routing + phases + drift + persistence in one function makes testing and modification high-risk.
- *Operation.* Extract: `select_model_for_task()`, `execute_phase_loop() -> PhaseLoopResult`, `detect_drift(PhaseLoopResult) -> DriftDecision`, `persist_transcript(PhaseLoopResult)`. Keep `process()` as the orchestrating thin wrapper.
- *Success.* `process()` body ≤60 lines; each extracted fn has its own unit test.

**FIX-B-02** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `runtime.rs:733` — `.split('-').next().unwrap()` on a UUID string. Panics if UUID format changes or value is empty.
- *Operation.* Replace with `s.split('-').next().ok_or(RuntimeError::MalformedUuid(s.to_string()))?`.
- *Success.* `rg '\.split.*\.next\(\)\.unwrap\(\)' crates/vox-orchestrator/src/runtime.rs` returns zero hits.

**FIX-B-03** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `runtime.rs:722-746` spawn counter uses `AtomicUsize::fetch_add(1, Ordering::Relaxed)`. Concurrent scale-up decisions may read a stale counter, triggering over-spawning.
- *Operation.* Change to `Ordering::AcqRel` for the fetch_add and `Ordering::Acquire` for loads.
- *Success.* Test with two concurrent `check_scaling` calls: total spawned agents ≤ configured max.

**FIX-B-04** `[OPEN]`
`surface: runtime` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: M`
- *Problem.* `handle_command`'s `ProcessQueue` branch (lines 474-532) is 60 lines with four levels of nesting. Hard to follow cancellation and error paths.
- *Operation.* Extract `process_queued_task(orchestrator: &Orchestrator, agent_id: AgentId) -> Result<ProcessOutcome>` as a standalone function.
- *Success.* `handle_command` ≤40 lines; `process_queued_task` has a dedicated unit test.

**FIX-B-05** `[OPEN]`
`surface: runtime` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: M`
- *Problem.* Queue-lock → find_task_mut → update pattern duplicated at lines 278-286 and 392-410 with subtle differences masking bugs.
- *Operation.* Add `fn with_task_mut<F, R>(orchestrator: &Orchestrator, task_id: TaskId, f: F) -> Result<R>` helper; use at both call sites.
- *Success.* Duplication gone; both call sites share one implementation.

**FIX-B-06** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P1` | `effort: M`
- *Problem.* `runtime.rs:259-261` acquires a Mutex inside a `reconciled_cost` closure that may run while another Mutex is held. Potential deadlock under contention.
- *Operation.* Move cost reconciliation outside the closure; compute the cost value first, then acquire the Mutex, then write.
- *Success.* Deadlock stress test (1000 concurrent tasks) completes without hang.

**FIX-B-07** `[OPEN]`
`surface: runtime` | `axis: hygiene` | `concern: contracts` | `P1` | `effort: S`
- *Problem.* Several paths in `process()` return `Err(String)`. String errors lose structured context and cannot be matched.
- *Operation.* Add `RuntimeError` enum (variants: `ModelNotFound`, `PhaseTimeout`, `DriftHalted`, `PersistenceFailed`). Replace all `Err("...")` in runtime.rs.
- *Success.* `rg 'Err(String' crates/vox-orchestrator/src/runtime.rs` returns zero hits.

**FIX-B-08** `[OPEN]`
`surface: runtime` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* No span emitted at TaskPhase transitions; phases are invisible in traces.
- *Operation.* At start of each `match TaskPhase::*` arm, emit `tracing::info_span!("phase", phase = %phase, task_id = %task_id)`.
- *Success.* A trace for a multi-phase task shows one child span per phase.

**FIX-B-09** `[OPEN]`
`surface: runtime` | `axis: perf` | `concern: perf` | `P2` | `effort: M`
- *Problem.* `runtime.rs:163-180` matches on 10 `ProviderType` variants to build a provider config. Same pattern exists in model resolution.
- *Operation.* Move to `ProviderType::to_route_config(&self) -> RouteConfig` method; call it at both sites.
- *Success.* `rg 'ProviderType::OpenRouter\|ProviderType::Anthropic' crates/vox-orchestrator/src/runtime.rs` returns zero hits (moved to impl block).

**FIX-B-10** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P1` | `effort: M`
- *Problem.* `handle_command` at `runtime.rs:452-454` receives orchestrator references across an `.await`. If the future is cancelled mid-call, the borrow may outlive the orchestrator's expected state.
- *Operation.* Audit with `#[must_use]` and `tokio-cancel-safety` doc tags. Replace borrowed references with owned `Arc` clones that are cancel-safe.
- *Success.* `cargo clippy` with `clippy::await_holding_refcell_ref` equivalent passes; cancel test does not corrupt agent state.

**FIX-B-11** `[OPEN]`
`surface: runtime` | `axis: capability` | `concern: capability` | `P1` | `effort: L`
- *Problem.* No doom-loop detector exists. An agent spending unbounded tokens with no task progress can run indefinitely. (Identified as P0 gap in nextgen research §4.1.)
- *Operation.* Track `(token_spend_delta, task_progress_delta)` per agent per window (configurable, default 5 min). If `cost_delta / progress_delta > threshold` for 3 consecutive windows → emit `AgentEventKind::DoomLoopDetected` → halt agent and notify operator.
- *Success.* Test: agent that loops 100 iterations with no task completion is halted within 3 windows; trace shows `DoomLoopDetected` event.

**FIX-B-12** `[OPEN]`
`surface: runtime` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* `AgentFleet::check_scaling` at line 657 performs arithmetic on agent counts without overflow protection.
- *Operation.* Replace plain arithmetic with `.checked_add()` / `.saturating_sub()`; return `ScalingError::ArithmeticOverflow` if bounds exceeded.
- *Success.* Test with `usize::MAX` agent count returns `Err` rather than panic.

---

### Cluster C — Handoff (`handoff.rs`)

**Design Note.** `handoff.rs` is the most structurally sound cluster in the codebase: invariant validation is well-separated from execution, and the six invariant checks are readable. Three issues undermine it: (1) three independent calls to `detect_transcript_bleed()` instead of one aggregating pass, (2) nearly identical JSON find+parse for two metadata keys creating copy-paste divergence risk, (3) double-parsing of `HARNESS_SPEC_JSON_METADATA_KEY` when no context envelope is found. FIX-C items are mostly S-effort cleanup with high return.

**FIX-C-01** `[OPEN]`
`surface: handoff` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: S`
- *Problem.* JSON find+parse pattern for `CONTEXT_ENVELOPE_JSON` (lines 361-371) and `HARNESS_SPEC_JSON_METADATA_KEY` (lines 372-391) is nearly identical but differs subtly — divergence risk.
- *Operation.* Extract `fn parse_metadata_key<T: DeserializeOwned>(metadata: &Value, key: &str) -> Result<Option<T>, HandoffError>`. Call it twice.
- *Success.* Lines 361-391 replaced by two calls; unit test covers missing key, malformed JSON, and correct parse.

**FIX-C-02** `[OPEN]`
`surface: handoff` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* `detect_transcript_bleed()` called three times (lines 340, 345, 354) with separately constructed errors. If the first check fails, downstream checks still run and may produce confusing compound errors.
- *Operation.* Collect all three results, then aggregate: `let bleed_errors: Vec<_> = [check_context, check_plan, check_metadata].into_iter().flatten().collect()`. Return once with all violations.
- *Success.* Test with bleed in two fields returns one error listing both violations.

**FIX-C-03** `[OPEN]`
`surface: handoff` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* `handoff.rs:405-418` re-parses `HARNESS_SPEC_JSON_METADATA_KEY` (lines 406-411 and 437-440) when no context envelope is found — parses the same key twice.
- *Operation.* Parse once, store in a local `let harness_spec = parse_metadata_key(...)?`, reuse.
- *Success.* `rg 'HARNESS_SPEC_JSON_METADATA_KEY' crates/vox-orchestrator/src/handoff.rs` shows ≤2 occurrences (one parse, one reference).

**FIX-C-04** `[OPEN]`
`surface: handoff` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: M`
- *Problem.* `ContextEnvelope` shape is validated only by Rust type-level deserialization; no JSON Schema contract for external producers (e.g., agent runtimes calling over MCP).
- *Operation.* Write `contracts/communication/handoff-envelope.v1.schema.json`. Call `jsonschema::validate()` in `validate_handoff_invariants` before deserialization.
- *Success.* A misshapen envelope from an external caller is rejected at the boundary, not silently defaults.

**FIX-C-05** `[OPEN]`
`surface: handoff` | `axis: risk` | `concern: testing` | `P1` | `effort: M`
- *Problem.* No dedicated handoff test file. The six invariants have no failing-path coverage.
- *Operation.* Create `crates/vox-orchestrator/tests/handoff_tests.rs`. Cover: (1) no verification criteria with pending tasks, (2) transcript bleed in context_notes, (3) remote handoff without A2A context, (4) malformed harness JSON, (5) valid full handoff, (6) empty handoff.
- *Success.* `cargo test -p vox-orchestrator handoff` passes 6 test cases.

**FIX-C-06** `[OPEN]`
`surface: handoff` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* Handoff success/failure is not visible in traces or event bus.
- *Operation.* At end of `execute_handoff`, emit `AgentEventKind::PlanHandoff` with invariant_check_passed, violation_count fields. On failure, emit `AgentEventKind::HandoffFailed` with error_kind.
- *Success.* A handoff violation is visible in the event log without checking handoff.rs source.

**FIX-C-07** `[OPEN]`
`surface: handoff` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* `validate_agent_harness_ingest()` at line 385 is called for its side effects; result is only error-checked, not acted upon for state changes. If it modifies shared state on success, accepting the result without storing it is a logic gap.
- *Operation.* Document explicitly whether `validate_agent_harness_ingest` is pure (validation only) or mutating. If mutating, capture return value and apply. Add a `// PURE` or `// MUTATES:` comment.
- *Success.* Code comment present; reviewer can audit without digging into implementation.

---

### Cluster D — Events Bus (`events.rs`)

**Design Note.** The `EventBus` implementation (lines 575-633) is a thin wrapper around a Tokio broadcast channel, which is correct and minimal. The problem is upstream: the `AgentEventKind` enum at lines 90-569 has grown to 50+ variants in a single discriminated union. String-typed IDs in some variants contradict typed IDs in others. The broadcast channel's silent-drop-on-overflow behavior at line 610 is the most operationally dangerous issue — a monitoring system that silently loses events will produce misleading dashboards.

**FIX-D-01** `[OPEN]`
`surface: events` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `events.rs:597` — `fetch_add(1, Ordering::Relaxed)`. Relaxed ordering means the assigned ID may be observed out-of-order by subscribers, producing non-monotonic event IDs.
- *Operation.* Change to `fetch_add(1, Ordering::AcqRel)` on write; `load(Ordering::Acquire)` on read.
- *Success.* 1000-event concurrent test: subscriber sees monotonically non-decreasing IDs.

**FIX-D-02** `[OPEN]`
`surface: events` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `events.rs:610` — `let _ = self.sender.send(event)` silently discards `SendError` (channel at capacity). Subscribers miss events with no indication.
- *Operation.* Replace with: if send fails, emit `tracing::warn!(dropped_event = ?event.kind, "event bus at capacity; event dropped")` and increment `EVENTS_DROPPED` counter metric.
- *Success.* Forcing channel overflow in a test shows a WARN log and a non-zero counter.

**FIX-D-03** `[OPEN]`
`surface: events` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: L`
- *Problem.* 50+ variants in `AgentEventKind` make adding, reading, and matching events error-prone. Pattern-matching the whole enum to handle task events means touching agent and system events.
- *Operation.* Split into `TaskEventKind`, `AgentEventKind`, `SystemEventKind`. Wrap in `enum OrchestratorEvent { Task(TaskEventKind), Agent(AgentEventKind), System(SystemEventKind) }`. Update all match sites.
- *Success.* Each sub-enum has ≤20 variants; `rg 'AgentEventKind::Task' crates/` returns zero hits.

**FIX-D-04** `[OPEN]`
`surface: events` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: M`
- *Problem.* `ActivityStarted`, `ActivityCompleted`, `ActivityRetried` (lines 406-415, 420, 424) use raw `String` for `agent_id` and `task_id`, while other variants use typed `AgentId`/`TaskId`.
- *Operation.* Replace `String` with `AgentId` / `TaskId` in these three variants. Update constructors at call sites.
- *Success.* `rg 'ActivityStarted.*String\|ActivityCompleted.*String' crates/vox-orchestrator/src/events.rs` returns zero hits.

**FIX-D-05** `[OPEN]`
`surface: events` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: M`
- *Problem.* `TaskStarted`, `TaskFailed`, `TaskCompleted` repeat `(task_id, agent_id, session_id, audit_report)` fields.
- *Operation.* Extract `struct TaskEventCore { task_id, agent_id, session_id, audit_report }`. Embed in each variant.
- *Success.* Three variants share one struct; adding a new task field is a one-line change.

**FIX-D-06** `[OPEN]`
`surface: events` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* No metric for event bus subscriber count or throughput; cannot diagnose slow subscribers causing channel pressure.
- *Operation.* Expose `subscriber_count()` result as a Prometheus gauge `vox_orchestrator_event_bus_subscribers`. Add `events_emitted_total` counter.
- *Success.* `curl /metrics` shows both gauges.

**FIX-D-07** `[OPEN]`
`surface: events` | `axis: risk` | `concern: correctness` | `P2` | `effort: M`
- *Problem.* Channel capacity is implicit (Tokio broadcast default). No documented policy on lossy vs. lossless semantics. Dashboard and audit consumers need lossless; monitoring consumers can be lossy.
- *Operation.* Write `contracts/communication/agent-event.v1.schema.json` including a `channel_policy: lossy | lossless` field. Create two bus instances: a lossless unbounded channel for audit (backed by a ring-buffer spill to disk), a lossy broadcast for real-time monitoring.
- *Success.* Audit consumer test verifies zero events dropped under 10k event burst.

**FIX-D-08** `[OPEN]`
`surface: events` | `axis: capability` | `concern: capability` | `P2` | `effort: M`
- *Problem.* No lock-propagation event variant. An agent performing a DB schema mutation cannot inform other agents to pause data-retrieval operations. (Gap identified in nextgen research §8.2.)
- *Operation.* Add `SystemEventKind::LockAcquired { resource_id, holder_agent_id, ttl_ms }` and `LockReleased { resource_id }`. Emit from `acquire_distributed_lock`. Agents subscribed to `SystemEventKind` can pause on LockAcquired.
- *Success.* Test: agent A acquires schema lock → agent B's data-retrieval tool pauses until `LockReleased` emitted.

---

### Cluster E — Grounding (`grounding.rs`)

**Design Note.** Grounding is the citation and evidence verification layer. The implementation is sound conceptually but `split_summary_into_claim_segments()` at lines 139-270 (132 lines) embeds Unicode boundary logic, German locale special-casing, and claim segmentation in one function with nested helpers. This creates brittleness for non-English content and makes the function resistant to testing. The `expect("char boundary")` at line 235 is the only hard-crash path found.

**FIX-E-01** `[OPEN]`
`surface: grounding` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `grounding.rs:235` — `.expect("char boundary")` when indexing `summary[i..]`. If summary contains corrupted UTF-8 or if `i` lands on a multi-byte boundary, panics in production.
- *Operation.* Replace with `summary.get(i..).ok_or(GroundingError::InvalidUtf8Boundary(i))?`. Add fuzz test with random byte strings.
- *Success.* Fuzz corpus of 10k random strings never panics; test with mid-codepoint boundary returns Err.

**FIX-E-02** `[OPEN]`
`surface: grounding` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: M`
- *Problem.* `split_summary_into_claim_segments` (132 lines) mixes char-boundary detection, locale rules, and segmentation loop. Unit testing one concern requires exercising all.
- *Operation.* Extract: `fn detect_char_boundary(s: &str, i: usize) -> Option<usize>`, `fn apply_locale_rules(token: &str) -> bool`, `fn segment_loop(s: &str) -> Vec<Range<usize>>`. Keep top-level as coordinator.
- *Success.* Each extracted fn has its own unit test; top-level fn ≤30 lines.

**FIX-E-03** `[OPEN]`
`surface: grounding` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: M`
- *Problem.* `classify_line_claim_kind()` (lines 79-125) hardcodes English phrases ("according to", "suggests that", etc.). Non-English citations are unclassifiable.
- *Operation.* Move phrase list to `contracts/grounding/claim-phrases.v1.yaml` keyed by language code. Load at startup. Default to `en`. Add `--grounding-locale` flag.
- *Success.* Adding German phrases requires only a YAML edit; existing English tests still pass.

**FIX-E-04** `[OPEN]`
`surface: grounding` | `axis: risk` | `concern: testing` | `P2` | `effort: M`
- *Problem.* No tests for edge cases in claim segmentation: emoji sequences, surrogate pairs, empty summary, mixed-script (Latin + CJK), German compound streets.
- *Operation.* Add `crates/vox-orchestrator/tests/grounding_tests.rs` with a parameterized test for each edge case.
- *Success.* All 8 edge cases pass; no panics.

**FIX-E-05** `[OPEN]`
`surface: grounding` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: S`
- *Problem.* `grounding.rs:153-156` and `163-164` duplicate suffix checks for `"str"` and `"strasse"`. Four lines of near-identical code.
- *Operation.* Extract `fn is_street_suffix(token: &str) -> bool` covering all street variants.
- *Success.* Duplication gone; adding "straße" (ß variant) is a one-liner.

**FIX-E-06** `[OPEN]`
`surface: grounding` | `axis: capability` | `concern: capability` | `P2` | `effort: L`
- *Problem.* Socrates confidence is computed from evidence heuristics, not from model token probability distributions. Hallucination entropy scoring from logprobs would give a model-native confidence signal. (nextgen research §6.2.)
- *Operation.* When provider returns `logprobs` (OpenAI/OpenRouter support; flag required), compute per-token entropy `H = -Σ p log p` over the completion. Average entropy > configurable threshold → `confidence_estimate` lowered. Feed into existing `SocratesGateOutcome.confidence`.
- *Success.* High-entropy completion on a factual question lowers Socrates confidence score; test with known uncertain model output.

---

### Cluster F — Usage / FinOps (`usage.rs`)

**Design Note.** `usage.rs` implements per-provider cost tracking and daily budget enforcement. The global static Mutex at lines 256-261 is the most severe performance bottleneck in the non-routing surface: it serialises every API call through a single lock, which is untenable at scale. The custom `today()` date implementation at lines 183-200 is mathematically correct but adds cognitive overhead and leap-second edge-case risk when `chrono` is already a transitive dependency.

**FIX-F-01** `[OPEN]`
`surface: usage-finops` | `axis: risk` | `concern: perf` | `P0` | `effort: M`
- *Problem.* `usage.rs:256-261` — `OnceLock<Mutex<()>>` serialises all `record_call_detailed` calls. Every API call (potentially hundreds per second) blocks on this single global lock.
- *Operation.* Replace with a `tokio::sync::mpsc::Sender<UsageRecord>` channel. Spawn a single writer task that drains the channel and batches writes to DB. `record_call_detailed` becomes a non-blocking `channel.send(record).ok()` call.
- *Success.* Load test: 500 concurrent `record_call_detailed` calls complete in <100ms total vs. serialized baseline.

**FIX-F-02** `[OPEN]`
`surface: usage-finops` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `usage.rs:265-268` — `.as_u64().unwrap_or(0)` and `.as_f64().unwrap_or(0.0)` silently accept schema drift. A column type change produces wrong math with no error.
- *Operation.* Replace with explicit type-check: `value.as_u64().ok_or(UsageError::SchemaFieldTypeMismatch { key, expected: "u64" })?`. Fail-open only in `Profile::Dev`.
- *Success.* Test with wrong-type JSON field returns Err in production profile; passes with fallback in dev.

**FIX-F-03** `[OPEN]`
`surface: usage-finops` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: S`
- *Problem.* `usage.rs:183-200` — custom date calculation. `chrono` is already a transitive dependency.
- *Operation.* Replace with `chrono::Utc::now().date_naive()`. Remove the custom function.
- *Success.* `rg 'fn today' crates/vox-orchestrator/src/usage.rs` returns zero hits.

**FIX-F-04** `[OPEN]`
`surface: usage-finops` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: S`
- *Problem.* `record_call()` (lines 203-226) just calls `record_call_detailed()` with constant defaults. An extra indirection with no added value.
- *Operation.* Inline the defaults at call sites or make `record_call_detailed()` take an `impl Into<UsageRecord>` so callers can use a builder.
- *Success.* `rg 'fn record_call\b' crates/vox-orchestrator/src/usage.rs` returns one hit.

**FIX-F-05** `[OPEN]`
`surface: usage-finops` | `axis: capability` | `concern: capability` | `P1` | `effort: L`
- *Problem.* `BudgetManager::max_financial_cost_micros` is a single global cap. No per-tenant or per-fleet-segment budget (nextgen research §4.2 P1 gap).
- *Operation.* Add `TenantBudget { tenant_id, daily_cap_micros, current_spend }` table. On `record_call_detailed`, look up tenant from `AgentTask.tenant_id` and enforce. Emit `BudgetExceeded { tenant_id }` event. `vox config budgets set <tenant> <amount>`.
- *Success.* Test: two tenants with separate caps; one exhausting budget does not affect the other.

**FIX-F-06** `[OPEN]`
`surface: usage-finops` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* Budget utilization is only checkable by querying DB. No real-time observable.
- *Operation.* Add `/v1/budget/status` HTTP endpoint (gateway, authenticated) returning `{ global: { cap, spend, remaining }, tenants: [...] }`.
- *Success.* `curl /v1/budget/status` returns JSON; integration test asserts spend updates after a task.

**FIX-F-07** `[OPEN]`
`surface: usage-finops` | `axis: risk` | `concern: correctness` | `P2` | `effort: M`
- *Problem.* No unit tests for budget enforcement: exhaustion, tenant isolation, reconciliation.
- *Operation.* Add `crates/vox-db/tests/budget_tests.rs`: (1) spend to cap → next call rejected, (2) tenant A exhausted → tenant B unaffected, (3) reconciliation updates balance.
- *Success.* Three tests green; budget enforcement path exercised.

**FIX-F-08** `[OPEN]`
`surface: usage-finops` | `axis: capability` | `concern: capability` | `P2` | `effort: M`
- *Problem.* Pre-execution token estimation uses 4 chars/token heuristic (nextgen research §4.2). Tool output length is unpredictable and not estimated.
- *Operation.* For each tool call, look up `p95_output_tokens` from `model_scoreboard` (if available). Use as the pre-execution estimate. Gate task dispatch if `estimated_cost > remaining_budget`.
- *Success.* Test: task with estimated cost > budget cap is rejected before dispatching any LLM call.

---

### Cluster G — MCP Dispatch (`mcp_tools/dispatch.rs`)

**Design Note.** `dispatch.rs` is 1,224 LOC with a `handle_tool_call_inner` match statement spanning 960 lines routing ~150 tools. This is the single largest function in the codebase. It has no per-tool timeout, no rate limiting, no backpressure, and no structured error taxonomy — all tool errors are stringly typed. The hardcoded sentinel strings (`SYSTEM_INTERVENTION`, `LAZY_GENERATION_DETECTED`, `RBAC_VIOLATION`) are used in error-handling logic but belong in typed constants. FIX-G-02 (tool registry) is the foundational change that enables most others.

**FIX-G-01** `[OPEN]`
`surface: mcp-dispatch` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `handle_tool_call_inner` has no per-tool execution timeout. A stalled tool call holds the dispatch slot indefinitely.
- *Operation.* Wrap each tool dispatch branch: `tokio::time::timeout(Duration::from_secs(tool_timeout_secs), handler()).await.map_err(|_| ToolError::Timeout { tool_name })`. Default `tool_timeout_secs = 30`; configurable per-tool via `Vox.toml [orchestrator.tool_limits]`.
- *Success.* Test: a handler that sleeps 60s returns `ToolError::Timeout` within 31s.

**FIX-G-02** `[OPEN]`
`surface: mcp-dispatch` | `axis: risk` | `concern: correctness` | `P1` | `effort: M`
- *Problem.* 960-line match statement is unmaintainable. Adding a tool means touching a 1k-line file; removing one requires careful search.
- *Operation.* Introduce `ToolRegistry`: `HashMap<&'static str, Box<dyn ToolHandler + Send + Sync>>`. Each tool module exposes `fn register(r: &mut ToolRegistry)`. `handle_tool_call_inner` becomes `registry.get(tool_name)?.call(params).await`.
- *Success.* `handle_tool_call_inner` ≤20 lines; adding a tool is a new impl + one `register()` call; no match arm needed.

**FIX-G-03** `[OPEN]`
`surface: mcp-dispatch` | `axis: risk` | `concern: security` | `P1` | `effort: S`
- *Problem.* Hardcoded string sentinels `"SYSTEM_INTERVENTION"`, `"LAZY_GENERATION_DETECTED"`, `"RBAC_VIOLATION"` at dispatch.rs:60, 77, 103 are used in conditional logic. A typo breaks gating silently.
- *Operation.* Define `enum DispatchGateReason { SystemIntervention, LazyGenerationDetected, RbacViolation }`. Replace all string comparisons with enum matching.
- *Success.* `rg '"SYSTEM_INTERVENTION"\|"RBAC_VIOLATION"' crates/vox-orchestrator-mcp/src/dispatch.rs` returns zero hits.

**FIX-G-04** `[OPEN]`
`surface: mcp-dispatch` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* `dispatch.rs:50,135` — `agent_id.parse::<u64>().ok().unwrap_or(0)` silently maps parse failures to agent ID 0. Agent 0 may be a valid agent, producing misrouted calls.
- *Operation.* Return `ToolError::InvalidParam { param: "agent_id", reason: "not a valid u64" }` on parse failure instead of defaulting to 0.
- *Success.* Test: malformed agent_id string returns `ToolError::InvalidParam`, not a call to agent 0.

**FIX-G-05** `[OPEN]`
`surface: mcp-dispatch` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* No span or metric per tool call. Latency and error rates are invisible per tool.
- *Operation.* In the registry dispatch path (FIX-G-02), wrap each call in `info_span!("tool_call", tool_name = %name, agent_id = %agent_id)`. Emit `tool_calls_total{tool_name}` and `tool_call_duration_seconds{tool_name}` metrics.
- *Success.* After 10 tool calls, `/metrics` shows non-zero counts per tool.

**FIX-G-06** `[OPEN]`
`surface: mcp-dispatch` | `axis: capability` | `concern: capability` | `P2` | `effort: M`
- *Problem.* No per-tool rate limiting. A single agent can flood the dispatch layer.
- *Operation.* Add `[orchestrator.tool_limits.<tool_name>] rate_per_minute = N` in `Vox.toml`. Apply `governor::RateLimiter` per `(agent_id, tool_name)`. On exceeded: return `ToolError::RateLimited { retry_after_ms }`.
- *Success.* Test: 100 rapid calls to a limited tool returns `RateLimited` after the configured limit.

**FIX-G-07** `[OPEN]`
`surface: mcp-dispatch` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: L`
- *Problem.* All tool registrations live in `dispatch.rs`. The file is a coupling point for all tool groups.
- *Operation.* Move registration into per-group modules: `chat_tools::register()`, `scientia_tools::register()`, etc. `dispatch.rs` calls each group's register() at startup.
- *Success.* `wc -l crates/vox-orchestrator-mcp/src/dispatch.rs` < 100.

**FIX-G-08** `[OPEN]`
`surface: mcp-dispatch` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* No backpressure mechanism. When agent queue is saturated, new tool calls continue to be accepted and pile up.
- *Operation.* Add `max_pending_tool_calls_per_agent: usize` (default 50, config). On exceed: return `ToolError::QueueFull { agent_id }`. Emit `AgentEventKind::SystemEventKind::BackpressureTriggered`.
- *Success.* Test: 100 concurrent tool calls for one agent → 51st returns `QueueFull`.

**FIX-G-09** `[OPEN]`
`surface: mcp-dispatch` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: M`
- *Problem.* Tool errors are stringly-typed `ToolResult::err(string)`. Callers cannot branch on error class.
- *Operation.* Define `enum ToolErrorKind { Budget, Auth, Validation, Internal, Timeout, RateLimited, QueueFull }`. `ToolResult` carries `error_kind: Option<ToolErrorKind>`. All dispatch paths use it.
- *Success.* `rg 'ToolResult::err.*String' crates/vox-orchestrator/src/mcp_tools' returns zero untyped hits.

---

### Cluster H — Chat Tools (`mcp_tools/chat_tools/`)

**Design Note.** The chat tools cluster contains the planning pipeline, which is the most user-visible MCP surface. `plan_goal()` at `plan.rs:186-699` is 514 lines — the largest single function in the tooling layer. It builds prompts, resolves models, synthesizes tasks, handles budget, and formats output all in one pass. `plan_loop.rs` `maybe_refine_plan()` is 337 lines. Both suffer from hardcoded numeric constants scattered across 8+ locations. Three `.expect()` calls in production code are P1 crash risks.

**FIX-H-01** `[OPEN]`
`surface: chat-tools` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `chat_tools/mod.rs:176` — `.expect("telemetry JSON must parse")`. If telemetry struct serialization fails (e.g., serde_json panic on NaN float), process crashes.
- *Operation.* Replace with `.unwrap_or_else(|e| { tracing::error!(err = %e, "telemetry serialization failed"); Value::Null })`.
- *Success.* Test: serializing a struct with NaN field returns `Value::Null` and a WARN log; no panic.

**FIX-H-02** `[OPEN]`
`surface: chat-tools` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `chat_tools/mod.rs:214` — `.expect("search_refinement field")`. Field may be absent if schema evolves.
- *Operation.* Replace with `.and_then(|v| v.get("search_refinement")).cloned().unwrap_or(Value::Null)`.
- *Success.* Test: JSON without search_refinement field returns Null; no panic.

**FIX-H-03** `[OPEN]`
`surface: chat-tools` | `axis: risk` | `concern: correctness` | `P1` | `effort: S`
- *Problem.* `chat/mentions.rs:9` — `Regex::new(pattern).unwrap()` in a `LazyLock`. If pattern is invalid (e.g., after editing), process panics on first use.
- *Operation.* Replace with `Regex::new(pattern).expect("mentions regex: compile error — check pattern")`. Add a unit test that instantiates the regex.
- *Success.* Deliberate syntax error in pattern fails at test time, not at mention parse time.

**FIX-H-04** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: modularity` | `P1` | `effort: L`
- *Problem.* `plan.rs:186-699` (514 lines) mixes prompt construction, model resolution, task synthesis, budget accounting, and output formatting in one function.
- *Operation.* Extract a `PlanningPipeline` with stages: `build_context() -> PlanContext`, `resolve_model(ctx) -> ModelHandle`, `synthesize_tasks(ctx, model) -> Vec<PlanNode>`, `enforce_budget(nodes) -> Vec<PlanNode>`, `format_output(nodes) -> PlanResult`. `plan_goal()` orchestrates the pipeline.
- *Success.* `plan_goal()` ≤60 lines; each stage has its own unit test; total coverage of error paths increases.

**FIX-H-05** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: M`
- *Problem.* `plan_loop.rs:151-487` (337 lines) mixes refinement logic, budget tracking, and result formatting.
- *Operation.* Extract: `assess_adequacy(plan, budget) -> AdequacyResult`, `refine_once(plan, budget, model) -> RefinedPlan`, `check_round_limit(round, max) -> Result<(), PlanError>`. Loop wrapper ≤50 lines.
- *Success.* Each extracted fn tested independently.

**FIX-H-06** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* Token caps at `plan.rs:55-60` are hardcoded `const` values (3072, 4096, 8192) keyed to plan depth. Changing them requires a code change and rebuild.
- *Operation.* Move to `Vox.toml [orchestrator.planning.token_caps_by_depth]` as an array. Read at plan creation time. Keep Rust consts as compile-time fallbacks.
- *Success.* Changing caps in `Vox.toml` takes effect without recompile; `vox config planning show` prints active caps.

**FIX-H-07** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* `plan.rs:187` `max_tasks.unwrap_or(30)` and `plan_loop.rs:119` `.min(8)` round cap are magic numbers with no operator control.
- *Operation.* Add `Vox.toml [orchestrator.planning] max_tasks_default = 30` and `max_refine_rounds = 8`. Operators can increase for large codebases.
- *Success.* Setting `max_tasks_default = 50` in Vox.toml is reflected in plan generation.

**FIX-H-08** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* `plan_loop.rs:103` `reserve_tokens = 4096_u32` and `:164` `refine_budget_tokens.unwrap_or(18_000)` are hardcoded.
- *Operation.* Move to config keys `planning.reserve_tokens` and `planning.refine_budget_tokens`.
- *Success.* Config keys documented; tests pass with non-default values.

**FIX-H-09** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* `plan_loop.rs:162` `.unwrap_or(0.28_f32)` refinement adequacy threshold is unexplained magic.
- *Operation.* Add `Vox.toml [orchestrator.planning] adequacy_threshold = 0.28` with a comment explaining the 0–1 scale. Document: below threshold → refinement triggered.
- *Success.* Threshold documented; unit test uses non-default value.

**FIX-H-10** `[OPEN]`
`surface: chat-tools` | `axis: risk` | `concern: correctness` | `P2` | `effort: M`
- *Problem.* `plan_loop.rs` refinement loop enforces `max_refine_rounds` but has no guard against the adequacy check always returning false (degenerate case → runs max rounds regardless).
- *Operation.* After `max_refine_rounds` without adequacy passing: emit `AgentEventKind::PlanRefinementExhausted` and return the best plan seen so far, tagged as `quality: BestEffort`.
- *Success.* Test: always-inadequate adequacy stub terminates after `max_refine_rounds` with a `BestEffort` result.

**FIX-H-11** `[OPEN]`
`surface: chat-tools` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* Plan generation is a black box in traces. Duration, task count, depth, and refinement rounds are invisible.
- *Operation.* Emit `tracing::info_span!("plan_generation", task_count, depth, refine_rounds, budget_tokens)` wrapping `plan_goal` execution.
- *Success.* A plan trace shows the span with non-zero attributes.

**FIX-H-12** `[OPEN]`
`surface: chat-tools` | `axis: risk` | `concern: testing` | `P2` | `effort: M`
- *Problem.* No test coverage for plan error paths: zero tasks, budget exceeded on synthesis, replan with empty base plan.
- *Operation.* Add `crates/vox-orchestrator/tests/plan_tests.rs` covering: (1) zero-task result, (2) budget cap hit before all tasks generated, (3) replan with no prior nodes, (4) refinement budget exhausted.
- *Success.* Four tests green.

**FIX-H-13** `[OPEN]`
`surface: chat-tools` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: S`
- *Problem.* `plan.rs:495` — `.unwrap_or(None).unwrap_or(0)` chained Option. The middle `unwrap_or(None)` is a no-op that adds confusion.
- *Operation.* Replace with `.flatten().unwrap_or(0)` or restructure the Option chain.
- *Success.* `rg 'unwrap_or(None)' crates/vox-orchestrator/src/mcp_tools/chat_tools/plan.rs` returns zero hits.

---

### Cluster I — Scientia Tools (`mcp_tools/scientia_tools/`)

**Design Note.** The Scientia tools implement the scholar research lifecycle: preflight, discovery, novelty checking, publication, and external job submission. The implementation is defensively coded (mostly safe `.unwrap_or()` patterns) but leaks policy into constants: score thresholds at 0.85, 0.62, 0.58 are embedded in code with no documentation of their derivation. External job submission loops have no retry limit guard. Functions over 100 lines in `novelty.rs` and `lifecycle.rs` need decomposition.

**FIX-I-01** `[OPEN]`
`surface: scientia-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* Score thresholds (0.85, 0.62, 0.58) in `novelty.rs` are undocumented magic numbers.
- *Operation.* Add `Vox.toml [orchestrator.scientia] novelty_high_threshold = 0.85 # reject as duplicate`, `novelty_medium_threshold = 0.62 # flag for review`, etc. Document thresholds in `docs/src/reference/scientia-thresholds.md`.
- *Success.* `rg '0\.85\|0\.62\|0\.58' crates/vox-orchestrator/src/mcp_tools/scientia_tools' returns zero code hits (moved to config read).

**FIX-I-02** `[OPEN]`
`surface: scientia-tools` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* `common.rs:87` `.unwrap_or(0.85)` evidence threshold is separate from the novelty thresholds and undocumented.
- *Operation.* Add `Vox.toml [orchestrator.scientia] evidence_confidence_floor = 0.85`. Document its role (minimum confidence to count a source as evidence).
- *Success.* Config key present and documented; test with non-default value shows different gating.

**FIX-I-03** `[OPEN]`
`surface: scientia-tools` | `axis: risk` | `concern: correctness` | `P2` | `effort: M`
- *Problem.* `external.rs` job submission and replay loops have no explicit retry limit. A stuck external job could loop indefinitely.
- *Operation.* Add `max_retries: u32` (default 3) and `retry_backoff_ms: u64` (default 1000, exponential) to external job config. After max retries: emit `JobFailed { job_id, reason: "max_retries_exceeded" }`.
- *Success.* Test: mock external endpoint always returns 500 → job fails after 3 retries with structured error.

**FIX-I-04** `[OPEN]`
`surface: scientia-tools` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* Scholar lifecycle events (submit, discover, publish) are not emitted to the event bus. Orchestrator cannot react to publication completion.
- *Operation.* Add `TaskEventKind::ScholiaSubmitted`, `ScholiaDiscovered`, `ScholiaPublished` variants. Emit from `lifecycle.rs` at each milestone.
- *Success.* Integration test: publishing a scholia emits `ScholiaPublished` event visible to a subscriber.

**FIX-I-05** `[OPEN]`
`surface: scientia-tools` | `axis: risk` | `concern: testing` | `P2` | `effort: M`
- *Problem.* No tests for edge cases: novelty check with empty corpus, discovery returning no results, lifecycle failure mid-way.
- *Operation.* Add `crates/vox-orchestrator/tests/scientia_tests.rs`: (1) empty corpus → `NoveltyCheckSkipped`, (2) discovery with zero results → `DiscoveryEmpty`, (3) lifecycle step failure → partial state rollback.
- *Success.* Three tests green.

**FIX-I-06** `[OPEN]`
`surface: scientia-tools` | `axis: hygiene` | `concern: modularity` | `P2` | `effort: L`
- *Problem.* `novelty.rs` and `scholar.rs` contain functions >100 lines mixing computation logic with side effects (DB writes, event emission).
- *Operation.* For each >100-line function: separate pure computation (returns data) from side effects (writes to DB, emits events). Apply the same stage-extraction pattern as FIX-H-04.
- *Success.* No function in scientia_tools > 80 lines; each stage independently testable.

**FIX-I-07** `[OPEN]`
`surface: scientia-tools` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* Evidence refresh calls in `discovery.rs` have no minimum refresh interval. An agent could trigger rapid re-fetches.
- *Operation.* Apply the same `min_refresh_interval_secs` pattern as OpenRouter catalog (from prior audit FIX-28). Gate discovery refresh calls with a per-session TTL.
- *Success.* Rapid consecutive refresh calls: only first executes; subsequent return cached result.

---

### Cluster J — LLM Bridge (`mcp_tools/llm_bridge/`)

**Design Note.** The LLM bridge is the provider-facing inference layer: it translates tool call params into provider-specific HTTP requests and normalizes responses. `infer.rs::mcp_infer_tool_completion` (390 lines) is the second-largest function in the tooling layer. The most critical gap is the absence of explicit HTTP timeout Durations on any provider client — a hanging provider connection will hold the dispatch slot and eventually exhaust the thread pool. The `Result<T, String>` error type prevents structured error handling upstream.

**FIX-J-01** `[OPEN]`
`surface: llm-bridge` | `axis: risk` | `concern: correctness` | `P0` | `effort: M`
- *Problem.* No explicit `Duration` timeout found in any provider HTTP client setup in `llm_bridge/`. A stalled provider holds the connection indefinitely.
- *Operation.* Add `connect_timeout(Duration::from_secs(10))` and `timeout(Duration::from_secs(provider_timeout_secs))` to every provider's `reqwest::Client::builder()`. Default 60s; configurable per provider in `Vox.toml [providers.<name>] timeout_secs`.
- *Success.* Test: provider that hangs for 120s is aborted after 61s with `ProviderError::Timeout`.

**FIX-J-02** `[OPEN]`
`surface: llm-bridge` | `axis: hygiene` | `concern: contracts` | `P1` | `effort: M`
- *Problem.* `infer.rs:190,224` returns `Result<(String, String, u64), String>`. String errors cannot be matched by callers.
- *Operation.* Define `enum ProviderError { Timeout, RateLimited { retry_after_ms }, Unauthorized, ServiceError { status: u16, body: String }, Serialization(serde_json::Error) }`. Replace all `Err(String)` in llm_bridge/.
- *Success.* `rg 'Result<.*String>' crates/vox-orchestrator/src/mcp_tools/llm_bridge' returns zero hits.

**FIX-J-03** `[OPEN]`
`surface: llm-bridge` | `axis: hygiene` | `concern: modularity` | `P1` | `effort: L`
- *Problem.* `mcp_infer_tool_completion` (390 lines, infer.rs:210-599) handles vision tokens, model selection, context packing, streaming, provider dispatch, fallback, and response normalization in one function.
- *Operation.* Extract pipeline stages: `pack_context(params) -> PackedContext`, `dispatch_to_provider(ctx, model) -> ProviderResponse`, `normalize_response(raw) -> InferResult`. Orchestrating wrapper ≤50 lines.
- *Success.* Each stage unit-tested; `mcp_infer_tool_completion` ≤50 lines.

**FIX-J-04** `[OPEN]`
`surface: llm-bridge` | `axis: hygiene` | `concern: contracts` | `P2` | `effort: S`
- *Problem.* `infer.rs:278` — `estimated_vision_tokens += 1000`. Hardcoded estimate regardless of image size or model context expectations.
- *Operation.* Replace with `model_spec.vision_tokens_per_image.unwrap_or(1000)` — add `vision_tokens_per_image: Option<u32>` to `ModelSpec` populated from catalog (OpenRouter returns image token cost).
- *Success.* Vision-capable models show provider-specific estimates; fallback to 1000 documented.

**FIX-J-05** `[OPEN]`
`surface: llm-bridge` | `axis: risk` | `concern: correctness` | `P2` | `effort: M`
- *Problem.* HTTP 429/408/504 are detected at `infer.rs:544-565` but no exponential backoff is applied. Provider is immediately retried, worsening rate-limit conditions.
- *Operation.* On 429: wait `retry_after` header value (or 1s default), then retry up to 3 times with exponential backoff (1s, 2s, 4s). On 408/504: retry once after 2s. Log each attempt with `ProviderError::attempt_number`.
- *Success.* Test: mock 429 → after 3 retries returns `ProviderError::RateLimited`; retry_after respected.

**FIX-J-06** `[OPEN]`
`surface: llm-bridge` | `axis: capability` | `concern: observability` | `P2` | `effort: S`
- *Problem.* Provider attempt details (status code, attempt number, provider name) are logged but not in structured spans.
- *Operation.* Emit `tracing::info_span!("provider_attempt", provider, attempt_number, status_code, latency_ms)` for each attempt. Link to parent `trace_id` from `AgentTask`.
- *Success.* A retried call shows multiple child spans under the parent trace.

**FIX-J-07** `[OPEN]`
`surface: llm-bridge` | `axis: capability` | `concern: capability` | `P2` | `effort: M`
- *Problem.* Routing to Anthropic via OpenRouter can silently fail on tool-call chains due to schema differences (nextgen research §3.1). No schema-aware translation layer.
- *Operation.* Add `fn translate_tool_calls(messages: &mut Vec<Message>, target: ProviderType)` that enforces Anthropic's strict alternating-turn structure when `target == Anthropic`. Apply in `provider_adapter.rs` before dispatch.
- *Success.* Test: OpenAI-style tool result chain translated to Anthropic format passes Anthropic validation rules.

**FIX-J-08** `[OPEN]`
`surface: llm-bridge` | `axis: risk` | `concern: correctness` | `P2` | `effort: S`
- *Problem.* Many provider response field fallbacks use `.unwrap_or_default()` silently. A changed provider response shape produces wrong token counts or costs.
- *Operation.* Add `validate_provider_response(raw: &Value, provider: ProviderType) -> Result<(), ProviderError>` that checks required fields before extraction. Log a WARN if optional fields are unexpectedly absent.
- *Success.* Test: response missing `usage.input_tokens` field → validation WARN emitted; call still succeeds with 0 tokens.

**FIX-J-09** `[OPEN]`
`surface: llm-bridge` | `axis: hygiene` | `concern: modularity` | `P3` | `effort: M`
- *Problem.* Per-provider adapters (gemini.rs, anthropic.rs, openai.rs, ollama_chat.rs) share structure but lack a common trait. Adding a new provider requires copying boilerplate.
- *Operation.* Formalise `trait ProviderAdapter { fn adapt_request(&self, ctx: &PackedContext) -> Value; fn extract_response(&self, raw: &Value) -> InferResult; }`. Each file implements it. `dispatch_to_provider` calls the trait.
- *Success.* New provider requires only implementing the two trait methods.

**FIX-J-10** `[OPEN]`
`surface: llm-bridge` | `axis: capability` | `concern: capability` | `P2` | `effort: M`
- *Problem.* No PII-aware routing. Tasks involving sensitive data (medical records, credentials) can be routed to any provider including cloud providers (nextgen research §3.2 P1 gap).
- *Operation.* Add `AgentTask.sensitivity: Option<SensitivityLevel { Public, Internal, Restricted }>`. In `best_for()` (prior audit) and `mcp_infer_tool_completion`, filter out cloud providers when `sensitivity == Restricted`; allow only `PopuliMesh` or `Ollama`.
- *Success.* Test: `Restricted` task never dispatched to `OpenRouter` or `Anthropic`.

---

### Cluster K — HTTP Gateway (`mcp_tools/http_gateway/`)

**Design Note.** The HTTP gateway received significant recent hardening (bearer auth, CORS, origin guard — see recent commits). Three P0/P1 security issues remain: `DEBUG println!` leaking env vars (line 176-177), an origin guard `starts_with` prefix-bypass, and WebSocket tokens in URL query parameters appearing in access logs. The eval endpoint runs untrusted code with only a 5-second wall-clock timeout and no input size limit — a CPU-bound loop can exhaust the timeout but continue burning CPU. All FIX-K items are S/M effort; P0 and P1 items should be landed in the next sprint.

*(FIX-K-01 through FIX-K-18 are fully specified in Part 2 above — all mechanical operations with clear success criteria. Key security items: FIX-K-01 and FIX-K-02 are P0, should land together.)*

---

### Cluster L — Memory Tools (`mcp_tools/memory_tools/`)

**Design Note.** Memory tools are the most defensively-written cluster in the MCP layer. The `err_with_remediation()` pattern with typed `REM_*` constants is excellent — it should be adopted as the standard across all MCP tool handlers (see FIX-X-03). The main gaps are operational: hardcoded item limits (10, 20) that should be operator-tunable, absence of rate limiting, and unclear memory isolation semantics between agents.

*(FIX-L-01 through FIX-L-05 fully specified in Part 2.)*

---

### Cluster M — Task Tools (`mcp_tools/task_tools/`)

**Design Note.** Task tools manage task submission, completion, and cancellation. The repeated companion lookup/upsert pattern (3×, lifecycle.rs) is a copy-paste debt risk — one of the three could silently diverge. Gamification companion updates are fire-and-forget async tasks with no error logging, so companion state can silently desync from task state. Priority string parsing with a silent `_` → Normal default is a foot-gun.

*(FIX-M-01 through FIX-M-05 fully specified in Part 2.)*

---

### Cluster N — DEI Shim (`dei_shim/`)

**Design Note.** `dei_shim/` is a 2,769 LOC legacy compatibility layer retained after the `vox-dei` crate retirement. The nomenclature convergence commits (Mens→Populi, etc.) reduced the shim's necessity but did not remove it. Its continued presence creates import ambiguity, keeps retired symbol names alive in code paths, and contradicts the retirement policy in `AGENTS.md:140`. The plan below follows a conservative deprecation path: annotate → migrate callers → gate with feature flag → remove.

*(FIX-N-01 through FIX-N-05 fully specified in Part 2.)*

---

### Cluster O — Daemon Binary + ADR-022 Boundaries

**Design Note.** `vox_orchestrator_d.rs` is the daemon entry point. Three correctness gaps stand out: no signal handler (SIGTERM leaves the process without flushing the outbox), DB init failure at line 75 that logs a warning and continues (rather than halting), and a `panic!` in `SessionManager` initialization at line 120. The ADR-022 Phase B split-plane RPC flag matrix is implemented but not documented in the ADR (which was written before Phase B was completed).

*(FIX-O-01 through FIX-O-09 fully specified in Part 2.)*

---

### Cluster P — Persistence Outbox + `ops_orchestrator.rs`

**Design Note.** The persistence outbox is the degraded-mode safety net: when DB is unavailable, operations are queued and replayed on reconnect. Two correctness gaps: (1) the fence token at line 45-56 has no overflow guard at `i64::MAX` — a long-lived deployment accumulates tokens indefinitely, (2) clock-skew between nodes can cause a lock to appear expired when it is actually valid. The outbox schema uses `additionalProperties: true` and has no version field, making schema evolution undetectable.

*(FIX-P-01 through FIX-P-10 fully specified in Part 2.)*

---

### Cluster Q — Planning (`planning/`)

**Design Note.** The planning module contains routing, synthesis, and replanning policy. Policy selection at `policy.rs:6-10` is string-based and case-insensitive with no compile-time verification. An infinite replan loop is possible if the replanning condition never resolves. The planning module's relationship to `chat_tools/plan.rs` is unclear — both deal with plan generation but apparently at different abstraction levels.

*(FIX-Q-01 through FIX-Q-05 fully specified in Part 2.)*

---

### Cluster R — Scaling & Services (`services/`, `scaling.rs`)

**Design Note.** The scaling and services layer wraps embeddings, routing-policy shim, gateway helpers, and agent count management. `check_scaling` at lines 657-771 is well-structured. The bundled `services/` directory lacks explicit service factory patterns, making it harder to stub for testing. Scaling decisions need idempotence guarantees and overflow-safe arithmetic.

*(FIX-R-01 through FIX-R-05 fully specified in Part 2.)*

---

### Cluster S — Agent Prompts

**Design Note.** Four agent prompt/policy documents exist: `.vox/agents/orchestrator.md` (pilot-facing operational manual), `.vox/agents/vox-orchestrator.md` (crate specialist scope), `docs/agents/orchestrator.md` (contributor-facing SSOT for boundaries), and `crates/vox-skills/skills/orchestrator.skill.md` (agent skill). The prior exploration found these are deliberately non-redundant — but they lack version stamps, making staleness undetectable. The doom-loop intervention policy and lock-propagation awareness (identified as P0/P1 capability gaps) have no representation in any prompt.

*(FIX-S-01 through FIX-S-09 fully specified in Part 2.)*

---

### Cluster T — Tests

**Design Note.** Three test files exist for the orchestrator: `orchestrator_e2e_test.rs` (675 LOC, good concurrency and watchdog coverage), `orchestrator_bootstrap_surface_parity_test.rs` (50 LOC, thin), and `ops_orchestrator_tests.rs` (144 LOC, good lock/heartbeat/circuit-breaker coverage). Critical gaps: Socrates gate enforce vs. shadow mode has no test, trust-gate-relax path has no test, handoff invariants have no dedicated test file, conflict resolution is untested, and the doom-loop detector (FIX-B-11) needs a test from day one.

*(FIX-T-01 through FIX-T-14 fully specified in Part 2.)*

---

### Cluster U — Contracts

**Design Note.** Only one orchestrator-specific contract file exists: `orchestrator-persistence-outbox.schema.json`. The handoff envelope, agent event schema, and MCP dispatch error taxonomy have no machine-readable contracts. Adding schemas closes the feedback loop between code changes and external consumers (agent runtimes, CLI tools, monitoring systems).

*(FIX-U-01 through FIX-U-05 fully specified in Part 2.)*

---

### Cluster V — ADRs & Docs

**Design Note.** ADR-022 covers bootstrap and daemon boundaries but was written before the Phase B split-plane flag matrix was implemented. Two missing ADRs: event bus semantics (lossy vs. lossless lanes — a design decision with production consequences) and distributed lock fence-token discipline (clock-skew policy is undocumented, meaning future contributors will rediscover the race).

*(FIX-V-01 through FIX-V-05 fully specified in Part 2.)*

---

## Part 4 — Cross-Cutting Findings

### Cluster X — Error Handling Discipline

**Design Note.** The codebase has inconsistent error handling: `memory_tools` uses an excellent `err_with_remediation()` pattern; `llm_bridge` uses `Result<T, String>`; `runtime.rs` uses bare `Err("...")` strings; `dispatch.rs` uses `ToolResult::err(string)`. This inconsistency means callers cannot write uniform error-handling code. The fix is twofold: define a top-level `OrchestratorError` enum and standardize the `err_with_remediation()` pattern across all MCP tools.

*(FIX-X-01 through FIX-X-06 fully specified in Part 2.)*

---

### Cluster Y — Async Cancellation Safety

**Design Note.** Tokio task cancellation drops a future at the next `.await` point. Any future that holds a `RwLock` guard, a `Mutex` guard, or a DB transaction across an `.await` will leak the guard on cancellation — a deadlock waiting to happen. This is not a hypothetical: `handle_command` in `runtime.rs` holds orchestrator references across `.await` boundaries. A systematic audit is required.

*(FIX-Y-01 through FIX-Y-04 fully specified in Part 2.)*

---

### Cluster Z — Observability (Beyond GenAI)

**Design Note.** The GenAI telemetry layer is well-specified in the prior routing audit. The non-GenAI observability gap is in: slow-operation detection (no alert if a tool handler stalls for >5s), metric naming consistency (some spans still use `vox_dei::` targets), log-level audit (DEBUG statements in hot paths), and per-agent/per-queue gauges that would make scaling decisions legible in dashboards.

*(FIX-Z-01 through FIX-Z-06 fully specified in Part 2.)*

---

### Cluster AA — Security (Beyond vox-secrets/Gateway)

**Design Note.** The gateway audit (Cluster K) covers the HTTP surface. Three additional security concerns span the broader system: (1) no tracing redaction middleware — a developer can `debug!("{:?}", secret)` and leak credentials to log aggregators; (2) no replay-attack protection on A2A delivery — a duplicate `jwe_payload` delivery is currently accepted; (3) the eval endpoint can initiate outbound network connections from the subprocess (SSRF risk).

*(FIX-AA-01 through FIX-AA-07 fully specified in Part 2.)*

---

### Cluster AB — Lifecycle Correctness

**Design Note.** `Orchestrator` has no `Drop` implementation. When the process exits unexpectedly, the outbox is not flushed, the DB handle is not cleanly closed, and distributed locks may not be released. Restart recovery depends entirely on the outbox replay mechanism — which only works if the outbox was written before the crash. An RAII guard for distributed locks and a structured shutdown sequence are the two highest-value items.

*(FIX-AB-01 through FIX-AB-05 fully specified in Part 2.)*

---

## Part 5 — Staged Rollout

| Stage | FIX clusters | Theme | Gate |
|-------|-------------|-------|------|
| **1 — Immediate security** | K-01, K-02, K-03, K-05, K-06 | Remove P0 security issues | All P0 items pass security review |
| **2 — Crash prevention** | B-02, B-06, E-01, H-01, H-02, H-03, J-01, O-02, O-03, X-01 | Eliminate panics and unchecked unwraps | `cargo test -p vox-orchestrator` green; no new panics in fuzz |
| **3 — Correctness & tests** | C-05, D-01, D-02, F-02, G-04, T-01..T-07, Y-01, Y-02, AB-01, AB-02 | Add critical test coverage; fix ordering bugs; shutdown/RAII | Test count increases by ≥40; CI green |
| **4 — FinOps & capability** | B-11, F-01, F-05, F-08, G-01, G-06, G-08, J-05, J-10 | Doom-loop, budget isolation, per-tool limits, backpressure | Load test passes; doom-loop test passes |
| **5 — Modularity** | B-01, B-04, B-05, G-02, G-07, H-04, H-05, I-06, J-03 | Decompose large functions; introduce tool registry | No regression; dispatch.rs < 100 lines |
| **6 — Observability** | D-06, F-06, G-05, J-06, K-18, Z-01..Z-06, B-08, I-04, M-05 | Metrics, spans, slow-op detection | `/metrics` shows orchestrator gauges; traces show per-phase spans |
| **7 — Contracts & docs** | C-04, D-07, P-03, P-04, U-01..U-05, V-01..V-05, S-01..S-09 | JSON schemas, new ADRs, prompt reconciliation | `vox ci contracts-validate` green; ADRs committed |
| **8 — DEI shim retirement** | N-01..N-05 | Full shim removal | `rg 'dei_shim' crates/vox-orchestrator/src` returns zero hits |

---

## Part 6 — Success Metrics

The following are grep-able or test-verifiable done-conditions for the audit as a whole:

- `rg 'unwrap\(\)\|expect\(' crates/vox-orchestrator/src/**/*.rs | grep -v test | grep -v '#\[cfg(test' | wc -l` decreases by ≥70% from baseline.
- `rg 'Err(String\|Result<.*String>' crates/vox-orchestrator/src | grep -v test | wc -l` returns zero.
- `cargo test -p vox-orchestrator` includes ≥6 new handoff tests, ≥6 Socrates gate tests, ≥5 outbox replay tests.
- `rg '"SYSTEM_INTERVENTION"\|"RBAC_VIOLATION"\|"LAZY_GENERATION"' crates/vox-orchestrator/src` returns zero.
- `rg 'target: "vox_dei' crates/ | grep -v archive | wc -l` returns zero.
- `rg 'dei_shim' crates/vox-orchestrator/src | grep -v 'deprecated\|tombstone' | wc -l` returns zero (Stage 8 gate).
- `wc -l crates/vox-orchestrator-mcp/src/dispatch.rs` < 100 (post FIX-G-02/G-07).
- `curl localhost:<port>/metrics` returns `vox_orchestrator_event_bus_subscribers`, `vox_orchestrator_queue_depth`, `vox_orchestrator_active_agents`.
- `curl localhost:<port>/healthz` returns 200 (FIX-K-09/O-04).
- `cargo audit -p vox-orchestrator` returns zero high-severity advisories (FIX-AA-07).
- All 8 rollout stage gates pass in CI.

---

## Part 7 — Sources & Related Work

- [model-orchestration-ssot-audit-2026.md](model-orchestration-ssot-audit-2026.md) — FIX-01..75 covering model routing, catalog, vox-secrets secret distribution, and telemetry alignment.
- [nextgen-orchestrator-research-2026.md](nextgen-orchestrator-research-2026.md) — Research synthesis with P0–P3 capability gaps that motivated FIX-B-11 (doom-loop), FIX-D-08 (lock propagation), FIX-E-06 (entropy scoring), FIX-F-05 (fleet throttle), FIX-J-07 (schema-aware translation), FIX-J-10 (PII routing).
- [docs/src/adr/022-orchestrator-bootstrap-and-daemon-boundaries.md](../adr/022-orchestrator-bootstrap-and-daemon-boundaries.md) — ADR governing daemon/MCP boundary and Phase B RPC flag matrix.
- [docs/agents/orchestrator.md](../../agents/orchestrator.md) — Contributor-facing SSOT for orchestrator boundaries and Socrates policy.
- OpenTelemetry GenAI semantic conventions v1.37 — Informs FIX-Z-02 span naming.
- tokio-cancel-safety documentation — Informs FIX-Y-01..Y-04.
- OWASP Top 10 2025 (A01 Broken Access Control, A04 Insecure Design) — Informs FIX-K-01..K-18.

