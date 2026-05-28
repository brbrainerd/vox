//! P2-T3 acceptance: `vox workflow drain` CLI argument parsing.

use clap::Parser;
use vox_cli::VoxCliRoot;

#[test]
fn vox_workflow_drain_parses() {
    let cli =
        VoxCliRoot::try_parse_from(["vox", "workflow", "drain", "--version", &"a".repeat(128)]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}

#[test]
fn vox_workflow_ls_parses() {
    let cli = VoxCliRoot::try_parse_from(["vox", "workflow", "ls"]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}

#[test]
fn vox_workflow_ls_draining_flag_parses() {
    let cli = VoxCliRoot::try_parse_from(["vox", "workflow", "ls", "--draining"]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}
