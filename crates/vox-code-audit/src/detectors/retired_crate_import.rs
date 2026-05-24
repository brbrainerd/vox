use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects references to retired Vox crate names per
/// [`AGENTS.md` §Retired Surfaces (LLM Guard)](../../../../../AGENTS.md).
///
/// Covers two retirement rows from
/// [`contracts/retirement/retired-surfaces.v1.yaml`](../../../../../contracts/retirement/retired-surfaces.v1.yaml):
/// `vox-ludus-crate` (→ `vox-gamify`) and `vox-sherpa-transcribe-plugin`
/// (→ `vox-tauri-stt`). Other crate retirements (vox-dei, vox-ars,
/// merged-compiler-crates) remain guarded by
/// `contracts/documentation/retired-symbols.v1.yaml` and the
/// `vox ci no-dei-import` CLI check; this detector complements those at the
/// code-call-site level.
///
/// File coverage:
///   - Rust source (`.rs`): catches `use vox_ludus::*`, `vox_ludus::` paths.
///   - Cargo.toml: catches `vox-ludus = ...` / `vox_ludus = ...` deps.
///   - Vox source (`.vox`): catches `import vox_ludus` style references.
///
/// Severity: `Warning` at land; escalation to `Error` per CR-L6 ratification.
pub struct RetiredCrateImportDetector {
    /// Rust `use` / path / Cargo dep patterns. Uses underscore form too because
    /// Rust path syntax always uses `_` regardless of the crate-name `-`.
    rust_pattern: Regex,
    /// Cargo.toml line-pattern (key on left of `=`).
    cargo_pattern: Regex,
    /// Vox `import` / package-name pattern.
    vox_pattern: Regex,
}

impl Default for RetiredCrateImportDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RetiredCrateImportDetector {
    pub fn new() -> Self {
        // Words matched: the underscore form (Rust paths) and the dash form
        // (Cargo, docs, Vox imports). Word-boundaries prevent matching inside
        // longer identifiers (e.g. `vox_ludus_compat`).
        Self {
            rust_pattern: Regex::new(r"\bvox_(ludus|sherpa_transcribe)\b").expect("valid regex"),
            cargo_pattern: Regex::new(
                r#"^\s*"?vox-(ludus|sherpa-transcribe)"?\s*="#,
            )
            .expect("valid regex"),
            vox_pattern: Regex::new(r"\bvox[_-](ludus|sherpa[_-]transcribe)\b")
                .expect("valid regex"),
        }
    }

    fn replacement_for(crate_match: &str) -> &'static str {
        // Normalize underscores and dashes to dashes for matching.
        let normalized = crate_match.replace('_', "-");
        match normalized.as_str() {
            "ludus" => "vox-gamify",
            "sherpa-transcribe" => "vox-tauri-stt",
            _ => "(see AGENTS.md §Retired Surfaces)",
        }
    }
}

impl DetectionRule for RetiredCrateImportDetector {
    fn id(&self) -> &'static str {
        "retired/crate-import"
    }

    fn name(&self) -> &'static str {
        "Retired Crate Import Detector"
    }

    fn description(&self) -> &'static str {
        "Detects references to retired Vox crate names (vox-ludus, vox-sherpa-transcribe → vox-tauri-stt)."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        // Languages this detector inspects directly. Cargo.toml is matched via
        // explicit filename check below, regardless of its `Unknown` language.
        &[Language::Rust, Language::Vox]
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::RETIRED_CRATE_IMPORT)
    }

    fn explain(&self) -> &'static str {
        "AGENTS.md §Retired Surfaces lists Vox crates that have been retired in favor of \
canonical replacements. LLMs trained on pre-2026 corpora may emit imports of these crates; \
this detector catches them at the code-call-site level (complementing the markdown text \
guard in contracts/documentation/retired-symbols.v1.yaml).\n\n\
Retired → Canonical:\n\
  vox-ludus               →  vox-gamify\n\
  vox-sherpa-transcribe   →  vox-tauri-stt\n\n\
For `vox-dei`, `vox-ars`, and the merged-compiler crates, see the markdown text guard."
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Check Cargo.toml by filename regardless of detected language.
        let is_cargo_toml = file
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n == "Cargo.toml");

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comments (Rust //, Vox //, TOML/Cargo #).
            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            let (matched, message_suffix) = match (file.language, is_cargo_toml) {
                (_, true) => {
                    if let Some(caps) = self.cargo_pattern.captures(line) {
                        let crate_match = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                        (
                            Some(caps.get(0).expect("group 0")),
                            format!(
                                "Retired Cargo dependency `vox-{crate_match}` — use `{}` instead.",
                                Self::replacement_for(crate_match)
                            ),
                        )
                    } else {
                        continue;
                    }
                }
                (Language::Rust, _) => {
                    if let Some(caps) = self.rust_pattern.captures(line) {
                        let crate_match = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                        (
                            Some(caps.get(0).expect("group 0")),
                            format!(
                                "Retired crate reference `vox_{crate_match}` — use `{}` instead.",
                                Self::replacement_for(crate_match)
                            ),
                        )
                    } else {
                        continue;
                    }
                }
                (Language::Vox, _) => {
                    if let Some(caps) = self.vox_pattern.captures(line) {
                        let crate_match = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                        (
                            Some(caps.get(0).expect("group 0")),
                            format!(
                                "Retired crate reference — use `{}` instead.",
                                Self::replacement_for(crate_match)
                            ),
                        )
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            let Some(m) = matched else { continue };
            findings.push(Finding {
                rule_id: self.id().to_string(),
                diagnostic_id: self.diagnostic_id().map(str::to_string),
                rule_name: self.name().to_string(),
                severity: Severity::Warning,
                file: file.path.clone(),
                line: line_num,
                column: m.start() + 1,
                message: message_suffix,
                suggestion: Some(
                    "Migrate to the canonical crate per AGENTS.md §Retired Surfaces. \
                     Vestigial call-sites during migration MUST carry a \
                     `// vox-deprecated-since=...` annotation."
                        .to_string(),
                ),
                alternatives: vec![],
                rationale: Some(
                    "Retired crates are removed from the workspace dependency surface in a \
                     future minor release. LLMs trained on pre-2026 corpora may emit them \
                     by habit; catching at the call-site lets the agent rewrite immediately."
                        .to_string(),
                ),
                context: file.context_around(line_num, 2),
                confidence: Some(FindingConfidence::High),
                evidence: None,
            });
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rust_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.rs"), code.to_string())
    }

    fn cargo_toml(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("Cargo.toml"), code.to_string())
    }

    fn vox_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_vox_ludus_use_in_rust() {
        let d = RetiredCrateImportDetector::new();
        let f = rust_source("use vox_ludus::Engine;");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("vox_ludus"));
        assert!(findings[0].message.contains("vox-gamify"));
    }

    #[test]
    fn flags_vox_sherpa_transcribe_path_in_rust() {
        let d = RetiredCrateImportDetector::new();
        let f = rust_source("let _ = vox_sherpa_transcribe::start();");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("vox-tauri-stt"));
    }

    #[test]
    fn flags_vox_ludus_dep_in_cargo_toml() {
        let d = RetiredCrateImportDetector::new();
        let f = cargo_toml("vox-ludus = \"0.5\"");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("vox-ludus"));
    }

    #[test]
    fn flags_quoted_vox_sherpa_transcribe_dep_in_cargo_toml() {
        let d = RetiredCrateImportDetector::new();
        let f = cargo_toml("\"vox-sherpa-transcribe\" = { workspace = true }");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("vox-tauri-stt"));
    }

    #[test]
    fn does_not_flag_vox_gamify_canonical() {
        let d = RetiredCrateImportDetector::new();
        let f = rust_source("use vox_gamify::Engine;");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "canonical crate must not fire");
    }

    #[test]
    fn does_not_flag_longer_identifier_containing_ludus() {
        let d = RetiredCrateImportDetector::new();
        // `vox_ludus_compat` and `vox_ludusish` should not match due to \b word boundary.
        let f = rust_source("use vox_ludus_compat::Engine;\nlet x = vox_ludusish();");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "word boundary should exclude prefix substrings"
        );
    }

    #[test]
    fn flags_vox_ludus_in_vox_source() {
        let d = RetiredCrateImportDetector::new();
        let f = vox_source("import vox_ludus");
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_comment_lines_in_rust() {
        let d = RetiredCrateImportDetector::new();
        let f = rust_source("// use vox_ludus::Engine;");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_comment_lines_in_cargo_toml() {
        let d = RetiredCrateImportDetector::new();
        let f = cargo_toml("# vox-ludus = \"0.5\"");
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn finding_diagnostic_id_is_stable() {
        let d = RetiredCrateImportDetector::new();
        let f = rust_source("use vox_ludus::Engine;");
        let findings = d.detect(&f, None);
        assert_eq!(
            findings[0].diagnostic_id.as_deref(),
            Some(catalog::RETIRED_CRATE_IMPORT)
        );
    }
}
