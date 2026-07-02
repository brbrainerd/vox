//! Parse nextest / JUnit XML and emit slow-test + retry/flaky summaries for CI telemetry.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use vox_bounded_fs::read_utf8_path_capped;

/// CLI options for `vox ci test-runtime-report`.
#[derive(Debug, Clone)]
pub struct TestRuntimeReportOpts {
    pub junit: PathBuf,
    pub json: bool,
    pub markdown: Option<PathBuf>,
    pub top: usize,
    pub fail_over_ms: Option<u64>,
    pub fail_retry_candidates: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaseStatus {
    Passed,
    Skipped,
    Failure,
    Error,
}

#[derive(Debug, Clone)]
struct RawCase {
    name: String,
    classname: Option<String>,
    file: Option<String>,
    time_secs: f64,
    status: CaseStatus,
    rerun_or_flaky_children: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlowTestEntry {
    pub duration_secs: f64,
    pub classname: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryFlakyCandidate {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classname: Option<String>,
    /// `duplicate_testcase_rows` or `rerun_or_flaky_xml_children`.
    pub signal: String,
    pub testcase_rows: usize,
    pub rerun_or_flaky_markers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestRuntimeReport {
    pub total_tests: usize,
    pub failures: usize,
    pub errors: usize,
    pub skipped: usize,
    pub passed: usize,
    pub total_duration_secs: f64,
    pub top_slow_tests: Vec<SlowTestEntry>,
    pub retry_flaky_candidates: Vec<RetryFlakyCandidate>,
    pub warnings: Vec<String>,
}

fn open_tag_end(xml: &str, open_after_lt: usize) -> Option<usize> {
    let slice = &xml[open_after_lt..];
    let mut quote: Option<char> = None;
    for (idx, ch) in slice.char_indices() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '>' => return Some(open_after_lt + idx),
                _ => {}
            },
        }
    }
    None
}

fn parse_attrs(open_tag: &str) -> BTreeMap<String, String> {
    let re = Regex::new(r#"([\w:-]+)\s*=\s*"([^"]*)""#).expect("attr regex");
    let mut m = BTreeMap::new();
    for cap in re.captures_iter(open_tag) {
        m.insert(cap[1].to_string(), cap[2].to_string());
    }
    m
}

fn classify_body(body: &str) -> (CaseStatus, usize) {
    let rerun_or_flaky = body.matches("<flakyFailure").count()
        + body.matches("<rerunFailure").count()
        + body.matches("<flakyPass").count();
    let status = if body.contains("<skipped") {
        CaseStatus::Skipped
    } else if body.contains("<failure") {
        CaseStatus::Failure
    } else if body.contains("<error") {
        CaseStatus::Error
    } else {
        CaseStatus::Passed
    };
    (status, rerun_or_flaky)
}

fn scan_testcases(xml: &str) -> Result<Vec<RawCase>> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = xml[search..].find("<testcase") {
        let start = search + rel;
        let after_kw = start + "<testcase".len();
        let gt =
            open_tag_end(xml, after_kw).ok_or_else(|| anyhow!("malformed XML: testcase tag"))?;
        let open_fragment = &xml[start..=gt];
        let attrs = parse_attrs(open_fragment);
        let name = attrs
            .get("name")
            .cloned()
            .ok_or_else(|| anyhow!("testcase missing name attribute"))?;
        let classname = attrs.get("classname").cloned();
        let file = attrs.get("file").cloned();
        let time_secs = attrs
            .get("time")
            .map(|s| s.parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);

        let self_closing = open_fragment.trim_end().ends_with("/>");
        let (status, rerun_markers, next_search) = if self_closing {
            (CaseStatus::Passed, 0usize, gt + 1)
        } else {
            let body_start = gt + 1;
            let close_rel = xml[body_start..]
                .find("</testcase>")
                .ok_or_else(|| anyhow!("malformed XML: missing </testcase>"))?;
            let body = &xml[body_start..body_start + close_rel];
            let (st, rm) = classify_body(body);
            let end = body_start + close_rel + "</testcase>".len();
            (st, rm, end)
        };

        out.push(RawCase {
            name,
            classname,
            file,
            time_secs,
            status,
            rerun_or_flaky_children: rerun_markers,
        });
        search = next_search;
    }
    Ok(out)
}

fn key(classname: &Option<String>, name: &str) -> (String, String) {
    (classname.clone().unwrap_or_default(), name.to_string())
}

/// Parse JUnit XML text into an aggregate report (`top` limits slow-test list).
pub fn build_report(xml: &str, top: usize) -> Result<TestRuntimeReport> {
    let cases = scan_testcases(xml)?;

    let total_tests = cases.len();
    let mut failures = 0usize;
    let mut errors = 0usize;
    let mut skipped = 0usize;
    let mut passed = 0usize;
    let mut total_duration_secs = 0.0_f64;

    let mut max_time: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut best_file: BTreeMap<(String, String), Option<String>> = BTreeMap::new();
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();

    for c in &cases {
        total_duration_secs += c.time_secs;
        match c.status {
            CaseStatus::Passed => passed += 1,
            CaseStatus::Skipped => skipped += 1,
            CaseStatus::Failure => failures += 1,
            CaseStatus::Error => errors += 1,
        }
        let k = key(&c.classname, &c.name);
        *counts.entry(k.clone()).or_insert(0) += 1;
        let e = max_time.entry(k.clone()).or_insert(0.0);
        *e = (*e).max(c.time_secs);
        if let Some(f) = &c.file {
            best_file.entry(k).or_insert_with(|| Some(f.clone()));
        }
    }

    let mut slow_pairs: Vec<((String, String), f64)> = max_time.into_iter().collect();
    slow_pairs.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (a.0).0.cmp(&(b.0).0))
            .then_with(|| (a.0).1.cmp(&(b.0).1))
    });

    let top_slow_tests: Vec<SlowTestEntry> = slow_pairs
        .into_iter()
        .take(top)
        .map(|((cn, n), dur)| {
            let src = best_file.get(&(cn.clone(), n.clone())).cloned().flatten();
            SlowTestEntry {
                duration_secs: dur,
                classname: (!cn.is_empty()).then_some(cn),
                name: n,
                source_path: src,
            }
        })
        .collect();

    let mut retry_flaky_candidates: Vec<RetryFlakyCandidate> = Vec::new();

    for c in &cases {
        if c.rerun_or_flaky_children > 0 {
            retry_flaky_candidates.push(RetryFlakyCandidate {
                name: c.name.clone(),
                classname: c.classname.clone(),
                signal: "rerun_or_flaky_xml_children".to_string(),
                testcase_rows: 1,
                rerun_or_flaky_markers: c.rerun_or_flaky_children,
            });
        }
    }

    for ((cn, n), ct) in &counts {
        if *ct > 1 {
            retry_flaky_candidates.push(RetryFlakyCandidate {
                name: n.clone(),
                classname: (!cn.is_empty()).then(|| cn.clone()),
                signal: "duplicate_testcase_rows".to_string(),
                testcase_rows: *ct,
                rerun_or_flaky_markers: 0,
            });
        }
    }

    retry_flaky_candidates.sort_by(|a, b| {
        a.signal
            .cmp(&b.signal)
            .then_with(|| a.classname.cmp(&b.classname))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(TestRuntimeReport {
        total_tests,
        failures,
        errors,
        skipped,
        passed,
        total_duration_secs,
        top_slow_tests,
        retry_flaky_candidates,
        warnings: Vec::new(),
    })
}

/// Deserialize JSON emitted by `vox ci test-runtime-report --json`.
pub fn parse_runtime_report_json(text: &str) -> Result<TestRuntimeReport> {
    serde_json::from_str(text).context("parse TestRuntimeReport JSON")
}

/// Number of retry/flaky heuristic rows (duplicate testcase rows + rerun/flaky XML markers).
pub fn retry_flaky_candidate_count(report: &TestRuntimeReport) -> usize {
    report.retry_flaky_candidates.len()
}

/// One test whose runtime grew versus baseline beyond `--percent` or `--absolute-ms`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeRegressFinding {
    pub classname: Option<String>,
    pub name: String,
    pub baseline_secs: f64,
    pub current_secs: f64,
    pub delta_secs: f64,
    pub percent_over_baseline: f64,
}

/// Diff between two [`TestRuntimeReport`] JSON snapshots (baseline vs current).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuntimeRegressReport {
    pub regressions: Vec<RuntimeRegressFinding>,
    /// Baseline top-slow entries not present in the current report's `top_slow_tests` slice.
    pub missing_in_current_top: Vec<SlowTestEntry>,
}

fn slow_tests_map(report: &TestRuntimeReport) -> std::collections::BTreeMap<(String, String), f64> {
    let mut m = std::collections::BTreeMap::new();
    for t in &report.top_slow_tests {
        let cn = t.classname.clone().unwrap_or_default();
        m.insert((cn, t.name.clone()), t.duration_secs);
    }
    m
}

/// Compare baseline vs current using only `top_slow_tests` entries present in both JSON payloads.
///
/// A regression is recorded when `current > baseline` and either relative slowdown exceeds
/// `percent` (%) or absolute slowdown exceeds `absolute_ms` milliseconds. Tests listed only in the
/// baseline top slice are reported in `missing_in_current_top` (no duration comparison).
pub fn compare_runtime_reports(
    baseline: &TestRuntimeReport,
    current: &TestRuntimeReport,
    percent: f64,
    absolute_ms: u64,
) -> RuntimeRegressReport {
    let cur_map = slow_tests_map(current);
    let abs_thresh = absolute_ms as f64 / 1000.0;
    let mut regressions = Vec::new();
    let mut missing_in_current_top = Vec::new();

    for t in &baseline.top_slow_tests {
        let cn = t.classname.clone().unwrap_or_default();
        let key = (cn.clone(), t.name.clone());
        let Some(&cur_sec) = cur_map.get(&key) else {
            missing_in_current_top.push(t.clone());
            continue;
        };
        let base_sec = t.duration_secs;
        let delta = cur_sec - base_sec;
        if delta <= f64::EPSILON {
            continue;
        }
        let pct_over = if base_sec > f64::EPSILON {
            (delta / base_sec) * 100.0
        } else {
            f64::INFINITY
        };
        let exceeds_abs = delta > abs_thresh;
        let exceeds_pct = pct_over > percent;
        if exceeds_abs || exceeds_pct {
            regressions.push(RuntimeRegressFinding {
                classname: t.classname.clone(),
                name: t.name.clone(),
                baseline_secs: base_sec,
                current_secs: cur_sec,
                delta_secs: delta,
                percent_over_baseline: pct_over,
            });
        }
    }

    RuntimeRegressReport {
        regressions,
        missing_in_current_top,
    }
}

fn apply_threshold_warnings(report: &mut TestRuntimeReport, opts: &TestRuntimeReportOpts) {
    if let Some(ms) = opts.fail_over_ms {
        let limit = ms as f64 / 1000.0;
        let mut bad = 0usize;
        for t in &report.top_slow_tests {
            if t.duration_secs > limit {
                bad += 1;
            }
        }
        if bad > 0 {
            report.warnings.push(format!(
                "--fail-over-ms={ms}: {bad} test(s) in top-{} exceed {limit}s (report-only; no exit failure)",
                opts.top
            ));
        }
    }
    if let Some(max_c) = opts.fail_retry_candidates {
        let n = report.retry_flaky_candidates.len();
        if n > max_c {
            report.warnings.push(format!(
                "--fail-retry-count={max_c}: {n} retry/flaky candidate row(s) (report-only; no exit failure)"
            ));
        }
    }
}

fn emit_markdown(report: &TestRuntimeReport, top: usize) -> String {
    let mut s = String::new();
    s.push_str("# Test runtime report (JUnit / nextest)\n\n");
    s.push_str("Generated by `vox ci test-runtime-report`. ");
    s.push_str("Retry/flaky signals are **heuristic** (duplicate `<testcase>` rows and `<flakyFailure>` / `<rerunFailure>` children per nextest).\n\n");

    s.push_str("## Summary\n\n");
    s.push_str(&format!(
        "| Metric | Value |\n|:---|---:|\n| Total testcase rows | {} |\n| Passed | {} |\n| Failures | {} |\n| Errors | {} |\n| Skipped | {} |\n| Sum of testcase `time` (s) | {:.3} |\n\n",
        report.total_tests,
        report.passed,
        report.failures,
        report.errors,
        report.skipped,
        report.total_duration_secs
    ));

    s.push_str(&format!("## Top slow tests (top {})\n\n", top));
    if report.top_slow_tests.is_empty() {
        s.push_str("_None parsed._\n\n");
    } else {
        s.push_str(
            "| Rank | Seconds | Classname | Name | Source |\n|:---:|:---:|:---|:---|:---|\n",
        );
        for (i, t) in report.top_slow_tests.iter().enumerate() {
            let cn = t.classname.as_deref().unwrap_or("");
            let src = t.source_path.as_deref().unwrap_or("");
            s.push_str(&format!(
                "| {} | {:.4} | `{}` | `{}` | `{}` |\n",
                i + 1,
                t.duration_secs,
                cn,
                t.name,
                src
            ));
        }
        s.push('\n');
    }

    s.push_str("## Retry / flaky candidates\n\n");
    if report.retry_flaky_candidates.is_empty() {
        s.push_str("_None inferred._\n\n");
    } else {
        s.push_str("| Signal | Classname | Name | Rows | Markers |\n|:---|:---|:---|---:|---:|\n");
        for r in &report.retry_flaky_candidates {
            let cn = r.classname.as_deref().unwrap_or("");
            s.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} |\n",
                r.signal, cn, r.name, r.testcase_rows, r.rerun_or_flaky_markers
            ));
        }
        s.push('\n');
    }

    s.push_str("## Caveats\n\n");
    s.push_str("- **Not a stability verdict:** duplicates may be tooling quirks; XML children reflect nextest retries when JUnit is emitted by nextest.\n");
    s.push_str("- **Sum of times:** wall-clock CI duration is not equal to the sum of per-test times (parallelism).\n");
    if !report.warnings.is_empty() {
        s.push_str("- **Threshold notices:**\n");
        for w in &report.warnings {
            s.push_str(&format!("  - {w}\n"));
        }
    }
    s.push('\n');
    s
}

fn print_human(report: &TestRuntimeReport, top: usize) {
    println!(
        "JUnit summary: {} tests ({} passed, {} failed, {} error, {} skipped), {:.3}s total (sum of testcase times)",
        report.total_tests,
        report.passed,
        report.failures,
        report.errors,
        report.skipped,
        report.total_duration_secs
    );
    println!("Top {top} slowest tests (max time per classname+name):");
    for (i, t) in report.top_slow_tests.iter().enumerate() {
        let cn = t.classname.as_deref().unwrap_or("(no class)");
        let src = t
            .source_path
            .as_deref()
            .map(|s| format!(" [{s}]"))
            .unwrap_or_default();
        println!(
            "  {:>2}. {:>8.4}s  {}::{}{}",
            i + 1,
            t.duration_secs,
            cn,
            t.name,
            src
        );
    }
    if !report.retry_flaky_candidates.is_empty() {
        println!("Retry/flaky candidates:");
        for r in &report.retry_flaky_candidates {
            println!(
                "  - [{}] {}::{} (rows={}, markers={})",
                r.signal,
                r.classname.as_deref().unwrap_or(""),
                r.name,
                r.testcase_rows,
                r.rerun_or_flaky_markers
            );
        }
    }
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
}

/// Run `vox ci test-runtime-report`.
pub fn run(_root: &Path, opts: TestRuntimeReportOpts) -> Result<()> {
    let xml = read_utf8_path_capped(&opts.junit)
        .with_context(|| format!("read JUnit XML {}", opts.junit.display()))?;
    let mut report = build_report(&xml, opts.top)?;
    apply_threshold_warnings(&mut report, &opts);

    if let Some(md_path) = &opts.markdown {
        if let Some(parent) = md_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
        }
        let md = emit_markdown(&report, opts.top);
        fs::write(md_path, md).with_context(|| format!("write {}", md_path.display()))?;
    }

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report, opts.top);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

    #[test]
    fn normal_times_and_top_n_tiebreak() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite" tests="3">
    <testcase name="b" classname="c" time="1.0"/>
    <testcase name="a" classname="c" time="1.0"/>
    <testcase name="z" classname="c" time="2.0"/>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 2).unwrap();
        assert_eq!(r.total_tests, 3);
        assert_eq!(r.passed, 3);
        assert_eq!(r.top_slow_tests.len(), 2);
        assert_eq!(r.top_slow_tests[0].name, "z");
        assert_eq!(r.top_slow_tests[1].name, "a");
    }

    #[test]
    fn skipped_failure_error_counts() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="ok" classname="m" time="0.01"/>
    <testcase name="sk" classname="m" time="0"><skipped/></testcase>
    <testcase name="bad" classname="m" time="0.02"><failure type="assert">no</failure></testcase>
    <testcase name="oops" classname="m" time="0.03"><error type="err">boom</error></testcase>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 10).unwrap();
        assert_eq!(r.total_tests, 4);
        assert_eq!(r.passed, 1);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.failures, 1);
        assert_eq!(r.errors, 1);
    }

    #[test]
    fn duplicate_testcase_rows_heuristic() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="dup" classname="x" time="0.1"/>
    <testcase name="dup" classname="x" time="0.2"/>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 5).unwrap();
        assert_eq!(r.retry_flaky_candidates.len(), 1);
        assert_eq!(
            r.retry_flaky_candidates[0].signal,
            "duplicate_testcase_rows"
        );
        assert_eq!(r.retry_flaky_candidates[0].testcase_rows, 2);
    }

    #[test]
    fn flaky_failure_children() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="flaky" classname="p" time="0.05">
      <flakyFailure time="0.01">a</flakyFailure>
      <flakyFailure time="0.02">b</flakyFailure>
    </testcase>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 5).unwrap();
        assert_eq!(r.retry_flaky_candidates.len(), 1);
        assert_eq!(
            r.retry_flaky_candidates[0].signal,
            "rerun_or_flaky_xml_children"
        );
        assert_eq!(r.retry_flaky_candidates[0].rerun_or_flaky_markers, 2);
    }

    #[test]
    fn threshold_warnings_fire() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="slow" classname="c" time="2.0"/>
    <testcase name="slow" classname="c" time="2.0"/>
  </testsuite>
</testsuites>"#
        );
        let mut r = build_report(&xml, 5).unwrap();
        let opts = TestRuntimeReportOpts {
            junit: PathBuf::from("_unused"),
            json: false,
            markdown: None,
            top: 5,
            fail_over_ms: Some(500),
            fail_retry_candidates: Some(0),
        };
        apply_threshold_warnings(&mut r, &opts);
        assert_eq!(r.warnings.len(), 2);
    }

    #[test]
    fn parse_json_roundtrip_preserves_counts() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="a" classname="c" time="0.5"/>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 3).unwrap();
        let j = serde_json::to_string(&r).unwrap();
        let r2 = parse_runtime_report_json(&j).unwrap();
        assert_eq!(r.total_tests, r2.total_tests);
        assert_eq!(
            r.retry_flaky_candidates.len(),
            r2.retry_flaky_candidates.len()
        );
    }

    #[test]
    fn runtime_regress_flags_percent_increase() {
        let base = TestRuntimeReport {
            total_tests: 1,
            failures: 0,
            errors: 0,
            skipped: 0,
            passed: 1,
            total_duration_secs: 1.0,
            top_slow_tests: vec![SlowTestEntry {
                duration_secs: 1.0,
                classname: Some("m".into()),
                name: "t".into(),
                source_path: None,
            }],
            retry_flaky_candidates: vec![],
            warnings: vec![],
        };
        let cur = TestRuntimeReport {
            total_tests: 1,
            failures: 0,
            errors: 0,
            skipped: 0,
            passed: 1,
            total_duration_secs: 1.5,
            top_slow_tests: vec![SlowTestEntry {
                duration_secs: 1.5,
                classname: Some("m".into()),
                name: "t".into(),
                source_path: None,
            }],
            retry_flaky_candidates: vec![],
            warnings: vec![],
        };
        let rep = compare_runtime_reports(&base, &cur, 25.0, 500);
        assert_eq!(rep.regressions.len(), 1);
        assert!(rep.missing_in_current_top.is_empty());
    }

    #[test]
    fn runtime_regress_missing_top_emits_warning_slice() {
        let base = TestRuntimeReport {
            total_tests: 1,
            failures: 0,
            errors: 0,
            skipped: 0,
            passed: 1,
            total_duration_secs: 2.0,
            top_slow_tests: vec![SlowTestEntry {
                duration_secs: 2.0,
                classname: Some("m".into()),
                name: "gone".into(),
                source_path: None,
            }],
            retry_flaky_candidates: vec![],
            warnings: vec![],
        };
        let cur = TestRuntimeReport {
            total_tests: 1,
            failures: 0,
            errors: 0,
            skipped: 0,
            passed: 1,
            total_duration_secs: 0.5,
            top_slow_tests: vec![SlowTestEntry {
                duration_secs: 0.5,
                classname: Some("x".into()),
                name: "other".into(),
                source_path: None,
            }],
            retry_flaky_candidates: vec![],
            warnings: vec![],
        };
        let rep = compare_runtime_reports(&base, &cur, 25.0, 500);
        assert!(rep.regressions.is_empty());
        assert_eq!(rep.missing_in_current_top.len(), 1);
    }

    #[test]
    fn source_path_from_file_attr() {
        let xml = format!(
            r#"{HEADER}
<testsuites>
  <testsuite name="suite">
    <testcase name="t" classname="c" time="0.5" file="crates/foo/src/lib.rs"/>
  </testsuite>
</testsuites>"#
        );
        let r = build_report(&xml, 3).unwrap();
        assert_eq!(
            r.top_slow_tests[0].source_path.as_deref(),
            Some("crates/foo/src/lib.rs")
        );
    }
}
