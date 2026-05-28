use super::SweepRule;
use crate::features::{ExtractedFeatures, LiteralContext};
use std::collections::HashMap;
use std::path::PathBuf;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};

pub struct LiteralDedupRule {
    pub threshold: usize,
    pub min_length: usize,
}

impl Default for LiteralDedupRule {
    fn default() -> Self {
        Self {
            threshold: 3,
            min_length: 8,
        }
    }
}

impl SweepRule for LiteralDedupRule {
    fn id(&self) -> &'static str {
        "sweep/duplicate-string-literal"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn sweep(
        &self,
        files: &[ExtractedFeatures],
        _ctx: &crate::rules::WorkspaceContext,
    ) -> Vec<Finding> {
        let mut index: HashMap<String, Vec<(PathBuf, usize)>> = HashMap::new();
        for f in files {
            for lit in &f.string_literals {
                if lit.value.len() < self.min_length {
                    continue;
                }
                if matches!(lit.ctx, LiteralContext::ConstDecl | LiteralContext::Doc) {
                    continue;
                }
                if is_ignored_path(&f.file) {
                    continue;
                }
                index
                    .entry(lit.value.clone())
                    .or_default()
                    .push((f.file.clone(), lit.loc.line));
            }
        }
        index.into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(value, locs)| {
                let others: Vec<String> = locs[1..].iter()
                    .map(|(p, l)| format!("{}:{}", p.display(), l))
                    .collect();
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate String Literal".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].1,
                    column: 0,
                    message: format!(
                        "{:?} appears {} times — consider a named constant",
                        value,
                        locs.len()
                    ),
                    suggestion: Some("Extract to a SSOT constant module".into()),
                    context: format!("Also at: {}", others.join(", ")),
                    confidence: Some(FindingConfidence::Medium),
                    evidence: Some(serde_json::json!({
                        "occurrences": locs.iter().map(|(p, l)| format!("{}:{}", p.display(), l)).collect::<Vec<_>>()
                    })),
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                }
            })
            .collect()
    }
}

fn is_ignored_path(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("/tests/")
        || s.contains("\\tests\\")
        || s.contains("/fixtures/")
        || s.contains("\\fixtures\\")
        || s.contains("/golden/")
        || s.contains("\\golden\\")
        || s.ends_with("_test.rs")
        || s.ends_with(".generated.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use crate::layers_manifest::LayersManifest;
    use crate::rules::WorkspaceContext;
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    fn ctx() -> WorkspaceContext {
        WorkspaceContext {
            workspace_version: "0.6.0".into(), // drift-allow(version-string): test fixture
            workspace_root: PathBuf::from("."),
            layers: LayersManifest::default(),
        }
    }

    fn make_file(path: &str, literals: &[&str]) -> ExtractedFeatures {
        let mut f = ExtractedFeatures::new(PathBuf::from(path), Language::Rust);
        for &v in literals {
            f.string_literals.push(LiteralLoc {
                value: v.to_string(),
                loc: Loc { line: 1, col: 0 },
                ctx: LiteralContext::Code,
            });
        }
        f
    }

    #[test]
    fn finds_string_over_threshold() {
        let files = vec![
            make_file("a.rs", &["duplicate-me", "other"]),
            make_file("b.rs", &["duplicate-me"]),
            make_file("c.rs", &["duplicate-me"]),
        ];
        let rule = LiteralDedupRule::default();
        let findings = rule.sweep(&files, &ctx());
        assert!(!findings.is_empty());
        let f = &findings[0];
        assert!(f.message.contains("3"));
        assert_eq!(f.rule_id, "sweep/duplicate-string-literal");
    }

    #[test]
    fn ignores_strings_below_threshold() {
        let files = vec![
            make_file("a.rs", &["only-twice"]),
            make_file("b.rs", &["only-twice"]),
        ];
        let rule = LiteralDedupRule::default();
        assert!(rule.sweep(&files, &ctx()).is_empty());
    }

    #[test]
    fn ignores_const_decl_context() {
        let mut f1 = make_file("a.rs", &[]);
        f1.string_literals.push(LiteralLoc {
            value: "dup".into(),
            loc: Loc::default(),
            ctx: LiteralContext::ConstDecl,
        });
        let f2 = make_file("b.rs", &["dup"]);
        let f3 = make_file("c.rs", &["dup"]);
        // Only 2 Code occurrences — below threshold
        let rule = LiteralDedupRule::default();
        assert!(rule.sweep(&[f1, f2, f3], &ctx()).is_empty());
    }
}
