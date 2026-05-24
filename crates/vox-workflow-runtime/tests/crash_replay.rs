#![allow(missing_docs)]
//! Phase 3.2: crash + resume proof.
//!
//! Seeds a completed activity into the journal via `VoxDbTracker`, then runs
//! `interpret_workflow_durable` against the same `run_id`. Verifies the
//! seeded `charge_card` activity is REPLAYED from the journal (interpreter
//! emits `ActivityReplayed` and skips re-execution) while `send_receipt`
//! still executes fresh.
//!
//! This is the durable runtime's core promise: crash mid-workflow, restart
//! on another node, and completed activities don't re-execute — the journal
//! is the source of truth.
//!
//! Refs ADR-019 v1 journal contract; ADR-021 generated-vs-interpreted parity.

use serde_json::json;
use std::sync::Arc;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::workflow::interpret_workflow_durable;
use vox_workflow_runtime::{VoxDbTracker, WorkflowTracker};

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/golden/durable_workflow_real.vox"
));

/// Replicates `vox_workflow_runtime::workflow::run::derive_activity_id`, which is
/// `pub(crate)` and therefore not reachable from an integration test. The
/// algorithm is locked in by `derive_activity_id_is_deterministic` + the 32-hex
/// shape test inside that module; this copy must stay in sync. If those unit
/// tests change, update here too.
fn derive_activity_id(workflow_name: &str, activity_name: &str, position: usize) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(workflow_name.as_bytes());
    hasher.update(b"\0");
    hasher.update(activity_name.as_bytes());
    hasher.update(b"\0");
    hasher.update(position.to_le_bytes().as_ref());
    let hash = hasher.finalize();
    let bytes = &hash.as_bytes()[..16];
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test]
async fn workflow_resumes_seeded_activity_from_journal() {
    let module = parse(lex(GOLDEN)).expect("durable_workflow_real.vox parses");
    let hir = lower_module(&module);

    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let run_id = "crash-replay-test-1".to_string();
    let mut tracker = VoxDbTracker::new(db.clone(), run_id.clone());

    // Seed: pretend `charge_card` (position 0 in the `checkout` workflow) already
    // completed in a prior crashed run. The interpreter uses the same
    // BLAKE3-derived activity_id, so a record under that id is what
    // `is_activity_completed` + `load_activity_result` will return on resume.
    let first_charge_id = derive_activity_id("checkout", "charge_card", 0);
    let seeded_result = json!({
        "event": "LocalActivity",
        "activity": "charge_card",
        "activity_id": first_charge_id.clone(),
        "status": "executed",
        "classification": "local",
        "seeded_marker": "crash-replay-seed",
    });

    tracker
        .on_activity_completed("checkout", "charge_card", &first_charge_id, &seeded_result)
        .await
        .expect("seed charge_card completion into journal");

    // Now run the workflow on the same run_id. The seeded activity should
    // surface as ActivityReplayed; send_receipt should still execute fresh.
    let journal = interpret_workflow_durable(&hir, "checkout", &mut tracker)
        .await
        .expect("checkout resumes against seeded journal");

    let event_names: Vec<&str> = journal
        .iter()
        .filter_map(|e| e.get("event").and_then(|v| v.as_str()))
        .collect();

    // ── Assertion 1: seeded charge_card replays from journal ────────────────
    let replayed_for_charge: Vec<&serde_json::Value> = journal
        .iter()
        .filter(|e| {
            e.get("event").and_then(|v| v.as_str()) == Some("ActivityReplayed")
                && e.get("activity").and_then(|v| v.as_str()) == Some("charge_card")
        })
        .collect();
    assert_eq!(
        replayed_for_charge.len(),
        1,
        "expected exactly one ActivityReplayed for seeded charge_card; full events={event_names:?}"
    );
    let replayed = replayed_for_charge[0];
    assert_eq!(
        replayed.get("activity_id").and_then(|v| v.as_str()),
        Some(first_charge_id.as_str()),
        "ActivityReplayed activity_id must match seeded id; entry={replayed}"
    );
    assert_eq!(
        replayed.get("replay_source").and_then(|v| v.as_str()),
        Some("workflow_activity_log"),
        "replay_source must be workflow_activity_log on journal replay; entry={replayed}"
    );

    // The seeded result payload should be embedded back into the journal
    // verbatim (carrying the marker we planted at seed time).
    let saw_seeded_marker = journal.iter().any(|e| {
        e.get("seeded_marker").and_then(|v| v.as_str()) == Some("crash-replay-seed")
    });
    assert!(
        saw_seeded_marker,
        "expected seeded result payload to be embedded in resumed journal; full events={event_names:?}"
    );

    // ── Assertion 2: send_receipt still executes fresh ──────────────────────
    let fresh_for_receipt: Vec<&serde_json::Value> = journal
        .iter()
        .filter(|e| {
            e.get("event").and_then(|v| v.as_str()) == Some("LocalActivity")
                && e.get("activity").and_then(|v| v.as_str()) == Some("send_receipt")
        })
        .collect();
    assert_eq!(
        fresh_for_receipt.len(),
        1,
        "expected exactly one fresh LocalActivity for un-seeded send_receipt; full events={event_names:?}"
    );

    // send_receipt should NOT have a Replayed entry.
    let replayed_for_receipt = journal.iter().any(|e| {
        e.get("event").and_then(|v| v.as_str()) == Some("ActivityReplayed")
            && e.get("activity").and_then(|v| v.as_str()) == Some("send_receipt")
    });
    assert!(
        !replayed_for_receipt,
        "send_receipt was not seeded, so it must not appear as ActivityReplayed; full events={event_names:?}"
    );

    // ── Assertion 3: workflow terminates cleanly ────────────────────────────
    assert_eq!(
        event_names.first().copied(),
        Some("WorkflowStarted"),
        "first event must be WorkflowStarted; got {event_names:?}"
    );
    assert_eq!(
        event_names.last().copied(),
        Some("WorkflowCompleted"),
        "last event must be WorkflowCompleted; got {event_names:?}"
    );
}

#[tokio::test]
async fn workflow_runs_fresh_when_journal_is_empty() {
    // Control: same workflow, no seed. Neither activity should appear as
    // Replayed — both must run fresh. Anchors the crash-replay assertion
    // above so a future change that always emits ActivityReplayed cannot
    // silently pass the seeded test.
    let module = parse(lex(GOLDEN)).expect("durable_workflow_real.vox parses");
    let hir = lower_module(&module);

    let db = Arc::new(VoxDb::connect(DbConfig::Memory).await.expect("memory db"));
    let mut tracker = VoxDbTracker::new(db, "crash-replay-control-1");

    let journal = interpret_workflow_durable(&hir, "checkout", &mut tracker)
        .await
        .expect("checkout runs fresh against empty journal");

    let event_names: Vec<&str> = journal
        .iter()
        .filter_map(|e| e.get("event").and_then(|v| v.as_str()))
        .collect();

    let any_replayed = event_names.iter().any(|n| *n == "ActivityReplayed");
    assert!(
        !any_replayed,
        "fresh run must not emit ActivityReplayed; got {event_names:?}"
    );
    assert_eq!(
        event_names.last().copied(),
        Some("WorkflowCompleted"),
        "fresh run must terminate with WorkflowCompleted; got {event_names:?}"
    );
}
