//! Cross-module pure types (`vox-orchestrator-types`).

use std::str::FromStr;

use vox_orchestrator_types::socrates_policy::shannon_entropy_bits;
use vox_orchestrator_types::{
    AgentId, ChatProviderRouteKind, ChatRouteBackend, DaemonId, HuggingFaceRouterEndpoint,
    MergeOutcome, TaskId, backend_telemetry_labels, route_backend_for_chat_route,
};

#[test]
fn task_and_agent_id_parse_and_display() {
    let t: TaskId = TaskId::from_str("T-0012").unwrap();
    assert_eq!(t.to_string(), "T-0012");
    let t2: TaskId = "12".parse().unwrap();
    assert_eq!(t2.0, 12);

    let a: AgentId = "A-03".parse().unwrap();
    assert_eq!(a.to_string(), "A-03");
}

#[test]
fn chat_route_backend_resolution_and_labels() {
    let cases = vec![
        (
            ChatProviderRouteKind::OpenRouter {
                model: "openai/gpt-4".into(),
            },
            ChatRouteBackend::OpenRouter,
        ),
        (
            ChatProviderRouteKind::PopuliLocal {
                base_url: "http://127.0.0.1:11434".into(),
                model: "llama".into(),
            },
            ChatRouteBackend::Ollama,
        ),
        (
            ChatProviderRouteKind::ManualOpenAiCompatible {
                base_url: "https://generativelanguage.googleapis.com/v1".into(),
                model: "gemini".into(),
                bearer: None,
            },
            ChatRouteBackend::GeminiDirect,
        ),
    ];

    for (route, expected) in cases {
        assert_eq!(route_backend_for_chat_route(&route), expected);
        let (provider, lane) = backend_telemetry_labels(expected);
        assert!(!provider.is_empty());
        assert!(!lane.is_empty());
    }

    let hf = ChatProviderRouteKind::HuggingFaceRouter(HuggingFaceRouterEndpoint {
        model: "m".into(),
        chat_completions_url: "https://hf.test/v1/chat/completions".into(),
        bearer_token: None,
    });
    assert_eq!(
        route_backend_for_chat_route(&hf),
        ChatRouteBackend::CascadeFallback
    );
}

#[test]
fn shannon_entropy_uniform_three_outcomes() {
    let h = shannon_entropy_bits(&[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
    assert!(h > 1.5 && h < 1.6, "unexpected H for uniform ternary: {h}");
}

#[test]
fn merge_outcome_lock_wait_json_roundtrip_preserves_fields() {
    let leader = DaemonId([9u8; 16]);
    let original = MergeOutcome::LockWait {
        path: std::path::PathBuf::from("crates/foo/src/lib.rs"),
        leader,
        lease_ms: 500,
        leader_lamport: 42,
    };
    let json = serde_json::to_string(&original).expect("MergeOutcome serializes");
    let back: MergeOutcome = serde_json::from_str(&json).expect("MergeOutcome deserializes");
    match back {
        MergeOutcome::LockWait {
            path,
            leader: l,
            lease_ms,
            leader_lamport,
        } => {
            assert_eq!(path, std::path::PathBuf::from("crates/foo/src/lib.rs"));
            assert_eq!(l, leader);
            assert_eq!(lease_ms, 500);
            assert_eq!(leader_lamport, 42);
        }
        other => panic!("expected LockWait, got {other:?}"),
    }
}
