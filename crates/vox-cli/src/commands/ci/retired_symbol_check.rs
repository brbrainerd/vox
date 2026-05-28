use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
struct SymbolPolicy {
    #[serde(rename = "schema_version", default)]
    _schema_version: String,
    symbols: Vec<RetiredSymbol>,
}

#[derive(Deserialize, Debug)]
struct RetiredSymbol {
    id: String,
    pattern: String,
    replacement: String,
    rationale: String,
}

#[derive(Clone, Copy)]
struct ScanCfg {
    is_md: bool,
    /// Skip markdown table rows (`| ... |`) — policy files intentionally list retired tokens.
    skip_md_table_rows: bool,
    /// Rust sources: skip comment-only lines (full-file scan is opt-in via env).
    is_rust: bool,
}

fn should_skip_rust_line(line: &str) -> bool {
    let t = line.trim_start();
    if t.is_empty() {
        return true;
    }
    if t.starts_with("//") || t.starts_with("#![") {
        return true;
    }
    if t.starts_with('*') && !t.starts_with("*/") {
        return true;
    }
    false
}

/// Docs that catalog codebase evolution (audits, findings, migration plans,
/// dated snapshots, design specs) intentionally name retired symbols while
/// explaining what replaced them. Treat these as documentation-of-history
/// surfaces, not as user-facing guidance, and skip the policy check for them.
///
/// This is a principled carve-out: anything under `docs/src/architecture/` that
/// is date-stamped or matches a known history-doc suffix, plus the entire
/// `history/` subtree and the `docs/superpowers/{specs,plans}/` design-doc
/// subtrees, qualifies.
fn is_historical_or_audit_doc(rel_path: &Path) -> bool {
    let s = rel_path.to_string_lossy().replace('\\', "/");
    let name = rel_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Design specs, master plans, follow-ups — these document migrations.
    if s.starts_with("docs/superpowers/specs/") || s.starts_with("docs/superpowers/plans/") {
        return true;
    }

    if s.starts_with("docs/src/architecture/") {
        // history/ subtree is by definition historical.
        if s.starts_with("docs/src/architecture/history/") {
            return true;
        }
        // Date-stamped architectural snapshots like `2026-05-08-workspace-reorg-*.md`.
        if name.starts_with("2026-") || name.starts_with("2025-") {
            return true;
        }
        // Known history-doc suffix patterns.
        const HISTORY_SUFFIXES: &[&str] = &[
            "-findings-2026.md",
            "-audit-2026.md",
            "-audit-2026-05-15.md",
            "-audit-and-plan-2026.md",
            "-backlog-2026.md",
            "-research-2026.md",
            "-redesign-2026.md",
            "-classification-2026.md",
            "-classification-2026-05-08.md",
            "-coverage-2026.md",
            "-ssot-2026.md",
            "-convergence-2026.md",
            "-fate-plan-2026-05-08.md",
            "-implementation-plan-2026.md",
        ];
        if HISTORY_SUFFIXES.iter().any(|sfx| name.ends_with(sfx)) {
            return true;
        }
        // Specific named history/criteria docs.
        if matches!(name, "v1-release-criteria.md" | "build-time-log.md") {
            return true;
        }
    }

    // populi-quickstart.md explains the vox-ml-cli → vox-populi rename — a one-time
    // migration footnote, not active guidance to a retired name.
    if s == "docs/src/how-to/populi-quickstart.md" {
        return true;
    }

    false
}

fn scan_source_lines(
    path: &Path,
    root: &Path,
    body: &str,
    regexes: &[(&RetiredSymbol, Regex)],
    cfg: ScanCfg,
) -> Vec<String> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let rel_path = path.strip_prefix(root).unwrap_or(path);
    let is_history_doc = cfg.is_md && is_historical_or_audit_doc(rel_path);
    let mut failures = Vec::new();
    let mut in_frontmatter = false;
    let mut frontmatter_closed = false;
    let mut in_fence = false;

    for (line_idx, line) in body.lines().enumerate() {
        if cfg.skip_md_table_rows && line.trim_start().starts_with('|') {
            continue;
        }
        if cfg.is_rust && should_skip_rust_line(line) {
            continue;
        }

        if cfg.is_md {
            let t = line.trim();
            if t.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if !frontmatter_closed && t == "---" {
                in_frontmatter = !in_frontmatter;
                if !in_frontmatter {
                    frontmatter_closed = true;
                }
                continue;
            }
            if in_frontmatter || in_fence {
                continue;
            }
        }

        for (sym, re) in regexes {
            if !re.is_match(line) {
                continue;
            }

            if line.contains("DEPRECATED")
                || line.contains("Historical note")
                || line.contains("ARCHIVED")
            {
                continue;
            }

            if filename.contains("-ARCHIVED.md") {
                continue;
            }

            // Documents that catalog codebase evolution (audits, migration
            // plans, dated architectural snapshots) intentionally mention
            // retired symbols while explaining what replaced them. Skip the
            // policy check for those whole files.
            if is_history_doc {
                continue;
            }

            if matches!(
                sym.id.as_str(),
                "turso-url-env" | "turso-token-env" | "vox-turso-url-env" | "vox-turso-token-env"
            ) && (filename == "env-vars.md" || filename == "secrets-ssot.md")
            {
                continue;
            }

            if sym.id == "vox-dei-old-crate"
                && (line.contains("crates/vox-dei") || line.contains("crates\\vox-dei"))
            {
                continue;
            }

            if sym.id == "vox-dei-old-crate" && line.contains("vox-dei-d") {
                continue;
            }

            // `vox-dei-shim` is the current HITL crate — not the retired large
            // orchestrator. Skip lines that mention only the shim.
            if sym.id == "vox-dei-old-crate" && line.contains("vox-dei-shim") {
                continue;
            }

            if sym.id == "vox-dei-old-crate"
                && (line.contains("no-vox-dei-import") || line.contains("no_vox_dei_import"))
            {
                continue;
            }

            if sym.id == "vox-dei-old-crate" && line.to_lowercase().contains("retired") {
                continue;
            }

            if sym.id == "vox-ml-cli-standalone" && line.contains("vox-ml-cli-") {
                continue;
            }

            if sym.id == "vox-ml-cli-standalone"
                && (line.contains("crates/vox-ml-cli")
                    || line.contains("crates\\vox-ml-cli")
                    || line.contains(r"crates\vox-ml-cli"))
            {
                continue;
            }

            if sym.id == "vox-ml-cli-standalone" {
                let plan_snapshot = filename.starts_with("2026-05-08-crate-org-followup")
                    || filename == "2026-05-08-naming-and-guards-design.md"
                    || filename == "cli.md"
                    || filename == "repo-cleanup-ledger-2026.md";
                if plan_snapshot {
                    continue;
                }
            }

            // Canonical naming SSOT documents retired ↔ canonical mappings verbatim.
            if sym.id == "vox-ars-crate" && filename == "canonical-runtime-names.md" {
                continue;
            }

            failures.push(format!(
                "{}:{}: Found retired symbol '{}': Use {} instead. ({})",
                path.strip_prefix(root).unwrap_or(path).display(),
                line_idx + 1,
                sym.id,
                sym.replacement,
                sym.rationale
            ));
        }
    }

    failures
}

fn collect_crate_rs_files(crates_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(crates_dir) else {
        return;
    };
    for crate_entry in entries.flatten() {
        let p = crate_entry.path();
        if p.is_dir() {
            walk_rs_files_inner(&p, out);
        }
    }
}

fn walk_rs_files_inner(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if matches!(
                name,
                "target" | "tests" | "benches" | "snapshots" | "fixtures" | ".git"
            ) {
                continue;
            }
            walk_rs_files_inner(&p, out);
        } else if p.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

fn collect_cursor_rule_files(rules_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(rules_dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_cursor_rule_files(&p, out);
        } else if p.extension().and_then(|ext| ext.to_str()) == Some("mdc") {
            if p.file_name().and_then(|n| n.to_str()) == Some("retired-surfaces.mdc") {
                continue;
            }
            out.push(p);
        }
    }
}

/// Enforce `contracts/documentation/retired-symbols.v1.yaml` across docs and agent-policy surfaces.
///
/// Rust sources under `crates/` are intentionally out of scope: many crates legitimately mention
/// retired names (guards, migrations, compatibility layers). Keep this check documentation-forward.
pub fn run(root: &Path) -> Result<()> {
    super::docs_deprecated_command_guard::verify(root)?;

    let policy_path = root.join("contracts/documentation/retired-symbols.v1.yaml");
    if !policy_path.exists() {
        return Err(anyhow!(
            "Policy file not found at {}",
            policy_path.display()
        ));
    }

    let content = fs::read_to_string(&policy_path)
        .with_context(|| format!("Failed to read {}", policy_path.display()))?;

    let policy: SymbolPolicy = serde_yaml::from_str(&content)
        .with_context(|| "Failed to parse retired-symbols.v1.yaml")?;

    let mut regexes: Vec<(&RetiredSymbol, Regex)> = Vec::new();
    for sym in &policy.symbols {
        let re = Regex::new(&sym.pattern)
            .with_context(|| format!("Invalid regex pattern for {}: {}", sym.id, sym.pattern))?;
        regexes.push((sym, re));
    }

    let mut failures = Vec::new();

    let docs_dir = root.join("docs");
    let mut dirs_to_visit = vec![docs_dir];
    while let Some(dir) = dirs_to_visit.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
            } else if path.extension().is_some_and(|e| e == "md" || e == "json") {
                let rel_display = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel_display.starts_with("docs/src/archive/") {
                    continue;
                }

                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if filename == "SUMMARY.md" || filename == "doc-inventory.json" {
                    continue;
                }
                if filename == "legacy-tombstone-remediation-ledger-2026.md" {
                    continue;
                }
                if filename.starts_with("2026-05-08-crate-org-followup") {
                    continue;
                }
                if let Ok(body) = fs::read_to_string(&path) {
                    let is_md = path.extension().and_then(|e| e.to_str()) == Some("md");
                    failures.extend(scan_source_lines(
                        &path,
                        root,
                        &body,
                        &regexes,
                        ScanCfg {
                            is_md,
                            skip_md_table_rows: false,
                            is_rust: false,
                        },
                    ));
                }
            }
        }
    }

    for extra in ["AGENTS.md", "GEMINI.md", "CLAUDE.md"] {
        let path = root.join(extra);
        if path.is_file() {
            let body = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            failures.extend(scan_source_lines(
                &path,
                root,
                &body,
                &regexes,
                ScanCfg {
                    is_md: true,
                    skip_md_table_rows: true,
                    is_rust: false,
                },
            ));
        }
    }

    let cursor_rules = root.join(".cursor/rules");
    if cursor_rules.is_dir() {
        let mut mdc_files = Vec::new();
        collect_cursor_rule_files(&cursor_rules, &mut mdc_files);
        for path in mdc_files {
            let body = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            failures.extend(scan_source_lines(
                &path,
                root,
                &body,
                &regexes,
                ScanCfg {
                    is_md: true,
                    skip_md_table_rows: true,
                    is_rust: false,
                },
            ));
        }
    }

    let scan_crates = std::env::var("VOX_CI_RETIRED_SYMBOL_SCAN_CRATES")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if scan_crates {
        eprintln!(
            "retired-symbol-check: scanning crates/**/*.rs (VOX_CI_RETIRED_SYMBOL_SCAN_CRATES is set)"
        );
        let crates_dir = root.join("crates");
        if crates_dir.is_dir() {
            let mut rs_files = Vec::new();
            collect_crate_rs_files(&crates_dir, &mut rs_files);
            for path in rs_files {
                let body = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                failures.extend(scan_source_lines(
                    &path,
                    root,
                    &body,
                    &regexes,
                    ScanCfg {
                        is_md: false,
                        skip_md_table_rows: false,
                        is_rust: true,
                    },
                ));
            }
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("{}", f);
        }
        let suffix = if scan_crates {
            "docs/, policy roots, .cursor/rules, and crates/**/*.rs"
        } else {
            "docs/, policy roots, and .cursor/rules"
        };
        return Err(anyhow!(
            "Found {} retired symbol violations in {}",
            failures.len(),
            suffix
        ));
    }

    println!("retired-symbol-check OK");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_skip_rust_line;

    #[test]
    fn rust_skip_skips_line_comments() {
        assert!(should_skip_rust_line("// vox-dei"));
        assert!(!should_skip_rust_line(r#"let _ = "vox-dei";"#));
    }
}
