---
title: "Dependency & Migration Handoff (2026)"
description: "Codebase-verified handoff for the deferred 'good and healthy' migrations the crate-leaf-design track did not land: the latest-version dependency upgrades (rmcp, wasmtime, swc, cargo_metadata, sysinfo, typify, jsonschema, thiserror, candle), the nominative renames, the plugin-SDK / publish-clean / GUI-release work, and a cross-reference to the Turso-ownership backlog. Each item carries blast radius, what breaks, what to fix, and honest difficulty/agony ratings (implementation cost, not just file count)."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-06-05"
training_eligible: true
training_rationale: "Captures verified per-migration cost/breakage analysis so a future contributor can pick up the latest-version upgrades and deferred refactors without re-deriving the blast radius or hitting the same landmines."
---

# Dependency & Migration Handoff (2026)

> **Why this exists.** The crate-leaf-design remediation ([PR #145](https://github.com/vox-foundation/vox/pull/145)) and the Wave-A dependency bumps ([PR #146](https://github.com/vox-foundation/vox/pull/146)) landed the *mechanical, low-risk* work. Everything that's "good and healthy but deferred" is captured here, with **codebase-verified** estimates so the next contributor can act without re-scoping. The stated goal is to **eventually reach the latest version of every dependency** unless there's a real disadvantage beyond effort — this doc records exactly how much effort and agony each one is.

## How to read the ratings

Two independent 1–5 axes, because blast radius ≠ difficulty:

- **Difficulty** = implementation complexity. `1` = bump the pin, recompile, done. `3` = compiler-guided edits across several files. `5` = redesign / hand-rebase against a moving upstream with no safety net.
- **Agony** = how unpleasant/risky it actually is (silent-failure potential, no-CI validation, twitchy trial-and-error, toolchain/MSRV ripple). A change can be low-difficulty but high-agony (e.g. needs hardware CI doesn't run).

All call-site counts, file paths, and API deltas below were verified against the tree at `0.6.0` (2026-06-05) and the upstream changelogs/docs cited.

---

## 1. Master summary

| Migration | Pin → latest | Crates / files | Difficulty | Agony | One-line verdict |
|---|---|---|---|---|---|
| **sysinfo** | 0.38 → 0.39 | 4 / 5 | **1** | **1** | No API breaks; code already modern. MSRV→1.95 only. |
| **jsonschema** | 0.26 → 0.46 | ~15 / ~18 | **1** | **1** | 20 minors but code already on `validator_for`; bump 3 manifest lines. |
| **thiserror** | 1 → 2 | ~56 (workspace=true) | **1** | **1** | Breaking patterns provably absent; `workspace-hack` already on 2. One-line bump. |
| **typify** | 0.6.2 → 0.7 | 1 / 1 | **1** | **2** | Additive; must **regenerate** the committed generated file. |
| **rmcp** | 0.16 → 1.7 | 1 / 2 | **2** | **2** | ~7 structs became `#[non_exhaustive]`; convert literals to builders. |
| **wasmtime/-wasi** | 42 → 45 | 2–3 / 1 | **2** | **2** | Vox is on stable sync-p1 path; real cost = MSRV→1.93 + `workspace-hack` re-pin. |
| **cargo_metadata** | 0.18 → 0.23 | 2 / 2 | **2** | **2** | `Package.name`→`PackageName` newtype + `TargetKind` cmp; ~10 real edits. |
| **swc_ecma_*** | 39/23/23/21 → 41/25/25/23 | 1 / 1 | **2** | **3** | `Atom`/`Ident` accessor churn; one optional-feature file; lockstep bump. |
| **candle** | 0.9 → 0.10 | 8 / ~73 | **4** | **5** | The hard one — **agony is 100% the `candle-kernels` patch re-cut** (GPU, zero CI). |

**Headline:** seven of the nine are difficulty ≤2. Several "scary, many-versions-behind" deps (jsonschema, sysinfo, thiserror) are **near-no-op bumps** because the code already targets the post-rewrite API and only the manifest pin is stale. Only **candle** is genuinely hard, and its cost is dominated by one hand-maintained patch, not the ~73 call sites.

---

## 2. The near-no-op tier (difficulty 1) — do these first, batch as "Wave B+"

### sysinfo 0.38 → 0.39
- **Blast radius:** 4 crates, ~12 sites — `vox-orchestrator` (`system-metrics` feature), `vox-cli-core/daemon_ipc/process_supervision.rs`, `vox-cli/commands/ci/kill_stuck_tests.rs`, `vox-ml-cli/commands/populi_cli.rs`.
- **What breaks:** **nothing.** The 0.30-era renames (`refresh_cpu_all`, `global_cpu_usage`, `ProcessesToUpdate::Some(_, true)`, `available_memory`) were already adopted. 0.39's only change is additive + **MSRV → Rust 1.95**.
- **Fix / verify:** bump pin; confirm CI toolchain ≥ 1.95; build the four crates (orchestrator with `--features system-metrics`).

### jsonschema 0.26 → 0.46
- **Blast radius:** ~15 crates / ~18 files — canonical wrapper `vox-jsonschema-util/src/lib.rs` (`validator_for`, `Validator`, `.validate()`), plus ~25 contract/test call sites. **Also pinned twice in `workspace-hack/Cargo.toml`** (must bump in lockstep).
- **What breaks:** **nothing reachable.** The big rewrites (`compile`→`validator_for`, single-error return, `iter_errors`, Draft 2020-12 default) all landed **at/before 0.26** and are already in use. The post-0.26 changes (0.29 consuming-`self` options builder; 0.33 `LocationSegment` `Cow`) touch APIs Vox doesn't call. `Cargo.lock` currently resolves **0.26.2**, so the bump is real (not already-resolved) but the code is forward-compatible.
- **Fix / verify:** bump root `Cargo.toml` + the two `workspace-hack` lines to `"0.46"` (keep `default-features = false` + `resolve-file`); run the schema-contract tests across `vox-jsonschema-util`, `vox-publisher`, `vox-orchestrator`, `vox-integration-tests`, `vox-cli`. Only friction risk: the `resolve-file` feature name in the 0.27–0.46 window.

### thiserror 1 → 2
- **Blast radius:** ~56 crates via `thiserror = { workspace = true }`; 115 `#[derive(Error)]`, 407 `#[error(...)]`, 98 `#[from]`/`#[source]` sites.
- **What breaks:** **nothing reachable.** Verified absent: `{r#...}` raw-ident format refs (0 hits), `#[backtrace]` (0 hits). Direct-dependency + MSRV (1.61) requirements already met. **`workspace-hack` already pins `thiserror = "2"`** (and the `qlora-rs`/`webview2-com-sys` patches use 2.x), so 1.x and 2.x already coexist in the lockfile — bumping the root **unifies** rather than introduces.
- **Fix / verify:** flip root `"1"`→`"2"`; `cargo build` the workspace; fix any compiler-flagged tuple/format edge cases (expected: zero–handful). **No silent-failure risk** — the compiler catches everything.

### typify 0.6.2 → 0.7
- **Blast radius:** **1 file** — `vox-scientia-jsonschema-codegen/src/main.rs` (`TypeSpace::default()`, `add_root_schema`, `to_stream`), an offline `publish = false` codegen binary.
- **What breaks:** no API break (0.7.0 is additive — string-pattern merges, newtype JsonSchema). But the additive codegen **may change the generated output** in `vox-research-events/src/schema_types.generated.rs`.
- **Fix / verify:** bump pin; **rerun the generator** (`cargo run -p vox-scientia-jsonschema-codegen`) and commit the regenerated `@generated` file (never hand-edit); run `vox-research-events` tests. Agony is 2 only because of the regen-and-commit dance + a possible drift-guard flag until regenerated.

---

## 3. The small-but-real tier (difficulty 2)

### rmcp 0.16 → 1.7  (the 0.x→1.x "scary major" that isn't)
- **Blast radius:** **1 real crate** — `vox-orchestrator-mcp` (`server.rs`, `registry.rs`, `lifecycle.rs`), ~7 struct-literal sites. `vox-cli`'s `rmcp` dep is feature-plumbing only (zero source refs). `mcp_client.rs` is a *custom* abstraction — not rmcp — leave it.
- **What breaks:** the 1.0 stabilization did **not** redesign traits or transport. `ServerHandler::{initialize,list_tools,call_tool}` signatures, `ServiceExt::serve`, `transport::stdio()`, and the `ErrorData`/`RoleServer`/`Content::text` surface are byte-compatible. The **one** break: several `model` structs became `#[non_exhaustive]` (`Tool`, `CallToolResult`, `InitializeResult`, `Implementation`, `ServerCapabilities`, `ToolsCapability`), so the ~7 struct literals fail to construct from outside the crate (E0639) — and `..Default::default()` does **not** exempt them.
- **What to fix:** convert each literal to the type's builder/factory — `Tool::new(...)`, `CallToolResult::success(...)/::error(...)`, `Type::default()` + field assignment for the capability structs. No logic changes, no transport/service rewiring.
- **Recommended:** single focused PR; pin-bump + ~7 edits in 2 files; smoke-test `vox mcp` stdio after. **< 1 day.** Don't believe the "needs its own spec cycle" framing in earlier notes — that was a pre-scoping guess; the verified surface is small.
- **Unknown:** exact 1.7 builder names for the capability structs (confirm `ServerCapabilities::builder()` vs `Default`). Let `cargo build` enumerate any extra non_exhaustive sites.

### wasmtime / wasmtime-wasi 42 → 45  ("3 majors" of mostly-irrelevant churn)
- **Blast radius:** `vox-wasm-engine` (the one real file, `engine.rs` + `preopen.rs`), `vox-plugin-runtime-wasm`, optional in `vox-cli`. ~18 sites. **Also:** `workspace-hack` pins `wasmtime-environ`/`wasmtime-internal-core` at `"42"` (bump or regen hakari).
- **What breaks:** **nothing on Vox's path.** Vox uses the stable **synchronous WASI Preview 1** embedding (`wasmtime_wasi::p1::{WasiP1Ctx, add_to_linker_sync}`, `build_p1()`, `preopened_dir`, fuel) — *not* the async component model, C API, or legacy `wasi-common` where all the 43/44/45 breakage lives. Real impacts: **MSRV → Rust 1.93** (workspace-wide ripple) and the `workspace-hack` re-pin.
- **Fix / verify:** single jump 42→45 (no stair-stepping needed); bump root + `workspace-hack` + toolchain floor together; build the two engine crates; run the wasm golden-execution tests. Most likely zero code edits.
- **Agony driver:** the MSRV bump and hakari regen, not the code.

### cargo_metadata 0.18 → 0.23  (the one with confirmed compile errors)
- **Blast radius:** 2 crates — `vox-arch-check/src/main.rs` (~33 sites) and `vox-cli/src/commands/ci/pre_push.rs` (~11 sites).
- **What breaks (verified by actually hitting it):**
  1. `Package.name: String` → `PackageName` newtype. `.as_str()` still works, but `pkg.name.clone()`/`dep.name.clone()` flowing into `String` containers break: `vox-arch-check:1195` (`Vec<(String,String)>`), `pre_push` `directly_changed`/`reverse_deps: HashMap<String, Vec<String>>`. Fix: `.name.as_str().to_string()`.
  2. `TargetKind` no longer `impl PartialEq<&str>`: `t.kind.iter().any(|k| k == "cdylib")` at `vox-arch-check:1169,1184`. Fix: compare to `cargo_metadata::TargetKind::CDyLib`.
- **Fix / verify:** ~10 compiler-guided edits across 2 files; `cargo build -p vox-arch-check -p vox-cli`. Mechanical.
- *(This is one of the 3 "held" bumps from [PR #146](https://github.com/vox-foundation/vox/pull/146).)*

### swc_ecma_parser/ast/visit/common 39/23/23/21 → 41/25/25/23
- **Blast radius:** **1 file** — `vox-drift-check/src/extractors/typescript.rs` (~10 sites), behind the optional `drift-typescript` feature. **Important correction:** the TS *emit* path (`vox-codegen/src/codegen_ts/`) is **string-template based, zero SWC** — the only SWC consumer is the drift-check feature extractor.
- **What breaks:** `Atom`/`JsWord` accessor churn (the code already uses the fallible `.value.as_str()` shape; it may move again), and possible `Ident`→`IdentName` split in `MemberProp`/`ImportSpecifier` payloads. **No AST construction** here (parse + read-only `Visit`), so the dreaded `ctxt: SyntaxContext` field additions mostly don't apply.
- **Fix / verify:** bump all four pins **in one commit** (SWC crates are version-locked); build `--features drift-typescript`; fix accessor/match-arm deltas; green the 2 unit tests.
- **Agony 3** = SWC's per-major twitchiness + mandatory lockstep, not depth.

---

## 4. The hard one: candle 0.9 → 0.10  (difficulty 4, agony 5)

**The call-site churn is tractable. The agony is one hand-maintained patch.**

- **Blast radius:** **8 crates, ~73 files** — `vox-quantize`, `vox-inference`, `vox-plugin-mens-candle-cuda`, `vox-plugin-mens-candle-metal`, `vox-populi` (mens), `vox-oratio` + `vox-plugin-oratio`, `vox-ml-cli`. (Correction: `vox-tensor`/`vox-hf-layout` do **not** import candle.) Usage is deep: `Tensor`, `Device`, `DType`, `VarBuilder`/`VarMap`, `quantized::{GgmlDType,QTensor,QMatMul}`, `gguf_file`, safetensors.
- **What breaks (likely):**
  1. **`DType`/`GgmlDType` enum churn** — candle ships *non-`#[non_exhaustive]`* enum additions in patch releases ([candle#3333](https://github.com/huggingface/candle/issues/3333)). Exhaustive `match`es (e.g. `vox-quantize/src/policy.rs`) will need new arms. Most probable real call-site break.
  2. **Transitive cudarc bump** — every candle minor tends to bump `cudarc` ([candle#2858](https://github.com/huggingface/candle/pull/2858)), changing PTX/driver expectations. This is what couples the kernels patch to the candle version.
  3. Minor `VarBuilder`/`gguf_file` signature drift (handful of edits).
- **THE CRUX — re-cutting `patches/candle-kernels-0.9.2`:** verified, this is **not** a thin shim. It's a customized pure-Rust fork: zero build-deps, shells `nvcc` directly to emit PTX (`-arch=sm_80`), self-heals stale PTX, applies `-Xcompiler /Zc:preprocessor` for the one C++17 kernel (`reduce.cu`), uses `pub static` (not `const`) to survive thin-LTO stripping (the `CUDA_ERROR_INVALID_IMAGE` fix), disables the `moe` WMMA kernels, and hard-codes the `KERNEL_NAMES`/`Id`/`ALL_IDS` inventory to the 0.9.2 kernel set. Re-cutting for 0.10.x = vendor new upstream kernels, **re-apply every customization by hand** onto an added/removed/renamed `.cu` set, regenerate+commit PTX, and validate on a **Windows+MSVC+CUDA box — which CI does not run** (the CUDA/metal plugins build in zero CI). A regression lands *silently* until a human runs CUDA inference.
- **Highest-leverage first question:** does upstream `candle-kernels 0.10` already fix the MSVC/LTO issues the fork works around? **If so, the patch can be dropped rather than re-cut** — that single question dominates the cost. Read the actual 0.9→0.10 diff before touching anything (the 0.10 changelog was not retrievable via search).
- **Recommended approach:** (1) read the diff + answer the can-we-drop-the-patch question; (2) bump candle + the `qlora-rs` patch together, fix call sites compiler-driven (DType matches first); (3) keep the **CPU-only path green first** as a cheap signal; (4) treat the kernels re-cut as an isolated sub-project gated on a GPU smoke test (`vox-inference` Qwen forward).
- **Verdict on "is it worth it?":** candle is the one place where the honest answer is *"the work is real and partly un-CI-able."* Defer until either (a) upstream retires the patch need, or (b) there's GPU CI. Not a blocker for anything else.

---

## 5. Non-dependency "good and healthy" refactors

### 5.1 Nominative renames (cheap, mechanical, but touch CI guards)
The `nomenclature_guard` Latin→English denylist defines an intended migration: `populi→ml`, `gamify→gamification`, `oratio→speech`, `schola→tutorial`, `mens→ml`, plus already-grandfathered names. Each rename follows the **same proven recipe** used for `vox-dei-shim → vox-research-shim` in [PR #145](https://github.com/vox-foundation/vox/pull/145): `git mv`, rewrite `vox_x`→`vox_y` imports, update root `Cargo.toml`/`layers.toml`/`where-things-live.md`, **update the `nomenclature_guard` HISTORICAL_ALLOWLIST and `retired_symbol_check`**, regenerate `doc-inventory.json`, and **repoint doc file-links** (the link-checker catches moved paths — see PR #145's doc-link fix). Difficulty 2, agony 2–3 each.
- **`vox-populi → vox-ml`** is the highest-value and **intersects the Turso migration** (§6) and the mens/candle crates — sequence carefully (see §6).
- From the nominative audit, two extra clarity renames worth doing: **`vox-eval → vox-eval-metrics`** (it's metrics, not the interpreter) and **`vox-runtime → vox-runtime-core`** (disambiguate from `vox-actor-runtime`). Both optional; the corrected descriptions already mitigate.
- **Gotcha:** doing a rename in the *same worktree* where you later branch off `main` can leave an orphaned `crates/<new-name>/` dir (untracked `.vox/` cache) that breaks Cargo's `crates/*` glob — `rm -rf` it. (Hit during this session.)

### 5.2 Plugin independence — SDK, publish-clean, ABI-parity CI
- **Publish-clean the foundation + leaves** (so third parties can build single-feature plugins off crates.io): drop `workspace-hack` from `vox-plugin-api` (+ the ready leaves `vox-nanopub`/`vox-research-events`/`vox-quantize`/`vox-crypto`), add them to the two `.config/hakari.toml` `workspace-members` exclude arrays, give independent versions. **Owner-gated** at the actual `cargo publish` step (needs a crates.io org/token) — but the publish-*readiness* is a small, mechanical PR. Difficulty 2, agony 2 (hakari fiddliness).
- **Plugin SDK** — a `#[vox_plugin]` proc-macro + `vox plugin new` scaffold to replace the ~30 lines of hand-copied `abi_stable` boilerplate per plugin. Additive; best done *after* the dep-boundary allowlist stabilizes. Difficulty 3.
- **`plugin-abi-parity` CI wiring** — the guard is fixed and green locally (PR #145), but wiring it into CI needs a runner that can build *every current-triple plugin cdylib including the CUDA one* — i.e. a **CUDA/Metal CI runner**. Infra-gated, not code.

### 5.3 GUI release assets
The `vox gui` install path (PR #145) builds-from-source in a checkout and falls back to instructions otherwise. The **turnkey prebuilt-asset download** needs a release-pipeline job that builds + uploads `vox-gui` per platform (currently none exist) — plus macOS codesigning/notarization for the Tauri app. Infra + owner work; the catalog `[[component]]` model and resolution are already in place.

---

## 6. Turso-ownership migration (verified corrections to the existing handoff)

A separate handoff already exists (`docs/agents/codex-turso-allowlist.md`, plus the "Turso Ownership Migration — Handoff") over the policy SSOT [`contracts/db/data-storage-policy.v1.yaml`](../../../contracts/db/data-storage-policy.v1.yaml) and guards `turso-import-guard` + `policy-allowlist-parity`. This track should **adopt that backlog**. A read-only re-verification against the tree produced these **corrections** — apply them before working it:

**Real migration sites (only 4 — characterized):**

| Site | turso surface | Size | Target vox-db op | Diff | Agony |
|---|---|---|---|---|---|
| `vox-populi/src/transport/store/voxdb.rs` (`VoxDbMeshStore`) | Row, Value, Error, `params!`, `params_from_iter` | **516 LoC, ~11 methods, ~9 SQL** | new `store/ops_mesh.rs` **or extend existing** `facade/vox_mesh.rs` + `mesh_locks.rs`/`mesh_exec_leases.rs` | **5** | **5** |
| `vox-gui/src/commands/memory.rs` | Rows, Row, Error, `params!` | 6 `COUNT(*)` reads | `ops_gui.rs` (new) or extend `ops_memory.rs` | 2 | 2 |
| `vox-scientia/src/producers/bench_history.rs` | `params!` + raw `.connection()` | 2 reads, ~45 LoC | **extend EXISTING `ops_scientia.rs`** | 2 | 2 |
| `vox-corpus/src/arca_replay.rs` | `params!` via `query_all` facade | 2 reads | `ops_corpus.rs` (new) or extend `ops_a2a.rs` | 2 | 3 |

**Corrections vs the old handoff (important):**
1. **`ops_scientia.rs` already exists** — the bench-history work *adds methods*, it doesn't create the file. (Old note said "once ops_scientia.rs is added.")
2. **`vox-workflow-runtime/tests/` allowlist entry is dead** (0 `turso::` hits) — remove it.
3. **`vox-plugin-populi-mesh/src/transport/store/` allowlist entry is dead** — it's a JSON-file fallback, no turso; the "mirrors vox-populi with direct turso" comment is false. Remove it.
4. **`vox-codegen/src/codegen_rust/` is a true false-positive** — all `turso::` are *generated string text* + test assertions; the `\bturso::` regex can't be removed until the guard learns to skip string contexts.
5. **Partial mesh ops already live in `vox-db`** (`facade/vox_mesh.rs`, `mesh_locks.rs`, `mesh_exec_leases.rs`) — site #1 is **not greenfield**; reconcile to avoid duplicate query surfaces.
6. **There are THREE allowlists, not one** — `turso-import-allowlist.txt`, `sql-connection-api-allowlist.txt`, and `query-all-allowlist.txt` (the last governs `arca_replay`'s `db.query_all(...)`). Migrating a site must drop its entry from *all* relevant lists.
7. **`vox-populi → vox-ml` rename intersection:** site #1's path moves under the rename. Do the Turso migration of site #1 **before or atomically with** the rename, or the allowlist line `crates/vox-populi/src/transport/store/` becomes a hard `policy-allowlist-parity` failure (points at a vanished dir).

---

## 7. Recommended order of attack

1. **Batch the near-no-ops** (sysinfo, jsonschema, thiserror) + typify into one "Wave B" currency PR — verify each builds, regen typify's output. (Confirm MSRV: sysinfo→1.95, which may be the real gate.)
2. **rmcp** as its own small PR (~7 edits, smoke-test stdio MCP).
3. **cargo_metadata** (finishes the held [#146](https://github.com/vox-foundation/vox/pull/146) trio) + **swc** (one file each).
4. **wasmtime 42→45** bundled with the **MSRV bump** as a deliberate workspace-wide step (this unblocks sysinfo's 1.95 floor too).
5. **Nominative renames** — start with `vox-populi→vox-ml` **coordinated with Turso site #1**; then the Turso satellites (gui/scientia/corpus) independently.
6. **Plugin publish-clean** (readiness PR) — actual publish is owner-gated.
7. **candle 0.9→0.10** *last*, gated on answering "can we drop the kernels patch?" and on GPU validation. This is the only item where deferring indefinitely is defensible.

**Net:** the goal of "latest version of everything" is realistic and mostly cheap — ~7 of 9 dep upgrades are a day or less each, with no real disadvantage beyond effort. The two genuine costs are the **MSRV ripple** (a one-time workspace bump that several upgrades share) and **candle's un-CI-able kernels patch**. Everything else is mechanical.

## 8. Cross-references

- Owning plan: `docs/superpowers/plans/2026-06-05-crate-leaf-design-remediation.md` (on [PR #145](https://github.com/vox-foundation/vox/pull/145), not yet merged to `main`).
- Turso backlog: `docs/agents/codex-turso-allowlist.md`; policy [`contracts/db/data-storage-policy.v1.yaml`](../../../contracts/db/data-storage-policy.v1.yaml); guards `crates/vox-cli/src/commands/ci/{run_body_helpers/guards.rs,policy_allowlist_parity.rs}`.
- Currency audit context: [`build-and-crate-org-improvement-plan-2026-06.md`](./build-and-crate-org-improvement-plan-2026-06.md).
- The candle kernels patch: `patches/candle-kernels-0.9.2/{build.rs,src/lib.rs}`; coupling pins in root `Cargo.toml` `[patch.crates-io]`.
- Nomenclature map: `crates/vox-cli/src/commands/ci/nomenclature_guard.rs` (LATIN_STRUCTURAL_DENYLIST).
