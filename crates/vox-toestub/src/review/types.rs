use std::path::PathBuf;
use std::fmt;
use serde::{Deserialize, Serialize};
use crate::rules::{Finding, Severity};

/// Configuration for a single review run.
#[derive(Debug, Clone)]
pub struct ReviewConfig {
    /// Files or directories to review (diff mode if git available, else full file).
    pub targets: Vec<PathBuf>,
    /// Override the provider instead of auto-discovery.
    pub provider_override: Option<super::providers::ReviewProvider>,
    /// Only use free providers (Ollama, Pollinations, Gemini).
    pub free_only: bool,
    /// Maximum number of tokens to send as context per file.
    pub max_context_tokens: usize,
    /// Output format.
    pub output_format: ReviewOutputFormat,
    /// Minimum severity to surface.
    pub min_severity: Severity,
    /// Whether to use git diff for context (only changed lines + surroundings).
    pub use_git_diff: bool,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            targets: vec![PathBuf::from(".")],
            provider_override: None,
            free_only: false,
            max_context_tokens: 6000,
            output_format: ReviewOutputFormat::Terminal,
            min_severity: Severity::Warning,
            use_git_diff: true,
        }
    }
}

/// Output format for review results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutputFormat {
    Terminal,
    Json,
    Sarif,
    Markdown,
}

impl ReviewOutputFormat {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => ReviewOutputFormat::Json,
            "sarif" => ReviewOutputFormat::Sarif,
            "markdown" | "md" => ReviewOutputFormat::Markdown,
            _ => ReviewOutputFormat::Terminal,
        }
    }
}

/// The output of a review run.
#[derive(Debug)]
pub struct ReviewResult {
    pub files_reviewed: usize,
    pub provider_used: String,
    pub findings: Vec<ReviewFinding>,
    pub cost_estimate_usd: f64,
    pub tokens_used: usize,
}

impl ReviewResult {
    /// Returns true if any finding is Error-level or above.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity >= Severity::Error)
    }

    /// Convert review findings to TOESTUB `Finding` structs for unified reporting.
    pub fn to_findings(&self) -> Vec<Finding> {
        self.findings
            .iter()
            .map(|rf| Finding {
                rule_id: format!("review/{}", rf.category.rule_prefix()),
                rule_name: format!("AI Code Review — {}", rf.category),
                severity: rf.severity,
                file: rf.file.clone(),
                line: rf.line,
                column: 0,
                message: rf.message.clone(),
                suggestion: rf.suggestion.clone(),
                context: String::new(),
            })
            .collect()
    }
}

/// A single issue identified during review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub category: ReviewCategory,
    pub severity: Severity,
    pub file: PathBuf,
    pub line: usize,
    pub message: String,
    pub suggestion: Option<String>,
    /// Confidence 0–100 from the LLM.
    pub confidence: u8,
}

/// Issue category, mirrors CodeRabbit's classification taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCategory {
    /// Logic errors, off-by-one, incorrect control flow, flawed conditions.
    Logic,
    /// Security vulnerabilities: injection, XSS, secrets, insecure refs.
    Security,
    /// Missing error handling: unchecked results, empty catch blocks.
    ErrorHandling,
    /// Performance: N+1 queries, excessive I/O, unnecessary allocations.
    Performance,
    /// Dead code, stubs, incomplete implementations, unreachable branches.
    DeadCode,
    /// Naming inconsistencies, formatting, readability, documentation.
    Style,
    /// Vox-specific: null safety, scope leaks, Option/Result misuse.
    VoxSpecific,
    /// Dependency and import issues.
    Dependencies,
}

impl ReviewCategory {
    pub fn rule_prefix(self) -> &'static str {
        match self {
            ReviewCategory::Logic => "logic",
            ReviewCategory::Security => "security",
            ReviewCategory::ErrorHandling => "error-handling",
            ReviewCategory::Performance => "performance",
            ReviewCategory::DeadCode => "dead-code",
            ReviewCategory::Style => "style",
            ReviewCategory::VoxSpecific => "vox",
            ReviewCategory::Dependencies => "deps",
        }
    }

    pub fn parse_category(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "logic" | "correctness" => ReviewCategory::Logic,
            "security" | "sec" => ReviewCategory::Security,
            "error_handling" | "error-handling" | "errors" => ReviewCategory::ErrorHandling,
            "performance" | "perf" => ReviewCategory::Performance,
            "dead_code" | "dead-code" | "stub" => ReviewCategory::DeadCode,
            "style" | "naming" | "readability" => ReviewCategory::Style,
            "vox" | "vox_specific" => ReviewCategory::VoxSpecific,
            "deps" | "dependencies" | "imports" => ReviewCategory::Dependencies,
            _ => ReviewCategory::Logic,
        }
    }
}

impl fmt::Display for ReviewCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewCategory::Logic => write!(f, "Logic & Correctness"),
            ReviewCategory::Security => write!(f, "Security"),
            ReviewCategory::ErrorHandling => write!(f, "Error Handling"),
            ReviewCategory::Performance => write!(f, "Performance"),
            ReviewCategory::DeadCode => write!(f, "Dead Code / Stubs"),
            ReviewCategory::Style => write!(f, "Style & Naming"),
            ReviewCategory::VoxSpecific => write!(f, "Vox-Specific"),
            ReviewCategory::Dependencies => write!(f, "Dependencies"),
        }
    }
}
