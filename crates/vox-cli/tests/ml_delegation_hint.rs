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

/// Persona split (spec §9.1): the `cargo install --path …` hint above is only
/// correct for a contributor (a source checkout with a Rust toolchain). An
/// installed end user has neither, so the failure path must be gated on
/// `contributor_mode::is_contributor_mode()` and the non-contributor message
/// must not tell them to run `cargo` or reference a repo-relative `crates/`
/// path.
#[test]
fn ml_cli_delegation_failure_gates_the_cargo_hint_on_contributor_mode() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains("contributor_mode::is_contributor_mode()"),
        "the vox-ml-cli delegation failure path must check contributor mode \
         before printing the `cargo install --path crates/vox-ml-cli` hint"
    );

    // First line containing the cargo hint is the contributor-mode branch
    // (per Correction 1 of the task-5 brief, `.find()` semantics apply).
    let first_cargo_hint_idx = src
        .lines()
        .position(|l| l.contains("cargo install --path crates/vox-ml-cli"))
        .expect("contributor-mode cargo hint must exist");

    // Every line strictly after that one, up to the closing of this error
    // arm, must not itself contain the cargo-install hint — i.e. there is
    // exactly one such line, and it lives in the contributor branch.
    let later_lines: Vec<&str> = src.lines().skip(first_cargo_hint_idx + 1).collect();
    let non_contributor_hint = later_lines
        .iter()
        .find(|l| l.contains("Install the Vox 'full' tier"))
        .expect("installed-user remedy line must exist after the contributor branch");
    assert!(
        !non_contributor_hint.contains("cargo "),
        "installed-user remedy must not mention cargo, got: {non_contributor_hint}"
    );
    assert!(
        !non_contributor_hint.contains("crates/"),
        "installed-user remedy must not reference a repo-relative path, got: {non_contributor_hint}"
    );
}
