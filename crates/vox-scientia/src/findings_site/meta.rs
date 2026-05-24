//! Highwire-style meta tags for Google Scholar pickup.
//!
//! Each `citation_*` tag emits as a separate `<meta name="...">` element.
//! Google Scholar uses these to ingest scholarly content; the canonical
//! reference is the Scholar Inclusion Guidelines (October 2024 revision).

use serde::{Deserialize, Serialize};

use super::page::FindingPage;

/// Materialized set of Highwire `citation_*` meta tags for one finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HighwireMetaTags {
    pub citation_title: String,
    pub citation_author: Vec<String>,
    pub citation_publication_date: String,
    pub citation_doi: Option<String>,
    pub citation_abstract_html_url: Option<String>,
    pub citation_pdf_url: Option<String>,
    /// Non-empty when the page is rendered for a retracted finding.
    pub citation_retracted: bool,
}

/// Build Highwire meta tags from a [`FindingPage`].
pub fn build_highwire_meta_tags(page: &FindingPage) -> HighwireMetaTags {
    HighwireMetaTags {
        citation_title: page.title.clone(),
        citation_author: page.authors.iter().map(|a| a.name.clone()).collect(),
        citation_publication_date: page.published_at_iso.clone(),
        citation_doi: page.doi.clone(),
        // The page itself is canonical; SSG layer can wire a PDF deposit
        // URL when available.
        citation_abstract_html_url: Some(format!("/findings/{}", page.trusty_uri)),
        citation_pdf_url: None,
        citation_retracted: page.retraction.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::page::{Author, RetractionNotice};

    fn sample_page() -> FindingPage {
        FindingPage {
            title: "A study".into(),
            authors: vec![Author {
                name: "Alice".into(),
                orcid: None,
            }],
            abstract_text: "x".into(),
            body_html: "<p>y</p>".into(),
            trusty_uri: "RA1234".into(),
            doi: Some("10.0000/test".into()),
            versions: vec![],
            verified_claims: vec![],
            replies: vec![],
            retraction: None,
            published_at_iso: "2026-05-15".into(),
        }
    }

    #[test]
    fn meta_tags_lift_title_authors_and_date() {
        let m = build_highwire_meta_tags(&sample_page());
        assert_eq!(m.citation_title, "A study");
        assert_eq!(m.citation_author, vec!["Alice".to_string()]);
        assert_eq!(m.citation_publication_date, "2026-05-15");
        assert_eq!(m.citation_doi.as_deref(), Some("10.0000/test"));
        assert!(!m.citation_retracted);
    }

    #[test]
    fn retracted_pages_set_citation_retracted_flag() {
        let mut p = sample_page();
        p.retraction = Some(RetractionNotice {
            reason: "data error".into(),
            issued_at_iso: "2026-06-01".into(),
            retraction_nanopub: None,
        });
        let m = build_highwire_meta_tags(&p);
        assert!(m.citation_retracted);
    }

    #[test]
    fn abstract_html_url_uses_trusty_uri_path() {
        let m = build_highwire_meta_tags(&sample_page());
        assert_eq!(m.citation_abstract_html_url.as_deref(), Some("/findings/RA1234"));
    }
}
