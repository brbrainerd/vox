//! arXiv-ready bundle assembly.
//!
//! Produces a `.tar.gz` containing the rendered `main.tex` plus any figure
//! blobs supplied by the caller. The arXiv submission ingest accepts this
//! layout directly:
//!
//! ```text
//! main.tex
//! figures/fig-01.svg
//! figures/fig-02.png
//! ```
//!
//! Figure paths are taken verbatim from `ScaffoldInput::figures[*].path`
//! so they line up with the LaTeX `\includegraphics{...}` references.

use crate::manuscript::scaffold::ScaffoldInput;
use flate2::Compression;
use flate2::write::GzEncoder;
use thiserror::Error;

use super::render::render_latex;

// ── arXiv handoff metadata ──────────────────────────────────────────────────

/// Optional handoff metadata included in the arXiv bundle as
/// `arxiv-metadata.json` and `UPLOAD-CHECKLIST.md`.
///
/// When passed to [`render_arxiv_bundle_with_handoff`], two additional tar
/// entries are appended to the bundle.  Callers that pass `None` get the
/// legacy behaviour (only `main.tex` + figures).
#[derive(Debug, Clone)]
pub struct ArxivHandoffMeta {
    /// arXiv primary category, e.g. `"cs.SE"`.
    pub primary_category: String,
    /// How the category was chosen: `"flag"` when the operator supplied
    /// `--primary-category`; `"default"` when it fell back to the default.
    pub category_origin: String,
    /// SPDX license identifier, e.g. `"CC-BY-4.0"`.  `None` → omitted from
    /// the sidecar; the checklist will remind the operator to choose.
    pub license_spdx: Option<String>,
    /// Free-text comment field forwarded to arXiv (e.g. "12 pages, 3 figures").
    pub comments: Option<String>,
    /// Per-author ORCID overrides: `(author_name, orcid)`.  Merged into
    /// the sidecar by name-match, supplementing ORCIDs already in the scaffold.
    pub orcids: Vec<(String, Option<String>)>,
}

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("tar write: {0}")]
    Tar(#[from] std::io::Error),
    #[error("figure {path:?} declared in scaffold but no blob supplied")]
    MissingFigureBlob { path: String },
}

/// Build an arXiv-shaped `.tar.gz` from a [`ScaffoldInput`] + figure blobs.
///
/// The `figure_blobs` slice MUST include an entry for every
/// `input.figures[i].path`. Missing blobs return
/// [`BundleError::MissingFigureBlob`] rather than silently producing a
/// broken bundle.
///
/// This is the **original** contract-stable entry-point.  It produces
/// exactly the same output as before and is kept for callers that do not
/// supply handoff metadata (and for existing golden/tests).
pub fn render_arxiv_bundle(
    input: &ScaffoldInput,
    figure_blobs: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, BundleError> {
    render_arxiv_bundle_with_handoff(input, figure_blobs, None)
}

/// Like [`render_arxiv_bundle`] but optionally appends two extra tar entries:
///
/// - `arxiv-metadata.json` — pre-filled submission metadata sidecar
/// - `UPLOAD-CHECKLIST.md` — step-by-step manual upload guide
///
/// When `meta` is `None` the output is byte-identical to
/// [`render_arxiv_bundle`].
pub fn render_arxiv_bundle_with_handoff(
    input: &ScaffoldInput,
    figure_blobs: &[(String, Vec<u8>)],
    meta: Option<&ArxivHandoffMeta>,
) -> Result<Vec<u8>, BundleError> {
    // Validate every declared figure has a blob.
    for f in &input.figures {
        if !figure_blobs.iter().any(|(path, _)| path == &f.path) {
            return Err(BundleError::MissingFigureBlob {
                path: f.path.clone(),
            });
        }
    }

    let tex = render_latex(input);
    let mut buf: Vec<u8> = Vec::with_capacity(tex.len() + 1024);
    {
        let gz = GzEncoder::new(&mut buf, Compression::default());
        let mut tar = tar::Builder::new(gz);

        // Write main.tex.
        let mut header = tar::Header::new_gnu();
        let tex_bytes = tex.as_bytes();
        header.set_path("main.tex")?;
        header.set_size(tex_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, tex_bytes)?;

        // Write each figure blob at its declared path.
        for (path, blob) in figure_blobs {
            let mut header = tar::Header::new_gnu();
            header.set_path(path)?;
            header.set_size(blob.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, blob.as_slice())?;
        }

        // Optional handoff sidecar + checklist.
        if let Some(m) = meta {
            let sidecar = build_metadata_sidecar(input, m);
            let sidecar_bytes = sidecar.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_path("arxiv-metadata.json")?;
            header.set_size(sidecar_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, sidecar_bytes)?;

            let checklist = build_upload_checklist(m);
            let checklist_bytes = checklist.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_path("UPLOAD-CHECKLIST.md")?;
            header.set_size(checklist_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, checklist_bytes)?;
        }

        let gz = tar.into_inner()?;
        gz.finish()?;
    }
    Ok(buf)
}

// ── internal helpers ────────────────────────────────────────────────────────

fn build_metadata_sidecar(input: &ScaffoldInput, meta: &ArxivHandoffMeta) -> String {
    // Merge ORCIDs: scaffold authors carry orcid; meta.orcids may override or
    // supplement by name-match.
    let authors: Vec<serde_json::Value> = input
        .authors
        .iter()
        .map(|a| {
            // Check if meta.orcids has an override for this author.
            let orcid = meta
                .orcids
                .iter()
                .find(|(name, _)| name == &a.name)
                .and_then(|(_, o)| o.clone())
                .or_else(|| a.orcid.clone());
            match orcid {
                Some(o) => serde_json::json!({"name": a.name, "orcid": o}),
                None => serde_json::json!({"name": a.name}),
            }
        })
        .collect();

    let mut obj = serde_json::json!({
        "title": input.title_hint,
        "abstract": build_abstract_text(input),
        "authors": authors,
        "primary_category": meta.primary_category,
        "category_origin": meta.category_origin,
    });
    if let Some(lic) = &meta.license_spdx {
        obj["license_spdx"] = serde_json::Value::String(lic.clone());
    }
    if let Some(c) = &meta.comments {
        obj["comments"] = serde_json::Value::String(c.clone());
    }
    serde_json::to_string_pretty(&obj).expect("serde_json serialization is infallible")
}

/// Build a plain-text abstract from the scaffold's results rows + limitations.
/// arXiv needs a plain-text abstract; we produce a concise one-line summary
/// using the verified claims and (if present) the methods summary.
fn build_abstract_text(input: &ScaffoldInput) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = &input.methods_summary {
        let m = m.trim();
        if !m.is_empty() {
            parts.push(m.to_string());
        }
    }
    if !input.results_rows.is_empty() {
        let claims: Vec<&str> = input
            .results_rows
            .iter()
            .map(|r| r.claim_text.as_str())
            .collect();
        parts.push(format!("Key findings: {}", claims.join("; ")));
    }
    if !input.limitations.is_empty() {
        parts.push(format!("Limitations: {}", input.limitations.join("; ")));
    }
    if parts.is_empty() {
        // Fallback — human must fill this in.
        return "<!-- TODO(abstract): fill in abstract before submission -->".to_string();
    }
    parts.join(" ")
}

fn build_upload_checklist(meta: &ArxivHandoffMeta) -> String {
    let category_warning = if meta.category_origin == "default" {
        " **Note: this category was set by default — please VERIFY it is correct before submitting.**"
    } else {
        ""
    };

    let license_hint = match &meta.license_spdx {
        Some(l) => format!(" (suggested: `{l}` from manifest)"),
        None => " (not specified — choose the appropriate license on arXiv)".to_string(),
    };

    format!(
        "# arXiv Upload Checklist\n\
         \n\
         This checklist accompanies `arxiv-metadata.json`.  Complete each step \
         in order on <https://arxiv.org/submit>.\n\
         \n\
         1. **arXiv account + endorsement** — Ensure you have an arXiv account.  \
            If this is your first submission in category `{cat}`, you may need \
            endorsement from an existing arXiv author in that category.{cat_warn}\n\
         2. **Start a new submission** — Click \"Start New Submission\" and select \
            primary category `{cat}`.{cat_warn}\n\
         3. **Upload source** — Upload the entire contents of this `.tar.gz` \
            (or re-tar it without the `.tar.gz` wrapper) as the submission source.\n\
         4. **Paste metadata** — Copy each field from `arxiv-metadata.json` into \
            the arXiv form:\n\
            - **Title** — paste `title`\n\
            - **Abstract** — paste `abstract`; review and edit as needed \
              (machine-generated, not final)\n\
            - **Authors** — paste from `authors[*].name`; add ORCIDs from \
              `authors[*].orcid` if present\n\
            - **Comments** (optional) — paste `comments` if present\n\
         5. **Choose license**{license_hint} — Select the appropriate license on the \
            arXiv submission form.\n\
         6. **Preview + submit** — Use arXiv's preview to confirm LaTeX compiles \
            correctly, then submit.\n\
         \n\
         ---\n\
         \n\
         **Why is this manual?**  arXiv has no viable programmatic submission API \
         as of 2026: SWORD v1 is gatekept/deprecated and the replacement API was \
         archived unbuilt.  This bundle is a complete, self-contained handoff \
         package designed to minimise manual data-entry; submission itself \
         remains a human step.\n",
        cat = meta.primary_category,
        cat_warn = category_warning,
        license_hint = license_hint,
    )
}

/// Parse a `.tar.gz` bundle back into `(path, bytes)` entries. Useful for
/// tests + downstream consumers that want to inspect what was written.
pub fn list_bundle_entries(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, BundleError> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let gz = GzDecoder::new(bytes);
    let mut tar = tar::Archive::new(gz);
    let mut out = Vec::new();
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        out.push((path, content));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manuscript::scaffold::{FigureEntry, ScaffoldInput};

    fn minimal_input() -> ScaffoldInput {
        ScaffoldInput {
            title_hint: "Demo".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![],
        }
    }

    #[test]
    fn bundle_with_no_figures_contains_only_main_tex() {
        let bundle = render_arxiv_bundle(&minimal_input(), &[]).unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "main.tex");
        let tex = String::from_utf8(entries[0].1.clone()).unwrap();
        assert!(tex.starts_with("\\documentclass"));
    }

    #[test]
    fn bundle_includes_supplied_figure_blobs_at_declared_paths() {
        let mut input = minimal_input();
        input.figures = vec![FigureEntry {
            path: "figures/fig-01.svg".into(),
            sha3_256_hex: "abcd".into(),
            source_script: "scripts/plot.py".into(),
            caption_hint: None,
        }];
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec();
        let bundle =
            render_arxiv_bundle(&input, &[("figures/fig-01.svg".to_string(), svg.clone())])
                .unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        let mut paths: Vec<&String> = entries.iter().map(|(p, _)| p).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![&"figures/fig-01.svg".to_string(), &"main.tex".to_string()]
        );
        let figure = entries
            .iter()
            .find(|(p, _)| p == "figures/fig-01.svg")
            .unwrap();
        assert_eq!(figure.1, svg);
    }

    #[test]
    fn missing_figure_blob_returns_structured_error() {
        let mut input = minimal_input();
        input.figures = vec![FigureEntry {
            path: "figures/missing.png".into(),
            sha3_256_hex: "abcd".into(),
            source_script: "scripts/plot.py".into(),
            caption_hint: None,
        }];
        let err = render_arxiv_bundle(&input, &[]).unwrap_err();
        match err {
            BundleError::MissingFigureBlob { path } => {
                assert_eq!(path, "figures/missing.png");
            }
            _ => panic!("expected MissingFigureBlob"),
        }
    }

    #[test]
    fn bundle_round_trips_through_list_entries() {
        let bundle = render_arxiv_bundle(&minimal_input(), &[]).unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn bundle_is_deterministic_for_same_input() {
        // tar headers include mtime; we explicitly do NOT set it here. The
        // GNU tar header zero-initializes mtime, so the same input must
        // produce byte-identical output.
        let a = render_arxiv_bundle(&minimal_input(), &[]).unwrap();
        let b = render_arxiv_bundle(&minimal_input(), &[]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn multiple_figures_each_appear_in_bundle() {
        let mut input = minimal_input();
        input.figures = vec![
            FigureEntry {
                path: "figures/a.svg".into(),
                sha3_256_hex: "a".into(),
                source_script: "x".into(),
                caption_hint: None,
            },
            FigureEntry {
                path: "figures/b.svg".into(),
                sha3_256_hex: "b".into(),
                source_script: "x".into(),
                caption_hint: None,
            },
        ];
        let bundle = render_arxiv_bundle(
            &input,
            &[
                ("figures/a.svg".to_string(), b"a-bytes".to_vec()),
                ("figures/b.svg".to_string(), b"b-bytes".to_vec()),
            ],
        )
        .unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        assert_eq!(entries.len(), 3); // main.tex + 2 figures
    }

    fn minimal_meta() -> ArxivHandoffMeta {
        ArxivHandoffMeta {
            primary_category: "cs.SE".into(),
            category_origin: "flag".into(),
            license_spdx: Some("CC-BY-4.0".into()),
            comments: Some("12 pages, 3 figures".into()),
            orcids: vec![],
        }
    }

    #[test]
    fn handoff_bundle_contains_metadata_sidecar_and_checklist() {
        use crate::manuscript::scaffold::AuthorEntry;
        let mut input = minimal_input();
        input.title_hint = "Test Title".into();
        input.authors = vec![AuthorEntry {
            name: "Alice Example".into(),
            orcid: Some("0000-0001-2345-6789".into()),
            affiliation_ror: None,
        }];
        let meta = minimal_meta();
        let bundle = render_arxiv_bundle_with_handoff(&input, &[], Some(&meta)).unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        let entry_names: Vec<&str> = entries.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            entry_names.contains(&"arxiv-metadata.json"),
            "missing arxiv-metadata.json; entries: {entry_names:?}"
        );
        assert!(
            entry_names.contains(&"UPLOAD-CHECKLIST.md"),
            "missing UPLOAD-CHECKLIST.md; entries: {entry_names:?}"
        );
        // Parse and assert key fields in the sidecar.
        let sidecar_bytes = entries
            .iter()
            .find(|(p, _)| p == "arxiv-metadata.json")
            .map(|(_, b)| b.as_slice())
            .unwrap();
        let sidecar: serde_json::Value =
            serde_json::from_slice(sidecar_bytes).expect("sidecar is valid JSON");
        assert_eq!(sidecar["title"], "Test Title");
        assert_eq!(sidecar["primary_category"], "cs.SE");
        assert_eq!(sidecar["category_origin"], "flag");
        assert_eq!(sidecar["license_spdx"], "CC-BY-4.0");
        // Author name should appear.
        let authors = sidecar["authors"].as_array().unwrap();
        assert_eq!(authors[0]["name"], "Alice Example");
        assert_eq!(authors[0]["orcid"], "0000-0001-2345-6789");
        // Checklist should mention category and "manual".
        let checklist = entries
            .iter()
            .find(|(p, _)| p == "UPLOAD-CHECKLIST.md")
            .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
            .unwrap();
        assert!(
            checklist.contains("cs.SE"),
            "category missing from checklist"
        );
        assert!(
            checklist.contains("no viable programmatic submission API"),
            "why-manual explanation missing from checklist"
        );
    }

    #[test]
    fn bundle_without_meta_is_unchanged() {
        let input = minimal_input();
        let bundle_no_meta = render_arxiv_bundle_with_handoff(&input, &[], None).unwrap();
        let entries = list_bundle_entries(&bundle_no_meta).unwrap();
        let entry_names: Vec<&str> = entries.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            !entry_names.contains(&"arxiv-metadata.json"),
            "sidecar should not appear when meta is None"
        );
        assert!(
            !entry_names.contains(&"UPLOAD-CHECKLIST.md"),
            "checklist should not appear when meta is None"
        );
        // Output must also be identical to the legacy render_arxiv_bundle call.
        let bundle_legacy = render_arxiv_bundle(&input, &[]).unwrap();
        assert_eq!(
            bundle_no_meta, bundle_legacy,
            "render_arxiv_bundle_with_handoff(None) must be byte-identical to render_arxiv_bundle"
        );
    }
}
