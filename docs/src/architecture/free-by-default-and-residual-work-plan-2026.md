---
title: "Free-by-Default & Residual Work Plan (post-audit 2026-05-24)"
description: "Single-file forward plan after the 50-task crate audit closes out P0-P4. Captures the 'free/fast tier first-class' product directive, the model-routing YAML changes, the 180 free_only/is_free call sites that need audit, the dei_shim/selection re-activation, the D-7-rescope NodeRecord topology decision, the residual Tier-D plan, and the push-to-origin sequence. Designed so no further planning sessions are required to execute."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-24"
training_eligible: true
training_rationale: "Forward-looking plan with explicit gates and acceptance criteria; high-value for future LLM continuation."
sort_order: 35
---

# Free-by-Default & Residual Work Plan (2026-05-24)

**Companion to:**
- [`crate-audit-and-plan-2026.md`](./crate-audit-and-plan-2026.md) (the 50-task audit, P0-P4 complete)
- [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md) (C5 / `vox-orchestrator-core` extraction, deferred)
- [`mesh-and-language-distribution-ssot-2026.md`](./mesh-and-language-distribution-ssot-2026.md) (mesh phase plan, intersects in §F)

## 0. Verified ground truth (refreshed 2026-05-25)

> **Update (2026-05-25):** F-B/C/D/E/F/G all executed in session 10. Only F-A
> (push) remains. F-H/F-I remain gated/deferred. The follow-up plan is
> [`post-sprint-forward-plan-2026-05-25.md`](./post-sprint-forward-plan-2026-05-25.md).

| Signal | Value |
|---|---|
| `vox-arch-check` | `build.1237`: **clean ✓** |
| `cargo check --workspace` wall-time | **~34s** (incremental after sprint) |
| `vox-orchestrator` LoC | 60,681 / 70,000 (13.3% headroom) — unchanged this sprint |
| `vox-dei-shim` LoC | ~5,016 / 8,000 (selection/ added without LoC growth) |
| Working tree | **clean** |
| Local `main` ↔ `origin/main` divergence | **+49 commits** (was +41; +8 from this sprint) |
| Tasks complete in audit | 50 of 50 actionable (A-22 / B-13 / D-17 retired this sprint; deferrals documented in post-sprint plan §R-G/R-H/R-I) |
| Tasks deferred | F-H/A-19 (C5, Rule-13 gated), F-I/A-20 (cli-ci, no pressure), R-E (D-7 step 3+), R-F (D-9 impl move), R-G (A-9 retired) |
| Free-tier infrastructure | **LIVE** — `default_cost_preference() = Economy`, `QualityLevel::Flash\|Balanced → Economy`; `ModelTier::Free` + `Fast` first-class |
| ModelTier variants in YAML | `Unknown`, `Local`, **`Free`**, **`Fast`**, `Light`, `Pro`, `Elite` |

## 1. The product gap: "free by default"

The user's directive (2026-05-24): *"Fast and Free tier variants are not only approved, they're first class. Code base wide, free by default has to work well."*

**Current contradicting reality:**
- `vox-orchestrator/src/config/defaults.rs:33-35` returns `CostPreference::Performance` by default
- `ModelTier` enum (generated from `contracts/orchestration/model-routing.v1.yaml`) does **not** include `Fast` or `Free` variants
- `CostPreference` does **not** derive `Default` (which is why the extracted `selection/scorer.rs` failed to compile)
- The `routing_table.rs` 31 hardcoded entries route to `Elite`/`Pro`/`Light` only — never to `Free`
- No `RoutingProfile` Rust enum exists (only a `VoxRoutingProfile` env-var string id in `vox-secrets`)

This plan's §F (Free-Tier Track) is the long-pole work to close that gap.

---

## 2. Work-track index (read this first)

The remaining work decomposes into seven independent tracks. Each track has its own acceptance criteria and can be executed independently across sessions; track ordering is by **dependency**, not priority.

| Track | Code | Size | Blocks | Status |
|---|---|---|---|---|
| **A. Push & merge backlog** | F-A | S | nothing | **READY** (49 commits, user-approved 2026-05-25) |
| **B. Tier-D plan refresh** | F-B | S | F-G | **✅ DONE** (`f93efdbb03`) |
| **C. Stale-ref sweep** | F-C | S | nothing | **✅ DONE** (`4f9c40f9fa`) |
| **D. `vox-populi-types` topology ADR** | F-D | S | F-E | **✅ DONE** (ADR-042, commit included in `a0a236ee44`) |
| **E. D-7-rescope Step 2** | F-E | M | nothing | **✅ DONE** (`a0a236ee44`) |
| **F. Free-tier & Fast-tier first-class** | F-F (1-7) | L | F-G | **✅ DONE** (`175a03d6c8` + `21b98edc74` + `7f2edd8e7e`) |
| **G. `selection/` re-activation in `vox-dei-shim`** | F-G | M | F-F-1, F-F-2 | **✅ DONE** (bundled in `175a03d6c8`) |
| **H. C5 / `vox-orchestrator-core`** | F-H | XL | none (Rule 13 gate) | gated — see post-sprint §R-H |
| **I. `vox-cli-ci` extraction** | F-I | L | none (no pressure) | deferred — see post-sprint §R-I |

---

## 3. Detailed tracks

### F-A. Push & merge backlog (S, immediate)

**Goal:** Get the 41 unpushed audit commits onto `origin/main` so they're durable and visible to other agents/branches.

**Verified state:**
- `git rev-list --count origin/main..HEAD` = **41**
- All commits build clean (workspace `cargo check` = 54.5s ✓)
- `vox-arch-check` clean ✓
- No working-tree drift

**Steps:**
1. Confirm there are no in-flight rebases / branch surgeries from sibling worktrees:
   - `git worktree list` shows 15 worktrees, 4 with active branches (jovial-buck, dashboard-vuv-port, docs-voxlang-cf-migration, naughty-dirac, share-s2-s9)
   - These should be left alone — they have independent unmerged work
2. `git push origin main` (assumes user has commit signing / CI keys configured)
3. Verify GitHub CI passes on the new HEAD
4. If CI fails, do **not** revert — fix forward; revert risks losing 41 commits' worth of audit work

**Acceptance:**
- `git log origin/main..HEAD` returns empty
- GitHub `main` CI shows green

**Risk:** Low. Worst case: a CI gate not exercised locally fails. Fix-forward applies.

---

### F-B. Tier-D plan refresh (S, partial)

**Done in session 9 (2026-05-24):** Updated TL;DR + §1 with post-A-12 numbers (60,681 LoC / 13.3% headroom).

**Remaining:** Audit the rest of the doc (subdir breakdown beyond `dei_shim/`, §4 prose still says "Recommended: do dei_shim/ extraction only if Rule 13 fires" — should now say "completed 2026-05-24"). One-page polish.

**Acceptance:** Read the entire doc; no contradiction with current LoC; §4 reflects A-12 landed.

---

### F-C. Stale-ref sweep (S, partial)

**Done in session 9:** `vox-db/src/research_pipeline.rs`, `vox-doc-inventory/src/constants.rs`.

**Still to audit:**
- `grep -rn "vox-orchestrator/src/dei_shim\|vox_orchestrator::dei_shim" docs/ contracts/ examples/ tools/` (only checked `crates/` in session 9)
- `grep -rn "5,005\|5005" docs/src/architecture/` — stale dei_shim LoC numbers in other architecture docs
- `grep -rn "dei_shim" docs/src/architecture/` — non-code doc references

**Acceptance:** Single grep run shows zero stale references outside the audit doc's history section.

---

### F-D. `vox-populi-types` topology ADR (S, blocks F-E)

**Goal:** Decide where `NodeRecord`, `PopuliRegistryFile`, `PopuliRegistryError` should canonically live.

**Constraint discovered in session 9:** `NodeRecord` depends on `vox_repository::TaskCapabilityHints` (L2). It **cannot** go to `vox-mesh-types` (L0) without also moving `TaskCapabilityHints`.

**Recommendation (to be confirmed in the ADR):**
- Create new crate `vox-populi-types` at **L2** (same layer as `vox-repository`)
- Move `NodeRecord`, `PopuliRegistryFile`, `PopuliRegistryError`, `MAX_MAINTENANCE_FOR_MS`, `node_maintenance_blocks_new_work`, `sweep_expired_maintenance_on_nodes` into it
- Leave runtime fns (`populi_env`, `node_record_for_current_process`, `local_registry_path`) in `vox-populi` — they need `vox-secrets` and FS access
- Both `vox-populi` and `vox-plugin-populi-mesh` consume from `vox-populi-types`

**ADR template:** `docs/src/architecture/_template-adr.md` (no template file exists yet); mimic format of [`mesh-and-language-distribution-ssot-2026.md`](./mesh-and-language-distribution-ssot-2026.md) section headers.

**Acceptance:** ADR merged; `layers.toml` has a `[planned]` entry for `vox-populi-types`; no code changes yet.

---

### F-E. D-7-rescope Step 2 — execute the topology (M, after F-D)

**Prereq:** F-D ADR merged.

**Steps:**
1. `cargo new --lib crates/vox-populi-types` with `description = "Pure-data L2 leaf for the populi node registry: NodeRecord, PopuliRegistryFile, registry error."`
2. Add `vox-repository = { workspace = true }` to its deps (for `TaskCapabilityHints`)
3. Move `vox-populi/src/node_registry.rs` content into `vox-populi-types/src/lib.rs`, splitting out the pure-data types from the file-IO ops (`PopuliRegistry::load/save/upsert_node` stay in `vox-populi`; `NodeRecord` struct + helpers move to the new crate)
4. `vox-populi` adds `pub use vox_populi_types::{NodeRecord, PopuliRegistryFile, PopuliRegistryError, ...}` for back-compat
5. `vox-plugin-populi-mesh` drops `vox-populi = { workspace = true }` from `[dependencies]`, adds `vox-populi-types = { workspace = true }`
6. The plugin's `lib.rs` line 17 `pub(crate) use vox_populi::{...}` becomes `pub(crate) use vox_populi_types::{...}`
7. Runtime functions (`populi_env`, `node_record_for_current_process`, `local_registry_path`) — if the plugin still needs them, **keep** `vox-populi` as a dep but mark it as a topology smell to be resolved in a follow-up (move those fns into a `vox-populi-runtime` L2 crate, or inline them into the plugin if their dep surface is small)

**Acceptance:**
- `vox-arch-check` clean
- `cargo check -p vox-plugin-populi-mesh` clean
- `cargo check -p vox-populi` clean
- Plugin no longer has `vox-populi` as a *compile-time data* dep (runtime-fn dep may remain pending follow-up)

**Risk:** Medium. Cargo workspace moves can break workspace-hack; rerun `cargo hakari generate` after the move.

---

### F-F. Free-tier & Fast-tier first-class (L, multi-step, blocks F-G)

This is the user's headline directive. Decomposes into 7 sub-steps; each is independently committable.

#### F-F-1. Add `Fast` and `Free` to `model-routing.v1.yaml` (S)

```yaml
tiers:
  - Unknown
  - Local
  - Free      # ← new: zero-cost models (free tier with rate limits)
  - Fast      # ← new: speed-optimized, lower-cost than Pro
  - Light
  - Pro
  - Elite
```

Bumping `x-vox-version` is **not** required (additive enum change). Down-stream serde defaults preserve unknown variants via `#[default]`.

**Side-effects:**
- `build.rs` in `vox-orchestrator` regenerates `ModelTier` enum
- `vox-dashboard`'s typeshare-equivalent surface may need a TS rebuild (`vox-tauri-codegen`)
- Any `match ModelTier { ... }` with non-exhaustive arms now warns; any `#[deny(non_exhaustive_omitted_patterns)]` will fail-stop

**Pre-emptive sweep:**
- `grep -rn "match.*ModelTier" crates/` — ~12 hit sites; each needs `Fast => ..., Free => ...` arms
- `grep -rn "ModelTier::Elite\|ModelTier::Pro\|ModelTier::Light\|ModelTier::Local\|ModelTier::Unknown" crates/` already returned 30 hits in `models/routing_table.rs`, `models/registry.rs`, `catalog.rs`, `routing/engine.rs`, `cli/commands/model/`

**Acceptance:** `cargo check --workspace` clean after enum bump + exhaustive arm fixes.

#### F-F-2. `impl Default for CostPreference` (S)

```rust
// crates/vox-orchestrator/src/config/enums.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostPreference {
    /// Prioritize model performance/quality over cost.
    Performance,
    /// Prioritize lower cost models — default for "free by default" behavior.
    #[default]
    Economy,
}
```

**Side-effects:**
- `default_cost_preference()` in `defaults.rs` can now just `CostPreference::default()`; semantically equivalent to the explicit Economy below
- Tests that asserted `CostPreference::Performance` as the default will fail — these need updating (search `assert_eq!(.*CostPreference::Performance)` and `expect.*Performance`)

#### F-F-3. Flip `default_cost_preference()` from `Performance` → `Economy` (S)

```rust
// crates/vox-orchestrator/src/config/defaults.rs:33-35
pub(super) fn default_cost_preference() -> CostPreference {
    CostPreference::Economy  // was: Performance — see free-by-default-plan-2026.md
}
```

**Side-effect surface (verified by grep `default_cost_preference`):**
- Only one definition; called from `OrchestratorConfig::default()`
- Any integration test that asserts "spent $X on default config" will likely see lower spend → assertions may need updating but in the correct direction

**Migration safety:**
- A `Clavis` config / env override (`VOX_ORCHESTRATOR_COST_PREFERENCE=performance`) lets users opt back into the old behavior
- Add a one-line note to `docs/src/reference/configuration.md` (or wherever the cost-preference env var is documented)

#### F-F-4. Add missing capability fields to `ModelCapabilities` (S)

The `selection/scorer.rs` referenced:
- `supports_file_input: bool` — missing
- `supports_jsonl: bool` — missing

Add to `crates/vox-orchestrator/src/models/spec.rs`. Update the build-script generator if it touches these (it doesn't — these are plain struct fields). Default values:
- `supports_file_input: false`
- `supports_jsonl: false`

Both should also be added to the `Capability`/`CapabilityFlags` infer machinery so model registries can declare them.

#### F-F-5. Add `supports_web_search()` method on `ModelSpec` (XS)

The selection scorer calls `model.supports_web_search()` as a method but the current API exposes only the field via `model.capabilities.supports_web_search`. Add a thin inherent impl:

```rust
impl ModelSpec {
    #[inline]
    pub fn supports_web_search(&self) -> bool {
        self.capabilities.supports_web_search
    }
}
```

Cheap, no breakage. Also consider adding `supports_vision()`, `supports_tool_use()`, etc. for symmetry — these are also accessed via `.capabilities.` elsewhere.

#### F-F-6. Define `RoutingProfile` enum (S)

Currently `RoutingProfile` is only a `SecretId::VoxRoutingProfile` env-var key. The selection code wants a Rust enum.

Recommended shape (mirror existing `ScalingProfile`):

```rust
// crates/vox-orchestrator/src/types/routing.rs (new file)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingProfile {
    /// Free-tier models only — no API keys required.
    #[default]
    Free,
    /// Mix free + paid; prefer free when quality is comparable.
    Mixed,
    /// Prioritize quality; paid models freely chosen.
    Performance,
    /// Local-only (Mens, Ollama); no external calls.
    Local,
}
```

Then add `config_to_routing_profile(cfg: &OrchestratorConfig) -> RoutingProfile` helper that reads `cost_preference` + the env `VoxRoutingProfile` overlay.

#### F-F-7. Free-by-default audit pass over the 180 `is_free`/`free_only` call sites (M)

Goal: every entry point that selects a model should respect "free by default" — i.e. when nothing is configured, only `is_free == true` models are chosen.

Survey already done:
- `vox-cli/commands/review/dei.rs`, `vox-code-audit/review/`, `vox-ml-cli/commands/mens/populi/`, `vox-orchestrator/catalog.rs`, `vox-orchestrator/mode.rs` are the main consumer clusters

For each cluster, audit:
1. Is there a default-construction path that creates a config with `free_only: false`?
2. Is there a CLI flag / env var to override?
3. Does the model registry actually have free-tier models for the requested task?

**Deliverable:** A short report (200 lines) listing each call-site cluster, its current default, and whether the default needs flipping. Land that report in `docs/src/architecture/free-by-default-audit-report-2026.md`.

**Acceptance:** Report shows zero clusters that silently spend money on a fresh install.

---

### F-G. `selection/` re-activation in `vox-dei-shim` (M, after F-F-1..6)

**Prereq:** F-F-1 (Fast/Free in YAML), F-F-2 (CostPreference Default), F-F-4 (capability fields), F-F-5 (supports_web_search method), F-F-6 (RoutingProfile enum).

**Steps:**
1. Re-copy the original 7 selection files from git history (`git show HEAD~3:crates/vox-orchestrator/src/dei_shim/selection/`) into `crates/vox-research-shim/src/selection/`
2. Re-run the `crate::` → `vox_orchestrator::` sed transformation (see session 9 commit `94ce7d5b5f` for the recipe)
3. Add `pub mod selection;` to `crates/vox-research-shim/src/lib.rs`
4. Resolve compilation errors — they should now all be type-available
5. Verify the `selection/tests.rs` test module compiles (it has `#[cfg(test)]`)
6. Run `cargo test -p vox-dei-shim` — selection tests should pass
7. Update `vox-dei-shim/src/lib.rs` doc comment to remove the "selection/ excluded" notice

**Acceptance:**
- `cargo check -p vox-dei-shim` clean
- `cargo test -p vox-dei-shim selection::` passes
- `vox-arch-check` clean
- `vox-dei-shim` LoC stays under 8,000 (currently 5,016; selection/ was ~1,400; total ~6,400 — comfortable)

---

### F-H. C5 — `vox-orchestrator-core` extraction (XL, gated)

**Gate:** Do not start until `vox-orchestrator` Rule 13 fires (>15% LoC growth from v0.5.0 baseline, ≈ 59,500 LoC threshold). Current state: 60,681 LoC, but Rule 13 is keyed off **growth-since-tag**, not absolute; the v0.5.0 tag baseline computation is in `vox-arch-check`.

**Plan reference:** [`2026-05-15-orchestrator-tier-d-plan.md`](./2026-05-15-orchestrator-tier-d-plan.md) §3 (already updated to reflect post-A-12 LoC).

**Estimated effort:** XL — ~20-25K LoC displacement, weeks of careful work. **Do not attempt until trigger fires.**

---

### F-I. `vox-cli-ci` extraction (L, deferred)

`vox-cli` has no LoC pressure currently. Defer unless `vox-cli` itself hits a budget. Plan owner: [`2026-05-15-cli-ci-extraction-plan.md`](./2026-05-15-cli-ci-extraction-plan.md).

---

## 4. False-positive / blind-spot audit

Re-running the FP/TP check against the audit's claims with fresh eyes (verification on top of session 8's verification pass):

| Original audit claim | New verdict | Notes |
|---|---|---|
| A-12 is "extraction-safe, no impl Orchestrator weaving" | **REFINED** | Correct that no impl weaving, but coupling to `models`, `mode`, `config`, `types` is deep. Resolved by `vox-dei-shim → vox-orchestrator` dep direction (wedge pattern). |
| A-12 dei_shim is "4,626 LoC" | **REFINED** | Actual is ~3,500 LoC of research-pipeline (selection/ was dead). Audit included dead WIP files in count. |
| selection/ in dei_shim is active | **NEW FP** | Audit assumed all dei_shim/ subdirs were live. selection/ was never in module tree — discovered in session 9 only via compile errors. |
| "Free by default" supposedly working | **NEW TP** | `default_cost_preference() = Performance`. The product directive is **not** satisfied by current code. This is the biggest gap in the codebase relative to stated intent. |
| Tier-D §4 says "dei_shim a lower-risk intermediate" | **CONFIRMED + STALE** | Recommendation was sound and we executed it; doc text needs updating to past-tense. |
| D-7-rescope Step 2 is "deferred — requires migrating NodeRecord to vox-mesh-types (L0)" | **REFUTED** | NodeRecord cannot go to L0 (depends on L2 `TaskCapabilityHints`). Correct target is new `vox-populi-types` at L2. |
| X-1 tracking PR | **PRUNED** | Direct-to-main workflow means a "PR" is just a push. Renamed to F-A (Push & merge backlog). |
| All P0-P4 done means "system is healthy" | **REFINED** | Healthy *architecturally* (arch-check clean, all layers respected). But the *product directive* "free by default" is unmet. Architectural health ≠ product correctness. |

---

## 5. Sequencing & calendar

Suggested run order (independent tracks can run in parallel by separate sessions / agents):

```
Session 10 (immediate, this push):
  F-A:    push 41 commits to origin/main
  F-B:    finish Tier-D plan polish
  F-C:    full stale-ref sweep
  Total ETA: 30 minutes

Session 11 (next session, can be different agent):
  F-D:    write vox-populi-types ADR
  F-F-1:  add Fast/Free to YAML + regenerate enum + fix non-exhaustive matches
  F-F-2:  CostPreference Default derive
  F-F-3:  flip default_cost_preference to Economy
  Total ETA: 2-3 hours

Session 12:
  F-E:    execute D-7-rescope Step 2 (vox-populi-types crate)
  F-F-4:  add missing ModelCapabilities fields
  F-F-5:  add supports_web_search() method (+ siblings)
  F-F-6:  define RoutingProfile enum
  Total ETA: 2 hours

Session 13:
  F-G:    re-activate selection/ in vox-dei-shim
  F-F-7:  free-by-default audit report (180 call sites)
  Total ETA: 4-6 hours

Session N (gated, indefinite future):
  F-H:    C5 vox-orchestrator-core extraction (XL, weeks)

Session M (gated, indefinite future):
  F-I:    vox-cli-ci extraction (L)
```

---

## 6. Acceptance criteria for "audit fully complete"

This plan is considered fully executed when **all** of these are true:

1. ✅ `vox-arch-check` clean (`build.1237`)
2. ✅ `cargo check --workspace` clean (~34s incremental)
3. ⬜ `git rev-list --count origin/main..HEAD` = 0 (after F-A — **ready to push, 49 commits**)
4. ✅ `grep -rn "vox-orchestrator/src/dei_shim" docs/ contracts/ examples/ tools/` returns zero (F-C, `4f9c40f9fa`)
5. ✅ `ModelTier::Free` and `ModelTier::Fast` exist in generated enum (F-F-1, `175a03d6c8`)
6. ✅ `CostPreference::default() == Economy` (F-F-2 + F-F-3, `175a03d6c8`)
7. ✅ `vox-populi-types` L2 crate landed; `vox-populi` re-exports from it (F-E + ADR-042, `a0a236ee44`)
8. ✅ `vox-dei-shim::selection` is a compiling, tested module (F-G, bundled in `175a03d6c8` — 10 tests passing)
9. ✅ Free-by-default audit report exists at `free-by-default-audit-2026-05-24.md`; all three follow-ups (Balanced→Economy, exploration parity, RoutingProfile docs) closed in `7f2edd8e7e`
10. ✅ Tier-D plan doc updated to show A-12 as completed past-tense (F-B, `f93efdbb03`)

---

## 7. What this plan deliberately does **not** address

- **F-H (C5)** is gated by Rule 13 and may not need execution this calendar year.
- **F-I (vox-cli-ci)** is deferred indefinitely; no LoC pressure exists.
- **Mens distributed training (Mn-T1..T15)** — separate SSOT (mesh-and-language-distribution-ssot-2026.md §3.5). Not in scope here.
- **Telemetry unification (vox-telemetry rollout)** — separate SSOT. Not in scope.
- **Vox language v1 release criteria (CR-L*)** — separate SSOT (vox-as-llm-target-audit-and-plan-2026.md). Not in scope.

If a future session feels the urge to "expand scope" into one of those, **stop and read the relevant SSOT first**. Each has its own gate criteria.

---

## 8. How to use this plan in a future session

1. Read **§0** to ground yourself in current state
2. Read **§2** index, pick the next unstarted track
3. Read **§3** for that track's full prescription
4. Execute — each track is self-contained
5. Update **§6 acceptance** checklist when you complete a track
6. Update **§0 ground truth table** at the bottom of the session

The plan is designed so a fresh agent can pick it up cold and not re-litigate decisions.

---

## 9. Session-10 completion log (2026-05-25)

All non-gated tracks executed. Commits in chronological order:

| Track | SHA | Subject |
|---|---|---|
| F-B (Tier-D refresh) | `f93efdbb03` | docs(arch): update Tier-D plan to reflect post-A-12 orchestrator LoC |
| F-C (stale-ref sweep) | `4f9c40f9fa` | fix(refs): update stale dei_shim path references after A-12 extraction |
| F-B + F-C (master plan) | `bbf9bed00b` | docs(arch): master forward plan + F-B/F-C stale-ref cleanup |
| F-F + F-G | `175a03d6c8` | feat(routing): land free-by-default + Fast/Free model tiers (F-F + F-G) |
| F-F-7 | `21b98edc74` | docs(routing): free-by-default audit + catalog tier reclassification |
| F-D + F-E | `a0a236ee44` | feat(F-E/ADR-042): extract vox-populi-types L2 crate |
| Bonus (docs) | `f28e58daf0` | docs: add vox-populi-types row to where-things-live.md |
| F-F-7 follow-ups | `7f2edd8e7e` | feat(free-by-default): close remaining F-F-7 audit gaps |
| B-2 (bonus, audit-doc cross-track) | `e83a29e9a8` | fix(B-2): wire voxup into workspace metadata + drop dirs dep |

**Remaining:** F-A push (49 commits → `origin/main`). After push, this plan is fully executed.

The follow-on forward plan that covers everything not in scope here (C-16, R-E, R-F, R-H, R-I, R-J, R-K) is [`post-sprint-forward-plan-2026-05-25.md`](./post-sprint-forward-plan-2026-05-25.md).
