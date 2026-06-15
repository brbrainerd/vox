use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::analysis::RustFileContext;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Severity of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational note or style suggestion.
    Info,
    /// Likely issue or risk worth addressing.
    Warning,
    /// Definite problem or policy violation.
    Error,
    /// Severe issue such as security or crash risk.
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
    /// Rust source (`.rs`).
    Rust,
    /// JavaScript/TypeScript (`.ts`, `.tsx`, `.js`, …).
    TypeScript,
    /// Python (`.py`).
    Python,
    /// Godot GDScript (`.gd`).
    GDScript,
    /// Vox source (`.vox`).
    Vox,
    /// Extension did not map to a known language.
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
    /// Path to the file on disk.
    pub path: PathBuf,
    /// Detected language from the path extension.
    pub language: Language,
    /// Full file text.
    pub content: String,
    /// Lines of `content`, one string per line (no trailing `\n`).
    pub lines: Vec<String>,
}

impl SourceFile {
    /// Loads metadata and splits `content` into lines; language is inferred from `path`.
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
    /// Stable diagnostic ID following `vox/<category>/<name>` scheme.
    /// New detectors always populate this; legacy detectors may leave it `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
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
    /// Optional fix suggestion (primary).
    pub suggestion: Option<String>,
    /// Alternative fix suggestions (when more than one valid approach exists).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    /// Constant per-rule prose explaining *why* this rule exists.
    /// Stable across occurrences — useful for `--explain` and LLM rationale fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Code context (surrounding lines).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub context: String,
    /// Detector-estimated confidence (e.g. heuristic vs AST-backed); omit when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<FindingConfidence>,
    /// Optional structured explain payload (per-rule), e.g. import candidates for unresolved-ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
}

/// Qualitative confidence for a finding (policy / reporting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingConfidence {
    High,
    Medium,
    Low,
}

impl Finding {
    /// Stable tie-breaker for deterministic ordering (path, line, rule, message hash).
    pub fn deterministic_key(&self) -> (PathBuf, usize, String, u64) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.message.hash(&mut h);
        (
            self.file.clone(),
            self.line,
            self.rule_id.clone(),
            h.finish(),
        )
    }

    /// Stable fingerprint for dedup / carry-forward caches (path + line + rule + message).
    pub fn fingerprint(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.file.hash(&mut h);
        self.line.hash(&mut h);
        self.column.hash(&mut h);
        self.rule_id.hash(&mut h);
        self.message.hash(&mut h);
        h.finish()
    }
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
    ///
    /// For Rust, `rust_ctx` is built once per file by the engine.
    fn detect(&self, file: &SourceFile, rust_ctx: Option<&RustFileContext>) -> Vec<Finding>;

    /// Stable `vox/<category>/<name>` diagnostic ID for this rule.
    /// Returns `None` for legacy detectors that pre-date the ID scheme.
    fn diagnostic_id(&self) -> Option<&'static str> {
        None
    }

    /// Prose explanation of *why* this rule exists, shown by `vox check --explain <id>`.
    /// Should include a "Bad" and "Good" example whenever practical.
    fn explain(&self) -> &'static str {
        ""
    }

    /// Minimal code snippet that reproduces a typical finding from this rule (≤ 8 lines).
    ///
    /// Included in `LintFindingPayload::minimal_repro` when present,
    /// so LLM repair agents can understand the violation without needing the original file.
    ///
    /// Rules that detect something self-evident from the `message` field may return `None`.
    fn minimal_repro(&self) -> Option<&'static str> {
        None
    }
}

// ---------------------------------------------------------------------------
// Shared line scanning (detectors)
// ---------------------------------------------------------------------------

/// Byte offset in `content` for the start of `line_1_indexed` (1-based), then `column_0` adds
/// columns within that line (byte offset, not Unicode scalar index).
pub(crate) fn byte_offset_in_file(content: &str, line_1_indexed: usize, column_0: usize) -> usize {
    if line_1_indexed == 0 {
        return column_0.min(content.len());
    }
    let mut off = 0usize;
    for (i, line) in content.lines().enumerate() {
        if i + 1 == line_1_indexed {
            return (off + column_0).min(content.len());
        }
        off = off.saturating_add(line.len()).saturating_add(1);
    }
    off.min(content.len())
}

/// True if the byte at (`line`, `column_0` within line) is inside comment or string (Rust).
#[inline]
pub(crate) fn rust_byte_is_non_code(
    file: &SourceFile,
    line_1_indexed: usize,
    column_0: usize,
    rust_ctx: Option<&RustFileContext>,
) -> bool {
    if file.language != Language::Rust {
        return false;
    }
    let abs = byte_offset_in_file(&file.content, line_1_indexed, column_0);
    match rust_ctx {
        Some(c) => c.token_map.is_non_code_byte(abs),
        None => crate::analysis::TokenMap::from_rust_source(&file.content).is_non_code_byte(abs),
    }
}

/// True if the byte is inside a **comment** only (not a string). Used so secrets still match inside literals.
#[inline]
pub(crate) fn rust_byte_is_comment(
    file: &SourceFile,
    line_1_indexed: usize,
    column_0: usize,
    rust_ctx: Option<&RustFileContext>,
) -> bool {
    if file.language != Language::Rust {
        return false;
    }
    let abs = byte_offset_in_file(&file.content, line_1_indexed, column_0);
    match rust_ctx {
        Some(c) => c.token_map.is_comment_byte(abs),
        None => crate::analysis::TokenMap::from_rust_source(&file.content).is_comment_byte(abs),
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;
    use std::path::PathBuf;

    // Catches: a new/removed extension alias silently remapping a language
    // (e.g. dropping "mjs"→TypeScript or mis-mapping "gd").
    #[test]
    fn from_extension_maps_known_and_unknown() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("mjs"), Language::TypeScript);
        assert_eq!(Language::from_extension("gd"), Language::GDScript);
        assert_eq!(Language::from_extension("vox"), Language::Vox);
        assert_eq!(Language::from_extension("toml"), Language::Unknown);
    }

    // Catches: off-by-one in the radius window or wrong 1-indexed gutter
    // numbering in context_around (start/end clamping bug).
    #[test]
    fn context_around_clamps_and_numbers_from_one() {
        let f = SourceFile::new(PathBuf::from("a.rs"), "l1\nl2\nl3".to_string());
        // radius 1 around line 1 => lines 1..=2, clamped at the top.
        assert_eq!(f.context_around(1, 1), "   1 | l1\n   2 | l2");
    }
}
