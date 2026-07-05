//! Contract smoke tests: DeI daemon JSON envelope uses stable method ids from [`vox_cli::dei_daemon`].

use serde_json::json;
use vox_cli::dei_daemon::method::*;
use vox_cli::{DispatchPayload, DispatchRequest, DispatchResponse};
use vox_orchestrator::contract::{
    DEI_PLAN_METHODS_NEW_REPLAN_STATUS, MCP_PLAN_TOOL_NAMES, plan_tool_daemon_alignment_valid,
};

#[test]
fn mcp_plan_tools_align_with_dei_plan_methods() {
    assert!(plan_tool_daemon_alignment_valid());
    for (i, tool) in MCP_PLAN_TOOL_NAMES.iter().enumerate() {
        let method = DEI_PLAN_METHODS_NEW_REPLAN_STATUS[i];
        assert!(
            !tool.is_empty() && !method.is_empty(),
            "empty plan mapping at index {i}"
        );
    }
}

#[test]
fn dispatch_request_serializes_expected_method_ids() {
    for (method, expected) in [
        (AI_CHECK, "ai.check"),
        (AI_FIX, "ai.fix"),
        (AI_REVIEW, "ai.review"),
        (AI_GENERATE, "ai.generate"),
        (CONFIG_GET, "config.get"),
        (AI_PLAN_NEW, "ai.plan.new"),
        (AI_PLAN_REPLAN, "ai.plan.replan"),
        (AI_PLAN_STATUS, "ai.plan.status"),
        (AI_PLAN_EXECUTE, "ai.plan.execute"),
    ] {
        assert_eq!(method, expected);
        let req = DispatchRequest {
            id: "1".into(),
            method: method.into(),
            params: json!({}),
            auth_token: None,
            permission_mode: None,
        };
        let s = serde_json::to_string(&req).expect("serialize");
        assert!(
            s.contains(expected),
            "serialized request should embed method: {s}"
        );
    }
}

#[test]
fn dispatch_response_error_roundtrip() {
    let msg = DispatchResponse {
        id: "x".into(),
        payload: DispatchPayload::Error {
            message: "daemon unavailable".into(),
            code: 7,
        },
    };
    let s = serde_json::to_string(&msg).expect("serialize");
    let back: DispatchResponse = serde_json::from_str(&s).expect("deserialize");
    match back.payload {
        DispatchPayload::Error { code, .. } => assert_eq!(code, 7),
        other => panic!("expected Error payload: {other:?}"),
    }
}
