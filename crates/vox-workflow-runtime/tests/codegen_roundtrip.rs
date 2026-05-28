//! Phase 2.2: end-to-end durable workflow execution.
//!
//! Loads the `durable_workflow_real.vox` golden, lowers to HIR, runs via
//! `interpret_workflow_durable` with `InMemoryTracker`, and asserts:
//!   - the workflow runs to completion without error,
//!   - the journal terminates with `WorkflowCompleted`,
//!   - both user-defined activities (`charge_card` + `send_receipt`) are
//!     exercised at least once (via `ActivityCompleted` or replay),
//!   - `extract_terminal_return::<serde_json::Value>` succeeds against the
//!     terminal event (the current interpreter records `return_value: null` —
//!     Phase 3+ tightens this to a typed value).
//!
//! This test exercises the runtime interpreter against the lowered HIR. It does
//! NOT compile the emitted Rust — the compile-link proof landed in Phase 1.4.

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_workflow_runtime::workflow::{
    InMemoryTracker, extract_terminal_return, interpret_workflow_durable,
};

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/golden/durable_workflow_real.vox"
));

#[tokio::test]
async fn durable_checkout_workflow_executes_end_to_end() {
    let module = parse(lex(GOLDEN)).expect("durable_workflow_real.vox parses");
    let hir = lower_module(&module);

    let mut tracker = InMemoryTracker::default();
    let journal = interpret_workflow_durable(&hir, "checkout", &mut tracker)
        .await
        .expect("checkout workflow runs to completion");

    let event_names: Vec<String> = journal
        .iter()
        .filter_map(|e| e.get("event").and_then(|v| v.as_str()).map(String::from))
        .collect();

    // The first event must be WorkflowStarted.
    assert_eq!(
        event_names.first().map(String::as_str),
        Some("WorkflowStarted"),
        "first event must be WorkflowStarted; got {event_names:?}"
    );

    // The terminal event must be WorkflowCompleted.
    assert_eq!(
        event_names.last().map(String::as_str),
        Some("WorkflowCompleted"),
        "last event must be WorkflowCompleted; got {event_names:?}"
    );

    // Both user-defined activities should be exercised in the journal.
    // We look at activity names recorded on ActivityStarted / ActivityCompleted /
    // ActivityReplayed / ActivityCacheHit / ActivitySkipped events.
    let activity_names_seen: std::collections::HashSet<String> = journal
        .iter()
        .filter_map(|e| {
            let evt = e.get("event").and_then(|v| v.as_str())?;
            let is_activity_event = matches!(
                evt,
                "ActivityStarted"
                    | "ActivityCompleted"
                    | "ActivityReplayed"
                    | "ActivityCacheHit"
                    | "ActivitySkipped"
                    | "ActivityTask"
            );
            if is_activity_event {
                e.get("activity").and_then(|v| v.as_str()).map(String::from)
            } else {
                None
            }
        })
        .collect();

    assert!(
        activity_names_seen.contains("charge_card"),
        "expected charge_card to be exercised; saw activities {activity_names_seen:?}, full event_names={event_names:?}"
    );
    assert!(
        activity_names_seen.contains("send_receipt"),
        "expected send_receipt to be exercised; saw activities {activity_names_seen:?}, full event_names={event_names:?}"
    );

    // `extract_terminal_return::<serde_json::Value>` must succeed — even when
    // the current interpreter emits `return_value: null`, that deserializes to
    // `Value::Null` cleanly. Subsequent phases (codegen recording typed return)
    // will tighten this to the actual `Result[str]` shape.
    let returned: serde_json::Value = extract_terminal_return(&journal)
        .expect("extract_terminal_return for serde_json::Value should succeed");
    // No specific shape assertion here — Phase 2 only proves extraction works.
    // Document the current contract for future readers / regression catches:
    assert!(
        returned.is_null() || returned.is_object() || returned.is_string(),
        "return_value should be null (current) or a typed value (future); got {returned:?}"
    );
}
