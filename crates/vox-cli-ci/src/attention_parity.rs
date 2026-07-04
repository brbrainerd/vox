use anyhow::{Result, anyhow};
use std::fs;
use std::path::Path;

pub fn run(root: &Path) -> Result<()> {
    let orch_config = root.join("crates/vox-orchestrator/src/config.rs");
    let mc_handlers = root.join("crates/vox-mcp/src/memory/handlers_preferences.rs");
    let cli_attention = root.join("crates/vox-cli/src/commands/attention.rs");

    if !orch_config.exists() || !mc_handlers.exists() || !cli_attention.exists() {
        return Ok(());
    }

    let orch_code = fs::read_to_string(&orch_config)?;
    let mc_code = fs::read_to_string(&mc_handlers)?;
    let cli_code = fs::read_to_string(&cli_attention)?;

    let required_keys = [
        "attention_enabled",
        "attention_budget_ms",
        "attention_alert_threshold",
    ];

    for key in required_keys {
        if !orch_code.contains(key) {
            return Err(anyhow!("Orchestrator config is missing {} mapping", key));
        }
        if !mc_code.contains(key) {
            return Err(anyhow!("MCP preference handler is missing {} mapping", key));
        }
        if !cli_code.contains(key) {
            return Err(anyhow!("CLI attention is missing {} mapping", key));
        }
    }

    println!("attention-config-parity OK");
    Ok(())
}
