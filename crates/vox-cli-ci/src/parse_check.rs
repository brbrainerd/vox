//! `vox ci json-parse-check` / `vox ci yaml-parse-check`
//!
//! Validates that every file matching the given glob is parseable JSON or YAML
//! respectively.  Replaces the `python3 -c "…"` / `python3 - <<'PY' …` blocks
//! that appeared in vox-mental-tracker.yml.

use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub fn run_json(globs: &[String]) -> Result<()> {
    let paths = expand_globs(globs)?;
    if paths.is_empty() {
        println!("json-parse-check: no files matched");
        return Ok(());
    }
    let mut failed = false;
    for path in &paths {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("json-parse-check: cannot read {}: {e}", path.display()))?;
        match serde_json::from_str::<serde_json::Value>(&contents) {
            Ok(_) => println!("OK {}", path.display()),
            Err(e) => {
                eprintln!("FAIL {}: {e}", path.display());
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow!("json-parse-check: one or more files failed"))
    } else {
        Ok(())
    }
}

pub fn run_yaml(globs: &[String]) -> Result<()> {
    let paths = expand_globs(globs)?;
    if paths.is_empty() {
        println!("yaml-parse-check: no files matched");
        return Ok(());
    }
    let mut failed = false;
    for path in &paths {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("yaml-parse-check: cannot read {}: {e}", path.display()))?;
        match serde_yaml::from_str::<serde_yaml::Value>(&contents) {
            Ok(_) => println!("OK {}", path.display()),
            Err(e) => {
                eprintln!("FAIL {}: {e}", path.display());
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow!("yaml-parse-check: one or more files failed"))
    } else {
        Ok(())
    }
}

fn expand_globs(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for pattern in patterns {
        let matches: Vec<_> = glob::glob(pattern)
            .map_err(|e| anyhow!("invalid glob pattern {pattern:?}: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("glob error for {pattern:?}: {e}"))?;
        paths.extend(matches);
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_fixture(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write fixture");
        path
    }

    #[test]
    fn json_parse_check_accepts_valid_json() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok.json", r#"{"a": 1}"#);
        let glob = format!("{}/ok.json", tmp.path().display());
        run_json(&[glob]).expect("valid json");
    }

    #[test]
    fn json_parse_check_rejects_invalid_json() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "bad.json", "{not json");
        let glob = format!("{}/bad.json", tmp.path().display());
        assert!(run_json(&[glob]).is_err());
    }

    #[test]
    fn yaml_parse_check_accepts_valid_yaml() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "ok.yaml", "key: value\n");
        let glob = format!("{}/ok.yaml", tmp.path().display());
        run_yaml(&[glob]).expect("valid yaml");
    }

    #[test]
    fn yaml_parse_check_rejects_invalid_yaml() {
        let tmp = TempDir::new().expect("tmpdir");
        write_fixture(tmp.path(), "bad.yaml", "key: [unclosed\n");
        let glob = format!("{}/bad.yaml", tmp.path().display());
        assert!(run_yaml(&[glob]).is_err());
    }

    #[test]
    fn expand_globs_invalid_pattern_errors() {
        let err = expand_globs(&["***invalid***".to_string()]);
        assert!(err.is_err());
    }
}
