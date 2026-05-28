//! Workspace test inventory for `vox ci test-inventory` — regenerable JSON / Markdown artifacts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// CLI options for [`run`].
#[derive(Debug, Clone, Default)]
pub struct TestInventoryOpts {
    pub json_stdout: bool,
    pub output: Option<PathBuf>,
    pub markdown: Option<PathBuf>,
    pub check: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestInventoryReport {
    pub schema_version: u32,
    pub summary: SummaryCounts,
    #[serde(default)]
    pub crates: BTreeMap<String, CrateEntry>,
    #[serde(default)]
    pub rust_files_by_kind: BTreeMap<String, u64>,
    #[serde(default)]
    pub zero_test_crates: Vec<String>,
    #[serde(default)]
    pub inline_only_crates: Vec<String>,
    #[serde(default)]
    pub top_ignored_files: Vec<IgnoredFileRow>,
    #[serde(default)]
    pub caveats: CaveatsSection,
    #[serde(default)]
    pub golden_vox: GoldenVoxSection,
    #[serde(default)]
    pub app_e2e_tests: AppE2eSection,
    #[serde(default)]
    pub doctest_candidates: DoctestCandidateSection,
    #[serde(default)]
    pub test_file_patterns: TestPatternCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SummaryCounts {
    pub workspace_crate_count: u64,
    pub rust_files_scanned: u64,
    pub cargo_unit_test_functions: u64,
    pub cargo_integration_test_functions: u64,
    pub cargo_bench_functions: u64,
    pub cargo_ignored_test_functions: u64,
    pub cargo_test_functions_total: u64,
    pub webir_related_ignored_tests: u64,
    pub webir_ignored_tests_likely_retired_note: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CrateEntry {
    pub unit_tests: u64,
    pub integration_tests: u64,
    pub bench_tests: u64,
    pub ignored_tests: u64,
    pub has_integration_dir: bool,
    pub integration_rs_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IgnoredFileRow {
    pub path: String,
    pub ignored_tests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CaveatsSection {
    pub webir_classification: String,
    pub nextest_vs_doctest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GoldenVoxSection {
    pub golden_files: u64,
    pub at_test_decorators: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AppE2eSection {
    pub files: u64,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DoctestCandidateSection {
    pub src_files_with_rust_doc_fence: u64,
    pub rust_doc_fence_lines: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TestPatternCounts {
    pub sleep_sites: u64,
    pub env_var_reads: u64,
    pub env_var_mutations: u64,
    pub command_new: u64,
    pub serial_test: u64,
    pub proptest_colon_colon: u64,
    pub quickcheck_colon_colon: u64,
    pub insta_colon_colon: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustFileKind {
    Unit,
    Integration,
    Bench,
    Other,
}

fn repo_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn crates_member_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let crates_root = root.join("crates");
    let mut out = Vec::new();
    if !crates_root.is_dir() {
        return Ok(out);
    }
    for ent in
        fs::read_dir(&crates_root).with_context(|| format!("read_dir {}", crates_root.display()))?
    {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir() && p.join("Cargo.toml").is_file() {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

fn classify_rust_file(repo_rel_path: &str) -> RustFileKind {
    let norm = repo_rel_path.replace('\\', "/");
    if norm.contains("/benches/") {
        return RustFileKind::Bench;
    }
    if norm.contains("/tests/") {
        return RustFileKind::Integration;
    }
    if norm.starts_with("crates/") && norm.ends_with(".rs") && norm.contains("/src/") {
        return RustFileKind::Unit;
    }
    RustFileKind::Other
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map(|(a, _)| a).unwrap_or(line)
}

fn extract_ignore_reason(attr_line: &str) -> Option<&str> {
    let s = attr_line.trim();
    if !s.starts_with("#[ignore") {
        return None;
    }
    // `#[ignore = "reason"]` (Rust stable syntax)
    let key = "#[ignore = \"";
    let start = s.find(key).map(|p| p + key.len())?;
    let end = s[start..].find('"').map(|i| start + i)?;
    Some(&s[start..end])
}

fn is_bare_ignore_attr(attr_line: &str) -> bool {
    let t = strip_line_comment(attr_line).trim();
    if !t.starts_with("#[ignore") || !t.ends_with(']') {
        return false;
    }
    let rest = t
        .strip_prefix("#[ignore")
        .unwrap_or("")
        .trim_start_matches(|c: char| c.is_whitespace());
    rest.starts_with(']')
}

fn has_iso_date_marker(reason: &str) -> bool {
    let b = reason.as_bytes();
    if b.len() < 10 {
        return false;
    }
    for w in b.windows(10) {
        if w[4] == b'-'
            && w[7] == b'-'
            && w[..4].iter().all(|c| c.is_ascii_digit())
            && w[5..7].iter().all(|c| c.is_ascii_digit())
            && w[8..10].iter().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

/// True when an `#[ignore = "..."]` reason documents ownership or a sunset-style removal note.
pub fn ignore_reason_has_governance_marker(reason: &str) -> bool {
    if reason.trim().is_empty() {
        return false;
    }
    if looks_like_retired_webir_note(reason) {
        return true;
    }
    let lower = reason.to_ascii_lowercase();
    if lower.contains("owner:") || lower.contains("owner=") {
        return true;
    }
    if lower.contains("sunset")
        || lower.contains("remove-by")
        || lower.contains("remove by")
        || lower.contains("remove_by")
    {
        return true;
    }
    has_iso_date_marker(reason)
}

/// Why an ignored test failed the governance gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IgnoredTestGovernanceIssue {
    BareIgnore,
    MissingOwnershipMarker { reason: String },
}

/// One ignored test that needs a richer ignore reason for enforce-mode CI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IgnoredTestGovernanceFinding {
    pub path: String,
    pub line: usize,
    pub test_function: String,
    pub issue: IgnoredTestGovernanceIssue,
}

fn extract_rust_fn_name(raw_line: &str) -> Option<String> {
    let line = strip_line_comment(raw_line).trim();
    const PREFIXES: &[&str] = &[
        "async fn ",
        "pub async fn ",
        "pub(crate) async fn ",
        "pub(super) async fn ",
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "pub(super) fn ",
    ];
    for p in PREFIXES {
        if let Some(rest) = line.strip_prefix(p) {
            let name = rest
                .split(|c: char| c == '(' || c.is_whitespace())
                .next()
                .filter(|s| !s.is_empty())?;
            return Some(name.to_string());
        }
    }
    None
}

fn classify_ignored_test_issue(attrs: &[String]) -> Option<IgnoredTestGovernanceIssue> {
    let ignore_lines: Vec<&String> = attrs
        .iter()
        .filter(|a| strip_line_comment(a).trim().starts_with("#[ignore"))
        .collect();
    if ignore_lines.is_empty() {
        return None;
    }
    if ignore_lines.iter().any(|a| is_bare_ignore_attr(a)) {
        return Some(IgnoredTestGovernanceIssue::BareIgnore);
    }
    let reason = ignore_lines
        .iter()
        .find_map(|a| extract_ignore_reason(a).map(|s| s.to_string()));
    match reason {
        None => Some(IgnoredTestGovernanceIssue::BareIgnore),
        Some(r) if ignore_reason_has_governance_marker(&r) => None,
        Some(r) => Some(IgnoredTestGovernanceIssue::MissingOwnershipMarker { reason: r }),
    }
}

/// Histogram of ignored tests per repo-relative Rust path (same walk as governance scan).
pub fn scan_ignored_tests_per_file(root: &Path) -> Result<BTreeMap<String, u64>> {
    let (_total, _findings, per_file) = scan_ignored_test_governance_findings_with_histogram(root)?;
    Ok(per_file)
}

/// Internal: shared walk for ignored-test governance + per-file histogram.
pub(super) fn scan_ignored_test_governance_findings_with_histogram(
    root: &Path,
) -> Result<(
    u64,
    Vec<IgnoredTestGovernanceFinding>,
    BTreeMap<String, u64>,
)> {
    let crates_path = root.join("crates");
    if !crates_path.is_dir() {
        return Ok((0, Vec::new(), BTreeMap::new()));
    }

    let mut total_ignored = 0u64;
    let mut per_file: BTreeMap<String, u64> = BTreeMap::new();
    let mut findings: Vec<IgnoredTestGovernanceFinding> = Vec::new();

    for ent in WalkDir::new(&crates_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target"
        })
        .filter_map(Result::ok)
    {
        if !ent.file_type().is_file() {
            continue;
        }
        let path = ent.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let rel = repo_rel(root, path);
        let kind = classify_rust_file(&rel);
        if !matches!(
            kind,
            RustFileKind::Unit | RustFileKind::Integration | RustFileKind::Bench
        ) {
            continue;
        }

        let source =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut pending = Vec::<String>::new();

        for (idx, raw_line) in source.lines().enumerate() {
            let line_no = idx + 1;
            let line = strip_line_comment(raw_line).trim();
            if line.starts_with("#[") && line.ends_with(']') {
                pending.push(line.to_string());
                continue;
            }

            let is_fn = line.starts_with("async fn ")
                || line.starts_with("pub async fn ")
                || line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub(crate) fn ")
                || line.starts_with("pub(super) fn ");

            if !is_fn {
                if !line.is_empty() && !line.starts_with("#[") {
                    pending.clear();
                }
                continue;
            }

            let attrs = std::mem::take(&mut pending);
            let test_like = attrs.iter().any(|a| {
                let a = strip_line_comment(a).trim();
                a.starts_with("#[test]")
                    || a.starts_with("#[tokio::test")
                    || a.starts_with("#[rstest")
                    || a.starts_with("#[proptest")
                    || a.starts_with("#[bench]")
            });

            if !test_like {
                pending.clear();
                continue;
            }

            let ignored = attrs
                .iter()
                .any(|a| strip_line_comment(a).trim().starts_with("#[ignore"));
            if !ignored {
                pending.clear();
                continue;
            }

            total_ignored += 1;
            *per_file.entry(rel.clone()).or_insert(0) += 1;
            if let Some(issue) = classify_ignored_test_issue(&attrs) {
                let test_function =
                    extract_rust_fn_name(raw_line).unwrap_or_else(|| "<unknown>".to_string());
                findings.push(IgnoredTestGovernanceFinding {
                    path: rel.clone(),
                    line: line_no,
                    test_function,
                    issue,
                });
            }
            pending.clear();
        }
    }

    findings.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.test_function.cmp(&b.test_function))
    });

    Ok((total_ignored, findings, per_file))
}

/// Scan `crates/**/*.rs` for ignored tests and governance markers (wrapper).
pub fn scan_ignored_test_governance_findings(
    root: &Path,
) -> Result<(u64, Vec<IgnoredTestGovernanceFinding>)> {
    let (total, findings, _per_file) = scan_ignored_test_governance_findings_with_histogram(root)?;
    Ok((total, findings))
}

fn looks_like_retired_webir_note(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    lower.contains("tombstone")
        || lower.contains("retired")
        || lower.contains("parity drop")
        || lower.contains("removed parity")
}

fn looks_webir_related(reason: &str, file_path: &str) -> bool {
    let rl = reason.to_ascii_lowercase();
    let pl = file_path.to_ascii_lowercase();
    rl.contains("webir") || rl.contains("web_ir") || pl.contains("webir") || pl.contains("web_ir")
}

/// Scan Rust source for test-related attributes and patterns (best-effort line parser).
#[derive(Debug, Default, Clone)]
struct RustScan {
    unit_tests: u64,
    integration_tests: u64,
    bench_tests: u64,
    ignored_tests: u64,
    sleep_sites: u64,
    env_reads: u64,
    env_mutations: u64,
    command_new: u64,
    serial_test: u64,
    proptest: u64,
    quickcheck: u64,
    insta: u64,
    rust_doc_fences: u64,
}

fn count_substrings(hay: &str, needle: &str) -> u64 {
    hay.match_indices(needle).count() as u64
}

fn scan_rust_source(source: &str, kind: RustFileKind, _repo_path: &str) -> RustScan {
    let mut scan = RustScan::default();
    let mut pending = Vec::<String>::new();

    let count_test_patterns = matches!(
        kind,
        RustFileKind::Unit | RustFileKind::Integration | RustFileKind::Bench
    );

    for raw_line in source.lines() {
        let line = strip_line_comment(raw_line).trim();

        if count_test_patterns {
            if line.contains("thread::sleep") || line.contains("std::thread::sleep") {
                scan.sleep_sites += 1;
            }
            if line.contains("tokio::time::sleep") {
                scan.sleep_sites += 1;
            }
            if line.contains("std::env::var") || line.contains("env::var") {
                scan.env_reads += 1;
            }
            if line.contains("std::env::set_var")
                || line.contains("std::env::remove_var")
                || line.contains("env::set_var")
                || line.contains("env::remove_var")
            {
                scan.env_mutations += 1;
            }
            if line.contains("Command::new") {
                scan.command_new += 1;
            }
            if line.contains("serial_test") {
                scan.serial_test += 1;
            }
            scan.proptest += count_substrings(line, "proptest::");
            scan.quickcheck += count_substrings(line, "quickcheck::");
            scan.insta += count_substrings(line, "insta::");
        }

        if matches!(kind, RustFileKind::Unit)
            && line.contains("```")
            && (line.contains("rust") || line.contains("no_run"))
        {
            scan.rust_doc_fences += 1;
        }

        if line.starts_with("#[") && line.ends_with(']') {
            pending.push(line.to_string());
            continue;
        }

        let is_fn = line.starts_with("async fn ")
            || line.starts_with("pub async fn ")
            || line.starts_with("fn ")
            || line.starts_with("pub fn ")
            || line.starts_with("pub(crate) fn ")
            || line.starts_with("pub(super) fn ");

        if !is_fn {
            if !line.is_empty() && !line.starts_with("#[") {
                pending.clear();
            }
            continue;
        }

        let attrs = std::mem::take(&mut pending);
        let test_like = attrs.iter().any(|a| {
            a.starts_with("#[test]")
                || a.starts_with("#[tokio::test")
                || a.starts_with("#[rstest")
                || a.starts_with("#[proptest")
                || a.starts_with("#[bench]")
        });

        if test_like {
            let ignored = attrs.iter().any(|a| a.starts_with("#[ignore"));

            match kind {
                RustFileKind::Unit => scan.unit_tests += 1,
                RustFileKind::Integration => scan.integration_tests += 1,
                RustFileKind::Bench => scan.bench_tests += 1,
                RustFileKind::Other => {}
            }

            if ignored {
                scan.ignored_tests += 1;
            }
        }

        pending.clear();
    }

    scan
}

fn count_webir_ignore_metadata(source: &str, repo_path: &str) -> (u64, u64) {
    let mut active = 0u64;
    let mut retired_note = 0u64;
    let mut pending = Vec::<String>::new();

    for raw_line in source.lines() {
        let line = strip_line_comment(raw_line).trim();
        if line.starts_with("#[") && line.ends_with(']') {
            pending.push(line.to_string());
            continue;
        }
        let is_fn = line.starts_with("async fn ")
            || line.starts_with("pub async fn ")
            || line.starts_with("fn ")
            || line.starts_with("pub fn ")
            || line.starts_with("pub(crate) fn ")
            || line.starts_with("pub(super) fn ");
        if !is_fn {
            if !line.is_empty() && !line.starts_with("#[") {
                pending.clear();
            }
            continue;
        }
        let attrs = std::mem::take(&mut pending);
        let test_like = attrs.iter().any(|a| {
            a.starts_with("#[test]")
                || a.starts_with("#[tokio::test")
                || a.starts_with("#[rstest")
                || a.starts_with("#[proptest")
                || a.starts_with("#[bench]")
        });
        if test_like {
            let ignored = attrs.iter().any(|a| a.starts_with("#[ignore"));
            if ignored {
                let reason = attrs
                    .iter()
                    .find_map(|a| extract_ignore_reason(a).map(|s| s.to_string()))
                    .unwrap_or_default();
                if looks_webir_related(&reason, repo_path) {
                    if looks_like_retired_webir_note(&reason) {
                        retired_note += 1;
                    } else {
                        active += 1;
                    }
                }
            }
        }
        pending.clear();
    }
    (active, retired_note)
}

fn count_at_test_in_vox(source: &str) -> u64 {
    source
        .lines()
        .filter(|l| l.trim_start().starts_with("@test"))
        .count() as u64
}

fn collect_app_e2e_files(root: &Path) -> Result<Vec<String>> {
    let apps = root.join("apps");
    let mut paths = Vec::new();
    if !apps.is_dir() {
        return Ok(paths);
    }
    for ent in WalkDir::new(&apps).into_iter().filter_map(Result::ok) {
        if !ent.file_type().is_file() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        let lower = name.to_ascii_lowercase();
        let ok = lower.ends_with(".test.ts")
            || lower.ends_with(".test.tsx")
            || lower.ends_with(".test.js")
            || lower.ends_with(".test.jsx")
            || lower.ends_with(".spec.ts")
            || lower.ends_with(".spec.tsx")
            || lower.ends_with(".spec.js")
            || lower.ends_with(".spec.jsx")
            || lower.ends_with(".spec.mjs")
            || lower.ends_with(".test.mjs");
        if ok {
            paths.push(repo_rel(root, ent.path()));
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn collect_golden_vox(root: &Path) -> Result<(u64, u64)> {
    let golden = root.join("examples/golden");
    let mut files = 0u64;
    let mut at_tests = 0u64;
    if !golden.is_dir() {
        return Ok((files, at_tests));
    }
    for ent in WalkDir::new(&golden).into_iter().filter_map(Result::ok) {
        if !ent.file_type().is_file() {
            continue;
        }
        if ent.path().extension().and_then(|s| s.to_str()) != Some("vox") {
            continue;
        }
        files += 1;
        let content = fs::read_to_string(ent.path())?;
        at_tests += count_at_test_in_vox(&content);
    }
    Ok((files, at_tests))
}

/// Build a full inventory scan (deterministic: sorted maps / vecs, stable path separators).
pub fn build_inventory(root: &Path) -> Result<TestInventoryReport> {
    let crate_dirs = crates_member_dirs(root)?;
    let mut crates_map: BTreeMap<String, CrateEntry> = BTreeMap::new();
    for d in &crate_dirs {
        let name = d
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        crates_map.insert(
            name,
            CrateEntry {
                has_integration_dir: d.join("tests").is_dir(),
                ..Default::default()
            },
        );
    }

    let mut kinds_count: BTreeMap<String, u64> = BTreeMap::new();
    let mut ignored_by_file: BTreeMap<String, u64> = BTreeMap::new();
    let mut patterns = TestPatternCounts::default();
    let mut doctest_files = 0u64;
    let mut doctest_lines = 0u64;
    let mut webir_active = 0u64;
    let mut webir_retired = 0u64;

    let crates_path = root.join("crates");
    if crates_path.is_dir() {
        for ent in WalkDir::new(&crates_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target"
            })
            .filter_map(Result::ok)
        {
            if !ent.file_type().is_file() {
                continue;
            }
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let rel = repo_rel(root, path);
            let kind = classify_rust_file(&rel);
            let kind_key = match kind {
                RustFileKind::Unit => "unit_src",
                RustFileKind::Integration => "integration_tests",
                RustFileKind::Bench => "benches",
                RustFileKind::Other => "other",
            };
            *kinds_count.entry(kind_key.to_string()).or_insert(0) += 1;

            let source = fs::read_to_string(path)?;
            let scan = scan_rust_source(&source, kind, &rel);
            let (wa, wr) = count_webir_ignore_metadata(&source, &rel);
            webir_active += wa;
            webir_retired += wr;

            if matches!(kind, RustFileKind::Unit) && scan.rust_doc_fences > 0 {
                doctest_files += 1;
                doctest_lines += scan.rust_doc_fences;
            }

            if matches!(
                kind,
                RustFileKind::Unit | RustFileKind::Integration | RustFileKind::Bench
            ) {
                patterns.sleep_sites += scan.sleep_sites;
                patterns.env_var_reads += scan.env_reads;
                patterns.env_var_mutations += scan.env_mutations;
                patterns.command_new += scan.command_new;
                patterns.serial_test += scan.serial_test;
                patterns.proptest_colon_colon += scan.proptest;
                patterns.quickcheck_colon_colon += scan.quickcheck;
                patterns.insta_colon_colon += scan.insta;
            }

            if scan.ignored_tests > 0 {
                *ignored_by_file.entry(rel.clone()).or_insert(0) += scan.ignored_tests;
            }

            let crate_name = rel
                .strip_prefix("crates/")
                .and_then(|s| s.split('/').next())
                .map(|s| s.to_string());
            if let Some(cn) = crate_name {
                if let Some(entry) = crates_map.get_mut(&cn) {
                    entry.unit_tests += scan.unit_tests;
                    entry.integration_tests += scan.integration_tests;
                    entry.bench_tests += scan.bench_tests;
                    entry.ignored_tests += scan.ignored_tests;
                    if matches!(kind, RustFileKind::Integration) {
                        entry.integration_rs_files += 1;
                    }
                }
            }
        }
    }

    let mut zero_test: Vec<String> = Vec::new();
    let mut inline_only: Vec<String> = Vec::new();
    for (name, e) in &crates_map {
        let total = e.unit_tests + e.integration_tests + e.bench_tests;
        if total == 0 {
            zero_test.push(name.clone());
        } else if e.integration_tests == 0 && e.bench_tests == 0 && e.unit_tests > 0 {
            inline_only.push(name.clone());
        }
    }

    let mut top_ignored: Vec<IgnoredFileRow> = ignored_by_file
        .into_iter()
        .map(|(path, ignored_tests)| IgnoredFileRow {
            path,
            ignored_tests,
        })
        .collect();
    top_ignored.sort_by(|a, b| {
        b.ignored_tests
            .cmp(&a.ignored_tests)
            .then_with(|| a.path.cmp(&b.path))
    });
    top_ignored.truncate(25);

    let rust_files_scanned: u64 = kinds_count.values().copied().sum();

    let unit_sum: u64 = crates_map.values().map(|c| c.unit_tests).sum();
    let int_sum: u64 = crates_map.values().map(|c| c.integration_tests).sum();
    let bench_sum: u64 = crates_map.values().map(|c| c.bench_tests).sum();
    let ign_sum: u64 = crates_map.values().map(|c| c.ignored_tests).sum();

    let app_paths = collect_app_e2e_files(root)?;
    let app_files = app_paths.len() as u64;

    let (golden_files, golden_at) = collect_golden_vox(root)?;

    Ok(TestInventoryReport {
        schema_version: 1,
        summary: SummaryCounts {
            workspace_crate_count: crate_dirs.len() as u64,
            rust_files_scanned,
            cargo_unit_test_functions: unit_sum,
            cargo_integration_test_functions: int_sum,
            cargo_bench_functions: bench_sum,
            cargo_ignored_test_functions: ign_sum,
            cargo_test_functions_total: unit_sum + int_sum + bench_sum,
            webir_related_ignored_tests: webir_active,
            webir_ignored_tests_likely_retired_note: webir_retired,
        },
        crates: crates_map,
        rust_files_by_kind: kinds_count,
        zero_test_crates: zero_test,
        inline_only_crates: inline_only,
        top_ignored_files: top_ignored,
        caveats: CaveatsSection {
            webir_classification: "Ignored tests that mention WebIR are treated as active internal pipeline tests unless the ignore reason clearly indicates tombstone, retired, or dropped parity.".to_string(),
            nextest_vs_doctest: "`cargo nextest` runs compiled test binaries (unit/integration in crates) but does not execute `cargo test` doctests; this inventory tracks doctest candidates separately via ```rust / ```no_run fences in crate src files.".to_string(),
        },
        golden_vox: GoldenVoxSection {
            golden_files,
            at_test_decorators: golden_at,
        },
        app_e2e_tests: AppE2eSection {
            files: app_files,
            paths: app_paths,
        },
        doctest_candidates: DoctestCandidateSection {
            src_files_with_rust_doc_fence: doctest_files,
            rust_doc_fence_lines: doctest_lines,
        },
        test_file_patterns: patterns,
    })
}

fn report_to_json(report: &TestInventoryReport) -> Result<String> {
    Ok(serde_json::to_string_pretty(report)?)
}

fn emit_markdown(report: &TestInventoryReport) -> String {
    let mut s = String::new();
    s.push_str("---\n");
    s.push_str("title: \"Workspace test inventory (2026)\"\n");
    s.push_str("description: \"Regenerable counts of Rust tests, ignores, and related harness patterns across the workspace (fully regenerated; refresh dates via git history).\"\n");
    s.push_str("category: \"architecture\"\n");
    s.push_str("status: \"current\"\n");
    s.push_str("training_eligible: false\n");
    s.push_str("---\n\n");
    s.push_str("# Workspace test inventory\n\n");
    s.push_str("Regenerate this page with:\n\n");
    s.push_str("`cargo run -p vox-cli -- ci test-inventory --markdown docs/src/architecture/test-inventory-2026.md`\n\n");
    s.push_str("Machine-readable JSON:\n\n");
    s.push_str("`cargo run -p vox-cli -- ci test-inventory --json`\n\n");
    s.push_str("## Summary counts\n\n");
    s.push_str("| Metric | Value |\n");
    s.push_str("| --- | ---: |\n");
    let sum = &report.summary;
    s.push_str(&format!(
        "| Workspace crates (`crates/*/Cargo.toml`) | {} |\n",
        sum.workspace_crate_count
    ));
    s.push_str(&format!(
        "| Rust files under `crates/**/*.rs` | {} |\n",
        sum.rust_files_scanned
    ));
    s.push_str(&format!(
        "| Cargo unit tests (`#[test]` / `tokio::test` / `rstest` / `proptest` in `src/`) | {} |\n",
        sum.cargo_unit_test_functions
    ));
    s.push_str(&format!(
        "| Cargo integration tests (`crates/.../tests/`) | {} |\n",
        sum.cargo_integration_test_functions
    ));
    s.push_str(&format!(
        "| Cargo bench fns (`#[bench]` in scanned paths) | {} |\n",
        sum.cargo_bench_functions
    ));
    s.push_str(&format!(
        "| Ignored test functions (best-effort parse) | {} |\n",
        sum.cargo_ignored_test_functions
    ));
    s.push_str(&format!(
        "| Golden `.vox` files (`examples/golden/**/*.vox`) | {} |\n",
        report.golden_vox.golden_files
    ));
    s.push_str(&format!(
        "| `@test` lines in golden Vox | {} |\n",
        report.golden_vox.at_test_decorators
    ));
    s.push_str(&format!(
        "| App E2E-style files (`apps/**/*.test.*` / `*.spec.*`) | {} |\n",
        report.app_e2e_tests.files
    ));
    s.push_str(&format!(
        "| Doctest candidate src files (rust/no_run doc fences) | {} |\n",
        report.doctest_candidates.src_files_with_rust_doc_fence
    ));
    s.push_str(&format!(
        "| Doctest fence lines counted | {} |\n",
        report.doctest_candidates.rust_doc_fence_lines
    ));
    s.push('\n');

    s.push_str("### Test harness patterns (Rust files in unit/integration/bench paths)\n\n");
    let p = &report.test_file_patterns;
    s.push_str("| Pattern | Count |\n| --- | ---: |\n");
    s.push_str(&format!("| `sleep` sites | {} |\n", p.sleep_sites));
    s.push_str(&format!(
        "| Env reads (`env::var` / `std::env::var`) | {} |\n",
        p.env_var_reads
    ));
    s.push_str(&format!(
        "| Env mutations (`set_var` / `remove_var`) | {} |\n",
        p.env_var_mutations
    ));
    s.push_str(&format!("| `Command::new` | {} |\n", p.command_new));
    s.push_str(&format!("| `serial_test` | {} |\n", p.serial_test));
    s.push_str(&format!("| `proptest::` | {} |\n", p.proptest_colon_colon));
    s.push_str(&format!(
        "| `quickcheck::` | {} |\n",
        p.quickcheck_colon_colon
    ));
    s.push_str(&format!("| `insta::` | {} |\n", p.insta_colon_colon));
    s.push('\n');

    s.push_str("## Caveats\n\n");
    s.push_str(&format!(
        "- **WebIR / internal pipelines:** {}\n",
        report.caveats.webir_classification
    ));
    s.push_str(&format!(
        "- **Nextest vs doctests:** {}\n",
        report.caveats.nextest_vs_doctest
    ));
    s.push_str(&format!(
        "- **WebIR-related ignored tests (active heuristic):** {}\n",
        sum.webir_related_ignored_tests
    ));
    s.push_str(&format!(
        "- **WebIR ignores with retired/tombstone-style reasons:** {}\n",
        sum.webir_ignored_tests_likely_retired_note
    ));
    s.push('\n');

    s.push_str("## Zero-test crates\n\n");
    if report.zero_test_crates.is_empty() {
        s.push_str("(none)\n\n");
    } else {
        for c in &report.zero_test_crates {
            s.push_str(&format!("- `{}`\n", c));
        }
        s.push('\n');
    }

    s.push_str("## Top ignored files\n\n");
    if report.top_ignored_files.is_empty() {
        s.push_str("(none)\n\n");
    } else {
        s.push_str("| File | Ignored tests |\n| --- | ---: |\n");
        for row in &report.top_ignored_files {
            s.push_str(&format!("| `{}` | {} |\n", row.path, row.ignored_tests));
        }
        s.push('\n');
    }

    s.push_str("## Rust files by kind\n\n");
    for (k, v) in &report.rust_files_by_kind {
        s.push_str(&format!("- `{}`: {}\n", k, v));
    }
    s.push('\n');

    s
}

/// Run `vox ci test-inventory`.
pub fn run(root: &Path, opts: TestInventoryOpts) -> Result<()> {
    let report = build_inventory(root)?;
    let json = report_to_json(&report)?;

    if let Some(check_path) = opts.check.as_ref() {
        let committed = fs::read_to_string(check_path)
            .with_context(|| format!("read {}", check_path.display()))?;
        let left: TestInventoryReport = serde_json::from_str(&committed)
            .context("parse committed JSON as TestInventoryReport")?;
        if left != report {
            anyhow::bail!(
                "test-inventory JSON differs from `{}`; regenerate with `vox ci test-inventory --output <path>`",
                check_path.display()
            );
        }
        println!("test-inventory: OK (matches {})", check_path.display());
    }

    if let Some(out) = opts.output.as_ref() {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out, &json).with_context(|| format!("write {}", out.display()))?;
        println!("Wrote {}", out.display());
    }

    if let Some(md_path) = opts.markdown.as_ref() {
        let md = emit_markdown(&report);
        if let Some(parent) = md_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(md_path, md).with_context(|| format!("write {}", md_path.display()))?;
        println!("Wrote {}", md_path.display());
    }

    if opts.json_stdout {
        println!("{}", json);
    } else if opts.check.is_none() && opts.output.is_none() && opts.markdown.is_none() {
        let s = &report.summary;
        println!(
            "Workspace test inventory (schema v{})",
            report.schema_version
        );
        println!("  crates: {}", s.workspace_crate_count);
        println!(
            "  cargo tests (unit + integration + bench): {}",
            s.cargo_test_functions_total
        );
        println!(
            "  ignored tests (parsed): {}",
            s.cargo_ignored_test_functions
        );
        println!("  rust files scanned: {}", s.rust_files_scanned);
        println!(
            "  golden .vox / @test: {} / {}",
            report.golden_vox.golden_files, report.golden_vox.at_test_decorators
        );
        println!("  app E2E-style files: {}", report.app_e2e_tests.files);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_rust_paths() {
        assert_eq!(
            classify_rust_file("crates/foo/src/lib.rs"),
            RustFileKind::Unit
        );
        assert_eq!(
            classify_rust_file("crates/foo/tests/it.rs"),
            RustFileKind::Integration
        );
        assert_eq!(
            classify_rust_file("crates/foo/benches/x.rs"),
            RustFileKind::Bench
        );
    }

    #[test]
    fn extract_ignore_reason_parses() {
        let line = r#"#[ignore = "flaky web_ir lower"]"#;
        assert_eq!(extract_ignore_reason(line), Some("flaky web_ir lower"));
    }

    #[test]
    fn webir_retired_detection() {
        assert!(looks_like_retired_webir_note("tombstone parity"));
        assert!(!looks_like_retired_webir_note("web_ir pipeline"));
        assert!(looks_webir_related(
            "",
            "crates/vox-codegen/src/web_ir/foo.rs"
        ));
    }

    #[test]
    fn scan_counts_tests_and_ignores() {
        let src = concat!(
            "\n",
            "#[ignore = \"slow\"]\n",
            "#[test]\n",
            "fn a() {}\n\n",
            "#[tokio::test]\n",
            "async fn b() {}\n\n",
            "#[test]\n",
            "fn c() {}\n",
        );
        let scan = scan_rust_source(src, RustFileKind::Integration, "crates/x/tests/t.rs");
        assert_eq!(scan.integration_tests, 3);
        assert_eq!(scan.ignored_tests, 1);
    }

    #[test]
    fn count_at_test_decorators() {
        let v = "@test\nfn foo() {}\n  @test\n";
        assert_eq!(count_at_test_in_vox(v), 2);
    }

    #[test]
    fn inventory_minimal_workspace_deterministic() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        fs::create_dir_all(root.join("crates/alpha/src"))?;
        fs::create_dir_all(root.join("crates/alpha/tests"))?;
        fs::write(
            root.join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        fs::write(
            root.join("crates/alpha/src/lib.rs"),
            "pub fn x() {}\n#[cfg(test)] mod t {\n#[test]\nfn u() {}\n}\n",
        )?;
        fs::write(
            root.join("crates/alpha/tests/it.rs"),
            "#[test]\nfn i() { std::thread::sleep(std::time::Duration::from_millis(1)); }\n",
        )?;

        let r = build_inventory(root)?;
        assert_eq!(r.summary.workspace_crate_count, 1);
        assert_eq!(r.summary.cargo_unit_test_functions, 1);
        assert_eq!(r.summary.cargo_integration_test_functions, 1);
        assert!(r.test_file_patterns.sleep_sites >= 1);
        let j1 = report_to_json(&r)?;
        let j2 = report_to_json(&r)?;
        assert_eq!(j1, j2);
        Ok(())
    }

    #[test]
    fn check_mode_passes_identical_json() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        fs::create_dir_all(root.join("crates/z/src"))?;
        fs::write(
            root.join("crates/z/Cargo.toml"),
            "[package]\nname = \"z\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        fs::write(root.join("crates/z/src/lib.rs"), "// empty\n")?;

        let report = build_inventory(root)?;
        let json_path = root.join("inv.json");
        fs::write(&json_path, report_to_json(&report)?)?;

        run(
            root,
            TestInventoryOpts {
                check: Some(json_path.clone()),
                ..Default::default()
            },
        )?;
        Ok(())
    }

    #[test]
    fn parses_ignore_attribute_line() {
        assert_eq!(
            extract_ignore_reason("#[ignore = \"reason\"]"),
            Some("reason")
        );
        assert_eq!(extract_ignore_reason("#[ignore]"), None);
    }

    #[test]
    fn webir_ignore_counts_split_retired() {
        let src = concat!(
            "\n",
            "#[ignore = \"tombstone web_ir\"]\n",
            "#[test]\n",
            "fn a() {}\n\n",
            "#[ignore = \"web_ir pipeline\"]\n",
            "#[test]\n",
            "fn b() {}\n",
        );
        let (active, retired) = count_webir_ignore_metadata(src, "crates/foo/src/lib.rs");
        assert_eq!(retired, 1);
        assert_eq!(active, 1);
    }

    #[test]
    fn governance_marker_detects_owner_and_date() {
        assert!(ignore_reason_has_governance_marker(
            "owner: platform — flaky disk"
        ));
        assert!(ignore_reason_has_governance_marker("sunset 2026-12-01"));
        assert!(ignore_reason_has_governance_marker("remove by 2030-01-01"));
        assert!(!ignore_reason_has_governance_marker("just slow"));
    }

    #[test]
    fn scan_ignored_governance_finds_bare_and_weak_reason() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path();
        fs::create_dir_all(root.join("crates/demo/tests"))?;
        fs::write(
            root.join("crates/demo/Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        fs::write(
            root.join("crates/demo/tests/g.rs"),
            concat!(
                "\n",
                "#[ignore]\n",
                "#[test]\n",
                "fn bare() {}\n\n",
                "#[ignore = \"just slow\"]\n",
                "#[test]\n",
                "fn weak() {}\n\n",
                "#[ignore = \"owner: qa — track in TASK-1\"]\n",
                "#[test]\n",
                "fn ok() {}\n",
            ),
        )?;

        let (total, findings) = scan_ignored_test_governance_findings(root)?;
        assert_eq!(total, 3);
        assert_eq!(findings.len(), 2);
        Ok(())
    }

    #[test]
    fn markdown_emit_has_no_calendar_last_updated_field() {
        let report = TestInventoryReport {
            schema_version: 1,
            summary: SummaryCounts::default(),
            crates: BTreeMap::new(),
            rust_files_by_kind: BTreeMap::new(),
            zero_test_crates: Vec::new(),
            inline_only_crates: Vec::new(),
            top_ignored_files: Vec::new(),
            caveats: CaveatsSection::default(),
            golden_vox: GoldenVoxSection::default(),
            app_e2e_tests: AppE2eSection::default(),
            doctest_candidates: DoctestCandidateSection::default(),
            test_file_patterns: TestPatternCounts::default(),
        };
        let md = emit_markdown(&report);
        assert!(
            !md.contains("\nlast_updated:"),
            "generated docs omit calendar last_updated (deterministic regen; use git history)"
        );
    }
}
