---
title: "Per-Task-Type LLM Cost/Model Policy — Design"
description: "Extends the (unexecuted) Band B orchestrator-settings plan with a TaskCategory x TriggerSource default-policy layer, so cost/model-selection strategy (free-tier-only / efficiency / balanced / cost-no-object) is configurable per task type instead of one global session dial."
category: "Architecture SSOTs"
status: "current"
---

# Per-Task-Type LLM Cost/Model Policy — Design

**Execution:** implemented via [docs/superpowers/plans/2026-08-02-per-task-type-cost-model-policy.md](../plans/2026-08-02-per-task-type-cost-model-policy.md).

**Predecessor context (verified against live code 2026-08-02):** Band A (LLM/AI settings SSOT: `vox-llm-config::LLM_CONFIG_KEYS`, reactive `vox://llm-config-changed`) and the `vox-llm-egress` single-egress consolidation are both **merged to main** (PRs #322, #329). Band B (`docs/superpowers/plans/2026-06-15-llm-ai-settings-ssot-band-b.md`, orchestrator settings under the registry) and `llm_bridge` wire consolidation (`docs/superpowers/plans/2026-06-15-llm-bridge-consolidation.md`) are both **plan-only, not executed**. This design is a narrow, self-contained slice that can land ahead of full Band B — it follows Band B's intended registry pattern but does not require Band B's other 100+ fields to exist first.

**Also verified:** `ClutchProfile` (Free/Efficiency/Balanced/Genius) and `RiskPosture` (High/Moderate/Low) already ship as real code in `crates/vox-orchestrator/src/mode.rs`, each resolving to concrete selection axes, cost preference, free-tier forcing, and budget-gate aggressiveness. `AgentTask` already carries optional `clutch`/`risk` hint fields end-to-end (parsed via `ClutchProfile::from_label`/`RiskPosture::from_label`, tested), falling back to `Balanced`/`Moderate` when unset. Every real call site today sets these to `None` — nothing assigns a clutch/risk by task type. This design fills that gap.

## Goal

Make cost/model-selection strategy configurable **per task type**, not just per session/human dial, by:
1. Introducing a `TriggerSource` axis (`Interactive` / `Automated` / `Subagent` / `Mesh`) alongside the existing `TaskCategory` axis.
2. Adding a small, registry-backed default-policy table (`TaskCategory → (clutch, risk)` and `TriggerSource → (clutch, risk)`) that end users can override live from a GUI panel.
3. Wiring the existing (currently-dead) `AgentTask.clutch`/`risk` hint plumbing and the MCP-direct `McpChatModelResolution` call sites to consult this table instead of silently defaulting to `Balanced`/`Moderate` for everything.

## Scope

**In:**
- `TriggerSource` enum + label parsing (mirrors `ClutchProfile`/`RiskPosture`).
- A registry of compiled-in defaults per `TaskCategory` and per `TriggerSource`, in the Band A/B pattern (`vox-llm-config`), with a live user-override layer.
- A single precedence resolver: **explicit task hint > TaskCategory policy > TriggerSource policy > global default** (today's hardcoded `Balanced`/`Moderate`).
- Wiring `AgentTask` (the queue/CI/scheduled/mesh/subagent path) and the MCP-direct chat/ghost-text/inline-edit call sites (always `Interactive`) onto the resolver.
- A minimal GUI panel (list of overrides, add/edit/remove) with a reactive change event, following the exact `get_user_config`/`vox://llm-config-changed` pattern Band A already shipped.
- Parity + resolver unit tests, a GUI vitest, and lightweight validation of malformed override entries (bad label strings) that fails soft (ignored + warned), never crashes.

**Out (explicitly deferred):**
- `llm_bridge` wire consolidation (confirmed decoupled — `model_route_policy` stays as-is regardless of when/whether the wire consolidation happens).
- The rest of Band B (the other ~100 orchestrator fields — scaling, planning, attention, MCP populi/federation, etc.).
- Combination-specific rules (e.g. "CodeGen, but only when Automated") — Approach B from the design discussion; the resolver's signature won't need to change if this is added later.
- Per-tenant policy (`AgentTask.tenant_id` exists but is untouched here).
- Changing `ClutchProfile`/`RiskPosture` semantics or adding new detents.
- Editing `contracts/orchestration/model-routing.v1.yaml` (that YAML's `task_categories` list is only *read* here, as the source of valid category names — not modified).

## Architecture

### The two axes

- **`TaskCategory`** (existing, generated from `contracts/orchestration/model-routing.v1.yaml::task_categories`): *what kind of work* — `CodeGen`, `Research`, `Chat`, `ToolOrchestration`, etc.
- **`TriggerSource`** (new): *who/what started the task* — `Interactive` (chat/GUI/editor features), `Automated` (CI/CD, scheduled, background jobs), `Subagent` (spawned by another agent), `Mesh` (A2A-delivered from another node).

These are independent. A `TaskCategory::CodeGen` task can arrive via any trigger source; a `TriggerSource::Automated` task can be any category.

### Precedence resolver

```
resolve_task_policy(explicit_clutch, explicit_risk, category, source) -> (ClutchProfile, RiskPosture)

  1. explicit_clutch / explicit_risk, if the caller set them        (already-wired AgentTask/hint fields)
  2. else: TaskCategory policy table entry for `category`, if any   (registry, user-override > compiled default)
  3. else: TriggerSource policy table entry for `source`, if any    (registry, user-override > compiled default)
  4. else: global default = Balanced / Moderate                     (today's hardcoded fallback, unchanged)
```

`explicit` and each table lookup independently cover clutch and risk — e.g. a task can inherit its category's clutch default but its source's risk default, if that's what the data says. A malformed override entry (a label string that fails `from_label`) is treated as "no entry" at that precedence level (falls through), never a hard error.

### Storage: two layers, Band A/B pattern

- **Compiled-in defaults** live in `vox-llm-config` (the existing layer-0 pure-data crate Band A/B established as the SSOT home) as plain string-keyed tables — `&[(&'static str, &'static str, &'static str)]` of `(category_or_source_name, clutch_label, risk_label)`. String-keyed (not the typed `TaskCategory`/`ClutchProfile` enums) because those enums live in `vox-orchestrator`, a much higher layer than `vox-llm-config` can depend on — the same reason `ClutchProfile::from_label` already takes a string today. No behavior change to those enums; this table just reuses their existing label vocabulary.
- **User overrides** persist wherever `OrchestratorConfig`'s existing Vox.toml loading already lives, as a new nested section:
  ```toml
  [orchestrator.task_policy.category]
  CodeGen = { clutch = "efficiency", risk = "moderate" }

  [orchestrator.task_policy.source]
  Automated = { clutch = "free", risk = "high" }
  ```
  merged over the compiled defaults, bumping the existing reactive-config-snapshot channel on write (same mechanism Band A's `vox-config::snapshot` already provides).

  *Implementer caveat (mirrors the caveat style already used in the Band A/B/egress plans in this repo): confirm at plan-writing time exactly which crate/file today owns `OrchestratorConfig`'s Vox.toml read/write/merge path (`orchestrator_fields.rs`/`impl_env.rs` per Band B's own notes) and whether `vox-orchestrator` already depends on `vox-llm-config` directly or only transitively — wire the override reader into whichever is authoritative, rather than assuming.*

- Seeding the compiled defaults (i.e., which categories/sources get a non-`None` default, and what) is **not decided in this design** — it's a data question, not an architecture question. It becomes a short Phase 0 inventory step in the implementation plan, mirroring how Band A seeded its own registry from a manifest rather than guessing values in the spec.

### Call-site wiring

- **`AgentTask`** gains `trigger_source: Option<TriggerSource>`. It is set **programmatically** by the paths that structurally know their own origin (background/discovery poll and scheduled dispatch → `Automated`; A2A delivery → `Mesh`; subagent fan-out → `Subagent`), and via an **explicit hint** (`TaskEnqueueHints.trigger_source: Option<String>`, parsed the same way `clutch`/`risk` hints already are) for the generic MCP task-submission path, defaulting to `Interactive` when neither applies (matching today's most common caller). `AgentTask::resolved_clutch()`'s current hardcoded `ClutchProfile::Balanced` fallback is replaced by a call into the new resolver.
- **MCP-direct call sites** (`chat_tools/agent_loop.rs`, `ghost_text.rs`, `plan_loop.rs`, `inline_edit.rs`, `models_tools.rs` — everywhere `McpChatModelResolution { clutch: None, .. }` is constructed today) pass `TriggerSource::Interactive` explicitly, since these are all live editor/chat features by construction — no inference needed.

### GUI

A minimal panel (new small section, not a new top-level surface) listing current overrides as rows (`scope: "Category: CodeGen"` or `"Source: Automated"`, clutch dropdown, risk dropdown, remove button) plus an "add override" control offering the categories/sources that don't yet have one. Backed by three new Tauri commands mirroring `get_user_config`/`set_user_config`/`reset_user_config`: `get_task_policy_overrides()`, `set_task_policy_override(scope_kind, scope_key, clutch, risk)`, `clear_task_policy_override(scope_kind, scope_key)`. Reactive via the existing `vox://llm-config-changed` event (reused, since this is conceptually the same "settings changed" signal — a new event is not justified for one small table).

## Data flow example

A CI pipeline enqueues a code-review task with no explicit clutch. `TaskEnqueueHints.trigger_source` is unset by the caller, but the CI dispatch path sets `AgentTask.trigger_source = Some(Automated)` programmatically. `task_category` resolves to `Review`. Suppose the operator has set an override `Automated → (Free, High)` but nothing for `Review`. The resolver returns `(Free, High)` — the pipeline runs on free-tier models with looser approval gating, without the operator having touched a single call site or a global dial that would've also cheapened their next interactive chat.

## Testing

- `vox-llm-config`: table well-formedness (no duplicate keys, every label string round-trips through the existing `from_label` parsers).
- `vox-orchestrator`: resolver precedence — one test per precedence level winning, one for malformed-override-falls-through, one for "no policy anywhere → global default" (extending the existing `resolved_clutch_risk_fall_back_to_neutral_defaults` test).
- `AgentTask`: `trigger_source` hint parsing (mirrors `apply_hints_parses_clutch_labels`), programmatic-set paths each get a unit test asserting the source they set.
- MCP-direct call sites: assert each constructs `TriggerSource::Interactive`.
- GUI: vitest that the panel renders current overrides and that add/remove round-trips through the mocked Tauri commands; a Rust test that the catalog reflects the registry (mirrors Band A's `catalog_matches_registry_nonsecret_keys`).
- A `vox doctor` (or CI) check that a malformed `[orchestrator.task_policy]` entry in `Vox.toml` produces a visible warning, not a silent no-op and not a crash.

## Error handling

- Unknown category/source name in an override (typo, or a category retired from `model-routing.v1.yaml`): ignored at load, warned once (mirrors `unregistered_llm_env`'s warn-once pattern), never blocks startup.
- Unknown clutch/risk label: same treatment as `apply_hints_unknown_clutch_risk_leaves_none` today — falls through to the next precedence level, not an error.

## Open questions to confirm during plan-writing (not blocking this design)

1. Exact current owner of `OrchestratorConfig`'s Vox.toml read/write path (file + whether `vox-orchestrator` already depends on `vox-llm-config`) — verify against live code, not this doc.
2. Whether the GUI panel belongs in the existing Runtime Settings view or a new small card — pick whichever the current `SettingsView.tsx` layout makes least awkward; not an architectural decision.
3. Compiled-default seed values per category/source — a short inventory pass, not a design decision.
