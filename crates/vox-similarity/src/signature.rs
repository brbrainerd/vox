//! Token shingling and blake3-derived simhash / minhash signatures. Deterministic.

use serde::{Deserialize, Serialize};

/// Split text into lowercase alphanumeric/underscore tokens.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// k-token shingles (sliding windows). Falls back to a single joined token when
/// the token count is below `k`. Empty input yields an empty vec.
pub fn shingle(text: &str, k: usize) -> Vec<String> {
    let toks = tokenize(text);
    if toks.is_empty() {
        return Vec::new();
    }
    if toks.len() < k || k == 0 {
        return vec![toks.join(" ")];
    }
    toks.windows(k).map(|w| w.join(" ")).collect()
}

/// 64-bit SimHash over shingles (blake3 of each shingle → per-bit vote).
pub fn simhash64(shingles: &[String]) -> u64 {
    let mut acc = [0i32; 64];
    for s in shingles {
        let h = blake3::hash(s.as_bytes());
        let v = u64::from_le_bytes(h.as_bytes()[0..8].try_into().unwrap());
        for (i, slot) in acc.iter_mut().enumerate() {
            if (v >> i) & 1 == 1 {
                *slot += 1;
            } else {
                *slot -= 1;
            }
        }
    }
    let mut out = 0u64;
    for (i, &slot) in acc.iter().enumerate() {
        if slot > 0 {
            out |= 1u64 << i;
        }
    }
    out
}

/// MinHash with `num_hashes` independent blake3-seeded hash functions.
pub fn minhash(shingles: &[String], num_hashes: usize) -> Vec<u32> {
    let mut mins = vec![u32::MAX; num_hashes];
    for s in shingles {
        for (i, slot) in mins.iter_mut().enumerate() {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&(i as u32).to_le_bytes());
            hasher.update(s.as_bytes());
            let h = hasher.finalize();
            let v = u32::from_le_bytes(h.as_bytes()[0..4].try_into().unwrap());
            if v < *slot {
                *slot = v;
            }
        }
    }
    mins
}

/// Hamming distance between two 64-bit simhashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Estimated Jaccard similarity from two equal-length minhash vectors.
pub fn jaccard_estimate(a: &[u32], b: &[u32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let eq = a.iter().zip(b).filter(|(x, y)| x == y).count();
    eq as f32 / a.len() as f32
}

/// A deterministic similarity signature for a piece of text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    pub simhash: u64,
    pub minhash: Vec<u32>,
}

impl Signature {
    pub fn from_text(text: &str, k: usize, num_hashes: usize) -> Self {
        let sh = shingle(text, k);
        Signature {
            simhash: simhash64(&sh),
            minhash: minhash(&sh, num_hashes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(tokenize("Foo.bar_baz 42"), vec!["foo", "bar_baz", "42"]);
    }

    #[test]
    fn shingle_makes_k_windows() {
        assert_eq!(shingle("a b c d", 2), vec!["a b", "b c", "c d"]);
    }

    #[test]
    fn identical_text_has_zero_hamming_and_full_jaccard() {
        let a = Signature::from_text("let x = compute(value) + 1", 3, 64);
        let b = Signature::from_text("let x = compute(value) + 1", 3, 64);
        assert_eq!(hamming(a.simhash, b.simhash), 0);
        assert_eq!(jaccard_estimate(&a.minhash, &b.minhash), 1.0);
    }

    #[test]
    fn dissimilar_text_has_low_jaccard() {
        let a = Signature::from_text("the quick brown fox jumps over", 3, 64);
        let b = Signature::from_text("completely unrelated tokens here now please", 3, 64);
        assert!(jaccard_estimate(&a.minhash, &b.minhash) < 0.3);
    }

    #[test]
    fn signatures_are_deterministic() {
        let a = Signature::from_text("repeat me", 2, 32);
        let b = Signature::from_text("repeat me", 2, 32);
        assert_eq!(a, b);
    }
}
