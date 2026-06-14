//! Stable repository identity (blake3).

use std::path::Path;

/// 16-character lowercase hex id: blake3(origin_url + NUL + canonical root), or root-only if no origin.
pub fn compute_repository_id(canonical_root: &Path, origin_url: Option<&str>) -> String {
    let mut h = blake3::Hasher::new();
    if let Some(url) = origin_url {
        let u = url.trim();
        if !u.is_empty() {
            h.update(u.as_bytes());
            h.update(&[0]);
        }
    }
    h.update(canonical_root.to_string_lossy().as_bytes());
    let out = h.finalize();
    let b = out.as_bytes();
    (0..8).map(|i| format!("{:02x}", b[i])).collect()
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::path::Path;

    #[test]
    fn compute_repository_id_returns_16_hex_chars() {
        let id = compute_repository_id(Path::new("/some/root"), None);
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn compute_repository_id_is_deterministic() {
        let p = Path::new("/some/root");
        let a = compute_repository_id(p, Some("https://github.com/example/repo"));
        let b = compute_repository_id(p, Some("https://github.com/example/repo"));
        assert_eq!(a, b);
    }

    #[test]
    fn compute_repository_id_differs_by_origin() {
        let p = Path::new("/some/root");
        let with_origin = compute_repository_id(p, Some("https://github.com/example/repo"));
        let without_origin = compute_repository_id(p, None);
        assert_ne!(with_origin, without_origin);
    }

    #[test]
    fn compute_repository_id_empty_origin_treated_as_none() {
        let p = Path::new("/some/root");
        let with_empty = compute_repository_id(p, Some(""));
        let with_whitespace = compute_repository_id(p, Some("   "));
        let no_origin = compute_repository_id(p, None);
        // empty/whitespace origin should be skipped, producing same hash as no origin
        assert_eq!(with_empty, no_origin);
        assert_eq!(with_whitespace, no_origin);
    }
}
