//! Integration tests for Tier::MicroVm planner (P6-T3).

use vox_skill_runtime::{MicroVmRuntime, SkillRuntime, Tier, plan_for_min_tier};

#[test]
fn microvm_runtime_is_not_available() {
    let rt = MicroVmRuntime::new("firecracker");
    assert!(!rt.available());
}

#[test]
fn microvm_runtime_tier() {
    let rt = MicroVmRuntime::new("kata");
    assert_eq!(rt.tier(), Tier::MicroVm);
}

#[test]
fn microvm_runtime_build_returns_err() {
    let rt = MicroVmRuntime::new("firecracker");
    let opts = vox_skill_runtime::BuildOpts {
        context_dir: std::path::PathBuf::from("/tmp"),
        artifact_path: None,
        tag: "test".to_string(),
        build_args: vec![],
    };
    assert!(rt.build(&opts).is_err());
}

#[test]
fn microvm_runtime_run_returns_err() {
    let rt = MicroVmRuntime::new("firecracker");
    let opts = vox_skill_runtime::RunOpts::default();
    assert!(rt.run(&opts).is_err());
}

#[test]
fn tier_ordering() {
    assert!(Tier::BareMetal < Tier::Wasm);
    assert!(Tier::Wasm < Tier::Container);
    assert!(Tier::Container < Tier::MicroVm);
}

#[test]
fn plan_for_min_tier_wasm_always_works() {
    let choice = plan_for_min_tier(Tier::Wasm);
    assert!(choice.is_ok());
    assert_eq!(choice.unwrap(), vox_skill_runtime::RuntimeChoice::Wasm);
}

#[test]
fn plan_for_min_tier_baremetal_returns_wasm() {
    let choice = plan_for_min_tier(Tier::BareMetal);
    assert!(choice.is_ok());
    assert_eq!(choice.unwrap(), vox_skill_runtime::RuntimeChoice::Wasm);
}

#[test]
fn plan_for_min_tier_microvm_always_errors() {
    let result = plan_for_min_tier(Tier::MicroVm);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("MicroVm") || msg.contains("micro-VM") || msg.contains("v1.x"));
}
