---
title: "Reference: `vox` CLI (minimal compiler binary)"
description: "Official documentation for the `vox` CLI — compiler driver, package manager, CI guards, and delegated toolchain binaries."
category: "Language Reference"
training_eligible: true
schema_type: "TechArticle"
---
# Reference: `vox` CLI (minimal compiler binary)

The **`vox`** executable is built from `crates/vox-cli` (repository root). This binary serves as the primary compiler driver and orchestrator. Starting in v0.5, specialized domains like **ML/AI training (`vox mens`)**, **mesh coordination (`vox populi`)**, and **speech-to-code (`vox speech`)** are decoupled into a separate binary (`vox-ml-cli`) but remain accessible through the main `vox` CLI via transparent delegation. (Training internals live in `vox-ml-cli`'s `commands::schola` module, reached via `vox mens train`.)

## Toolchain Binary Split

To minimize binary bloat and dependency sprawl, the Vox toolchain is split into the following authoritative binaries:

| Binary | Subcommands | Role |
|--------|-------------|------|
| `vox` | `build`, `check`, `run`, `pm`, `ci`, `dei`, `db` | Compiler core, package manager, and local orchestration. |
| `vox-ml-cli` | `mens`, `train`, `populi`, `speech` | Native ML training (QLoRA), inference serving, speech-to-code, and mesh coordination. |
| `vox` (core) | `scientia` | Research DB / capability-map facade (`commands::scientia`); stays in the main binary. |

If a delegated binary is missing from your `PATH`, the `vox` CLI prints actionable installation instructions.

### Install tiers

The release pipeline (`vox ci release-build --package <tier>`) ships these
artifacts per supported target triple, each as its own archive on the
[Releases page](https://github.com/vox-foundation/vox/releases):

| `--package` value | Produces | Use for |
|---|---|---|
| `vox` | `vox-<ver>-<target>.{tar.gz,zip}` | Lean install — compiler, package manager, orchestrator. No ML, no scientia. |
| `mens` | `vox-ml-cli-<ver>-<target>.{tar.gz,zip}` | ML / speech / populi / train plugin (heavy: Candle + Whisper). |
| `all` | Everything above | Full install for CI / dogfood. |

Download a plugin archive, extract the binary onto `PATH`, and `vox` will
dispatch automatically — no rebuild of the core required.


- **Global (before subcommand):** **`--color auto|always|never`** (see `NO_COLOR`), **`--json`** (sets `VOX_CLI_GLOBAL_JSON` for subcommands that support machine JSON), **`--verbose` / `-v`** (if `RUST_LOG` is unset, tracing uses `debug`), **`--quiet` / `-q`** (`VOX_CLI_QUIET`).
- **Completions:** **`vox completions bash`** | **`zsh`** | **`fish`** | **`powershell`** | **`elvish`** — print to stdout and install per your shell (e.g. bash: `vox completions bash > /path/to/bash_completion.d/vox`).
- **Dynamic command catalog:** **`vox commands`** — clap-derived list from the actual compiled binary; add `--recommended` for first-time essentials or `--format json --include-nested` for tooling.
- **Secrets namespace:** **`vox secrets`** (deprecated alias **`vox clavis`**) centralizes token health checks and credential compatibility storage.
- **Latin aliases (same behavior as flat commands):** **`vox fabrica`** (`fab`) — build/check/test/run/dev/**`bundle` (web app)** /fmt/script; top-level **`vox bundle`** is **plugin** bundles (`list` / `build` / `apply`), not the web-app bundler. **`vox diag`** — doctor, architect, stub-check; **`vox ars`** — snippet, share, skill, openclaw, ludus; **`vox recensio`** (`rec`, feature **`coderabbit`**) — same as **`vox review`**.

### Naming, aliases, and discovery (maintainer rules)

- **One canonical path per operation** in [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml); regenerate **`contracts/cli/command-registry.yaml`** with **`vox ci operations-sync --target cli --write`**, then keep **`vox ci command-compliance`** green (see [Command compliance](command-compliance.md)).
- **Latin mirrors** (`fabrica`, `diag`, `ars`, …) are intentional — do not delete them for “simplicity”; add English docs/examples beside them when introducing new verbs.
- **Aliases:** prefer **hidden** `alias = "…"` for legacy spellings; use **`visible_alias`** only when the alias is part of the supported learning path.
- **Disambiguate overloaded nouns in docs:** **`vox review`** (feature **`coderabbit`**) is batch PR review; **`vox mens review`** is the MENS lane (different feature gates). **`vox doctor`** is toolchain/environment diagnostics; **`vox secrets status`** may accept the hidden alias `doctor` for legacy scripts — prefer **`vox secrets status`** in new docs.
- **Machine vs human output:** prefer **`--json`** + stable stderr for progress; global **`--quiet` / `-q`** sets **`VOX_CLI_QUIET=1`** for commands that honor it.

### Product lanes

The command registry also carries a separate **`product_lane`** value used for bell-curve planning and discoverability. This is not a CLI rename and does not replace **`latin_ns`**.

| `product_lane` | Meaning | Representative commands |
|----------------|---------|-------------------------|
| `app` | typed app construction | `vox build`, `vox run`, `vox deploy` |
| `workflow` | automation and background execution | `vox script`, `vox populi` |
| `ai` | generation, review, eval, orchestration | `vox mens`, `vox review`, `vox orchestrator` (`vox dei`), `vox oratio` |
| `interop` | approved integration surfaces | `vox openclaw`, `vox skill`, `vox share` |
| `data` | database and publication workflows | `vox db`, `vox codex`, `vox scientia` |
| `platform` | packaging, diagnostics, compliance, secrets | `vox pm`, `vox ci`, `vox doctor`, `vox secrets`, `vox telemetry` |

## Visus (`vox visus`)

The `vox visus` suite provides visual intelligence, agentic GUI reasoning, and accessibility audits.

| Command | Role |
|---------|------|
| **`vox visus audit`** | Run accessibility and structural audits on web components using visual DOM parsing. |
| **`vox visus baseline`** | Manages golden visual baselines for regression testing. |

## Package management (`vox-package`)

Project dependencies are **declared** in `Vox.toml`, **locked** in `vox.lock`, and **materialized** under `.vox_modules/`. This is separate from **`vox upgrade`**, which refreshes the **Vox toolchain** (never edits `Vox.toml` / `vox.lock`): either a **release binary** or a **local git checkout** + source install.

Rust crate imports declared in `.vox` files (`import rust:<crate> ...`) are compiled into generated `Cargo.toml` dependencies. `vox.lock` remains the high-level Vox dependency contract; `Cargo.lock` is generated by Cargo at build time from the emitted manifest.

| Command | Role |
|---------|------|
| **`vox add`** `<name>` `[--version …] [--path …]` | Add a dependency stanza to `Vox.toml` only. |
| **`vox remove`** `<name>` | Remove a dependency from `Vox.toml`. |
| **`vox update`** | Refresh `vox.lock` from the local PM index (`.vox_modules/local_store.db`); skips missing index entries with warnings. |
| **`vox lock`** `[--locked]` | Resolve `Vox.toml` strictly and write `vox.lock`; `--locked` checks the lock matches without writing. |
| **`vox sync`** `[--registry URL] [--frozen]` | Download registry artifacts per `vox.lock` into `.vox_modules/dl/`; **`--frozen`** requires the lock to match a strict resolution. |
| **`vox deploy`** `[ENV]` `[--target …] [--runtime …] [--dry-run] [--detach] [--locked]` | Apply **`[deploy]`** in `Vox.toml` via **`vox-container`** { OCI build/push, Compose, Kubernetes manifests, or bare-metal SSH + systemd. **`ENV`** defaults to `production` (image tag suffix). **`--locked`** requires `vox.lock` to exist. See [`vox-portability-ssot.md`](vox-portability-ssot.md), [`deployment-compose.md`](deployment-compose.md). |
| **`vox upgrade`** | **Check-only** by default. **`--source release`** (default): **`--apply`** downloads release assets, verifies **`checksums.txt`**, installs into **`CARGO_HOME/bin`** (**`--provider`**, **`--repo`**, **`--version`**, semver gates, **`--allow-breaking`**, **`--allow-prerelease`**, **`--channel`**). **`--source repo`**: **`--apply`** runs **`git fetch`**, fast-forwards the tracked branch (or checks out **`--ref`**), then **`cargo install --locked --path crates/vox-cli`**; refuses a dirty worktree unless **`--allow-dirty`**; rolls back **`HEAD`** if install fails. Use **`--repo-root`** or **`VOX_REPO_ROOT`**; **`--remote`** / **`--branch`** when there is no upstream — **not** **`vox update`**. |
| **`vox pm`** `search \| info \| publish \| yank \| vendor \| verify \| mirror \| cache …` | Registry and operator workflows (HTTP search, publish with `VOX_REGISTRY_TOKEN`, vendor tree, verify hashes, **`mirror`** local artifact into the PM index for offline `vox lock`, cache status/clear). |

Explicit advanced verbs (registry parity): **`vox pm search`**, **`vox pm info`**, **`vox pm publish`**, **`vox pm yank`**, **`vox pm vendor`**, **`vox pm verify`**, **`vox pm mirror`** (`--file` *or* **`--from-registry`**), **`vox pm cache status`**, **`vox pm cache clear`**.

Git-source note: `vox sync` and `vox pm verify` do not fetch/verify git payloads in-repo yet. They fail fast by default; for explicit operator bypass in controlled environments set `VOX_PM_ALLOW_GIT_UNVERIFIED=1`.

**Removed:** the old **`vox install`** package verb — use **`vox add`**, **`vox lock`**, **`vox sync`**, and **`vox pm`** instead (`vox install` is an unrecognized subcommand).

**Migration note (old → new verbs):** [`pm-migration-2026.md`](pm-migration-2026.md).

Design rules and registry parity: [`cli-design-rules-ssot.md`](../archive/research-2026-q1/cli-design-rules-ssot.md), [`command-compliance.md`](command-compliance.md). Generated command table: [`cli-command-surface.generated.md`](cli-command-surface.generated.md) (`vox ci command-sync --write`).

**Environment variables:** canonical names and precedence — [`reference/env-vars.md`](env-vars.md) (alias: [`ref/env-vars.md`](../ref/env-vars.md)).

## Build & run

### `vox build <file>`

Compile a `.vox` source file.

| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--out-dir` | `dist` | Directory for generated **TypeScript** (and related frontend files) |
| `--scaffold` | off | When set, writes one-shot user scaffold files next to the project root (`app/App.tsx`, Vite, Tailwind v4, `components.json`) if they are missing — same as `VOX_WEB_EMIT_SCAFFOLD=1` |
| _(positional)_ | — | Path to the `.vox` file |

**Also writes** generated **Rust** under `target/generated/` (backend crate). If the module declares `@v0` UI components and output files are missing, the CLI invokes Vercel's `npx v0 add` sidecar process.

### `vox island …` (retired 2026-05-03)

The `vox island` subcommand was retired with the rest of the islands surface; UI now
ships as plain React/TSX components emitted from `component` declarations and consumed
by an external React frontend. See
[architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).
For AI-generated React components use `@v0` directly inside `.vox` source.

### `vox generate` (HTTP inference) vs MCP codegen

Top-level **`vox generate`** (`crates/vox-cli/src/commands/generate.rs`) posts to a **local HTTP** inference server via the orchestrator (default endpoint from `VOX_LOCAL_ENDPOINT`). It is intentionally narrow: QLoRA / playground style validation loops without requiring MCP.

**`--legacy-direct` flag (deprecated escape hatch):** bypasses the orchestrator and calls the inference server directly at `--server-url` (default `http://127.0.0.1:7863`). This was the pre-orchestrator behavior (pre-Task 1.9). Avoid unless debugging direct inference — it skips TTL-cached health probes, consistent endpoint resolution, and aligned telemetry.

**`vox_generate_code`** (and related MCP chat tools) use the **workspace orchestrator + Codex** path: model registry / Ludus routing, optional workspace journey DB, structured transcripts with [`journey-envelope.v1`](../../../contracts/orchestration/journey-envelope.v1.schema.json), and `routing_decisions` rows. The CLI HTTP path does **not** silently provide the same joins — use MCP when you need that unified telemetry story. A later optional bridge (for example an explicit MCP-backed codegen flag) would make the difference obvious in UX.

### `vox run <file> [-- <args>…]`

1. Runs the same pipeline as `build` (output to `dist/`).
2. If `.tsx` files are present under `dist/`, scaffolds a Vite app, runs **`pnpm install`** / **`pnpm run build`**, and copies assets into `target/generated/public/`.
3. Runs `cargo run -- <args>` in `target/generated`.

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | _(from `VOX_PORT` or 3000)_ | Sets `VOX_PORT` for the generated Axum server and Vite `/api` proxy |
| `--mode` | `auto` | `app` = always generated server; `script` = `fn main()` script lane (**needs** `cargo build -p vox-cli --features script-execution`); `auto` = script lane when the file has no `@page` and the binary was built with `script-execution`. |

Backend listens on the port from **`VOX_PORT`** (or **3000**) — same variable the generated `main.rs` reads.

**pnpm workspace (repo root):** when the scaffold wrote **`pnpm-workspace.yaml`** at the repository root (for example **`dist/.../app`** plus other packages), run **`pnpm install`** once from that root so workspace packages link correctly, then use per-package **`pnpm run build`** / **`pnpm run dev`** as needed. See [tanstack-web-backlog.md](../archive/research-2026-q1/tanstack-web-backlog.md) Phase 3.

### `vox script <file> [-- <args>…]` (feature `script-execution`)

**Not in default builds.** Same script runner as `vox run --mode script`, with explicit flags: `--sandbox`, `--no-cache`, `--isolation`, `--trust-class`. Build: `cargo build -p vox-cli --features script-execution`.

When **`VOX_MESH_ENABLED=1`** and the binary is built with **`--features populi`** (pulls in `vox-populi`; optionally combine with **`script-execution`**), `vox script` / script-mode `vox run` **best-effort** publishes a node record to the local registry file (see [mens SSOT](populi.md)).

### `vox populi …` (feature `populi`)

**Not in default builds.** One-command private mesh lifecycle helpers backed by the same Populi control plane. Build: `cargo build -p vox-cli --features populi`.

**Optional NVML-backed GPU inventory** on join/heartbeat `NodeRecord`s (ADR 018 Layer A): add **`mesh-nvml-probe`** (e.g. `cargo build -p vox-cli --features populi,mesh-nvml-probe`). Requires NVIDIA driver/NVML at runtime; see [GPU truth probe spec](../archive/research-2026-q1/populi-gpu-truth-probe-spec.md).

| Subcommand | Role |
|------------|------|
| `vox populi up` | Bootstraps a private populi config (`.vox/populi/mesh.env`), generates `VOX_MESH_TOKEN` + `VOX_MESH_SCOPE_ID` by default, and starts `vox populi serve` in the background. Supports `--mode lan|overlay`, `--bind`, and `--insecure-local` for dev-only LAN use. |
| `vox populi down` | Stops the background control-plane process recorded in `.vox/populi/mesh-state.json`. |
| `vox populi status` | Shows control-plane health (`/health`), token/scope posture, and overlay diagnostics (tailscale/wireguard/tunnel availability/connection hints). |
| `vox populi registry-snapshot` | Print local env and on-disk registry path + nodes (`--registry` override; `--json`; alias: `local-status`). |
| `vox populi serve` | Bind HTTP (`--bind 127.0.0.1:9847`); optional `--registry` seeds in-memory state from a JSON file. |
| `vox populi admin maintenance --node <id> --state on\|off [--until-unix-ms <ms> \| --for-minutes <n>]` | Cooperative drain; optional timed auto-clear (HTTP body `maintenance_until_unix_ms` or `maintenance_for_ms`). Use one optional timing flag with `--state on`. Same URL and bearer as other admin commands. |
| `vox populi admin quarantine --node <id> --state on\|off` | Quarantine toggle (`POST /v1/populi/admin/quarantine`). Same URL and auth as maintenance. |
| `vox populi admin exec-lease-revoke --lease-id <id>` | Operator removes a remote exec lease row (`POST /v1/populi/admin/exec-lease/revoke`); no holder `release` required. Same control URL and mesh/admin bearer as other admin commands. |

Interpreted **`vox mens workflow run`** (journal + `mesh_*` activity hooks; there is no top-level `vox workflow`) requires **`--features workflow-runtime`** (implies `mens-dei` + `vox-workflow-runtime`). The runtime emits versioned journal events (`journal_version: 1`) and durable rows keyed by a **run id** plus **`activity_id`**. Use `--run-id <id>` to resume the same interpreted workflow run; omit it to start a fresh run id. The interpreted runner can replay stored step results for linear workflows. Mens steps use **env-derived** `VOX_MESH_CONTROL_ADDR` / `Vox.toml` `[mens]` only — use `with { timeout: …, retries: …, initial_backoff: …, activity_id: …, id: …, mens: "noop" | "join" | "snapshot" | "heartbeat" }` on `mesh_*` calls (`id` is an alias for `activity_id`). Retry/backoff support currently applies to interpreted `mesh_*` activity execution; other interpreted activities remain journal-only no-ops. Codex append is enabled by default when DB config resolves and can be disabled with **`VOX_WORKFLOW_JOURNAL_CODEX_OFF=1`** ([orchestration SSOT](orchestration-unified.md), [durable execution](../explanation/expl-durable-execution.md)).

### `vox graph …`

Corpus registry and freshness status/ingest operations for Vox Graph knowledge maps (`contracts/retrieval/graphify-corpora.v1.yaml`). Aliases: `vox graphify`, `vox search`.

| Subcommand | Role |
|------------|------|
| `vox graph status` | Report per-corpus freshness (node/edge counts, graph path). `--corpus <id>` limits output; `--strict` exits non-zero when any corpus is stale; `--json` emits machine-readable status. |
| `vox graph ingest` | Project graph nodes into Turso `knowledge_nodes` via VoxDb. |
| `vox graph rebuild` | Rebuild the base AST code graph and cluster it. |

### `vox ci …`

Repository guards (manifest lockfile, docs/Codex SSOT, `vox-cli` feature matrix, doc inventory, milestone eval matrix contract, workflow `scripts/` allowlist, Mens gate matrix, TOESTUB scoped scan, optional CUDA checks). **Canonical:** **`vox ci <subcommand>`** when `vox` is on `PATH`. **CI/bootstrap:** `cargo run -p vox-cli --quiet -- ci <subcommand>` from the repo root (same code path).



| Subcommand | Role |
|------------|------|
| `manifest` | `cargo metadata --locked` |
| `check-docs-ssot` / `check-codex-ssot` | Required doc / Codex files + inventory / OpenAPI checks |
| `check-summary-drift` | Runs `cargo run -p vox-doc-pipeline -- --check`; fails if `SUMMARY.md` is out of sync with `docs/src` |
| `build-docs` | Regenerates `SUMMARY.md`, runs `mdbook build docs`, then `mdbook-sitemap-generator` (optional `MDBOOK_SITEMAP_DOMAIN`) |
| `check-links` | Fails on broken internal Markdown links under `docs/src` and root-level guides |
| `artifact-audit [--json]` | Inventory of workspace artifact classes (stale renames, repo-root `target-*` sprawl, OS-temp Cargo targets, `mens/runs/*`, root scratch files, canonical `target/`). JSON optional. Policy defaults: [`contracts/operations/workspace-artifact-retention.v1.yaml`](../../../contracts/operations/workspace-artifact-retention.v1.yaml) |
| `artifact-prune --dry-run \| --apply [--policy <path>]` | Prune untracked artifact paths per retention policy (requires exactly one of `--dry-run` or `--apply`). Skips git-tracked paths; Windows delete failures may rename to `*.stale-<epoch>`. |
| `ai-fixtures-coverage` | Static parity between [`contracts/agentos/ai-first-fixtures.v1.yaml`](../../../contracts/agentos/ai-first-fixtures.v1.yaml) and lexer tokens, `HirAiFixture` grafts, Rust AI fixture diagnostics, and TS emitter markers. |
| `backend-tests` | Runs `cargo test -p vox-actor-runtime`, `cargo test -p vox-orchestrator model_route_policy`, `cargo test -p vox-db research_metrics_contract`, `cargo test -p vox-sql --test p2_conformance`, `cargo test -p vox-sql --test p3_introspect_smoke`, `cargo test -p vox-sql --test p5_ddl_conformance`, and `cargo test -p vox-sql --test p5_migrate_smoke` (routing, orchestrator policy tests, VoxDb contract checks, and SQL interop P2/P3/P5 smoke lanes). |
| `doc-inventory generate \| verify` | Regenerate or verify `docs/agents/doc-inventory.json` (Rust; replaces retired Python scripts) |
| `docs-reality-audit verify \| metrics [--write]` | Documentation Reality Audit: validates [`contracts/reports/docs-reality-audit/`](../../../contracts/reports/docs-reality-audit/) JSON against schemas + inventory path hints; **`metrics`** refreshes `metrics.v1.json`. See [Documentation Reality Audit Program](../contributors/docs-reality-audit-program.md). Also runs inside **`ssot-drift`**. |
| `test-inventory [--json] [--output <path>] [--markdown <path>] [--check <path>]` | Regenerable workspace test inventory (Rust test attrs, ignores, golden Vox `@test`, app E2E paths, doctest candidates, harness-pattern totals). **`--check`** parses committed JSON and the fresh report and fails on structured inequality (not a raw text compare). |
| `config-hygiene [--update-baseline]` | Config guardrails gate: (A) no cwd-relative `contracts/...` paths in source, (B) no direct `env::var` reads in protected never-configure modules, (C) advisory declared-but-unwired config-resolver detector. Uses a baseline ratchet (`contracts/config/config-hygiene-baseline.txt`); `--update-baseline` regenerates it. |
| `config-registry-parity` | Verifies every `VOX_*` env knob read in source code is registered in the config registry. Fails if any operational env var is read outside the approved resolver path. |
| `safety-inventory [--json] [--output <path>] [--check <path>]` | Machine-readable safety / suppression snapshot (Rust `unsafe {` counts, ignored-test totals + governance violations, crate-root `#![allow]`, TS `eslint-disable` / `as any`). Committed baseline: [`contracts/reports/safety-inventory/baseline.v1.json`](../../../contracts/reports/safety-inventory/baseline.v1.json). |
| `test-runtime-report --junit <path> [--json] [--markdown <path>] [--top N]` | Parses nextest/JUnit XML (e.g. CI artifact under `target/nextest/ci/junit.xml`). Emits JSON or a human summary: totals, top slow tests (max time per classname+name), retry/flaky **heuristics** (duplicate `<testcase>` rows; `<flakyFailure>` / `<rerunFailure>` children). **`--fail-over-ms`** / **`--fail-retry-count`** log warnings only (no non-zero exit). |
| `ignored-test-age [--mode <warn|enforce>] [--inventory <test-inventory.json>] [--json]` | Governance gate on `#[ignore]` usage under `crates/` (unit/integration/bench paths). **`--mode warn`** (default): prints violations to stderr, exits 0. **`--mode enforce`**: fails if any ignored test uses bare `#[ignore]` or an `#[ignore = "..."]` reason without owner/sunset/date-style markers. Optional **`--inventory`** checks `summary.cargo_ignored_test_functions` matches the live scan. |
| `flake-budget (--report-json <path> \| --junit <path>) [--mode <warn|enforce>] [--max-candidates N] [--top N] [--json]` | Governance gate on retry/flaky candidate rows from **`test-runtime-report` JSON** or raw JUnit (same parser as `test-runtime-report`). Default **`warn`**. **`--mode enforce`** fails when candidate count exceeds **`--max-candidates`** (default 50). |
| `runtime-regress --current <report.json> --baseline <report.json> [--mode <warn|enforce>] [--percent P] [--absolute-ms MS] [--json]` | Compares two **`test-runtime-report --json`** payloads: flags slowdowns where delta exceeds **`--percent`** (default 25) or **`--absolute-ms`** (default 500ms). Only tests present in **both** top-slow slices are compared; baseline-only entries are reported as missing in current top. Default **`warn`**. |
| `eval-matrix verify` | Validates `contracts/eval/benchmark-matrix.json` against `contracts/eval/benchmark-matrix.schema.json` (M1–M5 milestones; `benchmark_classes` ids are a fixed enum in the schema) |
| `eval-matrix run [--milestone <id>]` | Runs `cargo` checks/tests mapped from each `benchmark_classes` entry (deduped); always re-runs `verify` first |
| `mens-scorecard verify \| run \| decide \| burn-rnd \| ingest-trust` | Validates and executes the Mens scorecard harness (`contracts/eval/mens-scorecard*.json`), computes promotion decisions from scorecard summaries, and can ingest `summary.json` into VoxDb trust observations. |
| `feature-matrix` / `no-dei-import` | `vox-cli` compile matrix + import guard (alias: `no-vox-orchestrator-import`) |
| `workflow-scripts` | Fail if `.github/workflows/*.yml` references `scripts/…` not in `docs/agents/workflow-script-allowlist.txt` |
| `line-endings` | Forward-only: changed LF-policy files must not contain CR/CRLF (`*.ps1` exempt). Env: `GITHUB_BASE_SHA` / `GITHUB_SHA`, or `VOX_LINE_ENDINGS_BASE` (+ optional `VOX_LINE_ENDINGS_HEAD`). Flags: `--all`, `--base <ref>` |
| `parse-status [--write]` | Regenerate or verify [`examples/PARSE_STATUS.md`](../../../examples/PARSE_STATUS.md) from `examples/golden/*.vox` parse results (`--write` updates the file). |
| `mesh-gate --profile ci_full \| m1m4 \| training` | Runs `scripts/populi/gates.yaml` steps (CLI falls back to `scripts/mens/gates.yaml` if present). **`--isolated-runner`** builds `vox-cli` under OS temp `…/vox-targets/<repo-hash>/mens-gate-safe` by default (override `--gate-build-target-dir`), copies `vox` to a temp path, and re-invokes the gate (**Windows + Unix**; avoids file locks). Hidden alias: `--windows-isolated-runner`. Legacy argv alias: `mens-gate`. Optional `--gate-log-file <path>` tees child output. |
| `mens-corpus-health`, `grpo-reward-baseline`, `collateral-damage-gate`, `constrained-gen-smoke` | **Placeholders** (print-only; no DB, corpus, or GRPO checks). Prefer **`mesh-gate`** and **`vox mens corpus …`** for real gates. Clap `--help` on each subcommand also marks placeholder intent. |
| `toestub-self-apply` | `cargo build -p vox-code-audit --release` then full-repo `toestub` scan (replaces `scripts/toestub_self_apply.*`) |
| `toestub-scoped` | Default scan `crates/vox-repository` |
| `scaling-audit verify \| emit-reports` | Scaling SSOT: validate `contracts/scaling/policy.yaml`; `emit-reports` regenerates per-crate backlog markdown + rollup + TOESTUB JSON on `crates/` |
| `cuda-features` | Optional CUDA compile checks when `nvcc` exists |
| `cuda-release-build` | `cargo build -p vox-cli --bin vox --release --features gpu,mens-candle-cuda` with tee to `mens/runs/logs/cuda_build_<UTC>.log` (same intent as workspace alias **`cargo vox-cuda-release`** / `scripts/populi/cursor_background_cuda_build.ps1`; needs nvcc + MSVC toolchain on Windows) |
| `data-ssot-guards` | Fast static checks for telemetry / DB SSOT drift: `vox mens watch-telemetry` keys vs Populi schema, required policy docs, and no `COALESCE(metric_value, …)` in codex `research_metrics` paths |
| `db-schema-coverage` | Verifies every `CREATE TABLE` in the workspace is owned by a crate in `tiers.a_relational.owners` (data-storage policy SSOT). |
| `build-bench [--label <name>] [--repeat <n>] [--write <path>] [--compare <path>]` | Reproducible wall-clock build scenarios from `contracts/ci/build-bench-scenarios.v1.json`. Each scenario `touch`es a source file, runs `cargo check`, and records wall-clock ms. **`--compare <baseline>`** diffs against a committed snapshot and emits a Markdown phase-delta table to stdout and `graphify-out/build-bench/REPORT.md`. **`--write <path>`** saves the full snapshot JSON. Default `--repeat 3` keeps the minimum run. Used by the build-time reduction program to track phase-over-phase deltas against `contracts/ci/build-bench-baseline.v1.json`. |
| `build-timings` | Wall-clock `cargo check` lanes: default `vox-cli`, GPU+stub, optional CUDA when `nvcc` is on `PATH` or under `CUDA_PATH`/`CUDA_HOME`; **`--json`** one object per line; **`--crates`** adds `vox-cli --no-default-features`, `vox-db`, `vox-oratio`, `vox-populi --features mens-train`, `vox-cli --features oratio`. Budgets: `docs/ci/build-timings/budgets.json`; env `VOX_BUILD_TIMINGS_BUDGET_WARN` / `VOX_BUILD_TIMINGS_BUDGET_FAIL`; `SKIP_CUDA_FEATURE_CHECK=1` skips CUDA lane. |
| `grammar-export-check` | Emits EBNF/GBNF/Lark/JSON-Schema from `vox-grammar-export`; fails on empty output or zero rules (wired in **main** `.github/workflows/ci.yml`). |
| `grammar-drift` | Compare/update EBNF SHA-256 vs `mens/data/grammar_fingerprint.txt` (+ Populi twin); `--emit github` for CI. **Primary workflow:** `.github/workflows/ml_data_extraction.yml` (data/ML lane), not the default Linux `ci.yml` job. |
| `repo-guards` | TypeVar / `opencode` / stray-root file guards |
| `nomenclature-guard` | Enforces the English-first crate naming policy (Phase 5). |
| `secret-env-guard [--all]` | Fails if Rust files add direct managed-secret env reads outside allowed modules (default: `git diff` changed files; set **`VOX_SECRET_GUARD_GIT_REF`** to a merge-base range on clean CI checkouts; `--all` scans all crates). |
| `sql-surface-guard [--all]` | Fails if sources use `connection().query(` / `connection().execute(` outside [`docs/agents/sql-connection-api-allowlist.txt`](../../../docs/agents/sql-connection-api-allowlist.txt) plus built-in `vox-db` / `vox-compiler` prefixes, and fails if Rust sources use `sqlx` path imports outside `crates/vox-sql/` (see [`docs/agents/database-nomenclature.md`](../../../docs/agents/database-nomenclature.md)). |
| `query-all-guard [--all]` | Fails if sources call the Codex `query_all` facade escape hatch outside [`docs/agents/query-all-allowlist.txt`](../../../docs/agents/query-all-allowlist.txt) plus `crates/vox-db/` (same nomenclature doc). |
| `turso-import-guard [--all]` | Fails if sources use the Turso crate path prefix outside [`docs/agents/turso-import-allowlist.txt`](../../../docs/agents/turso-import-allowlist.txt) plus built-in `vox-db` / `vox-package` / `vox-compiler` prefixes ([codex-turso-allowlist](../archive/research-2026-q1/codex-turso-allowlist.md)). |
| `policy-allowlist-parity` | Verifies `allow_direct_access` in `contracts/db/data-storage-policy.v1.yaml` matches [`docs/agents/turso-import-allowlist.txt`](../../../docs/agents/turso-import-allowlist.txt). |
| `retirement-audit` | Enforces removal of `vox-deprecated-since` markers whose `retire-by` version has been reached. Scans workspace Rust sources; fails when any marker's `retire-by` semver ≤ current workspace version. |
| `secrets-parity` | Verifies Secrets SSOT parity between managed secret specs and [`secrets-ssot.md`](secrets-ssot.md). Visible alias: **`clavis-parity`**. |
| `release-build --target <triple> [--version <tag>] [--out-dir dist] [--package vox\|mens\|all]` | Build and package allowlisted release artifacts (`cargo build --locked --profile dist`): `vox`, `vox-ml-cli`, or both. Unix archives are `.tar.gz`; Windows archives are `.zip`. Writes `checksums.txt` with one line per artifact (`<sha256>` + two spaces + `<basename>`). Contract: [`docs/src/ci/binary-release-contract.md`](../ci/binary-release-contract.md) |
| `command-compliance` | Validates `contracts/cli/command-registry.yaml` (and schema) against `vox-cli` top-level commands, CLI reference (`docs/src/reference/cli.md` or legacy `ref-cli.md`), reachability SSOT, compilerd/dei RPC names, MCP tool registry, script duals, and **`contracts/operations/completion-policy.v1.yaml`** (JSON Schema) — blocks orphan CLI drift |
| `completion-audit [--scan-extra <DIR>]…` | Scans **`crates/`** (always) plus optional extra directories under the repo (generated apps, codegen trees). Same detectors; paths must exist and resolve under the repository root. Writes **`contracts/reports/completion-audit.v1.json`**. CI uses **`--features completion-toestub`** to merge TOESTUB `victory-claim` (Tier C). |
| `completion-gates [--mode warn\|enforce]` | Applies Tier A hard blocks and Tier B regression limits from **`contracts/reports/completion-baseline.v1.json`** to the last audit report (CI uses **`enforce`**) |
| `completion-ingest [--report <path>] [--workflow …] [--run-kind …]` | Inserts the audit report into VoxDB **`ci_completion_*`** tables (optional telemetry; requires a working local/default DB) |
| `rust-ecosystem-policy` | Runs focused rust ecosystem contract parity checks (`cargo test -p vox-compiler --test rust_ecosystem_support_parity`) for faster local iteration than full CI suites |
| `policy-smoke` | Fast bundle: `cargo check -p vox-orchestrator`, in-process `command-compliance`, and `cargo test -p vox-compiler --test rust_ecosystem_support_parity` (same parity test as `rust-ecosystem-policy`) |
| `workflow-concurrency-guard [--strict]` | Requires a top-level `concurrency:` block containing `cancel-in-progress: true` on every `push`/`pull_request`-triggered workflow (a bare group string or a non-cancelling group does not count) — flood prevention for the local runner fleet; exceptions registered in [`concurrency-exceptions.md`](../ci/concurrency-exceptions.md). Advisory by default; `--strict` fails (used in `pre-push`). |
| `queue [--json] [--brief] [--from-snapshot] [--clear] [--dry-run] [--ttl-mins <n>] [--hook-guard]` | Local-first CI queue signal: classifies GitHub Actions runs active/superseded/stale, carries the async failure signal, and clears cancellable backlog. `--json`/`--brief`/plain read; `--from-snapshot` reads `~/.vox/ci-queue-snapshot.json` with no network; `--clear` cancels (live data only, capped, exempt-aware); `--hook-guard` is the PreToolUse enforcement mode. See [local-first-ci.md](../ci/local-first-ci.md). |
| `compile-matrix` | Smoke-checks `vox compile --help` through the CLI binary for cross-host parity with `compile-matrix.yml`. |
| `pre-push [--dry-run] [--quick] [--complete] [--full] [--report-json <path>]` | **Default (fast):** `cargo fmt --check`, line-endings, ssot-drift, `runner-policy-check`, `workflow-concurrency-guard`, **scoped** doc lint + doctest on changed `docs/src/**/*.md` (excludes `archive/`), and **`vox-drift-check`** — tuned for responsive `git push`. **`--complete`:** full static gate (whole-tree doc lint + doctest under `docs/src/`, doc-inventory, workspace **`clippy -D warnings`**, scoped TOESTUB). **`--full`:** **`--complete`** plus **`cargo nextest run --workspace --profile ci --no-fail-fast`**. **`--quick`** is a legacy alias for the default fast profile (conflicts with **`--complete`** / **`--full`**). Long steps print a ~3s heartbeat on stderr. **`--dry-run`** prints planned steps only. **`--report-json`** writes timings (`contracts/reports/pre-push-report.v1.schema.json`, `schema_version` 2 adds `profile`). Env **`VOX_PREPUSH_AUDIT_LOG`**: append one JSON line per successful run. CI still runs full docs-quality / merge gates. |
| `dep-cycles` | Runs Tarjan SCC over `cargo metadata` to detect workspace dependency cycles. **Hard-fails** (non-zero exit) on normal link-time cycles (cargo would reject them anyway, but this surfaces them earlier). Reports dev/build back-edge cycles as advisory (legal in cargo; written to `graphify-out/DEP_CYCLES.md`). Blocking gate in CI after arch-check. |
| `dev-loop-audit [--json]` | Heuristics for AI/local **inner-loop** overhead: detects **`CARGO_TARGET_DIR`** fragmentation vs repo **`target/`**, prints hints (use **`cargo check -p` / `cargo nextest run -p`** before **`vox ci pre-push`**). **`--json`** matches **`contracts/reports/dev-loop-audit.v1.schema.json`**. |
| `gui-smoke` | GUI regression bundle: runs ignored-only **`cargo nextest`** on **`web_ir_lower_emit_test`** (TanStack/router codegen guard — matches compiler CI smoke); when **`VOX_WEB_VITE_SMOKE=1`**, also ignored-only **`web_vite_smoke_test`**; when **`VOX_GUI_PLAYWRIGHT=1`**, ignored-only **`playwright_golden_route_test`** (requires `pnpm install` + `pnpm exec playwright install chromium` under `crates/vox-integration-tests`). Requires **`cargo-nextest`** on `PATH`. |
| `coverage-gates` | Compares `cargo llvm-cov report --json --summary-only` output to `.config/coverage-gates.toml`: `--summary-json <path>`, `--config` (default `.config/coverage-gates.toml`), `--mode warn\|enforce` (GitHub CI uses **`enforce`** with `workspace_min_lines_percent` in `.config/coverage-gates.toml`). Run this **after** `cargo llvm-cov nextest --workspace --profile ci`; the **`report`** subcommand does not accept `--workspace` (it merges the prior instrumented run’s profraw data). |
| `command-sync [--write]` | Regenerates or verifies [`cli-command-surface.generated.md`](cli-command-surface.generated.md) from `command-registry.yaml` (after `operations-sync --target cli`, run `--write` to refresh the table) |
| `operations-verify` | Validates [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml) vs committed MCP/CLI/capability registries (strict projections), dispatch + input schemas + read-role governance, inventory JSON |
| `operations-sync --target catalog\|mcp\|cli\|capability\|all [--write]` | Writes or verifies artifacts from the operations catalog (`all` = mcp → cli → capability) |
| `pipeline-parity` | Umbrella gate for script→emission SSOT: grammar SSOT, canonical golden ladder, feature-matrix smoke, and k-complexity budget (ladder-scoped) |
| `capability-sync [--write]` | Regenerates or verifies [`contracts/capability/model-manifest.generated.json`](../../../contracts/capability/model-manifest.generated.json) from the capability + MCP + CLI registries (run after `operations-sync --target capability`) |
| `pm-provenance [--strict] [--root <dir>]` | Validates `vox.pm.provenance/1` JSON under `<dir>/.vox_modules/provenance/` (emitted by **`vox pm publish`**). Without **`--strict`**, missing/empty dir is OK. Use **`--strict`** on release pipelines after publishing. |
| `contracts-index` | Validates `contracts/index.yaml` against `contracts/index.schema.json`, checks every listed contract path exists, and validates indexed YAML contracts against their index-listed JSON Schema when the schema id follows `{contract-id}-schema` (plus a small explicit override table for historical id pairs) |
| `coolify-eval discover \| sync-compose [--compose <path>] [--app-uuid <uuid>] [--domains <url>] [--no-deploy]` | Coolify HTTP API (Bearer via env / Clavis): **`discover`** prints `/api/v1/version` when present and lists applications (`uuid`, name, `fqdn`, whether `docker_compose_raw` is set). **`sync-compose`** reads `vox-eval.compose.yml` (default), **`PATCH`**es `COOLIFY_APP_UUID`’s `docker_compose_raw`, optionally sets **`domains`**, and triggers **`GET /api/v1/deploy?uuid=`** unless **`--no-deploy`**. Requires **`COOLIFY_BASE_URL`**, **`COOLIFY_TOKEN`** (write/deploy); optional **`COOLIFY_READ_TOKEN`** for read-only **`GET`**s. See [eval sandbox deployment](../architecture/eval-sandbox-deployment.md) and [deploy contract](../ci/deploy-contract.md). |
| `exec-policy-contract` | Validates `contracts/terminal/exec-policy.v1.yaml` against `exec-policy.v1.schema.json` and (when `pwsh`/`powershell` is on PATH) smoke-runs `vox shell check` on `Get-Location` and a small pipeline payload (`Write-Output 1 \| ConvertTo-Json -Compress`) |
| `openclaw-contract` | Validates OpenClaw protocol fixture contracts under `contracts/openclaw/protocol/` (required event/response shapes). |
| `scientia-worthiness-contract` | Validates `contracts/scientia/publication-worthiness.default.yaml` against `publication-worthiness.schema.json` and publisher invariants (weights sum, threshold ordering) |
| `scientia-novelty-ledger-contracts` | Validates example `contracts/reports/scientia-finding-candidate.example.v1.json` and `scientia-novelty-evidence-bundle.example.v1.json` against `finding-candidate.v1.schema.json` and `novelty-evidence-bundle.v1.schema.json` |
| `speech-runtime-suite [--run-id <id>] [--plugins-dir <dir>] [--limit <n>] [--skip-runtime]` | Runs the speech-to-code MUST+SHOULD audit matrix from `contracts/speech-to-code/audit-matrix.v1.yaml`, executes the CPU Candle Oratio eval when runtime is enabled, and writes per-cell `cell_result.json`, `cell_result.kpi.json`, and aggregate `scorecard.json` under `.vox/audit/<run-id>/`. |
| `ssot-drift` | Runs `check-docs-ssot`, `check-codex-ssot`, `sql-surface-guard --all` (includes `sqlx` isolation enforcement), `query-all-guard --all`, `turso-import-guard --all`, `operations-verify`, `command-compliance`, `capability-sync` (verify-only), `contracts-index`, **`docs-reality-audit verify`**, `exec-policy-contract`, in-process completion-policy Tier A scan (no audit JSON write), `scientia-worthiness-contract`, `scientia-novelty-ledger-contracts`, and `data-ssot-guards` in one pass |

### Bootstrap / dev launcher (missing `vox` on `PATH`)

When **`vox` is not installed** or not on `PATH`, use the repo launchers so **`cargo run -p vox-cli`** runs from the **workspace root** (Cargo decides incrementally whether to rebuild):

- **Windows (PowerShell):** `pwsh -File scripts/windows/vox-dev.ps1 <vox args…>` — [`scripts/windows/vox-dev.ps1`](../../../scripts/windows/vox-dev.ps1)
- **Linux / macOS / Git Bash:** `./scripts/vox-dev.sh <vox args…>` — [`scripts/vox-dev.sh`](../../../scripts/vox-dev.sh)

| Env | Meaning |
|-----|---------|
| `VOX_REPO_ROOT` | Force workspace root (root `Cargo.toml` must contain `[workspace]`). |
| `VOX_USE_PATH=1` | Prefer **`vox` on `PATH`** when present (default: **`cargo run`** from the clone so the binary matches sources). |
| `VOX_DEV_FEATURES` | Optional comma-separated Cargo features for `vox-cli` (e.g. `coderabbit,gpu`). If unset and an argument equals **`coderabbit`**, the launcher adds **`--features coderabbit`**. |
| `VOX_DEV_QUIET=1` | Pass **`--quiet`** to **`cargo run`**. |

**Full-repo CodeRabbit (build-if-needed + open PRs):** set **`GITHUB_TOKEN`** or **`GH_TOKEN`**, then from the repo root:

```powershell
pwsh -File scripts/windows/vox-dev.ps1 review coderabbit semantic-submit --full-repo --execute
```

```bash
./scripts/vox-dev.sh review coderabbit semantic-submit --full-repo --execute
```

Equivalent one-liner without the script: `cargo run -p vox-cli --features coderabbit -- review coderabbit semantic-submit --full-repo --execute` (plan-only: omit **`--execute`**).
### `vox secrets` (deprecated alias `vox clavis`)

Centralized secret diagnostics and compatibility credential storage.

| Subcommand | Role |
|------------|------|
| `vox secrets status --workflow chat\|mcp\|publish\|review\|db-remote\|mens-mesh --profile dev\|ci\|mobile\|prod --mode auto\|local\|cloud [--bundle minimal-local-dev\|minimal-cloud-dev\|gpu-cloud\|publish-review]` | Prints active-mode blocking vs optional secret readiness using requirement groups and optional bundle checks (alias: `vox secrets doctor …`). |
| `vox secrets login [--vault-url URL] [--vault-token TOKEN] [--account ID] [--backend NAME] [--force] [--non-interactive]` | Same as top-level `vox login` (vault URL/token + profile). |
| `vox secrets set <registry> <token> [--username <name>]` | Stores a registry token in `~/.vox/auth.json` through the Secrets API. |
| `vox secrets get <registry>` | Reads and prints redacted token status from Secrets resolution sources. |
| `vox secrets backend-status` | Prints backend mode (`env_only`/`infisical`/`vault`/`auto`) and backend availability diagnostics. |
| `vox secrets migrate-auth-store` | Migrates plaintext `auth.json` tokens to secure local store and leaves compatibility sentinels in JSON. |

### `vox repo`

Repository discovery from the current directory (`vox repo` with no subcommand defaults to **`status`**) plus explicit multi-repo catalog tools under `.vox/repositories.yaml`. Catalog query commands are **read-only** and treat remote repositories as adapter descriptors unless a later backend is configured.

| Subcommand | Role |
|------------|------|
| `vox repo` · `vox repo status [--json]` | Print discovered root, stable `repository_id`, Git origin when known, capability markers, and Cargo workspace members (compact JSON with `--json` or `VOX_CLI_GLOBAL_JSON=1`). Same JSON as MCP **`vox_repo_status`** ([`repo-workspace-status.schema.json`](../../../contracts/repository/repo-workspace-status.schema.json)). |
| `vox repo catalog list` | Resolve the current repo catalog and print the grouped local/remote descriptors, including local hydration status. |
| `vox repo catalog refresh` | Re-resolve the current repo catalog and write a snapshot cache under `.vox/cache/repos/<repository_id>/repo_catalog_snapshot.json`. |
| `vox repo query text <query> [--repo-id <id> ...] [--regex] [--case-sensitive]` | Search cataloged local repositories and group matches by `repository_id`. |
| `vox repo query file <path> [--repo-id <id> ...]` | Read one file path safely across selected cataloged repositories. |
| `vox repo query history [--repo-id <id> ...] [--path <path>] [--contains <text>]` | Read recent Git history per cataloged local repository. |

### `vox init`

Scaffolds **`Vox.toml`**, **`src/main.vox`**, **`.vox_modules/`**, or a **`<name>.skill.md`** file (same layout as MCP **`vox_project_init`**; success JSON schema [`vox-project-scaffold-result.schema.json`](../../../contracts/repository/vox-project-scaffold-result.schema.json)). Implementation: **`vox-project-scaffold`** crate (shared with **`vox-mcp`**).

### `vox login` (canonical)

Sign in for **VoxDB / Turso** cloud sync: writes URL + token to the OS keyring (`vox-secrets-env` / `turso-url`, `turso-token`) and optional `account_id` + `secrets_backend` to `~/.vox/login.toml`. Same implementation as `vox auth connect`, `vox auth login`, and `vox secrets login`.

| Invocation | Role |
|------------|------|
| `vox login [--vault-url URL] [--vault-token TOKEN] [--account ID] [--backend vox_cloud] [--force] [--non-interactive]` | Prompts or uses flags; `--non-interactive` requires both `--vault-url` and `--vault-token`. |
| `vox logout` | Clears keyring entries and removes `login.toml`. |
| `vox secrets login …` | Alias of `vox login` under the Secrets tree. |
| `vox auth connect …` · `vox auth login …` | Same handler as `vox login` (`Connect` exposes `login` as a Clap alias). |
| `vox auth` | Identity / trust: `vox auth init`, `vox auth unlock`, `vox auth whoami`, `vox auth trust`, `vox auth trust-list`, `vox auth untrust`. |

**Note:** older registry-token flows use `vox secrets set` / `vox secrets get`; vault DB sync uses the commands above.

### `vox commands`

Generate a dynamic command catalog from clap (`VoxCliRoot::command()`), so the list always matches what this binary actually exposes.

Why this exists: it is the discoverability source for first-timers, editor integrations, and docs/CI parity checks.

| Flag | Default | Description |
|------|---------|-------------|
| `--format text\|json` | `text` | Human table output or machine JSON |
| `--recommended` | `false` | Show only first-time starter commands |
| `--include-nested` | `false` | Include nested subcommands (`vox ci …`, `vox mens …`) |
| `--search PATTERN` | — | Fuzzy-search commands by name, alias, or description; implies `--include-nested` |

**Example — search for shell-related commands:**
```bash
vox commands --search shell
vox commands --search shell --format json
```

### `vox dev <file>`

Watch mode: spawns **`vox-compilerd`** (JSON lines on stdio; one `DispatchRequest` per process), sends a `dev` request with `file`, `out_dir`, optional **`target`** (build profile), `port`, and `open`, then streams daemon output until exit or Ctrl+C. Resolve the daemon the same way as other compilerd tools: sibling to the `vox` executable, then `PATH`.

Build the daemon from this repo: `cargo build -p vox-cli --bin vox-compilerd` → `target/debug/vox-compilerd(.exe)` (install next to `vox` or add to `PATH`).

| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--out-dir` | `dist` | Build artifact directory |
| `--target` | _(from `Vox.toml` / env)_ | `fullstack`, `server`, or `client` — same as `vox build --target` |
| `--port` | `3000` | Dev server port (when applicable) |
| `--open` | `false` | Open browser when the daemon reports a URL (`--target=server` suppresses browser open) |

### `vox emit client <file>`

Emits the **client** (Library) artifact set only: `vox-client.ts`, `openapi.json`, types/schemas, and a **`private`** root **`package.json`** (exports map + `peerDependencies.zod`) when any library export exists — same as **`vox build --target=client`** without running the fullstack Rust path.

| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--out-dir` | `dist` | Output directory |
| `--mobile-target` | — | Same as `vox build --mobile-target` when needed |
| `--emit-ir` | `false` | Write `web-ir.v1.json` into the output dir |

### `vox emit openapi <file>`

Emits a **standalone OpenAPI 3.1 JSON spec** from the `query`/`mutation`/`server` endpoints in a single Vox source file — no TypeScript, no npm directory. Conforms to the [wire-format-v1 SSOT](../architecture/wire-format-v1-ssot.md):
- `Decimal` / `BigInt` → `type: string`
- `DateTime` → `type: string, format: date-time`
- `Option<T>` → property absent from `required`
- Sum types → `oneOf` + shared `_tag` discriminant

| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--out` | `openapi.json` | Output file path |
| `--package-name` | `vox-api` | OpenAPI `info.title` |
| `--package-version` | `0.1.0` | OpenAPI `info.version` |

The output is byte-identical for identical inputs (canonical JSON, `BTreeMap`-sorted keys). Integrate with `openapi-typescript`, Orval, RTK Query, Postman, or any OpenAPI 3.1 consumer.

### `vox live`

Terminal dashboard subscribed to an in-process `vox-orchestrator` event bus (demo / local use). **Not in default builds:** `cargo build -p vox-cli --features live` then run `vox live`.

Set **`VOX_ORCHESTRATOR_EVENT_LOG`** to a file path to tail the same JSONL stream **`vox-mcp`** appends when that variable is set (shared runtime view across MCP and CLI).

### `vox compile [file]`

Product-facing **native compile** umbrella: combines codegen, optional `[bundle]` / `[workspace]` hints from `Vox.toml`, Tauri packaging **hints** (`target/generated/tauri-packaging/`), optional release archives (`.tar.gz` / `.zip` + checksum sidecar), and delegates script/WASI/server flows where noted.

| Flag | Default | Description |
|------|---------|-------------|
| `--target` | `native-binary` | `native-binary` (bundle-style binary), `desktop`, `mobile-android`, `mobile-ios`, `script`, `wasi`; `server` prints delegation hint to deploy/OCI |
| `--triple` | _(host)_ | Rust target triple for cross-compilation |
| `--out-dir`, `-o` | `dist` | Frontend / codegen output directory (same family as `build`) |
| `--workspace` | `false` | Build every `[workspace].members` entry from repo-root `Vox.toml` |
| `--release` | `true` | Release vs debug backend build |
| `--archive` | `false` | Emit `.zip` or `.tar.gz` plus `checksums-compile.txt` beside the binary |

Lower-level **`vox bundle <file>`** remains the Axum + embedded frontend path; **`vox compile`** is the stable UX entry point for multi-target packaging.

**Invocation hint:** put flags (`--target`, `--triple`, …) **before** the optional `FILE` positional — e.g. `vox compile --target desktop src/main.vox`.

### `vox bundle <file>`

End-to-end **shipping** flow: build → scaffold `dist/app` (Vite + React) → **`pnpm install`** + **`pnpm run build`** → copy static assets → `cargo build` on the backend → copy the resulting binary into `dist/<stem>` (plus `.exe` on Windows when applicable).

| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--out-dir` | `dist` | TS/frontend codegen output (same as `build`) |
| `--target` | _(host)_ | Optional Rust target triple for cross-compile (`rustup target add` attempted) |
| `--release` | `true` | Release vs debug backend build |

If no TSX components are detected after build, stops after codegen (“backend-only”).

### `vox migrate web`

Automated codemod runner for migrating legacy web concepts into standardized Path C React syntax.
`vox migrate web --apply` rewrites `.vox` files in place to remove legacy tags such as `@component` and updates them to standard block properties.

## Quality

### `vox check <file>`

Lex, parse, and type-check only. Prints diagnostics to stderr; exits with error if any **error**-severity diagnostic exists.

- **`--for-llm`**: print one JSON envelope to stdout (`CheckForLlmEnvelope` — structured diagnostics via `vox_compiler::pipeline::check_file`) and exit non-zero if any error-level diagnostic exists. See [`vox-diagnostic-ux-ssot-2026.md`](../architecture/vox-diagnostic-ux-ssot-2026.md#cli-envelope-vox-check---for-llm).
- `--emit-training-jsonl <PATH>`: append successful frontend records to JSONL for training corpus generation.

### `vox test <file>`

Runs `build`, then **`cargo test`** in `target/generated`.

### `vox fmt <file>`

Formats a **`.vox`** file using [`vox_compiler::fmt::try_format`](../../../crates/vox-compiler/src/fmt/mod.rs): parse → pretty-print → **re-parse** (fail-closed). Writes in place via a temp file + rename (see [`commands/fmt.rs`](../../../crates/vox-cli/src/commands/fmt.rs)). **`--check`**: exit non-zero if the file would change (CI-friendly). Constructs the formatter cannot print yet surface as **parse** errors once the printer/AST diverges; expand coverage in `vox-compiler` `fmt/` over time.

### `vox doctor`

**Canonical path (English):** `vox doctor …` — this is the primary spelling in docs, scripts, and muscle memory.

**Grouped Latin path:** `vox diag doctor …` — identical behavior; `diag` is the **registry `latin_ns`** bucket for diagnostics (see [Nomenclature migration map](../archive/research-2026-q1/nomenclature-migration-map.md#latin_ns-command-registry-group-labels)). Prefer `vox doctor` in new prose; use `vox diag doctor` when teaching the Latin lane.

Development environment checks (Rust/Cargo, Node/pnpm, Git, optional Docker/Podman, `Vox.toml`, Codex workspace registration, API keys, etc.). With **`VOX_WEB_TS_OUT`** set to your **`vox build`** TypeScript output directory, doctor also verifies **`@v0`** components use **named** exports for TanStack **`routes {`** (see [`env-vars.md`](env-vars.md#web--vite--tanstack-codegen)).

**Cross-compile preflight:** **`--compile-target <TRIPLE>`** runs rustup-target / Android SDK / Xcode probes aligned with **`vox compile --triple`** (e.g. `aarch64-linux-android`, `aarch64-apple-ios-sim`).

| Build | Flags |
|-------|--------|
| **Default** | `--auto-heal`, `--test-health`, **`--probe`** (OCI healthcheck: exit non-zero if any default check fails; no banner) |
| **`--features codex`** | Also `--build-perf`, `--scope`, `--json` (extended doctor in `commands::diagnostics::doctor`) |

Build: `cargo build -p vox-cli --features codex` for the extended path.

## Tooling

### `vox db`

Local **VoxDB** inspection and research helpers (`crates/vox-cli/src/commands/db.rs`, `db_cli.rs`). Uses the same connection resolution as Codex (`VOX_DB_*`, then Turso compatibility aliases per env-vars SSOT, or local path).

`vox db audit` prints read-only JSON to stdout: schema version, database paths, select storage `PRAGMA`s, and per-user-table row counts. Add `--timestamps` for heuristic `MIN`/`MAX` on a chosen time-like column per table (extra queries).

`vox db prune-plan` prints JSON counts for rows that match automated rules in [`contracts/db/retention-policy.yaml`](../../../contracts/db/retention-policy.yaml) (`days`, `ms_days`, `expires_lt_now`). `vox db prune-apply --i-understand` runs the matching `DELETE`s. Rationale, sensitivity classes, and table notes (including `ci_completion_*`) live in [telemetry-retention-sensitivity-ssot](../archive/research-2026-q1/telemetry-retention-sensitivity-ssot.md).

Common subcommands include `status`, `audit`, `schema`, `sample`, `migrate`, `introspect`, `verify`, `export` / `import`, `vacuum`, `build-health`, `build-regressions`, and `pref-get` / `pref-set` / `pref-list`. `vox db migrate --url <sql-url>` uses `vox-sql`'s app-plane `AppAutoMigrator` (`--dry-run` plans only), while `vox db introspect` and `vox db verify` resolve app-plane connectivity from `--url`, `VOX_APP_DB_URL`, then `VOX_DB_URL`. Publication operator controls: `publication-discovery-scan`, `publication-discovery-explain`, `publication-transform-preview`, `publication-route-simulate`, `publication-publish`, and `publication-retry-failed` accept **`--json`** for structured stdout. **`publication-publish`** enforces the same live gate as other surfaces when `--dry-run` is off: VoxDb with two digest approvers and `VOX_NEWS_PUBLISH_ARMED=1` (or orchestrator publish_armed is not read by this path); successful live runs update manifest state to `published` / `publish_failed` like MCP/orchestrator. Run `vox db --help` for the full tree.

Discovery/data-prep operator commands: `vox db publication-discovery-scan`, `vox db publication-discovery-explain`, `vox db publication-transform-preview`, and `vox db publication-discovery-refresh-evidence`. **`publication-discovery-explain` JSON** adds assist-only `impact_readership_projection` (not a publish gate) when `scientia_novelty_bundle` is present on the manifest. **Prior-art / worthiness operator JSON:** `vox db publication-novelty-fetch` (federated OpenAlex/Crossref/Semantic Scholar bundle; optional `--persist-metadata`; query limits/tunables from `contracts/scientia/impact-readership-projection.seed.v1.yaml`), `vox db publication-decision-explain` (Socrates/sidecar enrich + heuristic preflight + worthiness + discovery rank; optional `--live-prior-art`; includes the same assist-only projection when a novelty bundle is available), and `vox db publication-novelty-happy-path` (prior art + enrich + stdout: finding-candidate + bundle + merged rank + worthiness + `calibration_telemetry` + assist-only `impact_readership_projection`).

`vox db mirror-search-corpus` mirrors markdown into the Codex search corpus (delegates to the same implementation as `vox scientia mirror-search-corpus`).

### `vox memory`

**Hybrid retrieval** — runs the shared `vox-search` bundle against repository-discovered roots, default orchestrator memory paths (`.vox/memory/...`), and an optional local VoxDb (`VOX_DB_*`). **`vox memory search <words…>`** prints structured retrieval diagnostics JSON (policy version, backends used, evidence quality). Catalog: [`memory.search`](../../../contracts/operations/catalog.v1.yaml) (`commands::memory_cli`).

### `vox telemetry`

**Optional operator upload path** — not default-on, not product telemetry. Local JSON spool under `.vox/telemetry-upload-queue` (or `VOX_TELEMETRY_SPOOL_DIR`), explicit **`vox telemetry upload`**, secrets via Clavis (`VOX_TELEMETRY_UPLOAD_URL`, `VOX_TELEMETRY_UPLOAD_TOKEN`). Subcommands: **`vox telemetry status`**, **`vox telemetry export`**, **`vox telemetry enqueue --json <file>`**, **`vox telemetry upload`** (`--dry-run` supported). See [ADR 023](../adr/023-optional-telemetry-remote-upload.md), [telemetry remote sink spec](../archive/research-2026-q1/telemetry-remote-sink-spec.md), [env-vars](env-vars.md#optional-telemetry-upload-vox-telemetry).

### `vox scientia`

**Typing / ergonomics:** Publication subcommands are **long on purpose**—they are stable for scripting and match [`command-registry.yaml`](../../../contracts/cli/command-registry.yaml) / `vox ci command-compliance`. Mitigations { **`vox completions <shell>`** (tab-complete partial subcommand paths); repeat operators may use shell aliases or wrappers. There is no separate Latin umbrella for `scientia` today; use English **`vox scientia …`** only.

**Vox Scientia** — facade over Codex research and publication workflows.

- Research/capability helpers: `capability-list`, `research-list`, `research-map-list`, `retrieval-status`, `research-refresh`, `vox scientia finding-candidate-validate --json <path>`, `vox scientia novelty-evidence-bundle-validate --json <path>`, and `vox scientia mirror-search-corpus` (same behavior as `vox db mirror-search-corpus`).
- Scientific publication lifecycle:
  - `vox scientia publication-discovery-scan --publication-id <id> [--max-items <n>] [--source <name>] [--dry-run] [--json]` (run publication discovery enrichment and queue candidate evidence before downstream readiness/submit flows)
  - `vox scientia publication-discovery-explain --publication-id <id> [--max-items <n>] [--json]` (inspect discovery scoring/ranking evidence for a publication without mutating submission state)
  - `vox scientia publication-novelty-fetch --publication-id <id> [--persist-metadata] [--offline] [--json]` (prior-art bundle; mirrors `vox db publication-novelty-fetch`)
  - `vox scientia publication-decision-explain --publication-id <id> [--json]` (preflight + worthiness + discovery rank; mirrors `vox db publication-decision-explain`)
  - `vox scientia publication-novelty-happy-path --publication-id <id> [--offline] [--json]` (candidate + bundle + rank + worthiness + calibration snapshot; mirrors `vox db publication-novelty-happy-path`)
  - `vox scientia publication-transform-preview --publication-id <id> [--channel <name>] [--json]` (render a dry-run preview of channel-specific transformed copy prior to live publish)
  - `vox scientia collection-transform-preview --collection-id <id> [--channel <name>] [--json]` (preview transformed channel output for collection-level syndication before publish orchestration)
  - `vox scientia publication-prepare --publication-id <id> --author <name> [--title <title>] [--scholarly-metadata-json <file>] [--eval-gate-report-json <file>] [--benchmark-pair-report-json <file>] [--human-meaningful-advance] [--human-ai-disclosure-complete] [--preflight] [--preflight-profile default|double-blind] <path.md>` (title defaults from markdown frontmatter/first heading; structured evidence seeds `metadata_json.scientia_evidence` with discovery signals and draft-prep hints)
  - `vox scientia publication-prepare-validated` (same flags as prepare except preflight is always on)
  - `vox scientia publication-preflight --publication-id <id> [--profile default|double-blind] [--with-worthiness]` (returns readiness findings plus `manual_required` and ordered `next_actions`)
  - `vox scientia publication-zenodo-metadata --publication-id <id>` (stdout JSON for Zenodo deposit metadata; no HTTP)
  - `vox scientia publication-openreview-profile --publication-id <id>` (stdout JSON: merged OpenReview invitation/signature/readers + API base; no HTTP)
  - `vox scientia publication-worthiness-evaluate [--contract-yaml <path>] --metrics-json <path>` (stdout worthiness decision JSON from repo contract + metrics file; no DB)
  - `vox scientia publication-approve --publication-id <id> --approver <identity>`
  - `vox scientia publication-submit-local --publication-id <id>`
  - `vox scientia publication-status --publication-id <id> [--with-worthiness]` (includes the embedded default preflight report so status doubles as the operator checklist surface; `--with-worthiness` adds the worthiness rubric to that same report)
  - `vox scientia publication-scholarly-remote-status --publication-id <id> [--external-submission-id <id>]` (poll remote scholarly repository / deposit state for a stored submission)
  - `vox scientia publication-scholarly-remote-status-sync-all --publication-id <id>` (poll remote status for every `scholarly_submissions` row on that publication)
  - `vox scientia publication-scholarly-remote-status-sync-batch [--limit <n>] [--iterations <n>] [--interval-secs <s>] [--max-runtime-secs <s>] [--jitter-secs <s>]` (batch sync across publications ranked by recent submission activity; optional bounded loop for supervised workers)
  - `vox scientia publication-scholarly-staging-export --publication-id <id> --output-dir <dir> --venue zenodo|open-review|arxiv-assist` (write venue-scoped scholarly staging artifacts under `output-dir` and validate layout; Zenodo adds `zenodo.json`, arXiv assist adds `arxiv_handoff.json`, **`main.tex`** stub, and `arxiv_bundle.tar.gz`; mirrors `vox db publication-scholarly-staging-export`)
  - `vox scientia publication-scholarly-pipeline-run --publication-id <id> [--preflight-profile default|double-blind|metadata-complete] [--dry-run] [--staging-output-dir <dir> --venue zenodo|open-review|arxiv-assist] [--adapter <kind>] [--json]` (default scholarly happy path: preflight → dual-approval gate → optional staging export → scholarly submit unless `--dry-run`; `--json` = compact single-line JSON on stdout; mirrors `vox db publication-scholarly-pipeline-run`)
  - `vox scientia publication-arxiv-handoff-record --publication-id <id> --stage <staging-exported|…|published> [--operator <id>] [--note <text>] [--arxiv-id <id>]` (append-only operator milestone for arXiv assist; `published` requires `--arxiv-id`)
  - `vox scientia publication-external-jobs-due [--limit <n>]` (list external submission jobs due for retry/tick)
  - `vox scientia publication-external-jobs-dead-letter [--limit <n>]` (list terminal `failed` external submission jobs)
  - `vox scientia publication-external-jobs-replay --job-id <id>` (requeue one dead-letter job to `queued`)
  - `vox scientia publication-external-jobs-tick [--limit <n>] [--lock-ttl-ms <ms>] [--lock-owner <id>] [--iterations <n>] [--interval-secs <s>] [--max-runtime-secs <s>] [--jitter-secs <s>]` (advance external submission worker queue; optional repeated ticks)
  - `vox scientia publication-external-pipeline-metrics [--since-hours <h>]` (read-only JSON rollup: jobs, attempts, snapshots, scholarly rows, `publication_attempts` by channel; mirrors `vox db publication-external-pipeline-metrics`)

Connection resolution matches `vox db` (`VOX_DB_*`, …). The publication flow uses digest-bound dual approvals before scholarly submission.
For architecture/lingo and multi-platform routing internals, see `docs/src/architecture/voxgiantia-publication-architecture.md`.

### `vox shell`

PowerShell-first guardrails for autonomous IDE terminals (see [`AGENTS.md`](../../../AGENTS.md)): prefer **`pwsh`** on every host where it is installed. **CI** workflows may still use **bash** on Linux runners ([`docs/src/ci/runner-contract.md`](../ci/runner-contract.md)); that does not change the local/agent shell doctrine.

**Boundaries:** Vox does **not** ship a shell emulator product. See [Vox shell operations boundaries](../archive/research-2026-q1/vox-shell-operations-boundaries.md).

**Which surface to use**

| Situation | Surface |
|-----------|---------|
| Pasting/running commands in a real terminal | Host **`pwsh`** (or workflow shell); validate risky PowerShell with **`vox shell check`**. |
| Quick manual poke at `vox` without spawning `pwsh` | **`vox shell repl`** only (built-ins + optional naive passthrough; see below). |
| File/process logic in `.vox` source | **`std.fs` / `std.path` / `std.process`** (argv-first), not parsed shell strings. |

- `vox shell repl` — **dev-only micro-REPL**: built-in **`pwd` / `ls` / `cat`** (Rust; not PowerShell). Unknown lines are forwarded with **`split_whitespace` → OS spawn** (no quotes, pipes, redirection, or session `cd`). The first passthrough prints a **stderr note** describing those limits. Prefer **`pwsh`** for real shell work. Bare `vox shell` defaults to `repl`.
- `vox shell check --payload "<ps>"` — runs `Parser::ParseInput` via `contracts/terminal/pwsh_extract_command_asts.ps1` and enforces [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml). Optional `--policy <path>` overrides the default policy file.

**Compact PowerShell lexicon** (host terminal / `vox shell check` allowlist; not the `repl`):

| Intent | Cmdlet(s) |
|--------|-----------|
| Where am I? | `Get-Location` (`pwd`) |
| List entries | `Get-ChildItem` (`dir`, `ls`) |
| Read text file | `Get-Content -Raw` |
| Join / split path | `Join-Path`, `Split-Path` |
| Exists / canonical path | `Test-Path`, `Resolve-Path` |
| Filter / project | `Where-Object`, `Select-Object`, `ForEach-Object` |
| Emit / format text | `Write-Output`, `Write-Host`, `Out-String` |
| Structured data | `ConvertTo-Json`, `ConvertFrom-Json` (when allowlisted) |
| Approved externals | `vox`, `cargo`, `rustc`, `git`, `pwsh`, `powershell` (see policy YAML) |

Optional IDE wiring: [`.vscode/settings.json`](../../../.vscode/settings.json) adds terminal profiles **Vox Exec policy (PSReadLine)** (profile scripts live alongside editor config — see [`terminal exec policy SSOT`](../architecture/terminal-exec-policy-ssot.md)) and **Vox pwsh proxy (check only)** ([`.vox/bin/vox-pwsh-proxy.cmd`](../../../.vox/bin/vox-pwsh-proxy.cmd) — set `VOX_SHELL_CHECK_PAYLOAD` to the line to validate). See also [`terminal-ast-validation-research-2026.md`](../archive/research-2026-q1/terminal-ast-validation-research-2026.md).

### `vox codex`

**Codex** (Turso / Arca) utilities backed by `vox-db`.

`vox codex cutover` automates legacy-chain migration: exports JSONL + a JSON sidecar, creates a new local SQLite file at `--target-db`, imports, and prints the `VOX_DB_PATH` you should export next. Requires a **local** legacy file (`--source-db` or configured `VOX_DB_PATH`). Use `--force` only after backing up an existing target path.

| Subcommand | Description |
|------------|-------------|
| `verify` | Prints `schema_version` (baseline **1**), manifest-derived reactivity table check, and legacy-chain flag |
| `export-legacy -o <file>` | Writes JSONL for legacy table set (see `vox_db::codex_legacy::LEGACY_EXPORT_TABLES`) |
| `import-legacy -i <file>` | Restores rows from that JSONL (clears allowlisted tables on the target, then inserts; for fresh baselines only) |
| `cutover --target-db <new.db> [--source-db <old.db>] [--artifact-dir <dir>] [--force]` | Export + fresh target + import + `codex-cutover-*.{jsonl,sidecar.json}` artifacts |
| `import-orchestrator-memory --dir <dir> --agent-id <id> [--session-id <s>]` | One `memories` row per top-level `*.md` |
| `import-skill-bundle --file <bundle.json>` | JSON `{ id, version, manifest_json, skill_md }` → `skill_manifests` |
| `socrates-metrics [--repository-id <id>] [--limit N]` | Prints `SocratesSurfaceAggregate` JSON from recent `socrates_surface` `research_metrics` rows |
| `socrates-eval-snapshot --eval-id <id> [--repository-id <id>] [--limit N]` | Writes one `eval_runs` row via `VoxDb::record_socrates_eval_summary` (errors if no `socrates_surface` rows in window) |

Connection uses `DbConfig::resolve_standalone()` (`VOX_DB_*`, then Turso compatibility aliases per env-vars SSOT, or local path).

Always available in the minimal binary. **`vox snippet`** — `save`, `search`, and `export` use the local Codex database (`VOX_DB_URL` / `VOX_DB_TOKEN` or `.vox/store.db`). **`vox share`** — `publish`, `search`, `list`, `review` against the same index.

### `vox skill` (feature `ars`)

**Not in default builds.** `cargo build -p vox-cli --features ars`. Subcommands mirror the ARS helpers: `list`, `install`, `uninstall`, `search`, `info`, `create`, `eval-task`, `promote`, `run`, `context-assemble`, `discover` (see `commands::extras::ars`).

### `vox plugin install`

Installs a plugin from the catalog (`<id>`), a local directory (`--path <DIR>`), or a direct HTTPS URL (`--url <URL>`). The fetched archive is verified against a SHA-256 before extraction — it is `dlopen`'d as native code afterward, so an unverified download is arbitrary code execution. Catalog `github:` sources must pin a `version` and record a `sha256` in `catalog.toml`; an unpinned `latest` asset is refused before any network call, because the bytes behind a floating release change and no recorded hash could ever match them.

| Flag | Description |
|------|-------------|
| `--path <DIR>` | Install from a local directory containing `Plugin.toml`. No checksum (already-trusted workspace source). |
| `--url <URL>` | Install from an HTTPS URL pointing to a `.zip` archive. |
| `--yes` | Skip the confirmation prompt. |
| `--allow-unverified` | Install even when no `sha256` is recorded for the plugin. The archive is loaded as native code — only use this for a source you trust. |

Without `--allow-unverified`, an install with no matching `sha256` fails closed: `refusing to install …: no sha256 recorded for this plugin`. **`vox bundle apply`** (`vox plugin-bundle apply`) always passes `false` for this flag, so it refuses `github:` plugins until hashes are recorded in the catalog.

### `vox gamify` (alias `vox ludus`, feature `extras-ludus`)

**Not in default builds.** `cargo build -p vox-cli --features extras-ludus`. Companions, quests, shop, arena, collegium, etc. (`commands::extras::ludus`). Canonical command is **`vox gamify`**; the Latin **`vox ludus`** alias keeps working. Terminal HUD: **`vox gamify hud`** (alias `vox ludus hud`) requires **`--features ludus-hud`** (implies `extras-ludus` + `vox-orchestrator`).

### `vox stub-check` (feature `stub-check`)

**Not in default builds.** `cargo build -p vox-cli --features stub-check`. Runs **TOESTUB** (`vox-toestub`) over a directory tree, with optional **Codex** persistence (baselines, task queue, suppressions) and **Ludus** rewards on a clean run (`vox-gamify`).

| Argument / flag | Description |
|-----------------|-------------|
| `[PATH]` | Positional scan root (default `.` if omitted) |
| `-p`, `--path <PATH>` | Same as positional; mutually exclusive with `[PATH]` |
| `-f`, `--format <FMT>` | Output format (e.g. `terminal`, `json`, `markdown`) |
| `-s`, `--severity <LVL>` | Minimum severity: `info`, `warning`, `error`, `critical` |
| `--suggest-fixes` | Emit fix suggestions / task queue (default `true`) |
| `--rules <LIST>` | Comma-separated rule id prefixes |
| `--excludes <PATH>` | Repeatable exclude globs/paths |
| `--langs <LIST>` | Comma-separated languages (`rust`, `ts`, …) |
| `--baseline <NAME or FILE>` | Named baseline in VoxDB or path to a JSON file |
| `--save-baseline <NAME>` | Store current findings as a named baseline |
| `--task-list` | Print last saved task queue from VoxDB and exit |
| `--import-suppressions` | Import `toestub.toml` suppressions into VoxDB |
| `--ingest-findings <FILE>` | Ingest findings JSON into VoxDB task queue |
| `--fix-pipeline` / `--fix-pipeline-apply` | Staged doc/unwired fixes (apply = write) |
| `--gate <MODE>` / `--gate-budget-path <PATH>` | CI warning budget / ratchet |
| `--verify-impacted`, `--max-escalation`, `--self-heal-safe-mode` | Reserved / advanced hooks |

**CI / parity:** prefer **`vox ci toestub-scoped`** (default scan root `crates/vox-repository`) — same policy surface as GitHub Actions. Use **`vox stub-check …`** for interactive or repo-wide scans when you need clap flags (format, baselines, Ludus, etc.). The standalone **`toestub`** crate binary remains available for advanced tooling.

**`toestub` binary (crate `vox-toestub`):** besides `--mode`, `--format`, `--canary-crates`, and `--suppressions`, the rollout surface includes **`--tests-mode`** (`off` \| `include` \| `strict`, default `off` — skips noisy unresolved-ref under `.../tests/...` when `off`), **`--prelude-allowlist`** (JSON per `contracts/toestub/prelude-allowlist.v1.json`), and **`--feature-flags`** (comma-separated, e.g. `unwired-graph`, `scaling-fs-heuristic-fallback`).

### `vox architect` (features `stub-check` or `codex`)

**Not in default builds.** Requires `cargo build -p vox-cli --features stub-check` and/or `--features codex` (same feature gates as `commands::diagnostics`). Subcommands: **`check`** (workspace layout vs `vox-schema.json`), **`fix-sprawl`** (`--apply` to move misplaced crates), **`analyze`** (optional path, default `.` — god-object scan via TOESTUB; **needs `--features stub-check`**; with `codex` only, the command is available but **`analyze` exits with a hint to add `stub-check`**). Implementation: `crates/vox-cli/src/commands/diagnostics/tools/architect.rs`.

### `vox openclaw` (feature `ars`)

**Not in default builds.** Build with `cargo build -p vox-cli --features ars`, then run `vox openclaw` (alias `oc`). Vox resolves endpoints from explicit flags, env/Clavis, and upstream discovery (`/.well-known/openclaw.json`) with cache fallback. Subcommands include `import`, `list-remote`, `vox openclaw search-remote <query>`, `config` (prints resolved HTTP/WS/catalog/discovery source), `vox openclaw doctor` (health + optional sidecar autostart), MCP-backed `approvals` / `approve` / `deny`, WS-backed `subscribe` / `unsubscribe` / `subscriptions` / `notify` (JSON-capable), and `vox openclaw gateway-call --method <name> --params-json '{...}'` for direct WS method invocation. Sidecar lifecycle is also exposed via `vox openclaw sidecar status`, `vox openclaw sidecar start`, and `vox openclaw sidecar stop` (state-backed PID lifecycle). `serve` expects a `vox-gateway` binary on `PATH`. SSOT: [`openclaw-discovery-sidecar-ssot.md`](openclaw-discovery-sidecar-ssot.md).

### `vox lsp`

Spawns the **`vox-lsp`** binary (from the `vox-lsp` crate) with stdio inherited. Ensure `vox-lsp` is on `PATH` (e.g. `cargo build -p vox-lsp` and use `target/debug`).

## Mens / DeI (feature-gated)

**Normative semantics** (defaults, train / merge / serve matrix, data-prep SSOT, deferred trainer flags): **[`reference/mens-training.md`](mens-training.md)**. This section lists **CLI surfaces and build features** only; do not treat it as a second SSOT for training behavior.

**Doc parity (`vox ci command-compliance`):** **`vox mens corpus`**, **`vox mens pipeline`**, **`vox mens status`**, **`vox mens watch-telemetry`** (alias **`vox mens watch`**; tails stderr + training JSONL ~3s), **`vox mens plan`**, **`vox mens eval-gate`**, **`vox mens bench-completion`**, **`vox mens system-prompt-template`**, **`vox mens train`** (GPU / Candle QLoRA; same intent as **`vox-ml-cli` shim** (`vox mens …`)), **`vox oratio`**, **`vox mens serve`**, **`vox mens probe`**, **`vox mens merge-weights`**, **`vox mens merge-qlora`**, **`vox mens eval-local`**, **`vox mens generate`**, **`vox mens review`**, **`vox mens check`**, **`vox mens fix`**, **`vox mens workflow list`**, **`vox mens workflow inspect`**, **`vox mens workflow check`**, **`vox mens workflow run`**.

With default features (**`mens-base` only** — corpus + `vox-actor-runtime`, **no** Oratio / `vox-oratio` and **no** native training deps), **`vox mens`** covers corpus / pipeline / status / plan / eval-gate / bench-completion / system templates / etc. **`vox oratio`** (alias **`vox speech`**) requires **`--features oratio`** (STT stack; separate from the **`mens`** command tree). **Native train** / **serve** / **probe** / **merge-weights** / **merge-qlora** / **eval-local** (Burn + Candle) require **`cargo build -p vox-cli --features gpu`** (alias **`mens-qlora`**). For **Candle QLoRA on NVIDIA** with linked CUDA kernels, use **`cargo vox-cuda-release`** (workspace alias → `gpu,mens-candle-cuda`; see `.cargo/config.toml`). Optional: **`vox-ml-cli`** shim binary inserts the **`mens`** subcommand for argv ergonomics — use **`vox oratio`** for speech. `cargo build -p vox-cli --features mens-base`; add **`oratio`** on the same build for Oratio. See [vox-cli build feature inventory](../archive/research-2026-q1/vox-cli-build-feature-inventory.md). **`vox mens pipeline`** runs the dogfood corpus → eval → optional native train stages (replaces heavy orchestration in `scripts/run_mens_pipeline.ps1`). **`vox mens serve`** (HTTP/OpenAI-compatible API) requires **`gpu`** (Axum/control-plane pieces may additionally need **`execution-api`** for other REST surfaces — see `crates/vox-cli/Cargo.toml`). **`serve`** loads **Burn** LoRA `*.bin` or merged **`model_merged.bin`** (`merge-weights`); it does **not** load Candle **`merge-qlora`** f32 safetensor outputs. Corpus lives under **`vox mens corpus`** (e.g. `extract`, `validate`, `pairs`, **`mix`**, `eval`).

- **`vox mens train`** — native Mens training (contract/planner inside **`vox-populi`** (`mens::tensor`); use **`vox-ml-cli`** argv shim when you want the binary that inserts `mens`). **`--backend lora`** (default): Burn + wgpu LoRA; **`--tokenizer vox`** (default) or **`--tokenizer hf`** with **GPT-2-shaped** HF `config.json` + optional **HF embed warm-start** from safetensors. **`--backend qlora`**: Candle + **qlora-rs** — **NF4 frozen base** linear(s) + trainable LoRA; **mmap `f32`** for context embeddings (`wte` / `model.embed_tokens`). When all per-layer **output-projection** weights exist in shards, trains a **sequential stack** + LM head; else **LM-head-only**. **`--qlora-no-double-quant`** turns off qlora-rs **double quant** of scales (default: on). **`--qlora-require-full-proxy-stack`** fails preflight if expected middle projection keys are missing from shards (strict prod gate). **`--qlora-lm-head-only`** skips the middle `o_proj` stack even when shards are complete (stable CE on some CUDA dogfood paths; conflicts with **`--qlora-require-full-proxy-stack`**). **`--qlora-proxy-max-layers N`** caps stacked middle projections for ablation (`0` = LM-head-only; conflicts with **`--qlora-lm-head-only`** when `N > 0`). **`--qlora-ce-last-k K`** (default **64**, contract field `qlora_ce_last_k` in **`crates/vox-populi/src/mens/tensor/training_config.rs`**; CLI wiring in **`vox-ml-cli`** `src/commands/mens/populi/dispatch.rs`) applies next-token CE on the last **K** positions per JSONL row (bounded by **`seq_len`** and **64**). In-tree **qlora-rs** `training_step_lm`: pre-norm residual middles with **`1/√depth`** per block and again before the LM head. **`--qlora-max-skip-rate <0..=1>`** aborts training when skipped JSONL rows exceed the fraction per epoch. **`--log-dir DIR`** re-spawns training in the background with a timestamped log (parent returns immediately — avoids IDE/agent wall-clock timeouts; tail the log). **`--background`** lowers process priority and caps VRAM fraction for long runs. Same **`--device`** story; **CUDA** / **Metal** with **`mens-candle-cuda`** / **`mens-candle-metal`**. QLoRA needs **`--tokenizer hf`**, **`--model`**, HF safetensors + **`tokenizer.json`**. **`--deployment-target mobile_edge`** or **`--preset mobile_edge`**: planner gates for edge export + **`--device cpu`** required. See [`reference/mens-training.md`](mens-training.md), [`reference/mobile-edge-ai.md`](mobile-edge-ai.md), [`hf-finetune-capability-matrix.md`](../archive/research-2026-q1/hf-finetune-capability-matrix.md). Python QLoRA: **`vox train`** / `train_qlora.vox` with **`--features mens-dei`**.
- **`vox mens merge-weights`** — merges a **Burn** LoRA checkpoint (`*.bin`) into **`model_merged.bin`** (`gpu` only). Does **not** apply Candle qlora adapter tensors.
- **`vox mens merge-qlora`** (alias **`merge-adapter`**) — merges **`candle_qlora_adapter.safetensors`** + sidecar meta (**v2** `candle_qlora_adapter_meta.json` or **v3** `populi_adapter_manifest_v3.json`) into **f32** base shards (subset); **`*.bin`** Burn checkpoints are **rejected** (use **`merge-weights`**). See SSOT merge table.
- **`vox oratio`** (alias **`vox speech`**) — transcribe via **`vox-oratio`** (**Candle Whisper**, Rust + HF weights; not whisper.cpp). Build CLI with **`--features oratio`**. Includes `transcribe`, `status`, and sessionized `listen` (Enter-or-timeout gate, correction profile, route mode). Optional **`record-transcribe`** (default microphone → WAV → STT) needs **`--features oratio-mic`**. Env: `VOX_ORATIO_MODEL`, `VOX_ORATIO_REVISION`, `VOX_ORATIO_LANGUAGE`, etc. HTTP ingress: **`cargo run -p vox-audio-ingress`** (**`GET /api/audio/status`**, **`POST /api/audio/transcribe`** JSON `{"path":"…"}`, **`POST /api/audio/transcribe/upload`** multipart); relative paths use `VOX_ORATIO_WORKSPACE` or CWD. Bind with **`VOX_DASH_HOST`** / **`VOX_DASH_PORT`** (default `127.0.0.1:3847`). See [`speech-capture-architecture.md`](speech-capture-architecture.md). **VS Code / Cursor** Oratio flows: [`apps/editor/vox-vscode/README.md`](../../../apps/editor/vox-vscode/README.md) (MCP via `vox mcp`).
- **Vox source (`Speech.transcribe`)** — builtin module **`Speech`**: **`Speech.transcribe(path: str) → Result[str]`** uses Oratio and returns **refined** text (`display_text()`). Generated Rust crates depend on **`vox-oratio`** via codegen `Cargo.toml`.
- **Corpus mix `asr_refine`** — in mix YAML, set `record_format: asr_refine` on a source whose JSONL lines match **`mens/schemas/asr_refine_pairs.schema.json`** (`noisy_text` / `corrected_text`); output lines are **`prompt`/`response`** JSON for `train.jsonl`.
- **Corpus mix `tool_trace`** — set `record_format { tool_trace` for JSONL lines shaped like **`ToolTraceRecord`** in `vox-corpus` (`task_prompt`, `tool_name`, `arguments_json`, `result_json`, `success`, optional `followup_text`); schema **`mens/schemas/tool_trace_record.schema.json`**, example lines **`mens/data/tool_traces.example.jsonl`**. Emitted rows use **`category`: `tool_trace`** for **`--context-filter tool_trace`** during training.

- **`--features mens-dei`**: enables **`vox train`** (local provider **bails** with the canonical **`vox mens train --backend qlora …`** command; Together API; **`--native`** Burn scratch) and `vox mens` surfaces that call **`vox-orchestrator-d`** (generate, review, workflow, check, fix). RPC **method names** are centralized in [`crates/vox-cli/src/dei_daemon.rs`](../../../crates/vox-cli/src/dei_daemon.rs) (`crate::dei_daemon::method::*`) so CLI and daemon stay aligned. **`vox mens review`** uses `ai.review`; it does **not** embed the old TOESTUB/Fabrica/CodeRabbit tree.
- **`--features dei`**: **`vox orchestrator`** (`vox dei` compat alias) — orchestrator CLI (`commands::dei`); build with `cargo build -p vox-cli --features dei`. Subcommands include **`status`**, **`submit <description> [--files …] [--priority urgent|background] [--session-id <id>]`** (session groups context like MCP `session_id`), **`assistant`**: multi-line stdin submit loop with **`--session-id`** (default `cli-assistant`) and optional **`--files`** / **`--priority`**, **`queue`**, **`rebalance`**, **`config`**, **`pause`/`resume`**, **`save`/`load`**, **`undo`/`redo`**. Workspace/snapshot/oplog (JSON on stdout, same payloads as MCP **`vox_workspace_*`**, **`vox_snapshot_*`**, **`vox_oplog`**): **`vox dei workspace create <agent_id>`**, **`vox dei workspace status <agent_id>`**, **`vox dei workspace merge <agent_id>`**, **`vox dei snapshot list [--agent-id <id>] [--limit <n>]`**, **`vox dei snapshot diff <before> <after>`**, **`vox dei snapshot restore <snapshot_id>`** (`S-` prefix optional), **`vox dei oplog list [--agent-id <id>] [--limit <n>]`**, **`vox dei takeover-status [--agent-id <id>] [--human]`** (repo + workspace + short snapshot/oplog tails; **`--human`** prints a short summary before the JSON).
- **`--features coderabbit`**: enables **`vox review coderabbit`** — GitHub/CodeRabbit batch flows in Rust (`crates/vox-cli/src/commands/review/coderabbit/`). Build: `cargo build -p vox-cli --features coderabbit` (often pair with `mens-base` if you omit default features: `--no-default-features --features coderabbit,mens-base`). Set **`GITHUB_TOKEN`** or **`GH_TOKEN`**.

### `vox review coderabbit` (feature `coderabbit`)

Splits local changes into concern-based PRs with a **real baseline** (`origin/<default>` → `cr-baseline-*`) and **git worktrees** under **`.coderabbit/worktrees/`** so the main working tree is not checked out per chunk. **Plan-only** (default): writes **`.coderabbit-semantic-manifest.json`**. **Execute**: add **`--execute`** (pushes baseline, opens PRs into baseline, writes **`.coderabbit/run-state.json`** for resume). Before opening worktree PRs, **`semantic-submit --execute`** re-scans the dirty tree and **aborts with `[drift]`** if the changed-file set no longer matches the plan (replan without `--resume`). The drift check **ignores** paths the command itself creates as untracked files (**`.coderabbit-semantic-manifest.json`**, **`.coderabbit/run-state.json`**) so they do not false-trigger drift.

For full-repo waves (`--full-repo`), the semantic manifest persists coverage counters (`candidate_files`, `included_files`, `ignored_files`) and plan output now prints ignored-rule buckets so operators can audit what was intentionally excluded from a “0-100%” run. **`semantic-submit`** can write a machine-readable ignore audit via **`--write-ignored-paths <file.json>`** and add one-off prefix exclusions with repeatable **`--extra-exclude-prefix`** (merged after `Vox.toml`). When any paths map to the unassigned bucket, plan output also prints **top unassigned path prefixes**; optional **`max_unassigned_ratio`** in **`Vox.toml`** fails planning if that fraction of included files is unassigned.

| Step | Command |
|------|---------|
| Dry-run / plan | `vox review coderabbit semantic-submit` |
| Full-repo plan (all tracked files) | `vox review coderabbit semantic-submit --full-repo` |
| Apply | `vox review coderabbit semantic-submit --execute` |
| Full-repo apply (open PRs for whole tree) | `vox review coderabbit semantic-submit --full-repo --execute` |
| Resume after failure | **`--resume`** reuses baseline from **`.coderabbit/run-state.json`** if you omit **`--baseline-branch`**; or pass **`--baseline-branch`** that matches the saved baseline. **`--force-chunks`** redo all chunks. |
| Legacy “commit everything to default branch” | **`--commit-main`** (broad `git add -u` — use only if intentional) |
| Size batches from `git diff` | Plan: `vox review coderabbit batch-submit`. Write manifest: **`batch-submit --execute`**. Caps are **clamped to the selected tier** (`--tier` or `Vox.toml`, default Pro). |
| Full-repo stacked planner (orphan baseline, mutates checkout) | Plan + manifest: `vox review coderabbit stack-submit`. Live: **`stack-submit --execute`**. **`max_files_per_pr`** is tier-clamped; on failure the tool **restores your original branch** when possible. Prefer **`semantic-submit`**. |
| Single PR from current branch | `vox review coderabbit submit` (still does checkout/`git add -A` in-repo — avoid on dirty trees) |
| Ingest / tasks | `vox review coderabbit ingest <pr>` [`-o file`] [`--db-only` or `--db-and-cache`] [`--reingest-window <tag>`] [`--idempotency-key <key>`] / `vox review coderabbit tasks <pr> --format markdown` |
| Backfill local cache to DB | `vox review coderabbit db-backfill [--input .coderabbit/ingested_findings.json]` |
| DB reporting / recovery | `vox review coderabbit db-report <pr> [--json]` / `vox review coderabbit deadletter-retry <id>` |
| Wait for bot review | `vox review coderabbit wait <pr> [--timeout-secs N]` |

**Manifest files (when written)**

| Subcommand | Plan-only | With `--execute` |
|------------|-----------|------------------|
| `semantic-submit` | `.coderabbit-semantic-manifest.json` | same + git/PR actions |
| `batch-submit` | console only | `.coderabbit-batch-manifest.json` |
| `stack-submit` | `.coderabbit-stack-manifest.json` (always) | same + git/PR actions |

**`Vox.toml`** — optional **`[review.coderabbit]`**: `tier`, `delay_between_prs_secs`, `max_files_per_pr`, **`exclude_prefixes`** (path prefixes, forward slashes) -> drop noise paths from semantic/batch/stack planning; **`allow_markdown_prefixes`** — paths starting with these prefixes keep `*.md` / `*.txt` in semantic payloads (otherwise extension rules drop them for code-first review). **Semantic grouping** defaults to the bundled v1 rules in **`contracts/review/coderabbit-semantic-groups.v1.yaml`**. **`groups_config`** (repo-relative path) **replaces** that bundled file. **`semantic_workspace_crates`** (default `true`) runs **`cargo metadata`** once per plan and injects one prefix rule per workspace member under **`crates/<dir>/`** (chunk names like `crate_<package>`). **`legacy_chunk_split`** (default `false`) uses legacy alphabetical splits for oversized groups; CLI mirror: **`semantic-submit --legacy-chunk-split`**. **`max_unassigned_ratio`** (optional, `0.0`–`1.0`) aborts **`semantic-submit`** planning when the share of **included** files in the unassigned group exceeds the threshold.

**Coverage SSOT:** [`architecture/coderabbit-review-coverage-ssot.md`](../archive/research-2026-q1/coderabbit-review-coverage-ssot.md) defines the canonical scope and operational meaning of full-repository CodeRabbit coverage in Vox.

**VoxDB-first ingest:** `vox review coderabbit ingest` writes to `external_review_*` tables by default. Local `.coderabbit/ingested_findings.json` is now optional mirror state (`--db-and-cache`) rather than the authoritative source.

**Git hygiene**: `.gitignore` includes **`.coderabbit/worktrees/`**. You may commit **`.coderabbit/run-state.json`** if you want a shared run map (or keep it local). **Ignored in drift/planning (normalized repo-relative paths, including leading `./`)**: anything under **`.coderabbit/`** (local tooling, worktrees). Chunk worktree overlays **do not recurse into `.coderabbit/`** when copying from the main tree, so nested tool dirs are not duplicated.
- **`--features dashboard`**: reserved **no-op** in `vox-cli`. The old **`vox mens` chat / agent / dei / learn** commands are removed from the CLI surface (they depended on the historical **`vox-orchestrator`** module tree, not the minimal workspace crate). Use **`vox-codex-dashboard`** / the VS Code extension for dashboard-style surfaces.
- **`VOX_BENCHMARK=1`**: after training paths that invoke it, runs **`vox mens eval-local`** (requires `gpu`) using `VOX_BENCHMARK_MODEL` / `VOX_BENCHMARK_DIR` when set.

## Related docs

- **Rustdoc / layout**: [`docs/src/reference/cli.md`](#)
- **Ecosystem narrative** (may include commands beyond this binary): [`how-to-cli-ecosystem.md`](../how-to/how-to-cli-ecosystem.md)
- **Compiler pipeline** (orientation): [`architecture/vox-compiler-architecture-research-2026.md`](../architecture/vox-compiler-architecture-research-2026.md)


## Crate: `vox-cli`

Rust package path: **`crates/vox-cli`**. Produces the **`vox`** binary (`src/main.rs`) and **`vox-compilerd`** (`src/bin/vox-compilerd.rs`, stdio JSON dispatcher for `dev` and compiler-subcommand RPC).

### Scope

This checkout’s `vox-cli` is a **minimal** compiler driver: clap dispatch, codegen orchestration, and a growing set of subcommands (including **`vox init`**). Feature-gated surfaces (Mens, review, MCP server, etc.) still depend on `Cargo` features — see [`reference/cli.md`](cli.md).

Authoritative **user-facing** command list: [`reference/cli.md`](cli.md).

### Subcommands → source

| CLI | Module |
|-----|--------|
| `vox build` | `src/commands/build.rs` |
| `vox check` | `src/commands/check.rs` |
| `vox test` | `src/commands/test.rs` |
| `vox run` | `src/commands/run.rs` |
| `vox bundle` | `src/commands/bundle.rs` |
| `vox fmt` | `src/commands/fmt.rs` |
| `vox init` | `src/commands/init.rs` (shared scaffold: **`vox-project-scaffold`**) |
| `vox lsp` | `src/commands/lsp.rs` |
| `vox architect` | `src/commands/diagnostics/tools/architect.rs` (features **`codex`** and/or **`stub-check`**) |

**Library / dispatch modules (not always exposed as `vox` subcommands):** `src/commands/info.rs` (registry metadata), `src/commands/runtime/**` (extended run/dev/info/tree/shell). Inline script execution (`runtime/run/{script,backend,sandbox}`) builds with **`--features script-execution`**; Axum Mens inference server (`commands/ai/serve`) builds with **`--features execution-api`** (implies `script-execution` + `gpu` + Axum + `vox-corpus` validation helpers).

### Shared modules

| Path | Role |
|------|------|
| `src/pipeline.rs` | Shared lex → parse → typecheck → HIR frontend (prefer for new commands) |
| `src/config.rs` | `VOX_PORT` / `default_port()`, `set_process_vox_port` (compilerd + `vox run --port`) |
| `src/templates.rs` | Embedded Vite/React scaffold strings for `bundle` / `run` |
| `src/fs_utils.rs` | Directory helpers, `resolve_vox_runtime_path`, script-cache GC |
| `src/dispatch_protocol.rs` | JSON line types shared by `dispatch.rs` and `compilerd` |
| `src/dei_daemon.rs` | Stable **`vox-orchestrator-d`** RPC method ids + `call()` wrapper (spawn error hints) |
| `src/dispatch.rs` | Spawn `vox-compilerd` / named daemons, stream responses; `DAEMON_SPAWN_FAILED_PREFIX` for consistent spawn-failure text (`dei_daemon` enriches errors) |
| `src/compilerd.rs` | In-process stdio RPC implementation for `vox-compilerd` |
| `src/watcher.rs` | `notify` watch helper for `compilerd` `dev` rebuilds |
| `src/v0.rs` | Obsolete generation bridge (now handled by direct `npx v0 add` sidecar) |

### Library target

`src/lib.rs` owns the `Cli` parser, `run_vox_cli()`, and shared modules; `src/main.rs` only initializes tracing and calls `run_vox_cli()`.

### Build

```bash
cargo build -p vox-cli
# binaries: target/debug/vox(.exe), target/debug/vox-compilerd(.exe)
```

Install from the repo:

```bash
cargo install --locked --path crates/vox-cli
```


## CLI design rules

Single source for **shipped `vox` CLI** conventions (see also [`reference/cli.md`](cli.md), [`cli-scope-policy.md`](../archive/research-2026-q1/cli-scope-policy.md), [`CLI command reachability`](cli.md#cli-command-reachability)).

### Hierarchy and naming

- **One primary tree** of nouns/verbs; avoid near-synonyms (`update` vs `upgrade`) for the same action.
- **One canonical spelling per command** in docs/registries/scripts; preserve compatibility aliases in clap (example: canonical `mesh-gate`, alias `mens-gate`).
- **Latin-themed group commands** (`fabrica`, `mens`, `ars`, `recensio`) mirror the flat top-level commands for discoverability; legacy top-level names remain **active** (not hidden).
- **Subcommand depth** should stay ≤ 2 for most flows; deeper trees only for dense domains (e.g. `mens corpus`).
- **Retired / deprecated** commands stay in the registry with `status` and doc’d migration (see [`command-surface-duals.md`](../ci/command-surface-duals.md)).

### Help, output, and exit codes

- Every subcommand supports **`--help`**; root supports **`--version`** (via clap on `VoxCliRoot`).
- **Machine-readable / JSON** output belongs on **stdout** where a command documents it; **diagnostics and errors** on **stderr**.
- Prefer **`--json`**, **`--quiet`**, **`--verbose`** on subcommands that emit structured or noisy output; root sets hints via env (`VOX_CLI_GLOBAL_JSON`, `VOX_CLI_QUIET`) when using global flags.
- **Non-zero exits** must mean something actionable (document in help where non-obvious).

### Description style standard

Use one canonical command description in clap for each command, then reuse it in docs/editor surfaces.

- **What**: one sentence describing the operation.
- **Why/When**: one short phrase for first-time guidance when non-obvious.
- Keep wording stable so `vox commands` output, docs tables, and editor quick-picks do not drift.

### Global flags (root)

- **`--color auto|always|never`** — forwarded to `vox_cli::diagnostics` (`NO_COLOR` still wins when set).
- **`--json`** — sets `VOX_CLI_GLOBAL_JSON=1` for subcommands that honor it.
- **`--verbose` / `-v`** — if `RUST_LOG` is unset, sets it to `debug` before tracing init.
- **`--quiet` / `-q`** — sets `VOX_CLI_QUIET=1` for supported commands.
- **`doctor --json`** is the subcommand’s own machine JSON; **`vox --json doctor`** only sets `VOX_CLI_GLOBAL_JSON` for code paths that read it — do not assume they are interchangeable.

### Completions

- **`vox completions <shell>`** — use **`clap_complete`**; shells: **bash**, **zsh**, **fish**, **powershell**, **elvish**. Install by redirecting stdout to the appropriate completion path for your shell (see [`reference/cli.md`](cli.md)).

### Adding or renaming commands

1. Implement in `crates/vox-cli` (and internal surfaces as needed).
2. Add or update the **`vox-cli` projection** in **`contracts/operations/catalog.v1.yaml`** (schema: **`contracts/operations/catalog.v1.schema.json`**), then run **`vox ci operations-sync --target cli --write`** (or **`--target all`**) so **`contracts/cli/command-registry.yaml`** stays generated.
3. Update **`docs/src/reference/cli.md`** (including the **CLI command reachability** section) when `reachability_required` is not `false`.
4. Run **`vox ci operations-verify`** and **`vox ci command-compliance`** before merge (also enforced in CI).


## Command index — not yet covered in detail

The sections above document 50 of the 76 top-level `vox` commands. The remaining 26 are listed here with the description clap already carries, so the surface is at least discoverable while the prose catches up. Run `vox <command> --help` for flags and subcommands.

<!-- ponytail: hand-written index, sourced from `vox commands --format json` on 2026-08-23. It will drift as commands are added. The durable fix is generating this table from `build_catalog()` (which reflects the clap tree directly) and gating it in `ssot-drift`; see docs/src/api/DOC_GAPS.md. -->

| Command | What it does |
|---------|--------------|
| `vox audit` | Run quality-gate checks from `contracts/ci/check-targets.v1.yaml` (`vox audit`) |
| `vox bundle-app` | Bundle a Vox source file into a complete web application (use `vox fab bundle` or `vox fabrica bundle`) |
| `vox catalog` | Manage the `.vox/repositories.yaml` cross-repo catalog |
| `vox chat` | Send one message through the harness's chat pipeline and print the reply (`vox chat`) |
| `vox component` | Vendor a shadcn/ui component into the project (fetches source from ui.shadcn.com and writes it under your `components.json` aliases) |
| `vox config` | Manage global configuration and preferences |
| `vox container` | Manage OCI containers: build/run the current project with resource limits |
| `vox dispatch` | Dispatch-time routing preview — project what the dispatcher would do (P2-T6) |
| `vox drift-check` | Workspace drift and pattern-repetition linter |
| `vox ext` | Extensions lane — unified entry for legacy and ML subcommands (`ext`) |
| `vox grammar` | Export the Vox language grammar in various formats for MENS training |
| `vox harness` | Evaluate the harness itself (not a model) against a golden task set with a pass^k multi-sample gate (`vox harness eval`) |
| `vox llm` | LLM-native context and prompt generation tools (`vox llm prompt`) |
| `vox model` | Manage models: discovery, scoreboard, and explainability (`vox model`) |
| `vox new` | Scaffold a new Vox application from opinionated presets (`vox new web`) |
| `vox plan` | Agentic Planning tools: Create, replan, and bypass planning steps (`vox plan`) |
| `vox play` | Scaffold and immediately run a temporary Vox project (`vox play`) |
| `vox plugin` | Install, remove, list, and inspect Vox plugins |
| `vox policy` | View the unified policy catalog (CI gates, language rules, audits) |
| `vox repair` | Automatically repair syntax and type errors in a `.vox` file via LLM (`vox repair`) |
| `vox repl` | Interactive Vox expression REPL (read-eval-print loop) |
| `vox rollback` | Roll back the orchestration stack or task execution state using the vox-bounded-fs ledger |
| `vox snapshot` | Insta snapshot helpers: detect and clean up orphaned `.snap` files (`vox snapshot orphans [--clean]`) |
| `vox stop` | Emergency stop the orchestrator (MCP/daemon local stop request) |
| `vox term` | Headless-capable ratatui terminal UI — block-model shell + AI agent strip |
| `vox wasm` | Raw precompiled WASI module execution (needs `--features script-wasi`) |

## CLI command reachability

This page maps **`vox` subcommands** in [`crates/vox-cli/src/lib.rs`](../../../crates/vox-cli/src/lib.rs) -> their **implementation modules** under [`crates/vox-cli/src/commands/`](../../../crates/vox-cli/src/commands).

### Reachable from default / feature matrix

| CLI variant | Feature gate | Handler module |
|-------------|--------------|----------------|
| `build` | default | `commands::build` |
| `check` | default | `commands::check` |
| `test` | default | `commands::test` |
| `run` | default | `commands::run` |
| `script` | `script-execution` | `commands::runtime::run::script` |
| `dev` | default | `commands::dev` |
| `live` | `live` | `commands::live` |
| `bundle` | default | `commands::bundle` |
| `compile` | default | `commands::compile` — **`vox compile`** packaging umbrella (also **`vox fabrica compile`**) |
| `fmt` | default | `commands::fmt` (`vox_compiler::fmt::try_format`; `--check` supported) |
| `graph` | default | `commands::graphify` (aliases: `graphify`, `search`) |
| `add` | default | `commands::add` |
| `audit` | default | `commands::audit` |
| `auth` | default | `commands::auth` |
| `remove` | default | `commands::remove` |
| `update` | default | `commands::update` |
| `lock` | default | `commands::lock` |
| `sync` | default | `commands::sync` |
| `deploy` | default | `commands::deploy` |
| `upgrade` | default | `commands::upgrade` (toolchain only) |
| `init` | default | `commands::init` |
| `pm` | default | `commands::pm` |
| `login` | default | `commands::login_shared::run_login` |
| `logout` | default | `commands::login_shared::run_logout` |
| `lsp` | default | `commands::lsp` |
| `doctor` | default / `codex` | `commands::doctor` or `commands::diagnostics::doctor` |
| `secrets` | default | `commands::secrets` |
| `clavis` | default | deprecated alias of `secrets` |
| `architect` | `codex` or `stub-check` | `commands::diagnostics::tools::architect` |
| `snippet` | default | `commands::extras::snippet_cli` |
| `share` | default | `commands::extras::share_cli` |
| `codex` | default | `commands::codex` |
| `repo` | default | `commands::repo` |
| `research` | default | `commands::research`: infra **`vox research up`** / **`vox research down`** / **`vox research status`** / **`vox research eval`**; **`vox research run`** calls orchestrator `run_research` (`--json`, `--scope`, `--site-scope`, …) |
| `db` | default | `commands::db` + `commands::db_cli` dispatch |
| `scientia` | default | `commands::scientia` (facade over `db_cli` research helpers) |
| `telemetry` | default | `commands::telemetry` (optional upload queue; ADR 023) |
| `openclaw` | `ars` | `commands::openclaw` |
| `skill` | `ars` | `commands::extras::skill_cmd` |
| `gamify` | `extras-ludus` | `commands::extras::ludus_cli` |
| `stub-check` | `stub-check` | `commands::stub_check` |
| `ci` | default | `commands::ci` |
| `commands` | default | `command_catalog` |
| `mens` | `mens-base` or `gpu` | `commands::mens` |
| `populi` | `populi` | `commands::populi_cli` |
| `oratio` | `oratio` | `commands::oratio_cmd` |
| `speech` | `oratio` | `commands::oratio_cmd` (visible alias of `oratio`) |
| `review` | `coderabbit` | `commands::review` |
| `train` | `gpu` + `mens-dei` | `commands::ai::train` |
| `dei` | `dei` | `commands::dei` (alias `orchestrator`) |
| `bundle-app` | default | `commands::bundle` |
| `chat` | default | `commands::chat` |
| `component` | default | `commands::add_component` |
| `config` | default | `commands::config` |
| `container` | default | `commands::container` |
| `dispatch` | default | `commands::dispatch` |
| `drift-check` | default | `commands::drift_check` |
| `emit` | default | `commands::emit` |
| `ext` | default | `commands::ext` |
| `grammar` | default | `commands::grammar::handle` |
| `harness` | default | `commands::harness` |
| `llm` | default | `commands::llm` |
| `new` | default | `commands::new` |
| `play` | default | `commands::play` |
| `plugin` | default | `commands::plugin` |
| `policy` | default | `commands::policy` |
| `repair` | default | `commands::repair` |
| `rollback` | default | `commands::rollback` |
| `term` | default | `vox_term::app::run` |
| `wasm` | `script-wasi` | `commands::wasm` |

### `vox-compilerd` RPC (not CLI variants)

Daemon dispatch lives in [`crates/vox-cli/src/compilerd.rs`](../../../crates/vox-cli/src/compilerd.rs). Methods call **`commands::build`**, **`check`**, **`bundle`**, **`fmt`**, **`doc`**, **`test`**, **`run`**, **`dev`** — not the removed `commands/compiler/` tree.

### `vox-orchestrator-d` (orchestrator daemon sidecar)

`vox-orchestrator-d` is built from the orchestrator crate (not `vox-cli`) and exposes JSON-line `orch.*` methods for MCP sidecar pilots. Optional ADR 022 sidecar: **`vox-orchestrator-d`** can run as a long-lived process (`VOX_ORCHESTRATOR_DAEMON_SOCKET` TCP/stdio). MCP currently uses a split-plane transition model: daemon-aligned RPC pilots may own task/agent lifecycle slices, but many VCS/context/event/session features still read embedded stores unless explicitly moved behind daemon contracts.

- Build: `cargo build -p vox-orchestrator --bin vox-orchestrator-d`
- Run (TCP): `VOX_ORCHESTRATOR_DAEMON_SOCKET=127.0.0.1:9745 target/debug/vox-orchestrator-d`
- Run (stdio): `VOX_ORCHESTRATOR_DAEMON_SOCKET=stdio target/debug/vox-orchestrator-d`

When using with MCP, set MCP-side `VOX_ORCHESTRATOR_DAEMON_SOCKET` to the same TCP peer and optionally enable pilots with `VOX_MCP_ORCHESTRATOR_RPC_READS=1` / `VOX_MCP_ORCHESTRATOR_RPC_WRITES=1`. Repo-id mismatch warning/error behavior is controlled by `VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT`.

### Removed / non-compiled trees (historical)

The following directories under `commands/` were **not** referenced from `commands/mod.rs` or the CLI and have been **removed** to reduce dead surface {

- `commands/compiler/` — duplicate of canonical `build` / `check` / `doc` / `fmt` / `bundle` paths used by `compilerd` and CLI.
- `commands/pkg/` — unwired package manager experiment.
- `commands/serve_dashboard/` — superseded by `vox-codex-dashboard` / extension flows.
- `commands/infra/` — legacy unwired tree; **`vox deploy`** is implemented in **`commands::deploy`** (delegates to **`vox-container`**).
- `commands/learn.rs`, `commands/dashboard.rs` — orphan modules with no `mod` declaration.

### Shared subtrees

- `commands::runtime` — used by `run` (script lane), `dev` re-exports, and feature-gated script execution.
- `commands::extras` — snippet, share, skill, ludus, ARS helpers.

