//! Map `distribution_policy.retry_profile` / `rate_limit_profile` into bounded HTTP retries for social syndication.
//!
//! **Crate `backon`:** not used here — [`vox_foundation::primitives::backoff::backoff_ms_geometric_attempt`] matches
//! publisher policy (capped geometric delay, no extra dependency). Revisit only if we unify many async
//! retry loops behind one tower-style abstraction.

use std::time::Duration;

use crate::types::UnifiedNewsItem;

/// At least one attempt; caps total wall-clock backoff per channel.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SocialRetryBudget {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
}

#[must_use]
pub(crate) fn budget_from_distribution_policy(item: &UnifiedNewsItem) -> SocialRetryBudget {
    let rp = item
        .syndication
        .distribution_policy
        .retry_profile
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let rlp = item
        .syndication
        .distribution_policy
        .rate_limit_profile
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let (mut attempts, mut base_ms) = match rp.to_ascii_lowercase().as_str() {
        "aggressive" | "high" => (5u32, 400u64),
        "minimal" | "low" => (2u32, 1_200u64),
        "" | "standard" | "normal" => (3u32, 800u64),
        _ => (3u32, 800u64),
    };
    attempts = attempts.clamp(1, 8);
    match rlp.to_ascii_lowercase().as_str() {
        "patient" | "slow" | "strict" => {
            base_ms = base_ms.saturating_mul(2).min(15_000);
        }
        _ => {}
    }
    SocialRetryBudget {
        max_attempts: attempts,
        base_delay_ms: base_ms,
    }
}

/// Runs `f` up to `budget.max_attempts` times with exponential backoff (capped at 60s).
pub(crate) async fn run_with_retries<T, E, Fut, F>(
    budget: SocialRetryBudget,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut last_err: Option<E> = None;
    for attempt in 1..=budget.max_attempts {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt < budget.max_attempts && budget.base_delay_ms > 0 {
                    let ms = vox_foundation::primitives::backoff::backoff_ms_geometric_attempt(
                        attempt,
                        budget.base_delay_ms,
                        60_000,
                        6,
                    );
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                }
            }
        }
    }
    Err(last_err.expect("social retry: expected last error after failed attempts"))
}
