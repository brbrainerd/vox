//! CR-L6 retirement-guard parity check.
//!
//! Cross-references [`contracts/retirement/retired-surfaces.v1.yaml`](../../../contracts/retirement/retired-surfaces.v1.yaml)
//! against the actual detector registry in [`crate::detectors::all_rules`] and
//! the diagnostic-ID catalog in [`crate::diagnostics::catalog::ALL_KNOWN_IDS`].
//!
//! Reports drift between AGENTS.md §Retired Surfaces (the policy doc), the
//! retirement-surface contract (the machine-readable list), and the wired
//! enforcement (detector or CLI check).
//!
//! Council-ratified 2026-05-15 (D6/D25 in
//! `docs/src/architecture/v1-llm-target-implementation-plan-2026.md` §8.1).
//!
//! ## Usage
//!
//! Library callers (unit tests, future `vox ci retirement-audit` CLI):
//!
//! ```no_run
//! use std::path::Path;
//! use vox_code_audit::retirement_parity::{check_parity, ParityReport};
//!
//! let yaml = std::fs::read_to_string(
//!     Path::new("contracts/retirement/retired-surfaces.v1.yaml")
//! ).unwrap();
//! let report = check_parity(&yaml).unwrap();
//! assert!(report.is_clean(), "{report:#?}");
//! ```

use crate::detectors;
use crate::diagnostics::catalog;
use serde::Deserialize;
use thiserror::Error;

/// Errors that can arise while loading or interpreting the retirement contract.
#[derive(Debug, Error)]
pub enum ParityError {
    /// The YAML did not parse against the contract schema.
    #[error("retirement contract YAML did not parse: {0}")]
    YamlParse(#[from] serde_yaml::Error),
}

/// Top-level retirement contract shape.
///
/// Mirrors `contracts/retirement/retired-surfaces.v1.yaml`. Forward-compat:
/// unknown fields are tolerated via `#[serde(default)]`.
#[derive(Debug, Deserialize)]
pub struct RetirementContract {
    /// Schema-version field used by the contract framework. Unread today but
    /// surfaced so consumers can branch on schema version once we have v2.
    #[serde(default)]
    pub schema_version: u32,

    /// Every row of the contract — one per retired surface.
    pub surfaces: Vec<RetiredSurface>,
}

/// One row of the retirement contract.
#[derive(Debug, Deserialize)]
pub struct RetiredSurface {
    pub id: String,
    pub retired_pattern: String,
    pub canonical: String,
    #[serde(default)]
    pub agents_md_row: String,
    pub enforcement: Enforcement,
    #[serde(default)]
    pub rationale: String,
}

/// Tagged union of enforcement mechanisms.
///
/// `kind` discriminates the variant; the additional fields are optional and
/// depend on the kind. We deliberately use `#[serde(deny_unknown_fields)]`
/// nowhere here — unknown fields are tolerated for forward compatibility.
#[derive(Debug, Deserialize)]
pub struct Enforcement {
    pub kind: EnforcementKind,
    /// Present when `kind == Detector`.
    #[serde(default)]
    pub rule_id: Option<String>,
    /// Present when `kind == Detector`.
    #[serde(default)]
    pub diagnostic_id: Option<String>,
    /// Present when `kind == CliCheck`.
    #[serde(default)]
    pub command: Option<String>,
    /// Present when `kind == CliCheck` or `DocumentationOnly`.
    #[serde(default)]
    pub backed_by_contract: Option<String>,
    /// Present when `kind == CliCheck` or `DocumentationOnly` (single-symbol).
    #[serde(default)]
    pub contract_symbol_id: Option<String>,
    /// Present when `kind == DocumentationOnly` (multi-symbol).
    #[serde(default)]
    pub contract_symbol_ids: Vec<String>,
    /// Present when `kind == Deferred`.
    #[serde(default)]
    pub target_milestone: Option<String>,
    /// Present when `kind == Deferred`.
    #[serde(default)]
    pub tracked_in: Option<String>,
}

/// Enforcement-mechanism discriminator.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum EnforcementKind {
    Detector,
    CliCheck,
    DocumentationOnly,
    Deferred,
}

/// Parity-check outcome.
#[derive(Debug, Default)]
pub struct ParityReport {
    /// Detector rows whose `rule_id` resolves cleanly to a registered rule.
    pub detector_rows_ok: Vec<String>,
    /// Detector rows whose `rule_id` does NOT match any registered rule.
    pub detector_rows_missing_rule: Vec<MissingDetectorRule>,
    /// Detector rows whose `diagnostic_id` is not in the stable ID catalog.
    pub detector_rows_missing_diagnostic_id: Vec<MissingDiagnosticId>,
    /// Deferred rows that lack a `target_milestone` declaration.
    pub deferred_rows_missing_milestone: Vec<String>,
    /// Rows with an enforcement kind we recognize, accounted-for.
    pub deferred_rows_ok: Vec<String>,
    pub cli_check_rows_ok: Vec<String>,
    pub documentation_only_rows_ok: Vec<String>,
}

/// A detector-kind row that references a `rule_id` we cannot resolve.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingDetectorRule {
    pub surface_id: String,
    pub referenced_rule_id: String,
}

/// A detector-kind row that references a `diagnostic_id` not in the catalog.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingDiagnosticId {
    pub surface_id: String,
    pub referenced_diagnostic_id: String,
}

impl ParityReport {
    /// Returns true iff there are no drift findings.
    pub fn is_clean(&self) -> bool {
        self.detector_rows_missing_rule.is_empty()
            && self.detector_rows_missing_diagnostic_id.is_empty()
            && self.deferred_rows_missing_milestone.is_empty()
    }

    /// Human-readable summary line.
    pub fn summary(&self) -> String {
        format!(
            "retirement parity: \
             detector ok={}, missing-rule={}, missing-diagnostic-id={}; \
             cli-check ok={}; doc-only ok={}; \
             deferred ok={}, missing-milestone={}",
            self.detector_rows_ok.len(),
            self.detector_rows_missing_rule.len(),
            self.detector_rows_missing_diagnostic_id.len(),
            self.cli_check_rows_ok.len(),
            self.documentation_only_rows_ok.len(),
            self.deferred_rows_ok.len(),
            self.deferred_rows_missing_milestone.len(),
        )
    }
}

/// Run the parity check against the given retirement-contract YAML.
///
/// Returns a [`ParityReport`] describing every row's enforcement status; call
/// [`ParityReport::is_clean`] to gate CI.
pub fn check_parity(yaml: &str) -> Result<ParityReport, ParityError> {
    let contract: RetirementContract = serde_yaml::from_str(yaml)?;

    // Collect the set of detector rule IDs currently registered.
    let registered_rule_ids: std::collections::HashSet<String> = detectors::all_rules(None)
        .iter()
        .map(|rule| rule.id().to_string())
        .collect();

    // Collect the set of stable diagnostic IDs.
    let registered_diagnostic_ids: std::collections::HashSet<&'static str> =
        catalog::ALL_KNOWN_IDS.iter().copied().collect();

    let mut report = ParityReport::default();

    for surface in &contract.surfaces {
        match surface.enforcement.kind {
            EnforcementKind::Detector => {
                let Some(rule_id) = surface.enforcement.rule_id.as_deref() else {
                    report
                        .detector_rows_missing_rule
                        .push(MissingDetectorRule {
                            surface_id: surface.id.clone(),
                            referenced_rule_id: "<missing field>".to_string(),
                        });
                    continue;
                };
                if !registered_rule_ids.contains(rule_id) {
                    report
                        .detector_rows_missing_rule
                        .push(MissingDetectorRule {
                            surface_id: surface.id.clone(),
                            referenced_rule_id: rule_id.to_string(),
                        });
                } else {
                    report.detector_rows_ok.push(surface.id.clone());
                }

                if let Some(diag_id) = surface.enforcement.diagnostic_id.as_deref()
                    && !registered_diagnostic_ids.contains(diag_id)
                {
                    report.detector_rows_missing_diagnostic_id.push(
                        MissingDiagnosticId {
                            surface_id: surface.id.clone(),
                            referenced_diagnostic_id: diag_id.to_string(),
                        },
                    );
                }
            }
            EnforcementKind::CliCheck => {
                // The parity check cannot verify CLI command existence from
                // inside vox-code-audit without a CLI registry import — the
                // future `vox ci retirement-audit` CLI command will extend
                // this check by cross-referencing
                // contracts/cli/command-registry.yaml. For now we only
                // structurally validate the row.
                report.cli_check_rows_ok.push(surface.id.clone());
            }
            EnforcementKind::DocumentationOnly => {
                // Structural validation only: we trust the referenced
                // contracts/documentation/retired-symbols.v1.yaml row.
                report
                    .documentation_only_rows_ok
                    .push(surface.id.clone());
            }
            EnforcementKind::Deferred => {
                if surface.enforcement.target_milestone.is_none() {
                    report
                        .deferred_rows_missing_milestone
                        .push(surface.id.clone());
                } else {
                    report.deferred_rows_ok.push(surface.id.clone());
                }
            }
        }
    }

    Ok(report)
}

/// Convenience wrapper that loads the workspace-canonical retirement contract
/// from `<workspace_root>/contracts/retirement/retired-surfaces.v1.yaml`.
///
/// `workspace_root` is typically `env!("CARGO_MANIFEST_DIR")` / `../..` from
/// a crate under `crates/`.
pub fn check_parity_at_path(yaml_path: &std::path::Path) -> std::io::Result<ParityReport> {
    let yaml = std::fs::read_to_string(yaml_path)?;
    check_parity(&yaml).map_err(|err| std::io::Error::other(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Path from this crate to the workspace contracts directory.
    fn workspace_retirement_yaml_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("contracts")
            .join("retirement")
            .join("retired-surfaces.v1.yaml")
    }

    // ---------------------------------------------------------------------
    // Inline-YAML tests (don't touch the workspace file).
    // ---------------------------------------------------------------------

    #[test]
    fn parses_minimal_contract() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: example
    retired_pattern: "@foo"
    canonical: "@bar"
    enforcement:
      kind: detector
      rule_id: "retired/decorator-usage"
      diagnostic_id: "vox/retired/decorator-usage"
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(report.is_clean(), "expected clean: {}", report.summary());
        assert_eq!(report.detector_rows_ok.len(), 1);
    }

    #[test]
    fn flags_detector_row_with_unknown_rule_id() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: phantom
    retired_pattern: "@x"
    canonical: "@y"
    enforcement:
      kind: detector
      rule_id: "this/does-not-exist"
      diagnostic_id: "vox/retired/decorator-usage"
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(!report.is_clean());
        assert_eq!(report.detector_rows_missing_rule.len(), 1);
        assert_eq!(
            report.detector_rows_missing_rule[0].referenced_rule_id,
            "this/does-not-exist"
        );
    }

    #[test]
    fn flags_detector_row_with_unknown_diagnostic_id() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: bad-diag
    retired_pattern: "@x"
    canonical: "@y"
    enforcement:
      kind: detector
      rule_id: "retired/decorator-usage"
      diagnostic_id: "vox/not/real"
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(!report.is_clean());
        assert_eq!(report.detector_rows_missing_diagnostic_id.len(), 1);
    }

    #[test]
    fn deferred_row_missing_milestone_is_flagged() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: vague
    retired_pattern: "@later"
    canonical: "(removed)"
    enforcement:
      kind: deferred
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(!report.is_clean());
        assert_eq!(report.deferred_rows_missing_milestone.len(), 1);
    }

    #[test]
    fn deferred_row_with_milestone_is_clean() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: later
    retired_pattern: "@later"
    canonical: "(removed)"
    enforcement:
      kind: deferred
      target_milestone: "P1.4"
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(report.is_clean(), "{}", report.summary());
        assert_eq!(report.deferred_rows_ok.len(), 1);
    }

    #[test]
    fn cli_check_and_doc_only_rows_pass_structurally() {
        let yaml = r#"
schema_version: 1
surfaces:
  - id: dei
    retired_pattern: "vox-dei"
    canonical: "vox-orchestrator"
    enforcement:
      kind: cli-check
      command: "vox ci no-dei-import"
  - id: ars
    retired_pattern: "vox-ars"
    canonical: "vox-openclaw-runtime"
    enforcement:
      kind: documentation-only
      backed_by_contract: "contracts/documentation/retired-symbols.v1.yaml"
      contract_symbol_id: "vox-ars-crate"
"#;
        let report = check_parity(yaml).expect("parses");
        assert!(report.is_clean(), "{}", report.summary());
        assert_eq!(report.cli_check_rows_ok.len(), 1);
        assert_eq!(report.documentation_only_rows_ok.len(), 1);
    }

    #[test]
    fn enforcement_kind_serializes_kebab_case() {
        // `cli-check` → CliCheck; `documentation-only` → DocumentationOnly.
        let yaml = r#"
schema_version: 1
surfaces:
  - id: a
    retired_pattern: "x"
    canonical: "y"
    enforcement:
      kind: cli-check
      command: "vox ci foo"
"#;
        let contract: RetirementContract = serde_yaml::from_str(yaml).expect("parses");
        assert_eq!(
            contract.surfaces[0].enforcement.kind,
            EnforcementKind::CliCheck
        );
    }

    // ---------------------------------------------------------------------
    // Workspace-canonical YAML tests (load the real file under contracts/).
    // ---------------------------------------------------------------------

    #[test]
    fn workspace_retired_surfaces_yaml_exists_and_parses() {
        let path = workspace_retirement_yaml_path();
        assert!(
            path.exists(),
            "contract file missing at {}",
            path.display()
        );
        let yaml = std::fs::read_to_string(&path).expect("read contract");
        let contract: RetirementContract =
            serde_yaml::from_str(&yaml).expect("contract parses against schema");
        assert!(
            !contract.surfaces.is_empty(),
            "contract must declare at least one surface"
        );
    }

    #[test]
    fn workspace_retirement_parity_is_clean() {
        let path = workspace_retirement_yaml_path();
        let report = check_parity_at_path(&path).expect("loads and checks");
        assert!(
            report.is_clean(),
            "retirement parity drift: {}\n\
             missing rule_ids: {:?}\n\
             missing diagnostic_ids: {:?}\n\
             deferred rows missing milestone: {:?}",
            report.summary(),
            report.detector_rows_missing_rule,
            report.detector_rows_missing_diagnostic_id,
            report.deferred_rows_missing_milestone,
        );
    }

    #[test]
    fn workspace_retirement_contract_covers_retired_decorator_detector() {
        // Every retired pattern that the `retired/decorator-usage` detector
        // flags MUST be represented by at least one row in the contract.
        // Today: @component fn, @server/@query/@mutation fn, @py.import = 5 rows.
        let path = workspace_retirement_yaml_path();
        let report = check_parity_at_path(&path).expect("loads and checks");
        assert!(
            report.detector_rows_ok.len() >= 5,
            "expected at least 5 detector-enforced rows (the patterns retired_decorator covers), \
             got {}: {:?}",
            report.detector_rows_ok.len(),
            report.detector_rows_ok
        );
    }
}
