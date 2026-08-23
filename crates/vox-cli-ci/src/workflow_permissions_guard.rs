//! Gate: every workflow declares an explicit top-level `permissions:` block.
//!
//! Without one a workflow inherits the repository default token scope. If that
//! default is the legacy "read and write all scopes", every job — including ones
//! compiling 1600+ third-party crates — carries a fully privileged token.

use anyhow::{Result, bail};
use std::path::Path;

/// The top-level `permissions:` value, or `None` when absent.
pub fn top_level_permissions(yml: &str) -> Option<serde_yaml::Value> {
    let v: serde_yaml::Value = serde_yaml::from_str(yml).ok()?;
    let p = v.get("permissions")?;
    (!p.is_null()).then(|| p.clone())
}

/// Check every workflow. In `strict` mode a missing block is an error.
pub fn run(root: &Path, strict: bool) -> Result<()> {
    let dir = root.join(".github/workflows");
    // A checkout without workflows is not a violation — `read_dir` on a missing
    // path is an Err, which would fail every pre-push in such a tree.
    if !dir.is_dir() {
        return Ok(());
    }
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("yml") && ext != Some("yaml") {
            continue;
        }
        if top_level_permissions(&std::fs::read_to_string(&path)?).is_none() {
            offenders.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    offenders.sort();
    if !offenders.is_empty() {
        let list = offenders.join(", ");
        if strict {
            bail!("workflows without an explicit top-level `permissions:` block: {list}");
        }
        eprintln!("warning: workflows without `permissions:`: {list}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_top_level_block() {
        assert!(top_level_permissions("on:\n  push:\npermissions:\n  contents: read\n").is_some());
    }

    #[test]
    fn a_job_level_block_does_not_count() {
        let yml = "jobs:\n  build:\n    permissions:\n      contents: read\n";
        assert!(top_level_permissions(yml).is_none());
    }

    /// The real assertion: top-level defaults to read, and `contents: write`
    /// appears on the publishing job and nowhere else.
    #[test]
    fn release_workflows_grant_write_only_where_needed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // release-gui's sole job both builds and uploads, so it needs write
        // until that job is split — see the plan's honesty table.
        for (wf, writer) in [
            ("release-binaries.yml", Some("publish")),
            ("release-gui.yml", Some("build-tauri")),
            ("release-installers.yml", None),
        ] {
            let text = std::fs::read_to_string(root.join(".github/workflows").join(wf))
                .unwrap_or_else(|e| panic!("read {wf}: {e}"));
            let v: serde_yaml::Value = serde_yaml::from_str(&text).expect("valid YAML");

            assert_eq!(
                v["permissions"]["contents"].as_str(),
                Some("read"),
                "{wf} top-level contents must be read"
            );
            for (name, job) in v["jobs"].as_mapping().expect("jobs mapping") {
                let writes = job["permissions"]["contents"].as_str() == Some("write");
                let should = writer == name.as_str();
                assert_eq!(
                    writes, should,
                    "{wf} job {name:?}: contents:write must appear on {writer:?} and nowhere else"
                );
            }
        }
    }
}
