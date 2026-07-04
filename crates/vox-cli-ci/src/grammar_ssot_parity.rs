use anyhow::{Result, anyhow};
use vox_compiler::feature_matrix::DecoratorFeature;
use vox_compiler::language_surface;
use vox_grammar_export::ssot_markdown;

fn check_decorator_feature_lexer_parity() -> Result<()> {
    if let Some(detail) = language_surface::decorator_feature_lexer_parity_mismatch() {
        return Err(anyhow!(
            "script surface drift: {detail} — sync language_surface.rs with feature_matrix.rs"
        ));
    }
    Ok(())
}

pub async fn run() -> Result<()> {
    let repo_root = crate::repo_root();
    let ssot_path = repo_root.join("tree-sitter-vox").join("GRAMMAR_SSOT.md");

    if !ssot_path.exists() {
        return Err(anyhow!(
            "GRAMMAR_SSOT.md not found at {}",
            ssot_path.display()
        ));
    }

    let current_ssot = std::fs::read_to_string(&ssot_path)?;
    let expected_ssot = ssot_markdown::emit_ssot_markdown();

    if current_ssot.trim() != expected_ssot.trim() {
        eprintln!(
            "Error: GRAMMAR_SSOT.md is stale vs the language-surface SSOT (vox-language-surface, \
             re-exported by vox-compiler::language_surface). The exporter now renders directly \
             from that SSOT, so this means the checked-in doc was not regenerated after a \
             keyword/decorator change."
        );
        eprintln!(
            "Run `vox grammar --format ssot-markdown --output tree-sitter-vox/GRAMMAR_SSOT.md` to update."
        );
        return Err(anyhow::anyhow!("Grammar SSOT parity check failed"));
    }

    check_decorator_feature_lexer_parity()?;

    println!("GRAMMAR_SSOT.md is in sync with the language-surface SSOT.");
    println!(
        "script surface enum parity OK ({} decorators, {} total features)",
        DecoratorFeature::ALL.len(),
        vox_compiler::feature_matrix::Feature::all().len()
    );
    Ok(())
}
