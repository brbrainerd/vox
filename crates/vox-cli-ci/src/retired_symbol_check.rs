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
    /// Opt in to scanning `crates/**/*.rs` for this pattern on EVERY run,
    /// regardless of `VOX_CI_RETIRED_SYMBOL_SCAN_CRATES` (which is off by
    /// default in CI — a full-crate scan of every retired pattern currently
    /// surfaces ~96 pre-existing hits, almost all benign self-reference in
    /// the detectors/compat shims that implement or guard the retirement
    /// itself, not live regressions). Reserve this for entries that guard a
    /// security-critical regression (e.g. an auth/approval bypass) where the
    /// pattern is narrow enough not to false-positive.
    #[serde(default)]
    scan_rust_source: bool,
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

/// True if `name` contains an embedded `-YYYY-MM-DD` ISO date token (year 2025
/// or 2026) anywhere, not just as a leading filename prefix — e.g.
/// `vox-axis-harness-reliability-spec-plan-2026-07-02.md`. Requires a genuine
/// year/month/day triple (not a bare year) so this doesn't over-broaden to
/// filenames that merely mention "2026" once.
fn has_embedded_iso_date(name: &str) -> bool {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let parts: Vec<&str> = stem.split('-').collect();
    parts.windows(3).any(|w| {
        matches!(w[0], "2025" | "2026")
            && w[1].len() == 2
            && w[1].chars().all(|c| c.is_ascii_digit())
            && w[2].len() == 2
            && w[2].chars().all(|c| c.is_ascii_digit())
    })
}

/// Docs that catalog codebase evolution (audits, findings, migration plans,
/// dated snapshots, design specs) intentionally name retired symbols while
/// explaining what replaced them. Treat these as documentation-of-history
/// surfaces, not as user-facing guidance, and skip the policy check for them.
///
/// This is a principled carve-out: anything under `docs/src/architecture/` that
/// is date-stamped (leading or embedded) or matches a known history-doc suffix,
/// plus the entire `history/` subtree and the `docs/superpowers/{specs,plans}/`
/// design-doc subtrees, qualifies.
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
        // Date-stamped architectural snapshots like `2026-05-08-workspace-reorg-*.md`,
        // or a full ISO date embedded mid-filename like
        // `vox-axis-harness-reliability-spec-plan-2026-07-02.md`.
        if name.starts_with("2026-") || name.starts_with("2025-") || has_embedded_iso_date(name) {
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

/// For a markdown table row, return everything after the first data cell.
///
/// Policy files list the retired symbol in the first column on purpose, so that
/// cell is skipped — but the replacement column must stay scannable, because a
/// replacement that names a retired form is exactly the defect we are hunting.
///
/// `\|` inside a cell is escaped content, not a delimiter (the AGENTS.md
/// `@endpoint` row contains two), so it is masked before splitting.
fn first_cell_only(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let mut rest = trimmed.strip_prefix('|')?;
    let mut consumed = trimmed.len() - rest.len();
    loop {
        let i = rest.find('|')?;
        if rest[..i].ends_with('\\') {
            // An escaped pipe inside the cell (e.g. AGENTS.md's `@endpoint`
            // row) -- not a column delimiter. Keep scanning past it.
            consumed += i + 1;
            rest = &rest[i + 1..];
            continue;
        }
        return Some(&trimmed[consumed + i..]);
    }
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
    // Section-scoped carve-out: a "## Retired ..." / "### Historical" / "#### Superseded"
    // heading legitimately names retired symbols until the next heading at the
    // same or shallower level. This is narrower than `is_history_doc` (whole-file)
    // and catches the common case of a single Retired/Historical subsection inside
    // an otherwise-current, otherwise-prescriptive page -- e.g. a reference page's
    // "## Retired: `@endpoint`" migration note, or a roadmap's dated "Superseded"
    // callout -- without silencing the rest of the page.
    let mut in_retired_section = false;
    let mut retired_section_level = 0usize;

    for (line_idx, line) in body.lines().enumerate() {
        let line: &str = if cfg.skip_md_table_rows {
            first_cell_only(line).unwrap_or(line)
        } else {
            line
        };
        if cfg.is_rust && should_skip_rust_line(line) {
            continue;
        }

        if cfg.is_md {
            let t = line.trim();
            if !in_fence && t.starts_with('#') {
                let level = t.bytes().take_while(|b| *b == b'#').count();
                let heading = t[level..].trim();
                if in_retired_section && level <= retired_section_level {
                    in_retired_section = false;
                }
                let lower = heading.to_lowercase();
                if lower.starts_with("retired")
                    || lower.starts_with("historical")
                    || lower.starts_with("superseded")
                    || lower.contains("retired:")
                    || lower.contains("(retired)")
                    || lower.contains("(historical)")
                    || lower.contains("(superseded)")
                {
                    in_retired_section = true;
                    retired_section_level = level;
                }
            }
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
            if is_history_doc || in_retired_section {
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

            // A line already naming the correct replacement (lookup_fact_by_key) is
            // explaining the deprecation accurately, not recommending the retired
            // symbol -- don't flag it just for mentioning recall()/recall_async()
            // by name. (AGENTS.md/.cursor/rules/*.mdc state this identically but
            // never trip this check because their table rows are skipped via
            // skip_md_table_rows; docs/src/** files with the same row content
            // don't get that exemption, so this closes the gap explicitly.)
            if sym.id == "sync-recall-api" && line.contains("lookup_fact_by_key") {
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
    let walker = walkdir::WalkDir::new(crates_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !(e.file_type().is_dir()
                && matches!(
                    name,
                    "target" | "tests" | "benches" | "snapshots" | "fixtures" | ".git"
                ))
        });
    for entry in walker.filter_map(Result::ok) {
        let p = entry.path();
        if entry.file_type().is_file() && p.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(p.to_path_buf());
        }
    }
}

fn collect_cursor_rule_files(rules_dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in walkdir::WalkDir::new(rules_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if entry.file_type().is_file()
            && p.extension().and_then(|ext| ext.to_str()) == Some("mdc")
            && p.file_name().and_then(|n| n.to_str()) != Some("retired-surfaces.mdc")
        {
            out.push(p.to_path_buf());
        }
    }
}

/// Enforce `contracts/documentation/retired-symbols.v1.yaml` across docs and agent-policy surfaces.
///
/// Rust sources under `crates/` are intentionally out of scope: many crates legitimately mention
/// retired names (guards, migrations, compatibility layers). Keep this check documentation-forward.
pub fn run(root: &Path) -> Result<()> {
    crate::docs_deprecated_command_guard::run(root)?;

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
                // A changelog is a historical record by construction: every retired
                // symbol legitimately appears in the entry that introduced or removed
                // it, describing the tree as it stood at that release. This file is
                // synced verbatim from the repository-root CHANGELOG.md, which is
                // outside every scan root, so before the sync the same text was
                // simply invisible rather than compliant.
                if filename == "changelog.md" {
                    continue;
                }
                if filename.starts_with("2026-05-08-crate-org-followup") {
                    continue;
                }
                // GUI honesty-audit artifacts name retired symbols as *evidence of
                // what the audit found* in a surface, not as user-facing guidance —
                // the audits/findings carve-out applies (same as is_historical_or_audit_doc).
                if rel_display.starts_with("docs/agents/gui-honesty-") {
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
    let crates_dir = root.join("crates");
    if crates_dir.is_dir() {
        if scan_crates {
            // Opt-in full scan: every retired pattern against every crate source
            // file. Noisy on this repo today (~96 pre-existing hits, mostly
            // detector/compat self-reference) — not run in CI by default.
            eprintln!(
                "retired-symbol-check: scanning crates/**/*.rs (VOX_CI_RETIRED_SYMBOL_SCAN_CRATES is set)"
            );
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
        } else {
            // Always-on narrow scan: only patterns explicitly marked
            // `scan_rust_source: true` guard against a Rust-source regression,
            // so they run on every CI invocation without the opt-in noise.
            let always_on: Vec<(&RetiredSymbol, Regex)> = regexes
                .iter()
                .filter(|(sym, _)| sym.scan_rust_source)
                .map(|(sym, re)| (*sym, re.clone()))
                .collect();
            if !always_on.is_empty() {
                let mut rs_files = Vec::new();
                collect_crate_rs_files(&crates_dir, &mut rs_files);
                for path in rs_files {
                    let body = fs::read_to_string(&path)
                        .with_context(|| format!("Failed to read {}", path.display()))?;
                    failures.extend(scan_source_lines(
                        &path,
                        root,
                        &body,
                        &always_on,
                        ScanCfg {
                            is_md: false,
                            skip_md_table_rows: false,
                            is_rust: true,
                        },
                    ));
                }
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
    use super::{first_cell_only, should_skip_rust_line};

    #[test]
    fn rust_skip_skips_line_comments() {
        assert!(should_skip_rust_line("// vox-dei"));
        assert!(!should_skip_rust_line(r#"let _ = "vox-dei";"#));
    }

    #[test]
    fn first_cell_only_exposes_the_replacement_column() {
        // The real AGENTS.md row: escaped pipes inside the first cell must not
        // be treated as column separators.
        let row = r"| `@endpoint(kind: server\|query\|mutation) fn` (removed v0.6.0) | `server fn` / `query fn` / `mutation fn` |";
        let rest = first_cell_only(row).expect("table row");
        assert!(
            !rest.contains("@endpoint"),
            "the retired form lives in the first cell and must be skipped, got: {rest}"
        );
        assert!(
            rest.contains("server fn"),
            "the replacement column must remain scannable, got: {rest}"
        );
    }

    #[test]
    fn first_cell_only_returns_none_for_non_table_lines() {
        assert!(first_cell_only("plain prose line").is_none());
        assert!(first_cell_only("").is_none());
    }
}
