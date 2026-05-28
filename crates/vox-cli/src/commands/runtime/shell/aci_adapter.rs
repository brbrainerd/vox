//! Shell execution probes normalized toward ACI `execution_probe` shapes.

use serde_json::json;

use super::backends::{nushell, powershell};

/// Which structured shell backend to use for a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellBackendKind {
    PowerShell,
    Nushell,
}

/// Stdout/stderr/exit_code tuple aligned with `contracts/aci/agent-computer-interface.v1.schema.json` probe fields.
#[derive(Debug, Clone)]
pub struct ShellExecutionProbe {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl ShellExecutionProbe {
    /// JSON object suitable for merging into `aci.execution_probe`.
    pub fn to_json_value(&self) -> serde_json::Value {
        json!({
            "stdout": self.stdout,
            "stderr": self.stderr,
            "exit_code": self.exit_code,
        })
    }
}

/// Runs `script` on the chosen backend (best-effort; surfaces stderr on spawn failure).
pub fn run_shell_probe(kind: ShellBackendKind, script: &str) -> ShellExecutionProbe {
    let res = match kind {
        ShellBackendKind::PowerShell => powershell::run_pwsh_capture(script),
        ShellBackendKind::Nushell => nushell::run_nu_capture(script),
    };
    match res {
        Ok((stdout, stderr, exit_code)) => ShellExecutionProbe {
            stdout,
            stderr,
            exit_code,
        },
        Err(e) => ShellExecutionProbe {
            stdout: String::new(),
            stderr: format!("spawn error: {e}"),
            exit_code: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_serializes() {
        let p = ShellExecutionProbe {
            stdout: "a".into(),
            stderr: "".into(),
            exit_code: Some(0),
        };
        assert!(p.to_json_value()["stdout"].as_str() == Some("a"));
    }
}
