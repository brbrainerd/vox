use std::fs;
use std::path::{Path, PathBuf};

use vox_doc_pipeline::pipeline::doctest::check_doctests;
use vox_doc_pipeline::pipeline::types::LintError;

fn collect_md_files(target: &Path, out: &mut Vec<PathBuf>) {
    if target.is_file() {
        if target.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(target.to_path_buf());
        }
        return;
    }
    if !target.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_md_files(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
}

pub fn run(paths: Vec<PathBuf>, strict: bool) -> anyhow::Result<()> {
    let mut files = Vec::new();
    for p in paths {
        collect_md_files(&p, &mut files);
    }

    let mut errors: Vec<LintError> = Vec::new();
    let mut files_checked = 0;

    for path in files {
        if let Ok(content) = fs::read_to_string(&path) {
            files_checked += 1;
            check_doctests(&path, &content, &mut errors);
        }
    }

    if !errors.is_empty() {
        for err in &errors {
            eprintln!(
                "Doctest failure in {}:{}: {:?}",
                err.file.display(),
                err.line,
                err.kind
            );
        }
        eprintln!(
            "Checked {} files. Found {} errors.",
            files_checked,
            errors.len()
        );
        if strict {
            anyhow::bail!("Doctest failures detected.");
        }
    } else {
        println!("Doctests OK. Checked {} files.", files_checked);
    }

    Ok(())
}
