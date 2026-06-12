---
title: "Semantic Gap Audit — 2026-05-16"
category: "Architecture SSOTs"
status: "current"
---

# Semantic Gap Audit — 2026-05-16

**Goal:** Find code that *looks* finished — compiles, passes tests, no `todo!()` markers — but whose bodies don't honor the contract their names, signatures, or doc comments imply. This is the LLM-laziness pattern that the prior surface-keyword audit missed.

**Method (M1–M4 from the audit methodology):**
- **M1** — plan-vs-code reconciliation on the four most-active in-flight plans (telemetry A–D, mesh P0–P6, MENS Mn-T1..T15, language rules P1–P5).
- **M2** — name/contract drift in six production-critical crates (`vox-compiler`, `vox-codegen`, `vox-orchestrator`, `vox-orchestrator-mcp`, `vox-publisher`, `vox-populi`).
- **M3** — silent-failure anti-patterns workspace-wide (discarded `Result`s, swallowed `Err`, empty match fallthroughs, spawned-and-forgotten tasks).
- **M4** — trait impl skeletons across plugin and runtime crates.

**Verification gate (applied to every candidate before reporting):**
1. Read the full function body (not just the line).
2. Search tests + production callers.
3. Check for tracked deferred-markers (ADR / SSOT / phase plan references).
4. Discard LOW-confidence findings.

**Result:** 8 verified findings (7 HIGH-confidence + 1 meta-finding). Several "obvious" candidates were investigated and confirmed REAL — those are listed at the end as a confidence indicator that the audit is not over-flagging.

---

## Findings (HIGH confidence)

### F1 — `validate_manifest_symbols` is a public no-op with a working implementation sitting unwired next to it

- **File:** `crates/vox-codegen-ts/src/route_manifest.rs:25-27`
- **Method:** M2 (name/contract drift)
- **Severity:** HIGH — breaks the route-manifest validation contract; broken routes pass codegen silently.

Public entry point:

```rust
/// Fail-fast checks for manifest imports: HIR must define every component/loader/pending referenced
/// by the WebIR route tree when `routes { }` is present.
pub fn validate_manifest_symbols(_web: &WebIrModule, _hir: &HirModule) -> Result<(), String> {
    Ok(())
}
```

Real implementation sitting at lines 30–68 of the same file, `#[allow(dead_code)]`:

```rust
#[allow(dead_code)]
fn validate_contract_branch(
    e: &RouteContract,
    component_names: &BTreeSet<String>,
    query_names: &BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    let path = &e.pattern;
    match meta_str(&e.meta, "component").as_deref() {
        None | Some("Unknown") => {
            errors.push(format!("route manifest: route {path:?} has no resolved component in WebIR meta"));
        }
        Some(c) => {
            if !component_names.contains(c) {
                errors.push(format!("route manifest: component `{c}` (path {path:?}) has no matching generated .tsx"));
            }
        }
    }
    if let Some(l) = meta_str(&e.meta, "loader") {
        if !query_names.contains(&l) {
            errors.push(format!("route manifest: loader `{l}` (path {path:?}) is not declared as @query"));
        }
    }
    // ... and so on for pending + recursion into children
}
```

Caller propagates with `?` at line 75, expecting real validation to bubble errors:

```rust
pub fn try_emit_route_manifest_from_web_ir(
    web: &WebIrModule,
    hir: &HirModule,
) -> Result<Option<String>, String> {
    validate_manifest_symbols(web, hir)?;   // <-- expects real errors, gets Ok(()) unconditionally
    ...
}
```

**Why this is a gap:** the docstring promises fail-fast checks for component/loader/pending symbols; the caller's `?` operator says "errors from this should abort codegen"; the body is `Ok(())`. The intended logic exists 4 lines below but was never wired up. Routes referencing non-existent components, loaders, or pending states pass codegen and produce broken `routes.manifest.ts` output that fails at runtime instead of build time.

**Reproducer:** craft a `.vox` file with `routes { "/" to NonExistentComponent }` and run `vox build`. Expected behavior per docstring: build aborts with a validation error. Actual: builds clean, manifest references a non-existent symbol.

**No production tests** were found for `validate_manifest_symbols` itself.

---

### F2 — Endpoint reliability observations silently dropped on DB write failure

- **File:** `crates/vox-orchestrator/src/services/reliability.rs:33-41` (and the same crate's `record_observation` public helper at lines 75-83)
- **Method:** M3 pattern P1 (discarded `Result`)
- **Severity:** HIGH — reliability dashboard, model-selection scoring, and rate-limit/timeout tracking all depend on these rows.

```rust
AgentEventKind::EndpointReliabilityObservation {
    endpoint_url, model_id, hallucination_proxy, contradiction_ratio,
    infra_failure, rate_limit_hit, timeout_hit, ..
} => {
    let _ = self.store.record_endpoint_observation(
        endpoint_url, model_id, *hallucination_proxy, *contradiction_ratio,
        *infra_failure, *rate_limit_hit, *timeout_hit,
    ).await;
}
```

**Why this is a gap:** the function name implies the observation is *recorded*; an `Err` from the DB layer means it was *not* recorded. Discarding the result leaves the caller (and operators) with the impression the observation persisted. Every downstream user of the `endpoint_observations` table sees an under-counted population. There is no nearby comment, no telemetry emit, no fallback buffer — just `let _ =`. Same pattern at line 75–83 in the public `record_observation` helper.

**Reproducer:** induce a DB write failure (drop the table, lock the file, etc.) and emit an `EndpointReliabilityObservation` event. Expected per name: error surfaces somewhere. Actual: silent drop.

---

### F3 — Task reliability observations silently dropped (4 match arms)

- **File:** `crates/vox-orchestrator/src/services/reliability.rs:43-58`
- **Method:** M3 pattern P1
- **Severity:** HIGH — these are the inputs to the reliability scoring used for agent trust + promotion/demotion.

```rust
AgentEventKind::TaskCompleted { agent_id, .. } => {
    let agent_str = agent_id.0.to_string();
    let _ = self.store.record_task_reliability_observation(&agent_str, true).await;
}
AgentEventKind::TaskFailed { agent_id, .. } => {
    let agent_str = agent_id.0.to_string();
    let _ = self.store.record_task_reliability_observation(&agent_str, false).await;
}
AgentEventKind::AgentHandoffAccepted { from, .. } => {
    let agent_str = from.0.to_string();
    let _ = self.store.record_task_reliability_observation(&agent_str, true).await;
}
AgentEventKind::AgentHandoffRejected { from, .. } => {
    let agent_str = from.0.to_string();
    let _ = self.store.record_task_reliability_observation(&agent_str, false).await;
}
_ => {}
```

**Why this is a gap:** same pattern as F2 but four-fold. Task pass/fail signals feed the agent-reliability score that the orchestrator uses to pick which agents to trust with future work. Silent drops corrupt the scoring inputs. Note also the trailing `_ => {}` — see F8 for that.

---

### F4 — Orchestration lineage events silently dropped

- **File:** `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:1002-1014`
- **Method:** M3 pattern P1
- **Severity:** HIGH — primary audit trail for task submissions.

```rust
let payload_str = payload.to_string();
let _ = db
    .append_orchestration_lineage_event(
        &repo,
        "task_submitted",
        task_id.0 as i64,
        Some(agent_id.0 as i64),
        session_id.as_deref(),
        None,
        None,
        None,
        Some(payload_str.as_str()),
    )
    .await;
```

**Why this is a gap:** `append_orchestration_lineage_event` is the audit-trail write for task submission. The payload it persists contains the orchestration campaign id, benchmark tier, execution role, session id, and the full submission payload as JSON — i.e., the canonical "task X was submitted at time T with these parameters" record. A failed write produces a permanent gap in the lineage record that downstream consumers (compliance, debugging, telemetry replay) cannot reconstruct. Silently swallowed.

---

### F5 — Reconstruction campaign initialization silently dropped

- **File:** `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:97-105`
- **Method:** M3 pattern P1
- **Severity:** HIGH — if campaign init fails, downstream task processing operates on a non-existent campaign id.

```rust
if let (Some(campaign_id), Some(tier)) = (task.campaign_id.clone(), task.benchmark_tier) {
    let _ = self
        .begin_reconstruction_campaign(
            campaign_id,
            tier,
            task.description.clone(),
            session_id.as_deref(),
        )
        .await;
}
```

**Why this is a gap:** if `begin_reconstruction_campaign` errs (DB write failure, invalid tier, duplicate id), the task proceeds with a `campaign_id` that has no backing campaign row. Downstream lookups will silently miss.

---

### F6 — Budget execution-time record silently dropped

- **File:** `crates/vox-orchestrator/src/budget/persistence.rs:103`
- **Method:** M3 pattern P1
- **Severity:** HIGH — directly affects budget enforcement and cost accounting.

```rust
let record = BudgetExecutionTimeRecord {
    agent_id, repository_id, duration_ms,
    timeout_budget_ms: attempted_budget_ms,
    compute_tokens_used: None,
    vendor_cost_usd_micros: None,
    attention_cost_ms: None,
    outcome: vox_db::ExecOutcome::Success,
};
let _ = db.record_exec_time(&record).await;
```

**Why this is a gap:** `record_exec_time` feeds the cost accounting that the budget gate uses to decide whether agents should be throttled. A silent failure means an agent's actual usage is under-counted versus its budget, allowing it to exceed limits without the gate catching it.

---

### F7 — `CloudSync::list_remote_json` returns empty array without deferred-marker

- **File:** `crates/vox-plugin-cloud/src/sync.rs:46-48`
- **Method:** M4 (trait impl skeleton)
- **Severity:** MEDIUM — inconsistent with sibling methods in the same impl block.

```rust
fn list_remote_json(&self, _remote_prefix: RStr<'_>) -> RResult<RString, RBoxError> {
    RResult::ROk(RString::from("[]"))
}
```

Sibling methods in the same impl block return explicit errors:

```rust
fn upload(&self, _local_path: RStr<'_>, _remote_uri: RStr<'_>) -> RResult<(), RBoxError> {
    RResult::RErr(RBoxError::new(std::io::Error::other(
        "not yet implemented; SP7 scaffold",
    )))
}
```

**Why this is a gap:** the caller of `list_remote_json` cannot distinguish between "no remote artifacts" and "feature not implemented." The whole crate is an SP7 scaffold and the right semantic for an unimplemented enumerator is `Err`, matching `upload` and `download`. The asymmetry is silent laziness.

---

### F8 (meta-finding) — Telemetry Phase B plan doc has internal contradiction with its own Phase C preview

- **File:** `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-b.md`
- **Method:** M1 (plan-vs-code reconciliation)
- **Severity:** LOW — documentation inconsistency, not a code defect.

Phase B Task 3 instructs populating `task_id`, `parent_task_id`, `trace_id`, `caller_agent_id` from `current_trace_ctx()`. The same document's "Phase C preview" section explicitly notes those fields stay `None` until Phase C lands. The code follows the preview (line 504–507 in `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs` hardcodes all four as `None`). The Phase C plan at `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-c.md` is the canonical wiring.

**Why this is worth noting:** the contradiction means a future reader of Phase B alone would believe the wiring shipped. Resolution is a one-line edit to Phase B Task 3.

---

## Confirmed REAL (audited and not flagged)

For audit honesty — these were investigated and found to honor their contracts. The 5 false-positive rate of the prior round of audit is not repeated here:

- **`vox-compiler`** — parser, HIR lowering, typechecker. Every action-verb function with a substantive doc comment has substantive body. Defensive `unreachable!()` calls in exhaustive matches are real.
- **`vox-codegen`** outside the F1 site — Rust + TS + Web IR emit stacks have real bodies; the dual emission is by design, not split-brain.
- **`vox-orchestrator-mcp`** — every public action-verb function (`enforce_*`, `handle_tool_call`, `persist_fact`, `propagate_trust`, `openclaw_*`) has real body.
- **`vox-publisher`** — scholarly submission (Crossref, Zenodo, RSS), finding ledger, ro-crate JSON-LD all real. The earlier audit's "arXiv submission placeholder" finding stands at `submission/arxiv.rs:26` but is **explicitly documented as operator-assist mode** with no automation promise, so it's not a contract-drift finding under this method.
- **`vox-populi`** — `dispatch_script`, `execute_on_worker`, `sync_external_job_after_remote_status`, Vast/RunPod clients all real.
- **Trait impls in plugins:** `MlBackend` (CUDA + Metal both real), `SkillRuntime` (container + WASM both real), `Publication`, `Browser`, `SpeechToText` (Whisper backend), `TeeVerifier` (Stub returns explicit `Err(Unsupported)` — intentional and documented for Phase 6).
- **Mesh phase plans, language rules phase plans** — confirmed as draft/in-progress; no completion claims to verify against; no plan-vs-code gaps.

---

## Summary table

| ID | File:line | Severity | Method | One-line |
|---|---|---|---|---|
| F1 | `vox-codegen-ts/src/route_manifest.rs:25-27` | HIGH | M2 | Public validator returns `Ok(())`; real impl sits `#[allow(dead_code)]` adjacent |
| F2 | `vox-orchestrator/src/services/reliability.rs:33-41`, `:75-83` | HIGH | M3-P1 | `record_endpoint_observation` Result discarded (2 sites) |
| F3 | `vox-orchestrator/src/services/reliability.rs:43-58` | HIGH | M3-P1 | `record_task_reliability_observation` Result discarded × 4 match arms |
| F4 | `vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:1002-1014` | HIGH | M3-P1 | `append_orchestration_lineage_event` Result discarded |
| F5 | `vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:97-105` | HIGH | M3-P1 | `begin_reconstruction_campaign` Result discarded |
| F6 | `vox-orchestrator/src/budget/persistence.rs:103` | HIGH | M3-P1 | `record_exec_time` Result discarded |
| F7 | `vox-plugin-cloud/src/sync.rs:46-48` | MEDIUM | M4 | `list_remote_json` returns `"[]"` instead of explicit not-implemented error |
| F8 | `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-b.md` | LOW | M1 | Plan doc internally contradicts its own Phase C preview |

---

## What the audit did NOT find (and why that matters)

- **No widespread `todo!()`/`unimplemented!()` rot.** The vox-code-audit "27 stubs" claim from the previous round was about a specific lint-rule engine crate; this round's M2/M3/M4 sweep across production-critical crates found that those crates do not have widespread surface stubs.
- **No silently-broken plugin trait impls.** Every plugin trait impl audited either does real work or has an explicit `SP7 scaffold` / deferred-marker comment (F7 is the only outlier).
- **No fake test suites.** Spot-checks of test directories found real assertions, not `assert!(true)` placeholders.
- **No spawn-and-forget patterns in critical paths.** The orchestrator's spawned tasks use `JoinHandle` and supervised actors.

The signal density is low — 7 real findings across 6 critical crates and 14 plugin/runtime crates. That's good. The remaining LLM-laziness in this codebase is concentrated in **silently swallowed `Result`s from DB writes inside the orchestrator** (F2–F6) plus one unwired validator (F1). The fix is mechanical and tractable; see `semantic-gap-implementation-plan-2026.md` for the staged plan.
