//! P2-T4: helpers for deciding whether to inline a bundle or require a
//! `bundle_request` round-trip, and for recovering a `Bundle` from inline bytes.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use vox_package::bundle::{Bundle, BundleRef, BundleStore};

/// Bundles at or below this threshold are inlined as base64 in the envelope.
pub const INLINE_BUNDLE_BYTE_LIMIT: usize = 1024 * 1024; // 1 MiB

/// Decide whether to inline bundle bytes in the dispatch envelope.
///
/// Returns `(bundle_ref, Some(base64))` when the bundle is small enough to
/// inline, or `(bundle_ref, None)` when the receiver must do a `bundle_request`
/// round-trip.
pub fn ship_decision(bundle: &Bundle) -> (BundleRef, Option<String>) {
    let r = BundleRef {
        fn_hash: bundle.fn_hash,
    };
    if bundle.bytes.len() <= INLINE_BUNDLE_BYTE_LIMIT {
        let b64 = B64.encode(bundle.bytes.as_ref());
        (r, Some(b64))
    } else {
        (r, None)
    }
}

/// Attempt to resolve a bundle from the local store.
///
/// Returns `Ok(None)` on a cache miss; callers must then emit a `bundle_request`.
pub fn resolve_local(store: &BundleStore, r: &BundleRef) -> std::io::Result<Option<Bundle>> {
    store.lookup(r)
}

/// Reconstruct a `Bundle` from inline base64 bytes carried on the envelope.
pub fn decode_inline(
    r: &BundleRef,
    b64: &str,
    deps: Vec<BundleRef>,
    manifest: serde_json::Value,
) -> Result<Bundle, base64::DecodeError> {
    let bytes = B64.decode(b64)?;
    Ok(Bundle {
        fn_hash: r.fn_hash,
        deps,
        bytes: std::sync::Arc::new(bytes),
        manifest,
    })
}
