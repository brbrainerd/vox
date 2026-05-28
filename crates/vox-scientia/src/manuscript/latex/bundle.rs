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

use flate2::write::GzEncoder;
use flate2::Compression;
use thiserror::Error;
use crate::manuscript::scaffold::ScaffoldInput;

use super::render::render_latex;

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
pub fn render_arxiv_bundle(
    input: &ScaffoldInput,
    figure_blobs: &[(String, Vec<u8>)],
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

        let gz = tar.into_inner()?;
        gz.finish()?;
    }
    Ok(buf)
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
        let bundle = render_arxiv_bundle(
            &input,
            &[("figures/fig-01.svg".to_string(), svg.clone())],
        )
        .unwrap();
        let entries = list_bundle_entries(&bundle).unwrap();
        let mut paths: Vec<&String> = entries.iter().map(|(p, _)| p).collect();
        paths.sort();
        assert_eq!(paths, vec![&"figures/fig-01.svg".to_string(), &"main.tex".to_string()]);
        let figure = entries.iter().find(|(p, _)| p == "figures/fig-01.svg").unwrap();
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
}
