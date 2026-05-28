//! Resilient HTTP with geometric backoff ([`vox_foundation::primitives::backoff`]).
//!
//! **`backon` (crate):** evaluated and not adopted here: our retry surface is
//! endpoint fallback + capped geometric delay only; `backon` would add another
//! policy layer without simplifying multi-endpoint loops. Revisit if we
//! centralize **single-URL** async retries (e.g. Ludus transport) behind one helper.

use std::time::Duration;

/// Retry policy for resilient outbound HTTP calls.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Attempts per endpoint before trying the next or failing.
    pub max_attempts: usize,
    /// Initial backoff for the first retry on an endpoint (exponential growth).
    pub base_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 200,
        }
    }
}

/// Errors from [`ResilientHttpClient`] when no endpoint succeeds.
#[derive(Debug, thiserror::Error)]
pub enum ResilientHttpError {
    /// Every endpoint exhausted its retry budget.
    #[error("all endpoints failed after retries: {0}")]
    Exhausted(String),
    /// Invalid configuration (e.g. empty endpoint list) or client build failure.
    #[error("request build error: {0}")]
    Build(String),
}

/// HTTP client with retry and endpoint fallback support.
#[derive(Clone)]
pub struct ResilientHttpClient {
    client: reqwest::Client,
    policy: RetryPolicy,
}

impl ResilientHttpClient {
    /// Wraps a fresh `reqwest` client with the given retry policy.
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            client: vox_http_client::client(),
            policy,
        }
    }

    /// Builds a client from `VOX_HTTP_RETRY_MAX_ATTEMPTS` and `VOX_HTTP_RETRY_BASE_DELAY_MS` (with defaults).
    pub fn from_env() -> Self {
        let max_attempts =
            vox_config::env_parse::env_usize("VOX_HTTP_RETRY_MAX_ATTEMPTS", 3).max(1);
        let base_delay_ms = vox_config::env_parse::env_u64("VOX_HTTP_RETRY_BASE_DELAY_MS", 200);
        Self::new(RetryPolicy {
            max_attempts,
            base_delay_ms,
        })
    }

    /// POST JSON payload to a list of endpoints; retries each endpoint and falls back.
    pub async fn post_json_with_fallback<T: serde::Serialize + ?Sized>(
        &self,
        endpoints: &[String],
        body: &T,
        bearer_token: Option<&str>,
    ) -> Result<reqwest::Response, ResilientHttpError> {
        if endpoints.is_empty() {
            return Err(ResilientHttpError::Build(
                "no endpoints configured".to_string(),
            ));
        }
        let mut failures = Vec::new();
        for endpoint in endpoints {
            for attempt in 1..=self.policy.max_attempts {
                let mut req = self.client.post(endpoint).json(body);
                if let Some(token) = bearer_token {
                    req = req.bearer_auth(token);
                }
                match req.send().await {
                    Ok(resp) if resp.status().is_success() => return Ok(resp),
                    Ok(resp) => failures.push(format!(
                        "{endpoint} attempt {attempt} returned {}",
                        resp.status()
                    )),
                    Err(err) => {
                        failures.push(format!("{endpoint} attempt {attempt} failed: {err}"))
                    }
                }
                if attempt < self.policy.max_attempts {
                    tokio::time::sleep(self.backoff_duration(attempt)).await;
                }
            }
        }
        Err(ResilientHttpError::Exhausted(failures.join(" | ")))
    }

    fn backoff_duration(&self, attempt: usize) -> Duration {
        let ms = vox_foundation::primitives::backoff::backoff_ms_geometric_attempt(
            attempt as u32,
            self.policy.base_delay_ms,
            60_000,
            16,
        );
        Duration::from_millis(ms)
    }
}

impl Default for ResilientHttpClient {
    fn default() -> Self {
        Self::new(RetryPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_increases() {
        let client = ResilientHttpClient::new(RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 50,
        });
        assert_eq!(client.backoff_duration(1), Duration::from_millis(50));
        assert_eq!(client.backoff_duration(2), Duration::from_millis(100));
        assert_eq!(client.backoff_duration(3), Duration::from_millis(200));
    }
}
