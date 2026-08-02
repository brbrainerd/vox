//! Secrets-first LLM routing readiness (model prefs + at least one provider key).

use super::super::common::{Check, redact_key};
use vox_actor_runtime::llm::RATE_LIMITED_ERROR_CLASS;
use vox_config::inference::{OPENROUTER_CHAT_COMPLETIONS_URL, openrouter_chat_model_preference};
use vox_config::secrets_str;
use vox_secrets::SecretId;

pub async fn run(checks: &mut Vec<Check>) {
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

    // Task 12 (follow-up): distinguish "a credential is configured but OpenRouter's
    // free tier is currently rate-limiting us" from plain success/failure of the
    // Secrets check above. See `recent_rate_limit_check`'s doc comment for the local
    // (no live network call) DB-read approach.
    if let Some(check) = recent_rate_limit_check().await {
        checks.push(check);
    }
}

#[cfg(test)]
mod budget_status_tests {
    use super::*;

    #[tokio::test]
    async fn reports_budget_caps_in_detail_string() {
        // `run()` unconditionally calls `recent_rate_limit_check()`, which reads
        // `VOX_DB_PATH` via `DbConfig::resolve_canonical()` — the same process-global
        // env var `recent_rate_limit_check_tests` guards with `VOX_DB_PATH_LOCK`. This
        // test must hold that same lock (and redirect to its own temp DB) or it races
        // with those tests under cargo test's default multi-threaded execution.
        let _guard = super::recent_rate_limit_check_tests::VOX_DB_PATH_LOCK
            .lock()
            .unwrap();
        let (_dir, previous) = super::recent_rate_limit_check_tests::redirect_to_temp_db();

        let mut checks = Vec::new();
        run(&mut checks).await;
        super::recent_rate_limit_check_tests::restore_env(previous);

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
/// Called from [`recent_rate_limit_check`], which adapts a `vox-db`-read
/// [`vox_db::store::types::LastLlmAttemptRow`] into a synthetic
/// `EgressError::RateLimited` (the DB doesn't persist `retry_after`, so that field is
/// always `None` when reached this way — see that function's doc comment for why a DB
/// read rather than a live probe).
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

/// How long a recorded `llm_attempts` row is trusted as still representing "the free
/// tier is rate limited right now." Long enough to survive the gap between a chat turn
/// hitting a 429 and the user running `vox doctor` to investigate it; short enough that
/// an hours-old rate limit (which has almost certainly cleared — OpenRouter free-tier
/// throttle windows are on the order of seconds to low minutes, per
/// `vox_llm_egress::throttle`'s `Retry-After`/`X-RateLimit-Reset` handling, capped at
/// 120s there) isn't reported as a live condition.
const RATE_LIMIT_STALENESS_SECS: f64 = 300.0;

/// Reads the most recently recorded LLM dispatch attempt from `vox-db` (the
/// `llm_attempts` table, written by every real `llm_chat`/`llm_stream` call — see
/// `vox_actor_runtime::llm::chat::record_telemetry_attempt`) and, if it was a
/// rate-limited failure within [`RATE_LIMIT_STALENESS_SECS`], returns
/// [`rate_limit_check`]'s distinct `Check`.
///
/// This is a **local-only** read (no network I/O), consistent with `vox doctor`'s
/// existing convention elsewhere (see `provider_policy.rs`'s "no network I/O" doc
/// comment, and `model_telemetry.rs`'s identical `DbConfig::resolve_canonical()` +
/// `VoxDb::connect` pattern for a different table). It deliberately does **not** make a
/// live provider call itself — that would burn a real free-tier request just to run a
/// diagnostic, undermining this plan's Phase 1 budget-protection work — so the signal
/// is only as fresh as the most recent *real* dispatch a user (or the GUI) already made.
///
/// Returns `None` on any DB error, when no attempt has ever been recorded, when the
/// most recent attempt is stale or has a nonsensical (negative) age, or when it wasn't a
/// rate-limit failure — in every one of those cases `run()` silently falls back to
/// showing only the `LLM routing (Secrets)` check.
async fn recent_rate_limit_check() -> Option<Check> {
    let cfg = vox_db::DbConfig::resolve_canonical().ok()?;
    let db = vox_db::VoxDb::connect(cfg).await.ok()?;
    let last = db.get_last_llm_attempt().await.ok().flatten()?;
    if !(0.0..=RATE_LIMIT_STALENESS_SECS).contains(&last.age_seconds) {
        return None;
    }
    if last.error_class.as_deref() != Some(RATE_LIMITED_ERROR_CLASS) {
        return None;
    }
    // The DB row doesn't carry `retry_after` (only `error_class`/`outcome`/timing are
    // persisted), so the synthetic error always resolves to `rate_limit_check`'s
    // "resets at an unknown time" branch.
    rate_limit_check(&vox_llm_egress::EgressError::RateLimited { retry_after: None })
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

#[cfg(test)]
mod recent_rate_limit_check_tests {
    use super::*;
    use std::sync::Mutex;

    /// `recent_rate_limit_check` (via `DbConfig::resolve_canonical`) reads
    /// `VOX_DB_PATH` when set, else falls back to the real user-global DB file — so
    /// every test in this module (and `budget_status_tests::reports_budget_caps_in_detail_string`,
    /// which exercises `run()` and therefore `recent_rate_limit_check()` too) redirects
    /// it to an isolated `tempfile` DB and holds this lock for the duration, the same
    /// pattern `harness::eval::tests::LIVE_ENV_VAR_LOCK` uses for its process-global env
    /// var. `pub(super)` so that sibling test module can reuse the same lock/helpers
    /// instead of duplicating (and risking drift from) this pattern.
    pub(super) static VOX_DB_PATH_LOCK: Mutex<()> = Mutex::new(());

    /// Points `VOX_DB_PATH` at a fresh temp file and returns a guard that restores the
    /// previous value on drop. Must be called while holding `VOX_DB_PATH_LOCK`.
    pub(super) fn redirect_to_temp_db() -> (tempfile::TempDir, Option<String>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("doctor-rate-limit-test.db");
        let previous = std::env::var("VOX_DB_PATH").ok();
        // SAFETY: guarded by VOX_DB_PATH_LOCK; no other test touches this var without
        // holding the same lock.
        unsafe {
            std::env::set_var("VOX_DB_PATH", db_path.to_str().expect("utf8 path"));
        }
        (dir, previous)
    }

    pub(super) fn restore_env(previous: Option<String>) {
        // SAFETY: guarded by VOX_DB_PATH_LOCK (caller holds it for the whole test).
        unsafe {
            match previous {
                Some(v) => std::env::set_var("VOX_DB_PATH", v),
                None => std::env::remove_var("VOX_DB_PATH"),
            }
        }
    }

    #[tokio::test]
    async fn no_recorded_attempts_yields_no_rate_limit_check() {
        let _guard = VOX_DB_PATH_LOCK.lock().unwrap();
        let (_dir, previous) = redirect_to_temp_db();

        // Force schema creation (an empty temp path has no DB file / tables yet) by
        // connecting once before exercising the check under test.
        let cfg = vox_db::DbConfig::resolve_canonical().expect("resolve");
        vox_db::VoxDb::connect(cfg)
            .await
            .expect("connect creates schema");

        let check = recent_rate_limit_check().await;
        restore_env(previous);
        assert!(
            check.is_none(),
            "no llm_attempts rows recorded — must not surface a rate-limit Check"
        );
    }

    #[tokio::test]
    async fn recent_rate_limited_attempt_surfaces_distinct_check_via_run() {
        let _guard = VOX_DB_PATH_LOCK.lock().unwrap();
        let (_dir, previous) = redirect_to_temp_db();

        let cfg = vox_db::DbConfig::resolve_canonical().expect("resolve");
        let db = vox_db::VoxDb::connect(cfg).await.expect("connect");
        db.record_llm_attempt(vox_db::store::types::ModelAttempt {
            trace_id: "trace-doctor-test",
            attempt_number: 1,
            model_id: "openrouter/free-model",
            provider: "openrouter",
            outcome: "error",
            latency_ms: Some(0),
            error_class: Some("rate-limited"),
        })
        .await
        .expect("record a just-happened rate-limited attempt");

        let mut checks = Vec::new();
        run(&mut checks).await;
        restore_env(previous);

        let rate_limit = checks
            .iter()
            .find(|c| c.name == "LLM routing (rate limit)")
            .expect("run() must surface the distinct rate-limit Check end-to-end");
        assert!(!rate_limit.pass);
        assert!(
            rate_limit.detail.to_lowercase().contains("free tier limit"),
            "got: {}",
            rate_limit.detail
        );
        // The generic Secrets check is unaffected — this is additive, not a replacement.
        assert!(checks.iter().any(|c| c.name == "LLM routing (Secrets)"));
    }

    #[tokio::test]
    async fn stale_rate_limited_attempt_does_not_surface_check() {
        let _guard = VOX_DB_PATH_LOCK.lock().unwrap();
        let (_dir, previous) = redirect_to_temp_db();

        let cfg = vox_db::DbConfig::resolve_canonical().expect("resolve");
        let db = vox_db::VoxDb::connect(cfg).await.expect("connect");
        db.connection()
            .execute(
                "INSERT INTO llm_attempts
                     (trace_id, attempt_number, model_id, provider, outcome, latency_ms, error_class, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now', '-1 hour'))",
                turso::params![
                    "trace-stale",
                    1i32,
                    "openrouter/free-model",
                    "openrouter",
                    "error",
                    0i64,
                    "rate-limited",
                ],
            )
            .await
            .expect("insert stale rate-limited row");

        let check = recent_rate_limit_check().await;
        restore_env(previous);
        assert!(
            check.is_none(),
            "an hour-old rate-limited attempt is outside the staleness window and must \
             not be reported as a live condition"
        );
    }
}
