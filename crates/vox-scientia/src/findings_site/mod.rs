//! SCIENTIA Phase G — Vox-native publication reading surface.
//!
//! Pure HTML page builder for `/findings/<trusty-uri>` pages. Consumers
//! (the docs SSG or the dashboard) stage a [`FindingPage`] from
//! `publication_manifests` rows and call [`render_finding_page`] to get
//! a complete `<!doctype html>` document.
//!
//! Surfaces:
//!
//! - Highwire `citation_*` meta tags (Google Scholar pickup).
//! - Version history table with per-version DOIs (Living-Review semantics
//!   from Finalization Phase 3).
//! - Retraction banner emitted ABOVE the body when present, per COPE
//!   practice.
//! - Verified-claim sidebar linked to Trusty URIs.
//! - Reply-thread block (inline, not appendix — IMC measurement-paper
//!   convention).
//!
//! No DB queries, no network I/O, no JavaScript dependencies — the page is
//! server-rendered HTML; consumers can sprinkle JS at the SSG layer if they
//! want.

pub mod meta;
pub mod page;
pub mod render;

pub use meta::{HighwireMetaTags, build_highwire_meta_tags};
pub use page::{
    FindingPage, ReplyEntry, RetractionNotice, VersionHistoryEntry, VerifiedClaim,
};
pub use render::render_finding_page;
