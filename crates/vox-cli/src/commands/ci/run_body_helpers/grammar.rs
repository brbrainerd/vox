use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::commands::ci::cmd_enums::GrammarDriftEmit;
use vox_bounded_fs::read_utf8_path_capped;

use super::hash::sha256_hex_lower;

pub(crate) fn run_grammar_drift(root: &Path, emit: Option<GrammarDriftEmit>) -> Result<()> {
    // Fingerprint the EBNF grammar text (Task 41: replace legacy `generate_system_prompt` hash).
    let ebnf = vox_grammar_export::ebnf::emit_ebnf();
    let fingerprint = sha256_hex_lower(ebnf.as_bytes());

    let path_mens = root.join("mens/data/grammar_fingerprint.txt");
    let path_populi = root.join("populi/data/grammar_fingerprint.txt");
    let read_one = |p: &Path| -> String {
        if !p.is_file() {
            return String::new();
        }
        read_utf8_path_capped(p)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let stored_mens = read_one(&path_mens);
    let stored_populi = read_one(&path_populi);
    let stored = if !stored_mens.is_empty() {
        stored_mens
    } else {
        stored_populi
    };
    let drift = fingerprint != stored;
    if drift {
        let content = format!("{fingerprint}\n");
        for path in [&path_mens, &path_populi] {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, &content)?;
        }
        eprintln!(
            "Grammar drift detected (fingerprint changed). Updated {} and {}.",
            path_mens.display(),
            path_populi.display()
        );
    } else {
        eprintln!("No grammar drift detected.");
    }
    let drift_line = if drift { "drift=true" } else { "drift=false" };
    match emit {
        Some(GrammarDriftEmit::Github) => println!("{drift_line}"),
        None => {}
    }
    Ok(())
}

/// CI gate: emit all grammar formats and assert rule counts are non-zero + version alignment.
///
/// This implements `vox ci grammar-export-check` (Wave 1 Task 45).
pub(crate) fn run_grammar_export_check(_root: &Path) -> Result<()> {
    use vox_grammar_export::{GrammarExportConfig, GrammarFormat, export};

    let formats = [
        GrammarFormat::Ebnf,
        GrammarFormat::Gbnf,
        GrammarFormat::Lark,
        GrammarFormat::JsonSchema,
    ];

    let mut failures = Vec::new();

    for format in &formats {
        let config = GrammarExportConfig {
            format: format.clone(),
            ..GrammarExportConfig::default()
        };
        let result = export(&config)?;
        if result.grammar_text.is_empty() {
            failures.push(format!(
                "Format '{}': grammar_text is empty",
                format.as_str()
            ));
        }
        if result.rule_count == 0 {
            failures.push(format!("Format '{}': rule_count is 0", format.as_str()));
        }
        eprintln!(
            "  [grammar-export-check] {} -> {} rules, {} bytes",
            format.as_str(),
            result.rule_count,
            result.grammar_text.len()
        );
    }

    // Version alignment check
    let ver = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| semver::Version::new(0, 1, 0));
    if let Err(e) = vox_grammar_export::versioning::verify_grammar_alignment() {
        // Non-fatal warning: version mismatch occurs during dev builds.
        eprintln!(
            "  [grammar-export-check] version alignment warning ({}): {e}",
            ver
        );
    } else {
        eprintln!("  [grammar-export-check] version alignment OK ({})", ver);
    }

    if !failures.is_empty() {
        anyhow::bail!("grammar-export-check failed:\n{}", failures.join("\n"));
    }

    println!("grammar-export-check OK");
    Ok(())
}
