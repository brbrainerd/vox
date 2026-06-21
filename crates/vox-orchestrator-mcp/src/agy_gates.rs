//! Runs a plan-specified verification gate (build/test/arch-check/...) inside
//! the agy worktree jail and captures a structured pass/fail. This is the
//! deterministic defence against the ledger's "green gates ≠ correct code":
//! the pipeline proves the EFFECT instead of asserting it.
//!
//! Spawns mirror `agy_exec`: `kill_on_drop(true)` + timeout + CREATE_NO_WINDOW,
//! and NO pipe-to-head (a closed pipe orphans cargo workers on Windows).
//! `env` lets cargo gates set CARGO_TARGET_DIR to the main repo's target so a
//! worktree build reuses the cache instead of a cold rebuild.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Gate {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub output_tail: String,
    pub elapsed_ms: u64,
}

fn tail(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n {
        return s.to_string();
    }
    s.chars().skip(len - n).collect()
}

/// Spawn one gate inside `cwd`; pass = exit 0. A spawn error or timeout is a
/// FAILED gate (never a panic).
pub async fn run_gate(cwd: &Path, gate: &Gate, timeout_secs: u64) -> GateResult {
    let started = Instant::now();
    let mut cmd = tokio::process::Command::new(&gate.program);
    cmd.current_dir(cwd)
        .args(&gate.args)
        .envs(&gate.env)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return GateResult {
                name: gate.name.clone(),
                passed: false,
                exit_code: -1,
                output_tail: format!(
                    "gate '{}' failed to spawn '{}': {e}",
                    gate.name, gate.program
                ),
                elapsed_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    let dur = Duration::from_secs(timeout_secs.max(1));
    match tokio::time::timeout(dur, child.wait_with_output()).await {
        Ok(Ok(out)) => {
            let code = out.status.code().unwrap_or(-1);
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&out.stderr));
            GateResult {
                name: gate.name.clone(),
                passed: code == 0,
                exit_code: code,
                output_tail: tail(&combined, 2000),
                elapsed_ms: started.elapsed().as_millis() as u64,
            }
        }
        Ok(Err(e)) => GateResult {
            name: gate.name.clone(),
            passed: false,
            exit_code: -1,
            output_tail: format!("gate '{}' io error: {e}", gate.name),
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
        Err(_elapsed) => GateResult {
            name: gate.name.clone(),
            passed: false,
            exit_code: -1,
            output_tail: format!(
                "gate '{}' exceeded {}s; process killed",
                gate.name, timeout_secs
            ),
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    }
}

/// Run gates sequentially (they often share one cargo target dir; parallel
/// cargo would contend). One result per gate, in order.
pub async fn run_gates(cwd: &Path, gates: &[Gate], timeout_secs: u64) -> Vec<GateResult> {
    let mut out = Vec::with_capacity(gates.len());
    for g in gates {
        out.push(run_gate(cwd, g, timeout_secs).await);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_last_n_chars() {
        assert_eq!(tail("hello", 3), "llo");
        assert_eq!(tail("hi", 5), "hi");
    }

    #[tokio::test]
    async fn passing_gate_reports_pass() {
        let gate = Gate {
            name: "probe".into(),
            program: "git".into(),
            args: vec!["--version".into()],
            ..Default::default()
        };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(r.passed, "git --version should pass: {}", r.output_tail);
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.name, "probe");
    }

    #[tokio::test]
    async fn failing_gate_reports_fail() {
        let gate = Gate {
            name: "bad".into(),
            program: "git".into(),
            args: vec!["rev-parse".into(), "--definitely-not-a-flag".into()],
            ..Default::default()
        };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
        assert_ne!(r.exit_code, 0);
    }

    #[tokio::test]
    async fn missing_program_is_a_failed_gate_not_a_panic() {
        let gate = Gate {
            name: "nope".into(),
            program: "definitely-no-such-binary-xyz".into(),
            ..Default::default()
        };
        let r = run_gate(std::env::temp_dir().as_path(), &gate, 30).await;
        assert!(!r.passed);
    }

    #[tokio::test]
    async fn run_gates_runs_all_in_order() {
        let g = |n: &str| Gate {
            name: n.into(),
            program: "git".into(),
            args: vec!["--version".into()],
            ..Default::default()
        };
        let results = run_gates(std::env::temp_dir().as_path(), &[g("a"), g("b")], 30).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "a");
        assert_eq!(results[1].name, "b");
    }
}
