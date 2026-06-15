# LLM/AI Settings SSOT — Band B (Orchestrator)

**Status:** Plan-only (design map). For implementation by Claude Sonnet 4.6 with parallel subagents + TDD.
**Date:** 2026-06-15
**Predecessor:** Band A (landed on main) — `vox-llm-config` `LlmConfigKey` registry, vox-config accessors, reactive `vox://llm-config-changed`; cost-tracking SSOT (PR #329, landed).

## Goal

Bring the **orchestrator** and its 100+ tuning surfaces under the same single-source-of-truth +
reactive-GUI model Band A established for LLM config. One registry, views (not copies), reactive
surfacing, persistence tiers, enforcement.

## Current State

### Orchestrator config is fragmented
- `crates/vox-orchestrator/src/config/orchestrator_fields.rs:16-490` — `OrchestratorConfig`, 100+ fields, loaded from Vox.toml `[orchestrator]`, `VOX_ORCHESTRATOR_*` env (`impl_env.rs::merge_env_overrides()`), and macro defaults (`defaults.rs`).
- **GUI is problematic:** `vox-gui/src/commands/orchestrator.rs:388-416` `get_orchestrator_config()` reads **only cwd Vox.toml** (not env-merged effective config → inert sliders). `set_orchestrator_config()` (L288-382) **writes cwd Vox.toml only** (ignores `~/.vox/config.toml`), fire-and-forget daemon reload, no reactive event.

### Model selection / routing split-brain
- Registry+selection: `vox-orchestrator/src/models/{registry,select}.rs`; MCP resolution `vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs` reads `cost_preference` from `OrchestratorConfig:26` + capability pins from `vox-secrets`.
- Band A SSOT: `vox-llm-config/src/keys.rs` (40+ LLM keys) via vox-config accessors.
- **Drift:** (1) model selection's `cost_preference` lives in orchestrator config, budget caps in LLM config; (2) capability pins (`VoxCapabilityImageGenerationModel/VisionModel/CodeGenModel`) live in `vox_secrets::SecretId`, not registry, not GUI; (3) routing policy (`VOX_AUTO_ROUTING_PRIORITY`, gemini route flags) is env-only, in neither registry nor struct nor GUI.

### Cost tracking SSOT exists, not unified into orchestrator GUI
- `vox-db/src/store/ops_agents.rs` `record_llm_outcome()` (single writer) + `llm_spend_summary()` (session/day/total).
- `vox-gui/src/commands/user_config.rs:305-338` `get_llm_spend()` reads spend + budget caps (from Band A LLM config). **No cost/budget controls in the orchestrator panel.**

## SSOT Principle (mirror Band A)

1. Single registry is the home for all orchestrator keys. 2. Views, not copies (vox-config accessors, GUI catalog, operator registry read the one registry). 3. Reactive `vox://orchestrator-config-changed` watch channel. 4. Persistence tiers (`VoxConfig` / `FlatToml` / `EnvOnly`). 5. Enforcement (parity tests, arch-check, detectors, CI).

## Scope

**In:** every `OrchestratorConfig` field (scaling, cost budget, model routing, trust/Socrates/scope, planning/attention/exec-budget/observer, all timing thresholds, MCP populi/federation); model selection axes (`SelectionAxes` weights); calibration/bandit tuning; unified cost-preference controls (daily cap, per-session cap, cost-preference enum, budget-gate thresholds).
**Out (follow-on):** model-catalog sync (data not config), cost-accounting algorithms (business logic), capability-pin sources (separate negotiation SSOT), routing-policy resolution functions (refactored but not registry-homed yet).

## Phases

- **B.0 Inventory** — parallel subagents enumerate all knobs (`vox-orchestrator/src/{config,models,calibration}`, `vox-orchestrator-mcp/.../model_route_policy`, budget/attention/planning/observer); synthesize `docs/superpowers/specs/orchestrator-config-key-manifest.md`. No tests.
- **B.1 Registry foundation** — `vox-llm-config/src/orchestrator_keys.rs` (100+ entries, batched 3–4 PRs); `OrchestratorConfig` gains `snapshot()` resolving registry → struct; accessors become thin wrappers. **Critical path.** Parity test: registry keys == struct fields.
- **B.2 Reactive surfacing** — extend vox-config watch channel (`OrchestratorConfigSnapshot` or unified `RuntimeConfigSnapshot`); GUI subscriber forwards `vox://orchestrator-config-changed {keys, rev}` (mirror `spawn_orchestrator_status_stream`).
- **B.3 GUI auto-render** — `get_orchestrator_config_catalog()` → `Vec<GuiOrchestratorField>`; frontend renders dynamically by group, listens for change event. Parity test: catalog len == registry len.
- **B.4 Persistence tier unification** — classify each key's tier; `set_orchestrator_config()` writes correct tier (`~/.vox/config.toml` `[orchestrator]` / root / cwd override); `get_orchestrator_config()` returns env>project>user>default merged view (fixes inert sliders).
- **B.5 Cost/budget unification** — one "Cost & Budgets" GUI section: caps from LLM config, cost-preference + gate thresholds from orchestrator registry, spend progress via `get_llm_spend()`.
- **B.6a–e Model selection (fan-out):** (a) selection axes & scoring weights; (b) routing policy keys (gemini/OpenRouter pins, capability pins out of secrets enum); (c) autonomic & bandit tuning; (d) budget-gate thresholds (downgrade/halt fractions); (e) capability preferences (tool-use/reasoning/web-search/image-gen flags + task pins). Each lands a PR + parity test.
- **B.7 Advanced orchestrator panel** — group-aware sections, field-type-aware controls (slider/toggle/dropdown/text), real-time updates.
- **B.8 Docs & consolidation** — update `where-things-live.md`; supersede old orchestrator config docs.

## Parallelism

```
B.0 → B.1 → { B.2, B.3, B.4 concurrent } → B.5 → { B.6a..B.6e fan-out } → B.7 → B.8
```
Critical path is B.0→B.1 (registry blocks all consumers). Suggested grid after B.1: Agent A (B.2+B.3), Agent B (B.4), Agent C (B.5); then 5-agent fan-out for B.6.

## TDD Notes

- **B.1:** parity (keys==fields by count+env-name); snapshot round-trip; env>TOML>default per kind.
- **B.2:** invalidate-on-write + channel propagation; Tauri event payload format.
- **B.3:** parity (catalog len); enum→options, int/float→range, bool→none; vitest renders all keys+groups.
- **B.4:** precedence env>project>user>default; set→read-merged round-trip.
- **B.5:** spend+cap aggregate; spend bar updates on event; no error when store unavailable.
- **B.6a–e:** weights apply to selection; routing pins correct+precedence; epsilon affects arm selection; gate fractions clamp+trigger; capability requirements filter models.
- **B.7:** all sections render; vitest + E2E adjust→event→change.

## Risks

1. Snapshot coherence in tests → `env_scratch_with_snapshot_invalidate()` helper.
2. GUI perf rendering 100+ fields → per-field `key`, update changed field only; measure pre-B.7.
3. Daemon reload ack missing → `set_orchestrator_config()` awaits reload RPC (5s timeout), warn-not-block.
4. B.6 scope creep → strict per-track boundaries + one parity test each; defer resistant subsystems.
5. orch/LLM config split-brain persists → B.5 surfaces them together; explicit invariant + parity test.
6. Band A registry stability → start B.0 only after Band A parity tests green (already landed).
7. Arch-check misses implicit env deps → B.1 parity test + detector flagging `std::env::var("VOX_ORCHESTRATOR_*")` outside snapshot resolution.
8. Tauri event ordering (change before subscribe) → buffer last N snapshots; GUI fetches current on mount; event advisory.

## Key Files

- `crates/vox-llm-config/src/orchestrator_keys.rs` (new), `src/lib.rs:1-135`
- `crates/vox-orchestrator/src/config/orchestrator_fields.rs:16-490`, `impl_snapshot.rs` (new), `impl_env.rs`, `mod.rs`
- `crates/vox-config/src/lib.rs` (watch channel)
- `crates/vox-gui/src/commands/orchestrator.rs:288-416` (refactor get/set + `get_orchestrator_config_catalog()` + `spawn_orchestrator_config_subscriber()`)
- `crates/vox-orchestrator/src/models/{scoring,select}.rs`, `route_policy.rs`, `calibration.rs`
- `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:26-114`
- `crates/vox-orchestrator/src/budget_gate.rs:60-100`
- `crates/vox-gui/src/commands/user_config.rs:305-338`
- Frontend: `runtime-settings.*`, `advanced-orchestrator.*` (new)

## Notes for Sonnet 4.6

Reuse Band A patterns verbatim: the `vox://llm-config-changed` event/DTO shape, the watch-channel + Tauri subscription (`spawn_orchestrator_status_stream` as template), and the `vox-llm-config::tests` parity framework. Close every PR with `/code-review` + green tests, Windows-safe formatting per CLAUDE.md (`cargo fmt -p <crate>`, never `--all`).
