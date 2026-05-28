//! Named timeout constants — SSOT for [`std::time::Duration`] literals across the workspace.
//!
//! `vox-drift-check`'s `drift/timeout-literal` rule flags inline `Duration::from_secs(N)` /
//! `Duration::from_millis(N)` calls with common values; use a const from this module instead.
//!
//! The constants are grouped by intent. Many call sites have no strong semantic meaning —
//! they just need *some* shared timeout — so `D_5S`, `D_30S`, etc. are provided as
//! intent-free values. Prefer the semantic aliases (`HTTP_REQUEST`, `RETRY_BACKOFF_INITIAL`,
//! …) when one fits; the value-based constants are an escape hatch for cases where a more
//! specific name would be misleading.

use std::time::Duration;

// ──────────────────────────────── value-based constants ────────────────────────────────
//
// Plain "duration of X" constants. Use these only when the call site genuinely lacks a
// stronger semantic meaning, or when defining a more specific alias below.

pub const D_100MS: Duration = Duration::from_millis(100);
pub const D_250MS: Duration = Duration::from_millis(250);
pub const D_500MS: Duration = Duration::from_millis(500);
pub const D_1S: Duration = Duration::from_secs(1);
pub const D_5S: Duration = Duration::from_secs(5);
pub const D_10S: Duration = Duration::from_secs(10);
pub const D_15S: Duration = Duration::from_secs(15);
pub const D_30S: Duration = Duration::from_secs(30);
pub const D_60S: Duration = Duration::from_secs(60);
pub const D_120S: Duration = Duration::from_secs(120);
pub const D_300S: Duration = Duration::from_secs(300);
pub const D_600S: Duration = Duration::from_secs(600);
pub const D_1800S: Duration = Duration::from_secs(1800);
pub const D_3600S: Duration = Duration::from_secs(3600);

// ──────────────────────────────── HTTP / network ────────────────────────────────

/// Outbound HTTP connect timeout (matches `vox_http_client::CONNECT_TIMEOUT`).
pub const HTTP_CONNECT: Duration = D_15S;
/// Short HTTP request ceiling (interactive UI calls).
pub const HTTP_REQUEST_SHORT: Duration = D_10S;
/// Standard HTTP request ceiling.
pub const HTTP_REQUEST: Duration = D_30S;
/// Long HTTP request ceiling (large fetches, uploads).
pub const HTTP_REQUEST_LONG: Duration = D_60S;
/// Bulk / batch HTTP operation ceiling.
pub const HTTP_REQUEST_BULK: Duration = D_120S;

// ──────────────────────────────── polling / scheduling ────────────────────────────────

/// Fast loop / scheduler tick.
pub const POLL_TICK_FAST: Duration = D_100MS;
/// Medium poll for periodic state refresh.
pub const POLL_INTERVAL_FAST: Duration = D_5S;
/// Standard background poll interval.
pub const POLL_INTERVAL_STANDARD: Duration = D_30S;

// ──────────────────────────────── retry / backoff ────────────────────────────────

/// Initial exponential-backoff delay between retries.
pub const RETRY_BACKOFF_INITIAL: Duration = D_500MS;
/// Cap on per-attempt exponential backoff.
pub const RETRY_BACKOFF_CAP: Duration = D_30S;

// ──────────────────────────────── operation budgets ────────────────────────────────

/// Short single operation budget.
pub const OP_SHORT: Duration = D_5S;
/// Standard single operation budget.
pub const OP_STANDARD: Duration = D_60S;
/// Long-running operation budget.
pub const OP_LONG: Duration = D_300S;

// ──────────────────────────────── lease / retention ────────────────────────────────

/// One-hour lease (heartbeats, schedule windows).
pub const LEASE_HOUR: Duration = D_3600S;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_aliases_match_values() {
        assert_eq!(HTTP_CONNECT, Duration::from_secs(15));
        assert_eq!(HTTP_REQUEST, Duration::from_secs(30));
        assert_eq!(POLL_TICK_FAST, Duration::from_millis(100));
        assert_eq!(LEASE_HOUR, Duration::from_secs(3600));
    }
}
