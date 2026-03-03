use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Identity types
// ---------------------------------------------------------------------------

/// Unique identifier for a task within the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T-{:04}", self.0)
    }
}

/// Unique identifier for an agent within the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A-{:02}", self.0)
    }
}

/// Unique identifier mapping a question and response together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub u64);

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q-{:04}", self.0)
    }
}

/// Helper parsing error for identifiers.
#[derive(Debug, thiserror::Error)]
#[error("Invalid ID format")]
pub struct IdParseError;

impl FromStr for TaskId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("T-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(TaskId(n))
    }
}

impl FromStr for AgentId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("A-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(AgentId(n))
    }
}

impl FromStr for CorrelationId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("Q-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(CorrelationId(n))
    }
}

/// Unique identifier for a batch submission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(pub u64);

impl fmt::Display for BatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B-{:04}", self.0)
    }
}

impl FromStr for BatchId {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("B-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(BatchId(n))
    }
}

/// Handle for an active lock on a resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockToken(pub u64);

impl fmt::Display for LockToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L-{:04}", self.0)
    }
}

impl FromStr for LockToken {
    type Err = IdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = s
            .strip_prefix("L-")
            .unwrap_or(s)
            .parse()
            .map_err(|_| IdParseError)?;
        Ok(LockToken(n))
    }
}

// ---------------------------------------------------------------------------
// ID generators
// ---------------------------------------------------------------------------

/// Thread-safe counter for generating sequential TaskIds.
pub struct TaskIdGenerator(AtomicU64);

impl TaskIdGenerator {
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    pub fn next(&self) -> TaskId {
        TaskId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TaskIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe counter for generating sequential AgentIds.
pub struct AgentIdGenerator(AtomicU64);

impl AgentIdGenerator {
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    pub fn next(&self) -> AgentId {
        AgentId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for AgentIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe counter for generating sequential CorrelationIds.
pub struct CorrelationIdGenerator(AtomicU64);

impl CorrelationIdGenerator {
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    pub fn next(&self) -> CorrelationId {
        CorrelationId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for CorrelationIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Task priority & status
// ---------------------------------------------------------------------------

/// Priority level for a task. Higher priority tasks are dequeued first.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Background work — lowest priority.
    Background = 0,
    /// Normal priority — default.
    Normal = 1,
    /// Urgent — preempts normal tasks.
    Urgent = 2,
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Background => write!(f, "background"),
            Self::Normal => write!(f, "normal"),
            Self::Urgent => write!(f, "urgent"),
        }
    }
}

/// Current execution status of a task.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Waiting in the queue to be picked up.
    Queued,
    /// Currently being executed by an agent.
    InProgress,
    /// Successfully completed.
    Completed,
    /// Failed with an error reason.
    Failed(String),
    /// Blocked waiting for another task to complete.
    Blocked(TaskId),
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::InProgress => write!(f, "in-progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed(reason) => write!(f, "failed: {}", reason),
            Self::Blocked(dep) => write!(f, "blocked on {}", dep),
        }
    }
}

// ---------------------------------------------------------------------------
// File access
// ---------------------------------------------------------------------------

/// Kind of access an agent requires on a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessKind {
    /// Read-only access (multiple agents can hold simultaneously).
    Read,
    /// Exclusive write access (only one agent at a time).
    Write,
}

/// A file path paired with the access kind required for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileAffinity {
    pub path: PathBuf,
    pub access: AccessKind,
}

impl FileAffinity {
    pub fn read(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            access: AccessKind::Read,
        }
    }

    pub fn write(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            access: AccessKind::Write,
        }
    }
}

// ---------------------------------------------------------------------------
// Task categories
// ---------------------------------------------------------------------------

/// General category of a task to guide model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TaskCategory {
    Parsing,
    TypeChecking,
    Debugging,
    Research,
    Testing,
    #[default]
    CodeGen,
    Review,
}

// ---------------------------------------------------------------------------
// Agent task
// ---------------------------------------------------------------------------

/// Description of a task before it is assigned an ID and routed in the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDescriptor {
    pub description: String,
    pub priority: Option<TaskPriority>,
    pub file_manifest: Vec<FileAffinity>,
    pub depends_on: Vec<TaskId>, // dependencies on ALREADY submitted tasks
    pub temp_deps: Vec<usize>,   // indices within the same batch
}

/// A unit of work to be executed by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable description of the work.
    pub description: String,
    /// Execution priority.
    pub priority: TaskPriority,
    /// Current status.
    pub status: TaskStatus,
    /// Files this task needs to read or write.
    pub file_manifest: Vec<FileAffinity>,
    /// Tasks that must complete before this one can start.
    pub depends_on: Vec<TaskId>,
    /// Estimated complexity (1-10 scale).
    pub estimated_complexity: u8,
    /// Model preference string (if any).
    pub model_preference: Option<String>,
    /// Explicit override for the model to use.
    pub model_override: Option<String>,
    /// Task category to help select the best model.
    pub task_category: TaskCategory,
    /// Number of times this task has been re-routed due to validation failures.
    pub debug_iterations: u8,
    /// Number of retry attempts (for timeout/failure recovery).
    pub retry_count: u32,
    /// When the task was created (not serialized — reconstructed on load).
    #[serde(skip)]
    pub created_at: Option<Instant>,
}

impl AgentTask {
    /// Create a new task with the given parameters.
    pub fn new(
        id: TaskId,
        description: impl Into<String>,
        priority: TaskPriority,
        file_manifest: Vec<FileAffinity>,
    ) -> Self {
        Self {
            id,
            description: description.into(),
            priority,
            status: TaskStatus::Queued,
            file_manifest,
            depends_on: Vec::new(),
            estimated_complexity: 5,
            model_preference: None,
            model_override: None,
            task_category: TaskCategory::default(),
            debug_iterations: 0,
            retry_count: 0,
            created_at: Some(Instant::now()),
        }
    }

    /// Add a dependency on another task.
    pub fn depends_on(mut self, dep: TaskId) -> Self {
        self.depends_on.push(dep);
        self.status = TaskStatus::Blocked(dep);
        self
    }

    /// Set estimated complexity (clamped to 1-10).
    pub fn complexity(mut self, c: u8) -> Self {
        self.estimated_complexity = c.clamp(1, 10);
        self
    }

    /// Set task category.
    pub fn category(mut self, cat: TaskCategory) -> Self {
        self.task_category = cat;
        self
    }

    /// Check if all dependencies are in the given completed set.
    pub fn is_ready(&self, completed: &[TaskId]) -> bool {
        self.depends_on.iter().all(|dep| completed.contains(dep))
    }

    /// List of files this task will write to.
    pub fn write_files(&self) -> Vec<&PathBuf> {
        self.file_manifest
            .iter()
            .filter(|f| f.access == AccessKind::Write)
            .map(|f| &f.path)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Inter-agent messages
// ---------------------------------------------------------------------------

/// Messages exchanged via the shared bulletin board.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// An agent changed a file.
    FileChanged {
        path: PathBuf,
        agent: AgentId,
        summary: String,
    },
    /// An agent completed a task.
    TaskCompleted { task_id: TaskId, agent_id: AgentId },
    /// A dependency is now satisfied — unblock waiting tasks.
    DependencyReady { task_id: TaskId },
    /// Interrupt: an agent should pause or re-plan.
    Interrupt { agent_id: AgentId, reason: String },
    /// An agent was spawned in the pool.
    AgentSpawned { agent_id: AgentId, name: String },
    /// A task was assigned to an agent.
    TaskAssigned { agent_id: AgentId, task_id: TaskId },
    /// A file lock was acquired.
    LockAcquired { agent_id: AgentId, path: PathBuf },
    /// A task failed.
    TaskFailed {
        agent_id: AgentId,
        task_id: TaskId,
        error: String,
    },
    /// Phase 9: A question directed from one agent to another/user.
    Question {
        from: AgentId,
        to: AgentId,
        question: String,
        correlation_id: CorrelationId,
    },
    /// Phase 9: An answer back to the requesting agent.
    Answer {
        from: AgentId,
        to: AgentId,
        answer: String,
        correlation_id: CorrelationId,
    },
    /// Phase 9: A persistent announcement across all agents.
    Broadcast { from: AgentId, message: String },
    /// Phase 9: A context key update notification.
    ContextUpdate {
        from: AgentId,
        key: String,
        value: String,
    },
    /// A structured agent-to-agent message (Integrates A2A into Bulletin).
    A2A(A2AMessage),
}

/// Unique identifier for a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "M-{:06}", self.0)
    }
}

/// The type of A2A message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum A2AMessageType {
    /// A plan handoff from one agent to another.
    PlanHandoff,
    /// Request to claim scope over a set of files.
    ScopeRequest,
    /// Grant scope over requested files.
    ScopeGrant,
    /// Progress update on current work.
    ProgressUpdate,
    /// Request for help from another agent.
    HelpRequest,
    /// Notification that a task/objective is complete.
    CompletionNotice,
    /// Report of an error or failure.
    ErrorReport,
    /// Free-form text message.
    FreeForm,
    /// A file conflict has been detected — receiver should inspect.
    ConflictDetected,
    /// A file conflict has been resolved.
    ConflictResolved,
    /// A VCS state change event (branch, commit, merge).
    VcsEvent,
    /// Request to cancel a task the receiver is working on.
    CancelRequest,
    /// Snapshot ID exchange for before/after context sharing.
    SnapshotShare,
}

/// Priority of an A2A message, mirroring task priority but for messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessagePriority {
    /// Background info, no urgency.
    Low = 0,
    /// Default priority.
    #[default]
    Normal = 1,
    /// Time-sensitive — process before Normal messages.
    High = 2,
    /// Circuit-breaker level — must be acted on immediately.
    Critical = 3,
}

/// A unique identifier for a conversation thread between agents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadId(pub String);

impl ThreadId {
    /// Create a new random thread ID.
    pub fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        Self(format!("thread-{nonce:08x}"))
    }

    /// Create from an existing string.
    pub fn from(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// VCS context attached to an A2A message.
/// Allows agents to share the exact code state they are discussing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsContext {
    /// Snapshot ID before the operation (if applicable).
    pub snapshot_before: Option<u64>,
    /// Snapshot ID after the operation (if applicable).
    pub snapshot_after: Option<u64>,
    /// Files touched by the operation.
    pub touched_paths: Vec<PathBuf>,
    /// Logical change ID this message relates to.
    pub change_id: Option<u64>,
    /// Operation log entry ID this message relates to.
    pub op_id: Option<u64>,
    /// Content hash of the primary file being discussed.
    pub content_hash: Option<String>,
}

/// Envelope metadata for traceability and precedence (system > policy > user > peer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// Trace ID for this message chain.
    pub trace_id: Option<String>,
    /// Parent trace (e.g. handoff or task that triggered this).
    pub parent_trace_id: Option<String>,
    /// Intent/request ID for grouping related messages.
    pub intent_id: Option<String>,
    /// Hash of the goal/objective this message relates to.
    pub goal_hash: Option<String>,
    /// Sender role (e.g. "system", "user", "agent").
    pub sender_role: Option<String>,
    /// Trust level for provenance (e.g. "trusted", "untrusted").
    pub trust_level: Option<String>,
    /// Expiry timestamp in unix milliseconds (None = no expiry).
    pub expiry_ms: Option<u64>,
}

/// A structured message between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    /// Unique message ID.
    pub id: MessageId,
    /// Sender agent.
    pub sender: AgentId,
    /// Receiver (None = broadcast to all).
    pub receiver: Option<AgentId>,
    /// Message type.
    pub msg_type: A2AMessageType,
    /// Message payload (structured or free-form text).
    pub payload: String,
    /// Correlation ID for threading related messages.
    pub correlation_id: Option<String>,
    /// Whether the receiver has acknowledged this message.
    pub acknowledged: bool,
    /// Timestamp in unix milliseconds.
    pub timestamp_ms: u64,
    /// Optional envelope for traceability and conflict-aware handling.
    #[serde(default)]
    pub envelope: Option<MessageEnvelope>,
    /// Message priority — higher priority messages should be processed first.
    #[serde(default)]
    pub priority: MessagePriority,
    /// Conversation thread ID — groups related messages together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<ThreadId>,
    /// VCS context — the exact code state this message relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_context: Option<VcsContext>,
}

impl A2AMessage {
    /// Create a new message.
    pub fn new(
        id: MessageId,
        sender: AgentId,
        receiver: Option<AgentId>,
        msg_type: A2AMessageType,
        payload: impl Into<String>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id,
            sender,
            receiver,
            msg_type,
            payload: payload.into(),
            correlation_id: None,
            acknowledged: false,
            timestamp_ms,
            envelope: None,
            priority: MessagePriority::Normal,
            thread_id: None,
            vcs_context: None,
        }
    }

    /// Attach envelope metadata for traceability.
    pub fn with_envelope(mut self, envelope: MessageEnvelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    /// Set message priority.
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Assign to a conversation thread.
    pub fn in_thread(mut self, thread_id: ThreadId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Attach VCS context (snapshot IDs, touched paths, change ID).
    pub fn with_vcs_context(mut self, ctx: VcsContext) -> Self {
        self.vcs_context = Some(ctx);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_display() {
        assert_eq!(TaskId(42).to_string(), "T-0042");
    }

    #[test]
    fn agent_id_display() {
        assert_eq!(AgentId(3).to_string(), "A-03");
    }

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Urgent > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Background);
    }

    #[test]
    fn file_affinity_constructors() {
        let r = FileAffinity::read("foo.rs");
        assert_eq!(r.access, AccessKind::Read);
        let w = FileAffinity::write("bar.rs");
        assert_eq!(w.access, AccessKind::Write);
    }

    #[test]
    fn agent_task_dependency_check() {
        let task = AgentTask::new(TaskId(1), "test task", TaskPriority::Normal, vec![])
            .depends_on(TaskId(10))
            .depends_on(TaskId(20));

        assert!(!task.is_ready(&[TaskId(10)]));
        assert!(task.is_ready(&[TaskId(10), TaskId(20)]));
    }

    #[test]
    fn serialization_roundtrip() {
        let task = AgentTask::new(
            TaskId(1),
            "fix parser",
            TaskPriority::Urgent,
            vec![FileAffinity::write("src/parser.rs")],
        );
        let json = serde_json::to_string(&task).expect("serialize");
        let back: AgentTask = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, task.id);
        assert_eq!(back.priority, task.priority);
        assert_eq!(back.description, task.description);
    }

    #[test]
    fn message_serialization_roundtrip() {
        let msg = AgentMessage::FileChanged {
            path: PathBuf::from("src/lib.rs"),
            agent: AgentId(1),
            summary: "added TryCatch variant".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let back: AgentMessage = serde_json::from_str(&json).expect("deserialize");
        match back {
            AgentMessage::FileChanged {
                path,
                agent,
                summary,
            } => {
                assert_eq!(path, PathBuf::from("src/lib.rs"));
                assert_eq!(agent, AgentId(1));
                assert_eq!(summary, "added TryCatch variant");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn id_generators_are_sequential() {
        let tg = TaskIdGenerator::new();
        assert_eq!(tg.next(), TaskId(1));
        assert_eq!(tg.next(), TaskId(2));
        assert_eq!(tg.next(), TaskId(3));

        let ag = AgentIdGenerator::new();
        assert_eq!(ag.next(), AgentId(1));
        assert_eq!(ag.next(), AgentId(2));
    }
}
