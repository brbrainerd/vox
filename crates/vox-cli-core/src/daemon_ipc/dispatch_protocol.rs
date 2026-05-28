//! Newline JSON protocol — re-export wire types from [`vox_protocol`].

pub use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use vox_bounded_fs::read_utf8_path_capped;

    #[test]
    fn dispatch_response_roundtrip_done() {
        let msg = DispatchResponse {
            id: "abc".into(),
            payload: DispatchPayload::Done { exit: 0 },
        };
        let json = serde_json::to_string(&msg).expect("serialize DispatchResponse");
        let back: DispatchResponse =
            serde_json::from_str(&json).expect("deserialize DispatchResponse");
        assert_eq!(back.id, "abc");
        match back.payload {
            DispatchPayload::Done { exit } => assert_eq!(exit, 0),
            unexpected_payload => {
                panic!(
                    "dispatch roundtrip: expected terminal exit payload, got {unexpected_payload:?}"
                )
            }
        }
    }

    #[test]
    fn dispatch_request_validates_against_dei_rpc_schema() {
        use std::path::PathBuf;

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let schema_path = manifest.join("../../contracts/dei/rpc-methods.schema.json");
        let schema_src = read_utf8_path_capped(Path::new(&schema_path))
            .unwrap_or_else(|e| panic!("read {}: {e}", schema_path.display()));
        let schema_val: serde_json::Value =
            serde_json::from_str(&schema_src).expect("parse DeI RPC schema");
        let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
            .expect("compile DeI RPC schema");

        let req = DispatchRequest {
            id: "req-1".into(),
            method: vox_foundation::protocol::dei_method::AI_GENERATE.into(),
            params: serde_json::json!({ "prompt": "hello" }),
        };
        let instance = serde_json::to_value(&req).expect("serialize DispatchRequest");
        vox_jsonschema_util::validate(
            &instance,
            &validator,
            "DispatchRequest vs contracts/dei/rpc-methods.schema.json",
        )
        .expect("DispatchRequest must match contracts/dei/rpc-methods.schema.json");
    }

    #[test]
    fn dei_schema_method_enum_matches_daemon_constants() {
        use std::collections::HashSet;
        use std::path::PathBuf;

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let schema_path = manifest.join("../../contracts/dei/rpc-methods.schema.json");
        let schema_val: serde_json::Value = serde_json::from_str(
            &read_utf8_path_capped(Path::new(&schema_path)).expect("read DeI RPC schema"),
        )
        .expect("parse DeI RPC schema");
        let methods = schema_val["properties"]["method"]["enum"]
            .as_array()
            .expect("schema properties.method.enum");
        let as_set: HashSet<&str> = methods
            .iter()
            .map(|v| v.as_str().expect("enum string"))
            .collect();

        use vox_foundation::protocol::dei_method as method;
        for m in [
            method::AI_CHECK,
            method::AI_FIX,
            method::AI_REVIEW,
            method::AI_GENERATE,
            method::CONFIG_GET,
            method::AI_PLAN_NEW,
            method::AI_PLAN_REPLAN,
            method::AI_PLAN_STATUS,
            method::AI_PLAN_EXECUTE,
        ] {
            assert!(as_set.contains(m), "schema missing method {m}");
        }
        assert_eq!(as_set.len(), 9, "schema vs daemon method count drift");
    }
}
