//! Env passthrough + fingerprint-safe hashing.
//!
//! Cargo fingerprints depend on many env vars (`RUSTFLAGS`, `CARGO_INCREMENTAL`,
//! `RUSTC_WRAPPER`, feature vars, ...). The shim therefore passes the caller's
//! FULL environment through to the child cargo, minus a small denylist of
//! volatile vars that never affect a build. The coalescing key hashes the same
//! filtered set, so two invocations only coalesce when they would truly produce
//! the same build.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Volatile vars that do not affect cargo fingerprints; excluded from the build
/// env replay.
const DENYLIST: &[&str] = &[
    "PROMPT",
    "TERM",
    "TERM_SESSION_ID",
    "WT_SESSION",
    "PWD",
    "OLDPWD",
];

/// Env vars that genuinely change a cargo build's identity. The *coalescing key*
/// is computed from ONLY these (an allowlist), not from the full environment.
///
/// Rationale (review finding #1): every agent/IDE injects its own volatile vars
/// (`TEMP`, `WT_SESSION`, `VSCODE_*`, `_`, ...). Hashing the full env would make
/// two otherwise-identical `cargo test` runs from different agents hash
/// differently, so `would_coalesce` would read near-zero and bias the daemon
/// go/no-go decision. A denylist cannot enumerate every volatile var; an
/// allowlist of build-fingerprint vars is correct by construction. Prefixes
/// (ending in `*`) match any var starting with that stem.
const FINGERPRINT_ALLOW: &[&str] = &[
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTDOCFLAGS",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_BOOTSTRAP",
    "CARGO_INCREMENTAL",
    "CARGO_TARGET_DIR",
    "SOURCE_DATE_EPOCH",
    "CC",
    "CXX",
    "CFLAGS",
    "CXXFLAGS",
    "CARGO_BUILD_",   // prefix: CARGO_BUILD_JOBS, _TARGET, _RUSTFLAGS, ...
    "CARGO_FEATURE_", // prefix
    "CARGO_CFG_",     // prefix
];

fn is_fingerprint_var(key: &str) -> bool {
    FINGERPRINT_ALLOW.iter().any(|a| {
        if a.ends_with('_') {
            key.starts_with(a)
        } else {
            key == *a
        }
    })
}

/// The caller's env minus the volatile denylist, sorted for determinism.
pub fn passthrough_env(
    raw: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    raw.into_iter()
        .filter(|(k, _)| !DENYLIST.contains(&k.as_str()))
        .collect()
}

fn hash64<T: Hash>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Stable hash of build-fingerprint vars ONLY (the coalescing identity).
///
/// Accepts the full passthrough env but hashes only `FINGERPRINT_ALLOW` keys, so
/// volatile per-agent vars never perturb the coalescing key. See the
/// `FINGERPRINT_ALLOW` doc for why an allowlist (not the denylist) defines build
/// identity.
pub fn env_hash(env: &BTreeMap<String, String>) -> u64 {
    let pairs: Vec<(&String, &String)> =
        env.iter().filter(|(k, _)| is_fingerprint_var(k)).collect();
    hash64(&pairs)
}

/// Stable hash of argv.
pub fn argv_hash(argv: &[String]) -> u64 {
    hash64(&argv.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_dropped() {
        let env = passthrough_env([
            ("RUSTFLAGS".into(), "-Cdebuginfo=0".into()),
            ("PROMPT".into(), "$P$G".into()),
        ]);
        assert!(env.contains_key("RUSTFLAGS"));
        assert!(!env.contains_key("PROMPT"));
    }

    #[test]
    fn rustflags_change_changes_hash() {
        let a = passthrough_env([("RUSTFLAGS".into(), "-Cdebuginfo=0".into())]);
        let b = passthrough_env([("RUSTFLAGS".into(), "-Cdebuginfo=2".into())]);
        assert_ne!(env_hash(&a), env_hash(&b));
    }

    #[test]
    fn volatile_change_does_not_change_hash() {
        let a = passthrough_env([
            ("RUSTFLAGS".into(), "x".into()),
            ("PROMPT".into(), "1".into()),
        ]);
        let b = passthrough_env([
            ("RUSTFLAGS".into(), "x".into()),
            ("PROMPT".into(), "2".into()),
        ]);
        assert_eq!(env_hash(&a), env_hash(&b));
    }

    #[test]
    fn non_fingerprint_var_does_not_change_coalescing_hash() {
        // TEMP is neither denylisted nor a fingerprint var; it must not affect
        // the coalescing identity (review finding #1).
        let a = passthrough_env([
            ("RUSTFLAGS".into(), "x".into()),
            ("TEMP".into(), "C:/Temp/aaa".into()),
        ]);
        let b = passthrough_env([
            ("RUSTFLAGS".into(), "x".into()),
            ("TEMP".into(), "C:/Temp/bbb".into()),
        ]);
        assert!(a.contains_key("TEMP")); // still passed through to cargo
        assert_eq!(env_hash(&a), env_hash(&b)); // but not in the coalescing key
    }

    #[test]
    fn cargo_build_prefix_is_fingerprint() {
        let a = passthrough_env([("CARGO_BUILD_JOBS".into(), "4".into())]);
        let b = passthrough_env([("CARGO_BUILD_JOBS".into(), "8".into())]);
        assert_ne!(env_hash(&a), env_hash(&b));
    }

    #[test]
    fn argv_hash_sensitive() {
        assert_ne!(
            argv_hash(&["test".into(), "-p".into(), "a".into()]),
            argv_hash(&["test".into(), "-p".into(), "b".into()])
        );
    }
}
