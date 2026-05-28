//! Shared [`reqwest::Client`] / [`reqwest::ClientBuilder`] presets for Vox outbound HTTP.
//!
//! **Policy:** see `docs/src/architecture/outbound-http-policy.md` in the repo for when to use this crate, migration order, and exceptions.

pub mod envelope;

use reqwest::header::{HeaderValue, InvalidHeaderValue};
use std::time::Duration;

/// Default connect timeout for outbound HTTP (15s — interactive CLI / service).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Idle connection pool timeout (90s).
pub const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

fn default_user_agent() -> String {
    format!(
        "vox-http-client/{}",
        option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0")
    )
}

/// Builder with user-agent, connect timeout, and idle pool cap suitable for CLI and services.
pub fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(default_user_agent())
        .connect_timeout(CONNECT_TIMEOUT)
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
}

/// Fall-back client when a custom builder chain fails to [`build`](reqwest::ClientBuilder::build).
pub fn client() -> reqwest::Client {
    client_builder()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Build an `Authorization: Bearer <token>` header value. Returns `Err` if the token
/// contains characters invalid in an HTTP header value (control chars, non-ASCII).
///
/// SSOT for bearer-token formatting — drift-check flags inline `"Bearer {token}"` strings.
pub fn bearer_auth_header(token: &str) -> Result<HeaderValue, InvalidHeaderValue> {
    let mut v = HeaderValue::from_str(&format!("Bearer {token}"))?;
    v.set_sensitive(true);
    Ok(v)
}

/// As [`bearer_auth_header`], formatted as a raw `String` (for headers maps keyed by string).
pub fn bearer_auth_header_string(token: &str) -> String {
    format!("Bearer {token}")
}

#[cfg(feature = "middleware")]
mod populi_middleware {
    use async_trait::async_trait;
    use http::Extensions;
    use reqwest::{Request, Response};
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next, Result};
    use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

    /// Lightweight outbound trace hook for Populi control-plane requests (retry stack sits inner).
    #[derive(Clone, Debug)]
    struct PopuliOutboundTraceMiddleware;

    #[async_trait]
    impl Middleware for PopuliOutboundTraceMiddleware {
        async fn handle(
            &self,
            req: Request,
            extensions: &mut Extensions,
            next: Next<'_>,
        ) -> Result<Response> {
            tracing::trace!(
                target: "vox.http.populi",
                method = %req.method(),
                url = %req.url(),
                "populi_control_plane_request"
            );
            next.run(req, extensions).await
        }
    }

    /// Wrap a fully configured [`reqwest::Client`] with Populi middleware (trace + optional transient retries).
    ///
    /// When `retry_transient` is true, installs [`RetryTransientMiddleware`] with a small exponential backoff
    /// cap (see `reqwest-retry` defaults). Inner client timeouts and TLS remain unchanged.
    pub fn populi_control_plane_client(
        inner: reqwest::Client,
        retry_transient: bool,
    ) -> ClientWithMiddleware {
        let mut builder = ClientBuilder::new(inner).with(PopuliOutboundTraceMiddleware);
        if retry_transient {
            let retry_policy = ExponentialBackoff::builder().build_with_max_retries(2);
            builder = builder.with(RetryTransientMiddleware::new_with_policy(retry_policy));
        }
        builder.build()
    }
}

#[cfg(feature = "middleware")]
pub use populi_middleware::populi_control_plane_client;
