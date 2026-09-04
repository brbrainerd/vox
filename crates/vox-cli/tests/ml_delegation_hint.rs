//! The hint printed when `vox-ml-cli` is missing must name a command that
//! actually produces the delegated subcommand.
//!
//! `populi` is not in vox-ml-cli's `default = ["mens-base"]`, so the bare
//! `cargo install --path crates/vox-ml-cli` this message used to print yields a
//! binary whose `populi` subcommand is `#[cfg]`-ed out — the user follows the
//! advice, gets the identical error, and has no way to tell why.

#[test]
fn ml_cli_install_hint_enables_the_populi_feature() {
    let src = include_str!("../src/main.rs");
    let hint = src
        .lines()
        .find(|l| l.contains("cargo install --path crates/vox-ml-cli"))
        .expect("the delegation failure message must name the install command");
    assert!(
        hint.contains("--features populi"),
        "install hint must enable the `populi` feature, got: {hint}"
    );
}
