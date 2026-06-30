# vox-cli split — extraction recipes (turnkey)

> Companion to `docs/superpowers/specs/2026-06-30-vox-cli-split-design.md`. Recipes from a
> 5-agent parallel analysis. **Done so far:** `vox-cli-share` (`8f5aceb94a`), `vox-cli-research`
> (`46082fc420`). Pattern proven; each below is a self-contained PR.

## Proven extraction pattern (from share + research)
1. `mkdir crates/<crate>/src`; `git mv` the source dir's files in (the module's `mod.rs` → `lib.rs`).
2. `sed -i 's/crate::commands::<path>::/crate::/g'` the moved files (self-refs → crate-root).
3. Rewire every caller (see per-crate list) to `vox_cli_<x>::…`.
4. New `Cargo.toml` mirroring `vox-cli-ci` + **all** deps (scan `\b[a-z][a-z0-9_]+::`, not just `use` lines — that's the trap that cost two rebuilds on share).
5. Add `vox-cli-<x> = { workspace = true }` to `crates/vox-cli/Cargo.toml` and `… = { path = "crates/vox-cli-<x>" }` to root `[workspace.dependencies]`. (`crates/*` glob auto-adds the member.)
6. `cargo build -p vox-cli-<x> -p vox-cli` → commit → rebase onto main → push.

---

## vox-cli-review (READY, Tier 1.2 — highest value: removes vox-forge from default build)
**Feature-gated (`coderabbit`), needs mod.rs surgery + optional-dep wiring.**
- **Move:** `commands/review/coderabbit/` (30 files) → crate. **Also move** `ReviewCli` enum + `run_coderabbit` fn out of `commands/review/mod.rs` (lines ~12–22, the `#[cfg(feature="coderabbit")]` block) → the crate's `lib.rs` as `pub enum ReviewCli` + `pub async fn run(cli: ReviewCli)`. **`dei.rs` + its `#[cfg(feature="dei")]` block STAY** in `review/mod.rs`.
- **Crate is unconditional**; gating moves to vox-cli's dep: `vox-cli-review = { workspace = true, optional = true }` and `coderabbit = ["dep:vox-cli-review", …]` in vox-cli Cargo.toml.
- **Self-ref:** `crate::commands::review::coderabbit::` → `crate::`.
- **Callers:** `cli_dispatch/lanes.rs` + `cli_dispatch/mod.rs:558` (`run_coderabbit` → `vox_cli_review::run`); `lib.rs:595,602` (`commands::review::ReviewCli` → `vox_cli_review::ReviewCli`, keep the `#[cfg(feature="coderabbit")]`).
- **Deps:** anyhow, blake3(pure), chrono(clock,std,serde), clap(derive), reqwest(json,rustls-tls,stream), serde(derive), serde_json, serde_yaml(=serde_yaml_ng), tempfile, tokio(full set), toml, tracing, walkdir, vox-bounded-fs, vox-code-audit, vox-config, vox-corpus, vox-db, vox-forge, vox-git, vox-http-client, vox-secrets.
- **Verify both** the default build AND `--features coderabbit`.

## vox-cli-extras (READY, Tier 1.3 — selective)
- **Move:** `commands/extras/ludus`, `commands/extras/ars`, `commands/extras/ludus_cli.rs`, `commands/extras/skill_cmd.rs` → crate. **DO NOT move** `share`, `snippet`, `share_cli`, `snippet_cli` — they use `crate::workspace_db` (vox-cli internal).
- **Features:** crate exposes `extras-ludus` (gates ludus) + `ars` (gates ars), opt-in.
- **Self-ref:** `crate::commands::extras::{ludus,ars}::` → `crate::`.
- **Callers (rewire ONLY ludus_cli/skill_cmd; snippet/share_cli unchanged):** `lib.rs:381,389`; `commands/ext.rs:10,16`; `latin_cmd.rs:70,82`; `cli_dispatch/mod.rs:246,353` → `vox_cli_extras::{ludus_cli,skill_cmd}::…`.
- **Deps:** anyhow, clap(derive), tokio(full), serde(derive), serde_json, owo_colors, tracing, uuid(v4,serde), vox-bounded-fs, vox-code-audit, vox-config, vox-db, vox-gamify, vox-openclaw-runtime, vox-orchestrator, vox-skill-discovery.

## vox-cli-model (BLOCKED — small refactor first)
- **Blocker:** `commands/model/discover.rs` calls `crate::commands::model::eval::run()`. **Fix:** extract `discover` + `eval` together; rewire discover's call to `eval::run` *within* the crate (`crate::eval::run`). Then it's clean. Deps: clap, anyhow, serde_json, owo_colors, comfy_table(v7), tracing, vox-actor-runtime, vox-db, vox-orchestrator, vox-research-shim, vox-config, vox-secrets.

## vox-cli-runtime (BLOCKED — needs a layering refactor; defer)
- **8 coupling blockers** into vox-cli internals: `crate::{build_lock, build_service, cli_args, dispatch, fs_utils, isolation, pipeline, wasi_dir_mode}`. **Fix (Option A):** create a `vox-cli-execution` crate that ALSO pulls out `build_lock`, `fs_utils`, `isolation`, `wasi_dir_mode`, then runtime depends on it. This is the spec's "re-modularize first" case — a bigger effort; carries `script-wasi`/wasmtime (~69 crates). **Do after Tier 1 + `vox-cli-contracts`.**

## Order
Tier 1: review → extras (research + share done). Then `vox-cli-contracts` (the trait seam) → Tier 3 CI extraction (the headline win) → model → runtime. Gate each on `cargo build --timings` before/after evidence.
