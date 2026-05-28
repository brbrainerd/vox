//! Path A tests: extract_terminal_return operates on the real Vec<Value> journal.

use serde_json::json;
use vox_workflow_runtime::workflow::extract_terminal_return;

#[test]
fn extracts_return_value_from_workflow_completed_event() {
    let journal = vec![
        json!({"event": "ActivityCompleted", "activity": "step1", "value": 7}),
        json!({"event": "WorkflowCompleted", "workflow": "wf", "return_value": 42}),
    ];
    let got: i64 = extract_terminal_return(&journal).expect("extract");
    assert_eq!(got, 42);
}

#[test]
fn extract_errors_if_no_terminal_event() {
    let journal = vec![json!({"event": "ActivityCompleted", "activity": "step1", "value": 7})];
    let result: Result<i64, _> = extract_terminal_return(&journal);
    assert!(result.is_err(), "expected error for missing terminal event");
}

#[test]
fn extract_errors_on_type_mismatch() {
    let journal = vec![
        json!({"event": "WorkflowCompleted", "workflow": "wf", "return_value": "not a number"}),
    ];
    let result: Result<i64, _> = extract_terminal_return(&journal);
    assert!(result.is_err(), "expected type-mismatch error");
}

#[test]
fn extract_errors_when_return_value_field_missing() {
    let journal = vec![json!({"event": "WorkflowCompleted", "workflow": "wf"})];
    let result: Result<i64, _> = extract_terminal_return(&journal);
    assert!(
        result.is_err(),
        "expected MissingReturnValue error when field absent"
    );
}
