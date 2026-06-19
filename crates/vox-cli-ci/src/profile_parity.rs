//! `vox ci profile-parity` — enforces the lean-CLI crate-count budget and
//! forbidden-crate list from contracts/reports/lean-cli-profile.v1.json.

use std::process::Command;

#[derive(serde::Deserialize)]
pub struct ProfileBudget {
    pub profile: String,
    pub max_crates: usize,
    pub forbidden_crates: Vec<String>,
}

/// Pure check: given the actual crate set and the budget, return violations.
pub fn check(actual: &[String], budget: &ProfileBudget) -> Vec<String> {
    let mut v = Vec::new();
    if actual.len() > budget.max_crates {
        v.push(format!(
            "{}: {} crates exceeds budget {}",
            budget.profile,
            actual.len(),
            budget.max_crates
        ));
    }
    for f in &budget.forbidden_crates {
        if actual.iter().any(|c| c == f) {
            v.push(format!(
                "{}: forbidden crate `{}` present in lean build",
                budget.profile, f
            ));
        }
    }
    v
}

fn lean_crate_set() -> anyhow::Result<Vec<String>> {
    // Determine available features by checking if script-execution exists.
    // Fall back to no features if it doesn't.
    let out = Command::new("cargo")
        .args([
            "tree",
            "-p",
            "vox-cli",
            "--no-default-features",
            "--features",
            "script-execution",
            "-e",
            "normal",
            "--prefix",
            "none",
        ])
        .output();

    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => {
            // Try without features
            let o = Command::new("cargo")
                .args([
                    "tree",
                    "-p",
                    "vox-cli",
                    "--no-default-features",
                    "-e",
                    "normal",
                    "--prefix",
                    "none",
                ])
                .output()?;
            anyhow::ensure!(o.status.success(), "cargo tree failed");
            o
        }
    };

    let text = String::from_utf8(out.stdout)?;
    let mut set: Vec<String> = text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    set.sort();
    set.dedup();
    Ok(set)
}

/// Entry point for `vox ci profile-parity`. Returns Err if any violation found.
pub fn run() -> anyhow::Result<()> {
    let budget_path = "contracts/reports/lean-cli-profile.v1.json";
    let raw = std::fs::read_to_string(budget_path)
        .map_err(|e| anyhow::anyhow!("reading {budget_path}: {e}"))?;
    let budget: ProfileBudget = serde_json::from_str(&raw)?;
    let actual = lean_crate_set()?;
    let violations = check(&actual, &budget);
    if violations.is_empty() {
        println!(
            "profile-parity OK: lean build = {} crates (budget {})",
            actual.len(),
            budget.max_crates
        );
        Ok(())
    } else {
        for v in &violations {
            eprintln!("PROFILE-PARITY VIOLATION: {v}");
        }
        anyhow::bail!("{} profile-parity violation(s)", violations.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> ProfileBudget {
        ProfileBudget {
            profile: "lean".into(),
            max_crates: 3,
            forbidden_crates: vec!["vox-gamify".into(), "vox-gui".into()],
        }
    }

    #[test]
    fn clean_profile_has_no_violations() {
        let actual = vec!["vox-cli".to_string(), "vox-ast".to_string()];
        assert!(check(&actual, &budget()).is_empty());
    }

    #[test]
    fn forbidden_crate_is_flagged() {
        let actual = vec!["vox-cli".to_string(), "vox-gamify".to_string()];
        let v = check(&actual, &budget());
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("vox-gamify"));
    }

    #[test]
    fn over_budget_is_flagged() {
        let actual = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let v = check(&actual, &budget());
        assert!(v.iter().any(|m| m.contains("exceeds budget")));
    }

    #[test]
    fn exact_budget_passes() {
        let actual = vec!["a".into(), "b".into(), "c".into()];
        assert!(check(&actual, &budget()).is_empty());
    }

    #[test]
    fn both_forbidden_and_over_budget_both_reported() {
        let actual = vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "vox-gamify".into(),
        ];
        let v = check(&actual, &budget());
        assert_eq!(v.len(), 2);
    }
}
