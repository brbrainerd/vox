use super::SweepRule;
use crate::features::ExtractedFeatures;
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, FindingConfidence, Severity};

pub struct BodyHashRule {
    pub threshold: usize,
    pub min_lines: u32,
}

impl Default for BodyHashRule {
    fn default() -> Self {
        Self {
            threshold: 2,
            min_lines: 5,
        }
    }
}

impl SweepRule for BodyHashRule {
    fn id(&self) -> &'static str {
        "sweep/duplicate-body"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn sweep(&self, files: &[ExtractedFeatures]) -> Vec<Finding> {
        let mut index: HashMap<u64, Vec<(std::path::PathBuf, String, usize)>> = HashMap::new();
        for f in files {
            for def in &f.fn_definitions {
                let sig = f
                    .body_signatures
                    .iter()
                    .find(|b| b.parent_fn.as_deref() == Some(&def.name));
                if let Some(sig) = sig
                    && sig.line_count < self.min_lines
                {
                    continue;
                }
                index.entry(def.body_hash).or_default().push((
                    f.file.clone(),
                    def.name.clone(),
                    def.loc.line,
                ));
            }
        }
        index
            .into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(_, locs)| {
                let names: Vec<_> = locs.iter().map(|(_, n, _)| n.as_str()).collect();
                Finding {
                    rule_id: self.id().to_string(),
                    rule_name: "Duplicate Function Body".into(),
                    severity: self.severity(),
                    file: locs[0].0.clone(),
                    line: locs[0].2,
                    column: 0,
                    message: format!(
                        "Functions {:?} have identical bodies — extract a shared helper",
                        names
                    ),
                    suggestion: Some("Extract to a shared module".into()),
                    context: locs[1..]
                        .iter()
                        .map(|(p, n, l)| format!("{}:{} ({})", p.display(), l, n))
                        .collect::<Vec<_>>()
                        .join(", "),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
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
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    #[test]
    fn finds_duplicate_fn_bodies() {
        let make = |name: &str, hash: u64| {
            let mut f =
                ExtractedFeatures::new(PathBuf::from(format!("{}.rs", name)), Language::Rust);
            f.fn_definitions.push(FnDef {
                name: name.into(),
                body_hash: hash,
                sig_hash: hash,
                loc: Loc::default(),
            });
            // Add a body signature with enough lines
            f.body_signatures.push(BodySignature {
                hash,
                line_count: 10,
                parent_fn: Some(name.into()),
                loc: Loc::default(),
            });
            f
        };
        let files = vec![make("alpha", 42), make("beta", 42)];
        let rule = BodyHashRule::default();
        let findings = rule.sweep(&files);
        assert!(!findings.is_empty());
    }
}
