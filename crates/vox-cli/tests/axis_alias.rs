// crates/vox-cli/tests/axis_alias.rs
#![cfg(feature = "gui")]
use clap::Parser;
use vox_cli::{Cli, VoxCliRoot};

#[test]
fn vox_axis_is_an_alias_for_gui() {
    let parsed = VoxCliRoot::try_parse_from(["vox", "axis"]).expect("`vox axis` should parse");
    assert!(
        matches!(parsed.cmd, Cli::Gui { .. }),
        "`vox axis` must resolve to the Gui subcommand"
    );
}
