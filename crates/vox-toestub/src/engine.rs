use std::path::PathBuf;

use crate::detectors;
use crate::report::{OutputFormat, Reporter};
use crate::rules::{DetectionRule, Finding, Language, Severity};
use crate::scanner::Scanner;
use crate::task_queue::TaskQueue;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a TOESTUB analysis run.
#[derive(Debug, Clone)]
pub struct ToestubConfig {
    /// Root directories to scan.
    pub roots: Vec<PathBuf>,
    /// Minimum severity to include in the report.
    pub min_severity: Severity,
    /// Output format.
    pub format: OutputFormat,
    /// Whether to generate fix suggestions.
    pub suggest_fixes: bool,
    /// Language filter (None = all languages).
    pub languages: Option<Vec<Language>>,
    /// Extra glob patterns to exclude.
    pub excludes: Vec<String>,
    /// Specific rule IDs to run (None = all rules).
    pub rule_filter: Option<Vec<String>>,
    /// Path to vox-schema.json for architectural validation.
    pub schema_path: Option<PathBuf>,
    /// Path to unwired.json for scope-aware prioritization.
    pub unwired_path: Option<PathBuf>,
}

impl Default for ToestubConfig {
    fn default() -> Self {
        Self {
            roots: vec![PathBuf::from(".")],
            min_severity: Severity::Warning,
            format: OutputFormat::Terminal,
            suggest_fixes: false,
            languages: None,
            excludes: Vec::new(),
            rule_filter: None,
            schema_path: None,
            unwired_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The main analysis engine.
pub struct ToestubEngine {
    rules: Vec<Box<dyn DetectionRule>>,
    config: ToestubConfig,
}

impl ToestubEngine {
    /// Create a new engine with the given config, loading all built-in rules.
    pub fn new(config: ToestubConfig) -> Self {
        let mut rules = detectors::all_rules(config.schema_path.clone());

        // Apply rule filter if specified
        if let Some(ref filter) = config.rule_filter {
            rules.retain(|r| filter.iter().any(|f| r.id().starts_with(f.as_str())));
        }

        Self { rules, config }
    }

    /// Create an engine with a custom set of rules (useful for testing).
    pub fn with_rules(config: ToestubConfig, rules: Vec<Box<dyn DetectionRule>>) -> Self {
        Self { rules, config }
    }

    /// Run the full analysis pipeline and return findings.
    pub fn run(&self) -> AnalysisResult {
        // 1. Scan for source files
        let scanner = Scanner::new(
            self.get_roots(),
            &self.config.excludes,
            self.config.languages.clone(),
        );
        let files = scanner.scan();

        // 2. Run each rule on each file
        let mut all_findings: Vec<Finding> = Vec::new();
        for file in &files {
            for rule in &self.rules {
                // Skip rules that don't apply to this language
                if !rule.languages().contains(&file.language) {
                    continue;
                }
                let findings = rule.detect(file);
                all_findings.extend(findings);
            }
        }

        // 3. Filter by severity
        all_findings.retain(|f| f.severity >= self.config.min_severity);

        // 4. Sort by severity (critical first), then by file, then by line
        all_findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| a.line.cmp(&b.line))
        });

        // 5. Build the task queue
        let task_queue = if self.config.suggest_fixes {
            TaskQueue::from_findings(&all_findings)
        } else {
            TaskQueue::empty()
        };

        AnalysisResult {
            files_scanned: files.len(),
            rules_applied: self.rules.len(),
            findings: all_findings,
            task_queue,
        }
    }

    /// Run analysis and produce formatted output as a String.
    pub fn run_and_report(&self) -> (AnalysisResult, String) {
        let result = self.run();
        let output = Reporter::format(&result.findings, self.config.format, &result.task_queue);
        (result, output)
    }

    fn get_roots(&self) -> Vec<PathBuf> {
        if let Some(ref path) = self.config.unwired_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(roots) = json.get("roots").and_then(|r| r.as_array()) {
                        return roots
                            .iter()
                            .filter_map(|r| r.as_str().map(PathBuf::from))
                            .collect();
                    }
                }
            }
        }
        self.config.roots.clone()
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// The output of a TOESTUB analysis run.
pub struct AnalysisResult {
    /// Number of source files scanned.
    pub files_scanned: usize,
    /// Number of detection rules applied.
    pub rules_applied: usize,
    /// All findings (already filtered and sorted).
    pub findings: Vec<Finding>,
    /// Generated task queue with fix suggestions.
    pub task_queue: TaskQueue,
}

impl AnalysisResult {
    /// Returns `true` if any findings are at or above [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity >= Severity::Error)
    }

    /// Summary counts by severity.
    pub fn summary(&self) -> SeveritySummary {
        let mut s = SeveritySummary::default();
        for f in &self.findings {
            match f.severity {
                Severity::Info => s.info += 1,
                Severity::Warning => s.warning += 1,
                Severity::Error => s.error += 1,
                Severity::Critical => s.critical += 1,
            }
        }
        s
    }
}

#[derive(Debug, Default)]
pub struct SeveritySummary {
    pub info: usize,
    pub warning: usize,
    pub error: usize,
    pub critical: usize,
}
