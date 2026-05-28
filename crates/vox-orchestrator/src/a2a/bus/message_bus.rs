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
