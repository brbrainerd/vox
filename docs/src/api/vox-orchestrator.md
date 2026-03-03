# Crate API: vox-orchestrator

## Overview

Multi-agent file-affinity queue system. Routes tasks to AI agents based on file ownership, preventing race conditions when multiple agents work concurrently.

## Architecture

```
User Request
    │
    ▼
Orchestrator ──► FileAffinityMap ──► route to Agent
    │                                    │
    ▼                                    ▼
BulletinBoard ◄──── AgentQueue ──► FileLockManager
```

## Key Modules

| Module | Purpose |
|--------|---------|
| `orchestrator.rs` | Core orchestrator — task routing and lifecycle |
| `affinity.rs` | File-to-agent affinity mapping |
| `queue.rs` | Per-agent task queue with priority ordering |
| `locks.rs` | File-level lock manager (one writer per file) |
| `bulletin.rs` | Bulletin board for inter-agent coordination |
| `groups.rs` | Agent grouping and capability matching |
| `state.rs` | Orchestrator state persistence |
| `types.rs` | `AgentId`, `TaskId`, `TaskStatus`, etc. |
| `config.rs` | `OrchestratorConfig` |

## Feature Flags

| Feature | Description |
|---------|-------------|
| `runtime` | Actor-based agents via `vox-runtime` |
| `toestub-gate` | Post-task quality validation via TOESTUB |
| `lsp` | LSP integration for file ownership info |

## CLI

```bash
vox orchestrator enqueue --file src/main.vox --task "fix bug"
vox orchestrator status
```

---

## Module: `vox-orchestrator\src\a2a.rs`

Agent-to-Agent (A2A) structured messaging.

Enables typed message exchange between agents with inbox/outbox
support, routing (unicast, broadcast, multicast), and an audit trail.


### `struct MessageBus`

Message bus for A2A communication.

Provides inbox-based messaging with support for unicast,
broadcast, and multicast delivery.


### `struct FileAffinityMap`

Thread-safe map tracking which agent "owns" each file path.

The single-writer principle: at most one agent holds write affinity
for any given file. This prevents race conditions and lost updates.


### `struct AgentBudgetAllocation`

Per-agent budget allocation cap.


### `struct ContextBudget`

Configuration for an agent's context budget.


### `struct BudgetManager`

Tracks agent context budgets globally.


### `struct BulletinBoard`

Cross-agent communication channel using broadcast pub/sub.

Agents publish messages (file changes, task completions, interrupts)
and all other agents receive them. This follows the same pattern as
`vox-runtime::SubscriptionManager` but for orchestrator-level events.


## Module: `vox-orchestrator\src\compaction.rs`

Context compaction engine for Vox agents.

Prevents context window overflow by summarizing old conversation turns.
Adopts OpenClaw's compaction strategies:
- **Context window guard** — hard-stops if available tokens < minimum
- **Turn-based trimming** — trims whole turns, never mid-message
- **Head/tail preservation** — keeps first N + last M tokens of the context
- **Pre-compaction hook** — fires before summarization so memory can flush
- **Three strategies**: `Aggressive`, `Balanced`, `Conservative`


### `enum CompactionStrategy`

Strategy that controls how aggressively stale context is trimmed.


### `struct CompactionConfig`

Configuration for the compaction engine.


### `struct Turn`

A single conversation turn (user message or assistant response).


### `struct CompactionResult`

Outcome of a compaction pass.


### `enum CompactionError`

Errors from the compaction engine.


### `struct CompactionEngine`

Manages context window usage and trims conversation history when required.


### `enum OverflowStrategy`

Strategy for handling queue overflow when max tasks is reached.


### `enum CostPreference`

Preference for balancing model quality vs operational cost.


### `enum ScalingProfile`

User-governable scaling profile: when to scale up and how aggressively to scale down.


### `struct OrchestratorConfig`

Configuration for the orchestrator system.

Can be loaded from the `[orchestrator]` section in `Vox.toml`,
overridden by `VOX_ORCHESTRATOR_*` environment variables,
or constructed programmatically.


### `enum ConfigValidationError`

A validation error encountered when checking an orchestrator configuration.


### `enum ConfigError`

Errors that can occur loading orchestrator configuration.


## Module: `vox-orchestrator\src\conflicts.rs`

First-class conflict resolution — inspired by Jujutsu's `Merge<T>` model.

Instead of hard-blocking when two agents touch the same file, we record
both sides as a conflict and let resolution happen later (or automatically).
Conflicts live as first-class objects, not file-level markers.


### `struct ConflictId`

Unique conflict identifier.


### `struct ConflictIdGenerator`

Thread-safe generator for [`ConflictId`]s.


### `struct ConflictSide`

One side of a conflict — an agent's version of a file.


### `enum ConflictResolution`

How a conflict was (or should be) resolved.


### `struct FileConflict`

A file conflict between two or more agents.


### `struct ConflictManager`

Tracks and manages file-level conflicts between agents.


### `struct ContextEntry`

An entry in the shared context store.


### `struct ContextStore`

In-memory store for sharing context between agents.
State is designed to be serialized alongside the Orchestrator via state.rs.


## Module: `vox-orchestrator\src\continuation.rs`

Auto-continuation engine for idle agents.

Detects idle agents with pending work and generates continuation
prompts. Supports configurable strategies and per-agent cooldowns
to prevent spam-continuing.


### `enum ContinuationStrategy`

Strategy for auto-continuation.


### `struct ContinuationPrompt`

A continuation prompt generated for an idle agent.


### `struct ContinuationEngine`

Auto-continuation engine.

Watches for idle agents via the heartbeat monitor and generates
continuation prompts when appropriate.


## Module: `vox-orchestrator\src\events.rs`

Real-time event bus for agent activity broadcasting.

Publishes structured `AgentEvent`s over a tokio broadcast channel.
Consumers (dashboard SSE, monitors, gamify hooks) subscribe and receive
events as they happen — no polling, no JSONL heuristics.


### `struct EventId`

Monotonically increasing event ID.


### `enum AgentActivity`

What an agent is currently doing.


### `struct AgentEvent`

A structured event emitted by the orchestrator.

Each event carries a unique ID, timestamp, and typed payload.
This replaces Pixel Agents' heuristic-based JSONL parsing with
deterministic, structured events.


### `enum AgentEventKind`

The different kinds of events the orchestrator can emit.


### `struct EventBus`

Thread-safe event bus for broadcasting agent events.

Uses a tokio broadcast channel under the hood. Multiple consumers
(dashboard, monitor, gamify hooks) can subscribe independently.


### `struct AffinityGroup`

A named group of files that should be handled by the same agent.

Default groups correspond to Vox crate boundaries.


### `struct AffinityGroupRegistry`

Registry of all affinity groups with compiled glob matchers.


### `fn groups_from_workspace_members`

Load affinity groups from VoxWorkspace members.

Each workspace member becomes its own affinity group with a glob
pattern matching all files under its directory.


### `fn auto_assign_groups`

Dynamic auto-assign of a workspace mapping directly reading `Vox.toml` and creating groups per directory


### `fn load_from_config`

Try to load Affinity Groups directly from a `Vox.toml` or similar file format.


## Module: `vox-orchestrator\src\handoff.rs`

Plan/context handoff protocol between agents.

Enables one agent to serialise its current state (plan, completed tasks,
context summary) into a portable document that another agent can load
and resume from. This is critical for scaling beyond a single long-lived
agent session.


### `struct ExecutionStep`

A single step in the execution history preserved during handoff.


### `struct HandoffPayload`

A portable handoff document containing everything a receiving agent
needs to resume the sender's work.


### `fn execute_handoff`

Execute a handoff: emit the event and return the payload for the receiver.


## Module: `vox-orchestrator\src\heartbeat.rs`

Agent heartbeat monitor with auto-stale detection and graduated response.


### `enum StalenessLevel`

Graduated staleness levels.


### `struct HeartbeatPolicy`

Policy for graduated heartbeat response.


### `struct AgentHeartbeat`

Per-agent heartbeat state.


### `struct HeartbeatMonitor`

Tracks agent liveness and detects stale agents with graduated response.


## Module: `vox-orchestrator\src\lib.rs`

# vox-orchestrator

Multi-agent file-affinity queue system for the Vox programming language.

Routes tasks to agents based on **file ownership** — ensuring only one agent
writes to any given file at a time. Prevents race conditions and lost updates
when multiple AI agents work concurrently across a Vox workspace.

## Architecture

```text
User Request
│
▼
Orchestrator ──► FileAffinityMap ──► route to Agent
│                                    │
▼                                    ▼
BulletinBoard ◄──── AgentQueue ──► FileLockManager
```

## Features

- `runtime` — Actor-based agents using `vox-runtime` Scheduler/Supervisor
- `toestub-gate` — Post-task quality validation using TOESTUB (on by default)
- `lsp` — LSP diagnostic integration for file ownership info


### `enum LockKind`

Kind of lock an agent holds on a file.


### `struct FileLock`

A file lock held by an agent.


### `enum LockConflict`

Error returned when a lock cannot be acquired.


### `struct FileLockManager`

Thread-safe file-level lock manager.

Enforces the single-writer principle: at most one agent can hold an
exclusive lock on any file, while multiple agents can hold shared read locks.


### `struct OrchestratorDiagnosticProvider`

Exposes orchestrator file-locking and ownership status as LSP diagnostics.


## Module: `vox-orchestrator\src\memory.rs`

Persistent memory system for Vox agents.

Inspired by OpenClaw's file-first memory model:
- **Daily logs** (`memory/YYYY-MM-DD.md`) — append-only per-session notes
- **MEMORY.md** — curated long-term knowledge indexed by heading
- **MemoryManager** — coordinates daily logs + MEMORY.md + VoxDb embeddings,
bootstraps agent context on startup, and flushes critical state before
compaction to prevent knowledge loss.


### `struct MemoryConfig`

Configuration for the persistent memory system.


### `struct DailyLog`

An append-only daily log file (`memory/YYYY-MM-DD.md`).

Each call to [`append`] writes a timestamped bullet to disk immediately.
Survives restarts — the file is opened in append mode every time.


### `struct LongTermMemory`

Manages `MEMORY.md` — curated, human-editable long-term knowledge.

Sections are Markdown headings (`## key`). Each section contains free-form
text. [`get`] extracts the body under a heading; [`set`] upserts it.


### `struct MemoryFact`

A quick in-memory cache of a recently stored fact.


### `struct MemoryManager`

Central coordinator for the Vox persistent memory system.

On creation, call [`bootstrap_context`] to load today's + yesterday's
daily logs and the contents of MEMORY.md into a ready-to-inject string.

Before compaction, call [`flush_before_compaction`] with any critical
key-value pairs to persist them durably.

When a `VoxDb` is attached via [`with_db`], every `persist_fact` also
writes to the `agent_memory` table, and `recall` falls back to the DB
when the file-based lookup misses. Files are the hot cache; VoxDB is
the durable single source of truth.


### `struct SearchHit`

A matching line found by [`MemoryManager::search`].


### `enum MemoryError`

Errors from the memory subsystem.


### `struct HybridSearchHit`

Matches from the search engine.


### `struct MemorySearchEngine`

Search engine combining local file BM25 and DB vector search.


### `struct ModelSpec`

Specification for an LLM model in the registry.


### `struct ModelConfig`

Configuration wrapper for models.


### `struct ModelRegistry`

A registry managing available agent models and model routing.


### `struct AiMonitor`

AI Monitor for idle detection and continuation prompts.


## Module: `vox-orchestrator\src\oplog.rs`

Operation log — inspired by Jujutsu's operation log and `UnpublishedOperation` model.

Records every agent action as an immutable entry with before/after snapshots,
enabling universal undo/redo.  This is the safety net that lets agents
experiment fearlessly.


### `struct OperationId`

Unique operation identifier.


### `struct OperationIdGenerator`

Thread-safe generator for [`OperationId`]s.


### `enum OperationKind`

The kind of operation that was performed.


### `struct OperationEntry`

A single entry in the operation log.


### `struct OpLog`

Append-only operation log with undo/redo support.


### `enum OrchestratorError`

Error type for orchestrator operations.


### `struct TaskTraceStep`

One step in a task's lifecycle timeline (ingress → route → verification → outcome).


### `struct OrchestratorStatus`

Snapshot of the orchestrator state for display.


### `struct AgentSummary`

Summary info for one agent.


### `struct Orchestrator`

The central coordinator for the multi-agent file-affinity queue system.


### `struct AgentQueue`

Per-agent priority task queue.

Tasks are stored in priority order (Urgent > Normal > Background).
Within the same priority level, tasks are FIFO.


## Module: `vox-orchestrator\src\rebalance.rs`

Cost-aware rebalancing and dynamic scaling for orchestrator agents.


### `enum RebalanceStrategy`

Strategies for rebalancing work across agents.


### `enum ScalingAction`

Decisions for dynamic scaling.


### `struct LoadBalancer`

Logic for cost-aware rebalancing and dynamic scaling.


### `enum AgentCommand`

Message type sent to the ActorAgent to trigger task processing.


### `struct StubTaskProcessor`

A default stub processor that immediately completes tasks.


### `struct ActorAgent`

Actor process wrapping an `AgentQueue`.

Converts a reactive orchestrator queue into an active background worker
using `vox-runtime` actor primitives.


### `struct AgentFleet`

A fleet supervisor that manages multiple agent processes.


## Module: `vox-orchestrator\src\scope.rs`

Scope guard — prevents agents from editing outside their assigned files.

Uses the `FileAffinityMap` to validate that an agent only touches
files it has been assigned to. Emits `ScopeViolation` events when
an agent attempts to write outside its scope.


### `enum ScopeEnforcement`

How strictly to enforce scope boundaries.


### `enum ScopeCheckResult`

Result of a scope check.


### `struct ScopeGuard`

Manages file scope assignments for agents.

Each agent is assigned a set of file paths or glob patterns
that define its scope. Operations outside this scope are
either blocked or warned depending on the enforcement level.


## Module: `vox-orchestrator\src\security.rs`

Security model for the Vox agent system.

Provides:
- `SecurityPolicy` — per-agent permission rules
- `SecurityGuard` — validates requests against a policy
- `AuditLog` — append-only security event log
- Rate limiting primitives


### `enum SecurityAction`

Actions that can be permitted or denied.


### `struct PolicyRule`

A policy rule — either allow or deny an action.


### `struct SecurityPolicy`

Security policy for an agent or skill.


### `struct SecurityGuard`

Validates requests against policies and rate limits.


### `struct AuditEntry`

An audit log entry.


### `struct AuditLog`

In-memory append-only audit log (ring buffer, bounded to `capacity` entries).


## Module: `vox-orchestrator\src\services\gateway.rs`

Message gateway: unified fan-out to bulletin, A2A bus, and event bus.

Provides a single API to publish notifications so that dashboard,
monitors, and other agents see consistent updates.


### `struct MessageGateway`

Unified message gateway for orchestrator notifications.

Use the associated functions to publish to bulletin, A2A, and event bus
in one place so all consumers stay in sync.


## Module: `vox-orchestrator\src\services\mod.rs`

Orchestrator service layer: routing, scaling, messaging gateway, and policy.

These modules provide separation of concerns while the main `Orchestrator`
retains API-compatible wrappers that delegate to these services.

## Service boundaries

- **RoutingService** ([`routing`]): File-affinity and group-based task routing.
Inputs: file manifest, affinity map, group registry, agent queues, config.
Output: `RouteResult::Existing(AgentId)` or `RouteResult::SpawnAgent(name)`.
Orchestrator calls `resolve_route()` which uses this and performs spawn when needed.

- **ScalingService** ([`scaling`]): Scale-up/down decisions from load and policy.
Inputs: status, config, load history, idle dynamic agents (id, last_active).
Output: `ScalingAction::NoOp | ScaleUp { name } | ScaleDown { agent_ids }`.
Runtime/orchestrator applies the action (spawn_dynamic_agent / retire_agent).

- **MessageGateway** ([`gateway`]): Unified fan-out to bulletin, A2A bus, event bus.
Functions take mutable refs to the buses and publish task completed/failed,
agent spawned/retired so dashboard and monitors stay in sync.

- **PolicyEngine** ([`policy`]): Pre-queue validation (locks and optional scope).
Inputs: lock manager, optional scope guard, event bus, manifest, agent id.
Output: `PolicyCheckResult::Allowed | LockConflict(...) | ScopeDenied(...)`.
Call before enqueueing to fail fast and emit scope violation events.


## Module: `vox-orchestrator\src\services\policy.rs`

Policy engine: scope and lock checks before queueing tasks.

Validates that an agent can acquire required locks and (optionally)
that writes fall within the agent's scope. Call before enqueueing
to fail fast and emit scope violations.


### `enum PolicyCheckResult`

Result of a policy check before queueing a task.


### `struct PolicyEngine`

Stateless policy engine for pre-queue validation.


## Module: `vox-orchestrator\src\services\routing.rs`

Routing service: file-affinity and group-based task routing.

Decides which agent (existing or to be spawned) should receive a task
based on file manifest, affinity map, affinity groups, and load.


### `enum RouteResult`

Result of a routing decision: either use an existing agent or spawn one.


### `struct RoutingService`

Stateless routing service implementing file-affinity and group voting.


## Module: `vox-orchestrator\src\services\scaling.rs`

Scaling service: scale-up and scale-down decisions based on load and policy.

Produces scaling actions (spawn dynamic agents, retire idle ones) that
the orchestrator applies. Scale-down is guarded so agents with critical
work are not retired.


### `enum ScalingAction`

Action recommended by the scaling service.


### `struct ScalingService`

Stateless scaling service.


## Module: `vox-orchestrator\src\session.rs`

Session lifecycle management for Vox agents.

Inspired by OpenClaw's session model:
- Sessions are persisted as append-only JSONL files
- Each session has its own context, permissions, and state
- Supports reset, cleanup, idle timeout, and daily reset policies
- Sessions survive restarts via replay from JSONL


### `enum SessionState`

Lifecycle state of a session.


### `enum SessionEvent`

A JSONL event appended to the session file.


### `struct SessionTurn`

A single conversation turn stored in session history.


### `struct Session`

In-memory representation of a live session.


### `struct SessionConfig`

Configuration for the session manager.


### `enum SessionError`

Errors from session management.


### `struct SessionManager`

Manages agent sessions: creation, persistence, lifecycle, cleanup.

When a `VoxDb` is attached via [`with_db`], every session creation and
turn addition also writes to the `user_sessions` and `session_turns`
tables. JSONL files remain the hot cache; VoxDB is the durable SSOT.


## Module: `vox-orchestrator\src\snapshot.rs`

Auto-snapshot working state — inspired by Jujutsu's "working copy is a commit" model.

Every agent action is bracketed by automatic snapshots so the orchestrator
always knows the before/after state of every file.  This eliminates the need
for agents to manually `git add`/`git commit`.


### `struct SnapshotId`

Unique snapshot identifier.


### `struct SnapshotIdGenerator`

Thread-safe generator for [`SnapshotId`]s.


### `struct FileEntry`

Record of a single file at a single point in time.


### `struct Snapshot`

A full snapshot of tracked files at a single moment.


### `enum FileDiffKind`

Describes how a single file changed between two snapshots.


### `struct FileDiff`

A diff entry for one file.


### `struct SnapshotStore`

In-memory store of snapshots with a configurable retention limit.


### `struct OrchestratorState`

Serializable snapshot of orchestrator state for session persistence.


### `struct SavedAgentState`

Serialized state of a single agent.


### `enum StateError`

Errors for state persistence.


## Module: `vox-orchestrator\src\summary.rs`

Context summarization logic.

> **NOTE: This module is used only for metrics/observation.**
> The Vox AI Agent handles context compaction natively (`compaction: { auto: true }`).
> This module should not be used for actual agent task memory.


### `struct Interaction`

A single interaction within a context window.


### `struct SummaryChain`

A progressively summarized chain of agent context.


### `struct SummaryManager`

Manager tracking summary chains for all agents globally.


### `struct TaskId`

Unique identifier for a task within the orchestrator.


### `struct AgentId`

Unique identifier for an agent within the orchestrator.


### `struct CorrelationId`

Unique identifier mapping a question and response together.


### `struct IdParseError`

Helper parsing error for identifiers.


### `struct BatchId`

Unique identifier for a batch submission


### `struct LockToken`

Handle for an active lock on a resource


### `struct TaskIdGenerator`

Thread-safe counter for generating sequential TaskIds.


### `struct AgentIdGenerator`

Thread-safe counter for generating sequential AgentIds.


### `struct CorrelationIdGenerator`

Thread-safe counter for generating sequential CorrelationIds.


### `enum TaskPriority`

Priority level for a task. Higher priority tasks are dequeued first.


### `enum TaskStatus`

Current execution status of a task.


### `enum AccessKind`

Kind of access an agent requires on a file.


### `struct FileAffinity`

A file path paired with the access kind required for a task.


### `enum TaskCategory`

General category of a task to guide model selection.


### `struct TaskDescriptor`

Description of a task before it is assigned an ID and routed in the orchestrator.


### `struct AgentTask`

A unit of work to be executed by an agent.


### `enum AgentMessage`

Messages exchanged via the shared bulletin board.


### `struct MessageId`

Unique identifier for a message.


### `enum A2AMessageType`

The type of A2A message.


### `struct MessageEnvelope`

Envelope metadata for traceability and precedence (system > policy > user > peer).


### `struct A2AMessage`

A structured message between agents.


## Module: `vox-orchestrator\src\validation.rs`

Post-task quality validation using TOESTUB.

This module is only compiled when the `toestub-gate` feature is enabled.
It runs the TOESTUB analysis engine on files that an agent just modified,
checking for AI coding anti-patterns before the task is considered complete.


### `fn post_task_validate`

Run TOESTUB validation on the files in a completed task's manifest.

Returns the number of findings at or above the `error` severity level.
If the count is > 0, the task should be considered failed (quality gate not passed).


### `fn quality_gate`

Check whether a validation result passes the quality gate.


### `struct ValidationResult`

Result of a post-task TOESTUB validation.


## Module: `vox-orchestrator\src\workspace.rs`

Agent workspaces — inspired by Jujutsu's multi-workspace model.

Each agent gets a lightweight virtual workspace (a diff-overlay on top of
the shared base) so multiple agents can edit the same codebase in parallel
without stepping on each other.  Changes are merged back atomically.


### `enum WorkspaceEntry`

A single file entry in the workspace overlay.


### `struct ChangeId`

Stable identifier for a logical unit of work that survives rebases
and amendments — inspired by Jujutsu's Change IDs.


### `enum ChangeStatus`

Status of a logical change.


### `struct Change`

A logical change — groups related snapshots across edits and agents.


### `struct AgentWorkspace`

A per-agent virtual workspace overlaying the shared repository.


### `struct WorkspaceManager`

Manages per-agent workspaces and change tracking.
