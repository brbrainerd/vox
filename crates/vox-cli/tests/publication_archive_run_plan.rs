//! CI-safe dry-run planner for `vox scientia publication-archive-run` (Task 19).
//!
//! Exercises pure `plan_archive_run` — no DB, no network, no human gate.

use vox_publisher::archive_run::{ArchiveStep, plan_archive_run};
use vox_publisher::scientia_discovery::ManifestCompletionReport;

fn complete_report() -> ManifestCompletionReport {
    ManifestCompletionReport {
        completeness_0_100: 100,
        required_missing: vec![],
        inferred_ok: vec![],
        human_only_pending: vec![],
        field_provenance: vec![],
    }
}

#[test]
fn dry_run_plan_includes_swh_and_nanopub_when_flags_set() {
    let plan = plan_archive_run(&complete_report(), true, false, true);
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    let names = plan.step_names();
    assert!(names.contains(&"software_heritage_save"));
    assert!(names.contains(&"nanopub_test_server_publish"));
    assert!(!names.contains(&"zenodo_publish"));
}

#[test]
fn dry_run_blocks_when_required_fields_missing() {
    let mut report = complete_report();
    report.required_missing = vec!["publication_date".into()];
    let plan = plan_archive_run(&report, true, false, false);
    assert!(plan.steps.is_empty());
    assert!(
        plan.first_blocker()
            .unwrap_or("")
            .contains("publication_date")
    );
}

#[test]
fn dry_run_blocks_without_approval() {
    let plan = plan_archive_run(&complete_report(), false, true, false);
    assert!(plan.steps.is_empty());
    assert!(
        plan.first_blocker()
            .unwrap_or("")
            .contains("approved review")
    );
}

#[test]
fn publish_flag_inserts_zenodo_publish_before_swh() {
    let plan = plan_archive_run(&complete_report(), true, true, false);
    let names = plan.step_names();
    let zenodo_pub = names
        .iter()
        .position(|&s| s == ArchiveStep::ZenodoPublish.name())
        .expect("zenodo_publish");
    let swh = names
        .iter()
        .position(|&s| s == ArchiveStep::SoftwareHeritageSave.name())
        .expect("swh");
    assert!(zenodo_pub < swh);
}

#[test]
fn autofill_compose_unblocks_publication_date_for_archive_plan() {
    use vox_publisher::publication::PublicationManifest;
    use vox_publisher::scientia_autofill::{apply_autofill, compute_autofill};
    use vox_publisher::scientia_discovery::manifest_completion_report;

    let mut manifest = PublicationManifest {
        publication_id: "archive-plan-test".into(),
        content_type: "scientia".into(),
        source_ref: None,
        title: "Neural Networks and Deep Learning".into(),
        author: "Ada".into(),
        abstract_text: Some("Abs".into()),
        body_markdown: "b".into(),
        citations_json: Some("[]".into()),
        metadata_json: None,
    };
    let before = manifest_completion_report(&manifest);
    assert!(
        before
            .required_missing
            .contains(&"publication_date".to_string())
    );
    let blocked = plan_archive_run(&before, true, false, false);
    assert!(
        blocked.first_blocker().is_some(),
        "plan should block before autofill: {:?}",
        blocked.blockers
    );

    let autofill = compute_autofill(
        &manifest,
        None,
        Some("MIT"),
        Some("https://github.com/org/repo"),
        Some(env!("CARGO_PKG_VERSION")),
    );
    let new_meta = apply_autofill(
        manifest.metadata_json.as_deref(),
        &mut manifest.abstract_text,
        &autofill,
    )
    .expect("apply autofill");
    manifest.metadata_json = Some(new_meta);

    let after = manifest_completion_report(&manifest);
    let unblocked = plan_archive_run(&after, true, false, false);
    assert!(
        unblocked.first_blocker().is_none(),
        "autofill should clear required-field blockers: {:?}",
        unblocked.blockers
    );
}

#[test]
fn dual_approval_required_for_archive_plan_not_single_approver() {
    let plan = plan_archive_run(&complete_report(), false, false, false);
    assert!(plan.steps.is_empty());
    assert!(
        plan.first_blocker()
            .unwrap_or("")
            .contains("approved review")
    );
}
