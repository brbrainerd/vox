//! Automatic post-mutation verification feedback — the verification-driven agent loop.
//!
//! After a file-mutating MCP tool succeeds, if it touched a `.vox` file we run the
//! existing [`crate::code_validator::vox_check`] validator and surface any
//! error-severity diagnostics back inside that tool's result. The agent's normal
//! loop then self-corrects on its next turn, without having to remember to call the
//! validator. This makes "verification the agent can run itself" automatic — the
//! single most-loved Claude Code property.
//!
//! Gated by `VOX_VERIFY_ON_WRITE` (default ON; set to `0`/`false`/`off`/`no` to disable).

use crate::code_validator;
use crate::params::VoxCheckParams;
use crate::server_state::ServerState;

/// File-mutating MCP tools whose successful result should trigger auto-verification.
/// Mirrors the write-tool set already gated in [`crate::dispatch`].
const MUTATING_FILE_TOOLS: &[&str] = &[
    "vox_write_file",
    "vox_patch_file",
    "vox_inline_edit_file",
    "vox_multi_replace",
    "vox_multi_replace_file",
    "vox_apply_structured_edit",
];

/// Returns the workspace-relative `.vox` path to verify, iff `name` is a file-mutating
/// tool that touched a `.vox` file. Pure; performs no I/O.
pub fn verifiable_vox_path(name: &str, args: &serde_json::Value) -> Option<String> {
    if !MUTATING_FILE_TOOLS.contains(&name) {
        return None;
    }
    let path = args.get("path").and_then(|v| v.as_str())?;
    let is_vox = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("vox"));
    is_vox.then(|| path.to_string())
}

/// Whether auto-verification is enabled. Env-gated, default ON.
pub fn enabled() -> bool {
    match std::env::var("VOX_VERIFY_ON_WRITE") {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => true,
    }
}

/// Given the JSON returned by [`crate::code_validator::vox_check`], produce a concise,
/// model-facing advisory iff the check ran successfully **and** found error-severity
/// diagnostics. Returns `None` when the check is clean or when the check itself failed
/// to run (so we never fabricate a verification verdict). Pure.
pub fn advisory_from_check_json(check_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(check_json).ok()?;
    if v.get("success").and_then(|s| s.as_bool()) != Some(true) {
        return None;
    }
    let data = v.get("data")?;
    if !data
        .get("has_errors")
        .and_then(|b| b.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    let count = data.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
    let first = data
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .and_then(|arr| {
            arr.iter()
                .find_map(|d| d.get("message").and_then(|m| m.as_str()))
        })
        .unwrap_or("see diagnostics");
    Some(format!(
        "AUTO_VERIFICATION_FAILED: your edit was written, but `vox check` reports {count} \
         diagnostic(s) (first error: {first}). Fix the file and re-verify before continuing."
    ))
}

/// Attach an advisory to a tool-result payload. If the payload is a JSON object we set
/// a top-level `auto_verification_failed: true` flag and a `meta.auto_verification`
/// note (preserving any existing `meta`); otherwise we append the advisory as text.
/// Pure.
pub fn attach_advisory(payload: String, advisory: String) -> String {
    match serde_json::from_str::<serde_json::Value>(&payload) {
        Ok(mut v) if v.is_object() => {
            let obj = v.as_object_mut().expect("checked is_object");
            obj.insert(
                "auto_verification_failed".to_string(),
                serde_json::Value::Bool(true),
            );
            let meta = obj
                .entry("meta")
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if !meta.is_object() {
                *meta = serde_json::Value::Object(serde_json::Map::new());
            }
            meta.as_object_mut()
                .expect("meta normalized to object")
                .insert(
                    "auto_verification".to_string(),
                    serde_json::Value::String(advisory),
                );
            serde_json::to_string(&v).unwrap_or_else(|_| payload.clone())
        }
        _ => format!("{payload}\n\n{advisory}"),
    }
}

/// Run auto-verification for a just-succeeded mutating tool, returning the (already
/// attached) payload. No-op (returns `payload` unchanged) when disabled, when the tool
/// didn't touch a `.vox` file, or when the file checks clean. Performs the `vox_check`
/// I/O via the existing validator (DRY — no second compiler invocation path).
pub async fn verify_and_attach(
    state: &ServerState,
    name: &str,
    args: &serde_json::Value,
    payload: String,
) -> String {
    if !enabled() {
        return payload;
    }
    let Some(path) = verifiable_vox_path(name, args) else {
        return payload;
    };
    let check_json = code_validator::vox_check(state, VoxCheckParams { path }).await;
    match advisory_from_check_json(&check_json) {
        Some(advisory) => attach_advisory(payload, advisory),
        None => payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verifiable_path_is_some_for_vox_write_of_vox_file() {
        let args = json!({ "path": "src/foo.vox", "content": "x" });
        assert_eq!(
            verifiable_vox_path("vox_write_file", &args),
            Some("src/foo.vox".to_string())
        );
    }

    #[test]
    fn verifiable_path_is_none_for_non_mutating_tool() {
        let args = json!({ "path": "src/foo.vox" });
        assert_eq!(verifiable_vox_path("vox_git_status", &args), None);
    }

    #[test]
    fn verifiable_path_is_none_for_non_vox_extension() {
        let args = json!({ "path": "src/foo.rs" });
        assert_eq!(verifiable_vox_path("vox_write_file", &args), None);
    }

    #[test]
    fn verifiable_path_is_none_when_path_missing() {
        let args = json!({ "content": "x" });
        assert_eq!(verifiable_vox_path("vox_write_file", &args), None);
    }

    #[test]
    fn advisory_present_when_check_reports_errors() {
        let check_json = json!({
            "success": true,
            "data": { "has_errors": true, "count": 2,
                      "diagnostics": [{ "message": "cannot find value `x`" }] }
        })
        .to_string();
        let advisory = advisory_from_check_json(&check_json).expect("advisory expected");
        assert!(advisory.contains("AUTO_VERIFICATION_FAILED"));
        assert!(advisory.contains("cannot find value `x`"));
        assert!(advisory.contains('2'));
    }

    #[test]
    fn advisory_absent_when_check_is_clean() {
        let check_json = json!({
            "success": true,
            "data": { "has_errors": false, "count": 0, "diagnostics": [] }
        })
        .to_string();
        assert_eq!(advisory_from_check_json(&check_json), None);
    }

    #[test]
    fn advisory_absent_when_check_itself_failed() {
        // e.g. file unreadable / path rejected — don't fabricate a verification verdict.
        let check_json = json!({ "success": false, "error": "failed to read file" }).to_string();
        assert_eq!(advisory_from_check_json(&check_json), None);
    }

    #[test]
    fn attach_advisory_injects_meta_and_flag_into_json_object() {
        let payload = json!({ "success": true, "data": { "written": true } }).to_string();
        let out = attach_advisory(payload, "ADV".to_string());
        let v: serde_json::Value = serde_json::from_str(&out).expect("still valid json");
        assert_eq!(v["auto_verification_failed"], json!(true));
        assert_eq!(v["meta"]["auto_verification"], json!("ADV"));
        // original fields preserved
        assert_eq!(v["data"]["written"], json!(true));
    }

    #[test]
    fn attach_advisory_preserves_existing_meta_object() {
        let payload = json!({ "success": true, "meta": { "k": 1 } }).to_string();
        let out = attach_advisory(payload, "ADV".to_string());
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["meta"]["k"], json!(1));
        assert_eq!(v["meta"]["auto_verification"], json!("ADV"));
    }

    #[test]
    fn attach_advisory_appends_text_for_non_json_payload() {
        let out = attach_advisory("not json".to_string(), "ADV".to_string());
        assert!(out.contains("not json"));
        assert!(out.contains("ADV"));
    }

    /// End-to-end against the real compiler (`code_validator::vox_check` →
    /// `vox_compiler::pipeline::check_file`): a syntactically-broken `.vox` write
    /// gets an advisory attached; a clean `.vox` write is passed through untouched.
    /// Writes uniquely-named probe files under the repo root (then removes them) since
    /// path resolution rejects files outside the repository.
    #[tokio::test]
    async fn verify_and_attach_flags_broken_vox_and_passes_clean_vox() {
        let state = crate::server_state::ServerState::new_test().await;
        let root = state.repository.root.clone();
        let ok_payload =
            serde_json::json!({ "success": true, "data": { "written": true } }).to_string();

        // --- broken file -> advisory attached ---
        let bad_name = "__post_verify_probe_bad__.vox";
        let bad_path = root.join(bad_name);
        std::fs::write(&bad_path, "fn ( {").expect("write bad probe");
        let bad_args = serde_json::json!({ "path": bad_name });
        let bad_out =
            verify_and_attach(&state, "vox_write_file", &bad_args, ok_payload.clone()).await;
        let _ = std::fs::remove_file(&bad_path);
        let bad_v: serde_json::Value =
            serde_json::from_str(&bad_out).expect("broken-file output is still JSON");
        assert_eq!(
            bad_v["auto_verification_failed"],
            serde_json::json!(true),
            "broken .vox should attach a verification advisory; got: {bad_out}"
        );

        // --- clean file -> unchanged passthrough ---
        let ok_name = "__post_verify_probe_ok__.vox";
        let ok_path = root.join(ok_name);
        std::fs::write(&ok_path, "let answer = 42\n").expect("write ok probe");
        let ok_args = serde_json::json!({ "path": ok_name });
        let ok_out =
            verify_and_attach(&state, "vox_write_file", &ok_args, ok_payload.clone()).await;
        let _ = std::fs::remove_file(&ok_path);
        assert_eq!(
            ok_out, ok_payload,
            "clean .vox must pass the payload through untouched; got: {ok_out}"
        );
    }
}
