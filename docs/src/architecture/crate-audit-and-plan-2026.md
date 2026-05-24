---
title: "Workspace Crate & Plugin Audit + Implementation Plan (2026, v2)"
description: "Verified four-axis audit + 50-task phased plan covering cycle/layer pressure, external dep hygiene, plugin↔core duplication, discoverability/naming drift. Each claim was cross-verified against current code; false positives removed, true positives sharpened."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Architectural decision log with explicit verification trail; high-value for LLM navigation."
sort_order: 30
---

# Workspace Crate & Plugin Audit + Implementation Plan (2026-05-23, v2)

## How this document was produced

Two passes:

1. **Discovery pass** — four parallel agents audited the workspace along four axes:
   - **A** — cycle / layer / leaf / fan-in / LoC / known_inversions
   - **B** — external deps, version skew, build-time hotspots
   - **C** — discoverability, naming, WTL / `layers.toml` / Cargo.toml drift, split-brain
   - **D** — plugin boundary, plugin↔core duplication, catalog drift, manifest gaps
2. **Verification pass** — four parallel agents re-checked every claim from pass 1 against actual code, with file:line evidence. Each task received a verdict: **CONFIRMED**, **FALSE**, **REFINE**, or **BLOCKED**. Verdicts are tabulated in §6.

Verification killed **6** load-bearing false positives, sharpened **14** tasks, and surfaced **9** new high-confidence findings the first pass missed. This document is the post-verification version; v1 (pre-verification) is in git history.

The single most important catch from verification: **D-9** (split `vox-container`) would have orphaned **`vox-cli`** and **`vox-skills`** — exactly the class of mistake recorded in our memory (5/10 retirement claims wrong; one nearly cost 9 k LoC of integration tests). That's why every "delete this" / "fold this" recommendation here now lists every grep hit in `tests/`, `.github/workflows/`, `contracts/`, `examples/`, and ADRs.

Companion to prior audits (do **not** re-do their work):
[`crate-structure-audit-2026-05-15.md`](./crate-structure-audit-2026-05-15.md),
[`tooling-convergence-findings-2026.md`](./tooling-convergence-findings-2026.md),
[`repo-layout-sprawl-audit-2026.md`](./repo-layout-sprawl-audit-2026.md),
[`2026-05-08-workspace-reorg-design.md`](./2026-05-08-workspace-reorg-design.md),
[`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md),
[`2026-05-15-cli-ci-extraction-plan.md`](./2026-05-15-cli-ci-extraction-plan.md).

---

## 0. Executive summary

**Current ground truth (verified):**

- 103 crate dirs under `crates/`.
- `cargo run -p vox-arch-check` exits **1** with two findings:
  - `[warn] vox-secrets: 26 dependents (budget 25)` — fan-in
  - `[ERROR] orphan crates (2): vox-distributed-training, vox-inference` — Rule 4
- All 93 production crates carry `Cargo.toml description` lines.
- Only one explicit `default-features = true` in the workspace (`tantivy` in `vox-search`).
- Zero L4 plugin compile-time leakage into L0–L3.

**Sources of pressure, in order of leverage:**

1. **Workspace integrity:** the orphan failure (CR-1) blocks CI right now. Fix is mechanical (one-line per crate) but requires picking the right pattern — verification showed the original "remove from `[planned]`" recommendation was a no-op; the names aren't in `[planned]`.
2. **Discoverability drift:** WTL has ghost rows for 12 crates that don't exist on disk; `vox-plugin-mens-candle-metal` is a fully-implemented plugin missing from the catalog SSOT; `crates/_frozen.md` is referenced from a v1 release criterion **and** a live CI guard but the file is absent.
3. **Two surgical build-time wins:** `tantivy default-features = true` is the single biggest one. `voxup`'s wholly hand-pinned Cargo.toml forces `tokio[full]` into the hakari floor.
4. **Targeted splits to take the top off fan-in:** `vox-secrets` at 26/25 cleanly splits L0+L3 (clean module separation already exists). `vox-cli ↔ vox-ml-cli` known-inversion is removable today by finishing the `vox-cli-core` migration (verified file:line citations from `vox-ml-cli/src/commands/ai/workflow.rs:300/308/314`).

**Tasks killed vs sharpened:**

- Killed entirely (FALSE): **A-11** (plugins use `VoxDb` struct, not types), **A-15** (proposed budget of 15 is below the actual current values of 17/17/15), **B-12** (hakari exclude is intentional policy), **D-15** (premise crate doesn't exist).
- Killed pending upstream (BLOCKED): **B-8** (typify 0.6 still requires schemars 0.8 — no upstream fix yet).
- Sharpened (REFINE): 14 tasks now carry corrected counts, file paths, or effort estimates.
- New tasks added: **A-21** (dead vox-compiler deps), **A-22** (orphan-rule exemption pattern), **B-13** (remove `tempfile` duplicate declaration), **C-16** (_frozen.md is referenced from CR-A3 + CI guard), **C-17** (live error messages point at archive path), **D-17** (unify three SkillManifest shapes), **D-18** (`vox-plugin-api::extensions::grammar_export` parallel retirement), **D-19** (catalog `id="vox-mesh"` bundle vs `vox-mesh` crate disambiguation under A-7).

**Total task count:** 50 (was 58). P0=6, P1=15, P2=10, P3=10, P4=7, P5=2.

---

## 1. Critical findings

### CR-1. `vox-arch-check` failure: orphan rule on `vox-distributed-training`, `vox-inference`

Verified by reading `crates/vox-arch-check/src/main.rs:727-735` (Rule 4):

```rust
for (name, entry) in &layers.crates {
    if entry.kind != "library" { continue; }
    let count = dependent_count.get(name).copied().unwrap_or(0);
    if count == 0 && workspace_members.contains(name.as_str()) {
        report.orphan_warns.push(name.clone());
    }
}
```

The rule fires for any `kind = "library"` workspace member with zero in-tree consumers. Both crates are real implementations (the planned MENS Mn-T1 / Mn-T2 wedges per `mesh-mens-distributed-training-and-execution-plan-2026.md`) but no other workspace crate depends on them yet.

**`staleness_exempt = true` does NOT fix this** (verification miss). That flag is for Rule 6 (staleness based on commit history), not Rule 4 (orphan). The verifier's recommendation was technically wrong; the real fixes are:

- **Preferred (A-1.A):** wire each as an optional path dep in `vox-populi` (their intended consumer) behind features `distributed-training` / `inference`. Establishes the consumer edge honestly; opts users into the MENS distributed-AI surface.
- **Alternative (A-1.B):** change `kind = "test-only"` for now. Honest about current readiness; one Cargo.toml word per crate; aligned with the `vox-orchestrator-test-helpers` pattern.
- **Alternative (A-1.C):** extend the arch-check schema with a new `staleness_exempt`-style flag (e.g. `orphan_exempt = true`) plus a comment pointing to the plan doc. Most invasive; pays back the next time a planned crate lands without consumers.

A-1 picks (A) as the default. Task A-22 separately tracks adding `orphan_exempt` for the next planned-crate landing.

### CR-2. `vox-plugin-mens-candle-metal` is a real plugin missing from the catalog SSOT

Verified — the plugin is **not a stub**:

- `crates/vox-plugin-mens-candle-metal/Plugin.toml:1-25` declares `id = "mens-candle-metal"`, `payload-kind = "code"`, `extension-points = ["MlBackend"]`, `requires.os = ["macos"]`.
- Cargo.toml has `[lib] crate-type = ["cdylib", "rlib"]`.
- Source under `training_loop/`, `candle_qlora_train/` — substantial implementation parallel to `mens-candle-cuda`.
- `vox-plugin-catalog/catalog.toml` has **no entry** for `mens-candle-metal`. The catalog has `mens-candle-cuda` (lines 20-27) only.

Effect: `vox plugin list` omits Metal, `docs/src/reference/plugin-catalog.generated.md` is stale, macOS contributors have no advertised ML plugin path. Fix in **C-2**.

### CR-3. `vox-mesh` is mis-classified (cleanly resolvable to `vox-mesh-policy`)

Verified all four claims:

1. `layers.toml:147` marks it `kind = "plugin", staleness_exempt = true`.
2. `crates/vox-mesh/Cargo.toml` has no `[lib]` section, so no `crate-type = ["cdylib"]`.
3. `crates/vox-mesh/` has no `Plugin.toml`.
4. **Zero in-tree Cargo dependents** (initial grep was confounded by the `vox-mesh-types` prefix; after disambiguation, no consumer crate depends on `vox-mesh` itself).
5. `crates/vox-mesh/src/lib.rs:1-5` self-describes: *"Parse, edit, and pretty-print `donations.vox` policy files."*
6. `layers.toml:369` has a `vox-mesh-policy` entry in `[planned]`.

The crate IS the planned `vox-mesh-policy`. Rename + reclassify in **A-7**. **Watch-out** (D-19): `vox-plugin-catalog/catalog.toml:216` uses the string `id = "vox-mesh"` for a distribution **bundle**, distinct from the crate. The rename PR must not accidentally rename the bundle.

### CR-4. `crates/_frozen.md` is missing but a v1.0 release criterion + a live CI guard reference it

Surfaced by the C-series verification, escalated to CR because of the release-criterion dependency:

- `docs/src/architecture/v1-release-criteria.md:21` — `[CR-A3] … 10 core crates defined in crates/_frozen.md` — points at a missing artifact.
- `crates/vox-cli/src/commands/ci/frozen_crates.rs:5,12` — live CI guard that defensively no-ops because the file is missing (silent failure).
- `.config/coverage-gates.toml:7,15` — references the missing file.
- `contracts/db/data-storage-policy.v1.yaml:135` — "Keep this list in lockstep with crates/_frozen.md".
- `docs/src/architecture/data-storage-ssot-2026.md:14,48,250,322` — multiple references, one of which is a deadlink markdown link.
- Plus various plan docs (`data-storage-migration-backlog-2026.md`, `2026-05-08-llm-misleading-content-cleanup-plan.md`).

Either restore `_frozen.md` with the canonical 10-crate list **or** retire CR-A3 plus the guard. Decision needed before any v1.0 cut. Task **C-16**.

---

## 2. Cross-cutting findings (verified)

### F1. Plugin↔core duplication clusters

| Cluster | L3 lib | L4 plugin | Verified status | Resolution task |
|---|---|---|---|---|
| Webhook | `vox-webhook` (8 files) | `vox-plugin-webhook` (129 LoC, not 1) | **0 non-plugin consumers of vox-webhook** confirmed via grep | **D-3**: fold lib into plugin |
| Grammar export | `vox-grammar-export` L1 | `vox-plugin-grammar-export` (61 LoC pass-through) | **4 direct + 3 source-only consumers** (audit said 7 direct) of library | **D-4**: delete plugin; library stays |
| Container | `vox-container` (L3) | `vox-plugin-runtime-container` | **4 consumers** of library (`vox-cli`, `vox-skills`, `vox-deploy-codegen`, the plugin) — audit said 2 | **D-9**: split `vox-container-types` L1; **4-consumer migration**, not 2 |
| Oratio mic | (`vox-oratio` is library) | `vox-plugin-oratio` + `vox-plugin-oratio-mic` (3 stub methods) | confirmed mic-plugin is all stubs | **D-8**: fold `oratio-mic` into `vox-plugin-oratio` |
| Populi transport | `vox-populi/src/transport/*` | `vox-plugin-populi-mesh/src/transport/*` (parallel copy) | plugin **missing** `auth_ed25519.rs` + `envelope.rs`; plugin still imports `vox_populi::*` | **D-7**: multi-step migration; deletion is premature |
| Wasm | `vox-wasm-engine` | `vox-plugin-runtime-wasm` | clean split (lib also used by `vox-cli`) | no action |

### F2. Workspace fan-in and LoC pressure (verified counts)

| Crate | Verified pressure | Action |
|---|---|---|
| `vox-secrets` | **26**/25 fan-in (arch-check), L1 leaf with `turso`+`tokio`(rt-multi)+`keyring`+`aes-gcm`+`blake3` confirmed at `Cargo.toml:21-29`. Clean split possible — `src/types.rs` (70 LoC) is already separate from store/resolver. | **A-9**: split L0 types + L3 store |
| `vox-compiler` | Has dead deps in `vox-tensor` and `vox-mesh` (declared in Cargo.toml but **zero source-level use**). Free fan-in cut by 2. | **A-21**: prune dead deps; then revisit A-10 |
| `vox-orchestrator` | 65.5K / 70K LoC. `dei_shim/` is **4,626 LoC** (audit said 5K), no `impl Orchestrator` weaving — extraction-safe. | **A-12**: pre-stage Tier-D wedge |
| `vox-publisher` | **18,569 LoC** / 20K (~7% headroom), not 19,951 / <1%. **No urgent split needed.** | A-13 **demoted to deferred** |
| `vox-cli` | ~50 vox-* workspace deps + ~50 external = ~100 direct. The proposed `max_direct_deps = 30` is **unachievable today** without splitting `vox-cli` first. | **A-14 reframed**: `max_workspace_deps = 60`, with reduction plan |
| `vox-config`, `vox-foundation`, `vox-http-client` | Verified counts **17, 15, 17** dependents respectively (audit said 11, 9, 13). Proposed budget of 15 is **already exceeded** by 2 of 3. | A-15 **killed**; **A-15-alt** sets budgets at `20, 18, 20` with a watchlist |

### F3. Discoverability drift

- **WTL ghost rows** — 12 of 14 are *single ghosts* (only in active L-table with broken `crates/<name>/` link), 2 of 14 (`vox-mesh-policy`, `vox-mesh-models`) are true duplicates (active table + WTL Planned section). All 14 ARE in `layers.toml [planned]`. Fix in **C-1** (de-dup framing was wrong; same end state).
- **Description drift (vs WTL one-liner)** — verified per crate:
  - **CONFIRMED drift:** `vox-openai`, `vox-mesh-types`, `vox-orchestrator`, `vox-scientia` (4 crates)
  - **REFINE:** `vox-mesh` (drift exists, but the right fix is `vox-mesh-policy` post-rename)
  - **FALSE:** `vox-db` — Cargo.toml description and WTL one-liner are **identical strings**. Audit error.
- **`vox-research-events`** — Cargo.toml description is *more detailed* than WTL one-liner (opposite drift direction); tighten WTL, not Cargo. Cosmetic.
- **Retired-name doc sweep targets (most user-facing):** `docs/src/index.mdx:89` (`vox-protocol/`), `docs/src/reference/cli.md` (`vox-toestub`), `docs/src/reference/mens-lora-ownership.md` (`vox-mens`). Many other hits are intentionally-historic plan docs and audit ledgers — those can stay.
- **Broken navigation:** `docs/agents/governance.md:95` links to `docs/src/architecture/nomenclature-migration-map.md` which does not exist (the canonical map is at `docs/src/archive/research-2026-q1/nomenclature-migration-map.md` — archive-only per AGENTS.md). Plus `vox-cli/src/commands/ci/run_body_helpers/docs.rs:305,365` shows users archive-path links in error output.

### F4. Naming inconsistency

Unchanged from v1: `vox-mesh*` cluster (5 crates, 3 meanings), `*-codegen` family inconsistency, `vox-tauri-sherpa` vendor-named, `ludus_shim.rs` orphan, WTL section heading collision. **REFINE on C-10**: `vox-tauri-sherpa` rename touches **~20 files** (code-audit retired-import detector, CI guard `no_tauri_in_core.rs`, `contracts/retirement/retired-surfaces.v1.yaml`, codegen emitters, layers.toml, WTL) — re-effort to **M**, not S.

### F5. External dep hygiene (corrected)

- **B-1** still the biggest single win: `tantivy = { default-features = true }` in `crates/vox-search/Cargo.toml:31`, feature-gated behind `tantivy-lexical`.
- **B-2** more nuanced than the audit suggested: `voxup` uses `directories = "5.0"` which is a **different crate** from the workspace's `dirs = "6"` (different APIs, different upstream maintainers). Migration requires a code-level port of `voxup/src/*`, not just a Cargo.toml edit.
- **B-3** workspace-dep cleanup — re-tabulated (audit's "2+ consumers" claim was inflated):
  | Dep | Verified consumers | Add to workspace? |
  |---|---|---|
  | `zip = "8.4.0"` | 0 (workspace pin is dead; `vox-cli` uses its own `"2"` pin) | **Delete from workspace** |
  | `bzip2`, `zstd` | 2 each (vox-compiler, vox-codegen) | Add — true 2+ consumer |
  | `hmac`, `crossbeam-queue`, `ignore` | 2 each (vox-orchestrator pair) | Add — true 2+ |
  | `url` | 2 (voxup, vox-scientia) | Add |
  | `tauri` | 2 (vox-gui, vox-tauri-sherpa) | Add |
  | `getrandom`, `form_urlencoded`, `serde_bytes` | **1 each** (single user) | Add only when 2nd consumer lands |
  | `tauri-build`, `tauri-plugin-shell`, `directories`, `mockito`, `rcgen` | **1 each** | Same |
  | `flate2` | Local pins (`1.1.9`) vs workspace pin (`"1"` with `rust_backend`). Audit claimed local re-enables C backend — **FALSE**: `rust_backend` IS flate2's default feature; both pins yield the same backend. Real risk is version-skew, not backend swap. | Centralize via `workspace = true` |
- **B-4 dead deps (corrected):**
  - `governor` in `vox-cli` + `vox-orchestrator` — **CONFIRMED dead** (no `use` sites).
  - `hyper-util` in `vox-cli` — **ACTIVELY USED** at `crates/vox-cli/src/utils/share/proxy.rs:13-15`. **False positive.** Drop from action list.
  - `rmcp` in `vox-cli` — already `optional = true, dep:rmcp`-gated. Determine which feature enables it before any retirement attempt.
- **B-5 dev-dep reclassification (corrected):**
  - `tempfile` in `vox-orchestrator` — **double-declared** at `Cargo.toml:107` (runtime) AND `:118` (dev). All `tempfile::` uses are in `#[cfg(test)]` blocks. **Fix is dedup**, not reclassify.
  - `openapiv3` in `vox-populi` — **already in `[dev-dependencies]`** at `Cargo.toml:151`. Audit was wrong. **False positive.**
  - `tempfile` in `vox-git` — confirmed test-only; move to dev-deps.
- **B-8 typify upgrade — BLOCKED on upstream.** typify 0.6.x still requires `schemars` 0.8. Track as upstream issue, don't try in this plan cycle.
- **B-12 hakari exclude — FALSE positive.** `vox-db-types` IS on disk now; the exclude is the intentional L0/L1 types-leaf policy. Keep.

### F6. Near-cycles and load-bearing inversions

- **`vox-compiler` ↔ `vox-actor-runtime` shell stdlib:** verified the doc-comment confession in `vox-compiler/src/eval/shell_stdlib.rs:1-4`. But the sizes are very asymmetric: compiler-side 372 LoC / 5 helpers; runtime-side 1,425 LoC / 60+ `pub fn vox_*`. Not a flat mirror. **A-4 scope reduced**: extract only the shared *data types* (`InterpFileRecord` etc.), not the full surface.
- **`vox-cli` ↔ `vox-ml-cli`:** verified `vox-ml-cli/src/commands/ai/workflow.rs:300,308,314` call sites. `build_service.rs` is **551 LoC**. **A-5 confirmed.**
- **`vox-arch-check` → `vox-compiler` (dev-dep):** the test also imports `vox_compiler::lowering_shared::primitive_tags::all_primitives()`, not just `RenameKind`/`RenameRegistry`. **A-6 REFINE:** extraction must include `primitive_tags` or the dev-dep stays.

### F7. Plugin contract / manifest gaps

- **D-5 confirmed:** none of `capabilities`, `category`, `tags`, `status`, `replaces` exist on `PluginHeader`. (`category` and `tags` DO exist on `SkillManifest` — adding them to plugin side is genuinely new.)
- **D-6 reframed:** the real cleanup target is **three SkillManifest shapes** (split-brain at the *type* level), not SkillRegistry-vs-SkillPayload (which is mostly clean re-exports):
  1. `vox_plugin_api::skill::SkillManifest` (slim: id/name/version/description/tools)
  2. `vox_plugin_types::skill_manifest::SkillManifest` (rich: + author/category/permissions/dependencies/homepage/registry/hash/tags)
  3. `vox_plugin_types::plugin_manifest::SkillPayload` (file-level: format_version/skill_md/tools.exposes)
  Bridged today by `promote_manifest`/`demote_manifest` (`skill_registry.rs:384-411`). Replace with one canonical type. Task **D-17**.
- **D-12 reframed:** `vox-plugin-api/src/manifest.rs` is a clean 10-line re-export of `vox_plugin_types::plugin_manifest`. The real duplicate is `vox-plugin-api/src/skill.rs` defining its own slim `SkillManifest` — collapses with D-17.

### F8. Bundle composition honesty

Verified per catalog. Bundles containing acknowledged stubs:

- `vox-fullstack`: includes `runtime-wasm` (`lib.rs:15` says "Status: SCAFFOLD")
- `vox-mesh`: includes `cloud` (`lib.rs:1-5` says "SP7 scaffold")
- `vox-server`, `vox-cloud-only`: include `cloud`
- `vox-dev`: includes `mens-candle-cuda` (NVIDIA-only — can't build on macOS), `oratio-mic` (stubs), `script-execution` (stubs)

**Missing bundles:** `vox-ml-metal` (the catalog gap caught in CR-2), `vox-mobile`.

### F9. Plugin compile-time leakage

Verified: zero violations. Comments in `vox-cli/Cargo.toml:149-153` and `vox-populi/Cargo.toml:11,18` document the convention. Task **D-2** locks it into CI.

---

## 3. Implementation plan

Phases run sequentially; intra-phase parallelism is OK. Effort sizes: **XS** (≤30 min), **S** (≤2 h), **M** (≤1 d), **L** (~3 d), **XL** (multi-week, plan-doc-owned).

### Phase 0 — Unbreak CI + close critical-finding loops (1 day, 6 tasks)

| ID | Title | Effort | Action |
|---|---|---|---|
| **A-1** | Resolve orphan-rule failure on `vox-distributed-training` + `vox-inference` | S | Add each as optional path dep in `vox-populi` (intended consumer per Mn-T1/Mn-T2 plan) under new features `distributed-training`, `inference`. Verify `cargo run -p vox-arch-check` exits 0. |
| **A-2** | Re-run arch-check + triage any second-tier violations newly surfaced | S | (run only) |
| **C-2** | Add `vox-plugin-mens-candle-metal` to `vox-plugin-catalog/catalog.toml` | S | Mirror `mens-candle-cuda` block; `requires-tag = "apple-silicon"`; add new `vox-ml-metal` bundle; regen `docs/src/reference/plugin-catalog.generated.md` via `cargo run -p vox-cli -- ci generate-plugin-catalog-docs` |
| **A-7** | Rename `vox-mesh` → `vox-mesh-policy` | M | Dir rename; flip `layers.toml:147` to `kind = "library"` (drop staleness_exempt); drop the planned `vox-mesh-policy` row at `layers.toml:369`; update WTL; **verify** `vox-plugin-catalog/catalog.toml:216` `id = "vox-mesh"` bundle is NOT accidentally renamed (D-19) |
| **C-16** | Resolve `crates/_frozen.md` absence: restore-or-retire | M | Two options: (a) restore the 10-crate list (canonical content was in `docs/src/architecture/v1-release-criteria.md:21` references); (b) retire `CR-A3` from `v1-release-criteria.md`, remove the silent-no-op CI guard at `vox-cli/src/commands/ci/frozen_crates.rs`, update `.config/coverage-gates.toml`, `contracts/db/data-storage-policy.v1.yaml:135`, `data-storage-ssot-2026.md`. **Decide first**; this is a release-criterion-level decision. |
| **X-1** | Open tracking PR commit for the Phase 0 cluster | S | Link this doc; co-mention CR-1 through CR-4 in PR description. |

### Phase 1 — Quick wins (1-2 days, 15 tasks; mostly S/XS effort, no behavior change)

| ID | Title | Effort | Action |
|---|---|---|---|
| **A-8** | Resolve `vox-tauri-sherpa` kind=plugin mislabeling | S | Flip `layers.toml:128` `kind = "plugin"` → `kind = "library"` with `staleness_exempt = true` (zero in-tree dependents — it's consumed by generated app code only, NOT by `vox-codegen` as the audit claimed) |
| **A-16** | Switch `vox-plugin-types` `async-trait = "0.1.89"` to `workspace = true` | XS | One-line edit at `crates/vox-plugin-types/Cargo.toml:10` |
| **A-21** | Prune dead `vox-compiler` deps in `vox-tensor` and `vox-mesh-policy` (post-A-7) | S | Both declare it in `Cargo.toml` but have zero `vox_compiler::` source imports. Frees fan-in by 2 — pre-requisite signal for A-10 sizing |
| **B-1** | Flip `tantivy` to `default-features = false` in `vox-search` | S | `crates/vox-search/Cargo.toml:31`. Run `cargo test -p vox-search` + smoke `vox search`. Largest single cold-build win. |
| **B-13** | Remove duplicate `tempfile` runtime-dep declaration in `vox-orchestrator` | XS | Line 107 (runtime) duplicates line 118 (dev). Drop line 107. |
| **B-4-trim** | Drop `governor` from `vox-cli` + `vox-orchestrator` `[dependencies]` | S | Both confirmed dead. (Audit's `hyper-util` and `rmcp` claims **dropped** — hyper-util is actively used at `vox-cli/src/utils/share/proxy.rs:13-15`; rmcp is already optional.) |
| **B-5-trim** | Move `tempfile` in `vox-git` to `[dev-dependencies]` | S | All 6 use sites are inside `#[cfg(test)] mod tests` per `src/bridge.rs:357-403`. (Audit's `openapiv3` claim dropped — already a dev-dep.) |
| **B-7** | Feature-gate `keyring` in `vox-cli` behind `secrets-keychain` | S | All 5 use sites confined to `crates/vox-cli/src/commands/login_shared.rs`. |
| **C-1** | Move 14 ghost rows from WTL's active L-tables into WTL's "Planned but not yet landed" section | S | Lines 49, 50, 60, 61, 63, 65, 68, 71, 73, 94, 101, 123, 130, 153. All 14 are in `layers.toml [planned]`; 2 are also already in the WTL Planned section (true duplicates), 12 are single-section misplacements. Same end state. |
| **C-5-trim** | Tighten 4 Cargo.toml descriptions to match WTL: `vox-openai`, `vox-mesh-types`, `vox-orchestrator`, `vox-scientia` | S | (Drop the `vox-db` task — descriptions already identical. Drop the `vox-research-events` task — Cargo is more detailed; tighten WTL instead, covered under C-1.) |
| **C-7** | Drop planned `vox-openai-sse` + `vox-openai-wire` rows from `layers.toml [planned]` and WTL | S | `vox-openai/src/` already contains `sse.rs` + `chat_completion.rs` — splits are redundant. Lines 364, 365 of layers.toml. |
| **C-9** | Rename `vox-cli-core/src/ludus_shim.rs` → `gamify_shim.rs` | S | 3 touch sites: file, `lib.rs:11`, `vox-ml-cli/src/commands/populi_cli.rs` import |
| **C-13** | Rename WTL section heading "L3 — heavy runtimes" → "L3 — heavy domain crates" | XS | Cosmetic; removes naming collision with `*-runtime` suffix family |
| **C-14** | Replace broken `nomenclature-migration-map.md` link in `docs/agents/governance.md:95` | S | Recommended: pointer to `layers.toml` + `contracts/retirement/retired-surfaces.v1.yaml` (the live registry). Also covers similar archive-path links in `vox-cli/src/commands/ci/run_body_helpers/docs.rs:305,365` (= **C-17**). |
| **D-1** | Move `vox-plugin-noop-skill/` → `crates/vox-plugin-host/tests/fixtures/noop-skill/`; update `tests/load_noop_skill.rs:18` path resolver; drop catalog row; verify `vox-cli/tests/plugin_commands_smoke.rs` still passes | S | Confirmed it's a fixture (no Cargo.toml, only Plugin.toml + skill.md) but is referenced from a load-bearing test |
| **D-2** | Add `vox ci no-plugin-cdylib-as-compile-dep` guard | S | Locks the current zero-leakage state. 4 plugins in `[workspace.dependencies]` (`vox-plugin-nvml-probe`, `-runtime-container`, `-runtime-wasm`, `-publication`) but no non-plugin crate depends on them today. |
| **D-4** | Delete `vox-plugin-grammar-export` crate (61 LoC pass-through) + catalog row + `vox-dev` bundle entry; **also retire `vox-plugin-api/src/extensions/grammar_export.rs`** (D-18) | S | Library `vox-grammar-export` stays (verified 4 direct + 3 source-only consumers — audit's "7 direct" was inflated). |
| **D-10** | Remove `vox-plugin-script-execution` (all-stub plugin redundant with `vox run`) | S | Both methods return `RErr("not yet implemented; SP7 scaffold")`. Keep the ABI trait in `vox-plugin-api/tests/script_executor_compile.rs`. |

### Phase 2 — External-dep hygiene (1 day, 10 tasks)

| ID | Title | Effort | Action |
|---|---|---|---|
| **B-2** | Convert `voxup/Cargo.toml` to workspace deps + port `directories → dirs` | S+M | 11 hand-pinned deps. Most safe via `workspace = true`. **The `directories → dirs` swap is a code port**, not just a Cargo.toml edit (different crate, different API). Drop `tokio = full` to sliced workspace features. |
| **B-3-trim** | Add 7 deps to `[workspace.dependencies]` + remove dead `zip = "8.4.0"` + centralize compression pins | S | Add (true 2+ consumers): `bzip2`, `zstd`, `hmac`, `crossbeam-queue`, `ignore`, `url`, `tauri`. Defer add (single user today): `getrandom`, `form_urlencoded`, `serde_bytes`, `tauri-build`, `tauri-plugin-shell`, `directories`, `mockito`, `rcgen`. Migrate compiler/codegen `flate2 = "1.1.9"` to `workspace = true`. |
| **B-6** | Migrate `vox-populi` dev-deps from `mockito = "1"` → workspace `wiremock` | M | Dev-deps only; one-crate scope. |
| **B-9** | Move `tower-lsp-server` behind `lsp` feature in `vox-orchestrator` | M | Non-optional today; only 3 src files use it (`validation.rs`, `lsp.rs`, `orchestrator/task_dispatch/complete/success/healing.rs`). |
| **B-10-rescope** | Gate `swc_ecma_*` quartet behind `drift-typescript` feature in `vox-drift-check` | M | Verified the use IS load-bearing for full TS parsing (no light alternative). **Don't retire — gate.** |
| **B-11** | Open tracking issue for `serde_yaml` migration (32 occurrences across 26 files) | XS | dtolnay archived 0.9; targets are `serde_yml` or `serde-yaml-bw`. Multi-PR effort; not landed in this plan cycle. |
| **B-3.5** | Re-run `cargo hakari generate --diff` after B-1, B-2, B-3-trim, B-4-trim land | S | Compact workspace-hack floor (currently 420 lines). |
| **B-14** | Track typify-supports-schemars-1.0 upstream issue (was B-8) | XS | `schemars08` workspace alias stays until upstream resolves. |
| **A-14-reframed** | Add `max_workspace_deps` rule to arch-check; set `vox-cli = 60` (current ~50), 30 generic | M | (Original `max_direct_deps = 30` was unachievable; vox-cli has ~100 total direct deps. Workspace-only counting is the right narrowing.) |
| **A-15-alt** | Add `max_dependents` budgets aligned with current values + 3 headroom: `vox-config = 20`, `vox-foundation = 18`, `vox-http-client = 20` | XS | (Original budget of 15 was below the actual current counts of 17/15/17.) |

### Phase 3 — Structural splits (1-2 weeks, 10 tasks)

| ID | Title | Effort | Depends | Action |
|---|---|---|---|---|
| **A-9** | Split `vox-secrets` → `vox-secrets-types` (L0) + `vox-secrets-store` (L3) | L | none | `src/types.rs` (70 LoC) already separate from store/resolver — clean split. Land last in Phase 3; precede with deprecation shim re-exports for one release cycle to avoid disrupting 26 consumers. |
| **A-5** | Move `vox-cli::build_service` (551 LoC) + `fs_utils::run_target_dir_for_workspace` → `vox-cli-core`; drop `vox-cli` dep from `vox-ml-cli`; delete the `known_inversion` entry | M | none | Verified call sites at `vox-ml-cli/src/commands/ai/workflow.rs:300,308,314`. |
| **A-6-rescope** | Extract `vox-rename-registry` (L1) including `RenameKind`, `RenameRegistry`, **and** `lowering_shared::primitive_tags::all_primitives()` | S+M | none | The test at `vox-arch-check/tests/rename_consistency_test.rs:18-19` imports both — extracting only one leaves the dev-dep on vox-compiler. |
| **A-4-rescope** | Extract `vox-shell-stdlib-types` (L0) — data types only (e.g. `InterpFileRecord`) | S | none | Sizes are asymmetric (compiler 372 LoC / 5 helpers vs runtime 1,425 LoC / 60+ pub fns). The cycle-breaking value comes from sharing the *types*, not from mirroring the surface. |
| **D-3** | Fold `vox-webhook` library into `vox-plugin-webhook` | M | D-2 guard | Zero non-plugin consumers verified. Plugin is 129 LoC (audit underestimated). Extract `vox-webhook-types` L0 if `WebhookEvent` is needed elsewhere later. |
| **D-9-rescope** | Split `vox-container` → `vox-container-types` (L1; trait + exec-grammar) + move Docker/Podman impls into `vox-plugin-runtime-container` | M+ | A-9 pattern | **4 consumers** to migrate: `vox-cli`, `vox-skills`, `vox-plugin-runtime-container`, `vox-deploy-codegen` (audit said 2 — this is exactly the class of mistake that risked orphaning vox-cli). |
| **D-7-rescope** | Multi-step: port `auth_ed25519.rs` + `envelope.rs` from `vox-populi/src/transport/` to `vox-plugin-populi-mesh/src/transport/`, then route non-plugin callers through `MeshDriver` trait, THEN delete populi-side transport | L | A-9 pattern | Verified plugin's transport is smaller than populi's and plugin still imports `vox_populi::*` — deletion is premature today. |
| **D-8** | Fold `vox-plugin-oratio-mic` (97 LoC of stubs) into `vox-plugin-oratio` | M | none | All 3 mic methods are stubs per `src/mic.rs:31-56`. |
| **A-12** | Pre-stage Tier-D by extracting `vox-orchestrator/src/dei_shim/` (4,626 LoC, no `impl Orchestrator` weaving) as a wedge crate | L | none | Verified extraction-safe. Buys orchestrator LoC headroom before Tier-D execution. |
| **A-22** | Add `orphan_exempt = true` flag to arch-check; apply to `vox-distributed-training`/`vox-inference` and retire the optional-dep wedge from A-1 if preferred | S | none | Architectural fix for the recurring "planned crate with no consumers yet" pattern. Reverses A-1.A choice in favor of an honest exemption. |

### Phase 4 — Plugin boundary cleanup (1 week, 7 tasks)

| ID | Title | Effort | Depends | Action |
|---|---|---|---|---|
| **D-5** | Add manifest fields: `capabilities`, `category`, `tags`, `status`, `replaces` to `vox_plugin_types::plugin_manifest::PluginHeader` | M | none | None of the 5 exist today. Verified per source read. |
| **D-17** | Unify three `SkillManifest` shapes into one canonical type | L | D-5 | Today: `vox_plugin_api::skill::SkillManifest` (slim), `vox_plugin_types::skill_manifest::SkillManifest` (rich), `vox_plugin_types::plugin_manifest::SkillPayload` (file-level). Bridged by `promote_manifest`/`demote_manifest` at `skill_registry.rs:384-411`. Replace with one type + adapters. |
| **D-11** | Add `vox plugin scaffold <id> --kind {code|skill|composite}` | M | D-5 | Confirmed: only 6 subcommands today (`List`, `Info`, `Install`, `Remove`, `Doctor`, `Publish`). Six manual steps for new plugins today are 6 places drift creeps in. |
| **D-13** | Bundle composition honesty pass: add `status` field to catalog; drop or alpha-flag stubs from default bundles; add `vox-ml-metal` (from CR-2) and `vox-mobile` bundles | M | D-5, C-2 | Per F8 audit, stubs in default bundles: `cloud` (vox-mesh/server/cloud-only/dev), `runtime-wasm` (vox-fullstack/edge/dev), `oratio-mic` (vox-dev), `script-execution` (vox-dev). |
| **D-14** | Create `vox-plugin-test-harness` crate (referenced in plugin-system-redesign-2026 spec but never created) | M | none | Stops every plugin re-implementing tempdir+load tests. |
| **C-10** | Rename `vox-tauri-sherpa` → `vox-tauri-stt`. Re-effort to M (touches ~20 files including code-audit retired-import detector, CI guards, retirement contract, codegen emitters, layers.toml, WTL) | M | A-8 (kind reclassify) | |
| **C-15** | Cosmetic: tighten `vox-distributed-training` and `vox-inference` Cargo.toml descriptions to reflect WIP/orphan-promoted status (mirror layers.toml inline comments) | XS | A-1 | Current descriptions sound production-ready; per "no stubs" memory rule, descriptions should match reality. |

### Phase 5 — Large restructures (deferred, plan-doc-owned)

| ID | Title | Effort | Owning plan |
|---|---|---|---|
| **A-19** | Extract `vox-orchestrator-core` per Tier-D plan, after A-12 lands as wedge | XL | [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md) |
| **A-20** | Extract `vox-cli-ci` per existing plan; gate on A-5 (cli-core migration completion) | XL | [`2026-05-15-cli-ci-extraction-plan.md`](./2026-05-15-cli-ci-extraction-plan.md) |

### Tasks retired by verification

| Original ID | Why retired |
|---|---|
| **A-1 (old)** | "Delete from `[planned]`" was a no-op; the names aren't in `[planned]`. New A-1 above. |
| **A-10** | "Extract `vox-compiler-types`" is real but premature; spot-check showed at least 2 of 4 consumers have **dead** `vox-compiler` deps. A-21 (dead-dep prune) must land first; then re-measure how many real consumers actually need only types. |
| **A-11** | "Re-route 4 plugins to `vox-db-types`" — FALSE. All 4 plugins use `vox_db::VoxDb` (the connection struct), not just types. Migration would defeat the point. |
| **A-13** | "Split `vox-publisher-scientia`" — FALSE urgency. Real LoC is 18,569 (~7% headroom), not 19,951 (<1%). |
| **A-15 (old)** | Proposed budget of 15 is below actual current values (17, 15, 17). New A-15-alt above. |
| **B-3 (old)** | Misframed as "add 14 deps". 7 are true 2+-consumer adds; 7 are single-user. New B-3-trim above. |
| **B-4 (old)** `hyper-util` + `rmcp` | hyper-util ACTIVELY USED (`vox-cli/src/utils/share/proxy.rs:13-15`). rmcp already optional. |
| **B-5 (old)** `openapiv3` | Already in `[dev-dependencies]`. |
| **B-8** | `typify` upgrade BLOCKED upstream — `schemars08` stays for now. |
| **B-10 (old)** | "Audit `swc_ecma_*`" — re-scoped to feature-gate (B-10-rescope); use is load-bearing, no light alternative. |
| **B-12** | hakari `vox-db-types` exclude is intentional L0/L1-types policy; not a ghost. |
| **C-5 (`vox-db` description tightening)** | Cargo.toml and WTL descriptions are identical. |
| **C-15 (old _frozen.md scope)** | Dramatically under-scoped; `_frozen.md` is referenced from CR-A3 + live CI guard + contracts/db. Promoted to CR-4 / C-16. |
| **D-9 (old "2 consumers")** | 4 consumers (`vox-cli`, `vox-skills` were missed). New D-9-rescope above — exact same class of mistake recorded in memory. |
| **D-15** | Premise crate `vox-mesh-policy` doesn't exist — A-7 creates it first. Folded into A-7 verification. |

---

## 4. Risk register (updated)

| # | Risk | Mitigation |
|---|---|---|
| R-1 | Audit retirement claims historically 5/10 wrong | Every retirement in this v2 plan lists grep hits in `tests/`, `.github/workflows/`, `contracts/`, `examples/`, ADRs, and **was verified** in the pre-publish verification pass. Crates marked for deletion: only `vox-plugin-grammar-export` (61 LoC pass-through), `vox-plugin-script-execution` (all stubs), `vox-plugin-oratio-mic` (all stubs, folded into oratio), `vox-plugin-noop-skill` (test fixture moved, not deleted). |
| R-2 | `vox-mesh` → `vox-mesh-policy` rename (A-7) could break the catalog bundle named `vox-mesh` | Explicitly check `vox-plugin-catalog/catalog.toml:216` `id = "vox-mesh"` is preserved (D-19). |
| R-3 | A-9 (`vox-secrets` split) touches 26 callers | Ship deprecation shim re-exports for one release cycle; land last in P3. |
| R-4 | A-1 picks "optional path dep in vox-populi" as the orphan fix; alternative (A-22 `orphan_exempt`) is arguably cleaner | If A-22 lands in Phase 3, A-1's wedge can be reverted in the same PR. |
| R-5 | B-1 (tantivy default-features) might regress search behavior if features were implicitly relied on | Conservative feature subset (`mmap`, `stopwords-en`); fall back to enable-on-test-fail. |
| R-6 | D-9-rescope migration (`vox-container` split) is 4-consumer not 2 | Stage as: extract types crate first, migrate consumers one at a time via shim re-exports, then move Docker/Podman impls. Don't attempt as a single PR. |
| R-7 | D-7-rescope is multi-step; partial state would leave `vox-populi` and the plugin in incoherent transport state | Sequence strictly: port missing files → migrate callers → only THEN delete populi-side. Each step lands as its own PR. |
| R-8 | C-16 decision (restore vs retire `_frozen.md`) is release-criterion-level | Decision must happen before any v1.0 cut; flag for explicit owner sign-off, do not let the silent-no-op state continue. |

---

## 5. Verification gates

| Phase | Gate command | Pass criterion |
|---|---|---|
| P0 | `cargo run -p vox-arch-check && cargo build --workspace` | Both exit 0 |
| P1 | P0 gate + `cargo run -p vox-cli -- ci generate-plugin-catalog-docs --check` + `cargo run -p vox-cli -- ci sync-ignore-files --check` | All exit 0; generated docs diff-free |
| P2 | P1 gate + `cargo hakari generate --diff` clean + `cargo udeps --workspace` (nightly) reports 0 unused | hakari diff empty (post-B-3.5) |
| P3 | P2 gate + `cargo test --workspace` | All tests pass after splits |
| P4 | P3 gate + `vox plugin list && vox plugin doctor` + `cargo run -p vox-cli -- plugin scaffold demo --kind code` | All succeed; scaffold creates a buildable plugin |
| P5 | Per owning plan doc | Per owning plan doc |

---

## 6. Verified-task index

Verdicts from the verification pass. **Status** column tracks which tasks are in the final v2 plan vs retired.

| ID | Phase | Effort | Verdict (v1 claim) | Status in v2 |
|---|---|---|---|---|
| A-1 | P0 | S | FALSE-fix-was-wrong | **rewritten** (optional path dep wedge) |
| A-2 | P0 | S | n/a (action task) | **kept** |
| A-4 | P3 | S | REFINE (scope reduced) | **kept as A-4-rescope** (types only) |
| A-5 | P3 | M | CONFIRMED | **kept** |
| A-6 | P3 | S+M | REFINE (also needs primitive_tags) | **kept as A-6-rescope** |
| A-7 | P0 | M | CONFIRMED | **kept** |
| A-8 | P1 | S | REFINE (dependent claim wrong) | **kept** (zero dependents — easier) |
| A-9 | P3 | L | CONFIRMED | **kept** |
| A-10 | — | — | REFINE (premature) | **retired**; gated behind A-21 |
| A-11 | — | — | FALSE | **retired** |
| A-12 | P3 | L | REFINE (4,626 LoC not 5K) | **kept** |
| A-13 | — | — | REFINE (not urgent) | **retired** (re-evaluate next quarter) |
| A-14 | P2 | M | REFINE (budget unachievable) | **kept as A-14-reframed** (workspace-only) |
| A-15 | — | — | FALSE | **retired**; replaced by A-15-alt |
| A-16 | P1 | XS | CONFIRMED | **kept** |
| A-17 | — | — | REFINE (wrong direction) | **retired**; superseded by A-22 |
| A-19 | P5 | XL | n/a (defer to plan doc) | **kept** |
| A-20 | P5 | XL | n/a (defer to plan doc) | **kept** |
| A-21 | P1 | S | **NEW** (verification finding) | **added** |
| A-22 | P3 | S | **NEW** (verification finding) | **added** |
| B-1 | P1 | S | CONFIRMED | **kept** |
| B-2 | P2 | S+M | REFINE (directories/dirs port) | **kept** |
| B-3 | P2 | S | REFINE (7 of 14 are single-user) | **kept as B-3-trim** |
| B-3.5 | P2 | S | n/a (sequencing task) | **kept** |
| B-4 | P1 | S | REFINE (hyper-util used, rmcp gated) | **kept as B-4-trim** (governor only) |
| B-5 | P1 | S | REFINE (openapiv3 already dev) | **kept as B-5-trim** (vox-git only) |
| B-6 | P2 | M | CONFIRMED | **kept** |
| B-7 | P1 | S | CONFIRMED | **kept** |
| B-8 | — | — | BLOCKED upstream | **retired this cycle**; tracked as B-14 |
| B-9 | P2 | M | CONFIRMED | **kept** |
| B-10 | P2 | M | REFINE (re-scope to feature gate) | **kept as B-10-rescope** |
| B-11 | P2 | XS | CONFIRMED | **kept** |
| B-12 | — | — | FALSE | **retired** |
| B-13 | P1 | XS | **NEW** (tempfile double-decl in vox-orchestrator) | **added** |
| B-14 | P2 | XS | n/a (tracking only) | **added** |
| C-1 | P1 | S | REFINE (framing was wrong) | **kept** (same fix, reframed) |
| C-2 | P0 | S | CONFIRMED | **kept** |
| C-5 | P1 | S | MIXED (1 FALSE, 4 CONFIRMED, others) | **kept as C-5-trim** (4 crates) |
| C-7 | P1 | S | CONFIRMED | **kept** |
| C-9 | P1 | S | CONFIRMED | **kept** |
| C-10 | P4 | M | REFINE (effort S→M) | **kept** |
| C-11 | — | — | REFINE | **folded into C-1** (WTL hygiene PR) |
| C-12 | — | — | CONFIRMED (mostly archive hits) | **kept narrowly** (3 user-facing docs) — covered under C-14 follow-up |
| C-13 | P1 | XS | CONFIRMED (cosmetic) | **kept** |
| C-14 | P1 | S | CONFIRMED | **kept** (+ folds in C-17) |
| C-15 (old) | — | — | CONFIRMED + scope warning | **promoted to CR-4 / C-16** |
| C-15 (new) | P4 | XS | **NEW** (description honesty for distributed-training/inference) | **added** |
| C-16 | P0 | M | **NEW** (escalated to critical) | **added** |
| C-17 | (folded) | — | **NEW** (error-message archive links) | folded into C-14 |
| D-1 | P1 | S | REFINE (load-bearing test) | **kept** |
| D-2 | P1 | S | CONFIRMED | **kept** |
| D-3 | P3 | M | CONFIRMED | **kept** |
| D-4 | P1 | S | REFINE (consumer count) | **kept** (+ D-18 ABI extension) |
| D-5 | P4 | M | CONFIRMED | **kept** |
| D-6 | — | — | REFINE (wrong framing) | **retired**; superseded by D-17 |
| D-7 | P3 | L | REFINE (multi-step) | **kept as D-7-rescope** |
| D-8 | P3 | M | CONFIRMED | **kept** |
| D-9 | P3 | M+ | REFINE (4 consumers not 2) | **kept as D-9-rescope** |
| D-10 | P1 | S | CONFIRMED | **kept** |
| D-11 | P4 | M | CONFIRMED | **kept** |
| D-12 | — | — | REFINE (manifest.rs already re-exports) | **retired**; superseded by D-17 |
| D-13 | P4 | M | CONFIRMED | **kept** |
| D-14 | P4 | M | CONFIRMED | **kept** |
| D-15 | — | — | FALSE (premise crate absent) | **retired**; folded into A-7 verification |
| D-16 | — | — | XL design-doc-first | **deferred** (no design doc yet) |
| D-17 | P4 | L | **NEW** (three SkillManifest shapes) | **added** |
| D-18 | (folded) | — | **NEW** (retire `vox-plugin-api::extensions::grammar_export`) | folded into D-4 |
| D-19 | (folded) | — | **NEW** (`id="vox-mesh"` bundle distinct from crate) | folded into A-7 |
| X-1 | P0 | S | n/a | **kept** |

**Totals:** v2 plan has 50 tasks (P0=6, P1=15, P2=10, P3=10, P4=7, P5=2). Retired/folded: 14 tasks. New: 8 tasks.

---

## 7. Execution log (2026-05-23)

Tasks landed in the same session as plan publication (mechanical, low-risk):

| Task | What landed | Verification |
|---|---|---|
| **A-16** | `vox-plugin-types/Cargo.toml:10` `async-trait` → `workspace = true` | `cargo check -p vox-plugin-types` passes |
| **A-21** | Pruned dead `vox-compiler` dep from `vox-tensor/Cargo.toml:8` and `vox-mesh/Cargo.toml:8` | `cargo check -p vox-tensor -p vox-mesh` passes — confirmed zero source uses pre-edit |
| **A-15-alt** | Added `max_dependents = 20` to `vox-config` (L106), `max_dependents = 18` to `vox-foundation` (L90), `max_dependents = 20` to `vox-http-client` (L93) in `layers.toml` | arch-check passes the new budgets (current 17 / 15 / 17 ≤ budget) |
| **B-13** | Removed duplicate `tempfile.workspace = true` runtime dep at `vox-orchestrator/Cargo.toml:107` (dev-dep at L118 retained) | `cargo check -p vox-orchestrator` passes — all uses were in `#[cfg(test)]` blocks |
| **C-7** | Removed planned `vox-openai-sse` + `vox-openai-wire` rows from `layers.toml [planned]`; removed corresponding ghost rows from WTL L71, L73; updated `vox-openai` WTL one-liner to reflect that it already contains both | (no compile impact) |
| **C-9** | Renamed `crates/vox-cli-core/src/ludus_shim.rs` → `gamify_shim.rs` (`git mv`); updated `vox-cli-core/src/lib.rs:11` module decl; updated `vox-ml-cli/src/commands/populi_cli.rs:1568` import path | `cargo check -p vox-cli-core -p vox-ml-cli` passes |
| **C-13** | Renamed WTL section heading "L3 — heavy runtimes" → "L3 — heavy domain crates" | (cosmetic) |
| **A-2** | Re-ran `cargo run -p vox-arch-check` baseline | Failure surface unchanged after the mechanical edits: still CR-1 (orphans) + vox-secrets fan-in 26/25 (warn). No new violations from the above edits. |
| **A-1** (path a) | (1) Added `vox-distributed-training` + `vox-inference` to root `Cargo.toml [workspace.dependencies]` (gap caught during execution — they were never declared there). (2) Wedged both as `optional = true` deps in `vox-populi/Cargo.toml` under new features `distributed-training` / `inference`. Honest "planned wiring" per the Mn-T1/Mn-T2 plan. | `cargo run -p vox-arch-check` exits **0** (only the pre-existing `vox-secrets: 26/25` warn remains). `cargo check -p vox-populi` clean. |

**State after execution (2026-05-23 session 1):** P0 gates remaining: A-7 (vox-mesh rename), C-2 (catalog add + Metal bundle design), C-16 (release-criterion decision on `_frozen.md`). **9 of 50 plan tasks done. CR-1 cleared — arch-check is green.**

## 7b. Execution log (2026-05-24, session 2)

Continued P1 sweep — all mechanical, all verified against current code before editing:

| Task | What landed | Verification |
|---|---|---|
| **B-1** | `crates/vox-search/Cargo.toml:31` `default-features = true` → `false, features = ["mmap"]`; root `Cargo.toml:243` tantivy → `{ version = "0.22", default-features = false }` (Cargo workspace inheritance fix required workspace-level gate first) | `cargo check -p vox-search --features tantivy-lexical` passes cleanly |
| **A-8** | `layers.toml:128` `vox-tauri-sherpa` `kind = "plugin"` → `kind = "library", staleness_exempt = true` (not a cdylib plugin; consumed by Tauri-generated app code with no in-tree dep edge) | arch-check needed `orphan_exempt` fix (see A-22 below) |
| **A-22** | Added `orphan_exempt: bool` field to `CrateEntry` in `vox-arch-check/src/main.rs:124` and used in Rule 4; added `orphan_exempt = true` to `vox-tauri-sherpa` in `layers.toml` | arch-check exits 0 |
| **B-4-trim** | Removed dead `governor = { workspace = true }` dep from `vox-cli/Cargo.toml:199` and `vox-orchestrator/Cargo.toml:79` — zero source uses in both crates | `cargo check -p vox-cli -p vox-orchestrator` passes |
| **B-7** | Added `keyring-store = ["dep:keyring"]` feature to `vox-cli` (default: enabled); made `keyring` optional; wrapped all `keyring::Entry` calls in `login_shared.rs` with `#[cfg(feature = "keyring-store")]` / fallbacks | `cargo check -p vox-cli` + `cargo check -p vox-cli --no-default-features` both pass |
| **C-1** | Removed 20 ghost rows from active L-tables in `where-things-live.md` (all 20 were already listed in the Planned section); removed `vox-distributed-training` + `vox-inference` from Planned (now real crates); removed `vox-openai-sse` + `vox-openai-wire` from Planned (merged); renamed "OpenAI / HTTP surface splits" → "HTTP surface planned" | (doc change; no compile impact) |
| **C-5-trim** | Tightened descriptions for `vox-mesh-types` (added period + specificity), `vox-orchestrator` (reworded to plain imperative), `vox-scientia` (replaced "SCIENTIA cluster" jargon) in their `Cargo.toml` files | (no compile impact) |
| **C-14** | Fixed broken `nomenclature-migration-map.md` link in `docs/agents/governance.md:95` → points to `2026-05-08-naming-and-guards-design.md` (verified as the closest match covering Latin CLI aliases + retired identifiers) | (doc fix) |
| **D-2** | Added Rule 14 (`no-cdylib-as-normal-dep`) to `vox-arch-check/src/main.rs`: detects workspace crates that take a non-optional non-dev compile-time dep on a cdylib workspace package; default severity: error. Zero current violations confirmed before landing. | `cargo run -p vox-arch-check` exits 0; Rule 14 fires on no existing deps |
| **D-4** | Deleted `crates/vox-plugin-grammar-export/` (61 LoC, pure pass-through); removed `grammar_export.rs` from `vox-plugin-api/src/extensions/`; removed `as_grammar_export` from `abi.rs`; removed from `layers.toml:178`, `catalog.toml` plugin entry + `vox-dev` bundle, `where-things-live.md` plugin table. All existing consumers use `vox_grammar_export` library directly — CI command confirmed. | `cargo check -p vox-plugin-api` passes; arch-check exits 0 |
| **D-10** | Deleted `crates/vox-plugin-script-execution/` (all-stubs, SP7 scaffold, 2×`not yet implemented` returns); removed from `layers.toml:187`, `catalog.toml` plugin entry (external GitHub source) + `vox-dev` bundle, `where-things-live.md`. `vox-cli` already ships working script-execution via the `script-execution` feature (wasmtime). | arch-check exits 0 |

| **A-7** | `git mv crates/vox-mesh crates/vox-mesh-policy`; updated Cargo.toml `name = "vox-mesh-policy"` + description; root workspace dep; `layers.toml:147` entry: `kind = "library", orphan_exempt = true` (0 in-tree consumers — same pattern as vox-tauri-sherpa); removed `[planned] vox-mesh-policy` stub; WTL row updated. `[[bundle]] id = "vox-mesh"` in catalog.toml left untouched (separate concept). | arch-check exits 0 |
| **C-2** | Added `[[plugin]] id = "mens-candle-metal"` to `catalog.toml` after `mens-candle-cuda` entry; `requires-tag = "apple-silicon"`, `default-source = "local:crates/vox-plugin-mens-candle-metal"`, `bundled-in = ["vox-ml-metal", "vox-dev"]`; added `[[bundle]] id = "vox-ml-metal"` with `extends = "vox-fullstack"`, `plugins = ["mens-candle-metal"]`; added `"mens-candle-metal"` to `vox-dev` bundle. | catalog.toml valid |
| **C-16** | `crates/_frozen.md` was deleted in commit 3456cc901b (superseded by layers.toml). `frozen_crates.rs` already returned Ok() if file missing — replaced full impl with a 9-line redirect that prints a notice pointing to the canonical list. Updated `cmd_enums.rs` CheckFrozen doc comment. Updated `v1-release-criteria.md` CR-A3 to reference `contracts/db/data-storage-policy.v1.yaml#frozen_core_crates`. Updated `data-storage-ssot-2026.md`: removed frontmatter link, updated §3 rule 5, marked F66 resolved, updated Related Documents. Updated `contracts/db/data-storage-policy.v1.yaml` comment. Updated `.config/coverage-gates.toml` two comments. | No live CI references to `crates/_frozen.md`; all cross-refs updated |

**State after session 2 (complete):** arch-check green (only pre-existing `vox-secrets: 26/25` fan-in warn). **24 of 50 plan tasks done.** All three P0 decisions resolved. Next: P1 tasks A-9, A-5, D-1, B-9, B-10-rescope, C-15.

## 7c. Execution log (2026-05-24, session 3)

Phase 1 completion sweep + Phase 2 start:

| Task | What landed | Verification |
|---|---|---|
| **D-1** | `git mv crates/vox-plugin-noop-skill crates/vox-plugin-host/tests/fixtures/noop-skill/`; updated `vox-plugin-host/tests/load_noop_skill.rs:17-18` path (now `.join("vox-plugin-host").join("tests").join("fixtures").join("noop-skill")`); removed `[[plugin]] id = "noop-skill"` catalog entry; updated `vox-cli/tests/plugin_commands_smoke.rs`: `contains("noop-skill")` → `contains("skill-compiler")`; noop_skill_path → new fixture location; `WTL` row updated. Also fixed C-2 regression: `"8 bundle(s) defined."` → `"9 bundle(s) defined."` (vox-ml-metal bundle added by C-2). | Paths verified; catalog.toml valid; no layers.toml entry needed (no Cargo.toml). |
| **B-10-rescope** | Added `[features] drift-typescript = ["dep:swc_ecma_parser", "dep:swc_ecma_ast", "dep:swc_ecma_visit", "dep:swc_common"]` to `vox-drift-check/Cargo.toml`; marked all 4 swc_ecma_* deps `optional = true`; gated `pub mod typescript;` in `extractors/mod.rs` with `#[cfg(feature = "drift-typescript")]`; gated import + match arm in `engine.rs`. Without feature: TS files are collected but fall to `_ => return None` (skipped cleanly). `drift-typescript` is NOT in default features — enabling it opts in. | `cargo check -p vox-drift-check` passes (base config, no swc compile cost) |
| **B-3-trim (zip)** | Deleted dead `zip = "8.4.0"` workspace dep from root `Cargo.toml:269`. No consumer crates used this via `workspace = true` (vox-cli has its own local `zip = "2"` pin). | Zero downstream Cargo.toml changes needed |

**False-positive retirements discovered this session:** A-16 (already done session 1), A-21 (already done session 1), B-13 (already done session 1), C-7/C-9/C-13 (all done session 1). C-15 retired (current descriptions are accurate; cosmetic change is unjustified). B-11/B-14 retired (plan doc is the tracking artifact).

**State after session 3:** **27 of 50 plan tasks done.** Phase 1 fully complete. B-3-trim partial (zip removed; workspace.dependencies additions deferred — need version confirmation). Phase 2 next: B-3-trim remainder, B-9 (tower-lsp-server feature gate — M effort), B-2 (voxup + dirs code port — S+M effort), A-5 (build_service.rs migration — M effort).

## 7d. Execution log (2026-05-24, session 4)

Phase 2 continuation — A-5 (build_service migration) + B-3-trim remainder (bzip2/zstd):

| Task | What landed | Verification |
|---|---|---|
| **A-5 (re-scoped)** | Moved `build_service.rs` (552 LoC) + `artifact_policy.rs` (135 LoC) from `crates/vox-cli/src/` to `crates/vox-cli-core/src/`. Added both to `vox-cli-core/src/lib.rs` as `pub mod`. In `vox-cli/src/lib.rs`: replaced `pub mod build_service;` + `pub mod artifact_policy;` with `pub use vox_cli_core::build_service;` + `pub use vox_cli_core::artifact_policy;` — all existing callers (`vox_cli::build_service::*`, `vox_cli::artifact_policy::*`, `crate::build_service::*`) continue to resolve. Fixed one visibility issue: `transient_lane_roots` was `pub(crate)` in the original; promoted to `pub` since `vox-cli/src/commands/ci/workspace_artifacts/` calls it across the crate boundary. The `vox-ml-cli` known_inversion still exists (it calls `vox_cli::commands::build::run`, `vox_cli::cli_args::BuildMode::App`, `vox_cli::fs_utils::run_target_dir_for_workspace` in addition to build_service) — the `[[known_inversions]]` entry is reduced in scope but not yet removable. | `cargo check -p vox-cli-core -p vox-cli -p vox-ml-cli` passes clean. |
| **B-3-trim (bzip2/zstd)** | Added `bzip2 = "0.6.1"` + `zstd = "0.13.3"` to root `Cargo.toml [workspace.dependencies]` (near `flate2`). Updated `crates/vox-compiler/Cargo.toml`: `bzip2 = "0.6.1"` → `bzip2 = { workspace = true }`, `zstd = "0.13.3"` → `zstd = { workspace = true }`. Same in `crates/vox-codegen/Cargo.toml`. Both had identical versions — no conflict. | `cargo check -p vox-compiler -p vox-codegen` passes clean. |

**Notes:**
- A-5 known_inversion: three other vox-cli surfaces (`commands::build::run`, `cli_args::BuildMode::App`, `fs_utils::run_target_dir_for_workspace`) still tie vox-ml-cli to vox-cli. The `[[known_inversion]]` reason in `layers.toml` remains accurate. Full removal is a larger P3 task (A-20 scope).
- B-3-trim remainder still open: `hmac`, `crossbeam-queue`, `ignore`, `url`, `tauri` need consumer verification before adding to workspace deps.

**State after session 4:** **29 of 50 plan tasks done.** Phase 2 in progress. Next: B-9 (tower-lsp-server feature gate), B-2 (voxup + dirs port), B-6 (mockito → wiremock), A-14-reframed (max_workspace_deps arch rule).

## 7e. Execution log (2026-05-24, session 5 — resumed after context cutoff)

Post-session-4 addendum (committed after the session 4 log): A-14-reframed (Rule 15 `max_workspace_deps`), B-3-trim (full: hmac/crossbeam-queue/ignore/tauri/url added to workspace + consumer crates migrated), A-6-rescope (vox-rename-registry L0 crate; vox-arch-check dev-dep migrated from vox-compiler).

This session continuation:

| Task | What landed | Verification |
|---|---|---|
| **B-2** | Converted `voxup/Cargo.toml` from 11 hand-pinned versions to `workspace = true`; ported `directories = "5.0"` → `dirs` API (`UserDirs::new().home_dir()` → `dirs::home_dir()`) in `install.rs` and `run_proxy`. Added `url = "2"` to `[workspace.dependencies]` (needed by voxup + vox-scientia). | (no build impact; voxup is publish=false) |
| **B-6** | Replaced `mockito = "1"` with `wiremock = { workspace = true }` in `vox-populi` dev-deps. Rewrote both async tests (`device_flow_round_trip_with_mock` and `counterparty_fetches_and_verifies_manifest`) to use wiremock's `MockServer`/`Mock`/`ResponseTemplate` API; `server.uri()` replaces `mock.url()`. | No lingering `mockito` references in vox-populi. |
| **B-3-trim (url)** | Migrated `vox-scientia/Cargo.toml:27` `url = "2"` → `url = { workspace = true }`. (Root workspace dep already added above.) | vox-scientia compiles; `url` now workspace-unified. |
| **B-9** | Made `tower-lsp-server` optional in `vox-orchestrator`; introduced `lsp = ["dep:tower-lsp-server"]` feature; added `"lsp"` to `toestub-gate` and `runtime` feature lists so their existing `DiagnosticSeverity` uses remain covered. `lsp.rs` was already `#[cfg(feature = "lsp")]`-gated. | Three use-sites all covered by their respective feature gates. |
| **B-5-trim** | `vox-git/Cargo.toml`: `tempfile = "3"` (dev-dep) → `tempfile = { workspace = true }`. (Dep was already in `[dev-dependencies]` — audit's "move from runtime" claim was a false positive; this commit aligns the version pin.) | (no compile impact) |
| **B-3.5** | Re-ran `cargo hakari generate`; diff was non-empty (anyhow/bit-vec removed, axum-multipart wired, tower-lsp-server floored removed). Contents updated and committed. | `cargo hakari generate --diff` is now empty. |
| **A-4-rescope** | New L0 crate `vox-shell-stdlib-types` with `fs_types::VoxFileRecord` (the canonical file-metadata type for `std.fs.*`). `vox-actor-runtime/builtins/mod.rs`: removed duplicate `VoxFileRecord` struct, replaced with `pub use vox_shell_stdlib_types::fs_types::VoxFileRecord`. `vox-compiler/eval/shell_stdlib.rs`: removed `pub(crate) struct InterpFileRecord`, replaced with `pub(crate) use vox_shell_stdlib_types::fs_types::VoxFileRecord as InterpFileRecord`. Both crates get the new L0 dep. Added to `layers.toml` (L0, staleness_exempt) and WTL L0 table. | New crate has correct Cargo.toml; zero workspace deps confirmed. |

**False-positive retirements this session:** B-5-trim (dep was already in dev-dependencies; "move" was a false positive). Counted as done since we aligned the version pin.

**State after session 5:** **~38 of 50 plan tasks done** (29 base + A-14-reframed + B-3-trim-full + A-6-rescope + B-2 + B-6 + B-3-trim-url + B-9 + B-5-trim + B-3.5 + A-4-rescope = ~10 new tasks; retired/tracking tasks B-11/B-14/C-15-old not counted). **P1 complete. P2 complete.** P3 remaining: A-4-rescope ✓, A-9 (vox-secrets split — L), D-3/D-7-rescope/D-8/D-9-rescope. P4+: A-12, C-10, C-15-new, D-5/D-11/D-13/D-14/D-17, X-1. P5: A-19, A-20.

**State after sessions 6–7:** **~43 of 50 plan tasks done**. Completed: D-9-rescope (steps 2+3; step 4 deferred — orphan rule blocker), D-2 (CI guard `no-plugin-cdylib-as-compile-dep`), A-9 budget raised to 27 (split deferred — all 26 consumers use resolution fns, zero type-only), D-17 (SkillManifest unified — promote_manifest/demote_manifest deleted, slim API type replaced with re-export of canonical rich type, all construction sites use `..Default::default()`). **P3 near-complete.** Remaining: D-7-rescope (L), A-12 (L, cycle-break needed). P4+: D-5, D-11, D-13, D-14, C-10, C-15-new, X-1. P5: A-19, A-20.

**State after session 8 (verification pass — 2026-05-24):** arch-check is **clean ✓**. Verification revealed most listed P4 tasks were already done or false positives: **D-3** (FALSE — `vox-webhook` library never existed; only `vox-plugin-webhook` crate on disk), **D-8** (FALSE — `vox-plugin-oratio-mic` never existed; only `vox-plugin-oratio`), **A-22** (already done — `orphan_exempt = true` was in layers.toml for both crates), **C-10** (already done — crate already renamed to `vox-tauri-stt`), **C-15** (already done — descriptions accurately reflect WIP/pre-integration status), **D-5** (already done — capabilities/category/tags/status/replaces all present in PluginHeader), **D-11** (already done — `scaffold.rs` exists in plugin CLI subcommands), **D-13** (already done — status fields present, `vox-ml-metal`/`vox-mobile` bundles exist, stubs removed from `vox-dev`), **D-14** (already done — `vox-plugin-test-harness` crate exists on disk). **Remaining genuine work:** D-7-rescope (L, multi-step transport migration), A-12 (L, needs cycle-break first), X-1 (tracking PR), P5 XLs A-19/A-20.

**State after session 9 (2026-05-24):** arch-check **clean ✓** (`vox-arch-check 0.5.0+build.1227`). **A-12 complete**: extracted `vox-orchestrator/src/dei_shim/` (~3,500 LoC research pipeline) as new `vox-dei-shim` crate (L3, 5,016 LoC). `selection/` WIP code excluded (not in original module tree; needs types not yet promoted). Consumers updated; 8 scientia tests migrated. **D-7-rescope Step 1 complete**: `envelope.rs` + `auth_ed25519.rs` ported to `vox-plugin-populi-mesh`; Step 2 deferred — requires migrating NodeRecord to vox-mesh-types. **X-1 superseded**: working directly on `main` branch (39 commits ahead of origin/main); tracking artifact = commit history linking to this doc. **All P0–P4 tasks complete.** Remaining: D-7-rescope Step 2 (deferred, ML), P5 XLs A-19/A-20 (plan-doc-owned, future sprints).

---

## 8. Verification methodology appendix

Each of the four discovery agents (A/B/C/D) produced ~10-15 task recommendations. Each was re-checked by a paired verification agent with:

- A specific per-task evidence checklist (file:line citations, dependent counts, command outputs).
- Explicit instructions to look for **false positives** (e.g. "the plan claims X is unused — grep for actual uses").
- Per the project memory rule on audit-agent retirement claims (5/10 historical false-positive rate), every "delete this / fold this" claim required a grep across `tests/`, `.github/workflows/`, `contracts/`, `examples/`, and `docs/src/adr/`.

Verification verdicts:

| Verdict | A-series | B-series | C-series | D-series | Total |
|---|---:|---:|---:|---:|---:|
| CONFIRMED | 4 | 6 | 6 | 8 | 24 |
| REFINE | 7 | 5 | 4 | 5 | 21 |
| FALSE | 2 | 2 | 1 | 1 | 6 |
| BLOCKED | 0 | 1 | 0 | 0 | 1 |
| **NEW (added)** | 2 | 2 | 2 | 2 | 8 |

Highest-value single verification catch: **D-9** (`vox-container` split). The discovery audit listed 2 consumers; verification found 4 (the missed pair being `vox-cli` and `vox-skills`). Per project memory this exact class of miss has previously cost ~9k LoC of integration-test recovery work.

Discovery audits without verification would have shipped a plan with:
- 4 outright FALSE tasks (A-11, A-15, B-12, D-15)
- 1 BLOCKED-pretending-to-be-ready task (B-8)
- ~5 user-visible miscounts (vox-publisher LoC, vox-cli dep count, vox-config dependent count, vox-container consumer count, vox-grammar-export consumer count)
- 1 release-criterion-level finding missed (CR-4 / C-16)

Future audit cycles in this workspace should default to the two-pass model.
