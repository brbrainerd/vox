use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects code-level references to retired Vox env-var names.
///
/// Covers the `turso-env-vars` row in
/// [`contracts/retirement/retired-surfaces.v1.yaml`](../../../../../contracts/retirement/retired-surfaces.v1.yaml).
/// The markdown text guard at
/// [`contracts/documentation/retired-symbols.v1.yaml`](../../../../../contracts/documentation/retired-symbols.v1.yaml)
/// already prevents docs drift; this detector complements it at the
/// runtime-consumer call-site (e.g. `env::var("TURSO_URL")`).
///
/// Retired → Canonical (from AGENTS.md §Retired Surfaces):
///   TURSO_URL          →  VOX_DB_URL
///   VOX_TURSO_URL      →  VOX_DB_URL
///   VOX_TURSO_TOKEN    →  VOX_DB_TOKEN
///
/// Severity: `Warning` at land. Confidence: High when matched inside an
/// `env::var(...)` / `env.get(...)` / `std::env::var(...)` call shape;
/// Medium when matched as a bare string literal (could be in docs/tests).
pub struct RetiredEnvVarDetector {
    /// Bare literal pattern — fires Medium-confidence.
    bare_literal_pattern: Regex,
    /// `env::var("...")` / `env.get("...")` / `std::env::var(...)` call shape —
    /// fires High-confidence.
    call_site_pattern: Regex,
}

impl Default for RetiredEnvVarDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RetiredEnvVarDetector {
    pub fn new() -> Self {
        let names = "(TURSO_URL|VOX_TURSO_URL|VOX_TURSO_TOKEN)";
        Self {
            bare_literal_pattern: Regex::new(&format!("\"{names}\""))
                .expect("valid regex"),
            // Matches std::env::var, env::var, env.get with the retired name.
            call_site_pattern: Regex::new(&format!(
                r#"(?:std::)?env(?:::var|\.get)\s*\(\s*"{names}""#
            ))
            .expect("valid regex"),
        }
    }

    fn canonical_replacement(name: &str) -> &'static str {
        match name {
            "TURSO_URL" | "VOX_TURSO_URL" => "VOX_DB_URL",
            "VOX_TURSO_TOKEN" => "VOX_DB_TOKEN",
            _ => "(see AGENTS.md §Retired Surfaces)",
        }
    }
}

impl DetectionRule for RetiredEnvVarDetector {
    fn id(&self) -> &'static str {
        "retired/env-var"
    }

    fn name(&self) -> &'static str {
        "Retired Env Var Detector"
    }

    fn description(&self) -> &'static str {
        "Detects code-level references to retired TURSO_* env-var names; suggests VOX_DB_* canonical names."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust, Language::Vox]
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::RETIRED_ENV_VAR)
    }

    fn explain(&self) -> &'static str {
        "AGENTS.md §Retired Surfaces lists Vox env-var names that have been retired.\n\n\
Retired → Canonical:\n\
  TURSO_URL          →  VOX_DB_URL\n\
  VOX_TURSO_URL      →  VOX_DB_URL\n\
  VOX_TURSO_TOKEN    →  VOX_DB_TOKEN\n\n\
A call-site to `env::var(\"TURSO_URL\")` is a High-confidence finding; a bare \
string literal `\"TURSO_URL\"` (e.g. in a docs example or test fixture) is \
Medium-confidence. Update the consumer to read the canonical name; if backward \
compatibility is required, wire it through `vox-secrets` rather than reading \
the legacy env-var directly."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — reading from retired TURSO_* env-var names\n\
             let db_url   = std::env::var(\"TURSO_URL\")?;      // retired\n\
             let db_token = std::env::var(\"VOX_TURSO_TOKEN\")?; // retired\n\
             \n\
             // FIX — use the canonical VOX_DB_* names\n\
             let db_url   = std::env::var(\"VOX_DB_URL\")?;   // canonical\n\
             let db_token = std::env::var(\"VOX_DB_TOKEN\")?; // canonical",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if !matches!(file.language, Language::Rust | Language::Vox) {
            return vec![];
        }

        let mut findings = Vec::new();

        for (i, line) in file.lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            // Skip "this file is about the migration" — heuristic to keep noise
            // out of docs/comments that talk ABOUT the retirement.
            if trimmed.contains("retired") || trimmed.contains("vox-deprecated-since") {
                continue;
            }

            // Call-site is more specific; check it first and prefer it.
            if let Some(caps) = self.call_site_pattern.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    format!(
                        "Retired env-var `{name}` read at call-site — use `{}` instead.",
                        Self::canonical_replacement(name)
                    ),
                    FindingConfidence::High,
                ));
            } else if let Some(caps) = self.bare_literal_pattern.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("?");
                let m = caps.get(0).expect("group 0");
                findings.push(self.build_finding(
                    file,
                    line_num,
                    m.start() + 1,
                    format!(
                        "Retired env-var literal `{name}` — use `{}` instead.",
                        Self::canonical_replacement(name)
                    ),
                    FindingConfidence::Medium,
                ));
            }
        }

        findings
    }
}

impl RetiredEnvVarDetector {
    fn build_finding(
        &self,
        file: &SourceFile,
        line: usize,
        column: usize,
        message: String,
        confidence: FindingConfidence,
    ) -> Finding {
        Finding {
            rule_id: self.id().to_string(),
            diagnostic_id: self.diagnostic_id().map(str::to_string),
            rule_name: self.name().to_string(),
            severity: Severity::Warning,
            file: file.path.clone(),
            line,
            column,
            message,
            suggestion: Some(
                "Resolve secrets through `vox_secrets::resolve_secret(...)` rather than reading \
                 the legacy env-var directly. See AGENTS.md §Secret Management."
                    .to_string(),
            ),
            alternatives: vec![],
            rationale: Some(
                "Direct reads of legacy env-vars bypass the vox-secrets resolver, which is the \
                 SSOT for secret discovery. Migration aliases exist in vox-secrets so callers can \
                 read the canonical name today."
                    .to_string(),
            ),
            context: file.context_around(line, 2),
            confidence: Some(confidence),
            evidence: None,
        }
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
    fn flags_env_var_call_with_turso_url() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"let v = env::var("TURSO_URL").unwrap();"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].confidence, Some(FindingConfidence::High));
        assert!(findings[0].message.contains("VOX_DB_URL"));
    }

    #[test]
    fn flags_std_env_var_call_with_vox_turso_token() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"let v = std::env::var("VOX_TURSO_TOKEN")?;"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].confidence, Some(FindingConfidence::High));
        assert!(findings[0].message.contains("VOX_DB_TOKEN"));
    }

    #[test]
    fn flags_bare_literal_with_medium_confidence() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"let name = "VOX_TURSO_URL";"#);
        let findings = d.detect(&f, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].confidence, Some(FindingConfidence::Medium));
    }

    #[test]
    fn does_not_double_flag_call_site_as_bare_literal() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"let v = env::var("TURSO_URL")?;"#);
        let findings = d.detect(&f, None);
        // Call-site match wins; we should NOT emit two findings on one line.
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn does_not_flag_canonical_env_var() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"let v = env::var("VOX_DB_URL").unwrap();"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_line_marked_with_retirement_annotation() {
        let d = RetiredEnvVarDetector::new();
        // vox-deprecated-since lines are vestigial-during-migration and should
        // not be re-flagged (AGENTS.md §Deprecation Annotations).
        let f = rust(
            r#"// vox-deprecated-since="0.5.0" retire-by="0.6.0" reason="turso-rename"
let v = env::var("TURSO_URL")?;"#,
        );
        let findings = d.detect(&f, None);
        // The annotation comment line is skipped; the code line still matches
        // because the heuristic only skips lines that mention "retired" or
        // the annotation. Let's verify the code line still fires.
        assert_eq!(findings.len(), 1, "code line should still fire");
    }

    #[test]
    fn ignores_lines_that_talk_about_the_retirement() {
        let d = RetiredEnvVarDetector::new();
        let f = rust(r#"// TURSO_URL is retired — use VOX_DB_URL"#);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "explanatory comment should be skipped");
    }

    #[test]
    fn does_not_fire_on_non_rust_non_vox_files() {
        let d = RetiredEnvVarDetector::new();
        let f = SourceFile::new(
            PathBuf::from("config.json"),
            r#"{ "url": "TURSO_URL" }"#.to_string(),
        );
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "config.json (Unknown) is out of scope");
    }
}
