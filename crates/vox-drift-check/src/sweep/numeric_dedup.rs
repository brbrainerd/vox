use super::SweepRule;
use crate::features::{ExtractedFeatures, UnitHint};
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};

pub struct NumericDedupRule {
    pub threshold: usize,
}

impl Default for NumericDedupRule {
    fn default() -> Self {
        Self { threshold: 3 }
    }
}

impl SweepRule for NumericDedupRule {
    fn id(&self) -> &'static str {
        "sweep/duplicate-numeric-literal"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn sweep(
        &self,
        files: &[ExtractedFeatures],
        _ctx: &crate::rules::WorkspaceContext,
    ) -> Vec<Finding> {
        // Track allow-lines too so a `// drift-allow(duplicate-numeric-literal)`
        // marker can blank an individual occurrence (e.g. a foundation test where
        // the SSOT crate is structurally unreachable).
        let mut index: HashMap<
            (u64, u8),
            Vec<(std::path::PathBuf, usize, /*allowed:*/ bool)>,
        > = HashMap::new();
        for f in files {
            for n in &f.numeric_literals {
                // ConstDecl literals are the named constants the rule wants people
                // to define — counting them as "duplicates" defeats the rule.
                if n.in_const {
                    continue;
                }
                let unit_disc = match &n.unit {
                    Some(UnitHint::Seconds) => 1u8,
                    Some(UnitHint::Millis) => 2,
                    Some(UnitHint::Bytes) => 3,
                    _ => continue,
                };
                let key = (n.value.to_bits(), unit_disc);
                let allowed = crate::extractor::is_allowed_at(f, self.id(), n.loc.line);
                index
                    .entry(key)
                    .or_default()
                    .push((f.file.clone(), n.loc.line, allowed));
            }
        }
        index
            .into_iter()
            // Drop allow-marked occurrences before counting; a wholly-allowed
            // literal can't trigger the threshold.
            .map(|(k, mut locs)| {
                locs.retain(|(_, _, allowed)| !allowed);
                (k, locs)
            })
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|((bits, unit_disc), locs)| {
                let val = f64::from_bits(bits);
                let unit_str = match unit_disc { 1 => "s", 2 => "ms", _ => "bytes" };
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate Numeric Literal".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].1,
                    column: 0,
                    message: format!(
                        "{}{} appears {} times — define a named constant",
                        val, unit_str, locs.len()
                    ),
                    suggestion: Some(
                        "Add a const to vox-config::timeouts or the appropriate SSOT module".into(),
                    ),
                    context: String::new(),
                    confidence: Some(FindingConfidence::High),
                    evidence: Some(serde_json::json!({
                        "occurrences": locs.iter().map(|(p, l, _)| format!("{}:{}", p.display(), l)).collect::<Vec<_>>()
                    })),
                    diagnostic_id: None,
                    alternatives: vec![],
                    rationale: None,
                }
            })
            .collect()
    }
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

    #[test]
    fn finds_repeated_duration_constant() {
        let make = |line: usize, val: f64| ExtractedFeatures {
            numeric_literals: vec![NumericLoc {
                value: val,
                unit: Some(UnitHint::Seconds),
                loc: Loc { line, col: 0 },
                in_const: false,
            }],
            ..ExtractedFeatures::new(PathBuf::from(format!("{}.rs", line)), Language::Rust)
        };
        let files = vec![make(1, 30.0), make(2, 30.0), make(3, 30.0)];
        let rule = NumericDedupRule::default();
        let findings = rule.sweep(&files, &ctx());
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("30"));
    }

    #[test]
    fn const_decl_occurrences_do_not_count() {
        // 3 const-decl + 1 non-const = 1 effective occurrence → below threshold.
        let make_const = |line: usize| ExtractedFeatures {
            numeric_literals: vec![NumericLoc {
                value: 30.0,
                unit: Some(UnitHint::Seconds),
                loc: Loc { line, col: 0 },
                in_const: true,
            }],
            ..ExtractedFeatures::new(PathBuf::from(format!("c{}.rs", line)), Language::Rust)
        };
        let make_use = |line: usize| ExtractedFeatures {
            numeric_literals: vec![NumericLoc {
                value: 30.0,
                unit: Some(UnitHint::Seconds),
                loc: Loc { line, col: 0 },
                in_const: false,
            }],
            ..ExtractedFeatures::new(PathBuf::from(format!("u{}.rs", line)), Language::Rust)
        };
        let files = vec![make_const(1), make_const(2), make_const(3), make_use(4)];
        let rule = NumericDedupRule::default();
        assert!(rule.sweep(&files, &ctx()).is_empty());
    }

    #[test]
    fn per_line_allow_suppresses_an_occurrence() {
        // 3 occurrences total, but 1 is allow-marked → 2 left, below threshold.
        let make = |line: usize, allowed: bool| {
            let mut f = ExtractedFeatures::new(
                PathBuf::from(format!("{}.rs", line)),
                Language::Rust,
            );
            f.numeric_literals.push(NumericLoc {
                value: 30.0,
                unit: Some(UnitHint::Seconds),
                loc: Loc { line, col: 0 },
                in_const: false,
            });
            if allowed {
                let mut set = std::collections::HashSet::new();
                set.insert(line);
                f.allowed_lines
                    .insert("duplicate-numeric-literal".to_string(), set);
            }
            f
        };
        let files = vec![make(1, true), make(2, false), make(3, false)];
        let rule = NumericDedupRule::default();
        assert!(rule.sweep(&files, &ctx()).is_empty());
    }
}
