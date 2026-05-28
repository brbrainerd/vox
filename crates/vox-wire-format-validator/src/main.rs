//! CLI entry point for the wire-format SSOT drift check.
//!
//! Usage:
//!   cargo run -p vox-wire-format-validator            # check; exit 0 = ok, 1 = drift
//!   cargo run -p vox-wire-format-validator -- --update # rewrite expected_hash.rs with current hash

use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let update = args.iter().any(|a| a == "--update");

    // Walk up from the binary's location to find the repo root (contains Cargo.toml
    // at workspace level). Fallback to the current directory if not found.
    let repo_root = find_repo_root().unwrap_or_else(|| PathBuf::from("."));

    if update {
        run_update(&repo_root)
    } else {
        run_check(&repo_root)
    }
}

fn run_check(repo_root: &Path) -> anyhow::Result<()> {
    match vox_wire_format_validator::check_ssot_drift(repo_root) {
        Ok(()) => {
            println!(
                "✓ wire-format SSOT matches expected hash ({})",
                &vox_wire_format_validator::EXPECTED_SSOT_HASH[..16]
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn run_update(repo_root: &Path) -> anyhow::Result<()> {
    let hash = vox_wire_format_validator::compute_ssot_hash(repo_root)?;
    let expected_hash_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/expected_hash.rs");
    let new_content = format!(
        "/// blake3 hex digest of `docs/src/architecture/wire-format-v1-ssot.md`.\n\
         ///\n\
         /// Regenerate with:\n\
         /// ```pwsh\n\
         /// cargo run -p vox-wire-format-validator -- --update\n\
         /// ```\n\
         /// Commit the resulting change to this file alongside any Contract IR update.\n\
         pub const EXPECTED_SSOT_HASH: &str = \"{hash}\";\n"
    );
    std::fs::write(&expected_hash_path, new_content)?;
    println!(
        "✓ updated expected hash to {}\n  file: {}",
        &hash[..16],
        expected_hash_path.display()
    );
    println!("\nCommit src/expected_hash.rs alongside your Contract IR or SSOT change.");
    Ok(())
}

fn find_repo_root() -> Option<PathBuf> {
    // Start from the manifest directory of this binary and walk up until we
    // find a Cargo.toml that contains [workspace].
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dir = start.as_path();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists()
            && let Ok(content) = std::fs::read_to_string(&candidate)
            && content.contains("[workspace]")
        {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}
