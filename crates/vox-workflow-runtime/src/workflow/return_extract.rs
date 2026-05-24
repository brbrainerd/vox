//! `extract_terminal_return::<T>(journal)` walks a completed durable workflow
//! journal back to its `WorkflowCompleted` event and deserializes the recorded
//! `return_value` into the caller's T. Generated workflow bodies call this at
//! the end of `interpret_workflow_durable` to surface the typed return value.
//!
//! The journal today is `Vec<serde_json::Value>` (see `workflow::run`). Each
//! event is a JSON object discriminated by its `"event"` string field. The
//! terminal `WorkflowCompleted` event carries a `"return_value"` field whose
//! shape is workflow-specific.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Errors returned by [`extract_terminal_return`].
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    /// The journal contained no `WorkflowCompleted` event — the workflow did
    /// not terminate cleanly (interrupted, panicked, or still running).
    #[error("no WorkflowCompleted event in journal — workflow did not terminate cleanly")]
    NoTerminal,
    /// The terminal `WorkflowCompleted` event had no `return_value` field.
    /// Codegen produced by an earlier compiler version may emit this; bump and
    /// recompile the workflow.
    #[error("WorkflowCompleted event has no return_value field")]
    MissingReturnValue,
    /// The recorded `return_value` could not be deserialized into the caller's
    /// requested type `T`.
    #[error("terminal return value did not deserialize to expected type: {0}")]
    TypeMismatch(#[from] serde_json::Error),
}

/// Extract the typed return value from a completed durable workflow journal.
///
/// Scans backwards for the terminal `WorkflowCompleted` event and deserializes
/// its `return_value` field into `T`. Returns [`ExtractError`] if the event is
/// missing, the field is absent, or the value does not match `T`.
pub fn extract_terminal_return<T: DeserializeOwned>(
    journal: &[Value],
) -> Result<T, ExtractError> {
    let completed = journal
        .iter()
        .rev()
        .find(|e| e.get("event").and_then(Value::as_str) == Some("WorkflowCompleted"))
        .ok_or(ExtractError::NoTerminal)?;
    let return_value = completed
        .get("return_value")
        .ok_or(ExtractError::MissingReturnValue)?;
    Ok(serde_json::from_value(return_value.clone())?)
}
