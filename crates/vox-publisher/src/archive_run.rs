//! Deterministic step planner for `publication-archive-run`.
//! Pure: takes a completion report + approval flag, returns an ordered plan or blockers.
//!
//! The plan is a *preview* of the archive pipeline. The live Zenodo adapter
//! (`scholarly::submit_with_adapter`) is monolithic — one `submit` call performs
//! draft + staging upload + optional publish internally — so the executor maps a
//! single adapter call onto the granular [`ArchiveStep`]s listed here. See the
//! `publication-archive-run` handler for that mapping.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveStep {
    ZenodoDepositDraft,
    ZenodoUploadStaging,
    SoftwareHeritageSave,
    RecordReceipt,
    /// Only present when the publish flag is set.
    ZenodoPublish,
}

impl ArchiveStep {
    /// Stable snake_case name (does not depend on serde config).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            ArchiveStep::ZenodoDepositDraft => "zenodo_deposit_draft",
            ArchiveStep::ZenodoUploadStaging => "zenodo_upload_staging",
            ArchiveStep::SoftwareHeritageSave => "software_heritage_save",
            ArchiveStep::RecordReceipt => "record_receipt",
            ArchiveStep::ZenodoPublish => "zenodo_publish",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveRunPlan {
    pub steps: Vec<ArchiveStep>,
    pub blockers: Vec<String>,
}

impl ArchiveRunPlan {
    #[must_use]
    pub fn first_blocker(&self) -> Option<&str> {
        self.blockers.first().map(String::as_str)
    }

    /// Stable snake_case names of the planned steps, in order.
    #[must_use]
    pub fn step_names(&self) -> Vec<&'static str> {
        self.steps.iter().map(ArchiveStep::name).collect()
    }
}

/// Build the archive-run plan.
///
/// Blocks (returns blockers, no steps) when required manifest fields are missing
/// or when no approved review decision is recorded. Otherwise emits the ordered
/// step preview, inserting [`ArchiveStep::ZenodoPublish`] just before
/// [`ArchiveStep::RecordReceipt`] only when `include_publish` is set.
#[must_use]
pub fn plan_archive_run(
    completion: &crate::scientia_discovery::ManifestCompletionReport,
    approved: bool,
    include_publish: bool,
) -> ArchiveRunPlan {
    if !completion.required_missing.is_empty() {
        let blockers = completion
            .required_missing
            .iter()
            .map(|f| format!("required field missing: {f} (run publication-autofill)"))
            .collect();
        return ArchiveRunPlan {
            steps: Vec::new(),
            blockers,
        };
    }

    if !approved {
        return ArchiveRunPlan {
            steps: Vec::new(),
            blockers: vec![
                "archive run requires an approved review decision (run publication-approve)".into(),
            ],
        };
    }

    // The Zenodo adapter is monolithic: a single submit call does deposit +
    // staging-upload + (optional) publish atomically, so `ZenodoPublish` is
    // ordered immediately after the upload — it physically happens during the
    // one Zenodo call, BEFORE the independent Software Heritage save. This keeps
    // the planned order identical to the executor's recorded `executed_steps`.
    let mut steps = vec![
        ArchiveStep::ZenodoDepositDraft,
        ArchiveStep::ZenodoUploadStaging,
    ];
    if include_publish {
        steps.push(ArchiveStep::ZenodoPublish);
    }
    steps.push(ArchiveStep::SoftwareHeritageSave);
    steps.push(ArchiveStep::RecordReceipt);

    ArchiveRunPlan {
        steps,
        blockers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scientia_discovery::ManifestCompletionReport;

    fn complete_report() -> ManifestCompletionReport {
        ManifestCompletionReport {
            completeness_0_100: 100,
            required_missing: Vec::new(),
            inferred_ok: vec!["title".into()],
            human_only_pending: Vec::new(),
            field_provenance: Vec::new(),
        }
    }

    #[test]
    fn plan_blocks_on_incomplete_required_fields() {
        let mut report = complete_report();
        report.required_missing = vec!["license_spdx".into()];
        let plan = plan_archive_run(&report, true, false);
        assert!(plan.steps.is_empty());
        assert!(
            plan.first_blocker().unwrap().contains("license_spdx"),
            "blocker should name the missing field: {:?}",
            plan.first_blocker()
        );
    }

    #[test]
    fn plan_blocks_without_approval() {
        let report = complete_report();
        let plan = plan_archive_run(&report, false, false);
        assert!(plan.steps.is_empty());
        assert!(
            plan.first_blocker().unwrap().contains("approv"),
            "blocker should mention approval: {:?}",
            plan.first_blocker()
        );
    }

    #[test]
    fn complete_and_approved_plan_orders_steps() {
        let report = complete_report();
        let plan = plan_archive_run(&report, true, false);
        assert!(plan.blockers.is_empty());
        assert_eq!(
            plan.step_names(),
            vec![
                "zenodo_deposit_draft",
                "zenodo_upload_staging",
                "software_heritage_save",
                "record_receipt",
            ]
        );
    }

    #[test]
    fn publish_flag_inserts_publish_before_receipt() {
        let report = complete_report();
        let plan = plan_archive_run(&report, true, true);
        assert!(plan.blockers.is_empty());
        assert_eq!(
            plan.step_names(),
            vec![
                "zenodo_deposit_draft",
                "zenodo_upload_staging",
                "zenodo_publish",
                "software_heritage_save",
                "record_receipt",
            ]
        );
        // publish happens during the monolithic Zenodo call (before SWH) and
        // always precedes the receipt step.
        let names = plan.step_names();
        let publish_idx = names.iter().position(|n| *n == "zenodo_publish").unwrap();
        let swh_idx = names
            .iter()
            .position(|n| *n == "software_heritage_save")
            .unwrap();
        let receipt_idx = names.iter().position(|n| *n == "record_receipt").unwrap();
        assert!(publish_idx < swh_idx, "publish is part of the Zenodo call");
        assert!(swh_idx < receipt_idx);
    }
}
