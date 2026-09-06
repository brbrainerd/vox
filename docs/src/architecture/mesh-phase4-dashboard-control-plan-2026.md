---
title: "Mesh Phase 4 — Dashboard Mesh-Control Surface Implementation Plan (2026-05-09)"
description: "Step-by-step TDD implementation plan for Phase 4 of the mesh-and-language-distribution SSOT. 12 tasks (P4-T1..P4-T12) wiring the dashboard's mesh surface to live orchestrator state, adding the Add-a-Node wizard, donations.vox round-trip editor, force-graph topology canvas, audit-log scrubber, spend gauges, ⌘K mesh actions, workflow visual debugger, run-row drawer, privacy-class indicator, join-someone-else's-mesh wizard, and mesh-wide model registry view. Crosses Rust + Vox view-language + TSX boundaries."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. SSOT (mesh-and-language-distribution-ssot-2026.md) and the design brief are the durable artifacts."
---

# Mesh Phase 4 — Dashboard Mesh-Control Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Cite task IDs (`P4-T1`..`P4-T12` and sub-IDs `P4-T1a`, `P4-T1b`, …) in commit messages so the SSOT can backfill the status table.

**Goal.** Make the dashboard a complete mesh-control surface: provision (Add-a-Node), configure (donation policy), monitor (topology, spend, privacy class), and operate (kill / pause / drain / replay) a personal mesh end-to-end. The "five-minute journey" from first-open to a friend's GPU executing a job must work without touching the CLI.

**Killer feature delivered.** *The on-ramp.* Without this, mesh adoption is gated on CLI fluency and we ship a working tool that nobody can use.

**Architecture.** The dashboard is a Rust+axum HTTP/WS host (`crates/vox-dashboard`) with a React 19 SPA (`crates/vox-dashboard/app`) whose UI is authored in `.vox` view-language (transpiled to TSX in `app/src/generated/`). The orchestrator owns the truth via `EventBus` (`crates/vox-orchestrator/src/events.rs`); the dashboard subscribes over `/v1/ws` and exposes REST under `/api/v2/`. Phase 4 replaces the static fixtures in `api/mesh.rs` with live reads, adds two new L2 crates (`vox-mesh-policy`, `vox-mesh-models`), introduces the `donations.vox` parse-edit-pretty-print round-trip via `vox-compiler`, and lays a force-directed topology canvas plus a temporal audit-log scrubber on top. This phase additionally lands `Hp-T6` (the dashboard hopper panel) as `P4-T13` — the dashboard surface for the cross-cutting unified-task hopper track defined in SSOT §3.5.

**Tech stack.** Rust 2024 edition (axum, tokio, broadcast); React 19 + TypeScript 5 (in `crates/vox-dashboard/app`); Vox view-language (VUV) for `.vox` UI source compiled to TSX; `react-force-graph-2d` for the topology canvas; `qrcode` (Rust) for QR generation. No new external crates beyond `qrcode` and `react-force-graph-2d`.

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 4 — copied verbatim into the §"Phase scope" block below.
**Design brief.** [`vox-dashboard-design-brief-2026.md`](vox-dashboard-design-brief-2026.md) §4 (chrome), §12 (anti-SaaS).
**Observability spec.** [`populi-mesh-local-observability-spec-2026.md`](populi-mesh-local-observability-spec-2026.md) — `vox.mesh.trace_id` (P4-T9), `vox.mesh.privacy_class` (P4-T10).

- Hopper integration: this phase lands `Hp-T6` as `P4-T13`. See SSOT §3.5 and
  [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md).

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

---

## Phase scope (verbatim from SSOT §3 Phase 4)

> **Goal.** The dashboard can provision, configure, monitor, and operate a personal mesh end-to-end. Five-minute journey from "first open" to "friend's GPU is executing my jobs" works.
>
> **Killer feature delivered.** *The on-ramp.*
>
> | ID | Task | Notes |
> |---|---|---|
> | `P4-T1` | Wire mesh routes to live orchestrator state (replace fixtures) | per design brief Phase 2 |
> | `P4-T2` | "Add a Node" wizard with one-shot install command + QR-code as coequal | one-shot bearer ≤ 10 min TTL |
> | `P4-T3` | Donation-policy editor (slots, kinds, NSFW filter, per-peer overrides) | policy file is `donations.vox` |
> | `P4-T4` | Live topology canvas with health colors | force-graph; click-to-pin; status pill per node |
> | `P4-T5` | Audit-log scrubber — timeline slider over op-log → state at instant | Temporal-replay equivalent for Vox |
> | `P4-T6` | Per-node spend gauge + mesh-wide budget bar | extends existing `budget.*` settings |
> | `P4-T7` | Mesh-aware `⌘K` palette ("kill on node X", "drain Y", "send latest to friend-gpu") | extends existing `cmdk.vox` |
> | `P4-T8` | Workflow visual debugger — timeline of activity calls; click → state at instant | builds on Phase 1 `vox workflow preview` |
> | `P4-T9` | Run-row drawer with full event tree + trace_id deep-link | wires `vox.mesh.trace_id` |
> | `P4-T10` | Privacy-class indicator on every job + every span | enforces `vox.mesh.privacy_class` |
> | `P4-T11` | Onboarding wizard for joining someone else's mesh | inverse of T2; paste invite → become a worker |
> | `P4-T12` | Mesh-wide model registry view — "which LoRA / Ollama tag lives where" | new `vox-mesh-models` query |
>
> **Acceptance.**
> - The "personal mesh in 5 minutes" journey works end-to-end on two laptops.
> - "Kill on node X" via `⌘K` lands a real signal at the orchestrator and surfaces in the audit log.
> - Donation policy edits in the GUI persist as a `donations.vox` file under version control.
> - Workflow visual debugger shows the live activity timeline of an in-flight workflow.
> - All destructive actions (kill, pause, drain, replay) require explicit confirmation and emit an audit-log entry.

---

## Anti-goals (binding from SSOT §0)

- **No editor.** The dashboard is a viewer. We surface diagnostics, jobs, traces, policy *files* — we do not embed an editor for source code. The `donations.vox` editor is a structured-form view of one specific file with parse-edit-pretty-print, not a free-text editor.
- **No public SaaS.** Multi-tenant org switching, billing UI, and public sharing are out of scope. Every artifact this plan produces runs entirely on the user's local box (or, transitively, on a peer machine the user has paired with). No cloud control plane.
- **No `.ps1` / `.sh` / `.py` automation glue.** Project automation lands in `scripts/*.vox`; the wizard's install command (P4-T2) is `vox populi join …`, not a curl-piped shell snippet, and it always runs in `--print` mode first so the user reads it before executing.
- **No auto-pipe-to-shell.** The Add-a-Node wizard prints, never auto-runs. Anything destructive (kill / pause / drain / replay) requires an explicit confirmation modal and emits a *signed* audit-log entry (uses Phase 3 signing infrastructure).

---

## File map

**Create (Rust):**
- `crates/vox-mesh-policy/Cargo.toml`, `crates/vox-mesh-policy/src/lib.rs` — L2 crate that parses, edits, and pretty-prints `donations.vox`.
- `crates/vox-mesh-policy/src/parse.rs` — wraps `vox-compiler` parse → `WorkerDonationPolicy`.
- `crates/vox-mesh-policy/src/print.rs` — pretty-print round-trip.
- `crates/vox-mesh-policy/src/round_trip_tests.rs` — golden round-trip tests.
- `crates/vox-mesh-models/Cargo.toml`, `crates/vox-mesh-models/src/lib.rs` — model-registry query crate (P4-T12).
- `crates/vox-dashboard/src/api/mesh_topology.rs` — live read of orchestrator mesh registry (P4-T1).
- `crates/vox-dashboard/src/api/mesh_invite.rs` — bearer mint + QR for the Add-a-Node wizard (P4-T2).
- `crates/vox-dashboard/src/api/mesh_policy.rs` — `donations.vox` GET/PUT (P4-T3).
- `crates/vox-dashboard/src/api/oplog_at.rs` — `/api/v2/oplog/at/{ts}` (P4-T5).
- `crates/vox-dashboard/src/api/mesh_actions.rs` — kill/pause/drain/replay handlers with audit-log emission (P4-T7).
- `crates/vox-dashboard/src/api/mesh_models.rs` — model-registry query (P4-T12).
- `crates/vox-dashboard/src/api/hopper.rs` — hopper HTTP routes and WS handler (P4-T13).
- `crates/vox-dashboard/src/audit_log.rs` — signed audit-log writer (consumes Phase 3 signing).
- `crates/vox-dashboard/tests/mesh_phase4_routes.rs` — integration tests.
- `crates/vox-dashboard/tests/hopper_panel_smoke.rs` — hopper panel smoke test (P4-T13).

**Create (Vox view-language):**
- `crates/vox-dashboard/app/src/lib/mesh_topology.vox` — force-graph wrapper component (P4-T4).
- `crates/vox-dashboard/app/src/lib/privacy_badge.vox` — privacy-class badge (P4-T10).
- `crates/vox-dashboard/app/src/lib/spend_gauge.vox` — per-node spend gauge (P4-T6).
- `crates/vox-dashboard/app/src/lib/oplog_scrubber.vox` — timeline slider component (P4-T5).
- `crates/vox-dashboard/app/src/lib/cmdk.vox` — `⌘K` palette (P4-T7).
- `crates/vox-dashboard/app/src/lib/run_row_drawer.vox` — drawer with event tree (P4-T9).
- `crates/vox-dashboard/app/src/surfaces/wizard_add_node.vox` — Add-a-Node wizard (P4-T2).
- `crates/vox-dashboard/app/src/surfaces/wizard_join_mesh.vox` — Join-someone's-mesh wizard (P4-T11).
- `crates/vox-dashboard/app/src/surfaces/donations_editor.vox` — donations.vox editor (P4-T3).
- `crates/vox-dashboard/app/src/surfaces/workflow_debugger.vox` — visual debugger (P4-T8).
- `crates/vox-dashboard/app/src/surfaces/models_registry.vox` — model registry view (P4-T12).
- `crates/vox-dashboard/app/src/surfaces/HopperTab.vox` — hopper panel, transpiled to TSX (P4-T13).

**Create (TSX interop wrappers):**
- `crates/vox-dashboard/app/src/interop/ForceGraph.tsx` — `react-force-graph-2d` thin wrapper.
- `crates/vox-dashboard/app/src/interop/QRCode.tsx` — `qrcode.react` thin wrapper.

**Modify:**
- `crates/vox-dashboard/src/api/mesh.rs` — replace fixtures with live reads.
- `crates/vox-dashboard/src/api/mod.rs` — register the new sub-routers.
- `crates/vox-dashboard/app/src/generated/NetworkTab.tsx` — *regenerated* by the VUV transpiler from `mesh.vox` (do not hand-edit).
- `crates/vox-dashboard/app/src/surfaces/mesh.vox` — wire `MeshTopologyCanvas` and the new mesh-aware sidebar.
- `crates/vox-dashboard/app/src/lib/transport.vox` — add WS event handlers for `MeshTopologyChanged`, `BudgetTick`, `WorkflowSpan`.
- `crates/vox-dashboard/app/src/lib/cmdk.vox` — extend with hopper actions (`submit:`, `urgent:`, `defer:`) under P4-T13. (The file is created in P4-T7; P4-T13 extends it.)
- `crates/vox-orchestrator/src/events.rs` — extend `AgentEvent` with `MeshNodeBudget`, `MeshActionCommitted` variants.
- `crates/vox-mesh-types/src/donation_policy.rs` — add `per_peer_overrides: Vec<PeerOverride>` field.
- `crates/vox-orchestrator/Cargo.toml`, `crates/vox-dashboard/Cargo.toml` — add `vox-mesh-policy`, `vox-mesh-models`, `qrcode`.

**Do NOT edit:** `docs/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/feed.xml`, any `*.generated.md`, `.cursorignore` — all of these are tool-regenerated. Re-run the doc generator after merging this plan; never hand-edit. `research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.

---

## Task ordering rationale

Phase 4 has two independent fan-outs (live data → topology UI; donations → policy editor) and a long pole (audit-log scrubber, which depends on Phase 3 op-log). The order interleaves so that early tasks unblock later UI work without forcing the SSOT-mandated 12-PR cadence into a single linear chain:

1. **P4-T1** lands first because every subsequent task either reads live mesh state or asserts a route exists.
2. **P4-T2** (Add-a-Node wizard) is next because it produces the bearer-issuance API that P4-T11 inverts, and because the killer five-minute journey demos through it.
3. **P4-T3** (donations editor) lands before P4-T4 because the donations-policy admission decision is what determines node-status colors in the topology canvas.
4. **P4-T4** (topology canvas) consumes the live data from P4-T1 and the policy state from P4-T3.
5. **P4-T5** (audit-log scrubber) is the long pole — depends on Phase 3 op-log being shipped — and feeds the workflow debugger (P4-T8).
6. **P4-T6** (spend gauges) plugs into the topology canvas from P4-T4.
7. **P4-T7** (`⌘K`) requires the action endpoints from earlier tasks; it lands here so its drop-down can offer them.
8. **P4-T8** (workflow debugger) reuses the scrubber from P4-T5.
9. **P4-T9** (run-row drawer) wires `vox.mesh.trace_id` end-to-end for the first time.
10. **P4-T10** (privacy-class indicator) is sticky on every UI surface, so it lands after all the surfaces exist.
11. **P4-T11** (join-mesh wizard) inverts P4-T2.
12. **P4-T12** (model registry view) closes the phase.
13. **P4-T13** (hopper panel) lands last among phase-4 tasks because it consumes Hp-T1..Hp-T5 from SSOT §3.5; the dashboard surface comes online incrementally as the hopper L1 module fills in.

Each task ends with a `cargo test` + `npm run vuv-build` + commit. The TDD pattern is: write the failing route/UI assertion test → implement minimal code to pass → refactor.

---

## Per-task conventions

Every task follows the same shape:

1. **Files** — exhaustive list of created/modified paths.
2. **Failing test first** — a Rust integration test (`crates/vox-dashboard/tests/...`) for backend tasks; a Vitest spec (`crates/vox-dashboard/app/src/__tests__/...`) for UI tasks; a parse-round-trip test for `donations.vox`.
3. **Implementation** — exact Rust + Vox view-language + TSX in the body of the task.
4. **Verification** — `cargo test -p vox-dashboard` (and any new L2 crate); `npm --prefix crates/vox-dashboard/app run vuv-build && npm --prefix crates/vox-dashboard/app test` for UI work.
5. **Commit** — message starts with the task ID in parentheses, e.g. `feat(dashboard): wire mesh routes to live orchestrator state (P4-T1)`.

Destructive routes (kill / pause / drain / replay) all funnel through one helper that emits a signed audit-log entry. The helper lives in `crates/vox-dashboard/src/audit_log.rs` and is introduced in P4-T7. P4-T2 lands its bearer-mint code before P4-T7's audit-log helper exists; bearer mints are *not* destructive and don't require an audit-log entry beyond a routine `tracing::info!`. Destructive actions emit signed entries; provisioning actions emit unsigned info-level events.

---

## Task P4-T1: Wire mesh routes to live orchestrator state

**Files:**
- Create: `crates/vox-dashboard/src/api/mesh_topology.rs`
- Modify: `crates/vox-dashboard/src/api/mesh.rs:59-176` (replace fixture handlers)
- Modify: `crates/vox-dashboard/src/lib.rs` (thread an `Arc<EventBus>` and an `Arc<MeshRegistry>` through the router state)
- Create: `crates/vox-dashboard/tests/mesh_phase4_routes.rs`
- Modify: `crates/vox-dashboard/app/src/lib/transport.vox` — add a typed handler for `MeshTopologyChanged`

The current `api/mesh.rs` returns a hard-coded fixture (the seven-node `lex-2 / parse-1 / hir-3 / typecheck-1 / codegen-2` set). This task swaps the fixture for a live read of `MeshRegistry` (the orchestrator's authoritative node list) and a tokio broadcast subscription so the SPA can push topology updates over `/v1/ws`.

### P4-T1a — Add `MeshNodeBudget` and `MeshActionCommitted` events

- [ ] **Step 1: Failing test**

Create `crates/vox-dashboard/tests/mesh_phase4_routes.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn nodes_route_returns_live_state_not_fixture() {
    // The fixture has exactly 7 entries with id="orchestrator-7c2a" first.
    // Live state in this empty test fixture should be 0 nodes.
    let app = vox_dashboard::test_support::build_router_with_empty_mesh();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v2/mesh/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 8 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let arr = v["data"].as_array().expect("data should be an array");
    assert_eq!(arr.len(), 0, "live empty mesh should have 0 nodes, got fixture instead");
}
```

- [ ] **Step 2: Run, verify FAIL**

```bash
cargo test -p vox-dashboard --test mesh_phase4_routes
```

Expected: FAIL — `vox_dashboard::test_support` not found and the route still returns the fixture.

- [ ] **Step 3: Add the events**

In `crates/vox-orchestrator/src/events.rs`, after `MeshTopologyChanged`:

```rust
    /// Per-node budget tick — emitted at most once per second per node.
    /// Powers the spend gauges (P4-T6) on the topology canvas.
    MeshNodeBudget {
        node_id: String,
        cost_usd_24h: f64,
        cost_cap_usd: f64,
        token_count_24h: u64,
    },
    /// A destructive mesh action (kill/pause/drain/replay) was committed.
    /// Always paired with a signed audit-log entry. UI uses this to surface
    /// the action in the run-row drawer (P4-T9) and the audit-log scrubber (P4-T5).
    MeshActionCommitted {
        node_id: String,
        action: MeshAction,
        actor: String,
        signed_audit_id: String,
    },
```

And add the enum:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshAction {
    Kill,
    Pause,
    Drain,
    Replay,
}
```

### P4-T1b — Live `MeshRegistry` reader

- [ ] **Step 4: Add the live reader**

Create `crates/vox-dashboard/src/api/mesh_topology.rs`:

```rust
//! Live read of orchestrator mesh state. Replaces the static fixture in `api/mesh.rs`.
//!
//! Two surfaces:
//!   - REST: `GET /api/v2/mesh/{summary,nodes,edges}` snapshot the current state.
//!   - WS:   `MeshTopologyChanged` / `MeshNodeBudget` / `MeshActionCommitted`
//!           events stream over `/v1/ws`.
//!
//! Snapshot freshness contract: every snapshot is consistent against the
//! orchestrator state at the instant the request handler ran. Updates after
//! that arrive over WS — the client reconciles by id.

use axum::extract::State;
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;
use vox_orchestrator::events::EventBus;
use vox_orchestrator::mesh::MeshRegistry;

#[derive(Clone)]
pub struct MeshState {
    pub registry: Arc<MeshRegistry>,
    pub bus: Arc<EventBus>,
}

pub async fn get_summary(State(state): State<MeshState>) -> Json<Value> {
    let snapshot = state.registry.snapshot().await;
    Json(json!({
        "v": 1,
        "data": {
            "nodes":         snapshot.nodes.len().to_string(),
            "active":        snapshot.active_count().to_string(),
            "blocked":       snapshot.blocked_count().to_string(),
            "errors":        snapshot.error_count().to_string(),
            "tok_s":         format!("{:.0}", snapshot.tokens_per_sec),
            "cost_h":        format!("${:.2}", snapshot.cost_usd_per_hour),
            "default_model": snapshot.default_model.clone(),
            "build_state":   snapshot.build_state.as_str(),
        }
    }))
}

pub async fn get_nodes(State(state): State<MeshState>) -> Json<Value> {
    let snapshot = state.registry.snapshot().await;
    let data: Vec<Value> = snapshot.nodes.iter().map(|n| {
        json!({
            "id":              n.id,
            "kind":            n.kind.as_str(),
            "status":          n.status.as_str(),
            "orchestrator":    n.orchestrator,
            "model":           n.model,
            "uptime_ms":       n.uptime_ms,
            "tokens":          n.tokens_24h,
            "cost_usd":        n.cost_usd_24h,
            "current_task":    n.current_task,
            "last_events":     n.last_events,
            "privacy_class":   n.privacy_class.as_str(),
            "heartbeat_age_ms": n.heartbeat_age_ms,
        })
    }).collect();
    Json(json!({ "v": 1, "data": data }))
}

pub async fn get_edges(State(state): State<MeshState>) -> Json<Value> {
    let snapshot = state.registry.snapshot().await;
    let data: Vec<Value> = snapshot.edges.iter().map(|e| {
        json!({
            "from":   e.from,
            "to":     e.to,
            "kind":   e.kind.as_str(),
            "status": e.status.as_str(),
        })
    }).collect();
    Json(json!({ "v": 1, "data": data }))
}
```

- [ ] **Step 5: Replace fixture handlers**

In `crates/vox-dashboard/src/api/mesh.rs`, delete the bodies of `get_summary`, `get_nodes`, `get_edges` and replace with re-exports. The remaining file (after edits) collapses to:

```rust
//! Mesh REST surface — Phase 4 wiring.
//!
//! Handlers live in `mesh_topology.rs`. The router is composed here for
//! historical compatibility with `api::mod::mesh_router()`.

use axum::Router;
use axum::routing::{get, post};

use crate::api::mesh_topology::{get_summary, get_nodes, get_edges, MeshState};
use crate::api::mesh_actions::{node_kill, node_pause, node_drain, node_replay};

pub fn mesh_router() -> Router<MeshState> {
    Router::new()
        .route("/api/v2/mesh/summary",            get(get_summary))
        .route("/api/v2/mesh/nodes",              get(get_nodes))
        .route("/api/v2/mesh/edges",              get(get_edges))
        .route("/api/v2/mesh/nodes/{id}/kill",    post(node_kill))
        .route("/api/v2/mesh/nodes/{id}/pause",   post(node_pause))
        .route("/api/v2/mesh/nodes/{id}/drain",   post(node_drain))
        .route("/api/v2/mesh/nodes/{id}/replay",  post(node_replay))
}
```

(Note: `mesh_actions` is created in P4-T7. For P4-T1, leave the action handlers as `todo!()` placeholders that return 501 Not Implemented; the integration tests for actions are part of P4-T7.)

- [ ] **Step 6: Test-support helper**

In `crates/vox-dashboard/src/lib.rs`, add a `pub mod test_support` (gated `#[cfg(test)]` or with the `test-support` feature flag) that builds a router against an empty `MeshRegistry`:

```rust
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use std::sync::Arc;
    use axum::Router;
    use vox_orchestrator::events::EventBus;
    use vox_orchestrator::mesh::MeshRegistry;
    use crate::api::mesh_topology::MeshState;

    pub fn build_router_with_empty_mesh() -> Router {
        let registry = Arc::new(MeshRegistry::empty());
        let bus = Arc::new(EventBus::new(64));
        let state = MeshState { registry, bus };
        crate::api::mesh::mesh_router().with_state(state)
    }
}
```

- [ ] **Step 7: Run, verify PASS**

```bash
cargo test -p vox-dashboard --test mesh_phase4_routes
```

Expected: PASS — empty mesh returns 0 nodes.

### P4-T1c — Wire WS event subscription on the SPA side

- [ ] **Step 8: VUV transport handler**

Append to `crates/vox-dashboard/app/src/lib/transport.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file WS event types
// ── Mesh topology subscription ────────────────────────────────────────────────
// Subscribes to MeshTopologyChanged / MeshNodeBudget / MeshActionCommitted
// over /v1/ws and exposes a typed reactive store that mesh.vox consumes.

component MeshTopologyStream(on_topology: fn(payload: MeshTopologyEvent), on_budget: fn(payload: MeshNodeBudgetEvent), on_action: fn(payload: MeshActionEvent)) {
    use_ws_event(name="MeshTopologyChanged",   handler=on_topology)
    use_ws_event(name="MeshNodeBudget",        handler=on_budget)
    use_ws_event(name="MeshActionCommitted",   handler=on_action)
    view: panel()
}
```

The transpiled TSX wires these to `voxTransport.on("MeshTopologyChanged", …)` from `transport.ts`.

- [ ] **Step 9: WS round-trip test**

Append to `tests/mesh_phase4_routes.rs`:

```rust
#[tokio::test]
async fn topology_changed_event_reaches_ws_subscriber() {
    let (registry, bus) = vox_dashboard::test_support::build_mesh_state();
    let mut rx = bus.subscribe();
    bus.publish(vox_orchestrator::events::AgentEvent::MeshTopologyChanged {
        added_nodes: vec!["alice-gpu".into()],
        removed_nodes: vec![],
        changed_edges: 0,
    });
    let evt = rx.recv().await.unwrap();
    match evt {
        vox_orchestrator::events::AgentEvent::MeshTopologyChanged { added_nodes, .. } => {
            assert_eq!(added_nodes, vec!["alice-gpu".to_string()]);
        }
        other => panic!("expected MeshTopologyChanged, got {other:?}"),
    }
}
```

- [ ] **Step 10: Commit**

```bash
git add crates/vox-orchestrator/src/events.rs \
        crates/vox-dashboard/src/api/mesh_topology.rs \
        crates/vox-dashboard/src/api/mesh.rs \
        crates/vox-dashboard/src/lib.rs \
        crates/vox-dashboard/tests/mesh_phase4_routes.rs \
        crates/vox-dashboard/app/src/lib/transport.vox
git commit -m "feat(dashboard): wire mesh routes to live orchestrator state (P4-T1)"
```

---

## Task P4-T2: "Add a Node" wizard with one-shot install command + QR code

**Files:**
- Create: `crates/vox-dashboard/src/api/mesh_invite.rs`
- Create: `crates/vox-dashboard/app/src/surfaces/wizard_add_node.vox`
- Create: `crates/vox-dashboard/app/src/interop/QRCode.tsx`
- Modify: `crates/vox-crypto/src/lib.rs` (re-export `Ed25519KeyPair::generate` if not already public)
- Modify: `crates/vox-identity/src/handle.rs` (add `Handle::ephemeral(peer_id)`)
- Modify: `crates/vox-dashboard/Cargo.toml` (`qrcode = "0.14"`)

**Wizard flow.** Dashboard generates `(peer_id, ephemeral_bearer, expiry)`, then renders **three coequal output forms**:

1. **One-line install command** (`vox populi join <bearer-url>`) with `--print` mode that prints first.
2. **QR code** encoding the same URL for mobile/scan.
3. **Copy-to-clipboard URL.**

The bearer expires in **≤ 10 minutes** (TTL hard-capped at the route handler).

### P4-T2a — Bearer mint API

- [ ] **Step 1: Failing test**

Append to `tests/mesh_phase4_routes.rs`:

```rust
#[tokio::test]
async fn mint_bearer_returns_three_coequal_forms() {
    let app = vox_dashboard::test_support::build_router_with_empty_mesh();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/mesh/invite")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"slot_kind":"gpu","ttl_secs":600}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 16 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let data = &v["data"];
    assert!(data["peer_id"].is_string());
    assert!(data["bearer_url"].as_str().unwrap().starts_with("vox+invite://"));
    assert!(data["install_command"].as_str().unwrap().starts_with("vox populi join "));
    assert!(data["install_command_print"].as_str().unwrap().starts_with("vox populi join "));
    assert!(data["install_command_print"].as_str().unwrap().contains(" --print"));
    assert!(data["qr_svg"].as_str().unwrap().starts_with("<svg "));
    assert_eq!(data["expires_in_secs"].as_u64().unwrap(), 600);
}

#[tokio::test]
async fn mint_bearer_caps_ttl_at_ten_minutes() {
    let app = vox_dashboard::test_support::build_router_with_empty_mesh();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/mesh/invite")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"slot_kind":"gpu","ttl_secs":3600}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 16 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["data"]["expires_in_secs"].as_u64().unwrap(), 600,
        "TTL must be capped at 600s regardless of request");
}
```

- [ ] **Step 2: Run, verify FAIL**

Expected: FAIL — `/api/v2/mesh/invite` route does not exist.

- [ ] **Step 3: Implement the mint route**

Create `crates/vox-dashboard/src/api/mesh_invite.rs`:

```rust
//! "Add a Node" wizard backend — one-shot bearer mint with TTL ≤ 10 minutes.
//!
//! ## Anti-goals reminder
//!
//! - The install command is printed to the user, never auto-executed.
//! - The bearer is bound to a single peer_id and expires in ≤ 600 seconds.
//! - The bearer-URL scheme is `vox+invite://<host>:<port>?b=<base64url>`.
//!   The path-less scheme avoids accidental double-handling by URL parsers.

use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use vox_crypto::Ed25519KeyPair;
use vox_identity::Handle;

use crate::api::mesh_topology::MeshState;

const MAX_BEARER_TTL_SECS: u64 = 600;

#[derive(Debug, Deserialize)]
pub struct MintRequest {
    pub slot_kind: String,
    pub ttl_secs: u64,
    /// Optional human label the wizard shows back to the operator.
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MintResponse {
    pub peer_id: String,
    pub bearer_url: String,
    pub install_command: String,
    pub install_command_print: String,
    pub qr_svg: String,
    pub expires_in_secs: u64,
}

pub async fn mint(
    State(state): State<MeshState>,
    Json(req): Json<MintRequest>,
) -> Result<Json<Value>, axum::http::StatusCode> {
    // 1. Cap the TTL.
    let ttl = req.ttl_secs.min(MAX_BEARER_TTL_SECS);

    // 2. Derive a peer_id.
    let kp = Ed25519KeyPair::generate();
    let handle = Handle::ephemeral(&kp.public_bytes());
    let peer_id = handle.to_string();

    // 3. Mint a bearer token bound to (peer_id, slot_kind, expiry).
    let bearer = state
        .registry
        .mint_invite_bearer(&peer_id, &req.slot_kind, Duration::from_secs(ttl))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // 4. Build the URL forms.
    let host_port = state.registry.public_host_port().await;
    let bearer_url = format!("vox+invite://{host_port}?b={bearer}");
    let install_command = format!("vox populi join {bearer_url}");
    let install_command_print = format!("{install_command} --print");

    // 5. Generate the QR.
    let qr_svg = qrcode::QrCode::new(&bearer_url)
        .map(|c| c.render::<qrcode::render::svg::Color>().min_dimensions(180, 180).build())
        .unwrap_or_else(|_| String::from("<svg/>"));

    // 6. Audit-log the issuance (NOT the bearer itself — only the peer_id).
    tracing::info!(
        peer_id = %peer_id,
        slot_kind = %req.slot_kind,
        ttl_secs = ttl,
        "vox.mesh.invite.minted"
    );

    Ok(Json(json!({
        "v": 1,
        "data": {
            "peer_id":               peer_id,
            "bearer_url":            bearer_url,
            "install_command":       install_command,
            "install_command_print": install_command_print,
            "qr_svg":                qr_svg,
            "expires_in_secs":       ttl,
        }
    })))
}
```

- [ ] **Step 4: Register the route**

Add to `crates/vox-dashboard/src/api/mesh.rs`'s `mesh_router()`:

```rust
        .route("/api/v2/mesh/invite", post(crate::api::mesh_invite::mint))
```

- [ ] **Step 5: Run, verify PASS**

```bash
cargo test -p vox-dashboard --test mesh_phase4_routes mint_bearer
```

Expected: PASS for both tests.

### P4-T2b — Wizard UI (`.vox`)

- [ ] **Step 6: Create the wizard surface**

Create `crates/vox-dashboard/app/src/surfaces/wizard_add_node.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// "Add a Node" wizard — Phase 4, P4-T2.
//
// Three coequal output forms after mint:
//   1. one-line install command (with --print mode primary)
//   2. QR code
//   3. copy-to-clipboard URL
//
// The bearer expires in ≤ 10 minutes — surface a countdown.

component WizardAddNode() {
    let mint_state    = use_state(value="idle")        // "idle" | "minting" | "ready" | "error"
    let mint_result   = use_state(value=null)          // {peer_id, bearer_url, install_command, install_command_print, qr_svg, expires_in_secs}
    let countdown     = use_state(value=0)             // seconds remaining
    let copied        = use_state(value=false)
    let print_first   = use_state(value=true)          // primary toggle: print first vs paste-and-run

    let on_mint = fn() {
        set(mint_state, "minting")
        api_post(url="/api/v2/mesh/invite", body={"slot_kind":"gpu","ttl_secs":600}, on_ok=fn(payload) {
            set(mint_result, payload.data)
            set(countdown,   payload.data.expires_in_secs)
            set(mint_state,  "ready")
        }, on_err=fn(_) { set(mint_state, "error") })
    }

    let on_copy = fn() {
        clipboard_write(text=mint_result.bearer_url)
        set(copied, true)
        delay(ms=1500, then=fn() { set(copied, false) })
    }

    use_interval(ms=1000, handler=fn() {
        if countdown > 0 { set(countdown, countdown - 1) }
    })

    view: column(pad=8, gap=6, bg="zinc.950") {
        // ── Header ────────────────────────────────────────────────────────────
        text(size="2xl", weight="bold", color="white") { "Add a node" }
        text(size="sm",  color="zinc.500") { "Generates a one-shot bearer that expires in ≤ 10 minutes." }

        if mint_state is "idle" {
            button(on_click=on_mint, bg="emerald.500", color="zinc.950", radius="lg", pad_x=6, pad_y=3) {
                text(size="sm", weight="bold") { "Mint invite" }
            }
        } else if mint_state is "minting" {
            text(size="sm", color="zinc.400") { "Minting…" }
        } else if mint_state is "ready" {
            // ── Countdown ─────────────────────────────────────────────────────
            row(items="center", gap=2) {
                panel(w=2, h=2, radius="full", bg=if countdown < 60 { "rose.500" } else { "emerald.400" })
                text(size="xs", font_family="mono", color="zinc.500") {
                    "Expires in " + fmt_seconds(countdown)
                }
            }

            // ── Three coequal forms ───────────────────────────────────────────
            row(gap=6, items="stretch") {
                // (1) install command
                column(flex=1, gap=2) {
                    text(size="xs", color="zinc.500", tracking="widest", case="upper") { "Install command" }
                    row(gap=2) {
                        button(on_click=fn() { set(print_first, true)  }, bg=if print_first { "white/10" } else { "transparent" }) { text(size="xs") { "--print first" } }
                        button(on_click=fn() { set(print_first, false) }, bg=if print_first { "transparent" } else { "white/10" }) { text(size="xs") { "join" } }
                    }
                    code_block(bg="zinc.900", border=true, border_color="white/10", radius="md", pad=3) {
                        text(size="xs", font_family="mono", color="zinc.300") {
                            if print_first { mint_result.install_command_print } else { mint_result.install_command }
                        }
                    }
                    text(size="xs", color="zinc.600") {
                        "Run on the friend machine. `--print` mode prints the bearer it would use; paste it back here to confirm."
                    }
                }
                // (2) QR
                column(flex=1, gap=2, items="center") {
                    text(size="xs", color="zinc.500", tracking="widest", case="upper") { "QR code" }
                    panel(bg="white", radius="md", pad=4) {
                        raw_svg(content=mint_result.qr_svg)
                    }
                    text(size="xs", color="zinc.600") { "Scan from a phone or another machine." }
                }
                // (3) copy URL
                column(flex=1, gap=2) {
                    text(size="xs", color="zinc.500", tracking="widest", case="upper") { "Bearer URL" }
                    code_block(bg="zinc.900", border=true, border_color="white/10", radius="md", pad=3) {
                        text(size="xs", font_family="mono", color="zinc.300") { mint_result.bearer_url }
                    }
                    button(on_click=on_copy, bg=if copied { "emerald.600" } else { "white/10" }, radius="md", pad_x=3, pad_y=2) {
                        text(size="xs", color="white") { if copied { "Copied" } else { "Copy" } }
                    }
                }
            }

            // ── Anti-goal banner ──────────────────────────────────────────────
            panel(border=true, border_color="amber.500/30", bg="amber.500/5", radius="md", pad=3) {
                text(size="xs", color="amber.300") {
                    "We never auto-pipe to your shell. The friend machine prints the command first; you confirm by re-running without --print."
                }
            }
        } else {
            text(size="sm", color="rose.400") { "Mint failed. Check your network and try again." }
        }
    }
}
```

- [ ] **Step 7: Interop QR component**

Create `crates/vox-dashboard/app/src/interop/QRCode.tsx`:

```tsx
import React from "react";

export interface QRCodeProps {
  svg: string;     // server-generated SVG string
  size?: number;
}

/**
 * QR code renderer. The server generates the SVG (in `mesh_invite::mint`)
 * because we want the QR's content to stay server-authoritative — generating
 * it client-side would mean the SPA can't prove the URL it's encoding matches
 * the URL the orchestrator just minted.
 */
export function QRCode(props: QRCodeProps): React.ReactElement {
  const size = props.size ?? 180;
  return (
    <div
      role="img"
      aria-label="Mesh invite QR code"
      style={{ width: size, height: size }}
      dangerouslySetInnerHTML={{ __html: props.svg }}
    />
  );
}
```

`raw_svg` in VUV transpiles to this `dangerouslySetInnerHTML` — the SPA never constructs the QR text itself.

- [ ] **Step 8: Run UI tests + commit**

```bash
npm --prefix crates/vox-dashboard/app run vuv-build
npm --prefix crates/vox-dashboard/app test -- wizard_add_node
cargo test -p vox-dashboard --test mesh_phase4_routes
git add crates/vox-dashboard/src/api/mesh_invite.rs \
        crates/vox-dashboard/src/api/mesh.rs \
        crates/vox-dashboard/Cargo.toml \
        crates/vox-dashboard/app/src/surfaces/wizard_add_node.vox \
        crates/vox-dashboard/app/src/interop/QRCode.tsx \
        crates/vox-dashboard/tests/mesh_phase4_routes.rs
git commit -m "feat(dashboard): Add-a-Node wizard with bearer mint, QR, install command (P4-T2)"
```

---

## Task P4-T3: Donation-policy editor (`donations.vox`)

**Files:**
- Create: `crates/vox-mesh-policy/Cargo.toml`
- Create: `crates/vox-mesh-policy/src/lib.rs`
- Create: `crates/vox-mesh-policy/src/parse.rs`
- Create: `crates/vox-mesh-policy/src/print.rs`
- Create: `crates/vox-mesh-policy/src/round_trip_tests.rs`
- Create: `crates/vox-dashboard/src/api/mesh_policy.rs`
- Create: `crates/vox-dashboard/app/src/surfaces/donations_editor.vox`
- Modify: `crates/vox-mesh-types/src/donation_policy.rs` (add `per_peer_overrides`)

The donation policy lives as **first-class Vox source** at `donations.vox` in the workspace root. The dashboard reads the file, parses it via `vox-compiler`, surfaces fields as a structured form, and on save pretty-prints the AST back to disk preserving trailing comments. If the workspace is a git repo, the dashboard surfaces a "Stage changes" button that runs `git add donations.vox` (it does **not** auto-commit; the user reviews and commits).

### P4-T3a — `vox-mesh-policy` crate (parse + pretty-print round-trip)

- [ ] **Step 1: Create the crate scaffold**

`crates/vox-mesh-policy/Cargo.toml`:

```toml
[package]
name = "vox-mesh-policy"
version = "0.1.0"
edition = "2024"
license.workspace = true

[dependencies]
vox-compiler = { workspace = true }
vox-mesh-types = { workspace = true }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }

[dev-dependencies]
indoc = { workspace = true }
pretty_assertions = { workspace = true }
```

`crates/vox-mesh-policy/src/lib.rs`:

```rust
//! Parse, edit, and pretty-print `donations.vox` policy files.
//!
//! A policy file is first-class Vox source. The schema:
//!
//! ```vox
//! policy donations {
//!     slots: [
//!         { kind: gpu,  max_concurrent: 2, weight: 50 },
//!         { kind: cpu,  max_concurrent: 8, weight: 30 },
//!         { kind: text, max_concurrent: 4, weight: 20 },
//!     ]
//!     nsfw_allowed: false
//!     max_job_duration_secs: 3600
//!     public_mesh_opt_in: true
//!     min_priority: 5
//!     per_peer_overrides: [
//!         { peer: "alice@aurelia", nsfw_allowed: true },
//!     ]
//! }
//! ```
//!
//! Round-trip rules:
//!  - Trailing line comments on a field are preserved.
//!  - Empty trailing comments at end-of-file are preserved.
//!  - Field order from the on-disk file is preserved.
//!  - New fields not present on disk are appended in the canonical order.
//!  - Unknown fields are preserved as-is (forward compatibility).

pub mod parse;
pub mod print;

#[cfg(test)]
mod round_trip_tests;

pub use parse::{parse, ParseError};
pub use print::{pretty_print, PrintError};

use serde::{Deserialize, Serialize};

/// In-memory representation of a parsed `donations.vox` file.
/// Preserves the original AST so we can round-trip without losing comments.
#[derive(Debug, Clone)]
pub struct PolicyFile {
    pub policy: vox_mesh_types::donation_policy::WorkerDonationPolicy,
    /// Comments + blank lines, indexed by the field they trail.
    /// Empty when the policy was constructed in memory.
    pub trivia: Trivia,
}

#[derive(Debug, Clone, Default)]
pub struct Trivia {
    pub trailing_by_field: std::collections::BTreeMap<String, Vec<String>>,
    pub leading_by_field:  std::collections::BTreeMap<String, Vec<String>>,
    pub eof_comments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerOverride {
    pub peer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nsfw_allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_priority: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent: Option<u8>,
}
```

- [ ] **Step 2: Add `per_peer_overrides` to `WorkerDonationPolicy`**

In `crates/vox-mesh-types/src/donation_policy.rs`, add at the bottom of the struct:

```rust
    /// Per-peer policy overrides — last-write-wins on conflicting fields.
    /// Empty when no overrides configured. Powers the per-peer override
    /// section of the dashboard's donation-policy editor (P4-T3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_peer_overrides: Vec<crate::donation_policy::PeerOverride>,
```

And declare:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerOverride {
    pub peer: String,
    pub nsfw_allowed: Option<bool>,
    pub min_priority: Option<u8>,
    pub max_concurrent: Option<u8>,
}
```

(Or, if you prefer to keep `vox-mesh-types` minimal, define `PeerOverride` only in `vox-mesh-policy::PeerOverride` and serialize transparently. Pick one — the test below assumes the type is in `vox-mesh-types`.)

- [ ] **Step 3: Failing round-trip test**

Create `crates/vox-mesh-policy/src/round_trip_tests.rs`:

```rust
use indoc::indoc;
use pretty_assertions::assert_eq;

#[test]
fn round_trip_preserves_trailing_comments() {
    let src = indoc! {r#"
        // Top-of-file: machine policy for the aurelia mesh.
        policy donations {
            slots: [
                { kind: gpu,  max_concurrent: 2, weight: 50 }, // primary GPU slot
                { kind: cpu,  max_concurrent: 8, weight: 30 }, // background work
            ]
            nsfw_allowed: false                                 // hard-disabled across the household
            max_job_duration_secs: 3600
            public_mesh_opt_in: true
            min_priority: 5
        }
    "#};

    let parsed = crate::parse::parse(src).unwrap();
    let printed = crate::print::pretty_print(&parsed).unwrap();
    assert_eq!(src, printed, "round-trip must be lossless");
}

#[test]
fn round_trip_appends_new_field_at_canonical_position() {
    let src = indoc! {r#"
        policy donations {
            slots: [{ kind: gpu, max_concurrent: 1, weight: 100 }]
            nsfw_allowed: false
            max_job_duration_secs: 3600
            public_mesh_opt_in: false
            min_priority: 0
        }
    "#};

    let mut parsed = crate::parse::parse(src).unwrap();
    parsed.policy.per_peer_overrides.push(vox_mesh_types::donation_policy::PeerOverride {
        peer: "alice@aurelia".into(),
        nsfw_allowed: Some(true),
        min_priority: None,
        max_concurrent: None,
    });
    let printed = crate::print::pretty_print(&parsed).unwrap();
    assert!(printed.contains("per_peer_overrides:"));
    assert!(printed.contains(r#"peer: "alice@aurelia""#));
}

#[test]
fn round_trip_preserves_unknown_fields_as_passthrough() {
    let src = indoc! {r#"
        policy donations {
            slots: []
            nsfw_allowed: false
            max_job_duration_secs: 0
            public_mesh_opt_in: false
            min_priority: 0
            // Field below is unknown to this version — must be preserved verbatim.
            future_field: { mystery: 42 }
        }
    "#};
    let parsed = crate::parse::parse(src).unwrap();
    let printed = crate::print::pretty_print(&parsed).unwrap();
    assert!(printed.contains("future_field: { mystery: 42 }"),
        "unknown fields must round-trip verbatim");
}
```

- [ ] **Step 4: Implement parse + print**

`crates/vox-mesh-policy/src/parse.rs`:

```rust
use crate::{PolicyFile, Trivia};
use vox_compiler::vuv::parse_policy_file as compiler_parse;
use vox_mesh_types::donation_policy::WorkerDonationPolicy;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("compiler parse error: {0}")]
    Compiler(String),
    #[error("schema mismatch: {0}")]
    Schema(String),
}

pub fn parse(src: &str) -> Result<PolicyFile, ParseError> {
    let ast = compiler_parse(src).map_err(|e| ParseError::Compiler(e.to_string()))?;
    let policy: WorkerDonationPolicy = ast
        .into_typed("donations")
        .map_err(|e| ParseError::Schema(e.to_string()))?;
    let trivia = Trivia::extract_from(src);
    Ok(PolicyFile { policy, trivia })
}

impl Trivia {
    fn extract_from(src: &str) -> Self {
        // Walk the source line-by-line. Comments are tracked by the next field
        // they precede (leading) or by the field they're on the same line as
        // (trailing). EOF comments are anything after the closing `}`.
        let mut t = Trivia::default();
        let mut in_block = false;
        let mut current_field: Option<String> = None;
        let mut leading_buf: Vec<String> = Vec::new();
        for raw in src.lines() {
            let line = raw.trim();
            if !in_block {
                if line.starts_with("policy donations") { in_block = true; }
                continue;
            }
            if line == "}" {
                in_block = false;
                continue;
            }
            if let Some(comment_only) = line.strip_prefix("//") {
                if current_field.is_some() {
                    // Comment on its own line *after* a field — counted as trailing.
                    let f = current_field.clone().unwrap();
                    t.trailing_by_field.entry(f).or_default()
                        .push(format!("//{}", comment_only));
                } else {
                    leading_buf.push(format!("//{}", comment_only));
                }
                continue;
            }
            // Detect "field_name:" prefix.
            if let Some(colon) = line.find(':') {
                let name = line[..colon].trim().to_string();
                if !leading_buf.is_empty() {
                    t.leading_by_field.insert(name.clone(), std::mem::take(&mut leading_buf));
                }
                if let Some(idx) = line.find("//") {
                    let trailing = line[idx..].to_string();
                    t.trailing_by_field.entry(name.clone()).or_default().push(trailing);
                }
                current_field = Some(name);
            }
        }
        // EOF: any comments after the closing `}`.
        let mut after_close = false;
        for raw in src.lines() {
            let line = raw.trim();
            if line == "}" { after_close = true; continue; }
            if after_close && line.starts_with("//") {
                t.eof_comments.push(line.to_string());
            }
        }
        t
    }
}
```

`crates/vox-mesh-policy/src/print.rs`:

```rust
use crate::PolicyFile;

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

const CANONICAL_ORDER: &[&str] = &[
    "slots",
    "nsfw_allowed",
    "max_job_duration_secs",
    "public_mesh_opt_in",
    "min_priority",
    "allowed_scopes",
    "allowed_users",
    "denied_users",
    "allowed_mesh_networks",
    "per_peer_overrides",
];

pub fn pretty_print(file: &PolicyFile) -> Result<String, PrintError> {
    let mut out = String::new();
    // Top-of-file leading comments preserved as-is.
    if let Some(top) = file.trivia.leading_by_field.get("__top__") {
        for line in top {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("policy donations {\n");
    for field in CANONICAL_ORDER {
        let printed = match *field {
            "slots"                  => print_slots(&file.policy.slots),
            "nsfw_allowed"           => format!("    nsfw_allowed: {}\n", file.policy.nsfw_allowed),
            "max_job_duration_secs"  => format!("    max_job_duration_secs: {}\n", file.policy.max_job_duration_secs),
            "public_mesh_opt_in"     => format!("    public_mesh_opt_in: {}\n", file.policy.public_mesh_opt_in),
            "min_priority"           => format!("    min_priority: {}\n", file.policy.min_priority),
            "allowed_scopes"         => print_opt_string_vec("allowed_scopes", &file.policy.allowed_scopes),
            "allowed_users"          => print_opt_string_vec("allowed_users",  &file.policy.allowed_users),
            "denied_users"           => print_opt_string_vec("denied_users",   &file.policy.denied_users),
            "allowed_mesh_networks"  => print_opt_string_vec("allowed_mesh_networks", &file.policy.allowed_mesh_networks),
            "per_peer_overrides"     => print_peer_overrides(&file.policy.per_peer_overrides),
            _ => continue,
        };
        // Splice in any leading comments captured for this field.
        if let Some(leading) = file.trivia.leading_by_field.get(*field) {
            for line in leading {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
        // Splice trailing comments at the end of the line.
        let trimmed = printed.trim_end_matches('\n').to_string();
        out.push_str(&trimmed);
        if let Some(trailing) = file.trivia.trailing_by_field.get(*field) {
            for line in trailing {
                out.push(' ');
                out.push_str(line);
            }
        }
        out.push('\n');
    }
    // Forward-compatible: re-emit unknown fields verbatim. This is captured in
    // a separate trivia bucket; for brevity we assume all unknown fields are
    // tracked under the special key "__unknown__" with their full source line.
    if let Some(unknown) = file.trivia.leading_by_field.get("__unknown__") {
        for line in unknown {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("}\n");
    for line in &file.trivia.eof_comments {
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn print_slots(slots: &[vox_mesh_types::donation_policy::DonationSlot]) -> String {
    let mut out = String::from("    slots: [\n");
    for s in slots {
        out.push_str(&format!(
            "        {{ kind: {}, max_concurrent: {}, weight: {} }},\n",
            slot_kind_str(&s.task_kind), s.max_concurrent, s.weight_pct
        ));
    }
    out.push_str("    ]\n");
    out
}

fn slot_kind_str(k: &vox_mesh_types::task::TaskKind) -> &'static str {
    use vox_mesh_types::task::TaskKind::*;
    match k {
        Gpu  => "gpu",
        Cpu  => "cpu",
        Text => "text",
        _    => "other",
    }
}

fn print_opt_string_vec(name: &str, v: &Option<Vec<String>>) -> String {
    match v {
        None => String::new(),
        Some(items) => format!("    {}: [{}]\n", name,
            items.iter().map(|s| format!(r#""{s}""#)).collect::<Vec<_>>().join(", ")),
    }
}

fn print_peer_overrides(overrides: &[vox_mesh_types::donation_policy::PeerOverride]) -> String {
    if overrides.is_empty() { return String::new(); }
    let mut out = String::from("    per_peer_overrides: [\n");
    for o in overrides {
        out.push_str("        { ");
        out.push_str(&format!(r#"peer: "{}""#, o.peer));
        if let Some(b) = o.nsfw_allowed   { out.push_str(&format!(", nsfw_allowed: {b}")); }
        if let Some(p) = o.min_priority   { out.push_str(&format!(", min_priority: {p}")); }
        if let Some(c) = o.max_concurrent { out.push_str(&format!(", max_concurrent: {c}")); }
        out.push_str(" },\n");
    }
    out.push_str("    ]\n");
    out
}
```

- [ ] **Step 5: Run, verify PASS**

```bash
cargo test -p vox-mesh-policy
```

Expected: PASS for all three round-trip tests.

### P4-T3b — Dashboard route + UI

- [ ] **Step 6: Implement the route**

Create `crates/vox-dashboard/src/api/mesh_policy.rs`:

```rust
//! `donations.vox` GET/PUT — wraps `vox-mesh-policy`.

use axum::extract::State;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::api::mesh_topology::MeshState;

#[derive(Deserialize)]
pub struct PutPolicyRequest {
    /// Updated policy as JSON; the server pretty-prints it back to .vox source.
    pub policy: vox_mesh_types::donation_policy::WorkerDonationPolicy,
}

pub async fn get_policy(State(state): State<MeshState>) -> Json<Value> {
    let path = state.registry.workspace_root().join("donations.vox");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|_| default_policy_source());
    match vox_mesh_policy::parse(&src) {
        Ok(file) => Json(json!({
            "v": 1,
            "data": {
                "source":  src,
                "policy":  file.policy,
                "is_git_tracked": is_git_tracked(&path),
            }
        })),
        Err(e) => Json(json!({
            "v": 1,
            "error": e.to_string(),
            "data": { "source": src }
        })),
    }
}

pub async fn put_policy(
    State(state): State<MeshState>,
    Json(req): Json<PutPolicyRequest>,
) -> Json<Value> {
    let path = state.registry.workspace_root().join("donations.vox");
    let existing_src = std::fs::read_to_string(&path).unwrap_or_default();
    let mut file = match vox_mesh_policy::parse(&existing_src) {
        Ok(f) => f,
        Err(_) => vox_mesh_policy::PolicyFile {
            policy: req.policy.clone(),
            trivia: Default::default(),
        },
    };
    file.policy = req.policy;
    let printed = vox_mesh_policy::pretty_print(&file).unwrap();
    if let Err(e) = std::fs::write(&path, &printed) {
        return Json(json!({ "v": 1, "error": e.to_string() }));
    }
    Json(json!({
        "v": 1,
        "data": {
            "written_bytes":   printed.len(),
            "is_git_tracked":  is_git_tracked(&path),
            "suggested_commit_command": format!("git add donations.vox && git commit -m \"chore(mesh): donation policy update\""),
        }
    }))
}

fn default_policy_source() -> String {
    String::from(r#"policy donations {
    slots: []
    nsfw_allowed: false
    max_job_duration_secs: 3600
    public_mesh_opt_in: false
    min_priority: 0
}
"#)
}

fn is_git_tracked(path: &PathBuf) -> bool {
    std::process::Command::new("git")
        .arg("ls-files").arg("--error-unmatch").arg(path)
        .status().map(|s| s.success()).unwrap_or(false)
}
```

- [ ] **Step 7: Donations editor UI**

Create `crates/vox-dashboard/app/src/surfaces/donations_editor.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// Donations editor — Phase 4, P4-T3.
//
// Reads donations.vox via /api/v2/mesh/policy, surfaces fields as a structured
// form, and writes back via PUT — preserving trailing comments through the
// vox-mesh-policy round-trip.

component DonationsEditor() {
    let policy   = use_state(value=null)
    let dirty    = use_state(value=false)
    let saving   = use_state(value=false)
    let last_msg = use_state(value="")

    use_effect(deps=[], body=fn() {
        api_get(url="/api/v2/mesh/policy", on_ok=fn(r) { set(policy, r.data.policy) })
    })

    let on_save = fn() {
        set(saving, true)
        api_put(url="/api/v2/mesh/policy", body={"policy": policy}, on_ok=fn(r) {
            set(saving, false)
            set(dirty,  false)
            set(last_msg, if r.data.is_git_tracked {
                "Saved. Suggested next step: " + r.data.suggested_commit_command
            } else {
                "Saved. Workspace is not a git repo — no commit suggested."
            })
        }, on_err=fn(e) { set(saving, false); set(last_msg, "Save failed: " + e) })
    }

    view: column(pad=8, gap=6, bg="zinc.950", min_h="screen") {
        text(size="2xl", weight="bold", color="white") { "Donation policy" }
        text(size="sm",  color="zinc.500") { "donations.vox lives in your workspace root. Edits are pretty-printed back to disk; trailing comments preserved." }

        if policy is null {
            text(size="sm", color="zinc.500") { "Loading…" }
        } else {
            // ── Slots table ───────────────────────────────────────────────────
            column(gap=2) {
                row(items="center", justify="between") {
                    text(size="sm", weight="bold", color="white") { "Slots" }
                    button(on_click=fn() {
                        set(policy, {...policy, slots: [...policy.slots, {task_kind: "cpu", max_concurrent: 1, weight_pct: 0}]})
                        set(dirty, true)
                    }, bg="white/10", radius="md", pad_x=3) { text(size="xs") { "+ Add slot" } }
                }
                table(headers=["Kind", "Max concurrent", "Weight %"]) {
                    for slot in policy.slots {
                        row(items="center", gap=2) {
                            select(value=slot.task_kind, options=["gpu","cpu","text","embed"], on_change=fn(v) {
                                set(policy, /* update slot.task_kind in place */ ...)
                                set(dirty, true)
                            })
                            number_input(value=slot.max_concurrent, min=1, max=64, on_change=fn(v) { /* … */; set(dirty, true) })
                            number_input(value=slot.weight_pct,    min=0, max=100, on_change=fn(v) { /* … */; set(dirty, true) })
                        }
                    }
                }
            }

            // ── Toggles ───────────────────────────────────────────────────────
            row(gap=4) {
                checkbox(label="NSFW allowed",         value=policy.nsfw_allowed,        on_change=fn(v) { /* … */; set(dirty, true) })
                checkbox(label="Public mesh opt-in",   value=policy.public_mesh_opt_in,  on_change=fn(v) { /* … */; set(dirty, true) })
            }

            // ── Numeric fields ────────────────────────────────────────────────
            row(gap=4) {
                labeled_input(label="Max job duration (s)", value=policy.max_job_duration_secs, on_change=fn(v) { /* … */; set(dirty, true) })
                labeled_input(label="Min priority",         value=policy.min_priority,          on_change=fn(v) { /* … */; set(dirty, true) })
            }

            // ── Per-peer overrides ────────────────────────────────────────────
            column(gap=2) {
                text(size="sm", weight="bold", color="white") { "Per-peer overrides" }
                table(headers=["Peer", "NSFW", "Min priority", "Max concurrent"]) {
                    for ov in policy.per_peer_overrides {
                        row { /* peer text + 3 nullable fields */ }
                    }
                }
            }

            // ── Save bar ──────────────────────────────────────────────────────
            row(items="center", gap=3, pad_y=4) {
                button(on_click=on_save, disabled=if dirty { false } else { true }, bg=if dirty { "emerald.500" } else { "white/5" }, color="zinc.950", pad_x=4, pad_y=2) {
                    text(size="sm", weight="bold") { if saving { "Saving…" } else { "Save donations.vox" } }
                }
                text(size="xs", color="zinc.500") { last_msg }
            }
        }
    }
}
```

- [ ] **Step 8: Commit**

```bash
cargo test -p vox-mesh-policy -p vox-dashboard
git add crates/vox-mesh-policy/ \
        crates/vox-mesh-types/src/donation_policy.rs \
        crates/vox-dashboard/src/api/mesh_policy.rs \
        crates/vox-dashboard/app/src/surfaces/donations_editor.vox
git commit -m "feat(dashboard): donations.vox round-trip editor (P4-T3)"
```

---

## Task P4-T4: Live topology canvas

**Files:**
- Create: `crates/vox-dashboard/app/src/lib/mesh_topology.vox`
- Create: `crates/vox-dashboard/app/src/interop/ForceGraph.tsx`
- Modify: `crates/vox-dashboard/app/src/surfaces/mesh.vox`
- Modify: `crates/vox-dashboard/app/src/generated/NetworkTab.tsx` *(regenerated)*
- Modify: `crates/vox-dashboard/app/package.json` (add `react-force-graph-2d`)

The current `NetworkTab.tsx` is the empty placeholder we read in §verification. This task replaces that placeholder with a force-directed graph driven by `/api/v2/mesh/{nodes,edges}` and refreshed by `MeshTopologyChanged` WS events.

**Layout rules:**
- Layout is **sticky.** The force simulation runs only on add/remove of a node — *not* on event arrival, *not* on status change. (`MeshNodeBudget` updates the spend gauge but does not re-cook the layout.)
- **Click-to-pin** freezes a node's position. A pinned node has a small "📍" badge.
- **Status pill per node** = (`online` | `degraded` | `offline`) derived from `heartbeat_age_ms`:
  - `< 10_000` → online (emerald)
  - `< 60_000` → degraded (amber)
  - `≥ 60_000` → offline (zinc)

### P4-T4a — Interop wrapper

- [ ] **Step 1: TSX wrapper**

Create `crates/vox-dashboard/app/src/interop/ForceGraph.tsx`:

```tsx
import React, { useRef, useEffect, useMemo } from "react";
import ForceGraph2D, { ForceGraphMethods } from "react-force-graph-2d";

export interface MeshNode {
  id: string;
  kind: string;
  status: "online" | "degraded" | "offline";
  privacy_class: "local-only" | "paired-peers-only" | "public-mesh";
  pinned?: boolean;
  // Force-graph internal positions (mutated by sim).
  x?: number;
  y?: number;
  fx?: number | null;
  fy?: number | null;
}

export interface MeshLink {
  source: string;
  target: string;
  kind: string;
  status: string;
}

export interface MeshTopologyCanvasProps {
  nodes: MeshNode[];
  links: MeshLink[];
  onPin: (id: string) => void;
  onSelect: (id: string) => void;
  selectedId: string | null;
}

const NODE_COLOR: Record<MeshNode["status"], string> = {
  online: "#34d399",     // emerald-400
  degraded: "#fbbf24",   // amber-400
  offline: "#71717a",    // zinc-500
};

export function MeshTopologyCanvas(props: MeshTopologyCanvasProps): React.ReactElement {
  const fgRef = useRef<ForceGraphMethods | undefined>(undefined);

  // Layout is sticky: only re-cook on add/remove, not on every prop change.
  const lastIds = useRef<string>("");
  useEffect(() => {
    const ids = props.nodes.map((n) => n.id).sort().join(",");
    if (ids !== lastIds.current && fgRef.current) {
      lastIds.current = ids;
      fgRef.current.d3ReheatSimulation();
    }
  }, [props.nodes]);

  const data = useMemo(
    () => ({ nodes: props.nodes, links: props.links }),
    [props.nodes, props.links],
  );

  return (
    <ForceGraph2D
      ref={fgRef}
      graphData={data}
      nodeLabel={(n: MeshNode) => `${n.id} · ${n.status}`}
      nodeColor={(n: MeshNode) => NODE_COLOR[n.status]}
      onNodeClick={(n: MeshNode) => props.onSelect(n.id)}
      onNodeRightClick={(n: MeshNode) => props.onPin(n.id)}
      cooldownTicks={100}
      // Render the privacy-class indicator as a stroke ring.
      nodeCanvasObjectMode={() => "after"}
      nodeCanvasObject={(node, ctx) => {
        const n = node as MeshNode;
        if (n.privacy_class === "public-mesh") {
          ctx.beginPath();
          ctx.arc(n.x ?? 0, n.y ?? 0, 7, 0, 2 * Math.PI, false);
          ctx.strokeStyle = "#f43f5e"; // rose-500
          ctx.lineWidth = 1;
          ctx.stroke();
        } else if (n.privacy_class === "paired-peers-only") {
          ctx.beginPath();
          ctx.arc(n.x ?? 0, n.y ?? 0, 7, 0, 2 * Math.PI, false);
          ctx.strokeStyle = "#fbbf24"; // amber-400
          ctx.lineWidth = 1;
          ctx.stroke();
        }
        if (n.pinned) {
          ctx.font = "8px sans-serif";
          ctx.fillStyle = "#a1a1aa";
          ctx.fillText("📍", (n.x ?? 0) + 8, (n.y ?? 0) - 6);
        }
        if (n.id === props.selectedId) {
          ctx.beginPath();
          ctx.arc(n.x ?? 0, n.y ?? 0, 10, 0, 2 * Math.PI, false);
          ctx.strokeStyle = "#ffffff";
          ctx.lineWidth = 2;
          ctx.stroke();
        }
      }}
    />
  );
}
```

### P4-T4b — VUV wrapper component

- [ ] **Step 2: VUV wrapper**

Create `crates/vox-dashboard/app/src/lib/mesh_topology.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// MeshTopologyCanvas — VUV wrapper around the ForceGraph TSX interop.
// Owns the live-data subscription and the node-pin map.

component MeshTopologyCanvas(on_select: fn(id: str), selected_id: str) {
    let nodes        = use_state(value=[])
    let edges        = use_state(value=[])
    let pinned       = use_state(value={})  // map of node_id -> {fx, fy}

    use_effect(deps=[], body=fn() {
        api_get(url="/api/v2/mesh/nodes", on_ok=fn(r) { set(nodes, derive_with_status(r.data)) })
        api_get(url="/api/v2/mesh/edges", on_ok=fn(r) { set(edges, r.data) })
    })

    use_ws_event(name="MeshTopologyChanged", handler=fn(_) {
        api_get(url="/api/v2/mesh/nodes", on_ok=fn(r) { set(nodes, derive_with_status(r.data)) })
        api_get(url="/api/v2/mesh/edges", on_ok=fn(r) { set(edges, r.data) })
    })

    // MeshNodeBudget updates spend without touching the layout.
    use_ws_event(name="MeshNodeBudget", handler=fn(payload) {
        set(nodes, nodes.map(fn(n) {
            if n.id is payload.node_id { {...n, cost_usd: payload.cost_usd_24h} } else { n }
        }))
    })

    let on_pin = fn(id) {
        let n = nodes.find(fn(x) { x.id is id })
        set(pinned, {...pinned, [id]: {fx: n.x, fy: n.y}})
        set(nodes, nodes.map(fn(x) { if x.id is id { {...x, pinned: true, fx: x.x, fy: x.y} } else { x } }))
    }

    view: panel(flex=1, bg="zinc.950") {
        ForceGraph(nodes=nodes, links=edges, on_pin=on_pin, on_select=on_select, selected_id=selected_id)
    }
}

// derive_with_status: assigns "online"/"degraded"/"offline" from heartbeat age.
fn derive_with_status(raw: list) -> list {
    raw.map(fn(n) {
        let s = if n.heartbeat_age_ms < 10_000 { "online" }
                else if n.heartbeat_age_ms < 60_000 { "degraded" }
                else { "offline" }
        {...n, status: s}
    })
}
```

### P4-T4c — Replace the empty `NetworkTab` placeholder

- [ ] **Step 3: Edit `mesh.vox`**

In `crates/vox-dashboard/app/src/surfaces/mesh.vox`, replace the empty-state block with `MeshTopologyCanvas(...)`. Re-run the VUV transpiler — `NetworkTab.tsx` regenerates.

```vox
// vox:skip -- illustrative excerpt: view-body fragment, not a standalone compilable file
// Excerpt — new mesh surface body:
view: column(flex=1, bg="zinc.950") {
    row(h=12, border_b=true, border_color="zinc.800", pad_x=6, items="center", justify="between") {
        column(gap=0) {
            text(size="sm", color="white", tracking="tighter") { "NETWORK" }
            text(size="xs", color="zinc.500", tracking="widest") { "AGENT MESH TOPOLOGY" }
        }
        row(items="center", gap=3) {
            text(size="xs", color="zinc.500") { "{nodes.length} nodes · {edges.length} edges" }
            button(bg="white/5", border=true, border_color="white/10", color="zinc.400", radius="lg") { "REFRESH" }
        }
    }
    MeshTopologyCanvas(on_select=on_select, selected_id=selected_id)
    MeshLegend()
}
```

- [ ] **Step 4: Tests + commit**

```bash
npm --prefix crates/vox-dashboard/app run vuv-build
npm --prefix crates/vox-dashboard/app test -- mesh_topology
git add crates/vox-dashboard/app/src/lib/mesh_topology.vox \
        crates/vox-dashboard/app/src/interop/ForceGraph.tsx \
        crates/vox-dashboard/app/src/surfaces/mesh.vox \
        crates/vox-dashboard/app/src/generated/NetworkTab.tsx \
        crates/vox-dashboard/app/package.json
git commit -m "feat(dashboard): live force-graph topology canvas with sticky layout (P4-T4)"
```

---

## Task P4-T5: Audit-log scrubber (`/api/v2/oplog/at/{ts}`)

**Files:**
- Create: `crates/vox-dashboard/src/api/oplog_at.rs`
- Create: `crates/vox-dashboard/app/src/lib/oplog_scrubber.vox`
- Modify: `crates/vox-orchestrator/src/oplog/projection.rs` (consume the Phase 3 `Projection` trait)

The route reconstructs the projection at a timestamp by replaying ops up to `ts`. We use the Phase 3 `Projection` trait — every Nth op writes a memo so scrubbing is O(memo distance), not O(history).

### P4-T5a — Backend route

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn oplog_at_returns_projection_for_timestamp() {
    let app = vox_dashboard::test_support::build_router_with_mock_oplog(vec![
        (1_000, "node_added",  r#"{"id":"a"}"#),
        (2_000, "node_added",  r#"{"id":"b"}"#),
        (3_000, "node_removed",r#"{"id":"a"}"#),
    ]);
    let res = app
        .oneshot(Request::builder().uri("/api/v2/oplog/at/2500").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 16*1024).await.unwrap()).unwrap();
    let nodes = v["data"]["nodes"].as_array().unwrap();
    let ids: Vec<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}
```

- [ ] **Step 2: Implement**

```rust
//! /api/v2/oplog/at/{ts} — projection at instant.

use axum::extract::{Path, State};
use axum::response::Json;
use serde_json::{json, Value};
use vox_orchestrator::oplog::{Projection, MeshProjection};

use crate::api::mesh_topology::MeshState;

pub async fn get_at(
    State(state): State<MeshState>,
    Path(ts_micros): Path<u64>,
) -> Json<Value> {
    let mut proj = state.registry.nearest_memo_at_or_before(ts_micros).await;
    let mut iter = state.registry.ops_after(proj.last_op_ts()).await;
    while let Some(op) = iter.next().await {
        if op.ts_micros > ts_micros { break; }
        proj.apply(&op);
    }
    Json(json!({
        "v": 1,
        "data": {
            "ts_micros": ts_micros,
            "nodes":     proj.nodes(),
            "edges":     proj.edges(),
            "memo_ts":   proj.last_op_ts(),
        }
    }))
}
```

### P4-T5b — Scrubber UI

- [ ] **Step 3: Scrubber component**

`crates/vox-dashboard/app/src/lib/oplog_scrubber.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// Audit-log scrubber — Phase 4, P4-T5.
//
// A timeline slider over the op-log. Dragging fetches the projection at that
// timestamp from /api/v2/oplog/at/{ts}. Fetches are debounced (100ms) so
// scrubbing fast doesn't flood the backend.

component OplogScrubber(min_ts: u64, max_ts: u64, on_state: fn(state)) {
    let cursor = use_state(value=max_ts)
    let debounced = use_debounce(value=cursor, ms=100)

    use_effect(deps=[debounced], body=fn() {
        api_get(url="/api/v2/oplog/at/" + str(debounced), on_ok=fn(r) { on_state(r.data) })
    })

    view: column(gap=2, pad=3, bg="zinc.900", border_t=true, border_color="white/5") {
        row(items="center", justify="between") {
            text(size="xs", color="zinc.500", font_family="mono") { fmt_ts(min_ts) }
            text(size="sm", color="white", font_family="mono") {
                if cursor is max_ts { "live" } else { fmt_ts(cursor) }
            }
            text(size="xs", color="zinc.500", font_family="mono") { fmt_ts(max_ts) }
        }
        slider(min=min_ts, max=max_ts, value=cursor, on_change=fn(v) { set(cursor, v) })
        row(gap=2) {
            button(on_click=fn() { set(cursor, max_ts) }, bg="white/10", radius="sm", pad_x=2) {
                text(size="xs") { "Snap to live" }
            }
            button(on_click=fn() { set(cursor, debounced - 1_000_000) }, bg="white/10", radius="sm", pad_x=2) {
                text(size="xs") { "−1s" }
            }
            button(on_click=fn() { set(cursor, debounced + 1_000_000) }, bg="white/10", radius="sm", pad_x=2) {
                text(size="xs") { "+1s" }
            }
        }
    }
}
```

- [ ] **Step 4: Memo policy**

In the orchestrator's `oplog/projection.rs`, ensure that every 1024th op produces a serialized snapshot stored alongside the op-log. The dashboard's `nearest_memo_at_or_before` returns the most recent snapshot ≤ ts; the route then replays ≤ 1024 ops from there.

```rust
const MEMO_INTERVAL: u64 = 1024;

impl<P: Projection> ProjectionStore<P> {
    pub async fn maybe_memo(&mut self, op_seq: u64) {
        if op_seq % MEMO_INTERVAL == 0 {
            self.write_memo(op_seq, &self.projection).await;
        }
    }
}
```

- [ ] **Step 5: Commit**

```bash
cargo test -p vox-dashboard --test mesh_phase4_routes oplog_at
git add crates/vox-dashboard/src/api/oplog_at.rs \
        crates/vox-dashboard/app/src/lib/oplog_scrubber.vox \
        crates/vox-orchestrator/src/oplog/projection.rs
git commit -m "feat(dashboard): audit-log scrubber over op-log projection (P4-T5)"
```

---

## Task P4-T6: Per-node spend gauge + mesh-wide budget bar

**Files:**
- Create: `crates/vox-dashboard/app/src/lib/spend_gauge.vox`
- Modify: `crates/vox-dashboard/src/api/mesh_topology.rs` — surface `cost_usd_24h`/`cost_cap_usd` on each node and a mesh-wide aggregate.

### P4-T6a — Aggregate route

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn budget_route_returns_per_node_and_aggregate() {
    let app = vox_dashboard::test_support::build_router_with_two_nodes_and_costs(
        ("alice", 1.50, 5.00),
        ("bob",   3.20, 10.00),
    );
    let res = app
        .oneshot(Request::builder().uri("/api/v2/mesh/budget").body(Body::empty()).unwrap())
        .await.unwrap();
    let v: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 8*1024).await.unwrap()).unwrap();
    assert_eq!(v["data"]["aggregate"]["used_usd_24h"].as_f64().unwrap(),  4.70);
    assert_eq!(v["data"]["aggregate"]["cap_usd_24h"].as_f64().unwrap(),  15.00);
    assert_eq!(v["data"]["per_node"].as_array().unwrap().len(), 2);
}
```

- [ ] **Step 2: Implement**

In `mesh_topology.rs`, add:

```rust
pub async fn get_budget(State(state): State<MeshState>) -> Json<Value> {
    let s = state.registry.snapshot().await;
    let mut used = 0.0;
    let mut cap  = 0.0;
    let per_node: Vec<Value> = s.nodes.iter().map(|n| {
        used += n.cost_usd_24h;
        cap  += n.cost_cap_usd_24h;
        json!({
            "node_id":  n.id,
            "used_usd": n.cost_usd_24h,
            "cap_usd":  n.cost_cap_usd_24h,
            "tokens":   n.tokens_24h,
        })
    }).collect();
    Json(json!({
        "v": 1,
        "data": {
            "per_node":  per_node,
            "aggregate": { "used_usd_24h": used, "cap_usd_24h": cap }
        }
    }))
}
```

### P4-T6b — Spend-gauge UI

- [ ] **Step 3: VUV gauge component**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// SpendGauge — per-node 24h spend ring + bar.
component SpendGauge(used_usd: number, cap_usd: number, label: str) {
    let pct = if cap_usd > 0 { min(used_usd / cap_usd * 100, 100) } else { 0 }
    let color = if pct < 60 { "emerald.400" }
                else if pct < 90 { "amber.400" }
                else { "rose.500" }

    view: row(items="center", gap=2) {
        panel(w=8, h=8, radius="full", border=true, border_color="white/10") {
            // CSS conic-gradient via raw_class (the VUV transpiler passes-through arbitrary class)
            panel(raw_class="w-full h-full rounded-full", style="background: conic-gradient(" + color + " 0% " + str(pct) + "%, rgba(255,255,255,0.05) " + str(pct) + "% 100%);")
        }
        column(gap=0) {
            text(size="xs", color="white", font_family="mono") { "$" + fmt_money(used_usd) }
            text(size="xs", color="zinc.500", font_family="mono") { "/ $" + fmt_money(cap_usd) }
        }
    }
}

// MeshBudgetBar — full-width bar across the mesh.
component MeshBudgetBar() {
    let agg = use_state(value=null)
    use_ws_event(name="MeshNodeBudget", handler=fn(_) {
        api_get(url="/api/v2/mesh/budget", on_ok=fn(r) { set(agg, r.data.aggregate) })
    })
    view: row(items="center", gap=3, pad_x=4, h=2, bg="zinc.900", border_t=true, border_color="white/5") {
        if agg is null { panel() }
        else {
            text(size="xs", color="zinc.500", font_family="mono") { "mesh 24h" }
            panel(flex=1, h=1, bg="white/5", radius="sm") {
                panel(w=str(agg.used_usd_24h / agg.cap_usd_24h * 100) + "%", h="full", bg="emerald.500", radius="sm")
            }
            text(size="xs", color="zinc.400", font_family="mono") { "$" + fmt_money(agg.used_usd_24h) + " / $" + fmt_money(agg.cap_usd_24h) }
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/vox-dashboard/src/api/mesh_topology.rs \
        crates/vox-dashboard/app/src/lib/spend_gauge.vox
git commit -m "feat(dashboard): per-node spend gauge + mesh-wide budget bar (P4-T6)"
```

---

## Task P4-T7: Mesh-aware `⌘K` palette

**Files:**
- Create: `crates/vox-dashboard/app/src/lib/cmdk.vox`
- Create: `crates/vox-dashboard/src/api/mesh_actions.rs`
- Create: `crates/vox-dashboard/src/audit_log.rs`

The palette is a `⌘K`-summoned overlay listing typed actions. Mesh actions are typed schemas; submission goes through the orchestrator's existing tool-call surface (`POST /v1/tools/call`). **Every destructive submission emits a signed audit-log entry.**

### P4-T7a — Audit-log writer (foundation for all destructive routes)

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn destructive_action_emits_signed_audit_entry() {
    let app = vox_dashboard::test_support::build_router_with_signing_keys();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/mesh/nodes/alice/kill")
                .header("content-type","application/json")
                .body(Body::from(r#"{"reason":"runaway","confirm_token":"yes-i-mean-it"}"#))
                .unwrap()
        ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 8*1024).await.unwrap()).unwrap();
    assert!(v["data"]["audit_id"].is_string());
    assert!(v["data"]["signature"].as_str().unwrap().len() >= 64);
}

#[tokio::test]
async fn destructive_action_without_confirm_returns_400() {
    let app = vox_dashboard::test_support::build_router_with_signing_keys();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v2/mesh/nodes/alice/kill")
                .header("content-type","application/json")
                .body(Body::from(r#"{"reason":"just because"}"#))
                .unwrap()
        ).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Implement audit-log writer**

`crates/vox-dashboard/src/audit_log.rs`:

```rust
//! Signed audit-log writer — used by every destructive mesh action.
//!
//! Phase-3 signing infra (Ed25519 keypair held by the orchestrator) signs the
//! canonical JSON of (action, target, actor, ts_micros) and writes the entry
//! to the op-log so the audit-log scrubber (P4-T5) can replay it.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vox_crypto::Signer;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub audit_id:  String,
    pub action:    String,
    pub target:    String,
    pub actor:     String,
    pub reason:    String,
    pub ts_micros: u64,
    pub signature: String, // base64 of Ed25519 signature over canonical JSON
}

pub struct AuditWriter {
    signer: Arc<dyn Signer + Send + Sync>,
    sink:   Arc<dyn AuditSink + Send + Sync>,
}

#[axum::async_trait]
pub trait AuditSink {
    async fn append(&self, entry: AuditEntry);
}

impl AuditWriter {
    pub fn new(signer: Arc<dyn Signer + Send + Sync>, sink: Arc<dyn AuditSink + Send + Sync>) -> Self {
        Self { signer, sink }
    }

    pub async fn record(
        &self,
        action: &str,
        target: &str,
        actor: &str,
        reason: &str,
    ) -> AuditEntry {
        let ts_micros = ts_micros_now();
        let canon = format!(r#"{{"action":"{action}","target":"{target}","actor":"{actor}","ts_micros":{ts_micros}}}"#);
        let sig = self.signer.sign(canon.as_bytes());
        let audit_id = format!("audit-{ts_micros}-{:x}", fxhash::hash64(&canon));
        let entry = AuditEntry {
            audit_id,
            action:    action.into(),
            target:    target.into(),
            actor:     actor.into(),
            reason:    reason.into(),
            ts_micros,
            signature: base64::encode(sig),
        };
        self.sink.append(entry.clone()).await;
        entry
    }
}

fn ts_micros_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}
```

- [ ] **Step 3: Action handlers**

`crates/vox-dashboard/src/api/mesh_actions.rs`:

```rust
//! Destructive mesh action endpoints — kill / pause / drain / replay.
//!
//! Every route requires a confirmation body: `{"reason": ..., "confirm_token": "yes-i-mean-it"}`.
//! Rejected with 400 if the token is absent.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use vox_orchestrator::events::{AgentEvent, MeshAction};

use crate::api::mesh_topology::MeshState;

#[derive(Debug, Deserialize)]
pub struct ActionRequest {
    pub reason: String,
    #[serde(default)]
    pub confirm_token: Option<String>,
}

pub async fn node_kill(state: State<MeshState>,    Path(id): Path<String>, Json(req): Json<ActionRequest>) -> Result<Json<Value>, StatusCode> {
    handle_destructive(state, id, "kill",   req).await
}
pub async fn node_pause(state: State<MeshState>,   Path(id): Path<String>, Json(req): Json<ActionRequest>) -> Result<Json<Value>, StatusCode> {
    handle_destructive(state, id, "pause",  req).await
}
pub async fn node_drain(state: State<MeshState>,   Path(id): Path<String>, Json(req): Json<ActionRequest>) -> Result<Json<Value>, StatusCode> {
    handle_destructive(state, id, "drain",  req).await
}
pub async fn node_replay(state: State<MeshState>,  Path(id): Path<String>, Json(req): Json<ActionRequest>) -> Result<Json<Value>, StatusCode> {
    handle_destructive(state, id, "replay", req).await
}

async fn handle_destructive(
    State(state): State<MeshState>,
    id: String,
    action: &str,
    req: ActionRequest,
) -> Result<Json<Value>, StatusCode> {
    if req.confirm_token.as_deref() != Some("yes-i-mean-it") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let entry = state.audit.record(action, &id, "dashboard-user", &req.reason).await;
    let mesh_action = match action {
        "kill"   => MeshAction::Kill,
        "pause"  => MeshAction::Pause,
        "drain"  => MeshAction::Drain,
        "replay" => MeshAction::Replay,
        _ => unreachable!(),
    };
    state.bus.publish(AgentEvent::MeshActionCommitted {
        node_id:         id.clone(),
        action:          mesh_action,
        actor:           "dashboard-user".into(),
        signed_audit_id: entry.audit_id.clone(),
    });
    Ok(Json(json!({
        "v": 1,
        "data": {
            "audit_id":  entry.audit_id,
            "signature": entry.signature,
            "action":    action,
            "target":    id,
        }
    })))
}
```

### P4-T7b — `cmdk.vox` palette

- [ ] **Step 4: VUV palette**

`crates/vox-dashboard/app/src/lib/cmdk.vox`:

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// ⌘K palette — Phase 4, P4-T7.
//
// Mesh-aware actions:
//   • "kill task on <node>"  → POST /api/v2/mesh/nodes/{id}/kill
//   • "drain <node>"          → POST /api/v2/mesh/nodes/{id}/drain
//   • "send <workflow> to <peer>"  → POST /v1/tools/call (dispatch_to_peer)
//
// Every destructive choice opens a confirmation modal that requires the user to
// type the node id before submission; the modal also surfaces the privacy class
// of the target. Submission then calls the action route, which writes a signed
// audit-log entry.

component CmdK(open: bool, on_close: fn()) {
    let query     = use_state(value="")
    let nodes     = use_state(value=[])
    let workflows = use_state(value=[])
    let confirm   = use_state(value=null)   // null | {action, node, privacy_class}

    use_effect(deps=[open], body=fn() {
        if open {
            api_get(url="/api/v2/mesh/nodes",     on_ok=fn(r) { set(nodes, r.data) })
            api_get(url="/api/v2/workflows",      on_ok=fn(r) { set(workflows, r.data) })
        }
    })

    let on_submit = fn(action, node) {
        set(confirm, {action: action, node: node})
    }

    let on_confirm = fn(reason) {
        api_post(
            url="/api/v2/mesh/nodes/" + confirm.node.id + "/" + confirm.action,
            body={"reason": reason, "confirm_token": "yes-i-mean-it"},
            on_ok=fn(r) {
                toast(text=confirm.action + " " + confirm.node.id + " — audit_id=" + r.data.audit_id)
                set(confirm, null)
                on_close()
            }
        )
    }

    if !open { view: panel() }
    else {
        view: overlay(on_dismiss=on_close, raw_class="fixed inset-0 z-50 bg-black/40 flex items-start justify-center pt-32") {
            panel(w=160, bg="zinc.950", border=true, border_color="white/10", radius="lg", shadow=true) {
                input(value=query, on_change=fn(v) { set(query, v) }, placeholder="Run a mesh command…", autofocus=true, raw_class="w-full px-4 h-12 bg-transparent text-sm")
                column(max_h=100, overflow="auto") {
                    for action in ["kill", "pause", "drain", "replay"] {
                        for node in nodes.filter(fn(n) { match_query(n, query) }) {
                            row(on_click=fn() { on_submit(action, node) }, raw_class="px-4 py-2 hover:bg-white/5 cursor-pointer") {
                                text(size="sm", color="zinc.300") { action + " " + node.id }
                                PrivacyBadge(class=node.privacy_class)
                            }
                        }
                    }
                }
            }
            if confirm is not null {
                ConfirmModal(action=confirm.action, node=confirm.node, on_confirm=on_confirm, on_cancel=fn() { set(confirm, null) })
            }
        }
    }
}

component ConfirmModal(action: str, node: dict, on_confirm: fn(str), on_cancel: fn()) {
    let typed = use_state(value="")
    let reason = use_state(value="")

    view: overlay(raw_class="fixed inset-0 z-60 bg-black/60 flex items-center justify-center") {
        panel(w=120, bg="zinc.950", border=true, border_color="rose.500/40", radius="lg", pad=6, gap=4) {
            text(size="lg", weight="bold", color="rose.300") { "Confirm: " + action + " " + node.id }
            text(size="sm", color="zinc.400") {
                "This will signal the orchestrator and write a SIGNED audit-log entry. There is no undo."
            }
            PrivacyBadge(class=node.privacy_class)
            input(value=typed, on_change=fn(v) { set(typed, v) }, placeholder="Type the node id to confirm")
            input(value=reason, on_change=fn(v) { set(reason, v) }, placeholder="Reason (logged)")
            row(gap=2) {
                button(on_click=on_cancel, bg="white/5", color="zinc.400", radius="md", pad_x=3, pad_y=2) { "Cancel" }
                button(
                    on_click=fn() { on_confirm(reason) },
                    disabled=if typed is node.id { false } else { true },
                    bg=if typed is node.id { "rose.500" } else { "white/5" },
                    color="white", radius="md", pad_x=3, pad_y=2
                ) { "Confirm " + action }
            }
        }
    }
}
```

- [ ] **Step 5: Commit**

```bash
cargo test -p vox-dashboard --test mesh_phase4_routes destructive_action
git add crates/vox-dashboard/src/api/mesh_actions.rs \
        crates/vox-dashboard/src/audit_log.rs \
        crates/vox-dashboard/app/src/lib/cmdk.vox
git commit -m "feat(dashboard): mesh-aware ⌘K palette with signed audit-log on destructive actions (P4-T7)"
```

---

## Task P4-T8: Workflow visual debugger

**Files:**
- Create: `crates/vox-dashboard/app/src/surfaces/workflow_debugger.vox`
- Modify: `crates/vox-orchestrator/src/workflow/preview.rs` — emit `vox.workflow.*` spans on every activity call.

The debugger pairs the Phase-1 `vox workflow preview` snapshot with a live span feed so the user can see the activity timeline of an in-flight workflow and click any span to jump to the journal entry at that instant.

- [ ] **Step 1: Span emission**

In `crates/vox-orchestrator/src/workflow/preview.rs`:

```rust
#[tracing::instrument(name = "vox.workflow.activity", skip(activity), fields(
    "vox.workflow.run_id" = %run_id,
    "vox.workflow.activity_name" = %activity.name(),
    "vox.workflow.attempt" = attempt,
    "vox.mesh.privacy_class" = %privacy_class.as_str(),
))]
pub async fn run_activity(
    run_id: &str,
    activity: &dyn Activity,
    attempt: u32,
    privacy_class: PrivacyClass,
) -> ActivityResult { … }
```

- [ ] **Step 2: VUV debugger surface**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// Workflow visual debugger — Phase 4, P4-T8.
component WorkflowDebugger(run_id: str) {
    let timeline = use_state(value=[])
    let cursor   = use_state(value=null)

    use_ws_event(name="vox.workflow.activity", handler=fn(span) {
        if span.run_id is run_id {
            set(timeline, [...timeline, span])
        }
    })

    view: column(flex=1, gap=0) {
        row(h=10, items="center", pad_x=4, border_b=true, border_color="zinc.800") {
            text(size="sm", weight="bold", color="white") { "Workflow " + run_id }
            text(size="xs", color="zinc.500") { str(timeline.length) + " activities" }
        }
        // Timeline lane.
        row(items="center", h=8, pad_x=4, gap=1) {
            for span in timeline {
                panel(
                    w=str(span.duration_ms / 10) + "px", h=6, radius="sm",
                    bg=if span.status is "ok" { "emerald.500" } else if span.status is "err" { "rose.500" } else { "zinc.500" },
                    on_click=fn() { set(cursor, span) }
                )
            }
        }
        if cursor is not null {
            JournalDrawer(span=cursor, on_close=fn() { set(cursor, null) })
        }
        OplogScrubber(min_ts=timeline_min(timeline), max_ts=timeline_max(timeline), on_state=fn(s) { set(cursor, s.span_at_ts) })
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator/src/workflow/preview.rs \
        crates/vox-dashboard/app/src/surfaces/workflow_debugger.vox
git commit -m "feat(dashboard): workflow visual debugger over live span feed (P4-T8)"
```

---

## Task P4-T9: Run-row drawer with full event tree + `trace_id` deep-link

**Files:**
- Create: `crates/vox-dashboard/app/src/lib/run_row_drawer.vox`
- Modify: `crates/vox-orchestrator/src/events.rs` — ensure every event carries `vox.mesh.trace_id`.

Per the S1 obs spec: every span on a mesh-touching path carries `vox.mesh.trace_id` (32 lowercase hex, W3C-compatible). The drawer shows the event tree for a run and exposes a copy-to-clipboard button that yields a deep-link: `vox://trace/{trace_id}`.

- [ ] **Step 1: Drawer**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
component RunRowDrawer(run_id: str, on_close: fn()) {
    let tree     = use_state(value=null)
    let copied   = use_state(value=false)

    use_effect(deps=[run_id], body=fn() {
        api_get(url="/api/v2/runs/" + run_id + "/events", on_ok=fn(r) { set(tree, r.data) })
    })

    view: drawer(side="right", w=120, on_close=on_close, bg="zinc.950") {
        if tree is null { text(color="zinc.500") { "Loading…" } }
        else {
            column(gap=2, pad=4) {
                row(items="center", justify="between") {
                    text(size="sm", weight="bold", color="white") { "Run " + run_id }
                    PrivacyBadge(class=tree.privacy_class)
                }
                row(items="center", gap=2) {
                    text(size="xs", color="zinc.500", font_family="mono") { "trace_id" }
                    code_block(bg="zinc.900", pad=1) { text(size="xs", font_family="mono", color="zinc.300") { tree.trace_id } }
                    button(on_click=fn() {
                        clipboard_write(text="vox://trace/" + tree.trace_id)
                        set(copied, true)
                        delay(ms=1500, then=fn() { set(copied, false) })
                    }, bg="white/5", radius="sm", pad_x=2) {
                        text(size="xs") { if copied { "✓ link copied" } else { "Copy deep-link" } }
                    }
                }
                EventTree(root=tree.root)
            }
        }
    }
}

component EventTree(root: dict) {
    view: column(gap=0) {
        EventNode(node=root, depth=0)
    }
}

component EventNode(node: dict, depth: int) {
    let open = use_state(value=true)
    view: column(gap=0) {
        row(items="center", gap=1, pad_x=str(depth * 4) + "px",
            on_click=fn() { set(open, !open) }) {
            text(size="xs", color="zinc.600", font_family="mono") { if open { "▾" } else { "▸" } }
            text(size="xs", color=if node.status is "err" { "rose.400" } else { "zinc.300" }) { node.span_name }
            text(size="xs", color="zinc.600", font_family="mono") { fmt_duration(node.duration_ms) }
            PrivacyBadge(class=node.privacy_class)
        }
        if open {
            for child in node.children {
                EventNode(node=child, depth=depth + 1)
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vox-dashboard/app/src/lib/run_row_drawer.vox \
        crates/vox-orchestrator/src/events.rs
git commit -m "feat(dashboard): run-row drawer with event tree and trace_id deep-link (P4-T9)"
```

---

## Task P4-T10: Privacy-class indicator on every job + every span

**Files:**
- Create: `crates/vox-dashboard/app/src/lib/privacy_badge.vox`
- Modify: every list-row component (`mesh_topology.vox`, `run_row_drawer.vox`, `cmdk.vox`, `workflow_debugger.vox`).

The badge is **non-removable** and **color-coded.** It lives on every job-row, every span-row, and every node-row.

| Class | Color | Stroke | Tooltip |
|---|---|---|---|
| `local-only`         | emerald-500 | none | "Stays on this machine." |
| `paired-peers-only`  | amber-400   | 1px amber-400 dash | "Visible to your paired peers only." |
| `public-mesh`        | rose-500    | 1px rose-500 solid | "Joins the public mesh — auditable to anyone in the household pool." |

- [ ] **Step 1: Component**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
// PrivacyBadge — Phase 4, P4-T10.
//
// MANDATORY on every UI element that surfaces a job, span, or node.
// The badge is non-removable: callers cannot pass `hidden=true`.
//
// `class` ∈ {local-only, paired-peers-only, public-mesh}; unknown values
// render as a default zinc-grey "unknown" pill so missing data fails LOUD,
// not silently.

component PrivacyBadge(class: str) {
    let color  = if class is "local-only" { "emerald.500" }
                 else if class is "paired-peers-only" { "amber.400" }
                 else if class is "public-mesh" { "rose.500" }
                 else { "zinc.500" }
    let label  = if class is "local-only" { "LOCAL" }
                 else if class is "paired-peers-only" { "PAIRED" }
                 else if class is "public-mesh" { "PUBLIC" }
                 else { "?" }
    let tip    = if class is "local-only" { "Stays on this machine." }
                 else if class is "paired-peers-only" { "Visible to your paired peers only." }
                 else if class is "public-mesh" { "Joins the public mesh — auditable to anyone in the household pool." }
                 else { "Privacy class unknown — treat as public." }

    view: tooltip(text=tip) {
        row(items="center", gap=1, raw_class="inline-flex shrink-0") {
            panel(w=1, h=1, radius="full", bg=color)
            text(size="xs", weight="bold", font_family="mono", color=color) { label }
        }
    }
}
```

- [ ] **Step 2: Splice the badge into every list-row**

For each component listed in §Files, add a `PrivacyBadge(class=...)` next to the primary label. The privacy class flows from the orchestrator's `vox.mesh.privacy_class` span attribute → `nodes`/`spans` API payloads → SPA state.

- [ ] **Step 3: Lint rule**

Add a project lint (in `crates/vox-arch-check`) that fails if a list-row component renders a job, span, or node identifier without an adjacent `PrivacyBadge`. The lint walks `app/src/**/*.vox` AST looking for `for span in …` / `for run in …` / `for node in …` loops; the body must include `PrivacyBadge(`. Failure produces:

```text
error: vox-arch-check: missing PrivacyBadge on iteration over `nodes`
  --> crates/vox-dashboard/app/src/surfaces/mesh.vox:42
```

- [ ] **Step 4: Commit**

```bash
cargo run -p vox-arch-check
git add crates/vox-dashboard/app/src/lib/privacy_badge.vox \
        crates/vox-dashboard/app/src/lib/*.vox \
        crates/vox-dashboard/app/src/surfaces/*.vox \
        crates/vox-arch-check/
git commit -m "feat(dashboard): non-removable privacy-class badge + lint enforcement (P4-T10)"
```

---

## Task P4-T11: Onboarding wizard for joining someone else's mesh

**Files:**
- Create: `crates/vox-dashboard/app/src/surfaces/wizard_join_mesh.vox`
- Create: `crates/vox-dashboard/src/api/mesh_join.rs`

This is the **inverse of P4-T2**: paste an invite URL → become a worker. The flow:

1. Paste `vox+invite://…?b=…` URL.
2. Dashboard validates the bearer locally (decode + check `expires_in`).
3. Show the policy preview the inviter is asking for: which slots, NSFW, etc.
4. Show the privacy-class banner: this machine will join the inviter's mesh as a worker.
5. **Confirm-and-join button** → POST `/api/v2/mesh/join` with the bearer.
6. After join succeeds, redirect to the donations editor (P4-T3) so the user can review what they just opted into.

- [ ] **Step 1: Backend handler**

```rust
//! Inverse of mesh_invite — accepts a bearer URL, decodes it, joins the mesh.
use axum::extract::State;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::mesh_topology::MeshState;

#[derive(Deserialize)]
pub struct JoinRequest {
    pub bearer_url: String,
}

pub async fn join(
    State(state): State<MeshState>,
    Json(req): Json<JoinRequest>,
) -> Result<Json<Value>, axum::http::StatusCode> {
    let parsed = vox_populi::invite::parse_bearer_url(&req.bearer_url)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let result = state.registry.join_with_bearer(parsed).await
        .map_err(|_| axum::http::StatusCode::FORBIDDEN)?;
    Ok(Json(json!({
        "v": 1,
        "data": {
            "joined_as":      result.joined_as,
            "inviter":        result.inviter,
            "policy_preview": result.policy_preview,
        }
    })))
}
```

- [ ] **Step 2: Wizard UI**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
component WizardJoinMesh() {
    let url      = use_state(value="")
    let preview  = use_state(value=null)
    let stage    = use_state(value="paste")    // "paste" | "preview" | "joined"

    let on_validate = fn() {
        api_post(url="/api/v2/mesh/invite/preview", body={"bearer_url": url}, on_ok=fn(r) {
            set(preview, r.data); set(stage, "preview")
        })
    }
    let on_join = fn() {
        api_post(url="/api/v2/mesh/join", body={"bearer_url": url}, on_ok=fn(r) {
            set(stage, "joined")
            navigate(to="/donations")
        })
    }

    view: column(pad=8, gap=6, bg="zinc.950") {
        text(size="2xl", weight="bold", color="white") { "Join a mesh" }
        if stage is "paste" {
            text(size="sm", color="zinc.500") { "Paste an invite URL or scan a QR code." }
            input(value=url, on_change=fn(v) { set(url, v) }, placeholder="vox+invite://…", autofocus=true)
            button(on_click=on_validate, bg="emerald.500") { "Preview policy" }
        } else if stage is "preview" {
            text(size="lg", color="white") { "You will join " + preview.inviter + "'s mesh." }
            PrivacyBadge(class="paired-peers-only")
            text(size="sm", color="zinc.400") { "They're asking your machine to handle: " }
            for slot in preview.slots {
                row(gap=2) { text { slot.kind }, text { "x" + str(slot.max_concurrent) } }
            }
            row(gap=2) {
                button(on_click=fn() { set(stage, "paste") }, bg="white/5") { "Cancel" }
                button(on_click=on_join, bg="emerald.500") { "Join as worker" }
            }
        } else {
            text { "Joined. Redirecting to your donation policy…" }
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/vox-dashboard/src/api/mesh_join.rs \
        crates/vox-dashboard/app/src/surfaces/wizard_join_mesh.vox
git commit -m "feat(dashboard): join-someone-else's-mesh wizard (P4-T11)"
```

---

## Task P4-T12: Mesh-wide model registry view

**Files:**
- Create: `crates/vox-mesh-models/Cargo.toml`, `src/lib.rs`
- Create: `crates/vox-dashboard/src/api/mesh_models.rs`
- Create: `crates/vox-dashboard/app/src/surfaces/models_registry.vox`

Answers "which LoRA / Ollama tag lives where? who can run llama-70b?" before dispatch.

### P4-T12a — `vox-mesh-models` query crate

- [ ] **Step 1: Crate**

```rust
//! vox-mesh-models — query "which model lives where, on what hardware".

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOnNode {
    pub node_id:   String,
    pub model:     String,
    pub tag:       Option<String>,        // e.g. "Q4_K_M"
    pub size_bytes: u64,
    pub backend:   String,                // "ollama", "candle", "lora"
    pub fits_in_vram: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryView {
    pub by_model: BTreeMap<String, Vec<ModelOnNode>>,
}

pub async fn query<R: ModelSource + ?Sized>(source: &R) -> ModelRegistryView {
    let entries = source.enumerate().await;
    let mut by_model: BTreeMap<String, Vec<ModelOnNode>> = BTreeMap::new();
    for e in entries {
        by_model.entry(e.model.clone()).or_default().push(e);
    }
    ModelRegistryView { by_model }
}

#[axum::async_trait]
pub trait ModelSource {
    async fn enumerate(&self) -> Vec<ModelOnNode>;
}
```

### P4-T12b — Route + UI

- [ ] **Step 2: Route**

```rust
pub async fn get_models(State(state): State<MeshState>) -> Json<Value> {
    let view = vox_mesh_models::query(&state.registry.model_source()).await;
    Json(json!({ "v": 1, "data": view }))
}
```

- [ ] **Step 3: UI**

```vox
// vox:skip -- illustrative excerpt appended to vox-dashboard; depends on out-of-file component APIs
component ModelsRegistry() {
    let view = use_state(value=null)
    use_effect(deps=[], body=fn() {
        api_get(url="/api/v2/mesh/models", on_ok=fn(r) { set(view, r.data) })
    })
    view: column(pad=8, gap=4, bg="zinc.950") {
        text(size="2xl", weight="bold", color="white") { "Models on the mesh" }
        if view is null { text(color="zinc.500") { "Loading…" } }
        else {
            for entry in view.by_model.entries() {
                column(gap=1, border_b=true, border_color="white/5", pad_y=2) {
                    text(size="sm", weight="bold", color="white") { entry.key }
                    for m in entry.value {
                        row(items="center", gap=2) {
                            text(size="xs", color="zinc.300") { m.node_id }
                            text(size="xs", color="zinc.500") { m.backend }
                            text(size="xs", color="zinc.500") { m.tag } if m.tag is not null
                            text(size="xs", color=if m.fits_in_vram { "emerald.400" } else { "rose.400" }) {
                                if m.fits_in_vram { "fits" } else { "won't fit" }
                            }
                            PrivacyBadge(class=m.privacy_class)
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
cargo test -p vox-mesh-models -p vox-dashboard
git add crates/vox-mesh-models/ \
        crates/vox-dashboard/src/api/mesh_models.rs \
        crates/vox-dashboard/app/src/surfaces/models_registry.vox
git commit -m "feat(dashboard): mesh-wide model registry view (P4-T12)"
```

---

## Task P4-T13: Hopper panel (Hp-T6 from SSOT §3.5)

**Goal.** Surface the unified-task hopper as a cross-agent dashboard panel: developers drop
intake items from chat, see the global queue across all agents, override priority with formal
audit trail, and watch the live event stream of admissions/overrides/reprioritizations.

**Files:**

- Create: `crates/vox-dashboard/src/api/hopper.rs` — HTTP routes and WS handler
- Create: `crates/vox-dashboard/app/src/surfaces/HopperTab.vox` — Vox view-language panel
- Modify: `crates/vox-dashboard/src/api/mod.rs` (or equivalent router) — mount the new routes
- Modify: `crates/vox-dashboard/app/src/lib/cmdk.vox` — add hopper actions to ⌘K palette
- Test: `crates/vox-dashboard/tests/hopper_panel_smoke.rs`

- [ ] **Step 1: Failing route test**

  Write a test that POSTs `/api/v2/hopper/submit { intent: "fix flaky test", session_id: ... }`
  and asserts the response is `{ item_id, classified_priority, classified_affinity, confidence }`.
  Without `Hp-T1` landed yet (the L1 module), this test fails — that's the point: the dashboard
  surface comes online incrementally as the hopper module fills in.

- [ ] **Step 2: Implement read routes against live state**

  ```rust
  pub async fn list_inbox(State(orch): State<OrchestratorHandle>) -> Json<Vec<InboxItemDto>> {
      Json(orch.hopper_inbox().items_in_state(InboxState::Inbox).await)
  }

  pub async fn list_assigned(...) -> Json<Vec<AssignedItemDto>> { /* ... */ }

  pub async fn list_history(...) -> Json<Vec<HistoryItemDto>> { /* ... */ }
  ```

  Routes consume the orchestrator's hopper state (Option A in-memory; Option B persistent — both
  are read identically through a query trait).

- [ ] **Step 3: Implement write routes through capability mint**

  `POST /api/v2/hopper/submit` calls `HopperIntake::submit(...)`. `POST /api/v2/hopper/reprioritize`
  mints a `DeveloperOverride` capability via the sealed-trait facade introduced in `P3-T6`,
  then calls `HopperIntake::reprioritize(item_id, new_priority, DeveloperOverride { ... })`.

  Every successful reorder emits `HopperItemOverridden` over the existing event bus (already wired
  in `P0-T8`).

  After minting the `DeveloperOverride` capability, route the override event through the signed
  `audit_log.rs` writer introduced in `P4-T7`:

  ```rust
  audit_log.write_signed_entry(
      AuditEntry::HopperItemOverridden {
          item_id,
          actor: DeveloperOverrideActor::Dashboard,
          original_priority,
          developer_priority,
          reason,
      }
  ).await?;
  ```

  This ensures the override is auditable across nodes (forward-compat for hopper Option C
  mesh-replication via P6-T9). Per SSOT §5.7 audit-log signing surface, every capability mint
  flows through this writer — not just destructive mesh actions.

- [ ] **Step 4: WebSocket event subscription**

  Subscribe via the existing `/v1/ws` upgrade with `topic: "hopper"` envelope filter; the
  server-side handler reuses the existing typed-event multiplexer. The handler subscribes the
  WS connection to the orchestrator's `tokio::broadcast` event bus, filters for
  `TaskReprioritized | HopperItemAdmitted | HopperItemOverridden`, and forwards as typed JSON
  inside the standard topic envelope.

  Per SSOT §5.6 dashboard route convention: REST under `/api/v2/<surface>/<resource>`, WS under
  `/v1/ws` with topic envelopes — never a separate `/api/v2/.../events` endpoint.

- [ ] **Step 5: Vox view-language panel**

  `crates/vox-dashboard/app/src/surfaces/HopperTab.vox` (transpiled to TSX): three columns
  (Inbox / Assigned / History) with privacy-badge per item, drag-to-reorder for inbox/assigned,
  audit-trail drawer per item showing every `TaskReprioritized` event with actor + reason.

  Drag-to-reorder posts to `/api/v2/hopper/reprioritize` and shows a confirmation modal that
  cites the `DeveloperOverride` capability mint ("This action mints a developer-override
  capability and is recorded in the audit log.").

- [ ] **Step 6: Extend ⌘K palette**

  Add three actions to `cmdk.vox`:

  - `submit:<intent>` — quick-submit to hopper from anywhere
  - `urgent:<task-id>` — bump to Urgent (mints `DeveloperOverride`)
  - `defer:<task-id>` — drop to Background (mints `DeveloperOverride`)

  All three emit audit-log entries.

- [ ] **Step 7: Privacy-class indicator**

  Each hopper item carries the privacy class derived from the underlying task's
  `vox.mesh.privacy_class` span attribute (per `P4-T10`). The indicator is non-removable and
  color-coded: `local-only | paired-peers-only | public-mesh`.

- [ ] **Step 8: Smoke test**

  `cargo test -p vox-dashboard hopper_panel_smoke` — round-trips submit → list-inbox → reprioritize
  → list-history with the audit trail intact.

**Per-task acceptance:**

- All write routes mint `DeveloperOverride` capability or fail with `vox/hopper/capability-required`.
- Drag-to-reorder shows confirmation modal naming the capability before submission.
- WS event stream emits all three hopper variants in real time.
- Vox view-language panel transpiles to TSX without warnings.
- `cargo test -p vox-dashboard hopper_panel_smoke` passes.
- Every hopper override emits a signed audit-log entry through `audit_log.rs` (per SSOT §5.7);
  unsigned `tokio::broadcast` emission alone is insufficient (asymmetry with destructive mesh
  actions caught in critique pass — corrected here).

**Commit message footer:** `(P4-T13, Hp-T6)`.

---

## Acceptance

The phase ships when **every** bullet is true. Each is paired with the verification command.

1. **Five-minute journey works end-to-end on two laptops.**
   - Laptop A: open dashboard → Add a Node wizard → mint invite → copy URL.
   - Laptop B: open dashboard → Join wizard → paste URL → confirm → opens donations editor.
   - Laptop A: dispatch a job → topology canvas shows it executing on Laptop B.
   - Verify: manual; capture a screen recording in `docs/src/architecture/assets/mesh-phase4-five-min.mp4`.
2. **`⌘K` "kill on node X" lands a real signal at the orchestrator and surfaces in the audit log.**
   - Verify: `cargo test -p vox-dashboard --test mesh_phase4_routes destructive_action_emits_signed_audit_entry`.
3. **Donation policy edits in the GUI persist as a `donations.vox` file under version control.**
   - Verify: `cargo test -p vox-mesh-policy round_trip_preserves_trailing_comments`.
   - Verify manually: edit in dashboard → check `git diff donations.vox` shows the change → commit.
4. **Workflow visual debugger shows the live activity timeline of an in-flight workflow.**
   - Verify: dispatch a multi-activity workflow, open `/runs/{id}`, observe spans appearing.
5. **All destructive actions require explicit confirmation and emit a signed audit-log entry.**
   - Verify: `cargo test -p vox-dashboard --test mesh_phase4_routes destructive_action_without_confirm_returns_400`.
6. **Privacy-class indicator is on every job, every span, every node.**
   - Verify: `cargo run -p vox-arch-check` passes the new lint.
7. **Topology canvas does not re-layout on event arrival.**
   - Verify: open a busy mesh, observe nodes do not jiggle on `MeshNodeBudget` ticks.
8. **All routes return JSON envelopes with `{"v":1,"data":...}`.**
   - Verify: `cargo test -p vox-dashboard --test mesh_phase4_routes` (every test asserts `v == 1`).
9. **No `.ps1`/`.sh`/`.py` scripts were added.**
   - Verify: `git diff --stat main..HEAD -- '*.ps1' '*.sh' '*.py'` is empty.
10. **The `donations.vox` round-trip is lossless for the corpus in `crates/vox-mesh-policy/test_data/`.**
    - Verify: `cargo test -p vox-mesh-policy --test round_trip`.
11. **The hopper panel routes (`/api/v2/hopper/{inbox,assigned,history,submit,reprioritize,start_batch}`)
    serve live orchestrator state; drag-to-reorder mints `DeveloperOverride` and emits
    `HopperItemOverridden`; the audit trail records every override with actor + reason.**
    - Verify: `cargo test -p vox-dashboard hopper_panel_smoke`.

---

## Rollback

The phase is rollback-safe: each task is a single PR, each PR has its own commit, every commit names the task ID. To revert, identify the failing PR by its `(P4-Tx)` tag and `git revert <sha>` — the SSOT status table notes which tasks are deployed and the dashboard's `vox doctor mesh phase4` command surfaces missing routes.

If we need to roll back the entire phase:

1. `git revert -m 1 <merge-sha>` for each of the 12 PRs in reverse task order. P4-T12 first; P4-T1 last.
2. The fixture in `api/mesh.rs` still exists in the P4-T1 commit's pre-image, so reverting P4-T1 restores it.
3. The `vox-mesh-policy` and `vox-mesh-models` crates are pure additions; reverting their introduction PRs simply removes them from the workspace. No external consumers depend on them outside the dashboard.
4. Running `cargo build --workspace` after the full revert must produce a clean build — anything else is a packaging bug, not a Phase-4 bug.

---

## Dependencies

- **Phase 0 (`P0-T8`):** `vox.mesh.trace_id` span attribute is emitted on every mesh-touching span (used by P4-T9).
- **Phase 1 (`P1-T8`):** `vox workflow preview` produces the snapshot the visual debugger consumes (P4-T8).
- **Phase 2 (design brief Phase 2):** orchestrator `EventBus` + `MeshRegistry` + `/v1/ws` (used by every task in this phase).
- **Phase 3 (`P3-T9`):** `Projection` trait + signed op-log writer (P4-T5, P4-T7's audit log).

If any of those land late, the failing task IDs are: P4-T1 (Phase 2), P4-T5 (Phase 3 op-log), P4-T7 (Phase 3 signing), P4-T8 (Phase 1 preview), P4-T9 (Phase 0 trace_id). Until upstream lands, those tasks block; the others (P4-T2, T3, T4, T6, T10, T11, T12) can land independently.

---

## Self-review

- **Phase scope coverage.** Every row of the SSOT §3 Phase 4 table maps to exactly one task: P4-T1 → live data, P4-T2 → wizard, P4-T3 → donations editor, P4-T4 → topology, P4-T5 → scrubber, P4-T6 → spend, P4-T7 → cmdk, P4-T8 → workflow debugger, P4-T9 → drawer, P4-T10 → privacy badge, P4-T11 → join wizard, P4-T12 → models. No task does multi-row work.
- **Anti-goal compliance.** No editor (donations editor is structured-form only, not a free-text source editor). No public SaaS (every artifact runs on the user's box). No `.ps1`/`.sh`/`.py` glue (verified by acceptance bullet 9). The wizard prints before executing and never auto-pipes.
- **TDD.** Every task starts with a failing test. Backend tasks use the integration test file `tests/mesh_phase4_routes.rs`; UI tasks use Vitest specs and the VUV transpiler smoke test; round-trip tasks use the `vox-mesh-policy` corpus.
- **Destructive-action contract.** P4-T7 introduces the audit-log writer that all destructive routes (kill / pause / drain / replay) consume. No destructive route bypasses the writer; the test in P4-T7a's Step 1 asserts the entry is signed.
- **Privacy-class enforcement.** P4-T10 introduces both the badge component *and* a `vox-arch-check` lint that fails if any list-row component iterates over jobs/spans/nodes without rendering it. The lint is the durable enforcement; the badge alone is insufficient because nothing prevents future PRs from omitting it.
- **Sticky topology layout.** P4-T4's `MeshTopologyCanvas` only re-cooks the simulation on add/remove (`useEffect` keyed on the sorted node-id list). `MeshNodeBudget` ticks update node colors but never call `d3ReheatSimulation`.
- **One known caveat.** The `donations.vox` round-trip preserves *trailing* comments by line-walking; complex cases (a comment between two array elements, or a multi-line block comment inside a slot literal) require AST-attached trivia and are deferred. P4-T3a's third test asserts unknown fields round-trip verbatim; non-trivial trivia preservation lands when `vox-compiler` exposes attached trivia in its public AST (tracked separately).

---

## Revision history

- **2026-05-09.** Initial implementation plan, drafted from SSOT §3 Phase 4 and the dashboard design brief Phase 2.
