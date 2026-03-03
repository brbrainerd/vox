use std::time::Duration;

/// Authentication configuration used by generated Vox services.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub api_key: Option<String>,
    pub bearer_token: Option<String>,
    pub allow_unauthenticated: bool,
}

impl AuthConfig {
    /// Build auth config from environment variables.
    ///
    /// - `VOX_API_KEY`
    /// - `VOX_BEARER_TOKEN`
    /// - `VOX_ALLOW_UNAUTHENTICATED` (`true`/`false`, defaults to `true` when no key/token set)
    pub fn from_env() -> Self {
        let api_key = std::env::var("VOX_API_KEY").ok().filter(|v| !v.is_empty());
        let bearer_token = std::env::var("VOX_BEARER_TOKEN")
            .ok()
            .filter(|v| !v.is_empty());
        let allow_unauthenticated = std::env::var("VOX_ALLOW_UNAUTHENTICATED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(api_key.is_none() && bearer_token.is_none());
        Self {
            api_key,
            bearer_token,
            allow_unauthenticated,
        }
    }
}

/// Validate an incoming API key and/or bearer token.
pub fn authorize_request(
    api_key_header: Option<&str>,
    auth_header: Option<&str>,
    cfg: &AuthConfig,
) -> bool {
    if cfg.allow_unauthenticated {
        return true;
    }
    if let Some(required_key) = &cfg.api_key {
        let provided = api_key_header.unwrap_or_default();
        if constant_time_eq(provided.as_bytes(), required_key.as_bytes()) {
            return true;
        }
    }
    if let Some(required_token) = &cfg.bearer_token {
        let provided = parse_bearer_token(auth_header).unwrap_or_default();
        if constant_time_eq(provided.as_bytes(), required_token.as_bytes()) {
            return true;
        }
    }
    false
}

/// Parse `Authorization: Bearer <token>`.
pub fn parse_bearer_token(header: Option<&str>) -> Option<&str> {
    let value = header?.trim();
    let prefix = "Bearer ";
    value.strip_prefix(prefix)
}

/// Constant-time byte comparison to avoid timing side channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Parse an optional header duration used by throttling.
pub fn parse_retry_after_seconds(value: Option<&str>) -> Option<Duration> {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_parse_works() {
        assert_eq!(parse_bearer_token(Some("Bearer abc123")), Some("abc123"));
        assert_eq!(parse_bearer_token(Some("abc123")), None);
        assert_eq!(parse_bearer_token(None), None);
    }

    #[test]
    fn authorize_request_with_api_key() {
        let cfg = AuthConfig {
            api_key: Some("k123".into()),
            bearer_token: None,
            allow_unauthenticated: false,
        };
        assert!(authorize_request(Some("k123"), None, &cfg));
        assert!(!authorize_request(Some("wrong"), None, &cfg));
    }

    #[test]
    fn authorize_request_with_bearer() {
        let cfg = AuthConfig {
            api_key: None,
            bearer_token: Some("tok".into()),
            allow_unauthenticated: false,
        };
        assert!(authorize_request(None, Some("Bearer tok"), &cfg));
        assert!(!authorize_request(None, Some("Bearer nope"), &cfg));
    }
}
