//! Typed inputs to the page renderer.

use serde::{Deserialize, Serialize};

/// A complete render-input for one canonical `/findings/<trusty-uri>` page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FindingPage {
    /// Working title. Used in `<title>` and `<h1>`.
    pub title: String,
    /// Authors, in display order. Each may carry an ORCID URL.
    pub authors: Vec<Author>,
    /// Abstract paragraph(s). Plain text; renderer escapes HTML entities.
    pub abstract_text: String,
    /// The publication's canonical content as rendered HTML. Producer is
    /// responsible for sanitization — the renderer does NOT escape this.
    pub body_html: String,
    /// Trusty URI fingerprint for this version (canonical URL component).
    pub trusty_uri: String,
    /// DOI of *this version*. The version history table also lists DOIs of
    /// earlier versions.
    pub doi: Option<String>,
    /// Per-version history. Most recent first; the entry whose `trusty_uri`
    /// matches the page's `trusty_uri` is highlighted.
    pub versions: Vec<VersionHistoryEntry>,
    /// Verified atomic claims surfaced as a sidebar. Each row is one
    /// Trusty-URI-bound claim.
    pub verified_claims: Vec<VerifiedClaim>,
    /// Right-of-reply thread, inline. Producer orders by submitted_at_ms.
    pub replies: Vec<ReplyEntry>,
    /// Retraction notice. When present, a prominent banner is emitted at
    /// the top of the body and the page's `<meta>` tags include
    /// `citation_retracted`.
    pub retraction: Option<RetractionNotice>,
    /// Iso-8601 date `YYYY-MM-DD` for `citation_publication_date`.
    pub published_at_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Author {
    pub name: String,
    pub orcid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionHistoryEntry {
    pub trusty_uri: String,
    pub doi: Option<String>,
    pub published_at_iso: String,
    pub revision_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifiedClaim {
    pub claim_text: String,
    pub trusty_uri: String,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplyEntry {
    pub author_label: String,
    pub body_html: String,
    pub submitted_at_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetractionNotice {
    /// COPE-aligned reason (e.g., "data error", "ethics violation").
    pub reason: String,
    /// Iso-8601 date the retraction was issued.
    pub issued_at_iso: String,
    /// Optional Trusty URI of the retraction nanopublication.
    pub retraction_nanopub: Option<String>,
}
