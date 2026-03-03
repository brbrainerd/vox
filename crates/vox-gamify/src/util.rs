//! Shared utility functions for the gamification crate.

/// Current Unix timestamp in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Default user ID used when no authenticated user is available.
pub const DEFAULT_USER_ID: &str = "default";
