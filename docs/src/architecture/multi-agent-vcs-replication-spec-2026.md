---
title: "Multi-Agent VCS Replication — Architecture Spec (2026-05-03)"
description: "Architecture spec for op-log gossip on top of jj-lib and the Populi mesh. Defines the AgentChange / OpFragment / Convergence Set primitives, the gossip wire protocol, the auto-merge / escalation policy, and the four-phase rollout (local multi-agent → conflict UX → mesh gossip → policy/safety). Implements Path 1 from the research findings."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Canonical architecture for how Vox eliminates manual merging across local multi-agent fleets and mesh peers. Names the new primitives (AgentChange, OpFragment, ConvergenceSet, MergePolicy), the wire protocol, and the four implementation phases."
sourced_at: "2026-05-03"
vox_relevance:
  - "vox-orchestrator: jj_backend (extend), a2a/dispatch (new OpFragment envelope), conflict_manager (new escalation rules)"
  - "vox-populi: transport for op-log gossip; new gossip topic"
  - "vox-git: gix bridge unchanged; remains git-interop boundary"
  - "vox-socrates-policy: new arbitration rule for ambiguous semantic merges"
  - "vox-secrets: signing keys for op-log fragments"
---

# Multi-Agent VCS Replication — Architecture Spec (2026-05-03)

> **Companion research:** [`multi-agent-vcs-replication-research-2026.md`](multi-agent-vcs-replication-research-2026.md). This spec implements Path 1 from that research.

## Premise and goal

Multiple AI coding agents (Claude Code instances, MENS workers) and humans, on one machine and across the Populi mesh, edit the same codebase concurrently. Today they isolate via per-task git worktrees and serialize back through PRs.

**Goal:** non-conflicting edits auto-converge across the entire fleet — local agents, local humans, mesh peers — with no manual merge step. Conflicts surface as first-class navigable artifacts, not as `<<<<<<<` markers in working trees.

**Non-goals:**

- Replacing git interop. External git remotes (GitHub, GitLab) remain reachable via [`vox-git`](../../../crates/vox-git/). Vox is the inner substrate; the git wire protocol is preserved at the repo boundary.
- Real-time keystroke-level co-editing. We replicate **agent-commit-granularity ops**, not keystrokes. Use Yjs/CRDT editing in the IDE if that's wanted; it's out of scope here.
- Cross-organization federation. The mesh layer assumes a Populi-trust boundary (vox-secrets-issued identities, JWE-encrypted envelopes); cross-org sync is a future concern.

## Decisions baked into this spec

- **jj-lib stays the storage layer.** `jj_backend.rs` is extended, not replaced. No pivot to Pijul. See research §Recommendation.
- **Worktree-per-agent stays for now.** The migration to jj-workspace-per-agent is deferred to Phase 5+ (out of scope for this spec). Phases 1–4 work on top of the existing worktree pattern.
- **Op-log fragments, not snapshots, are the unit of exchange.** Smaller payloads, native to jj's model, preserves causality.
- **Populi is the transport.** The existing `a2a/dispatch/mesh.rs` envelope format is extended with an `OpFragment` variant. We do not introduce a parallel transport stack.
- **Iroh is evaluated in Phase 3, not adopted up front.** Populi suffices for v1; Iroh is the upgrade path if Populi's HTTP relay becomes a bottleneck or if NAT-traversal limits emerge. We keep the option open by keeping the gossip protocol transport-agnostic.
- **Patch-theory commutativity rules** (from Pijul) inform the auto-merge classifier without taking a project dependency.

---

## Architecture overview

```
┌──────────────────────────────────────────────────────────────────────┐
│ Local user box                                                       │
│                                                                      │
│   [Claude Tab A]  [Claude Tab B]  [MENS worker]   [Human IDE]        │
│        │              │                │                │            │
│        ▼              ▼                ▼                ▼            │
│   ┌─────────────────────────────────────────────────────────────┐   │
│   │ vox-orchestrator                                            │   │
│   │                                                             │   │
│   │   ┌──────────────────┐    ┌────────────────────────────┐    │   │
│   │   │  jj_backend.rs   │◄──►│  ConvergenceEngine (NEW)   │    │   │
│   │   │  (storage)       │    │  (auto-merge classifier)   │    │   │
│   │   └──────────────────┘    └─────────────┬──────────────┘    │   │
│   │                                          │                   │   │
│   │   ┌──────────────────┐    ┌─────────────▼──────────────┐    │   │
│   │   │ conflict_manager │◄───┤  MergePolicy (NEW)         │    │   │
│   │   │ (existing)       │    │  - patch commutativity     │    │   │
│   │   └──────────────────┘    │  - semantic guards         │    │   │
│   │                            │  - socrates arbitration    │    │   │
│   │   ┌──────────────────┐    └────────────────────────────┘    │   │
│   │   │ a2a/dispatch     │◄────► OpFragment envelope (NEW)      │   │
│   │   │ (mesh transport) │                                       │   │
│   │   └────────┬─────────┘                                       │   │
│   └────────────┼───────────────────────────────────────────────┘   │
│                │                                                     │
└────────────────┼─────────────────────────────────────────────────────┘
                 │   Populi mesh (HTTP relay; QUIC/Iroh in Phase 3+)
                 ▼
        Other peers (humans + agents on other boxes)
```

### New primitives

| Name | Purpose | Lives in |
|---|---|---|
| `AgentChange` | jj change ID owned by exactly one agent at any time. Replaces "this agent's branch" as the unit of work. | `vox-orchestrator/src/jj_backend.rs` (new submodule `agent_change.rs`) |
| `OpFragment` | A single jj operation packaged for replay on a peer: parent op IDs, the operation payload, the agent's signature, the convergence set membership. | `vox-orchestrator/src/jj_backend.rs::op_fragment` (new) |
| `ConvergenceSet` | A logical "branch" — a set of agents that intend their work to converge. Replaces today's manual branching. Each user has at minimum a `local` set. | `vox-orchestrator/src/convergence/` (new module) |
| `MergePolicy` | Pure decision function: given two `OpFragment`s with overlapping file ranges, classify as auto-mergeable, escalate-to-conflict, or block-on-policy. | `vox-orchestrator/src/convergence/policy.rs` (new) |
| `ConvergenceEngine` | The runtime that ingests local commits and remote `OpFragment`s, applies `MergePolicy`, and either auto-converges or routes to `conflict_manager`. | `vox-orchestrator/src/convergence/engine.rs` (new) |

### Components extended (not new)

- **`jj_backend.rs`** — gains `OpFragment` serialization/deserialization, op-log replay-from-fragment.
- **`a2a/envelope.rs`** — add `OpFragmentEnvelope` variant to the existing envelope enum.
- **`a2a/dispatch/mesh.rs`** — add a `gossip_op_fragment` topic; reuses existing JWE encryption and idempotency keys.
- **`mcp_tools/vcs_tools/`** — `change_create` returns an `AgentChange` instead of a raw branch name; `conflicts_list` reports the new convergence-set-aware conflict shape.
- **`vox-socrates-policy`** — gains a new arbitration rule: when two agents propose ops that the `MergePolicy` classifies as semantically ambiguous (e.g., both rename the same symbol to different names), Socrates can arbitrate via hallucination-score weighting before falling back to human conflict.

---

## Data model

### `AgentChange`

```rust
// vox:skip
pub struct AgentChange {
    pub change_id: ChangeId,           // jj change ID
    pub agent_id: AgentId,             // owner; exclusive — only this agent appends
    pub convergence_set: ConvergenceSetId,
    pub parent_op_id: OpId,            // op that created this change
    pub created_at: Timestamp,
}
```

Invariant: at any moment, an `AgentChange` has exactly one writer. Cross-agent handoff requires an explicit `change_handoff` op.

### `OpFragment`

```rust
// vox:skip
pub struct OpFragment {
    pub op_id: OpId,                   // content hash of (parents, payload, agent_id)
    pub parent_op_ids: Vec<OpId>,      // for causal ordering across the mesh
    pub agent_id: AgentId,             // who produced this op
    pub convergence_set: ConvergenceSetId,
    pub payload: OpPayload,            // jj-lib operation: snapshot, edit, abandon, ...
    pub signature: Signature,          // vox-secrets-issued; binds op_id to agent_id
    pub produced_at: Timestamp,
}

pub enum OpPayload {
    Snapshot { tree_id: TreeId, commit_id: CommitId, ... },
    Edit     { change_id: ChangeId, ... },
    Abandon  { change_id: ChangeId, ... },
    Squash   { source: ChangeId, dest: ChangeId, ... },
    Handoff  { change_id: ChangeId, from: AgentId, to: AgentId },
    // ...mirrors jj-lib's operation kinds
}
```

`op_id` is a content hash → identical ops dedupe naturally. `parent_op_ids` is a vector (not just one) so we preserve causal DAG semantics for ops produced concurrently across peers.

### `ConvergenceSet`

```rust
// vox:skip
pub struct ConvergenceSet {
    pub id: ConvergenceSetId,          // e.g., "local", "feature/auth-rewrite", "mesh:populi-org"
    pub members: Vec<AgentId>,         // explicit; not implicit from peer connectivity
    pub merge_policy: MergePolicyId,   // which policy applies inside this set
    pub upstream: Option<ConvergenceSetId>,  // optional parent for hierarchical convergence
}
```

A user's default set is `local` (all their agents on their machine). Joining a mesh-shared set is an explicit action. This is the new "branching" model — sets, not refs.

---

## Wire protocol

### Gossip, not pull

Each peer **streams `OpFragment`s** on its outbound channel as soon as they're produced and signed. Peers receiving fragments:

1. Verify the signature against vox-secrets-issued agent identities.
2. Check causal parents: if any `parent_op_id` is unknown, queue the fragment and request the missing ancestors.
3. Deduplicate by `op_id`.
4. Hand to `ConvergenceEngine` for replay + merge classification.

This is gossip-style eventual consistency, transport-agnostic. The Populi mesh provides ordered-per-peer delivery and JWE encryption; the protocol does not require it.

### Envelope shape (additive change to A2A)

```rust
// vox:skip
// In crates/vox-orchestrator/src/a2a/envelope.rs:
pub enum A2AMessage {
    // ... existing variants ...
    OpFragment(OpFragmentEnvelope),
    OpFragmentRequest(OpFragmentRequest),  // for backfill
    ConvergenceSetAnnouncement(ConvergenceSetAnnouncement),
}
```

Reuses the existing JWE encryption ([`a2a/jwe.rs`](../../../crates/vox-orchestrator/src/a2a/jwe.rs)), idempotency keys, and durability store ([`populi-mesh-a2a-durability-spec-2026.md`](populi-mesh-a2a-durability-spec-2026.md) — superseded but the VoxDb backing still applies).

### Backfill

A peer joining a convergence set mid-stream requests an op-log range starting from the most recent op it knows. Backfill is bounded: peers retain the last N ops in fast storage; older history is reconstructed from jj's normal op-store and replayed on demand.

---

## Merge classification (the auto-merge brain)

When two `OpFragment`s touch the same commit or file range, `MergePolicy` returns one of:

1. **Auto-merge** — patches commute (non-overlapping line ranges, distinct symbols, additive changes). Apply both; no human involved. Borrows from Pijul's patch theory: independent patches commute.
2. **Surface as conflict** — patches overlap and the bytes don't match. Materialize via `jj_backend.rs::ContentMerge::n_way` and route to the existing [`conflict_manager`](../../../crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs). Conflicts become first-class artifacts, not transient diffs in a working tree.
3. **Escalate to Socrates arbitration** — patches overlap but are semantically related (e.g., both rename the same symbol). [`vox-orchestrator-types::socrates_policy`](../../../crates/vox-orchestrator-types/src/socrates_policy/mod.rs) scores each side's hallucination risk + author trust and may auto-pick a winner; otherwise falls through to (2).
4. **Policy block** — the change violates a project rule (e.g., "agents can't edit `vox-secrets/src/spec.rs` without human review"). Hold the op; surface to a human.

The classifier is informed by:

- **Tree-sitter range overlap analysis** for code files. Two edits to the same function body but at non-overlapping byte ranges → check token-level overlap before declaring conflict.
- **Pijul-style patch commutativity** at the byte level for non-code files.
- **Semantic-Aware Replicated Data Type** rules (per ICSE 2025) for class/function-level operations: rename + rename of the same target = conflict; rename + add-call-site = auto-merge.

The classifier is pure (no I/O), making it cheap to test and audit.

---

## Phased rollout

### Phase 1 — Local multi-agent (4–6 weeks)

**Scope:** Two-plus Claude tabs / agents on one machine, one repo, ops gossiped between them via a local-only `ConvergenceSet`. No mesh, no remote.

**Deliverables:**

1. `AgentChange`, `OpFragment`, `ConvergenceSet`, `MergePolicy`, `ConvergenceEngine` types.
2. `jj_backend.rs` extension: `op_fragment::serialize` / `replay`.
3. Local `ConvergenceEngine` running inside `vox-orchestrator`, ingesting jj op-log writes and replaying ops from sibling agents.
4. `MergePolicy::v1` — byte-range overlap classifier; tree-sitter integration deferred.
5. `mcp_tools/vcs_tools/change_create` returns `AgentChange`; existing callers migrate.
6. Golden tests: 5-agent fixture, each adds non-overlapping functions to one file, all converge automatically; one fixture forces conflict and verifies it materializes.
7. Telemetry: `vox.convergence.*` span attributes (auto-merge / escalate / conflict counts).

**Success criterion:** With 5 Claude tabs editing one repo, ≥80% of edits auto-converge. Remaining 20% surface as named conflicts in `conflicts_list`.

### Phase 2 — Conflict UX (3–4 weeks)

**Scope:** Make the conflicts that *do* surface navigable. Without this, Phase 1's wins are invisible because users still drown in the conflicts that escape.

**Deliverables:**

1. New `vox vcs conflicts` CLI surface listing convergence-set conflicts, grouped by file and origin agent.
2. `vox vcs conflicts resolve <id>` — materializes the n-way merge, opens editor with markers, on save replays the resolution as a new op (preserving op-log lineage).
3. MCP tool surface: `conflicts_describe` (LLM-friendly conflict explanation: "agent A renamed `foo` to `bar`; agent B added a call site to `foo`").
4. Dashboard view (extends [`dashboard-migration-research-2026.md`](dashboard-migration-research-2026.md)) showing live convergence status across local agents.

**Success criterion:** Time-to-resolve a conflict drops by ≥50% vs. the git status quo (measured against a fixed conflict-corpus of recorded prior PR review threads).

### Phase 3 — Mesh gossip (4–6 weeks)

**Scope:** Extend the local protocol across the Populi mesh. Two users, single shared `ConvergenceSet`, op-fragments gossiped over `a2a/dispatch/mesh.rs`.

**Deliverables:**

1. `OpFragmentEnvelope` variant in `a2a/envelope.rs`.
2. Gossip topic + backfill protocol in `a2a/dispatch/mesh.rs`.
3. `ConvergenceSetAnnouncement` for set discovery.
4. Secrets-issued agent identities for op signing (extend [`crates/vox-secrets/`](../../../crates/vox-secrets/)).
5. Iroh evaluation: build a `Transport` trait so Populi or Iroh can be plugged in. Stay on Populi for v1; recommendation in a follow-up findings doc.

**Success criterion:** Two-user, two-agents-each (4 total) demo: all converge in real time across the mesh; no manual merge for non-overlapping work.

### Phase 4 — Policy / safety (3–4 weeks)

**Scope:** Socrates arbitration, project-rule policy blocks, audit trail.

**Deliverables:**

1. Socrates rule: hallucination-score-weighted arbitration for semantically ambiguous merges.
2. Project policy file (`Vox.toml [convergence.policy]`): file-glob rules, agent allowlists per path.
3. Op-log signing audit: `vox vcs audit` lists all auto-merges and arbitrations, with signer identity.
4. Rollback: `vox vcs op undo <op_id>` reverses a specific op across the convergence set, gossipped as a new "undo" op.

**Success criterion:** Audit trail covers 100% of auto-merged ops, attributable to signing agent. Policy blocks fire on the test fixture (forbidden-path edit by an agent without permission).

### Phase 5 (out of scope, named for completeness)

- jj-workspace-per-agent (retire `.claude/worktrees/` as the isolation primitive).
- Iroh as Populi-replacement transport, if measured contention warrants.
- Cross-organization federation (multi-Populi-trust convergence sets).
- Live keystroke-level CRDT layer underneath op-log for IDE-shared editing.

These are explicit follow-ups. They're not part of this spec because each requires its own design pass.

---

## Risk register

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| jj-lib 0.27 op-log API isn't stable enough to depend on | medium | high | Pin version; abstract behind `jj_backend.rs`; track jj upstream changelog as a dependency |
| `MergePolicy` mis-classifies a semantic conflict as auto-mergeable, corrupting code | medium | very high | Phase 1 ships byte-range-only (conservative); semantic rules gated behind explicit Socrates flag; always-on op-log lets `jj op undo` reverse any bad merge |
| Op-fragment volume saturates the Populi mesh | low (Phase 1–2), medium (Phase 3+) | medium | Local-first; Phase 3 measures actual bandwidth before committing transport choice |
| Multi-agent contention on the same `AgentChange` | low | medium | Single-writer invariant on `AgentChange`; cross-agent handoff is an explicit op |
| Worktree pattern fights the new model | medium | medium | Phase 1–4 work on top of worktrees; Phase 5+ migrates to jj-workspace-per-agent as a separate spec |
| Pijul-style commutativity rules are wrong for code | medium | high | Conservative defaults; opt-in for aggressive auto-merge; ICSE 2025 rules cited as upper-bound aspiration not v1 baseline |

## Open questions

- **Convergence-set membership UX.** How does a user discover and join a mesh-shared set? Punted to Phase 3 design; sketch only here.
- **Op-log retention.** How far back do we keep fast-replay storage? jj's defaults vs. our needs — measure in Phase 1.
- **Interaction with git remotes.** When does an auto-merged op become a git commit pushable to GitHub? Likely a periodic "publish" boundary controlled by the convergence-set policy. Detailed design deferred.

## Cross-references

- **Research foundation:** [`multi-agent-vcs-replication-research-2026.md`](multi-agent-vcs-replication-research-2026.md).
- **Mesh:** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), [`populi-mesh-improvement-backlog-2026.md`](populi-mesh-improvement-backlog-2026.md), [`populi-mesh-config-baseline-spec-2026.md`](populi-mesh-config-baseline-spec-2026.md).
- **Orchestrator context:** [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md).
- **Security / signing:** [`cryptography-ssot-2026.md`](cryptography-ssot-2026.md), `crates/vox-secrets/` for agent identity.
- **Code surfaces:** [`crates/vox-orchestrator/src/jj_backend.rs`](../../../crates/vox-orchestrator/src/jj_backend.rs), [`crates/vox-orchestrator/src/a2a/`](../../../crates/vox-orchestrator/src/a2a/), [`crates/vox-orchestrator/src/mcp_tools/vcs_tools/`](../../../crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs), [`crates/vox-git/`](../../../crates/vox-git/), [`crates/vox-orchestrator-types/src/socrates_policy/`](../../../crates/vox-orchestrator-types/src/socrates_policy/mod.rs).
- **Implementation plan:** [`multi-agent-vcs-replication-impl-plan-phase1-2026.md`](multi-agent-vcs-replication-impl-plan-phase1-2026.md) — Phase 1 step-by-step. Phases 2–4 will be drafted as separate plans when each is queued.
