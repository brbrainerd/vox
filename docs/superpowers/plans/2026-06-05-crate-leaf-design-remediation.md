# Crate & Leaf-Design Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Vox crate names/descriptions accurate, bring key external dependencies current, and turn the plugin/leaf design into one that lets single-feature crates be built and consumed independently of the Vox CLI.

**Architecture:** Seven independent tracks ordered by risk/effort. Tracks 0–1 are mechanical hygiene (descriptions, renames, dependency bumps). Tracks 2–5 build out the "independent crate / plugin" capability incrementally, each producing working software on its own. Track 6 is the strategic keystone (the `vox-db` → compiler coupling) and is **design-gated** — it must not start until a brainstorming/design pass closes its open questions.

**Tech Stack:** Rust 2024, cargo workspace + `[workspace.dependencies]`, `cargo-hakari` (`workspace-hack`), `abi_stable` plugin ABI, `vox-arch-check` (layers.toml enforcement), `vox ci` guards, Tauri 2 (GUI), Vox automation scripts (`scripts/*.vox`).

**Source audit:** This plan operationalizes the 5-agent audit of 2026-06-05 (dependency currency, independent-buildability graph, GUI-as-plugin, nominative accuracy, plugin-system architecture).

**Cross-cutting conventions (read before any track):**
- Never `cargo fmt --all` (Windows arg-limit `os error 206`). Format with `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- Adding/renaming a workspace crate requires editing root `Cargo.toml` `[workspace.dependencies]` AND adding/updating its row in `docs/src/architecture/layers.toml`, or `vox-arch-check` fails.
- After dependency edits affecting the lockfile, regenerate workspace-hack: `cargo hakari generate` then `cargo hakari verify`.
- Verification baseline used throughout: `cargo run -q -p vox-arch-check` (architecture), `cargo build -p vox-cli` (spine compiles), `cargo clippy -p <crate> -- -D warnings` (merge gate).

---

## Verification addendum (2026-06-05) — READ BEFORE EXECUTING

A 4-agent read-only pass verified every track's assumptions against live code. Deltas that change execution:

- **Track 0.2 (rename `vox-dei-shim`) — blast radius is larger than "update imports".** Beyond ~9 code sites (`vox-orchestrator-mcp`, `vox-cli`, `vox-db/src/research_pipeline.rs`, the crate's own 8 tests) it also appears in **`crates/vox-cli/src/commands/ci/nomenclature_guard.rs` and `retired_symbol_check.rs`** (CI guards may need updating / may trip) and in **`docs/agents/doc-inventory.json`** (auto-generated — regenerate, never hand-edit). Treat this as its own focused task with a full `cargo build --workspace` verify, not a quick rename.
- **Track 2.1 — corrected.** Only `vox-plugin-api` carries `workspace-hack`; `vox-plugin-types` has none and is **already excluded**. The hakari exclude key is **`[traversal-excludes].workspace-members` + `[final-excludes].workspace-members` in `.config/hakari.toml`**, NOT `[hakari].exclude`. ABI version is **12** (confirmed). To stop hakari re-adding the dep, add `"vox-plugin-api"` to those two `workspace-members` arrays.
- **Track 2.2/2.3 — confirmed greenfield, but 2.2 (SDK/derive macro) is additive scope** not in `plugin-system-redesign-2026.md` (which lists third-party out-of-tree as Goal 5, marketplace as a non-goal, and is silent on an SDK). Existing guards `no-plugin-cdylib-as-compile-dep` (inverse direction) and `plugin-abi-parity` exist; the new `plugin-dep-boundary` is genuinely missing. `vox-plugin-publication` violation confirmed (`src/ingest.rs:6-7`).
- **Track 3 — all three confirmed spine-free.** `vox-crypto` has **no `workspace-hack`** (that sub-step is a no-op for it; applies only to `vox-research-events` + `vox-quantize`). `cargo tree` always prints the crate's own root line — "no spine edges" means none *other than itself*.
- **Track 4 — EASIER than written, and one policy snag.** The `nanopub` module (`crates/vox-scientia/src/nanopub/{mod,trig,signing,network}.rs`) already uses `vox_crypto::facades` (not raw dalek) and depends on **only `vox-crypto` + `sha2` + `hex`** — it does **NOT** use `vox-research-events` (that linkage is conceptual in a doc-comment only) and its structs don't derive serde. **Snag:** `network.rs` is a live `publish_stub` ("Phase 8 stub"); extracting it ships a stub to crates.io (violates no-stubs policy). **Adapt:** extract only `trig` + `signing`; leave/defer `network`. Design gate 4.1 question (b) is already answered by code (vox-crypto).
- **Track 5 — partly moot / partly stale.** Build-skip is **already true** (Step 4 is a no-op). **`vox gui` already exists** (`crates/vox-cli/src/commands/gui.rs`) — *augment* it with install-if-absent, don't create it. Catalog schema needs editing in **both** `schema.rs` (new `Component` struct) **and** `lib.rs` (`CatalogFile` must deserialize `#[serde(rename = "component")]`). **Missing context:** `voxup/src/install.rs` has **zero** fetch/build machinery today (only writes placeholder shims) — the install mechanism (prebuilt-asset download vs `cargo build -p vox-gui` vs `self_update`) is an undecided design choice.
- **Track 6 — SHALLOW, and the `vox-codegen` edge is a PHANTOM.** `vox-db`'s `vox-codegen` dep had **zero real uses** (one doc-comment) — **removed in this branch** (build-verified). The remaining `vox-compiler` coupling is confined to `vox_compiler::ast::{decl, types, scalar_mapping, span}` across ~6 files (`ddl/emit.rs`, `ddl/diff.rs`, `auto_migrate.rs`, `facade/schema.rs`, `schema_digest/{helpers,api}.rs`) — used purely as a declaration-AST data source (no parser/typeck/eval calls). The one open design question: can those AST types be split out of `vox-compiler` without dragging the rest of the compiler?

---

## Track 0 — Nominative accuracy

**Status:** Description fixes (Task 0.1) are **already applied** in this worktree. The renames (0.2–0.4) remain.

### Task 0.1: Crate description corrections — DONE

**Files modified (for the record / review):**
- `crates/vox-eval/Cargo.toml` — was "Vox expression evaluator (interpreter…)"; now describes eval **metrics** and points at `vox-compiler/src/eval/` for the real interpreter.
- `crates/vox-scientia/Cargo.toml` — was "semantic search/RAG"; now the research-pipeline umbrella.
- `crates/vox-scientia/src/lib.rs` — header no longer calls `nanopub`/`claim_extractor`/`inspect_bridge`/`ro_crate`/`ingest` "planned"; they are present modules.
- `crates/vox-inference/Cargo.toml` + `crates/vox-distributed-training/Cargo.toml` — dropped "WIP/stub" framing; describe the shipped backends; keep honest "no in-tree consumers yet".
- `crates/vox-runtime/Cargo.toml` — removed `vox-inference` from the consumer list (it does not depend on `vox-runtime`).
- `docs/src/architecture/where-things-live.md` — fixed the `vox-eval` row (was actively misdirecting to the wrong crate) and de-stubbed the `vox-inference` row.

- [x] **Step 1:** Apply the seven edits above.
- [ ] **Step 2: Verify metadata still valid.** Run: `cargo run -q -p vox-arch-check`. Expected: no new errors (description-presence + layer rules unaffected by text changes).
- [ ] **Step 3: Commit.**
```bash
git add crates/vox-eval/Cargo.toml crates/vox-scientia/Cargo.toml crates/vox-scientia/src/lib.rs crates/vox-inference/Cargo.toml crates/vox-distributed-training/Cargo.toml crates/vox-runtime/Cargo.toml docs/src/architecture/where-things-live.md
git commit -m "docs(crates): correct misleading/stale crate descriptions (vox-eval, vox-scientia, vox-inference, vox-distributed-training, vox-runtime)"
```

### Task 0.2: Rename `vox-dei-shim` → `vox-research-shim`

**Why:** "dei" is opaque and falsely reads as Diversity/Equity/Inclusion. The crate is a research-pipeline + model-selection shim extracted from `vox-orchestrator`.

**Files:**
- Rename dir: `crates/vox-dei-shim/` → `crates/vox-research-shim/`
- Modify: `crates/vox-research-shim/Cargo.toml` (`name`), root `Cargo.toml` (`[workspace.dependencies]` entry), `docs/src/architecture/layers.toml` (crate row), `docs/src/architecture/where-things-live.md` (row), every `Cargo.toml` and `use vox_dei_shim::` site.

- [ ] **Step 1: Find the blast radius first.** Run:
```
Grep pattern="vox[_-]dei[_-]shim" output_mode="files_with_matches"
```
Expected: a finite list (Cargo.tomls of dependents + `use vox_dei_shim` imports + docs + layers.toml + memory docs). Record it — every hit is a edit site.
- [ ] **Step 2: Git-move the directory** (preserves history):
```bash
git mv crates/vox-dei-shim crates/vox-research-shim
```
- [ ] **Step 3:** In `crates/vox-research-shim/Cargo.toml` set `name = "vox-research-shim"`.
- [ ] **Step 4:** In root `Cargo.toml`, change the dependency line to `vox-research-shim = { path = "crates/vox-research-shim" }` (keep alphabetical-ish placement).
- [ ] **Step 5:** Update `docs/src/architecture/layers.toml` crate row key and `where-things-live.md` row (and its `vox-dei-shim (A-12 wedge)` mention) to the new name.
- [ ] **Step 6:** For each dependent found in Step 1, change its `Cargo.toml` dep key `vox-dei-shim` → `vox-research-shim` and every `use vox_dei_shim` / `vox_dei_shim::` → `vox_research_shim`.
- [ ] **Step 7: Verify.** Run: `cargo build -p vox-research-shim` then `cargo run -q -p vox-arch-check`. Expected: builds; arch-check clean (no orphan, row present).
- [ ] **Step 8:** Format touched crates: `cargo fmt -p vox-research-shim` (+ each dependent crate touched).
- [ ] **Step 9: Commit.**
```bash
git add -A
git commit -m "refactor(crate): rename vox-dei-shim -> vox-research-shim (name was opaque/misleading)"
```

### Task 0.3 (optional): Rename `vox-eval` → `vox-eval-metrics`

Same mechanical procedure as 0.2 (`Grep "vox[_-]eval\b"` — beware false hits on `vox-compiler` `eval` module paths; only the crate dep + `use vox_eval` are targets). Lower priority than 0.2 because the corrected description already disambiguates. Do only if reviewers still find `vox-eval` confusable with the compiler's `eval`.

### Task 0.4 (optional): Rename `vox-runtime` → `vox-runtime-core`

Disambiguates the passive foundation crate from `vox-actor-runtime` (the actual executor). Highest blast radius of the renames (it is depended on widely + by the mobile uniffi bridge). Defer unless the three-runtime confusion is actively biting; the corrected consumer list (0.1) is the cheap mitigation.

---

## Track 1 — Dependency currency

**Approach:** three waves by risk. Each dependency edit is in root `Cargo.toml` `[workspace.dependencies]`. After each wave: `cargo build -p vox-cli`, `cargo hakari generate && cargo hakari verify`, then run the affected crates' tests.

### Task 1.1: Wave A — low-risk bumps (do together)

**Files:** `Cargo.toml` (versions only).

- [ ] **Step 1:** Bump these pins:
```
hf-hub        0.4   -> 0.5
sherpa-onnx   1.12  -> 1.13
nvml-wrapper  0.11  -> 0.12
sysinfo       0.38  -> 0.39
lru           0.13  -> 0.18
strum         0.26  -> 0.28
cargo_metadata 0.18 -> 0.23
typify        0.6.2 -> 0.7
which         7     -> 8
self_update   0.43.1-> 0.44
```
- [ ] **Step 2:** `cargo build -p vox-cli`. Expected: compiles; fix any trivial API nits surfaced by the compiler (these crates are mostly additive). Record any that need a follow-up and revert just that one if non-trivial.
- [ ] **Step 3:** `cargo hakari generate && cargo hakari verify`.
- [ ] **Step 4: Commit.** `git commit -am "chore(deps): wave A low-risk dependency bumps"`.

### Task 1.2: Wave B — mechanical-but-breaking (one commit each)

Do **one per commit** so a regression is bisectable. For each: bump, `cargo build -p vox-cli` (+ the owning crate's tests), fix the mechanical breaks, commit.

- [ ] `thiserror` 1 → 2 (workspace-wide; `#[from]`/`source` attribute tweaks). Verify: `cargo build --workspace` then clippy gate.
- [ ] `tokio-tungstenite` 0.24 → 0.29 (already tracked as WS2-T5). Owning crates: webhook/populi-mesh transports.
- [ ] `tantivy` 0.22 → 0.26 (behind `heavy-retrieval` feature — build with that feature on: `cargo build -p vox-search --features heavy-retrieval`).
- [ ] `symphonia` 0.5 → 0.6 and `scraper` 0.20 → 0.27. ⚠️ Before committing `scraper`, run `cargo tree -d -p vox-search` (or the owning crate) to confirm it does not ADD a second `html5ever` stack — the build-org audit already flagged duplicate html5ever via tauri-utils.
- [ ] `reqwest` 0.12 → 0.13, `toml` 0.8 → 1.x, `tiktoken-rs` 0.5 → 0.12 (independent; one commit each).

### Task 1.3: Wave C — dedicated migrations (each gets its own spec→plan)

These are too large for this plan; each becomes its own brainstorming→plan cycle. Listed here so they are tracked, in recommended order:
1. **`rmcp` 0.16 → 1.7** (0.x→1.x MCP protocol break; do first — it is stable-1.0 now). Owner surface: `vox-orchestrator-mcp`.
2. **`candle` 0.9 → 0.10** ⚠️ **coupled to `patches/candle-kernels-0.9.2`** — bumping candle invalidates that patch and its MSVC `-ccbin` fix. The plan for this MUST include re-creating the kernel patch against 0.10. Also re-validate `patches/qlora-rs-1.0.5` and `patches/aegis-0.9.8`.
3. **`wasmtime` / `wasmtime-wasi` 42 → 45** (3 majors of component-model/WASI breaks; bump in lockstep; behind `script-execution`).
4. **`swc_ecma_*` 39/23/21 → matched latest set** (mismatched swc majors do not compile — bump as one set).
5. **`jsonschema` 0.26 → 0.46** (20 minors of validator-API rewrites).

- [ ] **Do NOT** touch `jj-lib` (`=0.27.0` exact pin is deliberate), `turso` (stay on 0.6 stable, not 0.7-pre), or the intentional `rand` 0.8/0.9 and `schemars` 1/0.8 duals.

### Task 1.4: Build-time feature trims (independent of currency)

- [ ] Set `default-features = false` on `tauri` (`crates/vox-gui` build only) and narrow `hyper`'s `features = ["full"]` to the actually-used set (`http1`,`http2`,`client`,`server` as applicable). Verify GUI and orchestrator-mcp still build. Commit separately.

---

## Track 2 — Plugin independence foundation

**Goal:** make the plugin system actually support independently-built single-feature plugins (today the ABI/runtime isolation is solid but build-time independence is not).

### Task 2.1: Make `vox-plugin-api` + `vox-plugin-types` publish-clean

**Files:** `crates/vox-plugin-api/Cargo.toml`, `crates/vox-plugin-types/Cargo.toml`.

- [ ] **Step 1:** Confirm the blocker. Run `Grep pattern="workspace-hack" path="crates/vox-plugin-api/Cargo.toml"`. Expected: a `workspace-hack = { workspace = true }` line (the publish blocker — it is `publish = false`).
- [ ] **Step 2:** Remove the `workspace-hack` dependency from both crates' `Cargo.toml`. (hakari excludes named crates via `[hakari] ... exclude`; add both to that exclude list in `.config/hakari.toml` so `cargo hakari verify` does not re-add them.)
- [ ] **Step 3:** Give them an independent version line: replace `version.workspace = true` with an explicit `version = "0.1.0"` (plugin-ABI versioning is independent of the monorepo `0.6.0`). Keep `VOX_PLUGIN_ABI_VERSION` as the runtime contract.
- [ ] **Step 4: Verify build still works in-tree.** `cargo build -p vox-plugin-host` (the host depends on the api). Then `cargo hakari verify`.
- [ ] **Step 5: Verify standalone-buildability.** From a clean temp checkout of just these crates' sources (or `cargo package -p vox-plugin-api`), confirm `cargo package -p vox-plugin-api` succeeds with no path-dep errors. Expected: packages cleanly.
- [ ] **Step 6: Commit.** `git commit -am "feat(plugin): make vox-plugin-api/-types publish-clean (drop workspace-hack, independent version)"`.

### Task 2.2: Plugin SDK — derive macro + scaffold

**Files:** new `crates/vox-plugin-sdk/` (proc-macro `#[vox_plugin]` emitting the `export_root_module`/`manifest_json`/`init` glue), new `crates/vox-plugin-api/templates/plugin/` (a `cargo generate` template), `vox-cli` `plugin new` subcommand (`crates/vox-cli/src/commands/`).

- [ ] **Step 1 (TDD):** In `crates/vox-plugin-sdk/tests/`, write a failing test that a struct annotated `#[vox_plugin(manifest = "Plugin.toml")]` exposes the three required root symbols and round-trips its manifest JSON. Run it — expect compile failure (macro absent).
- [ ] **Step 2:** Implement the proc-macro to emit the abi_stable boilerplate currently hand-copied by `vox-plugin-nvml-probe/src/lib.rs` (use it as the reference shape). Re-run test → pass.
- [ ] **Step 3:** Convert `vox-plugin-nvml-probe` to use `#[vox_plugin]` as the dogfood case; `cargo run -p vox-cli -- ci plugin-abi-parity` must still pass (ABI 12 unchanged).
- [ ] **Step 4:** Add `vox plugin new <id>` that stamps the template (Plugin.toml + lib.rs using the derive + a `requires` matrix). Add a `vox-cli-tests` E2E that the scaffold compiles.
- [ ] **Step 5:** Add row to `where-things-live.md` + `layers.toml` for `vox-plugin-sdk`. Commit.

### Task 2.3: Enforce the plugin dependency boundary (CI guard)

**Why:** today "depend only on `vox-plugin-api`" is convention, already violated by `vox-plugin-publication` (`use vox_db::VoxDb;`, `use vox_scientia::ingest::*`).

**Files:** new `crates/vox-cli/src/commands/ci/plugin_dep_boundary.rs`, register in `cmd_enums.rs` + `run_body.rs` (mirror `db_schema_coverage.rs` per where-things-live).

- [ ] **Step 1 (TDD):** Write a test fixture plugin that depends on `vox-db`; assert the new guard FAILS it. Run → fails (guard absent).
- [ ] **Step 2:** Implement: for every crate whose dir contains a `Plugin.toml` with a `[code]` payload, parse its `Cargo.toml` deps; allow only `vox-plugin-api`, `vox-plugin-sdk`, `abi_stable`, an explicit allowlist (`vox-config` to start), and any non-`vox-*` crate. Emit a finding per violation.
- [ ] **Step 3:** Run `cargo run -p vox-cli -- ci plugin-dep-boundary`. Expected: it flags `vox-plugin-publication` (and any others). **Do not auto-fix** — record the violations as the input to Track 6 (publication can only shed `vox-db`/`vox-scientia` once those are extractable).
- [ ] **Step 4:** Wire the guard into the merge gate as **warn-only** initially (publication is a known violator until Track 6). Add a `# allow` escape hatch keyed by crate name with a `reason`, mirroring `layers.toml` `[[known_inversions]]`. Commit.

### Task 2.4: Exercise the out-of-tree path for one plugin

- [ ] Move `vox-plugin-browser` to consume the published `vox-plugin-api` (path → version dep) and prove `vox plugin install browser` builds it standalone (catalog already advertises a `github:` `default-source`). This is the proof the catalog's promised loop actually closes. Scope as a small follow-up after 2.1–2.3.

---

## Track 3 — Independent-crate POC (publish the ready leaves)

**Goal:** prove the "stream of independent crates" vision with the three crates that are already leaf-shaped: `vox-research-events`, `vox-quantize`, `vox-crypto`. Each pulls in **no** Vox spine.

### Task 3.1: Decouple the three leaves for publishing

**Files:** `crates/vox-research-events/Cargo.toml`, `crates/vox-quantize/Cargo.toml`, `crates/vox-crypto/Cargo.toml`; `.config/hakari.toml` (exclude list).

- [ ] **Step 1:** For each, confirm the closure is spine-free: `cargo tree -p vox-quantize -e no-dev | Select-String "vox-"`. Expected: `vox-quantize` shows none; `vox-research-events` shows none; `vox-crypto` shows none. (If any unexpected `vox-*` edge appears, stop and record it.)
- [ ] **Step 2:** Remove `workspace-hack` from each and add each to the hakari exclude list. Give each an independent `version` (start `0.1.0`).
- [ ] **Step 3:** `cargo package -p vox-research-events`, `-p vox-quantize`, `-p vox-crypto`. Expected: each packages with no path-dep error. (candle is a legitimate crates.io dep for quantize.)
- [ ] **Step 4:** Add a `README.md` to each describing standalone use (these become the crates.io front page). Commit.
- [ ] **Step 5 (gated on user/org decision):** Actual `cargo publish` is an outward-facing, irreversible action — **do not run it without explicit owner sign-off and a crates.io org/token**. The plan delivers publish-*readiness*, not the publish itself.

---

## Track 4 — `vox-nanopub` as a true leaf (answers the SCIENTIA question)

**Goal:** deliver an independently-consumable nanopublication crate so an external publication system can depend on *just* nanopub — without dragging in `vox-db`/compiler. Today the nanopub code lives **inside** the spine-coupled `vox-scientia` umbrella (`vox_scientia::nanopub`).

**Design rule (non-negotiable):** `vox-scientia` depends on `vox-nanopub`, **never** the reverse.

### Task 4.1: Design gate

- [ ] **Step 1:** Run a `superpowers:brainstorming` pass on: (a) exact public API of the extracted `vox-nanopub` (TriG serialization, Ed25519 signing, Trusty URI), (b) whether it depends on `vox-crypto` (preferred — already a clean leaf, see Track 3) or raw `ed25519-dalek`, (c) what typed inputs it shares with `vox-research-events`. Output: a short spec under `docs/src/architecture/`.

### Task 4.2: Extract the module into a leaf crate (after gate)

**Files:** new `crates/vox-nanopub/` (move from `crates/vox-scientia/src/nanopub/`), `crates/vox-scientia/Cargo.toml` (+dep on `vox-nanopub`), root `Cargo.toml`, `layers.toml`, `where-things-live.md` (move the `vox-nanopub` row out of `[planned]`).

- [ ] **Step 1 (TDD):** Create `crates/vox-nanopub/` with one moved test (e.g. a known TriG fixture → signed nanopub with a stable Trusty URI). Run → fails (crate empty).
- [ ] **Step 2:** `git mv` the `nanopub` module sources into the new crate; depend only on `serde`, `vox-crypto` (or `ed25519-dalek`), and `vox-research-events`. Re-run the moved test → pass.
- [ ] **Step 3:** Replace `vox-scientia`'s inline `pub mod nanopub;` with `pub use vox_nanopub as nanopub;` (or re-export) so existing `vox_scientia::nanopub::*` consumers are unbroken.
- [ ] **Step 4: Verify no spine leaked in.** `cargo tree -p vox-nanopub -e no-dev | Select-String "vox-(db|compiler|codegen|actor|search|cli)"`. Expected: empty.
- [ ] **Step 5:** `cargo run -q -p vox-arch-check` (row present, layering clean), `cargo build -p vox-scientia`. Commit.
- [ ] **Step 6 (optional repeat):** Same procedure for `vox-ro-crate` and `vox-prereg` once nanopub proves the pattern.

---

## Track 5 — GUI as an optional installable component

**Goal:** formalize "CLI-only users don't build the GUI" (already true via `default-members = ["crates/vox-cli"]`) into a first-class opt-in install path. The GUI stays an L5 Tauri **binary** depending on `vox-cli` — it does **not** become a cdylib plugin (Tauri owns the event loop; plugins may not depend on `vox-cli`).

### Task 5.1: Add a `[[component]]` (app) kind to the plugin catalog

**Files:** `crates/vox-plugin-catalog/catalog.toml` (+ schema in `crates/vox-plugin-catalog/src/schema.rs`).

- [ ] **Step 1 (TDD):** In `vox-plugin-catalog` tests, add a failing test that the catalog can declare a `[[component]]` with `id`, `binary`, and `requires.{os,arch}`, distinct from `[[plugin]]`/`[[bundle]]`. Run → fails.
- [ ] **Step 2:** Add the `Component` struct + parse path mirroring the existing `[[bundle]]` shape; add a `vox-gui` component entry (`binary = "vox-gui"`, OS/arch matrix). Re-run → pass.
- [ ] **Step 3:** `cargo test -p vox-plugin-catalog`; commit.

### Task 5.2: Wire `voxup` / `vox gui` to install the component on opt-in

**Files:** `crates/voxup/src/install.rs` (currently a near-stub that does not provision the GUI), `crates/vox-cli/src/commands/` (a `vox gui` launch-or-install command).

- [ ] **Step 1 (TDD):** In `voxup` tests, assert `install --with-gui` resolves the `vox-gui` component from the catalog and records its target path (mock the fetch). Run → fails.
- [ ] **Step 2:** Implement component resolution + install (download/build to the toolchain dir). Re-run → pass.
- [ ] **Step 3:** Add `vox gui` that launches the installed binary, or prompts `voxup install --with-gui` if absent.
- [ ] **Step 4:** Verify CLI-only build is unaffected: `cargo build -p vox-cli` does **not** compile `vox-gui` (confirm with `cargo build -p vox-cli --timings` or that `vox-gui` is absent from the unit graph). Commit.

---

## Track 6 — Strategic keystone: break the `vox-db` → compiler coupling (DESIGN-GATED)

**Why:** `vox-db` depends on `vox-compiler` + `vox-codegen` (`crates/vox-db/Cargo.toml`). Every persistence-touching feature (`vox-scientia`, `vox-publisher`, `vox-search`) therefore inherits the entire language toolchain and cannot be extracted. This single edge is the reason the broad "independent crates" vision is only *partially* feasible today. It is also what makes `vox-plugin-publication` violate the Track 2.3 boundary.

**This track must NOT begin as code.** It requires a dedicated brainstorming → spec → plan cycle.

### Task 6.1: Design gate (brainstorming)

- [ ] **Step 1:** Run `superpowers:brainstorming` on: *why* does `vox-db` depend on `vox-compiler`/`vox-codegen`? (Find the exact `use` sites: `Grep pattern="use vox_compiler|use vox_codegen" path="crates/vox-db/src"`.) Determine whether that surface can move behind a trait or into a `vox-db-codegen` adapter crate, leaving a compiler-free `vox-db` core.
- [ ] **Step 2:** Produce a spec under `docs/src/architecture/` covering the cut line (types-only core vs. codegen adapter), the migration order (`vox-search` and `vox-scientia` re-point to the core), and the `vox-arch-check` rule change. Open questions to resolve there:
  - Does the SCIENTIA umbrella need DB *writes* or only typed *reads*? (Determines whether a read-only trait suffices.)
  - Can `vox-search`'s `vox-actor-runtime` dependency be narrowed in the same pass, or is that a separate keystone?
- [ ] **Step 3:** Only after the spec is approved, write a Track-6 implementation plan via `superpowers:writing-plans`. Do not attempt the extraction inline.

---

## Self-review notes

- **Spec coverage:** all five audit dimensions map to tracks — descriptions/names → Track 0; dependency currency + build-time → Track 1; plugin independence → Track 2; independent crates / SCIENTIA-nanopub question → Tracks 3–4; GUI-as-plugin → Track 5; the structural blocker behind the whole vision → Track 6.
- **Honest gates:** Tracks 4 and 6 are explicitly design-gated rather than fabricating precise code for un-designed extractions; Track 3 Step 5 and any `cargo publish` are owner-gated (outward-facing, irreversible). These are deliberate, not placeholders.
- **Ordering rationale:** 0 and 1 are safe wins that improve navigability and currency immediately; 2 builds the capability; 3 proves it cheaply; 4 delivers the headline user request on top of 3; 5 is independent and small; 6 is the high-value, high-effort keystone that unlocks the rest and is therefore sequenced last and gated.
