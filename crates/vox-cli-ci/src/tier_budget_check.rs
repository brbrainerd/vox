//! `vox ci tier-budget-check` — compare nextest JUnit elapsed time against
//! `contracts/budgets/test-tier-budgets.v1.yaml` without re-running tests.
//!
//! ## How it works
//!
//! Reads the `time` attribute of the `<testsuites>` root element in the nextest JUnit XML
//! artifact (seconds as a float), converts to milliseconds, then applies the same
//! `warn_ms` / `fail_ms` thresholds used by `vox ci pre-push --enforce-budgets`.
//!
//! This allows CI to enforce timing budgets **after** a completed nextest run without
//! paying the cost of a second full test run.
//!
//! ## Exit codes
//!
//! - **0**: elapsed ≤ `warn_ms`, or budgets file absent (no-op).
//! - **0** (with stderr warning): `warn_ms` < elapsed ≤ `fail_ms`.
//! - **1**: elapsed > `fail_ms`.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::Path;

/// Parse the `time` attribute from `<testsuites time="...">` in a nextest JUnit XML.
///
/// Returns the elapsed time in **milliseconds**. Nextest writes the value in seconds as a
/// float (e.g., `"42.318"`).
fn elapsed_ms_from_junit(junit_path: &Path) -> Result<u64> {
    let raw = fs::read_to_string(junit_path)
        .with_context(|| format!("read JUnit XML at {}", junit_path.display()))?;

    // We do a lightweight string scan rather than full XML parsing to avoid pulling in
    // an XML crate. The `<testsuites ... time="...">` attribute is always on the first
    // line in nextest output.
    for line in raw.lines().take(20) {
        // Match `time="<float>"` anywhere on the line.
        if let Some(idx) = line.find("time=\"") {
            let after = &line[idx + 6..];
            if let Some(end) = after.find('"') {
                let secs_str = &after[..end];
                let secs: f64 = secs_str
                    .parse()
                    .with_context(|| format!("parse JUnit time=\"{secs_str}\""))?;
                let ms = (secs * 1000.0).round() as u64;
                return Ok(ms);
            }
        }
    }

    Err(anyhow!(
        "could not find `time=\"...\"` attribute in the first 20 lines of {}",
        junit_path.display()
    ))
}

/// Map a profile name to the corresponding tier key in `test-tier-budgets.v1.yaml`.
///
/// Mirrors the mapping in `pre_push::tier_budget_key`.
fn tier_key(profile: &str) -> Option<&'static str> {
    match profile {
        "fast" => Some("fast"),
        "complete" => Some("complete"),
        "full" | "full+since" => Some("full"),
        "full+cov" | "full+cov+since" => Some("full_cov"),
        _ => None,
    }
}

/// Run the tier-budget-check from the repository root.
pub fn run(root: &Path, junit_path: &Path, profile: &str) -> Result<()> {
    let elapsed_ms = elapsed_ms_from_junit(junit_path)?;
    println!("tier-budget-check: JUnit elapsed = {elapsed_ms}ms (profile `{profile}`)");

    let budgets_path = root.join("contracts/budgets/test-tier-budgets.v1.yaml");
    if !budgets_path.exists() {
        println!(
            "tier-budget-check: no budgets file at {}; skipping.",
            budgets_path.display()
        );
        return Ok(());
    }

    let raw = fs::read_to_string(&budgets_path)
        .with_context(|| format!("read {}", budgets_path.display()))?;
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", budgets_path.display()))?;

    let Some(key) = tier_key(profile) else {
        println!("tier-budget-check: profile `{profile}` has no budget entry; skipping.");
        return Ok(());
    };

    let Some(tiers) = doc.get("tiers") else {
        println!("tier-budget-check: budgets file has no `tiers` key; skipping.");
        return Ok(());
    };

    let Some(tier) = tiers.get(key) else {
        println!("tier-budget-check: tier `{key}` not found in budgets file; skipping.");
        return Ok(());
    };

    let warn_ms = tier
        .get("warn_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX);
    let fail_ms = tier
        .get("fail_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX);

    if elapsed_ms > fail_ms {
        bail!(
            "tier-budget-check: profile `{profile}` elapsed {elapsed_ms}ms \
             > fail threshold {fail_ms}ms \
             (see contracts/budgets/test-tier-budgets.v1.yaml)"
        );
    }
    if elapsed_ms > warn_ms {
        eprintln!(
            "tier-budget-check: WARNING — profile `{profile}` elapsed {elapsed_ms}ms \
             > warn threshold {warn_ms}ms \
             (see contracts/budgets/test-tier-budgets.v1.yaml)"
        );
    } else {
        println!("tier-budget-check: OK — {elapsed_ms}ms ≤ warn threshold {warn_ms}ms");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_junit(dir: &Path, time_secs: &str) -> std::path::PathBuf {
        let path = dir.join("junit.xml");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="42" failures="0" errors="0" time="{time_secs}">
</testsuites>"#
        );
        std::fs::write(&path, xml).expect("write junit");
        path
    }

    fn write_budget_yaml(dir: &Path) {
        let budgets_dir = dir.join("contracts/budgets");
        std::fs::create_dir_all(&budgets_dir).expect("create dir");
        let yaml = "schema_version: 1\ntiers:\n  full:\n    measured_ms: 120000\n    warn_ms: 144000\n    fail_ms: 180000\n";
        std::fs::write(budgets_dir.join("test-tier-budgets.v1.yaml"), yaml).expect("write yaml");
    }

    #[test]
    fn parses_junit_time_secs() {
        let dir = tempdir().unwrap();
        let junit = write_junit(dir.path(), "42.318");
        let ms = elapsed_ms_from_junit(&junit).expect("parse");
        assert_eq!(ms, 42318);
    }

    #[test]
    fn junit_time_integer_secs() {
        let dir = tempdir().unwrap();
        let junit = write_junit(dir.path(), "100");
        let ms = elapsed_ms_from_junit(&junit).expect("parse");
        assert_eq!(ms, 100_000);
    }

    #[test]
    fn budget_check_under_warn_passes() {
        let dir = tempdir().unwrap();
        let junit = write_junit(dir.path(), "100"); // 100s = 100_000ms, under warn 144_000ms
        write_budget_yaml(dir.path());
        assert!(run(dir.path(), &junit, "full").is_ok());
    }

    #[test]
    fn budget_check_over_fail_errors() {
        let dir = tempdir().unwrap();
        let junit = write_junit(dir.path(), "200"); // 200s = 200_000ms, over fail 180_000ms
        write_budget_yaml(dir.path());
        assert!(run(dir.path(), &junit, "full").is_err());
    }

    #[test]
    fn missing_budgets_file_is_noop() {
        let dir = tempdir().unwrap();
        let junit = write_junit(dir.path(), "9999");
        // No contracts/budgets/ directory — must be a no-op.
        assert!(run(dir.path(), &junit, "full").is_ok());
    }

    #[test]
    fn tier_key_mapping() {
        assert_eq!(tier_key("fast"), Some("fast"));
        assert_eq!(tier_key("complete"), Some("complete"));
        assert_eq!(tier_key("full"), Some("full"));
        assert_eq!(tier_key("full+since"), Some("full"));
        assert_eq!(tier_key("full+cov"), Some("full_cov"));
        assert_eq!(tier_key("full+cov+since"), Some("full_cov"));
        assert_eq!(tier_key("bogus"), None);
    }
}
