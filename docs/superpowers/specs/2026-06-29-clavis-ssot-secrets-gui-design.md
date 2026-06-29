# Clavis-as-SSOT: Surface Secrets in GUI Search

**Date:** 2026-06-29
**Status:** Design — scoped after codebase verification
**Scope center:** All-three, GUI-led. Threat model: local + occasional sync/share. Source of truth: code (the Clavis spec registry).

## Verification reframe (read this first)

The original brainstorm assumed a broad set of gaps. A hands-on audit of the current tree found **nearly all of it is already shipped.** The corrected picture:

| Originally-claimed gap | Actual state in `main` |
|---|---|
| Clavis secret env-names not in any CI gate | **Shipped** — `config_hygiene.rs:223-229` unions `vox_secrets::spec::managed_secret_env_names()` into the recognized set. |
| Detection regex is `VOX_*`-only (~65 bare keys invisible) | **Shipped** — regex at `config_hygiene.rs:417` matches any `[A-Z][A-Z0-9_]{2,}` plus wrapper helpers (`env_flag`, `env_u32`, `env_u64`, `env_duration`, `env_truthy`, `env_i64`). |
| Hygiene gate not in required CI sweep | **Shipped** — `ci.yml:466-469` runs `config-hygiene`, `config-registry-parity`, and `config-gui-codegen --check`. |
| GUI panel doesn't group/categorize | **Shipped** — `SettingsView.tsx:433-464,671-698` groups by `taxonomySlug`, auto-expands required-missing groups, persists collapse state. |
| Panel lacks health/count/backend status | **Shipped** — count badges (`{set} set / {missing} missing`, line 688), "action needed" badge (683), and a backend strip with mode/profile/availability pills (595-610). |
| LLM runtime leaks keys into process env via `set_var` | **Not a gap** — the only `set_var` calls in `llm/types.rs:361-412` are test scaffolding (save/restore env around tests). |

**The single genuine gap that remains:** managed secrets are **not in the global settings search index.** `settingsIndex.ts:27` is one hardcoded `secrets-keys` entry. Config keys flow into search via `...GENERATED_SETTINGS_INDEX` (codegen'd from `CONFIG_KEYS`), but secrets have no equivalent. So typing "gemini" or "huggingface" in settings search surfaces nothing, even though the panel renders every key live from `list_secret_status()`.

## Goal

Make every managed secret searchable in the GUI by generating a secrets search index from the Clavis spec registry — exactly mirroring the existing `generatedSettingsIndex.ts` pattern — so adding a `SecretId` in Rust automatically makes it findable in settings search, with zero manual edits, drift-gated by CI.

## Principle (unchanged)

The Clavis `spec/registry/*.rs` modules are the single source of truth. The GUI search index becomes a generated **view** over that SSOT, never hand-maintained.

## Design

Extend `crates/vox-cli/src/commands/ci/config_gui_codegen.rs`:

- Add `render_generated_secrets_index_ts(specs)` that iterates `vox_secrets::spec::all_specs()`, skips config-only specs (`taxonomy_class.is_config_only()`), and emits one `SettingEntry` per secret:
  - `id` = `secret-` + kebab(`canonical_env`)
  - `section` = `'secrets'`
  - `label` = `canonical_env` (the panel already labels rows by canonical env)
  - `hint` = `scope_description` (fallback to `canonical_env` when empty)
  - `keywords` = `taxonomy_class.slug()` + lowercased `_`-split tokens of `canonical_env` + each lowercased alias + the literals `api key`, `token`, `secret`, `clavis`, deduped
  - sorted by (`slug`, `canonical_env`) for stable output
- Write it to `crates/vox-gui/ui/src/config/generatedSecretsIndex.ts` inside the existing `run()` (and compare in the `--check` branch), so the existing `config-gui-codegen --check` CI step (`ci.yml:468`) covers drift for free.
- `settingsIndex.ts`: replace the single hardcoded `secrets-keys` line with `...GENERATED_SECRETS_INDEX`, mirroring line 34's `...GENERATED_SETTINGS_INDEX`.

## What does NOT change

The vault, keyring, resolution precedence, `vox secrets` CLI, Tauri commands, redaction DTOs, the entire `KeysSecretsSection` panel (grouping/health/backend strip), and the hygiene/parity gates. We add one render function, one generated file, one spread import.

## Non-goals

- Panel visual changes (already done).
- Backend/SSOT/gate changes (already done).
- New capabilities (test-connection, rotation, export bundle, per-profile views) — declined in brainstorming.
- The `set_var` "fix" — not a real gap.

## Testing strategy

- **Rust:** `cargo test -p vox-cli` — unit test on `render_generated_secrets_index_ts` (header present, one row per non-config secret, a known key like `GEMINI_API_KEY` appears with `section: "secrets"`). Drift gate: `vox ci config-gui-codegen --check`.
- **GUI:** `pnpm vitest` — `searchSettings('gemini')` returns the Gemini key after the spread import lands. (`npm` fails — repo is pnpm-managed.)
- **Verification before completion:** run `vox ci config-gui-codegen` to regenerate, then `--check` to prove it's clean; show the search test passing.

## Risks / ceilings

- **Label quality:** using `canonical_env` as the label is intentionally minimal (matches the panel). `// ponytail:` upgrade path — add a human `display_name` to `SecretSpec` only if the raw env names read poorly in search.
- **Empty `scope_description`:** many `core_ids`/`config` specs have empty descriptions, but those are mostly config-only and filtered out; remaining empties fall back to `canonical_env`.

## File map (touch list)

| File | Change |
|------|--------|
| `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` | add `render_generated_secrets_index_ts` + emit/`--check` the new file in `run()` |
| `crates/vox-gui/ui/src/config/generatedSecretsIndex.ts` | generated artifact (new) |
| `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts` | replace hardcoded `secrets-keys` line with `...GENERATED_SECRETS_INDEX` |
| `crates/vox-gui/ui/src/lib/installedSkills.test.ts` (or a new `settingsIndex.test.ts`) | vitest: search finds a provider key |
