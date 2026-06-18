---
title: "Gamified Telemetry & Contributions Design Spec"
description: "Architectural design for rewarding telemetry sharing, skill creation, peer sharing, and Vox compiler achievements."
category: "research"
status: "current"
training_eligible: true
training_rationale: "Defines event routing, triggers, and testing strategy for gamified community contribution metrics."
---

# Gamified Telemetry & Contributions Design Spec (2026)

This specification details the architecture and integrations for **Option 1 (Event-Centric Telemetry Routing)**, rewarding users for helping the Vox community and developer ecosystem.

---

## 1. Overview & Goals

The goal is to reward positive community behaviors and language milestones in the gamification system (`crates/vox-gamify`). We define four new core event categories:
1.  **`"telemetry_shared"`**: Triggered when a user successfully shares performance, crash, or model logs to help build community insights.
2.  **`"skill_published"`**: Triggered when a new skill package or MCP tool is registered locally in the catalog.
3.  **`"skill_gossiped"`**: Triggered when a skill or visual skin is distributed over the Populi peer-to-peer mesh.
4.  **`"vox_feature_milestone"`**: Triggered when compiler compilation succeeds using advanced Vox primitives.

---

## 2. Architecture & Data Flow

```
   Compiler / Telemetry / Populi Mesh
                   │
                   ▼ (Event Trigger)
      event_router::route_event()
                   │
                   ▼ (Multiplier / Cooldown / Mode adjustments)
      reward_policy::base_reward()
                   │
                   ▼
         VoxDb (Score / Lumens / XP updated)
```

1.  **Event Generation**: Trigger points in other crates construct and emit json payloads specifying the new event types.
2.  **Reward Routing**: The existing `vox-gamify` event router captures these events, applies mode multipliers (Balanced vs Learning), validates streaks, and performs grind deduplication.
3.  **Persistence**: The resulting XP, crystals, or lumens are written to the database (`vox_hardened.db`).

---

## 3. Detailed Component Designs

### 3.1 Crate: `vox-gamify` (Reward Engine)

We add the base rewards in `crates/vox-gamify/src/reward_policy.rs`:

```rust
// In base_reward lookup:
"telemetry_shared" => BaseReward::new(20, 4), // 20 XP, 4 crystals
"skill_published" => BaseReward::new(100, 20), // 100 XP, 20 crystals
"skill_gossiped" => BaseReward::with_lumens(150, 30, 15), // 150 XP, 30 crystals, 15 generosity lumens
"vox_feature_milestone" => BaseReward::new(40, 8), // 40 XP, 8 crystals
```

### 3.2 Crate: `vox-telemetry` (Telemetry Trigger)

When the telemetry client successfully uploads a log slice (and user consent is true), it calls the event router:

```rust
let ev = serde_json::json!({
    "type": "telemetry_shared",
    "source": "vox-telemetry",
    "payload": { "bytes_shared": byte_len },
});
crate::event_router::route_event(db, &user_id, &ev).await;
```

### 3.3 Crate: `vox-skills` (Skill Publish Trigger)

When `vox skill discover` or `vox-plugin-catalog` registers a newly created skill:

```rust
let ev = serde_json::json!({
    "type": "skill_published",
    "source": "vox-skills",
    "payload": { "skill_name": skill_name },
});
crate::event_router::route_event(db, &user_id, &ev).await;
```

### 3.4 Crate: `vox-populi` (Populi Mesh Gossip Trigger)

When gossip completes synchronization of a peer's shared asset:

```rust
let ev = serde_json::json!({
    "type": "skill_gossiped",
    "source": "vox-populi",
    "payload": { "peer_id": peer_id },
});
crate::event_router::route_event(db, &user_id, &ev).await;
```

### 3.5 Crate: `vox-compiler` (Language Milestones Trigger)

When the compiler typechecker successfully compiles a file containing `@remote`, `@durable`, or `actor` keywords:

```rust
let ev = serde_json::json!({
    "type": "vox_feature_milestone",
    "source": "vox-compiler",
    "payload": { "feature": feature_name },
});
crate::event_router::route_event(db, &user_id, &ev).await;
```

---

## 4. Testing & Verification

### 4.1 Unit Tests
*   **Reward Mapping Test**: Verify `base_reward()` returns the correct values for the four new types.
*   **Double-gossip Guard Test**: Test that `grind_taper_end` suppresses rewards if a user spams telemetry or gossips many skills consecutively.

### 4.2 Integration Tests
*   Run mock compilation and verify that typechecking a file with an `actor` triggers the `vox_feature_milestone` reward.
*   Mock a telemetry transmission and assert that `telemetry_shared` is written to `gamify_policy_snapshots` in the database.
