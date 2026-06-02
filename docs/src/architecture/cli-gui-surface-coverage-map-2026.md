---
title: "CLI→GUI Surface Coverage Map 2026"
description: "Per-surface gap matrix of every top-level Vox CLI surface versus its GUI representation, and the association thesis linking the action-manifest derivation spine, the Ludus event bus, and Scientia as the first promoted surface."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-06-01"
training_eligible: true
training_rationale: "Grounded per-surface audit of CLI-to-GUI coverage; companion to the GUI capability audit."
---

# CLI→GUI Surface Coverage Map 2026

## Purpose

The [Vox GUI Capability Audit 2026](vox-gui-capability-audit-2026.md) answered the
*general* GUI-maturity question (is the catalog real? is the shell real? what is
mocked?) and laid out a five-phase plan. This document answers the *per-surface*
question it does not: **for each top-level CLI surface, what is its GUI coverage,
and where are the largest CLI-rich / GUI-poor gaps?**

It also states the **association thesis** — why VoxScientia, Vox Ludus, and the
other uncovered surfaces are one system, not three unrelated TODOs.

## Method

- CLI surface list: the `Cli` subcommand enum in
  [`crates/vox-cli/src/lib.rs`](../../../crates/vox-cli/src/lib.rs) (top-level
  variants; ~70 surfaces, 472 nested catalog entries per the capability audit).
- GUI reality: registered Tauri handlers in
  [`crates/vox-gui/src/main.rs`](../../../crates/vox-gui/src/main.rs) and the
  React surfaces under
  [`crates/vox-gui/ui/src/components/surfaces`](../../../crates/vox-gui/ui/src/components/surfaces/).
- Derivation spine: `command_catalog::build_catalog()` (clap introspection) and
  `contracts/operations/catalog.v1.yaml` → `build_action_manifest()`.

## Coverage tiers

Every surface falls into one of three tiers.

### Tier 1 — Real panel (live Tauri IPC, real backend)

| Surface | GUI panel | Backend wiring |
| --- | --- | --- |
| orchestrator (`dei`) | Dashboard, Flow | `get_orchestrator_status`, control-plane (pause/resume/doubt/overrule) |
| `model` | Models | `list_model_cards`, routing summary, scoreboard, explain |
| `memory` | Memory | `mnemosyne_recall` / `mnemosyne_reindex` (wired after the 2026-05-28 audit) |
| catalog / discovery | Catalog | `get_command_catalog` (clap-derived) + generic `execute_command` |
| runs | Runs | `start_gui_run` / `finish_gui_run` / `list_gui_runs` |
| config / preferences | Settings | `get_gui_preference` / `set_gui_preference` (partial) |

### Tier 2 — Catalog-metadata only

Discoverable in command search and runnable via the generic `execute_command`
shell-out, but with **no dedicated panel and no typed argument form**. This is
roughly 90% of the 472 catalog entries:

`build`, `check`, `test`, `run`, `fmt`, `add`, `remove`, `lock`, `sync`,
`doctor`, `audit`, `ci`, `db`, `repo`, `deploy`, `plan`, `llm`, `generate`,
`emit`, `pm`, `new`, `play`, `repair`, `migrate`, `grammar`, `snapshot`,
`drift-check`, `workflow`, `dispatch`, `telemetry`, `secrets`, `auth`,
`catalog`, `bundle`, `plugin`, `share`, `init`.

These are *discoverable, not ergonomic*: the user can find and fire them, but
gets no form, no validation, no output schema, no safety gating.

### Tier 3 — CLI-rich, GUI-absent or fixture-only

The named gap. These surfaces have substantial CLI/engine reality and little or
no live GUI.

| Surface | CLI reality | GUI reality |
| --- | --- | --- |
| **`scientia`** | ~40 subcommands; rich `contracts/scientia/` schemas; Phases G (findings-site) + H (dashboard JSON) **spec'd but unbuilt** | **None** — appears only as the `socrates` agent fixture in `claude-dashboard/data.js` |
| **`ludus`** | full `vox-gamify` engine (XP, quests, companions, battles, achievements, leaderboards, streaks, trust tiers, collegium/arena); `event_router::route_event()` already ingests CLI/orchestrator/GitHub events | `GamifyView.tsx` + `LudusBanner.tsx` **exist but render fixtures** — `commands/ludus.rs` is **not registered** in `main.rs` |
| `mens` (train/serve/probe) | delegated to `vox-ml-cli`; real fine-tuning + inference | no panel (Models surfaces routing, not training) |
| `populi` (mesh) | mesh join / status / admin | `MeshView.tsx` = fixture, no IPC |
| `oratio` (speech) | transcribe / listen | `SpeakPanel` / Loquela mic = local UI state |
| `schola` | scholarship domain (delegated to `vox-schola`) | none |
| `research` | infra up/down/status + eval harness + run | none |
| `visus` (GUI visual intelligence) | agentic visual bug detection | none (ironically) |
| `safety` / `attention` | guardrails + attention budgeting | `AttentionPanel.tsx` = fixture |

## Association thesis

The map makes the structural insight concrete: the three things the user named
(Scientia, Ludus, "other surfaces not in the GUI") are one system.

1. **The derivation spine is real but stops at Tier 2.** Clap catalog → metadata
   → generic shell-out. The capability audit's Phase 1 already prescribes the
   missing piece: a **GUI action manifest** with typed arguments, safety class,
   and output contract, so panels can be *generated* rather than hand-written.
   That is exactly what lifts a 40-subcommand surface like Scientia into the GUI
   without 40 hand-coded Tauri handlers.

2. **Scientia is the natural proving ground.** It is the largest Tier-3 surface
   and already plans (Phase H) to emit **dashboard JSON from the CLI**. The CLI
   is being built to produce GUI-shaped data; the GUI simply never consumed it.

3. **Ludus is already the cross-cutting event bus.** `route_event()` ingests
   events from every surface. Once any surface — derived or hand-built — emits
   actions through a common path, Ludus rewards follow for free. It is the one
   Tier-3 surface that *connects* the others, and it already has a half-built GUI
   (`GamifyView`) waiting to be wired.

**The system, stated once:** a typed action manifest (derivation) lets every
surface get a generated panel and emit a canonical action event; Ludus consumes
that event bus; Scientia is the first rich surface promoted on top of the spine.

## Relationship to existing plans

- Subsumes nothing; **companions** the capability audit (general maturity +
  5-phase plan) with the per-surface matrix it lacks.
- The action-manifest work is the capability audit's Phase 1, viewed through the
  lens of "what does it unlock for Tier-3 surfaces."
- Scientia Phases G/H and the `vox-gamify` engine are pre-existing; this map
  positions them as the first consumers of the spine rather than independent
  efforts.
