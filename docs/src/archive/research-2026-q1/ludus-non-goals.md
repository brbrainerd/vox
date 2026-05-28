---
title: "Ludus: scope and non-goals"
description: "Defines optional gamification boundaries—never blocking core flows, not a correctness layer, notification/HUD expectations, kill-switch pointers, and legacy gamify_* naming vs Ludus UX."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Ludus: scope and non-goals

Ludus is **optional** gamification: companions, streaks, light rewards, and teaching hints. It must never block core workflows.

## What Ludus is not

- **Not required** to use Vox, the CLI, MCP, or the orchestrator. Disable with config (`gamify_enabled = false`) or `VOX_LUDUS_EMERGENCY_OFF=1`.
- **Not a correctness layer.** Rewards and hints are advisory; CI and compilers remain authoritative.
- **Not a second notification system for product-critical alerts.** In-app rows live in `gamify_notifications`; use MCP `vox_ludus_notifications_list` and explicit ACK tools (`vox_ludus_notification_ack`, `vox_ludus_notifications_ack_all`) instead of side effects on “peek” paths.
- **HUD is opt-in.** CLI `vox ludus hud` is behind the `ludus-hud` feature and pulls orchestrator deps; default installs use lighter Ludus surfaces.

## Kill-switch and session overrides

See [`env-vars`](../reference/env-vars.md) (Ludus section) for `VOX_LUDUS_*` (emergency off, session mode, verbosity, channel, experiment).

## Legacy naming

Codex tables and some MCP tool names still use the `gamify_*` prefix. That is **legacy schema**, not a separate product. Prefer **Ludus** in docs and UX; renaming tables would be a dedicated migration project.

## Related

- Crate overview: [`vox-ludus`](../reference/cli.md)
- Integration contract: [`ludus-integration-contract.md`](ludus-integration-contract.md)

