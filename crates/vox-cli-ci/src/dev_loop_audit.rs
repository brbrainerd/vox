//! `vox ci dev-loop-audit` — heuristics for Cargo incremental-cache fragmentation and
//! expensive verification loops (common when AI agents spawn many shells).

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Run audit; when **`json`**, print [`DevLoopAuditV1`] JSON to stdout.
pub fn run(root: &Path, json: bool) -> Result<()> {
    let audit = gather(root);
    if json {
        println!("{}", serde_json::to_string_pretty(&audit)?);
    } else {
        print_human(&audit);
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DevLoopAuditV1 {
    pub schema_version: u32,
    pub repo_root_display: String,
    pub canonical_target_dir: String,
    pub cargo_target_dir_raw: Option<String>,
    pub effective_target_dir: String,
    pub fragmentation_risk: &'static str,
    pub warnings: Vec<String>,
    pub hints: Vec<String>,
}

fn gather(root: &Path) -> DevLoopAuditV1 {
    let canonical = root.join("target");
    let canonical_display = canonical.display().to_string();

    let (cargo_target_dir_raw, effective) = match std::env::var("CARGO_TARGET_DIR") {
        Ok(s) => {
            let p = PathBuf::from(&s);
            let eff = if p.is_absolute() { p } else { root.join(p) };
            (Some(s), eff)
        }
        Err(_) => (None, canonical.clone()),
    };
    let effective_display = effective.display().to_string();

    let mut warnings = Vec::new();
    let mut hints = Vec::new();

    let fragmentation_risk =
        classify_fragmentation_risk(root, &canonical, &effective, cargo_target_dir_raw.is_none());

    match fragmentation_risk {
        "none" => {
            if cargo_target_dir_raw.is_none() {
                hints.push(
                    "CARGO_TARGET_DIR unset — Cargo uses `.cargo/config.toml` unified `target/`."
                        .into(),
                );
            } else {
                hints.push(
                    "CARGO_TARGET_DIR matches the repo `target/` — shared incremental cache."
                        .into(),
                );
            }
        }
        "medium" => {
            warnings.push(format!(
                "CARGO_TARGET_DIR resolves to `{}`, not the canonical `{}`. \
Incremental artifacts do not align with default `target/` shells.",
                effective_display, canonical_display
            ));
            hints.push(
                "Use one target dir per task/session, or unset CARGO_TARGET_DIR for inner-loop edits."
                    .into(),
            );
        }
        "high" => {
            warnings.push(format!(
                "CARGO_TARGET_DIR `{}` is outside the repo root `{}` — expect repeated cold builds.",
                effective_display,
                root.display()
            ));
            hints.push(
                "Prefer repo-local `target/` (unset env var) unless you are deliberately isolating."
                    .into(),
            );
        }
        _ => {}
    }

    hints.push(
        "Inner loop: `cargo check -p <crate>` → `cargo nextest run -p <crate> --profile ci`; \
reserve `vox ci pre-push` for push readiness."
            .into(),
    );

    DevLoopAuditV1 {
        schema_version: 1,
        repo_root_display: root.display().to_string(),
        canonical_target_dir: canonical_display,
        cargo_target_dir_raw,
        effective_target_dir: effective_display,
        fragmentation_risk,
        warnings,
        hints,
    }
}

fn classify_fragmentation_risk(
    root: &Path,
    canonical: &Path,
    effective: &Path,
    cargo_target_dir_unset: bool,
) -> &'static str {
    if cargo_target_dir_unset {
        return "none";
    }
    if paths_equivalent(canonical, effective) {
        return "none";
    }
    if path_is_within_root(root, effective) {
        return "medium";
    }
    "high"
}

fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
    candidate.starts_with(root)
}

fn paths_equivalent(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

fn print_human(audit: &DevLoopAuditV1) {
    println!("dev-loop-audit (schema v{})", audit.schema_version);
    println!("  repo: {}", audit.repo_root_display);
    println!("  canonical target: {}", audit.canonical_target_dir);
    println!("  effective target: {}", audit.effective_target_dir);
    println!("  fragmentation_risk: {}", audit.fragmentation_risk);
    for w in &audit.warnings {
        eprintln!("  WARNING: {w}");
    }
    for h in &audit.hints {
        println!("  hint: {h}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_unset_is_none() {
        let root = Path::new("/workspace/vox");
        let canonical = root.join("target");
        assert_eq!(
            classify_fragmentation_risk(root, &canonical, &canonical, true),
            "none"
        );
    }

    #[test]
    fn classify_side_folder_under_repo_is_medium() {
        let root = Path::new("/workspace/vox");
        let canonical = root.join("target");
        let side = root.join("target-agent-ssot");
        assert_eq!(
            classify_fragmentation_risk(root, &canonical, &side, false),
            "medium"
        );
    }

    #[test]
    fn classify_outside_repo_is_high() {
        let root = Path::new("/workspace/vox");
        let canonical = root.join("target");
        let outside = std::env::temp_dir().join("rust-target");
        assert_eq!(
            classify_fragmentation_risk(root, &canonical, &outside, false),
            "high"
        );
    }
}
