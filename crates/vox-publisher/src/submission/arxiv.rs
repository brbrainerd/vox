use super::StagingExportError;
use crate::publication::PublicationManifest;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::path::Path;

#[must_use]
pub fn arxiv_operator_handoff_value(manifest: &PublicationManifest) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "workflow": "arxiv_operator_assist",
        "publication_id": manifest.publication_id,
        "title": manifest.title,
        "primary_author": manifest.author,
        "content_sha3_256": manifest.content_sha3_256(),
        "main_tex_relpath": "main.tex",
        "body_markdown_relpath": "body.md",
        "staging_generated_by": "vox-publisher/submission",
        "arxiv_bundle_relpath": "arxiv_bundle.tar.gz",
        "staging_checksums_relpath": "staging_checksums.json",
        "note": "Operator-assisted arXiv submission; not an automated arXiv API deposit.",
    })
}

pub fn arxiv_assist_main_tex(manifest: &PublicationManifest) -> String {
    let title = latex_escape_minimal(&manifest.title);
    let author = latex_escape_minimal(&manifest.author);
    let abs = manifest
        .abstract_text
        .as_deref()
        .map(latex_escape_minimal)
        .unwrap_or_else(|| "Abstract pending.".to_string());
    format!(
        "% Auto-generated main.tex for arXiv operator-assist staging (vox-publisher).\n\
        \\documentclass{{article}}\n\
        \\usepackage{{hyperref}}\n\
        \\title{{{title}}}\n\
        \\author{{{author}}}\n\
        \\begin{{document}}\n\
        \\maketitle\n\
        \\begin{{abstract}}\n\
        {abs}\n\
        \\end{{abstract}}\n\
        \\noindent\\textit{{Companion manuscript:}} \\texttt{{body.md}} in this bundle.\n\
        \\end{{document}}\n"
    )
}

fn latex_escape_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len().saturating_mul(2));
    for c in s.chars() {
        match c {
            '\\' | '{' | '}' | '#' | '$' | '%' | '^' | '_' | '&' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\n\n"),
            _ => out.push(c),
        }
    }
    out
}

pub fn pack_arxiv_staging_tar_gz(
    staging_dir: &Path,
    dest: &Path,
) -> Result<(), StagingExportError> {
    let _ = fs::remove_file(dest);
    let out = fs::File::create(dest)?;
    let enc = GzEncoder::new(out, Compression::default());
    let mut builder = tar::Builder::new(enc);
    builder.mode(tar::HeaderMode::Deterministic);

    let mut names: Vec<String> = fs::read_dir(staging_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n != "arxiv_bundle.tar.gz")
        .collect();
    names.sort_unstable();

    for name in names {
        let path = staging_dir.join(&name);
        let mut file = fs::File::open(&path)?;
        builder
            .append_file(&name, &mut file)
            .map_err(StagingExportError::Io)?;
    }

    let enc = builder.into_inner().map_err(StagingExportError::Io)?;
    enc.finish().map_err(StagingExportError::Io)?;
    Ok(())
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    #[test]
    fn latex_escape_minimal_escapes_specials_and_doubles_newline() {
        // Catches: missing escape of a TeX special (e.g. % or $ leaking unescaped → broken .tex),
        // or newline not expanded to a paragraph break (\n\n).
        assert_eq!(latex_escape_minimal("100% of $x"), r"100\% of \$x");
        assert_eq!(latex_escape_minimal("a_b^c"), r"a\_b\^c");
        assert_eq!(latex_escape_minimal("line1\nline2"), "line1\n\nline2");
        assert_eq!(latex_escape_minimal("plain text"), "plain text");
    }
}
