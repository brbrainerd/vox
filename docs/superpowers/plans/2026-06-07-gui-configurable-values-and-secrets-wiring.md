# GUI Configurable Values + Secrets Wiring (2026-06-07)

## Goal

Make the Vox GUI a complete, **honestly-wired** control surface for everything an end
user should be able to configure — secrets (Vox Clavis), runtime/general config, and
LLM endpoints/tuning — and fix the existing Settings panels that *look* wired but persist
to the wrong place or do nothing.

Persistence target decision (locked): **device-global `~/.vox/config.toml`** via
`vox-config`'s existing `set_user_config_value` / `load_user_config` /
`unset_user_config_value`. Correct for a desktop app, independent of cwd. Secrets stay in
the Clavis vault / auth.json as today.

---

## Verified current state (evidence)

### What already works (do not rebuild)

| Surface | Backend command(s) | Status |
|---|---|---|
| **Keys & Secrets (Clavis)** | `list_secret_status`, `set_secret`, `remove_secret` ([secrets.rs](../../../crates/vox-gui/src/commands/secrets.rs)) | **Real & wired.** Routes to auth.json registry tokens or `VoxCloudBackend` vault; write-only, redacted previews. Security invariant intact. |
| **Model routing** | `set_routing_priority` → `VOX_AUTO_ROUTING_PRIORITY` | Real, hydrates from `get_routing_summary_live`. |
| **Mesh & peers** | `trust_mesh_node` / `untrust_mesh_node` | Real. |
| **Signing keys** | `signing_key_status` / `rotate_signing_key` | Real. |
| **Gamification** | `get_gamify_settings` / `set_gamify_settings` | Real, hydrates. |

### Defects to fix ("fully wired" gaps)

1. **Orchestrator sliders are not hydrated and write to the wrong target.**
   - `vals` initialises to hardcoded literals (`concurrency: 7, capUsd: 5, doubtThresh: 0.6, …`),
     [SettingsView.tsx:447-451](../../../crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx).
     No read-back, so the panel never reflects reality.
   - `set_orchestrator_config` writes to a **project `Vox.toml` discovered from the GUI process `current_dir`**
     ([orchestrator.rs:249-252](../../../crates/vox-gui/src/commands/orchestrator.rs)). In a packaged
     desktop app cwd is unpredictable → wrong/missing manifest.
   - Field mapping is lossy: `concurrency→max_agents`, `capUsd→financial_cost_budget_micros`,
     `doubtThresh→trust_auto_approve_min`, `isolation→scope_enforcement`, `autobudget→exec_time_budget_enabled`.
     `doubt` and `checkpointMins` are pushed but **not mapped at all**.

2. **Theme & Telemetry are stored but inert.** Both persist to `gui.theme`/`gui.telemetry`
   via `set_gui_preference` and hydrate on mount, but no code *applies* them — grep for
   `data-theme`/`applyTheme`/`gui.theme` finds only the two `get_gui_preference` reads in
   SettingsView. Cosmetic toggles today.

3. **Keybinds** is a static display table (`KEYBINDS` const), not editable. (Out of scope —
   noted, not planned.)

### What should be configurable but has no GUI at all

- **Entire `vox-config` user config** (`~/.vox/config.toml`): default `model`,
  `daily_budget_usd`, `per_session_budget_usd`, `data_dir`, `model_dir`, `db_url`,
  `train_epochs`, `train_batch_size`. No Tauri command wraps
  [toml_config.rs](../../../crates/vox-config/src/toml_config.rs).
- **Inference profile + custom LLM endpoints + tuning** ([inference.rs](../../../crates/vox-config/src/inference.rs)):
  `vox_populi::inference_PROFILE`, OpenRouter/OpenAI/Ollama/HF base URLs, per-provider
  `*_TUNING_TEMPERATURE` / `*_TUNING_TOP_P` / `OLLAMA_TUNING_NUM_CTX`. Env-var-only, no
  persistence path, no GUI. Blocks proxy / Azure / self-hosted / LAN-Ollama users.

### Secrets-UX gaps (works, but unfriendly)

- All ~200+ non-config secrets render in one flat list; `taxonomySlug` is shown as a badge
  but the list is **not grouped/collapsed** by taxonomy.
- The rest of the Clavis CLI surface is absent from the GUI: backend/profile status,
  `import-env`, `migrate-auth-store`, `sync`.

---

## Design

### Persistence contract

All new GUI-edited config flows through three thin Tauri commands wrapping `vox-config`:

```
get_user_config()            -> Vec<UserConfigDto>   // load_user_config(), flattened
set_user_config(key, value)  -> ()                   // set_user_config_value
unset_user_config(key)       -> bool                 // unset_user_config_value
```

Keys are the canonical operator-tuning / env names already used as the `~/.vox/config.toml`
key space (e.g. `VOX_MODEL`, `VOX_BUDGET_USD`, `OPENROUTER_CHAT_COMPLETIONS_URL`,
`vox_populi::inference_PROFILE`, `OLLAMA_TUNING_NUM_CTX`). A small curated **catalog** of
the user-facing keys (label, hint, group, input kind, default, validation) is defined in
Rust and surfaced to the UI so the form is data-driven, not hand-maintained per field.

### Orchestrator fix

- Add `get_orchestrator_config()` reading the effective orchestrator config (defaults +
  `~/.vox/config.toml` overrides), returned in the `SettingsState` shape so the panel
  hydrates real values.
- Re-point `set_orchestrator_config` to persist via `set_user_config_value` (device-global)
  rather than cwd `Vox.toml`. Map **all** sliders, including the currently-dropped `doubt`
  and `checkpointMins`. Confirm each orchestrator field has a config-key / env override the
  runtime actually reads (verify against `vox-orchestrator` config loader during impl).

### Theme / telemetry

- Theme: apply `gui.theme` to the document root (`data-theme` attr or root class) at app
  bootstrap and on change, so the existing toggle becomes real.
- Telemetry: either wire `gui.telemetry` to the telemetry init path, or — if that can't be
  changed at runtime without a restart — surface that honestly ("applies on restart") rather
  than implying live effect. Decide during impl after reading `vox-telemetry` init.

### UI conventions (reuse, no new component lib)

Reuse `Row` / `Toggle` / `RangeInline` / `Glass` and the `KeysSecretsSection` /
`SigningKeysSection` patterns verbatim. New sections are added to the existing `SECTIONS`
nav in [SettingsView.tsx](../../../crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx).
Secret values use `type="password"` write-only inputs; non-secret config uses normal inputs
with save-on-change + debounced toast (same pattern as routing weights).

---

## Phases & tasks

### Phase A — Fix existing wiring (no new surfaces)

- **A1** Add `get_orchestrator_config` Tauri command; hydrate `vals` from it on mount
  (replace hardcoded literals).
- **A2** Re-point `set_orchestrator_config` to `~/.vox/config.toml` via `set_user_config_value`;
  map all fields incl. `doubt` + `checkpointMins`; drop the cwd `Vox.toml` discovery.
- **A3** Apply `gui.theme` at root on bootstrap + change.
- **A4** Resolve telemetry: wire it, or relabel as restart-scoped after reading `vox-telemetry` init.
- **A5** Verify round-trip: set in GUI → assert `~/.vox/config.toml` content → reload GUI → values persist.

### Phase B — New "Runtime / General" config section

- **B1** Rust: `UserConfigDto` + curated key catalog (label/hint/group/kind/default/validation)
  covering: `VOX_MODEL`, budgets, `data_dir`, `db_url`, train params, `inference_PROFILE`,
  OpenRouter/OpenAI/Ollama/HF base URLs, per-provider tuning (temperature/top_p/ctx).
- **B2** Rust: `get_user_config` / `set_user_config` / `unset_user_config` commands +
  register in `generate_handler!` ([main.rs](../../../crates/vox-gui/src/main.rs)).
- **B3** Frontend: new `RuntimeConfigSection` (grouped: General · Models & endpoints · Tuning ·
  Training), data-driven from the catalog, save-on-change with reset-to-default per field.
- **B4** Add `runtime` (and optionally `endpoints`) entries to `SECTIONS` nav.
- **B5** Round-trip + validation tests (bad URL rejected, numeric ranges clamped).

### Phase C — Secrets UX polish

- **C1** Group `KeysSecretsSection` rows by `taxonomySlug` into collapsible groups (match
  sidebar collapse behaviour).
- **C2** Add a Clavis status header: active backend + resolution profile (new
  `secrets_backend_status` command over `vox_secrets` backend-status).
- **C3** Add `import-env` (file picker → dry-run preview → apply) and `migrate-auth-store`
  actions as commands + buttons.

### Verification (all phases)

- `cargo build -p vox-gui`; `cargo fmt -p vox-gui` (never `--all`).
- `pnpm` (not npm) typecheck/build in `crates/vox-gui/ui`.
- `cargo run -p vox-arch-check` and `ssot-drift` clean if new command catalog/surfaces are
  registry-tracked.
- Manual round-trip per phase (set → inspect `~/.vox/config.toml` → reload).
- No stubs: every control calls a real command (per no-stubs policy).

---

## Open questions for impl time

1. Telemetry: live-reconfigurable, or restart-scoped? (drives A4 wording vs wiring).
2. Which of the ~30 hardcoded infra constants (HTTP timeouts, mailbox capacity, heartbeat
   intervals, SLO gates) are genuinely *operator*-facing vs internal? Default stance: **keep
   internal**, surface only the user-relevant set in B1 (model/budgets/endpoints/tuning/
   profile/data-dir/db-url). Revisit only on request.
3. Do we want an "Endpoints" sub-section split from "Runtime", or one combined section? (B3/B4).

---

## VERIFIED EXECUTION SPEC (2026-06-07, post-probe)

Three read-only probes confirmed the exact APIs. Decisions locked: **orchestrator stays in
`Vox.toml`** (daemon never reads `~/.vox/config.toml`; already hot-reloads via `RELOAD_CONFIG`),
**Phase B is full-feature** (convert inference consumers so endpoints/profile/tuning actually
take effect). A2 (re-point orchestrator) is **PRUNED**.

### Verified APIs

**VoxConfig (crates/vox-config):**
- `VoxConfig::load()` ([impl_ops.rs:15-28](../../../crates/vox-config/src/config/impl_ops.rs)) precedence:
  ENV > `Vox.toml` (workspace) > `~/.vox/config.toml` (global) > defaults. Already merges global.
- `VoxConfig::save()` ([impl_ops.rs:157-166](../../../crates/vox-config/src/config/impl_ops.rs)) →
  `save_merged_global_config` ([persist.rs:17-89](../../../crates/vox-config/src/config/persist.rs)):
  writes sectioned `[vox]`/`[train]`/`[db]`, **preserves unknown keys**. Use field-mutation + `save()`.
- `resolve_config_{str,u64,usize,bool}(name, default)` ([env_parse.rs](../../../crates/vox-config/src/env_parse.rs)):
  precedence env → `~/.vox/config.toml` → default. **No f32/i32 helper exists** — add
  `resolve_config_f32` / `resolve_config_i32` for tuning params.

**Inference conversion worksheet ([inference.rs](../../../crates/vox-config/src/inference.rs)):**
- Convert `inference_profile_from_env()` (l.32-44): raw `std::env::var("vox_populi::inference_PROFILE")`
  → `resolve_config_str("vox_populi::inference_PROFILE", "desktop_ollama")` + enum parse.
- Convert `local_ollama_populi_base_url()` (l.68-75): keep secret path; swap `POPULI_URL`/`OLLAMA_URL`
  env fallbacks → `resolve_config_str`.
- Un-hardcode base URLs (l.53-63): add `openrouter_base_url()` →
  `resolve_config_str("OPENROUTER_BASE_URL", "https://openrouter.ai/api")` and
  `openai_compatible_base_url()` → `resolve_config_str("VOX_OPENAI_BASE_URL"/"OPENAI_BASE_URL", "https://api.openai.com/v1")`;
  derive `/v1/chat/completions`, `/v1/models`, `/v1/embeddings`. Update the 6 OpenRouter + 5 OpenAI
  ref sites in vox-actor-runtime/llm/* + vox-orchestrator/catalog.rs.
- Tuning temp/top_p/num_ctx (l.137-211): currently via `resolve_secret`; make config-aware via new
  f32/i32 helpers (keep secret as one source).
- **DO NOT convert** (security boundary): API keys, HF token (env-only by design), model
  preferences routed through `secrets_str`, routing_policy/routing_migration.
- Tests to update: `local_base_prefers_populi_then_ollama` (inference.rs:221-250), profile-default
  tests in lib.rs; add toml-fallback tests.

**Orchestrator hydration (no GET_CONFIG IPC exists):**
- Add Tauri `get_orchestrator_config()` reading `Vox.toml [orchestrator]` (Option A — mirrors
  existing `set_orchestrator_config`; no daemon change). Field map:
  `concurrency←max_agents (def 8)`, `capUsd←financial_cost_budget_micros/1e6 (def 0.05)`,
  `doubtThresh←trust_auto_approve_min (def 0.85)`, `isolation←scope_enforcement (Warn/Wasm/Container/Native)`,
  `autobudget←exec_time_budget_enabled (def true)`, `doubt←socrates_gate_enforce (def false)`.
- **`checkpointMins` is an ORPHAN** — no `OrchestratorConfig` field. Decision: keep GUI-only
  preference (do not claim it drives the daemon) OR remove the control. Default: keep as
  `gui.checkpointMins` preference, labelled as UI-only, until a real durable-checkpoint field exists.
- Also map the currently-dropped `doubt` correctly in `set_orchestrator_config` (→ socrates_gate_enforce).

### Task order (sequential; frontend tasks share SettingsView.tsx so cannot parallelize)

1. **T1 (Rust, vox-config):** Add `resolve_config_f32/i32`; convert inference profile + local URL +
   base-URL helpers; update ref sites; TDD toml-fallback. *Prereq for T2 endpoints to take effect.*
2. **T2 (Rust+TS, vox-gui):** `get_user_config`/`set_user_config` Tauri cmds over `VoxConfig` +
   inference keys; new `RuntimeConfigSection` (General · Models & endpoints · Tuning · Training);
   register in `generate_handler!`.
3. **T3 (Rust+TS, vox-gui):** `get_orchestrator_config` cmd + hydrate orchestrator sliders; fix
   `doubt` mapping; mark `checkpointMins` UI-only.
4. **T4 (TS, vox-gui):** Apply theme (root `data-theme` + CSS vars for arcane/void/glacier).
5. **T5 (TS+Rust, vox-gui):** Secrets UX — group by taxonomy, backend/profile status command,
   import-env/migrate actions.

Telemetry 'cloud'/'local' remains descoped (needs net-new OTLP wiring) — leave as-is or hide.

## Non-goals

- Editable keybinds (display-only stays).
- Exposing every internal constant as a knob (explicitly avoided — config sprawl).
- Changing the secrets security model (write-only + redaction stays exactly as-is).
