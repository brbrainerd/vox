---
title: "Agentic Secretary: Research Synthesis for Dynamic Task Management, Memory Transparency, and Chat-Driven Orchestration (2026-06-17)"
description: "Comprehensive research synthesis (20+ sources) covering 2026 best practices for AI-driven dynamic task lists, persistent memory management with rolling context compaction, always-listening chat interfaces, codebase knowledge gap detection, and transparent GUI representation — all scoped to the Vox orchestrator, GUI, and OpenRouter integration."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Establishes research baseline for the Vox Agentic Secretary capability — the canonical reference for the secretary agent design, task hopper evolution, context-window visualization, and memory GUI authoring decisions. All implementation plans deriving from this feature area should cite this document."
sourced_at: "2026-06-17"
vox_relevance:
  - "vox-orchestrator: hopper, planning, memory, compaction, queue"
  - "vox-gui: TasksView, ChatSurface, MemoryView, ChatExecutionRail"
  - "vox-actor-runtime: llm facade, infer_with_retry"
  - "vox-search: hybrid tantivy + vector retrieval, EmbeddingService"
  - "vox-db: knowledge_node, knowledge_edge, hopper_inbox (planned)"
companion_docs:
  - "docs/src/architecture/unified-task-hopper-research-2026.md"
  - "docs/src/architecture/vox-memory-model-audit-and-value-optimization-2026-06-05.md"
---

# Agentic Secretary: Research Synthesis — Dynamic Task Management, Memory Transparency, and Chat-Driven Orchestration (2026-06-17)

*Synthesized from 20+ web searches across authoritative 2026 sources including mem0.ai, letta.com, openrouter.ai, Tauri docs, arxiv.org, and industry practitioner blogs. All findings pertain to Vox design decisions unless noted.*

---

## 1. Executive Summary

The vision: an AI that acts as a persistent **secretary agent** — always listening to chat, continuously maintaining and rewriting its task list, visually exposing its memory and context-window state to the user, and autonomously restructuring plans as new information arrives. This document synthesises 2026 industry consensus on how to build this correctly, identifies what Vox already has, and maps the gaps the implementation plan must close.

**Key 2026 insight:** The field has converged on treating the **context window as RAM** and requiring a separate **persistent memory layer as storage**. The sophistication is no longer in raw context size but in *context engineering* — selective retrieval, compression, isolation, and externalization. Task lists are no longer static; they are living DAGs that agents rewrite based on new information, subject to human approval gates.

---

## 2. Domain 1 — Dynamic AI Task List Management

### 2.1 Industry Consensus (2026)

The 2026 standard for AI task management has moved from "God Model" monolithic architectures to **supervisor/worker orchestration**:

- A **Supervisor Agent** decomposes high-level goals into a DAG of sub-tasks
- **Worker Agents** execute individual nodes, reporting completion to the supervisor
- The supervisor monitors for failure signals (compiler errors, test regressions, user intent changes) and triggers **dynamic replanning**

The agentic loop that matters: **Plan → Act → Observe → Reflect → Replan**. This is not sequential; agents run it continuously. When a user types something in chat, it is a new **Observe** event that may trigger Reflect+Replan even if the current task is still running.

**Key pattern — Living Risk Models:** The best systems replace static task lists with live monitors. Indicators like scope creep, repeated task re-openings, or dependency blocks trigger proactive replanning suggestions surfaced to the user for approval.

### 2.2 Priority Architecture

Production systems in 2026 use three priority tiers (Urgent > Normal > Background), which is exactly what Vox's `AgentQueue` already implements. The missing piece is the **cross-agent global hopper view** and **telemetry-driven priority-learning loop** (per the unified-task-hopper-research-2026.md recommendation).

### 2.3 Replanning Triggers (2026 standard)

| Trigger | Vox Status |
|---|---|
| Compiler error unresolved | ✅ `ReplanTrigger::CompilerErrorUnresolved` exists |
| Test failure / new regression | ✅ `ReplanTrigger::TestFailureNewRegression` exists |
| Missing capability | ✅ `ReplanTrigger::MissingCapability` exists |
| **User chat message changes intent** | ❌ Not wired — `secretary.rs` is the gap |
| **New codebase discovery changes scope** | ❌ Not wired — discovery → hopper path is missing |
| **Agent confidence falls below threshold** | ❌ Not implemented |

### 2.4 Task Persistence Gap

The `InMemoryHopper` (Hp-T1) is ephemeral — all items are lost on process restart. The industry standard (Hp-T5 in the existing roadmap) requires a `hopper_inbox` database table. Until this lands, the "secretary" system cannot safely restart. **This is a prerequisite for any always-on secretary agent.**

---

## 3. Domain 2 — Persistent Memory Management

### 3.1 The RAM/Hard Drive Model (2026 consensus)

The most-cited mental model in 2026: **context window = RAM, persistent memory layer = hard disk**.

- Context window is volatile (clears at session end), finite (even 1M-token windows suffer "lost in the middle" degradation), and expensive
- Persistent memory must live in an external layer (vector store + knowledge graph + KV store) and be **dynamically injected** into the context at runtime — only the relevant subset, not the full history

### 3.2 Four-Type Memory Model (Letta / Mem0 consensus)

| Memory Type | Description | Vox Mapping |
|---|---|---|
| **Working Memory** | Active context window — the "RAM" | `CompactionConfig.max_context_tokens` |
| **Episodic Memory** | Past events/interactions (vector-indexed for similarity retrieval) | `VoxDb.recall_memory()` with vector embeddings |
| **Semantic Memory** | Extracted facts/preferences (deduplicated, knowledge graph) | `MemoryManager.sync_to_db()` + `upsert_knowledge_node()` |
| **Procedural Memory** | Agent's learned skills/instructions that evolve | Skill system (`vox-skills`) |

Vox's `MemoryManager` maps well to this model. The gap is **GUI exposure** of all four types and **asynchronous consolidation** (sleep-time compute).

### 3.3 Sleep-Time Compute / Memory Consolidation

**2026 breakthrough pattern:** Advanced agents perform "offline" memory consolidation during idle time — without consuming inference tokens during user interactions. Functions:

1. **Summarization** — condense long interaction histories into high-density insights
2. **Importance tagging** — determine what is worth retaining vs. pruning
3. **Fact unification** — merge contradictory or redundant data (temporal validity windows: `valid_from` / `valid_until` per fact)
4. **Intentional forgetting** — remove stale/low-signal information

**Vox gap:** The `compaction.rs` engine handles context-window trimming (session-scoped), but there is no **background asynchronous memory consolidation** process that runs between sessions to prune `MEMORY.md` and the knowledge graph.

### 3.4 Contradiction Resolution / Deduplication

2026 state of the art uses **temporal validity windows** on facts (a `valid_from` / `valid_until` timestamp per knowledge node) rather than naive append. When a new fact supersedes an old one, the graph automatically marks the prior node as obsolete. Vox's `upsert_knowledge_node` does not currently attach temporal validity — this is a schema gap.

Production deduplication policy layers:

- **Importance:** what is worth saving at all
- **Merge:** unify related facts about the same entity into a single canonical record
- **Decay:** gradually lower confidence of old facts over time
- **Eviction:** remove outdated information to prevent attention dilution

### 3.5 User-Editable Memory (Transparency Imperative)

2026 consensus: **users must be able to inspect, edit, and delete AI memory facts**. Tools like Mem0, Cognee, and Claude Projects all provide dashboards for this. The pattern:

- **Inspect:** List all stored facts with source, timestamp, and confidence
- **Edit:** Inline edit any fact directly in the GUI
- **Delete:** Remove stale or incorrect facts with confirmation
- **Provenance:** Every fact links back to the conversation turn or file that generated it

**Vox gap:** `MemoryView.tsx` supports search/read (excellent) but **no inline edit or delete** of individual facts, and **no provenance trace**.

---

## 4. Domain 3 — Context Window Management and Visualization

### 4.1 Context Engineering (Karpathy / 2026 framing)

Andrej Karpathy's framing, now industry consensus:

> *"The LLM is a CPU. The context window is RAM. The engineer's role is the operating system — loading, managing, and curating the exact data the model needs."*

The 2026 progression:

| Era | Focus |
|---|---|
| 2022–2024 | Prompt engineering ("how do I phrase this?") |
| 2025 | Context engineering ("what information does the model need?") |
| **2026** | **Harness engineering** ("what system design ensures the right context?") |

Four strategies for managing the attention budget:

1. **Write (Externalize)** — move information into stable external structures
2. **Select (Retrieve)** — use high-precision RAG to inject only relevant information
3. **Compress (Summarize)** — condense long-form history into task-specific summaries
4. **Isolate (Partition)** — partition agent environments to prevent context clash

### 4.2 State of the Art: LCLMs and ACON (June 2026)

- **Latent Context Language Models (LCLMs):** compress input sequences into latent embeddings *before* the decoder — up to 16x compression with ~9x speedup on long-context benchmarks
- **ACON (Agentic Context Optimization, ICML 2026):** optimises compression specifically for long-horizon agents, preserving reasoning traces and tool-use histories without overwhelming the attention budget
- **Sparse Attention / Lightning Indexer:** models attend to a sparse, highly relevant subset of compressed history — reduces FLOPs and KV cache memory

**Vox relevance:** The existing `compaction.rs` with `CompactionStrategy::{Aggressive, Balanced, Conservative}` is well-aligned with ACON principles. The head/tail preservation matches the "Lost in the Middle" mitigation. The gap is **no GUI visualization** of this.

### 4.3 Context Window Visualization (2026 tool patterns)

Industry tools (TokenBar, Context Lens, Langfuse, Braintrust) provide:

- Color-coded progress bars (green → yellow → red as context fills)
- Zone breakdown: system prompt zone, RAG zone, conversation history zone, reserved response zone
- Alert thresholds (e.g., "80% = compaction imminent")
- Compaction timeline: "Compacted at 14:32 — summarized 47 turns"

**Vox gap:** No GUI component exposes `CompactionConfig.max_context_tokens`, current usage, threshold line, or compaction events to the user.

### 4.4 OpenRouter Integration Patterns (2026)

For Vox's OpenRouter backend via `vox_actor_runtime::llm`:

- **Context compression plugin:** OpenRouter provides a built-in `context-compression` plugin — automatic mid-prompt truncation if the model limit is exceeded
- **Prompt caching (`promptCacheKey`):** for recurring system prompts / memory blocks — reduces latency and cost by 50–90% on turn 6+ of a chat session
- **Model fallback chains:** define fallback sequences in the model registry entry
- **`X-OpenRouter-Metadata` header:** routing decisions, latency, and cost-per-chunk observability

---

## 5. Domain 4 — Always-Listening Chat Interface (Secretary Pattern)

### 5.1 2026 Industry Pattern: Always-On Agents

Leading examples: **Microsoft Scout**, **Google Remy**, **remio**. Common architecture:

- Agent runs as a **background task** (not invoked on demand) — continuously processing events from meetings, emails, files, and chat
- Multi-agent supervisor/worker: a "secretary" supervisor interprets intent; worker agents update the task list, memory, and knowledge graph
- **Unified interface:** chat is the single control surface; task list, memory, and context window visualizations are read-back surfaces

### 5.2 Chat → Task Pipeline (Intent Extraction)

Modern pipelines (LangGraph, CrewAI, AutoGen):

1. **Router agent:** classifies incoming chat message — new goal, priority change, scope reduction, or context only
2. **Context injection:** retrieves relevant memory and current plan state before delegating
3. **Hopper submission:** submits new `IntakeItem`s or calls `reprioritize()` on existing ones
4. **Handoff:** passes updated context to the next working agent

**Vox gap:** The `ChatSurface.tsx` and Loquela composer submit messages to a chat session; there is no pathway that also forwards these to a secretary agent that reads intent and mutates the task hopper.

### 5.3 A2A Protocol for Secretary → Workers

The 2026 A2A standard:

- Agents expose "Agent Cards" (`/.well-known/agent.json`) describing capabilities
- The secretary agent discovers workers via the capability registry
- Messages between agents carry typed context envelopes

Vox's existing `subagent_dispatch.rs` and A2A `remote_worker.rs` provide the plumbing. The gap is the **secretary agent** that initiates dispatch based on chat input.

---

## 6. Domain 5 — Codebase Knowledge Gap Visualization

### 6.1 2026 Trend: AST-Derived Codebase Graphs

- **Deterministic > LLM-extracted:** tree-sitter AST indexing is now preferred over LLM entity extraction for structural accuracy
- **GraphRAG:** treating the codebase as a graph enables "repo-wide" reasoning rather than isolated file reads
- **Hybrid indexing:** vector (semantic) + keyword/metadata (exact function names, error codes)

**Vox relevance:** `vox-search` already provides tantivy lexical + semantic indexer + RRF fusion. The `knowledge_node` / `knowledge_edge` schema in VoxDb supports graph traversal. The gap is a **coverage map**: which files have been indexed vs. not, surfaced visually.

### 6.2 Knowledge Gap Detection Methods

1. **Confidence scoring:** flag retrievals where similarity score < threshold as potential gaps
2. **Simulated querying:** proactively generate synthetic queries and measure if the system can retrieve a valid answer — identifies "sparse" areas
3. **Failure analysis:** treat retrieval failures as actionable signals — add the failed domain to the re-indexing queue

**Vox gap:** No GUI component shows which workspace files are not yet indexed. The `vox-search` `EmbeddingService` has indexing state internally but it is not surfaced.

### 6.3 Agentic UI for Knowledge Gaps (A2UI Pattern)

2026 emerging pattern: agents update visualizations dynamically during their reasoning loop. If a secretary agent hits a retrieval gap, it:

1. Flags the gap visually ("I don't have context on `crates/vox-new-crate/` yet")
2. Offers a button to trigger indexing
3. Proceeds to index in background while the user continues working

---

## 7. Domain 6 — Human-in-the-Loop Approval Flow (2026 patterns)

### 7.1 HOTL vs. HITL

The 2026 shift: from **HITL** (constant supervision) to **HOTL** (Human-on-the-Loop, exception-based):

- Agent operates autonomously within guardrails
- Only interrupts the user for **high-signal, high-risk, or irreversible** operations
- Visible countdown for auto-proceed (30–60s is common); default depends on action risk

### 7.2 Key UX Patterns

| Pattern | Description |
|---|---|
| **Default-Deny timeout** | For irreversible actions, default is DENY if timer expires |
| **Default-Proceed timeout** | For low-risk plan mutations, auto-proceeds after N seconds |
| **Context-rich approval banners** | Show tool being called, data affected, estimated risk |
| **Per-action autonomy** | Users set granular policies per action class |
| **Progressive autonomy** | UI shows current "autonomy mode" (Suggest / Confirm / Execute) |

**Vox gap:** The existing Approvals surface handles command-level approvals. For secretary-driven task mutations, a new **Task Evolution Banner** component is needed.

### 7.3 State-Managed Interruptions

Frameworks like LangGraph and Temporal preserve agent state during approval waits — if the user closes the app and returns 10 minutes later, the approval request and agent state are both still intact.

**Vox gap:** `InMemoryHopper` cannot survive a restart. The `vox-journal` / `vox-workflow-runtime` durable journal should be the backing store for suspended approval states.

---

## 8. Domain 7 — Real-Time GUI Push (Tauri Patterns)

### 8.1 Three Architectural Choices

| Mechanism | Best For | Vox Use |
|---|---|---|
| **Tauri native events** (`app.emit()` / `listen()`) | Rust backend → React frontend, status/state updates | **Primary choice** for task list updates |
| **SSE** | Unidirectional streaming (AI token-by-token output) | Already used for Loquela streaming |
| **WebSocket** | Bidirectional interactive agent control | Overkill for task list |

### 8.2 Recommended Pattern for TasksView

Replace the 4-second polling with Tauri event subscription:

```rust
// Rust: emit when any task state changes
app_handle.emit("orchestrator://task-changed", task_payload).unwrap();
```

```typescript
// React: listen in useEffect
useEffect(() => {
  let unlisten: () => void;
  listen<TaskRow[]>('orchestrator://task-changed', event => {
    setRows(event.payload);
  }).then(fn => { unlisten = fn; });
  return () => { unlisten?.(); };
}, []);
```

**State management recommendation:** sync event data into Zustand so all components (TasksView, ChatExecutionRail, Dashboard widgets) share a single live task store without prop-drilling.

### 8.3 Race Condition Guard

Keep **Rust as single source of truth**. If a window updates task state, broadcast a sync event to all other windows. Do not allow two windows to independently mutate task state.

---

## 9. Gap Matrix: Vox vs. 2026 Best Practices

| Capability | 2026 Standard | Vox Status | Gap Description |
|---|---|---|---|
| Dynamic task DAG | Plan → Act → Observe → Reflect → Replan | ✅ `PlanNode` + `replan.rs` | Solid foundation |
| Chat → task rewrite | Secretary agent, intent router | ❌ Missing | `secretary.rs` needed |
| Task hopper GUI | Single cross-agent view, live push | ⚠️ Partial | Polls 4s; no push; no completion purge |
| Task persistence | DB-backed hopper | ❌ `InMemoryHopper` only | Hp-T5 `hopper_inbox` table needed |
| Memory: 4-type architecture | Working/Episodic/Semantic/Procedural | ⚠️ Partial | Working + Semantic + partial Episodic exist |
| Memory: sleep-time consolidation | Background async deduplication | ❌ Missing | No background consolidation process |
| Memory: temporal validity | `valid_from` / `valid_until` per fact | ❌ Missing | Schema gap in `knowledge_node` |
| Memory: user editable GUI | Inspect / edit / delete facts | ⚠️ Read-only | `MemoryView.tsx` needs edit/delete |
| Memory: provenance trace | Every fact → source turn/file | ❌ Missing | Not stored in current schema |
| Context window visualization | Token meter, zone breakdown, compaction timeline | ❌ Missing | No GUI component exists |
| OpenRouter: prompt caching | `promptCacheKey` for system prompts | ❌ Not wired | LLM egress needs cache key support |
| Codebase knowledge gap | Coverage map, gap indicators | ❌ Missing | No GUI; `vox-search` has internal state |
| Real-time task push | Tauri event vs. 4s poll | ❌ 4s poll only | Need `app.emit()` on state change |
| HOTL approval flow | Task evolution banner, countdown | ❌ Missing | Only code-edit approvals exist |
| A2A chat-to-task pipeline | Intent router → hopper | ❌ Missing | Secretary agent needed |

---

## 10. Research Gaps and Open Questions

### RG-1: Secretary Agent LLM Model Selection
Must use `vox_actor_runtime::llm::llm_chat()`. Recommend: heuristic pre-filter (keyword → intent) + small/fast model as fallback. Add a `Secretary` role to the model registry with `latency_budget_ms: 500`. Avoid always invoking a large model for simple chat messages.

### RG-2: `hopper_inbox` Schema Design (Hp-T5)
Needs design before the secretary agent is safe to ship. Key questions: SQLite vs. Turso? Migration strategy from `InMemoryHopper`? The VoxDb schema migration plan must be defined.

### RG-3: Memory Consolidation Frequency
Recommend: on session end + idle timeout (30 min default, configurable). Following Letta's pattern of async consolidation during idle periods.

### RG-4: Context Window Token Counting Source
The `CompactionConfig` knows `max_context_tokens` but current token count is not exposed via Tauri. A new `get_context_budget` command is needed returning: `{used_tokens, max_tokens, threshold_tokens, strategy, last_compacted_at}`. Source: LLM response metadata from `vox-llm-egress`.

### RG-5: Knowledge Coverage Scanning API
To show un-indexed files: (1) walk workspace file tree, (2) query `EmbeddingService` for indexed paths, (3) diff the two sets. The `vox-search` crate needs a `list_indexed_paths()` API.

### RG-6: Task Evolution Banner vs. Existing Approvals Surface
Recommendation: keep separate. The existing Approvals surface handles code-edit approvals with different urgency signals and UX requirements. A new `TaskEvolutionBanner` component lives in the Tasks surface.

---

## 11. Recommended Implementation Phase Order

Based on dependency analysis and risk (details in implementation plan):

| Phase | What | Dependency |
|---|---|---|
| **P0** | Hp-T5: `hopper_inbox` DB table | Prerequisite for all persistence |
| **P1** | `get_context_budget` Tauri command + `ContextWindowMeter` widget | Independent quick win |
| **P2** | Real-time task push via `app.emit()` | Replace 4s poll |
| **P3** | `secretary.rs` + chat → hopper pipeline | Depends on P0 |
| **P4** | `TaskEvolutionBanner` + HOTL approval flow | Depends on P3 |
| **P5** | Memory inline edit/delete + provenance | Schema extension needed |
| **P6** | Knowledge gap coverage map | `vox-search` API extension |
| **P7** | Background memory consolidation (sleep-time) | Highest complexity; defer |

---

## 12. Sources Consulted (Selected)

- **mem0.ai** — Memory as Infrastructure patterns, four-type memory model, hybrid retrieval
- **letta.com** — OS-style agent architecture, sleep-time compute, git-backed memory
- **openrouter.ai** — context compression plugin, `promptCacheKey`, streaming, fallback chains
- **arxiv.org** — ACON (ICML 2026), LCLMs (June 2026 pre-print), GraphRAG codebase indexing
- **tauri.app** — native event system, `app.emit()` / `listen()`, plugin-websocket
- **agentic-patterns.com** — HOTL approval UX, per-action autonomy, state-managed interruptions
- **temporal.io** — durable execution for approval wait states
- **machinelearningmastery.com** — Mem0 vs. Letta comparison, 4-type memory architecture
- **supermemory.ai** — knowledge graph vs. vector store for contradiction resolution
- **cogitx.ai** — MCP as connectivity standard, OODA loop planning strategies
- **lancedb.com / cocoindex.io** — syntax-aware (tree-sitter) codebase indexing
- **tokenbar.site / Context Lens** — token budget visualization tools and UX patterns
- **truefoundry.com** — agent gateway patterns, RBAC, token budgets
- **Langfuse / Braintrust** — observability for production LLM systems
- **Andrej Karpathy** (via industry synthesis) — context window as RAM / harness engineering framing
- **Microsoft / Google** (Scout, Remy) — always-on secretary agent product patterns
