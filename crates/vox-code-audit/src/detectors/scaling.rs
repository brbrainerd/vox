//! Scaling / performance hygiene — blocking I/O in `async`, path and numeric literals, heuristics.
//!
//! Suppressions: `// toestub-ignore(scaling)` or rule-specific `toestub-ignore(scaling/blocking-in-async)`.

use std::collections::HashSet;
use std::path::Path;

use regex::Regex;
use vox_scaling_policy::ScalingPolicy;

#[path = "scaling_support.rs"]
mod scaling_support;

use crate::analysis::RustFileContext;
use crate::rules::{
    DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile,
    rust_byte_is_non_code,
};

fn substring_match_in_code(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    needle: &str,
    rust_ctx: Option<&RustFileContext>,
) -> bool {
    line.match_indices(needle)
        .any(|(i, _)| !rust_byte_is_non_code(file, line_num, i, rust_ctx))
}

fn regex_match_in_code(
    file: &SourceFile,
    line_num: usize,
    line: &str,
    re: &Regex,
    rust_ctx: Option<&RustFileContext>,
) -> bool {
    re.find(line)
        .is_some_and(|m| !rust_byte_is_non_code(file, line_num, m.start(), rust_ctx))
}

pub struct ScalingSurfacesDetector {
    policy: ScalingPolicy,
    path_literal_re: Regex,
    magic_num_re: Regex,
    sql_select_re: Regex,
    sql_from_re: Regex,
    sql_limit_re: Regex,
    client_new_re: Regex,
    vec_capacity_re: Regex,
    env_unwrap_or_re: Regex,
}

impl Default for ScalingSurfacesDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ScalingSurfacesDetector {
    pub fn new() -> Self {
        Self::with_policy(ScalingPolicy::embedded())
    }

    pub fn with_policy(policy: ScalingPolicy) -> Self {
        let mut fragments: Vec<String> = policy.path_literals.mens_runs_variants.clone();
        fragments.extend(policy.path_literals.extra_flag_literals.clone());

        let mut alt: Vec<String> = fragments.iter().map(|s| regex::escape(s)).collect();
        alt.sort_by_key(|s| std::cmp::Reverse(s.len()));
        let path_literal_re = Regex::new(&format!(r#""({})""#, alt.join("|"))).expect("valid");

        let hints: Vec<String> = policy
            .magic_numeric_hints
            .iter()
            .map(|n| regex::escape(&n.to_string()))
            .collect();
        let magic_num_re = Regex::new(&format!(r"\b({})\b", hints.join("|"))).expect("valid");

        Self {
            policy,
            path_literal_re,
            magic_num_re,
            sql_select_re: Regex::new(r"(?i)SELECT\s").expect("valid"),
            sql_from_re: Regex::new(r"(?i)\bFROM\b").expect("valid"),
            sql_limit_re: Regex::new(r"(?i)\bLIMIT\b").expect("valid"),
            client_new_re: Regex::new(r"Client::new\s*\(\s*\)").expect("valid"),
            vec_capacity_re: Regex::new(r"Vec::with_capacity\s*\(\s*(\d[\d_]*)\s*\)")
                .expect("valid"),
            env_unwrap_or_re: Regex::new(r#"unwrap_or\s*\(\s*"([^"]*)"\s*\)"#).expect("valid"),
        }
    }

    fn crate_from_path(&self, path: &Path) -> Option<String> {
        let s = path.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = s.split('/').collect();
        for w in parts.windows(2) {
            if w[0] == "crates" {
                return Some(w[1].to_string());
            }
        }
        None
    }

    fn crate_allows_blocking_fs(&self, path: &Path) -> bool {
        let Some(name) = self.crate_from_path(path) else {
            return false;
        };
        self.policy
            .per_crate_overrides
            .iter()
            .any(|o| o.crate_name == name && o.allow_blocking_fs_in_async)
    }

    fn line_suppressed(&self, line: &str, rule_suffix: &str) -> bool {
        if !line.contains("toestub-ignore") {
            return false;
        }
        if line.contains("toestub-ignore(all)") {
            return true;
        }
        if line.contains(&format!("toestub-ignore({rule_suffix})"))
            || line.contains("toestub-ignore(scaling)")
        {
            return true;
        }
        false
    }

    fn detect_rust_syn(&self, file: &SourceFile) -> Vec<Finding> {
        let crate_allow = self.crate_allows_blocking_fs(&file.path);
        scaling_support::detect_rust_syn_blockings(file, crate_allow)
    }

    fn detect_rust_lines(
        &self,
        file: &SourceFile,
        rust_ctx: Option<&RustFileContext>,
        fs_unbounded_ast_lines: &HashSet<usize>,
    ) -> Vec<Finding> {
        if file.language != Language::Rust {
            return Vec::new();
        }
        let mut findings = Vec::new();
        let mut in_test_block = false;
        let test_attr = Regex::new(r"#\[(?:cfg\(test\)|test)\]").expect("valid");
        let syn_ok = syn::parse_file(&file.content).is_ok();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            if test_attr.is_match(line) {
                in_test_block = true;
            }
            if in_test_block {
                let trimmed = line.trim();
                if (trimmed.starts_with("fn ") || trimmed.starts_with("mod "))
                    && !trimmed.contains("test")
                    && !line.starts_with(char::is_whitespace)
                {
                    in_test_block = false;
                }
            }
            if in_test_block || line.trim_start().starts_with("//") {
                continue;
            }
            if self.line_suppressed(line, "scaling/path-literal")
                && self.line_suppressed(line, "scaling/magic-limit")
            {
                continue;
            }

            if !self.line_suppressed(line, "scaling/path-literal")
                && regex_match_in_code(file, line_num, line, &self.path_literal_re, rust_ctx)
                && !line.contains("DEFAULT_MENS_RUNS")
                && !line.contains("vox_scaling_policy")
            {
                findings.push(Finding {
                    rule_id: "scaling/path-literal".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — path literal".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Repeated repo-relative path literal — use `vox_scaling_policy` constants or config"
                        .to_string(),
                    suggestion: Some(
                        "`vox_scaling_policy::DEFAULT_MENS_RUNS_ROOT` / env / SSOT policy.yaml"
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if regex_match_in_code(file, line_num, line, &self.magic_num_re, rust_ctx)
                && !self.line_suppressed(line, "scaling/magic-limit")
                && !line.contains("const ")
                && !line.contains("pub const")
                && !line.contains("DEFAULT_")
                && !line.contains("thresholds.")
            {
                findings.push(Finding {
                    rule_id: "scaling/magic-limit".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — magic numeric hint".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Numeric literal matches scaling SSOT hint list — centralize in `contracts/scaling/policy.yaml` / named constant"
                        .to_string(),
                    suggestion: Some(
                        "Use `ScalingPolicy::embedded()` thresholds or a named `const` near the callsite."
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if substring_match_in_code(file, line_num, line, "Regex::new(", rust_ctx)
                && !substring_match_in_code(file, line_num, line, "LazyLock", rust_ctx)
                && !substring_match_in_code(file, line_num, line, "OnceLock", rust_ctx)
                && !self.line_suppressed(line, "scaling/regex-new-hot")
            {
                findings.push(Finding {
                    rule_id: "scaling/regex-new-hot".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — Regex::new in hot path".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "`Regex::new` on a non-lazy path — compile once (`LazyLock`/`OnceLock`/`static`)"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if !fs_unbounded_ast_lines.contains(&line_num)
                && substring_match_in_code(file, line_num, line, "read_to_string(", rust_ctx)
                && substring_match_in_code(file, line_num, line, "std::fs", rust_ctx)
                && (!syn_ok || crate::run_context::feature_enabled("scaling-fs-heuristic-fallback"))
            {
                findings.push(Finding {
                    rule_id: "scaling/unbounded-read".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — fs read_to_string".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Unbounded `read_to_string` — consider size cap / streaming / `tokio::fs` in async contexts"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: Some(serde_json::json!({
                        "why": "line heuristic (parse failed or scaling-fs-heuristic-fallback)",
                        "evidence": ["line"]
                    })),
                });
            }

            if (substring_match_in_code(file, line_num, line, "read_to_string(", rust_ctx)
                || substring_match_in_code(file, line_num, line, "std::fs::read(", rust_ctx)
                || substring_match_in_code(file, line_num, line, "fs::read(", rust_ctx)
                || substring_match_in_code(file, line_num, line, "OpenOptions::", rust_ctx))
                && scaling_support::recent_line_starts_for_loop(&file.lines, i, 20)
                && !self.line_suppressed(line, "scaling/cache-miss-hot-read")
            {
                findings.push(Finding {
                    rule_id: "scaling/cache-miss-hot-read".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — repeated disk read in loop".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Filesystem read under a recent `for` loop — consider batching, caching, or mmap"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if let Some(m) = self.vec_capacity_re.find(line)
                && !rust_byte_is_non_code(file, line_num, m.start(), rust_ctx)
                && let Some(cap) = self.vec_capacity_re.captures(line)
                && let Some(n) = cap
                    .get(1)
                    .and_then(|mm| scaling_support::parse_rust_usize_literal(mm.as_str()))
                && n >= 100_000
                && !self.line_suppressed(line, "scaling/large-in-memory-accumulator")
            {
                findings.push(Finding {
                    rule_id: "scaling/large-in-memory-accumulator".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — very large Vec preallocation".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: format!(
                        "`Vec::with_capacity({n})` — ensure N is bounded; streaming may scale better"
                    ),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if substring_match_in_code(file, line_num, line, ".lines()", rust_ctx)
                && substring_match_in_code(file, line_num, line, "collect::<Vec", rust_ctx)
            {
                findings.push(Finding {
                    rule_id: "scaling/lines-collect-vec".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — lines().collect heap".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Collecting all lines into `Vec` can spike memory on large files — stream or batch"
                        .to_string(),
                    suggestion: Some(format!(
                        "See policy `corpus_validate_batch_lines` = {}",
                        self.policy.thresholds.corpus_validate_batch_lines
                    )),
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if substring_match_in_code(file, line_num, line, "serde_json::from_str", rust_ctx)
                && substring_match_in_code(file, line_num, line, "for ", rust_ctx)
                && (substring_match_in_code(file, line_num, line, " lines", rust_ctx)
                    || file.lines.get(i + 1).is_some_and(|l| {
                        substring_match_in_code(
                            file,
                            line_num.saturating_add(1),
                            l,
                            "for ",
                            rust_ctx,
                        )
                    }))
            {
                findings.push(Finding {
                    rule_id: "scaling/repeated-json-parse".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — JSON parse in loop".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Possible per-iteration `serde_json::from_str` — batch or parse once"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            let sql_scan = scaling_support::sql_line_for_keyword_scan(line);
            if substring_match_in_code(file, line_num, line, r#"""#, rust_ctx)
                && self.sql_select_re.is_match(&sql_scan)
                && self.sql_from_re.is_match(&sql_scan)
                && !self.sql_limit_re.is_match(&sql_scan)
                && !self.line_suppressed(line, "scaling/sql-no-limit")
            {
                findings.push(Finding {
                    rule_id: "scaling/sql-no-limit".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — SQL without LIMIT".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "SQL string may lack LIMIT — unbounded result sets don't scale"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: Some(serde_json::json!({
                        "why": "SQL keyword scan after comment/string strip",
                        "evidence": ["line", "sql_scan"]
                    })),
                });
            }

            if regex_match_in_code(file, line_num, line, &self.client_new_re, rust_ctx)
                && !self.line_suppressed(line, "scaling/http-client-no-timeout")
            {
                findings.push(Finding {
                    rule_id: "scaling/http-client-no-timeout".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — HTTP client default".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "`Client::new()` may lack timeouts — use builder with `.timeout(...)`"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 1),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }

            if substring_match_in_code(file, line_num, line, "for ", rust_ctx)
                && substring_match_in_code(file, line_num, line, " 0..", rust_ctx)
                && let Some(next) = file.lines.get(i + 1)
                && (substring_match_in_code(
                    file,
                    line_num.saturating_add(1),
                    next,
                    "(i + 1)..",
                    rust_ctx,
                ) || substring_match_in_code(
                    file,
                    line_num.saturating_add(1),
                    next,
                    "(i+1)..",
                    rust_ctx,
                ))
            {
                findings.push(Finding {
                    rule_id: "scaling/nested-pairwise-loop".to_string(),
                    diagnostic_id: None,
                    rule_name: "Scaling — pairwise nested loop".to_string(),
                    severity: Severity::Info,
                    file: file.path.clone(),
                    line: line_num,
                    column: 0,
                    message: "Nested loop with `(i+1)..` — ensure collection size stays bounded"
                        .to_string(),
                    suggestion: None,
                    alternatives: vec![],
                    rationale: None,
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Low),
                    evidence: None,
                });
            }
        }

        findings.extend(scaling_support::env_unwrap_or_duplicate_findings(
            file,
            &self.env_unwrap_or_re,
            rust_ctx,
        ));
        findings
    }
}

impl DetectionRule for ScalingSurfacesDetector {
    fn id(&self) -> &'static str {
        "scaling/surfaces"
    }

    fn name(&self) -> &'static str {
        "Scaling surfaces detector"
    }

    fn description(&self) -> &'static str {
        "Heuristics for scaling risks: blocking I/O in async, literals, regex, SQL/HTTP patterns"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — blocking I/O call inside an async function\n\
             async fn fetch_user_data(id: UserId) -> UserData {\n\
             // vox-arch-check: allow abs-path\n\
             \x20   let raw = std::fs::read_to_string(\"/var/data/users.json\") // blocking!\n\
             \x20       .unwrap();\n\
             \x20   parse_user(id, &raw)\n\
             }\n\
             \n\
             // FIX — use async I/O\n\
             async fn fetch_user_data(id: UserId) -> Result<UserData, Error> {\n\
             // vox-arch-check: allow abs-path\n\
             \x20   let raw = tokio::fs::read_to_string(\"/var/data/users.json\").await?;\n\
             \x20   Ok(parse_user(id, &raw))\n\
             }",
        )
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn detect(
        &self,
        file: &SourceFile,
        rust: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let fs_reads = scaling_support::fs_unbounded_read_findings(file);
        let fs_lines: HashSet<usize> = fs_reads.iter().map(|f| f.line).collect();
        let mut out = self.detect_rust_syn(file);
        out.extend(fs_reads);
        out.extend(self.detect_rust_lines(file, rust, &fs_lines));
        out
    }
}

#[cfg(test)]
#[path = "scaling_tests.rs"]
mod tests;
