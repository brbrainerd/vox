//! HIR nodes for Phase 3 HTTP ergonomics decorators (GA-06).
//!
//! `@cors`, `@rate_limit`, and `@endpoint(method:, path:)` overrides.
//! These ride on `HirEndpointFn` as optional sidecar fields rather than
//! being separate HIR declarations.

use crate::ast::span::Span;

/// CORS policy attached to an `@endpoint` via `@cors(origins: [...])`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirCorsPolicy {
    /// Allowed origins. `["*"]` means permissive; empty means deny-all.
    pub origins: Vec<String>,
    /// Whether credentials (`Cookie`, `Authorization`) are allowed cross-origin.
    pub allow_credentials: bool,
    pub span: Span,
}

impl HirCorsPolicy {
    /// Return `true` if this policy permits the given origin.
    pub fn allows_origin(&self, origin: &str) -> bool {
        self.origins.iter().any(|o| o == "*" || o == origin)
    }
}

/// Rate-limit policy attached to an `@endpoint` via
/// `@rate_limit(by: ip, per: 1m, max: 100)`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirRateLimitPolicy {
    /// Discriminant for the rate-limit bucket.
    pub by: RateLimitBy,
    /// Window in seconds.
    pub window_secs: u64,
    /// Maximum requests per window.
    pub max_requests: u64,
    pub span: Span,
}

/// How the rate-limit bucket is keyed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RateLimitBy {
    /// Per remote IP address.
    Ip,
    /// Per authenticated user ID.
    UserId,
    /// Per API key header value.
    ApiKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::span::Span;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn cors_wildcard_allows_any_origin() {
        let policy = HirCorsPolicy {
            origins: vec!["*".into()],
            allow_credentials: false,
            span: span(),
        };
        assert!(policy.allows_origin("https://example.com"));
    }

    #[test]
    fn cors_specific_origin_denies_other() {
        let policy = HirCorsPolicy {
            origins: vec!["https://trusted.com".into()],
            allow_credentials: false,
            span: span(),
        };
        assert!(policy.allows_origin("https://trusted.com"));
        assert!(!policy.allows_origin("https://evil.com"));
    }

    #[test]
    fn cors_empty_denies_all() {
        let policy = HirCorsPolicy {
            origins: vec![],
            allow_credentials: false,
            span: span(),
        };
        assert!(!policy.allows_origin("https://example.com"));
    }
}
