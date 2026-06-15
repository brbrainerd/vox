---
title: "Environment Variable SSOT — Design & Developer Handoff"
description: "Dynamic, auto-tracking single source of truth for all env vars and Clavis secrets, built on the PR #333 config-hygiene gate"
category: "Architecture SSOTs"
---

# Environment Variable SSOT — Design & Developer Handoff

> Status: design + handoff (no code in this doc). The plan in
> [Phased Handoff Plan](#phased-handoff-plan) is what an executor implements task-by-task.

## Overview & Problem

PR #333 landed a real, working config-hygiene gate
(`crates/vox-cli/src/commands/ci/config_hygiene.rs`) plus a finer parity gate
(`crates/vox-cli/src/commands/ci/config_registry_parity.rs`). They are the
foundation we build on. But four structural gaps mean the project does **not** yet
have a single source of truth (SSOT) for environment variables:

1. **VOX_-only blind spot.** Check D's recognition regex is
   `["']?(VOX_[A-Z0-9_]+)["']?` and it only fires on lines that also contain
   `env::var` / `env_var` (config_hygiene.rs:294,301-302). The env-read tracer found
   **~248 real distinct env names**, of which **~65 are non-VOX** — including *every
   bare-named credential read*: `OPENAI_API_KEY`, `VAULT_TOKEN`, `VAULT_ADDR`,
   `INFISICAL_TOKEN`, `INFISICAL_SERVICE_TOKEN`, `TURSO_URL`, `DB_PASSWORD`,
   `API_KEY`, `OPENROUTER_BASE_URL`, plus standard vars (`HOME`, `PATH`, `RUST_LOG`,
   `CARGO_*`, `XDG_*`, `GITHUB_SHA`). None of these are visible to the gate today.

2. **Grandfathering is a coarse `check|file` ratchet, not per-var.** `baseline_key =
   {check}|{file}` (config_hygiene.rs:19-21). Once a file is in
   `contracts/config/config-hygiene-baseline.txt` for `env-var-not-in-registry`, it
   is grandfathered for **all current and future** unregistered VOX_ vars in that
   file. Adding a brand-new unregistered var to an already-dirty file does **not**
   fail the gate. The on-disk baseline has **209** non-comment keys (141
   env-var-not-in-registry, 64 no-cwd-relative-contract-path, 3
   declared-but-unwired-config, 1 protected-module-no-env) — note this differs from
   the "271/382" figures in the original ask; those refer to the *separate*
   `config-registry-baseline.txt` (742 names). A redesign must reconcile that
   discrepancy explicitly.

3. **Clavis split-brain.** There are *already* multiple authoritative-ish registries:
   - `contracts/config/registry.v1.yaml` (YAML, 11 knobs, 6 with non-null `env_var`) — drives Check D.
   - `vox_config::config_registry::CONFIG_KEYS` (Rust) — drives `config-registry-parity` (742-name baseline).
   - `contracts/config/env-vars.v1.yaml` (cross-language surface contract).
   - The `vox-secrets` (Clavis) **SecretSpec registry** (~450 `SecretId` variants,
     9 registry modules, `managed_secret_env_names()`) — the *richest and most
     authoritative* env-name set in the repo, and it is invisible to every config gate.

4. **Line-based YAML parser fragility.** `load_registered_env_vars`
   (config_hygiene.rs:273-285) does `strip_prefix("env_var: ")`, ignores `status`
   entirely (a `deprecated` var still silences Check D), has no knob/owner linkage,
   and does not validate the YAML. On read failure it `unwrap_or_default()`s to an
   **empty set** (line 199) → every VOX_ read would be flagged. A no-space
   `env_var:VOX_X`, a tab, or inline-flow YAML is silently missed.

The goal: **one dynamic, auto-tracking SSOT covering ALL env vars (VOX and non-VOX)
and ALL Clavis secrets**, built by *extending* PR #333 — not replacing it, and not
re-implementing Clavis.

## Current-State Map

### What PR #333 gives us
| Asset | Path | Role |
|---|---|---|
| `config-hygiene` gate (Checks A–D) | `crates/vox-cli/src/commands/ci/config_hygiene.rs` | A: no cwd-relative contract paths; B: protected modules read no env; C: declared-but-unwired resolvers; **D: VOX_ env reads must be registered** |
| `config-registry-parity` gate | `crates/vox-cli/src/commands/ci/config_registry_parity.rs` | VOX_ names used vs `config_registry::registered_keys()`; supports prefix rows (`VOX_DB_`); warns on phantom rows |
| YAML registry | `contracts/config/registry.v1.yaml` | 11 knobs; fields `name, env_var, description, owner_crate, status, default_value, since`; status ∈ {active, declared, deprecated} |
| Hygiene baseline | `contracts/config/config-hygiene-baseline.txt` | 209 coarse `check|file` keys |
| Parity baseline | `contracts/config/config-registry-baseline.txt` | 742 bare VOX_ names |
| CLI wiring | `crates/vox-cli/src/commands/ci/run_body.rs:70-73` | Standalone `vox ci config-hygiene` / `config-registry-parity`; **not** in any aggregate sweep, **not** a required check |
| Catalog row | `contracts/operations/catalog.v1.yaml:2115` | `ci.config-hygiene` only (parity has none) |

### What Clavis (`vox-secrets`) gives us
The deepest existing SSOT. CLI `vox ln` (aliases `clavis`/`secrets`/`l`). A
compile-time `SecretId` enum (~450 variants) + `SecretSpec { id, canonical_env,
aliases, deprecated_aliases, backend_key, auth_registry, policy, remediation,
scope_description }`, aggregated by `ALL_REGISTRIES` / `all_specs()` and exposed as
`managed_secret_env_names()` (canonical + alias + deprecated, deduped). Resolution
precedence: env → backend (vox_vault/infisical/vault) → auth.json/secure-store →
populi env file. Clavis **reads env in; it never exports env back**. Example:
`GEMINI_API_KEY` → `SecretId::GeminiApiKey` (canonical `GEMINI_API_KEY`, alias
`VOX_GEMINI_API_KEY`, deprecated `GOOGLE_AI_STUDIO_KEY`, auth_registry `google`).

Key call: `vox_secrets::spec::managed_secret_env_names()`
(`crates/vox-secrets/src/spec/mod.rs:9-47`). This is the single function we will lean
on so that **all ~450 secret env names appear in the SSOT for free**.

### Full env-var reference trace (the "trace their references" deliverable)
| Bucket | Count (distinct, real) | Examples | Caught by #333 Check D today? |
|---|---|---|---|
| 1. VOX_* operational knobs | 183 | `VOX_WASM_SKILL_FUEL`, `VOX_MENS_DEFAULT_MODEL` | Yes (if read via one-line `env::var`) |
| 2. Standard third-party / OS | ~30 | `HOME`, `PATH`, `RUST_LOG`, `CARGO_*`, `XDG_*`, `GITHUB_SHA`, `OTEL_EXPORTER_OTLP_ENDPOINT` | **No** |
| 3. Secrets/credentials, bare-named | ~16 | `OPENAI_API_KEY`, `VAULT_TOKEN`, `INFISICAL_TOKEN`, `TURSO_URL`, `DB_PASSWORD` | **No** |
| 3b. VOX-prefixed credentials | (subset of VOX_) | `VOX_IDENTITY_MASTER_PWD`, `VOX_SECRETS_VAULT_TOKEN`, `VOX_TURSO_TOKEN` | Yes by name, but **mis-bucketed** as plain config |
| 4. Clavis/secrets-mgr selectors | ~20 | `VOX_SECRETS_BACKEND`, `VOX_SECRETS_PROFILE`, `VOX_CLAVIS_CUTOVER_PHASE` | Yes (VOX-prefixed) |

Totals: **~427** runtime `env::var`/`env::var_os` call sites + **1** `env::vars()` bulk
read across **195** files; **~356** compile-time `env!`/`option_env!` occurrences over
**7** distinct infra names (`CARGO_*`, `OUT_DIR`, `VOX_BUILD_NUMBER`, `VOX_GIT_HASH`).
Heaviest crates: vox-cli (51), vox-compiler (12), vox-ml-cli (10),
vox-secrets/vox-orchestrator-mcp/vox-orchestrator (8 each).

**Wrapper helpers** that a literal `env::var` regex misses and the redesign must
follow: `vox-telemetry` `env_flag` (config.rs:145); `vox-cli` `env_u32`/`env_i64`
(runner_scale.rs:70,77); `vox-config` `env_u64`/`env_duration`/`env_truthy`.

## Design Goals & Non-Goals

**Goals**
- **One SSOT** for every env var and every Clavis secret (one schema, one file family).
- **Dynamic / auto-tracking** — adding or removing an `env::var("FOO")` is detected and
  the registry is auto-updated (or the gate fails with a one-command auto-fix).
- **No feature loss** — Clavis's typed resolution, profiles, cutover phases, backends,
  capabilities all stay; we *reference* them, not reimplement.
- **Cost-effective & performant** — recognition derives from a compile-time / single-scan
  source; no per-runtime overhead; gate runtime stays in the low single-digit seconds.
- **Build on existing code** — extend Check D + the YAML registry + `managed_secret_env_names()`.

**Non-Goals**
- Do **not** replace Clavis or move secret *resolution* logic.
- Do **not** break the existing `config-hygiene` or `config-registry-parity` gates or
  their baselines; the redesign is strictly additive + a burn-down.
- Do **not** attempt to govern third-party crates' internal env reads (we can only
  annotate our own call sites).
- No greenfield registry format — extend `registry.v1.yaml` in place.

## Proposed Architecture

### The SSOT model — one enriched YAML, two feeder sources, one validated loader

Keep **`contracts/config/registry.v1.yaml` as the single human-facing SSOT** and make
it cover *all* buckets by adding fields. Clavis secrets are **not duplicated** into the
YAML by hand — they are **merged at gate time** from `managed_secret_env_names()` so
there is exactly one authoritative source per fact (knob facts in YAML; secret facts in
the Clavis registry).

Enriched schema (additive — existing rows keep working):

```yaml
schema_version: "2"
knobs:
  - name: wasm_skill_fuel
    env_var: VOX_WASM_SKILL_FUEL
    description: Fuel budget (instruction count) for WASM skill execution.
    owner_crate: vox-plugin-runtime-wasm
    status: active            # active | declared | deprecated
    default_value: "1000000000"
    since: "2026-06-15"
    # --- new in v2 ---
    secret: false             # true => credential material; redact in any dump
    source: env               # env | clavis | both | compile-time
    clavis_key: null          # SecretId variant name when source is clavis|both
    required: false           # gate may assert presence in a given profile
    bucket: vox-knob          # vox-knob | third-party | secret | clavis-selector
  - name: home_dir
    env_var: HOME
    description: User home directory for path resolution (voxup, telemetry).
    owner_crate: voxup
    status: active
    default_value: null
    since: "2026-06-15"
    secret: false
    source: env
    clavis_key: null
    required: false
    bucket: third-party
```

**Why this is the SSOT, not three registries:** the gate's recognition set becomes the
**union** of (a) `registry.v1.yaml` rows, (b) `managed_secret_env_names()` from Clavis,
and (c) `config_registry::CONFIG_KEYS` (the parity Rust set) — but each *fact* lives in
exactly one place. Clavis stays authoritative for secrets; the YAML stays authoritative
for knobs/third-party; the parity Rust set is folded in (Phase 4) so the two VOX_
registries stop diverging.

### Approach comparison (recognition + auto-tracking mechanism)

| Approach | How it tracks add/remove | Cost | Perf | Maintenance | Verdict |
|---|---|---|---|---|---|
| **(a) build.rs codegen** scans source, regenerates registry | build script greps `env::var` per crate, emits a generated list | Pays on *every* incremental build of the owning crate; cross-crate scan is awkward (build scripts see one crate) | Adds to compile time repo-wide; risks rebuild storms | Generated file churn, merge conflicts | ✗ rejected — taxes every build for a CI-time concern |
| **(b) derive/attribute macro** at each call site auto-registers | `env_var!("FOO")` macro that records into a linker-section/inventory registry | Requires touching all ~427 call sites + wrappers; proc-macro compile cost | Runtime inventory collection or build cost | High one-time churn; third-party + `env!` can't be annotated | ✗ rejected — huge blast radius, can't cover non-annotatable reads |
| **(c) CI-time scanner** diffs discovered vars vs registry, auto-proposes additions | Existing Check-D-style scan, extended to all names, with `--write` to append rows | Pays once per gate run (already paid for VOX_) | Zero runtime cost; single workspace pass | Low — reuses #333 machinery | ✓ **primary** |

**Primary = (c), extended.** It is the cheapest (we already run the scan for VOX_),
fastest (zero runtime cost, one ripgrep-class pass), lowest-churn (no call-site edits),
and the only one that can *also* fold in compile-time `env!`, `var_os`, wrapper helpers,
and Clavis names. We harden the recognizer and add an **auto-fix writer** so the registry
self-updates.

The detector pattern is already proven in the repo: `unregistered_llm_env`
(`crates/vox-code-audit/src/detectors/unregistered_llm_env.rs:30`) builds its registered
set *live* from `LLM_CONFIG_KEYS` so it cannot drift. We generalize that idea.

## Dynamic auto-tracking mechanism

Flow when a developer **adds** a read:

```text
dev writes  std::env::var("FOO_BAR")  in crates/foo/src/lib.rs
        │
        ▼
vox ci config-hygiene            (Check D, now bucket-aware, non-VOX-aware)
        │  FOO_BAR ∉ (registry.v1.yaml ∪ managed_secret_env_names() ∪ CONFIG_KEYS)
        ▼
FAIL with remediation:
  "Unregistered env var FOO_BAR read in crates/foo/src/lib.rs:42.
   Run: vox ci config-hygiene --write   (appends a stub row to registry.v1.yaml)"
        │
        ▼
dev runs  vox ci config-hygiene --write
        │  appends:
        │    - name: foo_bar
        │      env_var: FOO_BAR
        │      status: declared
        │      bucket: third-party        # auto-inferred (see heuristics)
        │      secret: false              # auto-flagged true if name ~ /(KEY|TOKEN|SECRET|PASSWORD|PWD)$/
        │      source: env
        │      owner_crate: foo           # inferred from path
        │      since: "<today>"
        ▼
dev fills description, re-runs gate → PASS
```

Flow when a developer **removes** a read: the scan finds a registry row whose `env_var`
no longer appears in any non-test source. The gate emits a **non-fatal WARN** (mirroring
parity's phantom-row warn at config_registry_parity.rs:124-130) and `--write` deletes the
orphan row (or, if `status: deprecated`, leaves it for one release then prunes). This keeps
the baseline monotonically shrinking.

**Auto-bucket / auto-secret heuristics** (used only to seed a row; human confirms):
- `secret: true` if name matches `/(API_KEY|_KEY|TOKEN|SECRET|PASSWORD|PWD|CREDENTIAL)$/`
  **or** the name is in `managed_secret_env_names()`.
- `bucket: clavis-selector` if name matches `/^VOX_(SECRETS|CLAVIS)_/`.
- `bucket: vox-knob` if `^VOX_` and not secret/selector; else `bucket: third-party`.
- `source: clavis` if name ∈ `managed_secret_env_names()`; `compile-time` if only seen in
  `env!`/`option_env!`.

**Data-model changes to `registry.v1.yaml`** (Phase 1): bump `schema_version` to `"2"`;
add `secret`, `source`, `clavis_key`, `required`, `bucket`. All optional with safe
defaults so existing 11 rows validate unchanged. Replace the line-based parser with a
serde struct (Phase 1) so malformed YAML is a hard error, never a silent empty set.

## Migration / burn-down plan

The 209 hygiene-baseline keys (esp. the 141 `env-var-not-in-registry` file keys) and the
742 parity names are retired **incrementally**, never big-bang:

1. **Freeze, don't grow.** First make the baseline *per-var* instead of `check|file`
   (Phase 2). This stops the "dirty file hides new vars" leak immediately — the most
   important single change — without forcing any cleanup yet.
2. **Ratchet-down test.** Add a CI assertion that the baseline line count is `<=` the
   committed count (monotonic shrink). `--update-baseline` may only *remove* lines unless
   explicitly overridden.
3. **Crate-by-crate burn-down.** Owners register the env vars in their crate (vox-cli first
   — it dominates with 51 files / 31 baseline files), deleting baseline lines as they go.
   Each crate is one small PR.
4. **Fold Clavis names** (Phase 3) — once `managed_secret_env_names()` is in the union,
   ~16 bare credential reads + the VOX_SECRETS_*/VOX_CLAVIS_* selectors drop out of
   "unregistered" automatically, shrinking the backlog without manual rows.
5. **Unify the two VOX_ registries** (Phase 4) so the 742-name parity baseline and the
   YAML stop diverging; converge on one recognition union.

## Phased Handoff Plan

Each phase is independently shippable, TDD-first, and references exact paths. Executor:
Sonnet 4.6.

### Phase 0 — Reconcile the count discrepancy + pin current truth
- **Touch:** `docs/src/architecture/env-var-ssot-design-and-handoff-2026-06-15.md` (this
  doc; record verified counts), add a test fixture `crates/vox-cli/tests/config_hygiene_baseline_counts.rs`.
- **Test:** assert `config-hygiene-baseline.txt` has exactly 209 non-comment lines split
  141/64/3/1; assert `config-registry-baseline.txt` has 742. Fails if anyone edits a
  baseline without updating the pinned count.
- **Commit:** `test(config-hygiene): pin baseline line counts (209 hygiene / 742 parity)`

### Phase 1 — Replace the fragile parser with serde + add v2 fields
- **Touch:** `contracts/config/registry.v1.yaml` (bump `schema_version: "2"`, add the 5
  new optional fields to existing rows with defaults), `crates/vox-cli/src/commands/ci/config_hygiene.rs`
  (replace `load_registered_env_vars` lines 273-285 with a `serde_yaml`-deserialized
  `RegistryFile { schema_version, knobs: Vec<KnobRow> }`; remove the
  `unwrap_or_default()` empty-set hazard at line 199 — make a read/parse error a hard gate
  failure).
- **Test:** `crates/vox-cli/tests/config_hygiene_registry_parse.rs` — (1) a malformed YAML
  fixture yields `Err`, not an empty set; (2) inline-flow `{env_var: VOX_X}` and no-space
  `env_var:VOX_X` are both recognized; (3) a `status: deprecated` row is still counted as
  registered (preserves current behavior) **but** flagged in output.
- **Commit:** `feat(config-hygiene): serde registry loader + v2 schema fields (secret/source/bucket)`

### Phase 2 — Per-var baseline (stop the dirty-file leak)
- **Touch:** `crates/vox-cli/src/commands/ci/config_hygiene.rs` (change `baseline_key`
  lines 19-21 from `{check}|{file}` to `{check}|{file}|{env_var}` for the
  `env-var-not-in-registry` check only; other checks keep file-granularity),
  `contracts/config/config-hygiene-baseline.txt` (regenerate via `--update-baseline`).
- **Test:** `crates/vox-cli/tests/config_hygiene_per_var.rs` — adding a *new* unregistered
  VOX_ var to an already-baselined file (fixture) now FAILS; the previously-baselined var
  in that file still passes.
- **Commit:** `fix(config-hygiene): per-var baseline keys for env-var-not-in-registry`

### Phase 3 — Fold Clavis secret names into the recognition union
- **Touch:** `crates/vox-cli/src/commands/ci/config_hygiene.rs` (Check D recognized-set =
  registry rows `∪ vox_secrets::spec::managed_secret_env_names()`); add `vox-secrets` as a
  dep of `vox-cli` if not already present (it is, via the secrets command).
- **Test:** `crates/vox-cli/tests/config_hygiene_clavis_union.rs` — `GEMINI_API_KEY` and
  `VAULT_TOKEN` (both in `managed_secret_env_names()`) are recognized as registered without
  any YAML row; a fictitious `ZZ_FAKE_KEY` is not.
- **Commit:** `feat(config-hygiene): recognize Clavis-managed secret env names (one SSOT union)`

### Phase 4 — Extend Check D beyond VOX_ (all buckets) + follow wrappers
- **Touch:** `crates/vox-cli/src/commands/ci/config_hygiene.rs` (replace the
  `(VOX_[A-Z0-9_]+)` regex at line 294 with `([A-Z][A-Z0-9_]{2,})` constrained to
  `env::var`/`env::var_os`/`env_var`/known wrapper calls; add a wrapper-name list:
  `env_flag`, `env_u32`, `env_i64`, `env_u64`, `env_duration`, `env_truthy`). Add an
  **allowlist** for unannotatable third-party/OS names seeded from the bucket-2 trace
  (HOME, PATH, RUST_LOG, CARGO_*, XDG_*, GITHUB_*) so they register as `bucket: third-party`
  rather than failing forever.
- **Test:** `crates/vox-cli/tests/config_hygiene_nonvox.rs` — a bare `env::var("DB_PASSWORD")`
  in a fixture is detected and (if unregistered) fails; `env_flag("VOX_FOO")` via the
  telemetry wrapper is detected.
- **Commit:** `feat(config-hygiene): detect all env reads (non-VOX, var_os, wrappers)`

### Phase 5 — Auto-fix writer (`--write`)
- **Touch:** `crates/vox-cli/src/commands/ci/config_hygiene.rs` (add `--write` flag distinct
  from `--update-baseline`: appends stub registry rows for unregistered names using the
  auto-bucket/auto-secret heuristics; prunes orphan rows whose env_var is unread),
  `crates/vox-cli/src/commands/ci/cmd_enums.rs:147` (add the flag),
  `crates/vox-cli/src/commands/ci/run_body.rs:70-73` (thread it through).
- **Test:** `crates/vox-cli/tests/config_hygiene_write.rs` — running `--write` against a
  fixture tree appends a well-formed `status: declared` row with `secret: true` for
  `NEW_API_KEY` and `bucket: third-party`; a second run is idempotent; an orphan row is
  removed.
- **Commit:** `feat(config-hygiene): --write auto-registers/prunes env-var rows`

### Phase 6 — Unify the two VOX_ registries + monotonic ratchet + wire as required gate
- **Touch:** `crates/vox-cli/src/commands/ci/config_registry_parity.rs` (source its
  recognized set from the same union as Check D so YAML and `CONFIG_KEYS` converge),
  add a baseline-shrink assertion to both gates, add `config-registry-parity` to
  `contracts/operations/catalog.v1.yaml` (mirror the `ci.config-hygiene` row at 2115),
  and add both to the pre-push aggregate sweep + a `.github/workflows/` required check.
- **Test:** `crates/vox-cli/tests/config_gates_monotonic.rs` — committing a baseline with
  *more* lines than HEAD fails; both gates agree on the recognized union for a shared
  fixture var.
- **Commit:** `feat(config): unify env registries, monotonic baseline, make gates required`

## Edge cases

- **`env!` / `option_env!` (compile-time).** ~356 occurrences, 7 distinct infra names
  (`CARGO_*`, `OUT_DIR`, `VOX_BUILD_NUMBER`, `VOX_GIT_HASH`). Register these with
  `source: compile-time`; scan for them with a separate regex but in the same gate. Do not
  fail builds — they are build-infra, allowlisted.
- **`env::var_os`.** Same treatment as `env::var`; Phase 4 regex covers both call forms.
- **Bulk `env::vars()`** (1 site). Cannot resolve a name statically → annotate the call
  site with `// config-hygiene: bulk-read <reason>` and allowlist that one line.
- **Dynamically-constructed names** (`format!`, concatenation). Unresolvable statically →
  require an inline annotation comment listing the names, checked by the gate; otherwise WARN.
- **Test-only reads.** The scan already skips test files for parity; keep `#[cfg(test)]` /
  `tests/` excluded so fixtures like `MY_API_KEY`, `FAKE_API_KEY` don't pollute the registry.
- **Third-party crate reads we can't annotate.** We only govern our own call sites; the
  bucket-2 allowlist (Phase 4) covers OS/toolchain names our code reads directly.
- **VOX-prefixed credentials** (`VOX_IDENTITY_MASTER_PWD`, `VOX_SECRETS_VAULT_TOKEN`): the
  auto-secret heuristic flags `secret: true` even though they pass a VOX_ regex, fixing the
  mis-bucketing.

## Cost & Performance

- **No runtime overhead.** Recognition is entirely CI/dev-time; production code paths are
  untouched. Resolution stays in Clavis exactly as today.
- **One scan, already paid.** Check D already walks `crates/` for VOX_; extending the regex
  and adding the Clavis union (`managed_secret_env_names()` is an in-memory `Vec` built from
  compile-time data) adds negligible cost. Target: **`vox ci config-hygiene` completes in
  < 5s** on the workspace (it is a ripgrep-class single pass + one YAML parse + one in-proc
  function call).
- **No build tax.** We explicitly reject build.rs codegen (approach a) precisely because it
  would pay on every incremental build; the CI-time scanner pays only when the gate runs.
- **Low maintenance.** `--write` removes the manual-row toil that would otherwise grow the
  registry by hand; the monotonic ratchet guarantees the backlog only shrinks.

## Acceptance Criteria

1. **Zero unregistered env reads outside baseline** for the extended (all-bucket) Check D —
   `vox ci config-hygiene` exits 0 on a clean tree and exits non-zero when an unregistered
   `env::var("X")` (VOX or non-VOX) is added.
2. **Baseline monotonically shrinks** — CI fails any commit that increases either
   `config-hygiene-baseline.txt` or `config-registry-baseline.txt` line count.
3. **All Clavis keys present in the SSOT union** — every name in
   `managed_secret_env_names()` is recognized as registered (verified by
   `config_hygiene_clavis_union.rs`); `GEMINI_API_KEY` and `VAULT_TOKEN` pass with no YAML row.
4. **Secrets are flagged** — every credential read carries `secret: true` (auto-heuristic +
   Clavis membership); no credential value is ever printed by the gate.
5. **Parser is robust** — a malformed `registry.v1.yaml` fails the gate loudly (no
   silent empty-set false positives).
6. **Two registries converge** — `config-hygiene` and `config-registry-parity` compute the
   same recognized union for a shared fixture.
7. **Gate runs in < 5s** on the full workspace and is wired as a required check.
