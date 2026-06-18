//! Guard: publication approval/decision paths must never consult gamify state.

#[test]
fn publication_decision_modules_do_not_reference_gamify() {
    let roots = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vox-cli/src/commands/db/publication/decision.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vox-publisher/src/publication_worthiness.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vox-scientia/src/review_flow.rs"
        )),
    ];
    for src in roots {
        assert!(
            !src.contains("vox_gamify")
                && !src.contains("gamify_enabled")
                && !src.contains("ludus_channel")
                && !src.contains("credit_kudos"),
            "publication path must not gate on gamify"
        );
    }
}
