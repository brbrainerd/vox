# Orchestrator A2A MessageBus-First Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make same-process agent-to-agent messages travel through an in-memory tokio broadcast channel instead of VoxDB, reducing local A2A latency from poll-interval-bounded to sub-millisecond. VoxDB remains the durable path for cross-process and remote messages.

**Architecture:** Introduce `LocalA2AChannel` (thin wrapper over `tokio::sync::broadcast`) and `A2ARouter` (delivery-plane decision: local channel vs. DB). `A2ARouter` tracks which agent IDs are local to this process. `Orchestrator` registers/unregisters agents in the router at their lifecycle boundaries. The existing `send_to_db` / `poll_inbox_from_db` path is unchanged — the router just calls it only when the target is not local. Durable messages (audit-trail required) always go to DB regardless of locality.

**Tech Stack:** Rust, `tokio::sync::broadcast`, `parking_lot::RwLock`, `cargo test`

**Prerequisite:** Plan 1 (Foundation) must be complete.

---

## File Map

| Action | Path |
|---|---|
| CREATE | `crates/vox-orchestrator/src/a2a/local_channel.rs` |
| CREATE | `crates/vox-orchestrator/src/a2a/router.rs` |
| MODIFY | `crates/vox-orchestrator/src/a2a/mod.rs` — export new types |
| MODIFY | `crates/vox-orchestrator/src/a2a/dispatch/` — call router before DB |
| MODIFY | `crates/vox-orchestrator/src/orchestrator.rs` (or `orchestrator/mod.rs`) — register in router |
| MODIFY | `crates/vox-orchestrator-types/src/lib.rs` — add `require_durable` field to `A2AMessage` |

---

## Task 1: Add `require_durable` flag to `A2AMessage`

Some messages must be persisted regardless of whether the target is local — for audit trails,
replay, and regulatory compliance. We add a boolean flag to the existing wire type.
This is a backward-compatible addition (serde default = false).

**Files:**
- Modify: `crates/vox-orchestrator-types/src/lib.rs` (or wherever `A2AMessage` is defined)

- [ ] **Step 1: Find the `A2AMessage` struct**

```powershell
rg "struct A2AMessage" crates/ --files-with-matches
```

Open that file. Note the existing fields.

- [ ] **Step 2: Write a failing test**

Add to the same file or its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a2a_message_defaults_require_durable_to_false() {
        let json = r#"{"id":"test-id","sender_agent_id":{"0":1},"recipient_agent_id":{"0":2},"message_type":"clarification","payload":"hello"}"#;
        let msg: A2AMessage = serde_json::from_str(json).unwrap();
        assert!(!msg.require_durable, "require_durable must default to false for backward compat");
    }

    #[test]
    fn a2a_message_require_durable_serializes() {
        let mut msg = A2AMessage::default_test();
        msg.require_durable = true;
        let json = serde_json::to_string(&msg).unwrap();
        let round: A2AMessage = serde_json::from_str(&json).unwrap();
        assert!(round.require_durable);
    }
}
```

- [ ] **Step 3: Run to verify failure**

```powershell
cargo test -p vox-orchestrator-types 2>&1 | tail -10
```

Expected: compile error — `require_durable` field does not exist yet.

- [ ] **Step 4: Add the field to `A2AMessage`**

Find the `A2AMessage` struct definition. Add:

```rust
/// When true, the message MUST be persisted to VoxDB regardless of whether
/// the target agent is local to this process. Use for audit-trail-required
/// messages (budget alerts, security events, task completions).
///
/// Defaults to `false`. Local agents receive the message via the broadcast
/// channel AND it is also written to DB when this flag is set.
#[serde(default)]
pub require_durable: bool,
```

Also add a test helper if one doesn't exist:

```rust
#[cfg(test)]
impl A2AMessage {
    pub fn default_test() -> Self {
        Self {
            id: MessageId(uuid::Uuid::new_v4().as_u128() as u64),
            sender_agent_id: AgentId(1),
            recipient_agent_id: AgentId(2),
            message_type: A2AMessageType::Clarification,
            payload: "test".to_string(),
            require_durable: false,
        }
    }
}
```

- [ ] **Step 5: Run tests**

```powershell
cargo test -p vox-orchestrator-types 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-types/
git commit -m "feat(orchestrator-types): add require_durable field to A2AMessage"
```

---

## Task 2: Create `LocalA2AChannel`

**Files:**
- Create: `crates/vox-orchestrator/src/a2a/local_channel.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-orchestrator/src/a2a/local_channel.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator_types::{A2AMessage, A2AMessageType, AgentId, MessageId};

    fn test_message(sender: u64, recipient: u64) -> A2AMessage {
        A2AMessage {
            id: MessageId(sender * 1000 + recipient),
            sender_agent_id: AgentId(sender),
            recipient_agent_id: AgentId(recipient),
            message_type: A2AMessageType::Clarification,
            payload: format!("msg-{}-{}", sender, recipient),
            require_durable: false,
        }
    }

    #[tokio::test]
    async fn send_and_receive_local_message() {
        let channel = LocalA2AChannel::new();
        let mut rx = channel.subscribe();

        let msg = test_message(1, 2);
        channel.send(msg.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.id, msg.id);
        assert_eq!(received.payload, "msg-1-2");
    }

    #[tokio::test]
    async fn multiple_subscribers_all_receive() {
        let channel = LocalA2AChannel::new();
        let mut rx1 = channel.subscribe();
        let mut rx2 = channel.subscribe();

        channel.send(test_message(1, 2)).unwrap();

        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        assert_eq!(r1.id, r2.id, "both subscribers receive the same message");
    }

    #[test]
    fn receiver_count_matches_subscribers() {
        let channel = LocalA2AChannel::new();
        assert_eq!(channel.receiver_count(), 0);
        let _rx1 = channel.subscribe();
        assert_eq!(channel.receiver_count(), 1);
        let _rx2 = channel.subscribe();
        assert_eq!(channel.receiver_count(), 2);
    }

    #[tokio::test]
    async fn send_to_empty_channel_returns_error() {
        let channel = LocalA2AChannel::new();
        // No subscribers — send should return Err (no receivers)
        let result = channel.send(test_message(1, 2));
        assert!(result.is_err(), "send with no receivers must return Err");
    }
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
cargo test -p vox-orchestrator local_channel 2>&1 | tail -10
```

Expected: compile error — `LocalA2AChannel` type not defined.

- [ ] **Step 3: Write the implementation**

```rust
//! In-process broadcast channel for agent-to-agent messages.
//!
//! `LocalA2AChannel` is the PRIMARY delivery path for same-process agents.
//! It uses a tokio broadcast channel (capacity 1024) for sub-millisecond
//! local delivery with no DB serialization.
//!
//! The VoxDB path (`send_to_db` / `poll_inbox_from_db`) remains the
//! cross-process and durable delivery path.

use tokio::sync::broadcast;
use vox_orchestrator_types::A2AMessage;

/// Capacity of the local broadcast channel.
/// 1024 slots = ~8MB at typical message sizes; lagged receivers get
/// `RecvError::Lagged` which is handled gracefully by dropping old messages.
const LOCAL_BUS_CAPACITY: usize = 1024;

/// Shared in-process broadcast channel for A2A messages.
///
/// Clone this to share across threads — the inner `Sender` is cheap to clone.
/// Each caller that wants to receive messages must call `subscribe()`.
#[derive(Clone, Debug)]
pub struct LocalA2AChannel {
    tx: broadcast::Sender<A2AMessage>,
}

impl LocalA2AChannel {
    /// Create a new channel. Typically one instance per `Orchestrator`.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(LOCAL_BUS_CAPACITY);
        Self { tx }
    }

    /// Subscribe to receive all subsequent messages on this channel.
    ///
    /// The returned `Receiver` will miss messages sent before `subscribe()` was
    /// called — this is intentional (no replay needed for local delivery).
    pub fn subscribe(&self) -> broadcast::Receiver<A2AMessage> {
        self.tx.subscribe()
    }

    /// Broadcast a message to all current subscribers.
    ///
    /// Returns `Err` if there are no active receivers (i.e., no agents are
    /// currently subscribed). Callers should fall back to `send_to_db` in
    /// this case.
    pub fn send(
        &self,
        msg: A2AMessage,
    ) -> Result<usize, broadcast::error::SendError<A2AMessage>> {
        self.tx.send(msg)
    }

    /// Number of active receivers (subscribed agents).
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for LocalA2AChannel {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Register the module in `a2a/mod.rs`**

Add to `crates/vox-orchestrator/src/a2a/mod.rs`:

```rust
pub mod local_channel;
pub use local_channel::LocalA2AChannel;
```

For `A2AMessage` to implement `Clone` (required by broadcast), verify it derives `Clone`.
If not, add `#[derive(Clone)]` to `A2AMessage` in `vox-orchestrator-types`.

- [ ] **Step 5: Run tests**

```powershell
cargo test -p vox-orchestrator local_channel 2>&1 | tail -10
```

Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator/src/a2a/local_channel.rs
git add crates/vox-orchestrator/src/a2a/mod.rs
git add crates/vox-orchestrator-types/  # if Clone was added
git commit -m "feat(a2a): add LocalA2AChannel — tokio broadcast for same-process delivery"
```

---

## Task 3: Create `A2ARouter`

The router holds the set of local agent IDs and decides which delivery plane to use for each
outbound message.

**Files:**
- Create: `crates/vox-orchestrator/src/a2a/router.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-orchestrator/src/a2a/router.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vox_orchestrator_types::{A2AMessage, A2AMessageType, AgentId, MessageId};

    fn make_router() -> A2ARouter {
        let channel = Arc::new(LocalA2AChannel::new());
        A2ARouter::new(channel)
    }

    fn msg(recipient: u64, durable: bool) -> A2AMessage {
        A2AMessage {
            id: MessageId(recipient),
            sender_agent_id: AgentId(0),
            recipient_agent_id: AgentId(recipient),
            message_type: A2AMessageType::Clarification,
            payload: "test".to_string(),
            require_durable: durable,
        }
    }

    #[test]
    fn unregistered_agent_is_not_local() {
        let router = make_router();
        assert!(!router.is_local(&AgentId(99)));
    }

    #[test]
    fn registered_agent_is_local() {
        let router = make_router();
        router.register_local(AgentId(42));
        assert!(router.is_local(&AgentId(42)));
    }

    #[test]
    fn unregistered_agent_is_no_longer_local() {
        let router = make_router();
        router.register_local(AgentId(7));
        router.unregister_local(&AgentId(7));
        assert!(!router.is_local(&AgentId(7)));
    }

    #[test]
    fn try_deliver_local_consumes_message_for_local_agent() {
        let channel = Arc::new(LocalA2AChannel::new());
        // Need at least one subscriber for the send to succeed
        let _rx = channel.subscribe();
        let router = A2ARouter::new(Arc::clone(&channel));
        router.register_local(AgentId(5));

        let remainder = router.try_deliver_local(msg(5, false));
        assert!(remainder.is_none(), "message to local agent must be consumed");
    }

    #[test]
    fn try_deliver_local_returns_message_for_remote_agent() {
        let router = make_router();
        // AgentId(99) not registered
        let remainder = router.try_deliver_local(msg(99, false));
        assert!(remainder.is_some(), "message to unknown agent must pass through to DB");
    }

    #[test]
    fn durable_message_to_local_agent_is_not_consumed() {
        let channel = Arc::new(LocalA2AChannel::new());
        let _rx = channel.subscribe();
        let router = A2ARouter::new(Arc::clone(&channel));
        router.register_local(AgentId(3));

        // require_durable = true: router still delivers locally BUT also
        // returns Some(msg) so the caller writes to DB too.
        let m = msg(3, true);
        let remainder = router.try_deliver_local(m);
        assert!(
            remainder.is_some(),
            "durable message must be returned so caller can also write to DB"
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
cargo test -p vox-orchestrator a2a::router 2>&1 | tail -10
```

Expected: compile errors — `A2ARouter` not defined.

- [ ] **Step 3: Write the implementation**

```rust
//! Delivery-plane router for A2A messages.
//!
//! `A2ARouter` decides whether an outbound message should be delivered via the
//! local `LocalA2AChannel` (same-process, sub-millisecond) or handed to
//! `send_to_db` (cross-process, durable).
//!
//! Rule table:
//!
//! | Target is local? | `require_durable`? | Action |
//! |---|---|---|
//! | Yes | No  | Local channel only. Returns `None`. |
//! | Yes | Yes | Local channel AND DB. Returns `Some(msg)` for DB write. |
//! | No  | *   | Returns `Some(msg)` for DB write. |

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::RwLock;
use vox_orchestrator_types::{A2AMessage, AgentId};

use super::local_channel::LocalA2AChannel;

/// Routes outbound A2A messages between the local broadcast channel and VoxDB.
pub struct A2ARouter {
    channel: Arc<LocalA2AChannel>,
    /// Agent IDs known to be alive in this process.
    local_agents: RwLock<HashSet<AgentId>>,
}

impl A2ARouter {
    /// Create a new router backed by the given local channel.
    pub fn new(channel: Arc<LocalA2AChannel>) -> Self {
        Self {
            channel,
            local_agents: RwLock::new(HashSet::new()),
        }
    }

    /// Mark `agent_id` as local to this process.
    /// Call this in `Orchestrator::register_agent()`.
    pub fn register_local(&self, agent_id: AgentId) {
        self.local_agents.write().insert(agent_id);
    }

    /// Remove `agent_id` from the local registry.
    /// Call this in `Orchestrator::unregister_agent()`.
    pub fn unregister_local(&self, agent_id: &AgentId) {
        self.local_agents.write().remove(agent_id);
    }

    /// Returns true if `agent_id` is registered as a local agent.
    pub fn is_local(&self, agent_id: &AgentId) -> bool {
        self.local_agents.read().contains(agent_id)
    }

    /// Try to deliver `msg` via the local channel.
    ///
    /// Returns:
    /// - `None` — message was consumed locally; caller does NOT write to DB.
    /// - `Some(msg)` — caller MUST write `msg` to DB (either target is remote,
    ///   or the message requires durable persistence).
    ///
    /// When `msg.require_durable` is true and the target is local, the message
    /// is sent on the local channel AND returned so the caller also writes to DB.
    pub fn try_deliver_local(&self, msg: A2AMessage) -> Option<A2AMessage> {
        if !self.is_local(&msg.recipient_agent_id) {
            // Target is remote or unknown — always go to DB.
            return Some(msg);
        }

        // Target is local — send on channel.
        // Ignore send errors (lagged/no-receivers): the DB fallback handles loss.
        let _ = self.channel.send(msg.clone());

        if msg.require_durable {
            // Also write to DB for audit trail.
            Some(msg)
        } else {
            None // consumed; caller skips DB write
        }
    }

    /// A reference to the underlying local channel.
    /// Use this to `subscribe()` from agent receive loops.
    pub fn channel(&self) -> &Arc<LocalA2AChannel> {
        &self.channel
    }
}
```

- [ ] **Step 4: Register the module**

Add to `crates/vox-orchestrator/src/a2a/mod.rs`:

```rust
pub mod router;
pub use router::A2ARouter;
```

- [ ] **Step 5: Run tests**

```powershell
cargo test -p vox-orchestrator a2a::router 2>&1 | tail -10
```

Expected: `test result: ok. 7 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator/src/a2a/router.rs
git add crates/vox-orchestrator/src/a2a/mod.rs
git commit -m "feat(a2a): add A2ARouter — local-channel-first delivery with durable fallback"
```

---

## Task 4: Wire `A2ARouter` into `Orchestrator`

The router needs to be held by `Orchestrator` and updated at agent lifecycle points.

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator.rs` (or `src/orchestrator/mod.rs`)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator/tests/a2a_router_lifecycle.rs` (new file):

```rust
//! Verifies that agent registration wires into A2ARouter so local agents
//! receive messages via the broadcast channel, not DB polling.

use std::time::Duration;
use vox_orchestrator_types::{A2AMessage, A2AMessageType, AgentId, MessageId};

#[tokio::test]
async fn registered_agent_receives_via_channel() {
    // Build a minimal Orchestrator. Use the test-helper constructor if available,
    // or build with OrchestratorConfig::default() + a no-op DB.
    let orch = vox_orchestrator::bootstrap::build_test_orchestrator().await;

    let agent_id = AgentId(101);
    // Subscribe before registering so we don't miss messages
    let mut rx = orch.a2a_channel().subscribe();

    orch.register_agent_local(agent_id);

    // Send a local message via the router
    let msg = A2AMessage {
        id: MessageId(1),
        sender_agent_id: AgentId(0),
        recipient_agent_id: agent_id,
        message_type: A2AMessageType::Clarification,
        payload: "hello from router".to_string(),
        require_durable: false,
    };

    orch.send_a2a(msg).await.unwrap();

    // Message should arrive on the channel within 10ms — no DB round-trip
    let received = tokio::time::timeout(Duration::from_millis(10), rx.recv())
        .await
        .expect("message must arrive within 10ms — no DB round-trip")
        .unwrap();

    assert_eq!(received.payload, "hello from router");

    orch.unregister_agent_local(agent_id);
    assert!(!orch.router().is_local(&agent_id));
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
cargo test -p vox-orchestrator a2a_router_lifecycle 2>&1 | tail -10
```

Expected: compile errors — `build_test_orchestrator`, `a2a_channel`, `register_agent_local`, `send_a2a`, `router` not defined.

- [ ] **Step 3: Add `A2ARouter` to `Orchestrator`**

In `crates/vox-orchestrator/src/orchestrator.rs` (or `orchestrator/mod.rs`), add the router field.
Find the `Orchestrator` struct:

```rust
use std::sync::Arc;
use crate::a2a::{A2ARouter, LocalA2AChannel};

pub struct Orchestrator {
    // ... existing fields ...

    /// In-process A2A delivery router. Registered agents receive messages via
    /// the broadcast channel instead of polling VoxDB.
    pub(crate) a2a_router: Arc<A2ARouter>,
    pub(crate) a2a_channel: Arc<LocalA2AChannel>,
}
```

In `Orchestrator::new()` (or equivalent constructor), initialize:

```rust
let a2a_channel = Arc::new(LocalA2AChannel::new());
let a2a_router = Arc::new(A2ARouter::new(Arc::clone(&a2a_channel)));
// ... include in struct init ...
```

Add public accessor methods:

```rust
impl Orchestrator {
    /// Register an agent as local to this process.
    /// Must be called when an actor agent is spawned into this orchestrator.
    pub fn register_agent_local(&self, agent_id: AgentId) {
        self.a2a_router.register_local(agent_id);
    }

    /// Remove an agent from the local registry.
    /// Must be called when an actor agent shuts down or moves to another process.
    pub fn unregister_agent_local(&self, agent_id: AgentId) {
        self.a2a_router.unregister_local(&agent_id);
    }

    /// Expose the channel so agents can subscribe to receive local messages.
    pub fn a2a_channel(&self) -> &Arc<LocalA2AChannel> {
        &self.a2a_channel
    }

    /// Expose the router for inspection (e.g., tests).
    pub fn router(&self) -> &Arc<A2ARouter> {
        &self.a2a_router
    }

    /// Send an A2A message. Delivers locally if the target is a registered
    /// local agent; otherwise (or if require_durable is set) writes to VoxDB.
    pub async fn send_a2a(&self, msg: A2AMessage) -> anyhow::Result<()> {
        use crate::a2a::dispatch::send_to_db;

        match self.a2a_router.try_deliver_local(msg) {
            None => {
                // Fully consumed by local channel — no DB write needed.
                Ok(())
            }
            Some(msg_for_db) => {
                // Either remote target or durable: write to DB.
                let db = self.db.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("DB not available for A2A delivery"))?;
                send_to_db(db, &msg_for_db).await
                    .map_err(|e| anyhow::anyhow!("A2A DB write failed: {e}"))
            }
        }
    }
}
```

- [ ] **Step 4: Add `build_test_orchestrator` to the test-helpers crate**

In `crates/vox-orchestrator-test-helpers/src/lib.rs`, add:

```rust
/// Construct a minimal `Orchestrator` suitable for unit tests.
/// Uses an in-memory DB stub and no background polling tasks.
pub async fn build_test_orchestrator() -> vox_orchestrator::Orchestrator {
    use vox_orchestrator::{Orchestrator, OrchestratorConfig};
    Orchestrator::new_for_test(OrchestratorConfig::default()).await
}
```

Add `Orchestrator::new_for_test` in `orchestrator.rs`:

```rust
#[cfg(any(test, feature = "test-helpers"))]
pub async fn new_for_test(config: OrchestratorConfig) -> Self {
    let a2a_channel = Arc::new(LocalA2AChannel::new());
    let a2a_router = Arc::new(A2ARouter::new(Arc::clone(&a2a_channel)));
    Self {
        config,
        a2a_router,
        a2a_channel,
        db: None,
        // ... other fields with test defaults ...
    }
}
```

- [ ] **Step 5: Run the test**

```powershell
cargo test -p vox-orchestrator a2a_router_lifecycle 2>&1 | tail -15
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator/src/orchestrator.rs
git add crates/vox-orchestrator/tests/a2a_router_lifecycle.rs
git add crates/vox-orchestrator-test-helpers/src/lib.rs
git commit -m "feat(orchestrator): wire A2ARouter into Orchestrator — agent lifecycle + send_a2a"
```

---

## Task 5: Update agent receive loop to use channel over DB polling

Agent actors currently call `poll_inbox_from_db` in a timer loop. Now that they can receive from
the broadcast channel, add the channel as the primary receive path with DB as fallback.

**Files:**
- Modify: whichever file runs the per-agent inbox poll loop

- [ ] **Step 1: Find the existing poll loop**

```powershell
rg "poll_inbox_from_db\|clarification_db_inbox_poll" crates/vox-orchestrator/src/ --files-with-matches
```

Open the file. Find the loop that calls `poll_inbox_from_db`.

- [ ] **Step 2: Write a test for the updated loop shape**

```rust
// In the test module of the poll file, or in a new test file:
#[tokio::test]
async fn agent_receive_loop_prefers_channel_over_db() {
    use tokio::time::Duration;
    use vox_orchestrator_types::{A2AMessage, A2AMessageType, AgentId, MessageId};

    let channel = LocalA2AChannel::new();
    let mut rx = channel.subscribe();

    // Simulate an incoming local message
    let expected_msg = A2AMessage {
        id: MessageId(99),
        sender_agent_id: AgentId(1),
        recipient_agent_id: AgentId(2),
        message_type: A2AMessageType::Clarification,
        payload: "channel-delivered".to_string(),
        require_durable: false,
    };
    channel.send(expected_msg.clone()).unwrap();

    // The message should be receivable immediately without a DB round-trip
    let received = tokio::time::timeout(Duration::from_millis(5), rx.recv())
        .await
        .expect("must arrive within 5ms")
        .unwrap();

    assert_eq!(received.id.0, 99);
    assert_eq!(received.payload, "channel-delivered");
}
```

- [ ] **Step 3: Update the agent receive loop to `tokio::select!` over channel + DB**

Find the per-agent poll loop. Change it from:

```rust
// OLD: pure DB polling (every N seconds)
loop {
    tokio::time::sleep(poll_interval).await;
    let messages = poll_inbox_from_db(&db, agent_id).await?;
    for msg in messages {
        handle_message(msg).await?;
    }
}
```

To:

```rust
// NEW: channel-first, DB as fallback on timer
let mut rx = a2a_channel.subscribe();
let mut db_poll_interval = tokio::time::interval(Duration::from_secs(30));
db_poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

loop {
    tokio::select! {
        // Primary path: channel message arrives immediately
        result = rx.recv() => {
            match result {
                Ok(msg) if msg.recipient_agent_id == agent_id => {
                    handle_message(msg).await?;
                }
                Ok(_) => { /* not for us — broadcast sends to all */ }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(agent_id = agent_id.0, dropped = n,
                        "A2A channel lagged; falling through to DB for missed messages");
                    // Fall through to DB poll immediately to recover missed messages
                    let messages = poll_inbox_from_db(&db, agent_id).await?;
                    for msg in messages {
                        handle_message(msg).await?;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::error!("A2A local channel closed unexpectedly");
                    break;
                }
            }
        }
        // Fallback path: DB poll on timer for cross-process / durable messages
        _ = db_poll_interval.tick() => {
            let messages = poll_inbox_from_db(&db, agent_id).await?;
            for msg in messages {
                handle_message(msg).await?;
            }
        }
    }
}
```

- [ ] **Step 4: Run all A2A-related tests**

```powershell
cargo test -p vox-orchestrator a2a 2>&1 | tail -15
```

Expected: all pass.

- [ ] **Step 5: Run full orchestrator suite**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator/src/
git commit -m "feat(a2a): agent receive loop uses channel-first select! with DB fallback

- Local messages arrive sub-millisecond via tokio broadcast
- DB poll interval reduced to 30s (was the primary path, now backup)
- Lagged receivers recover missed messages from DB automatically"
```

---

## Task 6: Integration test — local message latency

- [ ] **Step 1: Write a latency integration test**

Add to `crates/vox-orchestrator/tests/a2a_latency.rs` (new file):

```rust
//! Validates that local A2A delivery latency is sub-millisecond.

use std::time::Instant;
use vox_orchestrator_types::{A2AMessage, A2AMessageType, AgentId, MessageId};
use vox_orchestrator::a2a::LocalA2AChannel;

#[tokio::test]
async fn local_a2a_delivery_is_sub_millisecond() {
    let channel = LocalA2AChannel::new();
    let mut rx = channel.subscribe();

    let msg = A2AMessage {
        id: MessageId(1),
        sender_agent_id: AgentId(1),
        recipient_agent_id: AgentId(2),
        message_type: A2AMessageType::Clarification,
        payload: "latency-test".to_string(),
        require_durable: false,
    };

    let start = Instant::now();
    channel.send(msg).unwrap();
    let _received = rx.recv().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 1,
        "local A2A delivery must be sub-millisecond, got {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn one_hundred_messages_under_10ms_total() {
    let channel = LocalA2AChannel::new();
    let mut rx = channel.subscribe();

    let start = Instant::now();
    for i in 0..100u64 {
        let msg = A2AMessage {
            id: MessageId(i),
            sender_agent_id: AgentId(0),
            recipient_agent_id: AgentId(1),
            message_type: A2AMessageType::Clarification,
            payload: format!("msg-{i}"),
            require_durable: false,
        };
        channel.send(msg).unwrap();
    }
    for _ in 0..100 {
        rx.recv().await.unwrap();
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "100 local A2A messages must complete in under 10ms, got {}ms",
        elapsed.as_millis()
    );
}
```

- [ ] **Step 2: Run the latency tests**

```powershell
cargo test -p vox-orchestrator a2a_latency -- --nocapture 2>&1 | tail -10
```

Expected: both pass.

- [ ] **Step 3: Final commit for Plan 3**

```powershell
git add crates/vox-orchestrator/tests/a2a_latency.rs
git commit -m "test(a2a): add latency integration tests — local delivery must be sub-ms"

git commit --allow-empty -m "feat: Plan 3 complete — A2A MessageBus-first delivery

- LocalA2AChannel: tokio broadcast, capacity 1024, primary local path
- A2ARouter: delivery-plane decision (local channel vs DB)
- Orchestrator.send_a2a(): routes through router before DB
- Agent receive loop: tokio::select! over channel + DB timer fallback
- require_durable flag: forces DB write even for local agents
- Remote (Populi mesh) path: unchanged"
```

---

## Verification Checklist

Before marking Plan 3 complete:

- [ ] `cargo test -p vox-orchestrator-types` passes (new `require_durable` field)
- [ ] `cargo test -p vox-orchestrator local_channel` passes (4 tests)
- [ ] `cargo test -p vox-orchestrator a2a::router` passes (7 tests)
- [ ] `cargo test -p vox-orchestrator a2a_router_lifecycle` passes
- [ ] `cargo test -p vox-orchestrator a2a_latency` passes
- [ ] `cargo test -p vox-orchestrator` passes (full suite, no regressions)
- [ ] `rg "poll_inbox_from_db" crates/vox-orchestrator/src/` — DB poll is only in the fallback timer branch
