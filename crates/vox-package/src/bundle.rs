//! Content-addressed bundle store for compiled Vox workflow / activity functions.
//!
//! A `Bundle` is the unit of code mobility on the mesh: a `fn_hash` plus the
//! compiled-form bytes plus enough metadata for the runtime to dispatch.
//!
//! This module layers a simple directory store on top of the project-local
//! artifact cache root, without duplicating the SHA3-512 algorithm.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Raw bytes of a compiled bundle. `Arc<Vec<u8>>` because clones cross
/// async tasks (mesh dispatch) and we don't want to allocate per-clone.
pub type ContentBytes = Arc<Vec<u8>>;

/// Stable content-address of a workflow / activity bundle.
///
/// This is the SHA3-512 over the input set: source bytes, vox version,
/// transitive dep hashes. Computed by the HIR lowering pass (P2-T1d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BundleRef {
    /// Raw 64-byte SHA3-512 digest stored as bytes; hex-encode for display/paths.
    #[serde(with = "fn_hash_serde")]
    pub fn_hash: [u8; 64],
}

impl BundleRef {
    /// Hex-encode the 64-byte digest. Used for filesystem keys.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(128);
        for b in &self.fn_hash {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

/// A content-addressed bundle: hash, transitive dep hashes, compiled bytes,
/// and a free-form manifest the runtime uses to dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    /// Self-hash. The caller MUST guarantee `fn_hash` is the SHA3-512 of
    /// the input set used to build `bytes`. The store does not re-derive.
    #[serde(with = "fn_hash_serde")]
    pub fn_hash: [u8; 64],
    /// Other bundles this one depends on. Mesh fetcher walks these
    /// transitively when seeding a fresh node.
    pub deps: Vec<BundleRef>,
    /// Compiled-form bytes — opaque to the store.
    pub bytes: ContentBytes,
    /// Free-form JSON metadata: kind ("workflow" / "activity" / "actor"),
    /// declared name, vox compiler version, capability requirements.
    pub manifest: JsonValue,
}

mod fn_hash_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        let mut hex = String::with_capacity(128);
        for b in bytes {
            hex.push_str(&format!("{b:02x}"));
        }
        s.serialize_str(&hex)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(d)?;
        if s.len() != 128 {
            return Err(serde::de::Error::custom("fn_hash must be 128 hex chars"));
        }
        let mut out = [0u8; 64];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let hex = std::str::from_utf8(chunk)
                .map_err(|e| serde::de::Error::custom(format!("hex utf8: {e}")))?;
            out[i] = u8::from_str_radix(hex, 16)
                .map_err(|e| serde::de::Error::custom(format!("hex parse: {e}")))?;
        }
        Ok(out)
    }
}

/// A bundle store rooted at a directory.
///
/// Layout under `root`:
/// - `bundles/<first-64-hex-chars>/bundle.bin` — compiled bytes
/// - `bundles/<first-64-hex-chars>/manifest.json` — JSON manifest
/// - `bundles/<first-64-hex-chars>/deps.json` — serialised `Vec<BundleRef>`
pub struct BundleStore {
    root: PathBuf,
}

impl BundleStore {
    /// Open (or create) a bundle store under `root`.
    pub fn open(root: PathBuf) -> io::Result<Self> {
        std::fs::create_dir_all(root.join("bundles"))?;
        Ok(Self { root })
    }

    fn bundle_dir(&self, r: &BundleRef) -> PathBuf {
        // Use first 64 hex chars as the directory name (matches ArtifactCache convention).
        let hex = r.to_hex();
        let key = &hex[..64];
        self.root.join("bundles").join(key)
    }

    /// Look up a bundle by reference. `Ok(None)` for cache miss; `Err` for IO.
    pub fn lookup(&self, r: &BundleRef) -> io::Result<Option<Bundle>> {
        let dir = self.bundle_dir(r);
        let bytes_path = dir.join("bundle.bin");
        let manifest_path = dir.join("manifest.json");
        if !bytes_path.exists() || !manifest_path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&bytes_path)?;
        let manifest_str = std::fs::read_to_string(&manifest_path)?;
        let manifest: JsonValue = serde_json::from_str(&manifest_str).unwrap_or(JsonValue::Null);
        let deps_path = dir.join("deps.json");
        let deps: Vec<BundleRef> = if deps_path.exists() {
            serde_json::from_slice(&std::fs::read(&deps_path)?).map_err(io::Error::other)?
        } else {
            vec![]
        };
        Ok(Some(Bundle {
            fn_hash: r.fn_hash,
            deps,
            bytes: Arc::new(bytes),
            manifest,
        }))
    }

    /// Store a bundle by its self-asserted hash. Idempotent.
    pub fn put(&self, bundle: &Bundle) -> io::Result<BundleRef> {
        let r = BundleRef {
            fn_hash: bundle.fn_hash,
        };
        let dir = self.bundle_dir(&r);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("bundle.bin"), bundle.bytes.as_ref())?;
        let manifest_str = serde_json::to_string(&bundle.manifest).map_err(io::Error::other)?;
        std::fs::write(dir.join("manifest.json"), manifest_str)?;
        let deps_json = serde_json::to_vec_pretty(&bundle.deps).map_err(io::Error::other)?;
        std::fs::write(dir.join("deps.json"), deps_json)?;
        Ok(r)
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Bundle {}
    impl Sealed for crate::model_bundle::ModelBundle {}
}

/// Sealed trait for content-addressed bundle metadata.
/// Downstream crates may read hash and kind but cannot introduce new implementations.
pub trait BundleMeta: sealed::Sealed {
    fn content_hash(&self) -> [u8; 64];
    fn kind_label(&self) -> &'static str;
}

impl BundleMeta for Bundle {
    fn content_hash(&self) -> [u8; 64] {
        self.fn_hash
    }

    fn kind_label(&self) -> &'static str {
        match self.manifest.get("kind").and_then(|v| v.as_str()) {
            Some("workflow") => "workflow",
            Some("activity") => "activity",
            Some("actor") => "actor",
            _ => "unknown",
        }
    }
}

impl BundleMeta for crate::model_bundle::ModelBundle {
    fn content_hash(&self) -> [u8; 64] {
        self.bundle_hash
    }

    fn kind_label(&self) -> &'static str {
        "model"
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use std::sync::Arc;
    use serde_json::json;

    fn zero_hash() -> [u8; 64] {
        [0u8; 64]
    }

    fn make_bundle(kind: Option<&str>) -> Bundle {
        Bundle {
            fn_hash: zero_hash(),
            deps: vec![],
            bytes: Arc::new(vec![1, 2, 3]),
            manifest: match kind {
                Some(k) => json!({"kind": k}),
                None => json!({}),
            },
        }
    }

    // Catches: to_hex producing wrong length (not 128 chars) or wrong hex for known bytes.
    #[test]
    fn bundle_ref_to_hex_is_128_chars_and_correct() {
        let mut hash = [0u8; 64];
        hash[0] = 0xde;
        hash[1] = 0xad;
        hash[63] = 0xff;
        let r = BundleRef { fn_hash: hash };
        let hex = r.to_hex();
        assert_eq!(hex.len(), 128, "to_hex must produce 128 hex chars");
        assert!(hex.starts_with("dead"), "first bytes must be 'dead'");
        assert!(hex.ends_with("ff"), "last byte must be 'ff'");
    }

    // Catches: BundleRef fn_hash_serde rejecting a valid 128-char hex string on deserialize.
    #[test]
    fn bundle_ref_serde_round_trips() {
        let r = BundleRef { fn_hash: [0xab; 64] };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\""), "must serialize as a quoted hex string");
        let back: BundleRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.fn_hash, r.fn_hash);
    }

    // Catches: fn_hash_serde accepting a truncated hex string (< 128 chars) instead of erroring.
    #[test]
    fn bundle_ref_short_hex_rejected() {
        let bad_json = "\"abcd\"";
        let result: Result<BundleRef, _> = serde_json::from_str(bad_json);
        assert!(result.is_err(), "short hex must be rejected");
    }

    // Catches: fn_hash_serde accepting a non-hex string without error.
    #[test]
    fn bundle_ref_non_hex_rejected() {
        let long_non_hex: String = "g".repeat(128);
        let bad_json = format!("\"{}\"", long_non_hex);
        let result: Result<BundleRef, _> = serde_json::from_str(&bad_json);
        assert!(result.is_err(), "non-hex must be rejected");
    }

    // Catches: kind_label returning "unknown" even for known manifest kinds like "workflow".
    #[test]
    fn bundle_meta_kind_label_workflow() {
        let b = make_bundle(Some("workflow"));
        assert_eq!(b.kind_label(), "workflow");
    }

    #[test]
    fn bundle_meta_kind_label_activity() {
        let b = make_bundle(Some("activity"));
        assert_eq!(b.kind_label(), "activity");
    }

    // Catches: kind_label not falling back to "unknown" for an absent or unknown manifest kind.
    #[test]
    fn bundle_meta_kind_label_unknown_for_missing_kind() {
        let b = make_bundle(None);
        assert_eq!(b.kind_label(), "unknown");
        let b2 = make_bundle(Some("something_new"));
        assert_eq!(b2.kind_label(), "unknown");
    }

    // Catches: BundleStore::put not being idempotent (second put fails or corrupts data).
    #[test]
    fn bundle_store_put_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = BundleStore::open(tmp.path().to_path_buf()).unwrap();
        let b = make_bundle(Some("actor"));
        let r1 = store.put(&b).unwrap();
        let r2 = store.put(&b).unwrap(); // second put
        assert_eq!(r1.fn_hash, r2.fn_hash);
        let loaded = store.lookup(&r1).unwrap().expect("must be present after two puts");
        assert_eq!(loaded.bytes.as_ref(), b.bytes.as_ref());
    }

    // Catches: BundleStore::lookup returning None immediately after a put (no flush/sync issue).
    #[test]
    fn bundle_store_lookup_after_put() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = BundleStore::open(tmp.path().to_path_buf()).unwrap();
        let b = make_bundle(Some("workflow"));
        let r = store.put(&b).unwrap();
        let found = store.lookup(&r).unwrap();
        assert!(found.is_some(), "bundle must be findable after put");
        let found = found.unwrap();
        assert_eq!(found.fn_hash, b.fn_hash);
    }

    // Catches: BundleStore::lookup returning Some for a hash that was never stored.
    #[test]
    fn bundle_store_lookup_miss_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = BundleStore::open(tmp.path().to_path_buf()).unwrap();
        let r = BundleRef { fn_hash: [0xcc; 64] };
        assert!(store.lookup(&r).unwrap().is_none(), "miss must return None");
    }
}
