//! P3-T2 acceptance: Ed25519 sign / verify round-trip; tamper detection.

use vox_orchestrator_queue::oplog::{
    OperationEntry, OperationId, OperationKind,
    sign::{KeyRing, SignError, sign_entry, verify_entry},
};
use vox_orchestrator_types::AgentId;

fn make_entry() -> OperationEntry {
    OperationEntry {
        id: OperationId(1),
        agent_id: AgentId(1),
        timestamp_ms: 1_000_000,
        kind: OperationKind::FileEdit {
            paths: vec!["src/main.rs".into()],
        },
        description: "edit main.rs".into(),
        snapshot_before: None,
        snapshot_after: None,
        db_snapshot_before: None,
        db_snapshot_after: None,
        context_snapshot_before: None,
        context_snapshot_after: None,
        undone: false,
        change_id: None,
        model_id: None,
        predecessor_hash: None,
        signature: None,
        signing_key_id: None,
        daemon_id: [0u8; 16],
        parent_op_ids: Vec::new(),
    }
}

#[test]
fn signed_entry_round_trips_and_tampered_payload_fails() {
    let ring = KeyRing::ephemeral_for_tests();
    let daemon = ring.local_daemon_id();

    let mut entry = make_entry();
    sign_entry(&ring, &mut entry).expect("sign");
    assert!(
        verify_entry(&ring, &entry).is_ok(),
        "valid entry must verify"
    );

    // Tamper the payload — signature must no longer verify.
    entry.description.push('!');
    assert!(
        verify_entry(&ring, &entry).is_err(),
        "tampered description must fail verification"
    );
    let _ = daemon;
}

#[test]
fn unsigned_entry_fails_verification() {
    let ring = KeyRing::ephemeral_for_tests();
    let entry = make_entry(); // no signature set
    let result = verify_entry(&ring, &entry);
    assert!(
        matches!(result, Err(SignError::NoLocalKey)),
        "entry without signing_key_id should error: {result:?}"
    );
}

#[test]
fn unknown_key_id_rejected() {
    let ring_a = KeyRing::ephemeral_for_tests();
    let ring_b = KeyRing::ephemeral_for_tests(); // different keypair
    let mut entry = make_entry();
    sign_entry(&ring_a, &mut entry).unwrap();
    // ring_b doesn't know ring_a's key → UnknownKey
    let result = verify_entry(&ring_b, &entry);
    assert!(
        matches!(result, Err(SignError::UnknownKey(_))),
        "key not in ring should error: {result:?}"
    );
}
