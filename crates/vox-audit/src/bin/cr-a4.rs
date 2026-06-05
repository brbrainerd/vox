//! CR-A4 lifecycle metadata parity sweep.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.8
//! and v1-release-criteria CR-A4: "All orchestration contracts that affect
//! model routing/providers must declare lifecycle metadata
//! (`experimental`/`stable`/`deprecated`) and a migration window, with CI
//! parity checks."
//!
//! What this sweep does:
//!
//!   1. Enumerate every `.yaml` / `.toml` file under `contracts/orchestration/`.
//!   2. For each, check whether the top-level body declares a `lifecycle:`
//!      (or `stability:`) key with one of {experimental, stable, deprecated}
//!      AND a `migration_window:` key (only required for `deprecated`).
//!   3. Emit `contracts/reports/arch/cr-a4/<UTC>.json` listing
//!      `contracts_missing_lifecycle`. Exits non-zero if non-empty.

use serde_json::json;

fn main() {
    let workspace = vox_audit::workspace_root();
    let orchestration_dir = workspace.join("contracts").join("orchestration");
    if !orchestration_dir.is_dir() {
        eprintln!("CR-A4: no contracts/orchestration/ directory; nothing to audit");
        std::process::exit(2);
    }

    let mut total: u32 = 0;
    let mut with_lifecycle: Vec<String> = Vec::new();
    let mut missing_lifecycle: Vec<MissingLifecycle> = Vec::new();
    let mut missing_migration_window: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(&orchestration_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !p.is_file() {
            continue;
        }
        if !matches!(ext, "yaml" | "yml" | "toml") {
            continue;
        }
        total += 1;
        let Ok(body) = std::fs::read_to_string(p) else {
            continue;
        };
        let rel = p
            .strip_prefix(&workspace)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        match check_lifecycle(&body) {
            LifecycleCheck::Present {
                value,
                has_migration_window,
            } => {
                with_lifecycle.push(rel.clone());
                if value == "deprecated" && !has_migration_window {
                    missing_migration_window.push(rel);
                }
            }
            LifecycleCheck::Missing => {
                missing_lifecycle.push(MissingLifecycle { path: rel });
            }
        }
    }

    let met = missing_lifecycle.is_empty() && missing_migration_window.is_empty();
    let coverage_pct = if total == 0 {
        0.0
    } else {
        100.0 * (with_lifecycle.len() as f64) / f64::from(total)
    };

    eprintln!(
        "CR-A4: {} of {} orchestration contracts declare lifecycle metadata ({coverage_pct:.1}%)",
        with_lifecycle.len(),
        total
    );
    if !missing_lifecycle.is_empty() {
        eprintln!(
            "CR-A4: {} contract(s) missing lifecycle metadata:",
            missing_lifecycle.len()
        );
        for m in missing_lifecycle.iter().take(20) {
            eprintln!("  - {}", m.path);
        }
    }
    if !missing_migration_window.is_empty() {
        eprintln!(
            "CR-A4: {} deprecated contract(s) missing migration_window:",
            missing_migration_window.len()
        );
        for m in &missing_migration_window {
            eprintln!("  - {m}");
        }
    }

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-A4",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "orchestration_dir": orchestration_dir.display().to_string(),
        "total_contracts": total,
        "with_lifecycle": with_lifecycle,
        "missing_metadata": missing_lifecycle,
        "deprecated_missing_migration_window": missing_migration_window,
        "coverage_pct": coverage_pct,
        "threshold": {
            "target_coverage_pct": 100.0,
            "met": met,
        },
    });
    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("arch")
        .join("cr-a4");
    std::fs::create_dir_all(&out_dir).expect("create cr-a4 dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

#[derive(serde::Serialize)]
struct MissingLifecycle {
    path: String,
}

enum LifecycleCheck {
    Present {
        value: String,
        has_migration_window: bool,
    },
    Missing,
}

/// Search the contract body for a `lifecycle:` / `stability:` top-level key
/// (yaml) or `[*] lifecycle = "..."` (toml). Accepts both flat YAML
/// (`lifecycle: stable`) and nested YAML
/// (`lifecycle:\n  maturity: stable`) shapes. Accepts values
/// {experimental, stable, deprecated}.
fn check_lifecycle(body: &str) -> LifecycleCheck {
    let lines: Vec<&str> = body.lines().collect();
    let mut found_value: Option<String> = None;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        for key in ["lifecycle:", "stability:"] {
            if let Some(rest) = trimmed.strip_prefix(key) {
                let raw = rest
                    .trim()
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .to_string();
                let lower = raw.to_ascii_lowercase();
                if matches!(lower.as_str(), "experimental" | "stable" | "deprecated") {
                    found_value = Some(lower);
                    break;
                }
                // Nested form: `lifecycle:` followed by a child key like
                // `maturity:`/`level:`/`status:` on a subsequent line.
                // Peek at the next 6 lines (covers reasonable indent +
                // intervening blank lines).
                if raw.is_empty() {
                    let base_indent = line.len() - trimmed.len();
                    let end = lines.len().min(idx + 7);
                    for &nl in &lines[(idx + 1)..end] {
                        let nt = nl.trim_start();
                        let nl_indent = nl.len() - nt.len();
                        if nt.is_empty() {
                            continue;
                        }
                        // Must be indented deeper than the parent key to
                        // count as a child; if dedented, stop scanning.
                        if nl_indent <= base_indent {
                            break;
                        }
                        for child_key in ["maturity:", "level:", "status:"] {
                            if let Some(child_rest) = nt.strip_prefix(child_key) {
                                let craw = child_rest
                                    .trim()
                                    .trim_start_matches('"')
                                    .trim_end_matches('"')
                                    .to_string();
                                let clower = craw.to_ascii_lowercase();
                                if matches!(
                                    clower.as_str(),
                                    "experimental" | "stable" | "deprecated"
                                ) {
                                    found_value = Some(clower);
                                    break;
                                }
                            }
                        }
                        if found_value.is_some() {
                            break;
                        }
                    }
                    if found_value.is_some() {
                        break;
                    }
                }
            }
        }
        // toml form: `lifecycle = "stable"` (table-scoped)
        if (trimmed.starts_with("lifecycle =") || trimmed.starts_with("stability ="))
            && let Some(eq) = trimmed.find('=')
        {
            let raw = trimmed[eq + 1..]
                .trim()
                .trim_start_matches('"')
                .trim_end_matches('"')
                .to_string();
            let lower = raw.to_ascii_lowercase();
            if matches!(lower.as_str(), "experimental" | "stable" | "deprecated") {
                found_value = Some(lower);
            }
        }
    }
    let Some(value) = found_value else {
        return LifecycleCheck::Missing;
    };
    let has_migration_window = body.contains("migration_window")
        || body.contains("migration-window")
        || body.contains("migrationWindow");
    LifecycleCheck::Present {
        value,
        has_migration_window,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_lifecycle_stable() {
        let s = "name: foo\nlifecycle: stable\n";
        assert!(
            matches!(check_lifecycle(s), LifecycleCheck::Present { ref value, .. } if value == "stable")
        );
    }

    #[test]
    fn yaml_quoted_lifecycle() {
        let s = "lifecycle: \"experimental\"\n";
        assert!(
            matches!(check_lifecycle(s), LifecycleCheck::Present { ref value, .. } if value == "experimental")
        );
    }

    #[test]
    fn no_lifecycle_is_missing() {
        let s = "name: foo\nkind: routing\n";
        assert!(matches!(check_lifecycle(s), LifecycleCheck::Missing));
    }

    #[test]
    fn deprecated_needs_migration_window() {
        let s = "lifecycle: deprecated\n";
        let r = check_lifecycle(s);
        match r {
            LifecycleCheck::Present {
                value,
                has_migration_window,
            } => {
                assert_eq!(value, "deprecated");
                assert!(!has_migration_window);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn nested_yaml_lifecycle_maturity() {
        let s = "lifecycle:\n  maturity: \"experimental\"\n  migration_window_days: 90\n";
        let r = check_lifecycle(s);
        assert!(
            matches!(r, LifecycleCheck::Present { ref value, has_migration_window: true } if value == "experimental"),
            "nested form should be recognized"
        );
    }

    #[test]
    fn nested_yaml_lifecycle_level() {
        let s = "lifecycle:\n  level: stable\n";
        assert!(matches!(
            check_lifecycle(s),
            LifecycleCheck::Present { ref value, .. } if value == "stable"
        ));
    }

    #[test]
    fn deprecated_with_migration_window() {
        let s = "lifecycle: deprecated\nmigration_window: 90d\n";
        let r = check_lifecycle(s);
        assert!(
            matches!(r, LifecycleCheck::Present { ref value, has_migration_window: true } if value == "deprecated")
        );
    }
}
