use super::{MAX_STAGING_FILE_BYTES, ScholarlyVenue};
use crate::publication::PublicationManifest;
use flate2::read::GzDecoder;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tar::Archive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFinding {
    pub code: &'static str,
    pub message: String,
}

pub fn validate_scholarly_staging(
    out_dir: &Path,
    venue: ScholarlyVenue,
    manifest: &PublicationManifest,
) -> Result<(), Vec<ValidationFinding>> {
    let mut findings: Vec<ValidationFinding> = Vec::new();
    let plan = super::staging_artifacts(venue);

    for art in &plan {
        if art.relative_path.as_str() == "citations.json" {
            let src = manifest
                .citations_json
                .as_deref()
                .map(str::trim)
                .unwrap_or("");
            if src.is_empty() {
                continue;
            }
        }
        let p = out_dir.join(&art.relative_path);
        if !p.is_file() {
            findings.push(ValidationFinding {
                code: "staging_file_missing",
                message: format!("expected file {}", p.display()),
            });
            continue;
        }
        match fs::metadata(&p) {
            Ok(m) => {
                if m.len() > MAX_STAGING_FILE_BYTES {
                    findings.push(ValidationFinding {
                        code: "staging_file_too_large",
                        message: format!(
                            "{} exceeds max {} bytes",
                            p.display(),
                            MAX_STAGING_FILE_BYTES
                        ),
                    });
                }
                if m.len() == 0 && art.require_non_empty_source {
                    findings.push(ValidationFinding {
                        code: "staging_file_empty",
                        message: format!("{} must not be empty", p.display()),
                    });
                }
            }
            Err(e) => findings.push(ValidationFinding {
                code: "staging_metadata",
                message: format!("{}: {e}", p.display()),
            }),
        }
        if let Err(e) = file_kind_check(&p) {
            findings.push(e);
        }
        findings.extend(content_checks(&p, art.relative_path.as_str(), venue));
    }

    if matches!(venue, ScholarlyVenue::ArxivAssist) {
        findings.extend(arxiv_staging_handoff_quality_notes(out_dir, manifest));
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

pub fn validate_arxiv_submission_tar_gz(bytes: &[u8]) -> Vec<ValidationFinding> {
    let mut findings = Vec::new();
    if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        findings.push(ValidationFinding {
            code: "arxiv_bundle_not_gzip",
            message: "bundle must be gzip (0x1f 0x8b)".into(),
        });
        return findings;
    }
    let dec = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(dec);
    let mut tex_count = 0_usize;
    let mut main_tex_present = false;
    let mut entries = match archive.entries() {
        Ok(e) => e,
        Err(e) => {
            findings.push(ValidationFinding {
                code: "arxiv_bundle_tar_unreadable",
                message: format!("tar entries: {e}"),
            });
            return findings;
        }
    };
    for ent in entries.by_ref() {
        let ent = match ent {
            Ok(e) => e,
            Err(e) => {
                findings.push(ValidationFinding {
                    code: "arxiv_bundle_tar_entry",
                    message: format!("{e}"),
                });
                continue;
            }
        };
        let path = match ent.path() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let path_s = path.to_string_lossy();
        if path_s.contains("..") {
            findings.push(ValidationFinding {
                code: "arxiv_path_traversal",
                message: format!("unsafe path {path_s:?}"),
            });
            continue;
        }
        let lower = path_s.to_ascii_lowercase();
        if lower.ends_with(".tex") {
            tex_count += 1;
        }
        if lower == "main.tex" {
            main_tex_present = true;
        }
        // vox-arch-check: allow dynlib-ext
        for bad in [".exe", ".dll", ".dmg", ".pkg", ".deb", ".msi", ".app/"] {
            if lower.contains(bad) {
                findings.push(ValidationFinding {
                    code: "arxiv_disallowed_binary",
                    message: format!("path {path_s:?} matches disallowed pattern {bad:?}"),
                });
            }
        }
        for bad in [".zip", ".rar", ".7z", ".tar"] {
            if lower.ends_with(bad) {
                findings.push(ValidationFinding {
                    code: "arxiv_disallowed_nested_archive",
                    message: format!("path {path_s:?} uses nested archive suffix {bad:?}"),
                });
            }
        }
        if !arxiv_allowed_archive_path(&lower) {
            findings.push(ValidationFinding {
                code: "arxiv_unrecognized_file_family",
                message: format!(
                    "path {path_s:?} is not in the supported arXiv assist file families"
                ),
            });
        }
    }
    if tex_count == 0 {
        findings.push(ValidationFinding {
            code: "arxiv_missing_tex",
            message: "arXiv bundle must contain at least one .tex source".into(),
        });
    }
    if tex_count > 0 && !main_tex_present {
        findings.push(ValidationFinding {
            code: "arxiv_missing_main_tex",
            message: "arXiv assist bundle must include main.tex as the primary handoff entrypoint"
                .into(),
        });
    }
    if tex_count > 1 {
        findings.push(ValidationFinding {
            code: "arxiv_multiple_tex_sources",
            message: format!(
                "bundle contains {tex_count} `.tex` files \u{2014} ensure `main.tex` is the unique entrypoint or split submissions deliberately"
            ),
        });
    }
    findings
}

fn arxiv_allowed_archive_path(lower: &str) -> bool {
    [
        ".tex", ".sty", ".cls", ".bst", ".bib", ".bbl", ".bbx", ".cbx", ".cfg", ".clo", ".def",
        ".fd", ".ist", ".txt", ".md", ".json", ".yaml", ".yml", ".cff", ".png", ".jpg", ".jpeg",
        ".pdf", ".eps", ".ps", ".svg", ".csv", ".tsv",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}

fn file_kind_check(path: &Path) -> Result<(), ValidationFinding> {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return Err(ValidationFinding {
            code: "staging_bad_filename",
            message: path.display().to_string(),
        });
    };
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".md")
        || lower.ends_with(".cff")
        || lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tex")
    {
        return Ok(());
    }
    Err(ValidationFinding {
        code: "staging_unexpected_extension",
        message: format!(
            "{} (allowed: .md .cff .json .yaml .yml .tar.gz .tex)",
            path.display()
        ),
    })
}

fn content_checks(
    path: &Path,
    relative_path: &str,
    venue: ScholarlyVenue,
) -> Vec<ValidationFinding> {
    let mut out = Vec::new();
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            out.push(ValidationFinding {
                code: "staging_read",
                message: format!("{}: {e}", path.display()),
            });
            return out;
        }
    };
    let is_binary_staging = relative_path == "arxiv_bundle.tar.gz";
    if !is_binary_staging && std::str::from_utf8(&bytes).is_err() {
        out.push(ValidationFinding {
            code: "staging_invalid_utf8",
            message: format!("{} must be valid UTF-8", path.display()),
        });
        return out;
    }
    match relative_path {
        "citations.json" | "crossref_work.json" | "zenodo.json" | "arxiv_handoff.json" => {
            if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                out.push(ValidationFinding {
                    code: "staging_json_invalid",
                    message: format!("{}: {e}", path.display()),
                });
            }
        }
        "CITATION.cff" => {
            if let Err(e) = serde_yaml::from_slice::<serde_yaml::Value>(&bytes) {
                out.push(ValidationFinding {
                    code: "staging_cff_invalid",
                    message: format!("{}: {e}", path.display()),
                });
            }
        }
        "main.tex" => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                for (needle, code) in [
                    ("\\documentclass", "staging_arxiv_main_tex_documentclass"),
                    ("\\title{", "staging_arxiv_main_tex_title"),
                    ("\\author{", "staging_arxiv_main_tex_author"),
                    ("\\begin{abstract}", "staging_arxiv_main_tex_abstract"),
                ] {
                    if !text.contains(needle) {
                        out.push(ValidationFinding {
                            code,
                            message: format!("{} must contain {}", path.display(), needle),
                        });
                    }
                }
                let lower_tex = text.to_ascii_lowercase();
                if lower_tex.contains("\\usepackage{minted}") {
                    out.push(ValidationFinding {
                        code: "staging_arxiv_minted_package",
                        message: format!(
                            "{} uses `minted` \u{2014} arXiv may require `-shell-escape` / custom packaging; verify venue policy",
                            path.display()
                        ),
                    });
                }
            }
        }
        _ => {}
    }
    if matches!(venue, ScholarlyVenue::Zenodo)
        && relative_path == "zenodo.json"
        && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && val.get("metadata").and_then(|m| m.as_object()).is_none()
    {
        out.push(ValidationFinding {
            code: "staging_zenodo_json_shape",
            message: format!(
                "{} must be an object with a `metadata` object (Zenodo deposit envelope)",
                path.display()
            ),
        });
    }
    if matches!(venue, ScholarlyVenue::ArxivAssist)
        && relative_path == "arxiv_handoff.json"
        && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && !validate_arxiv_handoff_value(&val)
    {
        out.push(ValidationFinding {
            code: "staging_arxiv_handoff_shape",
            message: format!(
                "{} must include schema_version=1, workflow=arxiv_operator_assist, publication_id, title, primary_author, content_sha3_256, main_tex_relpath, body_markdown_relpath, arxiv_bundle_relpath, staging_checksums_relpath",
                path.display()
            ),
        });
    }
    if matches!(venue, ScholarlyVenue::ArxivAssist) && relative_path == "arxiv_bundle.tar.gz" {
        for f in validate_arxiv_submission_tar_gz(&bytes) {
            if f.code == "arxiv_bundle_not_gzip" {
                out.push(ValidationFinding {
                    code: "staging_arxiv_bundle_not_gzip",
                    message: format!(
                        "{} must be a gzip stream (starts with 1f 8b)",
                        path.display()
                    ),
                });
            } else {
                out.push(f);
            }
        }
    }
    out
}

fn validate_arxiv_handoff_value(v: &serde_json::Value) -> bool {
    v.get("schema_version").and_then(|n| n.as_i64()) == Some(1)
        && v.get("workflow").and_then(|s| s.as_str()) == Some("arxiv_operator_assist")
        && v.get("publication_id")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.is_empty())
        && v.get("title")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.is_empty())
        && v.get("primary_author")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.is_empty())
        && v.get("content_sha3_256")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.is_empty())
        && v.get("main_tex_relpath").and_then(|s| s.as_str()) == Some("main.tex")
        && v.get("body_markdown_relpath").and_then(|s| s.as_str()) == Some("body.md")
        && v.get("arxiv_bundle_relpath").and_then(|s| s.as_str()) == Some("arxiv_bundle.tar.gz")
        && v.get("staging_checksums_relpath").and_then(|s| s.as_str())
            == Some("staging_checksums.json")
}

fn arxiv_staging_handoff_quality_notes(
    out_dir: &Path,
    manifest: &PublicationManifest,
) -> Vec<ValidationFinding> {
    let mut v = Vec::new();
    let handoff_p = out_dir.join("arxiv_handoff.json");
    if handoff_p.is_file()
        && let Ok(raw) = fs::read_to_string(&handoff_p)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        if !validate_arxiv_handoff_value(&val) {
            v.push(ValidationFinding {
                        code: "arxiv_handoff_contract_mismatch",
                        message: "arxiv_handoff.json failed contract validation against expected arXiv assist operator envelope".into(),
                    });
        }
        let digest_ok = val.get("content_sha3_256").and_then(|x| x.as_str())
            == Some(manifest.content_sha3_256().as_str());
        if !digest_ok {
            v.push(ValidationFinding {
                        code: "arxiv_handoff_digest_mismatch",
                        message: "arxiv_handoff.json content_sha3_256 does not match current manifest digest \u{2014} regenerate staging".into(),
                    });
        }
    }
    let checksums = out_dir.join("staging_checksums.json");
    if checksums.is_file()
        && let Ok(raw) = fs::read_to_string(&checksums)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        let has_bundle = val
            .get("sha3_256")
            .and_then(|m| m.as_object())
            .is_some_and(|m| m.contains_key("arxiv_bundle.tar.gz"));
        if !has_bundle {
            v.push(ValidationFinding {
                code: "staging_checksums_missing_arxiv_bundle",
                message: "staging_checksums.json should record arxiv_bundle.tar.gz for custody"
                    .into(),
            });
        }
    }
    v
}
