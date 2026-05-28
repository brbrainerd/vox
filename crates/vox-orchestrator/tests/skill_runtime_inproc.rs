//! P0-T7 acceptance: in-process executor behind `SkillRuntime` trait.

use vox_orchestrator::skill_runtime_inproc::InProcessSkillRuntime;
use vox_skill_runtime::{RunOpts, SkillRuntime};

#[test]
fn in_process_runtime_is_available() {
    let rt = InProcessSkillRuntime::new();
    assert!(rt.available());
    assert_eq!(rt.name(), "inproc");
}

#[test]
fn in_process_runtime_runs_a_trivial_command() {
    let rt = InProcessSkillRuntime::new();
    let opts = RunOpts::default();
    let outcome = rt.run(&opts).expect("run succeeded");
    assert_eq!(outcome.exit_code, 0);
}

#[test]
fn run_with_secrets_appends_env_but_does_not_leak() {
    let rt = InProcessSkillRuntime::new();
    let opts = RunOpts::default();
    let secret_env = vec![("SUPER_SECRET".to_string(), "hunter2".to_string())];
    let outcome = rt
        .run_with_secrets(&opts, &secret_env)
        .expect("run_with_secrets succeeded");
    assert_eq!(outcome.exit_code, 0);
    assert!(!outcome.stdout.contains("hunter2"));
    assert!(!outcome.stderr.contains("hunter2"));
}
