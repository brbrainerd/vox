// P5-T7: kudos GpuComputeMs accounting reconciliation test.
use vox_mesh_types::{Attestation, kudos::gpu_compute_ms_from_attestation};

fn fake_attestation(task_id: &str, gpu_seconds: f64) -> Attestation {
    Attestation {
        task_id: task_id.to_string(),
        input_hash_blake3_hex: "aabbcc".to_string(),
        output_hash_blake3_hex: "ddeeff".to_string(),
        gpu_seconds,
        trace_blake3_hex: None,
        ephemeral_pubkey_hex: "00".repeat(32),
        signature_b64: "AAAA".to_string(),
        signed_at_unix_ms: 1_700_000_000_000,
        tee_quote: None,
        replay_proof_blake3_hex: None,
        kudos_signature_b64: None,
    }
}

#[test]
fn gpu_compute_ms_sum_matches_expected() {
    let attestations: Vec<Attestation> = (0..10)
        .map(|i| fake_attestation(&format!("task-{i}"), (i + 1) as f64))
        .collect();

    // Expected: sum of (i+1)*1000 for i in 0..10 = 1000+2000+...+10000 = 55000
    let expected_ms: u64 = (1..=10u64).map(|i| i * 1000).sum();
    let computed_ms: u64 = attestations
        .iter()
        .map(gpu_compute_ms_from_attestation)
        .sum();

    assert_eq!(
        computed_ms, expected_ms,
        "sum of GpuComputeMs must equal sum of gpu_seconds*1000"
    );
}

#[test]
fn gpu_compute_ms_conversion_is_correct() {
    let att = fake_attestation("t1", 2.5);
    assert_eq!(gpu_compute_ms_from_attestation(&att), 2500, "2.5s = 2500ms");

    let att2 = fake_attestation("t2", 0.001);
    assert_eq!(gpu_compute_ms_from_attestation(&att2), 1, "0.001s = 1ms");

    let att3 = fake_attestation("t3", 0.0);
    assert_eq!(gpu_compute_ms_from_attestation(&att3), 0, "0s = 0ms");
}

#[test]
fn gpu_compute_ms_batch_of_ten_sums_correctly() {
    // Each attestation carries 3.7 seconds of GPU compute.
    let gpu_secs = 3.7f64;
    let attestations: Vec<Attestation> = (0..10)
        .map(|i| fake_attestation(&format!("batch-{i}"), gpu_secs))
        .collect();

    let total: u64 = attestations
        .iter()
        .map(gpu_compute_ms_from_attestation)
        .sum();
    let expected = (gpu_secs * 1000.0) as u64 * 10;
    assert_eq!(total, expected, "10 × 3.7s should equal 37000ms");
}
