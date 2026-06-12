use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type().is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
    }
    Ok(())
}

pub fn run(root: &Path) -> Result<()> {
    let mcp_dir = root.join("crates/vox-mcp/src");

    if !mcp_dir.exists() {
        return Ok(());
    }

    let mut evaluate_files = Vec::new();
    let mut all_rs_files = Vec::new();
    let _ = visit_dirs(&mcp_dir, &mut all_rs_files);

    for path in all_rs_files {
        let content = fs::read_to_string(&path)?;
        if content.contains("evaluate_interruption") {
            if !content.contains("record_attention_event")
                && !content.contains("AttentionEventType::")
                && !path.to_string_lossy().contains("interruption_policy.rs")
                && !path.to_string_lossy().contains("lib.rs")
                && !path.to_string_lossy().contains("mod.rs")
                && !path.to_string_lossy().contains("attention_policy.rs")
            {
                evaluate_files.push(path.to_path_buf());
            }
        }
    }

    if !evaluate_files.is_empty() {
        let files: Vec<String> = evaluate_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        return Err(anyhow!(
            "The following files call `evaluate_interruption` but lack `record_attention_event` (attention-event-ledger-parity failure):\n{:#?}",
            files
        ));
    }

    println!("attention-event-ledger-parity OK");
    Ok(())
}
