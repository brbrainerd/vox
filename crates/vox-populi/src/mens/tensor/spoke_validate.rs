//! Drift-proofing for the spoke SSOT (mens/config/domain-profiles.yaml).
//! Exposed through vox ci spoke-check CI gate.

use crate::mens::tensor::domain_profiles::{DomainProfilesFile, TrainMethod};
use std::path::Path;

/// One human-readable validation problem.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SpokeViolation(pub String);

/// Validate every spoke that declares a `base`. Returns all violations.
/// Rules: (1) a fine-tune method requires `mix_config`; (2) a fine-tune
/// method requires `base.preset`; (3) `eval_gate`, if set, must exist on disk;
/// (4) `mix_config`, if set, must exist on disk.
pub fn validate(file: &DomainProfilesFile, workspace_root: &Path) -> Vec<SpokeViolation> {
    let mut v = Vec::new();
    let fine_tune = |m: TrainMethod| {
        matches!(
            m,
            TrainMethod::Qlora | TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo
        )
    };
    for (name, p) in &file.profiles {
        let Some(base) = &p.base else { continue };
        if fine_tune(base.method) {
            if p.mix_config.is_none() {
                v.push(SpokeViolation(format!(
                    "spoke '{name}': fine-tune method requires mix_config"
                )));
            }
            if base.preset.is_none() {
                v.push(SpokeViolation(format!(
                    "spoke '{name}': fine-tune method requires base.preset"
                )));
            }
        }
        if let Some(mc) = &p.mix_config
            && !workspace_root.join(mc).is_file() {
                v.push(SpokeViolation(format!(
                    "spoke '{name}': mix_config '{mc}' not found"
                )));
            }
        if let Some(eg) = &p.eval_gate
            && !workspace_root.join(eg).is_file() {
                v.push(SpokeViolation(format!(
                    "spoke '{name}': eval_gate '{eg}' not found"
                )));
            }
        // Rule 5: base.model resolvability check
        if !base.model.contains('/') {
            match crate::mens::tensor::spoke_base_resolver::load_overlay(workspace_root) {
                Ok(overlay) => {
                    if !overlay.contains_key(&base.model) {
                        v.push(SpokeViolation(format!(
                            "spoke '{name}': base.model capability tag '{}' not found in gpu-specs train_bases",
                            base.model
                        )));
                    }
                }
                Err(e) => {
                    v.push(SpokeViolation(format!(
                        "spoke '{name}': failed to load gpu-specs train_bases overlay: {e}"
                    )));
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mens::tensor::domain_profiles::DomainProfilesFile;

    #[test]
    fn flags_finetune_without_mix_config() {
        let yaml = r#"
profiles:
  broken:
    description: "x"
    base: { model: m, method: qlora }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        let v = validate(&file, std::path::Path::new("/nonexistent"));
        assert!(
            v.iter().any(|x| x.0.contains("requires mix_config")),
            "got {v:?}"
        );
    }

    #[test]
    fn rag_only_spoke_needs_no_mix() {
        let yaml = r#"
profiles:
  docs:
    description: "x"
    base: { model: m, method: rag_only }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        let v = validate(&file, std::path::Path::new("/nonexistent"));
        assert!(
            !v.iter().any(|x| x.0.contains("requires mix_config")),
            "got {v:?}"
        );
    }

    #[test]
    fn bogus_base_model_tag_violates() {
        let yaml = r#"
profiles:
  broken:
    description: "x"
    base: { model: bogus_tag, method: rag_only }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        // Since load_overlay will fail to read from /nonexistent, it will produce a violation
        let v = validate(&file, std::path::Path::new("/nonexistent"));
        assert!(
            v.iter()
                .any(|x| x.0.contains("failed to load") || x.0.contains("not found")),
            "got {v:?}"
        );
    }

    #[test]
    fn real_base_model_tag_passes() {
        let yaml = r#"
profiles:
  ok_spoke:
    description: "x"
    base: { model: strong_code_default, method: rag_only }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let v = validate(&file, root);
        assert!(v.is_empty(), "got violations: {v:?}");
    }
}
