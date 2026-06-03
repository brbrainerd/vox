---
title: "Data Storage Lint & CI Spec (2026)"
description: "Concrete lint and CI rules — clippy.toml additions, deny.toml bans, grep checks, a new `vox ci data-storage-guard` subcommand, and diffs against `.github/workflows/ci.yml` — that implement the regression gates called for by the Data Storage SSOT. (The former parallel `.gitlab-ci.yml` mirror was removed 2026-06-03; its wiring steps here are obsolete.) Every rule is small, specific, paired with a finding ID, and references a file path that resolves against HEAD."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Machine-checkable policy the repository will enforce; agents should assume these gates are live and reason about changes under their constraints."
---

# Data Storage Lint & CI Spec (2026)

> **Update (2026-06-03):** The repository's parallel `.gitlab-ci.yml` mirror has
> been **removed** — maintaining a second CI pipeline alongside GitHub Actions
> was not worth the upkeep. Wherever this spec wires checks into `.gitlab-ci.yml`
> (the `vox-ci-guards` job, the §6.1 diff, release-stage wiring, etc.), treat
> those steps as obsolete and wire the equivalent into `.github/workflows/ci.yml`
> only. The GitHub Actions guidance below is unaffected.

Companion to [data-storage-ssot-2026.md](data-storage-ssot-2026.md) and [data-storage-migration-backlog-2026.md](data-storage-migration-backlog-2026.md). Every rule here either prevents regression of a finding (F1–F74) or enforces a target-state invariant declared by §4 or §5 of the SSOT.

Three rules guide every entry:

1. **No phantom infrastructure.** Every path, function, crate, env var, and CI job named below has been grepped against HEAD (revision audited 2026-04-21). If a file must be created, the ticket (M-NN) that creates it is cited.
2. **One check runs in ≤ 5 s** on the 60-crate workspace. Checks that need `cargo build` (codegen drift) are segregated and opt-in; the fast path is stateless grep + file existence + YAML parse.
3. **Each failure points at the finding that motivated it.** Report rows carry `finding_id: F<n>` so CI annotations link back to the SSOT.

## 0. Rewrite ledger

The prior draft of this spec referenced three crates that do not exist at HEAD (`vox-schema`, `vox-observability`, and `vox-spool` as a pre-existing crate). Those references have been removed. Corrected anchors:

- **Schema codegen** lives in a new `codegen` module inside the existing `crates/vox-jsonschema-util/` (M-10). No new crate is created until a third consumer or ≥1,500 LOC forces extraction.
- **Tracing/observability init** is consolidated into the existing `crates/vox-runtime/src/observability.rs::init_structured_telemetry` (M-40). The existing `crates/vox-cli-core/src/lib.rs:29::init_tracing_for_cli` becomes a two-line wrapper that defers to `init_structured_telemetry` with a CLI-shaped `EnvFilter`. No `vox-observability` crate is introduced.
- **JSONL spool writer** `crates/vox-spool/` is created by the ticket that lands it (M-03) — scripts and lint rules that reference it assume `M-03 has landed or is landing in the same PR`. Until then, the check runs in `scaffolded` state (§1.4).

The `data-ssot-guards` CI sub-command already exists (`crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs`, 286 LOC) and targets scientia/research telemetry parity. It is distinct from, and not replaced by, the new `data-storage-guard` defined here.

## 1. The `vox ci data-storage-guard` subcommand

The **single entrypoint** for data-storage CI checks. New sibling to the ~70 existing variants in `crates/vox-cli/src/commands/ci/cmd_enums.rs::CiCmd` (exact count varies with in-flight PRs; verify with `grep -c '^    #\[command(name' crates/vox-cli/src/commands/ci/cmd_enums.rs`). Slots into the existing `vox-ci-guards` GitLab job (`.gitlab-ci.yml` L56–81, 21 cargo invocations at HEAD) and into the main GitHub Actions check job (`.github/workflows/ci.yml` L35–130) — this spec avoids inventing a parallel governance surface.

### 1.1 Command shape

```text
vox ci data-storage-guard [--check <name>] [--json] [--fix]
```

- **No argument** → run every sub-check; non-zero exit on any failure.
- **`--check <name>`** → run one named check. Names are stable; see §1.3.
- **`--json`** → machine-readable report. Schema: `contracts/db/data-storage-guard-report.v1.schema.json` (created by M-04, step 3).
- **`--fix`** → apply fixes where the check is deterministic (currently only `rename-all-policy` and `bom-config-files`). Never modifies committed files without the user's consent; writes to stdout a `git apply`-shaped patch by default.

### 1.2 Implementation location

- Variant appended to `crates/vox-cli/src/commands/ci/cmd_enums.rs::CiCmd` as `CiCmd::DataStorageGuard { check: Option<String>, json: bool, fix: bool }` (matching the clap patterns already used by `SecretEnvGuard`, `SqlSurfaceGuard`, `OperatorEnvGuard`).
- Handler module `crates/vox-cli/src/commands/ci/data_storage_guard/` (new directory), with:
  - `mod.rs` — dispatch & report writer
  - `grep_rules.rs` — the 21 rules in §4, declarative table consumed by the shared `grep_runner`
  - `checks/` — one file per non-grep check (file-existence, BOM, drift, policy-parse)
  - `report.rs` — JSON writer conforming to the schema
- Dispatch added to `crates/vox-cli/src/commands/ci/run_body.rs` next to the existing `DataSsotGuards` arm. The match must be exhaustive — `cargo build -p vox-cli` fails if the new variant is added but not wired.
- Each sub-check is a free function returning `Result<CheckReport, CheckError>`; the dispatcher collects every check's result before returning so CI sees the full panel even when one fails.

### 1.3 Sub-checks (stable names)

Every row pairs the check name → finding → the migration item that flips it from `scaffolded` → `enforcing`.

| Name | Replaces/extends | Finding | One-line description |
|---|---|---|---|
| `contracts-index` | delegates to existing `vox ci contracts-index` | — | Every contract listed in `contracts/index.yaml` validates against `contracts/index.schema.json`. |
| `version-header-parity` | new | F12, F76, M-10, M-76 | Every `*.vN.*.{json,yaml}` contract carries a version-header key (`x-vox-version`, `schema_version`, or `version`) equal to `N`. Canonical key is `x-vox-version`; the other two are sunsetting synonyms until M-76 lands. |
| `domain-format-single` | new | F16, F75, M-15, M-75 | Within one `contracts/<domain>/` subdir, only one file format is present *unless* the subdir is listed in `data-storage-policy.v1.yaml::contract_policy.allowed_formats_by_domain` (today: `mcp`, `populi`; M-75 adds `orchestration`). |
| `orchestration-contract-sibling-parity` | new | F75, M-75 | Every `contracts/orchestration/*.v1.yaml` has a sibling `*.v1.schema.json`; vice versa. |
| `provider-secret-parity` | new | F77, M-77 | Every `providers[].secret_id` in `contracts/orchestration/providers.v1.yaml` matches a `SecretId` variant in `crates/vox-clavis/src/spec/ids.rs` handled by `crates/vox-clavis/src/spec/registry/llm.rs`. |
| `retention-policy-coverage` | delegated to `data-ssot-guards` | F78, M-78 | Every `CREATE TABLE` in `crates/vox-db/src/schema/domains/*.rs` is named in `contracts/db/retention-policy.yaml`. Not re-implemented in `data-storage-guard`; referenced here only so the check appears in the panel. |
| `domain-format-single` | new | F16, M-15 | Within one `contracts/<domain>/` subdir, only one contract format is present (or a documented exception in that subdir's README). |
| `openapi-contract-ref-parity` | new | F17, M-16 | OpenAPI component schemas `$ref` into canonical JSON schemas; no inline duplicates of top-level contract types. |
| `schema-codegen-drift` | new | F11, F14, M-11 | Re-running `vox schema generate` (module shipped by M-10) produces byte-identical Rust sources. |
| `codex-ssot-digest` | refocused from existing `vox ci check-codex-ssot` | F1, F3, F5, M-20, M-23 | Baseline digest in `contracts/db/baseline-version-policy.yaml::repository_baseline_integer` matches `vox_db::schema::manifest::BASELINE_VERSION` + delta chain. Drift is widening (54 contract vs 59 code at HEAD 2026-04-21 after `411adcac` bumped to 58 and `ff0fdccc` bumped to 59). Gate flips to *enforcing* only after M-20 lands — M-20 has been promoted out of Phase 2 to Phase 1b for this reason. |
| `schemas-dir-empty` | new | F15, M-05 | Top-level `schemas/` does not exist. Encoded in `contracts/db/data-storage-policy.v1.yaml::repo_hygiene.forbid_existence`. |
| `repo-root-strays` | new | F25, M-06 | None of `build_errors.txt`, `codex-cutover-*.sidecar.json`, `test_lexer.rs`, `error.vox` exist at repo root. Policy list lives in the same YAML as above. |
| `gitignored-but-tracked` | new | F26, M-07 | `vox-agent.json`, `vox-schema.json`, `vox.tokens.json` are not in `git ls-files`. |
| `scratch-clean` | new | F27, M-08 | `scratch/` contains only `.gitkeep`. Enforced by `require_empty_dirs` in the policy YAML. |
| `row-wire-separation` | new | F31–F33, M-50..M-52 | No struct under `crates/*/src/**/store/types/row/*.rs` or `crates/*/src/**/types/row/*.rs` derives `Serialize` or `Deserialize`. |
| `rename-all-policy` | new | F39, M-55 | Rust serde attributes: snake_case (default) or camelCase (outward HTTP only), never kebab-case or PascalCase. |
| `turso-import-isolation` | extends existing `vox ci turso-import-guard` | F9, M-61 | `turso::` / `libsql::` imports appear only in the `data-storage-policy.v1.yaml::tiers.a_relational.allow_direct_access` allowlist. |
| `env-parity` | new | F46, M-58 | Every `env::var("VOX_…")` / `env::var_os("VOX_…")` match in `crates/**/*.rs` is listed in `contracts/config/env-vars.v1.yaml` (file created by M-13). |
| `telemetry-event-codegen-drift` | new | F50, M-42 | `contracts/telemetry/events.v1.yaml` regenerates the Rust event enum byte-for-byte (requires M-42's codegen). |
| `span-registry-parity` | new | F51, M-43 | Every span name in `contracts/telemetry/spans.v1.yaml` (created by M-43) is referenced somewhere in the workspace; vice versa. |
| `db-path-isolation` | new | F59, M-65 | String literals `store.db`, `clavis_vault.db`, `research-audit-codex.db` appear only in `vox-config`, `vox-db`, `vox-clavis`, `vox-test-harness`. |
| `vox-db-doctor` | new | F6, M-24 | Optional (nightly, not PR) — runs `vox db doctor --json` on a synthetic DB; asserts schema, digest, orphan count. |
| `vox-modules-local-db` | new | F61, M-26, M-67 | `.vox_modules/local_store.db` is opened via `vox_db`/`vox_config::paths` (no raw string literal) everywhere except `vox-pm` during its transition window. |
| `abandoned-target-dir` | new | F64, M-70 | No tracked path matches `^target-[a-z]+/`; the policy YAML's `forbidden_root_glob_prefixes: ["target-"]` carries the list. |
| `dist-schemas-drift` | new | F63, M-69 | After `vox schema generate --ts`, `dist/schemas.ts` is byte-identical to the regenerated output. Enforced only when `generated_files[].enforced: true` (flipped by M-69). |
| `bom-config-files` | new | F68, M-74 | `.gitignore`, `.gitattributes`, `rust-toolchain.toml`, `contracts/**/*.yaml` do not start with a UTF-8 BOM (`0xEF 0xBB 0xBF`). |
| `frozen-core-ddl-guard` | extends existing `vox ci check-frozen` | F62, M-68 | Row-struct or DDL changes in a crate under `data-storage-policy.v1.yaml::frozen_core_crates` require the governance token from `crates/_frozen.md` in the PR body. |
| `grammar-ssot-drift` | extends existing `vox ci grammar-ssot-parity` | F67, M-72 | Any change to `tree-sitter-vox/grammar.js` or `tree-sitter-vox/src/grammar.json` requires a sibling edit to `tree-sitter-vox/GRAMMAR_SSOT.md`. |
| `forbidden-file-exceptions` | new | F66 | Every `temporary_exceptions` / `temporary_file_exceptions` entry in `data-storage-policy.v1.yaml` has an `expiry`, `retired_by: M-NN`, and `owner`; entries past their `expiry` window fail. |

All checks are idempotent. None create state. The dispatcher emits one report row per check so CI annotators can surface only the failing lines. The fast path (everything except `schema-codegen-drift`, `telemetry-event-codegen-drift`, `dist-schemas-drift`, `vox-db-doctor`) runs in under 5 s on a cold workspace.

### 1.4 Scaffolded vs enforcing

Each check has two legal states:

- **`scaffolded`** — the check runs but never returns `fail`. It emits a `status: "scaffolded"` row with the tracking migration item. This is the state on first landing (M-04) for every row where the target-state infrastructure is not yet in place (e.g., `schema-codegen-drift` before M-10 ships the codegen).
- **`enforcing`** — `fail` causes non-zero exit.

Flip is one-line: changing `Severity::Scaffolded` → `Severity::Error` in `grep_rules.rs` for grep rules, or returning `Err` instead of `Ok(Scaffolded)` for handwritten checks. The migration backlog's per-ticket "Verification" step explicitly names the flip.

### 1.5 Report schema (sketch)

The actual JSON Schema lives at `contracts/db/data-storage-guard-report.v1.schema.json` (created by M-04 step 3). Shape:

```jsonc
{
  "$id": ".../contracts/db/data-storage-guard-report.v1.schema.json",
  "x-vox-version": 1,
  "type": "object",
  "required": ["checks", "summary"],
  "properties": {
    "checks": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "status"],
        "properties": {
          "name":       { "type": "string" },
          "status":     { "enum": ["pass", "fail", "skip", "scaffolded"] },
          "finding_id": { "type": "string", "pattern": "^F[0-9]+$" },
          "violations": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["path", "message"],
              "properties": {
                "path":     { "type": "string" },
                "line":     { "type": "integer" },
                "message":  { "type": "string" },
                "fix_hint": { "type": "string" }
              }
            }
          }
        }
      }
    },
    "summary": {
      "type": "object",
      "required": ["total", "passed", "failed", "scaffolded"],
      "properties": {
        "total":      { "type": "integer" },
        "passed":     { "type": "integer" },
        "failed":     { "type": "integer" },
        "scaffolded": { "type": "integer" }
      }
    }
  }
}
```

## 2. Clippy additions

The existing `clippy.toml` contains exactly two lines (`too-many-arguments-threshold = 12`, `type-complexity-threshold = 600`). This spec appends targeted denies, not tunings — data-storage violations are categorical, not statistical.

### 2.1 Workspace `clippy.toml` additions (M-56, step 2)

Append the following after the existing two lines; no existing content changes.

```toml
# Data-storage enforcement — see docs/src/architecture/data-storage-lint-and-ci-spec-2026.md §2.
disallowed-methods = [
  { path = "futures::executor::block_on",
    reason = "Use tokio runtime; see SSOT F41 and the vox-db block_on abstraction." },
  { path = "tokio::task::block_in_place",
    reason = "Only the vox-db block_on abstraction may use this; see policy allowlist in contracts/db/data-storage-policy.v1.yaml::rust_policy.block_in_place_allowlist (F41)." },
  { path = "std::fs::write",
    reason = "Do not write Tier B/C data through std::fs. Use vox-spool (Tier B) or vox-checksum-manifest (Tier C). Allowlist in contracts/db/data-storage-policy.v1.yaml::rust_policy.direct_fs_write_allowlist (F22)." },
  { path = "tokio::fs::write",
    reason = "See std::fs::write." },
]
```

Clippy's `disallowed-methods` is workspace-global. Per-file exemptions are expressed inline with `#[allow(clippy::disallowed_methods)]` plus a mandatory rustdoc comment citing the SSOT finding, e.g.:

```rust
/// Allowed here: see SSOT F41 and clippy-allowlist in
/// contracts/db/data-storage-policy.v1.yaml::rust_policy.block_in_place_allowlist.
#[allow(clippy::disallowed_methods)]
fn block_on<F: Future>(f: F) -> F::Output { /* … */ }
```

A new `data-storage-guard` sub-check `clippy-allowlist-parity` (added by M-56 step 3) asserts that every `#[allow(clippy::disallowed_methods)]` occurrence is either (a) in a file listed in the YAML allowlist, or (b) flagged as a violation. The YAML, not scattered rustdoc, is the single source of truth.

### 2.2 Crate-root deny lints

No new `#![deny(...)]` attributes are added in this PR. The crates that would carry them (`vox-db`, `vox-orchestrator`, `vox-spool`) are either already governed by the workspace-wide `-D warnings` in `.gitlab-ci.yml` L99 or do not yet exist (`vox-spool` lands in M-03). Revisit after M-03 merges.

### 2.3 Allowlists

All allowlists live in one place: `contracts/db/data-storage-policy.v1.yaml`. Do not scatter per-crate `clippy.toml` files or per-file comments that encode policy — comments may *reference* the YAML entry, never replace it.

Current (HEAD) policy-YAML allowlist contents — the file ships this already:

- `block_in_place_allowlist`:
  - `crates/vox-db/src/store/ops.rs` (F41 — the encapsulated sync/async abstraction)
  - `crates/vox-db/src/research.rs` (temporary; tracked to be replaced in M-56 follow-ups)
  - `crates/vox-orchestrator/src/session/manager/db_io.rs` (clean use, documented pattern)
- `direct_fs_write_allowlist`:
  - `crates/vox-config/**` (legitimate config file writes)
  - `crates/vox-spool/**` (the Tier B implementation itself, lands in M-03)
  - `crates/vox-checksum-manifest/**` (release-asset integrity verifier; Tier C role deferred to M-29-spike)
  - `crates/vox-cli/src/commands/init.rs` (one-time scaffolding)
  - `tests/**` (test fixtures)

M-56 tightens this allowlist; any later reduction is a policy-YAML diff, not a code edit.

## 3. `cargo-deny` additions

The existing `deny.toml` has `[bans]` with `multiple-versions = "warn"`, `highlight = "all"`, `wildcards = "allow"`. No `[[bans.deny]]` entries exist today. The following additions harden the relational plane and forbid re-introducing retired storage libs.

### 3.1 Ban direct libSQL/turso imports outside allowlist (F9, M-61)

```toml
# deny.toml addition — appended to [bans]
wildcards = "deny"  # tighten from current "allow"; verify via `cargo deny check bans` in the landing PR

# Require crates that depend on the libSQL client to be in a named allowlist.
[[bans.deny]]
name = "turso"
wrappers = ["vox-db", "vox-clavis", "vox-test-harness"]

[[bans.deny]]
name = "libsql"
wrappers = ["vox-db", "vox-clavis", "vox-test-harness"]

# vox-pm is temporarily allowed direct libSQL use via contracts/db/data-storage-policy.v1.yaml::tiers.a_relational.temporary_exceptions.
# That exception is removed from both the YAML and this wrappers list in the same PR as M-26.
```

`cargo-deny`'s `wrappers` mechanism forbids a transitive dependency anywhere except when the listed crates are the direct importer — exactly the isolation property SSOT F9 requires. The existing `vox ci turso-import-guard` (see `crates/vox-cli/src/commands/ci/cmd_enums.rs` L247) complements this by scanning Rust source; the deny check catches dependency-graph leaks that source-grep cannot.

### 3.2 Prevent re-introduction of retired storage libraries (F9)

Neither `sled`, `heed`, nor `redb` are allowed as workspace dependencies — Vox's relational plane is libSQL, and pure-KV needs of any crate should be met by a libSQL table via `vox-db`.

```toml
[[bans.deny]]
name = "sled"
[[bans.deny]]
name = "heed"
[[bans.deny]]
name = "redb"
```

### 3.3 CI wiring

`cargo deny check` is not currently invoked by `.gitlab-ci.yml` or `.github/workflows/ci.yml` (grep confirms). M-62 adds:

- A new GitLab job `cargo-deny` (stage `check`, mirrors the `clippy:` job on L95–99), script: `cargo install --locked cargo-deny && cargo deny check licenses sources bans`.
- A GitHub step in the same job as the other guards: `run: cargo deny check bans` (licenses/sources are slower and gated to nightly).

## 4. Grep-based CI checks (stateless, in-tree)

Several lints are easier and cheaper to express as ripgrep patterns than as clippy lints. They all run inside `vox ci data-storage-guard` via the in-repo `grep` crate (not the shell-out ripgrep) so the implementation is portable and does not require ripgrep on developer machines. Pattern table is declarative (§4.2); the runner is shared with the grep/guard helpers in `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs` (home of `run_sql_surface_guard`, `run_turso_import_guard`, etc.) — consumed by the `sql-surface-guard`, `query-all-guard`, `turso-import-guard`, `secret-env-guard`, and `operator-env-guard` clap variants in `cmd_enums.rs`.

### 4.1 Rules (21 total)

Each rule is listed as `(name, pattern, glob, where-not-allowed, motivating-finding, tracking-ticket)`.

1. **`direct-db-path-literal`** — pattern: `store\.db|clavis_vault\.db|research-audit-codex\.db` — glob: `crates/**/*.rs` — **not allowed in** any crate except `vox-config`, `vox-db`, `vox-clavis`, `vox-test-harness`. Finding: F59. Ticket: M-65.
2. **`hardcoded-vox-subdir`** — pattern: `"\.vox/"` — glob: `crates/**/*.rs` — **not allowed in** any crate except `vox-config`. Finding: F43. Ticket: M-57.
3. **`serde-on-row-struct`** — pattern: `#\[derive\([^)]*Deserialize` — glob: `crates/*/src/**/store/types/row/*.rs`, `crates/*/src/**/types/row/*.rs`. Finding: F31. Ticket: M-50.
4. **`libsql-value-outside-vox-db`** — pattern: `libsql::Value` — glob: `crates/**/*.rs` — **not allowed in** any crate except `vox-db`. Finding: F31. Ticket: M-50.
5. **`raw-env-var-vox`** — pattern: `env::var(_os)?\s*\(\s*"VOX_` — glob: `crates/**/*.rs` — **check**: every match must be listed in `contracts/config/env-vars.v1.yaml` (created by M-13). Finding: F46. Ticket: M-58.
6. **`raw-tracing-subscriber-init`** — pattern: `tracing_subscriber::fmt\(\)` or `Registry::default\(\)` — glob: `crates/**/*.rs` — **not allowed in** any crate except `vox-runtime::observability`, test harnesses. Finding: F49. Ticket: M-40.
7. **`std-fs-write-json`** — pattern: `(std|tokio)::fs::write\b.*\bserde_json::` — multiline — **not allowed** anywhere. Use `vox-spool` (Tier B) or `vox-checksum-manifest` (Tier C). Finding: F22. Ticket: M-03.
8. **`append-only-file-open`** — pattern: `OpenOptions::new\(\)[^;]*\.append\(\s*true\s*\)` — multiline — **not allowed in** any crate except `vox-spool`, `vox-runtime::observability`, `vox-checksum-manifest`, `tests/**`. Finding: F30. Ticket: M-03.
9. **`retired-env-var`** — pattern: legacy Turso-prefixed DB env aliases + `VOX_TELEMETRY_SPOOL_DIR` (exact spellings in env-vars SSOT; omitted here to satisfy docs hygiene guards) — **warn in** `vox-clavis` (sunset window), **fail in** everywhere else. Finding: F4. Ticket: M-28.
10. **`block-on-leak`** — pattern: `futures::executor::block_on` — glob: `crates/**/*.rs` — **not allowed** anywhere. (Duplicates clippy `disallowed-methods` so the guard reports even when clippy is skipped.) Finding: F41. Ticket: M-56.
11. **`gitignored-tracked-json`** — check: `git ls-files` does not contain `vox-agent.json`, `vox-schema.json`, or `vox.tokens.json`. Finding: F26. Ticket: M-07.
12. **`repo-root-stray`** — check: `ls` at repo root contains none of the entries in `data-storage-policy.v1.yaml::repo_hygiene.forbidden_root_files` or any glob match against `forbidden_root_globs`. Finding: F25. Ticket: M-06.
13. **`renameAll-kebab`** — pattern: `serde\(rename_all\s*=\s*"kebab-case"` — glob: `crates/**/*.rs` — **not allowed** anywhere. Finding: F39. Ticket: M-55.
14. **`renameAll-PascalCase`** — pattern: `serde\(rename_all\s*=\s*"PascalCase"` — glob: `crates/**/*.rs` — **not allowed** anywhere. Finding: F39. Ticket: M-55.
15. **`empty-schemas-dir`** — check: `test ! -d schemas` (verified against `forbid_existence` in the policy YAML). Finding: F15. Ticket: M-05.
16. **`vox-modules-local-db-literal`** — pattern: `\.vox_modules/local_store\.db|\.vox_modules"` — glob: `crates/**/*.rs` — **not allowed in** any file after M-26 lands. HEAD grep (2026-04-21) shows three call-site families: (a) the existing helper `open_local_pm_store()` at `vox-cli/src/commands/pm_lifecycle.rs:10–14` which hand-builds the path; (b) three callers that bypass the helper with raw string literals — `vox-cli/src/commands/search.rs:34`, `vox-cli/src/commands/diagnostics/tools/search.rs:35`, `vox-cli/src/commands/info.rs:33`; (c) context-message mentions in `update.rs:17`, `sync.rs:33`, `lock.rs:11`. M-26 (i) moves path construction into `vox_config::paths::local_pm_store_db()`, (ii) rewrites the three raw-literal callers to go through `open_local_pm_store()`, and (iii) changes the three `.context("open .vox_modules/...")` message strings to reference the helper's name instead of the path. Finding: F61. Ticket: M-26.
17. **`abandoned-target-dir`** — check: `git ls-files | rg '^target-[a-z]+/'` is empty; `.gitignore` contains `target-*/`. Finding: F64. Ticket: M-70.
18. **`dist-schemas-drift`** — check: after `vox schema generate --ts`, `git diff --exit-code dist/schemas.ts`. Enforced only when `data-storage-policy.v1.yaml::generated_files[0].enforced: true` (flipped by M-69). Finding: F63. Ticket: M-69.
19. **`bom-config-files`** — check: first three bytes of `.gitignore`, `.gitattributes`, `rust-toolchain.toml`, `contracts/**/*.yaml` are not `0xEF 0xBB 0xBF`. Finding: F68. Ticket: M-74.
20. **`frozen-core-ddl-guard`** — check: any diff touching `crates/<frozen_core_crate>/src/**/schema/**` or adding/removing `row`/`dto` module files for a crate in `data-storage-policy.v1.yaml::frozen_core_crates` must carry the governance token string `FROZEN-CORE-TOKEN: <sha256>` (format per `crates/_frozen.md`) in the PR body or top commit message. The existing `vox ci check-frozen` (`crates/vox-cli/src/commands/ci/frozen_crates.rs`) covers "no new crate added"; this rule covers "no DDL change without token". Finding: F62. Ticket: M-68.
21. **`grammar-ssot-drift`** — check: any diff touching `tree-sitter-vox/grammar.js` or `tree-sitter-vox/src/grammar.json` must also touch `tree-sitter-vox/GRAMMAR_SSOT.md`. The existing `vox ci grammar-ssot-parity` (`crates/vox-cli/src/commands/ci/grammar_ssot_parity.rs`) validates parity of keyword lists; this rule adds the sibling-edit requirement. Finding: F67. Ticket: M-72.

### 4.2 Rule table location

Each rule is a row in `crates/vox-cli/src/commands/ci/data_storage_guard/grep_rules.rs`:

```rust
// crates/vox-cli/src/commands/ci/data_storage_guard/grep_rules.rs
use crate::commands::ci::grep_runner::{GrepRule, Severity};

pub const RULES: &[GrepRule] = &[
    GrepRule {
        name: "direct-db-path-literal",
        finding_id: "F59",
        ticket:     "M-65",
        pattern:    r"store\.db|clavis_vault\.db|research-audit-codex\.db",
        glob:       Some("crates/**/*.rs"),
        allow_in:   &[
            "crates/vox-config/",
            "crates/vox-db/",
            "crates/vox-clavis/",
            "crates/vox-test-harness/",
        ],
        severity: Severity::Scaffolded, // flipped to Error by M-65 step 5
    },
    // ... remaining 20 rules, same shape
];
```

The runner reads `contracts/db/data-storage-policy.v1.yaml` at startup and merges the `allow_in` set with the policy's `allow_direct_access` / `temporary_exceptions` lists — a single policy edit removes an exception without code churn.

## 5. `contracts/db/data-storage-policy.v1.yaml` (shipped at HEAD, enriched in this PR)

This file **already exists** at HEAD (shipped by a prior PR, 154 lines). The current contents cover tiers, env vars, rust policy, repo hygiene, contract policy, ignored paths, frozen-core crates, forbidden-root-glob prefixes, and generated files. The schema validator `contracts/db/data-storage-policy.v1.schema.json` is **not yet shipped** — M-04 step 4 creates it.

### 5.1 Corrections required before M-04 lands

The HEAD file carries two stale rows that the rewritten SSOT and backlog contradict:

```yaml
# contracts/db/data-storage-policy.v1.yaml — CURRENT (HEAD), lines 28-41
    temporary_exceptions:
      - vox-pm            # retained — correct (retires with M-26)
    temporary_file_exceptions:
      - path: .vox_modules/local_store.db
        owner: vox-pm
        retired_by: M-67
        expiry: "2026-Q3"
      - path: vox_hardened.db
        owner: vox-clavis
        retired_by: M-26
        expiry: "2026-Q3"
```

Two fixes, landed in the same PR as M-04:

1. **`.vox_modules/local_store.db` is ACTIVE, not deprecated.** It is opened by four `vox-cli` commands (update.rs:26, sync.rs:42, search.rs:43, pm_lifecycle.rs:20). M-26 routes those opens through `vox-db` + `vox-config::paths` but does not remove the file. Therefore: `retired_by: M-67` is correct (M-67 folds the module state into Tier A), but `expiry` should be bumped to align with M-67's actual landing release — not left as a stale "2026-Q3" placeholder.
2. **`vox_hardened.db` is an ORPHAN artifact with zero crate references.** It is not owned by `vox-clavis` and not migrated by M-26. M-25 *deletes* it. The entry should be removed from `temporary_file_exceptions` and the forbidden-file list updated so a new copy cannot be recreated.

M-04 step 2 applies these two edits plus adds a `forbidden-file-exceptions` schema invariant (§1.3 row) requiring each entry to carry `owner`, `retired_by: M-NN`, and `expiry` — catching stale entries automatically.

### 5.2 Validator

`contracts/db/data-storage-policy.v1.schema.json` (created by M-04 step 4) validates the YAML against the known rust-policy shapes, finds the malformed exceptions, and is itself grepped by `contracts-index` to ensure the companion entry in `contracts/index.yaml` is present.

## 6. CI wiring

### 6.1 `.gitlab-ci.yml` — extend the `vox-ci-guards` job

The existing job at L56–81 runs 21 cargo invocations sequentially. Append one line plus an `artifacts:` stanza:

```yaml
# .gitlab-ci.yml (diff against L56–81)
 vox-ci-guards:
   extends: .base
   stage: check
   script:
     - cargo build -p vox-cli
     - cargo run -p vox-cli --quiet -- ci line-endings
     - cargo run -p vox-cli --quiet -- ci manifest
     - cargo run -p vox-cli --quiet -- ci check-codex-ssot
     - cargo run -p vox-cli --quiet -- ci check-docs-ssot
     - cargo run -p vox-cli --quiet -- ci command-compliance
     - cargo run -p vox-cli --quiet -- ci doc-inventory verify
     - cargo run -p vox-cli --quiet -- ci eval-matrix verify
     - cargo run -p vox-cli --quiet -- ci eval-matrix run --milestone m3-dei-contracts
     - cargo check -p vox-cli --features gpu
     - cargo run -p vox-cli --quiet -- ci workflow-scripts
     - cargo test -p vox-repository --lib && cargo test -p vox-orchestrator --lib detect_layout_node_workspaces && cargo test -p vox-mcp --lib && cargo check -p vox-git
     - cargo test -p vox-populi --features transport
     - cargo test -p vox-workflow-runtime
     - cargo check -p vox-cli --features mesh,workflow-runtime
     - cargo run -p vox-cli --quiet -- ci build-timings --crates
     - cargo run -p vox-cli --quiet -- ci feature-matrix
     - cargo run -p vox-cli --quiet -- ci no-vox-dei-import
     - cargo run -p vox-cli --quiet -- ci toestub-scoped --mode legacy
     - cargo run -p vox-cli --quiet -- ci cuda-features
     - cargo run -p vox-cli --quiet -- ci mens-gate --profile ci_full
+    - cargo run -p vox-cli --quiet -- ci data-storage-guard --json > artifacts/data-storage-guard.json
+  artifacts:
+    paths:
+      - artifacts/data-storage-guard.json
+    when: always
+    expire_in: 1 week
```

### 6.2 `.github/workflows/ci.yml` — add next to `data-ssot-guards`

The existing workflow runs `data-ssot-guards` at L67–68. Add the new guard directly beneath, in the same job (no parallel matrix — keeps the report artifact co-located):

```yaml
# .github/workflows/ci.yml (diff after L68)
       - name: Data / telemetry SSOT guards
         run: cargo run -p vox-cli --quiet -- ci data-ssot-guards

+      - name: Data storage guard (tiers A/B/C/D, contract drift, repo hygiene)
+        run: cargo run -p vox-cli --quiet -- ci data-storage-guard --json | tee data-storage-guard.json
+
+      - name: Upload data-storage guard report
+        if: always()
+        uses: actions/upload-artifact@v7
+        with:
+          name: data-storage-guard
+          path: data-storage-guard.json
```

### 6.3 Nightly job (`vox db doctor`)

`vox db doctor` is slow (seeds a DB, walks every domain fragment), so it does not run on every PR. Add a new workflow at `.github/workflows/nightly-doctor.yml` (file does not exist today):

```yaml
name: nightly-doctor
on:
  schedule: [{ cron: "0 6 * * *" }]   # 06:00 UTC daily
  workflow_dispatch:
jobs:
  db-doctor:
    runs-on: [self-hosted, linux]
    steps:
      - uses: actions/checkout@v6
      - run: cargo run -p vox-cli --quiet -- init --data-dir ${{ github.workspace }}/.ci-data
        env:
          VOX_DATA_DIR: ${{ github.workspace }}/.ci-data
      - run: cargo run -p vox-cli --quiet -- db doctor --json
        env:
          VOX_DATA_DIR: ${{ github.workspace }}/.ci-data
      - run: cargo run -p vox-cli --quiet -- ci data-storage-guard --check schema-codegen-drift --check telemetry-event-codegen-drift
```

M-24 lands the `vox db doctor --json` surface.

### 6.4 Pre-commit hook (opt-in, developer-local)

The repo's existing pre-commit infrastructure is `vox ci install-hooks` (`cmd_enums.rs` L432). Extend it to install an additional fast-subset guard:

```toml
# Snippet added to the generated .git/hooks/pre-commit by `vox ci install-hooks`
# (not to a .pre-commit-config.yaml — this repo does not use pre-commit.com).
cargo run -p vox-cli --quiet -- ci data-storage-guard \
  --check repo-root-strays \
  --check gitignored-but-tracked \
  --check retired-env-var \
  --check bom-config-files \
  --check abandoned-target-dir
```

Only the stateless, sub-second checks are in the fast subset; codegen-drift checks are deferred to CI.

## 7. `.cursor/rules/data-storage-policy.mdc` (shipped at HEAD, aligned in this PR)

This file exists at HEAD (33 lines, prior PR). Its non-negotiables already cover: no direct `turso::Connection` outside the allowlist, no `std::fs::write` for event data, no `env::var("TURSO_…")`, no hard-coded `.vox/` subpaths, no `Serialize`/`Deserialize` on `row/` structs, no unregistered `VOX_*` env vars, no handwritten Rust struct for a shape that already has a contract.

Two corrections needed so the rule matches the rewritten SSOT:

1. Remove the `regenerate from vox-schema instead` clause — there is no `vox-schema` crate. Replace with `regenerate via 'vox schema generate' (handled by vox-jsonschema-util::codegen after M-10)`.
2. Add a non-negotiable: `Do NOT add a row to contracts/db/data-storage-policy.v1.yaml::temporary_*_exceptions without owner + retired_by + expiry.`

No `globs:` line was present at HEAD (prior draft implied one); Cursor's default scope is adequate for this rule.

## 8. `.markdownlint.jsonc`

No additions needed. Existing tolerances (long lines for contract paths, frontmatter `title` as H1) are adequate for these docs.

## 9. Schema-drift release gate

When cutting a release, run the full guard plus the extended doctor:

```shell
vox ci data-storage-guard --json > artifacts/release-guard.json
vox db doctor --json           > artifacts/release-doctor.json
vox schema generate --verify --verbose
```

A release is blocked if any of the three exit non-zero. M-30 wires this into the existing `release-binaries.yml` and `.gitlab-ci.yml` release stage — no separate pipeline.

## 10. Rollout plan

1. **M-00** (this PR) — land SSOT + backlog + this spec together. No code change yet.
2. **M-01..M-09 (Phase 0)** — scaffolding. Land `vox ci data-storage-guard` with every sub-check returning `scaffolded`. The job is wired into both `.gitlab-ci.yml` and `.github/workflows/ci.yml` and emits artifacts, but cannot fail CI.
3. **M-10..M-19 (Phase 1)** — contracts surface flipping. Each ticket's Verification step explicitly flips its owning check from `Scaffolded` → `Error`.
4. **M-20..M-49 (Phases 2–4)** — data plane consolidation; checks flip per-ticket.
5. **M-50..M-59 (Phase 5)** — row/wire separation; `rename-all-policy`, `row-wire-separation`, `block-on-leak` flip.
6. **M-60..M-66 (Phase 6)** — tier-boundary enforcement; `turso-import-isolation`, `db-path-isolation` flip.
7. **M-67..M-74 (Phase 7)** — cleanup; `abandoned-target-dir`, `dist-schemas-drift`, `bom-config-files` flip.
8. After Phase 7 completes, `--check scaffolded` returns no rows; the guard is fully live. One release later, the `TURSO_*` and `VOX_TELEMETRY_SPOOL_DIR` deprecated aliases are removed (M-28b). Six months after full-live, mark this doc `status: current` and drop §0 and §10.

## 11. Appendix: regex cheat sheet for `grep_rules.rs`

All regex-shaped rules, as Rust raw strings. Paste straight into the `RULES` table. Non-regex rules (17, 18, 19, 20, 21) are implemented as procedural checks in `data_storage_guard::checks/`.

```text
# name                           pattern (as Rust raw string, regex-compatible)
direct-db-path-literal           r"store\.db|clavis_vault\.db|research-audit-codex\.db"
hardcoded-vox-subdir             r#""\.vox/""#
serde-on-row-struct              r"#\[derive\([^)]*Deserialize"          # path-scoped via glob
libsql-value-outside-vox-db      r"libsql::Value"
raw-env-var-vox                  r#"env::var(_os)?\s*\(\s*"VOX_"#
raw-tracing-subscriber-init      r"tracing_subscriber::fmt\(\)|Registry::default\(\)"
std-fs-write-json                r"(std|tokio)::fs::write\b.*\bserde_json::"   # multiline
append-only-file-open            r"OpenOptions::new\(\)[^;]*\.append\(\s*true\s*\)"
retired-env-var                  r"TURSO_URL|VOX_TURSO_URL|VOX_TURSO_TOKEN|TURSO_AUTH_TOKEN|VOX_TELEMETRY_SPOOL_DIR"
block-on-leak                    r"futures::executor::block_on"
renameAll-kebab                  r#"serde\(rename_all\s*=\s*"kebab-case""#
renameAll-PascalCase             r#"serde\(rename_all\s*=\s*"PascalCase""#
vox-modules-local-db-literal     r"\.vox_modules/local_store\.db"
```

## 12. Verification pass (to be run before merging)

For reviewers. Every citation in §1–§11 should resolve against HEAD. Run:

```shell
# Paths
for p in \
  crates/vox-cli/src/commands/ci/cmd_enums.rs \
  crates/vox-cli/src/commands/ci/run_body.rs \
  crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs \
  crates/vox-cli/src/commands/ci/frozen_crates.rs \
  crates/vox-cli/src/commands/ci/grammar_ssot_parity.rs \
  crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs \
  crates/vox-jsonschema-util/src/lib.rs \
  crates/vox-checksum-manifest/src/lib.rs \
  crates/vox-runtime/src/observability.rs \
  crates/vox-cli-core/src/lib.rs \
  clippy.toml deny.toml \
  contracts/db/data-storage-policy.v1.yaml \
  contracts/db/baseline-version-policy.yaml \
  .cursor/rules/data-storage-policy.mdc \
  .github/workflows/ci.yml .gitlab-ci.yml ; do
    test -e "$p" || echo "MISSING: $p"
done

# CI sub-command names must exist as clap variants
rg -nF 'CiCmd::' crates/vox-cli/src/commands/ci/cmd_enums.rs | wc -l      # expect > 40

# Paths referenced as future-state (must NOT exist, ticket must create them)
for p in \
  crates/vox-spool/ \
  crates/vox-cli/src/commands/ci/data_storage_guard/ \
  contracts/config/env-vars.v1.yaml \
  contracts/db/data-storage-guard-report.v1.schema.json \
  contracts/db/data-storage-policy.v1.schema.json \
  .github/workflows/nightly-doctor.yml ; do
    test -e "$p" && echo "EXISTS (should be created by ticket): $p"
done
```

Green ledger → this spec is faithful to HEAD. Red ledger → fix the spec before merging (do not silently retcon paths).
 