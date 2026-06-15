//! Per-user nanopublication review-flow CLI surface.
//!
//! The review-flow SSOT (DB + vault I/O) now lives in
//! [`vox_scientia::review_flow`] so the CLI and the GUI call ONE shared
//! implementation. This module re-exports those entry points so existing CLI
//! call sites keep working unchanged. It owns NO logic of its own; the only
//! thing it adds is a guard test asserting this build surface carries no
//! production-network publishing symbols.

pub use vox_scientia::review_flow::{
    approval_for, nanopub_build, nanopub_publish_test_server, record_claim_review,
    resolve_or_create_identity,
};

#[cfg(test)]
mod tests {
    /// Guard (TDD): the nanopub CLI surface must carry NO production-network
    /// publishing symbols. This file re-exports the build + publish-test-server
    /// paths from `vox_scientia::review_flow`; it must never grow a hardcoded
    /// production-network host or a bypass of the dual-gate contract.
    /// (Forbidden needles assembled from fragments so the guard cannot trip on
    /// its own assertion text.)
    #[test]
    fn no_production_network_publish_symbol_on_nanopub_path() {
        let src = include_str!("scientia_nanopub.rs");
        // Needles assembled from fragments — same semantics as the literal forms.
        let host = format!("{}{}", "knowledgepixels", ".com");
        let publish_net = format!("{}{}", "publish_to_", "network");
        let publish_prod = format!("{}{}", "publish_to_", "production");
        let use_test = format!("{}{}", "use_test_", "server");
        assert!(!src.contains(&host));
        assert!(!src.to_lowercase().contains(&publish_net));
        assert!(!src.to_lowercase().contains(&publish_prod));
        assert!(!src.contains(&use_test));
        // `nanopub_publish_test_server` IS allowed (it is the sanctioned symbol).
        // Only `publish_to_` + `network` / `publish_to_` + `production` are forbidden.
    }
}
