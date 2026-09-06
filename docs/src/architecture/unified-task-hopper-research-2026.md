---
title: "Unified Task Hopper — Research, Design Space, and Recommendation (2026-05-09)"
description: "Audits the proposal of a single developer-facing 'hopper' (one chat-driven intake that fans out across all agents and the mesh) against the existing per-agent priority queues, scope-based isolation, agentic VCS automation, and Populi mesh north-star. Identifies what is already built, what is genuinely new, the version-control consequences across concurrent agents, the telemetry-driven priority-learning loop, the developer-override invariant, the mesh dimension, and the do-nothing alternative. Concludes with a recommendation."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Establishes the canonical mental model for cross-agent task intake and prioritization, and the contract that orchestrator dispatch must obey developer overrides. Future implementation plans will cite this doc as the SSOT for hopper design choices."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-orchestrator: queue, scope, events, workspace, dispatcher"
  - "vox-orchestrator-queue: AgentQueue, priority tiers, dependency tracking"
  - "vox-orchestrator-types: AgentTask, TaskId, TaskPriority, capability tokens"
  - "vox-dashboard: Phase 2/3 surface — would host the hopper view"
  - "vox-populi: mesh dispatch (currently local-first, mesh secondary)"
  - "vox-telemetry: hopper priority-learning loop wiring"
companion_docs:
  - "docs/src/architecture/populi-mesh-north-star-2026.md"
  - "docs/src/architecture/agentic-version-control-automation-research-2026.md"
  - "docs/src/architecture/multi-agent-vcs-replication-spec-2026.md"
  - "docs/src/architecture/nextgen-orchestrator-research-2026.md"
  - "docs/src/architecture/dashboard-migration-research-2026.md"
---

# Unified Task Hopper — Research, Design Space, and Recommendation (2026-05-09)

## Executive Summary

The proposal: replace the current "agent thinks of work, agent picks it up" loop with a single
**developer-facing intake** ("hopper") where ideas are dropped into one chat, the orchestrator
decides how and when to slot them into outstanding work across all agents and the mesh, and the
developer retains full priority-override authority through the dashboard. Telemetry feeds back into
the prioritization model so the system gets smarter at scheduling.

**Recommendation, in one paragraph.** Build the hopper as a thin **intake-and-prioritization layer
above the existing per-agent queues** — not as a replacement for them, and not as a competitor to
git/jj branch isolation. The per-agent priority queues in `vox-orchestrator-queue` already do 80%
of the dispatch work that a hopper would need; what is genuinely missing is (1) a single
developer-facing intake surface, (2) a `TaskReprioritized` event the orchestrator must obey,
(3) a cross-agent global view in the dashboard, and (4) a telemetry-driven priority-suggestion
loop. The "hopper vs. branch isolation" framing is a category error: branch isolation solves
**file-write conflicts**, the hopper solves **work intake and prioritization**. They are
orthogonal layers and both should remain. The version-control implications of multiple agents
draining one hopper *do* matter, but they are already governed by the agentic-VCS automation work
(`@vcs.*` decorators, capability tokens, op-log gossip) — the hopper inherits those guarantees
rather than relitigating them. Net assessment: **modest but real improvement** over the current
orchestrator, with the bulk of the gain coming from the single-intake UX and the formal
override contract, not from the queue mechanics themselves. Defer mesh-wide hopper distribution
behind a `v0.7+` flag; do single-machine hopper first.

---

## Part 1 — Current State (Audited 2026-05-09)

The audit below is grounded in the code as of `cc_bdesktop2/elastic-euler-ecb79d`. Anything that
later drifts from this picture invalidates the recommendation; cite this section when re-auditing.

### 1.1 The unit of work already exists

`AgentTask` in [`crates/vox-orchestrator-types/src/agent_types/`](../../../crates/vox-orchestrator-types/)
is the dispatch unit. Each task carries an ID, a priority (`Urgent | Normal | Background`), an
assigned agent, optional dependencies, and capability hints. Tasks are not free-floating — at
dispatch time they are bound to an agent's queue.

### 1.2 Per-agent priority queues already exist

[`crates/vox-orchestrator/src/queue/mod.rs`](../../../crates/vox-orchestrator/src/queue/mod.rs)
implements `AgentQueue`:

- Three priority tiers (`Urgent > Normal > Background`), FIFO within tier.
- `enqueue()`, `dequeue()`, `mark_complete()`, dependency resolution.
- Per-queue pause flag, `last_active` timestamp, hardware capability hints, active-skill EWMA
  reliability scores, optional workflow-context binding, optional endpoint-reliability key,
  reducer cooldown.
- Pop variants split between `mod.rs` and the sibling [`drain.rs`](../../../crates/vox-orchestrator/src/queue/drain.rs) and [`priority.rs`](../../../crates/vox-orchestrator/src/queue/priority.rs).

This is most of a queue. It is per-agent, not global.

### 1.3 Isolation is *scope-based*, not branch-based

[`crates/vox-orchestrator/src/scope.rs`](../../../crates/vox-orchestrator/src/scope.rs)
implements `ScopeGuard` — file-level write boundaries per agent, validated by code, not by
git branch. Concurrent agents do not require concurrent branches; the scope guard prevents
write collisions inside the orchestrator.

[`crates/vox-orchestrator/src/workspace.rs`](../../../crates/vox-orchestrator/src/workspace.rs)
provides `AgentWorkspace` — a lightweight diff overlay over the shared repo. Each agent gets a
private change set; commits to git only happen when the agentic-VCS automation work mints the
necessary capability and goes through `git_exec.rs`.

**Branch and worktree are an external convention** (the Claude Code harness spawns
`.claude/worktrees/cc_*`), not an orchestrator-managed mechanism. The orchestrator does not
itself create branches or worktrees.

### 1.4 Task lifecycle telemetry exists; reprioritization telemetry does not

[`crates/vox-orchestrator/src/events.rs`](../../../crates/vox-orchestrator/src/events.rs) emits
on a `tokio::broadcast` channel:

- `TaskSubmitted`, `TaskStarted`, `TaskPhaseChanged`, `TaskCompleted`, `TaskFailed`
- `TaskDelegated` (parent → child agent), `TaskDoubted`, `TaskResolved`
- `OperatingModeChanged`, `ActivityChanged`

Notably absent: `TaskReprioritized`, `TaskReassigned`, `TaskParked`, `TaskQueueRebalanced`. The
queue exposes `reorder(TaskId, Priority)` mechanically but no event is emitted, which means a
developer-driven reprioritization is invisible to telemetry, the dashboard, and the
priority-learning loop.

### 1.5 Dashboard is a stub

[`crates/vox-dashboard/src/`](../../../crates/vox-dashboard/) ships
`/api/v2/runs` and `/api/v2/mesh` returning fixture data. The
[dashboard migration research doc](dashboard-migration-research-2026.md) explicitly notes
the missing operator's harness — no command palette, no status bar, no persistent run timeline,
no global queue view. There is nothing to override and nothing to drag in today.

The agentic-VCS Phase 3 plan (already drafted) lays down the read-only VCS panels at
`/api/v2/vcs/{branch-board,oplog,push-queue,ledger}` plus a WebSocket at `/api/v2/vcs/events`.
That is the natural location to hang the hopper view alongside.

### 1.6 Mesh dispatch is "local-first, mesh secondary"

[`docs/src/architecture/populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md)
makes this explicit: lease grants exist in the type system (`vox-orchestrator-types`) but the
dispatch loop does not consult them; remote workers shadow local execution rather than
displacing it (W1, "Authoritative leases"). Cross-node observability (W5) defines a trace
context but no specification for mesh-wide task dispatch or prioritization. ADR-017 keeps
remote execution non-authoritative.

This means a **mesh-wide hopper** — one intake that distributes work across multiple
machines — is genuinely greenfield. The single-machine hopper, by contrast, sits squarely on
top of code that already exists.

### 1.7 No prior "hopper" research in the repo

A repository-wide search for prior art on single intake / unified queue / cross-agent
prioritization returned nothing. This idea has not been written up before under any name.

---

## Part 2 — What "Hopper" Means (and What It Does Not)

Before evaluating, sharpen the term. The user's description in plain words:

> As a developer thinks of ideas, they can add them to the chat and the system and orchestrator
> will automatically figure out how, based on what its current task lists are across all agents,
> across the entire mesh, where to work these in based on the still outstanding tasks.

Decomposed:

| Capability | Already exists? | New? |
|---|---|---|
| Per-task priority and dependency tracking | Yes (`AgentQueue`) | — |
| Per-agent serial dispatch with affinity | Yes (`scope.rs`, `workspace.rs`) | — |
| Single developer-facing chat-style intake | No | **Yes** |
| Cross-agent global queue *view* | No (dashboard stub) | **Yes** |
| Auto-routing of new ideas across queues | Partial (file affinity exists; semantic routing does not) | Mostly new |
| Developer override with AI-must-adapt contract | No | **Yes** |
| Telemetry-driven priority *suggestions* | No | **Yes** |
| Mesh-wide task distribution | No (mesh dispatch is non-authoritative) | **Yes** |

What the hopper is **not**:

- **Not a replacement for per-agent queues.** The execution-side state (in-progress task,
  completed-task ledger, dependency unblock, capability hints) belongs on the agent's queue and
  should stay there. A global queue that pretends agents are interchangeable would lose the
  affinity, capability, and skill-reliability signal the orchestrator already uses.
- **Not a replacement for branch / worktree isolation.** Worktrees solve concurrent file writes
  and let agents `git commit` without stomping each other; the hopper is upstream of that and
  decides *what* to do, not *where* on disk to do it.
- **Not a replacement for the agentic-VCS capability model.** Capability tokens
  (`WorkingTreeWrite`, `BranchCreate`, etc.) gate write effects regardless of how the work
  arrived. Tasks coming through the hopper still mint commit envelopes through
  `vox_commit_create` and still pass the secret scanner.
- **Not an editor.** Per the dashboard SSOT, the dashboard is a read-and-influence surface,
  not a code-authoring surface. Reordering a task is "influence"; rewriting code is not.

---

## Part 3 — Three Design Options

### Option A: Lightweight Intake Adapter (recommended starting point)

A minimal "inbox" abstraction in `vox-orchestrator` that accepts free-form intent from a single
chat surface, classifies it (file affinity → existing routing logic; semantic priority hint →
new), and enqueues onto an existing `AgentQueue`. The developer's chat is the only new front
door; everything downstream is the existing dispatcher.

```
            ┌──────────────────────┐
   chat ──▶ │  HopperIntake (new)  │ ──▶ classify(file, semantic)
            └──────────────────────┘             │
                                                 ▼
                                    ┌────────────────────────┐
                                    │ AgentQueue (existing)  │
                                    │  Urgent / Normal / Bg  │
                                    └────────────────────────┘
                                                 │
                                                 ▼
                                          dispatcher (existing)
```

**New surface area** is a single L1 module `crates/vox-orchestrator/src/hopper/`:
- `IntakeItem { intent: String, affinity_hints: Vec<PathBuf>, priority_hint: PriorityHint, source: IntakeSource }`
- `HopperIntake::submit(item) -> Result<TaskId, IntakeError>`
- `HopperIntake::reprioritize(id, new_priority, reason) -> Result<(), ReprioritizeError>`
  - Emits a new `AgentEvent::TaskReprioritized` event (must be added to `events.rs`).

**Pros:** smallest possible delta; respects every existing invariant; ships in one PR.
**Cons:** the global view across agents is still the dashboard's responsibility, so the operator
UX gain is partial until the dashboard catches up.

### Option B: Full Hopper Service (with a Persistent Cross-Agent Inbox)

Promote the hopper to a first-class service: `vox-orchestrator-hopper` crate, persistent inbox
in the same SQLite store the agentic-VCS Phase 1 work uses, an admission policy independent of
per-agent queues, and a dashboard panel at `/api/v2/hopper/{inbox,assigned,history}`.

```
   chat ──▶  Hopper (new crate)  ──▶  inbox (sqlite, persistent)
                  │                            │
                  │                            ▼
                  │                    admission policy
                  │                  (capacity, affinity,
                  │                   capability, skill EWMA)
                  ▼                            │
            telemetry feed                     ▼
                                       AgentQueue (existing)
```

**Pros:** survives orchestrator restarts; explicit cross-agent view; clean place to attach the
priority-learning loop and developer-override audit log.
**Cons:** non-trivial state machine; admission-policy bug = "queues silently starve"; introduces
a second source of truth for "what work exists" alongside the per-agent queues.

### Option C: Mesh-Native Hopper (deferred)

Same shape as Option B, but the inbox is replicated across the mesh via the same op-log gossip
mechanism that `multi-agent-vcs-replication-spec-2026.md` defines for code. Any node can submit;
any node's agent can dequeue; conflicts (two nodes both trying to claim the same intake item) are
resolved by the same `ConvergenceEngine`.

**Pros:** symmetric with the VCS replication story; one mental model for "things that converge
across the mesh"; lays the foundation for the eventual public-mesh / volunteer-compute network.
**Cons:** dispatch correctness across a partitioned mesh is a hard problem; lease semantics are
still being worked out (per the Populi north-star W1); this almost certainly should not land
before `v0.7+` and almost certainly not before mesh dispatch becomes authoritative.

### Why Option A first

Per the project's "scope-check before implementing" feedback rule and per the Populi north-star's
"local-first, mesh secondary" stance, the right shape is to land Option A as the minimum viable
hopper, prove it earns its keep against the current per-agent loop, then promote to Option B once
there is real signal that persistent cross-agent state is needed. Option C waits for mesh dispatch
to become authoritative — that's a `v0.7+` conversation, not a `v0.6` one.

---

## Part 4 — The Mesh Dimension

Including the mesh in the hopper picture changes the geometry but not the recommendation.

### 4.1 The single-machine case

Single machine, multiple agents (the common case for a developer with multiple Claude / Cursor /
Codex sessions): the hopper sits in one process, intake is local, dispatch is local. No new
distributed-systems primitives are required. This is Option A territory and earns its keep
purely from the single-intake UX and the override contract.

### 4.2 The two-machine LAN case (`v0.7` target per Populi north-star)

Two machines paired over a trusted LAN. The natural failure modes:

- **Where does the intake live?** Either pin it to the laptop (typical case) and let the
  workstation be a worker, or replicate the inbox via op-log gossip. Pinning is simpler;
  replication is more symmetric.
- **What about offline?** If the laptop goes offline mid-task, the workstation's agent has
  to decide whether to keep working. Today this is governed by lease semantics (per the
  populi north-star W1, not yet authoritative). A mesh-aware hopper would refuse to dispatch
  to a peer for whom no lease can be granted, and would surface that on the dashboard.
- **How does the developer reprioritize when offline?** The override contract has to specify
  CRDT-style merging, or last-writer-wins with a clear UI affordance for "this got overridden
  by your laptop after you reordered on the workstation." The agentic-VCS replication spec's
  `MergePolicy` taxonomy (`auto-merge | surface | arbitrate | block`) is the right vocabulary.

### 4.3 The internet-facing personal mesh (`v1.0` target)

Once mesh nodes can be public-internet-reachable, the hopper inherits every threat from the
mesh (capability tokens, signed op-fragments, secret pairing). The hopper *itself* doesn't
introduce new threats — it consumes the existing mesh trust primitives. But it does multiply
the **blast radius** of an admission-policy bug: a hopper that lets a malicious peer enqueue
runs work on your machines.

### 4.4 The "grand network" (`v1.x`)

Volunteer compute. Out of scope for this document; covered separately in the populi north-star.
The hopper would surface as the primary developer-facing UI for offering work to and consuming
work from the network, but the substrate decisions live there.

### 4.5 Mesh recommendation

Land Option A on a single machine. Specify the persistence schema in Option B in a way that is
*compatible with* op-log gossip (use content-addressed identifiers; avoid mutable surrogate keys;
keep admission policy a pure function of mesh-observable state). When Populi mesh dispatch becomes
authoritative, Option C falls out of Option B with one new transport adapter. Do **not** design
for the grand network yet.

---

## Part 5 — Version Control Implications (the user's real concern)

This section is the load-bearing one. The user's concern is not "should we keep using
worktrees" — it is "what happens to version control when many agents are draining a single
intake on the same codebase."

### 5.1 What changes when multiple agents drain one hopper

Today's de-facto model is one chat → one agent → one branch (or worktree). The hopper inverts
this: one chat → many agents → many branches, with the orchestrator deciding the fan-out. The
VCS-relevant consequences:

| Concern | Current de-facto behavior | Hopper-induced change |
|---|---|---|
| Branch creation rate | One branch per chat session | Many branches per hopper session, possibly without the developer naming any of them |
| Branch naming | Human-meaningful (`fix-login`) | Synthetic (`hopper/<hopper-task-id>`) unless the intake item is annotated |
| Concurrent writes to the same file | Rare (one agent per worktree) | Routine (file-affinity routing must hold; if it doesn't, op-log gossip resolves) |
| PR cardinality | One PR per logical change | Could explode into dozens of micro-PRs unless the hopper batches |
| Review burden | Human reads N PRs from N chats | Human reads N PRs from one hopper session — much harder to maintain context |
| Rollback granularity | Per-PR | Per-hopper-task (smaller unit, but harder to find when you need it) |

### 5.2 What stays the same

The agentic-VCS automation work (Phases 1–4, see [agentic-version-control-automation-research-2026](agentic-version-control-automation-research-2026.md)
and the four implementation plans `agentic-vcs-automation-impl-plan-phase{1..4}-2026.md`) already
provides the **per-effect** guarantees a hopper needs:

- **Capability tokens** (`WorkingTreeWrite`, `BranchCreate`, `PushAllowed`, `ForcePushAllowed`,
  `DestructiveOp`) gate write side regardless of intake source.
- **`GitExec` wrapper** refuses banned commands at spawn — `clean -fxX`-style permutations,
  `push --force`, `branch -D` blocked at the executor layer.
- **Commit envelopes** minted by `vox_commit_create` carry `Co-authored-by`, `Vox-Model-Id`,
  `Vox-Workspace`. Refused on workspace↔branch mismatch.
- **Secret scanner** (regex for AWS / GitHub / OpenAI / Anthropic / Slack / Google / PEM) blocks
  leaks at commit time.
- **`vox.vcs.*` telemetry** locked by integration test — every write effect is observable.
- **`@vcs.read_only` / `@vcs.requires(T)` / `@vcs.linear_working_tree` / `@vcs.audit_trail`**
  Vox-language decorators (Phase 4) make the effect set part of the function type, not just a
  runtime check.

So the question is **not** "can multiple agents safely write to the codebase from a hopper?" —
that's already governed. The questions that *are* new:

### 5.3 New VCS questions the hopper raises

**Q1. Does the hopper claim a branch per intake item, per intake batch, or per agent-session?**

Three credible answers:

- **Per intake item:** finest granularity. Maps cleanly to "one task = one PR." Branch names
  become bookkeeping (`hopper/<task-id>`), forcing the dashboard or the PR template to carry
  the human-meaningful description. Likely produces too many branches.
- **Per intake batch (e.g., per developer-defined "session"):** matches today's mental model
  of "a chunk of related work." Requires the intake to expose a "start a new session" affordance
  or auto-start one on the first hopper message.
- **Per agent-session (one branch per agent until the agent goes idle):** lets the orchestrator
  pack work onto fewer branches. Mixes unrelated changes if the orchestrator's batching is
  coarse.

**Recommendation: per intake batch, with the developer in control of when a batch starts and
ends.** The hopper UI offers a `/new-batch` (or equivalent) affordance; absent that, intake items
within a small idle window (e.g., 5 minutes) coalesce into the same batch. Each batch maps to one
branch and one PR.

**Q2. How does the hopper interact with `multi-agent-vcs-replication-spec-2026.md`'s
`AgentChange` / `OpFragment` / `ConvergenceSet` primitives?**

The replication spec says concurrent agents in the same workspace produce `OpFragment`s that the
`ConvergenceEngine` merges via the `MergePolicyV1` byte-range classifier. A hopper that fans work
out across agents *generates* those concurrent fragments. The hopper does not need to know about
fragments — it just needs to keep the file-affinity routing tight enough that auto-merge is the
common case (`MergeOutcome::AutoMerged`) and surface (not auto-resolve) the rest.

**Recommendation: the hopper SHOULD pass file-affinity hints into the routing layer, and
SHOULD NOT bypass `MergePolicyV1` on the back end.** Conflicts surface to the dashboard via the
existing convergence telemetry; the hopper does not need a parallel merge story.

**Q3. What happens when the developer reprioritizes a task that is already in flight?**

Three states, three behaviors:

- **Queued, not started:** trivial — just reorder. Emit `TaskReprioritized`.
- **In progress, not yet committed:** the orchestrator must cooperatively pause, snapshot the
  agent's working tree to a stash (or to the agent's `AgentWorkspace` overlay; both already
  exist), and re-dispatch. The agentic-VCS Phase 1 work added the `AgentWorkspace` overlay
  precisely to make this kind of mid-flight pause cheap.
- **Committed, not yet pushed:** the work has crystallized into a commit on the agent's branch.
  Reprioritization here is a no-op for the in-flight work; what reprioritizes is the *next* work
  for that agent. The dashboard should make this visible: "task X already produced commit
  `abcd1234`; reprioritization applies to your next intake."

**Recommendation: explicit state machine on the intake item:**
`Inbox → Triaged → Assigned → Started → CommitMinted → Pushed → Closed`, with reprioritization
allowed at any state but with semantics that depend on the state.

**Q4. Should the hopper auto-push?**

No. Auto-push from a hopper is a footgun: the developer loses visibility into what's leaving
the machine. The hopper produces commits (via the existing `vox_commit_create` envelope) and
optionally drafts PRs (via the existing `vox_pr_open` from agentic-VCS Phase 2). Push remains an
explicit developer action through the dashboard or CLI, which mints `PushAllowed` per push and
emits the existing telemetry events.

### 5.4 Branch / worktree audit recommendation

The user asked for an audit of branch isolation under this regime. Findings:

- **Worktree isolation is sound** for the current "one chat → one agent → one branch" model and
  should be retained.
- **Worktree-per-hopper-batch is the natural extension** if Option A or B lands. The Claude Code
  harness already creates worktrees under `.claude/worktrees/cc_*`; a hopper-aware variant would
  create them under `.claude/worktrees/hopper-<batch-id>-*` and delete them when the batch closes.
- **Worktree-per-agent (regardless of batch) is the alternative** and is closer to what
  `agentjj` / jj does. Has the advantage that the agent's working state survives across batches.
  Has the disadvantage of long-lived branches that drift far from `main`.
- **Worktree-per-task (one worktree per intake item) is over-fitted** and would create immense
  filesystem churn. Reject this.
- **Branch isolation should NOT be removed.** The scope-based isolation in `scope.rs` plus
  worktree branch isolation are belt-and-suspenders — both are cheap and they catch different
  failure modes (logic bugs in `ScopeGuard` vs. agents that escape the guard via shell-out).

There is no good reason to retire worktrees in favor of a "single workspace shared by all
agents from one hopper." The blast radius of a single bad agent is too large under that model.

---

## Part 6 — Telemetry-Driven Priority Learning

The user explicitly asked: hook this to telemetry so the system learns and gets smarter over time.

### 6.1 What "smarter" can mean

Three credible interpretations, each with a different cost:

1. **Smarter ETA estimates.** The hopper learns that "fix flaky test" takes 8 minutes on
   average and surfaces that estimate next time. Cheap; uses the existing
   `TaskCompleted` event with timing.
2. **Smarter routing.** The hopper learns that agent X is reliably better at TypeScript and
   agent Y at Rust, and routes accordingly. Already partially there via the per-agent
   `active_skills: HashMap<String, f64>` EWMA scores in `AgentQueue`. The hopper should *consume*
   those scores, not maintain a parallel system.
3. **Smarter priority suggestions.** The hopper learns that the developer almost always reorders
   "doc-only changes" to the bottom of the stack and starts pre-deprioritizing them. This is the
   genuinely new capability and the most valuable one.

### 6.2 The wiring

The existing event bus in [`crates/vox-orchestrator/src/events.rs`](../../../crates/vox-orchestrator/src/events.rs)
is the right substrate. Three new events are required:

```rust
AgentEvent::TaskReprioritized {
    task_id: TaskId,
    old_priority: TaskPriority,
    new_priority: TaskPriority,
    actor: ReprioritizationActor,   // Developer | Orchestrator | LearningPolicy
    reason: Option<String>,
    session_id: Option<String>,
}

AgentEvent::HopperItemAdmitted {
    item_id: HopperItemId,
    classified_priority: TaskPriority,
    classified_affinity: Vec<PathBuf>,
    confidence: f32,
    session_id: Option<String>,
}

AgentEvent::HopperItemOverridden {
    item_id: HopperItemId,
    original_priority: TaskPriority,
    developer_priority: TaskPriority,
    delta_seconds_since_admit: u64,
}
```

These events feed two consumers:

- **Dashboard** (read path): live view of the hopper, override audit trail, ETA visualizations.
- **Priority-learning policy** (write path, optional): a `vox-orchestrator` policy module that
  watches `HopperItemOverridden` events and adjusts the classifier's priors. **Off by default.**
  Opt-in. Documented as a `vox.priority-policy` feature.

### 6.3 What to learn from

The signal is rich and explicit:

- **Time-to-override:** if the developer almost always reorders within 30 seconds of admit, the
  classifier is too coarse — the policy should ask the developer earlier, not learn silently.
- **Direction of override:** systematic upgrades (`Background → Normal`) suggest under-classifying;
  systematic downgrades suggest over-classifying.
- **Override scope:** if the developer overrides every "doc-only" item the same way, learn that
  rule; if overrides are situational, do not generalize.

### 6.4 What NOT to learn from

- **Implicit signal from "the developer didn't reorder."** Absence of override is ambiguous: it
  could mean "agree" or "didn't notice." Treat it as weakly positive at best.
- **Cross-developer signal.** Each developer's priority taste is their own. Do not pool training
  data across users without explicit opt-in.
- **Anything that would cause the policy to override the developer.** The policy *suggests*; it
  never *decides* against an explicit developer setting. See the next section.

### 6.5 Reuse, not parallelism

The vox-telemetry plane (per the [telemetry unification 2026 SSOT](telemetry-unification-design-2026.md))
is the canonical destination for all `vox.*` events. The hopper events sit in the
`vox.orchestrator.hopper.*` namespace and inherit the existing privacy / sampling /
local-vs-remote routing rules. No new telemetry plane.

---

## Part 7 — The Developer-Override Contract (AI Must Adapt)

This is the invariant the user named. It deserves a precise statement.

### 7.1 The contract

> **At any point, if the developer sets the priority of a hopper item or queue, the orchestrator
> MUST honor that priority over its own reasoning. The orchestrator MAY emit a
> `priority_suggestion` *advisory* event for the developer's information, but MUST NOT mutate
> the developer-set priority without an explicit developer action.**

This is a one-way authority gradient. Developer settings dominate orchestrator settings dominate
learning-policy settings. Inversions are bugs.

### 7.2 Enforcement

Three layers of enforcement:

- **Type-system layer.** Introduce `TaskPriority { value, source: PrioritySource }` where
  `PrioritySource = Developer | Orchestrator | LearningPolicy`. The reorder API exposes
  `set_priority_developer(...)` and `set_priority_advisory(...)`; the dispatcher's reorder
  enforces a partial order on the source.
- **Capability layer.** Mutating a `Developer`-sourced priority requires a
  `DeveloperOverride` capability token (analogous to the agentic-VCS capability tokens). Only the
  hopper intake / dashboard can mint it; orchestrator policies cannot.
- **Telemetry / audit layer.** Any inversion attempt emits `HopperOverrideViolation` and is
  visible in the dashboard's override audit trail. CI integration test asserts this can never
  silently happen.

### 7.3 Edge cases

- **Conflicting developer settings from two surfaces** (e.g., chat and dashboard at the same
  time). Last-write-wins, with the audit trail showing both. Acceptable because both come from
  the developer; this isn't an authority inversion.
- **Developer priority + dependency:** a `Background` task with a dependency on an `Urgent`
  task is fine — the dependency blocks dequeue regardless of priority. Document this clearly.
- **Mesh case** (Option C): conflicts between developer overrides on different nodes resolve
  via the same op-log gossip CRDT story as everything else. This is why Option C inherits from
  Option B's persistence schema — the source field has to gossip cleanly.

### 7.4 Why this is more than UX

Without this contract, the system collapses into "AI suggests, developer ignores." The
priority-learning loop becomes useless because every developer override is silently undone
the next dispatch. The contract is what makes the learning loop *teach the system* rather
than *fight the developer.*

---

## Part 8 — Risks, Anti-Patterns, and What We'd Lose

### 8.1 Real risks

- **Admission-policy bugs starve queues.** A bad hopper that won't admit anything looks like
  "the system is hung." Mitigation: the hopper exposes a CLI fallthrough (`vox task submit
  --bypass-hopper`) and the dashboard surfaces inbox depth.
- **Single intake = single point of failure.** If the hopper process dies, no work flows.
  Mitigation: persist the inbox (Option B), expose CLI fallthrough, treat hopper crashes as
  paging-class events in telemetry.
- **Priority churn.** Frequent reprioritization torches the orchestrator's batching and local
  caches. Mitigation: rate-limit reprioritization in the dispatcher (developer can still do it
  fast; the *dispatch effect* of the change is debounced).
- **Intake quality degrades when easy.** A frictionless "drop ideas in chat" surface produces
  more intake noise. Mitigation: the dashboard's batch view makes the noise visible; the
  hopper auto-archives stale intake older than N days.
- **Mesh hopper (Option C) is a distributed-systems bet.** Splitting the inbox across nodes
  means consensus questions during partition. Defer until mesh dispatch is authoritative.
- **Privacy of the priority-learning loop.** Any classifier learning from developer behavior
  is, in some sense, profiling the developer. Mitigation: opt-in, local-only by default, no
  cross-user pooling.

### 8.2 Anti-patterns to refuse

- **"Hopper as the only way to submit work."** The CLI must remain a peer entry point. Forcing
  every task through one UI surface is a regression in agent-extensibility.
- **"AI proposes, AI auto-accepts."** Without the developer-override contract, this turns the
  hopper into a black box. Refuse.
- **"Hopper as code editor."** Per the dashboard SSOT, the dashboard is read-and-influence,
  not authoring. Same applies here.
- **"Mesh hopper before mesh dispatch is authoritative."** Inverts the foundation; will cause
  silent dispatch divergence.
- **"Hopper-managed worktrees that bypass `GitExec`."** Every git side-effect goes through
  `git_exec.rs`. No exceptions; arch-check enforces it.

### 8.3 What we'd lose by adopting the hopper

- **The "one chat = one branch = one PR" mental model.** Today it is easy to point at a chat
  and the PR it produced. The hopper's batching breaks that 1:1 mapping. The PR template and
  dashboard need to compensate.
- **The terseness of the per-agent loop.** Today the orchestrator's job is "pull the next task
  off this agent's queue." Adding hopper intake adds an upstream stage that has to be debugged,
  monitored, and tuned. The compensating gain is that intake is now first-class instead of
  implicit.
- **Some autonomy.** Agents that previously self-generated their next task ("I'll go run the
  tests") now have to either go through the hopper or be explicitly granted a self-enqueue
  capability. The right call is to grant that capability narrowly (test runs, doc rebuilds,
  housekeeping) and require hopper intake for everything else.

### 8.4 What we'd gain

- **A single front door** for work — easier to teach, easier to demo, easier to instrument.
- **Cross-agent visibility** that the current per-agent queues do not provide.
- **A formal override contract** that makes "AI must adapt" enforceable rather than aspirational.
- **A telemetry feedback loop** that converts override behavior into prioritization improvement.
- **A natural place** to hang the eventual mesh-wide dispatch UI without redesigning the
  developer surface again.

---

## Part 9 — Comparison with the Current Orchestrator

| Dimension | Current state | Hopper (Option A) | Hopper (Option B) | Hopper (Option C, mesh) |
|---|---|---|---|---|
| Per-agent queue | Yes | Unchanged | Unchanged | Unchanged |
| Cross-agent intake | No | Yes (in-memory) | Yes (persistent) | Yes (mesh-replicated) |
| Cross-agent global view | No (dashboard stub) | Limited | Full | Full + mesh map |
| Developer-override contract | Implicit | Explicit (typed) | Explicit (typed + persisted audit) | Explicit + CRDT-merged |
| Telemetry: lifecycle | Yes | Yes + new events | Yes + new events | Yes + mesh-aware |
| Telemetry: priority learning | No | Optional, off by default | Optional, off by default | Same, but per-node policies |
| Worktree isolation | External convention | Unchanged | Unchanged + batch hint | Unchanged |
| VCS write side | Agentic-VCS Phase 1–2 | Inherited | Inherited | Inherited |
| Mesh dispatch authority | Non-authoritative | Local | Local | Authoritative (when mesh dispatch is) |
| Persistent across restarts | No | No | Yes | Yes (gossip) |
| New crates | — | None | `vox-orchestrator-hopper` | `vox-orchestrator-hopper` (mesh adapter) |
| Net LoC estimate | — | ~800–1500 | ~3000–5000 | ~3000–5000 + transport |
| Risk | — | Low | Medium | High (distributed systems) |
| Gain over current | — | Modest but real | Significant | Significant + future-proof |

---

## Part 10 — Recommendation

**Adopt the hopper, but specifically as Option A first, with a forward-compatible persistence
schema designed to grow into Option B without a rewrite.**

1. **Build the hopper as an L1 module inside `vox-orchestrator`.** Single new module
   `crates/vox-orchestrator/src/hopper/`. New types: `IntakeItem`, `HopperItemId`,
   `IntakeSource`, `PriorityHint`, `PrioritySource`. New events:
   `TaskReprioritized`, `HopperItemAdmitted`, `HopperItemOverridden`.

2. **Add the developer-override contract as a typed invariant.** `TaskPriority` carries a
   `PrioritySource`; the dispatch path enforces a partial order on the source. CI test asserts
   no policy can mutate a `Developer`-sourced priority without a `DeveloperOverride` capability.

3. **Wire the hopper events into the existing event bus** in
   [`events.rs`](../../../crates/vox-orchestrator/src/events.rs). No new telemetry plane.

4. **Add a dashboard panel** (when dashboard work picks up — see
   [dashboard-migration-research-2026](dashboard-migration-research-2026.md)) at
   `/api/v2/hopper/{inbox,assigned,history}` and a WebSocket at `/api/v2/hopper/events`. Read-
   and-influence only.

5. **Inherit, do not replicate, agentic-VCS guarantees.** Every commit minted from a hopper task
   uses `vox_commit_create`; every push uses `vox_push` with `PushAllowed`. The hopper does not
   need its own VCS story.

6. **Inherit, do not replicate, the convergence engine.** The hopper passes file-affinity hints
   into the existing routing layer; concurrent writes that survive routing converge through
   `MergePolicyV1`.

7. **Persist with a forward-compatible schema** so Option B is a transport change, not a
   rewrite. Use content-addressed identifiers; avoid mutable surrogate keys. Defer the actual
   sqlite store until Option A demands it.

8. **Defer the mesh hopper (Option C).** Wait for Populi mesh dispatch to become authoritative
   per the populi north-star W1.

9. **Defer the priority-learning policy.** Land event emission in Option A. Land a
   read-only learning policy as a separate, opt-in `vox-priority-policy` crate later, and only
   after the developer-override contract is provably enforced.

10. **Do not retire branch / worktree isolation.** Worktrees solve a different problem
    (file-write conflicts, git history hygiene). The hopper sits above; it does not replace.
    Worktree-per-hopper-batch is the natural fit if branches become a friction point under the
    hopper.

### Is this an improvement on the current orchestrator?

**Yes, modestly but genuinely.** The current orchestrator is solid on dispatch (per-agent
queues, scopes, capability hints, EWMA reliability). Where it is thin is the developer's seat:
intake is implicit, cross-agent view is missing, override is informal, learning is non-existent.
The hopper closes those gaps without forcing a rewrite of the dispatcher. The recommendation is
not "replace the orchestrator" — it is "give the orchestrator a front door."

### What we are explicitly *not* recommending

- We are NOT recommending replacing per-agent queues with a single global queue.
- We are NOT recommending retiring worktree / branch isolation.
- We are NOT recommending mesh-wide hopper distribution before mesh dispatch is authoritative.
- We are NOT recommending an automatic priority-learning policy that overrides developer choices.
- We are NOT recommending the hopper be the only way to submit work; the CLI stays a peer entry
  point.

---

## Part 11 — Open Questions

These are unresolved and will need answers in the implementation plan (which is *not* this
document, per the scope-check rule):

1. **Intake batch boundaries.** Time-window default? Explicit `/new-batch` only? Both?
2. **Self-enqueue capability.** Which agent activities (test runs, doc rebuilds, housekeeping)
   may bypass the hopper? Concrete list, not principles.
3. **Hopper item TTL.** When does an unactioned intake item expire? Auto-archive vs. require
   developer dismissal?
4. **Reprioritization rate-limit window.** What's the right debounce so that fast developer
   reordering doesn't churn the dispatcher?
5. **Persistence schema versioning.** What's the migration story when the schema changes
   between Option A's in-memory and Option B's sqlite?
6. **Worktree-per-batch lifecycle.** When does a batch worktree get garbage-collected — at
   batch close, at PR merge, at TTL?
7. **Dashboard-vs-chat conflict resolution UI.** What does "you reordered in chat while the
   dashboard had a different order" look like to the developer?
8. **Mesh-readiness checklist.** What specific Populi north-star milestones need to land
   before Option C is even worth designing?

---

## Part 12 — Where This Doc Goes Next

Per the project's scope-check rule, this is a research-and-design-space doc, not an
implementation plan. The next deliverable, only after this doc is reviewed and approved,
would be:

- **`unified-task-hopper-spec-2026.md`** — narrowed to Option A, with the type-level contract,
  the new event variants, the L1 module shape, the migration plan for the new capability token.
- **`unified-task-hopper-impl-plan-phase1-2026.md`** — TDD step-by-step, written only when the
  spec is approved and this work is queued.

Neither is in scope for the current ask. They are listed here so the trail is obvious when the
work *is* queued.

---

## Appendix A — Glossary

- **Hopper:** the developer-facing intake surface that accepts free-form task ideas and routes
  them across the existing per-agent queues.
- **Intake item:** one developer-submitted unit of intent before classification. Becomes a
  `Task` once admitted and assigned.
- **Batch:** a developer-bounded group of intake items, mapping to one branch and one PR.
- **Override contract:** the typed invariant that developer-set priority dominates orchestrator-
  and learning-policy-set priority.
- **Per-agent queue:** the existing `AgentQueue` per agent (`crates/vox-orchestrator/src/queue/mod.rs`).
- **Scope-based isolation:** the existing per-agent file-write boundary (`scope.rs`).
- **Worktree isolation:** external (Claude Code harness) convention of one git worktree per
  agent, distinct from scope-based isolation.

## Appendix B — Files Cited

- [`crates/vox-orchestrator/src/queue/mod.rs`](../../../crates/vox-orchestrator/src/queue/mod.rs) — `AgentQueue`
- [`crates/vox-orchestrator/src/queue/drain.rs`](../../../crates/vox-orchestrator/src/queue/drain.rs) — drain semantics
- [`crates/vox-orchestrator/src/queue/priority.rs`](../../../crates/vox-orchestrator/src/queue/priority.rs) — priority pop
- [`crates/vox-orchestrator/src/scope.rs`](../../../crates/vox-orchestrator/src/scope.rs) — `ScopeGuard`
- [`crates/vox-orchestrator/src/workspace.rs`](../../../crates/vox-orchestrator/src/workspace.rs) — `AgentWorkspace`
- [`crates/vox-orchestrator/src/events.rs`](../../../crates/vox-orchestrator/src/events.rs) — `AgentEvent` variants
- [`crates/vox-orchestrator-types/`](../../../crates/vox-orchestrator-types/) — `AgentTask`, `TaskPriority`, capability tokens
- [`crates/vox-dashboard/`](../../../crates/vox-dashboard/) — Phase 2/3 surface
- [`crates/vox-orchestrator-mcp/`](../../../crates/vox-orchestrator-mcp/) — MCP tool layer

## Appendix C — Related Architecture Docs

- [Populi Mesh North-Star (2026-05-01)](populi-mesh-north-star-2026.md) — mesh dispatch
  authority, lease semantics, S1/S2/S3 sequencing.
- [Agentic Version Control Automation Research (2026-05-08)](agentic-version-control-automation-research-2026.md) — capability
  tokens, `@vcs.*` decorators, `GitExec` policy.
- [Multi-Agent VCS Replication Spec (2026-05-03)](multi-agent-vcs-replication-spec-2026.md) — `OpFragment`,
  `ConvergenceEngine`, `MergePolicyV1`.
- [Next-Generation AI Orchestrator Research (2026-04-23)](nextgen-orchestrator-research-2026.md) — design
  patterns, FinOps, multi-agent coherence.
- [Dashboard Migration Research (2026)](dashboard-migration-research-2026.md) — operator's harness, panel
  conventions.

## Appendix D — Memory & Doc Drift Notes

While preparing this document, two pieces of repository state were verified against memory:

- **`docs/src/architecture/mesh-and-language-distribution-ssot-2026.md` does not exist** as of
  this writing. Memory indexed it as the canonical mesh SSOT; the actual canonical file is
  [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md). The memory pointer should
  be updated to reflect this. This document cites the populi-mesh-north-star file directly.
- The agentic-VCS Phase 1 commits (`3e294c1a5..7ca219d90`) have shipped;
  capability tokens, `GitExec`, secret scanner, and `vox.vcs.*` telemetry contract are present.
  The hopper recommendation assumes these guarantees are real — re-verify before implementation.
