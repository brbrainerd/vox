---
title: "ludus-adjudication-implementation-plan-2026"
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Ludus Adjudication System — Implementation Plan (2026)

> **Agent Directive:** This file is the single executable blueprint for implementing
> the Ludus dispute resolution, due-process adjudication, and reputation-gating system.
> Every step references a verified file path and function name. Do NOT invent paths.
> Mark completed items `- [x]`. Record all deviations in `## Deviations & State`.

---

## Architecture Overview

```
GitHub Event
    │
    ▼
sync_command()                          ← vox-cli/.../ludus/sync.rs
    │  try_claim_processed_event()      ← vox-gamify/src/db/dedupe.rs
    │  process_event_rewards()          ← vox-gamify/src/db/process_rewards.rs
    │      apply_policy()               ← vox-gamify/src/reward_policy.rs
    │      trust_tier_multiplier()      ← [NEW] vox-gamify/src/reward_policy.rs
    │      insert_policy_snapshot()     ← vox-gamify/src/db/teaching.rs
    ▼
gamify_profiles (VoxDb)                 ← vox-db/store/ops_ludus/gamify_world.rs
    ├── trust_tier   (V21 migration)
    ├── lumens / xp / crystals
    └── [SUPPRESSED flag]               ← [NEW] V22 migration

Dispute Lifecycle:
  file_dispute()    → gamify_disputes table      ← [NEW] vox-gamify/src/db/disputes.rs
  vote_on_dispute() → gamify_dispute_votes table ← [NEW] vox-gamify/src/db/disputes.rs
  tally_verdict()   → apply/dismiss penalty      ← [NEW] vox-gamify/src/db/disputes.rs
  appeal_dispute()  → re-opens, escalates        ← [NEW] vox-gamify/src/db/disputes.rs
```

### Key Verified Files (do not modify paths)

| Role | Path |
|---|---|
| TrustTier enum + LudusProfile | `crates/vox-gamify/src/profile.rs` |
| Schema migrations | `crates/vox-gamify/src/schema.rs` |
| DB profile get/upsert | `crates/vox-gamify/src/db/profile.rs` |
| DB module re-exports | `crates/vox-gamify/src/db/mod.rs` |
| Reward engine | `crates/vox-gamify/src/db/process_rewards.rs` |
| Policy engine | `crates/vox-gamify/src/reward_policy.rs` |
| Deduplication | `crates/vox-gamify/src/db/dedupe.rs` |
| Snapshot persistence | `crates/vox-gamify/src/db/teaching.rs` |
| GitHub sync CLI | `crates/vox-cli/src/commands/extras/ludus/sync.rs` |
| Identity context | `crates/vox-cli/src/commands/extras/ludus/ctx.rs` |
| Profile CLI commands | `crates/vox-cli/src/commands/extras/ludus/profile.rs` |
| Quests/notifications CLI | `crates/vox-cli/src/commands/extras/ludus/quests_notifications.rs` |
| Arena CLI | `crates/vox-cli/src/commands/extras/ludus/arena.rs` |
| VoxDb gamify world ops | `crates/vox-db/src/store/ops_ludus/gamify_world.rs` |
| VoxDb extended ops | `crates/vox-db/src/store/ops_ludus/gamify_extended.rs` |
| VoxDb misc ops | `crates/vox-db/src/store/ops_ludus/gamify_ludus_misc.rs` |
| VoxDb rewards/collegium | `crates/vox-db/src/store/ops_ludus/gamify_rewards_collegium.rs` |
| Research doc (security) | `docs/src/architecture/ludus-security-and-anti-cheat-research-2026.md` |
| Research index | Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`. |

### Migration Ladder (verified state)
- V19: `vox_identities` table
- V20: `gamify_policy_snapshots.metadata` column
- V21: `gamify_profiles.trust_tier` column  ← **current HEAD**
- V22–V25: **to be added by this plan**

### Trust Tier Values (verified in `profile.rs`)
```
TrustTier::Novice  = 0   (default, local-only)
TrustTier::Linked  = 1   (GitHub identity linked, auto-escalated in ctx.rs:75)
TrustTier::Proven  = 2   (10+ verified builds — NOT YET AUTO-GRANTED)
TrustTier::Master  = 3   (community-vouched — NOT YET AUTO-GRANTED)
```

---

## Phase 1 — Schema (V22–V25)

**Prerequisite:** None. Pure SQL. Append to `schema.rs` and `ALL_MIGRATIONS`.

### 1.1 V22: Suppression flag on profiles

- [x] MODIFY `crates/vox-gamify/src/schema.rs`:
  - Add after `SCHEMA_V21` constant:
  ```rust
  pub const SCHEMA_V22: &str = "
  ALTER TABLE gamify_profiles ADD COLUMN reward_suppressed INTEGER NOT NULL DEFAULT 0;
  ALTER TABLE gamify_profiles ADD COLUMN suppressed_until_ts INTEGER NOT NULL DEFAULT 0;
  ALTER TABLE gamify_profiles ADD COLUMN suppression_reason TEXT;
  ";
  ```
  - Add `("v22", SCHEMA_V22)` to `ALL_MIGRATIONS` slice.

### 1.2 V23: Dispute table

- [x] MODIFY `crates/vox-gamify/src/schema.rs`:
  - Add after `SCHEMA_V22`:
  ```rust
  pub const SCHEMA_V23: &str = "
  CREATE TABLE IF NOT EXISTS gamify_disputes (
      id TEXT PRIMARY KEY,
      accused_user_id TEXT NOT NULL,
      accuser_user_id TEXT NOT NULL,
      github_event_id TEXT,
      snapshot_id INTEGER,
      evidence_json TEXT NOT NULL,
      malice_score REAL NOT NULL DEFAULT 0.0,
      status TEXT NOT NULL DEFAULT 'pending',
      -- pending | under_review | guilty | innocent | appealed | dismissed
      created_at INTEGER NOT NULL,
      resolved_at INTEGER,
      appeal_deadline_ts INTEGER NOT NULL,
      penalty_applied INTEGER NOT NULL DEFAULT 0
  );
  CREATE INDEX IF NOT EXISTS idx_gamify_disputes_accused ON gamify_disputes(accused_user_id);
  CREATE INDEX IF NOT EXISTS idx_gamify_disputes_status ON gamify_disputes(status);
  ";
  ```
  - Add `("v23", SCHEMA_V23)` to `ALL_MIGRATIONS`.

### 1.3 V24: Dispute votes table

- [x] MODIFY `crates/vox-gamify/src/schema.rs`:
  - Add after `SCHEMA_V23`:
  ```rust
  pub const SCHEMA_V24: &str = "
  CREATE TABLE IF NOT EXISTS gamify_dispute_votes (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      dispute_id TEXT NOT NULL REFERENCES gamify_disputes(id),
      juror_user_id TEXT NOT NULL,
      verdict TEXT NOT NULL,
      -- 'guilty' | 'innocent'
      rationale TEXT,
      cast_at INTEGER NOT NULL,
      UNIQUE(dispute_id, juror_user_id)
  );
  CREATE INDEX IF NOT EXISTS idx_dispute_votes_dispute ON gamify_dispute_votes(dispute_id);
  CREATE INDEX IF NOT EXISTS idx_dispute_votes_juror ON gamify_dispute_votes(juror_user_id);
  ";
  ```
  - Add `("v24", SCHEMA_V24)` to `ALL_MIGRATIONS`.

### 1.4 V25: Juror pool assignment table

- [x] MODIFY `crates/vox-gamify/src/schema.rs`:
  - Add after `SCHEMA_V24`:
  ```rust
  pub const SCHEMA_V25: &str = "
  CREATE TABLE IF NOT EXISTS gamify_dispute_jury (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      dispute_id TEXT NOT NULL REFERENCES gamify_disputes(id),
      juror_user_id TEXT NOT NULL,
      assigned_at INTEGER NOT NULL,
      notified INTEGER NOT NULL DEFAULT 0,
      UNIQUE(dispute_id, juror_user_id)
  );
  ";
  ```
  - Add `("v25", SCHEMA_V25)` to `ALL_MIGRATIONS`.

### 1.5 Export V22–V25 from lib.rs

- [x] MODIFY `crates/vox-gamify/src/lib.rs`, line ~88-91:
  - Extend `pub use schema::{...}` to include `SCHEMA_V19, SCHEMA_V20, SCHEMA_V21, SCHEMA_V22, SCHEMA_V23, SCHEMA_V24, SCHEMA_V25`.
  - Also add `ALL_MIGRATIONS` if not already exported (currently it is NOT in lib.rs re-exports — verify before adding).

### 1.6 Verify build

- [x] RUN: `cargo check -p vox-gamify`

---

## Phase 2 — Database Operations (Dispute CRUD)

**Prerequisite:** Phase 1 complete. In this phase, we add the SQL operations to `vox-db` and the Rust wrappers to `vox-gamify/src/db/`.

### 2.1 VoxDb SQL Operations

- [x] CREATE `crates/vox-db/src/store/ops_ludus/gamify_disputes.rs` (or add to `gamify_extended.rs`):
  - `insert_gamify_dispute(...)`
  - `update_gamify_dispute_status(...)`
  - `insert_gamify_dispute_vote(...)`
  - `get_gamify_disputes_by_status(...)`
- [x] MODIFY `crates/vox-db/src/store/ops_ludus/gamify_world.rs`:
  - Update `upsert_gamify_profile` and `get_gamify_profile_raw` to handle `reward_suppressed`, `suppressed_until_ts`, and `suppression_reason`.

### 2.2 Ludus DB Wrappers

- [x] CREATE `crates/vox-gamify/src/db/disputes.rs`:
  - `file_dispute(db, accused, accuser, evidence)`
  - `cast_vote(db, dispute_id, juror, verdict, rationale)`
  - `assign_jury(db, dispute_id, juror_ids)`
- [x] MODIFY `crates/vox-gamify/src/db/mod.rs`:
  - `mod disputes;`
  - `pub use disputes::*;`
- [x] MODIFY `crates/vox-gamify/src/db/profile.rs`:
  - Update `get_profile` and `upsert_profile` to include the new suppression fields.

### 2.3 Verify Build

- [x] RUN: `cargo check -p vox-gamify`

---

## Phase 3 — Reward Policy Integration

**Prerequisite:** Phase 2 complete. Update the reward engine to multiply XP/Lumens based on Trust Tier and apply the suppression flag.

### 3.1 Reward Policy Modifications

- [x] MODIFY `crates/vox-gamify/src/reward_policy.rs`:
  - Add `pub fn trust_tier_multiplier(tier: TrustTier) -> f64`
    - `Novice` = 0.5
    - `Linked` = 1.0
    - `Proven` = 1.2
    - `Master` = 1.5
  - Update `apply_policy(...)` signature to accept `trust_tier: TrustTier`.
  - Apply the `trust_tier_multiplier` to XP and crystals in `apply_policy(...)`.

### 3.2 Reward Processing Hook

- [x] MODIFY `crates/vox-gamify/src/db/process_rewards.rs`:
  - If `profile.reward_suppressed` is true, immediately return `Ok(RouteResult::default())` to skip all rewards for that event.
  - Pass `profile.trust_tier.clone()` into the updated `apply_policy(...)` call.

### 3.3 Verify Build

- [x] RUN: `cargo check -p vox-gamify`

---

## Phase 4 — CLI Subcommands (Adjudication & Telemetry)

**Prerequisite:** Phase 3 complete. Add CLI commands so users can interact with the dispute system.

### 4.1 CLI Definitions

- [x] MODIFY `crates/vox-cli/src/commands/extras/ludus/mod.rs`:
  - Add the `Disputes` enum variant to `LudusCommands` with subcommands:
    - `File { target_user, event_id, rationale }`
    - `Vote { dispute_id, verdict, rationale }`
    - `Status { dispute_id }`
  - Add `mod disputes;` and route execution to `disputes::execute(...)`.

### 4.2 CLI Implementation

- [x] CREATE `crates/vox-cli/src/commands/extras/ludus/disputes.rs`:
  - Implement `execute()` routing for the new subcommands.
  - Call `vox_gamify::db::file_dispute(...)` in `File`.
  - Call `vox_gamify::db::cast_vote(...)` in `Vote`.

### 4.3 Profile CLI Update

- [x] MODIFY `crates/vox-cli/src/commands/extras/ludus/profile.rs`:
  - If `profile.reward_suppressed` is true, show a highly visible red warning banner in `vox ludus status`.

### 4.4 Verify Build

- [x] RUN: `cargo check -p vox-cli --features extras-ludus`

---

## Deviations & State

| Phase | Step | Original Plan | Actual Result | Agent Notes |
|---|---|---|---|---|
| — | — | — | — | Initialized. No deviations yet. |

