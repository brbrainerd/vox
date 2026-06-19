//! End-to-end pipeline proof: delegate a Python file → py_compile gate → green → ledger append.
//!
//! Gated with #[ignore] so CI never bills Antigravity credits. Run with:
//!   cargo test -p vox-orchestrator-mcp smoke_e2e_pipeline_python_compile -- --ignored --nocapture
//!
//! Prerequisites:
//!   - `agy` v1.0.9+ on PATH, interactive Google login complete
//!   - `python3` (or `python`) on PATH
//!   - Run from the repo root (a git work tree with committed HEAD)

use vox_orchestrator_mcp::agy_doctor::{AgyStatus, detect};
use vox_orchestrator_mcp::agy_exec::{AgyExec, AgySpec};
use vox_orchestrator_mcp::agy_gates::{Gate, run_gates};
use vox_orchestrator_mcp::agy_pipeline::classify_outcome;
use vox_orchestrator_mcp::agy_worktree::DelegationWorktree;

fn python_binary() -> Option<String> {
    for candidate in &["python3", "python"] {
        if which::which(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

#[tokio::test]
#[ignore = "live agy call — bills Antigravity credits"]
async fn smoke_e2e_pipeline_python_compile() {
    // 1. Pre-flight: agy must be ready.
    assert!(matches!(detect(), AgyStatus::Ready { .. }), "agy must be ready");

    // 2. Python must be available for the compile gate.
    let python = python_binary().expect("python3 or python must be on PATH");

    let repo_root = std::env::current_dir().expect("cwd");
    let slug = format!("e2e-{}", uuid::Uuid::new_v4().simple());
    let wt = DelegationWorktree::create(&repo_root, &slug).await.expect("worktree");

    // 3. Delegate: write a syntactically valid Python file.
    let exec = AgyExec::new(&wt.path);
    let spec = AgySpec {
        task: "Create a file named hello.py in the current directory containing exactly:\n\
               def greet(name: str) -> str:\n\
                   return f\"Hello, {name}!\"\n\n\
               if __name__ == \"__main__\":\n\
                   print(greet(\"world\"))\n\n\
               Use your file-writing tools. Do not run any git commands. — no other files."
            .into(),
        model: None,
        timeout_secs: 180,
    };
    let out = exec.run(&spec).await.expect("agy spawn");

    // 4. Capture before asserting.
    let (_diff, files_changed) = wt.capture().await.expect("capture");

    // 5. Gate: py_compile must pass on the written file.
    let hello_py = wt.path.join("hello.py");
    let gates = vec![Gate {
        name: "py_compile".into(),
        program: python.clone(),
        args: vec![
            "-m".into(),
            "py_compile".into(),
            hello_py.to_string_lossy().to_string(),
        ],
        ..Default::default()
    }];
    let results = run_gates(&wt.path, &gates, 30).await;

    // 6. Classify.
    let outcome = classify_outcome(files_changed, &results, out.timed_out);

    // 7. Clean up before asserting.
    wt.cleanup(&repo_root).await.expect("cleanup");

    eprintln!(
        "exit={} timed_out={} files_changed={} py_compile_passed={} outcome={outcome}",
        out.exit_code, out.timed_out, files_changed, results[0].passed,
    );

    assert!(!out.timed_out, "agy timed out");
    assert_eq!(out.exit_code, 0, "agy exited non-zero");
    assert!(files_changed > 0, "agy must have written hello.py");
    assert!(results[0].passed, "py_compile gate must pass — hello.py has a syntax error");
    assert_eq!(outcome, "green", "files written + gate passed => green");
}
