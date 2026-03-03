use std::collections::HashMap;

use similar::{ChangeTag, TextDiff};

use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

/// Detects near-duplicate code blocks across a single file.
///
/// Uses the `similar` crate to compute text similarity between function bodies.
/// Cross-file DRY detection is a Phase 2 feature (requires the engine to pass
/// multiple files to a single rule invocation).
pub struct DryViolationDetector {
    /// Minimum similarity ratio (0.0–1.0) to flag as a DRY violation.
    similarity_threshold: f64,
    /// Minimum number of lines for a block to be considered.
    min_block_lines: usize,
}

impl Default for DryViolationDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DryViolationDetector {
    pub fn new() -> Self {
        Self {
            similarity_threshold: 0.80,
            min_block_lines: 5,
        }
    }

    fn extract_blocks(&self, file: &SourceFile) -> Vec<CodeBlock> {
        match file.language {
            Language::Rust => self.extract_rust_blocks(file),
            Language::TypeScript => self.extract_brace_blocks(file),
            Language::Python => self.extract_python_blocks(file),
            _ => Vec::new(),
        }
    }

    fn extract_rust_blocks(&self, file: &SourceFile) -> Vec<CodeBlock> {
        self.extract_brace_blocks(file)
    }

    /// Extract function-level blocks delimited by braces.
    fn extract_brace_blocks(&self, file: &SourceFile) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let mut i = 0;
        while i < file.lines.len() {
            let line = file.lines[i].trim();
            // Look for function/method declarations
            let is_fn = line.starts_with("fn ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub(crate) fn ")
                || line.starts_with("async fn ")
                || line.starts_with("pub async fn ")
                || line.starts_with("function ")
                || line.starts_with("export function ")
                || line.starts_with("export async function ")
                || line.starts_with("async function ");

            if is_fn {
                if let Some((start, end)) = self.find_brace_body_range(file, i) {
                    if end - start >= self.min_block_lines {
                        let body: String = file.lines[start..end]
                            .iter()
                            .map(|l| l.trim())
                            .collect::<Vec<_>>()
                            .join("\n");
                        blocks.push(CodeBlock {
                            start_line: i + 1,
                            end_line: end + 1,
                            body,
                            header: file.lines[i].trim().to_string(),
                        });
                    }
                    i = end + 1;
                    continue;
                }
            }
            i += 1;
        }
        blocks
    }

    fn extract_python_blocks(&self, file: &SourceFile) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let mut i = 0;
        while i < file.lines.len() {
            let line = file.lines[i].trim_start();
            if line.starts_with("def ") || line.starts_with("async def ") {
                let indent = file.lines[i].len() - file.lines[i].trim_start().len();
                let start = i + 1;
                let mut end = start;
                // Read indented body
                while end < file.lines.len() {
                    let next = &file.lines[end];
                    if next.trim().is_empty() {
                        end += 1;
                        continue;
                    }
                    let next_indent = next.len() - next.trim_start().len();
                    if next_indent <= indent {
                        break;
                    }
                    end += 1;
                }
                if end - start >= self.min_block_lines {
                    let body: String = file.lines[start..end]
                        .iter()
                        .map(|l| l.trim())
                        .collect::<Vec<_>>()
                        .join("\n");
                    blocks.push(CodeBlock {
                        start_line: i + 1,
                        end_line: end,
                        body,
                        header: file.lines[i].trim().to_string(),
                    });
                }
                i = end;
                continue;
            }
            i += 1;
        }
        blocks
    }

    fn find_brace_body_range(&self, file: &SourceFile, start: usize) -> Option<(usize, usize)> {
        let mut depth = 0i32;
        let mut body_start = None;
        for j in start..file.lines.len() {
            for ch in file.lines[j].chars() {
                if ch == '{' {
                    if depth == 0 {
                        body_start = Some(j + 1);
                    }
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return body_start.map(|s| (s, j));
                    }
                }
            }
        }
        None
    }

    /// Compute similarity ratio between two strings (0.0 to 1.0).
    fn similarity(a: &str, b: &str) -> f64 {
        let diff = TextDiff::from_lines(a, b);
        let mut same = 0usize;
        let mut total = 0usize;
        for change in diff.iter_all_changes() {
            total += 1;
            if change.tag() == ChangeTag::Equal {
                same += 1;
            }
        }
        if total == 0 {
            return 0.0;
        }
        same as f64 / total as f64
    }
}

#[allow(dead_code)]
struct CodeBlock {
    start_line: usize,
    end_line: usize,
    body: String,
    header: String,
}

impl DetectionRule for DryViolationDetector {
    fn id(&self) -> &'static str {
        "dry-violation"
    }
    fn name(&self) -> &'static str {
        "DRY Violation Detector"
    }
    fn description(&self) -> &'static str {
        "Detects near-duplicate code blocks within the same file"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::TypeScript, Language::Python]
    }

    fn detect(&self, file: &SourceFile) -> Vec<Finding> {
        let blocks = self.extract_blocks(file);
        let mut findings = Vec::new();
        // Track which pairs we've already flagged
        let mut flagged: HashMap<(usize, usize), bool> = HashMap::new();

        for i in 0..blocks.len() {
            for j in (i + 1)..blocks.len() {
                if flagged.contains_key(&(i, j)) {
                    continue;
                }
                let sim = Self::similarity(&blocks[i].body, &blocks[j].body);
                if sim >= self.similarity_threshold {
                    flagged.insert((i, j), true);
                    findings.push(Finding {
                        rule_id: "dry-violation/duplicate-block".to_string(),
                        rule_name: self.name().to_string(),
                        severity: self.severity(),
                        file: file.path.clone(),
                        line: blocks[i].start_line,
                        column: 0,
                        message: format!(
                            "Near-duplicate code block ({:.0}% similar) — `{}` (L{}) and `{}` (L{})",
                            sim * 100.0,
                            blocks[i].header,
                            blocks[i].start_line,
                            blocks[j].header,
                            blocks[j].start_line,
                        ),
                        suggestion: Some(
                            "Extract the common logic into a shared function to avoid DRY violations."
                                .to_string(),
                        ),
                        context: file.context_around(blocks[i].start_line, 2),
                    });
                }
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{}", ext)), code.to_string())
    }

    #[test]
    fn similarity_computation() {
        let a = "line1\nline2\nline3\nline4\nline5";
        let b = "line1\nline2\nline3\nline4\nline5";
        assert!((DryViolationDetector::similarity(a, b) - 1.0).abs() < f64::EPSILON);

        let c = "completely\ndifferent\ncontent\nhere\nnow";
        assert!(DryViolationDetector::similarity(a, c) < 0.5);
    }

    #[test]
    fn detects_duplicate_blocks() {
        let d = DryViolationDetector::new();
        let code = r#"
fn process_alpha(items: Vec<i32>) -> Vec<i32> {
    let mut result = Vec::new();
    for item in items {
        if item > 0 {
            result.push(item * 2);
        }
    }
    result
}

fn process_beta(items: Vec<i32>) -> Vec<i32> {
    let mut result = Vec::new();
    for item in items {
        if item > 0 {
            result.push(item * 2);
        }
    }
    result
}
"#;
        let f = source("rs", code);
        let findings = d.detect(&f);
        assert!(!findings.is_empty(), "should detect duplicate blocks");
    }

    #[test]
    fn no_findings_for_unique_fns() {
        let d = DryViolationDetector::new();
        let code = r#"
fn add(a: i32, b: i32) -> i32 {
    let sum = a + b;
    println!("Result: {}", sum);
    if sum > 100 {
        return sum - 100;
    }
    sum
}

fn multiply(a: i32, b: i32) -> i32 {
    let product = a * b;
    if product < 0 {
        panic!("negative!");
    }
    let adjusted = product + 1;
    adjusted
}
"#;
        let f = source("rs", code);
        let findings = d.detect(&f);
        assert!(findings.is_empty(), "unique functions should not flag DRY");
    }
}
