use std::time::Duration;

/// Retry policy for resilient outbound HTTP calls.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: usize,
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

#[derive(Debug, thiserror::Error)]
pub enum ResilientHttpError {
    #[error("all endpoints failed after retries: {0}")]
    Exhausted(String),
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
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            client: reqwest::Client::new(),
            policy,
        }
    }

    pub fn from_env() -> Self {
        let max_attempts = std::env::var("VOX_HTTP_RETRY_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(3);
        let base_delay_ms = std::env::var("VOX_HTTP_RETRY_BASE_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(200);
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
        let factor = (attempt.saturating_sub(1)) as u32;
        let backoff = self.policy.base_delay_ms.saturating_mul(2u64.pow(factor));
        Duration::from_millis(backoff)
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
