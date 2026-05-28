//! `vox bundle list` — print all bundle definitions with resolved plugin counts.

use anyhow::Result;

pub fn run() -> Result<()> {
    let bundles = vox_plugin_catalog::all_bundles();

    println!("{:<25} {:>8}  DESCRIPTION", "ID", "PLUGINS");
    println!("{}", "-".repeat(80));

    for b in bundles {
        // Resolve the full plugin set via the extends chain.
        let count = match vox_plugin_catalog::bundle_resolved(&b.id) {
            Ok(plugins) => plugins.len(),
            Err(e) => {
                eprintln!("  [resolve error for '{}': {}]", b.id, e);
                0
            }
        };
        let extends_note = if let Some(ref parent) = b.extends {
            format!(" (extends: {})", parent)
        } else {
            String::new()
        };
        println!(
            "{:<25} {:>8}  {}{}",
            b.id, count, b.description, extends_note
        );
    }
    println!();
    println!("{} bundle(s) defined.", bundles.len());
    Ok(())
}
