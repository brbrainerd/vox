//! Shared [`reqwest::Client`] / [`reqwest::ClientBuilder`] presets for Vox outbound HTTP.
//!
//! **Policy:** see `docs/src/architecture/outbound-http-policy.md` in the repo for when to use this crate, migration order, and exceptions.

pub mod envelope;

use std::time::Duration;

fn default_user_agent() -> String {
    format!(
        "vox-reqwest-defaults/{}",
        option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0")
    )
}

/// Builder with user-agent, connect timeout, and idle pool cap suitable for CLI and services.
pub fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(default_user_agent())
        .connect_timeout(Duration::from_secs(15))
        .pool_idle_timeout(Duration::from_secs(90))
}

/// Fall-back client when a custom builder chain fails to [`build`](reqwest::ClientBuilder::build).
pub fn client() -> reqwest::Client {
    client_builder()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
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
