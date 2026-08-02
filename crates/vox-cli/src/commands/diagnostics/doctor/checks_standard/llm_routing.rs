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
