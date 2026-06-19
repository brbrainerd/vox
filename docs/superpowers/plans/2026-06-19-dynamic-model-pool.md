# Dynamic Multi-Source Model Pool — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> 🤖 **EXECUTION TARGET.** Phases 1–2 + 4 (Rust) are Gemini-Flash-3.5/Antigravity-shaped; Phase 3 (GUI) is Claude-Code-side. See `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`. Follow the Operating Rules + Flash Execution Addendum exactly.

**Goal:** Add an operator-curated **allowed-model pool** (dynamic rules ∪ explicit picks − excludes) that hard-filters the scorer's candidate set to models from enabled sources, so new matching models auto-join and selection stays within the pool.

**Architecture:** A pure, dependency-free predicate in `vox-config` resolves a `[model_pool]` config (from `~/.vox/config.toml`) against a minimal model view + the set of enabled providers, producing the allowed id set. The orchestrator applies it at the candidate-enumeration boundary before scoring. The GUI edits the pool via three thin Tauri commands. Empty pool ⇒ all enabled (today's behavior).

**Tech Stack:** Rust + serde + toml (`vox-config`, `vox-orchestrator`), Tauri (`vox-gui`), TypeScript/React + vitest + Playwright (`vox-gui/ui`).

**Spec:** `docs/superpowers/specs/2026-06-19-dynamic-model-pool-design.md`

---

**Operating Rules (every task):**
1. Atomic + green + committed; a kill between tasks leaves a compiling, tested tree.
2. **Verify before use:** every Step-1 `rg`/read is a BLOCKING gate — run it, paste output; if reality differs, STOP and report.
3. Two-strike circuit breaker: a step fails twice → STOP and report.
4. Split on overrun: an implement step touching >1 file or adding >1 fn → one atomic commit per sub-bullet.
5. **House rules:** `cargo test -p <crate> <filter>` → `cargo clippy -p <crate> -- -D warnings` → `cargo fmt -p <crate>` (never `cargo fmt --all`). GUI from `crates/vox-gui/ui`: `npx vitest run <path>` → `npx tsc --noEmit`; `// @vitest-environment jsdom` first line of component tests. `vox-gui` Rust: lib-only clippy. No stubs. (Tandem note: the repo build-lock may be contended; if `cargo` blocks, wait/retry — do not bypass.)
6. Tags: `[PARALLEL-SAFE]` (disjoint files) / `[SEQUENTIAL]` (shared file).

**Execution tiers:**

| Tier | Phases | Why |
|---|---|---|
| **Gemini-Flash** | 1 (predicate+config), 2 (commands+wiring), 4 (free-const) | pure Rust TDD, atomic, gated |
| **Claude-Code** | 3 (ModelPoolView GUI + Playwright) | inline-SVG/JSX/asset/visual surface |

---

## Flash Execution Addendum (2026-06-19)

**Global facts (confirm, don't assume):**
- `ModelCardDto` (`crates/vox-gui/src/commands/models.rs:12`) has: `id, provider, tier:String, cost_per_1k:f64, max_tokens:u32, is_free:bool, latency_p50_ms, success_rate, quality_score`.
- `list_model_cards` reads `registry_from_cache().list_models()`; each `ModelSpec` has `id, provider, cost_per_1k:f64, max_tokens, is_free, capabilities.tier`.
- `~/.vox/config.toml` layered resolution lives in `crates/vox-config/src/env_parse.rs`.
- Tauri commands are registered in `crates/vox-gui/src/main.rs` (~:153, the `tauri::generate_handler!` list).
- `OPENROUTER_FREE_MODELS` (`crates/vox-gamify/src/ai/constants.rs:16`) has 3 consumers: `ai/client/ctor.rs:194`, `ai/client/transport.rs:223`, `ai/provider.rs:76`.

**Mandatory pre-flight (run from repo root, paste output, confirm before code):**
```
rg -n "pub fn|config.toml|home|\.vox" crates/vox-config/src/env_parse.rs | head -20
rg -n "fn list_models|pub struct ModelSpec|capabilities|is_free|cost_per_1k|tier" crates/vox-orchestrator/src/models/spec.rs | head
rg -n "registry_from_cache|generate_handler|list_model_cards" crates/vox-gui/src/main.rs crates/vox-gui/src/commands/models.rs | head
rg -n "fn .*enabled|resolve_secret|SecretId::" crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs | head
```
Expected: a config.toml path/parse helper in env_parse; `ModelSpec` with the fields above; the `generate_handler!` macro list; key-presence detection in llm_routing. If any differs, STOP and report.

**Task-split:**

| Task | Touches | Tag |
|---|---|---|
| 1.1 `ModelPool` type + serde | `vox-config/src/model_pool.rs` (new) | [PARALLEL-SAFE] |
| 1.2 predicate `resolve` + `rule_matches` | `vox-config/src/model_pool.rs` | [SEQUENTIAL] (same file as 1.1) |
| 1.3 load/save `~/.vox/config.toml` | `vox-config/src/model_pool.rs` + `lib.rs` re-export | [SEQUENTIAL] |
| 2.1 `list_enabled_providers` fn | extract in `vox-config` or a shared fn; `llm_routing.rs` reuse | [PARALLEL-SAFE] |
| 2.2 scorer candidate wiring | `vox-orchestrator/src/models/registry.rs` (candidate site) | [SEQUENTIAL] |
| 2.3 Tauri commands | `vox-gui/src/commands/model_pool.rs` (new) + `main.rs` | [SEQUENTIAL] (main.rs) |
| 4.1 retire free-const | `vox-gamify/src/ai/*` (gated — see Phase 4) | [SEQUENTIAL] |

---

# PHASE 1 — `ModelPool` config + predicate (Gemini-Flash)

### Task 1.1 — `ModelPool` type + `PoolModelView` [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-config/src/model_pool.rs`
- Modify: `crates/vox-config/src/lib.rs` (add `pub mod model_pool;`)
- Test: in `model_pool.rs` `#[cfg(test)]`

- [ ] **Step 1 (gate):** `rg -n "pub mod" crates/vox-config/src/lib.rs | head` — confirm the module-declaration style to match.

- [ ] **Step 2: Write the failing test** (round-trip of the config shape):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pool_parses_rules_includes_excludes() {
        let toml_src = r#"
            rules = [{ kind = "free" }, { kind = "provider", value = "anthropic" }, { kind = "max_cost_per_1k", value = 0.005 }]
            includes = ["openai/gpt-5.5-pro"]
            excludes = ["x/deprecated"]
            disabled_sources = ["mesh"]
        "#;
        let pool: ModelPool = toml::from_str(toml_src).unwrap();
        assert_eq!(pool.rules.len(), 3);
        assert_eq!(pool.includes, vec!["openai/gpt-5.5-pro"]);
        assert_eq!(pool.excludes, vec!["x/deprecated"]);
        assert_eq!(pool.disabled_sources, vec!["mesh"]);
    }
    #[test]
    fn empty_pool_is_default() {
        let pool = ModelPool::default();
        assert!(pool.rules.is_empty() && pool.includes.is_empty() && pool.excludes.is_empty());
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config model_pool::tests::pool_parses -- --nocapture` (Expected: type not found.)

- [ ] **Step 4: Implement the types:**

```rust
//! Operator-curated allowed-model pool. Pure data + predicate; no I/O except the
//! load/save helpers in Task 1.3. Resolved against a minimal model view so this
//! crate never depends on the orchestrator's ModelSpec.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PoolRule {
    Free,
    Provider { value: String },
    MaxCostPer1k { value: f64 },
    Tier { value: String },
    MinContext { value: u64 },
    #[serde(other)]
    Unknown, // forward-compatible: unknown kinds are ignored by rule_matches
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelPool {
    pub rules: Vec<PoolRule>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub disabled_sources: Vec<String>,
}

/// Minimal projection of a model the predicate needs. Both ModelSpec and ModelCardDto
/// map into this (Task 2.2 / Phase 3).
#[derive(Debug, Clone)]
pub struct PoolModelView {
    pub id: String,
    pub provider: String,
    pub cost_per_1k: f64,
    pub max_tokens: u64,
    pub is_free: bool,
    pub tier: String, // e.g. "Elite" (Debug of ModelTier)
}
```

- [ ] **Step 5: Run → PASS.** Then `cargo clippy -p vox-config -- -D warnings`; `cargo fmt -p vox-config`.
- [ ] **Step 6: Commit** `feat(model-pool): ModelPool config type + PoolModelView`.

### Task 1.2 — predicate `resolve` + `rule_matches` [SEQUENTIAL]

**Files:** Modify `crates/vox-config/src/model_pool.rs`

- [ ] **Step 1: Write the failing tests** (precedence + empty=all + fail-open):

```rust
    fn mv(id: &str, provider: &str, cost: f64, free: bool, tier: &str, ctx: u64) -> PoolModelView {
        PoolModelView { id: id.into(), provider: provider.into(), cost_per_1k: cost, max_tokens: ctx, is_free: free, tier: tier.into() }
    }
    fn catalog() -> Vec<PoolModelView> {
        vec![
            mv("openrouter/free-x", "openrouter", 0.0, true, "Free", 8000),
            mv("anthropic/claude-opus", "anthropic", 0.015, false, "Elite", 200000),
            mv("openai/gpt-mini", "openai", 0.002, false, "Fast", 128000),
            mv("x/deprecated", "openai", 0.001, false, "Fast", 4000),
        ]
    }
    #[test]
    fn empty_pool_allows_all_enabled() {
        let enabled = ["openrouter".to_string(), "anthropic".to_string(), "openai".to_string()].into_iter().collect();
        let got = resolve(&ModelPool::default(), &catalog(), &enabled);
        assert_eq!(got.len(), 4);
    }
    #[test]
    fn rules_union_includes_minus_excludes() {
        let pool = ModelPool {
            rules: vec![PoolRule::Free, PoolRule::MaxCostPer1k { value: 0.005 }],
            includes: vec!["anthropic/claude-opus".into()],
            excludes: vec!["x/deprecated".into()],
            disabled_sources: vec![],
        };
        let enabled = ["openrouter".to_string(), "anthropic".to_string(), "openai".to_string()].into_iter().collect();
        let got = resolve(&pool, &catalog(), &enabled);
        // free-x (free) + gpt-mini (<=0.005) + claude-opus (include); deprecated excluded though <=0.005
        assert!(got.contains("openrouter/free-x") && got.contains("openai/gpt-mini") && got.contains("anthropic/claude-opus"));
        assert!(!got.contains("x/deprecated"));
    }
    #[test]
    fn disabled_source_and_unenabled_provider_drop() {
        let pool = ModelPool { disabled_sources: vec!["openrouter".into()], ..Default::default() };
        let enabled = ["openrouter".to_string(), "anthropic".to_string()].into_iter().collect();
        let got = resolve(&pool, &catalog(), &enabled);
        assert!(!got.contains("openrouter/free-x")); // disabled source
        assert!(!got.contains("openai/gpt-mini"));    // provider not enabled
    }
    #[test]
    fn empty_result_fails_open_to_all_enabled() {
        let pool = ModelPool { excludes: catalog().iter().map(|m| m.id.clone()).collect(), ..Default::default() };
        let enabled = ["openrouter".to_string(), "anthropic".to_string(), "openai".to_string()].into_iter().collect();
        let (got, fell_open) = resolve_with_fallback(&pool, &catalog(), &enabled);
        assert!(fell_open);
        assert_eq!(got.len(), 4);
    }
```

- [ ] **Step 2: Run → FAIL** (`resolve`/`resolve_with_fallback` not defined).

- [ ] **Step 3: Implement:**

```rust
use std::collections::BTreeSet;

pub fn rule_matches(rule: &PoolRule, m: &PoolModelView) -> bool {
    match rule {
        PoolRule::Free => m.is_free || m.cost_per_1k == 0.0,
        PoolRule::Provider { value } => m.provider.eq_ignore_ascii_case(value),
        PoolRule::MaxCostPer1k { value } => m.cost_per_1k <= *value,
        PoolRule::Tier { value } => m.tier.eq_ignore_ascii_case(value),
        PoolRule::MinContext { value } => m.max_tokens >= *value,
        PoolRule::Unknown => false,
    }
}

/// Resolve the allowed-id set. Empty rules+includes ⇒ all enabled (minus excludes/disabled).
pub fn resolve(pool: &ModelPool, catalog: &[PoolModelView], enabled: &BTreeSet<String>) -> BTreeSet<String> {
    let open = pool.rules.is_empty() && pool.includes.is_empty();
    catalog.iter()
        .filter(|m| enabled.contains(&m.provider))
        .filter(|m| !pool.disabled_sources.iter().any(|s| s.eq_ignore_ascii_case(&m.provider)))
        .filter(|m| !pool.excludes.contains(&m.id))
        .filter(|m| open || pool.includes.contains(&m.id) || pool.rules.iter().any(|r| rule_matches(r, m)))
        .map(|m| m.id.clone())
        .collect()
}

/// Like `resolve`, but if the result is empty, fall back to all-enabled and report it
/// (never zero candidates). Returns (ids, fell_open).
pub fn resolve_with_fallback(pool: &ModelPool, catalog: &[PoolModelView], enabled: &BTreeSet<String>) -> (BTreeSet<String>, bool) {
    let ids = resolve(pool, catalog, enabled);
    if ids.is_empty() {
        (resolve(&ModelPool::default(), catalog, enabled), true)
    } else {
        (ids, false)
    }
}
```

- [ ] **Step 4: Run → PASS** (4 tests). Then clippy + fmt.
- [ ] **Step 5: Commit** `feat(model-pool): pure predicate (rules ∪ includes − excludes, empty=all, fail-open)`.

### Task 1.3 — load/save `~/.vox/config.toml` `[model_pool]` [SEQUENTIAL]

**Files:** Modify `crates/vox-config/src/model_pool.rs`, `crates/vox-config/src/lib.rs`

- [ ] **Step 1 (gate):** `rg -n "config.toml|home_dir|dirs::|fn .*path" crates/vox-config/src/env_parse.rs` — find the existing `~/.vox/config.toml` PATH resolver (reuse it; do NOT hardcode the path). Paste it.

- [ ] **Step 2: Write the failing test** (round-trip via a temp file injected through the resolved path helper — if the path helper isn't injectable, test `load_from_str`/`save_to_string` pure helpers instead):

```rust
    #[test]
    fn load_from_str_reads_model_pool_table() {
        let doc = r#"
[model_pool]
rules = [{ kind = "free" }]
includes = ["a/b"]
"#;
        let pool = ModelPool::load_from_str(doc).unwrap();
        assert_eq!(pool.rules.len(), 1);
        assert_eq!(pool.includes, vec!["a/b"]);
    }
    #[test]
    fn missing_table_is_default() {
        assert_eq!(ModelPool::load_from_str("foo = 1").unwrap(), ModelPool::default());
    }
```

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement** (parse the `[model_pool]` sub-table; `load()`/`save()` use the env_parse path helper found in Step 1):

```rust
impl ModelPool {
    /// Parse the `[model_pool]` table out of a full config.toml document string.
    pub fn load_from_str(doc: &str) -> Result<Self, toml::de::Error> {
        #[derive(Deserialize)]
        struct Wrap { #[serde(default)] model_pool: ModelPool }
        Ok(toml::from_str::<Wrap>(doc).map(|w| w.model_pool).unwrap_or_default())
    }
    /// Load from `~/.vox/config.toml` (returns default if absent/unreadable — fail-open).
    pub fn load() -> Self {
        // <use the env_parse config.toml path helper from Step 1>
        // let path = vox_config::env_parse::config_toml_path();  // confirm exact name in gate
        // std::fs::read_to_string(path).ok().and_then(|s| Self::load_from_str(&s).ok()).unwrap_or_default()
        Self::default() // replace body with the path-based read once the helper name is confirmed
    }
}
```
> NOTE for executor: the `load()` body MUST be wired to the real path helper confirmed in Step 1. If you cannot find an injectable path helper, STOP and report — do not invent a path.

- [ ] **Step 5: Run → PASS;** clippy + fmt. Add `pub mod model_pool;` to `lib.rs` if not already (Task 1.1).
- [ ] **Step 6: Commit** `feat(model-pool): load/save [model_pool] from ~/.vox/config.toml`.

---

# PHASE 2 — enabled-providers + scorer wiring + Tauri commands (Gemini-Flash)

### Task 2.1 — `list_enabled_providers()` [PARALLEL-SAFE]

**Files:** new shared fn (e.g. `crates/vox-config/src/providers.rs` or extend `inference.rs`); reuse the detection currently in doctor `llm_routing.rs`.

- [ ] **Step 1 (gate):** `rg -n "resolve_secret|SecretId::|is_empty|enabled" crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs` — paste the key-presence detection so you reuse, not duplicate, the SecretId list.

- [ ] **Step 2: Write the failing test** (env-driven: a provider with a key present is reported enabled):

```rust
    #[test]
    fn provider_with_key_present_is_enabled() {
        // SAFETY: test-only env mutation
        unsafe { std::env::set_var("OPENROUTER_API_KEY", "sk-test"); }
        let enabled = list_enabled_providers();
        assert!(enabled.contains("openrouter"));
        unsafe { std::env::remove_var("OPENROUTER_API_KEY"); }
    }
```

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement** `pub fn list_enabled_providers() -> std::collections::BTreeSet<String>` that maps each provider `SecretId` → resolves via `vox_secrets::resolve_secret(id).expose()`; non-empty ⇒ insert the provider string (`"openrouter"`, `"anthropic"`, `"openai"`, `"google"`, `"groq"`, `"mistral"`, `"deepseek"`, …) matching the SecretId list from Step 1. Always include `"ollama"`/`"mens"` if local routing is policy-allowed (confirm via `route_capability_policy` in the gate). Keep the provider strings identical to `ModelSpec.provider` values.

- [ ] **Step 5: Run → PASS;** clippy + fmt.
- [ ] **Step 6: Commit** `feat(model-pool): list_enabled_providers from credential presence`.

### Task 2.2 — apply the predicate at the scorer candidate boundary [SEQUENTIAL]

**Files:** Modify `crates/vox-orchestrator/src/models/registry.rs` (the candidate-enumeration site that feeds the scorer)

- [ ] **Step 1 (gate):** `rg -n "fn list_models|candidate|fn score|select|fn pick" crates/vox-orchestrator/src/models/registry.rs` — find the exact fn that enumerates candidates for scoring. Paste it. If candidate enumeration is elsewhere (scorer module), follow it there and STOP to report the real site if it differs from this file.

- [ ] **Step 2: Write the failing test** — a registry/scorer test where a `ModelPool` excluding all-but-one model yields exactly that one candidate:

```rust
    // Build a registry with 3 models; set a pool with includes=["only/this"]; assert the
    // candidate set used for scoring == {"only/this"}. (Map ModelSpec -> PoolModelView via
    // the helper added in Step 4.)
```
(Write the concrete test against the real candidate fn found in Step 1; assert the filtered candidate ids.)

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement:**
  - Add `fn to_pool_view(&self) -> vox_config::model_pool::PoolModelView` on `ModelSpec` (id, provider, cost_per_1k, max_tokens, is_free, tier=`format!("{:?}", self.capabilities.tier)`).
  - At the candidate site: `let pool = ModelPool::load(); let enabled = list_enabled_providers(); let (allowed, _fell_open) = resolve_with_fallback(&pool, &views, &enabled);` then `candidates.retain(|m| allowed.contains(&m.id))`.
  - Pins (premium_alias) resolve AFTER this filter — a pin not in `allowed` is skipped (scored selection within the pool wins).

- [ ] **Step 5: Run → PASS;** clippy + fmt.
- [ ] **Step 6: Commit** `feat(model-pool): hard-filter scorer candidates through the pool predicate`.

### Task 2.3 — Tauri commands `get_model_pool` / `set_model_pool` / `list_enabled_providers` [SEQUENTIAL]

**Files:** Create `crates/vox-gui/src/commands/model_pool.rs`; Modify `crates/vox-gui/src/main.rs` (handler list ~:153) + `crates/vox-gui/src/commands/mod.rs`

- [ ] **Step 1 (gate):** `rg -n "generate_handler|pub mod|commands::models" crates/vox-gui/src/main.rs crates/vox-gui/src/commands/mod.rs` — paste the handler-registration list + module declarations to match style.

- [ ] **Step 2: Write the failing test** — a Rust test that `get_model_pool()` returns the default for a missing config and `set_model_pool` round-trips through `load_from_str`/`save`. (vox-gui Rust tests: keep it lib-level; if Tauri-command testing is heavy, test the underlying `ModelPool` helpers and assert the command wrappers compile + are registered via a `main.rs` grep gate.)

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement** three `#[tauri::command] pub async fn`:
  - `get_model_pool() -> Result<ModelPoolDto, String>` → `ModelPool::load()` + resolved member ids (call `resolve_with_fallback` against `registry_from_cache().list_models()` mapped to views + `list_enabled_providers()`), returning `{ rules, includes, excludes, disabled_sources, member_ids, fell_open }`.
  - `set_model_pool(pool: ModelPoolDto) -> Result<(), String>` → validate + `ModelPool::save()`.
  - `list_enabled_providers_cmd() -> Result<Vec<String>, String>`.
  - Register all three in `main.rs` `generate_handler!`.

- [ ] **Step 5: Run → PASS;** `cargo clippy -p vox-gui --lib -- -D warnings`; fmt.
- [ ] **Step 6: Commit** `feat(model-pool): Tauri commands get/set pool + list_enabled_providers`.

---

# PHASE 3 — `ModelPoolView` GUI 🧑‍🎨 (Claude-Code)

### Task 3.1 — transport bindings [PARALLEL-SAFE]

**Files:** Modify `crates/vox-gui/ui/src/transport.ts`; Test: `transport.modelPool.test.ts`

- [ ] **Step 1 (gate):** `rg -n "list_model_cards|invoke<|export function" crates/vox-gui/ui/src/transport.ts | head` — match the existing wrapper style.
- [ ] **Step 2: Write the failing test** asserting `voxTransport.getModelPool` / `setModelPool` / `listEnabledProviders` exist and call the right command names (mock `invoke`).
- [ ] **Step 3: Run → FAIL.**
- [ ] **Step 4: Implement** the three wrappers + a `ModelPoolDto` TS type matching the Rust DTO (`rules`, `includes`, `excludes`, `disabled_sources`, `member_ids`, `fell_open`).
- [ ] **Step 5: Run → PASS;** `npx tsc --noEmit`. **Step 6: Commit** `feat(model-pool): GUI transport bindings`.

### Task 3.2 — `ModelPoolView` grouped picker [SEQUENTIAL]

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/Models/ModelPoolView.tsx` (+ test)

- [ ] **Step 1: Write the failing test** (jsdom; mock transport): renders models grouped by provider/source; a model whose id is in `member_ids` shows an "in pool" state; unconfigured providers (not in `listEnabledProviders`) render greyed with an "Add key" affordance; toggling a model calls `setModelPool` with it added to `includes`.
- [ ] **Step 2: Run → FAIL.**
- [ ] **Step 3: Implement** `ModelPoolView`:
  - fetch `list_model_cards` + `getModelPool` + `listEnabledProviders` (react-query or effect, mirror `ModelsView.tsx`).
  - group cards by `provider`; section header per source with a per-source on/off toggle (writes `disabled_sources`); greyed section + "Add key" (reuse the existing Clavis key flow used elsewhere — find via gate) for providers not in enabled list.
  - each card: in-pool indicator (rule-derived vs explicit), include/exclude toggle.
  - a rules editor: chips for active rules + an "add rule" control (kind dropdown: free/provider/max_cost_per_1k/tier/min_context + value); writes `rules`.
  - a `fell_open` warning banner when the resolved pool is empty.
- [ ] **Step 4: Run → PASS;** `npx tsc --noEmit`. **Step 5: Commit** `feat(model-pool): ModelPoolView grouped multi-source picker + rules editor`.

### Task 3.3 — register surface + Playwright screenshot [SEQUENTIAL]

**Files:** surface registry wiring (mirror how `models` surface is registered); `e2e/model-pool.spec.ts`

- [ ] **Step 1 (gate):** `rg -n "models|ModelsView|viewKey" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — match how the Models surface is mounted; add `model-pool` (or a tab within Models) the same way.
- [ ] **Step 2: Write a Playwright spec** (mirror `e2e/axis-brand.spec.ts` + `screenshots.spec.ts` `installTauriMock`): navigate to the pool surface, assert provider-group headers render, assert an enabled and a greyed provider section, screenshot `e2e/screens/_model-pool.png`.
- [ ] **Step 3: Run** `npx playwright test model-pool.spec.ts --project=chromium` → PASS.
- [ ] **Step 4:** view the screenshot, confirm grouping/alignment. **Step 5: Commit** `feat(model-pool): surface registration + Playwright screenshot`.

---

# PHASE 4 — retire `OPENROUTER_FREE_MODELS` (Gemini-Flash, GATED)

> ⚠️ **Layering gate.** `OPENROUTER_FREE_MODELS` lives in `vox-gamify` and feeds its own AI client fallback (3 consumers). Replacing it with a live-catalog `free` derivation requires `vox-gamify` to reach the catalog. This may cross a layer boundary.

### Task 4.1 — derive free models from the catalog [SEQUENTIAL]

- [ ] **Step 1 (gate, BLOCKING):**
  ```
  rg -n "vox-orchestrator|vox-config|catalog|registry_from_cache" crates/vox-gamify/Cargo.toml crates/vox-gamify/src/ai/client/*.rs
  rg -n "OPENROUTER_FREE_MODELS" crates/vox-gamify/src/ai/client/ctor.rs crates/vox-gamify/src/ai/client/transport.rs crates/vox-gamify/src/ai/provider.rs
  ```
  If `vox-gamify` does NOT already depend on a crate exposing the live catalog, **STOP and report** — do not add a heavy new dependency just for this. Options to surface to the human: (a) inject the free list from the caller that already has catalog access, or (b) keep the const as a typed fallback and add a `free`-rule derivation only in the pool predicate (Phase 1, already done). Default recommendation: **(b)** — the pool's `free` rule already makes free models dynamic for routing; the gamify const becomes a pure offline fallback.
- [ ] **Step 2:** If the gate permits a clean path: write a failing test that the free list is derived from a catalog fixture (filter `is_free || cost_per_1k==0`), implement, retire the const, update the 3 consumers. Else: implement recommendation (b) — annotate the const as `// fallback only; dynamic free selection is via the model-pool `free` rule` and leave it. Either way, commit with a message stating which path was taken.

---

## Self-Review (plan author)
- **Spec coverage:** semantics→1.2 (hard filter, empty=all); membership rules∪includes−excludes→1.2; sources/enabled→2.1; disabled-source→1.2; persistence→1.3; scorer integration→2.2; GUI grouped picker+rules+greyed/add-key+fell_open→3.2; free-rule→1.2 + 4.1; Tauri commands→2.3; tests→every task. ✅
- **Placeholder scan:** the only deferred bodies (`load()` path, scorer candidate site, free-const path) are guarded by BLOCKING pre-flight gates with explicit STOP instructions — not silent TODOs. ✅
- **Type consistency:** `ModelPool`/`PoolRule`/`PoolModelView`/`resolve`/`resolve_with_fallback`/`rule_matches` used identically across 1.1→2.2→2.3; `ModelPoolDto` (TS) mirrors the Rust DTO in 2.3/3.1. ✅
- **Scope:** single cohesive subsystem; Phase 4 explicitly gated to avoid a layering rabbit hole. ✅
