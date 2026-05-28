//! Lightweight **evaluation metrics** for model outputs and Vox code samples (held-out / smoke tests).
//!
//! Functions are deterministic heuristics—not a replacement for human eval or full static analysis.

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

pub mod mens;

/// Aggregated scores from a single eval run (format, safety, quality proxy, parse validity, coverage).
pub struct EvalMetrics {
    /// Fraction of responses that pass basic format checks (non-empty, no immediate refusal prefix).
    pub format_validity: f64,
    /// Fraction flagged as likely prompt-injection or instruction override.
    pub safety_rejection_rate: f64,
    /// Heuristic quality score from response length bands.
    pub quality_proxy: f64,
    /// Fraction of samples that parse as valid Vox (when applicable).
    pub vox_parse_validity: f64,
    /// Normalized count of distinct language constructs detected in code samples.
    pub construct_coverage: f64,
}

const SAFETY_PATTERNS: &[&str] = &[
    "ignore previous instructions",
    "ignore all above",
    "disregard your instructions",
    "you are now",
    "new instructions:",
];

/// Scores model output based on whether it starts with common refusal prefixes.
pub fn format_validity_score(response: &str) -> f64 {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    let bad_starts = ["I cannot", "Error:", "Sorry,", "I'm unable"];
    if bad_starts.iter().any(|&bad| trimmed.starts_with(bad)) {
        return 0.0;
    }
    1.0
}

/// Returns `true` if the text matches known unsafe prompt-injection heuristics.
pub fn is_safety_rejection(response: &str) -> bool {
    let lower = response.to_lowercase();
    SAFETY_PATTERNS.iter().any(|&p| lower.contains(p))
}

/// Scores quality based on output length as a simple proxy.
pub fn quality_proxy_score(response: &str) -> f64 {
    let n = response.trim().len();
    if n == 0 {
        0.0
    } else if n < 10 {
        0.2
    } else if n < 50 {
        0.5
    } else if n < 200 {
        0.8
    } else {
        1.0
    }
}

/// Vox constructs and their matching regex patterns.
fn get_vox_constructs() -> &'static HashMap<&'static str, Regex> {
    static CONSTRUCTS: OnceLock<HashMap<&'static str, Regex>> = OnceLock::new();
    CONSTRUCTS.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(
            "type",
            Regex::new(r"(?m)^\s*type\s+\w+\s*=").expect("vox-eval static regex: type"), // OnceLock
        );
        m.insert(
            "fn",
            Regex::new(r"(?m)^\s*(?:@\w+\s+)?fn\s+\w+").expect("vox-eval static regex: fn"), // OnceLock
        );
        m.insert(
            "actor",
            Regex::new(r"(?m)^\s*actor\s+\w+").expect("vox-eval static regex: actor"), // OnceLock
        );
        m.insert(
            "workflow",
            Regex::new(r"(?m)^\s*workflow\s+\w+").expect("vox-eval static regex: workflow"), // OnceLock
        );
        m.insert(
            "activity",
            Regex::new(r"(?m)^\s*activity\s+\w+").expect("vox-eval static regex: activity"), // OnceLock
        );
        m.insert(
            "component",
            Regex::new(r"@component").expect("vox-eval static regex: component"), // OnceLock
        );
        m.insert(
            "table",
            Regex::new(r"@table").expect("vox-eval static regex: table"), // OnceLock
        );
        m.insert(
            "query",
            Regex::new(r"@query").expect("vox-eval static regex: query"), // OnceLock
        );

        m.insert(
            "mutation",
            Regex::new(r"@mutation").expect("vox-eval static regex: mutation"), // OnceLock
        );
        m.insert(
            "action",
            Regex::new(r"@action").expect("vox-eval static regex: action"), // OnceLock
        );
        m.insert(
            "server",
            Regex::new(r"@server").expect("vox-eval static regex: server"), // OnceLock
        );
        m.insert(
            "test",
            Regex::new(r"@test").expect("vox-eval static regex: test"), // OnceLock
        );
        m.insert(
            "mcp_tool",
            Regex::new(r"@mcp\.tool").expect("vox-eval static regex: mcp_tool"), // OnceLock
        );
        m.insert(
            "mcp_resource",
            Regex::new(r"@mcp\.resource").expect("vox-eval static regex: mcp_resource"), // OnceLock
        );
        m.insert(
            "agent_def",
            Regex::new(r"@agent_def").expect("vox-eval static regex: agent_def"), // OnceLock
        );
        m.insert(
            "skill",
            Regex::new(r"@skill").expect("vox-eval static regex: skill"), // OnceLock
        );
        m.insert(
            "routes",
            Regex::new(r"(?m)^routes:").expect("vox-eval static regex: routes"), // OnceLock
        );
        m.insert(
            "style",
            Regex::new(r"(?m)^style:").expect("vox-eval static regex: style"), // OnceLock
        );
        m.insert(
            "http",
            Regex::new(r"(?i)^http\s+(get|post|put|delete)").expect("vox-eval static regex: http"), // OnceLock
        );
        m.insert(
            "message",
            Regex::new(r"(?m)^\s*message\s+\w+").expect("vox-eval static regex: message"), // OnceLock
        );
        m.insert(
            "match",
            Regex::new(r"(?m)^\s*match\s+").expect("vox-eval static regex: match"), // OnceLock
        );
        m.insert(
            "import",
            Regex::new(r"(?m)^\s*import\s+").expect("vox-eval static regex: import"), // OnceLock
        );
        m.insert(
            "let",
            Regex::new(r"(?m)^\s*let\s+").expect("vox-eval static regex: let"), // OnceLock
        );
        m.insert(
            "return",
            Regex::new(r"(?m)^\s*return\s+").expect("vox-eval static regex: return"), // OnceLock
        );
        m.insert(
            "while",
            Regex::new(r"(?m)^\s*while\s+").expect("vox-eval static regex: while"), // OnceLock
        );
        m.insert(
            "loop",
            Regex::new(r"(?m)^\s*loop\s+").expect("vox-eval static regex: loop"), // OnceLock
        );
        m.insert(
            "assert",
            Regex::new(r"\bassert\(").expect("vox-eval static regex: assert"), // OnceLock
        );
        m.insert(
            "spawn",
            Regex::new(r"\bspawn\(").expect("vox-eval static regex: spawn"), // OnceLock
        );
        m.insert(
            "with_expr",
            Regex::new(r"\bwith\s*\{").expect("vox-eval static regex: with_expr"), // OnceLock
        );
        m.insert("v0", Regex::new(r"@v0").expect("vox-eval static regex: v0")); // OnceLock
        m
    })
}

/// Returns construct names whose regex matches at least once in `code`.
#[deprecated(since = "0.4.0", note = "Use ast_eval() for parser-backed evaluation")]
pub fn detect_constructs(code: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for (&name, re) in get_vox_constructs() {
        if re.is_match(code) {
            found.push(name);
        }
    }
    found
}

/// Maps number of distinct constructs matched to `[0, 1]` with a saturating denominator.
#[deprecated(
    since = "0.4.0",
    note = "Use ast_eval().coverage_score() for parser-backed evaluation"
)]
pub fn construct_coverage_score(code: &str) -> f64 {
    #[allow(deprecated)]
    let found = detect_constructs(code);
    (found.len() as f64 / 5.0).min(1.0)
}

// Parser-backed AST evaluation moved to `vox_compiler::ast_eval` (P0-008).
// Use `vox_compiler::ast_eval(code)` or `vox_compiler::AstEvalReport` directly.
// The `detect_constructs` and `construct_coverage_score` functions above are deprecated
// in favor of the parser-backed path.

/// Bounded-domain heuristic for docs / examples: `1.0` when no high-risk escape patterns appear.
///
/// Used by `vox doctor --scope` as a coarse guardrail (not a full security audit).
pub fn scope_compliance_score(snippet: &str) -> f64 {
    let lower = snippet.to_lowercase();
    const BAD: &[&str] = &[
        "std::process::command",
        "std::fs::remove_dir_all",
        "../../../etc/passwd",
        "child_process",
        "rm -rf ",
        "eval(",
        "base64 -d",
    ];
    if BAD.iter().any(|b| lower.contains(b)) {
        return 0.0;
    }
    1.0
}

#[cfg(test)]
mod scope_tests {
    use super::scope_compliance_score;

    #[test]
    fn scope_compliance_clean_snippet() {
        assert_eq!(scope_compliance_score("fn hello(): return 42"), 1.0);
    }

    #[test]
    fn scope_compliance_flags_process_spawn() {
        assert_eq!(
            scope_compliance_score("std::process::Command::new(\"rm\")"),
            0.0
        );
    }
}

// ── Collateral Damage Rate Monitoring (Task 2.4.2) ───────────────────────────

/// Result of evaluating collateral damage on a held-out benchmark.
#[derive(Debug, Clone)]
pub struct CollateralDamageReport {
    /// Name of the benchmark suite.
    pub benchmark_name: String,
    /// Score before the training run (0.0–1.0).
    pub pre_training_score: f64,
    /// Score after the training run (0.0–1.0).
    pub post_training_score: f64,
    /// Absolute degradation (positive = regression).
    pub degradation: f64,
    /// Degradation as a fraction of the pre-training score.
    pub degradation_rate: f64,
    /// Whether degradation exceeds the configured threshold.
    pub exceeds_threshold: bool,
}

/// Configuration for collateral damage evaluation.
#[derive(Debug, Clone)]
pub struct CollateralDamageConfig {
    /// Maximum allowed degradation rate before blocking model promotion (default 0.05 = 5%).
    pub max_degradation_rate: f64,
}

impl Default for CollateralDamageConfig {
    fn default() -> Self {
        Self {
            max_degradation_rate: 0.05,
        }
    }
}

/// Evaluate collateral damage by comparing pre/post training scores on a held-out benchmark.
///
/// Research (Continual Learning §catastrophic-forgetting) proves that fine-tuning
/// without held-out evaluation hides regression. This function computes the
/// degradation rate and recommends blocking promotion if it exceeds the threshold.
///
/// `eval_fn` is a caller-supplied closure that evaluates the model against the
/// benchmark and returns a score in `[0.0, 1.0]`.
pub fn eval_collateral_damage(
    benchmark_name: &str,
    pre_training_score: f64,
    post_training_score: f64,
    config: &CollateralDamageConfig,
) -> CollateralDamageReport {
    let degradation = (pre_training_score - post_training_score).max(0.0);
    let degradation_rate = if pre_training_score > f64::EPSILON {
        degradation / pre_training_score
    } else {
        0.0
    };
    let exceeds_threshold = degradation_rate > config.max_degradation_rate;

    CollateralDamageReport {
        benchmark_name: benchmark_name.to_string(),
        pre_training_score,
        post_training_score,
        degradation,
        degradation_rate,
        exceeds_threshold,
    }
}

/// Evaluate collateral damage across multiple benchmarks.
/// Returns `Err` with the first benchmark that exceeds the threshold.
pub fn eval_collateral_damage_suite(
    scores: &[(&str, f64, f64)], // (name, pre, post)
    config: &CollateralDamageConfig,
) -> Result<Vec<CollateralDamageReport>, CollateralDamageReport> {
    let mut reports = Vec::with_capacity(scores.len());
    for &(name, pre, post) in scores {
        let report = eval_collateral_damage(name, pre, post, config);
        if report.exceeds_threshold {
            return Err(report);
        }
        reports.push(report);
    }
    Ok(reports)
}

/// Compilation-driven feedback for Rust code.
/// Spawns a lightweight `cargo check` in a temporary directory containing the provided snippet.
/// Returns 1.0 if it passes, 0.0 otherwise.
pub fn cargo_build_reward(snippet: &str) -> f64 {
    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(_) => return 0.0,
    };

    let cargo_toml = r#"
[package]
name = "vox_eval_tmp"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;

    let src_dir = tmp_dir.path().join("src");
    if std::fs::create_dir_all(&src_dir).is_err() {
        return 0.0;
    }

    if std::fs::write(tmp_dir.path().join("Cargo.toml"), cargo_toml).is_err() {
        return 0.0;
    }

    if std::fs::write(src_dir.join("main.rs"), snippet).is_err() {
        return 0.0;
    }

    let output = std::process::Command::new("cargo")
        .arg("check")
        .current_dir(tmp_dir.path())
        .output();

    match output {
        Ok(out) if out.status.success() => 1.0,
        _ => 0.0,
    }
}

/// Test-driven feedback for Rust code.
/// Spawns a lightweight `cargo test` in a temporary directory containing the provided snippet.
/// Returns 1.0 if it passes, 0.0 otherwise.
pub fn cargo_test_reward(snippet: &str) -> f64 {
    let tmp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(_) => return 0.0,
    };

    let cargo_toml = r#"
[package]
name = "vox_eval_tmp"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;

    let src_dir = tmp_dir.path().join("src");
    if std::fs::create_dir_all(&src_dir).is_err() {
        return 0.0;
    }

    if std::fs::write(tmp_dir.path().join("Cargo.toml"), cargo_toml).is_err() {
        return 0.0;
    }

    if std::fs::write(src_dir.join("main.rs"), snippet).is_err() {
        return 0.0;
    }

    let output = std::process::Command::new("cargo")
        .arg("test")
        .current_dir(tmp_dir.path())
        .output();

    match output {
        Ok(out) if out.status.success() => 1.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod collateral_damage_tests {
    use super::*;

    #[test]
    fn no_degradation_passes() {
        let r = eval_collateral_damage("mmlu", 0.80, 0.80, &CollateralDamageConfig::default());
        assert!(!r.exceeds_threshold);
        assert!((r.degradation - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn small_degradation_passes() {
        let r = eval_collateral_damage("mmlu", 0.80, 0.77, &CollateralDamageConfig::default());
        assert!(!r.exceeds_threshold);
        assert!(r.degradation_rate < 0.05);
    }

    #[test]
    fn large_degradation_fails() {
        let r = eval_collateral_damage("gsm8k", 0.80, 0.70, &CollateralDamageConfig::default());
        assert!(r.exceeds_threshold);
        assert!(r.degradation_rate > 0.05);
    }

    #[test]
    fn improvement_never_fails() {
        let r = eval_collateral_damage("mmlu", 0.70, 0.85, &CollateralDamageConfig::default());
        assert!(!r.exceeds_threshold);
        assert!((r.degradation - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn suite_returns_err_on_first_failure() {
        let scores = &[
            ("mmlu", 0.80, 0.78),  // ok: 2.5%
            ("gsm8k", 0.80, 0.70), // fail: 12.5%
            ("arc", 0.90, 0.88),   // ok: 2.2%
        ];
        let result = eval_collateral_damage_suite(scores, &CollateralDamageConfig::default());
        assert!(result.is_err());
        let failing = result.unwrap_err();
        assert_eq!(failing.benchmark_name, "gsm8k");
    }

    #[test]
    fn suite_passes_when_all_ok() {
        let scores = &[("mmlu", 0.80, 0.78), ("arc", 0.90, 0.88)];
        let result = eval_collateral_damage_suite(scores, &CollateralDamageConfig::default());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }
}

/// Result of evaluating semantic entropy (diversity) of model samples.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SemanticEntropyReport {
    /// Fraction of sampled outputs that are structurally distinct "pseudo-ASTs".
    pub ast_diversity: f64,
    /// Variance in detected language construct counts across samples.
    pub construct_variance: f64,
    /// Whether the entropy is below the collapse warning threshold.
    pub collapse_warning: bool,
}

/// Sample `n` outputs from the model for the same prompt, extract code,
/// and measure structural diversity based on a pseudo-AST hash.
///
/// This avoids a circular dependency on the full vox-compiler by using
/// a regex-based structural "shape" extraction.
pub fn eval_semantic_entropy(outputs: &[String], collapse_threshold: f64) -> SemanticEntropyReport {
    if outputs.is_empty() {
        return SemanticEntropyReport {
            ast_diversity: 0.0,
            construct_variance: 0.0,
            collapse_warning: true,
        };
    }

    let mut unique_hashes = std::collections::HashSet::new();
    let mut construct_counts = Vec::with_capacity(outputs.len());

    // Regexes for stripping content while preserving structure
    let re_str = Regex::new(r#""(?:[^"\\]|\\.)*""#).expect("entropy regex: str");
    let re_num = Regex::new(r"\b\d+(\.\d+)?\b").expect("entropy regex: num");
    let re_ws = Regex::new(r"\s+").expect("entropy regex: ws");

    for out in outputs {
        // Extract code if wrapped in triple-backticks, otherwise treat as raw code
        let code = extract_vox_code(out).unwrap_or_else(|| out.clone());

        // Pseudo-AST: strip literals and normalize whitespace to get the "shape"
        let stripped_str = re_str.replace_all(&code, "\"\"");
        let stripped_num = re_num.replace_all(&stripped_str, "0");
        let pseudo_ast = re_ws.replace_all(&stripped_num, " ").to_string();

        let hash = xxhash_rust::xxh3::xxh3_64(pseudo_ast.as_bytes());
        unique_hashes.insert(hash);

        // Count language constructs for variance analysis
        let constructs = get_vox_constructs();
        let mut count = 0;
        for re in constructs.values() {
            count += re.find_iter(&code).count();
        }
        construct_counts.push(count as f64);
    }

    let ast_diversity = unique_hashes.len() as f64 / outputs.len() as f64;

    // Variance of construct counts
    let mean = construct_counts.iter().sum::<f64>() / construct_counts.len() as f64;
    let variance = construct_counts
        .iter()
        .map(|&c| (c - mean).powi(2))
        .sum::<f64>()
        / construct_counts.len() as f64;

    SemanticEntropyReport {
        ast_diversity,
        construct_variance: variance,
        collapse_warning: ast_diversity < collapse_threshold,
    }
}

/// Heuristic code extractor for triple-backticked blocks.
pub fn extract_vox_code(response: &str) -> Option<String> {
    if let Some(start) = response.find("```vox")
        && let Some(end) = response[start + 6..].find("```")
    {
        return Some(response[start + 6..start + 6 + end].trim().to_string());
    }
    None
}

#[cfg(test)]
mod entropy_tests {
    use super::*;

    #[test]
    fn entropy_detects_monoculture() {
        let samples = vec![
            "fn hello() { return 1 }".to_string(),
            "fn hello() { return 1 }".to_string(),
            "fn hello() { return 1 }".to_string(),
        ];
        let report = eval_semantic_entropy(&samples, 0.5);
        assert!(report.ast_diversity < 0.4);
        assert!(report.collapse_warning);
    }

    #[test]
    fn entropy_detects_diversity() {
        let samples = vec![
            "fn hello() { return 1 }".to_string(),
            "actor World { on msg() { pass } }".to_string(),
            "type Foo = | Bar".to_string(),
        ];
        let report = eval_semantic_entropy(&samples, 0.5);
        assert!(report.ast_diversity > 0.9);
        assert!(!report.collapse_warning);
    }
}
