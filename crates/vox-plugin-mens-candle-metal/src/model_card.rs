//! Human-readable `MODEL_CARD.md` next to checkpoints.
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/model_card.rs` (SP3 sub-batch C).

use std::path::Path;

pub struct ModelCard {
    pub title: String,
    pub base_model: Option<String>,
    pub train_file: String,
    pub vocab_size: usize,
    pub d_model: usize,
    pub n_layers: usize,
    pub n_heads: usize,
    pub notes: String,
}

pub fn write(out_dir: &Path, card: &ModelCard) -> anyhow::Result<()> {
    let mut s = String::new();
    s.push_str("# ");
    s.push_str(&card.title);
    s.push_str("\n\n");
    if let Some(ref b) = card.base_model {
        s.push_str("## Base model\n");
        s.push_str(b);
        s.push_str("\n\n");
    }
    s.push_str("## Data\n");
    s.push_str(&format!("- train file: `{}`\n", card.train_file));
    s.push_str("\n## Architecture\n");
    s.push_str(&format!(
        "- vocab: {}\n- d_model: {}\n- layers: {}\n- heads: {}\n\n",
        card.vocab_size, card.d_model, card.n_layers, card.n_heads
    ));
    s.push_str("## Notes\n");
    s.push_str(&card.notes);
    s.push('\n');
    std::fs::write(out_dir.join("MODEL_CARD.md"), s)?;
    Ok(())
}
