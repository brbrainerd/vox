# Soft HITL Phase 1 — Feedback Model + Gating Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a unified `FeedbackRequest` (Clarification | Doubt), agent-declared task gating via `ItemState::Blocked { gated_by }`, the `vox_ask_clarification` MCP tool, and block/unblock + re-admit logic — surfacing what `evaluate_interruption()` already decides.

**Architecture:** New `feedback` module in `vox-orchestrator`. Clarifications enter via a new MCP tool; doubts project into the same type. The existing `evaluate_interruption()` verdict sets `surface = NeedsYou | Withheld`. Gating edges live on the hopper item. Resolution clears edges, re-admits items, debits attention via the existing `BudgetManager::record_attention()`. Four new EventBus variants carry it to the GUI.

**Tech Stack:** Rust (vox-orchestrator, vox-orchestrator-mcp), tokio, serde, cargo test.

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §3, §4, §6

---

### Task 1: `FeedbackRequest` core types

**Files:**
- Create: `crates/vox-orchestrator/src/feedback/mod.rs`
- Create: `crates/vox-orchestrator/src/feedback/types.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs` (add `pub mod feedback;`)
- Test: inline `#[cfg(test)]` in `types.rs`

- [ ] **Step 1: Write the failing test**

In `crates/vox-orchestrator/src/feedback/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_request_round_trips_json_with_snake_case_kind() {
        let req = FeedbackRequest {
            id: FeedbackId("F-000001".into()),
            kind: FeedbackKind::Clarification,
            prompt: "DB schema?".into(),
            options: vec!["A".into(), "B".into()],
            gates: vec![],
            info_gain_bits: 0.8,
            surface: Surface::NeedsYou,
            session_id: None,
            agent_id: None,
            created_at_ms: 10,
            resolution: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"kind\":\"clarification\""));
        assert!(json.contains("\"surface\":\"needs_you\""));
        let back: FeedbackRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, req.id);
    }

    #[test]
    fn resolution_records_choice_and_text() {
        let r = FeedbackResolution {
            chosen_option: Some(1),
            free_text: None,
            decided_at_ms: 99,
            decided_by: "gui".into(),
        };
        assert_eq!(r.chosen_option, Some(1));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback::types -- --nocapture`
Expected: FAIL — module `feedback` does not exist.

- [ ] **Step 3: Implement the types**

`crates/vox-orchestrator/src/feedback/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::hopper::HopperItemId;
use crate::types::AgentId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeedbackId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind { Clarification, Doubt }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface { NeedsYou, Withheld }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackResolution {
    pub chosen_option: Option<usize>,
    pub free_text: Option<String>,
    pub decided_at_ms: u64,
    pub decided_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub id: FeedbackId,
    pub kind: FeedbackKind,
    pub prompt: String,
    pub options: Vec<String>,
    pub gates: Vec<HopperItemId>,
    pub info_gain_bits: f64,
    pub surface: Surface,
    pub session_id: Option<String>,
    pub agent_id: Option<AgentId>,
    pub created_at_ms: u64,
    pub resolution: Option<FeedbackResolution>,
}
```

`crates/vox-orchestrator/src/feedback/mod.rs`:

```rust
//! Unified soft-HITL feedback: clarifications + doubts projected into one type,
//! with agent-declared task gating. See
//! docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md
pub mod types;
pub use types::*;
```

Add `pub mod feedback;` to `crates/vox-orchestrator/src/lib.rs` (next to the other `pub mod` lines). If `AgentId` is not at `crate::types::AgentId`, fix the import to its real path (grep: `pub struct AgentId` / `pub type AgentId`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator feedback::types`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/ crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): FeedbackRequest core types"
```

---

### Task 2: Extend `ItemState` with `Blocked`

**Files:**
- Modify: `crates/vox-orchestrator/src/hopper/types.rs:77-100`
- Test: inline `#[cfg(test)]` in same file

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator/src/hopper/types.rs`:

```rust
#[cfg(test)]
mod blocked_state_tests {
    use super::*;
    use crate::feedback::FeedbackId;

    #[test]
    fn blocked_state_has_kind_and_lists_gates() {
        let st = ItemState::Blocked { gated_by: vec![FeedbackId("F-1".into())] };
        assert_eq!(st.kind(), "blocked");
    }

    #[test]
    fn blocked_serializes_snake_case() {
        let st = ItemState::Blocked { gated_by: vec![] };
        let j = serde_json::to_string(&st).unwrap();
        assert!(j.contains("blocked"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator blocked_state_tests`
Expected: FAIL — no `Blocked` variant.

- [ ] **Step 3: Add the variant and `kind()` arm**

In `ItemState` (after `Assigned`):

```rust
    /// Held pending one or more unresolved FeedbackRequests (agent-declared gating).
    Blocked { gated_by: Vec<crate::feedback::FeedbackId> },
```

In `ItemState::kind()` add:

```rust
            Self::Blocked { .. } => "blocked",
```

- [ ] **Step 4: Run test to verify it passes (and the crate still builds)**

Run: `cargo test -p vox-orchestrator blocked_state_tests && cargo build -p vox-orchestrator`
Expected: PASS; build clean. If non-exhaustive `match ItemState` errors appear elsewhere, add a `Blocked` arm that treats it as not-dispatchable (mirror the `Inbox`-but-not-ready handling, or skip in dispatch). Fix each at its site.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/hopper/types.rs
git commit -m "feat(hopper): ItemState::Blocked { gated_by } gating state"
```

---

### Task 3: FeedbackStore — register, surface verdict, resolve

**Files:**
- Create: `crates/vox-orchestrator/src/feedback/store.rs`
- Modify: `crates/vox-orchestrator/src/feedback/mod.rs` (add `pub mod store;`)
- Test: inline `#[cfg(test)]` in `store.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackKind, Surface};

    #[tokio::test]
    async fn register_assigns_id_and_lists_open() {
        let store = FeedbackStore::new();
        let id = store.register(FeedbackKind::Clarification, "q?".into(), vec![], vec![], 0.8, Surface::NeedsYou, None, None, 1).await;
        let open = store.open_needs_you().await;
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, id);
    }

    #[tokio::test]
    async fn withheld_not_in_needs_you_but_in_withheld() {
        let store = FeedbackStore::new();
        store.register(FeedbackKind::Clarification, "low?".into(), vec![], vec![], 0.05, Surface::Withheld, None, None, 1).await;
        assert_eq!(store.open_needs_you().await.len(), 0);
        assert_eq!(store.withheld().await.len(), 1);
    }

    #[tokio::test]
    async fn resolve_records_resolution_and_removes_from_open() {
        let store = FeedbackStore::new();
        let id = store.register(FeedbackKind::Clarification, "q?".into(), vec![], vec![], 0.8, Surface::NeedsYou, None, None, 1).await;
        let resolved = store.resolve(&id, FeedbackResolution { chosen_option: Some(0), free_text: None, decided_at_ms: 2, decided_by: "gui".into() }).await.unwrap();
        assert!(resolved.resolution.is_some());
        assert_eq!(store.open_needs_you().await.len(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback::store`
Expected: FAIL — `FeedbackStore` not defined.

- [ ] **Step 3: Implement the store**

`crates/vox-orchestrator/src/feedback/store.rs`:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::hopper::HopperItemId;
use crate::types::AgentId;
use super::{FeedbackId, FeedbackKind, FeedbackRequest, FeedbackResolution, Surface};

/// In-memory feedback registry (Option-A pattern, mirrors InMemoryHopper).
#[derive(Clone, Default)]
pub struct FeedbackStore {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default)]
struct Inner {
    seq: u64,
    items: Vec<FeedbackRequest>,
}

impl FeedbackStore {
    pub fn new() -> Self { Self::default() }

    #[allow(clippy::too_many_arguments)]
    pub async fn register(
        &self,
        kind: FeedbackKind,
        prompt: String,
        options: Vec<String>,
        gates: Vec<HopperItemId>,
        info_gain_bits: f64,
        surface: Surface,
        session_id: Option<String>,
        agent_id: Option<AgentId>,
        created_at_ms: u64,
    ) -> FeedbackId {
        let mut g = self.inner.write().await;
        g.seq += 1;
        let id = FeedbackId(format!("F-{:06}", g.seq));
        g.items.push(FeedbackRequest {
            id: id.clone(), kind, prompt, options, gates, info_gain_bits,
            surface, session_id, agent_id, created_at_ms, resolution: None,
        });
        id
    }

    pub async fn open_needs_you(&self) -> Vec<FeedbackRequest> {
        self.inner.read().await.items.iter()
            .filter(|r| r.resolution.is_none() && r.surface == Surface::NeedsYou)
            .cloned().collect()
    }

    pub async fn withheld(&self) -> Vec<FeedbackRequest> {
        self.inner.read().await.items.iter()
            .filter(|r| r.resolution.is_none() && r.surface == Surface::Withheld)
            .cloned().collect()
    }

    pub async fn get(&self, id: &FeedbackId) -> Option<FeedbackRequest> {
        self.inner.read().await.items.iter().find(|r| &r.id == id).cloned()
    }

    pub async fn resolve(&self, id: &FeedbackId, res: FeedbackResolution) -> Option<FeedbackRequest> {
        let mut g = self.inner.write().await;
        let item = g.items.iter_mut().find(|r| &r.id == id)?;
        item.resolution = Some(res);
        Some(item.clone())
    }
}
```

Add `pub mod store;` and `pub use store::*;` to `feedback/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator feedback::store`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/
git commit -m "feat(orchestrator): FeedbackStore register/surface/resolve"
```

---

### Task 4: Map an interruption verdict to a Surface

**Files:**
- Create: `crates/vox-orchestrator/src/feedback/surface_policy.rs`
- Modify: `crates/vox-orchestrator/src/feedback/mod.rs`
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::interruption_policy::InterruptionDecision;
    use crate::feedback::Surface;

    #[test]
    fn interrupt_now_surfaces_needs_you() {
        let d = InterruptionDecision::InterruptNow { reason: "x".into(), scaled_cost_ms: 1 };
        assert_eq!(surface_for(&d), Surface::NeedsYou);
    }

    #[test]
    fn require_human_surfaces_needs_you() {
        let d = InterruptionDecision::RequireHumanBeforeContinue { reason: "x".into(), scaled_cost_ms: 1 };
        assert_eq!(surface_for(&d), Surface::NeedsYou);
    }

    #[test]
    fn defer_and_batch_and_proceed_are_withheld() {
        assert_eq!(surface_for(&InterruptionDecision::DeferUntilCheckpoint { reason: "x".into() }), Surface::Withheld);
        assert_eq!(surface_for(&InterruptionDecision::BatchWithExistingPrompt { reason: "x".into() }), Surface::Withheld);
        assert_eq!(surface_for(&InterruptionDecision::ProceedAutonomously { reason: "x".into() }), Surface::Withheld);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback::surface_policy`
Expected: FAIL — `surface_for` undefined. (If the `InterruptionDecision` variant field names differ from the spec, grep `enum InterruptionDecision` in `attention/interruption_policy.rs` and match them exactly in the test.)

- [ ] **Step 3: Implement**

`crates/vox-orchestrator/src/feedback/surface_policy.rs`:

```rust
use crate::attention::interruption_policy::InterruptionDecision;
use super::Surface;

/// Surfacing is owned by the interruption policy: anything that says "ask now"
/// goes to Needs You; defer/batch/proceed stays Withheld (opt-in). No new policy.
pub fn surface_for(decision: &InterruptionDecision) -> Surface {
    match decision {
        InterruptionDecision::InterruptNow { .. }
        | InterruptionDecision::RequireHumanBeforeContinue { .. } => Surface::NeedsYou,
        InterruptionDecision::DeferUntilCheckpoint { .. }
        | InterruptionDecision::BatchWithExistingPrompt { .. }
        | InterruptionDecision::ProceedAutonomously { .. } => Surface::Withheld,
    }
}
```

Add `pub mod surface_policy;` and `pub use surface_policy::*;` to `feedback/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator feedback::surface_policy`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/
git commit -m "feat(orchestrator): map interruption verdict to feedback Surface"
```

---

### Task 5: New EventBus variants for feedback + gating

**Files:**
- Modify: `crates/vox-orchestrator/src/events.rs` (the `AgentEventKind` enum)
- Modify: `crates/vox-orchestrator/src/activity/mod.rs` (`is_loggable` allowlist)
- Test: inline in `events.rs`

- [ ] **Step 1: Write the failing test**

Add to `events.rs` tests:

```rust
#[test]
fn feedback_events_serialize_with_tagged_type() {
    let e = AgentEventKind::FeedbackRequested {
        feedback_id: "F-000001".into(),
        kind: "clarification".into(),
        gates: vec!["H-1".into()],
        surface: "needs_you".into(),
    };
    let j = serde_json::to_string(&e).unwrap();
    assert!(j.contains("FeedbackRequested"));
    assert!(j.contains("needs_you"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback_events_serialize`
Expected: FAIL — variant not found.

- [ ] **Step 3: Add the four variants**

In `AgentEventKind` (match the existing serde tagging style used by sibling variants — they derive a `type` tag; mirror it exactly):

```rust
    /// A FeedbackRequest was created and surfaced (or withheld).
    FeedbackRequested { feedback_id: String, kind: String, gates: Vec<String>, surface: String },
    /// A FeedbackRequest was resolved by the user (button or chat).
    FeedbackResolved { feedback_id: String },
    /// A hopper item was blocked pending feedback.
    HopperItemBlocked { item_id: String, gated_by: Vec<String> },
    /// A hopper item's gates all cleared; it returns to the inbox.
    HopperItemUnblocked { item_id: String },
```

- [ ] **Step 4: Add to the activity-log allowlist**

In `crates/vox-orchestrator/src/activity/mod.rs` `is_loggable()`, add `FeedbackRequested`, `FeedbackResolved`, `HopperItemBlocked`, `HopperItemUnblocked` to the `true` arm (these are high-signal, low-frequency).

- [ ] **Step 5: Run test + build**

Run: `cargo test -p vox-orchestrator feedback_events_serialize && cargo build -p vox-orchestrator`
Expected: PASS; build clean. Fix any non-exhaustive `match AgentEventKind` sites (e.g. `activity/project.rs`) by adding human-readable summary arms.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/events.rs crates/vox-orchestrator/src/activity/
git commit -m "feat(events): feedback + hopper block/unblock event variants"
```

---

### Task 6: Block / unblock + re-admit logic

**Files:**
- Create: `crates/vox-orchestrator/src/feedback/gating.rs`
- Modify: `crates/vox-orchestrator/src/feedback/mod.rs`
- Test: inline (uses `InMemoryHopper`)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::{InMemoryHopper, HopperIntake, PriorityHint, IntakeSource, ItemState};
    use crate::feedback::FeedbackId;

    #[tokio::test]
    async fn block_then_unblock_returns_item_to_inbox() {
        let hopper = InMemoryHopper::new();
        let item = hopper.submit("t".into(), vec![], PriorityHint::Normal, IntakeSource::Agent, None).await;
        let fid = FeedbackId("F-1".into());

        block_items(&hopper, &[item.item_id.clone()], &fid).await;
        let after = hopper.get(&item.item_id).await.unwrap();
        assert_eq!(after.state.kind(), "blocked");

        unblock_for_feedback(&hopper, &fid).await;
        let cleared = hopper.get(&item.item_id).await.unwrap();
        assert_eq!(cleared.state.kind(), "inbox");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback::gating`
Expected: FAIL — functions undefined. (If `InMemoryHopper` lacks `get`/state mutation, grep `impl InMemoryHopper` and add a `set_state(&self, id, ItemState)` test helper + `get` if missing; both are thin RwLock<Vec> ops.)

- [ ] **Step 3: Implement**

`crates/vox-orchestrator/src/feedback/gating.rs`:

```rust
use crate::hopper::{InMemoryHopper, HopperItemId, ItemState};
use super::FeedbackId;

/// Flip the named items to Blocked, accumulating the gating feedback id.
pub async fn block_items(hopper: &InMemoryHopper, items: &[HopperItemId], fid: &FeedbackId) {
    for id in items {
        if let Some(mut st) = hopper.get(id).await.map(|i| i.state) {
            let mut gated = match st {
                ItemState::Blocked { gated_by } => gated_by,
                _ => Vec::new(),
            };
            if !gated.contains(fid) { gated.push(fid.clone()); }
            st = ItemState::Blocked { gated_by: gated };
            hopper.set_state(id, st).await;
        }
    }
}

/// Remove `fid` from every blocked item's gate set; items with no gates left
/// return to Inbox.
pub async fn unblock_for_feedback(hopper: &InMemoryHopper, fid: &FeedbackId) {
    for item in hopper.inbox_and_blocked().await {
        if let ItemState::Blocked { gated_by } = &item.state {
            let remaining: Vec<_> = gated_by.iter().filter(|g| *g != fid).cloned().collect();
            let next = if remaining.is_empty() { ItemState::Inbox } else { ItemState::Blocked { gated_by: remaining } };
            hopper.set_state(&item.item_id, next).await;
        }
    }
}
```

If `get`, `set_state`, or `inbox_and_blocked` don't exist on `InMemoryHopper`, add them in `hopper/store.rs` as minimal RwLock<Vec> accessors (each ~4 lines) with their own one-line tests in that file, then commit those first under `feat(hopper): state accessors for gating`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator feedback::gating`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/ crates/vox-orchestrator/src/hopper/store.rs
git commit -m "feat(orchestrator): block/unblock + re-admit gating logic"
```

---

### Task 7: `vox_ask_clarification` MCP tool

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/params.rs` (add `AskClarificationParams`)
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs` (add JSON schema)
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` (route `vox_ask_clarification`)
- Create: `crates/vox-orchestrator-mcp/src/feedback_tools.rs` (handler)
- Test: inline in `feedback_tools.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn params_deserialize_with_optional_gates() {
        let j = r#"{"prompt":"schema?","options":["a","b"],"gates":[1,2]}"#;
        let p: AskClarificationParams = serde_json::from_str(j).unwrap();
        assert_eq!(p.options.len(), 2);
        assert_eq!(p.gates, vec![1, 2]);
    }

    #[test]
    fn params_default_empty_gates_and_options() {
        let p: AskClarificationParams = serde_json::from_str(r#"{"prompt":"q?"}"#).unwrap();
        assert!(p.gates.is_empty());
        assert!(p.options.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp ask_clarification`
Expected: FAIL — `AskClarificationParams` undefined.

- [ ] **Step 3: Define params + schema + handler**

In `params.rs`:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AskClarificationParams {
    pub prompt: String,
    #[serde(default)]
    pub options: Vec<String>,
    /// Hopper item ids (as submitted) this question gates; empty = non-gating.
    #[serde(default)]
    pub gates: Vec<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
}
```

In `feedback_tools.rs`, implement `ask_clarification(state, params) -> ToolResult` that:
1. Builds `InterruptionSignals` from the question (reuse the existing builder in `attention_policy.rs` — grep `pub fn` there for the clarification/chat signal constructor; pass `expected_information_gain_bits`, `expected_user_cost`, channel = chat clarification).
2. Calls the existing `evaluate_interruption(...)` with the live attention snapshot.
3. `surface = feedback::surface_for(&decision)`.
4. `let id = state.feedback.register(FeedbackKind::Clarification, prompt, options, gates_as_hopper_ids, info_gain, surface, session_id, agent_id, now_ms).await;`
5. If `surface == NeedsYou` and `!gates.is_empty()`: `feedback::block_items(&hopper, &gates, &id).await;` then emit `HopperItemBlocked`.
6. Emit `FeedbackRequested { feedback_id, kind:"clarification", gates, surface }`.
7. Return the feedback id + surface as the tool result.

Wire `vox_ask_clarification` into the `dispatch.rs` match (mirror the `vox_doubt_task` arm at the existing doubt route), and add the JSON schema in `input_schemas.rs` (mirror `DoubtTaskParams`' schema entry, with `prompt` required, `options`/`gates` optional arrays).

(Engineer note: `ServerState` must hold a `feedback: FeedbackStore`. If absent, add the field where `pending_approvals` is constructed and thread it through — grep `pending_approvals:` in the state constructor and add `feedback: FeedbackStore::new(),` beside it.)

- [ ] **Step 4: Run test + build**

Run: `cargo test -p vox-orchestrator-mcp ask_clarification && cargo build -p vox-orchestrator-mcp`
Expected: PASS; build clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/
git commit -m "feat(mcp): vox_ask_clarification tool with agent-declared gating"
```

---

### Task 8: Resolve tool + attention debit + unblock

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/params.rs` (`ResolveFeedbackParams`)
- Modify: `crates/vox-orchestrator-mcp/src/feedback_tools.rs` (handler)
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` + `input_schemas.rs`
- Test: inline

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod resolve_tests {
    use super::*;
    #[test]
    fn resolve_params_accept_choice_or_text() {
        let a: ResolveFeedbackParams = serde_json::from_str(r#"{"feedback_id":"F-1","chosen_option":0}"#).unwrap();
        assert_eq!(a.chosen_option, Some(0));
        let b: ResolveFeedbackParams = serde_json::from_str(r#"{"feedback_id":"F-1","free_text":"do X"}"#).unwrap();
        assert_eq!(b.free_text.as_deref(), Some("do X"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp resolve_params`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResolveFeedbackParams {
    pub feedback_id: String,
    #[serde(default)]
    pub chosen_option: Option<usize>,
    #[serde(default)]
    pub free_text: Option<String>,
}
```

Handler `resolve_feedback(state, params)`:
1. `let fid = FeedbackId(params.feedback_id);`
2. `let resolved = state.feedback.resolve(&fid, FeedbackResolution { chosen_option, free_text, decided_at_ms: now_ms(), decided_by: "gui".into() }).await;`
3. On `Some(req)`: `feedback::unblock_for_feedback(&hopper, &fid).await;` for each unblocked item emit `HopperItemUnblocked`; emit `FeedbackResolved { feedback_id }`.
4. Debit attention via the existing `BudgetManager::record_attention(&AttentionEvent { ... })` — construct the event the same way `chat_socrates_meta.rs` does after a surfaced question (grep there for `record_attention` and copy the field construction; `cost_ms` = the scaled interrupt cost already computed for this feedback, or `0` if it was Withheld).

Wire `vox_resolve_feedback` into dispatch + schema (mirror Task 7).

- [ ] **Step 4: Run test + build**

Run: `cargo test -p vox-orchestrator-mcp resolve_params && cargo build -p vox-orchestrator-mcp`
Expected: PASS; clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/
git commit -m "feat(mcp): vox_resolve_feedback — clear gates, re-admit, debit attention"
```

---

### Task 9: Project doubts into the feedback feed

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/doubt.rs` (where `doubt_task` sets `TaskStatus::Doubted` and emits `TaskDoubted`)
- Test: inline in `doubt.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod feedback_projection_tests {
    use super::*;
    #[test]
    fn doubt_reason_maps_to_clarification_prompt() {
        let prompt = super::doubt_feedback_prompt(Some("conflicting spec"));
        assert!(prompt.contains("conflicting spec"));
        let none = super::doubt_feedback_prompt(None);
        assert!(!none.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator feedback_projection`
Expected: FAIL — `doubt_feedback_prompt` undefined.

- [ ] **Step 3: Implement the projection helper + call it**

Add to `doubt.rs`:

```rust
/// Human-readable prompt shown in the Needs-You doubt card.
pub(crate) fn doubt_feedback_prompt(reason: Option<&str>) -> String {
    match reason {
        Some(r) if !r.is_empty() => format!("Agent flagged this task as suspect: {r}"),
        _ => "Agent flagged this task as suspect and is re-verifying.".to_string(),
    }
}
```

In `doubt_task`, after the `TaskDoubted` event is emitted, register a `FeedbackRequest { kind: Doubt, prompt: doubt_feedback_prompt(reason.as_deref()), options: vec![], gates: vec![<the doubted task's hopper id>], surface: NeedsYou, .. }` on the shared `FeedbackStore`, and emit `FeedbackRequested`. (Doubts always surface to NeedsYou — they represent an agent that has already stopped on this task.) If the orchestrator does not yet hold a `FeedbackStore` handle, add one to its constructed state beside the event bus and thread it in.

- [ ] **Step 4: Run test + build**

Run: `cargo test -p vox-orchestrator feedback_projection && cargo build -p vox-orchestrator`
Expected: PASS; clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/orchestrator/agent/doubt.rs
git commit -m "feat(orchestrator): project doubts into unified feedback feed"
```

---

### Task 10: GUI Tauri commands to read + resolve feedback

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (add `feedback_list` + `feedback_resolve` commands + DTOs)
- Modify: `crates/vox-gui/src/lib.rs` or command registration site (register the two commands)
- Test: inline `#[cfg(test)]` mod (DTO mapping)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod feedback_dto_tests {
    use super::*;
    #[test]
    fn dto_maps_kind_and_surface_to_strings() {
        let dto = FeedbackDto { feedback_id: "F-1".into(), kind: "doubt".into(), prompt: "p".into(), options: vec![], gates: vec!["H-1".into()], surface: "needs_you".into(), info_gain_bits: 0.4 };
        assert_eq!(dto.kind, "doubt");
        assert_eq!(dto.gates, vec!["H-1".to_string()]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gui feedback_dto`
Expected: FAIL — `FeedbackDto` undefined.

- [ ] **Step 3: Implement the DTO + two commands**

```rust
#[derive(Debug, serde::Serialize)]
pub struct FeedbackDto {
    pub feedback_id: String,
    pub kind: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub gates: Vec<String>,
    pub surface: String,
    pub info_gain_bits: f64,
}
```

`feedback_list()` calls the MCP `vox_feedback_list` tool (add a thin list tool in `feedback_tools.rs` returning `open_needs_you()` + `withheld()`), mapping each `FeedbackRequest` to `FeedbackDto`. `feedback_resolve(feedback_id, chosen_option, free_text)` invokes the `vox_resolve_feedback` tool. Both emit nothing themselves (the backend emits events); register them in the Tauri command list next to `hopper_list`.

- [ ] **Step 4: Run test + build**

Run: `cargo test -p vox-gui feedback_dto && cargo build -p vox-gui --lib`
Expected: PASS; lib builds. (Per project memory: build the **lib** only — `vox-gui` breaks `--all-targets` clippy via its Tauri build script.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/src/
git commit -m "feat(gui): feedback_list + feedback_resolve Tauri commands"
```

---

### Self-review notes
- Spec §3 (data model): Tasks 1, 2 — `FeedbackRequest`, `ItemState::Blocked`. ✓
- Spec §4 (backend flow): Task 4 (surface verdict, no new policy), Task 7 (raise + gate), Task 8 (resolve + re-admit + debit), Task 9 (doubt projection). ✓
- Spec §6 (events): Task 5. ✓
- Approvals untouched (non-goal). ✓ No new policy logic — `surface_for` only reads existing verdicts. ✓
- Type consistency: `FeedbackId`, `FeedbackKind`, `Surface`, `FeedbackResolution`, `block_items`, `unblock_for_feedback`, `surface_for` used consistently across tasks.
- Real-shape caveats flagged inline (AgentId path, ServerState field, InMemoryHopper accessors, InterruptionDecision field names) with the exact grep to confirm before coding — not placeholders, but guarded against drift.
