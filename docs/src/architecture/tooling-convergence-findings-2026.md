---
title: "Tooling Convergence — Findings & Plan (2026-05-09)"
description: "Audit of Vox testing, linting, code-quality, architectural, doc, search, and CI/CD tooling. Inventory of redundancy and gaps, plus a phased convergence plan that picks the best tool for each job and routes every check through a single owner."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# Tooling Convergence — Findings & Plan (2026-05-09)

## Executive summary

The repo is **mostly converged at the foundation** (one snapshot library, one
test runner, one coverage tool, one layer-rule engine) but **fragmented at the
surface** (multiple ways to invoke "the same" check from different entry
points, with inconsistent severity, scope, and blocking semantics).

Five high-leverage findings:

1. **TOESTUB runs in three modes with three rule subsets** (`skeleton` at
   pre-commit, `legacy` scoped at CI, `audit` full at CI). The same patch can
   pass pre-commit, fail CI scoped, and pass CI full — depending on which
   rules each context happens to enable.
2. **Rustfmt has no `rustfmt.toml` and no pre-commit hook**, only a CI gate.
   Drift between local and CI is structural, not occasional.
3. **`vox-drift-check` runs only on pre-push (lefthook)**, never in CI.
   Anyone who pushes without lefthook installed bypasses it entirely.
4. **CI test invocation is hand-coded YAML, not a `vox ci test` subcommand.**
   Other guard logic is `vox ci ...`; tests are the lone exception. This makes
   "what runs in CI vs. locally" hard to keep in sync.
5. **No umbrella runner.** `vox ci pre-push` is the closest thing, but it
   skips the docs-quality gates (frontmatter, doctest-md, link-check), so a
   green pre-push can still produce a red main CI.

Convergence is achievable in three phases without rewriting anything: codify
existing tools as the canonical owner of their axis, route every entry point
(pre-commit, pre-push, CI, ad-hoc) through one Rust-implemented umbrella
(`vox audit`), and graduate advisory CI checks to blocking once the umbrella
covers them locally.

---

## Method

Six parallel read-only audits (one per axis) inventoried the workspace:

- Rust testing infrastructure (`crates/vox-test-harness`, `vox-integration-tests`, snapshot, mutation, coverage)
- Vox-language testing (`@test`, `examples/golden/`, `vox-doc-pipeline` doctests, `tdd-guard` lefthook)
- Linting / code-quality (rustfmt, clippy, `vox-code-audit`/TOESTUB, `vox-drift-check`, markdown, TS)
- Architectural enforcement (`vox-arch-check`, `layers.toml`, `contracts/`, where-things-live)
- Search & CLI surface (`vox-search`, `vox ci`, lefthook hooks, `.vox` scripts, umbrella commands)
- Doc auditing & CI/CD wiring (`vox-doc-pipeline`, `vox-doc-inventory`, `.github/workflows/*.yml`, runner contract, `vox ci pre-push`)

Each agent reported with file paths and line numbers; this doc cites the
canonical paths only. Citations are anchored to the worktree at `2026-05-09`.

---

## Inventory matrix

One row per axis. *Owner* is the single Rust crate or config file that
should be the SoT. *SoT?* is "yes" if the axis already has one canonical
owner, "split" if responsibility is fractured, "missing" if no owner exists.

| Axis | Canonical owner today | SoT? | Pre-commit | Pre-push | CI | Notes |
|---|---|---|---|---|---|---|
| Rust unit tests | `cargo test` per-crate | yes | — | `--full` only | `cargo llvm-cov nextest --profile ci` | nextest is universal; doc tests run separately |
| Integration tests | `vox-integration-tests` + `vox-test-harness` | yes | — | — | nextest workspace | Test harness is the only shared primitive |
| Snapshot tests | `insta` 1.x | yes | — | — | nextest + `INSTA_UPDATE=unseen` | Single library, no duplication |
| Property/fuzz | none | **missing** | — | — | — | No proptest, quickcheck, cargo-fuzz |
| Mutation tests | `cargo-mutants` | partial | — | — | PRs touching `vox-compiler` / `vox-codegen` | Non-blocking; nightly full-workspace job |
| Coverage | `cargo-llvm-cov` + `.config/coverage-gates.toml` | yes | — | — | `vox ci coverage-gates --mode enforce` | Per-crate floors, workspace floor 50% |
| Vox `@test` | `vox-compiler` (parser) + `vox test` CLI | yes | `tdd-guard` (presence only) | — | nextest (compiled-Rust pathway) | No native Vox executor; lowers to Rust |
| Vox doctests | `vox-doc-pipeline` doctest runner | yes | — | — | docs-quality.yml | Runs `vox check` on every ` ```vox ` block |
| Test-first policy | `vox-code-audit` (`skeleton/*` detectors) | split | `tdd-guard` lefthook | scoped TOESTUB | scoped TOESTUB | Two detectors (`untested_pub_api`, `no_test_for_pub_fn`) — different code paths |
| Rustfmt | rustfmt default | **split** | — | `cargo fmt --check` | `cargo fmt --check` | No `rustfmt.toml`; no pre-commit hook |
| Clippy | `Cargo.toml [workspace.lints]` + `clippy.toml` | yes | — | `clippy --all-targets -D warnings` | same | Two settings in `clippy.toml`; consistent |
| TOESTUB code audit | `vox-code-audit` (23 detectors) | **split by mode** | `skeleton` rules | `toestub-scoped` (legacy) | scoped (legacy) + audit (info) | Three rule subsets in three contexts |
| Drift / repetition | `vox-drift-check` (6 rules) | yes (gate broken) | — | `vox-drift-check --fail-on warning` | — | **Not in CI.** Bypassable via missing hook |
| Layer ordering | `vox-arch-check` Rule 1 + `layers.toml` | yes | — | — | guards-fast | No compile-time gate; CI-only |
| Fan-in / LoC / orphan / staleness | `vox-arch-check` Rules 2/3/4/8 | yes | — | — | guards-fast | All authored in `layers.toml` |
| Where-things-live coverage | `vox-arch-check` Rule 7 | partial | — | — | guards-fast | Substring check only; no auto-fix |
| Contract schemas | per-consumer ad hoc | **scattered** | — | — | per-consumer | No unified jsonschema gate; `contracts/index.yaml` is hand-maintained |
| Markdown lint | `markdownlint-cli2` | advisory | — | — | docs-quality.yml (`continue-on-error: true`) | Not blocking |
| Doc frontmatter | `vox-doc-pipeline --lint-only` | yes | — | — | docs-quality.yml | Validates 22 categories + 6 statuses |
| Doc inventory | `vox-doc-inventory` | yes | — | `doc-inventory verify` | guards-fast | Drift gate works |
| Generated `.md` files | `vox ci command-sync` / `generate-plugin-catalog-docs` | yes | regenerate + stage | — | `--check` advisory | CI is non-blocking |
| `.cursorignore`/`.aiignore` | `.voxignore` + `vox ci sync-ignore-files` | yes | regenerate + stage | — | `--verify` advisory | CI is non-blocking |
| Internal link check | `vox ci check-links` (advisory) + lychee | partial | — | — | docs-quality (advisory) + link_checker.yml (blocking) | Two tools, both CI-only |
| Search retrieval | `vox-search::execute_search_plan` | yes | — | — | — | SoT per `search-retrieval-ssot-2026.md`; not a CI gate |
| TypeScript lint | tool-local eslint configs only | **missing** | — | — | — | No workspace eslint/biome; codegen output unvetted |
| CLI surface drift | `vox ci command-sync` + `cli-command-surface.generated.md` | yes | regenerate + stage | — | `--check` advisory | Same advisory pattern as above |
| MCP tool registry | `contracts/mcp/tool-registry.canonical.yaml` + `vox ci command-compliance` | yes | — | — | guards-fast | Strong: canonical YAML + CI gate |
| Cryptography | `vox-crypto` (per AGENTS.md) | yes | — | — | (policy doc only) | No automated guard; relies on review |
| Secrets | `vox-secrets` + `vox ci secret-env-guard` / `secrets-parity` | yes | — | — | guards-fast | Strong: SSOT + CI gate |
| Versioning | `Cargo.toml [workspace.package].version` | yes | — | — | guards-fast (hardcoded version check) | One source for all first-party crates |
| Umbrella runner | `vox ci pre-push` | partial | — | — | n/a | Skips docs-quality, doctest-md, link-check, drift-check |

---

## Per-axis findings

### Testing (Rust + Vox)

**Converged.** `cargo-nextest` is the single runner; `insta` is the single
snapshot tool; `cargo-llvm-cov` is the single coverage tool with per-crate
floors in [`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml).
`vox-test-harness` is the canonical shared fixture crate
([`crates/vox-test-harness/`](../../../crates/vox-test-harness/)) and
`vox-integration-tests` is the canonical L5 cross-crate harness.

**Gaps:**
- **No property-based testing.** No `proptest`, `quickcheck`, or `arbitrary` in
  workspace deps. The compiler and codegen are obvious candidates; mutation
  tests partially compensate but only on those two crates.
- **Mutation testing is non-blocking.** [`mutation-nightly.yml`](../../../.github/workflows/mutation-nightly.yml)
  uses `continue-on-error: true`. The former PR-scoped lane (`mutation-pr.yml`) was deleted
  2026-09 — 95 min median / 407 min max, not a required check, and fully duplicated by this
  nightly lane; see `.superpowers/sdd/2026-09-05-p4-ci-lanes/`.
- **No `vox ci test` subcommand.** Test invocation is encoded in
  [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) lines 244–280;
  changing it requires editing YAML, not Rust.
- **`vox test` compiles to Rust then runs `cargo test`.** There is no native
  Vox `@test` executor — `crates/vox-cli/src/commands/test.rs` lowers and
  shells out. This is fine pragmatically but means the `@test` decorator's
  contract is "produces a valid `#[test]` function," not "runs in a Vox
  runtime."
- **`scripts/*.vox` are not test-gated.** The `skeleton/no-test-for-pub-fn`
  detector at [`crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs`](../../../crates/vox-code-audit/src/detectors/no_test_for_pub_fn.rs)
  excludes them by path. Automation glue can ship untested.

### Linting & code quality

**Converged on tools, fragmented on configuration.** Clippy has consistent
config (`clippy.toml` plus `[workspace.lints]` in
[`Cargo.toml`](../../../Cargo.toml)). TOESTUB rules are centrally registered
in [`crates/vox-code-audit/src/detectors/mod.rs`](../../../crates/vox-code-audit/src/detectors/mod.rs).

**Gaps:**
- **No `rustfmt.toml`.** Defaults are stable but undocumented; future rustfmt
  edition bumps could silently shift behavior. There's also no pre-commit
  hook for `cargo fmt --check`, so formatting drift is caught only in CI.
- **TOESTUB runs in three rule subsets.** `tdd-guard` (lefthook) runs
  `--rules skeleton --min-severity warning --mode enforce-strict`. CI scoped
  runs `--mode legacy`. CI full runs `--mode audit --min-severity info` and
  pipes to a budget gate. Same code, different verdicts.
- **`vox-drift-check` is pre-push-only.** The lefthook hook
  ([`lefthook.yml`](../../../lefthook.yml) line ~38) is the sole runner.
  No CI invocation. Coverage gap if hooks aren't installed.
- **TypeScript linting is essentially absent.** Two tool-local configs
  (`apps/experimental/visualizer/eslint.config.js`, `apps/editor/vox-vscode/eslint.config.mjs`).
  No workspace-wide eslint or biome; emitted TS from `crates/vox-codegen/codegen_ts/`
  is unvetted by any linter.
- **Markdownlint is advisory.** `continue-on-error: true` in
  [`docs-quality.yml`](../../../.github/workflows/docs-quality.yml).
- **Two near-overlapping test-first detectors.** `untested_pub_api.rs` (Rust)
  and `no_test_for_pub_fn.rs` (Vox) live in the same crate but share no
  logic. If one is enhanced, the other can drift.

### Architectural enforcement

**Strongly converged.** [`docs/src/architecture/layers.toml`](./layers.toml)
is the SoT for layer assignment, fan-in budgets, LoC budgets, staleness
exemptions, and known inversions. [`crates/vox-arch-check`](../../../crates/vox-arch-check/)
is the single enforcement tool, run by
[`scripts/arch-check.vox`](../../../scripts/arch-check.vox) and CI guards-fast.

**Gaps:**
- **No compile-time layer enforcement.** A forbidden Cargo dep is caught only
  by the CI run of `vox-arch-check`; nothing stops it locally.
- **`where-things-live.md` coverage is one-way.** Rule 7 warns when a crate
  isn't mentioned in the table, but doesn't warn when a row references a
  crate that no longer exists.
- **Contract schemas are decentralized.** [`contracts/`](../../../contracts/)
  has 130+ files (YAML, JSON, JSON-Schema). Each consumer crate parses its
  own contract; there's no unified `vox ci contracts-validate` that walks
  every file against its schema. Specific contracts (`exec-policy.v1.yaml`,
  `command-compliance`, `secrets-parity`, `mcp/tool-registry.canonical.yaml`)
  *do* have CI gates. Most don't.
- **Phase numbering and `.well-known/llms.txt` are hand-maintained.** No tool
  validates that phase numbers in plans stay consistent across docs.

### Search & retrieval

**Converged at the API layer.** [`crates/vox-search`](../../../crates/vox-search/)
is the SoT per [`search-retrieval-ssot-2026.md`](./search-retrieval-ssot-2026.md);
all callers (CLI, MCP, orchestrator, chat preamble) go through
`execute_search_plan` with `SearchRuntimeContext` + `SearchPolicy`.

**Gaps:**
- **No `vox search` CLI command.** Search is exposed as MCP tools and
  internal API; users have no first-class CLI surface. This is more a UX
  gap than a SoT problem.
- **`SearchPolicy` defaults are baked in Rust** (`policy.rs`), not in
  `contracts/search/policy.v1.yaml`. Other tunables (scaling, exec-policy)
  live in `contracts/`; search is the odd one out.

### Documentation auditing

**Converged on tools, advisory in CI.** [`vox-doc-pipeline`](../../../crates/vox-doc-pipeline/)
owns frontmatter validation and Vox-block doctests; [`vox-doc-inventory`](../../../crates/vox-doc-inventory/)
owns `docs/agents/doc-inventory.json` (schema v3); generated `.md` files
have well-defined regenerate commands.

**Gaps:**
- **Frontmatter validation runs in CI but not pre-push.** Local commits with
  bad `category`/`status` reach CI before failing.
- **Doctest extraction (`vox ci doctest-md --strict`) is not in pre-push.**
  Same drift risk.
- **Generated-file drift is advisory in CI** (`continue-on-error: true` in
  docs-quality.yml lines 95, 103, 112). The pre-commit hook regenerates and
  stages, but if hooks aren't installed the only safety net is a warning.
- **Two link checkers.** `vox ci check-links` (advisory) plus lychee in
  `link_checker.yml` (blocking). Two implementations to maintain; only one
  fails the build.

### CI/CD wiring

**Three runners, four staging surfaces, partial overlap.**

| Stage | Tool | Hard gate? | Drift risk |
|---|---|---|---|
| Pre-commit (lefthook) | `vox ci sync-ignore-files`, `command-sync`, `generate-plugin-catalog-docs`, `tdd-guard` | yes (commit blocked) | only if hooks not installed |
| Pre-push (lefthook + `vox ci pre-push`) | `vox-drift-check`, `cargo fmt`, `line-endings`, `ssot-drift`, `doc-inventory verify`, clippy, `toestub-scoped`, optional `nextest` | local-only | bypassable; not in CI |
| GitHub Actions (`ci.yml`) | guards-fast → lints → audits → tests | yes | docs-quality is separate workflow |
| GitHub Actions (`docs-quality.yml`, `link_checker.yml`, `mutation-nightly.yml`, `ssot-drift.yml`, …) | per-axis | partial; many `continue-on-error: true` | local has no equivalent |

**Redundancy:**
- `cargo fmt --check`, line-endings, ssot-drift, clippy, scoped TOESTUB run
  in **both** `vox ci pre-push` and `ci.yml`. That's by design (parity), but
  the two invocations are independently authored — when one updates, the
  other can drift.
- TOESTUB scoped + TOESTUB full audit cover overlapping ground at different
  severities; running both spends CI minutes for marginal coverage.
- doc-quality.yml's `--check` flags duplicate what pre-commit hooks already
  enforce when installed.

**Performance:**
- Single `cargo build` cache across guards-fast, lints, audits, tests is
  shared (good), but each TOESTUB mode shells out independently.
- `cargo llvm-cov nextest --profile ci` is the single test invocation;
  doctests run separately. Coverage instrumentation slows tests substantially
  and is unconditional even on PRs that don't touch Rust.
- No skip-if-unchanged logic at the workflow level — guards-fast runs the
  full suite even on docs-only PRs.

**Coverage gaps in CI:**
- `vox-drift-check` (no CI invocation).
- TypeScript lint (no CI invocation).
- Property/fuzz tests (don't exist).
- Doc frontmatter / doctest-md / link-check / generator drift run only on
  paths matched by the docs-quality workflow filter; pre-push doesn't catch
  them.

### CLI / umbrella runner

**No single umbrella; two partial ones.**

- [`vox ci pre-push`](../../../crates/vox-cli/src/commands/ci/pre_push.rs)
  runs the local merge-blocking subset of CI (fmt, line-endings, ssot-drift,
  doc-inventory, clippy, scoped TOESTUB, optional nextest). It does **not**
  run docs-quality, doctest-md, link-check, drift-check, or coverage gates.
- `vox doctor` is a diagnostic probe, not a check runner.

There is no `vox audit` / `vox check` / `vox verify` that runs every gate.
Developers must memorize which subset matters for their change.

---

## Cross-cutting findings

### Drift risks (ranked)

1. **TOESTUB three-mode fragmentation.** Same patch, three verdicts. The
   "what does this rule do?" question has no single answer.
2. **rustfmt has no pre-commit and no `rustfmt.toml`.** Pure structural drift.
3. **`vox-drift-check` skipped if hooks not installed.** No CI safety net.
4. **docs-quality advisory checks.** Generator drift, frontmatter, doctests,
   link-check all have `continue-on-error: true` paths. Silent failure mode.
5. **CI test invocation in YAML.** Any change requires editing two places
   (the YAML and `vox ci pre-push`).
6. **Dual link checkers.** `vox ci check-links` and lychee. One blocks, one
   doesn't.
7. **TypeScript codegen is unvetted.** Generated TS from `vox-codegen` has no
   downstream lint or type-check gate in CI.

### Redundancy worth removing

- **TOESTUB legacy + TOESTUB audit** in CI. Pick one (legacy with stricter
  rule set, plus optional info-level audit on nightly).
- **`vox ci check-links` vs lychee.** Pick one; remove the other.
- **`vox ci pre-push` step list vs `ci.yml` job list.** They should derive
  from the same manifest, not be independently authored.

### Performance opportunities

- **Path-filtered guards.** Skip Rust guards on docs-only PRs (compute the
  filter once in a setup job; gate downstream jobs on its output). Saves the
  bulk of guards-fast on docs PRs.
- **Conditional coverage instrumentation.** Run plain `nextest` on PRs and
  `llvm-cov nextest` only on `push:main` or when the PR touches `crates/**`.
  The coverage report on a docs PR is meaningless.
- **TOESTUB single pass.** Run all detectors once at the highest severity
  threshold and partition the output, instead of three separate invocations.
- **Cached `vox-arch-check`.** It's deterministic given Cargo metadata + `layers.toml`;
  it could be cached by content hash and skipped on PRs that touch neither.

### Coverage gaps to close

- Property-based tests on `vox-compiler` and `vox-codegen` (highest leverage
  for grammar/typecheck regressions).
- `vox-drift-check` in CI (must run regardless of local hook installation).
- TypeScript lint on emitted code (`crates/vox-codegen/codegen_ts/` outputs +
  `docs-astro/`).
- Frontmatter, doctest-md, link-check in `vox ci pre-push`.
- Make pre-commit hook installation verifiable in CI (fail if a regenerated
  file would change but the hook is the only enforcement).

---

## Convergence plan

### Principle: best tool for the job, single owner per axis

| Axis | Canonical owner (post-convergence) | Rationale |
|---|---|---|
| Format | `rustfmt` with explicit `rustfmt.toml` | Stable, fast, the obvious choice |
| Rust lint | `clippy` with `[workspace.lints]` + `clippy.toml` | Already converged |
| Code quality / TOESTUB | `vox-code-audit` with **one** rule set per severity | One detector engine, one rule registry |
| Drift / repetition | `vox-drift-check` | Already converged; just needs CI |
| Architecture | `vox-arch-check` + `layers.toml` | Already converged |
| Contracts | `vox ci contracts-validate` (new umbrella) calling per-contract validators | Walk the directory once with a unified jsonschema gate |
| Test running | `cargo-nextest` | Already converged |
| Coverage | `cargo-llvm-cov` + `coverage-gates.toml` | Already converged |
| Mutation | `cargo-mutants`, expanded scope, blocking on critical crates | Already chosen; just needs teeth |
| Property-based | `proptest` (new) | Industry default, integrates with cargo-test |
| Snapshot | `insta` | Already converged |
| Vox compiler tests | `vox test` (lowers to nextest) | Already converged |
| Doctests | `vox-doc-pipeline` doctest runner | Already converged |
| Doc frontmatter | `vox-doc-pipeline --lint-only` | Already converged |
| Doc inventory | `vox-doc-inventory` | Already converged |
| Link check | `lychee` (single tool) | Maintained externally; remove `vox ci check-links` |
| Markdown lint | `markdownlint-cli2` (graduate to blocking) | Already configured |
| TS lint | `biome` (new, workspace-wide) | One tool for fmt + lint, fast |
| Search | `vox-search::execute_search_plan` | Already converged |
| Secrets | `vox-secrets` | Already converged |
| Crypto | `vox-crypto` | Already converged |
| Versioning | `[workspace.package].version` | Already converged |
| Umbrella | `vox audit` (new), composed from a manifest | Single discovery point |

### Phase 1 — close the obvious drift (1–2 weeks)

Ship these in any order; each is independent and small.

1. **Add `rustfmt.toml`** at repo root with explicit `edition = "2024"`,
   `unstable_features = false`, and any per-team preferences. Add a
   pre-commit hook calling `cargo fmt --all -- --check`. Pros: kills format
   drift dead. Risks: trivial; one CI failure on the introducing PR while
   everything reformats.
2. **Add `vox-drift-check` to `ci.yml`.** A lints-job step calling
   `cargo run -p vox-drift-check -- . --severity warning --fail-on warning`.
   Pros: closes the most-bypassable gate. Risks: existing offenders need
   suppression or fix; expect a small backlog.
3. **Make docs-quality generator-drift checks blocking.** Remove
   `continue-on-error: true` from the ignore-file, plugin-catalog, and
   command-sync verification steps in
   [`docs-quality.yml`](../../../.github/workflows/docs-quality.yml).
   Pros: silent drift becomes visible. Risks: PRs without installed hooks
   start failing — that's the point, but flag in CONTRIBUTING.
4. **Pick one link checker.** Recommend keeping lychee
   ([`link_checker.yml`](../../../.github/workflows/link_checker.yml)) and
   removing `vox ci check-links` (it's a thin reimplementation). Pros: one
   maintainer surface. Risks: lychee config differs slightly; reconcile
   ignores once.
5. **Promote markdownlint to blocking.** Drop `continue-on-error` in
   docs-quality.yml. Pros: real lint, not vibes. Risks: existing markdown
   may need a one-time pass; do it in the same PR that flips the gate.
6. **Add `vox ci doctest-md --strict` and `vox-doc-pipeline --lint-only docs/src`
   to `vox ci pre-push`.** Pros: parity with CI; doc PRs catch issues
   locally. Risks: pre-push gets ~5–10s slower (acceptable).

### Phase 2 — unify TOESTUB and the umbrella (3–6 weeks)

1. **Single TOESTUB rule registry per severity.** Replace `--mode skeleton`
   / `--mode legacy` / `--mode audit` with a single canonical run that
   reports all detectors at their authored severity. Define one per-rule
   severity in the registry (`detectors/mod.rs`), and have all callers
   choose only `--min-severity`. Pros: same code, same verdict everywhere.
   Risks: medium — needs a migration of suppressions and a CI dry-run to
   verify no rule changes severity unintentionally.
2. **`vox audit` umbrella command.** New `crates/vox-cli/src/commands/audit.rs`
   driven by a manifest `contracts/ci/check-targets.v1.yaml` listing every
   check (name, invocation, category, blocking, runs_on). The umbrella
   replaces ad-hoc step lists in `vox ci pre-push` and the GitHub workflow
   YAML. Workflows become thin: "run `vox audit --category <foo>`."
   Pros: one place to add or remove a check; eliminates pre-push ↔ CI drift;
   `vox audit --category docs` runs the same thing locally and in CI.
   Risks: requires migrating ~20 existing CI step blocks; do it incrementally,
   one category at a time.
3. **Path-filtered execution.** The manifest also declares which paths each
   check cares about. The umbrella consults `git diff` (or workflow
   `paths-filter` output) to skip irrelevant checks. Pros: docs PRs no longer
   pay for Rust guards. Risks: mis-filtering hides regressions; ship the
   filter behind a `--no-filter` opt-out for paranoid runs.
4. **Conditional coverage.** PRs that touch only `docs/`, `contracts/`, or
   `examples/` skip `cargo llvm-cov` and run plain `nextest`. `push:main`
   always runs llvm-cov. Pros: large CI time savings on docs PRs. Risks:
   coverage delta on PRs becomes "unknown" rather than "small"; surface this
   in the PR summary so reviewers know.

### Phase 3 — fill coverage gaps (6–12 weeks)

1. **Add `proptest` to the workspace.** Start with `vox-compiler` (parser
   round-trip, lower→print→reparse, type inference idempotence) and
   `vox-codegen` (Rust output compiles, deterministic output for same
   input). Wire into nextest. Pros: catches grammar regressions current
   golden corpus would miss; complements mutation testing. Risks: property
   tests can be flaky; constrain `cases` and shrink time budgets.
2. **Make mutation testing blocking on `vox-compiler` / `vox-codegen`.**
   Drop `continue-on-error` from
   [`mutation-nightly.yml`](../../../.github/workflows/mutation-nightly.yml)
   (the PR-scoped `mutation-pr.yml` lane was deleted 2026-09 as a fully
   duplicated, non-required lane) for those two crates. Pros: tests stop
   being optional. Risks: false positives are real; expect to add
   `cargo-mutants` exclusions for known-irrelevant mutations.
3. **TypeScript linting via biome.** Workspace-root `biome.json`. Lint TS in
   `docs-astro/`, `apps/editor/vox-vscode/`, `apps/experimental/visualizer/`, **and** the output
   from `crates/vox-codegen/codegen_ts/` test fixtures. Pros: codegen
   regressions surface as lint failures, not runtime bugs. Risks: existing
   TS may need cleanup; ship as warnings first, promote to errors after a
   pass.
4. **Unified contracts validator.** New `vox ci contracts-validate` that
   walks `contracts/`, picks up `*.schema.json`, and validates every
   matching `*.yaml` / `*.json`. Replaces ad-hoc per-consumer parsing.
   Pros: schema drift caught at one chokepoint. Risks: some contracts have
   no schema yet — fall back to "parse + spot-check."
5. **Compile-time layer guard.** Optional. A build-script in each crate
   that fails compilation if `Cargo.toml`'s declared deps cross a layer.
   Pros: layer violations caught locally without `vox-arch-check`. Risks:
   build.rs adds compile time; harder to override than a CI check; probably
   not worth it given arch-check is already fast.
6. **`@test` coverage for `scripts/*.vox`.** Either extend
   `no_test_for_pub_fn` to include the scripts directory, or add a separate
   detector. Pros: closes the automation-script blind spot. Risks: existing
   scripts will need tests; budget the work.

### Explicit non-goals

- **Do not collapse `vox-arch-check`, `vox-code-audit`, and `vox-drift-check`
  into one crate.** They answer different questions (layer fitness, code
  smell, repetition). One runner (`vox audit`) calling three crates is the
  right shape.
- **Do not write a custom format/lint engine.** Rustfmt + clippy + biome are
  the right tools. Custom logic belongs only in the project-specific
  detectors (TOESTUB, drift, arch).
- **Do not move CI off self-hosted runners** to chase "single environment"
  uniformity. The runner contract
  ([`docs/src/ci/runner-contract.md`](../ci/runner-contract.md)) is already
  pinned.
- **Do not introduce a new shell layer for orchestration.** The umbrella
  is `vox audit` (Rust), composed from a YAML manifest. No bash, no Python.

### Open questions

1. **TOESTUB rule registry migration scope.** How much severity adjustment
   is needed before the three-mode collapse is safe? A dry-run on `main`
   would answer.
2. **Property-test budget.** `proptest` cases × shrink iterations × CI
   minutes. Worth a small benchmark before committing to a `cases = N`
   default.
3. **biome adoption order.** Do we lint the TS that `vox-codegen` emits as
   part of the test fixture pipeline, or post-emit? The first is stricter
   but couples codegen tests to biome's stability.
4. **`vox audit` vs `vox ci audit` namespace.** The audit command is
   broader than CI gates (also runs locally). Recommend `vox audit` at the
   top level; `vox ci audit` would imply CI-only.
5. **Should `where-things-live.md` be auto-generated from `layers.toml`?**
   Adds a generator and a sync step but eliminates the hand-maintained
   risk. Trade-off worth weighing in Phase 3.

---

## Suggested first PR

If shipping this in pieces, the highest-leverage first PR is:

1. Add `rustfmt.toml`.
2. Add `cargo fmt --check` to lefthook pre-commit.
3. Add `vox-drift-check` step to `ci.yml`.
4. Drop `continue-on-error: true` from generator-drift verification in
   `docs-quality.yml`.
5. Remove `vox ci check-links` (use lychee only).

That's five small surgical changes, each independently revertable, and
together they kill the two top drift risks (rustfmt, drift-check) plus
half the silent-failure surface in docs-quality.

---

## Appendix: where the audit looked

- [`AGENTS.md`](../../../AGENTS.md), [`CLAUDE.md`](../../../CLAUDE.md)
- [`Cargo.toml`](../../../Cargo.toml), [`clippy.toml`](../../../clippy.toml),
  [`lefthook.yml`](../../../lefthook.yml),
  [`.config/nextest.toml`](../../../.config/nextest.toml),
  [`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml),
  [`.markdownlint.jsonc`](../../../.markdownlint.jsonc)
- [`.github/workflows/`](../../../.github/workflows/) — ci.yml,
  docs-quality.yml, link_checker.yml, mutation-nightly.yml, ssot-drift.yml,
  bench-nightly.yml, mutation-nightly.yml
- [`crates/vox-arch-check/`](../../../crates/vox-arch-check/),
  [`crates/vox-code-audit/`](../../../crates/vox-code-audit/),
  [`crates/vox-drift-check/`](../../../crates/vox-drift-check/),
  [`crates/vox-doc-pipeline/`](../../../crates/vox-doc-pipeline/),
  [`crates/vox-doc-inventory/`](../../../crates/vox-doc-inventory/),
  [`crates/vox-test-harness/`](../../../crates/vox-test-harness/),
  [`crates/vox-integration-tests/`](../../../crates/vox-integration-tests/),
  [`crates/vox-search/`](../../../crates/vox-search/),
  [`crates/vox-cli/src/commands/ci/`](../../../crates/vox-cli/src/commands/ci/)
- [`docs/src/architecture/layers.toml`](./layers.toml),
  [`docs/src/architecture/where-things-live.md`](./where-things-live.md),
  [`docs/src/architecture/search-retrieval-ssot-2026.md`](./search-retrieval-ssot-2026.md),
  [`docs/agents/governance.md`](../../agents/governance.md)
- [`contracts/`](../../../contracts/) (130+ files; key consumers cited inline)
