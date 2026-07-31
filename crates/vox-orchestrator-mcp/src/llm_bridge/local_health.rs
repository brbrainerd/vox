//! Short-TTL local-backend (Ollama/PopuliMesh) health gate for the synchronous
//! MCP model resolver. Peeks the vox-actor-runtime probe cache; unknown health
//! is optimistic (allowed) — the reactive fallback chain in the infer loop
//! remains the safety net for the first call after startup. VoxLocal is NOT
//! gated here: it runs on its own server (`VOX_LOCAL_ENDPOINT`) with its own
//! probe (`providers::probe_vox_local_health`) already in the call path.

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
static TEST_HEALTH_OVERRIDE: std::sync::Mutex<Option<Option<bool>>> = std::sync::Mutex::new(None);

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

/// Test-only seam for `VOX_INFERENCE_PRIVACY`, mirroring
/// `TEST_HEALTH_OVERRIDE` above — avoids mutating the real process env (which
/// is racy under parallel `cargo test`) while still exercising the real
/// decision logic in `privacy_allows`.
#[cfg(test)]
static TEST_PRIVACY_OVERRIDE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn set_test_privacy_override(v: Option<&str>) {
    *TEST_PRIVACY_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = v.map(str::to_string);
}

fn inference_privacy_mode() -> String {
    #[cfg(test)]
    if let Some(v) = TEST_PRIVACY_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return v;
    }
    std::env::var("VOX_INFERENCE_PRIVACY").unwrap_or_else(|_| "any".to_string())
}

/// Hard privacy filter consulted by the resolver's `routing_allows`, keyed
/// off `VOX_INFERENCE_PRIVACY` (`any` [default] | `local_only`).
///
/// This is UNRELATED to `VOX_MESH_EXEC_POLICY`: that key governs whether a
/// mesh *task* may be relayed for execution on another physical node
/// (task placement), while `VOX_INFERENCE_PRIVACY` governs whether a *model*
/// running on a remote cloud provider may ever be selected for an inference
/// call (F7). A user relying on `VOX_MESH_EXEC_POLICY=local_only` to keep
/// prompts off the network gets no such protection — this is the actual
/// control for that.
///
/// A hard filter, not a ranking hint: when `local_only`, non-local-provider
/// models are excluded from candidates entirely (see
/// `route_policy::is_local_http_provider` for the local/cloud split reused
/// here), never merely deprioritized. There is currently no per-request
/// override surface on the chat params path; if one is ever added, it MUST
/// be ANDed with this account/session-level setting (one-way ratchet: an
/// override may only tighten to `local_only`, never loosen an already-set
/// `local_only` back to `any`), mirroring OpenRouter's `zdr` semantics.
pub(crate) fn privacy_allows(m: &ModelSpec) -> bool {
    if inference_privacy_mode().trim().eq_ignore_ascii_case("local_only") {
        return vox_orchestrator::route_policy::is_local_http_provider(&m.provider_type);
    }
    true
}

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
        assert!(!local_candidate_allowed(&gate_spec(
            "ollama-m",
            ProviderType::Ollama
        )));
        assert!(!local_candidate_allowed(&gate_spec(
            "mesh-m",
            ProviderType::PopuliMesh
        )));
        assert!(local_candidate_allowed(&gate_spec(
            "vox-m",
            ProviderType::VoxLocal
        )));
        assert!(local_candidate_allowed(&gate_spec(
            "or-m",
            ProviderType::OpenRouter
        )));
        set_test_health_override(None);
        // Override cleared ⇒ real path; no fresh probe of the config base URL
        // in a unit-test env ⇒ unknown ⇒ optimistic.
        assert!(local_candidate_allowed(&gate_spec(
            "ollama-m",
            ProviderType::Ollama
        )));
    }

    // Both `any` and `local_only` cases live in one test (rather than two
    // separate `#[test]` fns) because `TEST_PRIVACY_OVERRIDE` is shared
    // process-global state: under `cargo test`'s default parallel threading,
    // two tests setting/clearing it independently would race and flake.
    #[test]
    fn privacy_allows_any_passes_all_local_only_excludes_cloud_hard() {
        set_test_privacy_override(Some("any"));
        assert!(privacy_allows(&gate_spec("ollama-m", ProviderType::Ollama)));
        assert!(privacy_allows(&gate_spec("vox-m", ProviderType::VoxLocal)));
        assert!(privacy_allows(&gate_spec("or-m", ProviderType::OpenRouter)));
        assert!(privacy_allows(&gate_spec(
            "anthropic-m",
            ProviderType::Anthropic
        )));

        set_test_privacy_override(Some("local_only"));
        assert!(privacy_allows(&gate_spec("ollama-m", ProviderType::Ollama)));
        assert!(privacy_allows(&gate_spec(
            "mesh-m",
            ProviderType::PopuliMesh
        )));
        assert!(privacy_allows(&gate_spec("vox-m", ProviderType::VoxLocal)));
        assert!(!privacy_allows(&gate_spec("or-m", ProviderType::OpenRouter)));
        assert!(!privacy_allows(&gate_spec(
            "anthropic-m",
            ProviderType::Anthropic
        )));

        // No override set: falls through to the real env read. In a unit-test
        // process VOX_INFERENCE_PRIVACY is normally unset, so this proves the
        // documented default ("any") when the key is absent entirely. (Kept
        // in this same test, not a separate `#[test]` fn, so the override
        // clear above and this real-env check can't race against another
        // thread's override mutation.)
        set_test_privacy_override(None);
        if std::env::var("VOX_INFERENCE_PRIVACY").is_err() {
            assert!(privacy_allows(&gate_spec("or-m", ProviderType::OpenRouter)));
        }
    }
}
