---
title: "ADR-042: Extract NodeRecord into vox-populi-types (L2)"
description: "NodeRecord cannot live in vox-mesh-types (L0) because it depends on vox-repository::TaskCapabilityHints (L2). A new L2 crate resolves this without a layering violation."
last_updated: "2026-05-24"
category: "Architecture SSOTs"
status: "current"
---

# ADR-042: Extract NodeRecord into vox-populi-types (L2)

**Status:** Accepted — implemented 2026-05-24.  
**Supersedes:** D-7 (workspace audit 2026-05-23) — original suggestion to move to L0 was wrong.

---

## Context

`NodeRecord` is a 28-field pure-data struct that describes a compute node in
the Populi mesh: GPU/CPU capabilities, security keys (Ed25519), scheduling
metadata, model advertisements, and donation policy.

It was originally defined in `vox-populi/src/node_registry.rs` alongside the
`LocalRegistry` persistence layer (the two were entangled). The workspace audit
flagged the entanglement as a D-series concern (topology type in a L3 crate).

The original D-7 proposal was to move `NodeRecord` to `vox-mesh-types` (L0) to
allow `vox-orchestrator` and other mid-layer crates to depend on it without
pulling in all of `vox-populi`. However, `NodeRecord` contains:

```rust
pub capabilities: vox_repository::TaskCapabilityHints,
```

`vox-repository` is an **L2** crate (repo catalog, host probing). An L0 crate
cannot depend on an L2 crate; the layer check would fail with a C2/C3 violation.

---

## Decision

Create a new crate **`vox-populi-types`** at **L2**, sitting alongside
`vox-repository` (both L2, no fan-in between them).

Layer placement:
```
L0: vox-mesh-types   (pure mesh protocol — no repo deps)
L2: vox-repository   (host probing, repo catalog)
L2: vox-populi-types (NodeRecord + task capability hints composition)  ← new
L3: vox-populi       (file I/O, local registry, mesh client)
L4: vox-plugin-populi-mesh (dispatch, API handlers)
```

`vox-populi-types` depends on:
- `vox-repository` (for `TaskCapabilityHints`)
- `vox-mesh-types` (for `ModelAdvertisement`, `WorkerDonationPolicy`)
- `serde`, `serde_json` (serialization)

`vox-populi-types` does **NOT** depend on:
- `vox-db`, `tokio`, `anyhow`, or any async/DB crate
- `vox-orchestrator` or any L3+ crate

---

## Consequences

**Positive:**
- `vox-orchestrator` (L3) can import `NodeRecord` directly from `vox-populi-types`
  (L2) rather than the full `vox-populi` (L3) crate. This avoids a same-layer
  circular dependency risk and keeps types accessible to the routing engine.
- `vox-plugin-populi-mesh` can depend on `vox-populi-types` directly, removing
  a redundant path through the full `vox-populi` crate for type-only imports.
- The LoC budget in `vox-populi` is reduced by extracting the pure-data struct.

**Negative / risks:**
- One more crate in the workspace (+1 to the total count).
- `vox-populi` still re-exports `NodeRecord` from `vox-populi-types` for
  backwards compatibility; callers can migrate at their own pace.

---

## Rejected Alternatives

| Alternative | Reason rejected |
|-------------|----------------|
| Move `NodeRecord` to `vox-mesh-types` (L0) | Violates layering: `TaskCapabilityHints` is L2 |
| Keep `NodeRecord` in `vox-populi` | No semantic issue, but forces L3 dependency for type-only consumers |
| Inline `TaskCapabilityHints` fields into `NodeRecord` | Breaks SSOT; would duplicate host-probe logic |
| Create `vox-populi-types` at L0 using only L0-compatible capability fields | Lossy — VRAM, arch, labels, routing tier are not in vox-mesh-types |
