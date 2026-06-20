use vox_terminal_core::pty::{default_shell, shell_integration_snippet};

#[test]
fn default_shell_nonempty() {
    assert!(!default_shell().is_empty());
}

#[test]
fn snippet_supports_pwsh_and_bash() {
    assert!(shell_integration_snippet("pwsh").is_some());
    assert!(shell_integration_snippet("bash").is_some());
    assert!(shell_integration_snippet("fish").is_none()); // until Track 6
}

#[test]
fn snippet_handles_exe_suffix() {
    assert!(shell_integration_snippet("PowerShell.EXE").is_some());
    // vox-arch-check: allow abs-path
    assert!(shell_integration_snippet("/usr/bin/bash").is_some());
}

#[test]
fn snippet_emits_osc633_markers() {
    assert!(shell_integration_snippet("pwsh").unwrap().contains("]633;"));
    assert!(shell_integration_snippet("bash").unwrap().contains("]633;"));
}
