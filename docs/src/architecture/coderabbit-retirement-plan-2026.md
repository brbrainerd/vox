---
title: "CodeRabbit Retirement Plan"
description: "Ordered, gate-aware removal plan for every CodeRabbit reference in the repo, with partner changes and regeneration commands per step."
category: "Architecture SSOTs"
---

CodeRabbit (the automated PR-review bot) has been retired by the repo owner.
AGENTS.md §PR & Review Discipline already reflects this. This document maps
**every remaining reference** found by searching the working tree (excluding
`target/`, `.vox/audit/`, `docs/src/archive/`) for `coderabbit`/`CodeRabbit`
and `*CODERABBIT*`, and lays out the order in which they can be removed
without tripping an SSOT/CI gate. **No removal has been performed** — this is
the plan only.

## Reference count by category

| Category | Count (approx., files) |
|---|---|
| First-party crate implementing the bot-orchestration CLI (`vox-cli-review`, `crates/vox-cli-review/src/coderabbit/**`) | 1 crate, ~20 files |
| `vox-cli` wiring (feature gate, dispatch, docs comments, e2e test) | 8 files |
| `vox-gui` (Tauri commands, React view + tests, transport wrappers, `App.tsx` view-key union) | ~7 files |
| SSOT contracts (operations catalog, command registry, GUI surface registry, secrets registries, crate-edges/layers, index) | 9 files |
| Generated artifacts (command-surface doc, gui-surface-coverage/registry reports, config-hygiene baseline) | 4 files |
| Root config (`.coderabbit.yaml`, `.voxignore` line, `Vox.toml` `[review.coderabbit]`) | 3 files |
| CI/workflow (`ci.yml` feature-build line, `pre_push.rs` reminder) | 2 files |
| Generic external-review ingest code that treats `"coderabbit"` as one data provider among others (`vox-code-audit`, `vox-corpus`, DB store types) | 3+ files — **do not remove**, see Unverified section |
| Docs/plans/specs mentioning CodeRabbit historically | ~85 files (`docs/superpowers/**`, `docs/src/reference/cli.md`, `.cursor/rules/*.mdc`, `scripts/README.md`, `scripts/quality/doc-policy-lint.vox` comment) |

Total files matching the search: 123 (`Grep -i coderabbit`, tree-wide, exclusions applied). The four categories above (crate, wiring, GUI, SSOT/generated/config/CI) are the ones with removal steps below; docs/plans are cleanup-only (§Step 9).

## Step-by-step removal order

### Step 1 — SSOT edit: `contracts/operations/catalog.v1.yaml`

- **What**: Delete the two operation entries `id: recensio` (`~line 9763`) and `id: review` (`~line 10360`), both carrying `cli.feature_gate: coderabbit`. Also reword the unrelated TOESTUB tool description at `~line 12879` ("Record or update TOESTUB anti-pattern findings from external reviews (GitHub/CodeRabbit)") — this one is prose only, not code-coupled, edit independently.
- **Does**: This is the hand-edited SSOT. `contracts/cli/command-registry.yaml` is *generated from it* (header: "GENERATED FROM contracts/operations/catalog.v1.yaml via `vox ci operations-sync --target cli --write`. Do not hand-edit vox-cli rows here.").
- **Safe standalone?** No — must land with Step 2 (the `commands::review` handler and `coderabbit` Cargo feature) in the same change, or `operations-sync` will fail/produce a registry pointing at a dead handler.
- **Gate / regenerate**: After editing catalog.v1.yaml, run `vox ci operations-sync --target cli --write` (regenerates `contracts/cli/command-registry.yaml`), then `vox ci command-sync --write` (regenerates `docs/src/reference/cli-command-surface.generated.md`, which currently has rows for `vox recensio` and `vox review` at lines 241/253). `vox ci ssot-drift` verifies both.

### Step 2 — `vox-cli` feature + dispatch removal

- **Files**: `crates/vox-cli/Cargo.toml` (`coderabbit = [...]` feature block, ~line 122, and its dependency on `vox-cli-review`), `crates/vox-cli/src/lib.rs` (two `#[cfg(feature = "coderabbit")]` blocks, lines 178-179 and 607-608), `crates/vox-cli/src/cli_dispatch/mod.rs` (3 cfg sites: 13, 271, 562), `crates/vox-cli/src/cli_dispatch/lanes.rs` (line 113), `crates/vox-cli/src/commands/mod.rs` (line 143-144), `crates/vox-cli/src/commands/review/mod.rs` (doc comment, whole module may become empty — check if `mens review`/`mens-dei` still needs it), `crates/vox-cli/src/main.rs` (doc comments only, lines 10 and 45), `crates/vox-cli/tests/coderabbit_e2e.rs` (delete only if it truly tests the retired live orchestration — **it doesn't**, see Unverified section: keep or move it).
- **Does**: Removes the compiled command surface (`vox review`, `vox recensio`) and the optional Cargo feature.
- **Partner**: Must land with Step 1 (catalog/registry) and Step 3 (crate deletion) in the same commit — a `#[cfg(feature = "coderabbit")]` referencing a deleted `vox_cli_review::run` won't compile otherwise.
- **Gate**: `cargo build -p vox-cli --locked --features completion-toestub,extras-ludus,ars,coderabbit` in `.github/workflows/ci.yml:115` explicitly compiles this feature to catch drift — **delete `,coderabbit` from that line in the same commit**, or the job hard-fails compiling a feature that no longer exists.

### Step 3 — delete the `vox-cli-review` crate

- **Files**: `crates/vox-cli-review/` (whole crate: `Cargo.toml`, `src/lib.rs`, `src/coderabbit/**` — `tasks.rs`, `stack_planner/*`, `semantic_planner/*`, `run_state.rs`, `ranker.rs`, `planner.rs`, `path_policy.rs`, `mod.rs`, `limits.rs`, `ingest.rs`, `historical_planner.rs`, `github/**`, `git.rs`, `config.rs`).
- **Does**: This crate *is* the CodeRabbit batch-PR orchestration tool (`vox review coderabbit …`) — planning, submitting, ranking, and ingesting CodeRabbit findings.
- **Partner**: Same commit as Step 2 (nothing else references it once the feature gate is gone) and Step 4 (workspace `Cargo.toml` member list / lockfile).
- **Gate**: `vox ci crate-edges` checks the exact edge-set (`contracts/ci/crate-edges.allow.v1.json` has 9 edges naming `vox-cli-review`, plus `contracts/ci/crate-layers.v1.json:16` assigns it layer 4). Deleting the crate without updating these makes `crate-edges` fail (edges pointing at a nonexistent crate). **Tightening is always allowed** per AGENTS.md §Dependency Discipline rule 2 — run `vox ci crate-edges --tighten` after the crate is gone to drop the stale entries; do not hand-edit the allow-list yourself for anything beyond removal-of-what-you-just-deleted (that is tightening, not admitting a new edge, so it's fine).
- Also update `Cargo.lock` (regenerates automatically on next `cargo build`/`cargo check`) and the workspace member list if `vox-cli-review` is explicitly listed.

### Step 4 — secrets registry

- **Files**: `crates/vox-secrets/src/spec/ids.rs:393` (`VoxCoderabbitGithubPerPage` variant in the `SecretId` enum), `crates/vox-secrets/src/spec/registry/platform.rs:464-471` (its `SecretSpec` entry, `canonical_env: "CODERABBIT_GITHUB_PER_PAGE"`).
- **Consumers**: exactly one — `crates/vox-cli-review/src/coderabbit/ingest.rs:100` (`vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCoderabbitGithubPerPage)`). No other crate references this variant, so removing it is **not** a breaking change to any surviving consumer of the public `SecretId` enum (it only breaks the crate being deleted in Step 3, which is fine).
- **Partner**: Land with Step 3 (same PR) so `ingest.rs`'s only caller of this secret disappears at the same time as the enum variant.
- **Gate**: `vox ci secret-env-guard` and `vox ci secrets-parity` regenerate/verify `contracts/secrets/secret-capabilities.v1.json` (line 1363-1364), `contracts/secrets/managed-env-names.v1.json` (line 1272-1273), and `contracts/secrets/managed-env-names.md` (line 7, `CODERABBIT_GITHUB_PER_PAGE`). These three are generated from the Rust spec — after editing `ids.rs`/`platform.rs`, run both `vox ci secret-env-guard` and `vox ci secrets-parity` (with `--write` if they support it, else re-run the underlying generator) to sync the three JSON/MD artifacts. Do not hand-edit the JSON files.

### Step 5 — GUI surface (Tauri backend + React frontend)

- **Files**:
  - `contracts/gui/surface-registry.v1.yaml:36-42` — the `view_key: coderabbit` entry (nav "CodeRabbit", `develop` group). This is the SSOT for the GUI nav.
  - `crates/vox-gui/src/commands/coderabbit.rs` (214 lines — 4 Tauri commands: `coderabbit_plan`, `coderabbit_run_async`, `coderabbit_report`, `coderabbit_token_present`) and their registration in `crates/vox-gui/src/main.rs:143-146`.
  - `crates/vox-gui/ui/src/transport.ts:1017-1044` — the 4 TS wrapper functions calling those commands.
  - `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/` — `CodeRabbitView.tsx`, `CodeRabbitView.test.ts`, `CodeRabbitView.test.tsx`.
  - `crates/vox-gui/ui/src/App.tsx:116,147` — `'coderabbit'` in the view-key union type and the surface array.
  - `crates/vox-gui/ui/e2e/lib/operatorShellMock.ts` — mock for the above commands, used by GUI e2e tests.
- **Does**: `commands/coderabbit.rs` does not depend on the `vox-cli-review` crate directly (confirmed: no `vox-cli-review`/`vox_cli_review` reference in `vox-gui`'s `Cargo.toml` or source) — it shells out to the `vox` **sidecar binary** (`vox review coderabbit …`) as a subprocess. So Steps 2/3 (removing the CLI feature) silently break this panel at runtime (the sidecar command stops existing) without breaking the `vox-gui` *build*. This makes Step 5 independent to build but **functionally coupled** to Steps 2-3 — ship them together or the GUI ships a dead panel.
- **Gate**: `contracts/gui/surface-registry.v1.yaml` is the hand-edited SSOT; regenerate downstream with `vox ci gui-surface-registry --write` (writes the generated TS + report — the tool errors "gui-surface-registry: … drift, run `vox ci gui-surface-registry --write`" if the generated file and registry disagree) and `vox ci gui-surface-coverage --write` (regenerates `contracts/reports/gui-surface-coverage.v1.json`, which lists `"coderabbit"`, `"coderabbit_plan"`, `"coderabbit_report"`, `"coderabbit_run_async"`, `"coderabbit_token_present"` at lines 1187, 1250-1253).

### Step 6 — root config files

- **`.coderabbit.yaml`**: the bot's own config file. Safe to delete standalone once the bot is confirmed off (no gate reads this file from our side — it's consumed only by the external CodeRabbit service). No partner required, but do it after Steps 1-5 land so a stray re-invocation of the bot during the transition doesn't reference a half-migrated repo.
- **`Vox.toml:16-43`**: `[review.coderabbit]` config block (`groups_config`, etc.), consumed only by `vox review coderabbit semantic-submit` (the crate deleted in Step 3). Remove in the same commit as Step 3, or `vox` will silently ignore an orphaned config table (not a hard failure, but dead config).
- **`contracts/review/coderabbit-semantic-groups.v1.yaml`**: confirmed via grep — consumed **only** by `crates/vox-cli-review/src/coderabbit/semantic_planner/rules.rs` (compiled in via `include_str!`) and cataloged at `contracts/index.yaml:1385-1389`. Nothing else reads it. Delete in the same commit as Step 3, and remove its `contracts/index.yaml` entry.
- **`.voxignore:101`**: `.coderabbit/` (the bot's local run-state directory, e.g. `run-state.json` mentioned in `vox-gui/src/commands/coderabbit.rs`). Remove once Step 3/5 land and the directory is never written again; until then leave it (it's harmless — just an ignore rule) or remove last.
- **Gate**: none of these four files are CI-generated; they're all hand-maintained. `vox ci sync-ignore-files` regenerates `.cursorignore`/`.aiignore`/`.aiexclude` from `.voxignore` — run it after editing `.voxignore` so the derived files don't drift.

### Step 7 — `pre_push.rs` reminder

- **File**: `crates/vox-cli/src/commands/ci/pre_push.rs:272-308` — prints "this re-push will NOT auto-trigger a CodeRabbit review (`.coderabbit.yaml` auto_incremental_review=false) … comment `@coderabbitai review`" on every pre-push. This logic is **stale relative to the already-updated AGENTS.md** — the doc says CodeRabbit is retired but the code still actively reminds contributors to use it.
- **Safe standalone**: yes, delete the whole reminder block independently — it has no downstream generator or gate consuming its output (it's a stderr print).
- No gate. This is a pure code cleanup; nothing regenerates from it. Do it in Step 6 or its own trivial follow-up commit.

### Step 8 — CI/config-hygiene baseline

- **File**: `contracts/config/config-hygiene-baseline.txt:44` — `env-var-not-in-registry|crates/vox-cli-review/src/coderabbit/semantic_planner/rules.rs|CARGO`. This is a generated baseline/allowlist entry keyed to a file path inside the crate being deleted in Step 3.
- **Gate**: whatever `vox ci` config-hygiene check reads this baseline will simply have a stale, harmless line (the file it points to no longer exists) unless the checker fails on dangling baseline entries. Regenerate it the same way its other entries are produced (search for the config-hygiene generator subcommand before removing by hand — **not independently verified which `vox ci` subcommand owns this file**; see Unverified section).

### Step 9 — documentation cleanup (no gate)

- ~85 files under `docs/superpowers/plans/**`, `docs/superpowers/specs/**`, `docs/superpowers/reviews/**`, `docs/src/reference/cli.md`, `.cursor/rules/cross-platform-source-hygiene.mdc`, `scripts/README.md`, `scripts/quality/doc-policy-lint.vox` (comment only) mention CodeRabbit historically (design docs for the `vox-cli-review` crate itself, e.g. `2026-06-29-coderabbit-review-gui-and-sweep-design.md`/`.md` plan, and incidental mentions of "reach human/CodeRabbit review" in `workflow-lint.yml` comments and `.cursor/rules/*.mdc`).
- **Do not bulk-edit these.** Historical plans/specs under `docs/superpowers/` are point-in-time records, not live SSOT — leave them as-is (rewriting history here has no functional effect and risks losing context on *why* the crate existed). The only doc worth a live edit is `docs/src/reference/cli.md` line 699 (the manually-authored companion to the generated command-surface doc) once Steps 1-2 land, and `.cursor/rules/cross-platform-source-hygiene.mdc:22`'s "(existing CodeRabbit paths)" parenthetical, which can be reworded or left (it's citing precedent for a `-c core.autocrlf=false` pattern, not describing live CodeRabbit integration).
- No gate enforces prose accuracy in `docs/superpowers/**`.

## Answers with evidence

- **Is there a `vox` CLI subcommand for CodeRabbit?** Yes — `vox review` (`commands::review`, `latin_ns: recensio`) and its Latin alias `vox recensio`, both `feature_gate: coderabbit`, defined in `contracts/operations/catalog.v1.yaml` (ids `review` and `recensio`) and generated into `contracts/cli/command-registry.yaml` (confirmed by its own header: "GENERATED FROM contracts/operations/catalog.v1.yaml via `vox ci operations-sync --target cli --write`. Do not hand-edit vox-cli rows here."). Removing it means: edit `catalog.v1.yaml` (Step 1) → `vox ci operations-sync --target cli --write` → `vox ci command-sync --write` (regenerates `docs/src/reference/cli-command-surface.generated.md`, which lists both rows today at lines 241 and 253). Confirmed by reading the registry and its generator, not guessed.
- **Is the CodeRabbit secret in the `SecretId` enum?** Yes — `SecretId::VoxCoderabbitGithubPerPage` (`crates/vox-secrets/src/spec/ids.rs:393`). It has exactly **one** consumer in the whole workspace: `crates/vox-cli-review/src/coderabbit/ingest.rs:100`. Since that consumer is deleted in the same step (Step 3), removing the enum variant breaks no surviving code — it is not a breaking change to any other consumer of the public enum.
- **Does any CI job FAIL if the bot never comments?** No. Grepped `.github/workflows/ci.yml` and `workflow-lint.yml` for coderabbit/wait/poll patterns: the only CI reference is a `cargo build -p vox-cli --locked --features …,coderabbit` compile-drift check (`ci.yml:115`) — it fails only if the `coderabbit` Cargo feature doesn't *compile*, not if the bot is silent. No job polls for a CodeRabbit comment or gates a merge on one. `.coderabbit.yaml` and `pre_push.rs` only affect whether a human *requests* a review, never CI pass/fail.
- **Is `contracts/review/coderabbit-semantic-groups.v1.yaml` consumed by anything besides CodeRabbit itself?** It's not consumed by the CodeRabbit *bot* at all — it's consumed by our own `vox review coderabbit semantic-submit` tool (`crates/vox-cli-review/src/coderabbit/semantic_planner/rules.rs`, loaded via `include_str!`) to group files for PR-splitting before submission, plus a catalog entry in `contracts/index.yaml`. Nothing else in the tree references the file (verified by grepping the exact filename across the repo). Safe to delete alongside the crate (Step 3/6).

## Unverified / needs owner decision

- **`crates/vox-cli/tests/coderabbit_e2e.rs`** and the `ingest_coderabbit_comments`/`is_coderabbit_nitpick` code in `crates/vox-code-audit/src/review/github.rs`, plus the `provider: "coderabbit"` string in `vox_corpus::external_review_replay` and `vox-db`'s `ExternalReviewRunParams`: these all treat "coderabbit" as **one historical data provider** in a generic external-review-ingest/training-corpus pipeline, not the live bot integration. They don't call out to the bot or depend on `vox-cli-review`. Whether the owner wants this historical-ingest path (and its test) kept for training-data purposes, or purged along with everything else, is a product decision this plan does not make.
- **`contracts/config/config-hygiene-baseline.txt:44`**: could not identify which `vox ci` subcommand generates/owns this baseline file in the time available — before hand-editing it, find and run that subcommand's `--write` (or equivalent) rather than deleting the line by hand.
- **`docs/superpowers/specs/2026-06-29-coderabbit-review-gui-and-sweep-design.md`** and its companion plan: whether these design docs (for the feature being retired) should be moved to `docs/src/archive/` or left in place is an owner call; this plan does not move them.
- **GUI e2e mock (`operatorShellMock.ts`)**: not read in depth — confirm it has no other surfaces mocked in the same block before deleting the CodeRabbit portion, so an unrelated mock isn't accidentally dropped.

## Riskiest step

**Step 3 (deleting the `vox-cli-review` crate) combined with Step 2 (removing the `coderabbit` Cargo feature) in one commit** is the highest-risk step: it's the largest surface-area change (an entire crate plus every `#[cfg(feature = "coderabbit")]` site across `vox-cli`), it must land atomically with the SSOT edit in Step 1 (dangling `commands::review` handler otherwise) and the secrets edit in Step 4 (dangling `SecretId` variant otherwise) to keep the build green, and it silently breaks the still-buildable `vox-gui` CodeRabbit panel (Step 5) at runtime rather than at compile time, since the GUI only talks to the CLI through a subprocess sidecar call.
