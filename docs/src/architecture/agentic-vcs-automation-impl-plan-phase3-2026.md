---
title: "Agentic VCS Automation — Phase 3 Implementation Plan (2026-05-09)"
description: "Step-by-step TDD plan that lands the dashboard surface for agentic VCS state: API routes under /api/v2/vcs/, five panels (workspace branch board, oplog viewer, push queue, capability ledger, leaked-secret diff scanner), WebSocket telemetry tap on vox.vcs.* events, and the React/HTMX-side wiring. Builds on Phases 1 and 2."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 3 makes the agentic VCS state legible to humans. Dashboard panels are how operators audit minted capabilities, undo bad ops, and see leaked-secret findings before push. Concrete code, exact file paths, exact commands. Future agents executing this plan should not need to invent code."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-dashboard: new api/v2/vcs/ module with five panels"
  - "vox-orchestrator-mcp: read-only HTTP read APIs for the panels"
  - "vox-orchestrator-queue: capability ledger query support"
  - "vox-orchestrator: WebSocket telemetry tap for vox.vcs.* events"
---

# Agentic VCS Automation — Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
>
> **Companion docs:** [Phase 1 plan](agentic-vcs-automation-impl-plan-phase1-2026.md), [Phase 2 plan](agentic-vcs-automation-impl-plan-phase2-2026.md), [research](agentic-version-control-automation-research-2026.md). Read the research doc's §"Layer 4 — Dashboard surface" before starting.

**Goal:** Make the agentic VCS state visible. After Phase 3, an operator can answer at a glance: which agents are on which branches, what they're committing, which capabilities have been minted to whom and why, what's queued for push, and which staged hunks contain secrets that would block the push. The dashboard is a *thin renderer* over already-persistent data: oplog entries, capability ledger, and `vox.vcs.*` telemetry.

**Architecture:** Five panels live under `crates/vox-dashboard/src/api/v2/vcs/`. Each panel is a route + an HTML/HTMX template; the data comes from read-only orchestrator queries that return JSON. A WebSocket route at `/api/v2/vcs/events` taps the `vox.vcs.*` `tracing` namespace via a `tracing-subscriber` layer that forwards events to a broadcast channel. The dashboard is presentational — no destructive ops are performed from it without surfacing the operation back through the existing MCP tool path with appropriate capabilities.

**Tech stack:** `axum` 0.8 (already a dep), `askama` for templates if not already present, `tokio::sync::broadcast` for the telemetry channel, `tracing-subscriber` 0.3 (likely already used somewhere — check), no new client-side framework — HTMX over the existing dashboard's HTML surface.

**Out of scope for Phase 3:**
- The Vox-language `@vcs.*` decorator UI (Phase 4).
- Cross-mesh sync of the capability ledger (replication spec).
- Rich diff visualization in the leaked-secret panel (Phase 3.5 if pursued; the MVP renders the regex match line in monospace).
- Authentication / multi-tenant capability scoping (the dashboard runs locally for now).

---

## Verification setup

- `cargo test -p vox-dashboard --lib` — handler unit tests.
- `cargo test -p vox-dashboard --test panels` — integration tests using `axum::Router::oneshot`.
- `cargo build -p vox-dashboard --bin vox-dashboard` — must compile cleanly.
- `cargo run -p vox-arch-check` — must remain green.
- Manual: `cargo run -p vox-dashboard` then visit each `/api/v2/vcs/*` route in a browser and confirm rendering.

The plan produces 7 commits.

---

## Task 1: Read-only orchestrator queries for the panels

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/services/routes/vcs_state.rs`
- Modify: `crates/vox-orchestrator-mcp/src/services/routes/mod.rs`

**Why this first:** The dashboard panels are renderers of orchestrator state. Define the JSON shape once at the orchestrator layer; the dashboard depends on those shapes, not on internal types.

- [ ] **Step 1: Write tests for the four query handlers**

Create `crates/vox-orchestrator-mcp/src/services/routes/vcs_state.rs` with the test stub at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_app() -> axum::Router {
        axum::Router::new().merge(routes(test_state().await))
    }

    #[tokio::test]
    async fn workspace_branch_board_returns_json() {
        let app = test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/vcs/branch-board").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 64).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v.get("workspaces").is_some());
    }

    #[tokio::test]
    async fn oplog_query_supports_kind_filter() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/vcs/oplog?kind=CapabilityMinted&limit=10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn push_queue_returns_pending_pr_opens() {
        let app = test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/vcs/push-queue").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn capability_ledger_returns_recent_mints() {
        let app = test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/vcs/ledger?since=24h").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
```

`test_state()` is whatever the existing `services::routes::tests` use — copy from a sibling module. If none exists, create a minimal `GatewayState::for_test_with_inmemory_oplog()` that wires an in-memory `OplogStore` impl.

- [ ] **Step 2: Implement the four handlers**

```rust
//! Read-only HTTP API for the dashboard's VCS panels.
//!
//! All four handlers are pure read paths — they query the orchestrator's
//! oplog and workspace state and serialize JSON. No mutation. The
//! dashboard's destructive affordances (e.g. "undo to op N") go back
//! through the MCP tool path with appropriate capabilities, not through
//! these routes.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};

use crate::services::GatewayState;

#[derive(Debug, Serialize)]
pub struct BranchBoardEntry {
    pub workspace_id: u64,
    pub workspace_label: String,            // e.g. "W-000042"
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub uncommitted_hunk_count: u32,
    pub last_snapshot_unix_ms: Option<u64>,
    pub conflict_count: u32,
}

#[derive(Debug, Serialize)]
pub struct BranchBoardResponse {
    pub workspaces: Vec<BranchBoardEntry>,
}

async fn workspace_branch_board(
    State(state): State<GatewayState>,
) -> Result<Json<BranchBoardResponse>, StatusCode> {
    let entries = state
        .orchestrator
        .list_workspaces()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|w| BranchBoardEntry {
            workspace_id: w.id.0,
            workspace_label: format!("W-{:06}", w.id.0),
            branch: w.bound_branch.map(|b| b.as_str().to_string()),
            ahead: w.ahead_count,
            behind: w.behind_count,
            uncommitted_hunk_count: w.uncommitted_hunks,
            last_snapshot_unix_ms: w.last_snapshot_ms,
            conflict_count: w.conflicts,
        })
        .collect();
    Ok(Json(BranchBoardResponse { workspaces: entries }))
}

#[derive(Debug, Deserialize)]
pub struct OplogQuery {
    pub kind: Option<String>,
    pub limit: Option<u32>,
    pub since: Option<String>,    // human duration like "24h" or "7d"
}

#[derive(Debug, Serialize)]
pub struct OplogEntryDto {
    pub op_id: u64,
    pub kind: String,
    pub workspace_id: Option<u64>,
    pub timestamp_unix_ms: u64,
    pub summary: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct OplogResponse {
    pub entries: Vec<OplogEntryDto>,
}

async fn oplog_query(
    State(state): State<GatewayState>,
    Query(q): Query<OplogQuery>,
) -> Result<Json<OplogResponse>, StatusCode> {
    let limit = q.limit.unwrap_or(100).min(500);
    let since_ms = q.since.as_deref().and_then(parse_duration_to_ms);
    let entries = state
        .orchestrator
        .query_oplog(q.kind.as_deref(), since_ms, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(oplog_entry_to_dto)
        .collect();
    Ok(Json(OplogResponse { entries }))
}

#[derive(Debug, Serialize)]
pub struct PushQueueEntry {
    pub workspace_id: u64,
    pub branch: String,
    pub remote: String,
    pub state: String,                       // "awaiting_ci" | "ready" | "blocked_secret_scan"
    pub commit_count: u32,
    pub ci_run_id: Option<String>,
    pub ci_conclusion: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PushQueueResponse {
    pub entries: Vec<PushQueueEntry>,
}

async fn push_queue(
    State(state): State<GatewayState>,
) -> Result<Json<PushQueueResponse>, StatusCode> {
    let entries = state
        .orchestrator
        .pending_pr_opens()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(push_pending_to_dto)
        .collect();
    Ok(Json(PushQueueResponse { entries }))
}

#[derive(Debug, Serialize)]
pub struct LedgerEntry {
    pub op_id: u64,
    pub kind: String,                        // CapabilityKind name
    pub workspace_id: u64,
    pub timestamp_unix_ms: u64,
    pub justification_hash_hex: Option<String>,
    pub justification_text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LedgerResponse {
    pub entries: Vec<LedgerEntry>,
}

async fn capability_ledger(
    State(state): State<GatewayState>,
    Query(q): Query<OplogQuery>,
) -> Result<Json<LedgerResponse>, StatusCode> {
    let since_ms = q.since.as_deref().and_then(parse_duration_to_ms);
    let entries = state
        .orchestrator
        .query_capability_ledger(since_ms, q.limit.unwrap_or(100).min(500))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(ledger_entry_to_dto)
        .collect();
    Ok(Json(LedgerResponse { entries }))
}

pub fn routes(state: GatewayState) -> Router {
    Router::new()
        .route("/vcs/branch-board", get(workspace_branch_board))
        .route("/vcs/oplog", get(oplog_query))
        .route("/vcs/push-queue", get(push_queue))
        .route("/vcs/ledger", get(capability_ledger))
        .with_state(state)
}

fn parse_duration_to_ms(s: &str) -> Option<u64> {
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    let n: u64 = s[..s.len()-1].parse().ok()?;
    let mult = match s.as_bytes().last()? {
        b'h' => 3_600_000,
        b'd' => 86_400_000,
        b'm' => 60_000,
        _ => return None,
    };
    Some(now_ms.saturating_sub(n * mult))
}

fn oplog_entry_to_dto(_e: ()) -> OplogEntryDto { /* fill in based on the actual oplog entry shape */ unimplemented!() }
fn push_pending_to_dto(_e: ()) -> PushQueueEntry { unimplemented!() }
fn ledger_entry_to_dto(_e: ()) -> LedgerEntry { unimplemented!() }
```

The three `unimplemented!()` fns are placeholders — fill them in once the orchestrator's existing oplog / pending-pr / ledger query methods are confirmed by reading the corresponding modules. The signatures above are the **dashboard contract**; the inner mapping is mechanical.

- [ ] **Step 3: Wire the routes**

In `crates/vox-orchestrator-mcp/src/services/routes/mod.rs`, add `vcs_state` to the route mount:

```rust
mod vcs_state;

pub fn router(state: GatewayState) -> Router {
    Router::new()
        // … existing routes …
        .merge(vcs_state::routes(state.clone()))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib services::routes::vcs_state`
Expected: PASS — 4/4. If a query method does not exist on the orchestrator, add it as a stub returning empty `Vec` and TODO-comment for Phase 3.5.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/services/routes/vcs_state.rs crates/vox-orchestrator-mcp/src/services/routes/mod.rs
git commit -m "feat(orchestrator-mcp): add /api/v2/vcs/{branch-board,oplog,push-queue,ledger} read-only routes"
```

---

## Task 2: WebSocket telemetry tap

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/services/routes/vcs_events.rs`
- Modify: `crates/vox-orchestrator-mcp/src/services/routes/mod.rs`
- Modify: `crates/vox-orchestrator/src/telemetry.rs` — add the broadcast subscriber layer (or wherever telemetry init happens)

**Why now:** The dashboard's "live update" affordances (e.g. push queue items appearing as PR opens fire) need a push channel from the orchestrator. WebSocket is the simplest fit and `axum` already supports it.

- [ ] **Step 1: Add a tracing layer that broadcasts vox.vcs.* events**

In `crates/vox-orchestrator/src/telemetry.rs` (create if absent):

```rust
//! Telemetry init for the orchestrator. The `VcsBroadcastLayer` taps the
//! `vox.vcs.*` namespace and forwards each event as a JSON line to a
//! `tokio::sync::broadcast` channel. The dashboard's WebSocket route
//! subscribes to a clone of the receiver.

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

#[derive(Clone)]
pub struct VcsEventTap {
    pub sender: broadcast::Sender<String>,
}

impl VcsEventTap {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}

pub struct VcsBroadcastLayer {
    pub tap: VcsEventTap,
}

impl<S> Layer<S> for VcsBroadcastLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let target = event.metadata().target();
        if !target.starts_with("vox.vcs") {
            return;
        }
        // Format: {"target": "...", "level": "INFO", "fields": {...}}
        let mut visitor = JsonFieldVisitor::default();
        event.record(&mut visitor);
        let payload = serde_json::json!({
            "target": target,
            "level": event.metadata().level().to_string(),
            "fields": visitor.fields,
        });
        let _ = self.sender_clone().send(payload.to_string());
    }
}

impl VcsBroadcastLayer {
    fn sender_clone(&self) -> broadcast::Sender<String> {
        self.tap.sender.clone()
    }
}

#[derive(Default)]
struct JsonFieldVisitor {
    fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonFieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().to_string(), serde_json::Value::String(value.to_string()));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Number(value.into()));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Number(value.into()));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(field.name().to_string(), serde_json::Value::String(format!("{:?}", value)));
    }
}
```

Wire the layer into the orchestrator's tracing init:

```rust
// in main / init
let vcs_tap = VcsEventTap::new(1024);
let layer = VcsBroadcastLayer { tap: vcs_tap.clone() };
tracing_subscriber::registry()
    .with(/* existing layers */)
    .with(layer)
    .init();
// stash vcs_tap on GatewayState so the WS handler can subscribe()
```

- [ ] **Step 2: Implement the WebSocket route**

Create `crates/vox-orchestrator-mcp/src/services/routes/vcs_events.rs`:

```rust
use axum::{
    extract::{ws::{Message, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::services::GatewayState;

pub fn routes(state: GatewayState) -> Router {
    Router::new()
        .route("/vcs/events", get(events_ws))
        .with_state(state)
}

async fn events_ws(
    State(state): State<GatewayState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let mut rx = state.vcs_tap.subscribe();
        loop {
            match rx.recv().await {
                Ok(line) => {
                    if socket.send(Message::Text(line.into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    })
}
```

Add `vcs_tap: VcsEventTap` to `GatewayState`. Mount the route in `routes/mod.rs`.

- [ ] **Step 3: Test (smoke only — full WS testing is integration-level)**

```rust
#[tokio::test]
async fn events_ws_route_exists() {
    let app = test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/vcs/events")
                .header("upgrade", "websocket")
                .header("connection", "Upgrade")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
}
```

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator/src/telemetry.rs crates/vox-orchestrator-mcp/src/services/routes/vcs_events.rs crates/vox-orchestrator-mcp/src/services/routes/mod.rs
git commit -m "feat(orchestrator): add VcsBroadcastLayer + /api/v2/vcs/events WebSocket route"
```

---

## Tasks 3–7: Five dashboard panels

Each panel is a pair of files: an HTML template + a handler that renders it. Phase 3 uses HTMX for live updates so the WebSocket from Task 2 can swap fragments into the page without a full reload.

The panels share a common skeleton; this plan documents Panel 1 in full and lists the diffs for Panels 2–5.

### Task 3: Panel 1 — Workspace branch board

**Files:**
- Create: `crates/vox-dashboard/src/api/v2/vcs/mod.rs`
- Create: `crates/vox-dashboard/src/api/v2/vcs/branch_board.rs`
- Create: `crates/vox-dashboard/templates/vcs/branch_board.html`

- [ ] **Step 1: HTML template**

```html
<!-- crates/vox-dashboard/templates/vcs/branch_board.html -->
<section class="vcs-panel" id="branch-board" hx-ext="ws" ws-connect="/api/v2/vcs/events">
  <h2>Workspace branch board</h2>
  <table>
    <thead>
      <tr><th>Workspace</th><th>Branch</th><th>Ahead</th><th>Behind</th><th>Uncommitted</th><th>Last snapshot</th><th>Conflicts</th></tr>
    </thead>
    <tbody id="branch-board-rows">
      {% for w in entries %}
      <tr data-workspace="{{ w.workspace_label }}">
        <td>{{ w.workspace_label }}</td>
        <td>{{ w.branch.as_deref().unwrap_or("(none)") }}</td>
        <td>{{ w.ahead }}</td>
        <td>{{ w.behind }}</td>
        <td>{{ w.uncommitted_hunk_count }}</td>
        <td>{{ w.last_snapshot_unix_ms.map(format_ms).unwrap_or_else(|| "—".into()) }}</td>
        <td>{{ w.conflict_count }}</td>
      </tr>
      {% endfor %}
    </tbody>
  </table>
</section>
```

(Adapt to the existing dashboard's templating engine — askama if that's what's already in use.)

- [ ] **Step 2: Handler**

```rust
//! crates/vox-dashboard/src/api/v2/vcs/branch_board.rs

use axum::{extract::State, response::Html};
use crate::AppState;

pub async fn render(State(state): State<AppState>) -> Html<String> {
    // Fetch from orchestrator's /api/v2/vcs/branch-board (Task 1).
    let body: serde_json::Value = state
        .orchestrator_client
        .get_json("/api/v2/vcs/branch-board")
        .await
        .unwrap_or_default();

    // Render template (askama or whatever the existing dashboard uses).
    Html(render_template("vcs/branch_board.html", &body))
}
```

- [ ] **Step 3: Mount under `/vcs/branch-board` in the dashboard**

In `crates/vox-dashboard/src/api/v2/vcs/mod.rs`:

```rust
pub mod branch_board;
// pub mod oplog;            // Task 4
// pub mod push_queue;       // Task 5
// pub mod ledger;           // Task 6
// pub mod secret_scanner;   // Task 7

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/branch-board", axum::routing::get(branch_board::render))
}
```

Mount `vcs::routes()` under `/api/v2/vcs/` in the dashboard's main router.

- [ ] **Step 4: Smoke test — render against a fixture state**

```rust
#[tokio::test]
async fn branch_board_renders_table() {
    let app = test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/api/v2/vcs/branch-board").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(body.contains("Workspace branch board"));
}
```

- [ ] **Step 5: Commit**

```
git add crates/vox-dashboard/src/api/v2/vcs/ crates/vox-dashboard/templates/vcs/branch_board.html
git commit -m "feat(dashboard): add Workspace branch board panel under /api/v2/vcs/"
```

### Task 4: Panel 2 — Oplog viewer

Adds `oplog.rs` + `oplog.html`. Renders a paginated list of `OperationKind` entries with filters (kind, workspace, time window). Each row has a "details" affordance that expands to show the full `details` JSON.

The "undo to op N" affordance is **read-only in Phase 3** — clicking it shows a modal that says "to undo, run `vox undo --to-op <N>` from the CLI" with the command pre-filled. Phase 3 does not execute undos from the dashboard; that requires capability minting from the dashboard, which is Phase 4 work.

Commit message: `feat(dashboard): add Oplog viewer panel with kind/workspace/time filters`

### Task 5: Panel 3 — Push queue

Adds `push_queue.rs` + `push_queue.html`. Renders pending PR opens with state column (awaiting_ci / ready / blocked_secret_scan). Each row links to the corresponding workspace in Panel 1. Live updates via the WebSocket from Task 2 — when a `vox.vcs.commit` or `vox.vcs.pr_open` event fires, HTMX swaps the row.

Commit message: `feat(dashboard): add Push queue panel with WebSocket live updates`

### Task 6: Panel 4 — Capability ledger

Adds `ledger.rs` + `ledger.html`. Renders capability mints with the justification text expanded in-line (when present). The hash column is a copyable monospace span; clicking it copies the hex to clipboard.

Filter: kind, workspace, time window, "with justification only" toggle.

Commit message: `feat(dashboard): add Capability ledger panel with justification rendering`

### Task 7: Panel 5 — Leaked-secret diff scanner

Adds `secret_scanner.rs` + `secret_scanner.html`. The data source is the result of `commit_create`'s pre-flight `scan_for_secrets` call (Phase 1) when it returns `CommitError::SecretsDetected`. The orchestrator persists those findings to a per-workspace `last_secret_scan_findings` field; the panel queries it.

The panel intentionally **does not** show the full secret string — it shows the redacted prefix from `SecretMatch::redacted` (Phase 1 design) and the file/line of the staged hunk.

Commit message: `feat(dashboard): add Leaked-secret diff scanner panel`

---

## Phase 3 acceptance criteria

- [ ] `cargo test -p vox-orchestrator-mcp --lib services::routes::vcs_state` passes (4 tests).
- [ ] `cargo test -p vox-dashboard --lib` passes (≥5 new tests).
- [ ] `cargo build -p vox-dashboard --bin vox-dashboard` succeeds.
- [ ] `cargo run -p vox-arch-check` is GREEN.
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes after the docs commit.
- [ ] Manual: each `/api/v2/vcs/*` route renders without 500 against a freshly-started orchestrator with at least one workspace.
- [ ] WebSocket at `/api/v2/vcs/events` emits a JSON line within 1 second of triggering a `vox.vcs.commit` event from a test commit.

---

## Notes for the implementing engineer

- **The dashboard is intentionally read-mostly.** Any "do X" affordance (undo, retry push, force-push from UI) goes through capability minting, which is a Phase 4 concern. Phase 3 surfaces information and copyable CLI commands, nothing more. Resist scope creep.
- **The WebSocket's broadcast channel is bounded (1024 events).** Slow consumers see `Lagged(_)`; the handler skips and continues. That's fine for a UI tap — humans don't need event-perfect history.
- **Templates assume the existing dashboard's templating engine.** If the dashboard uses a different engine than what's shown above (askama in the example), translate the syntax; the data shape is the contract.
- **The `unimplemented!()` mappers in Task 1 are real work.** Don't ship them as `unimplemented!()`. The first time the route fires they'll panic. Fill them in by reading the actual `OperationKind` shape in `vox-orchestrator-queue`.
- **Per-panel commits.** The 5 panel tasks can be reviewed in series; one commit per panel is right-sized. Don't bundle them all.
