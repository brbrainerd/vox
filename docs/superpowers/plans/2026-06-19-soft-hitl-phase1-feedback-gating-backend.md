# Soft HITL Phase 1 — Feedback Backend Implementation Plan (rev 2)

> 🤖 **EXECUTION TARGET — READ FIRST.** Gemini Flash in Antigravity (~48% unaided
> completion, no mid-task checkpoint, hard quota cutoff). See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.

**Operating Rules (EVERY task):**
1. Atomic + green + committed. Never leave a broken tree.
2. Every Step-1 `rg` is a BLOCKING gate: run it, paste output; reality differs → STOP and report.
3. Two-strike circuit breaker: a step fails twice → stop and report.
4. Split on overrun: an implement step touching >1 file or adding >1 new fn → one atomic green commit per sub-bullet.
5. Rust verification ritual per touched crate: `cargo test -p <crate> <filter>` → `cargo clippy -p <crate> -- -D warnings` → `cargo fmt -p <crate>` (NEVER `cargo fmt --all` — Windows). Run `vox stub-check` before declaring done. No stubs.
6. Tags: `[PARALLEL-SAFE]` = disjoint files; `[SEQUENTIAL]` = shares a file with another task — never two subagents on one file.

**Goal:** A unified `FeedbackRequest` (Clarification | Doubt) with agent-declared gating keyed by `TaskId`, the `vox_ask_clarification` / `vox_resolve_feedback` / `vox_feedback_list` MCP tools, a single shared `FeedbackStore`, and a doubt projector sink — surfacing what `evaluate_with_state` already decides. **No hopper mutation; no `ItemState` change.**

**Architecture:** New `feedback` module in `vox-orchestrator`. One `FeedbackStore` owned by the `Orchestrator`, `Arc`-shared into the MCP `ServerState`. Clarifications register from the MCP tool; doubts from an async EventBus projector sink (because `doubt_task` is synchronous). Gating is recorded as the request's `gates: Vec<TaskId>`; the GUI derives "blocked" by overlay (Phase 2). Two new events ride the existing EventBus → `vox://agent-events` bridge.

**Tech Stack:** Rust (vox-orchestrator, vox-orchestrator-mcp), tokio, serde, cargo test.

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §3, §4, §6, §8

---

## Flash Execution Addendum (2026-06-19)

**Global gates:**
- **ID model:** gating is keyed by `TaskId(pub u64)` (`vox_orchestrator_types::agent_types::ids::TaskId`), NOT `HopperItemId` (a UUID string). Agents and `doubt_task` hold `TaskId`. Do not introduce any `HopperItemId` gate.
- **No hopper change.** Do not add `ItemState::Blocked`, do not touch `hopper/`, do not gate the dispatcher. Blocked is a Phase-2 GUI overlay.
- **Single store.** The `FeedbackStore` is created ONCE on the `Orchestrator` and `Arc`-cloned into `ServerState`. Never `FeedbackStore::new()` in two places.
- **CI gates that will fail you:** (a) `attention_ledger_parity` (`crates/vox-cli/src/commands/ci/attention_ledger_parity.rs`) fails any source file that calls `evaluate_*` without also calling `record_attention_event` — keep both in the same file. (b) New MCP tools MUST be added to `contracts/mcp/tool-registry.canonical.yaml` (SSOT, consumed by `vox-mcp-registry/build.rs`) and the `http_gateway` allowlist, or the dispatch-coverage tests fail. (c) MCP param schemas use the `derived_tool_schema!` macro + `#[derive(JsonSchema)]` + `#[schemars(deny_unknown_fields)]`, not raw JSON.

**Mandatory pre-flight (run, paste, confirm):**
```
rg -n "pub struct TaskId|pub struct AgentId" crates/vox-orchestrator-types/src/agent_types/ids.rs
rg -n "pub struct HopperItemId" crates/vox-orchestrator/src/events.rs
rg -n "pub enum InterruptionDecision" -A 14 crates/vox-orchestrator/src/attention/interruption_policy.rs
rg -n "pub enum AgentEventKind" -A 3 crates/vox-orchestrator/src/events.rs
rg -n "pending_approvals" crates/vox-orchestrator-mcp/src/server_state.rs
rg -n "InterruptionSignals|evaluate_with_state|record_attention_event" crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs
rg -n "\"vox_doubt_task\"" crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs
rg -n "pub fn doubt_task" -A 8 crates/vox-orchestrator/src/orchestrator/agent/doubt.rs
rg -n "OVERRULE_TASK|fn run_sink" crates/vox-orchestrator/src/ crates/vox-orchestrator-mcp/src/
```
Expected anchors: `TaskId(pub u64)`, `AgentId(pub u64)`; `HopperItemId(pub String)`; the 5 `InterruptionDecision` variants; `#[serde(tag="type", rename_all="snake_case")]` on `AgentEventKind`; THREE `pending_approvals` ctor sites (server_state.rs ~:280, :340, :627); the inline `InterruptionSignals` literal at `chat_socrates_meta.rs:388-404` + `evaluate_with_state` (`attention_policy.rs:220`) + `state.record_attention_event(..)` (`server_state.rs:430`); the `vox_doubt_task` dispatch arm + `derived_tool_schema!` schema entry; `doubt_task` is **sync** (no `async`); the activity-sink pattern (`activity/sink.rs::run_sink`).

**Task-split table:**

| Task | Touches | Tag |
|---|---|---|
| 1 — feedback types | `feedback/mod.rs`, `feedback/types.rs`, `lib.rs` | [SEQUENTIAL] |
| 2 — FeedbackStore | `feedback/store.rs`, `feedback/mod.rs` | [SEQUENTIAL] |
| 3 — surface_for | `feedback/surface_policy.rs`, `feedback/mod.rs` | [SEQUENTIAL] |
| 4 — store on Orchestrator + Arc into ServerState | `orchestrator/*`, `server_state.rs` | [SEQUENTIAL] |
| 5 — events + is_loggable | `events.rs`, `activity/mod.rs` | [PARALLEL-SAFE] |
| 6a–6d — vox_ask_clarification | `params.rs`, `input_schemas.rs`, `feedback_tools.rs`, `dispatch.rs`, `tool-registry.canonical.yaml`, `http_gateway/mod.rs` | [SEQUENTIAL] |
| 7a–7c — vox_resolve_feedback | same MCP files | [SEQUENTIAL] |
| 8 — vox_feedback_list | `feedback_tools.rs`, `dispatch.rs`, registry | [SEQUENTIAL] |
| 9 — doubt projector sink | `feedback/doubt_sink.rs`, `orchestrator/*` wiring | [SEQUENTIAL] |

---

### Task 1 — Feedback core types [SEQUENTIAL]

**Files:** Create `crates/vox-orchestrator/src/feedback/{mod.rs,types.rs}`; modify `crates/vox-orchestrator/src/lib.rs` (`pub mod feedback;`).

- [ ] **Step 1 (gate):** confirm `TaskId(pub u64)` and `AgentId(pub u64)` per pre-flight. `TaskId` is re-exported at `crate::types::TaskId` (confirm with `rg -n "TaskId" crates/vox-orchestrator/src/types/mod.rs`); use the `crate::types::{TaskId, AgentId}` path.

- [ ] **Step 2: Write the failing test** (in `types.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips_with_snake_case_tags() {
        let req = FeedbackRequest {
            id: FeedbackId("F-000001".into()), kind: FeedbackKind::Clarification,
            prompt: "schema?".into(), options: vec!["a".into()], gates: vec![TaskId(7)],
            doubted_task_id: None, info_gain_bits: 0.8, scaled_cost_ms: 1000,
            surface: Surface::NeedsYou, session_id: None, agent_id: None,
            created_at_ms: 1, resolution: None,
        };
        let j = serde_json::to_string(&req).unwrap();
        assert!(j.contains("\"kind\":\"clarification\""));
        assert!(j.contains("\"surface\":\"needs_you\""));
        let back: FeedbackRequest = serde_json::from_str(&j).unwrap();
        assert_eq!(back.gates, vec![TaskId(7)]);
    }
    #[test]
    fn action_is_internally_tagged() {
        let a = FeedbackAction::Answer { option: Some(1), text: None };
        assert!(serde_json::to_string(&a).unwrap().contains("\"action\":\"answer\""));
        assert!(serde_json::to_string(&FeedbackAction::Overrule).unwrap().contains("overrule"));
    }
}
```

- [ ] **Step 3:** `cargo test -p vox-orchestrator feedback::types` → FAIL (module missing).

- [ ] **Step 4: Implement** `types.rs` exactly as spec §3 (FeedbackId, FeedbackKind, Surface, FeedbackAction tagged `action`, FeedbackResolution, FeedbackRequest with `gates: Vec<TaskId>`, `doubted_task_id: Option<TaskId>`, `scaled_cost_ms: u64`). `mod.rs`: `pub mod types; pub use types::*;`. Add `pub mod feedback;` to `lib.rs`.

- [ ] **Step 5:** `cargo test -p vox-orchestrator feedback::types` → PASS. clippy + fmt.

- [ ] **Step 6: Commit** `git commit -m "feat(orchestrator): FeedbackRequest core types (TaskId-keyed gating)"`

---

### Task 2 — FeedbackStore [SEQUENTIAL]

**Files:** Create `crates/vox-orchestrator/src/feedback/store.rs`; modify `feedback/mod.rs`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackKind, Surface, FeedbackAction, FeedbackResolution};
    use crate::types::TaskId;

    fn reg(s: &FeedbackStore, surface: Surface, gain: f64) -> impl std::future::Future<Output = crate::feedback::FeedbackId> + '_ {
        s.register(FeedbackKind::Clarification, "q?".into(), vec![], vec![TaskId(1)], None, gain, 500, surface, None, None, 1)
    }
    #[tokio::test]
    async fn needs_you_vs_withheld_partition() {
        let s = FeedbackStore::new();
        reg(&s, Surface::NeedsYou, 0.8).await;
        reg(&s, Surface::Withheld, 0.05).await;
        assert_eq!(s.open_needs_you().await.len(), 1);
        assert_eq!(s.withheld().await.len(), 1);
    }
    #[tokio::test]
    async fn resolve_is_idempotent_and_removes_from_open() {
        let s = FeedbackStore::new();
        let id = reg(&s, Surface::NeedsYou, 0.8).await;
        let res = FeedbackResolution { action: FeedbackAction::Skip, decided_at_ms: 2, decided_by: "gui".into() };
        assert!(s.resolve(&id, res.clone()).await.is_some());
        assert!(s.resolve(&id, res).await.is_none()); // already resolved => None
        assert_eq!(s.open_needs_you().await.len(), 0);
    }
    #[tokio::test]
    async fn resolve_unknown_id_returns_none() {
        let s = FeedbackStore::new();
        let res = FeedbackResolution { action: FeedbackAction::Skip, decided_at_ms: 2, decided_by: "x".into() };
        assert!(s.resolve(&crate::feedback::FeedbackId("nope".into()), res).await.is_none());
    }
}
```

- [ ] **Step 2:** run → FAIL.

- [ ] **Step 3: Implement** `store.rs`. `FeedbackStore { inner: Arc<RwLock<Inner>> }`, `#[derive(Clone, Default)]`. `Inner { seq: u64, items: Vec<FeedbackRequest> }`. `register(..)` (11 args matching spec field order, allow `clippy::too_many_arguments`) assigns `F-{:06}`. `open_needs_you()`/`withheld()` filter `resolution.is_none() && surface == ..`. `get(&id)`. `resolve(&id, res)`: return `None` if not found OR `resolution.is_some()` (idempotency); else set and return clone. Add `pub async fn promote_withheld<F: Fn(&FeedbackRequest)->Surface>(&self, f: F)` that re-runs `f` over withheld items and flips any now-`NeedsYou` (used on-resolve; cheap, no scheduler). Add `pub mod store; pub use store::*;` to `mod.rs`.

> Retention note: `items` is unbounded for v1 (sessions are bounded in practice). Resolved items are filtered from `open_*`. A ring/age prune is a documented follow-up, not v1.

- [ ] **Step 4:** run → PASS. clippy + fmt.
- [ ] **Step 5: Commit** `git commit -m "feat(orchestrator): FeedbackStore (idempotent resolve, withheld promotion)"`

---

### Task 3 — surface_for(decision) [SEQUENTIAL]

**Files:** Create `crates/vox-orchestrator/src/feedback/surface_policy.rs`; modify `mod.rs`. (This task is unchanged from rev 1 — the audit confirmed `InterruptionDecision` matches exactly.)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::interruption_policy::InterruptionDecision as D;
    use crate::feedback::Surface;
    #[test] fn now_and_require_human_are_needs_you() {
        assert_eq!(surface_for(&D::InterruptNow { reason: "x".into(), scaled_cost_ms: 1 }), Surface::NeedsYou);
        assert_eq!(surface_for(&D::RequireHumanBeforeContinue { reason: "x".into(), scaled_cost_ms: 1 }), Surface::NeedsYou);
    }
    #[test] fn defer_batch_proceed_are_withheld() {
        assert_eq!(surface_for(&D::DeferUntilCheckpoint { reason: "x".into() }), Surface::Withheld);
        assert_eq!(surface_for(&D::BatchWithExistingPrompt { reason: "x".into() }), Surface::Withheld);
        assert_eq!(surface_for(&D::ProceedAutonomously { reason: "x".into() }), Surface::Withheld);
    }
}
```

- [ ] **Step 2:** run → FAIL. **Step 3:** implement `pub fn surface_for(d: &InterruptionDecision) -> Surface` mapping InterruptNow/RequireHuman→NeedsYou, the other three→Withheld. Also export `pub fn scaled_cost_of(d: &InterruptionDecision) -> u64` returning the `scaled_cost_ms` for the two carrying it, else 0. **Step 4:** run → PASS. **Step 5:** commit `feat(orchestrator): surface_for + scaled_cost_of interruption mapping`.

---

### Task 4 — FeedbackStore on Orchestrator, Arc into ServerState [SEQUENTIAL]

**Files:** Modify `crates/vox-orchestrator/src/orchestrator/...` (the `Orchestrator` struct + constructor) and `crates/vox-orchestrator-mcp/src/server_state.rs`.

- [ ] **Step 1 (gate):** `rg -n "pub struct Orchestrator|impl Orchestrator|pub fn new" crates/vox-orchestrator/src/orchestrator/*.rs` and `rg -n "pending_approvals" crates/vox-orchestrator-mcp/src/server_state.rs` — locate the `Orchestrator` struct/ctor and ALL THREE `ServerState` ctor sites.

- [ ] **Step 2:** Add `feedback: crate::feedback::FeedbackStore` to the `Orchestrator` struct; initialize `FeedbackStore::new()` in its constructor; add `pub fn feedback(&self) -> &crate::feedback::FeedbackStore { &self.feedback }`. (FeedbackStore is `Clone` over an `Arc`, so `ServerState` holds a clone.)

- [ ] **Step 3:** In `ServerState`, do NOT add a separately-constructed store. Expose the orchestrator's: add `pub fn feedback(&self) -> vox_orchestrator::feedback::FeedbackStore { self.orchestrator.feedback().clone() }` (or store an `Arc` clone field initialized from `orchestrator.feedback().clone()` at all three ctor sites). Pick one; the invariant is **one instance**.

- [ ] **Step 4: Test** — add a unit test asserting `state.feedback()` and `orchestrator.feedback()` observe the same registration (register via one, read via the other). `cargo test -p vox-orchestrator-mcp feedback_shared` → PASS after impl. clippy + fmt both crates.

- [ ] **Step 5: Commit** `git commit -m "feat: single shared FeedbackStore (Orchestrator-owned, ServerState Arc clone)"`

---

### Task 5 — Feedback events + is_loggable [PARALLEL-SAFE]

**Files:** Modify `crates/vox-orchestrator/src/events.rs`, `crates/vox-orchestrator/src/activity/mod.rs`.

- [ ] **Step 1 (gate):** `rg -n "pub enum AgentEventKind" -A 3 crates/vox-orchestrator/src/events.rs` — confirm `#[serde(tag="type", rename_all="snake_case")]`. This means variant `FeedbackRequested` serializes with tag `"feedback_requested"`.

- [ ] **Step 2: Write the failing test** (note the snake_case assertion):

```rust
#[test]
fn feedback_events_serialize_snake_case_tag() {
    let e = AgentEventKind::FeedbackRequested {
        feedback_id: "F-000001".into(), kind: "clarification".into(),
        gates: vec![7], surface: "needs_you".into(),
    };
    let j = serde_json::to_string(&e).unwrap();
    assert!(j.contains("\"type\":\"feedback_requested\"")); // NOT "FeedbackRequested"
    assert!(j.contains("needs_you"));
}
```

- [ ] **Step 3: Add two variants** (`gates: Vec<u64>` on the wire = TaskId values):

```rust
    FeedbackRequested { feedback_id: String, kind: String, gates: Vec<u64>, surface: String },
    FeedbackResolved { feedback_id: String },
```

- [ ] **Step 4:** Add both to the `is_loggable` allowlist (`activity/mod.rs`). No exhaustive `match AgentEventKind` exists without a catch-all (audit-verified: `activity/project.rs:211` ends `other => ...`), so the crate compiles with no further match edits. Optionally add readable arms in `project.rs` so they don't log as kind `"Other"`.

- [ ] **Step 5:** `cargo test -p vox-orchestrator feedback_events_serialize && cargo build -p vox-orchestrator` → PASS/clean. clippy + fmt.

- [ ] **Step 6: Commit** `git commit -m "feat(events): FeedbackRequested/Resolved variants + is_loggable"`

---

### Task 6 — vox_ask_clarification (split 6a–6d) [SEQUENTIAL]

**Files:** `crates/vox-orchestrator-mcp/src/{params.rs,input_schemas.rs,feedback_tools.rs,dispatch.rs}`, `contracts/mcp/tool-registry.canonical.yaml`, `crates/vox-orchestrator-mcp/src/http_gateway/mod.rs`.

**6a — params + schema [SEQUENTIAL]**
- [ ] Test (in `feedback_tools.rs` or `params.rs`):
```rust
#[test] fn ask_params_deserialize() {
    let p: AskClarificationParams = serde_json::from_str(r#"{"prompt":"q?","options":["a"],"gates":[1,2]}"#).unwrap();
    assert_eq!(p.gates, vec![1, 2]); // TaskId values as u64
    let d: AskClarificationParams = serde_json::from_str(r#"{"prompt":"q?"}"#).unwrap();
    assert!(d.gates.is_empty() && d.options.is_empty());
}
```
- [ ] Implement in `params.rs` (derive convention):
```rust
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AskClarificationParams {
    pub prompt: String,
    #[serde(default)] pub options: Vec<String>,
    /// TaskId(u64) values this question gates; empty = non-gating.
    #[serde(default)] pub gates: Vec<u64>,
    #[serde(default)] pub session_id: Option<String>,
}
```
- [ ] Add `"vox_ask_clarification" => derived_tool_schema!(crate::params::AskClarificationParams),` to `input_schemas.rs` (mirror the `vox_doubt_task` entry verbatim from the Step-1 gate). Test → PASS. Commit `feat(mcp): AskClarificationParams + derived schema`.

**6b — handler [SEQUENTIAL]**
- [ ] Implement `pub async fn ask_clarification(state: &ServerState, params: AskClarificationParams) -> String` in `feedback_tools.rs`:
  1. Build `InterruptionSignals` inline (copy the literal from `chat_socrates_meta.rs:388-404` per gate; channel `InterruptionChannel::ChatClarification`; 14 fields).
  2. `let att = <live snapshot as chat_socrates_meta obtains it>;`
  3. `let decision = crate::attention_policy::evaluate_with_state(state, &signals, &att);`
  4. `let surface = vox_orchestrator::feedback::surface_for(&decision); let cost = vox_orchestrator::feedback::scaled_cost_of(&decision);`
  5. **Same-file record (parity gate):** `state.record_attention_event(vox_orchestrator::AttentionEvent { .. });` (copy the 12-field construction from `chat_socrates_meta.rs:415-428`; `event_type` an appropriate `AttentionEventType`; `cost_ms: cost`).
  6. `let gates = params.gates.iter().copied().map(vox_orchestrator::types::TaskId).collect::<Vec<_>>();`
  7. `let id = state.feedback().register(FeedbackKind::Clarification, params.prompt, params.options, gates.clone(), None, signals.expected_information_gain_bits, cost, surface, params.session_id, None, now_ms).await;`
  8. Emit `FeedbackRequested { feedback_id: id.0.clone(), kind: "clarification".into(), gates: gates.iter().map(|t| t.0).collect(), surface: surface_str }` via the orchestrator event bus.
  9. Return `crate::params::ToolResult::ok(serde_json::json!({"feedback_id": id.0, "surface": surface_str})).to_json()`.
- [ ] Test: a `#[tokio::test]` calling `ask_clarification` against a test `ServerState` and asserting a request lands in `state.feedback().open_needs_you()` (or `withheld()`). PASS. Commit `feat(mcp): ask_clarification handler (evaluate+record+register+emit)`.

**6c — dispatch arm [SEQUENTIAL]**
- [ ] Add `"vox_ask_clarification" => Ok(feedback_tools::ask_clarification(state, serde_json::from_value(args)?).await),` to the `match name` in `dispatch.rs` (mirror the `vox_doubt_task` arm). `cargo test -p vox-orchestrator-mcp` → green. Commit `feat(mcp): route vox_ask_clarification`.

**6d — registry SSOT [SEQUENTIAL]**
- [ ] Add `vox_ask_clarification` to `contracts/mcp/tool-registry.canonical.yaml` (mirror `vox_doubt_task`) and to the `http_gateway/mod.rs` tool list. Rebuild `vox-mcp-registry`. Run the dispatch coverage tests: `cargo test -p vox-orchestrator-mcp registry` → green. Commit `feat(mcp): register vox_ask_clarification in canonical tool registry`.

---

### Task 7 — vox_resolve_feedback (split 7a–7c) [SEQUENTIAL]

**7a — params + schema:** `ResolveFeedbackParams { feedback_id: String, action: vox_orchestrator::feedback::FeedbackAction }` (derive `Deserialize, JsonSchema`, `deny_unknown_fields`). Test deserialization of `{"feedback_id":"F-1","action":{"action":"answer","option":0}}` and `{"action":{"action":"overrule"}}`. Add derived schema entry + dispatch arm + registry (mirror Task 6d). Commit per sub-step.

**7b — resolve handler:** `pub async fn resolve_feedback(state: &ServerState, p: ResolveFeedbackParams) -> String`:
- `let fid = FeedbackId(p.feedback_id);`
- `let Some(req) = state.feedback().get(&fid).await else { return ToolResult::err_with_remediation("feedback not found").to_json() };`
- `let Some(resolved) = state.feedback().resolve(&fid, FeedbackResolution { action: p.action.clone(), decided_at_ms: now_ms, decided_by: "gui".into() }).await else { return ToolResult::err_with_remediation("already resolved").to_json() };` (never unwrap — audit fix)
- Debit attention in THIS file via `state.record_attention_event(AttentionEvent { cost_ms: req.scaled_cost_ms, .. })`.
- **Doubt Overrule dispatch:** `if req.kind == FeedbackKind::Doubt { if let (FeedbackAction::Overrule, Some(tid)) = (&p.action, req.doubted_task_id) { <call the existing OVERRULE_TASK path for tid — grep `OVERRULE_TASK` / `overrule_task`; for an in-process ServerState this is state.orchestrator.overrule_task(tid, reason)> } }` (LetVerify = no-op; the Verifier pass continues).
- Emit `FeedbackResolved { feedback_id: fid.0.clone() }`.
- `state.feedback().promote_withheld(|r| surface_for_recompute(r)).await;` (cheap re-evaluation; if recompute needs signals, v1 may skip and just re-list — keep it simple).
- Return `ToolResult::ok(json!({"resolved": true})).to_json()`.
- [ ] Test: resolve a Doubt with Overrule and assert the overrule path is invoked (use a spy/test orchestrator if available, else assert `TaskStatus` transition). PASS. Commit.

**7c — registry:** add `vox_resolve_feedback` to the canonical YAML + http_gateway; coverage tests green. Commit.

---

### Task 8 — vox_feedback_list [SEQUENTIAL]

- [ ] `pub async fn feedback_list(state: &ServerState, _p: serde_json::Value) -> String` returning `ToolResult::ok(json!({ "needs_you": <Vec<FeedbackRequest>>, "withheld": <..> })).to_json()` from `state.feedback().open_needs_you()` + `withheld()`. (`FeedbackRequest` is `Serialize`.) Add dispatch arm + canonical YAML entry + http_gateway. Test: register two (one needs_you, one withheld), call, assert the JSON partitions. PASS. Commit `feat(mcp): vox_feedback_list tool`.

---

### Task 9 — Doubt projector sink [SEQUENTIAL]

**Files:** Create `crates/vox-orchestrator/src/feedback/doubt_sink.rs`; wire it where the activity sink is spawned.

- [ ] **Step 1 (gate):** `rg -n "pub fn doubt_task" -A 10 crates/vox-orchestrator/src/orchestrator/agent/doubt.rs` (confirm it is **sync** and emits `AgentEventKind::TaskDoubted { task_id, agent_id, reason }`) and `rg -n "run_sink|spawn.*sink" crates/vox-orchestrator/src/activity/` (the subscribe-and-drain pattern to mirror).

- [ ] **Step 2: Write the failing test** (pure helper first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn prompt_includes_reason_or_default() {
        assert!(doubt_feedback_prompt(Some("conflicting spec")).contains("conflicting spec"));
        assert!(!doubt_feedback_prompt(None).is_empty());
    }
}
```

- [ ] **Step 3: Implement** `doubt_feedback_prompt(reason: Option<&str>) -> String` (pure) and an async `pub async fn run_doubt_sink(mut rx: broadcast::Receiver<AgentEvent>, store: FeedbackStore)` that, on each `AgentEventKind::TaskDoubted { task_id, .. }`, calls `store.register(FeedbackKind::Doubt, doubt_feedback_prompt(reason), vec![], vec![], Some(task_id), 0.0, 0, Surface::NeedsYou, None, Some(agent_id), now_ms)` — `gates: vec![]` (doubts don't park tasks), `doubted_task_id: Some(task_id)` (Overrule target). Spawn `run_doubt_sink` next to the activity sink, subscribing the same EventBus, passing the Orchestrator's `feedback()` clone.

> Why a sink: `doubt_task` is synchronous and must not `.await` a store write on the hot path. The sink is the same pattern as `activity/sink.rs` and keeps the single-store invariant (it gets the Orchestrator's clone).

- [ ] **Step 4:** `cargo test -p vox-orchestrator feedback::doubt_sink` → PASS. Add a `#[tokio::test]` that emits a `TaskDoubted` on a test bus and asserts a Doubt request appears in the store. clippy + fmt.

- [ ] **Step 5: Commit** `git commit -m "feat(orchestrator): doubt projector sink → unified feedback store"`

---

### Self-review notes (vs spec rev 2)
- §3 model: Tasks 1, 2 — `TaskId` gates, `FeedbackAction`, `doubted_task_id`, `scaled_cost_ms`. ✓
- §4 flow: Task 6 (raise+evaluate+record+register), Task 7 (resolve+debit+Overrule dispatch+withheld promote), Task 9 (doubt sink). ✓
- §6 events/registry: Task 5 + the 6d/7c/8 registry steps + parity-gate same-file record. ✓
- §8 corrections: no hopper mutation (no `ItemState::Blocked`, no gating helpers — dropped from rev 1); single store (Task 4); sync-doubt-via-sink (Task 9); `Vec<u64>=TaskId` not HopperItemId (Task 6a); `evaluate_with_state` not raw; `record_attention_event` not `BudgetManager::record_attention`; `derived_tool_schema!`. ✓
- Approvals untouched; no policy change. ✓
- Type consistency: `FeedbackId`, `FeedbackKind`, `Surface`, `FeedbackAction`, `FeedbackResolution`, `surface_for`, `scaled_cost_of`, `state.feedback()` consistent across tasks.
