//! Normalized errors for scholarly repository adapters (maps cleanly to DB `error_class`).
//!
//! ## Persisted `error_class` strings
//!
//! Adapter errors use [`ScholarlyError::error_class`]. The external job ledger may also write
//! `preflight` for operator gates (digest mismatch, missing manifest, missing dual approval) — that
//! value is **not** produced by this enum.
//!
//! | `error_class` | Source |
//! |---------------|--------|
//! | `disabled` | `VOX_SCHOLARLY_DISABLE*` |
//! | `config` | Missing credentials, bad env, unsupported adapter |
//! | `auth` | HTTP 401 / 403 (status stored; see [`scholarly_http_status_code`]) |
//! | `rate_limit` | HTTP 429 or explicit limit |
//! | `transient` | Timeouts, connection errors, HTTP 5xx (`source_status` when from HTTP) |
//! | `fatal` | Parse / validation-style failures |
//! | (derived from status) | [`ScholarlyError::Http`] — 429 → `rate_limit`, 5xx → `transient`, else `fatal` |

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

fn truncate_http_body(body: &str) -> String {
    if body.len() > 2048 {
        format!("{}…", body.chars().take(2040).collect::<String>())
    } else {
        body.to_string()
    }
}

/// Map a failed HTTP response to [`ScholarlyError`] (shared by Zenodo, OpenReview, etc.).
#[must_use]
pub(crate) fn classify_scholarly_http(status: u16, body: &str) -> ScholarlyError {
    let msg = truncate_http_body(body);
    match status {
        401 => ScholarlyError::Auth {
            status: 401,
            message: msg,
        },
        403 => ScholarlyError::Auth {
            status: 403,
            message: msg,
        },
        429 => ScholarlyError::RateLimited {
            message: msg,
            retry_after_secs: None,
        },
        s if (500..600).contains(&s) => ScholarlyError::Transient {
            message: format!("HTTP {s}: {msg}"),
            source_status: Some(s),
        },
        s => ScholarlyError::Http {
            status: s,
            message: msg,
        },
    }
}

/// Structured adapter failure; use [`ScholarlyError::error_class`] for persistence / retries.
#[derive(Debug, Clone)]
pub enum ScholarlyError {
    /// Operator or env disabled this path (`VOX_SCHOLARLY_DISABLE*`).
    Disabled { reason: String },
    /// Missing token, bad env, or unsupported adapter name.
    Config { message: String },
    /// Remote rejected credentials (HTTP status preserved when from REST adapters).
    Auth { status: u16, message: String },
    /// HTTP 429 or explicit rate limit body.
    RateLimited {
        message: String,
        retry_after_secs: Option<u64>,
    },
    /// Timeouts and likely-transient failures; `source_status` is set for mapped HTTP 5xx.
    Transient {
        message: String,
        source_status: Option<u16>,
    },
    /// Non-retryable client/validation errors.
    Fatal { code: String, message: String },
    /// HTTP response with status (body captured for diagnostics).
    Http { status: u16, message: String },
}

impl ScholarlyError {
    #[must_use]
    pub fn error_class(&self) -> &'static str {
        match self {
            ScholarlyError::Disabled { .. } => "disabled",
            ScholarlyError::Config { .. } => "config",
            ScholarlyError::Auth { .. } => "auth",
            ScholarlyError::RateLimited { .. } => "rate_limit",
            ScholarlyError::Transient { .. } => "transient",
            ScholarlyError::Fatal { .. } => "fatal",
            ScholarlyError::Http { status, .. } => {
                if *status == 429 {
                    "rate_limit"
                } else if (500..600).contains(status) {
                    "transient"
                } else {
                    "fatal"
                }
            }
        }
    }

    #[must_use]
    pub fn retryable(&self) -> bool {
        match self {
            ScholarlyError::RateLimited { .. } | ScholarlyError::Transient { .. } => true,
            ScholarlyError::Http { status, .. } => *status == 429 || (500..600).contains(status),
            _ => false,
        }
    }
}

impl fmt::Display for ScholarlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScholarlyError::Disabled { reason } => write!(f, "scholarly disabled: {reason}"),
            ScholarlyError::Config { message } => write!(f, "scholarly config: {message}"),
            ScholarlyError::Auth { status, message } => {
                write!(f, "scholarly auth HTTP {status}: {message}")
            }
            ScholarlyError::RateLimited {
                message,
                retry_after_secs,
            } => {
                if let Some(secs) = retry_after_secs {
                    write!(
                        f,
                        "scholarly rate limited ({message}), retry_after_secs={secs}"
                    )
                } else {
                    write!(f, "scholarly rate limited: {message}")
                }
            }
            ScholarlyError::Transient { message, .. } => {
                write!(f, "scholarly transient: {message}")
            }
            ScholarlyError::Fatal { code, message } => {
                write!(f, "scholarly fatal [{code}]: {message}")
            }
            ScholarlyError::Http { status, message } => {
                write!(f, "scholarly http {status}: {message}")
            }
        }
    }
}

impl std::error::Error for ScholarlyError {}

#[must_use]
pub fn scholarly_http_status_code(e: &ScholarlyError) -> Option<i32> {
    match e {
        ScholarlyError::Http { status, .. } => Some(i32::from(*status)),
        ScholarlyError::Auth { status, .. } => Some(i32::from(*status)),
        ScholarlyError::RateLimited { .. } => Some(429),
        ScholarlyError::Transient {
            source_status: Some(s),
            ..
        } => Some(i32::from(*s)),
        _ => None,
    }
}

/// Earliest suggested wall time (ms since UNIX epoch) for a retry when [`ScholarlyError::retryable`] is true.
#[must_use]
pub fn scholarly_retry_not_before_ms(e: &ScholarlyError) -> Option<i64> {
    if !e.retryable() {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let delay_ms: i64 = match e {
        ScholarlyError::RateLimited {
            retry_after_secs: Some(s),
            ..
        } => (*s as i64).saturating_mul(1000),
        ScholarlyError::RateLimited { .. } => 60_000,
        ScholarlyError::Http { status, .. } if *status == 429 => 60_000,
        ScholarlyError::Http { status, .. } if (500..600).contains(status) => 30_000,
        ScholarlyError::Transient { .. } => 30_000,
        _ => 30_000,
    };
    Some(now.saturating_add(delay_ms))
}

impl From<reqwest::Error> for ScholarlyError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() || value.is_connect() {
            return ScholarlyError::Transient {
                message: value.to_string(),
                source_status: None,
            };
        }
        ScholarlyError::Transient {
            message: value.to_string(),
            source_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_preserved_for_auth_and_5xx_transient() {
        let auth = classify_scholarly_http(403, "nope");
        assert_eq!(scholarly_http_status_code(&auth), Some(403));
        let tx = classify_scholarly_http(503, "unavailable");
        assert_eq!(scholarly_http_status_code(&tx), Some(503));
        assert_eq!(tx.error_class(), "transient");
        let rl = classify_scholarly_http(429, "slow");
        assert_eq!(scholarly_http_status_code(&rl), Some(429));
    }
}
