//! Compare `cargo llvm-cov report --json --summary-only` output to `.config/coverage-gates.toml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value;

use vox_cli_ci::cmd_enums::CoverageGateMode;
use super::repo_root;
use vox_bounded_fs::read_utf8_path_capped;

#[derive(Debug, Deserialize, Default)]
struct CoverageGatesFile {
    workspace_min_lines_percent: Option<f64>,
    #[serde(default)]
    crates: BTreeMap<String, f64>,
}

/// `vox ci coverage-gates …`
pub(crate) fn run(
    summary_json: PathBuf,
    mode: CoverageGateMode,
    config_path: PathBuf,
) -> Result<()> {
    let root = repo_root();
    let raw = read_utf8_path_capped(&summary_json)
        .with_context(|| format!("read {}", summary_json.display()))?;
    let v: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parse {} as llvm-cov JSON", summary_json.display()))?;

    let cfg_path = if config_path.is_absolute() {
        config_path
    } else {
        root.join(&config_path)
    };
    let cfg_text = read_utf8_path_capped(&cfg_path)
        .with_context(|| format!("read coverage gates config {}", cfg_path.display()))?;
    let cfg: CoverageGatesFile =
        toml::from_str(&cfg_text).with_context(|| format!("parse {}", cfg_path.display()))?;

    let mut violations: Vec<String> = Vec::new();

    if let Some(min) = cfg.workspace_min_lines_percent {
        let pct = workspace_lines_percent(&v).ok_or_else(|| {
            anyhow!(
                "could not compute workspace line coverage from {}",
                summary_json.display()
            )
        })?;
        if pct + f64::EPSILON < min {
            violations.push(format!(
                "workspace line coverage {:.2}% is below configured minimum {:.2}%",
                pct, min
            ));
        } else {
            println!(
                "workspace line coverage {:.2}% (minimum {:.2}%) — OK",
                pct, min
            );
        }
    }

    for (crate_name, min) in &cfg.crates {
        match crate_lines_percent(&v, crate_name) {
            Some(pct) if pct + f64::EPSILON < *min => {
                violations.push(format!(
                    "crate {crate_name} line coverage {:.2}% is below configured minimum {:.2}%",
                    pct, min
                ));
            }
            Some(pct) => {
                println!(
                    "crate {crate_name} line coverage {:.2}% (minimum {:.2}%) — OK",
                    pct, min
                );
            }
            None => violations.push(format!(
                "crate {crate_name}: no matching source files in coverage report (thresholds require data)"
            )),
        }
    }

    if cfg.workspace_min_lines_percent.is_none() && cfg.crates.is_empty() {
        println!(
            "coverage-gates: no thresholds in {}; skipped policy checks (report parsed OK)",
            cfg_path.display()
        );
    }

    if violations.is_empty() {
        return Ok(());
    }

    for msg in &violations {
        eprintln!("coverage-gates: {msg}");
    }

    match mode {
        CoverageGateMode::Warn => Ok(()),
        CoverageGateMode::Enforce => Err(anyhow!(
            "coverage-gates: {} threshold violation(s) (see stderr)",
            violations.len()
        )),
    }
}

fn workspace_lines_percent(root: &Value) -> Option<f64> {
    let data = root.get("data")?.as_array()?;
    let mut covered: u64 = 0;
    let mut count: u64 = 0;
    for block in data {
        let lines = block.get("totals")?.get("lines")?;
        covered = covered.checked_add(lines.get("covered")?.as_u64()?)?;
        count = count.checked_add(lines.get("count")?.as_u64()?)?;
    }
    if count == 0 {
        return None;
    }
    Some(100.0 * covered as f64 / count as f64)
}

fn crate_lines_percent(root: &Value, crate_name: &str) -> Option<f64> {
    let rel_dir = PathBuf::from("crates").join(crate_name);
    let data = root.get("data")?.as_array()?;
    let mut covered: u64 = 0;
    let mut count: u64 = 0;
    for block in data {
        let files = block.get("files")?.as_array()?;
        for f in files {
            let filename = f.get("filename")?.as_str()?;
            if path_matches_crate(filename, &rel_dir) {
                let lines = f.get("summary")?.get("lines")?;
                covered = covered.checked_add(lines.get("covered")?.as_u64()?)?;
                count = count.checked_add(lines.get("count")?.as_u64()?)?;
            }
        }
    }
    if count == 0 {
        return None;
    }
    Some(100.0 * covered as f64 / count as f64)
}

/// Match `crates/<crate>/` in normalized paths (Windows extended-length prefixes tolerated).
fn path_matches_crate(filename: &str, rel_dir: &Path) -> bool {
    let mut s = filename.replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    let needle = format!("{}/", rel_dir.to_string_lossy().replace('\\', "/"));
    s.contains(&needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_percent_sums_totals_blocks() {
        let j = r#"{"data":[
            {"files":[],"totals":{"lines":{"count":10,"covered":3,"percent":30.0}}},
            {"files":[],"totals":{"lines":{"count":10,"covered":5,"percent":50.0}}}
        ]}"#;
        let v: Value = serde_json::from_str(j).unwrap();
        let p = workspace_lines_percent(&v).unwrap();
        assert!((p - 40.0).abs() < 0.01);
    }

    #[test]
    fn crate_percent_filters_paths() {
        let j = r#"{"data":[{"files":[
            {"filename":"C:/vox/crates/vox-compiler/src/lib.rs","summary":{"lines":{"count":10,"covered":8,"percent":80.0}}},
            {"filename":"C:/vox/crates/vox-cli/src/lib.rs","summary":{"lines":{"count":10,"covered":1,"percent":10.0}}}
        ],"totals":{"lines":{"count":20,"covered":9,"percent":0}}}]}"#;
        let v: Value = serde_json::from_str(j).unwrap();
        let p = crate_lines_percent(&v, "vox-compiler").unwrap();
        assert!((p - 80.0).abs() < 0.01);
    }
}
