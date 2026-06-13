# HopperSync Mesh Replication — Design Spec (B4)

**Status:** design → implement
**Plan item:** B4 (wiring remediation, `docs/superpowers/plans/2026-06-07-wiring-remediation.md`)
**Depends on:** B3 (landed — `InMemoryHopper` + `HopperIntake` trait + daemon intake surface)

## Problem

`crates/vox-orchestrator/src/hopper/mesh_adapter.rs` defines `HopperOpSync` and the
`MIN_INTAKE_TRUST_TIER` constant, and `OpFragmentKind::HopperSync`
(`crates/vox-mesh-types/src/op_fragment.rs:91`) is a declared envelope kind — but
**nothing decodes, verifies, or applies a `HopperSync` op.** Cross-daemon hopper
replication is documented in the module's ASCII diagram (lines 11-19) and is otherwise
absent. The module's own docstring (lines 23-27) calls itself a stub.

## Verified current state

- `OpFragmentEnvelope` (`op_fragment.rs:19`) carries an untyped `object: serde_json::Value`
  and a `FederationSignature`. It exposes `canonical_signing_bytes()` (deterministic,
  signature blanked) but has **no** built-in sign/verify; the sign/verify pattern is
  ed25519-dalek over canonical bytes, demonstrated in
  `crates/vox-mesh-types/tests/federation_envelope.rs:68-97`.
- `HopperOpSync` (`mesh_adapter.rs:37`) has three variants: `ItemAdmitted`,
  `ItemOverridden`, `ItemTransitioned`. Only `ItemAdmitted` is in scope here (see §Scope).
- `HopperIntake` (`store.rs:38`) has `submit/inbox/assigned/history/reprioritize/assign/complete`.
  There is **no** `replay_admitted`. `IntakeItem::new` (`types.rs:152`) mints a **random**
  `item_id` (uuid v4) — unusable for replication, which must preserve the origin's id.
- `IntakeSource` (`types.rs:24`) is `Developer | Agent | Webhook` — no mesh-origin variant.
- `TaskPriority` (`types/tasks.rs:44`) is `Background=0 | Normal=1 | Urgent=2` (`u8` repr).
- **There is NO inbound `OpFragmentEnvelope` transport anywhere.** Grep confirms the only
  references to `OpFragmentKind`/`OpFragmentEnvelope` outside the type definition are the
  `mesh_adapter.rs` doc comments. `vox-populi/src/transport/` handles the *A2A* envelope
  (`SignedA2AEnvelope`, a different type), not federation op-fragments.

## Scope boundary (the honest cut)

Building a live cross-daemon network receive loop for federation envelopes is a separate
P6 transport epic and is **out of scope**. What this task delivers is the complete,
unit-tested **verify → decode → apply** seam that such a transport will call — a real
function that fully applies an admitted item, not a no-op match arm:

```
(transport, OUT OF SCOPE) → apply_op_fragment(envelope, peer_vk, peer_trust_tier, hopper)
                                 ├─ verify Ed25519 sig over canonical_signing_bytes
                                 ├─ reject if peer_trust_tier < MIN_INTAKE_TRUST_TIER
                                 ├─ match envelope.kind { HopperSync => decode HopperOpSync }
                                 └─ HopperOpSync::ItemAdmitted => hopper.replay_admitted(..)
```

The seam is reachable, tested, and complete; it is simply not yet *called* by a network
loop, because no federation inbound transport exists. This is documented, not hidden — the
function is exported and a follow-up (P6 transport) wires the caller.

### Hp-T5 persistence decision

**In-memory replication first; persistent `hopper_inbox` table deferred to Hp-T5.**
`replay_admitted` applies to whatever `Arc<dyn HopperIntake>` the daemon holds (today
`InMemoryHopper`). Convergence works across live daemons; durability across restarts is a
separate concern that swaps the impl behind the trait (per `store.rs:7-9`). **No vox-db
migration in this task.**

### Op-variant scope

`ItemAdmitted` is implemented end-to-end. `ItemOverridden` / `ItemTransitioned` are **not**
applied yet: the dispatch returns an explicit, surfaced `ReplicationError::UnsupportedOpVariant`
(logged at warn) — never a silent no-op. (Override replication needs cross-node cap
semantics; transition replication needs FSM-merge rules — both follow-ups.)

## Design

### 1. `IntakeSource::Mesh { node_id }`
Add a variant marking replicated provenance (so a replayed item is auditably distinct from
a locally-submitted one). Update `as_str` → `"mesh"`.

### 2. `TaskPriority::from_u8`
Add `pub fn from_u8(v: u8) -> TaskPriority` (0→Background, 1→Normal, 2→Urgent, else Normal)
to map the wire `priority: u8` in `ItemAdmitted`.

### 3. `HopperIntake::replay_admitted`
```rust
/// Idempotently apply a remote ItemAdmitted. Preserves the origin item_id;
/// a second apply of the same item_id is a no-op (returns the existing item).
async fn replay_admitted(&self, op: AdmittedReplay) -> IntakeItem;
```
where `AdmittedReplay { item_id, classified_priority, submitted_at_micros, task_kind, origin_node_id }`
is a small struct decoded from `HopperOpSync::ItemAdmitted`. The `InMemoryHopper` impl:
locks `items`, returns the existing clone if `item_id` already present (idempotent), else
constructs an `IntakeItem` with the **given** item_id, `state: Inbox`,
`source: IntakeSource::Mesh { node_id: origin_node_id }`, `intent` = a deterministic
`"[mesh:{task_kind}] replicated from {origin_node_id}"`, `priority_source: Orchestrator`,
and emits no local `HopperItemAdmitted` bus event (replication is not a local admission).
Add a private `IntakeItem::from_replay(..)` constructor in `types.rs` (the only path that
sets a caller-provided `item_id`).

### 4. `apply_op_fragment` dispatch (in `mesh_adapter.rs`)
```rust
pub enum ReplicationError { Signature, Untrusted{tier:u8}, Decode(String), UnsupportedOpVariant(&'static str), WrongKind }

pub async fn apply_op_fragment(
    env: &OpFragmentEnvelope,
    peer_vk: &ed25519_dalek::VerifyingKey,
    peer_trust_tier: u8,
    hopper: &dyn HopperIntake,
) -> Result<IntakeItem, ReplicationError>;
```
Steps: (a) `env.kind` must be `HopperSync` else `WrongKind`; (b) verify the base64 sig in
`env.signature.signature_b64` against `env.canonical_signing_bytes()` with `peer_vk` →
`Signature` on failure; (c) `peer_trust_tier >= MIN_INTAKE_TRUST_TIER` else `Untrusted`;
(d) `serde_json::from_value::<HopperOpSync>(env.object.clone())` → `Decode`; (e) match:
`ItemAdmitted{..}` → build `AdmittedReplay` (map `priority` via `TaskPriority::from_u8`,
`admitted_at_unix_ms*1000` → micros) → `hopper.replay_admitted(..)`; `ItemOverridden|ItemTransitioned`
→ `UnsupportedOpVariant`.

Trust tier + verifying key are **inputs** supplied by the (future) transport/trust layer,
keeping `apply_op_fragment` pure and testable.

## Test plan (`mesh_adapter.rs` `#[cfg(test)]`, model on federation_envelope.rs)
1. **happy path** — sign an `OpFragmentEnvelope{kind:HopperSync, object: ItemAdmitted{..}}`
   with a generated key, `apply_op_fragment(.., tier=3, hopper)`, assert the item appears in
   `hopper.inbox()` with the **same** item_id, priority mapped, `source == Mesh`.
2. **idempotent** — apply the same envelope twice → `inbox().len() == 1`.
3. **untrusted** — `tier=2` → `Err(Untrusted)`, hopper unchanged.
4. **bad signature** — tamper `object` after signing → `Err(Signature)`.
5. **wrong kind** — `kind: TaskResult` → `Err(WrongKind)`.
6. **unsupported variant** — `ItemTransitioned` → `Err(UnsupportedOpVariant)`.

## Out of scope (explicit, with follow-ups)
- Live federation inbound transport / gossip receive loop (P6 transport epic) — the caller of `apply_op_fragment`.
- Persistent `hopper_inbox` table (Hp-T5).
- `ItemOverridden` / `ItemTransitioned` replication.
- Outbound emit side (Daemon A → envelope) — `submit` already emits the bus event; wrapping it into a signed envelope for publish belongs with the transport epic.

## Verification
`cargo test -p vox-orchestrator -p vox-mesh-types` · `cargo run -p vox-arch-check`. Windows: `cargo fmt -p vox-orchestrator`, never `cargo fmt --all`.
