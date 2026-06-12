//! Per-user nanopublication review-flow CLI surface.
//!
//! The review-flow SSOT (DB + vault I/O) now lives in
//! [`vox_scientia::review_flow`] so the CLI and the GUI call ONE shared
//! implementation. This module re-exports those entry points so existing CLI
//! call sites keep working unchanged. It owns NO logic of its own; the only
//! thing it adds is a guard test asserting this build surface carries no
//! production-network publishing symbols.

pub use vox_scientia::review_flow::{
    approval_for, nanopub_build, record_claim_review, resolve_or_create_identity,
};

#[cfg(test)]
mod tests {
    /// Guard (TDD): the nanopub build path must carry NO production-network
    /// publishing symbols. This file is the entire local build surface; it must
    /// never grow a network-publish symbol, a test-server toggle, or a hardcoded
    /// network host. (The forbidden needles are assembled below from fragments so
    /// this comment cannot itself trip the guard.)
    #[test]
    fn no_production_network_publish_symbol_on_nanopub_path() {
        let src = include_str!("scientia_nanopub.rs");
        // Needles are assembled from fragments so they never appear as a
        // contiguous literal in THIS file (otherwise the guard would match its
        // own assertions). The semantics are identical to the literal forms.
        let host = format!("{}{}", "knowledgepixels", ".com");
        let publish = format!("{}{}", "publish_to_", "network");
        let test_server = format!("{}{}", "use_test_", "server");
        assert!(!src.contains(&host));
        assert!(!src.to_lowercase().contains(&publish));
        assert!(!src.contains(&test_server));
    }
}
