use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects call-sites to retired memory-API names (`recall()` / `recall_async()`).
///
/// Covers the `recall-fn-api` row in
/// [`contracts/retirement/retired-surfaces.v1.yaml`](../../../../../contracts/retirement/retired-surfaces.v1.yaml).
/// The markdown text guard at
/// [`contracts/documentation/retired-symbols.v1.yaml`](../../../../../contracts/documentation/retired-symbols.v1.yaml)
/// already prevents docs drift; this detector complements it at the
/// call-site level.
///
/// Per AGENTS.md §Retired Surfaces:
///   recall() / recall_async()  →  MemoryManager::lookup_fact_by_key (async)
///                                  or RAG / retrieval bundle —
///                                  see crates/vox-orchestrator/src/memory/manager.rs
///
/// Severity: `Warning`. The detector intentionally only fires on call shapes
/// (`recall(`, `recall_async(`) rather than identifier mentions, to avoid
/// false positives on local variable names or unrelated functions.
pub struct RetiredMemoryApiDetector {
    /// Matches `recall(...)` or `recall_async(...)` call-sites in Rust.
    /// Word-boundary before `recall` excludes `recalls(` and `recalling(`.
    call_pattern: Regex,
}

impl Default for RetiredMemoryApiDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RetiredMemoryApiDetector {
    pub fn new() -> Self {
        Self {
            call_pattern: Regex::new(r"\b(recall|recall_async)\s*\(").expect("valid regex"),
        }
    }
}

impl DetectionRule for RetiredMemoryApiDetector {
    fn id(&self) -> &'static str {
        "retired/memory-api"
    }

    fn name(&self) -> &'static str {
        "Retired Memory API Detector"
    }

    fn description(&self) -> &'static str {
        "Detects call-sites to retired `recall()` / `recall_async()` memory-API names."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::RETIRED_MEMORY_API)
    }

    fn explain(&self) -> &'static str {
        "AGENTS.md §Retired Surfaces lists `recall()` / `recall_async()` as retired memory-API \
names. The canonical replacement is `MemoryManager::lookup_fact_by_key` (async) for direct \
key lookups, or a RAG / retrieval bundle for higher-level queries — see \
`crates/vox-orchestrator/src/memory/manager.rs`.\n\n\
This detector fires on call shapes only (`recall(` / `recall_async(`); identifier \
mentions in comments or unrelated local-variable names are not flagged."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — retired memory API call\n\
             let fact = recall(\"user.preference.theme\");\n\
             \n\
             // FIX — use MemoryManager\n\
             let fact = memory_manager.lookup_fact_by_key(\"user.preference.theme\").await?;",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Rust {
            return vec![];
        }

        // The MemoryManager crate itself owns the canonical replacement; if it
        // legitimately defines or wraps `recall(`, we don't want the detector
        // to fire on its own implementation. Heuristic: skip files whose path
        // contains `memory/manager` (the canonical owner per AGENTS.md hint).
        let path_str = file.path.to_string_lossy();
        if path_str.contains("memory/manager") || path_str.contains("memory\\manager") {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Skip lines that talk ABOUT the retirement.
            if trimmed.contains("retired") || trimmed.contains("vox-deprecated-since") {
                continue;
            }

            if let Some(caps) = self.call_pattern.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    message: format!(
                        "Retired memory-API call `{name}(...)` — use \
                         `MemoryManager::lookup_fact_by_key` (async) or a RAG retrieval bundle."
                    ),
                    suggestion: Some(
                        "See crates/vox-orchestrator/src/memory/manager.rs for the canonical \
                         async lookup API. For higher-level queries, prefer the retrieval bundle \
                         pattern under `vox-search`."
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: Some(
                        "The async lookup_fact_by_key API replaced sync `recall()` to remove a \
                         hidden blocking call in agent loops. RAG retrieval bundles replace the \
                         hand-rolled `recall_async()` shape for higher-level queries."
                            .to_string(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rust(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.rs"), code.to_string())
    }

    #[test]
    fn flags_recall_call_in_rust() {
        let d = RetiredMemoryApiDetector::new();
        let f = rust("let v = recall(\"key\");");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("recall(..."));
    }

    #[test]
    fn flags_recall_async_call() {
        let d = RetiredMemoryApiDetector::new();
        let f = rust("let v = recall_async(\"key\").await;");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("recall_async(..."));
    }

    #[test]
    fn does_not_flag_word_recalls() {
        let d = RetiredMemoryApiDetector::new();
        let f = rust("fn recalls_count() -> usize { 0 }\nlet n = recalls_count();");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "word boundary excludes `recalls(`");
    }

    #[test]
    fn does_not_flag_lookup_fact_by_key_canonical() {
        let d = RetiredMemoryApiDetector::new();
        let f = rust(
            "let v = MemoryManager::lookup_fact_by_key(\"key\").await;",
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_fire_inside_canonical_memory_manager_file() {
        let d = RetiredMemoryApiDetector::new();
        let f = SourceFile::new(
            PathBuf::from("crates/vox-orchestrator/src/memory/manager.rs"),
            "fn recall(key: &str) -> Option<String> { todo!() }".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "the canonical owner of recall is exempt"
        );
    }

    #[test]
    fn ignores_comment_lines() {
        let d = RetiredMemoryApiDetector::new();
        let f = rust("// recall(\"key\") is retired");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_fire_on_non_rust_files() {
        let d = RetiredMemoryApiDetector::new();
        let f = SourceFile::new(
            PathBuf::from("test.vox"),
            "recall(\"key\")".to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "Vox source has different semantics");
    }
}
