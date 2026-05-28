//! Cheap lowercase-hex identifiers from wall-clock nanos (tracing/debug; not cryptographic).

/// Lowercase hex of nanoseconds since UNIX epoch from [`std::time::SystemTime::now`].
#[must_use]
pub fn simple_hex_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:x}")
}
