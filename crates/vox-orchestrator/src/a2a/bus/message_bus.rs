use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::types::{
    A2AMessage, A2AMessageType, AgentId, MessageId, MessagePriority, ThreadId, VcsContext,
};

static GLOBAL_MESSAGE_BUS: OnceLock<Arc<MessageBus>> = OnceLock::new();

/// Message bus for A2A communication.
///
/// Provides inbox-based messaging with support for unicast,
/// broadcast, and multicast delivery.
pub struct MessageBus {
    /// Per-agent inboxes.
    pub(crate) inboxes:
        std::sync::RwLock<HashMap<AgentId, std::sync::RwLock<VecDeque<A2AMessage>>>>,
    /// Audit trail of all messages (most recent at back).
    pub(crate) audit_trail: std::sync::RwLock<Vec<A2AMessage>>,
    /// Lock-free queue for ingesting audit messages.
    pub(crate) audit_queue: crossbeam_queue::SegQueue<A2AMessage>,
    /// ID generator.
    id_gen: AtomicU64,
    /// Maximum inbox size per agent before oldest messages are dropped.
    max_inbox_size: usize,
    /// Number of messages dropped due to inbox overflow.
    dropped_messages: AtomicU64,
}

impl MessageBus {
    /// Synchronize the lock-free audit queue into the main audit trail vector.
    fn sync_audit_trail(&self) {
        if !self.audit_queue.is_empty() {
            let mut locked = crate::sync_lock::rw_write(&self.audit_trail);
            while let Some(msg) = self.audit_queue.pop() {
                locked.push(msg);
            }
        }
    }
    /// Create a new message bus.
    pub fn new(max_inbox_size: usize) -> Self {
        Self {
            inboxes: std::sync::RwLock::new(HashMap::new()),
            audit_trail: std::sync::RwLock::new(Vec::new()),
            audit_queue: crossbeam_queue::SegQueue::new(),
            id_gen: AtomicU64::new(1),
            max_inbox_size,
            dropped_messages: AtomicU64::new(0),
        }
    }

    pub(crate) fn next_id(&self) -> MessageId {
        MessageId(self.id_gen.fetch_add(1, Ordering::Relaxed))
    }

    /// Register an agent (creates their inbox).
    pub fn register_agent(&self, agent_id: AgentId) {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        if !inboxes.contains_key(&agent_id) {
            drop(inboxes);
            let mut inboxes = crate::sync_lock::rw_write(&self.inboxes);
            inboxes
                .entry(agent_id)
                .or_insert_with(|| std::sync::RwLock::new(VecDeque::new()));
        }
    }

    /// Send a message to a specific agent.
    pub fn send(
        &self,
        sender: AgentId,
        receiver: AgentId,
        msg_type: A2AMessageType,
        payload: impl Into<String>,
    ) -> MessageId {
        let id = self.next_id();
        let msg = A2AMessage::new(id, sender, Some(receiver), msg_type, payload);

        {
            let inboxes = crate::sync_lock::rw_read(&self.inboxes);
            if let Some(inbox_lock) = inboxes.get(&receiver) {
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg.clone());
            } else {
                drop(inboxes);
                let mut inboxes = crate::sync_lock::rw_write(&self.inboxes);
                let inbox_lock = inboxes
                    .entry(receiver)
                    .or_insert_with(|| std::sync::RwLock::new(VecDeque::new()));
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg.clone());
            }
        }

        self.audit_queue.push(msg);

        tracing::debug!(
            from = %sender,
            to = %receiver,
            msg_id = %id,
            "A2A message sent"
        );

        id
    }

    /// Broadcast a message to all registered agents (except sender).
    pub fn broadcast(
        &self,
        sender: AgentId,
        msg_type: A2AMessageType,
        payload: impl Into<String>,
    ) -> MessageId {
        let id = self.next_id();
        let payload = payload.into();
        let msg = A2AMessage::new(id, sender, None, msg_type, payload);

        let agents: Vec<AgentId> = {
            let inboxes = crate::sync_lock::rw_read(&self.inboxes);
            inboxes.keys().copied().collect()
        };
        for agent_id in agents {
            if agent_id != sender {
                let inboxes = crate::sync_lock::rw_read(&self.inboxes);
                if let Some(inbox_lock) = inboxes.get(&agent_id) {
                    let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                    if inbox.len() >= self.max_inbox_size {
                        inbox.pop_front();
                        self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                    }
                    inbox.push_back(msg.clone());
                }
            }
        }

        self.audit_queue.push(msg);
        id
    }

    /// Send to a group of agents.
    pub fn send_to_group(
        &self,
        sender: AgentId,
        receivers: &[AgentId],
        msg_type: A2AMessageType,
        payload: impl Into<String>,
    ) -> MessageId {
        let id = self.next_id();
        let payload = payload.into();

        for &receiver in receivers {
            let msg = A2AMessage::new(id, sender, Some(receiver), msg_type.clone(), &payload);
            let inboxes = crate::sync_lock::rw_read(&self.inboxes);
            if let Some(inbox_lock) = inboxes.get(&receiver) {
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg);
            } else {
                drop(inboxes);
                let mut inboxes = crate::sync_lock::rw_write(&self.inboxes);
                let inbox_lock = inboxes
                    .entry(receiver)
                    .or_insert_with(|| std::sync::RwLock::new(VecDeque::new()));
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg);
            }
        }

        let audit_msg = A2AMessage::new(id, sender, None, msg_type, payload);
        self.audit_queue.push(audit_msg);
        id
    }

    /// Get unacknowledged messages for an agent, sorted by priority (highest first).
    pub fn inbox(&self, agent_id: AgentId) -> Vec<A2AMessage> {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        let mut msgs: Vec<_> = inboxes
            .get(&agent_id)
            .map(|inbox_lock| {
                let inbox = crate::sync_lock::rw_read(inbox_lock);
                inbox
                    .iter()
                    .filter(|m| {
                        if m.acknowledged {
                            return false;
                        }
                        if m.is_expired() {
                            return false;
                        }
                        true
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        msgs.sort_by_key(|m| std::cmp::Reverse(m.priority));
        msgs
    }

    /// Get all messages for an agent (including acknowledged).
    pub fn inbox_all(&self, agent_id: AgentId) -> Vec<A2AMessage> {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        inboxes
            .get(&agent_id)
            .map(|inbox_lock| {
                let inbox = crate::sync_lock::rw_read(inbox_lock);
                inbox.iter().cloned().collect()
            })
            .unwrap_or_default()
    }

    /// Retrieve all messages in a specific thread, across all agents.
    pub fn messages_in_thread(&self, thread_id: &ThreadId) -> Vec<A2AMessage> {
        self.sync_audit_trail();
        let audit = crate::sync_lock::rw_read(&self.audit_trail);
        let mut msgs: Vec<_> = audit
            .iter()
            .filter(|m| m.thread_id.as_ref() == Some(thread_id))
            .cloned()
            .collect();
        msgs.sort_by_key(|m| m.timestamp_ms);
        msgs
    }

    /// Retrieve an agent's inbox filtered to a specific thread.
    pub fn inbox_for_thread(&self, agent_id: AgentId, thread_id: &ThreadId) -> Vec<A2AMessage> {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        inboxes
            .get(&agent_id)
            .map(|inbox_lock| {
                let inbox = crate::sync_lock::rw_read(inbox_lock);
                inbox
                    .iter()
                    .filter(|m| m.thread_id.as_ref() == Some(thread_id) && !m.acknowledged)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Send a VCS-context-annotated message to an agent.
    pub fn send_with_vcs_context(
        &self,
        sender: AgentId,
        receiver: AgentId,
        msg_type: A2AMessageType,
        payload: impl Into<String>,
        vcs_context: VcsContext,
        priority: MessagePriority,
        thread_id: Option<ThreadId>,
    ) -> MessageId {
        let id = self.next_id();
        let msg = A2AMessage::new(id, sender, Some(receiver), msg_type, payload)
            .with_priority(priority)
            .with_vcs_context(vcs_context);
        let msg = if let Some(tid) = thread_id {
            msg.in_thread(tid)
        } else {
            msg
        };

        {
            let inboxes = crate::sync_lock::rw_read(&self.inboxes);
            if let Some(inbox_lock) = inboxes.get(&receiver) {
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg.clone());
            } else {
                drop(inboxes);
                let mut inboxes = crate::sync_lock::rw_write(&self.inboxes);
                let inbox_lock = inboxes
                    .entry(receiver)
                    .or_insert_with(|| std::sync::RwLock::new(VecDeque::new()));
                let mut inbox = crate::sync_lock::rw_write(inbox_lock);
                if inbox.len() >= self.max_inbox_size {
                    inbox.pop_front();
                    self.dropped_messages.fetch_add(1, Ordering::Relaxed);
                }
                inbox.push_back(msg.clone());
            }
        }
        self.audit_queue.push(msg);
        id
    }

    /// Send a conflict-detected notice (Critical priority, auto-annotated).
    pub fn send_conflict_notice(
        &self,
        sender: AgentId,
        receiver: AgentId,
        path: &str,
        snapshot_before: Option<u64>,
    ) -> MessageId {
        let ctx = VcsContext {
            snapshot_before,
            snapshot_after: None,
            touched_paths: vec![path.parse().unwrap_or_default()],
            change_id: None,
            op_id: None,
            content_hash: None,
        };
        self.send_with_vcs_context(
            sender,
            receiver,
            A2AMessageType::ConflictDetected,
            format!("Conflict detected on {path}"),
            ctx,
            MessagePriority::Critical,
            None,
        )
    }

    /// Acknowledge a message in an agent's inbox.
    pub fn acknowledge(&self, agent_id: AgentId, message_id: MessageId) -> bool {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        if let Some(inbox_lock) = inboxes.get(&agent_id) {
            let mut inbox = crate::sync_lock::rw_write(inbox_lock);
            let mut found = false;
            for msg in inbox.iter_mut() {
                if msg.id == message_id {
                    msg.acknowledged = true;
                    found = true;
                    break;
                }
            }
            if found {
                return true;
            }
        }
        false
    }

    /// Get the audit trail (all messages ever sent).
    pub fn audit_trail(&self) -> Vec<A2AMessage> {
        self.sync_audit_trail();
        crate::sync_lock::rw_read(&self.audit_trail).clone()
    }

    /// Get audit trail messages since a given timestamp.
    pub fn audit_since(&self, since_ms: u64) -> Vec<A2AMessage> {
        self.sync_audit_trail();
        crate::sync_lock::rw_read(&self.audit_trail)
            .iter()
            .filter(|m| m.timestamp_ms >= since_ms)
            .cloned()
            .collect()
    }

    /// Count unacknowledged messages for an agent.
    pub fn unread_count(&self, agent_id: AgentId) -> usize {
        let inboxes = crate::sync_lock::rw_read(&self.inboxes);
        inboxes
            .get(&agent_id)
            .map(|inbox_lock| {
                let inbox = crate::sync_lock::rw_read(inbox_lock);
                inbox.iter().filter(|m| !m.acknowledged).count()
            })
            .unwrap_or(0)
    }

    /// Total messages in the audit trail.
    pub fn total_messages(&self) -> usize {
        self.sync_audit_trail();
        crate::sync_lock::rw_read(&self.audit_trail).len()
    }

    /// Total count of dropped inbox messages due to per-agent inbox capacity.
    pub fn dropped_messages(&self) -> u64 {
        self.dropped_messages.load(Ordering::Relaxed)
    }

    /// Process-global in-process bus for codegen-emitted AI fixtures.
    #[must_use]
    pub fn global() -> Arc<MessageBus> {
        GLOBAL_MESSAGE_BUS
            .get_or_init(|| Arc::new(MessageBus::new(1024)))
            .clone()
    }

    /// Record an `@subagent` routing decision on the bus for audit / downstream observers.
    pub fn record_ai_subagent_fixture_routing(&self, decision: &str, prompt_byte_len: usize) {
        let sender = AgentId(9101);
        let receiver = AgentId(9102);
        self.register_agent(sender);
        self.register_agent(receiver);
        let _ = self.send(
            sender,
            receiver,
            A2AMessageType::PlanHandoff,
            format!("ai_fixture_subagent decision={decision} prompt_bytes={prompt_byte_len}"),
        );
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod semcov_wave10_tests {
    use super::*;
    use crate::types::{A2AMessageType, AgentId, MessageId, ThreadId};

    fn sender() -> AgentId {
        AgentId(1)
    }
    fn recv_a() -> AgentId {
        AgentId(2)
    }
    fn recv_b() -> AgentId {
        AgentId(3)
    }

    // ── MessageBus::new / Default ────────────────────────────────────────────

    #[test]
    fn new_starts_with_empty_inboxes_and_zero_dropped() {
        // Catches: a bus that leaks state from a previous test or static init
        let bus = MessageBus::new(10);
        assert_eq!(bus.dropped_messages(), 0, "dropped must be 0 on fresh bus");
        assert_eq!(bus.total_messages(), 0, "audit trail must be empty on fresh bus");
    }

    #[test]
    fn default_uses_100_inbox_cap_not_zero() {
        // Catches: Default::default() silently setting max_inbox_size=0 would drop every message
        let bus = MessageBus::default();
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        let _id = bus.send(s, r, A2AMessageType::FreeForm, "ping");
        // If cap were 0, the message would be dropped and inbox would be empty
        let msgs = bus.inbox_all(r);
        assert!(!msgs.is_empty(), "default bus must accept at least one message");
    }

    // ── register_agent ───────────────────────────────────────────────────────

    #[test]
    fn register_same_agent_twice_does_not_clear_inbox() {
        // Catches: second register_agent call replacing (and wiping) the existing inbox
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.send(s, r, A2AMessageType::FreeForm, "first");
        bus.register_agent(r); // double-register
        let msgs = bus.inbox_all(r);
        assert_eq!(msgs.len(), 1, "second register_agent must not wipe existing inbox");
    }

    #[test]
    fn register_agent_inbox_starts_empty() {
        // Catches: inbox pre-populated from a shared pool or leaked state
        let bus = MessageBus::new(10);
        bus.register_agent(recv_a());
        let msgs = bus.inbox(recv_a());
        assert!(msgs.is_empty(), "newly registered agent must have empty inbox");
    }

    // ── broadcast ────────────────────────────────────────────────────────────

    #[test]
    fn broadcast_with_no_agents_does_not_panic() {
        // Catches: unwrap() or index into empty agents list inside broadcast
        let bus = MessageBus::new(10);
        let _id = bus.broadcast(sender(), A2AMessageType::FreeForm, "hello");
        assert_eq!(bus.total_messages(), 1, "broadcast still lands in audit trail");
    }

    #[test]
    fn broadcast_single_recipient_receives_message() {
        // Catches: off-by-one that skips delivery when only one non-sender agent exists
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.broadcast(s, A2AMessageType::FreeForm, "hello");
        let msgs = bus.inbox(r);
        assert_eq!(msgs.len(), 1, "single non-sender agent must receive the broadcast");
    }

    #[test]
    fn broadcast_sender_does_not_receive_own_message() {
        // Catches: broadcast loop accidentally including the sender's own inbox
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.broadcast(s, A2AMessageType::FreeForm, "hi");
        let sender_inbox = bus.inbox(s);
        assert!(sender_inbox.is_empty(), "sender must not receive their own broadcast");
    }

    #[test]
    fn broadcast_all_n_recipients_receive_exactly_once() {
        // Catches: duplicate delivery or partial fan-out to only first N-1 agents
        let bus = MessageBus::new(50);
        let s = sender();
        bus.register_agent(s);
        let recipients: Vec<AgentId> = (10u64..15).map(AgentId).collect();
        for &r in &recipients {
            bus.register_agent(r);
        }
        bus.broadcast(s, A2AMessageType::FreeForm, "broadcast");
        for r in recipients {
            let count = bus.inbox(r).len();
            assert_eq!(count, 1, "agent {r:?} must receive exactly one broadcast");
        }
    }

    // ── send_to_group ────────────────────────────────────────────────────────

    #[test]
    fn send_to_group_empty_slice_no_panic_and_audit_entry() {
        // Catches: panic on empty receivers slice or missing audit entry
        let bus = MessageBus::new(10);
        bus.register_agent(sender());
        let _id = bus.send_to_group(sender(), &[], A2AMessageType::FreeForm, "nobody");
        assert_eq!(bus.total_messages(), 1, "audit must record even a zero-recipient multicast");
    }

    #[test]
    fn send_to_group_unknown_agent_auto_creates_inbox() {
        // Catches: silently dropping messages to unregistered agents vs creating their inbox
        let bus = MessageBus::new(10);
        bus.register_agent(sender());
        let unknown = AgentId(999);
        bus.send_to_group(sender(), &[unknown], A2AMessageType::FreeForm, "hey");
        let msgs = bus.inbox_all(unknown);
        assert_eq!(msgs.len(), 1, "send_to_group must auto-create inbox for unknown agents");
    }

    #[test]
    fn send_to_group_same_message_id_for_all_recipients() {
        // Catches: id_gen advanced per recipient, making N messages appear as N separate sends
        let bus = MessageBus::new(10);
        let s = sender();
        let a = recv_a();
        let b = recv_b();
        bus.register_agent(s);
        bus.register_agent(a);
        bus.register_agent(b);
        let id = bus.send_to_group(s, &[a, b], A2AMessageType::FreeForm, "group");
        let msg_a = &bus.inbox_all(a)[0];
        let msg_b = &bus.inbox_all(b)[0];
        assert_eq!(msg_a.id, id, "recipient A must see the returned message id");
        assert_eq!(msg_b.id, id, "recipient B must see the same message id as A");
    }

    // ── inbox ────────────────────────────────────────────────────────────────

    #[test]
    fn inbox_unregistered_agent_returns_empty_not_panic() {
        // Catches: unwrap on a missing inbox entry
        let bus = MessageBus::new(10);
        let msgs = bus.inbox(AgentId(404));
        assert!(msgs.is_empty(), "inbox for unregistered agent must be empty");
    }

    #[test]
    fn inbox_excludes_acknowledged_messages() {
        // Catches: acknowledge() marking the flag but inbox() not filtering on it
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        let id = bus.send(s, r, A2AMessageType::FreeForm, "ack-me");
        let acked = bus.acknowledge(r, id);
        assert!(acked, "acknowledge must succeed for a real message");
        let visible = bus.inbox(r);
        assert!(
            visible.is_empty(),
            "inbox() must hide acknowledged messages; got {} messages",
            visible.len()
        );
    }

    #[test]
    fn inbox_sorts_higher_priority_first() {
        // Catches: sort direction inversion — Low appearing before Critical
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        // Send low-priority first so insertion order != expected sort order
        let low = {
            let id = bus.next_id();
            let mut m = A2AMessage::new(id, s, Some(r), A2AMessageType::FreeForm, "low");
            m.priority = crate::types::MessagePriority::Low;
            m
        };
        let high = {
            let id = bus.next_id();
            let mut m = A2AMessage::new(id, s, Some(r), A2AMessageType::FreeForm, "high");
            m.priority = crate::types::MessagePriority::Critical;
            m
        };
        {
            let inboxes = crate::sync_lock::rw_read(&bus.inboxes);
            let inbox_lock = inboxes.get(&r).unwrap();
            let mut inbox = crate::sync_lock::rw_write(inbox_lock);
            inbox.push_back(low);
            inbox.push_back(high);
        }
        let msgs = bus.inbox(r);
        assert_eq!(msgs.len(), 2);
        assert_eq!(
            msgs[0].priority,
            crate::types::MessagePriority::Critical,
            "Critical must sort before Low"
        );
    }

    // ── messages_in_thread ───────────────────────────────────────────────────

    #[test]
    fn messages_in_thread_fifo_ordering() {
        // Catches: thread messages returned in reverse or insertion order instead of timestamp ASC
        let bus = MessageBus::new(50);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        let tid = ThreadId("t-fifo".to_string());

        // Send three messages; use send_with_vcs_context to attach thread_id
        use crate::types::{MessagePriority, VcsContext};
        let ctx = || VcsContext {
            snapshot_before: None,
            snapshot_after: None,
            touched_paths: vec![],
            change_id: None,
            op_id: None,
            content_hash: None,
        };
        bus.send_with_vcs_context(
            s,
            r,
            A2AMessageType::FreeForm,
            "first",
            ctx(),
            MessagePriority::Normal,
            Some(tid.clone()),
        );
        bus.send_with_vcs_context(
            s,
            r,
            A2AMessageType::FreeForm,
            "second",
            ctx(),
            MessagePriority::Normal,
            Some(tid.clone()),
        );
        bus.send_with_vcs_context(
            s,
            r,
            A2AMessageType::FreeForm,
            "third",
            ctx(),
            MessagePriority::Normal,
            Some(tid.clone()),
        );

        let thread_msgs = bus.messages_in_thread(&tid);
        assert_eq!(thread_msgs.len(), 3, "all three thread messages must be returned");
        // Timestamps must be non-decreasing (FIFO)
        for window in thread_msgs.windows(2) {
            assert!(
                window[0].timestamp_ms <= window[1].timestamp_ms,
                "messages_in_thread must be sorted FIFO by timestamp_ms"
            );
        }
        assert_eq!(thread_msgs[0].payload, "first", "first sent = first in FIFO order");
        assert_eq!(thread_msgs[2].payload, "third", "last sent = last in FIFO order");
    }

    #[test]
    fn messages_in_thread_filters_other_threads() {
        // Catches: returning ALL audit messages when filtering by thread_id
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        // A message with no thread_id
        bus.send(s, r, A2AMessageType::FreeForm, "no-thread");
        let tid = ThreadId("only-me".to_string());
        let msgs = bus.messages_in_thread(&tid);
        assert!(
            msgs.is_empty(),
            "messages_in_thread must not return messages from other threads"
        );
    }

    // ── audit_since ──────────────────────────────────────────────────────────

    #[test]
    fn audit_since_zero_returns_all_messages() {
        // Catches: since_ms=0 treated as "no filter" but implemented as "> 0" missing msg at ms=0
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.send(s, r, A2AMessageType::FreeForm, "a");
        bus.send(s, r, A2AMessageType::FreeForm, "b");
        let all = bus.audit_since(0);
        assert_eq!(all.len(), 2, "audit_since(0) must return all messages");
    }

    #[test]
    fn audit_since_u64_max_returns_empty() {
        // Catches: overflow when comparing timestamp_ms >= u64::MAX wraps to 0
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.send(s, r, A2AMessageType::FreeForm, "past");
        let none = bus.audit_since(u64::MAX);
        assert!(
            none.is_empty(),
            "audit_since(u64::MAX) must return empty; got {} messages",
            none.len()
        );
    }

    #[test]
    fn audit_since_is_inclusive_on_boundary() {
        // Catches: `>` instead of `>=` in the filter, missing messages at exactly the cutoff
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.send(s, r, A2AMessageType::FreeForm, "msg");
        bus.sync_audit_trail();
        let trail = bus.audit_trail();
        assert!(!trail.is_empty());
        let ts = trail[0].timestamp_ms;
        let at_boundary = bus.audit_since(ts);
        assert!(
            !at_boundary.is_empty(),
            "audit_since(exact timestamp) must be inclusive"
        );
    }

    // ── dropped_messages ─────────────────────────────────────────────────────

    #[test]
    fn dropped_messages_zero_initially() {
        // Catches: dropped counter initialized to garbage or leaking from a shared global
        let bus = MessageBus::new(5);
        assert_eq!(bus.dropped_messages(), 0, "fresh bus must have 0 dropped messages");
    }

    #[test]
    fn dropped_messages_increments_after_inbox_overflow() {
        // Catches: drop counter not incremented when oldest message is evicted on overflow
        let bus = MessageBus::new(2);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.send(s, r, A2AMessageType::FreeForm, "1");
        bus.send(s, r, A2AMessageType::FreeForm, "2");
        assert_eq!(bus.dropped_messages(), 0, "at capacity but not yet over");
        bus.send(s, r, A2AMessageType::FreeForm, "3"); // triggers eviction
        assert_eq!(
            bus.dropped_messages(),
            1,
            "one message must be counted as dropped after overflow"
        );
    }

    #[test]
    fn dropped_messages_monotonically_non_decreasing() {
        // Catches: counter reset or decrement on acknowledge/drain
        let bus = MessageBus::new(1);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        let id1 = bus.send(s, r, A2AMessageType::FreeForm, "first");
        let before = bus.dropped_messages();
        bus.send(s, r, A2AMessageType::FreeForm, "second"); // evicts first
        let after = bus.dropped_messages();
        assert!(after >= before, "dropped_messages must never decrease; {before} -> {after}");
        // Acknowledge evicted message id (already gone) — counter must not go down
        bus.acknowledge(r, id1);
        assert_eq!(
            bus.dropped_messages(),
            after,
            "acknowledge must not alter dropped counter"
        );
    }

    // ── acknowledge ──────────────────────────────────────────────────────────

    #[test]
    fn acknowledge_returns_false_for_unknown_message() {
        // Catches: returning true for any ack call regardless of whether id exists
        let bus = MessageBus::new(10);
        bus.register_agent(recv_a());
        let phantom = MessageId(u64::MAX);
        let result = bus.acknowledge(recv_a(), phantom);
        assert!(!result, "acknowledge must return false for a message id that doesn't exist");
    }

    #[test]
    fn acknowledge_idempotent_second_call_returns_true_but_inbox_still_filters() {
        // Catches: double-ack flipping acknowledged back to false (toggle bug)
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        bus.register_agent(s);
        bus.register_agent(r);
        let id = bus.send(s, r, A2AMessageType::FreeForm, "ack-twice");
        let first = bus.acknowledge(r, id);
        let second = bus.acknowledge(r, id);
        assert!(first, "first ack must succeed");
        assert!(second, "second ack on same message must also return true (idempotent)");
        // After double-ack the message must still be filtered from inbox()
        let visible = bus.inbox(r);
        assert!(
            visible.is_empty(),
            "double-ack must not un-acknowledge the message; inbox must still be empty"
        );
    }

    #[test]
    fn acknowledge_wrong_agent_does_not_ack() {
        // Catches: ack matching by message_id globally instead of per-agent
        let bus = MessageBus::new(10);
        let s = sender();
        let r = recv_a();
        let bystander = recv_b();
        bus.register_agent(s);
        bus.register_agent(r);
        bus.register_agent(bystander);
        let id = bus.send(s, r, A2AMessageType::FreeForm, "private");
        // Bystander tries to ack a message in someone else's inbox
        let result = bus.acknowledge(bystander, id);
        assert!(!result, "ack by wrong agent must return false");
        // Original recipient's message must still be unacknowledged
        let visible = bus.inbox(r);
        assert_eq!(visible.len(), 1, "original recipient must still have the unacked message");
    }

    // ── id_gen monotonicity ───────────────────────────────────────────────────

    #[test]
    fn next_id_is_strictly_monotonically_increasing() {
        // Catches: id_gen reset or non-atomic increment racing to produce duplicate ids
        let bus = MessageBus::new(100);
        let ids: Vec<MessageId> = (0..16).map(|_| bus.next_id()).collect();
        for window in ids.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "next_id must be strictly increasing; {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }
}
