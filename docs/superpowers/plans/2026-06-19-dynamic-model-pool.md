# Dynamic Model Pool — Claude Code (Sonnet 4.6) Plan

> **For Sonnet 4.6 executing in Claude Code.** This plan is the **Claude-side half** of the
> dynamic-model-pool feature. The Rust *foundation* (the `vox-config` `ModelPool` type +
> predicate, `list_enabled_providers`, and the three Tauri commands) is delivered
> separately by Gemini Flash in Antigravity — see
> `docs/superpowers/plans/2026-06-19-dynamic-model-pool-GEMINI-FLASH-HANDOFF.md`.
> This document contains **only** the two tasks Claude Code executes:
> **(P2.2) the scorer candidate-filter wiring** and **(P3) the `ModelPoolView` GUI**.
>
> Execute with superpowers:executing-plans (subagents are read-only in this sandbox →
> no subagent-driven). TDD, atomic green commits.

**Goal:** Make the orchestrator's model selection honor the operator's allowed-pool, and
give the operator a multi-source picker to edit it.

**Spec:** `docs/superpowers/specs/2026-06-19-dynamic-model-pool-design.md`

---

## Dependency contract (what Gemini delivers; Claude builds on it)

> **Status (audited 2026-06-19):** Gemini G1 + G2 are **committed** (`044b1b4ad3`, `a8e0bc7179`)
> — `ModelPool`/`PoolRule`/`PoolModelView`/`resolve`/`resolve_with_fallback`/`rule_matches`,
> `VoxConfig.model_pool`, and `list_enabled_providers()` are LIVE in `vox-config`. **→ P2.2 is
> UNBLOCKED; start it now.** G3 (the Tauri commands) is not yet committed (orphaned/unwired),
> so P3's *live* end-to-end wiring waits on it — but P3 builds+tests against the contract below
> with mocked `invoke` in the meantime.

These symbols are produced by the Gemini handoff. **P2.2 needs the predicate landed (it is); P3
can build against the command contract with mocked `invoke` and wire live once the
commands land.** If a symbol below is missing when you start a task, STOP — the Gemini
half hasn't landed yet.

```rust
// crates/vox-config/src/model_pool.rs  (Gemini P1)
pub enum PoolRule { Free, Provider{value:String}, MaxCostPer1k{value:f64}, Tier{value:String}, MinContext{value:u64}, Unknown }
pub struct ModelPool { pub rules:Vec<PoolRule>, pub includes:Vec<String>, pub excludes:Vec<String>, pub disabled_sources:Vec<String> }  // Default = empty
pub struct PoolModelView { pub id:String, pub provider:String, pub cost_per_1k:f64, pub max_tokens:u64, pub is_free:bool, pub tier:String }
pub fn resolve(pool:&ModelPool, catalog:&[PoolModelView], enabled:&BTreeSet<String>) -> BTreeSet<String>;
pub fn resolve_with_fallback(pool:&ModelPool, catalog:&[PoolModelView], enabled:&BTreeSet<String>) -> (BTreeSet<String>, bool); // (ids, fell_open)
pub fn list_enabled_providers() -> BTreeSet<String>;          // Gemini P2.1 (in vox-config)
// VoxConfig gains:  pub model_pool: ModelPool   (#[serde(default)], persists via VoxConfig::save())  (Gemini P1.3)
```
```ts
// crates/vox-gui/ui/src/transport.ts  (Gemini P2.3 exposes these Tauri commands)
get_model_pool()  -> { rules, includes, excludes, disabled_sources, member_ids: string[], fell_open: boolean }
set_model_pool(pool) -> void
list_enabled_providers_cmd() -> string[]
// existing, reuse:  list_model_cards(limit) -> ModelCardDto[]  (id, provider, tier, cost_per_1k, max_tokens, is_free, …)
//                   openrouter_key_status() -> { configured: boolean }   (per-provider key-presence pattern)
```

---

## Task P2.2 — Apply the pool filter at the scorer candidate boundary

**Why Claude/Sonnet (not Flash):** this touches **multiple enumeration sites** in routing-critical code; a silent miss changes which models the system picks. It needs whole-subsystem judgment.

**Files:**
- Modify: `crates/vox-orchestrator/src/models/select.rs` (candidate building — `select_inner`, `let all = registry.list_models()` ~:92 and the exploration-fallback loop ~:128)
- Modify: `crates/vox-orchestrator/src/models/spec.rs` (add `ModelSpec::to_pool_view`)
- Check: `crates/vox-orchestrator/src/models/policy.rs` (`select_free` ~:358, `select_pinned` ~:337) — confirm they draw from the filtered candidates, not a separate unfiltered `registry.list_models()`.
- Test: `crates/vox-orchestrator/src/models/select.rs` `#[cfg(test)]`

- [ ] **Step 1 (audit gate):** Enumerate EVERY selection-path call to `registry.list_models()`:
  ```
  rg -n "registry\.list_models\(\)|\.list_models\(\)" crates/vox-orchestrator/src/models/select.rs crates/vox-orchestrator/src/models/policy.rs
  rg -n "pub struct ModelSpec|pub provider|provider_type|is_free|cost_per_1k|max_tokens|capabilities" crates/vox-orchestrator/src/models/spec.rs | head
  ```
  Confirmed sites as of 2026-06-19: `select.rs:92`, `select.rs:128`. Also inspect `policy.rs` `select_free`/`select_pinned`. Paste output; if a site differs, adjust — every selection enumeration must pass through the filter.

- [ ] **Step 2: Add `to_pool_view`** in `spec.rs` (Step 3 commit bundles it):
```rust
impl ModelSpec {
    /// Minimal projection for the model-pool predicate (vox-config has no dep on this crate).
    pub fn to_pool_view(&self) -> vox_config::model_pool::PoolModelView {
        vox_config::model_pool::PoolModelView {
            id: self.id.clone(),
            provider: self.provider.clone(),
            cost_per_1k: self.cost_per_1k,
            max_tokens: self.max_tokens,
            is_free: self.is_free,
            tier: format!("{:?}", self.capabilities.tier),
        }
    }
}
```

- [ ] **Step 3: Write the failing test** in `select.rs` tests (model on the existing `ModelRegistry::new()` tests ~:1110):
```rust
#[test]
fn pool_excludes_constrain_candidates() {
    // A registry with the default catalog; a pool that excludes a known model id must
    // remove it from the candidate set used by select_inner.
    let registry = ModelRegistry::new();
    let ids: Vec<String> = registry.list_models().iter().map(|m| m.id.clone()).collect();
    assert!(ids.len() >= 2, "need ≥2 models in the default registry to test exclusion");
    let victim = ids[0].clone();
    let pool = vox_config::model_pool::ModelPool { excludes: vec![victim.clone()], ..Default::default() };
    let enabled: std::collections::BTreeSet<String> =
        registry.list_models().iter().map(|m| m.provider.clone()).collect();
    let allowed = apply_pool(&registry.list_models(), &pool, &enabled);
    assert!(!allowed.iter().any(|m| m.id == victim), "excluded model must not be a candidate");
    assert_eq!(allowed.len(), registry.list_models().len() - 1);
}
```

- [ ] **Step 4: Run → FAIL** (`apply_pool` undefined): `cargo test -p vox-orchestrator pool_excludes_constrain_candidates`.

- [ ] **Step 5: Implement** a single shared filter helper + apply it at every site from Step 1:
```rust
/// Single chokepoint: filter a model list to the operator's allowed pool.
/// Empty pool ⇒ all enabled; empty *result* ⇒ fail open to all-enabled (never zero).
fn apply_pool(models: &[ModelSpec], pool: &vox_config::model_pool::ModelPool,
              enabled: &std::collections::BTreeSet<String>) -> Vec<ModelSpec> {
    let views: Vec<_> = models.iter().map(|m| m.to_pool_view()).collect();
    let (allowed, _fell_open) = vox_config::model_pool::resolve_with_fallback(pool, &views, enabled);
    models.iter().filter(|m| allowed.contains(&m.id)).cloned().collect()
}
```
  At `select_inner` (and the exploration loop), immediately after `let all = registry.list_models();`:
```rust
    let pool = vox_config::VoxConfig::load().model_pool;
    let enabled = vox_config::model_pool::list_enabled_providers();
    let all = apply_pool(&all, &pool, &enabled);   // candidates now drawn from the pool
```
  Apply the SAME `apply_pool(...)` to the exploration-fallback loop's `registry.list_models()` and to any `policy.rs` selection enumeration found in Step 1. (Pins resolve after this filter; a pin outside the pool simply falls back to scored selection.)

- [ ] **Step 6: Run → PASS;** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`. Run the existing `select.rs` suite to confirm no routing regression: `cargo test -p vox-orchestrator models::select`.

- [ ] **Step 7: Commit** `feat(model-pool): hard-filter scorer candidates through the operator pool`.

---

## Task P3.1 — GUI transport bindings [PARALLEL-SAFE]

**Files:** Modify `crates/vox-gui/ui/src/transport.ts`; Test: `crates/vox-gui/ui/src/__tests__/transport.modelPool.test.ts`

- [ ] **Step 1 (gate):** `rg -n "list_model_cards|openrouter_key_status|invoke<|export function" crates/vox-gui/ui/src/transport.ts | head` — match the existing wrapper style.
- [ ] **Step 2: Failing test** (mock `@tauri-apps/api/core`): assert `voxTransport.getModelPool()`, `setModelPool(p)`, `listEnabledProviders()` call `invoke` with command names `get_model_pool`, `set_model_pool`, `list_enabled_providers_cmd`.
```ts
import { describe, it, expect, vi } from 'vitest';
const invoke = vi.fn().mockResolvedValue({ rules: [], includes: [], excludes: [], disabled_sources: [], member_ids: [], fell_open: false });
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
import { voxTransport } from '../transport';
describe('model-pool transport', () => {
  it('getModelPool calls get_model_pool', async () => { await voxTransport.getModelPool(); expect(invoke).toHaveBeenCalledWith('get_model_pool'); });
});
```
- [ ] **Step 3: Run → FAIL.**
- [ ] **Step 4: Implement** the three wrappers + a `ModelPoolDto` TS type mirroring the Rust DTO (`rules`, `includes`, `excludes`, `disabled_sources`, `member_ids`, `fell_open`).
- [ ] **Step 5: Run → PASS;** `npx tsc --noEmit`. **Step 6: Commit** `feat(model-pool): GUI transport bindings`.

## Task P3.2 — `ModelPoolView` grouped multi-source picker [SEQUENTIAL]

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/Models/ModelPoolView.tsx` + `ModelPoolView.test.tsx`

- [ ] **Step 1 (gate):** read `ModelsView.tsx` for the `list_model_cards` fetch pattern + a Clavis "add key" affordance (`rg -n "openrouter_key_status|add.*key|set_secret|secrets" crates/vox-gui/ui/src` — reuse the existing key flow; do not invent one).
- [ ] **Step 2: Failing test** (`// @vitest-environment jsdom`; mock transport):
  - models grouped by `provider`; a card whose `id ∈ member_ids` shows an "in pool" badge;
  - a provider NOT in `listEnabledProviders()` renders a greyed section with an "Add key" control;
  - toggling an in-enabled model calls `setModelPool` with that id added to `includes`;
  - when `fell_open` is true, a warning banner renders.
- [ ] **Step 3: Run → FAIL.**
- [ ] **Step 4: Implement** `ModelPoolView` (mirror `ModelsView.tsx` data-fetching):
  - fetch `list_model_cards(200)` + `getModelPool()` + `listEnabledProviders()`;
  - group cards by `provider`; per-group header with an on/off toggle writing `disabled_sources`; greyed group + "Add key" for providers absent from enabled;
  - per-card include/exclude toggle (writes `includes`/`excludes`); in-pool indicator from `member_ids` (and whether it's rule-derived vs explicit);
  - a rules editor: chips for active `rules` + an "add rule" control (kind ∈ free/provider/max_cost_per_1k/tier/min_context, with a value field) writing `rules`;
  - `fell_open` → amber banner "Pool resolves to no models — using all enabled. Loosen a rule.";
  - persist edits via `setModelPool`.
- [ ] **Step 5: Run → PASS;** `npx tsc --noEmit`. **Step 6: Commit** `feat(model-pool): ModelPoolView grouped multi-source picker + rules editor`.

## Task P3.3 — Register surface + Playwright screenshot [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (mount `model-pool`, mirror the `models` case); `crates/vox-gui/ui/e2e/model-pool.spec.ts`

- [ ] **Step 1 (gate):** `rg -n "case 'models'|ModelsView|viewKey" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — mount `ModelPoolView` the same way (as a new surface or a tab within Models; match the registry pattern). If a generated `SURFACE_REGISTRY` gate exists (`vox ci gui-surface-registry`), follow it.
- [ ] **Step 2: Playwright spec** (mirror `e2e/axis-brand.spec.ts` `installTauriMock`): navigate to the pool surface, assert provider-group headers + one enabled + one greyed section render, screenshot `e2e/screens/_model-pool.png`.
- [ ] **Step 3: Run** `npx playwright test model-pool.spec.ts --project=chromium` → PASS.
- [ ] **Step 4:** view the screenshot; confirm grouping + alignment look right. **Step 5: Commit** `feat(model-pool): surface registration + Playwright screenshot`.

---

## Parallelism & order (this plan)
- **Wait for Gemini P1** (the `ModelPool` predicate + `VoxConfig.model_pool` + `list_enabled_providers`) before **P2.2**.
- **P3.1 → P3.2 → P3.3** can run as soon as the command *contract* is known (build/test against mocked `invoke`); they go fully green end-to-end once Gemini P2.3 lands. **P3 is independent of P2.2** and can proceed in parallel with the Gemini backend.
- Within P3: P3.1 is PARALLEL-SAFE; P3.2/P3.3 are sequential (shared component/surface).

## Self-review
- Covers spec §5 GUI (grouped picker, rules editor, greyed/add-key, fell_open) → P3; spec §5 scorer integration → P2.2. Backend predicate/config/commands are in the Gemini doc (excised here, by design).
- No placeholders: the one residual unknown (exact extra enumeration sites in `policy.rs`) is a BLOCKING audit gate in P2.2 Step 1 with explicit "every site must pass through the filter" instruction.
- Types match the dependency contract block verbatim.
