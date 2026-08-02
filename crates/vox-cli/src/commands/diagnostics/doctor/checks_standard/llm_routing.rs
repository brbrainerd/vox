//! Secrets-first LLM routing readiness (model prefs + at least one provider key).

use super::super::common::{Check, redact_key};
use vox_config::inference::{OPENROUTER_CHAT_COMPLETIONS_URL, openrouter_chat_model_preference};
use vox_config::secrets_str;
use vox_secrets::SecretId;

pub fn run(checks: &mut Vec<Check>) {
    let model = openrouter_chat_model_preference();
    let routing_profile = secrets_str(SecretId::VoxRoutingProfile).unwrap_or_else(|| {
        // Default from routing contract / secrets spec — not a hard error when unset.
        "quality_first".to_string()
    });

    let mut keys: Vec<&'static str> = Vec::new();
    if vox_secrets::resolve_secret(SecretId::OpenRouterApiKey)
        .expose()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        keys.push("OpenRouter");
    }
    if vox_secrets::resolve_secret(SecretId::OpenaiApiKey)
        .expose()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        keys.push("OpenAI");
    }
    if vox_secrets::resolve_secret(SecretId::GeminiApiKey)
        .expose()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        keys.push("Gemini");
    }
    if vox_secrets::resolve_secret(SecretId::AnthropicApiKey)
        .expose()
        .filter(|s| !s.trim().is_empty())
        .is_some()
    {
        keys.push("Anthropic");
    }

    let budget_cfg = vox_config::VoxConfig::load();

    let detail = format!(
        "routing_profile={routing_profile}; openrouter_model={}; chat_completions_url={}; provider_keys_present=[{}]; daily_budget_usd={:.2}; per_session_budget_usd={:.2}",
        model,
        OPENROUTER_CHAT_COMPLETIONS_URL,
        if keys.is_empty() {
            "(none)".to_string()
        } else {
            keys.join(", ")
        },
        budget_cfg.daily_budget_usd,
        budget_cfg.per_session_budget_usd,
    );

    if keys.is_empty() {
        checks.push(Check::fail(
            "LLM routing (Secrets)",
            format!(
                "{detail} — no LLM API key resolved via Secrets; set e.g. OpenRouter via `vox secrets doctor` / login."
            ),
        ));
    } else {
        checks.push(Check::pass("LLM routing (Secrets)", detail));
    }

    // Informational: confirm account id for vault sync (optional).
    let acct = std::env::var(vox_secrets::OPERATOR_ACCOUNT_ID).unwrap_or_default();
    if acct.trim().is_empty() {
        checks.push(Check::new(
            "LLM routing — VOX_ACCOUNT_ID",
            true,
            "not set (optional for local keys only); use `vox secrets login` for cross-machine sync"
                .to_string(),
        ));
    } else {
        checks.push(Check::pass(
            "LLM routing — VOX_ACCOUNT_ID",
            format!("set ({})", redact_key(&acct)),
        ));
    }

    checks.push(Check::new(
        "Cloud vault login (profile)",
        true,
        crate::commands::login_shared::login_status_summary(),
    ));

    let cache_path = vox_config::paths::dot_vox_user_dir()
        .join("cache")
        .join("model-catalog.v1.json");
    let cache_status = if cache_path.exists() {
        match std::fs::read_to_string(&cache_path) {
            Ok(raw) => match serde_json::from_str::<Vec<serde_json::Value>>(&raw) {
                Ok(v) => format!(
                    "{} exists ({} cached model entries)",
                    cache_path.display(),
                    v.len()
                ),
                Err(_) => format!("{} exists (unparseable JSON)", cache_path.display()),
            },
            Err(e) => format!("{} exists (read error: {e})", cache_path.display()),
        }
    } else {
        format!(
            "{} missing — run `vox model discover`",
            cache_path.display()
        )
    };
    checks.push(Check::new(
        "LLM routing — model catalog cache",
        true,
        cache_status,
    ));
}

#[cfg(test)]
mod budget_status_tests {
    use super::*;

    #[test]
    fn reports_budget_caps_in_detail_string() {
        let mut checks = Vec::new();
        run(&mut checks);
        let llm_check = checks
            .iter()
            .find(|c| c.name == "LLM routing (Secrets)")
            .expect("LLM routing check present");
        assert!(
            llm_check.detail.contains("daily_budget_usd="),
            "detail should report the configured daily budget cap, got: {}",
            llm_check.detail
        );
    }
}

/// Task 12: distinguish "a credential is configured but OpenRouter's free tier is
/// rate-limiting us" from "no credential is configured at all" (the `LLM routing
/// (Secrets)` check above). Investigation (see the task's Step 1) found that
/// `vox_llm_egress::EgressError::RateLimited { retry_after }` is *already* a distinct
/// variant — not collapsed into a generic HTTP-error string — and that
/// `vox_actor_runtime::llm::chat::map_egress_error` already classifies it as
/// `error_class = "rate-limited"` when a real dispatch hits a 429
/// (`crates/vox-llm-egress/tests/wire_mock.rs::chat_once_maps_429_to_rate_limited`
/// already locks that wire-level behavior in).
///
/// What's *not* available today is a resolved `EgressError` inside this check: unlike
/// `model_telemetry.rs` (which reads already-recorded rows from `vox-db`), `run()`
/// above is a purely local/offline check (Secrets presence, `VoxConfig`, on-disk model
/// catalog cache) — it never dispatches a request, and `vox doctor` intentionally makes
/// no live provider calls elsewhere (see `provider_policy.rs`'s "no network I/O" doc
/// comment). Doctor probing OpenRouter live would itself burn a real free-tier request
/// just to run a diagnostic — exactly the kind of budget erosion this plan's Phase 1
/// exists to prevent. Nor is there yet a persisted "last dispatch outcome" a fresh `vox
/// doctor` process could read locally (the `llm_attempts` table in `vox-db` records
/// `error_class`, but there is no read query for it yet, and adding one is a bigger,
/// separately-scoped change than this task's stated file list).
///
/// So this function is the classification building block, not yet called from `run()`.
/// It takes a resolved `EgressError` — from a real dispatch, however it eventually gets
/// here (Task 12b's live-dispatch wiring, or a future doctor check that reads a
/// persisted last-attempt signal) — and produces the distinct rate-limit `Check` in
/// place of treating it as an opaque failure.
#[allow(dead_code)] // not yet called from `run()` — see doc comment above for why.
pub(crate) fn rate_limit_check(err: &vox_llm_egress::EgressError) -> Option<Check> {
    match err {
        vox_llm_egress::EgressError::RateLimited { retry_after } => {
            let reset = match retry_after {
                Some(d) => format!("in ~{}s", d.as_secs().max(1)),
                None => "at an unknown time".to_string(),
            };
            Some(Check::fail(
                "LLM routing (rate limit)",
                format!(
                    "OpenRouter free tier limit reached — resets {reset}; add your own key \
                     (`vox secrets login --oauth --provider openrouter`) or wait."
                ),
            ))
        }
        vox_llm_egress::EgressError::Status { .. }
        | vox_llm_egress::EgressError::Http(_)
        | vox_llm_egress::EgressError::Decode(_) => None,
    }
}

#[cfg(test)]
mod rate_limit_check_tests {
    use super::*;
    use std::time::Duration;
    use vox_llm_egress::EgressError;

    #[test]
    fn rate_limited_error_produces_distinct_check() {
        let err = EgressError::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        };
        let check = rate_limit_check(&err).expect("RateLimited must produce a distinct Check");
        assert_eq!(check.name, "LLM routing (rate limit)");
        assert!(!check.pass, "rate limiting is a non-pass condition");
        assert!(
            check.detail.to_lowercase().contains("free tier limit"),
            "detail should mention the free tier limit, got: {}",
            check.detail
        );
    }

    #[test]
    fn rate_limited_error_without_retry_after_still_produces_check() {
        let err = EgressError::RateLimited { retry_after: None };
        let check = rate_limit_check(&err).expect("RateLimited must produce a distinct Check");
        assert_eq!(check.name, "LLM routing (rate limit)");
        assert!(check.detail.to_lowercase().contains("free tier limit"));
    }

    #[test]
    fn non_rate_limit_errors_produce_no_rate_limit_check() {
        assert!(rate_limit_check(&EgressError::Http("connection reset".into())).is_none());
        assert!(
            rate_limit_check(&EgressError::Status {
                code: 401,
                body: "invalid api key".into(),
            })
            .is_none()
        );
        assert!(rate_limit_check(&EgressError::Decode("bad json".into())).is_none());
    }
}
