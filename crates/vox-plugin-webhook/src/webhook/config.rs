//! Operational configuration defaults for the webhook gateway.
//!
//! Each tunable follows the pure-resolver idiom: a `resolve_*(Option<&str>)`
//! function that contains all the parsing/fallback logic (and is unit-testable
//! without touching global process env), plus a thin `*_from_env` wrapper that
//! reads the corresponding `VOX_WEBHOOK_*` variable and delegates.
//!
//! All defaults equal the historical hard-coded literals, so behavior is
//! unchanged until an operator sets an override.

// ---------------------------------------------------------------------------
// Defaults (single source of truth)
// ---------------------------------------------------------------------------

/// Default bind address for the inbound HTTP listener.
pub const DEFAULT_WEBHOOK_BIND: &str = "0.0.0.0:9080";

/// Default maximum outbound delivery attempts.
pub const DEFAULT_WEBHOOK_RETRY_MAX: u32 = 3;

/// Default base backoff (milliseconds) between outbound retry attempts.
pub const DEFAULT_WEBHOOK_RETRY_BACKOFF_MS: u64 = 500;

/// Default inbound broadcast channel capacity.
pub const DEFAULT_WEBHOOK_CHANNEL_CAP: usize = 256;

// ---------------------------------------------------------------------------
// Pure resolvers
// ---------------------------------------------------------------------------

/// Resolve the bind address: a non-empty (trimmed) override, else the default.
pub fn resolve_bind_addr(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => DEFAULT_WEBHOOK_BIND.to_string(),
    }
}

/// Resolve the max retry count: a parseable override, else the default.
pub fn resolve_retry_max(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_WEBHOOK_RETRY_MAX)
}

/// Resolve the retry backoff in ms: a parseable override, else the default.
pub fn resolve_retry_backoff_ms(raw: Option<&str>) -> u64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_WEBHOOK_RETRY_BACKOFF_MS)
}

/// Resolve the broadcast channel capacity: a parseable, non-zero override, else
/// the default. A parsed `0` is rejected because a 0-capacity broadcast channel
/// is invalid (would panic at construction).
pub fn resolve_channel_cap(raw: Option<&str>) -> usize {
    match raw.and_then(|s| s.trim().parse::<usize>().ok()) {
        Some(n) if n > 0 => n,
        _ => DEFAULT_WEBHOOK_CHANNEL_CAP,
    }
}

// ---------------------------------------------------------------------------
// Thin env wrappers
// ---------------------------------------------------------------------------

/// Resolve the bind address from `VOX_WEBHOOK_ADDR` (the established env name).
pub fn bind_addr_from_env() -> String {
    resolve_bind_addr(std::env::var("VOX_WEBHOOK_ADDR").ok().as_deref())
}

/// Resolve the max retry count from `VOX_WEBHOOK_RETRY_MAX`.
pub fn retry_max_from_env() -> u32 {
    resolve_retry_max(std::env::var("VOX_WEBHOOK_RETRY_MAX").ok().as_deref())
}

/// Resolve the retry backoff (ms) from `VOX_WEBHOOK_RETRY_BACKOFF_MS`.
pub fn retry_backoff_ms_from_env() -> u64 {
    resolve_retry_backoff_ms(
        std::env::var("VOX_WEBHOOK_RETRY_BACKOFF_MS")
            .ok()
            .as_deref(),
    )
}

/// Resolve the broadcast channel capacity from `VOX_WEBHOOK_CHANNEL_CAP`.
pub fn channel_cap_from_env() -> usize {
    resolve_channel_cap(std::env::var("VOX_WEBHOOK_CHANNEL_CAP").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- bind addr ---------------------------------------------------------
    #[test]
    fn bind_override_applies() {
        assert_eq!(resolve_bind_addr(Some("127.0.0.1:1234")), "127.0.0.1:1234");
    }

    #[test]
    fn bind_trims_override() {
        assert_eq!(
            resolve_bind_addr(Some("  127.0.0.1:1234  ")),
            "127.0.0.1:1234"
        );
    }

    #[test]
    fn bind_missing_keeps_default() {
        assert_eq!(resolve_bind_addr(None), DEFAULT_WEBHOOK_BIND);
    }

    #[test]
    fn bind_empty_keeps_default() {
        assert_eq!(resolve_bind_addr(Some("   ")), DEFAULT_WEBHOOK_BIND);
    }

    // --- retry max ---------------------------------------------------------
    #[test]
    fn retry_max_override_applies() {
        assert_eq!(resolve_retry_max(Some("7")), 7);
    }

    #[test]
    fn retry_max_missing_keeps_default() {
        assert_eq!(resolve_retry_max(None), DEFAULT_WEBHOOK_RETRY_MAX);
    }

    #[test]
    fn retry_max_unparseable_keeps_default() {
        assert_eq!(resolve_retry_max(Some("nope")), DEFAULT_WEBHOOK_RETRY_MAX);
    }

    // --- retry backoff -----------------------------------------------------
    #[test]
    fn retry_backoff_override_applies() {
        assert_eq!(resolve_retry_backoff_ms(Some("1500")), 1500);
    }

    #[test]
    fn retry_backoff_missing_keeps_default() {
        assert_eq!(
            resolve_retry_backoff_ms(None),
            DEFAULT_WEBHOOK_RETRY_BACKOFF_MS
        );
    }

    #[test]
    fn retry_backoff_unparseable_keeps_default() {
        assert_eq!(
            resolve_retry_backoff_ms(Some("soon")),
            DEFAULT_WEBHOOK_RETRY_BACKOFF_MS
        );
    }

    // --- channel cap -------------------------------------------------------
    #[test]
    fn channel_cap_override_applies() {
        assert_eq!(resolve_channel_cap(Some("512")), 512);
    }

    #[test]
    fn channel_cap_missing_keeps_default() {
        assert_eq!(resolve_channel_cap(None), DEFAULT_WEBHOOK_CHANNEL_CAP);
    }

    #[test]
    fn channel_cap_unparseable_keeps_default() {
        assert_eq!(
            resolve_channel_cap(Some("lots")),
            DEFAULT_WEBHOOK_CHANNEL_CAP
        );
    }

    #[test]
    fn channel_cap_zero_keeps_default() {
        assert_eq!(resolve_channel_cap(Some("0")), DEFAULT_WEBHOOK_CHANNEL_CAP);
    }
}
