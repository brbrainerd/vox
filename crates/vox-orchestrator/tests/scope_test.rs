use std::path::PathBuf;
use vox_orchestrator::{AgentId, EventBus, ScopeCheckResult, ScopeEnforcement, ScopeGuard};

#[test]
fn test_scope_guard() {
    let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
    let agent1 = AgentId(1);
    let bus = EventBus::new(16);

    guard.assign_file(agent1, PathBuf::from("src/main.rs"));
    guard.assign_file(agent1, PathBuf::from("Cargo.toml"));

    // Allowed matches
    assert_eq!(
        guard.check_write(agent1, std::path::Path::new("src/main.rs"), &bus),
        ScopeCheckResult::Allowed
    );
    assert_eq!(
        guard.check_write(agent1, std::path::Path::new("Cargo.toml"), &bus),
        ScopeCheckResult::Allowed
    );

    // Denied match
    match guard.check_write(agent1, std::path::Path::new("src2/main.rs"), &bus) {
        ScopeCheckResult::Denied(_) => {} // expected
        _ => panic!("Should be denied"),
    }

    // Unrestricted match
    let agent2 = AgentId(2);
    assert_eq!(
        guard.check_write(agent2, std::path::Path::new("anything"), &bus),
        ScopeCheckResult::Allowed
    );

    // Warn Mode test
    guard.set_enforcement(ScopeEnforcement::Warn);
    match guard.check_write(agent1, std::path::Path::new("out_of_scope.rs"), &bus) {
        ScopeCheckResult::Warned(_) => {} // expected
        _ => panic!("Should be warned"),
    }
}
