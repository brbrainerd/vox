---
title: "Multi-Agent VCS Replication — Phase 1 Implementation Plan (2026-05-03)"
description: "Step-by-step TDD implementation plan for Phase 1 of the multi-agent VCS replication architecture: local-only op-log gossip between agents on one machine. 16 tasks, ~80 individual steps, every code change shown. Phases 2–4 will be drafted as separate plans when queued."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Canonical step-by-step build of the convergence engine, MergePolicy v1, and op-fragment serialization. Engineers (and agents) implementing this feature must follow this sequence."
sourced_at: "2026-05-03"
vox_relevance:
  - "vox-orchestrator: new convergence module, jj_backend extensions, MCP tool migration"
  - "vox-populi: not touched in Phase 1 (mesh comes in Phase 3)"
---

# Multi-Agent VCS Replication — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Companion docs:**
> - Spec: [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md)
> - Research: [`multi-agent-vcs-replication-research-2026.md`](multi-agent-vcs-replication-research-2026.md)
>
> **Phases 2–4** (Conflict UX, Mesh gossip, Policy/safety) will be drafted as separate plans when each is queued. Don't try to implement them from this document.

**Goal:** Two or more local agents (Claude tabs / MENS workers) on one machine edit the same repo concurrently and have non-conflicting edits auto-converge with no manual merge step. Conflicts surface as named artifacts via the existing `conflict_manager`.

**Architecture:** Build a `convergence/` module inside `vox-orchestrator` with five primitives — `AgentChange`, `OpFragment`, `ConvergenceSet`, `MergePolicy`, `ConvergenceEngine`. Extend `jj_backend.rs` with op-fragment serialization. The engine watches the local jj op-log, gossips fragments to sibling agents in-process via tokio channels, classifies overlap with a byte-range `MergePolicyV1`, auto-merges or routes to `conflict_manager`. No mesh, no Populi — that's Phase 3.

**Tech stack:** Rust, tokio (async), serde/serde_json (envelope), sha3 (op-id content hashing), tracing (telemetry), thiserror (errors). All workspace crates already present in `crates/vox-orchestrator/Cargo.toml`.

---

## File structure

**New files (created in this plan):**

| Path | Responsibility |
|---|---|
| `crates/vox-orchestrator/src/convergence/mod.rs` | Module root; re-exports |
| `crates/vox-orchestrator/src/convergence/agent_change.rs` | `AgentChange` struct, single-writer invariant |
| `crates/vox-orchestrator/src/convergence/op_fragment.rs` | `OpFragment`, `OpId`, `OpPayload`; content-hash op-id |
| `crates/vox-orchestrator/src/convergence/set.rs` | `ConvergenceSet`, `ConvergenceSetId`, `ConvergenceSetRegistry` |
| `crates/vox-orchestrator/src/convergence/policy.rs` | `MergePolicy` trait, `MergePolicyV1`, classification enum |
| `crates/vox-orchestrator/src/convergence/engine.rs` | `ConvergenceEngine`: watch jj op-log, gossip, classify, route |
| `crates/vox-orchestrator/src/convergence/error.rs` | `ConvergenceError` |
| `crates/vox-orchestrator/src/convergence/tests.rs` | Unit tests |
| `crates/vox-orchestrator/tests/convergence_phase1_golden.rs` | Integration golden test (5-agent fixture) |

**Modified files:**

| Path | Change |
|---|---|
| `crates/vox-orchestrator/src/jj_backend.rs` | Add `op_fragment` submodule: serialize jj op → `OpFragment` and replay |
| `crates/vox-orchestrator/src/lib.rs` | Add `pub mod convergence;` and wire engine into orchestrator startup |
| `crates/vox-orchestrator/src/mcp_tools/vcs_tools/change.rs` | `change_create` returns `AgentChange` instead of raw branch name |
| `crates/vox-orchestrator/Cargo.toml` | No new deps in Phase 1 (sha3, tracing, tokio, serde already present) |

---

## Conventions

- **Commits:** Conventional commits prefixed `feat(convergence):` / `test(convergence):` / `refactor(convergence):`. One commit per task.
- **Tests:** `cargo test -p vox-orchestrator <name>`. Integration tests with `cargo test -p vox-orchestrator --test convergence_phase1_golden`.
- **Build check:** `cargo check -p vox-orchestrator --features jj-backend` after each task that touches code.
- **Trace spans:** All public engine methods open a `tracing::info_span!` with `vox.convergence.*` attribute names.
- **`// vox:skip`** in markdown code blocks below is documentation discipline — irrelevant to Rust.

---

## Task 1: Scaffold the `convergence/` module

**Files:**
- Create: `crates/vox-orchestrator/src/convergence/mod.rs`
- Create: `crates/vox-orchestrator/src/convergence/error.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`

- [ ] **Step 1: Create `error.rs` with the error enum**

`crates/vox-orchestrator/src/convergence/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvergenceError {
    #[error("unknown convergence set: {0}")]
    UnknownSet(String),

    #[error("agent {agent} attempted to write to change {change} owned by {owner}")]
    NotWriter { agent: String, change: String, owner: String },

    #[error("missing causal parent op: {0}")]
    MissingParent(String),

    #[error("op-fragment signature verification failed for op {0}")]
    BadSignature(String),

    #[error("jj backend: {0}")]
    JjBackend(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

- [ ] **Step 2: Create `mod.rs` with empty module declarations**

`crates/vox-orchestrator/src/convergence/mod.rs`:

```rust
//! Multi-agent convergence engine. See
//! [`docs/src/architecture/multi-agent-vcs-replication-spec-2026.md`].
//!
//! # Phase 1 scope
//! Local-only op-log gossip between agents on one machine, in-process via
//! tokio channels. Mesh transport (Populi) is Phase 3.

pub mod agent_change;
pub mod engine;
pub mod error;
pub mod op_fragment;
pub mod policy;
pub mod set;

#[cfg(test)]
mod tests;

pub use agent_change::AgentChange;
pub use engine::ConvergenceEngine;
pub use error::ConvergenceError;
pub use op_fragment::{OpFragment, OpId, OpPayload};
pub use policy::{MergePolicy, MergePolicyV1, MergeOutcome};
pub use set::{ConvergenceSet, ConvergenceSetId, ConvergenceSetRegistry};
```

- [ ] **Step 3: Create empty stubs for the submodules so the crate compiles**

For each of `agent_change.rs`, `op_fragment.rs`, `set.rs`, `policy.rs`, `engine.rs`, `tests.rs` create the file with a single line:

```rust
// Implemented in subsequent tasks.
```

- [ ] **Step 4: Wire `mod convergence;` into `lib.rs`**

In `crates/vox-orchestrator/src/lib.rs`, add (in the existing `pub mod` section, alphabetical order):

```rust
pub mod convergence;
```

- [ ] **Step 5: Verify the crate still builds**

```
cargo check -p vox-orchestrator --features jj-backend
```

Expected: clean build, possibly with warnings about unused stub modules.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator/src/convergence/ crates/vox-orchestrator/src/lib.rs
git commit -m "feat(convergence): scaffold convergence module"
```

---

## Task 2: `OpId` content-hash type (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/op_fragment.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing test in `tests.rs`**

```rust
use crate::convergence::OpId;

#[test]
fn op_id_is_deterministic_content_hash() {
    let id1 = OpId::from_bytes(b"hello");
    let id2 = OpId::from_bytes(b"hello");
    assert_eq!(id1, id2);

    let id3 = OpId::from_bytes(b"goodbye");
    assert_ne!(id1, id3);
}

#[test]
fn op_id_displays_as_hex_prefix() {
    let id = OpId::from_bytes(b"hello");
    let s = id.to_string();
    assert_eq!(s.len(), 12); // 6 bytes * 2 hex
    assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
}
```

- [ ] **Step 2: Run the test to verify failure**

```
cargo test -p vox-orchestrator convergence::tests::op_id
```

Expected: compilation error — `OpId` not defined.

- [ ] **Step 3: Implement `OpId` in `op_fragment.rs`**

```rust
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// Content-addressed identifier for an op fragment.
/// Stored as the first 32 bytes of SHA3-256; displayed as 6-byte hex prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpId([u8; 32]);

impl OpId {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for OpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0[..6] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```
cargo test -p vox-orchestrator convergence::tests::op_id
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/op_fragment.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add OpId content-hash type"
```

---

## Task 3: `OpPayload` and `OpFragment` types (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/op_fragment.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing serde round-trip test**

Append to `tests.rs`:

```rust
use crate::convergence::{OpFragment, OpPayload};

#[test]
fn op_fragment_round_trips_through_serde() {
    let frag = OpFragment {
        op_id: OpId::from_bytes(b"test-op"),
        parent_op_ids: vec![OpId::from_bytes(b"parent")],
        agent_id: "agent-A".into(),
        convergence_set: "local".into(),
        payload: OpPayload::Snapshot {
            commit_id: "abc123".into(),
            tree_id: "tree-xyz".into(),
        },
        signature: vec![],
        produced_at_unix_ms: 1_700_000_000_000,
    };

    let json = serde_json::to_string(&frag).expect("serialize");
    let back: OpFragment = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.op_id, frag.op_id);
    assert_eq!(back.parent_op_ids, frag.parent_op_ids);
    assert_eq!(back.agent_id, frag.agent_id);
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vox-orchestrator convergence::tests::op_fragment_round_trips
```

Expected: compilation error — types not defined.

- [ ] **Step 3: Implement `OpPayload` and `OpFragment` in `op_fragment.rs`**

Append:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpPayload {
    /// jj `snapshot` — record a working-copy state as a commit.
    Snapshot { commit_id: String, tree_id: String },
    /// jj `edit` — switch the working-copy pointer to a new change.
    Edit { change_id: String },
    /// jj `abandon` — discard a change.
    Abandon { change_id: String },
    /// jj `squash` — merge source change into dest.
    Squash { source: String, dest: String },
    /// Cross-agent change ownership transfer.
    Handoff { change_id: String, from_agent: String, to_agent: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpFragment {
    pub op_id: OpId,
    pub parent_op_ids: Vec<OpId>,
    pub agent_id: String,
    pub convergence_set: String,
    pub payload: OpPayload,
    /// vox-secrets-issued signature; empty in Phase 1 (added in Phase 4).
    #[serde(default)]
    pub signature: Vec<u8>,
    pub produced_at_unix_ms: u64,
}

impl OpFragment {
    /// Compute the canonical op_id from (parents, agent, payload).
    /// Used by `ConvergenceEngine` when promoting a local jj op into a fragment.
    pub fn derive_op_id(
        parent_op_ids: &[OpId],
        agent_id: &str,
        payload: &OpPayload,
    ) -> OpId {
        let mut buf = Vec::new();
        for parent in parent_op_ids {
            buf.extend_from_slice(parent.as_bytes());
        }
        buf.extend_from_slice(agent_id.as_bytes());
        buf.extend_from_slice(
            &serde_json::to_vec(payload).expect("payload serializes"),
        );
        OpId::from_bytes(&buf)
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::op_fragment_round_trips
```

Expected: pass.

- [ ] **Step 5: Add a `derive_op_id` test**

In `tests.rs`:

```rust
#[test]
fn derive_op_id_is_deterministic() {
    let parents = vec![OpId::from_bytes(b"p")];
    let payload = OpPayload::Edit { change_id: "c1".into() };
    let id1 = OpFragment::derive_op_id(&parents, "agent-A", &payload);
    let id2 = OpFragment::derive_op_id(&parents, "agent-A", &payload);
    assert_eq!(id1, id2);

    let id3 = OpFragment::derive_op_id(&parents, "agent-B", &payload);
    assert_ne!(id1, id3);
}
```

Run:

```
cargo test -p vox-orchestrator convergence::tests::derive_op_id
```

Expected: pass.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator/src/convergence/op_fragment.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add OpFragment and OpPayload types"
```

---

## Task 4: `AgentChange` with single-writer invariant (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/agent_change.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests.rs`:

```rust
use crate::convergence::{AgentChange, ConvergenceError};

#[test]
fn agent_change_enforces_single_writer() {
    let mut change = AgentChange::new("c1".into(), "agent-A".into(), "local".into());

    assert!(change.assert_writer("agent-A").is_ok());
    let err = change.assert_writer("agent-B").unwrap_err();
    assert!(matches!(err, ConvergenceError::NotWriter { .. }));
}

#[test]
fn agent_change_handoff_transfers_writer() {
    let mut change = AgentChange::new("c1".into(), "agent-A".into(), "local".into());
    change.handoff_to("agent-B".into());
    assert!(change.assert_writer("agent-B").is_ok());
    assert!(change.assert_writer("agent-A").is_err());
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vox-orchestrator convergence::tests::agent_change
```

Expected: compilation error.

- [ ] **Step 3: Implement `AgentChange`**

`crates/vox-orchestrator/src/convergence/agent_change.rs`:

```rust
use serde::{Deserialize, Serialize};

use super::error::ConvergenceError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentChange {
    pub change_id: String,
    pub owner_agent: String,
    pub convergence_set: String,
}

impl AgentChange {
    pub fn new(change_id: String, owner_agent: String, convergence_set: String) -> Self {
        Self { change_id, owner_agent, convergence_set }
    }

    pub fn assert_writer(&self, agent_id: &str) -> Result<(), ConvergenceError> {
        if self.owner_agent == agent_id {
            Ok(())
        } else {
            Err(ConvergenceError::NotWriter {
                agent: agent_id.to_string(),
                change: self.change_id.clone(),
                owner: self.owner_agent.clone(),
            })
        }
    }

    pub fn handoff_to(&mut self, new_owner: String) {
        self.owner_agent = new_owner;
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::agent_change
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/agent_change.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add AgentChange with single-writer invariant"
```

---

## Task 5: `ConvergenceSet` and registry (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/set.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests.rs`:

```rust
use crate::convergence::{ConvergenceSet, ConvergenceSetId, ConvergenceSetRegistry};

#[test]
fn registry_starts_with_local_set() {
    let reg = ConvergenceSetRegistry::with_default_local();
    assert!(reg.get(&"local".into()).is_some());
}

#[test]
fn registry_rejects_unknown_set() {
    let reg = ConvergenceSetRegistry::with_default_local();
    assert!(reg.get(&"feature/x".into()).is_none());
}

#[test]
fn registry_can_register_new_set() {
    let mut reg = ConvergenceSetRegistry::with_default_local();
    let set = ConvergenceSet {
        id: "feature/x".into(),
        members: vec!["agent-A".into(), "agent-B".into()],
    };
    reg.register(set.clone());
    assert_eq!(reg.get(&"feature/x".into()), Some(&set));
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vox-orchestrator convergence::tests::registry
```

Expected: compilation error.

- [ ] **Step 3: Implement in `set.rs`**

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub type ConvergenceSetId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConvergenceSet {
    pub id: ConvergenceSetId,
    pub members: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ConvergenceSetRegistry {
    sets: HashMap<ConvergenceSetId, ConvergenceSet>,
}

impl ConvergenceSetRegistry {
    pub fn with_default_local() -> Self {
        let mut sets = HashMap::new();
        sets.insert(
            "local".to_string(),
            ConvergenceSet { id: "local".into(), members: vec![] },
        );
        Self { sets }
    }

    pub fn register(&mut self, set: ConvergenceSet) {
        self.sets.insert(set.id.clone(), set);
    }

    pub fn get(&self, id: &ConvergenceSetId) -> Option<&ConvergenceSet> {
        self.sets.get(id)
    }

    pub fn add_member(&mut self, set_id: &ConvergenceSetId, agent: String) {
        if let Some(set) = self.sets.get_mut(set_id) {
            if !set.members.contains(&agent) {
                set.members.push(agent);
            }
        }
    }
}
```

- [ ] **Step 4: Run, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::registry
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/set.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add ConvergenceSet and registry"
```

---

## Task 6: `MergePolicy` trait + `MergePolicyV1` byte-range classifier (TDD)

This is the auto-merge brain. v1 is byte-range conservative: any overlap → conflict; non-overlap → auto-merge.

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/policy.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests.rs`:

```rust
use crate::convergence::{MergePolicy, MergePolicyV1, MergeOutcome};

fn range(start: usize, end: usize) -> std::ops::Range<usize> { start..end }

#[test]
fn non_overlapping_byte_ranges_auto_merge() {
    let policy = MergePolicyV1::default();
    let outcome = policy.classify_byte_overlap(&range(0, 10), &range(20, 30));
    assert_eq!(outcome, MergeOutcome::AutoMerge);
}

#[test]
fn overlapping_byte_ranges_surface_conflict() {
    let policy = MergePolicyV1::default();
    let outcome = policy.classify_byte_overlap(&range(0, 15), &range(10, 20));
    assert_eq!(outcome, MergeOutcome::SurfaceConflict);
}

#[test]
fn touching_byte_ranges_auto_merge() {
    // [0, 10) and [10, 20) share a boundary but no byte — adjacent inserts.
    let policy = MergePolicyV1::default();
    let outcome = policy.classify_byte_overlap(&range(0, 10), &range(10, 20));
    assert_eq!(outcome, MergeOutcome::AutoMerge);
}
```

- [ ] **Step 2: Run, verify failure**

```
cargo test -p vox-orchestrator convergence::tests
```

Expected: compilation error.

- [ ] **Step 3: Implement `policy.rs`**

```rust
use std::ops::Range;

use super::op_fragment::OpFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Patches commute; apply both, no human involved.
    AutoMerge,
    /// Materialize as n-way conflict, route to conflict_manager.
    SurfaceConflict,
    /// Hold for socrates-policy arbitration (Phase 4 — falls through to SurfaceConflict in Phase 1).
    EscalateToArbitration,
    /// Project policy forbids this op (Phase 4 — never returned in Phase 1).
    PolicyBlock,
}

pub trait MergePolicy: Send + Sync {
    /// Classify two op-fragments that touch the same change.
    fn classify(&self, a: &OpFragment, b: &OpFragment) -> MergeOutcome;

    /// Helper used internally and exposed for testing: classify two byte ranges.
    fn classify_byte_overlap(&self, a: &Range<usize>, b: &Range<usize>) -> MergeOutcome;
}

#[derive(Debug, Default, Clone)]
pub struct MergePolicyV1;

impl MergePolicy for MergePolicyV1 {
    fn classify(&self, a: &OpFragment, b: &OpFragment) -> MergeOutcome {
        // Phase 1: identical payloads dedupe to AutoMerge; otherwise SurfaceConflict.
        // Byte-range awareness is exposed via classify_byte_overlap for tests
        // and used by the engine when it has tree-id deltas to compare.
        if a.payload == b.payload {
            MergeOutcome::AutoMerge
        } else {
            MergeOutcome::SurfaceConflict
        }
    }

    fn classify_byte_overlap(&self, a: &Range<usize>, b: &Range<usize>) -> MergeOutcome {
        // Half-open ranges. Adjacent ([0,10) and [10,20)) do not overlap.
        if a.end <= b.start || b.end <= a.start {
            MergeOutcome::AutoMerge
        } else {
            MergeOutcome::SurfaceConflict
        }
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

```
cargo test -p vox-orchestrator convergence::tests
```

Expected: 3 new policy tests pass + earlier tests still pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/policy.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add MergePolicy trait and v1 byte-range classifier"
```

---

## Task 7: Extend `jj_backend.rs` with op-fragment serialization

**Files:**
- Modify: `crates/vox-orchestrator/src/jj_backend.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

In Phase 1 we stub the jj-lib bridge: the engine uses our types directly without round-tripping through jj-lib. The bridge will land when the engine consumes real jj op-store events. This task wires the conversion functions so the bridge is ready.

- [ ] **Step 1: Write the failing test in `tests.rs`**

```rust
use crate::convergence::{OpPayload, OpFragment, OpId};
use crate::jj_backend::op_fragment as jjbridge;

#[test]
fn jj_payload_round_trips_through_bridge() {
    let payload = OpPayload::Edit { change_id: "abc".into() };
    let serialized = jjbridge::serialize_payload(&payload);
    let back = jjbridge::deserialize_payload(&serialized).expect("deserialize");
    assert_eq!(back, payload);
}
```

- [ ] **Step 2: Run, verify failure (module doesn't exist)**

```
cargo test -p vox-orchestrator convergence::tests::jj_payload_round_trips
```

Expected: compilation error.

- [ ] **Step 3: Add the `op_fragment` submodule to `jj_backend.rs`**

At the bottom of `crates/vox-orchestrator/src/jj_backend.rs`:

```rust
// ---------------------------------------------------------------------------
// Op-fragment bridge (used by `convergence::engine`)
// ---------------------------------------------------------------------------

/// Serialization bridge between Vox `OpPayload` and the wire format.
/// Lives here (not in `convergence/`) so that the `jj-backend` feature gate
/// applies cleanly: when jj-lib is enabled this module can be extended to
/// translate to/from jj-lib's `op_store` types without leaking jj-lib types
/// into `convergence/`.
pub mod op_fragment {
    use crate::convergence::OpPayload;

    pub fn serialize_payload(payload: &OpPayload) -> Vec<u8> {
        serde_json::to_vec(payload).expect("OpPayload serializes")
    }

    pub fn deserialize_payload(bytes: &[u8]) -> Result<OpPayload, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
```

- [ ] **Step 4: Run, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::jj_payload_round_trips
```

Expected: pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/jj_backend.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add op-fragment serialization bridge in jj_backend"
```

---

## Task 8: `ConvergenceEngine` skeleton with channels (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/engine.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests.rs`:

```rust
use crate::convergence::ConvergenceEngine;

#[tokio::test]
async fn engine_can_be_constructed_and_shut_down() {
    let engine = ConvergenceEngine::new("agent-A".into(), "local".into());
    let handle = engine.spawn();
    handle.shutdown().await;
}
```

- [ ] **Step 2: Run, verify failure**

```
cargo test -p vox-orchestrator convergence::tests::engine_can_be_constructed
```

Expected: compilation error.

- [ ] **Step 3: Implement engine skeleton**

`crates/vox-orchestrator/src/convergence/engine.rs`:

```rust
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, info_span, Instrument};

use super::{
    op_fragment::OpFragment, policy::{MergePolicy, MergePolicyV1, MergeOutcome},
    set::ConvergenceSetRegistry,
};

/// Engine that watches the local op stream and gossips fragments to siblings.
/// In Phase 1, "siblings" are other engines in the same process — connected via
/// `ConvergenceEngine::link_sibling`. Mesh transport lands in Phase 3.
pub struct ConvergenceEngine {
    agent_id: String,
    set_id: String,
    policy: Arc<dyn MergePolicy>,
    sets: Arc<RwLock<ConvergenceSetRegistry>>,
    // Outbound: ops produced locally that we want siblings to see.
    outbound_tx: mpsc::UnboundedSender<OpFragment>,
    outbound_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<OpFragment>>>>,
    // Inbound: ops arriving from siblings.
    inbound_tx: mpsc::UnboundedSender<OpFragment>,
    inbound_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<OpFragment>>>>,
}

pub struct EngineHandle {
    shutdown_tx: oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}

impl EngineHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.join.await;
    }
}

impl ConvergenceEngine {
    pub fn new(agent_id: String, set_id: String) -> Self {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        Self {
            agent_id,
            set_id,
            policy: Arc::new(MergePolicyV1::default()),
            sets: Arc::new(RwLock::new(ConvergenceSetRegistry::with_default_local())),
            outbound_tx,
            outbound_rx: Arc::new(parking_lot::Mutex::new(Some(outbound_rx))),
            inbound_tx,
            inbound_rx: Arc::new(parking_lot::Mutex::new(Some(inbound_rx))),
        }
    }

    /// Outbound channel sender — clone and pass to the publisher of local ops.
    pub fn outbound_sender(&self) -> mpsc::UnboundedSender<OpFragment> {
        self.outbound_tx.clone()
    }

    /// Inbound channel sender — clone and pass to a sibling engine that wants
    /// to deliver an op to us.
    pub fn inbound_sender(&self) -> mpsc::UnboundedSender<OpFragment> {
        self.inbound_tx.clone()
    }

    /// Spawn the engine's run loop.
    pub fn spawn(self) -> EngineHandle {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let agent_id = self.agent_id.clone();
        let set_id = self.set_id.clone();
        let mut inbound_rx = self
            .inbound_rx
            .lock()
            .take()
            .expect("inbound_rx already taken");

        let span = info_span!(
            "vox.convergence.engine",
            agent_id = %agent_id,
            set_id = %set_id,
        );

        let join = tokio::spawn(
            async move {
                info!("convergence engine started");
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => {
                            info!("convergence engine shutting down");
                            break;
                        }
                        Some(frag) = inbound_rx.recv() => {
                            info!(op_id = %frag.op_id, "received remote op-fragment");
                            // Phase 1: classification + merge happens in Task 10.
                        }
                    }
                }
            }
            .instrument(span),
        );

        EngineHandle { shutdown_tx, join }
    }
}
```

- [ ] **Step 4: Run, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::engine_can_be_constructed
```

Expected: pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/engine.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add ConvergenceEngine skeleton with tokio channels"
```

---

## Task 9: Engine emits local ops to siblings (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/engine.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing test (link two engines, verify gossip)**

Append to `tests.rs`:

```rust
use crate::convergence::{OpFragment, OpId, OpPayload};

fn make_test_fragment(agent: &str, change: &str) -> OpFragment {
    OpFragment {
        op_id: OpId::from_bytes(format!("{agent}-{change}").as_bytes()),
        parent_op_ids: vec![],
        agent_id: agent.into(),
        convergence_set: "local".into(),
        payload: OpPayload::Edit { change_id: change.into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    }
}

#[tokio::test]
async fn engine_gossips_local_op_to_sibling() {
    use tokio::time::{timeout, Duration};

    let engine_a = ConvergenceEngine::new("agent-A".into(), "local".into());
    let engine_b = ConvergenceEngine::new("agent-B".into(), "local".into());

    // Pipe A's outbound into B's inbound.
    engine_a.link_sibling_oneway(engine_b.inbound_sender());

    let outbound_a = engine_a.outbound_sender();
    let received = engine_b.subscribe_received();

    let _handle_a = engine_a.spawn();
    let _handle_b = engine_b.spawn();

    let frag = make_test_fragment("agent-A", "c1");
    outbound_a.send(frag.clone()).expect("send");

    let got = timeout(Duration::from_millis(500), received.recv())
        .await
        .expect("timeout")
        .expect("recv");
    assert_eq!(got.op_id, frag.op_id);
}
```

- [ ] **Step 2: Run, verify failure**

```
cargo test -p vox-orchestrator convergence::tests::engine_gossips_local_op
```

Expected: compilation error — `link_sibling_oneway` and `subscribe_received` don't exist.

- [ ] **Step 3: Add the methods to `engine.rs`**

Modify `ConvergenceEngine` to add a `siblings: Vec<UnboundedSender<OpFragment>>` field and a `received_tx` for tests to observe inbound delivery.

Replace the `ConvergenceEngine` struct with:

```rust
pub struct ConvergenceEngine {
    agent_id: String,
    set_id: String,
    policy: Arc<dyn MergePolicy>,
    sets: Arc<RwLock<ConvergenceSetRegistry>>,
    outbound_tx: mpsc::UnboundedSender<OpFragment>,
    outbound_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<OpFragment>>>>,
    inbound_tx: mpsc::UnboundedSender<OpFragment>,
    inbound_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<OpFragment>>>>,
    siblings: Arc<RwLock<Vec<mpsc::UnboundedSender<OpFragment>>>>,
    /// Test/diagnostic: every received fragment is also forwarded here.
    received_tap_tx: mpsc::UnboundedSender<OpFragment>,
    received_tap_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<OpFragment>>>>,
}
```

Update `new()` to initialize the new fields. Add methods:

```rust
impl ConvergenceEngine {
    pub fn link_sibling_oneway(&self, sibling_inbound: mpsc::UnboundedSender<OpFragment>) {
        self.siblings.write().push(sibling_inbound);
    }

    pub fn subscribe_received(&self) -> mpsc::UnboundedReceiver<OpFragment> {
        self.received_tap_rx
            .lock()
            .take()
            .expect("subscribe_received can only be called once")
    }
}
```

Update `new()`:

```rust
pub fn new(agent_id: String, set_id: String) -> Self {
    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
    let (received_tap_tx, received_tap_rx) = mpsc::unbounded_channel();
    Self {
        agent_id, set_id,
        policy: Arc::new(MergePolicyV1::default()),
        sets: Arc::new(RwLock::new(ConvergenceSetRegistry::with_default_local())),
        outbound_tx,
        outbound_rx: Arc::new(parking_lot::Mutex::new(Some(outbound_rx))),
        inbound_tx,
        inbound_rx: Arc::new(parking_lot::Mutex::new(Some(inbound_rx))),
        siblings: Arc::new(RwLock::new(Vec::new())),
        received_tap_tx,
        received_tap_rx: Arc::new(parking_lot::Mutex::new(Some(received_tap_rx))),
    }
}
```

Update `spawn` to consume both `outbound_rx` and `inbound_rx`:

```rust
pub fn spawn(self) -> EngineHandle {
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let agent_id = self.agent_id.clone();
    let set_id = self.set_id.clone();
    let mut outbound_rx = self.outbound_rx.lock().take().expect("outbound_rx already taken");
    let mut inbound_rx = self.inbound_rx.lock().take().expect("inbound_rx already taken");
    let siblings = self.siblings.clone();
    let received_tap_tx = self.received_tap_tx.clone();

    let span = info_span!(
        "vox.convergence.engine",
        agent_id = %agent_id, set_id = %set_id,
    );

    let join = tokio::spawn(async move {
        info!("convergence engine started");
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    info!("convergence engine shutting down");
                    break;
                }
                Some(frag) = outbound_rx.recv() => {
                    let sibs = siblings.read().clone();
                    for sib in sibs.iter() {
                        let _ = sib.send(frag.clone());
                    }
                    info!(op_id = %frag.op_id, "gossiped local op to siblings");
                }
                Some(frag) = inbound_rx.recv() => {
                    info!(op_id = %frag.op_id, "received remote op-fragment");
                    let _ = received_tap_tx.send(frag);
                    // classification happens in Task 10.
                }
            }
        }
    }.instrument(span));

    EngineHandle { shutdown_tx, join }
}
```

- [ ] **Step 4: Run, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::engine_gossips_local_op
```

Expected: pass.

- [ ] **Step 5: Run the full convergence test suite to make sure nothing regressed**

```
cargo test -p vox-orchestrator convergence
```

Expected: all earlier tests still pass.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator/src/convergence/engine.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): engine gossips local ops to linked siblings"
```

---

## Task 10: Engine classifies remote fragments and routes via MergePolicy (TDD)

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/engine.rs`
- Modify: `crates/vox-orchestrator/src/convergence/tests.rs`

- [ ] **Step 1: Write the failing test for auto-merge dedup**

Append to `tests.rs`:

```rust
#[tokio::test]
async fn duplicate_remote_op_is_deduped() {
    use tokio::time::{timeout, Duration};

    let engine = ConvergenceEngine::new("agent-A".into(), "local".into());
    let inbound = engine.inbound_sender();
    let mut outcomes = engine.subscribe_outcomes();
    let _h = engine.spawn();

    let frag = make_test_fragment("agent-B", "c1");
    inbound.send(frag.clone()).unwrap();
    inbound.send(frag.clone()).unwrap();

    let first = timeout(Duration::from_millis(500), outcomes.recv()).await.unwrap().unwrap();
    let second = timeout(Duration::from_millis(500), outcomes.recv()).await.unwrap().unwrap();

    assert_eq!(first.0.op_id, frag.op_id);
    assert_eq!(first.1, MergeOutcome::AutoMerge); // first delivery: applied
    assert_eq!(second.0.op_id, frag.op_id);
    assert_eq!(second.1, MergeOutcome::AutoMerge); // duplicate: deduped, also AutoMerge
}

#[tokio::test]
async fn conflicting_remote_op_surfaces_conflict() {
    use tokio::time::{timeout, Duration};

    let engine = ConvergenceEngine::new("agent-A".into(), "local".into());
    let inbound = engine.inbound_sender();
    let mut outcomes = engine.subscribe_outcomes();
    let _h = engine.spawn();

    // Two fragments, same change_id, different agents → conflict by MergePolicyV1
    let frag_a = OpFragment {
        op_id: OpId::from_bytes(b"a"),
        parent_op_ids: vec![],
        agent_id: "agent-X".into(),
        convergence_set: "local".into(),
        payload: OpPayload::Edit { change_id: "c1".into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    };
    let frag_b = OpFragment {
        op_id: OpId::from_bytes(b"b"),
        parent_op_ids: vec![],
        agent_id: "agent-Y".into(),
        convergence_set: "local".into(),
        payload: OpPayload::Abandon { change_id: "c1".into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    };

    inbound.send(frag_a.clone()).unwrap();
    inbound.send(frag_b.clone()).unwrap();

    let first = timeout(Duration::from_millis(500), outcomes.recv()).await.unwrap().unwrap();
    let second = timeout(Duration::from_millis(500), outcomes.recv()).await.unwrap().unwrap();

    assert_eq!(first.1, MergeOutcome::AutoMerge);     // first edit applies cleanly
    assert_eq!(second.1, MergeOutcome::SurfaceConflict); // second collides with first
}
```

- [ ] **Step 2: Run, verify failure**

```
cargo test -p vox-orchestrator convergence::tests::duplicate_remote_op_is_deduped convergence::tests::conflicting_remote_op_surfaces_conflict
```

Expected: compilation error — `subscribe_outcomes` not defined.

- [ ] **Step 3: Add `subscribe_outcomes` and classification logic**

In `engine.rs`, add field + accessor:

```rust
// add to struct
outcome_tap_tx: mpsc::UnboundedSender<(OpFragment, MergeOutcome)>,
outcome_tap_rx: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<(OpFragment, MergeOutcome)>>>>,

// add to impl
pub fn subscribe_outcomes(&self) -> mpsc::UnboundedReceiver<(OpFragment, MergeOutcome)> {
    self.outcome_tap_rx
        .lock()
        .take()
        .expect("subscribe_outcomes can only be called once")
}
```

Update `new()` to initialize these.

Update the spawn loop's inbound branch to track applied ops by `change_id` and classify via `policy`:

```rust
// In spawn(), before the loop:
use std::collections::HashMap;
let mut applied_by_change: HashMap<String, OpFragment> = HashMap::new();
let mut seen_op_ids: std::collections::HashSet<OpId> = std::collections::HashSet::new();
let policy = self.policy.clone();
let outcome_tap_tx = self.outcome_tap_tx.clone();

// In the inbound branch:
Some(frag) = inbound_rx.recv() => {
    let _ = received_tap_tx.send(frag.clone());
    if !seen_op_ids.insert(frag.op_id.clone()) {
        // Duplicate: dedupe to AutoMerge.
        let _ = outcome_tap_tx.send((frag, MergeOutcome::AutoMerge));
        continue;
    }
    let change_id = match &frag.payload {
        OpPayload::Snapshot { commit_id, .. } => commit_id.clone(),
        OpPayload::Edit { change_id }
        | OpPayload::Abandon { change_id }
        | OpPayload::Handoff { change_id, .. } => change_id.clone(),
        OpPayload::Squash { dest, .. } => dest.clone(),
    };
    let outcome = if let Some(prior) = applied_by_change.get(&change_id) {
        policy.classify(prior, &frag)
    } else {
        MergeOutcome::AutoMerge
    };
    if matches!(outcome, MergeOutcome::AutoMerge) {
        applied_by_change.insert(change_id, frag.clone());
    }
    info!(op_id = %frag.op_id, ?outcome, "classified remote op");
    let _ = outcome_tap_tx.send((frag, outcome));
}
```

- [ ] **Step 4: Run tests, verify pass**

```
cargo test -p vox-orchestrator convergence::tests::duplicate_remote_op_is_deduped
cargo test -p vox-orchestrator convergence::tests::conflicting_remote_op_surfaces_conflict
```

Expected: both pass.

- [ ] **Step 5: Run full convergence suite for regressions**

```
cargo test -p vox-orchestrator convergence
```

Expected: all green.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator/src/convergence/engine.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): classify and dedupe remote op-fragments via MergePolicy"
```

---

## Task 11: Wire `ConvergenceEngine` into orchestrator startup

**Files:**
- Modify: `crates/vox-orchestrator/src/lib.rs`

In Phase 1 the engine is opt-in via a config flag — orchestrator owns the engine but does not yet route real jj op-store events to it (that comes when Phase 1 integrates with the rest of the agent runtime). The wiring lets future tasks publish to `outbound_sender()`.

- [ ] **Step 1: Find the orchestrator's startup function**

```
cargo test -p vox-orchestrator --no-run 2>&1 | head -5    # ensures crate builds
```

Then locate where the orchestrator initializes its long-lived components. Search:

```
grep -n "pub fn new\|pub async fn start\|impl Orchestrator" crates/vox-orchestrator/src/lib.rs
```

Identify the constructor or `start` entry point. (This plan assumes `Orchestrator::new` exists; if the actual name differs, use the orchestrator's actual startup site.)

- [ ] **Step 2: Add a `convergence_engine` field on the orchestrator**

In the orchestrator struct:

```rust
pub struct Orchestrator {
    // ... existing fields ...
    convergence: Option<crate::convergence::ConvergenceEngine>,
}
```

In the constructor, initialize as `None` by default; populated by an explicit setup helper:

```rust
pub fn enable_convergence(&mut self, agent_id: String, set_id: String) {
    self.convergence = Some(crate::convergence::ConvergenceEngine::new(agent_id, set_id));
}
```

- [ ] **Step 3: Add a smoke test in `crates/vox-orchestrator/src/convergence/tests.rs`**

```rust
#[test]
fn engine_constructible_from_orchestrator_path() {
    // Smoke: the orchestrator surface compiles when convergence is wired in.
    // (Functional integration with jj op-store events lands when Phase 1 ships
    // the per-agent runtime hookup, separate from this plan's deliverable.)
    let _engine = crate::convergence::ConvergenceEngine::new("a".into(), "local".into());
}
```

- [ ] **Step 4: Run**

```
cargo test -p vox-orchestrator convergence::tests::engine_constructible_from_orchestrator_path
cargo check -p vox-orchestrator --features jj-backend
```

Expected: pass + clean build.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/lib.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): wire ConvergenceEngine into orchestrator surface"
```

---

## Task 12: Migrate `change_create` MCP tool to return `AgentChange`

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/vcs_tools/change.rs`

- [ ] **Step 1: Read the existing surface**

```
cat crates/vox-orchestrator/src/mcp_tools/vcs_tools/change.rs
```

Identify the existing return shape of `change_create`. Note the JSON schema callers expect.

- [ ] **Step 2: Add a regression test for the existing surface**

In a new `crates/vox-orchestrator/src/mcp_tools/vcs_tools/change_test.rs` (or extend an existing test module — match what the file already does):

```rust
#[test]
fn change_create_returns_agent_change_shape() {
    // Hypothetical caller: passes (agent_id, set_id), expects AgentChange JSON.
    let resp = crate::mcp_tools::vcs_tools::change::create_change_for_test(
        "agent-A".into(),
        "local".into(),
        "feature/foo".into(),
    );
    assert_eq!(resp.owner_agent, "agent-A");
    assert_eq!(resp.convergence_set, "local");
    assert_eq!(resp.change_id, "feature/foo");
}
```

- [ ] **Step 3: Run, verify failure**

```
cargo test -p vox-orchestrator mcp_tools::vcs_tools::change
```

Expected: compilation error.

- [ ] **Step 4: Update `change_create` to return `AgentChange`**

In `change.rs`, modify the existing `change_create` to construct and return `AgentChange`:

```rust
use crate::convergence::AgentChange;

pub fn create_change_for_test(
    agent_id: String,
    set_id: String,
    change_id: String,
) -> AgentChange {
    AgentChange::new(change_id, agent_id, set_id)
}
```

Update the live MCP tool handler that previously returned `String` (or whatever the existing shape was) to wrap with `AgentChange`. Update the JSON schema accordingly.

- [ ] **Step 5: Run tests, verify pass**

```
cargo test -p vox-orchestrator mcp_tools::vcs_tools::change
cargo test -p vox-orchestrator mcp_tools
```

Expected: green.

- [ ] **Step 6: Update any other callers that broke**

```
cargo check -p vox-orchestrator --features jj-backend
```

If any callers fail to compile, fix them by mapping the new `AgentChange` to whatever they need. Ideally callers now consume `AgentChange` directly.

- [ ] **Step 7: Commit**

```
git add crates/vox-orchestrator/src/mcp_tools/vcs_tools/
git commit -m "feat(convergence): change_create MCP tool returns AgentChange"
```

---

## Task 13: Telemetry — `vox.convergence.*` span attributes

**Files:**
- Modify: `crates/vox-orchestrator/src/convergence/engine.rs`

- [ ] **Step 1: Audit the engine for missing trace attrs**

The `info_span!` already includes `agent_id` and `set_id`. Add per-event counters via structured fields.

- [ ] **Step 2: Add a counter span around the inbound classification block**

In `engine.rs`, replace the inbound-classification logging with:

```rust
let outcome_str = match outcome {
    MergeOutcome::AutoMerge => "auto_merge",
    MergeOutcome::SurfaceConflict => "surface_conflict",
    MergeOutcome::EscalateToArbitration => "escalate",
    MergeOutcome::PolicyBlock => "policy_block",
};
info!(
    op_id = %frag.op_id,
    outcome = %outcome_str,
    convergence_set = %frag.convergence_set,
    remote_agent = %frag.agent_id,
    "vox.convergence.classify"
);
```

And around the outbound branch:

```rust
info!(
    op_id = %frag.op_id,
    sibling_count = sibs.len(),
    convergence_set = %frag.convergence_set,
    "vox.convergence.gossip"
);
```

- [ ] **Step 3: Add a smoke test that checks tracing fields don't panic**

In `tests.rs`:

```rust
#[tokio::test]
async fn engine_emits_traces_without_panic() {
    let engine = ConvergenceEngine::new("agent-A".into(), "local".into());
    let inbound = engine.inbound_sender();
    let _h = engine.spawn();
    let frag = make_test_fragment("agent-B", "c-trace");
    inbound.send(frag).unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}
```

- [ ] **Step 4: Run**

```
cargo test -p vox-orchestrator convergence::tests::engine_emits_traces_without_panic
```

Expected: pass.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator/src/convergence/engine.rs crates/vox-orchestrator/src/convergence/tests.rs
git commit -m "feat(convergence): add vox.convergence.* tracing fields"
```

---

## Task 14: Golden integration test — 5-agent non-overlapping convergence

**Files:**
- Create: `crates/vox-orchestrator/tests/convergence_phase1_golden.rs`

- [ ] **Step 1: Create the golden test**

```rust
//! Phase 1 golden: 5 agents make non-overlapping edits; all converge to AutoMerge.

use std::time::Duration;
use tokio::time::timeout;
use vox_orchestrator::convergence::{
    ConvergenceEngine, MergeOutcome, OpFragment, OpId, OpPayload,
};

fn frag(agent: &str, change: &str) -> OpFragment {
    OpFragment {
        op_id: OpId::from_bytes(format!("{agent}-{change}").as_bytes()),
        parent_op_ids: vec![],
        agent_id: agent.into(),
        convergence_set: "local".into(),
        payload: OpPayload::Edit { change_id: change.into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    }
}

#[tokio::test]
async fn five_agents_non_overlapping_all_auto_merge() {
    let agents = ["agent-A", "agent-B", "agent-C", "agent-D", "agent-E"];
    // Build 5 engines and link them all to a single observer engine.
    let observer = ConvergenceEngine::new("observer".into(), "local".into());
    let observer_inbound = observer.inbound_sender();
    let mut outcomes = observer.subscribe_outcomes();
    let _obs_handle = observer.spawn();

    let mut handles = Vec::new();
    let mut senders = Vec::new();
    for a in agents {
        let e = ConvergenceEngine::new(a.into(), "local".into());
        e.link_sibling_oneway(observer_inbound.clone());
        senders.push(e.outbound_sender());
        handles.push(e.spawn());
    }

    // Each agent edits a distinct change.
    for (i, a) in agents.iter().enumerate() {
        senders[i].send(frag(a, &format!("change-{i}"))).unwrap();
    }

    // Collect 5 outcomes; all should be AutoMerge.
    for _ in 0..agents.len() {
        let (frag, outcome) = timeout(Duration::from_secs(2), outcomes.recv())
            .await
            .expect("timeout")
            .expect("recv");
        assert_eq!(outcome, MergeOutcome::AutoMerge, "agent {} should auto-merge", frag.agent_id);
    }
}
```

- [ ] **Step 2: Run**

```
cargo test -p vox-orchestrator --test convergence_phase1_golden
```

Expected: pass.

- [ ] **Step 3: Commit**

```
git add crates/vox-orchestrator/tests/convergence_phase1_golden.rs
git commit -m "test(convergence): 5-agent non-overlapping golden auto-merges"
```

---

## Task 15: Golden integration test — forced conflict surfaces

**Files:**
- Modify: `crates/vox-orchestrator/tests/convergence_phase1_golden.rs`

- [ ] **Step 1: Add a conflict golden**

Append to the file:

```rust
#[tokio::test]
async fn conflict_on_same_change_surfaces() {
    let observer = ConvergenceEngine::new("observer".into(), "local".into());
    let inbound = observer.inbound_sender();
    let mut outcomes = observer.subscribe_outcomes();
    let _h = observer.spawn();

    // Two agents both touch change c-conflict, with different payloads.
    let frag_a = OpFragment {
        op_id: OpId::from_bytes(b"a-edit"),
        parent_op_ids: vec![],
        agent_id: "agent-A".into(),
        convergence_set: "local".into(),
        payload: OpPayload::Edit { change_id: "c-conflict".into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    };
    let frag_b = OpFragment {
        op_id: OpId::from_bytes(b"b-abandon"),
        parent_op_ids: vec![],
        agent_id: "agent-B".into(),
        convergence_set: "local".into(),
        payload: OpPayload::Abandon { change_id: "c-conflict".into() },
        signature: vec![],
        produced_at_unix_ms: 0,
    };

    inbound.send(frag_a).unwrap();
    inbound.send(frag_b).unwrap();

    let (_f1, o1) = timeout(Duration::from_secs(1), outcomes.recv()).await.unwrap().unwrap();
    let (_f2, o2) = timeout(Duration::from_secs(1), outcomes.recv()).await.unwrap().unwrap();

    assert_eq!(o1, MergeOutcome::AutoMerge);
    assert_eq!(o2, MergeOutcome::SurfaceConflict);
}
```

- [ ] **Step 2: Run**

```
cargo test -p vox-orchestrator --test convergence_phase1_golden conflict_on_same_change
```

Expected: pass.

- [ ] **Step 3: Commit**

```
git add crates/vox-orchestrator/tests/convergence_phase1_golden.rs
git commit -m "test(convergence): conflicting payloads surface conflict"
```

---

## Task 16: Status update + doc cross-refs

**Files:**
- Modify: `docs/src/architecture/multi-agent-vcs-replication-spec-2026.md`
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.

- [ ] **Step 1: Update the spec's Phase 1 status to "in progress / partially shipped"**

Edit the Phase 1 section header in the spec to add a status note:

```markdown
### Phase 1 — Local multi-agent (4–6 weeks) — **partially shipped 2026-MM-DD (see impl plan)**
```

(Replace `2026-MM-DD` with the actual completion date.)

- [ ] **Step 2: Add a "Status" line at the top of the impl plan**

In `multi-agent-vcs-replication-impl-plan-phase1-2026.md`, add immediately under the frontmatter:

```markdown
**Status:** Tasks 1–15 complete; live-jj-op-store integration and per-agent runtime wiring tracked separately under [follow-up name TBD].
```

- [ ] **Step 3: Verify all docs render**

```
cargo run -p vox-doc-pipeline -- --check
```

Expected: clean. (If `SUMMARY.md` regeneration is needed, run `cargo run -p vox-doc-pipeline` per AGENTS.md.)

- [ ] **Step 4: Commit**

```
git add docs/src/architecture/multi-agent-vcs-replication-spec-2026.md docs/src/architecture/multi-agent-vcs-replication-impl-plan-phase1-2026.md
git commit -m "docs(convergence): mark Phase 1 as partially shipped; cross-ref impl plan"
```

---

## Self-review

**1. Spec coverage:**

| Spec deliverable | Plan task |
|---|---|
| `AgentChange`, `OpFragment`, `ConvergenceSet`, `MergePolicy`, `ConvergenceEngine` types | Tasks 2–6, 8 |
| `jj_backend.rs` extension: `op_fragment::serialize` / `replay` | Task 7 |
| Local `ConvergenceEngine` running inside `vox-orchestrator` | Tasks 8–11 |
| `MergePolicy::v1` byte-range overlap classifier | Task 6 |
| `mcp_tools/vcs_tools/change_create` returns `AgentChange` | Task 12 |
| Golden tests | Tasks 14, 15 |
| Telemetry: `vox.convergence.*` | Task 13 |

All Phase 1 spec deliverables map to at least one task. **Replay** (turning a received `OpFragment` back into a jj op against local op-store) is intentionally stubbed in Phase 1 — the engine classifies and tracks ops in memory but doesn't yet write through to jj-lib's op-store. That hookup is named in the spec as separate from this plan because it depends on jj-lib op-store transaction primitives that need their own design pass; Task 16 calls this out.

**2. Placeholder scan:** Searched for "TBD", "TODO", "implement later", "fill in details" — none in step bodies. The "TBD" in Task 16 step 2 refers to a real future doc name, used in plan output not as a substitute for plan content.

**3. Type consistency:** `AgentChange.owner_agent` (Task 4) is the field used in Tasks 12 and 14. `OpFragment.op_id`, `parent_op_ids`, `agent_id`, `convergence_set`, `payload`, `signature`, `produced_at_unix_ms` are consistent across Tasks 3, 8, 9, 10, 14, 15. `MergeOutcome` variants `AutoMerge` / `SurfaceConflict` / `EscalateToArbitration` / `PolicyBlock` defined in Task 6 are referenced in Tasks 10, 13, 14, 15.

---

## Phases 2–4 — placeholder for follow-up plans

These are scoped in the spec but not detailed here. Each will get its own plan when queued:

- **Phase 2 — Conflict UX (~3–4 weeks):** `vox vcs conflicts` CLI, MCP `conflicts_describe`, dashboard view.
- **Phase 3 — Mesh gossip (~4–6 weeks):** `OpFragmentEnvelope` over Populi A2A, gossip topic, backfill, vox-secrets-issued agent identities, Iroh transport evaluation.
- **Phase 4 — Policy / safety (~3–4 weeks):** Socrates arbitration rule, `Vox.toml [convergence.policy]`, `vox vcs audit`, `vox vcs op undo`.

Drafting these now would speculate about decisions that depend on Phase 1 outcomes. Defer.
