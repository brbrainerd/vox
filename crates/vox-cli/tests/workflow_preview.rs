//! P1-T8 acceptance: `vox workflow preview` dry-run projector.

use vox_cli::commands::workflow::preview::project_workflow_from_source;

/// Helper: run the projector on Vox source string for the named workflow.
fn preview(
    src: &str,
    wf_name: &str,
) -> anyhow::Result<vox_cli::commands::workflow::preview::PreviewedWorkflow> {
    project_workflow_from_source(src, wf_name)
}

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = std::path::PathBuf::from(manifest)
        .join("..")
        .join("..") // workspace root
        .join("tests/fixtures/workflow_preview")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

#[test]
fn preview_simple_two_step_lists_both_activities() {
    let src = fixture("simple_two_step.vox");
    let p = preview(&src, "process").expect("preview");
    let names: Vec<&str> = p.steps.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"fetch_url"), "names: {names:?}");
    assert!(names.contains(&"parse_response"), "names: {names:?}");
}

#[test]
fn preview_remote_fn_is_marked_remote() {
    let src = fixture("simple_two_step.vox");
    let p = preview(&src, "process").expect("preview");
    let fetch = p
        .steps
        .iter()
        .find(|s| s.name == "fetch_url")
        .expect("fetch_url");
    assert!(fetch.is_remote, "fetch_url should be marked remote");
}

#[test]
fn preview_with_side_effect_lists_synthesised_activity() {
    let src = fixture("with_side_effect.vox");
    let p = preview(&src, "main").expect("preview");
    assert!(
        p.steps.iter().any(|s| s.name.starts_with("__side_effect_")),
        "expected synthesised side_effect activity; got: {:?}",
        p.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

#[test]
fn preview_empty_workflow_has_no_steps() {
    let src = r#"
        workflow noop() to int {
            return 0
        }
    "#;
    let p = preview(src, "noop").expect("preview");
    assert!(p.steps.is_empty(), "got: {:?}", p.steps);
}
