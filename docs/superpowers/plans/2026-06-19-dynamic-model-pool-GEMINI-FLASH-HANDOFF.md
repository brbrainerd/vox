# Dynamic Model Pool — Gemini Flash Handoff (2026-06-19)

Copy-paste brief for **Gemini Flash 3.5 in Antigravity**. Committed so the runner can read
it and every doc/path it references. This is the **backend half** (isolated, fully-testable
Rust); the scorer-wiring + GUI half runs in Claude Code (`…-dynamic-model-pool.md`) and does
not block you. Push the branch first if the runner is remote.

---

## ── COPY-PASTE BELOW THIS LINE ──

You are implementing the **backend of the dynamic model-pool** for the Vox repo. All
detail is committed in this checkout — read it from disk, do not rely on this message.

### Progress (audited 2026-06-19 — RESUME at G3)
- **G1 — DONE & committed** (`044b1b4ad3`): `crates/vox-config/src/model_pool.rs` (type + predicate) + `VoxConfig.model_pool` field; 7 unit tests pass.
- **G2 — DONE & committed** (`a8e0bc7179`): `list_enabled_providers()` in `model_pool.rs`.
- **G3 — IN PROGRESS, NEEDS FINISHING:** `crates/vox-gui/src/commands/model_pool.rs` exists (reviewed-correct, compiles) but is **untracked and UNWIRED** — its `pub mod model_pool;` (in `commands/mod.rs`) and the three `generate_handler!` entries (in `main.rs`) are NOT present in HEAD. **Your job: re-add `pub mod model_pool;` to `crates/vox-gui/src/commands/mod.rs`, register the 3 commands in `crates/vox-gui/src/main.rs`, build (`cargo test -p vox-gui` — bin crate, NO `--lib`), confirm the 2 tests in `model_pool.rs` run, then commit all three files together.** Do not rewrite the command code unless the build fails.
- **G4 — TODO** (annotate the const; annotation-only).

Skip G1/G2; start at G3.

### Read first (in this order)
1. `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md` — your own operating limits.
2. `docs/superpowers/specs/2026-06-19-dynamic-model-pool-design.md` — the design SSOT (read §2 locked decisions, §3 data model, §4 predicate).
3. `docs/superpowers/plans/2026-06-19-dynamic-model-pool-GEMINI-FLASH-HANDOFF.md` — THIS file. Execute tasks **G1 → G2 → G3 → G4 in order** (G1 first; the others depend on G1's types). Finish with the HANDBACK block.

### Operating rules (apply to EVERY task)
- **Atomic + green + committed.** A kill between tasks leaves a compiling, tested tree.
- **Verify before use:** every Step-1 `rg`/read is a BLOCKING gate — run it, paste output; if reality differs from this plan, **STOP and report**. Do not guess or invent APIs/paths.
- **Two-strike circuit breaker:** a step fails twice → STOP and report; do not thrash.
- **Stay in your lane.** You touch ONLY: `crates/vox-config/**`, `crates/vox-gui/src/commands/**`, `crates/vox-gui/src/main.rs`, `crates/vox-gamify/src/ai/constants.rs`. **Do NOT touch** `crates/vox-orchestrator/**` (the scorer wiring is Claude's), `crates/vox-gui/ui/**` (the GUI is Claude's), or any other crate. If a task seems to need a file outside this list, STOP and report.
- Verification ritual per task: `cargo test -p <crate> <filter>` → `cargo clippy -p <crate> -- -D warnings` → `cargo fmt -p <crate>`. **Never** `cargo fmt --all` (Windows arg-limit). For `vox-gui`: `cargo clippy -p vox-gui --lib -- -D warnings`. No stubs. (Tandem note: the repo `cargo` build-lock may be held by another session — if a build blocks, wait and retry; never bypass the lock.)
- A task is done only when its tests are green **and** committed.

### Resolved facts (verified 2026-06-19 — these are GROUND TRUTH, not guesses)
- `VoxConfig` struct: `crates/vox-config/src/config/vox_config.rs:13`. Its `impl` (incl. `load()` and `save()` which **merge-writes** `~/.vox/config.toml`) is in `crates/vox-config/src/config/impl_ops.rs` (`load()`:23, `save()`:213). **Persist the pool by adding a field to `VoxConfig` — do NOT write a second `config.toml` writer** (that would split-brain with the existing `[llm]` writer).
- `~/.vox/config.toml` layered resolution lives in `crates/vox-config/src/env_parse.rs` (scalar `resolve_config_*` only — not used here; you use `VoxConfig` instead).
- Provider credentials: `vox_secrets::resolve_secret(vox_secrets::SecretId::<X>).is_present()` (bool) — pattern already used at `crates/vox-gui/src/commands/llm_settings.rs:59` (`openrouter_key_status`). SecretIds: `OpenRouterApiKey`, `OpenaiApiKey`, `AnthropicApiKey`, `GeminiApiKey`, `GroqApiKey`, `MistralApiKey`, `DeepSeekApiKey`, `SambaNovaApiKey`, `TogetherApiKey`, `CerebrasApiKey`, `HuggingFaceToken` (confirm exact spelling: `rg -n "OpenaiApiKey|GeminiApiKey|GroqApiKey|MistralApiKey|DeepSeekApiKey" crates/vox-secrets/src/spec/registry/llm.rs`).
- Tauri commands register in `crates/vox-gui/src/main.rs` `tauri::generate_handler!` list (~:153, where `commands::models::list_model_cards` etc. are listed); modules in `crates/vox-gui/src/commands/mod.rs`.
- The live catalog: `crates/vox-gui/src/commands/models.rs` uses `registry_from_cache().list_models()` returning `ModelSpec { id, provider:String, cost_per_1k:f64, max_tokens:u64, is_free:bool, capabilities.tier:ModelTier, … }`. Map those fields into `PoolModelView` **inline** (do not depend on any orchestrator helper).
- `OPENROUTER_FREE_MODELS` (`crates/vox-gamify/src/ai/constants.rs:16`) has 3 consumers: `ai/client/ctor.rs:194`, `ai/client/transport.rs:223`, `ai/provider.rs:76`. `vox-gamify` depends on `vox-config` + `vox-mesh-types` but **NOT `vox-orchestrator`** — so it cannot reach the live catalog. **G4 is therefore an annotation-only task** (see G4), not a retirement.

### Provider-string convention
The `provider` string on `ModelSpec`/`ModelCardDto` and the keys you return from
`list_enabled_providers` MUST match (lowercase): `openrouter`, `openai`, `anthropic`,
`google`, `groq`, `mistral`, `deepseek`, `sambanova`, `together`, `cerebras`,
`huggingface`, plus `ollama`/`mens` for local/mesh. Confirm the real `provider` values
with `rg -n "provider:" crates/vox-orchestrator/src/models/spec.rs` and the bootstrap
catalog before finalizing G2.

### When done
Execute the **HANDBACK** section: confirm green, then emit the `## MODEL-POOL HANDBACK`
markdown block as your final message. Do NOT edit the ledger yourself.

## ── COPY-PASTE ABOVE THIS LINE ──

---

## Task G1 — `vox-config` `ModelPool` type + predicate + `VoxConfig` field

**Files:** Create `crates/vox-config/src/model_pool.rs`; Modify `crates/vox-config/src/lib.rs` (`pub mod model_pool;`), `crates/vox-config/src/config/vox_config.rs` (add field), tests inline.

- [ ] **Step 1 (gate):** `rg -n "pub mod" crates/vox-config/src/lib.rs | head` and `sed -n '1,40p' crates/vox-config/src/config/vox_config.rs` — confirm module style + the `VoxConfig` field list + its derives (must be `Serialize + Deserialize`).

- [ ] **Step 2: Failing tests** (predicate + parse):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    fn mv(id:&str,p:&str,c:f64,f:bool,t:&str,ctx:u64)->PoolModelView{PoolModelView{id:id.into(),provider:p.into(),cost_per_1k:c,max_tokens:ctx,is_free:f,tier:t.into()}}
    fn cat()->Vec<PoolModelView>{vec![mv("or/free","openrouter",0.0,true,"Free",8000),mv("an/opus","anthropic",0.015,false,"Elite",200000),mv("oa/mini","openai",0.002,false,"Fast",128000),mv("x/dep","openai",0.001,false,"Fast",4000)]}
    fn enabled()->BTreeSet<String>{["openrouter","anthropic","openai"].iter().map(|s|s.to_string()).collect()}
    #[test] fn empty_pool_all_enabled(){ assert_eq!(resolve(&ModelPool::default(),&cat(),&enabled()).len(),4); }
    #[test] fn rules_union_includes_minus_excludes(){
        let pool=ModelPool{rules:vec![PoolRule::Free,PoolRule::MaxCostPer1k{value:0.005}],includes:vec!["an/opus".into()],excludes:vec!["x/dep".into()],disabled_sources:vec![]};
        let g=resolve(&pool,&cat(),&enabled());
        assert!(g.contains("or/free")&&g.contains("oa/mini")&&g.contains("an/opus")&&!g.contains("x/dep"));
    }
    #[test] fn disabled_source_and_unenabled_drop(){
        let pool=ModelPool{disabled_sources:vec!["openrouter".into()],..Default::default()};
        let en:BTreeSet<String>=["openrouter","anthropic"].iter().map(|s|s.to_string()).collect();
        let g=resolve(&pool,&cat(),&en);
        assert!(!g.contains("or/free")&&!g.contains("oa/mini"));
    }
    #[test] fn empty_result_fails_open(){
        let pool=ModelPool{excludes:cat().iter().map(|m|m.id.clone()).collect(),..Default::default()};
        let (g,fo)=resolve_with_fallback(&pool,&cat(),&enabled()); assert!(fo&&g.len()==4);
    }
    #[test] fn parses_toml(){
        let p:ModelPool=toml::from_str(r#"rules=[{kind="free"},{kind="provider",value="anthropic"}]
includes=["a/b"]"#).unwrap();
        assert_eq!(p.rules.len(),2); assert_eq!(p.includes,vec!["a/b"]);
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-config model_pool`

- [ ] **Step 4: Implement** `crates/vox-config/src/model_pool.rs`:
```rust
//! Operator-curated allowed-model pool. Pure data + predicate; persistence is via the
//! `model_pool` field on `VoxConfig` (single writer to ~/.vox/config.toml).
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PoolRule {
    Free,
    Provider { value: String },
    MaxCostPer1k { value: f64 },
    Tier { value: String },
    MinContext { value: u64 },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelPool {
    pub rules: Vec<PoolRule>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub disabled_sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PoolModelView {
    pub id: String,
    pub provider: String,
    pub cost_per_1k: f64,
    pub max_tokens: u64,
    pub is_free: bool,
    pub tier: String,
}

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

pub fn resolve_with_fallback(pool: &ModelPool, catalog: &[PoolModelView], enabled: &BTreeSet<String>) -> (BTreeSet<String>, bool) {
    let ids = resolve(pool, catalog, enabled);
    if ids.is_empty() { (resolve(&ModelPool::default(), catalog, enabled), true) } else { (ids, false) }
}
```

- [ ] **Step 5: Add the `VoxConfig` field** in `config/vox_config.rs` (place near other tables; match existing `#[serde(default)]` style):
```rust
    /// Operator-curated allowed-model pool (see model_pool.rs). Empty ⇒ all enabled.
    #[serde(default)]
    pub model_pool: crate::model_pool::ModelPool,
```
  Add `pub mod model_pool;` to `lib.rs`. Confirm `VoxConfig::save()` already serializes the whole struct (so the new field round-trips) — if `save()` writes a hand-built subset rather than serializing `self`, STOP and report (the field must be persisted).

- [ ] **Step 6: Run → PASS** (5 tests); `cargo clippy -p vox-config -- -D warnings`; `cargo fmt -p vox-config`.
- [ ] **Step 7: Commit** `feat(model-pool): ModelPool type + predicate + VoxConfig.model_pool field`.

## Task G2 — `list_enabled_providers()`

**Files:** add to `crates/vox-config/src/model_pool.rs` (or a sibling `providers.rs`); reuse `vox_secrets`.

- [ ] **Step 1 (gate):** `rg -n "OpenaiApiKey|AnthropicApiKey|GeminiApiKey|GroqApiKey|MistralApiKey|DeepSeekApiKey|SambaNovaApiKey|TogetherApiKey|CerebrasApiKey|HuggingFaceToken" crates/vox-secrets/src/spec/registry/llm.rs` — confirm the exact `SecretId` variant names. And `rg -n "provider:" crates/vox-orchestrator/src/models/spec.rs` style / bootstrap catalog for the exact lowercase provider strings.

- [ ] **Step 2: Failing test:**
```rust
#[test] fn provider_with_key_is_enabled() {
    unsafe { std::env::set_var("OPENROUTER_API_KEY", "sk-test"); }
    assert!(list_enabled_providers().contains("openrouter"));
    unsafe { std::env::remove_var("OPENROUTER_API_KEY"); }
}
```

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement:**
```rust
/// Providers the operator has credentials for (key present), as lowercase strings
/// matching ModelSpec.provider. `ollama`/`mens` (local/mesh) are always candidates.
pub fn list_enabled_providers() -> BTreeSet<String> {
    use vox_secrets::SecretId::*;
    let mut out = BTreeSet::new();
    let pairs = [
        (OpenRouterApiKey, "openrouter"), (OpenaiApiKey, "openai"), (AnthropicApiKey, "anthropic"),
        (GeminiApiKey, "google"), (GroqApiKey, "groq"), (MistralApiKey, "mistral"),
        (DeepSeekApiKey, "deepseek"), (SambaNovaApiKey, "sambanova"), (TogetherApiKey, "together"),
        (CerebrasApiKey, "cerebras"), (HuggingFaceToken, "huggingface"),
    ];
    for (id, name) in pairs {
        if vox_secrets::resolve_secret(id).is_present() { out.insert(name.to_string()); }
    }
    out.insert("ollama".into()); out.insert("mens".into());
    out
}
```
  (Adjust variant names to match Step 1. If a SecretId doesn't exist, drop that row — do not invent one.)

- [ ] **Step 5: Run → PASS;** clippy + fmt.
- [ ] **Step 6: Commit** `feat(model-pool): list_enabled_providers from credential presence`.

## Task G3 — Tauri commands `get_model_pool` / `set_model_pool` / `list_enabled_providers_cmd`

**Files:** Create `crates/vox-gui/src/commands/model_pool.rs`; Modify `crates/vox-gui/src/commands/mod.rs` (+`pub mod model_pool;`), `crates/vox-gui/src/main.rs` (register 3 in `generate_handler!`).

- [ ] **Step 1 (gate):** `sed -n '150,160p' crates/vox-gui/src/main.rs` + `rg -n "registry_from_cache|pub mod" crates/vox-gui/src/commands/models.rs crates/vox-gui/src/commands/mod.rs` — match registration + how `registry_from_cache().list_models()` is called.

- [ ] **Step 2: Failing test** (in `model_pool.rs` `#[cfg(test)]`): map a fixture model list → `PoolModelView` inline and assert `resolve_with_fallback` member ids; and that `ModelPoolDto` serde round-trips. (Keep tests at the helper level; the `#[tauri::command]` wrappers are thin — verify registration via the Step-5 grep gate.)

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement:**
```rust
use serde::{Deserialize, Serialize};
use vox_config::model_pool::{ModelPool, PoolModelView, resolve_with_fallback, list_enabled_providers};

#[derive(Serialize, Deserialize, Default)]
pub struct ModelPoolDto {
    pub rules: Vec<vox_config::model_pool::PoolRule>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub disabled_sources: Vec<String>,
    #[serde(default)] pub member_ids: Vec<String>,
    #[serde(default)] pub fell_open: bool,
}

fn catalog_views() -> Vec<PoolModelView> {
    // reuse the same registry the model cards come from (see commands/models.rs)
    crate::commands::models::registry_from_cache().list_models().iter().map(|m| PoolModelView {
        id: m.id.clone(), provider: m.provider.clone(), cost_per_1k: m.cost_per_1k,
        max_tokens: m.max_tokens, is_free: m.is_free, tier: format!("{:?}", m.capabilities.tier),
    }).collect()
}

#[tauri::command]
pub async fn get_model_pool() -> Result<ModelPoolDto, String> {
    let pool = vox_config::VoxConfig::load().model_pool;
    let (ids, fell_open) = resolve_with_fallback(&pool, &catalog_views(), &list_enabled_providers());
    Ok(ModelPoolDto { rules: pool.rules, includes: pool.includes, excludes: pool.excludes,
        disabled_sources: pool.disabled_sources, member_ids: ids.into_iter().collect(), fell_open })
}

#[tauri::command]
pub async fn set_model_pool(pool: ModelPoolDto) -> Result<(), String> {
    let mut cfg = vox_config::VoxConfig::load();
    cfg.model_pool = ModelPool { rules: pool.rules, includes: pool.includes, excludes: pool.excludes, disabled_sources: pool.disabled_sources };
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_enabled_providers_cmd() -> Result<Vec<String>, String> {
    Ok(list_enabled_providers().into_iter().collect())
}
```
  (If `registry_from_cache` is private to `models.rs`, make it `pub(crate)` — that is the one allowed cross-module change; note it in the handback.)
  Register the three in `main.rs` `generate_handler!`; add `pub mod model_pool;` to `commands/mod.rs`.

- [ ] **Step 5: Run → PASS;** `cargo clippy -p vox-gui --lib -- -D warnings`; fmt. Confirm registration: `rg -n "model_pool::(get|set|list)" crates/vox-gui/src/main.rs`.
- [ ] **Step 6: Commit** `feat(model-pool): Tauri commands get/set pool + list_enabled_providers`.

## Task G4 — annotate `OPENROUTER_FREE_MODELS` (annotation-only; see Resolved facts)

`vox-gamify` cannot reach the live catalog (no `vox-orchestrator` dep), so retiring the const is out of scope. The dynamic free-model behavior is delivered by the pool's `free` rule (G1) in the main router instead.

- [ ] **Step 1:** In `crates/vox-gamify/src/ai/constants.rs`, above `OPENROUTER_FREE_MODELS`, add:
```rust
// Offline fallback list ONLY. Dynamic free-model selection for the main router is handled
// by the model-pool `free` rule (vox_config::model_pool); this const is the gamify AI
// client's last-resort fallback when no catalog/keys are available. Do NOT treat it as the
// source of truth for available free models.
```
- [ ] **Step 2:** Verify it still compiles: `cargo build -p vox-gamify`. **Step 3: Commit** `docs(model-pool): mark OPENROUTER_FREE_MODELS as offline fallback only`.

---

## HANDBACK (final step — emit verbatim, fill the fields)

````markdown
## MODEL-POOL HANDBACK → paste into Claude Code to update the ledger

```yaml
# --- AGH-NNNN ---   (Claude assigns the next free id)
id: AGH-NNNN
date: <YYYY-MM-DD>
plan: docs/superpowers/plans/2026-06-19-dynamic-model-pool-GEMINI-FLASH-HANDOFF.md
prompt_artifact: same
subsystem: dynamic-model-pool (backend)
target: gemini-3.5-flash / antigravity
delivered: [crates/vox-config/src/model_pool.rs, crates/vox-config/src/config/vox_config.rs, crates/vox-gui/src/commands/model_pool.rs, crates/vox-gamify/src/ai/constants.rs]
loc: <int>
outcome: <green|partial|failed>
verification: { tests: "<N> passed (vox-config <a>, vox-gui <b>)", clippy: <clean|warns>, fmt: ok }
errors_encountered:
  - { what: "<symptom or none>", root_cause: "<cause>", category: "<hallucinated-api|wrong-path|build-gate|fmt-gate|scope-creep|none>", who: <agent|plan> }
agent_deviations:
  - "<any file edited beyond the allowed lane, or 'none'>"
prompt_lessons:
  - "<1-3 lessons>"
commits: [<sha>, ...]
```

**Prose summary:** <2-4 sentences: what shipped, deviations, what Claude must verify (scorer wiring P2.2 consumes these types).>
````

---
### Notes for the human (not part of the prompt)
- Hand off **G1 alone first** (it's the foundation both halves depend on), review its ledger entry, then G2/G3/G4.
- Claude Code runs the scorer wiring (P2.2) + GUI (P3) in parallel — Claude's P2.2 needs G1 landed; Claude's P3 builds against the G3 command contract.
- On handback, Claude appends the ledger (next free `AGH-` id) + code-reviews the commits.
