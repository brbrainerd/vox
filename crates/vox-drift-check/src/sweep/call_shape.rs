use super::SweepRule;
use crate::features::ExtractedFeatures;
use std::collections::HashMap;
use vox_code_audit::rules::{Finding, Severity};

pub struct CallShapeRule {
    pub threshold: usize,
}

impl Default for CallShapeRule {
    fn default() -> Self {
        Self { threshold: 5 }
    }
}

impl SweepRule for CallShapeRule {
    fn id(&self) -> &'static str {
        "sweep/duplicate-call-pattern"
    }
    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn sweep(
        &self,
        files: &[ExtractedFeatures],
        _ctx: &crate::rules::WorkspaceContext,
    ) -> Vec<Finding> {
        let mut index: HashMap<String, Vec<(std::path::PathBuf, usize)>> = HashMap::new();
        for f in files {
            for cs in &f.call_sites {
                let key = format!("{}:{}", cs.path.join("::"), cs.arity);
                index
                    .entry(key)
                    .or_default()
                    .push((f.file.clone(), cs.loc.line));
            }
        }
        index
            .into_iter()
            .filter(|(_, locs)| locs.len() >= self.threshold)
            .map(|(key, locs)| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Repeated Call Pattern".into(),
                severity: self.severity(),
                file: locs[0].0.clone(),
                line: locs[0].1,
                column: 0,
                message: format!(
                    "`{}` called {} times — consider a wrapper helper",
                    key,
                    locs.len()
                ),
                suggestion: None,
                context: String::new(),
                confidence: None,
                evidence: None,
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
            })
            .collect()
    }
}
