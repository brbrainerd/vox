# Clavis-as-SSOT: Correctly-Categorized, Searchable, AI-Legible Key Management

**Date:** 2026-06-29 (rev 2 — after 4-reviewer adversarial audit)
**Status:** Design — re-scoped on verified findings
**Scope center:** All-three, GUI-led. Threat model: local + occasional sync/share. Source of truth: code (the Clavis spec registry).

## Verification reframe (read this first)

A hands-on audit found that the secrets *backend* and *parity gate* are already shipped, but a 4-lens adversarial review then found the categorization the user asked for is **broken at the SSOT**, not merely missing from search. Corrected picture:

| Originally-claimed gap | Verified state |
|---|---|
| Clavis env-names not in CI gate | **Shipped** — `config_hygiene.rs:224-229` unions `managed_secret_env_names()` into the recognized set used by `check_env_reads_registered` (`:243`, suppression at `:606`). |
| Detection regex `VOX_`-only | **Mostly shipped** — regex (`:417`/`:595`) matches any `[A-Z][A-Z0-9_]{2,}` + most wrapper helpers. **BUT** it omits `env_usize` and `env_duration_from_ms`; `env_usize` has a live production read at `resilient_http.rs:58` (`VOX_HTTP_RETRY_MAX_ATTEMPTS`) → silent parity blind spot. (Addressed in Phase D.) |
| Gate not in required CI sweep | **Shipped** — `ci.yml:464-469`, no `continue-on-error`/`if:` guard. |
| GUI panel doesn't group/categorize | **Code present but DEGENERATE** — `SettingsView.tsx:434-465` groups by `taxonomySlug`, which is `metadata().taxonomy_class.slug()` (`lib.rs:499`). |
| Panel lacks health/backend status | **Shipped** — count badges (`:688`), backend strip (`:594-608`). |
| `set_var` leaks secrets to process env | **Not a gap** — only non-test `set_var` is the `vox_env_set` stdlib builtin (`builtins/mod.rs:1822`); all others are `#[cfg(test)]`. |
| Secrets absent from settings search | **Real** — `settingsIndex.ts:27` is one hardcoded landing entry; config keys flow via `...GENERATED_SETTINGS_INDEX`, secrets have no equivalent. |

**The keystone defect (verified by direct read of `ids.rs:589-703`):** `SecretId::metadata()` assigns `taxonomy_class = AuxTooling` to ~50 enumerated IDs and to the `_` fallback, and `ScholarlyPublication` to exactly one. It **never returns** `LlmProviderKey`, `CloudGpuInfra`, `SocialSyndication`, `MeshTransport`, `TelemetrySearch`, `PlatformIdentity`, or `OperatorTuning`. So the slug that drives panel grouping is effectively constant ("aux") — every provider key, GPU key, mesh token, and social token collapses into a single "aux" group. The user's request ("built out by category… easier to navigate") is unmet because the classification SSOT is inert.

Crucially, the **correct classification already exists** in `capabilities_for_secret()` (`mod.rs:156-225`), which maps each `SecretId` to `Capability::{ChatCloudPrimary, ChatCloudAlt, GpuCloud, Mesh, ScholarlyPublication, ScientiaSyndication, …}`. `metadata().taxonomy_class` simply doesn't consume it.

## Goal

Make every managed secret **correctly categorized** and **searchable** in the GUI, driven entirely from the Clavis spec registry, so adding a `SecretId` in Rust automatically lands it in the right category in both the live panel and settings search — with no hand-maintained drift. Fix the classification at the SSOT (which repairs the existing panel) and surface secrets in the unified, drift-gated search index.

## Principle (unchanged)

The Clavis `spec/registry/*.rs` modules + their per-secret metadata are the single source of truth. The GUI panel and the GUI search index are both **generated views** over that SSOT. Classification lives in exactly one place.

## Design

### Phase A — Repair the taxonomy SSOT (keystone)

`SecretId::metadata()` must return the real `TaxonomyClass` per secret. Reuse the existing `capabilities_for_secret()` groupings as the single classification source rather than re-enumerating by hand:

- Add `const fn TaxonomyClass::from_capability(Capability) -> TaxonomyClass` mapping: `ChatCloudPrimary | ChatCloudAlt → LlmProviderKey`; `GpuCloud → CloudGpuInfra`; `Mesh → MeshTransport`; `ScholarlyPublication → ScholarlyPublication`; `ScientiaSyndication → SocialSyndication`; `RuntimeIngress | PublishReview | Orchestration → PlatformIdentity`; `DbRemote → TelemetrySearch`; `AuxTools | AutonomousResearch → AuxTooling`.
- In `metadata()`, set `taxonomy_class` from the secret's primary capability (`capabilities_for_secret(self)[0]`), keeping `OperatorTuning` for the tuning IDs (the `*Tuning*` variants currently in the big arm) so `is_config_only()` becomes meaningful again.
- This single change repairs the **existing** panel grouping at zero GUI cost — the panel already reads `taxonomy_slug`.

Outcome: ≥6 distinct slugs across the registry; LLM keys → `llm`, GPU → `gpu`, mesh → `mesh`, social → `social`, scholarly → `scholarly`, telemetry/db → `telemetry`, platform/identity → `platform`, genuinely-aux → `aux`, tuning → `config` (filtered out).

### Phase B — Emit secret rows into the unified search index

Mirror, but **unify** with, the existing config-key codegen — do not create a second file (the `SettingEntry.section` field is already the discriminator):

- In `config_gui_codegen.rs`, add `render_secret_rows(specs) -> String` producing `SettingEntry` rows, and have `run()` write **both** the config rows and the secret rows into the single existing `generatedSettingsIndex.ts` under the existing `GENERATED_SETTINGS_INDEX` export. Because we extend the existing file (not add a new one), the existing `config-gui-codegen --check` drift gate (`ci.yml:468`) covers it with **no new `--check` branch** — which also sidesteps the early-`return` hazard at `config_gui_codegen.rs:219`.
- Per secret (filter out `is_config_only()`):
  - `id` = `secret-` + kebab(`canonical_env`)
  - `section` = `"secrets"`
  - `label` = `canonical_env`, **carried through the row tuple** (never reconstructed from the id — the reconstruction is lossy/collision-prone)
  - `hint` = `scope_description` (fallback `canonical_env` when empty)
  - `keywords` = `taxonomy_class.slug()` + lowercased `_`-split tokens of `canonical_env` + lowercased `_`-split tokens of each alias **and each `deprecated_alias`** (so migration searches hit) + the single literal `"clavis"`. **No blanket `"token"`/`"secret"`/`"api key"` literals** — those match every credential, carry zero discriminating value, and (because `SETTINGS_INDEX` also feeds the uncapped federated/omnisearch index via `useFederatedSearchIndex.ts`) would dump ~200 rows on a query for "token".
  - sorted by (`slug`, `canonical_env`).

### Phase C — TS wiring (minimal) + tests

- **Keep** the curated `secrets-keys` landing entry at `settingsIndex.ts:27` (it's the result for a generic "keys & secrets" query; generated rows are bare env names). The generated secret rows arrive via the **existing** `...GENERATED_SETTINGS_INDEX` spread at line 34 — so `settingsIndex.ts` needs **no structural edit** beyond what already exists.
- Tests in Phase B/C below.

### Phase D — Hygiene-regex hardening (separable)

Add `env_usize` and `env_duration_from_ms` to the detection regex in `config_hygiene.rs` (`:417` and `:595`) so the parity gate stops missing reads through those helpers; re-baseline. Independent of A–C; ship as its own commit or defer.

## AI-first stance (explicit decision)

The agent-actionable metadata (`remediation`, `required`, `capability`) already lives in the redaction-safe `SecretStatusRow` DTO (`lib.rs:463-510`) and is reachable via `vox secrets list` and the Tauri commands — that is the surface agents consume. We deliberately keep the GUI *search index* human-shaped (the five `SettingEntry` fields) rather than bloating it. **The genuine AI-first advance here is Phase A:** a correct `taxonomy_class` turns `capabilities_for_secret` into a meaningful, machine-readable capability map ("to do X you need a key of class Y"), benefiting both the human panel and any agent reasoning over the registry. `// ponytail:` future — fold the two hand-rolled emitters into the generic `vox-codegen-ts` schema path (`type_maps.rs`/`from_hir.rs`) if a third generated index appears.

## What does NOT change

Vault, keyring, resolution precedence, `vox secrets` CLI, Tauri commands, redaction DTOs, the `KeysSecretsSection` panel JSX (Phase A fixes its grouping without touching it), and the hygiene/parity gates (except the Phase D regex line).

## Non-goals

- Panel visual restyle (already shipped; Phase A repairs its categories).
- New GUI capabilities (test-connection, rotation, export bundle, per-profile views) — declined in brainstorming.
- Adding agent fields to the search index (data already exists in the DTO/CLI).
- The `set_var` "fix" — not a real gap.

## Testing strategy

- **Phase A (Rust, `vox-secrets`):** distribution test over `all_specs()` — `GeminiApiKey` → slug `"llm"`; a GPU key → `"gpu"`; a social key → `"social"`; assert ≥6 distinct slugs and that no single slug holds >70% of specs (guards against regression to the degenerate state). A `*Tuning*` id → `is_config_only()` true.
- **Phase B (Rust, `vox-cli`):** SSOT-derived structural invariants mirroring the existing `ts_index_and_rust_fields_*` tests (`config_gui_codegen.rs:260-307`): emitted secret-row count == count of non-config specs; every emitted `id` unique; every row's `label` equals its `canonical_env` (round-trip property — fails if the lossy reconstruction is used); a known provider (`GEMINI_API_KEY`) present with its real slug keyword; **no** row contains the keyword `"token"` or `"secret"` (flooding guard).
- **Phase C (GUI, vitest):** `searchSettings('gemini')` returns a row whose `id` starts with `secret-` (genuinely fails before the spread carries generated rows — the hardcoded landing entry already matches 'gemini' in its hint, so the assertion must target a `secret-` id); merged `SETTINGS_INDEX` has unique ids; `searchSettings('token')` returns far fewer than the secret count (flooding guard at the merged layer).
- **House rules:** pnpm only (`npm` fails); never `cargo fmt --all` (use `cargo fmt -p <crate>`); `vox-gui` breaks `clippy --all-targets` (scope clippy to the crate).
- **Verification before completion:** run `vox ci config-gui-codegen` then `--check` (clean); show the Phase A distribution test and the flooding-guard test passing.

## File map (touch list)

| Phase | File | Change |
|---|---|---|
| A | `crates/vox-secrets/src/spec/types.rs` | add `TaxonomyClass::from_capability` |
| A | `crates/vox-secrets/src/spec/ids.rs` | `metadata()` sets `taxonomy_class` from capability; keep `OperatorTuning` for tuning ids |
| A | `crates/vox-secrets/src/spec/` (test) | taxonomy distribution test |
| B | `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` | add `render_secret_rows`; append secret rows into the single `generatedSettingsIndex.ts` in `run()`; structural tests |
| B | `crates/vox-gui/ui/src/config/generatedSettingsIndex.ts` | regenerated (now also contains `secret-*` rows) |
| C | `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts` | vitest (new): search + merged-id-uniqueness + flooding guard |
| D | `crates/vox-cli/src/commands/ci/config_hygiene.rs` | add `env_usize`, `env_duration_from_ms` to regex; re-baseline |
| — | `contracts/gui/omnisearch-index.v1.yaml` | one-line: note the `setting` lane now includes secrets |
