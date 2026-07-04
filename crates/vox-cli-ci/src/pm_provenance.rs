//! `vox ci pm-provenance` — validate `.vox_modules/provenance/*.json` (schema from `vox pm publish`).

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "vox.pm.provenance/1";

fn scan_root(repo_root: &Path, root: &Path) -> PathBuf {
    if root.is_absolute() {
        root.to_path_buf()
    } else {
        repo_root.join(root)
    }
}

fn validate_doc(raw: &str, path: &Path) -> Result<()> {
    let v: Value =
        serde_json::from_str(raw).with_context(|| format!("parse {}", path.display()))?;
    let schema = v
        .get("schema")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("{}: missing string `schema`", path.display()))?;
    if schema != SCHEMA {
        return Err(anyhow!(
            "{}: expected schema `{SCHEMA}`, got `{schema}`",
            path.display()
        ));
    }
    for key in ["package", "version", "content_hash"] {
        let s = v
            .get(key)
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("{}: missing string `{key}`", path.display()))?;
        if s.is_empty() {
            return Err(anyhow!("{}: `{key}` must be non-empty", path.display()));
        }
    }
    let epoch = v
        .get("built_at_epoch")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| anyhow!("{}: missing `built_at_epoch` (u64)", path.display()))?;
    if epoch == 0 {
        return Err(anyhow!(
            "{}: `built_at_epoch` must be non-zero",
            path.display()
        ));
    }
    if let Some(r) = v.get("registry") {
        let s = r
            .as_str()
            .ok_or_else(|| anyhow!("{}: `registry` must be a string when set", path.display()))?;
        if s.is_empty() {
            return Err(anyhow!(
                "{}: `registry` must be non-empty when set",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Verify PM provenance sidecars under `<root>/.vox_modules/provenance/`.
pub fn run(repo_root: &Path, root: &Path, strict: bool) -> Result<()> {
    let base = scan_root(repo_root, root);
    let prov_dir = base.join(".vox_modules").join("provenance");

    if !prov_dir.is_dir() {
        if strict {
            return Err(anyhow!(
                "strict: missing directory `{}` (run `vox pm publish` or copy provenance artifacts)",
                prov_dir.display()
            ));
        }
        println!(
            "pm-provenance: no `{}` — skip (not strict)",
            prov_dir.display()
        );
        return Ok(());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&prov_dir)
        .with_context(|| format!("read_dir {}", prov_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ex| ex == "json"))
        .collect();
    files.sort();

    if files.is_empty() {
        if strict {
            return Err(anyhow!("strict: no `*.json` under {}", prov_dir.display()));
        }
        println!(
            "pm-provenance: empty `{}` — skip (not strict)",
            prov_dir.display()
        );
        return Ok(());
    }

    let mut ok = 0usize;
    for p in &files {
        let raw = fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        validate_doc(&raw, p)?;
        ok += 1;
    }

    println!(
        "pm-provenance: OK — {ok} file(s) under {}",
        prov_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn validate_doc_accepts_publish_shape() {
        let j = r#"{"schema":"vox.pm.provenance/1","package":"p","version":"1.0.0","content_hash":"abc","built_at_epoch":1,"tool":"vox-cli","registry":"http://localhost:0"}"#;
        validate_doc(j, Path::new("x.json")).unwrap();
    }

    #[test]
    fn validate_doc_rejects_wrong_schema() {
        let j = r#"{"schema":"other","package":"p","version":"1.0.0","content_hash":"a","built_at_epoch":1}"#;
        assert!(validate_doc(j, Path::new("x")).is_err());
    }

    #[test]
    fn validate_doc_rejects_empty_registry() {
        let j = r#"{"schema":"vox.pm.provenance/1","package":"p","version":"1.0.0","content_hash":"a","built_at_epoch":1,"registry":""}"#;
        assert!(validate_doc(j, Path::new("x")).is_err());
    }

    #[test]
    fn run_non_strict_missing_dir_ok() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), Path::new("."), false).unwrap();
    }

    #[test]
    fn run_strict_missing_dir_fails() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(run(tmp.path(), Path::new("."), true).is_err());
    }

    #[test]
    fn run_strict_validates_json_files() {
        let tmp = tempfile::tempdir().unwrap();
        let prov = tmp.path().join(".vox_modules").join("provenance");
        fs::create_dir_all(&prov).unwrap();
        let mut f = fs::File::create(prov.join("a.json")).unwrap();
        writeln!(
            f,
            r#"{{"schema":"vox.pm.provenance/1","package":"x","version":"0.1.0","content_hash":"h","built_at_epoch":42}}"#
        )
        .unwrap();
        run(tmp.path(), Path::new("."), true).unwrap();
    }
}
