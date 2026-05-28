//! Integration tests for ProviderAtlasFinding (P6-T6).

use vox_publisher::atlas::provider_atlas::{AtlasObservation, ProviderAtlasFindingBuilder};

fn make_obs(
    node_id: &str,
    task_kinds: Vec<&str>,
    done: u64,
    fail: u64,
    opt_in: bool,
    ts: &str,
) -> AtlasObservation {
    AtlasObservation {
        node_id: node_id.to_string(),
        observed_at: ts.to_string(),
        task_kinds: task_kinds.into_iter().map(str::to_string).collect(),
        gpu_utilisation: 0.5,
        tasks_completed_delta: done,
        tasks_failed_delta: fail,
        vram_mb: Some(8192),
        public_mesh_opt_in: opt_in,
    }
}

#[test]
fn single_observation_builds_finding() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-001", "2026-05-10T00:00:00Z");
    builder.observe(&make_obs(
        "node-001",
        vec!["text_infer"],
        10,
        0,
        true,
        "2026-05-10T00:05:00Z",
    ));
    let f = builder.build();

    assert_eq!(f.node_id, "node-001");
    assert_eq!(f.tasks_completed, 10);
    assert_eq!(f.tasks_failed, 0);
    assert!((f.success_rate - 1.0).abs() < 0.001);
    assert!(f.consistently_opted_in);
    assert_eq!(f.declared_task_kinds, vec!["text_infer"]);
}

#[test]
fn multiple_observations_merge_task_kinds() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-002", "2026-05-10T00:00:00Z");
    builder.observe(&make_obs(
        "node-002",
        vec!["text_infer"],
        5,
        0,
        true,
        "2026-05-10T00:05:00Z",
    ));
    builder.observe(&make_obs(
        "node-002",
        vec!["image_gen"],
        3,
        1,
        true,
        "2026-05-10T00:10:00Z",
    ));
    let f = builder.build();

    assert_eq!(f.tasks_completed, 8);
    assert_eq!(f.tasks_failed, 1);
    assert_eq!(f.declared_task_kinds, vec!["image_gen", "text_infer"]); // sorted
}

#[test]
fn opt_out_resets_consistently_opted_in() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-003", "2026-05-10T00:00:00Z");
    builder.observe(&make_obs(
        "node-003",
        vec!["text_infer"],
        5,
        0,
        true,
        "2026-05-10T00:05:00Z",
    ));
    builder.observe(&make_obs(
        "node-003",
        vec!["text_infer"],
        2,
        0,
        false,
        "2026-05-10T00:10:00Z",
    )); // opts out
    let f = builder.build();

    assert!(!f.consistently_opted_in);
}

#[test]
fn vram_averaged_across_observations() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-004", "2026-05-10T00:00:00Z");
    let mut obs = make_obs(
        "node-004",
        vec!["text_infer"],
        1,
        0,
        true,
        "2026-05-10T00:05:00Z",
    );
    obs.vram_mb = Some(8192);
    builder.observe(&obs);
    let mut obs2 = make_obs(
        "node-004",
        vec!["text_infer"],
        1,
        0,
        true,
        "2026-05-10T00:10:00Z",
    );
    obs2.vram_mb = Some(16384);
    builder.observe(&obs2);

    let f = builder.build();
    assert!(f.avg_vram_mb.is_some());
    assert!((f.avg_vram_mb.unwrap() - 12288.0).abs() < 1.0);
}

#[test]
fn finding_id_contains_node_id() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-005", "2026-05-10T00:00:00Z");
    builder.observe(&make_obs(
        "node-005",
        vec!["embed"],
        1,
        0,
        true,
        "2026-05-10T00:01:00Z",
    ));
    let f = builder.build();
    assert!(f.id.contains("node-005"));
}

#[test]
fn finding_round_trip_json() {
    let mut builder = ProviderAtlasFindingBuilder::new("node-006", "2026-05-10T00:00:00Z");
    builder.observe(&make_obs(
        "node-006",
        vec!["text_infer"],
        20,
        2,
        true,
        "2026-05-10T01:00:00Z",
    ));
    let f = builder.build();

    let json = serde_json::to_string(&f).unwrap();
    let decoded: vox_publisher::atlas::provider_atlas::ProviderAtlasFinding =
        serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.node_id, "node-006");
    assert_eq!(decoded.tasks_completed, 20);
}
