use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Severity of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARN"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Language that a file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    Python,
    GDScript,
    Vox,
    Unknown,
}

impl Language {
    /// Determine language from a file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Language::Rust,
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" => Language::TypeScript,
            "py" => Language::Python,
            "gd" => Language::GDScript,
            "vox" => Language::Vox,
            _ => Language::Unknown,
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::TypeScript => write!(f, "TypeScript"),
            Language::Python => write!(f, "Python"),
            Language::GDScript => write!(f, "GDScript"),
            Language::Vox => write!(f, "Vox"),
            Language::Unknown => write!(f, "Unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Source file
// ---------------------------------------------------------------------------

/// A loaded source file ready for analysis.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub language: Language,
    pub content: String,
    pub lines: Vec<String>,
}

impl SourceFile {
    pub fn new(path: PathBuf, content: String) -> Self {
        let language = path
            .extension()
            .and_then(|e| e.to_str())
            .map(Language::from_extension)
            .unwrap_or(Language::Unknown);
        let lines = content.lines().map(String::from).collect();
        Self {
            path,
            language,
            content,
            lines,
        }
    }

    /// Return a context snippet around the given 1-indexed line number.
    pub fn context_around(&self, line: usize, radius: usize) -> String {
        let start = line.saturating_sub(radius + 1);
        let end = (line + radius).min(self.lines.len());
        self.lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{:>4} | {}", start + i + 1, l))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------

/// A single detected issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Rule identifier, e.g. `"stub/todo"`.
    pub rule_id: String,
    /// Human-readable rule name.
    pub rule_name: String,
    /// How serious this is.
    pub severity: Severity,
    /// File where the finding was detected.
    pub file: PathBuf,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column (0 if unknown).
    pub column: usize,
    /// Short description of the problem.
    pub message: String,
    /// Optional fix suggestion.
    pub suggestion: Option<String>,
    /// Code context (surrounding lines).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub context: String,
}

// ---------------------------------------------------------------------------
// Detection rule trait
// ---------------------------------------------------------------------------

/// Every detector implements this trait.
pub trait DetectionRule: Send + Sync {
    /// Unique rule identifier, e.g. `"stub/todo"`.
    fn id(&self) -> &'static str;

    /// Human-readable name, e.g. `"Todo Macro Detector"`.
    fn name(&self) -> &'static str;

    /// Short description of what this rule catches.
    fn description(&self) -> &'static str;

    /// Default severity for findings from this rule.
    fn severity(&self) -> Severity;

    /// Languages this rule understands.
    fn languages(&self) -> &[Language];

    /// Run the detector on a single source file.
    fn detect(&self, file: &SourceFile) -> Vec<Finding>;
}
