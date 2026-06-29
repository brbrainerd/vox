//! Guard tests for the `cli:` ingest catalog serialization (plan vs1, T6).
//!
//! `cli_catalog_json()` must emit a `CommandCatalog`-shaped JSON object whose leaf
//! set includes the feature-gated `mens`/`populi`/`oratio` subcommands even in a
//! default (non-gated) binary, so the structural index can ingest the full CLI tree.

/// `build_catalog()` (inside `cli_catalog_json`) walks the full clap tree, which
/// recurses deeply enough to overflow the default libtest worker-thread stack on
/// Windows. Run it on a thread with an ample stack — same workaround as the
/// `command_catalog_paths_baseline` test.
fn on_big_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .expect("spawn big-stack thread")
        .join()
        .expect("big-stack thread panicked")
}

#[test]
fn cli_catalog_json_includes_gated_mens_populi_oratio() {
    let json = on_big_stack(vox_cli::commands::graphify::cli_catalog_json);
    // Gated groups must be present even in a default binary (recovered from vox-ml-cli enums).
    for g in ["mens", "populi", "oratio"] {
        assert!(
            json.contains(g),
            "gated group {g} missing from cli catalog json"
        );
    }
    // Parses as a CommandCatalog-shaped object.
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(
        v.get("entries")
            .and_then(|e| e.as_array())
            .map(|a| a.len() > 100)
            .unwrap_or(false)
    );
}
