---
title: "Enforceable LLM/AI Settings SSOT"
description: "Collapse the three drifting LLM/AI config registries into one declarative single source of truth that generates the GUI surface and CI allowlist, reactively surfaces changes, and is enforced compile-time + CI + runtime — extended to cover orchestrator settings."
category: "Architecture SSOTs"
---

# Enforceable LLM/AI Settings SSOT — Design

**Status:** Approved design (brainstorming complete) — awaiting implementation plan.
**Implementer target:** Claude Sonnet 4.6, executed with workflows / parallel subagents.
**Supersedes the open parts of:** `docs/src/architecture/llm-model-selection-ssot-convergence-and-gui-plan-2026-06-07.md` and the GUI configurable-values/secrets plan. Those remain canonical for prior-art and threat model; this doc is canonical for the convergence + enforcement design.

## 1. Problem

Audit (graphify graph + ground-truth reads, 2026-06-15) found the LLM/AI **spine already exists** but the *settings* surface is fractured. Four concrete problems:

1. **The SSOT is split across three drifting registries of config keys.**
   - The *real* surface is ~30+ hand-written accessor functions in `vox-config` (`crates/vox-config/src/inference.rs`, `routing_policy.rs`): `openrouter_base_url`, `openai_compatible_base_url`, `ollama_tuning_*`, `openai_tuning_*`, `anthropic_tuning_*`, `gemini_tuning_*`, `together_tuning_*`, `hf_*`, `resolve_openrouter_model`, etc. Each reads an env var with a fallback.
   - `crates/vox-config/src/operator_registry.rs` (`OPERATOR_TUNING_ENVS`) is a **separate, partial** metadata registry used only by CI guards.
   - `crates/vox-gui/src/commands/user_config.rs` (`FIELDS: &[FieldSpec]`) is a **third**, hand-maintained catalog that surfaces only **~16 of the 30+ keys**. Keys like `anthropic_tuning_*`, `gemini_tuning_*`, `together_tuning_*`, `hf_*`, `VOX_GEMINI_ROUTE_POLICY`, `OPENROUTER_GEMINI_MODEL`, `GEMINI_DIRECT_MODEL`, `openrouter_chat_model_preference` are invisible/unconfigurable in the GUI.

2. **Dual egress.** `crates/vox-gamify/src/ai/client/transport.rs` is a *second, parallel* LLM client — its own `reqwest` calls to `generativelanguage.googleapis.com` and OpenRouter, its own key resolution (`resolve_gemini_key`/`resolve_openrouter_key`, `openrouter_base()`) — bypassing the sanctioned `vox-actor-runtime/src/llm/` facade.

3. **The enforcement detector is shallow and misaimed.** `crates/vox-code-audit/src/detectors/llm_provider_call.rs` is a line-regex (provider hostname literal near an HTTP call) whose message steers to `populi.*` builtins — it is aimed at **Vox-language** code, not the **Rust workspace**. It cannot distinguish a sanctioned Rust egress adapter from a rogue one, and the gamify second-egress path slips its model.

4. **No "surfaced as they change" mechanism.** The GUI reads config on demand (`get_user_config`); nothing pushes an update when config changes from `set_user_config`, env reload, or mesh sync.

### Existing assets to reuse (the "use existing systems" constraint)

| Concern | Existing system | Used in this design as |
|---|---|---|
| Crate-boundary enforcement | `vox-arch-check` (`docs/src/architecture/layers.toml`) | Rule: only the egress crate may reach egress primitives / provider base-url accessors |
| Source-pattern enforcement | `vox-code-audit` detectors | Upgraded `llm_provider_call` (Rust second-egress) + new registry-completeness / unregistered-env detectors |
| Config persistence | `vox-config` `VoxConfig` (`~/.vox/config.toml`) + flat `toml_config` keys | The registry is the schema over both tiers |
| Secrets | `vox-secrets` / Clavis | Registry marks `secret: true` keys; values still resolve via Clavis, never stored in config.toml |
| Egress facade | `vox-actor-runtime/src/llm/` (`chat/stream/embed/cascade/throttle/wire`) | The sole sanctioned egress; sealed in Phase 4 |
| Model selection | `vox-orchestrator/src/models/` (`registry/select/autonomic/policy/scoring/catalog`) + `tier_cascade.rs`, `calibration.rs` | A registered consumer; its knobs are onboarded in Band B |
| GUI IPC | Tauri commands + event bus | Reactive `vox://llm-config-changed` events |

## 2. Goal

One declarative **single source of truth** for every LLM/AI setting that:
- **(SSOT)** is the only declared home for every key; the GUI catalog, CI allowlist, and docs are *views* over it, not parallel copies.
- **(Surfaced as they change)** pushes effective-value changes to the GUI reactively (no polling).
- **(User-configurable via existing systems)** reads/writes through `VoxConfig` + `toml_config` + Clavis, unchanged at the persistence layer.
- **(Enforceable — strength D / belt-and-suspenders)**: compile-time where cheap (GUI inclusion, sealed egress via arch-check), CI parity for the key registry, runtime-authoritative resolution.
- **(Performant)** resolves once into a cached snapshot (no per-call `std::env::var`); all enforcement is compile/CI-time with **zero hot-path cost**.

## 3. Architecture

### 3.1 Component 1 — Declarative key registry (the SSOT)

A const table in a new module `crates/vox-config/src/llm_config_registry.rs`. Each entry is a rich spec:

```rust
pub struct LlmConfigKey {
    pub env: &'static str,              // e.g. "OPENROUTER_BASE_URL"
    pub default: DefaultValue,          // literal | computed accessor
    pub kind: Kind,                     // String | Url | Float | Int | Path | Enum | Bool
    pub group: Group,                   // General | ModelsAndEndpoints | Tuning | Training | Orchestrator(...)
    pub class: ConfigClass,             // UserPreference | NodeLocal | Bootstrap | CiGate (reuse operator_registry enum)
    pub label: &'static str,
    pub hint: &'static str,
    pub options: &'static [&'static str],
    pub secret: bool,                   // true → resolves via Clavis, never written to config.toml
    pub persistence: Tier,              // VoxConfig-sectioned | Flat-toml | EnvOnly
}
pub const LLM_CONFIG_KEYS: &[LlmConfigKey] = &[ /* every key, the single home */ ];
```

Everything else becomes a **view**:
- `operator_registry::OPERATOR_TUNING_ENVS` → a filtered iterator over `LLM_CONFIG_KEYS` (its `OperatorEnvSpec`/`ConfigClass` types are reused, not duplicated).
- `vox-gui` `FIELDS` → produced by `llm_config_registry::gui_fields()` (compile-time inclusion; **cannot** omit or drift keys).
- The ~30 accessors stay as **thin typed wrappers** (chosen over macro-generation for readability/debuggability). Each references its registry entry by `const` key, and resolution goes through the snapshot (§3.4).

**Enforcement (CI parity test):** a unit test in `vox-config` asserts the four sets are equal:
`{accessor keys} == {LLM_CONFIG_KEYS env} == {gui_fields keys} == {operator view keys}`.
Adding a key in one place without the others fails the test. A companion `vox-code-audit` detector flags any `std::env::var("…")` reading an LLM/AI-shaped name (provider/model/tuning/budget prefix) that is **not** registered.

### 3.2 Component 2 — Sealed egress chokepoint

`vox-actor-runtime/src/llm/` is the **only** module permitted to: construct a provider HTTP client, embed a provider hostname, or read a provider base-url accessor. Enforced two ways:
- **`vox-arch-check` rule** (check-time): a fan-in restriction so only the egress crate reaches the egress primitives. Drift fails the arch gate.
- **Upgraded `llm_provider_call` detector**: extend it from Vox/`populi.*` framing to also flag **Rust workspace** second-egress — `reqwest`/HTTP client construction co-located with a provider hostname *or* a `vox_config::*_base_url()` call, anywhere outside the allowlisted egress module. The allowlist is the egress crate path.
- **`compile_fail`/trybuild test** proving a provider client cannot be constructed outside the facade (to the extent the sealed-module pattern allows; the arch-check rule is the backstop where Rust visibility can't fully seal `reqwest::Client`).

Phase 4 **refactors `vox-gamify/src/ai/client/transport.rs` onto the facade**, eliminating the dual egress, then the detector/arch-rule prevent regression.

### 3.3 Component 3 — Reactive surfacing

A `tokio::sync::watch::Sender<LlmConfigSnapshot>` owned by `vox-config`. Writers bump it:
- `set_user_config` / `reset_user_config` / `set_user_config_value` (GUI tier writers),
- `toml_config::reload_user_config`,
- mesh secret/config sync.

A thin bridge in `vox-gui` subscribes to the watch receiver and forwards each change as a Tauri event `vox://llm-config-changed { keys: [...], snapshot_rev }`. The GUI Runtime/Orchestrator panels re-pull (or read the payload) and update without polling. This is the only new runtime machinery; it is event-driven, not a hot path.

### 3.4 Component 4 — Performance

- **Runtime:** the snapshot is resolved once (env > config.toml > default for each key) into `LlmConfigSnapshot`, held behind the watch channel (or `OnceCell` + invalidate-on-write). Accessors read the snapshot instead of calling `std::env::var` per invocation. Test-only env mutation paths invalidate the snapshot.
- **Enforcement:** parity test (unit), arch-check (graph), detectors (line-regex), trybuild (build-time) — all compile/CI-time. **No hot-path cost.**

## 4. Phasing & parallel-execution model

Each phase is independently landable, TDD-first, and ends with a `/code-review` + `verification-before-completion` gate. The dependency graph: **Band A is mostly sequential** (the registry must exist before its consumers migrate); **Phase 0 and Phase 6a–6e are parallel subagent fan-outs**; within Bands A phases, call-site migrations parallelize.

### Band A — Foundation

- **Phase 0 — Inventory (parallel fan-out).** One `Explore`/general-purpose subagent per crate-cluster enumerates every LLM/AI knob (env vars, hardcoded model/endpoint constants, tuning constants, orchestrator policy fields). A synthesis step merges into the canonical key manifest that seeds `LLM_CONFIG_KEYS`. **No code change** — produces `docs/superpowers/specs/llm-config-key-manifest.md`.
- **Phase 1 — Registry foundation.** Build `llm_config_registry.rs`; make `operator_registry` a view; add the parity test + unregistered-env detector. Accessors keep current behavior (still env-backed) — no snapshot yet.
- **Phase 2 — GUI derives from registry.** `vox-gui` `FIELDS` ← `gui_fields()`; delete the hand catalog; the parity test now covers the GUI set. All 30+ keys become visible/editable.
- **Phase 3 — Reactive surfacing.** watch channel in `vox-config`; Tauri `vox://llm-config-changed` bridge; GUI subscription.
- **Phase 4 — Seal egress.** Refactor `vox-gamify` onto the facade; add arch-check rule; upgrade `llm_provider_call`; add trybuild/`compile_fail` test.
- **Phase 5 — Snapshot-cache perf.** Introduce `LlmConfigSnapshot`; route accessors through it; invalidate on write; benchmark/verify no per-call `env::var` on hot paths.

### Band B — Orchestrator onboarding (expanded scope; internally parallel)

- **Phase 6 — Orchestrator knobs under the registry** as an `orchestrator.*` namespace. Sub-phased by independent subsystem, each a parallel subagent track against the now-stable registry:
  - **6a** routing priority / `routing_policy` (`VOX_AUTO_ROUTING_PRIORITY`, gemini route policy keys),
  - **6b** tier-cascade thresholds (`tier_cascade.rs` `RoutingTier`/`AlarmLevel`),
  - **6c** autonomic + bandit/calibration (`models/autonomic.rs`, `calibration.rs` `BanditArm`, exploration budget),
  - **6d** selection-axes / scoring weights (`models/scoring.rs`, `models/select.rs` `SelectionAxes`),
  - **6e** budgets / exploration gates.
  Each subsystem keeps its current types but sources its tunables from the registry snapshot; a parity sub-test per subsystem.
- **Phase 7 — GUI advanced panel** for orchestrator settings (new `Group::Orchestrator` rows render automatically from the registry), reactive via the same event bus.
- **Phase 8 — Consolidation.** Update this SSOT doc + `where-things-live.md`; retire the two superseded plan docs with pointers here.

### Parallelism summary (for the implementation plan)

| Phase | Concurrency |
|---|---|
| 0 | Fan-out: N subagents (1 per crate-cluster) → 1 synthesis |
| 1–5 | Serial across phases; within a phase, migrate call sites in parallel |
| 6a–6e | Fully parallel subagent tracks (independent consumers) → 1 merge |
| 7–8 | Serial after 6 |

## 5. Components as isolated units

- `llm_config_registry` — *what:* the key schema + resolution; *interface:* `LLM_CONFIG_KEYS`, `gui_fields()`, `resolve(key) -> Value`, `snapshot()`, `subscribe()`; *depends on:* env, `toml_config`, Clavis. Testable standalone via the parity test + snapshot tests.
- `egress seal` — *what:* the sole provider-reaching boundary; *interface:* `vox-actor-runtime/llm` public fns; *depends on:* registry (for base URLs/keys). Enforced by arch-check + detector.
- `gui config bridge` — *what:* expose registry to the Runtime/Orchestrator panels + forward change events; *interface:* `get_user_config`/`set_user_config` (unchanged signatures) + `vox://llm-config-changed`; *depends on:* registry watch channel.
- `orchestrator config adapters` (6a–6e) — *what:* each subsystem reads its tunables from the registry; *interface:* unchanged subsystem APIs; *depends on:* registry snapshot.

## 6. Testing strategy

- **Parity tests** (Phase 1+, extended per band): the four key-sets equal; per-orchestrator-subsystem sub-tests in Band B.
- **Detector tests**: Rust second-egress positive/negative cases; unregistered-env positive/negative.
- **Arch-check**: a fixture asserting a non-egress crate referencing a provider base-url accessor fails.
- **trybuild `compile_fail`**: provider-client construction outside the facade.
- **Snapshot tests**: env > toml > default precedence; invalidate-on-write; no per-call `env::var` (assert via a counting shim in tests).
- **GUI**: vitest that the rendered field list equals `gui_fields()` length; reactive event updates a field without re-mount.
- Every phase closes with `/code-review` + green `cargo clippy -p <touched crate> -- -D warnings` + targeted tests (Windows-safe formatting per AGENTS.md).

## 7. Non-goals (YAGNI)

- No change to *how* providers are called on the wire, model catalogs, or selection algorithms — only where their **settings** come from.
- No new persistence backend; `~/.vox/config.toml` + Clavis stay as-is.
- No macro-generated accessors (rejected for debuggability).
- No GUI redesign beyond auto-rendered registry rows + reactive updates.

## 8. Risks

- **`reqwest::Client` cannot be fully sealed by Rust visibility** → arch-check rule is the real backstop; trybuild covers the constructors we *do* control.
- **Phase 6 scope creep** → strict subsystem boundaries (6a–6e); each lands independently; a subsystem that resists registry-sourcing is deferred, not forced.
- **Snapshot/env coherence in tests** → provide a test helper that mutates env *and* invalidates the snapshot atomically (extend `vox-test-harness/env_scratch`).
- **arch-check / diagnostic crate-boundary quirks** (noted in prior audit) → validate the new rule against `vox-arch-check` early in Phase 4.
