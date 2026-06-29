# Vox Config Standardization via `#[derive(VoxConfig)]`

**Date:** 2026-06-29
**Status:** Design — pending review
**Scope:** A config-standardization *program* (the standard + per-domain migration of the long tail + the automation). Provider-routing tasks (SP1–SP6) are separate.

## Problem

~150 non-credential config knobs across the codebase are read at **point-of-use** via `vox_secrets::resolve_secret(SecretId::VoxXxx).expose()`, scattered across many files and crates (oratio alone: 67 read-sites / 13 files / 2 crates; search ~40 in `policy.rs`; plus MCP, dashboard, runner, mesh, etc.). This pattern has four defects, all verified this session:

1. **Wrong resolver.** `resolve_secret` (`vox-secrets/src/lib.rs:259-273`) resolves env → vault → auth.json → keyring — it **never reads `~/.vox/config.toml`**. So `vox config set VOX_X` silently does nothing for these knobs, and they pay for dead vault/keyring lookups.
2. **Registry pollution.** Each knob needs a `SecretId` + a placeholder `SecretSpec` (the ~252 non-credential entries in `registry/missing.rs`/`core_ids.rs`) purely so `managed_secret_env_names()` recognizes the env name — conflating config with secrets.
3. **Fragmented defaults + ~5-places-per-knob.** Defaults are inline at each read-site (`.unwrap_or(5)` / `.unwrap_or_else(default_fn)`), not centralized. Adding/changing a knob touches the struct field, `Default`, a `merge_env` line, a catalog entry, and a `CONFIG_KEYS` row (the orchestrator migration hand-wrote 76 such rows).
4. **Bypass of existing structs.** Domains that *already have* a config struct (`OratioRuntimeConfig` in `vox-speech/src/runtime_config.rs`; `SearchPolicy` in `vox-search/src/policy.rs`; `OrchestratorConfig`) are *bypassed* by the scattered reads, so the struct and the point-of-use reads diverge.

The conventions to fix this **already exist**: a domain `*Config` struct + `Default` + `merge_env()` reading via `vox_config::env_parse::resolve_config_*` (which gives the correct **env → `config.toml` → default** precedence). There is **no config derive/codegen** (the env-var-SSOT design `docs/src/architecture/env-var-ssot-design-and-handoff-2026-06-15.md` rejected *build.rs* codegen for rebuild-storm reasons — a proc-macro is a different, acceptable mechanism).

## Goal

Converge every non-credential config domain onto **one annotated struct** via a `#[derive(VoxConfig)]` proc-macro that generates the env-merge, a global cached snapshot accessor, a GUI catalog, and the registry contribution — **one source of truth per knob**. Retire the scattered `resolve_secret` reads and the non-credential `SecretId`s. Result: config reads through the proper resolver (with `config.toml`), no manual registry rows, no config/secret confusion, and future replacement = editing one struct.

## Principle

A config knob is declared **once** — as an annotated field on its domain's config struct. Everything else (env-merge, defaults, registry row, GUI catalog, snapshot access) is *derived*. Credentials are not config: they stay in `vox-secrets`. The macro structurally prevents the config/secret conflation that created the placeholders.

## Design

### 1. The standard: `#[derive(VoxConfig)]`

```rust
#[derive(VoxConfig, Default, Clone)]
#[vox_config(prefix = "VOX_SPEECH", group = "Speech")]
pub struct SpeechConfig {
    #[config(default = 8, label = "Max bias phrases", hint = "…")]
    pub max_bias_phrases: u32,
    #[config(env = "VOX_ORATIO_CUDA", default = false)] // explicit env preserves legacy names
    pub cuda: bool,
    #[config(default = 0.5, bound = "0.0..=1.0")]
    pub contextual_bias: f32,
}
```

The derive (new crate `vox-config-derive`, re-exported from `vox-config`) generates, for the struct:

- `pub fn merge_env(&mut self)` — for each field, `self.field = resolve_config_<kind>("<env>", self.field)` where `<env>` defaults to `<prefix>_<FIELD_SCREAMING>` (overridable per field) and `<kind>` is inferred from the field type (bool/int/float/string; `Option<T>` → the `resolve_config_opt_*` helpers).
- `pub fn get() -> &'static Self` — a `OnceLock<Self>` snapshot built from `Self::default()` then `merge_env()`. The single centralized read path; point-of-use becomes `SpeechConfig::get().max_bias_phrases`.
- `pub fn catalog(&self) -> Vec<ConfigField>` — current/default/kind/group/label/hint per field, for GUI surfacing + introspection (mirrors `OrchestratorConfig::to_catalog`).
- `pub const fn config_keys() -> &'static [ConfigKey]` — one `ConfigKey { key, kind, default, bound, group, class: NodeLocal, home: Env, gui, secret: false, status: Active, label, hint }` per field, for the registry.

Field-type → `ConfigKind`/resolver mapping is fixed (bool→Bool, integer→Int, f32/f64→Float, String/enum/Path→String). Nested sub-structs (oratio has 6) are supported via a `#[config(flatten)]` field attribute that recurses (prefix composes).

### 2. Credential boundary

The config struct holds **non-credential knobs only**. A field whose name/type implies a secret, or marked `#[config(secret)]`, is a **compile error** with a message directing it to `vox-secrets`. Credentials continue to use `resolve_secret(SecretId::X)` at point-of-use. This makes the conflation that produced the ~252 placeholders structurally impossible going forward.

### 3. Registry convergence (retires manual rows)

Each derived struct exposes `config_keys()`. A **high-level aggregator** — a new module in `vox-cli` (which already links every domain crate; this respects the layering where `vox-config` is low-level and cannot depend on `vox-speech`/etc.) — concatenates all domains' `config_keys()` into the set the `config-registry-parity` gate checks, unioned with the existing hand-written `CONFIG_KEYS` (for genuinely cross-cutting knobs). **Chosen over `linkme`/`inventory` distributed slices** for explicitness + layering safety + no dead-code-elimination surprises. Effect: domain knobs are registered automatically; the orchestrator's 76 hand-written rows become derived; drift is impossible.

**Two-gate note (important):** there are two gates. `config-registry-parity` checks literal `env::var("VOX_…")` reads against `CONFIG_KEYS` — satisfied by the generated `config_keys()`. `config-hygiene` Check-D checks against `registry.v1.yaml ∪ managed_secret_env_names()` and its regex matches `env::var`/`env_*` wrappers but **not** `resolve_config_*`. Because the generated `merge_env` reads via `resolve_config_<kind>(…)` (not `env::var`), config-hygiene does **not** see these reads — so **no `registry.v1.yaml` rows are required** for migrated knobs, and removing their `SecretSpec`s does not create config-hygiene violations (the reads are invisible to it). SP-A must confirm this empirically; if a stricter posture is wanted, add `resolve_config_*` to Check-D's recognizer and back the names with `registry.v1.yaml` (derivable from the same `config_keys()`).

### 4. Point-of-use migration (per domain)

Per domain: (a) ensure the `#[derive(VoxConfig)]` struct exists (oratio/search already have one — re-derive it; others: define it); (b) replace each scattered `resolve_secret(SecretId::VoxX).expose()…` with `DomainConfig::get().field`; (c) remove the non-credential `SecretId` variants + `SecretSpec`s (as the orchestrator bucket did, commit `53f5a3967a`); (d) the generated `config_keys()` registers them; (e) verify gates green + tests. Each domain is one sub-project.

### 5. Oratio improvements (the "along the way")

- Consolidate the 67 bypass reads across 13 files into the **existing `OratioRuntimeConfig`** (re-derived with `#[derive(VoxConfig)]`), gaining `config.toml` support + the GUI catalog and killing the bypass.
- Collapse the **`vox-speech` ↔ `vox-plugin-speech` duplication** (same knobs read in both crates) to one config source — `vox-plugin-speech` reads `vox-speech`'s `SpeechConfig::get()` (or the shared struct is hoisted to a common crate), eliminating the duplicate point-of-use reads.

### 6. Improvements captured

Single source per knob; auto-registry (no manual rows / no drift); `config.toml` for every migrated knob (was absent); a GUI-ready catalog for all knobs (feeds the provider-routing GUI work); config/secret confusion structurally prevented; future replacement = edit one struct.

## Decomposition (each its own spec → plan)

- **SP-A (foundation):** the `vox-config-derive` proc-macro + the `vox-cli` aggregator + a standardization lint (a `config-hygiene` check that flags *new* non-credential `resolve_secret(SecretId::Vox…)` point-of-use reads outside `vox-secrets`) + **re-derive the already-migrated `OrchestratorConfig`** to validate the macro end-to-end and auto-generate its 76 `ConfigKey`s. Proves the machinery on a known-good domain.
- **SP-B … SP-E:** migrate each remaining domain to the macro — oratio (vox-speech), search (vox-search), MCP, dashboard/runner/mesh — one per spec/plan, mirroring the orchestrator bucket but boilerplate-free.
- **SP-F (optional):** surface the aggregated `catalog()` in the GUI as a config editor.

## What does NOT change

- `vox_config::env_parse::resolve_config_*` (the resolver the macro emits calls to) and `toml_config` — reused as-is.
- `vox-secrets`/`resolve_secret` — still the path for real credentials.
- The orchestrator's *behavior* (SP-A re-derives its config to the same env names/defaults; output is equivalent).
- The provider-routing program (SP1–SP6) — separate.

## Non-goals

- A declarative/YAML-generated config (rejected: heaviest, and build.rs codegen was rejected by the SSOT doc).
- `linkme`/`inventory` distributed registration (rejected for layering/DCE safety).
- Migrating credentials out of `vox-secrets` (they belong there).
- Big-bang migration — strictly one domain per sub-project, each independently verified.

## Testing strategy

- **Macro:** `trybuild` compile tests (valid struct expands; a `#[config(secret)]` field fails with the right error); an expansion test asserting the generated `merge_env`/`config_keys` shape.
- **Per-domain:** round-trip unit tests — env var sets the field; `config.toml` sets it; neither → default; precedence env>toml>default. Snapshot test that `DomainConfig::get()` is stable.
- **Standardization gate:** a `config-hygiene` check asserting no scattered non-credential `resolve_secret(SecretId::Vox<domain>…)` reads remain (per migrated domain) + no `git`-new ones.
- **Registry:** `config-registry-parity` green against the aggregated keys; the orchestrator's derived keys equal its former hand-written rows (regression pin).
- **House rules:** never `cargo fmt --all`; `vox-gui` excluded from `clippy --all-targets`; pnpm for GUI.

## Risks / ceilings

- **Macro scope creep.** A config derive can balloon (validation, env aliases, nested flattening, enums). Mitigation: SP-A ships the *minimal* macro that covers the orchestrator + the field kinds actually used; extend per domain only as real fields demand. `// ponytail:` start with bool/int/float/string + `Option` + `flatten`; add enum/Path only when a migrated field needs it.
- **Legacy env names.** Many knobs have non-`<prefix>_<field>` names (`VOX_ORATIO_*`, not `VOX_SPEECH_*`). Handled by `#[config(env = "…")]` per field — but it must be set correctly per knob (the per-domain migration carries the env-name list from the existing specs).
- **Aggregator coverage.** The `vox-cli` aggregator must list every domain's `config_keys()`; a missing domain → its knobs aren't in the parity set. Mitigation: a test asserting every `#[derive(VoxConfig)]` struct in the workspace is in the aggregator (grep-based).
- **Global snapshot vs tests.** `get()` caches via `OnceLock`; tests that set env vars after first access see stale values. Mitigation: the macro also emits `merge_env`/`from_env_uncached()` for tests; document that `get()` is process-lifetime.
- **Behavior parity on re-derive (SP-A).** The orchestrator re-derive must reproduce exact env names + defaults. Mitigation: the regression pin (derived keys == former rows) + the existing orchestrator tests.

## File map (SP-A foundation)

| File | Change |
|------|--------|
| `crates/vox-config-derive/` | new crate — the `#[derive(VoxConfig)]` proc-macro |
| `crates/vox-config/src/lib.rs` | re-export the derive; `ConfigField`/`catalog` types if not present |
| `crates/vox-config/src/config_key.rs` | ensure `ConfigKey` is constructible from the macro (const-fn friendly) |
| `crates/vox-cli/src/commands/ci/` (new `config_aggregate.rs`) | aggregate all domains' `config_keys()`; wire into `config-registry-parity` |
| `crates/vox-cli/src/commands/ci/config_hygiene.rs` | new lint: flag new non-credential `resolve_secret` point-of-use reads |
| `crates/vox-orchestrator/src/config/` | re-derive `OrchestratorConfig` with `#[derive(VoxConfig)]`; drop hand-written `impl_env.rs` merge + the 76 manual `CONFIG_KEYS` rows |
| tests | `trybuild` + expansion + orchestrator regression pin |
