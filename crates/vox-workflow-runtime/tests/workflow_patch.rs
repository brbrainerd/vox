//! P2-T2 acceptance: `workflow.version()` emits a WorkflowPatch journal event.

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_workflow_runtime::workflow::run::interpret_workflow_durable;
use vox_workflow_runtime::workflow::tracker::DefaultTracker;

#[tokio::test]
async fn workflow_patch_emits_journal_event_first_run() {
    let src = r#"
        workflow wf() to int {
            let v = workflow.version("split-step", 1, 2)
            return 0
        }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let mut tracker = DefaultTracker;
    let journal = interpret_workflow_durable(&hir, "wf", &mut tracker)
        .await
        .expect("run");

    let patch_event = journal
        .iter()
        .find(|v| v["event"].as_str() == Some("WorkflowPatch"))
        .expect("WorkflowPatch must be journaled");

    assert_eq!(patch_event["change_id"], "split-step");
    assert_eq!(patch_event["replayed"], serde_json::Value::Bool(false));
    assert_eq!(patch_event["version"], serde_json::json!(2));
    assert_eq!(patch_event["min_supported"], serde_json::json!(1));
    assert_eq!(patch_event["max_supported"], serde_json::json!(2));
}
