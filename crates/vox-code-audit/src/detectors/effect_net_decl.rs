use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects public or endpoint functions in Vox files that call HTTP/net builtins without
/// an `@uses(net)` decorator.
pub struct EffectNetDeclDetector {
    /// Matches a public or endpoint function declaration line
    fn_decl: Regex,
    /// Matches `@uses(net)` (or containing `net`) in decorator lines
    uses_net: Regex,
    /// Matches HTTP / network builtin calls
    net_call: Regex,
    supported_langs: Vec<Language>,
}

impl Default for EffectNetDeclDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectNetDeclDetector {
    pub fn new() -> Self {
        Self {
            fn_decl: Regex::new(
                r"(?:^|\s)(?:pub\s+fn|@endpoint\s+fn)\s+\w+",
            )
            .expect("valid regex"),
            uses_net: Regex::new(
                r"@uses\s*\([^)]*\bnet\b[^)]*\)",
            )
            .expect("valid regex"),
            net_call: Regex::new(
                r"\b(?:http\.get\s*\(|http\.post\s*\(|http\.put\s*\(|http\.delete\s*\(|http\.patch\s*\(|fetch\s*\(|std\.http\.|populi\.|net\.)",
            )
            .expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for EffectNetDeclDetector {
    fn id(&self) -> &'static str {
        "vox/effect/missing-net-decl"
    }

    fn name(&self) -> &'static str {
        "Effect Missing Net Declaration Detector"
    }

    fn description(&self) -> &'static str {
        "Detects public or endpoint functions in Vox files that perform HTTP or network calls \
        without declaring `@uses(net)` on the function."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::EFFECT_MISSING_NET_DECL)
    }

    fn explain(&self) -> &'static str {
        "Public and endpoint functions that call `http.*`, `fetch()`, `net.*`, `populi.*`, or \
        `std.http.*` must declare `@uses(net)` so callers and the capability checker know this \
        function has network side-effects.\n\n\
        Bad:   pub fn fetch_user(id: UserId) -> User { http.get(\"/users/\" + id) }\n\
        Good:  @uses(net)\n       pub fn fetch_user(id: UserId) -> User { http.get(\"/users/\" + id) }"
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — pub fn calls http.get but lacks @uses(net)\n\
             pub fn fetch_prices(symbol: str) -> List[Price] {\n\
             \x20   http.get(\"/prices/\" + symbol)  // network call not declared!\n\
             }\n\
             \n\
             // FIX — add @uses(net) before the function\n\
             @uses(net)\n\
             pub fn fetch_prices(symbol: str) -> List[Price] {\n\
             \x20   http.get(\"/prices/\" + symbol)\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let mut findings = Vec::new();
        let lines = &file.lines;
        let n = lines.len();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }

            // Check if this is a pub fn or @endpoint fn declaration
            let Some(fn_match) = self.fn_decl.find(line) else {
                continue;
            };

            // Check the preceding 5 lines for @uses(net)
            let look_back_start = i.saturating_sub(5);
            let has_uses_net = lines[look_back_start..i]
                .iter()
                .any(|l| self.uses_net.is_match(l));

            if has_uses_net {
                continue;
            }

            // Check the next 50 lines (function body) for net calls
            let body_end = (i + 51).min(n);
            let has_net_call = lines[(i + 1)..body_end].iter().any(|l| {
                let t = l.trim();
                if t.starts_with("//") || t.starts_with('#') {
                    return false;
                }
                self.net_call.is_match(l)
            });

            if has_net_call {
                let fn_text = fn_match.as_str().trim().to_string();
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: self.diagnostic_id().map(str::to_string),
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    file: file.path.clone(),
                    line: line_num,
                    column: fn_match.start() + 1,
                    message: format!(
                        "`{fn_text}` performs HTTP/net calls but lacks `@uses(net)` decorator."
                    ),
                    suggestion: Some(
                        "Add `@uses(net)` on the line immediately before this function declaration.".into(),
                    ),
                    alternatives: vec![
                        "Refactor to extract the net call into a helper annotated with `@uses(net)`.".into(),
                    ],
                    rationale: Some(
                        "The Vox effect system requires that any function with transitive network \
                        side-effects be annotated with `@uses(net)`. This lets the capability \
                        checker enforce that callers have the net capability and makes the \
                        contract visible to readers.".into(),
                    ),
                    context: file.context_around(line_num, 2),
                    confidence: Some(FindingConfidence::Medium),
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

    fn vox_source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_pub_fn_with_http_get_no_uses_net() {
        let d = EffectNetDeclDetector::new();
        let code = "pub fn fetch_user(id: UserId) -> User {\n    http.get(\"/users/\" + id)\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag pub fn calling http.get() without @uses(net)"
        );
        assert!(findings[0].message.contains("@uses(net)"));
    }

    #[test]
    fn ignores_fn_with_uses_net_decorator() {
        let d = EffectNetDeclDetector::new();
        let code = "@uses(net)\npub fn fetch_user(id: UserId) -> User {\n    http.get(\"/users/\" + id)\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "fn with @uses(net) should not fire");
    }

    #[test]
    fn ignores_fn_with_no_net_calls() {
        let d = EffectNetDeclDetector::new();
        let code = "pub fn add(a: Int, b: Int) -> Int {\n    return a + b;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "fn with no net calls should not fire");
    }

    #[test]
    fn flags_endpoint_fn_calling_fetch() {
        let d = EffectNetDeclDetector::new();
        let code = "@endpoint fn get_data(req: Request) -> Response {\n    let result = fetch(\"/api/data\");\n    return result;\n}";
        let f = vox_source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "should flag @endpoint fn calling fetch() without @uses(net)"
        );
    }

    #[test]
    fn does_not_fire_on_non_vox_files() {
        let d = EffectNetDeclDetector::new();
        let code = "pub fn fetch_user() {\n    http.get(\"/users\");\n}";
        let f = SourceFile::new(PathBuf::from("test.rs"), code.to_string());
        let findings = d.detect(&f, None);
        assert!(findings.is_empty(), "should not fire on non-Vox files");
    }
}
