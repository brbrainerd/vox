use tokio::time::timeout;
use vox_orchestrator::events::{AgentEventKind, BuildStageKind, EventBus};

#[tokio::test]
async fn build_stage_event_round_trips_through_bus() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::BuildStage {
        run_id: "4f2a91".into(),
        stage: BuildStageKind::Hir,
        status: "running".into(),
        duration_ms: Some(1800),
        diagnostic_count: 0,
    });

    let received = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    match received.kind {
        AgentEventKind::BuildStage { run_id, stage, .. } => {
            assert_eq!(run_id, "4f2a91");
            assert_eq!(stage, BuildStageKind::Hir);
        }
        _ => panic!("wrong variant"),
    }
}

#[tokio::test]
async fn throughput_tick_event_round_trips() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::ThroughputTick {
        ts_ms: 1_700_000_000_000,
        tokens_per_sec: 42.5,
        active_runs: 3,
    });

    let received = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    match received.kind {
        AgentEventKind::ThroughputTick {
            tokens_per_sec,
            active_runs,
            ..
        } => {
            assert!((tokens_per_sec - 42.5).abs() < f32::EPSILON);
            assert_eq!(active_runs, 3);
        }
        _ => panic!("wrong variant"),
    }
}

#[tokio::test]
async fn cost_tick_event_round_trips() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::CostTick {
        ts_ms: 1_700_000_000_001,
        delta_usd: 0.0025,
        total_24h_usd: 1.50,
        model: "claude-sonnet-4-6".into(),
    });

    let received = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    match received.kind {
        AgentEventKind::CostTick {
            delta_usd, model, ..
        } => {
            assert!((delta_usd - 0.0025).abs() < 1e-10);
            assert_eq!(model, "claude-sonnet-4-6");
        }
        _ => panic!("wrong variant"),
    }
}

#[tokio::test]
async fn file_diag_changed_event_round_trips() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::FileDiagChanged {
        path: "src/main.vox".into(),
        error_count: 2,
        warn_count: 5,
    });

    let received = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    match received.kind {
        AgentEventKind::FileDiagChanged {
            path,
            error_count,
            warn_count,
        } => {
            assert_eq!(path, "src/main.vox");
            assert_eq!(error_count, 2);
            assert_eq!(warn_count, 5);
        }
        _ => panic!("wrong variant"),
    }
}

#[tokio::test]
async fn mesh_topology_changed_event_round_trips() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::MeshTopologyChanged {
        added_nodes: vec!["agent-7".into(), "agent-8".into()],
        removed_nodes: vec!["agent-3".into()],
        changed_edges: 4,
    });

    let received = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    match received.kind {
        AgentEventKind::MeshTopologyChanged {
            added_nodes,
            removed_nodes,
            changed_edges,
        } => {
            assert_eq!(added_nodes, vec!["agent-7", "agent-8"]);
            assert_eq!(removed_nodes, vec!["agent-3"]);
            assert_eq!(changed_edges, 4);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn build_stage_serializes_with_snake_case() {
    let evt = AgentEventKind::BuildStage {
        run_id: "abc123".into(),
        stage: BuildStageKind::Codegen,
        status: "done".into(),
        duration_ms: Some(500),
        diagnostic_count: 0,
    };
    let json = serde_json::to_value(&evt).unwrap();
    let s = json.to_string();
    // The `type` tag should contain "build_stage" (snake_case from serde tag)
    assert!(
        s.contains("build_stage"),
        "expected 'build_stage' in JSON wire shape, got: {s}"
    );
    // The stage field value should also be snake_case
    assert!(
        s.contains("codegen"),
        "expected 'codegen' in JSON wire shape, got: {s}"
    );
}

#[test]
fn all_build_stage_kind_variants_serialize() {
    use BuildStageKind::*;
    let stages = [Lex, Parse, Hir, Typecheck, Codegen];
    let expected = ["lex", "parse", "hir", "typecheck", "codegen"];
    for (stage, exp) in stages.iter().zip(expected.iter()) {
        let s = serde_json::to_string(stage).unwrap();
        assert!(s.contains(exp), "expected '{exp}' for {stage:?}, got: {s}");
    }
}
