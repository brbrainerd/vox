//! HTML page rendering.

use super::meta::build_highwire_meta_tags;
use super::page::FindingPage;

/// Render a complete `<!doctype html>` page for the given finding.
///
/// Sections (in order): retraction banner (if any), title, authors,
/// abstract, version history table, body (HTML; producer-sanitized),
/// verified-claims sidebar, reply thread, footer.
pub fn render_finding_page(page: &FindingPage) -> String {
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<title>");
    out.push_str(&escape(&page.title));
    out.push_str("</title>\n");

    // Highwire meta tags.
    let meta = build_highwire_meta_tags(page);
    push_meta(&mut out, "citation_title", &meta.citation_title);
    for author in &meta.citation_author {
        push_meta(&mut out, "citation_author", author);
    }
    push_meta(
        &mut out,
        "citation_publication_date",
        &meta.citation_publication_date,
    );
    if let Some(doi) = &meta.citation_doi {
        push_meta(&mut out, "citation_doi", doi);
    }
    if let Some(url) = &meta.citation_abstract_html_url {
        push_meta(&mut out, "citation_abstract_html_url", url);
    }
    if let Some(url) = &meta.citation_pdf_url {
        push_meta(&mut out, "citation_pdf_url", url);
    }
    if meta.citation_retracted {
        push_meta(&mut out, "citation_retracted", "yes");
    }

    out.push_str("</head>\n<body>\n");

    // Retraction banner (COPE-aligned; emitted BEFORE the body content).
    if let Some(ret) = &page.retraction {
        out.push_str("<aside class=\"vox-retraction-banner\" role=\"alert\">\n");
        out.push_str("  <strong>Retracted.</strong> Reason: ");
        out.push_str(&escape(&ret.reason));
        out.push_str(". Issued ");
        out.push_str(&escape(&ret.issued_at_iso));
        if let Some(np) = &ret.retraction_nanopub {
            out.push_str(". <a href=\"");
            out.push_str(&escape(np));
            out.push_str("\">Retraction nanopublication</a>");
        }
        out.push_str(".\n</aside>\n");
    }

    out.push_str("<h1>");
    out.push_str(&escape(&page.title));
    out.push_str("</h1>\n");

    // Authors.
    out.push_str("<p class=\"vox-authors\">\n  ");
    for (i, a) in page.authors.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        if let Some(orcid) = &a.orcid {
            out.push_str("<a href=\"");
            out.push_str(&escape(orcid));
            out.push_str("\">");
            out.push_str(&escape(&a.name));
            out.push_str("</a>");
        } else {
            out.push_str(&escape(&a.name));
        }
    }
    out.push_str("\n</p>\n");

    out.push_str("<section class=\"vox-abstract\">\n  <h2>Abstract</h2>\n  <p>");
    out.push_str(&escape(&page.abstract_text));
    out.push_str("</p>\n</section>\n");

    // Version history.
    out.push_str("<section class=\"vox-version-history\">\n  <h2>Version history</h2>\n");
    if page.versions.is_empty() {
        out.push_str("  <p>No earlier versions.</p>\n");
    } else {
        out.push_str("  <table>\n    <thead><tr><th>Date</th><th>DOI</th><th>Trusty URI</th><th>Revision summary</th></tr></thead>\n    <tbody>\n");
        for v in &page.versions {
            let is_current = v.trusty_uri == page.trusty_uri;
            out.push_str(if is_current {
                "      <tr class=\"vox-current-version\">"
            } else {
                "      <tr>"
            });
            out.push_str("<td>");
            out.push_str(&escape(&v.published_at_iso));
            out.push_str("</td>");
            out.push_str("<td>");
            if let Some(doi) = &v.doi {
                out.push_str(&escape(doi));
            } else {
                out.push_str("—");
            }
            out.push_str("</td>");
            out.push_str("<td><code>");
            out.push_str(&escape(&v.trusty_uri));
            out.push_str("</code></td>");
            out.push_str("<td>");
            out.push_str(&escape(v.revision_summary.as_deref().unwrap_or("—")));
            out.push_str("</td></tr>\n");
        }
        out.push_str("    </tbody>\n  </table>\n");
    }
    out.push_str("</section>\n");

    // Body (HTML; producer-sanitized).
    out.push_str("<section class=\"vox-body\">\n");
    out.push_str(&page.body_html);
    out.push_str("\n</section>\n");

    // Verified claims sidebar.
    out.push_str("<aside class=\"vox-verified-claims\">\n  <h2>Verified claims</h2>\n");
    if page.verified_claims.is_empty() {
        out.push_str("  <p>No verified atomic claims attached.</p>\n");
    } else {
        out.push_str("  <ul>\n");
        for c in &page.verified_claims {
            out.push_str("    <li><a href=\"");
            out.push_str(&escape(&c.trusty_uri));
            out.push_str("\">");
            out.push_str(&escape(&c.claim_text));
            out.push_str("</a> — ");
            out.push_str(&escape(&c.verdict));
            out.push_str("</li>\n");
        }
        out.push_str("  </ul>\n");
    }
    out.push_str("</aside>\n");

    // Reply thread.
    out.push_str("<section class=\"vox-replies\">\n  <h2>Replies</h2>\n");
    if page.replies.is_empty() {
        out.push_str("  <p>No replies have been submitted.</p>\n");
    } else {
        for r in &page.replies {
            out.push_str("  <article class=\"vox-reply\">\n");
            out.push_str("    <header><strong>");
            out.push_str(&escape(&r.author_label));
            out.push_str("</strong> · <time>");
            out.push_str(&escape(&r.submitted_at_iso));
            out.push_str("</time></header>\n");
            out.push_str("    <div class=\"vox-reply-body\">");
            // Reply body is producer-sanitized HTML; pass through.
            out.push_str(&r.body_html);
            out.push_str("</div>\n  </article>\n");
        }
    }
    out.push_str("</section>\n");

    out.push_str("</body>\n</html>\n");
    out
}

fn push_meta(out: &mut String, name: &str, content: &str) {
    out.push_str("<meta name=\"");
    out.push_str(name);
    out.push_str("\" content=\"");
    out.push_str(&escape(content));
    out.push_str("\">\n");
}

/// Minimal HTML-attribute / text escaper. Producer code already sanitizes
/// `body_html`; this is for everything else (titles, names, dates, etc.).
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::page::*;

    fn sample_page() -> FindingPage {
        FindingPage {
            title: "Fast Foo".into(),
            authors: vec![
                Author {
                    name: "Alice".into(),
                    orcid: Some("https://orcid.org/0000-0002-1825-0097".into()),
                },
                Author {
                    name: "Bob".into(),
                    orcid: None,
                },
            ],
            abstract_text: "A short abstract.".into(),
            body_html: "<p>The body.</p>".into(),
            trusty_uri: "RA1234567890abcdef".into(),
            doi: Some("10.0000/foo".into()),
            versions: vec![VersionHistoryEntry {
                trusty_uri: "RA1234567890abcdef".into(),
                doi: Some("10.0000/foo".into()),
                published_at_iso: "2026-05-15".into(),
                revision_summary: Some("Initial version".into()),
            }],
            verified_claims: vec![VerifiedClaim {
                claim_text: "p95 dropped 23%".into(),
                trusty_uri: "RAclaim001".into(),
                verdict: "Supported".into(),
            }],
            replies: vec![],
            retraction: None,
            published_at_iso: "2026-05-15".into(),
        }
    }

    #[test]
    fn rendered_page_starts_with_doctype_and_contains_title() {
        let html = render_finding_page(&sample_page());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>Fast Foo</title>"));
    }

    #[test]
    fn highwire_meta_tags_are_emitted() {
        let html = render_finding_page(&sample_page());
        assert!(html.contains("<meta name=\"citation_title\" content=\"Fast Foo\">"));
        assert!(html.contains("<meta name=\"citation_author\" content=\"Alice\">"));
        assert!(html.contains("<meta name=\"citation_author\" content=\"Bob\">"));
        assert!(html.contains("<meta name=\"citation_doi\" content=\"10.0000/foo\">"));
        assert!(html.contains("<meta name=\"citation_publication_date\" content=\"2026-05-15\">"));
    }

    #[test]
    fn retraction_banner_emitted_above_body_when_present() {
        let mut p = sample_page();
        p.retraction = Some(RetractionNotice {
            reason: "data error".into(),
            issued_at_iso: "2026-06-01".into(),
            retraction_nanopub: Some("https://np.example/retract".into()),
        });
        let html = render_finding_page(&p);
        let banner_idx = html.find("vox-retraction-banner").expect("banner missing");
        let body_idx = html.find("vox-body").expect("body missing");
        assert!(
            banner_idx < body_idx,
            "retraction banner must render BEFORE the body"
        );
        assert!(html.contains("<meta name=\"citation_retracted\" content=\"yes\">"));
        assert!(html.contains("data error"));
        assert!(html.contains("https://np.example/retract"));
    }

    #[test]
    fn no_retraction_means_no_banner_no_meta() {
        let html = render_finding_page(&sample_page());
        assert!(!html.contains("vox-retraction-banner"));
        assert!(!html.contains("citation_retracted"));
    }

    #[test]
    fn current_version_row_marked_in_history_table() {
        let mut p = sample_page();
        p.versions.insert(
            0,
            VersionHistoryEntry {
                trusty_uri: "RA1234567890abcdef".into(), // same as page.trusty_uri
                doi: Some("10.0000/foo".into()),
                published_at_iso: "2026-05-15".into(),
                revision_summary: None,
            },
        );
        let html = render_finding_page(&p);
        assert!(html.contains("vox-current-version"));
    }

    #[test]
    fn author_with_orcid_renders_as_link_others_as_text() {
        let html = render_finding_page(&sample_page());
        assert!(html.contains("<a href=\"https://orcid.org/0000-0002-1825-0097\">Alice</a>"));
        // Bob has no ORCID — should appear as plain text, not as a link.
        assert!(
            html.contains(">Bob<") || html.contains(", Bob"),
            "expected Bob as text node, got: {}",
            html
        );
    }

    #[test]
    fn verified_claims_render_as_trusty_uri_links() {
        let html = render_finding_page(&sample_page());
        assert!(html.contains("<a href=\"RAclaim001\">p95 dropped 23%</a>"));
        assert!(html.contains("Supported"));
    }

    #[test]
    fn empty_replies_show_explicit_message() {
        let html = render_finding_page(&sample_page());
        assert!(html.contains("No replies have been submitted"));
    }

    #[test]
    fn replies_render_with_author_and_timestamp() {
        let mut p = sample_page();
        p.replies = vec![ReplyEntry {
            author_label: "Reviewer 2".into(),
            body_html: "<p>Disagrees.</p>".into(),
            submitted_at_iso: "2026-05-20".into(),
        }];
        let html = render_finding_page(&p);
        assert!(html.contains("Reviewer 2"));
        assert!(html.contains("2026-05-20"));
        assert!(html.contains("<p>Disagrees.</p>"));
    }

    #[test]
    fn escape_neutralizes_html_in_title_and_authors() {
        let mut p = sample_page();
        p.title = "<script>alert(1)</script>".into();
        p.authors[0].name = "Mallory<svg/onload=alert(1)>".into();
        let html = render_finding_page(&p);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Mallory&lt;svg/onload=alert(1)&gt;"));
    }
}
