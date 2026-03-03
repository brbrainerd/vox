//! Standard library builtins available to compiled Vox programs.
//!
//! Three-tier hashing strategy:
//! - `vox_hash_fast`   → XXH3-128 (20-80 GB/s, non-cryptographic, 32-char hex)
//! - `vox_hash_secure` → BLAKE3   (6-12 GB/s, cryptographic, 64-char hex)
//! - `vox_uuid`        → monotonic unique ID (timestamp + atomic counter)
//! - `vox_now_ms`      → current UNIX time in milliseconds

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fast, non-cryptographic hash using XXH3-128 (128-bit output).
///
/// Use for: HashMap keys, cache keys, dedup within a process, activity IDs
/// in hot workflow paths where you control the input (no adversarial keys).
///
/// Output: 32-char lowercase hex string (128-bit → 2 × u64 in hex).
/// Deterministic for the same input within a process; also cross-machine
/// deterministic (XXH3-128 is unkeyed / uses a fixed internal secret).
///
/// ⚠ NOT cryptographic — do not use for stored provenance hashes.
pub fn vox_hash_fast(input: &str) -> String {
    use twox_hash::XxHash3_128;
    let h: u128 = XxHash3_128::oneshot(input.as_bytes());
    let lo = h as u64;
    let hi = (h >> 64) as u64;
    format!("{:016x}{:016x}", hi, lo)
}

/// Cryptographic hash using BLAKE3 (256-bit output).
///
/// Use for: `input_hash` provenance stored in DB, content-addressable IDs
/// shared across machines / process lifetimes, data integrity verification.
///
/// Output: 64-char lowercase hex string (256-bit).
/// Fully deterministic, cross-machine stable, collision probability ≈ 2^-128.
///
/// ✅ Cryptographically secure. Safe to store permanently.
pub fn vox_hash_secure(input: &str) -> String {
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex().to_string()
}

/// Generate a unique identifier.
///
/// Combines nanosecond-precision UNIX timestamp with a monotonic atomic counter
/// to guarantee uniqueness even within the same nanosecond (parallel workflow steps).
///
/// Format: `vox-{nanos_hex}-{counter_hex}`
/// Example: `vox-17a8c3f2d8b00000-0000000000000001`
pub fn vox_uuid() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("vox-{:016x}-{:016x}", nanos, count)
}

/// Current UNIX time in milliseconds.
pub fn vox_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_hash_is_deterministic() {
        assert_eq!(vox_hash_fast("hello world"), vox_hash_fast("hello world"));
        assert_eq!(vox_hash_fast("hello world").len(), 32);
    }

    #[test]
    fn fast_hash_differs_for_different_inputs() {
        assert_ne!(vox_hash_fast("foo"), vox_hash_fast("bar"));
    }

    #[test]
    fn fast_hash_differs_for_similar_inputs() {
        // Avalanche effect: single char change → totally different hash
        assert_ne!(vox_hash_fast("gain"), vox_hash_fast("Gain"));
        assert_ne!(vox_hash_fast("loss"), vox_hash_fast("los"));
    }

    #[test]
    fn secure_hash_is_deterministic() {
        assert_eq!(vox_hash_secure("hello world"), vox_hash_secure("hello world"));
        assert_eq!(vox_hash_secure("hello world").len(), 64);
    }

    #[test]
    fn secure_hash_known_vector() {
        // BLAKE3 test vector from official spec
        let h = vox_hash_secure("");
        assert_eq!(
            h,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn secure_hash_differs_from_fast_hash() {
        let input = "test input";
        assert_ne!(vox_hash_fast(input), vox_hash_secure(input));
    }

    #[test]
    fn uuid_is_unique() {
        let u1 = vox_uuid();
        let u2 = vox_uuid();
        assert_ne!(u1, u2);
        assert!(u1.starts_with("vox-"));
        // Format: vox-{16 hex}-{16 hex}
        let parts: Vec<&str> = u1.splitn(3, '-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1].len(), 16);
        assert_eq!(parts[2].len(), 16);
    }

    #[test]
    fn uuid_counter_is_monotonic() {
        let ids: Vec<String> = (0..100).map(|_| vox_uuid()).collect();
        // All must be unique
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), 100);
    }

    #[test]
    fn now_ms_is_reasonable() {
        let ts = vox_now_ms();
        // Must be after 2025-01-01T00:00:00Z (1735689600000 ms)
        assert!(ts > 1_735_689_600_000, "timestamp too old: {}", ts);
    }
}
