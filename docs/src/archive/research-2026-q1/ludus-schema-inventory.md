---
title: "Ludus / gamify schema inventory (SSOT pointers)"
description: "Index to vox-db gamification SQL, agents domain, legacy Ludus hook, plus vox-ludus router/rewards/schema code and key tests—no duplicated schema text."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Ludus / gamify schema inventory (SSOT pointers)

## Baseline (vox-db manifest)

- Core tables: [`crates/vox-db/src/schema/domains/sql/gamification.sql`](../../../crates/vox-db/src/schema/domains/sql/gamification.sql) (profiles, companions, quests, battles) plus coordination SQL in the same domain.
- Agents / events: [`crates/vox-db/src/schema/domains/agents.rs`](../../../crates/vox-db/src/schema/domains/agents.rs) (`agent_events`, `cost_records`, …).

## Baseline gamification coordination (extended tables)

Extended Ludus tables and column fixes live in the **gamification** / coordination fragments under [`crates/vox-db/src/schema/domains/`](../../../crates/vox-db/src/schema/domains/) (consumed by `manifest::baseline_sql`). The former `ludus_schema_cutover` module and its legacy entrypoint are removed; use baseline `migrate` only.

Covers, among others:

- `gamify_teaching_profiles`, `gamify_policy_snapshots`, `gamify_ai_feedback`, `gamify_periodic_rewards`, `gamify_level_history`
- `gamify_counters` (**column `name`**, not `counter_name`)
- `gamify_collegium` (singular; legacy `gamify_collegiums` renamed when present)
- `gamify_arena_*`, `gamify_daily_counters`, `gamify_event_config`, `gamify_notifications`
- `gamify_hint_telemetry`, `gamify_processed_events` (orchestrator idempotency)
- Profile / quest / companion column alignment (`personality` on companions, streak/lumens on profiles, …)

## Application code

- Router + rewards: [`crates/vox-ludus/src/event_router.rs`](../../../crates/vox-ludus/src/event_router.rs), [`crates/vox-ludus/src/db/process_rewards.rs`](../../../crates/vox-ludus/src/db/process_rewards.rs)
- SQL reference ladder (documentation / partial migrations): [`crates/vox-ludus/src/schema.rs`](../../../crates/vox-ludus/src/schema.rs)

## Tests

- Ludus SQL / ops: [`crates/vox-db/tests/ops_ludus_tests.rs`](../../../crates/vox-db/tests/ops_ludus_tests.rs)
- Policy / router: [`crates/vox-ludus/tests/gamify_integration_test.rs`](../../../crates/vox-ludus/tests/gamify_integration_test.rs)

