---
title: "Data Storage Migration Backlog (2026)"
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Backlog for the data storage migration"
audience: ["contributors", "agents"]
---

# Data Storage Migration Backlog (2026)

> **Update (2026-06-03):** The parallel GitLab CI mirror (`.gitlab-ci.yml`) has been retired and the file deleted. Every migration step (M-NN) below that previously wired a check into the GitLab `vox-ci-guards` job now targets `.github/workflows/ci.yml` only. All GitHub Actions guidance is unchanged.

## Execution order (authoritative)

The omni-agent reads this list top-down. An item is "landed" when `git log --all --grep "^M-NN\b" --pretty=oneline` returns at least one commit. The agent skips landed items and executes the first un-landed item whose listed blockers are all landed.

1. M-00, M-01, M-02, M-03, M-04, M-05, M-06, M-07, M-08, M-09    *(Phase 0 — scaffolding)*
2. M-20                                                            *(Phase 1b — baseline re-sync; promoted ahead of Phase 1 due to widening BASELINE_VERSION drift, see F1)*
3. M-10, M-11, M-12, M-13, M-14, M-15, M-16, M-17, M-18, M-19    *(Phase 1 — contracts surface; skip M-20, already landed)*
4. M-21, M-22, M-23, M-24, M-25, M-26, M-27, M-28, M-29-spike, M-29, M-30  *(Phase 2 — DDL flip; note M-29-spike before M-29)*
5. M-31, M-32, M-33, M-34, M-35, M-36, M-37, M-38, M-39          *(Phase 3 — Tier B/C rollout)*
6. M-40, M-41, M-42, M-43, M-44, M-45, M-46, M-47, M-48, M-49    *(Phase 4 — observability)*
7. M-50, M-51, M-52, M-53, M-54, M-55, M-56, M-57, M-58, M-59    *(Phase 5 — memory hygiene)*
8. M-60, M-61, M-62, M-63, M-64, M-65, M-66                      *(Phase 6 — regression gates)*
9. M-67, M-68, M-69, M-70, M-71, M-72, M-73, M-74                *(Phase 7 — gap cleanup)*
10. M-75, M-76, M-77, M-78                                        *(Phase 7b — reconcile orchestration surface; added 2026-04-21)*

Deferred side-items (M-55b, M-55c, M-57d) are opt-in and not part of the main sweep.

Ticket-level breakdown of the work described in [data-storage-ssot-2026.md](data-storage-ssot-2026.md). Every ticket has:

- **Owner** — default claimant.
- **Blast radius** — H/M/L how much of the workspace changes.
- **Blockers** — tickets that must complete first.
- **SSOT findings** — cross-reference into the SSOT.
- **Sub-steps** — numbered, each with a concrete file path or function name so a future LLM or human contributor can execute without re-deriving context.
- **Verification** — exactly how to tell it's done (a grep, a diff, a CI check, a test, or a doctor sub-command).

Every ticket ends by pointing at the guard sub-check or grep rule that prevents regression.

## Phase 0 — Scaffolding

### M-00 · Land the SSOT triplet

- **Owner**: Governance.
- **Blast radius**: L (doc-only).
- **Blockers**: none.
- **SSOT findings**: F56, F62–F74.
- **Sub-steps**:
  1. Write `docs/src/architecture/data-storage-ssot-2026.md` (this series).
  2. Write `docs/src/architecture/data-storage-migration-backlog-2026.md` (this document).
  3. Write `docs/src/architecture/data-storage-lint-and-ci-spec-2026.md`.
  4. Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
  5. Link from `docs/src/SUMMARY.md` if it exists (check first).
- **Verification**: `rg '^# Data Storage SSOT' docs/src/architecture/` returns exactly the three docs.

> Landed in a029121a on 2026-04-22; verification: green.

### M-01 · Scaffold `vox ci data-storage-guard` sub-command (stub)

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-00.
- **SSOT findings**: F56.
- **Sub-steps**:
  1. Add a new variant to the `CiCmd` enum at `crates/vox-cli/src/commands/ci/cmd_enums.rs` named `DataStorageGuard`.
  2. Create `crates/vox-cli/src/commands/ci/data_storage_guard.rs` with a `pub fn run(opts: &GuardOpts) -> anyhow::Result<GuardReport>` stub that logs "stub" and returns `Ok(GuardReport::empty())`.
  3. Register the handler in `crates/vox-cli/src/commands/ci/mod.rs` following the pattern of `contracts_index.rs` or `exec_policy_contract.rs`.
  4. Add a `GuardReport` type that mirrors `contracts/db/data-storage-guard-report.v1.schema.json` (authored in M-04).
  5. Wire a no-op invocation into the `.github/workflows/ci.yml` guard job so the command exists but is green.
- **Verification**: `vox ci data-storage-guard --help` succeeds locally; the GitHub Actions check job on a PR passes with a stub log line.

> Landed in 1fa8aa2c on 2026-04-22; verification: green.

### M-02 · Land `.cursor/rules/data-storage-policy.mdc` (NET-NEW)

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: M-00.
- **SSOT findings**: §3 non-negotiables.
- **Sub-steps**:
  1. Confirm the file does not exist (it is net-new from this work): `ls .cursor/rules/data-storage-policy.mdc`.
  2. Draft using the 8 sibling `.mdc` files as format template (`build-environment.mdc`, `ci-runner-convention.mdc`, etc.).
  3. Include a `description:` frontmatter and `alwaysApply: true`.
  4. List non-negotiables from SSOT §3 verbatim.
  5. Cross-link to the SSOT + lint spec via `mdc:` links.
- **Verification**: `cat .cursor/rules/data-storage-policy.mdc` is non-empty.

> Landed in df8fdb32 on 2026-04-22; verification: green.

### M-03 · Extract `vox-cli::telemetry_spool` into new `vox-spool` crate

- **Owner**: Data Core + CLI.
- **Blast radius**: M.
- **Blockers**: M-00.
- **SSOT findings**: F21, F22, F30, F45.
- **Sub-steps**:
  1. Read `crates/vox-cli/src/telemetry_spool.rs` in full and inventory every public function.
  2. Create new crate `crates/vox-spool/` via `cargo new --lib crates/vox-spool` (or write the `Cargo.toml` and `src/lib.rs` directly). This crate is born in this ticket — it did not pre-exist.
  3. Add `vox-spool = { path = "crates/vox-spool" }` to the workspace `[workspace.dependencies]` table in the root `Cargo.toml`.
  4. Move the existing queue logic into `vox_spool::queue::{enqueue, pending_dir, spool_root}` (preserving behavior).
  5. Add new JSONL API: `vox_spool::jsonl::{SpoolWriter, SpoolReader, ChannelName, RotationPolicy, FsyncPolicy, SpoolConfig}`.
  6. Implement `ChannelName::new(raw)` with regex `^[a-z][a-z0-9-]{0,31}$`.
  7. Implement `SpoolWriter::append(&mut self, &str) -> Result<()>` with rotation, fsync-per-rotation default, atomic rename.
  8. Re-export queue surface from `crates/vox-cli/src/telemetry_spool.rs` as thin wrappers that call `vox_spool::queue::*`; schedule removal in 0.6.
  9. Write unit tests for rotation boundary (hour change), channel validation, fsync policy assertion via `fdatasync` counter.
  10. Update `crates/vox-cli/src/commands/telemetry.rs` (or equivalent) to use the new API.
- **Verification**:
  - `cargo build -p vox-spool` succeeds.
  - `cargo test -p vox-spool` passes.
  - `rg 'OpenOptions::new\(\)[^;]*\.append\(\s*true' crates/ --glob '!crates/vox-spool/**' --glob '!tests/**'` returns at most the allowlisted exceptions (see lint spec rule 8).

> Landed in 1cf50d3b on 2026-04-22; verification: green.

### M-04 · Land the guard's policy-file schema

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-01.
- **SSOT findings**: F56.
- **Sub-steps**:
  1. Confirm `contracts/db/data-storage-policy.v1.yaml` already exists (it is pre-existing, not net-new; modified in prior work).
  2. Create a new JSON Schema `contracts/db/data-storage-policy.v1.schema.json` that validates the YAML shape: `tiers`, `env_vars`, `rust_policy`, `repo_hygiene`, `contract_policy`, `ignored_paths`, `frozen_core_crates`, `generated_files`.
  3. Add entry to `contracts/index.yaml` matching the sibling `exec-policy` entry's shape.
  4. Create `contracts/db/data-storage-guard-report.v1.schema.json` for the guard's output.
  5. Wire loading inside `vox_cli::commands::ci::data_storage_guard::load_policy(path) -> anyhow::Result<DataStoragePolicy>` using `serde_yaml` + `jsonschema::Validator`.
- **Verification**:
  - `vox ci contracts-index` still passes.
  - `vox ci data-storage-guard --check-policy-only` returns 0 on a good policy file and nonzero on a malformed one (add a synthetic broken copy at `tests/fixtures/bad-data-storage-policy.yaml`).

> Landed in 9d5e915c on 2026-04-22; verification: green.

### M-05 · Delete empty `schemas/` directory

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F16.
- **Sub-steps**:
  1. Verify `schemas/` is empty: `ls -la schemas/`.
  2. Create `.vox` VoxScript at `scripts/migrations/2026-phase1-delete-empty-schemas-dir.vox` that removes the dir.
  3. Run the script: `vox run scripts/migrations/2026-phase1-delete-empty-schemas-dir.vox`.
  4. Add guard sub-check `schemas-dir-absent`.
- **Verification**: `test ! -d schemas`. Guard `schemas-dir-absent` fails on a synthetic re-creation.

### M-06 · Delete loose repo-root strays

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F23, F24, F25.
- **Sub-steps**:
  1. Inventory strays: `build_errors.txt`, `codex-cutover-20260412T065226Z.sidecar.json`, `codex-cutover-20260412T071753Z.sidecar.json`, `test_lexer.rs`, `error.vox`, `prototype_vox_tokenizer.json`.
  2. For `prototype_vox_tokenizer.json`: reclassify — move to `crates/vox-compiler/tests/fixtures/prototype_tokenizer.json` (see M-12 context). Others are pure deletion.
  3. Create VoxScript `scripts/migrations/2026-phase1-delete-repo-root-strays.vox`.
  4. Add guard sub-check `repo-root-strays-absent` that fails if any of the six files reappears.
  5. Update `.gitignore` stanza to fail forward: don't add them, just forbid re-creation in the guard.
- **Verification**: `for f in build_errors.txt codex-cutover-*.sidecar.json test_lexer.rs error.vox; do [ ! -e "$f" ]; done`.

> Landed in a9372778 on 2026-04-22; verification: green.

### M-07 · Fix `vox-agent.json` / `vox-schema.json` / `vox.tokens.json` ignored-but-tracked contradiction

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F26.
- **Sub-steps**:
  1. For each of the three files, determine intent by reading contents and grepping for their consumers.
  2. Decide per file: keep-tracked (then remove from `.voxignore`/`.aiignore`) or gitignore (then `git rm --cached` and add to `.gitignore`).
  3. Add a `docs/src/reference/repo-root-files.md` explaining the canonical list.
  4. Add guard sub-check `ignore-tracked-parity` that enumerates `.voxignore`, `.aiignore`, `.cursorignore`, `.aiexclude` and fails if any listed file is also tracked by git.
- **Verification**: guard check passes; documentation reflects the decision.

> Landed in 64b17bf4 on 2026-04-22; verification: green.

### M-08 · `scratch/` hygiene

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F27.
- **Sub-steps**:
  1. `git add scratch/.gitkeep` (empty file).
  2. Add `scratch/*` to `.gitignore` with a `!scratch/.gitkeep` negation.
  3. Add guard sub-check `scratch-clean`: `git ls-files scratch/ | grep -v '.gitkeep' | wc -l` must be 0.
- **Verification**: guard passes on a fresh clone.

> Landed in 6c000bd9 on 2026-04-22; verification: green.

### M-09 · `.vox/` boot contract

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-26.
- **SSOT findings**: F28.
- **Sub-steps**:
  1. Inventory current `.vox/` on a fresh clone: which subdirs are documented, which are write-on-demand, which are stale.
  2. Author `docs/src/reference/dot-vox-layout.md` listing each subdir (`agents/`, `artifacts/`, `bin/`, `cache/`, `memory/`, `sessions/`) with purpose, writer crate, and lifetime.
  3. In `crates/vox-config/src/paths.rs`, add `pub fn known_dot_vox_subdirs() -> &'static [&'static str]`.
  4. Add guard sub-check `dot-vox-layout-contract`: on a fresh clone, `.vox/` contents must be a subset of the known list.
  5. Create a VoxScript `scripts/migrations/2026-phase7-dotvox-cleanup.vox` that deletes unknown subdirs after prompting.
- **Verification**: guard check passes post-migration.

## Phase 1 — Contracts canonical (first cut)

### M-10 · First codegen pass inside `vox-jsonschema-util`

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-04.
- **SSOT findings**: F11, F64 (no new crate needed).
- **Sub-steps**:
  1. In `crates/vox-jsonschema-util/src/lib.rs`, declare `pub mod codegen;`.
  2. Create `crates/vox-jsonschema-util/src/codegen/mod.rs` exporting `loader`, `validator`, `emit_rust`, `emit_sql`, `diff`.
  3. Implement `codegen::loader::load_contracts(root: &Path) -> Result<ContractIndex>` that parses `contracts/index.yaml` and loads each file.
  4. Implement `codegen::validator::validate(&ContractIndex) -> Result<()>` that checks each file against `contracts/index.schema.json` and enforces `x-vox-version`/filename parity (F12).
  5. Pick the generator for the first domain: start with a hand-rolled emitter for `contracts/orchestration/agent-harness.schema.json`. Benchmark against `typify` before committing the choice (Open Question #4).
  6. Output to `crates/vox-orchestrator/src/generated/agent_harness.rs` (new directory `src/generated/` with a single `mod.rs` re-export).
  7. Add `crates/vox-orchestrator/src/generated/` to `.gitignore` AND also commit a `generated.manifest.sha256` that CI diffs against.
- **Verification**:
  - `cargo build -p vox-jsonschema-util` succeeds.
  - `cargo run -p vox-jsonschema-util --example emit_agent_harness -- --dry-run` prints a diff but makes no writes.
  - `cargo run -p vox-jsonschema-util --example emit_agent_harness` writes the file; re-running is a no-op (idempotent).

> Landed in 2a819464 on 2026-04-22; verification: green.

### [x] M-11 · First consumer — `vox-orchestrator::harness` (Landed: 2026-04-22)

- **Owner**: Orchestrator.
- **Blast radius**: M.
- **Blockers**: M-10.
- **SSOT findings**: F11.
- **Sub-steps**:
  1. Move legacy hand-maintained harness structs and tests from `crates/vox-orchestrator/src/harness.rs` to `crates/vox-orchestrator/src/legacy/harness_hand.rs`.
  2. Update `vox-orchestrator` consumers to import generated types from `crate::generated::agent_harness`.
  3. Add CI sub-check `schema-codegen-drift` that runs `vox-jsonschema-util`'s emitter with `--verify` and fails if the output diff is non-empty.
  4. Update `contracts/index.yaml` entry for `agent-harness.schema.json` to add `enforced_by: - vox ci data-storage-guard schema-codegen-drift`.
- **Verification**: `cargo test -p vox-orchestrator` passes; `vox ci data-storage-guard --only schema-codegen-drift` is green.

> Landed in [x] on 2026-04-22; verification: green.

### M-12 · Version header parity (`x-vox-version` ↔ filename `.vN.`)

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: M-10.
- **SSOT findings**: F12.
- **Sub-steps**:
  1. Inventory: for every file under `contracts/` matching `*.v[0-9]+.*`, read `x-vox-version` field if present.
  2. Find mismatches. For each, decide: rename filename OR fix header (prefer header fix).
  3. Create VoxScript `scripts/migrations/2026-phase1-contract-headers.vox` that performs the fix with `--check` dry-run mode.
  4. Add guard sub-check `version-header-parity`.
- **Verification**: after running once, a second `--check` run produces no changes (idempotent).

### M-13 · Author `contracts/config/env-vars.v1.yaml`

- **Owner**: Config owner.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F13, F47.
- **Sub-steps**:
  1. Grep the workspace: `rg 'env::var(_os)?\s*\(\s*"(VOX|TURSO|XDG)_' crates/ --type rust -o` — inventory all env-var reads.
  2. Create `contracts/config/env-vars.v1.yaml` listing each with: `name`, `owner_crate`, `kind` (string/int/bool/path), `default`, `required`, `introduced_in`, `deprecates` (optional), `see_also` (optional).
  3. Add `contracts/config/env-vars.v1.schema.json` (JSON Schema for the YAML).
  4. Add to `contracts/index.yaml`.
- **Verification**: file exists and validates against its schema.

### M-14 · Enforce env-var parity

- **Owner**: Config owner.
- **Blast radius**: L.
- **Blockers**: M-13.
- **SSOT findings**: F47.
- **Sub-steps**:
  1. Add guard sub-check `env-parity`: every `env::var("VOX_…")` / `env::var_os("VOX_…")` in `crates/` must match a `name:` in `contracts/config/env-vars.v1.yaml`.
  2. Allow a per-file `// vox-env-skip: reason` pragma to opt out, logged in the guard report.
- **Verification**: guard fails on a synthetic PR that introduces `env::var("VOX_NEW_SYNTHETIC")` without updating the contract.

### M-15 · Author `contracts/telemetry/events.v1.yaml` (event catalog)

- **Owner**: Platform (observability).
- **Blast radius**: M.
- **Blockers**: none.
- **SSOT findings**: F14.
- **Sub-steps**:
  1. Inventory `contracts/telemetry/*.schema.json` (there are several: `completion-run.v1.schema.json`, `completion-detector-snapshot.v1.schema.json`, …).
  2. Create `contracts/telemetry/events.v1.yaml` with an array of event records: `id`, `schema_path`, `owner_crate`, `tier` (must be "B"), `retention_days`, `description`.
  3. Add `contracts/telemetry/events.v1.schema.json`.
  4. Add to `contracts/index.yaml`.
- **Verification**: file validates; `events.v1.yaml` lists every existing telemetry schema.

### M-16 · OpenAPI `$ref` drift

- **Owner**: Populi.
- **Blast radius**: M.
- **Blockers**: M-10.
- **SSOT findings**: F18.
- **Sub-steps**:
  1. For every OpenAPI spec under `contracts/populi/*.openapi.yaml`, resolve inline type definitions that duplicate top-level schemas.
  2. Rewrite inline duplicates to use `$ref: '../<domain>/<type>.v1.schema.json'`.
  3. Add guard sub-check `openapi-contract-ref-parity`: fail if a `type`/`properties` block is structurally equivalent to a top-level schema but not a `$ref`.
- **Verification**: guard passes; `vox schema generate --dry-run` produces byte-identical output before/after.

### M-17 · Format consolidation

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: M-16.
- **SSOT findings**: F19.
- **Sub-steps**:
  1. For each subdir of `contracts/`, list distinct file extensions.
  2. Author a per-subdir README.md that declares allowed formats (start with one, allow openapi as a sibling for public surface).
  3. Add guard sub-check `domain-format-single`.
- **Verification**: every `contracts/<subdir>/` has a README listing allowed formats.

## Phase 2 — DDL flip

### M-20 · Re-sync `BASELINE_VERSION` and baseline digest (raised priority: Phase 1b)

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-04.
- **SSOT findings**: F1, F20, F49.
- **Priority note**: Between 2026-04-20 and 2026-04-21 `BASELINE_VERSION` moved 55 → 58 → 59 while the contract stayed at 54. The gap is widening with each orchestration-schema PR; M-20 must land before Phase 2 rather than mid-Phase-2.
- **Sub-steps**:
  1. Compute current baseline digest: `cargo run -p vox-db --example print_baseline_digest` — or, if no such example exists, write one at `crates/vox-db/examples/print_baseline_digest.rs` that prints `vox_db::schema::schema_baseline_digest_hex()`.
  2. In `contracts/db/baseline-version-policy.yaml`, update `repository_baseline_integer` to match `crates/vox-db::schema::manifest::BASELINE_VERSION` (59 at HEAD 2026-04-21; read the value at PR-land time, do not hard-code in this ticket).
  3. Update `repository_baseline_digest_hex` to match `vox_db::schema::schema_baseline_digest_hex()`.
  4. Route `VOX_DB_URL` / `VOX_DB_TOKEN` reads through `vox_secrets::resolve_secret(...)`: modify `crates/vox-db/src/config.rs` to call vox-secrets first, fall back to `env::var` only during tests.
  5. Run existing `vox ci check-codex-ssot` — should now pass.
  6. Add auto-update logic inside the guard: when `schema_baseline_digest_hex()` changes, a follow-up PR is suggested (or auto-drafted via a CI bot) rather than a hard failure that blocks all PRs.
  7. Add a pre-landing check to the PR template: if `crates/vox-db/src/schema/manifest.rs` was touched in the PR, the PR body must include a line `BASELINE_VERSION: <new>` matching the Rust constant; used to auto-update the contract via a post-merge workflow.
- **Verification**:
  - `vox ci check-codex-ssot` passes.
  - `grep -E 'BASELINE_VERSION: i64 = ' crates/vox-db/src/schema/manifest.rs` output and `grep 'repository_baseline_integer:' contracts/db/baseline-version-policy.yaml` output agree byte-for-byte after stripping labels.
  - `cargo run -p vox-cli --quiet -- ci data-storage-guard --check codex-ssot-digest --json` returns `"status": "pass"`.

> Landed in 87d4031a on 2026-04-22; verification: green.

### M-21 · Delta migration framework

- **Owner**: Data Core.
- **Blast radius**: H.
- **Blockers**: M-20.
- **SSOT findings**: F2.
- **Sub-steps**:
  1. Create `crates/vox-db/src/schema/deltas/` with a `mod.rs`.
  2. For each future bump of `BASELINE_VERSION`, add a `vNN_vMM.rs` file with `pub fn upgrade(conn: &Connection) -> Result<()>`.
  3. Write `vox_db::migrate_from(current: i64, target: i64)` that applies deltas in order.
  4. Write integration test that starts from an old baseline, migrates to head, and verifies row parity.
  5. Decide forward-only vs forward+backward (Open Question #5). Default: forward-only.
- **Verification**: `cargo test -p vox-db --test delta_migration` passes; a synthetic "old" database migrates cleanly.

### M-22 · Emit SQL side of the codegen pipeline

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-10, M-21.
- **SSOT findings**: F3.
- **Sub-steps**:
  1. Implement `vox_jsonschema_util::codegen::emit_sql` as a visitor that walks a `ContractIndex` and produces DDL per domain.
  2. For the first domain (`foundation`), generate `crates/vox-db/src/schema/generated/foundation.sql` and verify byte-equality with `crates/vox-db/src/schema/domains/foundation.rs`'s `SCHEMA_FOUNDATION` constant.
  3. Commit the generated SQL as canonical if equal; else file a drift-fix ticket before proceeding to the remaining 18 domains.
- **Verification**: `cargo test -p vox-jsonschema-util --test sql_roundtrip` passes.

### M-23 · Flip DDL ownership

- **Owner**: Data Core.
- **Blast radius**: H.
- **Blockers**: M-22.
- **SSOT findings**: F3.
- **Sub-steps**:
  1. For each of the 19 domain fragments in `crates/vox-db/src/schema/domains/`, convert the Rust string constant into an `include_str!()` that reads from `crates/vox-db/src/schema/generated/<domain>.sql`.
  2. Move the authoritative DDL into `contracts/db/domains/<domain>.v1.yaml`.
  3. Remove the `pub const SCHEMA_<DOMAIN>: &str = r#"…"#;` definitions from `domains/<domain>.rs`, replace with a re-export of the included file.
  4. Add guard sub-check `ddl-owner-parity`: every SQL fragment in `vox-db` must be `include_str!`, never a hand-written string constant.
- **Verification**: `cargo build -p vox-db` succeeds; `rg 'pub const SCHEMA_' crates/vox-db/src/schema/domains/` returns zero (all constants now come via include_str or generation).

### M-24 · Fold `research-audit-codex.db` into `store.db`

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-23.
- **SSOT findings**: F4.
- **Sub-steps**:
  1. Locate the crate that writes to `research-audit-codex.db`: `rg 'research-audit-codex' crates/`.
  2. Add a `research_audit_` table prefix to the existing DDL (as a new domain in `contracts/db/domains/research_audit.v1.yaml`).
  3. Write VoxScript `scripts/migrations/2026-phase2-fold-research-audit.vox` that reads the old DB and inserts into `store.db`.
  4. Delete `research-audit-codex.db` post-fold.
  5. Update writers to point at `store.db`.
- **Verification**: `ls .vox/` does not contain `research-audit-codex.db`; writer tests pass.

### M-25 · Delete orphan `vox_hardened.db`

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F5.
- **Sub-steps**:
  1. Final confirmation: `rg 'vox_hardened|hardened\.db' crates/` returns zero (verified 2026-04-21).
  2. Create VoxScript `scripts/migrations/2026-phase2-delete-vox-hardened-db.vox`.
  3. The script: removes `vox_hardened.db`, `vox_hardened.db-wal`, and `vox_hardened.db-shm` if present.
  4. Add to `.gitignore` to prevent re-creation.
  5. Add guard sub-check `orphan-db-absent` that fails if `vox_hardened.db` reappears at repo root.
- **Verification**: `[ ! -e vox_hardened.db ]`; guard passes.

### M-26 · Route `.vox_modules/local_store.db` opens through `vox-db` + `vox-config`

- **Owner**: CLI + Data Core.
- **Blast radius**: M.
- **Blockers**: M-23.
- **SSOT findings**: F6.
- **Sub-steps** (HEAD grep 2026-04-21 found three call-site families; the ticket must close all three):
  1. Add `pub fn local_pm_store_path(root: &Path) -> PathBuf` to `crates/vox-config/src/paths.rs`. This is the *only* place that constructs the `.vox_modules/local_store.db` path string.
  2. Add `pub fn open_local_pm_store(root: &Path) -> Result<VoxDb>` to `crates/vox-db/src/lib.rs` that wraps the open and applies the PM subset of baseline. This replaces the existing helper shape at `crates/vox-cli/src/commands/pm_lifecycle.rs:10–14` — move, don't duplicate.
  3. Rewrite `crates/vox-cli/src/commands/pm_lifecycle.rs:10–14::open_local_pm_store()` to delegate to `vox_db::open_local_pm_store(&root)` + `vox_config::paths::local_pm_store_path(&root)` instead of hand-building the path; delete the literal `.vox_modules` at line 6 and the literal `local_store.db` at line 13.
  4. Replace the three raw-literal callers that bypass the helper today:
     - `crates/vox-cli/src/commands/search.rs:34` — delete the `let store_path = ".vox_modules/local_store.db";` line and the `VoxDb::open(store_path)` at line 35; call `pm_lifecycle::open_local_pm_store(&root)` instead.
     - `crates/vox-cli/src/commands/diagnostics/tools/search.rs:35` — same shape as above, same fix.
     - `crates/vox-cli/src/commands/info.rs:33` — same shape as above, same fix.
  5. Rewrite the three `.context("open .vox_modules/local_store.db")` strings at `update.rs:17`, `sync.rs:33`, `lock.rs:11` to reference the helper (e.g., `.context("open_local_pm_store")`) so grep rule 16 (`vox-modules-local-db-literal`) can flip from `Scaffolded` → `Error` without a false positive on context messages. The doc comment at `pm/mod.rs:66` may keep its `.vox_modules/local_store.db` reference — doc comments are excluded from the grep rule's glob.
  6. Update `crates/vox-cli/tests/pm_lifecycle_integration.rs` to exercise the new API.
  7. Flip the guard sub-check `vox-modules-local-db-literal` in `crates/vox-cli/src/commands/ci/data_storage_guard/grep_rules.rs` from `Severity::Scaffolded` to `Severity::Error`.
- **Verification**:
  - `cargo test -p vox-cli --test pm_lifecycle_integration` passes.
  - `rg '"\.vox_modules/local_store\.db"' crates/ --type rust` returns zero matches.
  - `rg 'let store_path = "\.vox_modules' crates/ --type rust` returns zero matches.
  - `cargo run -p vox-cli --quiet -- ci data-storage-guard --check vox-modules-local-db-literal --json` returns `"status": "pass"`.

### M-27 · `Collection::new()` name validation

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: M-23.
- **SSOT findings**: F7.
- **Sub-steps**:
  1. Read `crates/vox-db/src/collection.rs:83` to confirm the string-interpolated INSERT.
  2. Replace with a bound-parameter statement OR validate the collection name against `^[a-zA-Z_][a-zA-Z0-9_]{0,63}$` on `Collection::new(name)` construction before reaching the INSERT.
  3. Prefer the validation approach because collection names are the table identifier, not a value.
  4. Add unit tests for invalid names.
- **Verification**: `cargo test -p vox-db collection::` passes; an attempted `Collection::new("'; DROP TABLE")` returns `Err(InvalidCollectionName)`.

### M-28 · Replace fuzzy legacy-schema heuristics with digest check

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: M-20.
- **SSOT findings**: F8.
- **Sub-steps**:
  1. Read `crates/vox-db/src/codex_legacy.rs:50-51` to understand the current fuzzy checks.
  2. Replace `is_legacy_schema_chain` with a comparison against the historical baseline digests committed in `contracts/db/baseline-version-history.yaml` (new file, part of this ticket).
  3. On an unknown DB, fail fast with a clear error.
- **Verification**: unit tests for each historical baseline digest.

### M-29-docs · Document Turso-specific workarounds module-level

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F9.
- **Sub-steps**:
  1. Add a module-level doc-comment at the top of `crates/vox-db/src/store/ops_retention.rs` explaining why the code does row-by-row deletion.
  2. Link to the Turso upstream issue if one exists.
  3. Add a TODO with a sunset condition: "remove when Turso batch DELETE is supported".
- **Verification**: doc-comment present; `cargo doc -p vox-db` renders it.

### M-29-spike · Pick Tier C CAS owner

- **Owner**: Data Core.
- **Blast radius**: L (spike).
- **Blockers**: none.
- **SSOT findings**: F62, Open Question #3.
- **Sub-steps**:
  1. Write a 1-page decision memo at `docs/src/architecture/decisions/010-tier-c-cas-owner.md`.
  2. Options: (a) `vox-db::cas` submodule, (b) new `vox-cas` crate, (c) re-purpose `vox-bounded-fs`.
  3. Criteria: (i) dependency fan-out, (ii) whether CAS needs Tier A row coupling, (iii) whether CAS needs to live on Populi nodes (no libSQL).
  4. Recommendation default (absent counter-evidence): (a) — extend vox-db.
  5. Update SSOT Open Question #3 with the decision.
- **Verification**: decision memo exists and is linked from SSOT.

### M-30 · Sunset `TURSO_*` env vars

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-20.
- **SSOT findings**: F10.
- **Sub-steps**:
  1. Inventory: grep `crates/` for literals enumerated under Turso compatibility aliases in `contracts/config/env-vars.v1.yaml` / env-vars SSOT.
  2. In `crates/vox-secrets/src/backend/vox_vault.rs` and `lib.rs`, add a deprecation warning branch that logs once when a `TURSO_*` var is used and routes it into the canonical `VOX_DB_URL` / `VOX_DB_TOKEN`.
  3. Add the deprecation list to `contracts/config/env-vars.v1.yaml` (requires M-13).
  4. Plan sunset for release 0.6 (6 months).
  5. Add guard sub-check `retired-env-var` that warns in `vox-secrets` (sunset window), fails elsewhere.
- **Verification**: guard green; CI passes; a test with a legacy Turso URL env var set produces the deprecation warning.

## Phase 3 — Tier B / C rollout

### M-31 · `dist/schemas.ts` provenance spike

- **Owner**: Frontend.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F17, F67, Open Question #9.
- **Sub-steps**:
  1. Identify whether `dist/schemas.ts` is source or build output. Check `package.json` build scripts, `dist/.gitignore`, `docs/src/reference/frontend.md` if it exists.
  2. If source: move to `apps/interop/marquee_app/src/schemas.ts` or similar, and commit decision.
  3. If build output: move out of `dist/`, add to `.gitignore`, and write a regeneration task. CI check: `dist/schemas.ts` byte-identical to regeneration.
  4. Author decision memo at `docs/src/architecture/decisions/011-dist-schemas-ts-provenance.md`.
- **Verification**: decision memo committed; follow-up ticket filed.

### M-32 · Inventory JSONL / append-file call sites

- **Owner**: Data Core.
- **Blast radius**: L (inventory).
- **Blockers**: M-03.
- **SSOT findings**: F22.
- **Sub-steps**:
  1. `rg '\.jsonl\b|OpenOptions::new\(\)[^;]*\.append\(\s*true' crates/ --type rust -l` — produce file list.
  2. For each match, annotate in a table: `path`, `crate`, `current writer`, `is_test`, `intended_tier`, `planned_migration_ticket`.
  3. Commit the table at `docs/src/architecture/data-storage-jsonl-inventory-2026.md`.
- **Verification**: inventory doc exists; each entry either routed to a migration ticket or explicitly marked "test-only, no migration".

### M-33 · Migrate append-file callers to `vox-spool`

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-03, M-32.
- **SSOT findings**: F22, F30.
- **Sub-steps**:
  1. Using the inventory from M-32, migrate each non-test caller. One crate per PR.
  2. Each PR: replace direct file open with `vox_spool::jsonl::SpoolWriter::append`.
  3. Update tests accordingly.
  4. After all crates are migrated, flip `append-only-file-open` grep rule to error severity.
- **Verification**: guard green; inventory doc shows 100% migrated.

### M-34 · Repo index cache manifest

- **Owner**: Data Core.
- **Blast radius**: L.
- **Blockers**: M-03.
- **SSOT findings**: F29.
- **Sub-steps**:
  1. Inventory `target/dogfood/` consumers: `rg 'target/dogfood|CANONICAL_TRAIN_DATA_DIR' crates/`.
  2. Author `contracts/cache/repo-index-manifest.v1.yaml` describing: each cache dir, owner crate, writer function, TTL, regeneration command.
  3. Update `CANONICAL_TRAIN_DATA_DIR` in `crates/vox-corpus/src/training/mod.rs:13` to resolve through `vox_config::paths::cache_dir().join("dogfood")`.
- **Verification**: guard sub-check `cache-manifest-complete` passes.

### M-35 · `target-*` sibling directories

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F31.
- **Sub-steps**:
  1. Inventory: `ls | rg '^target-[a-z]'` — list all siblings.
  2. Create VoxScript `scripts/migrations/2026-phase7-target-cleanup.vox` that removes them (already scaffolded).
  3. Flesh out the script body (currently `vox:skip` illustrative): enumerate, confirm no tracked files, rm -rf, update `.gitignore`.
  4. Add guard sub-check `abandoned-target-dir`.
- **Verification**: `ls | rg '^target-[a-z]'` returns only `target/`; guard passes.

### M-36 · Canonical `VOX_SPOOL_DIR` env var

- **Owner**: Config owner.
- **Blast radius**: L.
- **Blockers**: M-03, M-13.
- **SSOT findings**: F45.
- **Sub-steps**:
  1. Add `VOX_SPOOL_DIR` resolution to `crates/vox-config/src/paths.rs`: `pub fn spool_dir() -> PathBuf` with default `data_dir().join("spool")`.
  2. In `crates/vox-cli/src/telemetry_spool.rs:13`, the `VOX_TELEMETRY_SPOOL_DIR` lookup is replaced by a call to `vox_config::paths::spool_dir()`. Keep reading the legacy var for one release with a deprecation warning.
  3. Update `contracts/config/env-vars.v1.yaml` (depends on M-13).
- **Verification**: a test that sets `VOX_SPOOL_DIR` places files there; setting `VOX_TELEMETRY_SPOOL_DIR` still works but warns.

## Phase 4 — Observability unification

### M-40 · Consolidate subscriber init

- **Owner**: Platform.
- **Blast radius**: M.
- **Blockers**: M-15.
- **SSOT findings**: F50, F74.
- **Sub-steps**:
  1. Author `docs/src/architecture/telemetry-trust-ssot.md` (currently missing per F74). Minimal scope: trust surface, subscriber policy, redaction.
  2. Author `contracts/telemetry/subscriber-policy.v1.yaml` — profiles `Cli`, `Daemon`, `Test`, each with level + appender config.
  3. In `crates/vox-actor-runtime/src/observability.rs`, generalize the existing `init_structured_telemetry()` (HEAD L15 area, currently Daemon-shaped) into `pub fn init(policy: SubscriberPolicy) -> bool`. Keep `init_structured_telemetry` as a thin `init(SubscriberPolicy::Daemon)` wrapper so no current caller breaks.
  4. Refactor `crates/vox-cli-core/src/lib.rs:29::init_tracing_for_cli` to call `vox_actor_runtime::observability::init(SubscriberPolicy::Cli)`. Final body: two lines (`vox_actor_runtime::observability::init(SubscriberPolicy::Cli); ()`).
  5. Remove the duplicated `EnvFilter` / `tracing_subscriber::fmt()` setup from `vox-cli-core`. After this step, `vox-cli-core/Cargo.toml` gains `vox-actor-runtime = { path = "../vox-actor-runtime" }` and loses direct `tracing_subscriber` plumbing (it may keep the crate for macro use).
  6. Add guard sub-check `single-subscriber-init`: `rg 'tracing_subscriber::fmt\(\)|Registry::default\(\)' crates/` returns only `crates/vox-actor-runtime/src/observability.rs` + `crates/*/tests/**`.
- **Verification**:
  - `cargo test -p vox-cli-core` passes (existing tracing test).
  - `cargo run -p vox-cli --quiet -- --version` and `cargo run -p vox-cli --quiet -- ci manifest` both emit identically-shaped JSON trace lines (diffable up to timestamps).
  - Guard `single-subscriber-init` returns `pass`.

### M-41 · Non-blocking file appender

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-40.
- **SSOT findings**: F54.
- **Sub-steps**:
  1. Add `tracing-appender` to workspace deps.
  2. In `vox-actor-runtime::observability::init`, wire a rolling file appender under `$VOX_STATE_DIR/logs/`.
  3. Document in `docs/src/reference/logging.md`.
- **Verification**: logs appear under the configured path; rotation happens at midnight.

### M-42 · Structured event macro

- **Owner**: Platform.
- **Blast radius**: M.
- **Blockers**: M-40, M-03.
- **SSOT findings**: F51.
- **Sub-steps**:
  1. Add a macro `vox_event!(kind = "foo", field1 = value, …)` in `vox-actor-runtime::events`.
  2. The macro emits via `tracing::info!` AND appends a JSONL record to the Tier B spool using `vox-spool`.
  3. Generated types from `contracts/telemetry/events.v1.yaml` (M-15) gate allowed `kind` values at compile time.
- **Verification**: a test calls `vox_event!` and finds both the trace log and the spool record.

### M-43 · Canonical span registry

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-15.
- **SSOT findings**: F52.
- **Sub-steps**:
  1. Author `contracts/telemetry/spans.v1.yaml` listing all canonical span names.
  2. Generate a `vox_actor_runtime::spans::Name` enum.
  3. Add guard sub-check `span-registry-parity`: every `span!(…, "name", …)` in crates must use a variant of the enum.
- **Verification**: guard green.

### M-44 · `RUST_LOG` documentation

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-40.
- **SSOT findings**: F55.
- **Sub-steps**:
  1. Author `docs/src/reference/logging.md` (if not created in M-41).
  2. List per-crate tracing targets.
  3. Cross-link from the top-level README and from `AGENTS.md`.
- **Verification**: doc exists.

## Phase 5 — Memory hygiene

### M-50 · Row ↔ DTO ↔ Conv split in `vox-db`

- **Owner**: Data Core.
- **Blast radius**: H.
- **Blockers**: M-11.
- **SSOT findings**: F32.
- **Sub-steps**:
  1. Under `crates/vox-db/src/store/types/`, introduce three sibling modules per domain: `row/<domain>.rs`, `dto/<domain>.rs`, `conv/<domain>.rs`.
  2. For each type currently carrying BOTH libSQL `FromRow` logic and `Serialize`/`Deserialize`, split: row variant keeps libSQL, dto variant keeps serde, conv holds `From`/`TryFrom` pairs.
  3. `row/` modules MUST NOT derive `Serialize`/`Deserialize` — enforced by the `serde-on-row-struct` grep rule.
- **Verification**: `cargo build -p vox-db` passes; guard `row-wire-separation` green.

### M-51 · Same in `vox-orchestrator` + `vox-orchestrator-types`

- **Owner**: Orchestrator.
- **Blast radius**: H.
- **Blockers**: M-50.
- **SSOT findings**: F33, F41.
- **Sub-steps**: analogous to M-50; files at `crates/vox-orchestrator/src/types/`.
- **Verification**: guard green; existing tests pass.

### M-52 · Same in `vox-gamify::schema`

- **Owner**: Ludus.
- **Blast radius**: M.
- **Blockers**: M-50.
- **SSOT findings**: F34.
- **Sub-steps**: analogous to M-50; files at `crates/vox-gamify/src/schema/`.
- **Verification**: guard green.

### M-53 · `ObservationReport` libsql leak

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-50.
- **SSOT findings**: F35.
- **Sub-steps**:
  1. Identify the file that imports `libsql::Value` into `ObservationReport`.
  2. Replace with a plain `serde_json::Value` or a domain enum.
  3. Guard rule `row-types-outside-vox-db` prevents regression.
- **Verification**: `rg 'libsql::Value' crates/ --type rust` returns only `vox-db`.

### M-54 · Shared error primitives

- **Owner**: Platform.
- **Blast radius**: M.
- **Blockers**: none.
- **SSOT findings**: F36.
- **Sub-steps**:
  1. Decide home: `vox-primitives::errors` is the default.
  2. Define a small set of canonical errors: `ResourceNotFound`, `SchemaMismatch`, `Timeout`, `PermissionDenied`, `ExternalService`.
  3. Migrate crates one at a time; preserve `thiserror` + `source` chains.
- **Verification**: `rg 'pub struct .*Error' crates/` — surface narrows over time; CI doesn't enforce this as a hard gate (too invasive); tracked in the regression doc.

### M-55 · Serde `rename_all` normalization

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F40.
- **Sub-steps**:
  1. Grep for `rename_all = "kebab-case"` and `rename_all = "PascalCase"`.
  2. Decide case-by-case: change default if internal; leave if external-wire (document with a comment).
  3. Add guard sub-checks `renameAll-kebab` and `renameAll-PascalCase` (deny unless allowlisted).
- **Verification**: guard green.

### M-56 · Deny blocking-in-async (with allowlist)

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F42.
- **Sub-steps**:
  1. Update `clippy.toml` at repo root with `disallowed-methods = [{ path = "futures::executor::block_on", … }, { path = "tokio::task::block_in_place", … }]`.
  2. Author an allowlist at `clippy-allowlist.toml` OR inline `#[allow(clippy::disallowed_methods)]` with a justification comment at each of the existing allowed call-sites (`crates/vox-db/src/store/ops.rs`, `crates/vox-db/src/research.rs`, `crates/vox-orchestrator/src/session/manager/db_io.rs`).
- **Verification**: `cargo clippy --workspace -- -D clippy::disallowed_methods` passes.

### M-57 · XDG `VOX_CACHE_DIR` / `VOX_STATE_DIR` / `VOX_CONFIG_DIR`

- **Owner**: Config owner.
- **Blast radius**: M.
- **Blockers**: M-13.
- **SSOT findings**: F43, F47, F48.
- **Sub-steps**:
  1. In `crates/vox-config/src/paths.rs`, add `pub fn cache_dir() -> PathBuf`, `pub fn state_dir() -> PathBuf`, `pub fn config_dir() -> PathBuf` with XDG defaults.
  2. Update all call sites that use `.vox/cache`, `.vox/state`, or `.vox/config` to route through these.
  3. Add guard sub-check `hardcoded-vox-subdir`: `"\.vox/"` literal only in `vox-config`.
- **Verification**: guard green.

### M-58 · Env var contract enforcement wired to CI

- **Owner**: Config owner.
- **Blast radius**: L.
- **Blockers**: M-13, M-14.
- **SSOT findings**: F47.
- **Sub-steps**: wire the existing `env-parity` guard sub-check into the `.github/workflows/ci.yml` guard job as a hard failure.
- **Verification**: CI blocks a synthetic PR that adds an undocumented `VOX_*` read.

### M-59 · `VOX_USER_ID` deterministic default

- **Owner**: Config owner.
- **Blast radius**: L.
- **Blockers**: M-57.
- **SSOT findings**: F46.
- **Sub-steps**:
  1. In `crates/vox-config/src/paths.rs:52`, replace the current `VOX_USER_ID` default with a deterministic value: `sha256(hostname + username + repo_root_path)[:16]`.
  2. Document the scheme in `docs/src/reference/identity.md`.
- **Verification**: two invocations in the same shell produce the same `VOX_USER_ID`.

## Phase 6 — Regression gates (hard)

### M-60 · Promote `data-storage-guard` to hard blocker

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: all Phase 0–5.
- **SSOT findings**: F56.
- **Sub-steps**:
  1. In `.github/workflows/ci.yml`, add `vox ci data-storage-guard --fail-on warn` to the guard job in the check phase.
  2. Remove the `--stub` flag that M-01 used.
- **Verification**: CI fails a synthetic PR that introduces any guarded violation.

### M-61 · `deny.toml` ban on direct Turso imports

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-60.
- **SSOT findings**: F9.
- **Sub-steps**:
  1. Update `deny.toml` with a `[[bans.deny]]` entry for `turso` outside allowlisted crates.
  2. Add a similar entry for `libsql` (the lower-level crate).
  3. Run `cargo deny check bans` locally.
- **Verification**: `cargo deny check bans` passes on a clean tree, fails on a synthetic PR that imports `turso` into `vox-actor-runtime`.

### M-62 · Fuzz targets per wire schema

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-11, M-50.
- **SSOT findings**: F59.
- **Sub-steps**:
  1. For each generated DTO, add a `cargo fuzz` target that deserializes arbitrary bytes and asserts no panic.
  2. Wire into the nightly workflow `.github/workflows/mutation-nightly.yml` (or sibling).
- **Verification**: nightly runs and catches regressions.

### M-63 · strace canary

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-03, M-60.
- **SSOT findings**: F61.
- **Sub-steps**:
  1. Author a benchmark harness in `crates/vox-db/benches/storage_canary.rs`.
  2. CI (Linux only) runs it under `strace -f -e trace=openat -o trace.log`, then asserts: no file in `trace.log` matches a path outside the allowlisted set (test tempdir, `/dev/urandom`, `/proc/*`).
- **Verification**: CI job `canary-storage` green; a synthetic PR that opens a stray file fails it.

### M-64 · Property / roundtrip tests for Tier A

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-50.
- **SSOT findings**: F58.
- **Sub-steps**:
  1. For each row struct in `crates/vox-db/src/store/types/row/`, add a `proptest`-based roundtrip: construct arbitrary values, insert, select, assert equality.
  2. Test the conversion layer: Row → Dto → serialize → deserialize → Dto → Row yields original.
- **Verification**: `cargo test -p vox-db --test roundtrip` passes.

### M-64b · Golden fixture policy

- **Owner**: Platform.
- **Blast radius**: L.
- **Blockers**: M-64.
- **SSOT findings**: F57.
- **Sub-steps**: author `docs/src/contributors/golden-fixtures.md` declaring the single-tool choice. Default: `insta`.
- **Verification**: doc exists; follow-up migration tickets track conversion per-crate.

### M-65 · Tests never touch real DB

- **Owner**: Data Core.
- **Blast radius**: M.
- **Blockers**: M-26.
- **SSOT findings**: F60.
- **Sub-steps**:
  1. Audit tests under `crates/**/tests/` and `crates/**/src/**/tests.rs` for real-filesystem paths.
  2. Require use of `vox-test-harness::TempDb` or `tempfile::tempdir()`.
  3. Add guard sub-check `test-db-isolation`: any test that opens a path literal not rooted in `tempdir()` fails.
- **Verification**: a synthetic test that opens `.vox/store.db` fails the guard.

### M-66 · `.vox/` post-merge cleanliness

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-09.
- **SSOT findings**: F28.
- **Sub-steps**:
  1. After each `main` merge, a CI nightly job runs `vox doctor --check-dot-vox`.
  2. Any unknown subdir or stale file produces a ticket.
- **Verification**: nightly runs for a week with no findings.

## Phase 7 — Gap cleanup

### M-67 · Reserved for future fold (if M-29-spike picks vox-db for PM)

- **Owner**: Data Core + CLI.
- **Blast radius**: M.
- **Blockers**: M-26, M-29-spike.
- **SSOT findings**: F65.
- **Sub-steps**: only executes if the M-29-spike decision recommends folding project-local PM state into vox-db as a reusable sub-DB. Otherwise closed with reference to M-26 sufficing.
- **Verification**: decision memo references this ticket.

### M-68 · Frozen Core cross-link

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: M-23.
- **SSOT findings**: F66.
- **Sub-steps**:
  1. In `crates/_frozen.md`, add a paragraph requiring data-storage-guard green for any frozen crate PR.
  2. In `contracts/db/data-storage-policy.v1.yaml`, list frozen crates under `frozen_core_crates:`.
  3. Add guard sub-check `frozen-core-ddl-guard`: any diff touching `crates/<frozen>/src/**/row|domains/*` requires a `Frozen-Core-Amendment: <token>` trailer in the commit.
- **Verification**: attempted unconfirmed frozen-core DDL PR fails the guard.

### M-69 · `dist/schemas.ts` gate (conditional on M-31 outcome)

- **Owner**: Frontend + Data Core.
- **Blast radius**: L.
- **Blockers**: M-31.
- **SSOT findings**: F17, F67.
- **Sub-steps**:
  1. If M-31 resolves "build output": add `dist-schemas-drift` guard sub-check that regenerates and diffs.
  2. If "source": skip.
- **Verification**: per the M-31 decision.

### M-70 · Delete `target-*` cleanup (finalize)

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: M-35.
- **SSOT findings**: F31.
- **Sub-steps**:
  1. Add `target-*/` to `.gitignore`.
  2. Add guard sub-check `abandoned-target-dir`.
  3. Flesh out `scripts/migrations/2026-phase7-target-cleanup.vox`.
- **Verification**: guard green.

### M-71 · Document `.jj/` coexistence

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F68.
- **Sub-steps**:
  1. Add `.jj/` to `ignored_paths:` in `contracts/db/data-storage-policy.v1.yaml`.
  2. Add a short note to `AGENTS.md` §Archival Protocol siblings.
- **Verification**: policy file lists `.jj/`; guard ignores `.jj/`.

### M-72 · Grammar SSOT parity + vox-tensor / vox-ml-cli boundary

- **Owner**: Platform + Mens.
- **Blast radius**: M.
- **Blockers**: none.
- **SSOT findings**: F69, Open Question #8.
- **Sub-steps**:
  1. Add guard sub-check `grammar-ssot-drift`: changes to `tree-sitter-vox/grammar.js` or `src/grammar.json` require a sibling change to `tree-sitter-vox/GRAMMAR_SSOT.md`.
  2. Author `docs/src/architecture/decisions/012-vox-tensor-vs-vox-ml-cli-boundary.md` resolving Open Question #8.
- **Verification**: guard green; decision memo committed.

### M-73 · Top-level dir READMEs (patches/examples/apps/interop/marquee_app/tools/infra)

- **Owner**: Governance.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F71, F72, F73.
- **Sub-steps**:
  1. Author a README.md in each if missing, declaring: tier (if any), whether runtime or build-time, guard coverage.
  2. Add `patches/` to `ignored_paths:` in the policy.
- **Verification**: each dir has a README.

### M-74 · `.gitignore` hygiene

- **Owner**: CI.
- **Blast radius**: L.
- **Blockers**: none.
- **SSOT findings**: F70.
- **Sub-steps**:
  1. Ensure `.gitignore`, `.gitattributes`, `rust-toolchain.toml`, and all `contracts/**/*.yaml` are UTF-8 without BOM.
  2. Add guard sub-check `bom-config-files`: fails on any of these starting with `0xEF 0xBB 0xBF`.
- **Verification**: a synthetic BOM-adding PR fails.

## Phase 7b — Reconciliation with 2026-04-21 orchestration surface (M-75..M-78)

These four tickets close the findings added in SSOT §I (F75–F78) after reconciling the plan with the 24-hour commit window `411adcac..6249453b`.

### M-75 · Cross-link `contracts/orchestration/` as a first-class contract domain

- **Owner**: Platform + Data Core.
- **Blast radius**: S.
- **Blockers**: M-04.
- **SSOT findings**: F75.
- **Sub-steps**:
  1. Update `contracts/index.yaml` (if not already present) so every file under `contracts/orchestration/` is listed; run `vox ci contracts-index` to confirm.
  2. Allowlist `contracts/orchestration/` in `data-storage-policy.v1.yaml::contract_policy.allowed_formats_by_domain` as a mixed-format domain (`[yaml, schema.json, json]`), mirroring the existing `mcp` exception. Pattern: the JSON fixtures + YAML catalog + schema sidecar are all legitimate siblings.
  3. Extend the lint spec's `domain-format-single` rule so the policy-YAML allowlist governs exceptions. In `crates/vox-cli/src/commands/ci/data_storage_guard/checks/domain_format.rs`, read `allowed_formats_by_domain[<subdir>]` before flagging.
  4. Cross-reference `contracts/orchestration/` from SSOT §4 Tier A as a seed source for the `model_pricing_catalog` and `model_scoreboard` tables.
  5. Add a new sub-check `orchestration-contract-sibling-parity` — every `.v1.yaml` in `contracts/orchestration/` has a sibling `.v1.schema.json`; every `.v1.schema.json` has a referenced YAML or fixture.
- **Verification**:
  - `vox ci contracts-index` passes.
  - `vox ci data-storage-guard --check domain-format-single --json` returns `"status": "pass"` with `contracts/orchestration/` allowlisted.

### M-76 · Normalize contract version-header field name

- **Owner**: Platform.
- **Blast radius**: S–M (touches ~15 files).
- **Blockers**: M-75.
- **SSOT findings**: F76.
- **Sub-steps**:
  1. Author ADR-style note in `docs/src/architecture/` (small, one page): `x-vox-version` is the canonical version-header key for all contracts; `version` and `schema_version` are deprecated aliases with a one-release sunset.
  2. Run inventory: `rg -n '^(version|schema_version|x-vox-version):' contracts/ --type yaml` — record the full list in the ADR.
  3. In `crates/vox-cli/src/commands/ci/data_storage_guard/checks/version_header.rs`, accept `{x-vox-version, version, schema_version}` as synonyms during the sunset window; emit `Severity::Warn` (not `Error`) for the non-canonical forms.
  4. Script-rename: add `scripts/migrations/2026-q2-normalize-version-header.vox` that rewrites every non-canonical key to `x-vox-version` and removes duplicates (the policy YAML has both `version: 1` and `x-vox-version: 1` — keep the latter).
  5. After script lands, flip the warn to error and drop the synonym allowance.
- **Verification**:
  - `rg -n '^(version|schema_version):' contracts/ --type yaml` returns zero post-script.
  - `vox ci data-storage-guard --check version-header-parity --json` returns `"status": "pass"` with no warnings.

### M-77 · Provider-secret parity (`providers.v1.yaml` ↔ vox-secrets)

- **Owner**: vox-secrets.
- **Blast radius**: S.
- **Blockers**: M-75.
- **SSOT findings**: F77.
- **Sub-steps**:
  1. Read every `providers[].secret_id` from `contracts/orchestration/providers.v1.yaml`.
  2. Assert each is registered in `crates/vox-secrets/src/spec/ids.rs` (the enum that `ff0fdccc` extended by 31 lines) and handled by `crates/vox-secrets/src/spec/registry/llm.rs` (new in `ff0fdccc`, 165 lines).
  3. Add guard sub-check `provider-secret-parity` in `data_storage_guard/checks/provider_secret.rs`. Implementation follows the same YAML-read + source-grep pattern as `data-ssot-guards::run_scientia_consumption_registry_guard`.
  4. Extend existing `vox ci secrets-parity` to cross-check in the reverse direction: every `SecretId` that carries the `llm` tag has exactly one referencing entry in `providers.v1.yaml`.
- **Verification**:
  - Adding a dummy `- name: FakeProvider, secret_id: "DoesNotExist"` to `providers.v1.yaml` makes `vox ci data-storage-guard --check provider-secret-parity` fail.
  - Removing a real `secret_id` makes `vox ci secrets-parity` fail.

### M-78 · Retention policy for `model_scoreboard` / `model_pricing_catalog`

- **Owner**: Data Core + Platform.
- **Blast radius**: S.
- **Blockers**: M-75.
- **SSOT findings**: F78.
- **Sub-steps**:
  1. Inspect `contracts/db/retention-policy.yaml` — add rules for both new tables. Rollup sources: `llm_interactions` (existing) → `model_pricing_catalog` (daily rollup, keep 90d) → `model_scoreboard` (per-(category, strength, window) aggregate, keep 180d).
  2. Enforce: extend `data-ssot-guards` (not `data-storage-guard`) with a new file-check: every Tier A table listed in a domain fragment under `crates/vox-db/src/schema/domains/` appears in `retention-policy.yaml` with either a concrete `keep_*` rule or an explicit `retention: append-only`.
  3. Add unit test `crates/vox-db/tests/retention_policy_coverage.rs` that parses the domain SQL, enumerates `CREATE TABLE` names, and asserts each is named in the retention contract.
  4. Backfill: for each existing table (~80 tables), add either a retention rule or `append-only` marker. This is mechanical but large; the unit test gates it.
- **Verification**:
  - `cargo test -p vox-db --test retention_policy_coverage` passes.
  - `vox ci data-ssot-guards` output includes `retention-policy coverage OK`.

## Deferred / side-item tickets

- **M-55b**: benchmark `smol_str::SmolStr` in `vox-actor-runtime` request hot path (F38).
- **M-55c**: adopt `SmallVec<[T; N]>` for `intent_tags`, CLI `args`, etc. (F39).
- **M-57d**: profile `vox-corpus` / `vox-ml-cli` parsers; adopt `bumpalo` only if allocation cost is visible (F43).

## Index: finding → migration item

- F1 → M-20
- F2 → M-21
- F3 → M-22, M-23
- F4 → M-24
- F5 → M-25
- F6 → M-26
- F7 → M-27
- F8 → M-28
- F9 → M-29-docs
- F10 → M-30
- F11 → M-10, M-11
- F12 → M-12
- F13 → M-13
- F14 → M-15
- F15 → M-40
- F16 → M-05
- F17 → M-31, M-69
- F18 → M-16
- F19 → M-17
- F20 → M-20
- F21, F22, F30 → M-03, M-32, M-33
- F23, F24, F25 → M-06
- F26 → M-07
- F27 → M-08
- F28 → M-09, M-66
- F29 → M-34
- F31 → M-35, M-70
- F32, F33, F34, F41 → M-50, M-51, M-52
- F35 → M-53
- F36 → M-54
- F37, F38, F39, F43 → policy-only / deferred
- F40 → M-55
- F42 → M-56
- F44, F47, F48 → M-57
- F45 → M-36
- F46 → M-59
- F49 → M-20
- F50 → M-40
- F51 → M-42
- F52 → M-43
- F53 → M-41
- F54 → M-41
- F55 → M-44
- F56 → M-01, M-60
- F57 → M-64b
- F58 → M-64
- F59 → M-62
- F60 → M-65
- F61 → M-63
- F62 → M-29-spike, §5.4
- F63 → §5.3 (no crate created)
- F64 → §5.1 (no crate created)
- F65 → M-03, M-67
- F66 → M-68
- F67 → M-31, M-69
- F68 → M-71
- F69 → M-72
- F70 → M-74
- F71 → M-71, M-73
- F72, F73 → M-73
- F74 → M-40

## Ownership legend

- **Data Core**: owners of `vox-db`, `vox-jsonschema-util` codegen module, `vox-test-harness`.
- **Platform**: owners of `vox-actor-runtime`, `vox-primitives`.
- **CLI**: owners of `vox-cli`, `vox-cli-core`.
- **Orchestrator**: owners of `vox-orchestrator`, `vox-orchestrator-types`.
- **Mens**: owners of `vox-ml-cli`, `vox-populi`, `vox-tensor`.
- **Ludus**: owners of `vox-gamify`.
- **Config owner**: owners of `vox-config`.
- **Frontend**: owners of `apps/interop/marquee_app/`, `dist/`, `apps/editor/vox-vscode`.
- **Governance**: owners of `AGENTS.md`, `crates/_frozen.md`, and `docs/src/architecture/decisions/`.
- **CI**: owners of `.github/workflows/` and the `vox ci` subcommands.

If a team lead isn't named when a ticket is claimed, the default claimant is the crate's most recent non-agent com