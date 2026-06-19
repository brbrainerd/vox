//! MCP tools for delegating to Antigravity `agy` (doctor + single + batch).

use crate::agy_doctor::{detect, remediation, AgyStatus};
use crate::params::ToolResult;
use crate::server_state::ServerState;

pub fn doctor_report_json() -> serde_json::Value {
    let status = detect();
    let (label, path) = match &status {
        AgyStatus::Missing => ("missing", None),
        AgyStatus::PresentUnauthed { path } => ("present_unauthed", Some(path.clone())),
        AgyStatus::Ready { path, .. } => ("ready", Some(path.clone())),
    };
    serde_json::json!({
        "status": label,
        "path": path,
        "remediation": remediation(&status),
    })
}

/// `vox_agy_doctor`
pub async fn vox_agy_doctor(_state: &ServerState, _args: serde_json::Value) -> String {
    ToolResult::ok(doctor_report_json()).to_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_json_has_status_and_remediation() {
        let v = doctor_report_json();
        assert!(v.get("status").is_some());
        assert!(v.get("remediation").is_some());
    }
}
