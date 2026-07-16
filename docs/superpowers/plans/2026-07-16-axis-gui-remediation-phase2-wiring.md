# Axis GUI Remediation Phase 2 (Wiring) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Axis GUI a real harness: model selection proactively gated on API-key presence and live local-server health, the two dead routing engines deleted (fork F3), a per-backend availability panel + a chat model pick threaded to the daemon via `enqueue_hints.model_override`, honest hopper DTOs, a merge-view Tasks surface over both task stores (fork F1) with mark-done, and working session rename/archive + per-message model attribution.

**Architecture:** All gating lands inside the one exercised selection path — `decide()` in `vox-orchestrator` (candidate filter) plus the MCP resolver's `routing_allows` closure for local-health — with the reactive fallback chain untouched as the safety net. Health probing is substrate in `vox-actor-runtime::inference_env` (TTL cache, sync peek + async refresh) shared by the resolver and a new Tauri command; the health gate covers only the providers that server actually serves (`Ollama | PopuliMesh` — VoxLocal has its own endpoint + probe). The chat model pick threads through the one channel the daemon actually consumes: `enqueue_hints.model_override` (`tasks.rs:241-243`) → `AgentTask::model_override` (`apply_hints`, `tasks.rs:861-862`) → `StreamRoute::UserModelOverride` at dispatch (`runtime.rs:408-421`). The Tasks surface merges the SQLite hopper read (extended with its *actually persisted* fields) with the existing daemon-backed `list_orchestrator_tasks` read, origin-tagged in the frontend. Chat model attribution rides the already-broadcast `cost_incurred` agent event (`events.rs:274-277` carries `agent_id` + `model`), routed to the owning session by `sessionChatStore.resolveSessionForEvent` (which gains a `cost_incurred` branch — at runtime every frame goes through the session store, never `chatReducer` directly) and correlated through the existing `agentToTask`/`taskToRun` maps.

**Tech Stack:** Rust (tauri 2, tokio, serde, serial_test), vox-orchestrator / vox-orchestrator-mcp / vox-actor-runtime / vox-config / vox-gui crates; React 19 + TypeScript + Vitest + Testing Library (pnpm-managed at `crates/vox-gui/ui`, **pnpm not npm**); Playwright e2e via `e2e/lib/tauriMock.ts`. Windows dev box: never `cargo fmt --all` (use `cargo fmt -p <crate>`), never pipe cargo output to `head`/`grep` (redirect to a file if needed), never `cargo clippy --all-targets` across the workspace with vox-gui included (use `-p <crate>`, and for workspace sweeps `--exclude vox-gui`).

---

## Resolved decisions baked into this plan (verified against source 2026-07-16)

- **Item 1 insertion point (B3):** the candidate filter of `decide()` (`crates/vox-orchestrator/src/models/select.rs:97-124` and the exploration loop at `:126-155`), activating `ModelRegistry::key_is_present_for` (`registry.rs:272-275`). Chosen over folding `available_inference_providers()` into `routing_allows` (`resolve.rs:233`) because: (a) `decide()` is consumed by every structured-selection caller — the MCP chat resolver (`resolve.rs:362`), the GUI decision preview (`vox-gui/src/commands/models.rs:200`) — not just MCP chat; (b) the gate lands *before* scoring, so the scorer picks the best *available* model instead of a keyless pick being discarded post-hoc; (c) rejections surface in `rejection_reasons`, which the GUI already renders. The MCP resolver's non-`decide()` paths (hard pin, free-tier, cheapest fallback) keep their reactive fallback net.
- **Item 3 deletion scope (F3), caller-verified:** `resolve_chat_provider_route` / `resolve_chat_provider_route_impl` / `populi_model_plausible` have **zero production callers** (only their own `#[cfg(test)]` tests; verified by grep — `llm/cascade.rs`, `vox-research-shim`, and `vox-codegen`'s emitted fixture consume only `RouteResolutionInput::{mens_chat_model, openrouter_model}` + `chat_route_to_llm_config`, which stay). `ModelPool::resolve`/`resolve_with_fallback`/`rule_matches`/`list_enabled_providers` have zero consumers outside `vox-config` itself (the 2026-06-19 dynamic-model-pool plan's Tauri commands were never built). `VoxToml` (`toml_schema.rs:9-20`) is `#[serde(default)]` with no `deny_unknown_fields`, so removing the `model_pool` field is parse-safe for existing `~/.vox/config.toml` files; `persist.rs` merge-writes seeded from the existing file, so we also explicitly `remove("model_pool")` on save to retire the key.
- **Item 4 picker channel (honest wiring):** the chat model pick does **not** ride `set_active_model` (`crates/vox-gui/src/commands/models.rs:156`) — that command only sets `VOX_MODEL` in the *GUI* process env and writes an `active_model` DB preference nothing in any selection path reads; chat tasks execute in the separate `vox-orchestrator-d` daemon. The pick threads as `enqueue_hints.model_override`: `TaskEnqueueHints.model_override` (`crates/vox-orchestrator/src/types/tasks.rs:241-243`) → applied to `AgentTask` (`tasks.rs:861-862`) → consumed at dispatch as `StreamRoute::UserModelOverride` (`crates/vox-orchestrator/src/runtime.rs:408-421`). `set_active_model` stays as-is for the Models-surface display only.
- **Item 5 real-fields check:** the hopper SQLite row persists `item_id, intent, affinity_json, priority, source(json), session_id, state(json), submitted_at` (`sqlite_store.rs:30-61,85-96`). Therefore the DTO gains **`session_id`** (column), **`agent_id`** (inside `ItemState::Assigned { agent_id }` state JSON), **`remote_node`** (inside `IntakeSource::Mesh { node_id }` source JSON) — and **not** `depends_on`/`write_files`/`estimated_complexity`, which the hopper does not persist. Those three come alive via the merge-view (Task 10): orchestrator task-graph rows already carry them (`control_plane.rs:211-305`).
- **Item 6 merge-view (F1), mirroring the spec's recorded decision (spec Phase 2 item 6):** frontend union in TasksView. `list_orchestrator_tasks` (daemon `LIST_TASKS`, registered `main.rs:157`) already returns the full `TaskRowDto`; `hopper_list` is extended **only** with terminal `done` items (bounded, most-recent-N — Task 9) so the `completed` branch is reachable, and TasksView merges those rows with the `list_orchestrator_tasks` read, origin-tagged. **Scope limit (recorded in the spec):** `hopper_list` itself stays hopper-only — other consumers (`useAttentionInbox` blocked-count, the urbs harness read) keep hopper-only semantics by design, so gated orchestrator tasks show as blocked rows in the Tasks surface but are **not** counted in the attention strip. No dedupe hazard: chat submissions go to the orchestrator graph (`SUBMIT_TASK`), hopper items only from the TasksView composer/secretary — disjoint stores by construction (this reasoning is also why the union must NOT be pushed into `hopper_list` itself).
- **Item 7 model attribution source:** no task event carries a model id (`TaskCompleted` at `events.rs:179-186` has none), but `CostIncurred { agent_id, provider, model, .. }` (`events.rs:274-277`) is broadcast on the same `vox://agent-events` stream. At runtime App routes **every** agent-event frame through `sessionChatStore` (`App.tsx:443-452` → `dispatchSessionChat({ type: 'agentEvent', … })`), so `resolveSessionForEvent` must gain a `cost_incurred` branch (its agent-id-scan group) or the frame is dropped before `chatReducer` ever sees it. Wire `cost_incurred → modelId` in both layers; persist via the already-tested `model_id` payload field in `chat.rs`.

**Suggested PR series** (each task = one commit; each PR independently green): PR-A Tasks 1–3 (selection gating) · PR-B Tasks 4–5 (engine deletions) · PR-C Tasks 6–7 (availability panel + picker) · PR-D Tasks 8–10 (Tasks surface) · PR-E Tasks 11–12 (session management + model badge).

All `cargo` commands run from repo root `C:\Users\Owner\vox`. All `pnpm` commands use `pnpm -C crates/vox-gui/ui …`.

---

### Task 1 — Key-gated candidate filter in `decide()`

**Files:**
- `crates/vox-orchestrator/src/models/registry.rs:272-275` (activate `key_is_present_for`)
- `crates/vox-orchestrator/src/models/select.rs:97-124` (main candidate loop), `:126-155` (exploration loop), tests module `:824+`

Current code being changed — `registry.rs:272-275`:

```rust
    #[allow(dead_code)]
    fn key_is_present_for(m: &ModelSpec) -> bool {
        provider_secret_is_available(&m.provider_type)
    }
```

and the `decide()` candidate loop, `select.rs:97-124` (abridged):

```rust
    for m in all {
        if !scope_allows(m.provider_type.clone(), request.candidate_scope) { … continue; }
        if !request.required_capabilities.iter().all(|cap| m.capabilities.supports(*cap)) { … continue; }
        if !supports_intent_constraints(&m, &request.intent) {
            rejection_reasons.push(format!("{} rejected: intent constraints", m.id));
            continue;
        }
        let conf = confidence_state_for_model(&m);
        if !is_routing_eligible(conf) { … continue; }
        candidates.push(m);
    }
```

- [ ] **Step 1: Failing test.** Append to the existing `mod tests` in `crates/vox-orchestrator/src/models/select.rs` (after `select_with_empty_policy_falls_through_to_cascade`, line ~1258), following the file's `#[serial]` + unsafe-env idiom (`select.rs:887-923`) and the env-key idiom of `models/tests.rs:133-163`, with the `ModelSpec` fixture shape from `models/policy.rs:411-431`:

```rust
    // ── Phase-2 wiring: key-gated candidate filter (B3) ─────────────────────
    fn key_gate_spec(id: &str, provider_type: ProviderType) -> ModelSpec {
        crate::models::ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type,
            max_tokens: 32_000,
            cost_per_1k: 0.001,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.001,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::StrengthTag::Generalist],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            // UserConfig ⇒ ModelConfidence::Confirmed ⇒ routing-eligible
            // (discovery_pipeline.rs:25-34), so only the key gate is under test.
            pricing_source: crate::models::spec::PricingSource::UserConfig,
            supported_parameters: vec![],
        }
    }

    #[test]
    #[serial]
    #[allow(unsafe_code)]
    fn key_gate_excludes_keyless_provider_and_reports_rejection() {
        // SAFETY: #[serial] serializes env mutation; mirrors models/tests.rs:133-140.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec("anthropic-direct-test", ProviderType::Anthropic));
        registry.register(key_gate_spec("ollama-local-test", ProviderType::Ollama));
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        let d = decide(&req, &registry).expect("local (keyless-OK) candidate remains");
        assert_eq!(d.selected_model, "ollama-local-test");
        assert!(
            d.rejection_reasons
                .iter()
                .any(|r| r.contains("anthropic-direct-test") && r.contains("missing provider key")),
            "keyless provider must be rejected with a key reason: {:?}",
            d.rejection_reasons
        );
    }

    #[test]
    #[serial]
    #[allow(unsafe_code)]
    fn key_gate_admits_provider_when_key_present() {
        let prior = std::env::var("ANTHROPIC_API_KEY").ok();
        // SAFETY: #[serial]; prior value restored below.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec("anthropic-direct-test", ProviderType::Anthropic));
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        let d = decide(&req, &registry).expect("keyed candidate is eligible");
        assert_eq!(d.selected_model, "anthropic-direct-test");
        #[allow(unsafe_code)]
        unsafe {
            match prior {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
    }
```

  Note: like the pre-existing `key_guard_tests`, this assumes the CI/dev environment does not hold a real Anthropic key in Clavis outside env vars — `models/tests.rs:134` already relies on exactly this.
- [ ] **Step 2: Watch it fail.**
  Run: `cargo test -p vox-orchestrator key_gate_`
  Expected (representative): `key_gate_excludes_keyless_provider_and_reports_rejection` FAILED — `assertion failed … keyless provider must be rejected with a key reason: []` (today `decide()` never emits a key rejection and happily selects `anthropic-direct-test`).
- [ ] **Step 3: Implement.**
  1. `registry.rs:272-275` — activate the filter:

```rust
    /// Credential gate used by the canonical selector: true iff the provider's
    /// primary key is resolvable right now (local providers always pass).
    pub(crate) fn key_is_present_for(m: &ModelSpec) -> bool {
        provider_secret_is_available(&m.provider_type)
    }
```

  (delete the `#[allow(dead_code)]` line; make it `pub(crate)`.)
  2. `select.rs` main candidate loop — insert between the `supports_intent_constraints` check (line 110-113) and the confidence check (line 114):

```rust
        if !ModelRegistry::key_is_present_for(&m) {
            rejection_reasons.push(format!("{} rejected: missing provider key", m.id));
            continue;
        }
```

  3. Same gate in the exploration loop (after the `supports_intent_constraints` check at `select.rs:140-142`, without the `rejection_reasons` push — that loop pushes none):

```rust
                if !ModelRegistry::key_is_present_for(&m) {
                    continue;
                }
```

- [ ] **Step 4: Watch it pass.** `cargo test -p vox-orchestrator key_gate_` → `test result: ok. 2 passed`. Then the full selection suite: `cargo test -p vox-orchestrator models::` → all green (the pre-existing `decide_*` tests are tolerant `if let Some(..)` shapes and survive a keyless environment).
- [ ] **Step 5: Lint + commit.**
  `cargo clippy -p vox-orchestrator -- -D warnings` and `cargo fmt -p vox-orchestrator`
  `git add crates/vox-orchestrator/src/models/registry.rs crates/vox-orchestrator/src/models/select.rs && git commit -m "feat(orchestrator): key-gate decide() candidates via key_is_present_for (B3)"`

### Task 2 — TTL-cached local-server probe substrate

**Files:**
- `crates/vox-actor-runtime/src/inference_env.rs` (probe lives at `:124-206`; add cache + two functions; tests module at `:237-275`)

Current probe signature (`inference_env.rs:124`): `pub async fn probe_populi_capabilities(base_url: &str) -> PopuliCapabilitySnapshot` — uncached, 5s timeout.

- [ ] **Step 1: Failing test.** Append to `mod tests` in `inference_env.rs` (idiom: `probe_unbound_port_unreachable` at `:250-255`):

```rust
    #[tokio::test]
    async fn cached_probe_stores_snapshot_and_sync_peek_respects_ttl() {
        let base = "http://127.0.0.1:1"; // guaranteed-unbound port, like probe_unbound_port_unreachable
        // No entry before the first probe.
        assert!(last_populi_probe(base, std::time::Duration::from_secs(60)).is_none());
        let s = probe_populi_capabilities_cached(base, std::time::Duration::from_secs(60)).await;
        assert!(!s.reachable);
        // Fresh entry is peekable without awaiting…
        let peeked = last_populi_probe(base, std::time::Duration::from_secs(60))
            .expect("snapshot cached after probe");
        assert!(!peeked.reachable);
        assert_eq!(peeked.base_url, s.base_url);
        // …and a zero TTL treats it as stale.
        assert!(last_populi_probe(base, std::time::Duration::ZERO).is_none());
    }
```

- [ ] **Step 2: Watch it fail.** `cargo test -p vox-actor-runtime inference_env` → compile error: ``cannot find function `last_populi_probe` in this scope`` (expected — functions don't exist yet).
- [ ] **Step 3: Implement.** Add below `probe_populi_capabilities` (after line 206):

```rust
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static POPULI_PROBE_CACHE: OnceLock<Mutex<HashMap<String, (Instant, PopuliCapabilitySnapshot)>>> =
    OnceLock::new();

fn probe_cache() -> &'static Mutex<HashMap<String, (Instant, PopuliCapabilitySnapshot)>> {
    POPULI_PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Non-blocking peek at the last probe of `base_url`, if fresher than `ttl`.
/// Sync so credential/health gates on synchronous selection paths can consult
/// it without an executor. `None` means "unknown" — callers must treat unknown
/// as allowed (optimistic) and trigger a refresh via
/// [`probe_populi_capabilities_cached`].
pub fn last_populi_probe(base_url: &str, ttl: Duration) -> Option<PopuliCapabilitySnapshot> {
    let key = base_url.trim_end_matches('/').to_string();
    let cache = probe_cache().lock().ok()?;
    let (at, snap) = cache.get(&key)?;
    (at.elapsed() <= ttl).then(|| snap.clone())
}

/// [`probe_populi_capabilities`] with a short-TTL process-wide cache: returns
/// the cached snapshot when fresher than `ttl`, otherwise probes and stores.
pub async fn probe_populi_capabilities_cached(
    base_url: &str,
    ttl: Duration,
) -> PopuliCapabilitySnapshot {
    if let Some(snap) = last_populi_probe(base_url, ttl) {
        return snap;
    }
    let snap = probe_populi_capabilities(base_url).await;
    if let Ok(mut cache) = probe_cache().lock() {
        cache.insert(snap.base_url.clone(), (Instant::now(), snap.clone()));
    }
    snap
}
```

  (Place the `use` items with the existing imports at the top of the file rather than mid-file; shown inline here for locality.)
- [ ] **Step 4: Watch it pass.** `cargo test -p vox-actor-runtime inference_env` → `test result: ok.` including `cached_probe_stores_snapshot_and_sync_peek_respects_ttl`.
- [ ] **Step 5: Lint + commit.**
  `cargo clippy -p vox-actor-runtime -- -D warnings` and `cargo fmt -p vox-actor-runtime`
  `git add crates/vox-actor-runtime/src/inference_env.rs && git commit -m "feat(actor-runtime): TTL-cached populi capability probe with sync peek (B4 substrate)"`

### Task 3 — Local-server health gating in the MCP chat resolver

**Files:**
- `crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs` (new)
- `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs:6-14` (add `pub(crate) mod local_health;`)
- `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:233-235` (`routing_allows` closure)

Current gate closure (`resolve.rs:233-235`) — applied at every acceptance point of `resolve_mcp_chat_model_sync_inner` (hard pin `:267`, secrets pin `:275`, free-tier `:313,328`, VoxLocal-preferred `:344`, `decide()` acceptance `:364`, cheapest fallbacks `:372-379`):

```rust
    let routing_allows = |m: &ModelSpec| {
        routing_policy.provider_filter_allows(m) && provider_allowed_by_route_policy(m)
    };
```

**Gate scope (deliberate):** the probe hits `vox_config::inference::local_ollama_populi_base_url()`, which serves **only** `Ollama` and `PopuliMesh` candidates (`model_resolution.rs:93-96` routes PopuliMesh to that base). `VoxLocal` is a *different* server on a different port — `VOX_LOCAL_ENDPOINT`, default `http://127.0.0.1:7863`, with its own probe `providers::probe_vox_local_health` (`crates/vox-orchestrator-mcp/src/llm_bridge/providers/probe.rs:17-24`) already guarding the call path inside `vox_local_generate` (`llm_bridge/mod.rs:52-66`). Gating VoxLocal on the Ollama/populi probe would wrongly exclude a healthy MENS server whenever Ollama is down — including the VoxLocal-preferred acceptance branch at `resolve.rs:337-348`. So VoxLocal is explicitly **ungated** here.

- [ ] **Step 1: Failing tests.** Create `crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs` containing only the tests first (module registered in Step 3's `mod.rs` edit — do that edit now so the tests compile into the tree):

```rust
//! Short-TTL local-backend (Ollama/PopuliMesh) health gate for the synchronous
//! MCP model resolver. Peeks the vox-actor-runtime probe cache; unknown health
//! is optimistic (allowed) — the reactive fallback chain in the infer loop
//! remains the safety net for the first call after startup. VoxLocal is NOT
//! gated here: it runs on its own server (`VOX_LOCAL_ENDPOINT`) with its own
//! probe (`providers::probe_vox_local_health`) already in the call path.

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::models::{ModelCapabilities, ModelSpec, ProviderType};

    #[test]
    fn unknown_health_is_optimistic_and_only_confirmed_down_excludes() {
        assert!(local_gate_allows(&ProviderType::Ollama, None));
        assert!(local_gate_allows(&ProviderType::PopuliMesh, Some(true)));
        assert!(!local_gate_allows(&ProviderType::Ollama, Some(false)));
        assert!(!local_gate_allows(&ProviderType::PopuliMesh, Some(false)));
        // VoxLocal is served by a different server (VOX_LOCAL_ENDPOINT) whose
        // own probe guards the call path — never gated on the populi probe.
        assert!(local_gate_allows(&ProviderType::VoxLocal, Some(false)));
        // Cloud providers are never health-gated by this check.
        assert!(local_gate_allows(&ProviderType::OpenRouter, Some(false)));
    }

    // Wiring test 1 (cache plumbing): seed a known-down snapshot through the
    // Task 2 substrate (unbound port ⇒ unreachable) and assert the peek reads
    // it back through the same TTL + key normalization the resolver will use.
    #[tokio::test]
    async fn cache_plumbing_reads_the_shared_probe_cache() {
        let base = "http://127.0.0.1:1"; // guaranteed-unbound, like inference_env's own test
        let snap = vox_actor_runtime::inference_env::probe_populi_capabilities_cached(
            base,
            LOCAL_HEALTH_TTL,
        )
        .await;
        assert!(!snap.reachable);
        assert_eq!(local_backend_health_for(base), Some(false));
        // An unprobed URL is unknown (peek returns None; only a background
        // refresh fires) — the optimistic path.
        assert_eq!(local_backend_health_for("http://127.0.0.1:2"), None);
    }

    fn gate_spec(id: &str, provider_type: ProviderType) -> ModelSpec {
        // Fixture idiom: provider_endpoints.rs:102-120.
        ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type,
            max_tokens: 8_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: vox_orchestrator::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    // Wiring test 2 (resolver gate): `local_candidate_allowed` is exactly what
    // the `routing_allows` closure calls — drive it through the test-only
    // health override so a botched health lookup or provider-match can't hide
    // behind "unknown ⇒ optimistic".
    #[test]
    fn resolver_gate_excludes_populi_backed_candidates_when_confirmed_down() {
        set_test_health_override(Some(Some(false)));
        assert!(!local_candidate_allowed(&gate_spec("ollama-m", ProviderType::Ollama)));
        assert!(!local_candidate_allowed(&gate_spec("mesh-m", ProviderType::PopuliMesh)));
        assert!(local_candidate_allowed(&gate_spec("vox-m", ProviderType::VoxLocal)));
        assert!(local_candidate_allowed(&gate_spec("or-m", ProviderType::OpenRouter)));
        set_test_health_override(None);
        // Override cleared ⇒ real path; no fresh probe of the config base URL
        // in a unit-test env ⇒ unknown ⇒ optimistic.
        assert!(local_candidate_allowed(&gate_spec("ollama-m", ProviderType::Ollama)));
    }
}
```

- [ ] **Step 2: Watch it fail.** `cargo test -p vox-orchestrator-mcp local_health` → compile error: ``cannot find function `local_gate_allows` in this scope`` (and `local_backend_health_for`, `local_candidate_allowed`, `set_test_health_override`, `LOCAL_HEALTH_TTL`).
- [ ] **Step 3: Implement.** Fill in `local_health.rs` above the test module:

```rust
use std::time::Duration;

use vox_orchestrator::models::{ModelSpec, ProviderType};

/// How long a probe result is trusted before a re-probe is triggered.
const LOCAL_HEALTH_TTL: Duration = Duration::from_secs(15);

/// Providers served by the shared Ollama/populi local server
/// (`vox_config::inference::local_ollama_populi_base_url()`). VoxLocal is
/// deliberately absent: different server, different port, own probe (F5).
fn is_populi_backed_local(p: &ProviderType) -> bool {
    matches!(p, ProviderType::Ollama | ProviderType::PopuliMesh)
}

/// Pure decision core (unit-tested): `health` = `Some(reachable)` from a fresh
/// probe, `None` = unknown. Unknown ⇒ allowed.
fn local_gate_allows(provider: &ProviderType, health: Option<bool>) -> bool {
    !is_populi_backed_local(provider) || health != Some(false)
}

/// Fresh-cached reachability of `base_url`; `None` = no fresh probe. A stale /
/// missing entry fires a non-blocking background refresh when a tokio runtime
/// is available (the MCP server always runs inside one). Parameterized on the
/// base URL so the cache plumbing is unit-testable without touching config.
fn local_backend_health_for(base_url: &str) -> Option<bool> {
    if let Some(snap) =
        vox_actor_runtime::inference_env::last_populi_probe(base_url, LOCAL_HEALTH_TTL)
    {
        return Some(snap.reachable);
    }
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let base = base_url.to_string();
        handle.spawn(async move {
            let _ = vox_actor_runtime::inference_env::probe_populi_capabilities_cached(
                &base,
                LOCAL_HEALTH_TTL,
            )
            .await;
        });
    }
    None
}

fn local_backend_health() -> Option<bool> {
    local_backend_health_for(&vox_config::inference::local_ollama_populi_base_url())
}

/// Test-only seam: `Some(health)` forces `local_candidate_allowed` to see that
/// health value; `None` restores the real cache-peek path.
#[cfg(test)]
static TEST_HEALTH_OVERRIDE: std::sync::Mutex<Option<Option<bool>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn set_test_health_override(v: Option<Option<bool>>) {
    *TEST_HEALTH_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = v;
}

/// Gate consulted by the resolver's `routing_allows`: Ollama/PopuliMesh
/// candidates are offered only while their shared local server is not
/// known-down. VoxLocal and cloud providers always pass.
pub(crate) fn local_candidate_allowed(m: &ModelSpec) -> bool {
    #[cfg(test)]
    if let Some(overridden) = *TEST_HEALTH_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
    {
        return local_gate_allows(&m.provider_type, overridden);
    }
    local_gate_allows(&m.provider_type, local_backend_health())
}
```

  Then register the module — `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs`, after line 9 (`mod limits;`): `pub(crate) mod local_health;`
  Then extend the closure at `resolve.rs:233-235`:

```rust
    let routing_allows = |m: &ModelSpec| {
        routing_policy.provider_filter_allows(m)
            && provider_allowed_by_route_policy(m)
            && crate::llm_bridge::local_health::local_candidate_allowed(m)
    };
```

  Confirm `vox-actor-runtime` is already a dependency of `vox-orchestrator-mcp` (it is — `resolve.rs:3` imports `vox_actor_runtime::model_resolution`). The `#[tokio::test]` needs tokio's `macros`+`rt` features available to the crate's tests — the crate is an async server and already runs tokio; if the attribute doesn't resolve, add `tokio = { workspace = true, features = ["macros", "rt"] }` under `[dev-dependencies]`.
- [ ] **Step 4: Watch it pass.** `cargo test -p vox-orchestrator-mcp local_health` → `test result: ok. 3 passed` (truth table + cache plumbing + resolver gate — no silent skips). Then `cargo test -p vox-orchestrator-mcp model_route_policy` → pre-existing resolver tests stay green (test env has no fresh probe of the config base URL ⇒ unknown ⇒ optimistic, behavior unchanged).
- [ ] **Step 5: Lint + commit.**
  `cargo clippy -p vox-orchestrator-mcp -- -D warnings` and `cargo fmt -p vox-orchestrator-mcp`
  `git add crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs && git commit -m "feat(mcp): health-gate Ollama/PopuliMesh candidates via cached populi probe (B4); VoxLocal ungated"`
  (Include `crates/vox-orchestrator-mcp/Cargo.toml` in the `git add` if the tokio dev-dependency tweak from Step 3 was needed.)

### Task 4 — Delete dead engine 1: `resolve_chat_provider_route` (fork F3)

**Files:**
- `crates/vox-actor-runtime/src/model_resolution.rs:80-277` (resolver impl + `populi_model_plausible` at `:67-72`), module doc `:1-12`, `RouteResolutionInput` fields `:24-41,49-62`, resolver tests `:385-436`, `:463-518`, `:570-609`

- [ ] **Step 1: Re-verify zero production callers (guard against drift since this plan was written).**
  `grep -rn "resolve_chat_provider_route" C:/Users/Owner/vox/crates --include=*.rs` → hits only inside `crates/vox-actor-runtime/src/model_resolution.rs`. If any other hit appears, STOP and re-scope.
  Also verify per-field before deleting each: `grep -rn "manual_model\|manual_base_url\|manual_bearer\|prefer_populi_when_gpu\|populi_probe\|hf_dedicated_chat_url\|hf_dedicated_chat_model\|hf_router_model" C:/Users/Owner/vox/crates --include=*.rs` → expected hits: `model_resolution.rs` itself **plus two same-named vox-config getter hits** — `crates/vox-config/src/inference.rs:262` (`pub fn hf_dedicated_chat_model()`) and its re-export at `crates/vox-config/src/lib.rs:62`. Those are config accessor *functions*, not `RouteResolutionInput` field consumers — they stay (they merely lose their last in-repo caller when the resolver's `Default` drops the hf fields). Any hit beyond those: STOP and re-scope. (As of writing, `cascade.rs`, `vox-research-shim/{verifier,planner,claims,stages,web_gather}.rs` and `vox-codegen/src/codegen_rust/emit/ai_fixture/llm.rs` use only `RouteResolutionInput::default()`, `.openrouter_model`, `.mens_chat_model`.)
- [ ] **Step 2: Delete.** In `model_resolution.rs`:
  1. Delete `populi_model_plausible` (`:67-72`), `resolve_chat_provider_route_impl` (`:80-271`), and `resolve_chat_provider_route` (`:273-277`).
  2. Slim `RouteResolutionInput` to the two fields its remaining consumers use, and its `Default` accordingly:

```rust
/// Model preferences threaded into the research cascade builders
/// ([`crate::llm::cascade`]). The former 7-way provider-route resolver that
/// consumed the full struct was deleted 2026-07-16 (Axis GUI remediation F3);
/// the single exercised selection path is `vox_orchestrator::models::decide()`
/// + the reactive fallback chain.
#[derive(Debug, Clone)]
pub struct RouteResolutionInput {
    /// Model tag to use with local Mens/Ollama when that lane is offered.
    pub mens_chat_model: String,
    /// Preferred OpenRouter model when that lane is offered.
    pub openrouter_model: String,
}

impl Default for RouteResolutionInput {
    fn default() -> Self {
        Self {
            mens_chat_model: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliModel)
                .expose()
                .filter(|s: &&str| !s.trim().is_empty())
                .map(|s: &str| s.to_string())
                .unwrap_or_else(|| "default-model".to_string()),
            openrouter_model: vox_config::inference::openrouter_chat_model_preference(),
        }
    }
}
```

  3. Update the module doc (`:1-12`) — replace the "Single **policy-shaped** resolver: manual → Mens …" headline with a description of what remains: `LlmConfig` conversion (`chat_route_to_llm_config`), telemetry labels, and `RouteResolutionInput` for the cascade.
  4. Delete the resolver tests: `manual_wins` (`:385-414`), `openrouter_id_without_base` (`:416-436`), `dedicated_endpoint_before_shared_router_when_token_present` (`:463-495`), `router_when_no_dedicated_fields` (`:497-518`), `selector_model_env_precedes_default_cascade` (`:570-609`). Keep `llm_config_ollama_chat_url_trimmed`, `llm_config_hf_router_matches_inference_env`, `telemetry_labels_openrouter_variant`, `route_backend_*` — they cover the surviving `chat_route_to_llm_config` / label functions (this also keeps the `eval_matrix.rs` `"vox_runtime_model_resolution_tests"` filter non-empty).
  5. Remove the now-unused import `use crate::inference_env::{self, PopuliCapabilitySnapshot};` → keep only what `chat_route_to_llm_config` tests need (`inference_env` is still used by `resolve_huggingface_router` in the kept test; adjust imports until clippy is clean).
- [ ] **Step 3: Prove the workspace still builds.**
  `cargo test -p vox-actor-runtime model_resolution` → kept tests pass.
  `cargo test -p vox-actor-runtime cascade` → cascade tests pass (they construct `RouteResolutionInput::default()` and read the two kept fields).
  `cargo build -p vox-research-shim -p vox-codegen -p vox-orchestrator-mcp` → green (compile-proof that no consumer referenced the deleted fields).
- [ ] **Step 4: Lint + commit.**
  `cargo clippy -p vox-actor-runtime -- -D warnings` and `cargo fmt -p vox-actor-runtime`
  `git add crates/vox-actor-runtime/src/model_resolution.rs && git commit -m "refactor(actor-runtime): delete dead resolve_chat_provider_route engine (B5/F3)"`

### Task 5 — Delete dead engine 2: `ModelPool` + config field (fork F3)

**Files:**
- `crates/vox-config/src/model_pool.rs` (delete entire file)
- `crates/vox-config/src/lib.rs:18` (`pub mod model_pool;`)
- `crates/vox-config/src/config/vox_config.rs:42-44` (field), `:90` (Default)
- `crates/vox-config/src/config/toml_schema.rs:19` (parse field)
- `crates/vox-config/src/config/impl_ops.rs:310-312` (merge), `:509-541` (test `reads_and_saves_model_pool`)
- `crates/vox-config/src/config/persist.rs:115-117` (save)

Current field (`vox_config.rs:42-44`):

```rust
    /// Operator-curated allowed-model pool (see model_pool.rs). Empty ⇒ all enabled.
    #[serde(default)]
    pub model_pool: crate::model_pool::ModelPool,
```

- [ ] **Step 1: Failing back-compat test.** Replace `reads_and_saves_model_pool` (`impl_ops.rs:509-541`) with a legacy-tolerance test (keep the surrounding test-module idiom — it writes a temp `config.toml` and round-trips `VoxConfig`; reuse its exact temp-dir/save helpers, only the body changes):

```rust
    #[test]
    fn tolerates_and_retires_legacy_model_pool_table() {
        // A pre-2026-07-16 config.toml may still contain [model_pool] (engine
        // deleted per Axis GUI remediation F3). It must parse fine and the key
        // must be dropped on the next save.
        let toml = r#"
[vox]
model = "anthropic/claude-sonnet-4"

[model_pool]
rules = [{ kind = "free" }]
includes = ["inc-1"]
"#;
        // …load via the same path reads_and_saves_model_pool used…
        // assert: cfg.model = "anthropic/claude-sonnet-4" (unknown table ignored)
        // then cfg.save() to the temp path and assert the re-read file text
        // does NOT contain "model_pool".
    }
```

  (Write the real body against the existing helper in that test module — it currently constructs the path with a tempdir and calls the same load/save pair; mirror it exactly. The assertion `!saved_text.contains("model_pool")` is the new behavior under test.)
- [ ] **Step 2: Watch it fail.** `cargo test -p vox-config tolerates_and_retires_legacy_model_pool_table` → fails: saved file still contains `model_pool` (persist.rs currently re-serializes the field, and the merge-write preserves the key).
- [ ] **Step 3: Implement the deletion.**
  1. Delete `crates/vox-config/src/model_pool.rs`; remove `pub mod model_pool;` from `lib.rs:18`.
  2. `vox_config.rs`: delete lines 42-44 (field) and line 90 (`model_pool: Default::default(),`).
  3. `toml_schema.rs`: delete line 19 (`pub(super) model_pool: Option<crate::model_pool::ModelPool>,`).
  4. `impl_ops.rs`: delete the merge block at 310-312 (`if let Some(v) = parsed.model_pool { self.model_pool = v; }`).
  5. `persist.rs`: replace lines 115-117 —

```rust
    let pool_val = toml::Value::try_from(&cfg.model_pool)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    root.insert("model_pool".to_string(), pool_val);
```

  with:

```rust
    // model_pool engine deleted 2026-07-16 (Axis GUI remediation F3): drop the
    // legacy key on save so it doesn't linger in config.toml forever.
    root.remove("model_pool");
```

- [ ] **Step 4: Watch it pass.** `cargo test -p vox-config` → all green including `tolerates_and_retires_legacy_model_pool_table`. Then compile-prove no external consumer existed: `cargo build -p vox-cli -p vox-gui -p vox-orchestrator` (exit 0).
- [ ] **Step 5: Lint + commit.**
  `cargo clippy -p vox-config -- -D warnings` and `cargo fmt -p vox-config`
  `git add -A crates/vox-config && git commit -m "refactor(config): delete dead ModelPool engine and VoxConfig.model_pool field (B5/F3)"`

### Task 6 — Availability panel backend: provider-status command

**Files:**
- `crates/vox-orchestrator/src/models/key_guard.rs:11-31` (extract candidate list, add statuses fn)
- `crates/vox-gui/src/commands/llm_settings.rs:49-61` (new DTO + command below `openrouter_key_status`)
- `crates/vox-gui/src/main.rs:169` (register after `commands::llm_settings::openrouter_key_status,`)

- [ ] **Step 1: Failing test (key_guard).** Append to `mod avail_tests` in `key_guard.rs` (existing idiom at `:57-71`):

```rust
    #[test]
    fn statuses_cover_every_candidate_and_mark_locals_present() {
        let statuses = inference_provider_statuses();
        assert_eq!(statuses.len(), CANDIDATE_PROVIDERS.len());
        for (p, present) in &statuses {
            if matches!(
                p,
                ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
            ) {
                assert!(*present, "local provider {p:?} must always report present");
            }
        }
    }
```

- [ ] **Step 2: Watch it fail.** `cargo test -p vox-orchestrator key_guard` → compile error: ``cannot find function `inference_provider_statuses` ``/``cannot find value `CANDIDATE_PROVIDERS` ``.
- [ ] **Step 3: Implement (key_guard).** Hoist the inline candidates slice out of `available_inference_providers` (`key_guard.rs:12-25`) into a const and add the statuses fn:

```rust
/// Every provider the selector will consider, in display order.
pub const CANDIDATE_PROVIDERS: &[ProviderType] = &[
    ProviderType::GoogleDirect,
    ProviderType::OpenRouter,
    ProviderType::Groq,
    ProviderType::Mistral,
    ProviderType::DeepSeek,
    ProviderType::SambaNova,
    ProviderType::Cerebras,
    ProviderType::Anthropic,
    ProviderType::HuggingFaceRouter,
    ProviderType::Ollama,
    ProviderType::PopuliMesh,
    ProviderType::VoxLocal,
];

pub fn available_inference_providers() -> Vec<ProviderType> {
    CANDIDATE_PROVIDERS
        .iter()
        .filter(|p| provider_secret_is_available(p))
        .cloned()
        .collect()
}

/// Per-provider credential presence for the full candidate list — the
/// GUI availability panel's SSOT (B9).
pub fn inference_provider_statuses() -> Vec<(ProviderType, bool)> {
    CANDIDATE_PROVIDERS
        .iter()
        .map(|p| (p.clone(), provider_secret_is_available(p)))
        .collect()
}
```

  `cargo test -p vox-orchestrator key_guard` → green.
- [ ] **Step 4: Failing test (GUI DTO).** In `llm_settings.rs`, add below the `openrouter_key_status` command a test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_status_dto_serializes_shape_frontend_expects() {
        let dto = ProviderStatusDto {
            provider: "Anthropic".into(),
            key_present: false,
            is_local: false,
            local_reachable: None,
            local_models: vec![],
        };
        let j = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(j["provider"], "Anthropic");
        assert_eq!(j["key_present"], false);
        assert!(j["local_reachable"].is_null());
    }
}
```

  `cargo test -p vox-gui provider_status_dto` → compile error (`ProviderStatusDto` missing).
- [ ] **Step 5: Implement (GUI command).** Append to `llm_settings.rs`:

```rust
#[derive(Debug, Serialize)]
pub struct ProviderStatusDto {
    /// Debug-format provider name, e.g. "OpenRouter", "Ollama".
    pub provider: String,
    pub key_present: bool,
    pub is_local: bool,
    /// Some(reachable) from the cached local probe; None for cloud providers.
    pub local_reachable: Option<bool>,
    /// Model names the local server reported (empty for cloud providers).
    pub local_models: Vec<String>,
}

/// Per-backend availability (B9): credential presence for every candidate
/// provider + live local-server health from the shared TTL-cached probe.
#[tauri::command]
pub async fn inference_provider_status() -> Result<Vec<ProviderStatusDto>, String> {
    use vox_orchestrator::models::ProviderType;
    let statuses = vox_orchestrator::models::key_guard::inference_provider_statuses();
    let base = vox_config::inference::local_ollama_populi_base_url();
    let probe = vox_actor_runtime::inference_env::probe_populi_capabilities_cached(
        &base,
        std::time::Duration::from_secs(15),
    )
    .await;
    Ok(statuses
        .into_iter()
        .map(|(p, key_present)| {
            let is_local = matches!(
                p,
                ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
            );
            ProviderStatusDto {
                provider: format!("{p:?}"),
                key_present,
                is_local,
                local_reachable: is_local.then_some(probe.reachable),
                local_models: if is_local {
                    probe.model_names.clone()
                } else {
                    Vec::new()
                },
            }
        })
        .collect())
}
```

  Register in `main.rs` after line 169 (`commands::llm_settings::openrouter_key_status,`): add `commands::llm_settings::inference_provider_status,`.
- [ ] **Step 6: Watch it pass.** `cargo test -p vox-gui provider_status_dto` → ok. `cargo build -p vox-gui` → green.
- [ ] **Step 7: Lint + commit.**
  `cargo clippy -p vox-orchestrator -- -D warnings` (vox-gui: build + tests stand in for clippy — vox-gui breaks `clippy --all-targets`; if you clippy it, do `cargo clippy -p vox-gui` alone and accept buildscript quirks) and `cargo fmt -p vox-orchestrator && cargo fmt -p vox-gui`
  `git add crates/vox-orchestrator/src/models/key_guard.rs crates/vox-gui/src/commands/llm_settings.rs crates/vox-gui/src/main.rs && git commit -m "feat(gui): inference_provider_status command over key_guard + cached probe (B9)"`

### Task 7 — Availability panel frontend + chat model picker

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Models/BackendAvailability.tsx` (new) + `BackendAvailability.test.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx:48-72` (fetch), `:88-127` (render)
- `crates/vox-gui/src/commands/control_plane.rs:10-31` (`SubmitTaskInput` gains `model_override`), `:69-102` (params builder extracted + hint inserted)
- `crates/vox-gui/ui/src/types/tauri.ts:40-54` (`ChatPayload` gains `model_override`)
- `crates/vox-gui/ui/src/App.tsx` (`chatModelOverride` state; composer submit at `:1056`; `handleLoquelaSubmit` input at `:690-702`)
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx:184-206` (thread override props into `ChatSurface`)
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatModelPicker.tsx` (new) + `ChatModelPicker.test.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx:30-73` (props), `:194-217` (mount picker beside the execution rail)
- `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (add `inference_provider_status` + `set_active_model` cases near `list_orchestrator_tasks` at `:307`)
- `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts:16-23` (add `inference_provider_status` to `LIST_CMDS` in `installEmptyStateMock`)

**Picker channel (per Resolved decision "Item 4"):** the pick does NOT go through `set_active_model` — that command never reaches the daemon that serves chat. It threads as `enqueue_hints.model_override` on the chat submit. e2e *interaction* specs for picker-apply are owned by **Phase 3 Task 13 (post-Phase-2)**; this task only supplies the mock command cases those specs will need.

- [ ] **Step 1: Failing test (presentational availability strip).** `BackendAvailability.test.tsx`, mirroring the Testing Library idiom of `ChatSessionRail.test.tsx:1-32`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { BackendAvailability, type ProviderStatus } from './BackendAvailability';

const statuses: ProviderStatus[] = [
  { provider: 'OpenRouter', key_present: true, is_local: false, local_reachable: null, local_models: [] },
  { provider: 'Anthropic', key_present: false, is_local: false, local_reachable: null, local_models: [] },
  { provider: 'Ollama', key_present: true, is_local: true, local_reachable: false, local_models: [] },
];

describe('BackendAvailability', () => {
  it('renders one row per backend with key and reachability state', () => {
    render(<BackendAvailability statuses={statuses} />);
    expect(screen.getByRole('listitem', { name: /OpenRouter/i })).toHaveTextContent(/key/i);
    expect(screen.getByRole('listitem', { name: /Anthropic/i })).toHaveTextContent(/no key/i);
    expect(screen.getByRole('listitem', { name: /Ollama/i })).toHaveTextContent(/offline/i);
  });

  it('renders nothing for an empty status list', () => {
    const { container } = render(<BackendAvailability statuses={[]} />);
    expect(container.firstChild).toBeNull();
  });
});
```

- [ ] **Step 2: Watch it fail.** `pnpm -C crates/vox-gui/ui test BackendAvailability` → `Cannot find module './BackendAvailability'` (or unresolved import).
- [ ] **Step 3: Implement `BackendAvailability.tsx`** (pure presentational; token classes copied from `ModelsView.tsx` grid cards):

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';

export interface ProviderStatus {
  provider: string;
  key_present: boolean;
  is_local: boolean;
  local_reachable: boolean | null;
  local_models: string[];
}

/** Per-backend live status strip for the Models surface (B9). */
export function BackendAvailability({ statuses }: { statuses: ProviderStatus[] }) {
  if (statuses.length === 0) return null;
  return (
    <Glass className="p-4">
      <div className="font-display text-[11px] tracking-[0.2em] uppercase text-text-muted">
        Backend availability
      </div>
      <div role="list" className="mt-2 grid grid-cols-2 md:grid-cols-3 xl:grid-cols-4 gap-2">
        {statuses.map(s => {
          const live = s.is_local ? s.local_reachable === true : s.key_present;
          const label = s.is_local
            ? s.local_reachable === true
              ? `online · ${s.local_models.length} models`
              : s.local_reachable === false
                ? 'offline'
                : 'probing…'
            : s.key_present
              ? 'key configured'
              : 'no key';
          return (
            <div
              key={s.provider}
              role="listitem"
              aria-label={s.provider}
              className="flex items-center gap-2 rounded-lg border border-border-subtle px-2 py-1.5"
            >
              <span
                aria-hidden
                className={`size-2 rounded-full ${live ? 'bg-emerald-400' : 'bg-zinc-600'}`}
              />
              <span className="font-mono text-[10px] text-text-primary truncate">{s.provider}</span>
              <span className="ml-auto text-[9px] uppercase tracking-widest text-text-muted">{label}</span>
            </div>
          );
        })}
      </div>
    </Glass>
  );
}
```

  `pnpm -C crates/vox-gui/ui test BackendAvailability` → 2 passed.
- [ ] **Step 4: Wire into `ModelsView.tsx`.** Add state + fetch to the existing `Promise.all` (`:51-55`) and render above the model grids (`:118`):

```tsx
import { BackendAvailability, type ProviderStatus } from './BackendAvailability';
// state:
const [providerStatuses, setProviderStatuses] = useState<ProviderStatus[]>([]);
// inside refresh(), extend the Promise.all tuple:
const [cards, routing, active, statuses] = await Promise.all([
  invoke<ModelCard[]>('list_model_cards', { limit: 120 }),
  invoke<RoutingSummary>('get_routing_summary_live'),
  invoke<string | null>('get_active_model'),
  invoke<ProviderStatus[]>('inference_provider_status').catch(() => [] as ProviderStatus[]),
]);
// Harden against a RESOLVED null (the e2e variant mocks and any future backend
// change resolve unknown commands to null — `.catch` never fires for that, and
// `statuses.length` would then TypeError inside BackendAvailability).
setProviderStatuses(Array.isArray(statuses) ? statuses : []);
// render, after the Decision Preview Glass block:
<BackendAvailability statuses={providerStatuses} />
```

- [ ] **Step 5: Failing Rust test (override reaches the daemon params).** Append a test module at the end of `crates/vox-gui/src/commands/control_plane.rs`:

```rust
#[cfg(test)]
mod submit_params_tests {
    use super::*;

    fn input(model_override: Option<&str>) -> SubmitTaskInput {
        SubmitTaskInput {
            description: "wire the picker".into(),
            files: vec![],
            priority: None,
            session_id: Some("gui-session-9".into()),
            mode: None,
            tier: None,
            allow_duplicate: None,
            model_hint: None,
            dry_run: None,
            active_skill: None,
            clutch: None,
            risk: None,
            model_override: model_override.map(str::to_string),
        }
    }

    #[test]
    fn submit_params_carry_model_override_enqueue_hint() {
        let params = submit_task_params(input(Some("anthropic/claude-opus-4.7")));
        assert_eq!(
            params["enqueue_hints"]["model_override"],
            "anthropic/claude-opus-4.7"
        );
    }

    #[test]
    fn no_pick_means_no_override_hint() {
        let params = submit_task_params(input(None));
        // No other hints set either ⇒ the enqueue_hints key is absent entirely
        // (the daemon rejects a null enqueue_hints).
        assert!(params.get("enqueue_hints").is_none());
    }
}
```

  Run: `cargo test -p vox-gui submit_params` → compile error: `no field model_override on type SubmitTaskInput` / ``cannot find function `submit_task_params` ``.
- [ ] **Step 6: Implement (control_plane.rs).**
  1. `SubmitTaskInput` (`:10-31`) gains, after `risk`:

```rust
    /// Explicit chat model pick from the ChatModelPicker; forwarded as the
    /// `model_override` enqueue hint (`TaskEnqueueHints.model_override`,
    /// tasks.rs:241-243 → `AgentTask::model_override` via apply_hints
    /// tasks.rs:861-862 → `StreamRoute::UserModelOverride` runtime.rs:408-421).
    pub model_override: Option<String>,
```

  2. Extract the params construction (currently inline at `:62-102` of `submit_orchestrator_task`) into a testable builder, inserting the new hint beside the tier→`model_preference` entry in the `enqueue_hints` map (`:83-92`):

```rust
/// Build the daemon SUBMIT_TASK params from the composer input. Extracted from
/// `submit_orchestrator_task` so the enqueue-hint wiring is unit-testable.
fn submit_task_params(input: SubmitTaskInput) -> serde_json::Value {
    let file_manifest: Vec<FileAffinity> = input.files.iter().map(FileAffinity::write).collect();
    let priority = match input.priority.as_deref() {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("normal") => Some(TaskPriority::Normal),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };
    let mut params = serde_json::json!({
        "description": input.description,
        "file_manifest": file_manifest,
        "priority": priority,
        "session_id": input.session_id.filter(|s| !s.trim().is_empty()),
        "allow_duplicate": input.allow_duplicate.unwrap_or(true),
        "model_hint": input.model_hint.filter(|s| !s.trim().is_empty()),
        "dry_run": input.dry_run,
        "active_skill": input.active_skill.filter(|s| !s.trim().is_empty()),
    });
    // Carry composer mode/tier/pick through as enqueue hints (tier →
    // model_preference; explicit pick → model_override). Only attach the key
    // when non-empty — the daemon rejects a null enqueue_hints.
    let mut enqueue_hints = serde_json::Map::new();
    if let Some(tier) = input.tier.as_deref().filter(|t| !t.trim().is_empty()) {
        enqueue_hints.insert("model_preference".into(), serde_json::json!(tier));
    }
    if let Some(model) = input.model_override.as_deref().filter(|m| !m.trim().is_empty()) {
        enqueue_hints.insert("model_override".into(), serde_json::json!(model));
    }
    if let Some(mode) = input.mode.as_deref().filter(|m| !m.trim().is_empty()) {
        enqueue_hints.insert("mode".into(), serde_json::json!(mode));
    }
    if let Some(clutch) = input.clutch.as_deref().filter(|c| !c.trim().is_empty()) {
        enqueue_hints.insert("clutch".into(), serde_json::json!(clutch));
    }
    if let Some(risk) = input.risk.as_deref().filter(|r| !r.trim().is_empty()) {
        enqueue_hints.insert("risk".into(), serde_json::json!(risk));
    }
    if !enqueue_hints.is_empty()
        && let Some(obj) = params.as_object_mut()
    {
        obj.insert(
            "enqueue_hints".into(),
            serde_json::Value::Object(enqueue_hints),
        );
    }
    params
}
```

  and shrink `submit_orchestrator_task`'s body to `let params = submit_task_params(input);` before the `call_orchestrator_daemon(&daemon, orch_daemon_method::SUBMIT_TASK, params)` call (the response handling at `:103-124` is unchanged).
  Run: `cargo test -p vox-gui submit_params` → `test result: ok. 2 passed`; `cargo build -p vox-gui` → green.
- [ ] **Step 7: Failing test (chat model picker + payload wiring guard).** `ChatModelPicker.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { ChatModelPicker } from './ChatModelPicker';

describe('ChatModelPicker', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_model_cards') return [{ id: 'openai/gpt-5.2-mini' }, { id: 'anthropic/claude-opus-4.7' }];
      return null;
    });
  });

  it('loads the catalog on open and reports a pick via onApplied — never set_active_model', async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel="openai/gpt-5.2-mini" onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: openai\/gpt-5\.2-mini/i }));
    await user.click(await screen.findByRole('option', { name: 'anthropic/claude-opus-4.7' }));
    expect(onApplied).toHaveBeenCalledWith('anthropic/claude-opus-4.7');
    // Honest wiring: set_active_model only touches the GUI process and is never
    // read by the daemon serving chat — the pick must NOT ride it.
    expect(invoke).not.toHaveBeenCalledWith('set_active_model', expect.anything());
  });

  it('offers auto-route to clear the override', async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<ChatModelPicker activeModel="anthropic/claude-opus-4.7" onApplied={onApplied} />);
    await user.click(screen.getByRole('button', { name: /model: anthropic/i }));
    await user.click(await screen.findByRole('option', { name: /auto-route/i }));
    expect(onApplied).toHaveBeenCalledWith(null);
  });
});

// Wiring guard (readFileSync idiom, mirroring Phase 1's ErrorBoundary.test.tsx):
// the picked model must reach the submit payload App sends to the daemon.
describe('model_override submit-payload wiring', () => {
  it('App.tsx threads the pick into the submit_orchestrator_task input', () => {
    const appSrc = readFileSync(path.resolve(__dirname, '../../../App.tsx'), 'utf8');
    // handleLoquelaSubmit maps the payload field into the daemon input…
    expect(appSrc).toMatch(/model_override:\s*payload\.model_override\s*\?\?\s*null/);
    // …and the composer call site injects the picker state into the payload.
    expect(appSrc).toMatch(/model_override:\s*chatModelOverride/);
  });
});
```

  Run `pnpm -C crates/vox-gui/ui test ChatModelPicker` → fails (module missing; wiring-guard regexes unmatched).
- [ ] **Step 8: Implement `ChatModelPicker.tsx` + thread the pick to the submit payload.**

```tsx
import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Chat-surface model pick. The pick is lifted to App state and threaded into
 *  the chat submit payload as the `model_override` enqueue hint — the one
 *  channel the daemon consumes (TaskEnqueueHints.model_override →
 *  AgentTask.model_override → StreamRoute::UserModelOverride). Deliberately
 *  NOT `set_active_model`, which only touches the GUI process (Resolved
 *  decision "Item 4"). `null` pick = auto-route (clear the override). */
export function ChatModelPicker({
  activeModel,
  onApplied,
}: {
  activeModel?: string | null;
  onApplied?: (modelId: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [models, setModels] = useState<Array<{ id: string }>>([]);
  const [error, setError] = useState<string | null>(null);

  const toggle = async () => {
    const next = !open;
    setOpen(next);
    if (next && models.length === 0) {
      try {
        const cards = await invoke<Array<{ id: string }>>('list_model_cards', { limit: 120 });
        setModels(Array.isArray(cards) ? cards : []);
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const apply = (id: string | null) => {
    onApplied?.(id);
    setOpen(false);
  };

  return (
    <div className="relative">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => void toggle()}
        className="rounded-lg border border-border-subtle px-2 py-1 font-mono text-[10px] text-text-muted hover:text-brass"
      >
        model: {activeModel ?? 'auto-route'}
      </button>
      {open && (
        <ul
          role="listbox"
          aria-label="Pick model for this chat"
          className="absolute z-50 mt-1 max-h-64 w-72 overflow-y-auto rounded-lg border border-border-subtle bg-bg-base p-1 custom-scrollbar"
        >
          <li key="auto-route">
            <button
              type="button"
              role="option"
              aria-selected={activeModel == null}
              onClick={() => apply(null)}
              className="w-full truncate rounded px-2 py-1 text-left font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
            >
              auto-route (clear override)
            </button>
          </li>
          {models.map(m => (
            <li key={m.id}>
              <button
                type="button"
                role="option"
                aria-selected={m.id === activeModel}
                onClick={() => apply(m.id)}
                className="w-full truncate rounded px-2 py-1 text-left font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
              >
                {m.id}
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && <div role="alert" className="mt-1 text-[10px] text-rose-400">{error}</div>}
    </div>
  );
}
```

  Then thread the pick end-to-end (the picker state lives in App, because the submit payload is built there):
  1. `types/tauri.ts` — `ChatPayload` (`:40-54`) gains `/** Explicit model pick for this submit; maps to the model_override enqueue hint. */ model_override?: string | null;`.
  2. `ChatSurface.tsx` — props (`:30-51`) gain `modelOverride?: string | null;` and `onModelOverrideChange?: (id: string | null) => void;` (destructure both). Import `ChatModelPicker` and render just above the transcript block, as the first child of the flex column that holds the transcript (`<div className="flex min-w-0 flex-1 flex-col gap-4">`):
  `<div className="mb-2 flex justify-end"><ChatModelPicker activeModel={modelOverride ?? activeModel} onApplied={id => onModelOverrideChange?.(id)} /></div>`
  3. `surfaceComponents.tsx` — `SurfaceProps` gains `chatModelOverride?: string | null;` and `onChatModelOverrideChange?: (id: string | null) => void;`; the `case 'chat'` mount (`:184-206`) passes `modelOverride={props.chatModelOverride}` and `onModelOverrideChange={props.onChatModelOverrideChange}`.
  4. `App.tsx` — add `const [chatModelOverride, setChatModelOverride] = useState<string | null>(null);` beside the other chat state; supply both new props where App builds the surface props (beside `chatActiveModel`); change the composer call site (`:1056`) to
  `onSubmit={(p) => handleLoquelaSubmit({ ...p, session_id: activeSessionId, model_override: chatModelOverride })}`
  and in `handleLoquelaSubmit`'s input object (`:690-702`), after `model_hint`, add `model_override: payload.model_override ?? null,`. (Leave `model_hint`/`tier` untouched — `model_override` is the binding channel, `model_hint` stays the non-binding one.)
- [ ] **Step 9: Watch it pass + typecheck.** `pnpm -C crates/vox-gui/ui test ChatModelPicker` → ok (2 picker tests + wiring guard); `pnpm -C crates/vox-gui/ui typecheck` → clean.
- [ ] **Step 10: e2e mock coverage (both mock layers — Phase 3 Task 13's interaction specs consume these).**
  1. `e2e/lib/tauriMock.ts` — add beside `case 'list_orchestrator_tasks': return [];` (`:307`):
  `case 'inference_provider_status': return [{ provider: 'OpenRouter', key_present: true, is_local: false, local_reachable: null, local_models: [] }, { provider: 'Ollama', key_present: true, is_local: true, local_reachable: true, local_models: ['llama3.2'] }];`
  `case 'set_active_model': return null;` (Models-surface apply path; needed by Phase 3 Task 13's specs.)
  2. `e2e/lib/tauriMockVariants.ts` — add `'inference_provider_status'` to `LIST_CMDS` in `installEmptyStateMock` (`:16-23`) so the empty-state sweep resolves it to `[]` instead of falling through to `default: return null` (which, without the Step 4 `Array.isArray` guard, would TypeError `BackendAvailability`). `installErrorStateMock` needs no entry: it has no LIST_CMDS, and its rejecting `list_model_cards` already fails the whole `Promise.all` before `setProviderStatuses` runs.
  3. Run the existing screenshot sweep locally to confirm Models still renders: `pnpm -C crates/vox-gui/ui exec playwright test e2e/screenshots.spec.ts --project=chromium` (or the models-only grep if the suite is slow).
- [ ] **Step 11: Lint + commit.**
  `cargo fmt -p vox-gui`
  `git add crates/vox-gui/src/commands/control_plane.rs crates/vox-gui/ui/src/components/surfaces/Models crates/vox-gui/ui/src/components/surfaces/Chat/ChatModelPicker.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatModelPicker.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/types/tauri.ts crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/e2e/lib/tauriMock.ts crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts && git commit -m "feat(gui): backend availability strip + chat model pick via model_override enqueue hint (B9)"`

### Task 8 — Hopper DTO: real persisted fields

**Files:**
- `crates/vox-gui/src/commands/orchestrator.rs:722-739` (DTO + mapper), `:823-847` (tests)
- `crates/vox-gui/ui/src/transport.ts:788-794` (TS `HopperTaskDto`)
- `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts:1-7` (TS DTO), `:75-96` (`mapHopperTasksToRows`), `:9-20` (`TaskRow.agent_id` type)
- `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.test.ts:84-118`

Current Rust DTO (`orchestrator.rs:722-739`):

```rust
#[derive(Debug, serde::Serialize)]
pub struct HopperTaskDto {
    pub item_id: String,
    pub intent: String,
    pub priority: u8,
    pub state: String,
    pub task_id: u64,
}

fn hopper_item_to_dto(item: &vox_orchestrator::hopper::IntakeItem) -> HopperTaskDto {
    HopperTaskDto {
        item_id: item.item_id.0.clone(),
        intent: item.intent.clone(),
        priority: item.classified_priority as u8,
        state: item.state.kind().to_string(),
        task_id: vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0),
    }
}
```

- [ ] **Step 1: Failing Rust test.** Extend `mod hopper_tests` (`orchestrator.rs:823-847`):

```rust
    #[test]
    fn hopper_dto_carries_persisted_session_agent_and_mesh_fields() {
        use vox_orchestrator::hopper::types::ItemState;
        let mut item = IntakeItem::new(
            "wired intent".to_string(),
            vec![],
            PriorityHint::Normal,
            IntakeSource::Mesh { node_id: "did:vox:peer-1".into() },
            Some("gui-session-9".to_string()),
        );
        item.state = ItemState::Assigned { agent_id: "agent-42".into() };
        let dto = hopper_item_to_dto(&item);
        assert_eq!(dto.session_id.as_deref(), Some("gui-session-9"));
        assert_eq!(dto.agent_id.as_deref(), Some("agent-42"));
        assert_eq!(dto.remote_node.as_deref(), Some("did:vox:peer-1"));
        // Inbox developer items carry none of the three.
        let plain = IntakeItem::new("p".into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None);
        let dto2 = hopper_item_to_dto(&plain);
        assert!(dto2.session_id.is_none() && dto2.agent_id.is_none() && dto2.remote_node.is_none());
    }
```

- [ ] **Step 2: Watch it fail.** `cargo test -p vox-gui hopper_dto_carries` → compile error: `no field session_id on type HopperTaskDto`.
- [ ] **Step 3: Implement (Rust).**

```rust
#[derive(Debug, serde::Serialize)]
pub struct HopperTaskDto {
    pub item_id: String,
    pub intent: String,
    pub priority: u8,
    pub state: String,
    pub task_id: u64,
    /// Chat/CLI session the item was submitted from (persisted column).
    pub session_id: Option<String>,
    /// Agent bound to the item while `state == "assigned"` (from state JSON).
    pub agent_id: Option<String>,
    /// Origin daemon for mesh-replicated items (from source JSON).
    pub remote_node: Option<String>,
}

fn hopper_item_to_dto(item: &vox_orchestrator::hopper::IntakeItem) -> HopperTaskDto {
    let agent_id = match &item.state {
        vox_orchestrator::hopper::ItemState::Assigned { agent_id } => Some(agent_id.clone()),
        _ => None,
    };
    let remote_node = match &item.source {
        vox_orchestrator::hopper::IntakeSource::Mesh { node_id } => Some(node_id.clone()),
        _ => None,
    };
    HopperTaskDto {
        item_id: item.item_id.0.clone(),
        intent: item.intent.clone(),
        priority: item.classified_priority as u8,
        state: item.state.kind().to_string(),
        task_id: vox_orchestrator::orchestrator::dispatch::stable_hash(&item.item_id.0),
        session_id: item.session_id.clone(),
        agent_id,
        remote_node,
    }
}
```

  `cargo test -p vox-gui hopper` → green (including the pre-existing `test_hopper_item_to_dto`).
- [ ] **Step 4: Failing TS test.** Extend `tasksHelpers.test.ts` `mapHopperTasksToRows` block:

```ts
  it('carries real session/agent/mesh fields from the DTO instead of hardcoded nulls', () => {
    const rows = mapHopperTasksToRows(
      [{
        item_id: 'a', intent: 'A', priority: 1, state: 'assigned', task_id: 1,
        session_id: 'gui-9', agent_id: 'agent-42', remote_node: 'did:vox:peer-1',
      }],
      new Set(),
    );
    expect(rows[0].session_id).toBe('gui-9');
    expect(rows[0].agent_id).toBe('agent-42');
    expect(rows[0].remote_node).toBe('did:vox:peer-1');
  });
```

  `pnpm -C crates/vox-gui/ui test tasksHelpers` → fails: TS error (unknown DTO props) / `expected null to be 'gui-9'`.
- [ ] **Step 5: Implement (TS).**
  1. `tasksHelpers.ts:1-7` and `transport.ts:788-794` — extend **both** `HopperTaskDto` interfaces identically (they are intentionally duplicated today; keep them in sync):

```ts
export interface HopperTaskDto {
  item_id: string;
  intent: string;
  priority: number;
  state: string;
  task_id: number;
  session_id?: string | null;
  agent_id?: string | null;
  remote_node?: string | null;
}
```

  2. `tasksHelpers.ts` — widen `TaskRow.agent_id` to `number | string | null` (hopper agent ids are strings, orchestrator ids numeric), and replace the hardcoded block at `:89-94`:

```ts
    agent_id: dto.agent_id ?? null,
    session_id: dto.session_id ?? null,
    // Not persisted by the hopper store (sqlite_store.rs) — real values for
    // these arrive on orchestrator-origin rows via the merge-view read.
    estimated_complexity: 1,
    depends_on: [],
    write_files: [],
    remote_node: dto.remote_node ?? null,
```

- [ ] **Step 6: Watch it pass.** `pnpm -C crates/vox-gui/ui test tasksHelpers` → all pass; `pnpm -C crates/vox-gui/ui typecheck` → clean.
- [ ] **Step 7: Commit.**
  `cargo fmt -p vox-gui`
  `git add crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.test.ts && git commit -m "feat(gui): hopper DTO carries persisted session/agent/mesh fields (B2)"`

### Task 9 — Hopper: list `done` items + `hopper_mark_done` command

**Files:**
- `crates/vox-db/src/store/ops_orchestrator.rs:352-374` (`hopper_history_list` — unbounded; add a bounded variant beside it)
- `crates/vox-orchestrator/src/hopper/sqlite_store.rs:232-252` (`complete` searches only `assigned()`), `:137-145` (`history()` — pattern for `history_recent`), tests `:415-444`
- `crates/vox-gui/src/commands/orchestrator.rs:741-755` (`hopper_list`), `:807-821` (`hopper_cancel` — pattern for the new command)
- `crates/vox-gui/src/main.rs:177-180` (register `hopper_mark_done`)
- `crates/vox-gui/ui/src/transport.ts:796-799` (wrapper)
- `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (add `hopper_mark_done` case — mock coverage for Phase 3 Task 13's interaction specs)

Two verified facts drive this task: `HopperIntake::complete` exists (`store.rs:96`) but `SqliteHopper::complete` only finds items in `assigned()` (`sqlite_store.rs:233-237`), while the in-memory store completes from any state (`store.rs:426-434`) — a to-do sitting in Inbox cannot be marked done today. And `hopper_list` reads only `inbox() + assigned()` (`orchestrator.rs:748-753`), so `state == "done"` never reaches the UI (`tasksHelpers` `completed` branch unreachable).

**Bound on the done read (F7):** `vox_db::hopper_history_list` (`ops_orchestrator.rs:352-357`) has **no LIMIT** — it returns every done/overridden/cancelled row ever persisted, ascending. `hopper_list` is re-polled on every `vox://tasks-changed` (TasksView.tsx:82-88) and by the shared attention-inbox path, so chaining the full history would grow the Tasks payload without bound. The done read is therefore bounded to the most recent **`DONE_HISTORY_LIMIT = 50`** items via a new `hopper_history_list_recent(limit)` query (`ORDER BY submitted_at DESC LIMIT ?1`), and the bound is pinned by tests below.

- [ ] **Step 1: Failing store test.** Append to `mod tests` in `sqlite_store.rs` (idiom `:421-443`):

```rust
    #[tokio::test]
    async fn complete_marks_inbox_item_done_and_history_lists_it() {
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db);
        let item = hopper
            .submit(
                "todo done directly from inbox".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        let done = hopper.complete(&item.item_id).await.expect("inbox item completable");
        assert_eq!(done.state, ItemState::Done);
        assert!(hopper.inbox().await.is_empty());
        assert!(hopper.history().await.iter().any(|i| i.item_id == item.item_id));
    }

    #[tokio::test]
    async fn history_recent_is_bounded_and_newest_first() {
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db);
        for intent in ["first", "second", "third"] {
            let item = hopper
                .submit(intent.into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None)
                .await;
            hopper.complete(&item.item_id).await.expect("completable");
        }
        let recent = hopper.history_recent(2).await;
        assert_eq!(recent.len(), 2, "limit must bound the read");
        // ORDER BY submitted_at DESC ⇒ newest first; "first" (oldest) is cut.
        assert!(recent.iter().all(|i| i.intent != "first"));
    }
```

  (If all three submits land in the same microsecond on a fast machine, tie-break flakiness is possible — assert on `len()` only in that case; `submitted_at` comes from `now_micros()`, so in practice the ordering assertion holds.)
- [ ] **Step 2: Watch it fail.** `cargo test -p vox-orchestrator hopper::sqlite_store` → `complete_marks_inbox_item_done_and_history_lists_it` FAILED: `called Result::unwrap()/expect on an Err value: NotFound(..)` (complete only searches assigned); `history_recent_is_bounded_and_newest_first` → compile error (`history_recent` doesn't exist).
- [ ] **Step 3: Implement (store parity + bounded history).**
  1. In `SqliteHopper::complete` (`sqlite_store.rs:232-242`), replace the lookup with the same inbox+assigned chain `cancel` uses (`:254-260`), including cancel's terminal guard (`:267-272`) for symmetry — note the guard is defensive-only through this lookup: `inbox()`/`assigned()` SQL filters (`ops_orchestrator.rs:306,331`) never return terminal rows, so it cannot fire here, but keeping `complete` and `cancel` structurally identical costs nothing and survives future lookup changes:

```rust
    async fn complete(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError> {
        let item = self
            .inbox()
            .await
            .into_iter()
            .chain(self.assigned().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        // Defensive parity with cancel (:267-272); unreachable via the
        // inbox/assigned chain today (their SQL excludes terminal states).
        if matches!(
            item.state,
            ItemState::Done | ItemState::Overridden | ItemState::Cancelled
        ) {
            return Err(HopperError::Terminal);
        }

        item.state = ItemState::Done;
        let state_json = serde_json::to_string(&item.state).unwrap();

        if let Err(e) = self.db.hopper_update_state(&item_id.0, &state_json).await {
            tracing::error!("Failed to update state in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }
```

  2. In `crates/vox-db/src/store/ops_orchestrator.rs`, add below `hopper_history_list` (`:374`) a bounded variant (param idiom: `turso::params![limit]`, see `oratio_eval.rs:193-198`):

```rust
    /// Most-recent `limit` hopper items in terminal states, newest first.
    /// Bounded companion to [`Self::hopper_history_list`] for hot read paths
    /// (the GUI Tasks surface re-polls on every tasks-changed event).
    pub async fn hopper_history_list_recent(
        &self,
        limit: u32,
    ) -> Result<Vec<HopperInboxRow>, StoreError> {
        let mut rows = self.conn.query(
            "SELECT item_id, intent, affinity_json, priority, source, session_id, state, submitted_at
             FROM hopper_inbox
             WHERE state IN ('\"done\"', '\"overridden\"', '\"cancelled\"')
             ORDER BY submitted_at DESC LIMIT ?1",
            turso::params![limit],
        ).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(HopperInboxRow {
                item_id: row.get(0)?,
                intent: row.get(1)?,
                affinity_json: row.get(2)?,
                priority: row.get(3)?,
                source: row.get(4)?,
                session_id: row.get(5)?,
                state: row.get(6)?,
                submitted_at: row.get(7)?,
            });
        }
        Ok(out)
    }
```

  3. In `sqlite_store.rs`, add an inherent method beside the `HopperIntake` impl (pattern: `history()` at `:137-145` — inherent, not a trait method, because the trait has no bounded read):

```rust
impl SqliteHopper {
    /// Most-recent `limit` terminal items, newest first (bounded history read).
    pub async fn history_recent(&self, limit: u32) -> Vec<IntakeItem> {
        match self.db.hopper_history_list_recent(limit).await {
            Ok(rows) => rows.into_iter().map(row_to_item).collect(),
            Err(e) => {
                tracing::error!("Failed to list recent history from sqlite hopper: {:?}", e);
                vec![]
            }
        }
    }
}
```

  `cargo test -p vox-orchestrator hopper` → green (both new tests).
- [ ] **Step 4: Implement (GUI command + bounded done in list).** In `orchestrator.rs`:
  1. `hopper_list` (`:742-755`) — include the most recent terminal `Done` items, bounded (skip `Cancelled`/`Overridden` — they are removals, not completions):

```rust
/// Most-recent bound on completed items chained into `hopper_list` (F7): the
/// command is re-polled on every tasks-changed event, so the done read must
/// not grow with all-time history. Pinned by
/// `done_history_limit_is_the_agreed_bound` below and exercised by
/// `history_recent_is_bounded_and_newest_first` in sqlite_store.rs.
const DONE_HISTORY_LIMIT: u32 = 50;

#[tauri::command]
pub async fn hopper_list() -> Result<Vec<HopperTaskDto>, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let inbox = hopper.inbox().await;
    let assigned = hopper.assigned().await;
    // Bounded, newest-first; the recent-window read also fetches overridden/
    // cancelled rows, so filter to Done here.
    let done: Vec<_> = hopper
        .history_recent(DONE_HISTORY_LIMIT)
        .await
        .into_iter()
        .filter(|i| matches!(i.state, vox_orchestrator::hopper::ItemState::Done))
        .collect();
    let mut all = Vec::new();
    for item in inbox.iter().chain(assigned.iter()).chain(done.iter()) {
        all.push(hopper_item_to_dto(item));
    }
    Ok(all)
}
```

  and append the bound's drift guard to `mod hopper_tests`:

```rust
    #[test]
    fn done_history_limit_is_the_agreed_bound() {
        // Spec Phase 2 item 6 records "bounded, most-recent-N" for the done
        // read; changing N is a product decision, not a drive-by.
        assert_eq!(DONE_HISTORY_LIMIT, 50);
    }
```

  2. New command, mirroring `hopper_cancel` (`:807-821`):

```rust
#[tauri::command]
pub async fn hopper_mark_done(
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<HopperTaskDto, String> {
    use vox_orchestrator::hopper::HopperIntake;
    let db = vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())?;
    let hopper = vox_orchestrator::hopper::SqliteHopper::new(Arc::new(db));
    let hid = vox_orchestrator::hopper::HopperItemId(item_id);
    let item = hopper.complete(&hid).await.map_err(|e| e.to_string())?;
    emit_tasks_changed(&app_handle);
    Ok(hopper_item_to_dto(&item))
}
```

  3. Register in `main.rs` after line 180 (`commands::orchestrator::hopper_cancel,`): `commands::orchestrator::hopper_mark_done,`.
  4. Transport wrapper in `transport.ts` under `hopperList` (`:796-799`):

```ts
/** Mark a hopper to-do done (terminal Done state; distinct from cancel). */
export function hopperMarkDone(itemId: string): Promise<HopperTaskDto> {
  return invoke<HopperTaskDto>('hopper_mark_done', { itemId });
}
```

  5. e2e mock coverage for the new command — in `e2e/lib/tauriMock.ts`, beside `case 'list_orchestrator_tasks': return [];` (`:307`):
  `case 'hopper_mark_done': return { item_id: 'mock-item', intent: 'mock to-do', priority: 1, state: 'done', task_id: 1, session_id: null, agent_id: null, remote_node: null };`
  (The mark-done *interaction* spec that drives this is owned by **Phase 3 Task 13 (post-Phase-2)**; this case is the mock coverage it will need.)
- [ ] **Step 5: Verify.** `cargo test -p vox-db hopper` (new bounded query compiles + existing hopper ops stay green), `cargo test -p vox-gui hopper` (includes `done_history_limit_is_the_agreed_bound`) and `cargo build -p vox-gui` → green. `pnpm -C crates/vox-gui/ui typecheck` → clean.
- [ ] **Step 6: Commit.**
  `cargo fmt -p vox-db && cargo fmt -p vox-orchestrator && cargo fmt -p vox-gui`
  `git add crates/vox-db/src/store/ops_orchestrator.rs crates/vox-orchestrator/src/hopper/sqlite_store.rs crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/src/main.rs crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/e2e/lib/tauriMock.ts && git commit -m "feat(gui): hopper_mark_done command + bounded done items in hopper_list; sqlite complete() inbox parity"`

### Task 10 — Merge-view Tasks read (fork F1) + shared priority constant

**Files:**
- `crates/vox-gui/ui/src/lib/taskPriority.ts` (new) + `taskPriority.test.ts` (new)
- `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts:9-41` (`TaskRow.origin`), `:75-96` (origin tag), new `mapOrchestratorTasksToRows`
- `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.test.ts:4-16` (row helper), new cases
- `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx:50-101` (fetch/merge), `:118-157` (per-origin actions), `:139-245` (columns), `:253` (subtitle)
- `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.copy.test.tsx` (created by Phase 1 Task 6 — rewrite for the merged-view subtitle)
- `crates/vox-gui/src/commands/orchestrator.rs` `mod hopper_tests` (Rust-side priority guard)

**Base state:** this task lands on top of Phase 1. Phase 1 Task 6 REPLACED the TasksView subtitle wholesale (the pre-Phase-1 sentence "Everything queued or running across the agent fleet." no longer exists in source) and pinned the interim copy with `TasksView.copy.test.tsx`; Phase 1 Task 5 added `TasksView.listeners.test.tsx`. Both tests mock `invoke` with `vi.fn().mockResolvedValue(null)` — every command they don't care about RESOLVES `null`, which is why Step 6.1 must coerce non-array results. Scope limit per the spec's recorded decision (Resolved decisions item 6): the union lives in TasksView only; `hopper_list`/`useAttentionInbox` stay hopper-only.

- [ ] **Step 1: Failing tests — shared priority constant (both sides).**
  TS — `crates/vox-gui/ui/src/lib/taskPriority.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { TASK_PRIORITY_WIRE, priorityLabel, priorityValue } from './taskPriority';

describe('task priority wire constants', () => {
  it('pins the Rust TaskPriority discriminants (crates/vox-orchestrator/src/types/tasks.rs:44-51)', () => {
    expect(TASK_PRIORITY_WIRE.background).toBe(0);
    expect(TASK_PRIORITY_WIRE.normal).toBe(1);
    expect(TASK_PRIORITY_WIRE.urgent).toBe(2);
  });
  it('round-trips label <-> value with normal as the fallback', () => {
    expect(priorityLabel(2)).toBe('urgent');
    expect(priorityLabel(0)).toBe('background');
    expect(priorityLabel(99)).toBe('normal');
    expect(priorityValue('urgent')).toBe(2);
    expect(priorityValue('garbage')).toBe(1);
  });
});
```

  Rust — append to `mod hopper_tests` in `crates/vox-gui/src/commands/orchestrator.rs`:

```rust
    #[test]
    fn task_priority_wire_values_match_frontend_constants() {
        // Mirror of crates/vox-gui/ui/src/lib/taskPriority.ts (TASK_PRIORITY_WIRE).
        // If either side changes, both tests must be updated together.
        use vox_orchestrator::types::TaskPriority;
        assert_eq!(TaskPriority::Background as u8, 0);
        assert_eq!(TaskPriority::Normal as u8, 1);
        assert_eq!(TaskPriority::Urgent as u8, 2);
    }
```

- [ ] **Step 2: Watch them fail/pass split.** `pnpm -C crates/vox-gui/ui test taskPriority` → module-not-found failure. `cargo test -p vox-gui task_priority_wire` → passes immediately (it pins existing discriminants) — that's fine; it's the drift guard.
- [ ] **Step 3: Implement `taskPriority.ts`:**

```ts
/** Wire values for task priority, shared with Rust `TaskPriority`
 *  (crates/vox-orchestrator/src/types/tasks.rs:44-51: Background=0, Normal=1,
 *  Urgent=2). Guarded on the Rust side by
 *  `task_priority_wire_values_match_frontend_constants`. */
export const TASK_PRIORITY_WIRE = { background: 0, normal: 1, urgent: 2 } as const;

export type PriorityLabel = keyof typeof TASK_PRIORITY_WIRE;

export function priorityLabel(value: number): PriorityLabel {
  if (value === TASK_PRIORITY_WIRE.urgent) return 'urgent';
  if (value === TASK_PRIORITY_WIRE.background) return 'background';
  return 'normal';
}

export function priorityValue(label: string): number {
  return TASK_PRIORITY_WIRE[label as PriorityLabel] ?? TASK_PRIORITY_WIRE.normal;
}
```

  `pnpm -C crates/vox-gui/ui test taskPriority` → green.
- [ ] **Step 4: Failing test — origin-tagged merge helpers.** In `tasksHelpers.test.ts`, extend the `row` helper (`:4-16`) with `origin: 'hopper' as const,` and add:

```ts
describe('mapOrchestratorTasksToRows', () => {
  it('tags orchestrator rows with their origin and passes real graph fields through', () => {
    const rows = mapOrchestratorTasksToRows(
      [{
        id: 41, description: 'graph task', priority: 'urgent', lifecycle: 'queued',
        agent_id: 7, session_id: 'gui-9', estimated_complexity: 3,
        depends_on: [40], write_files: ['src/a.rs'], remote_node: null,
      }],
      new Set(),
    );
    expect(rows[0]).toMatchObject({
      id: 41, origin: 'orchestrator', priority: 'urgent', lifecycle: 'queued',
      agent_id: 7, session_id: 'gui-9', depends_on: [40], write_files: ['src/a.rs'],
    });
  });

  it('marks gated orchestrator tasks blocked', () => {
    const rows = mapOrchestratorTasksToRows(
      [{
        id: 41, description: 'g', priority: 'normal', lifecycle: 'queued',
        agent_id: null, session_id: null, estimated_complexity: 1,
        depends_on: [], write_files: [], remote_node: null,
      }],
      new Set([41]),
    );
    expect(rows[0].lifecycle).toBe('blocked');
  });
});

describe('origin tagging', () => {
  it('hopper rows are origin-tagged hopper', () => {
    const rows = mapHopperTasksToRows(
      [{ item_id: 'a', intent: 'A', priority: 1, state: 'inbox', task_id: 1 }],
      new Set(),
    );
    expect(rows[0].origin).toBe('hopper');
  });
});
```

  `pnpm -C crates/vox-gui/ui test tasksHelpers` → fails (`mapOrchestratorTasksToRows` not exported; `origin` missing).
- [ ] **Step 5: Implement helpers.** In `tasksHelpers.ts`:
  1. `TaskRow` gains `origin: 'hopper' | 'orchestrator';`.
  2. `mapHopperTasksToRows` adds `origin: 'hopper' as const,` and replaces the inline priority ternary at `:79` with `priority: priorityLabel(dto.priority),` (`import { priorityLabel } from '../../../lib/taskPriority';`).
  3. Add (mirroring `TaskRowDto` from `control_plane.rs:211-226` / `transport.ts:366-368`):

```ts
export interface OrchestratorTaskDto {
  id: number;
  description: string;
  priority: string;   // normalized lowercase by the Tauri command
  lifecycle: string;  // normalized snake_case by the Tauri command
  agent_id: number | null;
  session_id: string | null;
  estimated_complexity: number;
  depends_on: number[];
  write_files: string[];
  remote_node: string | null;
}

/** Orchestrator task-graph rows (chat submissions land here) mapped into the
 *  same TaskRow shape as hopper rows, origin-tagged for the merge-view. */
export function mapOrchestratorTasksToRows(
  tasks: OrchestratorTaskDto[],
  gatedTaskIds: Set<number>,
): TaskRow[] {
  return tasks.map(t => ({
    id: t.id,
    description: t.description,
    priority: t.priority,
    lifecycle: gatedTaskIds.has(t.id) ? 'blocked' : t.lifecycle,
    agent_id: t.agent_id,
    session_id: t.session_id,
    estimated_complexity: t.estimated_complexity,
    depends_on: t.depends_on,
    write_files: t.write_files,
    remote_node: t.remote_node,
    origin: 'orchestrator' as const,
  }));
}
```

  `pnpm -C crates/vox-gui/ui test tasksHelpers` → green.
- [ ] **Step 6: Wire TasksView.**
  1. Fetch: add orchestrator rows in *both* modes (attention mode only supplies hopper rows). Add state `const [orchTasks, setOrchTasks] = useState<OrchestratorTaskDto[]>([]);` and a fetch inside `selfRefresh` (`:50-66`) *and* a parallel effect for attention mode (subscribe to the same `vox://tasks-changed` event) — concretely extend `selfRefresh`'s `Promise.all` with `voxTransport.listOrchestratorTasks().then(r => (Array.isArray(r) ? r : [])).catch(() => [])`, and in attention mode run a dedicated `useEffect` that fetches on mount + on `vox://tasks-changed`:

```ts
const fetchOrch = useCallback(async () => {
  try {
    const rows = await voxTransport.listOrchestratorTasks() as unknown as OrchestratorTaskDto[];
    // Coerce non-arrays (mirrors useAttentionInbox's `tasks ?? []`): the
    // transport returns the raw invoke result with no null guard
    // (transport.ts:366-368), and both Phase-1 TasksView tests mock invoke to
    // RESOLVE null — `rows.map` in the memo would TypeError otherwise.
    if (mounted.current) setOrchTasks(Array.isArray(rows) ? rows : []);
  } catch { /* daemon down — hopper rows still render */ }
}, []);
```

  Apply the same coercion to the `selfRefresh` extension: `voxTransport.listOrchestratorTasks().then(r => (Array.isArray(r) ? r : [])).catch(() => [])`.

  2. Merge in the `rows` memo (`:96-101`):

```ts
  const rows: TaskRow[] = useMemo(() => {
    const tasks = attention ? attention.hopperTasks : selfHopperTasks;
    const feedbackNeedsYou = attention ? attention.needsYou : selfNeedsYou;
    const gateSet = new Set<number>(feedbackNeedsYou.flatMap(f => f.gates ?? []));
    return [
      ...mapOrchestratorTasksToRows(orchTasks, gateSet),
      ...mapHopperTasksToRows(tasks, gateSet),
    ];
  }, [attention, selfHopperTasks, selfNeedsYou, orchTasks]);
```

  3. Per-origin actions (`:125-128`):

```ts
  const remove = (r: TaskRow) =>
    act(() =>
      r.origin === 'orchestrator'
        ? invoke('cancel_orchestrator_task', { taskId: Number(r.id) })
        : invoke('hopper_cancel', { itemId: String(r.id) }),
    );

  const markDone = (r: TaskRow) => act(() => hopperMarkDone(String(r.id)));

  const reprioritize = (r: TaskRow, priority: number) =>
    act(() =>
      r.origin === 'orchestrator'
        ? invoke('reorder_orchestrator_task', { taskId: Number(r.id), priority: priorityLabel(priority) })
        : invoke('hopper_reprioritize', { itemId: String(r.id), priority }),
    );
```

  (import `hopperMarkDone` from `../../../transport` and `priorityLabel`/`TASK_PRIORITY_WIRE` from `../../../lib/taskPriority`; update the two call sites at `:149` and `:236` to pass the row. Replace the option-value literals at `:146,154-156` with `TASK_PRIORITY_WIRE.urgent` / `.normal` / `.background`.)
  4. Actions column (`:227-244`): add a mark-done button for hopper rows before the cancel button:

```tsx
          {r.origin === 'hopper' && r.lifecycle !== 'completed' && (
            <Button variant="ghost" size="xs" onClick={() => markDone(r)} disabled={busy} title="Mark done">
              <Icon.check className="size-3.5 text-text-muted hover:text-emerald-400 transition" />
            </Button>
          )}
```

  (if `Icon.check` does not exist in `ui/Icons`, use the existing checkmark icon name found there — verify before use.)
  5. Origin chip in the description meta row (after the mesh badge at `:214-221`): `<span className="rounded border border-border-subtle px-1 font-mono text-[9px] text-text-muted">{r.origin}</span>`.
  6. `groupBy` (`:320`): completed rows currently fall into 'Queued'; extend: `r.lifecycle === 'completed' ? 'Completed' : …` before the queued fallback.
  7. Subtitle (`:253`) — **write a new subtitle** (Phase 1 Task 6 replaced the pre-Phase-1 sentence entirely; there is no old text to "keep"). The merged view makes a fleet-wide claim honest again, so use:
  `Everything queued or running across the agent fleet — hopper to-dos and orchestrator task graph runs, tagged by origin.`
  (Deliberately contains "hopper" and "orchestrator task graph", and avoids "chat submissions land here", so the copy-test rewrite in 6.8 stays minimal.)
  8. Rewrite `TasksView.copy.test.tsx` (created by Phase 1 Task 6) for the merged-view subtitle — keep its render scaffold and `vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(null) }))` untouched; replace the assertion block with:

```ts
    // Phase 2 Task 10: the merge-view supersedes Phase 1's interim caveat copy.
    // The old caveat sentence must be gone…
    expect(screen.queryByText(/chat submissions land here/i)).toBeNull();
    expect(screen.queryByText(/are not listed here yet/i)).toBeNull();
    // …and the new subtitle names both stores and the origin tagging.
    expect(screen.getByText(/tagged by origin/i)).toBeTruthy();
    expect(screen.getByText(/hopper/i)).toBeTruthy();
    expect(screen.getByText(/orchestrator task graph/i)).toBeTruthy();
```

  (The `queryByText(/chat submissions land here/i)` null assertion is kept because the new copy avoids that phrase; the `/hopper/` + `/orchestrator task graph/` assertions carry over but now pin the NEW sentence.)
- [ ] **Step 7: Verify.** `pnpm -C crates/vox-gui/ui test tasksHelpers taskPriority TasksView` → green — this explicitly includes the two Phase-1 regression suites: `TasksView.copy.test.tsx` (rewritten in 6.8) and `TasksView.listeners.test.tsx` (unmodified — its `invoke` mock resolves `null`, which the Step 6.1 `Array.isArray` coercion absorbs; if it fails with a `.map` TypeError, the coercion was skipped). Then `pnpm -C crates/vox-gui/ui typecheck` → clean; `cargo test -p vox-gui task_priority_wire` → green. Optionally drive the mocked surface: `pnpm -C crates/vox-gui/ui exec playwright test e2e/screenshots.spec.ts --project=chromium`.
- [ ] **Step 8: Commit.**
  `git add crates/vox-gui/ui/src/lib/taskPriority.ts crates/vox-gui/ui/src/lib/taskPriority.test.ts crates/vox-gui/ui/src/components/surfaces/Tasks crates/vox-gui/src/commands/orchestrator.rs && git commit -m "feat(gui): origin-tagged merge-view Tasks read + mark-done + shared priority constant (B1/F1)"`

### Task 11 — Session rail rename/archive wiring

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx:14-19` (props), `:74-97` (session row)
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx`
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx:114-128` (`loadSessions`), `:194-204` (rail mount)
- `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (add `chat_rename_session` + `chat_archive_session` cases — mock coverage for Phase 3 Task 13's interaction specs)

Backend commands `chat_rename_session` (`chat.rs:229-244`) and `chat_archive_session` (`chat.rs:246-260`) are implemented and registered (`main.rs:146-147`); Tauri camelCase arg mapping means the frontend passes `{ sessionId, title }`. The rename/archive e2e *interaction* specs (driving the row menu against a stateful mock) are owned by **Phase 3 Task 13 (post-Phase-2)** — this task lands the component wiring, its vitest coverage, and the tauriMock command cases those specs will need.

- [ ] **Step 1: Failing component test.** Append to `ChatSessionRail.test.tsx` (existing idiom `:19-57`):

```tsx
  it('renames a session through the row menu', async () => {
    const user = userEvent.setup();
    const onRename = vi.fn();
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
          onRenameSession={onRename}
          onArchiveSession={vi.fn()}
        />
      </LanguageProvider>,
    );
    await user.click(screen.getByRole('button', { name: /session actions for First/i }));
    await user.click(screen.getByRole('menuitem', { name: /rename/i }));
    const input = screen.getByRole('textbox', { name: /new session title/i });
    await user.clear(input);
    await user.type(input, 'Renamed{Enter}');
    expect(onRename).toHaveBeenCalledWith('s1', 'Renamed');
  });

  it('archives a session through the row menu', async () => {
    const user = userEvent.setup();
    const onArchive = vi.fn();
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
          onRenameSession={vi.fn()}
          onArchiveSession={onArchive}
        />
      </LanguageProvider>,
    );
    await user.click(screen.getByRole('button', { name: /session actions for Second/i }));
    await user.click(screen.getByRole('menuitem', { name: /archive/i }));
    expect(onArchive).toHaveBeenCalledWith('s2');
  });
```

- [ ] **Step 2: Watch it fail.** `pnpm -C crates/vox-gui/ui test ChatSessionRail` → TS error: `onRenameSession` not in `ChatSessionRailProps` / role queries fail.
- [ ] **Step 3: Implement rail menu.** In `ChatSessionRail.tsx`:
  1. Props (`:14-19`):

```ts
export interface ChatSessionRailProps {
  sessions: ChatSessionItem[];
  activeSessionId: string;
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onRenameSession?: (sessionId: string, title: string) => void;
  onArchiveSession?: (sessionId: string) => void;
}
```

  2. State: `const [menuFor, setMenuFor] = useState<string | null>(null);` and `const [renaming, setRenaming] = useState<string | null>(null);` (`import React, { useState } from 'react';`).
  3. Replace the session-row `Button` body (`:74-97`) with a row wrapper: keep the existing tab `Button`, add beside it a kebab trigger + menu (only when the handlers are provided):

```tsx
          {sessions.map(s => {
            const isActive = s.session_id === activeSessionId;
            if (renaming === s.session_id) {
              return (
                <input
                  key={s.session_id}
                  aria-label="New session title"
                  defaultValue={s.title}
                  autoFocus
                  onKeyDown={e => {
                    if (e.key === 'Enter') {
                      const title = (e.target as HTMLInputElement).value.trim();
                      if (title) onRenameSession?.(s.session_id, title);
                      setRenaming(null);
                    }
                    if (e.key === 'Escape') setRenaming(null);
                  }}
                  className="w-full rounded-lg border border-brass/40 bg-bg-base px-2.5 py-2 text-xs text-text-primary outline-none"
                />
              );
            }
            return (
              <div key={s.session_id} className="relative flex items-stretch gap-1">
                <Button
                  role="tab"
                  aria-pressed={isActive}
                  aria-selected={isActive}
                  onClick={() => onSessionChange(s.session_id)}
                  className={`min-w-0 flex-1 justify-start rounded-lg border px-2.5 py-2 text-left text-xs ${
                    isActive
                      ? 'border-brass/40 bg-brass/10 text-brass'
                      : 'border-border-subtle text-text-muted hover:text-text-secondary'
                  }`}
                >
                  <span className="block truncate">{s.title}</span>
                  {s.message_count > 0 ? (
                    <span className="mt-0.5 block font-mono text-[10px] text-text-muted">
                      {s.message_count} msg{s.message_count === 1 ? '' : 's'}
                    </span>
                  ) : null}
                </Button>
                {(onRenameSession || onArchiveSession) && (
                  <button
                    type="button"
                    aria-label={`Session actions for ${s.title}`}
                    aria-haspopup="menu"
                    aria-expanded={menuFor === s.session_id}
                    onClick={() => setMenuFor(m => (m === s.session_id ? null : s.session_id))}
                    className="shrink-0 rounded px-1 text-text-muted hover:bg-overlay-subtle hover:text-text-secondary"
                  >
                    ⋯
                  </button>
                )}
                {menuFor === s.session_id && (
                  <div
                    role="menu"
                    className="absolute right-0 top-full z-50 mt-1 w-28 rounded-lg border border-border-subtle bg-bg-base p-1"
                  >
                    {onRenameSession && (
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => { setMenuFor(null); setRenaming(s.session_id); }}
                        className="w-full rounded px-2 py-1 text-left text-xs text-text-secondary hover:bg-overlay-subtle"
                      >
                        Rename
                      </button>
                    )}
                    {onArchiveSession && (
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => { setMenuFor(null); onArchiveSession(s.session_id); }}
                        className="w-full rounded px-2 py-1 text-left text-xs text-rose-300 hover:bg-overlay-subtle"
                      >
                        Archive
                      </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}
```

- [ ] **Step 4: Wire ChatSurface handlers.** In `ChatSurface.tsx` after `createSession` (`:176-186`):

```tsx
  const renameSession = async (sessionId: string, title: string) => {
    try {
      await invoke('chat_rename_session', { sessionId, title });
      await loadSessions();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Rename failed', body: String(err), cause: 'backend-error' });
    }
  };

  const archiveSession = async (sessionId: string) => {
    try {
      await invoke('chat_archive_session', { sessionId });
      const remaining = sessions.filter(s => s.session_id !== sessionId);
      setSessions(remaining);
      if (activeId === sessionId && remaining.length > 0) onSessionChange?.(remaining[0].session_id);
      await loadSessions();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Archive failed', body: String(err), cause: 'backend-error' });
    }
  };
```

  and pass them at the rail mount (`:194-204`): `onRenameSession={(id, t) => void renameSession(id, t)} onArchiveSession={id => void archiveSession(id)}`.
- [ ] **Step 4b: e2e mock coverage.** In `e2e/lib/tauriMock.ts`, beside the existing chat cases (`chat_append_message` at `:227`):
  `case 'chat_rename_session': return null;`
  `case 'chat_archive_session': return null;`
  (Phase 3 Task 13's rename/archive interaction spec replaces these with a stateful variant; landing the cases now means the commands never fall through to an unrelated `default` while that spec is built.)
- [ ] **Step 5: Watch it pass.** `pnpm -C crates/vox-gui/ui test ChatSessionRail` → all pass (including the 3 pre-existing tests — handlers are optional props, so they need no changes); `pnpm -C crates/vox-gui/ui typecheck` → clean. e2e sanity: `pnpm -C crates/vox-gui/ui exec playwright test e2e/chat-session-rail.spec.ts --project=chromium` still green — note this pre-existing spec has zero rename/archive coverage (verified by grep); it is a regression sanity check only, NOT the interaction spec, which lands in Phase 3 Task 13.
- [ ] **Step 6: Commit.**
  `git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/e2e/lib/tauriMock.ts && git commit -m "feat(gui): wire chat_rename_session/chat_archive_session into session rail menu"`

### Task 12 — Persist + render `model_id` on assistant messages

**Files:**
- `crates/vox-gui/ui/src/lib/chatCorrelation.ts:11-21` (`ChatMessage`), `:140-246` (reducer `agentEvent` cases)
- `crates/vox-gui/ui/src/lib/chatCorrelation.test.ts` (new cases)
- `crates/vox-gui/ui/src/lib/sessionChatStore.ts` (`resolveSessionForEvent` agent-id-scan group — gains `cost_incurred`)
- `crates/vox-gui/ui/src/lib/sessionChatStore.test.ts` (end-to-end routing test)
- `crates/vox-gui/ui/src/App.tsx:629-650` (hydrate), `:849-856` (assistant persist)
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx:14-50` (`MessageBubble` renders `ModelBadge`)

Backend already done and tested: `ChatAppendInput.model_id` (`chat.rs:127-129`), payload write (`chat.rs:150-162`), read-back (`chat.rs:95-117`), DTO tests (`chat.rs:322-352`). Model source: `cost_incurred` frames (`events.rs:274-277`, serde `tag="type", rename_all="snake_case"` at `events.rs:114-115`) on the unfiltered `vox://agent-events` re-emit (`orchestrator.rs:82`, `main.rs:86`).

**Two layers, both mandatory (F13):** at runtime App never feeds `chatReducer` directly — every frame goes `listenAgentEvents → dispatchSessionChat({ type: 'agentEvent', … })` (`App.tsx:443-452`), and `sessionChatStore.resolveSessionForEvent` drops any frame it cannot route (`if (!sessionId) return base;`). Its agent-id-scan group currently routes only `token_streamed | tool_timed_out | activity_changed | snapshot_captured` — without a `cost_incurred` branch, the reducer case below is dead code and the badge/persist/hydrate never fire. **Base state is post-Phase-1:** Phase 1 Task 8 added a `pending: AgentEventFrame[]` buffer to the store (`BUFFERABLE_TYPES = token_streamed/task_started` only) and left `resolveSessionForEvent` itself untouched, so the routing edit below applies cleanly on top; `cost_incurred` is deliberately NOT added to `BUFFERABLE_TYPES` (cost frames arrive long after `submitResolved`; a frame lost to that race is recovered by the persist/hydrate path anyway).

- [ ] **Step 1: Failing tests (reducer + store routing).** Append to `describe('chatReducer', …)` in `chatCorrelation.test.ts` (helpers `evt`/`assistant` at `:12-18`):

```ts
  it('stamps modelId on the assistant bubble from cost_incurred via the agent map', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'hi' });
    s = chatReducer(s, { type: 'submitResolved', runId: 'R1', taskId: '7' });
    s = chatReducer(s, evt({ type: 'task_started', task_id: 7, agent_id: 3 }));
    s = chatReducer(s, evt({ type: 'cost_incurred', agent_id: 3, provider: 'openrouter', model: 'anthropic/claude-opus-4.7' }));
    expect(assistant(s, 'R1')?.modelId).toBe('anthropic/claude-opus-4.7');
  });

  it('keeps the first modelId when multiple cost frames arrive', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'hi' });
    s = chatReducer(s, { type: 'submitResolved', runId: 'R1', taskId: '7' });
    s = chatReducer(s, evt({ type: 'task_started', task_id: 7, agent_id: 3 }));
    s = chatReducer(s, evt({ type: 'cost_incurred', agent_id: 3, provider: 'openrouter', model: 'model-a' }));
    s = chatReducer(s, evt({ type: 'cost_incurred', agent_id: 3, provider: 'openrouter', model: 'model-b' }));
    expect(assistant(s, 'R1')?.modelId).toBe('model-a');
  });

  it('ignores cost_incurred for unknown agents', () => {
    const s = chatReducer(initialChatState, evt({ type: 'cost_incurred', agent_id: 99, provider: 'x', model: 'm' }));
    expect(s.messages).toHaveLength(0);
  });
```

  And the **store-level routing test** — append to `describe('sessionChatStore', …)` in `sessionChatStore.test.ts` (its `evt` helper at `:9-13` builds the `{ id, timestamp_ms, kind }` frame shape). This drives `sessionChatReducer` end-to-end, exactly the path App uses at runtime — it fails even with a perfect `chatReducer` case if `resolveSessionForEvent` drops the frame:

```ts
  it('routes cost_incurred through the agent map and stamps modelId end-to-end', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3, session_id: 'sess-a' }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'cost_incurred', agent_id: 3, provider: 'openrouter', model: 'anthropic/claude-opus-4.7' }),
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.modelId).toBe('anthropic/claude-opus-4.7');
  });
```

- [ ] **Step 2: Watch them fail.** `pnpm -C crates/vox-gui/ui test chatCorrelation sessionChatStore` → TS error `Property 'modelId' does not exist on type 'ChatMessage'` / `expected undefined to be 'anthropic/claude-opus-4.7'` (the store test fails on both counts: no `modelId` AND `resolveSessionForEvent` returning `undefined` for `cost_incurred`).
- [ ] **Step 3: Implement reducer + session routing.**
  1. `chatCorrelation.ts` — `ChatMessage` (`:11-21`) gains `/** Model that produced this assistant message (from cost_incurred). */ modelId?: string;`.
  2. `chatCorrelation.ts` — new case inside the `agentEvent` switch, next to `task_started` (`:143-147`):

```ts
        case 'cost_incurred': {
          const agentId = String(kind.agent_id);
          const taskId = state.agentToTask[agentId];
          const runId = taskId ? state.taskToRun[taskId] : undefined;
          const model = typeof kind.model === 'string' ? kind.model : '';
          if (!model) return state;
          return mapAssistant(state, runId, (m) => (m.modelId ? m : { ...m, modelId: model }));
        }
```

  3. `sessionChatStore.ts` — add `cost_incurred` to `resolveSessionForEvent`'s agent-id-scan group (the frame carries `agent_id`, so the existing `state.agentToTask[agentId]` session scan routes it). Current code (`:67-72`, unchanged by Phase 1):

```ts
  if (
    type === 'token_streamed' ||
    type === 'tool_timed_out' ||
    type === 'activity_changed' ||
    type === 'snapshot_captured'
  ) {
```

  becomes:

```ts
  if (
    type === 'token_streamed' ||
    type === 'tool_timed_out' ||
    type === 'activity_changed' ||
    type === 'snapshot_captured' ||
    type === 'cost_incurred'
  ) {
```

  `pnpm -C crates/vox-gui/ui test chatCorrelation sessionChatStore` → green (including Phase 1 Task 8's pending-buffer tests, untouched).
- [ ] **Step 4: Persist + hydrate (App.tsx).**
  1. Persist (`:849-856`) — add the field to the input object AND replace the silent `.catch(() => {})` with the same toast idiom the user-persist path already uses (`App.tsx:677-679`) — this closes the spec's P2 "silent `.catch(() => {})` on assistant-message persist" finding, which no other task touches:

```ts
        invoke('chat_append_message', {
          input: {
            session_id: sessionId,
            role: 'assistant',
            content,
            task_id: m.taskId ?? null,
            model_id: m.modelId ?? null,
          },
        }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: String(err), cause: 'backend-error' }));
```

  (The enclosing effect's dependency array is `[chatStore]` — add `pushToast` to it; `pushToast` is a stable callback, so the effect's behavior is unchanged.)

  2. Hydrate (`:632-645`) — extend the row type and mapping:

```ts
      const rows = await invoke<
        Array<{ id: number; role: string; content: string; task_id?: string; model_id?: string }>
      >('chat_get_messages', { sessionId, limit: 500 });
      …
        messages: rows.map(r => ({
          id: String(r.id),
          role: r.role as 'user' | 'assistant' | 'system',
          text: r.content,
          status: 'done' as const,
          runId: r.task_id ?? `persist-${r.id}`,
          taskId: r.task_id ?? undefined,
          modelId: r.model_id ?? undefined,
        })),
```

- [ ] **Step 5: Render.** `ChatTranscript.tsx` — import the existing badge (`./ModelBadge`, already unit-tested) and add to `MessageBubble` after the failed-error block (`:43-47`):

```tsx
      {message.role === 'assistant' && message.status === 'done' && message.modelId && (
        <div className="mt-1 flex justify-end">
          <ModelBadge model={message.modelId} />
        </div>
      )}
```

- [ ] **Step 6: Verify.** `pnpm -C crates/vox-gui/ui test chatCorrelation sessionChatStore ModelBadge` → green; `pnpm -C crates/vox-gui/ui typecheck` → clean; `cargo test -p vox-gui chat_message_dto` → still green (backend contract unchanged). Runtime spot-check (post-merge smoke covers it too): with a live daemon, send a chat message and confirm the badge appears on completion and survives a session switch (hydrate path) — this spot-check only works because Step 3.3 routes `cost_incurred` through the session store.
- [ ] **Step 7: Commit.**
  `git add crates/vox-gui/ui/src/lib/chatCorrelation.ts crates/vox-gui/ui/src/lib/chatCorrelation.test.ts crates/vox-gui/ui/src/lib/sessionChatStore.ts crates/vox-gui/ui/src/lib/sessionChatStore.test.ts crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx && git commit -m "feat(gui): persist and render assistant model_id via cost_incurred correlation"`

---

## Out of scope (owned elsewhere — recorded so nothing falls between plans)

- **e2e interaction specs for the Phase 2 wiring** — model-picker apply, session rename/archive, and hopper mark-done Playwright specs are owned by **Phase 3 Task 13 (post-Phase-2)**, sequenced after this series lands. This plan deliberately ships only vitest component/unit coverage plus the `tauriMock.ts`/`tauriMockVariants.ts` command cases those specs will consume (`inference_provider_status` + `set_active_model` in Task 7, `hopper_mark_done` in Task 9, `chat_rename_session`/`chat_archive_session` in Task 11). Do NOT add interaction specs here; do NOT expect them to exist when verifying this series.
- **Attention-strip counts stay hopper-only** — per the spec's recorded scope limit (Resolved decisions item 6), `useAttentionInbox`/`blockedTasksCount` are intentionally not extended to orchestrator tasks in this series.

## Final verification (whole series)

- [ ] `cargo test -p vox-orchestrator -p vox-actor-runtime -p vox-orchestrator-mcp -p vox-config -p vox-db -p vox-gui > target/phase2-tests.log 2>&1; tail -n 40 target/phase2-tests.log` (redirect — never pipe cargo to head/grep; `tail` here is Git-Bash — in PowerShell use `Get-Content -Tail 40 target/phase2-tests.log`)
- [ ] `cargo clippy -p vox-orchestrator -p vox-actor-runtime -p vox-orchestrator-mcp -p vox-config -p vox-db -- -D warnings` (vox-gui deliberately excluded from clippy; it is covered by `cargo test -p vox-gui` + build)
- [ ] `pnpm -C crates/vox-gui/ui test` and `pnpm -C crates/vox-gui/ui typecheck` (the full vitest run includes the Phase-1 regression suites this series must keep green: `TasksView.copy.test.tsx` as rewritten by Task 10, `TasksView.listeners.test.tsx`, and the Phase-1 `sessionChatStore` pending-buffer tests)
- [ ] `pnpm -C crates/vox-gui/ui exec playwright test e2e/chat-session-rail.spec.ts e2e/screenshots.spec.ts --project=chromium` (pre-existing specs only — regression sanity; the interaction specs for this series' wiring land in Phase 3 Task 13)
- [ ] `cargo fmt -p vox-orchestrator -p vox-actor-runtime -p vox-orchestrator-mcp -p vox-config -p vox-db -p vox-gui` (NEVER `cargo fmt --all`)
