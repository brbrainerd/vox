---
title: "Vox Ludus integration contract (producers)"
description: "Producer contract: snake_case event types, route_event on Codex (not raw process_event_rewards), ludus_dedupe_id for idempotency, config/env/CLI/MCP surfaces, canonical_user_id, and PR checklist."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Vox Ludus integration contract (producers)

## Canonical event pipeline

1. Build a JSON object with a **snake_case** `type` field matching [`vox_ludus::reward_policy::base_reward`](../reference/cli.md) keys (aligned with `serde` `AgentEventKind` in the orchestrator).
2. Call **`vox_ludus::event_router::route_event`** (or **`route_event_auto_user`**) on [`vox_db::Codex`]. Do **not** call `process_event_rewards` directly from MCP/orchestrator sinks — the router owns daily counters, companion sync, Phoenix/shield rules, combos, and teaching hooks.
3. For MCP / long-running orchestrator sinks, inject **`ludus_dedupe_id`** (numeric) into the payload so `gamify_processed_events` can suppress replays.

## Configuration and optionality

| Mechanism | Purpose |
|-----------|---------|
| `VoxConfig.gamify_enabled` + `gamify_mode` (persisted via `vox ludus …`) | Primary on-disk toggle and mode |
| `VOX_GAMIFY_ENABLED`, `VOX_GAMIFY_MODE` | Env overrides (see vox-config) |
| `VOX_LUDUS_SESSION_ENABLED`, `VOX_LUDUS_SESSION_MODE` | Non-persistent session overlay |
| `VOX_LUDUS_EMERGENCY_OFF=1` | Hard kill-switch for all Ludus side effects |
| `VOX_LUDUS_VERBOSITY=quiet\|normal\|rich` | CLI celebration noise (`vox_cli` + `output_policy`) |
| `VOX_LUDUS_MAX_MESSAGES_PER_HOUR` | Rate cap for celebration-style CLI lines (default 12) |

## CLI surface (feature `extras-ludus`)

- `vox ludus enable` / `vox ludus disable` — persist on/off
- `vox ludus mode --set …` / `vox ludus mode --effective` — view or change mode
- `vox ludus metrics` — local KPI aggregates
- `vox ludus digest` — short session summary
- `vox ludus profile-merge` — copy synthetic `default` user row into `local_user_id` when local is empty

Latin alias: `vox ars ludus …` (same subcommands).

## User id (canonical vs local)

Use **`vox_ludus::db::canonical_user_id()`** for all Codex writes that participate in Ludus (profile, quests, notifications, policy snapshots, teaching). Do not mix raw `vox_db::paths::local_user_id()` on those paths or rows will split across identities.

## MCP tools (Codex-attached)

Canonical names live in [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml). Besides notifications and `vox_ludus_progress_snapshot`, the server may expose **`vox_ludus_quest_list`**, **`vox_ludus_shop_catalog`**, **`vox_ludus_shop_buy`**, **`vox_ludus_collegium_join`**, **`vox_ludus_battle_start`**, and **`vox_ludus_battle_submit`** (see `vox-mcp` `gamify` module).

| Env | Role |
|-----|------|
| `VOX_LUDUS_CHANNEL` | UX channel (`digest-priority`, etc.) |
| `VOX_LUDUS_MCP_TOOL_ARGS` | `full` / `hash` / `omit` for MCP tool args in routed events |
| `VOX_LUDUS_EXPERIMENT` | A/B label + hint frequency multiplier |
| `VOX_LUDUS_EXPERIMENT_REWARD_MULT` | Optional extra multiplier on policy XP/crystals |
| `VOX_LUDUS_ROUTE_LOG_SAMPLE` | Sampled `route_event` tracing |
| `VOX_LSP_LUDUS_EVENTS` | Disable LSP → Ludus `diagnostics_clean` hooks |

## PR / producer checklist

When adding a **new** Ludus event producer or `type` string:

1. Add or confirm **`base_reward`** in [`reward_policy`](../../../crates/vox-ludus/src/reward_policy.rs).
2. Extend **`process_event_rewards`** companion / quest / counter behavior, or document **policy-only** in [`agent-event-kind-ludus-matrix`](agent-event-kind-ludus-matrix.md) (for orchestrator types).
3. If the signal indicates user mistakes, map it in **`teaching_hook`** in [`event_router`](../../../crates/vox-ludus/src/event_router.rs).
4. Run **`cargo test -p vox-ludus`** (and MCP dispatch tests if tools changed).

## UX principles

- **Serious** mode keeps rewards but suppresses overlays/hints (see `GamifyMode`).
- Teaching hints are **pull-biased** (`vox ludus hint`) and **telemetry-logged** (`gamify_hint_telemetry`).
- Notifications for level-ups are **persisted** (`gamify_notifications`) in addition to CLI toasts.

